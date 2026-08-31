// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! The two Phase 0 metrics, per design §5.2.
//!
//! **Span accuracy** — the fraction of gold spans the parser reproduces
//! exactly. Includes `ent_seq: null` spans: whether a function word is
//! delimited correctly is exactly as much a span question as whether a noun is.
//!
//! **Entry top-1** — of exactly-spanned gold spans with a non-null `ent_seq`,
//! the fraction where the parse ranked that entry first. Conditioned on correct
//! spanning because a wrong span has no meaningful entry list, and on non-null
//! gold because a null label asserts nothing about ranking.
//!
//! Reported separately, always, with numerator and denominator beside each
//! ratio. Never combined: a change that improves one and harms the other is a
//! finding, not noise.

use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::path::Path;

use crate::EngineChoice;
use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{ParseOptions, ParseResult};

use crate::corpus::{read_gold, validate, GoldRecord};

/// Field width for the label column in [`render`]. `span accuracy:` is the
/// longest label at 14 chars; 16 leaves two spaces after it.
const LABEL_WIDTH: usize = 16;

/// One decision's span-accuracy tally within [`Metrics::by_decision`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DecisionBreakdown {
    pub decision: String,
    pub hits: usize,
    pub total: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Metrics {
    pub sentences: usize,
    /// Span accuracy's denominator: every gold span, `null` ones included.
    pub gold_spans: usize,
    /// Records dropped as superseded by `read_gold`. Reported because a file
    /// that is a third supersessions is a signal about the labelling pass.
    pub superseded: usize,
    pub span_hits: usize,
    pub top1_hits: usize,
    pub top1_total: usize,
    /// Span accuracy split by `label`'s decision (Critical 1 in the Phase 0
    /// review): `j` blesses the hints-off parse verbatim, so a `j`-blessed
    /// sentence scores 1.0 under `score` hints-off by construction. Recording
    /// the split makes that bias visible instead of hiding inside the
    /// headline number. Empty when no gold record carries a `decision` (a
    /// hand-authored corpus, for instance), and printed and serialized only
    /// then. Ordered `j`, `v`, `e`, then anything else alphabetically —
    /// [`run`] sorts it before either output is produced.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub by_decision: Vec<DecisionBreakdown>,
    /// Whether `--hints` was used for this run. Recorded so two `--json`
    /// baselines are distinguishable on their own — without it a swapped
    /// hints-on/hints-off pair would silently sign-flip the reported hint
    /// delta (design §8.3, §5.3).
    pub hints: bool,
    /// Gold spans that strict span accuracy scored as misses, but which a
    /// single dictionary match swallowed whole — the merge starting and
    /// ending on gold boundaries, so it cut no gold span.
    ///
    /// Handoff §9's finding: of the 266 gold boundaries Vibrato supplies and
    /// jparser drops, roughly half are jparser merging into a real JMdict
    /// multi-word expression (`音がする`, `ことになっている`, `首を振る`) or a
    /// numeral+counter date headword (`2月`, `18日`). The corpus was
    /// adjudicated at the tokenizer's short-unit granularity, so strict span
    /// accuracy scores every one of those zero — penalising exactly the
    /// compound preservation master plan §5.2 calls the reason the DP exists.
    /// The decision log had already rejected boundary F1 for this and did not
    /// notice span accuracy inherits it.
    ///
    /// This does **not** replace `span_hits`. Both are reported: a Phase 1
    /// change that moves strict without moving tolerant has changed
    /// granularity, not correctness, and the pair is what says so. Zero on an
    /// exact parse, and skipped in both output forms when zero.
    #[serde(skip_serializing_if = "is_zero")]
    pub coarse_forgiven: usize,
    /// Distinct parse segments that did the forgiving. `coarse_forgiven`
    /// alone cannot distinguish one 40-span merge from forty benign 2-span
    /// ones, and those are different findings.
    #[serde(skip_serializing_if = "is_zero")]
    pub coarse_merges: usize,
    /// The index directory's final path component — its generation name
    /// (e.g. `gen-3`), or `None` if the path has none. Recorded beside the
    /// numbers for the same reason as `hints`.
    pub index_generation: Option<String>,
}

