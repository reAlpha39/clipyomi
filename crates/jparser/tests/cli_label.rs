// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! `jparser-cli label` end to end, driven by piped stdin.
//!
//! Line-buffered input is what makes this possible at all: a raw-keystroke
//! reader would need a TTY and none of these branches could be covered.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// One diff record over 言った, whose jparser and Vibrato boundaries are
/// supplied literally — `label` never tokenizes, so this needs no dictionary.
fn diff_file(dir: &Path, text: &str, jparser: &str, vibrato: &str) -> PathBuf {
    let path = dir.join("diff.jsonl");
    std::fs::write(
        &path,
        format!(
            "{{\"text\":\"{text}\",\"source\":\"test:1\",\"jparser\":{jparser},\
             \"vibrato\":{vibrato},\"conflicts\":[1],\"unknown_tokens\":0}}\n"
        ),
    )
    .unwrap();
    path
}

/// Drive `label` with `script` on stdin and return the gold file's contents.
fn label(index: &Path, diff: &Path, gold: &Path, script: &str) -> String {
    let mut child = Command::new(BIN)
        .arg("label")
        .arg(index)
        .arg(diff)
        .arg(gold)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("jparser-cli must be runnable");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write the decision script");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(gold).unwrap_or_default()
}

/// Bless jparser's spans, then pick 言う. The candidate list is sorted by
/// ent_seq, and 1000010 is the only candidate on the whole-text span, so `1`
/// is unambiguous.
#[test]
fn blessing_jparser_spans_and_picking_an_entry_writes_one_gold_record() {
    let dir = scratch("cli-label-bless-jparser");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\n1\n");

    assert_eq!(
        written.trim(),
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}"
    );
}

/// `n` is a positive label, not a skip: the span is recorded with a null
/// ent_seq and counts for span accuracy.
#[test]
fn n_records_a_null_ent_seq() {
    let dir = scratch("cli-label-null");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\nn\n");

    assert!(written.contains("\"ent_seq\":null"), "got {written}");
}

/// `s` appends nothing. A sentence the adjudicator cannot rule on stays
/// unlabelled rather than becoming a guess in the gold file.
#[test]
fn s_writes_no_record_at_all() {
    let dir = scratch("cli-label-skip");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    assert_eq!(label(&index, &diff, &gold, "s\n").trim(), "");
}

/// `v` blesses the tokenizer's boundaries instead. Vibrato cut 言|った here, so
/// two spans get labelled; neither is a JMdict entry on its own, so both are
/// null.
#[test]
fn blessing_vibrato_spans_labels_every_token() {
    let dir = scratch("cli-label-bless-vibrato");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "v\nn\nn\n");

    assert!(written.contains("\"start\":0,\"len\":1"), "got {written}");
    assert!(written.contains("\"start\":1,\"len\":2"), "got {written}");
}

/// `e` retypes the interior cuts. 0 and 3 are implied, so `1` means 言|った.
#[test]
fn e_takes_a_retyped_cut_list() {
    let dir = scratch("cli-label-edit");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "e\n1\nn\nn\n");

    assert!(written.contains("\"start\":0,\"len\":1"), "got {written}");
    assert!(written.contains("\"start\":1,\"len\":2"), "got {written}");
}

/// A span with no candidate is still labellable: `#` takes an ent_seq
/// directly, so the adjudicator never has to skip a sentence they understand.
#[test]
fn a_hash_prefixed_ent_seq_is_accepted_directly() {
    let dir = scratch("cli-label-manual-id");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\n#1000030\n");

    assert!(written.contains("\"ent_seq\":1000030"), "got {written}");
}
