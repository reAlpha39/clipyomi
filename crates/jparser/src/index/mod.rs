// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Memory-mapped dictionary index.
//!
//! Replaces ta-old's hand-rolled sorted-array-plus-binary-search index
//! (`LoadDict`/`FindMatches`, `ta-old/exe/util/Dictionary.cpp`). Keys are
//! surfaces normalized with `kana::unify`, so the kana-insensitive comparator
//! ta-old threaded through three separate binary searches does not exist here.
//! The original surface is kept in the payload so inexact hiragana/katakana
//! matches can still be detected and penalized in Phase 1B.

pub mod build;
pub mod load;

use serde::{Deserialize, Serialize};

use crate::conjugation::VerbTypeId;
use crate::stem::StemStats;

/// Bumped whenever the on-disk layout changes. A mismatch forces a rebuild; the
/// loader must never try to read an index it does not recognize.
pub const INDEX_FORMAT_VERSION: u32 = 3;

pub const HEADER_FILE: &str = "header.bin";
pub const FST_FILE: &str = "keys.fst";
pub const RECORDS_FILE: &str = "records.bin";
pub const ENTRIES_FILE: &str = "entries.bin";
pub const ENTRIES_INDEX_FILE: &str = "entries.idx";

/// Byte width of the little-endian length prefix `build` writes ahead of
/// each bincode blob in `records.bin`/`entries.bin`, and `load` reads back.
/// Shared so the two sides cannot drift apart independently.
pub const LEN_PREFIX_BYTES: usize = std::mem::size_of::<u32>();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexHeader {
    pub version: u32,
    pub keys: u32,
    pub records: u32,
    pub entries: u32,
    /// FNV-1a fingerprint of the conjugation asset this index's `verb_type`
    /// ids were assigned from (`conjugation::embedded_asset_fingerprint`).
    /// `Index::open` rejects a mismatch: reordering or adding a type in the
    /// asset changes every id downstream of the change, so an index built
    /// against a different asset would otherwise silently resolve
    /// `verb_type` to the wrong verb.
    pub conjugation_fingerprint: u64,
}

/// One headword or stem as stored in the payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredRecord {
    /// Original, unnormalized surface. Compared against the source text to set
    /// the inexact-match flag in Phase 1B.
    pub surface: String,
    pub flags: u16,
    /// `Some` for generated verb stems, `None` for plain headwords. ta-old
    /// stored `verbType` as `vt + 1`, with `0` meaning "not a verb"
    /// (`ConjInfo::verbType`, `Dictionary.h`); this port uses a 0-based id
    /// inside an `Option` instead, so the Phase 1B differential run must
    /// account for that offset (add 1, map `None` to `0`) when comparing
    /// against the old encoding.
    pub verb_type: Option<VerbTypeId>,
    pub entry_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SenseData {
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
    pub xrefs: Vec<String>,
    pub misc: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryData {
    pub id: u32,
    /// The entry's `<reb>` forms in document order, stored **only when the
    /// entry also has kanji forms** — exactly the set ta-old flags
    /// `JAP_WORD_PRONOUNCE`. Empty for kana-only entries, where the surface
    /// already is the reading and ta-old renders no furigana.
    pub readings: Vec<String>,
    pub senses: Vec<SenseData>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BuildReport {
    pub keys: usize,
    pub records: usize,
    pub entries: usize,
    /// Malformed JMdict entries skipped. Surfaced, never silently dropped.
    pub skipped_entries: usize,
    pub stems: StemStats,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("index fst failed: {0}")]
    Fst(#[from] fst::Error),
    #[error("index encoding failed: {0}")]
    Encoding(#[from] bincode::Error),
    #[error("reading JMdict failed: {0}")]
    Jmdict(#[from] crate::jmdict::JmdictError),
    #[error("index format version mismatch: found {found}, expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },
    #[error(
        "index conjugation asset mismatch: found fingerprint {found:#x}, \
         expected {expected:#x}"
    )]
    ConjugationMismatch { found: u64, expected: u64 },
}
