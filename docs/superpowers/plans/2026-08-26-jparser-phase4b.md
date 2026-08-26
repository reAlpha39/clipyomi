# JParser Phase 4B — Per-Pixel Background Alpha Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the main window a see-through background the user can tune with a slider, while the sentence, its furigana and every gloss stay fully opaque.

**Architecture:** The window is transparent permanently and the alpha lives entirely in CSS — one `--surface-alpha` token multiplied into a derived `--color-bg-surface`, which three window-surface rules use and the right-click menu deliberately does not. The `native` chrome mode clamps the effective alpha to 100 % through a pure function, without ever rewriting the stored value. On Windows the one piece of Rust that has to change is the `WM_ERASEBKGND` fill, which paints a background behind a webview that is now see-through.

**Tech Stack:** Rust + Tauri 2 (`src-tauri`), vanilla TypeScript + Vite (`src`), Vitest, Playwright, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-08-26-jparser-phase4b-design.md`

## Global Constraints

- **At 100 % alpha the app must render and behave exactly as it does today, on every platform** (spec §2, §5.4). `e2e/panes.spec.ts`, `e2e/popover.spec.ts` and `e2e/chrome.spec.ts` must pass with **zero screenshot baselines regenerated**. A baseline that needs refreshing is a regression, not a rebase — stop and find out why.
- **Playwright cannot prove the Windows half of that invariance.** The e2e suite runs the frontend in plain Chromium, so `transparent: true` has no effect there at all. Playwright covers the CSS swap; the Win32 erase path and the DWM blur-behind are covered only by Task 1's spike and Task 10's manual list. Never report the manual items as done on the strength of a green e2e run.
- Untouched by every task: `peek_titlebar`, `set_strip_shown`, `apply_round_region`, `apply_dwm_attributes`, `WM_NCHITTEST`, `WM_NCCALCSIZE`, `WM_WINDOWPOSCHANGING`, `apply_decorations_macos`, `set_titlebar_chrome`, `set_chrome_mode`, `resolve_chrome_mode`, `supported`, `native_supported`, `normalize`, `PEEK_HIDE_MS`, `BAND_HEIGHT`, `--band-h`, `peek_grows_frame`.
- `--color-bg` stays **opaque** in all three theme blocks of `tokens.css` — the bare `:root`, the `prefers-color-scheme: dark` branch, and `:root[data-theme='dark']`. Nothing in this phase edits those three blocks' colour values.
- `.context-menu` (`src/styles/global.css:510`) and every rule in `src/styles/settings.css` keep the **opaque** `--color-bg`. The menu is the app's only reach in content mode, and at 0 % alpha it is the only reach at all.
- The setting is a **percentage** (`0..100`) everywhere a human sees it — `settings.json`, the slider, the `<output>` — and a **ratio** (`0..1`) only inside the CSS token. `surfaceAlphaVar` is the single place that converts.
- The floor is `0`, deliberately, and is not to be raised (spec §4.3).
- Rust source files start with the existing GPL v2 header comment; new Rust files copy it verbatim from a sibling. **TypeScript files carry no header** — `font-sizes.ts`, `history.ts` and `context-menu.ts` all start straight at the first `import` or `export`. Match the neighbours.
- Commit messages: Conventional Commits (`feat`, `fix`, `docs`, `test`, `refactor`, `chore`), subject ≤ 50 chars, body in normal prose explaining *why*.
- Run `npm test` (Vitest), `npm run build` (tsc + vite), and from `src-tauri`: `cargo test` and `cargo clippy --all-targets`, before every commit. `cargo clippy` has four pre-existing `upper_case_acronyms` warnings on `RECT`, `STYLESTRUCT`, `SUBCLASSPROC`, `WINDOWPOS`; those four are the only ones allowed.
- `.gitignore` has an uncommitted local change. Never `git add .` or `git add -A` — stage explicit paths only.

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `src/surface-alpha.ts` | **create** — `ALPHA_LIMITS`, `clampAlpha`, `surfaceAlphaVar` (pure) | 2 |
| `src/surface-alpha.test.ts` | **create** | 2 |
| `src/styles/tokens.css` | modify — `--surface-alpha`, `--color-bg-surface` | 3 |
| `src/styles/global.css` | modify — three surface rules swap tokens; `.context-menu` does not | 3 |
| `src-tauri/src/settings.rs` | modify — `surface_alpha: Option<u32>` and its tests | 4 |
| `src/types.ts` | modify — `surface_alpha?: number` on `Settings` | 4 |
| `src-tauri/src/commands.rs` | modify — `save_settings` merge arm (4); `set_window_background`'s `opaque` and the `WM_ERASEBKGND` gate (6) | 4, 6 |
| `src/main.ts` | modify — applies the token and the clamp (5); sends `opaque` (6) | 5, 6 |
| `src/main.test.ts` | modify — the clamp contract (5); the payload shape (6) | 5, 6 |
| `src/settings.ts` | modify — the slider, the `broadcast` split, the disabled rule | 7 |
| `src/styles/settings.css` | modify — `.settings-range-row` | 7 |
| `src/settings.test.ts` | modify — slider behaviour | 7 |
| `src-tauri/tauri.conf.json` | modify — `"transparent": true`, `"macOSPrivateApi": true` | 8 |
| `src-tauri/Cargo.toml` | modify — `tauri` gains `macos-private-api` | 8 |
| `e2e/chrome.spec.ts` | modify — alpha reaches the background, the menu does not fade, native clamps | 9 |
| `docs/superpowers/specs/2026-08-12-jparser-port-design.md` | modify — phase table, §7.5, deviations | 10 |
| `docs/superpowers/2026-08-26-jparser-phase4b-handoff.md` | **create** | 10 |

---

## Task 1: The spike — does the erase fill survive transparency?

Spec §5.3. **Throwaway.** Its output is an answer, not code. Everything this task writes is reverted before Task 2 starts; only the recorded finding survives, and it decides how Task 6's comment is worded and whether Task 10 records a known limitation.

Run on **Windows**. If you are not on Windows, stop and say so — this task cannot be faked, and Task 6 ships the same gate either way, so the correct fallback is to skip to Task 2 and record in the handoff that the spike never ran.

**Files:**
- Modify (temporarily, reverted in Step 6): `src-tauri/tauri.conf.json`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: a finding, written into the scratch note in Step 5. No code.

- [ ] **Step 1: Turn the window transparent**

In `src-tauri/tauri.conf.json`, add `"transparent": true` to the single object in `app.windows`, next to `"shadow": false`.

- [ ] **Step 2: Make the erase fill impossible to miss**

In `src-tauri/src/commands.rs`, inside the `win32_frameless` module, find the `BACKGROUND` static and force it to magenta. The existing `set_background` writes to it, so override it in the `WM_ERASEBKGND` arm instead — replace the brush line:

```rust
                    let brush = CreateSolidBrush(BACKGROUND.load(Ordering::Relaxed));
```

with:

```rust
                    // SPIKE ONLY — reverted in Step 6. 0x00BBGGRR, so this is
                    // magenta: a colour the page never renders, which is how
                    // `commands.rs`'s own ponytail comment proved this fill
                    // lands on an opaque window.
                    let brush = CreateSolidBrush(0x00FF_00FF);
