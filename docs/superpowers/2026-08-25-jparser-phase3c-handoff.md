# JParser Phase 3C — Handoff

**Date:** 2026-08-25
**Spec:** [`docs/superpowers/specs/2026-08-25-jparser-phase3c-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-25-jparser-phase3c-design.md)
**Plan:** [`docs/superpowers/plans/2026-08-25-jparser-phase3c.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/plans/2026-08-25-jparser-phase3c.md)

---

## 1. What Was Completed in Phase 3C

1. **Pure sizing module ([`src/font-sizes.ts`](file:///D:/Code/oss/clipyomi/src/font-sizes.ts))**:
   - `FONT_LIMITS` — sentence 12–48px, gloss 10–28px, furigana 30–100% of the
     sentence. One statement of the bounds, read by the number inputs and by
     every apply site.
   - `clampFont` — pins into range, rounds fractions, returns `null` for
     anything not a finite number.
   - `fontVars` — settings to `[token, value]` pairs, including
     `--text-furigana-romaji` derived at 0.9 × the furigana ratio. An unset size
     yields an empty value, which the callers turn into `removeProperty`.

2. **Data contracts & persistence ([`src/types.ts`](file:///D:/Code/oss/clipyomi/src/types.ts), [`src-tauri/src/settings.rs`](file:///D:/Code/oss/clipyomi/src-tauri/src/settings.rs), [`src-tauri/src/commands.rs`](file:///D:/Code/oss/clipyomi/src-tauri/src/commands.rs))**:
   - `font_sentence`, `font_gloss`, `furigana_scale` as optional numbers in
     TypeScript and `Option<u32>` in Rust, skipped when serializing.
   - No default restated on either side — `tokens.css` remains the only place a
     font-size default lives.
   - `save_settings` **assigns** these three rather than merging them, so an
     emptied box reaches disk as unset instead of resurrecting the old size.

3. **Main window application ([`src/main.ts`](file:///D:/Code/oss/clipyomi/src/main.ts))**:
   - `applyFontSizes` writes the four tokens onto `document.documentElement` at
     load and on every `settings-changed`. A token write only — nothing
     re-renders, and the ruby, chips and definition pane reflow themselves.

4. **Tooltip webview ([`src/popover.ts`](file:///D:/Code/oss/clipyomi/src/popover.ts))**:
   - The gloss size travels with `popover-content` because that webview has its
     own `:root`, and is applied **before** `renderTooltip` — the window sizes
     itself off what that render measures.
   - The payload union still accepts a bare `Entry[]` and a `{ entries, filters }`
     object from an older main window.

5. **Settings UI ([`src/settings.ts`](file:///D:/Code/oss/clipyomi/src/settings.ts), [`src/styles/settings.css`](file:///D:/Code/oss/clipyomi/src/styles/settings.css))**:
   - A "Text Size" group with three `<input type="number">` controls, bounds
     from `FONT_LIMITS`, clamped again on the way into the snapshot because
     `min`/`max` bound the steppers but not typing or a paste.
   - Clearing a box means "use the stylesheet default", end to end.

6. **Verification gate**:
   - 159/159 Vitest tests passing (10 files; `font-sizes.test.ts` and
     `popover.test.ts` are new — the tooltip entry point had no test file before).
   - 72/72 Rust tests passing in `clipyomi`.
   - 0 TypeScript errors; `cargo clippy --all-targets` clean but for four
     pre-existing Win32 acronym warnings.
   - `npm run test:e2e`: 21 passed, 1 failed —
     [`e2e/popover.spec.ts:72`](file:///D:/Code/oss/clipyomi/e2e/popover.spec.ts).
     **Pre-existing**: the same failure reproduces on Windows with every `src`
     and `src-tauri` change stashed. It belongs with the Windows work tracked in
     [`docs/superpowers/2026-08-18-windows-titlebar-test-plan.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/2026-08-18-windows-titlebar-test-plan.md).

**Not done:** a manual pass under `npm run tauri dev`. The three controls have
never been driven by hand.

---

## 2. Deviations from Master Spec §7.7

§7.7 asks for two inputs bound to `--text-cjk` and `--text-furigana`. Neither
token exists in this port. Three controls shipped instead — sentence, gloss,
furigana — bound to the tokens that do exist, with furigana kept as a ratio
rather than an absolute size. Reasoning is in §2 of the design doc.

---

## 3. Next Up

Phase 3 is complete: 3A furigana, 3B gloss filters, 3C font sizes. The reading
aids of §7 are all built.

Phase 4's order is a dependency chain, not a preference (master spec §11):

- **4C — parse history (§7.8).** Depends on nothing, and is the one users feel:
  a ring buffer of 50 full `ParseResult`s with back/forward in the header. What
  makes clipboard monitoring safe to leave running, since today every new copy
  destroys the previous parse.
- **4A — the other two chrome states (§7.4)**, then **4B — background alpha
  (§7.5)**, which 4A must clamp.
- **4D — layout hotkeys (§7.9)** last, because a slot captures geometry, chrome
  mode, background alpha, always-on-top, furigana mode **and the three font
  sizes this phase added**. `font_sentence`, `font_gloss` and `furigana_scale`
  are the keys it will need to snapshot and restore.

Separately open, unrelated to the phase track: the Windows titlebar test plan of
2026-08-18, still unreported, and the pre-existing `popover.spec.ts` failure
noted above.