/// `skip_serializing_if` for the coarsening counters, so a corpus the parse
/// reproduces exactly serializes byte-identically to before they existed.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Bump one decision's span tally in `by_decision`, inserting it if this is
/// its first span. A linear scan over a handful of decisions costs less than
/// the map bookkeeping it would replace.
fn bump_decision(by_decision: &mut Vec<DecisionBreakdown>, decision: &str, hit: bool) {
    let entry = match by_decision.iter_mut().find(|d| d.decision == decision) {
        Some(entry) => entry,
        None => {
            by_decision.push(DecisionBreakdown {
                decision: decision.to_string(),
                hits: 0,
                total: 0,
            });
            by_decision.last_mut().expect("just pushed")
        }
    };
    entry.total += 1;
    if hit {
        entry.hits += 1;
    }
}

/// `j`, `v`, `e`, then anything else alphabetically — the order design's
/// Critical 1 fix prints the breakdown in, so `j`'s optimistic bias is always
/// the first thing a reader sees.
fn decision_rank(decision: &str) -> (u8, &str) {
    match decision {
        "j" => (0, decision),
        "v" => (1, decision),
        "e" => (2, decision),
        _ => (3, decision),
    }
}

/// Fold one sentence's gold and parse into `m`.
pub fn accumulate(gold: &GoldRecord, parsed: &ParseResult, m: &mut Metrics) {
    m.sentences += 1;
    // Every gold boundary, so a merge can be tested for cutting one. Starts
    // plus the last span's end: gold spans are a contiguous cover, so that is
    // the whole set.
    let mut bounds: BTreeSet<usize> = gold.spans.iter().map(|s| s.start).collect();
    if let Some(last) = gold.spans.last() {
        bounds.insert(last.start + last.len);
    }
    // Per record, so a segment forgiving three spans counts as one merge.
    let mut merges: BTreeSet<(usize, usize)> = BTreeSet::new();

    for span in &gold.spans {
        m.gold_spans += 1;
        let seg = parsed
            .segments
            .iter()
            .find(|s| s.start == span.start && s.len == span.len);
        if let Some(decision) = gold.decision.as_deref() {
            bump_decision(&mut m.by_decision, decision, seg.is_some());
        }
        if seg.is_none() {
            // Benign coarsening: a matched segment that contains this gold
            // span and whose own endpoints are gold boundaries merged whole
            // gold spans rather than cutting one. A linear scan over a
            // sentence's segments; corpora are hundreds of sentences, so the
            // index this would otherwise want is not worth its bookkeeping.
            if let Some(j) = parsed.segments.iter().find(|j| {
                j.matched
                    && j.start <= span.start
                    && span.start + span.len <= j.start + j.len
                    && bounds.contains(&j.start)
                    && bounds.contains(&(j.start + j.len))
            }) {
                m.coarse_forgiven += 1;
                merges.insert((j.start, j.len));
            }
        }
        let Some(seg) = seg else { continue };
        m.span_hits += 1;
        // A null gold span asserts nothing about ranking, so it leaves both
        // halves of entry top-1 untouched.
        let Some(want) = span.ent_seq else { continue };
        m.top1_total += 1;
        // `entries.first()` is None for an unmatched span, which is a miss
        // rather than an exclusion — design §5.2.
        if seg.entries.first().map(|e| e.entry_id) == Some(want) {
            m.top1_hits += 1;
        }
    }
    m.coarse_merges += merges.len();
}

/// Confirm every `ent_seq` a gold record names actually resolves in `index`.
/// Design §12: absent is a hard error, not a zero score — it means the
/// extractor was not re-run after the corpus grew, and scoring it as a miss
/// would misattribute a stale fixture to the engine. `cache` remembers each
/// id's presence across records, since the same id recurs across a corpus and
/// a miss would otherwise be a fresh decode off the mmap every time.
fn ensure_ent_seqs_exist(
    index: &Index,
    cache: &mut HashMap<u32, bool>,
    record: &GoldRecord,
) -> Result<(), Box<dyn Error>> {
    for span in &record.spans {
        let Some(ent_seq) = span.ent_seq else {
            continue;
        };
        let present = match cache.get(&ent_seq) {
            Some(&present) => present,
            None => {
                let present = index.entry(ent_seq)?.is_some();
                cache.insert(ent_seq, present);
                present
            }
        };
        if !present {
            return Err(format!(
                "gold record {:?} names ent_seq {ent_seq}, which is not in this index \
                 (likely cause: a stale subset — re-run tools/extract_jmdict_subset.py)",
                record.text
            )
            .into());
        }
    }
    Ok(())
}

