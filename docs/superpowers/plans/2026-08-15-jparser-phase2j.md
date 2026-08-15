# JParser Phase 2J — Definitions Pane Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the redundant bottom definitions pane so the segmented sentence is the sole child in the main output area.

**Architecture:** A pure removal. The frontend deletes `renderDefinitions`, `renderEntry`, its click-to-mark handler, and associated definition CSS rules. The popover window created in Phase 2I remains the single source for word definitions on hover and focus.

**Tech Stack:** Tauri 2 (`@tauri-apps/api` 2.11.1), Vite, TypeScript (strict), Vitest + happy-dom, Playwright, plain CSS custom properties. No frontend framework. Rust is untouched.

**Spec:** `docs/superpowers/specs/2026-08-15-jparser-phase2j-design.md` (authoritative, committed at `dbe7d61`). `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **No frontend framework.** Vanilla TypeScript and DOM APIs only.
- **Dictionary content in popover stays safe.** Popover continues using `textContent` on `<span>` elements, never `innerHTML`.
- **Rust is untouched.** Rust stays at 357 passing tests.
- **`npx tsc --noEmit` clean, `npm test` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean at the end of every task.** `strict` is on: no `any`, no non-null assertion except where a file already uses one for a required shell element.
- **File size** 200–400 lines typical, **800 hard maximum** including tests.
- **Scope fence.** The popover window, clipboard monitoring, and parser engine are untouched.

---

## Resolved facts — do not re-derive these

Measured against the tree at `dbe7d61`.

| Fact | Value |
|---|---|
| `src/render/definitions.ts` | 74 lines — `renderEntry`, `renderDefinitions` |
| `src/render/definitions.test.ts` | 71 lines — 6 tests |
| `src/main.ts` | 715 lines; `renderDefinitions` import at `:5`, `show()` call at `:647`, chip-click listener at `:653-660`, `output.replaceChildren` at `:663` |
| `src/styles/global.css` | `.def-row` at `:140-146`, `.def-row.marked` at `:150-153`, `.entry-head`...`details summary` at `:155-180`, `.sentence { margin-bottom }` at `:42` |
| `src/styles/typography.css` | `.definitions` rule at `:13-16` |
| `src/main.test.ts` | 668 lines; `.def-row` assertions at `:89`, `:103`, `:120` |
| `e2e/panes.spec.ts` | 270 lines; `.def-row` at `:40`, `:62`, `:72`, `:76`, `:78`, `:234`, `:237`, `:263` |
| Baseline counts | Vitest 83 (77 after deleting `definitions.test.ts` + 1 new assertion = 78), Playwright 20, Rust 357 |

---

## File Structure

| File | Responsibility |
|---|---|
| `src/render/definitions.ts` | *(deleted)* obsolete definitions pane renderer |
| `src/render/definitions.test.ts` | *(deleted)* obsolete tests |
| `src/main.ts` | *(modified)* renders only `sentence` into `output`; click-to-mark listener removed |
| `src/main.test.ts` | *(modified)* `.def-row` assertions removed; assertion pinning absence of `.definitions` added |
| `src/styles/global.css` | *(modified)* definition classes and `.sentence` bottom margin removed |
| `src/styles/typography.css` | *(modified)* `.definitions` font rule removed |
| `e2e/panes.spec.ts` | *(modified)* `.def-row` count and marking assertions removed |
| `e2e/panes.spec.ts-snapshots/*.png` | *(regenerated)* all ten baselines updated for sentence-only output |

---

## Task 1: Remove definitions from frontend code and styles

**Files:**
- Delete: `src/render/definitions.ts`, `src/render/definitions.test.ts`
- Modify: `src/main.ts`, `src/styles/global.css`, `src/styles/typography.css`, `src/main.test.ts`

**Interfaces:**
- Consumes: `renderSentence(result: ParseResult): HTMLElement`
- Produces: A DOM where `#output.panes` holds only `.sentence`, with zero `.definitions` or `.def-row` nodes.

- [ ] **Step 1: Write the failing / updated unit tests in `src/main.test.ts`**

In `src/main.test.ts`:
1. In the `parse-result handling` describe block (around line 80):
   Replace:
   ```ts
   expect(document.querySelector('.chip')).not.toBeNull();
   expect(document.querySelector('.def-row')).not.toBeNull();
   ```
   with:
   ```ts
   expect(document.querySelector('.chip')).not.toBeNull();
   expect(document.querySelector('.sentence')).not.toBeNull();
   expect(document.querySelector('.definitions')).toBeNull();
   expect(document.querySelector('.def-row')).toBeNull();
   ```
2. Remove the tests that specifically test row marking on chip clicks:
   Remove `a chip click marks its definition row` and `keyboard activation marks the same row a click does`.

- [ ] **Step 2: Delete `definitions.ts` and `definitions.test.ts`**

Delete:
- `src/render/definitions.ts`
- `src/render/definitions.test.ts`

- [ ] **Step 3: Update `src/main.ts`**

1. Remove:
   ```ts
   import { renderDefinitions } from './render/definitions';
   ```
2. In `show(result: ParseResult)`:
   Replace lines 646-664 with:
   ```ts
   const sentence = renderSentence(result);

   parseError.replaceChildren();
   output.replaceChildren(sentence);
   ```

- [ ] **Step 4: Clean up CSS rules**

1. In `src/styles/global.css`:
   - Line 42: Change `.sentence { margin-bottom: var(--space-pane); }` to `.sentence {}` or delete the `margin-bottom` property.
   - Delete all lines from `.def-row {` through `details summary { ... }` (lines 140-181).
2. In `src/styles/typography.css`:
   - Delete lines 13-16:
     ```css
     .definitions {
       font-family: var(--font-ui);
       font-size: var(--text-gloss);
     }
     ```

- [ ] **Step 5: Run tests and typecheck**

Run:
```bash
npm test && npx tsc --noEmit
```
Verify all Vitest tests pass cleanly.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat: remove the definitions pane from the main window"
```

---

## Task 2: Update e2e tests and regenerate screenshot baselines

**Files:**
- Modify: `e2e/panes.spec.ts`
- Regenerate: `e2e/panes.spec.ts-snapshots/*.png`

**Interfaces:**
- Consumes: Updated DOM layout from Task 1
- Produces: Green Playwright suite and updated baseline screenshots.

- [ ] **Step 1: Update `e2e/panes.spec.ts`**

1. In `test('panes render at ...')`:
   Replace:
   ```ts
   await expect(page.locator('.chip').first()).toBeVisible();
   await expect(page.locator('.def-row')).toHaveCount(2);
   ```
   with:
   ```ts
   await expect(page.locator('.chip').first()).toBeVisible();
   await expect(page.locator('.sentence')).toBeVisible();
   await expect(page.locator('.def-row')).toHaveCount(0);
   ```
2. Remove `test('a chip click marks its definition row')` and `test('keyboard activation marks the same row a click does, on Enter and Space')`.
3. In `test('dark theme matches data-theme override ...')` and `test('data-theme override takes precedence over system dark mode')`:
   Replace `.def-row` locator checks with `.chip` or `.panes` / `.sentence` checks.

- [ ] **Step 2: Run Playwright tests and update snapshots**

Run:
```bash
npx playwright test -u
```
Verify all Playwright tests pass and screenshots are regenerated.

- [ ] **Step 3: Run the full gate**

Run:
```bash
npm test && npx tsc --noEmit && cargo test && cargo clippy --workspace --all-targets -- -D warnings
```
Verify all checks pass.

- [ ] **Step 4: Commit**

```bash
git add e2e/
git commit -m "test: update e2e specs and baselines for definitions pane removal"
```
