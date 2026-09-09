// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where do the gold boundaries jparser misses actually go?
//!
//! Master plan §9.7's measurement produced a number nobody was looking for:
//! jparser recalls 0.8614 of the corpus's interior gold boundaries while the
//! Vibrato hint source it already loads recalls 0.9707. Roughly 280 gold
//! boundaries reach the engine and are discarded. That gap is six times larger
//! than any difference between tokenizers, so it — not the choice of hint
//! source — is what Phase 1 has to move.
//!
//! "Discarded" is two different defects with two different fixes, and the
//! master plan's §7 line-count estimate splits across nine files without
//! knowing which:
//!
//!   - the span was never a candidate, so no cost model could have chosen it.
//!     That is `matcher.rs` and `stem.rs` work.
//!   - the span was a candidate and the DP ranked it away. That is §4.3's
//!     "hints are advisory where they should be decisive", and it is
//!     `segment.rs` plus the cost model.
//!
//! Those three module names are v1's, and v1 was deleted at Phase 1b-ii. The
//! v2 modules that inherited the two jobs are `engine::lex` (candidate
//! generation) and `engine::lattice` plus `engine::cost` (the DP). The names
//! are left as written because this file records a Phase 0 measurement and
//! the attribution it made at the time.
//!
//! This separates them. For every gold boundary that Vibrato has and jparser
//! lacks, it finds the jparser segment that swallowed it and asks whether the
//! two halves are themselves matchable units.
//!
//! **The probe is `parse` on the substring**, not a raw FST lookup. Reaching
//! into the matcher would mean making it public, and Phase 0 writes nothing
//! under `src/`. Re-parsing `text[a..p]` on its own runs the real matcher,
//! conjugation recursion included, so a deinflected span counts as findable
//! the way `Index::prefixes_of_with` alone would not. It is still a
//! reconstruction:
//! a unit findable in isolation is not proof the lattice offered it in
//! context. Read the cost-model bucket as an upper bound and the lexical-gap
//! bucket as a lower one.
//!
//! **What it found.** The gap is 266 boundaries and most of it is not a
//! defect. Roughly 45% are jparser merging into a JMdict multi-word expression
//! or vs-noun headword — `音がする`, `ことになっている`, `首を振る` — which is
//! §5.2's compound preservation working, penalised because the corpus was
//! adjudicated at the tokenizer's short-unit granularity. Another 24% are
//! ordinary compound nouns and 7% numeral + counter.
//!
//! **The remaining quarter is two real defects.** 13.5% is the fallback path
//! lumping runs of punctuation into one unmatched segment. About 10.5% is
//! subtler and worse: jparser absorbs a preceding particle into the following
//! word and lands on a kana-unified match that is semantically absurd —
//! `は`+`いた` matched as 履く, `も`+`もう` as 揉む, `が`+`いった` as 甲斐,
//! `に`+`なる` as 荷なう. Vibrato has the boundary in every one, which is
//! §4.3's case for making hints decisive rather than advisory.
//!
//! So the headline recall number does not decompose into headroom, and the
//! per-kind lists below are the point rather than the totals. **Read them all
//! — this analysis first reported "every merge is correct" from a sample of
//! about 40 of 230, and the particle-absorbing class is entirely inside the
//! part that sample missed.** The example lists are uncapped for that reason.
//!
//! Run it hints-off, which is how the baseline is measured:
//!
//! ```text
//! cargo run --release -p jparser --example boundary_diag -- \
//!   corpus_staging/gold_index corpus_staging/gold_model.jsonl \
//!   corpus_staging/disagreements.jsonl
//! ```

use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs;

use jparser::engine::table::Table;
use jparser::index::load::Index;
use jparser::{parse, ParseOptions};

#[derive(serde::Deserialize)]
struct GoldSpan {
    start: usize,
}

#[derive(serde::Deserialize)]
struct GoldRecord {
    text: String,
    source: String,
    spans: Vec<GoldSpan>,
}

