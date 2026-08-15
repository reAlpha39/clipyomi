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
use std::sync::Mutex;

use jparser::conjugation::{ConjugationError, ConjugationTable};
use jparser::hints::{HintsError, VibratoTokenizer};
use jparser::index::generations;
use jparser::index::load::Index;
use jparser::index::IndexError;

use crate::settings::{save, Settings, SettingsError};

/// Environment variable naming an uncompressed compiled Vibrato dictionary.
///
/// An env var rather than a setting because settings persistence is 2E. When set
/// and unreadable this is fatal: a user who asked for hints and silently did not
/// get them receives a plausible result that is not what they asked for.
pub const HINTS_ENV: &str = "TA_HINTS_DICT";

/// Everything built once at launch and then only read.
///
/// Read by `parse::run_worker`, spawned from `main.rs`'s `setup`.
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

/// Whether startup found no index at all.
///
/// Managed unconditionally alongside `StartupFailure`, `false` meaning "there
/// is an index, or something worse is wrong", because a command parameter
/// cannot express "this state may not be managed" — see `StartupFailure`'s own
/// doc comment for the `Option<State<'_, T>>` reasoning.
pub struct NeedsDictionary(pub bool);

/// A non-fatal warning from loading settings (e.g. a corrupt `settings.json`),
/// kept separate from `StartupFailure` on purpose: `StartupFailure` being
/// non-empty is treated by the webview as fatal — `showStartupError` writes it
/// into `#output`, the pane a real parse result would otherwise occupy (see
/// `src/main.ts`). A corrupt settings file must not do that; it only means the
/// toggles reset to defaults, which is cosmetic, so `showSettingsWarning`
/// renders it into `#parse-error` instead and never touches `#output`.
/// Same empty-string-means-nothing sentinel as `StartupFailure`, for the same
/// reason: `commands::settings_warning` needs `State<'_, SettingsWarning>` to
/// always be there.
pub struct SettingsWarning(pub String);

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

