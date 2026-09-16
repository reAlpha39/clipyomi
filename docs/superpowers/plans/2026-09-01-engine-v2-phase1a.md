# Engine v2 Phase 1a Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a second, independently written Japanese segmentation engine in `crates/jparser`, reachable behind `--engine v2`, that meets or beats the Phase 0 baseline on both span metrics and entry top-1, parses `くるまではこう` correctly, and eliminates the particle-absorbing merge class Phase 0 measured.

**Architecture:** The new engine lives in `crates/jparser/src/engine/` beside the existing one, which stays the default for all of Phase 1a. Deinflection moves from build time to parse time: a reverse trie over conjugation suffixes generated from IPADIC replaces both the build-time stem generator and the parse-time suffix recursion. Segmentation is a min-cost path whose cost accrues per character covered rather than per match, with tokenizer boundaries as hard interior constraints and a price on crossing them.

**Tech Stack:** Rust 2021 (`crates/jparser`), `fst` for the dictionary index, `bincode` + `memmap2` for the payload, `clap` for the CLI, `insta` for snapshots, Python 3 for the table generator (matching the existing `tools/*.py` pattern). No new Rust dependencies.

**Spec:** [`docs/superpowers/specs/2026-09-01-engine-v2-phase1a-design.md`](../specs/2026-09-01-engine-v2-phase1a-design.md)

**Master plan:** [`docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`](../specs/2026-08-27-engine-v2-master-plan.md)
**Phase 0 handoff:** [`docs/superpowers/2026-08-28-engine-v2-phase0-handoff.md`](../2026-08-28-engine-v2-phase0-handoff.md)

## Global Constraints

These apply to every task. Read them before Task 1 and again before any commit.

**Clean-room (master plan §3.5, spec §3).**

- **Do not read, fetch, or quote Translation Aggregator / ta-old source.** Do not let it into the workspace or into CI. `../ta-old` does not exist; verify with `ls ../ta-old` before starting and it must fail.
- **Do not open the bodies of these nine files during Phase 1a:** `segment.rs`, `matcher.rs`, `matcher/verb.rs`, `stem.rs`, `rank.rs`, `record.rs`, `kana.rs`, `romaji.rs`, `conjugation.rs`. Everything you need from them is quoted in this plan or in spec §3's read-list. If you believe you need something more, stop and ask rather than opening the file.
- **Do not open `crates/jparser/assets/conjugations.json`.** The new table comes from IPADIC (Task 4). Reading the old asset and retyping it produces a derivative regardless of output format.
- **Every cost constant carries a written justification** naming the Phase 0 measurement it derives from, in a doc comment beside it. A constant with no such comment does not ship. This is the operative form of §3.5's prohibition on arriving at ta-old's numbers "via the same numbers happened to win".
- Files under `src/engine/` are the new work; files outside it that this plan modifies are soft-tier (`index/*`, `jmdict.rs`, `lib.rs`, `hints.rs`, the CLI) and are edited, not rewritten.

**Metric reporting (master plan §7 Phase 0, constraint 1).**

- **Quote both span metrics, never the strict one alone.** Baseline: **0.6810 strict / 0.8465 tolerant** hints-off, **0.6963 / 0.8678** hints-on. Entry top-1 **0.9159** hints-off, **0.9155** hints-on. Coarsening **358 spans in 167 merges** hints-off, **371 in 173** hints-on.
- **The merge count is a ceiling, not a floor** (handoff §9.6): a change that lifts tolerant accuracy by inflating the merge count has probably found more spurious matches, not more compounds.
- Absolute values are not a quality verdict. Every corpus sentence was sampled because two segmenters disagreed on it.

**Scope.**

- **No switchover in Phase 1a.** v1 stays the default. The nine files, `assets/conjugations.json`, `xtask`'s converter, every `ta-old` citation and every `ported from Translation Aggregator` header survive Phase 1a and are deleted in Phase 1b. Do not delete or renumber them here.
- **No English conjugation labels in Phase 1a.** `Entry::conjugation` stays `None` on the v2 path. Phase 1b writes the formatter.
- No k-best (Phase 2), no extra token layers (Phase 3), no UI change.

**Standing gates**, run before every commit:

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo clippy -p jparser --all-targets -- -D warnings
cargo clippy -p jparser --all-targets --features mecab -- -D warnings
cargo check -p jparser --no-default-features
```

Baseline at `b9c35be`: **324** tests default, **357** with `--features mecab`, all clippy gates clean.

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/jparser/src/engine/mod.rs` | `parse_v2`, the `Boundaries` trait, `ConjStep`, module wiring | 1, 14 |
| `crates/jparser/src/engine/kana.rs` | Character folding for index keys and matching; script predicates | 2 |
| `crates/jparser/src/engine/romaji.rs` | Kana-to-romaji conversion | 3 |
| `crates/jparser/src/engine/table_data.rs` | **Generated.** IPADIC conjugation classes and forms | 4 |
| `crates/jparser/src/engine/table.rs` | Table types, JMdict-POS-to-class mapping, deinflection trie and chain search | 4, 5 |
| `crates/jparser/src/engine/flags.rs` | `RawEntry` to headword records; `WordFlags` derivation | 6 |
| `crates/jparser/src/engine/build.rs` | v2 index build (headwords only, v2 fold, v2 fingerprint) | 7 |
| `crates/jparser/src/engine/lex.rs` | Candidate generation: FST walk plus deinflection, per position | 8 |
| `crates/jparser/src/engine/cost.rs` | The cost terms and their justifications | 9 |
| `crates/jparser/src/engine/lattice.rs` | The min-cost path, k = 1, shaped so k follows | 10 |
| `crates/jparser/src/engine/chain.rs` | Span shape of an auxiliary chain; `ConjStep` population | 11 |
| `crates/jparser/src/engine/assemble.rs` | `Candidate` to `Entry`: headword, reading, pos union, ordering | 12, 13 |
| `crates/jparser/src/index/build.rs` | Extract `write_index`, shared by both builders | 7 |
| `crates/jparser/src/index/load.rs` | Additive `prefixes_of_with(text, fold)` | 8 |
| `crates/jparser/src/hints.rs` | Additive `VibratoTokenizer::boundaries` and `impl Boundaries` | 1 |
| `crates/jparser/src/lib.rs` | `pub mod engine`, `pub use engine::parse_v2`, `Entry::steps` | 1, 11 |
| `crates/jparser/src/bin/jparser-cli/main.rs` | `--engine` on `parse`, `build-index`, `score` | 1, 7 |
| `crates/jparser/src/bin/jparser-cli/score.rs` | Engine selection at the parse call site | 1 |
| `tools/gen_conjugation_table.py` | IPADIC CSV pivot to `table_data.rs` | 4 |
| `crates/jparser/tests/engine_v2.rs` | Integration tests for the v2 path | 14, 15 |
| `crates/jparser/tests/fixtures/spurious_merges.txt` | Handoff §9.5's defect class, as a regression fixture | 15 |
| `crates/jparser/tests/snapshots/engine_v2__sentences.snap` | v2's own snapshot over `parse_sentences.txt` | 14 |

---

## Task 1: Engine skeleton, the `Boundaries` trait, and the `--engine` selector

**Files:**
- Create: `crates/jparser/src/engine/mod.rs`
- Modify: `crates/jparser/src/lib.rs` (add `pub mod engine;`, add `Entry::steps`)
- Modify: `crates/jparser/src/hints.rs` (add `boundaries` and `impl Boundaries`)
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (add `--engine` to `parse` and `score`)
- Modify: `crates/jparser/src/bin/jparser-cli/score.rs:348` (engine selection at the parse call)
- Test: `crates/jparser/src/engine/mod.rs` (unit tests in-file)

**Interfaces:**
- Consumes: `jparser::{ParseResult, Segment, Entry, ParseOptions, ParseError}`, `jparser::index::load::Index`.
- Produces:
  - `pub trait Boundaries { fn is_boundary(&self, pos: usize) -> bool; }` — `pos` is a character offset; `is_boundary(p)` is true when a tokenizer token ends at `p` and another begins there. `0` and `text.chars().count()` are boundaries by definition.
  - `pub struct ConjStep { pub class: ClassId, pub form: FormId, pub range: std::ops::Range<usize> }`
  - `pub fn parse_v2(index: &Index, table: &Table, text: &str, opts: &ParseOptions, bounds: Option<&dyn Boundaries>) -> Result<ParseResult, ParseError>`
  - `Entry::steps: Vec<ConjStep>` — a new public field, `Vec::new()` on the v1 path.
  - `VibratoTokenizer::boundaries(&self, text: &str) -> BoundarySet`, `pub struct BoundarySet(Vec<bool>)` with `impl Boundaries for BoundarySet`.

Task 4 defines `ClassId` and `FormId`; until then they are `pub type ClassId = u16; pub type FormId = u16;` declared in `engine/mod.rs` and re-exported from `engine/table.rs` in Task 4.

- [ ] **Step 1: Verify the clean-room precondition**

Run:
```powershell
ls ../ta-old
```
Expected: the path does not exist. If it does, **stop** — master plan §3.5 forbids it being in or beside the workspace.

- [ ] **Step 2: Write the failing tests**

Create `crates/jparser/src/engine/mod.rs` with only the tests and the module doc:

```rust
// ClipYomi — Engine v2, Phase 1a.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b, when the crate stops being GPL-2.0-only.

//! The Engine v2 segmentation core.
//!
//! Reachable only through `parse_v2` and `jparser-cli --engine v2` for the
//! whole of Phase 1a; `parse` remains the default path. The switchover is
//! Phase 1b's, so that no build ships without a conjugation label.

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Boundaries` that says every position is a boundary — the shape a
    /// character-level tokenizer would produce.
    struct EveryPosition;
    impl Boundaries for EveryPosition {
        fn is_boundary(&self, _pos: usize) -> bool {
            true
        }
    }

    #[test]
    fn the_stub_covers_the_input_exactly_once() {
        let r = stub_result("東京都に住む");
        assert_eq!(r.segments.len(), 1);
        assert_eq!(r.segments[0].start, 0);
        assert_eq!(r.segments[0].len, 6, "length is in characters, not bytes");
        assert_eq!(r.segments[0].surface, "東京都に住む");
        assert!(!r.segments[0].matched);
        assert!(r.segments[0].entries.is_empty());
    }

    #[test]
    fn empty_input_produces_no_segments() {
        assert!(stub_result("").segments.is_empty());
    }

    #[test]
    fn a_boundaries_impl_is_object_safe() {
        let b: &dyn Boundaries = &EveryPosition;
        assert!(b.is_boundary(0));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::`
Expected: FAIL — `cannot find function stub_result`, `cannot find trait Boundaries`.

- [ ] **Step 4: Write the minimal implementation**

Add above the test module in `crates/jparser/src/engine/mod.rs`:

```rust
use crate::index::load::Index;
use crate::{ParseError, ParseOptions, ParseResult, Segment};

pub type ClassId = u16;
pub type FormId = u16;

/// Where a tokenizer says one token ends and the next begins.
///
/// A character position `p` in `0..=len`. `is_boundary(0)` and
/// `is_boundary(len)` are true by definition, because every tokenizer agrees
/// the text begins and ends where it does.
///
/// This replaces the advisory `bad_start`/`bad_end` pair (master plan §3.2's
/// note on `flags_from_worker`): the engine asks where the tokenizer put its
/// cuts, not whether a position is interior to a token.
pub trait Boundaries {
    fn is_boundary(&self, pos: usize) -> bool;
}

/// One step of a deinflection chain: which conjugation class was applied, in
/// which form, over which characters of the span.
///
/// Filled by Phase 1a (`chain.rs`); named in English by Phase 1b. Phase 1a
/// leaves `Entry::conjugation` as `None`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConjStep {
    pub class: ClassId,
    pub form: FormId,
    pub range: std::ops::Range<usize>,
}

/// One unmatched segment covering the whole input. Replaced in Task 14.
fn stub_result(text: &str) -> ParseResult {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return ParseResult { segments: Vec::new() };
    }
    ParseResult {
        segments: vec![Segment {
            start: 0,
            len: chars.len(),
            surface: text.to_string(),
            reading: None,
            matched: false,
            entries: Vec::new(),
        }],
    }
}

