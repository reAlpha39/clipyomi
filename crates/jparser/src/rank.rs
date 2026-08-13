// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `sort_matches`, ta-old's `SortMatches` (`Dictionary.cpp:1025-1063`): dedupe
//! the candidates on one chosen span, then rank them so the definition list
//! shows the best reading first.
//!
//! Split out of `segment.rs` for file size only — see the plan's File Structure
//! note. Nothing here knows about the DP or about the text; it is a pure
//! ordering pass over candidates that already share a `(start, len)`.

use crate::conjugation::TENSE_NON_PAST;
use crate::matcher::{same_except_inexact, Match};
use crate::record::WordFlags;

/// Flag bits `CompareMatches` ranks on, compared descending as a raw integer —
/// bit-value priority, not popcount. ta-old `Dictionary.cpp:1013`. The
/// `JAP_WORD_TOP` variant of the mask is behind `#ifdef SETSUMI_CHANGES`,
/// which ta-old never defines, and is correctly excluded.
const RANK_FLAG_MASK: u16 = WordFlags::COUNTER.0
    | WordFlags::PARTICLE.0
    | WordFlags::COMMON.0
    | WordFlags::COMMON_LINE.0
    | WordFlags::PRIMARY.0;

/// Pass A's grouping key. A non-verb keys as type 0 so it sorts before every
/// verb of the same entry, mirroring ta-old's 1-based `verbType`.
fn group_key(m: &Match) -> (u32, usize, usize, u8) {
    match m.chain.first() {
        Some(l) => (m.entry_id, l.verb_type + 1, l.tense, l.form.0),
        None => (m.entry_id, 0, 0, 0),
    }
}

/// True when `matches[i]` is a verb's plain informal non-past sitting on the
/// same slot as a non-verb hit for the same entry, with no third candidate
/// following. ta-old `Dictionary.cpp:1046-1056`. The lookahead reads the
/// **uncompacted** tail, exactly as ta-old's does.
fn verb_plain_collapses(matches: &[Match], i: usize, d: usize) -> bool {
    let cur = &matches[i];
    let kept = &matches[d - 1];
    let Some(link) = cur.chain.first() else {
        return false;
    };
    cur.entry_id == kept.entry_id
        && kept.chain.is_empty()
        && link.form.0 == 0
        && link.tense == TENSE_NON_PAST
        && matches
            .get(i + 1)
            .map_or(true, |nx| nx.entry_id != cur.entry_id)
}

