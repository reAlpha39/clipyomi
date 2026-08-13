// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Romaji conversion, ported from `ChunkToRomaji`/`ToRomaji` and `romajiTable`
//! in `ta-old/Shared/StringUtil.cpp:23-450`.
//!
//! The table is keyed on **katakana**, so input is folded to katakana per
//! character before lookup. Digraphs are listed before single characters and
//! the first match wins, which is what makes キャ produce "kya" rather than
//! "kiya" — order is load-bearing, do not sort this table.

const HIRAGANA_FOLD_START: u32 = 0x3041;
const HIRAGANA_FOLD_END: u32 = 0x3096; // exclusive
const HIRAGANA_TO_KATAKANA: u32 = 0x60;

const KATAKANA_LOW: u32 = 0x30A1;
const KATAKANA_HIGH: u32 = 0x30FC;
const MIDDLE_DOT: u32 = 0x30FB;
const LONG_VOWEL_MARK: u32 = 0x30FC;
const COMBINING_MACRON: char = '\u{0304}';
const SOKUON: char = 'ッ';

/// Vowel-row katakana ァ..オ — an 'n' before these needs an apostrophe.
const VOWEL_ROW_START: u32 = 0x30A1;
const VOWEL_ROW_END: u32 = 0x30AB; // exclusive
/// Ya-row katakana ャ..ョ — same apostrophe rule.
const YA_ROW_START: u32 = 0x30E3;
const YA_ROW_END: u32 = 0x30E9; // exclusive

/// `(katakana, romaji)`. Digraphs first; order is significant.
const ROMAJI_TABLE: &[(&str, &str)] = &[
    ("キャ", "kya"), ("キュ", "kyu"), ("キョ", "kyo"),
    ("シャ", "sha"), ("シュ", "shu"), ("ショ", "sho"),
    ("チャ", "cha"), ("チュ", "chu"), ("チョ", "cho"),
    ("ニャ", "nya"), ("ニュ", "nyu"), ("ニョ", "nyo"),
    ("ヒャ", "hya"), ("ヒュ", "hyu"), ("ヒョ", "hyo"),
    ("ミャ", "mya"), ("ミュ", "myu"), ("ミョ", "myo"),
    ("リャ", "rya"), ("リュ", "ryu"), ("リョ", "ryo"),
    ("ヰャ", "wya"), ("ヰュ", "wyu"), ("ヰョ", "wyo"),
    ("ギャ", "gya"), ("ギュ", "gyu"), ("ギョ", "gyo"),
    ("ヂャ", "ja"),  ("ヂュ", "ju"),  ("ヂョ", "jo"),
    ("ジャ", "ja"),  ("ジュ", "ju"),  ("ジョ", "jo"),
    ("ビャ", "bya"), ("ビュ", "byu"), ("ビョ", "byo"),
    ("ピャ", "pya"), ("ピュ", "pyu"), ("ピョ", "pyo"),
    ("イィ", "yi"),  ("ユィ", "yi"),  ("イェ", "ye"), ("ユェ", "ye"),
    ("ヷ", "va"), ("ヴァ", "va"), ("ヸ", "vi"), ("ヴィ", "vi"),
    ("ヴ", "vu"), ("ヹ", "ve"), ("ヴェ", "ve"), ("ヺ", "vo"), ("ヴォ", "vo"),
    ("ヴャ", "vya"), ("ヴュ", "vyu"), ("ヴョ", "vyo"),
    ("シェ", "she"), ("ジェ", "je"), ("チェ", "che"),
    ("スィ", "si"), ("スャ", "sya"), ("スュ", "syu"), ("スョ", "syo"),
    ("ズィ", "zi"), ("ズャ", "zya"), ("ズュ", "zyu"), ("ズョ", "zyo"),
    ("ティ", "ti"), ("トゥ", "tu"),
    ("テャ", "tya"), ("テュ", "tyu"), ("テョ", "tyo"),
    ("ディ", "di"), ("ドゥ", "du"),
    ("デャ", "dya"), ("デュ", "dyu"), ("デョ", "dyo"),
    ("ツァ", "tsa"), ("ツィ", "tsi"), ("ツェ", "tse"), ("ツォ", "tso"),
    ("ファ", "fa"), ("フィ", "fi"), ("ホゥ", "hu"),
    ("フェ", "fe"), ("フォ", "fo"),
    ("フャ", "fya"), ("フュ", "fyu"), ("フョ", "fyo"),
    ("リェ", "rye"),
    ("ウァ", "wa"), ("ウィ", "wi"), ("ウェ", "we"), ("ウォ", "wo"),
    ("ウャ", "wya"), ("ウュ", "wyu"), ("ウョ", "wyo"),
    ("クァ", "kwa"), ("クヮ", "kwa"), ("クィ", "kwi"),
    ("クゥ", "kwu"), ("クェ", "kwe"), ("クォ", "kwo"),
    ("グァ", "gwa"), ("グヮ", "gwa"), ("グィ", "gwi"),
    ("グゥ", "gwu"), ("グェ", "gwe"), ("グォ", "gwo"),
    ("ァ", "a"), ("ィ", "i"), ("ゥ", "u"), ("ェ", "e"), ("ォ", "o"),
    ("ャ", "ya"), ("ュ", "yu"), ("ョ", "yo"), ("ヮ", "wa"),
    ("ア", "a"), ("イ", "i"), ("ウ", "u"), ("エ", "e"), ("オ", "o"),
    ("カ", "ka"), ("キ", "ki"), ("ク", "ku"), ("ケ", "ke"), ("コ", "ko"),
    ("サ", "sa"), ("シ", "shi"), ("ス", "su"), ("セ", "se"), ("ソ", "so"),
    ("タ", "ta"), ("チ", "chi"), ("ツ", "tsu"), ("テ", "te"), ("ト", "to"),
    ("ナ", "na"), ("ニ", "ni"), ("ヌ", "nu"), ("ネ", "ne"), ("ノ", "no"),
    ("マ", "ma"), ("ミ", "mi"), ("ム", "mu"), ("メ", "me"), ("モ", "mo"),
    ("ヤ", "ya"), ("ユ", "yu"), ("ヨ", "yo"),
    ("ラ", "ra"), ("リ", "ri"), ("ル", "ru"), ("レ", "re"), ("ロ", "ro"),
    ("ワ", "wa"), ("ヰ", "wi"), ("ヱ", "we"), ("ヲ", "wo"), ("ン", "n"),
    ("ガ", "ga"), ("ギ", "gi"), ("グ", "gu"), ("ゲ", "ge"), ("ゴ", "go"),
    ("ダ", "da"), ("ヂ", "ji"), ("ヅ", "dzu"), ("デ", "de"), ("ド", "do"),
    ("ザ", "za"), ("ジ", "ji"), ("ズ", "zu"), ("ゼ", "ze"), ("ゾ", "zo"),
    ("ハ", "ha"), ("ヒ", "hi"), ("フ", "fu"), ("ヘ", "he"), ("ホ", "ho"),
    ("バ", "ba"), ("ビ", "bi"), ("ブ", "bu"), ("ベ", "be"), ("ボ", "bo"),
    ("パ", "pa"), ("ピ", "pi"), ("プ", "pu"), ("ペ", "pe"), ("ポ", "po"),
];