/// Parse `text` with the Engine v2 core.
///
/// Signature-compatible with [`crate::parse`] apart from the boundary
/// parameter, which is a constraint source rather than an advisory hint.
pub fn parse_v2(
    index: &Index,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError> {
    let _ = (index, opts, bounds);
    Ok(stub_result(text))
}
```

`parse_v2` gains its `table: &Table` parameter in Task 5, once the type exists.

In `crates/jparser/src/lib.rs`, beside the existing `mod` declarations, add:

```rust
pub mod engine;
```

and add the new field to `Entry`, with its doc comment:

```rust
    /// The deinflection chain that produced `headword` from the matched
    /// surface, innermost step first. Empty for an uninflected match and on
    /// the v1 path, which does not compute it.
    ///
    /// Phase 1b's formatter turns this into `conjugation`'s English label; see
    /// docs/superpowers/specs/2026-09-01-engine-v2-phase1a-design.md §8.
    pub steps: Vec<crate::engine::ConjStep>,
```

Every existing `Entry { .. }` construction must now set `steps: Vec::new()`. Find them with:

```powershell
Select-String -Path crates/jparser/src,crates/jparser/tests,src-tauri/src -Pattern "Entry \{" -Recurse
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::`
Expected: PASS, 3 tests.

- [ ] **Step 6: Add the boundary source behind the mecab feature**

In `crates/jparser/src/hints.rs`, add — do not modify `BoundaryFlags` or `flags_from_worker`, which stay on the v1 path:

```rust
/// Token boundaries as the Engine v2 DP wants them: `set[p]` is true when a
/// token ends at character `p` and the next begins there.
///
/// `BoundaryFlags` answers "is this position interior to a token", which loses
/// single-character tokens entirely — master plan §4.3 cause 1, and the reason
/// `くるまではこう` merged `は`+`こう` for free. This answers the question the
/// constraint actually asks.
pub struct BoundarySet(Vec<bool>);

impl crate::engine::Boundaries for BoundarySet {
    fn is_boundary(&self, pos: usize) -> bool {
        self.0.get(pos).copied().unwrap_or(false)
    }
}

impl VibratoTokenizer {
    /// Character-offset boundary set for `text`.
    pub fn boundaries(&self, text: &str) -> BoundarySet {
        let char_len = text.chars().count();
        let mut set = vec![false; char_len + 1];
        set[0] = true;
        set[char_len] = true;

        let mut worker = self.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        for i in 0..worker.num_tokens() {
            let token = worker.token(i);
            let byte_range = token.range_byte();
            let start = text[..byte_range.start].chars().count();
            let end = text[..byte_range.end].chars().count();
            set[start] = true;
            set[end] = true;
        }
        BoundarySet(set)
    }
}
```

Read the existing `VibratoTokenizer::hints` for how a worker is constructed and how a token's range is read; copy that mechanism rather than inventing one. `hints.rs` is soft tier and may be read.

- [ ] **Step 7: Test the boundary source**

Add to `crates/jparser/src/hints.rs`'s test module, following the pattern of the existing tests there (which build a test dictionary through `write_test_dictionary`):

```rust
    #[test]
    fn a_single_character_token_still_yields_both_its_boundaries() {
        // The failure master plan §4.3 names: `bad_start`/`bad_end` mark
        // interiors, so a one-character token marks nothing at all.
        let t = tokenizer_over(&["東京", "都"]);
        let b = t.boundaries("東京都");
        use crate::engine::Boundaries;
        assert!(b.is_boundary(0));
        assert!(b.is_boundary(2), "東京 ends and 都 begins at 2");
        assert!(b.is_boundary(3));
        assert!(!b.is_boundary(1), "1 is interior to 東京");
    }
```

Write `tokenizer_over` as a helper following the existing tests' dictionary-construction pattern, or reuse whatever helper they already have.

Run: `cargo test -p jparser --features mecab hints::`
Expected: PASS.

- [ ] **Step 8: Add the `--engine` selector**

In `crates/jparser/src/bin/jparser-cli/main.rs`, above `enum Command`:

```rust
/// Which segmentation engine a subcommand runs.
///
/// v1 is the default for all of Phase 1a: the switchover is Phase 1b's, so no
/// build ships with `conjugation: None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
enum EngineChoice {
    V1,
    V2,
}
```

Add to the `Parse` and `Score` variants:

```rust
        /// Segmentation engine. `v2` is the Phase 1a clean-room core.
        #[arg(long, value_enum, default_value_t = EngineChoice::V1)]
        engine: EngineChoice,
```

In `score.rs`, `run` gains an `engine: EngineChoice` parameter (import the enum from `crate::EngineChoice`) and the call at `score.rs:348` becomes a match:

```rust
        let parsed = match engine {
            EngineChoice::V1 => jparser::parse(index, table, &record.text, &opts, hints)?,
            EngineChoice::V2 => jparser::engine::parse_v2(index, &record.text, &opts, bounds)?,
        };
```

`bounds` is `None` for now; Task 16 wires the real boundary source. Thread the parameter from `main.rs`'s match arm.

- [ ] **Step 9: Run the standing gates**

Run all five commands from Global Constraints.
Expected: all pass; test counts 327 default / 361 mecab (the three new engine tests, plus one hints test).

- [ ] **Step 10: Commit**

```powershell
git add crates/jparser/src/engine crates/jparser/src/lib.rs crates/jparser/src/hints.rs crates/jparser/src/bin/jparser-cli
git commit -m "feat(engine-v2): add the Phase 1a skeleton and the --engine selector

The new core lands beside the old one; v1 stays the default for all of
1a, so no build ships without a conjugation label. Boundaries replaces
the bad_start/bad_end pair with the question the DP actually asks --
where the tokenizer cut -- which a one-character token answers and the
interior flags do not."
```

---

## Task 2: `engine/kana.rs` — character folding

**Files:**
- Create: `crates/jparser/src/engine/kana.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod kana;`)
- Test: in-file

**Interfaces:**
- Produces:
  - `pub fn fold(c: char) -> char` — the index-key normalization. Must be `char -> char` so the FST walk can apply it per transition (spec §5).
  - `pub fn fold_str(s: &str) -> String`
  - `pub fn is_kanji(c: char) -> bool`, `pub fn is_kana(c: char) -> bool`, `pub fn is_digit(c: char) -> bool`, `pub fn is_japanese(c: char) -> bool` — used by `cost.rs` (Task 9) for the script-transition rule and by `lex.rs` (Task 8).

Write this from a Unicode chart. Do not open `src/kana.rs`.

Ranges you will need, from the Unicode Standard:

| Block | Range |
| --- | --- |
| Hiragana | U+3041–U+3096 |
| Katakana | U+30A1–U+30FA, plus U+30FC ー |
| Halfwidth katakana | U+FF66–U+FF9D |
| CJK Unified Ideographs | U+4E00–U+9FFF |
| CJK Extension A | U+3400–U+4DBF |
| Fullwidth digits | U+FF10–U+FF19 |

Hiragana to katakana is `+0x60`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiragana_and_katakana_fold_together() {
        assert_eq!(fold('か'), fold('カ'));
        assert_eq!(fold('ゃ'), fold('ャ'));
        assert_eq!(fold('ん'), fold('ン'));
    }

    #[test]
    fn halfwidth_katakana_folds_with_the_others() {
        assert_eq!(fold('ｶ'), fold('か'));
    }

    #[test]
    fn voicing_is_preserved() {
        // が and か are different words. Folding them together would make
        // every dakuten-dropped OCR artefact match its clean form, which is
        // a tier D concern the corpus deliberately carries.
        assert_ne!(fold('が'), fold('か'));
    }

    #[test]
    fn kanji_and_ascii_fold_to_themselves() {
        assert_eq!(fold('東'), '東');
        assert_eq!(fold('a'), 'a');
        assert_eq!(fold('、'), '、');
    }

    #[test]
    fn fullwidth_digits_fold_to_ascii() {
        assert_eq!(fold('２'), '2');
    }

    #[test]
    fn fold_str_folds_every_character() {
        assert_eq!(fold_str("とうきょう"), fold_str("トウキョウ"));
    }

    #[test]
    fn script_predicates_agree_with_the_chart() {
        assert!(is_kanji('東') && !is_kanji('と'));
        assert!(is_kana('と') && is_kana('ト') && !is_kana('東'));
        assert!(is_digit('7') && is_digit('７') && !is_digit('七'));
        assert!(is_japanese('東') && is_japanese('と') && !is_japanese('a'));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::kana`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

Write `fold` as a `match` over the ranges in the table above: hiragana to katakana by `+0x60`, halfwidth katakana to its fullwidth equivalent then to the fold target, fullwidth digits to ASCII, everything else unchanged. Halfwidth katakana is not a contiguous offset from fullwidth — `ｦ` U+FF66 maps to `ヲ` U+30F2 and the rest follow U+FF71 `ｱ` → U+30A2 `ア` in a different order — so write it as an explicit lookup array of the 58 halfwidth characters in code-point order paired with their fullwidth targets. Halfwidth dakuten (`ｶﾞ`) is two characters and folds as two characters; do not attempt to compose it here.

Use the same file header as Task 1's `engine/mod.rs`, with a one-line module doc naming what it was written from: `//! Written from the Unicode Standard's Hiragana, Katakana and Halfwidth Forms blocks.`

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::kana`
Expected: PASS, 7 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): fold kana from the Unicode chart

Voicing is preserved deliberately: folding dakuten away would make
every tier D OCR artefact match its clean form."
```

---

## Task 3: `engine/romaji.rs` — kana to romaji

**Files:**
- Create: `crates/jparser/src/engine/romaji.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod romaji;`)
- Test: in-file

**Interfaces:**
- Consumes: `engine::kana::fold`.
- Produces: `pub fn to_romaji(s: &str) -> String`.

Romaji is display-only: it reaches neither span accuracy nor entry top-1, and it is used by one CLI subcommand and one furigana mode. These unit tests are its only cover, so write them from a Hepburn chart before the table.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_syllables() {
        assert_eq!(to_romaji("さくら"), "sakura");
        assert_eq!(to_romaji("トウキョウ"), "toukyou");
    }

    #[test]
    fn digraphs_beat_their_first_character() {
        // The reason the table is ordered: キャ is "kya", never "kiya".
        assert_eq!(to_romaji("キャ"), "kya");
        assert_eq!(to_romaji("しゅ"), "shu");
        assert_eq!(to_romaji("ちょ"), "cho");
    }

    #[test]
    fn sokuon_doubles_the_following_consonant() {
        assert_eq!(to_romaji("がっこう"), "gakkou");
        assert_eq!(to_romaji("いっしょ"), "issho");
    }

    #[test]
    fn n_before_a_vowel_or_y_takes_an_apostrophe() {
        assert_eq!(to_romaji("しんいち"), "shin'ichi");
        assert_eq!(to_romaji("かんよう"), "kan'you");
        assert_eq!(to_romaji("かんじ"), "kanji", "no apostrophe before a consonant");
    }

    #[test]
    fn the_long_vowel_mark_repeats_the_preceding_vowel() {
        assert_eq!(to_romaji("ラーメン"), "raamen");
    }

    #[test]
    fn non_kana_passes_through() {
        assert_eq!(to_romaji("東京 tokyo!"), "東京 tokyo!");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::romaji`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

Fold each character to katakana with `kana::fold`, then walk the string trying a two-character digraph before a one-character syllable, so the digraph wins. Structure:

```rust
/// Katakana to Hepburn. Two-character entries come first and the first match
/// wins; that ordering is what makes キャ produce "kya" rather than "kiya",
/// so do not sort this table.
///
/// Written from a Hepburn romanization chart.
const TABLE: &[(&str, &str)] = &[
    ("キャ", "kya"), ("キュ", "kyu"), ("キョ", "kyo"),
    ("シャ", "sha"), ("シュ", "shu"), ("ショ", "sho"),
    // .. every digraph, then ..
    ("ア", "a"), ("イ", "i"), ("ウ", "u"), ("エ", "e"), ("オ", "o"),
    ("カ", "ka"), // ..
];
```

Handle the three specials outside the table: `ッ` doubles the next output's first consonant; `ー` repeats the previous output's last vowel; `ン` emits `n` plus an apostrophe when the next output starts with a vowel or `y`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::romaji`
Expected: PASS, 6 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): romanize kana from a Hepburn chart

Display-only, so neither metric covers it -- these unit tests are the
whole gate."
```

---

## Task 4: The IPADIC conjugation table

