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

use jparser::record::WordFlags;

fn open_v2(dir: &Path) -> jparser::index::load::Index {
    jparser::index::load::Index::open_with(dir, jparser::engine::build::fingerprint())
        .expect("open v2 index")
}

const FIXTURE: &str = include_str!("fixtures/jmdict_subset.xml");
const SENTENCES: &str = include_str!("fixtures/parse_sentences.txt");

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
    let index = jparser::index::load::Index::open_with(&dir, jparser::engine::build::fingerprint())
        .expect("open v2 index");
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

    let err = jparser::index::load::Index::open(&v2_dir)
        .expect_err("v1's fingerprint must not open a v2-built index");
    assert!(
        matches!(err, jparser::index::IndexError::ConjugationMismatch { .. }),
        "expected a fingerprint mismatch, got {err:?}"
    );

    let err =
        jparser::index::load::Index::open_with(&v1_dir, jparser::engine::build::fingerprint())
            .expect_err("v2's fingerprint must not open a v1-built index");
    assert!(
        matches!(err, jparser::index::IndexError::ConjugationMismatch { .. }),
        "expected a fingerprint mismatch, got {err:?}"
    );
}

// `cands` opens with `open_v2` (`Index::open_with` plus the v2 fingerprint)
// rather than the bare `Index::open` the task-8 brief's snippet shows:
// `Index::open` checks the index against v1's conjugation fingerprint, and a
// v2-built index fails that guard by design (see the test above) -- v1's
// open can never read a v2 index.
fn cands(dir: &Path, text: &str) -> Vec<Vec<jparser::engine::lex::Candidate>> {
    let index = open_v2(dir);
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
    // No deinflection fires here at all (see the gap test below) -- what
    // this exercises is a plain headword collision. The span "ハイ" folds to
    // the same FST key as the existing headword "はい" (a reading of 杯/盃,
    // ent_seq 2019640), and that stored surface differs from the raw
    // katakana query, so the match must come back marked inexact.
    let dir = scratch("lex-inexact");
    build_v2(&dir);
    let c = cands(&dir, "ハイタ");
    let hit = c[0]
        .iter()
        .find(|x| x.headword == "はい")
        .expect("ハイ -> はい (杯/盃's reading) is the kana-unified match");
    assert_eq!(hit.len, 2, "the match covers ハイ, not the whole katakana span");
    assert!(!hit.exact, "はい (hiragana) != the raw katakana query ハイ");
}

/// Opens a fresh v2 index, builds a fresh `Table`, and runs `parse_v2` with
/// whatever boundary source `bounds` names.
fn parse_v2_with_bounds(
    dir: &Path,
    text: &str,
    bounds: Option<&dyn jparser::engine::Boundaries>,
) -> jparser::ParseResult {
    let index = open_v2(dir);
    let table = jparser::engine::table::Table::new();
    jparser::engine::parse_v2(&index, &table, text, &jparser::ParseOptions::default(), bounds)
        .expect("parse_v2")
}

/// `parse_v2_with_bounds` with no boundary source -- the shape every assemble
/// test below needs and nothing more.
fn parse_v2_over(dir: &Path, text: &str) -> jparser::ParseResult {
    parse_v2_with_bounds(dir, text, None)
}

/// Locates the real Vibrato IPADIC dictionary: `TA_HINTS_DICT` if set (the
/// same override `hints.rs`'s doc comments and the CLI's `--hints`/startup
/// path already use), else the path `jparser-cli fetch-hints` publishes to by
/// default on this machine.
#[cfg(feature = "mecab")]
fn dictionary_path() -> PathBuf {
    if let Ok(path) = std::env::var("TA_HINTS_DICT") {
        return PathBuf::from(path);
    }
    PathBuf::from(std::env::var("APPDATA").expect("APPDATA must be set on Windows"))
        .join("dev.jparser.clipyomi")
        .join("hints")
        .join("current")
        .join("system.dic")
}

