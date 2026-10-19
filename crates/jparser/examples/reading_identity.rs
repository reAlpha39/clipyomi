// SPDX-License-Identifier: MIT OR Apache-2.0

//! Would a coarser reading identity move oracle@3? A throwaway spike.
//!
//! Phase 2's gate failed: oracle@3 tolerant hints on reached 1883 of 2163
//! gold spans, +40 over top-1's 1843, against the 55 required. The Phase 2
//! final review diagnosed why and named the lever this file measures.
//!
//! **The diagnosis.** `lattice::k_best` calls two readings distinct when
//! their matched-span *sets* differ. With `cost::CROSS` at 0 a real JMdict
//! compound ties its own split exactly, so `東京都` and `東京`+`都` are two
//! distinct, cost-tied readings competing for the same three slots. The
//! tolerant metric cannot tell them apart -- it forgives either given the
//! other -- so every such pair spends a rank and buys nothing. That is
//! visible in the gate measurement itself: strict oracle@3 gained +109 spans
//! (re-splits count for strict) while tolerant gained only +40.
//!
//! **The lever.** Collapse readings that differ only by a merge or split, so
//! ranks 2 and 3 hold analyses the tolerant metric can actually see. Two
//! readings are in that relation exactly when one's boundary set is a subset
//! of the other's: `東京都` cuts at {0,3} and `東京`+`都` at {0,2,3}, so the
//! first is the second with one boundary removed. Subset in *either*
//! direction collapses, because merge and split are the same relation read
//! from opposite ends.
//!
//! **Why this file changes no shipped code.** To free a rank the filter has
//! to run before the DP truncates to k, so filtering `parse`'s three
//! returned readings could only ever reduce them. Instead this asks the
//! engine for a deeper list and filters that, sweeping the depth
//! ([`DEEPS`]) because every rank above the shipped 3 costs the DP a wider
//! list at every position -- the point of the sweep is the shallowest depth
//! that clears the gate, not the largest gain. `parse` and `k_best` are
//! untouched, so top-1 cannot move and no re-measurement of the eight cells
//! is owed.
//!
//! **The control arm, since `demote` shipped.** `plain` below takes `parse`'s
//! first `KEEP` readings and is printed as the control that must reproduce
//! the recorded gate -- true only while `parse` returns readings in raw cost
//! order. Commit `bfe3a64` made `lattice::k_best` demote before truncating,
//! so at HEAD `parse` already returns a demoted list and `plain` is demoted
//! too, not the pre-Phase-2b order it is labelled as. Every row of the table
//! below changes meaning, not just `demoted`'s. §5 of the Phase 2b design
//! declined a `--no-demote` flag on the strength of this file being the A/B
//! instrument already; that reasoning now needs the same caveat: reproducing
//! the original A/B needs a checkout at or before `bfe3a64`, not a run of
//! this binary at HEAD.
//!
//! **What this does and does not answer.** It answers whether the metric
//! moves. It does not answer whether the collapse is good for a reader:
//! `くるまで` and `くる`+`まで` are tolerant-equivalent and would collapse,
//! yet they are genuinely different analyses someone might want to cycle
//! between. That is a product question this measurement cannot settle, and
//! the report below prints how often the filter fires so the size of that
//! question is visible rather than assumed.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example reading_identity -- \
//!   <index-dir> crates/jparser/tests/fixtures/gold.jsonl \
//!   "$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"
//! ```

#[cfg(not(feature = "mecab"))]
fn main() {
    eprintln!("build with --features mecab");
    std::process::exit(2);
}

#[cfg(feature = "mecab")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    spike::run()
}

#[cfg(feature = "mecab")]
mod spike {
    use std::collections::{BTreeSet, HashMap};
    use std::error::Error;
    use std::path::Path;

    use jparser::engine::Boundaries;
    use jparser::hints::VibratoTokenizer;
    use jparser::index::load::Index;
    use jparser::{parse, ParseOptions, Segment};