**Files:**
- Create: `tools/gen_conjugation_table.py`
- Create: `crates/jparser/src/engine/table_data.rs` (generated)
- Create: `crates/jparser/src/engine/table.rs`
- Create: `crates/jparser/assets/IPADIC_NOTICE` (the notice IPADIC's licence requires be retained)
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod table;`)
- Test: `crates/jparser/src/engine/table.rs` (in-file)

**Interfaces:**
- Produces:
  - `pub struct Class { pub name: &'static str, pub remove: &'static str, pub is_adjective: bool, pub forms: &'static [Form] }`
  - `pub struct Form { pub name: &'static str, pub suffix: &'static str }`
  - `pub type ClassId = u16` and `pub type FormId = u16`, re-exported from `engine::mod`. `ClassId` indexes `CLASSES`; `FormId` indexes a `Class`'s `forms`.
  - `pub static CLASSES: &[Class]` in `table_data.rs`.
  - `pub struct Table` — the runtime view, built once from `CLASSES` with the suffix trie of Task 5. `pub fn Table::new() -> Table`, infallible: the data is compiled in, so there is no parse and no error path.
  - `pub fn class_for_pos(pos: &str) -> Option<ClassId>` — JMdict POS code to conjugation class.

**The source.** IPADIC's CSV lexicon, `mecab-ipadic/*.csv` in `taku910/mecab`, EUC-JP encoded, 13 columns:

```
surface, left_id, right_id, cost, 品詞, 品詞細分類1, 品詞細分類2, 品詞細分類3, 活用型, 活用形, 原形, 読み, 発音
   0        1         2       3     4        5             6             7        8       9      10    11    12
```

**The pivot.** Group rows by (活用型, 原形). Within a group the stem is the longest common prefix of every surface. The class's remove-suffix is `基本形_surface[stem_len..]`; each row contributes the form `(活用形, surface[stem_len..])`. Every lemma in a class must produce the same form-to-suffix map — that assertion is the generator's own check that the pivot is correct, and a disagreement is a finding to report, not to silence.

Worked example, `一段` / `食べる`: surfaces 食べる, 食べ, 食べれ, 食べろ, 食べよ; LCP `食べ`; remove `る`; forms 基本形→`る`, 未然形→``, 連用形→``, 仮定形→`れ`, 命令ｒｏ→`ろ`. `サ変・スル` / `する` has LCP `""`, remove `する`, and every form's suffix is the whole surface — which is correct and is why parse-time deinflection does not need the empty-stem special case the v1 index has.

- [ ] **Step 1: Obtain the IPADIC CSVs, out of tree**

The archive must not enter the repository. Download `mecab-ipadic/` from `https://github.com/taku910/mecab` into the scratchpad directory, then record the commit SHA you fetched and the sha256 of each CSV in `tools/gen_conjugation_table.py`'s header. Verify `mecab-ipadic/COPYING` is present — its retention is the whole licence condition (master plan §3.3).

Run:
```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\ipadic"
New-Item -ItemType Directory -Force $scratch
# fetch mecab-ipadic/*.csv, char.def, and COPYING into $scratch
Get-ChildItem $scratch -Filter *.csv | Measure-Object
```
Expected: a non-zero count of CSV files, `COPYING` present.

Copy `COPYING`'s text into `crates/jparser/assets/IPADIC_NOTICE` and reference it from the generated file's header. Committing the notice is what makes committing the derived table licit.

- [ ] **Step 2: Write the generator's consistency check as its own failing assertion**

Create `tools/gen_conjugation_table.py`. Write the pivot and the assertion first, and run it before writing any emitter:

```python
#!/usr/bin/env python3
"""Generate crates/jparser/src/engine/table_data.rs from IPADIC's CSV lexicon.

Source: mecab-ipadic/*.csv from https://github.com/taku910/mecab at <SHA>,
EUC-JP, 13 columns. IPADIC's COPYING permits reproduction and distribution
with notice retention; the notice is at crates/jparser/assets/IPADIC_NOTICE.

Nothing here is derived from Translation Aggregator. See master plan §3.5.

Usage: python tools/gen_conjugation_table.py <ipadic-dir> <out.rs>
"""
import collections, glob, os, sys

SURFACE, CLASS, FORM, LEMMA = 0, 8, 9, 10
BASE_FORM = "基本形"


def read_rows(ipadic_dir):
    for path in sorted(glob.glob(os.path.join(ipadic_dir, "*.csv"))):
        with open(path, encoding="euc_jp") as f:
            for line in f:
                cols = line.rstrip("\n").split(",")
                if len(cols) < 13 or cols[CLASS] == "*":
                    continue
                yield cols


def pivot(rows):
    """(class, lemma) -> {form: suffix}, plus the class's remove-suffix."""
    groups = collections.defaultdict(list)
    for c in rows:
        groups[(c[CLASS], c[LEMMA])].append(c)

    per_class = collections.defaultdict(dict)   # class -> {form: suffix}
    removes = {}                                # class -> remove-suffix
    conflicts = []
    for (cls, _lemma), rows in sorted(groups.items()):
        surfaces = [r[SURFACE] for r in rows]
        stem = os.path.commonprefix(surfaces)
        base = next((r[SURFACE] for r in rows if r[FORM] == BASE_FORM), None)
        if base is None:
            continue                            # no 基本形: not a full paradigm
        remove = base[len(stem):]
        if removes.setdefault(cls, remove) != remove:
            conflicts.append((cls, "remove", removes[cls], remove))
        for r in rows:
            suffix = r[SURFACE][len(stem):]
            seen = per_class[cls].setdefault(r[FORM], suffix)
            if seen != suffix:
                conflicts.append((cls, r[FORM], seen, suffix))
    return per_class, removes, conflicts
```

- [ ] **Step 3: Run the consistency check**

Run:
```powershell
python -c "import sys; sys.argv=['x','$scratch']; exec(open('tools/gen_conjugation_table.py').read().split('def emit')[0]); p,r,c = pivot(read_rows(sys.argv[1])); print(len(p),'classes'); print(len(c),'conflicts'); [print(x) for x in c[:20]]"
```
Expected: tens of classes, and a conflict list. **Read every conflict.** Some are real irregularities IPADIC folds into one class name; each one needs a decision recorded in the generator, not a silent `continue`. Adding the lemma to a per-class exception list with a comment naming the verb is acceptable; suppressing the whole check is not.

- [ ] **Step 4: Write the emitter and generate the table**

Append to the generator:

```python
def emit(per_class, removes, out_path):
    names = sorted(per_class)
    with open(out_path, "w", encoding="utf-8", newline="\n") as f:
        f.write(HEADER)                          # the file header of Task 1, plus the IPADIC provenance
        f.write("use super::table::{Class, Form};\n\n")
        f.write("pub static CLASSES: &[Class] = &[\n")
        for cls in names:
            forms = sorted(per_class[cls].items())
            f.write(f'    Class {{ name: "{cls}", remove: "{removes[cls]}", '
                    f'is_adjective: {"true" if cls.startswith("形容詞") else "false"}, forms: &[\n')
            for form, suffix in forms:
                f.write(f'        Form {{ name: "{form}", suffix: "{suffix}" }},\n')
            f.write("    ] },\n")
        f.write("];\n")
```

Run:
```powershell
python tools/gen_conjugation_table.py $scratch crates/jparser/src/engine/table_data.rs
(Get-Content crates/jparser/src/engine/table_data.rs | Measure-Object -Line).Lines
```
Expected: a few thousand lines. Record the class count and the total form count in the commit message — master plan §7.2's planning figure is 32 types and 734 conjugations, so a result within an order of magnitude confirms the pivot; a wildly different one means read it again.

- [ ] **Step 5: Write the failing Rust tests**

In `crates/jparser/src/engine/table.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn class(name: &str) -> &'static Class {
        CLASSES.iter().find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no class {name}; the generator's class list changed"))
    }

    fn suffix(class_name: &str, form_name: &str) -> &'static str {
        class(class_name).forms.iter().find(|f| f.name == form_name)
            .unwrap_or_else(|| panic!("{class_name} has no form {form_name}"))
            .suffix
    }

    #[test]
    fn ichidan_removes_ru_and_has_an_empty_stem_form() {
        assert_eq!(class("一段").remove, "る");
        assert_eq!(suffix("一段", "基本形"), "る");
        assert_eq!(suffix("一段", "連用形"), "", "食べ is the bare stem");
    }

    #[test]
    fn godan_ka_i_onbin_keeps_its_irregular_ta_form() {
        // 書く: 連用形 is 書き but 連用タ接続 is 書い. That split is why IPADIC
        // has a separate class for it, and it is the check that the pivot
        // reads 活用形 rather than guessing.
        assert_eq!(class("五段・カ行イ音便").remove, "く");
        assert_eq!(suffix("五段・カ行イ音便", "連用形"), "き");
        assert_eq!(suffix("五段・カ行イ音便", "連用タ接続"), "い");
    }

    #[test]
    fn suru_has_an_empty_stem_so_every_suffix_is_the_whole_surface() {
        assert_eq!(class("サ変・スル").remove, "する");
        assert_eq!(suffix("サ変・スル", "未然形"), "し");
    }

    #[test]
    fn the_adjective_class_is_marked() {
        assert!(class("形容詞・アウオ段").is_adjective);
        assert!(!class("一段").is_adjective);
    }

    #[test]
    fn the_auxiliaries_the_label_chain_needs_are_present() {
        // Master plan §7.3's example chain: させ|られ|ませ|ん|でし|た.
        for name in ["特殊・タ", "特殊・マス", "特殊・ヌ", "特殊・デス", "特殊・ナイ"] {
            assert!(!class(name).forms.is_empty(), "{name} has no forms");
        }
    }

    #[test]
    fn every_jmdict_verb_pos_in_the_fixture_maps_to_a_real_class() {
        for pos in JMDICT_POS_CLASS.iter().map(|(p, _)| *p) {
            assert!(class_for_pos(pos).is_some(), "{pos} maps to nothing");
        }
        for (pos, name) in JMDICT_POS_CLASS {
            assert!(
                CLASSES.iter().any(|c| c.name == *name),
                "{pos} maps to {name}, which the generated table does not have"
            );
        }
    }
}
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::table`
Expected: FAIL — `table.rs` has no types yet.

- [ ] **Step 7: Write `table.rs`'s types and the POS mapping**

```rust
pub use crate::engine::{ClassId, FormId};
pub use crate::engine::table_data::CLASSES;

/// One IPADIC conjugation class (活用型).
pub struct Class {
    pub name: &'static str,
    /// What the base form adds to the stem: `る` for 一段, `する` for サ変・スル.
    pub remove: &'static str,
    pub is_adjective: bool,
    pub forms: &'static [Form],
}

/// One conjugation form (活用形) of a class, and the suffix it puts on the stem.
pub struct Form {
    pub name: &'static str,
    pub suffix: &'static str,
}

/// JMdict POS code to IPADIC conjugation class.
///
/// Hand-written: JMdict's POS vocabulary and IPADIC's 活用型 vocabulary have
/// no defined relationship (master plan §4.3), so this is the mapping that
/// connects them. Every row is checked by the tests above — the code must map
/// to a class the generated table actually carries.
pub const JMDICT_POS_CLASS: &[(&str, &str)] = &[
    ("v1", "一段"),
    ("v5k", "五段・カ行イ音便"),
    ("v5k-s", "五段・カ行促音便"),
    ("v5g", "五段・ガ行"),
    ("v5s", "五段・サ行"),
    ("v5t", "五段・タ行"),
    ("v5n", "五段・ナ行"),
    ("v5b", "五段・バ行"),
    ("v5m", "五段・マ行"),
    ("v5r", "五段・ラ行"),
    ("v5u", "五段・ワ行促音便"),
    ("vk", "カ変・クル"),
    ("vs-i", "サ変・スル"),
    ("adj-i", "形容詞・アウオ段"),
    // Complete this list from the generator's printed class list against the
    // JMdict POS codes present in tests/fixtures/jmdict_subset.xml. Every row
    // added must survive `every_jmdict_verb_pos_in_the_fixture_maps_to_a_real_class`.
];

pub fn class_for_pos(pos: &str) -> Option<ClassId> {
    let name = JMDICT_POS_CLASS.iter().find(|(p, _)| *p == pos)?.1;
    CLASSES.iter().position(|c| c.name == name).map(|i| i as ClassId)
}
```

Complete `JMDICT_POS_CLASS` by listing the classes the generator produced and the POS codes the fixture uses:

```powershell
Select-String -Path crates/jparser/tests/fixtures/jmdict_subset.xml -Pattern "&v[0-9a-z-]+;|&adj-i" -AllMatches |
  ForEach-Object { $_.Matches.Value } | Sort-Object -Unique
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::table`
Expected: PASS, 6 tests.

- [ ] **Step 9: Run the standing gates and commit**

```powershell
git add tools/gen_conjugation_table.py crates/jparser/src/engine crates/jparser/assets/IPADIC_NOTICE
git commit -m "feat(engine-v2): generate the conjugation table from IPADIC

<N> classes, <M> forms, pivoted from mecab-ipadic's CSV lexicon on
(活用型, 活用形) with the stem taken as each lemma group's longest
common prefix. The generator asserts every lemma in a class yields the
same form-to-suffix map; the <K> conflicts it found are recorded as
named exceptions rather than suppressed.

assets/conjugations.json was not opened. IPADIC's notice is retained at
assets/IPADIC_NOTICE, which is that licence's whole condition."
```

---

## Task 5: Deinflection — the reverse suffix trie and chain search

**Files:**
- Modify: `crates/jparser/src/engine/table.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (`parse_v2` gains `table: &Table`)
- Test: in-file

**Interfaces:**
- Consumes: `Class`, `Form`, `CLASSES`, `ClassId`, `FormId`, `ConjStep`.
- Produces:
  - `pub struct Table { /* the trie */ }`, `pub fn Table::new() -> Table` (infallible).
  - `pub struct ChainHit { pub consumed: usize, pub steps: Vec<ConjStep>, pub stem_class: ClassId }` — `consumed` characters were stripped from the tail of the span; `stem_class` is the class the remaining stem must belong to, and the dictionary key is `stem + CLASSES[stem_class].remove`.
  - `pub fn Table::chains_ending_at(&self, chars: &[char], end: usize, max_consumed: usize) -> Vec<ChainHit>` — every deinflection chain whose surface ends at character `end`. Always includes the empty chain (`consumed: 0`) so an uninflected word is found by the same call.

**Attachment.** IPADIC does not record which form an auxiliary attaches to — master plan §3.3: fields 2–4 are `*` on every auxiliary and there is no 使役 / 受身 / 丁寧 / 否定 tag anywhere. That information is a small hand-written table, and it is Phase 1a's because it decides which chains are findable, which is a span-shape question (spec §2.1).

```rust
/// Which 活用形 of the preceding word each auxiliary attaches to.
///
/// Written from a Japanese grammar reference. IPADIC carries the forms but
/// not the attachment (master plan §3.3), and this is the smaller half of
/// the §7.3 gap — the labels themselves are Phase 1b's.
const ATTACHES_TO: &[(&str, &str)] = &[
    ("せる", "未然形"), ("させる", "未然形"),
    ("れる", "未然形"), ("られる", "未然形"),
    ("ない", "未然形"), ("ぬ", "未然形"), ("ん", "未然形"),
    ("う", "未然ウ接続"), ("よう", "未然ウ接続"),
    ("ます", "連用形"), ("たい", "連用形"),
    ("た", "連用タ接続"), ("だ", "連用タ接続"),
    ("て", "連用タ接続"), ("で", "連用タ接続"),
    ("です", "基本形"),
    // Complete from the generator's class list: every class whose name starts
    // 特殊・ or 助動詞 is an auxiliary and needs a row, or it can never chain.
];
```

- [ ] **Step 1: Write the failing tests**

```rust
    fn table() -> Table { Table::new() }

    fn chars(s: &str) -> Vec<char> { s.chars().collect() }

    /// The stem the chain leaves behind, plus its class's remove-suffix:
    /// the exact string `lex.rs` will look up in the FST.
    fn key(c: &[char], hit: &ChainHit) -> String {
        let stem: String = c[..c.len() - hit.consumed].iter().collect();
        format!("{stem}{}", CLASSES[hit.stem_class as usize].remove)
    }

    #[test]
    fn an_uninflected_word_yields_the_empty_chain() {
        let c = chars("東京");
        let hits = table().chains_ending_at(&c, 2, 8);
        assert!(hits.iter().any(|h| h.consumed == 0 && h.steps.is_empty()));
    }

    #[test]
    fn a_single_step_recovers_the_dictionary_form() {
        let c = chars("食べた");
        let hits = table().chains_ending_at(&c, 3, 8);
        assert!(hits.iter().any(|h| key(&c, h) == "食べる"), "食べた -> 食べる");
    }

    #[test]
    fn the_godan_ta_form_is_found_through_its_own_class() {
        let c = chars("書いた");
        let hits = table().chains_ending_at(&c, 3, 8);
        assert!(hits.iter().any(|h| key(&c, h) == "書く"), "書いた -> 書く");
    }

    #[test]
    fn a_long_auxiliary_chain_resolves_to_the_verb() {
        // Master plan §7.3's example. Phase 1a must find it; naming it is 1b's.
        let c = chars("食べさせられませんでした");
        let hits = table().chains_ending_at(&c, c.len(), 16);
        let hit = hits.iter().find(|h| key(&c, h) == "食べる")
            .expect("the chain must reach 食べる");
        assert!(hit.steps.len() >= 5, "one step per auxiliary, got {}", hit.steps.len());
        assert_eq!(hit.steps[0].range.start, 2, "the innermost step starts after 食べ");
    }

    #[test]
    fn suru_resolves_from_an_empty_stem() {
        let c = chars("した");
        let hits = table().chains_ending_at(&c, 2, 8);
        assert!(hits.iter().any(|h| key(&c, h) == "する"));
    }

    #[test]
    fn an_adjective_conjugates() {
        let c = chars("高かった");
        let hits = table().chains_ending_at(&c, 4, 8);
        assert!(hits.iter().any(|h| key(&c, h) == "高い"));
    }

    #[test]
    fn the_search_is_bounded() {
        let c = chars("ててててててててててててててて");
        let hits = table().chains_ending_at(&c, c.len(), 4);
        assert!(hits.iter().all(|h| h.consumed <= 4));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::table`
Expected: FAIL — `Table` and `chains_ending_at` do not exist.

- [ ] **Step 3: Write the implementation**

`Table::new` builds a reverse trie keyed on suffix characters read right-to-left, each terminal node carrying every `(ClassId, FormId)` whose suffix spells that path. Build it once from `CLASSES`.

`chains_ending_at` is a bounded depth-first search from `end` leftward:

1. Emit the empty chain, with `stem_class` for every class — an uninflected word is a plain headword, so `consumed: 0` needs no class constraint; represent that as `stem_class` unset by emitting one hit per candidate only when a chain exists, and a single `consumed: 0` hit whose `stem_class` is a reserved `PLAIN` id whose `remove` is `""`. Add `PLAIN` as `CLASSES[0]` in the generator, named `"*"`, so the id is stable.
2. Walk the reverse trie from `end`, collecting `(class, form, len)` suffix hits.
3. For each hit, if `CLASSES[class]` is an auxiliary (its name appears as a lemma in `ATTACHES_TO`), recurse leftward requiring the next step's form to equal that auxiliary's attachment form; otherwise the hit terminates the chain and `stem_class` is its class.
4. Stop at `max_consumed` characters or when no suffix matches.

Push each step onto `steps` **after** the recursion returns, so `steps[0]` is the innermost — the main verb's own inflection — and the last element is the outermost auxiliary. `range` is the character range within the span that the step consumed.

Add `table: &Table` to `parse_v2`'s signature and thread it from the CLI, matching `parse`'s existing `table` parameter position.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::table`
Expected: PASS, 13 tests (6 from Task 4, 7 new).

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): deinflect at parse time via a reverse suffix trie

One mechanism instead of a build-time stem generator plus a parse-time
recursion, and one origin for the chain Phase 1b names. する and 来る
need no empty-stem special case: their whole surface is the suffix and
the trie finds it like any other.

Auxiliary attachment is a hand-written table because IPADIC does not
carry it -- master plan §3.3."
```

---

## Task 6: `engine/flags.rs` — headword records

**Files:**
- Create: `crates/jparser/src/engine/flags.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod flags;`)
- Test: in-file

**Interfaces:**
- Consumes: `jparser::jmdict::{RawEntry, KanjiForm, ReadingForm, RawSense}`, `jparser::record::WordFlags`, `engine::table::class_for_pos`.
- Produces: `pub struct Headword { pub surface: String, pub flags: WordFlags, pub classes: Vec<ClassId>, pub entry_id: u32 }` and `pub fn headwords(raw: &RawEntry) -> Vec<Headword>`.

`WordFlags` is reused, not redefined: `record.rs:69-79` pins its eight wire names as public API and the webview reads them to pick a chip's content class. Only the eight constants and their names are used here; `record.rs`'s derivation is not read.

**Flag semantics**, from the constants' own documentation:

| Flag | Set when |
| --- | --- |
| `PRIMARY` | The record is a kanji form, **or** the reading of an entry that has no kanji form |
| `PRONOUNCE` | The record is a reading of an entry that **does** have a kanji form |
| `COMMON_LINE` | Any form of the entry carries `has_priority` |
| `COMMON` | This specific form carries `has_priority` |
| `PARTICLE` | Any sense's `pos` contains `prt` or `conj` |
| `COUNTER` | Any sense's `pos` contains `ctr` or `suf`, and no sense's `misc` contains `arch` |
| `TOP`, `IS_NAME` | Never, in Phase 1a. Kept on the wire so no TypeScript moves |

The `PRIMARY` / `PRONOUNCE` split is load-bearing: spec §7.3's cost term prices a `PRONOUNCE`-only match that crosses a tokenizer boundary, and the fact that a kana-only word is `PRIMARY` is what keeps `は`, `こう` and `もう` out of that term.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::jmdict::{KanjiForm, RawEntry, RawSense, ReadingForm};

    fn entry(kanji: &[(&str, bool)], readings: &[(&str, bool)], pos: &[&str], misc: &[&str]) -> RawEntry {
        RawEntry {
            id: 1,
            kanji: kanji.iter().map(|(t, p)| KanjiForm { text: t.to_string(), has_priority: *p }).collect(),
            readings: readings.iter().map(|(t, p)| ReadingForm { text: t.to_string(), has_priority: *p }).collect(),
            senses: vec![RawSense {
                pos: pos.iter().map(|s| s.to_string()).collect(),
                misc: misc.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
        }
    }

    fn find<'a>(hs: &'a [Headword], surface: &str) -> &'a Headword {
        hs.iter().find(|h| h.surface == surface).expect(surface)
    }

    #[test]
    fn a_kanji_form_is_primary_and_its_reading_is_pronounce() {
        let hs = headwords(&entry(&[("履く", true)], &[("はく", false)], &["v5k"], &[]));
        assert!(find(&hs, "履く").flags.contains(WordFlags::PRIMARY));
        assert!(find(&hs, "はく").flags.contains(WordFlags::PRONOUNCE));
        assert!(!find(&hs, "はく").flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn a_kana_only_entry_is_primary_not_pronounce() {
        // The property spec §7.3 depends on: は and こう must not be caught by
        // the PRONOUNCE-crossing cost term.
        let hs = headwords(&entry(&[], &[("は", true)], &["prt"], &[]));
        assert!(find(&hs, "は").flags.contains(WordFlags::PRIMARY));
        assert!(!find(&hs, "は").flags.contains(WordFlags::PRONOUNCE));
    }

    #[test]
    fn priority_marks_the_form_and_the_line() {
        let hs = headwords(&entry(&[("東京", true), ("東亰", false)], &[("とうきょう", false)], &["n"], &[]));
        assert!(find(&hs, "東京").flags.contains(WordFlags::COMMON));
        assert!(find(&hs, "東亰").flags.contains(WordFlags::COMMON_LINE));
        assert!(!find(&hs, "東亰").flags.contains(WordFlags::COMMON));
    }

    #[test]
    fn particles_and_counters_are_flagged() {
        let hs = headwords(&entry(&[], &[("が", false)], &["prt"], &[]));
        assert!(find(&hs, "が").flags.contains(WordFlags::PARTICLE));
        let hs = headwords(&entry(&[("日", false)], &[("にち", false)], &["ctr"], &[]));
        assert!(find(&hs, "日").flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn an_archaic_counter_is_not_a_counter() {
        let hs = headwords(&entry(&[("枚", false)], &[("まい", false)], &["ctr"], &["arch"]));
        assert!(!find(&hs, "枚").flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn conjugation_classes_come_from_every_senses_pos() {
        let hs = headwords(&entry(&[("食べる", true)], &[("たべる", false)], &["v1"], &[]));
        assert!(!find(&hs, "食べる").classes.is_empty());
    }

    #[test]
    fn top_and_is_name_are_never_set() {
        for h in headwords(&entry(&[("東京", true)], &[("とうきょう", false)], &["n"], &[])) {
            assert!(!h.flags.contains(WordFlags::TOP));
            assert!(!h.flags.contains(WordFlags::IS_NAME));
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::flags`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

One `Headword` per kanji form and one per reading form. `classes` collects `class_for_pos` over every POS code of every sense, deduplicated — an entry tagged `v5r` in one sense and `v1` in another yields two, which is correct and is why the field is a `Vec`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::flags`
Expected: PASS, 7 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): derive headword records and flags from JMdict

WordFlags is reused rather than redefined: its eight wire names are
public API the webview reads. TOP and IS_NAME stay on the wire and are
set by nothing, so no TypeScript moves.

The PRIMARY/PRONOUNCE split is what the cost model's crossing term
depends on, and a kana-only word being PRIMARY is what keeps は and
こう out of it."
```

---

## Task 7: The v2 index build

**Files:**
- Create: `crates/jparser/src/engine/build.rs`
- Modify: `crates/jparser/src/index/build.rs` (extract `write_index`)
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod build;`)
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (`--engine` on `build-index`)
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Consumes: `jparser::jmdict::parse_entries`, `engine::flags::headwords`, `engine::kana::fold_str`, `index::{StoredRecord, EntryData, BuildReport, IndexError}`.
- Produces:
  - `pub(crate) fn crate::index::build::write_index(by_key: BTreeMap<String, Vec<StoredRecord>>, entries: Vec<EntryData>, record_count: usize, fingerprint: u64, out_dir: &Path) -> Result<(), IndexError>`
  - `pub fn engine::build::build_from_reader<R: BufRead>(xml: R, out_dir: &Path) -> Result<BuildReport, IndexError>`
  - `pub fn engine::build::fingerprint() -> u64`

**Why two builds.** `IndexHeader.conjugation_fingerprint` makes `Index::open` reject an index whose `verb_type` ids came from a different table. v2 assigns different ids, so it needs its own index — and the guard then makes it impossible for either engine to read the other's index silently. That is the guard working, not friction (spec §5).

v2 emits **headwords only, no generated stems**, because deinflection happens at parse time. `StoredRecord::verb_type` is therefore always `None` in a v2 index: a field present and unused, not a format difference. No version bump.

- [ ] **Step 1: Extract `write_index` from `index/build.rs`**

Move the block from `// Payload: length-prefixed bincode blobs;` through the `std::fs::write(out_dir.join(HEADER_FILE), ..)` line (`build.rs:96-134`) into a new private function, taking the fingerprint as a parameter rather than calling `embedded_asset_fingerprint()` directly. `build_from_reader` then calls it with `crate::conjugation::embedded_asset_fingerprint()`, unchanged in behaviour.

- [ ] **Step 2: Run the existing tests to prove the extraction changed nothing**

Run: `cargo test -p jparser`
Expected: PASS, 324 + whatever the earlier tasks added. Every index test must still pass — this step is behaviour-preserving.

Commit the extraction on its own:

```powershell
git add crates/jparser/src/index/build.rs
git commit -m "refactor(index): extract write_index so both builders share it"
```

- [ ] **Step 3: Write the failing test**

Create `crates/jparser/tests/engine_v2.rs`:

```rust
// ClipYomi — Engine v2, Phase 1a.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.

//! Integration tests for the Engine v2 path.

use std::io::BufReader;
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("fixtures/jmdict_subset.xml");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("jparser-engine-v2").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn build_v2(dir: &Path) {
    jparser::engine::build::build_from_reader(BufReader::new(FIXTURE.as_bytes()), dir)
        .expect("v2 build");
}

#[test]
fn a_v2_index_opens_and_carries_no_stem_records() {
    let dir = scratch("build");
    build_v2(&dir);
    let index = jparser::index::load::Index::open(&dir).expect("open v2 index");
    assert!(index.entry_count() > 0);
    for hit in index.prefixes_of("東京都").expect("walk") {
        for r in &hit.records {
            assert!(r.verb_type.is_none(), "v2 emits no generated stems");
        }
    }
}

#[test]
fn the_two_engines_cannot_read_each_others_indexes() {
    // The fingerprint guard, doing exactly what it exists for.
    let v2_dir = scratch("v2-for-v1");
    build_v2(&v2_dir);

    let table = jparser::conjugation::ConjugationTable::load_embedded().expect("v1 table");
    let v1_dir = scratch("v1-for-v2");
    jparser::index::build::build_from_reader(
        BufReader::new(FIXTURE.as_bytes()),
        &table,
        &jparser::stem::StemOptions::default(),
        &v1_dir,
    )
    .expect("v1 build");

    assert_ne!(
        jparser::engine::build::fingerprint(),
        jparser::conjugation::embedded_asset_fingerprint(),
        "the two tables must not collide, or the guard is vacuous"
    );
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p jparser --test engine_v2`
Expected: FAIL — `engine::build` does not exist.

- [ ] **Step 5: Write `engine/build.rs`**

The same entry loop as `index/build.rs` with three differences: `engine::flags::headwords(&raw)` instead of `record::headwords(&raw, table)`; no `generate_stems` call and no stem records; `engine::kana::fold_str` as the key normalization instead of `unify_str`. Then call `write_index` with `engine::build::fingerprint()`.

`fingerprint()` is an FNV-1a hash over `CLASSES`' names, remove-suffixes and form suffixes in order — the same property `embedded_asset_fingerprint` gives the old asset: reordering or adding a class changes every id downstream, so an index built against a different table must be rejected.

`BuildReport.stems` is `StemStats::default()`: v2 generates none.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p jparser --test engine_v2`
Expected: PASS, 2 tests.

- [ ] **Step 7: Add `--engine` to `build-index`**

The `BuildIndex` variant gains the same `#[arg(long, value_enum, default_value_t = EngineChoice::V1)] engine: EngineChoice` and its match arm selects the builder. `--no-v5-fallback` is meaningless on the v2 path — reject the combination with a clap error rather than ignoring it silently.

Register the test in `crates/jparser/Cargo.toml`:

```toml
[[test]]
name = "engine_v2"
path = "tests/engine_v2.rs"
```

- [ ] **Step 8: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine crates/jparser/tests/engine_v2.rs crates/jparser/Cargo.toml crates/jparser/src/bin/jparser-cli
git commit -m "feat(engine-v2): build a v2 index, headwords only

Deinflection is at parse time, so the FST needs no generated stems and
verb_type is always None -- a field present and unused, not a format
change, so no version bump. The existing fingerprint guard then makes
it impossible for either engine to read the other's index silently."
```

---

## Task 8: `engine/lex.rs` — candidate generation

**Files:**
- Create: `crates/jparser/src/engine/lex.rs`
- Modify: `crates/jparser/src/index/load.rs` (add `prefixes_of_with`)
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod lex;`)
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Consumes: `Index::prefixes_of_with`, `Table::chains_ending_at`, `WordFlags`.
- Produces:
  - `pub struct Candidate { pub start: usize, pub len: usize, pub entry_id: u32, pub flags: WordFlags, pub steps: Vec<ConjStep>, pub exact: bool, pub headword: String }` — `exact` is false when the source characters differ from the stored surface before folding, which is the kana-unification signal spec §7.4 prices.
  - `pub fn candidates(index: &Index, table: &Table, chars: &[char]) -> Result<Vec<Vec<Candidate>>, IndexError>` — indexed by start position.
  - `pub fn Index::prefixes_of_with(&self, text: &str, fold: fn(char) -> char) -> Result<Vec<PrefixHit>, IndexError>`

- [ ] **Step 1: Add `prefixes_of_with` to `index/load.rs`**

`prefixes_of` folds each character through `crate::kana::unify` inside the FST walk. Add a sibling taking the fold as a parameter, and make `prefixes_of` call it with `unify` so there is one walk, not two:

```rust
    /// [`prefixes_of`] with the key normalization supplied by the caller.
    ///
    /// The two engines fold differently and an index is built with the fold
    /// its own engine uses, so the walk has to take it as a parameter. At the
    /// Phase 1b switchover this becomes the only form.
    pub fn prefixes_of_with(
        &self,
        text: &str,
        fold: fn(char) -> char,
    ) -> Result<Vec<PrefixHit>, IndexError> {
        // the existing body, with `unify(ch)` replaced by `fold(ch)`
    }

    pub fn prefixes_of(&self, text: &str) -> Result<Vec<PrefixHit>, IndexError> {
        self.prefixes_of_with(text, unify)
    }
```

Run: `cargo test -p jparser`
Expected: PASS, unchanged count — this is behaviour-preserving.

- [ ] **Step 2: Write the failing tests**

Add to `crates/jparser/tests/engine_v2.rs`:

```rust
fn cands(dir: &Path, text: &str) -> Vec<Vec<jparser::engine::lex::Candidate>> {
    let index = jparser::index::load::Index::open(dir).expect("open");
    let table = jparser::engine::table::Table::new();
    let chars: Vec<char> = text.chars().collect();
    jparser::engine::lex::candidates(&index, &table, &chars).expect("candidates")
}

#[test]
fn a_plain_headword_is_found_at_its_position() {
    let dir = scratch("lex-plain");
    build_v2(&dir);
    let c = cands(&dir, "東京都に住む");
    assert!(c[0].iter().any(|x| x.headword == "東京" && x.len == 2));
}

#[test]
fn a_conjugated_verb_is_found_with_its_chain() {
    let dir = scratch("lex-conj");
    build_v2(&dir);
    let c = cands(&dir, "書いた");
    let hit = c[0].iter().find(|x| x.headword == "書く").expect("書いた -> 書く");
    assert_eq!(hit.len, 3, "the candidate spans the whole conjugated surface");
    assert!(!hit.steps.is_empty());
}

#[test]
fn a_kana_unified_match_is_marked_inexact() {
    // はいた matching 履く through the reading はく is the §9.5 defect class.
    // lex must find it -- suppressing it here would hide the case the cost
    // model exists to price -- but it must be marked.
    let dir = scratch("lex-inexact");
    build_v2(&dir);
    let c = cands(&dir, "ハイタ");
    assert!(c[0].iter().any(|x| !x.exact), "a katakana match of a hiragana key is inexact");
}
```

If `jmdict_subset.xml` does not carry the entries these tests name, extend the fixture rather than weakening the test — `tests/fixtures/README.md` documents how it was built.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p jparser --test engine_v2`
Expected: FAIL — `engine::lex` does not exist.

- [ ] **Step 4: Write the implementation**

For each start position `i`, and each end `j > i` reachable by the FST walk:

1. `table.chains_ending_at(chars, j, MAX_CHAIN)` gives every chain ending at `j`.
2. For each chain, the stem is `chars[i..j - consumed]` and the lookup key is `stem + CLASSES[stem_class].remove`.
3. `index.prefixes_of_with(&key, kana::fold)` — take only the hit whose `key_chars` equals the key's character length, so a prefix of the key is not mistaken for it.
4. For each `StoredRecord` in that hit, emit a `Candidate`. `exact` is `record.surface == key`; the fold made them equal as keys, and a difference in the unfolded strings is the kana-unification signal.
5. Discard a candidate whose chain requires a class the record's headword does not have — a `一段` chain over a `五段` verb is not a reading.

`MAX_CHAIN` is a named constant with a comment: the longest chain the corpus exercises plus headroom. Start at 12 characters and record the value that the Task 16 measurement supports.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p jparser --test engine_v2`
Expected: PASS, 5 tests.

- [ ] **Step 6: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine crates/jparser/src/index/load.rs crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine-v2): generate span candidates from the FST and the trie

