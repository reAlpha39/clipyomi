// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The webview's entry points: push input, toggle settings, read settings, ask
//! why startup failed or why settings fell back to defaults, ask whether the
//! dictionary needs downloading, run that download, and signal that the event
//! listeners are registered. Parsing itself runs in `parse::run_worker`;
//! results arrive as `parse-result` / `parse-error` events, not as a command
//! return value.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::{watch, Notify};

use crate::settings::Settings;
use crate::state::{AppState, NeedsDictionary, SettingsState, SettingsWarning, StartupFailure};

/// The sending half of the input channel, managed so commands can reach it.
pub struct InputSender(pub watch::Sender<String>);

/// Signals that the webview has registered its `parse-result` /
/// `parse-error` listeners. `frontend_ready` fires it; `clipboard::run_poll`
/// blocks its first tick on it — see `clipboard::wait_for_frontend` for why.
pub struct FrontendReady(pub Arc<Notify>);

/// Publish text for the worker to parse.
///
/// Separated from the command so it can be tested without a Tauri app handle.
fn push_input(tx: &watch::Sender<String>, text: String) -> Result<(), String> {
    tx.send(text)
        .map_err(|_| "the parse worker is no longer running".to_string())
}

/// Queue TEXT for parsing. The result arrives as a `parse-result` event.
///
/// Fire-and-forget rather than request/response: the clipboard produces parses
/// nobody asked for, so the webview renders from the event stream either way and
/// a returned value here would be a second, redundant path.
///
/// **This command deliberately has no caller.** Phase 2H removed the manual
/// text box, making the clipboard the only user-facing input path, and with it
/// the one `invoke` that reached this. It is kept as a test and debug entry
/// point: with clipboard-only input, exercising a parse by hand otherwise means
/// putting text on the system clipboard for every attempt, and this can be
/// driven from the DevTools console instead. Not dead code — do not remove it
/// without replacing that affordance.
#[tauri::command]
pub fn set_input(text: String, input: State<'_, InputSender>) -> Result<(), String> {
    push_input(&input.0, text)
}

