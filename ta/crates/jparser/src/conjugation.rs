// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The conjugation table, ported from `LoadConjugationTable`
//! (`ta-old/exe/util/Dictionary.cpp:584`).
//!
//! Two kinds of type live in this table and the distinction matters:
//!
//! * **Entry types** are reachable from a JMdict part-of-speech tag. Their
//!   names *are* EDICT POS codes: `v1`, `v5r`, `adj-i`, `vs-i`, and so on.
//! * **Chain-only types** are never matched against a dictionary POS and exist
//!   solely as `Next Type` targets: `copula`, `adj-ta`, `v-i-stem`,
//!   `v-a-stem`, `v-ta-stem`, `v-u-stem`.
//!
//! Four names appear twice (`vk`, `vs`, `v5r-i`, `v5uru`). This is deliberate:
//! one entry carries kanji-form suffixes and its twin carries kana-form
//! suffixes, which is what lets readings be reconstructed for irregular verbs.
//! Lookup therefore returns *all* matching type ids, never just the first.

use serde::Deserialize;

pub type TenseId = usize;
pub type VerbTypeId = usize;

/// Maximum recursive verb conjugations, ta-old's `MAX_CONJ_DEPTH`.
pub const MAX_CONJ_DEPTH: usize = 5;

pub const TENSE_REMOVE: TenseId = 0;
pub const TENSE_NON_PAST: TenseId = 1;
pub const TENSE_STEM: TenseId = 2;
pub const TENSE_POTENTIAL: TenseId = 3;

/// Tense names seeded in fixed order. The first four positions are
/// special-cased by the matcher and must not be reordered. Names beyond this
/// list are interned from the asset in encounter order.
const STATIC_TENSES: &[&str] = &[
    "Remove", "Non-past", "Stem", "Potential", "Past", "Te-form", "Conditional",
    "Provisional", "Passive", "Causative", "Caus-Pass", "Volitional",
    "Conjectural", "Adverbal", "Alternative", "Imperative", "Imperfective",
    "Continuative", "Hypothetical", "Prenominal",
];

const PART_OF_SPEECH_VERB: &str = "Verb";
const PART_OF_SPEECH_ADJ: &str = "Adj";

/// Formality and polarity, packed as ta-old did: `formal | negative << 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Form(pub u8);

impl Form {
    pub const FORMAL: u8 = 1;
    pub const NEGATIVE: u8 = 2;

    pub fn from_flags(formal: bool, negative: bool) -> Self {
        Form(u8::from(formal) | (u8::from(negative) << 1))
    }
    pub fn is_formal(self) -> bool {
        self.0 & Self::FORMAL != 0
    }
    pub fn is_negative(self) -> bool {
        self.0 & Self::NEGATIVE != 0
    }
}

#[derive(Debug, Clone)]
pub struct Conjugation {
    pub tense: TenseId,
    pub form: Form,
    /// Suffix to match against the source text. For linked conjugations this
    /// has already had the target type's remove-suffix trimmed off.
    pub suffix: String,
    pub next_verb_type: Option<VerbTypeId>,
}

#[derive(Debug, Clone)]
pub struct VerbType {
    pub name: String,
    pub is_adjective: bool,
    /// Tense whose suffix is stripped to form the stem: `TENSE_REMOVE` when the
    /// type declares one, otherwise `TENSE_NON_PAST`.
    pub remove_tense: TenseId,
    pub conjugations: Vec<Conjugation>,
}

#[derive(Debug, Clone)]
pub struct ConjugationTable {
    types: Vec<VerbType>,
    tense_names: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConjugationError {
    #[error("conjugation asset is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("verb type {name:?} has unsupported part of speech {pos:?}")]
    BadPartOfSpeech { name: String, pos: String },
    #[error(
        "verb type {name:?} references Next Type {next:?}, which has no \
         remove-tense conjugation whose suffix matches"
    )]
    UnresolvedNextType { name: String, next: String },
}

