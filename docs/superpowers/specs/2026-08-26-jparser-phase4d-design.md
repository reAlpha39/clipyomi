# JParser Phase 4D — Layout Hotkeys Design

**Date:** 2026-08-26
**Status:** Designed, not started.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.9, §8, §11, §12)
**Depends on:** 3C (font sizes), 4A (`chrome_mode`), 4B (`surface_alpha`) — a slot
captures the state all three introduced. 4C settles one thing for this phase:
a slot does **not** capture history (4C handoff §4).

---

## 1. Goal

§7.9 asks for ta-old's `alt-#` / `shift-alt-#`: nine layout slots, restored by
`Alt+1..9` and bound by `Shift+Alt+1..9`. A slot captures geometry, chrome
mode, background alpha, always-on-top, the three font sizes, and the furigana
mode — the whole shape of the window, in one keystroke, from inside a game that
holds the focus.

This is the last sub-phase of Phase 4, and it is last because a slot captures
the state 3C, 4A and 4B introduce. All three have shipped.

Beyond §7.9's scope, this phase also adds two global history keys, `Alt+[` and
`Alt+]`. 4A §9 deferred keyboard history navigation to "4D's `Alt+1..9`
surface" precisely so it would land here rather than duplicating the header's
chevrons; content-only mode has had no way to reach back/forward at all since
4A shipped. Recorded as deviation #13 (§10).

---

## 2. Scope

**In:**

- `tauri-plugin-global-shortcut`, registered entirely in Rust.
- Twenty global shortcuts: `Alt+Digit1..9`, `Shift+Alt+Digit1..9`,
  `Alt+BracketLeft`, `Alt+BracketRight`.
- `Settings.layout_slots` — a map of occupied slots, persisted in
  `settings.json`.
- Bind: capture the live window box plus the persisted settings into a slot.
- Restore: merge a slot back, apply the window-level half in Rust, and emit one
  full `settings-changed` for the token-level half.
- A pure geometry fit that keeps a restored window on a monitor that still
  exists.
- A startup warning naming any shortcut another application already owns.

**Out:**

- A transient "Bound 3" indicator. §9 gives the reason.
- User-editable key bindings.
- A slot list / clear affordance in the settings window.
- Disk-persisted history inside a slot. 4C handoff §4 rules this out
  permanently: history is session-only and holds no persisted key, and if disk
  persistence is ever built it belongs in its own file.

---

## 3. What a slot holds, and where it lives

### 3.1 The shape

```json
"layout_slots": {
  "3": {
    "window_width": 720, "window_height": 200, "window_x": 1200, "window_y": 40,
    "chrome_mode": "content",
    "always_on_top": true,
    "surface_alpha": 35,
    "font_sentence": 30,
    "furigana_mode": "hiragana"
  }
}
```

In `settings.rs`, beside `ChromeMode` — this is a persisted shape, and
`settings.rs` already owns every other one:

```rust
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
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}
```

and on `Settings`:

```rust
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub layout_slots: BTreeMap<String, LayoutSlot>,
```

### 3.2 Four shape decisions

**A map, not `[Option<LayoutSlot>; 9]`.** Only occupied slots reach the file.
An array would write nine `null`s the first time anything saves.

**A `String` key, not a `u8`.** This is the decision with real consequences.
`load` turns *any* parse failure into the whole settings file reset to defaults
plus a warning (`settings.rs:186`) — the behaviour `an_unknown_mode_name_falls_
back_to_defaults_with_a_warning` pins deliberately. With a `u8` key, one
hand-typed `"1a"` inside `layout_slots` costs the user their fonts, geometry,
alpha and chrome mode as well. With a `String` key it costs them nothing: the
key is simply never looked up, and `extra`-style pass-through keeps it in the
file. Nine slots is nine times the surface for that typo, which is what tips
this away from the stricter type.

