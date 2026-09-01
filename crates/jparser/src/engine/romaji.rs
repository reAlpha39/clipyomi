// ClipYomi — Engine v2, Phase 1a.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b, when the crate stops being GPL-2.0-only.

//! Kana to Hepburn romaji, for display only.
//!
//! Written from a published Hepburn romanization chart, keyed on katakana:
//! callers fold with [`kana::fold`] first, so hiragana, halfwidth katakana
//! and katakana all land on the same table entries.

use super::kana;

/// Katakana digraphs (a consonant kana plus a small ya/yu/yo) and single
/// syllables, mapped to Hepburn romaji.
///
/// Grouped digraphs-then-singles for readability, but [`match_kana`] always
/// tries a two-character match before a one-character one regardless of
/// table order, so a misplaced row can't silently turn "キャ" into "kiya" —
/// the ordering that mattered for a flat scan is not load-bearing here.
///
/// Covers the gojūon (vowels, k/s/t/n/h/m/y/r/w), the voiced and
/// semi-voiced rows (g/z/d/b/p), every one of those rows' small-ya/yu/yo
/// digraph, the small vowels, and the historical ヰ/ヱ. `ッ`, `ー` and `ン`
/// are handled outside the table, in [`render`].
///
/// `ヴ` (the systematic voiced extension of `ウ`, parallel to カ→ガ) and its
/// small-vowel digraphs `ヴァ`/`ヴィ`/`ヴェ`/`ヴォ` are included: unlike the
/// exclusions below, `ヴ` has one unambiguous reading and is common in
/// loanword headwords (`ヴァイオリン`).
///
/// Deliberately excluded: `ヵ`/`ヶ` (contextually ambiguous — a small-kana
/// counter/place-name abbreviation for "ka"/"ke", not a fixed reading) and
/// `ヮ`, `ヷ`, `ヸ`, `ヹ`, `ヺ` (genuinely rare). They pass through unchanged,
/// like any other character `TABLE` doesn't cover.
const TABLE: &[(&str, &str)] = &[
    // -- Digraphs: a row's consonant kana + small ya/yu/yo --
    ("キャ", "kya"),
    ("キュ", "kyu"),
    ("キョ", "kyo"),
    ("ギャ", "gya"),
    ("ギュ", "gyu"),
    ("ギョ", "gyo"),
    ("シャ", "sha"),
    ("シュ", "shu"),
    ("ショ", "sho"),
    ("ジャ", "ja"),
    ("ジュ", "ju"),
    ("ジョ", "jo"),
    ("チャ", "cha"),
    ("チュ", "chu"),
    ("チョ", "cho"),
    ("ヂャ", "ja"),
    ("ヂュ", "ju"),
    ("ヂョ", "jo"),
    ("ニャ", "nya"),
    ("ニュ", "nyu"),
    ("ニョ", "nyo"),
    ("ヒャ", "hya"),
    ("ヒュ", "hyu"),
    ("ヒョ", "hyo"),
    ("ビャ", "bya"),
    ("ビュ", "byu"),
    ("ビョ", "byo"),
    ("ピャ", "pya"),
    ("ピュ", "pyu"),
    ("ピョ", "pyo"),
    ("ミャ", "mya"),
    ("ミュ", "myu"),
    ("ミョ", "myo"),
    ("リャ", "rya"),
    ("リュ", "ryu"),
    ("リョ", "ryo"),
    ("ヴァ", "va"),
    ("ヴィ", "vi"),
    ("ヴェ", "ve"),
    ("ヴォ", "vo"),
    // -- Singles: vowels, then each row a-i-u-e-o --
    ("ア", "a"),
    ("イ", "i"),
    ("ウ", "u"),
    ("エ", "e"),
    ("オ", "o"),
    ("カ", "ka"),
    ("キ", "ki"),
    ("ク", "ku"),
    ("ケ", "ke"),
    ("コ", "ko"),
    ("ガ", "ga"),
    ("ギ", "gi"),
    ("グ", "gu"),
    ("ゲ", "ge"),
    ("ゴ", "go"),
    ("サ", "sa"),
    ("シ", "shi"),
    ("ス", "su"),
    ("セ", "se"),
    ("ソ", "so"),
    ("ザ", "za"),
    ("ジ", "ji"),
    ("ズ", "zu"),
    ("ゼ", "ze"),
    ("ゾ", "zo"),
    ("タ", "ta"),
    ("チ", "chi"),
    ("ツ", "tsu"),
    ("テ", "te"),
    ("ト", "to"),
    ("ダ", "da"),
    ("ヂ", "ji"),
    ("ヅ", "zu"),
    ("デ", "de"),
    ("ド", "do"),
    ("ナ", "na"),
    ("ニ", "ni"),
    ("ヌ", "nu"),
    ("ネ", "ne"),
    ("ノ", "no"),
    ("ハ", "ha"),
    ("ヒ", "hi"),
    ("フ", "fu"),
    ("ヘ", "he"),
    ("ホ", "ho"),
    ("バ", "ba"),
    ("ビ", "bi"),
    ("ブ", "bu"),
    ("ベ", "be"),
    ("ボ", "bo"),
    ("パ", "pa"),
    ("ピ", "pi"),
    ("プ", "pu"),
    ("ペ", "pe"),
    ("ポ", "po"),
    ("マ", "ma"),
    ("ミ", "mi"),
    ("ム", "mu"),
    ("メ", "me"),
    ("モ", "mo"),
    ("ヤ", "ya"),
    ("ユ", "yu"),
    ("ヨ", "yo"),
    ("ラ", "ra"),
    ("リ", "ri"),
    ("ル", "ru"),
    ("レ", "re"),
    ("ロ", "ro"),
    ("ワ", "wa"),
    ("ヰ", "wi"),
    ("ヱ", "we"),
    ("ヲ", "wo"),
    ("ヴ", "vu"),
    // -- Small vowels, and standalone small ya/yu/yo (not part of a digraph) --
    ("ァ", "a"),
    ("ィ", "i"),
    ("ゥ", "u"),
    ("ェ", "e"),
    ("ォ", "o"),
    ("ャ", "ya"),
    ("ュ", "yu"),
    ("ョ", "yo"),
];

