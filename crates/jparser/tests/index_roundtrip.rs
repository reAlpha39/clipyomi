// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use jparser::engine::build::{build_from_reader, build_from_readers};
use jparser::engine::kana::fold;
use jparser::index::load::Index;
use jparser::index::{BuildReport, IndexError, INDEX_FORMAT_VERSION};
use jparser::WordFlags;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");
const KANJIDIC_FIXTURE: &str = include_str!("fixtures/kanjidic_mini.xml");

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn build(dir: &Path) -> BuildReport {
    build_from_reader(std::io::Cursor::new(FIXTURE), dir).expect("build must succeed")
}

/// The two-reader build: JMdict plus the four-character KANJIDIC2 fixture.
fn build_with_kanji(dir: &Path) -> BuildReport {
    build_from_readers(
        std::io::Cursor::new(FIXTURE),
        std::io::Cursor::new(KANJIDIC_FIXTURE),
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
        "kanji.bin",
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

/// Phase 2c: `Walk` is the walk `prefixes_of_with` performs, exposed so one
/// start position can be walked once instead of re-walked per span end.
#[test]
fn a_walk_finds_the_key_that_ends_where_it_stands() {
    let dir = tmpdir("walk-exact");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(
        walk.records().expect("records").is_none(),
        "no key ends at the root"
    );
    for ch in "言う".chars() {
        assert!(walk.push(ch, fold), "言う is in the fixture");
    }
    let records = walk.records().expect("records").expect("言う ends here");
    assert!(records.iter().any(|r| r.surface == "言う"));
}

#[test]
fn a_walk_that_leaves_the_index_is_spent() {
    let dir = tmpdir("walk-dead");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(!walk.push('z', fold), "no fixture key starts with z");
    // Spent, and it stays spent: a caller enumerating spans from one start
    // stops here rather than testing longer ones, which is the whole point.
    assert!(!walk.push('言', fold));
    assert!(walk.records().expect("records").is_none());
}

#[test]
fn a_walk_that_leaves_the_index_mid_character_is_spent() {
    let dir = tmpdir("walk-dead-mid-char");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    // 'z' fails on its first (and only) UTF-8 byte, so it never exercises a
    // `push` that has already advanced `node`/`output` partway through a
    // character before the walk dies. 'う' (E3 81 86) shares its first two
    // UTF-8 bytes with the fixture's only key starting there, 'は' (E3 81
    // AF), and diverges only on the third -- so this push advances the walk
    // two bytes into the transition before finding none for the third,
    // which is exactly the half-advanced case the `alive` latch exists to
    // make unobservable.
    let mut walk = index.walk();
    assert!(
        !walk.push('う', fold),
        "no fixture key is 'う' or starts with it"
    );
    assert!(walk.records().expect("records").is_none());
    // Spent, and it stays spent, same as the first-byte-failure case above.
    assert!(!walk.push('は', fold));
    assert!(walk.records().expect("records").is_none());
}

#[test]
fn records_after_reaches_a_key_the_walk_itself_has_not() {
    let dir = tmpdir("walk-after");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(walk.push('言', fold));
    // 言 alone is not the key; 言う is. `records_after` asks about the
    // extension without committing the walk to it -- which is how a
    // deinflected key (stem plus the class's remove-suffix) is tested.
    let records = walk
        .records_after("う", fold)
        .expect("records_after")
        .expect("言 + う is a key");
    assert!(records.iter().any(|r| r.surface == "言う"));
    assert!(
        walk.records_after("zz", fold)
            .expect("records_after")
            .is_none(),
        "an extension that leaves the index yields nothing"
    );
}

#[test]
fn records_after_does_not_move_the_walk_it_was_called_on() {
    let dir = tmpdir("walk-immutable");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(walk.push('言', fold));
    let _ = walk.records_after("う", fold).expect("records_after");
    // Still at 言, so pushing う still arrives at 言う.
    assert!(walk.push('う', fold));
    let records = walk.records().expect("records").expect("言う ends here");
    assert!(records.iter().any(|r| r.surface == "言う"));
}

/// Phase 3a: the sixth file. Records are sorted by codepoint on write and
/// binary-searched on read; the fixture's document order is 東 京 都 食,
/// which is not codepoint order, so a writer that forgot to sort would miss.
#[test]
fn a_kanjidic_record_round_trips() {
    let dir = tmpdir("kanji-hit");
    let report = build_with_kanji(&dir);
    assert_eq!(report.kanji, 4);
    assert_eq!(report.skipped_kanji, 0);

    let index = Index::open(&dir).unwrap();
    assert_eq!(index.kanji_count(), 4);
    for literal in ['東', '京', '都', '食'] {
        assert!(index.kanji(literal).is_some(), "{literal} must be found");
    }
    let east = index.kanji('東').expect("東");
    assert_eq!(east.on, vec!["トウ"]);
    assert_eq!(east.kun, vec!["ひがし"]);
    assert_eq!(east.meanings, vec!["east"]);
}

#[test]
fn a_character_without_a_record_misses() {
    let dir = tmpdir("kanji-miss");
    build_with_kanji(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.kanji('あ').is_none(), "kana have no record");
    // A JMdict headword character that the KANJIDIC2 fixture does not carry.
    assert!(index.kanji('本').is_none());
}

/// `build_from_reader` keeps its signature and supplies no KANJIDIC2, so
/// every pre-existing fixture and probe builds a valid index whose kanji
/// lookups all miss — never an error.
#[test]
fn an_index_built_without_kanjidic_opens_and_misses() {
    let dir = tmpdir("kanji-none");
    let report = build(&dir);
    assert_eq!(report.kanji, 0);
    assert!(
        dir.join("kanji.bin").exists(),
        "the file is written even when empty"
    );

    let index = Index::open(&dir).unwrap();
    assert_eq!(index.kanji_count(), 0);
    assert!(index.kanji('東').is_none());
}

/// The upgrade path. An installed v4 index has a 24-byte header — four `u32`
/// counts and the `u64` fingerprint — and `ensure_dictionary` rebuilds on
/// exactly `VersionMismatch`. That only works if a v5 loader can still
/// *decode* a v4 header far enough to read its version, which is why
/// `IndexHeader` did not gain a field for this format (plan §Global
/// Constraints): a sixth field would turn this into an `Encoding` error
/// before the version is ever compared, and the app would report a failure
/// a download cannot fix instead of rebuilding.
#[test]
fn a_v4_header_is_refused_with_a_version_mismatch() {
    let dir = tmpdir("kanji-v4-header");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let fingerprint = index.header().conjugation_fingerprint;
    drop(index);

    // The v4 layout, byte for byte: version, keys, records, entries, fingerprint.
    let v4 = bincode::serialize(&(4u32, 3u32, 3u32, 3u32, fingerprint)).unwrap();
    assert_eq!(v4.len(), 24);
    std::fs::write(dir.join("header.bin"), v4).unwrap();

    match Index::open(&dir).unwrap_err() {
        IndexError::VersionMismatch { found, expected } => {
            assert_eq!(found, 4);
            assert_eq!(expected, INDEX_FORMAT_VERSION);
        }
        other => panic!("a v4 header must be a version mismatch, got {other:?}"),
    }
}

/// Phase 3b changed `KanjiData`'s layout, so a v5 `kanji.bin` must never be
/// decoded by this loader; the header's version is compared before that
/// file is read. The header itself is still the 24-byte v4/v5 layout, which
/// is what lets a v6 loader read a v5 file's version at all.
#[test]
fn a_v5_header_is_refused_with_a_version_mismatch() {
    let dir = tmpdir("kanji-v5-header");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let fingerprint = index.header().conjugation_fingerprint;
    drop(index);

    // The v5 layout, byte for byte: version, keys, records, entries, fingerprint.
    let v5 = bincode::serialize(&(5u32, 3u32, 3u32, 3u32, fingerprint)).unwrap();
    assert_eq!(v5.len(), 24);
    std::fs::write(dir.join("header.bin"), v5).unwrap();

    match Index::open(&dir).unwrap_err() {
        IndexError::VersionMismatch { found, expected } => {
            assert_eq!(found, 5);
            assert_eq!(expected, INDEX_FORMAT_VERSION);
        }
        other => panic!("a v5 header must be a version mismatch, got {other:?}"),
    }
}

/// A build whose KANJIDIC2 is unreadable fails the build, the way a bad
/// JMdict does — knowingly, over a silent-degrade path (design §4).
#[test]
fn a_malformed_kanjidic_fails_the_build() {
    let dir = tmpdir("kanji-malformed");
    let err = build_from_readers(
        std::io::Cursor::new(FIXTURE),
        std::io::Cursor::new("<kanjidic2><character><literal>本"),
        &dir,
    )
    .unwrap_err();
    assert!(matches!(err, IndexError::Kanjidic(_)), "got {err:?}");
}
