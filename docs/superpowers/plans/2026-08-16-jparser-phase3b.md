# JParser Phase 3B — Gloss & Definition Filters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the three JMdict Gloss & Definition Filters (`hide_pos`, `hide_xrefs`, `hide_usage`) in ClipYomi's webview UI, configurable via a Settings (⚙) menu dropdown in the header and persisted to user settings, allowing users to customize definition density in tooltip popovers.

**Architecture:** `src/render/tooltip-text.ts` formats sense lines by composing POS labels, sense numbering, glosses, usage annotations (`<misc>`, `<s_inf>`), cross-references (`<xref>`), and common word flags (`(P)`), filtering them according to `GlossFilters`. The secondary webview in `src/popover.ts` receives filters alongside entries to render the popover. The main window header in `src/main.ts` provides a Settings button (⚙) with a dropdown menu containing filter toggles, backed by Tauri `settings.rs` persistence.

**Tech Stack:** TypeScript, HTML/CSS, Vitest, Rust / Tauri 2.

**Spec:** [`docs/superpowers/specs/2026-08-16-jparser-phase3b-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-16-jparser-phase3b-design.md)

## Global Constraints

- Zero additional external crate dependencies added to `src-tauri/Cargo.toml` or `Cargo.toml`.
- All Rust code must pass `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.
- All TypeScript code must pass `npm test` and `npx tsc --noEmit`.
- Lexical colourising compatibility must be preserved without modifying `src/render/tooltip-colour.ts`.

---

### Task 1: Data Contracts & Settings Persistence

**Files:**
- Modify: `src/types.ts`
- Modify: `src-tauri/src/settings.rs`
- Test: `src-tauri/src/settings.rs`

**Interfaces:**
- Produces:
  - TypeScript interface `GlossFilters { hide_pos?: boolean; hide_xrefs?: boolean; hide_usage?: boolean; }`
  - Fields `hide_pos?: boolean; hide_xrefs?: boolean; hide_usage?: boolean;` on `Settings` in `src/types.ts`
  - Fields `pub hide_pos: Option<bool>`, `pub hide_xrefs: Option<bool>`, `pub hide_usage: Option<bool>` on `Settings` struct in `src-tauri/src/settings.rs`

- [ ] **Step 1: Write Rust unit test in `src-tauri/src/settings.rs` for gloss filter settings**

```rust
#[test]
fn settings_roundtrips_gloss_filters() {
    let settings = Settings {
        always_on_top: true,
        clipboard_monitoring: true,
        decorations: false,
        furigana_mode: Some("hiragana".to_string()),
        hide_pos: Some(true),
        hide_xrefs: Some(true),
        hide_usage: Some(false),
        window_width: Some(400),
        window_height: Some(300),
        window_x: None,
        window_y: None,
        extra: serde_json::Map::new(),
    };
    let json = serde_json::to_string(&settings).expect("serialization succeeds");
    assert!(json.contains("\"hide_pos\":true"));
    assert!(json.contains("\"hide_xrefs\":true"));
    assert!(json.contains("\"hide_usage\":false"));
    let deserialized: Settings = serde_json::from_str(&json).expect("deserialization succeeds");
    assert_eq!(deserialized.hide_pos, Some(true));
    assert_eq!(deserialized.hide_xrefs, Some(true));
    assert_eq!(deserialized.hide_usage, Some(false));
}
```

- [ ] **Step 2: Update `src-tauri/src/settings.rs`**

Add `hide_pos`, `hide_xrefs`, `hide_usage` to `Settings` struct and update `Default`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
    #[serde(default = "default_true")]
    pub decorations: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_pos: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_xrefs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_usage: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            clipboard_monitoring: true,
            decorations: true,
            furigana_mode: None,
            hide_pos: None,
            hide_xrefs: None,
            hide_usage: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            extra: serde_json::Map::new(),
        }
    }
}
```

- [ ] **Step 3: Update `src/types.ts`**

```typescript
export interface GlossFilters {
  hide_pos?: boolean;
  hide_xrefs?: boolean;
  hide_usage?: boolean;
}

export interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
  decorations: boolean;
  furigana_mode?: FuriganaMode;
  hide_pos?: boolean;
  hide_xrefs?: boolean;
  hide_usage?: boolean;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}
```

- [ ] **Step 4: Verify Rust tests and TypeScript type checking**

Run: `cargo test --manifest-path src-tauri/Cargo.toml` and `npx tsc --noEmit`

---

### Task 2: Gloss & Definition Filter Rendering Engine

**Files:**
- Modify: `src/render/tooltip-text.ts`
- Modify: `src/render/tooltip-text.test.ts`
- Modify: `src/render/tooltip.ts`

**Interfaces:**
- Consumes: `GlossFilters` from `src/types.ts`
- Produces:
  - `assembleTooltipText(entries: Entry[], filters?: GlossFilters): string`
  - `renderTooltip(entries: Entry[], filters?: GlossFilters): HTMLElement`

- [ ] **Step 1: Write failing unit tests in `src/render/tooltip-text.test.ts`**

Add tests covering:
1. Senses with `xrefs`, `misc`, `info` formatted correctly when unfiltered.
2. `hide_pos: true` omitting `(pos)` prefix.
3. `hide_xrefs: true` stripping `(see ...)`.
4. `hide_usage: true` stripping `(misc)` and `(info)`.
5. All three filters combined.

```typescript
test('renders full sense metadata (pos, misc, info, xrefs, common) when unfiltered', () => {
  const e = entry({
    senses: [
      {
        pos: ['v5r'],
        glosses: ['to say', 'to utter'],
        misc: ['uk'],
        info: ['usually written in kana'],
        xrefs: ['言われる'],
      },
    ],
    flags: ['primary', 'common'],
  });
  expect(assembleTooltipText([e])).toBe(
    '消える【きえる】\n(v5r) (1) to say/to utter/(uk)/(usually written in kana)/(see 言われる)/(P)',
  );
});

test('hide_pos strips the pos tag from sense lines', () => {
  const e = entry({
    senses: [
      {
        pos: ['v5r'],
        glosses: ['to say'],
        misc: [],
        info: [],
        xrefs: [],
      },
    ],
  });
  expect(assembleTooltipText([e], { hide_pos: true })).toBe(
    '消える【きえる】\n(1) to say',
  );
});

test('hide_xrefs strips cross references', () => {
  const e = entry({
    senses: [
      {
        pos: ['v5r'],
        glosses: ['to say'],
        misc: [],
        info: [],
        xrefs: ['言われる'],
      },
    ],
  });
  expect(assembleTooltipText([e], { hide_xrefs: true })).toBe(
    '消える【きえる】\n(v5r) (1) to say',
  );
});

test('hide_usage strips misc and s_inf usage notes', () => {
  const e = entry({
    senses: [
      {
        pos: ['v5r'],
        glosses: ['to say'],
        misc: ['uk'],
        info: ['usually written in kana'],
        xrefs: [],
      },
    ],
  });
  expect(assembleTooltipText([e], { hide_usage: true })).toBe(
    '消える【きえる】\n(v5r) (1) to say',
  );
});

