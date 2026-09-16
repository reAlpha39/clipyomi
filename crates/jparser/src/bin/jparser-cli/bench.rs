// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! Wall-time at clipboard length, against a recorded baseline.
//!
//! Master plan §5.2 records the gap this closes in its own words: "Every
//! metric here is measured at corpus sentence lengths, and nothing in the
//! harness measures a clipboard-length input — which is why a superlinearity
//! that made the app's own 10,000-character ceiling unusable survived four
//! phases (Phase 2c). A timing figure inside the harness, compared against a
//! recorded baseline, is still not built."
//!
//! So this is deliberately *not* a timer bolted onto `score`. Timing the
//! 150-sentence corpus would reproduce exactly the blindness that let Phase
//! 2c's defect through, because the defect was invisible at corpus sentence
//! lengths and fatal at paste lengths. The subject here is a paste.
//!
//! **The cell worth recording is `scaling`, not the milliseconds.** Absolute
//! times move with the machine; the ratio of the time ratio to the length
//! ratio does not. Linear is 1.00 whatever the hardware, and Phase 2c's
//! once-per-position index walk would have read near [`SHORT`]'s own
//! multiple of [`LONG`] — roughly 10 — which is a number no one could have
//! mistaken for noise.
//!
//! `score`'s output is untouched on purpose: the eight recorded baseline
//! cells are transcribed from it and pinned by a test, so a new metric goes
//! beside it rather than into it.

use std::error::Error;
use std::path::Path;
use std::time::{Duration, Instant};

use jparser::index::load::Index;
use jparser::ParseOptions;

use crate::corpus::read_gold;

/// The app's own input ceiling (`src-tauri`'s per-copy cap), so the long
/// point measures the worst case a user can actually produce rather than an
/// arbitrary large number.
const LONG: usize = 10_000;

/// The short point, an order of magnitude down. Two points are what a
/// scaling figure needs; a third would buy a curve nothing reads.
const SHORT: usize = 1_000;

/// Field width for the label column, matching `score`'s own reports.
const LABEL_WIDTH: usize = 18;

/// One length's timing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Point {
    /// Characters actually pasted. Whole sentences only, so this lands at or
    /// just above the target rather than exactly on it.
    pub chars: usize,
    pub median_ms: u128,
    pub min_ms: u128,
    pub max_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Timings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_generation: Option<String>,
    pub hints: bool,
    pub readings: usize,
    pub runs: usize,
    pub points: Vec<Point>,
    /// Time ratio divided by length ratio, in parts per hundred so the cell
    /// is an integer a baseline can be compared against exactly. 100 is
    /// linear; Phase 2c's defect would have read near 1,000.
    pub scaling_centi: u128,
}

/// A paste of at least `target` characters, built by cycling the corpus
/// sentences in file order.
///
/// Deterministic from a committed fixture, which is what makes the figure a
/// baseline rather than an anecdote — no paste file to keep beside the
/// repository, and no sampling to reproduce. Whole sentences only: truncating
/// mid-sentence would put a partial word at the end and make the last parse
/// step depend on where the cut fell.
fn paste(texts: &[&str], target: usize) -> String {
    assert!(!texts.is_empty(), "the corpus has no sentences to cycle");
    let mut out = String::new();
    let mut chars = 0;
    for text in texts.iter().cycle() {
        out.push_str(text);
        chars += text.chars().count();
        if chars >= target {
            break;
        }
    }
    out
}

