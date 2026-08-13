// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Dictionary matching at one position.
//!
//! Ports ta-old's `FindMatches` (`ta-old/exe/util/Dictionary.cpp:807`). ta-old
//! searched a per-dictionary array of headwords sorted by a kana-insensitive
//! comparator, once per prefix length; Phase 1A replaced that with a single FST
//! walk (`Index::prefixes_of`), so this module only interprets what the walk
//! returns.
//!
//! Two comparisons are in play and they are not the same one:
//!
//! * the **loose** comparison (ta-old's `wcsnijcmp`) folds kana type, width and
//!   ASCII case. The FST walk already applies it to the key.
//! * the **strict** comparison (ta-old's `wcsnicmp`) folds ASCII case only. A
//!   hit that passes the loose test and fails the strict one is *inexact*: the
//!   user typed katakana where the dictionary spells hiragana. Inexactness is
//!   scored, not rejected, and depends on what was typed, so it cannot be
//!   precomputed into the index.

use crate::conjugation::{ConjugationTable, Form, TenseId, VerbTypeId};
use crate::index::load::Index;
use crate::record::WordFlags;
use crate::ParseError;

// The conjugation recursion and its nine rule tests. A child module, not a
// sibling, so it keeps access to `strict_eq`; split out purely for the 800-line
// cap — see the plan's File Structure note.
mod verb;

/// One conjugation layer of a match, ta-old's `ConjInfo`
/// (`ta-old/exe/util/Dictionary.h:45`). The index in `Match::chain` is ta-old's
/// `depth`: index 0 is the layer applied directly to the dictionary stem — the
/// first suffix consumed, leftmost in the text — and increasing indices move
/// outward toward the end of the word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConjLink {
    /// 0-based index into `ConjugationTable::types()`. ta-old stored this
    /// 1-based with 0 meaning "not a verb"; here "not a verb" is an empty
    /// `chain`, and Phase 6's differential run adds 1.
    pub(crate) verb_type: VerbTypeId,
    pub(crate) tense: TenseId,
    pub(crate) form: Form,
    /// Index into `types()[verb_type].conjugations`, needed to recover the
    /// suffix for the kuruHack twin search.
    pub(crate) conj: usize,
}

/// One dictionary hit at one position, ta-old's `Match`
/// (`ta-old/exe/util/Dictionary.h:72`).
///
/// There is deliberately no `dict_index` and no `first_jstring`: the port has
/// one dictionary, and `entry_id` carries both identities ta-old split across
/// those two fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    /// Char offset into the parsed text.
    pub(crate) start: usize,
    /// Total matched length in **chars**: the dictionary key plus every
    /// conjugation suffix consumed along `chain`.
    pub(crate) len: usize,
    /// Length in **chars** of the dictionary key alone — the
    /// `PrefixHit::key_chars` this match came from. Equal to `len` for a
    /// non-verb match.
    pub(crate) src_len: usize,
    /// `StoredRecord::surface` verbatim: the stem for a verb record, the
    /// headword for a plain one. Never normalized.
    pub(crate) surface: String,
    pub(crate) flags: WordFlags,
    pub(crate) entry_id: u32,
    /// The source text and the dictionary spelling disagree in kana type,
    /// width, or **non-ASCII** case — `strict_eq` folds ASCII case, so an
    /// ASCII case disagreement alone is still exact. ta-old's `inexactMatch`,
    /// narrowed from its tri-state `int`: the sign only ever reflected
    /// alphabetical order, not quality.
    pub(crate) inexact: bool,
    /// Empty for a non-verb match (ta-old's `conj[0].verbType == 0`). Never
    /// longer than `MAX_CONJ_DEPTH`.
    pub(crate) chain: Vec<ConjLink>,
}

/// Field-by-field equality over every field except `inexact`. Replaces ta-old's
/// `memcmp(a, b, sizeof(Match) - sizeof(int))` (`Dictionary.cpp:885`, `:1046`),
/// which relied on `inexactMatch` being the struct's last field and compared
/// padding bytes as a side effect.
pub(crate) fn same_except_inexact(a: &Match, b: &Match) -> bool {
    a.start == b.start
        && a.len == b.len
        && a.src_len == b.src_len
        && a.surface == b.surface
        && a.flags == b.flags
        && a.entry_id == b.entry_id
        && a.chain == b.chain
}

