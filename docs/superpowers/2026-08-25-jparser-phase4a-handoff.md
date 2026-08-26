# JParser Phase 4A — Handoff

**Date:** 2026-08-26
**Spec:** [`docs/superpowers/specs/2026-08-25-jparser-phase4a-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-25-jparser-phase4a-design.md)
**Commits:** `9330ff8..eee8320` on `develop` (nine commits, unpushed)

---

## 1. What Was Completed in Phase 4A

1. **`ChromeMode` replaces the `decorations` boolean** (`372c623`, `6504842`).
   Three values — `native` / `header` / `content` — persisted as
   `chrome_mode: Option<ChromeMode>` in `settings.rs`. The legacy field
   becomes `decorations: Option<bool>` with `skip_serializing_if`.

   **The migration is self-completing.** `normalize()` (`settings.rs:152`) runs
   once inside `load()`: it folds a present `decorations` into `chrome_mode`
   when no `chrome_mode` is present, resolves the result through `supported`
   (§4.3 of the design), and always sets `settings.decorations = None`
   afterward. Because the field is `skip_serializing_if = "Option::is_none"`,
   a `None` never round-trips back into the file — so the key is read at most
   once per install and is gone from the file after the very first save,
   with no separate migration pass or version marker needed.

2. **Content-only mode** (`ee0d6bf`) needed **no new Win32 code**. It is
   header mode with the hover reveal switched off — `showPeek`'s gate became
   `chromeMode !== 'header'` instead of a boolean check, and the band's
   children go `visibility: hidden` (not `opacity`, so they leave the tab
   order — §3.2). Everything content mode depends on already existed for
   header mode: the reserved band row, the region clip, the `HTCAPTION` drag
   strip, and the resize edges. Nothing in `win32_frameless.rs` changed for
   this state.

3. **Three-way settings control and right-click menu** (`678c3c9`, `672bb6a`,
   `e7742c3`, `4c7fed7`). The settings window's checkbox became a segmented
   control (reusing the furigana picker's markup/CSS/`aria-checked` pattern),
   omitting the `native` button on Windows. A new `src/context-menu.ts` adds
   a right-click / Shift+F10 menu, bound on `document` (not `#app`, since a
   keyboard-invoked menu in content mode targets `<body>`, an ancestor of
   `#app`) with a pure `clampMenu(x, y, w, h, vw, vh)` placement function.
   The toggles did **not** move into this menu — they had already moved to
   the settings window in 3B — so the menu carries the mode switch, Settings,
   and Quit instead.

4. **The Windows native-frame finding** (`9330ff8`, folded into `6504842`'s
   resolution). This is the single most important thing this phase leaves
   behind for later work:

   - `WS_CAPTION` is stripped for the entire life of the window on Windows,
     by two independent paths: `apply_dwm_attributes` calls
     `frameless_style(style)` unconditionally on every call (and
     `init_window` calls it), and the `WM_STYLECHANGING` window-proc arm
     strips `WS_CAPTION` from any style tao tries to set afterward.
   - Consequently **there is no OS-native-frame state to observe on
     Windows** — `region_top_inset`'s `WS_CAPTION` check could never see a
     decorated window even before this phase.
   - `native` therefore resolves to `header` on Windows through one pure
     function, `supported(mode, native_supported)` in `settings.rs`, called
     once from `load` with `native_supported = !cfg!(target_os = "windows")`.
     Windows never offers `native` as a choice in either control.
   - The **previously-shipped `decorations: true` state on Windows was
     already broken**, independent of this phase: the window shrank 28px via
     `grow_top(-band)`, the band row was reserved and its controls painted,
     then that same row was clipped out of the window region by
     `region_top_inset`'s (always-false) `decorated` check — an invisible
     header with no reachable gear.
   - Restoring a real OS caption on Windows is **deferred to its own
     sub-phase** (§4.2, and now master spec §13). It was rejected here
     because it requires mode-aware `frameless_style`/`apply_dwm_attributes`,
     early `DefSubclassProc` returns from three window-proc arms,
     `SetWindowRgn(hwnd, null)`, and — critically — it would **re-open the
     DWM shadow that commit `e45d55f` deliberately removed** by dropping
     `WS_CAPTION` in the first place. That is a correctness trade a rider on
     this phase should not make silently.

