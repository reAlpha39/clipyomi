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
use crate::rank::sort_matches;
use crate::record::WordFlags;

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

        // 2. Every match starting here, in bucket order.
        for m in &matches[pos] {
            let cost = score_match(text, m, hints, best[pos].cost);
            let next = pos + m.len;
            // `>=`: on a tie the LAST writer wins (`Dictionary.cpp:1255`).
            // Deliberately a different comparison from the skip above; do not
            // route both through one shared helper.
            if best[next].cost >= cost {
                best[next] = Cell {
                    cost,
                    back_len: m.len,
                };
            }
        }
    }

    Segmentation {
        total_cost: best[n].cost,
        spans: backtrack(text, matches, &best, n),
    }
}

/// Base cost of using a dictionary match. ta-old `Dictionary.cpp:1179`.
const MATCH_BASE: i32 = 10;
/// `WordFlags::PARTICLE`. First leg of the three-way else-if chain.
/// ta-old `Dictionary.cpp:1187`.
const PARTICLE_BONUS: i32 = -2;
/// Non-particle match of exactly one char. Second leg of the same chain.
/// ta-old `Dictionary.cpp:1190`.
const SINGLE_CHAR_PENALTY: i32 = 1;
/// Starting a non-particle, multi-char match between two digit characters.
/// Third leg of the same chain. ta-old `Dictionary.cpp:1193`.
const MID_NUMBER_BREAK: i32 = 100;
/// `COMMON` **or** `COMMON_LINE`. Independent `if`, stacks with the chain
/// above. ta-old `Dictionary.cpp:1197`.
const COMMON_BONUS: i32 = -3;
/// `COUNTER` preceded (skipping ASCII and ideographic spaces) by a digit.
/// When the test fails the flag is cleared instead, in the backtrack.
/// ta-old `Dictionary.cpp:1204`.
const COUNTER_AFTER_NUMBER: i32 = -2;
/// Source text and dictionary spelling disagree in kana type/width/case.
/// ta-old `Dictionary.cpp:1210`.
const INEXACT_PENALTY: i32 = 10;
/// Per char, for an `IS_NAME` match that is inexact or not an isolated
/// katakana run. Mutually exclusive with `NAME_DICT_OK`. Dormant in v1:
/// nothing sets `IS_NAME`. ta-old `Dictionary.cpp:1232`.
const NAME_DICT_BAD_PER_CHAR: i32 = 500;
/// An `IS_NAME` match that *is* an isolated exact katakana run. Dormant in v1.
/// ta-old `Dictionary.cpp:1234`.
const NAME_DICT_OK: i32 = 5;
/// `BoundaryHints::bad_start(m.start)`. ta-old `Dictionary.cpp:1181`.
const MECAB_BAD_START: i32 = 10;
/// `BoundaryHints::bad_end(m.start + m.len - 1)`. ta-old `Dictionary.cpp:1183`.
const MECAB_BAD_END: i32 = 10;

/// ASCII space and ideographic space, skipped when looking behind a counter
/// for its number. ta-old `Dictionary.cpp:1201`.
const COUNTER_SKIPPED_SPACES: [char; 2] = [' ', '\u{3000}'];

/// Cost of extending the path at `m.start` with `m`, ta-old
/// `Dictionary.cpp:1179-1235`. The clause order is load-bearing.
fn score_match(text: &[char], m: &Match, hints: Option<&dyn BoundaryHints>, base: i32) -> i32 {
    let mut s = base.saturating_add(MATCH_BASE);
    if hints.is_some_and(|h| h.bad_start(m.start)) {
        s += MECAB_BAD_START;
    }
    if hints.is_some_and(|h| h.bad_end(m.start + m.len - 1)) {
        s += MECAB_BAD_END;
    }

    // One three-way else-if chain, not three independent tests, and the legs
    // are in ta-old's order: PARTICLE pre-empts len == 1, which pre-empts the
    // mid-number break.
    if m.flags.contains(WordFlags::PARTICLE) {
        s += PARTICLE_BONUS;
    } else if m.len == 1 {
        s += SINGLE_CHAR_PENALTY;
    } else if m.start > 0 && kana::is_digit(text[m.start]) && kana::is_digit(text[m.start - 1]) {
        s += MID_NUMBER_BREAK;
    }

    // `contains` is an exact-subset test, so this must be two calls: one call
    // with both bits set would require the match to carry both.
    if m.flags.contains(WordFlags::COMMON) || m.flags.contains(WordFlags::COMMON_LINE) {
        s += COMMON_BONUS;
    }
    if m.flags.contains(WordFlags::COUNTER) && counter_after_number(text, m.start) {
        s += COUNTER_AFTER_NUMBER;
    }
    if m.inexact {
        s += INEXACT_PENALTY;
    }
    if m.flags.contains(WordFlags::IS_NAME) {
        let bad = m.inexact || !isolated_katakana_run(text, m.start, m.len);
        s += if bad {
            NAME_DICT_BAD_PER_CHAR * m.len as i32
        } else {
            NAME_DICT_OK
        };
    }
    s
}