/// ta-old's `wcsnicmp` (`Shared/Shrink.h:124`): ASCII-case-insensitive, kana
/// type and width **sensitive**. Used only to decide inexactness — the loose
/// comparison is already guaranteed by the FST walk.
fn strict_eq(a: &[char], b: &str) -> bool {
    let mut rhs = b.chars();
    for &lhs in a {
        let Some(other) = rhs.next() else {
            return false;
        };
        // `eq_ignore_ascii_case`, not a `to_ascii_lowercase` pair: clippy's
        // `manual_ignore_case_cmp` is a hard error under `-D warnings`, and
        // the two are semantically identical (ASCII folding only, kana and
        // width untouched).
        if !lhs.eq_ignore_ascii_case(&other) {
            return false;
        }
    }
    rhs.next().is_none()
}

/// ta-old's `wcsnijcmp` (`Shared/Shrink.h:197`): kana type, width and ASCII case
/// all folded through `kana::unify`. This is the comparison the FST key already
/// applies to the dictionary key; conjugation suffixes are not in the FST, so
/// they need it applied here.
///
/// `pub(crate)` rather than private because Task 7's `tails_match` is a caller
/// outside this module — the kuruHack tail comparison is the same one.
pub(crate) fn unified_eq(a: &[char], b: &str) -> bool {
    let mut rhs = b.chars();
    for &lhs in a {
        let Some(other) = rhs.next() else {
            return false;
        };
        if crate::kana::unify(lhs) != crate::kana::unify(other) {
            return false;
        }
    }
    rhs.next().is_none()
}

/// Commit one candidate, applying ta-old's post-match filters in its own order
/// (`Dictionary.cpp:882-894`).
fn commit(out: &mut Vec<Match>, candidate: Match) {
    // Zero-length drop. An empty stem meeting an empty trimmed suffix produces
    // a match covering no text. ta-old never collected one — its cheapest
    // possible match delta is 10 - 2 - 3 - 2 = +3 > 0, so the DP could never
    // choose it — and allowing one would be a self-loop in the DP.
    if candidate.len == 0 {
        return;
    }
    // Same-entry duplicate collapse. Exact wins: the committed copy loses its
    // inexact flag, and the newcomer is dropped either way. Distinct entry ids
    // never compare equal, so two homographs are never merged.
    if let Some(existing) = out.iter_mut().find(|m| same_except_inexact(m, &candidate)) {
        if !candidate.inexact {
            existing.inexact = false;
        }
        return;
    }
    // Names-inexact suppression: an inexact hit from a names source is
    // discarded outright, not merely ranked lower. Dormant in v1 — nothing sets
    // IS_NAME — but implemented so JMnedict needs no matcher change.
    if candidate.inexact && candidate.flags.contains(WordFlags::IS_NAME) {
        return;
    }
    out.push(candidate);
}

