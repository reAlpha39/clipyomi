// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser.

//! The escape from the boundary constraint, for tokens nothing can cover.
//!
//! `cost::is_allowed` forbids a span from starting or ending strictly inside
//! a tokenizer token, which is what lets `東京都` survive a tokenizer that
//! cuts it `東京|都` while keeping a span from straddling half a word. The
//! rule is right, and it has no escape for the case where it forbids
//! *everything*: when no candidate can cover a token, every span touching it
//! is illegal and its characters fall to unmatched.
//!
//! That is not a rare shape. IPADIC and JMdict are built from different
//! inventories, so wherever IPADIC holds a unit JMdict lacks the constraint
//! turns evidence into a veto. Measured 2026-09-15 over eighteen sentences
//! reported from real use, hints matched 426 characters of 630 against 487
//! with hints off, and the smallest case is three characters: IPADIC holds
//! `本当は` as one entry, JMdict does not hold it as a key, so `本当` and `は`
//! are both forbidden and both come back unmatched with the boundaries still
//! correct.
//!
//! [`Relaxed`] reopens exactly those positions **in a token that carries at
//! least one hiragana character**. The first implementation had no such
//! condition and failed its gate: it fired as readily on `品川`, `羽田空港`,
//! `1969` and `ニュルンベルク` -- proper nouns and numbers IPADIC holds whole
//! and JMdict does not hold at all -- whose interiors are full of
//! one-character entries waiting to match, so a reader hovering `品川` got
//! "goods" and "river". See [`opens_to_a_span`] for why hiragana is the
//! condition, and the Phase 4 design §2, §8 and §9 for the rule, the failure
//! and the measurement that narrowed it. Over the same eighteen sentences the
//! narrowed escape matches 475 characters of 630 against hints-off's 487.

use crate::engine::lex::Candidate;
use crate::engine::{kana, Boundaries};

/// A [`Boundaries`] that also permits a span endpoint strictly inside any
/// token the candidate set cannot cover.
///
/// `is_boundary` is the inner set verbatim — `cost::cost` counts crossings
/// through it and must not see the widening. `is_usable_endpoint` is the
/// widened one, and `cost::is_allowed` is its only caller in the engine;
/// `examples/span_dump.rs` also asks it directly, to report which positions
/// were reopened.
pub struct Relaxed<'a> {
    inner: &'a dyn Boundaries,
    /// Indexed by position in `0..=n`. True where a span may start or end.
    usable: Vec<bool>,
}

impl<'a> Relaxed<'a> {
    /// Reads `inner`'s tokens over `0..=chars.len()` and marks the interiors
    /// of the unmatchable ones.
    ///
    /// A token `[a, b)` is **matchable** when some candidate covers it whole
    /// between two real boundaries — `c.start <= a` and `c.start + c.len >=
    /// b`, with both of `c`'s own endpoints boundaries. A single covering
    /// candidate is the only way it can happen: a token has no interior
    /// boundaries by construction, so under the strict rule every allowed
    /// span either covers a token wholly or does not touch it, and no chain
    /// of allowed spans can tile one. Coverage is judged under the strict
    /// rule rather than the widened one, which is what keeps the definition
    /// from being circular.
    ///
    /// Being unmatchable is necessary and not sufficient: the token must also
    /// pass [`opens_to_a_span`], which is what keeps a proper noun or a number
    /// shut.
    ///
    /// One left-to-right pass: `O(n + candidates)`.
    pub fn over(chars: &[char], by_start: &[Vec<Candidate>], inner: &'a dyn Boundaries) -> Self {
        let n = chars.len();
        let mut usable: Vec<bool> = (0..=n).map(|p| inner.is_boundary(p)).collect();

        // The greatest end any strictly-allowed candidate starting at or
        // before the current position reaches. A token [a, b) is matchable
        // exactly when this is >= b at a.
        let mut reach = 0usize;
        let mut token_start = 0usize;
        for pos in 0..=n {
            if !usable[pos] {
                continue;
            }
            // Close the token that ended here, if its start is not this
            // position. `pos == 0` opens the first one and closes nothing.
            if pos > token_start && reach < pos && opens(chars, by_start, token_start, pos) {
                // Nothing covers [token_start, pos) and it is not one word:
                // reopen its interior.
                for p in usable.iter_mut().take(pos).skip(token_start + 1) {
                    *p = true;
                }
            }
            token_start = pos;
            for cand in by_start.get(pos).map(Vec::as_slice).unwrap_or(&[]) {
                let end = cand.start + cand.len;
                // `usable` is still the strict set at `end` unless a previous
                // token reopened it, and a reopened position is interior to a
                // token this candidate would have to straddle -- which the
                // strict rule forbids it to end on. Ask `inner` so the test
                // cannot drift as `usable` is written.
                if end <= n && inner.is_boundary(end) {
                    reach = reach.max(end);
                }
            }
        }

        Self { inner, usable }
    }
}