/// One decoded unit of the folded input: a table hit, one of the three
/// specials, or a character `TABLE` doesn't cover (passed through as-is).
#[derive(Clone, Copy)]
enum Token {
    Kana(&'static str),
    Sokuon,
    Chouon,
    Moraic,
    Other(char),
}

/// Match the longest kana unit at the start of `rest` against [`TABLE`],
/// trying a two-character digraph before a one-character syllable. Returns
/// the romaji and the byte length of `rest` it consumed.
fn match_kana(rest: &str) -> Option<(&'static str, usize)> {
    let mut chars = rest.char_indices();
    let (_, first) = chars.next()?;
    if let Some((second_start, second)) = chars.next() {
        let two_end = second_start + second.len_utf8();
        let two = &rest[..two_end];
        if let Some(val) = TABLE.iter().find_map(|&(k, v)| (k == two).then_some(v)) {
            return Some((val, two_end));
        }
    }
    let one_end = first.len_utf8();
    TABLE
        .iter()
        .find_map(|&(k, v)| (k == &rest[..one_end]).then_some(v))
        .map(|val| (val, one_end))
}

/// Split already-folded text into tokens, peeling `ッ`/`ー`/`ン` off before
/// table lookup so they never collide with a table entry.
fn tokenize(folded: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut rest = folded;
    while let Some(c) = rest.chars().next() {
        let consumed = match c {
            'ッ' => {
                tokens.push(Token::Sokuon);
                c.len_utf8()
            }
            'ー' => {
                tokens.push(Token::Chouon);
                c.len_utf8()
            }
            'ン' => {
                tokens.push(Token::Moraic);
                c.len_utf8()
            }
            _ => match match_kana(rest) {
                Some((val, len)) => {
                    tokens.push(Token::Kana(val));
                    len
                }
                None => {
                    tokens.push(Token::Other(c));
                    c.len_utf8()
                }
            },
        };
        rest = &rest[consumed..];
    }
    tokens
}

/// Does `s` begin with a vowel or `y`? `ン`'s apostrophe rule exists to stop
/// the reader resyllabifying `n` into a following mora that starts there.
fn starts_with_vowel_or_y(s: &str) -> bool {
    matches!(s.chars().next(), Some('a' | 'i' | 'u' | 'e' | 'o' | 'y'))
}

/// Render tokens to romaji. `ッ`, `ー` and `ン` aren't table rows because
/// each one's output depends on a neighboring token, not just itself:
///
/// - `ッ` doubles the *next* token's first letter. Before a vowel, before
///   `ン`, or at the end of the string there is no consonant to double —
///   real sokuon never actually precedes those — so it contributes nothing.
/// - `ー` repeats the last vowel already in `out`. A leading `ー`, or one
///   following a token that didn't end in a vowel, has nothing to repeat,
///   so it also contributes nothing.
/// - `ン` emits `n`, plus an apostrophe when the next token's romaji starts
///   with a vowel or `y` (`kan'you`, `shin'ichi`).
fn render(tokens: &[Token]) -> String {
    let mut out = String::new();
    for (i, token) in tokens.iter().enumerate() {
        match *token {
            Token::Kana(val) => out.push_str(val),
            Token::Other(c) => out.push(c),
            Token::Chouon => {
                if let Some(last) = out.chars().last() {
                    if matches!(last, 'a' | 'i' | 'u' | 'e' | 'o') {
                        out.push(last);
                    }
                }
            }
            Token::Sokuon => {
                if let Some(&Token::Kana(next)) = tokens.get(i + 1) {
                    if let Some(c) = next.chars().next() {
                        if !matches!(c, 'a' | 'i' | 'u' | 'e' | 'o') {
                            out.push(c);
                        }
                    }
                }
            }
            Token::Moraic => {
                out.push('n');
                let apostrophe = matches!(
                    tokens.get(i + 1),
                    Some(&Token::Kana(next)) if starts_with_vowel_or_y(next)
                );
                if apostrophe {
                    out.push('\'');
                }
            }
        }
    }
    out
}

/// Romanize `s` with Hepburn conventions: fold every character toward
/// katakana with [`kana::fold`], then walk the result matching digraphs
/// before single syllables. Anything not in `TABLE` — kanji, ASCII,
/// punctuation, and the handful of katakana extensions `TABLE` deliberately
/// omits — passes through unchanged. Never panics.
pub fn to_romaji(s: &str) -> String {
    let folded: String = s.chars().map(kana::fold).collect();
    render(&tokenize(&folded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_syllables() {
        assert_eq!(to_romaji("さくら"), "sakura");
        assert_eq!(to_romaji("トウキョウ"), "toukyou");
    }

    #[test]
    fn digraphs_beat_their_first_character() {
        // The reason the table is ordered: キャ is "kya", never "kiya".
        assert_eq!(to_romaji("キャ"), "kya");
        assert_eq!(to_romaji("しゅ"), "shu");
        assert_eq!(to_romaji("ちょ"), "cho");
    }

    #[test]
    fn sokuon_doubles_the_following_consonant() {
        assert_eq!(to_romaji("がっこう"), "gakkou");
        assert_eq!(to_romaji("いっしょ"), "issho");
    }

    #[test]
    fn n_before_a_vowel_or_y_takes_an_apostrophe() {
        assert_eq!(to_romaji("しんいち"), "shin'ichi");
        assert_eq!(to_romaji("かんよう"), "kan'you");
        assert_eq!(
            to_romaji("かんじ"),
            "kanji",
            "no apostrophe before a consonant"
        );
    }

    #[test]
    fn the_long_vowel_mark_repeats_the_preceding_vowel() {
        assert_eq!(to_romaji("ラーメン"), "raamen");
    }

    #[test]
    fn non_kana_passes_through() {
        assert_eq!(to_romaji("東京 tokyo!"), "東京 tokyo!");
    }

    #[test]
    fn to_romaji_never_panics_on_awkward_input() {
        // Leading ー, a doubled ッッ, kanji, ASCII, punctuation, and a
        // trailing ッ with nothing after it to double.
        let input = "ー日本語abc123!?、。ッッキャーン東京ッ";
        let _ = to_romaji(input);
    }

    #[test]
    fn vu_and_its_digraphs_romanize_instead_of_leaking_katakana() {
        // ヴァイオリン is a real JMdict headword; `ヴ` is the systematic
        // voiced extension of `ウ`, not one of the excluded ambiguous or
        // rare characters below, so it must not fall back to pass-through.
        assert_eq!(to_romaji("ヴァイオリン"), "vaiorin");
    }

    #[test]
    fn the_whole_katakana_block_is_covered_or_deliberately_excluded() {
        // Contextually ambiguous (ヵ/ヶ) or genuinely rare (the rest); see
        // `TABLE`'s doc comment for why each one is left unmapped.
        const EXCLUDED: [char; 7] = ['ヵ', 'ヶ', 'ヮ', 'ヷ', 'ヸ', 'ヹ', 'ヺ'];

        for cp in 0x30A1u32..=0x30FA {
            let c = char::from_u32(cp).expect("valid code point");
            let romaji = to_romaji(&c.to_string());
            if EXCLUDED.contains(&c) {
                assert_eq!(
                    romaji,
                    c.to_string(),
                    "U+{cp:04X} ({c:?}) should pass through"
                );
            } else if c == 'ッ' {
                // Standalone sokuon has nothing following it to double.
                assert_eq!(romaji, "", "a lone ッ should contribute nothing");
            } else {
                assert!(
                    !romaji.is_empty() && romaji.chars().all(|r| r.is_ascii_lowercase()),
                    "U+{cp:04X} ({c:?}) romanized to {romaji:?}, expected ascii"
                );
            }
        }
    }
}
