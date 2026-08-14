# JParser Phase 2I — The Tooltip as a Real Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The word tooltip becomes a real OS window that can extend past the main window onto the desktop, showing every dictionary match in ta-old's colouring.

**Architecture:** Rust creates one hidden `WebviewWindow` labelled `popover` at startup and exposes two commands that position and hide it. TypeScript keeps all the arithmetic: a text assembler and a lexical colouriser (both pure, both ported from `MyDrawText`'s rules rather than from its output), a `placePopover` rewritten against the monitor work area, and a keep rule driven by polling `cursorPosition()` — which is what makes ta-old's direction-of-travel test work in the gap between two webviews where neither sees mouse moves.

**Tech Stack:** Tauri 2 (`@tauri-apps/api` 2.11.1), Vite with two entry points, TypeScript (strict), Vitest + happy-dom, Playwright, plain CSS custom properties. No frontend framework. **Rust changes are in scope this phase** — 2G's "no Rust" rule was a scoping decision for that phase, not an invariant.

**Spec:** `docs/superpowers/specs/2026-08-15-jparser-phase2i-design.md` (authoritative, committed at `dd6ad60`). It supersedes §2, §3.3, §3.4, and §4 of `docs/superpowers/specs/2026-08-14-jparser-phase2g-design.md`. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **No frontend framework.** Vanilla TypeScript and DOM APIs only.
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`.** The colouriser emits `<span>`s and sets `.textContent` on each. This is the rule this phase is most tempting to break — a colourised string is exactly the shape that invites `innerHTML`. Do not.
- **Every colour is defined on bare `:root` first**, in `src/styles/tokens.css`; the `@media (prefers-color-scheme: dark)` and `:root[data-theme='dark']` blocks may only redefine. A new token must appear in **all three** blocks.
- **Only `transform` and `opacity` are animated.** Any `@media (prefers-reduced-motion: reduce)` override must come **after** the base rule it overrides. 2F shipped that exact override dead by placing it before, and no test in either suite caught it.
- **Names are frozen except where this plan adds them.** New this phase: commands `place_popover`, `hide_popover`; events `popover-content`, `popover-measured`. Existing commands `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`, `startup_error`, `settings_warning`, `frontend_ready`, `download_dictionary`, `needs_dictionary` and events `parse-result`, `parse-error`, `dictionary-status` are unchanged.
- **`GAP` = 2, `MARGIN` = 8.** `GAP` drops from 2G's 6 to match ta-old's `rAvoid.top-2`.
- **`DWELL_MS` = 350**, unchanged — ta-old's `dwHoverTime`.
- **File size** 200–400 lines typical, **800 hard maximum** including tests.
- **`npx tsc --noEmit` clean**, `npm run build` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean at the end of every task **except Task 5**, which deliberately leaves `tsc` red at one boundary and says so. `strict` is on: no `any`, no non-null assertion except where a file already uses one for a required shell element.
- **The ten committed screenshot baselines must not change**: `panes-{compact,default}-{light,dark}.png`, `panes-activated-{light,dark}.png`, four `panes-download-*.png`. The two `panes-popover-{light,dark}-darwin.png` from 2G **are deleted** in Task 7 — the DOM surface they cover stops existing.
- **Screenshots run locally only.** Every `toHaveScreenshot` call in `e2e/` is wrapped in `if (!process.env.CI)`. No new baselines this phase: the tooltip is a separate OS window, outside `page` capture.
- **Scope fence.** The definitions pane keeps `renderEntry` and its layout, untouched. Pane density, font sizes, gloss filters, and furigana are Phase 3. Multi-spelling headwords are spec §9, a later phase.

**Invariants this phase must not break:** the definitions pane remains a complete route to a definition; unmatched runs stay non-interactive `<span>`s never in the tab order; chips stay real `<button>`s with Enter/Space activation; `.panes` keeps `overflow-y: auto`; the popup page never imports `main.ts`.

---

## Resolved facts — do not re-derive these

Measured against the tree at `dd6ad60`.

| Fact | Value |
|---|---|
| Rust entry | `src-tauri/src/main.rs`, 140 lines; `.setup(` at `:29`, `.invoke_handler(` at `:125`, `mod` block at `:13-19` |
| Existing modules | `clipboard`, `commands`, `parse`, `settings`, `state`, `test_support` |
| Capability file | `src-tauri/capabilities/default.json` — grants only `core:event:allow-listen`, `"windows": ["main"]` |
| Vite config | `vite.config.ts` — has `build: { target: 'esnext' }`; no `rollupOptions` yet |
| Vitest include | `src/**/*.test.ts` — new tests must live under `src/` to be collected |
| HTML shell | `index.html` at repo root, loads `/src/main.ts` |
| 2G popover module | `src/render/popover.ts`, 89 lines — `placePopover`, `showEntryPopover`, `hideEntryPopover`, module-local element |
| 2G popover CSS | `src/styles/global.css:199-242` — `.entry-popover`, `.entry-popover.open`, reduced-motion block |
| 2G wiring | `src/main.ts` 418 lines — `DWELL_MS`, `lastResult`, `segmentAt`, `chipFrom`, `clearDwell`, `closePopover`, `openFor`, six listeners, `closePopover()` first in `show()` |
| Entry shape | `src/types.ts` — `Entry { headword, reading: string\|null, conjugation: string\|null, pos: string[], senses: Sense[], flags: FlagName[] }`; `Sense { pos, glosses, xrefs, misc, info }` |
| `common` flag | `FlagName` includes `'common'` and `'common_line'` — `(P)` comes from `'common'` |
| Tauri JS APIs confirmed present | `cursorPosition()`, `currentMonitor()`, `monitorFromPoint()`, `Monitor.workArea`, `outerPosition()`, `scaleFactor()` — all exported from `@tauri-apps/api/window` 2.11.1 |
| Baseline counts | Vitest 44, Playwright 24, Rust 357 |

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/popover.rs` | *(new)* creates the popup window; the `place_popover` / `hide_popover` commands |
| `src-tauri/src/main.rs` | *(modified)* registers the module, the setup call, and the two commands |
| `src-tauri/capabilities/popover.json` | *(new)* grants `core:event:allow-listen` to the `popover` label |
| `popover.html` | *(new)* the popup's page shell |
| `vite.config.ts` | *(modified)* two rollup entry points |
| `src/popover.ts` | *(new)* the popup's entry: listens for content, renders, reports its size |
| `src/render/tooltip-text.ts` | *(new)* `assembleTooltipText` — entries to ta-old's text block |
| `src/render/tooltip-text.test.ts` | *(new)* |
| `src/render/tooltip-colour.ts` | *(new)* `colourLine` — the lexical colouriser |
| `src/render/tooltip-colour.test.ts` | *(new)* |
| `src/render/tooltip.ts` | *(new)* `renderTooltip` — assemble, colourise, emit spans |
| `src/render/popover.ts` | *(modified)* `placePopover` rewritten against the work area; `shouldKeep`; the DOM element code deleted |
| `src/render/popover.test.ts` | *(modified)* |
| `src/styles/tokens.css` | *(modified)* seven new tokens in all three blocks |
| `src/styles/tooltip.css` | *(new)* the popup page's styles |
| `src/styles/global.css` | *(modified)* `.entry-popover` rules removed |
| `src/main.ts` | *(modified)* screen-coordinate conversion, keep-rule polling, new dismissals |
| `src/main.test.ts` | *(modified)* |
| `e2e/popover.spec.ts` | *(new)* the popup page's content and scrolling |
| `e2e/panes.spec.ts` | *(modified)* 2G's DOM-popover specs removed |

---

## Task 1: The popup window

**Files:**
- Create: `src-tauri/src/popover.rs`, `src-tauri/capabilities/popover.json`, `popover.html`, `src/popover.ts`
- Modify: `src-tauri/src/main.rs`, `vite.config.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: Tauri commands `place_popover(x: i32, y: i32, width: u32, height: u32)` and `hide_popover()`, both returning `Result<(), String>`; a window labelled `popover` loading `popover.html`; the event name `popover-content`. Tasks 4 and 6 use all of these.

- [ ] **Step 1: Add the second Vite entry point**

In `vite.config.ts`, replace this line:

```ts
  build: { target: 'esnext' },
```

with:

```ts
  build: {
    target: 'esnext',
    // Two pages: the app, and the tooltip window. Without naming both here
    // Rollup emits only index.html and the popup 404s in a production build —
    // which `npm run dev` does NOT reveal, because the dev server serves any
    // HTML file on disk.
    rollupOptions: {
      input: {
        main: resolve(__dirname, 'index.html'),
        popover: resolve(__dirname, 'popover.html'),
      },
    },
  },
```

Add these imports above the existing `defineConfig` import:

```ts
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
```

and this line immediately below the imports:

```ts
const __dirname = dirname(fileURLToPath(import.meta.url));
```

(`vite.config.ts` is ESM, so `__dirname` is not defined for free.)

- [ ] **Step 2: Create the popup's page shell**

Create `popover.html` at the repo root, beside `index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Definition</title>
  </head>
  <body>
    <main id="tooltip"></main>
    <script type="module" src="/src/popover.ts"></script>
  </body>
</html>
```

It loads `/src/popover.ts` and **never** `/src/main.ts`. Importing the app entry here would run the whole application a second time, including the clipboard handshake and a second set of `listen` registrations.

- [ ] **Step 3: Create the popup's entry module**

Create `src/popover.ts`:

```ts
// The tooltip window's entry point. Deliberately tiny and deliberately not
// importing anything from `main.ts`: this page runs in a second webview, and
// pulling in the app entry would start a second clipboard handshake and a
// second set of event listeners against the same backend.
const tooltip = document.querySelector<HTMLElement>('#tooltip')!;

// Task 4 replaces this with the real renderer. For now it proves the page
// loads and the second entry point is wired.
tooltip.textContent = 'tooltip';
```

- [ ] **Step 4: Write the Rust module**

Create `src-tauri/src/popover.rs`:

```rust
//! The tooltip window: a second, undecorated, always-on-top webview that can
//! extend past the main window onto the desktop.
//!
//! It exists for the process lifetime and is shown and hidden rather than
//! created per hover — building a webview costs hundreds of milliseconds,
//! which would be plainly visible after the 350 ms dwell that precedes it.

use tauri::{App, Manager, WebviewUrl, WebviewWindowBuilder};

/// The window's label. The capability file and the frontend both name it, so
/// it lives here as one constant rather than three string literals.
pub const LABEL: &str = "popover";

/// Build the hidden tooltip window. Called once from `main`'s `setup`.
pub fn create(app: &App) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        // ta-old's tooltip is `WS_POPUP | WS_BORDER` with
        // `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST`
        // (`MyToolTip.cpp:825`). These are the Tauri equivalents; the border is
        // CSS on the page, since a decorationless window has no frame to style.
        .decorations(false)
        .always_on_top(true)
        .focused(false)
        .skip_taskbar(true)
        .visible(false)
        .resizable(false)
        .shadow(false)
        .inner_size(320.0, 120.0)
        .build()?;
    Ok(())
}

/// Size, position, and show the tooltip, in that order.
///
/// One command rather than three so the window is never painted at a stale
/// position: it stays hidden until the last statement here.
#[tauri::command]
pub fn place_popover(
    app: tauri::AppHandle,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| format!("no window labelled {LABEL}"))?;
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())
}

