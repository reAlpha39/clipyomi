// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! The adjudicated compound metric — the third thing `score` reports, and the
//! only one that can register compound preservation as a gain.
//!
//! Master plan §5.2's two span metrics are both scored against a gold corpus
//! adjudicated at a tokenizer's short-unit granularity. Strict span accuracy
//! therefore *punishes* keeping `音がする` whole, and the compound-tolerant
//! metric only declines to punish it: neither can reward it. Measured
//! (`docs/superpowers/2026-09-09-frequency-signal-candidates.md` §10.3), a
//! signal that gives an inverted-priority compound parity with its parts
//! costs about 6.9 points strict for at most 0.8 points tolerant — so no
//! frequency source, whatever its licence, could ever pass a gate built on
//! that pair. The layer Phase 3 exists to add is unmeasurable by the harness
//! that would have to approve it.
//!
//! This closes that gap with a hand-adjudicated file rather than a heuristic.
//! Every window where the engine faced a compound decision on the corpus —
//! both the merges it made and the compounds it declined, 194 of them — was
//! judged once per distinct `(surface, headword)` type against one rule:
//!
//! **`lexical`** — the whole is a dictionary unit whose force a reader looks
//! up as one thing: a compound noun (`副大統領`), an idiomatic expression
//! (`首を振る`), a numeral with its counter (`16日`), a conjugated single verb
//! (`行かなきゃ`), or a fixed grammatical construction whose meaning is not
//! its parts stacked (`である`, `によると`, `ことになっている`). The parse
//! should place a segment on exactly this window.
//!
//! **`transparent`** — function-word stacking whose parts are each what the
//! reader needs to see, and whose meaning composes from them: `には`, `での`,
//! `のか`, `彼の`. Merging these hides the particle doing the work, so the
//! parse should not place a segment here. This is the class where a reader's
//! interest and JMdict's headword list genuinely diverge: JMdict lists `には`
//! because it is a frequent collocation, not because it is opaque.
//!
//! **`false-match`** — the headword is a different word from what the text
//! says, so merging would display a wrong gloss: `がした` as `がす` (to gas),
//! `名の` as `名のる`, `はこう` as `はく`. §5.2's spurious-match class. The
//! parse should not place a segment here, and this is the class a *frequency*
//! rank separates from `lexical` where a build-time COMMON derivation cannot.
//!
//! **`ambiguous`** — genuinely context-dependent at type level (`では` as the
//! locative plus topic or as the conjunction; `時に` as `時` plus `に` or as
//! the adverb). Excluded from the ratio and counted beside it rather than
//! guessed — §5.2's finding that every mechanical discriminator for the
//! spurious merges cut the wrong way is the precedent for not faking this.
//!
//! Nesting is resolved per sentence, not per type: two overlapping windows
//! cannot both be segments, so among overlapping `lexical` windows the
//! longest is the one expected to merge and the shorter ones carry
//! `expect: split` with a note naming the window that won. That is why the
//! class and the expectation are recorded separately — the class is the
//! type-level judgment, the expectation is what this sentence permits.
//!
//! Reported separately from both span metrics, never combined, for the same
//! reason §5.2 gives: a change that lifts this and drops strict has changed
//! granularity, and the pair is what says so.

use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use jparser::Segment;

/// What the parse should do with one window, after nesting is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Expect {
    /// A segment should sit on exactly this window.
    Merge,
    /// No segment should sit on exactly this window.
    Split,
    /// Adjudication declined; excluded from the ratio.
    Skip,
}

/// One adjudicated compound decision. `text` keys it to a gold record;
/// `class` is the type-level judgment and `expect` what this sentence permits.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CompoundRecord {
    pub text: String,
    /// The gold record's own source id, carried for reading the file by hand.
    #[allow(dead_code)]
    pub source: String,
    pub start: usize,
    pub len: usize,
    pub surface: String,
    /// The headword the candidate at this window would display.
    #[allow(dead_code)]
    pub headword: String,
    /// The JMdict `ent_seq` that headword came from.
    #[allow(dead_code)]
    pub entry_id: u32,
    pub class: String,
    pub expect: Expect,
    /// Why a `lexical` window is nonetheless expected to split here.
    #[allow(dead_code)]
    #[serde(default)]
    pub note: Option<String>,
}