/// Whether this startup error is the first-run condition the download screen
/// can fix, rather than a genuine failure.
///
/// Matched exhaustively rather than with a wildcard: a future `StartupError`
/// variant must force a decision here. Offering a download for an error a
/// download cannot fix would loop the user through a wait that changes nothing.
pub fn is_missing_dictionary(error: &StartupError) -> bool {
    match error {
        StartupError::NoIndex { .. } => true,
        StartupError::Index(_) | StartupError::Conjugation(_) | StartupError::Hints(_) => false,
    }
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

/// Settings plus the path they persist to, shared between the commands that
/// change them and the poll that reads them.
pub struct SettingsState {
    path: PathBuf,
    settings: Mutex<Settings>,
}

impl SettingsState {
    pub fn new(path: PathBuf, settings: Settings) -> Self {
        Self {
            path,
            settings: Mutex::new(settings),
        }
    }

    /// A lock poisoned by a panic in another thread still holds usable settings:
    /// every write replaces the whole value, so there is no torn state to
    /// protect against. Recovering beats refusing to read a toggle.
    fn locked(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> Settings {
        self.locked().clone()
    }

    pub fn monitoring_enabled(&self) -> bool {
        self.locked().clipboard_monitoring
    }

    /// Apply a change and persist it.
    ///
    /// The in-memory value is updated even when the write fails, so a read-only
    /// config dir degrades to "settings do not survive restart" rather than
    /// "the toggles do not work".
    ///
    /// The lock is released before `save`'s file I/O runs, deliberately: holding
    /// it across a write would block every other read and update for the
    /// duration of a disk operation. The cost is a narrow, real race — two
    /// concurrent `update` calls can interleave so the file on disk ends up
    /// holding the older of the two snapshots while memory holds the newer.
    /// Not closed here: both callers (`set_always_on_top`,
    /// `set_clipboard_monitoring`) are driven by clicks on one webview, one at
    /// a time, so the window is narrow — and holding the lock across I/O to
    /// close it would be worse than the race itself.
    pub fn update<F>(&self, change: F) -> Result<(), SettingsError>
    where
        F: FnOnce(&mut Settings),
    {
        let snapshot = {
            let mut settings = self.locked();
            change(&mut settings);
            settings.clone()
        };
        save(&self.path, &snapshot)
    }
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

    #[test]
    fn settings_state_reports_the_monitoring_flag() {
        let dir = scratch("monitoring");
        let state = SettingsState::new(dir.join("settings.json"), Settings::default());
        assert!(state.monitoring_enabled(), "default is monitoring on");

        state
            .update(|s| s.clipboard_monitoring = false)
            .expect("update");
        assert!(!state.monitoring_enabled());
    }

    /// The toggle has to outlive the process, or it is not a setting.
    #[test]
    fn updating_settings_writes_them_to_disk() {
        let dir = scratch("persist");
        let path = dir.join("settings.json");
        let state = SettingsState::new(path.clone(), Settings::default());

        state.update(|s| s.always_on_top = true).expect("update");

        let (reloaded, warning) = crate::settings::load(&path);
        assert!(reloaded.always_on_top, "not persisted");
        assert!(warning.is_none(), "got {warning:?}");
    }

    /// `main.rs` builds `SettingsWarning` from `settings::load`'s second
    /// return value as `SettingsWarning(warning.unwrap_or_default())` — this
    /// proves that mapping keeps the sentinel convention: non-empty for a
    /// corrupt file, empty for a clean load.
    #[test]
    fn a_corrupt_settings_file_yields_a_non_empty_settings_warning() {
        let dir = scratch("warning-corrupt");
        let path = dir.join("settings.json");
        std::fs::write(&path, b"{ not json").expect("write");

        let (_, warning) = crate::settings::load(&path);
        let warning = SettingsWarning(warning.unwrap_or_default());
        assert!(!warning.0.is_empty());
    }

    /// A missing file (first run) is not corruption and must not surface as
    /// a warning: same distinction `settings::load` itself already makes.
    #[test]
    fn a_clean_settings_load_yields_an_empty_settings_warning() {
        let dir = scratch("warning-clean");
        let path = dir.join("settings.json");

        let (_, warning) = crate::settings::load(&path);
        let warning = SettingsWarning(warning.unwrap_or_default());
        assert!(warning.0.is_empty(), "got {:?}", warning.0);
    }

    /// A read-only config dir must not make the toggles stop working in the
    /// running session — the in-memory value still changes.
    #[test]
    fn a_failed_write_still_updates_the_in_memory_value() {
        // A path whose parent is a file, not a directory: create_dir_all fails.
        let dir = scratch("readonly");
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"not a directory").expect("write");
        let state = SettingsState::new(blocker.join("settings.json"), Settings::default());

        let result = state.update(|s| s.always_on_top = true);
        assert!(result.is_err(), "the write should have failed");
        assert!(
            state.snapshot().always_on_top,
            "in-memory value must still change"
        );
    }

    /// The first-run condition is fixable from inside the window, so it must
    /// not take the fatal path that puts a fatal message in `#output` for the
    /// rest of the session.
    #[test]
    fn a_missing_index_is_the_first_run_condition_not_a_failure() {
        let err = StartupError::NoIndex {
            root: PathBuf::from("/nowhere"),
        };
        assert!(is_missing_dictionary(&err));
    }

    /// Everything else stays fatal. Listed exhaustively rather than with a
    /// wildcard so a new `StartupError` variant forces a decision here instead
    /// of silently defaulting to "offer a download that cannot help".
    #[test]
    fn every_other_startup_error_stays_fatal() {
        let fatal: Vec<StartupError> = vec![
            StartupError::Index(IndexError::Io(std::io::Error::other("boom"))),
            StartupError::Conjugation(ConjugationError::BadPartOfSpeech {
                name: "x".to_string(),
                pos: "y".to_string(),
            }),
            StartupError::Hints(HintsError::Dictionary("boom".to_string())),
        ];
        for err in fatal {
            assert!(
                !is_missing_dictionary(&err),
                "{err:?} must not offer a download"
            );
        }
    }
}
