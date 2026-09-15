// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! The segmentation engine. `crate::parse` is this module's `parse`.
//!
//! Written clean-room for ClipYomi (master plan §3.5); the Phase 1a design
//! records its cost model and the Phase 1b-i design its label formatter.

use crate::index::load::Index;
use crate::index::IndexError;
use crate::{ParseError, ParseOptions, ParseResult};

pub mod assemble;
pub mod build;
pub mod chain;
pub mod cost;
pub mod flags;
pub mod kana;
pub mod label;
pub mod lattice;
pub mod lex;
pub mod relax;
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

    /// Whether a span may start or end at `pos` — `cost::is_allowed`'s
    /// question, which is not quite `is_boundary`'s.
    ///
    /// They differ only under [`relax::Relaxed`], which also permits a
    /// position strictly inside a token no candidate can cover (Phase 4
    /// design §2). The default is `is_boundary`, so a tokenizer's own
    /// boundary set keeps exactly today's behaviour, and `cost::cost` goes on
    /// asking `is_boundary` for its crossing count — widening *that* would
    /// reprice every PRONOUNCE-only match through `PRONOUNCE_CROSS`.
    fn is_usable_endpoint(&self, pos: usize) -> bool {
        self.is_boundary(pos)
    }
}

/// One step of a deinflection chain: which conjugation class was applied, in
/// which form, over which characters of the span.
///
/// Filled by Phase 1a (`chain.rs`); named in English by Phase 1b-i
/// (`label.rs`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConjStep {
    pub class: ClassId,
    pub form: FormId,
    pub range: std::ops::Range<usize>,
    /// The auxiliary this step is, as `table.rs`'s `ATTACHES_TO` spells its
    /// lemma — `Some("られる")`, `Some("ます")` — or `None` on the first step,
    /// which is the main word's own inflection and belongs to no auxiliary.
    /// Recorded here because the class alone cannot say: せる, させる, れる
    /// and られる are all 一段, and ん, う, よう, て and で are all 不変化型.
    pub aux: Option<&'static str>,
}

/// Parse `text` against an already-open index — the crate's public entry
/// point, re-exported as `jparser::parse`.
///
/// `bounds` is a tokenizer's boundary set, applied as a hard constraint:
/// no span may start or end strictly inside a token. `None` lifts every
/// constraint.
///
/// Three stages, each already a separate module: [`lex::candidates`] finds
/// every dictionary match at every span, [`lattice::k_best`] chooses the
/// `opts.readings` cheapest distinct covers among them, and
/// [`assemble::to_segment`] resolves each step of each cover into the
/// public [`Segment`]/[`Entry`] shape. The cheapest cover is
/// `ParseResult::segments`; the rest are `alternatives`.
///
/// [`Segment`]: crate::Segment
/// [`Entry`]: crate::Entry
pub fn parse(
    index: &Index,
    table: &table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    let by_start = lex::candidates(index, table, &chars)?;
    // Phase 4: a token no candidate can cover forbids every span touching it,
    // which leaves its characters unmatchable rather than merely expensive.
    // `Relaxed` reopens those positions and no others; `None` stays `None`,
    // so the no-tokenizer path is untouched by construction.
    let relaxed = bounds.map(|b| relax::Relaxed::over(&chars, &by_start, b));
    let bounds = relaxed.as_ref().map(|r| r as &dyn Boundaries).or(bounds);
    let mut paths = lattice::k_best(&chars, &by_start, bounds, opts.readings);
    let assemble = |path: &[lattice::Step]| -> Result<Vec<crate::Segment>, IndexError> {
        path.iter()
            .map(|step| assemble::to_segment(index, &chars, step, &by_start))
            .collect()
    };
    // `k_best` always returns at least one path, an empty one for empty
    // input. `remove`, not `swap_remove`: the rest must stay in cost order.
    let segments = assemble(&paths.remove(0))?;
    let mut alternatives = Vec::with_capacity(paths.len());
    for path in &paths {
        alternatives.push(assemble(path)?);
    }
    Ok(ParseResult {
        segments,
        alternatives,
    })
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
    fn a_boundaries_impl_is_object_safe() {
        let b: &dyn Boundaries = &EveryPosition;
        assert!(b.is_boundary(0));
    }
}
