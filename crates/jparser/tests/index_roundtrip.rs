// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use jparser::engine::build::build_from_reader;
use jparser::engine::kana::fold;
use jparser::index::load::Index;
use jparser::index::{BuildReport, INDEX_FORMAT_VERSION};
use jparser::WordFlags;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &Path) -> BuildReport {
    build_from_reader(std::io::Cursor::new(FIXTURE), dir).expect("build must succeed")
}

#[test]
fn builds_and_reports_counts() {
    let report = build(&tmpdir("counts"));
    assert_eq!(report.entries, 3);
    assert_eq!(report.skipped_entries, 0);
    assert!(report.keys > 0);
    assert!(report.records >= report.keys);
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
        .prefixes_of_with("言う", fold)
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .any(|r| r.surface == "言う");
    assert!(found, "the full headword must be retrievable");
}

#[test]
fn prefix_walk_returns_distinct_ascending_lengths() {
    // Two headwords, one a prefix of the other, so walking "言葉狩り" must
    // surface hits at more than one length. `jmdict_mini.xml` cannot serve
    // this on its own: no key in it is a prefix of another, because the
    // builder stores headwords and nothing else.
    let dir = tmpdir("prefixes");
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry>
<ent_seq>2000010</ent_seq>
<k_ele><keb>言</keb></k_ele>
<r_ele><reb>げん</reb></r_ele>
<sense><pos>&n;</pos><gloss>word</gloss></sense>
</entry>
<entry>
<ent_seq>2000020</ent_seq>
<k_ele><keb>言葉</keb></k_ele>
<r_ele><reb>ことば</reb></r_ele>
<sense><pos>&n;</pos><gloss>language</gloss></sense>
</entry>
</JMdict>"#;
    build_from_reader(std::io::Cursor::new(xml), &dir).expect("build must succeed");
    let index = Index::open(&dir).unwrap();
    let hits = index.prefixes_of_with("言葉狩り", fold).unwrap();
    let lengths: Vec<usize> = hits.iter().map(|h| h.key_chars).collect();
    assert_eq!(lengths, vec![1, 2], "got lengths {lengths:?}");
}

#[test]
fn matches_katakana_text_against_a_hiragana_headword() {
    let dir = tmpdir("kana");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    // The key is normalized, so katakana input finds the hiragana headword,
    // while the stored surface stays hiragana so inexactness stays detectable.
    let surfaces: Vec<String> = index
        .prefixes_of_with("イウ", fold)
        .unwrap()
        .iter()
        .flat_map(|h| &h.records)
        .map(|r| r.surface.clone())
        .collect();
    assert!(surfaces.iter().any(|s| s == "いう"), "got {surfaces:?}");
}

#[test]
fn returns_no_hits_for_text_with_no_dictionary_prefix() {
    // The builder stores headwords only, and no JMdict entry has an empty
    // one, so no index it writes carries the empty key. "" is a prefix of
    // every string, so an index that DID carry it would report a
    // key_chars: 0 hit here too, for "zzz" same as anything else.
    let dir = tmpdir("miss");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.prefixes_of_with("zzz", fold).unwrap().is_empty());
}

#[test]
fn preserves_flags_through_the_round_trip() {
    let dir = tmpdir("flags");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let hits = index.prefixes_of_with("は", fold).unwrap();
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
    assert_eq!(entry.senses[0].pos, vec!["v5u"]);
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
    // silently resolve class ids against a different conjugation table.
    let path = dir.join("header.bin");
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] = bytes[last].wrapping_add(1);
    std::fs::write(&path, bytes).unwrap();
    let msg = Index::open(&dir).unwrap_err().to_string();
    assert!(msg.contains("fingerprint"), "got {msg}");
}

#[test]
fn a_shared_normalized_key_collects_records_from_distinct_entries() {
    let dir = tmpdir("multi-record");
    // とる (hiragana) and トル (katakana) fold to the same FST key via
    // engine::kana::fold_str, so these two otherwise-unrelated entries must
    // land in the same bucket. tests/fixtures/jmdict_mini.xml is untouched;
    // every fixture key maps to exactly one record and can't exercise this.
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
    build_from_reader(std::io::Cursor::new(xml), &dir).expect("build must succeed");
    let index = Index::open(&dir).unwrap();

    let records: Vec<_> = index
        .prefixes_of_with("とる", fold)
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
