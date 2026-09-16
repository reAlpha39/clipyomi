// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

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
//! That phase implemented the latter two only, and this paragraph used to end
//! "not implemented, not forgotten". **The particle term shipped after all**:
//! `FUNCTION_STACK` below *is* a particle-in-context term. `lex` computes
//! `Candidate::function_stack` by asking whether a span partitions into
//! candidates that each carry `COMMON` and `PARTICLE`, and `cost` prices
//! exactly that. The narrow claim that nothing in *this module* reads the flag
//! is still true, and only because the flag is read one module upstream on
//! this term's behalf. `assemble::order` reads it too, as of 2026-09-16.

use crate::engine::flags::WordFlags;
use crate::engine::kana;
use crate::engine::lex::Candidate;
use crate::engine::Boundaries;

/// Cost of using a match at all, charged once per span regardless of length.
///
/// **Master plan §4.3 removed a constant of this shape and this reintroduces
/// it, at a magnitude the guards it lacked can hold.** The original made
/// `はこう` (one match) beat `は` + `こう` (two), because a per-match price
/// always favours fewer, longer spans. Since then the engine gained
/// `PRONOUNCE_CROSS`, `is_allowed`'s boundary constraint, `FUNCTION_STACK`,
/// and `spurious_merges.txt` as a live 26-row gate — so the question was
/// worth re-asking with a measurement rather than assuming the old answer.
///
/// **Why it is needed.** Every other term is per character, and `COMMON` is
/// -2 against `COVER`'s +1, so a COMMON span costs -1 a character and a
/// non-COMMON one +1: *a tiling of COMMON fragments beats a longer
/// non-COMMON dictionary word at every magnitude*. Nothing per-character can
/// fix that, and the effect is not occasional — it is the dominant ordering.
/// On `ある日起きたら、英国海軍第一卿になって...` the engine ranked the cover
/// that keeps all four of `ある日`, `起きたら`, `第一` and `なって` whole
/// **eighteenth**, behind every cover that refuses at least one of them.
///
/// **Why 2.** Swept against the gold corpus, hints on, with the compound
/// metric's class lines read rather than its headline:
///
/// | M | strict | compounds | lexical | false-match | transparent | merge gate |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | 0 | 1825 | 114 | 42 | 30 / 32 | 42 / 42 | 26 / 26 |
/// | **2** | **1823** | **118** | **46** | **30 / 32** | **42 / 42** | **26 / 26** |
/// | 4 | 1744 | 106 | — | — | — | 26 / 26 |
/// | 6 | 1694 | 123 | 75 | 27 / 32 | 30 / 42 | 25 / 26 |
///
/// 6 is where `ある日` is finally kept whole, and it is refused: its compound
/// headline rises on the `lexical` class while `transparent` collapses 42 to
/// 30 and `false-match` 30 to 27 — it merges twelve compounds that *should*
/// split. §5.2 reports those classes separately for exactly this reason, and
/// the headline alone would have hidden it. 2 gains 4 compounds and breaks
/// neither split class.
///
/// **What it costs and buys.** Hints on, 2 spans (1825 -> 1823); hints off,
/// it *gains* 11 (1588 -> 1599). Compounds 114 -> 118 and 109 -> 112. Entry
/// top-1 up in both states. And it fixes `ミスった`, which the Phase 4 design
/// scoped out as a ranking defect its own change could not reach.
///
/// A symmetric `PARTICLE_CROSS` was built and measured alongside this — a
/// per-crossing surcharge for a PARTICLE match, the one shape neither
/// `PRONOUNCE_CROSS` nor `FUNCTION_STACK` catches (`それを` as "even so").
/// It is needed at M >= 6 and **not** at 2, where it reads -2 strict for +1
/// compound, so it is not here.
const MATCH: i32 = 2;

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
/// **The sentence that used to follow was wrong twice and is corrected here
/// (2026-09-16).** It read "every legitimate merge in §9.1 is kanji-bearing:
/// `音がする`, `ことになっている`, `首を振る`, `これまでに`, `東京都`, `2月`" —
/// but `ことになっている` and `これまでに` carry no kanji at all, so two of its
/// own six examples falsify it, and `東京都` is not in §9.1 (the handoff's list
/// is `音がした`, `ことになっている`, `首を振った`, `これまでに`, `天と地`, `2月`
/// and `18日`). The generalisation was never load-bearing. **What actually
/// keeps this term off ordinary particles is the flag, not the script:** a
/// kana-only word is PRIMARY by `WordFlags`' own definition, so `は`, `こう`
/// and `もう` are not caught whatever they are made of.
///
/// Handoff §9.6's refusal to add a heuristic to the *metric* is unaffected:
/// there the object is a merged span the corpus carries no label for. Here the
/// flag is already computed.
///
/// Task 16b measured this term's magnitude as flat across the whole safe
/// range: 10, 5, 2 and 1 (trials 5, 7, 8, 9) all reproduced the
/// `spurious_merges.txt` fixture's exact 4-of-26 failures, no different from
/// the original 25. **"Exact corpus numbers" overstated the trial log and is
/// corrected here (2026-09-16):** only trial 5 reproduced trial 2 cell for
/// cell; trial 7 read 0.8035 (1738) / 0.8525 (1844) and trial 9 0.8040 (1739)
/// / 0.8530 (1845), and trial 8 recorded no corpus figures at all. The
/// handoff's own word was "measurement-identical", which is the honest one:
/// the spread is three spans across a 25-fold change in magnitude. Only the sign matters here,
/// not the size: trial 6 (`PRONOUNCE_CROSS = 0`) broke 6 additional fixture
/// rows (`はいた`, `はこう`, `ももう`, `になって`, `になった`, `してい`) for a
/// corpus tolerant gain (0.8521 → 0.8682 hints-on) now attributable to that
/// same spurious class reappearing, not to real compounds -- see
/// handoff §10. Left at the original value since nothing measured
/// argues for changing it, and it carries a larger margin against any
/// spurious-merge shape this corpus and fixture do not already cover.
///
/// **The 4-of-26 those trials held constant were an artefact of the fixture's
/// subject, not of this term (2026-09-09).** The test measured each surface in
/// isolation, and given two bare characters Vibrato calls `はこ`, `うん` and
/// `んで` one token each -- so the crossing count below, which looks strictly
/// *between* a span's endpoints, found nothing to price and this term could
/// not fire at any magnitude. That is why every trial in the range measured
/// identically on those rows. Re-pointed at the gold sentence each surface
/// occurs in, the fixture passes 26 of 26 and the test is no longer ignored;
/// the flatness finding above stands, since it was measured on the corpus as
/// well.
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