/// Group, dedupe with inexact reconciliation, then rank. Called per span, so
/// every element already shares `(start, len)` and identity is `entry_id`
/// alone — it carries both of ta-old's identity fields.
///
/// The compaction is deliberately the original's adjacency-only one-behind
/// scan. It is **not** a global group-by; the port is bug-for-bug compatible
/// here on purpose.
pub(crate) fn sort_matches(matches: &mut Vec<Match>) {
    if matches.is_empty() {
        return;
    }

    // Pass A — group sort, ta-old's CompareIdenticalMatches. Its weighted
    // integer key is lexicographic (verbType, verbTense, verbForm) ascending,
    // so the tuple below is a faithful — and non-overflowing — port.
    matches.sort_by_key(group_key);

    // Pass B — one-behind compaction, write cursor `d`.
    let mut d = 1;
    for i in 1..matches.len() {
        // 1. Inexact reconciliation (`Dictionary.cpp:1031-1042`): a run of one
        //    entry disagreeing on `inexact` is forced to exact, which is what
        //    lets step 2 see the run as duplicates at all.
        let mut j = i;
        let mut k = d - 1;
        while matches[j].entry_id == matches[k].entry_id && matches[j].inexact != matches[k].inexact
        {
            matches[j].inexact = false;
            matches[k].inexact = false;
            j = k;
            if k == 0 {
                break;
            }
            k -= 1;
        }

        // 2. Exact-duplicate drop.
        if same_except_inexact(&matches[i], &matches[d - 1]) {
            continue;
        }

        // 3. Verb-plain vs non-verb collapse.
        if verb_plain_collapses(matches, i, d) {
            continue;
        }

        // 4. Keep. `swap` rather than ta-old's `matches[d++] = matches[i]`:
        //    behaviourally identical here, because the step-3 lookahead only
        //    reads indices > i and the step-1 walk only reads indices < d,
        //    neither of which a swap disturbs — and it avoids a clone.
        matches.swap(d, i);
        d += 1;
    }
    matches.truncate(d);

    // Pass C — final rank, ta-old's CompareMatches minus `start` (constant
    // within a span). Stable, so ties keep matcher emission order.
    matches.sort_by(|a, b| {
        a.inexact
            .cmp(&b.inexact)
            .then(
                a.flags
                    .contains(WordFlags::IS_NAME)
                    .cmp(&b.flags.contains(WordFlags::IS_NAME)),
            )
            .then((b.flags.0 & RANK_FLAG_MASK).cmp(&(a.flags.0 & RANK_FLAG_MASK)))
            // Ascending, unlike ta-old's descending `dictIndex`/`firstJString`
            // (Dictionary.cpp:1019-1021): lower `entry_id` (JMdict `ent_seq`)
            // is usually the more established entry — see plan Self-Review §5.
            .then(a.entry_id.cmp(&b.entry_id))
            .then(
                a.chain
                    .first()
                    .map_or(0, |l| l.form.0)
                    .cmp(&b.chain.first().map_or(0, |l| l.form.0)),
            )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{Form, TenseId};
    use crate::matcher::ConjLink;

    /// Any tense that is not Non-past. The static table's index 4 is "Past".
    const TENSE_PAST: TenseId = 4;

    /// A non-verb candidate on the fixed span (0, 2).
    fn plain(entry_id: u32, flags: WordFlags) -> Match {
        Match {
            start: 0,
            len: 2,
            src_len: 2,
            surface: "ある".to_string(),
            flags,
            entry_id,
            inexact: false,
            chain: Vec::new(),
        }
    }

    /// The same span, reached as a one-link conjugation of `entry_id`.
    fn verb(entry_id: u32, tense: TenseId, form: u8) -> Match {
        Match {
            chain: vec![ConjLink {
                verb_type: 0,
                tense,
                form: Form(form),
                conj: 0,
            }],
            ..plain(entry_id, WordFlags::default())
        }
    }

    fn ids(ms: &[Match]) -> Vec<u32> {
        ms.iter().map(|m| m.entry_id).collect()
    }

    #[test]
    fn reconciles_an_inexact_pair_into_one_exact_match() {
        let exact = plain(1, WordFlags::default());
        let mut fuzzy = exact.clone();
        fuzzy.inexact = true;
        let mut ms = vec![fuzzy, exact];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 1);
        assert!(!ms[0].inexact);
    }

    #[test]
    fn keeps_distinct_entries_with_identical_surfaces() {
        let mut ms = vec![
            plain(4, WordFlags::default()),
            plain(5, WordFlags::default()),
        ];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![4, 5]);
    }

    #[test]
    fn drops_a_plain_non_past_verb_beside_its_non_verb_twin() {
        let mut ms = vec![plain(1, WordFlags::default()), verb(1, TENSE_NON_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 1);
        assert!(ms[0].chain.is_empty());
    }

    #[test]
    fn keeps_the_verb_when_a_third_candidate_follows() {
        // The lookahead reads the uncompacted tail: another candidate for the
        // same entry blocks the collapse.
        let mut ms = vec![
            plain(1, WordFlags::default()),
            verb(1, TENSE_NON_PAST, 0),
            verb(1, TENSE_PAST, 0),
        ];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 3);
    }

    #[test]
    fn does_not_collapse_a_conjugated_verb() {
        let mut ms = vec![plain(1, WordFlags::default()), verb(1, TENSE_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms.len(), 2);
    }

    #[test]
    fn ranks_exact_before_inexact() {
        let mut fuzzy = plain(1, WordFlags::default());
        fuzzy.inexact = true;
        let mut ms = vec![fuzzy, plain(2, WordFlags::default())];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn ranks_by_flag_bit_value_not_popcount() {
        // COUNTER alone (0x20) outranks PRIMARY|COMMON|COMMON_LINE (0x0D).
        assert_eq!(RANK_FLAG_MASK, 0x003D);
        let mut many = WordFlags::PRIMARY;
        many.insert(WordFlags::COMMON);
        many.insert(WordFlags::COMMON_LINE);
        let mut ms = vec![plain(1, many), plain(2, WordFlags::COUNTER)];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn ranks_non_name_before_name() {
        let mut ms = vec![plain(1, WordFlags::IS_NAME), plain(2, WordFlags::default())];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![2, 1]);
    }

    #[test]
    fn breaks_remaining_ties_by_entry_id_ascending() {
        let mut ms = vec![
            plain(9, WordFlags::default()),
            plain(3, WordFlags::default()),
        ];
        sort_matches(&mut ms);
        assert_eq!(ids(&ms), vec![3, 9]);
    }

    #[test]
    fn orders_conjugation_forms_ascending() {
        // form 0 (informal affirmative) before form 3 (formal negative).
        let mut ms = vec![verb(1, TENSE_PAST, 3), verb(1, TENSE_PAST, 0)];
        sort_matches(&mut ms);
        assert_eq!(ms[0].chain[0].form, Form(0));
        assert_eq!(ms[1].chain[0].form, Form(3));
    }

    #[test]
    fn handles_zero_and_one_element_lists() {
        let mut empty: Vec<Match> = Vec::new();
        sort_matches(&mut empty);
        assert!(empty.is_empty());
        let mut one = vec![plain(1, WordFlags::default())];
        sort_matches(&mut one);
        assert_eq!(one.len(), 1);
    }
}
