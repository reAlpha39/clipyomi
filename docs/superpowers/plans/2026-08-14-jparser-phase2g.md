# JParser Phase 2G — Hover-to-Preview Popover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dwelling on a word — or focusing it with the keyboard — shows that word's definition next to it, without a click and without touching the pane.

**Architecture:** A single reused `<div class="entry-popover">` lives in `#app`, outside the scrolling `.panes`, positioned `fixed` from the chip's `getBoundingClientRect()`. Its content is `renderEntry` reused verbatim from the definitions pane, so the two cannot drift. `pointer-events: none` makes the overlap-flicker loop impossible rather than merely avoided. The placement arithmetic is a pure function because happy-dom returns zeros from `getBoundingClientRect()`, so DOM-asserted geometry in Vitest would pass no matter what the code did.

**Tech Stack:** Vite, TypeScript (strict), Vitest + happy-dom, Playwright, plain CSS custom properties. No frontend framework. **No Rust changes at all** — the first phase since 2A that touches nothing under `src-tauri/` or `crates/`.

**Spec:** `docs/superpowers/specs/2026-08-14-jparser-phase2g-design.md` (authoritative). Its §6.2 predecessor is `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md`. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **No frontend framework.** Vanilla TypeScript and DOM APIs only.
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`.** `renderEntry` already obeys this and is reused rather than reimplemented. The one pre-existing `app.innerHTML = ...` template in `src/main.ts` builds the static shell and is not to be extended with dynamic content.
- **Every colour is defined on bare `:root` first**, in `src/styles/tokens.css`; the `@media (prefers-color-scheme: dark)` and `:root[data-theme='dark']` blocks may only redefine. A new token must appear in **all three** blocks — that is the file's existing pattern.
- **Only `transform` and `opacity` are animated.** The `@media (prefers-reduced-motion: reduce)` override must come **after** the base rule it overrides. Equal specificity means later wins; 2F shipped that exact override dead by placing it before, and no test in either suite caught it.
- **Names are frozen.** No new Tauri command, no new event: 2G is frontend-only. Existing commands `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`, `startup_error`, `settings_warning`, `frontend_ready`, `download_dictionary`, `needs_dictionary` and events `parse-result`, `parse-error`, `dictionary-status` are unchanged.
- **File size** 200–400 lines typical, **800 hard maximum** including tests. `src/main.ts` is at 320 and will land near 390.
- **`npx tsc --noEmit` clean** and `npm run build` clean at the end of every task. `strict` is on: no `any`, no non-null assertion except where the file already uses one for a required shell element.
- **The ten committed screenshot baselines must not change.** They are `panes-{compact,default}-{light,dark}.png`, `panes-activated-{light,dark}.png`, and 2F's four `panes-download-*.png`. The popover is absent until hovered, so none of them can move.
- **Screenshots run locally only.** Every `toHaveScreenshot` call in `e2e/panes.spec.ts` is wrapped in `if (!process.env.CI)`, because CI is ubuntu-latest and these baselines were written on macOS. New baselines follow that convention exactly.
- **Scope fence.** Pane density, font sizes, gloss filters, and furigana are Phase 3. The moment this touches them it has become Phase 3 under another name.

**Invariants this phase must not break:** the definitions pane remains the complete route to a definition; `renderEntry` has one definition used by both surfaces; unmatched runs stay non-interactive `<span>`s that are never in the tab order; chips stay real `<button>`s with Enter/Space activation; `.panes` keeps `overflow-y: auto`.

---

## Resolved facts — do not re-derive these

Measured against the tree at commit `f74522b`.

| Fact | Value |
|---|---|
| `renderEntry` | `src/render/definitions.ts:3`, currently module-private, returns `HTMLElement` with class `entry` |
| Chip markup | `src/render/sentence.ts` — matched segments are `<button class="chip …" data-start="N">`, unmatched are `<span class="unmatched">` with no `.chip` class |
| Segment lookup key | `data-start`, already used by the click-to-mark path in `src/main.ts`'s `show()` |
| `show()` | `src/main.ts:254`; replaces `#output` wholesale with a fresh sentence + definitions pair |
| Event listeners | registered in one `Promise.all` at `src/main.ts:289-294` |
| Scrolling container | `.panes` — `overflow-y: auto` at `src/styles/global.css:58` |
| Existing tokens | `--color-surface`, `--color-rule`, `--color-text`, `--space-pane` (16px), `--text-gloss`, `--duration-fast` (120ms), `--ease-out` — all in `src/styles/tokens.css` |
| Missing token | there is **no** shadow colour token; Task 1 adds one |
| Current sizes | `main.ts` 320, `definitions.ts` 73, `global.css` 197, `panes.spec.ts` 276 |
| Baseline counts | Vitest 34, Playwright 17, Rust 357 (unchanged this phase) |
| happy-dom | `getBoundingClientRect()` returns zeros; `window.innerWidth` is 1024 |
| Playwright screenshots | gated on `!process.env.CI` (`e2e/panes.spec.ts:47`), `animations: 'disabled'` is the project default |