**Geometry and `chrome_mode` are not `Option`.** A bind always has a real
window box and a mode that has already been through `supported()`. There is no
"unset" to represent.

**Alpha, the three sizes, and the furigana mode are `Option`.** "Unset —
`tokens.css` owns the default" is a real, reachable state that 3C and 4B both
went out of their way to keep distinguishable from a value that happens to
equal the default. A slot bound while the font boxes are empty has to restore
*empty*, not silently pin `tokens.css`'s 24 px. This is also why restore assigns rather than
merges (§5.2).

**`extra` on the slot as well as on `Settings`.** Same data-loss class the
existing `unknown_keys_survive_a_rewrite` test exists to prevent: a slot key
added by a later version must survive a rewrite by this one.

### 3.3 `save_settings` must not learn about `layout_slots`

`save_settings` (`commands.rs:1108`) merges a snapshot key by key and only
touches keys it names. The settings window's `currentSnapshot()`
(`settings.ts:192`) does not build `layout_slots`, so the key arrives as
`#[serde(default)]` — an empty map — on every single save from that window.
Assigning it would wipe every slot the moment the user changed the theme.

Not naming it is therefore the whole fix, and it is invisible in the diff. It
gets a test of its own for exactly that reason: nothing in the code says
"leave `layout_slots` alone", so only a test stops a future contributor
completing the pattern by adding the assignment.

### 3.4 A new module

`src-tauri/src/layout.rs` holds the two paths and the pure helpers.

Not `commands.rs`: that file is 1918 lines against the project's 800-line
ceiling, and `popover.rs` is already the precedent for a feature owning its own
module including its own commands. `LayoutSlot` itself stays in `settings.rs`
with the other persisted shapes.

`bind` and `restore` are **plain functions**, not `#[tauri::command]`s. The
shortcut handler is the only caller; they take an `&AppHandle` and reach state
through `app.state::<Arc<SettingsState>>()`. Nothing goes into
`invoke_handler`, nothing goes into `capabilities/`, and the parts worth
testing are pure functions that need no window at all (§8.1).

---

## 4. Registration

### 4.1 The dependency

`tauri-plugin-global-shortcut = "2"` in `src-tauri/Cargo.toml`, pinned to the
exact resolved version the way `tauri-plugin-clipboard-manager = "2.3.2"` is.

It is MIT OR Apache-2.0, taken under **MIT** — Apache-2.0 alone is incompatible
with this project's GPL-2.0-only, which is the note the clipboard plugin
already carries and the reason it carries it.

The crate is not in `Cargo.lock` and is not cached on the development machine,
so **Task 0 verifies the license and the API surface before any code is
written** (§7.1). Nothing downstream in this document should be read as an
assertion about that API; it is the shape the design expects, to be confirmed.

### 4.2 The twenty shortcuts

| Keys | Action |
| --- | --- |
| `Alt+Digit1` … `Alt+Digit9` | Restore slot 1–9 |
| `Shift+Alt+Digit1` … `Shift+Alt+Digit9` | Bind slot 1–9 |
| `Alt+BracketLeft` | History back |
| `Alt+BracketRight` | History forward |

Modifier matching is exact, so `Alt+Digit1` and `Shift+Alt+Digit1` do not
shadow one another.

`Alt+[` / `Alt+]` was chosen over `Alt+Left` / `Alt+Right` because these
registrations are **global**: they are taken from every other application for
as long as ClipYomi runs. `Alt+Left` is Back in every browser and file manager
on the machine, and a reading tool that breaks Back system-wide is a worse
trade than a slightly less discoverable key. The bracket pair is adjacent,
one-handed, already the prev/next idiom in editors, and effectively unbound
globally. Because the shortcut is matched by physical key code, the two stay
adjacent on layouts that print something else there.

### 4.3 Two handler details that are bugs if missed

