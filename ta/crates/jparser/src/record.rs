// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `RawEntry` to `HeadwordRecord`, with flag derivation.
//!
//! Mirrors the per-headword bookkeeping in ta-old's `CreateDict`
//! (`ta-old/exe/util/Dictionary.cpp:246`). Flag values match ta-old's
//! `JAP_WORD_*` constants (`Dictionary.h:25-43`) so records can be compared
//! against the original during the Phase 1B differential run.

use crate::conjugation::{ConjugationTable, VerbTypeId};
use crate::jmdict::RawEntry;

/// POS codes that make a word a particle for scoring purposes. ta-old's
/// `posList` in `GetPartsOfSpeech` (`Dictionary.cpp:1409`).
const PARTICLE_POS: &[&str] = &["prt", "conj"];
/// POS codes that make a word a counter.
const COUNTER_POS: &[&str] = &["ctr", "suf"];
/// Misc code that disqualifies a counter.
const ARCHAIC_MISC: &str = "arch";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WordFlags(pub u16);

impl WordFlags {
    /// Kanji spelling of a kanji word, or the hiragana of a kana-only word.
    pub const PRIMARY: WordFlags = WordFlags(0x0001);
    /// A reading for something that also has a kanji spelling.
    pub const PRONOUNCE: WordFlags = WordFlags(0x0002);
    /// Any form of this entry carries a priority marker.
    pub const COMMON_LINE: WordFlags = WordFlags(0x0004);
    /// This specific form carries a priority marker.
    pub const COMMON: WordFlags = WordFlags(0x0008);
    pub const PARTICLE: WordFlags = WordFlags(0x0010);
    pub const COUNTER: WordFlags = WordFlags(0x0020);
    /// Reserved for JMnedict. Nothing sets this in v1.
    pub const IS_NAME: WordFlags = WordFlags(0x0040);

