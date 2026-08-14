// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The clipboard poll.
//!
//! The decision of whether a copy is worth parsing lives in `should_parse`,
//! which is pure. The loop around it is a thin shell — the system clipboard is
//! global state shared with the developer's own desktop, so no test touches it.

use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::watch;

use crate::state::SettingsState;

/// Poll interval. Port design §6.
#[allow(dead_code)] // Read by `run_poll` below; not reachable from `main` until
                    // Task 4 spawns the poll.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Longest input worth parsing, in characters.
///
/// A soundness guard rather than a performance one: the matcher does offset
/// arithmetic over the whole input, and an unbounded paste is the cheapest way
/// to find out what that costs.
#[allow(dead_code)] // Read by `should_parse` below and unit-tested directly;
                    // not reachable from `main` until Task 4 spawns the poll.
pub const MAX_INPUT_CHARS: usize = 10_000;

/// Kana or a CJK ideograph — ta-old's test for "is this worth parsing".
#[allow(dead_code)] // Called by `should_parse` below; not reachable from
                    // `main` until Task 4 spawns the poll.
fn is_japanese(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{309F}'   // hiragana
        | '\u{30A0}'..='\u{30FF}' // katakana
        | '\u{4E00}'..='\u{9FFF}' // CJK unified ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK unified ideographs extension A
    )
}

/// Whether a clipboard value should be parsed.
///
/// `last_written` is always `None` in this phase — nothing writes to the
/// clipboard yet. It is in the signature because this predicate is the thing
/// under test, and adding a parameter later would invalidate those tests.
#[allow(dead_code)] // Called by `run_poll` below and unit-tested directly; not
                    // reachable from `main` until Task 4 spawns the poll.
pub fn should_parse(text: &str, last_seen: Option<&str>, last_written: Option<&str>) -> bool {
    if last_seen == Some(text) || last_written == Some(text) {
        return false;
    }
    let mut chars = 0usize;
    let mut has_japanese = false;
    for c in text.chars() {
        chars += 1;
        if chars > MAX_INPUT_CHARS {
            return false;
        }
        has_japanese |= is_japanese(c);
    }
    has_japanese
}

/// Poll the clipboard and push anything worth parsing into the input channel.
///
/// Runs for the life of the app. Pausing is a settings flag rather than a
/// stopped task: restarting a task on every toggle is more moving parts than
/// checking a bool five times a second.
#[allow(dead_code)] // Task 4 spawns `run_poll` from `main.rs`; nothing calls it
                    // yet.
pub async fn run_poll(app: AppHandle, tx: watch::Sender<String>, settings: Arc<SettingsState>) {
    let mut last_seen: Option<String> = None;
    let mut read_failing = false;

    loop {
        tokio::time::sleep(POLL_INTERVAL).await;

        if !settings.monitoring_enabled() {
            continue;
        }

        // A read can fail transiently when another app holds the clipboard.
        // Skipped rather than surfaced: that is not information the user can
        // act on (design §5). Logged once per failure streak, not every
        // tick, so a persistent failure (e.g. a denied permission) is
        // debuggable without flooding stderr five times a second.
        let text = match app.clipboard().read_text() {
            Ok(text) => {
                if read_failing {
                    read_failing = false;
                    eprintln!("reading the clipboard recovered");
                }
                text
            }
            Err(e) => {
                if !read_failing {
                    read_failing = true;
                    eprintln!(
                        "reading the clipboard failed, skipping ticks until it recovers: {e}"
                    );
                }
                continue;
            }
        };

        if !should_parse(&text, last_seen.as_deref(), None) {
            continue;
        }

        last_seen = Some(text.clone());
        if tx.send(text).is_err() {
            // The worker is gone; nothing left to poll for.
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_text_is_worth_parsing() {
        assert!(should_parse("東京は", None, None));
        assert!(should_parse("とうきょう", None, None));
        assert!(should_parse("トウキョウ", None, None));
    }

    /// Every poll tick re-reads the clipboard; without this the parser would
    /// re-run five times a second on text that has not changed.
    #[test]
    fn unchanged_text_is_skipped() {
        assert!(!should_parse("東京", Some("東京"), None));
    }

    /// ta-old's rule. English, code, and URLs pass through the clipboard
    /// constantly and none of them are worth a parse.
    #[test]
    fn text_with_no_japanese_is_skipped() {
        assert!(!should_parse("hello world", None, None));
        assert!(!should_parse("https://example.com/path?q=1", None, None));
        assert!(!should_parse("", None, None));
    }

    /// Copying an entry out of our own definition pane must not trigger a
    /// re-parse of our own output.
    #[test]
    fn text_this_app_wrote_is_skipped() {
        assert!(!should_parse("東京", None, Some("東京")));
    }

    /// A soundness guard, not a performance one: the matcher does offset
    /// arithmetic over the whole input.
    #[test]
    fn text_over_the_cap_is_skipped() {
        let long: String = "あ".repeat(MAX_INPUT_CHARS + 1);
        assert!(!should_parse(&long, None, None));
    }

    #[test]
    fn text_exactly_at_the_cap_is_parsed() {
        let exact: String = "あ".repeat(MAX_INPUT_CHARS);
        assert!(should_parse(&exact, None, None));
    }

    /// The cap counts characters, not bytes: Japanese is three bytes a
    /// character in UTF-8, so a byte cap would reject a third of the intended
    /// length.
    #[test]
    fn the_cap_counts_characters_not_bytes() {
        let text: String = "あ".repeat(4000);
        assert!(
            text.len() > MAX_INPUT_CHARS,
            "precondition: byte length exceeds the cap"
        );
        assert!(should_parse(&text, None, None));
    }
}
