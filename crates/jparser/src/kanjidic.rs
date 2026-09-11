// SPDX-License-Identifier: MIT OR Apache-2.0

//! Streaming KANJIDIC2 XML reader.
//!
//! One `<character>` per kanji. Kept: `<literal>`, every `<reading>` whose
//! `r_type` is `ja_on` or `ja_kun`, and every `<meaning>` with no `m_lang`
//! attribute (English), capped at [`MAX_MEANINGS`]. Read and discarded:
//! `<nanori>`, every other `r_type` (pinyin, korean, vietnam), every other
//! language, and `<misc>`'s grade, stroke count, frequency and JLPT fields —
//! one line each to keep, the day a layer wants them (Phase 3a design §1).
//!
//! KANJIDIC2 declares no entities in its DTD (checked against the 2026-09-11
//! file: zero `<!ENTITY` lines), so the entity spike `jmdict.rs` records does
//! not recur here and `unescape()` is enough. The file does carry a DTD,
//! which `quick-xml` surfaces as a `DocType` event this reader ignores.

use std::io::BufRead;

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Error, Reader};

use crate::index::KanjiData;

/// English meanings kept per character. KANJIDIC2 gives up to about ten for
/// a common character; four is the decode-from-parts need (design §1.1).
pub const MAX_MEANINGS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum KanjidicError {
    #[error("malformed KANJIDIC2 XML: {0}")]
    Xml(#[from] quick_xml::Error),
}

pub fn parse_characters<R: BufRead>(reader: R) -> KanjidicReader<R> {
    KanjidicReader {
        reader: Reader::from_reader(reader),
        buf: Vec::new(),
        skipped: 0,
        done: false,
    }
}

pub struct KanjidicReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
    skipped: usize,
    done: bool,
}

impl<R: BufRead> KanjidicReader<R> {
    /// Characters skipped because their `<literal>` was not exactly one
    /// character. Never silently discarded — the caller surfaces this count.
    pub fn skipped_count(&self) -> usize {
        self.skipped
    }
}

/// Which leaf's text is being accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Literal,
    On,
    Kun,
    Meaning,
}

/// `r_type="ja_on"` or `"ja_kun"` selects a field; every other reading —
/// pinyin, korean, vietnam — is skipped.
fn reading_field(e: &BytesStart) -> Field {
    match e.try_get_attribute("r_type") {
        Ok(Some(attr)) => match attr.value.as_ref() {
            b"ja_on" => Field::On,
            b"ja_kun" => Field::Kun,
            _ => Field::None,
        },
        _ => Field::None,
    }
}

/// A `<meaning>` with no `m_lang` attribute is English.
fn meaning_field(e: &BytesStart) -> Field {
    match e.try_get_attribute("m_lang") {
        Ok(None) => Field::Meaning,
        _ => Field::None,
    }
}

impl<R: BufRead> Iterator for KanjidicReader<R> {
    type Item = Result<KanjiData, KanjidicError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut in_character = false;
        let mut literal = String::new();
        let mut on: Vec<String> = Vec::new();
        let mut kun: Vec<String> = Vec::new();
        let mut meanings: Vec<String> = Vec::new();
        let mut field = Field::None;
        let mut text = String::new();