#[derive(serde::Deserialize)]
struct DiffRecord {
    source: String,
    vibrato: Vec<usize>,
}

/// Which of the three defects swallowed one boundary.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Bucket {
    /// jparser's covering span matched nothing, so it is a fallback run rather
    /// than a dictionary decision. Nothing in the cost model chose this.
    Fallback,
    /// Both halves match on their own: the candidates existed and the DP
    /// preferred the longer one. §4.3.
    CostModel,
    /// At least one half is not a matchable unit, so the split was never on
    /// offer. `matcher.rs` / `stem.rs` under v1; `engine::lex` under v2.
    Lexical,
}

impl Bucket {
    fn label(self) -> &'static str {
        match self {
            Bucket::Fallback => "fallback  (covering span matched nothing)",
            Bucket::CostModel => "cost model(both halves matchable; DP took the longer)",
            Bucket::Lexical => "lexical   (a half is not a matchable unit)",
        }
    }

    /// Short form, for tagging a per-kind example line with its stage.
    fn tag(self) -> &'static str {
        match self {
            Bucket::Fallback => "fall",
            Bucket::CostModel => "cost",
            Bucket::Lexical => "lex ",
        }
    }
}

fn read_jsonl<T: serde::de::DeserializeOwned>(path: &str) -> Result<Vec<T>, Box<dyn Error>> {
    let mut out = Vec::new();
    for line in fs::read_to_string(path)?.lines() {
        if !line.trim().is_empty() {
            out.push(serde_json::from_str(line)?);
        }
    }
    Ok(out)
}

/// Does `parse` cover this exact string with one matched segment?
fn is_unit(index: &Index, table: &Table, s: &str) -> Result<bool, Box<dyn Error>> {
    if s.is_empty() {
        return Ok(false);
    }
    let r = parse(index, table, s, &ParseOptions::default(), None)?;
    Ok(r.segments.len() == 1 && r.segments[0].matched)
}

/// Punctuation, brackets and symbols — anything that is not a letter or digit.
/// The fallback path lumps runs of these into one unmatched segment; gold and
/// Vibrato both split them.
fn is_punct(c: char) -> bool {
    !c.is_alphanumeric()
}

