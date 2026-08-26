# JParser Phase 4A — Chrome Modes Design

**Date:** 2026-08-25
**Status:** Approved, not yet implemented.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.2, §7.4, §11)

---

## 1. Goal

§7.4 names three window chrome states, ported from ta-old's three (`MakeWindow`,
`TranslationAggregator.cpp:542`). 4A adds the missing one — **content only** —
replaces today's boolean with the three-way control §11 asks for, and settles
what the third state means on Windows, where it currently does not exist.

Content only is the mode the app is for: maximum real estate over a game, with
nothing of ours drawn above the sentence. It is also the mode 4B has to clamp
against, which is why §11 puts 4A ahead of it in the dependency chain.

---

## 2. Scope

A `chrome_mode` setting with three values, the content-only mode itself, a
right-click menu as the reach that mode removes, a three-way control in the
settings window in place of today's checkbox, and the removal of the
Windows-only dead ends the boolean left behind.

**Hard constraint:** the hidden-title-bar mode and its hover reveal are out of
scope and must come out of this phase behaving exactly as they do now. §4.5 is
the invariance argument and the gate that proves it.

Out of scope, and why:

| Not doing | Reason |
| --- | --- |
| A real OS caption on Windows | §4. The port is permanently frameless there by `d07711f` and `e45d55f`; restoring `WS_CAPTION` is its own sub-phase, not a rider on this one |
| Per-pixel background alpha | 4B. 4A only *defines* the state 4B clamps: native frame forces alpha to 100 % |
| Layout hotkeys | 4D. A slot captures chrome mode among other state, so it must come after this |
| A tray icon | Not in §7 at all. Quit in the right-click menu is the reach content mode needs |
| Native OS popup menu | §6.4. Untestable from Vitest and Playwright, and the in-webview menu is enough at the sizes this window is used at |
| Removing `WS_THICKFRAME` in content mode | ta-old's `WS_POPUP` had no resize. Losing drag *and* resize makes the mode a trap; see §3.3 |

---

## 3. The three states

`ChromeMode` = `native` | `header` | `content`.

| Mode | Band | Peek | OS chrome | Windows |
| --- | --- | --- | --- | --- |
| `native` | reserved 28px grid row, gear always visible, divider visible | n/a | OS title bar and buttons | **not offered** — see §4 |
| `header` | out of the grid, 6px idle overlay → 28px while `.peeked` | yes | hidden | reserved row, region-clipped |
| `content` | 6px idle overlay, **never peeks**, children hidden | no | hidden | identical to `header` |

On macOS the mapping onto what ships is exact: today's `decorations: true` **is**
`native`, `decorations: false` **is** `header`, and neither changes behaviour in
4A. Only the name it is stored under does.

### 3.1 Why `content` is `header` with the reveal switched off

This is the finding that makes the new mode small. Everything content mode needs
on Windows already exists for header mode:

- the reserved row above the content, clipped away by the region
  (`region_top_inset`, `apply_round_region`);
- `HTCAPTION` over the first 28 logical px of the *visible* top, which is what
  keeps a frameless window draggable (`WM_NCHITTEST`, measuring from
  `visual_top`);
- the eight resize edges, DOM-side, plus `WS_THICKFRAME` and the hit-test arms
  behind them.

So content mode is header mode that never calls `peek_titlebar`. **No new Win32
code, and no geometry difference between the two modes** — which also means the
initial height treats them identically and switching between them moves nothing
on screen.

macOS reaches the same place from its own direction: `set_titlebar_chrome(false)`
already hides the title text and the four standard buttons, and the 6px
`.controls` sliver already carries `data-tauri-drag-region`. Content mode keeps
both and simply never grows the frame.

### 3.2 Hiding the band's children: `visibility`, not `opacity`

Header mode idles its controls at `opacity: 0; pointer-events: none` and fades
them in with `.peeked`. Content mode cannot reuse that: an `opacity: 0` button
is still a tab stop, and the band never reveals, so tabbing would move focus
onto controls that can never become visible. That is exactly the WCAG 2.4.7 trap
4C had to fix for the history nav (`4c-design` §7.1) — the difference being that
there the fix was *to reveal*, and here there is nothing to reveal.

So content mode hides the three children — `.history-controls`,
`#settings-toggle`, `.window-controls` — with `visibility: hidden`, which removes
them from the tab order and from hit-testing in one property. `.controls` itself
stays visible-but-transparent so it remains the drag region.

