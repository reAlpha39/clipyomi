// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Layout slots and their hotkeys (§7.9).
//!
//! Bind captures the live window box plus the persisted settings; restore puts
//! them back. Both live here rather than in `commands`, which is already far
//! past the project's file-size ceiling. The parts carrying the real logic are
//! pure functions that need no window at all, and they live one level down in
//! [`geometry`] — this module is the window plumbing that calls them.

mod geometry;

use std::sync::{Arc, OnceLock};

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::settings::{supported, ChromeMode, LayoutSlot, Settings};
use crate::state::SettingsState;

pub use geometry::{fit, fit_and_persist, unpeek, LogicalBox, MonitorRect, PhysRect};

/// Write a slot back over the live settings.
///
/// Assignment, not merge, for every key: a slot that recorded "unset" has to
/// clear it, which a merge cannot express. The mode is resolved on the way in,
/// so nothing downstream has to remember to.
///
/// `native_supported` is a parameter rather than a `cfg!` for the reason
/// `supported` gives: both arms are then testable wherever the suite runs.
pub fn apply_slot(settings: &mut Settings, slot: &LayoutSlot, native_supported: bool) {
    settings.window_width = Some(slot.window_width);
    settings.window_height = Some(slot.window_height);
    settings.window_x = Some(slot.window_x);
    settings.window_y = Some(slot.window_y);
    settings.chrome_mode = Some(supported(slot.chrome_mode, native_supported));
    settings.always_on_top = slot.always_on_top;
    settings.surface_alpha = slot.surface_alpha;
    settings.font_sentence = slot.font_sentence;
    settings.font_gloss = slot.font_gloss;
    settings.furigana_scale = slot.furigana_scale;
    settings.furigana_mode = slot.furigana_mode.clone();
}

/// The key a slot index is stored under, or `None` for an index outside
/// §7.9's `1..9`.
///
/// No registered hotkey reaches `bind` or `restore` today — the fixed slot keys
/// are gone and their callers are parked — so an out-of-range index is a
/// programming error rather than user input, and it stays that way once the
/// remappable phase feeds these from a user key table. It returns rather than
/// panics, because a hotkey has no way to report a panic.
pub fn slot_key(index: u8) -> Option<String> {
    (1..=9).contains(&index).then(|| index.to_string())
}

/// Build a slot from the persisted settings and a window box.
///
/// Everything but the geometry comes from `Settings`, which is what makes the
/// stored `surface_alpha` — rather than the value the frontend clamps for
/// `native` chrome — the one that gets captured. The clamped value is computed
/// by `surfaceAlphaVar` in the webview and never crosses back; only the
/// `opaque` boolean on `set_window_background` does.
pub fn slot_from(settings: &Settings, box_: LogicalBox) -> LayoutSlot {
    LayoutSlot {
        window_width: box_.width,
        window_height: box_.height,
        window_x: box_.x,
        window_y: box_.y,
        // Always `Some` after `normalize`. `Header` is the fallback because it
        // is the shape every platform can draw.
        chrome_mode: settings.chrome_mode.unwrap_or(ChromeMode::Header),
        always_on_top: settings.always_on_top,
        surface_alpha: settings.surface_alpha,
        font_sentence: settings.font_sentence,
        font_gloss: settings.font_gloss,
        furigana_scale: settings.furigana_scale,
        furigana_mode: settings.furigana_mode.clone(),
        extra: serde_json::Map::new(),
    }
}

/// Read the main window's box in the same units `save_window_geometry` stores:
/// inner size, outer position, logical, rounded (`main.ts:1104`). One
/// coordinate system for both, so a slot and `window_*` can be compared
/// without converting either.
fn live_box(window: &tauri::Window) -> Result<LogicalBox, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    let position = window
        .outer_position()
        .map_err(|e| e.to_string())?
        .to_logical::<f64>(scale);
    Ok(LogicalBox {
        x: position.x.round() as i32,
        y: position.y.round() as i32,
        width: size.width.round() as u32,
        height: size.height.round() as u32,
    })
}

