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
//!
//! Design §7.2's cost formula also names a `particle-in-context` evidence
//! term, alongside `common` (`COMMON` below) and `exact` (`INEXACT` below).
//! This phase implements the latter two only: `WordFlags::PARTICLE` is
//! carried on records but nothing in this module reads it, so a particle's
//! surrounding context prices no differently than any other match. Not
//! implemented, not forgotten.

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
///
/// Task 16b considered this constant and left it untouched: `COVER`'s
/// per-character shape is what the other terms' own length-proportionality
/// (`COMMON`, this task's own fix) is built to stay neutral against, and no
/// trial found evidence bearing on its magnitude specifically.
const COVER: i32 = 1;

/// Cost of one character in an unmatched run.
///
/// Far above COVER, so a fallback run never outbids a dictionary reading.
/// Handoff §9.1 measured the opposite failure: 36 of 266 lost gold boundaries
/// were runs of punctuation lumped into one unmatched segment (`」、`, `、「喜`),
/// the one unambiguous defect in that decomposition.
///
/// Task 16b considered this constant and left it untouched: no trial in this
/// task's cost-model tuning pass touched split-vs-merge decisions between two
/// *matched* candidates, which is where every measured defect lived, so
/// nothing bore on `UNMATCHED`'s own magnitude. It remains exactly the
/// Task 9-era value, unmeasured against the gold corpus.
const UNMATCHED: i32 = 30;

/// Extra cost for each script transition inside a single unmatched run.
///
/// `UNMATCHED` alone prices a run only by its length, so a mixed run like
/// `丨FIFA` costs exactly the same whether it is kept whole or split at its
/// kanji/roman boundary — nothing about a per-character price prefers the
/// split. This surcharge is what makes leaving a script change inside one
/// span cost more than breaking there, which a flat per-character price
/// cannot do on its own (see `an_unmatched_run_prefers_splitting_at_its_
/// script_change` below).
///
/// Whole-branch review correction: an earlier version of this comment cited
/// handoff §9.1's `」、`/`、「喜` punctuation runs as this term's measured
/// motivation. `classify` below has no punctuation class — `」`, `、` and
/// `「` all classify as `Script::Other` — so a pure-punctuation pair like
/// `」、` earns no surcharge whole or split; the citation was false and is
/// removed rather than repeated. Priced well under `UNMATCHED` so it only
/// ever tips a tie between equal-length partitions, never competes with
/// `UNMATCHED` itself. Task 16b (the tuning pass, split from the original
/// Task 16) found no corpus evidence bearing on this term either way -- the
/// two diagnosed defects (master plan §3.5, handoff §9) were both in
/// `COMMON`/`CROSS`, not here — so it was left untouched. Unlike `CROSS`
/// and `PRONOUNCE_CROSS`, no corpus measurement backs this specific value
/// — chosen for sign and rough magnitude only, and still open for a future
/// pass with evidence to spend.
const SCRIPT_TRANSITION: i32 = 10;

/// Cost of crossing one tokenizer boundary.
///
/// Priced, never forbidden: master plan §4.3's asymmetry requires `東京都` to
/// merge across the `東京|都` boundary and win. Task 16b measured this down
/// from 4 to 0 against the 2,163-span gold corpus (trials 2 and 11): with
/// `COMMON` length-proportional (below), a real compound overwhelmingly ties
/// its split alternative exactly on `COVER`+`COMMON` alone, so any positive
/// `CROSS` disqualified it outright rather than merely taxing it -- trial 11
/// (`CROSS = 1`) collapsed hints-on span tolerant from 0.8521 to 0.8049 and
/// the merge count from 53 to 19. At `CROSS = 0` the tie is real, and
/// `lattice::relax`'s longer-run tie-break resolves it toward the compound.
/// `PRONOUNCE_CROSS` (below) is what still keeps a spurious PRONOUNCE-only
/// crossing (`はこう`-shaped) out even though this term no longer taxes it.
const CROSS: i32 = 0;

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
///
/// Task 16b measured this term's magnitude as flat across the whole safe
/// range: 10, 5, 2 and 1 (trials 5, 7, 8, 9) all reproduced trial 2's exact
/// corpus numbers *and* the `spurious_merges.txt` fixture's exact 4-of-26
/// failures, no different from the original 25. Only the sign matters here,
/// not the size: trial 6 (`PRONOUNCE_CROSS = 0`) broke 6 additional fixture
/// rows (`はいた`, `はこう`, `ももう`, `になって`, `になった`, `してい`) for a
/// corpus tolerant gain (0.8521 → 0.8682 hints-on) now attributable to that
/// same spurious class reappearing, not to real compounds -- see
/// handoff §10. Left at the original value since nothing measured
/// argues for changing it, and it carries a larger margin against any
/// spurious-merge shape this corpus and fixture do not already cover.
const PRONOUNCE_CROSS: i32 = 25;