    /// Depths swept, shallowest first: how many readings the engine is asked
    /// for before filtering. The last, 64, is twenty times the shipped cap of
    /// 3 and the depth of the full report below; the sweep exists because
    /// every rank above 3 costs the DP a wider list at every position, so the
    /// shallowest depth that clears the gate is the one worth shipping.
    const DEEPS: [usize; 6] = [3, 4, 6, 12, 24, 64];
    /// Readings kept after filtering -- the shipped k, so the number this
    /// prints is comparable to the recorded gate figure.
    const KEEP: usize = 3;

    #[derive(serde::Deserialize)]
    struct GoldSpan {
        start: usize,
        len: usize,
    }

    #[derive(serde::Deserialize)]
    struct GoldRecord {
        text: String,
        spans: Vec<GoldSpan>,
    }

    /// Where a reading cuts: every segment start, plus the end of the last.
    fn cuts(segments: &[Segment]) -> BTreeSet<usize> {
        let mut set: BTreeSet<usize> = segments.iter().map(|s| s.start).collect();
        if let Some(last) = segments.last() {
            set.insert(last.start + last.len);
        }
        set
    }

    /// Whether two readings differ only by merges and splits -- one's cut set
    /// contained in the other's, in either direction.
    fn same_reading(a: &BTreeSet<usize>, b: &BTreeSet<usize>) -> bool {
        a.is_subset(b) || b.is_subset(a)
    }

    /// The first `KEEP` readings no earlier kept reading is merge/split
    /// equivalent to. Rank 0 is always kept: nothing precedes it.
    fn collapse(readings: &[Vec<Segment>]) -> Vec<usize> {
        let mut kept: Vec<usize> = Vec::new();
        let mut kept_cuts: Vec<BTreeSet<usize>> = Vec::new();
        for (i, reading) in readings.iter().enumerate() {
            if kept.len() == KEEP {
                break;
            }
            let c = cuts(reading);
            if kept_cuts.iter().any(|k| same_reading(k, &c)) {
                continue;
            }
            kept_cuts.push(c);
            kept.push(i);
        }
        kept
    }

    /// Every reading, reordered so that a reading which is merge/split
    /// equivalent to an earlier one sinks below every reading that is not.
    /// Cost order is preserved within each group, so rank 0 cannot move:
    /// nothing precedes it, so it is never a duplicate.
    ///
    /// This is the variant worth preferring over [`collapse`]. It puts the
    /// same analyses in the top `KEEP` slots, so the metric sees what the
    /// collapse showed it, but nothing is deleted -- the duplicates survive
    /// further down where a reading-cycling UI could still reach them. The
    /// collapse's +54 is earned by removing readings the tolerant metric is
    /// defined not to see, which is the same shape of error master plan §10
    /// records for compound preservation; demotion has no such cost.
    fn demote(readings: &[Vec<Segment>]) -> Vec<usize> {
        let mut fresh: Vec<usize> = Vec::new();
        let mut dupes: Vec<usize> = Vec::new();
        let mut fresh_cuts: Vec<BTreeSet<usize>> = Vec::new();
        for (i, reading) in readings.iter().enumerate() {
            let c = cuts(reading);
            if fresh_cuts.iter().any(|k| same_reading(k, &c)) {
                dupes.push(i);
            } else {
                fresh_cuts.push(c);
                fresh.push(i);
            }
        }
        fresh.extend(dupes);
        fresh.truncate(KEEP);
        fresh
    }

