// SPDX-License-Identifier: MIT OR Apache-2.0

//! Startup state: the index and the conjugation table.
//!
//! Everything here is built once at launch and then read-only, which is what
//! lets `AppState` live in Tauri's managed state behind a shared reference —
//! and what `parse::catch_parse`'s unwind-safety argument rests on. Hints are
//! deliberately not here: they change while the app runs, and they travel to
//! the worker on their own `watch` channel instead.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use jparser::engine::table::Table;
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
    pub table: Table,
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

/// The global hotkeys another application already owns, as one sentence, or
/// empty when there are none.
///
/// Separate from `SettingsWarning` for the reason that state's own doc gives
/// for being separate from `StartupFailure`: one managed string per warning
/// kind, one command each, because the messages are independent and either can
/// be present without the other.
pub struct ShortcutWarning(pub String);

/// Why hints are off while the setting says they are on, or empty when they
/// agree.
///
/// Its own managed string on `ShortcutWarning`'s stated precedent: one per
/// warning kind, one command each, because the messages are independent and any
/// of them can be present without the others.
pub struct HintsWarning(pub String);

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "no dictionary index in {root}. Build one with:\n    \
         jparser-cli build-index <JMdict_e.xml> {root}"
    )]
    NoIndex { root: PathBuf },
    #[error("opening the dictionary index failed: {0}")]
    Index(#[from] IndexError),
    #[error("{HINTS_ENV} is set but the dictionary could not be loaded: {0}")]
    Hints(#[from] HintsError),
}

/// Whether this startup error is one the download screen can fix: no index at
/// all, or an index a previous engine or on-disk format built.
///
/// The second case is deliberate. `ensure_dictionary` rebuilds on exactly
/// `ConjugationMismatch` and `VersionMismatch` — both expected after an
/// application upgrade — so routing them to the download screen repairs a
/// stale index through the path that already exists, with no new UI. Every
/// other `IndexError` is a real failure a download cannot fix.
///
/// Matched exhaustively rather than with a wildcard: a future `StartupError`
/// variant must force a decision here. Offering a download for an error a
/// download cannot fix would loop the user through a wait that changes nothing.
pub fn is_missing_dictionary(error: &StartupError) -> bool {
    match error {
        StartupError::NoIndex { .. } => true,
        StartupError::Index(
            IndexError::ConjugationMismatch { .. } | IndexError::VersionMismatch { .. },
        ) => true,
        StartupError::Index(_) | StartupError::Hints(_) => false,
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
    let table = Table::new();

    Ok(AppState { index, table })
}

/// The warning shown when the setting asks for hints and there are none.
///
/// It exists because §5's rule creates a visible mismatch: the setting stays on
/// while hints are off, so the checkbox reads on and the parse has no hints.
/// The sentence names that state and the gesture that fixes it.
fn hints_off_warning(detail: &str) -> String {
    format!(
        "MeCab boundary hints are switched on but not loaded ({detail}). \
         Open Settings and toggle Boundary hints off and on to install them."
    )
}

/// The tokenizer this launch should parse with, and the warning to show if the
/// setting asks for one that is not there.
///
/// Deliberately **not** part of [`load_state`]. That function returns
/// `Err(NoIndex)` on first run and never reaches its own tail, so hints folded
/// into it would go unloaded on exactly the run that downloads JMdict, and stay
/// unloaded until a restart.
///
/// [`HINTS_ENV`] keeps 2C's fatal because it keeps 2C's meaning: an explicit
/// request, made outside the app, with no UI to recover in. That reasoning does
/// not describe a persisted setting, which has a control the user can see and a
/// warning surface to explain itself — so an absent dictionary there is hints
/// off, not a failed launch.
///
/// **Never acquires.** It calls `published`, never `resolve`: no network, and no
/// unpack of a hand-placed archive either. An unpack is a second of work with no
/// gesture behind it and its own ways to fail. The consequence is worth stating
/// — an archive dropped in while the app was closed is not installed by the next
/// launch. The toggle installs it, and the settings note says so.
pub fn load_hints(
    hints_dir: &Path,
    enabled: bool,
) -> Result<(Option<VibratoTokenizer>, String), StartupError> {
    if let Some(path) = std::env::var_os(HINTS_ENV) {
        return Ok((
            Some(VibratoTokenizer::load(Path::new(&path))?),
            String::new(),
        ));
    }

    if !enabled {
        return Ok((None, String::new()));
    }

    let Some(path) = vibrato_source::published(hints_dir) else {
        return Ok((
            None,
            hints_off_warning(&format!("no dictionary in {}", hints_dir.display())),
        ));
    };

    match VibratoTokenizer::load(&path) {
        Ok(tokenizer) => Ok((Some(tokenizer), String::new())),
        // A file that is there but will not load is the same user-visible state
        // as one that is not there, and the same gesture fixes both.
        Err(e) => Ok((None, hints_off_warning(&e.to_string()))),
    }
}

/// Everything `setup` must manage, decided from the startup loads.
///
/// One struct rather than four separate decisions inlined into `setup`, so the
/// decision itself is testable without a `tauri::App`: `setup`'s own job
/// shrinks to unpacking this into four `app.manage` calls and two `watch`
/// sends.
pub struct Startup {
    pub hints_warning: HintsWarning,
    pub hints: Option<VibratoTokenizer>,
    pub failure: StartupFailure,
    pub needs_dictionary: NeedsDictionary,
    pub index: Option<AppState>,
}

/// Decide everything `setup` needs to manage from this launch's hints and
/// index loads.
///
/// `index` is a closure, not a second `Result`, and that is the one thing to
/// get right here. Today `load_state` is called only from inside this
/// function's own success branch — a fatal hints load never opens the index at
/// all. A plain `Result` parameter would force every caller to open the index
/// *before* calling this function, which is eager: it would open an index
/// nobody uses on exactly the branch that must not touch one. The closure
/// keeps `load_state`'s laziness intact — it runs at most once, and only when
/// the hints load succeeded.
///
/// The four branches, behaviour-identical to what `setup` decided inline
/// before this extraction:
/// 1. `hints` is `Err(e)` — `TA_HINTS_DICT` set and unreadable. `hints_warning`
///    is empty, `hints` is `None`, `failure` is `e`'s message, `needs_dictionary`
///    is `false`, `index` is `None`, and **`index` is never called.**
/// 2. `hints` is `Ok`, `index()` is `Ok(s)` — the happy path. Everything as
///    loaded, `index` is `Some(s)`.
/// 3. `hints` is `Ok`, `index()` is `Err(e)` and [`is_missing_dictionary`] — the
///    expected first run. It is fixable from inside the window rather than a
///    failed launch, so `failure` stays empty and `needs_dictionary` is `true`
///    instead.
/// 4. `hints` is `Ok`, `index()` is `Err(e)` otherwise — a genuine failure.
///    `failure` is `e`'s message.
pub fn startup(
    hints: Result<(Option<VibratoTokenizer>, String), StartupError>,
    index: impl FnOnce() -> Result<AppState, StartupError>,
) -> Startup {
    let (tokenizer, warning) = match hints {
        Ok(pair) => pair,
        // `TA_HINTS_DICT` set and unreadable — the same fatal arm `load_state`'s
        // own hints error used to land in, so no behaviour moves. `index` is
        // deliberately not called on this branch; see this function's own doc
        // comment for why that laziness matters.
        Err(e) => {
            return Startup {
                hints_warning: HintsWarning(String::new()),
                hints: None,
                failure: StartupFailure(e.to_string()),
                needs_dictionary: NeedsDictionary(false),
                index: None,
            };
        }
    };

    let hints_warning = HintsWarning(warning);
    match index() {
        Ok(state) => Startup {
            hints_warning,
            hints: tokenizer,
            failure: StartupFailure(String::new()),
            needs_dictionary: NeedsDictionary(false),
            index: Some(state),
        },
        // The expected first run. Deliberately not a failure: that puts a
        // fatal message in `#output` for the rest of the session, and this
        // state is fixable from inside the window instead — the download
        // screen handles it.
        Err(e) if is_missing_dictionary(&e) => Startup {
            hints_warning,
            hints: tokenizer,
            failure: StartupFailure(String::new()),
            needs_dictionary: NeedsDictionary(true),
            index: None,
        },
        Err(e) => Startup {
            hints_warning,
            hints: tokenizer,
            failure: StartupFailure(e.to_string()),
            needs_dictionary: NeedsDictionary(false),
            index: None,
        },
    }
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
    /// The lock is held across `save`'s file I/O deliberately. The callers are
    /// no longer one webview's clicks taken one at a time: `layout::bind` and
    /// `layout::restore` run on the global-shortcut plugin's own message-loop
    /// thread, so two `update` calls can now genuinely overlap — pressing
    /// `Shift+Alt+1` inside the frontend's 300 ms geometry-save debounce is
    /// enough to do it. Releasing the lock before the write would let the later
    /// snapshot reach disk first and leave the file holding the older one: a
    /// bind that silently does not survive a restart. Blocking the other
    /// readers for the length of one small `write` is the cheaper cost.
    pub fn update<F>(&self, change: F) -> Result<(), SettingsError>
    where
        F: FnOnce(&mut Settings),
    {
        let mut settings = self.locked();
        change(&mut settings);
        save(&self.path, &settings)
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

    /// After an engine upgrade the newest generation carries the previous
    /// engine's fingerprint, or an older on-disk format. `ensure_dictionary`
    /// already rebuilds on exactly these two errors, so the download screen
    /// can repair them; treating them as fatal would strand every upgrading
    /// user on an error a click can fix.
    #[test]
    fn a_stale_index_is_the_download_screens_to_fix() {
        let stale_fingerprint = StartupError::Index(IndexError::ConjugationMismatch {
            found: 1,
            expected: 2,
        });
        let stale_format = StartupError::Index(IndexError::VersionMismatch {
            found: 3,
            expected: 4,
        });
        assert!(is_missing_dictionary(&stale_fingerprint));
        assert!(is_missing_dictionary(&stale_format));
    }

    /// Every other index failure stays fatal: a download cannot fix a failing
    /// disk or a corrupt payload, and offering one would loop the user through
    /// a wait that changes nothing.
    #[test]
    fn a_corrupt_or_unreadable_index_is_still_fatal() {
        let io = StartupError::Index(IndexError::Io(std::io::Error::other("boom")));
        assert!(!is_missing_dictionary(&io));
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

    /// Write a real, loadable dictionary into `hints_dir`'s published slot.
    fn publish_fixture_dictionary(hints_dir: &std::path::Path) {
        let dir = hints_dir.join(vibrato_source::DICT_DIR);
        std::fs::create_dir_all(&dir).expect("slot");
        jparser::hints::write_test_dictionary(&dir.join(vibrato_source::SYSTEM_DIC))
            .expect("fixture dictionary");
    }

    /// The setting off is the ordinary state, and it must cost nothing: no
    /// tokenizer, no warning, no look at the disk.
    #[test]
    fn hints_off_loads_nothing_and_warns_about_nothing() {
        let dir = scratch("hints-off");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (tokenizer, warning) = load_hints(&dir, false).expect("must not fail");

        assert!(tokenizer.is_none());
        assert!(warning.is_empty(), "got {warning:?}");
    }

    /// The happy path: the setting is on, a dictionary is published, and it loads.
    #[test]
    fn hints_on_loads_the_published_dictionary() {
        let dir = scratch("hints-published");
        publish_fixture_dictionary(&dir);
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (tokenizer, warning) = load_hints(&dir, true).expect("must not fail");

        assert!(tokenizer.is_some(), "a published dictionary must load");
        assert!(warning.is_empty(), "got {warning:?}");
    }

    /// Master spec §11's 5B rule: an absent dictionary means hints off, not a
    /// fatal. The setting stays on, so the mismatch has to be named somewhere the
    /// user can see it.
    #[test]
    fn hints_on_without_a_dictionary_is_a_warning_not_a_failure() {
        let dir = scratch("hints-absent");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

        assert!(tokenizer.is_none());
        assert!(!warning.is_empty(), "the mismatch must be named");
        assert!(
            warning.contains("Settings"),
            "the warning must name the gesture that fixes it, got {warning}"
        );
    }

    /// `published` only asks whether the file is there. A truncated or corrupt one
    /// answers yes and then fails to load, and that is the same user-visible state
    /// as an absent one.
    #[test]
    fn a_published_dictionary_that_will_not_load_is_a_warning_too() {
        let dir = scratch("hints-corrupt");
        let slot = dir.join(vibrato_source::DICT_DIR);
        std::fs::create_dir_all(&slot).expect("slot");
        std::fs::write(slot.join(vibrato_source::SYSTEM_DIC), b"not a dictionary").expect("write");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

        assert!(tokenizer.is_none());
        assert!(!warning.is_empty(), "got an empty warning");
    }

    /// Startup must never acquire: no network, and no unpack of a hand-placed
    /// archive either. An archive dropped in while the app was closed is installed
    /// by the toggle, not by the next launch.
    #[test]
    fn startup_never_installs_a_hand_placed_archive() {
        let dir = scratch("hints-archive-ignored");
        std::fs::create_dir_all(&dir).expect("hints dir");
        std::fs::write(dir.join("ipadic.tar.xz"), b"not unpacked at startup").expect("write");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

        assert!(tokenizer.is_none(), "startup must not unpack");
        assert!(!warning.is_empty());
        assert!(
            !dir.join(vibrato_source::DICT_DIR).exists(),
            "nothing may be published by a launch"
        );
    }

    /// `TA_HINTS_DICT` keeps 2C's fatal because it keeps 2C's meaning: an explicit
    /// request, made outside the app, with no UI to recover in. Unlike the setting,
    /// there is no control to show and no warning surface to explain itself.
    ///
    /// No index is built here any more: `load_hints` is independent of
    /// `load_state`, which is the whole point of it being a separate function.
    #[test]
    fn a_missing_hints_dictionary_is_reported_as_startup_error_hints() {
        let dir = scratch("hints-env-missing");
        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: `ENV_LOCK` above is the only writer of `TA_HINTS_DICT` in this
        // binary, held for the full set/read/clear cycle, so no other thread can
        // observe this variable mid-write.
        unsafe { std::env::set_var(HINTS_ENV, "/nonexistent/ta-hints-dict.dic") };
        let result = load_hints(&dir, false);
        unsafe { std::env::remove_var(HINTS_ENV) };

        match result {
            Err(StartupError::Hints(_)) => {}
            Ok(_) => panic!("expected StartupError::Hints, got Ok(_)"),
            Err(other) => panic!("expected StartupError::Hints, got a different error: {other}"),
        }
    }

    /// The env var outranks the setting in both directions: it loads even with the
    /// setting off, which is what makes it an override rather than a second toggle.
    #[test]
    fn the_env_override_loads_even_with_the_setting_off() {
        let dir = scratch("hints-env-present");
        let path = dir.join("system.dic");
        jparser::hints::write_test_dictionary(&path).expect("fixture dictionary");

        let _guard = ENV_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: as above.
        unsafe { std::env::set_var(HINTS_ENV, &path) };
        let result = load_hints(&dir, false);
        unsafe { std::env::remove_var(HINTS_ENV) };

        let (tokenizer, warning) = result.expect("a readable override must load");
        assert!(tokenizer.is_some());
        assert!(warning.is_empty(), "got {warning:?}");
    }

    // --- `startup`, Phase 5B check-up Piece 1 -------------------------------
    //
    // `AppState` derives no `Debug` (see its own doc comment: it is built once
    // and read-only, and nothing in the running app ever needs to print one),
    // so `Startup` cannot either. These assert on individual fields rather than
    // `matches!` with a `{result:?}` panic message, which is what the rest of
    // this file's `AppState`-adjacent tests already do (see `parse.rs`'s own
    // tests, which hit the same thing building one via `test_support`).

    /// Branch 1: a fatal hints load must never open the index — there is no
    /// `AppState` anyone will read on this branch, so opening one would be
    /// wasted work at best and, for a first-run directory, a spurious "no
    /// index" report layered on top of the real failure. A `Cell` the closure
    /// sets is the cheapest proof the closure ran at all.
    #[test]
    fn a_fatal_hints_load_never_calls_the_index_closure() {
        let index_called = std::cell::Cell::new(false);
        let err = StartupError::Hints(HintsError::Dictionary("boom".to_string()));

        let result = startup(Err(err), || {
            index_called.set(true);
            Err(StartupError::NoIndex {
                root: PathBuf::from("/never"),
            })
        });

        assert!(
            !index_called.get(),
            "a fatal hints load must never open the index"
        );
        assert!(result.hints_warning.0.is_empty());
        assert!(result.hints.is_none());
        assert!(
            !result.failure.0.is_empty(),
            "the hints error must be fatal"
        );
        assert!(!result.needs_dictionary.0);
        assert!(result.index.is_none());
    }

    /// Branch 2: the happy path. The warning string is a value distinct from
    /// every other branch's own test, so a field wired to the wrong source
    /// inside `startup` would show up as a mismatch here rather than passing
    /// by coincidence.
    #[test]
    fn a_successful_index_open_carries_the_loaded_hints_through() {
        let dir = scratch("startup-happy");
        let generation = crate::test_support::build_index_generation(&dir);
        let tokenizer_path = dir.join("system.dic");
        jparser::hints::write_test_dictionary(&tokenizer_path).expect("fixture dictionary");
        let tokenizer = VibratoTokenizer::load(&tokenizer_path).expect("load fixture");

        let result = startup(
            Ok((Some(tokenizer), "branch-2 warning".to_string())),
            || {
                Ok(AppState {
                    index: Index::open(&generation).expect("open"),
                    table: Table::new(),
                })
            },
        );

        assert_eq!(result.hints_warning.0, "branch-2 warning");
        assert!(
            result.hints.is_some(),
            "the loaded tokenizer must carry through"
        );
        assert!(result.failure.0.is_empty(), "got {:?}", result.failure.0);
        assert!(!result.needs_dictionary.0);
        assert!(result.index.is_some(), "a successful open must be kept");
    }

    /// Branch 3: `NoIndex` is the first-run condition, not a failure —
    /// `failure` must stay empty so the webview does not show a fatal, and
    /// `needs_dictionary` must flip so it shows the download screen instead.
    #[test]
    fn a_missing_index_offers_the_download_screen_instead_of_failing() {
        let result = startup(Ok((None, String::new())), || {
            Err(StartupError::NoIndex {
                root: PathBuf::from("/nowhere"),
            })
        });

        assert!(result.hints.is_none());
        assert!(result.failure.0.is_empty(), "got {:?}", result.failure.0);
        assert!(result.needs_dictionary.0);
        assert!(result.index.is_none());
    }

    /// Branch 4: every other index failure stays fatal, unlike branch 3's
    /// `NoIndex` — the mirror of `every_other_startup_error_stays_fatal`
    /// above, at the `startup` level rather than `is_missing_dictionary`'s own.
    #[test]
    fn any_other_index_failure_stays_fatal() {
        let result = startup(Ok((None, String::new())), || {
            Err(StartupError::Index(IndexError::Io(std::io::Error::other(
                "boom",
            ))))
        });

        assert!(
            !result.failure.0.is_empty(),
            "a genuine index failure must be fatal"
        );
        assert!(!result.needs_dictionary.0);
        assert!(result.index.is_none());
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

    /// The write happens under the same lock as the change, so the last thread
    /// to release the lock is also the last to reach the file. Without that,
    /// two overlapping `update`s can land on disk in the opposite order to
    /// memory and leave the file holding the older snapshot — a bound layout
    /// slot that silently does not survive a restart. Threads rather than one
    /// sequential loop because the hotkey handler really does call `update`
    /// from a thread of its own.
    #[test]
    fn concurrent_updates_leave_the_file_agreeing_with_memory() {
        let dir = scratch("concurrent");
        let path = dir.join("settings.json");
        let state = std::sync::Arc::new(SettingsState::new(path.clone(), Settings::default()));

        let writers: Vec<_> = (0..8u32)
            .map(|n| {
                let state = std::sync::Arc::clone(&state);
                std::thread::spawn(move || {
                    let _ = state.update(|s| s.window_width = Some(700 + n));
                })
            })
            .collect();
        for writer in writers {
            writer.join().expect("writer thread");
        }

        let (reloaded, warning) = crate::settings::load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert_eq!(
            reloaded.window_width,
            state.snapshot().window_width,
            "the file and memory disagree"
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