**Gate on `ShortcutState::Pressed`.** The handler is called for press *and*
release. Ungated, every hotkey performs its action twice — and for bind, the
second one is a no-op only because bind happens to be idempotent, which is
luck rather than design.

**Auto-repeat is deliberately not debounced.** Holding `Alt+3` repeats at the
OS rate. Restore is idempotent (it re-applies the same slot) and so is bind (it
re-captures the same state), so a held key is harmless. Recorded so that a
future reader does not add a debounce for a problem that does not exist.

### 4.4 A key another application already owns

`register` fails per shortcut, and this is the phase's only genuinely invisible
failure: a user whose `Alt+5` belongs to something else would see a feature
that simply does nothing, with no way to tell that from a bug in the feature.

The failed shortcut names are collected during `setup` into a managed
`ShortcutWarning(String)`, exposed by a `shortcut_warning` command, and
rendered by the frontend exactly as `showSettingsWarning` renders its own
(`main.ts:120`).

This is the house pattern rather than a new one: `startup_error` and
`settings_warning` are already one managed string plus one command plus one
frontend renderer each. Appending to `SettingsWarning` instead was considered
and rejected — that state means "the settings file could not be read", and a
sentence about hotkeys inside it makes the name lie.

---

## 5. The two paths

### 5.1 Bind — `Shift+Alt+N`

All Rust, no webview involvement, because the webview may be unfocused,
mid-render, or reloading when the key arrives.

- **Geometry from the live window:** `inner_size()` + `outer_position()` +
  `scale_factor()`, converted to logical and rounded. This is exactly the pair
  `save_window_geometry` stores (`main.ts:1104`), so a slot and `window_*` are
  in one coordinate system and neither needs converting to compare with the
  other.
- **Everything else from `SettingsState`:** the resolved `chrome_mode`,
  `always_on_top`, `surface_alpha`, the three sizes, `furigana_mode`.

**The stored alpha, structurally.** 4B handoff §3 warns 4D that a snapshot
reading the live `--surface-alpha` while chrome is `native` would capture `1`
and silently overwrite the user's chosen value. Reading `SettingsState` closes
that off by construction rather than by care: the backend has never had access
to the clamped value — `surfaceAlphaVar` computes it in the frontend, and the
only thing that crosses back is the `opaque` boolean on
`set_window_background`.

Two corrections the live read needs:

**Peek in flight, macOS only.** `peek_titlebar` grows the frame by
`BAND_HEIGHT` logical and shifts `y` by the negative of it
(`commands.rs:999`); `peek_grows_frame()` is `cfg!(macos)`. A bind during a
peek would store the grown box, so every restore of that slot would place the
window 28 px too tall and 28 px too high. `PEEKED` already exists
(`commands.rs:940`), so the correction is a pure function:

```rust
pub fn unpeek(box_: LogicalBox, peeked: bool, grows_frame: bool) -> LogicalBox
```

This is the same hazard `scheduleGeometrySave` guards with `if (peeking)
return` (`main.ts:1099`), reached by a different route: the frontend can skip
the save and wait for the next resize, but a keystroke has to be answered now.

**Windows' reserved row stays in.** The band row is part of the window's real
box and is persisted that way. `initial_height`'s doc comment
(`commands.rs:1099`) states the invariant: the only place a band is *added* is
first launch, and anything that adds one on a save-or-restore path breaks the
fixed point. Bind adds nothing and restore subtracts nothing.

**What bind captures is the persisted state, which lags an in-flight drag by
one commit.** The alpha slider broadcasts on `input` and saves on `change`
(`settings.ts:288`, `:306`), so a bind pressed mid-drag records the last
committed value. Accepted and recorded rather than fixed: nobody presses
`Shift+Alt+3` in the middle of dragging a slider, and closing the gap would
mean the webview participating in capture, which this section opens by
rejecting: the key can arrive while the webview is unfocused, mid-render or
reloading, and it would also hand the *clamped* alpha back as the stored one.

