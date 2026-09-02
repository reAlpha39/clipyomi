// ClipYomi — Engine v2, Phase 1a.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b, when the crate stops being GPL-2.0-only.

//! Builds a v2 index from JMdict XML.
//!
//! Shares `index::build::write_index` with the v1 builder: the on-disk format
//! does not change between engines, only what fills it. v2 emits one
//! `StoredRecord` per [`crate::engine::flags::Headword`] and none of v1's
//! generated stems, because deinflection happens at parse time via
//! [`crate::engine::table::Table`] instead of at build time — so
//! `StoredRecord::verb_type` is always `None` here, and [`fingerprint`] rather
//! than `conjugation::embedded_asset_fingerprint` stamps the header, so
//! `Index::open_with` rejects an index built by the other engine.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use crate::engine::flags::headwords;
use crate::engine::kana::fold_str;
use crate::engine::table::CLASSES;
use crate::index::build::write_index;
use crate::index::{BuildReport, EntryData, IndexError, SenseData, StoredRecord};
use crate::jmdict::parse_entries;
use crate::stem::StemStats;

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
/// The property this must have is the same one
/// `conjugation::embedded_asset_fingerprint` gives the v1 asset: reordering,
/// adding or removing a class or form changes every `verb_type`/class id
/// downstream of the change, so it must also change this fingerprint, or an
/// index built against a stale table would open cleanly and silently
/// misresolve classes.
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

/// Builds a v2 index from `xml` into `out_dir`.
///
/// Signature-compatible with [`crate::index::build::build_from_reader`] apart
/// from the missing `table`/`opts` parameters, which v1 needs to generate
/// stems and v2 does not.
pub fn build_from_reader<R: BufRead>(xml: R, out_dir: &Path) -> Result<BuildReport, IndexError> {
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
                    verb_type: None,
                    entry_id: head.entry_id,
                },
            ) {
                record_count += 1;
            }
        }
    }
    let skipped_entries = reader.skipped_count();

    let keys = by_key.len();
    let entry_count = entries.len();
    write_index(by_key, entries, record_count, fingerprint(), out_dir)?;

    Ok(BuildReport {
        keys,
        records: record_count,
        entries: entry_count,
        skipped_entries,
        // v2 generates no stems: deinflection happens at parse time instead.
        stems: StemStats::default(),
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

    #[test]
    fn fingerprint_differs_from_the_v1_conjugation_asset() {
        // If these ever collided, the fingerprint guard in `Index::open`
        // could not tell the two engines' indexes apart.
        assert_ne!(
            fingerprint(),
            crate::conjugation::embedded_asset_fingerprint()
        );
    }
}
