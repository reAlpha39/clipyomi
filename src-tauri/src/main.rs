// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Tauri shell. Opens the index and settings at startup, spawns the parse
//! worker and the clipboard poll, and exposes the input and settings commands.

mod clipboard;
mod commands;
mod parse;
mod settings;
mod state;
#[cfg(test)]
mod test_support;

use std::sync::Arc;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let root = state::resolve_dict_root(&config_dir);

            // Settings first: the poll needs the monitoring flag before it starts.
            let settings_path = settings::settings_path(&config_dir);
            let (loaded, settings_warning) = settings::load(&settings_path);
            if let Some(warning) = settings_warning {
                eprintln!("{warning}");
            }
            if loaded.always_on_top {
                if let Some(window) = app.get_webview_window("main") {
                    // Not fatal: the window exists, it just is not pinned, and
                    // the toggle can retry.
                    let _ = window.set_always_on_top(true);
                }
            }
            // Managed as `Arc<SettingsState>`, not bare `SettingsState`: the
            // poll below needs its own `Arc` clone to hold across `.await`
            // points, and managing the bare type while giving the poll a
            // second, separately constructed `Arc` would mean two objects and
            // two sources of truth. One `Arc` is built once and shared by
            // both the managed state (for commands, via `State<'_,
            // Arc<SettingsState>>`, which derefs straight through to
            // `SettingsState`) and the poll.
            let settings = Arc::new(state::SettingsState::new(settings_path, loaded));
            app.manage(Arc::clone(&settings));

            let (tx, rx) = tokio::sync::watch::channel(String::new());
            app.manage(commands::InputSender(tx.clone()));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(clipboard::run_poll(handle, tx, Arc::clone(&settings)));

            // `StartupFailure` is managed on both branches, empty string meaning
            // success: `startup_error` needs `State<'_, StartupFailure>` to
            // always be there, because `Option<State<'_, T>>` does not
            // implement Tauri's `CommandArg` (verified by compiling it — the
            // error is "the trait bound `State<'_, StartupFailure>:
            // Deserialize<'_>` is not satisfied"), so a command parameter
            // cannot express "this state may not be managed".
            match state::load_state(&root) {
                // `Arc` because `run_worker` moves a handle into `spawn_blocking`,
                // and `tauri::State` itself is not `Send`.
                Ok(s) => {
                    let shared = Arc::new(s);
                    app.manage(Arc::clone(&shared));
                    app.manage(state::StartupFailure(String::new()));
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(parse::run_worker(handle, shared, rx));
                }
                Err(e) => {
                    // Startup failures are surfaced to the webview rather than
                    // aborting: an app that will not launch cannot tell the user
                    // to run `build-index`. `rx` is intentionally not moved on
                    // this branch: it is dropped here, so the poll's `tx.send`
                    // starts failing and the poll task exits instead of
                    // spinning forever with nothing downstream.
                    app.manage(state::StartupFailure(e.to_string()));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_input,
            commands::set_clipboard_monitoring,
            commands::set_always_on_top,
            commands::get_settings,
            commands::startup_error
        ])
        // If the runtime cannot start, there is no window to report anything
        // in, so the alternative to this `expect` is a silent exit.
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
