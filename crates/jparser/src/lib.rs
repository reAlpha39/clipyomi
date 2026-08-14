// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Japanese text parsing: dictionary matching, segmentation, entry assembly.
//!
//! `parse` is the whole public surface. It runs three stages:
//! `matcher::matches_at` at every character position, `segment::segment` over
//! the resulting match table, then entry assembly — resolving each surviving
//! match's `entry_id` through `Index::entry`, restoring its dictionary form,
//! rendering its conjugation label, and reconstructing its reading.
//!
//! Every offset in this API is a **character** offset. The single conversion
//! point is `text.chars().collect()` at the top of `parse`; nothing below it
//! ever sees a byte index.
//!
//! Reading reconstruction ports `JParseWindow.cpp:186-208` plus the `kuruHack`
//! block of `GetDictEntry` (`ta-old/exe/util/Dictionary.cpp:1323-1360`).
//! ta-old walked an entry's `JapString` chain for the `JAP_WORD_PRONOUNCE`
//! sibling; this port stores those readings on `EntryData` instead.

use std::collections::HashMap;

pub mod conjugation;
#[cfg(feature = "mecab")]
pub mod hints;
pub mod index;
pub mod jmdict;
pub mod kana;
mod matcher;
mod rank;
pub mod record;
pub mod romaji;
mod segment;
pub mod stem;

use crate::conjugation::{ConjugationTable, VerbTypeId};
use crate::index::load::Index;
use crate::index::EntryData;
use crate::matcher::{ConjLink, Match};
use crate::record::WordFlags;

/// One dictionary sense. A re-export rather than a parallel owned struct with
/// the same five fields.
pub use crate::index::SenseData as Sense;
pub use crate::segment::BoundaryHints;

/// Parse-time options.
///
/// Deliberately empty in Phase 1B: boundary votes arrive through `parse`'s
/// `hints` parameter, gloss filters and furigana modes are Phase 3 display
/// concerns above this crate, and the v5 mis-annotation fallback is a
/// build-time `StemOptions` flag. `#[non_exhaustive]` so adding a field later
/// is not a breaking change; construct with `ParseOptions::default()`.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ParseOptions {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    /// A contiguous cover of the input in ascending `start` order: every
    /// character belongs to exactly one segment, matched or not. Empty iff
    /// the input is empty.
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// Character offset into the parsed text. Never a byte offset.
    pub start: usize,
    /// Length in characters.
    pub len: usize,
    /// `text[start..start + len]` verbatim.
    pub surface: String,
    /// Display reading, taken from `entries[0].reading`.
    ///
    /// `None` for an unmatched run — there is no morphological-analyzer
    /// fallback — and `None` whenever the primary entry has no reading:
    /// because the match already is a reading (`WordFlags::PRONOUNCE`),
    /// because the entry has no kanji form and so stores no readings, or
    /// because no stored reading could be stripped back to a stem.
    pub reading: Option<String>,
    pub matched: bool,
    /// Every dictionary entry aligning to this exact span, ranked by
    /// `sort_matches`: the primary candidate first, then alternatives. Empty
    /// when `!matched`.
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The dictionary form: the matched surface for a plain headword, or the
    /// stem with its verb type's remove-suffix restored for a stem match.
    pub headword: String,
    pub reading: Option<String>,
    /// `render_conjugation_label` output, e.g. `"Negative Formal Past"`.
    /// `None` for a non-verb match, and `None` when the label renders empty —
    /// which `GetConjString` legitimately does for an all-Stem chain.
    pub conjugation: Option<String>,
    /// Union of every sense's `pos`, in first-seen order, deduplicated.
    pub pos: Vec<String>,
    pub senses: Vec<Sense>,
    /// The match's flags after the DP's stale-`COUNTER` clearing.
    pub flags: WordFlags,
}

/// Everything `parse` can fail at. Reading the memory-mapped index payload is
/// the only fallible step in Phase 1B; the enum exists so `parse` does not
/// leak `IndexError` into its public signature, and so variants can be added
/// without a breaking change.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("reading the index failed: {0}")]
    Index(#[from] crate::index::IndexError),
}

