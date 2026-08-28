// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! `Entry::entry_id` is the JMdict `ent_seq`, and Phase 0's entry-top-1 metric
//! is unmeasurable without it. `jmdict.rs` reads `ent_seq` straight off the
//! element into `RawEntry::id`, so this asserts the whole chain from XML to
//! public API rather than just that a field exists.

use std::path::PathBuf;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::ParseOptions;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

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

/// 言う is `<ent_seq>1000010</ent_seq>` in the fixture, and 言った matches it
/// through the v5u mis-annotation fallback — the same case `cli_parse.rs`
/// pins the rendered output of.
#[test]
fn the_top_entry_carries_the_ent_seq_the_fixture_declares() {
    let dir = index_dir("entry-id-verb");
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(&dir).unwrap();

    let result = jparser::parse(&index, &table, "言った", &ParseOptions::default(), None)
        .expect("parse must succeed");

    let seg = result.segments.first().expect("one segment");
    assert_eq!((seg.start, seg.len), (0, 3), "one span over the whole text");
    let entry = seg.entries.first().expect("a matched entry");
    assert_eq!(entry.entry_id, 1000010, "言う's ent_seq");
}

/// A particle, so the id survives a non-verb path too: は is
/// `<ent_seq>1000020</ent_seq>` and reaches `assemble_entry` with an empty
/// conjugation chain.
#[test]
fn a_non_verb_entry_carries_its_ent_seq_too() {
    let dir = index_dir("entry-id-particle");
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(&dir).unwrap();

    let result = jparser::parse(&index, &table, "は", &ParseOptions::default(), None)
        .expect("parse must succeed");

    let entry = result.segments[0].entries.first().expect("a matched entry");
    assert_eq!(entry.entry_id, 1000020, "は's ent_seq");
}
