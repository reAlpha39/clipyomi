// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The deliverable: `resolve` feeding Phase 2A's `ensure_dictionary`.
//!
//! The only place both crates appear together, and the only test that proves
//! they compose. `jparser` is a dev-dependency here, not a dependency — the
//! direction matters, because `jmdict-source` must stay usable without it.

use std::io::Write;
use std::path::PathBuf;

use jmdict_source::SOURCE_FILE;
use jparser::conjugation::ConjugationTable;
use jparser::stem::StemOptions;

const XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn a_gzipped_archive_builds_an_index_through_ensure_dictionary() {
    let dir = scratch("seam");
    let source_dir = dir.join("source");
    let root = dir.join("dictionary");
    std::fs::create_dir_all(&source_dir).expect("mkdir");

    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(XML.as_bytes()).expect("gz write");
    std::fs::write(source_dir.join(SOURCE_FILE), e.finish().expect("gz")).expect("write");

    let table = ConjugationTable::load_embedded().expect("table");
    let opts = StemOptions::default();

    let index = jparser::index::ensure_dictionary(&root, &table, &opts, 2, || {
        jmdict_source::resolve(&source_dir)
    })
    .expect("ensure_dictionary");

    assert_eq!(
        index.entry(1000010).expect("entry").expect("present").id,
        1000010
    );
    assert!(root.join("gen-1").exists());
    // The source directory is a sibling of the generation root, so 2A's
    // sweep-and-list machinery never sees it, and the archive is not consumed.
    assert!(source_dir.join(SOURCE_FILE).exists(), "the source vanished");
}
