// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! The k best paths over the candidate lattice, in the rank order [`k_best`]
//! defines: given every candidate span
//! [`lex::candidates`](crate::engine::lex::candidates) produced and the
//! price [`cost`](crate::engine::cost) puts on each, choose the contiguous
//! covers of the sentence that rank best.
//!
//! A forward DP over end positions `0..=chars.len()`, relaxed in the single
//! place [`relax`], holding a bounded list of distinct readings per position
//! (Phase 2 design §3). [`best_path`] is the k = 1 case and is what the
//! engine produced before Phase 2, node for node.

use std::collections::BTreeSet;

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
/// gap takes. Both halves of that are **measured, 2026-09-09**. On real text
/// the bound is nowhere near binding: sweeping it down over 2,000 characters
/// against the full index, every value from 4 to 64 gives byte-identical
/// output and 2 is the first that diverges, so the longest run the text needs
/// is 3 characters and 64 is twenty-one times that. On a deliberately
/// pathological input -- a 200-character single-script run of `x` between two
/// real Japanese fragments -- every value changes the output instead, which
/// is this comment's own claim seen directly: an all-one-script gap costs
/// `UNMATCHED` per character however it is split, so the hop count, and with
/// it the number of unmatched segments the caller sees, is exactly what this
/// constant decides. Coverage is invariant; the segmentation of a
/// pathological gap is not, and never was.
const MAX_UNMATCHED_RUN: usize = 64;

/// Raw readings the DP keeps per position for every reading `k_best` is
/// asked for. `undominated` runs after the backtrack and can only remove
/// readings, so the DP over-provisions: each matched span of a reading has a
/// variant that merely leaves it unmatched, about `UNMATCHED - COVER` dearer
/// and never a reading in its own right.
///
/// ponytail: fixed factor. A top-1 with more than `3 * k` matched spans has
/// more such variants than the raw list has spare slots, and a genuine
/// alternate priced above them (a PRONOUNCE crossing plus INEXACT, about
/// +33) can fall off the end; the upgrade path is dominance pruning inside
/// the lists, which is safe because domination at a position survives every
/// extension.
const RAW_FACTOR: usize = 4;

/// One step of a segmentation: either a dictionary match, or a run of input
/// no candidate covered.
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

/// One reading reaching an end position: what it cost, which reading at
/// `from` it extends, the step that closed the gap, and its identity.
struct Node {
    cost: i32,
    from: usize,
    /// Index into `best[from]` of the reading this one extends. Position 0
    /// has no list -- it is the start of the sentence -- so a node with
    /// `from == 0` ignores this.
    from_rank: usize,
    step: Step,
    /// The reading's identity: a hash of every matched `(start, len)` on the
    /// path so far, in position order. Unmatched steps leave it unchanged,
    /// so two paths that differ only in how they split an unmatched run, or
    /// in which same-span entry they carry, are one reading and share a
    /// slot. A collision would merge two genuinely different readings; at
    /// tens of spans and 64 bits that is a non-event, and the failure is a
    /// missing alternate, never a wrong segment.
    key: u64,
}

