// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! The adjudicator, per design §7.
//!
//! **Candidates are listed by `ent_seq`, not by rank.** Show them in the
//! parser's ranked order and the adjudicator reads top-down, accepts the
//! plausible first candidate, and gold `ent_seq` becomes a copy of the answer
//! under measurement — entry top-1 then reads near 1.0 by construction and the
//! baseline is a tautology that looks like a good result. Sorting by `ent_seq`
//! decorrelates list position from what the parser preferred, so position
//! carries no signal. Nothing printed reveals rank either.
//!
//! Input is read a line at a time rather than as raw keystrokes. A raw-mode
//! reader needs a TTY and a TTY cannot be driven from a test; one Enter per
//! decision buys an adjudicator whose every branch is covered by
//! `tests/cli_label.rs` piping a decision script. No new dependency.

use std::error::Error;
use std::io::{BufRead, Write};
use std::path::Path;

use jparser::index::load::Index;
use jparser::{ParseOptions, ParseResult};

use crate::corpus::{append, read_diffs, validate, GoldRecord, GoldSpan};

/// Consecutive cuts to `(start, len)` spans.
pub fn spans_from_cuts(cuts: &[usize]) -> Vec<(usize, usize)> {
    cuts.windows(2).map(|w| (w[0], w[1] - w[0])).collect()
}

/// The `e` command's cut list: interior cuts only, with 0 and `chars` implied.
///
/// One primitive for splitting, merging, and moving a boundary — retyping the
/// cut list does all three, and three separate commands would be three times
/// the surface for no more capability.
pub fn parse_cuts(line: &str, chars: usize) -> Result<Vec<usize>, String> {
    let mut cuts = vec![0];
    for field in line.split_whitespace() {
        let cut: usize = field
            .parse()
            .map_err(|_| format!("{field:?} is not a char offset"))?;
        if cut == 0 || cut >= chars {
            return Err(format!(
                "{cut} is not an interior cut: 0 and {chars} are implied"
            ));
        }
        cuts.push(cut);
    }
    cuts.push(chars);
    cuts.sort_unstable();
    cuts.dedup();
    Ok(cuts)
}

/// `(ent_seq, one-line label)` for every entry the index offers on this exact
/// span, sorted ascending by `ent_seq`.
///
/// The sort is the tautology guard (see the module doc). It also makes the list
/// stable across runs, which a rank-ordered list would not be once the ranker
/// changes in Phase 1.
///
/// Candidates come from parsing the span's own substring, not from the
/// whole-sentence parse: an adjudicator who retyped the cuts has spans the
/// sentence parse never produced. A substring parse that does not come back as
/// one span covering the whole substring offers nothing, and `#<ent_seq>`
/// covers that case.
pub fn candidates(
    index: &Index,
    table: &jparser::engine::table::Table,
    text: &str,
    start: usize,
    len: usize,
) -> Result<Vec<(u32, String)>, Box<dyn Error>> {
    let span_text: String = text.chars().skip(start).take(len).collect();
    let parsed: ParseResult =
        jparser::parse(index, table, &span_text, &ParseOptions::default(), None)?;
    let mut out: Vec<(u32, String)> = match parsed.segments.as_slice() {
        [seg] if seg.start == 0 && seg.len == len => seg
            .entries
            .iter()
            .map(|e| {
                let gloss = e
                    .senses
                    .first()
                    .map(|s| s.glosses.join("; "))
                    .unwrap_or_default();
                let reading = e.reading.as_deref().unwrap_or("-");
                (e.entry_id, format!("{} ({reading}) {gloss}", e.headword))
            })
            .collect(),
        _ => Vec::new(),
    };
    out.sort_by_key(|(id, _)| *id);
    out.dedup_by_key(|(id, _)| *id);
    Ok(out)
}

