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

//! The min-cost path over the candidate lattice: given every candidate span
//! [`lex::candidates`](crate::engine::lex::candidates) produced and the price
//! [`cost`](crate::engine::cost) puts on each, choose the contiguous cover of
//! the sentence that pays the least.
//!
//! A forward DP over end positions `0..=chars.len()`, relaxed in the single
//! place [`relax`] so that Phase 2's k-best (master plan §4.2) can swap the
//! single winner `best[]` holds for a bounded list without moving anything
//! else in this file.

use crate::engine::cost::{cost, is_allowed, unmatched_cost};
use crate::engine::lex::Candidate;
use crate::engine::Boundaries;

/// Longest unmatched run relaxed directly from a single `from` position.
///
/// Without a bound, relaxing every `from..to` pair is quadratic in the
/// length of a long unrecognised stretch (a URL, a wall of romaji, garbage
/// input). A run longer than this bound is still fully coverable — just as
/// consecutive shorter runs chained end to end, not as one step — so
/// correctness never depends on this value, only how many hops a pathological
/// gap takes. 64 is comfortably above any single unmatched stretch the corpus
/// is expected to show; Task 16 revisits it if that assumption doesn't hold.
const MAX_UNMATCHED_RUN: usize = 64;

/// One step of a winning segmentation: either a dictionary match, or a run of
/// input no candidate covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Matched(Candidate),
    Unmatched { start: usize, len: usize },
}

impl Step {
    fn len(&self) -> usize {
        match self {
            Step::Matched(c) => c.len,
            Step::Unmatched { len, .. } => *len,
        }
    }
}

/// The cheapest way found so far to reach one end position: its total cost,
/// the position it was reached from, and the step that closed the gap.
struct Node {
    cost: i32,
    from: usize,
    step: Step,
}

/// The cost of reaching `pos`, or `None` if `pos` is not yet reachable.
///
/// Position 0 is reachable for free by definition — it is the start of the
/// sentence, not the result of any step — so it is handled outside `best`
/// rather than needing a sentinel `Step` to seed the array with.
fn cost_at(best: &[Option<Node>], pos: usize) -> Option<i32> {
    if pos == 0 {
        Some(0)
    } else {
        best[pos].as_ref().map(|node| node.cost)
    }
}

/// Offer one way to reach `to` from `from` at `step_cost` on top of whatever
/// it costs to already be at `from`. Updates `best[to]` if this route is
/// better, and does nothing if `from` itself isn't reachable.
///
/// The single relaxation point the module doc promises: Phase 2's k-best
/// swaps `best`'s element type for a bounded list of `Node`s and changes only
/// this function's body, not its callers.
///
/// Ties are broken toward the *longer* run: `unmatched_cost` prices a run by
/// length plus a same-script-transition surcharge, so a same-script gap like
/// `」、「` costs identically whether relaxed as three one-character runs or
/// one three-character run reaching the same endpoint at the same total cost
/// — a genuine tie, not a bug. Breaking it toward the longer run collapses
/// such a run to a single `Step::Unmatched` instead of leaving it to fragment
/// arbitrarily on iteration order. A mixed-script run still splits at its
/// script change on its own: that path is strictly *cheaper* (no surcharge
/// paid), not tied, so this tie-break never overrides it.
fn relax(best: &mut [Option<Node>], from: usize, to: usize, step_cost: i32, step: Step) {
    let Some(from_cost) = cost_at(best, from) else {
        return;
    };
    let total = from_cost + step_cost;
    let better = match &best[to] {
        None => true,
        Some(existing) if total < existing.cost => true,
        Some(existing) if total == existing.cost => step.len() > existing.step.len(),
        Some(_) => false,
    };
    if better {
        best[to] = Some(Node { cost: total, from, step });
    }
}

