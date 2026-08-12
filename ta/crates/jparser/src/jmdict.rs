// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Streaming JMdict XML reader.
//!
//! Replaces ta-old's EDICT2 line parser (`CreateDict`,
//! `ta-old/exe/util/Dictionary.cpp:246`). JMdict carries structured data where
//! ta-old did string surgery on English glosses: real `<pos>` tags instead of
//! `strncmp` against a gloss prefix, and real `<ke_pri>`/`<re_pri>` markers
//! instead of searching for the substring "(P)".
//!
//! Parts of speech arrive as entity references (`<pos>&v5r;</pos>`) whose
//! expansions are defined in JMdict's internal DTD. We want the *code*, since
//! that is what matches conjugation type names.
//!
//! SPIKE RESULT (Task 5 Step 2): there is no distinct entity-reference event.
//! `quick_xml::events::Event` in the pinned 0.36.2 has no `GeneralRef`
//! variant at all (its variants are `Start`, `End`, `Empty`, `Text`, `CData`,
//! `Comment`, `Decl`, `PI`, `DocType`, `Eof`). Running the spike against
//! `<pos>&v5r;</pos>` produced this event sequence:
//! `Start(BytesStart { buf: Borrowed("pos"), .. })`,
//! `Text(BytesText { content: Borrowed("&v5r;") })`,
//! `End(BytesEnd { name: Borrowed("pos") })`.
//! The entity reference surfaces inside an ordinary `Event::Text`, as the
//! literal, un-decoded bytes `&v5r;` — quick-xml does not process the
//! internal DTD, so it neither expands nor rejects the reference; it passes
//! the source text through untouched. Calling `.unescape()` on that text
//! fails (`v5r` is not a predefined XML entity), so `pos`/`misc`/`dial`/
//! `field` text is decoded with a small local helper (`decode_text` below)
//! that recognizes a whole-element bare reference `&name;` and takes `name`
//! verbatim as the code, falling back to `.unescape()` for ordinary text.
//! No `quick-xml` version bump was needed.

use std::io::BufRead;

use quick_xml::errors::IllFormedError;
use quick_xml::escape::EscapeError;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Error, Reader};

