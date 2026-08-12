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
pub const INDEX_FORMAT_VERSION: u32 = 1;

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
}

/// One headword or stem as stored in the payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredRecord {
    /// Original, unnormalized surface. Compared against the source text to set
    /// the inexact-match flag in Phase 1B.
    pub surface: String,
    pub flags: u16,
    /// `Some` for generated verb stems, `None` for plain headwords.
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
}
