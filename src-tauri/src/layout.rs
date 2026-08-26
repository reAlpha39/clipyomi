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
//! past the project's file-size ceiling, and the parts carrying the real logic
//! are pure functions that need no window at all.

use std::sync::{Arc, OnceLock};

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::commands::BAND_HEIGHT_LOGICAL;
use crate::settings::{supported, ChromeMode, LayoutSlot, Settings};
use crate::state::SettingsState;

/// A window box in logical (scale-independent) pixels — the units
/// `save_window_geometry` and `LayoutSlot` both store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalBox {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// One monitor's *work area* in physical pixels, plus its scale factor.
///
/// A plain struct rather than `tauri::Monitor` so every case in `fit` is
/// testable with fabricated numbers and no window — the same reason
/// `supported` takes the platform answer as a parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

/// A window box in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// `tauri.conf.json`'s `minWidth` and `minHeight`.
pub const MIN_LOGICAL_WIDTH: u32 = 160;
pub const MIN_LOGICAL_HEIGHT: u32 = 80;

/// The area of the intersection of `box_` and `monitor`, in physical pixels.
///
/// `i64` for the whole calculation, not just the product: two `i32` extents
/// multiply straight past `i32`, and `x + width` alone overflows for a
/// hand-edited `window_x` near `i32::MAX` — which in a debug build is a panic,
/// on a keystroke, which is the one thing this module must not do.
fn overlap(box_: &PhysRect, monitor: &MonitorRect) -> i64 {
    let (bx, by) = (box_.x as i64, box_.y as i64);
    let (mx, my) = (monitor.x as i64, monitor.y as i64);
    let left = bx.max(mx);
    let top = by.max(my);
    let right = (bx + box_.width as i64).min(mx + monitor.width as i64);
    let bottom = (by + box_.height as i64).min(my + monitor.height as i64);
    (right - left).max(0) * (bottom - top).max(0)
}

/// Pin a stored slot box onto a monitor that exists now.
///
/// Physical pixels are the only unambiguous space here: under mixed DPI each
/// monitor has its own scale, so there is no consistent logical plane across
/// the set. `placeFor` records the same reasoning for testing points against
/// `position`/`size` rather than calling `monitorFromPoint` (`main.ts:905`).
///
/// The ceiling: step 1 converts with `scale_hint`, the *current* window's
/// scale, so on a mixed-DPI setup the monitor picked can be the neighbour of
/// the one the slot was bound on. The consequence is a window fully on-screen
/// on the wrong monitor — not a window off-screen, which is what this exists
/// to prevent. A second pass re-probing with the target's scale would narrow
/// it and is not worth the code until someone reports it.
pub fn fit(
    slot: LogicalBox,
    monitors: &[MonitorRect],
    primary: usize,
    scale_hint: f64,
) -> PhysRect {
    fit_and_persist(slot, monitors, primary, scale_hint).0
}

/// A physical box back in the logical pixels `window_*` and `LayoutSlot` store.
fn to_logical(rect: PhysRect, scale: f64) -> LogicalBox {
    LogicalBox {
        x: (rect.x as f64 / scale).round() as i32,
        y: (rect.y as f64 / scale).round() as i32,
        width: (rect.width as f64 / scale).round() as u32,
        height: (rect.height as f64 / scale).round() as u32,
    }
}

/// `fit`'s box — the clamp itself is documented there — plus the same box in
/// logical pixels, which is what a caller persists.
///
/// The conversion back uses the *target* monitor's scale rather than the
/// current window's, because the window ends up on the target: the logical box
/// this returns is the one `save_window_geometry` would record for the window
/// once it is there (`main.ts:1104`), so `window_*` on disk stays in the units
/// everything else writes them in.
///
/// Persisting the raw slot instead is how a restore leaves the file naming a
/// monitor that is gone: the window lands on-screen, `settings.json` keeps the
/// off-screen origin, and a second `Alt+N` cannot correct it — `set_position`
/// with the coordinates the window already occupies fires no move event, so the
/// frontend's debounced geometry save never runs. Quit in that state and the
/// next launch opens off-screen, where no hotkey can reach it.
pub fn fit_and_persist(
    slot: LogicalBox,
    monitors: &[MonitorRect],
    primary: usize,
    scale_hint: f64,
) -> (PhysRect, LogicalBox) {
    let probe = PhysRect {
        x: (slot.x as f64 * scale_hint).round() as i32,
        y: (slot.y as f64 * scale_hint).round() as i32,
        width: (slot.width as f64 * scale_hint).round() as u32,
        height: (slot.height as f64 * scale_hint).round() as u32,
    };
    if monitors.is_empty() {
        return (probe, to_logical(probe, scale_hint));
    }

    let overlapping = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| (i, overlap(&probe, m)))
        .filter(|(_, area)| *area > 0)
        .max_by_key(|(_, area)| *area)
        .map(|(i, _)| i);
    // `min` rather than a bare index: `primary` comes from matching two
    // monitor lists against each other, and a caller that got it wrong must
    // not panic a hotkey.
    let target = &monitors[overlapping.unwrap_or(primary.min(monitors.len() - 1))];

    let width = ((slot.width.max(MIN_LOGICAL_WIDTH) as f64) * target.scale).round() as u32;
    let height = ((slot.height.max(MIN_LOGICAL_HEIGHT) as f64) * target.scale).round() as u32;
    let width = width.min(target.width);
    let height = height.min(target.height);
    // Both maxima are >= their minima because the extents above are already
    // capped at the work area's, so neither `clamp` can panic.
    let x = probe
        .x
        .clamp(target.x, target.x + target.width as i32 - width as i32);
    let y = probe
        .y
        .clamp(target.y, target.y + target.height as i32 - height as i32);
    let fitted = PhysRect { x, y, width, height };
    (fitted, to_logical(fitted, target.scale))
}

