# JParser Phase 3B — Handoff

**Date:** 2026-08-16  
**Status:** Completed and verified  
**Branch:** `develop`  
**Latest Commit:** `812c008`  

---

## 1. What Was Completed in Phase 3B

1. **Data Contracts & Persistence ([`src/types.ts`](file:///D:/Code/oss/clipyomi/src/types.ts), [`src-tauri/src/settings.rs`](file:///D:/Code/oss/clipyomi/src-tauri/src/settings.rs), [`src-tauri/src/commands.rs`](file:///D:/Code/oss/clipyomi/src-tauri/src/commands.rs))**:
   - `GlossFilters` interface added with optional flags `hide_pos`, `hide_xrefs`, `hide_usage`.
   - Rust `Settings` struct updated with optional `hide_pos`, `hide_xrefs`, `hide_usage` booleans with Serde round-trip tests.
   - Tauri `save_settings` command updated to persist gloss filter settings to disk in `SettingsState`.

2. **Tooltip & Definition Rendering Engine ([`src/render/tooltip-text.ts`](file:///D:/Code/oss/clipyomi/src/render/tooltip-text.ts), [`src/render/tooltip.ts`](file:///D:/Code/oss/clipyomi/src/render/tooltip.ts))**:
   - `assembleTooltipText` supports `GlossFilters`:
     - `hide_pos`: Drops `(pos)` part-of-speech label from sense lines.
     - `hide_xrefs`: Drops `(see ${xref})` cross-references.
     - `hide_usage`: Drops `(${misc})` and `(${info})` (`<s_inf>`) usage notes.
     - Common marker `(P)` preserved on the final sense.
   - `renderTooltip` passes active filters through to `assembleTooltipText`.
   - Lexical colouring compatibility in [`src/render/tooltip-colour.ts`](file:///D:/Code/oss/clipyomi/src/render/tooltip-colour.ts) preserved without modification.
   - Comprehensive test suite in [`src/render/tooltip-text.test.ts`](file:///D:/Code/oss/clipyomi/src/render/tooltip-text.test.ts) (14 tests).

3. **Popover Webview IPC Contract ([`src/popover.ts`](file:///D:/Code/oss/clipyomi/src/popover.ts))**:
   - `popover-content` event handler accepts both backward-compatible `Entry[]` array and `{ entries: Entry[], filters?: GlossFilters }` object payloads.
   - Popover renders definitions with the active gloss filter configuration.

4. **Settings Menu UI (⚙) & Interaction ([`src/main.ts`](file:///D:/Code/oss/clipyomi/src/main.ts), [`src/styles/global.css`](file:///D:/Code/oss/clipyomi/src/styles/global.css))**:
   - Settings toggle button (`#settings-toggle`) in the header opening a dropdown menu (`#settings-menu`).
   - Checkboxes for `Hide parts of speech (POS)`, `Hide cross-references`, `Hide usage notes`.
   - Accessible ARIA attributes (`aria-haspopup`, `aria-expanded`) and dismissal via `Escape` key and outside clicks.
   - Real-time settings persistence on change and startup state synchronization in `applySettings()`.
   - Full integration test suites in [`src/main.test.ts`](file:///D:/Code/oss/clipyomi/src/main.test.ts) and [`src/main-tooltip.test.ts`](file:///D:/Code/oss/clipyomi/src/main-tooltip.test.ts).

5. **Verification Gate**:
   - 115/115 Vitest tests passing.
   - 289/289 Rust tests passing (236 jparser + 53 clipyomi).
   - 0 TypeScript compiler errors, 0 Clippy linter warnings.

---

## 2. Next Up: Phase 3C — Font Sizes & Density

According to Master Spec §7.7 (`docs/superpowers/specs/2026-08-12-jparser-port-design.md`):
- `normalFontSize` and `furiganaFontSize` bound to CSS tokens `--text-cjk` and `--text-furigana`.
- Font scale controls in Settings dropdown popover.
