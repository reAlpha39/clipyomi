// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Boundary hints derived from Vibrato tokenization.
//!
//! `segment.rs` weights `BoundaryHints` into the segmentation DP but has had no
//! implementation since Phase 1B. This module is that implementation: it
//! tokenizes with Vibrato and marks the *interior* positions of each token, so
//! the DP is discouraged from splitting inside a word the tokenizer recognized.

/// Index of the reading in an IPADIC feature string. ta-old skips a token whose
/// reading is `*` or absent — "If katakana is '*' or does not exist, not real
/// word, so don't penalize" (`ta-old/exe/util/Dictionary.cpp:1115-1121`).
const READING_FIELD: usize = 7;

use crate::BoundaryHints;

/// Positions where a word should not begin or end, derived from tokenization.
///
/// Indexed by **char** position, matching [`BoundaryHints`] and `segment.rs`.
/// Vibrato reports char ranges directly, so nothing here converts from bytes.
pub struct BoundaryFlags {
    bad_start: Vec<bool>,
    bad_end: Vec<bool>,
}

impl BoundaryHints for BoundaryFlags {
    fn bad_start(&self, pos: usize) -> bool {
        // Out of range is not an error: `segment.rs` queries positions derived
        // from match lengths, which may exceed the tokenized text.
        self.bad_start.get(pos).copied().unwrap_or(false)
    }

    fn bad_end(&self, pos: usize) -> bool {
        self.bad_end.get(pos).copied().unwrap_or(false)
    }
}

/// Derive flags from a tokenized worker. Port of `ta-old/exe/util/
/// Dictionary.cpp:1115-1126`.
///
/// For each token, the **interior** positions are marked: a word should not end
/// before the token's last char, nor start after its first. The token's own
/// boundaries stay free — the hint says "do not split inside this," not "split
/// here." A single-char token therefore marks nothing.
///
/// A token whose reading ([`READING_FIELD`]) is absent or `*` is skipped
/// entirely. That is ta-old's unknown-word guard: penalizing splits inside a
/// word the tokenizer only guessed at would be worse than staying silent.
///
/// ta-old carries a second guard — a fuzzy re-match of the token against the
/// source, commented "I don't trust mecab all that much" — which is
/// deliberately **not** ported. It existed because ta-old drove MeCab through a
/// text pipe and had to re-find each token by scanning. Vibrato returns char
/// ranges into the exact string it was handed, so misalignment cannot occur and
/// the branch would be untestable.
fn flags_from_worker(
    worker: &vibrato::tokenizer::worker::Worker,
    char_len: usize,
) -> BoundaryFlags {
    let mut bad_start = vec![false; char_len];
    let mut bad_end = vec![false; char_len];

    for i in 0..worker.num_tokens() {
        let token = worker.token(i);
        let reading = token.feature().split(',').nth(READING_FIELD);
        if !matches!(reading, Some(r) if !r.is_empty() && r != "*") {
            continue;
        }

        let range = token.range_char();
        for pos in range.start..range.end.saturating_sub(1) {
            if pos < char_len {
                bad_end[pos] = true;
            }
            if pos + 1 < char_len {
                bad_start[pos + 1] = true;
            }
        }
    }

    BoundaryFlags { bad_start, bad_end }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-entry IPADIC-shaped dictionary, built in memory.
    ///
    /// Deliberately not a fixture file: the real dictionary is 7.7 MB and the
    /// tests must not touch the network or the filesystem. These four readers
    /// are the same inputs vibrato's own `compile` binary takes.
    pub(crate) fn test_dictionary() -> vibrato::Dictionary {
        const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                           都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        vibrato::SystemDictionaryBuilder::from_readers(
            LEX.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("the built-in test dictionary must build")
    }

    #[test]
    fn the_test_dictionary_tokenizes_a_known_sentence() {
        let dict = test_dictionary();
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("東京都");
        worker.tokenize();

        assert_eq!(worker.num_tokens(), 2, "expected 東京 + 都");
        assert_eq!(worker.token(0).surface(), "東京");
        assert_eq!(worker.token(0).range_char(), 0..2);
        assert_eq!(worker.token(1).surface(), "都");
        assert_eq!(worker.token(1).range_char(), 2..3);
        // Field 7 is IPADIC's reading. The derivation's guard depends on it.
        let reading = worker.token(0).feature().split(',').nth(READING_FIELD);
        assert_eq!(reading, Some("トウキョウ"));
    }

    /// Build flags for `text` using the built-in test dictionary.
    fn flags_for(text: &str) -> BoundaryFlags {
        let tokenizer = vibrato::Tokenizer::new(test_dictionary());
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        flags_from_worker(&worker, text.chars().count())
    }

    /// 東京 spans chars 0..2, so only its interior boundary is marked: a word
    /// may still end at 1 and start at 0, but not the reverse.
    #[test]
    fn a_multi_char_token_marks_only_its_interior() {
        let f = flags_for("東京都");

        assert!(f.bad_end(0), "0 is interior to 東京");
        assert!(f.bad_start(1), "1 is interior to 東京");

        assert!(
            !f.bad_start(0),
            "a word may start at the token's first char"
        );
        assert!(!f.bad_end(1), "a word may end at the token's last char");
    }

    /// 都 is one char, so its loop body never runs. An off-by-one here would
    /// silently penalize every single-char token in the language.
    #[test]
    fn a_single_char_token_marks_nothing() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(2), "都 must not mark its own start");
        assert!(!f.bad_end(2), "都 must not mark its own end");
    }

    #[test]
    fn empty_input_yields_empty_flags() {
        let f = flags_for("");
        assert!(!f.bad_start(0));
        assert!(!f.bad_end(0));
    }

    /// `segment.rs` queries `m.start + m.len - 1`, which can exceed what the
    /// tokenizer saw. A panic here would crash the DP.
    #[test]
    fn out_of_range_positions_are_false_not_a_panic() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(999));
        assert!(!f.bad_end(999));
    }

    /// A token whose reading is `*` is a guess, and ta-old refuses to penalize
    /// splits inside a guess. Without this guard the DP would be pushed away
    /// from splitting inside anything the tokenizer failed to recognize.
    #[test]
    fn a_token_without_a_reading_marks_nothing() {
        const LEX: &str = "謎語,0,0,5000,名詞,一般,*,*,*,*,*,*,*\n";
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        let dict = vibrato::SystemDictionaryBuilder::from_readers(
            LEX.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("dictionary");
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("謎語");
        worker.tokenize();
        let f = flags_from_worker(&worker, 2);

        assert!(!f.bad_end(0), "a reading-less token must not be penalized");
        assert!(
            !f.bad_start(1),
            "a reading-less token must not be penalized"
        );
    }
}
