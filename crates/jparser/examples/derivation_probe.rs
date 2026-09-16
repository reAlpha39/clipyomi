// SPDX-License-Identifier: MIT OR Apache-2.0

//! Is there a build-time derivation that separates the adjudicated compound
//! classes, now that frequency is ruled out?
//!
//! `docs/superpowers/2026-09-09-frequency-signal-candidates.md` §10.4 says a
//! build-time derivation "cannot tell `副大統領` from `名の`" — but it tested
//! exactly one derivation, COMMON-if-all-parts-COMMON, and concluded that a
//! frequency rank was what would separate the two groups. §12 then ruled
//! frequency out on both buildable corpora. So the question §10.4 answered
//! for one derivation is open for the rest of them, and the 12 `には`/`にも`
//! merges the `transparent` class scores as misses are the live gap.
//!
//! This probe asks, for each of the 139 adjudicated `(surface, headword)`
//! types, whether the surface admits a full partition into index entries all
//! sharing some property. Each property is a candidate derivation:
//!
//! | property | the rule it would justify |
//! | --- | --- |
//! | every part `PARTICLE` | refuse a merge that is nothing but stacked function words |
//! | every part `COMMON` | §10.4's own derivation, as the reference point |
//! | every part exact | refuse a merge built on a kana-unified match |
//!
//! A derivation is only interesting if it holds for the `transparent` and
//! `false-match` types and **not** for the `lexical` ones. The benefit is
//! bounded at 14 windows whatever happens — `score --compounds` reports
//! `transparent 30 / 42` and `false-match 30 / 32`, so 12 + 2 windows are
//! wrong merges available to block — so the number that decides this is the
//! *cost* on `lexical`, which this prints per type rather than in aggregate.
//!
//! ```text
//! cargo run --release -p jparser --example derivation_probe -- \
//!   <index-dir> crates/jparser/tests/fixtures/compound_gold.jsonl
//! ```

use std::collections::BTreeMap;

use jparser::engine::flags::WordFlags;
use jparser::engine::lex::{self, Candidate};
use jparser::engine::table::Table;
use jparser::index::load::Index;

/// One candidate derivation: a name, and what it demands of every part.
struct Derivation {
    name: &'static str,
    holds: fn(&Candidate) -> bool,
}

const DERIVATIONS: &[Derivation] = &[
    Derivation {
        name: "all parts PARTICLE",
        holds: |c| c.flags.contains(WordFlags::PARTICLE),
    },
    Derivation {
        name: "all parts COMMON",
        holds: |c| c.flags.contains(WordFlags::COMMON),
    },
    Derivation {
        name: "all parts exact",
        holds: |c| c.exact,
    },
    // The conjunction that would matter if either half alone is too broad:
    // stacked function words that are also each independently common.
    Derivation {
        name: "all parts PARTICLE+COMMON",
        holds: |c| c.flags.contains(WordFlags::PARTICLE) && c.flags.contains(WordFlags::COMMON),
    },
];

/// Derivations that also constrain the partition's *last* part.
///
/// §13.5's finding was that a discount reaching `副大統領` also reaches
/// `これは`, and nothing separated them. But those two differ in one visible
/// way: `これは` ends in a `PARTICLE` and `副大統領` does not. `音がする`
/// contains one in the middle and still ends in a verb, so the constraint has
/// to be on the last part rather than on every part.
const TAIL_DERIVATIONS: &[Derivation] = &[Derivation {
    name: "all parts COMMON, last part not PARTICLE",
    holds: |c| c.flags.contains(WordFlags::COMMON),
}];

/// Whether `chars` admits a full cover by candidates that all satisfy `holds`
/// **and** whose final part is not a `PARTICLE`.
///
/// Reachability first, then one question about the last step: for some `k`,
/// is `width - k` reachable by qualifying parts and is there a qualifying,
/// non-`PARTICLE` candidate of length `k` there?
fn splits_into_open_tail(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    holds: fn(&Candidate) -> bool,
) -> bool {
    let len = chars.len();
    if len < 2 {
        return false;
    }
    let mut reachable = vec![false; len + 1];
    reachable[0] = true;
    for p in 0..len {
        if !reachable[p] {
            continue;
        }
        for cand in &by_start[p] {
            if cand.len >= len || p + cand.len > len {
                continue;
            }
            if holds(cand) {
                reachable[p + cand.len] = true;
            }
        }
    }
    (0..len).any(|p| {
        reachable[p]
            && by_start[p].iter().any(|c| {
                c.len < len
                    && p + c.len == len
                    && holds(c)
                    && !c.flags.contains(WordFlags::PARTICLE)
            })
    })
}