/// Whether an unmatchable token may be read as a *concatenation* rather than
/// as one word.
///
/// The test is a single hiragana character anywhere in it. Japanese writes its
/// inflection and its particles in hiragana, so hiragana inside a token is the
/// only evidence the engine has that the token glues a content word to
/// something else -- which is the whole case the escape exists for. `本当は`
/// is 本当 + a particle, `気持ち良かっ` is 気持ち + a conjugated adjective,
/// `ぴゅんさせてあげるから` is an unknown word followed by three ordinary ones.
///
/// A token with no hiragana in it is a run of kanji, katakana or digits that
/// IPADIC chose not to cut, and that is a single lexical unit by construction:
/// `品川`, `羽田空港`, `1969`, `ニュルンベルク`. Reopening one of those buys
/// nothing, because JMdict has no entry for the unit either -- it only lets
/// the one-character entries inside it win a span each.
///
/// Measured over the 150-sentence gold corpus, hints on, against the
/// unconditional version (Phase 4 design §9): of the 189 unmatchable tokens
/// that version reopened, the 47 hiragana-bearing ones were worth +29 gold
/// spans against -3. The other 142 -- 68 all-kanji, 36 all-katakana, 25
/// all-digit, 13 Latin or punctuation -- were worth +8 against -88.
///
/// **What it does not separate**, and cannot: a hiragana run that is itself one
/// unit, `ぱたぱたぱたぱた` or `ぇぇぇ`. Those are 3 of the corpus's 5
/// remaining losses, and they are the price of admitting `ぱひゅぱひゅぱひゅと`,
/// which is the same shape with a particle on the end.
fn opens_to_a_span(token: &[char]) -> bool {
    token.iter().copied().any(kana::is_hiragana)
}

/// Shortest piece a decomposition may use, for [`opens`]'s second clause.
///
/// **Swept against the gold corpus and the eighteen reported sentences, hints
/// on, 2026-09-16.** 3 is the only value that gains on both registers at once:
///
/// | min | strict | tolerant | eighteen sentences |
/// | --- | --- | --- | --- |
/// | off | 1822 | 0.8784 | 475 / 630 |
/// | 2 | 1817 | 0.8761 | 486 |
/// | **3** | **1825** | **0.8798** | **486** |
/// | 4 | 1825 | 0.8798 | 475 |
/// | 5 | 1825 | 0.8798 | 475 |
///
/// 2 is too permissive and costs 5 gold spans. 4 keeps the corpus gain and
/// loses the reported one, for a reason worth keeping: the case that pays for
/// this clause is `ロイヤル` + `ネイビー` + `ガール`, and `ガール` is exactly
/// three characters.
const MIN_PIECE: usize = 3;

/// Whether the token `[a, b)` may be reopened at all.
///
/// Two ways in. [`opens_to_a_span`] is the first: the token carries hiragana,
/// so it is a concatenation rather than one word.
///
/// The second is for a token with no hiragana in it — a run of kanji,
/// katakana or digits, which is a single lexical unit *usually*, but not when
/// it is several words the tokenizer glued. The test is whether the token can
/// be tiled end to end by candidates that are each at least [`MIN_PIECE`]
/// characters: **"it decomposes into real words", not "it is katakana"**.
/// `ロイヤルネイビーガール` tiles as 4 + 4 + 3 and opens;
/// `ニュルンベルク` only tiles as ニ+ュ+ル+ン+ベル+ク and stays shut, which is
/// the whole distinction the script test could not make.
///
/// Measured over the 150-sentence corpus this reaches **exactly one** token,
/// `クライシスマッピングパーティー` — three gold spans, one of the four real
/// compounds the hiragana rule alone had to give up — and reopens none of the
/// gazetteer that rule exists to keep shut.
///
/// `O(token length × candidates in it)`, over tokens that are a few characters
/// long, and only for tokens already known to be unmatchable.
fn opens(chars: &[char], by_start: &[Vec<Candidate>], a: usize, b: usize) -> bool {
    if opens_to_a_span(&chars[a..b]) {
        return true;
    }
    let mut reach = vec![false; b - a + 1];
    reach[0] = true;
    for p in a..b {
        if !reach[p - a] {
            continue;
        }
        for c in by_start.get(p).map(Vec::as_slice).unwrap_or(&[]) {
            let end = c.start + c.len;
            if c.len >= MIN_PIECE && end <= b {
                reach[end - a] = true;
            }
        }
    }
    reach[b - a]
}