/// Skip spaces backwards from `start - 1`; true when the first non-space char
/// found is in bounds and is a digit. ta-old `Dictionary.cpp:1200-1205`.
fn counter_after_number(text: &[char], start: usize) -> bool {
    let mut i = start;
    while i > 0 {
        i -= 1;
        if !COUNTER_SKIPPED_SPACES.contains(&text[i]) {
            return kana::is_digit(text[i]);
        }
    }
    false
}

/// Every char of the span is katakana and the span is not glued to more
/// katakana on either side. ta-old `Dictionary.cpp:1214-1231`.
fn isolated_katakana_run(text: &[char], start: usize, len: usize) -> bool {
    let end = start + len;
    if !text[start..end].iter().all(|c| kana::is_katakana(*c)) {
        return false;
    }
    if start > 0 && kana::is_katakana(text[start - 1]) {
        return false;
    }
    if end < text.len() && kana::is_katakana(text[end]) {
        return false;
    }
    true
}

/// For every match carrying `COUNTER` whose `counter_after_number` test fails,
/// clear the flag, so a counter reading that was not actually preceded by a
/// number cannot be promoted by `sort_matches`. ta-old mutated the shared
/// `Match` in place (`Dictionary.cpp:1206`); `segment` must not mutate its
/// input, so the predicate is recomputed here on the span's clones. It is a
/// pure function of `(text, start)`, so the answer is the one the DP used.
fn clear_stale_counter_flags(text: &[char], group: &mut [Match]) {
    for m in group.iter_mut() {
        if m.flags.contains(WordFlags::COUNTER) && !counter_after_number(text, m.start) {
            m.flags.remove(WordFlags::COUNTER);
        }
    }
}

