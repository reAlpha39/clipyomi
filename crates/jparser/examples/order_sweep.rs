// SPDX-License-Identifier: MIT OR Apache-2.0

//! Sweep candidate orderings against the gold corpus's entry top-1.
//!
//! `assemble::order` decides which same-span candidate a reader sees first.
//! It runs *after* the DP has chosen a path, so changing it cannot move a
//! single boundary — which means every ordering can be scored offline from
//! one parse, without a rebuild per variant. That is what this does: parse
//! each gold sentence once, and for every gold span the parse placed exactly
//! and that carries an `ent_seq`, rebuild the same-span candidate group
//! `assemble` would have built and ask each ordering whether it puts the gold
//! entry first.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example order_sweep -- \
//!   gold_index crates/jparser/tests/fixtures/gold.jsonl --hints .../system.dic
//! ```
//!
//! Master plan §5.2 constrains what may go in a key: "a fact JMdict or the
//! match itself supplies, never a frequency table this project does not
//! have". Every variant below obeys that, and
//! `docs/superpowers/2026-09-09-frequency-signal-candidates.md` is why the
//! constraint is not negotiable.

use std::error::Error;
use std::fs;
use std::path::Path;

use jparser::engine::flags::WordFlags;
use jparser::engine::lex::Candidate;
use jparser::engine::table::Table;
use jparser::index::load::Index;
use jparser::{parse, ParseOptions};

#[derive(serde::Deserialize)]
struct GoldSpan {
    start: usize,
    len: usize,
    ent_seq: Option<u32>,
}

#[derive(serde::Deserialize)]
struct GoldRecord {
    text: String,
    spans: Vec<GoldSpan>,
}

fn has(c: &Candidate, f: WordFlags) -> bool {
    c.flags.contains(f)
}

/// What the text around a span says about it. `order` has none of this today
/// -- it sees a candidate and nothing else -- and the whole question below is
/// whether a *contextual* fact separates the cases a context-free prior
/// cannot.
#[derive(Clone, Copy)]
pub struct Ctx {
    /// The character immediately before the span is a digit. `1969|年`.
    prev_is_digit: bool,
}