/// Raw asset shape. Field names match ta-old's JSON exactly.
#[derive(Deserialize)]
struct RawType {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Part of Speech")]
    part_of_speech: String,
    #[serde(rename = "Tenses")]
    tenses: Vec<RawConjugation>,
}

#[derive(Deserialize)]
struct RawConjugation {
    #[serde(rename = "Formal")]
    formal: bool,
    #[serde(rename = "Negative")]
    negative: bool,
    #[serde(rename = "Suffix")]
    suffix: String,
    #[serde(rename = "Tense")]
    tense: String,
    #[serde(rename = "Next Type")]
    next_type: Option<String>,
}

const EMBEDDED_ASSET: &str = include_str!("../assets/conjugations.json");

impl ConjugationTable {
    pub fn load_embedded() -> Result<Self, ConjugationError> {
        Self::from_json(EMBEDDED_ASSET)
    }

    pub fn from_json(json: &str) -> Result<Self, ConjugationError> {
        let raw: Vec<RawType> = serde_json::from_str(json)?;
        let mut tense_names: Vec<String> =
            STATIC_TENSES.iter().map(|s| (*s).to_string()).collect();
        let mut types = Vec::with_capacity(raw.len());
        // Parallel to `types`: the unresolved Next Type name per conjugation,
        // so resolution can run once every type exists.
        let mut pending: Vec<Vec<Option<String>>> = Vec::with_capacity(raw.len());

        for rt in raw {
            let is_adjective = match rt.part_of_speech.as_str() {
                PART_OF_SPEECH_ADJ => true,
                PART_OF_SPEECH_VERB => false,
                other => {
                    return Err(ConjugationError::BadPartOfSpeech {
                        name: rt.name,
                        pos: other.to_string(),
                    })
                }
            };

            let mut remove_tense = TENSE_NON_PAST;
            let mut conjugations = Vec::with_capacity(rt.tenses.len());
            let mut names = Vec::with_capacity(rt.tenses.len());

            for rc in rt.tenses {
                let tense = match tense_names.iter().position(|n| *n == rc.tense) {
                    Some(id) => id,
                    None => {
                        tense_names.push(rc.tense.clone());
                        tense_names.len() - 1
                    }
                };
                if tense == TENSE_REMOVE {
                    remove_tense = TENSE_REMOVE;
                }
                conjugations.push(Conjugation {
                    tense,
                    form: Form::from_flags(rc.formal, rc.negative),
                    suffix: rc.suffix,
                    next_verb_type: None,
                });
                names.push(rc.next_type);
            }

            types.push(VerbType { name: rt.name, is_adjective, remove_tense, conjugations });
            pending.push(names);
        }

        // Resolve chained conjugations. For each conjugation carrying a Next
        // Type, find that target type's remove-tense/form-0 conjugation whose
        // suffix is a suffix of this one, trim it off, and store the link.
        // This is what allows conjugations to stack (て + いる + ない), and it
        // is ta-old's four nested loops in LoadConjugationTable.
        for ti in 0..types.len() {
            for ci in 0..types[ti].conjugations.len() {
                let Some(target_name) = pending[ti][ci].clone() else { continue };
                let suffix = types[ti].conjugations[ci].suffix.clone();
                let mut link = None;

                'outer: for tj in 0..types.len() {
                    if types[tj].name != target_name {
                        continue;
                    }
                    let remove_tense = types[tj].remove_tense;
                    for c2 in &types[tj].conjugations {
                        if c2.tense != remove_tense || c2.form.0 != 0 {
                            continue;
                        }
                        if let Some(trimmed) = strip_unified_suffix(&suffix, &c2.suffix) {
                            link = Some((tj, trimmed));
                            break 'outer;
                        }
                    }
                }

                let Some((target, trimmed)) = link else {
                    return Err(ConjugationError::UnresolvedNextType {
                        name: types[ti].name.clone(),
                        next: target_name,
                    });
                };
                types[ti].conjugations[ci].suffix = trimmed;
                types[ti].conjugations[ci].next_verb_type = Some(target);
            }
        }

