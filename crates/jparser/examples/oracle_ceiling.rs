// SPDX-License-Identifier: MIT OR Apache-2.0

//! Oracle@∞ ceiling for Engine v2 Phase 2 — a throwaway spike, not kept code.
//!
//! Master plan §7 Phase 2's gate is oracle@k, which needs the k-best DP to
//! exist. This measures the bound that needs no DP: for every gold span, does
//! `lex::candidates` already hold a candidate at the gold `(start, len)`, and
//! one carrying the gold `ent_seq`? No k-best path can reproduce a span the
//! lattice never offered, so these are strict upper bounds on oracle@k for
//! every k. Hints on = the candidate also passes `cost::is_allowed` under
//! Vibrato's boundaries, which is the only way hints touch candidate existence.
//!
//! Fallback is held at top-1: an unmatched run can cover any span, so the
//! literal k=∞ span ceiling is 1.0 and says nothing. A span top-1 already
//! reproduces counts as reachable; otherwise a candidate must exist.
//!
//! The tolerant ceiling adds one more way in: strict, plus any allowed
//! candidate that spans the gold span with both its own endpoints on gold
//! boundaries — the same merge `accumulate`'s tolerant rule forgives a
//! matched top-1 segment for, extended to a candidate no path has taken yet.
//! It is strict plus that one extra allowance, not an independently defined
//! bound.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example oracle_ceiling -- \
//!   corpus_staging/gold_index crates/jparser/tests/fixtures/gold.jsonl \
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

    use jparser::engine::cost::is_allowed;
    use jparser::engine::lex::{candidates, Candidate};
    use jparser::engine::table::Table;
    use jparser::engine::Boundaries;
    use jparser::hints::VibratoTokenizer;
    use jparser::index::load::Index;
    use jparser::{parse, ParseOptions, ParseResult};

    #[derive(serde::Deserialize)]
    struct GoldSpan {
        start: usize,
        len: usize,
        ent_seq: Option<u32>,
    }

    #[derive(serde::Deserialize)]
    struct GoldRecord {
        text: String,
        source: String,
        spans: Vec<GoldSpan>,
    }

    #[derive(Default)]
    struct Tally {
        spans: usize,
        top1_strict: usize,
        top1_tolerant: usize,
        ceil_strict: usize,
        ceil_tolerant: usize,
        // Non-null spans only.
        nonnull: usize,
        top1_exact: usize,
        ceil_exact: usize,
        /// Top-1 got the span wrong; the exact (start, len, ent_seq) candidate
        /// exists and is allowed. The only spans k-best can newly win.
        headroom_span: usize,
        /// `headroom_span` minus the spans top-1 already forgives as a benign
        /// merge: what k-best would win beyond re-splitting a compound the
        /// corpus splits.
        headroom_real: usize,
        /// `headroom_real` per sentence, for the whole-sentence reading list:
        /// one alternate path must flip every one of them at once.
        per_sentence: Vec<usize>,
        /// Top-1 got the span right, ranked another entry first; the exact
        /// candidate exists. Within-span ranking, not a k-best matter.
        rank_only: usize,
        miss_no_span: usize,
        miss_wrong_entry: usize,
        miss_forbidden: usize,
        // Null spans only.
        null: usize,
        null_top1: usize,
        null_ceil: usize,
        samples_no_span: Vec<String>,
        samples_wrong_entry: Vec<String>,
    }

    const SAMPLES: usize = 25;

    fn read_gold(path: &str) -> Result<(Vec<GoldRecord>, usize), Box<dyn Error>> {
        // Last record per `text` wins, as `corpus::read_gold` does.
        let mut all: Vec<GoldRecord> = Vec::new();
        for line in std::fs::read_to_string(path)?.lines() {
            if !line.trim().is_empty() {
                all.push(serde_json::from_str(line)?);
            }
        }
        let mut last: HashMap<&str, usize> = HashMap::new();
        for (i, r) in all.iter().enumerate() {
            last.insert(&r.text, i);
        }
        let keep: BTreeSet<usize> = last.into_values().collect();
        let superseded = all.len() - keep.len();
        let out = all
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect();
        Ok((out, superseded))
    }

    fn slice(chars: &[char], a: usize, b: usize) -> String {
        chars.get(a..b).unwrap_or_default().iter().collect()
    }

    /// `accumulate`'s forgiveness rule: a matched segment containing the span
    /// whose own endpoints are gold boundaries.
    fn forgiven(parsed: &ParseResult, s: &GoldSpan, gb: &BTreeSet<usize>) -> bool {
        parsed.segments.iter().any(|j| {
            j.matched
                && j.start <= s.start
                && s.start + s.len <= j.start + j.len
                && gb.contains(&j.start)
                && gb.contains(&(j.start + j.len))
        })
    }

    /// Some allowed candidate contains the span with both its endpoints on
    /// gold boundaries — the merge the tolerant metric would forgive, if any
    /// path took it.
    fn merge_available(
        by_start: &[Vec<Candidate>],
        s: &GoldSpan,
        gb: &BTreeSet<usize>,
        bounds: Option<&dyn Boundaries>,
    ) -> bool {
        let end = s.start + s.len;
        (0..=s.start).any(|a| {
            gb.contains(&a)
                && by_start[a]
                    .iter()
                    .any(|c| a + c.len >= end && gb.contains(&(a + c.len)) && is_allowed(c, bounds))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn tally(
        rec: &GoldRecord,
        chars: &[char],
        by_start: &[Vec<Candidate>],
        bounds: Option<&dyn Boundaries>,
        parsed: &ParseResult,
        t: &mut Tally,
        collect_samples: bool,
    ) {
        let mut gb: BTreeSet<usize> = rec.spans.iter().map(|s| s.start).collect();
        if let Some(last) = rec.spans.last() {
            gb.insert(last.start + last.len);
        }

        let mut real_here = 0;
        for s in &rec.spans {
            t.spans += 1;
            let seg = parsed
                .segments
                .iter()
                .find(|j| j.start == s.start && j.len == s.len);
            let top1_strict = seg.is_some();
            let top1_tolerant = top1_strict || forgiven(parsed, s, &gb);
            t.top1_strict += top1_strict as usize;
            t.top1_tolerant += top1_tolerant as usize;

            let at: Vec<&Candidate> = by_start[s.start]
                .iter()
                .filter(|c| c.len == s.len)
                .collect();
            let span_any = at.iter().any(|c| is_allowed(c, bounds));
            let ceil_strict = top1_strict || span_any;
            let ceil_tolerant =
                ceil_strict || top1_tolerant || merge_available(by_start, s, &gb, bounds);
            t.ceil_strict += ceil_strict as usize;
            t.ceil_tolerant += ceil_tolerant as usize;

            let Some(e) = s.ent_seq else {
                t.null += 1;
                t.null_top1 += top1_strict as usize;
                t.null_ceil += ceil_strict as usize;
                continue;
            };
            t.nonnull += 1;
            let top1_exact = seg.is_some_and(|j| j.entries.first().map(|x| x.entry_id) == Some(e));
            t.top1_exact += top1_exact as usize;
            let exact_any = at.iter().any(|c| c.entry_id == e);
            let exact = at.iter().any(|c| c.entry_id == e && is_allowed(c, bounds));
            t.ceil_exact += exact as usize;
            if exact && !top1_strict {
                t.headroom_span += 1;
                if !top1_tolerant {
                    t.headroom_real += 1;
                    real_here += 1;
                }
            }
            if exact && top1_strict && !top1_exact {
                t.rank_only += 1;
            }
            if exact {
                continue;
            }
            let surface = slice(chars, s.start, s.start + s.len);
            if exact_any {
                t.miss_forbidden += 1;
            } else if at.is_empty() {
                t.miss_no_span += 1;
                if collect_samples && t.samples_no_span.len() < SAMPLES {
                    t.samples_no_span
                        .push(format!("  {:<14} {surface:<8} want {e}", rec.source));
                }
            } else {
                t.miss_wrong_entry += 1;
                if collect_samples && t.samples_wrong_entry.len() < SAMPLES {
                    let have: Vec<String> = at
                        .iter()
                        .map(|c| format!("{}[{}]", c.headword, c.entry_id))
                        .collect();
                    t.samples_wrong_entry.push(format!(
                        "  {:<14} {surface:<8} want {e}  have {}",
                        rec.source,
                        have.join(" ")
                    ));
                }
            }
        }
        t.per_sentence.push(real_here);
    }

    fn ratio(n: usize, d: usize) -> String {
        format!("{:.4} ({n}/{d})", n as f64 / d as f64)
    }

    fn pts(n: usize, d: usize) -> String {
        format!("{:+.2} pts", 100.0 * n as f64 / d as f64)
    }

    fn report(label: &str, t: &Tally) {
        println!("== hints {label} ==");
        println!(
            "{:<8} strict {}  tolerant {}  exact {}",
            "top-1",
            ratio(t.top1_strict, t.spans),
            ratio(t.top1_tolerant, t.spans),
            ratio(t.top1_exact, t.nonnull)
        );
        println!(
            "{:<8} strict {}  tolerant {}  exact {}",
            "ceiling",
            ratio(t.ceil_strict, t.spans),
            ratio(t.ceil_tolerant, t.spans),
            ratio(t.ceil_exact, t.nonnull)
        );
        println!(
            "headroom strict {} ({})  tolerant {} ({})  exact {} ({})",
            t.ceil_strict - t.top1_strict,
            pts(t.ceil_strict - t.top1_strict, t.spans),
            t.ceil_tolerant - t.top1_tolerant,
            pts(t.ceil_tolerant - t.top1_tolerant, t.spans),
            t.ceil_exact - t.top1_exact,
            pts(t.ceil_exact - t.top1_exact, t.nonnull),
        );
        println!(
            "  of exact headroom: {} are span misses k-best could fix ({} not already forgiven as a merge), {} are within-span ranking",
            t.headroom_span, t.headroom_real, t.rank_only
        );
        let mut hist = [0usize; 5];
        for &n in &t.per_sentence {
            hist[n.min(4)] += 1;
        }
        println!(
            "  sentences by unforgiven-but-reachable spans: 0:{} 1:{} 2:{} 3:{} 4+:{}  (max {})",
            hist[0],
            hist[1],
            hist[2],
            hist[3],
            hist[4],
            t.per_sentence.iter().max().copied().unwrap_or(0)
        );
        println!(
            "  non-null misses with no reachable exact candidate: {}  = no span {} + wrong entry {} + forbidden by is_allowed {}",
            t.nonnull - t.ceil_exact,
            t.miss_no_span,
            t.miss_wrong_entry,
            t.miss_forbidden
        );
        println!(
            "  null spans {}: top-1 {}  ceiling {} (fallback held at top-1)",
            t.null, t.null_top1, t.null_ceil
        );
        println!();
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let args: Vec<String> = std::env::args().collect();
        let [_, index_dir, gold_path, dic] = args.as_slice() else {
            return Err("usage: oracle_ceiling <index-dir> <gold.jsonl> <system.dic>".into());
        };
        let table = Table::new();
        let index = Index::open(Path::new(index_dir))?;
        let tokenizer = VibratoTokenizer::load(Path::new(dic))?;
        let (gold, superseded) = read_gold(gold_path)?;
        let opts = ParseOptions::default();

        let mut off = Tally::default();
        let mut on = Tally::default();
        let mut max_len = 0;
        for rec in &gold {
            let chars: Vec<char> = rec.text.chars().collect();
            let by_start = candidates(&index, &table, &chars)?;
            max_len = max_len.max(rec.spans.iter().map(|s| s.len).max().unwrap_or(0));

            let parsed_off = parse(&index, &table, &rec.text, &opts, None)?;
            tally(rec, &chars, &by_start, None, &parsed_off, &mut off, true);

            let bset = tokenizer.boundaries(&rec.text);
            let bounds: &dyn Boundaries = &bset;
            let parsed_on = parse(&index, &table, &rec.text, &opts, Some(bounds))?;
            tally(
                rec,
                &chars,
                &by_start,
                Some(bounds),
                &parsed_on,
                &mut on,
                false,
            );
        }

        println!(
            "sentences {}  superseded {superseded}  gold spans {} (non-null {}, null {})  longest gold span {max_len}\n",
            gold.len(),
            off.spans,
            off.nonnull,
            off.null
        );
        report("off", &off);
        report("on", &on);

        println!(
            "SAMPLE non-null misses, hints off: no candidate at the gold span ({} shown)",
            off.samples_no_span.len()
        );
        for l in &off.samples_no_span {
            println!("{l}");
        }
        println!(
            "\nSAMPLE non-null misses, hints off: span present, gold entry absent ({} shown)",
            off.samples_wrong_entry.len()
        );
        for l in &off.samples_wrong_entry {
            println!("{l}");
        }
        Ok(())
    }
}
