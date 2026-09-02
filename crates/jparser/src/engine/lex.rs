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

//! Candidate generation: every dictionary entry that could match some span
//! of the input, including through deinflection.
//!
//! The segmentation DP (later tasks) can only choose among what this module
//! produces, so a match is never suppressed here for looking dubious -- it is
//! generated and flagged instead. `exact` is that flag: false whenever a
//! candidate exists only because [`kana::fold`] unified hiragana and
//! katakana, which is the signal spec §7.4's cost term prices.

use std::collections::HashMap;

use crate::engine::kana;
use crate::engine::table::{classes_for_pos, ChainHit, Table, CLASSES};
use crate::engine::{ClassId, ConjStep};
use crate::index::load::Index;
use crate::index::{EntryData, IndexError, StoredRecord};
use crate::record::WordFlags;

/// Longest deinflection chain `chains_ending_at` is asked to search, in
/// characters consumed from a span's tail. Set from the corpus's deepest
/// known chain (master plan §7.3's させ|られ|ませ|ん|でし|た, six steps) plus
/// headroom for chains the fixtures don't yet exercise; whether this value
/// holds against the full gold corpus is unmeasured -- see handoff §8's
/// "three size/shape assumptions" bullet.
const MAX_CHAIN: usize = 12;

/// One dictionary entry that could match a span of the input, possibly
/// through deinflection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub start: usize,
    pub len: usize,
    pub entry_id: u32,
    pub flags: WordFlags,
    pub steps: Vec<ConjStep>,
    /// False when the source characters differ from the stored surface
    /// before folding -- the FST matched only because kana was unified, not
    /// because the two strings actually agree.
    pub exact: bool,
    pub headword: String,
}

/// Every candidate covering some span of `chars`, indexed by start position.
///
/// For each span `[start, end)`, every deinflection chain `table` can strip
/// from its tail turns the remainder into a dictionary key (the bare span
/// itself for the empty chain); every `StoredRecord` the index has under
/// that key, once folded, becomes one `Candidate`. A chain whose class the
/// matched entry does not actually have is discarded -- a 一段 reading of a
/// 五段 verb is not a reading of it -- but an inexact kana match is kept and
/// marked, never dropped.
///
/// **Ordering guarantee:** within `out[start]`, candidates are ordered
/// longest span first. The same headword can legitimately recur at several
/// lengths from the same start -- `Table`'s top-level search accepts any
/// class's any form as a terminal, so a mid-chain fragment (書いた's onbin
/// "書い" alone) can independently reconstruct the same headword as the full
/// chain ("書いた") one step further -- and both are kept, not just the
/// fuller one. A caller that wants the fullest parse for a headword (as
/// `a_conjugated_verb_is_found_with_its_chain` does via `.find()`) can rely
/// on it being first.
///
/// **Dedup guarantee:** within `out[start]`, no two candidates ever share
/// both the same `len` *and* the same `entry_id` -- different chains can
/// independently reconstruct the same entry over the exact same span (an
/// empty-consumption class substitution rediscovering a plain lookup's own
/// key, say), and only the shallowest chain for that `(len, entry_id)` pair
/// survives; see [`dedup_shallowest`]. `len` is part of that key precisely
/// so it never collapses the case above -- 書い and 書いた both reconstruct
/// 書く but at *different* lengths, so they are different keys and both
/// keep their place.
pub fn candidates(
    index: &Index,
    table: &Table,
    chars: &[char],
) -> Result<Vec<Vec<Candidate>>, IndexError> {
    let len = chars.len();
    let mut out: Vec<Vec<Candidate>> = vec![Vec::new(); len];

    // `chains_ending_at(chars, end, ..)` depends only on where a span ends,
    // never on where it starts, so it is computed once per end position and
    // shared by every start that reaches it instead of once per (start, end)
    // pair.
    let chains_by_end: Vec<Vec<ChainHit>> = (1..=len)
        .map(|end| table.chains_ending_at(chars, end, MAX_CHAIN))
        .collect();

    let mut class_cache: HashMap<u32, Vec<ClassId>> = HashMap::new();

    for start in 0..len {
        // Longest span first: a span's own suffix can independently satisfy
        // a shorter span ending mid-chain (e.g. 書いた's onbin fragment 書い
        // alone reconstructs 書く too, one step short of the full た-chain),
        // so the same headword can legitimately recur at several lengths.
        // Both are kept -- the DP chooses among them -- but the fuller match
        // is pushed first.
        for end in (start + 1..=len).rev() {
            for hit in &chains_by_end[end - 1] {
                // `hit.consumed` can never exceed `end`: `Table::search`
                // walks leftward from `end` and stops before running past
                // its start, so this subtraction never wraps.
                let stem_end = end - hit.consumed;
                if stem_end < start {
                    continue;
                }
                let stem: String = chars[start..stem_end].iter().collect();
                let key = match hit.stem_class {
                    Some(class) => format!("{stem}{}", CLASSES[class as usize].remove),
                    None => stem,
                };
                let key_len = key.chars().count();

                for prefix_hit in index.prefixes_of_with(&key, kana::fold)? {
                    // Only the hit spanning the whole key: the FST also
                    // returns every dictionary key that is a prefix of it,
                    // and those are prefixes of the constructed key, not
                    // matches for it.
                    if prefix_hit.key_chars != key_len {
                        continue;
                    }
                    for record in &prefix_hit.records {
                        if !allows_chain(index, &mut class_cache, record, hit.stem_class)? {
                            continue;
                        }
                        out[start].push(Candidate {
                            start,
                            len: end - start,
                            entry_id: record.entry_id,
                            flags: WordFlags(record.flags),
                            steps: hit.steps.clone(),
                            exact: record.surface == key,
                            headword: record.surface.clone(),
                        });
                    }
                }
            }
        }
        out[start] = dedup_shallowest(std::mem::take(&mut out[start]));
    }

    Ok(out)
}