/// Hide it. Not an error when the window is already hidden.
#[tauri::command]
pub fn hide_popover(app: tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| format!("no window labelled {LABEL}"))?;
    window.hide().map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Register it**

In `src-tauri/src/main.rs`, add to the `mod` block (`:13-19`), keeping the list alphabetical:

```rust
mod popover;
```

Inside `.setup(|app| {` (`:29`), as the **last** statement before that closure's `Ok(())`:

```rust
            // Built here rather than declared in tauri.conf.json's `windows`
            // array: the config array has no way to express "create it hidden
            // and never show it until asked", and a tooltip that flashes at
            // startup is worse than no tooltip.
            popover::create(app)?;
```

And add both commands to `.invoke_handler(tauri::generate_handler![` (`:125`), after `commands::needs_dictionary` — add a trailing comma to that line first:

```rust
            popover::place_popover,
            popover::hide_popover
```

- [ ] **Step 6: Grant the popup its capability**

Create `src-tauri/capabilities/popover.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "popover",
  "description": "Content delivery for the tooltip window.",
  "windows": ["popover"],
  "permissions": ["core:event:allow-listen"]
}
```

Capabilities are per-window-label and `default.json` names only `main` — without this file the popup's `listen` call is denied at runtime with no compile-time warning.

- [ ] **Step 7: Verify the gate is unchanged**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace 2>&1 | grep -E "^test result"
npx tsc --noEmit
npm run build
npx vitest run
ls dist/
```

Expected: clippy clean; Rust still 357 passing; `tsc` silent; Vitest still 44; `dist/` contains **both** `index.html` and `popover.html`. Report the `ls dist/` output — a missing second page here is a silent failure that only surfaces in a packaged build.

- [ ] **Step 8: Verify the window exists by hand**

**Required.** `npm run tauri dev`. The app opens as before with **no second window visible** — that is the pass condition, since the popup is created hidden.

Report what you observed. If a second empty window flashes or stays on screen, that is a finding: `.visible(false)` is not taking effect, and Task 6 would inherit a permanently-visible tooltip.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/popover.rs src-tauri/src/main.rs src-tauri/capabilities/popover.json popover.html src/popover.ts vite.config.ts
git commit -m "feat: add the hidden tooltip window and its placement commands"
```

---

## Task 2: The text assembler

**Files:**
- Create: `src/render/tooltip-text.ts`, `src/render/tooltip-text.test.ts`

**Interfaces:**
- Consumes: `Entry` from `src/types.ts`.
- Produces: `assembleTooltipText(entries: Entry[]): string` and `CONJ_MARKER: string`. Tasks 3 and 4 import them.

- [ ] **Step 1: Write the failing tests**

Create `src/render/tooltip-text.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import type { Entry } from '../types';
import { assembleTooltipText, CONJ_MARKER } from './tooltip-text';

function entry(overrides: Partial<Entry> = {}): Entry {
  return {
    headword: '消える',
    reading: 'きえる',
    conjugation: null,
    pos: ['v1', 'vi'],
    senses: [
      { pos: ['v1', 'vi'], glosses: ['to disappear', 'to vanish'], xrefs: [], misc: [], info: [] },
    ],
    flags: ['primary'],
    ...overrides,
  };
}

describe('assembleTooltipText', () => {
  // The headword line is ta-old's `headword【reading】` from
  // DictionaryUtil.cpp:57-77; senses are `(pos) (N) gloss/gloss`.
  test('renders a headword line and a numbered sense line', () => {
    expect(assembleTooltipText([entry()])).toBe(
      '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
    );
  });

  // Glosses join with "/" — ta-old's separator — not the pane's "; ".
  test('joins glosses with a slash', () => {
    const e = entry({
      senses: [{ pos: ['n'], glosses: ['a', 'b', 'c'], xrefs: [], misc: [], info: [] }],
    });
    expect(assembleTooltipText([e])).toContain('(n) (1) a/b/c');
  });

  // Numbering is per entry and starts at 1, matching the reference screenshot.
  test('numbers senses from one, per entry', () => {
    const e = entry({
      senses: [
        { pos: ['v1'], glosses: ['first'], xrefs: [], misc: [], info: [] },
        { pos: ['v1'], glosses: ['second'], xrefs: [], misc: [], info: [] },
      ],
    });
    expect(assembleTooltipText([e])).toContain('(v1) (2) second');
  });

  // Every match stacked is the point of this phase: the pane was previously
  // the only place alternates appeared at all.
  test('stacks every entry', () => {
    const text = assembleTooltipText([entry(), entry({ headword: '来る', reading: 'くる' })]);
    expect(text).toContain('消える【きえる】');
    expect(text).toContain('来る【くる】');
  });

  // A kana-only word has no separate reading to bracket.
  test('omits the bracket when there is no reading', () => {
    expect(assembleTooltipText([entry({ headword: 'ある', reading: null })])).toContain('ある\n');
  });

  // `(P)` is ta-old's common-word marker, appended to the last sense only.
  test('appends (P) for a common entry', () => {
    expect(assembleTooltipText([entry({ flags: ['primary', 'common'] })])).toMatch(/\/\(P\)$/);
  });

  // The conjugation gets its own line, marked so the colouriser can find it —
  // DictionaryUtil.cpp:46 sets `temp[0] = 1` for exactly this purpose.
  test('puts a conjugation on its own marked line', () => {
    const text = assembleTooltipText([entry({ conjugation: 'Negative Formal Past' })]);
    expect(text.split('\n')[0]).toBe(`${CONJ_MARKER}Negative Formal Past`);
  });

  test('renders nothing for no entries', () => {
    expect(assembleTooltipText([])).toBe('');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/render/tooltip-text.test.ts`