/// Parse `text` against an already-open index.
///
/// `index` and `table` are parameters rather than globals: the port design
/// forbids globals and the Phase 1A handoff pins "pass `&Index`". No Phase 1B
/// type stores an index directory path, which is what keeps Phase 2's
/// generation-directory layout cheap.
pub fn parse(
    index: &Index,
    table: &ConjugationTable,
    text: &str,
    opts: &ParseOptions,
    hints: Option<&dyn BoundaryHints>,
) -> Result<ParseResult, ParseError> {
    // ParseOptions carries no fields yet; the parameter exists so adding one
    // later is not a breaking change.
    let _ = opts;

    let chars: Vec<char> = text.chars().collect();
    let mut buckets: Vec<Vec<Match>> = Vec::with_capacity(chars.len());
    for i in 0..chars.len() {
        buckets.push(matcher::matches_at(index, table, &chars, i)?);
    }
    let segmentation = segment::segment(&chars, &buckets, hints);

    let mut cache: HashMap<u32, Option<EntryData>> = HashMap::new();
    let mut segments = Vec::with_capacity(segmentation.spans.len());
    for span in &segmentation.spans {
        let surface: String = chars
            .get(span.start..span.start + span.len)
            .unwrap_or_default()
            .iter()
            .collect();
        // `Span::matches` is empty whenever `!matched`, so this loop is the
        // matched/unmatched branch as well.
        let mut entries: Vec<Entry> = Vec::new();
        for m in &span.matches {
            // An entry_id with no EntryData cannot happen for an index built
            // by `build_from_reader`. Drop the match rather than invent an
            // Entry, and leave `matched` alone: the span still covers its
            // characters, and flipping it would paper over a corrupt index.
            let Some(data) = entry_data(index, &mut cache, m.entry_id)? else {
                continue;
            };
            entries.push(assemble_entry(m, data, table, &chars));
        }
        segments.push(Segment {
            start: span.start,
            len: span.len,
            surface,
            reading: entries.first().and_then(|e| e.reading.clone()),
            matched: span.matched,
            entries,
        });
    }
    Ok(ParseResult { segments })
}

/// `Index::entry` memoized for the duration of one parse. The same
/// `entry_id` recurs across alternatives within a span and across spans, and
/// every miss is a fresh bincode decode off the mmap.
fn entry_data<'a>(
    index: &Index,
    cache: &'a mut HashMap<u32, Option<EntryData>>,
    id: u32,
) -> Result<Option<&'a EntryData>, ParseError> {
    // `Entry` rather than a `contains_key` + `insert` pair (clippy::map_entry):
    // that shape does two lookups and cannot hold the `?` result across them
    // without a second borrow of `cache`.
    let slot = match cache.entry(id) {
        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
        std::collections::hash_map::Entry::Vacant(e) => e.insert(index.entry(id)?),
    };
    Ok(slot.as_ref())
}

/// One `Match` plus its `EntryData` into a public `Entry`.
fn assemble_entry(m: &Match, data: &EntryData, table: &ConjugationTable, text: &[char]) -> Entry {
    let label = matcher::render_conjugation_label(&m.chain, table);
    let mut pos: Vec<String> = Vec::new();
    for sense in &data.senses {
        for code in &sense.pos {
            if !pos.contains(code) {
                pos.push(code.clone());
            }
        }
    }
    Entry {
        headword: dictionary_form(m, table),
        reading: reconstruct_reading(m, data, table, text),
        conjugation: (!label.is_empty()).then_some(label),
        pos,
        senses: data.senses.clone(),
        flags: m.flags,
    }
}

/// The dictionary form of a match: a stem with its verb type's remove-suffix
/// restored, or the surface itself for a plain headword.
///
/// This inverts `stem::generate_stems`, which stripped the first
/// remove-tense/form-0 suffix that actually matched the headword's tail.
/// Three types declare more than one such conjugation — `copula` (だ, である),
/// `adj-i` (い, し), `v5uru` (うる, える) — so the inverse is not unique and the
/// original headword is not stored anywhere. The contract pins "first"; first
/// is what this uses, which mis-renders the rare `adj-i` word whose dictionary
/// form ends in し as one ending in い.
fn dictionary_form(m: &Match, table: &ConjugationTable) -> String {
    let Some(link) = m.chain.first() else {
        return m.surface.clone();
    };
    let Some(ty) = table.types().get(link.verb_type) else {
        return m.surface.clone();
    };
    match ty
        .conjugations
        .iter()
        .find(|c| c.tense == ty.remove_tense && c.form.0 == 0)
    {
        Some(c) => format!("{}{}", m.surface, c.suffix),
        // Structurally impossible: without one, the stem could not exist.
        None => m.surface.clone(),
    }
}

