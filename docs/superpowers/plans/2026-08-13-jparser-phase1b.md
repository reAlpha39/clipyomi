# JParser Phase 1B — Matcher, Segmenter & `parse()` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 1 of the JParser port by adding dictionary matching with
verb-conjugation recursion, the min-cost segmentation DP with ta-old's scoring
model, conjugation-label rendering, reading reconstruction, and the public
`parse()` surface — verifiable through a CLI that segments a sentence and through
insta snapshots over real vocabulary.

**Architecture:** Two new private modules sit between the Phase 1A index and a
single public entry point. `matcher.rs` turns one `Index::prefixes_of` walk at one
character position into `Match` values carrying a `(verb_type, tense, form, conj)`
chain; `segment.rs` runs a min-cost dynamic program over those matches, backtracks
into a contiguous cover of spans, and ranks every candidate aligning to each
chosen span. `lib.rs::parse` drives both, resolves each surviving match's entry
through the memory-mapped payload, and assembles the public `Segment`/`Entry`
tree. Nothing here opens a file, learns a directory path, or knows about Tauri.

**Tech Stack:** Rust 2021 (MSRV 1.75, toolchain 1.97.1), `fst`, `memmap2`,
`bincode`, `serde`, `thiserror`, `clap` (CLI), `insta` (snapshots, already a
dev-dependency), `cargo-llvm-cov` (coverage), `python3` stdlib (one-off JMdict
curation tool).

**Reference:** `docs/superpowers/specs/2026-08-13-jparser-phase1b-design.md`
(addendum, authoritative) and
`docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§5.1, §5.3, §5.4,
§5.5, §5.7, §10), with `docs/superpowers/2026-08-13-jparser-phase1a-handoff.md`
for the invariants Phase 1A leaves behind. The C++ original is in `ta-old/` and is
**read-only — never modify it**; every `Dictionary.cpp` / `JParseWindow.cpp` line
number in this plan is a citation, not an edit target.

---

## Global Constraints

- **License:** GPL v2. Every new source file gets the standard GPL v2 header
  comment — including `tools/extract_jmdict_subset.py`. The one exception is
  `tests/fixtures/jmdict_subset.xml`: third-party **data** under CC BY-SA 4.0,
  which carries the EDRDG notice instead and is not relicensed.
- **Crate purity:** `crates/jparser` must not depend on Tauri, any UI crate, or
  any HTTP client, and **no new dependency may be added**. `insta` is already a
  `[dev-dependencies]` entry; there is no `tempfile` crate and none is coming —
  temp dirs use the existing
  `std::env::temp_dir().join(format!("jparser-test-{name}"))` +
  `let _ = std::fs::remove_dir_all(&dir);` pattern from
  `tests/index_roundtrip.rs:20`.
- **Errors are explicit:** no `unwrap()` or `expect()` in library code outside
  tests. Every fallible path returns `Result` with a `thiserror` variant. Never
  silently skip data without counting it. Phase 1B has exactly one fallible
  operation — reading the mmap'd payload — and exactly one error type,
  `ParseError::Index`.
- **All offsets are CHAR offsets, never byte offsets.** `Match::start`,
  `Match::len`, `Match::src_len`, `Span::start`, `Segment::start`,
  `BoundaryHints::bad_start`/`bad_end`, and `PrefixHit::key_chars` are all counted
  in `char`s. The single `&str` → `Vec<char>` conversion in the whole phase is
  `text.chars().collect()` at the top of `parse`.
- **Immutability:** all public types are owned and immutable. `segment()` takes
  `&[Vec<Match>]` and must not mutate it; the stale-`COUNTER` clear operates on
  the span's clones (§6.4).
- **No magic numbers:** every threshold, cost, flag mask, and Unicode boundary is
  a named `const`. That includes the two space characters the counter lookbehind
  skips (`COUNTER_SKIPPED_SPACES`) and the kuruHack's three-character cap
  (`KURU_HACK_MAX_CHARS`).
- **File size:** 200–400 lines typical, **800 hard maximum including the
  `#[cfg(test)] mod tests` block**. Two size-driven splits are taken deliberately
  in this phase and flagged below; take no third one silently.
- **Naming:** types `PascalCase`, functions and variables `snake_case`, constants
  `UPPER_SNAKE_CASE`. One name per concept, frozen: `Match::chain`, `ConjLink`,
  `Match::inexact`, `Match::len`/`src_len`, `matches_at`,
  `render_conjugation_label`, `segment`, `sort_matches`, `parse`,
  `Segmentation::total_cost`, `Cell::cost`, `Cell::back_len`, `Span::matched`,
  `strict_eq`/`unified_eq`, `Entry::headword`, `Sense`.
- **Coverage target:** 80% line coverage on `crates/jparser`, measured by
  `cargo llvm-cov -p jparser --summary-only --fail-under-lines 80`. Phase 1A
  finished at 93.85%.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never
  `cargo fmt -p jparser`** — it reformats committed files this phase does not
  touch. Note that `crates/jparser/src/conjugation.rs` is currently *not*
  rustfmt-clean (`rustfmt --check` reports four hunks) while `record.rs`,
  `stem.rs` and `index/load.rs` are, so the crate's existing style is already
  inconsistent with what these steps produce; do not "fix" conjugation.rs here.

**Phase 1A invariants Phase 1B must not break** (handoff, "Invariants"):

- **No Phase 1B type stores the index directory path.** `parse` takes `&Index`;
  the generation-directory layout lands in Phase 2 without touching this code.
- **`kana::strip_suffix_unified` is shared** by conjugation-chain resolution
  (`conjugation.rs`) and stem generation (`stem.rs`). `reconstruct_reading`'s
  `strip_remove_suffix` must use the *same* expression, or a kana stem exists at
  parse time that did not exist at build time (or vice versa).
- **A stem carries exactly one verb type.** `StoredRecord::verb_type` is
  `Option<VerbTypeId>`; `Some` means "generated stem", `None` means "plain
  headword". `Match::chain` being empty is the port's encoding of ta-old's
  `conj[0].verbType == 0`.
- **`kana::unify` is character-wise and therefore prefix-stable**; the FST walk
  and the Python curation tool both depend on it.
- **Duplicate type names are deliberate** (`vk`, `vs`, `v5r-i`, `v5uru`): one twin
  carries kanji-form suffixes, the other kana-form. `types_named` returns all
  ids and every caller must handle more than one — that pairing *is* the kuruHack.
- **Fixed tense discriminants** `Remove = 0`, `NonPast = 1`, `Stem = 2`,
  `Potential = 3` must not be reordered.

**One Phase 1A data-shape change is permitted and required** (contract §2):
`EntryData` gains a `readings` field and `INDEX_FORMAT_VERSION` goes 2 → 3.
Task 7 does it. Beyond that, `index/mod.rs`, `index/build.rs`, and the CLI's new
`parse` subcommand are the only Phase 1A files this phase may modify.

---

## File Structure

| File | Responsibility |
|---|---|
| `ta/crates/jparser/src/matcher.rs` | `ConjLink`, `Match`, `same_except_inexact`, `strict_eq`, `unified_eq`, `commit`, `matches_at`, `render_conjugation_label` |
| `ta/crates/jparser/src/matcher/verb.rs` | `recurse` — `FindVerbMatches`' five conjugation-chaining rules, and the nine tests that pin them |
| `ta/crates/jparser/src/segment.rs` | `BoundaryHints`, `Segmentation`, `Span`, `Cell`, `segment`, `score_match`, `backtrack`, `counter_after_number`, `isolated_katakana_run`, `clear_stale_counter_flags`, all 13 scoring constants |
| `ta/crates/jparser/src/rank.rs` | `sort_matches`, `group_key`, `verb_plain_collapses`, `RANK_FLAG_MASK` |
| `ta/crates/jparser/src/lib.rs` | `parse`, `ParseOptions`, `ParseResult`, `Segment`, `Entry`, `ParseError`, the `Sense`/`BoundaryHints` re-exports, and the entry-assembly helpers (`entry_data`, `assemble_entry`, `dictionary_form`, `reconstruct_reading`, `strip_remove_suffix`, `kuru_hack`, `tails_match`) |
| `ta/crates/jparser/src/index/mod.rs` | *(modified)* `EntryData.readings`, `INDEX_FORMAT_VERSION = 3` |
| `ta/crates/jparser/src/index/build.rs` | *(modified)* populate `EntryData.readings` |
| `ta/crates/jparser/src/bin/jparser-cli.rs` | *(modified)* the `parse` subcommand |
| `ta/crates/jparser/tests/fixtures/jmdict_matcher.xml` | 8-entry hand-written fixture for the matcher's golden tests |
| `ta/crates/jparser/tests/parse_irregular.rs` | End-to-end `parse` regression for 来る (empty stem + kuruHack) |
| `ta/crates/jparser/tests/cli_parse.rs` | `jparser-cli parse` byte-exact output test |
| `ta/crates/jparser/tests/parse_snapshots.rs` | insta snapshots over 30 sentences, plus two targeted irregular-verb assertions |
| `ta/crates/jparser/tests/fixtures/parse_sentences.txt` | The snapshot corpus |
| `ta/crates/jparser/tests/fixtures/jmdict_subset.xml` | Generated, committed curated JMdict subset (CC BY-SA 4.0) |
| `ta/crates/jparser/tests/fixtures/README.md` | Fixture provenance and EDRDG attribution |
| `ta/crates/jparser/tests/snapshots/parse_snapshots__sentences.snap` | Generated by insta, committed |
| `ta/tools/extract_jmdict_subset.py` | One-off curation tool, stdlib `python3` only |

**Rationale for the splits, including two flagged deviations from the frozen
contract's §7 module map.** The contract maps every matcher item to
`src/matcher.rs` and `sort_matches` to `src/segment.rs`. Neither fits under the
800-line cap once §4.1's in-module tests are counted — `pub(crate)` items are
unreachable from `tests/`, so every matcher, DP, and ranking test *must* live
inside its own file. Measured on assembled builds of this plan's own code blocks,
rustfmt applied:

- a single `matcher.rs` holding Tasks 1–3 is **1033 lines** (376 impl / 657 test),
  and is already at **879** at the end of Task 2. Pulling only the label renderer
  out removes ~154 and leaves 879 — still over. So the recursion and its nine
  JSON-fixture tests move to `src/matcher/verb.rs` (a child module, so it keeps
  access to `matcher`'s private `strict_eq`), leaving `matcher.rs` at ~625 and
  `verb.rs` at ~435. The label renderer stays in `matcher.rs`, because the
  contract explicitly says "`label.rs` and `entry.rs` do not exist".
- a single `segment.rs` holding Tasks 4–6 is **≈990 lines**, derived as
  740 + 272 − 1 (`use crate::rank::sort_matches;`) − 21 (duplicated GPL header,
  module doc, `use` block, and test-module scaffolding). So `sort_matches` and its
  three helpers move to `src/rank.rs` (272 lines), leaving `segment.rs` at ~765.

Both keep the contract's names, signatures, visibility, and behaviour; only the
file changes. **This plan supersedes contract §7's module-map rows for
`recurse`/`matches_at`-internals and for `sort_matches`, and its test-home rows
for the matcher tests, `sort_matches`, and `tests/parse_irregular.rs`.** The
table above is authoritative. The alternative — deleting required tests to fit —
is worse. Take no further split without flagging it the same way.

`lib.rs` finishes at ≈650 lines (≈370 non-test), over contract §7's "~400 lines →
split to `entry.rs`" advisory but well under the 800 hard cap, and the contract's
own test-home table puts the reading-reconstruction and kuruHack tests in
`lib.rs`'s `mod tests`, which only makes sense if the code under test lives there.
The split is **not** taken; headroom afterwards is ~150 lines, which the next task
to touch `lib.rs` needs to know.

---

## Scope

Explicitly **out** of Phase 1B, with the phase that owns each (addendum §1):

| Item | Phase |
|---|---|
| Vibrato / `morph.rs` — the real `BoundaryHints` implementation | 5 |
| Furigana display modes, and the `to_katakana` `>= 0x3097` bail conflict | 3 |
| Differential run against ta-old (**not** Phase 1B — this corrects both the Phase 1A plan and the Phase 1A handoff) | 6 |
| Tauri shell, clipboard, `ensure_dictionary` | 2 |
| Generation-directory index layout | 2 |
| JMnedict — `NAME_DICT_*` and `IS_NAME` are implemented but dormant; nothing sets the flag in v1 | deferred |
| Half-width katakana offset map | deferred |

Also out, and named so nobody implements them by accident: ta-old's
`#ifdef SETSUMI_CHANGES` `score = -999999` override for `JAP_WORD_TOP`
(`Dictionary.cpp:1237-1244`) and its commented-out `JAP_WORD_PRIMARY` bonus
(`:1246-1252`). `SETSUMI_CHANGES` is defined nowhere in ta-old; `WordFlags::TOP`
stays reserved and unread.

Phase 1B ships `BoundaryHints` as a trait plus test stubs only. `None` hints must
behave exactly like an implementation returning `false` everywhere, and Task 5 has
a test asserting that equality.

---

## Task 1: `Match`, and the non-verb path of `matches_at`

**Files:**
- Create: `ta/crates/jparser/tests/fixtures/jmdict_matcher.xml`
- Create: `ta/crates/jparser/src/matcher.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes:
  - `jparser::index::load::{Index, PrefixHit}` — `Index::prefixes_of(&self, text: &str) -> Result<Vec<PrefixHit>, IndexError>`
  - `jparser::index::StoredRecord { surface: String, flags: u16, verb_type: Option<VerbTypeId>, entry_id: u32 }`
  - `jparser::record::WordFlags` — `WordFlags(u16)`, `contains`, `insert`, `WordFlags::IS_NAME`
  - `jparser::conjugation::{ConjugationTable, Form, TenseId, VerbTypeId}`
  - `jparser::index::build::build_from_reader`, `jparser::stem::StemOptions` (tests only)
- Produces:
  - `pub(crate) struct matcher::ConjLink { pub(crate) verb_type: VerbTypeId, pub(crate) tense: TenseId, pub(crate) form: Form, pub(crate) conj: usize }`
  - `pub(crate) struct matcher::Match { pub(crate) start: usize, pub(crate) len: usize, pub(crate) src_len: usize, pub(crate) surface: String, pub(crate) flags: WordFlags, pub(crate) entry_id: u32, pub(crate) inexact: bool, pub(crate) chain: Vec<ConjLink> }`
  - `pub(crate) fn matcher::same_except_inexact(a: &Match, b: &Match) -> bool`
  - `pub(crate) fn matcher::matches_at(index: &Index, table: &ConjugationTable, text: &[char], i: usize) -> Result<Vec<Match>, ParseError>` — **non-verb records only**; Task 2 fills the verb arm
  - `fn matcher::strict_eq(a: &[char], b: &str) -> bool` (private)
  - `fn matcher::commit(out: &mut Vec<Match>, candidate: Match)` (private)
  - `pub enum ParseError { Index(#[from] IndexError) }` in `lib.rs`

**Resolved gaps** (the contract names none of these; the choices are the obvious
ones and are frozen here for Tasks 2–9):

1. **`ParseError` must exist before `matches_at` compiles.** Contract §7 homes it
   in `lib.rs`, which belongs to Task 7. Task 1 creates it verbatim as §3.3
   specifies; **Task 7 verifies rather than redefines it.**
2. **The dead-code window.** Nothing outside the module's own tests calls the
   matcher until `parse` lands, so Task 1 registers it as
   `#[allow(dead_code)] mod matcher;`. Task 7 removes the attribute. The attribute
   covers the child module `matcher::verb` too.
3. **`strict_eq` / `unified_eq` signatures.** `fn strict_eq(a: &[char], b: &str) -> bool`
   and `pub(crate) fn unified_eq(a: &[char], b: &str) -> bool`. `&[char]` on the
   left because every offset here is a char offset; `&str` on the right because
   that is what `StoredRecord::surface` and `Conjugation::suffix` already are.
   `unified_eq` is `pub(crate)` because Task 7's `tails_match` is a caller outside
   `matcher`; `strict_eq` stays private (only `matcher` and its `verb` child use
   it).
4. **The recursion's signature.**
   `fn recurse(table, text, start, slen, vtype, chain: &[ConjLink], inexact) -> Vec<Match>`,
   `pub(super)` in `matcher/verb.rs`, with **`depth == chain.len()`** as the
   invariant replacing ta-old's separate `depth` parameter.
5. **The three post-filters** are `fn commit(out: &mut Vec<Match>, candidate: Match)`,
   applying zero-length drop → duplicate collapse → names-inexact suppression in
   ta-old's own order (`Dictionary.cpp:882-894`).
6. **What a `Match` holds in flight.** The recursion returns matches whose
   `src_len`, `surface`, `flags`, `entry_id` are documented placeholders
   (`0`, `String::new()`, `WordFlags::default()`, `0`) that `matches_at` stamps
   from the `StoredRecord` — mirroring ta-old, where `FindVerbMatches` created the
   slot and `FindMatches` filled those fields afterwards
   (`Dictionary.cpp:900-927`).
7. **Only four `TenseId`s are named consts.** Tests resolve `Past`, `Te-form`, and
   an unknown id by name through `ConjugationTable::tense_name`, never by literal.
8. **Fixture size.** Contract §7 says "~20-entry `jmdict_matcher.xml`". This task
   writes **8 entries** — exactly what Tasks 1–3 assert against. An unused entry is
   an unverified entry.
9. **`inexact` is seeded, not OR-ed.** Contract §6.1 says both "run the recursion
   with `inexact` as computed" and "its `inexact` OR-ed with the record's".
   `inexact2 = inexact || …` is monotonic, so seeding the recursion makes the later
   OR a no-op; only the seed is implemented.

**Two risks, flagged rather than papered over.** First, the duplicate-collapse
filter is **unreachable through an index built by `build_from_reader`**:
`index/build.rs::push` already drops byte-identical `StoredRecord`s inside a
bucket, and `same_except_inexact` compares `surface`, `chain` and `entry_id`, so
two records differing only in kana spelling never compare equal. It is implemented
because the contract mandates it and JMnedict may change the premise, and it is
tested at the `commit` level with hand-built `Match` values — no fixture can
produce an end-to-end case. Second, `strict_eq` is pinned to **ASCII** case
folding (§6.1); ta-old's `wcsnicmp` was Win32 `CompareStringW` with
`NORM_IGNORECASE`, which also folds non-ASCII case. Japanese text never hits the
difference; a mixed-script line could, and Phase 6's differential run is where
that would surface.

ta-old walked a per-dictionary array sorted by `wcsijcmp`, once per prefix length,
to find every headword prefixing the text at this position
(`Dictionary.cpp:807-944`). Phase 1A's FST walk already returns exactly that set.
What is left is the part the index cannot precompute: whether the text the user
actually typed spells the headword the way the dictionary does.

- [ ] **Step 1: Create the matcher fixture**

`ta/crates/jparser/tests/fixtures/jmdict_matcher.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
<!ENTITY prt "particle">
<!ENTITY v1 "Ichidan verb">
<!ENTITY vs-i "suru verb - irregular">
]>
<JMdict>
<!-- 2000010/2000020: the same word spelled hiragana in one entry and katakana
     in another. Both land in the FST bucket "ネコ", so a hiragana query gets
     one exact and one inexact record at the same length. -->
<entry>
<ent_seq>2000010</ent_seq>
<k_ele><keb>猫</keb></k_ele>
<r_ele><reb>ねこ</reb></r_ele>
<sense><pos>&n;</pos><gloss>cat</gloss></sense>
</entry>
<entry>
<ent_seq>2000020</ent_seq>
<r_ele><reb>ネコ</reb></r_ele>
<sense><pos>&n;</pos><gloss>cat (zoological)</gloss></sense>
</entry>
<!-- 2000030: a one-character particle, for the flag round trip. -->
<entry>
<ent_seq>2000030</ent_seq>
<r_ele><reb>は</reb></r_ele>
<sense><pos>&prt;</pos><gloss>topic marker</gloss></sense>
</entry>
<!-- 2000040/2000050: two distinct entries spelled identically. They must never
     be collapsed into one match. -->
<entry>
<ent_seq>2000040</ent_seq>
<k_ele><keb>二</keb></k_ele>
<r_ele><reb>に</reb></r_ele>
<sense><pos>&n;</pos><gloss>two</gloss></sense>
</entry>
<entry>
<ent_seq>2000050</ent_seq>
<k_ele><keb>二</keb></k_ele>
<r_ele><reb>ふた</reb></r_ele>
<sense><pos>&n;</pos><gloss>two (prefixed to counters)</gloss></sense>
</entry>
<!-- 2000060: a longer headword starting with 2000010's, so one position yields
     two prefix lengths. -->
<entry>
<ent_seq>2000060</ent_seq>
<k_ele><keb>猫舌</keb></k_ele>
<r_ele><reb>ねこじた</reb></r_ele>
<sense><pos>&n;</pos><gloss>tongue that cannot handle hot food</gloss></sense>
</entry>
<!-- 2000070: a v1 verb, for the conjugation recursion in Task 2. Its stem 食べ
     is indexed under the key 食ベ. -->
<entry>
<ent_seq>2000070</ent_seq>
<k_ele><keb>食べる</keb></k_ele>
<r_ele><reb>たべる</reb></r_ele>
<sense><pos>&v1;</pos><gloss>to eat</gloss></sense>
</entry>
<!-- 2000080: vs-i, whose remove-suffix is the whole word, so its stem is the
     empty string. Every prefixes_of() call on this index therefore returns a
     key_chars == 0 hit; the matcher must survive it. -->
<entry>
<ent_seq>2000080</ent_seq>
<r_ele><reb>する</reb></r_ele>
<sense><pos>&vs-i;</pos><gloss>to do</gloss></sense>
</entry>
</JMdict>
```

- [ ] **Step 2: Write the failing tests**

Create `ta/crates/jparser/src/matcher.rs` containing only this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::build::build_from_reader;
    use crate::index::load::Index;
    use crate::record::WordFlags;
    use crate::stem::StemOptions;

    const FIXTURE: &str = include_str!("../tests/fixtures/jmdict_matcher.xml");

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("embedded asset must load")
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// Build the matcher fixture into its own directory and open it. Mirrors
    /// `tests/index_roundtrip.rs`: no `tempfile` dependency, and one directory
    /// per test so a parallel test can never write into a live mmap.
    fn index(name: &str) -> Index {
        let dir = std::env::temp_dir().join(format!("jparser-matcher-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        build_from_reader(
            std::io::Cursor::new(FIXTURE),
            &table(),
            &StemOptions::default(),
            &dir,
        )
        .expect("fixture must build");
        Index::open(&dir).expect("fixture index must open")
    }

    /// A `Match` with every field at a neutral value, for the `commit` and
    /// `same_except_inexact` unit tests.
    fn plain(entry_id: u32, inexact: bool) -> Match {
        Match {
            start: 0,
            len: 1,
            src_len: 1,
            surface: "猫".to_string(),
            flags: WordFlags::PRIMARY,
            entry_id,
            inexact,
            chain: Vec::new(),
        }
    }

    #[test]
    fn a_non_verb_record_yields_one_match_spanning_its_key() {
        // 猫 is a one-character noun; だ is not part of any key, so exactly one
        // record hits at this position.
        let idx = index("nonverb");
        let text = chars("猫だ");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].start, 0);
        assert_eq!(got[0].len, 1);
        assert_eq!(got[0].src_len, 1, "len == src_len is a non-verb invariant");
        assert_eq!(got[0].surface, "猫");
        assert_eq!(got[0].entry_id, 2000010);
        assert!(!got[0].inexact);
        assert!(got[0].chain.is_empty(), "a non-verb match has no chain");
    }

    #[test]
    fn kana_type_disagreement_marks_a_match_inexact() {
        // Both ねこ (entry 2000010) and ネコ (entry 2000020) normalize to the
        // key ネコ, so a hiragana query returns both at key_chars 2: the
        // hiragana spelling exactly, the katakana spelling inexactly.
        let idx = index("inexact");
        let text = chars("ねこ");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let two: Vec<&Match> = got.iter().filter(|m| m.len == 2).collect();
        assert_eq!(two.len(), 2, "got {got:#?}");
        let exact = two.iter().find(|m| m.surface == "ねこ").expect("hiragana record");
        let fuzzy = two.iter().find(|m| m.surface == "ネコ").expect("katakana record");
        assert!(!exact.inexact);
        assert!(fuzzy.inexact, "katakana spelling of a hiragana query is inexact");
        assert_eq!(exact.entry_id, 2000010);
        assert_eq!(fuzzy.entry_id, 2000020);
    }

    #[test]
    fn strict_eq_folds_ascii_case_but_not_kana_type() {
        // ta-old's wcsnicmp: NORM_IGNORECASE only. The kana-insensitive half of
        // the comparison is the FST walk's job, not this function's.
        assert!(strict_eq(&chars("abc"), "ABC"));
        assert!(strict_eq(&chars("ねこ"), "ねこ"));
        assert!(!strict_eq(&chars("ねこ"), "ネコ"));
        assert!(!strict_eq(&chars("ねこ"), "ねこだ"));
        assert!(!strict_eq(&chars("ねこだ"), "ねこ"));
    }

    #[test]
    fn one_position_yields_one_match_per_distinct_key_length() {
        // 猫 and 猫舌 are both keys and both prefix the text.
        let idx = index("lengths");
        let text = chars("猫舌");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let mut lengths: Vec<usize> = got.iter().map(|m| m.len).collect();
        lengths.sort_unstable();
        assert_eq!(lengths, vec![1, 2], "got {got:#?}");
    }

    #[test]
    fn distinct_entries_sharing_a_surface_both_survive() {
        // Entries 2000040 and 2000050 are both spelled 二. ta-old's dedupe keyed
        // on entry identity, never on spelling, so both must appear.
        let idx = index("homograph");
        let text = chars("二");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let mut ids: Vec<u32> = got.iter().map(|m| m.entry_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![2000040, 2000050], "got {got:#?}");
    }

    #[test]
    fn word_flags_are_carried_through_from_the_record() {
        let idx = index("flags");
        let text = chars("は");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let particle = got.iter().find(|m| m.surface == "は").expect("は must match");
        assert!(particle.flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn start_is_stamped_with_the_queried_position() {
        // ta-old set start to 0 inside FindMatches and stamped the real offset
        // in FindAllMatches; matches_at stamps it directly.
        let idx = index("start");
        let text = chars("猫だ猫");
        let got = matches_at(&idx, &table(), &text, 2).expect("matcher must not fail");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].start, 2);
        assert_eq!(got[0].len, 1);
    }

    #[test]
    fn text_with_no_indexed_prefix_yields_nothing() {
        let idx = index("miss");
        let text = chars("zzz");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert!(got.is_empty(), "got {got:#?}");
    }

    #[test]
    fn the_empty_key_hit_never_produces_a_zero_length_match() {
        // する/vs-i has an empty stem, so prefixes_of returns a key_chars == 0
        // hit on every call against this index. A zero-length match would be a
        // self-loop in the DP.
        let idx = index("emptykey");
        let text = chars("する");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert!(got.iter().all(|m| m.len > 0), "got {got:#?}");
        assert!(
            got.iter().any(|m| m.len == 2 && m.chain.is_empty()),
            "the plain する headword must still match: {got:#?}"
        );
    }

    #[test]
    fn commit_drops_a_zero_length_candidate() {
        let mut out = Vec::new();
        let mut zero = plain(1, false);
        zero.len = 0;
        commit(&mut out, zero);
        assert!(out.is_empty());
    }

    #[test]
    fn commit_collapses_a_duplicate_and_lets_exact_win() {
        // ta-old Dictionary.cpp:882-893: the committed copy is forced exact and
        // the newcomer is dropped.
        let mut out = Vec::new();
        commit(&mut out, plain(1, true));
        commit(&mut out, plain(1, false));
        assert_eq!(out.len(), 1);
        assert!(!out[0].inexact, "an exact duplicate clears the committed flag");
    }

    #[test]
    fn commit_suppresses_an_inexact_name_match_entirely() {
        // ta-old Dictionary.cpp:894. Dormant in v1 — nothing sets IS_NAME.
        let mut named = plain(1, true);
        named.flags.insert(WordFlags::IS_NAME);
        let mut out = Vec::new();
        commit(&mut out, named.clone());
        assert!(out.is_empty(), "an inexact name hit is dropped, not ranked down");

        named.inexact = false;
        commit(&mut out, named);
        assert_eq!(out.len(), 1, "an exact name hit is kept");
    }

    #[test]
    fn same_except_inexact_ignores_exactly_one_field() {
        assert!(same_except_inexact(&plain(1, true), &plain(1, false)));
        assert!(!same_except_inexact(&plain(1, false), &plain(2, false)));
        let mut longer = plain(1, false);
        longer.len = 2;
        assert!(!same_except_inexact(&plain(1, false), &longer));
    }
}
```

- [ ] **Step 3: Register the module and the error type**

In `ta/crates/jparser/src/lib.rs`, add the module declaration alongside the
existing ones and the error type below them:

```rust
// Dead until `parse` lands: nothing outside the module's own tests calls the
// matcher yet. Task 7 removes this attribute. It covers the child module
// `matcher::verb` that Task 2 adds, too.
#[allow(dead_code)]
mod matcher;
```

```rust
/// Everything `parse` can fail at. Reading the memory-mapped index payload is
/// the only fallible step in Phase 1B; the enum exists so `parse` does not
/// leak `IndexError` into its public signature, and so variants can be added
/// without a breaking change.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("reading the index failed: {0}")]
    Index(#[from] crate::index::IndexError),
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser matcher`
Expected: FAIL — `cannot find type 'Match' in this scope`, `cannot find type
'ConjugationTable' in this scope` (the test module's `table()` helper resolves it
only through `use super::*`, which imports nothing yet), plus `cannot find
function 'matches_at' in this scope`, `cannot find function 'strict_eq' in this
scope`, `cannot find function 'commit' in this scope`, and `cannot find function
'same_except_inexact' in this scope`.

- [ ] **Step 5: Implement the types and the non-verb path**

Insert above the test module in `ta/crates/jparser/src/matcher.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Dictionary matching at one position.
//!
//! Ports ta-old's `FindMatches` (`ta-old/exe/util/Dictionary.cpp:807`). ta-old
//! searched a per-dictionary array of headwords sorted by a kana-insensitive
//! comparator, once per prefix length; Phase 1A replaced that with a single FST
//! walk (`Index::prefixes_of`), so this module only interprets what the walk
//! returns.
//!
//! Two comparisons are in play and they are not the same one:
//!
//! * the **loose** comparison (ta-old's `wcsnijcmp`) folds kana type, width and
//!   ASCII case. The FST walk already applies it to the key.
//! * the **strict** comparison (ta-old's `wcsnicmp`) folds ASCII case only. A
//!   hit that passes the loose test and fails the strict one is *inexact*: the
//!   user typed katakana where the dictionary spells hiragana. Inexactness is
//!   scored, not rejected, and depends on what was typed, so it cannot be
//!   precomputed into the index.

use crate::conjugation::{ConjugationTable, Form, TenseId, VerbTypeId};
use crate::index::load::Index;
use crate::record::WordFlags;
use crate::ParseError;

/// One conjugation layer of a match, ta-old's `ConjInfo`
/// (`ta-old/exe/util/Dictionary.h:45`). The index in `Match::chain` is ta-old's
/// `depth`: index 0 is the layer applied directly to the dictionary stem — the
/// first suffix consumed, leftmost in the text — and increasing indices move
/// outward toward the end of the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConjLink {
    /// 0-based index into `ConjugationTable::types()`. ta-old stored this
    /// 1-based with 0 meaning "not a verb"; here "not a verb" is an empty
    /// `chain`, and Phase 6's differential run adds 1.
    pub(crate) verb_type: VerbTypeId,
    pub(crate) tense: TenseId,
    pub(crate) form: Form,
    /// Index into `types()[verb_type].conjugations`, needed to recover the
    /// suffix for the kuruHack twin search.
    pub(crate) conj: usize,
}

/// One dictionary hit at one position, ta-old's `Match`
/// (`ta-old/exe/util/Dictionary.h:72`).
///
/// There is deliberately no `dict_index` and no `first_jstring`: the port has
/// one dictionary, and `entry_id` carries both identities ta-old split across
/// those two fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    /// Char offset into the parsed text.
    pub(crate) start: usize,
    /// Total matched length in **chars**: the dictionary key plus every
    /// conjugation suffix consumed along `chain`.
    pub(crate) len: usize,
    /// Length in **chars** of the dictionary key alone — the
    /// `PrefixHit::key_chars` this match came from. Equal to `len` for a
    /// non-verb match.
    pub(crate) src_len: usize,
    /// `StoredRecord::surface` verbatim: the stem for a verb record, the
    /// headword for a plain one. Never normalized.
    pub(crate) surface: String,
    pub(crate) flags: WordFlags,
    pub(crate) entry_id: u32,
    /// The source text and the dictionary spelling disagree in kana type,
    /// width, or **non-ASCII** case — `strict_eq` folds ASCII case, so an
    /// ASCII case disagreement alone is still exact. ta-old's `inexactMatch`,
    /// narrowed from its tri-state `int`: the sign only ever reflected
    /// alphabetical order, not quality.
    pub(crate) inexact: bool,
    /// Empty for a non-verb match (ta-old's `conj[0].verbType == 0`). Never
    /// longer than `MAX_CONJ_DEPTH`.
    pub(crate) chain: Vec<ConjLink>,
}