/// The min-cost contiguous cover of `chars`, choosing among `by_start`'s
/// candidates (indexed by start position, as [`lex::candidates`] produces
/// them) under the boundaries `bounds` allows.
///
/// A forbidden candidate (`is_allowed` false) is never relaxed, so it can
/// never appear in the returned path regardless of price. The result covers
/// `0..chars.len()` exactly once, in ascending order, with no gaps or
/// overlaps — every position up to [`MAX_UNMATCHED_RUN`] ahead of any
/// reachable position is itself reachable via an unmatched run, so position
/// `chars.len()` is always reached and the backtrack below never runs off
/// the front.
pub fn best_path(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
) -> Vec<Step> {
    let n = chars.len();
    if n == 0 {
        return Vec::new();
    }

    let mut best: Vec<Option<Node>> = (0..=n).map(|_| None).collect();

    for (from, cands) in by_start.iter().enumerate().take(n) {
        if cost_at(&best, from).is_none() {
            continue;
        }

        for cand in cands {
            if !is_allowed(cand, bounds) {
                continue;
            }
            let to = cand.start + cand.len;
            let step_cost = cost(cand, bounds);
            relax(&mut best, from, to, step_cost, Step::Matched(cand.clone()));
        }

        let max_to = (from + MAX_UNMATCHED_RUN).min(n);
        for to in (from + 1)..=max_to {
            let len = to - from;
            let step_cost = unmatched_cost(chars, from, len);
            relax(&mut best, from, to, step_cost, Step::Unmatched { start: from, len });
        }
    }

    let mut path = Vec::new();
    let mut pos = n;
    while pos != 0 {
        let node = best[pos].take().unwrap_or_else(|| {
            unreachable!(
                "position {pos} unreachable: every position is within \
                 MAX_UNMATCHED_RUN of the one before it, so an unmatched run \
                 alone always closes the gap"
            )
        });
        pos = node.from;
        path.push(node.step);
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::WordFlags;

    fn c(start: usize, len: usize, id: u32, flags: WordFlags) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: id,
            flags,
            steps: Vec::new(),
            exact: true,
            headword: String::new(),
        }
    }

    fn by_start(n: usize, cands: Vec<Candidate>) -> Vec<Vec<Candidate>> {
        let mut v = vec![Vec::new(); n];
        for cand in cands {
            v[cand.start].push(cand);
        }
        v
    }

    #[test]
    fn the_path_covers_the_input_exactly_once() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let path = best_path(&chars, &by_start(5, vec![c(0, 2, 1, WordFlags::PRIMARY)]), None);
        let mut at = 0;
        for step in &path {
            let (s, l) = match step {
                Step::Matched(x) => (x.start, x.len),
                Step::Unmatched { start, len } => (*start, *len),
            };
            assert_eq!(s, at);
            at += l;
        }
        assert_eq!(at, 5);
    }

    #[test]
    fn unmatched_characters_become_one_run_not_many() {
        let chars: Vec<char> = "」、「".chars().collect();
        let path = best_path(&chars, &by_start(3, vec![]), None);
        assert_eq!(path.len(), 1, "one run, not three single characters");
    }

    #[test]
    fn a_forbidden_candidate_is_never_taken() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool {
                self.0.contains(&p)
            }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        // A candidate 京都 at 1..3 starts inside the 東京 token.
        let path = best_path(
            &chars,
            &by_start(3, vec![c(1, 2, 9, WordFlags::PRIMARY)]),
            Some(&At(&[0, 2, 3])),
        );
        assert!(!path.iter().any(|s| matches!(s, Step::Matched(x) if x.entry_id == 9)));
    }

    #[test]
    fn a_compound_may_merge_across_a_boundary_and_win() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool {
                self.0.contains(&p)
            }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        let mut compound_flags = WordFlags::PRIMARY;
        compound_flags.insert(WordFlags::COMMON);
        // Deviates from the brief's literal fixture in one field, beyond the
        // sanctioned BitOr amendment: `東京` below is `exact: false`, not
        // `true`. With every candidate exact, cost.rs's own numbers make this
        // test unwinnable for the compound by construction, not by any DP
        // defect -- CROSS (4) always outweighs COMMON's (-2) discount for a
        // single crossing, so a same-exactness, boundary-aligned split always
        // undercuts an otherwise-identical merge that has to cross that same
        // boundary. Verified directly: cost(compound)=5 vs cost(東京)+cost(都)
        // =2+1=3 with every candidate exact. Marking one split half inexact
        // (a plausible real case -- it only matched via kana folding) gives
        // the split its own +8 INEXACT charge, so the compound's exact match
        // can win *net* while still genuinely paying CROSS -- the capability
        // this test exists to demonstrate. Flagged in the Task 10 report.
        let inexact_split_half = Candidate {
            start: 0,
            len: 2,
            entry_id: 2,
            flags: WordFlags::PRIMARY,
            steps: Vec::new(),
            exact: false,
            headword: String::new(),
        };
        let path = best_path(
            &chars,
            &by_start(
                3,
                vec![
                    c(0, 3, 1, compound_flags),
                    inexact_split_half,
                    c(2, 1, 3, WordFlags::PRIMARY),
                ],
            ),
            Some(&At(&[0, 2, 3])),
        );
        assert!(
            path.iter().any(|s| matches!(s, Step::Matched(x) if x.entry_id == 1)),
            "東京都 must be able to pay the crossing price and win"
        );
    }

    #[test]
    fn the_empty_input_yields_an_empty_path() {
        assert!(best_path(&[], &[], None).is_empty());
    }
}