/// Rebuild the kana reading of a match, ta-old's `JParseWindow.cpp:186-208`.
///
/// `None` is a normal outcome, not an error: the match may already be kana,
/// the entry may have no kanji at all, or no twin conjugation may exist — as
/// for the Imperfective 来, whose kana `vk` twin has no Imperfective row.
fn reconstruct_reading(
    m: &Match,
    data: &EntryData,
    table: &ConjugationTable,
    text: &[char],
) -> Option<String> {
    // The match already is a reading; ta-old renders no furigana over kana.
    if m.flags.contains(WordFlags::PRONOUNCE) {
        return None;
    }
    // A kana-only entry stores no readings, because its surface is one.
    if data.readings.is_empty() {
        return None;
    }
    let Some(link) = m.chain.first() else {
        return data.readings.first().cloned();
    };
    let ty = table.types().get(link.verb_type)?;
    let tail: String = text
        .get(m.start + m.src_len..m.start + m.len)?
        .iter()
        .collect();
    // ta-old computed kuruHack once, in GetDictEntry, before the renderer
    // looped over the entry's spellings.
    let hack = kuru_hack(link, table);

    for reading in &data.readings {
        // Same-type path: the kana spelling conjugates through the same table
        // rows as the matched spelling, so its own stem plus the matched
        // suffix characters is the whole answer.
        if let Some(stem) = strip_remove_suffix(table, link.verb_type, reading) {
            return Some(stem + &tail);
        }
        // kuruHack path: the kana spelling is registered under a different,
        // identically named verb type — 来る's kanji rows are a separate `vk`
        // block from くる's kana rows. `hack` supplies the reading of the one
        // leading kanji and the rest of the matched text is copied verbatim,
        // which is what the `+ 1` skips over.
        let Some(hack) = hack.as_deref() else {
            continue;
        };
        for twin in table.types_named(&ty.name) {
            if twin == link.verb_type {
                continue;
            }
            let Some(stem) = strip_remove_suffix(table, twin, reading) else {
                continue;
            };
            // `src_len == len` with a non-empty chain — every consumed suffix
            // was empty — makes this range inverted. §6.6 says continue, not
            // abort: a `?` here would skip every later reading that would have
            // succeeded. Unreachable in the shipped asset today, because the
            // same-type path fires first for every such match.
            let Some(rest_slice) = text.get(m.start + m.src_len + 1..m.start + m.len) else {
                continue;
            };
            let rest: String = rest_slice.iter().collect();
            return Some(stem + hack + &rest);
        }
    }
    None
}

/// The stem of `s` under `id`'s remove-tense — the same expression
/// `stem::generate_stems` used at build time, so a kana stem exists here iff
/// one was generated then.
fn strip_remove_suffix(table: &ConjugationTable, id: VerbTypeId, s: &str) -> Option<String> {
    let ty = table.types().get(id)?;
    ty.conjugations
        .iter()
        .filter(|c| c.tense == ty.remove_tense && c.form.0 == 0)
        .find_map(|c| kana::strip_suffix_unified(s, &c.suffix))
}

/// ta-old's `kuruHack` destination is `wchar_t[4]` (`Dictionary.h:103`), so at
/// most three characters were ever written; a longer substitution was silently
/// discarded and the scan continued to the next same-named type. Kept, rather
/// than dropped as a C buffer artefact, so Phase 6's differential run compares
/// like for like.
const KURU_HACK_MAX_CHARS: usize = 3;