/// Fold one character to katakana for table lookup.
fn fold(c: char) -> char {
    let x = c as u32;
    if (HIRAGANA_FOLD_START..HIRAGANA_FOLD_END).contains(&x) {
        char::from_u32(x + HIRAGANA_TO_KATAKANA).unwrap_or(c)
    } else {
        c
    }
}

/// True when an 'n' followed by `next` would be ambiguous, requiring `'`.
fn needs_apostrophe(next: Option<char>) -> bool {
    let Some(n) = next.map(fold) else { return false };
    let x = n as u32;
    (VOWEL_ROW_START..VOWEL_ROW_END).contains(&x)
        || (YA_ROW_START..YA_ROW_END).contains(&x)
}

/// Convert one chunk. Returns `(romaji, chars_consumed)`; a count of zero means
/// "not kana", and the caller copies the character through unchanged.
fn chunk_to_romaji(chars: &[char]) -> (String, usize) {
    let Some(&first) = chars.first() else {
        return (String::new(), 0);
    };
    let c1 = fold(first);
    let x1 = c1 as u32;

    if x1 == LONG_VOWEL_MARK {
        return (COMBINING_MACRON.to_string(), 1);
    }
    if !(KATAKANA_LOW..=KATAKANA_HIGH).contains(&x1) || x1 == MIDDLE_DOT {
        return (String::new(), 0);
    }

    let c2 = chars.get(1).copied().map(fold);

    if c1 == SOKUON {
        // Double the next syllable's initial consonant. 'c' becomes 't' so っち
        // yields "tchi". Vowels and 'y' cannot be doubled.
        let Some(next) = c2 else { return (String::new(), 0) };
        for (jap, ascii) in ROMAJI_TABLE {
            let mut it = jap.chars();
            let (Some(j0), None) = (it.next(), it.next()) else { continue };
            if j0 != next {
                continue;
            }
            let letter = ascii.as_bytes()[0] as char;
            if matches!(letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
                continue;
            }
            let doubled = if letter == 'c' { 't' } else { letter };
            return (doubled.to_string(), 1);
        }
        return (String::new(), 0);
    }

    for (jap, ascii) in ROMAJI_TABLE {
        let mut it = jap.chars();
        let Some(j0) = it.next() else { continue };
        let j1 = it.next();
        if j0 != c1 {
            continue;
        }
        let consumed = match j1 {
            Some(second) if Some(second) == c2 => 2,
            Some(_) => continue,
            None => 1,
        };
        let mut out = (*ascii).to_string();
        if out.ends_with('n') && needs_apostrophe(chars.get(consumed).copied()) {
            out.push('\'');
        }
        return (out, consumed);
    }

    (String::new(), 0)
}