```

- [ ] **Step 3: Run the app**

```bash
npm run tauri dev
```

- [ ] **Step 4: Make the three observations**

1. **Does the fill still land?** Drag the window's bottom-right corner to resize it, quickly, several times. Watch the client area during the drag.
   - Magenta flickers → the fill lands under DWM blur-behind.
   - Never any magenta → the fill is dead; GDI wrote alpha zero and the surface is showing through.
2. **Are the round corners intact?** Compare the four corners against the app running on `develop` without transparency. Look for square corners, hard edges, or a returned drop shadow.
3. **Does the header peek still reveal cleanly?** Switch to header mode, move the pointer into the window ten times, and watch the top band appear. Any desktop-coloured strip flashing above the band is the 4A glitch resurfacing.

- [ ] **Step 5: Write the finding down**

Create `docs/superpowers/2026-08-26-phase4b-spike-notes.md` with, in plain prose: which of the two branches observation 1 landed on, whether the corners changed, how many of the ten peeks flashed, and the Windows build you ran on. Three or four sentences. This file is committed and referenced by Task 10's handoff.

- [ ] **Step 6: Revert every code change**

```bash
git checkout -- src-tauri/tauri.conf.json src-tauri/src/commands.rs
git status --short
```

Expected: only `.gitignore` (the pre-existing local change) and the new untracked spike note. If `tauri.conf.json` or `commands.rs` still show as modified, the revert did not take — fix it before continuing, because Task 8 is where transparency is turned on for real.

- [ ] **Step 7: Commit the note**

```bash
git add docs/superpowers/2026-08-26-phase4b-spike-notes.md
git commit -m "docs(phase4b): record the transparency spike finding"
```

---

## Task 2: `surface-alpha.ts` — the clamp and the token

Spec §4.2, §4.3, §4.4. Pure addition — nothing imports it yet, so this task cannot change behaviour and is reviewable on its own.

**Files:**
- Create: `src/surface-alpha.ts`
- Create: `src/surface-alpha.test.ts`

**Interfaces:**
- Consumes: `ChromeMode` from `./types` (already exists — `'native' | 'header' | 'content'`).
- Produces:
  - `export const ALPHA_LIMITS: { readonly min: 0; readonly max: 100 }`
  - `export function clampAlpha(value: unknown): number | null`
  - `export function surfaceAlphaVar(alpha: number | null, chrome: ChromeMode | undefined): [string, string]`

- [ ] **Step 1: Write the failing tests**

Create `src/surface-alpha.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { ALPHA_LIMITS, clampAlpha, surfaceAlphaVar } from './surface-alpha';

describe('clampAlpha', () => {
  test('pins a value into range', () => {
    expect(clampAlpha(-20)).toBe(ALPHA_LIMITS.min);
    expect(clampAlpha(140)).toBe(ALPHA_LIMITS.max);
    expect(clampAlpha(62)).toBe(62);
  });

  test('rounds a fractional value', () => {
    expect(clampAlpha(61.6)).toBe(62);
  });

  // settings.json is hand-editable, so a string or a NaN is a real input, not
  // a hypothetical one.
  test('rejects anything that is not a finite number', () => {
    expect(clampAlpha('62')).toBeNull();
    expect(clampAlpha(Number.NaN)).toBeNull();
    expect(clampAlpha(Number.POSITIVE_INFINITY)).toBeNull();
    expect(clampAlpha(undefined)).toBeNull();
  });

  // ta-old clamped uniform alpha at 20/255 because zero took the text with it.
  // Per-pixel alpha does not, so the floor moves to zero (spec §4.3).
  test('the floor is zero', () => {
    expect(ALPHA_LIMITS.min).toBe(0);
    expect(clampAlpha(0)).toBe(0);
  });
});