---

## File Structure

| File | Responsibility |
|---|---|
| `src/render/popover.ts` | *(new)* owns the popover element, its content, and the placement arithmetic |
| `src/render/popover.test.ts` | *(new)* `placePopover` unit tests — the geometry happy-dom cannot verify through the DOM |
| `src/render/definitions.ts` | *(modified)* one word: `export` on `renderEntry` |
| `src/styles/tokens.css` | *(modified)* `--color-shadow` in all three `:root` blocks |
| `src/styles/global.css` | *(modified)* `.entry-popover`, its `.open` state, and the reduced-motion override placed after them |
| `src/main.ts` | *(modified)* the triggers, the dismissals, and the last-result lookup |
| `src/main.test.ts` | *(modified)* the wiring, with fake timers |
| `e2e/panes.spec.ts` | *(modified)* real-geometry specs and two new baselines |

---

## Task 1: The popover surface

**Files:**
- Create: `src/render/popover.ts`, `src/render/popover.test.ts`
- Modify: `src/render/definitions.ts` (one word), `src/styles/tokens.css`, `src/styles/global.css`

**Interfaces:**
- Consumes: `Entry` from `src/types.ts`; `renderEntry` from `src/render/definitions.ts`.
- Produces: `showEntryPopover(chip: HTMLElement, entry: Entry): void`, `hideEntryPopover(): void`, and `placePopover(chip: DOMRect, popover: { width: number; height: number }, viewport: { width: number }): { left: number; top: number }`. Task 2 imports all three.

**Note on the signature:** the spec's §2 sketch gave `viewport` both a width and a height. Only width is used — the vertical decision needs the chip's own top, and a popover taller than the viewport is capped by `max-height` rather than repositioned. The parameter is therefore `{ width: number }`. Report this as a deliberate trim, not an omission.

- [ ] **Step 1: Write the failing tests**

Create `src/render/popover.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { placePopover } from './popover';

/** A chip 40×28 sitting 100px down and 60px in from the viewport's left edge. */
function chip(overrides: Partial<DOMRect> = {}): DOMRect {
  const base = {
    x: 60,
    y: 100,
    width: 40,
    height: 28,
    top: 100,
    left: 60,
    right: 100,
    bottom: 128,
  };
  return { ...base, ...overrides } as DOMRect;
}

const POPOVER = { width: 200, height: 60 };
const VIEWPORT = { width: 480 };

describe('placePopover', () => {
  // Above by preference: above the sentence is the input row, which nobody is
  // reading, while below it are the definition rows, which they might be.
  test('sits above the chip when there is room, with the 6px gap', () => {
    const { top, left } = placePopover(chip(), POPOVER, VIEWPORT);
    expect(top).toBe(100 - 60 - 6);
    expect(left).toBe(60);
  });

  // A chip on the first line of a sentence has almost nothing above it. Placing
  // there anyway would push the popover off the top of the window, where the
  // user cannot read it and nothing scrolls it back.
  test('flips below the chip when there is no room above', () => {
    const { top } = placePopover(chip({ top: 20, bottom: 48 }), POPOVER, VIEWPORT);
    expect(top).toBe(48 + 6);
  });

  // The last chip of a wrapped line sits near the right edge; left-aligning the
  // popover to it would hang the popover's tail off-screen.
  test('clamps to the right edge rather than overflowing it', () => {
    const { left } = placePopover(chip({ left: 400, right: 440 }), POPOVER, VIEWPORT);
    expect(left).toBe(480 - 200 - 8);
  });

  // The left clamp is applied last, so a popover wider than the viewport is
  // pinned at the left margin rather than pushed off the left edge by the
  // right-edge clamp producing a negative value.
  test('pins to the left margin when the popover is wider than the viewport', () => {
    const { left } = placePopover(chip({ left: 4 }), { width: 600, height: 60 }, VIEWPORT);
    expect(left).toBe(8);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/render/popover.test.ts`

