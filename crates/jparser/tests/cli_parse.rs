// SPDX-License-Identifier: MIT OR Apache-2.0

//! `jparser-cli parse` end to end, against the hand-written mini fixture.
//!
//! Exact-output assertions, not substring checks: the format is a frozen
//! interface (Task 9 Step 10 diffs two runs of it against each other), so a
//! change to it must break a test rather than silently change what a
//! downstream comparison compares.

use std::path::{Path, PathBuf};
use std::process::Command;

use jparser::engine::build::build_from_reader;

/// Cargo sets this for integration tests of a package that declares the bin.
const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

/// Same temp-dir convention as `tests/index_roundtrip.rs`; the crate has no
/// `tempfile` dependency and is not gaining one.
fn index_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    build_from_reader(std::io::Cursor::new(FIXTURE), &dir).expect("the mini fixture must build");
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
    // 言う is `v5u`; 言っ is 五段・ワ行促音便's 連用タ接続 and た its past
    // auxiliary, so the label renders as "Past". The reading strips う from
    // いう and appends った; the glosses are the first sense of entry 1000010.
    let dir = index_dir("cli-parse-verb");
    assert_eq!(
        parse(&dir, "言った"),
        "start=0 len=3 言った matched reading=いった\n\
         \x20   言う (Past) [いった] to say; to utter\n"
    );
}

#[test]
fn prints_an_unconjugated_particle_and_coalesces_the_skipped_tail() {
    // は is kana-only, so `EntryData::readings` is empty and the reading is
    // "-", not "は". "zz" is two unmatched characters, coalesced into one
    // span.
    let dir = index_dir("cli-parse-particle");
    assert_eq!(
        parse(&dir, "はzz"),
        "start=0 len=1 は matched reading=-\n\
         \x20   は (-) [-] topic marker\n\
         start=1 len=2 zz unmatched\n"
    );
}