describe('surfaceAlphaVar', () => {
  test('turns a percentage into the ratio the stylesheet wants', () => {
    expect(surfaceAlphaVar(62, 'header')).toEqual(['--surface-alpha', '0.62']);
    expect(surfaceAlphaVar(0, 'content')).toEqual(['--surface-alpha', '0']);
    expect(surfaceAlphaVar(100, 'header')).toEqual(['--surface-alpha', '1']);
  });

  // Same convention as `fontVars`: the caller removes the property, and
  // `tokens.css` owns the default again. Writing '1' would pin an inline style
  // the stylesheet could never override.
  test('an unset alpha empties the value', () => {
    expect(surfaceAlphaVar(null, 'header')).toEqual(['--surface-alpha', '']);
    expect(surfaceAlphaVar(null, undefined)).toEqual(['--surface-alpha', '']);
  });

  // The clamp. `titleBarStyle: "Overlay"` means the OS draws a solid caption in
  // this mode, and a see-through body under it is an artifact, not a design.
  test('native forces full opacity whatever the stored value', () => {
    expect(surfaceAlphaVar(0, 'native')).toEqual(['--surface-alpha', '1']);
    expect(surfaceAlphaVar(40, 'native')).toEqual(['--surface-alpha', '1']);
    expect(surfaceAlphaVar(null, 'native')).toEqual(['--surface-alpha', '1']);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test -- src/surface-alpha.test.ts
```

Expected: FAIL — `Failed to resolve import "./surface-alpha"`.

- [ ] **Step 3: Write the implementation**

Create `src/surface-alpha.ts`:

```ts
import type { ChromeMode } from './types';

/**
 * Bounds for the opacity control.
 *
 * The floor is 0, not ta-old's 20/255: that clamp existed because uniform
 * whole-window alpha at zero took the text with it, and per-pixel alpha does
 * not — at 0 % the sentence, its furigana and the chips are all still fully
 * opaque.
 *
 * These are the guard, not just the `min`/`max` on the input: the settings
 * file is hand-editable, so every value arriving from it is untrusted and
 * passes through `clampAlpha` before it reaches a stylesheet.
 */
export const ALPHA_LIMITS = { min: 0, max: 100 } as const;

/** The token the alpha is bound to. */
const TOKEN = '--surface-alpha';

/** Percent to the 0..1 ratio the stylesheet wants. */
const PERCENT = 100;

/** VALUE pinned into range, or `null` if it is not a finite number. */
export function clampAlpha(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return Math.min(ALPHA_LIMITS.max, Math.max(ALPHA_LIMITS.min, Math.round(value)));
}

/**
 * The custom property ALPHA asks for under CHROME, as a `[token, value]` pair.
 *
 * `native` forces full opacity whatever the stored value: the OS draws a solid
 * title bar in that mode, and a see-through body underneath it is a rendering
 * artifact. The stored value is never rewritten, so leaving native restores
 * exactly what the user chose.
 *
 * An unset alpha produces an empty value rather than `'1'`, matching
 * `fontVars`: the caller removes the property and `tokens.css` owns the
 * default again. This module never restates a default of its own.
 */
export function surfaceAlphaVar(
  alpha: number | null,
  chrome: ChromeMode | undefined,
): [string, string] {
  if (chrome === 'native') return [TOKEN, '1'];
  if (alpha === null) return [TOKEN, ''];
  return [TOKEN, String(alpha / PERCENT)];
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npm test -- src/surface-alpha.test.ts
```

Expected: PASS, 7 tests.

- [ ] **Step 5: Run the full gate and commit**

```bash
npm test
npm run build
git add src/surface-alpha.ts src/surface-alpha.test.ts
git commit -m "feat(alpha): add the surface-alpha clamp module"
```

---

## Task 3: The token and the three surface rules

Spec §3. CSS only. At the default `--surface-alpha: 1` this changes nothing anyone can see, which is exactly what the existing screenshot baselines are here to prove.

**Files:**
- Modify: `src/styles/tokens.css`
- Modify: `src/styles/global.css:31`, `:88`, `:234` (and deliberately **not** `:510`)

**Interfaces:**
- Consumes: nothing.
- Produces: the CSS custom properties `--surface-alpha` (default `1`) and `--color-bg-surface`, both readable through `getComputedStyle(document.documentElement)`.

- [ ] **Step 1: Add the two tokens**

In `src/styles/tokens.css`, in the **bare `:root` block only**, immediately after the `--color-shadow` line:

```css
  /* Window background opacity (§7.5), as a ratio. `main.ts` overrides it on
     the root element; nothing else does, which is what keeps the settings
     window and the popover opaque without either of them needing a guard. */
  --surface-alpha: 1;
  /* The see-through form of the background, derived once so it follows every
     theme branch without being restated in any of them. `color-mix`
     interpolates premultiplied, so this moves the alpha channel and leaves the
     RGB exactly as authored. `--color-bg` itself stays opaque: `.context-menu`
     uses it, and that menu is the app's only reach in content mode. */
  --color-bg-surface: color-mix(
    in srgb,
    var(--color-bg) calc(var(--surface-alpha) * 100%),
    transparent
  );
```

Do **not** add either token to the `@media (prefers-color-scheme: dark)` block or the `:root[data-theme='dark']` block. `--color-bg-surface` references `--color-bg`, so it re-resolves in each theme on its own.

- [ ] **Step 2: Swap the page surface**

In `src/styles/global.css`, the `html, body` rule at line 28:

```css
html,
body {
  margin: 0;
  background: var(--color-bg-surface);
  color: var(--color-text);
}
```

- [ ] **Step 3: Swap the Windows peeked band**

In `src/styles/global.css`, the rule at line 87:

```css
#app[data-platform='windows']:not([data-chrome='native']).peeked .controls {
  background: var(--color-bg-surface);
  border-bottom: 1px solid var(--color-rule);
}
```

- [ ] **Step 4: Swap the macOS/Linux peeked band**

In `src/styles/global.css`, the rule at line 232:

```css
#app:not([data-platform='windows']):not([data-chrome='native']).peeked .controls {
  height: var(--band-h);
  background: var(--color-bg-surface);
  border-bottom: 1px solid var(--color-rule);
}
```

- [ ] **Step 5: Confirm the menu was left alone**

```bash
grep -n "color-bg" src/styles/global.css
```

Expected: exactly four hits — three now reading `--color-bg-surface` (lines 31, 88, 234) and one still reading `--color-bg`, inside `.context-menu`. If `.context-menu` reads `--color-bg-surface`, the escape hatch is gone; revert that line.

- [ ] **Step 6: Run the invariance gate**

```bash
npx playwright test
```

Expected: every spec passes with **no baseline regenerated**. If any screenshot diff appears, `color-mix` is resolving to something other than the authored colour at alpha 1 — stop, do not update the baseline, and investigate. This step is the only automated proof that the token swap is invisible at 100 %.

- [ ] **Step 7: Run the full gate and commit**

```bash
npm test
npm run build
git add src/styles/tokens.css src/styles/global.css
git commit -m "feat(alpha): derive a see-through background token"
```

---

## Task 4: `surface_alpha` persists

Spec §4.1, §7.1.1. Backend and type only — nothing reads the value yet.

**Files:**
- Modify: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/commands.rs` (the `save_settings` body and its tests)
- Modify: `src/types.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - Rust: `Settings::surface_alpha: Option<u32>`, serialized as `"surface_alpha"`, omitted when `None`.
  - TypeScript: `Settings.surface_alpha?: number`.

- [ ] **Step 1: Write the failing Rust tests**

Append to `mod tests` in `src-tauri/src/settings.rs`:

```rust
    /// `Option` for the same reason the font sizes are: "never set" has to
    /// stay distinguishable from a value that happens to equal the stylesheet
    /// default, which is what keeps `tokens.css` the one place a default lives.
    #[test]
    fn surface_alpha_round_trips_through_the_file() {
        let path = scratch("surface-alpha").join("settings.json");
        let written = Settings {
            surface_alpha: Some(62),
            ..Default::default()
        };

        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        assert_eq!(read_back.surface_alpha, Some(62));
        assert!(warning.is_none(), "got {warning:?}");
    }

    #[test]
    fn an_unset_surface_alpha_is_left_out_of_the_file() {
        let path = scratch("surface-alpha-unset").join("settings.json");
        save(&path, &Settings::default()).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("surface_alpha"), "got {raw}");
    }

    /// Zero is a real, storable setting — not a missing value. Per-pixel alpha
    /// leaves the text opaque, so a 0 % background is still a usable window.
    #[test]
    fn a_zero_surface_alpha_survives_the_file() {
        let path = scratch("surface-alpha-zero").join("settings.json");
        let written = Settings {
            surface_alpha: Some(0),
            ..Default::default()
        };

        save(&path, &written).expect("save");
        assert_eq!(load(&path).0.surface_alpha, Some(0));
    }
```

Also add one line to the existing `defaults_leave_every_optional_key_unset` test, next to the other `assert_eq!(s.…, None)` lines:

```rust
        assert_eq!(s.surface_alpha, None);
```

- [ ] **Step 2: Write the failing `save_settings` test**

Append to `mod tests` in `src-tauri/src/commands.rs`, next to `save_settings_updates_gloss_filters_in_settings_state`:

```rust
    /// The guard that separates alpha from the font sizes. The sizes are
    /// assigned unguarded, deliberately, so that clearing a box restores the
    /// stylesheet default. A range input cannot be cleared, so alpha has no
    /// "absent means reset" state to express, and an unguarded assign would
    /// only hand a future caller a way to wipe the value by omission.
    #[test]
    fn save_settings_leaves_a_stored_alpha_alone_when_the_payload_omits_it() {
        let dir = scratch("save-settings-alpha");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings {
                surface_alpha: Some(40),
                ..Default::default()
            },
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));

        save_settings(
            Settings {
                surface_alpha: None,
                ..Default::default()
            },
            app.state::<Arc<SettingsState>>(),
        )
        .unwrap();

        assert_eq!(state.snapshot().surface_alpha, Some(40));
    }

    #[test]
    fn save_settings_writes_a_supplied_alpha() {
        let dir = scratch("save-settings-alpha-set");
        let state = Arc::new(SettingsState::new(
            dir.join("settings.json"),
            Settings::default(),
        ));
        let app = tauri::test::mock_app();
        app.manage(Arc::clone(&state));

        save_settings(
            Settings {
                surface_alpha: Some(0),
                ..Default::default()
            },
            app.state::<Arc<SettingsState>>(),
        )
        .unwrap();

        assert_eq!(state.snapshot().surface_alpha, Some(0));
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test 2>&1 | head -40; cd ..
```

Expected: compile errors — `struct Settings has no field named surface_alpha`.

- [ ] **Step 4: Add the field**

In `src-tauri/src/settings.rs`, in the `Settings` struct, immediately after the `furigana_scale` field:

```rust
    /// Window background opacity as a percentage (§7.5). `None` leaves
    /// `tokens.css` owning the default. Not clamped here: out-of-range values
    /// from a hand-edited file are pinned at apply time by `clampAlpha`,
    /// exactly as the font sizes are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_alpha: Option<u32>,
```

In `impl Default for Settings`, next to `furigana_scale: None`:

```rust
            surface_alpha: None,
```

Two existing tests build a `Settings` literal exhaustively rather than with `..Default::default()` — `settings_roundtrips_furigana_mode` and `settings_roundtrips_gloss_filters`. Add `surface_alpha: None,` to both, after their `furigana_scale: None,` line, or they will not compile.

- [ ] **Step 5: Add the merge arm**

In `src-tauri/src/commands.rs`, inside `save_settings`'s `state.update(|s| { … })` closure, immediately after the `hide_usage` arm and **before** the three unguarded font-size assignments:

```rust
            // Merged like the toggles above rather than assigned like the
            // sizes below: a range input cannot be cleared, so alpha has no
            // "absent means reset" state, and an unguarded assign would only
            // give a future caller a way to wipe the value by omission.
            if settings.surface_alpha.is_some() {
                s.surface_alpha = settings.surface_alpha;
            }
```

- [ ] **Step 6: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test 2>&1 | tail -20; cd ..
```

Expected: PASS. The `cargo test` total rises by five against the 4A gate's 83.

- [ ] **Step 7: Add the TypeScript type**

In `src/types.ts`, in the `Settings` interface, immediately after `furigana_scale`:

```ts
  /** Window background opacity as a percentage, bound to `--surface-alpha`. */
  surface_alpha?: number;
```

- [ ] **Step 8: Run the full gate and commit**

```bash
npm test
npm run build
cd src-tauri && cargo test && cargo clippy --all-targets 2>&1 | tail -20; cd ..
git add src-tauri/src/settings.rs src-tauri/src/commands.rs src/types.ts
git commit -m "feat(alpha): persist surface_alpha"
```

---

## Task 5: The main window applies the token

Spec §4.4, §4.6. The clamp contract becomes real here. This is the phase's most important task to get right.

**Files:**
- Modify: `src/main.ts:289` (state), `:370` (`applyChromeMode`), `:518` (near `applyFontSizes`), `:537` (`applySettings`), `:564` (the listener)
- Modify: `src/main.test.ts`

**Interfaces:**
- Consumes: `clampAlpha`, `surfaceAlphaVar` from `./surface-alpha` (Task 2); `Settings.surface_alpha` (Task 4).
- Produces: `--surface-alpha` present on `document.documentElement`'s inline style whenever a value is set or the mode is `native`; absent otherwise.

- [ ] **Step 1: Write the failing tests**

Append to `src/main.test.ts`, inside the same `describe` that holds the `load(mode, peekGrowsFrame, platformName)` helper at line 483. These tests need a settings payload carrying an alpha, which that helper does not send, so they use their own loader:

```ts
  /** Like `load`, but with a `surface_alpha` in the settings snapshot. */
  function loadWithAlpha(mode: ChromeMode, alpha?: number) {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings')
        return Promise.resolve({ chrome_mode: mode, surface_alpha: alpha });
      if (cmd === 'platform') return Promise.resolve('macos');
      if (cmd === 'is_macos') return Promise.resolve(true);
      if (cmd === 'peek_grows_frame') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    vi.resetModules();
    return import('./main');
  }

  function alphaToken(): string {
    return document.documentElement.style.getPropertyValue('--surface-alpha');
  }

  test('a persisted alpha reaches the root element as a ratio', async () => {
    await loadWithAlpha('header', 62);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('0.62');
  });

  // Zero is a real setting, and `''` would mean "unset" — the two must not
  // collapse into each other on the way to the stylesheet.
  test('a zero alpha is written, not treated as unset', async () => {
    await loadWithAlpha('content', 0);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('0');
  });

  test('an absent alpha leaves the property off so the stylesheet owns it', async () => {
    await loadWithAlpha('header', undefined);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('');
  });

  // The clamp, half one: an opaque OS caption over a see-through body is an
  // artifact, so native renders fully opaque whatever is stored.
  test('native chrome clamps the applied alpha to full opacity', async () => {
    await loadWithAlpha('native', 20);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('1');
  });

  // The clamp, half two, and the reason it is applied rather than persisted:
  // the user's value has to come back when they leave native.
  test('leaving native restores the stored alpha', async () => {
    await loadWithAlpha('native', 20);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('1');

    emit('settings-changed', { chrome_mode: 'header' });
    await Promise.resolve();
    expect(alphaToken()).toBe('0.2');
  });

  test('an alpha arriving by event is applied without a reload', async () => {
    await loadWithAlpha('header', 100);
    await Promise.resolve();
    await Promise.resolve();

    emit('settings-changed', { surface_alpha: 35 });
    await Promise.resolve();
    expect(alphaToken()).toBe('0.35');
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test -- src/main.test.ts
```

Expected: FAIL — the six new tests all report `''` (or `'1'` never appearing), because nothing writes the property yet.

- [ ] **Step 3: Import the module and add the state**

In `src/main.ts`, next to the existing `import { clampFont, fontVars } from './font-sizes';` (line 18):

```ts
import { clampAlpha, surfaceAlphaVar } from './surface-alpha';
```

Immediately after `let chromeMode: ChromeMode = 'native';` (line 289):

```ts
/**
 * The persisted background opacity as a percentage, or `null` while none is
 * persisted and `tokens.css` owns it. Stored raw: the `native` clamp is
 * applied on the way to the stylesheet, never written back here, so leaving
 * native restores what the user chose.
 */
let currentAlpha: number | null = null;
```

- [ ] **Step 4: Add the apply function**

In `src/main.ts`, immediately after `applyFontSizes` (which ends around line 530):

```ts
/**
 * Push the background-opacity token onto the root element, clamped by the
 * chrome mode.
 *
 * Separate from `applyFontSizes` because it has a second input — the mode —
 * which is also why `applyChromeMode` calls it: a mode change is itself a
 * reason for the clamp to flip, and that path carries no alpha of its own.
 */
function applySurfaceAlpha(): void {
  const [token, value] = surfaceAlphaVar(currentAlpha, chromeMode);
  if (value === '') document.documentElement.style.removeProperty(token);
  else document.documentElement.style.setProperty(token, value);
}
```

- [ ] **Step 5: Call it from `applyChromeMode`**

In `src/main.ts`, at the end of `applyChromeMode` (after the `if (mode !== 'header') { … }` block, before the closing brace):

```ts
  // The clamp reads the mode, so it has to be re-evaluated whenever the mode
  // moves — `native` renders opaque whatever `currentAlpha` holds.
  applySurfaceAlpha();
```

- [ ] **Step 6: Call it from `applySettings`**

In `src/main.ts`'s `applySettings`, set `currentAlpha` **before** the `chrome_mode` branch, because `applyChromeMode` now reads it:

```ts
  const settings = await invoke<Settings>('get_settings');
  currentAlpha = clampAlpha(settings.surface_alpha);
  if (settings.chrome_mode !== undefined) {
    applyChromeMode(settings.chrome_mode);
  }
```

and add one line at the very end of the function, after `applyFontSizes(settings);`:

```ts
  // Unconditional: a settings file with no `chrome_mode` never reaches
  // `applyChromeMode`, and the token still has to be written.
  applySurfaceAlpha();
```

- [ ] **Step 7: Call it from the `settings-changed` listener**

In `src/main.ts`'s `listen<Settings>('settings-changed', …)` handler, immediately after the `applyFontSizes(settings);` line:

```ts
  if (settings.surface_alpha !== undefined) {
    currentAlpha = clampAlpha(settings.surface_alpha);
    applySurfaceAlpha();
  }
```

Leave the existing `if (settings.chrome_mode !== undefined) applyChromeMode(settings.chrome_mode);` line where it is — it now re-applies the clamp on its own, and `applySurfaceAlpha` is idempotent, so the two together are harmless.

- [ ] **Step 8: Run the tests to verify they pass**

```bash
npm test -- src/main.test.ts
```

Expected: PASS. If an unrelated existing test fails on an `invoke` call count, read it before changing it — this task adds no new `invoke` calls, so a count change means something else moved.

- [ ] **Step 9: Run the full gate and commit**

```bash
npm test
npm run build
git add src/main.ts src/main.test.ts
git commit -m "feat(alpha): apply the alpha token and its clamp"
```

---

## Task 6: `opaque` end to end

Spec §5.4. The Rust argument and the frontend that supplies it ship together: adding the parameter alone would make every existing `set_window_background` call fail deserialization on a missing field, and the call site swallows errors, so the failure would be silent.

**Files:**
- Modify: `src-tauri/src/commands.rs` (`set_window_background`, `win32_frameless::set_background`, the `WM_ERASEBKGND` arm)
- Modify: `src/main.ts` (`reportBackground`, and the three `applySurfaceAlpha` callers)
- Modify: `src/main.test.ts:694`

**Interfaces:**
- Consumes: `surfaceAlphaVar` (Task 2), `currentAlpha` and `chromeMode` (Task 5).
- Produces: `set_window_background(r: u8, g: u8, b: u8, opaque: bool)`. The frontend always sends all four.

- [ ] **Step 1: Update the failing frontend test**

In `src/main.test.ts`, line 694 currently reads:

```ts
      expect(invoke).toHaveBeenCalledWith('set_window_background', { r: 18, g: 18, b: 18 });
```

Replace it with:

```ts
      // `opaque` is the *effective* alpha, not the stored one: this fixture
      // persists none, so the stylesheet default of 1 applies and the Windows
      // erase fill stays enabled.
      expect(invoke).toHaveBeenCalledWith('set_window_background', {
        r: 18,
        g: 18,
        b: 18,
        opaque: true,
      });
```

Then append, in the same `describe` as Task 5's tests:

```ts
  test('a see-through surface tells the backend to stop filling', async () => {
    await loadWithAlpha('header', 40);
    await Promise.resolve();
    await Promise.resolve();

    const calls = invoke.mock.calls.filter(([cmd]) => cmd === 'set_window_background');
    expect(calls.length).toBeGreaterThan(0);
    expect(calls[calls.length - 1][1]).toMatchObject({ opaque: false });
  });

  // Native renders fully opaque whatever is stored, and that is exactly the
  // mode whose erase fill stops the top-edge flash. Reading the stored value
  // here would switch the fill off in the one mode that needs it.
  test('native keeps the fill enabled despite a low stored alpha', async () => {
    await loadWithAlpha('native', 20);
    await Promise.resolve();
    await Promise.resolve();

    const calls = invoke.mock.calls.filter(([cmd]) => cmd === 'set_window_background');
    expect(calls[calls.length - 1][1]).toMatchObject({ opaque: true });
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test -- src/main.test.ts
```

Expected: FAIL — the payload has no `opaque` key.

- [ ] **Step 3: Send `opaque` from the frontend**

In `src/main.ts`, replace the body of `reportBackground`:

```ts
function reportBackground(): void {
  const rendered = getComputedStyle(document.body).backgroundColor;
  const rgb = resolveColor(rendered);
  if (rgb === null) return;
  // The *effective* alpha, not the stored one: `native` renders fully opaque
  // whatever `surface_alpha` holds, and that is precisely the mode whose erase
  // fill stops the top-edge flash. An empty value means unset, which the
  // stylesheet resolves to 1.
  const [, effective] = surfaceAlphaVar(currentAlpha, chromeMode);
  const opaque = effective === '' || effective === '1';
  void invoke('set_window_background', {
    r: rgb[0],
    g: rgb[1],
    b: rgb[2],
    opaque,
  }).catch(() => {});
}
```

- [ ] **Step 4: Re-report whenever the effective alpha moves**

In `src/main.ts`, add one line at the end of `applySurfaceAlpha` (from Task 5):

```ts
  // `reportBackground` runs at startup and on an OS theme change today. The
  // erase gate rides on the effective alpha too, so it has to follow both of
  // this function's inputs. Sending it unconditionally is cheaper than
  // tracking whether the value crossed the opaque boundary.
  reportBackground();
```

- [ ] **Step 5: Run the frontend tests to verify they pass**

```bash
npm test -- src/main.test.ts
```

Expected: PASS.

- [ ] **Step 6: Take the argument in Rust**

In `src-tauri/src/commands.rs`, replace `set_window_background`:

```rust
/// The webview reporting the background colour it renders, and whether that
/// background is fully opaque.
///
/// The colour is so the frame growth on a peek erases newly exposed pixels in
/// what the page draws rather than in a colour guessed from the system theme.
/// `opaque` is the *effective* alpha — `native` renders fully opaque whatever
/// is persisted — and gates the fill entirely, for the reason the
/// `WM_ERASEBKGND` arm gives.
#[tauri::command]
pub fn set_window_background(r: u8, g: u8, b: u8, opaque: bool) {
    let color = colorref(r, g, b);
    #[cfg(target_os = "windows")]
    win32_frameless::set_background(color, opaque);
    let _ = (color, opaque);
}
```

- [ ] **Step 7: Store it**

In `src-tauri/src/commands.rs`, inside the `win32_frameless` module, next to the existing `STRIP_SHOWN` static:

```rust
    /// Whether the page's background is fully opaque. Starts `true`: the
    /// window is opaque until the renderer says otherwise, which is the same
    /// direction `BACKGROUND`'s theme guess errs in.
    static BACKGROUND_OPAQUE: AtomicBool = AtomicBool::new(true);
```

and extend `set_background`:

```rust
    pub fn set_background(colorref: u32, opaque: bool) {
        BACKGROUND.store(colorref, Ordering::Relaxed);
        BACKGROUND_OPAQUE.store(opaque, Ordering::Relaxed);
    }
```

- [ ] **Step 8: Gate the fill**

In `src-tauri/src/commands.rs`, in `frameless_subclass_proc`'s `WM_ERASEBKGND` arm, insert before the `GetClientRect` call:

```rust
                // Skipped once the surface is see-through. `CreateSolidBrush`
                // takes a COLORREF, which carries no alpha, so GDI writes zero
                // into the alpha byte — and under the blur-behind DWM applies
                // to a transparent window, zero is fully transparent. Filling
                // would paint a hole rather than a background.
                if !BACKGROUND_OPAQUE.load(Ordering::Relaxed) {
                    return DefSubclassProc(hwnd, msg, wparam, lparam);
                }
```

- [ ] **Step 9: Run the Rust gate**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets 2>&1 | tail -20; cd ..
```

Expected: PASS, and exactly the four pre-existing `upper_case_acronyms` warnings.

- [ ] **Step 10: Run the full gate and commit**

```bash
npm test
npm run build
git add src-tauri/src/commands.rs src/main.ts src/main.test.ts
git commit -m "feat(alpha): gate the Windows erase fill on opacity"
```

---

## Task 7: The slider

Spec §7. The first task a user can see.

**Files:**
- Modify: `src/settings.ts`
- Modify: `src/styles/settings.css`
- Modify: `src/settings.test.ts`

**Interfaces:**
- Consumes: `ALPHA_LIMITS`, `clampAlpha` from `./surface-alpha` (Task 2); `Settings.surface_alpha` (Task 4).
- Produces: `#setting-surface-alpha` (an `<input type="range">`) and `#setting-surface-alpha-value` (an `<output>`) inside the settings window; a `settings-changed` event carrying `{ surface_alpha }` on every `input`, and a full snapshot on every `change`.

- [ ] **Step 1: Write the failing tests**

Append to the `describe('Settings Window', …)` block in `src/settings.test.ts`:

```ts
  /** Boot the settings window with a given snapshot and settle its promises. */
  async function boot(settings: Partial<Settings>) {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();
  }

  function slider(): HTMLInputElement {
    const el = container.querySelector<HTMLInputElement>('#setting-surface-alpha');
    if (el === null) throw new Error('#setting-surface-alpha missing');
    return el;
  }

  test('the slider shows the persisted opacity', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 62 });
    expect(slider().value).toBe('62');
    expect(
      container.querySelector('#setting-surface-alpha-value')?.textContent,
    ).toBe('62%');
  });

  // A range input always has a value, so unlike an empty font box it cannot
  // signal "unset" by being blank -- it is seeded from the stylesheet instead.
  test('with nothing persisted the slider shows the stylesheet default', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true });
    expect(slider().value).toBe('100');
  });

  // A drag fires `input` per frame. Broadcasting the whole snapshot at that
  // rate would re-run the main window's gloss merge and furigana comparison
  // sixty times a second for a value neither of them contains.
  test('dragging broadcasts a partial payload without persisting', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 100 });
    emitted.length = 0;
    invoke.mockClear();

    slider().value = '35';
    slider().dispatchEvent(new Event('input'));

    expect(emitted).toEqual([['settings-changed', { surface_alpha: 35 }]]);
    expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
  });

  test('releasing the slider persists the full snapshot', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 100 });
    invoke.mockClear();

    slider().value = '35';
    slider().dispatchEvent(new Event('change'));

    const save = invoke.mock.calls.find(([cmd]) => cmd === 'save_settings');
    expect(save).toBeDefined();
    expect((save?.[1] as { settings: Settings }).settings.surface_alpha).toBe(35);
  });

  // The native frame renders fully opaque whatever is stored, so an enabled
  // slider there would move nothing.
  test('the slider is disabled in native chrome and enabled on leaving it', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' });
    expect(slider().disabled).toBe(true);

    container.querySelector<HTMLButtonElement>('[data-chrome-mode="content"]')?.click();
    expect(slider().disabled).toBe(false);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test -- src/settings.test.ts
```

Expected: FAIL — `#setting-surface-alpha missing`.

- [ ] **Step 3: Add the markup**

In `src/settings.ts`, in the template literal, immediately after the closing `</section>` of the Window Chrome group:

```html
      <section class="settings-group">
        <h2 class="settings-group-title">Background</h2>
        <label class="settings-range-row">
          <span class="label-text">Opacity</span>
          <input type="range" id="setting-surface-alpha"
                 min="${ALPHA_LIMITS.min}" max="${ALPHA_LIMITS.max}" step="1" />
          <output class="unit" id="setting-surface-alpha-value">100%</output>
        </label>
      </section>
```

Extend the existing import at the top of the file:

```ts
import { ALPHA_LIMITS, clampAlpha } from './surface-alpha';
```

- [ ] **Step 4: Grab the elements and seed them**

In `src/settings.ts`, next to the other `querySelector` lines:

```ts
  const surfaceAlpha = container.querySelector<HTMLInputElement>('#setting-surface-alpha')!;
  const surfaceAlphaValue = container.querySelector<HTMLOutputElement>(
    '#setting-surface-alpha-value',
  )!;

  function showAlpha(value: number) {
    surfaceAlphaValue.textContent = `${value}%`;
  }

  /**
   * Seed the slider from the stylesheet's own opacity.
   *
   * The font boxes detect "nothing persisted" by being empty and fill in the
   * default afterwards. A range input always carries a value — the midpoint,
   * by default — so it has to be seeded up front and overwritten if
   * `get_settings` turns out to carry one.
   */
  function seedAlphaFromStylesheet() {
    const raw = getComputedStyle(document.documentElement).getPropertyValue('--surface-alpha');
    const ratio = Number.parseFloat(raw);
    const seeded = clampAlpha(Number.isFinite(ratio) ? ratio * 100 : ALPHA_LIMITS.max);
    const value = seeded ?? ALPHA_LIMITS.max;
    surfaceAlpha.value = String(value);
    showAlpha(value);
  }

  seedAlphaFromStylesheet();
```

- [ ] **Step 5: Split the broadcast**

In `src/settings.ts`, replace `saveAndBroadcast` with the pair:

```ts
  function broadcast(payload: Partial<Settings>) {
    void emit('settings-changed', payload).catch(() => {});
  }

  function saveAndBroadcast() {
    const snapshot = currentSnapshot();
    void invoke('save_settings', { settings: snapshot }).catch(() => {});
    broadcast(snapshot);
  }
```

- [ ] **Step 6: Put the alpha in the snapshot**

In `src/settings.ts`'s `currentSnapshot`, after `furigana_scale`:

```ts
      surface_alpha: clampAlpha(surfaceAlpha.valueAsNumber) ?? ALPHA_LIMITS.max,
```

- [ ] **Step 7: Wire the two events**

In `src/settings.ts`, next to the other `addEventListener` calls:

```ts
  surfaceAlpha.addEventListener('input', () => {
    const value = clampAlpha(surfaceAlpha.valueAsNumber) ?? ALPHA_LIMITS.max;
    showAlpha(value);
    // A partial payload, not `currentSnapshot()`: this fires once per frame
    // during a drag, and the main window's `settings-changed` listener would
    // otherwise re-run the gloss merge and the furigana comparison — which
    // re-renders the whole sentence — for a value neither of them contains.
    // `main.ts` already emits a bare `{ chrome_mode }` the same way.
    broadcast({ surface_alpha: value });
  });

  surfaceAlpha.addEventListener('change', saveAndBroadcast);
```

- [ ] **Step 8: Disable it in native mode**

In `src/settings.ts`'s `updateChromeButtons`, after the `chromeButtons.forEach(…)` block:

```ts
    // The native frame renders fully opaque whatever is stored, so the control
    // has to say so rather than sit there moving nothing. A disabled control
    // with no stated reason reads as a bug, hence the title.
    surfaceAlpha.disabled = mode === 'native';
    surfaceAlpha.title = mode === 'native' ? 'The native window frame is always fully opaque' : '';
```

In `applySettings`, next to the other field restores:

```ts
    if (s.surface_alpha !== undefined) {
      const value = clampAlpha(s.surface_alpha) ?? ALPHA_LIMITS.max;
      surfaceAlpha.value = String(value);
      showAlpha(value);
    }
```

- [ ] **Step 9: Style the row**

Append to `src/styles/settings.css`:

```css
.settings-range-row {
  display: flex;
  align-items: center;
  gap: 10px;
  font-family: var(--font-ui);
  font-size: 13px;
  color: var(--color-text);
  padding: 2px 0;
}

/* The label takes the slack, so this row's control lines up with the number
   rows above it. */
.settings-range-row .label-text {
  flex: 1;
}

.settings-range-row input[type='range'] {
  width: 120px;
  margin: 0;
  cursor: pointer;
  accent-color: var(--color-kanji);
}

.settings-range-row input[type='range']:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.settings-range-row .unit {
  color: var(--color-muted);
  font-size: var(--text-tag, 11px);
  min-width: 76px;
}
```

- [ ] **Step 10: Run the tests to verify they pass**

```bash
npm test -- src/settings.test.ts
```

Expected: PASS. If `with nothing persisted the slider shows the stylesheet default` fails with `50`, happy-dom is not resolving the imported stylesheet — the `?? ALPHA_LIMITS.max` fallback in `seedAlphaFromStylesheet` is what makes the value `100` in that case, so check that branch rather than weakening the test.

- [ ] **Step 11: Run the full gate and commit**

```bash
npm test
npm run build
git add src/settings.ts src/settings.test.ts src/styles/settings.css
git commit -m "feat(alpha): add the opacity slider"
```

---

## Task 8: Turn the window transparent

Spec §3.3, §6. Everything before this task is inert and correct; this is where the feature arms.

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: everything from Tasks 3–7.
- Produces: a main window whose background actually composites with alpha.

- [ ] **Step 1: Make the window transparent**

In `src-tauri/tauri.conf.json`, in the single object inside `app.windows`, next to `"shadow": false`:

```json
        "transparent": true
```

- [ ] **Step 2: Enable the macOS private API**

In the same file, inside `app`, next to `"security"`:

```json
    "macOSPrivateApi": true
```

- [ ] **Step 3: Enable the matching cargo feature**

In `src-tauri/Cargo.toml`, change the `tauri` dependency line:

```toml
# `macos-private-api` is what `macOSPrivateApi` in tauri.conf.json requires:
# transparency on macOS goes through a private NSWindow API. It rules out Mac
# App Store distribution, which this self-distributed GPL v2 build already
# accepts (spec §7.5).
tauri = { version = "2", features = ["macos-private-api"] }
```

- [ ] **Step 4: Verify the build accepts the pairing**

```bash
cd src-tauri && cargo build 2>&1 | tail -20; cd ..
```

Expected: builds clean. A message about `macOSPrivateApi` requiring a feature means Step 3 did not take.

- [ ] **Step 5: Run the invariance gate**

```bash
npm test
npx playwright test
cd src-tauri && cargo test && cargo clippy --all-targets 2>&1 | tail -20; cd ..
```

Expected: everything passes, **no baseline regenerated**. Remember what this does and does not prove — Playwright runs in plain Chromium, so it says nothing about the Win32 half. That is Task 10's manual list.

- [ ] **Step 6: Look at the real window**

```bash
npm run tauri dev
```

Confirm, at the default 100 % opacity, in header mode: the window looks exactly as it did before this phase, the corners are still round, and a hover reveal still works. Then drag the opacity slider down and watch the background fade while the sentence stays crisp. If the text fades too, the alpha reached the wrong token — stop and check Task 3 Step 5.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "feat(alpha): make the main window transparent"
```

---

## Task 9: Playwright coverage

Spec §9. Assertions go against computed values, not pixels: a see-through page photographed in headless Chromium captures the harness's backdrop rather than the app.

**Files:**
- Modify: `e2e/chrome.spec.ts`

**Interfaces:**
- Consumes: everything shipped in Tasks 3–8.
- Produces: no exports; three tests.

- [ ] **Step 1: Let the stub carry an alpha**

In `e2e/chrome.spec.ts`, replace `stubFor`'s signature and its `get_settings` line so the settings snapshot can carry an opacity:

```ts
function stubFor(
  mode: (typeof MODES)[number],
  platform: 'macos' | 'windows',
  alpha?: number,
): string {
  return `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'platform') return Promise.resolve('${platform}');
    if (cmd === 'is_macos') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'peek_grows_frame') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: '${mode}', surface_alpha: ${alpha ?? 'undefined'} });
    return realInvoke(cmd, args);
  };
`;
}
```

Every existing call site passes two arguments and keeps working — `alpha` is optional and serializes to `undefined`, which is what the previous stub effectively sent.

- [ ] **Step 2: Write the failing tests**

Append to `e2e/chrome.spec.ts`:

```ts
// The assertions below compare computed colours to each other rather than to a
// literal. `color-mix()` with an `oklch()` input serializes in whichever space
// the engine picks, so pinning a string would test Chromium's serializer, not
// the app.
test('the opacity setting reaches the window background', async ({ page }) => {
  await page.addInitScript(stubFor('header', 'macos', 100));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'header');
  const opaque = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);

  await page.addInitScript(stubFor('header', 'macos', 0));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'header');
  const clear = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);

  expect(clear).not.toBe(opaque);
});

