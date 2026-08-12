# JParser Phase 1A — Foundations & Dictionary Index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure-Rust foundations of the JParser port — character
handling, romaji, the conjugation table, JMdict ingestion, verb stem generation,
and a memory-mapped FST dictionary index — verifiable through a CLI that builds
an index and dumps every dictionary record matching a query.

**Architecture:** A single library crate `jparser` with no Tauri dependency and
no I/O beyond its own asset and index files. Data flows one way:
`Conjugations.json` + `JMdict_e.xml` → `HeadwordRecord` stream (headwords plus
generated verb stems) → sorted FST keyed on normalized surfaces, with a
memory-mapped payload holding the records and entry data. Phase 1B consumes this
index; nothing here knows about matching or segmentation.

**Tech Stack:** Rust 2021, `fst` (memory-mapped finite-state transducer),
`quick-xml` (streaming XML), `serde` + `serde_json` (conjugation asset),
`bincode` (payload encoding), `memmap2`, `thiserror`, `clap` (CLI),
`insta` (snapshots), `cargo-llvm-cov` (coverage).

**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md`.
The C++ original is in `ta-old/` and is read-only — never modify it.

## Global Constraints

- **Crate purity:** `crates/jparser` must not depend on Tauri, any UI crate, or
  any HTTP client. It must build and test with no window and no network.
- **License:** GPL v2. Every new source file gets the standard GPL v2 header
  comment. This port is a derivative of ta-old (GPL v2) and reuses its
  `Conjugations.txt` data.
- **Fixed tense discriminants:** `Remove = 0`, `NonPast = 1`, `Stem = 2`,
  `Potential = 3`. These four are special-cased by the algorithms and must never
  be reordered. All other tenses get dynamic IDs assigned at load time.
- **Immutability:** All public types are owned and immutable. Mutation is allowed
  only inside a function, never observable through a public API.
- **No magic numbers:** Every threshold, offset, and Unicode boundary is a named
  `const`.
- **Errors are explicit:** No `unwrap()` or `expect()` in library code outside
  tests. Every fallible path returns `Result` with a `thiserror` variant. Never
  silently skip data without counting it.
- **File size:** 200–400 lines typical, 800 hard maximum. Split when a file
  exceeds this.
- **Coverage target:** 80% line coverage on `crates/jparser`, measured by
  `cargo llvm-cov`.
- **Naming:** types `PascalCase`, functions and variables `snake_case`,
  constants `UPPER_SNAKE_CASE`.

---

## File Structure

| File | Responsibility |
|---|---|
| `ta/Cargo.toml` | Workspace root |
| `ta/crates/jparser/Cargo.toml` | Library manifest |
| `ta/crates/jparser/src/lib.rs` | Module declarations |
| `ta/crates/jparser/src/kana.rs` | `unify()` normalization + character classifiers |
| `ta/crates/jparser/src/romaji.rs` | Romaji table, chunk conversion, particle fixup |
| `ta/crates/jparser/src/conjugation.rs` | Conjugation table types, loader, `Next Type` resolution |
| `ta/crates/jparser/src/jmdict.rs` | Streaming JMdict XML → `RawEntry` |
| `ta/crates/jparser/src/record.rs` | `RawEntry` → `HeadwordRecord`, flag derivation |
| `ta/crates/jparser/src/stem.rs` | Verb stem generation, v5 fallback, instrumentation |
| `ta/crates/jparser/src/index/mod.rs` | `IndexHeader`, format version, shared types |
| `ta/crates/jparser/src/index/build.rs` | Records → FST + payload files |
| `ta/crates/jparser/src/index/load.rs` | Memory-mapped load + prefix walk |
| `ta/crates/jparser/assets/conjugations.json` | UTF-8 conversion of ta-old's asset |
| `ta/crates/jparser/src/bin/jparser-cli.rs` | `build-index`, `lookup`, `romaji` commands |
| `ta/crates/jparser/tests/fixtures/jmdict_mini.xml` | Hand-written JMdict fixture |
| `ta/crates/jparser/tests/index_roundtrip.rs` | Integration: build → load → query |
| `ta/xtask/src/main.rs` | One-off asset conversion (UTF-16LE → UTF-8) |

Rationale for the splits: `kana` and `romaji` are pure functions with no
dependencies and are used by everything, so they come first and stay separate.
`record` and `stem` are split because stem generation is the highest-risk logic
in this phase and deserves its own file and test module. `index/build` and
`index/load` are split because they run at different times for different callers
and share only the header type.

---

## Task 1: Workspace scaffold and kana normalization

**Files:**
- Create: `ta/Cargo.toml`
- Create: `ta/crates/jparser/Cargo.toml`
- Create: `ta/crates/jparser/src/lib.rs`
- Create: `ta/crates/jparser/src/kana.rs`

**Interfaces:**
- Consumes: nothing (first task)
- Produces:
  - `pub fn kana::unify(c: char) -> char`
  - `pub fn kana::unify_str(s: &str) -> String`
  - `pub fn kana::is_hiragana(c: char) -> bool`
  - `pub fn kana::is_katakana(c: char) -> bool`
  - `pub fn kana::is_half_width_katakana(c: char) -> bool`
  - `pub fn kana::is_kanji(c: char) -> bool`
  - `pub fn kana::is_cjk_ideograph(c: char) -> bool`
  - `pub fn kana::is_japanese(c: char) -> bool`
  - `pub fn kana::has_japanese(s: &str) -> bool`
  - `pub fn kana::is_digit(c: char) -> bool`
  - `pub fn kana::to_katakana(s: &str) -> Option<String>`

- [ ] **Step 1: Create the workspace manifest**

`ta/Cargo.toml`:

```toml
[workspace]
members = ["crates/jparser", "xtask"]
resolver = "2"

[workspace.package]
edition = "2021"
license = "GPL-2.0-only"
rust-version = "1.75"
```

- [ ] **Step 2: Create the library manifest**

`ta/crates/jparser/Cargo.toml`:

```toml
[package]
name = "jparser"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
bincode = "1.3"
clap = { version = "4", features = ["derive"] }
fst = "0.4"
memmap2 = "0.9"
quick-xml = "0.36"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"

[dev-dependencies]
insta = "1"

[[bin]]
name = "jparser-cli"
path = "src/bin/jparser-cli.rs"
```

- [ ] **Step 3: Create the crate root**

`ta/crates/jparser/src/lib.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

pub mod kana;
```

- [ ] **Step 4: Write the failing tests for `kana`**

Create `ta/crates/jparser/src/kana.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_folds_hiragana_to_katakana() {
        assert_eq!(unify('あ'), 'ア');
        assert_eq!(unify('ん'), 'ン');
        assert_eq!(unify('っ'), 'ッ');
    }

    #[test]
    fn unify_leaves_katakana_unchanged() {
        assert_eq!(unify('ア'), 'ア');
        assert_eq!(unify('ー'), 'ー');
    }

    #[test]
    fn unify_folds_fullwidth_punctuation_and_digits_to_ascii() {
        assert_eq!(unify('！'), '!');
        assert_eq!(unify('０'), '0');
        assert_eq!(unify('９'), '9');
    }

    #[test]
    fn unify_leaves_fullwidth_letters_unchanged() {
        // ta-old deliberately stops the fullwidth fold at U+FF20 so fullwidth
        // Latin letters are not folded. Ａ is U+FF21.
        assert_eq!(unify('Ａ'), 'Ａ');
    }

    #[test]
    fn unify_uppercases_ascii_lowercase() {
        assert_eq!(unify('a'), 'A');
        assert_eq!(unify('z'), 'Z');
    }

    #[test]
    fn unify_leaves_kanji_unchanged() {
        assert_eq!(unify('言'), '言');
    }

    #[test]
    fn unify_str_folds_every_character() {
        assert_eq!(unify_str("いわれた"), "イワレタ");
        assert_eq!(unify_str("言われた"), "言ワレタ");
    }

    #[test]
    fn unify_str_is_prefix_stable() {
        // The FST relies on this: folding a prefix must equal the prefix of the
        // folded string, or key lookup desynchronizes from the source text.
        let s = "言われなかった";
        for (i, _) in s.char_indices() {
            assert!(unify_str(s).starts_with(&unify_str(&s[..i])));
        }
    }

    #[test]
    fn classifies_hiragana() {
        assert!(is_hiragana('あ'));
        assert!(!is_hiragana('ア'));
        assert!(!is_hiragana('言'));
    }

    #[test]
    fn classifies_katakana_excluding_middle_dot() {
        assert!(is_katakana('ア'));
        assert!(is_katakana('ー'));
        assert!(!is_katakana('・'));
        assert!(!is_katakana('あ'));
    }

    #[test]
    fn classifies_kanji_including_repeat_mark() {
        assert!(is_kanji('言'));
        assert!(is_kanji('々'));
        assert!(!is_kanji('あ'));
    }

    #[test]
    fn cjk_ideograph_excludes_the_repeat_mark() {
        // The segmenter's kanji penalty uses the ideograph range only.
        assert!(is_cjk_ideograph('言'));
        assert!(!is_cjk_ideograph('々'));
    }

    #[test]
    fn is_japanese_covers_all_four_classes() {
        assert!(is_japanese('あ'));
        assert!(is_japanese('ア'));
        assert!(is_japanese('言'));
        assert!(is_japanese('ｱ'));
        assert!(!is_japanese('a'));
        assert!(!is_japanese('!'));
    }

    #[test]
    fn has_japanese_detects_any_japanese_character() {
        assert!(has_japanese("hello 言"));
        assert!(!has_japanese("hello world"));
        assert!(!has_japanese(""));
    }

    #[test]
    fn is_digit_covers_ascii_fullwidth_and_kanji_numerals() {
        assert!(is_digit('7'));
        assert!(is_digit('７'));
        assert!(is_digit('三'));
        assert!(is_digit('万'));
        assert!(!is_digit('あ'));
    }

    #[test]
    fn to_katakana_converts_hiragana() {
        assert_eq!(to_katakana("いわれた").as_deref(), Some("イワレタ"));
    }

    #[test]
    fn to_katakana_passes_non_hiragana_through() {
        assert_eq!(to_katakana("言う").as_deref(), Some("言ウ"));
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser kana`
Expected: FAIL — `cannot find function 'unify' in this scope`, and similar for
every other function.

- [ ] **Step 6: Implement `kana.rs`**

Insert above the test module, after the GPL v2 header comment from Step 3:

```rust
//! Character classification and normalization.
//!
//! `unify` is a direct port of `unify()` in `ta-old/Shared/Shrink.h:180`, which
//! backs ta-old's `wcsijcmp`/`wcsnijcmp` kana-insensitive comparators. It folds
//! three things: hiragana to katakana, fullwidth punctuation and digits to
//! ASCII, and ASCII lowercase to uppercase.
//!
//! The whole dictionary index depends on `unify` being applied identically at
//! build time and at query time. It is deliberately character-wise so that
//! folding a prefix equals the prefix of a folded string.

// Hiragana block, per ta-old's IsHiragana (Shared/StringUtil.cpp:521).
const HIRAGANA_START: u32 = 0x3040;
const HIRAGANA_END: u32 = 0x30A0; // exclusive

// Katakana block, per ta-old's IsKatakana (Shared/StringUtil.cpp:516).
const KATAKANA_START: u32 = 0x30A0;
const KATAKANA_END: u32 = 0x3100; // exclusive
const KATAKANA_MIDDLE_DOT: u32 = 0x30FB; // '・', excluded from katakana

// Halfwidth katakana, per ta-old's IsHalfWidthKatakana.
const HALFWIDTH_KATAKANA_START: u32 = 0xFF65;
const HALFWIDTH_KATAKANA_END: u32 = 0xFF9C; // exclusive

// CJK ideographs, per ta-old's IsKanji (Shared/StringUtil.cpp:531).
const CJK_START: u32 = 0x4E00;
const CJK_END: u32 = 0x9FC0; // exclusive
const KANJI_REPEAT_MARK: u32 = 0x3005; // '々'

// Ranges used by unify(). Bounds are ta-old's exactly; the fullwidth fold
// deliberately stops before U+FF20 so fullwidth Latin letters do not fold.
const UNIFY_HIRAGANA_START: u32 = 0x3041;
const UNIFY_HIRAGANA_END: u32 = 0x3096; // exclusive
const HIRAGANA_TO_KATAKANA: u32 = 0x30A1 - 0x3041; // 0x60
const UNIFY_FULLWIDTH_START: u32 = 0xFF01;
const UNIFY_FULLWIDTH_END: u32 = 0xFF20; // exclusive
const FULLWIDTH_TO_ASCII: u32 = 0xFF01 - 0x0021;

/// Kanji numerals counted as digits by ta-old's `IsDigit`
/// (`exe/util/Dictionary.cpp:1069`).
const KANJI_DIGITS: &[char] = &[
    '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万',
];

/// Fold a character into the comparison space used by the dictionary index.
pub fn unify(c: char) -> char {
    let mut x = c as u32;
    if (UNIFY_HIRAGANA_START..UNIFY_HIRAGANA_END).contains(&x) {
        x += HIRAGANA_TO_KATAKANA;
    } else if (UNIFY_FULLWIDTH_START..UNIFY_FULLWIDTH_END).contains(&x) {
        x -= FULLWIDTH_TO_ASCII;
    }
    let Some(ch) = char::from_u32(x) else { return c };
    // ta-old's lowercase check excludes 'z' (a `< 0x7A` off-by-one). We fold
    // the full range: correctness here only requires that build-time and
    // query-time normalization agree, and an inconsistent fold would be a
    // worse bug than a faithful one.
    if ch.is_ascii() {
        return ch.to_ascii_uppercase();
    }
    ch
}

/// Fold every character of a string. Character-wise, so prefix-stable.
pub fn unify_str(s: &str) -> String {
    s.chars().map(unify).collect()
}

pub fn is_hiragana(c: char) -> bool {
    (HIRAGANA_START..HIRAGANA_END).contains(&(c as u32))
}

pub fn is_katakana(c: char) -> bool {
    let x = c as u32;
    (KATAKANA_START..KATAKANA_END).contains(&x) && x != KATAKANA_MIDDLE_DOT
}

pub fn is_half_width_katakana(c: char) -> bool {
    (HALFWIDTH_KATAKANA_START..HALFWIDTH_KATAKANA_END).contains(&(c as u32))
}

pub fn is_kanji(c: char) -> bool {
    let x = c as u32;
    (CJK_START..CJK_END).contains(&x) || x == KANJI_REPEAT_MARK
}

/// True for the CJK ideograph range the segmenter penalizes skipping. Excludes
/// the repeat mark, matching `FindBestMatches`'s inline range test.
pub fn is_cjk_ideograph(c: char) -> bool {
    (CJK_START..CJK_END).contains(&(c as u32))
}

pub fn is_japanese(c: char) -> bool {
    is_hiragana(c) || is_katakana(c) || is_kanji(c) || is_half_width_katakana(c)
}

pub fn has_japanese(s: &str) -> bool {
    s.chars().any(is_japanese)
}

pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || ('０'..='９').contains(&c) || KANJI_DIGITS.contains(&c)
}

