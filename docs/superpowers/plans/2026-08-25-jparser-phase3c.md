# JParser Phase 3C — Font Sizes & Density Implementation Plan

> **Record, not a work order.** Unlike the plans for 1A–3B, this one was written
> after the phase shipped, so its boxes are checked. It exists because every
> earlier phase has a plan file and 4D will want to read what 3C introduced. The
> ordering below is the order the work actually ran in, each task RED before
> GREEN.

**Goal:** Bind `--text-sentence`, `--text-gloss`, `--text-furigana` and
`--text-furigana-romaji` to three persisted settings, controlled from the
settings window, applied in both webviews.

**Architecture:** A pure module, `src/font-sizes.ts`, owns the bounds, the
clamp, and the settings-to-token mapping. `main.ts` writes the tokens onto the
root element and forwards the gloss size with `popover-content`; `popover.ts`
applies that size before the render it measures. `settings.ts` owns the three
number inputs; `settings.rs` and `commands.rs` persist the keys.

**Tech Stack:** TypeScript, HTML/CSS, Vitest, Rust / Tauri 2.

**Spec:** [`docs/superpowers/specs/2026-08-25-jparser-phase3c-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-25-jparser-phase3c-design.md)

## Global Constraints

- Zero additional dependencies, either crate or npm.
- `cargo test`, `npm test`, `npx tsc --noEmit` and `cargo clippy --all-targets`
  green before the phase is called done.
- `tokens.css` untouched: it stays the single place a font-size default lives,
  and no default may be restated in TypeScript or Rust.
- Clamp at every boundary the settings file can reach — it is hand-editable.

---

### Task 1: The pure module

**Files:** create `src/font-sizes.ts`, `src/font-sizes.test.ts`; modify `src/types.ts`

- [x] **Step 1:** Write `src/font-sizes.test.ts` — px mapping, the furigana em
      ratio, the 0.9 romaji derivation, clamping from a hand-edited file,
      non-finite input, and (added later, see Task 5) the empty-value clear
      contract.
- [x] **Step 2:** Watch it fail against a stub module returning `[]` and `null`,
      so the failure is an assertion and not an import error.
- [x] **Step 3:** Implement `FONT_LIMITS`, `clampFont`, `fontVars`. Add
      `font_sentence`, `font_gloss`, `furigana_scale` to `Settings` in
      `src/types.ts`.

**Verify:** `npx vitest run src/font-sizes.test.ts` — 12 passed.

---

### Task 2: The main window applies the tokens

**Files:** modify `src/main.ts`, `src/main.test.ts`

- [x] **Step 1:** Write tests — sizes applied on load, absent sizes leave the
      tokens untouched, a `settings-changed` event restyles the root, an
      out-of-range file value clamps.
- [x] **Step 2:** Watch three of the four fail on an empty `style` attribute.
- [x] **Step 3:** Add `applyFontSizes`, called from `applySettings` and from the
      `settings-changed` listener.

**Verify:** `npx vitest run src/main.test.ts` — 48 passed.

---

### Task 3: The gloss size reaches the tooltip webview

**Files:** modify `src/main.ts`, `src/popover.ts`, `src/main.test.ts`; create `src/popover.test.ts`

- [x] **Step 1:** Write main-side tests — the payload carries the clamped size,
      and omits the key when nothing is persisted.
- [x] **Step 2:** Extend the `popover-content` payload with an optional `gloss`,
      tracked in `currentGloss`.
- [x] **Step 3:** Write `src/popover.test.ts`, the first test file for that
      entry point: the size reaches `--text-gloss`, clamps, is absent-safe, and
      a bare `Entry[]` payload still renders.
- [x] **Step 4:** Apply the size in `popover.ts` **before** `renderTooltip`, so
      the measurement that sizes the window sees the new text.

**Verify:** `npx vitest run src/popover.test.ts src/main.test.ts` — green.

---

### Task 4: The settings window controls

**Files:** modify `src/settings.ts`, `src/settings.test.ts`, `src/styles/settings.css`

- [x] **Step 1:** Write tests — inputs initialize from `get_settings`, a change
      saves and broadcasts, a typed-past-the-limit value is pinned back into
      range, an emptied box sends no key, an incoming event mirrors into the
      input.
- [x] **Step 2:** Add the "Text Size" group with three number inputs, bounds
      read from `FONT_LIMITS`, and `sizeOf` clamping on the way into the
      snapshot.
- [x] **Step 3:** Add `.settings-number-row` to `settings.css`.

**Verify:** `npx vitest run src/settings.test.ts` — 10 passed.

---

### Task 5: Persistence, and making a cleared box stay cleared

**Files:** modify `src-tauri/src/settings.rs`, `src-tauri/src/commands.rs`, `src/font-sizes.ts`, `src/main.ts`

- [x] **Step 1:** Write the Rust round-trip test and an assertion that an unset
      size is absent from the written JSON.
- [x] **Step 2:** Add the three `Option<u32>` fields; fix the two exhaustive
      struct literals in the existing tests.
- [x] **Step 3:** Write `save_settings_persists_the_font_sizes`, watch it fail,
      wire the command.
- [x] **Step 4:** Gap found while wiring — an omitted key left the old size in
      place, on disk and on the root element, so a cleared box could not stay
      cleared. Write the three failing tests for it (Rust: the payload omits a
      size; TS: a `settings-changed` without the key clears the token, and the
      popover stops receiving the size).
- [x] **Step 5:** `fontVars` returns an empty value for an unset size,
      `applyFontSizes` turns that into `removeProperty`, and `save_settings`
      assigns the three sizes rather than merging them. Update the four
      `font-sizes` tests that encoded the old omit-the-pair contract.

**Verify:** `cargo test` — 72 passed. `npx vitest run` — 159 passed.

---

### Task 6: Whole-suite gate

- [x] `npx vitest run` — 159 passed, 10 files.
- [x] `cargo test` in `src-tauri` — 72 passed.
- [x] `npx tsc --noEmit` — clean.
- [x] `cargo clippy --all-targets` — clean but for four pre-existing Win32
      acronym warnings.
- [x] `npm run test:e2e` — 21 passed, 1 failed: `e2e/popover.spec.ts:72`.
      Pre-existing on Windows; reproduced with every `src` and `src-tauri`
      change stashed. Not this phase's, not fixed here.
- [x] Manual check in a debug build (2026-08-25). Port 1420 was held by an
      older vite server, so the Rust half ran directly against it with `cargo
      run --no-default-features`. The three boxes read 24 / 14 / 58 on a profile
      with nothing persisted, confirmed by the user.
- [x] Follow-up shipped as `9316555`: the boxes opened empty on a first run.
      `stylesheetDefault` reads the value back from the token, which needed
      `--text-furigana` stated in `tokens.css` rather than living only as the
      fallback arm of `var(--text-furigana, 0.58em)`.

> Note for anyone testing a frontend change against a running debug build: the
> settings window is created hidden in `setup` and only shown and hidden after
> that, so it never navigates again. A CSS edit that lands after launch can miss
> it while a JS edit has already been picked up — which shows up as some boxes
> filled and others empty. Restart the app rather than debugging the symptom.
