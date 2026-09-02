// SPDX-License-Identifier: MIT OR Apache-2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Tauri shell. Opens the index and settings at startup, spawns the parse
//! worker and the clipboard poll, and exposes the input and settings commands.

mod clipboard;
mod commands;
mod layout;
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

/// The stored startup geometry, clamped onto a monitor that exists now.
///
/// `layout::fit` is the same pure clamp `layout::restore` uses (design §6),
/// and startup is the other place a stored box meets a monitor layout that may
/// have changed since it was written — the only place where being wrong
/// survives a reboot, because `settings.json` still holds the off-screen origin
/// and every key that could correct it is attached to the window that went
/// off-screen with it.
///
/// `None` means "apply what was stored, the way this always did": either there
/// is no stored position, or the monitor list could not be read, and refusing
/// to place the window at all would be worse than placing it unclamped.
///
/// `height` arrives already assembled by the caller, reserved row included
/// where the platform has one. Nothing is added here: `initial_height` stays
/// the only place a band is ever added (`commands.rs:1104`).
fn startup_box(
    window: &tauri::Window,
    loaded: &settings::Settings,
    width: u32,
    height: u32,
) -> Option<layout::PhysRect> {
    let (x, y) = (loaded.window_x?, loaded.window_y?);
    let (rects, primary) = layout::monitors(window).ok()?;
    let scale = window.scale_factor().ok()?;
    let box_ = layout::LogicalBox {
        x,
        y,
        width,
        height,
    };
    Some(layout::fit(box_, &rects, primary, scale))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    // The handler fires for press AND release. Ungated, every
                    // hotkey would act twice per keystroke.
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        layout::dispatch(app, shortcut);
                    }
                })
                .build(),
        )
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
                #[cfg(target_os = "macos")]
                if loaded.chrome_mode != Some(settings::ChromeMode::Native) {
                    let _ =
                        commands::apply_decorations_macos(&main_window.as_ref().window(), false);
                }
                #[cfg(target_os = "windows")]
                {
                    commands::win32_frameless::init_window(&main_window.as_ref().window());
                }
                // On Windows the window is a band taller than it looks: the
                // reserved row lives above the content and the region clips it
                // away until a peek. That row is part of the saved box, so saved
                // geometry restores verbatim and only a first launch has the
                // band added to it — by `initial_height`, which answers for
                // every platform and is still the only place a band appears.
                let width = loaded
                    .window_width
                    .unwrap_or(commands::DEFAULT_WINDOW_WIDTH);
                let height = commands::initial_height(loaded.window_height);
                // One path for both platforms now, because the clamp has to run
                // wherever a stored box can name a monitor that is gone.
                match startup_box(&main_window.as_ref().window(), &loaded, width, height) {
                    Some(fitted) => {
                        let size = tauri::PhysicalSize::new(fitted.width, fitted.height);
                        let position = tauri::PhysicalPosition::new(fitted.x, fitted.y);
                        let _ = main_window.set_size(size);
                        let _ = main_window.set_position(position);
                    }
                    None => {
                        let _ = main_window.set_size(tauri::LogicalSize::new(width, height));
                        if let (Some(x), Some(y)) = (loaded.window_x, loaded.window_y) {
                            let _ = main_window.set_position(tauri::LogicalPosition::new(x, y));
                        }
                    }
                }
            }

            // Managed as `Arc<SettingsState>`, not bare `SettingsState`: the
            // poll below needs its own `Arc` clone to hold across `.await`
            // points, and managing the bare type while giving the poll a
            // second, separately constructed `Arc` would mean two objects and
            // two sources of truth. One `Arc` is built once and shared by
            // both the managed state (for commands, via `State<'_,
            // Arc<SettingsState>>`) and the background thread.
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

            // Hints travel on their own channel rather than inside `AppState`:
            // that state is built once and then read-only — which is what
            // `parse::catch_parse`'s unwind-safety argument rests on — and a
            // toggle must not rebuild the index to change one flag.
            let (hints_tx, hints_rx) =
                tokio::sync::watch::channel(None::<Arc<jparser::hints::VibratoTokenizer>>);
            app.manage(commands::HintsSender(hints_tx.clone()));

            let source_dir = config_dir.join(jmdict_source::SOURCE_DIR);
            let hints_dir = config_dir.join(vibrato_source::HINTS_DIR);
            app.manage(commands::DictionaryPaths {
                root: root.clone(),
                source_dir,
                hints_dir: hints_dir.clone(),
            });
            app.manage(commands::DownloadInFlight(Arc::new(AtomicBool::new(false))));
            app.manage(commands::HintsInFlight(Arc::new(AtomicBool::new(false))));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(parse::run_worker(handle, index_rx, hints_rx, rx));

            // Hints first, and outside `load_state`: that function returns
            // `Err(NoIndex)` on first run and never reaches its own tail, so
            // hints folded into it would go unloaded on exactly the run that
            // downloads JMdict, and stay unloaded until a restart.
            //
            // The four-branch decision itself lives in `state::startup` — see
            // its own doc comment for the branches and for why the index load
            // stays a lazy closure here rather than a second, eager `Result`.
            // This block is only the plumbing left after that extraction: the
            // stderr echo, the four `app.manage` calls, and the two `watch`
            // sends.
            let startup = state::startup(
                state::load_hints(&hints_dir, settings.snapshot().mecab_hints),
                || state::load_state(&root),
            );

            // Also kept for developers running from a terminal, the way the
            // settings warning above already is.
            if !startup.hints_warning.0.is_empty() {
                eprintln!("{}", startup.hints_warning.0);
            }
            app.manage(startup.hints_warning);
            // A send failure would mean the worker died between its spawn
            // above and this line, which cannot happen — it is awaiting its
            // first input.
            let _ = hints_tx.send(startup.hints.map(Arc::new));

            // `StartupFailure` and `NeedsDictionary` are both managed on every
            // branch: `Option<State<'_, T>>` is not a valid Tauri 2 command
            // parameter, so a command cannot express "this state may not be
            // managed". Empty string / `false` are the "nothing wrong" values.
            app.manage(startup.failure);
            app.manage(startup.needs_dictionary);
            if let Some(s) = startup.index {
                // A send failure would mean the worker died between its spawn
                // above and this line, which cannot happen — it is awaiting
                // its first input. Ignored with that reason rather than turned
                // into a startup abort.
                let _ = index_tx.send(Some(Arc::new(s)));
            }
            // Built here rather than declared in tauri.conf.json's `windows`
            // array: the config array has no way to express "create it hidden
            // and never show it until asked", and a tooltip that flashes at
            // startup is worse than no tooltip.
            popover::create(app)?;
            // Hidden, and for the same reason — see `create_settings_window`.
            commands::create_settings_window(app)?;
            // After the windows exist: a hotkey pressed the instant the app
            // starts must find a window to act on.
            let taken = layout::register_all(app.handle());
            app.manage(state::ShortcutWarning(layout::warning(&taken)));
            if !taken.is_empty() {
                eprintln!("hotkeys already in use: {}", taken.join(", "));
            }
            mouse_tracker::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::set_input,
            commands::set_clipboard_monitoring,
            commands::set_always_on_top,
            commands::set_chrome_mode,
            commands::peek_titlebar,
            commands::peek_grows_frame,
            commands::update_clip_region,
            commands::set_window_background,
            commands::platform,
            commands::is_macos,
            commands::minimize_window,
            commands::toggle_maximize_window,
            commands::close_window,
            commands::save_window_geometry,
            commands::save_settings,
            commands::get_settings,
            commands::open_settings_window,
            commands::startup_error,
            commands::settings_warning,
            commands::shortcut_warning,
            commands::frontend_ready,
            commands::download_dictionary,
            commands::needs_dictionary,
            commands::set_mecab_hints,
            commands::hints_warning,
            commands::hints_dir,
            popover::place_popover,
            popover::hide_popover
        ])
        // If the runtime cannot start, there is no window to report anything
        // in, so the alternative to this `expect` is a silent exit.
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