/// Convert hiragana in a reading to katakana for display, per ta-old's
/// `FuriganaWindow::GetFurigana` katakana branch. Non-hiragana passes through.
/// Returns `None` only if a converted code point is not a valid `char`.
pub fn to_katakana(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_hiragana(c) {
            out.push(char::from_u32(c as u32 + HIRAGANA_TO_KATAKANA)?);
        } else {
            out.push(c);
        }
    }
    Some(out)
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser kana`
Expected: PASS, 17 tests.

- [ ] **Step 8: Verify the crate has no forbidden dependencies**

Run: `cd ta && cargo tree -p jparser | grep -Ei 'tauri|reqwest|hyper|winit' || echo clean`
Expected: prints `clean`.

- [ ] **Step 9: Commit**

```bash
cd ta && git add Cargo.toml crates/jparser
git commit -m "feat: add jparser crate scaffold and kana normalization

Ports unify() from ta-old/Shared/Shrink.h:180, which backs the
wcsijcmp/wcsnijcmp kana-insensitive comparators, plus the character
classifiers from Shared/StringUtil.cpp.

unify is character-wise and therefore prefix-stable, which the FST
index depends on."
```

---

## Task 2: Romaji conversion

**Files:**
- Create: `ta/crates/jparser/src/romaji.rs`
- Modify: `ta/crates/jparser/src/lib.rs` (add `pub mod romaji;`)

**Interfaces:**
- Consumes: nothing from Task 1 at runtime (it folds kana itself, since the
  romaji table is katakana-keyed and `unify` also folds ASCII, which would
  corrupt lookups)
- Produces:
  - `pub fn romaji::to_romaji(s: &str) -> String`
  - `pub fn romaji::apply_particle_fixup(s: &str) -> String`

- [ ] **Step 1: Write the failing tests**

Create `ta/crates/jparser/src/romaji.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_katakana() {
        assert_eq!(to_romaji("カタカナ"), "katakana");
    }

    #[test]
    fn converts_hiragana_by_folding_first() {
        assert_eq!(to_romaji("いわれた"), "iwareta");
    }

    #[test]
    fn prefers_digraphs_over_singles() {
        // The table lists digraphs first; キャ must not become "kiya".
        assert_eq!(to_romaji("キャ"), "kya");
        assert_eq!(to_romaji("しゃ"), "sha");
        assert_eq!(to_romaji("ちょ"), "cho");
    }

    #[test]
    fn sokuon_doubles_the_following_consonant() {
        assert_eq!(to_romaji("かった"), "katta");
        assert_eq!(to_romaji("いっぱい"), "ippai");
    }

    #[test]
    fn sokuon_before_chi_becomes_t_not_c() {
        // ta-old maps 'c' to 't', so っち is "tchi" rather than "cchi".
        assert_eq!(to_romaji("まっちゃ"), "matcha");
    }

    #[test]
    fn sokuon_before_a_vowel_is_passed_through() {
        // No consonant to double, so the chunk reports zero consumed and the
        // character is copied verbatim.
        assert_eq!(to_romaji("っあ"), "っa");
    }

    #[test]
    fn long_vowel_mark_becomes_combining_macron() {
        assert_eq!(to_romaji("ラーメン"), "ra\u{0304}men");
    }

    #[test]
    fn inserts_apostrophe_after_n_before_vowel_or_ya_row() {
        assert_eq!(to_romaji("しんあい"), "shin'ai");
        assert_eq!(to_romaji("かんゆ"), "kan'yu");
    }

    #[test]
    fn does_not_insert_apostrophe_after_n_before_a_consonant() {
        assert_eq!(to_romaji("かんじ"), "kanji");
    }

    #[test]
    fn passes_non_kana_through_unchanged() {
        assert_eq!(to_romaji("言う"), "言u");
        assert_eq!(to_romaji("ABC"), "ABC");
        assert_eq!(to_romaji("・"), "・");
    }

    #[test]
    fn particle_fixup_turns_trailing_ha_into_wa() {
        assert_eq!(apply_particle_fixup("ha"), "wa");
    }

    #[test]
    fn particle_fixup_spares_cha() {
        // The 'c' guard exists so "cha" is not corrupted into "cwa".
        assert_eq!(apply_particle_fixup("cha"), "cha");
    }

    #[test]
    fn particle_fixup_turns_bare_he_into_e() {
        assert_eq!(apply_particle_fixup("he"), "e");
    }

    #[test]
    fn particle_fixup_leaves_longer_he_words_alone() {
        // Only an exactly-two-character "he" collapses to "e".
        assert_eq!(apply_particle_fixup("heya"), "heya");
    }

    #[test]
    fn particle_fixup_leaves_unrelated_strings_alone() {
        assert_eq!(apply_particle_fixup("no"), "no");
        assert_eq!(apply_particle_fixup(""), "");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser romaji`
Expected: FAIL — `cannot find function 'to_romaji' in this scope`.

- [ ] **Step 3: Implement the table**

Insert above the test module, after a GPL v2 header comment:

```rust
//! Romaji conversion, ported from `ChunkToRomaji`/`ToRomaji` and `romajiTable`
//! in `ta-old/Shared/StringUtil.cpp:23-450`.
//!
//! The table is keyed on **katakana**, so input is folded to katakana per
//! character before lookup. Digraphs are listed before single characters and
//! the first match wins, which is what makes キャ produce "kya" rather than
//! "kiya" — order is load-bearing, do not sort this table.

const HIRAGANA_FOLD_START: u32 = 0x3041;
const HIRAGANA_FOLD_END: u32 = 0x3096; // exclusive
const HIRAGANA_TO_KATAKANA: u32 = 0x60;

const KATAKANA_LOW: u32 = 0x30A1;
const KATAKANA_HIGH: u32 = 0x30FC;
const MIDDLE_DOT: u32 = 0x30FB;
const LONG_VOWEL_MARK: u32 = 0x30FC;
const COMBINING_MACRON: char = '\u{0304}';
const SOKUON: char = 'ッ';

/// Vowel-row katakana ァ..オ — an 'n' before these needs an apostrophe.
const VOWEL_ROW_START: u32 = 0x30A1;
const VOWEL_ROW_END: u32 = 0x30AB; // exclusive
/// Ya-row katakana ャ..ョ — same apostrophe rule.
const YA_ROW_START: u32 = 0x30E3;
const YA_ROW_END: u32 = 0x30E9; // exclusive

/// `(katakana, romaji)`. Digraphs first; order is significant.
const ROMAJI_TABLE: &[(&str, &str)] = &[
    ("キャ", "kya"), ("キュ", "kyu"), ("キョ", "kyo"),
    ("シャ", "sha"), ("シュ", "shu"), ("ショ", "sho"),
    ("チャ", "cha"), ("チュ", "chu"), ("チョ", "cho"),
    ("ニャ", "nya"), ("ニュ", "nyu"), ("ニョ", "nyo"),
    ("ヒャ", "hya"), ("ヒュ", "hyu"), ("ヒョ", "hyo"),
    ("ミャ", "mya"), ("ミュ", "myu"), ("ミョ", "myo"),
    ("リャ", "rya"), ("リュ", "ryu"), ("リョ", "ryo"),
    ("ヰャ", "wya"), ("ヰュ", "wyu"), ("ヰョ", "wyo"),
    ("ギャ", "gya"), ("ギュ", "gyu"), ("ギョ", "gyo"),
    ("ヂャ", "ja"),  ("ヂュ", "ju"),  ("ヂョ", "jo"),
    ("ジャ", "ja"),  ("ジュ", "ju"),  ("ジョ", "jo"),
    ("ビャ", "bya"), ("ビュ", "byu"), ("ビョ", "byo"),
    ("ピャ", "pya"), ("ピュ", "pyu"), ("ピョ", "pyo"),
    ("イィ", "yi"),  ("ユィ", "yi"),  ("イェ", "ye"), ("ユェ", "ye"),
    ("ヷ", "va"), ("ヴァ", "va"), ("ヸ", "vi"), ("ヴィ", "vi"),
    ("ヴ", "vu"), ("ヹ", "ve"), ("ヴェ", "ve"), ("ヺ", "vo"), ("ヴォ", "vo"),
    ("ヴャ", "vya"), ("ヴュ", "vyu"), ("ヴョ", "vyo"),
    ("シェ", "she"), ("ジェ", "je"), ("チェ", "che"),
    ("スィ", "si"), ("スャ", "sya"), ("スュ", "syu"), ("スョ", "syo"),
    ("ズィ", "zi"), ("ズャ", "zya"), ("ズュ", "zyu"), ("ズョ", "zyo"),
    ("ティ", "ti"), ("トゥ", "tu"),
    ("テャ", "tya"), ("テュ", "tyu"), ("テョ", "tyo"),
    ("ディ", "di"), ("ドゥ", "du"),
    ("デャ", "dya"), ("デュ", "dyu"), ("デョ", "dyo"),
    ("ツァ", "tsa"), ("ツィ", "tsi"), ("ツェ", "tse"), ("ツォ", "tso"),
    ("ファ", "fa"), ("フィ", "fi"), ("ホゥ", "hu"),
    ("フェ", "fe"), ("フォ", "fo"),
    ("フャ", "fya"), ("フュ", "fyu"), ("フョ", "fyo"),
    ("リェ", "rye"),
    ("ウァ", "wa"), ("ウィ", "wi"), ("ウェ", "we"), ("ウォ", "wo"),
    ("ウャ", "wya"), ("ウュ", "wyu"), ("ウョ", "wyo"),
    ("クァ", "kwa"), ("クヮ", "kwa"), ("クィ", "kwi"),
    ("クゥ", "kwu"), ("クェ", "kwe"), ("クォ", "kwo"),
    ("グァ", "gwa"), ("グヮ", "gwa"), ("グィ", "gwi"),
    ("グゥ", "gwu"), ("グェ", "gwe"), ("グォ", "gwo"),
    ("ァ", "a"), ("ィ", "i"), ("ゥ", "u"), ("ェ", "e"), ("ォ", "o"),
    ("ャ", "ya"), ("ュ", "yu"), ("ョ", "yo"), ("ヮ", "wa"),
    ("ア", "a"), ("イ", "i"), ("ウ", "u"), ("エ", "e"), ("オ", "o"),
    ("カ", "ka"), ("キ", "ki"), ("ク", "ku"), ("ケ", "ke"), ("コ", "ko"),
    ("サ", "sa"), ("シ", "shi"), ("ス", "su"), ("セ", "se"), ("ソ", "so"),
    ("タ", "ta"), ("チ", "chi"), ("ツ", "tsu"), ("テ", "te"), ("ト", "to"),
    ("ナ", "na"), ("ニ", "ni"), ("ヌ", "nu"), ("ネ", "ne"), ("ノ", "no"),
    ("マ", "ma"), ("ミ", "mi"), ("ム", "mu"), ("メ", "me"), ("モ", "mo"),
    ("ヤ", "ya"), ("ユ", "yu"), ("ヨ", "yo"),
    ("ラ", "ra"), ("リ", "ri"), ("ル", "ru"), ("レ", "re"), ("ロ", "ro"),
    ("ワ", "wa"), ("ヰ", "wi"), ("ヱ", "we"), ("ヲ", "wo"), ("ン", "n"),
    ("ガ", "ga"), ("ギ", "gi"), ("グ", "gu"), ("ゲ", "ge"), ("ゴ", "go"),
    ("ダ", "da"), ("ヂ", "ji"), ("ヅ", "dzu"), ("デ", "de"), ("ド", "do"),
    ("ザ", "za"), ("ジ", "ji"), ("ズ", "zu"), ("ゼ", "ze"), ("ゾ", "zo"),
    ("ハ", "ha"), ("ヒ", "hi"), ("フ", "fu"), ("ヘ", "he"), ("ホ", "ho"),
    ("バ", "ba"), ("ビ", "bi"), ("ブ", "bu"), ("ベ", "be"), ("ボ", "bo"),
    ("パ", "pa"), ("ピ", "pi"), ("プ", "pu"), ("ペ", "pe"), ("ポ", "po"),
];
```

- [ ] **Step 4: Implement the conversion functions**

Append below the table, still above the test module:

```rust
/// Fold one character to katakana for table lookup.
fn fold(c: char) -> char {
    let x = c as u32;
    if (HIRAGANA_FOLD_START..HIRAGANA_FOLD_END).contains(&x) {
        char::from_u32(x + HIRAGANA_TO_KATAKANA).unwrap_or(c)
    } else {
        c
    }
}

/// True when an 'n' followed by `next` would be ambiguous, requiring `'`.
fn needs_apostrophe(next: Option<char>) -> bool {
    let Some(n) = next.map(fold) else { return false };
    let x = n as u32;
    (VOWEL_ROW_START..VOWEL_ROW_END).contains(&x)
        || (YA_ROW_START..YA_ROW_END).contains(&x)
}

/// Convert one chunk. Returns `(romaji, chars_consumed)`; a count of zero means
/// "not kana", and the caller copies the character through unchanged.
fn chunk_to_romaji(chars: &[char]) -> (String, usize) {
    let Some(&first) = chars.first() else {
        return (String::new(), 0);
    };
    let c1 = fold(first);
    let x1 = c1 as u32;

    if x1 == LONG_VOWEL_MARK {
        return (COMBINING_MACRON.to_string(), 1);
    }
    if x1 < KATAKANA_LOW || x1 > KATAKANA_HIGH || x1 == MIDDLE_DOT {
        return (String::new(), 0);
    }

    let c2 = chars.get(1).copied().map(fold);

    if c1 == SOKUON {
        // Double the next syllable's initial consonant. 'c' becomes 't' so っち
        // yields "tchi". Vowels and 'y' cannot be doubled.
        let Some(next) = c2 else { return (String::new(), 0) };
        for (jap, ascii) in ROMAJI_TABLE {
            let mut it = jap.chars();
            let (Some(j0), None) = (it.next(), it.next()) else { continue };
            if j0 != next {
                continue;
            }
            let letter = ascii.as_bytes()[0] as char;
            if matches!(letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
                continue;
            }
            let doubled = if letter == 'c' { 't' } else { letter };
            return (doubled.to_string(), 1);
        }
        return (String::new(), 0);
    }

    for (jap, ascii) in ROMAJI_TABLE {
        let mut it = jap.chars();
        let Some(j0) = it.next() else { continue };
        let j1 = it.next();
        if j0 != c1 {
            continue;
        }
        let consumed = match j1 {
            Some(second) if Some(second) == c2 => 2,
            Some(_) => continue,
            None => 1,
        };
        let mut out = (*ascii).to_string();
        if out.ends_with('n') && needs_apostrophe(chars.get(consumed).copied()) {
            out.push('\'');
        }
        return (out, consumed);
    }

    (String::new(), 0)
}

/// Convert a kana string to romaji. Non-kana characters pass through unchanged.
pub fn to_romaji(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let (written, consumed) = chunk_to_romaji(&chars[i..]);
        if consumed == 0 {
            out.push(chars[i]);
            i += 1;
        } else {
            out.push_str(&written);
            i += consumed;
        }
    }
    out
}

