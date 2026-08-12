// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Build an index from JMdict XML.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufWriter, Write};
use std::path::Path;

use crate::conjugation::ConjugationTable;
use crate::index::{
    BuildReport, EntryData, IndexError, IndexHeader, SenseData, StoredRecord, ENTRIES_FILE,
    ENTRIES_INDEX_FILE, FST_FILE, HEADER_FILE, INDEX_FORMAT_VERSION, RECORDS_FILE,
};
use crate::jmdict::parse_entries;
use crate::kana::unify_str;
use crate::record::headwords;
use crate::stem::{generate_stems, StemOptions, StemStats};

pub fn build_from_reader<R: BufRead>(
    xml: R,
    table: &ConjugationTable,
    opts: &StemOptions,
    out_dir: &Path,
) -> Result<BuildReport, IndexError> {
    std::fs::create_dir_all(out_dir)?;

    // BTreeMap gives the lexicographic key order fst::MapBuilder requires.
    let mut by_key: BTreeMap<String, Vec<StoredRecord>> = BTreeMap::new();
    let mut entries: Vec<EntryData> = Vec::new();
    let mut stems = StemStats::default();
    let mut record_count = 0usize;

    let mut reader = parse_entries(xml);
    while let Some(result) = reader.next() {
        let raw = result?;
        entries.push(EntryData {
            id: raw.id,
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

        for head in headwords(&raw, table) {
            let stem_records = generate_stems(&head, table, opts, &mut stems);

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

            for stem in stem_records {
                if push(
                    &mut by_key,
                    &stem.surface,
                    StoredRecord {
                        surface: stem.surface.clone(),
                        flags: stem.flags.0,
                        verb_type: stem.verb_types.first().copied(),
                        entry_id: stem.entry_id,
                    },
                ) {
                    record_count += 1;
                }
            }
        }
    }
    let skipped_entries = reader.skipped_count();

    // Payload: length-prefixed bincode blobs; the FST value is the offset.
    let mut records_blob: Vec<u8> = Vec::new();
    let mut fst_builder =
        fst::MapBuilder::new(BufWriter::new(File::create(out_dir.join(FST_FILE))?))?;
    for (key, records) in &by_key {
        let offset = records_blob.len() as u64;
        let encoded = bincode::serialize(records)?;
        records_blob.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        records_blob.extend_from_slice(&encoded);
        fst_builder.insert(key.as_bytes(), offset)?;
    }
    fst_builder.into_inner()?.flush()?;
    std::fs::write(out_dir.join(RECORDS_FILE), &records_blob)?;

    // Entry data plus a sorted (id, offset) table for binary search on load.
    let mut entries_blob: Vec<u8> = Vec::new();
    let mut entry_offsets: Vec<(u32, u64)> = Vec::with_capacity(entries.len());
    for entry in &entries {
        let offset = entries_blob.len() as u64;
        let encoded = bincode::serialize(entry)?;
        entries_blob.extend_from_slice(&(encoded.len() as u32).to_le_bytes());
        entries_blob.extend_from_slice(&encoded);
        entry_offsets.push((entry.id, offset));
    }
    entry_offsets.sort_unstable_by_key(|(id, _)| *id);
    std::fs::write(out_dir.join(ENTRIES_FILE), &entries_blob)?;
    std::fs::write(
        out_dir.join(ENTRIES_INDEX_FILE),
        bincode::serialize(&entry_offsets)?,
    )?;

    let header = IndexHeader {
        version: INDEX_FORMAT_VERSION,
        keys: by_key.len() as u32,
        records: record_count as u32,
        entries: entries.len() as u32,
    };
    std::fs::write(out_dir.join(HEADER_FILE), bincode::serialize(&header)?)?;

    Ok(BuildReport {
        keys: by_key.len(),
        records: record_count,
        entries: entries.len(),
        skipped_entries,
        stems,
    })
}

/// Insert under the normalized key. Returns false if an identical record was
/// already present, so counts do not double-report duplicates.
fn push(
    map: &mut BTreeMap<String, Vec<StoredRecord>>,
    surface: &str,
    record: StoredRecord,
) -> bool {
    let bucket = map.entry(unify_str(surface)).or_default();
    if bucket.contains(&record) {
        return false;
    }
    bucket.push(record);
    true
}
