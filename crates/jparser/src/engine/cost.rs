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

//! The cost model: which span of the input a candidate should be priced at,
//! and whether a tokenizer's boundaries allow it at all.
//!
//! `is_allowed` and `cost` are deliberately separate. A span either may or
//! may not exist (`is_allowed`); among the spans that may, `cost` says which
//! the DP should prefer. Master plan §4.3's asymmetry lives in that split: a
//! span may never start or end inside a tokenizer token, but merging two
//! tokens into one span is always allowed and only ever priced.

use crate::engine::kana;
use crate::engine::lex::Candidate;
use crate::engine::Boundaries;
use crate::record::WordFlags;

/// Cost of one covered character in a matched span.
///
/// Per character, not per match. A per-match constant is what makes `はこう`
/// (one match) beat `は`+`こう` (two) — master plan §4.3 cause 2 — and the
/// interior constraint does not forbid that merge, because `は` and `こう`
/// are separate tokens and merging across a boundary is always permitted
/// (§4.3's asymmetry, the rule that saves `東京都`). Removing the bias
/// structurally is the only form of the fix that satisfies success criterion 3.
const COVER: i32 = 1;

/// Cost of one character in an unmatched run.
///
/// Far above COVER, so a fallback run never outbids a dictionary reading.
/// Handoff §9.1 measured the opposite failure: 36 of 266 lost gold boundaries
/// were runs of punctuation lumped into one unmatched segment (`」、`, `、「喜`),
/// the one unambiguous defect in that decomposition.
const UNMATCHED: i32 = 30;

/// Extra cost for each script transition inside a single unmatched run.
///
/// `UNMATCHED` alone prices a run only by its length, so a mixed run like
/// `丨FIFA` costs exactly the same whether it is kept whole or split at its
/// kanji/roman boundary — nothing about a per-character price prefers the
/// split. Handoff §9.1's 36-of-266 count is runs of exactly this shape
/// (`」、`, `、「喜`) collapsed into one unmatched segment; this surcharge is
/// what makes leaving a script change inside one span cost more than
/// breaking there, which a flat per-character price cannot do on its own.
/// Priced well under `UNMATCHED` so it only ever tips a tie between
/// equal-length partitions, never competes with `UNMATCHED` itself; the exact
/// magnitude is Task 16's to tune against the corpus, per master plan §3.5.
const SCRIPT_TRANSITION: i32 = 10;

/// Cost of crossing one tokenizer boundary.
///
/// Priced, never forbidden: master plan §4.3's asymmetry requires `東京都` to
/// merge across the `東京|都` boundary and win. Tuned in Task 16 against the
/// corpus, with `東京都` and `音がする` as the merges that must survive it.
const CROSS: i32 = 4;

/// Extra cost, per boundary crossed, when a PRONOUNCE-only match crosses a
/// tokenizer boundary.
///
/// Handoff §9.5's ~28 spurious merges are every one of them a match against a
/// kanji headword's *reading*: `はいた`→履く via はく, `ももう`→揉む via もむ,
/// `がい`→甲斐, `してい`→指定, `どうし`→同士, `ことし`→今年, `になって`→荷なう.
/// Every legitimate merge in §9.1 is kanji-bearing: `音がする`, `ことになっている`,
/// `首を振る`, `これまでに`, `東京都`, `2月`. A kana-only word is PRIMARY by that
/// flag's own definition, so `は`, `こう` and `もう` are not caught.
///
/// Handoff §9.6's refusal to add a heuristic to the *metric* is unaffected:
/// there the object is a merged span the corpus carries no label for. Here the
/// flag is already computed.
const PRONOUNCE_CROSS: i32 = 25;

/// Discount for a match whose entry carries a priority marker.
///
/// A frequency signal JMdict supplies directly, and the only evidence
/// available for choosing between two equally-covering readings.
const COMMON: i32 = -2;

/// Penalty for a match whose source characters differ from the stored surface
/// before folding.
///
/// Every §9.5 absurdity is inexact against its headword; every §9.1
/// legitimate merge is exact modulo conjugation. Independent of
/// PRONOUNCE_CROSS and measured separately in Task 16.
const INEXACT: i32 = 8;