/// Real Vibrato boundaries for `text`, needed only by the one test that pins
/// master plan success criterion 3 against genuine IPADIC tokenization rather
/// than a hand-built stand-in dictionary.
#[cfg(feature = "mecab")]
fn boundaries_over(text: &str) -> jparser::hints::BoundarySet {
    let path = dictionary_path();
    let tokenizer = jparser::hints::VibratoTokenizer::load(&path)
        .unwrap_or_else(|e| panic!("load the vibrato dictionary at {}: {e}", path.display()));
    tokenizer.boundaries(text)
}

#[cfg(feature = "mecab")]
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

/// Every character offset `bounds` marks as a cut, `0` and `text`'s char
/// length included. Mirrors `jparser-cli`'s own private
/// `diff::boundaries_from_segments` (unreachable from this integration test
/// binary), computed the other direction: from a `Boundaries` source rather
/// than a `Segment` cover.
#[cfg(feature = "mecab")]
fn cuts_of(text: &str, bounds: &dyn jparser::engine::Boundaries) -> Vec<usize> {
    (0..=text.chars().count())
        .filter(|&p| bounds.is_boundary(p))
        .collect()
}

/// The cut offsets a `ParseResult`'s own segments imply: every `start`, plus
/// the final segment's `start + len`. See [`cuts_of`] -- comparing this
/// against a real `Boundaries` source's own cuts is what
/// `the_particle_absorbing_merges_are_gone` needs.
#[cfg(feature = "mecab")]
fn segment_cuts(segments: &[jparser::Segment]) -> Vec<usize> {
    let mut out: Vec<usize> = segments.iter().map(|s| s.start).collect();
    if let Some(last) = segments.last() {
        out.push(last.start + last.len);
    }
    out
}

#[cfg(feature = "mecab")]
#[test]
#[ignore = "24 of 25 rows land exactly on Vibrato's own isolated-surface cuts. \
            いんじゃない does not: Vibrato cuts it at [0, 2, 4, 6] (いん/じゃ/ない), \
            but v2 still crosses the boundary at 2, matching いんじゃ (0..4, chained \
            through いん [2511890] 淫/婬) as one candidate and leaving ない separate \
            -- got [0, 4, 6] [\"いんじゃ\", \"ない\"]. This is the defect class itself \
            surviving for one surface, not the shattering defect \
            `a_compound_of_two_common_kanji_should_not_shatter` pins above: the DP \
            still prices crossing that one interior cut as cheaper than respecting \
            it. Per this task's brief (`task-15-brief.md` step 4), the fix is \
            raising PRONOUNCE_CROSS (`cost.rs`) against the corpus in Task 16, not \
            special-casing this surface out of the fixture -- cost.rs is off limits \
            to this task. Unignore once that lands and this passes unmodified."]