/// Whether `chars` admits a full cover by candidates that all satisfy `holds`.
///
/// Reachability over start positions rather than an enumeration of
/// partitions: the question is only whether *some* partition qualifies, and
/// the surfaces here are short but a 7-character one has enough partitions to
/// make enumeration pointless work.
///
/// A single candidate spanning the whole surface is excluded deliberately.
/// The rule any of these derivations would justify is about a merge *against
/// its own split*, so a "partition" that is the merge itself would make every
/// property trivially true of every type that has an entry at all.
fn splits_into(chars: &[char], by_start: &[Vec<Candidate>], holds: fn(&Candidate) -> bool) -> bool {
    let len = chars.len();
    let mut reachable = vec![false; len + 1];
    reachable[0] = true;
    for p in 0..len {
        if !reachable[p] {
            continue;
        }
        for cand in &by_start[p] {
            if cand.len == len {
                continue;
            }
            if holds(cand) {
                reachable[p + cand.len] = true;
            }
        }
    }
    reachable[len]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, index_dir, gold] = args.as_slice() else {
        return Err("usage: derivation_probe <index-dir> <compound_gold.jsonl>".into());
    };
    let index = Index::open(std::path::Path::new(index_dir))?;
    let table = Table::new();

    // Distinct types in file order, with the window count each stands for --
    // the metric is per window, so a type's cost is its window count and not 1.
    let mut types: Vec<(String, String, String)> = Vec::new();
    let mut windows: BTreeMap<(String, String), usize> = BTreeMap::new();
    for line in std::fs::read_to_string(std::path::Path::new(gold))?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)?;
        let s = v["surface"].as_str().unwrap_or_default().to_string();
        let h = v["headword"].as_str().unwrap_or_default().to_string();
        let c = v["class"].as_str().unwrap_or_default().to_string();
        let key = (s.clone(), h.clone());
        if windows.insert(key.clone(), 1).is_none() {
            types.push((s, h, c));
        } else {
            *windows.get_mut(&key).unwrap() += 1;
        }
    }

    for (derivation, tail) in DERIVATIONS
        .iter()
        .map(|d| (d, false))
        .chain(TAIL_DERIVATIONS.iter().map(|d| (d, true)))
    {
        // Types the property holds for, by class, with their window counts.
        let mut hit: BTreeMap<&str, (usize, usize, Vec<String>)> = BTreeMap::new();
        let mut total: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        for (surface, headword, class) in &types {
            let chars: Vec<char> = surface.chars().collect();
            let by_start = lex::candidates(&index, &table, &chars)?;
            let w = windows[&(surface.clone(), headword.clone())];
            let t = total.entry(class.as_str()).or_insert((0, 0));
            t.0 += 1;
            t.1 += w;
            let qualifies = if tail {
                splits_into_open_tail(&chars, &by_start, derivation.holds)
            } else {
                splits_into(&chars, &by_start, derivation.holds)
            };
            if qualifies {
                let e = hit.entry(class.as_str()).or_insert((0, 0, Vec::new()));
                e.0 += 1;
                e.1 += w;
                e.2.push(surface.clone());
            }
        }

        println!("\n### {}", derivation.name);
        println!("  {:14} {:>12} {:>14}", "class", "types", "windows");
        for class in ["lexical", "transparent", "false-match", "ambiguous"] {
            let (ht, hw, _) = hit.get(class).cloned().unwrap_or((0, 0, Vec::new()));
            let (tt, tw) = total.get(class).copied().unwrap_or((0, 0));
            println!("  {class:14} {ht:>5} / {tt:<4} {hw:>7} / {tw:<4}");
        }
        // The cost is the interesting half: a rule that blocks these merges
        // breaks every `lexical` type it holds for, and those are nameable.
        if let Some((_, _, names)) = hit.get("lexical") {
            if !names.is_empty() {
                println!("  lexical types it would break: {}", names.join(" "));
            }
        }
        for class in ["transparent", "false-match"] {
            if let Some((_, _, names)) = hit.get(class) {
                println!("  {class} types it would fix: {}", names.join(" "));
            }
        }
    }

    Ok(())
}
