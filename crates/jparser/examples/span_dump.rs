// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dump one segmentation per input line, as JSONL, so two engine builds can be
//! compared span by span rather than through `score`'s totals.
//!
//! Phase 4 §8's method, and the reason it exists: `score`'s `strict` credits a
//! segment sitting exactly on a gold span **whether or not it matched
//! anything**. A change that turns an unmatched-but-exact hit into a matched
//! span that shatters reads as a pure loss, and one that turns a fragment into
//! a real match reads as nothing at all. The totals cannot tell those apart,
//! and Phase 4's first implementation was misjudged in both directions until
//! the two halves were separated. Decomposing needs the segments themselves.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example span_dump -- \
//!   gold_index pool.tsv --hints .../system.dic > after.jsonl
//! ```
//!
//! `pool.tsv` is `source<TAB>text`, one sentence per line — build it from a
//! gold corpus by joining the `source` and `text` fields, and join the dumps
//! back onto the gold spans by `source`. Output is one object per line:
//!
//! ```text
//! {"source":..,"text":..,"segs":[[start,len,matched],..],
//!  "bounds":[..],"relaxed":[..]}
//! ```
//!
//! `bounds` is the tokenizer's own boundary set and `relaxed` the positions
//! [`jparser::engine::relax::Relaxed`] reopened on top of it, so a diagnostic
//! can ask what the escape bought over each individual hint token rather than
//! over the corpus. Both are empty without `--hints`.

use std::error::Error;
use std::fs;
use std::path::Path;

use jparser::engine::table::Table;
use jparser::engine::Boundaries;
use jparser::index::load::Index;
use jparser::{parse, ParseOptions};

const USAGE: &str = "usage: span_dump <index-dir> <pool.tsv> [--hints <system.dic>]";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (index_dir, pool) = match args.as_slice() {
        [index, pool, ..] => (index.clone(), pool.clone()),
        _ => return Err(USAGE.into()),
    };
    let hints = match args.iter().position(|a| a == "--hints") {
        Some(i) => Some(args.get(i + 1).ok_or(USAGE)?.clone()),
        None => None,
    };

    let index = Index::open(Path::new(&index_dir))?;
    let table = Table::new();
    let tokenizer = match &hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(Path::new(path))?),
        None => None,
    };
    let opts = ParseOptions::default();

    for line in fs::read_to_string(&pool)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let (source, text) = line
            .split_once('\t')
            .ok_or("every line must be source<TAB>text")?;
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let bound_set = tokenizer.as_ref().map(|t| t.boundaries(text));
        let bounds: Option<&dyn Boundaries> = bound_set.as_ref().map(|b| b as &dyn Boundaries);
        let parsed = parse(&index, &table, text, &opts, bounds)?;

        // Recomputed rather than returned by `parse`: the escape is internal
        // and this is a probe, so it pays the second candidate pass instead of
        // widening the engine's own result type.
        let (token_bounds, relaxed): (Vec<usize>, Vec<usize>) = match bounds {
            Some(b) => {
                let by_start = jparser::engine::lex::candidates(&index, &table, &chars)?;
                let r = jparser::engine::relax::Relaxed::over(&chars, &by_start, b);
                (
                    (0..=n).filter(|&p| b.is_boundary(p)).collect(),
                    (0..=n)
                        .filter(|&p| r.is_usable_endpoint(p) && !b.is_boundary(p))
                        .collect(),
                )
            }
            None => (Vec::new(), Vec::new()),
        };

        let segs: Vec<serde_json::Value> = parsed
            .segments
            .iter()
            .map(|s| serde_json::json!([s.start, s.len, s.matched]))
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "source": source,
                "text": text,
                "segs": segs,
                "bounds": token_bounds,
                "relaxed": relaxed,
            })
        );
    }
    Ok(())
}