// 0% is a real, reachable setting (spec §4.3). It stays escapable because the
// menu never fades: content mode already removes the gear, the history nav and
// the window buttons, so at 0% this menu is the only reach into the app.
test('the right-click menu stays opaque at zero opacity', async ({ page }) => {
  await page.addInitScript(stubFor('content', 'macos', 0));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'content');

  await page.locator('#app').click({ button: 'right' });
  const menu = page.locator('.context-menu');
  await expect(menu).toBeVisible();

  const [menuBg, bodyBg] = await page.evaluate(() => [
    getComputedStyle(document.querySelector('.context-menu')!).backgroundColor,
    getComputedStyle(document.body).backgroundColor,
  ]);
  expect(menuBg).not.toBe(bodyBg);
});

// The clamp, from the outside: a stored 0 renders fully opaque under the
// native frame, and the stored value is not rewritten to say so.
test('native chrome clamps the opacity to fully opaque', async ({ page }) => {
  await page.addInitScript(stubFor('native', 'macos', 0));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'native');

  const token = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--surface-alpha').trim(),
  );
  expect(token).toBe('1');
});
```

- [ ] **Step 3: Run the new tests**

```bash
npx playwright test e2e/chrome.spec.ts
```

Expected: PASS, and no baseline regenerated. If `the right-click menu stays opaque` fails because the menu never opened, check that the right-click target is inside `#app` — `context-menu.ts` binds on `document`, so the click coordinates matter less than the page having loaded.