/// Whether a tokenizer allows `cand`'s span to exist at all.
///
/// A span may not *start or end* inside a token: `bounds` must call both
/// `cand.start` and `cand.start + cand.len` boundaries. Nothing forbids a
/// span from *crossing* boundaries in between — that is `cost`'s job, not
/// this one — which is the asymmetry that lets `東京都` survive a tokenizer
/// that cuts it as `東京|都`.
///
/// With `bounds == None` every constraint lifts: the engine must still work
/// with no tokenizer at all.
pub fn is_allowed(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> bool {
    let Some(bounds) = bounds else {
        return true;
    };
    bounds.is_boundary(cand.start) && bounds.is_boundary(cand.start + cand.len)
}

/// The price of matching `cand`, given where a tokenizer put its boundaries.
///
/// Sums `COVER` per character, `INEXACT` for a folded-not-exact match,
/// `COMMON` for a priority entry, and `CROSS` for each interior tokenizer
/// boundary the span swallows, with `PRONOUNCE_CROSS` added on top of `CROSS`
/// per boundary when the match is PRONOUNCE-only. With `bounds == None` there
/// is nothing to cross, so only the per-character and per-match terms apply.
pub fn cost(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> i32 {
    let mut total = COVER * cand.len as i32;

    if !cand.exact {
        total += INEXACT;
    }
    if cand.flags.contains(WordFlags::COMMON) {
        total += COMMON;
    }

    if let Some(bounds) = bounds {
        // Interior positions of `start..start+len`: every `p` strictly
        // between the span's own two endpoints. `start` and `start+len` are
        // excluded because crossing *those* is `is_allowed`'s concern, not a
        // price this span pays for merely existing.
        let crossings = ((cand.start + 1)..(cand.start + cand.len))
            .filter(|&p| bounds.is_boundary(p))
            .count() as i32;
        total += crossings * CROSS;
        if cand.flags.contains(WordFlags::PRONOUNCE) {
            total += crossings * PRONOUNCE_CROSS;
        }
    }

    total
}

/// A character's script class, for `unmatched_cost`'s transition surcharge.
#[derive(PartialEq, Eq)]
enum Script {
    Kanji,
    Kana,
    Digit,
    Other,
}

fn classify(c: char) -> Script {
    if kana::is_kanji(c) {
        Script::Kanji
    } else if kana::is_kana(c) {
        Script::Kana
    } else if kana::is_digit(c) {
        Script::Digit
    } else {
        Script::Other
    }
}

/// The price of leaving `chars[start..start+len]` unmatched.
///
/// `UNMATCHED` per character, plus `SCRIPT_TRANSITION` for each adjacent pair
/// inside the run whose script class differs — the surcharge that prefers
/// breaking a mixed run like `丨FIFA` at its script change over lumping it
/// into one unmatched segment.
pub fn unmatched_cost(chars: &[char], start: usize, len: usize) -> i32 {
    let mut total = UNMATCHED * len as i32;
    for pair in chars[start..start + len].windows(2) {
        if classify(pair[0]) != classify(pair[1]) {
            total += SCRIPT_TRANSITION;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    struct At(&'static [usize]);
    impl crate::engine::Boundaries for At {
        fn is_boundary(&self, pos: usize) -> bool {
            self.0.contains(&pos)
        }
    }

    fn cand(start: usize, len: usize, flags: WordFlags, exact: bool) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: 1,
            flags,
            steps: Vec::new(),
            exact,
            headword: String::new(),
        }
    }

    #[test]
    fn coverage_cost_does_not_reward_using_fewer_matches() {
        // The §4.3 fix, as an equality. Without it `はこう` wins by arithmetic.
        let whole = cost(&cand(4, 3, WordFlags::PRIMARY, true), None);
        let split = cost(&cand(4, 1, WordFlags::PRIMARY, true), None)
            + cost(&cand(5, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(whole, split);
    }

    #[test]
    fn crossing_a_boundary_costs_something_but_is_allowed() {
        let b = At(&[0, 4, 5, 7]);
        let c = cand(4, 3, WordFlags::PRIMARY, true);
        assert!(
            is_allowed(&c, Some(&b)),
            "merging across a boundary is legal"
        );
        assert!(cost(&c, Some(&b)) > cost(&c, None));
    }

    #[test]
    fn a_pronounce_match_that_crosses_is_priced_far_above_a_primary_one() {
        // はいた as 履く against a real は + いた.
        let b = At(&[0, 1, 3]);
        let reading = cost(&cand(0, 3, WordFlags::PRONOUNCE, true), Some(&b));
        let kanji = cost(&cand(0, 3, WordFlags::PRIMARY, true), Some(&b));
        assert!(reading > kanji + 20, "reading {reading} kanji {kanji}");
    }

    #[test]
    fn a_kana_only_word_is_not_caught_by_the_pronounce_term() {
        // は, こう and もう are PRIMARY by that flag's own definition — the
        // hiragana of a kana-only entry — so the PRONOUNCE term must not fire
        // on them. If it ever did, the term would penalise ordinary particles
        // and make the engine worse rather than better.
        let b = At(&[0, 1, 3]);
        let kana_only = cand(0, 3, WordFlags::PRIMARY, true);
        let crossing = cost(&kana_only, Some(&b));
        let not_crossing = cost(&kana_only, None);
        assert_eq!(
            crossing - not_crossing,
            CROSS,
            "a PRIMARY span pays only the plain crossing price"
        );

        let reading = cand(0, 3, WordFlags::PRONOUNCE, true);
        assert_eq!(
            cost(&reading, Some(&b)) - crossing,
            PRONOUNCE_CROSS,
            "and the PRONOUNCE variant of the same span pays exactly the extra"
        );
    }

    #[test]
    fn a_span_starting_or_ending_inside_a_token_is_forbidden() {
        let b = At(&[0, 2, 3]);
        assert!(!is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), Some(&b)));
        assert!(!is_allowed(&cand(0, 1, WordFlags::PRIMARY, true), Some(&b)));
        assert!(is_allowed(&cand(0, 2, WordFlags::PRIMARY, true), Some(&b)));
    }

    #[test]
    fn every_constraint_lifts_when_there_is_no_tokenizer() {
        assert!(is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), None));
    }

    #[test]
    fn an_unmatched_character_costs_far_more_than_a_matched_one() {
        let chars: Vec<char> = "」、".chars().collect();
        assert!(
            unmatched_cost(&chars, 0, 2) > cost(&cand(0, 2, WordFlags::PRIMARY, true), None) * 10
        );
    }
}