impl Boundaries for Relaxed<'_> {
    fn is_boundary(&self, pos: usize) -> bool {
        self.inner.is_boundary(pos)
    }

    fn is_usable_endpoint(&self, pos: usize) -> bool {
        self.usable.get(pos).copied().unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WordFlags;

    /// Boundaries at exactly the listed positions.
    struct At(&'static [usize]);
    impl Boundaries for At {
        fn is_boundary(&self, pos: usize) -> bool {
            self.0.contains(&pos)
        }
    }

    fn cand(start: usize, len: usize) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: 0,
            flags: WordFlags::PRIMARY,
            steps: Vec::new(),
            exact: true,
            function_stack: false,
            digit_counter: false,
            after_digit_noncounter: false,
            headword: String::new(),
        }
    }

    fn by_start(n: usize, cands: Vec<Candidate>) -> Vec<Vec<Candidate>> {
        let mut out: Vec<Vec<Candidate>> = (0..n).map(|_| Vec::new()).collect();
        for c in cands {
            out[c.start].push(c);
        }
        out
    }

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn a_token_some_candidate_covers_is_left_alone() {
        // 東京|都 with 東京都 available: the tokenizer's own cut at 2 stays a
        // boundary, and nothing interior opens, because both tokens sit
        // inside the covering span.
        let b = At(&[0, 2, 3]);
        let cands = by_start(3, vec![cand(0, 3), cand(0, 2), cand(2, 1)]);
        let r = Relaxed::over(&chars("東京都"), &cands, &b);
        assert!(r.is_usable_endpoint(0));
        assert!(r.is_usable_endpoint(2));
        assert!(r.is_usable_endpoint(3));
        assert!(!r.is_usable_endpoint(1), "1 is interior to a covered token");
    }

    #[test]
    fn a_token_nothing_covers_opens_its_interior() {
        // 本当は as one token, with 本当 and は as the only candidates: both
        // end or start at 2, which the strict rule forbids, and nothing
        // reaches 3 from 0.
        let b = At(&[0, 3]);
        let cands = by_start(3, vec![cand(0, 2), cand(2, 1)]);
        let r = Relaxed::over(&chars("本当は"), &cands, &b);
        assert!(r.is_usable_endpoint(2), "the token is unmatchable");
        assert!(r.is_usable_endpoint(1));
        // The inner set is untouched: `cost` still counts crossings through
        // the tokenizer's own boundaries.
        assert!(!r.is_boundary(2));
        assert!(r.is_boundary(3));
    }

    #[test]
    fn only_the_unmatchable_token_opens() {
        // Two tokens, [0,3) covered by a candidate and [3,6) covered by
        // nothing.
        let b = At(&[0, 3, 6]);
        let cands = by_start(6, vec![cand(0, 3), cand(3, 2)]);
        let r = Relaxed::over(&chars("本当はですから"), &cands, &b);
        assert!(!r.is_usable_endpoint(1));
        assert!(!r.is_usable_endpoint(2));
        assert!(r.is_usable_endpoint(4), "the second token is unmatchable");
        assert!(r.is_usable_endpoint(5));
    }

    #[test]
    fn a_candidate_that_ends_off_a_boundary_does_not_count_as_cover() {
        // The only candidate spanning the token ends at 2, which is not a
        // boundary, so it is illegal under the strict rule and cannot be what
        // makes the token matchable.
        let b = At(&[0, 3]);
        let cands = by_start(3, vec![cand(0, 2)]);
        let r = Relaxed::over(&chars("本当は"), &cands, &b);
        assert!(r.is_usable_endpoint(1));
        assert!(r.is_usable_endpoint(2));
    }

    #[test]
    fn a_covering_span_longer_than_the_token_still_covers_it() {
        // 気持ち良かっ|た with a candidate reaching past the token's end: the
        // first token is covered, so nothing in it opens.
        let b = At(&[0, 3, 4]);
        let cands = by_start(4, vec![cand(0, 4)]);
        let r = Relaxed::over(&chars("気持ちた"), &cands, &b);
        assert!(!r.is_usable_endpoint(1));
        assert!(!r.is_usable_endpoint(2));
    }

    #[test]
    fn empty_input_has_one_position_and_opens_nothing() {
        let b = At(&[0]);
        let r = Relaxed::over(&[], &[], &b);
        assert!(r.is_usable_endpoint(0));
        assert!(!r.is_usable_endpoint(1), "past the end is not an endpoint");
    }

    /// The narrowing, as the property that separates the two failures.
    ///
    /// Both tokens are unmatchable in exactly the same way -- one token, no
    /// candidate long enough, one-character entries inside -- and the escape
    /// must open one and not the other. `品川` is a place; opening it bought
    /// the reader "goods" and "river", and cost the gold corpus 42 spans in
    /// its script class alone.
    #[test]
    fn a_token_with_no_hiragana_in_it_stays_shut() {
        let b = At(&[0, 2]);
        let cands = by_start(2, vec![cand(0, 1), cand(1, 1)]);

        let place = Relaxed::over(&chars("品川"), &cands, &b);
        assert!(
            !place.is_usable_endpoint(1),
            "an all-kanji token is one lexical unit"
        );

        let number = Relaxed::over(&chars("19"), &cands, &b);
        assert!(!number.is_usable_endpoint(1), "and so is a number");

        let katakana = Relaxed::over(&chars("ベル"), &cands, &b);
        assert!(!katakana.is_usable_endpoint(1), "and so is a katakana run");

        // The contrast, with everything else held identical: one hiragana
        // character is the whole difference.
        let phrase = Relaxed::over(&chars("本は"), &cands, &b);
        assert!(phrase.is_usable_endpoint(1));
    }

    /// The decomposability clause, as the pair it exists to separate.
    ///
    /// Both tokens are all-katakana, unmatchable, and identical to the
    /// hiragana test. The only difference is what the candidate set can do
    /// inside them: one tiles into three real words, the other only into
    /// fragments. Reported from the app — a reader hovering
    /// `ロイヤルネイビーガール` got nothing at all, while Translation
    /// Aggregator offered three words.
    #[test]
    fn a_katakana_token_opens_only_when_it_tiles_into_real_words() {
        let b = At(&[0, 11]);
        // ロイヤル(4) + ネイビー(4) + ガール(3), every piece at MIN_PIECE or
        // longer.
        let title = by_start(11, vec![cand(0, 4), cand(4, 4), cand(8, 3)]);
        let r = Relaxed::over(&chars("ロイヤルネイビーガール"), &title, &b);
        assert!(r.is_usable_endpoint(4), "ロイヤル | ネイビー");
        assert!(r.is_usable_endpoint(8), "ネイビー | ガール");

        // The same shape, tiled only by fragments: ニ+ュ+ル+ン+ベル+ク. No
        // piece reaches MIN_PIECE, so nothing opens and the name stays whole.
        let b2 = At(&[0, 7]);
        let name = by_start(
            7,
            vec![
                cand(0, 1),
                cand(1, 1),
                cand(2, 1),
                cand(3, 1),
                cand(4, 2),
                cand(6, 1),
            ],
        );
        let r2 = Relaxed::over(&chars("ニュルンベルク"), &name, &b2);
        for p in 1..7 {
            assert!(!r2.is_usable_endpoint(p), "position {p} must stay shut");
        }
    }

    /// A partial tiling is not a tiling: the clause asks whether the token can
    /// be covered *end to end*, not whether it contains a long word. Without
    /// that, `羽田空港` would open on `空港` alone and come back as
    /// 羽田(unmatched) + 空港.
    #[test]
    fn a_token_that_only_partly_tiles_stays_shut() {
        let b = At(&[0, 4]);
        // Only 空港 at [2,4) is long enough; [0,2) has nothing.
        let partial = by_start(4, vec![cand(2, 2)]);
        let r = Relaxed::over(&chars("羽田空港"), &partial, &b);
        for p in 1..4 {
            assert!(!r.is_usable_endpoint(p), "position {p} must stay shut");
        }
    }

    /// The hiragana need not be at the end, or be a particle.
    ///
    /// `気持ち良かっ` is IPADIC's token for a conjugated adjective, and what
    /// the escape has to reach there is the cut at 3 between 気持ち and
    /// 良かった -- interior hiragana, not a tail. Shortened to five characters
    /// here so the fixture's boundary set can stay literal.
    #[test]
    fn interior_hiragana_opens_the_token_too() {
        let b = At(&[0, 5]);
        let cands = by_start(5, vec![cand(0, 3)]);
        let r = Relaxed::over(&chars("気持ち良か"), &cands, &b);
        assert!(r.is_usable_endpoint(3));
    }
}
