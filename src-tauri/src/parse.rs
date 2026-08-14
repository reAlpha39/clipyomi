// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The parse worker: one task, latest-wins, panics contained.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use jparser::{BoundaryHints, ParseOptions, ParseResult};
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::state::AppState;

/// Emitted with a `ParseResult` payload whenever a parse succeeds.
pub const PARSE_RESULT_EVENT: &str = "parse-result";
/// Emitted with a `String` payload when a parse fails or panics.
pub const PARSE_ERROR_EVENT: &str = "parse-error";

/// Run a parse, converting a panic into an error rather than an abort.
///
/// `AssertUnwindSafe` is sound here: the managed state is read-only after
/// startup and no `&mut` crosses this boundary, so there is no invariant a
/// half-completed parse could leave broken.
pub fn catch_parse<F>(input_len: usize, f: F) -> Result<ParseResult, String>
where
    F: FnOnce() -> Result<ParseResult, String>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => Err(format!(
            "the parser panicked on an input of {input_len} characters; \
             the previous result is still shown"
        )),
    }
}

/// Wait for new input and return the newest value, skipping anything superseded.
///
/// `borrow_and_update` is what makes this latest-wins: a `watch` channel keeps
/// only its most recent value, so inputs that arrived during a parse are already
/// collapsed by the time we look.
pub async fn next_input(rx: &mut watch::Receiver<String>) -> Option<String> {
    rx.changed().await.ok()?;
    let text = rx.borrow_and_update().clone();
    Some(text)
}

/// Parse each new input and emit the outcome to the webview.
pub async fn run_worker(app: AppHandle, state: Arc<AppState>, mut rx: watch::Receiver<String>) {
    while let Some(text) = next_input(&mut rx).await {
        let state = Arc::clone(&state);
        let len = text.chars().count();

        let outcome = tauri::async_runtime::spawn_blocking(move || {
            catch_parse(len, || {
                let flags = state.hints.as_ref().map(|t| t.hints(&text));
                jparser::parse(
                    &state.index,
                    &state.table,
                    &text,
                    &ParseOptions::default(),
                    flags.as_ref().map(|f| f as &dyn BoundaryHints),
                )
                .map_err(|e| e.to_string())
            })
        })
        .await;

        let emitted = match outcome {
            Ok(Ok(result)) => app.emit(PARSE_RESULT_EVENT, result),
            Ok(Err(message)) => app.emit(PARSE_ERROR_EVENT, message),
            Err(e) => app.emit(
                PARSE_ERROR_EVENT,
                format!("the parse task failed to run: {e}"),
            ),
        };

        if let Err(e) = emitted {
            // Nothing to fall back on: if the event cannot reach the webview
            // there is no other channel to report it through. Log and continue
            // so one failed emit does not end monitoring.
            eprintln!("emitting a parse event failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port design's rule is "stop and rerun", not "queue": when text
    /// arrives faster than parsing, the worker must skip to the newest rather
    /// than working through a backlog the user can no longer see.
    #[tokio::test]
    async fn the_worker_sees_only_the_newest_input() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        tx.send("one".to_string()).expect("send");
        tx.send("two".to_string()).expect("send");
        tx.send("three".to_string()).expect("send");

        assert_eq!(next_input(&mut rx).await.as_deref(), Some("three"));
    }

    /// A dropped sender ends the worker rather than spinning.
    #[tokio::test]
    async fn a_closed_channel_ends_the_worker() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        drop(tx);
        assert_eq!(next_input(&mut rx).await, None);
    }

    #[test]
    fn catch_parse_returns_the_value_when_nothing_panics() {
        let out = catch_parse(3, || Ok(ParseResult { segments: vec![] })).expect("ok");
        assert!(out.segments.is_empty());
    }

    /// The matcher does offset arithmetic over the whole input. A panic there
    /// must not take the app down — the previous result stays on screen and the
    /// worker lives to parse the next copy.
    #[test]
    fn catch_parse_contains_a_panic_and_names_the_input_length() {
        let err = catch_parse(4096, || panic!("offset out of range")).expect_err("must be Err");
        assert!(err.contains("4096"), "got {err}");
    }

    /// Ported from `commands::tests::a_parse_failure_is_reported_as_its_display_string`:
    /// Task 4 deletes `run_parse`, which would otherwise orphan this test. It
    /// moves here because `catch_parse` is where that `ParseError`-to-`String`
    /// mapping now lives, and this is the crate's only test that drives a real
    /// `ParseError` rather than a fabricated string.
    ///
    /// `catch_parse`'s `Ok(result) => result` arm needs a real `ParseError` to
    /// exercise, not a fabricated string — this drives one by corrupting a
    /// freshly built index's payload file. Corruption happens *before*
    /// `Index::open`, never after: mutating a file underneath a live `Index`
    /// is documented UB (see `jparser::index::load::Index::open`'s doc
    /// comment), so this builds a working index, drops it, corrupts the file
    /// on disk, and only then opens the corrupted copy this test queries.
    #[test]
    fn a_parse_failure_is_reported_as_its_display_string() {
        use jparser::index::load::Index;

        let root = crate::test_support::scratch("parse-worker-failure");
        let generation = crate::test_support::build_index_generation(&root);

        // A 4-byte `records.bin` cannot hold a valid length-prefixed blob at
        // any real offset, so the first lookup that reaches it fails no
        // matter what that offset actually is.
        std::fs::write(generation.join(jparser::index::RECORDS_FILE), [0xFFu8; 4])
            .expect("corrupt the records file");

        let index = Index::open(&generation).expect("header.bin and entries.idx are untouched");
        let table = jparser::conjugation::ConjugationTable::load_embedded().expect("table");

        // "本" is the fixture's own headword (see `test_support`), so this is
        // a real FST hit that then fails reading its record payload back —
        // the actual `Err` path `catch_parse` exists to map, not a fabrication.
        let text = "本";
        let err = catch_parse(text.chars().count(), || {
            jparser::parse(&index, &table, text, &ParseOptions::default(), None)
                .map_err(|e| e.to_string())
        })
        .expect_err("a corrupt records file must fail the parse");
        assert!(!err.is_empty(), "parse error message must not be empty");
    }
}
