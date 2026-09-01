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

    jparser::index::load::Index::open(&v2_dir)
        .expect_err("v1's fingerprint must not open a v2-built index");
    jparser::index::load::Index::open_with(&v1_dir, jparser::engine::build::fingerprint())
        .expect_err("v2's fingerprint must not open a v1-built index");
}
