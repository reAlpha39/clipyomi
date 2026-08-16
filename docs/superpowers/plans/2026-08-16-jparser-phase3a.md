# JParser Phase 3A — Furigana Display Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the four Furigana display modes (`none`, `hiragana`, `katakana`, `romaji`) in ClipYomi's webview UI, selectable via a header segmented control `[— ひ カ R]` and persisted to user settings, providing immediate visual phonetic annotations on top of segmented Japanese word chips.

**Architecture:** A pure-frontend transliteration engine in `src/render/furigana.ts` converts readings into Katakana or Romaji. `src/render/sentence.ts` generates semantic HTML `<ruby><rt>...</rt></ruby>` elements on top of word chips based on the active mode (`none`, `hiragana`, `katakana`, `romaji`). The header bar in `src/main.ts` hosts a segmented button group that updates the view in 60fps and persists the selection via Tauri `settings.rs`.

**Tech Stack:** TypeScript, HTML5 Ruby Typography, CSS3, Vitest, Rust / Tauri 2.

**Spec:** [`docs/superpowers/specs/2026-08-16-jparser-phase3a-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-16-jparser-phase3a-design.md)

## Global Constraints

- Zero additional external crate dependencies added to `src-tauri/Cargo.toml` or `Cargo.toml`.
- Zero webfonts added.
- All Rust code must pass `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.
- All TypeScript code must pass `npm test` and `npx tsc --noEmit`.
- Mode switching must be instantaneous (zero-latency client-side re-render).

---

### Task 1: Data Contracts & Settings Persistence

**Files:**
- Modify: `src/types.ts`
- Modify: `src-tauri/src/settings.rs`
- Test: `src-tauri/src/settings.rs`

**Interfaces:**
- Produces:
  - TypeScript type `FuriganaMode = 'none' | 'hiragana' | 'katakana' | 'romaji'`.
  - Field `furigana_mode?: FuriganaMode` on TypeScript `Settings` interface.
  - Field `pub furigana_mode: Option<String>` on Rust `Settings` struct.

- [ ] **Step 1: Write Rust unit test in `src-tauri/src/settings.rs` for `furigana_mode` serialization**

```rust
#[test]
fn settings_roundtrips_furigana_mode() {
    let settings = Settings {
        always_on_top: true,
        clipboard_monitoring: true,
        decorations: false,
        furigana_mode: Some("hiragana".to_string()),
        window_width: Some(400.0),
        window_height: Some(300.0),
        window_x: None,
        window_y: None,
    };
    let json = serde_json::to_string(&settings).expect("serialization succeeds");
    assert!(json.contains("\"furigana_mode\":\"hiragana\""));
    let deserialized: Settings = serde_json::from_str(&json).expect("deserialization succeeds");
    assert_eq!(deserialized.furigana_mode, Some("hiragana".to_string()));
}
```

- [ ] **Step 2: Update `src-tauri/src/settings.rs`**

Add `furigana_mode: Option<String>` to `Settings` and update default / serialization logic:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct Settings {
    pub always_on_top: bool,
    pub clipboard_monitoring: bool,
    pub decorations: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<f64>,
}
```

- [ ] **Step 3: Update `src/types.ts`**

```typescript
export type FuriganaMode = 'none' | 'hiragana' | 'katakana' | 'romaji';

export interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
  decorations: boolean;
  furigana_mode?: FuriganaMode;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}
```

- [ ] **Step 4: Verify Rust tests and clippy**

Run: `cargo test -p clipyomi`
Expected: All tests pass.

- [ ] **Step 5: Commit changes**

```bash
git add src/types.ts src-tauri/src/settings.rs
git commit -m "feat(types): add furigana_mode to frontend and backend Settings contracts"
```

---

### Task 2: Frontend Transliteration Module (`furigana.ts`)

**Files:**
- Create: `src/render/furigana.ts`
- Create: `src/render/furigana.test.ts`

**Interfaces:**
- Produces:
  - `export function toKatakana(text: string): string`
  - `export function toRomaji(text: string, isParticle = false): string`
  - `export function furiganaFor(segment: Segment, mode: FuriganaMode): string | null`

- [ ] **Step 1: Write comprehensive unit tests in `src/render/furigana.test.ts`**

```typescript
import { describe, expect, test } from 'vitest';
import { furiganaFor, toKatakana, toRomaji } from './furigana';
import type { Segment } from '../types';

