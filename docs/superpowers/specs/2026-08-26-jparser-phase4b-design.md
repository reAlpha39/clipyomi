# JParser Phase 4B — Per-Pixel Background Alpha Design

**Date:** 2026-08-26
**Status:** Designed, not started.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.5, §11, §12 #4)
**Depends on:** 4A (`docs/superpowers/specs/2026-08-25-jparser-phase4a-design.md`), whose
`chrome_mode` is what this phase clamps against.

---

## 1. Goal

§7.5 asks for a see-through window background with fully opaque text: the app
sits over a game, the Japanese stays crisp, the artwork behind it stays
visible. ta-old did this with uniform whole-window alpha and dimmed the text
the user was trying to read; master spec deviation #4 already refuses that
trade. This phase delivers the per-pixel version — one custom property,
`--surface-alpha`, and a slider bound to it.

4A is the dependency because a window wearing a native OS title bar cannot be
see-through underneath it without looking broken. 4A defined that state; 4B
clamps it.

---

## 2. Scope

A `surface_alpha` setting, a permanently transparent main window, a derived
background token the alpha multiplies into, a range slider with live preview,
and the clamp that pins alpha to 100 % in `native` chrome mode.

**Hard constraint:** at 100 % alpha the app must render and behave exactly as
it does today, on every platform. Transparency is a capability the window
gains, not a change to how it looks by default. §5.4 is the gate.

Out of scope, and why:

| Not doing | Reason |
| --- | --- |
| Alpha on the popover window | Decided in brainstorming: the popover is a transient reading surface over arbitrary game pixels, where legibility matters most. It imports `tooltip.css` alone and never `global.css`, so it is unaffected by construction rather than by a guard |
| Alpha on the settings window | A see-through form is unreadable, and the window is decorated on every platform |
| Runtime toggling of `transparent` | tao applies `DwmEnableBlurBehindWindow` at window creation only (§5.2). Toggling means destroying and rebuilding the main window; 4A deleted `grow_top` to stop doing exactly this |
| OS-level uniform alpha | `SetLayeredWindowAttributes` / `NSWindow.alphaValue` dims the text. Master spec deviation #4 |
| Click-through below a threshold | Rejected in brainstorming: `set_ignore_cursor_events` makes the window undraggable and the right-click menu unreachable — the same trap one layer down, for a state nobody asked for |
| Layout hotkeys | 4D. A slot captures background alpha among other state, so it must come after this |
| Restoring a real OS caption on Windows | Master spec §13, unchanged by this phase |

---

## 3. The token and where it multiplies

### 3.1 Why `--color-bg` cannot carry the alpha

The obvious move — rewrite `--color-bg` as `oklch(L C H / var(--surface-alpha))`
in all three theme blocks — is wrong, and the reason is one line of CSS.

`--color-bg` has four use sites in `src/styles/global.css`:

| Line | Rule | Should it fade? |
| --- | --- | --- |
| 31 | `html, body` | **yes** — this is the window surface |
| 88 | `#app[data-platform='windows'] … .peeked .controls` | **yes** — the band is part of the surface |
| 234 | `#app:not([data-platform='windows']) … .peeked .controls` | **yes**, the same band on the other platforms |
| 510 | `.context-menu` | **no** |

The context menu is the escape hatch. Content-only mode already removes the
gear, the history nav and the window buttons, which makes the right-click menu
the only reach into the app in that mode (4A §3.3). At 0 % alpha it becomes the
only reach *at all*, and a menu that faded with the window would take the app
with it. It stays on the opaque token.

`src/styles/settings.css` also references `--color-bg` and `--color-surface`,
and must not fade either.

### 3.2 The derived token

Two additions to `src/styles/tokens.css`, in the bare `:root` block only:

```css
--surface-alpha: 1;
--color-bg-surface: color-mix(
  in srgb,
  var(--color-bg) calc(var(--surface-alpha) * 100%),
  transparent
);
```