/// Capture the current layout into slot `index` (`Shift+Alt+N`).
///
/// Silent on success. The Windows reserved row stays in the captured height
/// deliberately: it is part of the window's real box and is persisted that
/// way, and `initial_height` (`commands.rs:1099`) is the one place a band is
/// ever *added*.
pub fn bind(app: &tauri::AppHandle, index: u8) -> Result<(), String> {
    let Some(key) = slot_key(index) else {
        return Err(format!("slot {index} is outside 1..9"));
    };
    let window = crate::commands::chrome_target(app)?;
    let box_ = unpeek(
        live_box(&window)?,
        crate::commands::is_peeked(),
        crate::commands::peek_grows_frame(),
    );
    let settings = app.state::<Arc<SettingsState>>();
    let slot = slot_from(&settings.snapshot(), box_);
    settings
        .update(|s| {
            s.layout_slots.insert(key, slot);
        })
        .map_err(|e| e.to_string())
}

/// The slot `index` names, or `None` when it is unbound or out of range.
pub fn slot_to_restore(settings: &Settings, index: u8) -> Option<LayoutSlot> {
    let key = slot_key(index)?;
    settings.layout_slots.get(&key).cloned()
}

/// Every monitor's work area, plus the index of the primary one.
///
/// Matched by position and size rather than by name: `name()` is documented as
/// returning `None` for a monitor that no longer exists, which is precisely
/// the situation this whole path is built for.
///
/// `pub(crate)` for `main.rs`'s startup restore, which meets the same
/// possibly-changed monitor layout and needs the same two answers.
pub(crate) fn monitors(window: &tauri::Window) -> Result<(Vec<MonitorRect>, usize), String> {
    let available = window.available_monitors().map_err(|e| e.to_string())?;
    let primary = window.primary_monitor().map_err(|e| e.to_string())?;
    let rects = available
        .iter()
        .map(|m| {
            let area = m.work_area();
            MonitorRect {
                x: area.position.x,
                y: area.position.y,
                width: area.size.width,
                height: area.size.height,
                scale: m.scale_factor(),
            }
        })
        .collect();
    let primary_index = primary
        .and_then(|p| {
            available
                .iter()
                .position(|m| m.position() == p.position() && m.size() == p.size())
        })
        .unwrap_or(0);
    Ok((rects, primary_index))
}

/// Restore slot `index` (`Alt+N`).
///
/// Persist-then-apply, matching `set_chrome_mode`, and carrying the same
/// weakness 4A handoff §9 recorded: a failure in the apply half leaves a
/// persisted value the window was never updated to match. Not fixed here —
/// fixing it means reordering `set_chrome_mode` too — but this is now the
/// second caller of the pattern.
pub fn restore(app: &tauri::AppHandle, index: u8) -> Result<(), String> {
    let state = app.state::<Arc<SettingsState>>();
    let Some(slot) = slot_to_restore(&state.snapshot(), index) else {
        return Ok(());
    };
    let native_supported = crate::settings::native_supported();
    let mode = supported(slot.chrome_mode, native_supported);
    // The clamp runs before the persist so what reaches disk is the geometry
    // the window is about to get. `apply_slot` writes the raw slot box; the
    // closure below overwrites those four keys with the fitted one.
    let window = crate::commands::chrome_target(app)?;
    let (rects, primary) = monitors(&window)?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let (fitted, persisted) = fit_and_persist(
        LogicalBox {
            x: slot.window_x,
            y: slot.window_y,
            width: slot.window_width,
            height: slot.window_height,
        },
        &rects,
        primary,
        scale,
    );
    state
        .update(|s| {
            apply_slot(s, &slot, native_supported);
            s.window_width = Some(persisted.width);
            s.window_height = Some(persisted.height);
            s.window_x = Some(persisted.x);
            s.window_y = Some(persisted.y);
        })
        .map_err(|e| e.to_string())?;

    // Chrome before geometry: restoring a `native` slot from a frameless mode
    // adds a title bar on macOS, and adding one after `set_size` has fixed the
    // inner size leaves the content a title bar short.
    crate::commands::apply_chrome(&window, mode)?;
    window
        .set_size(tauri::PhysicalSize::new(fitted.width, fitted.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(fitted.x, fitted.y))
        .map_err(|e| e.to_string())?;
    window
        .set_always_on_top(slot.always_on_top)
        .map_err(|e| e.to_string())?;

    // One full snapshot carries the token-level half — alpha, the three sizes,
    // the furigana mode — to both webviews. It works because of attributes
    // already on `Settings`: every optional key skips serialization when
    // absent, which is exactly what `applyFontSizes` reads as "clear it", and
    // `always_on_top` + `clipboard_monitoring` are always present, which is
    // what gets the payload past `isFullSnapshot` (`main.ts:639`).
    app.emit("settings-changed", state.snapshot())
        .map_err(|e| e.to_string())
}

/// The event a history hotkey emits to the main window, with `-1` or `1`.
///
/// History is session-only and lives in the webview (4C handoff §4), so this
/// is the one action that has to leave Rust.
pub const HISTORY_STEP_EVENT: &str = "history-step";

/// What a hotkey does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    // Parked, not dead: `dispatch` still routes both, and `bind`/`restore` and
    // everything they call are still reachable and still tested. Only the two
    // *constructors* are unreachable, because `BINDINGS` no longer names a
    // fixed key for any slot. The remappable-bindings phase constructs these
    // again from the user's own key table and retires this suppression.
    #[allow(dead_code)]
    Restore(u8),
    #[allow(dead_code)]
    Bind(u8),
    /// The step direction, `-1` back or `1` forward.
    History(i8),
}

