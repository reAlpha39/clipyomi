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
}