prefixes_of takes its fold as a parameter now: the two engines normalize
differently and each index is built with its own engine's fold.

Inexact matches are found and marked, not suppressed -- the cost model
prices them, and hiding them here would hide the class it exists for."
```

---

## Task 9: `engine/cost.rs` — the cost terms

**Files:**
- Create: `crates/jparser/src/engine/cost.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod cost;`)
- Test: in-file

**Interfaces:**
- Consumes: `Candidate`, `Boundaries`, `WordFlags`.
- Produces:
  - `pub fn cost(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> i32`
  - `pub fn unmatched_cost(chars: &[char], start: usize, len: usize) -> i32`
  - `pub fn is_allowed(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> bool`

**Every constant carries its justification.** This is spec §3's operative rule and this file is where it binds hardest.

```rust
/// Cost of one covered character in a matched span.
///
/// Per character, not per match. A per-match constant is what makes `はこう`
/// (one match) beat `は`+`こう` (two) — master plan §4.3 cause 2 — and the
/// interior constraint does not forbid that merge, because `は` and `こう`
/// are separate tokens and merging across a boundary is always permitted
/// (§4.3's asymmetry, the rule that saves `東京都`). Removing the bias
/// structurally is the only form of the fix that satisfies success criterion 3.
const COVER: i32 = 1;

/// Cost of one character in an unmatched run.
///
/// Far above COVER, so a fallback run never outbids a dictionary reading.
/// Handoff §9.1 measured the opposite failure: 36 of 266 lost gold boundaries
/// were runs of punctuation lumped into one unmatched segment (`」、`, `、「喜`),
/// the one unambiguous defect in that decomposition.
const UNMATCHED: i32 = 30;

/// Cost of crossing one tokenizer boundary.
///
/// Priced, never forbidden: master plan §4.3's asymmetry requires `東京都` to
/// merge across the `東京|都` boundary and win. Tuned in Task 16 against the
/// corpus, with `東京都` and `音がする` as the merges that must survive it.
const CROSS: i32 = 4;

/// Extra cost when a PRONOUNCE-only match crosses a tokenizer boundary.
///
/// Handoff §9.5's ~28 spurious merges are every one of them a match against a
/// kanji headword's *reading*: `はいた`→履く via はく, `ももう`→揉む via もむ,
/// `がい`→甲斐, `してい`→指定, `どうし`→同士, `ことし`→今年, `になって`→荷なう.
/// Every legitimate merge in §9.1 is kanji-bearing: `音がする`, `ことになっている`,
/// `首を振る`, `これまでに`, `東京都`, `2月`. A kana-only word is PRIMARY by that
/// flag's own definition, so `は`, `こう` and `もう` are not caught.
///
/// Handoff §9.6's refusal to add a heuristic to the *metric* is unaffected:
/// there the object is a merged span the corpus carries no label for. Here the
/// flag is already computed.
const PRONOUNCE_CROSS: i32 = 25;

/// Discount for a match whose entry carries a priority marker.
///
/// A frequency signal JMdict supplies directly, and the only evidence
/// available for choosing between two equally-covering readings.
const COMMON: i32 = -2;

/// Penalty for a match whose source characters differ from the stored surface
/// before folding.
///
/// Every §9.5 absurdity is inexact against its headword; every §9.1
/// legitimate merge is exact modulo conjugation. Independent of
/// PRONOUNCE_CROSS and measured separately in Task 16.
const INEXACT: i32 = 8;
```

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct At(&'static [usize]);
    impl crate::engine::Boundaries for At {
        fn is_boundary(&self, pos: usize) -> bool { self.0.contains(&pos) }
    }

    fn cand(start: usize, len: usize, flags: WordFlags, exact: bool) -> Candidate {
        Candidate { start, len, entry_id: 1, flags, steps: Vec::new(), exact,
                    headword: String::new() }
    }

    #[test]
    fn coverage_cost_does_not_reward_using_fewer_matches() {
        // The §4.3 fix, as an equality. Without it `はこう` wins by arithmetic.
        let whole = cost(&cand(4, 3, WordFlags::PRIMARY, true), None);
        let split = cost(&cand(4, 1, WordFlags::PRIMARY, true), None)
                  + cost(&cand(5, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(whole, split);
    }

    #[test]
    fn crossing_a_boundary_costs_something_but_is_allowed() {
        let b = At(&[0, 4, 5, 7]);
        let c = cand(4, 3, WordFlags::PRIMARY, true);
        assert!(is_allowed(&c, Some(&b)), "merging across a boundary is legal");
        assert!(cost(&c, Some(&b)) > cost(&c, None));
    }

    #[test]
    fn a_pronounce_match_that_crosses_is_priced_far_above_a_primary_one() {
        // はいた as 履く against a real は + いた.
        let b = At(&[0, 1, 3]);
        let reading = cost(&cand(0, 3, WordFlags::PRONOUNCE, true), Some(&b));
        let kanji = cost(&cand(0, 3, WordFlags::PRIMARY, true), Some(&b));
        assert!(reading > kanji + 20, "reading {reading} kanji {kanji}");
    }

    #[test]
    fn a_kana_only_word_is_not_caught_by_the_pronounce_term() {
        let b = At(&[0, 1, 3]);
        let kana_only = cand(0, 3, WordFlags::PRIMARY, true);
        assert_eq!(cost(&kana_only, Some(&b)), cost(&kana_only, Some(&b)));
        assert!(!kana_only.flags.contains(WordFlags::PRONOUNCE));
    }

    #[test]
    fn a_span_starting_or_ending_inside_a_token_is_forbidden() {
        let b = At(&[0, 2, 3]);
        assert!(!is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), Some(&b)));
        assert!(!is_allowed(&cand(0, 1, WordFlags::PRIMARY, true), Some(&b)));
        assert!(is_allowed(&cand(0, 2, WordFlags::PRIMARY, true), Some(&b)));
    }

    #[test]
    fn every_constraint_lifts_when_there_is_no_tokenizer() {
        assert!(is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), None));
    }

    #[test]
    fn an_unmatched_character_costs_far_more_than_a_matched_one() {
        let chars: Vec<char> = "」、".chars().collect();
        assert!(unmatched_cost(&chars, 0, 2) > cost(&cand(0, 2, WordFlags::PRIMARY, true), None) * 10);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::cost`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

`is_allowed` returns false when `bounds` is `Some` and either `!is_boundary(start)` or `!is_boundary(start + len)`. With `bounds == None` it always returns true.

`cost` sums the terms above, counting interior boundaries as the number of `p` in `start+1 .. start+len` for which `is_boundary(p)`.

`unmatched_cost` is `UNMATCHED * len`, plus a per-script-transition surcharge inside the run so `丨FIFA` prefers to break at the script change — handoff §9.1's cheap half of the fallback defect. Use `kana::is_kanji` / `is_kana` / `is_digit` to classify.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::cost`
Expected: PASS, 7 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): cost per character, and price boundary crossing

Every constant carries the Phase 0 measurement that justifies it; a
constant without one does not ship (master plan §3.5).

The PRONOUNCE crossing term is the §9.5 defect class stated as a rule:
all ~28 are reading-form matches of kanji headwords, all the legitimate
merges are kanji-bearing, and a kana-only word is PRIMARY so は and
こう are untouched."
```

---

## Task 10: `engine/lattice.rs` — the min-cost path

**Files:**
- Create: `crates/jparser/src/engine/lattice.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod lattice;`)
- Test: in-file

**Interfaces:**
- Consumes: `Candidate`, `cost`, `unmatched_cost`, `is_allowed`.
- Produces:
  - `pub enum Step { Matched(Candidate), Unmatched { start: usize, len: usize } }`
  - `pub fn best_path(chars: &[char], by_start: &[Vec<Candidate>], bounds: Option<&dyn Boundaries>) -> Vec<Step>` — a contiguous cover of `0..chars.len()` in ascending order.

Shape it so k follows (master plan §4.2): keep a `best: Vec<Option<(i32, usize, Step)>>` array indexed by end position, and write the relaxation as a function of `(from, to, step_cost)`. Phase 2 replaces the single best with a bounded list; nothing else changes.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn c(start: usize, len: usize, id: u32, flags: WordFlags) -> Candidate {
        Candidate { start, len, entry_id: id, flags, steps: Vec::new(), exact: true,
                    headword: String::new() }
    }

    fn by_start(n: usize, cands: Vec<Candidate>) -> Vec<Vec<Candidate>> {
        let mut v = vec![Vec::new(); n];
        for cand in cands { v[cand.start].push(cand); }
        v
    }

    #[test]
    fn the_path_covers_the_input_exactly_once() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let path = best_path(&chars, &by_start(5, vec![c(0, 2, 1, WordFlags::PRIMARY)]), None);
        let mut at = 0;
        for step in &path {
            let (s, l) = match step {
                Step::Matched(x) => (x.start, x.len),
                Step::Unmatched { start, len } => (*start, *len),
            };
            assert_eq!(s, at);
            at += l;
        }
        assert_eq!(at, 5);
    }

    #[test]
    fn unmatched_characters_become_one_run_not_many() {
        let chars: Vec<char> = "」、「".chars().collect();
        let path = best_path(&chars, &by_start(3, vec![]), None);
        assert_eq!(path.len(), 1, "one run, not three single characters");
    }

    #[test]
    fn a_forbidden_candidate_is_never_taken() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool { self.0.contains(&p) }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        // A candidate 京都 at 1..3 starts inside the 東京 token.
        let path = best_path(&chars, &by_start(3, vec![c(1, 2, 9, WordFlags::PRIMARY)]),
                             Some(&At(&[0, 2, 3])));
        assert!(!path.iter().any(|s| matches!(s, Step::Matched(x) if x.entry_id == 9)));
    }

    #[test]
    fn a_compound_may_merge_across_a_boundary_and_win() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool { self.0.contains(&p) }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        let path = best_path(
            &chars,
            &by_start(3, vec![c(0, 3, 1, WordFlags::PRIMARY | WordFlags::COMMON),
                              c(0, 2, 2, WordFlags::PRIMARY),
                              c(2, 1, 3, WordFlags::PRIMARY)]),
            Some(&At(&[0, 2, 3])),
        );
        assert!(path.iter().any(|s| matches!(s, Step::Matched(x) if x.entry_id == 1)),
                "東京都 must be able to pay the crossing price and win");
    }

    #[test]
    fn the_empty_input_yields_an_empty_path() {
        assert!(best_path(&[], &[], None).is_empty());
    }
}
```

`WordFlags` needs `BitOr` for the fourth test — add it to `record.rs`'s impl block if it is not there, which is an additive change to a file whose *bodies* Phase 1a does not rewrite but whose public API it may extend. If you would rather not touch it, construct the flags with two `insert` calls instead.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::lattice`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

A forward dynamic program over end positions `0..=n`. At each reachable `from`, relax every allowed candidate starting there, and also relax every unmatched run `from..to` for `to` in `from+1 ..= n` — bounded by a named `MAX_UNMATCHED_RUN` so the relaxation is linear rather than quadratic in pathological input, with a comment giving the bound's reason. Reconstruct by back-pointers.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::lattice`
Expected: PASS, 5 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): min-cost path over the candidate lattice

