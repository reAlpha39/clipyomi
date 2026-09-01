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

//! Candidates to entries: turning the winning path's [`Step`]s into the
//! public [`Segment`]/[`Entry`] shape the application consumes.
//!
//! Per spec §8, [`Entry::conjugation`] stays `None` throughout Phase 1a --
//! Phase 1b writes the formatter that renders `Entry::steps` into an English
//! label. What this module owns instead is the one piece Phase 1a *does*
//! have to get right: reading reconstruction. `EntryData::readings` holds a
//! reading only for a kanji entry, and only in its dictionary form, so a
//! conjugated match has to strip that dictionary form's own suffix and
//! rebuild it from the matched surface's tail. A reading that does not
//! strip cleanly is never guessed at -- `None` is what goes out, because a
//! wrong reading renders as furigana the reader has no reason to doubt.

use crate::engine::chain;
use crate::engine::lattice::Step;
use crate::engine::lex::Candidate;
use crate::engine::table::CLASSES;
use crate::index::load::Index;
use crate::index::{EntryData, IndexError, SenseData};
use crate::record::WordFlags;
use crate::{Entry, Segment};

/// Turn one step of the winning path into the [`Segment`] the application
/// consumes: an unmatched run as-is, a match resolved through `index` into
/// one [`Entry`] per same-span sibling candidate.
///
/// `by_start` is the whole per-start candidate table `lex::candidates`
/// produced -- not just `step`'s own span -- because siblings sharing
/// `step`'s span are found by indexing into it directly. Guarded with
/// `by_start.get(start)` rather than trusted to cover `step`'s start: a
/// caller may reasonably pass a table shorter than `chars`.
///
/// `Segment::entries`' order is currently just "the DP's own winner first,
/// then whatever order `by_start` held the rest in" -- Phase 1a has no
/// ranking pass over the alternatives. A later task is expected to impose a
/// real order there; nothing should be built on the current arrangement
/// beyond `entries[0]` being the match the DP actually chose.
pub fn to_segment(
    index: &Index,
    chars: &[char],
    step: &Step,
    by_start: &[Vec<Candidate>],
) -> Result<Segment, IndexError> {
    let (start, len) = match step {
        Step::Matched(cand) => (cand.start, cand.len),
        Step::Unmatched { start, len } => (*start, *len),
    };
    let surface: String = chars[start..start + len].iter().collect();

    let Step::Matched(cand) = step else {
        return Ok(Segment {
            start,
            len,
            surface,
            reading: None,
            matched: false,
            entries: Vec::new(),
        });
    };

    // `cand` itself is always first (see this function's doc comment on
    // `entries`' order), and `Segment::reading` is documented as coming from
    // `entries[0].reading`.
    let siblings = by_start.get(start).map(Vec::as_slice).unwrap_or(&[]);
    let mut entries = Vec::with_capacity(1);
    entries.push(build_entry(index, chars, cand)?);
    for sibling in siblings {
        if sibling.len == cand.len && sibling != cand {
            entries.push(build_entry(index, chars, sibling)?);
        }
    }

    let reading = entries[0].reading.clone();
    Ok(Segment {
        start,
        len,
        surface,
        reading,
        matched: true,
        entries,
    })
}

/// One [`Entry`] for `cand`: its dictionary data resolved through `index`,
/// its reading reconstructed per this module's contract, and `conjugation`
/// left `None` for Phase 1b to fill.
fn build_entry(index: &Index, chars: &[char], cand: &Candidate) -> Result<Entry, IndexError> {
    // A candidate's `entry_id` always names an entry in the same index it
    // was matched from -- `lex::candidates` reads both from `index` in the
    // same call. A well-formed index never disagrees with itself.
    let entry_data = index.entry(cand.entry_id)?.expect(
        "invariant: lex::candidates only ever emits a Candidate whose entry_id it just \
         read out of this same `index`, so `index.entry(cand.entry_id)` resolving to None \
         here means the index disagreed with itself between the two calls",
    );

    let reading = reading_for(&entry_data, cand, chars);
    let pos = union_pos(&entry_data.senses);

    Ok(Entry {
        headword: cand.headword.clone(),
        reading,
        conjugation: None,
        pos,
        senses: entry_data.senses,
        flags: cand.flags,
        entry_id: cand.entry_id,
        steps: chain::steps_for(cand),
    })
}

/// Every sense's POS code, in first-seen order, deduplicated globally --
/// not just between neighbours, since the same code can legitimately recur
/// in a later sense.
fn union_pos(senses: &[SenseData]) -> Vec<String> {
    let mut pos = Vec::new();
    for sense in senses {
        for code in &sense.pos {
            if !pos.contains(code) {
                pos.push(code.clone());
            }
        }
    }
    pos
}