Expected: FAIL — the module does not exist, so the import cannot resolve. That is the intended RED.

- [ ] **Step 3: Export `renderEntry`**

In `src/render/definitions.ts`, change line 3 from:

```ts
function renderEntry(entry: Entry): HTMLElement {
```

to:

```ts
export function renderEntry(entry: Entry): HTMLElement {
```

Change nothing else in that file. Reusing it verbatim is what keeps the popover and the pane row from drifting, and it is why 2G makes no typography decisions of its own.

- [ ] **Step 4: Implement the popover module**

Create `src/render/popover.ts`:

```ts
import type { Entry } from '../types';
import { renderEntry } from './definitions';

/** Distance between the chip and the popover, in px. */
const GAP = 6;
/** Closest the popover may sit to a viewport edge, in px. */
const MARGIN = 8;

/**
 * Where to put the popover, as a pure function of three rectangles.
 *
 * Separated from the DOM because happy-dom returns zeros from
 * `getBoundingClientRect()`: geometry asserted through the DOM in a unit test
 * would pass regardless of what this computed.
 */
export function placePopover(
  chip: DOMRect,
  popover: { width: number; height: number },
  viewport: { width: number },
): { left: number; top: number } {
  // Above the sentence is the input row, which nobody is reading; below it are
  // the definition rows, which they might be. So above is preferred, and below
  // is the fallback when the preferred position would clip the window top.
  const above = chip.top - popover.height - GAP;
  const top = above >= MARGIN ? above : chip.bottom + GAP;

  // The left clamp is applied last on purpose: for a popover wider than the
  // viewport the right-edge limit goes negative, and `Math.max` pins it to the
  // margin instead of letting that negative value push it off-screen.
  const rightmost = viewport.width - popover.width - MARGIN;
  const left = Math.max(MARGIN, Math.min(chip.left, rightmost));

  return { left, top };
}

/**
 * The one popover element, created on first use.
 *
 * Module-local rather than recreated per hover: one node means one place for
 * the open state to live, and nothing to leak when a sentence is replaced.
 */
let popover: HTMLElement | null = null;

function element(): HTMLElement {
  if (popover === null) {
    popover = document.createElement('div');
    popover.className = 'entry-popover';
    // The same entry is already in the definitions pane, so announcing it here
    // too would read it twice. The chip keeps its own accessible name; this
    // surface is a visual convenience only.
    popover.setAttribute('aria-hidden', 'true');
    // `#app`, not `.panes`: `.panes` is `overflow-y: auto`, so a popover inside
    // it would be clipped at the pane edge and would scroll away from the word
    // it describes. Created lazily, which is also why `main.ts` assigning
    // `app.innerHTML` at import time cannot wipe it.
    document.querySelector<HTMLElement>('#app')!.append(popover);
  }
  return popover;
}

/** Show ENTRY beside CHIP. Safe to call when one is already open. */
export function showEntryPopover(chip: HTMLElement, entry: Entry): void {
  const el = element();
  el.replaceChildren(renderEntry(entry));
  // Measured before it is placed, because the height decides which side of the
  // chip it goes on. The base style is `visibility: hidden`, which still lays
  // out — so this measures correctly and no frame is ever painted at the
  // pre-placement position.
  const { left, top } = placePopover(chip.getBoundingClientRect(), el.getBoundingClientRect(), {
    width: window.innerWidth,
  });
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
  el.classList.add('open');
}

/** Hide it. A no-op when nothing has been shown yet. */
export function hideEntryPopover(): void {
  popover?.classList.remove('open');
}
```

- [ ] **Step 5: Add the shadow token**

In `src/styles/tokens.css`, add to the bare `:root` block, after `--color-counter`:

```css
  --color-shadow: oklch(0% 0 0 / 0.18);