### 5.2 Restore — `Alt+N`

An absent slot is a **silent no-op**. `Alt+7` on a fresh install does nothing
and says nothing; there is no error state in pressing a key you have not bound
yet.

Otherwise, in order:

1. Resolve `chrome_mode` through `supported()`. A slot bound on macOS in
   `native` restores as `header` on Windows — the existing mechanic, reached
   from a new caller, with no `cfg!` of its own.
2. **Merge the slot into `SettingsState` by assignment, not by merge.** A slot
   that recorded "fonts unset" must clear them; a merge could never express
   that. This mirrors `save_settings`'s own split, where the sizes are assigned
   and the toggles merged, and for the same stated reason.
3. Apply the window-level half in Rust: the fitted geometry (§6),
   `set_always_on_top`, and the chrome mode.
4. Emit one bare `settings-changed` carrying the full `Settings`.

**Chrome application shares `set_chrome_mode`'s body.** The per-platform half
of `set_chrome_mode` (`commands.rs:901`) is extracted as
`apply_chrome(&window, mode)` and called by both. Nothing is duplicated, and
the macOS/Windows/other split stays in one place.

**Ordering inherits 4A's open finding.** Persist-then-apply matches
`set_chrome_mode`, and carries the same weakness 4A handoff §9 recorded: a
failure in the apply half leaves a persisted value the window was never updated
to match. Not fixed here — fixing it means reordering `set_chrome_mode` too,
which is a separate change with its own risk — but named, because restore adds
a second caller to the pattern.

### 5.3 One emit does the token-level half

The emit is what keeps this phase small, and it works because of attributes
already on `Settings`:

- Every optional key carries `skip_serializing_if = "Option::is_none"`, so a
  slot that restored "fonts unset" produces a payload with those keys
  **absent** — which is precisely what `applyFontSizes` reads as "clear it"
  (`main.ts:564`).
- `always_on_top` and `clipboard_monitoring` are the two non-optional keys, so
  the payload satisfies 4B's `isFullSnapshot` discriminator (`main.ts:639`) and
  the font sizes are allowed through. That discriminator exists because of 4B
  ruling 1, and this is the first path other than the settings window to depend
  on being on the right side of it.
- `settings.ts:336` picks the same event up and re-syncs its own controls, so
  the settings window is correct after a restore even if it was open at the
  time.

No new applier is written on either side. The payload also now carries
`layout_slots`; the TypeScript `Settings` type does not declare it and nothing
in the frontend reads it, so it rides along ignored.

**One frontend gap this exposes.** `settings.ts`'s `applySettings` sets a font
box only `if (s.font_sentence !== undefined)`, so a restore-to-unset leaves the
box showing a stale number while the main window has already cleared the token.
The fix is the same discriminator `main.ts` uses: on a full snapshot, assign
the boxes including clearing them; on a partial payload, leave them alone. It
cannot be an unconditional clear — a partial `{ surface_alpha }` from a slider
drag carries no font keys, and clearing on that would reproduce 4B ruling 1's
bug on the settings surface instead of the main one.

### 5.4 History — `Alt+[` / `Alt+]`

History is session-only and lives in the webview (4C handoff §4), so this pair
is the one action that has to leave Rust. The handler emits `history-step` to
the main window with `-1` or `1`, and the listener is the same one-liner the
chevrons already are (`main.ts:1191`):

```ts
void listen<number>('history-step', (e) =>
  display(e.payload < 0 ? history.back() : history.forward()),
);
```

`back()` and `forward()` return `null` at either end and `display(null)` is a
no-op, so neither end needs a guard — the same property the click handlers rely
on. `display` calls `syncHistoryButtons`, so the chevrons' disabled state
follows a keyboard step for free.

---

## 6. Keeping a restored window on screen

