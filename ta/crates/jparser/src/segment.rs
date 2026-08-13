// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Segmentation: a min-cost dynamic program over character positions, ported
//! from ta-old's `FindBestMatches` (`ta-old/exe/util/Dictionary.cpp:1075-1306`).
//!
//! **Low cost wins.** ta-old's own comment at `Dictionary.cpp:1143`: *"High
//! score is bad, low is good."*
//!
//! `segment` is a pure function of `(text, matches, hints)`: no index, no
//! conjugation table, no I/O, and therefore infallible. That is what lets the
//! cost assertions in this file's test module run against a hand-built match
//! table with no dictionary at all.

use crate::kana;
use crate::matcher::Match;

/// Cost of leaving one character unmatched. ta-old `Dictionary.cpp:1164`.
const SKIP_CHAR: i32 = 100;
/// Extra cost of skipping a CJK ideograph (`kana::is_cjk_ideograph`,
/// 0x4E00..=0x9FBF). Stacks on `SKIP_CHAR`; never applies to a match.
/// ta-old `Dictionary.cpp:1166`.
const SKIP_KANJI_EXTRA: i32 = 400;

/// Boundary votes from a morphological analyzer. Phase 5 supplies the Vibrato
/// implementation; Phase 1B ships only this trait and test stubs.
///
/// `pos` is a **char** offset, matching `Segment::start` — never a byte
/// offset. `None` hints must behave exactly like an implementation that
/// returns `false` everywhere.
pub trait BoundaryHints {
    /// True when a word should not begin at `pos`.
    fn bad_start(&self, pos: usize) -> bool;
    /// True when a word should not end at `pos`.
    fn bad_end(&self, pos: usize) -> bool;
}

/// The chosen cover of the input plus the DP's own total cost.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Segmentation {
    /// Contiguous cover of the input in ascending `start` order: every char
    /// position belongs to exactly one span, matched or not. Empty iff the
    /// input is empty.
    pub(crate) spans: Vec<Span>,
    /// `best[len].cost`, asserted directly by the cost tests.
    ///
    /// Read by this module's tests and by nothing else in the library:
    /// `ParseResult` deliberately does not carry a cost (a display/diagnostic
    /// concern above this crate), so port design §10's "assert the cost, not
    /// just the winning segmentation" is satisfied here and only here. The
    /// attribute is the narrowest possible — one field, not the module.
    #[allow(dead_code)]
    pub(crate) total_cost: i32,
}

/// One chosen span. `matched` is false for a skipped run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) start: usize,
    pub(crate) len: usize,
    pub(crate) matched: bool,
    /// Every match aligning to `(start, len)` — not only the DP winner —
    /// already run through `sort_matches`. Always empty when `!matched`.
    pub(crate) matches: Vec<Match>,
}

/// One DP cell: the cheapest cost of covering `text[..pos]`, plus how the
/// cheapest path arrived.
#[derive(Debug, Clone, Copy)]
struct Cell {
    cost: i32,
    /// Char length of the match that reached this position, or `0` when it was
    /// reached by a skip. Sound only because `matches_at` drops `len == 0`
    /// candidates.
    back_len: usize,
}

/// Segment `text`, where `matches[p]` holds every match with `start == p`.
pub(crate) fn segment(
    text: &[char],
    matches: &[Vec<Match>],
    hints: Option<&dyn BoundaryHints>,
) -> Segmentation {
    debug_assert_eq!(matches.len(), text.len(), "one match bucket per character");
    debug_assert!(
        matches.iter().enumerate().all(|(p, bucket)| bucket
            .iter()
            .all(|m| m.start == p && m.start + m.len <= text.len())),
        "every match in bucket p must have start == p and end inside the text"
    );
    // Hints price matches only; the skip transition never consults them.
    let _ = hints;

    let n = text.len();
    let mut best = vec![
        Cell {
            cost: i32::MAX,
            back_len: 0
        };
        n + 1
    ];
    best[0] = Cell {
        cost: 0,
        back_len: 0,
    };

    for pos in 0..n {
        // 1. The skip transition, computed FIRST. The tie rules depend on this
        //    order, exactly as ta-old's do.
        let mut cost = best[pos].cost.saturating_add(SKIP_CHAR);
        if kana::is_cjk_ideograph(text[pos]) {
            cost = cost.saturating_add(SKIP_KANJI_EXTRA);
        }
        // STRICT `>`: on a tie the value already written wins
        // (`Dictionary.cpp:1169`).
        if best[pos + 1].cost > cost {
            best[pos + 1] = Cell { cost, back_len: 0 };
        }
    }

    Segmentation {
        total_cost: best[n].cost,
        spans: backtrack(&best, n),
    }
}