/// The orderings under test. Variant 0 is what ships.
type Key = (bool, bool, bool, bool, bool, usize, u32);
type Variant = (&'static str, fn(&Candidate, Ctx) -> Key);
const VARIANTS: &[Variant] = &[
    ("0 shipped: exact, COMMON, PRONOUNCE, steps, id", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            has(c, WordFlags::PRONOUNCE),
            false,
            false,
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("1 + COMMON_LINE after COMMON", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !has(c, WordFlags::COMMON_LINE),
            has(c, WordFlags::PRONOUNCE),
            false,
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("2 + COUNTER first among equals", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !has(c, WordFlags::COUNTER),
            has(c, WordFlags::PRONOUNCE),
            false,
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("3 + PARTICLE first among equals", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !has(c, WordFlags::PARTICLE),
            has(c, WordFlags::PRONOUNCE),
            false,
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("4 steps dropped from the key", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            has(c, WordFlags::PRONOUNCE),
            false,
            false,
            0,
            c.entry_id,
        )
    }),
    ("5 deeper chain first (steps reversed)", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            has(c, WordFlags::PRONOUNCE),
            false,
            false,
            usize::MAX - c.steps.len(),
            c.entry_id,
        )
    }),
    ("6 COUNTER + COMMON_LINE", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !has(c, WordFlags::COUNTER),
            !has(c, WordFlags::COMMON_LINE),
            has(c, WordFlags::PRONOUNCE),
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("7 COUNTER + PARTICLE + COMMON_LINE", |c, _x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !(has(c, WordFlags::COUNTER) || has(c, WordFlags::PARTICLE)),
            !has(c, WordFlags::COMMON_LINE),
            has(c, WordFlags::PRONOUNCE),
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("8 COUNTER first only after a digit", |c, x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !(x.prev_is_digit && has(c, WordFlags::COUNTER)),
            has(c, WordFlags::PRONOUNCE),
            false,
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("9 = 8 + PARTICLE first among equals", |c, x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON),
            !(x.prev_is_digit && has(c, WordFlags::COUNTER)),
            !has(c, WordFlags::PARTICLE),
            has(c, WordFlags::PRONOUNCE),
            c.steps.len(),
            c.entry_id,
        )
    }),
    ("10 = 9 + COMMON_LINE", |c, x| {
        (
            !c.exact,
            !has(c, WordFlags::COMMON) && !has(c, WordFlags::COMMON_LINE),
            !(x.prev_is_digit && has(c, WordFlags::COUNTER)),
            !has(c, WordFlags::PARTICLE),
            has(c, WordFlags::PRONOUNCE),
            c.steps.len(),
            c.entry_id,
        )
    }),
];

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (index_dir, gold_path) = match args.as_slice() {
        [i, g, ..] => (i.clone(), g.clone()),
        _ => return Err("usage: order_sweep <index-dir> <gold.jsonl> [--hints <dic>]".into()),
    };
    let hints = args
        .iter()
        .position(|a| a == "--hints")
        .map(|i| args[i + 1].clone());
    let detail = args.iter().any(|a| a == "--detail");

    let index = Index::open(Path::new(&index_dir))?;
    let table = Table::new();
    let tokenizer = match &hints {
        Some(p) => Some(jparser::hints::VibratoTokenizer::load(Path::new(p))?),
        None => None,
    };
    let opts = ParseOptions::default();

    let mut hits = vec![0usize; VARIANTS.len()];
    let mut total = 0usize;
    // Per variant, the surfaces it newly gets right and newly gets wrong
    // against variant 0 -- a net figure alone would hide a wash.
    let mut gained: Vec<Vec<String>> = vec![Vec::new(); VARIANTS.len()];
    let mut lost: Vec<Vec<String>> = vec![Vec::new(); VARIANTS.len()];

    for line in fs::read_to_string(&gold_path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let rec: GoldRecord = serde_json::from_str(line)?;
        let chars: Vec<char> = rec.text.chars().collect();
        let bound_set = tokenizer.as_ref().map(|t| t.boundaries(&rec.text));
        let bounds: Option<&dyn jparser::engine::Boundaries> = bound_set
            .as_ref()
            .map(|b| b as &dyn jparser::engine::Boundaries);
        let parsed = parse(&index, &table, &rec.text, &opts, bounds)?;
        let by_start = jparser::engine::lex::candidates(&index, &table, &chars)?;

        for span in &rec.spans {
            let Some(want) = span.ent_seq else { continue };
            // Strictly placed *and* matched: `score` counts an unmatched
            // exact segment as a top-1 miss, and its entry list is empty, so
            // no ordering can reach it. Those stay in the denominator.
            let Some(seg) = parsed
                .segments
                .iter()
                .find(|s| s.start == span.start && s.len == span.len)
            else {
                continue;
            };
            total += 1;
            if !seg.matched {
                continue;
            }
            let group: Vec<&Candidate> = by_start
                .get(span.start)
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter(|c| c.len == span.len)
                .collect();
            if group.is_empty() {
                continue;
            }
            let surface: String = chars[span.start..span.start + span.len].iter().collect();
            let ctx = Ctx {
                prev_is_digit: span.start > 0
                    && jparser::engine::kana::is_digit(chars[span.start - 1]),
            };
            let mut base_hit = false;
            for (v, (_, key)) in VARIANTS.iter().enumerate() {
                let mut g = group.clone();
                g.sort_by_key(|c| key(c, ctx));
                let hit = g[0].entry_id == want;
                hits[v] += hit as usize;
                if v == 0 {
                    base_hit = hit;
                } else if hit && !base_hit {
                    gained[v].push(surface.clone());
                } else if !hit && base_hit {
                    lost[v].push(surface.clone());
                }
            }
        }
    }

    println!(
        "entry top-1 over {total} gold spans, hints {}",
        if hints.is_some() { "on" } else { "off" }
    );
    for (v, (name, _)) in VARIANTS.iter().enumerate() {
        let d = hits[v] as i64 - hits[0] as i64;
        println!(
            "  {:>4} {:.4}  {:+4}   +{:<3} -{:<3}  {name}",
            hits[v],
            hits[v] as f64 / total as f64,
            d,
            gained[v].len(),
            lost[v].len()
        );
    }
    if detail {
        for (v, (name, _)) in VARIANTS.iter().enumerate().skip(1) {
            println!("\n--- {name}");
            println!("  gained: {:?}", tally(&gained[v]));
            println!("  lost:   {:?}", tally(&lost[v]));
        }
    }
    Ok(())
}

fn tally(v: &[String]) -> Vec<(String, usize)> {
    let mut m: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for s in v {
        *m.entry(s.as_str()).or_default() += 1;
    }
    let mut out: Vec<(String, usize)> = m.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}