Expected: FAIL — the module does not exist, so the import cannot resolve. That is the intended RED.

- [ ] **Step 3: Implement the assembler**

Create `src/render/tooltip-text.ts`:

```ts
import type { Entry } from '../types';

/**
 * Prefix marking a line as the conjugation label.
 *
 * ta-old uses the literal control character `\x01` for this
 * (`DictionaryUtil.cpp:46`), and the colouriser looks for it the same way
 * `MyDrawText` does. Kept as a control character rather than a friendlier
 * sentinel precisely because it cannot occur in dictionary text.
 */
export const CONJ_MARKER = '';

/** One entry's block: an optional conjugation line, a headword line, then senses. */
function entryLines(entry: Entry): string[] {
  const lines: string[] = [];

  if (entry.conjugation !== null) lines.push(`${CONJ_MARKER}${entry.conjugation}`);

  // `headword【reading】`, with the bracket omitted for a kana-only word where
  // the surface already is the reading.
  lines.push(entry.reading === null ? entry.headword : `${entry.headword}【${entry.reading}】`);

  const common = entry.flags.includes('common');
  entry.senses.forEach((sense, i) => {
    // Glosses join with "/" — ta-old's separator. The pane uses "; "; the two
    // surfaces render differently on purpose from this phase onward.
    const glosses = sense.glosses.join('/');
    const tail = common && i === entry.senses.length - 1 ? '/(P)' : '';
    lines.push(`(${sense.pos.join(',')}) (${i + 1}) ${glosses}${tail}`);
  });

  return lines;
}

/**
 * Every match for one word, as ta-old's flat text block.
 *
 * Flat text rather than structured markup because the colouring that follows
 * is lexical, not semantic (see `tooltip-colour.ts`): it reads characters, so
 * what it needs is characters.
 */
export function assembleTooltipText(entries: Entry[]): string {
  return entries.flatMap(entryLines).join('\n');
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npx vitest run src/render/tooltip-text.test.ts
npx tsc --noEmit
```

Expected: 8 tests PASS; `tsc` silent. Report the count.

- [ ] **Step 5: Commit**

```bash
git add src/render/tooltip-text.ts src/render/tooltip-text.test.ts
git commit -m "feat: assemble the tooltip's text the way ta-old does"
```

---

## Task 3: The lexical colouriser

**Files:**
- Create: `src/render/tooltip-colour.ts`, `src/render/tooltip-colour.test.ts`

**Interfaces:**
- Consumes: `CONJ_MARKER` from `src/render/tooltip-text.ts` (Task 2).
- Produces: `type RunKind = 'conj' | 'paren' | 'kana' | 'kanji' | 'text'`, `interface Run { text: string; kind: RunKind }`, and `colourLine(line: string): Run[]`. Task 4 imports all three.

**Why this is its own task:** it is the subtlest piece in the phase and the one most likely to be approximated rather than ported. `MyDrawText` (`MyToolTip.cpp:125-268`) never knows what a headword or a part of speech is — it colours by what characters *are*. Reproducing the rule reproduces the edge cases; reproducing the screenshot does not.

- [ ] **Step 1: Write the failing tests**

Create `src/render/tooltip-colour.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { CONJ_MARKER } from './tooltip-text';
import { colourLine } from './tooltip-colour';

/** The kinds a line produces, in order — the shape most assertions care about. */
function kinds(line: string): string[] {
  return colourLine(line).map((run) => run.kind);
}

describe('colourLine', () => {
  // MyToolTip.cpp:154-160 — a line flagged with \x01 is drawn entirely in the
  // conjugation colour, marker stripped.
  test('a marked line is one conjugation run with the marker removed', () => {
    expect(colourLine(`${CONJ_MARKER}Negative Formal Past`)).toEqual([
      { text: 'Negative Formal Past', kind: 'conj' },
    ]);
  });

  // A run containing kanji takes the kanji colour; an all-kana run does not.
  // This is why 消える reads red and 【きえる】 green in the reference shot.
  test('splits a headword line into a kanji run and a kana run', () => {
    expect(colourLine('消える【きえる】')).toEqual([
      { text: '消える', kind: 'kanji' },
      { text: '【きえる】', kind: 'kana' },
    ]);
  });

  // The break before 【 is what separates them: without it the whole string is
  // one Japanese run and the reading inherits the kanji colour.
  test('breaks before 【 even mid-run', () => {
    expect(kinds('旅だつ【たびだつ】')).toEqual(['kana', 'kana']);
  });

  // MyToolTip.cpp:214-217 — everything from ( to its match is one paren run.
  test('a parenthesised span is one run', () => {
    expect(colourLine('(v1,vi) x')).toEqual([
      { text: '(v1,vi)', kind: 'paren' },
      { text: ' x', kind: 'text' },
    ]);
  });

  // Parenthesis colouring is tested BEFORE the Japanese check
  // (MyToolTip.cpp:224), so kanji inside parentheses does not win.
  test('parenthesis beats kanji', () => {
    expect(colourLine('(e.g. 寿司)')).toEqual([{ text: '(e.g. 寿司)', kind: 'paren' }]);
  });

  // The converse: kanji loose in gloss text takes the kanji colour — an
  // outcome semantic markup would get wrong.
  test('kanji loose in gloss text still takes the kanji colour', () => {
    expect(kinds('a type of 寿司 dish')).toEqual(['text', 'kanji', 'text']);
  });

  test('nested parentheses close at the outermost match', () => {
    expect(colourLine('(a (b) c)')).toEqual([{ text: '(a (b) c)', kind: 'paren' }]);
  });

  // An unclosed parenthesis runs to the end of the line rather than throwing
  // or silently dropping the rest of a gloss over a typo in JMdict.
  test('an unclosed parenthesis runs to end of line', () => {
    expect(colourLine('x (abc')).toEqual([
      { text: 'x ', kind: 'text' },
      { text: '(abc', kind: 'paren' },
    ]);
  });

  test('an empty line produces no runs', () => {
    expect(colourLine('')).toEqual([]);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/render/tooltip-colour.test.ts`

Expected: FAIL — the module does not exist. That is the intended RED.

- [ ] **Step 3: Implement the colouriser**

Create `src/render/tooltip-colour.ts`:

