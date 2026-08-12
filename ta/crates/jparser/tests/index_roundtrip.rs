// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

use std::path::{Path, PathBuf};

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::index::{BuildReport, INDEX_FORMAT_VERSION};
use jparser::record::WordFlags;
use jparser::stem::StemOptions;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &Path) -> BuildReport {
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        dir,
    )
    .expect("build must succeed")
}

#[test]
fn builds_and_reports_counts() {
    let report = build(&tmpdir("counts"));
    assert_eq!(report.entries, 3);
    assert_eq!(report.skipped_entries, 0);
    assert!(report.keys > 0);
    assert!(report.records >= report.keys);
    assert!(report.stems.exact_stems > 0);
}

#[test]
fn writes_all_expected_files() {
    let dir = tmpdir("files");
    build(&dir);
    for name in [
        "header.bin",
        "keys.fst",
        "records.bin",
        "entries.bin",
        "entries.idx",
    ] {
        assert!(dir.join(name).exists(), "{name} must be written");
    }
}

#[test]
fn round_trips_an_exact_headword() {
    let dir = tmpdir("exact");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let found = index
        .prefixes_of("言う")
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .any(|r| r.surface == "言う");
    assert!(found, "the full headword must be retrievable");
}

#[test]
fn prefix_walk_returns_distinct_ascending_lengths() {
    let dir = tmpdir("prefixes");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    // 言う is a headword and 言 is its generated stem, so walking "言うから"
    // must surface hits at more than one length.
    let hits = index.prefixes_of("言うから").unwrap();
    let lengths: Vec<usize> = hits.iter().map(|h| h.key_chars).collect();
    assert!(lengths.len() >= 2, "got lengths {lengths:?}");
    assert!(lengths.windows(2).all(|w| w[0] < w[1]), "got {lengths:?}");
}

#[test]
fn matches_katakana_text_against_a_hiragana_headword() {
    let dir = tmpdir("kana");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    // The key is normalized, so katakana input finds the hiragana headword,
    // while the stored surface stays hiragana so inexactness stays detectable.
    let surfaces: Vec<String> = index
        .prefixes_of("イウ")
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .map(|r| r.surface.clone())
        .collect();
    assert!(surfaces.iter().any(|s| s == "いう"), "got {surfaces:?}");
}

#[test]
fn returns_no_hits_for_text_with_no_dictionary_prefix() {
    // This holds only because jmdict_mini.xml's build has empty_stems: 0 (no
    // irregular verb like する/vs-i or 来る/vk in the fixture). "" is a
    // prefix of every string, so an index that DID contain an empty-key
    // record would report a key_chars: 0 hit here too, for "zzz" same as
    // anything else. Do not change jmdict_mini.xml on the strength of this
    // test alone: see prefix_walk_returns_the_empty_stem_hit_for_an_irregular_verb
    // below for that case, exercised against its own dedicated fixture.
    let dir = tmpdir("miss");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.prefixes_of("zzz").unwrap().is_empty());
}

#[test]
fn prefix_walk_returns_the_empty_stem_hit_for_an_irregular_verb() {
    // する is tagged vs-i, one of ta-old's irregular verbs: vs-i's
    // remove-tense/form-0 conjugation strips suffix "する" from surface
    // "する" itself, leaving the empty stem. "" is a prefix of every
    // string, so prefixes_of must surface it as a key_chars: 0 hit — this
    // is exactly the failure mode from the bug report, where looking up
    // "します" found nothing because the empty key was unreachable.
    let dir = tmpdir("empty-stem");
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY vs-i "noun or participle which takes the aux. verb suru">
]>
<JMdict>
<entry>
<ent_seq>3000001</ent_seq>
<r_ele><reb>する</reb></r_ele>
<sense><pos>&vs-i;</pos><gloss>to do</gloss></sense>
</entry>
</JMdict>"#;
    let table = ConjugationTable::load_embedded().unwrap();
    let report = build_from_reader(
        std::io::Cursor::new(xml),
        &table,
        &StemOptions::default(),
        &dir,
    )
    .expect("build must succeed");
    assert!(
        report.stems.empty_stems > 0,
        "fixture must produce an empty stem, got {:?}",
        report.stems
    );

    let index = Index::open(&dir).unwrap();
    let hits = index.prefixes_of("します").unwrap();
    let lengths: Vec<usize> = hits.iter().map(|h| h.key_chars).collect();
    assert!(
        lengths.windows(2).all(|w| w[0] < w[1]),
        "ascending order must hold even with a 0-length hit first: got {lengths:?}"
    );
    let empty_hit = hits
        .iter()
        .find(|h| h.key_chars == 0)
        .expect("the empty stem must be reachable through prefixes_of");
    assert!(
        empty_hit
            .records
            .iter()
            .any(|r| r.surface.is_empty() && r.entry_id == 3_000_001),
        "got {:?}",
        empty_hit.records
    );
}