/// Field-by-field equality over every field except `inexact`. Replaces ta-old's
/// `memcmp(a, b, sizeof(Match) - sizeof(int))` (`Dictionary.cpp:885`, `:1046`),
/// which relied on `inexactMatch` being the struct's last field and compared
/// padding bytes as a side effect.
pub(crate) fn same_except_inexact(a: &Match, b: &Match) -> bool {
    a.start == b.start
        && a.len == b.len
        && a.src_len == b.src_len
        && a.surface == b.surface
        && a.flags == b.flags
        && a.entry_id == b.entry_id
        && a.chain == b.chain
}

/// ta-old's `wcsnicmp` (`Shared/Shrink.h:124`): ASCII-case-insensitive, kana
/// type and width **sensitive**. Used only to decide inexactness — the loose
/// comparison is already guaranteed by the FST walk.
fn strict_eq(a: &[char], b: &str) -> bool {
    let mut rhs = b.chars();
    for &lhs in a {
        let Some(other) = rhs.next() else { return false };
        // `eq_ignore_ascii_case`, not a `to_ascii_lowercase` pair: clippy's
        // `manual_ignore_case_cmp` is a hard error under `-D warnings`, and
        // the two are semantically identical (ASCII folding only, kana and
        // width untouched).
        if !lhs.eq_ignore_ascii_case(&other) {
            return false;
        }
    }
    rhs.next().is_none()
}

/// Commit one candidate, applying ta-old's post-match filters in its own order
/// (`Dictionary.cpp:882-894`).
fn commit(out: &mut Vec<Match>, candidate: Match) {
    // Zero-length drop. An empty stem meeting an empty trimmed suffix produces
    // a match covering no text. ta-old never collected one — its cheapest
    // possible match delta is 10 - 2 - 3 - 2 = +3 > 0, so the DP could never
    // choose it — and allowing one would be a self-loop in the DP.
    if candidate.len == 0 {
        return;
    }
    // Same-entry duplicate collapse. Exact wins: the committed copy loses its
    // inexact flag, and the newcomer is dropped either way. Distinct entry ids
    // never compare equal, so two homographs are never merged.
    if let Some(existing) = out.iter_mut().find(|m| same_except_inexact(m, &candidate)) {
        if !candidate.inexact {
            existing.inexact = false;
        }
        return;
    }
    // Names-inexact suppression: an inexact hit from a names source is
    // discarded outright, not merely ranked lower. Dormant in v1 — nothing sets
    // IS_NAME — but implemented so JMnedict needs no matcher change.
    if candidate.inexact && candidate.flags.contains(WordFlags::IS_NAME) {
        return;
    }
    out.push(candidate);
}

/// Every dictionary match starting at char offset `i`.
///
/// Emission order is load-bearing — the DP's match relaxation keeps the *last*
/// candidate on a tie and the final rank sort is stable — so it is fixed as:
/// ascending `key_chars`, then records in stored order, then (for a verb
/// record) the recursion's own order.
pub(crate) fn matches_at(
    index: &Index,
    table: &ConjugationTable,
    text: &[char],
    i: usize,
) -> Result<Vec<Match>, ParseError> {
    // ponytail: O(n^2) tail allocation; pass a char→byte offset table and slice
    // the original &str if a 10k-char input ever measures slow.
    let tail: String = text[i..].iter().collect();
    let mut out: Vec<Match> = Vec::new();

    for hit in index.prefixes_of(&tail)? {
        let k = hit.key_chars;
        // `key_chars` counts chars of the query, so this slice always exists;
        // taking it fallibly costs one line and removes a panic path.
        let Some(source) = text.get(i..i + k) else { continue };
        for record in hit.records {
            let inexact = !strict_eq(source, &record.surface);
            match record.verb_type {
                None => commit(
                    &mut out,
                    Match {
                        start: i,
                        len: k,
                        src_len: k,
                        surface: record.surface,
                        flags: WordFlags(record.flags),
                        entry_id: record.entry_id,
                        inexact,
                        chain: Vec::new(),
                    },
                ),
                Some(vtype) => {
                    // The conjugation recursion lands in the next task; until
                    // then a verb record contributes no matches.
                    let _ = (table, vtype);
                }
            }
        }
    }
    Ok(out)
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser matcher`
Expected: PASS, 13 tests.

- [ ] **Step 7: Format, lint, and check the file size**

Run: `cd ta && rustfmt --edition 2021 crates/jparser/src/matcher.rs && cargo clippy -p jparser --all-targets -- -D warnings && wc -l crates/jparser/src/matcher.rs`

Expected: clippy reports no warnings, and roughly **411** lines (188
implementation, 223 tests). rustfmt **does** rewrite the file — it expands
`let … else { … };` one-liners and inline struct literals — so do not treat a
non-empty diff as a problem; treat a clippy warning as one. Do **not** run
`cargo fmt -p jparser`: it reformats unrelated committed files, and
`conjugation.rs` is not currently rustfmt-clean.

- [ ] **Step 8: Commit**

```bash
cd ta && git add crates/jparser/src/matcher.rs crates/jparser/src/lib.rs \
  crates/jparser/tests/fixtures/jmdict_matcher.xml
git commit -m "feat: match non-verb dictionary records at a position

Ports the non-verb branch of ta-old's FindMatches. The FST walk from
Phase 1A already returns every headword that prefixes the text here,
so this adds only what the index cannot precompute: whether the text
as typed spells the headword the way the dictionary does. That is
ta-old's second, stricter comparison (wcsnicmp, ASCII case only)
layered on top of the kana-insensitive one the FST key already
applied.

Match drops ta-old's dictIndex and firstJString: the port has one
dictionary and entry_id carries both identities. inexactMatch narrows
from a tri-state int to a bool — its sign reflected alphabetical
order, not match quality.

commit() reproduces the three post-match filters. The duplicate
collapse is defensive: index/build already dedupes identical stored
records and Match compares surface, so no fixture can reach it, and
it is tested at the commit level instead."
```

---

## Task 2: The verb-conjugation recursion, in `matcher/verb.rs`

**Files:**
- Create: `ta/crates/jparser/src/matcher/verb.rs`
- Modify: `ta/crates/jparser/src/matcher.rs`

**Interfaces:**
- Consumes:
  - Task 1's `Match`, `ConjLink`, `commit`, `strict_eq`, `matches_at` (`verb.rs`
    is a child module of `matcher`, so it can use `matcher`'s private items)
  - `jparser::conjugation::{MAX_CONJ_DEPTH, TENSE_REMOVE, TENSE_STEM, TENSE_POTENTIAL, VerbTypeId}`
  - `jparser::conjugation::VerbType { conjugations: Vec<Conjugation> }`,
    `Conjugation { tense: TenseId, form: Form, suffix: String, next_verb_type: Option<VerbTypeId> }`
    — suffixes of linked conjugations were **already trimmed at load time**; the
    matcher does no further trimming
  - `jparser::kana::unify`
- Produces:
  - `pub(super) fn matcher::verb::recurse(table: &ConjugationTable, text: &[char], start: usize, slen: usize, vtype: VerbTypeId, chain: &[ConjLink], inexact: bool) -> Vec<Match>`
  - `pub(crate) fn matcher::unified_eq(a: &[char], b: &str) -> bool`
  - `matches_at`'s `Some(vtype)` arm, replacing Task 1's stub

**File-split deviation, flagged not silent.** Contract §7 maps the whole matcher
to `src/matcher.rs`. Assembled as one file, Tasks 1–3 measure 1033 rustfmt'd
lines against an 800 hard cap, and are already at 879 at the end of *this* task.
`recurse` and its nine JSON-fixture tests therefore live in `src/matcher/verb.rs`,
a child module — which keeps `strict_eq` private to `matcher` while remaining
visible to the recursion. `matcher.rs` ends this task at ≈470 lines and `verb.rs`
at ≈435. Same names, same signatures, same behaviour; only the file changes. See
the File Structure rationale.

Five rules, from `FindVerbMatches` (`Dictionary.cpp:738-805`). Four of them are
easy to implement approximately and hard to notice when wrong, so each gets its
own test that asserts the resulting **chain**, not merely that something matched.

`depth` is not a parameter: `depth == chain.len()`, because the chain is built
top-down here where ta-old filled `conj[depth]` on the unwind. The two are
equivalent, and dropping the parameter removes the way to get them out of sync.

- [ ] **Step 1: Write the failing tests for the recursion**

Create `ta/crates/jparser/src/matcher/verb.rs` containing the GPL v2 header and
only this test module:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{Form, TenseId, TENSE_NON_PAST};

    /// Highest tense id any fixture in this file can reach. The four fixed ids
    /// are consts; every other tense is a position in the table's name list, so
    /// tests resolve it by name rather than hard-coding a number.
    const TENSE_LOOKUP_LIMIT: TenseId = 64;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn tense(t: &ConjugationTable, name: &str) -> TenseId {
        (0..TENSE_LOOKUP_LIMIT)
            .find(|&i| t.tense_name(i) == Some(name))
            .unwrap_or_else(|| panic!("tense {name:?} must exist in the table"))
    }

    /// Run the recursion from the start of `text` against the first type named
    /// `name`, as if a zero-length stem had just been matched.
    fn run(t: &ConjugationTable, name: &str, text: &str) -> Vec<Match> {
        let vtype = t.types_named(name)[0];
        recurse(t, &chars(text), 0, 0, vtype, &[], false)
    }

    /// One type declares a "Remove" tense whose suffix matches the text; the
    /// other has no Remove entry at all, so its `remove_tense` defaults to
    /// Non-past.
    const REMOVE_JSON: &str = r#"[
      {"Name":"has-remove","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"く","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"き","Tense":"Past"}]},
      {"Name":"no-remove","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"く","Tense":"Non-past"}]}
    ]"#;

    /// Chaining fixtures. Every link target declares an empty "Remove" suffix,
    /// so load-time trimming is a no-op and each conjugation's suffix in the
    /// loaded table is exactly what is written here.
    const CHAIN_JSON: &str = r#"[
      {"Name":"two-step","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"leaf"}]},
      {"Name":"root","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"mid"}]},
      {"Name":"root-neg","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"mid-neg"}]},
      {"Name":"mid","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"い","Tense":"Stem","Next Type":"leaf"}]},
      {"Name":"mid-neg","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":true,"Suffix":"い","Tense":"Stem","Next Type":"leaf"}]},
      {"Name":"leaf","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"う","Tense":"Past"}]}
    ]"#;

    /// Six types in a row. d4 offers both a sixth link and a terminal
    /// alternative, so the test can tell "branch dropped" from "match
    /// truncated".
    const DEEP_JSON: &str = r#"[
      {"Name":"d0","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"d1"}]},
      {"Name":"d1","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"い","Tense":"Past","Next Type":"d2"}]},
      {"Name":"d2","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"う","Tense":"Past","Next Type":"d3"}]},
      {"Name":"d3","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"え","Tense":"Past","Next Type":"d4"}]},
      {"Name":"d4","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"お","Tense":"Past","Next Type":"d5"},
        {"Formal":false,"Negative":false,"Suffix":"お","Tense":"Te-form"}]},
      {"Name":"d5","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"か","Tense":"Past"}]}
    ]"#;

    /// pot-inner offers the same suffix under two tenses, so one sibling is
    /// dropped by the guard and one survives.
    const POTENTIAL_JSON: &str = r#"[
      {"Name":"pot-outer","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"れ","Tense":"Potential","Next Type":"pot-inner"}]},
      {"Name":"past-outer","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"れ","Tense":"Past","Next Type":"pot-inner"}]},
      {"Name":"pot-inner","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"ら","Tense":"Potential"},
        {"Formal":false,"Negative":false,"Suffix":"ら","Tense":"Past"}]}
    ]"#;

    #[test]
    fn remove_tense_conjugations_are_never_matched() {
        // Rule 1. The skip tests the global TENSE_REMOVE sentinel, never
        // VerbType::remove_tense — they differ for every type without an
        // explicit Remove entry, where remove_tense defaults to Non-past.
        let t = ConjugationTable::from_json(REMOVE_JSON).expect("fixture must load");

        // has-remove's Remove suffix く matches the text exactly and must still
        // be skipped; its only other conjugation is き, which does not match.
        assert!(run(&t, "has-remove", "く").is_empty());

        // no-remove's remove_tense IS Non-past, and its Non-past conjugation
        // must still be matchable.
        let no_remove = t.types_named("no-remove")[0];
        assert_eq!(t.types()[no_remove].remove_tense, TENSE_NON_PAST);
        let got = run(&t, "no-remove", "く");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 1);
        assert_eq!(
            got[0].chain,
            vec![ConjLink {
                verb_type: no_remove,
                tense: TENSE_NON_PAST,
                form: Form(0),
                conj: 0,
            }]
        );
    }

    #[test]
    fn chaining_through_next_verb_type_consumes_both_suffixes() {
        // two-step あ links to leaf う, and the match only exists if both
        // suffixes are consumed.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let two_step = t.types_named("two-step")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "two-step", "あう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 2, "len counts stem + every suffix");
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink { verb_type: two_step, tense: past, form: Form(0), conj: 0 },
                ConjLink { verb_type: leaf, tense: past, form: Form(0), conj: 1 },
            ]
        );

        // The chain cannot complete when the text runs out mid-way, and reading
        // past the end must not panic.
        assert!(run(&t, "two-step", "あ").is_empty());
    }

    #[test]
    fn an_informal_stem_above_depth_zero_leaves_no_chain_link() {
        // Rule 2 and rule 3. mid's Stem い is reached at depth 1, so it consumes
        // no depth and adds no link — but slen still advances past it, so three
        // suffix-consuming steps produce a two-link chain of length 3.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let root = t.types_named("root")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "root", "あいう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 3, "the skipped Stem's char is still counted");
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink { verb_type: root, tense: past, form: Form(0), conj: 0 },
                ConjLink { verb_type: leaf, tense: past, form: Form(0), conj: 1 },
            ],
            "mid must appear in no chain slot"
        );
    }

    #[test]
    fn the_same_stem_conjugation_at_depth_zero_takes_a_chain_slot() {
        // Rule 2's depth guard is load-bearing: the identical conjugation, now
        // the first link off the stem, behaves like any other.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let mid = t.types_named("mid")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "mid", "いう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 2);
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink { verb_type: mid, tense: TENSE_STEM, form: Form(0), conj: 1 },
                ConjLink { verb_type: leaf, tense: tense(&t, "Past"), form: Form(0), conj: 1 },
            ]
        );
    }

    #[test]
    fn a_negative_stem_above_depth_zero_is_not_skipped() {
        // Rule 2 is `form.0 == 0` exactly — informal-affirmative — not "the
        // formal bit is clear". mid-neg's Stem carries Negative, so it keeps its
        // slot and the chain grows to three links.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let mid_neg = t.types_named("mid-neg")[0];

        let got = run(&t, "root-neg", "あいう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].chain.len(), 3, "got {:#?}", got[0].chain);
        assert_eq!(
            got[0].chain[1],
            ConjLink {
                verb_type: mid_neg,
                tense: TENSE_STEM,
                form: Form(Form::NEGATIVE),
                conj: 1,
            }
        );
    }

    #[test]
    fn a_branch_needing_a_sixth_layer_is_dropped_whole() {
        // Rule 4. Chaining is allowed only while depth < MAX_CONJ_DEPTH - 1, so
        // d4's link to d5 never fires: no six-char match exists, and no
        // truncated five-layer stand-in is recorded for it either. d4's terminal
        // Te-form alternative shows five layers are still fine.
        let t = ConjugationTable::from_json(DEEP_JSON).expect("fixture must load");
        let got = run(&t, "d0", "あいうえおか");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 5, "the sixth suffix is never consumed");
        assert_eq!(got[0].chain.len(), MAX_CONJ_DEPTH);
        assert_eq!(
            t.tense_name(got[0].chain[4].tense),
            Some("Te-form"),
            "the surviving fifth layer is the terminal alternative, not a \
             truncated version of the dropped branch"
        );
    }

    #[test]
    fn a_potential_chained_into_a_potential_is_dropped() {
        // Rule 5. pot-inner offers ら under both Potential and Past; only the
        // Potential child is dropped.
        let t = ConjugationTable::from_json(POTENTIAL_JSON).expect("fixture must load");
        let got = run(&t, "pot-outer", "れら");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].chain[0].tense, TENSE_POTENTIAL);
        assert_eq!(
            got[0].chain[1].tense,
            tense(&t, "Past"),
            "the Potential+Potential sibling must be gone"
        );
    }

    #[test]
    fn a_repeated_non_potential_tense_survives() {
        // Rule 5 fires only when both adjacent tenses are Potential. Past into
        // Past is left alone, and so is Past into Potential.
        let t = ConjugationTable::from_json(POTENTIAL_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let got = run(&t, "past-outer", "れら");
        assert_eq!(got.len(), 2, "got {got:#?}");
        let mut inner: Vec<TenseId> = got.iter().map(|m| m.chain[1].tense).collect();
        inner.sort_unstable();
        let mut want = vec![TENSE_POTENTIAL, past];
        want.sort_unstable();
        assert_eq!(inner, want);
        assert!(got.iter().all(|m| m.chain[0].tense == past));
    }

    #[test]
    fn an_inexact_suffix_poisons_the_whole_chain() {
        // inexact is monotonic: the loose comparison lets katakana text match a
        // hiragana suffix, the strict one records that it did.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let exact = run(&t, "two-step", "あう");
        assert!(!exact[0].inexact);
        let fuzzy = run(&t, "two-step", "アう");
        assert_eq!(fuzzy.len(), 1, "got {fuzzy:#?}");
        assert!(fuzzy[0].inexact, "one inexact suffix marks the whole match");
    }
}
```

- [ ] **Step 2: Write the failing test for the stamping in `matches_at`**

Append to the `mod tests` block in `ta/crates/jparser/src/matcher.rs`, inside the
existing braces, and add `use crate::conjugation::TENSE_STEM;` to that test
module's `use` list (Task 3 moves it up into the module import and deletes this
line):

```rust
    #[test]
    fn matches_at_stamps_record_fields_onto_recursion_output() {
        // End to end against the real embedded table: 食べる is v1, its stem 食べ
        // is indexed, and 食べた reaches v1's Stem た, which links to v-ta-stem's
        // terminal Past (an empty suffix). No other path off 食べ reaches three
        // characters.
        let t = table();
        let idx = index("verb");
        let text = chars("食べた");
        let got = matches_at(&idx, &t, &text, 0).expect("matcher must not fail");
        let three: Vec<&Match> = got.iter().filter(|m| m.len == 3).collect();
        assert_eq!(three.len(), 1, "got {got:#?}");
        let m = three[0];

        assert_eq!(m.start, 0);
        assert_eq!(m.src_len, 2, "src_len is the key alone, len is key + suffixes");
        assert_eq!(m.surface, "食べ");
        assert_eq!(m.entry_id, 2000070);
        assert!(m.flags.contains(WordFlags::PRIMARY));
        assert!(!m.inexact);

        assert_eq!(m.chain.len(), 2);
        assert_eq!(m.chain[0].verb_type, t.types_named("v1")[0]);
        assert_eq!(m.chain[0].tense, TENSE_STEM);
        assert_eq!(
            t.types()[m.chain[0].verb_type].conjugations[m.chain[0].conj].suffix,
            "た",
            "conj must index back to the conjugation that was matched"
        );
        assert_eq!(t.types()[m.chain[1].verb_type].name, "v-ta-stem");
        assert_eq!(t.tense_name(m.chain[1].tense), Some("Past"));
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser matcher`
Expected: FAIL — `file not found for module 'verb'` until Step 4 declares it, then
`cannot find function 'recurse' in this scope` from `verb.rs`'s tests, and
`cannot find value 'TENSE_STEM' in this scope` from `matcher.rs`'s new test until
Step 2's import is in place.

- [ ] **Step 4: Declare the child module and add the loose comparison**

In `ta/crates/jparser/src/matcher.rs`, directly below the `use` block:

```rust
// The conjugation recursion and its nine rule tests. A child module, not a
// sibling, so it keeps access to `strict_eq`; split out purely for the 800-line
// cap — see the plan's File Structure note.
mod verb;
```

Leave the conjugation import exactly as Task 1 wrote it:

```rust
use crate::conjugation::{ConjugationTable, Form, TenseId, VerbTypeId};
```

`MAX_CONJ_DEPTH`, `TENSE_REMOVE`, `TENSE_STEM` and `TENSE_POTENTIAL` are used by
`recurse`, which now lives in `verb.rs`; importing them here would be an unused
import, and `#[allow(dead_code)]` does **not** cover `unused_imports`.

Add, directly below `strict_eq`:

```rust
/// ta-old's `wcsnijcmp` (`Shared/Shrink.h:197`): kana type, width and ASCII case
/// all folded through `kana::unify`. This is the comparison the FST key already
/// applies to the dictionary key; conjugation suffixes are not in the FST, so
/// they need it applied here.
///
/// `pub(crate)` rather than private because Task 7's `tails_match` is a caller
/// outside this module — the kuruHack tail comparison is the same one.
pub(crate) fn unified_eq(a: &[char], b: &str) -> bool {
    let mut rhs = b.chars();
    for &lhs in a {
        let Some(other) = rhs.next() else { return false };
        if crate::kana::unify(lhs) != crate::kana::unify(other) {
            return false;
        }
    }
    rhs.next().is_none()
}
```

- [ ] **Step 5: Implement the recursion**

Insert into `ta/crates/jparser/src/matcher/verb.rs`, between the GPL v2 header
and `#[cfg(test)]`:

```rust
//! The verb-conjugation recursion, ta-old's `FindVerbMatches`
//! (`ta-old/exe/util/Dictionary.cpp:738-805`).
//!
//! A child module of `matcher` for one reason: the 800-line cap. Keeping it a
//! child rather than a sibling preserves access to `matcher`'s private
//! `strict_eq`, so no comparison becomes crate-visible just to be split.

use crate::conjugation::{
    ConjugationTable, VerbTypeId, MAX_CONJ_DEPTH, TENSE_POTENTIAL, TENSE_REMOVE, TENSE_STEM,
};
use crate::record::WordFlags;

use super::{strict_eq, unified_eq, ConjLink, Match};

/// Match conjugation suffixes onward from a dictionary stem.
///
/// `slen` is the number of chars consumed since `start`: the dictionary key
/// plus every suffix matched along this path. `depth` is not a parameter —
/// `chain.len()` is it. ta-old filled `conj[depth]` while unwinding; the chain
/// is built top-down here, which produces the same array and removes the way
/// for the two to disagree.
///
/// The returned matches are incomplete on purpose: `src_len`, `surface`,
/// `flags` and `entry_id` are placeholders that `matches_at` stamps from the
/// `StoredRecord`, exactly as ta-old's `FindMatches` filled `srcLen`, `jString`
/// and `dictIndex` into the slots `FindVerbMatches` had just appended
/// (`Dictionary.cpp:900-927`).
///
/// Conjugation suffixes arrive already trimmed — the conjugation table stripped
/// each link target's remove-suffix at load time — so there is no trimming
/// arithmetic here, exactly as in ta-old.
///
// ponytail: the Stem-skip arm advances neither `depth` nor the cap, so a
// zero-width Stem/form-0 cycle in a conjugation asset would recurse until the
// stack overflows. The shipped asset has six zero-width stem-skip edges
// (v5uru -> v-i-stem/v-a-stem, v1 -> v-i-stem/v-a-stem, both vs -> vs-i) and is
// acyclic, and `Index::open`'s fingerprint check binds an index to its asset —
// so there is no hazard today. Add a visited-(vtype, slen) set here if
// `from_json` ever ingests an asset this crate did not ship.
pub(super) fn recurse(
    table: &ConjugationTable,
    text: &[char],
    start: usize,
    slen: usize,
    vtype: VerbTypeId,
    chain: &[ConjLink],
    inexact: bool,
) -> Vec<Match> {
    let depth = chain.len();
    let mut out: Vec<Match> = Vec::new();
    // A verb_type id from an index built against a different conjugation asset.
    // `Index::open`'s fingerprint check makes this unreachable; returning
    // nothing still beats indexing out of bounds.
    let Some(ty) = table.types().get(vtype) else { return out };

    for (cj, c) in ty.conjugations.iter().enumerate() {
        // Rule 1: the global Remove sentinel, never `ty.remove_tense`. A Remove
        // conjugation is bookkeeping that tells stem generation what to strip;
        // it is never a real match.
        if c.tense == TENSE_REMOVE {
            continue;
        }
        let n = c.suffix.chars().count();
        let from = start + slen;
        // Nothing may read past the end of the text.
        let Some(slice) = text.get(from..from + n) else { continue };
        if !unified_eq(slice, &c.suffix) {
            continue;
        }
        // Monotonic: an inexact suffix anywhere poisons the whole chain.
        let inexact2 = inexact || !strict_eq(slice, &c.suffix);
        let link = ConjLink { verb_type: vtype, tense: c.tense, form: c.form, conj: cj };

        match c.next_verb_type {
            // Terminal: this is the only place a match is created, and the only
            // place `len` is ever written.
            None => {
                let mut full = chain.to_vec();
                full.push(link);
                out.push(Match {
                    start,
                    len: slen + n,
                    src_len: 0,
                    surface: String::new(),
                    flags: WordFlags::default(),
                    entry_id: 0,
                    inexact: inexact2,
                    chain: full,
                });
            }
            // Rule 2: an informal-affirmative Stem above depth 0 consumes no
            // depth and records no link — but rule 3, `slen` still advances, so
            // its characters are counted in `len`. `form.0 == 0` is exact:
            // informal *and* affirmative, not merely "not formal".
            Some(next) if depth > 0 && c.tense == TENSE_STEM && c.form.0 == 0 => {
                out.extend(recurse(table, text, start, slen + n, next, chain, inexact2));
            }
            // Rule 4: chaining is allowed only while a further layer fits.
            Some(next) if depth < MAX_CONJ_DEPTH - 1 => {
                let mut extended = chain.to_vec();
                extended.push(link);
                let mut kids =
                    recurse(table, text, start, slen + n, next, &extended, inexact2);
                // Rule 5: drop a child whose own layer repeats this frame's
                // Potential (`Dictionary.cpp:780-792`). Any other repeated tense
                // is left alone.
                //
                // Documented fidelity divergence: ta-old removed the child with
                // a swap-remove (`matches[m] = matches[numMatches-1]`,
                // `:784-790`), which permutes the surviving siblings; `retain`
                // preserves order. Emission order feeds the DP's `>=` tie-break,
                // so this is a real if small deviation. The contract's
                // pseudocode mandates `retain`; Phase 6's differential run is
                // where a difference would surface.
                if c.tense == TENSE_POTENTIAL {
                    kids.retain(|m| {
                        m.chain.get(depth + 1).map(|l| l.tense) != Some(TENSE_POTENTIAL)
                    });
                }
                out.append(&mut kids);
            }
            // Rule 4, the other half: at the cap the branch is dropped whole. No
            // recursion, no match, and deliberately no truncated stand-in.
            Some(_) => {}
        }
    }
    out
}
```

- [ ] **Step 6: Replace the verb stub in `matches_at`**

In `ta/crates/jparser/src/matcher.rs`, replace

```rust
                Some(vtype) => {
                    // The conjugation recursion lands in the next task; until
                    // then a verb record contributes no matches.
                    let _ = (table, vtype);
                }
```

with

```rust
                Some(vtype) => {
                    for mut m in verb::recurse(table, text, i, k, vtype, &[], inexact) {
                        m.src_len = k;
                        m.surface = record.surface.clone();
                        m.flags = WordFlags(record.flags);
                        m.entry_id = record.entry_id;
                        commit(&mut out, m);
                    }
                }
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser matcher`
Expected: PASS, 23 tests — 14 in `matcher::tests` and 9 in
`matcher::verb::tests`. Task 1's 13 still pass unchanged, including
`text_with_no_indexed_prefix_yields_nothing` and
`the_empty_key_hit_never_produces_a_zero_length_match`, which now exercise the
recursion against the empty `する` stem.

- [ ] **Step 8: Format, lint, and check both file sizes**

Run:

```bash
cd ta && rustfmt --edition 2021 crates/jparser/src/matcher.rs crates/jparser/src/matcher/verb.rs \
  && cargo clippy -p jparser --all-targets -- -D warnings \
  && wc -l crates/jparser/src/matcher.rs crates/jparser/src/matcher/verb.rs
```