/// Apply ta-old's particle-only romaji corrections
/// (`FuriganaWindow::GetFurigana`, romaji branch): は reads "wa" not "ha", and
/// へ reads "e" not "he". Call this **only** for particle words.
pub fn apply_particle_fixup(s: &str) -> String {
    if s == "he" {
        return "e".to_string();
    }
    if let Some(stem) = s.strip_suffix("ha") {
        // The 'c' guard keeps "cha" intact.
        if !stem.ends_with('c') {
            return format!("{stem}wa");
        }
    }
    s.to_string()
}
```

- [ ] **Step 5: Register the module**

In `ta/crates/jparser/src/lib.rs`, after `pub mod kana;`:

```rust
pub mod romaji;
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser romaji`
Expected: PASS, 15 tests.

- [ ] **Step 7: Commit**

```bash
cd ta && git add crates/jparser/src/romaji.rs crates/jparser/src/lib.rs
git commit -m "feat: add romaji conversion

Ports romajiTable, ChunkToRomaji, and ToRomaji from
ta-old/Shared/StringUtil.cpp, including sokuon consonant doubling with
its c-to-t special case, the long-vowel-mark to combining-macron
mapping, and the n-apostrophe disambiguation rule.

Table order is significant: digraphs precede singles so キャ yields
kya rather than kiya."
```

---

## Task 3: Convert the conjugation asset to UTF-8

**Files:**
- Create: `ta/xtask/Cargo.toml`
- Create: `ta/xtask/src/main.rs`
- Create: `ta/crates/jparser/assets/conjugations.json` (generated, committed)

**Interfaces:**
- Consumes: `ta-old/dictionaries/Conjugations.txt` (read-only)
- Produces: `assets/conjugations.json`, a UTF-8 JSON array consumed by Task 4.
  Element shape: `{"Name": String, "Part of Speech": "Verb"|"Adj", "Tenses":
  [{"Formal": bool, "Negative": bool, "Suffix": String, "Tense": String,
  "Next Type": String (optional)}]}`

- [ ] **Step 1: Create the xtask manifest**

`ta/xtask/Cargo.toml`:

```toml
[package]
name = "xtask"
version = "0.1.0"
edition.workspace = true
license.workspace = true
publish = false

[dependencies]
serde_json = "1"
```

- [ ] **Step 2: Write the converter**

`ta/xtask/src/main.rs`:

```rust
//! One-off asset conversion: ta-old's UTF-16LE Conjugations.txt to UTF-8 JSON.
//!
//! Run with: cargo run -p xtask -- convert-conjugations

use std::fs;
use std::path::Path;

const SOURCE: &str = "../ta-old/dictionaries/Conjugations.txt";
const DEST: &str = "crates/jparser/assets/conjugations.json";
const UTF16LE_BOM: [u8; 2] = [0xFF, 0xFE];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("convert-conjugations") {
        eprintln!("usage: cargo run -p xtask -- convert-conjugations");
        std::process::exit(2);
    }

    let bytes = fs::read(SOURCE)?;
    let body = bytes.strip_prefix(&UTF16LE_BOM[..]).unwrap_or(&bytes);
    if body.len() % 2 != 0 {
        return Err("source is not valid UTF-16LE: odd byte count".into());
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    let text = String::from_utf16(&units)?;

    // Round-trip through serde_json so the committed asset is validated and
    // normalized rather than trusted verbatim.
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let array = value.as_array().ok_or("expected a top-level JSON array")?;

    let path = Path::new(DEST);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&value)?)?;

    let conjugations: usize = array
        .iter()
        .filter_map(|t| t.get("Tenses")?.as_array().map(Vec::len))
        .sum();
    println!("wrote {DEST}: {} types, {conjugations} conjugations", array.len());
    Ok(())
}
```

- [ ] **Step 3: Run the converter**

Run: `cd ta && cargo run -p xtask -- convert-conjugations`
Expected: `wrote crates/jparser/assets/conjugations.json: 32 types, N conjugations`

- [ ] **Step 4: Verify the converted asset matches the source**

Run:

```bash
cd ta && python3 -c "
import json
src = json.load(open('../ta-old/dictionaries/Conjugations.txt', encoding='utf-16'))
dst = json.load(open('crates/jparser/assets/conjugations.json', encoding='utf-8'))
assert src == dst, 'converted asset differs from source'
names = [t['Name'] for t in dst]
nxt = sum(1 for t in dst for c in t['Tenses'] if 'Next Type' in c)
print('types', len(dst), 'unique names', len(set(names)), 'next-type', nxt)
"
```

Expected: `types 32 unique names 28 next-type 223`

The 32-vs-28 gap is the four duplicated names (`v5r-i`, `v5uru`, `vk`, `vs`) and
is correct. Task 4 asserts these exact numbers.

- [ ] **Step 5: Commit**

```bash
cd ta && git add xtask crates/jparser/assets/conjugations.json Cargo.toml
git commit -m "feat: convert Conjugations.txt from UTF-16LE to UTF-8 JSON

ta-old's conjugation table is the most valuable artifact in the
original: hand-tuned, 32 verb/adjective types, 223 chained
conjugations, not reconstructible. Committed as UTF-8 so it loads with
serde and diffs in review.

Verified equal to the source after a JSON round-trip."
```

---

## Task 4: Conjugation table loader and `Next Type` resolution

**Files:**
- Create: `ta/crates/jparser/src/conjugation.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes: `kana::unify` (Task 1), `assets/conjugations.json` (Task 3)
- Produces:
  - `pub type conjugation::TenseId = usize`
  - `pub type conjugation::VerbTypeId = usize` (0-based; ta-old stored `vt + 1`)
  - `pub const conjugation::MAX_CONJ_DEPTH: usize = 5`
  - `pub const conjugation::TENSE_REMOVE: TenseId = 0`
  - `pub const conjugation::TENSE_NON_PAST: TenseId = 1`
  - `pub const conjugation::TENSE_STEM: TenseId = 2`
  - `pub const conjugation::TENSE_POTENTIAL: TenseId = 3`
  - `pub struct conjugation::Form(pub u8)` with `Form::FORMAL: u8 = 1`,
    `Form::NEGATIVE: u8 = 2`, `Form::from_flags(bool, bool) -> Form`,
    `Form::is_formal(self) -> bool`, `Form::is_negative(self) -> bool`
  - `pub struct conjugation::Conjugation { pub tense: TenseId, pub form: Form, pub suffix: String, pub next_verb_type: Option<VerbTypeId> }`
  - `pub struct conjugation::VerbType { pub name: String, pub is_adjective: bool, pub remove_tense: TenseId, pub conjugations: Vec<Conjugation> }`
  - `pub struct conjugation::ConjugationTable` with
    `load_embedded() -> Result<Self, ConjugationError>`,
    `from_json(&str) -> Result<Self, ConjugationError>`,
    `types(&self) -> &[VerbType]`,
    `tense_name(&self, TenseId) -> Option<&str>`,
    `types_named(&self, &str) -> Vec<VerbTypeId>`
  - `pub enum conjugation::ConjugationError` with variants `Json`,
    `BadPartOfSpeech { name, pos }`, `UnresolvedNextType { name, next }`

- [ ] **Step 1: Write the failing tests**

Create `ta/crates/jparser/src/conjugation.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("embedded asset must load")
    }

    #[test]
    fn loads_all_thirty_two_types() {
        assert_eq!(table().types().len(), 32);
    }

    #[test]
    fn retains_all_duplicate_named_types() {
        // vk, vs, v5r-i and v5uru each appear twice. Both entries must survive:
        // GetDictEntry's kuruHack pairs a kanji-suffix type with its kana twin
        // to reconstruct readings for irregular verbs.
        let t = table();
        for name in ["vk", "vs", "v5r-i", "v5uru"] {
            assert_eq!(t.types_named(name).len(), 2, "{name} should appear twice");
        }
        assert_eq!(t.types_named("v1").len(), 1);
        assert_eq!(t.types_named("nonexistent").len(), 0);
    }

    #[test]
    fn fixed_tense_discriminants_are_stable() {
        let t = table();
        assert_eq!(t.tense_name(TENSE_REMOVE), Some("Remove"));
        assert_eq!(t.tense_name(TENSE_NON_PAST), Some("Non-past"));
        assert_eq!(t.tense_name(TENSE_STEM), Some("Stem"));
        assert_eq!(t.tense_name(TENSE_POTENTIAL), Some("Potential"));
    }

    #[test]
    fn interns_tense_names_beyond_the_static_list() {
        let t = table();
        // Contributed by the asset, not the static list.
        assert!(
            (0..64).any(|i| t.tense_name(i) == Some("Past Volitional")),
            "asset tense names must be interned"
        );
    }

    #[test]
    fn adjective_types_are_flagged() {
        let t = table();
        let adj = t.types_named("adj-i");
        assert!(t.types()[adj[0]].is_adjective);
        let verb = t.types_named("v1");
        assert!(!t.types()[verb[0]].is_adjective);
    }

    #[test]
    fn chain_only_types_are_present() {
        // These are never matched against a dictionary POS; they exist solely
        // as Next Type targets, so they must still load.
        let t = table();
        for name in ["copula", "adj-ta", "v-i-stem", "v-a-stem", "v-ta-stem", "v-u-stem"] {
            assert!(!t.types_named(name).is_empty(), "{name} must load");
        }
    }

    #[test]
    fn resolves_every_next_type_reference() {
        // 223 conjugations carry a Next Type; all must resolve or load fails.
        let t = table();
        let linked = t
            .types()
            .iter()
            .flat_map(|ty| &ty.conjugations)
            .filter(|c| c.next_verb_type.is_some())
            .count();
        assert_eq!(linked, 223);
    }

    #[test]
    fn every_link_target_has_a_remove_tense_conjugation() {
        let t = table();
        for ty in t.types() {
            for c in &ty.conjugations {
                let Some(next) = c.next_verb_type else { continue };
                let target = &t.types()[next];
                assert!(
                    target
                        .conjugations
                        .iter()
                        .any(|c2| c2.tense == target.remove_tense && c2.form.0 == 0),
                    "target type {} needs a remove/form-0 conjugation",
                    target.name
                );
            }
        }
    }

    #[test]
    fn remove_tense_is_remove_or_non_past() {
        // ta-old defaults remove_tense to NON_PAST and switches to REMOVE only
        // when the type declares a "Remove" tense.
        let t = table();
        for ty in t.types() {
            assert!(ty.remove_tense == TENSE_REMOVE || ty.remove_tense == TENSE_NON_PAST);
        }
    }

    #[test]
    fn form_packs_formal_and_negative_bits() {
        assert_eq!(Form::from_flags(false, false).0, 0);
        assert_eq!(Form::from_flags(true, false).0, 1);
        assert_eq!(Form::from_flags(false, true).0, 2);
        assert_eq!(Form::from_flags(true, true).0, 3);
        assert!(Form(3).is_formal());
        assert!(Form(3).is_negative());
        assert!(!Form(1).is_negative());
        assert!(!Form(2).is_formal());
    }

    #[test]
    fn max_conj_depth_matches_ta_old() {
        assert_eq!(MAX_CONJ_DEPTH, 5);
    }

    #[test]
    fn rejects_an_unresolvable_next_type() {
        let json = r#"[
          {"Name":"v1","Part of Speech":"Verb","Tenses":[
            {"Formal":false,"Negative":false,"Suffix":"る","Tense":"Non-past",
             "Next Type":"does-not-exist"}
          ]}
        ]"#;
        let err = ConjugationTable::from_json(json).unwrap_err();
        assert!(matches!(err, ConjugationError::UnresolvedNextType { .. }), "got {err:?}");
    }

    #[test]
    fn rejects_an_unknown_part_of_speech() {
        let json = r#"[{"Name":"x","Part of Speech":"Noun","Tenses":[]}]"#;
        let err = ConjugationTable::from_json(json).unwrap_err();
        assert!(matches!(err, ConjugationError::BadPartOfSpeech { .. }), "got {err:?}");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = ConjugationTable::from_json("{not json").unwrap_err();
        assert!(matches!(err, ConjugationError::Json(_)), "got {err:?}");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser conjugation`
Expected: FAIL — `failed to resolve: use of undeclared type 'ConjugationTable'`.

- [ ] **Step 3: Implement the types**

Insert above the test module, after a GPL v2 header comment:

```rust
//! The conjugation table, ported from `LoadConjugationTable`
//! (`ta-old/exe/util/Dictionary.cpp:584`).
//!
//! Two kinds of type live in this table and the distinction matters:
//!
//! * **Entry types** are reachable from a JMdict part-of-speech tag. Their
//!   names *are* EDICT POS codes: `v1`, `v5r`, `adj-i`, `vs-i`, and so on.
//! * **Chain-only types** are never matched against a dictionary POS and exist
//!   solely as `Next Type` targets: `copula`, `adj-ta`, `v-i-stem`,
//!   `v-a-stem`, `v-ta-stem`, `v-u-stem`.
//!
//! Four names appear twice (`vk`, `vs`, `v5r-i`, `v5uru`). This is deliberate:
//! one entry carries kanji-form suffixes and its twin carries kana-form
//! suffixes, which is what lets readings be reconstructed for irregular verbs.
//! Lookup therefore returns *all* matching type ids, never just the first.

use serde::Deserialize;

pub type TenseId = usize;
pub type VerbTypeId = usize;

/// Maximum recursive verb conjugations, ta-old's `MAX_CONJ_DEPTH`.
pub const MAX_CONJ_DEPTH: usize = 5;

pub const TENSE_REMOVE: TenseId = 0;
pub const TENSE_NON_PAST: TenseId = 1;
pub const TENSE_STEM: TenseId = 2;
pub const TENSE_POTENTIAL: TenseId = 3;

/// Tense names seeded in fixed order. The first four positions are
/// special-cased by the matcher and must not be reordered. Names beyond this
/// list are interned from the asset in encounter order.
const STATIC_TENSES: &[&str] = &[
    "Remove", "Non-past", "Stem", "Potential", "Past", "Te-form", "Conditional",
    "Provisional", "Passive", "Causative", "Caus-Pass", "Volitional",
    "Conjectural", "Adverbal", "Alternative", "Imperative", "Imperfective",
    "Continuative", "Hypothetical", "Prenominal",
];

const PART_OF_SPEECH_VERB: &str = "Verb";
const PART_OF_SPEECH_ADJ: &str = "Adj";

/// Formality and polarity, packed as ta-old did: `formal | negative << 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Form(pub u8);

impl Form {
    pub const FORMAL: u8 = 1;
    pub const NEGATIVE: u8 = 2;

    pub fn from_flags(formal: bool, negative: bool) -> Self {
        Form(u8::from(formal) | (u8::from(negative) << 1))
    }
    pub fn is_formal(self) -> bool {
        self.0 & Self::FORMAL != 0
    }
    pub fn is_negative(self) -> bool {
        self.0 & Self::NEGATIVE != 0
    }
}