        Ok(ConjugationTable { types, tense_names })
    }

    pub fn types(&self) -> &[VerbType] {
        &self.types
    }

    pub fn tense_name(&self, id: TenseId) -> Option<&str> {
        self.tense_names.get(id).map(String::as_str)
    }

    /// All type ids with this name. Returns more than one for `vk`, `vs`,
    /// `v5r-i`, and `v5uru`; callers must handle every result.
    pub fn types_named(&self, name: &str) -> Vec<VerbTypeId> {
        self.types
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name == name)
            .map(|(i, _)| i)
            .collect()
    }
}

/// If `target` is a kana-insensitive suffix of `suffix`, return `suffix` with it
/// removed. Comparison uses `unify` so hiragana and katakana forms match.
fn strip_unified_suffix(suffix: &str, target: &str) -> Option<String> {
    let s: Vec<char> = suffix.chars().collect();
    let t: Vec<char> = target.chars().collect();
    if t.len() > s.len() {
        return None;
    }
    let split = s.len() - t.len();
    let matches = s[split..]
        .iter()
        .zip(t.iter())
        .all(|(a, b)| crate::kana::unify(*a) == crate::kana::unify(*b));
    matches.then(|| s[..split].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ConjugationTable {
        ConjugationTable::load_embedded().expect("embedded asset must load")
    }

    #[test]
    fn loads_all_thirty_two_types() {
        assert_eq!(table().types().len(), 32);
    }

    #[test]
    fn retains_all_duplicate_named_types() {
        // vk, vs, v5r-i and v5uru each appear twice. Both entries must survive:
        // GetDictEntry's kuruHack pairs a kanji-suffix type with its kana twin
        // to reconstruct readings for irregular verbs.
        let t = table();
        for name in ["vk", "vs", "v5r-i", "v5uru"] {
            assert_eq!(t.types_named(name).len(), 2, "{name} should appear twice");
        }
        assert_eq!(t.types_named("v1").len(), 1);
        assert_eq!(t.types_named("nonexistent").len(), 0);
    }

    #[test]
    fn fixed_tense_discriminants_are_stable() {
        let t = table();
        assert_eq!(t.tense_name(TENSE_REMOVE), Some("Remove"));
        assert_eq!(t.tense_name(TENSE_NON_PAST), Some("Non-past"));
        assert_eq!(t.tense_name(TENSE_STEM), Some("Stem"));
        assert_eq!(t.tense_name(TENSE_POTENTIAL), Some("Potential"));
    }

    #[test]
    fn interns_tense_names_beyond_the_static_list() {
        let t = table();
        // Contributed by the asset, not the static list.
        assert!(
            (0..64).any(|i| t.tense_name(i) == Some("Past Volitional")),
            "asset tense names must be interned"
        );
    }

    #[test]
    fn adjective_types_are_flagged() {
        let t = table();
        let adj = t.types_named("adj-i");
        assert!(t.types()[adj[0]].is_adjective);
        let verb = t.types_named("v1");
        assert!(!t.types()[verb[0]].is_adjective);
    }

    #[test]
    fn chain_only_types_are_present() {
        // These are never matched against a dictionary POS; they exist solely
        // as Next Type targets, so they must still load.
        let t = table();
        for name in ["copula", "adj-ta", "v-i-stem", "v-a-stem", "v-ta-stem", "v-u-stem"] {
            assert!(!t.types_named(name).is_empty(), "{name} must load");
        }
    }

    #[test]
    fn resolves_every_next_type_reference() {
        // 223 conjugations carry a Next Type; all must resolve or load fails.
        let t = table();
        let linked = t
            .types()
            .iter()
            .flat_map(|ty| &ty.conjugations)
            .filter(|c| c.next_verb_type.is_some())
            .count();
        assert_eq!(linked, 223);
    }

    #[test]
    fn resolution_trims_the_target_remove_suffix_from_the_linked_conjugation() {
        // Hermetic fixture mirroring a real chain (v5s/Potential せる + v1
        // remove-suffix る → せ). Pins both effects of a Next Type link: if
        // `suffix = trimmed;` were dropped and only `next_verb_type` kept,
        // `next_verb_type` would still be `Some` but `suffix` would stay
        // "せる" — this test would then fail on the suffix assertion.
        let json = r#"[
          {"Name":"target","Part of Speech":"Verb","Tenses":[
            {"Formal":false,"Negative":false,"Suffix":"る","Tense":"Non-past"}
          ]},
          {"Name":"source","Part of Speech":"Verb","Tenses":[
            {"Formal":false,"Negative":false,"Suffix":"せる","Tense":"Potential",
             "Next Type":"target"}
          ]}
        ]"#;
        let t = ConjugationTable::from_json(json).expect("fixture must load");
        let target_id = t.types_named("target")[0];
        let source_id = t.types_named("source")[0];
        let c = &t.types()[source_id].conjugations[0];
        assert_eq!(c.suffix, "せ");
        assert_eq!(c.next_verb_type, Some(target_id));
    }

    #[test]
    fn every_link_target_has_a_remove_tense_conjugation() {
        let t = table();
        for ty in t.types() {
            for c in &ty.conjugations {
                let Some(next) = c.next_verb_type else { continue };
                let target = &t.types()[next];
                assert!(
                    target
                        .conjugations
                        .iter()
                        .any(|c2| c2.tense == target.remove_tense && c2.form.0 == 0),
                    "target type {} needs a remove/form-0 conjugation",
                    target.name
                );
            }
        }
    }

    #[test]
    fn remove_tense_is_remove_or_non_past() {
        // ta-old defaults remove_tense to NON_PAST and switches to REMOVE only
        // when the type declares a "Remove" tense.
        let t = table();
        for ty in t.types() {
            assert!(ty.remove_tense == TENSE_REMOVE || ty.remove_tense == TENSE_NON_PAST);
        }
    }

    #[test]
    fn form_packs_formal_and_negative_bits() {
        assert_eq!(Form::from_flags(false, false).0, 0);
        assert_eq!(Form::from_flags(true, false).0, 1);
        assert_eq!(Form::from_flags(false, true).0, 2);
        assert_eq!(Form::from_flags(true, true).0, 3);
        assert!(Form(3).is_formal());
        assert!(Form(3).is_negative());
        assert!(!Form(1).is_negative());
        assert!(!Form(2).is_formal());
    }

    #[test]
    fn max_conj_depth_matches_ta_old() {
        assert_eq!(MAX_CONJ_DEPTH, 5);
    }

    #[test]
    fn rejects_an_unresolvable_next_type() {
        let json = r#"[
          {"Name":"v1","Part of Speech":"Verb","Tenses":[
            {"Formal":false,"Negative":false,"Suffix":"る","Tense":"Non-past",
             "Next Type":"does-not-exist"}
          ]}
        ]"#;
        let err = ConjugationTable::from_json(json).unwrap_err();
        assert!(matches!(err, ConjugationError::UnresolvedNextType { .. }), "got {err:?}");
    }

    #[test]
    fn rejects_an_unknown_part_of_speech() {
        let json = r#"[{"Name":"x","Part of Speech":"Noun","Tenses":[]}]"#;
        let err = ConjugationTable::from_json(json).unwrap_err();
        assert!(matches!(err, ConjugationError::BadPartOfSpeech { .. }), "got {err:?}");
    }

    #[test]
    fn rejects_malformed_json() {
        let err = ConjugationTable::from_json("{not json").unwrap_err();
        assert!(matches!(err, ConjugationError::Json(_)), "got {err:?}");
    }
}
