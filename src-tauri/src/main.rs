// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Tauri shell. Opens the index once at startup and exposes `parse_text`.

mod state;

use std::sync::Arc;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let root = state::resolve_dict_root(&config_dir);
            match state::load_state(&root) {
                // `Arc` because `parse_text` moves a handle into `spawn_blocking`,
                // and `tauri::State` itself is not `Send`.
                Ok(s) => {
                    app.manage(Arc::new(s));
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
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