- [ ] **Step 4: Run the whole suite and commit**

```bash
npm test
npm run build
npx playwright test
git add e2e/chrome.spec.ts
git commit -m "test(alpha): cover the token, the menu and the clamp"
```

---

## Task 10: Close the phase in the docs

Follows 4A's pattern: the master spec is updated and a handoff written once the code is in.

**Files:**
- Modify: `docs/superpowers/specs/2026-08-12-jparser-port-design.md`
- Create: `docs/superpowers/2026-08-26-jparser-phase4b-handoff.md`

**Interfaces:**
- Consumes: the whole phase, plus Task 1's spike note.
- Produces: documentation only.

- [ ] **Step 1: Run the manual checks and write the results down**

On Windows, under `npm run tauri dev`. Record each result — a pass, a fail, or "not run" — for the handoff. Do not skip a line and leave it unmentioned.

1. At 100 % opacity, header mode: the window is indistinguishable from `develop` before this phase.
2. At 100 % opacity: the corners are round and there is no drop shadow.
3. At 100 % opacity, header mode: ten pointer entries, counting how many flash a desktop-coloured strip above the band. Compare against Task 1's spike note.
4. Drag the slider 100 → 0: the background fades, the sentence and its furigana stay fully crisp.
5. At 0 % opacity, content mode: right-click anywhere in the window. The menu appears solid, and Settings opens from it.
6. Switch to `native` (macOS/Linux only — the button is absent on Windows): the window goes fully opaque. Switch back to `header`: the previous opacity returns, and `settings.json`'s `surface_alpha` never changed.