Expected: clippy clean; `matcher.rs` around **470** lines and `verb.rs` around
**435**, both far under 800. (Assembled as one file these two measure 879 —
which is why they are two. If either individually approaches 700, stop and
re-split rather than pushing to 800.) rustfmt rewrites both files; that is
expected, not a failure.

- [ ] **Step 9: Commit**

```bash
cd ta && git add crates/jparser/src/matcher.rs crates/jparser/src/matcher/verb.rs
git commit -m "feat: recurse verb conjugation chains in the matcher

Ports FindVerbMatches. A stem record in the index is the entry point:
the recursion matches already-trimmed conjugation suffixes onward from
it, chaining through next_verb_type, and every match carries the full
(type, tense, form, conj) chain that renders its label.

depth is not a parameter — chain.len() is it. ta-old filled
conj[depth] while unwinding; building the chain top-down produces the
same array with no second source of truth for the depth.

Four rules are individually tested against the chain they produce,
because each is easy to implement approximately and invisible when
wrong: the Remove skip tests the global sentinel rather than the
type's remove_tense (they differ whenever a type declares no Remove
entry); an informal-affirmative Stem above depth 0 consumes no depth
and records no link while still advancing the matched length; a branch
that would need a sixth layer is dropped whole rather than truncated;
and a Potential chained straight into a Potential is dropped while any
other repeated tense survives.

recurse lives in matcher/verb.rs, not matcher.rs as the interface
contract's module map says: assembled as one file the matcher measures
1033 lines against an 800 hard cap, and 879 at the end of this task
alone. A child module rather than a sibling, so strict_eq stays
private to matcher. Same name, signature, and behaviour; only the file
differs — the same size-driven split the contract already anticipates
for entry.rs, flagged rather than taken silently."
```

---

## Task 3: `render_conjugation_label`

**Files:**
- Modify: `ta/crates/jparser/src/matcher.rs`

**Interfaces:**
- Consumes: Task 1's `ConjLink` and its `table()`/`chars()` test helpers;
  `ConjugationTable::tense_name(&self, id: TenseId) -> Option<&str>`;
  `conjugation::{TENSE_NON_PAST, TENSE_STEM}`; `Form::is_formal`,
  `Form::is_negative`
- Produces:
  - `pub(crate) fn matcher::render_conjugation_label(chain: &[ConjLink], table: &ConjugationTable) -> String`

ta-old's `GetConjString` (`Dictionary.cpp:1449-1468`). Three clauses carry all the
behaviour, and two of them are easy to get backwards:

- `"Negative "` is emitted **before** `"Formal "`, and both are emitted for every
  layer — including a layer whose tense word is then dropped.
- `Stem` is dropped at **every** depth, including depth 0.
- `Non-past` is dropped at every depth > 0, **with exactly one exception**: depth 1
  keeps it when depth 0 was a `Stem`. Without that exception a Stem+Non-past pair
  — a genuine plain non-past form — would render as the empty string, which is the
  whole reason the clause exists.

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `ta/crates/jparser/src/matcher.rs`. The
`TENSE_LOOKUP_LIMIT`/`tense` pair mirrors the one `matcher/verb.rs` carries; the
two test modules are separate scopes and cannot share it.

```rust
    /// Highest tense id the embedded table can reach. The four fixed ids are
    /// consts; every other tense is a position in the table's name list, so
    /// tests resolve it by name rather than hard-coding a number.
    const TENSE_LOOKUP_LIMIT: TenseId = 64;

    /// No table names this id, so `tense_name` returns None.
    const UNNAMED_TENSE: TenseId = 9_999;

    fn tense(t: &ConjugationTable, name: &str) -> TenseId {
        (0..TENSE_LOOKUP_LIMIT)
            .find(|&i| t.tense_name(i) == Some(name))
            .unwrap_or_else(|| panic!("tense {name:?} must exist in the table"))
    }

    /// The renderer reads only `tense` and `form`, so the other two fields are
    /// held at zero.
    fn link(tense: TenseId, form: u8) -> ConjLink {
        ConjLink { verb_type: 0, tense, form: Form(form), conj: 0 }
    }

    #[test]
    fn renders_a_single_non_past_layer() {
        let t = table();
        assert_eq!(
            render_conjugation_label(&[link(TENSE_NON_PAST, 0)], &t),
            "Non-past"
        );
    }

    #[test]
    fn emits_negative_before_formal() {
        // form 3 = FORMAL | NEGATIVE. ta-old tests bit 1 first, so "Negative"
        // leads even though it is the higher bit.
        let t = table();
        let past = tense(&t, "Past");
        assert_eq!(
            render_conjugation_label(&[link(past, Form::FORMAL | Form::NEGATIVE)], &t),
            "Negative Formal Past"
        );
    }

    #[test]
    fn a_stem_layer_is_dropped_but_still_contributes_its_prefixes() {
        // The prefixes are appended before the skip checks run. No Stem in the
        // shipped asset carries a form, but the code does not prevent one.
        let t = table();
        assert_eq!(
            render_conjugation_label(&[link(TENSE_STEM, Form::NEGATIVE)], &t),
            "Negative"
        );
    }

    #[test]
    fn depth_one_non_past_survives_when_depth_zero_was_a_stem() {
        // The exception. Without it both layers would be skipped and a plain
        // non-past form would render as "".
        let t = table();
        let chain = [link(TENSE_STEM, 0), link(TENSE_NON_PAST, 0)];
        assert_eq!(render_conjugation_label(&chain, &t), "Non-past");
    }

    #[test]
    fn depth_one_non_past_is_suppressed_after_a_non_stem() {
        let t = table();
        let chain = [link(tense(&t, "Past"), 0), link(TENSE_NON_PAST, 0)];
        assert_eq!(render_conjugation_label(&chain, &t), "Past");
    }

    #[test]
    fn non_past_below_depth_one_is_suppressed_even_after_a_stem() {
        // The exception is `i == 1` exactly, and it inspects chain[0], never
        // chain[i - 1].
        let t = table();
        let chain = [
            link(TENSE_STEM, 0),
            link(tense(&t, "Past"), 0),
            link(TENSE_NON_PAST, 0),
        ];
        assert_eq!(render_conjugation_label(&chain, &t), "Past");
    }

    #[test]
    fn renders_prefixes_from_a_deeper_layer() {
        let t = table();
        let chain = [link(TENSE_STEM, 0), link(tense(&t, "Past"), Form::NEGATIVE)];
        assert_eq!(render_conjugation_label(&chain, &t), "Negative Past");
    }

    #[test]
    fn an_empty_chain_renders_as_the_empty_string() {
        // ta-old returned 0 with the buffer left empty for an unconjugated hit.
        // Callers must read that as "no label applies", not as an error.
        let t = table();
        assert_eq!(render_conjugation_label(&[], &t), "");
    }

    #[test]
    fn a_tense_the_table_cannot_name_contributes_nothing() {
        let t = table();
        let chain = [link(tense(&t, "Past"), 0), link(UNNAMED_TENSE, 0)];
        assert_eq!(render_conjugation_label(&chain, &t), "Past");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser matcher`
Expected: FAIL — `cannot find function 'render_conjugation_label' in this scope`
(9 occurrences), plus `cannot find value 'TENSE_NON_PAST' in this scope` until
Step 3 widens the module import.

- [ ] **Step 3: Widen the module import**

In `ta/crates/jparser/src/matcher.rs`, replace the conjugation import with:

```rust
use crate::conjugation::{
    ConjugationTable, Form, TenseId, VerbTypeId, TENSE_NON_PAST, TENSE_STEM,
};
```

Both new names are read by the renderer below, so this is not an unused import.
Delete the `use crate::conjugation::TENSE_STEM;` line Task 2 added to the test
module — `use super::*` now supplies it.

- [ ] **Step 4: Implement the renderer**

Add to `ta/crates/jparser/src/matcher.rs`, below `matches_at`:

```rust
/// Emitted for a layer whose form carries `Form::NEGATIVE`, before the formal
/// prefix, per `Dictionary.cpp:1453-1454`.
const NEGATIVE_PREFIX: &str = "Negative ";
/// Emitted for a layer whose form carries `Form::FORMAL`.
const FORMAL_PREFIX: &str = "Formal ";
/// The only depth at which a `Non-past` escapes the depth > 0 suppression, and
/// then only when depth 0 was a `Stem`.
const STEM_NON_PAST_DEPTH: usize = 1;

/// ta-old's `GetConjString` (`Dictionary.cpp:1449-1468`): render a match's
/// conjugation chain as a label such as "Negative Formal Past".
///
/// Layers are visited shallowest first, so the label reads from the layer
/// nearest the dictionary stem outward — reverse the order and every stacked
/// form renders backwards.
///
/// An empty result is legitimate: an unconjugated hit has no chain, and a chain
/// of nothing but skipped layers renders as "".
pub(crate) fn render_conjugation_label(chain: &[ConjLink], table: &ConjugationTable) -> String {
    let mut out = String::new();
    for (i, link) in chain.iter().enumerate() {
        // Both prefixes are emitted before the skip checks, so a layer whose
        // tense word is dropped below still contributes them.
        if link.form.is_negative() {
            out.push_str(NEGATIVE_PREFIX);
        }
        if link.form.is_formal() {
            out.push_str(FORMAL_PREFIX);
        }
        // Non-past is suppressed at every depth > 0 except depth 1 after a
        // Stem — without that exception a Stem+Non-past pair would render
        // empty, since the next clause drops the Stem too. The test inspects
        // chain[0], never chain[i - 1].
        if i > 0
            && link.tense == TENSE_NON_PAST
            && (i > STEM_NON_PAST_DEPTH || chain[0].tense != TENSE_STEM)
        {
            continue;
        }
        if link.tense == TENSE_STEM {
            continue;
        }
        let Some(name) = table.tense_name(link.tense) else { continue };
        out.push_str(name);
        out.push(' ');
    }
    // Every appended word carries exactly one trailing space and a skipped
    // layer appends none, so at most one space can accumulate. This is ta-old's
    // single conditional decrement, not a general trim.
    if out.ends_with(' ') {
        out.pop();
    }
    out
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser matcher`
Expected: PASS, 32 tests — 23 in `matcher::tests`, 9 in `matcher::verb::tests`.

- [ ] **Step 6: Run the whole crate suite**

Run: `cd ta && cargo test -p jparser`
Expected: PASS — every Phase 1A test plus the 32 matcher tests. Nothing in
Phase 1A was modified beyond `lib.rs`'s two additions, so no existing test
changes behaviour.

- [ ] **Step 7: Format, lint, and check the file budget**

Run:

```bash
cd ta && rustfmt --edition 2021 crates/jparser/src/matcher.rs \
  && cargo clippy -p jparser --all-targets -- -D warnings \
  && wc -l crates/jparser/src/matcher.rs crates/jparser/src/matcher/verb.rs
```

Expected: clippy clean; `matcher.rs` around **625** lines and `verb.rs` unchanged
at around **435**. Both under 800, with ~175 lines of headroom in `matcher.rs`.
Assembled as one file these would be 1033 — the split taken in Task 2 is what
keeps them legal, and no further split is needed here.

- [ ] **Step 8: Commit**

```bash
cd ta && git add crates/jparser/src/matcher.rs
git commit -m "feat: render a match's conjugation chain as a label

Ports GetConjString. Layers render shallowest first, so the label
reads outward from the dictionary stem; reversed, every stacked form
would come out backwards.

Three clauses carry the behaviour and two invite paraphrase. Negative
precedes Formal, and both are emitted for a layer even when its tense
word is then dropped. Stem is dropped at every depth including zero.
Non-past is dropped at every depth above zero with one exception —
depth 1 keeps it when depth 0 was a Stem — which exists solely so a
Stem+Non-past pair does not render as the empty string. Each of those,
including the empty-string cases, has a test asserting the exact
output.

The renderer stays in matcher.rs rather than moving to a label.rs: the
interface contract is explicit that label.rs does not exist, and with
the recursion already split into matcher/verb.rs the file fits."
```

---

## Task 4: `BoundaryHints` and the DP skeleton over skipped runs

**Files:**
- Create: `ta/crates/jparser/src/segment.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes:
  - `crate::matcher::Match` (Task 2)
  - `crate::kana::is_cjk_ideograph` (Phase 1A)
- Produces:
  - `pub trait segment::BoundaryHints { fn bad_start(&self, pos: usize) -> bool; fn bad_end(&self, pos: usize) -> bool; }`, re-exported as `jparser::BoundaryHints`
  - `pub(crate) struct segment::Segmentation { pub(crate) spans: Vec<Span>, pub(crate) total_cost: i32 }`
  - `pub(crate) struct segment::Span { pub(crate) start: usize, pub(crate) len: usize, pub(crate) matched: bool, pub(crate) matches: Vec<Match> }`
  - `pub(crate) fn segment::segment(text: &[char], matches: &[Vec<Match>], hints: Option<&dyn BoundaryHints>) -> Segmentation`
  - private `struct Cell { cost: i32, back_len: usize }`, `fn backtrack`
  - private `const SKIP_CHAR: i32 = 100`, `const SKIP_KANJI_EXTRA: i32 = 400`

**Resolved gaps.** The contract names `Cell::cost`/`Cell::back_len` (§8) but never
declares `Cell`: it is a private `#[derive(Debug, Clone, Copy)] struct Cell` here.
`score_match` and `backtrack` appear in §6.3's pseudocode with no signature; they
are fixed as
`fn score_match(text: &[char], m: &Match, hints: Option<&dyn BoundaryHints>, base: i32) -> i32`
and `fn backtrack(text: &[char], matches: &[Vec<Match>], best: &[Cell], n: usize) -> Vec<Span>`
(the last two parameters arrive in Task 6), both private. Empty input is
undefined by the contract and is defined here as
`Segmentation { spans: vec![], total_cost: 0 }`, which is what §3.2's "empty iff
the input is empty" implies.

This task exists to make one property true and testable before any scoring lands:
**`segment()` is a pure function of `(text, matches, hints)` with no index, no
conjugation table, and no I/O.** Addendum §4 is explicit that this shape is what
lets §10's cost assertions run against a hand-built `Vec<Vec<Match>>`. Every test
in this task therefore uses a zero-entry dictionary — literally
`vec![Vec::new(); n]` — and asserts `total_cost` directly.

The signature is the frozen one from day one, including the `matches` parameter
this task never reads for scoring and the `hints` parameter it never reads at all.
Tasks 5 and 6 fill the body; they do not change the interface.

- [ ] **Step 1: Register the module and the re-export**

`ta/crates/jparser/src/lib.rs` — the module block becomes exactly this. Keep
Task 1's `#[allow(dead_code)]` on `mod matcher;` and Task 1's `ParseError` enum
where they are; dropping either reintroduces the dead-code errors Task 1's clippy
gate depends on.

```rust
pub mod conjugation;
pub mod index;
pub mod jmdict;
pub mod kana;
// Dead until `parse` lands: nothing outside the module's own tests calls the
// matcher yet. Task 7 removes this attribute. It covers `matcher::verb` too.
#[allow(dead_code)]
mod matcher;
pub mod record;
pub mod romaji;
mod segment;
pub mod stem;

pub use crate::segment::BoundaryHints;
```

`segment` is a private module per contract §4.1; `BoundaryHints` is the one `pub`
item inside it, because Phase 5 implements the trait from outside. `segment`
deliberately gets **no** `#[allow(dead_code)]` — Step 7 asserts the resulting
warnings, and they clear on their own in Task 7.

- [ ] **Step 2: Write the failing tests**