/// Collapse `bucket` (one `start`'s worth of candidates) so no two entries
/// share both `len` and `entry_id`, keeping whichever chain has fewer
/// `steps` -- an uninflected or shallower reading of the same span is the
/// likelier one, the same call `assemble::order` makes at display time. A
/// true tie (equal `steps.len()`, which only happens for a literal
/// duplicate chain) keeps whichever copy was seen first; the two are
/// otherwise indistinguishable.
///
/// Insertion order of the *keys* is preserved -- the first `(len,
/// entry_id)` pair encountered keeps its position in the output -- so this
/// never disturbs the longest-span-first guarantee above: a fuller and a
/// shorter reconstruction of the same headword differ in `len`, so they are
/// different keys, and this function never touches the relative order
/// between different keys, dropped or kept.
fn dedup_shallowest(bucket: Vec<Candidate>) -> Vec<Candidate> {
    let mut best: HashMap<(usize, u32), Candidate> = HashMap::new();
    let mut order: Vec<(usize, u32)> = Vec::new();
    for cand in bucket {
        let key = (cand.len, cand.entry_id);
        match best.get(&key) {
            Some(existing) if existing.steps.len() <= cand.steps.len() => {
                // Already holding an equal-or-shallower chain for this
                // exact (len, entry_id) pair -- `cand` adds nothing.
            }
            _ => {
                if !best.contains_key(&key) {
                    order.push(key);
                }
                best.insert(key, cand);
            }
        }
    }
    order
        .into_iter()
        .map(|key| {
            best.remove(&key)
                .expect("every key in `order` was inserted into `best` right before")
        })
        .collect()
}

/// Whether `record`'s entry actually conjugates in `stem_class` -- `None`
/// (the empty chain, an uninflected lookup) never requires one. Looked up
/// through `index.entry`, memoized per `entry_id` since the same headword
/// recurs across many spans.
fn allows_chain(
    index: &Index,
    cache: &mut HashMap<u32, Vec<ClassId>>,
    record: &StoredRecord,
    stem_class: Option<ClassId>,
) -> Result<bool, IndexError> {
    let Some(class) = stem_class else {
        return Ok(true);
    };
    if let Some(classes) = cache.get(&record.entry_id) {
        return Ok(classes.contains(&class));
    }
    let classes = match index.entry(record.entry_id)? {
        Some(entry) => entry_classes(&entry),
        // A record whose entry is missing has no classes to check against;
        // treated as disqualifying rather than trusted, since a well-formed
        // index never produces this.
        None => Vec::new(),
    };
    let allowed = classes.contains(&class);
    cache.insert(record.entry_id, classes);
    Ok(allowed)
}

/// Every conjugation class any sense of `entry` maps to, deduplicated. Mirrors
/// `engine::flags`'s POS-to-class derivation, but reads it back off the
/// index's stored `EntryData` rather than the original JMdict entry.
fn entry_classes(entry: &EntryData) -> Vec<ClassId> {
    let mut classes = Vec::new();
    for sense in &entry.senses {
        for pos in &sense.pos {
            for class in classes_for_pos(pos) {
                if !classes.contains(&class) {
                    classes.push(class);
                }
            }
        }
    }
    classes
}