- [ ] **Step 2: Update the master spec**

In `docs/superpowers/specs/2026-08-12-jparser-port-design.md`:

- The `### Phase 4` heading and its intro paragraph: 4B is now shipped, leaving 4D alone.
- The §11 table's **4B** row: change the scope cell to past tense and record that the window is transparent unconditionally with the alpha clamped in `native`, rather than §7.5's `transparent: true` + `decorations: false` pairing.
- §7.5: add a sentence recording that the shipped control's floor is 0 %, not ta-old's 20/255, and why (per-pixel alpha leaves the text opaque).
- The §12 deviations table: extend deviation #4's text with that floor consequence. Add no new numbered row unless the manual checks turned up a behaviour that warrants one.
- The §11 **4D** row: note that a slot must capture the **stored** `surface_alpha`, not the clamped one.

- [ ] **Step 3: Write the handoff**

Create `docs/superpowers/2026-08-26-jparser-phase4b-handoff.md`, following the structure of `docs/superpowers/2026-08-25-jparser-phase4a-handoff.md`: what was completed, the design decisions worth carrying forward, the verification gate with real numbers from the runs, and what 4D inherits. Cover at minimum:

- **The spike's finding**, quoted from `docs/superpowers/2026-08-26-phase4b-spike-notes.md`, and whether the `WM_ERASEBKGND` fill survives blur-behind. If it does not, this is a known limitation with the alternatives named (a premultiplied DIB with `AlphaBlend`, or accepting the residual flash) — not a silent regression.
- **Why `--color-bg` stayed opaque** and `--color-bg-surface` was derived: `.context-menu` is the app's only reach in content mode, and at 0 % alpha it is the only reach at all.
- **The clamp is applied, never persisted**, and 4D's slot must capture the stored value.
- **Windows never needed a platform branch for the clamp**, because `supported()` means it structurally cannot resolve to `native` — 4A's handoff §5 predicted this and was right.
- **What is unverified:** the macOS arms, exactly as 4A left them; and every manual item from Step 1 that did not run.
- **What this phase did not clear:** 4A's manual checklist, `docs/superpowers/2026-08-18-windows-titlebar-test-plan.md`, and 4A's open finding on `set_chrome_mode`'s persist ordering.