```

and to **both** dark blocks — the one inside `@media (prefers-color-scheme: dark)` and `:root[data-theme='dark']` — after their own `--color-counter`:

```css
    --color-shadow: oklch(0% 0 0 / 0.5);
```

(Match each block's existing indentation.) A stronger shadow in dark mode because the surface and background sit only 4% apart there, so a light-mode shadow reads as nothing.

- [ ] **Step 6: Add the styles**

Append to `src/styles/global.css`:

```css
.entry-popover {
  position: fixed;
  z-index: 1;
  /* `visibility` rather than `display: none`: a hidden popover still lays out,
     so `showEntryPopover` can measure it before placing it. Playwright also
     treats a `visibility: hidden` element as not visible, which an
     `opacity: 0` one is not. */
  visibility: hidden;
  opacity: 0;
  transform: translateY(4px);
  transition:
    opacity var(--duration-fast) var(--ease-out),
    transform var(--duration-fast) var(--ease-out);
  /* Load-bearing, not polish: the popover can never intercept the hover that
     opened it, so the flicker loop — overlap, mouseout, hide, mouseover, show —
     cannot occur at all. It also settles that nothing inside can be clicked,
     expanded, scrolled, or selected. */
  pointer-events: none;
  max-width: min(320px, 100vw - 16px);
  max-height: calc(100vh - 16px);
  overflow: hidden;
  padding: var(--space-pane);
  font-size: var(--text-gloss);
  color: var(--color-text);
  background: var(--color-surface);
  border: 1px solid var(--color-rule);
  border-radius: 4px;
  box-shadow: 0 2px 8px var(--color-shadow);
}

.entry-popover.open {
  visibility: visible;
  opacity: 1;
  transform: none;
}

/* AFTER the rules above, deliberately: equal specificity means later wins, and
   2F shipped this exact kind of override dead by placing it before its base
   rule. No test in either suite catches that — only reading the cascade does. */
@media (prefers-reduced-motion: reduce) {
  .entry-popover {
    transition: none;
  }
}
```

- [ ] **Step 7: Run the tests to verify they pass**

```bash
npx vitest run src/render/popover.test.ts
npx vitest run
npx tsc --noEmit
```

Expected: the four `placePopover` tests PASS; the whole Vitest suite PASS at 38 (34 + 4); `tsc` silent. Report the counts.

- [ ] **Step 8: Prove the reduced-motion override is live**

The one thing no test here covers. Confirm by reading the final `src/styles/global.css`: the `@media (prefers-reduced-motion: reduce)` block containing `.entry-popover` must appear at a **later line number** than the `.entry-popover` base rule. Quote both line numbers in your report.

2F's identical rule shipped dead. Do not skip this by reasoning that you just wrote it correctly.

- [ ] **Step 9: Commit**

```bash
git add src/render/popover.ts src/render/popover.test.ts src/render/definitions.ts src/styles/tokens.css src/styles/global.css
git commit -m "feat: add the entry popover surface and its placement arithmetic"
```

---

## Task 2: Wire the triggers into the app

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`

**Interfaces:**
- Consumes: `showEntryPopover`, `hideEntryPopover` from `src/render/popover.ts` (Task 1).
- Produces: nothing later in this phase imports. Task 3 exercises the result through the real DOM.

- [ ] **Step 1: Write the failing tests**

Add to `src/main.test.ts`, after the existing `describe('the event-driven render path')` block:

```ts
describe('the hover popover', () => {
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

  function open(): Element | null {
    return document.querySelector('.entry-popover.open');
  }

  beforeEach(async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
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

  // The dwell is what stops a popover firing for every chip the cursor sweeps
  // across on its way somewhere else.
  test('a completed dwell opens the popover', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    expect(open()).toBeNull();
    vi.advanceTimersByTime(350);
    expect(open()?.textContent).toContain('Tokyo');
  });

  test('a cursor that leaves before the dwell completes opens nothing', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    vi.advanceTimersByTime(200);
    chip().dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
    vi.advanceTimersByTime(500);
    expect(open()).toBeNull();
  });

  // Focus moves only on a deliberate keypress, so it has no sweeping problem
  // for a dwell to solve — waiting 350 ms there would be delay with no purpose.
  test('focus opens it immediately, with no timer', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(open()).not.toBeNull();
  });

  // Escape must not move focus: the user is mid-sentence and would otherwise
  // have to Tab back in from the start.
  test('Escape hides it and leaves focus on the chip', () => {
    const target = chip();
    target.focus();
    target.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(open()).toBeNull();
    expect(document.activeElement).toBe(target);
  });

  // The guard with teeth: `show()` replaces `#output` wholesale, so a popover
  // left open would be anchored to a chip that is no longer in the document.
  test('a new parse-result hides it', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(open()).not.toBeNull();

    emit('parse-result', SEGMENTS);
    expect(open()).toBeNull();
  });
});
```

If `afterEach` is not already imported at the top of the file, extend the existing import:

```ts
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts`

Expected: FAIL — `.entry-popover.open` is never created, so `open()` is `null` where the tests require an element. That is the intended RED.

- [ ] **Step 3: Implement the wiring**

In `src/main.ts`, add to the imports at the top:

```ts
import { hideEntryPopover, showEntryPopover } from './render/popover';
```

and replace the existing `import type { ParseResult } from './types';` with:

```ts
import type { ParseResult, Segment } from './types';
```

Add a `panes` reference beside the existing element constants (near `src/main.ts:23`):

```ts
const panes = app.querySelector<HTMLElement>('.panes')!;
```

Then insert above `show()`:

```ts
/** Milliseconds the cursor must rest on a chip before its popover opens. */
const DWELL_MS = 350;

/**
 * The most recent parse, kept so a hover can find the entry for the chip under
 * the cursor. The chips carry only `data-start`; the entries live here.
 */
let lastResult: ParseResult | null = null;

/** Pending dwell timer, or `undefined` when none is armed. */
let dwell: number | undefined;

function segmentAt(start: string | undefined): Segment | undefined {
  if (start === undefined || lastResult === null) return undefined;
  return lastResult.segments.find((segment) => String(segment.start) === start);
}

/**
 * The chip an event happened on, or `null`.
 *
 * `.unmatched` runs are `<span>`s without the `chip` class, so this returns
 * `null` for them and they get no popover — an empty box for a span with no
 * entries is the one wrong outcome available here.
 */
function chipFrom(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;
  return target.closest<HTMLElement>('.chip');
}

function clearDwell(): void {
  if (dwell === undefined) return;
  clearTimeout(dwell);
  dwell = undefined;
}

function closePopover(): void {
  clearDwell();
  hideEntryPopover();
}

/** Open the popover for CHIP, if the last parse still knows that span. */
function openFor(chip: HTMLElement): void {
  const entry = segmentAt(chip.dataset.start)?.entries[0];
  // No entry means a stale chip from a superseded parse, which is not an error
  // worth surfacing — the next hover on a live chip works.
  if (entry === undefined) return;
  showEntryPopover(chip, entry);
}

// Delegated on `#output` rather than on `.sentence`: `show()` replaces the
// sentence element on every parse, so a listener bound to it would be dropped
// with it, while `#output` lives for the app's lifetime.
output.addEventListener('mouseover', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  // Re-armed per chip with no sticky swap: moving between chips hides the open
  // popover and starts a fresh dwell. One rule, and it is the rule the dwell
  // was introduced to enforce.
  closePopover();
  dwell = window.setTimeout(() => openFor(chip), DWELL_MS);
});

output.addEventListener('mouseout', (e) => {
  if (chipFrom(e.target) === null) return;
  closePopover();
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
  // dismissing a popover should not cost them their place in the tab order.
  if (e.key === 'Escape') closePopover();
});