Create `ta/crates/jparser/src/segment.rs` containing the GPL v2 header and only
this test module:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysBad;
    impl BoundaryHints for AlwaysBad {
        fn bad_start(&self, _pos: usize) -> bool {
            true
        }
        fn bad_end(&self, _pos: usize) -> bool {
            true
        }
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn no_matches(text: &[char]) -> Vec<Vec<Match>> {
        vec![Vec::new(); text.len()]
    }

    fn shape(seg: &Segmentation) -> Vec<(usize, usize, bool)> {
        seg.spans
            .iter()
            .map(|s| (s.start, s.len, s.matched))
            .collect()
    }

    fn assert_contiguous(seg: &Segmentation, n: usize) {
        let mut at = 0;
        for s in &seg.spans {
            assert_eq!(s.start, at, "gap or overlap at {at}: {:?}", shape(seg));
            assert!(s.len >= 1, "zero-length span: {:?}", shape(seg));
            if !s.matched {
                assert!(s.matches.is_empty(), "unmatched span carries matches");
            }
            at += s.len;
        }
        assert_eq!(at, n, "spans do not cover the input: {:?}", shape(seg));
    }

    // ---- skipped runs ---------------------------------------------------

    #[test]
    fn empty_input_costs_nothing_and_produces_no_spans() {
        let text: Vec<char> = Vec::new();
        let seg = segment(&text, &[], None);
        assert_eq!(seg.total_cost, 0);
        assert!(seg.spans.is_empty());
    }

    #[test]
    fn each_skipped_character_costs_skip_char() {
        // 3 kana, nothing in the dictionary: 3 x SKIP_CHAR = 300.
        let text = chars("あいう");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 300);
        assert_eq!(shape(&seg), vec![(0, 3, false)]);
        assert_contiguous(&seg, 3);
    }

    #[test]
    fn skipping_a_cjk_ideograph_adds_the_kanji_extra() {
        // SKIP_CHAR 100 + SKIP_KANJI_EXTRA 400 = 500.
        let text = chars("言");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 500);
        assert_eq!(shape(&seg), vec![(0, 1, false)]);
    }

    #[test]
    fn the_kanji_repeat_mark_is_not_a_cjk_ideograph() {
        // is_kanji covers U+3005, is_cjk_ideograph deliberately does not, so
        // the repeat mark skips at the base rate: 100, not 500.
        let text = chars("々");
        assert_eq!(segment(&text, &no_matches(&text), None).total_cost, 100);
    }

    #[test]
    fn mixed_kanji_and_kana_skips_add_up() {
        // 言 = 100 + 400, う = 100. Total 600, coalesced into one span.
        let text = chars("言う");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 600);
        assert_eq!(shape(&seg), vec![(0, 2, false)]);
    }

    #[test]
    fn hints_never_change_a_skip() {
        // MECAB_BAD_START/END apply to matches only (Dictionary.cpp:1180-1183).
        let text = chars("言う");
        let with = segment(&text, &no_matches(&text), Some(&AlwaysBad));
        let without = segment(&text, &no_matches(&text), None);
        assert_eq!(with.total_cost, 600);
        assert_eq!(with, without);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser --lib segment::`
Expected: FAIL to compile — `error[E0432]: unresolved import
'crate::segment::BoundaryHints'` from `lib.rs`, plus, from the test module,
`cannot find trait 'BoundaryHints' in this scope`, `cannot find type
'Segmentation' in this scope`, `cannot find type 'Match' in this scope`, and
`cannot find function 'segment' in this scope`.

- [ ] **Step 4: Implement the skeleton**

Insert above the test module, between the GPL v2 header and `#[cfg(test)]`:

```rust
//! Segmentation: a min-cost dynamic program over character positions, ported
//! from ta-old's `FindBestMatches` (`ta-old/exe/util/Dictionary.cpp:1075-1306`).
//!
//! **Low cost wins.** ta-old's own comment at `Dictionary.cpp:1143`: *"High
//! score is bad, low is good."*
//!
//! `segment` is a pure function of `(text, matches, hints)`: no index, no
//! conjugation table, no I/O, and therefore infallible. That is what lets the
//! cost assertions in this file's test module run against a hand-built match
//! table with no dictionary at all.

use crate::kana;
use crate::matcher::Match;

/// Cost of leaving one character unmatched. ta-old `Dictionary.cpp:1164`.
const SKIP_CHAR: i32 = 100;
/// Extra cost of skipping a CJK ideograph (`kana::is_cjk_ideograph`,
/// 0x4E00..=0x9FBF). Stacks on `SKIP_CHAR`; never applies to a match.
/// ta-old `Dictionary.cpp:1166`.
const SKIP_KANJI_EXTRA: i32 = 400;

/// Boundary votes from a morphological analyzer. Phase 5 supplies the Vibrato
/// implementation; Phase 1B ships only this trait and test stubs.
///
/// `pos` is a **char** offset, matching `Segment::start` — never a byte
/// offset. `None` hints must behave exactly like an implementation that
/// returns `false` everywhere.
pub trait BoundaryHints {
    /// True when a word should not begin at `pos`.
    fn bad_start(&self, pos: usize) -> bool;
    /// True when a word should not end at `pos`.
    fn bad_end(&self, pos: usize) -> bool;
}

/// The chosen cover of the input plus the DP's own total cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segmentation {
    /// Contiguous cover of the input in ascending `start` order: every char
    /// position belongs to exactly one span, matched or not. Empty iff the
    /// input is empty.
    pub(crate) spans: Vec<Span>,
    /// `best[len].cost`, asserted directly by the cost tests.
    ///
    /// Read by this module's tests and by nothing else in the library:
    /// `ParseResult` deliberately does not carry a cost (a display/diagnostic
    /// concern above this crate), so port design §10's "assert the cost, not
    /// just the winning segmentation" is satisfied here and only here. The
    /// attribute is the narrowest possible — one field, not the module.
    #[allow(dead_code)]
    pub(crate) total_cost: i32,
}

/// One chosen span. `matched` is false for a skipped run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) len: usize,
    pub(crate) matched: bool,
    /// Every match aligning to `(start, len)` — not only the DP winner —
    /// already run through `sort_matches`. Always empty when `!matched`.
    pub(crate) matches: Vec<Match>,
}

/// One DP cell: the cheapest cost of covering `text[..pos]`, plus how the
/// cheapest path arrived.
#[derive(Debug, Clone, Copy)]
struct Cell {
    cost: i32,
    /// Char length of the match that reached this position, or `0` when it was
    /// reached by a skip. Sound only because `matches_at` drops `len == 0`
    /// candidates.
    back_len: usize,
}

/// Segment `text`, where `matches[p]` holds every match with `start == p`.
pub(crate) fn segment(
    text: &[char],
    matches: &[Vec<Match>],
    hints: Option<&dyn BoundaryHints>,
) -> Segmentation {
    debug_assert_eq!(matches.len(), text.len(), "one match bucket per character");
    debug_assert!(
        matches.iter().enumerate().all(|(p, bucket)| bucket
            .iter()
            .all(|m| m.start == p && m.start + m.len <= text.len())),
        "every match in bucket p must have start == p and end inside the text"
    );
    // Hints price matches only; the skip transition never consults them.
    let _ = hints;

    let n = text.len();
    let mut best = vec![
        Cell {
            cost: i32::MAX,
            back_len: 0
        };
        n + 1
    ];
    best[0] = Cell {
        cost: 0,
        back_len: 0,
    };

    for pos in 0..n {
        // 1. The skip transition, computed FIRST. The tie rules depend on this
        //    order, exactly as ta-old's do.
        let mut cost = best[pos].cost.saturating_add(SKIP_CHAR);
        if kana::is_cjk_ideograph(text[pos]) {
            cost = cost.saturating_add(SKIP_KANJI_EXTRA);
        }
        // STRICT `>`: on a tie the value already written wins
        // (`Dictionary.cpp:1169`).
        if best[pos + 1].cost > cost {
            best[pos + 1] = Cell { cost, back_len: 0 };
        }
    }

    Segmentation {
        total_cost: best[n].cost,
        spans: backtrack(&best, n),
    }
}

/// Walk the backpointers from the end, ta-old `Dictionary.cpp:1280-1305`.
fn backtrack(best: &[Cell], n: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = n;
    while pos > 0 {
        // Coalesce the whole run of skipped chars into one unmatched span.
        // ta-old emitted nothing for these (`Dictionary.cpp:1288-1290`); the
        // port emits them so `parse` can return unmatched `Segment`s.
        let end = pos;
        while pos > 0 && best[pos].back_len == 0 {
            pos -= 1;
        }
        debug_assert!(pos < end, "a non-zero back_len with no match transition");
        spans.push(Span {
            start: pos,
            len: end - pos,
            matched: false,
            matches: Vec::new(),
        });
    }
    spans.reverse();
    spans
}
```

Four details that are decisions, not accidents:

- `saturating_add` throughout. Every position is reachable through the skip
  chain, so `i32::MAX` is never read as an operand; the saturation is
  belt-and-braces against a debug-build overflow panic if that ever stops being
  true.
- The skip relaxation is a **strict `>`**. Task 5 adds a match relaxation using
  `>=`. They must stay two separate comparisons — routing both through one
  `relax` helper changes which candidate wins a tie and produces a different
  (equally cheap) segmentation. Task 5 has a test for exactly that.
- `let _ = hints;` is deliberate: the parameter is part of the frozen signature
  and Task 5 starts reading it. Silencing it with `#[allow(unused_variables)]` or
  renaming it to `_hints` would both have to be undone next task.
- The bucket `debug_assert!` also checks `m.start + m.len <= text.len()`. The
  match relaxation in Task 5 indexes `best[pos + m.len]` with no bound check; a
  hand-built bucket that overruns should fail with this module's own diagnostic,
  not a bare index-out-of-bounds.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser --lib segment::`
Expected: PASS, 6 tests.

- [ ] **Step 6: Format and check the file size**

Run: `cd ta && rustfmt --edition 2021 crates/jparser/src/segment.rs && wc -l crates/jparser/src/segment.rs`
Expected: rustfmt makes no change, and roughly `246 crates/jparser/src/segment.rs`
(a few lines more for the widened `debug_assert!` and the `total_cost`
attribute). Never run `cargo fmt -p jparser`.

- [ ] **Step 7: Confirm the expected dead-code warnings, and only those**

Run: `cd ta && cargo build -p jparser 2>&1 | grep "never" | sort`

Expected: exactly these **seven** lines, in some order —

```text
constant `SKIP_CHAR` is never used
constant `SKIP_KANJI_EXTRA` is never used
function `backtrack` is never used
function `segment` is never used
struct `Cell` is never constructed
struct `Segmentation` is never constructed
struct `Span` is never constructed
```

rustc's dead-code pass lints every unreachable item individually, not just the
root of the dead chain, so `backtrack` is reported alongside its only caller.
Nothing from `matcher` appears: Task 1's `#[allow(dead_code)]` covers it. These
clear when Task 7 calls `segment` from `parse`. Do not add `#[allow(dead_code)]`
to make them go away.

- [ ] **Step 8: Commit**

```bash
cd ta && git add crates/jparser/src/segment.rs crates/jparser/src/lib.rs
git commit -m "feat: add BoundaryHints and the segmentation DP skeleton

Ports the skip transition of ta-old's FindBestMatches
(Dictionary.cpp:1160-1174) and its backtrack's skip-run coalescing
(:1288-1290), plus the BoundaryHints trait from port design 5.7.

segment() takes its final signature now — a &[char], a match table
bucketed by start position, and Option<&dyn BoundaryHints> — so it is
a pure function with no index and no conjugation table. That is what
lets the DP cost tests run against a hand-built Vec<Vec<Match>> with
no dictionary at all, which is the shape the Phase 1B addendum 4
requires.

The skip relaxation uses a strict >; the match relaxation added next
uses >=. They are two comparisons on purpose: ta-old resolves the two
kinds of tie in opposite directions and the resulting segmentation
differs even when the total cost does not.

Unlike ta-old, skipped runs are emitted as unmatched spans rather than
dropped, so parse() can report unmatched text instead of silently
losing it."
```

---

## Task 5: Match transitions and the eleven remaining scoring constants

**Files:**
- Modify: `ta/crates/jparser/src/segment.rs`

**Interfaces:**
- Consumes:
  - `crate::matcher::Match` (Task 2) — reads `start`, `len`, `flags`, `inexact`
  - `crate::record::WordFlags` (Phase 1A) — `PARTICLE`, `COMMON`, `COMMON_LINE`, `COUNTER`, `IS_NAME`, `contains`, `insert`
  - `crate::kana::{is_digit, is_katakana}` (Phase 1A)
  - `segment::{BoundaryHints, Cell, Span}` (Task 4)
- Produces:
  - private `fn score_match(text: &[char], m: &Match, hints: Option<&dyn BoundaryHints>, base: i32) -> i32`
  - private `fn counter_after_number(text: &[char], start: usize) -> bool`
  - private `fn isolated_katakana_run(text: &[char], start: usize, len: usize) -> bool`
  - private consts `MATCH_BASE`, `PARTICLE_BONUS`, `SINGLE_CHAR_PENALTY`,
    `MID_NUMBER_BREAK`, `COMMON_BONUS`, `COUNTER_AFTER_NUMBER`, `INEXACT_PENALTY`,
    `NAME_DICT_BAD_PER_CHAR`, `NAME_DICT_OK`, `MECAB_BAD_START`, `MECAB_BAD_END`,
    `COUNTER_SKIPPED_SPACES`
  - `segment()` relaxes match transitions; `backtrack` emits matched spans

Contract §5 lists thirteen constants. Two are already in the file from Task 4
(`SKIP_CHAR`, `SKIP_KANJI_EXTRA`); the remaining eleven land here, plus
`COUNTER_SKIPPED_SPACES` for the two space characters the counter lookbehind
walks past (a named const, because the no-magic-numbers rule applies to
characters too).

Every test in this task asserts a **cost**, not just a winner — the addendum's §7
risk: "a scoring constant applied in the wrong branch often leaves the winning
segmentation unchanged on short inputs". Two tests do the reverse and assert the
*winner* at a fixed cost, because the two tie rules are invisible to a cost
assertion by construction.

`NAME_DICT_BAD_PER_CHAR` is the one constant whose arithmetic cannot be observed
through `total_cost`: a bad name match costs at least 510 per char against a
skip's 100 or 500, so it can never win a path. That is precisely what ta-old
intends. Its per-char scaling is therefore asserted by calling `score_match`
directly, which the in-module test can do. **These three `IS_NAME` tests are also
the crate's only coverage of `NAME_DICT_BAD_PER_CHAR` and `NAME_DICT_OK`** —
nothing sets `IS_NAME` in v1, so no end-to-end test can reach them. Do not drop
them to save lines.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` helper block in `segment.rs`, immediately after
`struct AlwaysBad`'s `impl`:

```rust
    struct Marked {
        starts: Vec<usize>,
        ends: Vec<usize>,
    }
    impl BoundaryHints for Marked {
        fn bad_start(&self, pos: usize) -> bool {
            self.starts.contains(&pos)
        }
        fn bad_end(&self, pos: usize) -> bool {
            self.ends.contains(&pos)
        }
    }

    fn marked(starts: &[usize], ends: &[usize]) -> Marked {
        Marked {
            starts: starts.to_vec(),
            ends: ends.to_vec(),
        }
    }
```

and immediately after `fn no_matches`:

```rust
    fn plain(text: &[char], start: usize, len: usize, flags: WordFlags) -> Match {
        Match {
            start,
            len,
            src_len: len,
            surface: text[start..start + len].iter().collect(),
            flags,
            entry_id: 1,
            inexact: false,
            chain: Vec::new(),
        }
    }

    fn buckets(text: &[char], ms: Vec<Match>) -> Vec<Vec<Match>> {
        let mut out = vec![Vec::new(); text.len()];
        for m in ms {
            out[m.start].push(m);
        }
        out
    }
```

Then append these twenty tests at the end of `mod tests`, after
`hints_never_change_a_skip`:

```rust
    // ---- matches and the scoring constants ------------------------------

    #[test]
    fn a_plain_match_costs_only_the_base() {
        // MATCH_BASE 10, versus 200 for skipping both characters.
        let text = chars("ねこ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::default())]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 10);
        assert_eq!(shape(&seg), vec![(0, 2, true)]);
        assert_contiguous(&seg, 2);
    }

    #[test]
    fn a_single_char_non_particle_pays_the_penalty() {
        // MATCH_BASE 10 + SINGLE_CHAR_PENALTY 1 = 11.
        let text = chars("ね");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 11);
    }

    #[test]
    fn a_particle_takes_the_bonus_instead_of_the_single_char_penalty() {
        // MATCH_BASE 10 + PARTICLE_BONUS -2 = 8. Two independent `if`s would
        // give 9; the else-if chain gives 8.
        let text = chars("は");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::PARTICLE)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 8);
    }

    #[test]
    fn a_single_char_between_two_digits_pays_the_penalty_not_the_break() {
        // The chain's leg ORDER, not just its existence: `m.len == 1` pre-empts
        // the mid-number leg, so 100 (skip '1') + 10 + 1 = 111, never 210.
        // Swapping the two legs gives 210; three independent `if`s give 211.
        let text = chars("12");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 111);
    }

    #[test]
    fn common_and_common_line_each_grant_the_bonus_once() {
        // MATCH_BASE 10 + COMMON_BONUS -3 = 7, for either flag and for both.
        let text = chars("ねこ");
        let mut both = WordFlags::COMMON;
        both.insert(WordFlags::COMMON_LINE);
        for flags in [WordFlags::COMMON, WordFlags::COMMON_LINE, both] {
            let ms = buckets(&text, vec![plain(&text, 0, 2, flags)]);
            assert_eq!(segment(&text, &ms, None).total_cost, 7, "flags {flags:?}");
        }
    }

    #[test]
    fn the_common_bonus_stacks_with_the_particle_bonus() {
        // 10 - 2 - 3 = 5. COMMON_BONUS is its own `if`, not part of the chain.
        let text = chars("は");
        let mut flags = WordFlags::PARTICLE;
        flags.insert(WordFlags::COMMON);
        let ms = buckets(&text, vec![plain(&text, 0, 1, flags)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 5);
    }

    #[test]
    fn starting_a_match_between_two_digits_costs_mid_number_break() {
        // '1' skipped = 100, then MATCH_BASE 10 + MID_NUMBER_BREAK 100 = 210.
        let text = chars("12月");
        let ms = buckets(&text, vec![plain(&text, 1, 2, WordFlags::default())]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 210);
        assert_eq!(shape(&seg), vec![(0, 1, false), (1, 2, true)]);

        // Same match, non-digit predecessor: 100 + 10 = 110. The 100 delta is
        // MID_NUMBER_BREAK and nothing else.
        let text = chars("あ2月");
        let ms = buckets(&text, vec![plain(&text, 1, 2, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 110);
    }

    #[test]
    fn a_counter_after_a_number_takes_its_bonus() {
        // '3' skipped = 100, then 10 + SINGLE_CHAR_PENALTY 1
        // + COUNTER_AFTER_NUMBER -2 = 109.
        let text = chars("3日");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 109);

        // No number in front: 100 + 10 + 1 = 111, a delta of exactly 2.
        let text = chars("あ日");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 111);
    }

    #[test]
    fn the_counter_lookbehind_skips_both_kinds_of_space() {
        // 100 + 100 (two skipped chars) + 10 + 1 - 2 = 209 in both cases.
        for gap in [' ', '\u{3000}'] {
            let text: Vec<char> = vec!['3', gap, '日'];
            let ms = buckets(&text, vec![plain(&text, 2, 1, WordFlags::COUNTER)]);
            assert_eq!(segment(&text, &ms, None).total_cost, 209, "gap {gap:?}");
        }
    }

    #[test]
    fn a_counter_at_position_zero_has_no_number_behind_it() {
        // 10 + SINGLE_CHAR_PENALTY 1 = 11; the lookbehind runs off the front.
        let text = chars("日");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 11);
        assert!(!counter_after_number(&text, 0));
    }

    #[test]
    fn an_inexact_match_pays_the_inexact_penalty() {
        // 10 + INEXACT_PENALTY 10 = 20.
        let text = chars("ねこ");
        let mut m = plain(&text, 0, 2, WordFlags::default());
        m.inexact = true;
        assert_eq!(segment(&text, &buckets(&text, vec![m]), None).total_cost, 20);
    }

    #[test]
    fn boundary_hints_add_ten_at_each_end() {
        let text = chars("ねこ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::default())]);
        // bad_start(0) only: 10 + 10 = 20.
        assert_eq!(segment(&text, &ms, Some(&marked(&[0], &[]))).total_cost, 20);
        // bad_end is tested at start + len - 1 == 1, so a flag on 2 is inert.
        assert_eq!(segment(&text, &ms, Some(&marked(&[0], &[2]))).total_cost, 20);
        // Both ends flagged: 10 + 10 + 10 = 30.
        assert_eq!(segment(&text, &ms, Some(&marked(&[0], &[1]))).total_cost, 30);
        // None must equal an implementation answering false everywhere.
        assert_eq!(
            segment(&text, &ms, Some(&marked(&[], &[]))),
            segment(&text, &ms, None)
        );
    }

    #[test]
    fn an_isolated_katakana_name_takes_name_dict_ok() {
        // 10 + NAME_DICT_OK 5 = 15, then 'だ' skipped: 15 + 100 = 115.
        let text = chars("ネコだ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::IS_NAME)]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 115);
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 1, false)]);
    }

    #[test]
    fn a_name_glued_to_more_katakana_is_priced_out() {
        // A bad name costs at least 510 per char against a 100/500 skip, so it
        // can never win: skipping all three characters costs 300 and the match
        // never appears. That is what the constant is for, and it is why the
        // per-char scaling is asserted through score_match, not total_cost.
        let text = chars("ネコン");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::IS_NAME)]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 300);
        assert_eq!(shape(&seg), vec![(0, 3, false)]);

        // 10 + 500 * 2 and 10 + 500 * 3, on a run long enough that neither
        // span reaches the end of the katakana.
        let long = chars("ネコンド");
        assert_eq!(
            score_match(&long, &plain(&long, 0, 2, WordFlags::IS_NAME), None, 0),
            1010
        );
        assert_eq!(
            score_match(&long, &plain(&long, 0, 3, WordFlags::IS_NAME), None, 0),
            1510
        );
    }

    #[test]
    fn an_inexact_name_is_bad_even_inside_an_isolated_run() {
        // 10 + INEXACT_PENALTY 10 + 500 * 2 = 1020.
        let text = chars("ネコだ");
        let mut m = plain(&text, 0, 2, WordFlags::IS_NAME);
        m.inexact = true;
        assert_eq!(score_match(&text, &m, None, 0), 1020);
    }

    #[test]
    fn isolated_katakana_run_rejects_katakana_on_either_side() {
        let text = chars("ンネコン");
        assert!(!isolated_katakana_run(&text, 1, 2)); // katakana before
        assert!(!isolated_katakana_run(&text, 0, 2)); // katakana after
        assert!(isolated_katakana_run(&text, 0, 4)); // the whole text
        assert!(!isolated_katakana_run(&chars("ネこ"), 0, 2)); // not all katakana
    }

    #[test]
    fn a_match_wins_a_tie_against_an_earlier_match() {
        // Synthetic table chosen so two matches reach position 4 at the same
        // cost. Particle matches at (0,1) and (0,2) both cost 10 - 2 = 8, so
        // best[1] == best[2] == 8; (1,3) and (2,2) then both cost 8 + 10 = 18.
        // The match relaxation uses `>=`, so the LAST writer — the one from the
        // later start — keeps position 4.
        let text = chars("あいうえ");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 0, 1, WordFlags::PARTICLE),
                plain(&text, 0, 2, WordFlags::PARTICLE),
                plain(&text, 1, 3, WordFlags::default()),
                plain(&text, 2, 2, WordFlags::default()),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 18);
        // A strict `>` here would give [(0, 1, true), (1, 3, true)] at the same
        // total cost — the exact regression a cost-only assertion misses.
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 2, true)]);
    }

    #[test]
    fn a_skip_does_not_overwrite_an_equal_cost_match() {
        // '1' skipped = 100. At pos 1 a PARTICLE match (1,1) costs
        // 100 + 10 - 2 = 108, and a COUNTER match (1,2) costs
        // 100 + 10 + MID_NUMBER_BREAK 100 + COUNTER_AFTER_NUMBER -2 = 208.
        // At pos 2 the skip offers 108 + 100 = 208 — an exact tie. The skip
        // relaxation uses a STRICT `>`, so the match keeps position 3.
        let text = chars("12あ");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 1, 1, WordFlags::PARTICLE),
                plain(&text, 1, 2, WordFlags::COUNTER),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 208);
        // With `>=` the skip would take position 2 and the shape would become
        // [(0, 1, false), (1, 1, true), (2, 1, false)] — the (1,2) match loses,
        // at the same total cost.
        assert_eq!(shape(&seg), vec![(0, 1, false), (1, 2, true)]);
    }

    #[test]
    fn a_skipped_run_between_two_matches_becomes_one_unmatched_span() {
        // The backtrack's coalesce-then-continue branch with a matched span on
        // BOTH sides, which the leading/trailing skip tests never reach.
        // 8 + 100 + 8 = 116.
        let text = chars("はあは");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 0, 1, WordFlags::PARTICLE),
                plain(&text, 2, 1, WordFlags::PARTICLE),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 116);
        assert_eq!(shape(&seg), vec![(0, 1, true), (1, 1, false), (2, 1, true)]);
        assert_contiguous(&seg, 3);
    }

    #[test]
    fn every_constant_at_once() {
        // A three-char inexact common counter starting between two digits,
        // with both boundary hints firing:
        // 0 + 10 + 10 + 10 + 100 - 3 - 2 + 10 = 135.
        let text = chars("1２三日か");
        let mut flags = WordFlags::COMMON;
        flags.insert(WordFlags::COUNTER);
        let mut m = plain(&text, 1, 3, flags);
        m.inexact = true;
        assert_eq!(score_match(&text, &m, Some(&marked(&[1], &[3])), 0), 135);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd ta && cargo test -p jparser --lib segment::`
Expected: FAIL to compile — `cannot find function 'score_match' in this scope`,
`cannot find function 'counter_after_number' in this scope`, `cannot find
function 'isolated_katakana_run' in this scope`, and `cannot find type
'WordFlags' in this scope`. The six Task 4 tests do not run.

- [ ] **Step 3: Add the `WordFlags` import**

In `segment.rs`, the `use` block becomes:

```rust
use crate::kana;
use crate::matcher::Match;
use crate::record::WordFlags;
```

- [ ] **Step 4: Add the remaining constants and the scoring function**

Insert between `segment()` and `backtrack()`:

```rust
/// Base cost of using a dictionary match. ta-old `Dictionary.cpp:1179`.
const MATCH_BASE: i32 = 10;
/// `WordFlags::PARTICLE`. First leg of the three-way else-if chain.
/// ta-old `Dictionary.cpp:1187`.
const PARTICLE_BONUS: i32 = -2;
/// Non-particle match of exactly one char. Second leg of the same chain.
/// ta-old `Dictionary.cpp:1190`.
const SINGLE_CHAR_PENALTY: i32 = 1;
/// Starting a non-particle, multi-char match between two digit characters.
/// Third leg of the same chain. ta-old `Dictionary.cpp:1193`.
const MID_NUMBER_BREAK: i32 = 100;
/// `COMMON` **or** `COMMON_LINE`. Independent `if`, stacks with the chain
/// above. ta-old `Dictionary.cpp:1197`.
const COMMON_BONUS: i32 = -3;
/// `COUNTER` preceded (skipping ASCII and ideographic spaces) by a digit.
/// When the test fails the flag is cleared instead, in the backtrack.
/// ta-old `Dictionary.cpp:1204`.
const COUNTER_AFTER_NUMBER: i32 = -2;
/// Source text and dictionary spelling disagree in kana type/width/case.
/// ta-old `Dictionary.cpp:1210`.
const INEXACT_PENALTY: i32 = 10;
/// Per char, for an `IS_NAME` match that is inexact or not an isolated
/// katakana run. Mutually exclusive with `NAME_DICT_OK`. Dormant in v1:
/// nothing sets `IS_NAME`. ta-old `Dictionary.cpp:1232`.
const NAME_DICT_BAD_PER_CHAR: i32 = 500;
/// An `IS_NAME` match that *is* an isolated exact katakana run. Dormant in v1.
/// ta-old `Dictionary.cpp:1234`.
const NAME_DICT_OK: i32 = 5;
/// `BoundaryHints::bad_start(m.start)`. ta-old `Dictionary.cpp:1181`.
const MECAB_BAD_START: i32 = 10;
/// `BoundaryHints::bad_end(m.start + m.len - 1)`. ta-old `Dictionary.cpp:1183`.
const MECAB_BAD_END: i32 = 10;

/// ASCII space and ideographic space, skipped when looking behind a counter
/// for its number. ta-old `Dictionary.cpp:1201`.
const COUNTER_SKIPPED_SPACES: [char; 2] = [' ', '\u{3000}'];

/// Cost of extending the path at `m.start` with `m`, ta-old
/// `Dictionary.cpp:1179-1235`. The clause order is load-bearing.
fn score_match(text: &[char], m: &Match, hints: Option<&dyn BoundaryHints>, base: i32) -> i32 {
    let mut s = base.saturating_add(MATCH_BASE);
    if hints.is_some_and(|h| h.bad_start(m.start)) {
        s += MECAB_BAD_START;
    }
    if hints.is_some_and(|h| h.bad_end(m.start + m.len - 1)) {
        s += MECAB_BAD_END;
    }

    // One three-way else-if chain, not three independent tests, and the legs
    // are in ta-old's order: PARTICLE pre-empts len == 1, which pre-empts the
    // mid-number break.
    if m.flags.contains(WordFlags::PARTICLE) {
        s += PARTICLE_BONUS;
    } else if m.len == 1 {
        s += SINGLE_CHAR_PENALTY;
    } else if m.start > 0 && kana::is_digit(text[m.start]) && kana::is_digit(text[m.start - 1]) {
        s += MID_NUMBER_BREAK;
    }

    // `contains` is an exact-subset test, so this must be two calls: one call
    // with both bits set would require the match to carry both.
    if m.flags.contains(WordFlags::COMMON) || m.flags.contains(WordFlags::COMMON_LINE) {
        s += COMMON_BONUS;
    }
    if m.flags.contains(WordFlags::COUNTER) && counter_after_number(text, m.start) {
        s += COUNTER_AFTER_NUMBER;
    }
    if m.inexact {
        s += INEXACT_PENALTY;
    }
    if m.flags.contains(WordFlags::IS_NAME) {
        let bad = m.inexact || !isolated_katakana_run(text, m.start, m.len);
        s += if bad {
            NAME_DICT_BAD_PER_CHAR * m.len as i32
        } else {
            NAME_DICT_OK
        };
    }
    s
}

/// Skip spaces backwards from `start - 1`; true when the first non-space char
/// found is in bounds and is a digit. ta-old `Dictionary.cpp:1200-1205`.
fn counter_after_number(text: &[char], start: usize) -> bool {
    let mut i = start;
    while i > 0 {
        i -= 1;
        if !COUNTER_SKIPPED_SPACES.contains(&text[i]) {
            return kana::is_digit(text[i]);
        }
    }
    false
}

/// Every char of the span is katakana and the span is not glued to more
/// katakana on either side. ta-old `Dictionary.cpp:1214-1231`.
fn isolated_katakana_run(text: &[char], start: usize, len: usize) -> bool {
    let end = start + len;
    if !text[start..end].iter().all(|c| kana::is_katakana(*c)) {
        return false;
    }
    if start > 0 && kana::is_katakana(text[start - 1]) {
        return false;
    }
    if end < text.len() && kana::is_katakana(text[end]) {
        return false;
    }
    true
}
```

Four things that are wrong in the obvious implementation and right here:

- `PARTICLE` / `len == 1` / mid-number are **one** else-if chain
  (`Dictionary.cpp:1186-1193`), **in that order**. A single-char particle must
  cost `-2`, not `-2 + 1`; a single-char match between two digits must cost `+1`,
  not `+100`.
- `COMMON_BONUS` is a separate `if` and fires on **either** flag. It cannot be
  written as `contains(WordFlags(COMMON.0 | COMMON_LINE.0))` — `contains` is an
  exact-subset test and that call would require both bits.
- The mid-number test reads the match's own start character and its predecessor
  in the *text*, not digits inside the matched span.
- `bad_end` is queried at `m.start + m.len - 1`, the last char *inside* the span,
  never at the position after it.

- [ ] **Step 5: Relax match transitions in `segment()`**

Delete the two lines

```rust
    // Hints price matches only; the skip transition never consults them.
    let _ = hints;
```

and append this inside the `for pos in 0..n` loop, after the skip block:

```rust
        // 2. Every match starting here, in bucket order.
        for m in &matches[pos] {
            let cost = score_match(text, m, hints, best[pos].cost);
            let next = pos + m.len;
            // `>=`: on a tie the LAST writer wins (`Dictionary.cpp:1255`).
            // Deliberately a different comparison from the skip above; do not
            // route both through one shared helper.
            if best[next].cost >= cost {
                best[next] = Cell {
                    cost,
                    back_len: m.len,
                };
            }
        }
```

**Do not re-sort a bucket.** Not because within-bucket order changes this loop's
outcome — it cannot: two matches in the same bucket reaching the same `next`
necessarily have the same `len`, hence write the same `back_len`, so at equal
cost the resulting `Cell` is bit-identical. Bucket order is load-bearing one step
later: `sort_matches`' Pass C is a *stable* sort whose remaining ties keep matcher
emission order (contract §6.5), and the matcher's own `commit` dedup depends on
the order candidates arrive in. Re-sorting here silently reorders the alternatives
list every span shows.

- [ ] **Step 6: Emit matched spans from the backtrack**

Replace `backtrack` in full:

```rust
/// Walk the backpointers from the end, ta-old `Dictionary.cpp:1280-1305`.
fn backtrack(best: &[Cell], n: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = n;
    while pos > 0 {
        if best[pos].back_len == 0 {
            // Coalesce the whole run of skipped chars into one unmatched span.
            // ta-old emitted nothing for these (`Dictionary.cpp:1288-1290`);
            // the port emits them so `parse` can return unmatched `Segment`s.
            let end = pos;
            while pos > 0 && best[pos].back_len == 0 {
                pos -= 1;
            }
            spans.push(Span {
                start: pos,
                len: end - pos,
                matched: false,
                matches: Vec::new(),
            });
            continue;
        }
        let len = best[pos].back_len;
        let start = pos - len;
        spans.push(Span {
            start,
            len,
            matched: true,
            matches: Vec::new(),
        });
        pos = start;
    }
    spans.reverse();
    spans
}
```

The matched span's `matches` list stays empty until Task 6 collects it; that is
what makes Task 6's first test fail for the right reason. `back_len == 0`
unambiguously means "skip" only because `matches_at` drops `len == 0` candidates
(contract §6.1's zero-length drop) — if that post-filter is ever removed, a
zero-length match becomes a DP self-loop.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser --lib segment::`
Expected: PASS, 26 tests.

- [ ] **Step 8: Format and check the file size**

Run: `cd ta && rustfmt --edition 2021 crates/jparser/src/segment.rs && wc -l crates/jparser/src/segment.rs`
Expected: approximately **692** lines (667 for the eighteen tests the cost model
strictly needs, plus the two coverage tests added above), under the 800 cap. Task
6 adds ~75 more; if this reading is already above 720, stop and move
`counter_after_number`/`isolated_katakana_run` and their two tests into `rank.rs`
before continuing.

- [ ] **Step 9: Commit**

```bash
cd ta && git add crates/jparser/src/segment.rs
git commit -m "feat: score match transitions with ta-old's cost model

Ports the match relaxation and every scoring clause of FindBestMatches
(Dictionary.cpp:1176-1262), keeping the constants verbatim: they are
tuned against real Japanese and are not to be re-derived by feel.

Three shapes that are easy to get approximately right and are pinned
by tests here: PARTICLE/single-char/mid-number is one else-if chain in
that order, so a single-char particle costs -2 and a single-char match
between two digits costs +1; COMMON_BONUS is a separate if that fires
on either COMMON or COMMON_LINE, which cannot use a combined mask
because WordFlags::contains is an exact-subset test; and bad_end is
asked about start + len - 1, the last character inside the span.

Tests assert total cost rather than only the winning segmentation, per
port design 10 — a constant applied in the wrong branch usually leaves
the winner unchanged on short inputs. The two tie rules are the
mirror image: they are invisible to a cost assertion, so both have a
test that fixes the cost and asserts the spans.

NAME_DICT_BAD_PER_CHAR can never win a path (510+ per char against a
100/500 skip), so its per-char scaling is asserted through score_match
directly. Those three IS_NAME tests are the only coverage either
NAME_DICT_* constant will get until JMnedict lands."
```

---

## Task 6: The backtrack's collection pass and `sort_matches`

**Files:**
- Create: `ta/crates/jparser/src/rank.rs`
- Modify: `ta/crates/jparser/src/segment.rs`
- Modify: `ta/crates/jparser/src/lib.rs`

**Interfaces:**
- Consumes:
  - `crate::matcher::{Match, ConjLink, same_except_inexact}` (Tasks 1–2)
  - `crate::conjugation::{TENSE_NON_PAST, Form, TenseId}` (Phase 1A)
  - `crate::record::WordFlags` (Phase 1A)
  - `segment::counter_after_number` (Task 5) — consumed by the new
    `segment::clear_stale_counter_flags`, **not** by `rank.rs`, which knows
    nothing about the text
- Produces:
  - `pub(crate) fn rank::sort_matches(matches: &mut Vec<Match>)`
  - private `rank::RANK_FLAG_MASK`, `rank::group_key`, `rank::verb_plain_collapses`
  - private `segment::clear_stale_counter_flags(text: &[char], group: &mut [Match])`
  - `segment::backtrack` gains `text` and `matches` parameters and fills `Span::matches`

Two behaviours the port would otherwise lose silently.

**The collection pass.** Addendum §7: "collect every match aligning to a chosen
span, not only the winners. Skipping it yields a single guess per segment and
silently removes the alternative readings the definition list exists to show."
The DP records one backpointer; the backtrack re-reads bucket `start` and takes
*every* match of that length.

**The counter flag clear.** ta-old mutated the shared `Match` in place
(`Dictionary.cpp:1206`) so its later `SortMatches` could not promote a counter
reading that was not actually preceded by a number. `segment` takes
`&[Vec<Match>]` and must not mutate its input, so contract §6.4 recomputes the
predicate on the span's clones. It is a pure function of `(text, start)`, so the
answer is the one the DP already used. Dropping it regresses candidate *ordering*,
not the segmentation — exactly the kind of regression nobody notices.

**Module-map deviation, flagged not silent.** `sort_matches` is specified to live
in `segment.rs` (contract §7). It does not fit under the 800-line cap once the
required in-module tests are counted: merged back, `segment.rs` measures **≈990**
lines (740 + 272 − 1 for the dropped `use crate::rank::sort_matches;` − 21 for the
duplicated GPL header, module doc, `use` block, and test-module scaffolding). It
moves to `src/rank.rs` with the same name, signature, visibility, and behaviour.
See the File Structure rationale, which is authoritative over contract §7's
module map and its test-home table for this row.

- [ ] **Step 1: Write the failing tests for `rank.rs`**

Create `ta/crates/jparser/src/rank.rs` containing the GPL v2 header and only this
test module:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{Form, TenseId};
    use crate::matcher::ConjLink;

    /// Any tense that is not Non-past. The static table's index 4 is "Past".
    const TENSE_PAST: TenseId = 4;

    /// A non-verb candidate on the fixed span (0, 2).
    fn plain(entry_id: u32, flags: WordFlags) -> Match {
        Match {
            start: 0,
            len: 2,
            src_len: 2,
            surface: "ある".to_string(),
            flags,
            entry_id,
            inexact: false,
            chain: Vec::new(),
        }
    }

    /// The same span, reached as a one-link conjugation of `entry_id`.
    fn verb(entry_id: u32, tense: TenseId, form: u8) -> Match {
        Match {
            chain: vec![ConjLink {
                verb_type: 0,
                tense,
                form: Form(form),
                conj: 0,
            }],
            ..plain(entry_id, WordFlags::default())
        }
    }

    fn ids(ms: &[Match]) -> Vec<u32> {
        ms.iter().map(|m| m.entry_id).collect()
    }

    #[test]
    fn reconciles_an_inexact_pair_into_one_exact_match() {
        let exact = plain(1, WordFlags::default());
        let mut fuzzy = exact.clone();
        fuzzy.inexact = true;
        let mut ms = vec![fuzzy, exact];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 1);
        assert!(!ms[0].inexact);
    }

    #[test]
    fn keeps_distinct_entries_with_identical_surfaces() {
        let mut ms = vec![
            plain(4, WordFlags::default()),
            plain(5, WordFlags::default()),
        ];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![4, 5]);
    }

    #[test]
    fn drops_a_plain_non_past_verb_beside_its_non_verb_twin() {
        let mut ms = vec![plain(1, WordFlags::default()), verb(1, TENSE_NON_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 1);
        assert!(ms[0].chain.is_empty());
    }

    #[test]
    fn keeps_the_verb_when_a_third_candidate_follows() {
        // The lookahead reads the uncompacted tail: another candidate for the
        // same entry blocks the collapse.
        let mut ms = vec![
            plain(1, WordFlags::default()),
            verb(1, TENSE_NON_PAST, 0),
            verb(1, TENSE_PAST, 0),
        ];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 3);
    }

    #[test]
    fn does_not_collapse_a_conjugated_verb() {
        let mut ms = vec![plain(1, WordFlags::default()), verb(1, TENSE_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 2);
    }

    #[test]
    fn ranks_exact_before_inexact() {
        let mut fuzzy = plain(1, WordFlags::default());
        fuzzy.inexact = true;
        let mut ms = vec![fuzzy, plain(2, WordFlags::default())];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn ranks_by_flag_bit_value_not_popcount() {
        // COUNTER alone (0x20) outranks PRIMARY|COMMON|COMMON_LINE (0x0D).
        assert_eq!(RANK_FLAG_MASK, 0x003D);
        let mut many = WordFlags::PRIMARY;
        many.insert(WordFlags::COMMON);
        many.insert(WordFlags::COMMON_LINE);
        let mut ms = vec![plain(1, many), plain(2, WordFlags::COUNTER)];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn ranks_non_name_before_name() {
        let mut ms = vec![plain(1, WordFlags::IS_NAME), plain(2, WordFlags::default())];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn breaks_remaining_ties_by_entry_id_ascending() {
        let mut ms = vec![
            plain(9, WordFlags::default()),
            plain(3, WordFlags::default()),
        ];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![3, 9]);
    }

    #[test]
    fn orders_conjugation_forms_ascending() {
        // form 0 (informal affirmative) before form 3 (formal negative).
        let mut ms = vec![verb(1, TENSE_PAST, 3), verb(1, TENSE_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms[0].chain[0].form, Form(0));
        assert_eq!(ms[1].chain[0].form, Form(3));
    }

    #[test]
    fn handles_zero_and_one_element_lists() {
        let mut empty: Vec<Match> = Vec::new();
        sort_matches(&mut empty);
        assert!(empty.is_empty());
        let mut one = vec![plain(1, WordFlags::default())];
        sort_matches(&mut one);
        assert_eq!(one.len(), 1);
    }
}
```

- [ ] **Step 2: Write the failing tests for the collection pass**

Append to `mod tests` in `segment.rs`, after `every_constant_at_once`:

```rust
    // ---- the backtrack's collection pass --------------------------------

    #[test]
    fn the_backtrack_collects_every_match_on_the_chosen_span() {
        // Two entries share (0, 2); a third match at the same start but a
        // different length must not be collected.
        let text = chars("ねこだ");
        let mut a = plain(&text, 0, 2, WordFlags::default());
        a.entry_id = 7;
        let mut b = plain(&text, 0, 2, WordFlags::default());
        b.entry_id = 9;
        let mut c = plain(&text, 0, 1, WordFlags::default());
        c.entry_id = 11;
        let seg = segment(&text, &buckets(&text, vec![a, b, c]), None);
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 1, false)]);
        let ids: Vec<u32> = seg.spans[0].matches.iter().map(|m| m.entry_id).collect();
        assert_eq!(ids, vec![7, 9]);
    }

    #[test]
    fn a_stale_counter_flag_is_cleared_on_the_emitted_match() {
        // The counter is not preceded by a number, so COUNTER is cleared and
        // the COMMON candidate outranks it.
        let text = chars("日");
        let mut counter = plain(&text, 0, 1, WordFlags::COUNTER);
        counter.entry_id = 2;
        let mut common = plain(&text, 0, 1, WordFlags::COMMON);
        common.entry_id = 3;
        let seg = segment(&text, &buckets(&text, vec![counter, common]), None);
        let got = &seg.spans[0].matches;
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].entry_id, 3, "COMMON must outrank a stale COUNTER");
        assert!(!got[1].flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn a_live_counter_flag_survives_and_outranks() {
        let text = chars("3日");
        let mut counter = plain(&text, 1, 1, WordFlags::COUNTER);
        counter.entry_id = 2;
        let mut common = plain(&text, 1, 1, WordFlags::COMMON);
        common.entry_id = 3;
        let seg = segment(&text, &buckets(&text, vec![counter, common]), None);
        let got = &seg.spans[1].matches;
        assert_eq!(got[0].entry_id, 2);
        assert!(got[0].flags.contains(WordFlags::COUNTER));
    }
```

- [ ] **Step 3: Register `rank` and run the tests to verify they fail**

Add `mod rank;` to `lib.rs` between `mod matcher;` and `pub mod record;`, then
run: `cd ta && cargo test -p jparser --lib`

Expected: FAIL to compile in `rank.rs` — `cannot find function 'sort_matches' in
this scope`, `cannot find type 'Match' in this scope`, `cannot find type
'WordFlags' in this scope`, `cannot find value 'RANK_FLAG_MASK' in this scope`,
`cannot find value 'TENSE_NON_PAST' in this scope`.

Once `rank.rs` compiles (after Step 4), re-running shows the real failure in
`segment.rs`: `the_backtrack_collects_every_match_on_the_chosen_span` panics with
``assertion `left == right` failed: left: [], right: [7, 9]`` — Task 5 emits
matched spans with an empty `matches` list.

- [ ] **Step 4: Implement `rank.rs`**

Insert above the test module, between the GPL v2 header and `#[cfg(test)]`:

```rust
//! `sort_matches`, ta-old's `SortMatches` (`Dictionary.cpp:1025-1063`): dedupe
//! the candidates on one chosen span, then rank them so the definition list
//! shows the best reading first.
//!
//! Split out of `segment.rs` for file size only — see the plan's File Structure
//! note. Nothing here knows about the DP or about the text; it is a pure
//! ordering pass over candidates that already share a `(start, len)`.

use crate::conjugation::TENSE_NON_PAST;
use crate::matcher::{same_except_inexact, Match};
use crate::record::WordFlags;

/// Flag bits `CompareMatches` ranks on, compared descending as a raw integer —
/// bit-value priority, not popcount. ta-old `Dictionary.cpp:1013`. The
/// `JAP_WORD_TOP` variant of the mask is behind `#ifdef SETSUMI_CHANGES`,
/// which ta-old never defines, and is correctly excluded.
const RANK_FLAG_MASK: u16 = WordFlags::COUNTER.0
    | WordFlags::PARTICLE.0
    | WordFlags::COMMON.0
    | WordFlags::COMMON_LINE.0
    | WordFlags::PRIMARY.0;

/// Pass A's grouping key. A non-verb keys as type 0 so it sorts before every
/// verb of the same entry, mirroring ta-old's 1-based `verbType`.
fn group_key(m: &Match) -> (u32, usize, usize, u8) {
    match m.chain.first() {
        Some(l) => (m.entry_id, l.verb_type + 1, l.tense, l.form.0),
        None => (m.entry_id, 0, 0, 0),
    }
}

/// True when `matches[i]` is a verb's plain informal non-past sitting on the
/// same slot as a non-verb hit for the same entry, with no third candidate
/// following. ta-old `Dictionary.cpp:1046-1056`. The lookahead reads the
/// **uncompacted** tail, exactly as ta-old's does.
fn verb_plain_collapses(matches: &[Match], i: usize, d: usize) -> bool {
    let cur = &matches[i];
    let kept = &matches[d - 1];
    let Some(link) = cur.chain.first() else {
        return false;
    };
    cur.entry_id == kept.entry_id
        && kept.chain.is_empty()
        && link.form.0 == 0
        && link.tense == TENSE_NON_PAST
        && matches
            .get(i + 1)
            .map_or(true, |nx| nx.entry_id != cur.entry_id)
}

/// Group, dedupe with inexact reconciliation, then rank. Called per span, so
/// every element already shares `(start, len)` and identity is `entry_id`
/// alone — it carries both of ta-old's identity fields.
///
/// The compaction is deliberately the original's adjacency-only one-behind
/// scan. It is **not** a global group-by; the port is bug-for-bug compatible
/// here on purpose.
pub(crate) fn sort_matches(matches: &mut Vec<Match>) {
    if matches.is_empty() {
        return;
    }

    // Pass A — group sort, ta-old's CompareIdenticalMatches. Its weighted
    // integer key is lexicographic (verbType, verbTense, verbForm) ascending,
    // so the tuple below is a faithful — and non-overflowing — port.
    matches.sort_by_key(group_key);

    // Pass B — one-behind compaction, write cursor `d`.
    let mut d = 1;
    for i in 1..matches.len() {
        // 1. Inexact reconciliation (`Dictionary.cpp:1031-1042`): a run of one
        //    entry disagreeing on `inexact` is forced to exact, which is what
        //    lets step 2 see the run as duplicates at all.
        let mut j = i;
        let mut k = d - 1;
        while matches[j].entry_id == matches[k].entry_id && matches[j].inexact != matches[k].inexact
        {
            matches[j].inexact = false;
            matches[k].inexact = false;
            j = k;
            if k == 0 {
                break;
            }
            k -= 1;
        }

        // 2. Exact-duplicate drop.
        if same_except_inexact(&matches[i], &matches[d - 1]) {
            continue;
        }

        // 3. Verb-plain vs non-verb collapse.
        if verb_plain_collapses(matches, i, d) {
            continue;
        }

        // 4. Keep. `swap` rather than ta-old's `matches[d++] = matches[i]`:
        //    behaviourally identical here, because the step-3 lookahead only
        //    reads indices > i and the step-1 walk only reads indices < d,
        //    neither of which a swap disturbs — and it avoids a clone.
        matches.swap(d, i);
        d += 1;
    }
    matches.truncate(d);

    // Pass C — final rank, ta-old's CompareMatches minus `start` (constant
    // within a span). Stable, so ties keep matcher emission order.
    matches.sort_by(|a, b| {
        a.inexact
            .cmp(&b.inexact)
            .then(
                a.flags
                    .contains(WordFlags::IS_NAME)
                    .cmp(&b.flags.contains(WordFlags::IS_NAME)),
            )
            .then((b.flags.0 & RANK_FLAG_MASK).cmp(&(a.flags.0 & RANK_FLAG_MASK)))
            .then(a.entry_id.cmp(&b.entry_id))
            .then(
                a.chain
                    .first()
                    .map_or(0, |l| l.form.0)
                    .cmp(&b.chain.first().map_or(0, |l| l.form.0)),
            )
    });
}
```

Four things a "cleaner" rewrite gets wrong:

- Pass B compares only against the single most recently *kept* element
  (`matches[d - 1]`). ta-old's scan is adjacency-only and can miss a duplicate
  that ended up non-adjacent. Reproduce it; do not upgrade it to a group-by.
- The step-3 lookahead reads `matches[i + 1]` from the **uncompacted** tail, not
  from the compacted prefix at indices `< d`.
- `map_or`, not `Option::is_none_or` — the workspace MSRV is 1.75 and
  `is_none_or` needs 1.82.
- Key 3 is **descending** (`b` before `a`) while every other key is ascending. A
  uniformly-ascending port silently inverts candidate priority for the flag mask.
  Keys 4 and 5 replace ta-old's `dictIndex`/`firstJString`, which have no port
  analogue: `entry_id` ascending is the contract §6.5 substitute, chosen because a
  heap-address ordering was never reproducible.

- [ ] **Step 5: Add the counter clear and the collection pass to `segment.rs`**

Insert `clear_stale_counter_flags` immediately before `backtrack`:

```rust
/// For every match carrying `COUNTER` whose `counter_after_number` test fails,
/// clear the flag, so a counter reading that was not actually preceded by a
/// number cannot be promoted by `sort_matches`. ta-old mutated the shared
/// `Match` in place (`Dictionary.cpp:1206`); `segment` must not mutate its
/// input, so the predicate is recomputed here on the span's clones. It is a
/// pure function of `(text, start)`, so the answer is the one the DP used.
fn clear_stale_counter_flags(text: &[char], group: &mut [Match]) {
    for m in group.iter_mut() {
        if m.flags.contains(WordFlags::COUNTER) && !counter_after_number(text, m.start) {
            m.flags.remove(WordFlags::COUNTER);
        }
    }
}
```

Add the import `use crate::rank::sort_matches;` to the `use` block, which becomes:

```rust
use crate::kana;
use crate::matcher::Match;
use crate::rank::sort_matches;
use crate::record::WordFlags;
```

Change the call in `segment()` from `spans: backtrack(&best, n)` to:

```rust
        spans: backtrack(text, matches, &best, n),
```

and replace `backtrack`'s signature line and its matched branch:

```rust
/// Walk the backpointers from the end, ta-old `Dictionary.cpp:1280-1305`.
fn backtrack(text: &[char], matches: &[Vec<Match>], best: &[Cell], n: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = n;
    while pos > 0 {
        if best[pos].back_len == 0 {
            // Coalesce the whole run of skipped chars into one unmatched span.
            // ta-old emitted nothing for these (`Dictionary.cpp:1288-1290`);
            // the port emits them so `parse` can return unmatched `Segment`s.
            let end = pos;
            while pos > 0 && best[pos].back_len == 0 {
                pos -= 1;
            }
            spans.push(Span {
                start: pos,
                len: end - pos,
                matched: false,
                matches: Vec::new(),
            });
            continue;
        }
        let len = best[pos].back_len;
        let start = pos - len;
        // Collect EVERY match aligning to the chosen span, not only the DP
        // winner (`Dictionary.cpp:1280-1299`). This is what populates the
        // alternative readings.
        let mut group: Vec<Match> = matches[start]
            .iter()
            .filter(|m| m.len == len)
            .cloned()
            .collect();
        clear_stale_counter_flags(text, &mut group);
        sort_matches(&mut group);
        spans.push(Span {
            start,
            len,
            matched: true,
            matches: group,
        });
        pos = start;
    }
    spans.reverse();
    spans
}
```

The filter is `m.len == len`, not "the match the DP picked": a match at the same
start with a *different* length is not on the chosen path and must not appear.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser --lib segment::`
Expected: PASS, 29 tests.

Run: `cd ta && cargo test -p jparser --lib rank::`
Expected: PASS, 11 tests.

- [ ] **Step 7: Run the whole crate's tests**

Run: `cd ta && cargo test -p jparser`
Expected: PASS. Nothing in Phase 1A or in Tasks 1–3 changes; the two new modules
only add tests. `cargo build`'s dead-code warning list grows to include
`sort_matches`, `group_key`, `verb_plain_collapses`, `RANK_FLAG_MASK` and
`clear_stale_counter_flags` — `rank` is reachable only from the still-dead
`segment`. They all clear in Task 7; do not silence them.

- [ ] **Step 8: Format and check both file sizes**

```bash
cd ta && rustfmt --edition 2021 crates/jparser/src/segment.rs crates/jparser/src/rank.rs \
  && wc -l crates/jparser/src/segment.rs crates/jparser/src/rank.rs
```

Expected: `segment.rs` around **765** and `rank.rs` **272**, both under the 800
cap. Merged into one file they would be ≈990. `segment.rs` now has only ~35 lines
of headroom: **the next task to add anything to it must check `wc -l` first.**
Do not run `cargo fmt -p jparser`.

- [ ] **Step 9: Commit**

```bash
cd ta && git add crates/jparser/src/rank.rs crates/jparser/src/segment.rs \
  crates/jparser/src/lib.rs
git commit -m "feat: collect every aligning match per span and rank them

Ports the backtrack's collection pass (Dictionary.cpp:1280-1299) and
SortMatches (:1025-1063). The DP records one backpointer per position;
the backtrack re-reads the bucket and keeps every match of the chosen
length. Without it each segment carries a single guess and the
alternative readings the definition list exists to show disappear
silently.

ta-old cleared a stale COUNTER flag by mutating the shared Match in
place so its later SortMatches could not promote a counter reading
that no number preceded. segment() takes &[Vec<Match>] and must not
mutate its input, so the predicate — a pure function of (text, start),
the same one the DP scored with — is recomputed on the span's clones.

sort_matches reproduces ta-old's adjacency-only one-behind compaction
bug-for-bug, including the lookahead into the uncompacted tail, rather
than 'improving' it into a group-by. In the final rank the flag-mask
key is descending while every other key ascends; ta-old's dictIndex
and firstJString keys have no port analogue and are replaced by
entry_id ascending, which is deterministic where a heap address never
was.

sort_matches lives in rank.rs, not segment.rs as the interface
contract's module map says: merged back, segment.rs measures about 990
lines against an 800-line hard cap. Same name, signature, visibility,
and behaviour; only the file differs. This is the size-driven split the
contract already anticipates for entry.rs, flagged rather than taken
silently."
```

---

## Task 7: `EntryData::readings`, the `parse()` surface, and entry assembly

**Files:**
- Modify: `ta/crates/jparser/src/index/mod.rs`
- Modify: `ta/crates/jparser/src/index/build.rs`
- Modify: `ta/crates/jparser/src/lib.rs`
- Create: `ta/crates/jparser/tests/parse_irregular.rs`

**Interfaces:**

- Consumes, from Phase 1A:
  - `pub fn index::load::Index::entry(&self, id: u32) -> Result<Option<EntryData>, IndexError>`
  - `pub struct index::SenseData { pub pos: Vec<String>, pub glosses: Vec<String>, pub xrefs: Vec<String>, pub misc: Vec<String>, pub info: Vec<String> }`
  - `pub struct conjugation::ConjugationTable` with `types() -> &[VerbType]`,
    `tense_name(TenseId) -> Option<&str>`, `types_named(&str) -> Vec<VerbTypeId>`
  - `pub struct conjugation::VerbType { pub name: String, pub remove_tense: TenseId, pub conjugations: Vec<Conjugation>, .. }`,
    `pub struct conjugation::Conjugation { pub tense: TenseId, pub form: Form, pub suffix: String, pub next_verb_type: Option<VerbTypeId> }`
  - `pub fn kana::strip_suffix_unified(surface: &str, suffix: &str) -> Option<String>`
  - `pub fn kana::is_cjk_ideograph(c: char) -> bool`
  - `pub struct record::WordFlags(pub u16)` with `contains`, and `WordFlags::PRONOUNCE`
- Consumes, from Tasks 1–6:
  - `matcher::{ConjLink, Match, matches_at, render_conjugation_label, unified_eq}`
  - `segment::{segment, Segmentation, Span, BoundaryHints}` — `Span::matches` is
    already `sort_matches`-ranked and is always empty when `!matched`
- Produces:
  - `pub const index::INDEX_FORMAT_VERSION: u32 = 3` and
    `pub struct index::EntryData { pub id: u32, pub readings: Vec<String>, pub senses: Vec<SenseData> }`
  - `pub struct ParseOptions {}` — `#[non_exhaustive]`, `Debug + Clone + Default`
  - `pub struct ParseResult { pub segments: Vec<Segment> }`
  - `pub struct Segment { pub start: usize, pub len: usize, pub surface: String, pub reading: Option<String>, pub matched: bool, pub entries: Vec<Entry> }`
  - `pub struct Entry { pub headword: String, pub reading: Option<String>, pub conjugation: Option<String>, pub pos: Vec<String>, pub senses: Vec<Sense>, pub flags: WordFlags }`
  - `pub enum ParseError { Index(#[from] crate::index::IndexError) }` (verified, not redefined — Task 1 created it)
  - `pub use crate::index::SenseData as Sense;`, `pub use crate::segment::BoundaryHints;`
  - `pub fn parse(index: &Index, table: &ConjugationTable, text: &str, opts: &ParseOptions, hints: Option<&dyn BoundaryHints>) -> Result<ParseResult, ParseError>`
  - private in `lib.rs`: `entry_data`, `assemble_entry`, `dictionary_form`,
    `reconstruct_reading`, `strip_remove_suffix`, `kuru_hack`, `tails_match`,
    `const KURU_HACK_MAX_CHARS: usize = 3`

**Resolved gaps and documented divergences.**

1. **`EntryData` has no home task in the contract's task list, but §2 mandates the
   change and Task 7 is its first and only consumer.** Step 1 makes it. This is the
   single Phase 1A data-shape change the whole phase is allowed.
2. **§6.6's worked kuruHack example quotes the *untrimmed* asset suffix.** It says
   "`来られる` Potential (suffix `来られる`, Next Type `v1`) → `"こ"`". The loaded
   table stores that suffix **trimmed** — `from_json` strips the target type's
   remove-suffix, and `v1`'s is `る`, so the stored suffix is `来られ` and its twin
   is `こられ`. The result is unchanged, but a test written against `来られる` will
   not find the conjugation. The tests below assert against the stored, trimmed
   form and say so.
3. **`dictionary_form` is called "the exact inverse of `stem::generate_stems`" but
   is specified as "first", and the two differ.** `generate_stems` uses the first
   remove-tense/form-0 conjugation that *strips*; §4.4 pins `dictionary_form` to
   the *first* such conjugation, full stop. Three shipped types declare more than
   one — `copula` (`だ`, `である`), `adj-i` (`い`, `し`), `v5uru` (`うる`, `える`) —
   so for a stem generated through the second suffix, `Entry::headword`
   reconstructs with the first: an `adj-i` word whose dictionary form ends in `し`
   renders as one ending in `い`. This task implements the contract's literal
   "first" and documents the divergence in the function's doc comment. Fixing it
   needs data the index does not store (the original headword), so it is a
   contract decision, not an implementation one.
4. **`Segment::reading`'s contract doc says "when the surface is already kana", but
   no kana test is performed.** §6.6's normative steps use `WordFlags::PRONOUNCE`
   and an empty `EntryData::readings` as the two proxies. This task implements §6.6
   exactly and rewords the doc to describe the mechanism rather than the intent, so
   nobody later "fixes" it by adding a `kana::has_japanese` check.
5. **`tests/parse_irregular.rs` is a test file contract §7's test-home table does
   not name** (it lists `tests/parse_snapshots.rs` for parse e2e). It is an
   addition, not a conflict: the 来る regression must exist before Task 9's
   30-sentence corpus does, and it needs a one-entry fixture, not a curated
   dictionary. Flagged here; the File Structure table is authoritative.

**Every offset produced here is a character offset.** The one and only
`&str` → `Vec<char>` conversion is at the top of `parse`; nothing downstream sees
a byte index, and `tests/parse_irregular.rs` pins that with a multi-byte assertion
(`。` at char offset 2 of `来た。`, byte offset 6).

- [ ] **Step 1: Extend `EntryData` and bump the index format version**

Reading reconstruction cannot be built on the current `EntryData`: it stores only
`{ id, senses }`, so there is no way to recover an entry's kana reading from a
match. ta-old walked the entry's `JapString` chain for the `JAP_WORD_PRONOUNCE`
sibling (`JParseWindow.cpp:186-208`); the port has no equivalent.

In `ta/crates/jparser/src/index/mod.rs`:

```rust
pub const INDEX_FORMAT_VERSION: u32 = 3;
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryData {
    pub id: u32,
    /// The entry's `<reb>` forms in document order, stored **only when the
    /// entry also has kanji forms** — exactly the set ta-old flags
    /// `JAP_WORD_PRONOUNCE`. Empty for kana-only entries, where the surface
    /// already is the reading and ta-old renders no furigana.
    pub readings: Vec<String>,
    pub senses: Vec<SenseData>,
}
```

In `ta/crates/jparser/src/index/build.rs`, inside the entry loop, add the field
between `id` and `senses`:

```rust
        entries.push(EntryData {
            id: raw.id,
            readings: if raw.kanji.is_empty() {
                Vec::new()
            } else {
                raw.readings.iter().map(|r| r.text.clone()).collect()
            },
            senses: /* unchanged */,
        });
```

**Field order matters for bincode:** `readings` sits between `id` and `senses`
exactly as written. Nothing else in Phase 1A changes. `tests/index_roundtrip.rs`
still passes untouched — its version test flips a byte rather than comparing
against a literal, and it asserts `index.header().version ==
INDEX_FORMAT_VERSION`. Any index built by Phase 1A now fails `Index::open` with
`VersionMismatch { found: 2, expected: 3 }`, which is the intent.

Run: `cd ta && cargo test -p jparser`
Expected: PASS, unchanged test count. The round-trip of `readings` itself is
asserted end to end by Step 3's `reconstructs_the_reading_of_a_conjugated_irregular_verb`,
which cannot produce `きた` unless the field survives the payload.

- [ ] **Step 2: Write the failing unit tests**

Append this test module to `ta/crates/jparser/src/lib.rs`. It needs no index:
every case is a hand-built `Match` plus a hand-built `EntryData` against the
embedded conjugation table, which is what contract §7's test table asks for.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{ConjugationTable, VerbTypeId};
    use crate::matcher::{ConjLink, Match};

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("the embedded table must load")
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// The single type with this name. Panics for `vk`/`vs`/`v5r-i`/`v5uru`,
    /// which is the point: those four need `vk_kanji`-style disambiguation.
    fn only(t: &ConjugationTable, name: &str) -> VerbTypeId {
        let ids = t.types_named(name);
        assert_eq!(ids.len(), 1, "{name} must name exactly one type");
        ids[0]
    }

    /// The `vk` block whose suffixes are spelled in kanji. `types_named`
    /// returns both blocks and which one comes first is an asset-ordering
    /// detail no test may depend on, so pick it by the property that actually
    /// matters: it is the one with kanji-leading suffixes.
    fn vk_kanji(t: &ConjugationTable) -> VerbTypeId {
        t.types_named("vk")
            .into_iter()
            .find(|&id| {
                t.types()[id].conjugations.iter().any(|c| {
                    c.suffix.chars().next().is_some_and(crate::kana::is_cjk_ideograph)
                })
            })
            .expect("one vk block must carry kanji suffixes")
    }

    /// A `ConjLink` for the first form-0 conjugation of `ty` whose **stored**
    /// suffix is `suffix` and whose chaining state is `chained`.
    ///
    /// Stored suffixes are post-trim: a conjugation naming a `Next Type` had
    /// that type's remove-suffix stripped at load, so `vk`'s Potential is
    /// stored as `来られ`, not the asset's `来られる`. `chained` disambiguates
    /// the pairs that share a spelling — `vk` has both a terminal Non-past
    /// `来る` and a chaining Stem `来る`. It does **not** disambiguate two
    /// chaining rows with the same spelling; where the asset has such a pair
    /// (`来られ` is both Potential and Passive into `v1`), the caller asserts
    /// the tense it got.
    fn link(t: &ConjugationTable, ty: VerbTypeId, suffix: &str, chained: bool) -> ConjLink {
        let vt = &t.types()[ty];
        let conj = vt
            .conjugations
            .iter()
            .position(|c| {
                c.form.0 == 0 && c.suffix == suffix && c.next_verb_type.is_some() == chained
            })
            .unwrap_or_else(|| {
                panic!("{} has no form-0 conjugation {suffix:?} (chained={chained})", vt.name)
            });
        ConjLink {
            verb_type: ty,
            tense: vt.conjugations[conj].tense,
            form: vt.conjugations[conj].form,
            conj,
        }
    }

    fn hit(surface: &str, start: usize, src_len: usize, len: usize, chain: Vec<ConjLink>) -> Match {
        Match {
            start,
            len,
            src_len,
            surface: surface.to_string(),
            flags: WordFlags::PRIMARY,
            entry_id: 1,
            inexact: false,
            chain,
        }
    }

    fn sense(pos: &[&str], glosses: &[&str]) -> Sense {
        Sense {
            pos: pos.iter().map(|s| (*s).to_string()).collect(),
            glosses: glosses.iter().map(|s| (*s).to_string()).collect(),
            xrefs: Vec::new(),
            misc: Vec::new(),
            info: Vec::new(),
        }
    }

    fn make_entry(readings: &[&str], senses: Vec<Sense>) -> EntryData {
        EntryData {
            id: 1,
            readings: readings.iter().map(|s| (*s).to_string()).collect(),
            senses,
        }
    }

    #[test]
    fn kuru_hack_reads_the_kanji_of_a_plain_non_past_suffix() {
        // 来る's Non-past twin is くる. len == len2 == 2, so want == 1 and the
        // substitution is the twin's first character: く.
        let t = table();
        let vk = vk_kanji(&t);
        assert_eq!(
            kuru_hack(&link(&t, vk, "来る", false), &t).as_deref(),
            Some("く")
        );
    }

    #[test]
    fn kuru_hack_reads_a_chained_suffix_after_load_time_trimming() {
        // The asset spells this Potential 来られる with Next Type v1; the
        // loader strips v1's remove-suffix る, so the STORED suffix is 来られ
        // and its twin is こられ. len == len2 == 3, want == 1 → こ.
        //
        // vk's kanji block has TWO form-0 chained rows spelled 来られ —
        // Potential and Passive, both into v1 — and `link` takes whichever the
        // asset lists first. Pin it, because both twins are こられ and the
        // assertion would otherwise pass without testing what its name says.
        let t = table();
        let vk = vk_kanji(&t);
        let l = link(&t, vk, "来られ", true);
        assert_eq!(t.tense_name(l.tense), Some("Potential"));
        assert_eq!(kuru_hack(&l, &t).as_deref(), Some("こ"));
    }

    #[test]
    fn kuru_hack_returns_none_when_the_kana_twin_has_no_such_tense() {
        // 来 Imperfective. The kana vk block goes Imperative → Hypothetical
        // with no Imperfective row at all, so no twin conjugation matches on
        // (tense, form, next_verb_type) and the scan yields nothing. This is
        // a normal outcome, not an error.
        let t = table();
        let vk = vk_kanji(&t);
        assert_eq!(kuru_hack(&link(&t, vk, "来", false), &t), None);
    }

    #[test]
    fn kuru_hack_returns_none_for_a_suffix_that_does_not_start_with_a_kanji() {
        // v1's Non-past is る. The CJK check fires before any twin scan.
        let t = table();
        let v1 = only(&t, "v1");
        assert_eq!(kuru_hack(&link(&t, v1, "る", false), &t), None);
    }

    #[test]
    fn reconstructs_an_irregular_reading_through_the_kana_twin() {
        // 来た: the vk stem is the empty string, chain[0] is the Stem
        // conjugation 来た → v-ta-stem, and the kanji vk type cannot strip
        // itself off くる. kuru_hack pairs it with the kana vk twin's きた,
        // yielding き, then the twin's own stem ("") plus き plus the text
        // after the one kanji ("た") gives きた.
        let t = table();
        let vk = vk_kanji(&t);
        let m = hit("", 0, 0, 2, vec![link(&t, vk, "来た", true)]);
        let data = make_entry(&["くる"], vec![sense(&["vk"], &["to come"])]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("来た")).as_deref(),
            Some("きた")
        );
    }

    #[test]
    fn reconstructs_a_regular_verb_reading_from_the_same_type() {
        // 食べる: the kana spelling たべる conjugates with the same v1 rows,
        // so the same-type path strips る to たべ and re-appends the matched
        // tail text[src_len..len] == る.
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        let data = make_entry(&["たべる"], vec![]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("食べる")).as_deref(),
            Some("たべる")
        );
    }

    #[test]
    fn returns_the_first_reading_for_a_plain_headword_match() {
        // 言う lists いう then ゆう; ta-old rendered the first PRONOUNCE
        // sibling it found walking the entry's spelling chain.
        let t = table();
        let m = hit("言う", 0, 2, 2, vec![]);
        let data = make_entry(&["いう", "ゆう"], vec![]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("言う")).as_deref(),
            Some("いう")
        );
    }

    #[test]
    fn returns_no_reading_for_a_match_that_is_already_a_reading() {
        let t = table();
        let mut m = hit("いう", 0, 2, 2, vec![]);
        m.flags = WordFlags::PRONOUNCE;
        let data = make_entry(&["いう"], vec![]);
        assert_eq!(reconstruct_reading(&m, &data, &t, &chars("いう")), None);
    }

    #[test]
    fn returns_no_reading_for_a_kana_only_entry() {
        // A kana-only entry stores no readings: its surface already is one.
        let t = table();
        let m = hit("は", 0, 1, 1, vec![]);
        let data = make_entry(&[], vec![]);
        assert_eq!(reconstruct_reading(&m, &data, &t, &chars("は")), None);
    }

    #[test]
    fn dictionary_form_restores_the_remove_suffix_of_a_stem() {
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        assert_eq!(dictionary_form(&m, &t), "食べる");
    }

    #[test]
    fn dictionary_form_of_a_plain_headword_is_its_surface() {
        let t = table();
        let m = hit("高い", 0, 2, 2, vec![]);
        assert_eq!(dictionary_form(&m, &t), "高い");
    }

    #[test]
    fn dictionary_form_restores_an_empty_stem() {
        // The whole surface of 来る is its own remove-suffix, so the stem is
        // "" — Phase 1A's empty-key case. The dictionary form must still come
        // back whole.
        let t = table();
        let vk = vk_kanji(&t);
        let m = hit("", 0, 0, 2, vec![link(&t, vk, "来る", false)]);
        assert_eq!(dictionary_form(&m, &t), "来る");
    }

    #[test]
    fn assemble_entry_renders_the_conjugation_label_and_dedupes_pos() {
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        let data = make_entry(
            &["たべる"],
            vec![
                sense(&["v1", "vt"], &["to eat"]),
                sense(&["v1"], &["to live on"]),
            ],
        );
        let e = assemble_entry(&m, &data, &t, &chars("食べる"));
        assert_eq!(e.headword, "食べる");
        assert_eq!(e.reading.as_deref(), Some("たべる"));
        assert_eq!(e.conjugation.as_deref(), Some("Non-past"));
        assert_eq!(e.pos, vec!["v1", "vt"], "first-seen order, deduped");
        assert_eq!(e.senses.len(), 2);
        assert_eq!(e.flags, WordFlags::PRIMARY);
    }

    #[test]
    fn assemble_entry_leaves_conjugation_none_for_a_plain_headword() {
        let t = table();
        let m = hit("高い", 0, 2, 2, vec![]);
        let data = make_entry(&["たかい"], vec![sense(&["adj-i"], &["tall"])]);
        let e = assemble_entry(&m, &data, &t, &chars("高い"));
        assert_eq!(e.headword, "高い");
        assert_eq!(e.conjugation, None);
        assert_eq!(e.reading.as_deref(), Some("たかい"));
        assert_eq!(e.pos, vec!["adj-i"]);
    }

    #[test]
    fn assemble_entry_leaves_conjugation_none_when_the_label_renders_empty() {
        // GetConjString skips Stem at every depth, so a chain of nothing but
        // Stem links renders as "". That is "no label", not an empty label.
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 2, vec![link(&t, v1, "", true)]);
        let data = make_entry(&["たべる"], vec![]);
        assert_eq!(crate::matcher::render_conjugation_label(&m.chain, &t), "");
        let e = assemble_entry(&m, &data, &t, &chars("食べ"));
        assert_eq!(e.conjugation, None);
    }
}
```

- [ ] **Step 3: Write the failing end-to-end test**

Create `ta/crates/jparser/tests/parse_irregular.rs`. This is the standing
regression addendum §6 asks for: 来る is the irregular verb whose stem is the
empty string, so it exercises Phase 1A's empty-FST-key path *and* the kuruHack
twin pairing in one run. The fixture is inline rather than a file under
`tests/fixtures/` because it exists only for this one regression and is four lines
of XML.

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! End-to-end `parse` regression for 来る, the one irregular verb whose kanji
//! reading changes with the conjugation.
//!
//! Two Phase 1 mechanisms meet here and both are easy to break silently:
//! the whole surface of 来る is its own remove-suffix, so its generated stem
//! is the empty string and the only way to reach it is the empty FST key
//! (Phase 1A); and the kanji and kana spellings live in two separate,
//! identically named `vk` blocks, so the reading can only be rebuilt through
//! the kuruHack twin search (contract §6.6).

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::{parse, ParseOptions};

const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY vk "Kuru verb - special class">
]>
<JMdict>
<entry>
<ent_seq>1000040</ent_seq>
<k_ele><keb>来る</keb></k_ele>
<r_ele><reb>くる</reb></r_ele>
<sense><pos>&vk;</pos><gloss>to come</gloss></sense>
</entry>
</JMdict>
"#;

fn open_index(name: &str) -> Index {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &dir,
    )
    .expect("build must succeed");
    Index::open(&dir).expect("open must succeed")
}

#[test]
fn parses_the_dictionary_form_of_an_irregular_verb() {
    let index = open_index("parse-kuru-plain");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来る", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 1);
    let s = &out.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来る"));
    assert!(s.matched);
    assert_eq!(s.reading.as_deref(), Some("くる"));
    // Two matches align to (0, 2): the plain headword 来る and the empty stem
    // conjugated by vk's Non-past. sort_matches' verb-plain collapse
    // (contract §6.5 pass B step 3) drops the second, so exactly one entry
    // survives and it is the unconjugated one.
    assert_eq!(s.entries.len(), 1);
    assert_eq!(s.entries[0].headword, "来る");
    assert_eq!(s.entries[0].conjugation, None);
}

#[test]
fn reconstructs_the_reading_of_a_conjugated_irregular_verb() {
    let index = open_index("parse-kuru-past");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来た", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 1);
    let s = &out.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来た"));
    assert!(s.matched);
    // 来 alone is not a key; the match can only be found by walking the empty
    // key to the "" stem and then matching vk's Stem suffix 来た into
    // v-ta-stem's empty Past. Its reading is only recoverable through the
    // kana vk twin's きた → the substitution き, plus the verbatim tail た.
    assert_eq!(s.reading.as_deref(), Some("きた"));
    assert_eq!(s.entries.len(), 1);
    let e = &s.entries[0];
    assert_eq!(e.headword, "来る");
    assert_eq!(e.reading.as_deref(), Some("きた"));
    assert_eq!(e.conjugation.as_deref(), Some("Past"));
    assert_eq!(e.pos, vec!["vk"]);
    assert_eq!(e.senses[0].glosses, vec!["to come"]);
}

#[test]
fn leaves_an_unmatched_run_with_no_reading_and_no_entries() {
    // 。 is in no dictionary here. ta-old emitted nothing at all for skipped
    // characters; the port emits a matched:false Segment so a caller can
    // rebuild the input verbatim. There is no morphological-analyzer
    // fallback: an unmatched run simply has no reading.
    let index = open_index("parse-unmatched");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来た。", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 2);
    assert!(out.segments[0].matched);
    let tail = &out.segments[1];
    // Char offset 2, not byte offset 6: 来た。 is 3 chars and 9 bytes.
    assert_eq!((tail.start, tail.len, tail.surface.as_str()), (2, 1, "。"));
    assert!(!tail.matched);
    assert_eq!(tail.reading, None);
    assert!(tail.entries.is_empty());
}

#[test]
fn parses_empty_text_into_no_segments() {
    let index = open_index("parse-empty");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "", &ParseOptions::default(), None).unwrap();
    assert!(out.segments.is_empty());
}
```