/// Walk the backpointers from the end, ta-old `Dictionary.cpp:1280-1305`.
fn backtrack(best: &[Cell], n: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = n;
    while pos > 0 {
        // Coalesce the whole run of skipped chars into one unmatched span.
        // ta-old emitted nothing for these (`Dictionary.cpp:1288-1290`); the
        // port emits them so `parse` can return unmatched `Segment`s.
        let end = pos;
        while pos > 0 && best[pos].back_len == 0 {
            pos -= 1;
        }
        debug_assert!(pos < end, "a non-zero back_len with no match transition");
        spans.push(Span {
            start: pos,
            len: end - pos,
            matched: false,
            matches: Vec::new(),
        });
    }
    spans.reverse();
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AlwaysBad;
    impl BoundaryHints for AlwaysBad {
        fn bad_start(&self, _pos: usize) -> bool {
            true
        }
        fn bad_end(&self, _pos: usize) -> bool {
            true
        }
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn no_matches(text: &[char]) -> Vec<Vec<Match>> {
        vec![Vec::new(); text.len()]
    }

    fn shape(seg: &Segmentation) -> Vec<(usize, usize, bool)> {
        seg.spans
            .iter()
            .map(|s| (s.start, s.len, s.matched))
            .collect()
    }

    fn assert_contiguous(seg: &Segmentation, n: usize) {
        let mut at = 0;
        for s in &seg.spans {
            assert_eq!(s.start, at, "gap or overlap at {at}: {:?}", shape(seg));
            assert!(s.len >= 1, "zero-length span: {:?}", shape(seg));
            if !s.matched {
                assert!(s.matches.is_empty(), "unmatched span carries matches");
            }
            at += s.len;
        }
        assert_eq!(at, n, "spans do not cover the input: {:?}", shape(seg));
    }

    // ---- skipped runs ---------------------------------------------------

    #[test]
    fn empty_input_costs_nothing_and_produces_no_spans() {
        let text: Vec<char> = Vec::new();
        let seg = segment(&text, &[], None);
        assert_eq!(seg.total_cost, 0);
        assert!(seg.spans.is_empty());
    }

    #[test]
    fn each_skipped_character_costs_skip_char() {
        // 3 kana, nothing in the dictionary: 3 x SKIP_CHAR = 300.
        let text = chars("あいう");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 300);
        assert_eq!(shape(&seg), vec![(0, 3, false)]);
        assert_contiguous(&seg, 3);
    }

    #[test]
    fn skipping_a_cjk_ideograph_adds_the_kanji_extra() {
        // SKIP_CHAR 100 + SKIP_KANJI_EXTRA 400 = 500.
        let text = chars("言");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 500);
        assert_eq!(shape(&seg), vec![(0, 1, false)]);
    }

    #[test]
    fn the_kanji_repeat_mark_is_not_a_cjk_ideograph() {
        // is_kanji covers U+3005, is_cjk_ideograph deliberately does not, so
        // the repeat mark skips at the base rate: 100, not 500.
        let text = chars("々");
        assert_eq!(segment(&text, &no_matches(&text), None).total_cost, 100);
    }

    #[test]
    fn mixed_kanji_and_kana_skips_add_up() {
        // 言 = 100 + 400, う = 100. Total 600, coalesced into one span.
        let text = chars("言う");
        let seg = segment(&text, &no_matches(&text), None);
        assert_eq!(seg.total_cost, 600);
        assert_eq!(shape(&seg), vec![(0, 2, false)]);
    }

    #[test]
    fn hints_never_change_a_skip() {
        // MECAB_BAD_START/END apply to matches only (Dictionary.cpp:1180-1183).
        let text = chars("言う");
        let with = segment(&text, &no_matches(&text), Some(&AlwaysBad));
        let without = segment(&text, &no_matches(&text), None);
        assert_eq!(with.total_cost, 600);
        assert_eq!(with, without);
    }
}