fn the_particle_absorbing_merges_are_gone() {
    // Handoff §9.5's defect class. The metric cannot catch this: the tolerant
    // accuracy *credits* these merges, because their endpoints are both gold
    // boundaries. Spec §2.3.
    //
    // The brief's own sketch asserts only non-merger (`segments.len() != 1`),
    // and that is vacuous in this tree: `a_compound_of_two_common_kanji_should_not_shatter`
    // above already pins a separate, real cost-model defect (flat COMMON
    // discount) that shatters 63 of 81 multi-character spans v1 keeps whole
    // across the whole gold corpus under `bounds = None` -- *nothing* comes
    // back as one segment there, so a bare non-merger check would pass
    // whether or not this defect were reintroduced.
    //
    // Supplying real, isolated-surface Vibrato boundaries (`boundaries_over`,
    // the same helper `kurumadehakou_parses_as_four_words` uses) sidesteps
    // that rather than reproducing it: `cost::is_allowed` only permits a
    // candidate whose own start *and* end both land on a `bounds` cut, so v2
    // cannot cut *inside* a single Vibrato token here regardless of the
    // shattering defect above -- the only freedom the DP keeps is whether to
    // *cross* an interior cut by matching a longer span, which is exactly the
    // spurious-merge decision this defect class is about. So "the correct
    // segmentation" for each isolated surface is definitionally "wherever
    // Vibrato itself cuts it", and comparing v2's own segment cuts against
    // Vibrato's cut set is a real assertion rather than the vacuous one --
    // and it needs no hand-transcription of the fixture's third column, which
    // documents the wider sentence each merge was measured in (sometimes a
    // lemma, sometimes text outside the isolated surface) rather than always
    // being a literal reconstruction of the surface alone -- see the
    // fixture's header and `spurious_merges.txt`'s own commit for the
    // per-row reasoning.
    const FIXTURE: &str = include_str!("fixtures/spurious_merges.txt");
    let dir = scratch("spurious");
    build_v2(&dir);

    let mut wrong = Vec::new();
    for line in FIXTURE
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let surface = line.split('\t').next().expect("surface");
        let bounds = boundaries_over(surface);
        let want = cuts_of(surface, &bounds);
        let r = parse_v2_with_bounds(&dir, surface, Some(&bounds));
        let got = segment_cuts(&r.segments);
        if got != want {
            let surfaces: Vec<&str> = r.segments.iter().map(|s| s.surface.as_str()).collect();
            wrong.push(format!(
                "{surface}: v2 cut at {got:?} {surfaces:?}, Vibrato cuts at {want:?}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "did not land on Vibrato's own cuts:\n{}",
        wrong.join("\n")
    );
}

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

#[test]
fn a_matched_segment_carries_its_entry_and_senses() {
    // Not 東京 (the brief's own example): 東 and 京 are each independently a
    // COMMON-priority entry in this fixture (ent_seq 1447440 and 1233320),
    // so splitting into two one-character COMMON matches costs 2 less than
    // one COMMON two-character match -- `cost.rs`'s COMMON discount applies
    // per match, not per character, and nothing in Phase 1a's cost model
    // (Task 16's to tune) prefers the compound here. 学校 has no such
    // competing split: 学 alone is COMMON but 校 alone is not, so the
    // one-match/two-match costs tie and `lattice::relax`'s longer-run
    // tie-break keeps it whole -- confirmed against this fixture directly
    // rather than assumed.
    let dir = scratch("assemble-basic");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "学校");
    let seg = &r.segments[0];
    assert!(seg.matched);
    assert_eq!(seg.surface, "学校");
    assert!(!seg.entries.is_empty());
    assert!(!seg.entries[0].senses.is_empty());
    assert!(!seg.entries[0].pos.is_empty());
    // ent_seq 1206730 in tests/fixtures/jmdict_subset.xml, <keb>学校</keb>.
    assert_eq!(seg.entries[0].entry_id, 1206730, "the ent_seq entry top-1 is scored on");
}

// The general shape, not just this one word: with `COVER` priced per
// character, a flat *per-match* discount (`COMMON`, -2) rewards more matches
// over fewer, and a flat per-match penalty (`INEXACT`, +8) rewards fewer
// over more -- exactly the "fewer words wins" bug this engine exists to
// remove, reintroduced from the discount side. The model is
// granularity-neutral only when every candidate on both sides of a split
// carries identical per-match flags, which real text never guarantees.
// Common single kanji are everywhere in Japanese, so this generalises well
// past 東京.
#[test]
#[ignore = "cost.rs's COMMON discount (-2) is flat per match, not \
            length-proportional: 東京 as one COMMON match costs 2*1 - 2 = 0, \
            but 東 (COMMON, ent_seq 1447440) + 京 (COMMON, ent_seq 1233320) as \
            two COMMON matches costs (1 - 2) + (1 - 2) = -2, so the split wins \
            and 東京 shatters. Candidate fix: make COMMON length-proportional \
            (-2 * len), which ties the merge against its parts instead of \
            losing to them. Task 16 owns cost.rs tuning; unignore once it lands."]
fn a_compound_of_two_common_kanji_should_not_shatter() {
    let dir = scratch("assemble-compound-shatter");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "東京");
    assert_eq!(r.segments.len(), 1, "東京 must not shatter into 東 + 京");
    assert_eq!(r.segments[0].surface, "東京");
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
    assert!(r
        .segments
        .iter()
        .any(|s| !s.matched && s.entries.is_empty() && s.reading.is_none()));
}

