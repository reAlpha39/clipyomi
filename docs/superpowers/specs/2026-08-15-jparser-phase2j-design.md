# JParser Phase 2J — Definitions Pane Removal (Design)

**Date:** 2026-08-15  
**Status:** Approved design, ready for implementation planning  
**Reference implementation:** `ta-old/` (Translation Aggregator, GPL v2)  
**Predecessor code:** the shipped Phase 2H/2I tree  

## 1. Goal & Rationale

In Phase 2I, the word definition tooltip graduated from an in-page DOM element to a dedicated, undecorated, always-on-top OS window (`popover.html` + `src/popover.ts`) with full lexical colouring and multi-monitor positioning.

With the tooltip operating as a real window, having a redundant, duplicate list of definitions (`.definitions`) rendered below the sentence in the main window is unnecessary. Phase 2J is a removal phase: it deletes the definitions pane and its supporting styles/tests, leaving the segmented sentence (`.sentence`) as the sole content in the main output area (`#output.panes`).

### In scope

- Delete `src/render/definitions.ts` and `src/render/definitions.test.ts`
- Update `src/main.ts` so `show()` renders only `renderSentence(result)` into `#output`
- Delete the chip-click `.marked` / `scrollIntoView` listener in `src/main.ts`
- Remove obsolete definition styles from `src/styles/global.css` and `src/styles/typography.css`
- Reduce minimum window dimensions in `src-tauri/tauri.conf.json` from 480×320 to 160×80 to allow a compact single-line sentence strip
- Update `src/main.test.ts` to assert the absence of `.definitions` and `.def-row`
- Update `e2e/panes.spec.ts` to remove definition row assertions and update/regenerate screenshot baselines
- Amend predecessor design specs where appropriate

### Out of scope / Non-goals

- No changes to the popover window (`popover.html`, `src/popover.ts`, `src/render/tooltip*.ts`)
- No changes to chip hover/focus behaviour (`mouseover` dwell and `focusin` still open the popover)
- No changes to backend Rust code (Cargo tests remain at 357 passing)
- Furigana rendering modes, font-size customization, and gloss filters remain fenced for Phase 3

---

## 2. Component & Code Removals

### 2.1 The Render Module
`src/render/definitions.ts` (which exports `renderEntry` and `renderDefinitions`) is deleted along with its unit test suite `src/render/definitions.test.ts`.

`renderEntry` was exclusively used by `definitions.ts`. `src/render/tooltip.ts` has its own dedicated renderer (`renderTooltip`, using `assembleTooltipText` and `colourLine`), so deleting `definitions.ts` causes no broken imports across the rest of the application.

### 2.2 Application Entry (`src/main.ts`)
1. Remove `import { renderDefinitions } from './render/definitions';`.
2. In `show(result)`:
   - Call `const sentence = renderSentence(result);`.
   - Remove the `sentence.addEventListener('click', ...)` listener that searched for `.def-row` by `chip.dataset.start` and toggled `.marked`.
   - Replace `output.replaceChildren(sentence, definitions)` with `output.replaceChildren(sentence)`.

### 2.3 Chip Interactions
- Chips remain `<button class="chip ...">` elements.
- **Hover:** `mouseover` triggers the 350ms dwell timer, opening the popover via `openFor(chip)`.
- **Keyboard:** `focusin` (Tab / Shift-Tab) immediately opens the popover.
- **Click:** A mouse click naturally gives focus to the chip, keeping the popover visible. The obsolete definition-row scrolling and `.marked` highlighting are eliminated.

---

## 3. Styling & Layout

### 3.1 `src/styles/global.css`
Remove the definition-specific CSS rules:
- `.def-row` and `.def-row.marked`
- `.entry-head`, `.headword`, `.conjugation`, `.reading`, `.senses`
- `details summary`
- Remove `margin-bottom: var(--space-pane);` on `.sentence` (since `.sentence` is now the sole child in `#output.panes`).

### 3.2 `src/styles/typography.css`
Remove the `.definitions` selector block (`font-family: var(--font-ui); font-size: var(--text-gloss);`).

### 3.3 `src/styles/tokens.css`
Retain all tokens, including `--text-gloss` (which is used by `src/styles/tooltip.css`).

### 3.4 Window Geometry (`src-tauri/tauri.conf.json`)
With both the text input (Phase 2H) and the definition list (Phase 2J) removed, the main window only needs to host the top control bar and the parsed sentence.

`src-tauri/tauri.conf.json` reduces `"minWidth"` from `480` to `160` and `"minHeight"` from `320` to `80`, enabling the user to resize the app down to a minimal single-line sentence reader. Default size remains `720×480`.

---

## 4. Testing & Verification

### 4.1 Unit & Integration Tests (`src/main.test.ts`)
- Remove assertions checking for `.def-row` existence and `.marked` classes on chip clicks.
- Add an explicit assertion confirming that `#output` renders `.sentence` and that no `.definitions` or `.def-row` elements exist in the DOM after receiving a `parse-result`.

### 4.2 End-to-End Tests (`e2e/panes.spec.ts`)
- Remove `.def-row` count checks and click/keyboard `.marked` row assertions.
- Verify that chips render and keyboard focus behaves correctly.
- Regenerate the ten screenshot baselines (`panes-compact-*.png`, `panes-default-*.png`) for local screenshot regression testing.

### 4.3 Backend & Static Analysis
- `npx tsc --noEmit` must pass cleanly with zero type errors.
- `cargo test` must hold at 357 passing tests.
- `cargo clippy --workspace --all-targets -- -D warnings` must remain clean.
