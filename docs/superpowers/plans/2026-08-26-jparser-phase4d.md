# JParser Phase 4D — Layout Hotkeys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `Alt+1..9` restores a saved window layout, `Shift+Alt+1..9` binds one, and `Alt+[` / `Alt+]` step the parse history — all global, all working while a game holds the focus.

**Architecture:** The backend owns both paths. Bind reads the live window box plus `SettingsState` (which holds the *stored* `surface_alpha`, never the value the frontend clamps for `native` chrome). Restore merges a slot back, applies the window-level half in Rust, and emits one full `settings-changed` so both webviews apply the token-level half through appliers that already exist. A pure `fit` keeps a restored window on a monitor that still exists.

**Tech Stack:** Rust + Tauri 2.11.5, `tauri-plugin-global-shortcut`, vanilla TypeScript, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-08-26-jparser-phase4d-design.md`

## Global Constraints

- `src-tauri` tracks `rust-version = "1.88"`. Do not raise it.
- Project licence is **GPL-2.0-only**. A new dependency must be usable under a GPL-2.0-compatible licence — MIT, or `MIT OR Apache-2.0` taken under MIT. Apache-2.0 alone is incompatible and must be refused. Record which licence was taken, in a comment above the dependency, the way `tauri-plugin-clipboard-manager` does.
- Files stay under 800 lines. `src-tauri/src/commands.rs` is already 1918 and must not grow beyond the small extractions this plan names.
- `cargo clippy --all-targets` must stay at exactly four warnings, all `upper_case_acronyms`, on `RECT`, `STYLESTRUCT`, `SUBCLASSPROC`, `WINDOWPOS`. No new clippy debt.
- Tests are Arrange-Act-Assert with names that state the behaviour. Doc comments explain *why*, in the prose style of the surrounding code.
- Playwright screenshot baselines must **not** be regenerated. This phase changes no default rendering.
- Conventional Commits. Commit at the end of every task.
- Starting counts, for the gate in Task 11: Vitest 244, `cargo test` 88, Playwright 44.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src-tauri/src/settings.rs` | Modify. Gains `LayoutSlot` and `Settings.layout_slots` — the persisted shapes all live here. |
| `src-tauri/src/layout.rs` | **Create.** The whole feature: the pure geometry helpers (`fit`, `unpeek`), the slot merge (`apply_slot`, `slot_from`), `bind`, `restore`, the binding table, registration and dispatch. |
| `src-tauri/src/commands.rs` | Modify. Three small extractions (`merge_saved`, `apply_chrome`, `is_peeked`), `chrome_target` widened to `pub(crate)`, and the `shortcut_warning` command. |
| `src-tauri/src/state.rs` | Modify. `ShortcutWarning`, beside `SettingsWarning`. |
| `src-tauri/src/main.rs` | Modify. `mod layout;`, the plugin, the registration call, one handler registration. |
| `src-tauri/Cargo.toml` | Modify. One dependency. |
| `src/main.ts` | Modify. The `history-step` listener and `showShortcutWarning`. |
| `src/settings.ts` | Modify. The font boxes follow a full snapshot, including emptying. |
| `src/main.test.ts`, `src/settings.test.ts` | Modify. Frontend coverage. |

---

## Task 0: Verify the plugin before writing any code

**Files:** none. This task produces an answer, not a diff.

**Interfaces:**
- Consumes: nothing.
- Produces: confirmation (or correction) of the API every later task's code assumes.

`tauri-plugin-global-shortcut` is neither in `Cargo.lock` nor cached on this machine, so every line of Task 7 is currently an expectation rather than a fact.

- [ ] **Step 1: Add the dependency and let cargo resolve it**

```bash
cd src-tauri && cargo add tauri-plugin-global-shortcut@2
```

- [ ] **Step 2: Confirm the licence**

```bash
cd src-tauri && cargo metadata --format-version 1 \
  | python -c "import json,sys; [print(p['name'], p['license']) for p in json.load(sys.stdin)['packages'] if 'global-shortcut' in p['name'] or p['name'] == 'global-hotkey']"
```

Expected: `MIT OR Apache-2.0` for both the plugin and its `global-hotkey` dependency. **If either is Apache-2.0 only, stop and report** — the phase cannot ship it under GPL-2.0-only.

- [ ] **Step 3: Confirm five API facts**

Read the resolved source under `~/.cargo/registry/src/*/tauri-plugin-global-shortcut-*/src/`:

1. `Builder::new().with_handler(|app, shortcut, event| ...).build()` exists, and the handler's third argument carries a state discriminated by `ShortcutState::Pressed` / `Released`.
2. `app.global_shortcut().register(shortcut)` exists (the `GlobalShortcutExt` trait).
3. `Shortcut` parses from a string of the form `"Alt+Digit1"` / `"Shift+Alt+BracketLeft"` — via `FromStr`, `TryFrom<&str>`, or both.
4. `Shortcut` implements `Clone` and `PartialEq` (dispatch compares the fired shortcut against a registered table).
5. macOS needs no accessibility permission for it. Expected: true, because the backend is Carbon hot keys — but confirm rather than assert.

- [ ] **Step 4: Record the answers**

If all five hold, write one line into the commit body and continue. If any differs, **correct §4 of the design spec first**, then continue — later tasks quote that section's shape.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml Cargo.lock
git commit -m "chore(layout): add tauri-plugin-global-shortcut