### 3.3 What content mode takes away, and what replaces it

It removes the gear, the history chevrons, minimise and close, and (on macOS)
the OS title bar. Everything reachable from the band becomes unreachable, which
is why the right-click menu (§6.4) is load-bearing in this phase rather than
polish.

Dragging and resizing both survive untouched, per §3.1 — the top strip drags and
the edges resize whether or not anything is painted there. A deliberate
departure from ta-old, whose content-only mode was a `WS_POPUP` with neither: a
mode that can be entered but not moved, resized, or left without editing
`settings.json` is a trap, and keeping these costs nothing.

Worth stating plainly because the window does not show it: **the top 28 logical
px of visible content is a drag region on Windows, and the top 6px is one on
macOS, in both non-native modes.** Already true today in header mode.

---

## 4. Windows offers two modes, not three

### 4.1 What is actually there today

`native` does not exist on Windows, and `decorations: true` is a broken state:

- `apply_dwm_attributes` (`commands.rs:533`) writes `frameless_style(style)` on
  every call, clearing `WS_CAPTION`; `init_window` calls it, and
  `set_decorations` calls `init_window` on every toggle.
- the `WM_STYLECHANGING` arm strips `WS_CAPTION` from any style tao tries to
  set, for the window's whole life.
- `set_decorations`'s Windows arm never calls `window.set_decorations(enabled)` —
  that line lives in the `not(macos), not(windows)` branch.
- so `region_top_inset`'s `WS_CAPTION` check can never see a decorated window,
  and the top 28px is always clipped.
- `global.css` reserves `padding-top: var(--band-h)` on Windows in **both**
  modes ("BOTH modes reserve the top 28px row"), and the `opacity: 0` idle rules
  are qualified to `[data-decorations='false']` only.

Together: `decorations: true` on Windows shrinks the window 28px via
`grow_top(-band)`, reserves and paints the band row with its controls visible,
then clips that row out of the region. The user gets an invisible header, a
see-through top strip, and no reachable gear. `initial_height(None, true)` also
assumes no reserved row while the CSS reserves one unconditionally.

This is residue, not a fresh regression. `d07711f` is *"make in-app band the
permanent title bar on Windows"* and `e45d55f` dropped `WS_CAPTION` on purpose
to stop DWM drawing a shadow. The Windows decision was one chrome shape; the
`true` position of the boolean was left behind.

### 4.2 The resolution

`ChromeMode` keeps all three variants — the setting is cross-platform and a file
must not be invalid on the platform that reads it — but **Windows resolves
`native` to `header`** and never offers it in a control.

Rejected: making the OS caption real on Windows. It means mode-aware
`frameless_style` and `apply_dwm_attributes`, an early `DefSubclassProc` return
from three window-proc arms, `SetWindowRgn(hwnd, null)`, restoring the DWM
rounding and shadow `e45d55f` spent a commit removing, and getting the
transition geometry right in a file whose comments are largely scars from that
class of bug. It is a sub-phase, not a rider.

Also rejected: keeping three modes on Windows with `native` redefined as "our
band, always visible, never clipped". It is about thirty lines and would give
three working, distinct shapes — but a control labelled *Native frame* that
produces no OS title bar, no OS buttons and no OS shadow misrepresents itself,
and the master spec's three-state mapping stops being a mapping.

Recorded as a deviation from §7.4: **ta-old's `WS_OVERLAPPEDWINDOW` state has no
Windows equivalent in this port.** ta-old was Windows-only, so this is the one
deviation where the port is narrower than the original on the original's own
platform. The mode exists on macOS and Linux, where Tauri's own
`set_decorations` handles it.

### 4.3 Resolution is one pure function

In `settings.rs`, beside the enum:

```rust
/// A mode the running platform can actually present.
pub fn supported(mode: ChromeMode, native_supported: bool) -> ChromeMode {
    if mode == ChromeMode::Native && !native_supported { ChromeMode::Header } else { mode }
}
```

Called once from `load`, with `native_supported = !cfg!(target_os = "windows")`.
Pure and boolean-parameterised rather than `cfg`-gated inside, so both arms are
testable on either platform — the same reason `peek_grows_frame` is a function
and not a `cfg!` at each use site.

Consequence worth stating: **every existing Windows install lands in `header`.**
`decorations: true` migrates to `native`, which resolves to `header`;
`decorations: false` migrates straight to `header`. So on Windows the
user-visible effect of 4A is that the broken state disappears and `content`
appears.