Written as a relaxation over a best[] array indexed by end position, so
Phase 2's k-best replaces the single best with a bounded list and
nothing else moves (master plan §4.2)."
```

---

## Task 11: `engine/chain.rs` — the span shape of an auxiliary chain

**Files:**
- Create: `crates/jparser/src/engine/chain.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod chain;`)
- Test: in-file

**Interfaces:**
- Consumes: `ConjStep`, `Candidate`.
- Produces: `pub fn steps_for(cand: &Candidate) -> Vec<ConjStep>` — the chain in innermost-first order, ranges relative to the parsed text.

This is the half of master plan §7.3's recombiner that Phase 1a owns (spec §2.1). The span shape is already decided by Task 10 — a chain that survives as one candidate is one segment — so this task's job is to make the steps that reach `Entry` correct, ordered, and correctly ranged, because Phase 1b's formatter reads nothing else.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_innermost_first() {
        // 食べさせられませんでした: the verb's own inflection first, then each
        // auxiliary outward. Phase 1b renders left to right from this order.
        let cand = candidate_for("食べさせられませんでした", 0);
        let steps = steps_for(&cand);
        assert!(steps.len() >= 5);
        assert!(steps[0].range.start <= steps[1].range.start);
        assert_eq!(steps[0].range.start, 2, "食べ is the stem");
        assert_eq!(steps.last().unwrap().range.end, 12);
    }

    #[test]
    fn the_ranges_tile_the_conjugated_tail_without_gaps() {
        let cand = candidate_for("食べさせられませんでした", 0);
        let steps = steps_for(&cand);
        for pair in steps.windows(2) {
            assert_eq!(pair[0].range.end, pair[1].range.start, "no gap, no overlap");
        }
    }

    #[test]
    fn an_uninflected_match_has_no_steps() {
        assert!(steps_for(&candidate_for("東京", 0)).is_empty());
    }

    #[test]
    fn the_ranges_are_offset_by_the_spans_start() {
        let cand = candidate_for("私は食べた", 2);
        let steps = steps_for(&cand);
        assert_eq!(steps[0].range.start, 4, "食べ starts at 2, た at 4");
    }
}
```

