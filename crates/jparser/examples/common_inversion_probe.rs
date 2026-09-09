// SPDX-License-Identifier: MIT OR Apache-2.0

//! How much of the gold corpus's compound behaviour is JMdict's inverted
//! priority marker? A throwaway measurement for Phase 3, kept for provenance
//! like `oracle_ceiling` and `ipadic_cost_probe`.
//!
//! Master plan §5.2 and §7 justify Phase 3's frequency layer by one claim:
//! JMdict's priority marker is inverted on the compounds the engine exists
//! to preserve -- 東京都 carries no priority while its parts do -- so
//! `cost::COMMON` (-2 per character on a priority headword) rewards the
//! split. Before a frequency corpus is sourced, this sizes that claim on the
//! corpus itself, in two populations that need to be read together:
//!
//! 1. **The merges `score` forgives** ("coarsening: N gold spans inside M
//!    dictionary merges"): a matched segment whose endpoints are both gold
//!    boundaries and which swallows two or more gold spans. These are the
//!    compounds the engine *did* preserve. For each, is the compound
//!    un-COMMON while its parts are COMMON?
//!
//! 2. **The compounds it declined**: every window between two gold
//!    boundaries, two or more gold spans wide, at which an allowed candidate
//!    exists but the parse placed no segment. Population 1 cannot show the
//!    inversion on its own -- with `CROSS = 0` a compound and its exact split
//!    tie on `COVER` and the tie goes to the longer step, so a compound that
//!    lacks a discount its parts carry is priced *out* and never becomes a
//!    merge. The inverted cases, if they exist, live here.
//!
//! "COMMON" for a span means the cheapest allowed candidate at exactly that
//! span carries `WordFlags::COMMON`, which is the candidate the DP prices
//! (same-key nodes keep the cheaper). A part with no allowed candidate at all
//! is reported separately: it has nothing to be COMMON. For a declined window
//! the probe also asks what a COMMON discount on the compound would have
//! done: the compound wins the window iff its discounted cost is at most what
//! the parse actually paid across it (tie -> longer step), measured only when
//! the parse's segments tile the window exactly.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example common_inversion_probe -- \
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
    probe::run()
}

#[cfg(feature = "mecab")]
mod probe {
    use std::collections::{BTreeSet, HashMap};
    use std::error::Error;
    use std::path::Path;

    use jparser::engine::cost::{cost, is_allowed, unmatched_cost};
    use jparser::engine::lex::{candidates, Candidate};
    use jparser::engine::table::Table;
    use jparser::engine::Boundaries;
    use jparser::hints::VibratoTokenizer;
    use jparser::index::load::Index;
    use jparser::{parse, ParseOptions, ParseResult, WordFlags};

    #[derive(serde::Deserialize)]
    struct GoldSpan {
        start: usize,
        len: usize,
    }

    #[derive(serde::Deserialize)]
    struct GoldRecord {
        text: String,
        source: String,
        spans: Vec<GoldSpan>,
    }