/// Undo a peek's frame growth on a box just read from the live window.
///
/// `peek_titlebar` grows the frame upward by `BAND_HEIGHT_LOGICAL` and moves
/// the origin up by the same amount where `peek_grows_frame()` is true — macOS
/// (`commands.rs:999`). The frontend guards the identical hazard by skipping
/// the save entirely (`main.ts:1099`) and waiting for the resize that follows
/// the peek; a keystroke has to be answered now, so it corrects instead.
pub fn unpeek(box_: LogicalBox, peeked: bool, grows_frame: bool) -> LogicalBox {
    if !(peeked && grows_frame) {
        return box_;
    }
    LogicalBox {
        y: box_.y + BAND_HEIGHT_LOGICAL as i32,
        height: box_.height.saturating_sub(BAND_HEIGHT_LOGICAL),
        ..box_
    }
}

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
/// Only the twenty registered hotkeys reach `bind` and `restore` today, so an
/// out-of-range index is a programming error rather than user input — but it
/// returns rather than panics, because a hotkey has no way to report a panic.
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
    Restore(u8),
    Bind(u8),
    /// The step direction, `-1` back or `1` forward.
    History(i8),
}

/// The twenty global bindings, in registration order.
///
/// Written out rather than generated: a `&'static str` cannot be formatted at
/// runtime, and a table this size is worth being able to grep.
pub const BINDINGS: [(&str, Action); 20] = [
    ("Alt+Digit1", Action::Restore(1)),
    ("Alt+Digit2", Action::Restore(2)),
    ("Alt+Digit3", Action::Restore(3)),
    ("Alt+Digit4", Action::Restore(4)),
    ("Alt+Digit5", Action::Restore(5)),
    ("Alt+Digit6", Action::Restore(6)),
    ("Alt+Digit7", Action::Restore(7)),
    ("Alt+Digit8", Action::Restore(8)),
    ("Alt+Digit9", Action::Restore(9)),
    ("Shift+Alt+Digit1", Action::Bind(1)),
    ("Shift+Alt+Digit2", Action::Bind(2)),
    ("Shift+Alt+Digit3", Action::Bind(3)),
    ("Shift+Alt+Digit4", Action::Bind(4)),
    ("Shift+Alt+Digit5", Action::Bind(5)),
    ("Shift+Alt+Digit6", Action::Bind(6)),
    ("Shift+Alt+Digit7", Action::Bind(7)),
    ("Shift+Alt+Digit8", Action::Bind(8)),
    ("Shift+Alt+Digit9", Action::Bind(9)),
    ("Alt+BracketLeft", Action::History(-1)),
    ("Alt+BracketRight", Action::History(1)),
];

/// The shortcuts that registered, paired with what they do.
///
/// Built once during `setup`; read by `dispatch` on every keypress. A `Vec`
/// scanned linearly rather than a map: twenty entries, once per keystroke.
static REGISTERED: OnceLock<Vec<(Shortcut, Action)>> = OnceLock::new();