/// Penalty for a match whose span is nothing but stacked common function
/// words -- `には` where `に` + `は` are both there.
///
/// A tie-breaker, not a tax. `には` as one COMMON entry of length 2 costs
/// `COVER*2 + COMMON*2 = -2`; `に` + `は` as two COMMON entries of length 1
/// cost `(1-2) + (1-2) = -2` as well, and `lattice::relax`'s longer-run
/// tie-break then hands the tie to the merge. Any positive value here hands
/// it back to the split, which is why this is flat rather than
/// length-proportional: `COMMON`'s proportionality exists to stay neutral
/// across lengths (see its comment), while this term is aimed at one shape.
///
/// Measured 2026-09-09 before it was written, off-engine, over the 139
/// adjudicated compound types (`examples/derivation_probe.rs`): the property
/// holds for 7 of 22 `transparent` types, 3 of 23 `false-match`, and 2 of 87
/// `lexical`. Cross-referenced against what the engine does today, that is
/// **+12 windows** it can fix (`には` in 8 sentences, `にも` in 4 -- every one
/// of the `transparent` class's misses) against **-3** it breaks (`として` in
/// 2, `となり` in 1, all three currently kept whole).
///
/// Unusually for a Phase 3 candidate, the two span metrics point the same
/// way: the short-unit gold corpus splits all four surfaces, so the merges
/// this blocks currently destroy 30 gold spans (16 + 8 + 4 + 2) that strict
/// accuracy scores as misses today. Frequency could not manage that -- doc
/// §10.3 priced it at about -6.9 points strict for the compound gain -- and
/// it is why this term was worth trying where a blanket COMMON derivation
/// (doc §10.4) was not.
///
/// **What it measured on-engine 2026-09-09, hints on — these are the cells as
/// they stood that day, and the hints-on half has since moved twice (Phase 4's
/// escape and `order`'s two new clauses): strict is 1792, tolerant 1870 and
/// entry top-1 0.9236 today, while the hints-off half below is still exact
/// because neither change touched that state. Master plan §5.2 carries the
/// current figures.** strict 1737 -> 1768 spans
/// (+31), tolerant 1843 -> 1844 (+1), entry top-1 0.9063 -> 0.9092,
/// compounds 101 -> 114 windows, `transparent` 30 -> 42 of 42 with
/// `lexical` 41 -> 42 and `false-match` 30 -> 30 -- no class regressed.
/// Hints off: strict 1573 -> 1588, compounds 96 -> 109, tolerant 1725 ->
/// 1714 (-11, against 75 -> 62 merges, so it is coarsening the tolerant
/// metric was crediting; §5.2 records that metric as over-forgiving by about
/// 1.3 points for exactly this reason). Latency unmoved: `bench`'s scaling
/// cell reads 1.10x hints on against 1.12x before.
///
/// **Magnitude is flat from 1 to 50 in both hint states** -- identical cells
/// throughout -- so only the sign matters here, as with `PRONOUNCE_CROSS`
/// above. 4 is the small end: this term suppresses a *shape* that overlaps a
/// legitimate class, so unlike `PRONOUNCE_CROSS` there is no argument for
/// extra margin, and less of any lattice this corpus does not cover is
/// perturbed.
///
/// **The `steps.is_empty()` gate below is load-bearing and was measured, not
/// assumed.** Ungated, this term shattered `なりたいです` into `なり` +
/// `たいです` and `して` into `し` + `て`, because `なり` and `し` are
/// themselves COMMON particle entries -- the §4.3 shattering class this whole
/// rewrite exists to remove, and a worse defect than the merge it cures. The
/// gold corpus's own metrics *improved* through that damage; the curated
/// `parse_sentences.txt` snapshot is what caught it. With the gate the
/// snapshot is byte-identical.
const FUNCTION_STACK: i32 = 4;

