// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Verb stem generation, ported from the stem-emitting block of ta-old's
//! `CreateDict` (`ta-old/exe/util/Dictionary.cpp:413-470`).
//!
//! The index holds headwords **and** generated stems as separate entries. The
//! matcher walks to a stem and then matches conjugation suffixes onward from
//! it, so without these no verb ever conjugates.
//!
//! ## The v5 fallback
//!
//! ta-old accepts a candidate conjugation type when its name equals the word's
//! POS code *or* when both names start with `v5` and have the same length. Its
//! comment: *"Fix a couple dozen incorrectly annotated verbs. Doesn't get them
//! all, but gets a lot."* A verb mis-tagged `v5r` therefore also gets stems for
//! `v5k`, `v5m`, `v5t`, and so on.
//!
//! This is preserved because JMdict is the source EDICT2 was generated from, so
//! changing formats does not fix the mis-annotations, and the failure without it
//! is silent: a verb simply never conjugates and nothing says why. It is gated
//! behind `StemOptions::v5_misannotation_fallback` and instrumented via
//! `StemStats` so its real cost and benefit can be measured.

use crate::conjugation::ConjugationTable;
use crate::kana::strip_suffix_unified;
use crate::record::HeadwordRecord;

/// Name prefix that qualifies a type for the mis-annotation fallback.
const V5_PREFIX: &str = "v5";

#[derive(Debug, Clone, Copy)]
pub struct StemOptions {
    /// Cross-generate stems for same-length `v5*` types. See module docs.
    pub v5_misannotation_fallback: bool,
}

impl Default for StemOptions {
    fn default() -> Self {
        Self {
            v5_misannotation_fallback: true,
        }
    }
}

/// Counts of stems generated per fallback-vs-exact origin. Attribution can be
/// wrong for a headword carrying both a correct and a same-length
/// mis-annotated `v5*` tag: `record::headwords` attaches every id a POS code
/// names in source order, `generate_stems` iterates `record.verb_types` in
/// that same order, and the per-(surface, type) dedup silently drops the
/// second candidate before its counter ever fires — so whichever tag comes
/// first in the entry's POS list wins the count, not necessarily the
/// genuinely exact one. These counters are a rough signal, not a precise
/// measurement; the authoritative measurement of the v5 fallback's real
/// value is the `--no-v5-fallback` A/B comparison (build once with it on,
/// once with it off, and diff the resulting indexes), not these counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StemStats {
    /// Stems generated from a type whose name matched the POS code exactly.
    pub exact_stems: usize,
    /// Stems generated only because of the v5 fallback.
    pub v5_fallback_stems: usize,
    /// Stems whose surface is the empty string.
    pub empty_stems: usize,
}

/// True when `candidate` may stand in for `annotated` under the v5 fallback.
fn v5_fallback_applies(annotated: &str, candidate: &str) -> bool {
    annotated.starts_with(V5_PREFIX)
        && candidate.starts_with(V5_PREFIX)
        && annotated.len() == candidate.len()
}