/// Discount per character for a match whose entry carries a priority marker.
///
/// A frequency signal JMdict supplies directly, and the only evidence
/// available for choosing between two equally-covering readings.
///
/// Task 16b trial 1: changed from a flat per-match `-2` (applied once
/// regardless of length) to `COMMON * cand.len` in `cost` (per character).
/// The flat form is exactly master plan §4.3 cause 2's bug: 東京 as one
/// COMMON match cost `2 - 2 = 0`, but 東(COMMON) + 京(COMMON) as two COMMON
/// matches cost `(1-2) + (1-2) = -2`, so the split always won. Scaled by
/// length, both cost `-2` -- a tie `lattice::relax`'s longer-run tie-break
/// resolves toward the compound. Measured against the 2,163-span gold
/// corpus: hints-off span tolerant rose from 0.6015 (1301/2163) to 0.7975
/// (1725/2163) and hints-off strict, which was not expected to move, rose
/// too (0.5895 → 0.7272) -- both directions confirm this length-scaling was
/// blocking whole-word matches from shattering into common single-character
/// fragments, not only multi-word compounds.
///
/// Magnitude was then tested separately: trial 3 (`-1`) reproduced trial 1's
/// numbers exactly on this corpus, and trial 4 (`0`, disabling the term
/// entirely) *regressed* hints-off strict below the v1 baseline (0.6657 vs
/// baseline's 0.6810) while only partially recovering tolerant, because
/// removing all evidence let some genuinely wrong merges win the length
/// tie-break too. Kept at `-2`: the sign and length-proportionality are what
/// the corpus rewards, not the specific magnitude (handoff §10's trial log
/// has both directions).
///
/// What this term structurally *cannot* fix, at any magnitude: an entry like
/// 東京都 (ent_seq 1447740 in `gold_jmdict_subset.xml`) that carries no
/// `ke_pri`/`re_pri` of its own while its parts (東, 京都) individually do.
/// The split's aggregate COMMON discount is then strictly larger than the
/// compound's zero, by an amount proportional to whatever `COMMON` is set
/// to -- the sign of that comparison does not depend on the magnitude. This
/// is JMdict's own priority-tagging gap (a well-known place name is simply
/// untagged), not something this cost term has the information to see; it
/// is part of why hints-off/on tolerant still trail baseline after tuning.
const COMMON: i32 = -2;