/// The kana spelling the reading of the leading kanji of `link`'s conjugation
/// suffix, ta-old's `kuruHack` (`Dictionary.cpp:1323-1360`).
///
/// Fires only for a suffix starting with a CJK ideograph, which in the shipped
/// asset means only the kanji `vk` block (来る). Pairing is by verb type
/// *name*: `types_named` deliberately keeps every duplicate-named type
/// reachable so the kana twin can be found. Only `chain[0]` is ever inspected,
/// which is sufficient for arbitrarily deep stacks because `chain[0]` is
/// always the type applied directly to the dictionary stem.
fn kuru_hack(link: &ConjLink, table: &ConjugationTable) -> Option<String> {
    let ty = table.types().get(link.verb_type)?;
    let c = ty.conjugations.get(link.conj)?;
    if !kana::is_cjk_ideograph(c.suffix.chars().next()?) {
        return None;
    }
    let len = c.suffix.chars().count();

    for twin in table.types_named(&ty.name) {
        if twin == link.verb_type {
            continue;
        }
        let Some(twin_ty) = table.types().get(twin) else {
            continue;
        };
        // ta-old keeps the SHORTEST twin suffix at least as long as this one
        // (`len2 < len || len2 >= best` rejects), not the longest.
        let mut best: Option<(usize, &str)> = None;
        for c2 in &twin_ty.conjugations {
            if c2.tense != c.tense || c2.form != c.form || c2.next_verb_type != c.next_verb_type {
                continue;
            }
            let len2 = c2.suffix.chars().count();
            if len2 < len || best.is_some_and(|(shortest, _)| len2 >= shortest) {
                continue;
            }
            if tails_match(&c.suffix, &c2.suffix, len, len2) {
                best = Some((len2, c2.suffix.as_str()));
            }
        }
        let Some((len2, suffix)) = best else { continue };
        let want = len2 - len + 1;
        if want <= KURU_HACK_MAX_CHARS {
            return Some(suffix.chars().take(want).collect());
        }
        // No break: ta-old skips an over-long twin and keeps scanning
        // (`Dictionary.cpp:1355`).
    }
    None
}

