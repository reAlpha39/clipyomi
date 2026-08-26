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

use crate::commands::BAND_HEIGHT_LOGICAL;
use crate::settings::{supported, LayoutSlot, Settings};

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
/// `i64` because two `i32` extents multiply straight past `i32`.
// Temporary: Task 6's `restore` function calls `fit`, making this reachable;
// remove when restore lands.
#[allow(dead_code)]
fn overlap(box_: &PhysRect, monitor: &MonitorRect) -> i64 {
    let left = box_.x.max(monitor.x);
    let top = box_.y.max(monitor.y);
    let right = (box_.x + box_.width as i32).min(monitor.x + monitor.width as i32);
    let bottom = (box_.y + box_.height as i32).min(monitor.y + monitor.height as i32);
    let width = (right - left).max(0) as i64;
    let height = (bottom - top).max(0) as i64;
    width * height
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
// Temporary: Task 6's `restore` function calls this; remove when restore lands.
#[allow(dead_code)]
pub fn fit(
    slot: LogicalBox,
    monitors: &[MonitorRect],
    primary: usize,
    scale_hint: f64,
) -> PhysRect {
    let probe = PhysRect {
        x: (slot.x as f64 * scale_hint).round() as i32,
        y: (slot.y as f64 * scale_hint).round() as i32,
        width: (slot.width as f64 * scale_hint).round() as u32,
        height: (slot.height as f64 * scale_hint).round() as u32,
    };
    if monitors.is_empty() {
        return probe;
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
    PhysRect { x, y, width, height }
}

/// Undo a peek's frame growth on a box just read from the live window.
///
/// `peek_titlebar` grows the frame upward by `BAND_HEIGHT_LOGICAL` and moves
/// the origin up by the same amount where `peek_grows_frame()` is true — macOS
/// (`commands.rs:999`). The frontend guards the identical hazard by skipping
/// the save entirely (`main.ts:1099`) and waiting for the resize that follows
/// the peek; a keystroke has to be answered now, so it corrects instead.
// Temporary: Task 5's `bind` calls `unpeek`; remove when bind lands.
#[allow(dead_code)]
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
// Temporary: Task 6's `restore` function calls `apply_slot`; remove when
// restore lands.
#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::ChromeMode;

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
}
