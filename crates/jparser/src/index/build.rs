// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Writes the on-disk index. The builder is [`crate::engine::build`]; this is
//! the format's one writer, so the two cannot drift.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::index::{
    EntryData, IndexError, IndexHeader, StoredRecord, ENTRIES_FILE, ENTRIES_INDEX_FILE, FST_FILE,
    HEADER_FILE, INDEX_FORMAT_VERSION, LEN_PREFIX_BYTES, RECORDS_FILE,
};

/// Writes the five index files from an already-assembled payload: the FST plus
/// the two length-prefixed bincode blobs (records, entries), the entry offset
/// table, and the header.
///
/// The format's one writer. `fingerprint` is a parameter rather than a
/// constant read inside this function: the caller supplies the table
/// fingerprint it built against.
pub(crate) fn write_index(
    by_key: BTreeMap<String, Vec<StoredRecord>>,
    entries: Vec<EntryData>,
    record_count: usize,
    fingerprint: u64,
    out_dir: &Path,
) -> Result<(), IndexError> {
    // Payload: length-prefixed bincode blobs; the FST value is the offset.
    let mut records_blob: Vec<u8> = Vec::new();
    let mut fst_builder =
        fst::MapBuilder::new(BufWriter::new(File::create(out_dir.join(FST_FILE))?))?;
    for (key, records) in &by_key {
        let offset = records_blob.len() as u64;
        let encoded = bincode::serialize(records)?;
        let len_prefix: [u8; LEN_PREFIX_BYTES] = (encoded.len() as u32).to_le_bytes();
        records_blob.extend_from_slice(&len_prefix);
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
        let len_prefix: [u8; LEN_PREFIX_BYTES] = (encoded.len() as u32).to_le_bytes();
        entries_blob.extend_from_slice(&len_prefix);
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
        conjugation_fingerprint: fingerprint,
    };
    std::fs::write(out_dir.join(HEADER_FILE), bincode::serialize(&header)?)?;

    Ok(())
}