```ts
import { CONJ_MARKER } from './tooltip-text';

export type RunKind = 'conj' | 'paren' | 'kana' | 'kanji' | 'text';

export interface Run {
  text: string;
  kind: RunKind;
}

/** Hiragana, katakana, and the prolonged sound mark. */
const KANA = /[぀-ヿ]/;
/** 【 and 】, which ta-old treats as Japanese and breaks before. */
const OPEN_LENTICULAR = '【';
const BRACKET = /[【】]/;
/** CJK ideographs, including extension A and the compatibility block. */
const CJK = /[㐀-䶿一-鿿豈-﫿]/;

function isJapanese(ch: string): boolean {
  return KANA.test(ch) || BRACKET.test(ch) || CJK.test(ch);
}

/** Index just past the `)` matching the `(` at START, or the line's end. */
function closeParen(line: string, start: number): number {
  let depth = 0;
  for (let i = start; i < line.length; i += 1) {
    if (line[i] === '(') depth += 1;
    else if (line[i] === ')') {
      depth -= 1;
      if (depth === 0) return i + 1;
    }
  }
  // Unclosed: ta-old's FindCloseBrace returns null and the run simply extends.
  // Truncating instead would drop the rest of a gloss over a typo in JMdict.
  return line.length;
}

/** Index just past a Japanese run starting at START. */
function endOfJapanese(line: string, start: number): number {
  let i = start;
  while (i < line.length && isJapanese(line[i])) {
    // Break BEFORE a 【 that is not the run's first character
    // (MyToolTip.cpp:216). Without this, 消える【きえる】 is one run and the
    // reading inherits the kanji colour instead of the kana one.
    if (i > start && line[i] === OPEN_LENTICULAR) break;
    i += 1;
  }
  return i;
}

/**
 * One line of assembled tooltip text, split into coloured runs.
 *
 * Ported from `MyDrawText` (`MyToolTip.cpp:125-268`), which colours by what
 * characters *are* rather than by what they mean: it has no notion of a
 * headword or a part of speech. That is why `(v1,vi)`, `(1)` and `(P)` all
 * render alike — they are parentheses — and it is why this is a port of the
 * rule rather than of its output.
 */
export function colourLine(line: string): Run[] {
  if (line.startsWith(CONJ_MARKER)) {
    const text = line.slice(CONJ_MARKER.length);
    return text === '' ? [] : [{ text, kind: 'conj' }];
  }

  const runs: Run[] = [];
  let i = 0;

  while (i < line.length) {
    if (line[i] === '(') {
      // Checked first, so kanji inside parentheses stays parenthesis-coloured.
      const end = closeParen(line, i);
      runs.push({ text: line.slice(i, end), kind: 'paren' });
      i = end;
      continue;
    }

    if (isJapanese(line[i])) {
      const end = endOfJapanese(line, i);
      const text = line.slice(i, end);
      runs.push({ text, kind: CJK.test(text) ? 'kanji' : 'kana' });
      i = end;
      continue;
    }

    let end = i;
    while (end < line.length && line[end] !== '(' && !isJapanese(line[end])) end += 1;
    runs.push({ text: line.slice(i, end), kind: 'text' });
    i = end;
  }

  return runs;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npx vitest run src/render/tooltip-colour.test.ts
npx tsc --noEmit
```

Expected: 9 tests PASS; `tsc` silent. Report the count.

- [ ] **Step 5: Commit**

```bash
git add src/render/tooltip-colour.ts src/render/tooltip-colour.test.ts
git commit -m "feat: port ta-old's lexical tooltip colouring"
```

---

## Task 4: Rendering into the popup

**Files:**
- Create: `src/render/tooltip.ts`, `src/styles/tooltip.css`
- Modify: `src/popover.ts`, `src/styles/tokens.css`

**Interfaces:**
- Consumes: `assembleTooltipText` (Task 2); `colourLine` (Task 3); `Entry` from `src/types.ts`; the `popover-content` event name (Task 1).
- Produces: `renderTooltip(entries: Entry[]): HTMLElement`, and the `popover-measured` event carrying `{ width: number; height: number }`. Task 6 listens for that event; Task 7 exercises the renderer through a real DOM.

- [ ] **Step 1: Add the tokens**

In `src/styles/tokens.css`, add to the bare `:root` block after `--color-shadow`:

```css
  --tt-bg: oklch(97% 0.03 100);
  --tt-border: oklch(70% 0.02 100);
  --tt-text: oklch(20% 0 0);
  --tt-kanji: oklch(45% 0.19 25);
  --tt-kana: oklch(48% 0.15 145);
  --tt-paren: oklch(52% 0.15 245);
  --tt-conj: oklch(45% 0.12 300);
```

and to **both** dark blocks — the one inside `@media (prefers-color-scheme: dark)` and `:root[data-theme='dark']` — after their own `--color-shadow`, matching each block's existing indentation:

```css
    --tt-bg: oklch(24% 0.02 100);
    --tt-border: oklch(40% 0.02 100);
    --tt-text: oklch(93% 0 0);
    --tt-kanji: oklch(72% 0.16 25);
    --tt-kana: oklch(75% 0.14 145);
    --tt-paren: oklch(75% 0.13 245);
    --tt-conj: oklch(75% 0.11 300);
```

The light background approximates Windows' `COLOR_INFOBK` pale yellow, which the reference screenshots show. Dark values are picked fresh rather than darkened: a crimson legible on pale yellow is not legible on near-black.

- [ ] **Step 2: Write the popup's styles**

Create `src/styles/tooltip.css`:

```css
@import './tokens.css';

/* The window is undecorated, so the page paints its own border — this is
   ta-old's `WS_BORDER` (`MyToolTip.cpp:826`), which a Tauri decorationless
   window has no frame to provide. */
html,
body {
  margin: 0;
  height: 100%;
  background: var(--tt-bg);
  color: var(--tt-text);
}

#tooltip {
  box-sizing: border-box;
  height: 100%;
  padding: 3px 4px;
  border: 1px solid var(--tt-border);
  /* Scrolls rather than truncating: 2G capped the height and showed only an
     entry's head, which spec §3.4 reverses. ta-old scrolls here too
     (`MyToolTip.cpp:492-497`). */
  overflow-y: auto;
  font-family: var(--font-ui);
  font-size: var(--text-gloss);
  line-height: 1.35;
}

.tt-line {
  /* Wrapped continuations hang by 10px, matching MyDrawText's `xOffset = 10`,
     so a long sense reads as a column rather than as a paragraph. */
  padding-left: 10px;
  text-indent: -10px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}

.tt-kanji {
  color: var(--tt-kanji);
}
.tt-kana {
  color: var(--tt-kana);
}
.tt-paren {
  color: var(--tt-paren);
}
.tt-conj {
  color: var(--tt-conj);
}
.tt-text {
  color: var(--tt-text);
}
```

- [ ] **Step 3: Write the renderer**

Create `src/render/tooltip.ts`:

```ts
import type { Entry } from '../types';
import { assembleTooltipText } from './tooltip-text';
import { colourLine } from './tooltip-colour';

/**
 * The tooltip's body: every match for one word, coloured ta-old's way.
 *
 * Deliberately NOT `renderEntry`. 2G shared one renderer between the pane and
 * the popover so they could not drift; from this phase they are meant to
 * differ, so each owns its own. The pane is untouched.
 */
export function renderTooltip(entries: Entry[]): HTMLElement {
  const root = document.createElement('div');
  root.className = 'tt';

  for (const line of assembleTooltipText(entries).split('\n')) {
    const el = document.createElement('div');
    el.className = 'tt-line';
    for (const run of colourLine(line)) {
      const span = document.createElement('span');
      span.className = `tt-${run.kind}`;
      // `textContent`, never `innerHTML`: this is the one place in the phase
      // where a pre-assembled string meets the DOM, and it is exactly the
      // shape that invites the wrong API.
      span.textContent = run.text;
      el.append(span);
    }
    // An empty line still needs height, or stacked entries run together.
    if (!el.hasChildNodes()) el.append(document.createElement('br'));
    root.append(el);
  }

  return root;
}
```

- [ ] **Step 4: Wire the popup entry**

Replace the whole of `src/popover.ts` with:

```ts
import { emit, listen } from '@tauri-apps/api/event';
import { renderTooltip } from './render/tooltip';
import type { Entry } from './types';
import './styles/tooltip.css';

// The tooltip window's entry point. Deliberately does not import `main.ts`:
// this page runs in a second webview, and pulling in the app entry would start
// a second clipboard handshake and a second set of event listeners.
const tooltip = document.querySelector<HTMLElement>('#tooltip')!;

void listen<Entry[]>('popover-content', (e) => {
  tooltip.replaceChildren(renderTooltip(e.payload));
  tooltip.scrollTop = 0;
  // The main window cannot measure this content — it is in another webview —
  // so the size round-trips back. `scrollWidth`/`scrollHeight` rather than
  // `getBoundingClientRect`: the window is still at its previous size, so the
  // laid-out box is the old one and only the scroll extent reflects the new
  // content.
  void emit('popover-measured', {
    width: tooltip.scrollWidth,
    height: tooltip.scrollHeight,
  });
});
```

- [ ] **Step 5: Verify the gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
```

Expected: Vitest 61 (44 + 8 from Task 2 + 9 from Task 3); `tsc` silent; the build emits both pages. Report the count.

- [ ] **Step 6: Prove the token blocks are complete**

Read the final `src/styles/tokens.css` and confirm each of the seven `--tt-*` tokens appears **exactly three times** — once on bare `:root`, once inside `@media (prefers-color-scheme: dark)`, once under `:root[data-theme='dark']`. Quote the count per token.

A token missing from one dark block is invisible until someone switches theme, and no test in either suite covers it.

- [ ] **Step 7: Commit**

```bash
git add src/render/tooltip.ts src/styles/tooltip.css src/popover.ts src/styles/tokens.css
git commit -m "feat: render the tooltip's content in the popup window"
```

---

## Task 5: Placement against the work area

**Files:**
- Modify: `src/render/popover.ts`, `src/render/popover.test.ts`, `src/styles/global.css`

**Interfaces:**
- Consumes: nothing new.
- Produces: `interface Rect { left: number; top: number; right: number; bottom: number }`, `interface Point { x: number; y: number }`, `placePopover(chip: Rect, size: { width: number; height: number }, work: Rect): { left: number; top: number }`, and `shouldKeep(prev: Point, next: Point, centre: Point): boolean`. Task 6 imports all four.

**Note on the rewrite:** `showEntryPopover` and `hideEntryPopover` are **deleted** — the DOM element they managed no longer exists. `placePopover` keeps its name and its purity but changes its second and third parameters. Report the deletion as intentional, not as an omission.

- [ ] **Step 1: Replace the tests**

Replace the whole of `src/render/popover.test.ts` with:

```ts
import { describe, expect, test } from 'vitest';
import { placePopover, shouldKeep, type Rect } from './popover';

/** A chip 40×28 at (60,100), in screen coordinates. */
function chip(overrides: Partial<Rect> = {}): Rect {
  return { left: 60, top: 100, right: 100, bottom: 128, ...overrides };
}

const SIZE = { width: 200, height: 60 };
/** A 1000×700 work area at the origin — no dock, no menu bar. */
const WORK: Rect = { left: 0, top: 0, right: 1000, bottom: 700 };

describe('placePopover', () => {
  // Below is preferred, reversing 2G. That reasoning was about what the
  // popover covered inside the window; a tooltip that leaves the window
  // mostly covers desktop, and below-first is ta-old's behaviour.
  test('sits below the chip when there is room, with the 2px gap', () => {
    expect(placePopover(chip(), SIZE, WORK)).toEqual({ left: 60, top: 130 });
  });

  // A word near the bottom of the screen has no room below, and the tooltip
  // would otherwise be pushed under the dock.
  test('flips above when below would overflow the work area', () => {
    expect(placePopover(chip({ top: 600, bottom: 628 }), SIZE, WORK).top).toBe(600 - 60 - 2);
  });

  // Neither side fits: pin to the work-area bottom and go right of the word.
  // This is MyToolTip.cpp:518-523, the case 2G had no answer for.
  test('pins to the bottom and moves right when neither side fits', () => {
    const tall = { width: 200, height: 690 };
    expect(placePopover(chip({ top: 300, bottom: 328 }), tall, WORK)).toEqual({
      left: 102,
      top: 10,
    });
  });

  // No room on the right either, so it goes to the word's left.
  test('moves left when there is no room on the right', () => {
    const tall = { width: 200, height: 690 };
    const right = chip({ left: 850, right: 890, top: 300, bottom: 328 });
    expect(placePopover(right, tall, WORK).left).toBe(850 - 200 - 2);
  });

  // The horizontal clamp is last, so a tooltip anchored near the right edge is
  // pulled back inside rather than hanging off it.
  test('clamps the right edge into the work area', () => {
    expect(placePopover(chip({ left: 950, right: 990 }), SIZE, WORK).left).toBe(1000 - 200 - 8);
  });

  // The left clamp wins over the right one, so a tooltip wider than the work
  // area pins at the left margin instead of going negative.
  test('pins to the left margin when wider than the work area', () => {
    expect(placePopover(chip(), { width: 1200, height: 60 }, WORK).left).toBe(8);
  });

  // A work area that does not start at the origin — a Mac with a menu bar, or
  // a second monitor to the right of the first.
  test('respects a work area with a non-zero origin', () => {
    const work: Rect = { left: 1000, top: 25, right: 2000, bottom: 700 };
    expect(placePopover(chip({ left: 1060, right: 1100 }), SIZE, work)).toEqual({
      left: 1060,
      top: 130,
    });
  });
});

describe('shouldKeep', () => {
  const centre = { x: 500, y: 500 };

  // ta-old's rule (MyToolTip.cpp:354): moving toward the tooltip keeps it, so
  // the cursor can cross the gap between the word and the tooltip to scroll it.
  test('keeps it when the cursor moves toward the tooltip', () => {
    expect(shouldKeep({ x: 0, y: 0 }, { x: 100, y: 100 }, centre)).toBe(true);
  });

  test('dismisses it when the cursor moves away', () => {
    expect(shouldKeep({ x: 100, y: 100 }, { x: 0, y: 0 }, centre)).toBe(false);
  });

  // Equal distance is not "toward": a cursor circling at a fixed radius is not
  // heading for the tooltip, and treating that as intent would make the
  // tooltip impossible to dismiss by moving sideways.
  test('dismisses it when the distance does not change', () => {
    expect(shouldKeep({ x: 500, y: 400 }, { x: 400, y: 500 }, centre)).toBe(false);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/render/popover.test.ts`

Expected: FAIL — `shouldKeep` and `Rect` do not exist, and `placePopover`'s current signature takes a `DOMRect` and a viewport width. That is the intended RED.

- [ ] **Step 3: Rewrite the module**

Replace the whole of `src/render/popover.ts` with:

```ts
/** Distance between the word and the tooltip, in px. ta-old's `rAvoid.top-2`. */
const GAP = 2;
/** Closest the tooltip may sit to a work-area edge, in px. */
const MARGIN = 8;

/** A rectangle in screen coordinates. */
export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Point {
  x: number;
  y: number;
}

/**
 * Where to put the tooltip window, as a pure function of three rectangles.
 *
 * Separated from the DOM because happy-dom returns zeros from
 * `getBoundingClientRect()` — geometry asserted through a DOM in Vitest would
 * pass regardless of what this computed. That reasoning survives 2G unchanged;
 * what changed is that the third argument is the monitor's work area, so the
 * tooltip clamps above the dock rather than to the app's own window.
 */
export function placePopover(
  chip: Rect,
  size: { width: number; height: number },
  work: Rect,
): { left: number; top: number } {
  let top: number;
  let left = chip.left;

  const below = chip.bottom + GAP;
  const above = chip.top - GAP - size.height;

  if (below + size.height <= work.bottom) {
    top = below;
  } else if (above >= work.top) {
    top = above;
  } else {
    // Fits on neither side: pin to the bottom of the work area and step
    // sideways, right first (MyToolTip.cpp:518-523). This is the case a small
    // window hits most — a tall entry on a word low on the screen.
    top = work.bottom - size.height;
    if (chip.right + GAP + size.width <= work.right) left = chip.right + GAP;
    else if (chip.left - GAP - size.width >= work.left) left = chip.left - GAP - size.width;
  }

  // Applied last, and the left clamp wins: for a tooltip wider than the work
  // area the right-edge limit falls below the left margin, and `Math.max` pins
  // it to the margin instead of letting that push it off-screen.
  left = Math.max(work.left + MARGIN, Math.min(left, work.right - size.width - MARGIN));
  // The vertical pin can undershoot on a work area shorter than the tooltip;
  // the same argument applies.
  top = Math.max(work.top + MARGIN, top);

  return { left, top };
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/**
 * Whether a cursor that has left the word should keep the tooltip open.
 *
 * ta-old's rule (`MyToolTip.cpp:354`): compare the cursor's distance to the
 * tooltip's centre against its distance from the previous sample. Closer means
 * the user is heading for the tooltip. Chosen over a grace period because it
 * holds no timer for every dismissal path to remember to clear, and because it
 * distinguishes moving *toward* the tooltip from merely moving slowly.
 */
export function shouldKeep(prev: Point, next: Point, centre: Point): boolean {
  return distance(next, centre) < distance(prev, centre);
}
```

- [ ] **Step 4: Delete the 2G popover styles**

In `src/styles/global.css`, delete the `.entry-popover` base rule, the `.entry-popover.open` rule, and the `@media (prefers-reduced-motion: reduce)` block that names `.entry-popover` — everything 2G appended, currently `:199-242`. Leave every other rule in the file alone.

Report the file's line count before and after.

- [ ] **Step 5: Run the tests to verify they pass**

```bash
npx vitest run src/render/popover.test.ts
npx tsc --noEmit
```

Expected: the 7 `placePopover` tests and the 3 `shouldKeep` tests PASS.

`tsc` will still report errors in `src/main.ts`, which imports the now-deleted `showEntryPopover`/`hideEntryPopover`. **That is expected at this boundary and Task 6 fixes it** — this is the one task in the plan that does not end `tsc`-clean. Report the passing count and quote the exact `tsc` errors so the next task knows what it inherits.

- [ ] **Step 6: Commit**

```bash
git add src/render/popover.ts src/render/popover.test.ts src/styles/global.css
git commit -m "feat: place the tooltip against the monitor work area"
```

---

## Task 6: Wiring the window into the app

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`

**Interfaces:**
- Consumes: `placePopover`, `shouldKeep`, `Rect`, `Point` (Task 5); the `place_popover` / `hide_popover` commands and the `popover-content` event (Task 1); the `popover-measured` event (Task 4).
- Produces: nothing later in this phase imports.

- [ ] **Step 1: Extend the event mock**

In `src/main.test.ts`, replace the existing `@tauri-apps/api/event` mock block with:

```ts
const listeners = new Map<string, (e: { payload: unknown }) => void>();
/** Every `emit` the app made, as [event, payload] pairs. */
const emitted: [string, unknown][] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
  emit: (event: string, payload: unknown) => {
    emitted.push([event, payload]);
    return Promise.resolve();
  },
}));
```

Add `emitted.length = 0;` to **every** `beforeEach` block that already calls `listeners.clear()`.

`src/main.ts` will subscribe to window events through `listen('tauri://move')` rather than `getCurrentWindow().onMoved()` for exactly this reason: the former is already mocked and testable, the latter would need a second mock of `@tauri-apps/api/window`.

- [ ] **Step 2: Write the failing tests**

In `src/main.test.ts`, replace the entire `describe('the hover popover', ...)` block with:

```ts
describe('the hover tooltip', () => {
  const SEGMENTS = {
    segments: [
      {
        start: 0,
        len: 2,
        surface: '東京',
        reading: 'とうきょう',
        matched: true,
        entries: [
          {
            headword: '東京',
            reading: 'とうきょう',
            conjugation: null,
            pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          },
        ],
      },
    ],
  };

  function chip(): HTMLButtonElement {
    const el = document.querySelector<HTMLButtonElement>('.chip');
    if (el === null) throw new Error('.chip missing');
    return el;
  }

  /** Names of the commands invoked so far, in order. */
  function calls(): string[] {
    return invoke.mock.calls.map((c) => c[0] as string);
  }

  /** Names of the events emitted so far, in order. */
  function events(): string[] {
    return emitted.map((e) => e[0]);
  }

  beforeEach(async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    emit('parse-result', SEGMENTS);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // The dwell is what stops the tooltip firing for every chip the cursor
  // sweeps across on its way somewhere else.
  test('a completed dwell sends the content', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    expect(events()).not.toContain('popover-content');
    vi.advanceTimersByTime(350);
    expect(emitted).toContainEqual(['popover-content', SEGMENTS.segments[0].entries]);
  });

  test('a cursor that leaves before the dwell completes sends nothing', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    vi.advanceTimersByTime(200);
    chip().dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
    vi.advanceTimersByTime(500);
    expect(events()).not.toContain('popover-content');
  });

  // Focus moves only on a deliberate keypress, so it has no sweeping problem
  // for a dwell to solve.
  test('focus sends the content immediately, with no timer', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(events()).toContain('popover-content');
  });

  // Escape must not move focus: the user is mid-sentence and would otherwise
  // have to Tab back in from the start.
  test('Escape hides it and leaves focus on the chip', () => {
    const target = chip();
    target.focus();
    target.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(calls()).toContain('hide_popover');
    expect(document.activeElement).toBe(target);
  });

  // `show()` replaces `#output` wholesale, so a tooltip left open would be
  // anchored to a chip that is no longer in the document.
  test('a new parse-result hides it', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('parse-result', SEGMENTS);
    expect(calls()).toContain('hide_popover');
  });

  // New this phase: a separate window does not travel with its parent, so a
  // moved main window would strand it on the desktop.
  test('moving the main window hides it', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('tauri://move', {});
    expect(calls()).toContain('hide_popover');
  });

  test('resizing the main window hides it', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('tauri://resize', {});
    expect(calls()).toContain('hide_popover');
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts`

Expected: FAIL — no `popover-content` is ever emitted and no `hide_popover` is ever invoked. That is the intended RED.

- [ ] **Step 4: Rewire main.ts**

Replace this import:

```ts
import { hideEntryPopover, showEntryPopover } from './render/popover';
```

with:

```ts
import { placePopover, shouldKeep, type Point, type Rect } from './render/popover';
```

Extend the event import so it reads:

```ts
import { emit, listen } from '@tauri-apps/api/event';
```

and add:

```ts
import { cursorPosition, getCurrentWindow, monitorFromPoint } from '@tauri-apps/api/window';
```

Replace everything from `function closePopover(): void {` through the `window.addEventListener('resize', closePopover);` line with:

```ts
/** How often the keep rule samples the cursor while the tooltip is open, in ms. */
const KEEP_POLL_MS = 60;

/** Centre of the tooltip as last placed, in screen px, or `null` when closed. */
let tooltipCentre: Point | null = null;
/** Previous cursor sample, so the keep rule has something to compare against. */
let lastCursor: Point | null = null;
/** The keep-rule poll, or `undefined` when the tooltip is closed. */
let keepPoll: number | undefined;
/** The chip awaiting a measurement, so the reply knows what to anchor to. */
let pendingChip: HTMLElement | null = null;

function closePopover(): void {
  clearDwell();
  if (keepPoll !== undefined) {
    clearInterval(keepPoll);
    keepPoll = undefined;
  }
  tooltipCentre = null;
  lastCursor = null;
  pendingChip = null;
  void invoke('hide_popover');
}

/**
 * Watch the cursor while the tooltip is open.
 *
 * Polled rather than event-driven because the cursor spends the decisive
 * moments over NEITHER webview — in the gap between the word and the tooltip —
 * where no `mousemove` reaches either page. `cursorPosition()` reads the
 * global position, which is the only thing that works there.
 */
function startKeepPoll(): void {
  if (keepPoll !== undefined) return;
  keepPoll = window.setInterval(() => {
    if (tooltipCentre === null) return;
    void cursorPosition().then((position) => {
      const next = { x: position.x, y: position.y };
      const previous = lastCursor;
      lastCursor = next;
      if (previous === null || tooltipCentre === null) return;
      // A resting cursor is not movement away, so it keeps the tooltip.
      if (previous.x === next.x && previous.y === next.y) return;
      if (!shouldKeep(previous, next, tooltipCentre)) closePopover();
    });
  }, KEEP_POLL_MS);
}

/** Ask the popup to render CHIP's entries, if the last parse still knows that span. */
function openFor(chip: HTMLElement): void {
  const entries = segmentAt(chip.dataset.start)?.entries;
  // No entries means a stale chip from a superseded parse, or an unmatched
  // run — neither is an error worth surfacing.
  if (entries === undefined || entries.length === 0) return;
  pendingChip = chip;
  void emit('popover-content', entries);
}

/**
 * Convert the chip's client rect to screen coordinates and place the window.
 *
 * `outerPosition` and the monitor are physical pixels; everything the DOM
 * reports is CSS pixels. Dividing by the scale factor before mixing them is
 * invisible on a 1x display and doubles every offset on a Retina one.
 */
async function placeFor(chip: HTMLElement, size: { width: number; height: number }): Promise<void> {
  const current = getCurrentWindow();
  const [origin, scale] = await Promise.all([current.outerPosition(), current.scaleFactor()]);
  const box = chip.getBoundingClientRect();
  const left = origin.x / scale + box.left;
  const top = origin.y / scale + box.top;
  const rect: Rect = { left, top, right: left + box.width, bottom: top + box.height };

  // The monitor under the WORD, not the app's own — a window straddling two
  // screens must clamp against the one the user is looking at.
  const monitor = await monitorFromPoint(left * scale, top * scale);
  if (monitor === null) return;
  const work: Rect = {
    left: monitor.workArea.position.x / scale,
    top: monitor.workArea.position.y / scale,
    right: (monitor.workArea.position.x + monitor.workArea.size.width) / scale,
    bottom: (monitor.workArea.position.y + monitor.workArea.size.height) / scale,
  };

  // Never taller than the work area allows: past that it scrolls, as ta-old
  // does, rather than being placed off-screen.
  const height = Math.min(size.height, work.bottom - work.top - 16);
  const placed = placePopover(rect, { width: size.width, height }, work);
  tooltipCentre = { x: placed.left + size.width / 2, y: placed.top + height / 2 };
  lastCursor = null;
  await invoke('place_popover', {
    x: Math.round(placed.left),
    y: Math.round(placed.top),
    width: Math.round(size.width),
    height: Math.round(height),
  });
  startKeepPoll();
}

// Delegated on `#output` rather than on `.sentence`: `show()` replaces the
// sentence element on every parse, so a listener bound to it would be dropped
// with it, while `#output` lives for the app's lifetime.
output.addEventListener('mouseover', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  // Re-armed per chip with no sticky swap: moving between chips hides the open
  // tooltip and starts a fresh dwell.
  closePopover();
  dwell = window.setTimeout(() => openFor(chip), DWELL_MS);
});

output.addEventListener('mouseout', (e) => {
  if (chipFrom(e.target) === null) return;
  // Only the pending dwell is cancelled here. An OPEN tooltip is left to the
  // keep rule — leaving the word *toward* the tooltip must not dismiss it,
  // which is the whole point of spec §3.2.
  clearDwell();
});

output.addEventListener('focusin', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  clearDwell();
  openFor(chip);
});

output.addEventListener('focusout', (e) => {
  if (chipFrom(e.target) === null) return;
  closePopover();
});

document.addEventListener('keydown', (e) => {
  // Focus is deliberately not moved: the user is mid-sentence, and Escape
  // dismissing a tooltip should not cost them their place in the tab order.
  if (e.key === 'Escape') closePopover();
});

// The tooltip is placed from a rectangle all of these invalidate. `move` is
// new this phase: a DOM popover travelled with its parent for free, a separate
// window does not, and would be stranded on the desktop.
panes.addEventListener('scroll', closePopover);
void listen('tauri://move', closePopover);
void listen('tauri://resize', closePopover);
```

Then add one more element to the `Promise.all([...])` array at the bottom of the file, alongside the existing `listen` calls:

```ts
  listen<{ width: number; height: number }>('popover-measured', (e) => {
    const chip = pendingChip;
    pendingChip = null;
    // A chip removed by a parse that landed mid-round-trip has nothing to
    // anchor to; dropping the measurement is the correct outcome.
    if (chip === null || !chip.isConnected) return;
    void placeFor(chip, e.payload);
  }),
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
npx vitest run src/main.test.ts
npx vitest run
npx tsc --noEmit
npm run build
```

Expected: PASS, and `tsc` now silent — the errors Task 5 left are resolved here. Report the total Vitest count and `src/main.ts`'s final line count.

- [ ] **Step 6: Commit**

```bash
git add src/main.ts src/main.test.ts
git commit -m "feat: drive the tooltip window from the app's hover and focus paths"
```

---

## Task 7: End-to-end, cleanup, and the gate

**Files:**
- Create: `e2e/popover.spec.ts`
- Modify: `e2e/panes.spec.ts`
- Delete: `e2e/panes.spec.ts-snapshots/panes-popover-light-darwin.png`, `e2e/panes.spec.ts-snapshots/panes-popover-dark-darwin.png`

**Interfaces:**
- Consumes: everything Tasks 1–6 produced.
- Produces: nothing.

- [ ] **Step 1: Remove 2G's DOM-popover specs**

In `e2e/panes.spec.ts`, delete every spec that asserts against `.entry-popover`:

- `dwelling on a chip opens its definition above it, inside the viewport`
- `a chip at the right edge gets a clamped popover, not an overflowing one`
- `focusing a chip opens the popover, and Escape closes it without losing focus`
- `an unmatched run opens no popover`
- the `for (const theme of THEMES)` block titled `the popover renders correctly in ${theme}` (two specs)
- `respects prefers-reduced-motion: the popover opens with no transition`

and the `emitWideResult` and `rightmostChip` helpers, which only those specs used.

They assert on a DOM element that no longer exists. Their replacements are Step 3 here and the unit tests in Tasks 3 and 5.

Report the file's line count before and after, and the number of specs removed.

- [ ] **Step 2: Delete the two obsolete baselines**

```bash
git rm e2e/panes.spec.ts-snapshots/panes-popover-light-darwin.png \
       e2e/panes.spec.ts-snapshots/panes-popover-dark-darwin.png
git status --porcelain e2e/panes.spec.ts-snapshots/
```

Expected: exactly two deletions and **no other change** in that directory. The ten remaining baselines must be untouched — the tooltip was never in them.

**If any other baseline shows as modified, stop and report.** Do not regenerate one to make a mismatch go away.

- [ ] **Step 3: Add the popup page's specs**

Create `e2e/popover.spec.ts`:

```ts
import { expect, test } from '@playwright/test';

declare global {
  interface Window {
    __TA_EMIT__: (event: string, payload: unknown) => void;
  }
}

/**
 * The tooltip page, loaded directly.
 *
 * Playwright drives webviews, not the OS window manager, so it can prove what
 * the page renders and that it scrolls — but NOT that the window escapes the
 * main window's bounds, never steals focus, or clamps above the dock. Those
 * are Step 6's manual checks, and nothing here stands in for them.
 */
const ENTRIES = [
  {
    headword: '消える',
    reading: 'きえる',
    conjugation: null,
    pos: ['v1', 'vi'],
    senses: [
      { pos: ['v1', 'vi'], glosses: ['to disappear', 'to vanish'], xrefs: [], misc: [], info: [] },
      {
        pos: ['v1', 'vi'],
        glosses: ['to go out (of a fire, light, etc.)', 'to die'],
        xrefs: [],
        misc: [],
        info: [],
      },
    ],
    flags: ['primary', 'common'],
  },
];

/** Stub the event bridge before the page's module runs. */
const STUB = `
  window.__TA_CB__ = () => {};
  window.__TA_EMIT__ = (event, payload) => window.__TA_CB__({ event, payload });
  window.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { window.__TA_CB__ = cb; return 1; },
    invoke: () => Promise.resolve(1),
  };
`;

test('renders every sense, coloured by ta-old rules', async ({ page }) => {
  await page.addInitScript(STUB);
  await page.goto('/popover.html');
  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), ENTRIES);

  const tooltip = page.locator('#tooltip');
  await expect(tooltip).toContainText('消える【きえる】');
  await expect(tooltip).toContainText('(1) to disappear/to vanish');
  await expect(tooltip).toContainText('(2)');
  // `(P)` comes from the `common` flag, on the last sense only.
  await expect(tooltip).toContainText('(P)');

  // The colour classes are the contract between the colouriser and the CSS.
  // Asserting classes rather than computed colours keeps this from breaking
  // every time a token is retuned.
  await expect(page.locator('.tt-kanji').first()).toHaveText('消える');
  await expect(page.locator('.tt-kana').first()).toHaveText('【きえる】');
  await expect(page.locator('.tt-paren').first()).toHaveText('(v1,vi)');
});

test('scrolls rather than truncating when the content is tall', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 120 });
  await page.addInitScript(STUB);
  await page.goto('/popover.html');

  const many = [
    {
      ...ENTRIES[0],
      senses: Array.from({ length: 20 }, (_, i) => ({
        pos: ['v1'],
        glosses: [`sense number ${i + 1} with enough text to wrap onto a second line`],
        xrefs: [],
        misc: [],
        info: [],
      })),
    },
  ];
  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), many);

  // 2G truncated here and offered no way to read the rest; spec §3.4 reverses
  // that, and this is the assertion that fails if `overflow-y` regresses.
  const scrollable = await page
    .locator('#tooltip')
    .evaluate((el) => el.scrollHeight > el.clientHeight);
  expect(scrollable).toBe(true);
});
```

If the `__TAURI_INTERNALS__` stub above does not drive `listen` in this Tauri version, read `e2e/stub.ts` — the main suite already solves this problem — and reuse its mechanism rather than inventing a second one. Report which you used.

- [ ] **Step 4: Run both suites**

```bash
npx playwright test
CI=1 npx playwright test
```

Expected: the two new `popover.spec.ts` specs PASS, and `panes.spec.ts` passes with 7 fewer specs than before. **Report the exact totals both ways — do not assume a number.** Phase 2G's plan stated a total that was wrong for its own code, and the discrepancy was only caught because the implementer counted.

- [ ] **Step 5: Run the full gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
ls dist/
npx playwright test
CI=1 npx playwright test
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
git status --porcelain
```