/// Discount for a span that opens with a number and whose reading carries the
/// sound change that number makes — `1週間` as `いっしゅうかん`.
///
/// **Why it needs a term at all.** Number and counter are one phonological
/// word in Japanese and JMdict records the merged reading, but the merged
/// entry is usually `[primary, common-line]` while the counter alone is
/// `COMMON` — so `週間` earns −2 a character and `１週間` earns nothing, and
/// the split wins by 2. The reader is then shown `ichi shuukan` for a word
/// pronounced `isshuukan`: a mispronunciation, not merely a different cut.
///
/// **Why it is gated on the reading actually changing**, in
/// [`lex::Candidate::digit_counter`]: that is exactly where the merge earns
/// its keep. `一週間` reads `いっしゅうかん` and `四人` `よにん`, which
/// `いち` + `しゅうかん` and `よん` + `にん` cannot produce, so splitting
/// mispronounces them. `三回` reads `さんかい` and `二番` `にばん`, which is
/// what the split already says. Measured over 2,743 lines of real text, an
/// **ungated** discount re-cut `３|回目` as `３回|目` **49 times**, breaking a
/// genuine counter suffix to fix readings that were never wrong. Gated, 129
/// spans change and they are euphonic merges — `１週間` 62, `２人` 16 (`ふたり`,
/// not `に` + `にん`), `1週間` 15, `1ヶ月` 4.
///
/// **Magnitude.** −3 is the smallest that flips the 2-point gap above; −5 and
/// −8 measured identically on every cell, so only the sign and clearing the
/// gap matter. Gold is untouched in both hint states — every cell and every
/// compound class line identical. `1969年` is unaffected: an all-digit span
/// fails the "continues past it" half of the shape test, which is what keeps
/// years out.
///
/// **What it does not reach.** `8人目` stays `8 | 人目`, because `はちにん` is
/// exactly `はち` + `にん` and the gate correctly declines — so the same
/// construction is fixed at 4 and not at 8. The underlying fault there is
/// that `人目` is a different word (`ひとめ`, "public gaze") winning after a
/// digit, which is an entry-selection problem rather than a merge one, and
/// JMdict's `COUNTER` flag cannot separate it: `週間`, `時間` and `匹` are
/// counters and carry no flag.
const DIGIT_COUNTER: i32 = -3;

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
/// `cand.start` and `cand.start + cand.len` usable endpoints. Nothing forbids
/// a span from *crossing* boundaries in between — that is `cost`'s job, not
/// this one — which is the asymmetry that lets `東京都` survive a tokenizer
/// that cuts it as `東京|都`.
///
/// The question is [`Boundaries::is_usable_endpoint`] and not `is_boundary`,
/// and the two differ in exactly one place: a token that no candidate can
/// cover *and* that carries a hiragana character has no legal span at all
/// under the strict reading, so [`crate::engine::relax::Relaxed`] reopens its
/// interior (Phase 4 design §2 and §9). Both halves are load-bearing — an
/// all-kanji, all-katakana or all-digit token nothing covers is a proper noun
/// or a number, and stays shut. Everywhere the tokenizer and the dictionary
/// agree about where words are, the two questions have the same answer.
///
/// With `bounds == None` every constraint lifts: the engine must still work
/// with no tokenizer at all.
pub fn is_allowed(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> bool {
    let Some(bounds) = bounds else {
        return true;
    };
    bounds.is_usable_endpoint(cand.start) && bounds.is_usable_endpoint(cand.start + cand.len)
}

/// The price of matching `cand`, given where a tokenizer put its boundaries.
///
/// Sums `COVER` per character, `INEXACT` for a folded-not-exact match,
/// `COMMON` per character for a priority entry, `FUNCTION_STACK` for an
/// unconjugated stack of common function words, and `CROSS` for each interior
/// tokenizer boundary the span swallows, with `PRONOUNCE_CROSS` added on top
/// of `CROSS` per boundary when the match is PRONOUNCE-only. With
/// `bounds == None` there is nothing to cross, so only the per-character and
/// per-match terms apply.
pub fn cost(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> i32 {
    let mut total = COVER * cand.len as i32 + MATCH;

    if !cand.exact {
        total += INEXACT;
    }
    if cand.flags.contains(WordFlags::COMMON) {
        total += COMMON * cand.len as i32;
    }
    // Gated on an unconjugated match. A conjugated form is not function-word
    // stacking, and without this gate the term shatters exactly the class
    // §4.3 exists to protect: measured, `なりたいです` (`なる` Want-to Polite,
    // one segment) split into `なり` + `たいです`, and `して` into `し` + `て`,
    // because `なり` and `し` are themselves COMMON particle entries. Both are
    // in `parse_sentences.txt`, so the curated snapshot caught what the gold
    // corpus's own metrics did not -- they improved through it.
    if cand.function_stack && cand.steps.is_empty() {
        total += FUNCTION_STACK;
    }
    if cand.digit_counter {
        total += DIGIT_COUNTER;
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
        // A crossing strictly inside the characters the deinflection chain
        // consumed is the tokenizer cutting an *inflection*, not proposing a
        // word boundary, so it is not evidence of a spurious merge and pays
        // nothing. Everything else pays in full. The range is the chain's as a
        // whole, not each step's: `なった` decomposes into two steps, [1,2) for
        // the っ and [2,3) for the た, and IPADIC's cut lands exactly on the
        // junction between them -- inside the inflection, but inside neither
        // step. See `PRONOUNCE_CROSS`'s comment for the measurement.
        if cand.flags.contains(WordFlags::PRONOUNCE) {
            let chain = cand
                .steps
                .iter()
                .fold(None, |acc: Option<(usize, usize)>, s| {
                    Some(match acc {
                        None => (s.range.start, s.range.end),
                        Some((a, b)) => (a.min(s.range.start), b.max(s.range.end)),
                    })
                });
            let charged = ((cand.start + 1)..(cand.start + cand.len))
                .filter(|&p| bounds.is_boundary(p))
                .filter(|&p| !matches!(chain, Some((a, b)) if a < p && p < b))
                .count() as i32;
            total += charged * PRONOUNCE_CROSS;
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
    use crate::engine::ConjStep;

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
            function_stack: false,
            digit_counter: false,
            headword: String::new(),
        }
    }

    #[test]
    fn a_function_stack_pays_only_while_it_is_unconjugated() {
        // The gate `FUNCTION_STACK`'s comment calls load-bearing, as an
        // arithmetic contract rather than as a corpus number. Ungated the
        // same term shattered `なりたいです`, because a conjugated match whose
        // span happens to decompose into common particles is not stacking.
        let plain = cand(0, 2, WordFlags::COMMON, true);
        let mut stacked = plain.clone();
        stacked.function_stack = true;
        let mut conjugated = stacked.clone();
        conjugated.steps = vec![ConjStep {
            class: 0,
            form: 0,
            range: 0..2,
            aux: None,
        }];

        assert_eq!(
            cost(&stacked, None) - cost(&plain, None),
            FUNCTION_STACK,
            "an unconjugated function stack pays the term exactly once"
        );
        assert_eq!(
            cost(&conjugated, None),
            cost(&plain, None),
            "a conjugated match pays nothing, however its span decomposes"
        );
    }

    /// `MATCH` is charged once per span, whatever its length — that is the
    /// whole point, and it is what makes a four-character headword able to
    /// beat a stem plus its tail. The companion below pins the *other* half:
    /// per-character coverage must stay neutral, so the two terms together
    /// prefer the longer match without ever rewarding a merge that covers
    /// more ground than its parts.
    #[test]
    fn the_match_cost_is_flat_and_the_longer_span_wins_on_it() {
        let one = cost(&cand(0, 4, WordFlags::PRIMARY, true), None);
        let two = cost(&cand(0, 2, WordFlags::PRIMARY, true), None)
            + cost(&cand(2, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(
            two - one,
            MATCH,
            "two matches pay it twice, one pays it once"
        );
        assert!(one < two, "so the whole span is preferred to the split");
    }

    #[test]
    fn coverage_cost_does_not_reward_using_fewer_matches() {
        // §4.3's fix, which used to be a plain equality: `whole == split`,
        // because a per-match price is what made `はこう` beat `は` + `こう`.
        // `MATCH` reintroduced that price deliberately and at a measured
        // magnitude, so the equality now holds **once the per-match term is
        // taken out** -- which is the property §4.3 actually needed. `COVER`
        // still contributes nothing to the preference between a span and its
        // parts; only `MATCH` does, and only by its own size.
        let whole = cost(&cand(4, 3, WordFlags::PRIMARY, true), None);
        let split = cost(&cand(4, 1, WordFlags::PRIMARY, true), None)
            + cost(&cand(5, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(
            whole - MATCH,
            split - 2 * MATCH,
            "per-character cost is flat"
        );
        assert_eq!(split - whole, MATCH, "and the only difference is one match");
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
        assert!(
            cost(&c, Some(&b)) >= cost(&c, None),
            "never a discount for crossing"
        );
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
    /// OCR-noise shape Phase 0 handoff §9.1 names -- not §7.2, which carries only
    /// the cost formula) prices splitting at the script change
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