Licence verified as MIT OR Apache-2.0, taken under MIT — Apache-2.0 alone
is incompatible with this project's GPL-2.0-only. API surface confirmed
against the resolved source: builder handler, ShortcutState, register,
string parsing, and Clone + PartialEq on Shortcut."
```

---

## Task 1: `LayoutSlot`, `Settings.layout_slots`, and the save that must not touch them

**Files:**
- Modify: `src-tauri/src/settings.rs` (add the struct and the key; extend the test module)
- Modify: `src-tauri/src/commands.rs:1108-1145` (extract `merge_saved` out of `save_settings`)

**Interfaces:**
- Consumes: `ChromeMode`, `Settings` (`settings.rs`).
- Produces: `pub struct LayoutSlot` with fields `window_width: u32`, `window_height: u32`, `window_x: i32`, `window_y: i32`, `chrome_mode: ChromeMode`, `always_on_top: bool`, `surface_alpha: Option<u32>`, `font_sentence: Option<u32>`, `font_gloss: Option<u32>`, `furigana_scale: Option<u32>`, `furigana_mode: Option<String>`, `extra: serde_json::Map<String, serde_json::Value>`; `Settings.layout_slots: BTreeMap<String, LayoutSlot>`; `pub(crate) fn merge_saved(current: &mut Settings, incoming: Settings)`.

- [ ] **Step 1: Write the failing tests**

Add to `settings.rs`'s `mod tests`:

```rust
    /// A slot fixture with every optional key set, so a round trip that drops
    /// one is visible.
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
    fn a_layout_slot_round_trips_through_the_file() {
        let path = scratch("layout-slot").join("settings.json");
        let mut written = Settings::default();
        written
            .layout_slots
            .insert("3".to_string(), slot_fixture());

        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert_eq!(read_back.layout_slots.get("3"), Some(&slot_fixture()));
    }

    /// The same reason the font sizes are `Option`: a slot bound while the
    /// boxes were empty has to restore them empty, so "unset" must survive the
    /// file as an absent key rather than as a number.
    #[test]
    fn a_slots_unset_keys_are_left_out_of_the_file() {
        let path = scratch("layout-slot-unset").join("settings.json");
        let mut written = Settings::default();
        written.layout_slots.insert(
            "1".to_string(),
            LayoutSlot {
                surface_alpha: None,
                font_sentence: None,
                font_gloss: None,
                furigana_scale: None,
                furigana_mode: None,
                ..slot_fixture()
            },
        );

        save(&path, &written).expect("save");
        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("surface_alpha"), "got {raw}");
        assert!(!raw.contains("font_sentence"), "got {raw}");
        assert_eq!(load(&path).0.layout_slots["1"].surface_alpha, None);
    }

    #[test]
    fn no_slots_means_no_key_in_the_file() {
        let path = scratch("layout-slot-empty").join("settings.json");
        save(&path, &Settings::default()).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("layout_slots"), "got {raw}");
    }

    /// The reason the key is a `String` and not a `u8`. `load` turns any parse
    /// failure into the whole file reset to defaults — so with a stricter key
    /// type, one hand-typed slot name would also cost the user their fonts,
    /// geometry, alpha and chrome mode. It costs them nothing instead.
    #[test]
    fn a_garbage_slot_key_is_ignored_without_resetting_the_file() {
        let path = scratch("layout-slot-garbage").join("settings.json");
        std::fs::write(
            &path,
            br#"{"always_on_top":true,"clipboard_monitoring":true,
                 "layout_slots":{"1a":{"window_width":720,"window_height":200,
                 "window_x":0,"window_y":0,"chrome_mode":"content",
                 "always_on_top":false}}}"#,
        )
        .expect("write");

        let (loaded, warning) = load(&path);
        assert!(warning.is_none(), "the file parsed, so nothing to warn about");
        assert!(loaded.always_on_top, "an unrelated key was reset");
        assert!(loaded.layout_slots.contains_key("1a"), "the key was dropped");
        assert!(loaded.layout_slots.get("1").is_none());
    }

    /// A slot key added by a later version must survive a rewrite by this one,
    /// exactly as `unknown_keys_survive_a_rewrite` requires of the top level.
    #[test]
    fn an_unknown_key_inside_a_slot_survives_a_rewrite() {
        let path = scratch("layout-slot-extra").join("settings.json");
        std::fs::write(
            &path,
            br#"{"always_on_top":false,"clipboard_monitoring":true,
                 "layout_slots":{"2":{"window_width":720,"window_height":200,
                 "window_x":0,"window_y":0,"chrome_mode":"header",
                 "always_on_top":false,"theme":"dark"}}}"#,
        )
        .expect("write");

        let (loaded, _) = load(&path);
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(raw.contains("\"theme\""), "got {raw}");
    }