Write `candidate_for(text, start)` as a test helper that runs `lex::candidates` over a v2 index built from the fixture and picks the candidate at `start` spanning to the end of the text. If the fixture lacks 食べる, extend it.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser engine::chain`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

`Table::chains_ending_at` already builds the steps innermost-first with ranges relative to the span; `steps_for` offsets each range by `cand.start` and returns them. Keep the function even though it is thin: it is the named seam Phase 1b reads, and a test that pins the order and the tiling is worth more than the line it saves.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser engine::chain`
Expected: PASS, 4 tests.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine
git commit -m "feat(engine-v2): fix the chain's span shape and step order

Phase 1a's half of master plan §7.3: the steps, their order and their
ranges. Phase 1b's formatter reads nothing else, so pinning the tiling
here is what keeps 1b from forcing a 1a rework."
```

---

## Task 12: `engine/assemble.rs` — candidates to entries

**Files:**
- Create: `crates/jparser/src/engine/assemble.rs`
- Modify: `crates/jparser/src/engine/mod.rs` (add `pub mod assemble;`)
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Consumes: `Index::entry`, `Candidate`, `steps_for`, `EntryData`, `SenseData`.
- Produces: `pub fn to_segment(index: &Index, chars: &[char], step: &Step, siblings: &[Candidate]) -> Result<Segment, IndexError>`

Per spec §8, `conjugation` is `None` throughout Phase 1a.

**Reading reconstruction.** `EntryData::readings` holds an entry's `<reb>` forms **only when the entry also has kanji forms**; it is empty for a kana-only entry, where the surface already is the reading. So: a `PRONOUNCE` match is its own reading and needs none attached; a `PRIMARY` kanji match takes `readings[0]`; a kana-only match has no separate reading. When the match is conjugated, the stored reading is the dictionary form's — strip its own class's remove-suffix and append the matched surface's tail, or return `None` if it does not strip cleanly. Do not invent a reading.

- [ ] **Step 1: Write the failing tests**

Add to `crates/jparser/tests/engine_v2.rs`:

```rust
#[test]
fn a_matched_segment_carries_its_entry_and_senses() {
    let dir = scratch("assemble-basic");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "東京");
    let seg = &r.segments[0];
    assert!(seg.matched);
    assert_eq!(seg.surface, "東京");
    assert!(!seg.entries.is_empty());
    assert!(!seg.entries[0].senses.is_empty());
    assert!(!seg.entries[0].pos.is_empty());
    assert_eq!(seg.entries[0].entry_id, 1568660, "the ent_seq entry top-1 is scored on");
}

