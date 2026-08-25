# JParser Phase 4C — Parse History Implementation Plan

**Goal:** A session-scoped ring buffer of 50 full `ParseResult`s with Back and
Forward in the header band, so clipboard monitoring can be left running without
each copy destroying the last parse.

**Architecture:** A pure module, `src/history.ts`, owns the buffer, the cursor,
the cap and the dedup rule. `main.ts` splits its `show()` into a `display()` that
renders whatever parse it is handed and a `show()` that records first — making
`display` the single writer of `lastResult`, so the hover lookup always resolves
against the sentence on screen. Nothing enters Rust.

**Tech Stack:** TypeScript, HTML/CSS, Vitest, Playwright.

**Spec:** [`docs/superpowers/specs/2026-08-25-jparser-phase4c-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-25-jparser-phase4c-design.md)

## Global Constraints

- Zero additional dependencies, crate or npm.
- No Rust changes. `cargo test` and `cargo clippy` are run to confirm 4C did not
  reach them, not as part of the work.
- `npm test` and `npx tsc --noEmit` green before the phase is called done.
- Every task RED before GREEN — the failing run is watched, not assumed.
- `50` appears once, as the exported `HISTORY_CAP`.

---

### Task 1: The pure module

**Files:** create `src/history.ts`, `src/history.test.ts`

- [ ] **Step 1:** Write `src/history.test.ts` against the API in spec §6.1, with
      a `parse(...surfaces)` helper so each entry is identifiable by its surface
      text alone. Cover: the empty state; a single entry offering no navigation;
      the two-entry walk; both ends holding rather than running off; `current()`
      before the first push, after a push, across a skipped duplicate, and as
      the cursor moves; consecutive dedup; a non-consecutive repeat *is* stored;
      the key ignores segment boundaries (`['猫','が']` equals `['猫が']`); a
      skipped duplicate leaves the cursor where it was; nav model A keeps the
      entries ahead of the cursor; eviction at `HISTORY_CAP`; the cursor still
      on the newest parse across an eviction.
- [ ] **Step 2:** Run it. Expect a transform failure on the missing import —
      that is the RED.
- [ ] **Step 3:** Implement `HISTORY_CAP`, the `History` interface and
      `createHistory()`. `push` compares joined surface text against the newest
      entry and returns early on a match; on a real push it appends, `shift()`s
      past the cap, and sets `cursor = buffer.length - 1` **from the length, not
      by incrementing** — that is what keeps the cursor off a slot the shift
      moved.

**Verify:** `npx vitest run src/history.test.ts` — green, with a test for every behaviour listed in Step 1.

---

### Task 2: The header buttons

**Files:** modify `src/main.ts`, `src/main.test.ts`

- [ ] **Step 1:** Write the integration tests in `src/main.test.ts`: both
      buttons start disabled; a single parse leaves them disabled; a second
      enables Back only; Back renders the previous parse and flips both disabled
      states; Forward returns; a hover after navigating emits `popover-content`
      carrying the **displayed** parse's entries; a new parse takes over the view
      and keeps the entries ahead of the cursor; a duplicate of the newest leaves
      the view where it is; a parse navigated to renders with the furigana mode
      current now.
- [ ] **Step 2:** Run. Expect every new test to fail with `Cannot read properties of null
      (reading 'click')` — the buttons do not exist yet.
- [ ] **Step 3:** Add the `<nav class="history-controls">` with `#history-back`
      and `#history-forward`, both `disabled`, ahead of `#settings-toggle` in the
      header template. Query them beside the other header elements.
- [ ] **Step 4:** Split `show()` per spec §6.2 into `display(result | null)` and
      `show(result)`, add `syncHistoryButtons()`, and wire the two click
      handlers to `display(history.back())` / `display(history.forward())`.
      `show` must display `history.current()`, never the parse it just pushed.

**Verify:** `npx vitest run src/history.test.ts src/main.test.ts` — all green,
including the pre-existing `main.test.ts` cases untouched by the split.

---

### Task 3: Styling, and the tab-stop trap it exposes

**Files:** modify `src/styles/global.css`, `src/main.ts`, `src/main.test.ts`

- [ ] **Step 1:** Add the `.history-controls` rules of spec §7 — inline-flex with
      a 2 px gap, `position: relative; z-index: 60` so the pair stays clickable
      above the band's drag region, 24 px square buttons, hover only when not
      disabled, `opacity: 0.45` when disabled.
- [ ] **Step 2:** Add `.history-controls` to both `[data-decorations='false']`
      rules (the idle hide and the `.peeked` reveal) and to the
      `prefers-reduced-motion` block.
- [ ] **Step 3:** Write the focus test: in frameless mode, dispatching `focus` on
      `#history-back` adds `peeked` to `#app`, `blur` plus the hide delay removes
      it, and `#history-forward` behaves the same. This is spec §7.1 — the pair
      sits ahead of the gear, so it becomes the first tab stop and an invisible
      one would be a WCAG 2.4.7 trap.
- [ ] **Step 4:** Watch it fail, then wire `showPeek`/`hidePeek` onto both
      buttons' `focus`/`blur`, mirroring what `settingsToggle` already does.

**Verify:** `npx vitest run` — full suite green. `npx tsc --noEmit` clean.

---

### Task 4: End-to-end and the baselines

**Files:** modify `e2e/panes.spec.ts`; regenerate `e2e/panes.spec.ts-snapshots/`

- [ ] **Step 1:** Add two assertions to the existing hidden-title-bar test:
      `nav.history-controls` resolves to `opacity: 0` while idle and `1` after
      the band is hovered. This is the only layer that can prove the shared CSS
      rule, since happy-dom applies no stylesheets.
- [ ] **Step 2:** Regenerate the ten full-page baselines that include the header
      (`panes-default-*`, `panes-compact-*`, `panes-activated-*`,
      `panes-download-offer-*`, `panes-download-downloading-*`) with
      `--update-snapshots`, and eyeball one dark and one light before accepting
      them. The chevrons at `opacity: 0.45` are faint at full-page scale — a
      cropped `header.controls` screenshot is the way to confirm they rendered
      at all rather than concluding from the full page that they did not.
- [ ] **Step 3:** Run the whole Playwright suite twice. Expect 21 passing and one
      failure whose identity *moves between runs* — the cold-dev-server race
      documented in spec §8. Confirm it is pre-existing by checking that a run
      with 4C stashed fails one test too. Do not chase it here; 6B owns the
      suite.

**Verify:** `npx playwright test e2e/panes.spec.ts` — 19 green.

---

### Task 5: Close the phase

**Files:** modify `docs/superpowers/specs/2026-08-12-jparser-port-design.md`

- [ ] **Step 1:** Run `cargo test` and `cargo clippy --all-targets` to confirm
      4C left Rust alone.
- [ ] **Step 2:** Add 4C to the §11 Phase 4 table as shipped, with the date and
      the one-line summary, the way 3A–3C are recorded.
- [ ] **Step 3:** Commit the implementation and the regenerated baselines. The
      spec and this plan are committed ahead of Task 1, so the history reads in
      the order the work was decided.

**Verify:** `npm test`, `npx tsc --noEmit`, `cargo test`, `cargo clippy
--all-targets` all clean; `git status` shows nothing stray.