test('combined filters strip pos, xrefs, and usage simultaneously', () => {
  const e = entry({
    senses: [
      {
        pos: ['v5r'],
        glosses: ['to say', 'to utter'],
        misc: ['uk'],
        info: ['usually written in kana'],
        xrefs: ['言われる'],
      },
    ],
    flags: ['primary', 'common'],
  });
  expect(
    assembleTooltipText([e], { hide_pos: true, hide_xrefs: true, hide_usage: true }),
  ).toBe('消える【きえる】\n(1) to say/to utter/(P)');
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `npm test`
Expected: FAIL on new filter test cases

- [ ] **Step 3: Implement `assembleTooltipText` in `src/render/tooltip-text.ts`**

```typescript
import type { Entry, GlossFilters } from '../types';

export const CONJ_MARKER = '\u0001';

/** One entry's block: an optional conjugation line, a headword line, then senses. */
function entryLines(entry: Entry, filters?: GlossFilters): string[] {
  const lines: string[] = [];

  if (entry.conjugation !== null) lines.push(`${CONJ_MARKER}${entry.conjugation}`);

  lines.push(entry.reading === null ? entry.headword : `${entry.headword}【${entry.reading}】`);

  const common = entry.flags.includes('common');
  entry.senses.forEach((sense, i) => {
    const parts: string[] = [...sense.glosses];

    if (!filters?.hide_usage) {
      for (const m of sense.misc) {
        parts.push(`(${m})`);
      }
      for (const inf of sense.info) {
        parts.push(`(${inf})`);
      }
    }

    if (!filters?.hide_xrefs) {
      for (const xr of sense.xrefs) {
        parts.push(`(see ${xr})`);
      }
    }

    if (common && i === entry.senses.length - 1) {
      parts.push('(P)');
    }

    const posPrefix =
      !filters?.hide_pos && sense.pos.length > 0 ? `(${sense.pos.join(',')}) ` : '';
    const glosses = parts.join('/');
    lines.push(`${posPrefix}(${i + 1}) ${glosses}`);
  });

  return lines;
}

export function assembleTooltipText(entries: Entry[], filters?: GlossFilters): string {
  return entries.flatMap((e) => entryLines(e, filters)).join('\n');
}
```

- [ ] **Step 4: Update `src/render/tooltip.ts`**

```typescript
import type { Entry, GlossFilters } from '../types';
import { assembleTooltipText } from './tooltip-text';
import { colourLine } from './tooltip-colour';

export function renderTooltip(entries: Entry[], filters?: GlossFilters): HTMLElement {
  const root = document.createElement('div');
  root.className = 'tt';

  for (const line of assembleTooltipText(entries, filters).split('\n')) {
    const el = document.createElement('div');
    el.className = 'tt-line';
    for (const run of colourLine(line)) {
      const span = document.createElement('span');
      span.className = `tt-${run.kind}`;
      span.textContent = run.text;
      el.append(span);
    }
    if (!el.hasChildNodes()) el.append(document.createElement('br'));
    root.append(el);
  }

  return root;
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm test`
Expected: PASS on all tests in `src/render/`

---

### Task 3: Popover Webview IPC Contract

**Files:**
- Modify: `src/popover.ts`
- Modify: `src/main.ts`

**Interfaces:**
- `popover-content` event payload: `Entry[] | { entries: Entry[]; filters?: GlossFilters }`

- [ ] **Step 1: Update `src/popover.ts` to handle payload with filters**

```typescript
import { emit, listen } from '@tauri-apps/api/event';
import { renderTooltip } from './render/tooltip';
import type { Entry, GlossFilters } from './types';
import './styles/tooltip.css';

const tooltip = document.querySelector<HTMLElement>('#tooltip')!;
const VERTICAL_PADDING = 6;

type PopoverPayload = Entry[] | { entries: Entry[]; filters?: GlossFilters };

void listen<PopoverPayload>('popover-content', (e) => {
  const payload = e.payload;
  const entries = Array.isArray(payload) ? payload : payload.entries;
  const filters = Array.isArray(payload) ? undefined : payload.filters;

  const content = renderTooltip(entries, filters);
  tooltip.replaceChildren(content);
  tooltip.scrollTop = 0;

  const chromeWidth = tooltip.offsetWidth - tooltip.clientWidth;
  const chromeHeight = tooltip.offsetHeight - tooltip.clientHeight;
  void emit('popover-measured', {
    width: tooltip.scrollWidth + chromeWidth,
    height: content.getBoundingClientRect().height + VERTICAL_PADDING + chromeHeight,
  }).catch(() => {});
});
```

- [ ] **Step 2: Verify `src/popover.ts` compiles cleanly**

Run: `npx tsc --noEmit`

---

### Task 4: Settings Menu UI (⚙) & State Persistence

**Files:**
- Modify: `src/main.ts`
- Modify: `src/styles/global.css`
- Modify: `src/main.test.ts`
- Modify: `src/main-tooltip.test.ts`

- [ ] **Step 1: Add HTML markup in `src/main.ts`**

Add `#settings-toggle` button and `#settings-menu` dropdown inside `<header class="controls">`:

```html
<header class="controls" data-tauri-drag-region>
  <button id="always-on-top" type="button" aria-pressed="false">Always on top</button>
  <button id="monitor" type="button" aria-pressed="true">Monitoring</button>
  <button id="decorations" type="button" aria-pressed="true">Title bar</button>
  <div class="segmented-control" role="radiogroup" aria-label="Furigana mode">
    <button type="button" role="radio" data-mode="none" aria-checked="true" title="No furigana">—</button>
    <button type="button" role="radio" data-mode="hiragana" aria-checked="false" title="Hiragana furigana">ひ</button>
    <button type="button" role="radio" data-mode="katakana" aria-checked="false" title="Katakana furigana">カ</button>
    <button type="button" role="radio" data-mode="romaji" aria-checked="false" title="Romaji phonetic">R</button>
  </div>
  <div class="settings-container">
    <button id="settings-toggle" type="button" aria-haspopup="true" aria-expanded="false" title="Settings">⚙</button>
    <div id="settings-menu" class="settings-menu" hidden>
      <div class="settings-section-title">Gloss Filters</div>
      <label class="settings-item">
        <input type="checkbox" id="filter-hide-pos">
        <span>Hide parts of speech (POS)</span>
      </label>
      <label class="settings-item">
        <input type="checkbox" id="filter-hide-xrefs">
        <span>Hide cross-references</span>
      </label>
      <label class="settings-item">
        <input type="checkbox" id="filter-hide-usage">
        <span>Hide usage notes</span>
      </label>
    </div>
  </div>
</header>
```

- [ ] **Step 2: Add CSS styling in `src/styles/global.css`**

Style `.settings-container`, `#settings-toggle`, `.settings-menu`, `.settings-section-title`, `.settings-item`.

- [ ] **Step 3: Wire up settings interaction, dropdown behavior, and persistence in `src/main.ts`**

1. Maintain `currentFilters: GlossFilters = { hide_pos: false, hide_xrefs: false, hide_usage: false }`.
2. Toggle dropdown on `#settings-toggle` click; close on outside click or `Escape`.
3. Listen for changes on `#filter-hide-pos`, `#filter-hide-xrefs`, `#filter-hide-usage`.
4. On change, update `currentFilters`, invoke `save_settings`, and pass `currentFilters` in `openFor()` when emitting `popover-content`.
5. Update `applySettings()` to populate checkboxes from loaded settings.

- [ ] **Step 4: Update `src/main.test.ts` and `src/main-tooltip.test.ts`**

Add tests verifying:
- Settings menu opens on click, closes on Escape or outside click.
- Filter checkboxes toggle and trigger `save_settings`.
- `openFor()` emits `{ entries, filters: currentFilters }`.
- `applySettings()` initializes checkbox states.

- [ ] **Step 5: Run full Vitest suite**

Run: `npm test`
Expected: PASS on all test files.

---

### Task 5: End-to-End Verification Gate & Handoff

**Files:**
- Create: `docs/superpowers/2026-08-16-jparser-phase3b-handoff.md`

- [ ] **Step 1: Run complete verification suite**

1. `npx tsc --noEmit`
2. `npm test`
3. `cargo test --workspace`
4. `cargo clippy --workspace --all-targets -- -D warnings`

- [ ] **Step 2: Create handoff document `docs/superpowers/2026-08-16-jparser-phase3b-handoff.md`**