#[derive(Debug, Clone)]
pub struct Conjugation {
    pub tense: TenseId,
    pub form: Form,
    /// Suffix to match against the source text. For linked conjugations this
    /// has already had the target type's remove-suffix trimmed off.
    pub suffix: String,
    pub next_verb_type: Option<VerbTypeId>,
}

#[derive(Debug, Clone)]
pub struct VerbType {
    pub name: String,
    pub is_adjective: bool,
    /// Tense whose suffix is stripped to form the stem: `TENSE_REMOVE` when the
    /// type declares one, otherwise `TENSE_NON_PAST`.
    pub remove_tense: TenseId,
    pub conjugations: Vec<Conjugation>,
}

#[derive(Debug, Clone)]
pub struct ConjugationTable {
    types: Vec<VerbType>,
    tense_names: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConjugationError {
    #[error("conjugation asset is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verb type {name:?} has unsupported part of speech {pos:?}")]
    BadPartOfSpeech { name: String, pos: String },
    #[error(
        "verb type {name:?} references Next Type {next:?}, which has no \
         remove-tense conjugation whose suffix matches"
    )]
    UnresolvedNextType { name: String, next: String },
}

/// Raw asset shape. Field names match ta-old's JSON exactly.
#[derive(Deserialize)]
struct RawType {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Part of Speech")]
    part_of_speech: String,
    #[serde(rename = "Tenses")]
    tenses: Vec<RawConjugation>,
}

#[derive(Deserialize)]
struct RawConjugation {
    #[serde(rename = "Formal")]
    formal: bool,
    #[serde(rename = "Negative")]
    negative: bool,
    #[serde(rename = "Suffix")]
    suffix: String,
    #[serde(rename = "Tense")]
    tense: String,
    #[serde(rename = "Next Type")]
    next_type: Option<String>,
}
```

- [ ] **Step 4: Implement loading and `Next Type` resolution**

Append below the raw types, still above the test module:

```rust
const EMBEDDED_ASSET: &str = include_str!("../assets/conjugations.json");

impl ConjugationTable {
    pub fn load_embedded() -> Result<Self, ConjugationError> {
        Self::from_json(EMBEDDED_ASSET)
    }

    pub fn from_json(json: &str) -> Result<Self, ConjugationError> {
        let raw: Vec<RawType> = serde_json::from_str(json)?;
        let mut tense_names: Vec<String> =
            STATIC_TENSES.iter().map(|s| (*s).to_string()).collect();
        let mut types = Vec::with_capacity(raw.len());
        // Parallel to `types`: the unresolved Next Type name per conjugation,
        // so resolution can run once every type exists.
        let mut pending: Vec<Vec<Option<String>>> = Vec::with_capacity(raw.len());

        for rt in raw {
            let is_adjective = match rt.part_of_speech.as_str() {
                PART_OF_SPEECH_ADJ => true,
                PART_OF_SPEECH_VERB => false,
                other => {
                    return Err(ConjugationError::BadPartOfSpeech {
                        name: rt.name,
                        pos: other.to_string(),
                    })
                }
            };

            let mut remove_tense = TENSE_NON_PAST;
            let mut conjugations = Vec::with_capacity(rt.tenses.len());
            let mut names = Vec::with_capacity(rt.tenses.len());

            for rc in rt.tenses {
                let tense = match tense_names.iter().position(|n| *n == rc.tense) {
                    Some(id) => id,
                    None => {
                        tense_names.push(rc.tense.clone());
                        tense_names.len() - 1
                    }
                };
                if tense == TENSE_REMOVE {
                    remove_tense = TENSE_REMOVE;
                }
                conjugations.push(Conjugation {
                    tense,
                    form: Form::from_flags(rc.formal, rc.negative),
                    suffix: rc.suffix,
                    next_verb_type: None,
                });
                names.push(rc.next_type);
            }

            types.push(VerbType { name: rt.name, is_adjective, remove_tense, conjugations });
            pending.push(names);
        }

        // Resolve chained conjugations. For each conjugation carrying a Next
        // Type, find that target type's remove-tense/form-0 conjugation whose
        // suffix is a suffix of this one, trim it off, and store the link.
        // This is what allows conjugations to stack (て + いる + ない), and it
        // is ta-old's four nested loops in LoadConjugationTable.
        for ti in 0..types.len() {
            for ci in 0..types[ti].conjugations.len() {
                let Some(target_name) = pending[ti][ci].clone() else { continue };
                let suffix = types[ti].conjugations[ci].suffix.clone();
                let mut link = None;

                'outer: for tj in 0..types.len() {
                    if types[tj].name != target_name {
                        continue;
                    }
                    let remove_tense = types[tj].remove_tense;
                    for c2 in &types[tj].conjugations {
                        if c2.tense != remove_tense || c2.form.0 != 0 {
                            continue;
                        }
                        if let Some(trimmed) = strip_unified_suffix(&suffix, &c2.suffix) {
                            link = Some((tj, trimmed));
                            break 'outer;
                        }
                    }
                }

                let Some((target, trimmed)) = link else {
                    return Err(ConjugationError::UnresolvedNextType {
                        name: types[ti].name.clone(),
                        next: target_name,
                    });
                };
                types[ti].conjugations[ci].suffix = trimmed;
                types[ti].conjugations[ci].next_verb_type = Some(target);
            }
        }

        Ok(ConjugationTable { types, tense_names })
    }

    pub fn types(&self) -> &[VerbType] {
        &self.types
    }

    pub fn tense_name(&self, id: TenseId) -> Option<&str> {
        self.tense_names.get(id).map(String::as_str)
    }

    /// All type ids with this name. Returns more than one for `vk`, `vs`,
    /// `v5r-i`, and `v5uru`; callers must handle every result.
    pub fn types_named(&self, name: &str) -> Vec<VerbTypeId> {
        self.types
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name == name)
            .map(|(i, _)| i)
            .collect()
    }
}

/// If `target` is a kana-insensitive suffix of `suffix`, return `suffix` with it
/// removed. Comparison uses `unify` so hiragana and katakana forms match.
fn strip_unified_suffix(suffix: &str, target: &str) -> Option<String> {
    let s: Vec<char> = suffix.chars().collect();
    let t: Vec<char> = target.chars().collect();
    if t.len() > s.len() {
        return None;
    }
    let split = s.len() - t.len();
    let matches = s[split..]
        .iter()
        .zip(t.iter())
        .all(|(a, b)| crate::kana::unify(*a) == crate::kana::unify(*b));
    matches.then(|| s[..split].iter().collect())
}
```

- [ ] **Step 5: Register the module**

In `ta/crates/jparser/src/lib.rs`:

```rust
pub mod conjugation;
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser conjugation`
Expected: PASS, 14 tests. `resolves_every_next_type_reference` must report
exactly 223.

- [ ] **Step 7: Commit**

```bash
cd ta && git add crates/jparser/src/conjugation.rs crates/jparser/src/lib.rs
git commit -m "feat: add conjugation table loader with Next Type resolution

Ports LoadConjugationTable from ta-old/exe/util/Dictionary.cpp:584.

Next Type resolution is the highest-risk part of the port: for each
chained conjugation, find the target type's remove-tense/form-0
conjugation whose suffix is a suffix of this one, trim it, and link.
That is what lets conjugations stack. All 223 references must resolve
or loading fails loudly, matching ta-old's strictness.

Duplicate type names are all retained rather than first-match-wins,
because GetDictEntry pairs kanji-suffix types with their kana twins to
reconstruct readings for irregular verbs."
```

---

## Task 5: JMdict streaming parse

**Files:**
- Create: `ta/crates/jparser/src/jmdict.rs`
- Create: `ta/crates/jparser/tests/fixtures/jmdict_mini.xml`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces:
  - `pub struct jmdict::KanjiForm { pub text: String, pub has_priority: bool }`
  - `pub struct jmdict::ReadingForm { pub text: String, pub has_priority: bool }`
  - `pub struct jmdict::RawSense { pub pos: Vec<String>, pub glosses: Vec<String>, pub xrefs: Vec<String>, pub misc: Vec<String>, pub info: Vec<String> }`
  - `pub struct jmdict::RawEntry { pub id: u32, pub kanji: Vec<KanjiForm>, pub readings: Vec<ReadingForm>, pub senses: Vec<RawSense> }`
  - `pub fn jmdict::parse_entries<R: BufRead>(r: R) -> JmdictReader<R>`
  - `pub struct jmdict::JmdictReader<R>`: `Iterator<Item = Result<RawEntry, JmdictError>>`, plus `pub fn skipped_count(&self) -> usize`
  - `pub enum jmdict::JmdictError` with variants `Xml`, `BadEntry { id, reason }`

**Critical detail — POS codes arrive as entity references.** JMdict writes parts
of speech as `<pos>&v5r;</pos>`, and its internal DTD defines
`<!ENTITY v5r "Godan verb with 'ru' ending">`. We need the **code** (`v5r`),
because that is what matches conjugation type names — not the expanded
description. `quick-xml` does not process DTDs, so an entity reference surfaces
as a distinct event rather than as text. Step 1 measures exactly how before any
parsing code is written.

- [ ] **Step 1: Create the fixture**

Create `ta/crates/jparser/tests/fixtures/jmdict_mini.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
<!ENTITY prt "particle">
<!ENTITY v1 "Ichidan verb">
<!ENTITY v5r "Godan verb with `ru' ending">
<!ENTITY adj-i "adjective (keiyoushi)">
<!ENTITY uk "word usually written using kana alone">
]>
<JMdict>
<entry>
<ent_seq>1000010</ent_seq>
<k_ele><keb>言う</keb><ke_pri>ichi1</ke_pri><ke_pri>news1</ke_pri></k_ele>
<r_ele><reb>いう</reb><re_pri>ichi1</re_pri></r_ele>
<r_ele><reb>ゆう</reb></r_ele>
<sense>
<pos>&v5r;</pos>
<gloss>to say</gloss>
<gloss>to utter</gloss>
<xref>言われる</xref>
<misc>&uk;</misc>
<s_inf>usually written in kana</s_inf>
</sense>
</entry>
<entry>
<ent_seq>1000020</ent_seq>
<r_ele><reb>は</reb></r_ele>
<sense><pos>&prt;</pos><gloss>topic marker</gloss></sense>
</entry>
<entry>
<ent_seq>1000030</ent_seq>
<k_ele><keb>高い</keb></k_ele>
<r_ele><reb>たかい</reb></r_ele>
<sense><pos>&adj-i;</pos><gloss>high</gloss><gloss>tall</gloss></sense>
</entry>
</JMdict>
```

- [ ] **Step 2: Spike — determine how the parser surfaces `&v5r;`**

Create `ta/crates/jparser/src/jmdict.rs` with only this temporary module:

```rust
#[cfg(test)]
mod spike {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    #[test]
    #[ignore = "spike: run manually to inspect parser events"]
    fn show_how_entities_surface() {
        let xml = include_str!("../tests/fixtures/jmdict_mini.xml");
        let mut r = Reader::from_str(xml);
        let mut log = Vec::new();
        loop {
            match r.read_event() {
                Ok(Event::Eof) => break,
                Ok(ev) => log.push(format!("{ev:?}")),
                Err(e) => {
                    log.push(format!("ERROR {e}"));
                    break;
                }
            }
        }
        panic!("{}", log.join("\n"));
    }
}
```

Run: `cd ta && cargo test -p jparser spike -- --ignored --nocapture`
Expected: the panic output lists every event. Find the one carrying `v5r` and
record which variant it is — `Event::GeneralRef`, an `Event::Text` holding the
literal `&v5r;`, or an error.

**Write the answer into the `SPIKE RESULT` comment in Step 4 before continuing,
and delete this spike module.** The `Event::GeneralRef` arm in Step 5 must be
adjusted to whatever this actually reports.

- [ ] **Step 3: Write the failing tests**

Replace the spike module with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<RawEntry> {
        let xml = include_str!("../tests/fixtures/jmdict_mini.xml");
        parse_entries(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture must parse")
    }

    #[test]
    fn reads_every_entry() {
        assert_eq!(entries().len(), 3);
    }

    #[test]
    fn reads_entry_ids() {
        assert_eq!(entries()[0].id, 1000010);
        assert_eq!(entries()[2].id, 1000030);
    }

    #[test]
    fn reads_kanji_and_reading_forms_in_order() {
        let e = &entries()[0];
        assert_eq!(e.kanji.len(), 1);
        assert_eq!(e.kanji[0].text, "言う");
        assert_eq!(e.readings.len(), 2);
        assert_eq!(e.readings[0].text, "いう");
        assert_eq!(e.readings[1].text, "ゆう");
    }

    #[test]
    fn reads_priority_markers() {
        let e = &entries()[0];
        assert!(e.kanji[0].has_priority);
        assert!(e.readings[0].has_priority);
        assert!(!e.readings[1].has_priority);
    }

    #[test]
    fn resolves_pos_entities_to_codes_not_descriptions() {
        // The whole point: we need "v5r", not "Godan verb with `ru' ending",
        // because the code is what matches conjugation type names.
        assert_eq!(entries()[0].senses[0].pos, vec!["v5r"]);
        assert_eq!(entries()[1].senses[0].pos, vec!["prt"]);
        assert_eq!(entries()[2].senses[0].pos, vec!["adj-i"]);
    }

    #[test]
    fn reads_glosses_in_order() {
        assert_eq!(entries()[0].senses[0].glosses, vec!["to say", "to utter"]);
    }

    #[test]
    fn reads_xrefs_misc_and_info() {
        let s = &entries()[0].senses[0];
        assert_eq!(s.xrefs, vec!["言われる"]);
        assert_eq!(s.misc, vec!["uk"]);
        assert_eq!(s.info, vec!["usually written in kana"]);
    }

    #[test]
    fn handles_entries_with_no_kanji() {
        let e = &entries()[1];
        assert!(e.kanji.is_empty());
        assert_eq!(e.readings[0].text, "は");
    }

    #[test]
    fn reports_malformed_xml_as_an_error() {
        let bad = "<JMdict><entry><ent_seq>1</ent_seq>";
        let result: Result<Vec<_>, _> =
            parse_entries(std::io::Cursor::new(bad)).collect();
        assert!(result.is_err());
    }

    #[test]
    fn skips_an_entry_with_a_non_numeric_id_and_counts_it() {
        let bad = r#"<JMdict>
<entry><ent_seq>abc</ent_seq><r_ele><reb>あ</reb></r_ele></entry>
<entry><ent_seq>2</ent_seq><r_ele><reb>い</reb></r_ele></entry>
</JMdict>"#;
        let mut reader = parse_entries(std::io::Cursor::new(bad));
        let all: Vec<_> = reader.by_ref().collect();
        let ok = all.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 1, "the good entry must still be returned");
        assert_eq!(reader.skipped_count(), 1);
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser jmdict`
Expected: FAIL — `cannot find function 'parse_entries' in this scope`.

- [ ] **Step 5: Implement the types**

Insert above the test module, after a GPL v2 header comment:

```rust
//! Streaming JMdict XML reader.
//!
//! Replaces ta-old's EDICT2 line parser (`CreateDict`,
//! `ta-old/exe/util/Dictionary.cpp:246`). JMdict carries structured data where
//! ta-old did string surgery on English glosses: real `<pos>` tags instead of
//! `strncmp` against a gloss prefix, and real `<ke_pri>`/`<re_pri>` markers
//! instead of searching for the substring "(P)".
//!
//! Parts of speech arrive as entity references (`<pos>&v5r;</pos>`) whose
//! expansions are defined in JMdict's internal DTD. We want the *code*, since
//! that is what matches conjugation type names.
//!
//! SPIKE RESULT (Task 5 Step 2): <record here which quick-xml event carries the
//! entity name, then make the matching arm below agree with it>.