Expected: everything green; Rust unchanged at 357; clippy clean; `dist/` contains **both** `index.html` and `popover.html`. Report every count.

- [ ] **Step 6: Verify by hand what no test can**

**Required, not optional. This phase depends on it more than any before it** — every item here is a window-manager behaviour no assertion in this repo can observe.

`npm run tauri dev`, then:

1. **Resize the main window to its 480×320 minimum.** Hover a word with several senses. Confirm the tooltip **extends past the window onto the desktop**. This is the entire point of the phase and the defect that prompted it.
2. Confirm the tooltip **never takes focus**: the main window's title bar stays active, and if a chip was focused its focus ring stays visible.
3. Hover a word near the **bottom** of the screen. Confirm the tooltip clamps **above the dock or taskbar**, not under it.
4. Hover a word with many senses in a short window. Confirm the tooltip **scrolls** with the wheel rather than truncating.
5. Move the cursor from the word **toward** the tooltip. Confirm it stays open long enough to reach and scroll it. Then sweep the cursor **past** it. Confirm it dismisses.
6. Sweep quickly across a whole sentence. Confirm **no** tooltip fires along the way.
7. **Drag the main window** while a tooltip is open. Confirm it disappears rather than being stranded on the desktop.
8. If you have a second monitor or can change display scaling, hover a word with the app on each. Confirm the tooltip lands beside the word rather than at an offset — this is the scale-factor risk in spec §7.