`--color-bg` is left exactly as authored, opaque, in all three theme blocks —
the light `:root`, the `prefers-color-scheme: dark` branch, and the
`:root[data-theme='dark']` override. The derived token references it, so it
follows every theme branch without being restated in any of them, and the
single place a background default lives stays `tokens.css`.

`color-mix` interpolates with premultiplied alpha, so mixing an opaque colour
toward `transparent` moves only the alpha channel and leaves the RGB
untouched. `in srgb` rather than a polar space so there is no hue path to
reason about at all.

`global.css` lines 31, 88 and 234 swap to `var(--color-bg-surface)`. Line 510
and everything in `settings.css` do not.

Nothing else needs to change for the visual effect. Text, ruby, chips, and
glosses never reference a background token, so they are opaque at every alpha
by construction — which is the whole point of per-pixel over uniform.

### 3.3 One transparent window, declared once

`"transparent": true` goes into the single window declared in
`src-tauri/tauri.conf.json` and stays there permanently. At 100 % alpha the
window renders identically to today; transparency costs nothing when the
token is `1`.

There is no second creation path to keep in step: the only other
`WebviewWindowBuilder` for `index.html` is the `#[cfg(test)]` helper at
`src-tauri/src/commands.rs:1643`, which exists so `chrome_target` has a
labelled window to resolve.

### 3.4 Only the main window sets the property

`--surface-alpha` defaults to `1` in `tokens.css` and is written at runtime
only by `src/main.ts`. The settings window imports `global.css` but never sets
the property, so it computes to `1` there, and `.settings-page` paints opaque
`--color-bg` over the body regardless. The popover imports `tooltip.css` only.

---

## 4. Persistence and the native clamp

### 4.1 The setting

`surface_alpha: Option<u32>` on `Settings` in `src-tauri/src/settings.rs`, a
percentage, with `#[serde(default, skip_serializing_if = "Option::is_none")]`
— the font-size precedent verbatim, including its semantics: `None` means
"never set", the stylesheet owns the value, and the key stays out of the file.

No `normalize` arm. There is no predecessor key to fold in, so there is
nothing to migrate.

No clamp on the Rust side. Out-of-range values from a hand-edited file clamp
at apply time exactly as font sizes do, and a negative number fails
`Option<u32>` deserialization into the existing corrupt-file-plus-warning
path in `load`.

### 4.2 A new module, not a fourth `FontKind`

`src/surface-alpha.ts`, roughly thirty lines, mirroring `src/font-sizes.ts`'s
shape without borrowing its machinery:

```ts
export const ALPHA_LIMITS = { min: 0, max: 100 };
export function clampAlpha(value: unknown): number | null;
export function surfaceAlphaVar(
  alpha: number | null,
  chrome: ChromeMode | undefined,
): [string, string];
```

Its own file because alpha is not a font size: `FONT_LIMITS`, the `TOKENS`
map, `ROMAJI_PER_MILLE` and `stylesheetDefault`'s em-versus-percent branch
have nothing to lend it, and folding a fourth kind into `FontKind` would make
every one of them carry a case that does not apply.

### 4.3 The floor is 0 %, deliberately

`ALPHA_LIMITS.min` is `0`, not ta-old's clamp. ta-old bounded
`SetLayeredWindowAttributes` at 20–255 because uniform alpha at zero took the
text with it. Per-pixel alpha does not: at 0 % the background is gone and the
sentence, its furigana and the chips are all still fully opaque.

The residual hazard is that a window with a fully transparent background still
consumes mouse clicks inside its rect. Two things already in the design answer
it, at no extra cost:

- the right-click menu keeps an opaque background (§3.1), so right-click →
  Settings still works at 0 %;
- opaque text keeps the window locatable, including the empty-state
  placeholder when there is no parse result.

No threshold, no guard, no click-through. The trap the floor could have
created is closed by two rules that exist for other reasons.

### 4.4 The clamp is a pure function of two arguments

`surfaceAlphaVar(alpha, chrome)` returns the `[token, value]` pair the caller
writes onto `document.documentElement`:

