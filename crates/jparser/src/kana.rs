// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Character classification and normalization.
//!
//! `unify` is a direct port of `unify()` in `ta-old/Shared/Shrink.h:180`, which
//! backs ta-old's `wcsijcmp`/`wcsnijcmp` kana-insensitive comparators. It folds
//! three things: hiragana to katakana, fullwidth punctuation and digits to
//! ASCII, and ASCII lowercase to uppercase.
//!
//! The whole dictionary index depends on `unify` being applied identically at
//! build time and at query time. It is deliberately character-wise so that
//! folding a prefix equals the prefix of a folded string.

// Hiragana block, per ta-old's IsHiragana (Shared/StringUtil.cpp:521).
const HIRAGANA_START: u32 = 0x3040;
const HIRAGANA_END: u32 = 0x30A0; // exclusive

// Katakana block, per ta-old's IsKatakana (Shared/StringUtil.cpp:516).
const KATAKANA_START: u32 = 0x30A0;
const KATAKANA_END: u32 = 0x3100; // exclusive
const KATAKANA_MIDDLE_DOT: u32 = 0x30FB; // '・', excluded from katakana

// Halfwidth katakana, per ta-old's IsHalfWidthKatakana.
const HALFWIDTH_KATAKANA_START: u32 = 0xFF65;
const HALFWIDTH_KATAKANA_END: u32 = 0xFF9C; // exclusive

// CJK ideographs, per ta-old's IsKanji (Shared/StringUtil.cpp:531).
const CJK_START: u32 = 0x4E00;
const CJK_END: u32 = 0x9FC0; // exclusive
const KANJI_REPEAT_MARK: u32 = 0x3005; // '々'

// Ranges used by unify(). Bounds are ta-old's exactly; the fullwidth fold
// deliberately stops before U+FF20 so fullwidth Latin letters do not fold.
const UNIFY_HIRAGANA_START: u32 = 0x3041;
const UNIFY_HIRAGANA_END: u32 = 0x3096; // exclusive
const HIRAGANA_TO_KATAKANA: u32 = 0x30A1 - 0x3041; // 0x60
const UNIFY_FULLWIDTH_START: u32 = 0xFF01;
const UNIFY_FULLWIDTH_END: u32 = 0xFF20; // exclusive
const FULLWIDTH_TO_ASCII: u32 = 0xFF01 - 0x0021;

/// Kanji numerals counted as digits by ta-old's `IsDigit`
/// (`exe/util/Dictionary.cpp:1069`).
const KANJI_DIGITS: &[char] = &[
    '一', '二', '三', '四', '五', '六', '七', '八', '九', '十', '百', '千', '万',
];

/// Fold a character into the comparison space used by the dictionary index.
pub fn unify(c: char) -> char {
    let mut x = c as u32;
    if (UNIFY_HIRAGANA_START..UNIFY_HIRAGANA_END).contains(&x) {
        x += HIRAGANA_TO_KATAKANA;
    } else if (UNIFY_FULLWIDTH_START..UNIFY_FULLWIDTH_END).contains(&x) {
        x -= FULLWIDTH_TO_ASCII;
    }
    let Some(ch) = char::from_u32(x) else { return c };
    // ta-old's lowercase check excludes 'z' (a `< 0x7A` off-by-one). We fold
    // the full range: correctness here only requires that build-time and
    // query-time normalization agree, and an inconsistent fold would be a
    // worse bug than a faithful one.
    if ch.is_ascii() {
        return ch.to_ascii_uppercase();
    }
    ch
}

/// Fold every character of a string. Character-wise, so prefix-stable.
pub fn unify_str(s: &str) -> String {
    s.chars().map(unify).collect()
}

pub fn is_hiragana(c: char) -> bool {
    (HIRAGANA_START..HIRAGANA_END).contains(&(c as u32))
}

pub fn is_katakana(c: char) -> bool {
    let x = c as u32;
    (KATAKANA_START..KATAKANA_END).contains(&x) && x != KATAKANA_MIDDLE_DOT
}

pub fn is_half_width_katakana(c: char) -> bool {
    (HALFWIDTH_KATAKANA_START..HALFWIDTH_KATAKANA_END).contains(&(c as u32))
}

pub fn is_kanji(c: char) -> bool {
    let x = c as u32;
    (CJK_START..CJK_END).contains(&x) || x == KANJI_REPEAT_MARK
}

/// True for the CJK ideograph range the segmenter penalizes skipping. Excludes
/// the repeat mark, matching `FindBestMatches`'s inline range test.
pub fn is_cjk_ideograph(c: char) -> bool {
    (CJK_START..CJK_END).contains(&(c as u32))
}

pub fn is_japanese(c: char) -> bool {
    is_hiragana(c) || is_katakana(c) || is_kanji(c) || is_half_width_katakana(c)
}

pub fn has_japanese(s: &str) -> bool {
    s.chars().any(is_japanese)
}

pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || ('０'..='９').contains(&c) || KANJI_DIGITS.contains(&c)
}

/// Strip `suffix` from the end of `surface`, comparing kana-insensitively
/// under `unify`. Returns `None` when `suffix` is longer than `surface`, or
/// when the tail does not match once both are folded.
///
/// Shared by conjugation-chain resolution (trimming a linked type's
/// remove-suffix off a Next Type conjugation, `conjugation.rs`) and stem
/// generation (stripping a verb's dictionary-form ending, `stem.rs`). Both
/// callers must agree on this exact behavior, or generated stems stop
/// lining up with the conjugation chains that are supposed to match them.
pub fn strip_suffix_unified(surface: &str, suffix: &str) -> Option<String> {
    let s: Vec<char> = surface.chars().collect();
    let t: Vec<char> = suffix.chars().collect();
    if t.len() > s.len() {
        return None;
    }
    let split = s.len() - t.len();
    let matches = s[split..]
        .iter()
        .zip(t.iter())
        .all(|(a, b)| unify(*a) == unify(*b));
    matches.then(|| s[..split].iter().collect())
}

