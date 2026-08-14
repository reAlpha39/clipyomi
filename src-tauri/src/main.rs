// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Tauri shell. Opens the index once at startup and exposes `parse_text`.

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
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let root = state::resolve_dict_root(&config_dir);
            // `StartupFailure` is managed on both branches, empty string meaning
            // success: `startup_error` needs `State<'_, StartupFailure>` to
            // always be there, because `Option<State<'_, T>>` does not
            // implement Tauri's `CommandArg` (verified by compiling it — the
            // error is "the trait bound `State<'_, StartupFailure>:
            // Deserialize<'_>` is not satisfied"), so a command parameter
            // cannot express "this state may not be managed".
            match state::load_state(&root) {
                // `Arc` because `parse_text` moves a handle into `spawn_blocking`,
                // and `tauri::State` itself is not `Send`.
                Ok(s) => {
                    app.manage(Arc::new(s));
                    app.manage(state::StartupFailure(String::new()));
                }
                Err(e) => {
                    // Startup failures are surfaced to the webview rather than
                    // aborting: an app that will not launch cannot tell the user
                    // to run `build-index`.
                    app.manage(state::StartupFailure(e.to_string()));
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::parse_text,
            commands::startup_error
        ])
        // If the runtime cannot start, there is no window to report anything
        // in, so the alternative to this `expect` is a silent exit.
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