/// Render a cut list as `a|b|c` over the text, for the two-parse display.
fn render_spans(text: &str, cuts: &[usize]) -> String {
    let chars: Vec<char> = text.chars().collect();
    spans_from_cuts(cuts)
        .iter()
        .map(|&(start, len)| chars[start..start + len].iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("|")
}

/// Read one line, trimmed. `None` at end of input.
fn ask(prompt: &str) -> Option<String> {
    print!("{prompt}");
    // Unbuffered prompts would appear after the answer under a pipe.
    std::io::stdout().flush().ok()?;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// Adjudicate `diff_path` into `gold_path`, appending as it goes.
pub fn run(index_dir: &Path, diff_path: &Path, gold_path: &Path) -> Result<(), Box<dyn Error>> {
    let table = jparser::engine::table::Table::new();
    let index = Index::open(index_dir)?;
    let records = read_diffs(diff_path)?;
    let total = records.len();

    for (i, record) in records.iter().enumerate() {
        let chars = record.text.chars().count();
        println!(
            "\n[{}/{total}] source={} unknown_tokens={}",
            i + 1,
            record.source,
            record.unknown_tokens
        );
        println!("{}", record.text);
        println!("j: {}", render_spans(&record.text, &record.jparser));
        println!("v: {}", render_spans(&record.text, &record.vibrato));
        println!("   conflicts at {:?}", record.conflicts);

        let (cuts, decision) = loop {
            let Some(answer) = ask("[j] jparser  [v] vibrato  [e] edit  [s] skip  [q] quit > ")
            else {
                return Ok(());
            };
            match answer.as_str() {
                "j" => break (record.jparser.clone(), Some("j".to_string())),
                "v" => break (record.vibrato.clone(), Some("v".to_string())),
                "s" => break (Vec::new(), None),
                "q" => return Ok(()),
                "e" => {
                    let Some(line) =
                        ask(&format!("cuts (interior only, 0 and {chars} implied) > "))
                    else {
                        return Ok(());
                    };
                    match parse_cuts(&line, chars) {
                        Ok(cuts) => break (cuts, Some("e".to_string())),
                        Err(e) => println!("  {e}"),
                    }
                }
                other => println!("  {other:?} is not one of j v e s q"),
            }
        };
        if cuts.is_empty() {
            continue;
        }

        let mut spans = Vec::new();
        for (start, len) in spans_from_cuts(&cuts) {
            let surface: String = record.text.chars().skip(start).take(len).collect();
            println!("\n  span {start}..{} 「{surface}」", start + len);
            let cands = candidates(&index, &table, &record.text, start, len)?;
            for (n, (id, label)) in cands.iter().enumerate() {
                println!("    {}) {id} {label}", n + 1);
            }
            if cands.is_empty() {
                println!("    (no candidate on this span)");
            }
            let ent_seq = loop {
                let Some(answer) =
                    ask("  [1-N] pick  [n] no entry  [#<ent_seq>] by id > ")
                else {
                    return Ok(());
                };
                if answer == "n" {
                    break None;
                }
                if let Some(rest) = answer.strip_prefix('#') {
                    match rest.parse::<u32>() {
                        Ok(id) => match index.entry(id)? {
                            Some(_) => break Some(id),
                            None => {
                                println!("    {id} is not in this index");
                                continue;
                            }
                        },
                        Err(_) => {
                            println!("    {rest:?} is not an ent_seq");
                            continue;
                        }
                    }
                }
                match answer.parse::<usize>() {
                    Ok(n) if (1..=cands.len()).contains(&n) => break Some(cands[n - 1].0),
                    _ => println!("    pick 1-{}, n, or #<ent_seq>", cands.len()),
                }
            };
            spans.push(GoldSpan {
                start,
                len,
                ent_seq,
            });
        }

        let gold = GoldRecord {
            text: record.text.clone(),
            source: record.source.clone(),
            spans,
            decision,
        };
        // Validate before appending: an invalid record in the gold file is a
        // hard error at score time, and catching it here costs one check.
        validate(&gold)?;
        append(gold_path, &gold)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-label-unit-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Pins the tautology guard directly: two candidates on one span, built
    /// so jparser's own ranked order and ent_seq-ascending order disagree.
    ///
    /// A plain tie is not discriminating here — `engine::assemble::order`
    /// breaks its last tie on `entry_id`, so two candidates with identical
    /// flags already come back ent_seq-ascending with no help from
    /// `candidates`'s own sort. `re_pri` on the *higher* ent_seq breaks that:
    /// it marks that entry `COMMON`, which `order` ranks ahead of an unmarked
    /// entry, so jparser's ranked order becomes `[2000002, 2000001]` while
    /// ent_seq order is `[2000001, 2000002]`. Only `candidates`'s
    /// `out.sort_by_key(|(id, _)| *id)` produces the latter; delete that line
    /// and this test fails.
    #[test]
    fn candidates_are_sorted_by_ent_seq_not_by_jparsers_rank() {
        const XML: &str = concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            "<JMdict>",
            "<entry><ent_seq>2000002</ent_seq>",
            "<r_ele><reb>かみ</reb><re_pri>news1</re_pri></r_ele>",
            "<sense><pos>&n;</pos><gloss>god</gloss></sense></entry>",
            "<entry><ent_seq>2000001</ent_seq>",
            "<r_ele><reb>かみ</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>hair</gloss></sense></entry>",
            "</JMdict>",
        );

        let table = jparser::engine::table::Table::new();
        let dir = scratch("ent-seq-not-rank");
        let report = jparser::engine::build::build_from_reader(XML.as_bytes(), &dir)
            .expect("the fixture must build");
        assert_eq!(
            report.skipped_entries, 0,
            "the fixture must not be malformed"
        );
        let index = Index::open(&dir).expect("open the built index");

        let ids: Vec<u32> = candidates(&index, &table, "かみ", 0, 2)
            .expect("candidates must parse the span")
            .into_iter()
            .map(|(id, _)| id)
            .collect();

        assert_eq!(
            ids,
            vec![2000001, 2000002],
            "ent_seq ascending, not jparser's ranked order"
        );
    }

    #[test]
    fn cuts_become_spans() {
        assert_eq!(spans_from_cuts(&[0, 3, 4, 7]), vec![(0, 3), (3, 1), (4, 3)]);
    }

    #[test]
    fn two_cuts_are_one_span() {
        assert_eq!(spans_from_cuts(&[0, 2]), vec![(0, 2)]);
    }

    #[test]
    fn fewer_than_two_cuts_is_no_spans() {
        assert_eq!(spans_from_cuts(&[0]), Vec::<(usize, usize)>::new());
        assert_eq!(spans_from_cuts(&[]), Vec::<(usize, usize)>::new());
    }

    /// The `e` command takes interior cuts only; 0 and the char length are
    /// implied, so splitting, merging, and moving a boundary are all one
    /// primitive rather than three commands.
    #[test]
    fn an_edit_line_adds_the_implied_outer_cuts() {
        assert_eq!(parse_cuts("3 4", 7), Ok(vec![0, 3, 4, 7]));
    }

    #[test]
    fn an_empty_edit_line_is_one_span_over_the_whole_text() {
        assert_eq!(parse_cuts("", 7), Ok(vec![0, 7]));
    }

    #[test]
    fn edit_cuts_are_sorted_and_deduplicated() {
        assert_eq!(parse_cuts("4 3 3", 7), Ok(vec![0, 3, 4, 7]));
    }

    #[test]
    fn a_cut_outside_the_text_is_rejected() {
        assert!(parse_cuts("9", 7).is_err());
        assert!(parse_cuts("0", 7).is_err(), "0 is implied, not an interior cut");
        assert!(parse_cuts("7", 7).is_err(), "the length is implied too");
    }

    #[test]
    fn a_non_numeric_edit_line_is_rejected() {
        assert!(parse_cuts("3 x", 7).is_err());
    }
}