// The popover is placed from a rectangle that both of these invalidate, and it
// cannot follow the chip — `pointer-events: none` means there is nothing to
// reposition against once the geometry moves.
panes.addEventListener('scroll', closePopover);
window.addEventListener('resize', closePopover);
```

Then change `show()` (`src/main.ts:254`) so its first statements are:

```ts
function show(result: ParseResult): void {
  // Before anything is replaced: a popover left open would be anchored to a
  // chip from the previous sentence that is about to leave the document.
  closePopover();
  lastResult = result;

  const sentence = renderSentence(result);
```

The rest of `show()` is unchanged.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npx vitest run src/main.test.ts
npx vitest run
npx tsc --noEmit
npm run build
```

Expected: PASS. Vitest lands at 43 (38 + 5). Report the count and `src/main.ts`'s final line count.

- [ ] **Step 5: Prove the unmatched-span guard is load-bearing**

Change `chipFrom`'s selector from `.chip` to `*` so unmatched spans resolve, and add this temporary test:

```ts
  test('TEMPORARY: an unmatched run opens nothing', () => {
    const span = document.querySelector<HTMLElement>('.unmatched');
    if (span === null) throw new Error('fixture has no unmatched run');
    span.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(open()).toBeNull();
  });
```

The block's `SEGMENTS` has no unmatched run, so this throws rather than asserting — which is itself the finding: **report that the guard cannot be proven with this fixture, restore the `.chip` selector, delete the temporary test, and leave the real coverage to Task 3**, where a payload with an unmatched segment can be emitted into a real DOM. Do not fabricate a passing test here.

- [ ] **Step 6: Commit**

```bash
git add src/main.ts src/main.test.ts
git commit -m "feat: open the entry popover on dwell and on chip focus"
```

---

## Task 3: Real geometry, real keyboard, baselines

**Files:**
- Modify: `e2e/panes.spec.ts`
- Create: two baseline PNGs under `e2e/panes.spec.ts-snapshots/` (one screenshot × two themes)

**Interfaces:**
- Consumes: everything Tasks 1–2 produced, through the real DOM.
- Produces: nothing.

- [ ] **Step 1: Add the specs**

Append to `e2e/panes.spec.ts`:

```ts
/**
 * A sentence long enough to wrap, so some chip ends a line hard against the
 * right edge — the case the horizontal clamp exists for. The committed
 * fixture's three-chip sentence never gets near it.
 */
async function emitWideResult(page: Page): Promise<void> {
  await page.evaluate(() => {
    const segments = Array.from({ length: 24 }, (_, i) => ({
      start: i,
      len: 1,
      surface: '本',
      reading: 'ほん',
      matched: true,
      entries: [
        {
          headword: '本',
          reading: 'ほん',
          conjugation: null,
          pos: ['n'],
          senses: [{ pos: ['n'], glosses: ['book'], xrefs: [], misc: [], info: [] }],
          flags: ['primary'],
        },
      ],
    }));
    window.__TA_EMIT__('parse-result', { segments });
  });
}

/** Index of the chip whose left edge is furthest right — the one nearest the edge. */
async function rightmostChip(page: Page): Promise<number> {
  const chips = page.locator('.chip');
  const count = await chips.count();
  let index = 0;
  let furthest = -1;
  for (let i = 0; i < count; i += 1) {
    const box = await chips.nth(i).boundingBox();
    if (box !== null && box.x > furthest) {
      furthest = box.x;
      index = i;
    }
  }
  return index;
}

test('dwelling on a chip opens its definition above it, inside the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const chip = page.locator('.chip').first();
  await chip.hover();
  // No explicit wait for the 350 ms dwell: the visibility assertion auto-waits,
  // which is the deterministic form of the same check.
  const popover = page.locator('.entry-popover');
  await expect(popover).toBeVisible();
  await expect(popover).toContainText('Tokyo');

  const chipBox = await chip.boundingBox();
  const popBox = await popover.boundingBox();
  if (chipBox === null || popBox === null) throw new Error('no box');
  expect(popBox.y + popBox.height).toBeLessThanOrEqual(chipBox.y);
  expect(popBox.x).toBeGreaterThanOrEqual(8);
  expect(popBox.y).toBeGreaterThanOrEqual(0);
});

test('a chip at the right edge gets a clamped popover, not an overflowing one', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 320 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitWideResult(page);

  const index = await rightmostChip(page);
  await page.locator('.chip').nth(index).hover();
  const popover = page.locator('.entry-popover');
  await expect(popover).toBeVisible();

  const popBox = await popover.boundingBox();
  if (popBox === null) throw new Error('no box');
  // 8px margin, with a pixel of slack for fractional layout.
  expect(popBox.x + popBox.width).toBeLessThanOrEqual(480 - 8 + 1);
});

test('focusing a chip opens the popover, and Escape closes it without losing focus', async ({
  page,
}) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  await page.locator('.chip').first().focus();
  const popover = page.locator('.entry-popover');
  await expect(popover).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(popover).not.toBeVisible();
  // The chip keeps focus, so the user does not have to Tab back in.
  await expect(page.locator('.chip').first()).toBeFocused();
});

// Task 2 could not prove this: its fixture has no unmatched run, and inventing
// one there would have tested the mock rather than the guard.
test('an unmatched run opens no popover', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.evaluate(() => {
    window.__TA_EMIT__('parse-result', {
      segments: [{ start: 0, len: 3, surface: 'xyz', reading: null, matched: false, entries: [] }],
    });
  });

  await page.locator('.unmatched').hover();
  await expect(page.locator('.entry-popover')).not.toBeVisible();
});

for (const theme of THEMES) {
  test(`the popover renders correctly in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
    await page.goto('/');
    await emitFixtureResult(page);

    await page.locator('.chip').first().hover();
    await expect(page.locator('.entry-popover')).toBeVisible();

    // Local only, exactly as every other baseline in this file: CI is
    // ubuntu-latest and these were written on macOS.
    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-popover-${theme}.png`);
    }
  });
}
```

- [ ] **Step 2: Run the suite**

```bash
npx playwright test
CI=1 npx playwright test
```

Expected: PASS at 22 (17 + 5) locally; `CI=1` also PASS with the two screenshot comparisons skipped. Report both counts.

- [ ] **Step 3: Look at the baselines you just created**

Open both `e2e/panes.spec.ts-snapshots/panes-popover-light-darwin.png` and `…-dark-darwin.png` with the Read tool and **describe what you actually see** — where the popover sits relative to the chip, whether its text is legible against the surface, and whether the border and shadow read as a raised surface in both themes.

Do not accept a baseline you have not looked at. If the popover is clipped, covers the chip, or is invisible in one theme, that is a finding to report, not a baseline to commit.

- [ ] **Step 4: Confirm the committed baselines did NOT change**

```bash
git status --porcelain e2e/panes.spec.ts-snapshots/
```

Expected: only the two new `panes-popover-*.png` files, as additions. The popover is absent until hovered, so the ten pre-existing baselines cannot move.

**If any existing baseline changed, stop and report.** Do not regenerate one to make a mismatch go away.

- [ ] **Step 5: Run the full gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
npx playwright test
CI=1 npx playwright test
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
git diff --stat
```

Expected: Vitest 43, Playwright 22, Rust 357 unchanged, clippy clean. The Rust commands are here to prove this phase changed nothing on that side — `git diff --stat` must list no file under `src-tauri/` or `crates/`.

- [ ] **Step 6: Verify by hand what no test can**

**Required, not optional.** `npm run tauri dev`, then:

1. Copy Japanese text so a sentence renders. Rest the cursor on a word and confirm the popover appears after a beat, showing that word's definition.
2. Sweep the cursor quickly across the whole sentence. Confirm **no** popovers fire along the way — that is what the dwell is for, and it is the one behaviour a Playwright `hover()` cannot reproduce.
3. Tab to a chip. Confirm the popover appears immediately, and that Escape dismisses it while leaving the focus ring where it was.
4. Hover a word on the **first** line of a sentence, where there is no room above. Confirm the popover flips below instead of being cut off by the window top.
5. Enable Reduce Motion in System Settings → Accessibility → Display, then hover a chip. Confirm the popover appears with no slide.

**Report exactly what you observed.** If something cannot be verified in your environment, say so plainly rather than claiming it. 2E's Critical defect — an entire event architecture dead behind 340 green Rust tests — was found only because this step was attempted honestly, and 2F's dead reduced-motion rule was found by reading a cascade no test covered.

- [ ] **Step 7: Commit**

```bash
git add e2e
git commit -m "test: cover the entry popover's geometry, keyboard path, and baselines"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 hover popover showing one entry | 1 (surface), 2 (trigger) |
| §1 same popover on focus, no dwell | 2 (Step 3 `focusin`), 3 (Step 1 keyboard spec) |
| §1 dismissal on every stale-making path | 2 (mouseout, focusout, Escape, scroll, resize, `show()`) |
| §1 placement never clips off-screen | 1 (`placePopover`), 3 (right-edge spec) |
| §1 alternates stay in the pane | 1 — `renderEntry(entries[0])` only, no `<details>` |
| §2 `renderEntry` reused, not reimplemented | 1 Step 3 (one-word export) |
| §2 popover lives outside `.panes` | 1 Step 4 (`#app`), with the reason in the comment |
| §2 `aria-hidden="true"` | 1 Step 4 |
| §2 no Rust changes | 3 Step 5 (`git diff --stat` check) |
| §3.1 `DWELL_MS` = 350, re-armed per chip | 2 Step 3, tested in 2 Step 1 |
| §3.2 click keeps it open | 2 Step 3 — no click handler is added, which is what makes this true |
| §3.2 `parse-result` dismissal | 2 Step 3 (`show()`), tested in 2 Step 1 |
| §3.3 `pointer-events: none` | 1 Step 6 |
| §3.4 height cap | 1 Step 6 (`max-height`) |
| §4 GAP 6, MARGIN 8, above-preferred, flip, clamp | 1 Step 4, all four tested in 1 Step 1 |
| §5 opacity + translateY, override after base rule | 1 Step 6, checked in 1 Step 8, observed in 3 Step 6 item 5 |
| §6 Vitest placement + wiring, Playwright geometry, two baselines | 1, 2, 3 |
| §7 `.unmatched` ignored | 2 Step 3 (`closest('.chip')`), proven in 3 Step 1 |
| §7 stale chip reference | 2 Step 3 (`openFor`'s `undefined` guard) |
| §8 inherited constraints | Global Constraints |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Two steps direct the implementer to **verify and report** rather than assume — Task 1 Step 8 (the cascade order) and Task 2 Step 5 (which is expected to *fail* to prove the guard, and says so).

**3. Type consistency across task boundaries.** Checked:

- `placePopover(chip: DOMRect, popover: { width, height }, viewport: { width })` — defined in Task 1 Step 4, called with exactly those three shapes in `showEntryPopover` and in Task 1 Step 1's tests — match. The spec's `viewport.height` is deliberately dropped, and Task 1 says to report it.
- `showEntryPopover(chip: HTMLElement, entry: Entry)` — defined in Task 1, called in Task 2's `openFor` with `chip: HTMLElement` and `entries[0]: Entry` — match.
- `hideEntryPopover()` — defined in Task 1, called in Task 2's `closePopover` — match.
- `Segment` and `ParseResult` come from `src/types.ts`; Task 2's `segmentAt` returns `Segment | undefined` and reads `.entries[0]`, which `Segment` declares as `Entry[]` — match.
- `.entry-popover` / `.entry-popover.open` — written in Task 1 Step 6, asserted by class in Task 2's tests and by visibility in Task 3's — match.
- `DWELL_MS` = 350 in Task 2 Step 3 and `vi.advanceTimersByTime(350)` in Task 2 Step 1 — match.
- `GAP` = 6 and `MARGIN` = 8 in Task 1 Step 4; Task 1 Step 1 asserts `100 - 60 - 6` and `480 - 200 - 8`, Task 3 asserts `>= 8` and `<= 480 - 8 + 1` — match.

**4. Residual risks a human should look at.**

- **Task 2 Step 5 is designed to fail.** It asks the implementer to discover that the unmatched-span guard cannot be proven with that block's fixture and to defer the coverage to Task 3. A reviewer should expect a report saying "could not prove, restored, deferred" and not score it as an incomplete step.
- **`document.addEventListener('keydown')` accumulates across Vitest module resets.** Each `vi.resetModules()` leaves the previous module's Escape handler attached to the persistent `document`. Harmless — the stale handler hides a detached element — but a future test that counts listeners would trip on it.
- **The dwell is unobservable in Playwright.** `hover()` plus an auto-waiting assertion proves the popover *opens*, not that it waited 350 ms first. The sweep behaviour is Task 3 Step 6 item 2, by hand, and nothing else stands behind it.
- **`max-height` with `overflow: hidden` truncates silently** for a word with many senses in a 320px-tall window. Spec §3.4 accepts this because the pane holds the full entry; it is the one place where the popover is knowingly incomplete.
- **`z-index: 1` is the only stacking claim in the app.** Nothing else sets one today, so the popover wins by document order anyway; if a later phase adds an overlay, this is the line that decides which is on top.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-14-jparser-phase2g.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