#[test]
fn phase_1a_emits_no_conjugation_label_but_does_emit_the_steps() {
    let dir = scratch("assemble-steps");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "書いた");
    let e = &r.segments[0].entries[0];
    assert_eq!(e.headword, "書く");
    assert_eq!(e.conjugation, None, "Phase 1b writes the formatter");
    assert!(!e.steps.is_empty(), "and Phase 1a hands it the steps");
}

#[test]
fn an_unmatched_run_has_no_entries_and_no_reading() {
    let dir = scratch("assemble-unmatched");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "」、「");
    assert!(r.segments.iter().any(|s| !s.matched && s.entries.is_empty() && s.reading.is_none()));
}

#[test]
fn pos_is_the_union_of_every_sense_in_first_seen_order() {
    let dir = scratch("assemble-pos");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "東京");
    let pos = &r.segments[0].entries[0].pos;
    let mut seen = pos.clone();
    seen.dedup();
    assert_eq!(&seen, pos, "deduplicated");
}
```

Replace `1568660` with the real `ent_seq` of 東京 in `jmdict_subset.xml`; read it from the fixture rather than guessing. Write `parse_v2_over(dir, text)` as a helper that opens the index, builds a `Table`, and calls `parse_v2` with `bounds: None`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --test engine_v2`
Expected: FAIL — `engine::assemble` does not exist, and `parse_v2` still returns the stub.

- [ ] **Step 3: Write the implementation**

`to_segment` resolves `entry_id` through `Index::entry`, builds one `Entry` per sibling candidate on the same span, unions their POS codes in first-seen order with duplicates removed, and sets `Segment::reading` from `entries[0].reading`. An `Unmatched` step becomes a segment with `matched: false`, `reading: None`, `entries: vec![]`.

`Segment::surface` is `chars[start..start+len]` verbatim — the source text, never the folded key.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --test engine_v2`
Expected: PASS.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine-v2): assemble candidates into segments and entries

A reading is never invented: a stored reading that does not strip back
to the matched stem yields None, which is what the field's contract
already says."
```

---

## Task 13: Candidate ordering within a span

**Files:**
- Modify: `crates/jparser/src/engine/assemble.rs`
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Produces: `pub fn order(cands: &mut Vec<Candidate>)` — a total order, applied before entries are built.

This is exactly what entry top-1 measures. Baseline **0.9159** hints-off and **0.9155** hints-on, and Task 16 will not pass if this is careless. The gold corpus's candidate lists were shown to the adjudicator **ordered by `ent_seq`, never by the ranker's score** (master plan §5.4), so the metric is independent of this ordering — which means a regression here is real and not an artefact.

Order by, in sequence: exact before inexact; `COMMON` before not; `PRIMARY` before `PRONOUNCE`; longer chain last (an uninflected reading of the same surface is the likelier one); then ascending `entry_id` as the stable tiebreak. Every clause is a fact JMdict or the match supplies; none is a frequency table this project does not have.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn an_exact_match_outranks_an_inexact_one() {
    let dir = scratch("order-exact");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "東京");
    let e = &r.segments[0].entries;
    let first_inexact = e.iter().position(|x| x.flags.contains(WordFlags::PRONOUNCE));
    let last_exact = e.iter().rposition(|x| x.flags.contains(WordFlags::PRIMARY));
    if let (Some(i), Some(l)) = (first_inexact, last_exact) {
        assert!(l < i, "every PRIMARY entry precedes every PRONOUNCE one");
    }
}

#[test]
fn the_order_is_stable_and_total() {
    let dir = scratch("order-stable");
    build_v2(&dir);
    let a = parse_v2_over(&dir, "東京都に住む");
    let b = parse_v2_over(&dir, "東京都に住む");
    assert_eq!(a, b, "two identical parses order identically");
}

#[test]
fn ties_break_on_entry_id_ascending() {
    let dir = scratch("order-ties");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "は");
    let ids: Vec<u32> = r.segments[0].entries.iter()
        .filter(|e| e.flags.contains(WordFlags::PARTICLE))
        .map(|e| e.entry_id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "particles with equal evidence sort by ent_seq");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --test engine_v2`
Expected: FAIL — `order` does not exist and the entries come out in FST order.

- [ ] **Step 3: Write the implementation**

`sort_by` over a tuple key in the sequence above. Use a total, deterministic key so `sort_unstable_by_key` is safe; `entry_id` last makes the order total.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --test engine_v2`
Expected: PASS.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine-v2): order candidates within a span

This is what entry top-1 measures -- baseline 0.9159 hints-off, 0.9155
hints-on. Every clause is a fact JMdict or the match supplies; ent_seq
ascending is the stable tiebreak, and the corpus's candidate lists were
ent_seq-ordered too, so the metric stays independent of this order."
```

---

## Task 14: Wire `parse_v2` end to end

**Files:**
- Modify: `crates/jparser/src/engine/mod.rs` (replace the stub)
- Create: `crates/jparser/tests/snapshots/engine_v2__sentences.snap` (generated by `insta`)
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Produces: the real `parse_v2`, wiring `lex::candidates` → `lattice::best_path` → `assemble::to_segment`.

- [ ] **Step 1: Write the failing test — the motivating bug**

```rust
#[test]
fn kurumadehakou_parses_as_four_words() {
    // Master plan success criterion 3, and the sentence this whole phase
    // exists for. Vibrato tokenizes it correctly and the v1 engine discards
    // that, producing くる / まで / はこう (§4.3).
    let dir = scratch("kurumadehakou");
    build_v2(&dir);
    let bounds = boundaries_over("くるまではこう");
    let r = parse_v2_with_bounds(&dir, "くるまではこう", Some(&bounds));
    let cuts: Vec<&str> = r.segments.iter().map(|s| s.surface.as_str()).collect();
    assert_eq!(cuts, ["くる", "まで", "は", "こう"]);
}
```

`boundaries_over` uses `VibratoTokenizer::boundaries` and the test is `#[cfg(feature = "mecab")]`. Point it at the dictionary through the same environment variable the existing `cli_hints.rs` tests use; read that file for the pattern.

Add a second test that does not need the tokenizer, so the no-mecab build still covers the path:

```rust
#[test]
fn the_result_is_a_contiguous_cover_in_ascending_order() {
    let dir = scratch("cover");
    build_v2(&dir);
    for text in ["東京都に住む", "」、「", "", "書いた本を読んだ"] {
        let r = parse_v2_over(&dir, text);
        let mut at = 0;
        for s in &r.segments {
            assert_eq!(s.start, at);
            at += s.len;
        }
        assert_eq!(at, text.chars().count());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --features mecab --test engine_v2`
Expected: FAIL — `parse_v2` still returns one unmatched segment.

- [ ] **Step 3: Write the implementation**

```rust
pub fn parse_v2(
    index: &Index,
    table: &table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError> {
    let _ = opts;
    let chars: Vec<char> = text.chars().collect();
    let by_start = lex::candidates(index, table, &chars)?;
    let path = lattice::best_path(&chars, &by_start, bounds);
    let mut segments = Vec::with_capacity(path.len());
    for step in &path {
        segments.push(assemble::to_segment(index, &chars, step, &by_start)?);
    }
    Ok(ParseResult { segments })
}
```

Delete `stub_result` and its two tests, which Task 1 wrote to be replaced here.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --features mecab --test engine_v2`
Expected: PASS. If `くるまではこう` still fails, the cost model needs the tuning of Task 16 — record the actual segmentation in the commit message and come back to this test after Task 16 rather than weakening it.

- [ ] **Step 5: Add v2's own snapshot**

Add a snapshot test over `tests/fixtures/parse_sentences.txt`, following `parse_snapshots.rs`'s rendering — read that file for the format; it is not one of the nine.

**This is a new snapshot, reviewed on its own terms.** Do not copy `parse_snapshots__sentences.snap` and do not diff against it. Master plan §5.1 makes v1 a disagreement source, not an oracle: a v1/v2 difference is a sentence worth reading, never a failure.

Run: `cargo insta test -p jparser --accept` (or set `INSTA_UPDATE=always`), then **read the generated snapshot** and record in the commit message how many of the 30 sentences differ from v1's and whether each difference is an improvement.

- [ ] **Step 6: Run the standing gates and commit**

```powershell
git add crates/jparser/src/engine crates/jparser/tests
git commit -m "feat(engine-v2): wire parse_v2 end to end

くるまではこう parses as くる / まで / は / こう -- master plan success
criterion 3, and the sentence this phase exists for.

v2 gets its own snapshot over the 30 curated sentences, reviewed on its
own terms. <N> of 30 differ from v1: <summary>. v1's snapshot is not
promoted into v2's gate; §5.1 makes it a disagreement source."
```

---

## Task 15: The §9.5 regression fixture

**Files:**
- Create: `crates/jparser/tests/fixtures/spurious_merges.txt`
- Modify: `crates/jparser/examples/boundary_diag.rs` (accept `--engine v2`)
- Test: `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Produces: a committed list of the surfaces handoff §9.5 identified, and a test that none of them is produced as a single matched segment by v2.

This is the gate condition spec §2.3 adds. The metric table cannot catch this class: compound-tolerant accuracy **credits** these merges, because they are matched segments whose endpoints are both gold boundaries.

- [ ] **Step 1: Regenerate the list from committed artefacts**

