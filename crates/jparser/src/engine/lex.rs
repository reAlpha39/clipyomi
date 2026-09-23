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

use std::collections::hash_map::Entry;
use std::collections::HashMap;

use crate::engine::flags::WordFlags;
use crate::engine::kana;
use crate::engine::table::{
    classes_for_pos, is_godan_potential, is_kuru_kanji_headword, ChainHit, Table, CLASSES,
    KURU_KANJI_CLASS,
};
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
    /// The span opens with a run of digits and continues past it -- `1週間`,
    /// `1ヶ月`, `10分`. Number and counter are one phonological word in
    /// Japanese and JMdict records the euphonic reading (`いっしゅうかん`, not
    /// `いち` + `しゅうかん`), so a candidate of this shape carries a reading
    /// the split cannot reconstruct. `cost` discounts it; see `DIGIT_COUNTER`.
    ///
    /// All-digit spans are excluded by the "continues past it" half, which is
    /// what keeps `1969` out of this.
    pub digit_counter: bool,
    /// The span starts immediately after a run of digits, carries no
    /// `COUNTER`, and a `COUNTER` candidate starts at the same position.
    ///
    /// `4人目` is the shape: `人目` is a noun (`ひとめ`, "public gaze") and
    /// wins the span, so the reader is shown "four public-gaze" instead of
    /// `4人` + `目`. `人` is a counter and starts in the same place, which is
    /// the signal. `cost` penalises it; see `DIGIT_NONCOUNTER`.
    pub after_digit_noncounter: bool,
    /// False when the source characters differ from the stored surface
    /// before folding -- the FST matched only because kana was unified, not
    /// because the two strings actually agree.
    pub exact: bool,
    /// Every character of the span is katakana.
    ///
    /// Set here rather than recomputed in `cost`, which never receives
    /// `chars` -- the same reason `digit_counter` lives on this struct. Read
    /// by `cost::cost` to decide whether a tokenizer boundary *inside* the
    /// span is evidence of anything; see `KATAKANA_MIN_PIECE`.
    pub all_katakana: bool,
    /// Every character of the span is katakana **and** a longer katakana run
    /// encloses it — the span is a proper piece of that run, on at least one
    /// side.
    ///
    /// Geometry only: how short a piece has to be before it is charged is
    /// `cost::KATAKANA_MIN_PIECE`'s business, so the threshold lives there
    /// with the constant it shares. `ユリナス` tiles into two such pieces and
    /// reads "lily eggplant"; `ソロ|ホームラン` tiles into one and is right.
    pub inside_katakana_run: bool,
    /// Whether this span also partitions into two or more candidates that
    /// each carry both `COMMON` and `PARTICLE` -- `には` as `に` + `は`.
    ///
    /// A property of the span against the rest of the lattice, not of the
    /// entry, so it cannot be a `WordFlags` bit set at build time: the same
    /// headword is a function stack at one position and not at another. Set
    /// by a second pass in [`candidates`], read by `cost::cost`.
    pub function_stack: bool,
    /// Whether the span is a grammatical stack in the wider sense
    /// [`is_grammatical_stack`] defines. Read only by `cost::cost`, to
    /// withhold the length gain. Set by the same second pass.
    pub grammatical_stack: bool,
    /// JMdict says the entry is usually written in kana: some sense that is
    /// not `aux-v` is tagged `uk`, the test [`EntryFacts`] applies to the
    /// potential as well. Filled only for a deinflected match through a kana
    /// reading (`PRONOUNCE`), where [`allows_chain`] has already read the
    /// entry's facts; false everywhere else. Read by `cost::cost`, for the cut
    /// at an empty stem, and only under that flag.
    pub usually_kana: bool,
    pub headword: String,
}

