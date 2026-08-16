# JParser Phase 3B — Gloss & Definition Filters Design

**Date:** 2026-08-16
**Status:** Approved design, ready for implementation planning
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.2, §7.6, §8)

---

## 1. Goal

Implement the three JMdict Gloss & Definition Filters (`hide_pos`, `hide_xrefs`, `hide_usage`) in ClipYomi, persisted in `settings.json` and configurable via a Settings (⚙) menu dropdown in the header, allowing users to customize definition density in tooltip popovers without modifying the core parser/indexer.

---

## 2. Filter Specifications & Formatting Rules

In JMdict, entries contain structured senses with parts of speech (`<pos>`), gloss translations (`<gloss>`), cross-references (`<xref>`), usage annotations (`<s_inf>`), and miscellaneous tags (`<misc>`).

| Filter Setting                  | Key in Settings | Default   | Behavior when TRUE (Checked / Filtered)                                           | Behavior when FALSE (Normal / Unfiltered)                                                                   |
| ------------------------------- | --------------- | --------- | --------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **Hide POS**              | `hide_pos`    | `false` | Omits the`(pos)` prefix from sense lines (e.g. `(1) to disappear/to vanish`). | Includes`(${pos.join(',')}) ` prefix when pos is non-empty (e.g. `(v1,vi) (1) to disappear/to vanish`). |
| **Hide Cross-References** | `hide_xrefs`  | `false` | Drops all`<xref>` cross-reference annotations.                                  | Formats each`<xref>` as `(see ${xref})` appended to the sense.                                          |
| **Hide Usage Notes**      | `hide_usage`  | `false` | Drops all`<s_inf>` (`info`) and `<misc>` annotations.                       | Formats`<misc>` as `(${misc})` and `<s_inf>` as `(${info})` appended to the sense.                  |

### 2.1 Sense Line Assembly (`src/render/tooltip-text.ts`)

A sense line is formatted as:

```text
[ (pos) ] (N) gloss1/gloss2 [ /(misc) ] [ /(s_inf) ] [ /(see xref) ] [ /(P) ]
```

1. **POS Prefix**:
   - If `!filters.hide_pos` and `sense.pos.length > 0`: `(${sense.pos.join(',')}) `
   - If `filters.hide_pos` is `true` or `sense.pos` is empty: `""`
2. **Sense Number**:
   - `(${i + 1}) `
3. **Gloss & Annotation Tokens** (joined with `/`):
   - All translation glosses: `...sense.glosses`
   - If `!filters.hide_usage`:
     - Each misc code in `sense.misc`: `(${misc})`
     - Each info text in `sense.info`: `(${info})`
   - If `!filters.hide_xrefs`:
     - Each cross-reference in `sense.xrefs`: `(see ${xref})`
   - If `common` flag and `i === entry.senses.length - 1`: `(P)`

### 2.2 Lexical Colouring Compatibility (`src/render/tooltip-colour.ts`)

Because all filtered/unfiltered metadata tags (`(v1,vi)`, `(1)`, `(uk)`, `(usually written in kana)`, `(see 言われる)`, `(P)`) are enclosed in standard parentheses `(...)`, the existing lexical colouriser in `tooltip-colour.ts` automatically tags them with `kind: 'paren'` (`--tt-paren`), preserving consistent lexical styling with 0 syntax changes to `tooltip-colour.ts`.

---

## 3. Architecture & Data Flow

```
src/
├── types.ts                  # Added GlossFilters interface, updated Settings
├── render/
│   ├── tooltip-text.ts       # assembleTooltipText accepts optional GlossFilters
│   ├── tooltip-text.test.ts  # Tests for hide_pos, hide_xrefs, hide_usage permutations
│   ├── tooltip.ts            # renderTooltip passes GlossFilters to assembleTooltipText
│   ├── tooltip.test.ts       # DOM tests verifying rendered tooltip output with filters
│   └── popover.ts            # places popover window (existing)
├── styles/
│   ├── global.css            # Styles for #settings-toggle and #settings-menu
│   └── tooltip.css           # Tooltip typography (existing)
├── popover.ts                # Secondary webview entry: unpacks { entries, filters }
├── main.ts                   # Settings button (⚙), dropdown popover, persistence
└── main.test.ts              # Settings integration tests for gloss filters
src-tauri/
└── src/
    └── settings.rs           # Added hide_pos, hide_xrefs, hide_usage to Settings
```

### 3.1 IPC Event Contract

- When `main.ts` emits `popover-content`, it sends `{ entries: Entry[], filters: GlossFilters }` (or supports `Entry[]` directly for backward compatibility).
- `popover.ts` unpacks the payload and calls `renderTooltip(payload.entries, payload.filters)`.

---

## 4. UI & Settings Menu Specifications

### 4.1 Header Settings Button

- Button `#settings-toggle` in `<header class="controls">`:
  - `type="button"`, `aria-haspopup="true"`, `aria-expanded="false"`, `title="Settings"`, label `⚙`.
- Dropdown menu `#settings-menu`:
  - Hidden by default (`hidden` attribute / CSS `display: none`).
  - Positioned directly below the `⚙` button (`position: absolute`).
  - Contains checkboxes/toggles:
    - `<input type="checkbox" id="filter-hide-pos">` "Hide parts of speech (POS)"
    - `<input type="checkbox" id="filter-hide-xrefs">` "Hide cross-references (xrefs)"
    - `<input type="checkbox" id="filter-hide-usage">` "Hide usage notes & misc"
- Dismissal:
  - Clicking outside `#settings-menu` or pressing `Escape` closes the dropdown and sets `aria-expanded="false"`.
  - Focus returns gracefully to `#settings-toggle`.

### 4.2 Settings Persistence

- Toggling any filter updates in-memory filter state immediately.
- Asynchronously saves to `settings.json` via Tauri `save_settings`.
- On startup, `applySettings()` reads `get_settings` and sets checkbox states and active filter variables.

---

## 5. Verification & Testing Strategy

1. **Rust Tests**:
   - `settings_roundtrips_gloss_filters` in `src-tauri/src/settings.rs` testing serialization, deserialization, and unknown key preservation.
2. **TypeScript Unit Tests**:
   - `src/render/tooltip-text.test.ts`:
     - Test default unfiltered output (includes POS, misc, info, xrefs, common marker).
     - Test `hide_pos: true` (strips `(pos)` prefix).
     - Test `hide_xrefs: true` (strips `(see ...)`).
     - Test `hide_usage: true` (strips `(misc)` and `(s_inf)`).
     - Test all filters combined.
3. **Integration Tests**:
   - `src/main.test.ts` & `src/main-tooltip.test.ts`:
     - Test settings toggle menu interaction (open, close, escape key, outside click).
     - Test checkbox state synchronization with `Settings` object.
     - Test `popover-content` payload passes updated filters.
