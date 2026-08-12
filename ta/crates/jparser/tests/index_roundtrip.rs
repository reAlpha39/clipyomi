use std::path::PathBuf;

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

fn build(dir: &PathBuf) -> BuildReport {
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
    let dir = tmpdir("miss");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.prefixes_of("zzz").unwrap().is_empty());
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