/// One class's tally within [`CompoundMetrics`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ClassBreakdown {
    pub class: String,
    pub hits: usize,
    pub total: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompoundMetrics {
    /// Windows the parse handled as adjudicated.
    pub hits: usize,
    /// Windows with a `merge` or `split` expectation. Excludes `skip`.
    pub total: usize,
    /// `ambiguous` windows, excluded from `total` and reported beside it so
    /// the ratio's denominator is never quietly smaller than the file.
    pub skipped: usize,
    /// Of `total`, the windows expected to merge — the half a frequency
    /// signal is meant to move. Printed beside the ratio because a change
    /// that lifts one half by sacrificing the other is the failure mode.
    pub merge_hits: usize,
    pub merge_total: usize,
    pub split_hits: usize,
    pub split_total: usize,
    /// Per `class`, in the fixture's first-seen order.
    pub by_class: Vec<ClassBreakdown>,
}

/// Read an adjudicated compound file, indexed by sentence.
///
/// A window outside its own sentence is rejected here; a sentence the corpus
/// being scored does not hold is rejected by [`crate::score::run`], which is
/// the only place both files are in scope. Both are errors rather than skips
/// because a fixture that had drifted from the corpus would otherwise
/// silently score fewer windows than it lists.
pub fn read(path: &Path) -> Result<HashMap<String, Vec<CompoundRecord>>, Box<dyn Error>> {
    let body = std::fs::read_to_string(path)?;
    let mut by_text: HashMap<String, Vec<CompoundRecord>> = HashMap::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let rec: CompoundRecord =
            serde_json::from_str(line).map_err(|e| format!("{}:{}: {e}", path.display(), i + 1))?;
        let chars = rec.text.chars().count();
        if rec.len == 0 || rec.start + rec.len > chars {
            return Err(format!(
                "{}:{}: window {}..{} is outside a {chars}-character sentence",
                path.display(),
                i + 1,
                rec.start,
                rec.start + rec.len
            )
            .into());
        }
        by_text.entry(rec.text.clone()).or_default().push(rec);
    }
    Ok(by_text)
}

/// The metric's three lines, appended to `score`'s human output.
pub fn render(m: &CompoundMetrics) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<16}{:.4}  ({} / {})\n",
        "compounds:",
        m.hits as f64 / m.total as f64,
        m.hits,
        m.total
    ));
    out.push_str(&format!(
        "  kept whole:   {} / {}     split apart: {} / {}\n",
        m.merge_hits, m.merge_total, m.split_hits, m.split_total
    ));
    let parts: Vec<String> = m
        .by_class
        .iter()
        .map(|c| format!("{} {} / {}", c.class, c.hits, c.total))
        .collect();
    out.push_str(&format!(
        "  by class:     {}   ({} ambiguous, excluded)\n",
        parts.join("  "),
        m.skipped
    ));
    out
}

/// Fold one sentence's adjudicated windows into `m`.
pub fn accumulate(records: &[CompoundRecord], segments: &[Segment], m: &mut CompoundMetrics) {
    for rec in records {
        if rec.expect == Expect::Skip {
            m.skipped += 1;
            continue;
        }
        let merged = segments
            .iter()
            .any(|s| s.matched && s.start == rec.start && s.len == rec.len);
        let hit = merged == (rec.expect == Expect::Merge);
        m.total += 1;
        m.hits += hit as usize;
        if rec.expect == Expect::Merge {
            m.merge_total += 1;
            m.merge_hits += hit as usize;
        } else {
            m.split_total += 1;
            m.split_hits += hit as usize;
        }
        let entry = match m.by_class.iter_mut().find(|c| c.class == rec.class) {
            Some(entry) => entry,
            None => {
                m.by_class.push(ClassBreakdown {
                    class: rec.class.clone(),
                    hits: 0,
                    total: 0,
                });
                m.by_class.last_mut().expect("just pushed")
            }
        };
        entry.total += 1;
        entry.hits += hit as usize;
    }
}

