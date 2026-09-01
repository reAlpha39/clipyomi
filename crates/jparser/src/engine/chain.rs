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

//! The span shape of an auxiliary chain -- Phase 1a's half of master plan
//! §7.3's recombiner (spec §2.1). Task 10 already decided the span shape (a
//! chain that survives as one candidate is one segment); what is left is
//! making sure the steps that reach `Entry` are correct, ordered, and
//! correctly ranged, because Phase 1b's formatter reads nothing else.

use crate::engine::lex::Candidate;
use crate::engine::ConjStep;

/// The deinflection chain behind `cand`, in the order and ranges Phase 1b's
/// English-label renderer relies on.
///
/// The body is `cand.steps.clone()` -- `Table::search` (by way of
/// `chains_ending_at` and then `lex::candidates`, which clones `hit.steps`
/// verbatim into `Candidate::steps`) already builds the steps in the shape
/// described below, so there is nothing left to compute. This function is
/// kept anyway as the one named place Phase 1b reads, rather than a struct
/// field reached into from another module. The tests in `tests/engine_v2.rs`
/// (`candidate_for` and friends) are what actually pin the contract:
///
/// - **Innermost first.** `steps[0]` is the main word's own inflection --
///   the class and form the dictionary entry conjugates in directly -- and
///   each following step is one auxiliary further out, ending with the
///   outermost. Phase 1b renders left to right from this order; reversing
///   it produces a backwards label.
/// - **Ranges are absolute.** Every `range` is a character offset into the
///   whole text `lex::candidates` was called with, *not* relative to
///   `cand.start`. `Table::search` builds each step from an absolute `end`
///   position (`lex::candidates` calls `chains_ending_at` once per absolute
///   end, shared across every start that reaches it), and nothing between
///   there and here re-bases them. Do not offset by `cand.start`.
/// - **The ranges tile the conjugated tail.** For consecutive steps,
///   `steps[i].range.end == steps[i + 1].range.start` -- no gap, no
///   overlap. A step's range can be empty (`n..n`): a class whose form has
///   an empty suffix (an ichidan verb's bare 連用形, e.g.) is a legitimate
///   zero-consumption terminal, and it tiles like any other step because an
///   empty range trivially satisfies `end == start`.
/// - **An uninflected match has no steps.** The empty chain (a plain
///   headword looked up as-is) carries `stem_class: None` and never reaches
///   `Table::search`, so `cand.steps` -- and this function's return -- is
///   empty.
pub fn steps_for(cand: &Candidate) -> Vec<ConjStep> {
    cand.steps.clone()
}
