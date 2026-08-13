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