/// Convert hiragana in a reading to katakana for display, per ta-old's
/// `FuriganaWindow::GetFurigana` katakana branch. Non-hiragana passes through.
/// Returns `None` only if a converted code point is not a valid `char`.
pub fn to_katakana(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_hiragana(c) {
            out.push(char::from_u32(c as u32 + HIRAGANA_TO_KATAKANA)?);
        } else {
            out.push(c);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unify_folds_hiragana_to_katakana() {
        assert_eq!(unify('あ'), 'ア');
        assert_eq!(unify('ん'), 'ン');
        assert_eq!(unify('っ'), 'ッ');
    }

    #[test]
    fn unify_leaves_katakana_unchanged() {
        assert_eq!(unify('ア'), 'ア');
        assert_eq!(unify('ー'), 'ー');
    }

    #[test]
    fn unify_folds_fullwidth_punctuation_and_digits_to_ascii() {
        assert_eq!(unify('！'), '!');
        assert_eq!(unify('０'), '0');
        assert_eq!(unify('９'), '9');
    }

    #[test]
    fn unify_leaves_fullwidth_letters_unchanged() {
        // ta-old deliberately stops the fullwidth fold at U+FF20 so fullwidth
        // Latin letters are not folded. Ａ is U+FF21.
        assert_eq!(unify('Ａ'), 'Ａ');
    }

    #[test]
    fn unify_uppercases_ascii_lowercase() {
        assert_eq!(unify('a'), 'A');
        assert_eq!(unify('z'), 'Z');
    }

    #[test]
    fn unify_leaves_kanji_unchanged() {
        assert_eq!(unify('言'), '言');
    }

    #[test]
    fn unify_str_folds_every_character() {
        assert_eq!(unify_str("いわれた"), "イワレタ");
        assert_eq!(unify_str("言われた"), "言ワレタ");
    }

    #[test]
    fn unify_str_is_prefix_stable() {
        // The FST relies on this: folding a prefix must equal the prefix of the
        // folded string, or key lookup desynchronizes from the source text.
        let s = "言われなかった";
        for (i, _) in s.char_indices() {
            assert!(unify_str(s).starts_with(&unify_str(&s[..i])));
        }
    }

    #[test]
    fn classifies_hiragana() {
        assert!(is_hiragana('あ'));
        assert!(!is_hiragana('ア'));
        assert!(!is_hiragana('言'));
    }

    #[test]
    fn classifies_katakana_excluding_middle_dot() {
        assert!(is_katakana('ア'));
        assert!(is_katakana('ー'));
        assert!(!is_katakana('・'));
        assert!(!is_katakana('あ'));
    }

    #[test]
    fn classifies_kanji_including_repeat_mark() {
        assert!(is_kanji('言'));
        assert!(is_kanji('々'));
        assert!(!is_kanji('あ'));
    }

    #[test]
    fn cjk_ideograph_excludes_the_repeat_mark() {
        // The segmenter's kanji penalty uses the ideograph range only.
        assert!(is_cjk_ideograph('言'));
        assert!(!is_cjk_ideograph('々'));
    }

    #[test]
    fn is_japanese_covers_all_four_classes() {
        assert!(is_japanese('あ'));
        assert!(is_japanese('ア'));
        assert!(is_japanese('言'));
        assert!(is_japanese('ｱ'));
        assert!(!is_japanese('a'));
        assert!(!is_japanese('!'));
    }

    #[test]
    fn has_japanese_detects_any_japanese_character() {
        assert!(has_japanese("hello 言"));
        assert!(!has_japanese("hello world"));
        assert!(!has_japanese(""));
    }

    #[test]
    fn is_digit_covers_ascii_fullwidth_and_kanji_numerals() {
        assert!(is_digit('7'));
        assert!(is_digit('７'));
        assert!(is_digit('三'));
        assert!(is_digit('万'));
        assert!(!is_digit('あ'));
    }

    #[test]
    fn strip_suffix_unified_strips_a_kana_insensitive_tail() {
        // Hiragana surface, katakana suffix: unify folds both before comparing.
        assert_eq!(
            strip_suffix_unified("たべる", "ル").as_deref(),
            Some("たべ")
        );
    }

    #[test]
    fn strip_suffix_unified_returns_none_when_the_suffix_is_longer() {
        assert_eq!(strip_suffix_unified("る", "たべる"), None);
    }

    #[test]
    fn strip_suffix_unified_returns_none_when_the_tail_does_not_match() {
        assert_eq!(strip_suffix_unified("たべる", "く"), None);
    }

    #[test]
    fn strip_suffix_unified_allows_an_empty_result() {
        assert_eq!(strip_suffix_unified("する", "する").as_deref(), Some(""));
    }

    #[test]
    fn to_katakana_converts_hiragana() {
        assert_eq!(to_katakana("いわれた").as_deref(), Some("イワレタ"));
    }

    #[test]
    fn to_katakana_passes_non_hiragana_through() {
        assert_eq!(to_katakana("言う").as_deref(), Some("言ウ"));
    }
}