/// `num / den` to four places, with the fraction beside it. `n/a` padded to the
/// same eight columns a ratio occupies, so the fractions stay aligned.
fn ratio(num: usize, den: usize) -> String {
    if den == 0 {
        format!("n/a     ({num} / {den})")
    } else {
        format!("{:.4}  ({num} / {den})", num as f64 / den as f64)
    }
}

/// The human form. A frozen interface for the first five lines: `cli_score.rs`
/// asserts them exactly, and the recorded baseline is transcribed from them.
/// The `by decision:` line is additive — printed only when `m.by_decision` is
/// non-empty, in whatever order it is given (design's Critical 1 fix; [`run`]
/// sorts it before calling this).
pub fn render(m: &Metrics) -> String {
    let w = LABEL_WIDTH;
    let mut out = String::new();
    out.push_str(&format!("{:<w$}{}\n", "sentences:", m.sentences));
    out.push_str(&format!("{:<w$}{}\n", "gold spans:", m.gold_spans));
    out.push_str(&format!("{:<w$}{}\n", "superseded:", m.superseded));
    out.push_str(&format!(
        "{:<w$}{}\n",
        "span accuracy:",
        ratio(m.span_hits, m.gold_spans)
    ));
    out.push_str(&format!(
        "{:<w$}{}\n",
        "entry top-1:",
        ratio(m.top1_hits, m.top1_total)
    ));
    if !m.by_decision.is_empty() {
        let parts: Vec<String> = m
            .by_decision
            .iter()
            .map(|d| {
                format!(
                    "{} {:.4} ({} / {})",
                    d.decision,
                    d.hits as f64 / d.total as f64,
                    d.hits,
                    d.total
                )
            })
            .collect();
        out.push_str(&format!("  by decision:  {}\n", parts.join("  ")));
    }
    // Additive like `by decision:`, and silent when there is no coarsening —
    // which is itself the reading "strict and tolerant agree".
    if m.coarse_forgiven > 0 {
        out.push_str(&format!(
            "{:<w$}{}\n",
            "span tolerant:",
            ratio(m.span_hits + m.coarse_forgiven, m.gold_spans)
        ));
        let s = |n: usize| if n == 1 { "" } else { "s" };
        out.push_str(&format!(
            "  coarsening:   {} gold span{} inside {} dictionary merge{}\n",
            m.coarse_forgiven,
            s(m.coarse_forgiven),
            m.coarse_merges,
            s(m.coarse_merges)
        ));
    }
    out
}