A slot stores a box that was valid when it was bound. Monitor layouts change:
a laptop undocks, a second screen goes away, DPI changes. Restoring verbatim
would put the window somewhere unreachable — and every key that could fix it is
attached to that window. That is the trap 4A deviation #12 refused for content
mode ("a mode that cannot be left without editing `settings.json`"), reached by
a different route.

So restore goes through one pure function:

```rust
pub fn fit(slot: LogicalBox, monitors: &[MonitorRect], primary: usize, scale_hint: f64) -> PhysRect
```

1. Probe: the slot's logical box converted to physical with `scale_hint`, the
   current window's scale factor.
2. Target: the monitor whose **work area** overlaps the probe most. None
   overlaps → `primary`.
3. Re-derive the physical size from the logical size using the *target's*
   scale, so a slot bound on a 100 % monitor occupies the same apparent size on
   a 150 % one.
4. Clamp the size into the work area, floored at the window's configured
   160×80 minimum.
5. Shift the origin until the box is inside.

`MonitorRect` is a plain struct (work area position and size, plus scale), not
`tauri::Monitor`, so every case is testable with fabricated numbers and no
window. `Monitor::work_area()` exists in tauri 2.11.5 and is physical.

**Physical is the coordinate space because it is the only unambiguous one.**
Under mixed DPI there is no consistent logical plane across monitors — each has
its own scale — so the union only makes sense in device pixels. This is the
same reasoning `placeFor` records for testing points against `position`/`size`
rather than calling `monitorFromPoint`, whose units differ per platform
(`main.ts:905`).

**The ceiling, commented at the site:** step 2 converts with `scale_hint`, the
*current* window's scale, so on a mixed-DPI setup the monitor it picks can be
the neighbour of the one the slot was bound on. The consequence is a window
that lands fully on-screen on the wrong monitor — not a window that lands
off-screen, which is the entire purpose of the function. A second pass that
re-probed with the target's scale would narrow this; it is not worth the code
until someone reports it.

An empty monitor list returns the probe unchanged. There is nothing to clamp
against, and refusing to move the window would make the slot silently useless.

---

## 7. Task order

### 7.1 Task 0 — verify the plugin before writing anything

The crate is neither in `Cargo.lock` nor cached locally. Confirm, in this
order: the license is MIT OR Apache-2.0; the builder/handler API and the
`ShortcutState` enum are as §4 assumes; a shortcut string parses from
`"Alt+Digit1"`-style names; and that macOS needs no accessibility permission
for it (the plugin uses Carbon hot keys, which do not, but this phase should
not ship a claim it did not check).

If any of those four is different, the affected section of this document is
wrong and gets corrected before implementation continues rather than worked
around.

### 7.2 The rest

1. `LayoutSlot` + `Settings.layout_slots` + their persistence tests, including
   the `save_settings`-leaves-it-alone test (§3.3).
2. `layout.rs` with `fit` and `unpeek` and their tests. Pure, no window.
3. `apply_slot` — the assignment merge — and its tests.
4. Bind, wired to nothing yet.
5. Restore, including extracting `apply_chrome` out of `set_chrome_mode`.
6. The plugin, the twenty registrations, and `ShortcutWarning`.
7. `history-step` on both sides.
8. The `settings.ts` font-box gap (§5.3).
9. Manual checks (§8.4).

Steps 1–3 are the parts that carry the phase's real logic and they are all
testable without a window, which is why they come first.

---

## 8. Verification

### 8.1 Rust unit

`fit`: exact fit; a probe overlapping nothing falls to primary; a box larger
than the work area shrinks and floors at 160×80; a monitor at a negative origin
(left of or above primary); the mixed-DPI size re-derivation of step 3; an
empty monitor list returns the probe unchanged.

`unpeek`: peeked-and-grows removes the band and restores `y`; peeked-without-
growth passes through; not-peeked passes through.

`apply_slot`: assigns rather than merges; clears unset fonts; resolves the mode
through `supported()`.