/// `key` extended by `step`: unchanged for an unmatched run; for a match,
/// the span mixed in through a splitmix64 finaliser so the result depends on
/// the whole set of spans and not on the last one alone.
fn extend_key(key: u64, step: &Step) -> u64 {
    let Step::Matched(c) = step else {
        return key;
    };
    let span = ((c.start as u64) << 32) | c.len as u64;
    let mut x = (key ^ span).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The k = 1 tie-break as an ordering: cheaper first, then the longer last
/// step first. A full tie is decided by insertion order -- `insert` places a
/// new node after every node it does not strictly beat -- which is what
/// `Some(_) => false` did when there was one slot: the same-script gap
/// `」、「` costs the same as one run or three, and the longer run wins so it
/// never fragments on iteration order, while a mixed-script run still splits
/// at its script change because that path is strictly cheaper.
fn beats(cost: i32, len: usize, other: &Node) -> bool {
    cost < other.cost || (cost == other.cost && len > other.step.len())
}

/// Whether a node of `cost`, last-step `len` and `key` would enter `list`
/// under `cap`: not if a node with the same key is already there and is not
/// beaten, and not if it would land at or past `cap`. Split from `insert` so
/// `relax` clones a `Step` only for nodes that are actually kept.
fn accepts(list: &[Node], cost: i32, len: usize, key: u64, cap: usize) -> bool {
    if let Some(same) = list.iter().find(|n| n.key == key) {
        if !beats(cost, len, same) {
            return false;
        }
    }
    let at = list
        .iter()
        .position(|n| beats(cost, len, n))
        .unwrap_or(list.len());
    at < cap
}

/// Put `node` in rank order, replacing a same-key node it beats, and drop
/// whatever falls past `cap`. Callers check `accepts` first.
fn insert(list: &mut Vec<Node>, node: Node, cap: usize) {
    if let Some(i) = list.iter().position(|n| n.key == node.key) {
        list.remove(i);
    }
    let at = list
        .iter()
        .position(|n| beats(node.cost, node.step.len(), n))
        .unwrap_or(list.len());
    list.insert(at, node);
    list.truncate(cap);
}

/// Offer every reading at `from` one way to reach `to`: `step`, at
/// `step_cost` on top of what that reading already cost.
///
/// The single relaxation point the module doc promises. Position 0 is the
/// start of the sentence and holds one implicit reading at cost 0 with an
/// empty key. Any other position offers each node of its list in rank order,
/// so on a full tie the extension of a higher-ranked reading is inserted
/// first and stays ahead -- which is what keeps rank 0 at every position
/// equal to the single winner the k = 1 DP produced.
fn relax(best: &mut [Vec<Node>], from: usize, to: usize, step_cost: i32, step: &Step, cap: usize) {
    // `from < to` always, so `from` is in the left half and `to` heads the right.
    let (before, rest) = best.split_at_mut(to);
    let dst = &mut rest[0];
    let len = step.len();
    let mut offer = |from_rank: usize, from_cost: i32, from_key: u64| {
        let cost = from_cost + step_cost;
        let key = extend_key(from_key, step);
        if accepts(dst.as_slice(), cost, len, key, cap) {
            insert(
                dst,
                Node {
                    cost,
                    from,
                    from_rank,
                    step: step.clone(),
                    key,
                },
                cap,
            );
        }
    };
    if from == 0 {
        offer(0, 0, 0);
    } else {
        for (rank, node) in before[from].iter().enumerate() {
            offer(rank, node.cost, node.key);
        }
    }
}

/// The path that ends as `best[end][rank]`, walked back through
/// `(from, from_rank)` to position 0.
fn path_of(best: &[Vec<Node>], end: usize, rank: usize) -> Vec<Step> {
    let mut path = Vec::new();
    let (mut pos, mut rank) = (end, rank);
    while pos != 0 {
        let node = &best[pos][rank];
        path.push(node.step.clone());
        (pos, rank) = (node.from, node.from_rank);
    }
    path.reverse();
    path
}

fn matched_spans(path: &[Step]) -> Vec<(usize, usize)> {
    path.iter()
        .filter_map(|s| match s {
            Step::Matched(c) => Some((c.start, c.len)),
            Step::Unmatched { .. } => None,
        })
        .collect()
}

/// The first `bound` of `readings` (already in cost order) that no cheaper
/// kept reading dominates. A reading is dominated when another, cheaper one
/// matches every span it matches: it only leaves words unmatched that the
/// other found, a worse copy of that reading rather than a different one.
/// Rank 0 is never dominated. Checking against the kept list alone is
/// enough, because every dominator of a dropped reading is cheaper than it
/// and so was either kept or dominated in turn by something kept.
///
/// The kept list is built in cost order and stops when full, so its first
/// `n` entries are the same for every `bound >= n`: a larger bound extends
/// the list and never disturbs its prefix. `k_best` relies on that to pass
/// the whole raw pool's bound here and cut to `k` only after `demote`.
fn undominated(readings: Vec<Vec<Step>>, bound: usize) -> Vec<Vec<Step>> {
    // No `Vec::with_capacity(bound)`: `bound` derives from a caller-controlled
    // `k` (see `k_best`) and must not let a huge request pre-allocate an
    // absurd amount up front.
    let mut kept: Vec<Vec<Step>> = Vec::new();
    let mut kept_spans: Vec<Vec<(usize, usize)>> = Vec::new();
    for reading in readings {
        if kept.len() == bound {
            break;
        }
        let spans = matched_spans(&reading);
        let dominated = kept_spans
            .iter()
            .any(|other| spans.iter().all(|s| other.contains(s)));
        if dominated {
            continue;
        }
        kept_spans.push(spans);
        kept.push(reading);
    }
    kept
}

/// Where a reading cuts: 0, then the end of every step. A path is a
/// contiguous cover from 0, so the step ends alone determine it.
fn cuts(path: &[Step]) -> BTreeSet<usize> {
    let mut at = 0;
    let mut set = BTreeSet::from([0]);
    for step in path {
        at += step.len();
        set.insert(at);
    }
    set
}

/// `pool`, in cost order, reordered so that a reading merge/split equivalent
/// to an earlier *fresh* reading sinks below every reading that is not, then
/// cut to `k`. Two readings are merge/split equivalent when one's cut set is
/// contained in the other's -- `東京都` at {0,3} against `東京`+`都` at
/// {0,2,3} -- in either direction, because merge and split are one relation
/// read from opposite ends (Phase 2b design §3.1; the asymmetric variant was
/// measured and loses, Phase 2 handoff §7).
///
/// Rank 0 meets an empty fresh list, so it is always fresh and always first:
/// at k = 1 this is the identity, which is what keeps `best_path` and the
/// eight cells untouched by construction. A reading is compared against the
/// fresh list only, never against sunk ones -- otherwise one sunk reading
/// would drag unrelated readings down behind it. Containment in either
/// direction is not transitive, so this is a greedy sweep whose result
/// depends on the order it sweeps; cost order is the only order there is,
/// and rank 0 is the reading the engine chose.
///
/// Nothing is deleted. A sunk reading past `k` is not returned, as any
/// reading past `k` never was; what changes is which readings the cut
/// keeps.
fn demote(pool: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>> {
    // No `Vec::with_capacity(k)`, for `undominated`'s reason.
    let mut fresh: Vec<Vec<Step>> = Vec::new();
    let mut fresh_cuts: Vec<BTreeSet<usize>> = Vec::new();
    let mut sunk: Vec<Vec<Step>> = Vec::new();
    for reading in pool {
        let c = cuts(&reading);
        if fresh_cuts.iter().any(|f| f.is_subset(&c) || c.is_subset(f)) {
            sunk.push(reading);
        } else {
            fresh_cuts.push(c);
            fresh.push(reading);
        }
    }
    fresh.extend(sunk);
    fresh.truncate(k);
    fresh
}

/// The `k` best distinct contiguous covers of `chars`, choosing among
/// `by_start`'s candidates under the boundaries `bounds` allows. Distinct
/// means by the set of spans matched (see `Node::key`), and a reading that
/// only un-matches what a cheaper one matches is not offered (see
/// `undominated`). Rank 0 is the cheapest; below it the order is cost order
/// among readings not merge/split equivalent to an earlier one, then the
/// equivalent ones in cost order (see `demote`) -- so a re-split of the top
/// reading does not spend a rank a genuinely different reading could hold.
/// Always at least one path; empty input yields exactly one empty path.
///
/// `by_start[i]` is every candidate starting at character offset `i`, the
/// shape [`lex::candidates`] produces; a position past the end of a short
/// `by_start` is treated as having no candidates. A forbidden candidate
/// (`is_allowed` false) is never relaxed, so it appears in no reading. Every
/// position up to [`MAX_UNMATCHED_RUN`] ahead of a reachable one is itself
/// reachable via an unmatched run, so position `chars.len()` always holds at
/// least one node and `path_of` never runs off the front.
///
/// Rank 0 is exactly what the single-slot DP produced: same comparator
/// (`beats`), extensions of a position's rank-0 reading inserted before its
/// lower ranks', and every lower-ranked extension at least as dear as the
/// rank-0 extension with the same step.
///
/// The `3 * k`-of-`4 * k` guarantee is on the raw pool, not the returned
/// order: whenever fewer than `3 * k` of the `4 * k` cheapest raw readings
/// are dominated, no genuine alternate falls off the raw list before
/// `undominated` and `demote` ever see it; above that threshold one can (see
/// [`RAW_FACTOR`]). What comes back is `demote`'s rank order over the
/// undominated pool, truncated to `k` -- the `k` best readings in that order,
/// not the `k` cheapest undominated ones. Rank 0 is still exactly the
/// cheapest. Fewer than `k` readings come back when the lattice holds fewer
/// distinct undominated covers -- 35 of the 150 gold-corpus sentences do,
/// measured at a cap eight times the shipped one.
///
/// That ceiling was measured before `demote`, not merely bounded: at k = 3
/// the shipped cap of 12 returned byte-identical top-three lists to a cap of
/// 96 on all 150 gold-corpus sentences in both hint states, because with
/// `CROSS = 0` the interesting alternates are exact-cost ties while the
/// one-word-unmatched variants they compete with cost about +30. With
/// `demote` the top three depend on the whole pool, not its first three, so
/// that measurement no longer covers them; the Phase 2b handoff §2 records
/// what the pool at this cap reaches.
pub fn k_best(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
    k: usize,
) -> Vec<Vec<Step>> {
    // Clamped rather than trusted to the caller: `ParseOptions::with_readings`
    // clamps too, but this function is also reachable directly, and a `k`
    // near `usize::MAX` would overflow `k * RAW_FACTOR` below.
    let k = k.clamp(1, crate::MAX_READINGS);
    let n = chars.len();
    if n == 0 {
        return vec![Vec::new()];
    }
    // Rank 0 is never dominated (see `undominated`), so at k = 1 a single
    // slot is exactly the pre-Phase-2 single-winner DP -- with `cap == 1`,
    // `accepts` reduces to the old `better` match arms. The `RAW_FACTOR`
    // over-provision only earns its keep once there is more than one rank to
    // protect from domination.
    let cap = if k == 1 { 1 } else { k * RAW_FACTOR };
    let mut best: Vec<Vec<Node>> = (0..=n).map(|_| Vec::new()).collect();

    for from in 0..n {
        if from != 0 && best[from].is_empty() {
            continue;
        }
        let cands = by_start.get(from).map(Vec::as_slice).unwrap_or(&[]);

        for cand in cands {
            if !is_allowed(cand, bounds) {
                continue;
            }
            let to = cand.start + cand.len;
            let step_cost = cost(cand, bounds);
            relax(
                &mut best,
                from,
                to,
                step_cost,
                &Step::Matched(cand.clone()),
                cap,
            );
        }

        let max_to = (from + MAX_UNMATCHED_RUN).min(n);
        for to in (from + 1)..=max_to {
            let len = to - from;
            let step_cost = unmatched_cost(chars, from, len);
            relax(
                &mut best,
                from,
                to,
                step_cost,
                &Step::Unmatched { start: from, len },
                cap,
            );
        }
    }

    let raw: Vec<Vec<Step>> = (0..best[n].len())
        .map(|rank| path_of(&best, n, rank))
        .collect();
    // Filter the whole raw pool, not the first `k`: `demote` can only free a
    // rank if it runs before the list is cut (Phase 2b design §3.2).
    demote(undominated(raw, cap), k)
}

/// The min-cost contiguous cover of `chars`: [`k_best`] at k = 1.
pub fn best_path(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
) -> Vec<Step> {
    k_best(chars, by_start, bounds, 1).swap_remove(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::flags::WordFlags;

    fn c(start: usize, len: usize, id: u32, flags: WordFlags) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: id,
            flags,
            steps: Vec::new(),
            exact: true,
            all_katakana: false,
            function_stack: false,
            grammatical_stack: false,
            digit_counter: false,
            after_digit_noncounter: false,
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
        let path = best_path(
            &chars,
            &by_start(5, vec![c(0, 2, 1, WordFlags::PRIMARY)]),
            None,
        );
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
        assert!(!path
            .iter()
            .any(|s| matches!(s, Step::Matched(x) if x.entry_id == 9)));
    }

    // Master plan §4.3 requires a compound to be able to pay the crossing
    // price and still win -- that is the reason this DP prices a boundary
    // crossing instead of forbidding it. At the original constants (CROSS=4,
    // flat COMMON=-2) that was impossible for *any* compound: merging cost
    // COVER (coverage-neutral) plus CROSS=4 per boundary crossed, and
    // COMMON=-2 flat was the only discount a merge could earn once, never
    // enough to pay off even one crossing.
    //
    // Task 16b's fix, measured against the 2,163-span gold corpus (see
    // `cost.rs`'s constant comments and handoff §10): COMMON became
    // length-proportional (`COMMON * cand.len`), which ties a compound
    // against its equally-evidenced parts on the COVER+COMMON terms alone,
    // and CROSS came down to 0 -- not a magnitude tweak but a full removal,
    // because the gold corpus's real compounds overwhelmingly tie their
    // split alternative exactly, so any positive CROSS priced them out
    // categorically rather than merely taxing them (CROSS=1 collapsed
    // hints-on tolerant accuracy from 0.8521 to 0.8049 on the corpus).
    // Below, 東京都 (COMMON) costs `3 - 2*3 + 0 = -3` against the split's
    // 東京(PRIMARY only, `2`) + 都(PRIMARY only, `1`) = `3` -- a clean win
    // here because only the compound carries COMMON, not a tie needing the
    // tie-break. On the real gold corpus most surviving merges instead tie
    // exactly on COVER+COMMON (both sides equally evidenced) and are decided
    // by `relax`'s longer-run tie-break -- see handoff §10's trial
    // log for why CROSS had to reach exactly 0, not merely fall, for those
    // to resolve in the compound's favour. `PRONOUNCE_CROSS` (unchanged at
    // 25) still keeps はこう-shaped spurious PRONOUNCE merges out even with
    // CROSS=0, since it applies independently per crossing.
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
        let path = best_path(
            &chars,
            &by_start(
                3,
                vec![
                    c(0, 3, 1, compound_flags),
                    c(0, 2, 2, WordFlags::PRIMARY),
                    c(2, 1, 3, WordFlags::PRIMARY),
                ],
            ),
            Some(&At(&[0, 2, 3])),
        );
        assert!(
            path.iter()
                .any(|s| matches!(s, Step::Matched(x) if x.entry_id == 1)),
            "東京都 must be able to pay the crossing price and win"
        );
    }

    #[test]
    fn the_empty_input_yields_an_empty_path() {
        assert!(best_path(&[], &[], None).is_empty());
    }

    #[test]
    fn k_best_with_k_one_is_best_path() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let cands = by_start(
            5,
            vec![
                c(0, 2, 1, WordFlags::PRIMARY),
                c(2, 3, 2, WordFlags::PRIMARY),
            ],
        );
        assert_eq!(
            k_best(&chars, &cands, None, 1),
            vec![best_path(&chars, &cands, None)]
        );
    }

    #[test]
    fn readings_come_out_distinct_and_in_cost_order() {
        // あいう as one word, or あい + う. Nothing else matches, and every
        // path that merely un-matches one of those words is dominated.
        let chars: Vec<char> = "あいう".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // Phase 2b's demotion leaves this order alone, and not by luck: あい+う
        // cuts at {0,2,3}, a superset of あいう's {0,3}, so it is a split of
        // the top reading and sinks -- and being the only sunk reading here,
        // its position among the two survivors is unchanged.
        // Equal cost (3 each); the longer last step wins the tie, as at k = 1.
        assert_eq!(spans, vec![vec![(0, 3)], vec![(0, 2), (2, 1)]]);
    }

    #[test]
    fn two_entries_at_one_span_are_one_reading() {
        let chars: Vec<char> = "あ".chars().collect();
        let cands = by_start(
            1,
            vec![
                c(0, 1, 1, WordFlags::PRIMARY),
                c(0, 1, 2, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        assert_eq!(
            readings.len(),
            1,
            "the second entry is a sibling, not a reading"
        );
        assert_eq!(matched_spans(&readings[0]), vec![(0, 1)]);
    }

    #[test]
    fn an_unmatched_run_yields_no_fragment_variants() {
        let chars: Vec<char> = "」、「".chars().collect();
        let readings = k_best(&chars, &by_start(3, vec![]), None, 3);
        assert_eq!(
            readings.len(),
            1,
            "every split of the run is the same reading"
        );
        assert_eq!(readings[0].len(), 1, "and it is kept as one run");
    }

    #[test]
    fn a_reading_that_only_unmatches_a_word_is_dropped() {
        let chars: Vec<char> = "あい".chars().collect();
        let cands = by_start(
            2,
            vec![
                c(0, 1, 1, WordFlags::PRIMARY),
                c(1, 1, 2, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        assert_eq!(
            readings.len(),
            1,
            "あ+い is the only reading; leaving either unmatched is a worse copy of it"
        );
    }

    #[test]
    fn a_reading_that_leaves_an_unmatchable_character_unmatched_is_kept() {
        // あい, or いう -- each leaves the other's spare character unmatched,
        // and neither matches everything the other does.
        let chars: Vec<char> = "あいう".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 2, 1, WordFlags::PRIMARY),
                c(1, 2, 2, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // Both cost 32; いう closes the sentence with a longer step than う.
        assert_eq!(spans, vec![vec![(1, 2)], vec![(0, 2)]]);
    }

    #[test]
    fn a_forbidden_candidate_appears_in_no_reading() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool {
                self.0.contains(&p)
            }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(1, 2, 9, WordFlags::PRIMARY), // 京都 starts inside the 東京 token
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, Some(&At(&[0, 2, 3])), 3);
        assert!(readings.len() >= 2);
        for path in &readings {
            assert!(!path
                .iter()
                .any(|s| matches!(s, Step::Matched(x) if x.entry_id == 9)));
        }
    }

    #[test]
    fn every_reading_covers_the_input_exactly_once() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let cands = by_start(
            5,
            vec![
                c(0, 2, 1, WordFlags::PRIMARY),
                c(0, 3, 2, WordFlags::PRIMARY),
                c(3, 2, 3, WordFlags::PRIMARY),
                c(2, 3, 4, WordFlags::PRIMARY),
            ],
        );
        for path in k_best(&chars, &cands, None, 3) {
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
    }

    #[test]
    fn the_empty_input_yields_one_empty_reading() {
        assert_eq!(k_best(&[], &[], None, 3), vec![Vec::<Step>::new()]);
    }

    #[test]
    fn a_huge_k_neither_panics_nor_overflows() {
        // `k * RAW_FACTOR` would overflow `usize` before the clamp; the
        // caller-facing route (`jparser-cli score --readings`) has no upper
        // bound of its own, so this function must hold the line alone.
        let chars: Vec<char> = "あい".chars().collect();
        let cands = by_start(
            2,
            vec![
                c(0, 1, 1, WordFlags::PRIMARY),
                c(1, 1, 2, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, usize::MAX / 2 + 1);
        assert!(!readings.is_empty());
        assert!(readings.len() <= crate::MAX_READINGS);
    }
    // Phase 2b (design §3): rank is cost order among readings not merge/split
    // equivalent to an earlier kept one, then the equivalent ones in cost
    // order. Two readings are equivalent when one's cut set is contained in
    // the other's, in either direction.

    /// 東京都市 over five candidates: 東京 and 都市 COMMON, 東, 京 and 京都市
    /// plain. Exactly three undominated readings, at distinct costs, so no
    /// assertion below rides on a tie-break: 東京+都市 at (2-4)+(2-4) = -4,
    /// 東+京+都市 at 1+1+(2-4) = 0, 東+京都市 at 1+3 = 4. Every other cover
    /// leaves a word unmatched that one of these three matches, and is
    /// dropped by `undominated` before `demote` sees it.
    fn tokyo_toshi() -> (Vec<char>, Vec<Vec<Candidate>>) {
        let mut common = WordFlags::PRIMARY;
        common.insert(WordFlags::COMMON);
        let chars: Vec<char> = "東京都市".chars().collect();
        let cands = by_start(
            4,
            vec![
                c(0, 2, 1, common),             // 東京
                c(2, 2, 2, common),             // 都市
                c(0, 1, 3, WordFlags::PRIMARY), // 東
                c(1, 1, 4, WordFlags::PRIMARY), // 京
                c(1, 3, 5, WordFlags::PRIMARY), // 京都市
            ],
        );
        (chars, cands)
    }

    #[test]
    fn a_reading_unrelated_to_rank_zero_outranks_a_re_split_of_it() {
        let (chars, cands) = tokyo_toshi();
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // Cost order is 東京+都市, 東+京+都市, 東+京都市. The middle one cuts
        // at {0,1,2,4}, a superset of rank 0's {0,2,4} -- the same reading
        // re-split -- so it sinks. 東+京都市 cuts at {0,1,4}, comparable with
        // rank 0's set in neither direction, so it is fresh and now rank 1.
        //
        // {0,1,4} *is* a subset of the sunk reading's {0,1,2,4}. Were readings
        // compared against sunk ones too, 東+京都市 would sink behind it; it
        // is compared against the fresh list only, and does not. A refactor
        // that compares against every earlier reading fails here.
        assert_eq!(
            spans,
            vec![
                vec![(0, 2), (2, 2)],
                vec![(0, 1), (1, 3)],
                vec![(0, 1), (1, 1), (2, 2)]
            ]
        );
    }

    #[test]
    fn demotion_frees_a_rank_the_cut_would_have_spent() {
        // The point of the phase: at k = 2 the re-split no longer occupies the
        // second slot, so a reading the old cut never returned now does.
        let (chars, cands) = tokyo_toshi();
        let readings = k_best(&chars, &cands, None, 2);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        assert_eq!(spans, vec![vec![(0, 2), (2, 2)], vec![(0, 1), (1, 3)]]);
    }

    #[test]
    fn demotion_never_moves_rank_zero() {
        let (chars, cands) = tokyo_toshi();
        assert_eq!(
            k_best(&chars, &cands, None, 3)[0],
            best_path(&chars, &cands, None)
        );
    }

    #[test]
    fn readings_all_equivalent_to_rank_zero_keep_their_cost_order() {
        // 東京都 as one word, or 東京+都, or 東+京都. Every full cover cuts at
        // a superset of the single word's {0,3}, so every alternative sinks,
        // and sinking together they keep the order they arrived in: demotion
        // reorders between the fresh group and the sunk group, never inside
        // either. 東+京都 and 東京+都 cut at {0,1,3} and {0,2,3}, comparable
        // with neither each other -- the relation is not transitive, and the
        // sweep does not pretend it is.
        let chars: Vec<char> = "東京都".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
                c(0, 1, 4, WordFlags::PRIMARY),
                c(1, 2, 5, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // All three cost 3; the longer last step wins each tie, as at k = 1.
        assert_eq!(
            spans,
            vec![vec![(0, 3)], vec![(0, 1), (1, 2)], vec![(0, 2), (2, 1)]]
        );
    }

    #[test]
    fn cuts_are_zero_and_every_step_end() {
        let path = vec![
            Step::Matched(c(0, 2, 1, WordFlags::PRIMARY)),
            Step::Unmatched { start: 2, len: 1 },
            Step::Matched(c(3, 2, 2, WordFlags::PRIMARY)),
        ];
        assert_eq!(cuts(&path), BTreeSet::from([0, 2, 3, 5]));
        assert_eq!(cuts(&[]), BTreeSet::from([0]));
    }
}
