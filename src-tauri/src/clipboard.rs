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
use tokio::sync::{watch, Notify};

use crate::state::SettingsState;

/// Poll interval. Port design §6.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Longest input worth parsing, in characters.
///
/// A soundness guard rather than a performance one: the matcher does offset
/// arithmetic over the whole input, and an unbounded paste is the cheapest way
/// to find out what that costs.
pub const MAX_INPUT_CHARS: usize = 10_000;

/// Kana or a CJK ideograph — ta-old's test for "is this worth parsing".
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

/// How long `wait_for_frontend` waits for the readiness signal before giving
/// up and logging it, rather than hanging forever with no diagnostic.
///
/// Generous on purpose: this is a check for "the signal never arrives at
/// all" (a webview that failed to load, or a `listen()` call that rejected —
/// see `main.ts`'s own comment on why that specific failure is left
/// unswallowed), not a deadline tuned to ordinary startup latency. A slow
/// machine loading a webview must not trip it.
const FRONTEND_READY_TIMEOUT: Duration = Duration::from_secs(30);

/// Blocks until the webview has registered its `parse-result` /
/// `parse-error` listeners (see `commands::frontend_ready`, which fires this),
/// or until `timeout` elapses. Returns `false` and logs a warning in the
/// latter case.
///
/// Without this gate, `run_poll`'s first tick can read clipboard text left
/// over from before launch, decide it is worth parsing, and record it as
/// `last_seen` — all before the webview's `listen()` calls have finished
/// their async round-trip to register on the Rust side. The `emit` that
/// follows then reaches zero listeners and is silently dropped (`emit`
/// returns `Ok` either way), and because `last_seen` is already set, the same
/// clipboard text is never retried: the user sees an empty pane until they
/// copy something different. Waiting here closes that window deterministically
/// — no delay to tune, no guess about how long the webview takes to load.
///
/// Split from `wait_for_frontend` only so a test can drive it with a
/// millisecond-scale `timeout` instead of the real 30-second production one.
async fn wait_for_frontend_within(ready: &Notify, timeout: Duration) -> bool {
    if tokio::time::timeout(timeout, ready.notified())
        .await
        .is_ok()
    {
        return true;
    }
    // Not a panic and not fatal: `run_poll`'s caller decides what to do with
    // `false` (see its own doc). This is purely so the hang has a visible
    // cause in the logs instead of looking like the app silently did
    // nothing.
    eprintln!(
        "the webview never signalled frontend_ready within {timeout:?}; \
         clipboard monitoring will not start this session"
    );
    false
}

async fn wait_for_frontend(ready: &Notify) -> bool {
    wait_for_frontend_within(ready, FRONTEND_READY_TIMEOUT).await
}

/// Poll the clipboard and push anything worth parsing into the input channel.
///
/// Runs for the life of the app. Pausing is a settings flag rather than a
/// stopped task: restarting a task on every toggle is more moving parts than
/// checking a bool five times a second.
pub async fn run_poll(
    app: AppHandle,
    tx: watch::Sender<String>,
    settings: Arc<SettingsState>,
    ready: Arc<Notify>,
) {
    if !wait_for_frontend(&ready).await {
        // Stay stopped rather than starting anyway: if the signal never
        // arrived, the webview's `listen()` calls never finished registering
        // either (the same handshake gates both — see `main.ts`), so
        // `parse-result`/`parse-error` cannot reach it regardless of what the
        // poll does. The manual Parse button is equally dead in that state.
        // Starting the loop here would reintroduce exactly the
        // dropped-first-parse race this gate exists to close, for a webview
        // that by now is almost certainly never coming back — 30 seconds is
        // long past any realistic load time.
        return;
    }

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

    /// The other three `is_japanese` blocks each have a case above; this one
    /// (CJK Unified Ideographs Extension A) did not, so a typo in its range
    /// could ship silently. `'㐀'` (U+3400) is the block's first codepoint.
    #[test]
    fn cjk_extension_a_is_worth_parsing() {
        assert!(should_parse("㐀", None, None));
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

    /// The seam behind Important 2's fix: nothing downstream of
    /// `wait_for_frontend` may run before the frontend signals ready, however
    /// long that takes. `run_poll` cannot be exercised directly (it needs a
    /// real `AppHandle` and touches the system clipboard, §7.1), so this
    /// drives the exact gate `run_poll` awaits first, using the same
    /// `Notify` type, with a `watch::Sender::send` standing in for the
    /// clipboard publish it guards.
    #[tokio::test]
    async fn wait_for_frontend_blocks_the_first_publish_until_signalled() {
        let ready = Arc::new(Notify::new());
        let (tx, mut rx) = watch::channel(String::new());

        let ready_clone = Arc::clone(&ready);
        let task = tokio::spawn(async move {
            let signalled = wait_for_frontend(&ready_clone).await;
            assert!(signalled, "expected the ready signal, not a timeout");
            tx.send("東京".to_string()).expect("send");
        });

        // Give the spawned task every chance to run before it is signalled.
        for _ in 0..16 {
            tokio::task::yield_now().await;
        }
        assert!(
            !rx.has_changed().expect("sender still alive"),
            "published before the frontend signalled ready"
        );

        ready.notify_one();
        task.await.expect("wait_for_frontend task panicked");

        assert_eq!(
            next_input_for_test(&mut rx).await.as_deref(),
            Some("東京"),
            "did not publish after the ready signal"
        );
    }

    /// Residual 3 (final review): a webview that never signals ready must
    /// not hang `wait_for_frontend` with no way to tell. Drives
    /// `wait_for_frontend_within` directly with a millisecond-scale timeout
    /// rather than waiting out the real 30-second production one — the
    /// `Notify` here is never signalled, so this is a guaranteed timeout on
    /// every run, not a race against anything.
    #[tokio::test]
    async fn wait_for_frontend_within_times_out_and_reports_false_when_never_signalled() {
        let ready = Notify::new();
        let signalled = wait_for_frontend_within(&ready, Duration::from_millis(20)).await;
        assert!(!signalled, "expected a timeout, not a signal");
    }

    /// Local stand-in for `parse::next_input` so this test does not need a
    /// dependency from `clipboard` onto `parse` just to read one value back.
    async fn next_input_for_test(rx: &mut watch::Receiver<String>) -> Option<String> {
        rx.changed().await.ok()?;
        Some(rx.borrow_and_update().clone())
    }
}