Persistence: a slot round-trips through the file; unset keys are absent from the
JSON; a garbage slot key is preserved and ignored **without** resetting the file
(§3.2); `save_settings` leaves `layout_slots` untouched (§3.3); an out-of-range
slot index is rejected.

### 8.2 Vitest

The `history-step` listener steps in both directions, and does nothing at
either end of the timeline.

`settings.ts` clears its font boxes on a full snapshot whose font keys are
absent, and leaves them alone on a partial payload — the second surface of 4B
ruling 1's bug class (§5.3).

`main.ts`: one restore emit moves alpha, the fonts, the furigana mode and the
chrome mode together.

### 8.3 Playwright — nothing, and this is stated rather than papered over

Chromium cannot fire a global shortcut and has no OS window to move or
reposition. A Playwright test here would exercise the harness and prove
nothing about the feature. 4B §5 is the precedent for saying so plainly
instead of adding a test that reads as coverage.

Existing baselines must pass **unregenerated**: this phase changes no default
rendering.

### 8.4 Manual, and the list ships whether or not it is run

1. Bind content mode + 35 % alpha + large fonts to slot 1, header-mode defaults
   to slot 2, alternate — all six captured things move each way.
2. A slot bound with the font boxes empty restores to empty.
3. Bind on a second monitor, unplug it, restore — the window lands on the
   remaining screen.
4. Hold `Alt+3` — the repeats are harmless.
5. Run with another application owning `Alt+5` — the startup warning names it.
6. macOS/Linux only: restore a 40 % slot while chrome is `native` — the screen
   clamps to 100 %, and `settings.json` still reads 40. This is 4B handoff §3's
   requirement, observed rather than reasoned.
7. macOS only: bind during a peek — the stored height carries no extra band.

Whichever of these do not run are reported as not run, by name, in the
handoff. 4B §5 is the standard: an unrun check that goes unmentioned reads to
the next phase as a check that passed.

### 8.5 Gate

Vitest, `npm run build`, Playwright with no regenerated baselines, `cargo
test`, and `cargo clippy --all-targets` holding at the four pre-existing
`upper_case_acronyms` warnings with no new debt.

---

## 9. Deferred

- **A transient "Bound 3" indicator.** Bind is silent. Restore is its own
  verification — press `Alt+N` and the window snaps — and the one failure that
  would otherwise be invisible, a key another application owns, is surfaced at
  startup (§4.4). New transient UI, its CSS and its tests are not worth that
  gap.
- **User-editable key bindings.** The twenty are fixed. A rebinding surface
  needs persistence, conflict detection and a settings UI; nothing asks for it
  yet.
- **A slot list or clear affordance.** Slots are visible in `settings.json`
  and overwritable by rebinding. Removing one means editing the file, which is
  acceptable for now.
- **Disk-persisted history in a slot.** Permanently out, per 4C handoff §4.
- **The mixed-DPI second pass in `fit`.** §6's ceiling.
- **`set_chrome_mode`'s persist-before-apply ordering.** 4A handoff §9's open
  finding, which restore now shares (§5.2).

---

## 10. Deviations to record in the master spec at phase close

§12 currently ends at 12.

| #  | Deviation | Why |
| -- | --- | --- |
| 13 | Global history hotkeys `Alt+[` / `Alt+]` added beyond §7.9's scope | 4A §9 named this phase's keyboard surface as their home; content-only mode has had no route to back/forward since 4A. `Alt+Left`/`Right` was rejected because a global grab would take Back from every browser and file manager on the machine. §4.2 |
| 14 | Restore clamps geometry into a visible monitor instead of restoring it verbatim | A slot bound on a monitor that is now absent would otherwise place the window somewhere unreachable, and every key that could fix it is on that window — 4A deviation #12's trap by another route. §6 |

§8 already lists "layout slots 1–9" among the persisted settings, so no change
is needed there. §11's 4D row moves to past tense and records the two
deviations above.
