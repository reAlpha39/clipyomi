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

//! The Engine v2 segmentation core.
//!
//! Reachable only through `parse_v2` and `jparser-cli --engine v2` for the
//! whole of Phase 1a; `parse` remains the default path. The switchover is
//! Phase 1b's, so that no build ships without a conjugation label.

use crate::index::load::Index;
use crate::{ParseError, ParseOptions, ParseResult, Segment};

pub mod build;
pub mod cost;
pub mod flags;
pub mod kana;
pub mod lattice;
pub mod lex;
pub mod romaji;
pub mod table;
mod table_data;

pub type ClassId = u16;
pub type FormId = u16;

/// Where a tokenizer says one token ends and the next begins.
///
/// A character position `p` in `0..=len`. `is_boundary(0)` and
/// `is_boundary(len)` are true by definition, because every tokenizer agrees
/// the text begins and ends where it does.
///
/// This replaces the advisory `bad_start`/`bad_end` pair (master plan §3.2's
/// note on `flags_from_worker`): the engine asks where the tokenizer put its
/// cuts, not whether a position is interior to a token.
pub trait Boundaries {
    fn is_boundary(&self, pos: usize) -> bool;
}

/// One step of a deinflection chain: which conjugation class was applied, in
/// which form, over which characters of the span.
///
/// Filled by Phase 1a (`chain.rs`); named in English by Phase 1b. Phase 1a
/// leaves `Entry::conjugation` as `None`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConjStep {
    pub class: ClassId,
    pub form: FormId,
    pub range: std::ops::Range<usize>,
}

/// One unmatched segment covering the whole input. Replaced in Task 14.
fn stub_result(text: &str) -> ParseResult {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return ParseResult {
            segments: Vec::new(),
        };
    }
    ParseResult {
        segments: vec![Segment {
            start: 0,
            len: chars.len(),
            surface: text.to_string(),
            reading: None,
            matched: false,
            entries: Vec::new(),
        }],
    }
}

/// Parse `text` with the Engine v2 core.
///
/// Signature-compatible with [`crate::parse`] apart from the boundary
/// parameter, which is a constraint source rather than an advisory hint.
pub fn parse_v2(
    index: &Index,
    table: &table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError> {
    let _ = (index, table, opts, bounds);
    Ok(stub_result(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Boundaries` that says every position is a boundary — the shape a
    /// character-level tokenizer would produce.
    struct EveryPosition;
    impl Boundaries for EveryPosition {
        fn is_boundary(&self, _pos: usize) -> bool {
            true
        }
    }

    #[test]
    fn the_stub_covers_the_input_exactly_once() {
        let r = stub_result("東京都に住む");
        assert_eq!(r.segments.len(), 1);
        assert_eq!(r.segments[0].start, 0);
        assert_eq!(r.segments[0].len, 6, "length is in characters, not bytes");
        assert_eq!(r.segments[0].surface, "東京都に住む");
        assert!(!r.segments[0].matched);
        assert!(r.segments[0].entries.is_empty());
    }

    #[test]
    fn empty_input_produces_no_segments() {
        assert!(stub_result("").segments.is_empty());
    }

    #[test]
    fn a_boundaries_impl_is_object_safe() {
        let b: &dyn Boundaries = &EveryPosition;
        assert!(b.is_boundary(0));
    }
}
