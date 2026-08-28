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

use std::error::Error;
use std::path::Path;

use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{ParseOptions, ParseResult};

use crate::corpus::{read_gold, validate, GoldRecord};

/// Field width for the label column in [`render`]. `span accuracy:` is the
/// longest label at 14 chars; 16 leaves two spaces after it.
const LABEL_WIDTH: usize = 16;

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
}

/// Fold one sentence's gold and parse into `m`.
pub fn accumulate(gold: &GoldRecord, parsed: &ParseResult, m: &mut Metrics) {
    m.sentences += 1;
    for span in &gold.spans {
        m.gold_spans += 1;
        let Some(seg) = parsed
            .segments
            .iter()
            .find(|s| s.start == span.start && s.len == span.len)
        else {
            continue;
        };
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

/// The human form. A frozen interface: `cli_score.rs` asserts it exactly, and
/// the recorded baseline is transcribed from it.
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
) -> Result<(), Box<dyn Error>> {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(index_dir)?;
    let (records, superseded) = read_gold(gold_path)?;

    #[cfg(feature = "mecab")]
    let tokenizer = match hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
        None => None,
    };

    let mut m = Metrics {
        superseded,
        ..Metrics::default()
    };
    for record in &records {
        validate(record)?;
        #[cfg(feature = "mecab")]
        let flags = tokenizer.as_ref().map(|t| t.hints(&record.text));
        #[cfg(feature = "mecab")]
        let hints: Option<&dyn jparser::BoundaryHints> =
            flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
        #[cfg(not(feature = "mecab"))]
        let hints: Option<&dyn jparser::BoundaryHints> = None;

        let parsed = jparser::parse(
            &index,
            &table,
            &record.text,
            &ParseOptions::default(),
            hints,
        )?;
        accumulate(record, &parsed, &mut m);
    }

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
        };

        assert!(
            render(&m).contains("entry top-1:    n/a     (0 / 0)"),
            "got {}",
            render(&m)
        );
    }
}