/// Convert a kana string to romaji. Non-kana characters pass through unchanged.
pub fn to_romaji(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let (written, consumed) = chunk_to_romaji(&chars[i..]);
        if consumed == 0 {
            out.push(chars[i]);
            i += 1;
        } else {
            out.push_str(&written);
            i += consumed;
        }
    }
    out
}

/// Apply ta-old's particle-only romaji corrections
/// (`FuriganaWindow::GetFurigana`, romaji branch): は reads "wa" not "ha", and
/// へ reads "e" not "he". Call this **only** for particle words.
pub fn apply_particle_fixup(s: &str) -> String {
    if s == "he" {
        return "e".to_string();
    }
    if let Some(stem) = s.strip_suffix("ha") {
        // The 'c' guard keeps "cha" intact.
        if !stem.ends_with('c') {
            return format!("{stem}wa");
        }
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_katakana() {
        assert_eq!(to_romaji("カタカナ"), "katakana");
    }

    #[test]
    fn converts_hiragana_by_folding_first() {
        assert_eq!(to_romaji("いわれた"), "iwareta");
    }

    #[test]
    fn prefers_digraphs_over_singles() {
        // The table lists digraphs first; キャ must not become "kiya".
        assert_eq!(to_romaji("キャ"), "kya");
        assert_eq!(to_romaji("しゃ"), "sha");
        assert_eq!(to_romaji("ちょ"), "cho");
    }

    #[test]
    fn sokuon_doubles_the_following_consonant() {
        assert_eq!(to_romaji("かった"), "katta");
        assert_eq!(to_romaji("いっぱい"), "ippai");
    }

    #[test]
    fn sokuon_before_chi_becomes_t_not_c() {
        // ta-old maps 'c' to 't', so っち is "tchi" rather than "cchi".
        assert_eq!(to_romaji("まっちゃ"), "matcha");
    }

    #[test]
    fn sokuon_before_a_vowel_is_passed_through() {
        // No consonant to double, so the chunk reports zero consumed and the
        // character is copied verbatim.
        assert_eq!(to_romaji("っあ"), "っa");
    }

    #[test]
    fn long_vowel_mark_becomes_combining_macron() {
        assert_eq!(to_romaji("ラーメン"), "ra\u{0304}men");
    }

    #[test]
    fn inserts_apostrophe_after_n_before_vowel_or_ya_row() {
        assert_eq!(to_romaji("しんあい"), "shin'ai");
        assert_eq!(to_romaji("かんゆ"), "kan'yu");
    }

    #[test]
    fn does_not_insert_apostrophe_after_n_before_a_consonant() {
        assert_eq!(to_romaji("かんじ"), "kanji");
    }

    #[test]
    fn passes_non_kana_through_unchanged() {
        assert_eq!(to_romaji("言う"), "言u");
        assert_eq!(to_romaji("ABC"), "ABC");
        assert_eq!(to_romaji("・"), "・");
    }

    #[test]
    fn particle_fixup_turns_trailing_ha_into_wa() {
        assert_eq!(apply_particle_fixup("ha"), "wa");
    }

    #[test]
    fn particle_fixup_spares_cha() {
        // The 'c' guard exists so "cha" is not corrupted into "cwa".
        assert_eq!(apply_particle_fixup("cha"), "cha");
    }

    #[test]
    fn particle_fixup_turns_bare_he_into_e() {
        assert_eq!(apply_particle_fixup("he"), "e");
    }

    #[test]
    fn particle_fixup_leaves_longer_he_words_alone() {
        // Only an exactly-two-character "he" collapses to "e".
        assert_eq!(apply_particle_fixup("heya"), "heya");
    }

    #[test]
    fn particle_fixup_leaves_unrelated_strings_alone() {
        assert_eq!(apply_particle_fixup("no"), "no");
        assert_eq!(apply_particle_fixup(""), "");
    }
}