```

And in `commands.rs`'s `mod tests`:

```rust
    /// The settings window's `currentSnapshot()` (`settings.ts:192`) does not
    /// build `layout_slots`, so it arrives here as serde's default — an empty
    /// map — on every save from that window. Assigning it would wipe every
    /// slot the moment the user changed the theme, and nothing in the code
    /// says "leave it alone", so only this test stops a future contributor
    /// completing the pattern.
    #[test]
    fn a_save_from_the_settings_window_leaves_the_layout_slots_alone() {
        let mut current = crate::settings::Settings::default();
        current.layout_slots.insert(
            "1".to_string(),
            crate::settings::LayoutSlot {
                window_width: 720,
                window_height: 200,
                window_x: 0,
                window_y: 0,
                chrome_mode: ChromeMode::Content,
                always_on_top: false,
                surface_alpha: Some(35),
                font_sentence: None,
                font_gloss: None,
                furigana_scale: None,
                furigana_mode: None,
                extra: serde_json::Map::new(),
            },
        );

        merge_saved(
            &mut current,
            crate::settings::Settings {
                furigana_mode: Some("katakana".to_string()),
                ..Default::default()
            },
        );

        assert_eq!(current.layout_slots.len(), 1, "a settings save wiped a slot");
        assert_eq!(current.layout_slots["1"].surface_alpha, Some(35));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test layout_slot merge_saved
```

Expected: compile failure — `LayoutSlot` and `merge_saved` do not exist.

- [ ] **Step 3: Add the struct and the key**

In `settings.rs`, add `use std::collections::BTreeMap;` to the imports and, immediately after `ChromeMode`'s `supported`:

```rust
/// One layout slot (§7.9): the whole shape of the window, restorable with one
/// keystroke.
///
/// Geometry and `chrome_mode` are not `Option` — a bind always has a real
/// window box and a mode that has already been through `supported()`. The rest
/// are, for the reason 3C and 4B both give: "unset, `tokens.css` owns the
/// default" is a reachable state, and a slot bound while the font boxes were
/// empty has to restore them empty rather than silently pinning 24 px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutSlot {
    pub window_width: u32,
    pub window_height: u32,
    pub window_x: i32,
    pub window_y: i32,
    pub chrome_mode: ChromeMode,
    pub always_on_top: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_alpha: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_sentence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_gloss: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_mode: Option<String>,
    /// Keys a later version added, carried through on rewrite for the same
    /// reason `Settings::extra` exists.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

In `Settings`, after `window_y`:

```rust
    /// The nine layout slots (§7.9), keyed by digit, holding only the ones
    /// actually bound.
    ///
    /// A `String` key rather than a `u8`: `load` turns any parse failure into
    /// the whole file reset to defaults, so a stricter key type would make one
    /// hand-typed slot name cost the user every other setting too. An unknown
    /// key here is simply never looked up.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout_slots: BTreeMap<String, LayoutSlot>,
```

And in `impl Default for Settings`, after `window_y: None,`:

```rust
            layout_slots: BTreeMap::new(),
```

Two existing tests construct `Settings` with every field listed
(`settings_roundtrips_furigana_mode`, `settings_roundtrips_gloss_filters`) —
add `layout_slots: BTreeMap::new(),` to both.

- [ ] **Step 4: Extract `merge_saved`**

In `commands.rs`, replace `save_settings`'s body with a call, moving the merge itself out — the closure body is unchanged, only relocated:

```rust
/// The merge `save_settings` performs, without the state wrapper.
///
/// Split out for the reason `resolve_chrome_mode` is: a `#[tauri::command]`
/// taking `State<'_, _>` cannot be called from a test, and the rule this
/// function encodes — which keys are merged, which are assigned, and which are
/// not touched at all — is worth pinning.
///
/// `layout_slots` is deliberately absent. The settings window never sends it,
/// so it arrives as an empty map on every save; assigning it would wipe every
/// bound slot.
pub(crate) fn merge_saved(current: &mut Settings, incoming: Settings) {
    if incoming.furigana_mode.is_some() {
        current.furigana_mode = incoming.furigana_mode;
    }
    if incoming.hide_pos.is_some() {
        current.hide_pos = incoming.hide_pos;
    }
    if incoming.hide_xrefs.is_some() {
        current.hide_xrefs = incoming.hide_xrefs;
    }
    if incoming.hide_usage.is_some() {
        current.hide_usage = incoming.hide_usage;
    }
    // Merged like the toggles above rather than assigned like the sizes below:
    // a range input cannot be cleared, so alpha has no "absent means reset"
    // state, and an unguarded assign would only give a future caller a way to
    // wipe the value by omission.
    if incoming.surface_alpha.is_some() {
        current.surface_alpha = incoming.surface_alpha;
    }
    // Assigned rather than merged, unlike the toggles above: an absent size
    // means the user cleared that box and wants the stylesheet default back,
    // which a merge could never express. The settings window is the only
    // caller and always sends all three.
    current.font_sentence = incoming.font_sentence;
    current.font_gloss = incoming.font_gloss;
    current.furigana_scale = incoming.furigana_scale;
}

#[tauri::command]
pub fn save_settings(
    settings: Settings,
    state: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    state
        .update(|s| merge_saved(s, settings))
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test
```

Expected: PASS, with six more tests than the starting 88.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/commands.rs
git commit -m "feat(layout): persist layout slots"
```

---

## Task 2: `layout.rs` and the geometry fit

**Files:**
- Create: `src-tauri/src/layout.rs`
- Modify: `src-tauri/src/main.rs` (add `mod layout;`)

**Interfaces:**
- Consumes: nothing (pure).
- Produces: `pub struct LogicalBox { x: i32, y: i32, width: u32, height: u32 }`; `pub struct MonitorRect { x: i32, y: i32, width: u32, height: u32, scale: f64 }`; `pub struct PhysRect { x: i32, y: i32, width: u32, height: u32 }`; `pub const MIN_LOGICAL_WIDTH: u32`; `pub const MIN_LOGICAL_HEIGHT: u32`; `pub fn fit(slot: LogicalBox, monitors: &[MonitorRect], primary: usize, scale_hint: f64) -> PhysRect`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/layout.rs` containing only the licence header, the module doc, and this test module:

```rust
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
}
```

Add `mod layout;` to `main.rs`'s module list (after `mod clipboard;`, keeping the list alphabetical: `clipboard`, `commands`, `layout`, `mouse_tracker`, …).

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test layout::
```

Expected: compile failure — `LogicalBox`, `MonitorRect`, `PhysRect`, `fit` do not exist.

- [ ] **Step 3: Write the implementation**

Above the test module in `layout.rs`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test layout::
```

Expected: PASS, seven tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/layout.rs src-tauri/src/main.rs
git commit -m "feat(layout): fit a stored box onto a live monitor"
```

---

## Task 3: `unpeek`, and reading the peek flag

**Files:**
- Modify: `src-tauri/src/layout.rs`
- Modify: `src-tauri/src/commands.rs` (a reader for `PEEKED`, near its declaration at `commands.rs:940`)

**Interfaces:**
- Consumes: `LogicalBox` (Task 2); `commands::BAND_HEIGHT_LOGICAL`.
- Produces: `pub fn unpeek(box_: LogicalBox, peeked: bool, grows_frame: bool) -> LogicalBox`; `commands::is_peeked() -> bool`.

- [ ] **Step 1: Write the failing tests**

In `layout.rs`'s `mod tests`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test layout::tests::a_peek
```

Expected: compile failure — `unpeek` does not exist.

- [ ] **Step 3: Write the implementation**

In `layout.rs`:

```rust
use crate::commands::BAND_HEIGHT_LOGICAL;

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
```

In `commands.rs`, directly under `static PEEKED`:

```rust
/// Whether a peek is showing right now.
///
/// Read by `layout::bind`, which has to correct a box captured mid-peek. The
/// flag stays private so `peek_titlebar` remains its only writer.
pub(crate) fn is_peeked() -> bool {
    PEEKED.load(Ordering::SeqCst)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test layout::
```

Expected: PASS, ten tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/layout.rs src-tauri/src/commands.rs
git commit -m "feat(layout): undo a peek when capturing a box"
```

---

## Task 4: `apply_slot` — restoring by assignment

**Files:**
- Modify: `src-tauri/src/layout.rs`

**Interfaces:**
- Consumes: `settings::{LayoutSlot, Settings, ChromeMode, supported}` (Task 1).
- Produces: `pub fn apply_slot(settings: &mut Settings, slot: &LayoutSlot, native_supported: bool)`.

- [ ] **Step 1: Write the failing tests**

In `layout.rs`'s `mod tests`, plus `use crate::settings::{ChromeMode, LayoutSlot, Settings};` at the top of the test module:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test layout::tests::applying
```

Expected: compile failure — `apply_slot` does not exist.

- [ ] **Step 3: Write the implementation**

```rust
use crate::settings::{supported, ChromeMode, LayoutSlot, Settings};

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
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test layout::
```

Expected: PASS, thirteen tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/layout.rs
git commit -m "feat(layout): apply a slot over the live settings"
```

---

## Task 5: Bind — capture the live box and the stored settings

**Files:**
- Modify: `src-tauri/src/layout.rs`
- Modify: `src-tauri/src/commands.rs:76` (widen `chrome_target` to `pub(crate)`)

**Interfaces:**
- Consumes: `LogicalBox`, `unpeek` (Tasks 2–3); `apply_slot`'s neighbours in `settings` (Task 1); `commands::{chrome_target, is_peeked, peek_grows_frame}`.
- Produces: `pub fn slot_key(index: u8) -> Option<String>`; `pub fn slot_from(settings: &Settings, box_: LogicalBox) -> LayoutSlot`; `pub fn bind(app: &tauri::AppHandle, index: u8) -> Result<(), String>`.

- [ ] **Step 1: Write the failing tests**

In `layout.rs`'s `mod tests`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test layout::tests::a_bound_slot layout::tests::only_the_nine
```

Expected: compile failure — `slot_from` and `slot_key` do not exist.

- [ ] **Step 3: Write the implementation**

In `layout.rs`:

```rust
use std::sync::Arc;

use tauri::Manager;

use crate::state::SettingsState;

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
```

In `commands.rs:76`, widen the helper so `layout` can reach the window through it:

```rust
pub(crate) fn chrome_target<R: tauri::Runtime>(
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test layout:: && cargo clippy --all-targets 2>&1 | tail -5
```

Expected: PASS, seventeen tests; clippy still at four `upper_case_acronyms`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/layout.rs src-tauri/src/commands.rs
git commit -m "feat(layout): bind the current layout to a slot"
```

---

## Task 6: Restore — apply the window half, emit the rest

**Files:**
- Modify: `src-tauri/src/commands.rs:901-940` (extract `apply_chrome` from `set_chrome_mode`)
- Modify: `src-tauri/src/layout.rs`

**Interfaces:**
- Consumes: `fit`, `apply_slot`, `slot_key` (Tasks 2, 4, 5); `commands::{chrome_target, apply_chrome}`.
- Produces: `commands::apply_chrome(&tauri::Window, ChromeMode) -> Result<(), String>`; `layout::restore(app: &tauri::AppHandle, index: u8) -> Result<(), String>`.

- [ ] **Step 1: Extract `apply_chrome`**

There is no new behaviour to test here — this is a move, covered by the existing `set_chrome_mode` tests. In `commands.rs`, lift the per-platform tail of `set_chrome_mode` into:

```rust
/// The per-platform half of `set_chrome_mode`: everything after the mode has
/// been resolved and persisted.
///
/// Shared with `layout::restore`, which resolves and persists a whole slot and
/// then needs exactly this. Extracted rather than duplicated so the three-way
/// platform split stays in one place.
pub(crate) fn apply_chrome(window: &tauri::Window, mode: ChromeMode) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    apply_decorations_macos(window, mode == ChromeMode::Native)?;
    #[cfg(target_os = "windows")]
    {
        // No `grow_top` here any more. It existed to shed and regain the
        // reserved row across the native boundary, and `supported` means
        // Windows never crosses it — `header` and `content` reserve the row
        // identically, so switching between them must move nothing.
        win32_frameless::init_window(window);
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = window.set_decorations(mode == ChromeMode::Native);
    }
    // Windows alone reads nothing from `mode`: the reserved row is identical
    // in `header` and `content`.
    let _ = mode;
    Ok(())
}
```

and reduce `set_chrome_mode` to:

```rust
#[tauri::command]
pub fn set_chrome_mode(
    mode: ChromeMode,
    app: tauri::AppHandle,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    let mode = resolve_chrome_mode(mode, &settings)?;
    apply_chrome(&chrome_target(&app)?, mode)
}
```

- [ ] **Step 2: Run the existing suite to prove the move changed nothing**

```bash
cd src-tauri && cargo test && cargo build
```

Expected: PASS, same count as after Task 5.

- [ ] **Step 3: Commit the extraction on its own**

```bash
git add src-tauri/src/commands.rs
git commit -m "refactor(chrome): share the per-platform apply with layout"
```

- [ ] **Step 4: Write the failing test for the slot lookup**

Restore's own body needs a window, so what gets a test is the decision that
does not: an unbound slot is a no-op, not an error. In `layout.rs`'s
`mod tests`:

```rust
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
```

- [ ] **Step 5: Run it to verify it fails**

```bash
cd src-tauri && cargo test layout::tests::an_unbound_slot
```

Expected: compile failure — `slot_to_restore` does not exist.

- [ ] **Step 6: Write the implementation**

```rust
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
fn monitors(window: &tauri::Window) -> Result<(Vec<MonitorRect>, usize), String> {
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
    state
        .update(|s| apply_slot(s, &slot, native_supported))
        .map_err(|e| e.to_string())?;

    let window = crate::commands::chrome_target(app)?;
    let (rects, primary) = monitors(&window)?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let fitted = fit(
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
    window
        .set_size(tauri::PhysicalSize::new(fitted.width, fitted.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::PhysicalPosition::new(fitted.x, fitted.y))
        .map_err(|e| e.to_string())?;
    window
        .set_always_on_top(slot.always_on_top)
        .map_err(|e| e.to_string())?;
    crate::commands::apply_chrome(&window, mode)?;

    // One full snapshot carries the token-level half — alpha, the three sizes,
    // the furigana mode — to both webviews. It works because of attributes
    // already on `Settings`: every optional key skips serialization when
    // absent, which is exactly what `applyFontSizes` reads as "clear it", and
    // `always_on_top` + `clipboard_monitoring` are always present, which is
    // what gets the payload past `isFullSnapshot` (`main.ts:639`).
    app.emit("settings-changed", state.snapshot())
        .map_err(|e| e.to_string())
}
```

Add `use tauri::{Emitter, Manager};` to `layout.rs`'s imports.

- [ ] **Step 7: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test layout:: && cargo build
```

Expected: PASS, twenty tests.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/layout.rs
git commit -m "feat(layout): restore a slot"
```

---

## Task 7: The plugin, the twenty registrations, and the taken-key warning

**Files:**
- Modify: `src-tauri/src/layout.rs`
- Modify: `src-tauri/src/state.rs` (add `ShortcutWarning` beside `SettingsWarning` at `state.rs:66`)
- Modify: `src-tauri/src/commands.rs` (add `shortcut_warning`, mirroring `settings_warning` at `commands.rs:1228`)
- Modify: `src-tauri/src/main.rs` (plugin, registration, handler list)
- Modify: `src/main.ts`, `src/main.test.ts`

**Interfaces:**
- Consumes: `bind`, `restore` (Tasks 5–6).
- Produces: `pub enum Action { Restore(u8), Bind(u8), History(i8) }`; `pub const BINDINGS: [(&str, Action); 20]`; `pub const HISTORY_STEP_EVENT: &str`; `pub fn register_all(app: &tauri::AppHandle) -> Vec<String>`; `pub fn warning(taken: &[String]) -> String`; `pub fn dispatch(app: &tauri::AppHandle, fired: &Shortcut)`; `state::ShortcutWarning(pub String)`; the `shortcut_warning` command; `showShortcutWarning()` in `main.ts`.

- [ ] **Step 1: Write the failing tests**

In `layout.rs`'s `mod tests`:

```rust
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
```

In `src/main.test.ts`, inside the existing top-level describe that already
covers `showSettingsWarning` (search for `settings_warning`):

```ts
  test('a hotkey another app owns is reported in the window', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'shortcut_warning') {
        return Promise.resolve('These layout hotkeys are already in use: Alt+Digit5');
      }
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showShortcutWarning();

    expect(document.querySelector('#parse-error')?.textContent).toContain('Alt+Digit5');
  });

  test('a settings warning and a hotkey warning both survive', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'settings_warning') return Promise.resolve('settings.json could not be read');
      if (cmd === 'shortcut_warning') return Promise.resolve('hotkeys in use: Alt+Digit5');
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showSettingsWarning();
    await main.showShortcutWarning();

    const text = document.querySelector('#parse-error')?.textContent ?? '';
    expect(text).toContain('could not be read');
    expect(text).toContain('Alt+Digit5');
  });
```

- [ ] **Step 2: Run both suites to verify they fail**

```bash
cd src-tauri && cargo test layout::tests::the_binding_table
cd .. && npx vitest run src/main.test.ts -t 'hotkey another app'
```

Expected: Rust compile failure (`BINDINGS` missing); Vitest failure on
`main.showShortcutWarning is not a function`.

- [ ] **Step 3: Write the Rust side**

In `layout.rs`:

```rust
use std::sync::OnceLock;

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

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
        match app.global_shortcut().register(shortcut.clone()) {
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
```

In `state.rs`, under `SettingsWarning`:

```rust
/// The layout hotkeys another application already owns, as one sentence, or
/// empty when there are none.
///
/// Separate from `SettingsWarning` for the reason that state's own doc gives
/// for being separate from `StartupFailure`: one managed string per warning
/// kind, one command each, because the messages are independent and either can
/// be present without the other.
pub struct ShortcutWarning(pub String);
```

In `commands.rs`, beside `settings_warning`:

```rust
/// The hotkey registration warning, or `None` when every key took.
#[tauri::command]
pub fn shortcut_warning(warning: State<'_, ShortcutWarning>) -> Option<String> {
    if warning.0.is_empty() {
        None
    } else {
        Some(warning.0.clone())
    }
}
```

adding `ShortcutWarning` to the `use crate::state::{...}` list.

In `main.rs`: add the plugin to the builder chain, register in `setup`, and add
the command to the handler list.

```rust
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
```

and inside `setup`, after `commands::create_settings_window(app)?;`:

```rust
            // After the windows exist: a hotkey pressed the instant the app
            // starts must find a window to act on.
            let taken = layout::register_all(app.handle());
            app.manage(state::ShortcutWarning(layout::warning(&taken)));
            if !taken.is_empty() {
                eprintln!("hotkeys already in use: {}", taken.join(", "));
            }
```

and `commands::shortcut_warning,` in `generate_handler!`.

- [ ] **Step 4: Write the frontend side**

In `src/main.ts`, directly after `showSettingsWarning`:

```ts
// A hotkey another application owns is the one failure in the layout-slot
// feature that would otherwise be invisible: the keys would simply do nothing.
// Exported for the same reason `showStartupError` is.
export async function showShortcutWarning(): Promise<void> {
  const message = await invoke<string | null>('shortcut_warning');
  if (message === null) return;
  // `append`, not `replaceChildren`: a corrupt settings.json puts its own,
  // unrelated warning in this slot, and both are worth showing. The next
  // successful parse clears the slot, exactly as it already does for that one.
  parseError.append(errorBlock(message));
}
```

and beside the existing fire-and-forget calls at the bottom of the module:

```ts
void showShortcutWarning();
```

- [ ] **Step 5: Run both suites to verify they pass**

```bash
cd src-tauri && cargo test && cargo build
cd .. && npx vitest run src/main.test.ts
```

Expected: PASS on both.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/layout.rs src-tauri/src/state.rs src-tauri/src/commands.rs src-tauri/src/main.rs src/main.ts src/main.test.ts
git commit -m "feat(layout): register the global hotkeys"
```

---

## Task 8: `Alt+[` and `Alt+]` reach the history

**Files:**
- Modify: `src/main.ts` (a listener beside the chevrons' click handlers at `main.ts:1191`)
- Modify: `src/main.test.ts`

**Interfaces:**
- Consumes: `layout::HISTORY_STEP_EVENT` (`'history-step'`, payload `-1` | `1`) from Task 7; `history` and `display` in `main.ts`.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing tests**

In `src/main.test.ts`, inside `describe('the header history buttons', ...)` so
the fixtures and `beforeEach` are already in place:

```ts
  test('a history-step event walks back and forward', () => {
    emit('parse-result', parse('一'));
    emit('parse-result', parse('二'));

    emit('history-step', -1);
    expect(document.querySelector('#output')?.textContent).toContain('一');

    emit('history-step', 1);
    expect(document.querySelector('#output')?.textContent).toContain('二');
  });

  test('a history-step at either end changes nothing', () => {
    emit('parse-result', parse('一'));

    emit('history-step', -1);
    emit('history-step', 1);

    expect(document.querySelector('#output')?.textContent).toContain('一');
  });

  // The chevrons and the keys share one cursor, so the buttons have to follow
  // a keyboard step or they would advertise the wrong state.
  test('a history-step updates the chevrons', () => {
    emit('parse-result', parse('一'));
    emit('parse-result', parse('二'));

    emit('history-step', -1);

    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(false);
  });
```

- [ ] **Step 2: Run them to verify they fail**

```bash
npx vitest run src/main.test.ts -t 'history-step'
```

Expected: FAIL with `nothing listening for history-step`.

- [ ] **Step 3: Write the implementation**

In `src/main.ts`, immediately after the two chevron click handlers:

```ts
// The global `Alt+[` / `Alt+]` pair (§7.9). Delegated to the same one-liner
// the chevrons use, so the disabled-state sync and the popover close in
// `display` come along for free — and `back()`/`forward()` return `null` at
// either end, which `display` already ignores.
void listen<number>(HISTORY_STEP_EVENT, (e) =>
  display(e.payload < 0 ? history.back() : history.forward()),
).catch(() => {
  // Same reasoning as the geometry listeners above: a listener that fails to
  // register leaves the keys inert, which is not worth taking the app down for.
});
```

with the event name declared beside the module's other constants:

```ts
/** Mirrors `layout::HISTORY_STEP_EVENT`. */
const HISTORY_STEP_EVENT = 'history-step';
```

- [ ] **Step 4: Run them to verify they pass**

```bash
npx vitest run src/main.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/main.ts src/main.test.ts
git commit -m "feat(history): step the history from the keyboard"
```

---

## Task 9: The settings window's font boxes follow a restore

**Files:**
- Modify: `src/settings.ts:218-226` (inside `applySettings`)
- Modify: `src/settings.test.ts`

**Interfaces:**
- Consumes: the full `Settings` payload restore emits (Task 6).
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing tests**

In `src/settings.test.ts`:

```ts
  test('a full snapshot with no font sizes empties the boxes', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          font_sentence: 30,
          font_gloss: 18,
          furigana_scale: 70,
        });
      }
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    // What `layout::restore` emits for a slot that recorded the sizes unset:
    // a full snapshot whose three font keys are simply absent.
    emit('settings-changed', { always_on_top: false, clipboard_monitoring: true });

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')!.value).toBe('');
    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')!.value).toBe('');
    expect(container.querySelector<HTMLInputElement>('#setting-furigana-scale')!.value).toBe('');
  });

  test('a partial payload leaves the font boxes alone', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          font_sentence: 30,
        });
      }
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    // A slider drag emits this sixty times a second. Clearing the boxes on it
    // would reproduce 4B ruling 1's bug on this surface.
    emit('settings-changed', { surface_alpha: 40 });

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')!.value).toBe('30');
  });
