// ClipYomi — Engine v2, Phase 1b-i.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b-ii, when the crate stops being GPL-2.0-only.
//
// English vocabulary from "Japanese verb conjugation", English Wikipedia,
// https://en.wikipedia.org/wiki/Japanese_verb_conjugation, read 2026-09;
// terminology only. assets/conjugations.json, src/conjugation.rs and
// src/matcher/ (matcher.rs and matcher/verb.rs) were not opened while this
// file or tests/fixtures/conjugation_labels.txt was written.

//! The English conjugation label -- Phase 1b-i's half of master plan §7.3's
//! recombiner. Reads [`ConjStep`]s, writes `Entry::conjugation`.
//!
//! Three tables and four rules (design §3). Every auxiliary lemma
//! contributes one word, in step order; the outermost step's form
//! contributes one more; each word appears at most once; and three
//! `(lemma, outermost form)` pairs collapse into a single word. Zero words is
//! `None`. The tables are whatever passes
//! `tests/fixtures/conjugation_labels.txt`, which was written first and is
//! authoritative -- a row this code cannot pass is a finding about the
//! table, not a row to edit.

use crate::engine::table::CLASSES;
use crate::engine::ConjStep;

/// One word per auxiliary lemma, spelled as `table.rs`'s `ATTACHES_TO`
/// spells it (design §3.1). れる/られる are ambiguous between passive,
/// potential and honorific and the parser cannot tell, so the label says so.
/// だ here is the voiced た of 読んだ, not the copula. じゃ/や are
/// `ATTACHES_TO`'s placeholder rows.
const AUX_WORD: &[(&str, &str)] = &[
    ("せる", "Causative"),
    ("させる", "Causative"),
    ("れる", "Passive/Potential"),
    ("られる", "Passive/Potential"),
    ("ない", "Negative"),
    ("ぬ", "Negative"),
    ("ん", "Negative"),
    ("う", "Volitional"),
    ("よう", "Volitional"),
    ("ます", "Polite"),
    ("です", "Polite"),
    ("たい", "Want-to"),
    ("た", "Past"),
    ("だ", "Past"),
    ("て", "Te-form"),
    ("で", "Te-form"),
    ("じゃ", "Copula"),
    ("や", "Copula"),
];

/// One word from the outermost step's 活用形 (design §3.2). A form not
/// listed here -- 基本形, every 未然/連用 attachment stem, 体言接続, the
/// 文語 and 音便 variants -- adds nothing: it is either the plain form or a
/// dangling stem whose meaning arrives with the next auxiliary.
const FORM_WORD: &[(&str, &str)] = &[
    ("仮定形", "Conditional"),
    ("仮定縮約１", "Conditional"),
    ("仮定縮約２", "Conditional"),
    ("命令ｅ", "Imperative"),
    ("命令ｉ", "Imperative"),
    ("命令ｒｏ", "Imperative"),
    ("命令ｙｏ", "Imperative"),
    ("連用テ接続", "Adverbial"),
];

/// `(lemma, outermost form)` pairs whose two words collapse into one
/// (design §3.3 rule 2). たら is a conditional, not a "Past Conditional";
/// 読んで through the copula class's 連用形 is the te-form, not "Past".
const OVERRIDE: &[(&str, &str, &str)] = &[
    ("た", "仮定形", "Conditional"),
    ("だ", "仮定形", "Conditional"),
    ("だ", "連用形", "Te-form"),
];

/// The word for an auxiliary lemma, if `AUX_WORD` knows it.
pub(crate) fn aux_word(lemma: &str) -> Option<&'static str> {
    AUX_WORD.iter().find(|(l, _)| *l == lemma).map(|(_, w)| *w)
}

fn form_word(form: &str) -> Option<&'static str> {
    FORM_WORD.iter().find(|(f, _)| *f == form).map(|(_, w)| *w)
}

fn override_word(lemma: &str, form: &str) -> Option<&'static str> {
    OVERRIDE
        .iter()
        .find(|(l, f, _)| *l == lemma && *f == form)
        .map(|(_, _, w)| *w)
}

fn form_name(step: &ConjStep) -> &'static str {
    CLASSES[step.class as usize].forms[step.form as usize].name
}

/// Rule 1's dedup: a word already present is not repeated.
fn push(words: &mut Vec<&'static str>, word: Option<&'static str>) {
    if let Some(w) = word {
        if !words.contains(&w) {
            words.push(w);
        }
    }
}

