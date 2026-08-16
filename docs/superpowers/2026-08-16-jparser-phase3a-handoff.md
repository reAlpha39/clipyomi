# JParser Phase 3A — Handoff

**Date:** 2026-08-16  
**Status:** Completed and verified  
**Branch:** `develop`  
**Latest Commit:** `ce27f1a`  

---

## 1. What Was Completed in Phase 3A

1. **Data Contracts & Persistence**:
   - `FuriganaMode = 'none' | 'hiragana' | 'katakana' | 'romaji'` added to [`src/types.ts`](file:///D:/Code/oss/clipyomi/src/types.ts) and [`src-tauri/src/settings.rs`](file:///D:/Code/oss/clipyomi/src-tauri/src/settings.rs).
   - Rust settings serialization and round-trip verified.

2. **Frontend Transliteration Module ([`src/render/furigana.ts`](file:///D:/Code/oss/clipyomi/src/render/furigana.ts))**:
   - `toKatakana`: Pure-function conversion of Hiragana (`0x3041..=0x3096`) to Katakana via `+0x60`.
   - `toRomaji`: Full Hepburn romanization table supporting digraphs, sokuon (`っ`/`ッ`), syllabic `ん`/`ン` (with apostrophe before vowels and ya-row), and particle overrides (`は` → `wa`, `へ` → `e`).
   - `furiganaFor`: Resolves appropriate annotations based on active mode and kanji presence.
   - Comprehensive test suite in [`src/render/furigana.test.ts`](file:///D:/Code/oss/clipyomi/src/render/furigana.test.ts) (11 tests).

3. **Sentence Rendering & Ruby Typography ([`src/render/sentence.ts`](file:///D:/Code/oss/clipyomi/src/render/sentence.ts), [`src/styles/global.css`](file:///D:/Code/oss/clipyomi/src/styles/global.css))**:
   - Renders semantic HTML `<ruby><rt>...</rt></ruby>` on top of word chips for `hiragana`, `katakana`, and `romaji` modes.
   - Mode `none` renders plain chips without `<ruby>`.
   - CSS styling with `user-select: none` on `<rt>`.

4. **Header Segmented Control ([`src/main.ts`](file:///D:/Code/oss/clipyomi/src/main.ts))**:
   - Added `[— | ひ | カ | R]` segmented button group in header.
   - In-place 60fps re-rendering on click and async persistence to `settings.json`.

5. **Verification Gate**:
   - 102/102 Vitest tests passing.
   - 320/320 Rust tests passing.
   - 0 TypeScript compiler errors, 0 Clippy linter warnings.

---

## 2. Next Up: Phase 3B — Gloss & Definition Filters

According to Master Spec §7.6 (`docs/superpowers/specs/2026-08-12-jparser-port-design.md`):

1. **Filters to Implement**:
   - **Hide Cross-References**: Strip `<xref>` and `<ant>` from dictionary popover entries.
   - **Hide Usage Notes**: Strip `<s_inf>` and `<misc>` annotations.
   - **Hide POS**: Suppress part-of-speech labels (`<pos>`) for a compact definition layout.
2. **Files to Touch**:
   - `src/types.ts` & `src-tauri/src/settings.rs` (settings flags: `hide_xrefs`, `hide_usage`, `hide_pos`).
   - `src/render/tooltip-text.ts` / `src/render/popover.ts` (rendering filters).
   - `src/render/tooltip-text.test.ts` & `src/render/popover.test.ts` (unit tests).