/// The upper-middle element for an even `runs`, which is what keeps the
/// figure deterministic; the choice of side is arbitrary and pinned by
/// `median_is_the_upper_middle_so_an_even_run_count_is_deterministic` below.
fn median(mut sorted: Vec<Duration>) -> Duration {
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

/// Time ratio over length ratio, in parts per hundred. 100 is linear.
///
/// Guarded against a zero short-point time, which a fast machine and a
/// coarse clock can produce together: with nothing to divide by there is no
/// scaling figure, and 0 says so rather than panicking or reporting a
/// fabricated 100.
fn scaling_centi(short: &Point, long: &Point) -> u128 {
    let Some(time_ratio) = (long.median_ms * 100).checked_div(short.median_ms) else {
        return 0;
    };
    let Some(char_ratio) = (long.chars * 100).checked_div(short.chars) else {
        return 0;
    };
    (time_ratio * 100)
        .checked_div(char_ratio as u128)
        .unwrap_or(0)
}

pub fn run(
    index_dir: &Path,
    gold_path: &Path,
    #[cfg(feature = "mecab")] hints: Option<&Path>,
    readings: usize,
    runs: usize,
    json: bool,
) -> Result<(), Box<dyn Error>> {
    let table = jparser::engine::table::Table::new();
    let index = Index::open(index_dir)?;
    let (records, _superseded) = read_gold(gold_path)?;
    let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();

    #[cfg(feature = "mecab")]
    let tokenizer = match hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
        None => None,
    };
    #[cfg(feature = "mecab")]
    let hints_used = tokenizer.is_some();
    #[cfg(not(feature = "mecab"))]
    let hints_used = false;

    let opts = ParseOptions::with_readings(readings);
    let mut points = Vec::new();
    for target in [SHORT, LONG] {
        let text = paste(&texts, target);
        let mut elapsed = Vec::with_capacity(runs);
        for _ in 0..runs {
            // Tokenization is inside the timed region because the app pays it
            // per paste too: hints-on latency is what the boundary set costs
            // plus what the DP costs, and a change that moved cost from one
            // to the other would otherwise read as free.
            let start = Instant::now();
            #[cfg(feature = "mecab")]
            let bound_set = tokenizer.as_ref().map(|t| t.boundaries(&text));
            #[cfg(feature = "mecab")]
            let bounds: Option<&dyn jparser::engine::Boundaries> = bound_set
                .as_ref()
                .map(|b| b as &dyn jparser::engine::Boundaries);
            #[cfg(not(feature = "mecab"))]
            let bounds: Option<&dyn jparser::engine::Boundaries> = None;
            let parsed = jparser::parse(&index, &table, &text, &opts, bounds)?;
            elapsed.push(start.elapsed());
            // Nothing reads the parse, and a release build is free to notice.
            std::hint::black_box(&parsed);
        }
        let min = *elapsed.iter().min().expect("runs >= 1");
        let max = *elapsed.iter().max().expect("runs >= 1");
        points.push(Point {
            chars: text.chars().count(),
            median_ms: median(elapsed).as_millis(),
            min_ms: min.as_millis(),
            max_ms: max.as_millis(),
        });
    }

    let t = Timings {
        index_generation: index_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned()),
        hints: hints_used,
        readings,
        runs,
        scaling_centi: scaling_centi(&points[0], &points[1]),
        points,
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&t)?);
    } else {
        print!("{}", render(&t));
    }
    Ok(())
}