describe('toKatakana', () => {
  test('converts basic hiragana to katakana', () => {
    expect(toKatakana('とうきょう')).toBe('トウキョウ');
    expect(toKatakana('あいうえお')).toBe('アイウエオ');
    expect(toKatakana('きゃきゅきょ')).toBe('キャキュキョ');
  });

  test('passes non-hiragana characters untouched', () => {
    expect(toKatakana('東京123!')).toBe('東京123!');
    expect(toKatakana('テスト')).toBe('テスト');
  });
});

describe('toRomaji', () => {
  test('converts basic kana to romaji', () => {
    expect(toRomaji('とうきょう')).toBe('toukyou');
    expect(toRomaji('にほん')).toBe('nihon');
    expect(toRomaji('すし')).toBe('sushi');
  });

  test('handles digraphs', () => {
    expect(toRomaji('きょうと')).toBe('kyouto');
    expect(toRomaji('しゃしん')).toBe('shashin');
    expect(toRomaji('おちゃ')).toBe('ocha');
    expect(toRomaji('ちょっと')).toBe('chotto');
  });

  test('handles sokuon double consonants', () => {
    expect(toRomaji('いった')).toBe('itta');
    expect(toRomaji('やっぱり')).toBe('yappari');
    expect(toRomaji('がっこう')).toBe('gakkou');
  });

  test('handles syllabic n with apostrophe before vowels and ya-row', () => {
    expect(toRomaji('かんい')).toBe("kan'i");
    expect(toRomaji('しんよう')).toBe("shin'you");
    expect(toRomaji('かんじ')).toBe('kanji');
  });

  test('applies particle override for wa and e', () => {
    expect(toRomaji('は', true)).toBe('wa');
    expect(toRomaji('へ', true)).toBe('e');
    expect(toRomaji('は', false)).toBe('ha');
    expect(toRomaji('へ', false)).toBe('he');
  });
});

describe('furiganaFor', () => {
  const kanjiSegment: Segment = {
    start: 0,
    len: 2,
    surface: '東京',
    reading: 'とうきょう',
    matched: true,
    entries: [],
  };

  const kanaSegment: Segment = {
    start: 2,
    len: 2,
    surface: 'これ',
    reading: 'これ',
    matched: true,
    entries: [],
  };

  const particleSegment: Segment = {
    start: 4,
    len: 1,
    surface: 'は',
    reading: 'は',
    matched: true,
    entries: [{ headword: 'は', reading: 'は', conjugation: null, pos: ['prt'], senses: [], flags: ['particle'] }],
  };

  test('mode none returns null', () => {
    expect(furiganaFor(kanjiSegment, 'none')).toBeNull();
    expect(furiganaFor(kanaSegment, 'none')).toBeNull();
  });

  test('mode hiragana returns reading only for kanji-bearing words', () => {
    expect(furiganaFor(kanjiSegment, 'hiragana')).toBe('とうきょう');
    expect(furiganaFor(kanaSegment, 'hiragana')).toBeNull();
    expect(furiganaFor(particleSegment, 'hiragana')).toBeNull();
  });

  test('mode katakana returns katakana reading only for kanji-bearing words', () => {
    expect(furiganaFor(kanjiSegment, 'katakana')).toBe('トウキョウ');
    expect(furiganaFor(kanaSegment, 'katakana')).toBeNull();
    expect(furiganaFor(particleSegment, 'katakana')).toBeNull();
  });

  test('mode romaji returns romanization for all segments', () => {
    expect(furiganaFor(kanjiSegment, 'romaji')).toBe('toukyou');
    expect(furiganaFor(kanaSegment, 'romaji')).toBe('kore');
    expect(furiganaFor(particleSegment, 'romaji')).toBe('wa');
  });
});
```

- [ ] **Step 2: Run test to verify failure**

Run: `npx vitest run src/render/furigana.test.ts`
Expected: FAIL (module not found).

- [ ] **Step 3: Implement `src/render/furigana.ts`**

```typescript
import type { FuriganaMode, Segment } from '../types';

