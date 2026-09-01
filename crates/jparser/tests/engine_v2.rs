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
