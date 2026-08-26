# JParser Phase 4B — Handoff

**Date:** 2026-08-26
**Spec:** [`docs/superpowers/specs/2026-08-26-jparser-phase4b-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-26-jparser-phase4b-design.md)
**Commits:** `8543d9b..77a91f7` on `develop` (eight commits, unpushed)

---

## 1. What Was Completed in Phase 4B

1. **A see-through main window with per-pixel alpha** (`861fb1a`, `119fa25`,
   `2e8fbe4`, `91a0db2`). `"transparent": true` is set once on the only
   `WebviewWindowBuilder` for `index.html` and stays there unconditionally.
   At 100 % alpha the CSS layer renders identically to before — Playwright's
   screenshot baselines pass unchanged, which is what proves the token being
   `1` costs nothing *there*. Whether the window itself is unchanged is not
   established: `transparent: true` makes tao call
   `DwmEnableBlurBehindWindow` regardless of the token's value, and whether
   the `WM_ERASEBKGND` fill still lands under blur-behind at 100 % is
   exactly what Task 1's spike was meant to answer and did not (§5). Step
   1's first two manual checks are the ones that would settle it.

   `--surface-alpha` is a new custom property (`src/styles/tokens.css`), and
   `--color-bg-surface` is derived from it with `color-mix(in srgb,
   var(--color-bg) calc(var(--surface-alpha) * 100%), transparent)`.
   `--color-bg` itself is untouched in all three theme blocks; only the three
   surface use sites in `src/styles/global.css` (the body, and the two
   platform-split peeked-band rules) swap to the derived token. The
   `.context-menu` rule and everything in `settings.css` stay on the opaque
   `--color-bg` — see item 2.

2. **`surface_alpha` persisted, clamped, and applied** (`0f0bb2e`, `2e8fbe4`,
   `f813fdc`). `Settings.surface_alpha: Option<u32>` follows the font-size
   precedent exactly: absent means "stylesheet owns it", no Rust-side clamp,
   out-of-range values clamp at apply time. `src/surface-alpha.ts` is a new,
   deliberately separate module — not a fourth `FontKind` — exporting
   `ALPHA_LIMITS`, `clampAlpha`, and `surfaceAlphaVar(alpha, chrome)`, the
   pure function that returns `['--surface-alpha', '1']` in `native`,
   `['--surface-alpha', '']` (property removed) when unset, and the ratio
   string otherwise. It is called from `applySettings`, the
   `settings-changed` listener, and `applyChromeMode` in `main.ts`, and all
   three paths call `reportBackground()` afterward so the Windows erase gate
   (item 3) learns the effective alpha in the same beat.

3. **The Windows `WM_ERASEBKGND` fill gated on opacity** (`8dc1e3e`).
   `set_window_background` grows a fourth argument, `opaque: bool`; the erase
   arm falls through to `DefSubclassProc` instead of filling when it is
   false. The value sent is the *effective* alpha from `surfaceAlphaVar`,
   not the stored `surface_alpha` — `native` is always `opaque: true`
   regardless of what the setting holds, because an opaque OS caption over a
   see-through body would be a rendering artifact. The backend never
   receives the alpha number itself, only permission to fill.

4. **The opacity slider** (`f813fdc`). A new "Background" settings group
   directly under Window Chrome. `input` broadcasts a partial
   `{ surface_alpha }` payload without saving (so a drag doesn't run the
   gloss-filter merge or a full sentence re-render sixty times a second);
   `change` goes through the existing save-then-broadcast path. Disabled and
   given a visible explanatory note (not a `title`) whenever chrome is
   `native` — see Ruling 2 below.

5. **macOS's `macos-private-api` feature** (`91a0db2`), added to
   `src-tauri/Cargo.toml` and `"macOSPrivateApi": true` in
   `tauri.conf.json`. Mechanical; not exercised, see §5.

6. **Coverage** (`77a91f7`): `src/surface-alpha.test.ts` (new), extensions to
   `src/settings.test.ts`, `src/main.test.ts`, `e2e/chrome.spec.ts`, and
   `src-tauri/src/settings.rs` / `commands.rs`.

---

## 2. Why `--color-bg` Stayed Opaque and `--color-bg-surface` Was Derived

The obvious move — rewrite `--color-bg` itself with the alpha folded in — was
rejected because `--color-bg` has a fourth use site the other three don't
share: `.context-menu` (`global.css:510`). Content-only mode (4A) already
removes the gear, history nav, and window buttons, which makes the
right-click menu the app's only reach into itself in that mode. At 0 %
surface alpha it becomes the only reach *at all* — a menu that faded with the
window would take the app down with it. `settings.css` (`.settings-page`,
`.settings-group`, etc.) has the same requirement for the same reason: a
see-through settings form is unreadable regardless of mode.

So the fade lives on a second, derived token instead: `--color-bg-surface`,
defined once in the bare `:root` block of `tokens.css`, referencing
`--color-bg` through `color-mix` so it follows every theme branch (light,
`prefers-color-scheme: dark`, `[data-theme='dark']`) without being restated
in any of them. `color-mix` interpolates with premultiplied alpha, so mixing
an opaque colour toward `transparent` moves only the alpha channel and
leaves the RGB untouched — `in srgb` rather than a polar space, so there is
no hue path to reason about. Three use sites (`global.css:31`, `:88`, `:234`
— the window surface and the two platform-split peeked-band rules) swap to
the derived token; the fourth (`:510`, the context menu) and everything in
`settings.css` do not.

---

## 3. The Clamp Is Applied, Never Persisted

`surfaceAlphaVar(alpha, chrome)` forces the CSS token to `1` whenever
`chrome === 'native'`, but nothing on that path ever writes `surface_alpha`
to `settings.json`. Switching to `native` and back returns the user's chosen
alpha unchanged — the same discipline 4A's handoff §7 defends for
`chrome_mode` itself: a value the platform or another mode temporarily
resolves to must not overwrite the value the user actually chose.

`src/main.test.ts`'s pair of tests for this — forced to `1` while native,
restored on leaving it — is, per the design spec §9, "the whole clamp
contract and the phase's most important test." Both pass.

**4D's slot must capture the stored value, not the clamped one.** A future
layout-hotkey snapshot that reads the live `--surface-alpha` CSS value while
the window happens to be in `native` mode would capture `1` and silently
overwrite whatever the user had actually set the slider to. It must read
`settings.json`'s `surface_alpha` (or the in-memory value that mirrors it)
instead. Recorded in the master spec's §11 4D row as of this close.

---

## 4. Windows Never Needed a Platform Branch for the Clamp

4A's handoff §5 predicted this and was right: because Windows'
`supported()` resolves `native` to `header` before a value is ever
persisted (4A, `settings.rs`), Windows structurally cannot be in the state
the clamp exists for. `surfaceAlphaVar` keys on the resolved `chrome_mode`
value alone and reaches the correct answer on all three platforms without a
`cfg!(target_os = ...)` check anywhere in the alpha path. No platform branch
was added, and none was needed.

---

## 5. What Is Unverified

- **The macOS arms, exactly as 4A left them.** `macos-private-api` taking
  effect, and native-mode alpha correctly forcing to 100 % under macOS's
  `titleBarStyle: "Overlay"`, were reasoned about (design spec §6) but not
  run. This phase was implemented and verified entirely on Windows, like 4A.
- **`transparent: true` is exercised by nothing in the automated suite.**
  Playwright drives the frontend in plain Chromium — a browser tab, not the
  Tauri window — where the `transparent` config key has no effect at all.
  A green Playwright run (see §6) proves the CSS token math and computed
  styles are correct; it proves nothing about whether the Win32 or macOS
  window layers actually render a see-through surface. That can only be
  seen by a human looking at the real window, which did not happen — see
  §6's manual items below.
- **Every one of the six manual checks in the task brief's Step 1 — NOT
  RUN.** Individually, by name:
  1. Header mode at 100 % opacity vs. pre-phase `develop` — **NOT RUN.**
  2. Round corners, no drop shadow, at 100 % opacity — **NOT RUN.**
  3. Ten pointer entries, counting desktop-coloured flashes, compared
     against the spike — **NOT RUN** (and see below: there is no spike
     result to compare against).
  4. Dragging the slider 100 → 0, confirming the sentence and furigana stay
     crisp — **NOT RUN.**
  5. Right-click at 0 % opacity, content mode, confirming the menu is solid
     and Settings opens — **NOT RUN.**
  6. Switching to `native` and back on macOS/Linux, confirming
     `surface_alpha` never changes in `settings.json` — **NOT RUN** (also
     inapplicable on the Windows machine this phase was built on, since the
     `native` button is absent there).

  None of these six is claimed as passed, inferred, or simulated anywhere in
  this handoff. An unrun check that goes unmentioned would read to whoever
  picks up 4D as a check that passed; it did not run at all.

- **Task 1's spike never ran, and there is no spike-notes file.** The design
  spec (§5.3) called for a throwaway Windows probe to answer one question:
  does the `WM_ERASEBKGND` fill still land once tao has called
  `DwmEnableBlurBehindWindow`? `CreateSolidBrush` takes a `COLORREF`, which
  carries no alpha channel, so GDI may write zero into the alpha byte of the
  fill — and under blur-behind, zero means fully transparent. If the fill is
  dead, the anti-flash mechanic 4A measured down to 1 % of reveals is
  degraded even at 100 % surface alpha, where the user asked for no
  transparency at all, and nobody currently knows whether that is happening.
  The spike was never executed — it needs a human watching a real Windows
  window resize while a colour is forced to magenta — so
  `docs/superpowers/2026-08-26-phase4b-spike-notes.md` does not exist; do
  not treat any reference to it elsewhere (including this phase's own design
  spec and task brief) as describing a file that was actually written.
  The `opaque` gate (§1 item 3 above) shipped regardless of the answer,
  because it is the correct behaviour either way — below 100 % the fill is
  *meant* to be skipped, and at 100 % it is meant to run. What remains
  unknown is only whether, at 100 %, it still visibly does what it always
  did. This is the design spec's own §5.4 "spike says the fill is dead"
  branch: a known limitation, not a silent regression, with its two named
  alternatives (a premultiplied DIB with `AlphaBlend`, or accepting the
  residual flash) still on the table for whoever eventually runs the check.

- **Task 8's Step 6 also never ran** — looking at the real window after
  `transparent: true` was switched on for real (not the spike's temporary
  flag). Same class of gap as the spike: nothing in this phase's automated
  suite can substitute for it, because Playwright cannot see it (above).

---

## 6. Verification Gate

Run at commit `77a91f7`, the phase's final tip:

- **Vitest:** 244 passed (up from 228 before the phase).
- **`npm run build`:** clean.
- **Playwright:** 44 passed (up from 41), and — the invariance gate this
  phase most needed — **zero screenshot baselines were regenerated at any
  point in the phase.** The existing `e2e/chrome.spec.ts` baselines pass
  unchanged at the default 100 % alpha; new assertions added by this phase
  go against computed styles (the body's background alpha tracking the
  slider, `.context-menu` staying fully opaque at 0 %), not pixels, because
  screenshotting a see-through window in headless Chromium would test the
  test harness's own backdrop rather than the app.
- **`cargo test`:** 88 passed (up from 83).
- **`cargo clippy --all-targets`:** exactly four warnings, all
  `upper_case_acronyms`, on `RECT`, `STYLESTRUCT`, `SUBCLASSPROC`,
  `WINDOWPOS` — the same four that predate this phase. 4B added no new
  clippy debt.

---

## 7. Design Decisions Worth Carrying Forward (Rulings Taken During Execution)

1. **A latent Phase 4A bug was found and fixed here.** `main.ts`'s
   `settings-changed` listener called `applyFontSizes(payload)`
   unconditionally, and `fontVars` treats an absent size as "clear it". So
   any partial `settings-changed` payload wiped the user's font-size tokens
   — and `main.ts` already emitted a bare `{ chrome_mode }` when a mode was
   picked from the right-click menu, meaning **picking a chrome mode in the
   shipped 4A build silently reset the font sizes and the popover gloss
   size.** 4B's slider, which emits `{ surface_alpha }` alone on every
   `input` event, would have hit the identical path up to sixty times a
   second during a drag. Fixed in `2e8fbe4` with a guard that runs
   `applyFontSizes` only for a payload recognisable as a full snapshot,
   discriminated by the two non-optional `Settings` keys (not by "does the
   payload carry a font key?", which is wrong: clearing all three font boxes
   legitimately produces a full snapshot with all three keys absent, and
   those CSS tokens must still be removed in that case).

2. **The disabled slider's explanation had to become visible text, not a
   `title`.** The design spec (§7.3) specified a `title` naming the native
   frame as the reason the control is disabled. That is unreachable: a
   `disabled` input leaves the tab order entirely, so its `title` is a
   mouse-hover-only affordance, and the wrapping label already supplies the
   accessible name — so the tooltip would never be announced to a keyboard
   or assistive-technology user either. Replaced with a visible note
   element in the Background settings group, shown only in `native` mode
   (`f813fdc`), following the same reasoning 4A already applied elsewhere in
   this codebase (`visibility: hidden` over `opacity: 0` so unreachable
   controls leave the tab order; the Windows `native` button removed
   outright rather than disabled).

3. **`currentSnapshot()` always carries `surface_alpha`,** because a range
   input always has a value — unlike the font-size boxes, it cannot
   represent "unset" by being empty. So changing any unrelated setting
   persists the seeded 100 into `settings.json`, even though the design
   spec's §7.4 says the key "stays absent... until the user actually
   drags". Accepted, no code change: the font boxes already behave exactly
   this way once `showDefaultsInEmptyBoxes` seeds them, and persisting 100
   when the stylesheet default is also 100 is behaviourally inert — nothing
   reads the two states differently. Worth recording here so a future reader
   does not mistake the early key for a bug; §7.4's sentence describes the
   intent, not the literal mechanism.

---

## 8. What This Phase Did Not Clear

- **4A's own manual checklist** — never run in 4A, still not run here; 4B
  did not touch anything on that list and does not get to mark it done.
- **`docs/superpowers/2026-08-18-windows-titlebar-test-plan.md`** — carried
  forward unreported since at least 4A's handoff §6; still outstanding.
- **4A's open finding on `set_chrome_mode`'s persist ordering** (4A handoff
  §9): persistence now runs before `chrome_target` and
  `apply_decorations_macos`, so a failure in either leaves a persisted mode
  the window was never actually updated to match. 4B adds no new writer to
  `set_chrome_mode`'s path and does not touch or resolve this.
- **All six of Step 1's manual checks and the two Windows-only manual items**
  named in §5 above.

---

## 9. Master Spec Changes Made at This Close

- **Phase 4 heading and intro** (§11): now reads "4A, 4B and 4C shipped; 4D
  remaining", with the intro rewritten to describe 4B as shipped rather than
  as a sub-phase with "a defined state to clamp".
- **§11's 4B row**: rewritten to past tense, and to record that the window
  is transparent *unconditionally* with alpha clamped to 100 % in `native`
  — not the `"transparent": true` + `decorations: false` pairing §7.5
  originally described, which predates 4A's three-way `chrome_mode` and was
  already stale before this phase started.
- **§7.5**: gained one paragraph recording the 0 % floor and why it differs
  from ta-old's 20/255 clamp (per-pixel alpha leaves text opaque, so the
  reason for a floor above zero does not apply).
- **§12's deviation #4**: extended in place with the same floor consequence,
  per the brief's instruction to extend the existing row rather than add a
  new numbered one. No new row was warranted — the manual checks that could
  have turned up a genuinely new behaviour did not run (§5), so there is no
  new finding to number. This is stated explicitly rather than silently
  choosing not to add a row.
- **§11's 4D row**: gained a clause noting the slot must capture the
  *stored* `surface_alpha`, not the value 4B's clamp resolves it to in
  `native` mode.

No other master-spec text was touched. The brief's Step 2 did not ask for
edits beyond these five, and none of them needed a wholesale rewrite — each
was a targeted addition or tense change to an existing sentence or table
cell.