**Report exactly what you observed.** If something cannot be verified in your environment, say so plainly rather than claiming it. 2E's Critical defect — an entire event architecture dead behind 340 green Rust tests — was found only because this step was attempted honestly; 2F's dead reduced-motion rule was found by reading a cascade no test covered; and 2G shipped a serif gloss the suites not only missed but committed a baseline ratifying.

- [ ] **Step 7: Commit**

```bash
git add e2e
git commit -m "test: cover the tooltip window's content and scrolling"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1.1 tooltip escapes the main window | 1 (the window), 6 (screen coords), 7 Step 6 item 1 |
| §2.1 window flags, created hidden, lives for the process | 1 Step 4 |
| §2.2 TypeScript computes, Rust applies; two commands | 1 Step 4, 5 (arithmetic), 6 (the call) |
| §2.2 no new frontend window permissions | 1 Step 6 — one capability, `allow-listen` only |
| §2.3 content by event, measured before shown | 4 Step 4 (`popover-measured`), 6 Step 4 (`placeFor`) |
| §2.4 screen coordinates and scale factor | 6 Step 4, verified in 7 Step 6 item 8 |
| §3.1 dwell 350, focus with no dwell | 6 Step 4, tested in 6 Step 2 |
| §3.2 direction-of-travel keep rule | 5 (`shouldKeep`), 6 (`startKeepPoll`), 7 Step 6 item 5 |
| §3.3 every dismissal path including window move | 6 Step 4, tested in 6 Step 2 |
| §3.4 scrolling replaces the height cap | 4 Step 2 (`overflow-y`), 7 Step 3 (the scroll spec) |
| §4 GAP 2, work area, below-preferred, flip, sideways, clamp | 5 Step 3, all six tested in 5 Step 1 |
| §5.1 assembled text, `/` joins, `(N)`, `(P)`, 【】 | 2 |
| §5.2 lexical colouring, five rules, paren beats kanji | 3 |
| §5.3 seven tokens in all three blocks | 4 Step 1, checked in 4 Step 6 |
| §5.4 10px hanging indent, break behaviour | 4 Step 2 |
| §6 pure-function tests, wiring tests, Playwright's limits | 2, 3, 5, 6, 7 |
| §6 no new baselines; the two 2G ones deleted | 7 Step 2 |
| §7 focus stealing, cursor gap, scale factor, two bundles | 7 Step 6 items 2, 5, 8; 1 Step 2 (no `main.ts` import) |
| §8 inherited constraints | Global Constraints |
| §9 multi-spelling headwords deferred | Not implemented, by design |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Three steps direct the implementer to verify and report rather than assume — Task 1 Step 7 (`ls dist/`), Task 4 Step 6 (token counts per block), and Task 7 Step 4 (spec totals, deliberately *not* pre-stated, because 2G's plan got that arithmetic wrong and only a counting implementer caught it).

**3. Type consistency across task boundaries.** Checked:

- `place_popover(x: i32, y: i32, width: u32, height: u32)` — defined in Task 1, invoked in Task 6's `placeFor` with four rounded numbers under exactly those keys — match.
- `hide_popover()` — defined in Task 1, invoked in Task 6's `closePopover` — match.
- `popover-content` carries `Entry[]` — emitted in Task 6's `openFor`, typed `listen<Entry[]>` in Task 4's `src/popover.ts`, asserted in Task 6's tests — match.
- `popover-measured` carries `{ width, height }` — emitted in Task 4, typed identically in Task 6's listener — match.
- `assembleTooltipText(entries: Entry[]): string` and `CONJ_MARKER` — Task 2; imported by Task 3's implementation, both Task 2 and Task 3 test files, and Task 4's renderer — match.
- `colourLine(line: string): Run[]`, `RunKind` — Task 3; called in Task 4, and `tt-${run.kind}` matches the five classes in Task 4 Step 2's CSS — match.
- `placePopover(chip: Rect, size: { width, height }, work: Rect)` — Task 5, called with exactly those three shapes in Task 6's `placeFor` — match.
- `shouldKeep(prev: Point, next: Point, centre: Point)` — Task 5, called in Task 6's poll — match.
- `Rect` and `Point` exported from `src/render/popover.ts` in Task 5, imported as types in Task 6 — match.
- `GAP` = 2 in Task 5 Step 3; Task 5 Step 1 asserts `130` (`128 + 2`), `600 - 60 - 2`, and `102` (`100 + 2`) — match.
- `DWELL_MS` = 350 survives from 2G unchanged; Task 6's tests advance by exactly 350 — match.

**4. Residual risks a human should look at.**

- **Task 5 deliberately leaves the tree with `tsc` errors.** `src/main.ts` still imports the deleted `showEntryPopover`/`hideEntryPopover` until Task 6 lands. The step says so and asks for the errors to be quoted. A reviewer should expect a red `tsc` at that one boundary and not score it as a failure — the Global Constraints section carves out the exception explicitly.
- **The keep-rule poll is the phase's least certain piece.** It samples every 60 ms; a fast diagonal flick could cross the gap between two samples. ta-old sampled on every `WM_MOUSEMOVE`, which is finer. If item 5 of the manual pass feels wrong, `KEEP_POLL_MS` is the first knob.
- **`monitorFromPoint` may return `null`** for a point outside every monitor — reachable when the main window is dragged mostly off-screen. `placeFor` returns early, so the tooltip simply does not open. That is the safe failure, but it is silent.
- **The measurement round-trip has no timeout.** If the popup never replies with `popover-measured` — a crashed webview, a denied capability — `pendingChip` stays set and no tooltip ever opens, with nothing logged. Task 1 Step 6's capability file is what makes this unlikely; nothing makes it observable.
- **`scrollWidth` rounds against the popup's own layout.** The popup reports its content extent while still at its previous size, so the first measurement of an unusually wide entry may come back a line taller than the final layout needs. The result is a slightly oversized window, not a wrong position.
- **No screenshot baseline covers the tooltip at all.** It is a separate OS window, outside `page` capture, so its appearance is protected only by the class assertions in Task 7 Step 3 and by human eyes. A future retune of the `--tt-*` tokens will not be caught by any suite.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-15-jparser-phase2i.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
