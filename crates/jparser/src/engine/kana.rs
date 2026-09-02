// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! Written from the Unicode Standard's Hiragana, Katakana and Halfwidth Forms
//! blocks.

/// Halfwidth katakana (U+FF66..=U+FF9F), mapped to their fullwidth
/// equivalents in code-point order. Not a constant offset from fullwidth:
/// the block opens with WO, then the small vowels and small TU, then follows
/// gojūon order starting from U+FF71 A. The final two entries are the
/// halfwidth voiced and semi-voiced sound marks (U+FF9E, U+FF9F), which are
/// marks rather than letters — see `fold`'s doc comment on the halfwidth
/// match arm for how those two are handled.
const HALFWIDTH_TO_FULLWIDTH: [char; 58] = [
    'ヲ', 'ァ', 'ィ', 'ゥ', 'ェ', 'ォ', 'ャ', 'ュ', 'ョ', 'ッ', // U+FF66..=U+FF6F
    'ー', // U+FF70
    'ア', 'イ', 'ウ', 'エ', 'オ', // U+FF71..=U+FF75
    'カ', 'キ', 'ク', 'ケ', 'コ', // U+FF76..=U+FF7A
    'サ', 'シ', 'ス', 'セ', 'ソ', // U+FF7B..=U+FF7F
    'タ', 'チ', 'ツ', 'テ', 'ト', // U+FF80..=U+FF84
    'ナ', 'ニ', 'ヌ', 'ネ', 'ノ', // U+FF85..=U+FF89
    'ハ', 'ヒ', 'フ', 'ヘ', 'ホ', // U+FF8A..=U+FF8E
    'マ', 'ミ', 'ム', 'メ', 'モ', // U+FF8F..=U+FF93
    'ヤ', 'ユ', 'ヨ', // U+FF94..=U+FF96
    'ラ', 'リ', 'ル', 'レ', 'ロ',       // U+FF97..=U+FF9B
    'ワ',       // U+FF9C
    'ン',       // U+FF9D
    '\u{309B}', // U+FF9E halfwidth voiced sound mark -> fullwidth spacing form
    '\u{309C}', // U+FF9F halfwidth semi-voiced sound mark -> fullwidth spacing form
];

/// Fold `c` toward its katakana index-key form: hiragana shifts `+0x60` into
/// katakana, halfwidth katakana maps to its fullwidth katakana equivalent,
/// and fullwidth digits map to ASCII. Everything else, including katakana
/// itself, kanji, and ASCII, is unchanged.
///
/// Folds toward katakana, not hiragana, because Task 3's romanizer folds
/// first and then looks the folded character up in a katakana-keyed table.
///
/// Dakuten and handakuten are preserved: `が` and `か` fold to different
/// characters, since collapsing voicing would match dakuten-dropped OCR
/// artefacts against their clean forms.
///
/// `char -> char` by construction, no lookahead: the FST walk applies this
/// per transition (spec §5), so it cannot compose e.g. halfwidth `ｶ` + `ﾞ`
/// into `ガ`. The halfwidth voiced/semi-voiced marks fold to the fullwidth
/// *spacing* marks U+309B / U+309C — deliberately not NFKC's combining marks
/// U+3099 / U+309A. `fold` never has the previous character in hand, so a
/// combining-mark result would silently attach to whatever precedes it
/// wherever the folded text is later rendered or re-normalized; the spacing
/// forms stay inert as their own two-character unit instead.
pub fn fold(c: char) -> char {
    match c {
        // Hiragana -> katakana: a constant +0x60 offset across the block.
        '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
        // Halfwidth katakana (letters and the two sound marks) -> fullwidth.
        '\u{FF66}'..='\u{FF9F}' => HALFWIDTH_TO_FULLWIDTH[(c as u32 - 0xFF66) as usize],
        // Fullwidth digits -> ASCII digits.
        '\u{FF10}'..='\u{FF19}' => char::from_u32(c as u32 - 0xFF10 + '0' as u32).unwrap_or(c),
        _ => c,
    }
}