    pub fn contains(self, other: WordFlags) -> bool {
        self.0 & other.0 == other.0
    }
    pub fn insert(&mut self, other: WordFlags) {
        self.0 |= other.0;
    }
    pub fn remove(&mut self, other: WordFlags) {
        self.0 &= !other.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadwordRecord {
    pub surface: String,
    pub flags: WordFlags,
    /// Every conjugation type this word can take. More than one when the POS
    /// code is duplicated in the table (`vk`, `vs`, `v5r-i`, `v5uru`).
    pub verb_types: Vec<VerbTypeId>,
    pub entry_id: u32,
}

/// True when the sense set makes this a counter: a counter POS present and no
/// archaic marker anywhere. Matches ta-old's
/// `(wcsstr(eng, "(ctr)") || wcsstr(eng, "(suf)")) && !wcsstr(eng, "(arch)")`.
pub(crate) fn counter_flag(pos: &[&str], misc: &[&str]) -> bool {
    let has_counter = pos.iter().any(|p| COUNTER_POS.contains(p));
    let archaic = misc.iter().any(|m| *m == ARCHAIC_MISC);
    has_counter && !archaic
}

/// Expand one entry into headword records: every kanji form, then every reading
/// form, in document order.
pub fn headwords(entry: &RawEntry, table: &ConjugationTable) -> Vec<HeadwordRecord> {
    let all_pos: Vec<&str> = entry
        .senses
        .iter()
        .flat_map(|s| s.pos.iter().map(String::as_str))
        .collect();
    let all_misc: Vec<&str> = entry
        .senses
        .iter()
        .flat_map(|s| s.misc.iter().map(String::as_str))
        .collect();

    let mut shared = WordFlags::default();
    if all_pos.iter().any(|p| PARTICLE_POS.contains(p)) {
        shared.insert(WordFlags::PARTICLE);
    }
    if counter_flag(&all_pos, &all_misc) {
        shared.insert(WordFlags::COUNTER);
    }
    if entry.kanji.iter().any(|k| k.has_priority) || entry.readings.iter().any(|r| r.has_priority) {
        shared.insert(WordFlags::COMMON_LINE);
    }

    // A POS code may name several types; every one is attached.
    let mut verb_types: Vec<VerbTypeId> = Vec::new();
    for p in &all_pos {
        for id in table.types_named(p) {
            if !verb_types.contains(&id) {
                verb_types.push(id);
            }
        }
    }

    let has_kanji = !entry.kanji.is_empty();
    let mut out = Vec::with_capacity(entry.kanji.len() + entry.readings.len());

    for (i, k) in entry.kanji.iter().enumerate() {
        let mut flags = shared;
        if i == 0 {
            flags.insert(WordFlags::PRIMARY);
        }
        if k.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        out.push(HeadwordRecord {
            surface: k.text.clone(),
            flags,
            verb_types: verb_types.clone(),
            entry_id: entry.id,
        });
    }

    for (i, r) in entry.readings.iter().enumerate() {
        let mut flags = shared;
        if has_kanji {
            flags.insert(WordFlags::PRONOUNCE);
        } else if i == 0 {
            flags.insert(WordFlags::PRIMARY);
        }
        if r.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        out.push(HeadwordRecord {
            surface: r.text.clone(),
            flags,
            verb_types: verb_types.clone(),
            entry_id: entry.id,
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::ConjugationTable;
    use crate::jmdict::{parse_entries, RawEntry};

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().unwrap()
    }

    fn fixture() -> Vec<RawEntry> {
        let xml = include_str!("../tests/fixtures/jmdict_mini.xml");
        parse_entries(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn emits_one_record_per_kanji_and_reading_form() {
        let recs = headwords(&fixture()[0], &table());
        let surfaces: Vec<&str> = recs.iter().map(|r| r.surface.as_str()).collect();
        assert_eq!(surfaces, vec!["言う", "いう", "ゆう"]);
    }

    #[test]
    fn marks_only_the_first_headword_primary() {
        let recs = headwords(&fixture()[0], &table());
        assert!(recs[0].flags.contains(WordFlags::PRIMARY));
        assert!(!recs[1].flags.contains(WordFlags::PRIMARY));
        assert!(!recs[2].flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn marks_readings_pronounce_when_the_entry_has_kanji() {
        let recs = headwords(&fixture()[0], &table());
        assert!(!recs[0].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[1].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[2].flags.contains(WordFlags::PRONOUNCE));
    }

    #[test]
    fn treats_a_kana_only_reading_as_primary() {
        // は has no kanji form, so its reading is the primary spelling.
        let recs = headwords(&fixture()[1], &table());
        assert_eq!(recs.len(), 1);
        assert!(!recs[0].flags.contains(WordFlags::PRONOUNCE));
        assert!(recs[0].flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn sets_common_per_form_and_common_line_per_entry() {
        // 言う: keb and the first reb are marked; ゆう is not.
        let recs = headwords(&fixture()[0], &table());
        assert!(recs[0].flags.contains(WordFlags::COMMON));
        assert!(recs[1].flags.contains(WordFlags::COMMON));
        assert!(!recs[2].flags.contains(WordFlags::COMMON));
        for r in &recs {
            assert!(r.flags.contains(WordFlags::COMMON_LINE));
        }
    }

    #[test]
    fn omits_common_line_when_no_form_has_priority() {
        let recs = headwords(&fixture()[2], &table());
        assert!(!recs[0].flags.contains(WordFlags::COMMON_LINE));
    }

    #[test]
    fn sets_particle_flag_from_pos() {
        let recs = headwords(&fixture()[1], &table());
        assert!(recs[0].flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn does_not_set_particle_on_verbs() {
        let recs = headwords(&fixture()[0], &table());
        assert!(!recs[0].flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn attaches_verb_types_from_pos_codes() {
        let t = table();
        let recs = headwords(&fixture()[0], &t);
        let expected = t.types_named("v5r");
        assert!(!expected.is_empty(), "v5r must exist in the table");
        assert_eq!(recs[0].verb_types, expected);
    }

    #[test]
    fn attaches_adjective_types() {
        let t = table();
        let recs = headwords(&fixture()[2], &t);
        assert_eq!(recs[0].verb_types, t.types_named("adj-i"));
    }

    #[test]
    fn leaves_verb_types_empty_for_non_conjugating_words() {
        let recs = headwords(&fixture()[1], &table());
        assert!(recs[0].verb_types.is_empty());
    }

    #[test]
    fn counter_requires_ctr_or_suf_and_forbids_arch() {
        assert!(counter_flag(&["ctr"], &[]));
        assert!(counter_flag(&["suf"], &[]));
        assert!(!counter_flag(&["ctr"], &["arch"]));
        assert!(!counter_flag(&["n"], &[]));
    }

    #[test]
    fn flag_operations_insert_remove_and_test() {
        let mut f = WordFlags::default();
        f.insert(WordFlags::COMMON);
        assert!(f.contains(WordFlags::COMMON));
        f.remove(WordFlags::COMMON);
        assert!(!f.contains(WordFlags::COMMON));
    }

    #[test]
    fn flag_values_match_ta_old_constants() {
        // Kept identical to JAP_WORD_* so the Phase 1B differential run can
        // compare flags directly.
        assert_eq!(WordFlags::PRIMARY.0, 0x0001);
        assert_eq!(WordFlags::PRONOUNCE.0, 0x0002);
        assert_eq!(WordFlags::COMMON_LINE.0, 0x0004);
        assert_eq!(WordFlags::COMMON.0, 0x0008);
        assert_eq!(WordFlags::PARTICLE.0, 0x0010);
        assert_eq!(WordFlags::COUNTER.0, 0x0020);
    }
}