const HIRAGANA_START = 0x3041;
const HIRAGANA_END = 0x3096;
const HIRAGANA_TO_KATAKANA = 0x60;

/**
 * Transliterates Hiragana characters to Katakana via +0x60 offset.
 */
export function toKatakana(text: string): string {
  let result = '';
  for (const ch of text) {
    const code = ch.charCodeAt(0);
    if (code >= HIRAGANA_START && code <= HIRAGANA_END) {
      result += String.fromCharCode(code + HIRAGANA_TO_KATAKANA);
    } else {
      result += ch;
    }
  }
  return result;
}

const ROMAJI_TABLE: [string, string][] = [
  ['キャ', 'kya'], ['キュ', 'kyu'], ['キョ', 'kyo'],
  ['シャ', 'sha'], ['シュ', 'shu'], ['ショ', 'sho'],
  ['チャ', 'cha'], ['チュ', 'chu'], ['チョ', 'cho'],
  ['ニャ', 'nya'], ['ニュ', 'nyu'], ['ニョ', 'nyo'],
  ['ヒャ', 'hya'], ['ヒュ', 'hyu'], ['ヒョ', 'hyo'],
  ['ミャ', 'mya'], ['ミュ', 'myu'], ['ミョ', 'myo'],
  ['リャ', 'rya'], ['リュ', 'ryu'], ['リョ', 'ryo'],
  ['ヰャ', 'wya'], ['ヰュ', 'wyu'], ['ヰョ', 'wyo'],
  ['ギャ', 'gya'], ['ギュ', 'gyu'], ['ギョ', 'gyo'],
  ['ヂャ', 'ja'],  ['ヂュ', 'ju'],  ['ヂョ', 'jo'],
  ['ジャ', 'ja'],  ['ジュ', 'ju'],  ['ジョ', 'jo'],
  ['ビャ', 'bya'], ['ビュ', 'byu'], ['ビョ', 'byo'],
  ['ピャ', 'pya'], ['ピュ', 'pyu'], ['ピョ', 'pyo'],
  ['イィ', 'yi'],  ['ユィ', 'yi'],  ['イェ', 'ye'], ['ユェ', 'ye'],
  ['ヷ', 'va'], ['ヴァ', 'va'], ['ヸ', 'vi'], ['ヴィ', 'vi'],
  ['ヴ', 'vu'], ['ヹ', 've'], ['ヴェ', 've'], ['ヺ', 'vo'], ['ヴォ', 'vo'],
  ['ヴャ', 'vya'], ['ヴュ', 'vyu'], ['ヴョ', 'vyo'],
  ['シェ', 'she'], ['ジェ', 'je'], ['チェ', 'che'],
  ['スィ', 'si'], ['スャ', 'sya'], ['スュ', 'syu'], ['スョ', 'syo'],
  ['ズィ', 'zi'], ['ズャ', 'zya'], ['ズュ', 'zyu'], ['ズョ', 'zyo'],
  ['ティ', 'ti'], ['トゥ', 'tu'], ['テャ', 'tya'], ['テュ', 'tyu'], ['テョ', 'tyo'],
  ['ディ', 'di'], ['ドゥ', 'du'], ['デャ', 'dya'], ['デュ', 'dyu'], ['デョ', 'dyo'],
  ['ツァ', 'tsa'], ['ツィ', 'tsi'], ['ツェ', 'tse'], ['ツォ', 'tso'],
  ['ファ', 'fa'], ['フィ', 'fi'], ['フェ', 'fe'], ['フォ', 'fo'],
  ['フャ', 'fya'], ['フュ', 'fyu'], ['フョ', 'fyo'],
  ['クァ', 'kwa'], ['クィ', 'kwi'], ['クェ', 'kwe'], ['クォ', 'kwo'],
  ['グァ', 'gwa'], ['グィ', 'gwi'], ['グェ', 'gwe'], ['グォ', 'gwo'],
  ['ア', 'a'], ['イ', 'i'], ['ウ', 'u'], ['エ', 'e'], ['オ', 'o'],
  ['カ', 'ka'], ['キ', 'ki'], ['ク', 'ku'], ['ケ', 'ke'], ['コ', 'ko'],
  ['サ', 'sa'], ['シ', 'shi'], ['ス', 'su'], ['セ', 'se'], ['ソ', 'so'],
  ['タ', 'ta'], ['チ', 'chi'], ['ツ', 'tsu'], ['テ', 'te'], ['ト', 'to'],
  ['ナ', 'na'], ['ニ', 'ni'], ['ヌ', 'nu'], ['ネ', 'ne'], ['ノ', 'no'],
  ['ハ', 'ha'], ['ヒ', 'hi'], ['フ', 'fu'], ['ヘ', 'he'], ['ホ', 'ho'],
  ['マ', 'ma'], ['ミ', 'mi'], ['ム', 'mu'], ['メ', 'me'], ['モ', 'mo'],
  ['ヤ', 'ya'], ['ユ', 'yu'], ['ヨ', 'yo'],
  ['ラ', 'ra'], ['リ', 'ri'], ['ル', 'ru'], ['レ', 're'], ['ロ', 'ro'],
  ['ワ', 'wa'], ['ヲ', 'wo'], ['ン', 'n'],
  ['ガ', 'ga'], ['ギ', 'gi'], ['グ', 'gu'], ['ゲ', 'ge'], ['ゴ', 'go'],
  ['ザ', 'za'], ['ジ', 'ji'], ['ズ', 'zu'], ['ゼ', 'ze'], ['ゾ', 'zo'],
  ['ダ', 'da'], ['ヂ', 'ji'], ['ヅ', 'zu'], ['デ', 'de'], ['ド', 'do'],
  ['バ', 'ba'], ['ビ', 'bi'], ['ブ', 'bu'], ['ベ', 'be'], ['ボ', 'bo'],
  ['パ', 'pa'], ['ピ', 'pi'], ['プ', 'pu'], ['ペ', 'pe'], ['ポ', 'po'],
  ['ァ', 'a'], ['ィ', 'i'], ['ゥ', 'u'], ['ェ', 'e'], ['ォ', 'o'],
  ['ャ', 'ya'], ['ュ', 'yu'], ['ョ', 'yo'], ['ッ', 'tsu'],
  ['ヮ', 'wa'], ['ヰ', 'wi'], ['ヱ', 'we'],
  ['ー', '-'], ['・', ' '],
];

