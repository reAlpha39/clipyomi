// SPDX-License-Identifier: MIT OR Apache-2.0

//! Japanese text parsing: dictionary matching, segmentation, entry assembly.
//!
//! `parse` is the whole public surface. It runs the Engine v2 core
//! (`engine`): candidate generation over a deinflection trie, a min-cost
//! segmentation with tokenizer boundaries as hard constraints, and entry
//! assembly — headword, reading and conjugation label — for each span.
//!
//! Every offset in this API is a **character** offset. The single conversion
//! point is `text.chars().collect()` at the top of `parse`; nothing below it
//! ever sees a byte index.

pub mod engine;
#[cfg(feature = "mecab")]
pub mod hints;
pub mod index;
pub mod jmdict;

pub use crate::engine::flags::WordFlags;
pub use crate::engine::parse;
pub use crate::engine::Boundaries;
/// One dictionary sense. A re-export rather than a parallel owned struct with
/// the same five fields.
pub use crate::index::SenseData as Sense;

/// Upper bound on [`ParseOptions::readings`]. The DP keeps a per-position
/// list proportional to `readings` (`engine::lattice::RAW_FACTOR`), and
/// nothing has a use for a reading list longer than this.
pub const MAX_READINGS: usize = 64;

/// Parse-time options.
///
/// `readings` is how many distinct segmentations `parse` returns: the best
/// as `ParseResult::segments`, the rest as `ParseResult::alternatives`. The
/// default is one, so a caller that never asks sees exactly what it always
/// did. Gloss filters and furigana modes are Phase 3 display concerns above
/// this crate. `#[non_exhaustive]` so adding a field later is not a breaking
/// change; construct with `ParseOptions::default()` or
/// [`ParseOptions::with_readings`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ParseOptions {
    /// Distinct readings to return, at least 1 and at most [`MAX_READINGS`].
    pub readings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self { readings: 1 }
    }
}

impl ParseOptions {
    /// `readings` distinct segmentations, cheapest first; clamped to
    /// `1..=MAX_READINGS` -- anything below 1 reads as 1, anything above
    /// `MAX_READINGS` reads as `MAX_READINGS`.
    pub fn with_readings(readings: usize) -> Self {
        Self {
            readings: readings.clamp(1, MAX_READINGS),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParseResult {
    /// A contiguous cover of the input in ascending `start` order: every
    /// character belongs to exactly one segment, matched or not. Empty iff
    /// the input is empty.
    pub segments: Vec<Segment>,
    /// Readings 2..k, each a contiguous cover like `segments` and distinct
    /// from it and from each other by the spans they match (Phase 2 design
    /// §3, §4). The order is the rank, and rank is not cost order alone:
    /// readings that differ from `segments` or an earlier alternative only
    /// by a merge or split come after every reading that does not, so the
    /// first alternatives are genuinely different analyses rather than
    /// re-splits of the top one (Phase 2b design §3, §4). Empty unless the
    /// caller asked for more than one reading through
    /// [`ParseOptions::with_readings`].
    pub alternatives: Vec<Vec<Segment>>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Segment {
    /// Character offset into the parsed text. Never a byte offset.
    pub start: usize,
    /// Length in characters.
    pub len: usize,
    /// `text[start..start + len]` verbatim.
    pub surface: String,
    /// Display reading, taken from `entries[0].reading`.
    ///
    /// `None` for an unmatched run — there is no morphological-analyzer
    /// fallback — and `None` whenever the primary entry has no reading:
    /// because the match already is a reading (`WordFlags::PRONOUNCE`),
    /// because the entry has no kanji form and so stores no readings, or
    /// because no stored reading could be stripped back to a stem.
    pub reading: Option<String>,
    pub matched: bool,
    /// Every dictionary entry aligning to this exact span, ordered most
    /// likely first: a caller that wants one entry to show by default can
    /// always take `entries[0]`, and the rest follow in descending order of
    /// likelihood. Empty when `!matched`.
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Entry {
    /// The dictionary form: the matched surface for a plain headword, or the
    /// stem with its verb type's remove-suffix restored for a stem match.
    pub headword: String,
    pub reading: Option<String>,
    /// `engine::label::fmt`'s English label for the deinflection chain, e.g.
    /// `"Polite Negative Past"` (Phase 1b-i design §3). `None` when no step
    /// contributes a word: an uninflected match, a plain form, or a bare
    /// stem.
    pub conjugation: Option<String>,
    /// Union of every sense's `pos`, in first-seen order, deduplicated.
    pub pos: Vec<String>,
    pub senses: Vec<Sense>,
    /// The match's flags, as carried by the dictionary record that won this
    /// span.
    ///
    /// A set flag says the record carries it. It is not a guarantee that a
    /// flag no longer applicable to this specific match (e.g. `COUNTER` on a
    /// candidate not used as a counter here) has been cleared before reaching
    /// this field. Whether and how the engine reconciles a record's flags
    /// against the winning match is an implementation detail — see
    /// [`crate::engine::assemble`] for what it does, if anything, before
    /// populating this field.
    pub flags: WordFlags,
    /// The JMdict `ent_seq` this candidate came from.
    ///
    /// Read straight off the element by `jmdict.rs` into `RawEntry::id`, so it
    /// is the same stable public identifier a JMdict consumer would cite. The
    /// Phase 0 harness scores entry top-1 against it (`gold.jsonl`'s
    /// `ent_seq`); exposing it changes nothing about what `parse` computes.
    pub entry_id: u32,
    /// The deinflection chain that produced `headword` from the matched
    /// surface, innermost step first. Empty for an uninflected match.
    /// `engine::label::fmt` turns this into `conjugation`'s English label;
    /// see docs/superpowers/specs/2026-09-02-engine-v2-phase1b-i-design.md §3.
    pub steps: Vec<crate::engine::ConjStep>,
}

/// Everything `parse` can fail at. Reading the memory-mapped index payload is
/// the only fallible step; the enum exists so `parse` does not leak
/// `IndexError` into its public signature, and so variants can be added
/// without a breaking change.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("reading the index failed: {0}")]
    Index(#[from] crate::index::IndexError),
}