```

All three ids are confirmed against `settings.ts`'s own template:
`#setting-font-sentence`, `#setting-font-gloss`, `#setting-furigana-scale`.

- [ ] **Step 2: Run them to verify they fail**

```bash
npx vitest run src/settings.test.ts -t 'font boxes'
```

Expected: the first test FAILS — the box still reads `30`. The second passes
already; it is the regression guard for the change.

- [ ] **Step 3: Write the implementation**

Replace the three font lines in `applySettings`:

```ts
    // Full snapshots own the boxes outright, including emptying them: an
    // absent size in a full payload means the user cleared that box, or a
    // restored layout slot recorded it as unset. A partial payload —
    // `{ surface_alpha }` from a slider drag, `{ chrome_mode }` from the main
    // window's menu — carries no sizes at all, and clearing on those would
    // reproduce 4B ruling 1's bug on this surface. Discriminated by the two
    // keys `Settings` does not mark optional, exactly as `main.ts` does.
    const isFullSnapshot =
      s.always_on_top !== undefined && s.clipboard_monitoring !== undefined;
    if (isFullSnapshot) {
      fontSentence.value = s.font_sentence === undefined ? '' : String(s.font_sentence);
      fontGloss.value = s.font_gloss === undefined ? '' : String(s.font_gloss);
      furiganaScale.value = s.furigana_scale === undefined ? '' : String(s.furigana_scale);
    }
```