/// The learner-facing label for a deinflection chain, innermost step first,
/// or `None` when no step contributes a word (an uninflected match, a plain
/// form, or a bare stem).
pub fn fmt(steps: &[ConjStep]) -> Option<String> {
    let (last, inner) = steps.split_last()?;
    let mut words: Vec<&'static str> = Vec::new();
    for step in inner {
        push(&mut words, step.aux.and_then(aux_word));
    }
    let form = form_name(last);
    match last.aux {
        Some(lemma) => match override_word(lemma, form) {
            Some(w) => push(&mut words, Some(w)),
            None => {
                push(&mut words, aux_word(lemma));
                push(&mut words, form_word(form));
            }
        },
        None => push(&mut words, form_word(form)),
    }
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_id(name: &str) -> u16 {
        CLASSES
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("no class {name}")) as u16
    }

    fn form_id(class: &str, form: &str) -> u16 {
        CLASSES[class_id(class) as usize]
            .forms
            .iter()
            .position(|f| f.name == form)
            .unwrap_or_else(|| panic!("{class} has no form {form}")) as u16
    }

    /// A step by names. Ranges are irrelevant to `fmt` and left empty.
    fn step(class: &str, form: &str, aux: Option<&'static str>) -> ConjStep {
        ConjStep {
            class: class_id(class),
            form: form_id(class, form),
            range: 0..0,
            aux,
        }
    }

    #[test]
    fn no_steps_is_no_label() {
        assert_eq!(fmt(&[]), None);
    }

    #[test]
    fn a_plain_form_or_a_bare_stem_is_no_label() {
        assert_eq!(fmt(&[step("一段", "基本形", None)]), None);
        assert_eq!(fmt(&[step("五段・カ行イ音便", "連用タ接続", None)]), None);
        assert_eq!(fmt(&[step("一段", "未然形", None)]), None);
    }

    #[test]
    fn a_form_alone_can_carry_a_word() {
        assert_eq!(
            fmt(&[step("五段・カ行イ音便", "仮定形", None)]).as_deref(),
            Some("Conditional")
        );
        assert_eq!(
            fmt(&[step("形容詞・アウオ段", "連用テ接続", None)]).as_deref(),
            Some("Adverbial")
        );
    }

    #[test]
    fn words_come_in_step_order_then_the_outermost_form() {
        let steps = [
            step("一段", "未然形", None),
            step("一段", "未然形", Some("させる")),
            step("一段", "仮定形", Some("られる")),
        ];
        assert_eq!(
            fmt(&steps).as_deref(),
            Some("Causative Passive/Potential Conditional")
        );
    }

    #[test]
    fn a_repeated_word_appears_once() {
        // ませんでした: ます and です both say Polite.
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "未然形", Some("ます")),
            step("不変化型", "基本形", Some("ん")),
            step("特殊・デス", "連用形", Some("です")),
            step("特殊・タ", "基本形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Polite Negative Past"));
    }

    #[test]
    fn an_override_replaces_both_words_but_keeps_the_inner_ones() {
        // たら alone is Conditional, not Past Conditional...
        let steps = [
            step("一段", "連用形", None),
            step("特殊・タ", "仮定形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Conditional"));
        // ...and ましたら keeps Polite in front of it.
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "連用形", Some("ます")),
            step("特殊・タ", "仮定形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Polite Conditional"));
    }

    #[test]
    fn only_the_outermost_steps_form_contributes_a_word() {
        // No chain the table can generate today distinguishes this from
        // "any step's form": every 活用形 an auxiliary attaches to
        // contributes no word, so a form word can only ever land on the
        // last step. That equivalence is accidental, and it ends the moment
        // an ATTACHES_TO row requiring 連用テ接続 is added -- design §7's
        // first table finding contemplates exactly that. Built by hand
        // rather than through the trie, so the distinction is pinned before
        // it becomes reachable.
        let steps = [
            step("一段", "連用形", None),
            step("特殊・タ", "仮定形", Some("た")),
            step("特殊・マス", "基本形", Some("ます")),
        ];
        // た:仮定形 is both a FORM_WORD form and an OVERRIDE pair, but it
        // sits on an inner step here, so only た's own word survives: an
        // inner form word would give "Past Conditional Polite", and an
        // inner override would give "Conditional Polite".
        assert_eq!(fmt(&steps).as_deref(), Some("Past Polite"));
    }

    #[test]
    fn an_unknown_lemma_contributes_nothing_rather_than_panicking() {
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "仮定形", Some("not-a-lemma")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Conditional"));
    }
}