5. **Migration matrix and defaults** (`672bb6a`). Every existing Windows
   install lands in `header`: `decorations: true` → `native` → resolved to
   `header`; `decorations: false` → `header` directly. The shipped default
   was corrected from §7.4's stated `header` to what `Settings::default()`
   has always actually been, `native` — recorded as deviation #11 in the
   master spec, §12.

---

## 2. What Task 0 Deleted, and Where Its Measurements Went

`9330ff8` reverted roughly 200 lines marked `THROWAWAY PROBE (spike, revert
me)` across `commands.rs`, `xtask/`, and `tools/`: `PROBE_FLUSH`,
`PROBE_PAINTS`, `PROBE_ACTIVE`, `probe_clip`, `probe_wait_compose`,
`probe_window_rect`, `probe_qpc`, `probe_log_toggle[_at]`, `probe_note_paint`,
`probe_autotoggle`, the `probe_region_clip` command, the `WM_PAINT` probe arm,
the `DwmFlush` extern, the probe block in `peek_titlebar`, the call site in
`main.rs`, `xtask/src/probe.rs`, and the three `tools/probe-*` scripts. It went
first, in its own commit, partly because its injected JS keyed off
`data-decorations`, which this phase renames — it would have broken regardless
of anything else in 4A.

Kept: `update_clip_region` (a real command used on scale changes),
`STRIP_SHOWN`, and the measurement recorded in its doc comment (39% of reveals
flashed the desktop when the frame grew vs. 1% when the row is reserved and
only the region changes) — that number justifies the shipped design and
outlives the probe.

**The spike's two refuted measurement results now live in the design spec's
§7, not in code comments** — they no longer had a home once the probe code
was gone:

- `DwmFlush` on the main thread before the region change: worse, not better.
  300 reveals each, matched duration — without it 8/300 flashed (median
  3.6ms); with it 89/300 flashed (median 12.4ms), and it dragged the sampler
  from 144Hz to 80Hz because blocking the message pump delays the webview's
  own presents.
- The same wait moved off-thread, removing that objection: still worse.
  Matched 300s runs at 144Hz — without it 3/298 flashed and 1 stalled; with
  it 9/286 flashed and 12 stalled, median wait roughly doubled. `DwmFlush`
  returns when DWM *finishes* a frame, so changing the region right then
  lands just after it latched.

Together they rule out compose alignment as an approach, not one
implementation of it. The residual flash is a known, accepted glitch,
documented where it happens rather than chased further.

---

## 3. The Invariance Gate

Header mode's hover reveal was explicitly out of scope and had to come out of
this phase unchanged — §4.5 of the design spec makes the argument for why each
deletion in Task 0 and the Windows resolution work is safe for that mode by
construction (`showPeek`'s gate is logically identical to the old
`titlebarShown` check with one more member on the blocking side; `grow_top`
was never on the peek path; `region_top_inset` already returned `strip_px`
unconditionally on Windows before this phase; `initial_height`'s Windows
branch already returned the same number once its parameter collapsed).

The gate that proves it: **zero `e2e/panes.spec.ts` screenshot baselines
changed across all nine commits of this phase.** Verified directly against
the working tree — `e2e/panes.spec.ts-snapshots/` contains only the baselines
that predate this phase; the six new files this phase added are all under
`e2e/chrome.spec.ts-snapshots/`:

```
chrome-content-dark-win32.png    chrome-header-dark-win32.png    chrome-native-dark-win32.png
chrome-content-light-win32.png   chrome-header-light-win32.png   chrome-native-light-win32.png
```

Three modes × two themes, `-win32` suffixed, at 768×480 — the new spec file's
own baselines, not a rebase of the existing ones. `panes.spec.ts`'s two
hidden-title-bar stubs changed only their settings key, `decorations: false`
→ `chrome_mode: 'header'`, and still pass unmodified.

---

## 4. Verification Gate

