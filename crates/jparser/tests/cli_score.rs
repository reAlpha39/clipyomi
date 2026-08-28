// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! `jparser-cli score` end to end against the hand-written mini fixture.
//!
//! Exact-output assertions: the format is a frozen interface — the recorded
//! Phase 0 baseline is transcribed from it — so a change must break a test
//! rather than silently change what a later phase is compared against.

use std::path::{Path, PathBuf};
use std::process::Command;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::stem::StemOptions;

const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");
const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn index_dir(dir: &Path) -> PathBuf {
    let index = dir.join("index");
    std::fs::create_dir_all(&index).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &index,
    )
    .expect("the mini fixture must build");
    index
}

fn score(index: &Path, gold: &Path) -> String {
    let out = Command::new(BIN)
        .arg("score")
        .arg(index)
        .arg(gold)
        .output()
        .expect("jparser-cli must be runnable");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("output must be UTF-8")
}

/// 言う is ent_seq 1000010 and は is 1000020 in the mini fixture, and
/// `cli_parse.rs` already pins that 言った parses as one span over all three
/// chars. Both metrics are therefore 1.0 by construction, which is what makes
/// this a test of the harness rather than of the engine.
#[test]
fn a_fully_correct_gold_corpus_scores_one_on_both_metrics() {
    let dir = scratch("cli-score-perfect");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}\n\
         {\"text\":\"は\",\"source\":\"test:2\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":1000020}]}\n",
    )
    .unwrap();

    assert_eq!(
        score(&index, &gold),
        "sentences:      2\n\
         gold spans:     2\n\
         superseded:     0\n\
         span accuracy:  1.0000  (2 / 2)\n\
         entry top-1:    1.0000  (2 / 2)\n"
    );
}

/// A wrong ent_seq must cost entry top-1 and leave span accuracy alone. If a
/// future change conflated the two metrics, only this test would catch it.
#[test]
fn a_wrong_ent_seq_costs_only_entry_top_1() {
    let dir = scratch("cli-score-wrong-entry");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":9999999}]}\n",
    )
    .unwrap();

    assert_eq!(
        score(&index, &gold),
        "sentences:      1\n\
         gold spans:     1\n\
         superseded:     0\n\
         span accuracy:  1.0000  (1 / 1)\n\
         entry top-1:    0.0000  (0 / 1)\n"
    );
}

/// A cover violation is a hard error naming the record, not a skipped
/// sentence: skipping would move span accuracy's denominator silently.
#[test]
fn a_gold_span_past_the_end_of_its_text_is_a_hard_error() {
    let dir = scratch("cli-score-bad-cover");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":4,\"ent_seq\":null}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .output()
        .expect("runnable");

    assert!(!out.status.success(), "must exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("は"), "names the record: {stderr}");
}

#[test]
fn the_json_form_carries_the_same_numbers() {
    let dir = scratch("cli-score-json");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":1000020}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .arg("--json")
        .output()
        .expect("runnable");
    assert!(out.status.success());

    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "{\"sentences\":1,\"gold_spans\":1,\"superseded\":0,\"span_hits\":1,\"top1_hits\":1,\"top1_total\":1}"
    );
}
