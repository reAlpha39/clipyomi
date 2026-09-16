// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! Candidate generation: every dictionary entry that could match some span
//! of the input, including through deinflection.
//!
//! The segmentation DP (later tasks) can only choose among what this module
//! produces, so a match is never suppressed here for looking dubious -- it is
//! generated and flagged instead. `exact` is that flag: false whenever a
//! candidate exists only because [`kana::fold`] unified hiragana and
//! katakana, which is the signal spec §7.4's cost term prices.
//!
//! Generation walks the key FST once per start position, not once per span:
//! the walk holds the stem, each conjugation class's remove-suffix is tried
//! against it without repeating the walk, and the walk's own dead end is
//! what bounds how far a span can reach. Before Phase 2c this enumerated
//! every `(start, end)` pair and built a key per pair, which was superlinear
//! in input length.

use std::collections::HashMap;

use crate::engine::flags::WordFlags;
use crate::engine::kana;
use crate::engine::table::{classes_for_pos, ChainHit, Table, CLASSES};
use crate::engine::{ClassId, ConjStep};
use crate::index::load::Index;
use crate::index::{EntryData, IndexError, StoredRecord};

/// Longest deinflection chain `chains_ending_at` is asked to search, in
/// characters consumed from a span's tail. Set from the corpus's deepest
/// known chain (master plan §7.3's させ|られ|ませ|ん|でし|た, six steps) plus
/// headroom for chains the fixtures don't yet exercise; whether this value
/// holds against the full gold corpus is **measured, 2026-09-09**: walking
/// this constant down and comparing output byte for byte, the deepest chain
/// the corpus actually needs consumes 4 characters. Every value from 4 to 24
/// gives identical output on all four `score` runs -- both hint states, at
/// `--readings 1` and `--readings 3` -- and 3 is the first that diverges. The
/// same sweep against the full 218,449-entry index over 2,000 characters of
/// corpus text agrees exactly, so the margin is not an artefact of the gold
/// subset's smaller lexicon. 12 is three times what is used.
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
    /// Whether this span also partitions into two or more candidates that
    /// each carry both `COMMON` and `PARTICLE` -- `には` as `に` + `は`.
    ///
    /// A property of the span against the rest of the lattice, not of the
    /// entry, so it cannot be a `WordFlags` bit set at build time: the same
    /// headword is a function stack at one position and not at another. Set
    /// by a second pass in [`candidates`], read by `cost::cost`.
    pub function_stack: bool,
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

    // The same chains, indexed by where the *stem* ends rather than where
    // the span does. One walk from `start` arrives at stem ends ascending,
    // so that is the order the lookups need; each entry carries the span end
    // it implies and its position in its own `chains_by_end` bucket, which
    // the emission order below needs (Phase 2c design §3.2, §3.3).
    let mut chains_by_stem_end: Vec<Vec<(usize, usize, &ChainHit)>> = vec![Vec::new(); len + 1];
    for (i, bucket) in chains_by_end.iter().enumerate() {
        let end = i + 1;
        for (chain_index, hit) in bucket.iter().enumerate() {
            // `hit.consumed` can never exceed `end`: `Table::search`
            // walks leftward from `end` and stops before running past
            // its start, so this subtraction never wraps.
            chains_by_stem_end[end - hit.consumed].push((end, chain_index, hit));
        }
    }

    let mut class_cache: HashMap<u32, Vec<ClassId>> = HashMap::new();

    for start in 0..len {
        // Collected as the walk finds them -- stem-end ascending -- and
        // emitted longest span first below, which is the order the old
        // `(start + 1..=len).rev()` loop pushed in and `dedup_shallowest`
        // depends on.
        let mut found: Vec<(usize, usize, Candidate)> = Vec::new();
        let mut walk = index.walk();
        let mut stem_end = start;

        loop {
            for &(end, chain_index, hit) in &chains_by_stem_end[stem_end] {
                // The old loop ran `end` from `start + 1`, so a zero-length
                // span was unreachable; indexed by stem end, the identity
                // chain at `stem_end == start` implies `end == start`
                // (design §3.2).
                if end <= start {
                    continue;
                }
                let remove = match hit.stem_class {
                    Some(class) => CLASSES[class as usize].remove,
                    None => "",
                };
                // Ask the index whether anything is there before building a
                // key to ask with: the walk already holds the stem.
                let found_records = if remove.is_empty() {
                    walk.records()?
                } else {
                    walk.records_after(remove, kana::fold)?
                };
                let Some(records) = found_records else {
                    continue;
                };
                // Built only for a span that matched, because `exact`
                // compares against it.
                let key: String = chars[start..stem_end]
                    .iter()
                    .copied()
                    .chain(remove.chars())
                    .collect();
                for record in &records {
                    if !allows_chain(index, &mut class_cache, record, hit.stem_class)? {
                        continue;
                    }
                    found.push((
                        end,
                        chain_index,
                        Candidate {
                            start,
                            len: end - start,
                            entry_id: record.entry_id,
                            flags: WordFlags(record.flags),
                            steps: hit.steps.clone(),
                            exact: record.surface == key,
                            // The second pass below fills this in: it needs
                            // every position's candidates, which do not
                            // exist yet.
                            function_stack: false,
                            headword: record.surface.clone(),
                        },
                    ));
                }
            }

            // The walk's own dead end bounds the span: no key has this
            // prefix, so none has a longer one, and nothing further from
            // this start can match. That is the bound the old shape lacked.
            if stem_end == len || !walk.push(chars[stem_end], kana::fold) {
                break;
            }
            stem_end += 1;
        }

        // Longest span first, and within one span end the order that end's
        // own chain bucket had. `sort_by` is stable, so record order within
        // one chain hit survives too -- which is what makes this reproduce
        // the old push order term for term (design §3.3).
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        out[start] = dedup_shallowest(found.into_iter().map(|(_, _, c)| c).collect());
    }

    // Second pass for `function_stack`, which asks about a span against the
    // candidates *inside* it and so cannot be answered until every position
    // has been walked. Computed into a side vector first because the answer
    // for one candidate reads the same structure another candidate is stored
    // in.
    let stacks: Vec<Vec<bool>> = (0..len)
        .map(|start| {
            out[start]
                .iter()
                .map(|cand| is_function_stack(&out, start, start + cand.len))
                .collect()
        })
        .collect();
    for (start, flags) in stacks.into_iter().enumerate() {
        for (cand, stack) in out[start].iter_mut().zip(flags) {
            cand.function_stack = stack;
        }
    }

    Ok(out)
}

/// Whether `[start, end)` partitions into two or more candidates that each
/// carry both `COMMON` and `PARTICLE`.
///
/// A part spanning the whole width is skipped: the question is whether the
/// merge decomposes, and a span is not a decomposition of itself. Two or
/// more parts therefore falls out of plain reachability rather than needing
/// a step count.
///
/// Scoped to the span's own positions on purpose. A reachability table over
/// the whole text would be O(n^2) in a function whose superlinearity at
/// clipboard length was the entire subject of Phase 2c; this is bounded by
/// the span width, which the FST walk's own dead end already bounds.
/// `jparser-cli bench`'s scaling cell is the check.
fn is_function_stack(by_start: &[Vec<Candidate>], start: usize, end: usize) -> bool {
    let width = end - start;
    if width < 2 {
        return false;
    }
    let mut reachable = vec![false; width + 1];
    reachable[0] = true;
    for p in 0..width {
        if !reachable[p] {
            continue;
        }
        for cand in &by_start[start + p] {
            if cand.len >= width || p + cand.len > width {
                continue;
            }
            if cand.flags.contains(WordFlags::COMMON) && cand.flags.contains(WordFlags::PARTICLE) {
                reachable[p + cand.len] = true;
            }
        }
    }
    reachable[width]
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