/// Every dictionary match starting at char offset `i`.
///
/// Emission order is load-bearing — the DP's match relaxation keeps the *last*
/// candidate on a tie and the final rank sort is stable — so it is fixed as:
/// ascending `key_chars`, then records in stored order, then (for a verb
/// record) the recursion's own order.
pub(crate) fn matches_at(
    index: &Index,
    table: &ConjugationTable,
    text: &[char],
    i: usize,
) -> Result<Vec<Match>, ParseError> {
    // ponytail: O(n^2) tail allocation; pass a char→byte offset table and slice
    // the original &str if a 10k-char input ever measures slow.
    let tail: String = text[i..].iter().collect();
    let mut out: Vec<Match> = Vec::new();

    for hit in index.prefixes_of(&tail)? {
        let k = hit.key_chars;
        // `key_chars` counts chars of the query, so this slice always exists;
        // taking it fallibly costs one line and removes a panic path.
        let Some(source) = text.get(i..i + k) else {
            continue;
        };
        for record in hit.records {
            let inexact = !strict_eq(source, &record.surface);
            match record.verb_type {
                None => commit(
                    &mut out,
                    Match {
                        start: i,
                        len: k,
                        src_len: k,
                        surface: record.surface,
                        flags: WordFlags(record.flags),
                        entry_id: record.entry_id,
                        inexact,
                        chain: Vec::new(),
                    },
                ),
                Some(vtype) => {
                    for mut m in verb::recurse(table, text, i, k, vtype, &[], inexact) {
                        m.src_len = k;
                        m.surface = record.surface.clone();
                        m.flags = WordFlags(record.flags);
                        m.entry_id = record.entry_id;
                        commit(&mut out, m);
                    }
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::TENSE_STEM;
    use crate::index::build::build_from_reader;
    use crate::index::load::Index;
    use crate::record::WordFlags;
    use crate::stem::StemOptions;

    const FIXTURE: &str = include_str!("../tests/fixtures/jmdict_matcher.xml");

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("embedded asset must load")
    }

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// Build the matcher fixture into its own directory and open it. Mirrors
    /// `tests/index_roundtrip.rs`: no `tempfile` dependency, and one directory
    /// per test so a parallel test can never write into a live mmap.
    fn index(name: &str) -> Index {
        let dir = std::env::temp_dir().join(format!("jparser-matcher-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        build_from_reader(
            std::io::Cursor::new(FIXTURE),
            &table(),
            &StemOptions::default(),
            &dir,
        )
        .expect("fixture must build");
        Index::open(&dir).expect("fixture index must open")
    }

    /// A `Match` with every field at a neutral value, for the `commit` and
    /// `same_except_inexact` unit tests.
    fn plain(entry_id: u32, inexact: bool) -> Match {
        Match {
            start: 0,
            len: 1,
            src_len: 1,
            surface: "猫".to_string(),
            flags: WordFlags::PRIMARY,
            entry_id,
            inexact,
            chain: Vec::new(),
        }
    }

    #[test]
    fn a_non_verb_record_yields_one_match_spanning_its_key() {
        // 猫 is a one-character noun; だ is not part of any key, so exactly one
        // record hits at this position.
        let idx = index("nonverb");
        let text = chars("猫だ");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].start, 0);
        assert_eq!(got[0].len, 1);
        assert_eq!(got[0].src_len, 1, "len == src_len is a non-verb invariant");
        assert_eq!(got[0].surface, "猫");
        assert_eq!(got[0].entry_id, 2000010);
        assert!(!got[0].inexact);
        assert!(got[0].chain.is_empty(), "a non-verb match has no chain");
    }

    #[test]
    fn kana_type_disagreement_marks_a_match_inexact() {
        // Both ねこ (entry 2000010) and ネコ (entry 2000020) normalize to the
        // key ネコ, so a hiragana query returns both at key_chars 2: the
        // hiragana spelling exactly, the katakana spelling inexactly.
        let idx = index("inexact");
        let text = chars("ねこ");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let two: Vec<&Match> = got.iter().filter(|m| m.len == 2).collect();
        assert_eq!(two.len(), 2, "got {got:#?}");
        let exact = two
            .iter()
            .find(|m| m.surface == "ねこ")
            .expect("hiragana record");
        let fuzzy = two
            .iter()
            .find(|m| m.surface == "ネコ")
            .expect("katakana record");
        assert!(!exact.inexact);
        assert!(
            fuzzy.inexact,
            "katakana spelling of a hiragana query is inexact"
        );
        assert_eq!(exact.entry_id, 2000010);
        assert_eq!(fuzzy.entry_id, 2000020);
    }

    #[test]
    fn strict_eq_folds_ascii_case_but_not_kana_type() {
        // ta-old's wcsnicmp: NORM_IGNORECASE only. The kana-insensitive half of
        // the comparison is the FST walk's job, not this function's.
        assert!(strict_eq(&chars("abc"), "ABC"));
        assert!(strict_eq(&chars("ねこ"), "ねこ"));
        assert!(!strict_eq(&chars("ねこ"), "ネコ"));
        assert!(!strict_eq(&chars("ねこ"), "ねこだ"));
        assert!(!strict_eq(&chars("ねこだ"), "ねこ"));
    }

    #[test]
    fn one_position_yields_one_match_per_distinct_key_length() {
        // 猫 and 猫舌 are both keys and both prefix the text.
        let idx = index("lengths");
        let text = chars("猫舌");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let mut lengths: Vec<usize> = got.iter().map(|m| m.len).collect();
        lengths.sort_unstable();
        assert_eq!(lengths, vec![1, 2], "got {got:#?}");
    }

    #[test]
    fn distinct_entries_sharing_a_surface_both_survive() {
        // Entries 2000040 and 2000050 are both spelled 二. ta-old's dedupe keyed
        // on entry identity, never on spelling, so both must appear.
        let idx = index("homograph");
        let text = chars("二");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let mut ids: Vec<u32> = got.iter().map(|m| m.entry_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![2000040, 2000050], "got {got:#?}");
    }

    #[test]
    fn word_flags_are_carried_through_from_the_record() {
        let idx = index("flags");
        let text = chars("は");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        let particle = got
            .iter()
            .find(|m| m.surface == "は")
            .expect("は must match");
        assert!(particle.flags.contains(WordFlags::PARTICLE));
    }

    #[test]
    fn start_is_stamped_with_the_queried_position() {
        // ta-old set start to 0 inside FindMatches and stamped the real offset
        // in FindAllMatches; matches_at stamps it directly.
        let idx = index("start");
        let text = chars("猫だ猫");
        let got = matches_at(&idx, &table(), &text, 2).expect("matcher must not fail");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].start, 2);
        assert_eq!(got[0].len, 1);
    }

    #[test]
    fn text_with_no_indexed_prefix_yields_nothing() {
        let idx = index("miss");
        let text = chars("zzz");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert!(got.is_empty(), "got {got:#?}");
    }

    #[test]
    fn the_empty_key_hit_never_produces_a_zero_length_match() {
        // する/vs-i has an empty stem, so prefixes_of returns a key_chars == 0
        // hit on every call against this index. A zero-length match would be a
        // self-loop in the DP.
        let idx = index("emptykey");
        let text = chars("する");
        let got = matches_at(&idx, &table(), &text, 0).expect("matcher must not fail");
        assert!(got.iter().all(|m| m.len > 0), "got {got:#?}");
        assert!(
            got.iter().any(|m| m.len == 2 && m.chain.is_empty()),
            "the plain する headword must still match: {got:#?}"
        );
    }

    #[test]
    fn commit_drops_a_zero_length_candidate() {
        let mut out = Vec::new();
        let mut zero = plain(1, false);
        zero.len = 0;
        commit(&mut out, zero);
        assert!(out.is_empty());
    }

    #[test]
    fn commit_collapses_a_duplicate_and_lets_exact_win() {
        // ta-old Dictionary.cpp:882-893: the committed copy is forced exact and
        // the newcomer is dropped.
        let mut out = Vec::new();
        commit(&mut out, plain(1, true));
        commit(&mut out, plain(1, false));
        assert_eq!(out.len(), 1);
        assert!(
            !out[0].inexact,
            "an exact duplicate clears the committed flag"
        );
    }

    #[test]
    fn commit_suppresses_an_inexact_name_match_entirely() {
        // ta-old Dictionary.cpp:894. Dormant in v1 — nothing sets IS_NAME.
        let mut named = plain(1, true);
        named.flags.insert(WordFlags::IS_NAME);
        let mut out = Vec::new();
        commit(&mut out, named.clone());
        assert!(
            out.is_empty(),
            "an inexact name hit is dropped, not ranked down"
        );

        named.inexact = false;
        commit(&mut out, named);
        assert_eq!(out.len(), 1, "an exact name hit is kept");
    }

    #[test]
    fn same_except_inexact_ignores_exactly_one_field() {
        assert!(same_except_inexact(&plain(1, true), &plain(1, false)));
        assert!(!same_except_inexact(&plain(1, false), &plain(2, false)));
        let mut longer = plain(1, false);
        longer.len = 2;
        assert!(!same_except_inexact(&plain(1, false), &longer));
    }

    #[test]
    fn matches_at_stamps_record_fields_onto_recursion_output() {
        // End to end against the real embedded table: 食べる is v1, its stem 食べ
        // is indexed, and 食べた reaches v1's Stem た, which links to v-ta-stem's
        // terminal Past (an empty suffix). No other path off 食べ reaches three
        // characters.
        let t = table();
        let idx = index("verb");
        let text = chars("食べた");
        let got = matches_at(&idx, &t, &text, 0).expect("matcher must not fail");
        let three: Vec<&Match> = got.iter().filter(|m| m.len == 3).collect();
        assert_eq!(three.len(), 1, "got {got:#?}");
        let m = three[0];

        assert_eq!(m.start, 0);
        assert_eq!(
            m.src_len, 2,
            "src_len is the key alone, len is key + suffixes"
        );
        assert_eq!(m.surface, "食べ");
        assert_eq!(m.entry_id, 2000070);
        assert!(m.flags.contains(WordFlags::PRIMARY));
        assert!(!m.inexact);

        assert_eq!(m.chain.len(), 2);
        assert_eq!(m.chain[0].verb_type, t.types_named("v1")[0]);
        assert_eq!(m.chain[0].tense, TENSE_STEM);
        assert_eq!(
            t.types()[m.chain[0].verb_type].conjugations[m.chain[0].conj].suffix,
            "た",
            "conj must index back to the conjugation that was matched"
        );
        assert_eq!(t.types()[m.chain[1].verb_type].name, "v-ta-stem");
        assert_eq!(t.tense_name(m.chain[1].tense), Some("Past"));
    }
}