/**
 * Converts Hiragana or Katakana text to Romaji.
 */
export function toRomaji(text: string, isParticle = false): string {
  if (isParticle) {
    if (text === 'は' || text === 'ハ') return 'wa';
    if (text === 'へ' || text === 'ヘ') return 'e';
  }

  const kata = toKatakana(text);
  let res = '';
  let i = 0;

  while (i < kata.length) {
    const ch = kata[i];

    // Sokuon (っ / ッ)
    if (ch === 'ッ') {
      if (i + 1 < kata.length) {
        const nextSub = kata.slice(i + 1);
        let nextRomaji = '';
        for (const [k, r] of ROMAJI_TABLE) {
          if (nextSub.startsWith(k)) {
            nextRomaji = r;
            break;
          }
        }
        if (nextRomaji.length > 0) {
          res += nextRomaji[0] === 'c' ? 't' : nextRomaji[0];
          i++;
          continue;
        }
      }
      res += 'tsu';
      i++;
      continue;
    }

    // Check digraphs (2 chars) then single kana
    let matched = false;
    for (const [k, r] of ROMAJI_TABLE) {
      if (kata.startsWith(k, i)) {
        if (k === 'ン') {
          res += 'n';
          if (i + 1 < kata.length) {
            const nextCh = kata[i + 1];
            // If next is a vowel or ya/yu/yo, append apostrophe
            const nextCode = nextCh.charCodeAt(0);
            if (
              (nextCode >= 0x30A1 && nextCode <= 0x30AB) || // ァ..オ
              (nextCode >= 0x30E3 && nextCode <= 0x30E9)    // ャ..ョ
            ) {
              res += "'";
            }
          }
        } else {
          res += r;
        }
        i += k.length;
        matched = true;
        break;
      }
    }

    if (!matched) {
      res += ch;
      i++;
    }
  }

  return res;
}