#[tauri::command]
pub fn set_clipboard_monitoring(
    enabled: bool,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    settings
        .update(|s| s.clipboard_monitoring = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_always_on_top(
    enabled: bool,
    window: tauri::Window,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|e| e.to_string())?;
    settings
        .update(|s| s.always_on_top = enabled)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn apply_decorations_macos(window: &tauri::Window, enabled: bool) -> Result<(), String> {
    use std::ffi::c_void;
    let ptr_addr = window.ns_window().map_err(|e| e.to_string())? as usize;
    let win = window.clone();
    window
        .run_on_main_thread(move || {
            let ptr = ptr_addr as *mut c_void;
            extern "C" {
                fn objc_msgSend(receiver: *mut c_void, sel: *const c_void, ...) -> *mut c_void;
                fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
            }
            unsafe {
                let sel_set_style_mask = sel_registerName(c"setStyleMask:".as_ptr());
                let mask: usize = if enabled {
                    // NSWindowStyleMaskTitled (1) | NSWindowStyleMaskClosable (2) |
                    // NSWindowStyleMaskMiniaturizable (4) | NSWindowStyleMaskResizable (8) |
                    // NSWindowStyleMaskFullSizeContentView (32768)
                    1 | 2 | 4 | 8 | 32768
                } else {
                    // NSWindowStyleMaskBorderless (0) | NSWindowStyleMaskResizable (8)
                    8
                };
                let set_mask_fn: unsafe extern "C" fn(*mut c_void, *const c_void, usize) =
                    std::mem::transmute(objc_msgSend as *const ());
                set_mask_fn(ptr, sel_set_style_mask, mask);
            }
            let _ = win.set_theme(win.theme().ok());
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_decorations(
    enabled: bool,
    window: tauri::Window,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    apply_decorations_macos(&window, enabled)?;
    #[cfg(not(target_os = "macos"))]
    window
        .set_decorations(enabled)
        .map_err(|e| e.to_string())?;
    settings
        .update(|s| s.decorations = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_window_geometry(
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    settings
        .update(|s| {
            s.window_width = Some(width);
            s.window_height = Some(height);
            s.window_x = Some(x);
            s.window_y = Some(y);
        })
        .map_err(|e| e.to_string())
}


#[tauri::command]
pub fn get_settings(settings: State<'_, Arc<SettingsState>>) -> Settings {
    settings.snapshot()
}

/// Called once by the webview immediately after both `parse-result` and
/// `parse-error` listeners finish registering. Lifts the gate that keeps
/// `clipboard::run_poll` from reading the clipboard before anything is
/// listening for the result — see `clipboard::wait_for_frontend`.
#[tauri::command]
pub fn frontend_ready(ready: State<'_, FrontendReady>) {
    ready.0.notify_one();
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

/// A settings-load warning (e.g. a corrupt `settings.json`), or `null` when
/// settings loaded cleanly. Unlike `startup_error`, this is never fatal — see
/// `SettingsWarning`'s doc comment for why the two are separate.
#[tauri::command]
pub fn settings_warning(warning: State<'_, SettingsWarning>) -> Option<String> {
    if warning.0.is_empty() {
        None
    } else {
        Some(warning.0.clone())
    }
}

/// Emitted with a `String` payload as the download moves between phases:
/// `"downloading"`, `"building"`, `"ready"`, or an error message.
pub const DICTIONARY_STATUS_EVENT: &str = "dictionary-status";

/// The sending half of the index channel, managed so `download_dictionary` can
/// publish a freshly built index to the already-running worker.
pub struct IndexSender(pub watch::Sender<Option<Arc<AppState>>>);

/// Where the dictionary lives. Resolved once in `main`'s `setup`, so the
/// command does not need the app config dir, and so `source/` stays a sibling
/// of `dict/` rather than living inside a published generation, which is
/// immutable.
pub struct DictionaryPaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
}

/// Guards against two concurrent `ensure_dictionary` runs. An `Arc` rather than
/// a bare `AtomicBool` because the command must clone it out of `State` before
/// its first `.await` — `tauri::State` is not `Send`.
pub struct DownloadInFlight(pub Arc<AtomicBool>);

/// Take the download slot, or report that it is already taken.
///
/// Separated from the command so the exclusion is testable without a Tauri app
/// handle or a real download.
fn claim_download(flag: &AtomicBool) -> Result<(), String> {
    if flag.swap(true, Ordering::SeqCst) {
        Err("a dictionary download is already running".to_string())
    } else {
        Ok(())
    }
}

/// Release the slot. Called on every exit path, success or failure — a failed
/// download that never released would disable Retry for the rest of the session.
fn release_download(flag: &AtomicBool) {
    flag.store(false, Ordering::SeqCst);
}

/// Whether startup found no dictionary, so the webview should show the download
/// screen rather than the panes.
#[tauri::command]
pub fn needs_dictionary(needs: State<'_, NeedsDictionary>) -> bool {
    needs.0
}

/// Emit a phase label, logging rather than propagating a failed emit.
fn emit_status(app: &AppHandle, status: &str) {
    if let Err(e) = app.emit(DICTIONARY_STATUS_EVENT, status.to_string()) {
        // Nothing to fall back on: if the event cannot reach the webview there
        // is no other channel to report progress through.
        eprintln!("emitting a dictionary-status event failed: {e}");
    }
}

/// The failure text for `err`, preferring a more specific inner error when
/// one is recoverable.
///
/// `ensure_dictionary`'s source closure hands `resolve`'s failure to it as a
/// plain `std::io::Error` — `jmdict_source::resolve_from` boxes the original
/// `SourceError` via `std::io::Error::other`, since `jmdict-source` cannot
/// depend on `jparser`'s `IndexError` to return one directly — and
/// `ensure_dictionary` then wraps *that* a second time as `IndexError::Io`.
/// Shown as-is, that reads as "index io failed: could not obtain the
/// dictionary after 3 attempts (...)": the outer wrapper's prefix describes a
/// build-time I/O failure, which is not what happened. Downcasting recovers
/// the original `SourceError` and shows its own, more specific message
/// instead. `jmdict_source`'s own `the_typed_error_survives_the_io_wrapper`
/// test exists to prove this downcast is reliable; a genuine index-level I/O
/// failure (no `SourceError` inside) falls through to `IndexError`'s own
/// message unchanged.
fn describe_index_error(err: &jparser::index::IndexError) -> String {
    if let jparser::index::IndexError::Io(io_err) = err {
        if let Some(source_err) = io_err
            .get_ref()
            .and_then(|e| e.downcast_ref::<jmdict_source::SourceError>())
        {
            return source_err.to_string();
        }
    }
    err.to_string()
}

/// Append the manual-fallback instructions to `reason`, unless it already
/// names `source_dir_display`.
///
/// `SourceError::Http` and `SourceError::TooManyAttempts` already end with
/// "place a JMdict_e.gz in `<source_dir>` manually to bypass the download"
/// (`crates/jmdict-source/src/lib.rs`) — appending the same instructions
/// again would say it twice on screen. Every other failure (a corrupt
/// archive, a conjugation-table load failure, a dead worker) says nothing
/// about the directory at all, so this is the only place those would ever
/// learn about the manual escape hatch.
fn with_fallback(reason: String, source_dir_display: &str) -> String {
    if reason.contains(source_dir_display) {
        format!("{reason}. Retry.")
    } else {
        format!(
            "{reason}. Retry, or place {} in {source_dir_display} and retry.",
            jmdict_source::SOURCE_FILE,
        )
    }
}

/// Download JMdict if absent, build the index, and publish it to the worker.
///
/// Returns `Err` only when the work could not be *started*; every outcome after
/// that — including failure — arrives as a `dictionary-status` event. Same
/// split as `set_input`, and for the same reason: one path to the screen.
#[tauri::command]
pub async fn download_dictionary(
    app: AppHandle,
    paths: State<'_, DictionaryPaths>,
    index: State<'_, IndexSender>,
    inflight: State<'_, DownloadInFlight>,
) -> Result<(), String> {
    claim_download(&inflight.0)?;

    // Everything needed after the `.await` is cloned out first: `tauri::State`
    // is not `Send`, so holding one across an await point does not compile.
    let flag = Arc::clone(&inflight.0);
    let root = paths.root.clone();
    let source_dir = paths.source_dir.clone();
    // Read before `source_dir` moves into the closure below, so the fallback
    // message can still be built from it once the closure has finished.
    let source_dir_display = source_dir.display().to_string();
    let tx = index.0.clone();
    let handle = app.clone();

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        emit_status(&handle, "downloading");
        let table = jparser::conjugation::ConjugationTable::load_embedded()
            .map_err(|e| format!("loading the conjugation table failed: {e}"))?;
        let opts = jparser::stem::StemOptions::default();

        // `resolve` reuses an already-downloaded archive before it reaches the
        // network, so a retry after a *build* failure is fast and offline.
        // `ensure_dictionary` runs the closure only when a rebuild is needed.
        let index = jparser::index::ensure_dictionary(
            &root,
            &table,
            &opts,
            jparser::index::generations::DEFAULT_KEEP_GENERATIONS,
            || {
                let reader = jmdict_source::resolve(&source_dir)?;
                // Emitted only after `resolve` returns, not before: the network
                // fetch (or reuse of an already-downloaded archive) happens
                // inside `resolve` itself, so emitting "building" any earlier
                // would mislabel the download as index-building. Matches the
                // design spec's §4 data flow: "downloading" covers `resolve`,
                // "building" covers the `build_from_reader` work that follows.
                emit_status(&handle, "building");
                Ok(reader)
            },
        )
        .map_err(|e| describe_index_error(&e))?;

        // mirrors state::load_state's hints branch
        let hints = match std::env::var_os(crate::state::HINTS_ENV) {
            Some(path) => Some(
                jparser::hints::VibratoTokenizer::load(std::path::Path::new(&path))
                    .map_err(|e| format!("loading the hints dictionary failed: {e}"))?,
            ),
            None => None,
        };

        Ok::<AppState, String>(AppState {
            index,
            table,
            hints,
        })
    })
    .await;

    release_download(&flag);

    let message = match outcome {
        Ok(Ok(state)) => {
            if tx.send(Some(Arc::new(state))).is_err() {
                // The worker is gone, which no retry fixes. Reported like any
                // other failure rather than swallowed.
                "the parse worker is no longer running".to_string()
            } else {
                emit_status(&app, "ready");
                return Ok(());
            }
        }
        // `with_fallback` names the archive and directory only when `message`
        // does not already: `resolve` serves a hand-placed file without
        // touching the network, so a user behind a proxy can drop it in and
        // press Retry either way.
        Ok(Err(message)) => with_fallback(message, &source_dir_display),
        Err(e) => with_fallback(
            format!("the download task failed to run: {e}"),
            &source_dir_display,
        ),
    };

    emit_status(&app, &message);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// `set_input` is a thin push into the watch channel; the part worth testing
    /// is that the newest value is what a reader sees.
    #[tokio::test]
    async fn set_input_publishes_the_text_to_the_channel() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        push_input(&tx, "東京".to_string()).expect("push");
        assert_eq!(
            crate::parse::next_input(&mut rx).await.as_deref(),
            Some("東京")
        );
    }

    /// A dead worker must surface as an error rather than a silent no-op: the
    /// user pressed Parse and nothing would ever arrive.
    #[tokio::test]
    async fn set_input_reports_a_dead_worker() {
        let (tx, rx) = tokio::sync::watch::channel(String::new());
        drop(rx);
        assert!(push_input(&tx, "東京".to_string()).is_err());
    }

    /// Two overlapping `ensure_dictionary` runs would build two generations
    /// against one source directory. The frontend's own guard protects one
    /// button in one webview, not the command, so the command needs its own.
    #[test]
    fn a_second_download_is_rejected_while_one_is_running() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("the first claim succeeds");
        assert!(
            claim_download(&flag).is_err(),
            "a concurrent claim must be refused"
        );
    }

    /// A finished download — successful or not — must leave the door open, or
    /// one failure would disable Retry for the rest of the session.
    #[test]
    fn releasing_the_claim_allows_a_retry() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("first");
        release_download(&flag);
        claim_download(&flag).expect("retry after release");
    }

    /// Final review, Finding 1: `resolve_from` boxes a `SourceError` inside a
    /// plain `io::Error` (`std::io::Error::other`), which `ensure_dictionary`
    /// wraps again as `IndexError::Io`. The downcast must recover the
    /// original message rather than showing the generic "index io failed: "
    /// wrapper ahead of it.
    #[test]
    fn a_wrapped_source_error_replaces_the_generic_io_prefix() {
        let source_err = jmdict_source::SourceError::TooManyAttempts {
            attempts: 3,
            source_dir: PathBuf::from("/tmp/example/source"),
            last: "downloading the dictionary failed: connection refused".to_string(),
        };
        let expected = source_err.to_string();
        let wrapped = jparser::index::IndexError::Io(std::io::Error::other(source_err));

        let reason = describe_index_error(&wrapped);
        assert_eq!(reason, expected, "must show SourceError's own message");
        assert!(
            !reason.starts_with("index io failed"),
            "the generic wrapper prefix must not leak: {reason}"
        );
    }

    /// An `IndexError::Io` with no `SourceError` inside it is a genuine
    /// index-level I/O failure (e.g. a disk full while publishing a
    /// generation), which `IndexError`'s own "index io failed: " prefix
    /// describes correctly — the downcast must not touch it.
    #[test]
    fn a_genuine_index_io_error_keeps_its_own_message() {
        let wrapped = jparser::index::IndexError::Io(std::io::Error::other("disk full"));
        let reason = describe_index_error(&wrapped);
        assert_eq!(reason, wrapped.to_string());
        assert!(reason.starts_with("index io failed"), "got {reason}");
    }

    /// `SourceError::Http` and `SourceError::TooManyAttempts` already name the
    /// source directory as part of their own manual-fallback advice —
    /// appending a second copy would say it twice, which is the bug the whole
    /// review finding is about.
    #[test]
    fn the_fallback_is_not_repeated_when_the_reason_already_names_the_directory() {
        let dir = "/tmp/example/source";
        let reason = format!(
            "could not obtain the dictionary after 3 attempts (...); place a \
             JMdict_e.gz in {dir} manually to bypass the download"
        );

        let full = with_fallback(reason, dir);

        assert_eq!(
            full.matches(dir).count(),
            1,
            "the directory must appear exactly once: {full}"
        );
        assert!(full.contains("Retry"), "got {full}");
    }

    /// A corrupt-archive build failure (`IndexError::Jmdict`) never mentions
    /// the source directory on its own, so the fallback must still be added —
    /// this is the escape hatch's only appearance for that failure.
    #[test]
    fn the_fallback_is_added_when_the_reason_does_not_name_the_directory() {
        let dir = "/tmp/example/source";
        let full = with_fallback("reading JMdict failed: unexpected eof".to_string(), dir);

        assert!(full.contains(dir), "got {full}");
        assert!(full.contains("Retry"), "got {full}");
        assert!(full.contains(jmdict_source::SOURCE_FILE), "got {full}");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-commands-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn save_window_geometry_updates_settings_state() {
        let dir = scratch("save-geometry");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));
        let state_arg = app.state::<Arc<SettingsState>>();
        save_window_geometry(600, 200, 50, 80, state_arg).unwrap();
        let s = state.snapshot();
        assert_eq!(s.window_width, Some(600));
        assert_eq!(s.window_height, Some(200));
        assert_eq!(s.window_x, Some(50));
        assert_eq!(s.window_y, Some(80));
    }
}