- [ ] **Step 4: Run both test sets to verify they fail**

Run: `cd ta && cargo test -p jparser --lib`
Expected: FAIL to compile — `error[E0412]: cannot find type 'Sense' in this scope`
(and the same for `WordFlags`, which `lib.rs` does not import yet), plus
`error[E0425]: cannot find function 'kuru_hack' in this scope` and the same for
`reconstruct_reading`, `dictionary_form`, and `assemble_entry`.

Run: `cd ta && cargo test -p jparser --test parse_irregular`
Expected: FAIL to compile — `error[E0432]: unresolved imports 'jparser::parse',
'jparser::ParseOptions'`.

- [ ] **Step 5: Declare the public surface in `lib.rs`**

`mod matcher;`, `mod rank;`, `mod segment;`, the `BoundaryHints` re-export, and
`ParseError` already exist from Tasks 1, 4 and 6. **Verify what is present matches
character for character; do not create a second copy of anything.** The one
removal is `#[allow(dead_code)]` on `mod matcher;` — `parse` now calls in, so the
attribute would hide real dead code from here on.

Replace the module-declaration block in `ta/crates/jparser/src/lib.rs` with the
following, keeping the existing GPL v2 header comment at the top of the file:

```rust
//! Japanese text parsing: dictionary matching, segmentation, entry assembly.
//!
//! `parse` is the whole public surface. It runs three stages:
//! `matcher::matches_at` at every character position, `segment::segment` over
//! the resulting match table, then entry assembly — resolving each surviving
//! match's `entry_id` through `Index::entry`, restoring its dictionary form,
//! rendering its conjugation label, and reconstructing its reading.
//!
//! Every offset in this API is a **character** offset. The single conversion
//! point is `text.chars().collect()` at the top of `parse`; nothing below it
//! ever sees a byte index.
//!
//! Reading reconstruction ports `JParseWindow.cpp:186-208` plus the `kuruHack`
//! block of `GetDictEntry` (`ta-old/exe/util/Dictionary.cpp:1323-1360`).
//! ta-old walked an entry's `JapString` chain for the `JAP_WORD_PRONOUNCE`
//! sibling; this port stores those readings on `EntryData` instead.

use std::collections::HashMap;

pub mod conjugation;
pub mod index;
pub mod jmdict;
pub mod kana;
mod matcher;
mod rank;
pub mod record;
pub mod romaji;
mod segment;
pub mod stem;

use crate::conjugation::{ConjugationTable, VerbTypeId};
use crate::index::load::Index;
use crate::index::EntryData;
use crate::matcher::{ConjLink, Match};
use crate::record::WordFlags;

/// One dictionary sense. A re-export rather than a parallel owned struct with
/// the same five fields.
pub use crate::index::SenseData as Sense;
pub use crate::segment::BoundaryHints;

/// Parse-time options.
///
/// Deliberately empty in Phase 1B: boundary votes arrive through `parse`'s
/// `hints` parameter, gloss filters and furigana modes are Phase 3 display
/// concerns above this crate, and the v5 mis-annotation fallback is a
/// build-time `StemOptions` flag. `#[non_exhaustive]` so adding a field later
/// is not a breaking change; construct with `ParseOptions::default()`.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ParseOptions {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    /// A contiguous cover of the input in ascending `start` order: every
    /// character belongs to exactly one segment, matched or not. Empty iff
    /// the input is empty.
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Character offset into the parsed text. Never a byte offset.
    pub start: usize,
    /// Length in characters.
    pub len: usize,
    /// `text[start..start + len]` verbatim.
    pub surface: String,
    /// Display reading, taken from `entries[0].reading`.
    ///
    /// `None` for an unmatched run — there is no morphological-analyzer
    /// fallback — and `None` whenever the primary entry has no reading:
    /// because the match already is a reading (`WordFlags::PRONOUNCE`),
    /// because the entry has no kanji form and so stores no readings, or
    /// because no stored reading could be stripped back to a stem.
    pub reading: Option<String>,
    pub matched: bool,
    /// Every dictionary entry aligning to this exact span, ranked by
    /// `sort_matches`: the primary candidate first, then alternatives. Empty
    /// when `!matched`.
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The dictionary form: the matched surface for a plain headword, or the
    /// stem with its verb type's remove-suffix restored for a stem match.
    pub headword: String,
    pub reading: Option<String>,
    /// `render_conjugation_label` output, e.g. `"Negative Formal Past"`.
    /// `None` for a non-verb match, and `None` when the label renders empty —
    /// which `GetConjString` legitimately does for an all-Stem chain.
    pub conjugation: Option<String>,
    /// Union of every sense's `pos`, in first-seen order, deduplicated.
    pub pos: Vec<String>,
    pub senses: Vec<Sense>,
    /// The match's flags after the DP's stale-`COUNTER` clearing.
    pub flags: WordFlags,
}
```

The `ParseError` enum Task 1 added stays exactly where it is:

```rust
/// One variant on purpose: reading the memory-mapped payload is the only way
/// any Phase 1B code can fail. A distinct enum keeps `IndexError` out of
/// `parse`'s public signature and leaves room for variants later.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("reading the index failed: {0}")]
    Index(#[from] crate::index::IndexError),
}
```

- [ ] **Step 6: Add the entry-assembly helpers to `lib.rs`**

Insert below the type definitions, above the test module:

```rust
/// `Index::entry` memoized for the duration of one parse. The same
/// `entry_id` recurs across alternatives within a span and across spans, and
/// every miss is a fresh bincode decode off the mmap.
fn entry_data<'a>(
    index: &Index,
    cache: &'a mut HashMap<u32, Option<EntryData>>,
    id: u32,
) -> Result<Option<&'a EntryData>, ParseError> {
    if !cache.contains_key(&id) {
        let fetched = index.entry(id)?;
        cache.insert(id, fetched);
    }
    Ok(cache.get(&id).and_then(|slot| slot.as_ref()))
}