The initial `get_settings` path is unaffected: it passes a full snapshot and is
followed by `showDefaultsInEmptyBoxes()`, which seeds whatever this leaves
empty — the same sequence as before.

- [ ] **Step 4: Run the suite to verify it passes**

```bash
npx vitest run src/settings.test.ts
```

Expected: PASS, including every existing font-box test.

- [ ] **Step 5: Commit**

```bash
git add src/settings.ts src/settings.test.ts
git commit -m "fix(settings): let a full snapshot empty the font boxes"
```

---

## Task 10: One emit moves everything

**Files:**
- Modify: `src/main.test.ts`

**Interfaces:**
- Consumes: the payload shape `layout::restore` emits (Task 6).
- Produces: nothing.

This is the test that pins the contract Task 6's whole design rests on: that
one `settings-changed` carrying a full `Settings` is enough for the main
window's half of a restore.

- [ ] **Step 1: Write the failing test**

In `src/main.test.ts`, in the describe that already covers `settings-changed`:

```ts
  test('one restore snapshot moves the alpha, the sizes, the furigana and the chrome', () => {
    const root = document.documentElement;

    // Exactly what `layout::restore` emits: a full `Settings`, with the keys a
    // slot recorded as unset simply absent.
    emit('settings-changed', {
      always_on_top: true,
      clipboard_monitoring: true,
      chrome_mode: 'content' as ChromeMode,
      surface_alpha: 35,
      font_sentence: 30,
      furigana_mode: 'hiragana',
    });

    expect(root.style.getPropertyValue('--surface-alpha')).toBe('0.35');
    expect(root.style.getPropertyValue('--text-sentence')).toBe('30px');
    expect(document.querySelector('#app')?.getAttribute('data-chrome')).toBe('content');
    // Absent in the payload, so the token has to be gone rather than stale —
    // this is the half of the contract that depends on serde omitting the key.
    expect(root.style.getPropertyValue('--text-gloss')).toBe('');
  });
```