/// Penalty for a match whose source characters differ from the stored surface
/// before folding.
///
/// Every §9.5 absurdity is inexact against its headword; every §9.1
/// legitimate merge is exact modulo conjugation. Independent of
/// PRONOUNCE_CROSS.
///
/// Task 16b trial 10 halved this to 4 and measured a small regression on
/// both span metrics hints-off (strict 1573→1572, tolerant 1725→1722 spans),
/// not the gain the flat-per-match reasoning that motivated `COMMON`'s
/// change would predict for this term too (a flat penalty already
/// *disadvantages* merges relative to splits, the opposite bias from
/// `COMMON`, so halving it should have made merges relatively more
/// attractive -- it did not move the corpus meaningfully in that direction).
/// Reverted to 8; nothing measured argues for changing it.
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
/// `COMMON` per character for a priority entry, and `CROSS` for each interior
/// tokenizer boundary the span swallows, with `PRONOUNCE_CROSS` added on top
/// of `CROSS` per boundary when the match is PRONOUNCE-only. With
/// `bounds == None` there is nothing to cross, so only the per-character and
/// per-match terms apply.
pub fn cost(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> i32 {
    let mut total = COVER * cand.len as i32;

    if !cand.exact {
        total += INEXACT;
    }
    if cand.flags.contains(WordFlags::COMMON) {
        total += COMMON * cand.len as i32;
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
///
/// # Panics
///
/// Panics if `start + len` exceeds `chars.len()`; the segmentation DP is
/// expected to construct only valid ranges.
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
    fn crossing_a_boundary_is_allowed_and_never_a_discount() {
        // Renamed from `..._costs_something_but_is_allowed`: at Task 16b's
        // measured CROSS=0, a PRIMARY match's crossing is cost-*neutral*, not
        // cost-*positive* -- `CROSS` no longer strictly separates the two.
        // Trial 2 (CROSS=4 -> 0) is why: the gold corpus's real compounds
        // overwhelmingly tie their split alternative on COVER+COMMON alone,
        // so any positive CROSS priced them out categorically (trial 11,
        // CROSS=1, collapsed hints-on tolerant from 0.8521 to 0.8049 and
        // merges from 53 to 19) rather than merely taxing them. What must
        // still hold -- and is asserted below -- is that a crossing is never
        // *rewarded*: `is_allowed` still permits it, and `cost` never prices
        // it below the no-crossing case. A PRONOUNCE-flagged crossing paying
        // strictly more is `a_pronounce_match_that_crosses_is_priced_far_above_a_primary_one`'s
        // job, not this test's.
        let b = At(&[0, 4, 5, 7]);
        let c = cand(4, 3, WordFlags::PRIMARY, true);
        assert!(
            is_allowed(&c, Some(&b)),
            "merging across a boundary is legal"
        );
        assert!(cost(&c, Some(&b)) >= cost(&c, None), "never a discount for crossing");
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
    fn pronounce_cross_scales_with_each_boundary_crossed() {
        // Every other test in this file uses a fixture with exactly one
        // interior boundary, so none of them can tell per-crossing scaling
        // apart from a flat one-time surcharge. This pins the current
        // choice explicitly: PRONOUNCE_CROSS is charged once per boundary
        // crossed, same as CROSS itself.
        //
        // The defect class it targets (Handoff §9.5) is entirely
        // single-crossing, so per-crossing scaling does no known harm today.
        // But it does not come free: at two crossings the term alone adds
        // 2 * PRONOUNCE_CROSS = 50, more than UNMATCHED charges for two whole
        // fallback characters, so a legitimate multi-boundary reading-form
        // merge could in principle lose to an unmatched run rather than
        // merely cost more. Not yet seen happening against the corpus, and
        // no task currently owns revisiting it -- see handoff §8's "three
        // size/shape assumptions" bullet.
        let b = At(&[0, 1, 3, 5]);
        let primary = cost(&cand(0, 5, WordFlags::PRIMARY, true), Some(&b));
        let pronounce = cost(&cand(0, 5, WordFlags::PRONOUNCE, true), Some(&b));
        assert_eq!(
            pronounce - primary,
            2 * PRONOUNCE_CROSS,
            "two boundaries crossed, so the PRONOUNCE surcharge applies twice"
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

    /// Design §7.2/`SCRIPT_TRANSITION`'s own worked example: a genuinely
    /// mixed-script run (kanji/CJK-radical followed by roman letters, the
    /// OCR-noise shape §7.2 names) prices splitting at the script change
    /// strictly cheaper than keeping it whole, by exactly `SCRIPT_TRANSITION`
    /// -- the one script pairing `classify` actually distinguishes, unlike
    /// the punctuation-only example the doc comment above no longer cites.
    #[test]
    fn an_unmatched_run_prefers_splitting_at_its_script_change() {
        let chars: Vec<char> = "丨FIFA".chars().collect();
        let whole = unmatched_cost(&chars, 0, chars.len());
        let split = unmatched_cost(&chars, 0, 1) + unmatched_cost(&chars, 1, chars.len() - 1);
        assert_eq!(
            whole - split,
            SCRIPT_TRANSITION,
            "the only difference between keeping `丨FIFA` whole and splitting \
             it at the kanji/roman boundary must be the one script-transition \
             surcharge the whole run pays and the split does not"
        );
    }
}
