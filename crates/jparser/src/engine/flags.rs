// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! `RawEntry` to `Headword`: one record per kanji form and per reading form,
//! flagged for the cost model.
//!
//! The `PRIMARY`/`PRONOUNCE` split is load-bearing (spec §7.3): a kana-only
//! entry's reading is `PRIMARY`, never `PRONOUNCE`, which is what keeps
//! ordinary particles like は and こう out of the cost term that prices a
//! `PRONOUNCE`-only match crossing a tokenizer boundary.

use super::table::classes_for_pos;
use super::ClassId;
use crate::jmdict::RawEntry;

/// POS codes that make a word a particle for scoring purposes.
const PARTICLE_POS: &[&str] = &["prt", "conj"];
/// POS codes that make a word a counter.
const COUNTER_POS: &[&str] = &["ctr", "suf"];
/// Misc code that disqualifies a counter.
const ARCHAIC_MISC: &str = "arch";

/// Per-record flags the cost model and the webview read.
///
/// A `u16` newtype with eight named bits. The eight **names** are the
/// contract: they are the strings `Entry::flags` serialises and
/// `src/types.ts`'s `FlagName` declares, and the webview picks a chip's
/// content class by them. The bit **values** are arbitrary — an index stores
/// the raw `u16` (`StoredRecord::flags`), and Phase 1b-ii bumped the index
/// format at the same time it assigned these, so nothing built before them
/// can be read regardless. See `headwords` below for when each is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WordFlags(pub u16);

impl WordFlags {
    pub const PRIMARY: WordFlags = WordFlags(1 << 0);
    pub const PRONOUNCE: WordFlags = WordFlags(1 << 1);
    pub const COMMON_LINE: WordFlags = WordFlags(1 << 2);
    pub const COMMON: WordFlags = WordFlags(1 << 3);
    pub const PARTICLE: WordFlags = WordFlags(1 << 4);
    pub const COUNTER: WordFlags = WordFlags(1 << 5);
    /// Reserved; set by nothing (Phase 1a design §8).
    pub const TOP: WordFlags = WordFlags(1 << 6);
    /// Reserved for JMnedict; set by nothing (Phase 1a design §8).
    pub const IS_NAME: WordFlags = WordFlags(1 << 7);

    /// Every flag with its wire name, in bit order.
    const NAMES: [(WordFlags, &'static str); 8] = [
        (Self::PRIMARY, "primary"),
        (Self::PRONOUNCE, "pronounce"),
        (Self::COMMON_LINE, "common_line"),
        (Self::COMMON, "common"),
        (Self::PARTICLE, "particle"),
        (Self::COUNTER, "counter"),
        (Self::TOP, "top"),
        (Self::IS_NAME, "is_name"),
    ];

    pub fn contains(self, other: WordFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: WordFlags) {
        self.0 |= other.0;
    }

    /// The wire names of every set flag, in bit order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        Self::NAMES
            .iter()
            .filter(move |(flag, _)| self.contains(*flag))
            .map(|(_, name)| *name)
    }
}

impl serde::Serialize for WordFlags {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.names())
    }
}

/// One headword — a single kanji or reading form of a `RawEntry`, flagged
/// for the cost model and tagged with the conjugation classes its senses map
/// to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headword {
    pub surface: String,
    pub flags: WordFlags,
    pub classes: Vec<ClassId>,
    pub entry_id: u32,
}

/// Every POS code across every sense of `raw`, in order, without duplicates.
fn all_pos(raw: &RawEntry) -> impl Iterator<Item = &str> {
    raw.senses
        .iter()
        .flat_map(|s| s.pos.iter().map(String::as_str))
}

/// Every misc code across every sense of `raw`.
fn all_misc(raw: &RawEntry) -> impl Iterator<Item = &str> {
    raw.senses
        .iter()
        .flat_map(|s| s.misc.iter().map(String::as_str))
}

/// `classes_for_pos` over every sense's POS codes, deduplicated. An entry
/// tagged `v5r` in one sense and `v1` in another legitimately yields two
/// classes, and `vk` alone yields two, which is why this is a `Vec` and not
/// an `Option`.
fn classes(raw: &RawEntry) -> Vec<ClassId> {
    let mut out = Vec::new();
    for pos in all_pos(raw) {
        for class in classes_for_pos(pos) {
            if !out.contains(&class) {
                out.push(class);
            }
        }
    }
    out
}

/// `PARTICLE`/`COUNTER`/`COMMON_LINE` are properties of the entry as a
/// whole, shared by every headword it produces.
fn shared_flags(raw: &RawEntry) -> WordFlags {
    let mut flags = WordFlags::default();

    if all_pos(raw).any(|p| PARTICLE_POS.contains(&p)) {
        flags.insert(WordFlags::PARTICLE);
    }

    let has_counter_pos = all_pos(raw).any(|p| COUNTER_POS.contains(&p));
    let has_archaic_sense = all_misc(raw).any(|m| m == ARCHAIC_MISC);
    if has_counter_pos && !has_archaic_sense {
        flags.insert(WordFlags::COUNTER);
    }

    let any_priority =
        raw.kanji.iter().any(|k| k.has_priority) || raw.readings.iter().any(|r| r.has_priority);
    if any_priority {
        flags.insert(WordFlags::COMMON_LINE);
    }

    flags
}