/// The global bindings, in registration order.
///
/// The eighteen `Alt+Digit`/`Shift+Alt+Digit` slot keys were removed: on a real
/// Windows desktop `Alt+1`, `Alt+2`, `Alt+3` and `Shift+Alt+1` were all already
/// owned by other software, so a third of the fixed table was dead on arrival.
/// A fixed table cannot answer that; the replacement is a later phase's
/// remappable bindings, which can be reassigned or switched off per key and
/// globally. Everything the slot keys called is parked below, not deleted.
pub const BINDINGS: [(&str, Action); 2] = [
    ("Alt+BracketLeft", Action::History(-1)),
    ("Alt+BracketRight", Action::History(1)),
];

/// The shortcuts that registered, paired with what they do.
///
/// Built once during `setup`; read by `dispatch` on every keypress. A `Vec`
/// scanned linearly rather than a map: two entries, once per keystroke.
static REGISTERED: OnceLock<Vec<(Shortcut, Action)>> = OnceLock::new();

/// Register every binding and return the names of the ones that would not take.
pub fn register_all(app: &tauri::AppHandle) -> Vec<String> {
    let mut registered = Vec::with_capacity(BINDINGS.len());
    let mut taken = Vec::new();
    for (name, action) in BINDINGS {
        // A name this module wrote itself failing to parse is a programming
        // error, but it must not abort startup — it is reported the same way a
        // taken key is, and the same test would catch it.
        let Ok(shortcut) = name.parse::<Shortcut>() else {
            taken.push(name.to_string());
            continue;
        };
        // `Shortcut` is `Copy`: the same value is passed to `register` and
        // pushed into the table, no `.clone()` needed for either use.
        match app.global_shortcut().register(shortcut) {
            Ok(()) => registered.push((shortcut, action)),
            Err(_) => taken.push(name.to_string()),
        }
    }
    // `setup` runs once, so a second call would be a bug; ignoring the result
    // keeps it from being a panic.
    let _ = REGISTERED.set(registered);
    taken
}

/// The user-facing text for the keys another application already owns.
///
/// Empty when there are none, which is how `ShortcutWarning` spells "nothing
/// to report" — the convention `SettingsWarning` and `StartupFailure` already
/// use.
pub fn warning(taken: &[String]) -> String {
    if taken.is_empty() {
        return String::new();
    }
    format!(
        "These hotkeys are already in use by another application and will not work: {}",
        taken.join(", ")
    )
}

