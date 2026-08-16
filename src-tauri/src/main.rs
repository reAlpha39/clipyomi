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
mod mouse_tracker;
mod parse;
mod popover;
mod settings;
mod state;
#[cfg(test)]
mod test_support;

use std::sync::atomic::AtomicBool;
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
            // Also kept for developers running from a terminal: `settings_warning`
            // (the command) is how the webview learns about this, but stderr is
            // free and this app already logs other soft failures the same way.
            if let Some(warning) = &settings_warning {
                eprintln!("{warning}");
            }
            app.manage(state::SettingsWarning(settings_warning.unwrap_or_default()));
            if let Some(main_window) = app.get_webview_window("main") {
                if loaded.always_on_top {
                    // Not fatal: the window exists, it just is not pinned, and
                    // the toggle can retry.
                    let _ = main_window.set_always_on_top(true);
                }
                if !loaded.decorations {
                    #[cfg(target_os = "macos")]
                    let _ = commands::apply_decorations_macos(&main_window.as_ref().window(), false);
                    #[cfg(not(target_os = "macos"))]
                    let _ = main_window.set_decorations(false);
                }
                if let (Some(w), Some(h)) = (loaded.window_width, loaded.window_height) {
                    let _ = main_window.set_size(tauri::LogicalSize::new(w, h));
                }
                if let (Some(x), Some(y)) = (loaded.window_x, loaded.window_y) {
                    let _ = main_window.set_position(tauri::LogicalPosition::new(x, y));
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

            // Fires once, from `commands::frontend_ready`, right after the
            // webview finishes registering its event listeners. `run_poll`
            // blocks its first tick on this — see
            // `clipboard::wait_for_frontend` for the race it closes.
            let ready = Arc::new(tokio::sync::Notify::new());
            app.manage(commands::FrontendReady(Arc::clone(&ready)));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(clipboard::run_poll(
                handle,
                tx,
                Arc::clone(&settings),
                ready,
            ));

            // The worker is spawned unconditionally, before startup knows
            // whether an index exists. 2E spawned it only on the success branch
            // and dropped `rx` otherwise, which left no live channel for a
            // first-run download's result to arrive on. Starting it always
            // deletes that special case rather than adding one.
            let (index_tx, index_rx) = tokio::sync::watch::channel(None::<Arc<state::AppState>>);
            app.manage(commands::IndexSender(index_tx.clone()));

            let source_dir = config_dir.join(jmdict_source::SOURCE_DIR);
            app.manage(commands::DictionaryPaths {
                root: root.clone(),
                source_dir,
            });
            app.manage(commands::DownloadInFlight(Arc::new(AtomicBool::new(false))));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(parse::run_worker(handle, index_rx, rx));

            // `StartupFailure` and `NeedsDictionary` are both managed on every
            // branch: `Option<State<'_, T>>` is not a valid Tauri 2 command
            // parameter, so a command cannot express "this state may not be
            // managed". Empty string / `false` are the "nothing wrong" values.
            match state::load_state(&root) {
                Ok(s) => {
                    app.manage(state::StartupFailure(String::new()));
                    app.manage(state::NeedsDictionary(false));
                    // A send failure would mean the worker died between its
                    // spawn above and this line, which cannot happen — it is
                    // awaiting its first input. Ignored with that reason rather
                    // than turned into a startup abort.
                    let _ = index_tx.send(Some(Arc::new(s)));
                }
                // The expected first run. Deliberately not a `StartupFailure`:
                // that one puts a fatal message in `#output` for the rest of
                // the session, and this state is fixable from inside the
                // window instead — the download screen handles it.
                Err(e) if state::is_missing_dictionary(&e) => {
                    app.manage(state::StartupFailure(String::new()));
                    app.manage(state::NeedsDictionary(true));
                }
                Err(e) => {
                    app.manage(state::StartupFailure(e.to_string()));
                    app.manage(state::NeedsDictionary(false));
                }
            }
            // Built here rather than declared in tauri.conf.json's `windows`
            // array: the config array has no way to express "create it hidden
            // and never show it until asked", and a tooltip that flashes at
            // startup is worse than no tooltip.
            popover::create(app)?;
            mouse_tracker::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_input,
            commands::set_clipboard_monitoring,
            commands::set_always_on_top,
            commands::set_decorations,
            commands::save_window_geometry,
            commands::save_settings,
            commands::get_settings,
            commands::open_settings_window,
            commands::startup_error,
            commands::settings_warning,
            commands::frontend_ready,
            commands::download_dictionary,
            commands::needs_dictionary,
            popover::place_popover,
            popover::hide_popover
        ])

        // If the runtime cannot start, there is no window to report anything
        // in, so the alternative to this `expect` is a silent exit.
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