`examples/boundary_diag.rs` takes every gold boundary Vibrato has and jparser lacks and reports the covering span. Run it against v1 to reproduce handoff §9's decomposition, and read the `ordinary longer headword` bucket:

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"
cargo run --release -p jparser --features mecab --example boundary_diag -- `
  "$app\dict\gen-1" crates/jparser/tests/fixtures/gold.jsonl "$app\hints\current\system.dic"
```
Expected: the table of handoff §9 — 266 boundaries, 178 cost model, 52 lexical, 36 fallback.

- [ ] **Step 2: Write the fixture**

Create `crates/jparser/tests/fixtures/spurious_merges.txt`, one `surface<TAB>ent_seq<TAB>gold split` per line, with a header naming its provenance:

```
# Handoff §9.5's particle-absorbing merge class: jparser absorbing a preceding
# particle or fragment into the following word and matching a kana-unified
# headword that is semantically absurd. Vibrato has the boundary in every one.
#
# Regenerated from examples/boundary_diag.rs over tests/fixtures/gold.jsonl.
# Nothing here is captured text or ta-old expression: these are ordinary
# Japanese strings this project measured about its own engine.
#
# surface	ent_seq	what it actually is
はいた	1399760	は + いた
ももう	1567610	も + もう
はこ	1585650	は + この
がい	1204330	が + いった
になって	1599900	に + なる
してい	1308000	して + いた
どうし	1451380	どう + しました
ことし	1579130	こと + してる
```

Complete it from Step 1's output — handoff §9.5 lists ten example rows out of roughly 28. Add every row the run produces in that bucket, and record the final count in the commit message.

- [ ] **Step 3: Write the failing test**

```rust
#[test]
fn the_particle_absorbing_merges_are_gone() {
    // Handoff §9.5's defect class. The metric cannot catch this: the tolerant
    // accuracy *credits* these merges, because their endpoints are both gold
    // boundaries. Spec §2.3.
    const FIXTURE: &str = include_str!("fixtures/spurious_merges.txt");
    let dir = scratch("spurious");
    build_v2(&dir);

    let mut survivors = Vec::new();
    for line in FIXTURE.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut cols = line.split('\t');
        let surface = cols.next().expect("surface");
        let bounds = boundaries_over(surface);
        let r = parse_v2_with_bounds(&dir, surface, Some(&bounds));
        if r.segments.len() == 1 && r.segments[0].matched {
            survivors.push(surface.to_string());
        }
    }
    assert!(survivors.is_empty(), "still merged as one entry: {survivors:?}");
}
```

`#[cfg(feature = "mecab")]`: the whole class is defined by Vibrato having the boundary, so the test needs the tokenizer.

- [ ] **Step 4: Run the test to verify it fails against v1's behaviour and passes for v2**

Run: `cargo test -p jparser --features mecab --test engine_v2 spurious`
Expected: PASS. If any survive, the `PRONOUNCE_CROSS` term is too low — raise it in Task 16 against the corpus rather than special-casing the surfaces.

- [ ] **Step 5: Run the standing gates and commit**

```powershell
git add crates/jparser/tests crates/jparser/examples
git commit -m "test(engine-v2): pin the §9.5 merge class as a regression fixture

<N> surfaces, regenerated from boundary_diag over the committed gold
corpus. The metric cannot catch this class -- compound-tolerant accuracy
credits these merges, because their endpoints are both gold boundaries."
```

---

## Task 16: Measure, tune, and record the result

**Files:**
- Modify: `crates/jparser/src/engine/cost.rs` (the constants, with their comments updated to record what was measured)
- Modify: `crates/jparser/src/bin/jparser-cli/score.rs` (pass the real boundary source on the v2 path)
- Create: `docs/superpowers/2026-09-01-engine-v2-phase1a-handoff.md`

**Interfaces:** none new.

- [ ] **Step 1: Wire the real boundary source into `score`**

On the v2 path, `--hints <dic>` builds a `VibratoTokenizer` and calls `boundaries(text)` per sentence, passing it as `Some(&set)`. Without `--hints`, `bounds` is `None`. This is the hints-off / hints-on pair for v2.

- [ ] **Step 2: Build the v2 indexes**

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase1a"
New-Item -ItemType Directory -Force $scratch
cargo run --release -p jparser --bin jparser-cli -- build-index --engine v2 `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\gold-index-v2"
```

- [ ] **Step 3: Measure**

```powershell
$app  = "$env:APPDATA\dev.jparser.clipyomi"
$gold = "crates/jparser/tests/fixtures/gold.jsonl"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index-v2" $gold --engine v2
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index-v2" $gold --engine v2 --hints "$app\hints\current\system.dic"
```

Expected output shape, from Phase 0:

```
span accuracy:  0.____  (____ / 2163)
entry top-1:    0.____  (____ / ____)
  by decision:  e 0.____ (____ / 2163)
span tolerant:  0.____  (____ / 2163)
  coarsening:   ___ gold spans inside ___ dictionary merges
```

The gate, all eight cells:

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | ≥ 0.6810 | ≥ 0.8465 | ≥ 0.9159 | ≤ 167 |
| hints on | ≥ 0.6963 | ≥ 0.8678 | ≥ 0.9155 | ≤ 173 |

- [ ] **Step 4: Tune, one constant at a time**

If a cell misses, change **one** constant, re-run both hint states, and record the pair. Never quote strict alone: strict moving without tolerant moving means granularity changed, not correctness.

Rules while tuning:

- A rising merge count is a warning even when tolerant accuracy rises with it (handoff §9.6).
- Do not special-case a surface. If `PRONOUNCE_CROSS` cannot separate the §9.5 class from `音がする` and `東京都`, that is the finding — record it and say the term does not work, rather than making it work on the eight rows the fixture names.
- If the hard interior constraint is costing more than spec §7.5's measured 46 boundaries, master plan §4.3's softening clause applies: the constraint becomes a large finite penalty. That is a parameter change, and it must be recorded with the number that justified it.
- Update each constant's doc comment with what was measured, not just what was predicted.

- [ ] **Step 5: Run the equivalence check**

```powershell
cargo run --release -p jparser --bin jparser-cli -- build-index --engine v2 `
  <path-to-JMdict_e.xml> "$scratch\full-index-v2"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\full-index-v2" $gold --engine v2 --json > "$scratch\v2_full_off.json"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index-v2" $gold --engine v2 --json > "$scratch\v2_subset_off.json"
```

Repeat with `--hints`. Every field but `index_generation` must match across each pair. If they differ, **that difference is the finding** — the offline gate is not met; do not adjust anything to make it match.

- [ ] **Step 6: Write the handoff**

`docs/superpowers/2026-09-01-engine-v2-phase1a-handoff.md`, following the Phase 0 handoff's shape: what exists, the measured table beside the baseline, what each constant ended at and why, what is still open, and what Phase 1b needs. State plainly whether every gate cell passed, and if one did not, say which and by how much.

- [ ] **Step 7: Commit**

```powershell
git add crates/jparser/src/engine/cost.rs crates/jparser/src/bin/jparser-cli docs/superpowers
git commit -m "feat(engine-v2): measure Phase 1a against the Phase 0 baseline

<strict> strict / <tolerant> tolerant hints-off, <strict> / <tolerant>
hints-on, entry top-1 <off> / <on>, coarsening <n> spans in <m> merges.
Baseline 0.6810 / 0.8465 and 0.6963 / 0.8678, entry top-1 0.9159 /
0.9155, 167 and 173 merges.

Each cost constant's comment now records what was measured, not only
what was predicted."
```

---

## Task 17: Fold the split into the master plan

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`

Spec §12 owes these, and they are due with Phase 1a rather than after it.

- [ ] **Step 1: Split §7 Phase 1 into Phase 1a and Phase 1b**

Each with its own **In** / **Out** / **Done when**, and the size table restated per sub-phase from spec §9. Phase 1a's done-when is spec §10's four conditions. Phase 1b's is the formatter, its class × chain gate, and the switchover. Phase 1's own done-when is unchanged and is met at the end of 1b.

- [ ] **Step 2: Amend Phase 1's done-when to name both strings**

Current text: *"no `ta-old` reference remains under `crates/jparser/`"*. It passes a grep today while 24 files in that crate still carry the header `Japanese text parser ported from Translation Aggregator`. Amend to name both strings, over `crates/jparser/`, `xtask/`, and the two crates §3.2 calls clean.

- [ ] **Step 3: Correct §3.2's clean tier**

Its evidence column reads *"Zero `ta-old` references. Verified by grep"* for `crates/jmdict-source` and `crates/vibrato-source`. That grep was for the wrong string: 10 files across those two crates carry the port header. Record the correction; the tier itself does not change, because a boilerplate header is not protected expression.

- [ ] **Step 4: Amend §4.3**

Add spec §7.1 — the asymmetry that saves `東京都` leaves `はこう` a legal merge, so the constraint alone does not fix the section's own example — and spec §7.5, the 46 gold boundaries the hard constraint forbids, as the number the softening clause should be judged against.

- [ ] **Step 5: Amend §7.2**

Add spec §11.5: `hints` is optional and feature-gated with `--no-default-features` enforced, so the engine deinflects without a tokenizer and `stem.rs`'s work moves into the table rather than shrinking.

- [ ] **Step 6: Add the decision-log rows**

Copy the rows from spec §13 into §10, in the master plan's existing format.

- [ ] **Step 7: Commit**

```powershell
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
git commit -m "docs(engine-v2): fold the 1a/1b split into the master plan

§7 Phase 1 becomes 1a and 1b with separate done-whens; the done-when
names 'ported from Translation Aggregator' as well as 'ta-old', because
the existing clause passes a grep while 24 files in the crate still
declare themselves a port; §3.2's clean tier is corrected for the same
reason; §4.3 gains the two corrections and §7.2 the one."
```

---

## Self-Review

**Spec coverage.**

| Spec section | Task |
| --- | --- |
| §1 scope, §2 the split line | Task 17 (folded into the master plan); the Out constraints bind every task |
| §3 clean-room discipline | Global Constraints; Task 1 Step 1 verifies the precondition |
| §4 staging, no switchover in 1a | Tasks 1, 7 (the `--engine` selector); Global Constraints forbids the switchover |
| §5 two indexes, `prefixes_of_with` | Tasks 7, 8 |
| §6 parse-time deinflection | Tasks 4, 5 |
| §6.1 hints supply boundaries only | Task 1 (`Boundaries` carries positions, not lemmas); Task 8 never consults a hint for a candidate |
| §6.2 the table's source | Task 4 |
| §7 the cost model | Tasks 9, 10, 16 |
| §7.3 the PRONOUNCE term | Tasks 6 (the flag), 9 (the term), 15 (its gate) |
| §7.5 the constraint's price | Task 16 Step 4 |
| §8 the seam | Tasks 1 (`ConjStep`, `Entry::steps`), 11 (order and ranges), 12 (`conjugation: None`) |
| §9 the budget | implicit; Task 16's handoff records the actual |
| §10.1–10.3 the gate | Tasks 14, 15, 16 |
| §10.4 what is not a gate | Task 14 Step 5 |
| §10.6 test strategy | every task's TDD cycle |
| §11 findings | Task 17 |
| §12 master plan edits | Task 17 |

No gaps.

**Placeholder scan.** Three tasks leave a table for the implementer to complete rather than listing it in full: `JMDICT_POS_CLASS` (Task 4), `ATTACHES_TO` (Task 5), and `spurious_merges.txt` (Task 15). Each names its source, gives the anchor rows, and carries a mechanical check that fails if the completion is wrong — the POS test asserts every row maps to a class the generated table has, the chain tests fail if an auxiliary has no attachment row, and the fixture is regenerated from a committed diagnostic. That is a deliberate refusal to write Japanese strings from memory that must be right, not an unfinished plan.

**Type consistency.** `ClassId`/`FormId` are declared in `engine/mod.rs` (Task 1) and re-exported from `engine/table.rs` (Task 4). `Candidate`'s seven fields are fixed in Task 8 and used unchanged in Tasks 9, 10, 11, 12, 13. `Boundaries::is_boundary` has one signature throughout. `parse_v2` gains `table: &Table` in Task 5 and is not changed again. `Step` is defined in Task 10 and consumed in Tasks 12 and 14.

---

## Execution

Plan complete and saved to `docs/superpowers/plans/2026-09-01-engine-v2-phase1a.md`.
