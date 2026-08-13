// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! insta snapshots over real sentences, against a curated JMdict subset.
//!
//! The subset is committed (see `fixtures/README.md`) so this suite runs
//! offline from a fresh clone and does not move when JMdict is rebuilt. Two
//! targeted assertions sit beside the snapshot for する and 来る, whose
//! generated stem is the empty string: a snapshot diff would report that
//! breakage as "something changed", while these report what changed.

use std::path::PathBuf;
use std::sync::OnceLock;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::{ParseOptions, ParseResult};

const FIXTURE: &str = include_str!("fixtures/jmdict_subset.xml");
const SENTENCES: &str = include_str!("fixtures/parse_sentences.txt");

/// Alternatives printed per span. The full list is often a dozen entries for a
/// single-kana particle; five is enough to pin `sort_matches`' ranking while
/// keeping the snapshot reviewable by a human, which is the entire point of it.
const MAX_ALTERNATIVES: usize = 5;

/// Printed wherever a reading or conjugation is `None`.
const NONE_LABEL: &str = "-";

static INDEX_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Build the curated index exactly once for the whole test binary. Tests run
/// in parallel threads; `Index::open`'s contract forbids writing to a
/// directory while an index over it is alive, and `get_or_init` is what keeps
/// every `open` strictly after the single build.
fn index_dir() -> &'static PathBuf {
    INDEX_DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("jparser-test-parse-snapshots");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let table = ConjugationTable::load_embedded().unwrap();
        let report = build_from_reader(
            std::io::Cursor::new(FIXTURE),
            &table,
            &StemOptions::default(),
            &dir,
        )
        .expect("the curated fixture must build");
        assert_eq!(
            report.skipped_entries, 0,
            "the curated fixture must not contain malformed entries"
        );
        dir
    })
}

fn sentences() -> impl Iterator<Item = &'static str> {
    SENTENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn parsed(text: &str) -> ParseResult {
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(index_dir()).unwrap();
    jparser::parse(&index, &table, text, &ParseOptions::default(), None).unwrap()
}

/// Test-only renderer. `jparser-cli` has its own copy of this shape; a `[[bin]]`
/// cannot export code to an integration test, and making a formatter public
/// would put a display concern in the library's API for the sake of fifteen
/// lines. The two deliberately differ: the CLI prints every alternative and all
/// of the first sense's glosses, this prints five alternatives and one gloss.
fn render(text: &str, result: &ParseResult) -> String {
    let mut out = String::new();
    out.push_str("=== ");
    out.push_str(text);
    out.push('\n');
    for seg in &result.segments {
        if !seg.matched {
            out.push_str(&format!(
                "start={} len={} {} unmatched\n",
                seg.start, seg.len, seg.surface
            ));
            continue;
        }
        out.push_str(&format!(
            "start={} len={} {} matched reading={}\n",
            seg.start,
            seg.len,
            seg.surface,
            seg.reading.as_deref().unwrap_or(NONE_LABEL)
        ));
        for entry in seg.entries.iter().take(MAX_ALTERNATIVES) {
            let gloss = entry
                .senses
                .first()
                .and_then(|s| s.glosses.first())
                .map(String::as_str)
                .unwrap_or("");
            out.push_str(&format!(
                "    {} ({}) [{}] {gloss}\n",
                entry.headword,
                entry.conjugation.as_deref().unwrap_or(NONE_LABEL),
                entry.reading.as_deref().unwrap_or(NONE_LABEL),
            ));
        }
        if seg.entries.len() > MAX_ALTERNATIVES {
            out.push_str(&format!(
                "    ... {} more\n",
                seg.entries.len() - MAX_ALTERNATIVES
            ));
        }
    }
    out
}

#[test]
fn snapshots_every_sentence() {
    let mut out = String::new();
    for sentence in sentences() {
        out.push_str(&render(sentence, &parsed(sentence)));
        out.push('\n');
    }
    insta::assert_snapshot!("sentences", out);
}

#[test]
fn corpus_has_thirty_sentences_covering_both_irregulars() {
    // Guards the corpus itself: addendum §6 requires ~30 sentences and at
    // least one する or 来る. Deleting a sentence to make a snapshot green
    // should fail here first.
    let all: Vec<&str> = sentences().collect();
    assert_eq!(all.len(), 30, "got {} sentences", all.len());
    assert!(all.iter().any(|s| s.contains("する")));
    assert!(all.iter().any(|s| s.contains("来る")));
}

#[test]
fn matches_suru_through_the_empty_stem_key() {
    // する is vs-i, whose remove-tense/form-0 suffix is the whole word, so its
    // generated stem is "" and the FST returns a key_chars == 0 hit: src_len 0,
    // len 4. Chain is vs-i Stem "し" -> v-i-stem Formal Past "ました".
    //
    // 勉強をする's bare する cannot be used here: the plain headword する is also
    // indexed (surface "する", verb_type None) at the same span and for the same
    // entry, and sort_matches' verb-plain/non-verb collapse (§6.5 pass B rule 3)
    // drops the Non-past verb match in favour of it — so that span's entry has
    // conjugation None and never touches the empty key. しました is immune
    // because chain[0].tense is Stem, not Non-past.
    let result = parsed("昨日は宿題をしました。");
    let seg = result
        .segments
        .iter()
        .find(|s| s.surface == "しました")
        .expect("しました must be one span");
    let entry = &seg.entries[0];
    assert_eq!(entry.headword, "する");
    assert_eq!(entry.conjugation.as_deref(), Some("Formal Past"));
}

#[test]
fn reconstructs_the_kuru_reading_through_the_kanji_kana_twin() {
    // 来ます is the kuruHack path: chain[0] is vk's kanji type, whose Stem
    // suffix is 来 -> v-i-stem, and the reading cannot be rebuilt from the
    // kana sibling くる by suffix stripping alone. kuru_hack finds the kana
    // twin's matching slot (き), and the reading becomes
    //   "" + "き" + text[start + src_len + 1 .. start + len]  ==  きます
    // The label is "Formal Non-past": depth 0 is Stem (always skipped), and
    // depth 1's Non-past survives only because depth 0's tense was Stem.
    let result = parsed("明日友達が来ます。");
    let seg = result
        .segments
        .iter()
        .find(|s| s.surface == "来ます")
        .expect("来ます must be one span");
    assert_eq!(seg.reading.as_deref(), Some("きます"));
    let entry = &seg.entries[0];
    assert_eq!(entry.headword, "来る");
    assert_eq!(entry.conjugation.as_deref(), Some("Formal Non-past"));
}