/**
 * Determines the furigana annotation string for a segment given the active mode.
 */
export function furiganaFor(segment: Segment, mode: FuriganaMode): string | null {
  if (mode === 'none') return null;

  const hasKanji = /[一-鿿]/.test(segment.surface);
  const reading = segment.reading ?? segment.surface;
  const isParticle = segment.entries[0]?.flags?.includes('particle') ?? false;

  if (mode === 'hiragana') {
    return hasKanji ? segment.reading : null;
  }
  if (mode === 'katakana') {
    return hasKanji && segment.reading ? toKatakana(segment.reading) : null;
  }
  if (mode === 'romaji') {
    return toRomaji(reading, isParticle);
  }

  return null;
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `npx vitest run src/render/furigana.test.ts`
Expected: PASS (all 13 tests passing).

- [ ] **Step 5: Commit changes**

```bash
git add src/render/furigana.ts src/render/furigana.test.ts
git commit -m "feat(render): add furigana transliteration module and unit tests"
```

---

### Task 3: Sentence Rendering with Ruby / Romaji & CSS

**Files:**
- Modify: `src/render/sentence.ts`
- Modify: `src/render/sentence.test.ts`
- Modify: `src/styles/global.css`

**Interfaces:**
- Modifies:
  - `export function renderSentence(result: ParseResult, mode: FuriganaMode = 'none'): HTMLElement`

- [ ] **Step 1: Write failing tests in `src/render/sentence.test.ts`**

```typescript
import { describe, expect, test } from 'vitest';
import { renderSentence } from './sentence';
import type { ParseResult } from '../types';

describe('renderSentence with furigana modes', () => {
  const result: ParseResult = {
    segments: [
      {
        start: 0,
        len: 2,
        surface: '東京',
        reading: 'とうきょう',
        matched: true,
        entries: [{ headword: '東京', reading: 'とうきょう', conjugation: null, pos: ['n'], senses: [], flags: ['primary'] }],
      },
      {
        start: 2,
        len: 1,
        surface: 'に',
        reading: 'に',
        matched: true,
        entries: [{ headword: 'に', reading: 'に', conjugation: null, pos: ['prt'], senses: [], flags: ['particle'] }],
      },
      {
        start: 3,
        len: 1,
        surface: '。',
        reading: null,
        matched: false,
        entries: [],
      },
    ],
  };

  test('mode none renders clean text without ruby', () => {
    const el = renderSentence(result, 'none');
    const chips = el.querySelectorAll('.chip');
    expect(chips[0].textContent).toBe('東京');
    expect(chips[0].querySelector('ruby')).toBeNull();
    expect(chips[1].textContent).toBe('に');
  });

  test('mode hiragana renders ruby for kanji word only', () => {
    const el = renderSentence(result, 'hiragana');
    const chips = el.querySelectorAll('.chip');
    const ruby0 = chips[0].querySelector('ruby');
    expect(ruby0).not.toBeNull();
    expect(ruby0?.querySelector('rt')?.textContent).toBe('とうきょう');
    expect(chips[1].querySelector('ruby')).toBeNull();
  });

  test('mode katakana renders katakana ruby for kanji word', () => {
    const el = renderSentence(result, 'katakana');
    const chips = el.querySelectorAll('.chip');
    const ruby0 = chips[0].querySelector('ruby');
    expect(ruby0).not.toBeNull();
    expect(ruby0?.querySelector('rt')?.textContent).toBe('トウキョウ');
    expect(chips[1].querySelector('ruby')).toBeNull();
  });

  test('mode romaji renders romaji ruby for all matched segments', () => {
    const el = renderSentence(result, 'romaji');
    const chips = el.querySelectorAll('.chip');
    const rt0 = chips[0].querySelector('rt.romaji');
    expect(rt0).not.toBeNull();
    expect(rt0?.textContent).toBe('toukyou');

    const rt1 = chips[1].querySelector('rt.romaji');
    expect(rt1).not.toBeNull();
    expect(rt1?.textContent).toBe('ni');
  });
});
```

- [ ] **Step 2: Update `src/render/sentence.ts`**

```typescript
import type { FuriganaMode, ParseResult, Segment } from '../types';
import { furiganaFor } from './furigana';

/**
 * Content class for a segment's chip, from the flags the Rust side named.
 */
function contentClass(segment: Segment): string {
  const flags = segment.entries[0]?.flags ?? [];
  if (flags.includes('particle')) return 'particle';
  if (flags.includes('counter')) return 'counter';
  return /[一-鿿]/.test(segment.surface) ? 'kanji' : 'kana';
}

export function renderSentence(result: ParseResult, mode: FuriganaMode = 'none'): HTMLElement {
  const root = document.createElement('div');
  root.className = 'sentence';

  for (const segment of result.segments) {
    const el = document.createElement(segment.matched ? 'button' : 'span');
    if (el instanceof HTMLButtonElement) el.type = 'button';
    el.dataset.start = String(segment.start);
    el.className = segment.matched ? `chip ${contentClass(segment)}` : 'unmatched';

    const annotation = furiganaFor(segment, mode);

    if (annotation !== null) {
      const ruby = document.createElement('ruby');
      ruby.textContent = segment.surface;
      const rt = document.createElement('rt');
      rt.textContent = annotation;
      if (mode === 'romaji') {
        rt.className = 'romaji';
      }
      ruby.append(rt);
      el.append(ruby);
    } else {
      el.textContent = segment.surface;
    }

    root.append(el);
  }

  return root;
}
```

- [ ] **Step 3: Update `src/styles/global.css`**

Add ruby typography and segmented control styles:

```css
/* --- Furigana Ruby Typography --- */
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

/* --- Header Segmented Control --- */
.segmented-control {
  display: inline-flex;
  border-radius: 4px;
  background: var(--surface-control, rgba(255, 255, 255, 0.08));
  padding: 2px;
  gap: 2px;
  align-items: center;
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
  line-height: 1.4;
  transition: background 0.15s ease, color 0.15s ease;
}

.segmented-control button:hover {
  color: var(--color-text, #ffffff);
}

.segmented-control button[aria-checked="true"] {
  background: var(--surface-active, rgba(255, 255, 255, 0.18));
  color: var(--color-text, #ffffff);
}
```

- [ ] **Step 4: Run sentence tests**

Run: `npx vitest run src/render/sentence.test.ts`
Expected: PASS (all tests passing).

- [ ] **Step 5: Commit changes**

```bash
git add src/render/sentence.ts src/render/sentence.test.ts src/styles/global.css
git commit -m "feat(render): add ruby annotations to sentence chips and global styling"
```

---

### Task 4: Header Segmented Control & Main Application Wiring

**Files:**
- Modify: `src/main.ts`
- Modify: `src/main.test.ts`

**Interfaces:**
- Consumes:
  - `renderSentence(result, furiganaMode)`
  - `furigana_mode` from `Settings`

- [ ] **Step 1: Write integration tests in `src/main.test.ts` for Furigana mode switching and persistence**

```typescript
test('furigana segmented control initializes from settings and toggles mode', async () => {
  invoke.mockImplementation((cmd) => {
    if (cmd === 'load_settings') {
      return Promise.resolve({
        always_on_top: false,
        clipboard_monitoring: true,
        decorations: true,
        furigana_mode: 'hiragana',
      });
    }
    return Promise.resolve(null);
  });

  vi.resetModules();
  await import('./main');

  const btnHiragana = document.querySelector<HTMLButtonElement>('.segmented-control button[data-mode="hiragana"]');
  const btnRomaji = document.querySelector<HTMLButtonElement>('.segmented-control button[data-mode="romaji"]');

  expect(btnHiragana?.getAttribute('aria-checked')).toBe('true');
  expect(btnRomaji?.getAttribute('aria-checked')).toBe('false');

  // Click Romaji button
  btnRomaji?.click();

  expect(btnRomaji?.getAttribute('aria-checked')).toBe('true');
  expect(btnHiragana?.getAttribute('aria-checked')).toBe('false');

  // Expect save_settings was called with furigana_mode: 'romaji'
  expect(invoke).toHaveBeenCalledWith('save_settings', expect.objectContaining({
    settings: expect.objectContaining({
      furigana_mode: 'romaji',
    }),
  }));
});
```

- [ ] **Step 2: Update `src/main.ts`**

1. Add the segmented control to `app.innerHTML`:
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
  </header>
```

2. Maintain `currentFuriganaMode: FuriganaMode = 'none'` and wire up mode button clicks:
```typescript
let currentFuriganaMode: FuriganaMode = 'none';
let currentParseResult: ParseResult | null = null;

const modeButtons = app.querySelectorAll<HTMLButtonElement>('.segmented-control button[data-mode]');

function updateFuriganaButtons(mode: FuriganaMode): void {
  currentFuriganaMode = mode;
  modeButtons.forEach((btn) => {
    btn.setAttribute('aria-checked', btn.dataset.mode === mode ? 'true' : 'false');
  });
}

modeButtons.forEach((btn) => {
  btn.addEventListener('click', () => {
    const mode = (btn.dataset.mode as FuriganaMode) ?? 'none';
    if (mode === currentFuriganaMode) return;
    updateFuriganaButtons(mode);
    if (currentParseResult !== null) {
      output.replaceChildren(renderSentence(currentParseResult, currentFuriganaMode));
    }
    void persistSettings();
  });
});
```

3. Initialize mode in `initSettings`:
```typescript
if (s.furigana_mode) {
  updateFuriganaButtons(s.furigana_mode);
}
```

4. Pass `currentFuriganaMode` to `renderSentence` when `parse-result` event fires:
```typescript
void listen<ParseResult>('parse-result', (event) => {
  currentParseResult = event.payload;
  output.replaceChildren(renderSentence(currentParseResult, currentFuriganaMode));
});
```

5. Include `furigana_mode: currentFuriganaMode` in `persistSettings()` payload.

- [ ] **Step 3: Run all frontend tests**

Run: `npm test && npx tsc --noEmit`
Expected: PASS (all frontend test files pass).

- [ ] **Step 4: Commit changes**

```bash
git add src/main.ts src/main.test.ts
git commit -m "feat: add furigana mode segmented control to header and wire settings persistence"
```

---

### Task 5: Full Gate Verification

**Files:**
- Full repository test suite.

- [ ] **Step 1: Run complete verification gate**

Run: `npm test && npx tsc --noEmit && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: All TypeScript and Rust tests pass, zero type errors, zero clippy warnings.

- [ ] **Step 2: Commit any final cleanup or doc updates**