/// One `Match` plus its `EntryData` into a public `Entry`.
fn assemble_entry(m: &Match, data: &EntryData, table: &ConjugationTable, text: &[char]) -> Entry {
    let label = matcher::render_conjugation_label(&m.chain, table);
    let mut pos: Vec<String> = Vec::new();
    for sense in &data.senses {
        for code in &sense.pos {
            if !pos.contains(code) {
                pos.push(code.clone());
            }
        }
    }
    Entry {
        headword: dictionary_form(m, table),
        reading: reconstruct_reading(m, data, table, text),
        conjugation: (!label.is_empty()).then_some(label),
        pos,
        senses: data.senses.clone(),
        flags: m.flags,
    }
}

/// The dictionary form of a match: a stem with its verb type's remove-suffix
/// restored, or the surface itself for a plain headword.
///
/// This inverts `stem::generate_stems`, which stripped the first
/// remove-tense/form-0 suffix that actually matched the headword's tail.
/// Three types declare more than one such conjugation — `copula` (だ, である),
/// `adj-i` (い, し), `v5uru` (うる, える) — so the inverse is not unique and the
/// original headword is not stored anywhere. The contract pins "first"; first
/// is what this uses, which mis-renders the rare `adj-i` word whose dictionary
/// form ends in し as one ending in い.
fn dictionary_form(m: &Match, table: &ConjugationTable) -> String {
    let Some(link) = m.chain.first() else {
        return m.surface.clone();
    };
    let Some(ty) = table.types().get(link.verb_type) else {
        return m.surface.clone();
    };
    match ty
        .conjugations
        .iter()
        .find(|c| c.tense == ty.remove_tense && c.form.0 == 0)
    {
        Some(c) => format!("{}{}", m.surface, c.suffix),
        // Structurally impossible: without one, the stem could not exist.
        None => m.surface.clone(),
    }
}

/// Rebuild the kana reading of a match, ta-old's `JParseWindow.cpp:186-208`.
///
/// `None` is a normal outcome, not an error: the match may already be kana,
/// the entry may have no kanji at all, or no twin conjugation may exist — as
/// for the Imperfective 来, whose kana `vk` twin has no Imperfective row.
fn reconstruct_reading(
    m: &Match,
    data: &EntryData,
    table: &ConjugationTable,
    text: &[char],
) -> Option<String> {
    // The match already is a reading; ta-old renders no furigana over kana.
    if m.flags.contains(WordFlags::PRONOUNCE) {
        return None;
    }
    // A kana-only entry stores no readings, because its surface is one.
    if data.readings.is_empty() {
        return None;
    }
    let Some(link) = m.chain.first() else {
        return data.readings.first().cloned();
    };
    let ty = table.types().get(link.verb_type)?;
    let tail: String = text
        .get(m.start + m.src_len..m.start + m.len)?
        .iter()
        .collect();
    // ta-old computed kuruHack once, in GetDictEntry, before the renderer
    // looped over the entry's spellings.
    let hack = kuru_hack(link, table);

    for reading in &data.readings {
        // Same-type path: the kana spelling conjugates through the same table
        // rows as the matched spelling, so its own stem plus the matched
        // suffix characters is the whole answer.
        if let Some(stem) = strip_remove_suffix(table, link.verb_type, reading) {
            return Some(stem + &tail);
        }
        // kuruHack path: the kana spelling is registered under a different,
        // identically named verb type — 来る's kanji rows are a separate `vk`
        // block from くる's kana rows. `hack` supplies the reading of the one
        // leading kanji and the rest of the matched text is copied verbatim,
        // which is what the `+ 1` skips over.
        let Some(hack) = hack.as_deref() else { continue };
        for twin in table.types_named(&ty.name) {
            if twin == link.verb_type {
                continue;
            }
            let Some(stem) = strip_remove_suffix(table, twin, reading) else {
                continue;
            };
            // `src_len == len` with a non-empty chain — every consumed suffix
            // was empty — makes this range inverted. §6.6 says continue, not
            // abort: a `?` here would skip every later reading that would have
            // succeeded. Unreachable in the shipped asset today, because the
            // same-type path fires first for every such match.
            let Some(rest_slice) = text.get(m.start + m.src_len + 1..m.start + m.len) else {
                continue;
            };
            let rest: String = rest_slice.iter().collect();
            return Some(stem + hack + &rest);
        }
    }
    None
}

/// The stem of `s` under `id`'s remove-tense — the same expression
/// `stem::generate_stems` used at build time, so a kana stem exists here iff
/// one was generated then.
fn strip_remove_suffix(table: &ConjugationTable, id: VerbTypeId, s: &str) -> Option<String> {
    let ty = table.types().get(id)?;
    ty.conjugations
        .iter()
        .filter(|c| c.tense == ty.remove_tense && c.form.0 == 0)
        .find_map(|c| kana::strip_suffix_unified(s, &c.suffix))
}

/// ta-old's `kuruHack` destination is `wchar_t[4]` (`Dictionary.h:103`), so at
/// most three characters were ever written; a longer substitution was silently
/// discarded and the scan continued to the next same-named type. Kept, rather
/// than dropped as a C buffer artefact, so Phase 6's differential run compares
/// like for like.
const KURU_HACK_MAX_CHARS: usize = 3;

/// The kana spelling the reading of the leading kanji of `link`'s conjugation
/// suffix, ta-old's `kuruHack` (`Dictionary.cpp:1323-1360`).
///
/// Fires only for a suffix starting with a CJK ideograph, which in the shipped
/// asset means only the kanji `vk` block (来る). Pairing is by verb type
/// *name*: `types_named` deliberately keeps every duplicate-named type
/// reachable so the kana twin can be found. Only `chain[0]` is ever inspected,
/// which is sufficient for arbitrarily deep stacks because `chain[0]` is
/// always the type applied directly to the dictionary stem.
fn kuru_hack(link: &ConjLink, table: &ConjugationTable) -> Option<String> {
    let ty = table.types().get(link.verb_type)?;
    let c = ty.conjugations.get(link.conj)?;
    if !kana::is_cjk_ideograph(c.suffix.chars().next()?) {
        return None;
    }
    let len = c.suffix.chars().count();

    for twin in table.types_named(&ty.name) {
        if twin == link.verb_type {
            continue;
        }
        let Some(twin_ty) = table.types().get(twin) else {
            continue;
        };
        // ta-old keeps the SHORTEST twin suffix at least as long as this one
        // (`len2 < len || len2 >= best` rejects), not the longest.
        let mut best: Option<(usize, &str)> = None;
        for c2 in &twin_ty.conjugations {
            if c2.tense != c.tense || c2.form != c.form || c2.next_verb_type != c.next_verb_type {
                continue;
            }
            let len2 = c2.suffix.chars().count();
            if len2 < len || best.is_some_and(|(shortest, _)| len2 >= shortest) {
                continue;
            }
            if tails_match(&c.suffix, &c2.suffix, len, len2) {
                best = Some((len2, c2.suffix.as_str()));
            }
        }
        let Some((len2, suffix)) = best else { continue };
        let want = len2 - len + 1;
        if want <= KURU_HACK_MAX_CHARS {
            return Some(suffix.chars().take(want).collect());
        }
        // No break: ta-old skips an over-long twin and keeps scanning
        // (`Dictionary.cpp:1355`).
    }
    None
}

/// ta-old's `wcsnijcmp(conj->suffix + 1, conj2->suffix + len2 - len + 1,
/// len - 1)`: the two suffixes must agree under `kana::unify` once each has
/// lost its leading substitution characters.
fn tails_match(kanji_suffix: &str, kana_suffix: &str, len: usize, len2: usize) -> bool {
    let kanji_tail: String = kanji_suffix.chars().skip(1).collect();
    let kana_tail: Vec<char> = kana_suffix.chars().skip(len2 - len + 1).collect();
    matcher::unified_eq(&kana_tail, &kanji_tail)
}
```

- [ ] **Step 7: Add `parse` to `lib.rs`**

Insert directly above `entry_data`:

```rust
/// Parse `text` against an already-open index.
///
/// `index` and `table` are parameters rather than globals: the port design
/// forbids globals and the Phase 1A handoff pins "pass `&Index`". No Phase 1B
/// type stores an index directory path, which is what keeps Phase 2's
/// generation-directory layout cheap.
pub fn parse(
    index: &Index,
    table: &ConjugationTable,
    text: &str,
    opts: &ParseOptions,
    hints: Option<&dyn BoundaryHints>,
) -> Result<ParseResult, ParseError> {
    // ParseOptions carries no fields yet; the parameter exists so adding one
    // later is not a breaking change.
    let _ = opts;

    let chars: Vec<char> = text.chars().collect();
    let mut buckets: Vec<Vec<Match>> = Vec::with_capacity(chars.len());
    for i in 0..chars.len() {
        buckets.push(matcher::matches_at(index, table, &chars, i)?);
    }
    let segmentation = segment::segment(&chars, &buckets, hints);

    let mut cache: HashMap<u32, Option<EntryData>> = HashMap::new();
    let mut segments = Vec::with_capacity(segmentation.spans.len());
    for span in &segmentation.spans {
        let surface: String = chars
            .get(span.start..span.start + span.len)
            .unwrap_or_default()
            .iter()
            .collect();
        // `Span::matches` is empty whenever `!matched`, so this loop is the
        // matched/unmatched branch as well.
        let mut entries: Vec<Entry> = Vec::new();
        for m in &span.matches {
            // An entry_id with no EntryData cannot happen for an index built
            // by `build_from_reader`. Drop the match rather than invent an
            // Entry, and leave `matched` alone: the span still covers its
            // characters, and flipping it would paper over a corrupt index.
            let Some(data) = entry_data(index, &mut cache, m.entry_id)? else {
                continue;
            };
            entries.push(assemble_entry(m, data, table, &chars));
        }
        segments.push(Segment {
            start: span.start,
            len: span.len,
            surface,
            reading: entries.first().and_then(|e| e.reading.clone()),
            matched: span.matched,
            entries,
        });
    }
    Ok(ParseResult { segments })
}
```

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cd ta && cargo test -p jparser --lib`
Expected: PASS. The 15 new tests report as `tests::kuru_hack_…`,
`tests::reconstructs_…`, `tests::returns_…`, `tests::dictionary_form_…`,
`tests::assemble_entry_…`. (Do not filter with `tests::` — every module's test
block matches that substring.)

Run: `cd ta && cargo test -p jparser --test parse_irregular`
Expected: PASS, 4 tests.

Run: `cd ta && cargo test -p jparser`
Expected: PASS, the whole crate — the Phase 1A suites are untouched by this task
apart from `EntryData`, and must stay green.

- [ ] **Step 9: Format, lint, and check the file-size trigger**

Run: `cd ta && rustfmt --edition 2021 crates/jparser/src/lib.rs crates/jparser/tests/parse_irregular.rs crates/jparser/src/index/mod.rs crates/jparser/src/index/build.rs`
(Individual files only. Never `cargo fmt -p jparser`.)

Run: `cd ta && cargo build -p jparser 2>&1 | grep -c "never"`
Expected: `0`. Every dead-code warning Tasks 4–6 left behind clears here, because
`parse` reaches `segment`, which reaches `rank`, which reaches `matcher`. If any
survive, the removal of `#[allow(dead_code)]` from `mod matcher;` in Step 5 has
exposed something genuinely unreachable — investigate, do not re-add the
attribute.

Run: `cd ta && cargo clippy -p jparser --all-targets -- -D warnings`
Expected: clean. This is the first task since Task 3 to lint the non-test lib
target with `-D warnings`, so it is the first place a stale `#[allow]` or an
unread field would surface.

Run: `cd ta && wc -l crates/jparser/src/lib.rs`
Expected: roughly **650** lines, of which ~370 are non-test.

Contract §7 says entry assembly splits into `src/entry.rs` "if `lib.rs` passes
~400 lines", and calls that split a deviation from §3's module list that must be
flagged rather than taken silently. **This task does not take it**, for two
reasons: the trigger is advisory while the 800-line cap is binding, and the
contract's own test-home table puts the reading-reconstruction and kuruHack tests
in `src/lib.rs`'s `mod tests`, which only makes sense if the code under test lives
there too. **Remaining headroom is ~150 lines, not ~260** — if a later task pushes
`lib.rs` past 800, move `entry_data`, `assemble_entry`, `dictionary_form`,
`reconstruct_reading`, `strip_remove_suffix`, `kuru_hack`, `tails_match`,
`KURU_HACK_MAX_CHARS` and their tests to `src/entry.rs` as `pub(crate)` items
behind `mod entry;`, and flag it in that task.

- [ ] **Step 10: Commit**

```bash
cd ta && git add crates/jparser/src/lib.rs crates/jparser/src/index/mod.rs \
  crates/jparser/src/index/build.rs crates/jparser/tests/parse_irregular.rs
git commit -m "feat: add the parse() public surface and entry assembly

Completes Phase 1B's user-visible half: ParseResult, Segment, Entry,
ParseError, ParseOptions, and Sense re-exported from index::SenseData.
parse() runs matches_at per position, hands the match table to the DP,
then assembles entries from the surviving spans.

EntryData gains a readings field and INDEX_FORMAT_VERSION goes 2 to 3.
The Phase 1A payload stored only {id, senses}, so there was no way to
recover an entry's kana reading from a match; ta-old walked the entry's
JapString chain for its JAP_WORD_PRONOUNCE sibling and the port has no
equivalent. Readings are stored only for entries that also have kanji
forms, which is exactly ta-old's PRONOUNCE set. This is the only
Phase 1A data-shape change Phase 1B makes.

Entry assembly resolves each match's entry_id through Index::entry
behind a per-parse memo, restores the dictionary form by re-appending
the verb type's remove-suffix, labels the conjugation via
render_conjugation_label, and reconstructs the reading.

Reading reconstruction ports JParseWindow.cpp:186-208 and the kuruHack
block of GetDictEntry. An irregular verb like 来る keeps its kanji and
kana spellings in two separate, identically named vk blocks, so the
same-type stem trick fails and the reading has to come from the twin:
kuru_hack finds the kana conjugation occupying the same tense/form/next
slot and takes the characters that spell the leading kanji. ta-old's
three-character cap is kept deliberately so Phase 6's differential run
compares like for like.

Segment.reading is None for an unmatched run. There is no
morphological-analyzer fallback and none is planned: unmatched means
unmatched. Every offset in the API is a character offset; the single
&str-to-Vec<char> conversion is at the top of parse.

tests/parse_irregular.rs builds a one-entry 来る index and parses 来る,
来た, 来た。 and the empty string. 来た is the load-bearing case: the vk
stem is the empty string, so it only matches through Phase 1A's empty
FST key, and its reading きた only exists if the kuruHack twin search
works — and if the new EntryData.readings field survives the payload."
```

### Task 7 notes for the reviewer

- **What the 来た case actually proves.** The stored `vk` kanji rows give `来る` a
  stem of `""`, so the match is reachable only through the empty FST key.
  `chain[0]` is the Stem conjugation `来た → v-ta-stem`, whose kana twin `きた`
  occupies the same `(tense, form, next_verb_type)` slot; `kuru_hack` returns `き`,
  the twin type strips its own `くる` to `""`, and the verbatim tail `た` completes
  `きた`. `render_conjugation_label` skips the Stem link at depth 0 and prints
  `v-ta-stem`'s Past, giving `"Past"`. Every one of those values was checked
  against `assets/conjugations.json` as the loader stores it, after `Next Type`
  trimming.
- **It also guards two matcher rules incidentally**, so a failure here is not
  automatically a Task 7 bug. `v-ta-stem`'s conjugation 0 is
  `tense=Remove, form=0, suffix="", terminal`: a matcher that fails to skip
  Remove emits a *second* len-2 match whose chain is `[Stem, {v-ta-stem, Remove}]`,
  and since both chains are non-empty neither pass-B rule collapses them, so
  `assert_eq!(s.entries.len(), 1)` fails. And if the Stem-skip were wrongly applied
  at depth 0, `chain[0]` would become `{v-ta-stem, Past}`, so `dictionary_form`
  returns `""` and `kuru_hack` bails on the empty suffix — breaking both
  `e.headword == "来る"` and `e.reading == Some("きた")`.
- **Why the 来る case is weaker but still worth keeping.** Both candidates at
  `(0, 2)` — the plain headword and the empty stem conjugated Non-past —
  reconstruct `くる`, one through the non-verb path and one through kuruHack, so
  the assertion holds whichever survives `sort_matches`. It is asserted at
  `entries.len() == 1` anyway, which is a live cross-check on the §6.5 pass-B
  verb-plain collapse.
- **Dormant by design:** `WordFlags::IS_NAME` is never set in v1, so
  `reconstruct_reading` and `assemble_entry` need no names-dictionary branch; the
  scorer's `NAME_DICT_*` handling lives entirely in `segment.rs`.
- **Uncovered branch, knowingly:** `kuru_hack`'s `want > KURU_HACK_MAX_CHARS`
  continue. No real `vk` slot needs four substitution characters, so reaching it
  requires a synthetic `from_json` table. Left uncovered here; Task 9 Step 11 names
  it as the first thing to add if the coverage gate falls short.

---

## Task 8: `parse` subcommand on `jparser-cli`

Phase 1's user-facing deliverable. Everything Tasks 1–7 built is invisible until a
human can type a sentence and read back the segmentation, the readings, the
conjugation labels, the flags-derived ranking, and the glosses. This is also the
tool Task 9's curation step uses to prove the committed JMdict subset reproduces
the full dictionary's parse.

**Files:**
- Modify: `ta/crates/jparser/src/bin/jparser-cli.rs`
- Create: `ta/crates/jparser/tests/cli_parse.rs`

**Interfaces:**
- Consumes:
  - `pub fn jparser::parse(index: &Index, table: &ConjugationTable, text: &str, opts: &ParseOptions, hints: Option<&dyn BoundaryHints>) -> Result<ParseResult, ParseError>` (Task 7)
  - `pub struct jparser::ParseOptions` (`Default`, `#[non_exhaustive]`) (Task 7)
  - `pub struct jparser::ParseResult { pub segments: Vec<Segment> }` (Task 7)
  - `pub struct jparser::Segment { pub start, pub len, pub surface, pub reading, pub matched, pub entries }` (Task 7)
  - `pub struct jparser::Entry { pub headword, pub reading, pub conjugation, pub pos, pub senses, pub flags }` (Task 7)
  - `pub use jparser::Sense` (= `index::SenseData`, with `pub glosses: Vec<String>`) (Task 7)
  - `pub enum jparser::ParseError` (`thiserror`, so `?` into `Box<dyn Error>` works) (Task 7)
  - `jparser::index::load::Index::open`, `jparser::conjugation::ConjugationTable::load_embedded` (Phase 1A)
- Produces:
  - a `parse` subcommand on `jparser-cli`: `Command::Parse { index: PathBuf, text: String }`
  - the frozen text format below, which Task 9 Step 10 diffs full-index against
    subset-index output with

**Resolved gap.** Contract §4.5 fixes the `Parse` variant's shape but leaves the
output format free, requiring only `start`, `len`, surface, `matched`, reading,
and per entry the headword, conjugation label, and first sense's glosses. This
task freezes an exact format and asserts it byte-for-byte; it prints `start=` and
`len=` literally rather than a `0..3` range so the contract's wording is met
verbatim.

**Output format (frozen here, used by Task 9):**

```text
start={start} len={len} {surface} matched reading={reading|-}
    {headword} ({conjugation|-}) [{reading|-}] {gloss; gloss; …}
start={start} len={len} {surface} unmatched
```

One segment line per `Segment`, then one indented entry line per `Entry` in
`Segment::entries` order (which is `sort_matches` order — primary first). `-`
stands in for every `None`. Glosses are the **first** sense's, joined with
`"; "`. Unmatched segments print no entry lines.

- [ ] **Step 1: Write the failing integration test**

Create `ta/crates/jparser/tests/cli_parse.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `jparser-cli parse` end to end, against the hand-written mini fixture.
//!
//! Exact-output assertions, not substring checks: the format is a frozen
//! interface (Task 9 Step 10 diffs two runs of it against each other), so a
//! change to it must break a test rather than silently change what a
//! downstream comparison compares.

use std::path::{Path, PathBuf};
use std::process::Command;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::stem::StemOptions;

/// Cargo sets this for integration tests of a package that declares the bin.
const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

/// Same temp-dir convention as `tests/index_roundtrip.rs`; the crate has no
/// `tempfile` dependency and is not gaining one.
fn index_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &dir,
    )
    .expect("the mini fixture must build");
    dir
}

/// `&Path`, not `&PathBuf`: clippy's `ptr_arg` is warn-by-default and Step 5
/// lints this file under `-D warnings`. Call sites pass `&dir` unchanged.
fn parse(dir: &Path, text: &str) -> String {
    let out = Command::new(BIN)
        .arg("parse")
        .arg(dir)
        .arg(text)
        .output()
        .expect("jparser-cli must be runnable");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty(), "unexpected stderr");
    String::from_utf8(out.stdout).expect("output must be UTF-8")
}

#[test]
fn prints_a_conjugated_verb_with_its_reconstructed_reading() {
    // 言う is tagged v5r in the fixture but ends in う, so its only stem comes
    // from the v5 mis-annotation fallback under v5u: surface "言". Matching
    // "言った" is then v5u Stem "った" -> v-ta-stem Past "" (empty suffix), a
    // two-link chain whose label renders as "Past" (the Stem link is skipped
    // at every depth, GetConjString / contract §6.2).
    //
    // headword  = "言" + v5u's remove-suffix "う"          = 言う
    // reading   = strip("いう", "う") + text[1..3]         = い + った = いった
    // glosses   = first sense of entry 1000010
    let dir = index_dir("cli-parse-verb");
    assert_eq!(
        parse(&dir, "言った"),
        "start=0 len=3 言った matched reading=いった\n\
         \x20   言う (Past) [いった] to say; to utter\n"
    );
}

#[test]
fn prints_an_unconjugated_particle_and_coalesces_the_skipped_tail() {
    // は is a kana-only entry, so `EntryData::readings` is empty (contract §2)
    // and `reconstruct_reading` returns None at STEP 2 — not step 1: a
    // kana-only entry's record gets PRIMARY, never PRONOUNCE
    // (record.rs:140-148). So reading is "-", not "は". "zz" is two skipped
    // chars, which the backtrack coalesces into one unmatched span (§6.3).
    let dir = index_dir("cli-parse-particle");
    assert_eq!(
        parse(&dir, "はzz"),
        "start=0 len=1 は matched reading=-\n\
         \x20   は (-) [-] topic marker\n\
         start=1 len=2 zz unmatched\n"
    );
}
```

