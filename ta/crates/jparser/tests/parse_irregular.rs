// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! End-to-end `parse` regression for 来る, the one irregular verb whose kanji
//! reading changes with the conjugation.
//!
//! Two Phase 1 mechanisms meet here and both are easy to break silently:
//! the whole surface of 来る is its own remove-suffix, so its generated stem
//! is the empty string and the only way to reach it is the empty FST key
//! (Phase 1A); and the kanji and kana spellings live in two separate,
//! identically named `vk` blocks, so the reading can only be rebuilt through
//! the kuruHack twin search (contract §6.6).

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::{parse, ParseOptions};

const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY vk "Kuru verb - special class">
]>
<JMdict>
<entry>
<ent_seq>1000040</ent_seq>
<k_ele><keb>来る</keb></k_ele>
<r_ele><reb>くる</reb></r_ele>
<sense><pos>&vk;</pos><gloss>to come</gloss></sense>
</entry>
</JMdict>
"#;

fn open_index(name: &str) -> Index {
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
    .expect("build must succeed");
    Index::open(&dir).expect("open must succeed")
}

#[test]
fn parses_the_dictionary_form_of_an_irregular_verb() {
    let index = open_index("parse-kuru-plain");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来る", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 1);
    let s = &out.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来る"));
    assert!(s.matched);
    assert_eq!(s.reading.as_deref(), Some("くる"));
    // Two matches align to (0, 2): the plain headword 来る and the empty stem
    // conjugated by vk's Non-past. sort_matches' verb-plain collapse
    // (contract §6.5 pass B step 3) drops the second, so exactly one entry
    // survives and it is the unconjugated one.
    assert_eq!(s.entries.len(), 1);
    assert_eq!(s.entries[0].headword, "来る");
    assert_eq!(s.entries[0].conjugation, None);
}

#[test]
fn reconstructs_the_reading_of_a_conjugated_irregular_verb() {
    let index = open_index("parse-kuru-past");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来た", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 1);
    let s = &out.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来た"));
    assert!(s.matched);
    // 来 alone is not a key; the match can only be found by walking the empty
    // key to the "" stem and then matching vk's Stem suffix 来た into
    // v-ta-stem's empty Past. Its reading is only recoverable through the
    // kana vk twin's きた → the substitution き, plus the verbatim tail た.
    assert_eq!(s.reading.as_deref(), Some("きた"));
    assert_eq!(s.entries.len(), 1);
    let e = &s.entries[0];
    assert_eq!(e.headword, "来る");
    assert_eq!(e.reading.as_deref(), Some("きた"));
    assert_eq!(e.conjugation.as_deref(), Some("Past"));
    assert_eq!(e.pos, vec!["vk"]);
    assert_eq!(e.senses[0].glosses, vec!["to come"]);
}

#[test]
fn leaves_an_unmatched_run_with_no_reading_and_no_entries() {
    // 。 is in no dictionary here. ta-old emitted nothing at all for skipped
    // characters; the port emits a matched:false Segment so a caller can
    // rebuild the input verbatim. There is no morphological-analyzer
    // fallback: an unmatched run simply has no reading.
    let index = open_index("parse-unmatched");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "来た。", &ParseOptions::default(), None).unwrap();

    assert_eq!(out.segments.len(), 2);
    assert!(out.segments[0].matched);
    let tail = &out.segments[1];
    // Char offset 2, not byte offset 6: 来た。 is 3 chars and 9 bytes.
    assert_eq!((tail.start, tail.len, tail.surface.as_str()), (2, 1, "。"));
    assert!(!tail.matched);
    assert_eq!(tail.reading, None);
    assert!(tail.entries.is_empty());
}

#[test]
fn parses_empty_text_into_no_segments() {
    let index = open_index("parse-empty");
    let table = ConjugationTable::load_embedded().unwrap();
    let out = parse(&index, &table, "", &ParseOptions::default(), None).unwrap();
    assert!(out.segments.is_empty());
}