/// Apply [`fold`] to every character of `s`.
pub fn fold_str(s: &str) -> String {
    s.chars().map(fold).collect()
}

/// CJK Unified Ideographs (U+4E00..=U+9FFF) or CJK Extension A
/// (U+3400..=U+4DBF).
pub fn is_kanji(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}')
}

/// Hiragana (U+3041..=U+3096), katakana (U+30A1..=U+30FA, plus the
/// prolonged sound mark U+30FC), or halfwidth katakana (U+FF66..=U+FF9D).
pub fn is_kana(c: char) -> bool {
    matches!(
        c,
        '\u{3041}'..='\u{3096}' | '\u{30A1}'..='\u{30FA}' | '\u{30FC}' | '\u{FF66}'..='\u{FF9D}'
    )
}

/// ASCII digit or fullwidth digit (U+FF10..=U+FF19).
pub fn is_digit(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{FF10}'..='\u{FF19}')
}

/// Kanji or kana: the script-transition rule's notion of "Japanese text".
pub fn is_japanese(c: char) -> bool {
    is_kanji(c) || is_kana(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hiragana_and_katakana_fold_together() {
        assert_eq!(fold('か'), fold('カ'));
        assert_eq!(fold('ゃ'), fold('ャ'));
        assert_eq!(fold('ん'), fold('ン'));
    }

    #[test]
    fn halfwidth_katakana_folds_with_the_others() {
        assert_eq!(fold('ｶ'), fold('か'));
    }

    #[test]
    fn voicing_is_preserved() {
        // が and か are different words. Folding them together would make
        // every dakuten-dropped OCR artefact match its clean form, which is
        // a tier D concern the corpus deliberately carries.
        assert_ne!(fold('が'), fold('か'));
    }

    #[test]
    fn kanji_and_ascii_fold_to_themselves() {
        assert_eq!(fold('東'), '東');
        assert_eq!(fold('a'), 'a');
        assert_eq!(fold('、'), '、');
    }

    #[test]
    fn fullwidth_digits_fold_to_ascii() {
        assert_eq!(fold('２'), '2');
    }

    #[test]
    fn fold_str_folds_every_character() {
        assert_eq!(fold_str("とうきょう"), fold_str("トウキョウ"));
    }

    #[test]
    fn script_predicates_agree_with_the_chart() {
        assert!(is_kanji('東') && !is_kanji('と'));
        assert!(is_kana('と') && is_kana('ト') && !is_kana('東'));
        assert!(is_digit('7') && is_digit('７') && !is_digit('七'));
        assert!(is_japanese('東') && is_japanese('と') && !is_japanese('a'));
    }

    #[test]
    fn folding_goes_toward_katakana() {
        // Pinned, not implied: Task 3 romanizes by folding first and then
        // looking the result up in a katakana-keyed table.
        assert_eq!(fold('か'), 'カ');
        assert_eq!(fold('ｶ'), 'カ');
        assert_eq!(fold('カ'), 'カ');
    }

    #[test]
    fn the_halfwidth_sound_marks_fold_to_the_spacing_forms() {
        // Deliberate divergence from NFKC, which maps these to the combining
        // marks U+3099/U+309A. `fold` is char -> char with no lookahead, so
        // it cannot compose ｶ + ﾞ into ガ either way; the spacing forms are
        // chosen because they stand on their own instead of silently
        // attaching to whatever character happens to precede them later.
        assert_eq!(fold('\u{FF9E}'), '\u{309B}');
        assert_eq!(fold('\u{FF9F}'), '\u{309C}');
    }

    #[test]
    fn every_halfwidth_katakana_letter_folds_to_a_fullwidth_katakana_character() {
        for cp in 0xFF66u32..=0xFF9D {
            let c = char::from_u32(cp).expect("valid code point");
            let folded = fold(c);
            assert!(
                ('\u{30A1}'..='\u{30FA}').contains(&folded) || folded == '\u{30FC}',
                "U+{cp:04X} ({c:?}) folded to {folded:?}, which is not katakana"
            );
        }
    }
}