Check the expected `--surface-alpha` string against `surfaceAlphaVar`'s actual
output for 35 (`src/surface-alpha.ts`) and the `--text-sentence` unit against
`fontVars` (`src/font-sizes.ts`); use what those functions produce, not what
this test guesses.

- [ ] **Step 2: Run it**

```bash
npx vitest run src/main.test.ts -t 'one restore snapshot'
```

Expected: PASS if `--text-gloss` was already clear, FAIL on a stale token if a
previous test leaked state — in which case set the leaked tokens in the
Arrange half so the assertion is about this emit and nothing else.

- [ ] **Step 3: Commit**

```bash
git add src/main.test.ts
git commit -m "test(layout): pin the one-emit restore contract"
```

---

## Task 11: The gate, the manual checks, and the handoff

**Files:**
- Create: `docs/superpowers/2026-08-26-jparser-phase4d-handoff.md`
- Modify: `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§11's 4D row, §12's new rows)

**Interfaces:**
- Consumes: everything.
- Produces: the handoff.

- [ ] **Step 1: Run the whole gate**

```bash
npx vitest run
npm run build
npx playwright test
cd src-tauri && cargo test
cd src-tauri && cargo clippy --all-targets
```

Expected: Vitest above 244; `cargo test` above 88; Playwright 44 with **no**
regenerated baselines; clippy at exactly four `upper_case_acronyms` warnings.
Record the real numbers — they go in the handoff.

- [ ] **Step 2: Run the app and work the manual list**

```bash
npm run tauri dev
```

Work spec §8.4 in order, writing down what each check actually showed:

1. Bind content mode + 35 % alpha + large fonts to slot 1, header-mode
   defaults to slot 2, alternate — all six captured things move each way.
2. A slot bound with the font boxes empty restores to empty.
3. Bind on a second monitor, unplug it, restore — the window lands on the
   remaining screen.
4. Hold `Alt+3` — the repeats are harmless.
5. Run with another application owning `Alt+5` — the startup warning names it.
6. macOS/Linux only: restore a 40 % slot while chrome is `native` — the screen
   clamps to 100 %, and `settings.json` still reads 40.
7. macOS only: bind during a peek — the stored height carries no extra band.

- [ ] **Step 3: Write the handoff**

`docs/superpowers/2026-08-26-jparser-phase4d-handoff.md`, following 4B's
handoff structure: what shipped, the rulings taken during execution, the
verification numbers from Step 1, and — by name, individually — **every check
from Step 2 that did not run**. An unrun check that goes unmentioned reads to
the next phase as a check that passed.

- [ ] **Step 4: Update the master spec**

- §11's 4D row: past tense, and note the two deviations.
- §11's Phase 4 heading and intro: "4A, 4B, 4C and 4D shipped" — Phase 4 is
  complete.
- §12: add rows 13 and 14 exactly as the design spec §10 states them.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/2026-08-26-jparser-phase4d-handoff.md docs/superpowers/specs/2026-08-12-jparser-port-design.md
git commit -m "docs(phase4d): close the phase"
```