    fn read_gold(path: &str) -> Result<Vec<GoldRecord>, Box<dyn Error>> {
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
        let mut out: Vec<GoldRecord> = all
            .into_iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r)
            .collect();
        for r in &mut out {
            r.spans.sort_by_key(|s| s.start);
        }
        Ok(out)
    }

    fn slice(chars: &[char], a: usize, b: usize) -> String {
        chars.get(a..b).unwrap_or_default().iter().collect()
    }

    /// The candidate the DP prices at exactly `(start, len)`: the cheapest
    /// allowed one, with its cost.
    fn cheapest<'a>(
        by_start: &'a [Vec<Candidate>],
        start: usize,
        len: usize,
        bounds: &dyn Boundaries,
    ) -> Option<(&'a Candidate, i32)> {
        by_start
            .get(start)?
            .iter()
            .filter(|c| c.len == len && is_allowed(c, Some(bounds)))
            .map(|c| (c, cost(c, Some(bounds))))
            .min_by_key(|&(_, k)| k)
    }

    /// What `cost` would charge the same candidate if it carried COMMON.
    fn discounted(c: &Candidate, bounds: &dyn Boundaries) -> i32 {
        let mut with = c.clone();
        with.flags.insert(WordFlags::COMMON);
        cost(&with, Some(bounds))
    }

    /// Gold spans inside `[a, b)`.
    fn inside(rec: &GoldRecord, a: usize, b: usize) -> impl Iterator<Item = &GoldSpan> {
        rec.spans
            .iter()
            .filter(move |s| a <= s.start && s.start + s.len <= b)
    }

    /// COMMON status of every gold span inside `[a, b)`: `Some(true)` the
    /// DP's candidate there is COMMON, `Some(false)` it is not, `None` no
    /// allowed candidate sits on the span at all.
    fn parts(
        rec: &GoldRecord,
        by_start: &[Vec<Candidate>],
        a: usize,
        b: usize,
        bounds: &dyn Boundaries,
    ) -> Vec<Option<bool>> {
        inside(rec, a, b)
            .map(|s| {
                cheapest(by_start, s.start, s.len, bounds)
                    .map(|(c, _)| c.flags.contains(WordFlags::COMMON))
            })
            .collect()
    }

    fn mark(p: &Option<bool>) -> &'static str {
        match p {
            Some(true) => "C",
            Some(false) => "-",
            None => "0",
        }
    }

    fn parts_text(
        rec: &GoldRecord,
        chars: &[char],
        a: usize,
        b: usize,
        ps: &[Option<bool>],
    ) -> String {
        inside(rec, a, b)
            .zip(ps)
            .map(|(s, p)| format!("{}{}", slice(chars, s.start, s.start + s.len), mark(p)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The four cells the question is about, plus the parts nobody could
    /// have tagged.
    #[derive(Default)]
    struct Cells {
        n: usize,
        /// Compound lacks COMMON, at least one part has it: the inversion.
        inverted: usize,
        /// Compound lacks COMMON, every part has it: the strict form.
        inverted_all: usize,
        lacks_lacks: usize,
        has_has: usize,
        has_lacks: usize,
        /// Windows where some part has no candidate at all.
        with_bare_part: usize,
        inexact_compound: usize,
    }

    impl Cells {
        fn add(&mut self, compound_common: bool, exact: bool, parts: &[Option<bool>]) {
            self.n += 1;
            let any = parts.contains(&Some(true));
            let all = parts.iter().all(|p| *p == Some(true));
            match (compound_common, any) {
                (false, true) => {
                    self.inverted += 1;
                    if all {
                        self.inverted_all += 1;
                    }
                }
                (false, false) => self.lacks_lacks += 1,
                (true, true) => self.has_has += 1,
                (true, false) => self.has_lacks += 1,
            }
            if parts.iter().any(Option::is_none) {
                self.with_bare_part += 1;
            }
            if !exact {
                self.inexact_compound += 1;
            }
        }

        fn print(&self) {
            println!(
                "  compound lacks COMMON, a part has it (INVERTED): {:>3}   (every part: {})",
                self.inverted, self.inverted_all
            );
            println!(
                "  compound lacks COMMON, no part has it:           {:>3}",
                self.lacks_lacks
            );
            println!(
                "  compound has COMMON, a part has it:              {:>3}",
                self.has_has
            );
            println!(
                "  compound has COMMON, no part has it:             {:>3}",
                self.has_lacks
            );
            println!(
                "  windows with a part that has no candidate at all: {}   inexact compound: {}",
                self.with_bare_part, self.inexact_compound
            );
        }
    }

    /// Per declined window: how the parse actually covered it.
    #[derive(Default)]
    struct Declined {
        cells: Cells,
        /// Every gold span inside is a strict hit -- the short-unit gold's
        /// own answer.
        gold_split: usize,
        /// Not the gold split, but every inner span is strict or forgiven by
        /// a smaller merge.
        sub_merge: usize,
        /// Some inner span is a real miss.
        miss: usize,
        /// Windows the parse tiles exactly, so the flip test is meaningful.
        tiled: usize,
        /// Of `tiled`: a COMMON discount on the compound would have won the
        /// window, split by what it would have replaced.
        flip_gold_split: usize,
        flip_sub_merge: usize,
        flip_miss: usize,
        /// Of `tiled` and INVERTED: same flip split.
        inv_tiled: usize,
        inv_flip_gold_split: usize,
        inv_flip_other: usize,
        /// Distinct gold spans (by source and start) inside flipped windows,
        /// so nested windows do not double count: the strict spans a COMMON
        /// derivation would put at risk, and the spans it could newly forgive.
        flip_gold_split_spans: BTreeSet<(String, usize)>,
        flip_other_spans: BTreeSet<(String, usize)>,
        samples: Vec<String>,
    }

    /// Strict/forgiven status of the gold spans inside `[a, b)`, using
    /// `score::fit`'s two rules.
    fn window_outcome(
        rec: &GoldRecord,
        parsed: &ParseResult,
        gb: &BTreeSet<usize>,
        a: usize,
        b: usize,
    ) -> &'static str {
        let mut all_strict = true;
        let mut all_ok = true;
        for s in inside(rec, a, b) {
            let strict = parsed
                .segments
                .iter()
                .any(|j| j.start == s.start && j.len == s.len);
            let forgiven = strict
                || parsed.segments.iter().any(|j| {
                    j.matched
                        && j.start <= s.start
                        && s.start + s.len <= j.start + j.len
                        && gb.contains(&j.start)
                        && gb.contains(&(j.start + j.len))
                });
            all_strict &= strict;
            all_ok &= forgiven;
        }
        if all_strict {
            "gold split"
        } else if all_ok {
            "sub-merge"
        } else {
            "miss"
        }
    }

    /// What the parse paid across `[a, b)`, if its segments tile the window.
    fn window_cost(
        parsed: &ParseResult,
        chars: &[char],
        by_start: &[Vec<Candidate>],
        bounds: &dyn Boundaries,
        a: usize,
        b: usize,
    ) -> Option<i32> {
        let inside: Vec<_> = parsed
            .segments
            .iter()
            .filter(|j| a <= j.start && j.start + j.len <= b)
            .collect();
        let covered: usize = inside.iter().map(|j| j.len).sum();
        if covered != b - a {
            return None;
        }
        let mut total = 0;
        for j in inside {
            total += if j.matched {
                cheapest(by_start, j.start, j.len, bounds)?.1
            } else {
                unmatched_cost(chars, j.start, j.len)
            };
        }
        Some(total)
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        let args: Vec<String> = std::env::args().collect();
        let [_, index_dir, gold_path, dic] = args.as_slice() else {
            return Err(
                "usage: common_inversion_probe <index-dir> <gold.jsonl> <system.dic>".into(),
            );
        };
        let table = Table::new();
        let index = Index::open(Path::new(index_dir))?;
        let tokenizer = VibratoTokenizer::load(Path::new(dic))?;
        let gold = read_gold(gold_path)?;
        let opts = ParseOptions::default();

        let mut merges = Cells::default();
        let mut forgiven_spans = 0;
        let mut merge_lines: Vec<String> = Vec::new();
        let mut declined = Declined::default();
        let mut gold_spans = 0;

        for rec in &gold {
            let chars: Vec<char> = rec.text.chars().collect();
            let by_start = candidates(&index, &table, &chars)?;
            let bset = tokenizer.boundaries(&rec.text);
            let bounds: &dyn Boundaries = &bset;
            let parsed = parse(&index, &table, &rec.text, &opts, Some(bounds))?;
            gold_spans += rec.spans.len();

            let mut gb: BTreeSet<usize> = rec.spans.iter().map(|s| s.start).collect();
            if let Some(last) = rec.spans.last() {
                gb.insert(last.start + last.len);
            }
            let ends: Vec<usize> = gb.iter().copied().collect();

            // Population 1: the merges `score` forgives.
            for j in &parsed.segments {
                let (a, b) = (j.start, j.start + j.len);
                if !j.matched || !gb.contains(&a) || !gb.contains(&b) {
                    continue;
                }
                let ps = parts(rec, &by_start, a, b, bounds);
                if ps.len() < 2 {
                    continue;
                }
                forgiven_spans += ps.len();
                let (c, k) = cheapest(&by_start, a, b - a, bounds)
                    .expect("a matched segment has a candidate");
                let cc = c.flags.contains(WordFlags::COMMON);
                merges.add(cc, c.exact, &ps);
                merge_lines.push(format!(
                    "  {:<14} {:<12} {} cost {:>3} {:<7}  parts {}",
                    rec.source,
                    j.surface,
                    if cc { "C" } else { "-" },
                    k,
                    if c.exact { "exact" } else { "inexact" },
                    parts_text(rec, &chars, a, b, &ps)
                ));
            }

            // Population 2: windows with an allowed compound the parse declined.
            for (i, &a) in ends.iter().enumerate() {
                for &b in ends.iter().skip(i + 2) {
                    if parsed
                        .segments
                        .iter()
                        .any(|j| j.start == a && j.start + j.len == b)
                    {
                        continue;
                    }
                    let Some((c, k)) = cheapest(&by_start, a, b - a, bounds) else {
                        continue;
                    };
                    let ps = parts(rec, &by_start, a, b, bounds);
                    let cc = c.flags.contains(WordFlags::COMMON);
                    declined.cells.add(cc, c.exact, &ps);
                    let inverted = !cc && ps.contains(&Some(true));
                    let outcome = window_outcome(rec, &parsed, &gb, a, b);
                    match outcome {
                        "gold split" => declined.gold_split += 1,
                        "sub-merge" => declined.sub_merge += 1,
                        _ => declined.miss += 1,
                    }
                    let paid = window_cost(&parsed, &chars, &by_start, bounds, a, b);
                    let flips = paid.map(|p| discounted(c, bounds) <= p);
                    if paid.is_some() {
                        declined.tiled += 1;
                    }
                    if flips == Some(true) {
                        match outcome {
                            "gold split" => declined.flip_gold_split += 1,
                            "sub-merge" => declined.flip_sub_merge += 1,
                            _ => declined.flip_miss += 1,
                        }
                    }
                    if inverted && paid.is_some() {
                        declined.inv_tiled += 1;
                        if flips == Some(true) {
                            if outcome == "gold split" {
                                declined.inv_flip_gold_split += 1;
                            } else {
                                declined.inv_flip_other += 1;
                            }
                        }
                    }
                    if flips == Some(true) {
                        let set = match outcome {
                            "gold split" => &mut declined.flip_gold_split_spans,
                            _ => &mut declined.flip_other_spans,
                        };
                        for s in inside(rec, a, b) {
                            set.insert((rec.source.clone(), s.start));
                        }
                    }
                    {
                        declined.samples.push(format!(
                            "  {:<4} {:<4} {:<14} {:<12} [{}] cost {:>3} paid {:<4} {:<10} {:<7} parts {}",
                            if inverted { "INV" } else if cc { "C/C" } else { "-/-" },
                            match flips {
                                Some(true) => "flip",
                                Some(false) => "stay",
                                None => "n/a",
                            },
                            rec.source,
                            slice(&chars, a, b),
                            c.headword,
                            k,
                            paid.map(|p| p.to_string())
                                .unwrap_or_else(|| "n/a".into()),
                            outcome,
                            if c.exact { "exact" } else { "inexact" },
                            parts_text(rec, &chars, a, b, &ps)
                        ));
                    }
                }
            }
        }

        println!(
            "sentences {}  gold spans {gold_spans}  merges {} forgiving {forgiven_spans} spans  (score's coarsening line must agree)\n",
            gold.len(),
            merges.n
        );
        println!("== 1. merges the parse made (compound WON) ==");
        merges.print();
        println!("\n  every merge  (C = COMMON, - = candidate without, 0 = no candidate at the gold span):");
        for l in &merge_lines {
            println!("{l}");
        }

        println!("\n== 2. windows with an allowed compound the parse DECLINED ==");
        println!(
            "  windows {}   (nested windows each count once; a window spans >= 2 gold spans)",
            declined.cells.n
        );
        declined.cells.print();
        println!(
            "  what the parse did instead: gold split {}  sub-merge {}  miss {}",
            declined.gold_split, declined.sub_merge, declined.miss
        );
        println!(
            "  tiled windows {}: a COMMON discount on the compound would flip {} -- replacing gold split {}, sub-merge {}, miss {}",
            declined.tiled,
            declined.flip_gold_split + declined.flip_sub_merge + declined.flip_miss,
            declined.flip_gold_split,
            declined.flip_sub_merge,
            declined.flip_miss
        );
        println!(
            "  INVERTED and tiled {}: flip {} -- replacing gold split {}, other {}",
            declined.inv_tiled,
            declined.inv_flip_gold_split + declined.inv_flip_other,
            declined.inv_flip_gold_split,
            declined.inv_flip_other
        );
        println!(
            "  distinct gold spans inside flipped windows: {} now strict (at risk), {} now unforgiven (could be forgiven)",
            declined.flip_gold_split_spans.len(),
            declined.flip_other_spans.len()
        );
        println!(
            "\n  every declined window (INV = inverted, C/C = both COMMON, -/- = neither; flip = compound wins with a COMMON discount):",
        );
        for l in &declined.samples {
            println!("{l}");
        }
        Ok(())
    }
}