**Why the first test's expected string is exactly that**, traced against the real
asset so a failure is diagnosable: `v5r`'s remove suffix (it declares no `Remove`
tense, so `remove_tense` defaults to Non-past) is `る`, which does not strip from
`言う`; the v5 fallback admits `v5s v5u v5b v5g v5k v5m v5n v5t`, of which only
`v5u`'s Non-past `う` strips, giving exactly one stem `言` with
`verb_type = Some(v5u)`. `v5u` conjugation 13 is `Stem / form 0 / った /
next=v-ta-stem`, and it stays **untrimmed** because `v-ta-stem`'s Remove suffix is
`""` — no remove-tense/form-0 entry anywhere in the asset carries a `Next Type`,
so load-time trimming never touches one. At depth 0 the Stem link takes
`chain[0]`; recursing into `v-ta-stem` at `slen = 3`, conjugation 0
(`Remove`/`""`) is skipped by the Remove rule and conjugation 1 (`Past`/`""`) is
terminal, giving `len = 3` and `chain = [Stem, Past]`. There is no empty-suffix
terminal in `v5u`, so exactly one match exists at `(0, 3)` and exactly two lines
are printed.

**This one assertion also guards two of the matcher's subtle rules.** Drop the
Remove-tense skip and `v-ta-stem` conjugation 0 emits a second terminal, adding a
`言う (Remove) …` line. Wrongly apply the Stem-skip at depth 0 and `chain[0]`
becomes `v-ta-stem`, degrading the headword to `言` and the reading to `いうった`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ta && cargo test -p jparser --test cli_parse`

Expected: FAIL. Both tests panic inside `parse()` on the `out.status.success()`
assertion, with clap's own message on stderr:

```text
error: unrecognized subcommand 'parse'
```

and exit code `2`. (The test binary itself compiles — it only uses Phase 1A
library items plus the bin path.)

- [ ] **Step 3: Add the `Parse` variant to the CLI**

In `ta/crates/jparser/src/bin/jparser-cli.rs`, add the import beside the existing
`use jparser::...` lines:

```rust
use jparser::ParseOptions;
```

Add the stand-in string rendered for every `None`, beside `FLAG_LABELS`:

```rust
/// Rendered wherever a `reading` or `conjugation` is `None`. Named because it
/// is part of the frozen output format, not incidental formatting.
const NONE_LABEL: &str = "-";
```

Add the variant to `enum Command`, after `Lookup`:

```rust
    /// Segment TEXT against an index and print the result.
    Parse {
        /// Index directory.
        index: PathBuf,
        /// Text to parse.
        text: String,
    },
```

And the arm to `main`'s `match`, after the `Command::Lookup` arm:

```rust
        Command::Parse { index, text } => {
            let table = ConjugationTable::load_embedded()?;
            let index = Index::open(&index)?;
            // `None` hints: BoundaryHints has no implementation until Phase 5,
            // and `None` must behave exactly like one that always returns false.
            let result = jparser::parse(&index, &table, &text, &ParseOptions::default(), None)?;
            for seg in &result.segments {
                if !seg.matched {
                    println!(
                        "start={} len={} {} unmatched",
                        seg.start, seg.len, seg.surface
                    );
                    continue;
                }
                println!(
                    "start={} len={} {} matched reading={}",
                    seg.start,
                    seg.len,
                    seg.surface,
                    seg.reading.as_deref().unwrap_or(NONE_LABEL)
                );
                for entry in &seg.entries {
                    let glosses = entry
                        .senses
                        .first()
                        .map(|s| s.glosses.join("; "))
                        .unwrap_or_default();
                    println!(
                        "    {} ({}) [{}] {glosses}",
                        entry.headword,
                        entry.conjugation.as_deref().unwrap_or(NONE_LABEL),
                        entry.reading.as_deref().unwrap_or(NONE_LABEL),
                    );
                }
            }
        }
```

Finally retitle the harness, since it is no longer Phase 1A only:

```rust
#[command(name = "jparser-cli", about = "JParser Phase 1 harness")]
```

No test reads `about`, so this breaks nothing.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd ta && cargo test -p jparser --test cli_parse`

Expected: `test result: ok. 2 passed; 0 failed`.

If `prints_a_conjugated_verb_with_its_reconstructed_reading` fails on the
*reading* only, the fault is `reconstruct_reading`'s tail slice (Task 7), not this
task: `tail` is `text[start + src_len .. start + len]`, i.e. `った`, and `src_len`
is the **key** length (1), not the match length (3).

- [ ] **Step 5: Format and lint the two files**

```bash
cd ta && rustfmt --edition 2021 crates/jparser/src/bin/jparser-cli.rs \
  crates/jparser/tests/cli_parse.rs
cargo clippy -p jparser --bin jparser-cli --tests -- -D warnings
```

Expected: clippy clean. `--tests` lints the integration-test targets too, which is
why `parse` in the test file takes `&Path` — `&PathBuf` trips `clippy::ptr_arg`
and fails the step. Never run `cargo fmt -p jparser`.

- [ ] **Step 6: Verify by hand against the mini fixture**

```bash
cd ta
cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/jparser-cli-index
cargo run -q -p jparser --bin jparser-cli -- \
  parse /tmp/jparser-cli-index "高かった"
```

Expected exactly:

```text
start=0 len=4 高かった matched reading=たかかった
    高い (Past) [たかかった] high; tall
```

(`高い` is `adj-i`, which declares no `Remove` tense, so its remove-tense/form-0
suffix is the Non-past `い` and the stem is `高`; `adj-i` conjugation 7 is
`Past / form 0 / かった`, terminal, giving `len = 4` and a one-link chain. The
reading is `strip("たかい", "い") + "かった"`.)

Not a defect, but worth knowing while debugging: `adj-i` conjugation 32 is an
`Adj Stem` with an empty terminal suffix, so a **len-1** match also exists at
position 0 and `best[1] = 11`. It cannot win `best[4] = 10`, and the backtrack's
`m.len == len` filter keeps it out of the printed span group — but it will appear
in the bucket if you dump one.

```bash
cargo run -q -p jparser --bin jparser-cli -- parse /tmp/jparser-cli-index "犬"
```

Expected exactly (nothing in the fixture matches `犬`, and one skipped char is
still a segment):

```text
start=0 len=1 犬 unmatched
```

- [ ] **Step 7: Commit**

```bash
cd ta && git add crates/jparser/src/bin/jparser-cli.rs crates/jparser/tests/cli_parse.rs
git commit -m "feat: add a parse subcommand to jparser-cli

Phase 1's deliverable is 'verifiable via a CLI harness' (port design
§11), and until now nothing printed a segmentation. parse takes an
index directory and a string and prints, per segment, the char offset
and length, the surface, whether it matched, the display reading, and
every ranked alternative with its dictionary form, conjugation label,
and first-sense glosses.

The output format is frozen and asserted byte-for-byte by
tests/cli_parse.rs rather than checked for substrings, because the
curated-JMdict-subset step diffs two runs of this format against each
other to prove the committed fixture reproduces the full dictionary's
parse. A silent format change there would silently weaken that proof.

Hints are None: BoundaryHints has no implementation until Phase 5, and
None must behave exactly as an implementation returning false
everywhere."
```

---

## Task 9: Curated JMdict fixture, snapshot suite, and the coverage gate

The snapshots are, per port design §10, "the highest-value test here; it answers
'did my refactor change the parse?'". They need real vocabulary, which means a
real `JMdict_e.xml` — which is **not in this repo** and must be fetched once,
deliberately, with the operator's confirmation. What gets committed is a curated
subset containing only the entries these sentences can reach, so the suite runs
offline from a fresh clone and is immune to JMdict's daily rebuilds.

**Files:**
- Create: `ta/crates/jparser/tests/fixtures/parse_sentences.txt`
- Create: `ta/crates/jparser/tests/parse_snapshots.rs`
- Create: `ta/tools/extract_jmdict_subset.py`
- Create: `ta/crates/jparser/tests/fixtures/jmdict_subset.xml` (generated, committed)
- Create: `ta/crates/jparser/tests/fixtures/README.md` (attribution)
- Create: `ta/crates/jparser/tests/snapshots/parse_snapshots__sentences.snap` (generated by insta, committed)

**Interfaces:**
- Consumes: everything Task 8 consumes, plus
  `jparser::index::build::build_from_reader` and
  `jparser::index::load::Index::open` (Phase 1A), and the `parse` subcommand
  (Task 8) for the sufficiency proof in Step 10.
- Produces: no library API. It produces the committed fixtures, the snapshot
  baseline, and the 80%-coverage gate command that CI and every later phase run.

**Resolved gaps.**

1. **No fixture or tool paths are frozen.** Contract §7 names the snapshot test
   file and says its fixture is a "committed curated JMdict subset", but names
   neither the fixture nor the sentence corpus nor the curation tool. Chosen and
   used consistently by Tasks 8–9:
   `tests/fixtures/jmdict_subset.xml`, `tests/fixtures/parse_sentences.txt`,
   `tests/fixtures/README.md`, `ta/tools/extract_jmdict_subset.py`.
2. **Contract §2 freezes every Phase 1A file**, and `ta/xtask/src/main.rs` is a
   Phase 1A file — so the JMdict curation tool **cannot** be a second `xtask`
   subcommand even though xtask exists precisely for one-off asset conversion.
   This task adds a standalone `python3` script (stdlib only, no cargo dependency,
   no Phase 1A file touched). Preferring an xtask subcommand would be a contract
   change, not a task change.
3. **`Segmentation::total_cost` is `pub(crate)` and `ParseResult` does not carry
   it**, so neither the CLI nor the snapshots can assert cost, and port design
   §10's "assert the cost, not just the winning segmentation" is satisfied **only**
   by `segment.rs`'s in-module tests (Tasks 4–5). These snapshots pin the winner
   and the alternatives list, not the cost. Catching cost regressions end to end
   would need a `total_cost` field on `ParseResult` — a contract change.
4. **The snapshot renderer is duplicated in the CLI.** §7 mandates a *test-only*
   formatter, and a `[[bin]]` cannot export code to an integration test. Two ~15
   line formatters exist on purpose; neither is promoted into the library, because
   a display concern does not belong in this crate's public API. They deliberately
   differ in what they print per entry (all glosses vs. one), so they are not
   drifting copies of one thing.
5. **`NAME_DICT_BAD_PER_CHAR` / `NAME_DICT_OK` are unreachable in v1** — nothing
   sets `WordFlags::IS_NAME` — so no parse can cover them. Task 5's three
   hand-built `IS_NAME` tests are their only coverage and are assigned there, not
   here; this task does not reach into another task's file.

**Constraints reaffirmed:** the Python tool gets a GPL v2 header like every other
new source file; the generated XML fixture does **not** — it is third-party data
under its own licence and carries the EDRDG notice instead (Step 7). No new cargo
dependency: `insta` is already a dev-dependency and the tool is stdlib `python3`.

- [ ] **Step 1: Write the sentence corpus**

Create `ta/crates/jparser/tests/fixtures/parse_sentences.txt`. Thirty sentences,
one per line; `#` comments and blank lines are skipped by both the extractor and
the test. They are short on purpose — every extra character widens the curated
subset, and the fixture has a size budget.

```text
# Snapshot corpus for tests/parse_snapshots.rs and tools/extract_jmdict_subset.py.
# One sentence per line. Blank lines and lines starting with '#' are ignored.
#
# Coverage intent, per sentence group:
#   1-5    plain nouns, particles, i- and na-adjectives
#   6-10   する and 来る: the two irregulars whose generated stem is the EMPTY
#          string, so prefixes_of returns a key_chars == 0 hit at every
#          position. That path broke and was fixed late in Phase 1A and is the
#          standing regression the addendum §6 asks for.
#   11-16  stacked conjugations: passive, causative, potential, negative, past
#   17-22  te-form chains, -tai, -teiru, -tekuru
#   23-27  numbers and counters, katakana runs
#   28-30  longer mixed sentences
猫が好きです。
本を読みました。
これは面白い映画だ。
新しい車を買った。
冷たい水を飲まない。
彼は学校に来る。
明日友達が来ます。
彼はまだ来ていない。
勉強をする時間がない。
昨日は宿題をしました。
彼に何も言われなかった。
手紙を書かせられた。
日本語を話せますか。
早く帰りたかった。
雨が降りそうだ。
高い山に登った。
水が飲みたい。
毎日走っている。
電車が遅れています。
ドアを開けてください。
子供が公園で遊んでいる。
もう一度説明してください。
三人の学生が待っている。
コーヒーを二杯ください。
会議は十時に始まる。
東京から大阪まで行く。
音楽を聞きながら歩く。
先生になりたいです。
パソコンが壊れてしまった。
出て来るのを待っていた。
```

- [ ] **Step 2: Write the failing snapshot test**

Create `ta/crates/jparser/tests/parse_snapshots.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! insta snapshots over real sentences, against a curated JMdict subset.
//!
//! The subset is committed (see `fixtures/README.md`) so this suite runs
//! offline from a fresh clone and does not move when JMdict is rebuilt. Two
//! targeted assertions sit beside the snapshot for する and 来る, whose
//! generated stem is the empty string: a snapshot diff would report that
//! breakage as "something changed", while these report what changed.

use std::path::PathBuf;
use std::sync::OnceLock;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::{ParseOptions, ParseResult};

const FIXTURE: &str = include_str!("fixtures/jmdict_subset.xml");
const SENTENCES: &str = include_str!("fixtures/parse_sentences.txt");

/// Alternatives printed per span. The full list is often a dozen entries for a
/// single-kana particle; five is enough to pin `sort_matches`' ranking while
/// keeping the snapshot reviewable by a human, which is the entire point of it.
const MAX_ALTERNATIVES: usize = 5;

/// Printed wherever a reading or conjugation is `None`.
const NONE_LABEL: &str = "-";

static INDEX_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Build the curated index exactly once for the whole test binary. Tests run
/// in parallel threads; `Index::open`'s contract forbids writing to a
/// directory while an index over it is alive, and `get_or_init` is what keeps
/// every `open` strictly after the single build.
fn index_dir() -> &'static PathBuf {
    INDEX_DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("jparser-test-parse-snapshots");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = ConjugationTable::load_embedded().unwrap();
        let report = build_from_reader(
            std::io::Cursor::new(FIXTURE),
            &table,
            &StemOptions::default(),
            &dir,
        )
        .expect("the curated fixture must build");
        assert_eq!(
            report.skipped_entries, 0,
            "the curated fixture must not contain malformed entries"
        );
        dir
    })
}

fn sentences() -> impl Iterator<Item = &'static str> {
    SENTENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn parsed(text: &str) -> ParseResult {
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(index_dir()).unwrap();
    jparser::parse(&index, &table, text, &ParseOptions::default(), None).unwrap()
}

/// Test-only renderer. `jparser-cli` has its own copy of this shape; a `[[bin]]`
/// cannot export code to an integration test, and making a formatter public
/// would put a display concern in the library's API for the sake of fifteen
/// lines. The two deliberately differ: the CLI prints every alternative and all
/// of the first sense's glosses, this prints five alternatives and one gloss.
fn render(text: &str, result: &ParseResult) -> String {
    let mut out = String::new();
    out.push_str("=== ");
    out.push_str(text);
    out.push('\n');
    for seg in &result.segments {
        if !seg.matched {
            out.push_str(&format!(
                "start={} len={} {} unmatched\n",
                seg.start, seg.len, seg.surface
            ));
            continue;
        }
        out.push_str(&format!(
            "start={} len={} {} matched reading={}\n",
            seg.start,
            seg.len,
            seg.surface,
            seg.reading.as_deref().unwrap_or(NONE_LABEL)
        ));
        for entry in seg.entries.iter().take(MAX_ALTERNATIVES) {
            let gloss = entry
                .senses
                .first()
                .and_then(|s| s.glosses.first())
                .map(String::as_str)
                .unwrap_or("");
            out.push_str(&format!(
                "    {} ({}) [{}] {gloss}\n",
                entry.headword,
                entry.conjugation.as_deref().unwrap_or(NONE_LABEL),
                entry.reading.as_deref().unwrap_or(NONE_LABEL),
            ));
        }
        if seg.entries.len() > MAX_ALTERNATIVES {
            out.push_str(&format!(
                "    ... {} more\n",
                seg.entries.len() - MAX_ALTERNATIVES
            ));
        }
    }
    out
}

#[test]
fn snapshots_every_sentence() {
    let mut out = String::new();
    for sentence in sentences() {
        out.push_str(&render(sentence, &parsed(sentence)));
        out.push('\n');
    }
    insta::assert_snapshot!("sentences", out);
}

#[test]
fn corpus_has_thirty_sentences_covering_both_irregulars() {
    // Guards the corpus itself: addendum §6 requires ~30 sentences and at
    // least one する or 来る. Deleting a sentence to make a snapshot green
    // should fail here first.
    let all: Vec<&str> = sentences().collect();
    assert_eq!(all.len(), 30, "got {} sentences", all.len());
    assert!(all.iter().any(|s| s.contains("する")));
    assert!(all.iter().any(|s| s.contains("来る")));
}

#[test]
fn matches_suru_through_the_empty_stem_key() {
    // する is vs-i, whose remove-tense/form-0 suffix is the whole word, so its
    // generated stem is "" and the FST returns a key_chars == 0 hit: src_len 0,
    // len 4. Chain is vs-i Stem "し" -> v-i-stem Formal Past "ました".
    //
    // 勉強をする's bare する cannot be used here: the plain headword する is also
    // indexed (surface "する", verb_type None) at the same span and for the same
    // entry, and sort_matches' verb-plain/non-verb collapse (§6.5 pass B rule 3)
    // drops the Non-past verb match in favour of it — so that span's entry has
    // conjugation None and never touches the empty key. しました is immune
    // because chain[0].tense is Stem, not Non-past.
    let result = parsed("昨日は宿題をしました。");
    let seg = result
        .segments
        .iter()
        .find(|s| s.surface == "しました")
        .expect("しました must be one span");
    let entry = &seg.entries[0];
    assert_eq!(entry.headword, "する");
    assert_eq!(entry.conjugation.as_deref(), Some("Formal Past"));
}

#[test]
fn reconstructs_the_kuru_reading_through_the_kanji_kana_twin() {
    // 来ます is the kuruHack path: chain[0] is vk's kanji type, whose Stem
    // suffix is 来 -> v-i-stem, and the reading cannot be rebuilt from the
    // kana sibling くる by suffix stripping alone. kuru_hack finds the kana
    // twin's matching slot (き), and the reading becomes
    //   "" + "き" + text[start + src_len + 1 .. start + len]  ==  きます
    // The label is "Formal Non-past": depth 0 is Stem (always skipped), and
    // depth 1's Non-past survives only because depth 0's tense was Stem.
    let result = parsed("明日友達が来ます。");
    let seg = result
        .segments
        .iter()
        .find(|s| s.surface == "来ます")
        .expect("来ます must be one span");
    assert_eq!(seg.reading.as_deref(), Some("きます"));
    let entry = &seg.entries[0];
    assert_eq!(entry.headword, "来る");
    assert_eq!(entry.conjugation.as_deref(), Some("Formal Non-past"));
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cd ta && cargo test -p jparser --test parse_snapshots`

Expected: FAIL to compile, on the missing fixture:

```text
error: couldn't read crates/jparser/tests/fixtures/jmdict_subset.xml: No such file or directory (os error 2)
```

That is the intended RED. `include_str!` is used instead of a runtime read
precisely so a missing fixture is a compile error rather than a panic inside one
test.

- [ ] **Step 4: STOP — confirm before fetching JMdict, then fetch it**

**This step downloads a ~20 MB archive from a third-party FTP mirror. Ask the
user to confirm before running it. If they decline, or the fetch fails, do not
improvise: go to Step 4a.**

- **Source:** the Electronic Dictionary Research and Development Group (EDRDG),
  `http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz` — the English-only build.
  (`JMdict.gz` carries every target language, is several times the size, and
  nothing in this port reads a non-English gloss.)
- **Licence:** JMdict is © EDRDG and is licensed **Creative Commons
  Attribution-ShareAlike 4.0 International (CC BY-SA 4.0)**,
  <https://www.edrdg.org/edrdg/licence.html>. **Attribution is mandatory** for the
  subset this task commits; Step 7 writes it, and the extractor stamps a notice
  into the fixture itself.
- **Flag for a human, not a decision for this task:** the crate is `GPL-2.0-only`
  and CC BY-SA 4.0 is one-way compatible with GPL **v3**, not v2. The subset is
  committed as a separately-licensed *data* asset under its own notice, not
  relicensed under the crate's GPL header — which is how projects shipping JMdict
  normally handle it. Raise it in review; do not resolve it here.
- **Reproducibility:** JMdict is rebuilt daily, so two extractions on different
  days will not be byte-identical. Record the retrieval date in the fixture README
  (Step 7); that is what makes the committed subset the reproducible artifact
  instead of the download.

```bash
mkdir -p /tmp/jmdict && cd /tmp/jmdict
curl -fL -o JMdict_e.gz http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz
gunzip -kf JMdict_e.gz
mv -f JMdict_e JMdict_e.xml
ls -lh JMdict_e.xml
grep -c '<entry>' JMdict_e.xml
```

Expected: a file tens of megabytes in size (the exact size drifts with every daily
build, so do not assert on it) and an entry count above 200000. If the entry count
is 0, the download is an HTML error page, not XML — delete it and re-check the URL
before going further.

- [ ] **Step 4a: If JMdict cannot be obtained**

**Do not skip the snapshots, do not `#[ignore]` the test, and do not shrink the
corpus silently.** Tell the user JMdict is unavailable and that the fallback
trades away the "real vocabulary" half of addendum §6 while keeping the
"deterministic, offline, version-drift-immune" half. Then, with their agreement:

1. Replace `parse_sentences.txt` with exactly these four sentences (keeping the
   header comment, and updating `corpus_has_thirty_sentences_covering_both_irregulars`
   to expect `4`):

```text
本を読みました。
彼は学校に来る。
勉強をする時間がない。
高い山に登った。
```

   Also replace `matches_suru_through_the_empty_stem_key` with an assertion over
   `勉強をする`'s bare する that pins `conjugation.is_none()` and names the pass-B
   collapse as the reason — the corpus no longer contains a `しました`.

2. Hand-write `ta/crates/jparser/tests/fixtures/jmdict_subset.xml` with the sixteen
   entries those four sentences need, in the same shape as `jmdict_mini.xml`.
   `ent_seq` values are synthetic and only need to be unique, because this file is
   not derived from JMdict:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
<!ENTITY pn "pronoun">
<!ENTITY prt "particle">
<!ENTITY v5m "Godan verb with `mu' ending">
<!ENTITY v5r "Godan verb with `ru' ending">
<!ENTITY vk "Kuru verb - special class">
<!ENTITY vs "noun or participle which takes the aux. verb suru">
<!ENTITY vs-i "suru verb - included">
<!ENTITY adj-i "adjective (keiyoushi)">
]>
<!-- NOT derived from JMdict: hand-written stand-in used because JMdict_e.xml
     could not be obtained. Replace with a real curated subset (see README.md)
     as soon as it can be. -->
<JMdict>
<entry><ent_seq>9000001</ent_seq><k_ele><keb>本</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>ほん</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&n;</pos><gloss>book</gloss></sense></entry>
<entry><ent_seq>9000002</ent_seq><r_ele><reb>を</reb></r_ele><sense><pos>&prt;</pos><gloss>indicates direct object of action</gloss></sense></entry>
<entry><ent_seq>9000003</ent_seq><k_ele><keb>読む</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>よむ</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&v5m;</pos><gloss>to read</gloss></sense></entry>
<entry><ent_seq>9000004</ent_seq><k_ele><keb>彼</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>かれ</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&pn;</pos><gloss>he</gloss><gloss>him</gloss></sense></entry>
<entry><ent_seq>9000005</ent_seq><r_ele><reb>は</reb></r_ele><sense><pos>&prt;</pos><gloss>topic marker</gloss></sense></entry>
<entry><ent_seq>9000006</ent_seq><k_ele><keb>学校</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>がっこう</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&n;</pos><gloss>school</gloss></sense></entry>
<entry><ent_seq>9000007</ent_seq><r_ele><reb>に</reb></r_ele><sense><pos>&prt;</pos><gloss>indicates such things as location</gloss></sense></entry>
<entry><ent_seq>9000008</ent_seq><k_ele><keb>来る</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>くる</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&vk;</pos><gloss>to come</gloss></sense></entry>
<entry><ent_seq>9000009</ent_seq><k_ele><keb>勉強</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>べんきょう</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&n;</pos><pos>&vs;</pos><gloss>study</gloss></sense></entry>
<entry><ent_seq>9000010</ent_seq><k_ele><keb>為る</keb></k_ele><r_ele><reb>する</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&vs-i;</pos><gloss>to do</gloss></sense></entry>
<entry><ent_seq>9000011</ent_seq><k_ele><keb>時間</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>じかん</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&n;</pos><gloss>time</gloss><gloss>hour</gloss></sense></entry>
<entry><ent_seq>9000012</ent_seq><r_ele><reb>が</reb></r_ele><sense><pos>&prt;</pos><gloss>indicates sentence subject</gloss></sense></entry>
<entry><ent_seq>9000013</ent_seq><r_ele><reb>ない</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&adj-i;</pos><gloss>nonexistent</gloss><gloss>not being</gloss></sense></entry>
<entry><ent_seq>9000014</ent_seq><k_ele><keb>高い</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>たかい</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&adj-i;</pos><gloss>high</gloss><gloss>tall</gloss></sense></entry>
<entry><ent_seq>9000015</ent_seq><k_ele><keb>山</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>やま</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&n;</pos><gloss>mountain</gloss></sense></entry>
<entry><ent_seq>9000016</ent_seq><k_ele><keb>登る</keb><ke_pri>ichi1</ke_pri></k_ele><r_ele><reb>のぼる</reb><re_pri>ichi1</re_pri></r_ele><sense><pos>&v5r;</pos><gloss>to climb</gloss></sense></entry>
</JMdict>
```

3. Skip Steps 5, 6 and 10 (there is nothing to extract from and nothing to diff
   against), do Steps 7–9 and 11–13 unchanged, and write the gap into the fixture
   README **and** into this plan's Self-Review: *"the snapshot corpus is
   hand-authored, not real JMdict; redo Task 9 Steps 4–6 and 10 when
   `JMdict_e.xml` is obtainable."*

- [ ] **Step 5: Write the extraction tool**

Create `ta/tools/extract_jmdict_subset.py`. It keeps an entry when **any index key
that entry would produce** occurs as a substring of some corpus sentence, which
makes the filter a provable superset of what the parser can reach, not a
heuristic:

- keys are `unify_str(surface)` for every `<keb>`/`<reb>`, plus one stem per
  remove-tense suffix that actually strips — the same `strip_suffix_unified` rule
  `stem::generate_stems` uses;
- the stem half applies only to entries whose block declares a `<pos>&v…` or
  `<pos>&adj-…` code. Every conjugation-table name is `v*`, `adj-*`, or `copula`,
  and `copula` is never a JMdict POS code, so that predicate is exactly "would
  `record::headwords` attach a non-empty `verb_types`" — the gate loses no
  reachable stem and keeps the fixture from ballooning;
- the suffix list is read from the committed `assets/conjugations.json`, so it
  cannot drift from the table the crate actually loads;
- the empty key is always wanted, which is what keeps `する`/`来る` in;
- all substrings are considered, with no length cap, so nothing is missed by
  truncation.

Entries are copied **verbatim by text slicing**. Do not reach for an XML parser
here: JMdict's POS codes arrive as entity references (`<pos>&v5r;</pos>`), and any
DTD-aware parser would expand them into their English descriptions and destroy
exactly the field `record::headwords` reads.

```python
#!/usr/bin/env python3
# JParser — Japanese text parser ported from Translation Aggregator.
# Copyright (C) 2026
#
# This program is free software; you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 2 as published
# by the Free Software Foundation.
"""Extract the JMdict subset the parse-snapshot sentences can reach.

Usage:
    python3 tools/extract_jmdict_subset.py \\
        /tmp/jmdict/JMdict_e.xml \\
        crates/jparser/tests/fixtures/parse_sentences.txt \\
        crates/jparser/assets/conjugations.json \\
        crates/jparser/tests/fixtures/jmdict_subset.xml

Entries are copied verbatim by text slicing, never re-serialized: JMdict's
parts of speech are entity references and a DTD-aware parser would expand
them into prose, which is precisely the data jparser reads as POS codes.
"""

import json
import re
import sys
from pathlib import Path

ENTRY_RE = re.compile(r"<entry>.*?</entry>", re.DOTALL)
SURFACE_RE = re.compile(r"<(?:keb|reb)>(.*?)</(?:keb|reb)>", re.DOTALL)
# Exactly the POS codes record::headwords can map to a conjugation type: every
# table name is v*, adj-*, or copula, and copula is not a JMdict code.
CONJUGABLE_POS_RE = re.compile(r"<pos>&(?:v|adj-)")

# Mirrors kana::unify (crates/jparser/src/kana.rs). Character-wise, so folding
# a prefix equals the prefix of a folded string — which is what lets this work
# on substrings at all.
HIRAGANA_START, HIRAGANA_END = 0x3041, 0x3096      # end exclusive
HIRAGANA_TO_KATAKANA = 0x60
FULLWIDTH_START, FULLWIDTH_END = 0xFF01, 0xFF20    # end exclusive
FULLWIDTH_TO_ASCII = 0xFEE0

# The tense a type strips to make a stem: its own "Remove" entry if it declares
# one, otherwise "Non-past" (conjugation.rs / contract §1.2).
REMOVE_TENSE = "Remove"
DEFAULT_REMOVE_TENSE = "Non-past"

NOTICE = """<!-- Curated subset of JMdict_e.xml, derived for jparser's parse snapshots.

     Source:  Electronic Dictionary Research and Development Group (EDRDG),
              http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz
     Licence: Creative Commons Attribution-ShareAlike 4.0 International
              (CC BY-SA 4.0), https://www.edrdg.org/edrdg/licence.html
     Notice:  This file contains material from JMdict, Copyright (C) EDRDG.

     Only entries reachable from tests/fixtures/parse_sentences.txt are kept.
     Every kept entry is byte-identical to its JMdict original; nothing is
     rewritten. Regenerate with tools/extract_jmdict_subset.py.
     See tests/fixtures/README.md for the retrieval date. -->
"""


def unify(ch):
    x = ord(ch)
    if HIRAGANA_START <= x < HIRAGANA_END:
        x += HIRAGANA_TO_KATAKANA
    elif FULLWIDTH_START <= x < FULLWIDTH_END:
        x -= FULLWIDTH_TO_ASCII
    c = chr(x)
    return c.upper() if c.isascii() else c


def unify_str(s):
    return "".join(unify(c) for c in s)


