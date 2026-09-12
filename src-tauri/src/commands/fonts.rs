// SPDX-License-Identifier: MIT OR Apache-2.0

//! The installed font families, for the settings window's font selects.
//!
//! Enumerated once and cached: the platform walks every installed face to
//! answer, and the coverage check below reads each file, which is hundreds
//! of milliseconds nobody wants paid on every open of the settings window.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Serialize;

/// One family as the selects want it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FontFamily {
    pub name: String,
    /// Whether some face of the family has glyphs for kana and kanji. The
    /// sentence and furigana selects list only these: a Latin-only family
    /// applied to Japanese text falls back glyph by glyph to the system CJK
    /// font, so the pick looks like nothing happened.
    pub japanese: bool,
}

/// Family names as a select wants them, from `(name, japanese)` faces:
/// sorted case-insensitively, one entry per name with `japanese` true when
/// any face of that family covers it, blanks dropped.
pub fn families_by_coverage<I>(faces: I) -> Vec<FontFamily>
where
    I: IntoIterator<Item = (String, bool)>,
{
    let mut families: Vec<FontFamily> = Vec::new();
    for (name, japanese) in faces {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        match families.iter_mut().find(|f| f.name == name) {
            Some(existing) => existing.japanese |= japanese,
            None => families.push(FontFamily {
                name: name.to_owned(),
                japanese,
            }),
        }
    }
    // ponytail: the linear find above is O(n²) over faces; a HashMap if a
    // machine with thousands of families ever makes the first open noticeable.
    families.sort_by_cached_key(|f| f.name.to_lowercase());
    families
}

/// Whether the face at INDEX in DATA has glyphs for both a hiragana and a
/// kanji. Both, not either: a kana-only face renders the furigana and
/// leaves the sentence to fall back.
pub fn covers_japanese(data: &[u8], index: u32) -> bool {
    ttf_parser::Face::parse(data, index)
        .map(|face| face.glyph_index('あ').is_some() && face.glyph_index('日').is_some())
        .unwrap_or(false)
}

/// Whether any face in the font file DATA covers Japanese. A collection
/// (`.ttc`) is judged by its most capable face: the platform reports the
/// family per face but not which face in the file it is, and the collections
/// that ship with an OS group faces of one script anyway.
pub fn any_face_covers_japanese(data: &[u8]) -> bool {
    let faces = ttf_parser::fonts_in_collection(data).unwrap_or(1);
    (0..faces).any(|index| covers_japanese(data, index))
}

/// Family names as the platform font system reports them — DirectWrite,
/// CoreText or fontconfig — because that is the system the webview resolves
/// CSS `font-family` against. Names read out of the font file itself can
/// differ: `MotoyaLMNyaa` in the file's name table is `MotoyaLMNyaa W3 mono`
/// to DirectWrite, and only the latter renders.
fn enumerate() -> Vec<FontFamily> {
    let fonts = match font_enumeration::Collection::new() {
        Ok(fonts) => fonts.take(),
        // An empty list leaves the selects showing only Default; the window
        // still works, the user just cannot pick a family from it.
        Err(_) => return Vec::new(),
    };
    // One read per file, not per face: a collection holds several faces and
    // the Japanese system fonts are mostly collections.
    let mut covered: HashMap<PathBuf, bool> = HashMap::new();
    let faces = fonts.into_iter().map(|font| {
        let japanese = *covered.entry(font.path.clone()).or_insert_with(|| {
            std::fs::read(&font.path)
                .map(|data| any_face_covers_japanese(&data))
                .unwrap_or(false)
        });
        (font.family_name, japanese)
    });
    families_by_coverage(faces)
}

/// The installed font families, sorted and deduplicated, each flagged for
/// Japanese coverage.
///
/// `async` so the first call's enumeration runs off the main thread; every
/// later call answers from the cache. Two first calls racing both enumerate,
/// which costs time once and changes nothing.
#[tauri::command]
pub async fn system_fonts() -> Vec<FontFamily> {
    static FONTS: OnceLock<Vec<FontFamily>> = OnceLock::new();
    if let Some(fonts) = FONTS.get() {
        return fonts.clone();
    }
    let fonts = tauri::async_runtime::spawn_blocking(enumerate)
        .await
        .unwrap_or_default();
    FONTS.get_or_init(|| fonts).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn faces(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter().map(|(n, j)| ((*n).to_owned(), *j)).collect()
    }

    fn family(name: &str, japanese: bool) -> FontFamily {
        FontFamily {
            name: name.to_owned(),
            japanese,
        }
    }

    #[test]
    fn families_are_sorted_case_insensitively_and_deduplicated() {
        let got = families_by_coverage(faces(&[
            ("Yu Gothic", true),
            ("arial", false),
            ("Yu Gothic", true),
            ("Meiryo", true),
        ]));
        assert_eq!(
            got,
            vec![
                family("arial", false),
                family("Meiryo", true),
                family("Yu Gothic", true),
            ]
        );
    }

    /// Collections mix coverage: one face of a family may carry the CJK
    /// glyphs and a sibling not. The family counts if any face does.
    #[test]
    fn a_family_is_japanese_when_any_face_covers_it() {
        let got = families_by_coverage(faces(&[("Yu Gothic", false), ("Yu Gothic", true)]));
        assert_eq!(got, vec![family("Yu Gothic", true)]);
    }

    #[test]
    fn blank_names_are_dropped() {
        let got = families_by_coverage(faces(&[("", true), ("  ", false), (" Meiryo ", true)]));
        assert_eq!(got, vec![family("Meiryo", true)]);
    }

    #[test]
    fn a_file_that_is_not_a_font_covers_nothing() {
        assert!(!covers_japanese(b"not a font", 0));
    }
}
#[cfg(test)]
mod file_tests {
    use super::*;

    #[test]
    fn a_file_that_is_not_a_font_has_no_covering_face() {
        assert!(!any_face_covers_japanese(b"not a font"));
    }
}