/// Every window in `records` the parse handled against its adjudication, for
/// `--compounds-detail`. One line each, so a regression names its cases
/// instead of only moving a ratio.
pub fn misses(records: &[CompoundRecord], segments: &[Segment]) -> Vec<String> {
    let mut out = Vec::new();
    for rec in records {
        if rec.expect == Expect::Skip {
            continue;
        }
        let merged = segments
            .iter()
            .any(|s| s.matched && s.start == rec.start && s.len == rec.len);
        if merged == (rec.expect == Expect::Merge) {
            continue;
        }
        out.push(format!(
            "  {:<12} {:<12} expected {} but the parse {}",
            rec.surface,
            rec.class,
            if rec.expect == Expect::Merge {
                "merge"
            } else {
                "split"
            },
            if merged { "merged" } else { "split" }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: usize, len: usize, matched: bool) -> Segment {
        Segment {
            start,
            len,
            surface: String::new(),
            reading: None,
            matched,
            entries: Vec::new(),
        }
    }

    fn rec(start: usize, len: usize, class: &str, expect: Expect) -> CompoundRecord {
        CompoundRecord {
            text: "音がする".into(),
            source: "t".into(),
            start,
            len,
            surface: "音がする".into(),
            headword: "音がする".into(),
            entry_id: 1,
            class: class.into(),
            expect,
            note: None,
        }
    }

    #[test]
    fn a_preserved_lexical_compound_scores_where_strict_span_accuracy_cannot() {
        let mut m = CompoundMetrics::default();
        accumulate(
            &[rec(0, 4, "lexical", Expect::Merge)],
            &[seg(0, 4, true)],
            &mut m,
        );
        assert_eq!((m.hits, m.total), (1, 1), "the merge is a hit, not a miss");
        assert_eq!((m.merge_hits, m.merge_total), (1, 1));
    }

    #[test]
    fn a_spurious_merge_of_a_false_match_scores_as_a_miss() {
        let mut m = CompoundMetrics::default();
        accumulate(
            &[rec(1, 3, "false-match", Expect::Split)],
            &[seg(1, 3, true)],
            &mut m,
        );
        assert_eq!((m.hits, m.total), (0, 1));
        assert_eq!((m.split_hits, m.split_total), (0, 1));
    }

    #[test]
    fn an_unmatched_run_over_the_window_is_not_a_merge() {
        let mut m = CompoundMetrics::default();
        accumulate(
            &[rec(0, 4, "lexical", Expect::Merge)],
            &[seg(0, 4, false)],
            &mut m,
        );
        assert_eq!(
            (m.hits, m.total),
            (0, 1),
            "a fallback run covering the window is not the compound"
        );
    }

    #[test]
    fn an_ambiguous_window_leaves_the_ratio_untouched() {
        let mut m = CompoundMetrics::default();
        accumulate(
            &[rec(0, 2, "ambiguous", Expect::Skip)],
            &[seg(0, 2, true)],
            &mut m,
        );
        assert_eq!((m.hits, m.total, m.skipped), (0, 0, 1));
        assert!(m.by_class.is_empty(), "a skip joins no class tally");
    }

    #[test]
    fn classes_are_tallied_separately() {
        let mut m = CompoundMetrics::default();
        accumulate(
            &[
                rec(0, 4, "lexical", Expect::Merge),
                rec(1, 2, "transparent", Expect::Split),
            ],
            &[seg(0, 4, true)],
            &mut m,
        );
        assert_eq!(m.by_class.len(), 2);
        assert_eq!(m.by_class[0].class, "lexical");
        assert_eq!((m.by_class[0].hits, m.by_class[0].total), (1, 1));
        assert_eq!((m.by_class[1].hits, m.by_class[1].total), (1, 1));
        assert_eq!((m.hits, m.total), (2, 2));
    }

    #[test]
    fn misses_name_the_window_and_both_directions() {
        let lines = misses(
            &[
                rec(0, 4, "lexical", Expect::Merge),
                rec(1, 2, "transparent", Expect::Split),
            ],
            &[seg(1, 2, true)],
        );
        assert_eq!(lines.len(), 2, "one miss in each direction");
        assert!(lines[0].contains("expected merge but the parse split"));
        assert!(lines[1].contains("expected split but the parse merged"));
    }
}