#[test]
fn preserves_flags_through_the_round_trip() {
    let dir = tmpdir("flags");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let hits = index.prefixes_of("は").unwrap();
    let particle = hits
        .iter()
        .flat_map(|h| &h.records)
        .find(|r| r.surface == "は")
        .expect("は must be indexed");
    assert!(WordFlags(particle.flags).contains(WordFlags::PARTICLE));
}

#[test]
fn retrieves_entry_data_by_id() {
    let dir = tmpdir("entry");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let entry = index.entry(1000010).unwrap().expect("entry must exist");
    assert_eq!(entry.senses[0].glosses, vec!["to say", "to utter"]);
    assert_eq!(entry.senses[0].pos, vec!["v5r"]);
}

#[test]
fn returns_none_for_an_unknown_entry_id() {
    let dir = tmpdir("noentry");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.entry(999_999).unwrap().is_none());
}

#[test]
fn records_header_counts() {
    let dir = tmpdir("header");
    let report = build(&dir);
    let index = Index::open(&dir).unwrap();
    assert_eq!(index.header().version, INDEX_FORMAT_VERSION);
    assert_eq!(index.header().entries as usize, report.entries);
    assert_eq!(index.header().keys as usize, report.keys);
}

#[test]
fn rejects_a_header_with_the_wrong_format_version() {
    let dir = tmpdir("version");
    build(&dir);
    // Corrupt the version so load must refuse rather than misread the files.
    let path = dir.join("header.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[0] = bytes[0].wrapping_add(1);
    std::fs::write(&path, bytes).unwrap();
    let msg = Index::open(&dir).unwrap_err().to_string();
    assert!(msg.contains("version"), "got {msg}");
}

#[test]
fn rejects_a_header_with_the_wrong_conjugation_fingerprint() {
    let dir = tmpdir("fingerprint");
    build(&dir);
    // conjugation_fingerprint is the header's last field (u32, u32, u32, u32,
    // then u64), so flipping the encoded blob's last byte corrupts only it
    // and leaves version and the counts intact. Load must refuse rather than
    // silently resolve verb_type ids against a different conjugation asset.
    let path = dir.join("header.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] = bytes[last].wrapping_add(1);
    std::fs::write(&path, bytes).unwrap();
    let msg = Index::open(&dir).unwrap_err().to_string();
    assert!(msg.contains("fingerprint"), "got {msg}");
}

#[test]
fn stems_carry_the_producing_verb_type_and_headwords_carry_none() {
    let dir = tmpdir("verb-type");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    // v5u is not one of the four duplicate-named types, so it has exactly
    // one id in the table.
    let v5u_id = table.types_named("v5u")[0];

    let records: Vec<_> = index
        .prefixes_of("言う")
        .unwrap()
        .iter()
        .flat_map(|h| h.records.clone())
        .collect();

    // 言 is the v5-fallback stem generated from 言う via sibling type v5u
    // (see the module docs on the mis-annotation fallback), so its stored
    // verb_type must survive the build -> mmap -> bincode round trip.
    let stem = records
        .iter()
        .find(|r| r.surface == "言")
        .expect("言 stem must be indexed");
    assert_eq!(stem.verb_type, Some(v5u_id));

    // The plain headword carries no verb type of its own; Task 9 uses this
    // distinction to tell a stem from a headword.
    let headword = records
        .iter()
        .find(|r| r.surface == "言う")
        .expect("言う headword must be indexed");
    assert_eq!(headword.verb_type, None);
}

#[test]
fn a_shared_normalized_key_collects_records_from_distinct_entries() {
    let dir = tmpdir("multi-record");
    // とる (hiragana) and トル (katakana) normalize to the same FST key via
    // kana::unify_str, so these two otherwise-unrelated entries must land in
    // the same bucket. tests/fixtures/jmdict_mini.xml is untouched; every
    // fixture key maps to exactly one record and can't exercise this.
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry>
<ent_seq>2000001</ent_seq>
<r_ele><reb>とる</reb></r_ele>
<sense><pos>&n;</pos><gloss>to take</gloss></sense>
</entry>
<entry>
<ent_seq>2000002</ent_seq>
<r_ele><reb>トル</reb></r_ele>
<sense><pos>&n;</pos><gloss>a different noun</gloss></sense>
</entry>
</JMdict>"#;
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(xml),
        &table,
        &StemOptions::default(),
        &dir,
    )
    .expect("build must succeed");
    let index = Index::open(&dir).unwrap();

    let records: Vec<_> = index
        .prefixes_of("とる")
        .unwrap()
        .iter()
        .flat_map(|h| h.records.clone())
        .collect();

    assert!(
        records
            .iter()
            .any(|r| r.surface == "とる" && r.entry_id == 2000001),
        "got {records:?}"
    );
    assert!(
        records
            .iter()
            .any(|r| r.surface == "トル" && r.entry_id == 2000002),
        "got {records:?}"
    );
}
