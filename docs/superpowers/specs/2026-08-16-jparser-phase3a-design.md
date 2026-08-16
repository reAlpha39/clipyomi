# JParser Phase 3A — Furigana Display Modes Design

**Date:** 2026-08-16  
**Status:** Approved design, ready for implementation planning  
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§5.6, §7.2, §8, §11)  

---

## 1. Goal

Implement the four Furigana display modes (`none`, `hiragana`, `katakana`, `romaji`) in ClipYomi's webview UI, selectable via a header segmented control `[— ひ カ R]` and persisted to user settings, providing immediate visual phonetic annotations **on top of** segmented Japanese word chips without altering the underlying dictionary/parser core.

---

## 2. Display Modes & Rendering Rules

All phonetic annotations (`hiragana`, `katakana`, `romaji`) render uniformly on top of the word chips using standard HTML `<ruby><rt>...</rt></ruby>` elements.

| Mode | Key | Header Label | Rendering Behavior | Example (`東京`) | Example (`これ`) |
|---|---|---|---|---|---|
| **None** | `none` | `—` | Standard plain text; no phonetic annotations. | `<button class="chip kanji">東京</button>` | `<button class="chip kana">これ</button>` |
| **Hiragana** | `hiragana` | `ひ` | `<ruby>` reading annotation on kanji-bearing segments only. | `<button class="chip kanji"><ruby>東京<rt>とうきょう</rt></ruby></button>` | `<button class="chip kana">これ</button>` |
| **Katakana** | `katakana` | `カ` | `<ruby>` reading converted to Katakana on kanji segments only. | `<button class="chip kanji"><ruby>東京<rt>トウキョウ</rt></ruby></button>` | `<button class="chip kana">これ</button>` |
| **Romaji** | `romaji` | `R` | `<ruby>` phonetic romanization rendered on top of **all** segments. | `<button class="chip kanji"><ruby>東京<rt class="romaji">toukyou</rt></ruby></button>` | `<button class="chip kana"><ruby>これ<rt class="romaji">kore</rt></ruby></button>` |

### Scope Rules
- **Hiragana / Katakana modes**:
  - Render `<ruby>` with `<rt>` only when `/[一-鿿]/.test(segment.surface)` is true and `segment.reading` is non-null.
  - Kana-only segments (e.g. `これ`, `は`) and unmatched punctuation do not generate ruby markup.
- **Romaji mode**:
  - Renders `<ruby>` with `<rt class="romaji">` on top of all matched segments (using `segment.reading ?? segment.surface`) and unmatched kana words.
  - Particle romanization fixups: `は` as a particle romanizes to `wa` (preserving `cha` in non-particle contexts), `へ` as a particle romanizes to `e`.
  - Double consonants (sokuon `っ`/`ッ`) double the subsequent consonant (e.g. `いった` → `itta`).
  - Syllabic `ん`/`ン` appends an apostrophe before vowels and ya-row kana (e.g. `かんい` → `kan'i`).

---

## 3. Architecture & Component Design

```
src/
├── types.ts                  # Added FuriganaMode type & Settings.furigana_mode
├── render/
│   ├── furigana.ts           # toKatakana, toRomaji, furiganaFor helper functions
│   ├── furigana.test.ts      # Unit tests for transliteration & annotation rules
│   ├── sentence.ts           # renderSentence(result, mode) with <ruby>/<rt> layout
│   └── sentence.test.ts      # Snapshot/DOM tests across all 4 modes
├── styles/
│   └── global.css            # Styles for <ruby>, <rt>, <rt.romaji>, .segmented-control
├── main.ts                   # Segmented control in header, click handlers, persistence
└── main.test.ts              # Header control and settings round-trip integration tests
src-tauri/
└── src/
    └── settings.rs           # Updated Rust Settings struct with furigana_mode
```

---

## 4. Transliteration Specifications (`src/render/furigana.ts`)

### 4.1 Hiragana to Katakana (`toKatakana`)
- For each character in the input string:
  - If code point is in range `0x3041..=0x3096`, offset by `+0x60` (`0x30A1..=0x30F6`).
  - All other characters (kanji, punctuation, existing katakana, Latin) pass through untouched.