| `chrome` | `alpha` | Result |
| --- | --- | --- |
| `native` | anything | `['--surface-alpha', '1']` |
| `header` / `content` | `null` (unset) | `['--surface-alpha', '']` — property removed, `tokens.css` owns it |
| `header` / `content` | `0..100` | `['--surface-alpha', String(alpha / 100)]` |

Why the clamp exists: `tauri.conf.json` sets `titleBarStyle: "Overlay"`, and
`native` means the OS draws a solid title bar. A see-through body under an
opaque OS caption is a rendering artifact, not a design.

Why it needs no platform branch, per 4A's handoff §5: Windows resolves
`native` to `header` in `supported()` before the value is ever persisted, so
Windows structurally cannot be in the state the clamp exists for. Keying on
the resolved mode alone reaches the right answer on all three platforms.

Returning `''` for an unset value rather than `'1'` matters: `fontVars` uses
the same convention, and `main.ts`'s apply loop already turns `''` into
`removeProperty`. Writing `'1'` would pin an inline style that the stylesheet
could then never override.

### 4.5 The stored value survives the clamp

The clamp is applied, never persisted. Switching to `native` and back returns
the user's alpha unchanged, because nothing on the native path writes
`surface_alpha`. This is the same discipline 4A's handoff §7 defends for
`chrome_mode` — the value the user chose is not overwritten by a value the
platform or another mode temporarily resolves to.

It also makes the clamp a Vitest case instead of a manual check, in both
directions: forced to `1` while native, restored on the way out.

### 4.6 Where it is applied

Alongside `applyFontSizes` in `src/main.ts`, on the paths that already exist:

- `applySettings` at startup;
- the `settings-changed` listener;
- inside `applyChromeMode` — the mode changing is itself a reason for the
  clamp to flip, and that path does not otherwise carry an alpha.

All three then call `reportBackground()`, which today runs only at startup and
on an OS theme change. It has to follow the alpha as well, because §5.4's
`opaque` argument rides on it: switching to `native` makes an alpha of 40 %
render as 100 %, and the Windows erase gate has to learn that in the same beat
the token does. Sending it unconditionally on these paths is cheaper than
tracking whether the effective value crossed the boundary.

---

## 5. Windows: the erase path

### 5.1 What is there today

`src-tauri/src/commands.rs`'s `WM_ERASEBKGND` arm fills the whole client rect
with `CreateSolidBrush(BACKGROUND)` and returns 1. `BACKGROUND` is a
`COLORREF` the webview itself reports through `set_window_background`, so the
fill matches what the page is about to paint.

The `ponytail:` comment above `frameless_subclass_proc` records what this is
for and what has already been measured. Two of its findings decide this
section:

> `the erase colour: traced as 0x121212 against a page rendering rgb(18, 18, 18)`
> `— it already matches, a magenta erase proved the fill lands`

> `the WebView2 surface: an opaque cyan webview background never showed, so the`
> `gap is parent client area, not webview content`

So the fill demonstrably paints today, it is parent-side, and the anti-flash
behaviour 4A measured down to 1 % of reveals depends on it.

### 5.2 What transparency does to it

Verified by reading the dependencies rather than assumed:

- `tao-0.35.3/src/platform_impl/windows/window.rs:1284` — when `transparent`
  is set, tao calls `DwmEnableBlurBehindWindow` with an empty blur region.
  This is the standard per-pixel-alpha-without-`WS_EX_LAYERED` trick, and it
  is applied **at window creation only**. There is no runtime toggle, which
  is what makes §2's "no runtime toggling" entry a fact about the runtime and
  not a preference.
- `wry-0.55.1/src/webview2/mod.rs:127` and `:452` — a transparent window
  gets the WebView2 controller's `DefaultBackgroundColor` set to `(0, 0, 0, 0)`,
  so the webview composites with alpha over whatever the parent window shows.