use std::io::BufRead;

use quick_xml::events::Event;
use quick_xml::Reader;

/// Priority markers that mean "common word", per JMdict's documentation.
/// Replaces ta-old's `(P)` substring search.
const PRIORITY_MARKERS: &[&str] = &["news1", "ichi1", "spec1", "gai1"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanjiForm {
    pub text: String,
    pub has_priority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingForm {
    pub text: String,
    pub has_priority: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSense {
    /// EDICT POS codes, e.g. `v5r`, `prt`, `adj-i`.
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
    pub xrefs: Vec<String>,
    pub misc: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub id: u32,
    pub kanji: Vec<KanjiForm>,
    pub readings: Vec<ReadingForm>,
    pub senses: Vec<RawSense>,
}

#[derive(Debug, thiserror::Error)]
pub enum JmdictError {
    #[error("malformed JMdict XML: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("entry {id:?} is malformed: {reason}")]
    BadEntry { id: String, reason: String },
}

pub fn parse_entries<R: BufRead>(reader: R) -> JmdictReader<R> {
    JmdictReader {
        reader: Reader::from_reader(reader),
        buf: Vec::new(),
        skipped: 0,
        done: false,
    }
}

pub struct JmdictReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
    skipped: usize,
    done: bool,
}

impl<R: BufRead> JmdictReader<R> {
    /// Entries skipped because they were malformed. Never silently discarded —
    /// the caller surfaces this count.
    pub fn skipped_count(&self) -> usize {
        self.skipped
    }
}
```

- [ ] **Step 6: Implement the iterator**

Append below the types, still above the test module:

```rust
/// Leaf elements whose text we accumulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Other,
}

impl<R: BufRead> Iterator for JmdictReader<R> {
    type Item = Result<RawEntry, JmdictError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut in_entry = false;
        let mut id_text = String::new();
        let mut kanji: Vec<KanjiForm> = Vec::new();
        let mut readings: Vec<ReadingForm> = Vec::new();
        let mut senses: Vec<RawSense> = Vec::new();
        let mut field = Field::None;
        let mut text = String::new();

        loop {
            self.buf.clear();
            let event = match self.reader.read_event_into(&mut self.buf) {
                Ok(ev) => ev,
                Err(e) => {
                    self.done = true;
                    return Some(Err(JmdictError::Xml(e)));
                }
            };

            match event {
                Event::Eof => {
                    self.done = true;
                    return None;
                }
                Event::Start(e) => match e.local_name().as_ref() {
                    b"entry" => {
                        in_entry = true;
                        id_text.clear();
                        kanji.clear();
                        readings.clear();
                        senses.clear();
                    }
                    b"k_ele" => {
                        kanji.push(KanjiForm { text: String::new(), has_priority: false })
                    }
                    b"r_ele" => readings
                        .push(ReadingForm { text: String::new(), has_priority: false }),
                    b"sense" => senses.push(RawSense::default()),
                    _ => {
                        field = Field::Other;
                        text.clear();
                    }
                },
                // Entity reference: carries the POS/misc *code*. Adjust this arm
                // to the variant the Step 2 spike identified.
                Event::GeneralRef(r) => {
                    if in_entry {
                        text.push_str(&String::from_utf8_lossy(r.as_ref()));
                    }
                }
                Event::Text(t) => {
                    if in_entry && field != Field::None {
                        match t.unescape() {
                            Ok(s) => text.push_str(&s),
                            Err(e) => {
                                self.done = true;
                                return Some(Err(JmdictError::Xml(e)));
                            }
                        }
                    }
                }
                Event::End(e) => {
                    let name = e.local_name();
                    let value = text.trim().to_string();
                    match name.as_ref() {
                        b"entry" => {
                            let Ok(id) = id_text.trim().parse::<u32>() else {
                                self.skipped += 1;
                                in_entry = false;
                                field = Field::None;
                                text.clear();
                                continue;
                            };
                            return Some(Ok(RawEntry {
                                id,
                                kanji: std::mem::take(&mut kanji),
                                readings: std::mem::take(&mut readings),
                                senses: std::mem::take(&mut senses),
                            }));
                        }
                        b"ent_seq" => id_text = value,
                        b"keb" => {
                            if let Some(k) = kanji.last_mut() {
                                k.text = value;
                            }
                        }
                        b"ke_pri" => {
                            if let Some(k) = kanji.last_mut() {
                                if PRIORITY_MARKERS.contains(&value.as_str()) {
                                    k.has_priority = true;
                                }
                            }
                        }
                        b"reb" => {
                            if let Some(r) = readings.last_mut() {
                                r.text = value;
                            }
                        }
                        b"re_pri" => {
                            if let Some(r) = readings.last_mut() {
                                if PRIORITY_MARKERS.contains(&value.as_str()) {
                                    r.has_priority = true;
                                }
                            }
                        }
                        b"pos" | b"gloss" | b"xref" | b"misc" | b"s_inf" => {
                            if let Some(s) = senses.last_mut() {
                                match name.as_ref() {
                                    b"pos" => s.pos.push(value),
                                    b"gloss" => s.glosses.push(value),
                                    b"xref" => s.xrefs.push(value),
                                    b"misc" => s.misc.push(value),
                                    _ => s.info.push(value),
                                }
                            }
                        }
                        _ => {}
                    }
                    field = Field::None;
                    text.clear();
                }
                _ => {}
            }
        }
    }
}
```

- [ ] **Step 7: Register the module and run the tests**

Add `pub mod jmdict;` to `lib.rs`, then run:
`cd ta && cargo test -p jparser jmdict`
Expected: PASS, 10 tests. If
`resolves_pos_entities_to_codes_not_descriptions` fails, the entity arm does not
match what the Step 2 spike reported — fix the arm, not the test.

- [ ] **Step 8: Commit**

```bash
cd ta && git add crates/jparser/src/jmdict.rs crates/jparser/src/lib.rs \
  crates/jparser/tests/fixtures/jmdict_mini.xml
git commit -m "feat: add streaming JMdict XML reader

Replaces ta-old's EDICT2 line parser. JMdict gives structured <pos>
tags and <ke_pri>/<re_pri> priority markers instead of ta-old's
substring search for '(P)' and strncmp against gloss prefixes.

Parts of speech arrive as entity references whose expansions are
defined in JMdict's internal DTD; we capture the entity name because
the code (v5r) is what matches conjugation type names, not the
expanded description.

Malformed entries are skipped and counted, never silently dropped."
```

---

## Task 6: Headword records and flag derivation

**Files:**
- Create: `ta/crates/jparser/src/record.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes: `jmdict::RawEntry` (Task 5), `conjugation::ConjugationTable` (Task 4)
- Produces:
  - `pub struct record::WordFlags(pub u16)` with consts `PRIMARY = 0x0001`,
    `PRONOUNCE = 0x0002`, `COMMON_LINE = 0x0004`, `COMMON = 0x0008`,
    `PARTICLE = 0x0010`, `COUNTER = 0x0020`, `IS_NAME = 0x0040`, and methods
    `contains(self, WordFlags) -> bool`, `insert(&mut self, WordFlags)`,
    `remove(&mut self, WordFlags)`
  - `pub struct record::HeadwordRecord { pub surface: String, pub flags: WordFlags, pub verb_types: Vec<VerbTypeId>, pub entry_id: u32 }`
  - `pub fn record::headwords(entry: &RawEntry, table: &ConjugationTable) -> Vec<HeadwordRecord>`
  - `pub(crate) fn record::counter_flag(pos: &[&str], misc: &[&str]) -> bool`

Flag values mirror ta-old's `JAP_WORD_*` constants
(`ta-old/exe/util/Dictionary.h:25-43`) so records can be compared against the
original in the Phase 1B differential run.

- [ ] **Step 1: Write the failing tests**

Create `ta/crates/jparser/src/record.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::ConjugationTable;
    use crate::jmdict::{parse_entries, RawEntry};

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().unwrap()
    }

    fn fixture() -> Vec<RawEntry> {
        let xml = include_str!("../tests/fixtures/jmdict_mini.xml");
        parse_entries(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn emits_one_record_per_kanji_and_reading_form() {
        let recs = headwords(&fixture()[0], &table());
        let surfaces: Vec<&str> = recs.iter().map(|r| r.surface.as_str()).collect();
        assert_eq!(surfaces, vec!["言う", "いう", "ゆう"]);
    }

    #[test]
    fn marks_only_the_first_headword_primary() {
        let recs = headwords(&fixture()[0], &table());
        assert!(recs[0].flags.contains(WordFlags::PRIMARY));
        assert!(!recs[1].flags.contains(WordFlags::PRIMARY));
        assert!(!recs[2].flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn marks_readings_pronounce_when_the_entry_has_kanji() {
        let recs = headwords(&fixture()[0], &table());
        assert!(!recs[0].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[1].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[2].flags.contains(WordFlags::PRONOUNCE));
    }

    #[test]
    fn treats_a_kana_only_reading_as_primary() {
        // は has no kanji form, so its reading is the primary spelling.
        let recs = headwords(&fixture()[1], &table());
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[0].flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn sets_common_per_form_and_common_line_per_entry() {
        // 言う: keb and the first reb are marked; ゆう is not.
        let recs = headwords(&fixture()[0], &table());
        assert!(recs[0].flags.contains(WordFlags::COMMON));
        assert!(recs[1].flags.contains(WordFlags::COMMON));
        assert!(!recs[2].flags.contains(WordFlags::COMMON));
        for r in &recs {
            assert!(r.flags.contains(WordFlags::COMMON_LINE));
        }
    }

    #[test]
    fn omits_common_line_when_no_form_has_priority() {
        let recs = headwords(&fixture()[2], &table());
        assert!(!recs[0].flags.contains(WordFlags::COMMON_LINE));
    }

    #[test]
    fn sets_particle_flag_from_pos() {
        let recs = headwords(&fixture()[1], &table());
        assert!(recs[0].flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn does_not_set_particle_on_verbs() {
        let recs = headwords(&fixture()[0], &table());
        assert!(!recs[0].flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn attaches_verb_types_from_pos_codes() {
        let t = table();
        let recs = headwords(&fixture()[0], &t);
        let expected = t.types_named("v5r");
        assert!(!expected.is_empty(), "v5r must exist in the table");
        assert_eq!(recs[0].verb_types, expected);
    }

    #[test]
    fn attaches_adjective_types() {
        let t = table();
        let recs = headwords(&fixture()[2], &t);
        assert_eq!(recs[0].verb_types, t.types_named("adj-i"));
    }

    #[test]
    fn leaves_verb_types_empty_for_non_conjugating_words() {
        let recs = headwords(&fixture()[1], &table());
        assert!(recs[0].verb_types.is_empty());
    }

    #[test]
    fn counter_requires_ctr_or_suf_and_forbids_arch() {
        assert!(counter_flag(&["ctr"], &[]));
        assert!(counter_flag(&["suf"], &[]));
        assert!(!counter_flag(&["ctr"], &["arch"]));
        assert!(!counter_flag(&["n"], &[]));
    }

    #[test]
    fn flag_operations_insert_remove_and_test() {
        let mut f = WordFlags::default();
        f.insert(WordFlags::COMMON);
        assert!(f.contains(WordFlags::COMMON));
        f.remove(WordFlags::COMMON);
        assert!(!f.contains(WordFlags::COMMON));
    }

    #[test]
    fn flag_values_match_ta_old_constants() {
        // Kept identical to JAP_WORD_* so the Phase 1B differential run can
        // compare flags directly.
        assert_eq!(WordFlags::PRIMARY.0, 0x0001);
        assert_eq!(WordFlags::PRONOUNCE.0, 0x0002);
        assert_eq!(WordFlags::COMMON_LINE.0, 0x0004);
        assert_eq!(WordFlags::COMMON.0, 0x0008);
        assert_eq!(WordFlags::PARTICLE.0, 0x0010);
        assert_eq!(WordFlags::COUNTER.0, 0x0020);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser record`
Expected: FAIL — `cannot find function 'headwords' in this scope`.

- [ ] **Step 3: Implement `record.rs`**

Insert above the test module, after a GPL v2 header comment:

