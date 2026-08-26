// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Pure window geometry: the rectangles a slot is stored as, and the clamp
//! that pins one back onto a monitor that exists now.
//!
//! Split out of `layout` when that module passed the 800-line ceiling. The
//! seam is the one the parent already claimed: nothing here touches a window,
//! so every case is testable with fabricated numbers.

use crate::commands::BAND_HEIGHT_LOGICAL;

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
    let fitted = PhysRect {
        x,
        y,
        width,
        height,
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A single 1920×1080 monitor at the origin, unscaled — the common case.
    fn one_monitor() -> Vec<MonitorRect> {
        vec![MonitorRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
        }]
    }

    #[test]
    fn a_box_already_inside_its_monitor_comes_back_unchanged() {
        let slot = LogicalBox {
            x: 100,
            y: 100,
            width: 720,
            height: 480,
        };
        assert_eq!(
            fit(slot, &one_monitor(), 0, 1.0),
            PhysRect {
                x: 100,
                y: 100,
                width: 720,
                height: 480
            }
        );
    }

    /// The whole reason this function exists: a slot bound on a monitor that
    /// has since been unplugged must not park the window somewhere unreachable
    /// — every key that could fix it is attached to that window.
    #[test]
    fn a_box_on_a_monitor_that_is_gone_lands_on_the_one_that_remains() {
        let slot = LogicalBox {
            x: 3000,
            y: 200,
            width: 720,
            height: 480,
        };
        let fitted = fit(slot, &one_monitor(), 0, 1.0);
        assert_eq!(fitted.x, 1920 - 720);
        assert_eq!(fitted.y, 200);
    }

    #[test]
    fn a_box_bigger_than_the_work_area_shrinks_to_it() {
        let monitors = vec![MonitorRect {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            scale: 1.0,
        }];
        let slot = LogicalBox {
            x: 0,
            y: 0,
            width: 1600,
            height: 1200,
        };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect {
                x: 0,
                y: 0,
                width: 800,
                height: 600
            }
        );
    }

    /// `tauri.conf.json`'s `minWidth`/`minHeight`. Shrinking below what the
    /// window manager would refuse anyway buys nothing.
    #[test]
    fn the_configured_minimum_is_the_floor() {
        let slot = LogicalBox {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        };
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
            MonitorRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            MonitorRect {
                x: -1920,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
        ];
        let slot = LogicalBox {
            x: -1800,
            y: 50,
            width: 720,
            height: 480,
        };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect {
                x: -1800,
                y: 50,
                width: 720,
                height: 480
            }
        );
    }

    /// The size is re-derived with the TARGET monitor's scale, not the probe's:
    /// a slot bound at 100 % has to look the same size at 150 %, which means
    /// more physical pixels, not the same number.
    #[test]
    fn the_target_monitors_scale_sets_the_physical_size() {
        let monitors = vec![MonitorRect {
            x: 0,
            y: 0,
            width: 2880,
            height: 1620,
            scale: 1.5,
        }];
        let slot = LogicalBox {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };
        assert_eq!(
            fit(slot, &monitors, 0, 1.0),
            PhysRect {
                x: 0,
                y: 0,
                width: 1080,
                height: 720
            }
        );
    }

    /// Nothing to clamp against. Refusing to move the window would make the
    /// slot silently useless, so the probe stands.
    #[test]
    fn an_empty_monitor_list_returns_the_probe_unchanged() {
        let slot = LogicalBox {
            x: 40,
            y: 40,
            width: 720,
            height: 480,
        };
        assert_eq!(
            fit(slot, &[], 0, 2.0),
            PhysRect {
                x: 80,
                y: 80,
                width: 1440,
                height: 960
            }
        );
    }

    /// No monitor overlaps the probe, so the fallback runs — and it has to be
    /// the primary monitor, not simply the first one in the list.
    #[test]
    fn a_box_that_overlaps_nothing_lands_on_the_primary_monitor() {
        let monitors = vec![
            MonitorRect {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            MonitorRect {
                x: -1920,
                y: 0,
                width: 1280,
                height: 1024,
                scale: 1.0,
            },
        ];
        let slot = LogicalBox {
            x: 6000,
            y: 6000,
            width: 720,
            height: 480,
        };

        let fitted = fit(slot, &monitors, 1, 1.0);

        assert_eq!(fitted.x, -1920 + 1280 - 720);
        assert_eq!(fitted.y, 1024 - 480);
    }

    /// `primary` comes from matching two monitor lists against each other, so a
    /// caller can hand over an index the list no longer has. The guard exists
    /// for exactly that, and a hotkey is the wrong place to panic.
    #[test]
    fn a_primary_index_past_the_end_of_the_list_clamps_instead_of_panicking() {
        let slot = LogicalBox {
            x: 9000,
            y: 0,
            width: 720,
            height: 480,
        };

        let fitted = fit(slot, &one_monitor(), 5, 1.0);

        assert_eq!(fitted.x, 1920 - 720);
    }

    /// A hand-edited `window_x` near `i32::MAX` used to overflow the
    /// intersection arithmetic, which in a debug build is a panic — on a
    /// keystroke, with no channel to report it.
    #[test]
    fn a_box_at_the_far_edge_of_the_coordinate_space_does_not_panic() {
        let slot = LogicalBox {
            x: i32::MAX - 10,
            y: 0,
            width: 720,
            height: 480,
        };

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
        let slot = LogicalBox {
            x: 3000,
            y: 200,
            width: 720,
            height: 480,
        };

        let (fitted, persisted) = fit_and_persist(slot, &one_monitor(), 0, 1.0);

        assert_eq!(
            fitted,
            PhysRect {
                x: 1200,
                y: 200,
                width: 720,
                height: 480
            }
        );
        assert_eq!(
            persisted,
            LogicalBox {
                x: 1200,
                y: 200,
                width: 720,
                height: 480
            }
        );
        assert_ne!(persisted, slot, "the raw slot must not be what gets stored");
    }

    /// The conversion back is at the TARGET monitor's scale, not the window's:
    /// the window now sits on the target, so this is the logical box
    /// `save_window_geometry` would record for it (`main.ts:1104`).
    #[test]
    fn the_box_to_persist_is_logical_at_the_target_monitors_scale() {
        let monitors = vec![MonitorRect {
            x: 0,
            y: 0,
            width: 2880,
            height: 1620,
            scale: 1.5,
        }];
        let slot = LogicalBox {
            x: 0,
            y: 0,
            width: 720,
            height: 480,
        };

        let (fitted, persisted) = fit_and_persist(slot, &monitors, 0, 1.0);

        assert_eq!(
            fitted,
            PhysRect {
                x: 0,
                y: 0,
                width: 1080,
                height: 720
            }
        );
        assert_eq!(
            persisted, slot,
            "a fitted box that moved nothing round-trips"
        );
    }

    /// macOS grows the frame upward for a peek, so a bind pressed mid-peek
    /// would store a box a band too tall sitting a band too high — and every
    /// restore of that slot would repeat the error.
    #[test]
    fn a_peek_that_grew_the_frame_is_undone() {
        let peeked = LogicalBox {
            x: 100,
            y: 72,
            width: 720,
            height: 508,
        };
        assert_eq!(
            unpeek(peeked, true, true),
            LogicalBox {
                x: 100,
                y: 100,
                width: 720,
                height: 480
            }
        );
    }

    /// Windows reserves the row permanently and only changes the clip region,
    /// so there is no growth to undo and subtracting one would corrupt the box.
    #[test]
    fn a_peek_that_did_not_grow_the_frame_is_left_alone() {
        let box_ = LogicalBox {
            x: 100,
            y: 100,
            width: 720,
            height: 480,
        };
        assert_eq!(unpeek(box_, true, false), box_);
    }

    #[test]
    fn a_box_read_outside_a_peek_is_left_alone() {
        let box_ = LogicalBox {
            x: 100,
            y: 100,
            width: 720,
            height: 480,
        };
        assert_eq!(unpeek(box_, false, true), box_);
    }
}
