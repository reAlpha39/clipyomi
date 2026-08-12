// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Open an index and walk it.

use std::fs::File;
use std::path::Path;

use fst::raw::Fst;
use memmap2::Mmap;

use crate::index::{
    EntryData, IndexError, IndexHeader, StoredRecord, ENTRIES_FILE, ENTRIES_INDEX_FILE, FST_FILE,
    HEADER_FILE, INDEX_FORMAT_VERSION, RECORDS_FILE,
};
use crate::kana::unify;

/// One dictionary key that is a prefix of the queried text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixHit {
    /// Length of the matched key **in characters** of the source text.
    pub key_chars: usize,
    pub records: Vec<StoredRecord>,
}

pub struct Index {
    fst: Fst<Mmap>,
    records: Mmap,
    entries: Mmap,
    entry_offsets: Vec<(u32, u64)>,
    header: IndexHeader,
}

// `fst::raw::Fst` does not implement `Debug`, so this is written by hand.
// Needed so `Result<Index, IndexError>::unwrap_err()` compiles in tests.
impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

fn map(path: &Path) -> Result<Mmap, IndexError> {
    let file = File::open(path)?;
    // Safety: index files are immutable for the process lifetime. The app
    // rebuilds to a temp directory and renames rather than mutating a live
    // index in place, so no writer can shrink these files underneath us.
    Ok(unsafe { Mmap::map(&file)? })
}

impl Index {
    pub fn open(dir: &Path) -> Result<Self, IndexError> {
        let header: IndexHeader = bincode::deserialize(&std::fs::read(dir.join(HEADER_FILE))?)?;
        if header.version != INDEX_FORMAT_VERSION {
            return Err(IndexError::VersionMismatch {
                found: header.version,
                expected: INDEX_FORMAT_VERSION,
            });
        }
        let entry_offsets: Vec<(u32, u64)> =
            bincode::deserialize(&std::fs::read(dir.join(ENTRIES_INDEX_FILE))?)?;
        Ok(Self {
            fst: Fst::new(map(&dir.join(FST_FILE))?)?,
            records: map(&dir.join(RECORDS_FILE))?,
            entries: map(&dir.join(ENTRIES_FILE))?,
            entry_offsets,
            header,
        })
    }

    pub fn header(&self) -> &IndexHeader {
        &self.header
    }

    /// Every dictionary key that is a prefix of `text`, shortest first.
    ///
    /// This single walk replaces ta-old's binary-search-per-length loop:
    /// stepping the transducer one character at a time, every node that is
    /// final marks a complete headword or stem.
    pub fn prefixes_of(&self, text: &str) -> Result<Vec<PrefixHit>, IndexError> {
        let mut node = self.fst.root();
        let mut output = 0u64;
        let mut hits = Vec::new();

        for (consumed, ch) in text.chars().enumerate() {
            let mut buf = [0u8; 4];
            for &byte in unify(ch).encode_utf8(&mut buf).as_bytes() {
                let Some(i) = node.find_input(byte) else {
                    return Ok(hits);
                };
                let transition = node.transition(i);
                output += transition.out.value();
                node = self.fst.node(transition.addr);
            }
            if node.is_final() {
                let offset = output + node.final_output().value();
                hits.push(PrefixHit {
                    key_chars: consumed + 1,
                    records: self.records_at(offset)?,
                });
            }
        }
        Ok(hits)
    }

    fn records_at(&self, offset: u64) -> Result<Vec<StoredRecord>, IndexError> {
        Ok(bincode::deserialize(slice_at(&self.records, offset)?)?)
    }

    pub fn entry(&self, id: u32) -> Result<Option<EntryData>, IndexError> {
        let Ok(i) = self.entry_offsets.binary_search_by_key(&id, |(k, _)| *k) else {
            return Ok(None);
        };
        let (_, offset) = self.entry_offsets[i];
        Ok(Some(bincode::deserialize(slice_at(
            &self.entries,
            offset,
        )?)?))
    }
}

/// Read a `u32`-length-prefixed blob at `offset`.
fn slice_at(blob: &[u8], offset: u64) -> Result<&[u8], IndexError> {
    let start = usize::try_from(offset).map_err(|_| corrupt("offset out of range"))?;
    let len_end = start
        .checked_add(4)
        .ok_or_else(|| corrupt("offset overflows"))?;
    let prefix = blob
        .get(start..len_end)
        .ok_or_else(|| corrupt("length prefix past end of payload"))?;
    let len = u32::from_le_bytes(prefix.try_into().expect("checked 4 bytes")) as usize;
    let end = len_end
        .checked_add(len)
        .ok_or_else(|| corrupt("length overflows"))?;
    blob.get(len_end..end)
        .ok_or_else(|| corrupt("blob past end of payload"))
}

fn corrupt(reason: &str) -> IndexError {
    IndexError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("index payload corrupt: {reason}"),
    ))
}
