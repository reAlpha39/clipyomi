// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

pub mod conjugation;
pub mod index;
pub mod jmdict;
pub mod kana;
// Dead until `parse` lands: nothing outside the module's own tests calls the
// matcher yet. Task 7 removes this attribute. It covers the child module
// `matcher::verb` that Task 2 adds, too.
#[allow(dead_code)]
mod matcher;
mod rank;
pub mod record;
pub mod romaji;
mod segment;
pub mod stem;

pub use crate::segment::BoundaryHints;

/// Everything `parse` can fail at. Reading the memory-mapped index payload is
/// the only fallible step in Phase 1B; the enum exists so `parse` does not
/// leak `IndexError` into its public signature, and so variants can be added
/// without a breaking change.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("reading the index failed: {0}")]
    Index(#[from] crate::index::IndexError),
}