/// ta-old's `wcsnijcmp(conj->suffix + 1, conj2->suffix + len2 - len + 1,
/// len - 1)`: the two suffixes must agree under `kana::unify` once each has
/// lost its leading substitution characters.
fn tails_match(kanji_suffix: &str, kana_suffix: &str, len: usize, len2: usize) -> bool {
    let kanji_tail: String = kanji_suffix.chars().skip(1).collect();
    let kana_tail: Vec<char> = kana_suffix.chars().skip(len2 - len + 1).collect();
    matcher::unified_eq(&kana_tail, &kanji_tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{ConjugationTable, VerbTypeId};
    use crate::matcher::{ConjLink, Match};

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("the embedded table must load")
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// The single type with this name. Panics for `vk`/`vs`/`v5r-i`/`v5uru`,
    /// which is the point: those four need `vk_kanji`-style disambiguation.
    fn only(t: &ConjugationTable, name: &str) -> VerbTypeId {
        let ids = t.types_named(name);
        assert_eq!(ids.len(), 1, "{name} must name exactly one type");
        ids[0]
    }

    /// The `vk` block whose suffixes are spelled in kanji. `types_named`
    /// returns both blocks and which one comes first is an asset-ordering
    /// detail no test may depend on, so pick it by the property that actually
    /// matters: it is the one with kanji-leading suffixes.
    fn vk_kanji(t: &ConjugationTable) -> VerbTypeId {
        t.types_named("vk")
            .into_iter()
            .find(|&id| {
                t.types()[id].conjugations.iter().any(|c| {
                    c.suffix
                        .chars()
                        .next()
                        .is_some_and(crate::kana::is_cjk_ideograph)
                })
            })
            .expect("one vk block must carry kanji suffixes")
    }

    /// A `ConjLink` for the first form-0 conjugation of `ty` whose **stored**
    /// suffix is `suffix` and whose chaining state is `chained`.
    ///
    /// Stored suffixes are post-trim: a conjugation naming a `Next Type` had
    /// that type's remove-suffix stripped at load, so `vk`'s Potential is
    /// stored as `来られ`, not the asset's `来られる`. `chained` disambiguates
    /// the pairs that share a spelling — `vk` has both a terminal Non-past
    /// `来る` and a chaining Stem `来る`. It does **not** disambiguate two
    /// chaining rows with the same spelling; where the asset has such a pair
    /// (`来られ` is both Potential and Passive into `v1`), the caller asserts
    /// the tense it got.
    fn link(t: &ConjugationTable, ty: VerbTypeId, suffix: &str, chained: bool) -> ConjLink {
        let vt = &t.types()[ty];
        let conj = vt
            .conjugations
            .iter()
            .position(|c| {
                c.form.0 == 0 && c.suffix == suffix && c.next_verb_type.is_some() == chained
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} has no form-0 conjugation {suffix:?} (chained={chained})",
                    vt.name
                )
            });
        ConjLink {
            verb_type: ty,
            tense: vt.conjugations[conj].tense,
            form: vt.conjugations[conj].form,
            conj,
        }
    }

    fn hit(surface: &str, start: usize, src_len: usize, len: usize, chain: Vec<ConjLink>) -> Match {
        Match {
            start,
            len,
            src_len,
            surface: surface.to_string(),
            flags: WordFlags::PRIMARY,
            entry_id: 1,
            inexact: false,
            chain,
        }
    }

    fn sense(pos: &[&str], glosses: &[&str]) -> Sense {
        Sense {
            pos: pos.iter().map(|s| (*s).to_string()).collect(),
            glosses: glosses.iter().map(|s| (*s).to_string()).collect(),
            xrefs: Vec::new(),
            misc: Vec::new(),
            info: Vec::new(),
        }
    }

    fn make_entry(readings: &[&str], senses: Vec<Sense>) -> EntryData {
        EntryData {
            id: 1,
            readings: readings.iter().map(|s| (*s).to_string()).collect(),
            senses,
        }
    }

    #[test]
    fn kuru_hack_reads_the_kanji_of_a_plain_non_past_suffix() {
        // 来る's Non-past twin is くる. len == len2 == 2, so want == 1 and the
        // substitution is the twin's first character: く.
        let t = table();
        let vk = vk_kanji(&t);
        assert_eq!(
            kuru_hack(&link(&t, vk, "来る", false), &t).as_deref(),
            Some("く")
        );
    }

    #[test]
    fn kuru_hack_reads_a_chained_suffix_after_load_time_trimming() {
        // The asset spells this Potential 来られる with Next Type v1; the
        // loader strips v1's remove-suffix る, so the STORED suffix is 来られ
        // and its twin is こられ. len == len2 == 3, want == 1 → こ.
        //
        // vk's kanji block has TWO form-0 chained rows spelled 来られ —
        // Potential and Passive, both into v1 — and `link` takes whichever the
        // asset lists first. Pin it, because both twins are こられ and the
        // assertion would otherwise pass without testing what its name says.
        let t = table();
        let vk = vk_kanji(&t);
        let l = link(&t, vk, "来られ", true);
        assert_eq!(t.tense_name(l.tense), Some("Potential"));
        assert_eq!(kuru_hack(&l, &t).as_deref(), Some("こ"));
    }

    #[test]
    fn kuru_hack_returns_none_when_the_kana_twin_has_no_such_tense() {
        // 来 Imperfective. The kana vk block goes Imperative → Hypothetical
        // with no Imperfective row at all, so no twin conjugation matches on
        // (tense, form, next_verb_type) and the scan yields nothing. This is
        // a normal outcome, not an error.
        let t = table();
        let vk = vk_kanji(&t);
        assert_eq!(kuru_hack(&link(&t, vk, "来", false), &t), None);
    }

    #[test]
    fn kuru_hack_returns_none_for_a_suffix_that_does_not_start_with_a_kanji() {
        // v1's Non-past is る. The CJK check fires before any twin scan.
        let t = table();
        let v1 = only(&t, "v1");
        assert_eq!(kuru_hack(&link(&t, v1, "る", false), &t), None);
    }

    #[test]
    fn reconstructs_an_irregular_reading_through_the_kana_twin() {
        // 来た: the vk stem is the empty string, chain[0] is the Stem
        // conjugation 来た → v-ta-stem, and the kanji vk type cannot strip
        // itself off くる. kuru_hack pairs it with the kana vk twin's きた,
        // yielding き, then the twin's own stem ("") plus き plus the text
        // after the one kanji ("た") gives きた.
        let t = table();
        let vk = vk_kanji(&t);
        let m = hit("", 0, 0, 2, vec![link(&t, vk, "来た", true)]);
        let data = make_entry(&["くる"], vec![sense(&["vk"], &["to come"])]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("来た")).as_deref(),
            Some("きた")
        );
    }

    #[test]
    fn reconstructs_a_regular_verb_reading_from_the_same_type() {
        // 食べる: the kana spelling たべる conjugates with the same v1 rows,
        // so the same-type path strips る to たべ and re-appends the matched
        // tail text[src_len..len] == る.
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        let data = make_entry(&["たべる"], vec![]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("食べる")).as_deref(),
            Some("たべる")
        );
    }

    #[test]
    fn returns_the_first_reading_for_a_plain_headword_match() {
        // 言う lists いう then ゆう; ta-old rendered the first PRONOUNCE
        // sibling it found walking the entry's spelling chain.
        let t = table();
        let m = hit("言う", 0, 2, 2, vec![]);
        let data = make_entry(&["いう", "ゆう"], vec![]);
        assert_eq!(
            reconstruct_reading(&m, &data, &t, &chars("言う")).as_deref(),
            Some("いう")
        );
    }

    #[test]
    fn returns_no_reading_for_a_match_that_is_already_a_reading() {
        let t = table();
        let mut m = hit("いう", 0, 2, 2, vec![]);
        m.flags = WordFlags::PRONOUNCE;
        let data = make_entry(&["いう"], vec![]);
        assert_eq!(reconstruct_reading(&m, &data, &t, &chars("いう")), None);
    }

    #[test]
    fn returns_no_reading_for_a_kana_only_entry() {
        // A kana-only entry stores no readings: its surface already is one.
        let t = table();
        let m = hit("は", 0, 1, 1, vec![]);
        let data = make_entry(&[], vec![]);
        assert_eq!(reconstruct_reading(&m, &data, &t, &chars("は")), None);
    }

    #[test]
    fn dictionary_form_restores_the_remove_suffix_of_a_stem() {
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        assert_eq!(dictionary_form(&m, &t), "食べる");
    }

    #[test]
    fn dictionary_form_of_a_plain_headword_is_its_surface() {
        let t = table();
        let m = hit("高い", 0, 2, 2, vec![]);
        assert_eq!(dictionary_form(&m, &t), "高い");
    }

    #[test]
    fn dictionary_form_restores_an_empty_stem() {
        // The whole surface of 来る is its own remove-suffix, so the stem is
        // "" — Phase 1A's empty-key case. The dictionary form must still come
        // back whole.
        let t = table();
        let vk = vk_kanji(&t);
        let m = hit("", 0, 0, 2, vec![link(&t, vk, "来る", false)]);
        assert_eq!(dictionary_form(&m, &t), "来る");
    }

    #[test]
    fn assemble_entry_renders_the_conjugation_label_and_dedupes_pos() {
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 3, vec![link(&t, v1, "る", false)]);
        let data = make_entry(
            &["たべる"],
            vec![
                sense(&["v1", "vt"], &["to eat"]),
                sense(&["v1"], &["to live on"]),
            ],
        );
        let e = assemble_entry(&m, &data, &t, &chars("食べる"));
        assert_eq!(e.headword, "食べる");
        assert_eq!(e.reading.as_deref(), Some("たべる"));
        assert_eq!(e.conjugation.as_deref(), Some("Non-past"));
        assert_eq!(e.pos, vec!["v1", "vt"], "first-seen order, deduped");
        assert_eq!(e.senses.len(), 2);
        assert_eq!(e.flags, WordFlags::PRIMARY);
    }

    #[test]
    fn assemble_entry_leaves_conjugation_none_for_a_plain_headword() {
        let t = table();
        let m = hit("高い", 0, 2, 2, vec![]);
        let data = make_entry(&["たかい"], vec![sense(&["adj-i"], &["tall"])]);
        let e = assemble_entry(&m, &data, &t, &chars("高い"));
        assert_eq!(e.headword, "高い");
        assert_eq!(e.conjugation, None);
        assert_eq!(e.reading.as_deref(), Some("たかい"));
        assert_eq!(e.pos, vec!["adj-i"]);
    }

    #[test]
    fn assemble_entry_leaves_conjugation_none_when_the_label_renders_empty() {
        // GetConjString skips Stem at every depth, so a chain of nothing but
        // Stem links renders as "". That is "no label", not an empty label.
        let t = table();
        let v1 = only(&t, "v1");
        let m = hit("食べ", 0, 2, 2, vec![link(&t, v1, "", true)]);
        let data = make_entry(&["たべる"], vec![]);
        assert_eq!(crate::matcher::render_conjugation_label(&m.chain, &t), "");
        let e = assemble_entry(&m, &data, &t, &chars("食べ"));
        assert_eq!(e.conjugation, None);
    }
}