#[test]
fn pos_is_the_union_of_every_sense_in_first_seen_order() {
    let dir = scratch("assemble-pos");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "学校"); // see the fragmentation note above -- 東京 splits
    let pos = &r.segments[0].entries[0].pos;
    let mut seen: Vec<&String> = Vec::new();
    for code in pos {
        assert!(
            !seen.contains(&code),
            "{code} appears twice in {pos:?}, not just adjacently deduplicated"
        );
        seen.push(code);
    }
}

// The candidate at `start` whose span reaches the end of `text` -- what
// `chain.rs`'s tests need to pin `steps_for`'s contract. Takes `dir` (unlike
// the brief's two-argument sketch) so each test can build into its own
// scratch subdirectory, the same reason `cands` above does: `scratch` wipes
// its directory on every call, and cargo runs tests in parallel.
fn candidate_for(dir: &Path, text: &str, start: usize) -> jparser::engine::lex::Candidate {
    let total = text.chars().count();
    cands(dir, text)[start]
        .iter()
        .find(|c| c.start + c.len == total)
        .unwrap_or_else(|| panic!("no candidate at {start} spanning to the end of {text:?}"))
        .clone()
}

#[test]
fn steps_are_innermost_first() {
    // 食べさせられませんでした: the verb's own inflection first, then each
    // auxiliary outward. Phase 1b renders left to right from this order.
    let dir = scratch("chain-innermost");
    build_v2(&dir);
    let cand = candidate_for(&dir, "食べさせられませんでした", 0);
    let steps = jparser::engine::chain::steps_for(&cand);
    assert!(steps.len() >= 5);
    assert!(steps[0].range.start <= steps[1].range.start);
    assert_eq!(steps[0].range.start, 2, "食べ is the stem");
    assert_eq!(steps.last().unwrap().range.end, 12);
}

#[test]
fn the_ranges_tile_the_conjugated_tail_without_gaps() {
    let dir = scratch("chain-tiling");
    build_v2(&dir);
    let cand = candidate_for(&dir, "食べさせられませんでした", 0);
    let steps = jparser::engine::chain::steps_for(&cand);
    for pair in steps.windows(2) {
        assert_eq!(pair[0].range.end, pair[1].range.start, "no gap, no overlap");
    }
}

#[test]
fn an_uninflected_match_has_no_steps() {
    let dir = scratch("chain-uninflected");
    build_v2(&dir);
    let cand = candidate_for(&dir, "東京", 0);
    assert!(jparser::engine::chain::steps_for(&cand).is_empty());
}

#[test]
fn the_ranges_are_absolute_not_relative_to_the_spans_start() {
    // The brief's original name for this test ("...offset by the span's
    // start") assumed `steps_for` shifts each range by `cand.start`. It
    // doesn't -- see `chain.rs`'s contract doc. 食べ sits at absolute
    // positions 2..4 regardless of where the candidate's own span begins,
    // so た's step starts at 4, not at 4 + 2.
    let dir = scratch("chain-absolute");
    build_v2(&dir);
    let cand = candidate_for(&dir, "私は食べた", 2);
    let steps = jparser::engine::chain::steps_for(&cand);
    assert_eq!(steps[0].range.start, 4, "食べ starts at 2, た at 4");
}

