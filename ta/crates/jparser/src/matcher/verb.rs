// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The verb-conjugation recursion, ta-old's `FindVerbMatches`
//! (`ta-old/exe/util/Dictionary.cpp:738-805`).
//!
//! A child module of `matcher` for one reason: the 800-line cap. Keeping it a
//! child rather than a sibling preserves access to `matcher`'s private
//! `strict_eq`, so no comparison becomes crate-visible just to be split.

use crate::conjugation::{
    ConjugationTable, VerbTypeId, MAX_CONJ_DEPTH, TENSE_POTENTIAL, TENSE_REMOVE, TENSE_STEM,
};
use crate::record::WordFlags;

use super::{strict_eq, unified_eq, ConjLink, Match};

/// Match conjugation suffixes onward from a dictionary stem.
///
/// `slen` is the number of chars consumed since `start`: the dictionary key
/// plus every suffix matched along this path. `depth` is not a parameter —
/// `chain.len()` is it. ta-old filled `conj[depth]` while unwinding; the chain
/// is built top-down here, which produces the same array and removes the way
/// for the two to disagree.
///
/// The returned matches are incomplete on purpose: `src_len`, `surface`,
/// `flags` and `entry_id` are placeholders that `matches_at` stamps from the
/// `StoredRecord`, exactly as ta-old's `FindMatches` filled `srcLen`, `jString`
/// and `dictIndex` into the slots `FindVerbMatches` had just appended
/// (`Dictionary.cpp:900-927`).
///
/// Conjugation suffixes arrive already trimmed — the conjugation table stripped
/// each link target's remove-suffix at load time — so there is no trimming
/// arithmetic here, exactly as in ta-old.
///
// ponytail: the Stem-skip arm advances neither `depth` nor the cap, so a
// zero-width Stem/form-0 cycle in a conjugation asset would recurse until the
// stack overflows. The shipped asset has six zero-width stem-skip edges
// (v5uru -> v-i-stem/v-a-stem, v1 -> v-i-stem/v-a-stem, both vs -> vs-i) and is
// acyclic, and `Index::open`'s fingerprint check binds an index to its asset —
// so there is no hazard today. Add a visited-(vtype, slen) set here if
// `from_json` ever ingests an asset this crate did not ship.
pub(super) fn recurse(
    table: &ConjugationTable,
    text: &[char],
    start: usize,
    slen: usize,
    vtype: VerbTypeId,
    chain: &[ConjLink],
    inexact: bool,
) -> Vec<Match> {
    let depth = chain.len();
    let mut out: Vec<Match> = Vec::new();
    // A verb_type id from an index built against a different conjugation asset.
    // `Index::open`'s fingerprint check makes this unreachable; returning
    // nothing still beats indexing out of bounds.
    let Some(ty) = table.types().get(vtype) else {
        return out;
    };

    for (cj, c) in ty.conjugations.iter().enumerate() {
        // Rule 1: the global Remove sentinel, never `ty.remove_tense`. A Remove
        // conjugation is bookkeeping that tells stem generation what to strip;
        // it is never a real match.
        if c.tense == TENSE_REMOVE {
            continue;
        }
        let n = c.suffix.chars().count();
        let from = start + slen;
        // Nothing may read past the end of the text.
        let Some(slice) = text.get(from..from + n) else {
            continue;
        };
        if !unified_eq(slice, &c.suffix) {
            continue;
        }
        // Monotonic: an inexact suffix anywhere poisons the whole chain.
        let inexact2 = inexact || !strict_eq(slice, &c.suffix);
        let link = ConjLink {
            verb_type: vtype,
            tense: c.tense,
            form: c.form,
            conj: cj,
        };

        match c.next_verb_type {
            // Terminal: this is the only place a match is created, and the only
            // place `len` is ever written.
            None => {
                let mut full = chain.to_vec();
                full.push(link);
                out.push(Match {
                    start,
                    len: slen + n,
                    src_len: 0,
                    surface: String::new(),
                    flags: WordFlags::default(),
                    entry_id: 0,
                    inexact: inexact2,
                    chain: full,
                });
            }
            // Rule 2: an informal-affirmative Stem above depth 0 consumes no
            // depth and records no link — but rule 3, `slen` still advances, so
            // its characters are counted in `len`. `form.0 == 0` is exact:
            // informal *and* affirmative, not merely "not formal".
            Some(next) if depth > 0 && c.tense == TENSE_STEM && c.form.0 == 0 => {
                out.extend(recurse(table, text, start, slen + n, next, chain, inexact2));
            }
            // Rule 4: chaining is allowed only while a further layer fits.
            Some(next) if depth < MAX_CONJ_DEPTH - 1 => {
                let mut extended = chain.to_vec();
                extended.push(link);
                let mut kids = recurse(table, text, start, slen + n, next, &extended, inexact2);
                // Rule 5: drop a child whose own layer repeats this frame's
                // Potential (`Dictionary.cpp:780-792`). Any other repeated tense
                // is left alone.
                //
                // Documented fidelity divergence: ta-old removed the child with
                // a swap-remove (`matches[m] = matches[numMatches-1]`,
                // `:784-790`), which permutes the surviving siblings; `retain`
                // preserves order. Emission order feeds the DP's `>=` tie-break,
                // so this is a real if small deviation. The contract's
                // pseudocode mandates `retain`; Phase 6's differential run is
                // where a difference would surface.
                if c.tense == TENSE_POTENTIAL {
                    kids.retain(|m| {
                        m.chain.get(depth + 1).map(|l| l.tense) != Some(TENSE_POTENTIAL)
                    });
                }
                out.append(&mut kids);
            }
            // Rule 4, the other half: at the cap the branch is dropped whole. No
            // recursion, no match, and deliberately no truncated stand-in.
            Some(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{Form, TenseId, TENSE_NON_PAST};

    /// Highest tense id any fixture in this file can reach. The four fixed ids
    /// are consts; every other tense is a position in the table's name list, so
    /// tests resolve it by name rather than hard-coding a number.
    const TENSE_LOOKUP_LIMIT: TenseId = 64;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn tense(t: &ConjugationTable, name: &str) -> TenseId {
        (0..TENSE_LOOKUP_LIMIT)
            .find(|&i| t.tense_name(i) == Some(name))
            .unwrap_or_else(|| panic!("tense {name:?} must exist in the table"))
    }

    /// Run the recursion from the start of `text` against the first type named
    /// `name`, as if a zero-length stem had just been matched.
    fn run(t: &ConjugationTable, name: &str, text: &str) -> Vec<Match> {
        let vtype = t.types_named(name)[0];
        recurse(t, &chars(text), 0, 0, vtype, &[], false)
    }

    /// One type declares a "Remove" tense whose suffix matches the text; the
    /// other has no Remove entry at all, so its `remove_tense` defaults to
    /// Non-past.
    const REMOVE_JSON: &str = r#"[
      {"Name":"has-remove","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"く","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"き","Tense":"Past"}]},
      {"Name":"no-remove","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"く","Tense":"Non-past"}]}
    ]"#;

    /// Chaining fixtures. Every link target declares an empty "Remove" suffix,
    /// so load-time trimming is a no-op and each conjugation's suffix in the
    /// loaded table is exactly what is written here.
    const CHAIN_JSON: &str = r#"[
      {"Name":"two-step","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"leaf"}]},
      {"Name":"root","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"mid"}]},
      {"Name":"root-neg","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"mid-neg"}]},
      {"Name":"mid","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"い","Tense":"Stem","Next Type":"leaf"}]},
      {"Name":"mid-neg","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":true,"Suffix":"い","Tense":"Stem","Next Type":"leaf"}]},
      {"Name":"leaf","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"う","Tense":"Past"}]}
    ]"#;

    /// Six types in a row. d4 offers both a sixth link and a terminal
    /// alternative, so the test can tell "branch dropped" from "match
    /// truncated".
    const DEEP_JSON: &str = r#"[
      {"Name":"d0","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"あ","Tense":"Past","Next Type":"d1"}]},
      {"Name":"d1","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"い","Tense":"Past","Next Type":"d2"}]},
      {"Name":"d2","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"う","Tense":"Past","Next Type":"d3"}]},
      {"Name":"d3","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"え","Tense":"Past","Next Type":"d4"}]},
      {"Name":"d4","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"お","Tense":"Past","Next Type":"d5"},
        {"Formal":false,"Negative":false,"Suffix":"お","Tense":"Te-form"}]},
      {"Name":"d5","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"か","Tense":"Past"}]}
    ]"#;

    /// pot-inner offers the same suffix under two tenses, so one sibling is
    /// dropped by the guard and one survives.
    const POTENTIAL_JSON: &str = r#"[
      {"Name":"pot-outer","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"れ","Tense":"Potential","Next Type":"pot-inner"}]},
      {"Name":"past-outer","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"れ","Tense":"Past","Next Type":"pot-inner"}]},
      {"Name":"pot-inner","Part of Speech":"Verb","Tenses":[
        {"Formal":false,"Negative":false,"Suffix":"","Tense":"Remove"},
        {"Formal":false,"Negative":false,"Suffix":"ら","Tense":"Potential"},
        {"Formal":false,"Negative":false,"Suffix":"ら","Tense":"Past"}]}
    ]"#;

    #[test]
    fn remove_tense_conjugations_are_never_matched() {
        // Rule 1. The skip tests the global TENSE_REMOVE sentinel, never
        // VerbType::remove_tense — they differ for every type without an
        // explicit Remove entry, where remove_tense defaults to Non-past.
        let t = ConjugationTable::from_json(REMOVE_JSON).expect("fixture must load");

        // has-remove's Remove suffix く matches the text exactly and must still
        // be skipped; its only other conjugation is き, which does not match.
        assert!(run(&t, "has-remove", "く").is_empty());

        // no-remove's remove_tense IS Non-past, and its Non-past conjugation
        // must still be matchable.
        let no_remove = t.types_named("no-remove")[0];
        assert_eq!(t.types()[no_remove].remove_tense, TENSE_NON_PAST);
        let got = run(&t, "no-remove", "く");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 1);
        assert_eq!(
            got[0].chain,
            vec![ConjLink {
                verb_type: no_remove,
                tense: TENSE_NON_PAST,
                form: Form(0),
                conj: 0,
            }]
        );
    }

    #[test]
    fn chaining_through_next_verb_type_consumes_both_suffixes() {
        // two-step あ links to leaf う, and the match only exists if both
        // suffixes are consumed.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let two_step = t.types_named("two-step")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "two-step", "あう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 2, "len counts stem + every suffix");
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink {
                    verb_type: two_step,
                    tense: past,
                    form: Form(0),
                    conj: 0
                },
                ConjLink {
                    verb_type: leaf,
                    tense: past,
                    form: Form(0),
                    conj: 1
                },
            ]
        );

        // The chain cannot complete when the text runs out mid-way, and reading
        // past the end must not panic.
        assert!(run(&t, "two-step", "あ").is_empty());
    }

    #[test]
    fn an_informal_stem_above_depth_zero_leaves_no_chain_link() {
        // Rule 2 and rule 3. mid's Stem い is reached at depth 1, so it consumes
        // no depth and adds no link — but slen still advances past it, so three
        // suffix-consuming steps produce a two-link chain of length 3.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let root = t.types_named("root")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "root", "あいう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 3, "the skipped Stem's char is still counted");
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink {
                    verb_type: root,
                    tense: past,
                    form: Form(0),
                    conj: 0
                },
                ConjLink {
                    verb_type: leaf,
                    tense: past,
                    form: Form(0),
                    conj: 1
                },
            ],
            "mid must appear in no chain slot"
        );
    }

    #[test]
    fn the_same_stem_conjugation_at_depth_zero_takes_a_chain_slot() {
        // Rule 2's depth guard is load-bearing: the identical conjugation, now
        // the first link off the stem, behaves like any other.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let mid = t.types_named("mid")[0];
        let leaf = t.types_named("leaf")[0];

        let got = run(&t, "mid", "いう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 2);
        assert_eq!(
            got[0].chain,
            vec![
                ConjLink {
                    verb_type: mid,
                    tense: TENSE_STEM,
                    form: Form(0),
                    conj: 1
                },
                ConjLink {
                    verb_type: leaf,
                    tense: tense(&t, "Past"),
                    form: Form(0),
                    conj: 1
                },
            ]
        );
    }

    #[test]
    fn a_negative_stem_above_depth_zero_is_not_skipped() {
        // Rule 2 is `form.0 == 0` exactly — informal-affirmative — not "the
        // formal bit is clear". mid-neg's Stem carries Negative, so it keeps its
        // slot and the chain grows to three links.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let mid_neg = t.types_named("mid-neg")[0];

        let got = run(&t, "root-neg", "あいう");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].chain.len(), 3, "got {:#?}", got[0].chain);
        assert_eq!(
            got[0].chain[1],
            ConjLink {
                verb_type: mid_neg,
                tense: TENSE_STEM,
                form: Form(Form::NEGATIVE),
                conj: 1,
            }
        );
    }

    #[test]
    fn a_branch_needing_a_sixth_layer_is_dropped_whole() {
        // Rule 4. Chaining is allowed only while depth < MAX_CONJ_DEPTH - 1, so
        // d4's link to d5 never fires: no six-char match exists, and no
        // truncated five-layer stand-in is recorded for it either. d4's terminal
        // Te-form alternative shows five layers are still fine.
        let t = ConjugationTable::from_json(DEEP_JSON).expect("fixture must load");
        let got = run(&t, "d0", "あいうえおか");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].len, 5, "the sixth suffix is never consumed");
        assert_eq!(got[0].chain.len(), MAX_CONJ_DEPTH);
        assert_eq!(
            t.tense_name(got[0].chain[4].tense),
            Some("Te-form"),
            "the surviving fifth layer is the terminal alternative, not a \
             truncated version of the dropped branch"
        );
    }

    #[test]
    fn a_potential_chained_into_a_potential_is_dropped() {
        // Rule 5. pot-inner offers ら under both Potential and Past; only the
        // Potential child is dropped.
        let t = ConjugationTable::from_json(POTENTIAL_JSON).expect("fixture must load");
        let got = run(&t, "pot-outer", "れら");
        assert_eq!(got.len(), 1, "got {got:#?}");
        assert_eq!(got[0].chain[0].tense, TENSE_POTENTIAL);
        assert_eq!(
            got[0].chain[1].tense,
            tense(&t, "Past"),
            "the Potential+Potential sibling must be gone"
        );
    }

    #[test]
    fn a_repeated_non_potential_tense_survives() {
        // Rule 5 fires only when both adjacent tenses are Potential. Past into
        // Past is left alone, and so is Past into Potential.
        let t = ConjugationTable::from_json(POTENTIAL_JSON).expect("fixture must load");
        let past = tense(&t, "Past");
        let got = run(&t, "past-outer", "れら");
        assert_eq!(got.len(), 2, "got {got:#?}");
        let mut inner: Vec<TenseId> = got.iter().map(|m| m.chain[1].tense).collect();
        inner.sort_unstable();
        let mut want = vec![TENSE_POTENTIAL, past];
        want.sort_unstable();
        assert_eq!(inner, want);
        assert!(got.iter().all(|m| m.chain[0].tense == past));
    }

    #[test]
    fn an_inexact_suffix_poisons_the_whole_chain() {
        // inexact is monotonic: the loose comparison lets katakana text match a
        // hiragana suffix, the strict one records that it did.
        let t = ConjugationTable::from_json(CHAIN_JSON).expect("fixture must load");
        let exact = run(&t, "two-step", "あう");
        assert!(!exact[0].inexact);
        let fuzzy = run(&t, "two-step", "アう");
        assert_eq!(fuzzy.len(), 1, "got {fuzzy:#?}");
        assert!(fuzzy[0].inexact, "one inexact suffix marks the whole match");
    }
}