Put the actual counts in: Vitest total, Playwright total, `cargo test` total, and confirmation that clippy reported only the four pre-existing `upper_case_acronyms` warnings.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/specs/2026-08-12-jparser-port-design.md docs/superpowers/2026-08-26-jparser-phase4b-handoff.md
git commit -m "docs(phase4b): close the phase and record the deviations"
```

---

## Self-Review

**Spec coverage.** §2 scope → Tasks 2–9; the six out-of-scope rows are enforced by the Global Constraints. §3.1 why `--color-bg` cannot carry the alpha → Task 3 Steps 1, 5. §3.2 the derived token → Task 3 Steps 1–4. §3.3 one transparent window → Task 8 Step 1. §3.4 only the main window sets the property → Task 5 Step 4 (nothing in `settings.ts` writes the token; Task 7 only reads it). §4.1 the setting → Task 4 Steps 1, 4. §4.2 the new module → Task 2. §4.3 the 0 % floor → Task 2 Steps 1, 3, plus Task 4 Step 1's zero test and Task 9 Step 2's menu test. §4.4 the clamp table → Task 2 Step 1, all three rows. §4.5 the stored value survives → Task 5 Step 1's `leaving native restores the stored alpha`. §4.6 where it is applied → Task 5 Steps 5–7, and the `reportBackground` call in Task 6 Step 4. §5.1–5.2 the erase path → Task 6 Step 8's comment. §5.3 the spike → Task 1. §5.4 the gate → Task 6. §5.5 what transparency forecloses → Task 10 Step 3. §6 macOS → Task 8 Steps 2–4. §7 the control → Task 7. §7.1.1 the merge arm → Task 4 Steps 2, 5. §8 files touched → the File Structure table. §9 verification → distributed, plus Task 9. §10 deferred → Task 10 Step 3. §11 deviations → Task 10 Step 2.

**Placeholder scan.** No `TBD`, no "similar to Task N", no "add error handling". Task 10's Steps 2 and 3 describe documentation rather than showing it, which is the same shape 4A's plan used for its own close — the content depends on numbers that do not exist until the runs happen, and the bullets pin what must be covered.

**Type consistency.** `clampAlpha` and `surfaceAlphaVar` keep the same signatures in Tasks 2, 5, 6 and 7. `ALPHA_LIMITS.max` is the fallback in every `?? ` in `settings.ts`. `set_window_background`'s four arguments match between Task 6's Rust (`r, g, b, opaque`) and its TypeScript payload. `surface_alpha` is spelled identically in `settings.rs`, `types.ts`, `settings.ts`, `main.ts`, the e2e stub and the tests. `--surface-alpha` and `--color-bg-surface` are spelled identically in `tokens.css`, `global.css`, `surface-alpha.ts` and the e2e assertions. `BACKGROUND_OPAQUE` appears only in Task 6, in the two places that use it.

**One risk the plan cannot remove.** Task 1's spike may report that the erase fill dies under blur-behind. Task 6 ships the same gate either way, so no task depends on the answer — but if it dies, the residual top-edge flash at 100 % alpha is a real regression against 4A's measured 1 %, and Task 10 Step 3 is where that gets recorded honestly rather than discovered later.