#[test]
fn katakana_conjugation_is_not_deinflected_yet() {
    // Deliberate gap, not a bug, kept for Phase 1a: `Table`'s conjugation
    // trie is hiragana-keyed with no folding in `Table::search`, so a fully
    // katakana surface can never reach a Terminal/Continuation node through
    // an actual suffix match. Folding the trie too would fix that, but it
    // would also turn every katakana loanword ending in タ (データ, メーター,
    // カルタ) into a spurious past-tense candidate -- exactly the
    // kana-unified false-match class handoff §9.5 already measured as this
    // engine's worst defect. Task 16's per-tier measurement is what would
    // reopen this trade-off.
    //
    // `chains_ending_at` separately always offers a zero-consumption
    // substitution for any class whose own form has an empty suffix (an
    // ichidan-family bare 連用形 stem, e.g.) -- that fires for every input
    // regardless of script, since it never compares a character at all, so
    // it is unrelated to this gap. The check below is scoped to steps that
    // actually consumed a character, which is what "deinflected" means here.
    let dir = scratch("lex-katakana-gap");
    build_v2(&dir);
    let c = cands(&dir, "ハイタ");
    assert!(
        c.iter()
            .flatten()
            .all(|x| x.steps.iter().all(|s| s.range.is_empty())),
        "no candidate should explain any of ハイタ's characters through a real conjugation ending"
    );
}

// Not 東京 (the brief's own example): 東京 shatters into 東 + 京 in this
// fixture (see `a_compound_of_two_common_kanji_should_not_shatter`'s ignored
// test), so `segments[0]` for it is 東 alone, not 東京 -- inspecting its
// entries would not exercise what this test means to. る is used instead:
// ent_seq 2249020 (kanji 流, reading る -- a PRONOUNCE candidate) and ent_seq
// 2849996 (kana-only suffix る -- a PRIMARY candidate) collide on this exact
// one-character span, neither carries a priority tag, and both are reached
// by a direct, unfolded lookup, so `order`'s COMMON tier never interferes
// with the PRIMARY/PRONOUNCE clause this test is about.
//
// This is an end-to-end check against a real fixture; what actually proves
// the PRIMARY-before-PRONOUNCE clause in isolation (a hand-built trap where
// a lower-priority signal favors the wrong candidate) is
// `assemble::tests::order_puts_primary_before_pronounce_regardless_of_id`.
#[test]
fn an_exact_match_outranks_an_inexact_one() {
    let dir = scratch("order-exact");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "る");
    let e = &r.segments[0].entries;
    let has_primary = e.iter().any(|x| x.flags.contains(WordFlags::PRIMARY));
    let has_pronounce = e.iter().any(|x| x.flags.contains(WordFlags::PRONOUNCE));
    assert!(
        has_primary && has_pronounce,
        "fixture must offer both kinds or this test proves nothing"
    );
    let first_inexact = e.iter().position(|x| x.flags.contains(WordFlags::PRONOUNCE));
    let last_exact = e.iter().rposition(|x| x.flags.contains(WordFlags::PRIMARY));
    assert!(
        last_exact.expect("checked above") < first_inexact.expect("checked above"),
        "every PRIMARY entry precedes every PRONOUNCE one"
    );
}

#[test]
fn the_order_is_stable_and_total() {
    let dir = scratch("order-stable");
    build_v2(&dir);
    let a = parse_v2_over(&dir, "東京都に住む");
    let b = parse_v2_over(&dir, "東京都に住む");
    assert_eq!(a, b, "two identical parses order identically");
}

// Not は (the brief's own example): this fixture carries exactly one
// PARTICLE-flagged entry for は (ent_seq 2028920), so filtering on
// `WordFlags::PARTICLE` there leaves a single-element list and the sort
// assertion below would pass vacuously. で offers four -- ent_seq 2028980
// (COMMON), 2654270 and 2844355 (neither COMMON, both an uninflected
// zero-step match on the exact surface), and 2854413 (inexact -- its
// stored surface is デ, reached only through kana folding). 2654270 and
// 2844355 tie on every earlier clause (exact, COMMON, PRIMARY, chain
// length), so ascending `entry_id` is the only thing deciding 2654270
// before 2844355 between them.
//
// This fixture's per-key candidate order already happens to run ascending
// by `entry_id` before `order` touches it (JMdict's source document is
// itself `ent_seq`-ordered), so this test passes whether or not the
// tiebreak clause runs at all -- it is an end-to-end sanity check, not
// proof. What actually proves the clause is
// `assemble::tests::order_breaks_ties_on_entry_id_ascending`, a unit test
// against `order` directly with hand-built candidates handed in descending
// id order (and identical on every other field), which can only pass if
// the ascending-`entry_id` tiebreak is really being applied.
#[test]
fn ties_break_on_entry_id_ascending() {
    let dir = scratch("order-ties");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "で");
    let ids: Vec<u32> = r.segments[0]
        .entries
        .iter()
        .filter(|e| e.flags.contains(WordFlags::PARTICLE))
        .map(|e| e.entry_id)
        .collect();
    assert!(
        ids.len() >= 2,
        "fixture must offer several particle entries or this test proves nothing"
    );
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "particles with equal evidence sort by ent_seq");
}