        loop {
            self.buf.clear();
            let event = match self.reader.read_event_into(&mut self.buf) {
                Ok(ev) => ev,
                Err(e) => {
                    self.done = true;
                    return Some(Err(KanjidicError::Xml(e)));
                }
            };

            match event {
                Event::Eof => {
                    self.done = true;
                    if in_character {
                        // The plain event reader does not check that every
                        // opened element was closed; a document truncated
                        // mid-character must still be an error, not a
                        // silently shorter result. Same rule as `jmdict.rs`.
                        return Some(Err(KanjidicError::Xml(Error::IllFormed(
                            IllFormedError::MissingEndTag("character".to_string()),
                        ))));
                    }
                    return None;
                }
                Event::Start(e) => {
                    if !in_character {
                        if e.local_name().as_ref() == b"character" {
                            in_character = true;
                        }
                        continue;
                    }
                    field = match e.local_name().as_ref() {
                        b"literal" => Field::Literal,
                        b"reading" => reading_field(&e),
                        b"meaning" => meaning_field(&e),
                        _ => Field::None,
                    };
                    text.clear();
                }
                Event::Text(t) => {
                    if field != Field::None {
                        match t.unescape() {
                            Ok(s) => text.push_str(&s),
                            Err(e) => {
                                self.done = true;
                                return Some(Err(KanjidicError::Xml(e)));
                            }
                        }
                    }
                }
                Event::End(e) => {
                    if !in_character {
                        continue;
                    }
                    let value = text.trim().to_string();
                    match field {
                        Field::Literal => literal = value,
                        Field::On => on.push(value),
                        Field::Kun => kun.push(value),
                        Field::Meaning => {
                            if meanings.len() < MAX_MEANINGS {
                                meanings.push(value);
                            }
                        }
                        Field::None => {}
                    }
                    field = Field::None;
                    text.clear();

                    if e.local_name().as_ref() == b"character" {
                        in_character = false;
                        let mut chars = literal.chars();
                        match (chars.next(), chars.next()) {
                            (Some(ch), None) => {
                                return Some(Ok(KanjiData {
                                    literal: ch,
                                    on: std::mem::take(&mut on),
                                    kun: std::mem::take(&mut kun),
                                    meanings: std::mem::take(&mut meanings),
                                    grade: None,
                                    strokes: None,
                                    freq: None,
                                    jlpt: None,
                                }));
                            }
                            _ => {
                                self.skipped += 1;
                                literal.clear();
                                on.clear();
                                kun.clear();
                                meanings.clear();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn characters() -> Vec<KanjiData> {
        let xml = include_str!("../tests/fixtures/kanjidic_mini.xml");
        parse_characters(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture must parse")
    }

    fn find(literal: char) -> KanjiData {
        characters()
            .into_iter()
            .find(|k| k.literal == literal)
            .unwrap_or_else(|| panic!("{literal} is in the fixture"))
    }

    #[test]
    fn reads_every_character_in_document_order() {
        let literals: Vec<char> = characters().iter().map(|k| k.literal).collect();
        assert_eq!(literals, vec!['東', '京', '都', '食']);
    }

    #[test]
    fn reads_on_and_kun_readings_in_order() {
        let k = find('食');
        assert_eq!(k.on, vec!["ショク", "ジキ"]);
        assert_eq!(
            k.kun,
            vec!["く.う", "く.らう", "た.べる", "は.む", "-ぐ.い"]
        );
    }

    /// The `.` okurigana boundary and the `-` affix mark are KANJIDIC2's own
    /// notation and are kept verbatim (design §7): stripping them is a
    /// popover-text question, not a data one.
    #[test]
    fn keeps_okurigana_and_affix_marks_verbatim() {
        let k = find('食');
        assert!(k.kun.contains(&"た.べる".to_string()));
        assert!(k.kun.contains(&"-ぐ.い".to_string()));
    }

    #[test]
    fn drops_nanori_and_non_japanese_readings() {
        let k = find('東');
        assert_eq!(k.on, vec!["トウ"]);
        assert_eq!(k.kun, vec!["ひがし"]);
        let all: Vec<&String> = k.on.iter().chain(&k.kun).collect();
        assert!(!all
            .iter()
            .any(|r| *r == "あずま" || *r == "dong1" || *r == "동"));
    }

    #[test]
    fn keeps_english_meanings_only() {
        assert_eq!(find('東').meanings, vec!["east"]);
    }

    #[test]
    fn caps_meanings_at_four() {
        let k = find('京');
        assert_eq!(k.meanings.len(), MAX_MEANINGS);
        assert_eq!(
            k.meanings,
            vec!["capital", "10**16", "metropolis", "ten quadrillion"]
        );
        // Exactly four is not truncated.
        assert_eq!(find('都').meanings.len(), 4);
    }

    #[test]
    fn skips_a_character_with_a_bad_literal_and_counts_it() {
        let xml = "<kanjidic2>\
            <character><literal>ab</literal></character>\
            <character><literal></literal></character>\
            <character><literal>本</literal><reading_meaning><rmgroup>\
            <reading r_type=\"ja_on\">ホン</reading></rmgroup></reading_meaning></character>\
            </kanjidic2>";
        let mut reader = parse_characters(std::io::Cursor::new(xml));
        let all: Vec<_> = reader.by_ref().collect();
        let ok: Vec<_> = all.into_iter().filter_map(Result::ok).collect();
        assert_eq!(ok.len(), 1, "the good character must still be returned");
        assert_eq!(ok[0].literal, '本');
        assert_eq!(ok[0].on, vec!["ホン"]);
        assert_eq!(reader.skipped_count(), 2);
    }

    #[test]
    fn decodes_a_standard_escape_in_a_meaning() {
        let xml = "<kanjidic2><character><literal>本</literal>\
            <reading_meaning><rmgroup><meaning>book &amp; origin</meaning></rmgroup>\
            </reading_meaning></character></kanjidic2>";
        let k = parse_characters(std::io::Cursor::new(xml))
            .next()
            .expect("one character")
            .expect("parses");
        assert_eq!(k.meanings, vec!["book & origin"]);
    }

    #[test]
    fn reports_malformed_xml_as_an_error() {
        let bad = "<kanjidic2><character><literal>本</literal>";
        let result: Result<Vec<_>, _> = parse_characters(std::io::Cursor::new(bad)).collect();
        assert!(result.is_err());
    }

    /// `build_from_reader` supplies `std::io::empty()`; it must read as a
    /// KANJIDIC2 with no characters, not as an error.
    #[test]
    fn an_empty_reader_yields_nothing() {
        let mut reader = parse_characters(std::io::empty());
        assert!(reader.next().is_none());
        assert_eq!(reader.skipped_count(), 0);
    }
}