```rust
//! `RawEntry` to `HeadwordRecord`, with flag derivation.
//!
//! Mirrors the per-headword bookkeeping in ta-old's `CreateDict`
//! (`ta-old/exe/util/Dictionary.cpp:246`). Flag values match ta-old's
//! `JAP_WORD_*` constants (`Dictionary.h:25-43`) so records can be compared
//! against the original during the Phase 1B differential run.

use crate::conjugation::{ConjugationTable, VerbTypeId};
use crate::jmdict::RawEntry;

/// POS codes that make a word a particle for scoring purposes. ta-old's
/// `posList` in `GetPartsOfSpeech` (`Dictionary.cpp:1409`).
const PARTICLE_POS: &[&str] = &["prt", "conj"];
/// POS codes that make a word a counter.
const COUNTER_POS: &[&str] = &["ctr", "suf"];
/// Misc code that disqualifies a counter.
const ARCHAIC_MISC: &str = "arch";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WordFlags(pub u16);

impl WordFlags {
    /// Kanji spelling of a kanji word, or the hiragana of a kana-only word.
    pub const PRIMARY: WordFlags = WordFlags(0x0001);
    /// A reading for something that also has a kanji spelling.
    pub const PRONOUNCE: WordFlags = WordFlags(0x0002);
    /// Any form of this entry carries a priority marker.
    pub const COMMON_LINE: WordFlags = WordFlags(0x0004);
    /// This specific form carries a priority marker.
    pub const COMMON: WordFlags = WordFlags(0x0008);
    pub const PARTICLE: WordFlags = WordFlags(0x0010);
    pub const COUNTER: WordFlags = WordFlags(0x0020);
    /// Reserved for JMnedict. Nothing sets this in v1.
    pub const IS_NAME: WordFlags = WordFlags(0x0040);

    pub fn contains(self, other: WordFlags) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: WordFlags) {
        self.0 |= other.0;
    }
    pub fn remove(&mut self, other: WordFlags) {
        self.0 &= !other.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadwordRecord {
    pub surface: String,
    pub flags: WordFlags,
    /// Every conjugation type this word can take. More than one when the POS
    /// code is duplicated in the table (`vk`, `vs`, `v5r-i`, `v5uru`).
    pub verb_types: Vec<VerbTypeId>,
    pub entry_id: u32,
}

/// True when the sense set makes this a counter: a counter POS present and no
/// archaic marker anywhere. Matches ta-old's
/// `(wcsstr(eng, "(ctr)") || wcsstr(eng, "(suf)")) && !wcsstr(eng, "(arch)")`.
pub(crate) fn counter_flag(pos: &[&str], misc: &[&str]) -> bool {
    let has_counter = pos.iter().any(|p| COUNTER_POS.contains(p));
    let archaic = misc.iter().any(|m| *m == ARCHAIC_MISC);
    has_counter && !archaic
}

/// Expand one entry into headword records: every kanji form, then every reading
/// form, in document order.
pub fn headwords(entry: &RawEntry, table: &ConjugationTable) -> Vec<HeadwordRecord> {
    let all_pos: Vec<&str> = entry
        .senses
        .iter()
        .flat_map(|s| s.pos.iter().map(String::as_str))
        .collect();
    let all_misc: Vec<&str> = entry
        .senses
        .iter()
        .flat_map(|s| s.misc.iter().map(String::as_str))
        .collect();

    let mut shared = WordFlags::default();
    if all_pos.iter().any(|p| PARTICLE_POS.contains(p)) {
        shared.insert(WordFlags::PARTICLE);
    }
    if counter_flag(&all_pos, &all_misc) {
        shared.insert(WordFlags::COUNTER);
    }
    if entry.kanji.iter().any(|k| k.has_priority)
        || entry.readings.iter().any(|r| r.has_priority)
    {
        shared.insert(WordFlags::COMMON_LINE);
    }

    // A POS code may name several types; every one is attached.
    let mut verb_types: Vec<VerbTypeId> = Vec::new();
    for p in &all_pos {
        for id in table.types_named(p) {
            if !verb_types.contains(&id) {
                verb_types.push(id);
            }
        }
    }

    let has_kanji = !entry.kanji.is_empty();
    let mut out = Vec::with_capacity(entry.kanji.len() + entry.readings.len());

    for (i, k) in entry.kanji.iter().enumerate() {
        let mut flags = shared;
        if i == 0 {
            flags.insert(WordFlags::PRIMARY);
        }
        if k.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        out.push(HeadwordRecord {
            surface: k.text.clone(),
            flags,
            verb_types: verb_types.clone(),
            entry_id: entry.id,
        });
    }

    for (i, r) in entry.readings.iter().enumerate() {
        let mut flags = shared;
        if has_kanji {
            flags.insert(WordFlags::PRONOUNCE);
        } else if i == 0 {
            flags.insert(WordFlags::PRIMARY);
        }
        if r.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        out.push(HeadwordRecord {
            surface: r.text.clone(),
            flags,
            verb_types: verb_types.clone(),
            entry_id: entry.id,
        });
    }

    out
}
```

- [ ] **Step 4: Register the module and run the tests**

Add `pub mod record;` to `lib.rs`, then run:
`cd ta && cargo test -p jparser record`
Expected: PASS, 14 tests.

- [ ] **Step 5: Commit**

```bash
cd ta && git add crates/jparser/src/record.rs crates/jparser/src/lib.rs
git commit -m "feat: derive headword records and flags from JMdict entries

Flag values mirror ta-old's JAP_WORD_* constants so records can be
compared against the original in the Phase 1B differential run.

COMMON is per-form and COMMON_LINE is entry-wide, matching ta-old's
distinction between a (P) on a specific spelling and a (P) anywhere on
the line. A POS code naming several conjugation types attaches all of
them."
```

---

## Task 7: Verb stem generation with the v5 fallback