// で is where the duplication was actually observed while building the
// ordering tests above: `lex::candidates` independently reconstructed
// ent_seq 1338240/2831062/2871509 (all headword でる) twice each at start 0,
// through two different empty-consumption chain hits landing on the same
// key. Without `lex::dedup_shallowest`, `Segment::entries` for で carries
// each of those three ent_seq values twice -- the same dictionary entry
// showing up twice in what a caller reads as the candidate list.
#[test]
fn no_two_entries_in_a_segment_share_an_entry_id() {
    let dir = scratch("lex-no-duplicate-entries");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "で");
    let ids: Vec<u32> = r.segments[0].entries.iter().map(|e| e.entry_id).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        ids.len(),
        unique.len(),
        "each entry_id must appear at most once in a segment's entries, got {ids:?}"
    );
}

// The dedup above keys on `(len, entry_id)`, not `(start, entry_id)` alone,
// precisely so this still holds: 書いた's onbin fragment 書い (len 2) and
// the full chain 書いた (len 3) both reconstruct headword 書く (the same
// entry_id) from the same start, and both must survive as distinct
// candidates -- collapsing the key down to `(start, entry_id)` would drop
// one of them silently.
#[test]
fn the_same_headword_at_two_different_lengths_is_not_deduplicated_away() {
    let dir = scratch("lex-keeps-both-lengths");
    build_v2(&dir);
    let c = cands(&dir, "書いた");
    let lens: Vec<usize> = c[0]
        .iter()
        .filter(|x| x.headword == "書く")
        .map(|x| x.len)
        .collect();
    assert!(
        lens.contains(&3) && lens.contains(&2),
        "both 書いた's full chain (len 3) and its 書い fragment (len 2) must \
         survive the dedup, got lens {lens:?}"
    );
}

// v2's own snapshot over the curated 30-sentence corpus, reviewed on its own
// terms (master plan §5.1: v1 is a disagreement source, not an oracle, so
// this is a fresh snapshot rather than a copy of
// `parse_snapshots__sentences.snap`). The rendering below mirrors
// `parse_snapshots.rs`'s `render` so a human can diff the two by eye; it is
// a separate copy rather than a shared one because the two integration test
// binaries cannot share non-`pub` code, and making this rendering `pub` in
// the library for fifteen lines would leak a display concern into the API --
// the same trade-off `parse_snapshots.rs` documents for its own copy.

/// Alternatives printed per span. See `parse_snapshots.rs`'s identical constant.
const MAX_ALTERNATIVES: usize = 5;

/// Printed wherever a reading or conjugation is `None`.
const NONE_LABEL: &str = "-";

fn sentences() -> impl Iterator<Item = &'static str> {
    SENTENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn render_v2(text: &str, result: &jparser::ParseResult) -> String {
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
fn v2_snapshots_every_sentence() {
    let dir = scratch("v2-snapshot");
    build_v2(&dir);
    let mut out = String::new();
    for sentence in sentences() {
        out.push_str(&render_v2(sentence, &parse_v2_over(&dir, sentence)));
        out.push('\n');
    }
    insta::assert_snapshot!("sentences", out);
}