/// Walk the backpointers from the end, ta-old `Dictionary.cpp:1280-1305`.
fn backtrack(text: &[char], matches: &[Vec<Match>], best: &[Cell], n: usize) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    let mut pos = n;
    while pos > 0 {
        if best[pos].back_len == 0 {
            // Coalesce the whole run of skipped chars into one unmatched span.
            // ta-old emitted nothing for these (`Dictionary.cpp:1288-1290`);
            // the port emits them so `parse` can return unmatched `Segment`s.
            let end = pos;
            while pos > 0 && best[pos].back_len == 0 {
                pos -= 1;
            }
            spans.push(Span {
                start: pos,
                len: end - pos,
                matched: false,
                matches: Vec::new(),
            });
            continue;
        }
        let len = best[pos].back_len;
        let start = pos - len;
        // Collect EVERY match aligning to the chosen span, not only the DP
        // winner (`Dictionary.cpp:1280-1299`). This is what populates the
        // alternative readings.
        let mut group: Vec<Match> = matches[start]
            .iter()
            .filter(|m| m.len == len)
            .cloned()
            .collect();
        clear_stale_counter_flags(text, &mut group);
        sort_matches(&mut group);
        spans.push(Span {
            start,
            len,
            matched: true,
            matches: group,
        });
        pos = start;
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

    struct Marked {
        starts: Vec<usize>,
        ends: Vec<usize>,
    }
    impl BoundaryHints for Marked {
        fn bad_start(&self, pos: usize) -> bool {
            self.starts.contains(&pos)
        }
        fn bad_end(&self, pos: usize) -> bool {
            self.ends.contains(&pos)
        }
    }

    fn marked(starts: &[usize], ends: &[usize]) -> Marked {
        Marked {
            starts: starts.to_vec(),
            ends: ends.to_vec(),
        }
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn no_matches(text: &[char]) -> Vec<Vec<Match>> {
        vec![Vec::new(); text.len()]
    }

    fn plain(text: &[char], start: usize, len: usize, flags: WordFlags) -> Match {
        Match {
            start,
            len,
            src_len: len,
            surface: text[start..start + len].iter().collect(),
            flags,
            entry_id: 1,
            inexact: false,
            chain: Vec::new(),
        }
    }

    fn buckets(text: &[char], ms: Vec<Match>) -> Vec<Vec<Match>> {
        let mut out = vec![Vec::new(); text.len()];
        for m in ms {
            out[m.start].push(m);
        }
        out
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

    // ---- matches and the scoring constants ------------------------------

    #[test]
    fn a_plain_match_costs_only_the_base() {
        // MATCH_BASE 10, versus 200 for skipping both characters.
        let text = chars("ねこ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::default())]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 10);
        assert_eq!(shape(&seg), vec![(0, 2, true)]);
        assert_contiguous(&seg, 2);
    }

    #[test]
    fn a_single_char_non_particle_pays_the_penalty() {
        // MATCH_BASE 10 + SINGLE_CHAR_PENALTY 1 = 11.
        let text = chars("ね");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 11);
    }

    #[test]
    fn a_particle_takes_the_bonus_instead_of_the_single_char_penalty() {
        // MATCH_BASE 10 + PARTICLE_BONUS -2 = 8. Two independent `if`s would
        // give 9; the else-if chain gives 8.
        let text = chars("は");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::PARTICLE)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 8);
    }

    #[test]
    fn a_single_char_between_two_digits_pays_the_penalty_not_the_break() {
        // The chain's leg ORDER, not just its existence: `m.len == 1` pre-empts
        // the mid-number leg, so 100 (skip '1') + 10 + 1 = 111, never 210.
        // Swapping the two legs gives 210; three independent `if`s give 211.
        let text = chars("12");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 111);
    }

    #[test]
    fn common_and_common_line_each_grant_the_bonus_once() {
        // MATCH_BASE 10 + COMMON_BONUS -3 = 7, for either flag and for both.
        let text = chars("ねこ");
        let mut both = WordFlags::COMMON;
        both.insert(WordFlags::COMMON_LINE);
        for flags in [WordFlags::COMMON, WordFlags::COMMON_LINE, both] {
            let ms = buckets(&text, vec![plain(&text, 0, 2, flags)]);
            assert_eq!(segment(&text, &ms, None).total_cost, 7, "flags {flags:?}");
        }
    }

    #[test]
    fn the_common_bonus_stacks_with_the_particle_bonus() {
        // 10 - 2 - 3 = 5. COMMON_BONUS is its own `if`, not part of the chain.
        let text = chars("は");
        let mut flags = WordFlags::PARTICLE;
        flags.insert(WordFlags::COMMON);
        let ms = buckets(&text, vec![plain(&text, 0, 1, flags)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 5);
    }

    #[test]
    fn starting_a_match_between_two_digits_costs_mid_number_break() {
        // '1' skipped = 100, then MATCH_BASE 10 + MID_NUMBER_BREAK 100 = 210.
        let text = chars("12月");
        let ms = buckets(&text, vec![plain(&text, 1, 2, WordFlags::default())]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 210);
        assert_eq!(shape(&seg), vec![(0, 1, false), (1, 2, true)]);

        // Same match, non-digit predecessor: 100 + 10 = 110. The 100 delta is
        // MID_NUMBER_BREAK and nothing else.
        let text = chars("あ2月");
        let ms = buckets(&text, vec![plain(&text, 1, 2, WordFlags::default())]);
        assert_eq!(segment(&text, &ms, None).total_cost, 110);
    }

    #[test]
    fn a_counter_after_a_number_takes_its_bonus() {
        // '3' skipped = 100, then 10 + SINGLE_CHAR_PENALTY 1
        // + COUNTER_AFTER_NUMBER -2 = 109.
        let text = chars("3日");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 109);

        // No number in front: 100 + 10 + 1 = 111, a delta of exactly 2.
        let text = chars("あ日");
        let ms = buckets(&text, vec![plain(&text, 1, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 111);
    }

    #[test]
    fn the_counter_lookbehind_skips_both_kinds_of_space() {
        // 100 + 100 (two skipped chars) + 10 + 1 - 2 = 209 in both cases.
        for gap in [' ', '\u{3000}'] {
            let text: Vec<char> = vec!['3', gap, '日'];
            let ms = buckets(&text, vec![plain(&text, 2, 1, WordFlags::COUNTER)]);
            assert_eq!(segment(&text, &ms, None).total_cost, 209, "gap {gap:?}");
        }
    }

    #[test]
    fn a_counter_at_position_zero_has_no_number_behind_it() {
        // 10 + SINGLE_CHAR_PENALTY 1 = 11; the lookbehind runs off the front.
        let text = chars("日");
        let ms = buckets(&text, vec![plain(&text, 0, 1, WordFlags::COUNTER)]);
        assert_eq!(segment(&text, &ms, None).total_cost, 11);
        assert!(!counter_after_number(&text, 0));
    }

    #[test]
    fn an_inexact_match_pays_the_inexact_penalty() {
        // 10 + INEXACT_PENALTY 10 = 20.
        let text = chars("ねこ");
        let mut m = plain(&text, 0, 2, WordFlags::default());
        m.inexact = true;
        assert_eq!(
            segment(&text, &buckets(&text, vec![m]), None).total_cost,
            20
        );
    }

    #[test]
    fn boundary_hints_add_ten_at_each_end() {
        let text = chars("ねこ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::default())]);
        // bad_start(0) only: 10 + 10 = 20.
        assert_eq!(segment(&text, &ms, Some(&marked(&[0], &[]))).total_cost, 20);
        // bad_end is tested at start + len - 1 == 1, so a flag on 2 is inert.
        assert_eq!(
            segment(&text, &ms, Some(&marked(&[0], &[2]))).total_cost,
            20
        );
        // Both ends flagged: 10 + 10 + 10 = 30.
        assert_eq!(
            segment(&text, &ms, Some(&marked(&[0], &[1]))).total_cost,
            30
        );
        // None must equal an implementation answering false everywhere.
        assert_eq!(
            segment(&text, &ms, Some(&marked(&[], &[]))),
            segment(&text, &ms, None)
        );
    }

    #[test]
    fn an_isolated_katakana_name_takes_name_dict_ok() {
        // 10 + NAME_DICT_OK 5 = 15, then 'だ' skipped: 15 + 100 = 115.
        let text = chars("ネコだ");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::IS_NAME)]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 115);
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 1, false)]);
    }

    #[test]
    fn a_name_glued_to_more_katakana_is_priced_out() {
        // A bad name costs at least 510 per char against a 100/500 skip, so it
        // can never win: skipping all three characters costs 300 and the match
        // never appears. That is what the constant is for, and it is why the
        // per-char scaling is asserted through score_match, not total_cost.
        let text = chars("ネコン");
        let ms = buckets(&text, vec![plain(&text, 0, 2, WordFlags::IS_NAME)]);
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 300);
        assert_eq!(shape(&seg), vec![(0, 3, false)]);

        // 10 + 500 * 2 and 10 + 500 * 3, on a run long enough that neither
        // span reaches the end of the katakana.
        let long = chars("ネコンド");
        assert_eq!(
            score_match(&long, &plain(&long, 0, 2, WordFlags::IS_NAME), None, 0),
            1010
        );
        assert_eq!(
            score_match(&long, &plain(&long, 0, 3, WordFlags::IS_NAME), None, 0),
            1510
        );
    }

    #[test]
    fn an_inexact_name_is_bad_even_inside_an_isolated_run() {
        // 10 + INEXACT_PENALTY 10 + 500 * 2 = 1020.
        let text = chars("ネコだ");
        let mut m = plain(&text, 0, 2, WordFlags::IS_NAME);
        m.inexact = true;
        assert_eq!(score_match(&text, &m, None, 0), 1020);
    }

    #[test]
    fn isolated_katakana_run_rejects_katakana_on_either_side() {
        let text = chars("ンネコン");
        assert!(!isolated_katakana_run(&text, 1, 2)); // katakana before
        assert!(!isolated_katakana_run(&text, 0, 2)); // katakana after
        assert!(isolated_katakana_run(&text, 0, 4)); // the whole text
        assert!(!isolated_katakana_run(&chars("ネこ"), 0, 2)); // not all katakana
    }

    #[test]
    fn a_match_wins_a_tie_against_an_earlier_match() {
        // Synthetic table chosen so two matches reach position 4 at the same
        // cost. Particle matches at (0,1) and (0,2) both cost 10 - 2 = 8, so
        // best[1] == best[2] == 8; (1,3) and (2,2) then both cost 8 + 10 = 18.
        // The match relaxation uses `>=`, so the LAST writer — the one from the
        // later start — keeps position 4.
        let text = chars("あいうえ");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 0, 1, WordFlags::PARTICLE),
                plain(&text, 0, 2, WordFlags::PARTICLE),
                plain(&text, 1, 3, WordFlags::default()),
                plain(&text, 2, 2, WordFlags::default()),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 18);
        // A strict `>` here would give [(0, 1, true), (1, 3, true)] at the same
        // total cost — the exact regression a cost-only assertion misses.
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 2, true)]);
    }

    #[test]
    fn a_skip_does_not_overwrite_an_equal_cost_match() {
        // '1' skipped = 100. At pos 1 a PARTICLE match (1,1) costs
        // 100 + 10 - 2 = 108, and a COUNTER match (1,2) costs
        // 100 + 10 + MID_NUMBER_BREAK 100 + COUNTER_AFTER_NUMBER -2 = 208.
        // At pos 2 the skip offers 108 + 100 = 208 — an exact tie. The skip
        // relaxation uses a STRICT `>`, so the match keeps position 3.
        let text = chars("12あ");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 1, 1, WordFlags::PARTICLE),
                plain(&text, 1, 2, WordFlags::COUNTER),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 208);
        // With `>=` the skip would take position 2 and the shape would become
        // [(0, 1, false), (1, 1, true), (2, 1, false)] — the (1,2) match loses,
        // at the same total cost.
        assert_eq!(shape(&seg), vec![(0, 1, false), (1, 2, true)]);
    }

    #[test]
    fn a_skipped_run_between_two_matches_becomes_one_unmatched_span() {
        // The backtrack's coalesce-then-continue branch with a matched span on
        // BOTH sides, which the leading/trailing skip tests never reach.
        // 8 + 100 + 8 = 116.
        let text = chars("はあは");
        let ms = buckets(
            &text,
            vec![
                plain(&text, 0, 1, WordFlags::PARTICLE),
                plain(&text, 2, 1, WordFlags::PARTICLE),
            ],
        );
        let seg = segment(&text, &ms, None);
        assert_eq!(seg.total_cost, 116);
        assert_eq!(shape(&seg), vec![(0, 1, true), (1, 1, false), (2, 1, true)]);
        assert_contiguous(&seg, 3);
    }

    #[test]
    fn every_constant_at_once() {
        // A three-char inexact common counter starting between two digits,
        // with both boundary hints firing:
        // 0 + 10 + 10 + 10 + 100 - 3 - 2 + 10 = 135.
        let text = chars("1２三日か");
        let mut flags = WordFlags::COMMON;
        flags.insert(WordFlags::COUNTER);
        let mut m = plain(&text, 1, 3, flags);
        m.inexact = true;
        assert_eq!(score_match(&text, &m, Some(&marked(&[1], &[3])), 0), 135);
    }

    // ---- the backtrack's collection pass --------------------------------

    #[test]
    fn the_backtrack_collects_every_match_on_the_chosen_span() {
        // Two entries share (0, 2); a third match at the same start but a
        // different length must not be collected.
        let text = chars("ねこだ");
        let mut a = plain(&text, 0, 2, WordFlags::default());
        a.entry_id = 7;
        let mut b = plain(&text, 0, 2, WordFlags::default());
        b.entry_id = 9;
        let mut c = plain(&text, 0, 1, WordFlags::default());
        c.entry_id = 11;
        let seg = segment(&text, &buckets(&text, vec![a, b, c]), None);
        assert_eq!(shape(&seg), vec![(0, 2, true), (2, 1, false)]);
        let ids: Vec<u32> = seg.spans[0].matches.iter().map(|m| m.entry_id).collect();
        assert_eq!(ids, vec![7, 9]);
    }

    #[test]
    fn a_stale_counter_flag_is_cleared_on_the_emitted_match() {
        // The counter is not preceded by a number, so COUNTER is cleared and
        // the COMMON candidate outranks it.
        let text = chars("日");
        let mut counter = plain(&text, 0, 1, WordFlags::COUNTER);
        counter.entry_id = 2;
        let mut common = plain(&text, 0, 1, WordFlags::COMMON);
        common.entry_id = 3;
        let seg = segment(&text, &buckets(&text, vec![counter, common]), None);
        let got = &seg.spans[0].matches;
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].entry_id, 3, "COMMON must outrank a stale COUNTER");
        assert!(!got[1].flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn a_live_counter_flag_survives_and_outranks() {
        let text = chars("3日");
        let mut counter = plain(&text, 1, 1, WordFlags::COUNTER);
        counter.entry_id = 2;
        let mut common = plain(&text, 1, 1, WordFlags::COMMON);
        common.entry_id = 3;
        let seg = segment(&text, &buckets(&text, vec![counter, common]), None);
        let got = &seg.spans[1].matches;
        assert_eq!(got[0].entry_id, 2);
        assert!(got[0].flags.contains(WordFlags::COUNTER));
    }
}