**Files:**
- Create: `ta/crates/jparser/src/stem.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes: `record::HeadwordRecord`, `record::WordFlags` (Task 6),
  `conjugation::{ConjugationTable, VerbTypeId}` (Task 4), `kana::unify` (Task 1)
- Produces:
  - `pub struct stem::StemOptions { pub v5_misannotation_fallback: bool }`, `Default` = `true`
  - `pub struct stem::StemStats { pub exact_stems: usize, pub v5_fallback_stems: usize, pub empty_stems: usize }`, `Default` + `Copy`
  - `pub fn stem::generate_stems(record: &HeadwordRecord, table: &ConjugationTable, opts: &StemOptions, stats: &mut StemStats) -> Vec<HeadwordRecord>`

Stems are **additional index entries**, not replacements. Phase 1B's matcher
walks the FST to a stem and matches conjugation suffixes onward from it, so
without these no verb ever conjugates.

- [ ] **Step 1: Write the failing tests**

Create `ta/crates/jparser/src/stem.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::ConjugationTable;
    use crate::record::WordFlags;

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().unwrap()
    }

    fn record(surface: &str, type_name: &str, t: &ConjugationTable) -> HeadwordRecord {
        HeadwordRecord {
            surface: surface.to_string(),
            flags: WordFlags::PRIMARY,
            verb_types: t.types_named(type_name),
            entry_id: 1,
        }
    }

    fn surfaces(recs: &[HeadwordRecord]) -> Vec<&str> {
        recs.iter().map(|r| r.surface.as_str()).collect()
    }

    #[test]
    fn strips_the_remove_suffix_to_form_a_stem() {
        // 食べる is v1 (ichidan); removing る leaves 食べ.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(surfaces(&out).contains(&"食べ"), "got {:?}", surfaces(&out));
    }

    #[test]
    fn tags_each_stem_with_exactly_the_type_that_produced_it() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let stem = out.iter().find(|r| r.surface == "食べ").unwrap();
        assert_eq!(stem.verb_types.len(), 1, "a stem carries exactly one type");
        assert!(t.types_named("v1").contains(&stem.verb_types[0]));
    }

    #[test]
    fn v5_fallback_adds_stems_for_same_length_v5_siblings() {
        // 言う is annotated v5u. With the fallback on it also gets stems for
        // the other three-character v5 types, absorbing EDICT mis-annotation.
        let t = table();
        let mut on = StemStats::default();
        let with = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: true },
            &mut on,
        );
        let mut off = StemStats::default();
        let without = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: false },
            &mut off,
        );
        assert!(
            with.len() > without.len(),
            "fallback must add stems: {} vs {}",
            with.len(),
            without.len()
        );
        assert!(on.v5_fallback_stems > 0);
        assert_eq!(off.v5_fallback_stems, 0);
    }

    #[test]
    fn v5_fallback_requires_equal_length_type_names() {
        // v5u (3 chars) must not cross-generate with v5u-s (5 chars).
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: true },
            &mut stats,
        );
        let long_named: Vec<VerbTypeId> = t
            .types()
            .iter()
            .enumerate()
            .filter(|(_, ty)| ty.name.starts_with("v5") && ty.name.len() != 3)
            .map(|(i, _)| i)
            .collect();
        for r in &out {
            for vt in &r.verb_types {
                assert!(!long_named.contains(vt), "crossed a name-length boundary");
            }
        }
    }

    #[test]
    fn v5_fallback_does_not_apply_to_non_v5_types() {
        let t = table();
        let mut on = StemStats::default();
        let with = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: true },
            &mut on,
        );
        let mut off = StemStats::default();
        let without = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: false },
            &mut off,
        );
        assert_eq!(with.len(), without.len());
        assert_eq!(on.v5_fallback_stems, 0);
    }

    #[test]
    fn retains_empty_stems() {
        // ta-old: "len 0 is for verbs which have 0 characters after removing
        // the suffix." A verb whose whole surface is the remove-suffix yields
        // an empty stem, and it must survive.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("る", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(out.iter().any(|r| r.surface.is_empty()));
        assert_eq!(stats.empty_stems, 1);
    }

    #[test]
    fn deduplicates_identical_stem_and_type_pairs() {
        // ta-old's dedupe is dead code; ours works. No two output records may
        // share both a surface and a type.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let mut seen: Vec<(String, Vec<VerbTypeId>)> = Vec::new();
        for r in &out {
            let key = (r.surface.clone(), r.verb_types.clone());
            assert!(!seen.contains(&key), "duplicate stem {key:?}");
            seen.push(key);
        }
    }

    #[test]
    fn produces_nothing_for_words_with_no_verb_types() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &HeadwordRecord {
                surface: "は".into(),
                flags: WordFlags::PARTICLE,
                verb_types: vec![],
                entry_id: 1,
            },
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn produces_nothing_when_no_remove_suffix_matches_the_tail() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("xyz", "v1", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: false },
            &mut stats,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn stems_inherit_flags_and_entry_id() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let stem = out.iter().find(|r| r.surface == "食べ").unwrap();
        assert_eq!(stem.entry_id, 1);
        assert!(stem.flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn counts_exact_stems_separately_from_fallback_stems() {
        let t = table();
        let mut stats = StemStats::default();
        generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions { v5_misannotation_fallback: true },
            &mut stats,
        );
        assert!(stats.exact_stems > 0);
        assert!(stats.v5_fallback_stems > 0);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser stem`
Expected: FAIL — `cannot find function 'generate_stems' in this scope`.

- [ ] **Step 3: Implement `stem.rs`**

Insert above the test module, after a GPL v2 header comment:

```rust
//! Verb stem generation, ported from the stem-emitting block of ta-old's
//! `CreateDict` (`ta-old/exe/util/Dictionary.cpp:413-470`).
//!
//! The index holds headwords **and** generated stems as separate entries. The
//! matcher walks to a stem and then matches conjugation suffixes onward from
//! it, so without these no verb ever conjugates.
//!
//! ## The v5 fallback
//!
//! ta-old accepts a candidate conjugation type when its name equals the word's
//! POS code *or* when both names start with `v5` and have the same length. Its
//! comment: *"Fix a couple dozen incorrectly annotated verbs. Doesn't get them
//! all, but gets a lot."* A verb mis-tagged `v5r` therefore also gets stems for
//! `v5k`, `v5m`, `v5t`, and so on.
//!
//! This is preserved because JMdict is the source EDICT2 was generated from, so
//! changing formats does not fix the mis-annotations, and the failure without it
//! is silent: a verb simply never conjugates and nothing says why. It is gated
//! behind `StemOptions::v5_misannotation_fallback` and instrumented via
//! `StemStats` so its real cost and benefit can be measured.

use crate::conjugation::{ConjugationTable, VerbTypeId};
use crate::kana::unify;
use crate::record::HeadwordRecord;

/// Name prefix that qualifies a type for the mis-annotation fallback.
const V5_PREFIX: &str = "v5";

#[derive(Debug, Clone, Copy)]
pub struct StemOptions {
    /// Cross-generate stems for same-length `v5*` types. See module docs.
    pub v5_misannotation_fallback: bool,
}

impl Default for StemOptions {
    fn default() -> Self {
        Self { v5_misannotation_fallback: true }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StemStats {
    /// Stems generated from a type whose name matched the POS code exactly.
    pub exact_stems: usize,
    /// Stems generated only because of the v5 fallback.
    pub v5_fallback_stems: usize,
    /// Stems whose surface is the empty string.
    pub empty_stems: usize,
}

/// True when `candidate` may stand in for `annotated` under the v5 fallback.
fn v5_fallback_applies(annotated: &str, candidate: &str) -> bool {
    annotated.starts_with(V5_PREFIX)
        && candidate.starts_with(V5_PREFIX)
        && annotated.len() == candidate.len()
}

/// If `suffix` is a kana-insensitive suffix of `surface`, return the stem.
fn strip_suffix_unified(surface: &str, suffix: &str) -> Option<String> {
    let s: Vec<char> = surface.chars().collect();
    let t: Vec<char> = suffix.chars().collect();
    if t.len() > s.len() {
        return None;
    }
    let split = s.len() - t.len();
    let matches = s[split..]
        .iter()
        .zip(t.iter())
        .all(|(a, b)| unify(*a) == unify(*b));
    matches.then(|| s[..split].iter().collect())
}

/// Generate every stem record for one headword.
pub fn generate_stems(
    record: &HeadwordRecord,
    table: &ConjugationTable,
    opts: &StemOptions,
    stats: &mut StemStats,
) -> Vec<HeadwordRecord> {
    let mut out: Vec<HeadwordRecord> = Vec::new();

    for &annotated_id in &record.verb_types {
        let annotated_name = table.types()[annotated_id].name.clone();

        // ta-old iterates every type rather than only the annotated one, which
        // is what lets the v5 fallback fire and also what makes both entries of
        // the duplicate-named types (vk, vs, v5r-i, v5uru) reachable.
        for (candidate_id, candidate) in table.types().iter().enumerate() {
            let exact = candidate.name == annotated_name;
            let fallback = !exact
                && opts.v5_misannotation_fallback
                && v5_fallback_applies(&annotated_name, &candidate.name);
            if !exact && !fallback {
                continue;
            }

            // Find the remove-tense/form-0 conjugation whose suffix ends this
            // surface. That suffix is the dictionary-form ending.
            let stem = candidate
                .conjugations
                .iter()
                .filter(|c| c.tense == candidate.remove_tense && c.form.0 == 0)
                .find_map(|c| strip_suffix_unified(&record.surface, &c.suffix));

            let Some(surface) = stem else { continue };
            let verb_types = vec![candidate_id];

            // Deduplicate on (surface, type). ta-old's equivalent guard is dead
            // code; see the module docs and the spec's deviation table.
            if out.iter().any(|r| r.surface == surface && r.verb_types == verb_types) {
                continue;
            }

            if surface.is_empty() {
                stats.empty_stems += 1;
            }
            if fallback {
                stats.v5_fallback_stems += 1;
            } else {
                stats.exact_stems += 1;
            }

            out.push(HeadwordRecord {
                surface,
                flags: record.flags,
                verb_types,
                entry_id: record.entry_id,
            });
        }
    }

    out
}
```

- [ ] **Step 4: Register the module and run the tests**

Add `pub mod stem;` to `lib.rs`, then run:
`cd ta && cargo test -p jparser stem`
Expected: PASS, 11 tests.

- [ ] **Step 5: Commit**

```bash
cd ta && git add crates/jparser/src/stem.rs crates/jparser/src/lib.rs
git commit -m "feat: generate verb stems including the v5 fallback

Ports the stem-emitting block of ta-old's CreateDict. Stems are extra
index entries, not replacements: the matcher walks to a stem and
matches conjugation suffixes onward, so without these no verb
conjugates. Empty stems are legal and retained.

The v5 mis-annotation fallback is preserved because JMdict is the
source EDICT2 was generated from, so the format change does not fix
the annotations, and the failure without it is silent. It is gated
behind StemOptions and instrumented via StemStats so its cost and
benefit can be measured rather than assumed.

Unlike ta-old, whose dedupe guard is unreachable, duplicate
(stem, type) pairs are actually removed."
```

---

## Task 8: FST index build and load

**Files:**
- Create: `ta/crates/jparser/src/index/mod.rs`
- Create: `ta/crates/jparser/src/index/build.rs`
- Create: `ta/crates/jparser/src/index/load.rs`
- Create: `ta/crates/jparser/tests/index_roundtrip.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes: `jmdict::parse_entries` (Task 5), `record::{headwords, WordFlags}`
  (Task 6), `stem::{generate_stems, StemOptions, StemStats}` (Task 7),
  `conjugation::{ConjugationTable, VerbTypeId}` (Task 4), `kana::unify` and
  `kana::unify_str` (Task 1)
- Produces:
  - `pub const index::INDEX_FORMAT_VERSION: u32 = 1`
  - `pub const index::{HEADER_FILE, FST_FILE, RECORDS_FILE, ENTRIES_FILE, ENTRIES_INDEX_FILE}: &str`
  - `pub struct index::IndexHeader { pub version: u32, pub keys: u32, pub records: u32, pub entries: u32 }`
  - `pub struct index::StoredRecord { pub surface: String, pub flags: u16, pub verb_type: Option<VerbTypeId>, pub entry_id: u32 }`
  - `pub struct index::SenseData { pub pos: Vec<String>, pub glosses: Vec<String>, pub xrefs: Vec<String>, pub misc: Vec<String>, pub info: Vec<String> }`
  - `pub struct index::EntryData { pub id: u32, pub senses: Vec<SenseData> }`
  - `pub struct index::BuildReport { pub keys: usize, pub records: usize, pub entries: usize, pub skipped_entries: usize, pub stems: StemStats }`
  - `pub enum index::IndexError` with variants `Io`, `Fst`, `Encoding`, `Jmdict`, `VersionMismatch { found, expected }`
  - `pub fn index::build::build_from_reader<R: BufRead>(xml: R, table: &ConjugationTable, opts: &StemOptions, out_dir: &Path) -> Result<BuildReport, IndexError>`
  - `pub struct index::load::PrefixHit { pub key_chars: usize, pub records: Vec<StoredRecord> }`
  - `pub struct index::load::Index` with `open(&Path) -> Result<Self, IndexError>`,
    `header(&self) -> &IndexHeader`,
    `prefixes_of(&self, &str) -> Result<Vec<PrefixHit>, IndexError>`,
    `entry(&self, u32) -> Result<Option<EntryData>, IndexError>`

- [ ] **Step 1: Write the failing integration test**

Create `ta/crates/jparser/tests/index_roundtrip.rs`:

```rust
use std::path::PathBuf;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::index::{BuildReport, INDEX_FORMAT_VERSION};
use jparser::record::WordFlags;
use jparser::stem::StemOptions;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &PathBuf) -> BuildReport {
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        dir,
    )
    .expect("build must succeed")
}

#[test]
fn builds_and_reports_counts() {
    let report = build(&tmpdir("counts"));
    assert_eq!(report.entries, 3);
    assert_eq!(report.skipped_entries, 0);
    assert!(report.keys > 0);
    assert!(report.records >= report.keys);
    assert!(report.stems.exact_stems > 0);
}

#[test]
fn writes_all_expected_files() {
    let dir = tmpdir("files");
    build(&dir);
    for name in ["header.bin", "keys.fst", "records.bin", "entries.bin", "entries.idx"] {
        assert!(dir.join(name).exists(), "{name} must be written");
    }
}

#[test]
fn round_trips_an_exact_headword() {
    let dir = tmpdir("exact");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let found = index
        .prefixes_of("言う")
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .any(|r| r.surface == "言う");
    assert!(found, "the full headword must be retrievable");
}

#[test]
fn prefix_walk_returns_distinct_ascending_lengths() {
    let dir = tmpdir("prefixes");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    // 言う is a headword and 言 is its generated stem, so walking "言うから"
    // must surface hits at more than one length.
    let hits = index.prefixes_of("言うから").unwrap();
    let lengths: Vec<usize> = hits.iter().map(|h| h.key_chars).collect();
    assert!(lengths.len() >= 2, "got lengths {lengths:?}");
    assert!(lengths.windows(2).all(|w| w[0] < w[1]), "got {lengths:?}");
}

#[test]
fn matches_katakana_text_against_a_hiragana_headword() {
    let dir = tmpdir("kana");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    // The key is normalized, so katakana input finds the hiragana headword,
    // while the stored surface stays hiragana so inexactness stays detectable.
    let surfaces: Vec<String> = index
        .prefixes_of("イウ")
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .map(|r| r.surface.clone())
        .collect();
    assert!(surfaces.iter().any(|s| s == "いう"), "got {surfaces:?}");
}

#[test]
fn returns_no_hits_for_text_with_no_dictionary_prefix() {
    let dir = tmpdir("miss");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.prefixes_of("zzz").unwrap().is_empty());
}

#[test]
fn preserves_flags_through_the_round_trip() {
    let dir = tmpdir("flags");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let hits = index.prefixes_of("は").unwrap();
    let particle = hits
        .iter()
        .flat_map(|h| &h.records)
        .find(|r| r.surface == "は")
        .expect("は must be indexed");
    assert!(WordFlags(particle.flags).contains(WordFlags::PARTICLE));
}

#[test]
fn retrieves_entry_data_by_id() {
    let dir = tmpdir("entry");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let entry = index.entry(1000010).unwrap().expect("entry must exist");
    assert_eq!(entry.senses[0].glosses, vec!["to say", "to utter"]);
    assert_eq!(entry.senses[0].pos, vec!["v5r"]);
}

#[test]
fn returns_none_for_an_unknown_entry_id() {
    let dir = tmpdir("noentry");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.entry(999_999).unwrap().is_none());
}

#[test]
fn records_header_counts() {
    let dir = tmpdir("header");
    let report = build(&dir);
    let index = Index::open(&dir).unwrap();
    assert_eq!(index.header().version, INDEX_FORMAT_VERSION);
    assert_eq!(index.header().entries as usize, report.entries);
    assert_eq!(index.header().keys as usize, report.keys);
}

#[test]
fn rejects_a_header_with_the_wrong_format_version() {
    let dir = tmpdir("version");
    build(&dir);
    // Corrupt the version so load must refuse rather than misread the files.
    let path = dir.join("header.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] = bytes[0].wrapping_add(1);
    std::fs::write(&path, bytes).unwrap();
    let msg = Index::open(&dir).unwrap_err().to_string();
    assert!(msg.contains("version"), "got {msg}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ta && cargo test -p jparser --test index_roundtrip`
Expected: FAIL — `unresolved import 'jparser::index'`.

- [ ] **Step 3: Implement the shared index types**

Create `ta/crates/jparser/src/index/mod.rs` with a GPL v2 header, then:

```rust
//! Memory-mapped dictionary index.
//!
//! Replaces ta-old's hand-rolled sorted-array-plus-binary-search index
//! (`LoadDict`/`FindMatches`, `ta-old/exe/util/Dictionary.cpp`). Keys are
//! surfaces normalized with `kana::unify`, so the kana-insensitive comparator
//! ta-old threaded through three separate binary searches does not exist here.
//! The original surface is kept in the payload so inexact hiragana/katakana
//! matches can still be detected and penalized in Phase 1B.

pub mod build;
pub mod load;

use serde::{Deserialize, Serialize};

use crate::conjugation::VerbTypeId;
use crate::stem::StemStats;

/// Bumped whenever the on-disk layout changes. A mismatch forces a rebuild; the
/// loader must never try to read an index it does not recognize.
pub const INDEX_FORMAT_VERSION: u32 = 1;

pub const HEADER_FILE: &str = "header.bin";
pub const FST_FILE: &str = "keys.fst";
pub const RECORDS_FILE: &str = "records.bin";
pub const ENTRIES_FILE: &str = "entries.bin";
pub const ENTRIES_INDEX_FILE: &str = "entries.idx";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexHeader {
    pub version: u32,
    pub keys: u32,
    pub records: u32,
    pub entries: u32,
}

/// One headword or stem as stored in the payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredRecord {
    /// Original, unnormalized surface. Compared against the source text to set
    /// the inexact-match flag in Phase 1B.
    pub surface: String,
    pub flags: u16,
    /// `Some` for generated verb stems, `None` for plain headwords.
    pub verb_type: Option<VerbTypeId>,
    pub entry_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SenseData {
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
    pub xrefs: Vec<String>,
    pub misc: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryData {
    pub id: u32,
    pub senses: Vec<SenseData>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BuildReport {
    pub keys: usize,
    pub records: usize,
    pub entries: usize,
    /// Malformed JMdict entries skipped. Surfaced, never silently dropped.
    pub skipped_entries: usize,
    pub stems: StemStats,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("index fst failed: {0}")]
    Fst(#[from] fst::Error),
    #[error("index encoding failed: {0}")]
    Encoding(#[from] bincode::Error),
    #[error("reading JMdict failed: {0}")]
    Jmdict(#[from] crate::jmdict::JmdictError),
    #[error("index format version mismatch: found {found}, expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },
}
```

- [ ] **Step 4: Implement the builder**

Create `ta/crates/jparser/src/index/build.rs` with a GPL v2 header, then:

```rust
//! Build an index from JMdict XML.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufWriter, Write};
use std::path::Path;

use crate::conjugation::ConjugationTable;
use crate::index::{
    BuildReport, EntryData, IndexError, IndexHeader, SenseData, StoredRecord,
    ENTRIES_FILE, ENTRIES_INDEX_FILE, FST_FILE, HEADER_FILE, INDEX_FORMAT_VERSION,
    RECORDS_FILE,
};
use crate::jmdict::parse_entries;
use crate::kana::unify_str;
use crate::record::headwords;
use crate::stem::{generate_stems, StemOptions, StemStats};

pub fn build_from_reader<R: BufRead>(
    xml: R,
    table: &ConjugationTable,
    opts: &StemOptions,
    out_dir: &Path,
) -> Result<BuildReport, IndexError> {
    std::fs::create_dir_all(out_dir)?;

    // BTreeMap gives the lexicographic key order fst::MapBuilder requires.
    let mut by_key: BTreeMap<String, Vec<StoredRecord>> = BTreeMap::new();
    let mut entries: Vec<EntryData> = Vec::new();
    let mut stems = StemStats::default();
    let mut record_count = 0usize;

    let mut reader = parse_entries(xml);
    while let Some(result) = reader.next() {
        let raw = result?;
        entries.push(EntryData {
            id: raw.id,
            senses: raw
                .senses
                .iter()
                .map(|s| SenseData {
                    pos: s.pos.clone(),
                    glosses: s.glosses.clone(),
                    xrefs: s.xrefs.clone(),
                    misc: s.misc.clone(),
                    info: s.info.clone(),
                })
                .collect(),
        });

        for head in headwords(&raw, table) {
            let stem_records = generate_stems(&head, table, opts, &mut stems);

            if push(&mut by_key, &head.surface, StoredRecord {
                surface: head.surface.clone(),
                flags: head.flags.0,
                verb_type: None,
                entry_id: head.entry_id,
            }) {
                record_count += 1;
            }

            for stem in stem_records {
                if push(&mut by_key, &stem.surface, StoredRecord {
                    surface: stem.surface.clone(),
                    flags: stem.flags.0,
                    verb_type: stem.verb_types.first().copied(),
                    entry_id: stem.entry_id,
                }) {
                    record_count += 1;
                }
            }
        }
    }
    let skipped_entries = reader.skipped_count();

    // Payload: length-prefixed bincode blobs; the FST value is the offset.
    let mut records_blob: Vec<u8> = Vec::new();
    let mut fst_builder =
        fst::MapBuilder::new(BufWriter::new(File::create(out_dir.join(FST_FILE))?))?;
    for (key, records) in &by_key {
        let offset = records_blob.len() as u64;
        let encoded = bincode::serialize(records)?;
        records_blob.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        records_blob.extend_from_slice(&encoded);
        fst_builder.insert(key.as_bytes(), offset)?;
    }
    fst_builder.into_inner()?.flush()?;
    std::fs::write(out_dir.join(RECORDS_FILE), &records_blob)?;

    // Entry data plus a sorted (id, offset) table for binary search on load.
    let mut entries_blob: Vec<u8> = Vec::new();
    let mut entry_offsets: Vec<(u32, u64)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let offset = entries_blob.len() as u64;
        let encoded = bincode::serialize(entry)?;
        entries_blob.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        entries_blob.extend_from_slice(&encoded);
        entry_offsets.push((entry.id, offset));
    }
    entry_offsets.sort_unstable_by_key(|(id, _)| *id);
    std::fs::write(out_dir.join(ENTRIES_FILE), &entries_blob)?;
    std::fs::write(
        out_dir.join(ENTRIES_INDEX_FILE),
        bincode::serialize(&entry_offsets)?,
    )?;

    let header = IndexHeader {
        version: INDEX_FORMAT_VERSION,
        keys: by_key.len() as u32,
        records: record_count as u32,
        entries: entries.len() as u32,
    };
    std::fs::write(out_dir.join(HEADER_FILE), bincode::serialize(&header)?)?;

    Ok(BuildReport {
        keys: by_key.len(),
        records: record_count,
        entries: entries.len(),
        skipped_entries,
        stems,
    })
}

/// Insert under the normalized key. Returns false if an identical record was
/// already present, so counts do not double-report duplicates.
fn push(
    map: &mut BTreeMap<String, Vec<StoredRecord>>,
    surface: &str,
    record: StoredRecord,
) -> bool {
    let bucket = map.entry(unify_str(surface)).or_default();
    if bucket.contains(&record) {
        return false;
    }
    bucket.push(record);
    true
}
```

- [ ] **Step 5: Implement the loader**

Create `ta/crates/jparser/src/index/load.rs` with a GPL v2 header, then:

```rust
//! Open an index and walk it.

use std::fs::File;
use std::path::Path;

use fst::raw::Fst;
use memmap2::Mmap;

use crate::index::{
    EntryData, IndexError, IndexHeader, StoredRecord, ENTRIES_FILE,
    ENTRIES_INDEX_FILE, FST_FILE, HEADER_FILE, INDEX_FORMAT_VERSION, RECORDS_FILE,
};
use crate::kana::unify;

/// One dictionary key that is a prefix of the queried text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixHit {
    /// Length of the matched key **in characters** of the source text.
    pub key_chars: usize,
    pub records: Vec<StoredRecord>,
}

pub struct Index {
    fst: Fst<Mmap>,
    records: Mmap,
    entries: Mmap,
    entry_offsets: Vec<(u32, u64)>,
    header: IndexHeader,
}

fn map(path: &Path) -> Result<Mmap, IndexError> {
    let file = File::open(path)?;
    // Safety: index files are immutable for the process lifetime. The app
    // rebuilds to a temp directory and renames rather than mutating a live
    // index in place, so no writer can shrink these files underneath us.
    Ok(unsafe { Mmap::map(&file)? })
}

impl Index {
    pub fn open(dir: &Path) -> Result<Self, IndexError> {
        let header: IndexHeader =
            bincode::deserialize(&std::fs::read(dir.join(HEADER_FILE))?)?;
        if header.version != INDEX_FORMAT_VERSION {
            return Err(IndexError::VersionMismatch {
                found: header.version,
                expected: INDEX_FORMAT_VERSION,
            });
        }
        let entry_offsets: Vec<(u32, u64)> =
            bincode::deserialize(&std::fs::read(dir.join(ENTRIES_INDEX_FILE))?)?;
        Ok(Self {
            fst: Fst::new(map(&dir.join(FST_FILE))?)?,
            records: map(&dir.join(RECORDS_FILE))?,
            entries: map(&dir.join(ENTRIES_FILE))?,
            entry_offsets,
            header,
        })
    }

    pub fn header(&self) -> &IndexHeader {
        &self.header
    }

    /// Every dictionary key that is a prefix of `text`, shortest first.
    ///
    /// This single walk replaces ta-old's binary-search-per-length loop:
    /// stepping the transducer one character at a time, every node that is
    /// final marks a complete headword or stem.
    pub fn prefixes_of(&self, text: &str) -> Result<Vec<PrefixHit>, IndexError> {
        let mut node = self.fst.root();
        let mut output = 0u64;
        let mut hits = Vec::new();

        for (consumed, ch) in text.chars().enumerate() {
            let mut buf = [0u8; 4];
            for &byte in unify(ch).encode_utf8(&mut buf).as_bytes() {
                let Some(i) = node.find_input(byte) else {
                    return Ok(hits);
                };
                let transition = node.transition(i);
                output += transition.out.value();
                node = self.fst.node(transition.addr);
            }
            if node.is_final() {
                let offset = output + node.final_output().value();
                hits.push(PrefixHit {
                    key_chars: consumed + 1,
                    records: self.records_at(offset)?,
                });
            }
        }
        Ok(hits)
    }

    fn records_at(&self, offset: u64) -> Result<Vec<StoredRecord>, IndexError> {
        Ok(bincode::deserialize(slice_at(&self.records, offset)?)?)
    }

    pub fn entry(&self, id: u32) -> Result<Option<EntryData>, IndexError> {
        let Ok(i) = self.entry_offsets.binary_search_by_key(&id, |(k, _)| *k) else {
            return Ok(None);
        };
        let (_, offset) = self.entry_offsets[i];
        Ok(Some(bincode::deserialize(slice_at(&self.entries, offset)?)?))
    }
}

/// Read a `u32`-length-prefixed blob at `offset`.
fn slice_at(blob: &[u8], offset: u64) -> Result<&[u8], IndexError> {
    let start = usize::try_from(offset).map_err(|_| corrupt("offset out of range"))?;
    let len_end = start.checked_add(4).ok_or_else(|| corrupt("offset overflows"))?;
    let prefix = blob
        .get(start..len_end)
        .ok_or_else(|| corrupt("length prefix past end of payload"))?;
    let len = u32::from_le_bytes(prefix.try_into().expect("checked 4 bytes")) as usize;
    let end = len_end.checked_add(len).ok_or_else(|| corrupt("length overflows"))?;
    blob.get(len_end..end)
        .ok_or_else(|| corrupt("blob past end of payload"))
}

fn corrupt(reason: &str) -> IndexError {
    IndexError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("index payload corrupt: {reason}"),
    ))
}
```

- [ ] **Step 6: Register the module and run the tests**

Add `pub mod index;` to `lib.rs`, then run:
`cd ta && cargo test -p jparser --test index_roundtrip`
Expected: PASS, 11 tests.

- [ ] **Step 7: Run the whole suite**

Run: `cd ta && cargo test -p jparser`
Expected: PASS, all tests from Tasks 1–8.

- [ ] **Step 8: Commit**

```bash
cd ta && git add crates/jparser/src/index crates/jparser/src/lib.rs \
  crates/jparser/tests/index_roundtrip.rs
git commit -m "feat: add memory-mapped FST dictionary index

Replaces ta-old's sorted-array-plus-binary-search index. One
transducer walk per position finds every dictionary prefix, where
ta-old ran a binary search per candidate length with a custom
kana-insensitive comparator threaded through three call sites.

Keys are normalized with kana::unify; the original surface is kept in
the payload so inexact hiragana/katakana matches stay detectable and
can be penalized in Phase 1B.

The header carries a format version, and a mismatch refuses the load
rather than misreading the files."
```

---

## Task 9: CLI harness

**Files:**
- Create: `ta/crates/jparser/src/bin/jparser-cli.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–8
- Produces: a `jparser-cli` binary with `build-index`, `lookup`, and `romaji`
  subcommands. This is Phase 1A's user-facing deliverable and the tool Phase 1B's
  differential run extends.

- [ ] **Step 1: Write the CLI**

Create `ta/crates/jparser/src/bin/jparser-cli.rs` with a GPL v2 header, then:

```rust
//! Phase 1A verification harness. No UI, no Tauri, no network.

use std::io::BufReader;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::record::WordFlags;
use jparser::stem::StemOptions;

/// Flags rendered by `lookup`, paired with their display labels.
const FLAG_LABELS: &[(WordFlags, &str)] = &[
    (WordFlags::PRIMARY, "primary"),
    (WordFlags::PRONOUNCE, "reading"),
    (WordFlags::COMMON, "common"),
    (WordFlags::COMMON_LINE, "common-line"),
    (WordFlags::PARTICLE, "particle"),
    (WordFlags::COUNTER, "counter"),
];

#[derive(Parser)]
#[command(name = "jparser-cli", about = "JParser Phase 1A harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build an index from a JMdict XML file.
    BuildIndex {
        /// Path to JMdict_e.xml (uncompressed).
        xml: PathBuf,
        /// Directory to write the index into.
        out: PathBuf,
        /// Disable the v5 mis-annotation fallback, to measure its effect.
        #[arg(long)]
        no_v5_fallback: bool,
    },
    /// Print every dictionary record that is a prefix of TEXT.
    Lookup {
        /// Index directory.
        index: PathBuf,
        /// Text to walk.
        text: String,
    },
    /// Convert kana to romaji.
    Romaji {
        text: String,
        /// Apply the particle-only corrections (は to wa, へ to e).
        #[arg(long)]
        particle: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::BuildIndex { xml, out, no_v5_fallback } => {
            let table = ConjugationTable::load_embedded()?;
            let opts = StemOptions { v5_misannotation_fallback: !no_v5_fallback };
            let file = BufReader::new(std::fs::File::open(&xml)?);
            let report = build_from_reader(file, &table, &opts, &out)?;
            println!("entries:             {}", report.entries);
            println!("skipped entries:     {}", report.skipped_entries);
            println!("keys:                {}", report.keys);
            println!("records:             {}", report.records);
            println!("stems (exact):       {}", report.stems.exact_stems);
            println!("stems (v5 fallback): {}", report.stems.v5_fallback_stems);
            println!("stems (empty):       {}", report.stems.empty_stems);
            if report.skipped_entries > 0 {
                eprintln!(
                    "warning: {} malformed entries were skipped",
                    report.skipped_entries
                );
            }
        }
        Command::Lookup { index, text } => {
            let table = ConjugationTable::load_embedded()?;
            let index = Index::open(&index)?;
            for hit in index.prefixes_of(&text)? {
                let matched: String = text.chars().take(hit.key_chars).collect();
                println!("[{matched}] ({} chars)", hit.key_chars);
                for r in &hit.records {
                    let verb = match r.verb_type {
                        Some(vt) => table.types()[vt].name.as_str(),
                        None => "-",
                    };
                    let flags = WordFlags(r.flags);
                    let labels: Vec<&str> = FLAG_LABELS
                        .iter()
                        .filter(|(f, _)| flags.contains(*f))
                        .map(|(_, name)| *name)
                        .collect();
                    let glosses = index
                        .entry(r.entry_id)?
                        .and_then(|e| e.senses.first().map(|s| s.glosses.join("; ")))
                        .unwrap_or_default();
                    println!(
                        "    {:8} type={verb:8} [{}] {glosses}",
                        r.surface,
                        labels.join(",")
                    );
                }
            }
        }
        Command::Romaji { text, particle } => {
            let out = jparser::romaji::to_romaji(&text);
            let out = if particle {
                jparser::romaji::apply_particle_fixup(&out)
            } else {
                out
            };
            println!("{out}");
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify the CLI builds cleanly**

Run: `cd ta && cargo build -p jparser --bin jparser-cli`
Expected: compiles with no warnings.

- [ ] **Step 3: Verify romaji end to end**

Run: `cd ta && cargo run -q -p jparser --bin jparser-cli -- romaji "いっぱい"`
Expected: `ippai`

Run: `cd ta && cargo run -q -p jparser --bin jparser-cli -- romaji --particle "ha"`
Expected: `wa`

- [ ] **Step 4: Verify index build and lookup against the fixture**

Run:

```bash
cd ta && cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/jparser-cli-index
```

Expected: `entries: 3`, `skipped entries: 0`, non-zero keys, records, and exact
stems.

Run:

```bash
cd ta && cargo run -q -p jparser --bin jparser-cli -- \
  lookup /tmp/jparser-cli-index "言うから"
```

Expected: at least two bracketed prefix lines, one showing `言う` with
`to say; to utter`.

- [ ] **Step 5: Measure the v5 fallback cost**

```bash
cd ta && cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/jp-on
cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/jp-off --no-v5-fallback
```

Expected: the `--no-v5-fallback` run reports `stems (v5 fallback): 0` and fewer
total records. Record both numbers in the commit message.

This is the first half of the instrumentation the spec asked for. The second
half — how often a fallback stem *wins* a segmentation — needs the segmenter and
lands in Phase 1B.

- [ ] **Step 6: Check coverage**

Run: `cd ta && cargo llvm-cov -p jparser --summary-only`
Expected: ≥80% line coverage. If short, the gap is most likely
`index/load.rs`'s `slice_at` error paths — add tests for corrupt payload offsets
rather than lowering the target.

- [ ] **Step 7: Commit**

```bash
cd ta && git add crates/jparser/src/bin/jparser-cli.rs
git commit -m "feat: add Phase 1A CLI harness

build-index, lookup, and romaji make the parser foundations verifiable
with no UI and no network, and give Phase 1B's differential run
something to extend.

lookup prints every dictionary prefix of a string with flags, verb
type, and first-sense glosses, which is the direct analogue of what
ta-old's FindMatches produces before scoring.

build-index --no-v5-fallback reports stem counts with the
mis-annotation fallback disabled, so its cost can be measured instead
of assumed."
```

---

## Self-Review

**1. Spec coverage.** Phase 1A claims spec §11 Phase 1 minus matching and
segmentation:

| Spec requirement | Task |
|---|---|
| §4.1 asset conversion UTF-16LE → UTF-8 | 3 |
| §4.1 entry vs chain-only types; duplicate names retained | 4 |
| §4.1 fixed tense discriminants; dynamic interning | 4 |
| §4.2 `Next Type` resolution; hard failure on unresolved | 4 |
| §4.3 JMdict streaming; structured POS and priority | 5 |
| §5.2 headword records; every flag derivation | 6 |
| §5.2 stem generation; v5 fallback; empty stems; real dedupe | 7 |
| §5.2 FST keys/values/payload; version header; `unify` keys | 8 |
| §5.6 kana classification and katakana conversion | 1 |
| §5.6 romaji table, っ doubling, particle fixup | 2 |
| §9 malformed entries skipped and counted | 5, 8, 9 |
| §9 index version mismatch forces rebuild | 8 |
| §10 80% coverage on the crate | 9 |
| Refinement 2: v5 flag plus instrumentation | 7, 9 |

**Deferred to Phase 1B by design:** §5.1 `parse()`/`Segment`/`Entry`, §5.3
matcher, §5.4 segmenter and scoring constants, §5.5 conjugation label rendering,
§5.6 furigana assembly and the `kuruHack` reading reconstruction, §5.7
`BoundaryHints` **including refinement 1's stub** — it has no consumer until the
DP exists, so landing it here would be untested scaffolding — insta snapshots
over real sentences, and the differential run. The half-width katakana
limitation (§5.2) also belongs there, since offsets only matter once matching
begins.

**2. Placeholder scan.** No `TBD`, `TODO`, or "implement later"; every code step
carries runnable code. The one intentional blank is Task 5 Step 2's spike, which
writes its finding into the named `SPIKE RESULT` comment before Step 5 proceeds.
That is a measurement, not a placeholder: the answer depends on `quick-xml`'s
actual event model, and guessing it would produce a plan that silently indexes
gloss descriptions instead of POS codes.

**3. Type consistency.** Checked across task boundaries:

- `WordFlags` is `u16` (Task 6) and `StoredRecord.flags` is `u16` (Task 8); the
  CLI reconstructs with `WordFlags(r.flags)`. Consistent.
- `HeadwordRecord.verb_types` is `Vec<VerbTypeId>` — a POS code can name several
  types — while `StoredRecord.verb_type` is `Option<VerbTypeId>` — a stem carries
  exactly one. Task 8 maps between them via `stem.verb_types.first().copied()`,
  and Task 7's `tags_each_stem_with_exactly_the_type_that_produced_it` pins the
  invariant that makes that lossless.
- `VerbTypeId` is 0-based everywhere. ta-old stored `vt + 1` with 0 meaning "not
  a verb"; `Option` carries that instead, so no off-by-one crosses the boundary.
- `types_named` returns `Vec<VerbTypeId>` and every caller (Tasks 6, 7, tests)
  treats it as multi-valued.
- `StemStats` is defined in `stem.rs` and re-exported through
  `index::BuildReport`; `index/mod.rs` imports it from `crate::stem`. It derives
  `Copy` + `Default`, which `BuildReport`'s own `Copy` + `Default` require.
- `PrefixHit.key_chars` is a **character** count, matching the spec's rule that
  all offsets are char-based; Task 9 relies on it via
  `text.chars().take(hit.key_chars)`.
- `ConjugationError`, `JmdictError`, and `IndexError` are separate types;
  `IndexError` wraps `JmdictError` via `#[from]`. `ConjugationError` is not
  wrapped because the table is loaded by the caller before any build begins.

Three issues found and fixed during this review:

1. `counter_flag` was private but used by `record.rs`'s own tests — changed to
   `pub(crate)` and declared in the Interfaces block.
2. Task 2's Interfaces originally listed `kana::unify` as a dependency. It must
   **not** use it: `unify` also uppercases ASCII, which would corrupt
   romaji-table lookups. `romaji.rs` folds kana with its own local `fold`, and
   the Interfaces block now says so explicitly.
3. `to_katakana` originally returned `None` for katakana-only characters, but
   ta-old's bail applies to characters `>= 0x3097` *within the hiragana branch*,
   which cannot occur for real readings. Returning `None` there would have made
   `to_katakana("ヴ")` fail even though ta-old passes it through. Simplified to
   pass non-hiragana through, with the test updated to match.

---

## Execution Handoff

Plan complete and saved to
`docs/superpowers/plans/2026-08-12-jparser-phase1a-index.md`. Two execution
options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task,
review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans,
batch execution with checkpoints.

Which approach?
