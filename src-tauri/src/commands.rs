// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The webview's two entry points: parse a string, and ask why startup failed.

use std::sync::Arc;

use jparser::conjugation::ConjugationTable;
use jparser::hints::VibratoTokenizer;
use jparser::index::load::Index;
use jparser::{BoundaryHints, ParseOptions, ParseResult};
use tauri::State;

use crate::state::{AppState, StartupFailure};

/// Parse `text`, applying hints when a tokenizer was loaded.
///
/// `index: None` yields an empty result rather than an error, but this branch
/// is test-only: in production it is unreachable. `parse_text` takes
/// `State<'_, Arc<AppState>>`, and Tauri's `State` extractor rejects the
/// invocation before this function's body ever runs when `AppState` is
/// unmanaged (the no-index case manages `StartupFailure` instead — see
/// `main.rs`'s `setup`). The branch exists only so `run_parse` can be
/// unit-tested without a live Tauri app or a real index fixture.
fn run_parse(
    index: Option<&Index>,
    table: &ConjugationTable,
    text: &str,
    hints: Option<&VibratoTokenizer>,
) -> Result<ParseResult, String> {
    let Some(index) = index else {
        return Ok(ParseResult { segments: vec![] });
    };
    let flags = hints.map(|t| t.hints(text));
    jparser::parse(
        index,
        table,
        text,
        &ParseOptions::default(),
        flags.as_ref().map(|f| f as &dyn BoundaryHints),
    )
    .map_err(|e| e.to_string())
}

/// Parse TEXT against the loaded index.
///
/// The parse runs on a blocking thread: `jparser::parse` is synchronous CPU work
/// over the whole input, and running it on the async runtime would stall the
/// webview. `tauri::State` is not `Send`, so the `Arc` is cloned out first and
/// the clone is what crosses the boundary.
#[tauri::command]
pub async fn parse_text(
    text: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ParseResult, String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        run_parse(
            Some(&state.index),
            &state.table,
            &text,
            state.hints.as_ref(),
        )
    })
    .await
    .map_err(|e| format!("the parse task failed to run: {e}"))?
}

/// The startup error, or `null` when startup succeeded.
///
/// Takes `State<'_, StartupFailure>` rather than `Option<State<'_, StartupFailure>>`:
/// the latter is not a valid Tauri 2 command parameter, since only `State`
/// itself implements `CommandArg`, not `Option<State<_>>`. `main`'s `setup`
/// manages `StartupFailure` unconditionally instead, empty string meaning
/// success.
#[tauri::command]
pub fn startup_error(failure: State<'_, StartupFailure>) -> Option<String> {
    if failure.0.is_empty() {
        None
    } else {
        Some(failure.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `parse_text` is a thin async wrapper, so the part worth testing is the
    /// piece that is not Tauri: turning parser output into the command's Result.
    /// A Tauri command needs a live app handle; this seam does not.
    ///
    /// This exercises `run_parse`'s `index: None` shortcut, not empty-input
    /// parsing against a real index — `text` here is non-empty on purpose, to
    /// show the empty result comes from the missing index, not from the input.
    /// `src-tauri` has no index fixture and `jparser`'s index-building test
    /// helpers are private to that crate, so a genuine empty-input parse is not
    /// cheaply reachable from this crate.
    #[test]
    fn a_missing_index_parses_to_no_segments() {
        let table = jparser::conjugation::ConjugationTable::load_embedded().expect("table");
        let out = run_parse(None, &table, "some text", None).expect("no index parses to nothing");
        assert!(out.segments.is_empty());
    }

    /// `run_parse`'s `.map_err(|e| e.to_string())` needs a real `ParseError`
    /// to exercise, not a fabricated string — this drives one by corrupting a
    /// freshly built index's payload file. Corruption happens *before*
    /// `Index::open`, never after: mutating a file underneath a live `Index`
    /// is documented UB (see `jparser::index::load::Index::open`'s doc
    /// comment), so this builds a working index, drops it, corrupts the file
    /// on disk, and only then opens the corrupted copy this test queries.
    #[test]
    fn a_parse_failure_is_reported_as_its_display_string() {
        let root = crate::test_support::scratch("parse-failure");
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
        // the actual `Err` path `run_parse` exists to map, not a fabrication.
        let err = run_parse(Some(&index), &table, "本", None)
            .expect_err("a corrupt records file must fail the parse");
        assert!(!err.is_empty(), "parse error message must not be empty");
    }
}