/// Score `gold` against the engine backed by `index`.
///
/// `hints` optional: this is where the hints-on and hints-off baseline pair is
/// produced (master plan §5.4). Nothing here touches the network, and with
/// `hints` absent nothing loads a tokenizer — which is what keeps the offline
/// gate free of the 46 MB dictionary.
pub fn run(
    index_dir: &Path,
    gold_path: &Path,
    #[cfg(feature = "mecab")] hints: Option<&Path>,
    json: bool,
    engine: EngineChoice,
) -> Result<(), Box<dyn Error>> {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(index_dir)?;
    let (records, superseded) = read_gold(gold_path)?;

    #[cfg(feature = "mecab")]
    let tokenizer = match hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
        None => None,
    };
    #[cfg(feature = "mecab")]
    let hints_used = tokenizer.is_some();
    #[cfg(not(feature = "mecab"))]
    let hints_used = false;
    let index_generation = index_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());

    let mut m = Metrics {
        superseded,
        hints: hints_used,
        index_generation,
        ..Metrics::default()
    };
    let mut ent_seq_cache: HashMap<u32, bool> = HashMap::new();
    for record in &records {
        validate(record)?;
        ensure_ent_seqs_exist(&index, &mut ent_seq_cache, record)?;
        #[cfg(feature = "mecab")]
        let flags = tokenizer.as_ref().map(|t| t.hints(&record.text));
        #[cfg(feature = "mecab")]
        let hints: Option<&dyn jparser::BoundaryHints> =
            flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
        #[cfg(not(feature = "mecab"))]
        let hints: Option<&dyn jparser::BoundaryHints> = None;

        let opts = ParseOptions::default();
        // `bounds` is `None` for now; Task 16 wires the real boundary source.
        let bounds: Option<&dyn jparser::engine::Boundaries> = None;
        let parsed = match engine {
            EngineChoice::V1 => jparser::parse(&index, &table, &record.text, &opts, hints)?,
            EngineChoice::V2 => jparser::engine::parse_v2(&index, &record.text, &opts, bounds)?,
        };
        accumulate(record, &parsed, &mut m);
    }
    m.by_decision
        .sort_by(|a, b| decision_rank(&a.decision).cmp(&decision_rank(&b.decision)));

    if json {
        println!("{}", serde_json::to_string(&m)?);
    } else {
        print!("{}", render(&m));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{GoldRecord, GoldSpan};
    use jparser::record::WordFlags;
    use jparser::{Entry, ParseResult, Segment};

    fn gold(text: &str, spans: &[(usize, usize, Option<u32>)]) -> GoldRecord {
        GoldRecord {
            text: text.to_string(),
            source: "test:1".to_string(),
            spans: spans
                .iter()
                .map(|&(start, len, ent_seq)| GoldSpan {
                    start,
                    len,
                    ent_seq,
                })
                .collect(),
            decision: None,
        }
    }

    fn gold_with_decision(
        text: &str,
        spans: &[(usize, usize, Option<u32>)],
        decision: &str,
    ) -> GoldRecord {
        GoldRecord {
            decision: Some(decision.to_string()),
            ..gold(text, spans)
        }
    }

    /// A segment carrying `entry_ids` as its ranked candidate list. Only
    /// `start`, `len`, `matched` and `entries[..].entry_id` affect the metrics.
    fn seg(start: usize, len: usize, entry_ids: &[u32]) -> Segment {
        Segment {
            start,
            len,
            surface: String::new(),
            reading: None,
            matched: !entry_ids.is_empty(),
            entries: entry_ids
                .iter()
                .map(|&entry_id| Entry {
                    headword: String::new(),
                    reading: None,
                    conjugation: None,
                    pos: Vec::new(),
                    senses: Vec::new(),
                    flags: WordFlags::default(),
                    entry_id,
                    steps: Vec::new(),
                })
                .collect(),
        }
    }

    fn parsed(segments: Vec<Segment>) -> ParseResult {
        ParseResult { segments }
    }

    #[test]
    fn a_perfect_parse_scores_one_on_both_metrics() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あい", &[(0, 1, Some(10)), (1, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[10]), seg(1, 1, &[20])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (2, 2));
        assert_eq!((m.top1_total, m.top1_hits), (2, 2));
        assert_eq!(m.sentences, 1);
    }

    /// One boundary moved costs exactly the two spans it straddles, and takes
    /// them out of entry top-1's denominator with it.
    #[test]
    fn a_moved_boundary_costs_exactly_the_spans_it_straddles() {
        let mut m = Metrics::default();
        accumulate(
            &gold(
                "あいう",
                &[(0, 1, Some(10)), (1, 1, Some(20)), (2, 1, Some(30))],
            ),
            &parsed(vec![seg(0, 2, &[10]), seg(2, 1, &[30])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (3, 1), "only (2,1) survives");
        assert_eq!((m.top1_total, m.top1_hits), (1, 1));
    }

    /// The §9 finding: a parse that merges whole gold spans into one
    /// dictionary entry is a coarser analysis, not a wrong one, and strict
    /// span accuracy scores it zero.
    #[test]
    fn a_dictionary_merge_across_whole_gold_spans_is_tolerantly_correct() {
        let mut m = Metrics::default();
        accumulate(
            &gold(
                "あいう",
                &[(0, 1, Some(10)), (1, 1, Some(20)), (2, 1, Some(30))],
            ),
            &parsed(vec![seg(0, 3, &[40])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (3, 0), "strict scores none");
        assert_eq!(m.coarse_forgiven, 3, "all three are inside the merge");
        assert_eq!(m.coarse_merges, 1, "one segment did the forgiving");
    }

    /// Forgiveness is for *dictionary* merges. An unmatched run that happens
    /// to span the same characters is the fallback path, not an analysis.
    #[test]
    fn an_unmatched_merge_is_not_forgiven() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あいう", &[(0, 1, None), (1, 1, None), (2, 1, None)]),
            &parsed(vec![seg(0, 3, &[])]),
            &mut m,
        );

        assert_eq!((m.span_hits, m.coarse_forgiven), (0, 0));
        assert_eq!(m.coarse_merges, 0);
    }

    /// A merge that ends inside a gold span cut that span, which is a real
    /// error however well the rest of it lines up. Only merges whose *both*
    /// endpoints are gold boundaries are benign.
    #[test]
    fn a_merge_ending_inside_a_gold_span_is_not_forgiven() {
        let mut m = Metrics::default();
        accumulate(
            // gold boundaries are 0, 1, 3 — the merge below ends at 2.
            &gold("あいう", &[(0, 1, Some(10)), (1, 2, Some(20))]),
            &parsed(vec![seg(0, 2, &[40]), seg(2, 1, &[50])]),
            &mut m,
        );

        assert_eq!(m.span_hits, 0);
        assert_eq!(
            m.coarse_forgiven, 0,
            "(0,1) sits inside a merge that cut (1,2), so it is not benign"
        );
    }

    #[test]
    fn an_exact_parse_leaves_the_coarsening_counters_at_zero() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あい", &[(0, 1, Some(10)), (1, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[10]), seg(1, 1, &[20])]),
            &mut m,
        );

        assert_eq!((m.span_hits, m.coarse_forgiven, m.coarse_merges), (2, 0, 0));
    }

    #[test]
    fn a_null_gold_span_counts_for_span_accuracy_and_not_for_entry_top_1() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あい", &[(0, 1, None), (1, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[99]), seg(1, 1, &[20])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (2, 2), "both spans are right");
        assert_eq!(
            (m.top1_total, m.top1_hits),
            (1, 1),
            "the null span is out of both halves of the ratio"
        );
    }

    #[test]
    fn a_correct_span_ranked_second_is_a_top_1_miss() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[10, 20])]),
            &mut m,
        );

        assert_eq!(m.span_hits, 1);
        assert_eq!((m.top1_total, m.top1_hits), (1, 0));
    }

    /// Design §5.2: an exactly-spanned non-null gold span whose segment matched
    /// nothing counts in the denominator and fails the numerator. Excluding it
    /// would flatter the baseline.
    #[test]
    fn an_unmatched_segment_on_a_correct_span_is_a_top_1_miss_not_an_exclusion() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[])]),
            &mut m,
        );

        assert_eq!(m.span_hits, 1, "the span is still correctly delimited");
        assert_eq!((m.top1_total, m.top1_hits), (1, 0));
    }

    #[test]
    fn metrics_accumulate_across_sentences() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(10))]),
            &parsed(vec![seg(0, 1, &[10])]),
            &mut m,
        );
        accumulate(
            &gold("い", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[99])]),
            &mut m,
        );

        assert_eq!(m.sentences, 2);
        assert_eq!((m.gold_spans, m.span_hits), (2, 2));
        assert_eq!((m.top1_total, m.top1_hits), (2, 1));
    }

    #[test]
    fn the_rendered_form_carries_both_ratios_with_their_fractions() {
        let m = Metrics {
            sentences: 250,
            gold_spans: 2481,
            superseded: 12,
            span_hits: 2167,
            top1_hits: 1502,
            top1_total: 1898,
            ..Metrics::default()
        };

        assert_eq!(
            render(&m),
            "sentences:      250\n\
             gold spans:     2481\n\
             superseded:     12\n\
             span accuracy:  0.8734  (2167 / 2481)\n\
             entry top-1:    0.7914  (1502 / 1898)\n"
        );
    }

    /// An all-null corpus has no entry top-1 denominator. `n/a`, not a divide
    /// by zero and not a misleading 0.0000.
    #[test]
    fn an_empty_denominator_renders_as_not_applicable() {
        let m = Metrics {
            sentences: 1,
            gold_spans: 1,
            superseded: 0,
            span_hits: 1,
            top1_hits: 0,
            top1_total: 0,
            ..Metrics::default()
        };

        assert!(
            render(&m).contains("entry top-1:    n/a     (0 / 0)"),
            "got {}",
            render(&m)
        );
    }

    /// Critical 1's fix, at the `accumulate` level: a `j`-blessed record and a
    /// `v`-blessed record land in separate `by_decision` buckets, and a record
    /// with no `decision` touches neither.
    #[test]
    fn accumulate_splits_span_accuracy_by_decision() {
        let mut m = Metrics::default();
        accumulate(
            &gold_with_decision("あい", &[(0, 1, None), (1, 1, None)], "j"),
            &parsed(vec![seg(0, 1, &[]), seg(1, 1, &[])]),
            &mut m,
        );
        accumulate(
            // Only the first span matches; the second gold span (1,1) has no
            // corresponding segment in this parse, so it is a miss for `v`.
            &gold_with_decision("あい", &[(0, 1, None), (1, 1, None)], "v"),
            &parsed(vec![seg(0, 1, &[])]),
            &mut m,
        );
        accumulate(
            &gold("う", &[(0, 1, None)]),
            &parsed(vec![seg(0, 1, &[])]),
            &mut m,
        );

        let mut by_decision = m.by_decision.clone();
        by_decision.sort_by(|a, b| decision_rank(&a.decision).cmp(&decision_rank(&b.decision)));
        assert_eq!(
            by_decision,
            vec![
                DecisionBreakdown {
                    decision: "j".to_string(),
                    hits: 2,
                    total: 2,
                },
                DecisionBreakdown {
                    decision: "v".to_string(),
                    hits: 1,
                    total: 2,
                },
            ],
            "the undecided record contributes to neither bucket"
        );
    }

    /// The breakdown line is additive and appears only when there is one to
    /// show, in the order it is given — `run` is what sorts it into `j, v, e`.
    #[test]
    fn render_prints_the_decision_breakdown_only_when_present() {
        let mut m = Metrics {
            sentences: 2,
            gold_spans: 2,
            superseded: 0,
            span_hits: 1,
            top1_hits: 0,
            top1_total: 0,
            ..Metrics::default()
        };
        assert!(!render(&m).contains("by decision"), "nothing to show yet");

        m.by_decision = vec![
            DecisionBreakdown {
                decision: "j".to_string(),
                hits: 1,
                total: 1,
            },
            DecisionBreakdown {
                decision: "v".to_string(),
                hits: 0,
                total: 1,
            },
        ];
        assert!(
            render(&m).contains("  by decision:  j 1.0000 (1 / 1)  v 0.0000 (0 / 1)\n"),
            "got {}",
            render(&m)
        );
    }

    fn tiny_index(dir: &std::path::Path) -> Index {
        const XML: &str = concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            "<JMdict>",
            "<entry><ent_seq>1</ent_seq>",
            "<r_ele><reb>あ</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>ah</gloss></sense></entry>",
            "</JMdict>",
        );
        let table = ConjugationTable::load_embedded().expect("embedded table");
        jparser::index::build::build_from_reader(
            XML.as_bytes(),
            &table,
            &jparser::stem::StemOptions::default(),
            dir,
        )
        .expect("the fixture must build");
        Index::open(dir).expect("open the built index")
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-score-unit-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Design §12: an ent_seq absent from the index is a hard error naming
    /// both the record's text and the offending id, not a silent miss.
    #[test]
    fn an_absent_ent_seq_is_a_hard_error() {
        let dir = scratch("absent-ent-seq");
        let index = tiny_index(&dir);
        let mut cache = HashMap::new();

        let err = ensure_ent_seqs_exist(&index, &mut cache, &gold("あ", &[(0, 1, Some(999))]))
            .expect_err("999 is not in the tiny index");

        let msg = err.to_string();
        assert!(msg.contains("あ"), "names the text: {msg}");
        assert!(msg.contains("999"), "names the ent_seq: {msg}");
    }

    #[test]
    fn a_present_ent_seq_passes_and_is_cached() {
        let dir = scratch("present-ent-seq");
        let index = tiny_index(&dir);
        let mut cache = HashMap::new();
        let record = gold("あ", &[(0, 1, Some(1))]);

        ensure_ent_seqs_exist(&index, &mut cache, &record).expect("1 is in the tiny index");
        // Second span, same id: must still pass from the cache alone.
        ensure_ent_seqs_exist(&index, &mut cache, &record).expect("cached hit still passes");

        assert_eq!(cache.get(&1), Some(&true));
    }

    /// A `null` gold span names no entry at all, so it can never trip the
    /// absent-ent_seq hard error.
    #[test]
    fn a_null_ent_seq_never_triggers_the_hard_error() {
        let dir = scratch("null-ent-seq");
        let index = tiny_index(&dir);
        let mut cache = HashMap::new();

        ensure_ent_seqs_exist(&index, &mut cache, &gold("あ", &[(0, 1, None)]))
            .expect("a null span names nothing to look up");
    }
}