/// The human form, as a string rather than a series of `println!`s.
///
/// Returned rather than printed for the reason `score::render` is: the format
/// is the thing a recorded baseline is transcribed from, and a function that
/// prints can only be tested by capturing stdout. Same signature shape and
/// the same `print!` at the one call site above.
fn render(t: &Timings) -> String {
    let w = LABEL_WIDTH;
    let mut out = String::new();
    if let Some(generation) = &t.index_generation {
        out.push_str(&format!("{:<w$}{generation}\n", "index:"));
    }
    let hints = if t.hints { "on" } else { "off" };
    out.push_str(&format!("{:<w$}{hints}\n", "hints:"));
    out.push_str(&format!("{:<w$}{}\n", "readings:", t.readings));
    out.push_str(&format!("{:<w$}{}\n", "runs:", t.runs));
    out.push('\n');
    out.push_str(&format!(
        "{:>10}  {:>10}  {:>7}  {:>7}\n",
        "chars", "median ms", "min", "max"
    ));
    for p in &t.points {
        out.push_str(&format!(
            "{:>10}  {:>10}  {:>7}  {:>7}\n",
            p.chars, p.median_ms, p.min_ms, p.max_ms
        ));
    }
    out.push('\n');
    // The recorded cell, printed with its own reference points because a bare
    // multiple invites the reader to compare it against 1. `checked_div`
    // rather than a length assumption: `points` is built by this module's own
    // loop over two lengths, but a zero-length corpus would make the short
    // point 0 characters and the reference meaningless rather than a panic.
    let quadratic = (t.points[1].chars * 100)
        .checked_div(t.points[0].chars)
        .unwrap_or(0);
    out.push_str(&format!(
        "{:<w$}{}.{:02}x   (linear 1.00x, quadratic {}.{:02}x)\n",
        "scaling:",
        t.scaling_centi / 100,
        t.scaling_centi % 100,
        quadratic / 100,
        quadratic % 100,
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_reaches_the_target_with_whole_sentences() {
        let texts = ["あいうえお", "かきくけこさしすせそ"];
        let p = paste(&texts, 12);
        // Two sentences is 15 chars, one is 5: it must stop at the first
        // whole sentence that reaches the target, not before and not on it.
        assert_eq!(p, "あいうえおかきくけこさしすせそ");
        assert!(p.chars().count() >= 12);
    }

    #[test]
    fn paste_cycles_rather_than_running_out() {
        let texts = ["あい"];
        assert_eq!(paste(&texts, 7), "あいあいあいあい");
    }

    #[test]
    fn median_is_the_upper_middle_so_an_even_run_count_is_deterministic() {
        let d = |ms| Duration::from_millis(ms);
        assert_eq!(median(vec![d(30), d(10), d(20)]), d(20));
        assert_eq!(median(vec![d(40), d(10), d(30), d(20)]), d(30));
    }

    #[test]
    fn scaling_reads_100_for_linear_and_the_length_ratio_for_quadratic() {
        let point = |chars, ms| Point {
            chars,
            median_ms: ms,
            min_ms: ms,
            max_ms: ms,
        };
        // Ten times the characters, ten times the time.
        assert_eq!(scaling_centi(&point(1_000, 50), &point(10_000, 500)), 100);
        // Ten times the characters, a hundred times the time — Phase 2c.
        assert_eq!(
            scaling_centi(&point(1_000, 50), &point(10_000, 5_000)),
            1_000
        );
    }

    #[test]
    fn scaling_is_zero_rather_than_a_panic_when_the_short_point_is_unmeasurable() {
        let point = |chars, ms| Point {
            chars,
            median_ms: ms,
            min_ms: ms,
            max_ms: ms,
        };
        assert_eq!(scaling_centi(&point(1_000, 0), &point(10_000, 500)), 0);
    }

    fn timings(scaling_centi: u128) -> Timings {
        Timings {
            index_generation: Some("gen-1".to_string()),
            hints: true,
            readings: 3,
            runs: 5,
            points: vec![
                Point {
                    chars: 1_005,
                    median_ms: 46,
                    min_ms: 44,
                    max_ms: 47,
                },
                Point {
                    chars: 10_040,
                    median_ms: 518,
                    min_ms: 497,
                    max_ms: 523,
                },
            ],
            scaling_centi,
        }
    }

    /// Exact output, for `score`'s own reason: the format is what a recorded
    /// baseline is transcribed from, so a change to it must break a test
    /// rather than quietly change what a later phase is compared against.
    #[test]
    fn render_prints_the_two_points_and_the_scaling_cell() {
        assert_eq!(
            render(&timings(112)),
            "index:            gen-1\n\
             hints:            on\n\
             readings:         3\n\
             runs:             5\n\
             \n\
             \x20    chars   median ms      min      max\n\
             \x20     1005          46       44       47\n\
             \x20    10040         518      497      523\n\
             \n\
             scaling:          1.12x   (linear 1.00x, quadratic 9.99x)\n"
        );
    }

    /// The reference points are the whole value of the cell: 1.12 means
    /// nothing until it is read against a linear 1.00 and this corpus's own
    /// quadratic 9.99.
    #[test]
    fn the_scaling_cell_carries_both_reference_points() {
        let out = render(&timings(999));
        assert!(
            out.contains("scaling:          9.99x   (linear 1.00x, quadratic 9.99x)"),
            "a quadratic reading must be indistinguishable from its own reference: {out}"
        );
    }

    #[test]
    fn an_unmeasurable_scaling_renders_as_zero_rather_than_being_omitted() {
        // `scaling_centi` returns 0 when the short point is below the clock's
        // resolution. A missing line would read as "no regression"; a 0.00x
        // reads as what it is — nothing was measurable.
        assert!(render(&timings(0)).contains("scaling:          0.00x"));
    }

    #[test]
    fn hints_off_and_a_bare_index_directory_render_without_their_lines() {
        let mut t = timings(100);
        t.hints = false;
        t.index_generation = None;
        let out = render(&t);
        assert!(out.starts_with("hints:            off\n"), "{out}");
        assert!(!out.contains("index:"), "{out}");
    }
}