fn slice(chars: &[char], a: usize, b: usize) -> String {
    chars.get(a..b).unwrap_or_default().iter().collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, index_dir, gold_path, diff_path] = args.as_slice() else {
        return Err("usage: boundary_diag <index-dir> <gold.jsonl> <disagreements.jsonl>".into());
    };

    let table = Table::new();
    let index = Index::open(std::path::Path::new(index_dir))?;
    let gold: Vec<GoldRecord> = read_jsonl(gold_path)?;
    let vibrato: HashMap<String, BTreeSet<usize>> = read_jsonl::<DiffRecord>(diff_path)?
        .into_iter()
        .map(|d| (d.source, d.vibrato.into_iter().collect()))
        .collect();

    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut kinds: HashMap<&str, usize> = HashMap::new();
    let mut spans_seen: BTreeSet<(String, usize, usize)> = BTreeSet::new();
    let mut examples: HashMap<&str, Vec<String>> = HashMap::new();
    let (mut gold_interior, mut jparser_hit, mut vibrato_hit, mut targets) = (0, 0, 0, 0);

    for g in &gold {
        let chars: Vec<char> = g.text.chars().collect();
        let n = chars.len();
        let interior = |b: &usize| *b > 0 && *b < n;

        let gb: BTreeSet<usize> = g.spans.iter().map(|s| s.start).filter(interior).collect();
        let parsed = parse(&index, &table, &g.text, &ParseOptions::default(), None)?;
        let jb: BTreeSet<usize> = parsed
            .segments
            .iter()
            .map(|s| s.start)
            .filter(interior)
            .collect();
        let Some(vb) = vibrato.get(&g.source) else {
            continue;
        };

        gold_interior += gb.len();
        jparser_hit += gb.intersection(&jb).count();
        vibrato_hit += gb.intersection(vb).count();

        // The boundaries the hint source hands jparser and jparser drops.
        for &p in gb.iter().filter(|p| vb.contains(p) && !jb.contains(p)) {
            targets += 1;
            // `segments` is a contiguous cover, so a non-boundary is interior
            // to exactly one segment.
            let Some(seg) = parsed
                .segments
                .iter()
                .find(|s| s.start < p && p < s.start + s.len)
            else {
                continue;
            };
            let (a, b) = (seg.start, seg.start + seg.len);

            let bucket = if !seg.matched {
                Bucket::Fallback
            } else if is_unit(&index, &table, &slice(&chars, a, p))?
                && is_unit(&index, &table, &slice(&chars, p, b))?
            {
                Bucket::CostModel
            } else {
                Bucket::Lexical
            };

            *counts.entry(bucket.label()).or_default() += 1;
            spans_seen.insert((g.source.clone(), a, b));
            let head = seg
                .entries
                .first()
                .map(|e| format!("{} [{}]", e.headword, e.entry_id))
                .unwrap_or_else(|| "-".to_string());

            // The buckets above say which stage lost the boundary. This says
            // what it lost the boundary *to*, which is what decides whether
            // losing it was a defect at all — §5.2 wants JMdict compounds
            // preserved, and some of these merges are that behaviour working.
            let kind = if !seg.matched {
                if seg.surface.chars().all(is_punct) {
                    "  fallback: an all-punctuation run kept as one segment"
                } else {
                    "  fallback: a mixed unmatched run"
                }
            } else if seg.surface.chars().any(|c| c.is_numeric()) {
                "  merge: numeral + counter, kept as a numeric/date headword"
            } else if seg
                .entries
                .first()
                .is_some_and(|e| e.pos.iter().any(|p| p == "exp"))
            {
                "  merge: a JMdict multi-word expression (pos=exp)"
            } else if seg
                .entries
                .first()
                .is_some_and(|e| e.pos.iter().any(|p| p.starts_with("vs")))
            {
                "  merge: noun + する, kept as the vs-noun headword"
            } else {
                "  merge: an ordinary longer headword"
            };
            *kinds.entry(kind).or_default() += 1;
            // Keyed by kind, not by stage, and uncapped. Reading a sample and
            // generalising from it is how the first pass of this analysis
            // produced a claim about 230 spans from about 40 of them.
            examples.entry(kind).or_default().push(format!(
                "  {:<14} [{}] {}·{}   kept {:<14} as {}",
                g.source,
                bucket.tag(),
                slice(&chars, a, p),
                slice(&chars, p, b),
                seg.surface,
                head
            ));
        }
    }

    println!(
        "interior gold boundaries: {gold_interior}\n\
         jparser has:              {jparser_hit}  ({:.4})\n\
         vibrato has:              {vibrato_hit}  ({:.4})\n\
         vibrato has, jparser not: {targets}   <- read the breakdown before calling this headroom\n",
        jparser_hit as f64 / gold_interior as f64,
        vibrato_hit as f64 / gold_interior as f64,
    );

    println!(
        "those {targets} boundaries come from {} distinct covering spans\n",
        spans_seen.len()
    );

    let pct = |n: usize| 100.0 * n as f64 / targets as f64;
    let mut rows: Vec<_> = counts.iter().collect();
    rows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    println!("WHICH STAGE LOST IT");
    for (label, n) in &rows {
        println!("{:>5}  {:>5.1}%  {label}", n, pct(**n));
    }

    let mut krows: Vec<_> = kinds.iter().collect();
    krows.sort_by_key(|(_, n)| std::cmp::Reverse(**n));
    println!("\nWHAT IT LOST THE BOUNDARY TO");
    for (label, n) in &krows {
        println!("{:>5}  {:>5.1}%{label}", n, pct(**n));
    }

    for (label, n) in &krows {
        println!("\n=== {} ({n}) ===", label.trim());
        for line in &examples[*label] {
            println!("{line}");
        }
    }
    Ok(())
}
