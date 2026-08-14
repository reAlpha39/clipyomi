// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The webview's entry points: push input, toggle settings, read settings,
//! and ask why startup failed. Parsing itself runs in `parse::run_worker`;
//! results arrive as `parse-result` / `parse-error` events, not as a command
//! return value.

use std::sync::Arc;

use tauri::State;
use tokio::sync::watch;

use crate::settings::Settings;
use crate::state::{SettingsState, StartupFailure};

/// The sending half of the input channel, managed so commands can reach it.
pub struct InputSender(pub watch::Sender<String>);

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
}