The parent window shows the `WM_ERASEBKGND` fill. And `CreateSolidBrush` takes
a `COLORREF`, which carries no alpha, so GDI writes zero into the alpha byte of
the redirection surface. Under DWM per-pixel alpha, zero means fully
transparent.

The fill that exists to stop the desktop showing through may therefore become
a hole showing the desktop — at 100 % alpha, where the user asked for no
transparency at all. That would be a regression in the one mechanic 4A spent a
phase measuring, and it is not a thing to guess about.

### 5.3 Task 0 — the spike

Throwaway, reusing this file's own proven probe. Before any real code:

1. Add `"transparent": true`, leave alpha at 100 %, run `npm run tauri dev` on
   Windows.
2. Force `BACKGROUND` to magenta and resize the window. Magenta visible → the
   fill still lands under blur-behind. Nothing visible → it does not.
3. Watch the round corners (`apply_round_region` plus `DWMWCP_DONOTROUND`) and
   one header-mode peek reveal.

The spike's code is discarded either way. Only the answer is kept, and it
decides one branch of §5.4.

### 5.4 The gate, either way

`set_window_background` grows a fourth argument, `opaque: bool`, and the
`WM_ERASEBKGND` arm falls through to `DefSubclassProc` instead of filling when
it is false. That ships regardless of the spike's answer.

The alpha comes from the frontend's already-clamped value, not from re-reading
the body colour through `resolveColor`'s canvas. One writer, no
premultiplication arithmetic to get wrong, and the regex fallback for
canvas-less environments stays as it is. The backend never needs the alpha
value — only permission to fill.

**`opaque` is the *effective* alpha, not the stored one.** It is true when the
value `surfaceAlphaVar` resolved is 100 % — which includes every native-mode
case, whatever `surface_alpha` happens to hold. Reading the stored value here
would skip the fill in native mode, where the window is fully opaque and the
fill is exactly what stops the flash.

- **Spike says the fill lands** → the gate is the whole story. Below 100 % the
  fill is skipped and the desktop showing through is the feature.
- **Spike says the fill is dead** → the gate still ships, and the residual
  flash at 100 % is recorded as a known limitation with its alternatives named
  (a premultiplied DIB with `AlphaBlend`, or accepting it) rather than left as
  a silent regression.

**The hard constraint, in the shape 4A §4.5 used:** at 100 % alpha, Windows
must behave exactly as it does today — same round corners, same reveal, same
resize behaviour. §9's Playwright baselines and the manual list are where that
is proved.

### 5.5 What transparency forecloses

Worth recording so it is not rediscovered later:

- The `ponytail:` comment's own suggested structural fix — "put an opaque
  filler child behind the webview" — is now permanently off the table.
- `DwmExtendFrameIntoClientArea` was already ruled out there for bringing back
  the drop shadow `e45d55f` removed. Unchanged.
- The accepted top-edge glitch changes character rather than disappearing: at
  low alpha, "the desktop shows through for one frame" stops being a glitch
  and becomes the effect. Its severity is now alpha-dependent, which is why
  the gate is pinned at 100 %.

---

## 6. macOS

Two mechanical additions:

```jsonc
// src-tauri/tauri.conf.json
"app": { "macOSPrivateApi": true, ... }
```

```toml
# src-tauri/Cargo.toml
tauri = { version = "2", features = ["macos-private-api"] }
```

`tauri-utils`' `AppConfig` reads `macOSPrivateApi` (alias `macos-private-api`)
and `AppConfig::features()` reports it, so a mismatch between the config flag
and the cargo feature surfaces at build time rather than as a silent no-op.

The App Store exclusion is real and already accepted in master spec §7.5 — a
self-distributed GPL v2 build. Nothing new to record.

Native mode needs no code beyond §4.4's clamp: alpha forced to `1` means the
body paints opaque under the `titleBarStyle: "Overlay"` caption, which is
today's rendering exactly.

**Limitation, stated up front.** This phase will be implemented and verified on
Windows, as 4A was. The macOS arms — the private-API flag taking effect, and
native-mode alpha at 100 % — will be reasoned about and left unverified. The
handoff says so rather than claiming otherwise.