/// Generate every stem record for one headword.
pub fn generate_stems(
    record: &HeadwordRecord,
    table: &ConjugationTable,
    opts: &StemOptions,
    stats: &mut StemStats,
) -> Vec<HeadwordRecord> {
    let mut out: Vec<HeadwordRecord> = Vec::new();

    for &annotated_id in &record.verb_types {
        let annotated_name = table.types()[annotated_id].name.clone();

        // ta-old iterates every type rather than only the annotated one, which
        // is what lets the v5 fallback fire and also what makes both entries of
        // the duplicate-named types (vk, vs, v5r-i, v5uru) reachable.
        for (candidate_id, candidate) in table.types().iter().enumerate() {
            let exact = candidate.name == annotated_name;
            let fallback = !exact
                && opts.v5_misannotation_fallback
                && v5_fallback_applies(&annotated_name, &candidate.name);
            if !exact && !fallback {
                continue;
            }

            // Find the remove-tense/form-0 conjugation whose suffix ends this
            // surface. That suffix is the dictionary-form ending.
            let stem = candidate
                .conjugations
                .iter()
                .filter(|c| c.tense == candidate.remove_tense && c.form.0 == 0)
                .find_map(|c| strip_suffix_unified(&record.surface, &c.suffix));

            let Some(surface) = stem else { continue };
            let verb_types = vec![candidate_id];

            // Deduplicate on (surface, type). ta-old's equivalent guard is dead
            // code; see the module docs and the spec's deviation table.
            if out
                .iter()
                .any(|r| r.surface == surface && r.verb_types == verb_types)
            {
                continue;
            }

            if surface.is_empty() {
                stats.empty_stems += 1;
            }
            if fallback {
                stats.v5_fallback_stems += 1;
            } else {
                stats.exact_stems += 1;
            }

            out.push(HeadwordRecord {
                surface,
                flags: record.flags,
                verb_types,
                entry_id: record.entry_id,
            });
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conjugation::{ConjugationTable, VerbTypeId};
    use crate::record::WordFlags;

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().unwrap()
    }

    fn record(surface: &str, type_name: &str, t: &ConjugationTable) -> HeadwordRecord {
        HeadwordRecord {
            surface: surface.to_string(),
            flags: WordFlags::PRIMARY,
            verb_types: t.types_named(type_name),
            entry_id: 1,
        }
    }

    fn surfaces(recs: &[HeadwordRecord]) -> Vec<&str> {
        recs.iter().map(|r| r.surface.as_str()).collect()
    }

    #[test]
    fn strips_the_remove_suffix_to_form_a_stem() {
        // 食べる is v1 (ichidan); removing る leaves 食べ.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(surfaces(&out).contains(&"食べ"), "got {:?}", surfaces(&out));
    }

    #[test]
    fn tags_each_stem_with_exactly_the_type_that_produced_it() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let stem = out.iter().find(|r| r.surface == "食べ").unwrap();
        assert_eq!(stem.verb_types.len(), 1, "a stem carries exactly one type");
        assert!(t.types_named("v1").contains(&stem.verb_types[0]));
    }

    #[test]
    fn v5_fallback_adds_stems_for_same_length_v5_siblings() {
        // 言う is deliberately mis-tagged v5s here (its real row is v5u,
        // suffix う; v5s's suffix is す). With the fallback off, the
        // annotated type v5s fails to strip す from the surface's tail, so
        // no stem is produced at all. With it on, the same-length sibling
        // v5u is also tried, its suffix う matches, and the stem is
        // produced. That is precisely the EDICT mis-annotation the fallback
        // exists to absorb.
        let t = table();
        let mut on = StemStats::default();
        let with = generate_stems(
            &record("言う", "v5s", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: true,
            },
            &mut on,
        );
        let mut off = StemStats::default();
        let without = generate_stems(
            &record("言う", "v5s", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: false,
            },
            &mut off,
        );
        assert!(
            with.len() > without.len(),
            "fallback must add stems: {} vs {}",
            with.len(),
            without.len()
        );
        assert!(on.v5_fallback_stems > 0);
        assert_eq!(off.v5_fallback_stems, 0);
    }

    #[test]
    fn v5_fallback_requires_equal_length_type_names() {
        // v5u (3 chars) must not cross-generate with v5u-s (5 chars).
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: true,
            },
            &mut stats,
        );
        let long_named: Vec<VerbTypeId> = t
            .types()
            .iter()
            .enumerate()
            .filter(|(_, ty)| ty.name.starts_with("v5") && ty.name.len() != 3)
            .map(|(i, _)| i)
            .collect();
        for r in &out {
            for vt in &r.verb_types {
                assert!(!long_named.contains(vt), "crossed a name-length boundary");
            }
        }
    }

    #[test]
    fn v5_fallback_does_not_apply_to_non_v5_types() {
        let t = table();
        let mut on = StemStats::default();
        let with = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: true,
            },
            &mut on,
        );
        let mut off = StemStats::default();
        let without = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: false,
            },
            &mut off,
        );
        assert_eq!(with.len(), without.len());
        assert_eq!(on.v5_fallback_stems, 0);
    }

    #[test]
    fn retains_empty_stems() {
        // ta-old: "len 0 is for verbs which have 0 characters after removing
        // the suffix." A verb whose whole surface is the remove-suffix yields
        // an empty stem, and it must survive.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("る", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(out.iter().any(|r| r.surface.is_empty()));
        assert_eq!(stats.empty_stems, 1);
    }

    #[test]
    fn deduplicates_identical_stem_and_type_pairs() {
        // ta-old's dedupe is dead code; ours works. No two output records may
        // share both a surface and a type.
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let mut seen: Vec<(String, Vec<VerbTypeId>)> = Vec::new();
        for r in &out {
            let key = (r.surface.clone(), r.verb_types.clone());
            assert!(!seen.contains(&key), "duplicate stem {key:?}");
            seen.push(key);
        }
    }

    #[test]
    fn produces_nothing_for_words_with_no_verb_types() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &HeadwordRecord {
                surface: "は".into(),
                flags: WordFlags::PARTICLE,
                verb_types: vec![],
                entry_id: 1,
            },
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn produces_nothing_when_no_remove_suffix_matches_the_tail() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("xyz", "v1", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: false,
            },
            &mut stats,
        );
        assert!(out.is_empty());
    }

    #[test]
    fn stems_inherit_flags_and_entry_id() {
        let t = table();
        let mut stats = StemStats::default();
        let out = generate_stems(
            &record("食べる", "v1", &t),
            &t,
            &StemOptions::default(),
            &mut stats,
        );
        let stem = out.iter().find(|r| r.surface == "食べ").unwrap();
        assert_eq!(stem.entry_id, 1);
        assert!(stem.flags.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn counts_exact_and_fallback_stems_independently() {
        // The two counters are tracked separately, and a single record can
        // only ever produce one or the other: a correctly-annotated verb
        // (v5u on 言う) only ever counts as exact; a mis-annotated one (v5s
        // on 言う, see the fallback test above) only ever counts as
        // fallback. Each case asserts both the positive and the zero side,
        // which is what actually pins the separation.
        let t = table();

        let mut exact_only = StemStats::default();
        generate_stems(
            &record("言う", "v5u", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: true,
            },
            &mut exact_only,
        );
        assert!(exact_only.exact_stems > 0);
        assert_eq!(exact_only.v5_fallback_stems, 0);

        let mut fallback_only = StemStats::default();
        generate_stems(
            &record("言う", "v5s", &t),
            &t,
            &StemOptions {
                v5_misannotation_fallback: true,
            },
            &mut fallback_only,
        );
        assert_eq!(fallback_only.exact_stems, 0);
        assert!(fallback_only.v5_fallback_stems > 0);
    }

    #[test]
    fn reaches_the_non_annotated_twin_of_a_duplicate_named_type() {
        // "vk" (来る, "kuru") is one of the four duplicate-named types: one
        // table entry's remove/form-0 suffix is the kanji form 来る, the
        // other is the kana form くる. A record's `verb_types` normally
        // carries both ids (record::headwords attaches every id
        // `types_named` returns), but this test pins the mechanism inside
        // `generate_stems` directly: a record annotated with only ONE
        // twin's id must still reach the OTHER twin, because the candidate
        // loop scans every type in the table by name, not by id. If the
        // loop were narrowed to just the annotated id(s), this candidate
        // would never be tried and the stem would be silently lost.
        let t = table();
        let ids = t.types_named("vk");
        assert_eq!(ids.len(), 2, "vk must be duplicated in the embedded table");

        let remove_suffix = |id: VerbTypeId| -> String {
            let ty = &t.types()[id];
            ty.conjugations
                .iter()
                .find(|c| c.tense == ty.remove_tense && c.form.0 == 0)
                .expect("vk must declare a remove/form-0 conjugation")
                .suffix
                .clone()
        };

        let (kanji_id, kana_id) = if remove_suffix(ids[0]) == "来る" {
            (ids[0], ids[1])
        } else {
            (ids[1], ids[0])
        };
        assert_eq!(remove_suffix(kanji_id), "来る");
        assert_eq!(remove_suffix(kana_id), "くる");

        // Annotate with only the kanji twin's id, but give a kana-only
        // surface that only the kana twin's suffix can strip.
        let rec = HeadwordRecord {
            surface: "くる".to_string(),
            flags: WordFlags::PRIMARY,
            verb_types: vec![kanji_id],
            entry_id: 1,
        };
        let mut stats = StemStats::default();
        let out = generate_stems(&rec, &t, &StemOptions::default(), &mut stats);

        assert_eq!(out.len(), 1, "got {:?}", out);
        assert!(
            out[0].surface.is_empty() && out[0].verb_types == vec![kana_id],
            "the non-annotated kana twin must still be reachable: got {:?}",
            out
        );
    }
}