    /// Like [`demote`], but asymmetric: a reading sinks only when it is a
    /// *refinement* of a kept one -- its cut set a superset, i.e. the same
    /// reading with boundaries added. A coarser reading stays fresh.
    ///
    /// The asymmetry mirrors the tolerant rule's own. Forgiveness runs one
    /// way: a matched segment swallowing gold spans whole at gold boundaries
    /// is credited, a segment finer than the gold span is not. So a
    /// refinement of a kept reading cannot earn tolerant credit the kept one
    /// does not already have, while a *coarser* reading can -- where gold
    /// merges and the kept reading splits, only the coarse alternative is
    /// forgiven. Symmetric demotion sinks those too, which is a loss it need
    /// not take.
    fn demote_refinements(readings: &[Vec<Segment>]) -> Vec<usize> {
        let mut fresh: Vec<usize> = Vec::new();
        let mut dupes: Vec<usize> = Vec::new();
        let mut fresh_cuts: Vec<BTreeSet<usize>> = Vec::new();
        for (i, reading) in readings.iter().enumerate() {
            let c = cuts(reading);
            if fresh_cuts.iter().any(|k| c.is_superset(k)) {
                dupes.push(i);
            } else {
                fresh_cuts.push(c);
                fresh.push(i);
            }
        }
        fresh.extend(dupes);
        fresh.truncate(KEEP);
        fresh
    }

    /// One gold span against one reading, by `score.rs`'s two rules: a
    /// segment sitting exactly on it, or a matched segment swallowing it
    /// whole with both endpoints on gold boundaries.
    fn hits(segments: &[Segment], span: &GoldSpan, bounds: &BTreeSet<usize>) -> (bool, bool) {
        let strict = segments
            .iter()
            .any(|s| s.start == span.start && s.len == span.len);
        if strict {
            return (true, true);
        }
        let forgiven = segments.iter().any(|j| {
            j.matched
                && j.start <= span.start
                && span.start + span.len <= j.start + j.len
                && bounds.contains(&j.start)
                && bounds.contains(&(j.start + j.len))
        });
        (false, forgiven)
    }

    #[derive(Default)]
    struct Tally {
        spans: usize,
        top1_strict: usize,
        top1_tolerant: usize,
        /// oracle@3 over the engine's own first three readings -- reproduces
        /// the recorded gate figure, as a control.
        plain_strict: usize,
        plain_tolerant: usize,
        /// oracle@3 over the first three that survive the collapse.
        filtered_strict: usize,
        filtered_tolerant: usize,
        /// oracle@3 over the first three after demotion, which deletes
        /// nothing. Weakly better than the collapse by construction: where a
        /// sentence has fewer than three non-duplicate readings, demotion
        /// still fills the remaining slots with duplicates, which can add a
        /// strict hit and can never subtract one.
        demoted_strict: usize,
        demoted_tolerant: usize,
        /// oracle@3 after demoting refinements only -- the asymmetric
        /// variant, see `demote_refinements`.
        refined_strict: usize,
        refined_tolerant: usize,
        /// Sentences whose whole gold cover appears as one reading. Cycling
        /// only ever fully fixes a sentence when this fires, so it is the
        /// honest ceiling on the feature rather than on the metric.
        sent_top1: usize,
        sent_plain: usize,
        sent_demoted: usize,
        /// Readings the filter dropped, and sentences where it dropped any.
        dropped: usize,
        sentences_filtered: usize,
        /// Sentences whose kept set reaches deeper than rank 2, i.e. where
        /// the filter actually surfaced a reading the shipped k never saw.
        sentences_reaching_deeper: usize,
        deepest_rank_used: usize,
    }

    fn union(
        segments_list: &[&Vec<Segment>],
        span: &GoldSpan,
        bounds: &BTreeSet<usize>,
    ) -> (bool, bool) {
        let mut strict = false;
        let mut tolerant = false;
        for segments in segments_list {
            let (s, t) = hits(segments, span, bounds);
            strict |= s;
            tolerant |= t;
        }
        (strict, tolerant)
    }