/// One `Headword` per kanji form, in document order, followed by one per
/// reading form, in document order.
pub fn headwords(raw: &RawEntry) -> Vec<Headword> {
    let shared = shared_flags(raw);
    let has_kanji = !raw.kanji.is_empty();
    let classes = classes(raw);

    let kanji_headwords = raw.kanji.iter().map(|k| {
        let mut flags = shared;
        flags.insert(WordFlags::PRIMARY);
        if k.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        Headword {
            surface: k.text.clone(),
            flags,
            classes: classes.clone(),
            entry_id: raw.id,
        }
    });

    let reading_headwords = raw.readings.iter().map(|r| {
        let mut flags = shared;
        // A reading of a kanji entry pronounces that kanji; a reading of a
        // kana-only entry *is* the headword, so it is PRIMARY instead.
        flags.insert(if has_kanji {
            WordFlags::PRONOUNCE
        } else {
            WordFlags::PRIMARY
        });
        if r.has_priority {
            flags.insert(WordFlags::COMMON);
        }
        Headword {
            surface: r.text.clone(),
            flags,
            classes: classes.clone(),
            entry_id: raw.id,
        }
    });

    kanji_headwords.chain(reading_headwords).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jmdict::{KanjiForm, RawEntry, RawSense, ReadingForm};

    fn entry(
        kanji: &[(&str, bool)],
        readings: &[(&str, bool)],
        pos: &[&str],
        misc: &[&str],
    ) -> RawEntry {
        RawEntry {
            id: 1,
            kanji: kanji
                .iter()
                .map(|(t, p)| KanjiForm {
                    text: t.to_string(),
                    has_priority: *p,
                })
                .collect(),
            readings: readings
                .iter()
                .map(|(t, p)| ReadingForm {
                    text: t.to_string(),
                    has_priority: *p,
                })
                .collect(),
            senses: vec![RawSense {
                pos: pos.iter().map(|s| s.to_string()).collect(),
                misc: misc.iter().map(|s| s.to_string()).collect(),
                ..Default::default()
            }],
        }
    }

    fn find<'a>(hs: &'a [Headword], surface: &str) -> &'a Headword {
        hs.iter().find(|h| h.surface == surface).expect(surface)
    }

    #[test]
    fn a_kanji_form_is_primary_and_its_reading_is_pronounce() {
        let hs = headwords(&entry(&[("履く", true)], &[("はく", false)], &["v5k"], &[]));
        assert!(find(&hs, "履く").flags.contains(WordFlags::PRIMARY));
        assert!(find(&hs, "はく").flags.contains(WordFlags::PRONOUNCE));
        assert!(!find(&hs, "はく").flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn a_kana_only_entry_is_primary_not_pronounce() {
        // The property spec §7.3 depends on: は and こう must not be caught by
        // the PRONOUNCE-crossing cost term.
        let hs = headwords(&entry(&[], &[("は", true)], &["prt"], &[]));
        assert!(find(&hs, "は").flags.contains(WordFlags::PRIMARY));
        assert!(!find(&hs, "は").flags.contains(WordFlags::PRONOUNCE));
    }

    #[test]
    fn priority_marks_the_form_and_the_line() {
        let hs = headwords(&entry(
            &[("東京", true), ("東亰", false)],
            &[("とうきょう", false)],
            &["n"],
            &[],
        ));
        assert!(find(&hs, "東京").flags.contains(WordFlags::COMMON));
        assert!(find(&hs, "東亰").flags.contains(WordFlags::COMMON_LINE));
        assert!(!find(&hs, "東亰").flags.contains(WordFlags::COMMON));
    }

    #[test]
    fn particles_and_counters_are_flagged() {
        let hs = headwords(&entry(&[], &[("が", false)], &["prt"], &[]));
        assert!(find(&hs, "が").flags.contains(WordFlags::PARTICLE));
        let hs = headwords(&entry(&[("日", false)], &[("にち", false)], &["ctr"], &[]));
        assert!(find(&hs, "日").flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn an_archaic_counter_is_not_a_counter() {
        let hs = headwords(&entry(
            &[("枚", false)],
            &[("まい", false)],
            &["ctr"],
            &["arch"],
        ));
        assert!(!find(&hs, "枚").flags.contains(WordFlags::COUNTER));
    }

    #[test]
    fn conjugation_classes_come_from_every_senses_pos() {
        let hs = headwords(&entry(
            &[("食べる", true)],
            &[("たべる", false)],
            &["v1"],
            &[],
        ));
        assert!(!find(&hs, "食べる").classes.is_empty());
    }

    #[test]
    fn flags_serialise_as_their_wire_names_in_bit_order() {
        let mut f = WordFlags::default();
        f.insert(WordFlags::COMMON);
        f.insert(WordFlags::PRIMARY);
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            r#"["primary","common"]"#,
            "the eight names are what src/types.ts's FlagName declares"
        );
        assert_eq!(serde_json::to_string(&WordFlags::default()).unwrap(), "[]");
    }

    #[test]
    fn contains_and_insert_are_bitwise() {
        let mut f = WordFlags::PRIMARY;
        assert!(f.contains(WordFlags::PRIMARY));
        assert!(!f.contains(WordFlags::COMMON));
        f.insert(WordFlags::COMMON);
        assert!(f.contains(WordFlags::COMMON));
        assert!(f.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn the_eight_wire_names_are_exactly_types_ts_flagname() {
        let names: Vec<&str> = WordFlags(u16::MAX).names().collect();
        assert_eq!(
            names,
            [
                "primary",
                "pronounce",
                "common_line",
                "common",
                "particle",
                "counter",
                "top",
                "is_name"
            ]
        );
    }

    #[test]
    fn top_and_is_name_are_never_set() {
        for h in headwords(&entry(
            &[("東京", true)],
            &[("とうきょう", false)],
            &["n"],
            &[],
        )) {
            assert!(!h.flags.contains(WordFlags::TOP));
            assert!(!h.flags.contains(WordFlags::IS_NAME));
        }
    }
}