/// Register all twenty and return the names of the ones that would not take.
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
        "These layout hotkeys are already in use by another application and will not work: {}",
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

    /// A single 1920×1080 monitor at the origin, unscaled — the common case.
    fn one_monitor() -> Vec<MonitorRect> {
        vec![MonitorRect { x: 0, y: 0, width: 1920, height: 1080, scale: 1.0 }]
    }

    #[test]
    fn a_box_already_inside_its_monitor_comes_back_unchanged() {
        let slot = LogicalBox { x: 100, y: 100, width: 720, height: 480 };
        assert_eq!(
            fit(slot, &one_monitor(), 0, 1.0),
            PhysRect { x: 100, y: 100, width: 720, height: 480 }
        );
    }

    /// The whole reason this function exists: a slot bound on a monitor that
    /// has since been unplugged must not park the window somewhere unreachable
    /// — every key that could fix it is attached to that window.
    #[test]
    fn a_box_on_a_monitor_that_is_gone_lands_on_the_one_that_remains() {
        let slot = LogicalBox { x: 3000, y: 200, width: 720, height: 480 };
        let fitted = fit(slot, &one_monitor(), 0, 1.0);
        assert_eq!(fitted.x, 1920 - 720);
        assert_eq!(fitted.y, 200);
    }

    #[test]
    fn a_box_bigger_than_the_work_area_shrinks_to_it() {
        let monitors = vec![MonitorRect { x: 0, y: 0, width: 800, height: 600, scale: 1.0 }];
        let slot = LogicalBox { x: 0, y: 0, width: 1600, height: 1200 };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect { x: 0, y: 0, width: 800, height: 600 }
        );
    }

    /// `tauri.conf.json`'s `minWidth`/`minHeight`. Shrinking below what the
    /// window manager would refuse anyway buys nothing.
    #[test]
    fn the_configured_minimum_is_the_floor() {
        let slot = LogicalBox { x: 0, y: 0, width: 10, height: 10 };
        let fitted = fit(slot, &one_monitor(), 0, 1.0);
        assert_eq!(fitted.width, MIN_LOGICAL_WIDTH);
        assert_eq!(fitted.height, MIN_LOGICAL_HEIGHT);
    }

    /// A monitor to the left of primary has a negative origin, and a box
    /// legitimately inside it has negative coordinates too. Clamping to zero
    /// would drag the window onto the wrong screen.
    #[test]
    fn a_monitor_left_of_primary_keeps_its_negative_coordinates() {
        let monitors = vec![
            MonitorRect { x: 0, y: 0, width: 1920, height: 1080, scale: 1.0 },
            MonitorRect { x: -1920, y: 0, width: 1920, height: 1080, scale: 1.0 },
        ];
        let slot = LogicalBox { x: -1800, y: 50, width: 720, height: 480 };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect { x: -1800, y: 50, width: 720, height: 480 }
        );
    }

    /// The size is re-derived with the TARGET monitor's scale, not the probe's:
    /// a slot bound at 100 % has to look the same size at 150 %, which means
    /// more physical pixels, not the same number.
    #[test]
    fn the_target_monitors_scale_sets_the_physical_size() {
        let monitors = vec![MonitorRect { x: 0, y: 0, width: 2880, height: 1620, scale: 1.5 }];
        let slot = LogicalBox { x: 0, y: 0, width: 720, height: 480 };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect { x: 0, y: 0, width: 1080, height: 720 }
        );
    }

    /// Nothing to clamp against. Refusing to move the window would make the
    /// slot silently useless, so the probe stands.
    #[test]
    fn an_empty_monitor_list_returns_the_probe_unchanged() {
        let slot = LogicalBox { x: 40, y: 40, width: 720, height: 480 };
        assert_eq!(
            fit(slot, &[], 0, 2.0),
            PhysRect { x: 80, y: 80, width: 1440, height: 960 }
        );
    }

    /// No monitor overlaps the probe, so the fallback runs — and it has to be
    /// the primary monitor, not simply the first one in the list.
    #[test]
    fn a_box_that_overlaps_nothing_lands_on_the_primary_monitor() {
        let monitors = vec![
            MonitorRect { x: 0, y: 0, width: 1920, height: 1080, scale: 1.0 },
            MonitorRect { x: -1920, y: 0, width: 1280, height: 1024, scale: 1.0 },
        ];
        let slot = LogicalBox { x: 6000, y: 6000, width: 720, height: 480 };

        let fitted = fit(slot, &monitors, 1, 1.0);

        assert_eq!(fitted.x, -1920 + 1280 - 720);
        assert_eq!(fitted.y, 1024 - 480);
    }

    /// `primary` comes from matching two monitor lists against each other, so a
    /// caller can hand over an index the list no longer has. The guard exists
    /// for exactly that, and a hotkey is the wrong place to panic.
    #[test]
    fn a_primary_index_past_the_end_of_the_list_clamps_instead_of_panicking() {
        let slot = LogicalBox { x: 9000, y: 0, width: 720, height: 480 };

        let fitted = fit(slot, &one_monitor(), 5, 1.0);

        assert_eq!(fitted.x, 1920 - 720);
    }

    /// A hand-edited `window_x` near `i32::MAX` used to overflow the
    /// intersection arithmetic, which in a debug build is a panic — on a
    /// keystroke, with no channel to report it.
    #[test]
    fn a_box_at_the_far_edge_of_the_coordinate_space_does_not_panic() {
        let slot = LogicalBox { x: i32::MAX - 10, y: 0, width: 720, height: 480 };

        let fitted = fit(slot, &one_monitor(), 0, 1.0);

        assert_eq!(fitted.x, 1920 - 720);
    }

    /// The invariant behind `fit_and_persist`: what restore writes to disk is a
    /// box the window actually occupied. Persisting the raw slot leaves
    /// `settings.json` naming the monitor that went away, and the next launch
    /// opens there — off-screen, with every key that could fix it attached to
    /// the window that is off-screen.
    #[test]
    fn the_box_to_persist_is_the_fitted_one_not_the_slot() {
        let slot = LogicalBox { x: 3000, y: 200, width: 720, height: 480 };

        let (fitted, persisted) = fit_and_persist(slot, &one_monitor(), 0, 1.0);

        assert_eq!(fitted, PhysRect { x: 1200, y: 200, width: 720, height: 480 });
        assert_eq!(persisted, LogicalBox { x: 1200, y: 200, width: 720, height: 480 });
        assert_ne!(persisted, slot, "the raw slot must not be what gets stored");
    }

    /// The conversion back is at the TARGET monitor's scale, not the window's:
    /// the window now sits on the target, so this is the logical box
    /// `save_window_geometry` would record for it (`main.ts:1104`).
    #[test]
    fn the_box_to_persist_is_logical_at_the_target_monitors_scale() {
        let monitors = vec![MonitorRect { x: 0, y: 0, width: 2880, height: 1620, scale: 1.5 }];
        let slot = LogicalBox { x: 0, y: 0, width: 720, height: 480 };

        let (fitted, persisted) = fit_and_persist(slot, &monitors, 0, 1.0);

        assert_eq!(fitted, PhysRect { x: 0, y: 0, width: 1080, height: 720 });
        assert_eq!(persisted, slot, "a fitted box that moved nothing round-trips");
    }

    /// macOS grows the frame upward for a peek, so a bind pressed mid-peek
    /// would store a box a band too tall sitting a band too high — and every
    /// restore of that slot would repeat the error.
    #[test]
    fn a_peek_that_grew_the_frame_is_undone() {
        let peeked = LogicalBox { x: 100, y: 72, width: 720, height: 508 };
        assert_eq!(
            unpeek(peeked, true, true),
            LogicalBox { x: 100, y: 100, width: 720, height: 480 }
        );
    }

    /// Windows reserves the row permanently and only changes the clip region,
    /// so there is no growth to undo and subtracting one would corrupt the box.
    #[test]
    fn a_peek_that_did_not_grow_the_frame_is_left_alone() {
        let box_ = LogicalBox { x: 100, y: 100, width: 720, height: 480 };
        assert_eq!(unpeek(box_, true, false), box_);
    }

    #[test]
    fn a_box_read_outside_a_peek_is_left_alone() {
        let box_ = LogicalBox { x: 100, y: 100, width: 720, height: 480 };
        assert_eq!(unpeek(box_, false, true), box_);
    }

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
        let native = LayoutSlot { chrome_mode: ChromeMode::Native, ..slot_fixture() };

        let mut unsupported = Settings::default();
        apply_slot(&mut unsupported, &native, false);
        assert_eq!(unsupported.chrome_mode, Some(ChromeMode::Header));

        let mut supported_here = Settings::default();
        apply_slot(&mut supported_here, &native, true);
        assert_eq!(supported_here.chrome_mode, Some(ChromeMode::Native));
    }

    fn box_fixture() -> LogicalBox {
        LogicalBox { x: 1200, y: 40, width: 720, height: 200 }
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
        settings.layout_slots.insert("7".to_string(), slot_fixture());

        assert_eq!(slot_to_restore(&settings, 7), Some(slot_fixture()));
    }

    #[test]
    fn an_out_of_range_index_resolves_to_nothing_to_do() {
        let mut settings = Settings::default();
        settings.layout_slots.insert("0".to_string(), slot_fixture());

        assert!(slot_to_restore(&settings, 0).is_none());
    }

    /// Twenty bindings, each name distinct, each action distinct. A duplicated
    /// name would silently lose one action to the other at registration time.
    #[test]
    fn the_binding_table_is_twenty_distinct_keys() {
        assert_eq!(BINDINGS.len(), 20);

        let names: std::collections::BTreeSet<&str> = BINDINGS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names.len(), 20, "a shortcut name is repeated");

        let actions: std::collections::BTreeSet<String> =
            BINDINGS.iter().map(|(_, a)| format!("{a:?}")).collect();
        assert_eq!(actions.len(), 20, "an action is repeated");
    }

    #[test]
    fn every_slot_has_a_restore_and_a_bind() {
        for n in 1..=9u8 {
            assert!(
                BINDINGS.iter().any(|(_, a)| *a == Action::Restore(n)),
                "no restore for slot {n}"
            );
            assert!(
                BINDINGS.iter().any(|(_, a)| *a == Action::Bind(n)),
                "no bind for slot {n}"
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