---

## Self-Review Notes

**Spec coverage.** §3.1–3.2 → Task 1. §3.3 → Task 1 Step 4 plus its test.
§3.4 → Task 2. §4.1 → Task 0. §4.2 → Task 7's table and its three tests.
§4.3 → Task 7's handler gate (the comment and the `ShortcutState` check) and
the auto-repeat note recorded in the design spec. §4.4 → Task 7's
`ShortcutWarning`, `shortcut_warning`, and `showShortcutWarning`. §5.1 → Tasks
3 and 5. §5.2 → Task 6. §5.3 → Tasks 6, 9 and 10. §5.4 → Task 8. §6 → Task 2.
§7 → this plan's task order. §8 → Tasks 1–10 for the automated half, Task 11
for the gate and the manual list. §9's deferrals are absent by construction.
§10 → Task 11 Step 4.

**Type consistency.** `LogicalBox`/`MonitorRect`/`PhysRect` are defined in
Task 2 and used with the same field names in Tasks 3, 5 and 6. `LayoutSlot`'s
field names are identical in Tasks 1, 4, 5 and 6. `apply_slot` and
`slot_from`'s signatures are quoted identically in their Interfaces blocks and
their implementations. `HISTORY_STEP_EVENT` is `'history-step'` on both sides
(Tasks 7 and 8).

**What a task must check rather than assume.** The plugin API — all five facts
in Task 0, none of which could be verified offline — and the exact strings
`surfaceAlphaVar` and `fontVars` return, which Task 10 asserts against and says
so at the point of use. Everything else in this plan was read out of the files
it names.