Run at commit `eee8320` (this phase's tip before the docs commits):

- **Vitest:** 222 passed across 12 files.
- **`npm run build`:** clean.
- **Playwright:** 41 passed (22 pre-existing in `e2e/panes.spec.ts` and
  `e2e/popover.spec.ts` + 19 new in `e2e/chrome.spec.ts`).
- **`cargo test`:** 83 passed.
- **`cargo clippy --all-targets`:** exactly four warnings, all
  `upper_case_acronyms`, on `RECT`, `STYLESTRUCT`, `SUBCLASSPROC`,
  `WINDOWPOS` — the same four that predate this phase; 4A added no new
  clippy debt.

These numbers were produced by a run outside this documentation task and are
reported here as given, not re-executed for this handoff.

---

## 5. What 4B Inherits

**4B (per-pixel background alpha, §7.5) must clamp `--surface-alpha` to 100%
whenever `chrome_mode` is `native`.** That state now exists only on macOS and
Linux — Windows's `supported()` resolution means `native` is never the
resolved mode there, so **the Windows arm of that clamp has nothing to do.**
4B's implementation can gate the clamp on the resolved `chrome_mode` value
alone; it does not need a separate Windows branch to reach the same result,
because Windows structurally cannot be in the state the clamp exists for.

---

## 6. Known Limitations

- **The six new screenshot baselines are `-win32` only.** This phase was
  implemented and verified on Windows. A macOS run would need to generate its
  own `-darwin` baselines for `e2e/chrome.spec.ts`, matching the split
  `panes.spec.ts` already carries between `*-win32.png` and `*-darwin.png`.
  Until then, `chrome.spec.ts` is only a regression gate on Windows.
- **The macOS-only code paths were not exercised by anything in this phase.**
  `apply_decorations_macos`, the frame-growth half of `peek_titlebar`, and the
  macOS branch of `set_chrome_mode` are `cfg`-gated out of a Windows build, so
  nothing here ran or verified them beyond reading the code and the design
  spec's reasoning. The design spec's manual-verification list (§8, and the
  brief's manual-verification checklist) is what actually covers macOS — it
  was not re-run as part of this phase's own gate, and this handoff makes no
  claim about macOS behavior beyond what the spec asserts.

---

## 7. Corrections Made to the Docs During Phase Close

Three inaccuracies were found and fixed while writing this handoff, each the
result of a decision taken during implementation rather than a copy error:

1. **4A design spec §6.1** said `save_settings` "gains a `chrome_mode` merge
   arm alongside the existing ones." It does not, and must not:
   `chrome_mode` is persisted by its own dedicated command, `set_chrome_mode`,
   exactly as `always_on_top` and `clipboard_monitoring` are by
   `set_always_on_top` and `set_clipboard_monitoring` — neither of those has
   a `save_settings` merge arm either. A merge arm would let the settings
   window's snapshot (which includes `chrome_mode` in its payload, per
   `src/settings.ts`'s `currentSnapshot`) overwrite the value
   `set_chrome_mode` had just resolved through `supported()`, which on
   Windows would turn a transient in-session mismatch into a persisted wrong
   value. Rewritten to state what the code does and why.

2. **Master spec §7.2** still described the header/idle-band mechanic in
   terms of `#app[data-decorations]` and a `decorations: true/false` table.
   Renamed to `#app[data-chrome]` / `chrome_mode: native/header`, with a note
   that `header` and `content` share this band shape (content mode's
   never-peeking behavior is out of this table's scope; §7.4 covers it). This
   was the only literal `decorations`/`data-decorations` reference found in
   §7.2 or §7.4 — see the note below.

3. **Master spec §11/§12/§13** — the Phase 4 heading, the 4A scope cell, and
   deviations 10–12 — updated per the task brief's prescribed wording,
   confirmed against this phase's actual commits and the design spec's §4.2,
   §5.1, and §3.3.

---

## 8. Staleness Not Named by the Brief

**§7.4 turned out not to contain any literal `decorations` or
`data-decorations` text.** It already describes the three chrome states by
name (`Header + resize edges`, `Content only`, `Native frame`), so no rename
was needed there — contrary to what the correction instructions assumed. Left
as is, per "keep changes minimal, do not rewrite wholesale": §7.4's `Header +
resize edges (default)` label is still accurate to what §7.4 itself asks for,
and the divergence from the shipped default is already recorded as deviation
#11 rather than edited into the table.

**The 4A design spec's §8 (Verification) does not actually claim
`initial_height` takes a mode parameter** — its own bullet list already reads
"the initial height reserves the row on Windows and not elsewhere," with no
parameter mentioned. The two places in the design spec that *do* mention an
`initial_height` mode parameter (§4.4's deletions table, §4.5's invariance
table) both correctly describe it in the past tense, as something removed.
No edit was made to §8 as a result — the correction's premise did not hold
against the current text.