/// The maximal katakana run containing each character, or `None` where the
/// character is not katakana.
///
/// Computed once per sentence so the per-candidate test in [`candidates`] is
/// O(1) rather than O(len): `jparser-cli bench`'s scaling cell is what a
/// per-candidate scan would show up in, and Phase 2c exists because an
/// earlier version of this module was superlinear in input length.
fn katakana_runs(chars: &[char]) -> Vec<Option<(usize, usize)>> {
    let mut runs = vec![None; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if !kana::is_katakana(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && kana::is_katakana(chars[i]) {
            i += 1;
        }
        for slot in &mut runs[start..i] {
            *slot = Some((start, i));
        }
    }
    runs
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

    let mut class_cache: HashMap<u32, EntryFacts> = HashMap::new();

    let runs = katakana_runs(chars);

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
                    if !allows_chain(index, &mut class_cache, record, hit)? {
                        continue;
                    }
                    // Only `cost`'s PRONOUNCE charge reads it, so a kanji
                    // headword never pays the look-up.
                    let usually_kana = hit.stem_class.is_some()
                        && WordFlags(record.flags).contains(WordFlags::PRONOUNCE)
                        && facts(index, &mut class_cache, record.entry_id)?.usually_kana;
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
                            all_katakana: chars[start..end].iter().all(|&c| kana::is_katakana(c)),
                            inside_katakana_run: match runs[start] {
                                // `end <= run_end` puts the whole span in the
                                // same run: runs are maximal and contiguous,
                                // so a span that starts in one and ends
                                // within its end never leaves it.
                                Some((run_start, run_end)) if end <= run_end => {
                                    run_start < start || run_end > end
                                }
                                _ => false,
                            },
                            // Filled from `chars` below, where the span is
                            // known; the walk here has only the record.
                            digit_counter: false,
                            after_digit_noncounter: false,
                            // The second pass below fills this in: it needs
                            // every position's candidates, which do not
                            // exist yet.
                            function_stack: false,
                            grammatical_stack: false,
                            usually_kana,
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
    // A read-only copy for the reading comparison below, which asks about the
    // *parts* of a span while the originals are being written through.
    let out_ro = out.clone();
    let stacks: Vec<Vec<(bool, bool)>> = (0..len)
        .map(|start| {
            out[start]
                .iter()
                .map(|cand| {
                    (
                        is_function_stack(&out, start, start + cand.len),
                        is_grammatical_stack(&out, start, start + cand.len),
                    )
                })
                .collect()
        })
        .collect();
    for (start, flags) in stacks.into_iter().enumerate() {
        for (cand, (stack, grammatical)) in out[start].iter_mut().zip(flags) {
            cand.function_stack = stack;
            cand.grammatical_stack = grammatical;
            cand.digit_counter = opens_with_a_number(chars, cand.start, cand.len)
                && reading_is_not_its_parts(index, &out_ro, chars, cand);
            cand.after_digit_noncounter = !cand.flags.contains(WordFlags::COUNTER)
                && loses_to_a_counted_number(chars, &out_ro, cand);
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
/// Whether the span's own reading differs from its parts read separately.
///
/// **This is the euphonic change, and it is the whole reason to prefer the
/// merge.** `一週間` reads `いっしゅうかん`, which `いち` + `しゅうかん` cannot
/// produce, and `四人` reads `よにん`, which `よん` + `にん` cannot — so the
/// split mispronounces both. `三回` reads `さんかい` and `二番` `にばん`, which
/// is exactly what `さん` + `かい` and `に` + `ばん` say, so merging buys
/// nothing there.
///
/// That distinction is load-bearing in both directions. `回目` and `番目` are
/// JMdict counters and `3 | 回目`, `2 | 番目` are the right cuts; `人目` is a
/// different word entirely (`ひとめ`, "public gaze"), so `4 | 人目` reads
/// "four public-gaze" and `4人 | 目` is the right cut. A sokuon test gets the
/// first two right and the third wrong, because `よにん` has no sokuon.
///
/// Reads the first reading of each part, which is the entry's own primary
/// form. Only digit-initial candidates reach here, so the index traffic is
/// bounded by a rare shape.
fn reading_is_not_its_parts(
    index: &Index,
    by_start: &[Vec<Candidate>],
    chars: &[char],
    cand: &Candidate,
) -> bool {
    let whole = match first_reading(index, cand.entry_id) {
        Some(r) => r,
        None => return false,
    };
    // Only the split at the end of the number, which is the analysis actually
    // competing with the merge. Any-split was too permissive: `1週間` was
    // ruled a non-change because `1週`(いっしゅう) + `間`(かん) reconstructs
    // `いっしゅうかん`, which is true and irrelevant -- nothing was proposing
    // that cut.
    let dlen = chars[cand.start..cand.start + cand.len]
        .iter()
        .take_while(|c| kana::is_digit(**c))
        .count();
    let digits = by_start
        .get(cand.start)
        .into_iter()
        .flatten()
        .filter(|c| c.len == dlen)
        .filter_map(|c| first_reading(index, c.entry_id).map(|r| (c.len, r)));
    for (dlen, head) in digits {
        let tail_start = cand.start + dlen;
        let tail_len = cand.len - dlen;
        let Some(tails) = by_start.get(tail_start) else {
            continue;
        };
        for t in tails.iter().filter(|c| c.len == tail_len) {
            if let Some(tail) = first_reading(index, t.entry_id) {
                if format!("{head}{tail}") == whole {
                    return false; // the split says the same thing
                }
            }
        }
    }
    true
}

/// The entry's primary reading, or `None` when it stores none.
fn first_reading(index: &Index, entry_id: u32) -> Option<String> {
    index
        .entry(entry_id)
        .ok()
        .flatten()
        .and_then(|e| e.readings.first().cloned())
}

/// Whether this span sits after a number, begins where a `COUNTER` begins,
/// and **the number-plus-counter merge is itself an available candidate**.
///
/// The last clause is what keeps the rule honest. `4人目` wants `4人` + `目`
/// because `人目` is `ひとめ`, "public gaze", and `4人` is in the dictionary —
/// so there is a counted number to lose to. `1969年度` wants `1969` + `年度`:
/// `年` is a counter and `年度` is not, which looks identical, but `1969年` is
/// no entry, so nothing is on offer and the compound is left alone. Same for
/// `点差` against `2点`. Without this clause the rule splits both into `年` +
/// `度` and `点` + `差`, which cost 3 gold spans when measured.
fn loses_to_a_counted_number(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    cand: &Candidate,
) -> bool {
    if cand.start == 0
        || !kana::is_digit(chars[cand.start - 1])
        || kana::is_digit(chars[cand.start])
    {
        return false;
    }
    let digit_start = chars[..cand.start]
        .iter()
        .rposition(|c| !kana::is_digit(*c))
        .map_or(0, |i| i + 1);
    let Some(here) = by_start.get(cand.start) else {
        return false;
    };
    let counters = here.iter().filter(|c| c.flags.contains(WordFlags::COUNTER));
    counters
        .flat_map(|k| {
            by_start
                .get(digit_start)
                .into_iter()
                .flatten()
                .map(move |n| (k, n))
        })
        .any(|(k, n)| n.start + n.len == cand.start + k.len)
}

/// Whether `[start, start + len)` opens with a digit run and continues past
/// it. See [`Candidate::digit_counter`].
fn opens_with_a_number(chars: &[char], start: usize, len: usize) -> bool {
    let span = &chars[start..start + len];
    let digits = span.iter().take_while(|c| kana::is_digit(**c)).count();
    digits > 0 && digits < span.len()
}

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

/// Whether `[start, start + len)` is a *grammatical* stack rather than a word:
/// tileable entirely by COMMON pieces of at most two characters, where a
/// one-character piece must itself be a PARTICLE and at least one piece is.
///
/// Wider than [`is_function_stack`], and used for one thing only -- deciding
/// whether `cost`'s length gain applies. The two are separate predicates on
/// purpose: `FUNCTION_STACK`'s own penalty keeps the narrower test it was
/// measured with, so nothing moves in a hint state the gain does not reach.
///
/// The two-character cap is what keeps a content word out. `ことになっている`
/// tiles as `こと|に|なっ|て|いる`, every piece short; `物思いにふける` needs
/// `ふける` at three and so is a word. The particle requirement on
/// one-character pieces is what keeps `唐揚げ` out: single COMMON kana and
/// kanji are everywhere, and without it `物思いにふける` tiles as
/// `物|思|い|に|ふ|け|る` and every word is a stack.
fn is_grammatical_stack(by_start: &[Vec<Candidate>], start: usize, end: usize) -> bool {
    const MAX_PIECE: usize = 2;
    let width = end - start;
    if width < 2 {
        return false;
    }
    // `[p][saw_particle]`: a tiling of `[start, start + p)` into qualifying
    // pieces, `saw_particle` recording whether any of them was a PARTICLE.
    let mut reachable = vec![[false; 2]; width + 1];
    reachable[0][0] = true;
    for p in 0..width {
        for seen in 0..2 {
            if !reachable[p][seen] {
                continue;
            }
            for cand in &by_start[start + p] {
                if cand.len >= width || p + cand.len > width || cand.len > MAX_PIECE {
                    continue;
                }
                let particle = cand.flags.contains(WordFlags::PARTICLE);
                if !cand.flags.contains(WordFlags::COMMON) || (cand.len < 2 && !particle) {
                    continue;
                }
                reachable[p + cand.len][usize::from(seen == 1 || particle)] = true;
            }
        }
    }
    reachable[width][1]
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
    // Shallower chain first, then an exact surface. The second half matters
    // because one entry can carry two kana headwords -- `イチャイチャ` and
    // `いちゃいちゃ` are the same `ent_seq` -- and they collapse to the same
    // key here. Keeping whichever came first meant a hiragana input could be
    // represented by the katakana record and marked `exact: false`, paying
    // `INEXACT` for a headword spelled character-for-character like itself.
    let rank = |c: &Candidate| (c.steps.len(), !c.exact);
    for cand in bucket {
        let key = (cand.len, cand.entry_id);
        match best.get(&key) {
            Some(existing) if rank(existing) <= rank(&cand) => {
                // Already holding an equal-or-better representative of this
                // (len, entry_id) pair -- `cand` adds nothing.
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
    cache: &mut HashMap<u32, EntryFacts>,
    record: &StoredRecord,
    hit: &ChainHit,
) -> Result<bool, IndexError> {
    let Some(class) = hit.stem_class else {
        return Ok(true);
    };
    // The copula conjugates in no class its POS names (`cop` maps to none, see
    // `table::POS_WITHOUT_A_CLASS`), so this is settled by the lemma instead.
    if CLASSES[class as usize].name == COPULA_DESU_CLASS {
        return Ok(record.surface == "です"
            && is_desu_past(&hit.steps)
            && index
                .entry(record.entry_id)?
                .is_some_and(|e| e.senses.iter().any(|s| s.pos.iter().any(|p| p == "cop"))));
    }
    // Before the entry's own POS is consulted, because this one is settled by
    // the headword alone: カ変・来ル removes る, so a kana `vk` headword
    // satisfies it by accident and deinflects over 取ってく rather than
    // 取ってこ. See `table::is_kuru_kanji_headword`.
    if CLASSES[class as usize].name == KURU_KANJI_CLASS && !is_kuru_kanji_headword(&record.surface)
    {
        return Ok(false);
    }
    let facts = facts(index, cache, record.entry_id)?;
    if is_godan_potential(&hit.steps) && !facts.takes_a_potential(WordFlags(record.flags)) {
        return Ok(false);
    }
    Ok(facts.classes.contains(&class))
}

/// `entry_id`'s [`EntryFacts`], read through `index` once and memoized.
fn facts<'a>(
    index: &Index,
    cache: &'a mut HashMap<u32, EntryFacts>,
    entry_id: u32,
) -> Result<&'a EntryFacts, IndexError> {
    Ok(match cache.entry(entry_id) {
        Entry::Occupied(known) => known.into_mut(),
        Entry::Vacant(slot) => slot.insert(match index.entry(entry_id)? {
            Some(entry) => EntryFacts::of(&entry),
            // A record whose entry is missing has no classes to check against;
            // treated as disqualifying rather than trusted, since a well-formed
            // index never produces this.
            None => EntryFacts::default(),
        }),
    })
}

/// What [`allows_chain`] and `Candidate::usually_kana` need from an entry,
/// memoized per `entry_id`.
#[derive(Default)]
struct EntryFacts {
    classes: Vec<ClassId>,
    /// Some sense that is not `aux-v` is tagged `uk`, "usually written using
    /// kana alone" -- the plain JMdict tag, not `WordFlags::UK`, which drops
    /// every entry with an `aux-v` sense and so would drop 貰う, 行く and 成る.
    /// An `aux-v` sense does not count: 腐る's only `uk` sense is the 〜くさる
    /// suffix, whose potential nobody writes, and counting it read hints-off
    /// `良くされる` as 良 + 腐る Potential (about 40 sentences). What it costs
    /// is 置く, whose `uk` senses are all 〜ておく: `放っておけない` goes back
    /// to the split it had, 4 sentences hints on.
    usually_kana: bool,
    /// Some sense is `exp`: a multi-word expression.
    expression: bool,
}

impl EntryFacts {
    fn of(entry: &EntryData) -> EntryFacts {
        EntryFacts {
            classes: entry_classes(entry),
            usually_kana: entry
                .senses
                .iter()
                .any(|s| s.misc.iter().any(|m| m == "uk") && !s.pos.iter().any(|p| p == "aux-v")),
            expression: entry
                .senses
                .iter()
                .any(|s| s.pos.iter().any(|p| p == "exp")),
        }
    }

    /// Whether the godan potential may reach this entry through a record
    /// carrying `flags` (`table::is_godan_potential`). Two refusals, both
    /// measured over twelve visual-novel corpora (151,225 sentences) against
    /// the same build without them, and both of them the shape `LEN_GAIN` and
    /// `cost`'s inflection-internal exemption reward: a long span that crosses
    /// IPADIC's cut between the e-row kana and the auxiliary for nothing.
    ///
    /// **Not an expression.** `口を開けて` is 口 + を + 開けて, "open one's
    /// mouth", and 口を開く's potential only happens to spell it; so do
    /// `腰を浮かせて` (31 spans hints on), `口を開け-` (22), `目を開け-` (10)
    /// and `頭を悩ませる` (4), each a 一段 verb that an expression's godan
    /// swallows whole under a "Potential" label. What it costs is the handful
    /// that really are potentials -- `目を離せない`, `好きになれる`,
    /// `一緒になれて`, `役に立てる` -- which go back to the split they had.
    ///
    /// **Not the kana reading of a word JMdict says is written in kanji.** A
    /// potential nobody spelled, of a verb matched only through its reading,
    /// is two inferences deep, and that is where the false chains were:
    /// `わけない` read as 湧く Potential Negative where it is 訳 + ない (122
    /// hints on, where the split had been right), `していただけませんか` cut
    /// `いた|だけません` and read as 抱く (19), `イ|かせて` as 貸す (91); and
    /// hints off `ありません` as 増す (317), `眉をひそめる` as 潜む,
    /// `あるけれども` as 歩く, `抱きすくめた` as 汲む. The kana verbs whose
    /// potential is common in running text -- 貰う, 行く, 成る, 頂く, 遣る --
    /// are the ones JMdict tags `uk`, and keep it. The plain tag is read, not
    /// `WordFlags::UK`, because that flag also drops every entry with an
    /// `aux-v` sense, and those five are exactly such entries.
    ///
    /// What this refusal gives back is not only false chains. Without it, the
    /// potential of 止む crossed `やめ|て` for free and so kept the 一段 verb
    /// やめる whole -- about 400 spans of kana 一段 verbs (`やめて`, `ふるえて`,
    /// `つづけて`) that `PRONOUNCE_CROSS` splits at the stem, repaired by a
    /// candidate nobody displays. They go back to the split they had at the
    /// tip; the defect is `cost`'s, and is recorded in the phase's handoff
    /// rather than patched here through a side door.
    fn takes_a_potential(&self, flags: WordFlags) -> bool {
        !self.expression && (self.usually_kana || !flags.contains(WordFlags::PRONOUNCE))
    }
}

/// The copula's own class. A noun has no conjugation, so `雨でした` has no
/// chain to reach through the `です` auxiliary row (which follows a 基本形);
/// until 2026-09-23 it read 雨 | でし | た, glossed "pupil; disciple" and "want
/// to do" -- 231 spans over twelve visual-novel corpora, hints on.
const COPULA_DESU_CLASS: &str = "特殊・デス";

/// Whether `steps` is the copula's 連用形 with the past `た` on it and nothing
/// else -- でした, でしたら, でしたろ. Every other auxiliary on でし is refused,
/// because `で` + `し` is far more often the particle and する: IPADIC itself
/// tokenizes `みんなでしたい` as the copula, and the want-to form of です is
/// not Japanese. `でして` is refused for the same reason on the corpus, where
/// `口でして` and `奥底でしている` are the particle in half the IPADIC-copula
/// readings.
fn is_desu_past(steps: &[ConjStep]) -> bool {
    matches!(steps, [own, past]
        if CLASSES[own.class as usize].forms[own.form as usize].name == "連用形"
            && past.aux == Some("た"))
}

/// Whether `cand` is the copula conjugating as its own headword -- the one
/// chain [`allows_chain`] admits through [`COPULA_DESU_CLASS`]. `cost` gates
/// it on the tokenizer: see `cost::is_allowed`.
pub fn is_copula_desu_chain(cand: &Candidate) -> bool {
    cand.steps
        .first()
        .is_some_and(|s| CLASSES[s.class as usize].name == COPULA_DESU_CLASS)
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

#[cfg(test)]
mod tests {
    /// The guard that keeps `1969年度` together, as the pair it separates.
    /// Both have a counter starting after the digits and a non-counter
    /// two-character candidate there; only one has a number-plus-counter
    /// entry to lose to.
    #[test]
    fn a_noncounter_after_a_number_only_loses_to_a_merge_that_exists() {
        use crate::engine::flags::WordFlags;
        let counter = {
            let mut f = WordFlags::PRIMARY;
            f.insert(WordFlags::COUNTER);
            f
        };
        let at = |start: usize, len: usize, flags: WordFlags| super::Candidate {
            start,
            len,
            entry_id: 1,
            flags,
            steps: Vec::new(),
            exact: true,
            all_katakana: false,
            inside_katakana_run: false,
            digit_counter: false,
            after_digit_noncounter: false,
            function_stack: false,
            grammatical_stack: false,
            usually_kana: false,
            headword: String::new(),
        };

        // 4人目: 人 is a counter at 1, and 4人 spans [0,2) -- fire.
        let c: Vec<char> = "4人目".chars().collect();
        let mut by: Vec<Vec<super::Candidate>> = vec![Vec::new(); 3];
        by[0].push(at(0, 2, WordFlags::PRIMARY)); // 4人
        by[1].push(at(1, 1, counter)); // 人
        let hitome = at(1, 2, WordFlags::PRIMARY); // 人目
        assert!(super::loses_to_a_counted_number(&c, &by, &hitome));

        // 1969年度: 年 is a counter at 4, but nothing spans [0,5) -- hold.
        let c2: Vec<char> = "1969年度".chars().collect();
        let mut by2: Vec<Vec<super::Candidate>> = vec![Vec::new(); 6];
        by2[4].push(at(4, 1, counter)); // 年
        let nendo = at(4, 2, WordFlags::PRIMARY); // 年度
        assert!(!super::loses_to_a_counted_number(&c2, &by2, &nendo));
    }

    /// The shape test, without the index half. `1969` is the case that must
    /// fail it: an all-digit span has nothing after the number, so a year can
    /// never be discounted into a merge with what follows.
    #[test]
    fn a_number_alone_is_not_a_number_plus_counter() {
        let c: Vec<char> = "1969年1週間".chars().collect();
        assert!(!super::opens_with_a_number(&c, 0, 4), "1969 is all digits");
        assert!(
            super::opens_with_a_number(&c, 0, 5),
            "1969年 continues past it"
        );
        assert!(super::opens_with_a_number(&c, 5, 3), "1週間");
        assert!(
            !super::opens_with_a_number(&c, 4, 1),
            "年 opens with no digit"
        );
    }

    use super::*;
    use crate::engine::ConjStep;

    fn rec(entry_id: u32, exact: bool, steps: usize, headword: &str) -> Candidate {
        Candidate {
            start: 0,
            len: 6,
            entry_id,
            flags: WordFlags::PRIMARY,
            steps: vec![
                ConjStep {
                    class: 0,
                    form: 0,
                    range: 0..0,
                    aux: None,
                };
                steps
            ],
            exact,
            all_katakana: false,
            inside_katakana_run: false,
            function_stack: false,
            grammatical_stack: false,
            usually_kana: false,
            digit_counter: false,
            after_digit_noncounter: false,
            headword: headword.to_string(),
        }
    }

    /// The copula conjugates as its own headword only as でし + た; the rest
    /// of what IPADIC would let follow でし is far more often `で` + する.
    #[test]
    fn only_the_copulas_past_reaches_desu() {
        let class =
            |name: &str| CLASSES.iter().position(|c| c.name == name).expect("class") as ClassId;
        let form = |c: ClassId, name: &str| {
            CLASSES[c as usize]
                .forms
                .iter()
                .position(|f| f.name == name)
                .expect("form") as crate::engine::FormId
        };
        let desu = class("特殊・デス");
        let step = |class: ClassId, form_name: &str, aux: Option<&'static str>| ConjStep {
            class,
            form: form(class, form_name),
            range: 0..0,
            aux,
        };
        let deshi = step(desu, "連用形", None);
        let ta = step(class("特殊・タ"), "基本形", Some("た"));
        let te = step(class("不変化型"), "基本形", Some("て"));
        let tai = step(class("特殊・タイ"), "基本形", Some("たい"));
        assert!(is_desu_past(&[deshi.clone(), ta.clone()]), "でした");
        assert!(
            !is_desu_past(&[deshi.clone(), te]),
            "口でして is the particle"
        );
        assert!(
            !is_desu_past(&[deshi.clone(), tai]),
            "みんなでしたい is the particle"
        );
        assert!(!is_desu_past(std::slice::from_ref(&deshi)), "a bare でし");
        assert!(
            !is_desu_past(&[step(desu, "未然形", None), ta]),
            "でしょ + た"
        );
    }

    /// Which entries the godan potential may reach through a kana reading:
    /// `uk` on a lexical sense (貰う), never on an `aux-v` sense alone (腐る's
    /// 〜くさる), never an expression, and a kanji spelling always.
    #[test]
    fn a_potential_reaches_a_kana_reading_only_where_jmdict_writes_it_in_kana() {
        let sense = |pos: &[&str], uk: bool| crate::index::SenseData {
            pos: pos.iter().map(|p| p.to_string()).collect(),
            glosses: Vec::new(),
            xrefs: Vec::new(),
            misc: if uk {
                vec!["uk".to_string()]
            } else {
                Vec::new()
            },
            info: Vec::new(),
        };
        let facts = |senses: Vec<crate::index::SenseData>| {
            EntryFacts::of(&EntryData {
                id: 1,
                readings: Vec::new(),
                senses,
            })
        };
        let kana = WordFlags::PRONOUNCE;
        let kanji = WordFlags::PRIMARY;
        let morau = facts(vec![sense(&["v5u"], true), sense(&["aux-v", "v5u"], true)]);
        assert!(morau.takes_a_potential(kana), "もらえる is 貰う Potential");
        let kusaru = facts(vec![sense(&["v5r"], false), sense(&["aux-v", "v5r"], true)]);
        assert!(
            !kusaru.takes_a_potential(kana),
            "くされる is not 腐る Potential"
        );
        assert!(kusaru.takes_a_potential(kanji), "腐れる in kanji still is");
        let waku = facts(vec![sense(&["v5k"], false)]);
        assert!(
            !waku.takes_a_potential(kana),
            "わけない is not 湧く Potential"
        );
        let kuchi = facts(vec![sense(&["exp", "v5k"], false)]);
        assert!(
            !kuchi.takes_a_potential(kanji),
            "口を開けて is not 口を開く Potential"
        );
    }

    /// One entry, two kana headwords, one span: `イチャイチャ` and
    /// `いちゃいちゃ` share an `ent_seq`, so they collapse to the same
    /// `(len, entry_id)` key here. Keeping whichever arrived first let a
    /// hiragana input be represented by the katakana record and marked
    /// inexact -- `cost` then charged `INEXACT` for a headword spelled
    /// character-for-character like the input, 8 on a span that covers 6.
    #[test]
    fn dedup_keeps_the_exact_record_when_the_chains_tie() {
        let kept = dedup_shallowest(vec![
            rec(2026650, false, 0, "イチャイチャ"),
            rec(2026650, true, 0, "いちゃいちゃ"),
        ]);
        assert_eq!(kept.len(), 1, "one entry, one candidate");
        assert!(kept[0].exact);
        assert_eq!(kept[0].headword, "いちゃいちゃ");

        // Order of arrival must not matter.
        let other_way = dedup_shallowest(vec![
            rec(2026650, true, 0, "いちゃいちゃ"),
            rec(2026650, false, 0, "イチャイチャ"),
        ]);
        assert!(other_way[0].exact);
    }

    /// The chain depth still outranks exactness: a shallower inexact match is
    /// the better representative of the entry, because the depth is what
    /// `assemble::order` and `cost` both read first.
    #[test]
    fn a_shallower_chain_still_wins_over_an_exact_deeper_one() {
        let kept = dedup_shallowest(vec![
            rec(1, false, 0, "カタカナ"),
            rec(1, true, 2, "かたかな"),
        ]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].steps.len(), 0, "the shallow one survives");
    }

    #[test]
    fn a_katakana_run_is_the_maximal_span_around_each_of_its_characters() {
        let chars: Vec<char> = "をユリナスが".chars().collect();
        let runs = super::katakana_runs(&chars);
        assert_eq!(runs[0], None, "を is not katakana");
        assert_eq!(runs[1], Some((1, 5)), "ユ opens the run");
        assert_eq!(runs[4], Some((1, 5)), "ス closes it");
        assert_eq!(runs[5], None, "が is not katakana");
    }

    #[test]
    fn the_prolonged_sound_mark_stays_inside_the_run() {
        let chars: Vec<char> = "コーヒー".chars().collect();
        let runs = super::katakana_runs(&chars);
        assert_eq!(runs[1], Some((0, 4)), "ー does not break the run");
    }
}