def remove_suffixes(asset_path):
    """Every remove-tense/form-0 suffix in the conjugation asset, unified."""
    out = set()
    for ty in json.loads(Path(asset_path).read_text(encoding="utf-8")):
        tenses = ty["Tenses"]
        names = {t["Tense"] for t in tenses}
        remove = REMOVE_TENSE if REMOVE_TENSE in names else DEFAULT_REMOVE_TENSE
        for t in tenses:
            if (
                t["Tense"] == remove
                and not t.get("Formal", False)
                and not t.get("Negative", False)
            ):
                out.add(unify_str(t["Suffix"]))
    return {s for s in out if s}


def wanted_keys(sentences):
    """Every unified substring of the corpus, plus the empty key.

    The empty key is always wanted: する and 来る strip to nothing, so their
    stem matches at every position and they must never be filtered out.
    """
    keys = {""}
    for sentence in sentences:
        u = unify_str(sentence)
        for i in range(len(u)):
            for j in range(i + 1, len(u) + 1):
                keys.add(u[i:j])
    return keys


def is_needed(block, wanted, suffixes):
    surfaces = SURFACE_RE.findall(block)
    for surface in surfaces:
        if unify_str(surface) in wanted:
            return True
    # The stem rule applies only where record::headwords would attach a verb
    # type. Gating on it drops no reachable stem and keeps the subset small.
    if not CONJUGABLE_POS_RE.search(block):
        return False
    for surface in surfaces:
        u = unify_str(surface)
        for suffix in suffixes:
            if u.endswith(suffix) and u[: len(u) - len(suffix)] in wanted:
                return True
    return False


def main(argv):
    if len(argv) != 5:
        print(__doc__, file=sys.stderr)
        return 2
    jmdict_path, sentences_path, asset_path, out_path = argv[1:]

    sentences = [
        line.strip()
        for line in Path(sentences_path).read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    if not sentences:
        print("no sentences: check the corpus path", file=sys.stderr)
        return 1

    wanted = wanted_keys(sentences)
    suffixes = remove_suffixes(asset_path)
    text = Path(jmdict_path).read_text(encoding="utf-8")
    first = text.index("<entry>")

    prolog = text[:first].replace("<JMdict>", NOTICE + "<JMdict>", 1)
    kept = [
        m.group(0)
        for m in ENTRY_RE.finditer(text, first)
        if is_needed(m.group(0), wanted, suffixes)
    ]

    out = Path(out_path)
    out.write_text(prolog + "\n".join(kept) + "\n</JMdict>\n", encoding="utf-8")
    print(f"sentences:     {len(sentences)}")
    print(f"wanted keys:   {len(wanted)}")
    print(f"stem suffixes: {len(suffixes)}")
    print(f"kept entries:  {len(kept)}")
    print(f"output bytes:  {out.stat().st_size}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
```

- [ ] **Step 6: Extract the fixture and check its size**

```bash
cd ta && python3 tools/extract_jmdict_subset.py \
  /tmp/jmdict/JMdict_e.xml \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  crates/jparser/assets/conjugations.json \
  crates/jparser/tests/fixtures/jmdict_subset.xml
```

Expected, both computed directly from the committed asset and corpus:

- `stem suffixes: 19` — the asset's distinct non-empty remove-tense/form-0
  suffixes, which are exactly
  `ある い う うる える く くる ぐ し す する だ つ である ぬ ぶ む る 来る`.
  (The tool holds them unified, i.e. katakana-folded; `unify` is one-to-one over
  the hiragana block, so the count is the same either way. No remove-tense/form-0
  entry in the asset carries a `Next Type`, so load-time trimming never rewrites
  one and reading the raw JSON suffixes cannot drift from the loaded table.)
- `wanted keys: 1431` for the corpus exactly as committed. Any other number means
  the corpus was edited.
- `kept entries` and `output bytes` are **not** predicted here. The surface half of
  the filter pulls every entry whose reading is a corpus substring, and single-kana
  readings like `い` or `し` alone pull dozens; several thousand entries and a
  multi-megabyte file are realistic. Record what you actually get in the commit
  message.

Sanity-check the result:

```bash
grep -c '<entry>' crates/jparser/tests/fixtures/jmdict_subset.xml
grep -c '<ent_seq>' crates/jparser/tests/fixtures/jmdict_subset.xml  # must match
grep -q 'CC BY-SA 4.0' crates/jparser/tests/fixtures/jmdict_subset.xml && echo notice-ok
grep -q '<pos>&' crates/jparser/tests/fixtures/jmdict_subset.xml && echo pos-entities-intact
ls -l crates/jparser/tests/fixtures/jmdict_subset.xml
```

`pos-entities-intact` is the one that matters: if POS arrives as prose
(`<pos>Godan verb …`) instead of `<pos>&v5r;</pos>`, something expanded the DTD
and no verb in the fixture will ever conjugate.

**If the file exceeds ~2 MB**, there are exactly two honest levers and one
forbidden one:

1. Tighten the *surface* half of the filter — for example require a corpus
   substring of at least two characters for kana-only readings. This is **not**
   supersetness-preserving and must be validated by Step 10's diff before it is
   accepted.
2. Shorten the corpus. Cutting a long sentence removes real substrings and is the
   only lever that is safe by construction. **If you do this, also update
   `assert_eq!(all.len(), 30)` in `corpus_has_thirty_sentences_covering_both_irregulars`**
   — Step 4a remembers that; this step used not to.
3. **Forbidden:** loosening the stem rule's POS gate or dropping remove suffixes.
   Both drop candidates the DP would really have seen and make the snapshot
   describe a dictionary that does not exist.

- [ ] **Step 7: Write the attribution README**

Create `ta/crates/jparser/tests/fixtures/README.md`. Fill in the retrieval date
with the date Step 4 actually ran — Step 13 greps for the placeholder and fails if
you did not:

````markdown
# Test fixtures

## `jmdict_mini.xml`

Hand-written, three entries, no external source. Used by
`tests/index_roundtrip.rs`, `tests/cli_parse.rs`, and `record.rs`'s own tests.

## `jmdict_matcher.xml`

Hand-written, eight entries, no external source. Used by `src/matcher.rs`'s own
test module. Covers a hiragana/katakana homophone pair, a particle, two
homographs, a nested-prefix pair, a `v1` verb, and a `vs-i` verb whose stem is
the empty string.

## `jmdict_subset.xml`

A curated subset of **JMdict**, containing only the entries the sentences in
`parse_sentences.txt` can reach. Used by `tests/parse_snapshots.rs`.

- **Source:** Electronic Dictionary Research and Development Group (EDRDG),
  <http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz>
- **Retrieved:** YYYY-MM-DD (fill in; JMdict is rebuilt daily, so the committed
  subset — not the download — is the reproducible artifact)
- **Licence:** Creative Commons Attribution-ShareAlike 4.0 International
  (CC BY-SA 4.0), <https://www.edrdg.org/edrdg/licence.html>
- **Notice:** This file contains material from JMdict, Copyright (C) EDRDG.

This is third-party **data**, not source: it carries the EDRDG notice in its own
XML header instead of the crate's GPL v2 header, and it is not relicensed. The
crate is `GPL-2.0-only` and CC BY-SA 4.0 is one-way compatible with GPL v3, not
v2; the subset is kept as a separately-licensed data asset for that reason.

Regenerate with:

```bash
python3 tools/extract_jmdict_subset.py \
  /path/to/JMdict_e.xml \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  crates/jparser/assets/conjugations.json \
  crates/jparser/tests/fixtures/jmdict_subset.xml
```

## `parse_sentences.txt`

The snapshot corpus. Adding a sentence requires re-running the extractor against
a real `JMdict_e.xml` — the committed subset will not contain the new
vocabulary, and the snapshot will show unmatched spans instead of failing
loudly.
````

- [ ] **Step 8: Generate the snapshot baseline and read it**

```bash
cd ta && INSTA_UPDATE=always cargo test -p jparser --test parse_snapshots
```

Expected: all four tests pass, and
`crates/jparser/tests/snapshots/parse_snapshots__sentences.snap` is written. (If
this build of insta rejects `always`, use `INSTA_FORCE_UPDATE=1` instead; either
writes the `.snap` in place, so `cargo-insta` does not need installing.)

**Now actually read it.** This is the human-verification step the whole phase
exists for; a snapshot accepted without being read is a snapshot of a bug. The
baseline is generated from whatever the implementation does, so this checklist is
the only backstop for the rules no unit test can reach end to end. Check, at
minimum:

- `猫が好きです。` splits as **`猫 / が / 好きです / 。`** — four spans, not five.
  `adj-na` (asset index 1) declares an explicit `Remove` tense with an empty
  suffix, so every `adj-na` word's stem is the word itself (`好き`), and its
  conjugation 4 is `Non-past / Formal / です`, terminal. One 4-char span costs
  `10 − 3 (COMMON) = 7`; `好き` + `です` costs `7 + 7 = 14`, so the DP takes the
  single span. **A split into `好き / です` is the bug, not the fix.**
- every `。` is its own `unmatched` span of `len=1`;
- a segmentation that is one long run of single-char matches means matches are not
  being found — five `SKIP_CHAR` hits cost 500, so real matches should dominate;
- `言われなかった` carries the label **`Passive Negative Past`**, not `-` and not
  `Negative Past`. `言う` is `&v5u;` in real JMdict, so the stem is exact:
  `言` + `v5u` Passive `われる`, trimmed to `われ` by `v1`'s remove suffix `る`,
  then `v1` `Past / Negative / なかった`, terminal. Depth 0 contributes
  `"Passive "` (no form bits); depth 1 contributes `"Negative "` then `"Past"`;
- readings are kana for kanji-bearing spans and `-` for kana-only entries. The `-`
  there comes from `reconstruct_reading` **step 2** — an empty `data.readings`,
  because §2 stores readings only for kanji-bearing entries — **not** step 1: a
  kana-only entry's record gets `PRIMARY`, never `PRONOUNCE` (`record.rs:140-148`).
  Step 1 fires for the kana *reading of a kanji-bearing entry*, e.g. the `する`
  record of 為る. A reading printed over a kana-only surface is a bug;
- `start`/`len` are contiguous: each segment's `start + len` equals the next
  segment's `start`, and the last one equals the sentence's char count. A gap or an
  overlap means the backtrack lost a span.

Anything that looks wrong is a bug in Tasks 1–7, not something to accept. Fix it
there and regenerate.

- [ ] **Step 9: Re-run clean to confirm the snapshot is stable**

Run: `cd ta && cargo test -p jparser --test parse_snapshots`

Expected: `test result: ok. 4 passed; 0 failed`, with no `.snap.new` file left
behind (`ls crates/jparser/tests/snapshots/` shows only `*.snap`). A `.snap.new`
means the parse is not deterministic across runs — most likely an iteration order
that depends on a `HashMap`, which would make every future snapshot review
worthless.

- [ ] **Step 10: Prove the subset reproduces the full dictionary's parse**

This is what makes the curated fixture trustworthy, and it can only be done while
`JMdict_e.xml` is still on disk. Build both indexes and diff the CLI's output —
the format Task 8 froze — sentence by sentence. **Release profile throughout:**
this is a behavioural cross-check, so the profile is free, and `index/build.rs`'s
`push()` does an O(n) `bucket.contains` scan per insert, which makes a 200k-entry
debug build dramatically slower than a release one.

```bash
cd ta
cargo build -q --release -p jparser --bin jparser-cli
cargo run -q --release -p jparser --bin jparser-cli -- \
  build-index /tmp/jmdict/JMdict_e.xml /tmp/jparser-index-full
cargo run -q --release -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_subset.xml /tmp/jparser-index-subset
```

Expected: both report `skipped entries: 0`. The full build reads ~200k entries; a
few minutes in release is normal.

```bash
bash -c '
fail=0
while IFS= read -r s; do
  case "$s" in ""|"#"*) continue;; esac
  a=$(./target/release/jparser-cli parse /tmp/jparser-index-full  "$s")
  b=$(./target/release/jparser-cli parse /tmp/jparser-index-subset "$s")
  if [ "$a" != "$b" ]; then
    fail=1
    echo "MISMATCH: $s"
    diff <(printf "%s\n" "$a") <(printf "%s\n" "$b")
  fi
done < crates/jparser/tests/fixtures/parse_sentences.txt
echo "mismatches: $fail"
'
```

Expected: `mismatches: 0` and no `MISMATCH:` lines.

A mismatch is **not** a licence to widen the filter and move on. The filter is an
exact superset of the keys the parser can reach, so a difference means one of
those assumptions is false — in order of likelihood: `unify()` in the Python tool
has drifted from `kana::unify`; `stem::generate_stems` produces a key shape the
`strip_suffix_unified` rule above does not model; the `CONJUGABLE_POS_RE` gate is
excluding an entry whose POS code the conjugation table does reach; or the
entry-slicing regex missed an entry whose markup differs (e.g. `<entry >`).
Diagnose which, fix the tool, re-extract, and re-run Steps 6, 8 and 9.

- [ ] **Step 11: Hold the 80% coverage line**

```bash
cd ta
cargo llvm-cov --version || cargo install cargo-llvm-cov
cargo llvm-cov -p jparser --summary-only --fail-under-lines 80
```

Expected: exit 0, with the summary at or above 80% line coverage on
`crates/jparser`. Phase 1A finished at 93.85%.

If it is short, the gaps are predictable and each has a real test rather than a
lowered target:

- **`NAME_DICT_BAD_PER_CHAR` / `NAME_DICT_OK` in `score_match`** — unreachable from
  any parse, because nothing sets `WordFlags::IS_NAME` in v1. Already covered by
  Task 5's three hand-built `IS_NAME` tests
  (`an_isolated_katakana_name_takes_name_dict_ok`,
  `a_name_glued_to_more_katakana_is_priced_out`,
  `an_inexact_name_is_bad_even_inside_an_isolated_run`). Nothing to add.
- **`recurse`'s depth cap** — the `Some(_) => {}` arm at
  `depth == MAX_CONJ_DEPTH - 1`. Covered by Task 2's
  `a_branch_needing_a_sixth_layer_is_dropped_whole` and its synthetic six-level
  `DEEP_JSON` chain. No corpus sentence can reach it.
- **`recurse`'s Potential-Potential drop** — likewise unreachable from any corpus
  sentence; covered by Task 2's `a_potential_chained_into_a_potential_is_dropped`
  and `a_repeated_non_potential_tense_survives` against `POTENTIAL_JSON`.
- **`recurse`'s informal-Stem-at-depth>0 arm** — exercised by corpus sentence 13
  (`話せます` = `v5s` Potential `せ` → `v1` Stem `""` → `v-i-stem` `ます`, label
  `Potential Formal`), but it produces *identical output* whether the rule is
  right or wrong, so the real assertion is Task 2's structural `chain.len()` check
  in `an_informal_stem_above_depth_zero_leaves_no_chain_link`. Do not let a green
  snapshot stand in for it.
- **`kuru_hack`'s `want > KURU_HACK_MAX_CHARS` continue** — genuinely uncovered.
  No real `vk` slot needs four substitution characters, so it needs a synthetic
  `ConjugationTable::from_json` table in `lib.rs`'s test module whose twin suffix
  is four or more characters longer than the kanji one. Add it there if this is
  what tips the number below 80.
- **`jparser-cli`'s `build-index` and `lookup` arms** — exercised by hand in
  Phase 1A, never by a test. If the bin's uncovered lines are what tip the number
  below 80, extend `tests/cli_parse.rs` with a `lookup` invocation rather than
  excluding the binary from the measurement.

- [ ] **Step 12: Format and lint the new files**

```bash
cd ta && rustfmt --edition 2021 crates/jparser/tests/parse_snapshots.rs
cargo clippy -p jparser --tests -- -D warnings
python3 -m py_compile tools/extract_jmdict_subset.py && echo tool-compiles
```

Expected: clippy clean and `tool-compiles`. Never run `cargo fmt -p jparser`.

- [ ] **Step 13: Commit**

First, fail loudly if the README placeholder is still there:

```bash
cd ta && ! grep -q 'YYYY-MM-DD' crates/jparser/tests/fixtures/README.md \
  || { echo "FILL IN the Retrieved: date in tests/fixtures/README.md"; false; }
```

Then:

```bash
cd ta && git add \
  tools/extract_jmdict_subset.py \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  crates/jparser/tests/fixtures/jmdict_subset.xml \
  crates/jparser/tests/fixtures/README.md \
  crates/jparser/tests/parse_snapshots.rs \
  crates/jparser/tests/snapshots/parse_snapshots__sentences.snap
git commit -m "test: add insta snapshots over 30 real sentences

Port design §10 calls these the highest-value tests in the crate: they
answer 'did my refactor change the parse?' in a way no unit test can.
They run against a curated JMdict subset committed to the repo, so the
suite works offline from a fresh clone and does not move when JMdict is
rebuilt, while still exercising real vocabulary.

tools/extract_jmdict_subset.py keeps an entry when any index key that
entry would produce — its surfaces, plus one stem per remove-tense
suffix that actually strips, for entries whose POS is one the
conjugation table can reach — occurs as a substring of some corpus
sentence. That is a provable superset of what the parser can reach
rather than a heuristic, and it was verified by diffing jparser-cli
parse output for all 30 sentences between an index over the full
JMdict_e.xml and one over the subset: identical.

Entries are copied verbatim by text slicing. A DTD-aware parser would
expand JMdict's part-of-speech entity references into their English
descriptions and destroy the exact field record::headwords reads.

Six sentences cover する and 来る, whose generated stem is the empty
string. That path returns a key_chars == 0 hit at every position, broke
late in Phase 1A, and now has both a snapshot and two targeted
assertions standing over it — the する one deliberately asserts しました
rather than a bare する, because sort_matches' verb-plain collapse drops
the empty-stem match in favour of the plain headword at that span.

The subset is third-party data under CC BY-SA 4.0 (EDRDG), attributed
in the fixture header and in tests/fixtures/README.md, and is not
relicensed under the crate's GPL v2."
```

---

## Self-Review

**1. Spec coverage.** Phase 1B claims the rest of port design §11's Phase 1 —
everything Phase 1A deferred:

| Spec requirement | Task |
|---|---|
| §5.3 `matches_at`, non-verb branch, strict vs loose comparison, the three post-filters | 1 |
| §5.3 `FindVerbMatches` recursion: Remove-tense skip, Stem-skip, `slen` advance, depth cap, Potential-Potential | 2 |
| §5.5 conjugation label rendering (`GetConjString`) | 3 |
| §5.7 `BoundaryHints` trait, no Vibrato implementation | 4 |
| §5.4 the DP: skip transition, `SKIP_CHAR`/`SKIP_KANJI_EXTRA`, strict-`>` tie | 4 |
| §5.4 the remaining eleven scoring constants, `score_match` clause order, `counter_after_number`, `isolated_katakana_run`, `>=` match tie | 5 |
| §5.4 backtrack collection pass, stale-`COUNTER` clear, `SortMatches` (dedupe + rank) | 6 |
| Addendum §1 `EntryData.readings`, `INDEX_FORMAT_VERSION = 3` | 7 |
| §5.1 `parse()`, `ParseResult`, `Segment`, `Entry`, `ParseError`, `ParseOptions` | 7 |
| §5.6 reading reconstruction (`JParseWindow.cpp:186-208`) and the `kuruHack` twin pairing | 7 |
| §11 "verifiable via a CLI harness" — the `parse` subcommand | 8 |
| §10 insta snapshots over ~30 real sentences, including する and 来る | 9 |
| §10 80% coverage on the crate | 9 |
| Addendum §6 standing empty-stem regression | 7 (`parse_irregular.rs`), 9 (corpus + two targeted assertions) |

**Deferred, with the owning phase, per addendum §1:** Vibrato/`morph.rs` (5),
furigana display modes and the `to_katakana` conflict (3), the differential run
against ta-old (6 — *not* 1B, correcting both the Phase 1A plan and the Phase 1A
handoff), the Tauri shell and generation-directory index layout (2), JMnedict and
`IS_NAME` (deferred), the half-width katakana offset map (deferred).

**2. Placeholder scan.** No `TBD`, no `TODO`, no "implement later", no "similar to
Task N", and no test that asserts nothing. Every code step carries runnable code
and every test step carries a concrete expected value — 94 assertions in the
segmenter group alone, all re-derived from `Dictionary.cpp:1160-1262`. There is
exactly one intentional literal placeholder in a *deliverable*: the
`**Retrieved:** YYYY-MM-DD` line in `tests/fixtures/README.md`, which the operator
must fill with the real fetch date, and which Task 9 Step 13 greps for and refuses
to commit around.

**3. Type consistency across task boundaries.** Checked:

- `ParseError` is created in Task 1 (the matcher needs it to name `matches_at`'s
  error type) and *verified, not redefined*, in Task 7 Step 5. One definition,
  character-identical to contract §3.3.
- `WordFlags` is `u16`; `StoredRecord::flags` is a raw `u16`; the matcher wraps
  with `WordFlags(record.flags)`. `contains` is an exact-subset test, so the
  `COMMON_BONUS` condition is two calls, never a combined mask.
- `Match::chain` empty ⇔ non-verb, at every consumer: `matches_at`'s non-verb arm,
  `score_match` (untouched by chains), `rank::group_key`'s `(id, 0, 0, 0)`,
  `rank::verb_plain_collapses`, `dictionary_form`, `reconstruct_reading`,
  `assemble_entry`'s `conjugation: None`. Nobody encodes "not a verb" as
  `verb_type == 0`; that was ta-old's 1-based scheme and Phase 6 re-adds the `+ 1`.
- `VerbTypeId` is 0-based and is an index into `ConjugationTable::types()`
  everywhere: `ConjLink::verb_type`, `StoredRecord::verb_type`,
  `strip_remove_suffix`, `kuru_hack`. `rank::group_key` adds 1 *only* inside its
  sort key, to reproduce ta-old's ordering; nothing else does.
- `types_named` returns `Vec<VerbTypeId>` and every caller treats it as
  multi-valued: `kuru_hack` and `reconstruct_reading` iterate it and skip
  `twin == link.verb_type`; the tests use `vk_kanji`/`only` rather than `[0]` where
  the name is ambiguous.
- `Match::src_len` vs `Match::len`: `src_len` is the FST key length,
  `len = src_len + Σ suffix chars`. Equal for a non-verb match. `reconstruct_reading`
  slices `text[start + src_len .. start + len]` for the tail and
  `text[start + src_len + 1 .. start + len]` for the kuruHack rest; both are
  bounds-checked with `get`, and the second one `continue`s rather than aborting
  (§6.6 step 4).
- `unified_eq` is `pub(crate)` because `lib.rs::tails_match` calls it; `strict_eq`
  stays private to `matcher` and is reachable from `matcher::verb` only because
  `verb` is a child module. That is the whole reason the Task 2 split is a child
  and not a sibling.
- `sort_matches(&mut Vec<Match>)` takes `&mut Vec`, not `&mut [Match]`, because
  Pass B truncates. `segment::backtrack` owns the clone it passes.
- `Segmentation::total_cost` is read only by `segment.rs`'s tests, and carries the
  single narrowest `#[allow(dead_code)]` in the phase, on the field. Without it
  Task 7 Step 9's `cargo clippy --all-targets -- -D warnings` fails on
  "field is never read".
- `EntryData` field order is `id, readings, senses` — bincode is positional, and
  the version bump to 3 is what makes an old index refuse to load rather than
  mis-decode.
- Char offsets everywhere. The only `&str`/byte boundary in the phase is
  `text.chars().collect()` at the top of `parse` and the `String` the matcher
  builds for `prefixes_of`, whose `key_chars` is a char count by Phase 1A's
  contract.

**4. Corrections applied during review** (each was caught by verifying the drafted
task text against the real crate, the real asset, or a compiled reconstruction —
not by reading):

1. `strict_eq` used `to_ascii_lowercase()` on both sides, which trips
   `clippy::manual_ignore_case_cmp` — a hard error under `-D warnings` on the
   pinned toolchain. Replaced with `!lhs.eq_ignore_ascii_case(&other)`.
2. Task 2's widened import added `TENSE_NON_PAST` with no non-test reader, an
   `unused_imports` error that `#[allow(dead_code)]` does not cover. Resolved by
   the module split: the recursion's constants live in `verb.rs` and
   `TENSE_NON_PAST` is a test-module import there.
3. The matcher's 800-line budget was wrong. Measured post-rustfmt it is 411 / 879 /
   1033 across Tasks 1–3, i.e. already 79 lines over the cap before Task 3 begins,
   and the contingency the draft named (pulling out the label renderer) would not
   have been enough. Resolved up front by splitting the recursion into
   `matcher/verb.rs`, and flagged in File Structure rather than discovered at a
   Step 6.
4. Task 4's expected dead-code list omitted `function 'backtrack' is never used`.
   rustc lints every unreachable item, not just the root of the dead chain.
5. Task 5's counterfactual comment for the skip tie-break claimed `>=` would give
   `[(0, 3, false)]`; the real result is
   `[(0, 1, false), (1, 1, true), (2, 1, false)]`.
6. Task 5's rationale for "bucket order is load-bearing" was vacuous — two matches
   in the same bucket reaching the same `next` write the same `back_len`. Bucket
   order *is* load-bearing, for `sort_matches`' stable Pass C and the matcher's
   dedup; the instruction stands with the correct reason.
7. Two coverage gaps closed in Task 5: the else-if chain's *leg order* (a
   single-char match between two digits must cost `+1`, not `+100`) and a skipped
   run with a matched span on both sides.
8. `sort_matches` merged back into `segment.rs` measures **≈990** lines, not the
   1021 the draft claimed as measured. Conclusion unchanged; the figure is now
   derived in the text.
9. Task 7's `reconstruct_reading` used `?` on the kuruHack `rest` slice, aborting
   the whole function where §6.6 step 4 says continue to the next reading.
   Replaced with a `let … else { continue }`.
10. Task 7's `link()` helper cannot disambiguate `vk`'s two form-0 chained `来られ`
    rows (Potential and Passive, both into `v1`), so the test named
    "…a_chained_suffix…" would have passed without testing its claim. It now
    asserts the tense it got.
11. Task 8's CLI test took `&PathBuf`, which trips `clippy::ptr_arg` under the
    `--tests -D warnings` gate the same task runs. Changed to `&Path`.
12. Task 9's `matches_suru_through_the_empty_stem_key` asserted
    `Some("Non-past")`; the real value is `None`, and the test did not exercise the
    empty-stem path at all — `sort_matches` pass B rule 3 drops the empty-stem verb
    match in favour of the plain `する` headword indexed at the same span. Replaced
    with a `しました` assertion (`Formal Past`), which is immune because
    `chain[0].tense` is `Stem`.
13. Three wrong facts in Task 9's human review checklist, the one place a wrong
    expectation makes a reviewer "fix" correct code: `猫が好きです。` splits as
    `猫 / が / 好きです / 。` (four spans, `adj-na`'s `です` is a terminal
    conjugation); `言われなかった` renders `Passive Negative Past`, not
    `Negative Past`; and a kana-only entry's `-` reading comes from
    `reconstruct_reading` step 2, not step 1.
14. Task 9's size remedy told the operator to shorten the corpus without updating
    `assert_eq!(all.len(), 30)`, and offered no supersetness-preserving
    alternative. Both fixed, and the POS gate that keeps the fixture small is now
    in the tool from the start.
15. Task 9's cross-check ran `cargo run` in debug against a 200k-entry JMdict with
    an O(n)-per-insert bucket scan. Switched to `--release` throughout.
16. Task 9 had no format/lint step for the two files it creates, and no check that
    the README's `YYYY-MM-DD` placeholder was filled. Both added.
17. Task 9's coverage-gap list assigned the `IS_NAME` test to a task that already
    has three, and never mentioned Potential-Potential. Corrected, with each gap
    pointed at the test that actually covers it.

**5. Documented fidelity divergences from ta-old** (each is deliberate, each is
commented at the site, and each is a candidate finding for Phase 6's differential
run):

- **Potential-Potential drop uses `retain`, ta-old used swap-remove**
  (`Dictionary.cpp:784-790`), which permutes the surviving siblings. Emission order
  feeds the DP's `>=` tie-break, so this is a real if small difference. The
  contract's pseudocode mandates `retain`.
- **`dictIndex` dropped, `firstJString` replaced by `entry_id` ascending.**
  ta-old's key was a heap address and was never reproducible across runs.
- **`inexactMatch` narrowed from a tri-state `int` to a `bool`.** Its sign
  reflected alphabetical order, not match quality.
- **`dictionary_form` uses the *first* remove-tense/form-0 conjugation, not the
  first that strips.** For `copula`, `adj-i`, and `v5uru` — the three types
  declaring two — a stem generated through the second suffix reconstructs with the
  first. Fixing it needs the original headword, which the index does not store; it
  is a contract decision.
- **`strict_eq` folds ASCII case only**, where ta-old's `wcsnicmp` was
  `CompareStringW` with `NORM_IGNORECASE` and folded non-ASCII case too. Japanese
  never hits it; a mixed-script line could.
- **Skipped runs are emitted as unmatched `Segment`s**, where ta-old emitted
  nothing. Required so `parse` can return a contiguous cover.
- **The zero-length match drop** is new, and is behaviour-preserving: the cheapest
  possible match delta is `+10 −2 −3 −2 = +3 > 0`, so the DP could never choose one,
  and it removes a self-loop hazard.

**6. Residual gaps a human should look at:**

- `matcher/verb.rs`'s Stem-skip arm advances neither `depth` nor the cap, so a
  zero-width `Stem`/form-0 *cycle* in a conjugation asset would recurse until the
  stack overflows. The shipped asset has six zero-width stem-skip edges and is
  acyclic, and `Index::open`'s fingerprint check binds an index to its asset — so
  there is no hazard today. `from_json` nonetheless accepts arbitrary JSON, and
  nothing guards or tests it. A `ponytail:` comment names the ceiling and the
  upgrade path; decide whether that is enough.
- The `kuru_hack` `want > 3` branch is uncovered and has no test in any task.
- CC BY-SA 4.0 (JMdict) versus `GPL-2.0-only` (the crate). The subset is committed
  as a separately-licensed data asset, which is standard practice, but it wants a
  human decision rather than a task's.
- Contract §7's module map and test-home table are now stale in four rows
  (`recurse` → `matcher/verb.rs`, `sort_matches` → `rank.rs`, matcher tests split
  across two files, `tests/parse_irregular.rs` added). The File Structure table in
  this plan is authoritative; fold the rows back into the contract if it is going
  to outlive this plan.
- Port design §10's "assert the cost" is satisfied only by `segment.rs`'s in-module
  tests, because `Segmentation::total_cost` never reaches `ParseResult`. If cost
  regressions must be caught end to end, that is a contract change.

---

## Execution Handoff

Plan complete and saved to
`docs/superpowers/plans/2026-08-13-jparser-phase1b.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task,
review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans,
batch execution with checkpoints.

Which approach?