/// Reconstruct `cand`'s reading from `entry`'s stored data, or `None` when
/// no reading can be attached without inventing one.
///
/// - A [`WordFlags::PRONOUNCE`] match already *is* a reading (it is a kana
///   `<reb>` form of a kanji entry) -- it gets none of its own.
/// - `entry.readings` is empty for a kana-only entry, where the surface
///   already is the reading -- also none.
/// - An uninflected match (`cand.steps` empty) is a plain kanji headword:
///   `entry.readings[0]` applies as stored.
/// - A conjugated match's stored reading is the *dictionary* form's. The
///   innermost step (`cand.steps[0]`, per `chain`'s contract) names the
///   class the entry itself conjugates in; stripping that class's
///   `remove` suffix off the dictionary reading recovers the reading's own
///   stem, and the matched surface's tail from that same point onward
///   (`chars[step.range.start..cand.start + cand.len]`) is already kana in
///   the ordinary case, so appending it verbatim reconstructs the
///   conjugated reading (書いた -> stem "か" + tail "いた" -> かいた). When
///   the dictionary reading does not end in that suffix at all -- an
///   irregular verb whose kana stem does not track its kanji stem, such as
///   来る's く/こ/き alternation -- the strip fails and this returns `None`
///   rather than emit a reading built on a wrong assumption.
fn reading_for(entry: &EntryData, cand: &Candidate, chars: &[char]) -> Option<String> {
    if cand.flags.contains(WordFlags::PRONOUNCE) {
        return None;
    }
    let dict_reading = entry.readings.first()?;

    let Some(first_step) = cand.steps.first() else {
        return Some(dict_reading.clone());
    };

    let class = &CLASSES[first_step.class as usize];
    let stem = dict_reading.strip_suffix(class.remove)?;
    let tail: String = chars[first_step.range.start..cand.start + cand.len]
        .iter()
        .collect();
    Some(format!("{stem}{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::table::class_for_pos;
    use crate::engine::ConjStep;

    fn entry(readings: &[&str]) -> EntryData {
        EntryData {
            id: 1,
            readings: readings.iter().map(|s| s.to_string()).collect(),
            senses: Vec::new(),
        }
    }

    fn candidate(start: usize, len: usize, flags: WordFlags, steps: Vec<ConjStep>) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: 1,
            flags,
            steps,
            exact: true,
            headword: String::new(),
        }
    }

    #[test]
    fn a_pronounce_match_gets_no_reading_of_its_own() {
        let e = entry(&["かく"]);
        let c = candidate(0, 2, WordFlags::PRONOUNCE, Vec::new());
        assert_eq!(reading_for(&e, &c, &['か', 'く']), None);
    }

    #[test]
    fn a_kana_only_entry_has_no_separate_reading() {
        let e = entry(&[]); // no kanji forms -> no stored readings
        let c = candidate(0, 2, WordFlags::PRIMARY, Vec::new());
        assert_eq!(reading_for(&e, &c, &['は', 'く']), None);
    }

    #[test]
    fn an_uninflected_primary_match_takes_the_first_stored_reading() {
        let e = entry(&["とうきょう", "とうけい"]);
        let c = candidate(0, 2, WordFlags::PRIMARY, Vec::new());
        assert_eq!(
            reading_for(&e, &c, &['東', '京']),
            Some("とうきょう".to_string())
        );
    }

    #[test]
    fn a_conjugated_match_strips_the_dictionary_form_and_appends_the_tail() {
        // 書いた -> 書く (かく). The godan-ka class's `remove` is "く"; かく
        // strips to か, then いた (the matched surface's tail from the
        // stem's end onward) is appended: かいた.
        let e = entry(&["かく"]);
        let class = class_for_pos("v5k").expect("v5k maps to a class");
        let chars: Vec<char> = "書いた".chars().collect();
        let steps = vec![ConjStep {
            class,
            form: 0,
            range: 1..3,
        }];
        let c = candidate(0, 3, WordFlags::PRIMARY, steps);
        assert_eq!(reading_for(&e, &c, &chars), Some("かいた".to_string()));
    }

    fn sense(pos: &[&str]) -> SenseData {
        SenseData {
            pos: pos.iter().map(|s| s.to_string()).collect(),
            glosses: Vec::new(),
            xrefs: Vec::new(),
            misc: Vec::new(),
            info: Vec::new(),
        }
    }

    #[test]
    fn union_pos_removes_a_duplicate_even_when_it_is_not_adjacent() {
        // sense 1 and sense 3 both carry "n", with "vs" in between --
        // `Vec::dedup` alone would miss this, since it only collapses
        // adjacent runs.
        let senses = [sense(&["n"]), sense(&["vs"]), sense(&["n"])];
        assert_eq!(union_pos(&senses), vec!["n".to_string(), "vs".to_string()]);
    }

    #[test]
    fn a_reading_that_does_not_end_in_the_classs_suffix_is_never_invented() {
        // A synthetic mismatch: readings[0] does not end in the ichidan
        // class's "る" at all, so the strip fails outright. `None` is the
        // only correct answer -- a guess here would render as wrong
        // furigana with no visible sign it was fabricated.
        let e = entry(&["かく"]);
        let class = class_for_pos("v1").expect("v1 maps to a class");
        let chars: Vec<char> = "かく".chars().collect();
        let steps = vec![ConjStep {
            class,
            form: 0,
            range: 0..2,
        }];
        let c = candidate(0, 2, WordFlags::PRIMARY, steps);
        assert_eq!(reading_for(&e, &c, &chars), None);
    }
}
