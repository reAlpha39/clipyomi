// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Startup state: the index, the conjugation table, and optional hints.
//!
//! Everything here is built once at launch and then read-only, which is what
//! lets `AppState` live in Tauri's managed state behind a shared reference.

use std::path::{Path, PathBuf};

use jparser::conjugation::{ConjugationError, ConjugationTable};
use jparser::hints::{HintsError, VibratoTokenizer};
use jparser::index::generations;
use jparser::index::load::Index;
use jparser::index::IndexError;

/// Environment variable naming an uncompressed compiled Vibrato dictionary.
///
/// An env var rather than a setting because settings persistence is 2E. When set
/// and unreadable this is fatal: a user who asked for hints and silently did not
/// get them receives a plausible result that is not what they asked for.
pub const HINTS_ENV: &str = "TA_HINTS_DICT";

/// Everything built once at launch and then only read.
///
/// Read by `commands::run_parse`, added in Task 3.
pub struct AppState {
    pub index: Index,
    pub table: ConjugationTable,
    pub hints: Option<VibratoTokenizer>,
}

/// A startup failure, kept as a rendered string so the webview can display it.
///
/// Managed unconditionally alongside `AppState` — empty string meaning
/// success — because `commands::startup_error` needs `State<'_,
/// StartupFailure>` to always be there: `Option<State<'_, T>>` is not a valid
/// Tauri 2 command parameter.
pub struct StartupFailure(pub String);

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "no dictionary index in {root}. Build one with:\n    \
         jparser-cli build-index <JMdict_e.xml> {root}"
    )]
    NoIndex { root: PathBuf },
    #[error("opening the dictionary index failed: {0}")]
    Index(#[from] IndexError),
    #[error("loading the conjugation table failed: {0}")]
    Conjugation(#[from] ConjugationError),
    #[error("{HINTS_ENV} is set but the dictionary could not be loaded: {0}")]
    Hints(#[from] HintsError),
}

/// The directory holding published index generations.
pub fn resolve_dict_root(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("dict")
}

/// Open the newest published generation and build the app's read-only state.
pub fn load_state(root: &Path) -> Result<AppState, StartupError> {
    let generation = generations::latest(root)?.ok_or_else(|| StartupError::NoIndex {
        root: root.to_path_buf(),
    })?;

    let index = Index::open(&generation)?;
    let table = ConjugationTable::load_embedded()?;

    let hints = match std::env::var_os(HINTS_ENV) {
        Some(path) => Some(VibratoTokenizer::load(Path::new(&path))?),
        None => None,
    };

    Ok(AppState {
        index,
        table,
        hints,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-state-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// The expected first-run condition, not an error: 2E adds the download.
    #[test]
    fn an_empty_root_reports_no_index_rather_than_failing() {
        let dir = scratch("no-index");
        let err = load_state(&dir.join("dict")).err().expect("must fail");
        assert!(matches!(err, StartupError::NoIndex { .. }), "got {err:?}");
    }

    /// The message has to name the command that fixes it: this is the state
    /// every new user hits first, and "no index" without a remedy reads as a bug.
    #[test]
    fn the_no_index_error_names_the_build_command() {
        let dir = scratch("no-index-msg");
        let err = load_state(&dir.join("dict")).err().expect("must fail");
        let msg = err.to_string();
        assert!(msg.contains("build-index"), "got {msg}");
    }

    /// A directory that looks like a generation but is not one must surface as a
    /// distinct failure — collapsing it into NoIndex would tell the user to build
    /// an index they already have.
    #[test]
    fn a_corrupt_generation_is_an_index_error_not_no_index() {
        let dir = scratch("corrupt");
        let root = dir.join("dict");
        std::fs::create_dir_all(root.join("gen-1")).expect("mkdir");
        std::fs::write(root.join("gen-1").join("header.bin"), b"not a header").expect("write");

        let err = load_state(&root).err().expect("must fail");
        assert!(matches!(err, StartupError::Index(_)), "got {err:?}");
    }

    /// Guards the sentinel `main.rs` relies on: an empty `StartupFailure`
    /// string means "startup succeeded" (see `StartupFailure`'s own doc
    /// comment). If a future `StartupError` variant ever rendered an empty
    /// string, a real startup failure would silently read as success.
    /// **Extend this list whenever a variant is added to `StartupError`.**
    #[test]
    fn every_startup_error_variant_renders_a_non_empty_string() {
        let variants: Vec<StartupError> = vec![
            StartupError::NoIndex {
                root: PathBuf::from("/nowhere"),
            },
            StartupError::Index(IndexError::Io(std::io::Error::other("boom"))),
            StartupError::Conjugation(ConjugationError::BadPartOfSpeech {
                name: "x".to_string(),
                pos: "y".to_string(),
            }),
            StartupError::Hints(HintsError::Dictionary("boom".to_string())),
        ];
        for variant in variants {
            assert!(
                !variant.to_string().is_empty(),
                "{variant:?} rendered an empty string"
            );
        }
    }

    // Serializes every test in this binary that mutates `TA_HINTS_DICT`.
    // `load_state` is the only code in this crate that reads it, and the
    // test below is currently the only one that writes it — mutating process
    // env is a whole-process hazard (why `set_var`/`remove_var` are `unsafe`
    // at this toolchain version), not just a hazard for this one variable, so
    // a future test that also touches `TA_HINTS_DICT` (or any other env var,
    // to be fully safe) should take this same lock rather than adding an
    // unguarded second one.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `TA_HINTS_DICT` pointing at nothing must surface as
    /// `StartupError::Hints`, not silently skip hints or panic. This needs a
    /// real, openable index: `load_state` only reaches the hints branch after
    /// the index and conjugation table both load, so the no-index shortcut
    /// `commands::run_parse`'s tests use cannot reach this code path.
    #[test]
    fn a_missing_hints_dictionary_is_reported_as_startup_error_hints() {
        let root = scratch("hints-missing");
        crate::test_support::build_index_generation(&root);

        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: `ENV_LOCK` above is the only writer of `TA_HINTS_DICT` in
        // this binary, held for the full set/read/clear cycle, so no other
        // thread can observe this variable mid-write.
        unsafe { std::env::set_var(HINTS_ENV, "/nonexistent/ta-hints-dict.dic") };
        let result = load_state(&root);
        unsafe { std::env::remove_var(HINTS_ENV) };

        // Matched by hand rather than `matches!(result, ..., "got {result:?}")`:
        // `AppState` (the `Ok` payload) has no `Debug` impl, so a
        // format-string assert on the whole `Result` will not compile.
        match result {
            Err(StartupError::Hints(_)) => {}
            Ok(_) => panic!("expected StartupError::Hints, got Ok(_)"),
            Err(other) => panic!("expected StartupError::Hints, got a different error: {other}"),
        }
    }
}
