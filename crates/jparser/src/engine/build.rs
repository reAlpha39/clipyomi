// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! Builds the index from JMdict XML and KANJIDIC2 XML: one `StoredRecord`
//! per [`crate::engine::flags::Headword`], keyed by the folded surface, and
//! no generated stems — deinflection is a parse-time walk of
//! [`crate::engine::table::Table`]. [`fingerprint`] stamps the header so
//! `Index::open` refuses an index built against a different table.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use crate::engine::flags::headwords;
use crate::engine::kana::fold_str;
use crate::engine::table::CLASSES;
use crate::index::build::write_index;
use crate::index::{BuildReport, EntryData, IndexError, KanjiData, SenseData, StoredRecord};
use crate::jmdict::parse_entries;
use crate::kanjidic::parse_characters;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Folds `s`'s bytes into `hash` with FNV-1a, then a trailing zero byte so
/// two fields cannot be told apart from their concatenation (`"ab"` + `"c"`
/// would otherwise hash the same as `"a"` + `"bc"`).
fn fnv1a_feed(mut hash: u64, s: &str) -> u64 {
    for byte in s.bytes().chain(std::iter::once(0)) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// FNV-1a fingerprint of [`CLASSES`]: every class's name and remove-suffix,
/// then every one of its forms' name and suffix, in table order.
///
/// The property this must have: reordering, adding or removing a class or
/// form changes every [`crate::engine::ClassId`] downstream of the change, so
/// it must also change this fingerprint, or an index built against a stale
/// table would open cleanly and silently misresolve classes. The index header
/// carries the value and `Index::open` refuses a mismatch.
pub fn fingerprint() -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for class in CLASSES {
        hash = fnv1a_feed(hash, class.name);
        hash = fnv1a_feed(hash, class.remove);
        for form in class.forms {
            hash = fnv1a_feed(hash, form.name);
            hash = fnv1a_feed(hash, form.suffix);
        }
    }
    hash
}

/// Builds an index from JMdict `xml` alone: [`build_from_readers`] with an
/// empty KANJIDIC2 reader, so `Index::kanji` answers `None` for every
/// character. Kept with this signature so every fixture, test, example and
/// probe that predates the kanji layer builds unchanged (Phase 3a design §3.2).
pub fn build_from_reader<R: BufRead>(xml: R, out_dir: &Path) -> Result<BuildReport, IndexError> {
    build_from_readers(xml, std::io::empty(), out_dir)
}

/// Builds an index from JMdict `xml` and KANJIDIC2 `kanjidic` into `out_dir`.
pub fn build_from_readers<J: BufRead, K: BufRead>(
    xml: J,
    kanjidic: K,
    out_dir: &Path,
) -> Result<BuildReport, IndexError> {
    std::fs::create_dir_all(out_dir)?;

    // BTreeMap gives the lexicographic key order fst::MapBuilder requires.
    let mut by_key: BTreeMap<String, Vec<StoredRecord>> = BTreeMap::new();
    let mut entries: Vec<EntryData> = Vec::new();
    let mut record_count = 0usize;

    let mut reader = parse_entries(xml);
    // `by_ref()` rather than `while let Some(..) = reader.next()`: the loop
    // only needs a temporary borrow, and `reader.skipped_count()` below needs
    // the iterator itself back afterward.
    for result in reader.by_ref() {
        let raw = result?;
        entries.push(EntryData {
            id: raw.id,
            readings: if raw.kanji.is_empty() {
                Vec::new()
            } else {
                raw.readings.iter().map(|r| r.text.clone()).collect()
            },
            senses: raw
                .senses
                .iter()
                .map(|s| SenseData {
                    pos: s.pos.clone(),
                    glosses: s.glosses.clone(),
                    xrefs: s.xrefs.clone(),
                    misc: s.misc.clone(),
                    info: s.info.clone(),
                })
                .collect(),
        });

        // Headwords only, no generated stems: deinflection is a parse-time
        // reverse-trie walk (`engine::table::Table`) on this path.
        for head in headwords(&raw) {
            if push(
                &mut by_key,
                &head.surface,
                StoredRecord {
                    surface: head.surface.clone(),
                    flags: head.flags.0,
                    entry_id: head.entry_id,
                },
            ) {
                record_count += 1;
            }
        }
    }
    let skipped_entries = reader.skipped_count();

    // Read after JMdict, not interleaved: the two files are independent and
    // a failure in either fails the build the same way.
    let mut kanji_reader = parse_characters(kanjidic);
    let kanji: Vec<KanjiData> = kanji_reader.by_ref().collect::<Result<_, _>>()?;
    let skipped_kanji = kanji_reader.skipped_count();
    let kanji_count = kanji.len();

    let keys = by_key.len();
    let entry_count = entries.len();
    write_index(by_key, entries, record_count, kanji, fingerprint(), out_dir)?;

    Ok(BuildReport {
        keys,
        records: record_count,
        entries: entry_count,
        kanji: kanji_count,
        skipped_entries,
        skipped_kanji,
    })
}

/// Insert under the v2-normalized key. Returns false if an identical record
/// was already present, so counts do not double-report duplicates.
fn push(
    map: &mut BTreeMap<String, Vec<StoredRecord>>,
    surface: &str,
    record: StoredRecord,
) -> bool {
    let bucket = map.entry(fold_str(surface)).or_default();
    if bucket.contains(&record) {
        return false;
    }
    bucket.push(record);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_across_calls() {
        assert_eq!(fingerprint(), fingerprint());
    }

    /// Pinned so a regeneration of `table_data.rs` is a visible, deliberate
    /// change: every class and form id an index stores is relative to that
    /// table, and `Index::open` refuses an index whose fingerprint differs.
    /// After an intended regeneration, run this test, take the `left` value
    /// from its failure, and replace the literal. Never loosen it.
    #[test]
    fn fingerprint_is_pinned() {
        assert_eq!(fingerprint(), 0xfeae_49c6_63b9_fb24);
    }
}
