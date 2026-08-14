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
// Read only by `emit_status`, which nothing calls until Task 4 registers
// `download_dictionary` — see that fn's own `#[allow(dead_code)]` for why the
// whole chain below reads as unreachable until then.
#[allow(dead_code)]
pub const DICTIONARY_STATUS_EVENT: &str = "dictionary-status";

/// The sending half of the index channel, managed so `download_dictionary` can
/// publish a freshly built index to the already-running worker.
#[allow(dead_code)] // constructed in Task 4's main.rs setup, not this task
pub struct IndexSender(pub watch::Sender<Option<Arc<AppState>>>);

/// Where the dictionary lives. Resolved once in `main`'s `setup`, so the
/// command does not need the app config dir, and so `source/` stays a sibling
/// of `dict/` rather than living inside a published generation, which is
/// immutable.
#[allow(dead_code)] // constructed in Task 4's main.rs setup, not this task
pub struct DictionaryPaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
}

/// Guards against two concurrent `ensure_dictionary` runs. An `Arc` rather than
/// a bare `AtomicBool` because the command must clone it out of `State` before
/// its first `.await` — `tauri::State` is not `Send`.
#[allow(dead_code)] // constructed in Task 4's main.rs setup, not this task
pub struct DownloadInFlight(pub Arc<AtomicBool>);

/// Take the download slot, or report that it is already taken.
///
/// Separated from the command so the exclusion is testable without a Tauri app
/// handle or a real download.
// Exercised directly by the tests below; the `#[allow]` covers only the
// non-test binary, where `download_dictionary` — this fn's other caller — is
// itself unreachable until Task 4 registers it.
#[allow(dead_code)]
fn claim_download(flag: &AtomicBool) -> Result<(), String> {
    if flag.swap(true, Ordering::SeqCst) {
        Err("a dictionary download is already running".to_string())
    } else {
        Ok(())
    }
}

/// Release the slot. Called on every exit path, success or failure — a failed
/// download that never released would disable Retry for the rest of the session.
#[allow(dead_code)] // called from download_dictionary, itself unreachable until Task 4 registers it
fn release_download(flag: &AtomicBool) {
    flag.store(false, Ordering::SeqCst);
}

/// Whether startup found no dictionary, so the webview should show the download
/// screen rather than the panes.
#[tauri::command]
#[allow(dead_code)] // registered in Task 4's invoke_handler, not this task
pub fn needs_dictionary(needs: State<'_, NeedsDictionary>) -> bool {
    needs.0
}

/// Emit a phase label, logging rather than propagating a failed emit.
#[allow(dead_code)] // called from download_dictionary, itself unreachable until Task 4 registers it
fn emit_status(app: &AppHandle, status: &str) {
    if let Err(e) = app.emit(DICTIONARY_STATUS_EVENT, status.to_string()) {
        // Nothing to fall back on: if the event cannot reach the webview there
        // is no other channel to report progress through.
        eprintln!("emitting a dictionary-status event failed: {e}");
    }
}

/// Download JMdict if absent, build the index, and publish it to the worker.
///
/// Returns `Err` only when the work could not be *started*; every outcome after
/// that — including failure — arrives as a `dictionary-status` event. Same
/// split as `set_input`, and for the same reason: one path to the screen.
#[tauri::command]
#[allow(dead_code)] // registered in Task 4's invoke_handler, not this task
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
    let tx = index.0.clone();
    let handle = app.clone();
    let failure_hint = format!(
        " Retry, or place {} in {} and retry.",
        jmdict_source::SOURCE_FILE,
        source_dir.display()
    );

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
        .map_err(|e| e.to_string())?;

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
        // The failure hint names the archive and directory, which is the whole
        // manual fallback: `resolve` serves a hand-placed file without touching
        // the network, so a user behind a proxy can drop it in and press Retry.
        Ok(Err(message)) => format!("{message}.{failure_hint}"),
        Err(e) => format!("the download task failed to run: {e}.{failure_hint}"),
    };

    emit_status(&app, &message);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