---

## 7. The control

A new `<section class="settings-group">` titled **Background**, placed
immediately after Window Chrome so the clamp's cause sits next to its effect:

```html
<label class="settings-range-row">
  <span class="label-text">Opacity</span>
  <input type="range" id="setting-surface-alpha" min="0" max="100" step="1" />
  <output class="unit">100%</output>
</label>
```

### 7.1 Two events, two jobs

`input` broadcasts a partial payload, without writing. `change` goes through
the existing `saveAndBroadcast`, which sends the full `currentSnapshot()` —
`save_settings` merges field by field over a whole `Settings` and has no
fragment form. A drag therefore updates the main window continuously and
touches `settings.json` once.

`saveAndBroadcast` splits into `broadcast(payload)` plus a save-then-broadcast
that calls it with `currentSnapshot()`. That is the only refactor in
`src/settings.ts`.

### 7.1.1 `save_settings` gains a merge arm — unlike `chrome_mode`

`s.surface_alpha = settings.surface_alpha` guarded by `is_some()`, alongside
the gloss-filter arms rather than with the font sizes.

The font sizes are *assigned* unguarded, deliberately, so that clearing a box
restores the stylesheet default (`commands.rs`'s own comment on that block). A
range input cannot be cleared, so alpha has no "absent means reset" state to
express and an unguarded assign would only add a way for a future caller to
wipe the value by omission.

This is not in tension with 4A's handoff §7, which argues *against* a
`chrome_mode` merge arm. That argument turns on `chrome_mode` having its own
command, `set_chrome_mode`, which resolves the value through `supported()` — a
second writer could overwrite a resolved mode with an unresolved one.
`surface_alpha` has no dedicated command and no resolution step: `save_settings`
is its only writer, and the clamp lives in the stylesheet layer, not in the
persisted value (§4.5).

### 7.2 The `input` broadcast is a partial payload

`{ surface_alpha: n }`, not `currentSnapshot()`.

Precedent: `src/main.ts:405` already emits a bare `{ chrome_mode: mode }`, and
the `settings-changed` listener guards every field with `!== undefined`.

This matters beyond tidiness. A full snapshot at drag rate would run the gloss
filter merge and the furigana comparison sixty times a second for a value
neither contains, and the furigana branch calls `display(lastResult)` — a full
re-render of the sentence.

No throttle. `input` on a range already coalesces to roughly one event per
frame under a mouse drag; `requestAnimationFrame` batching is what to add if
that turns out to be false, not before.

### 7.3 Disabled in native mode

`slider.disabled` is set in both `applySettings` and `updateChromeButtons` —
the mode can change from inside this same window, so the control has to react
to its own sibling. It carries a `title` naming the native frame, because a
disabled control with no stated reason reads as a bug.

On Windows the `native` button is removed at startup (4A §6.3), so the slider
is never disabled there.

### 7.4 One asymmetry with the font boxes

A range input always has a value; it cannot represent "unset" by being empty.
It initialises from `getComputedStyle`'s `--surface-alpha` — 100 — the same way
`showDefaultsInEmptyBoxes` reads the font defaults back out of the stylesheet.
`surface_alpha` stays absent from `settings.json` until the user actually
drags. Same semantics as the font sizes, different mechanism, because the
element is different.

---

## 8. Files touched

| File | Change |
| --- | --- |
| `src-tauri/tauri.conf.json` | `"transparent": true` on the main window; `"macOSPrivateApi": true` |
| `src-tauri/Cargo.toml` | `tauri` gains the `macos-private-api` feature |
| `src-tauri/src/settings.rs` | `surface_alpha: Option<u32>` plus its round-trip tests |
| `src-tauri/src/commands.rs` | `set_window_background` gains `opaque: bool`; the `WM_ERASEBKGND` arm honours it; `save_settings` gains a guarded `surface_alpha` merge arm (§7.1.1) |
| `src/styles/tokens.css` | `--surface-alpha`, `--color-bg-surface` |
| `src/styles/global.css` | lines 31, 88, 234 swap to the derived token; line 510 does not |
| `src/styles/settings.css` | `.settings-range-row` |
| `src/surface-alpha.ts` | **new** — `ALPHA_LIMITS`, `clampAlpha`, `surfaceAlphaVar` |
| `src/surface-alpha.test.ts` | **new** |
| `src/types.ts` | `surface_alpha?: number` on `Settings` |
| `src/main.ts` | applies the token in `applySettings`, the listener, and `applyChromeMode`; passes `opaque` to `set_window_background` |
| `src/settings.ts` | the slider, the `broadcast` split, the disabled rule |
| `e2e/` | computed-style assertions; existing baselines must pass unchanged |

---

## 9. Verification

**Vitest.**

- `src/surface-alpha.test.ts` — `clampAlpha` bounds, non-numbers, rounding;
  `surfaceAlphaVar`'s three rows from §4.4; unset returning `''`.
- `src/settings.test.ts` — `input` broadcasts without persisting; `change`
  does both; the slider is disabled in native mode and re-enabled on leaving
  it.
- `src/main.test.ts` — the token reaches `documentElement`; native forces `1`
  **while the stored value survives**; switching back restores it. That pair
  is the whole clamp contract and is the phase's most important test.

**Rust.** `settings.rs`: `surface_alpha` round-trips through the file, and an
unset value stays out of it. `commands.rs`: `save_settings` with
`surface_alpha: None` leaves a stored value alone — the guard from §7.1.1,
which is the one line separating alpha from the font sizes' assign. No test on
the `set_window_background` gate —
asserting that a boolean reaches an `if` is tautology, and the Win32 arm it
guards is unreachable from `MockRuntime`. Recorded in the handoff rather than
papered over.

**Playwright.** The invariance gate is free: at the default 100 % alpha,
`e2e/chrome.spec.ts`'s existing baselines must pass **unchanged**. A diff
there means transparency altered the opaque rendering, which is the failure
this phase most needs to catch.

New assertions go against computed styles, not pixels — the body's
`backgroundColor` alpha tracks the slider, and `.context-menu` stays fully
opaque at 0 % — because screenshotting a see-through window in headless
Chromium tests the harness's backdrop rather than the app.

**Manual, Windows.** The spike's three observations re-run against the real
implementation at 100 %: the magenta-probe fill behaviour, the round corners,
one header-mode peek. Then a drag from 100 % to 0 %, confirming the menu stays
solid and the window stays locatable by its opaque text.

**Not cleared by this phase.** 4A's manual checklist and
`docs/superpowers/2026-08-18-windows-titlebar-test-plan.md` remain
outstanding. 4B inherits them and does not get to mark them done.

---

## 10. Deferred

- Alpha on the popover window (§2).
- A premultiplied-DIB erase, if and only if §5.3 shows the `COLORREF` fill
  dies under blur-behind and the residual 100 % flash proves intolerable.
- Restoring a real OS caption on Windows — master spec §13, untouched here.
- 4A's open finding on `set_chrome_mode`'s persist ordering (4A handoff §9).
  4B adds no new writer to that path and does not resolve it.

---

## 11. Deviations to record in the master spec at phase close

1. **§7.5's `"transparent": true` + `decorations: false` pairing no longer
   describes the implementation.** It was written when chrome was a boolean.
   4A replaced that with `chrome_mode`, and 4B keeps the window transparent
   unconditionally while clamping alpha to 100 % in `native` — the same
   intent, expressed against the three-way control that actually exists.

2. **Deviation #4 gains a consequence worth stating:** because per-pixel alpha
   leaves the text opaque, the control's floor is 0 % rather than ta-old's
   20/255. ta-old's floor existed to stop uniform alpha taking the text with
   it, and that reason does not survive the change of mechanism.

3. **4D's slot must capture `surface_alpha`**, and must capture the stored
   value rather than the clamped one — §4.5.