### 4.2 Romaji Conversion (`toRomaji`)
Port of `ta-old`'s `romajiTable` / `crates/jparser/src/romaji.rs`:
- **Digraph Priority**: Checks 2-character katakana combinations first (`キャ` → `kya`, `シャ` → `sha`, `チャ` → `cha`, `ヴァ` → `va`, etc.).
- **Single Kana**: Maps individual kana (`ア` → `a`, `カ` → `ka`, `サ` → `sa`, etc.).
- **Sokuon (`っ` / `ッ`)**: Doubles the initial consonant of the following token (e.g. `ッパ` → `ppa`, `ッチ` → `cchi` / `tchi`).
- **Hatsuon (`ん` / `ン`)**: Maps to `n`, inserting `'` if immediately preceding a vowel (`a, i, u, e, o`) or y-sound (`ya, yu, yo`).
- **Particle overrides**: If `isParticle` is true, `は` / `ハ` → `wa`, `へ` / `ヘ` → `e`.

### 4.3 Annotation Resolution (`furiganaFor`)
- If `mode === 'none'` → returns `null`.
- If `mode === 'hiragana'` → returns `segment.reading` if `/[一-鿿]/.test(segment.surface)`, else `null`.
- If `mode === 'katakana'` → returns `toKatakana(segment.reading)` if `/[一-鿿]/.test(segment.surface)`, else `null`.
- If `mode === 'romaji'` → returns `toRomaji(segment.reading ?? segment.surface, isParticle)`.

---

## 5. UI Layout & Styling (`src/styles/global.css`)

### 5.1 Ruby Typography
```css
ruby {
  ruby-align: center;
}

rt {
  font-size: var(--text-furigana, 0.58em);
  color: var(--color-text-muted, #8e8e93);
  user-select: none;
  line-height: 1;
  text-align: center;
}

rt.romaji {
  font-family: var(--font-mono, monospace);
  font-size: var(--text-furigana-romaji, 0.52em);
  letter-spacing: -0.02em;
}
```

### 5.2 Segmented Control Header
```css
.segmented-control {
  display: inline-flex;
  border-radius: 4px;
  background: var(--surface-control, rgba(255, 255, 255, 0.08));
  padding: 2px;
  gap: 2px;
}

.segmented-control button {
  padding: 2px 8px;
  font-size: 11px;
  font-weight: 500;
  border-radius: 3px;
  border: none;
  background: transparent;
  color: var(--color-text-muted, #8e8e93);
  cursor: pointer;
}

.segmented-control button[aria-checked="true"] {
  background: var(--surface-active, rgba(255, 255, 255, 0.18));
  color: var(--color-text, #ffffff);
}
```

---

## 6. Settings Persistence

- Key in `Settings`: `furigana_mode?: FuriganaMode;`
- Default value: `"none"`
- When user clicks any mode button:
  1. `currentFuriganaMode` updates.
  2. Header buttons update `aria-checked` states.
  3. `renderSentence(lastParseResult, currentFuriganaMode)` re-renders immediately.
  4. Async `invoke('save_settings', { settings })` persists selection.

---

## 7. Testing Strategy

1. **Unit Tests (`src/render/furigana.test.ts`)**:
   - `toKatakana`: complete mapping & non-hiragana passthrough.
   - `toRomaji`: basic kana, digraphs, sokuon doubling, hatsuon apostrophe, particle rules.
   - `furiganaFor`: correct mode resolution and kanji check.
2. **DOM / Component Tests (`src/render/sentence.test.ts`)**:
   - Verify `<ruby>` generated on top of kanji for `hiragana`/`katakana`.
   - Verify `<ruby><rt class="romaji">` generated on top of all segments in `romaji`.
   - Verify `none` renders plain chips.
3. **Integration Tests (`src/main.test.ts`)**:
   - Setting restoration on launch.
   - Click handling on `[— ひ カ R]` segmented control.
4. **Rust Settings Unit Tests (`src-tauri/src/settings.rs`)**:
   - Serde roundtrip tests for `furigana_mode`.