/// Run the action a fired shortcut names.
///
/// Every failure is logged and dropped: a hotkey has no return channel, and a
/// failed restore must not take the app down with it.
pub fn dispatch(app: &tauri::AppHandle, fired: &Shortcut) {
    let Some(table) = REGISTERED.get() else {
        return;
    };
    let Some((_, action)) = table.iter().find(|(shortcut, _)| shortcut == fired) else {
        return;
    };
    let outcome = match *action {
        Action::Restore(index) => restore(app, index),
        Action::Bind(index) => bind(app, index),
        Action::History(delta) => app
            .emit_to("main", HISTORY_STEP_EVENT, delta)
            .map_err(|e| e.to_string()),
    };
    if let Err(e) = outcome {
        eprintln!("the {action:?} hotkey failed: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot_fixture() -> LayoutSlot {
        LayoutSlot {
            window_width: 720,
            window_height: 200,
            window_x: 1200,
            window_y: 40,
            chrome_mode: ChromeMode::Content,
            always_on_top: true,
            surface_alpha: Some(35),
            font_sentence: Some(30),
            font_gloss: Some(18),
            furigana_scale: Some(70),
            furigana_mode: Some("hiragana".to_string()),
            extra: serde_json::Map::new(),
        }
    }

    #[test]
    fn applying_a_slot_writes_every_captured_key() {
        let mut settings = Settings::default();
        apply_slot(&mut settings, &slot_fixture(), true);

        assert_eq!(settings.window_width, Some(720));
        assert_eq!(settings.window_height, Some(200));
        assert_eq!(settings.window_x, Some(1200));
        assert_eq!(settings.window_y, Some(40));
        assert_eq!(settings.chrome_mode, Some(ChromeMode::Content));
        assert!(settings.always_on_top);
        assert_eq!(settings.surface_alpha, Some(35));
        assert_eq!(settings.font_sentence, Some(30));
        assert_eq!(settings.font_gloss, Some(18));
        assert_eq!(settings.furigana_scale, Some(70));
        assert_eq!(settings.furigana_mode, Some("hiragana".to_string()));
    }

    /// Assignment, not merge: a slot bound while the font boxes were empty
    /// recorded them unset, and restoring it has to empty them again. A merge
    /// could never express that, which is why `save_settings` splits the same
    /// way for the same keys.
    #[test]
    fn applying_a_slot_clears_the_keys_it_recorded_as_unset() {
        let mut settings = Settings {
            surface_alpha: Some(80),
            font_sentence: Some(40),
            font_gloss: Some(20),
            furigana_scale: Some(50),
            furigana_mode: Some("romaji".to_string()),
            ..Default::default()
        };
        let bare = LayoutSlot {
            surface_alpha: None,
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            furigana_mode: None,
            ..slot_fixture()
        };

        apply_slot(&mut settings, &bare, true);

        assert_eq!(settings.surface_alpha, None);
        assert_eq!(settings.font_sentence, None);
        assert_eq!(settings.font_gloss, None);
        assert_eq!(settings.furigana_scale, None);
        assert_eq!(settings.furigana_mode, None);
    }

    /// A slot bound on macOS in `native` restores as `header` on Windows — the
    /// existing mechanic, reached from a new caller, with no `cfg!` of its own.
    #[test]
    fn a_native_slot_resolves_to_header_where_native_is_unsupported() {
        let native = LayoutSlot {
            chrome_mode: ChromeMode::Native,
            ..slot_fixture()
        };

        let mut unsupported = Settings::default();
        apply_slot(&mut unsupported, &native, false);
        assert_eq!(unsupported.chrome_mode, Some(ChromeMode::Header));

        let mut supported_here = Settings::default();
        apply_slot(&mut supported_here, &native, true);
        assert_eq!(supported_here.chrome_mode, Some(ChromeMode::Native));
    }

    fn box_fixture() -> LogicalBox {
        LogicalBox {
            x: 1200,
            y: 40,
            width: 720,
            height: 200,
        }
    }

    #[test]
    fn a_bound_slot_carries_the_window_box_and_every_setting() {
        let settings = Settings {
            always_on_top: true,
            chrome_mode: Some(ChromeMode::Content),
            surface_alpha: Some(35),
            font_sentence: Some(30),
            furigana_mode: Some("hiragana".to_string()),
            ..Default::default()
        };

        let slot = slot_from(&settings, box_fixture());

        assert_eq!(slot.window_width, 720);
        assert_eq!(slot.window_height, 200);
        assert_eq!(slot.window_x, 1200);
        assert_eq!(slot.window_y, 40);
        assert_eq!(slot.chrome_mode, ChromeMode::Content);
        assert!(slot.always_on_top);
        assert_eq!(slot.surface_alpha, Some(35));
        assert_eq!(slot.font_sentence, Some(30));
        assert_eq!(slot.furigana_mode, Some("hiragana".to_string()));
    }

    /// The phase's most important test. 4B handoff §3 warns that a snapshot
    /// reading the live `--surface-alpha` while chrome is `native` would
    /// capture 100 % and silently overwrite the alpha the user chose. Capture
    /// reads the persisted value, so the clamped one is not reachable from
    /// here at all — this test is what keeps it that way.
    #[test]
    fn a_slot_bound_in_native_chrome_records_the_stored_alpha() {
        let settings = Settings {
            chrome_mode: Some(ChromeMode::Native),
            surface_alpha: Some(40),
            ..Default::default()
        };

        assert_eq!(slot_from(&settings, box_fixture()).surface_alpha, Some(40));
    }

    /// A slot bound while nothing was persisted has to record that, not a
    /// number: restoring it must hand the stylesheet its defaults back.
    #[test]
    fn a_slot_bound_with_nothing_persisted_records_nothing() {
        let slot = slot_from(&Settings::default(), box_fixture());

        assert_eq!(slot.surface_alpha, None);
        assert_eq!(slot.font_sentence, None);
        assert_eq!(slot.font_gloss, None);
        assert_eq!(slot.furigana_scale, None);
        assert_eq!(slot.furigana_mode, None);
    }

    #[test]
    fn only_the_nine_slot_indices_have_a_key() {
        assert_eq!(slot_key(1).as_deref(), Some("1"));
        assert_eq!(slot_key(9).as_deref(), Some("9"));
        assert_eq!(slot_key(0), None);
        assert_eq!(slot_key(10), None);
    }

    /// Pressing `Alt+7` before ever binding it must do nothing and say
    /// nothing. There is no error state in a key you have not used yet.
    #[test]
    fn an_unbound_slot_resolves_to_nothing_to_do() {
        let settings = Settings::default();
        assert!(slot_to_restore(&settings, 7).is_none());
    }

    #[test]
    fn a_bound_slot_resolves_to_its_contents() {
        let mut settings = Settings::default();
        settings
            .layout_slots
            .insert("7".to_string(), slot_fixture());

        assert_eq!(slot_to_restore(&settings, 7), Some(slot_fixture()));
    }

    #[test]
    fn an_out_of_range_index_resolves_to_nothing_to_do() {
        let mut settings = Settings::default();
        settings
            .layout_slots
            .insert("0".to_string(), slot_fixture());

        assert!(slot_to_restore(&settings, 0).is_none());
    }

    /// Twenty bindings, each name distinct, each action distinct. A duplicated
    /// name would silently lose one action to the other at registration time.
    #[test]
    fn the_binding_table_is_two_distinct_keys() {
        assert_eq!(BINDINGS.len(), 2);

        let names: std::collections::BTreeSet<&str> = BINDINGS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 2, "a shortcut name is repeated");

        let actions: std::collections::BTreeSet<String> =
            BINDINGS.iter().map(|(_, a)| format!("{a:?}")).collect();
        assert_eq!(actions.len(), 2, "an action is repeated");
    }

    /// The parked slot machinery must stay unreachable until the remappable
    /// phase gives it a new entry point. A stray row here would hand a fixed
    /// key back to a feature whose fixed keys were just removed.
    #[test]
    fn no_slot_has_a_fixed_key() {
        for n in 1..=9u8 {
            assert!(
                !BINDINGS.iter().any(|(_, a)| *a == Action::Restore(n)),
                "slot {n} still has a fixed restore key"
            );
            assert!(
                !BINDINGS.iter().any(|(_, a)| *a == Action::Bind(n)),
                "slot {n} still has a fixed bind key"
            );
        }
    }

    /// `Alt+Left`/`Right` was rejected for these: a global grab takes Back and
    /// Forward from every browser and file manager on the machine for as long
    /// as this app runs.
    #[test]
    fn the_history_pair_is_the_brackets() {
        assert!(BINDINGS.contains(&("Alt+BracketLeft", Action::History(-1))));
        assert!(BINDINGS.contains(&("Alt+BracketRight", Action::History(1))));
    }

    /// Nothing taken means nothing to say. An empty warning is how
    /// `shortcut_warning` reports "no problem", exactly as `SettingsWarning`
    /// does.
    #[test]
    fn no_taken_keys_produces_no_warning() {
        assert_eq!(warning(&[]), "");
    }

    /// A key another application owns is this phase's only otherwise-invisible
    /// failure: the feature would just do nothing, with no way to tell that
    /// from a bug in the feature.
    #[test]
    fn taken_keys_are_named_in_the_warning() {
        let text = warning(&["Alt+Digit5".to_string(), "Alt+BracketLeft".to_string()]);
        assert!(text.contains("Alt+Digit5"), "got {text}");
        assert!(text.contains("Alt+BracketLeft"), "got {text}");
    }
}
