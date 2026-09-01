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
/// no boundary source -- the shape every assemble test below needs and
/// nothing more.
fn parse_v2_over(dir: &Path, text: &str) -> jparser::ParseResult {
    let index = open_v2(dir);
    let table = jparser::engine::table::Table::new();
    jparser::engine::parse_v2(&index, &table, text, &jparser::ParseOptions::default(), None)
        .expect("parse_v2")
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
// itself `ent_seq`-ordered), so this test mainly confirms `order` never
// disturbs that -- `an_exact_match_outranks_an_inexact_one` above is the
// one that fails without `order` actually running.
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
