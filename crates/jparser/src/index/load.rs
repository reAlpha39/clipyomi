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
    HEADER_FILE, INDEX_FORMAT_VERSION, LEN_PREFIX_BYTES, RECORDS_FILE,
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
    // SAFETY: memmap2 requires that the mapped file is not mutated or
    // truncated while the mapping is alive, or the mapping is undefined
    // behavior (a SIGBUS on truncation, or this type handing out a `&[u8]`
    // that is presented as immutable but is actually being rewritten
    // underneath the reader). This crate does not enforce that on its own:
    // `build::build_from_reader` writes every index file in place via
    // `File::create`/`std::fs::write` (there is no build-to-temp-and-rename),
    // so nothing stops a rebuild from racing an `Index` that already has this
    // directory mapped. Soundness rests entirely on the caller obligation
    // documented on `Index::open`: no process may write to an index
    // directory while an `Index` for it is open.
    //
    // When a caller finally needs that obligation lifted, the fix is a fresh
    // directory per build — `<root>/.build-<nonce>` renamed to `<root>/gen-<N>`,
    // with readers taking the highest `gen-N` — and NOT a swap onto a live
    // path. Renaming over `dir`, flipping a symlink, and updating a `CURRENT`
    // pointer all look atomic and are all insufficient, because `Index::open`
    // is not atomic: it reads five files in sequence, so a reader that starts
    // mid-swap can splice one generation's `entries.idx` onto another's
    // `entries.bin` and then return well-formed wrong answers with no error.
    // A `gen-N` directory's contents never change after creation, so an open
    // that straddles a publish either succeeds wholly or fails with ENOENT.
    // Note also that `fs::rename` cannot replace a non-empty directory at all
    // (ENOTEMPTY), so the swap is not even expressible as a single operation.
    Ok(unsafe { Mmap::map(&file)? })
}

impl Index {
    /// Opens the index `build::build_from_reader` wrote to `dir`.
    ///
    /// # Caller obligation
    ///
    /// No process may write to `dir` for as long as the returned `Index`
    /// stays alive. The index files are memory-mapped, and the builder
    /// writes in place rather than building to a temp directory and
    /// renaming, so rebuilding into a directory with an open `Index` is
    /// undefined behavior, not just a race that yields stale reads.
    ///
    /// A rebuild interrupted partway is not detected either. The header is
    /// written last and only its version and conjugation fingerprint are
    /// validated here, and both still match after a torn rebuild, so the
    /// index opens cleanly and can then return wrong data with no error.
    /// Build into a fresh directory and point readers at it only once the
    /// build has completed; see the note on `map` above for why swapping a
    /// live path is not a substitute.
    pub fn open(dir: &Path) -> Result<Self, IndexError> {
        let header: IndexHeader = bincode::deserialize(&std::fs::read(dir.join(HEADER_FILE))?)?;
        if header.version != INDEX_FORMAT_VERSION {
            return Err(IndexError::VersionMismatch {
                found: header.version,
                expected: INDEX_FORMAT_VERSION,
            });
        }
        let expected_fingerprint = crate::conjugation::embedded_asset_fingerprint();
        if header.conjugation_fingerprint != expected_fingerprint {
            return Err(IndexError::ConjugationMismatch {
                found: header.conjugation_fingerprint,
                expected: expected_fingerprint,
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

    /// Every dictionary key that is a prefix of `text`, shortest first,
    /// including the empty key when one is indexed.
    ///
    /// This single walk replaces ta-old's binary-search-per-length loop:
    /// stepping the transducer one character at a time, every node that is
    /// final marks a complete headword or stem. `""` is a prefix of every
    /// string, so the root node's finality is checked before consuming any
    /// character; an irregular verb whose whole surface is its own
    /// remove-suffix (する/vs-i, 来る や くる/vk) generates exactly this
    /// empty-key stem, and it is the only way that stem is ever returned.
    pub fn prefixes_of(&self, text: &str) -> Result<Vec<PrefixHit>, IndexError> {
        let mut node = self.fst.root();
        let mut output = 0u64;
        let mut hits = Vec::new();

        if node.is_final() {
            let offset = output + node.final_output().value();
            hits.push(PrefixHit {
                key_chars: 0,
                records: self.records_at(offset)?,
            });
        }

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

    /// Number of entries in the payload. Cheap: the offset table is already
    /// resident after `open`, so this touches no mmap page.
    pub fn entry_count(&self) -> usize {
        self.entry_offsets.len()
    }
}

/// Read a `u32`-length-prefixed blob at `offset`.
fn slice_at(blob: &[u8], offset: u64) -> Result<&[u8], IndexError> {
    let start = usize::try_from(offset).map_err(|_| corrupt("offset out of range"))?;
    let len_end = start
        .checked_add(LEN_PREFIX_BYTES)
        .ok_or_else(|| corrupt("offset overflows"))?;
    let prefix = blob
        .get(start..len_end)
        .ok_or_else(|| corrupt("length prefix past end of payload"))?;
    let len =
        u32::from_le_bytes(prefix.try_into().expect("checked LEN_PREFIX_BYTES bytes")) as usize;
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