/// Priority markers that mean "common word", per JMdict's documentation.
/// Replaces ta-old's `(P)` substring search.
///
/// UNVERIFIED against a real `JMdict_e.xml`: none exists in this repo, and
/// ta-old read EDICT2, where `(P)` was already baked in, so it cannot settle
/// this either. `spec2` is included on the strength of the documented EDICT
/// `(P)` rule (fires on `ichi1`, `news1`, `spec1`, `spec2`, and `gai1`) — it
/// is the hand-curated common-word list, so omitting it would drop `COMMON`
/// from real entries. Confirm this against the JMdict DTD's `ke_pri`/`re_pri`
/// documentation once a real `JMdict_e.xml` is available.
const PRIORITY_MARKERS: &[&str] = &["news1", "ichi1", "spec1", "spec2", "gai1"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KanjiForm {
    pub text: String,
    pub has_priority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingForm {
    pub text: String,
    pub has_priority: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawSense {
    /// EDICT POS codes, e.g. `v5r`, `prt`, `adj-i`.
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
    pub xrefs: Vec<String>,
    pub misc: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEntry {
    pub id: u32,
    pub kanji: Vec<KanjiForm>,
    pub readings: Vec<ReadingForm>,
    pub senses: Vec<RawSense>,
}

#[derive(Debug, thiserror::Error)]
pub enum JmdictError {
    #[error("malformed JMdict XML: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("entry {id:?} is malformed: {reason}")]
    BadEntry { id: String, reason: String },
}

/// Byte length of each of the `&` and `;` delimiters around an entity name,
/// used to locate the bytes surrounding an unrecognized reference.
const ENTITY_DELIMITER_LEN: usize = 1;

/// True when every byte in `bytes` is ASCII whitespace (vacuously true for
/// an empty slice).
fn is_all_ascii_whitespace(bytes: &[u8]) -> bool {
    bytes.iter().all(u8::is_ascii_whitespace)
}

/// True when `range` (an unrecognized entity's name span, excluding its
/// `&`/`;` delimiters) is the only non-whitespace content of `t`'s text —
/// i.e. the text is nothing but optional whitespace, the entity reference,
/// and optional whitespace. The `End` handler that consumes this value
/// already does `text.trim()`, so `decode_text` must not be stricter about
/// padding than the code that reads its result.
fn is_padded_bare_entity(t: &BytesText, range: &std::ops::Range<usize>) -> bool {
    let bytes: &[u8] = t;
    let before = &bytes[..range.start - ENTITY_DELIMITER_LEN];
    let after = &bytes[range.end + ENTITY_DELIMITER_LEN..];
    is_all_ascii_whitespace(before) && is_all_ascii_whitespace(after)
}

/// Decodes the text of a JMdict element. `unescape()` already resolves every
/// standard XML escape (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, numeric
/// refs) correctly, and fails with `EscapeError::UnrecognizedEntity` only for
/// a name it does not know how to expand. quick-xml reports that error the
/// moment it hits the bad reference, discarding whatever text (if any)
/// preceded it and never looking at what follows — so the error alone cannot
/// tell us whether the reference was the *entire* text or just part of it.
///
/// We accept the bare name as JMdict's DTD-only code (`v5r`, `uk`, ...) when
/// nothing but whitespace surrounds it (`is_padded_bare_entity`).
/// `UnrecognizedEntity`'s range is the name's byte span within the decoded
/// text, excluding both delimiters (see `quick-xml-0.36.2/src/escape.rs:279`).
/// An unrecognized entity anywhere else — e.g. embedded mid-string among
/// non-whitespace text — is genuinely malformed input and must still
/// propagate as a loud error rather than silently discarding the
/// surrounding text.
fn decode_text(t: &BytesText) -> Result<String, JmdictError> {
    match t.unescape() {
        Ok(s) => Ok(s.into_owned()),
        // `range`'s exact byte semantics (the name span, excluding the `&`/
        // `;` delimiters) are an unversioned, undocumented implementation
        // detail read from quick-xml 0.36.2's source
        // (`quick-xml-0.36.2/src/escape.rs:279`), not a contract in its
        // public docs. `ta/Cargo.lock` pins the exact version this was
        // verified against; a patch bump that shifts this would show up as
        // a failure in the escape/entity tests below (in particular the
        // `decodes_a_*_padded_bare_entity_to_its_code` and
        // `propagates_an_error_for_an_unknown_entity_embedded_mid_string`
        // cases), which are the canary if it ever changes.
        Err(Error::EscapeError(EscapeError::UnrecognizedEntity(range, name)))
            if is_padded_bare_entity(t, &range) =>
        {
            Ok(name)
        }
        Err(e) => Err(JmdictError::Xml(e)),
    }
}

pub fn parse_entries<R: BufRead>(reader: R) -> JmdictReader<R> {
    JmdictReader {
        reader: Reader::from_reader(reader),
        buf: Vec::new(),
        skipped: 0,
        done: false,
    }
}

pub struct JmdictReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
    skipped: usize,
    done: bool,
}

impl<R: BufRead> JmdictReader<R> {
    /// Entries skipped because they were malformed. Never silently discarded —
    /// the caller surfaces this count.
    pub fn skipped_count(&self) -> usize {
        self.skipped
    }
}

/// Leaf elements whose text we accumulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Other,
}

impl<R: BufRead> Iterator for JmdictReader<R> {
    type Item = Result<RawEntry, JmdictError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut in_entry = false;
        let mut id_text = String::new();
        let mut kanji: Vec<KanjiForm> = Vec::new();
        let mut readings: Vec<ReadingForm> = Vec::new();
        let mut senses: Vec<RawSense> = Vec::new();
        let mut field = Field::None;
        let mut text = String::new();

        loop {
            self.buf.clear();
            let event = match self.reader.read_event_into(&mut self.buf) {
                Ok(ev) => ev,
                Err(e) => {
                    self.done = true;
                    return Some(Err(JmdictError::Xml(e)));
                }
            };

            match event {
                Event::Eof => {
                    self.done = true;
                    if in_entry {
                        // quick-xml's plain event reader does not itself
                        // check that every opened element was closed; it
                        // just runs out of bytes. A document truncated
                        // mid-entry must still surface as an error, not as
                        // a silently shorter result.
                        return Some(Err(JmdictError::Xml(Error::IllFormed(
                            IllFormedError::MissingEndTag("entry".to_string()),
                        ))));
                    }
                    return None;
                }
                Event::Start(e) => match e.local_name().as_ref() {
                    b"entry" => {
                        in_entry = true;
                        id_text.clear();
                        kanji.clear();
                        readings.clear();
                        senses.clear();
                    }
                    b"k_ele" => kanji.push(KanjiForm {
                        text: String::new(),
                        has_priority: false,
                    }),
                    b"r_ele" => readings.push(ReadingForm {
                        text: String::new(),
                        has_priority: false,
                    }),
                    b"sense" => senses.push(RawSense::default()),
                    _ => {
                        field = Field::Other;
                        text.clear();
                    }
                },
                Event::Text(t) => {
                    if in_entry && field != Field::None {
                        match decode_text(&t) {
                            Ok(s) => text.push_str(&s),
                            Err(e) => {
                                self.done = true;
                                return Some(Err(e));
                            }
                        }
                    }
                }
                Event::End(e) => {
                    let name = e.local_name();
                    let value = text.trim().to_string();
                    match name.as_ref() {
                        b"entry" => {
                            let Ok(id) = id_text.trim().parse::<u32>() else {
                                self.skipped += 1;
                                in_entry = false;
                                field = Field::None;
                                text.clear();
                                continue;
                            };
                            return Some(Ok(RawEntry {
                                id,
                                kanji: std::mem::take(&mut kanji),
                                readings: std::mem::take(&mut readings),
                                senses: std::mem::take(&mut senses),
                            }));
                        }
                        b"ent_seq" => id_text = value,
                        b"keb" => {
                            if let Some(k) = kanji.last_mut() {
                                k.text = value;
                            }
                        }
                        b"ke_pri" => {
                            if let Some(k) = kanji.last_mut() {
                                if PRIORITY_MARKERS.contains(&value.as_str()) {
                                    k.has_priority = true;
                                }
                            }
                        }
                        b"reb" => {
                            if let Some(r) = readings.last_mut() {
                                r.text = value;
                            }
                        }
                        b"re_pri" => {
                            if let Some(r) = readings.last_mut() {
                                if PRIORITY_MARKERS.contains(&value.as_str()) {
                                    r.has_priority = true;
                                }
                            }
                        }
                        b"pos" | b"gloss" | b"xref" | b"misc" | b"s_inf" => {
                            if let Some(s) = senses.last_mut() {
                                match name.as_ref() {
                                    b"pos" => s.pos.push(value),
                                    b"gloss" => s.glosses.push(value),
                                    b"xref" => s.xrefs.push(value),
                                    b"misc" => s.misc.push(value),
                                    _ => s.info.push(value),
                                }
                            }
                        }
                        _ => {}
                    }
                    field = Field::None;
                    text.clear();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<RawEntry> {
        let xml = include_str!("../tests/fixtures/jmdict_mini.xml");
        parse_entries(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture must parse")
    }

    #[test]
    fn reads_every_entry() {
        assert_eq!(entries().len(), 3);
    }

    #[test]
    fn reads_entry_ids() {
        assert_eq!(entries()[0].id, 1000010);
        assert_eq!(entries()[2].id, 1000030);
    }

    #[test]
    fn reads_kanji_and_reading_forms_in_order() {
        let e = &entries()[0];
        assert_eq!(e.kanji.len(), 1);
        assert_eq!(e.kanji[0].text, "言う");
        assert_eq!(e.readings.len(), 2);
        assert_eq!(e.readings[0].text, "いう");
        assert_eq!(e.readings[1].text, "ゆう");
    }

    #[test]
    fn reads_priority_markers() {
        let e = &entries()[0];
        assert!(e.kanji[0].has_priority);
        assert!(e.readings[0].has_priority);
        assert!(!e.readings[1].has_priority);
    }

    #[test]
    fn resolves_pos_entities_to_codes_not_descriptions() {
        // The whole point: we need "v5r", not "Godan verb with `ru' ending",
        // because the code is what matches conjugation type names.
        assert_eq!(entries()[0].senses[0].pos, vec!["v5r"]);
        assert_eq!(entries()[1].senses[0].pos, vec!["prt"]);
        assert_eq!(entries()[2].senses[0].pos, vec!["adj-i"]);
    }

    #[test]
    fn reads_glosses_in_order() {
        assert_eq!(entries()[0].senses[0].glosses, vec!["to say", "to utter"]);
    }

    #[test]
    fn reads_xrefs_misc_and_info() {
        let s = &entries()[0].senses[0];
        assert_eq!(s.xrefs, vec!["言われる"]);
        assert_eq!(s.misc, vec!["uk"]);
        assert_eq!(s.info, vec!["usually written in kana"]);
    }

    #[test]
    fn handles_entries_with_no_kanji() {
        let e = &entries()[1];
        assert!(e.kanji.is_empty());
        assert_eq!(e.readings[0].text, "は");
    }

    #[test]
    fn reports_malformed_xml_as_an_error() {
        let bad = "<JMdict><entry><ent_seq>1</ent_seq>";
        let result: Result<Vec<_>, _> = parse_entries(std::io::Cursor::new(bad)).collect();
        assert!(result.is_err());
    }

    #[test]
    fn skips_an_entry_with_a_non_numeric_id_and_counts_it() {
        let bad = r#"<JMdict>
<entry><ent_seq>abc</ent_seq><r_ele><reb>あ</reb></r_ele></entry>
<entry><ent_seq>2</ent_seq><r_ele><reb>い</reb></r_ele></entry>
</JMdict>"#;
        let mut reader = parse_entries(std::io::Cursor::new(bad));
        let all: Vec<_> = reader.by_ref().collect();
        let ok = all.iter().filter(|r| r.is_ok()).count();
        assert_eq!(ok, 1, "the good entry must still be returned");
        assert_eq!(reader.skipped_count(), 1);
    }

    /// Parses a single inline entry, for regression tests that don't belong
    /// in the shared fixture (`jmdict_mini.xml` is asserted on exactly by
    /// later tasks and must not change).
    fn parse_one(xml: &str) -> RawEntry {
        parse_entries(std::io::Cursor::new(xml))
            .next()
            .expect("must produce one entry")
            .expect("entry must parse")
    }

    #[test]
    fn decodes_a_standalone_amp_escape_to_the_literal_ampersand() {
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <sense><gloss>&amp;</gloss></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].glosses, vec!["&"]);
    }

    #[test]
    fn decodes_a_standalone_lt_escape_to_the_literal_less_than_sign() {
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <sense><gloss>&lt;</gloss></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].glosses, vec!["<"]);
    }

    #[test]
    fn decodes_a_numeric_character_reference() {
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <k_ele><keb>&#x9AD8;</keb></k_ele></entry></JMdict>";
        assert_eq!(parse_one(xml).kanji[0].text, "高");
    }

    #[test]
    fn decodes_a_standard_escape_embedded_mid_string() {
        // Regression guard: this case already worked before the fix, because
        // the whole element wasn't a bare `&name;` span. It must keep working.
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <sense><gloss>AT&amp;T Corp</gloss></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].glosses, vec!["AT&T Corp"]);
    }

    #[test]
    fn resolves_a_bare_dtd_only_entity_to_its_code() {
        // The original intended case, pinned again here as an isolated,
        // fixture-independent regression guard alongside the escape tests
        // above: a name `unescape()` cannot recognize is the code itself.
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <sense><pos>&v5r;</pos></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].pos, vec!["v5r"]);
    }

    #[test]
    fn propagates_an_error_for_an_unknown_entity_embedded_mid_string() {
        // Regression: an unrecognized entity that is NOT the whole element
        // text is genuinely malformed input, not a JMdict DTD code. It must
        // be a loud error, not a silent `Ok` that drops "foo " and " bar".
        let xml = "<JMdict><entry><ent_seq>1</ent_seq>\
                    <sense><gloss>foo &someunknown; bar</gloss></sense></entry></JMdict>";
        let result: Result<Vec<_>, _> = parse_entries(std::io::Cursor::new(xml)).collect();
        assert!(result.is_err(), "expected an error, got {result:?}");
    }

    #[test]
    fn decodes_a_whitespace_padded_bare_entity_to_its_code() {
        // The `End` handler already trims field text, so decode_text must
        // not be stricter than that: padding around a bare entity is not
        // "other content" mixed in with it.
        let xml =
            "<JMdict><entry><ent_seq>1</ent_seq><sense><pos> &v5r; </pos></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].pos, vec!["v5r"]);
    }

    #[test]
    fn decodes_a_newline_padded_bare_entity_to_its_code() {
        let xml = "<JMdict><entry><ent_seq>1</ent_seq><sense><pos>\n&v5r;\n</pos></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].pos, vec!["v5r"]);
    }

    #[test]
    fn decodes_a_one_sided_padded_bare_entity_to_its_code() {
        let xml =
            "<JMdict><entry><ent_seq>1</ent_seq><sense><pos>&v5r; </pos></sense></entry></JMdict>";
        assert_eq!(parse_one(xml).senses[0].pos, vec!["v5r"]);
    }
}