    fn ratio(n: usize, d: usize) -> String {
        format!("{:.4} ({n} / {d})", n as f64 / d as f64)
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let args: Vec<String> = std::env::args().collect();
        let [_, index_dir, gold_path, dic] = args.as_slice() else {
            return Err("usage: reading_identity <index-dir> <gold.jsonl> <system.dic>".into());
        };
        let table = jparser::engine::table::Table::new();
        let index = Index::open(Path::new(index_dir))?;
        let tokenizer = VibratoTokenizer::load(Path::new(dic))?;

        // Last record per text wins, as `corpus::read_gold` does.
        let mut all: Vec<GoldRecord> = Vec::new();
        for line in std::fs::read_to_string(gold_path)?.lines() {
            if !line.trim().is_empty() {
                all.push(serde_json::from_str(line)?);
            }
        }
        let mut last: HashMap<&str, usize> = HashMap::new();
        for (i, r) in all.iter().enumerate() {
            last.insert(&r.text, i);
        }
        let keep: BTreeSet<usize> = last.into_values().collect();
        let gold: Vec<&GoldRecord> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect();
        /// One full pass over the corpus at a given raw-list depth.
        fn measure(
            index: &Index,
            table: &jparser::engine::table::Table,
            tokenizer: &VibratoTokenizer,
            gold: &[&GoldRecord],
            deep: usize,
        ) -> Result<Tally, Box<dyn Error>> {
            let opts = ParseOptions::with_readings(deep);
            let mut t = Tally::default();

            for record in gold {
                let bset = tokenizer.boundaries(&record.text);
                let bounds_dyn: &dyn Boundaries = &bset;
                let parsed = parse(index, table, &record.text, &opts, Some(bounds_dyn))?;

                let readings: Vec<Vec<Segment>> = std::iter::once(parsed.segments.clone())
                    .chain(parsed.alternatives.iter().cloned())
                    .collect();

                let kept = collapse(&readings);
                let ranked = demote(&readings);
                let refined_ranks = demote_refinements(&readings);
                let plain: Vec<&Vec<Segment>> = readings.iter().take(KEEP).collect();
                let filtered: Vec<&Vec<Segment>> = kept.iter().map(|&i| &readings[i]).collect();
                let demoted: Vec<&Vec<Segment>> = ranked.iter().map(|&i| &readings[i]).collect();
                let refined: Vec<&Vec<Segment>> =
                    refined_ranks.iter().map(|&i| &readings[i]).collect();

                let deepest = kept.last().copied().unwrap_or(0);
                t.deepest_rank_used = t.deepest_rank_used.max(deepest);
                if deepest >= KEEP {
                    t.sentences_reaching_deeper += 1;
                }
                let dropped_here = kept
                    .last()
                    .map(|&d| d + 1)
                    .unwrap_or(0)
                    .saturating_sub(kept.len());
                if dropped_here > 0 {
                    t.sentences_filtered += 1;
                    t.dropped += dropped_here;
                }

                let gold_bounds: BTreeSet<usize> = {
                    let mut set: BTreeSet<usize> = record.spans.iter().map(|s| s.start).collect();
                    if let Some(l) = record.spans.last() {
                        set.insert(l.start + l.len);
                    }
                    set
                };

                // Whole-cover match, span for span, the master plan's own
                // definition of oracle@k at the sentence level.
                let covers = |segments: &Vec<Segment>| {
                    segments.len() == record.spans.len()
                        && segments
                            .iter()
                            .zip(&record.spans)
                            .all(|(s, g)| s.start == g.start && s.len == g.len)
                };
                t.sent_top1 += covers(&parsed.segments) as usize;
                t.sent_plain += plain.iter().any(|r| covers(r)) as usize;
                t.sent_demoted += demoted.iter().any(|r| covers(r)) as usize;

                for span in &record.spans {
                    t.spans += 1;
                    let (s, tol) = hits(&parsed.segments, span, &gold_bounds);
                    t.top1_strict += s as usize;
                    t.top1_tolerant += tol as usize;
                    let (ps, pt) = union(&plain, span, &gold_bounds);
                    t.plain_strict += ps as usize;
                    t.plain_tolerant += pt as usize;
                    let (fs, ft) = union(&filtered, span, &gold_bounds);
                    t.filtered_strict += fs as usize;
                    t.filtered_tolerant += ft as usize;
                    let (ds, dt) = union(&demoted, span, &gold_bounds);
                    t.demoted_strict += ds as usize;
                    t.demoted_tolerant += dt as usize;
                    let (rs, rt) = union(&refined, span, &gold_bounds);
                    t.refined_strict += rs as usize;
                    t.refined_tolerant += rt as usize;
                }
            }
            Ok(t)
        }

        let g = |n: usize, base: usize| n as i64 - base as i64;

        // How deep must the engine look for demotion to pay? The shipped k is
        // 3; every row above it costs the DP a wider list at every position.
        println!("       tolerant gain over top-1        strict gain over top-1");
        println!("deep   plain  collaps  demote  refine    plain  collaps  demote  refine");
        let mut deepest = Tally::default();
        for deep in DEEPS {
            let t = measure(&index, &table, &tokenizer, &gold, deep)?;
            println!(
                "{deep:<5}  {:+5}  {:+9}  {:+7}  {:+7}    {:+5}  {:+9}  {:+7}  {:+7}",
                g(t.plain_tolerant, t.top1_tolerant),
                g(t.filtered_tolerant, t.top1_tolerant),
                g(t.demoted_tolerant, t.top1_tolerant),
                g(t.refined_tolerant, t.top1_tolerant),
                g(t.plain_strict, t.top1_strict),
                g(t.filtered_strict, t.top1_strict),
                g(t.demoted_strict, t.top1_strict),
                g(t.refined_strict, t.top1_strict),
            );
            deepest = t;
        }
        println!(
            "
(gate needs tolerant +55)
"
        );

        let t = deepest;
        println!("sentences {}  gold spans {}\n", gold.len(), t.spans);
        println!("                     strict              tolerant");
        println!(
            "top-1            {}   {}",
            ratio(t.top1_strict, t.spans),
            ratio(t.top1_tolerant, t.spans)
        );
        println!(
            "oracle@3 plain    {}   {}   <- control, must match the recorded gate",
            ratio(t.plain_strict, t.spans),
            ratio(t.plain_tolerant, t.spans)
        );
        println!(
            "oracle@3 filtered {}   {}",
            ratio(t.filtered_strict, t.spans),
            ratio(t.filtered_tolerant, t.spans)
        );
        println!(
            "oracle@3 demoted  {}   {}   <- deletes nothing",
            ratio(t.demoted_strict, t.spans),
            ratio(t.demoted_tolerant, t.spans)
        );
        println!(
            "oracle@3 refined  {}   {}   <- demotes refinements only",
            ratio(t.refined_strict, t.spans),
            ratio(t.refined_tolerant, t.spans)
        );
        println!();
        println!(
            "tolerant gain over top-1: plain {:+}, collapsed {:+}, demoted {:+}  (gate needs +55)",
            g(t.plain_tolerant, t.top1_tolerant),
            g(t.filtered_tolerant, t.top1_tolerant),
            g(t.demoted_tolerant, t.top1_tolerant)
        );
        println!(
            "strict gain over top-1:   plain {:+}, collapsed {:+}, demoted {:+}",
            g(t.plain_strict, t.top1_strict),
            g(t.filtered_strict, t.top1_strict),
            g(t.demoted_strict, t.top1_strict)
        );
        println!(
            "\nwhole gold cover present, of {} sentences: top-1 {}, oracle@3 plain {}, \n\
             oracle@3 demoted {}. Cycling only ever fully fixes a sentence when this fires,\n\
             so it is the honest ceiling on the feature rather than on the metric.",
            gold.len(),
            t.sent_top1,
            t.sent_plain,
            t.sent_demoted
        );
        println!();
        println!(
            "filter fired on {} of {} sentences, dropping {} readings; it surfaced a rank the \n\
             shipped k never saw on {} sentences (deepest rank used: {})",
            t.sentences_filtered,
            gold.len(),
            t.dropped,
            t.sentences_reaching_deeper,
            t.deepest_rank_used
        );
        Ok(())
    }
}