### 4.4 What this deletes

With no native boundary on Windows, four things stop having a live caller and go
rather than sit there drifting:

| Removed | Because |
| --- | --- |
| `region_top_inset`'s `decorated` read (`GWL_STYLE` / `WS_CAPTION`) | Always false by construction. Becomes `if STRIP_SHOWN { 0 } else { strip_px(dpi) }` |
| the `grow_top(±band)` call in the chrome command | No native boundary exists on Windows to cross |
| `grow_top` and `top_grown` themselves, and their three tests | That call was the only caller — the peek moved to `set_strip_shown` in `1671a77`. Tests of a deleted function guard nothing; the drift they caught cannot recur once the arithmetic is gone |
| `initial_height`'s mode parameter | Windows always reserves the row, macOS never does. It collapses to `saved.unwrap_or(DEFAULT_WINDOW_HEIGHT + if cfg!(windows) { BAND_HEIGHT_LOGICAL } else { 0 })` |

`docs/superpowers/2026-08-18-windows-titlebar-test-plan.md` check 1 ("the gear
will sit below the Windows caption") becomes moot for the same reason. Left as
the historical record it is.

### 4.5 Header mode is invariant

The hidden-title-bar mode and its hover reveal are **not in this phase's scope
and must come out of it unchanged**. That holds by construction rather than by
care, and each of the deletions above is safe for a specific reason:

| Change | Why header mode cannot notice |
| --- | --- |
| `showPeek`'s gate becomes `chromeMode !== 'header'` | Logically identical to the old `titlebarShown` gate for this mode — only the *other* blocking side gains a member |
| `grow_top` and its call deleted | Not on the peek path. `1671a77` moved the reveal to `set_strip_shown`; `grow_top`'s only caller was the native toggle |
| `region_top_inset`'s `decorated` read deleted | It already returns `strip_px` unconditionally on Windows, because `WS_CAPTION` is never set (§4.1). No returned value changes |
| `initial_height` loses its mode parameter | Header mode on Windows already gets `DEFAULT_WINDOW_HEIGHT + BAND_HEIGHT_LOGICAL`; the collapsed form returns the same number |
| `data-decorations` → `data-chrome` | `header` and `content` share every band rule, so header's computed styles are identical. A mechanical swap, verifiable with `git grep data-decorations` returning nothing |
| Task 0's probe removal | `probe_note_paint` only pushed a tick and fell through; the `WM_PAINT` arm did the same. `WM_ERASEBKGND` keeps its fill. `probe_autotoggle` ran only under `CLIPYOMI_PROBE` |

Untouched entirely: `peek_titlebar`, `set_strip_shown`, `apply_round_region`,
`WM_NCHITTEST`, `WM_NCCALCSIZE`, `apply_decorations_macos`,
`set_titlebar_chrome`, `PEEK_HIDE_MS`, `BAND_HEIGHT`, `--band-h`,
`peek_grows_frame`, and the geometry debounce that skips a resize while peeking.

**The acceptance gate is the existing suite, not a new one.** `e2e/panes.spec.ts`
already carries both halves of this mode: `HIDDEN_TITLEBAR_STUB` (macOS — the
6px idle sliver, gear and history nav at `opacity: 0`, the sentence not moving
when the band reveals) and `WINDOWS_STUB` (the reserved row, nothing moving on
peek). Those tests must pass **with zero screenshot baseline updates**, their
stubs changing only the settings key: `decorations: false` → `chrome_mode:
'header'`. A baseline that needs regenerating is a regression in this mode, not a
rebase — treat it as a failure and find out why.

`e2e/stub.ts` keeps answering `native` (today's `decorations: true`) and
`platform: 'macos'`, so every other committed baseline stays valid too. The stub
is a CSS harness, not a platform simulator; it deliberately reports a state real
Windows no longer reaches, and the new `e2e/chrome.spec.ts` is where per-platform
mode combinations get their own baselines.

One real, disclosed consequence, on Windows only: a settings file with
`decorations: true` was written while `grow_top(-band)` had shrunk the window by
28px, and `save_window_geometry` persisted that smaller box. Migrating to
`header` restores the saved box verbatim, so the content area is 28px shorter
until the window is next resized. One-time and cosmetic. No migration fixup: the
bundle is inactive at 0.1.0, so the affected population is the developer, and a
height-adjusting special case in `load` would re-apply on any launch that
crashed before the first save.

---

## 5. Persistence and migration

```rust
#[derive(..., Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChromeMode { #[default] Native, Header, Content }

// in Settings
#[serde(default, skip_serializing_if = "Option::is_none")]
pub chrome_mode: Option<ChromeMode>,
/// Legacy, pre-4A. Read once by `load` to seed `chrome_mode`, then cleared.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub decorations: Option<bool>,
```

`decorations` changes from `bool` with `default_true` to `Option<bool>` with
`skip_serializing_if`, which is what makes the migration self-completing: the
key stops being written the first time anything saves.

**`load()` normalizes, once.** It sets `chrome_mode` to `Some(_)` on every path,
runs it through `supported` (§4.3), and clears `decorations` — so nothing
downstream of `load` sees the legacy key, an absent mode, or a mode this
platform cannot present:

| File contains | Mode, macOS / Linux | Mode, Windows |
| --- | --- | --- |
| `chrome_mode: "content"` | `content` | `content` |
| `chrome_mode: "native"` | `native` | `header` |
| `decorations: false`, no `chrome_mode` | `header` | `header` |
| `decorations: true`, no `chrome_mode` | `native` | `header` |
| neither | `native` (§5.1) | `header` |
| unparseable file | `native`, plus the existing warning | `header`, same warning |

A present `chrome_mode` wins over a present `decorations`.

Normalizing in `load` rather than behind an accessor is the same call
`initial_height`'s existing comment makes for geometry: one place converts, and
nothing else has to remember to.

### 5.1 The default is `native`, deviating from §7.4

§7.4's table marks "Header + resize edges" as *(default)*, but the shipped
`Settings::default()` has `decorations: true`, which is `native`. 4A resolves
the conflict in favour of what ships.

Existing users are unaffected either way — every settings file written since 2K
carries an explicit `decorations`, so they migrate to a named mode rather than to
a default. Only a fresh install is decided here, and on macOS a first-run window
with no OS title bar is a worse introduction than one with: nothing on screen
would explain how to move, resize, or close it. On Windows §4.3 resolves it to
`header`, which is the working shape there.

---

## 6. Architecture

```
settings window  ──┐
                   ├── invoke set_chrome_mode(mode) ──► commands.rs
right-click menu ──┘                                      │
                                                          ├─ macOS: apply_decorations_macos(mode == Native)
                   emit settings-changed({chrome_mode})   ├─ Windows: init_window
                            │                             └─ settings.update(chrome_mode)
                            ▼
              both webviews' listeners
              main.ts:     applyChromeMode  → #app[data-chrome]
              settings.ts: applySettings    → segmented control
```

### 6.1 Backend

`set_decorations(enabled)` becomes `set_chrome_mode(mode)`. On macOS it is the
old body with its boolean derived — `apply_decorations_macos(&window, mode ==
ChromeMode::Native)`. On Windows the `grow_top` call goes (§4.4) and only
`init_window` remains. Same `chrome_target(&app)` resolution by label (§7.4 —
never the injected `tauri::Window`, or the settings window undecorates itself).

The command runs `supported` over its argument before applying or persisting it,
so an out-of-date webview cannot write a mode this platform will not honour.
`chrome_mode` is persisted by `set_chrome_mode` itself, exactly as
`always_on_top` and `clipboard_monitoring` are persisted by their own dedicated
commands — neither of those has a `save_settings` merge arm either, and
`chrome_mode` does not gain one. A merge arm would let the settings window's
snapshot overwrite the value `set_chrome_mode` had just resolved through
`supported`, which on Windows would turn a transient in-session mismatch into
a persisted wrong value. `main.rs`'s startup restore and its handler list
follow the rename.

Nothing else in `win32_frameless` changes.

### 6.2 Frontend state

`#app[data-decorations]` becomes `#app[data-chrome]`, carrying the mode name.
`applyDecorations(shown)` becomes `applyChromeMode(mode)`, and the module-level
`titlebarShown: boolean` becomes `chromeMode: ChromeMode`.

`showPeek` gains one gate: it returns early unless `chromeMode === 'header'`.
Previously it returned early when `titlebarShown` — the same rule, now with two
modes on the blocking side instead of one. `applyChromeMode` cancels an
in-flight peek whenever the new mode is not `header`, for the reason the old code
cancelled it on `shown`: the `pointerleave` that follows must not undo chrome the
user just switched to.

CSS: every current `[data-decorations='false']` rule becomes
`[data-chrome='header'], [data-chrome='content']` — the two modes share the
band's geometry exactly. Content mode adds one rule of its own, the
`visibility: hidden` of §3.2. `.peeked` and `.peek-offset` need no mode
qualifier, because content mode never receives those classes.

### 6.3 Settings window

The "Title bar & window borders" checkbox becomes a `.segmented-control` — the
same markup, CSS and `aria-checked` pattern the furigana picker already uses, so
no new stylesheet work. It invokes `platform()` once at init and omits the
`native` button on Windows. `Settings.decorations` leaves `src/types.ts`;
`chrome_mode: ChromeMode` replaces it.

### 6.4 The right-click menu

A new module, `src/context-menu.ts`, plus rules in `global.css`.

```
● Native frame            → invoke('set_chrome_mode', { mode }) + emit('settings-changed', { chrome_mode: mode })
○ Header + resize edges      (the native item is absent on Windows)
○ Content only
─────────────────────
  Settings…              → invoke('open_settings_window')
  Quit                   → invoke('close_window')
```

Every command it calls already exists. The `emit` keeps the settings window's
control in step, and a partial payload is safe: `settings.ts`'s `applySettings`
guards every field on `!== undefined`. `main.ts` already resolves
`currentPlatform` at startup, so the Windows omission needs no new call.

**Available in all three modes**, not gated to `content`. One code path is less
code than a conditional, and a two-click mode switch is useful in every mode.

**The listener goes on `document`, not `#app`.** A keyboard-invoked context menu
(Shift+F10, or the Menu key) targets the focused element, which in content mode
is `<body>` — an *ancestor* of `#app`, so the event would never reach a listener
bound there. Content mode has no tab stops at all by §3.2, so `document` is the
only binding that keeps the menu reachable without a pointer. Chromium reports
`clientX/clientY` of the focused element (or `0,0`) for those invocations, which
the clamp below handles as an ordinary near-origin position.

`preventDefault()` on the same handler suppresses the WebView2 default menu.

**Placement is a pure function**, `clampMenu(x, y, w, h, vw, vh)`, mirroring how
`render/popover.ts` keeps `placePopover` pure and unit-tested rather than
measuring inside a DOM callback.

Dismissal: `Escape`, a `pointerdown` outside the menu, or the window losing
focus. Keyboard: `ArrowUp`/`ArrowDown` move, `Enter` activates, focus moves to
the first item on open. The mode items are a `role="radio"` group with
`aria-checked`, matching the furigana picker.

`ponytail:` an in-webview menu cannot escape the window, so at the configured
`minWidth: 160 / minHeight: 80` it is cramped. Native `Menu::popup` if that ever
matters — the cost is that an OS surface is invisible to both test suites.

---

## 7. Task 0 — revert the Windows flash spike

`commands.rs`, `xtask` and `tools/` carry roughly 200 lines marked
`THROWAWAY PROBE (spike, revert me)`. Both routes it measured were refuted, and
its injected JS is keyed off `data-decorations`, which §6.2 renames — so it
breaks in this phase whatever else happens. It goes first, in its own commit, so
the 4A diff is readable.

Removed: `PROBE_FLUSH`, `PROBE_PAINTS`, `PROBE_ACTIVE`, `probe_clip`,
`probe_wait_compose`, `probe_window_rect`, `probe_qpc`, `probe_log_toggle`,
`probe_log_toggle_at`, `probe_note_paint`, `probe_autotoggle`, the
`probe_region_clip` command and its handler entry, the `WM_PAINT` arm and its
constant, the `DwmFlush` extern, the probe block in `peek_titlebar`, the
`probe_autotoggle` call in `main.rs`, `xtask/src/probe.rs` with its arms in
`xtask/src/main.rs`, and `tools/probe-strip.mjs`, `tools/probe-appearance.mjs`,
`tools/probe-shot.ps1`.

Kept: `update_clip_region` (a real command, called from `main.ts` on a scale
change), `STRIP_SHOWN`, and the measurement in its doc comment — 39 % of reveals
flashed the desktop when the frame grew, 1 % when the row is reserved and only
the region changes. That number justifies the shipped design and outlives the
probe.

The two refuted results, recorded here so the deletion does not lose them:

- **`DwmFlush` on the main thread before the region change.** 300 reveals each,
  matched duration: without it 8/300 flashed, median wait 3.6 ms, worst 7.9 ms;
  with it 89/300 flashed, median 12.4 ms, worst 61.3 ms, and 84 of those spanned
  two sample intervals. It also dragged the sampler from 144 Hz to 80 Hz —
  blocking the message pump delays the webview's own presents.
- **The same wait moved off-thread**, which removes that objection. Matched
  300 s runs, sentinel band, pinned geometry, 144 Hz: without it 3/298 flashed
  and 1 stalled; with it 9/286 flashed and 12 stalled, median wait 3.8 ms →
  6.9 ms. Three times worse. `DwmFlush` returns when DWM *finishes* a frame, so
  changing the region right then lands just after it latched.

Together those rule out compose alignment as an approach, not merely as one
implementation of it. The residual flash stays a known, accepted glitch,
documented where it happens.

---

## 8. Verification

**Rust — `settings.rs`, `commands.rs`**

- `supported` as a unit table: `native` with and without support, `header` and
  `content` untouched in both;
- the migration matrix of §5, both platform columns, driven through `supported`
  rather than through `cfg`;
- the legacy `decorations` key is absent from the file after a load-and-save;
- `chrome_mode` round-trips through the file;
- an unknown `chrome_mode` string fails to parse, which `load` already reports
  as a warning and falls back to defaults — pinned so the fallback is deliberate
  rather than incidental;
- the initial height reserves the row on Windows and not elsewhere, with saved
  values passing through untouched on both;
- `set_chrome_mode` persists the resolved mode, not the requested one, and
  `chrome_target` still resolves `"main"` with a settings window present.

**Vitest — `main.test.ts`, `context-menu.test.ts`, `settings.test.ts`**

- `#app[data-chrome]` mirrors the setting at startup and on `settings-changed`;
- `showPeek` is a no-op in `content` and in `native`, and works in `header`;
- switching away from `header` mid-peek cancels the peek and drops the classes;
- the band's three children are not tab stops in `content`;
- `contextmenu` on `document` opens the menu and calls `preventDefault`;
- the menu omits the `native` item when `platform()` reports `windows`, and
  offers it otherwise;
- the radio group reflects the current mode, and selecting one invokes
  `set_chrome_mode` and emits `settings-changed`;
- Escape, outside `pointerdown`, and window blur each close it;
- `clampMenu` as a unit table: fits, overflows right, overflows bottom,
  overflows both, and a menu larger than the viewport;
- the settings window's segmented control reflects and writes the mode, and
  drops its `native` button on Windows.

**Playwright — `e2e/panes.spec.ts` first, as the regression gate (§4.5)**

- the whole existing suite green with **no baseline regenerated**, both existing
  hidden-title-bar tests included, their stubs changed only from
  `decorations: false` to `chrome_mode: 'header'`;
- `git grep data-decorations` returns nothing.

**Playwright — `e2e/chrome.spec.ts`**

- each offered mode at 320 / 768 / 1024, both themes: band shape, gear
  visibility, divider;
- the menu opens and lies entirely within the viewport in `content` mode,
  including from a right-click in the bottom-right corner;
- `panes.spec.ts`'s comment referencing the old attribute is updated.

Manual, per §10's three unautomatable items: every offered mode transition on
each platform, confirming the window box does not drift, and that drag and
resize work in `content`.

---

## 9. Deferred

- **A real OS caption on Windows.** §4.2. Its own sub-phase, and the thing that
  would let the deviation below be retired.
- **Native popup menu.** §6.4's ceiling. Only worth it if the window is
  routinely used near its 160×80 minimum.
- **History navigation in the menu.** 4D's `Alt+1..9` surface is the right home
  for keyboard history, and duplicating the chevrons here would leave two
  controls to drift. Content mode without history nav is a real gap for one
  phase.
- **Alpha clamping in `native`.** 4B, which owns `--surface-alpha`. Nothing to
  clamp until it exists.

## 10. Deviations to record in the master spec at phase close

| # | Deviation | Why |
| - | --- | --- |
| 10 | Native frame is macOS/Linux only | The Windows window is permanently frameless (`d07711f`, `e45d55f`); restoring `WS_CAPTION` re-opens the DWM shadow that commit removed. §4.2 |
| 11 | Default chrome mode is `native`, not `header` | §7.4 marks header as default; the shipped `Settings::default()` has always been the other one, and a first-run window with no title bar cannot explain itself. §5.1 |
| 12 | Content-only keeps drag and resize | ta-old's `WS_POPUP` had neither. A mode that cannot be left without editing `settings.json` is a trap, and keeping them costs nothing. §3.3 |
