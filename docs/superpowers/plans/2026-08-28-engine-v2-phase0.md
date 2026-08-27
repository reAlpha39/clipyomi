# Engine v2 Phase 0 — Harness and Baseline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `jparser-cli diff | label | score`, a committed genre-extended gold corpus, and a recorded span-accuracy / entry-top-1 baseline for the current engine — the instrument every later Engine v2 phase is measured against.

**Architecture:** Three new subcommands on the existing `jparser-cli` binary, which becomes a directory of focused modules. `diff` samples sentences where jparser has a segmentation cut that Vibrato does not (one-directional, so JMdict compound preservation is not treated as a conflict). `label` adjudicates them into append-only JSONL with candidates listed by `ent_seq` so gold cannot become a copy of the ranker's answer. `score` measures two metrics against a committed JMdict subset, offline. No parsing code changes; one additive read-only field on `jparser::Entry`.

**Tech Stack:** Rust 2021 (rust-version 1.85), `clap` 4 derive, `serde` / `serde_json`, `vibrato` 0.5 behind the existing `mecab` feature, `insta` for existing snapshots only. Python 3 stdlib for the two `tools/` scripts. No new dependency in any language.

**Spec:** [`docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md`](../specs/2026-08-28-engine-v2-phase0-design.md) — approved. Its parent is [`docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`](../specs/2026-08-27-engine-v2-master-plan.md) §5, §7 Phase 0, §9.1.

## Global Constraints

Every task's requirements implicitly include this section.

- **Repo:** `D:\Code\oss\clipyomi`, branch `develop`. Workspace members: `crates/jparser`, `crates/jmdict-source`, `crates/vibrato-source`, `xtask`, `src-tauri`.
- **Do not touch the parser library.** No edit to `crates/jparser/src/segment.rs`, `matcher.rs`, `matcher/verb.rs`, `rank.rs`, `stem.rs`, `conjugation.rs`, `hints.rs`, `index/*`, `jmdict.rs`, `record.rs`, `kana.rs`, `romaji.rs`. The only permitted `src/` change outside `src/bin/` is Task 2's two lines in `lib.rs` (spec §2.1, §5.4).
- **Do not touch the existing snapshot assets.** `crates/jparser/tests/fixtures/parse_sentences.txt`, `crates/jparser/tests/fixtures/jmdict_subset.xml`, and `crates/jparser/tests/parse_snapshots.rs` are read-only for this whole plan (spec §2, §8.2).
- **No new dependencies.** Not in `crates/jparser/Cargo.toml`, not in `tools/*.py` beyond the Python standard library (spec §9.2, §9.3).
- **New Rust file header**, verbatim, on every file this plan creates. It differs from the existing ported-file header on purpose: these files have no ta-old provenance, and master plan §3.5 requires a header a reviewer can establish provenance from without asking.

  ```rust
  // ClipYomi — Engine v2 Phase 0 measurement harness.
  // Copyright (C) 2026
  //
  // This program is free software; you can redistribute it and/or modify it
  // under the terms of the GNU General Public License version 2 as published
  // by the Free Software Foundation.
  //
  // Written independently for the Engine v2 measurement harness. Nothing here
  // derives from Translation Aggregator.
  ```

- **Character offsets, never byte offsets.** Everywhere: gold spans, diff boundaries, `Segment::start`, `Segment::len`, `vibrato`'s `range_char()`.
- **Gate, run before every commit:**
  ```powershell
  cargo test -p jparser
  cargo check -p jparser --no-default-features
  cargo clippy --workspace -- -D warnings
  ```
  The `--no-default-features` check is load-bearing: it proves no harness code leaked into the library (spec §11.5).
- **Metric definitions are fixed by spec §5.2** and may not be adjusted to make a number look better. Span accuracy counts `ent_seq: null` spans. Entry top-1 is conditioned on exact spanning and non-`null` gold, and an exactly-spanned non-`null` gold span whose segment has `matched: false` or an empty `entries` list counts in the denominator and fails the numerator.

## File Structure

**Created:**

| File | Responsibility |
| --- | --- |
| `crates/jparser/src/bin/jparser-cli/main.rs` | clap definitions and dispatch. Moved from `src/bin/jparser-cli.rs`; gains three subcommand variants and their dispatch arms, no logic of its own. |
| `crates/jparser/src/bin/jparser-cli/corpus.rs` | `GoldSpan`, `GoldRecord`, `DiffRecord`, append-only writer, last-wins reader, cover validation, `CorpusError`. |
| `crates/jparser/src/bin/jparser-cli/score.rs` | `Metrics`, `accumulate`, the two metrics, the printed and `--json` forms. |
| `crates/jparser/src/bin/jparser-cli/diff.rs` | boundary extraction, the `Bj \ Bv` predicate, Vibrato tokenization, pool reading. |
| `crates/jparser/src/bin/jparser-cli/label.rs` | the interactive adjudicator. |
| `crates/jparser/tests/entry_id.rs` | Task 2's proof that `parse` returns the `ent_seq` the fixture declares. |
| `crates/jparser/tests/cli_score.rs` | `score` end to end against the mini fixture. |
| `crates/jparser/tests/cli_label.rs` | `label` end to end, driven by piped stdin. |
| `crates/jparser/tests/fixtures/gold.jsonl` | the committed gold corpus. |
| `crates/jparser/tests/fixtures/gold_jmdict_subset.xml` | the committed JMdict subset for that corpus. |
| `crates/jparser/tests/fixtures/gold_sources.md` | provenance manifest and derivation tables. |
| `tools/extract_vn_pool.py` | `vn_data/*.sqlite` to a TSV sampling pool. |
| `docs/superpowers/2026-08-28-engine-v2-phase0-handoff.md` | the recorded baseline. |

**Modified:**

| File | Change |
| --- | --- |
| `crates/jparser/Cargo.toml` | `[[bin]] path` to the new directory; three new `[[test]]` entries. |
| `crates/jparser/src/lib.rs` | `pub entry_id: u32` on `Entry`, set in `assemble_entry`. Two lines. |
| `tools/extract_jmdict_subset.py` | read `.jsonl` corpora as well as `.txt`. |
| `crates/jparser/tests/fixtures/README.md` | sections for the three new fixtures. |
| `.gitignore` | the uncommitted pool and local-gold paths. |
| `docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md` | §8.3's baseline table, filled. |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | §9.1 answered, §10 decision-log rows, the seven §14 deviations. |

---

## Task 1: Move the binary to a directory

Pure move. No behaviour change, and the existing CLI suites are the test — `cli_parse`, `cli_hints`, and `cli_generations` all invoke the binary through `env!("CARGO_BIN_EXE_jparser-cli")`, so if the move breaks the target they fail to build.

**Files:**
- Move: `crates/jparser/src/bin/jparser-cli.rs` → `crates/jparser/src/bin/jparser-cli/main.rs`
- Modify: `crates/jparser/Cargo.toml` (the `[[bin]]` block)

**Interfaces:**
- Consumes: nothing.
- Produces: the binary target `jparser-cli` at its new path. Every later task adds modules as siblings of `main.rs` and declares them there.

- [ ] **Step 1: Record the green baseline**

Run, and keep the output — Task 1 must not change any of it:

```powershell
cargo test -p jparser
```

Expected: all suites pass. Note the test counts per suite.

- [ ] **Step 2: Move the file**

```powershell
git mv crates/jparser/src/bin/jparser-cli.rs crates/jparser/src/bin/jparser-cli/main.rs
```

- [ ] **Step 3: Repoint the bin target**

In `crates/jparser/Cargo.toml`, the `[[bin]]` block becomes:

```toml
[[bin]]
name = "jparser-cli"
path = "src/bin/jparser-cli/main.rs"
required-features = ["cli"]
```

- [ ] **Step 4: Verify nothing changed**

```powershell
cargo test -p jparser
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
```

Expected: identical test counts to Step 1, all passing. Clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/jparser/Cargo.toml crates/jparser/src/bin/
git commit -m "refactor(jparser-cli): make the bin a directory

Phase 0 adds three subcommands with JSONL IO, a metric pass and an
interactive adjudicator. One 350-line file is the wrong place for them.
Pure move: the bin target and its output are unchanged."
```

---

## Task 2: Expose `entry_id` on `Entry`

Entry top-1 compares the parse's top-ranked `ent_seq` against gold. `assemble_entry` has `Match::entry_id` in hand and drops it, so the metric is unmeasurable as the API stands (spec §5.4). Two lines, additive, read-only, no behaviour change.

**Files:**
- Create: `crates/jparser/tests/entry_id.rs`
- Modify: `crates/jparser/src/lib.rs:94-110` (the `Entry` struct), `crates/jparser/src/lib.rs:196-214` (`assemble_entry`)
- Modify: `crates/jparser/Cargo.toml` (new `[[test]]` entry)

**Interfaces:**
- Consumes: nothing.
- Produces: `jparser::Entry::entry_id: u32` — the JMdict `ent_seq` of the entry this candidate came from. Tasks 4, 5, and 7 all read it.

- [ ] **Step 1: Write the failing test**

Create `crates/jparser/tests/entry_id.rs`:

```rust
// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! `Entry::entry_id` is the JMdict `ent_seq`, and Phase 0's entry-top-1 metric
//! is unmeasurable without it. `jmdict.rs` reads `ent_seq` straight off the
//! element into `RawEntry::id`, so this asserts the whole chain from XML to
//! public API rather than just that a field exists.

use std::path::PathBuf;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::stem::StemOptions;
use jparser::ParseOptions;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn index_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &dir,
    )
    .expect("the mini fixture must build");
    dir
}

/// 言う is `<ent_seq>1000010</ent_seq>` in the fixture, and 言った matches it
/// through the v5u mis-annotation fallback — the same case `cli_parse.rs`
/// pins the rendered output of.
#[test]
fn the_top_entry_carries_the_ent_seq_the_fixture_declares() {
    let dir = index_dir("entry-id-verb");
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(&dir).unwrap();

    let result = jparser::parse(&index, &table, "言った", &ParseOptions::default(), None)
        .expect("parse must succeed");

    let seg = result.segments.first().expect("one segment");
    assert_eq!((seg.start, seg.len), (0, 3), "one span over the whole text");
    let entry = seg.entries.first().expect("a matched entry");
    assert_eq!(entry.entry_id, 1000010, "言う's ent_seq");
}

/// A particle, so the id survives a non-verb path too: は is
/// `<ent_seq>1000020</ent_seq>` and reaches `assemble_entry` with an empty
/// conjugation chain.
#[test]
fn a_non_verb_entry_carries_its_ent_seq_too() {
    let dir = index_dir("entry-id-particle");
    let table = ConjugationTable::load_embedded().unwrap();
    let index = Index::open(&dir).unwrap();

    let result = jparser::parse(&index, &table, "は", &ParseOptions::default(), None)
        .expect("parse must succeed");

    let entry = result.segments[0].entries.first().expect("a matched entry");
    assert_eq!(entry.entry_id, 1000020, "は's ent_seq");
}
```

Register it in `crates/jparser/Cargo.toml`, after the existing `[[test]]` blocks:

```toml
[[test]]
name = "entry_id"
path = "tests/entry_id.rs"
```

- [ ] **Step 2: Run the test to verify it fails**

```powershell
cargo test -p jparser --test entry_id
```

Expected: FAIL to compile — `no field 'entry_id' on type 'jparser::Entry'`.

- [ ] **Step 3: Add the field**

In `crates/jparser/src/lib.rs`, in `pub struct Entry`, after `pub flags: WordFlags,`:

```rust
    /// The JMdict `ent_seq` this candidate came from.
    ///
    /// Read straight off the element by `jmdict.rs` into `RawEntry::id`, so it
    /// is the same stable public identifier a JMdict consumer would cite. The
    /// Phase 0 harness scores entry top-1 against it (`gold.jsonl`'s
    /// `ent_seq`); exposing it changes nothing about what `parse` computes.
    pub entry_id: u32,
```

In `assemble_entry`, in the `Entry { .. }` literal, after `flags: m.flags,`:

```rust
        entry_id: m.entry_id,
```

- [ ] **Step 4: Run the test to verify it passes**

```powershell
cargo test -p jparser --test entry_id
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Run the full gate**

```powershell
cargo test -p jparser
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
```

Expected: all green. `parse_snapshots` in particular must be unchanged — `Entry` is `Serialize`, but the snapshot renderer prints named fields rather than serializing the struct, so an added field does not move the snapshots. If a snapshot *does* move, stop and report it; do not accept the new snapshot.

- [ ] **Step 6: Commit**

```bash
git add crates/jparser/Cargo.toml crates/jparser/src/lib.rs crates/jparser/tests/entry_id.rs
git commit -m "feat(jparser): expose ent_seq on Entry

Entry top-1 compares the parse's top-ranked ent_seq against gold, and
assemble_entry had Match::entry_id in hand and dropped it. Additive and
read-only: parse computes nothing new, it only admits what it computed.

The alternative -- resolving gold ent_seq through Index::entry and
comparing headword and reading -- passes silently when two entries share
both, which is exactly the case the metric exists to catch."
```

---

## Task 3: `corpus.rs` — the JSONL formats

Record types, the append-only writer, the last-wins reader, and cover validation. No CLI surface yet; unit tests only. Bin targets run their `#[cfg(test)]` modules under `cargo test`, so these tests need no separate harness.

**Files:**
- Create: `crates/jparser/src/bin/jparser-cli/corpus.rs`
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (declare the module)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `struct GoldSpan { start: usize, len: usize, ent_seq: Option<u32> }`
  - `struct GoldRecord { text: String, source: String, spans: Vec<GoldSpan> }`
  - `struct DiffRecord { text: String, source: String, jparser: Vec<usize>, vibrato: Vec<usize>, conflicts: Vec<usize>, unknown_tokens: usize }`
  - `fn append<T: Serialize>(path: &Path, record: &T) -> Result<(), CorpusError>`
  - `fn read_gold(path: &Path) -> Result<(Vec<GoldRecord>, usize), CorpusError>` — records in first-seen order with each `text`'s **last** record kept, plus the count of superseded records dropped
  - `fn read_diffs(path: &Path) -> Result<Vec<DiffRecord>, CorpusError>`
  - `fn validate(record: &GoldRecord) -> Result<(), CorpusError>`
  - `enum CorpusError { Io, Json { line, source }, Cover { text, detail } }`

- [ ] **Step 1: Write the failing tests**

Create `crates/jparser/src/bin/jparser-cli/corpus.rs` containing **only** the header, the doc comment, and this test module — no implementation yet:

```rust
// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! The Phase 0 JSONL formats: the gold corpus, and `diff`'s disposable output.
//!
//! Writing is append-only and a later record for the same `text` supersedes
//! every earlier one (design §4.2). An interrupted labelling session therefore
//! loses nothing and a correction is another append, so no reader ever has to
//! rewrite the file. `read_gold` reports how many records it dropped as
//! superseded, because a file that is a third supersessions is a signal about
//! the labelling pass that silence would hide.

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-corpus-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn gold(text: &str, spans: &[(usize, usize, Option<u32>)]) -> GoldRecord {
        GoldRecord {
            text: text.to_string(),
            source: "test:1".to_string(),
            spans: spans
                .iter()
                .map(|&(start, len, ent_seq)| GoldSpan {
                    start,
                    len,
                    ent_seq,
                })
                .collect(),
        }
    }

    #[test]
    fn a_gold_record_round_trips_through_jsonl() {
        let dir = scratch("round-trip");
        let path = dir.join("gold.jsonl");
        let record = gold("言った", &[(0, 3, Some(1000010))]);

        append(&path, &record).expect("append");
        let (records, superseded) = read_gold(&path).expect("read");

        assert_eq!(records, vec![record]);
        assert_eq!(superseded, 0);
    }

    /// The append-only contract: the later record wins and the earlier one is
    /// counted, not silently discarded.
    #[test]
    fn a_later_record_supersedes_an_earlier_one_for_the_same_text() {
        let dir = scratch("last-wins");
        let path = dir.join("gold.jsonl");
        append(&path, &gold("は", &[(0, 1, Some(1000020))])).expect("append");
        append(&path, &gold("は", &[(0, 1, None)])).expect("append");

        let (records, superseded) = read_gold(&path).expect("read");

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].spans[0].ent_seq, None, "the later record wins");
        assert_eq!(superseded, 1);
    }

    /// First-seen order, so a corrected record does not jump to the end of the
    /// corpus and reorder the score output's per-sentence walk.
    #[test]
    fn supersession_keeps_first_seen_order() {
        let dir = scratch("order");
        let path = dir.join("gold.jsonl");
        append(&path, &gold("あ", &[(0, 1, None)])).expect("append");
        append(&path, &gold("い", &[(0, 1, None)])).expect("append");
        append(&path, &gold("あ", &[(0, 1, Some(7))])).expect("append");

        let (records, superseded) = read_gold(&path).expect("read");

        let texts: Vec<&str> = records.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["あ", "い"]);
        assert_eq!(records[0].spans[0].ent_seq, Some(7));
        assert_eq!(superseded, 1);
    }

    #[test]
    fn a_null_ent_seq_survives_the_round_trip_as_a_positive_label() {
        let dir = scratch("null-label");
        let path = dir.join("gold.jsonl");
        append(&path, &gold("は", &[(0, 1, None)])).expect("append");

        let (records, _) = read_gold(&path).expect("read");

        assert_eq!(records[0].spans[0].ent_seq, None);
        let line = std::fs::read_to_string(&path).expect("read raw");
        assert!(line.contains("\"ent_seq\":null"), "explicit null: {line}");
    }

    #[test]
    fn an_empty_file_reads_as_no_records() {
        let dir = scratch("empty");
        let path = dir.join("gold.jsonl");
        std::fs::write(&path, "").expect("write");

        assert_eq!(read_gold(&path).expect("read"), (Vec::new(), 0));
    }

    #[test]
    fn a_malformed_line_names_its_line_number() {
        let dir = scratch("malformed");
        let path = dir.join("gold.jsonl");
        std::fs::write(&path, "{\"text\":\"あ\",\"source\":\"t\",\"spans\":[]}\nnot json\n")
            .expect("write");

        let err = read_gold(&path).err().expect("must fail");
        assert!(err.to_string().contains("line 2"), "got {err}");
    }

    #[test]
    fn a_cover_with_a_gap_is_rejected() {
        let err = validate(&gold("あいう", &[(0, 1, None), (2, 1, None)]))
            .err()
            .expect("must fail");
        assert!(err.to_string().contains("あいう"), "names the text: {err}");
    }

    #[test]
    fn overlapping_spans_are_rejected() {
        assert!(validate(&gold("あいう", &[(0, 2, None), (1, 2, None)])).is_err());
    }

    #[test]
    fn a_span_running_past_the_text_is_rejected() {
        assert!(validate(&gold("あ", &[(0, 2, None)])).is_err());
    }

    #[test]
    fn a_cover_stopping_short_of_the_text_is_rejected() {
        assert!(validate(&gold("あい", &[(0, 1, None)])).is_err());
    }

    #[test]
    fn a_zero_length_span_is_rejected() {
        assert!(validate(&gold("あ", &[(0, 0, None), (0, 1, None)])).is_err());
    }

    #[test]
    fn no_spans_at_all_is_rejected_for_a_non_empty_text() {
        assert!(validate(&gold("あ", &[])).is_err());
    }

    /// Char offsets, not bytes: every character here is three UTF-8 bytes, so a
    /// byte-offset implementation passes every ASCII test and fails this one.
    #[test]
    fn a_valid_multibyte_cover_is_accepted() {
        assert!(validate(&gold("言った", &[(0, 1, Some(1)), (1, 2, None)])).is_ok());
    }

    #[test]
    fn a_diff_record_round_trips() {
        let dir = scratch("diff-round-trip");
        let path = dir.join("diff.jsonl");
        let record = DiffRecord {
            text: "くるまではこう".to_string(),
            source: "test:1".to_string(),
            jparser: vec![0, 3, 4, 7],
            vibrato: vec![0, 2, 4, 5, 7],
            conflicts: vec![3],
            unknown_tokens: 0,
        };

        append(&path, &record).expect("append");

        assert_eq!(read_diffs(&path).expect("read"), vec![record]);
    }
}
```

Declare the module in `main.rs`, directly under the `//!` doc comment:

```rust
mod corpus;
```

- [ ] **Step 2: Run the tests to verify they fail**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: FAIL to compile — `cannot find type 'GoldRecord' in this scope`, and the same for every other name.

- [ ] **Step 3: Write the implementation**

Insert above the `#[cfg(test)] mod tests` block in `corpus.rs`:

```rust
use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One labelled span. Offsets are **char** offsets into the record's `text`,
/// matching `jparser::Segment`, so a gold record and a parse result compare
/// directly with no normalisation step between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldSpan {
    pub start: usize,
    pub len: usize,
    /// The JMdict entry this span should resolve to, or `None`.
    ///
    /// `None` is a **positive label**: no JMdict entry belongs first here. A
    /// function word with no useful entry, an unmatched run, or a span the
    /// adjudicator ruled nothing in the dictionary fits. It counts in span
    /// accuracy and is excluded from entry top-1 (design §5.2).
    pub ent_seq: Option<u32>,
}

/// One labelled sentence. `text` is the record's identity for supersession.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoldRecord {
    pub text: String,
    /// `tier:id`, resolved through `fixtures/gold_sources.md`.
    pub source: String,
    /// A contiguous cover of `text` in ascending `start` order — the same
    /// invariant `jparser::ParseResult` documents. Enforced by [`validate`].
    pub spans: Vec<GoldSpan>,
}

/// One sampled disagreement. Disposable: `label` consumes it and the gold
/// corpus is the artefact that survives.
///
/// Boundary offsets rather than spans, because that is what the sampler
/// compares (design §6.1). Both arrays begin at 0 and end at the sentence's
/// char length, so a cover is implied and the record stays short.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffRecord {
    pub text: String,
    pub source: String,
    pub jparser: Vec<usize>,
    pub vibrato: Vec<usize>,
    /// The jparser cuts that are not Vibrato cuts. Non-empty by construction
    /// unless `diff --all` produced the record.
    pub conflicts: Vec<usize>,
    /// Vibrato tokens whose IPADIC reading field is `*`, absent, or empty —
    /// the count, not the positions. It tells the adjudicator how much of the
    /// disagreement is the tokenizer guessing, which is the normal case on the
    /// noise tier and on proper nouns.
    pub unknown_tokens: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum CorpusError {
    #[error("corpus io failed at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path} line {line} is not a valid record: {source}")]
    Json {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    /// A gold record whose spans are not a contiguous cover. A hard error, not
    /// a skip: skipping would move a metric denominator silently, which is the
    /// one thing a baseline may not do (design §12).
    #[error("gold record {text:?} is not a contiguous cover: {detail}")]
    Cover { text: String, detail: String },
}

/// Append one record as a single JSONL line.
///
/// Creates the file if absent. `BufWriter` is flushed explicitly rather than
/// on drop: a drop-flushed writer swallows the error that matters here.
pub fn append<T: Serialize>(path: &Path, record: &T) -> Result<(), CorpusError> {
    let io = |source| CorpusError::Io {
        path: path.to_path_buf(),
        source,
    };
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(io)?;
    let mut out = BufWriter::new(file);
    // `to_string` cannot fail for these types — every field is a primitive,
    // a String, or a Vec of them — so this is not a variant of CorpusError.
    let line = serde_json::to_string(record).expect("a record must serialize");
    writeln!(out, "{line}").map_err(io)?;
    out.flush().map_err(io)
}

/// Read every JSONL record, deserializing each line into `T`.
///
/// A missing file reads as no records: `label` appends to a path that does not
/// exist yet, and requiring the caller to create it first would only move the
/// same branch outward.
fn read_lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, CorpusError> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(CorpusError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| CorpusError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(
            serde_json::from_str(&line).map_err(|source| CorpusError::Json {
                path: path.to_path_buf(),
                line: i + 1,
                source,
            })?,
        );
    }
    Ok(out)
}

/// Every gold record, last-wins by `text`, in first-seen order. The second
/// element is the count of superseded records dropped.
pub fn read_gold(path: &Path) -> Result<(Vec<GoldRecord>, usize), CorpusError> {
    let all: Vec<GoldRecord> = read_lines(path)?;
    // `text` -> index into `out`. A Vec plus a map rather than an IndexMap:
    // first-seen order is the requirement and the crate has no such dependency.
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut out: Vec<GoldRecord> = Vec::new();
    let mut superseded = 0;
    for record in all {
        match seen.get(&record.text) {
            Some(&at) => {
                out[at] = record;
                superseded += 1;
            }
            None => {
                seen.insert(record.text.clone(), out.len());
                out.push(record);
            }
        }
    }
    Ok((out, superseded))
}

pub fn read_diffs(path: &Path) -> Result<Vec<DiffRecord>, CorpusError> {
    read_lines(path)
}

/// The spans must be a contiguous cover of `text` in ascending `start` order:
/// starting at 0, each span beginning exactly where the previous ended, every
/// span at least one char, and the last ending at the text's char length.
pub fn validate(record: &GoldRecord) -> Result<(), CorpusError> {
    let cover = |detail: String| CorpusError::Cover {
        text: record.text.clone(),
        detail,
    };
    let chars = record.text.chars().count();
    let mut at = 0;
    for (i, span) in record.spans.iter().enumerate() {
        if span.len == 0 {
            return Err(cover(format!("span {i} has zero length")));
        }
        if span.start != at {
            return Err(cover(format!(
                "span {i} starts at {} but the previous span ended at {at}",
                span.start
            )));
        }
        at = span.start + span.len;
        if at > chars {
            return Err(cover(format!(
                "span {i} ends at {at}, past the text's {chars} chars"
            )));
        }
    }
    if at != chars {
        return Err(cover(format!(
            "the cover ends at {at} but the text has {chars} chars"
        )));
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: PASS, 14 tests.

If `read_gold`'s tuple comparison in `an_empty_file_reads_as_no_records` fails to compile, `PartialEq` on the tuple needs `GoldRecord: PartialEq`, which the derive already provides — check the derive list rather than changing the test.

- [ ] **Step 5: Run the full gate**

```powershell
cargo test -p jparser
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
```

Clippy will flag an unused-import or dead-code warning on anything `main.rs` does not yet call. Silence it for this task only by adding, directly above `mod corpus;` in `main.rs`:

```rust
// Task 4 wires `score` to these; the module lands first so its formats can be
// tested before anything depends on them.
#[allow(dead_code)]
mod corpus;
```

Remove the `#[allow(dead_code)]` in Task 4 once `score` consumes the module. Leaving it is a plan failure.

- [ ] **Step 6: Commit**

```bash
git add crates/jparser/src/bin/jparser-cli/
git commit -m "feat(harness): add the Phase 0 JSONL corpus formats

Gold and diff record types, an append-only writer, a last-wins reader
that counts supersessions, and contiguous-cover validation.

Append-only with last-wins by text means an interrupted labelling
session loses nothing and a correction is another append. A cover
violation is a hard error: skipping a bad record would move a metric
denominator silently."
```

---

## Task 4: `score`, and the extractor's JSONL sniffing

Produces a working, tested `score` before any corpus work begins. It is proved against a hand-written 10-sentence gold file whose sentences come from the existing `parse_sentences.txt`, so the **existing** `jmdict_subset.xml` already covers the vocabulary and nothing needs regenerating — read-only, per the global constraints.

**Files:**
- Create: `crates/jparser/src/bin/jparser-cli/score.rs`
- Create: `crates/jparser/tests/cli_score.rs`
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (module, `Score` variant, dispatch arm; drop Task 3's `#[allow(dead_code)]`)
- Modify: `crates/jparser/Cargo.toml` (new `[[test]]` entry)
- Modify: `tools/extract_jmdict_subset.py`

**Interfaces:**
- Consumes: `corpus::{GoldRecord, GoldSpan, read_gold, validate}`, `jparser::Entry::entry_id`.
- Produces:
  - `struct Metrics { sentences, gold_spans, superseded, span_hits, top1_hits, top1_total: usize }`
  - `fn accumulate(gold: &GoldRecord, parsed: &jparser::ParseResult, m: &mut Metrics)`
  - `fn render(m: &Metrics) -> String` — the human form, exactly as `cli_score.rs` asserts
  - `fn run(index: &Path, gold: &Path, hints: Option<&Path>, json: bool) -> Result<(), Box<dyn Error>>`
  - the CLI surface `jparser-cli score <index> <gold.jsonl> [--hints <dic>] [--json]`

- [ ] **Step 1: Write the failing unit tests**

Create `crates/jparser/src/bin/jparser-cli/score.rs` with the standard header, this doc comment, and only the test module:

```rust
//! The two Phase 0 metrics, per design §5.2.
//!
//! **Span accuracy** — the fraction of gold spans the parser reproduces
//! exactly. Includes `ent_seq: null` spans: whether a function word is
//! delimited correctly is exactly as much a span question as whether a noun is.
//!
//! **Entry top-1** — of exactly-spanned gold spans with a non-null `ent_seq`,
//! the fraction where the parse ranked that entry first. Conditioned on correct
//! spanning because a wrong span has no meaningful entry list, and on non-null
//! gold because a null label asserts nothing about ranking.
//!
//! Reported separately, always, with numerator and denominator beside each
//! ratio. Never combined: a change that improves one and harms the other is a
//! finding, not noise.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{GoldRecord, GoldSpan};
    use jparser::record::WordFlags;
    use jparser::{Entry, ParseResult, Segment};

    fn gold(text: &str, spans: &[(usize, usize, Option<u32>)]) -> GoldRecord {
        GoldRecord {
            text: text.to_string(),
            source: "test:1".to_string(),
            spans: spans
                .iter()
                .map(|&(start, len, ent_seq)| GoldSpan {
                    start,
                    len,
                    ent_seq,
                })
                .collect(),
        }
    }

    /// A segment carrying `entry_ids` as its ranked candidate list. Only
    /// `start`, `len`, `matched` and `entries[..].entry_id` affect the metrics.
    fn seg(start: usize, len: usize, entry_ids: &[u32]) -> Segment {
        Segment {
            start,
            len,
            surface: String::new(),
            reading: None,
            matched: !entry_ids.is_empty(),
            entries: entry_ids
                .iter()
                .map(|&entry_id| Entry {
                    headword: String::new(),
                    reading: None,
                    conjugation: None,
                    pos: Vec::new(),
                    senses: Vec::new(),
                    flags: WordFlags::default(),
                    entry_id,
                })
                .collect(),
        }
    }

    fn parsed(segments: Vec<Segment>) -> ParseResult {
        ParseResult { segments }
    }

    #[test]
    fn a_perfect_parse_scores_one_on_both_metrics() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あい", &[(0, 1, Some(10)), (1, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[10]), seg(1, 1, &[20])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (2, 2));
        assert_eq!((m.top1_total, m.top1_hits), (2, 2));
        assert_eq!(m.sentences, 1);
    }

    /// One boundary moved costs exactly the two spans it straddles, and takes
    /// them out of entry top-1's denominator with it.
    #[test]
    fn a_moved_boundary_costs_exactly_the_spans_it_straddles() {
        let mut m = Metrics::default();
        accumulate(
            &gold(
                "あいう",
                &[(0, 1, Some(10)), (1, 1, Some(20)), (2, 1, Some(30))],
            ),
            &parsed(vec![seg(0, 2, &[10]), seg(2, 1, &[30])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (3, 1), "only (2,1) survives");
        assert_eq!((m.top1_total, m.top1_hits), (1, 1));
    }

    #[test]
    fn a_null_gold_span_counts_for_span_accuracy_and_not_for_entry_top_1() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あい", &[(0, 1, None), (1, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[99]), seg(1, 1, &[20])]),
            &mut m,
        );

        assert_eq!((m.gold_spans, m.span_hits), (2, 2), "both spans are right");
        assert_eq!(
            (m.top1_total, m.top1_hits),
            (1, 1),
            "the null span is out of both halves of the ratio"
        );
    }

    #[test]
    fn a_correct_span_ranked_second_is_a_top_1_miss() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[10, 20])]),
            &mut m,
        );

        assert_eq!(m.span_hits, 1);
        assert_eq!((m.top1_total, m.top1_hits), (1, 0));
    }

    /// Design §5.2: an exactly-spanned non-null gold span whose segment matched
    /// nothing counts in the denominator and fails the numerator. Excluding it
    /// would flatter the baseline.
    #[test]
    fn an_unmatched_segment_on_a_correct_span_is_a_top_1_miss_not_an_exclusion() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[])]),
            &mut m,
        );

        assert_eq!(m.span_hits, 1, "the span is still correctly delimited");
        assert_eq!((m.top1_total, m.top1_hits), (1, 0));
    }

    #[test]
    fn metrics_accumulate_across_sentences() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(10))]),
            &parsed(vec![seg(0, 1, &[10])]),
            &mut m,
        );
        accumulate(
            &gold("い", &[(0, 1, Some(20))]),
            &parsed(vec![seg(0, 1, &[99])]),
            &mut m,
        );

        assert_eq!(m.sentences, 2);
        assert_eq!((m.gold_spans, m.span_hits), (2, 2));
        assert_eq!((m.top1_total, m.top1_hits), (2, 1));
    }

    #[test]
    fn the_rendered_form_carries_both_ratios_with_their_fractions() {
        let m = Metrics {
            sentences: 250,
            gold_spans: 2481,
            superseded: 12,
            span_hits: 2167,
            top1_hits: 1502,
            top1_total: 1898,
        };

        assert_eq!(
            render(&m),
            "sentences:      250\n\
             gold spans:     2481\n\
             superseded:     12\n\
             span accuracy:  0.8734  (2167 / 2481)\n\
             entry top-1:    0.7912  (1502 / 1898)\n"
        );
    }

    /// An all-null corpus has no entry top-1 denominator. `n/a`, not a divide
    /// by zero and not a misleading 0.0000.
    #[test]
    fn an_empty_denominator_renders_as_not_applicable() {
        let m = Metrics {
            sentences: 1,
            gold_spans: 1,
            superseded: 0,
            span_hits: 1,
            top1_hits: 0,
            top1_total: 0,
        };

        assert!(
            render(&m).contains("entry top-1:    n/a     (0 / 0)"),
            "got {}",
            render(&m)
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: FAIL to compile — `cannot find type 'Metrics'`, `cannot find function 'accumulate'`, `cannot find function 'render'`.

- [ ] **Step 3: Write the implementation**

Insert above the test module in `score.rs`:

```rust
use std::error::Error;
use std::path::Path;

use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{GoldOrNothing as _, ParseOptions, ParseResult};

use crate::corpus::{read_gold, validate, GoldRecord};

/// Field width for the label column in [`render`]. `span accuracy:` is the
/// longest label at 14 chars; 16 leaves two spaces after it.
const LABEL_WIDTH: usize = 16;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Metrics {
    pub sentences: usize,
    /// Span accuracy's denominator: every gold span, `null` ones included.
    pub gold_spans: usize,
    /// Records dropped as superseded by `read_gold`. Reported because a file
    /// that is a third supersessions is a signal about the labelling pass.
    pub superseded: usize,
    pub span_hits: usize,
    pub top1_hits: usize,
    pub top1_total: usize,
}

/// Fold one sentence's gold and parse into `m`.
pub fn accumulate(gold: &GoldRecord, parsed: &ParseResult, m: &mut Metrics) {
    m.sentences += 1;
    for span in &gold.spans {
        m.gold_spans += 1;
        let Some(seg) = parsed
            .segments
            .iter()
            .find(|s| s.start == span.start && s.len == span.len)
        else {
            continue;
        };
        m.span_hits += 1;
        // A null gold span asserts nothing about ranking, so it leaves both
        // halves of entry top-1 untouched.
        let Some(want) = span.ent_seq else { continue };
        m.top1_total += 1;
        // `entries.first()` is None for an unmatched span, which is a miss
        // rather than an exclusion — design §5.2.
        if seg.entries.first().map(|e| e.entry_id) == Some(want) {
            m.top1_hits += 1;
        }
    }
}

/// `num / den` to four places, with the fraction beside it. `n/a` padded to the
/// same eight columns a ratio occupies, so the fractions stay aligned.
fn ratio(num: usize, den: usize) -> String {
    if den == 0 {
        format!("n/a     ({num} / {den})")
    } else {
        format!("{:.4}  ({num} / {den})", num as f64 / den as f64)
    }
}

/// The human form. A frozen interface: `cli_score.rs` asserts it exactly, and
/// the recorded baseline is transcribed from it.
pub fn render(m: &Metrics) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<LABEL_WIDTH$}{}\n",
        "sentences:", m.sentences
    ));
    out.push_str(&format!(
        "{:<LABEL_WIDTH$}{}\n",
        "gold spans:", m.gold_spans
    ));
    out.push_str(&format!(
        "{:<LABEL_WIDTH$}{}\n",
        "superseded:", m.superseded
    ));
    out.push_str(&format!(
        "{:<LABEL_WIDTH$}{}\n",
        "span accuracy:",
        ratio(m.span_hits, m.gold_spans)
    ));
    out.push_str(&format!(
        "{:<LABEL_WIDTH$}{}\n",
        "entry top-1:",
        ratio(m.top1_hits, m.top1_total)
    ));
    out
}
```

Then the driver, in the same file:

```rust
/// Score `gold` against the engine backed by `index`.
///
/// `hints` optional: this is where the hints-on and hints-off baseline pair is
/// produced (master plan §5.4). Nothing here touches the network, and with
/// `hints` absent nothing loads a tokenizer — which is what keeps the offline
/// gate free of the 46 MB dictionary.
pub fn run(
    index_dir: &Path,
    gold_path: &Path,
    #[cfg(feature = "mecab")] hints: Option<&Path>,
    json: bool,
) -> Result<(), Box<dyn Error>> {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(index_dir)?;
    let (records, superseded) = read_gold(gold_path)?;

    #[cfg(feature = "mecab")]
    let tokenizer = match hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
        None => None,
    };

    let mut m = Metrics {
        superseded,
        ..Metrics::default()
    };
    for record in &records {
        validate(record)?;
        #[cfg(feature = "mecab")]
        let flags = tokenizer.as_ref().map(|t| t.hints(&record.text));
        #[cfg(feature = "mecab")]
        let hints: Option<&dyn jparser::BoundaryHints> =
            flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
        #[cfg(not(feature = "mecab"))]
        let hints: Option<&dyn jparser::BoundaryHints> = None;

        let parsed = jparser::parse(&index, &table, &record.text, &ParseOptions::default(), hints)?;
        accumulate(record, &parsed, &mut m);
    }

    if json {
        println!("{}", serde_json::to_string(&m)?);
    } else {
        print!("{}", render(&m));
    }
    Ok(())
}
```

Remove the bogus `use jparser::{GoldOrNothing as _, ...}` line if you copied it — the import list is:

```rust
use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{ParseOptions, ParseResult};
```

In `main.rs`: drop Task 3's `#[allow(dead_code)]` and its comment, add `mod score;` beside `mod corpus;`, add the subcommand variant to `enum Command`:

```rust
    /// Score a gold corpus against the current engine: span accuracy and
    /// entry top-1, reported separately.
    Score {
        /// Index directory.
        index: PathBuf,
        /// Gold corpus JSONL.
        gold: PathBuf,
        /// Path to an uncompressed compiled Vibrato dictionary. Given, the
        /// hints-on half of the baseline pair is measured; absent, hints-off.
        #[cfg(feature = "mecab")]
        #[arg(long)]
        hints: Option<PathBuf>,
        /// Emit the metrics as one JSON object instead of the human form.
        #[arg(long)]
        json: bool,
    },
```

and the dispatch arm:

```rust
        Command::Score {
            index,
            gold,
            #[cfg(feature = "mecab")]
            hints,
            json,
        } => score::run(
            &index,
            &gold,
            #[cfg(feature = "mecab")]
            hints.as_deref(),
            json,
        )?,
```

- [ ] **Step 4: Run the unit tests to verify they pass**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: PASS, 22 tests (Task 3's 14 plus these 8).

- [ ] **Step 5: Write the failing integration test**

Create `crates/jparser/tests/cli_score.rs`, with the standard new-file header:

```rust
//! `jparser-cli score` end to end against the hand-written mini fixture.
//!
//! Exact-output assertions: the format is a frozen interface — the recorded
//! Phase 0 baseline is transcribed from it — so a change must break a test
//! rather than silently change what a later phase is compared against.

use std::path::{Path, PathBuf};
use std::process::Command;

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::stem::StemOptions;

const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");
const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn index_dir(dir: &Path) -> PathBuf {
    let index = dir.join("index");
    std::fs::create_dir_all(&index).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &index,
    )
    .expect("the mini fixture must build");
    index
}

fn score(index: &Path, gold: &Path) -> String {
    let out = Command::new(BIN)
        .arg("score")
        .arg(index)
        .arg(gold)
        .output()
        .expect("jparser-cli must be runnable");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("output must be UTF-8")
}

/// 言う is ent_seq 1000010 and は is 1000020 in the mini fixture, and
/// `cli_parse.rs` already pins that 言った parses as one span over all three
/// chars. Both metrics are therefore 1.0 by construction, which is what makes
/// this a test of the harness rather than of the engine.
#[test]
fn a_fully_correct_gold_corpus_scores_one_on_both_metrics() {
    let dir = scratch("cli-score-perfect");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}\n\
         {\"text\":\"は\",\"source\":\"test:2\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":1000020}]}\n",
    )
    .unwrap();

    assert_eq!(
        score(&index, &gold),
        "sentences:      2\n\
         gold spans:     2\n\
         superseded:     0\n\
         span accuracy:  1.0000  (2 / 2)\n\
         entry top-1:    1.0000  (2 / 2)\n"
    );
}

/// A wrong ent_seq must cost entry top-1 and leave span accuracy alone. If a
/// future change conflated the two metrics, only this test would catch it.
#[test]
fn a_wrong_ent_seq_costs_only_entry_top_1() {
    let dir = scratch("cli-score-wrong-entry");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":9999999}]}\n",
    )
    .unwrap();

    assert_eq!(
        score(&index, &gold),
        "sentences:      1\n\
         gold spans:     1\n\
         superseded:     0\n\
         span accuracy:  1.0000  (1 / 1)\n\
         entry top-1:    0.0000  (0 / 1)\n"
    );
}

/// A cover violation is a hard error naming the record, not a skipped
/// sentence: skipping would move span accuracy's denominator silently.
#[test]
fn a_gold_span_past_the_end_of_its_text_is_a_hard_error() {
    let dir = scratch("cli-score-bad-cover");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":4,\"ent_seq\":null}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .output()
        .expect("runnable");

    assert!(!out.status.success(), "must exit non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("は"), "names the record: {stderr}");
}

#[test]
fn the_json_form_carries_the_same_numbers() {
    let dir = scratch("cli-score-json");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"は\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":1,\"ent_seq\":1000020}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .arg("--json")
        .output()
        .expect("runnable");
    assert!(out.status.success());

    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        "{\"sentences\":1,\"gold_spans\":1,\"superseded\":0,\"span_hits\":1,\"top1_hits\":1,\"top1_total\":1}"
    );
}
```

Register it in `crates/jparser/Cargo.toml`:

```toml
[[test]]
name = "cli_score"
path = "tests/cli_score.rs"
required-features = ["cli"]
```

- [ ] **Step 6: Run the integration test**

```powershell
cargo test -p jparser --test cli_score
```

Expected: PASS, 4 tests. If `a_fully_correct_gold_corpus_scores_one_on_both_metrics` fails on the は record, run `cargo run -p jparser --bin jparser-cli -- parse <index> は` and read the actual span and top entry — the mini fixture is small enough that the real answer is unambiguous, and the gold line in the test is what should change, never the metric.

- [ ] **Step 7: Teach the extractor to read JSONL**

In `tools/extract_jmdict_subset.py`, replace the sentence-reading block inside `main()`:

```python
    sentences = [
        line.strip()
        for line in Path(sentences_path).read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
```

with a call to a new function, defined above `main()`:

```python
def read_corpus(path):
    """Sentences from a plain-text or JSONL corpus, deduplicated in order.

    `.txt` is one sentence per line, '#' comments and blanks ignored — the
    parse_sentences.txt format. `.jsonl` reads each record's "text" field, so
    the gold corpus is its own single source of truth and no parallel sentence
    list has to be kept in sync with it.

    Duplicates are dropped rather than rejected: the substring closure is a
    set, so a repeated sentence would only widen it redundantly.
    """
    p = Path(path)
    raw = p.read_text(encoding="utf-8")
    if p.suffix == ".jsonl":
        texts = [
            json.loads(line)["text"] for line in raw.splitlines() if line.strip()
        ]
    else:
        texts = [
            s
            for s in (line.strip() for line in raw.splitlines())
            if s and not s.startswith("#")
        ]
    return list(dict.fromkeys(texts))
```

and in `main()`:

```python
    sentences = read_corpus(sentences_path)
```

`json` is already imported. Update the module docstring's usage block to show the `.jsonl` form beside the `.txt` one.

- [ ] **Step 8: Verify the extractor change**

Write a throwaway 3-record JSONL to the scratch directory and confirm the printed sentence count, which is the extractor's own reported check:

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0"
New-Item -ItemType Directory -Force $scratch | Out-Null
@'
{"text":"猫が好きです。","source":"t:1","spans":[]}
{"text":"本を読みました。","source":"t:2","spans":[]}
{"text":"猫が好きです。","source":"t:3","spans":[]}
'@ | Out-File -Encoding utf8 "$scratch\probe.jsonl"
python tools/extract_jmdict_subset.py `
  "$env:APPDATA\dev.jparser.clipyomi\source\JMdict_e.xml" `
  "$scratch\probe.jsonl" `
  crates/jparser/assets/conjugations.json `
  "$scratch\probe_subset.xml"
```

Expected: `sentences:     2` — three records, one duplicate dropped.

`JMdict_e.xml` is not on disk uncompressed; `%APPDATA%\dev.jparser.clipyomi\source\` holds `JMdict_e.gz`. Expand it into the scratch directory first and point the command at that:

```powershell
$gz = "$env:APPDATA\dev.jparser.clipyomi\source\JMdict_e.gz"
$xml = "$scratch\JMdict_e.xml"
$in = [System.IO.File]::OpenRead($gz)
$gzs = New-Object System.IO.Compression.GzipStream($in, [System.IO.Compression.CompressionMode]::Decompress)
$out = [System.IO.File]::Create($xml)
$gzs.CopyTo($out); $out.Close(); $gzs.Close(); $in.Close()
```

Keep `$xml` — Task 8 needs it again. It is in the scratch directory, not the repo.

- [ ] **Step 9: Run the full gate**

```powershell
cargo test -p jparser
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
```

Expected: all green. `--no-default-features` proves `score` did not drag the tokenizer into the library build.

- [ ] **Step 10: Commit**

```bash
git add crates/jparser/Cargo.toml crates/jparser/src/bin/jparser-cli/ crates/jparser/tests/cli_score.rs tools/extract_jmdict_subset.py
git commit -m "feat(harness): add jparser-cli score

Span accuracy and entry top-1, reported separately with numerator and
denominator beside each ratio, so a corpus change shows up as a
denominator change rather than as unexplained metric movement.

Null gold spans count for span accuracy and are excluded from entry
top-1; an exactly-spanned non-null gold span that matched nothing is a
top-1 miss, not an exclusion. A cover violation is a hard error.

The subset extractor now reads JSONL corpora, so the gold corpus is its
own source of truth with no parallel sentence list to keep in sync."
```

---

## Task 5: `diff` — the disagreement sampler

The predicate is the whole point of this task, and it is one-directional on purpose. jparser emits JMdict long units, Vibrato emits IPADIC short-unit morphemes, and they disagree on most sentences legitimately — a sampler keyed on set inequality selects nearly the whole pool and makes the same mistake master plan §5.2 rejects boundary F1 for. Sample where jparser has a cut Vibrato does not.

**Files:**
- Create: `crates/jparser/src/bin/jparser-cli/diff.rs`
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (module, `Diff` variant, dispatch arm)

**Interfaces:**
- Consumes: `corpus::{DiffRecord, append}`, `jparser::Segment`.
- Produces:
  - `fn boundaries_from_segments(segments: &[jparser::Segment]) -> Vec<usize>`
  - `fn conflicts(jparser: &[usize], vibrato: &[usize]) -> Vec<usize>`
  - `fn read_pool(path: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>>` — `(source, text)` pairs from a TSV pool
  - `fn run(index: &Path, pool: &Path, hints: &Path, all: bool, limit: Option<usize>) -> Result<(), Box<dyn Error>>`
  - the CLI surface `jparser-cli diff <index> <pool.tsv> --hints <dic> [--all] [--limit N]`
  - the **pool file format**: TSV, `source \t text [\t speaker]`, `#` comments and blanks skipped, extra columns ignored. Task 6's `extract_vn_pool.py` writes it.

- [ ] **Step 1: Write the failing unit tests**

Create `crates/jparser/src/bin/jparser-cli/diff.rs` with the standard header, this doc comment, and only the test module:

```rust
//! The disagreement sampler, per design §6.
//!
//! The obvious predicate — "the boundary sets differ" — is unusable. jparser
//! emits JMdict long units and Vibrato emits IPADIC short units, so `東京都`
//! is one span against two tokens and every compound in the language would be
//! flagged. That is the same mistake master plan §5.2 rejects boundary F1 for:
//! it punishes compound preservation, which is a feature.
//!
//! The usable predicate is one-directional. With *Bj* jparser's cut offsets and
//! *Bv* Vibrato's, sample the sentence when **Bj \ Bv is non-empty** — jparser
//! has at least one cut that is not a Vibrato cut. Coarsening (jparser merged
//! whole tokens) is excluded; an interior split and a crossing are both kept.

#[cfg(test)]
mod tests {
    use super::*;

    /// `東京都`: Vibrato splits 東京|都, jparser keeps the JMdict headword
    /// whole. Every jparser cut is a Vibrato cut, so this is coarsening and
    /// must not be sampled — it is the bulk of the pool.
    #[test]
    fn coarsening_is_not_a_conflict() {
        assert_eq!(conflicts(&[0, 3], &[0, 2, 3]), Vec::<usize>::new());
    }

    /// The case master plan §5.1 says the sampler must surface. From §4.3's
    /// recorded flags, Vibrato reads `くる|まで|は|こう`; jparser produces
    /// `くるま|で|はこう`. The cut at 3 sits strictly inside Vibrato's まで.
    #[test]
    fn the_kuruma_dewa_kou_case_is_sampled() {
        assert_eq!(conflicts(&[0, 3, 4, 7], &[0, 2, 4, 5, 7]), vec![3]);
    }

    /// jparser cutting inside a token Vibrato recognised — the failure the
    /// hint mechanism exists to prevent, so a sample of it is worth labelling.
    #[test]
    fn an_interior_split_is_a_conflict() {
        assert_eq!(conflicts(&[0, 1, 2], &[0, 2]), vec![1]);
    }

    #[test]
    fn identical_boundaries_are_not_a_conflict() {
        assert_eq!(conflicts(&[0, 2, 4], &[0, 2, 4]), Vec::<usize>::new());
    }

    /// Neither set contains the other. Both offending cuts are reported, so the
    /// output can be ranked by conflict density.
    #[test]
    fn a_crossing_reports_every_offending_cut() {
        assert_eq!(conflicts(&[0, 2, 5, 6], &[0, 3, 4, 6]), vec![2, 5]);
    }

    #[test]
    fn boundaries_come_from_the_segment_cover() {
        let segs = vec![span(0, 2), span(2, 1), span(3, 4)];
        assert_eq!(boundaries_from_segments(&segs), vec![0, 2, 3, 7]);
    }

    #[test]
    fn an_empty_segment_list_has_no_boundaries() {
        assert_eq!(boundaries_from_segments(&[]), Vec::<usize>::new());
    }

    #[test]
    fn a_pool_line_splits_on_tabs_and_ignores_extra_columns() {
        let dir = std::env::temp_dir().join("jparser-diff-test-pool");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pool.tsv");
        std::fs::write(
            &path,
            "# a comment\n\
             c2:a\tああ\n\
             \n\
             c2:b\tいい\tテスト太郎\n",
        )
        .unwrap();

        assert_eq!(
            read_pool(&path).unwrap(),
            vec![
                ("c2:a".to_string(), "ああ".to_string()),
                ("c2:b".to_string(), "いい".to_string()),
            ]
        );
    }

    /// Only `start` and `len` matter to boundary extraction.
    fn span(start: usize, len: usize) -> jparser::Segment {
        jparser::Segment {
            start,
            len,
            surface: String::new(),
            reading: None,
            matched: false,
            entries: Vec::new(),
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```powershell
cargo test -p jparser --bin jparser-cli --features mecab
```

Expected: FAIL to compile — `cannot find function 'conflicts'`, `boundaries_from_segments`, `read_pool`.

- [ ] **Step 3: Write the implementation**

Insert above the test module in `diff.rs`:

```rust
use std::error::Error;
use std::path::Path;

use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{ParseOptions, Segment};

use crate::corpus::{append, DiffRecord};

/// Index of the reading in an IPADIC feature string, for the unknown-token
/// count. Deliberately duplicated from `hints.rs`' private constant rather
/// than made public: `hints.rs` is out of scope for Phase 0, and one integer
/// is a smaller cost than an API change to the parser library.
const READING_FIELD: usize = 7;

/// The cut offsets of a segment cover, including 0 and the text's char length.
///
/// `ParseResult::segments` is documented as a contiguous cover in ascending
/// `start` order, so the starts plus the final end are exactly the cuts.
pub fn boundaries_from_segments(segments: &[Segment]) -> Vec<usize> {
    let Some(last) = segments.last() else {
        return Vec::new();
    };
    segments
        .iter()
        .map(|s| s.start)
        .chain(std::iter::once(last.start + last.len))
        .collect()
}

/// The jparser cuts that are not Vibrato cuts — `Bj \ Bv`, in ascending order.
///
/// Empty means jparser's cover is a coarsening of Vibrato's: it merged whole
/// tokens and split nothing the tokenizer had joined. That is JMdict compound
/// preservation and is not a conflict. Non-empty means jparser cut inside a
/// token Vibrato recognised, which is either an interior split or a crossing,
/// and both are worth a human's time.
///
/// `contains` on a slice rather than a `HashSet`: these are single-digit
/// lengths and the allocation would cost more than the scan.
pub fn conflicts(jparser: &[usize], vibrato: &[usize]) -> Vec<usize> {
    jparser
        .iter()
        .copied()
        .filter(|cut| !vibrato.contains(cut))
        .collect()
}

/// `(source, text)` pairs from a TSV pool.
///
/// Format: `source \t text [\t speaker]`. Blank lines and lines starting with
/// `#` are skipped; columns past the second are ignored, so
/// `extract_vn_pool.py`'s separated speaker column travels with the pool
/// without diff having to know about it.
pub fn read_pool(path: &Path) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let raw = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.split('\t');
        let (Some(source), Some(text)) = (cols.next(), cols.next()) else {
            return Err(format!(
                "{} line {}: expected 'source<TAB>text'",
                path.display(),
                i + 1
            )
            .into());
        };
        out.push((source.to_string(), text.to_string()));
    }
    Ok(out)
}

/// Tokenize `text` and return its cut offsets plus the count of tokens whose
/// IPADIC reading field is `*`, absent, or empty.
///
/// Deliberately **not** `VibratoTokenizer::hints`. `BoundaryFlags` stores only
/// interior-position booleans, so token boundaries are not recoverable from it,
/// and `flags_from_worker` skips reading-less tokens — the right call for hints
/// and the wrong one for sampling, because a sentence the tokenizer recognised
/// nothing in is a sentence worth looking at.
fn tokenize(tokenizer: &vibrato::Tokenizer, text: &str) -> (Vec<usize>, usize) {
    let mut worker = tokenizer.new_worker();
    worker.reset_sentence(text);
    worker.tokenize();

    let mut cuts = Vec::new();
    let mut unknown = 0;
    let mut end = 0;
    for i in 0..worker.num_tokens() {
        let token = worker.token(i);
        let reading = token.feature().split(',').nth(READING_FIELD);
        if !matches!(reading, Some(r) if !r.is_empty() && r != "*") {
            unknown += 1;
        }
        let range = token.range_char();
        cuts.push(range.start);
        end = range.end;
    }
    if !cuts.is_empty() {
        cuts.push(end);
    }
    (cuts, unknown)
}

/// Emit the pool sentences worth labelling, as JSONL on stdout.
///
/// Ranked by conflict count descending, so `--limit N` is a stopping point
/// rather than an arbitrary truncation. `--all` drops the predicate and emits
/// every sentence whose boundary sets differ at all — kept only to verify by
/// hand that the predicate is not filtering out real conflicts (design §11.2).
pub fn run(
    index_dir: &Path,
    pool: &Path,
    hints: &Path,
    all: bool,
    limit: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(index_dir)?;
    let tokenizer = jparser::hints::VibratoTokenizer::load(hints)?;
    // `run` needs the raw tokenizer, and `VibratoTokenizer` does not expose
    // one, so load the dictionary a second time for tokenization. Two loads of
    // a 46 MB memory-mapped file at startup of a corpus-construction tool is
    // cheaper than an API change to the parser library, which is out of scope.
    let raw = vibrato::Tokenizer::new(vibrato::Dictionary::read(std::io::BufReader::new(
        std::fs::File::open(hints)?,
    ))?);
    // Keep `tokenizer` alive only for its load-error message, which names the
    // file; drop it before the loop so nothing holds two dictionaries.
    drop(tokenizer);

    let mut records = Vec::new();
    for (source, text) in read_pool(pool)? {
        let parsed = jparser::parse(&index, &table, &text, &ParseOptions::default(), None)?;
        let j = boundaries_from_segments(&parsed.segments);
        let (v, unknown_tokens) = tokenize(&raw, &text);
        let c = conflicts(&j, &v);
        let interesting = if all { j != v } else { !c.is_empty() };
        if !interesting {
            continue;
        }
        records.push(DiffRecord {
            text,
            source,
            jparser: j,
            vibrato: v,
            conflicts: c,
            unknown_tokens,
        });
    }

    // Descending by conflict count, then by text for a deterministic order —
    // two sentences with the same count must not swap between runs.
    records.sort_by(|a, b| {
        b.conflicts
            .len()
            .cmp(&a.conflicts.len())
            .then_with(|| a.text.cmp(&b.text))
    });
    if let Some(n) = limit {
        let dropped = records.len().saturating_sub(n);
        records.truncate(n);
        if dropped > 0 {
            eprintln!("note: --limit {n} dropped {dropped} further disagreements");
        }
    }

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for record in &records {
        use std::io::Write;
        writeln!(out, "{}", serde_json::to_string(record)?)?;
    }
    Ok(())
}
```

`append` is imported but unused by `run`, which writes to stdout — drop `append` from the `use crate::corpus::...` line, leaving `use crate::corpus::DiffRecord;`.

In `main.rs`, add `#[cfg(feature = "mecab")] mod diff;`, the variant:

```rust
    /// Emit the pool sentences where jparser has a segmentation cut Vibrato
    /// does not — the gold corpus's sampling signal.
    ///
    /// `--hints` is required: Vibrato is the second segmenter, so there is
    /// nothing to diff against without it.
    #[cfg(feature = "mecab")]
    Diff {
        /// Index directory.
        index: PathBuf,
        /// TSV pool: `source<TAB>text[<TAB>speaker]`.
        pool: PathBuf,
        /// Path to an uncompressed compiled Vibrato dictionary.
        #[arg(long)]
        hints: PathBuf,
        /// Emit every sentence whose boundary sets differ at all, not only
        /// those where jparser cut inside a Vibrato token. For verifying the
        /// predicate by hand; not for sampling.
        #[arg(long)]
        all: bool,
        /// Keep only the N densest disagreements. What was dropped is reported
        /// on stderr.
        #[arg(long)]
        limit: Option<usize>,
    },
```

and the dispatch arm:

```rust
        #[cfg(feature = "mecab")]
        Command::Diff {
            index,
            pool,
            hints,
            all,
            limit,
        } => diff::run(&index, &pool, &hints, all, limit)?,
```

- [ ] **Step 4: Run the tests to verify they pass**

```powershell
cargo test -p jparser --bin jparser-cli --features mecab
```

Expected: PASS, 30 tests.

- [ ] **Step 5: Smoke-test against the real dictionary**

The full index and the Vibrato dictionary are already on disk. Build a five-line pool by hand, including the case the sampler must surface:

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0"
@"
probe:1`t東京都に住んでいる。
probe:2`tくるまではこう
probe:3`t猫が好きです。
probe:4`t彼はまだ来ていない。
probe:5`tパソコンが壊れてしまった。
"@ | Out-File -Encoding utf8 "$scratch\probe_pool.tsv"

cargo run -p jparser --features mecab --bin jparser-cli -- `
  diff "$env:APPDATA\dev.jparser.clipyomi\dict\gen-1" `
       "$scratch\probe_pool.tsv" `
       --hints "$env:APPDATA\dev.jparser.clipyomi\hints\current\system.dic"
```

Expected: `probe:2` appears in the output with a non-empty `conflicts`. Expected: `probe:1` does **not** appear — `東京都` is coarsening. If `probe:1` appears, read its `jparser` and `vibrato` arrays before changing anything: jparser may genuinely be cutting inside a token there, which is a real finding and not a predicate bug.

Record the observed output in the commit message. This is the first evidence the sampler works on real data rather than on constructed sets.

- [ ] **Step 6: Run the full gate**

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
cargo clippy -p jparser --features mecab -- -D warnings
```

Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/src/bin/jparser-cli/
git commit -m "feat(harness): add jparser-cli diff

Samples the sentences worth labelling. The predicate is one-directional
on purpose: sample where jparser has a cut Vibrato does not. Boundary
set inequality would flag every JMdict compound -- 東京都 against
東京|都 -- and select nearly the whole pool, which is the same mistake
master plan §5.2 rejects boundary F1 for.

Verified on constructed sets for all four cases, and on the real
dictionary: くるまではこう is sampled, 東京都に住んでいる is not.

Tokenizes through vibrato directly rather than through
VibratoTokenizer::hints, which stores only interior booleans and skips
reading-less tokens. hints.rs is untouched."
```

---

## Task 6: The `vn_data` pool, and the tier C1 coverage intent

`vn_data/` holds 67,469 real visual-novel dialogue lines across two LunaTranslator caches. The text is copyrighted and never committed. It is used as the disagreement pool that tells us which register classes to hand-write tier C1 against — register and style are not copyrightable; transcribed lines are.

**Files:**
- Create: `tools/extract_vn_pool.py`
- Create: `crates/jparser/tests/fixtures/gold_sources.md` (header and the tier C1 coverage intent; Task 8 fills the per-sentence rows)
- Modify: `.gitignore`

**Interfaces:**
- Consumes: `diff`'s pool format from Task 5 — TSV, `source \t text \t speaker`.
- Produces: `vn_data/pool.tsv` (uncommitted), and `gold_sources.md`'s tier C1 coverage-intent list, which Task 8 writes sentences against.

- [ ] **Step 1: Write the extractor**

Create `tools/extract_vn_pool.py`:

```python
#!/usr/bin/env python3
# ClipYomi — Engine v2 Phase 0 measurement harness.
# Copyright (C) 2026
#
# This program is free software; you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 2 as published
# by the Free Software Foundation.
#
# Written independently for the Engine v2 measurement harness. Nothing here
# derives from Translation Aggregator.
"""Extract a sampling pool from LunaTranslator capture caches.

Usage:
    python3 tools/extract_vn_pool.py vn_data vn_data/pool.tsv

Reads every *.sqlite in the input directory. The schema is
`artificialtrans(id, source, machineTrans, origin)`; only `source` is read —
`origin` stores each line doubled and is unusable.

Output is `source_id<TAB>text<TAB>speaker` TSV, which is what
`jparser-cli diff` reads. The speaker name is separated out rather than
dropped: a captured line often carries one, and it belongs in the noise tier
rather than in every dialogue line's parse.

The captured text is copyrighted. The pool is NEVER committed --- `vn_data/`
is in .gitignore --- and its only committed product is which register classes
the disagreement sampler surfaced. Register and style are not copyrightable;
transcribed lines are.
"""

import re
import sqlite3
import sys
from pathlib import Path

# A speaker prefix: a name, then an opening quote. Captures split on the quote
# so the quote itself stays with the text, where the parser will see it.
SPEAKER_RE = re.compile(r"^([^「『（\s]{1,12})(?=[「『])")

# Below this a line carries no segmentation signal; above it, a capture has
# almost always concatenated several lines and the sentence is not a sentence.
MIN_CHARS = 6
MAX_CHARS = 120


def lines(db_path):
    """Every non-empty `source` value in one cache, in row order."""
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        for (text,) in conn.execute(
            "select source from artificialtrans where source is not null"
        ):
            yield text
    finally:
        conn.close()


def clean(text):
    """One captured line to `(text, speaker)`, or None to drop it.

    CRLF and the ideographic space U+3000 are normalised out of the *line
    structure* only: a capture that spans two script lines is not one sentence,
    and an interior U+3000 is an indent artefact rather than a word boundary.
    Both survive elsewhere as deliberate noise-tier artefacts (design §3.1),
    which is why the noise tier is hand-derived rather than sampled from here.
    """
    text = text.replace("\r", "").replace("\n", "").strip()
    text = text.strip("\u3000")
    if not text:
        return None
    speaker = ""
    m = SPEAKER_RE.match(text)
    if m:
        speaker = m.group(1)
        text = text[len(speaker):]
    if not (MIN_CHARS <= len(text) <= MAX_CHARS):
        return None
    if "\t" in text:
        return None
    return text, speaker


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    in_dir, out_path = Path(argv[1]), Path(argv[2])

    dbs = sorted(in_dir.glob("*.sqlite"))
    if not dbs:
        print(f"no *.sqlite in {in_dir}", file=sys.stderr)
        return 1

    seen = {}
    read = 0
    for db in dbs:
        stem = db.name.split("_")[0]
        for i, raw in enumerate(lines(db)):
            read += 1
            cleaned = clean(raw)
            if cleaned is None:
                continue
            text, speaker = cleaned
            # First occurrence wins, so the source id is stable across runs.
            seen.setdefault(text, (f"c2:{stem}-{i}", speaker))

    with out_path.open("w", encoding="utf-8", newline="\n") as out:
        out.write("# Sampling pool from vn_data. NEVER COMMIT THIS FILE.\n")
        out.write("# source<TAB>text<TAB>speaker\n")
        for text, (source, speaker) in seen.items():
            out.write(f"{source}\t{text}\t{speaker}\n")

    print(f"caches:  {len(dbs)}")
    print(f"read:    {read}")
    print(f"kept:    {len(seen)}")
    print(f"dropped: {read - len(seen)}")
    return 0


def demo():
    """Self-check for `clean`, the only branching logic here."""
    assert clean("テスト太郎「これはためしの文です」") == (
        "「いや、それ以前に、こんなの考えたこともないか?」",
        "テスト太郎",
    )
    assert clean("「ここにもためしの文があります」") == ("「ここにもためしの文があります」", "")
    assert clean("短い") is None, "under MIN_CHARS"
    assert clean("あ" * 200) is None, "over MAX_CHARS"
    assert clean("  \u3000 ") is None, "whitespace only"
    assert clean("前半です。\r\n後半です。") == ("前半です。後半です。", "")
    print("demo: ok")


if __name__ == "__main__":
    if sys.argv[1:2] == ["--demo"]:
        demo()
    else:
        sys.exit(main(sys.argv))
```

- [ ] **Step 2: Run the self-check**

```powershell
python tools/extract_vn_pool.py --demo
```

Expected: `demo: ok`.

If the first assertion fails, print what `clean` actually returned before adjusting `SPEAKER_RE` — the expected value is taken from a real row in `riruka_*.sqlite` and is the specification, not a guess.

- [ ] **Step 3: Build the pool**

```powershell
python tools/extract_vn_pool.py vn_data vn_data/pool.tsv
```

Expected: `caches: 2`, `read: 67469`, and a `kept` count in the tens of thousands. Record the four numbers; they go in the Task 9 handoff as the pool's provenance.

- [ ] **Step 4: Gitignore the pool and the local gold file**

Append to `.gitignore`, under the existing `vn_data/` line:

```gitignore
# Phase 0: the local gold corpus over vn_data lines. Copyrighted text, scored
# by the same `jparser-cli score` as the committed corpus but never committed
# (Phase 0 design §3.2). The committed corpus is the reproducible contract.
crates/jparser/tests/fixtures/gold_local.jsonl
```

`vn_data/pool.tsv` needs no entry — `vn_data/` already covers it.

- [ ] **Step 5: Sample the pool and read the register classes**

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  diff "$app\dict\gen-1" vn_data/pool.tsv --hints "$app\hints\current\system.dic" `
  > "$scratch\vn_disagreements.jsonl"
```

`--release`: this is tens of thousands of parses and a debug build makes it a coffee break rather than a minute.

Read the top 100 records. For each, note *why* jparser cut inside a Vibrato token. Group the reasons into register classes — expect contracted verb forms, sentence-final particle stacks, elongation marks, interjections, and kana-only stretches with no kanji anchor, but **write down what is actually there**, not what this plan expects.

- [ ] **Step 6: Write the manifest header and the tier C1 coverage intent**

Create `crates/jparser/tests/fixtures/gold_sources.md`:

```markdown
# Gold corpus provenance

Companion to `gold.jsonl`. One row per `source` id. Nothing parses this file; a
reviewer reads it, and it carries the attribution tiers B and D legally require.

Written for [Engine v2 Phase 0](../../../../docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md) §4.4.

## Tiers

| Tier | Source | Licence |
| --- | --- | --- |
| A | Aozora Bunko, 新字新仮名 editions only, max 8 sentences per work | Public domain |
| B | ja.Wikinews / ja.Wikipedia | CC BY 2.5 / CC BY-SA 4.0 |
| C1 | Hand-written pastiche | GPL-2.0-only, with the crate |
| D | Noise derived from A–C1 | Follows its source line |

Tier C2 (real `vn_data` lines) is never committed and has no rows here.

## Tier A stripping rules

Aozora ruby (`《…》`), reading marks (`｜`), and editorial annotations
(`［＃…］`) are removed before a line enters the pool. Note the collision: `｜`
is both an Aozora ruby delimiter and a tier D vertical-text artefact. Stripping
happens at pool-construction time, before any tier D derivation, so a `｜` in a
committed line is a deliberate artefact and never residue.

Pre-war orthography is rejected outright. Both segmenters flail on 旧字体 and
旧仮名遣い, so an unfiltered Aozora sample returns disproportionately many of
them — a distribution ClipYomi never sees, which would then be what Phase 1 is
tuned against.

## Tier D derivation table

**Real capture artefacts**, observed in `vn_data`: ASCII `......` where `……`
belongs; halfwidth `?` and `!`; U+3000 mid-sentence; speaker-name prefixes;
`――` em-dash runs; halfwidth digits in Japanese context (`1つ`).

**Synthetic OCR confusables**, which text hooking cannot produce and only real
OCR does: 力/カ, 口/ロ, 一/ー/丨, ニ/二, 八/ハ, 卜/ト, dropped dakuten
(`ば`→`は`), full/half-width mixing, spurious intra-word space.

**Vertical-text artefacts**: `ー` rendered as `｜` or `丨`, `〜` as `～`.

## Tier C1 coverage intent

Read out of a `jparser-cli diff` run over the whole `vn_data` pool
(67,469 captured lines), so the pastiche covers the register classes that
actually generate disagreement rather than the ones a plan guessed at.

<!-- Task 6 Step 6: replace this comment with the observed classes, one
     bullet each, naming the class and how many of the top 100 disagreements
     it accounted for. Task 8 writes tier C1 sentences against this list and
     records which class each sentence targets. -->
```

Replace the HTML comment with the observed classes from Step 5. Leaving the comment in place is a plan failure.

- [ ] **Step 7: Commit**

```bash
git add tools/extract_vn_pool.py .gitignore crates/jparser/tests/fixtures/gold_sources.md
git commit -m "feat(harness): add the vn_data sampling pool extractor

67,469 captured visual-novel lines to a TSV pool jparser-cli diff reads.
The text is copyrighted and the pool is gitignored; its only committed
product is which register classes the disagreement sampler surfaced,
which is what tier C1's pastiche is then written against. Register and
style are not copyrightable; transcribed lines are.

Speaker names are separated into their own column rather than dropped:
a captured line usually carries one, and it belongs in the noise tier
rather than in every dialogue line's parse."
```

---

## Task 7: `label` — the adjudicator

Two levels: bless a span set at sentence level with one keystroke, then choose an entry per span. Candidates are listed **by `ent_seq`, not by rank** — this is the load-bearing detail. Ranked order makes the adjudicator read top-down, accept the plausible first candidate, and turn gold into a copy of the parser's own answer; entry top-1 would then read near 1.0 by construction and the whole baseline would be a tautology that looks like a good result.

**Files:**
- Create: `crates/jparser/src/bin/jparser-cli/label.rs`
- Create: `crates/jparser/tests/cli_label.rs`
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` (module, `Label` variant, dispatch arm)
- Modify: `crates/jparser/Cargo.toml` (new `[[test]]` entry)

**Interfaces:**
- Consumes: `corpus::{DiffRecord, GoldRecord, GoldSpan, append, read_diffs, validate}`, `jparser::Entry::entry_id`.
- Produces:
  - `fn spans_from_cuts(cuts: &[usize]) -> Vec<(usize, usize)>`
  - `fn parse_cuts(line: &str, chars: usize) -> Result<Vec<usize>, String>` — the `e` command's cut-list reader
  - `fn candidates(index: &Index, table: &ConjugationTable, text: &str, start: usize, len: usize) -> Result<Vec<(u32, String)>, ParseError>` — `(ent_seq, label)` pairs sorted ascending by `ent_seq`
  - the CLI surface `jparser-cli label <index> <disagreements.jsonl> <gold.jsonl>`

- [ ] **Step 1: Write the failing unit tests**

Create `crates/jparser/src/bin/jparser-cli/label.rs` with the standard header, this doc comment, and only the test module:

```rust
//! The adjudicator, per design §7.
//!
//! **Candidates are listed by `ent_seq`, not by rank.** Show them in the
//! parser's ranked order and the adjudicator reads top-down, accepts the
//! plausible first candidate, and gold `ent_seq` becomes a copy of the answer
//! under measurement — entry top-1 then reads near 1.0 by construction and the
//! baseline is a tautology that looks like a good result. Sorting by `ent_seq`
//! decorrelates list position from what the parser preferred, so position
//! carries no signal. Nothing printed reveals rank either.
//!
//! Input is read a line at a time rather than as raw keystrokes. A raw-mode
//! reader needs a TTY and a TTY cannot be driven from a test; one Enter per
//! decision buys an adjudicator whose every branch is covered by
//! `tests/cli_label.rs` piping a decision script. No new dependency.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_become_spans() {
        assert_eq!(spans_from_cuts(&[0, 3, 4, 7]), vec![(0, 3), (3, 1), (4, 3)]);
    }

    #[test]
    fn two_cuts_are_one_span() {
        assert_eq!(spans_from_cuts(&[0, 2]), vec![(0, 2)]);
    }

    #[test]
    fn fewer_than_two_cuts_is_no_spans() {
        assert_eq!(spans_from_cuts(&[0]), Vec::<(usize, usize)>::new());
        assert_eq!(spans_from_cuts(&[]), Vec::<(usize, usize)>::new());
    }

    /// The `e` command takes interior cuts only; 0 and the char length are
    /// implied, so splitting, merging, and moving a boundary are all one
    /// primitive rather than three commands.
    #[test]
    fn an_edit_line_adds_the_implied_outer_cuts() {
        assert_eq!(parse_cuts("3 4", 7), Ok(vec![0, 3, 4, 7]));
    }

    #[test]
    fn an_empty_edit_line_is_one_span_over_the_whole_text() {
        assert_eq!(parse_cuts("", 7), Ok(vec![0, 7]));
    }

    #[test]
    fn edit_cuts_are_sorted_and_deduplicated() {
        assert_eq!(parse_cuts("4 3 3", 7), Ok(vec![0, 3, 4, 7]));
    }

    #[test]
    fn a_cut_outside_the_text_is_rejected() {
        assert!(parse_cuts("9", 7).is_err());
        assert!(parse_cuts("0", 7).is_err(), "0 is implied, not an interior cut");
        assert!(parse_cuts("7", 7).is_err(), "the length is implied too");
    }

    #[test]
    fn a_non_numeric_edit_line_is_rejected() {
        assert!(parse_cuts("3 x", 7).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: FAIL to compile — `cannot find function 'spans_from_cuts'`, `parse_cuts`.

- [ ] **Step 3: Write the pure helpers**

Insert above the test module in `label.rs`:

```rust
use std::error::Error;
use std::io::{BufRead, Write};
use std::path::Path;

use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::{ParseOptions, ParseResult};

use crate::corpus::{append, read_diffs, validate, GoldRecord, GoldSpan};

/// Consecutive cuts to `(start, len)` spans.
pub fn spans_from_cuts(cuts: &[usize]) -> Vec<(usize, usize)> {
    cuts.windows(2).map(|w| (w[0], w[1] - w[0])).collect()
}

/// The `e` command's cut list: interior cuts only, with 0 and `chars` implied.
///
/// One primitive for splitting, merging, and moving a boundary — retyping the
/// cut list does all three, and three separate commands would be three times
/// the surface for no more capability.
pub fn parse_cuts(line: &str, chars: usize) -> Result<Vec<usize>, String> {
    let mut cuts = vec![0];
    for field in line.split_whitespace() {
        let cut: usize = field
            .parse()
            .map_err(|_| format!("{field:?} is not a char offset"))?;
        if cut == 0 || cut >= chars {
            return Err(format!(
                "{cut} is not an interior cut: 0 and {chars} are implied"
            ));
        }
        cuts.push(cut);
    }
    cuts.push(chars);
    cuts.sort_unstable();
    cuts.dedup();
    Ok(cuts)
}
```

- [ ] **Step 4: Run the unit tests to verify they pass**

```powershell
cargo test -p jparser --bin jparser-cli
```

Expected: PASS, 38 tests (30 from Task 5 plus these 8).

- [ ] **Step 5: Write the failing integration test**

Create `crates/jparser/tests/cli_label.rs` with the standard new-file header:

```rust
//! `jparser-cli label` end to end, driven by piped stdin.
//!
//! Line-buffered input is what makes this possible at all: a raw-keystroke
//! reader would need a TTY and none of these branches could be covered.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::stem::StemOptions;

const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");
const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn index_dir(dir: &Path) -> PathBuf {
    let index = dir.join("index");
    std::fs::create_dir_all(&index).unwrap();
    let table = ConjugationTable::load_embedded().unwrap();
    build_from_reader(
        std::io::Cursor::new(FIXTURE),
        &table,
        &StemOptions::default(),
        &index,
    )
    .expect("the mini fixture must build");
    index
}

/// One diff record over 言った, whose jparser and Vibrato boundaries are
/// supplied literally — `label` never tokenizes, so this needs no dictionary.
fn diff_file(dir: &Path, text: &str, jparser: &str, vibrato: &str) -> PathBuf {
    let path = dir.join("diff.jsonl");
    std::fs::write(
        &path,
        format!(
            "{{\"text\":\"{text}\",\"source\":\"test:1\",\"jparser\":{jparser},\
             \"vibrato\":{vibrato},\"conflicts\":[1],\"unknown_tokens\":0}}\n"
        ),
    )
    .unwrap();
    path
}

/// Drive `label` with `script` on stdin and return the gold file's contents.
fn label(index: &Path, diff: &Path, gold: &Path, script: &str) -> String {
    let mut child = Command::new(BIN)
        .arg("label")
        .arg(index)
        .arg(diff)
        .arg(gold)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("jparser-cli must be runnable");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("write the decision script");
    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(gold).unwrap_or_default()
}

/// Bless jparser's spans, then pick 言う. The candidate list is sorted by
/// ent_seq, and 1000010 is the only candidate on the whole-text span, so `1`
/// is unambiguous.
#[test]
fn blessing_jparser_spans_and_picking_an_entry_writes_one_gold_record() {
    let dir = scratch("cli-label-bless-jparser");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\n1\n");

    assert_eq!(
        written.trim(),
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}"
    );
}

/// `n` is a positive label, not a skip: the span is recorded with a null
/// ent_seq and counts for span accuracy.
#[test]
fn n_records_a_null_ent_seq() {
    let dir = scratch("cli-label-null");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\nn\n");

    assert!(written.contains("\"ent_seq\":null"), "got {written}");
}

/// `s` appends nothing. A sentence the adjudicator cannot rule on stays
/// unlabelled rather than becoming a guess in the gold file.
#[test]
fn s_writes_no_record_at_all() {
    let dir = scratch("cli-label-skip");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    assert_eq!(label(&index, &diff, &gold, "s\n").trim(), "");
}

/// `v` blesses the tokenizer's boundaries instead. Vibrato cut 言|った here, so
/// two spans get labelled; neither is a JMdict entry on its own, so both are
/// null.
#[test]
fn blessing_vibrato_spans_labels_every_token() {
    let dir = scratch("cli-label-bless-vibrato");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,1,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "v\nn\nn\n");

    assert!(written.contains("\"start\":0,\"len\":1"), "got {written}");
    assert!(written.contains("\"start\":1,\"len\":2"), "got {written}");
}

/// `e` retypes the interior cuts. 0 and 3 are implied, so `1` means 言|った.
#[test]
fn e_takes_a_retyped_cut_list() {
    let dir = scratch("cli-label-edit");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "e\n1\nn\nn\n");

    assert!(written.contains("\"start\":0,\"len\":1"), "got {written}");
    assert!(written.contains("\"start\":1,\"len\":2"), "got {written}");
}

/// A span with no candidate is still labellable: `#` takes an ent_seq
/// directly, so the adjudicator never has to skip a sentence they understand.
#[test]
fn a_hash_prefixed_ent_seq_is_accepted_directly() {
    let dir = scratch("cli-label-manual-id");
    let index = index_dir(&dir);
    let diff = diff_file(&dir, "言った", "[0,3]", "[0,3]");
    let gold = dir.join("gold.jsonl");

    let written = label(&index, &diff, &gold, "j\n#1000030\n");

    assert!(written.contains("\"ent_seq\":1000030"), "got {written}");
}
```

Register it in `crates/jparser/Cargo.toml`:

```toml
[[test]]
name = "cli_label"
path = "tests/cli_label.rs"
required-features = ["cli"]
```

- [ ] **Step 6: Run the integration test to verify it fails**

```powershell
cargo test -p jparser --test cli_label
```

Expected: FAIL — `error: unrecognized subcommand 'label'`.

- [ ] **Step 7: Write the adjudicator**

Append to `label.rs`, below the pure helpers:

```rust
/// `(ent_seq, one-line label)` for every entry the index offers on this exact
/// span, sorted ascending by `ent_seq`.
///
/// The sort is the tautology guard (see the module doc). It also makes the list
/// stable across runs, which a rank-ordered list would not be once the ranker
/// changes in Phase 1.
///
/// Candidates come from parsing the span's own substring, not from the
/// whole-sentence parse: an adjudicator who retyped the cuts has spans the
/// sentence parse never produced. A substring parse that does not come back as
/// one span covering the whole substring offers nothing, and `#<ent_seq>`
/// covers that case.
pub fn candidates(
    index: &Index,
    table: &ConjugationTable,
    text: &str,
    start: usize,
    len: usize,
) -> Result<Vec<(u32, String)>, Box<dyn Error>> {
    let span_text: String = text.chars().skip(start).take(len).collect();
    let parsed: ParseResult =
        jparser::parse(index, table, &span_text, &ParseOptions::default(), None)?;
    let mut out: Vec<(u32, String)> = match parsed.segments.as_slice() {
        [seg] if seg.start == 0 && seg.len == len => seg
            .entries
            .iter()
            .map(|e| {
                let gloss = e
                    .senses
                    .first()
                    .map(|s| s.glosses.join("; "))
                    .unwrap_or_default();
                let reading = e.reading.as_deref().unwrap_or("-");
                (e.entry_id, format!("{} ({reading}) {gloss}", e.headword))
            })
            .collect(),
        _ => Vec::new(),
    };
    out.sort_by_key(|(id, _)| *id);
    out.dedup_by_key(|(id, _)| *id);
    Ok(out)
}

/// Render a cut list as `a|b|c` over the text, for the two-parse display.
fn render_spans(text: &str, cuts: &[usize]) -> String {
    let chars: Vec<char> = text.chars().collect();
    spans_from_cuts(cuts)
        .iter()
        .map(|&(start, len)| chars[start..start + len].iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("|")
}

/// Read one line, trimmed. `None` at end of input.
fn ask(prompt: &str) -> Option<String> {
    print!("{prompt}");
    // Unbuffered prompts would appear after the answer under a pipe.
    std::io::stdout().flush().ok()?;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// Adjudicate `diff_path` into `gold_path`, appending as it goes.
pub fn run(index_dir: &Path, diff_path: &Path, gold_path: &Path) -> Result<(), Box<dyn Error>> {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(index_dir)?;
    let records = read_diffs(diff_path)?;
    let total = records.len();

    for (i, record) in records.iter().enumerate() {
        let chars = record.text.chars().count();
        println!(
            "\n[{}/{total}] source={} unknown_tokens={}",
            i + 1,
            record.source,
            record.unknown_tokens
        );
        println!("{}", record.text);
        println!("j: {}", render_spans(&record.text, &record.jparser));
        println!("v: {}", render_spans(&record.text, &record.vibrato));
        println!("   conflicts at {:?}", record.conflicts);

        let cuts = loop {
            let Some(answer) = ask("[j] jparser  [v] vibrato  [e] edit  [s] skip  [q] quit > ")
            else {
                return Ok(());
            };
            match answer.as_str() {
                "j" => break record.jparser.clone(),
                "v" => break record.vibrato.clone(),
                "s" => break Vec::new(),
                "q" => return Ok(()),
                "e" => {
                    let Some(line) =
                        ask(&format!("cuts (interior only, 0 and {chars} implied) > "))
                    else {
                        return Ok(());
                    };
                    match parse_cuts(&line, chars) {
                        Ok(cuts) => break cuts,
                        Err(e) => println!("  {e}"),
                    }
                }
                other => println!("  {other:?} is not one of j v e s q"),
            }
        };
        if cuts.is_empty() {
            continue;
        }

        let mut spans = Vec::new();
        for (start, len) in spans_from_cuts(&cuts) {
            let surface: String = record.text.chars().skip(start).take(len).collect();
            println!("\n  span {start}..{} 「{surface}」", start + len);
            let cands = candidates(&index, &table, &record.text, start, len)?;
            for (n, (id, label)) in cands.iter().enumerate() {
                println!("    {}) {id} {label}", n + 1);
            }
            if cands.is_empty() {
                println!("    (no candidate on this span)");
            }
            let ent_seq = loop {
                let Some(answer) =
                    ask("  [1-N] pick  [n] no entry  [#<ent_seq>] by id > ")
                else {
                    return Ok(());
                };
                if answer == "n" {
                    break None;
                }
                if let Some(rest) = answer.strip_prefix('#') {
                    match rest.parse::<u32>() {
                        Ok(id) => break Some(id),
                        Err(_) => {
                            println!("    {rest:?} is not an ent_seq");
                            continue;
                        }
                    }
                }
                match answer.parse::<usize>() {
                    Ok(n) if (1..=cands.len()).contains(&n) => break Some(cands[n - 1].0),
                    _ => println!("    pick 1-{}, n, or #<ent_seq>", cands.len()),
                }
            };
            spans.push(GoldSpan {
                start,
                len,
                ent_seq,
            });
        }

        let gold = GoldRecord {
            text: record.text.clone(),
            source: record.source.clone(),
            spans,
        };
        // Validate before appending: an invalid record in the gold file is a
        // hard error at score time, and catching it here costs one check.
        validate(&gold)?;
        append(gold_path, &gold)?;
    }
    Ok(())
}
```

In `main.rs`, add `mod label;`, the variant:

```rust
    /// Adjudicate sampled disagreements into the gold corpus.
    ///
    /// Candidates are listed by ent_seq, never by rank: a rank-ordered list
    /// would make gold a copy of the ranking under measurement.
    Label {
        /// Index directory.
        index: PathBuf,
        /// `jparser-cli diff` output.
        disagreements: PathBuf,
        /// Gold corpus JSONL, appended to. Created if absent.
        gold: PathBuf,
    },
```

and the dispatch arm:

```rust
        Command::Label {
            index,
            disagreements,
            gold,
        } => label::run(&index, &disagreements, &gold)?,
```

- [ ] **Step 8: Run the integration test to verify it passes**

```powershell
cargo test -p jparser --test cli_label
```

Expected: PASS, 6 tests.

`blessing_vibrato_spans_labels_every_token` and `e_takes_a_retyped_cut_list` assume 言 alone and った alone have no JMdict candidate in the mini fixture, so `n` is the right answer for both spans. If either offers a candidate, the assertion to change is the decision script, not the implementation — run `cargo run -p jparser --bin jparser-cli -- parse <index> 言` to see what the fixture actually offers.

- [ ] **Step 9: Run the full gate**

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
cargo clippy -p jparser --features mecab -- -D warnings
```

Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add crates/jparser/Cargo.toml crates/jparser/src/bin/jparser-cli/ crates/jparser/tests/cli_label.rs
git commit -m "feat(harness): add jparser-cli label

Bless a span set at sentence level with one keystroke, then choose an
entry per span. That fast path is what makes full-sentence gold
affordable, and full-sentence gold is what gives span accuracy a fixed
denominator.

Candidates are listed by ent_seq, not by rank. A rank-ordered list makes
the adjudicator accept the plausible first candidate, gold becomes a
copy of the ranking under measurement, and entry top-1 reads near 1.0 by
construction. One sort_by_key is the difference between a baseline and a
number that flatters itself.

Reads lines rather than raw keystrokes, so every branch is covered by a
piped decision script instead of needing a TTY."
```

---

## Task 8: Build and adjudicate the corpus

The expensive artefact, and it comes after a proven harness on purpose. Mostly manual selection and adjudication; the acceptance conditions are counts and checks, not code.

**Files:**
- Create: `crates/jparser/tests/fixtures/gold.jsonl`
- Create: `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
- Create (uncommitted): `crates/jparser/tests/fixtures/gold_local.jsonl`
- Modify: `crates/jparser/tests/fixtures/gold_sources.md` (per-sentence rows)
- Modify: `crates/jparser/tests/fixtures/README.md`

**Interfaces:**
- Consumes: `diff`, `label`, `score`, `extract_jmdict_subset.py`'s JSONL mode, `gold_sources.md`'s tier C1 coverage intent.
- Produces: the committed corpus and subset that Task 9 measures.

- [ ] **Step 1: Build the tier A and B pools**

Select from Aozora and ja.Wikinews / ja.Wikipedia by hand, into TSV pools in the scratch directory (not the repo):

- **Tier A**, target 80 sentences from **at least 10 works**, max 8 per work. 新字新仮名 editions only — the card page states the variant; reject 旧字旧仮名. Strip `《…》`, `｜`, and `［＃…］` before the line enters the pool.
- **Tier B**, target 50 sentences. Prefer ja.Wikinews (CC BY 2.5, and actual news register) over ja.Wikipedia (CC BY-SA 4.0) where both would serve.

Source ids: `a:<aozora-card>-<n>` and `b:<wikinews-or-wikipedia>-<article>-<n>`. Record every work and article in `gold_sources.md` as you select, with author, card number or URL plus revision id, licence, and retrieval date — not afterwards from memory.

- [ ] **Step 2: Sample tiers A and B**

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  diff "$app\dict\gen-1" "$scratch\pool_ab.tsv" --hints "$app\hints\current\system.dic" `
  > "$scratch\disagreements_ab.jsonl"
```

If the sampled count is far below 130, the pools are too small — widen them rather than lowering the target. If it is far above, that is fine: `label` walks them ranked by conflict density and you stop when the tier targets are met.

- [ ] **Step 3: Write tier C1**

80 hand-written sentences against the coverage-intent list in `gold_sources.md`, written in Task 6 Step 6 out of the real `vn_data` disagreements. One `gold_sources.md` row per sentence naming which class it targets, so a reviewer can check the tier covers what it claims.

Source ids `c1:pastiche-001` upward. Put them in a TSV pool and run them through `diff` and `label` like any other tier — a hand-written sentence gets no exemption from adjudication, and the sampler's ranking is useful evidence about whether the pastiche actually hits the hard classes.

- [ ] **Step 4: Derive tier D**

40 lines, each derived from an already-selected line in A, B, or C1, using only the transformations in `gold_sources.md`'s derivation table. Source ids `d:<source-id-it-came-from>`. A tier D line derived from a tier B source stays CC BY-SA and keeps that attribution.

Spread the coverage: real capture artefacts, synthetic OCR confusables, and vertical-text artefacts should each get roughly a third, and no single confusable should account for more than three lines.

- [ ] **Step 5: Adjudicate**

```powershell
cargo run --release -p jparser --bin jparser-cli -- `
  label "$app\dict\gen-1" "$scratch\disagreements_all.jsonl" `
  crates/jparser/tests/fixtures/gold.jsonl
```

Full-sentence gold: every span in a selected sentence gets a label. `s` any sentence you cannot rule on — an unlabelled sentence costs a denominator entry, a guessed one costs the baseline's meaning.

The session is resumable: `label` appends, and re-labelling a sentence supersedes the earlier record. `score` will report the supersession count.

- [ ] **Step 6: Measure the subset size at 100 sentences**

Stop at 100 labelled sentences and check the projection before labelling the rest:

```powershell
python tools/extract_jmdict_subset.py `
  "$scratch\JMdict_e.xml" `
  crates/jparser/tests/fixtures/gold.jsonl `
  crates/jparser/assets/conjugations.json `
  "$scratch\gold_subset_probe.xml"
```

Read the printed `output bytes`. If 2.5× that exceeds **8 MB**, cap the corpus where it holds under 8 MB and record the cap and the measured numbers in `gold_sources.md`. Do not compress the fixture — that buys a dependency and loses reviewability.

- [ ] **Step 7: Finish adjudicating and generate the committed subset**

```powershell
python tools/extract_jmdict_subset.py `
  "$scratch\JMdict_e.xml" `
  crates/jparser/tests/fixtures/gold.jsonl `
  crates/jparser/assets/conjugations.json `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml
```

- [ ] **Step 8: Build tier C2's local gold file**

100 lines sampled from `disagreements` over `vn_data/pool.tsv` (Task 6 Step 5), adjudicated into `crates/jparser/tests/fixtures/gold_local.jsonl` — gitignored in Task 6 Step 4. No `gold_sources.md` rows; the text is not committed and cannot be cited.

- [ ] **Step 9: Verify the committed corpus scores**

```powershell
cargo run -p jparser --bin jparser-cli -- build-index `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\gold-index"
cargo run -p jparser --bin jparser-cli -- score `
  "$scratch\gold-index" crates/jparser/tests/fixtures/gold.jsonl
```

Expected: both metrics print. `sentences` matches the label count and `superseded` is a number you can account for.

An `ent_seq` in gold that the subset index does not carry is a hard error meaning the extractor was not re-run after the corpus grew — re-run Step 7, do not work around it.

- [ ] **Step 10: Document the fixtures**

Append to `crates/jparser/tests/fixtures/README.md`:

```markdown
## `gold.jsonl`

The Engine v2 Phase 0 gold corpus: labelled `(span, ent_seq)` pairs over a
genre-extended sentence set, sampled by jparser-against-Vibrato disagreement.
Format and tiers: [Phase 0 design](../../../../docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md)
§3, §4.1. Provenance and attribution: `gold_sources.md`.

Independent of `parse_sentences.txt`, which stays the `parse_snapshots.rs`
corpus and is untouched by Phase 0.

Score it with:

```bash
cargo run -p jparser --bin jparser-cli -- build-index \
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml /tmp/gold-index
cargo run -p jparser --bin jparser-cli -- score \
  /tmp/gold-index crates/jparser/tests/fixtures/gold.jsonl
```

## `gold_jmdict_subset.xml`

The JMdict subset `gold.jsonl` can reach, same extractor and same licence terms
as `jmdict_subset.xml` above. It scores identically to a full JMdict index on
this corpus: the extractor's closure is a superset of what can match at any
corpus position, index flags are entry-local, and `sort_matches` is per-span and
keyed on `entry_id` alone. The Phase 0 handoff records the measurement that
confirms it.

Regenerate after **any** change to `gold.jsonl`:

```bash
python3 tools/extract_jmdict_subset.py \
  /path/to/JMdict_e.xml \
  crates/jparser/tests/fixtures/gold.jsonl \
  crates/jparser/assets/conjugations.json \
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml
```

## `gold_sources.md`

Provenance manifest for `gold.jsonl`: one row per source id, plus the tier
definitions, the Aozora stripping rules, and the noise-tier derivation table.
Carries the CC BY and CC BY-SA attribution tiers B and D require.
```

- [ ] **Step 11: Commit**

```bash
git add crates/jparser/tests/fixtures/gold.jsonl crates/jparser/tests/fixtures/gold_jmdict_subset.xml crates/jparser/tests/fixtures/gold_sources.md crates/jparser/tests/fixtures/README.md
git commit -m "feat(harness): add the Phase 0 gold corpus

Four committed tiers: Aozora prose (新字新仮名 only, capped per work),
ja.Wikinews and ja.Wikipedia for the news register Aozora does not
carry, hand-written pastiche for the visual-novel register classes the
vn_data sampler actually surfaced, and hand-derived capture and OCR
noise. Full-sentence gold, so span accuracy has a fixed denominator.

Pre-war orthography is rejected outright: both segmenters flail on it,
so an unfiltered Aozora sample would have biased the corpus toward a
distribution ClipYomi never sees.

The real visual-novel tier is scored from a gitignored local file. Its
text is copyrighted and never committed; the committed corpus is the
reproducible contract."
```

---

## Task 9: Record the baseline

The four numbers every later phase is measured against, plus the one check that cannot be replaced by reasoning.

**Files:**
- Create: `docs/superpowers/2026-08-28-engine-v2-phase0-handoff.md`
- Modify: `docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md` (§8.3's table)
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` (§9.1, §10, the §14 deviations)

**Interfaces:**
- Consumes: everything above.
- Produces: the recorded baseline.

- [ ] **Step 1: Measure the committed corpus, both ways**

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0"
$gold = "crates/jparser/tests/fixtures/gold.jsonl"

cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index" $gold
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index" $gold --hints "$app\hints\current\system.dic"
```

Keep both outputs verbatim.

- [ ] **Step 2: Measure the local tier C2 corpus, both ways**

Same two commands against `crates/jparser/tests/fixtures/gold_local.jsonl`, and against the **full** index — the local corpus has no committed subset and needs none:

```powershell
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$app\dict\gen-1" crates/jparser/tests/fixtures/gold_local.jsonl
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$app\dict\gen-1" crates/jparser/tests/fixtures/gold_local.jsonl `
  --hints "$app\hints\current\system.dic"
```

- [ ] **Step 3: Run the equivalence check**

The gate that matters. Score the committed corpus against the full index and against the subset index, both ways, and compare all four outputs character for character:

```powershell
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$app\dict\gen-1" $gold --json > "$scratch\full_off.json"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index" $gold --json > "$scratch\subset_off.json"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$app\dict\gen-1" $gold --hints "$app\hints\current\system.dic" --json > "$scratch\full_on.json"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index" $gold --hints "$app\hints\current\system.dic" --json > "$scratch\subset_on.json"

Compare-Object (Get-Content "$scratch\full_off.json") (Get-Content "$scratch\subset_off.json")
Compare-Object (Get-Content "$scratch\full_on.json") (Get-Content "$scratch\subset_on.json")
```

Expected: both `Compare-Object` calls print nothing.

**If they differ, stop.** The offline gate is not met, the design's §8.2 argument is wrong, and the difference is the finding — report it with both JSON objects rather than adjusting anything. The likeliest cause is a corpus edit after the last extractor run; re-run Task 8 Step 7 and re-check before concluding the argument is wrong.

- [ ] **Step 4: Verify the offline gate in a clean checkout**

```powershell
$clone = "$scratch\offline-clone"
git clone --depth 1 (Get-Location) $clone
cd $clone
cargo run -p jparser --bin jparser-cli -- build-index `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\clone-index"
cargo run -p jparser --bin jparser-cli -- score `
  "$scratch\clone-index" crates/jparser/tests/fixtures/gold.jsonl
cd -
```

Expected: both metrics print, matching Step 1's hints-off numbers. Disconnect the network first if you can — nothing in this path should reach for it, and the only way to be sure is to take it away.

- [ ] **Step 5: Write the handoff**

Create `docs/superpowers/2026-08-28-engine-v2-phase0-handoff.md`, following the shape of `2026-08-27-jparser-phase5b-handoff.md`. It must carry:

- The four committed-corpus numbers, with numerators and denominators.
- The two tier C2 numbers, marked irreproducible.
- The corpus composition: sentences per tier, total gold spans, supersession count.
- The equivalence check's result and the index generation it ran against (`dict/gen-1`).
- The `vn_data` pool numbers from Task 6 Step 3.
- The subset size measured at Task 8 Step 6 and whether a cap was applied.
- The commit hash of the corpus, so a later "the number moved" is answerable.
- What Phase 1 must not do: change a metric definition, re-label a sentence to make a number move, or regenerate the subset without re-running Step 3.

- [ ] **Step 6: Fill the design's baseline table**

In `docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md` §8.3, replace the empty table with the measured numbers and delete the "deliberately empty here" sentence.

- [ ] **Step 7: Fold the deviations into the master plan**

In `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`:

- **§5.3** — replace the "Open question (§9.1): corpus sourcing" paragraph with the answer, pointing at the Phase 0 design §3.
- **§5.4** — record that `diff`'s predicate is `Bj \ Bv ≠ ∅`, not boundary disagreement, and that `label` reads a line rather than a keystroke.
- **§7 Phase 0** — record the amended reading of "no code under `crates/jparser/src/`" and the two exceptions.
- **§9** — strike open question 1; it is answered.
- **§10** — add the decision-log rows:

  | Decision | Rationale |
  | --- | --- |
  | Sample on `Bj \ Bv`, not on boundary disagreement | jparser emits JMdict long units and Vibrato IPADIC short units, so set inequality flags every compound and selects nearly the whole pool — the same mistake §5.2 rejects boundary F1 for. One-directional excludes coarsening and still surfaces `くるまではこう` (Phase 0 design §6.1) |
  | Full-sentence gold, not disagreement-spans-only | A hard-spans-only denominator shifts with corpus difficulty, so a Phase 1 number would not be comparable to Phase 0's (Phase 0 design §5.1) |
  | `label` lists candidates by `ent_seq`, not by rank | A rank-ordered list makes gold a copy of the ranking under measurement and entry top-1 reads near 1.0 by construction (Phase 0 design §7.2) |
  | Aozora is not the news-register source | It is literary prose, 1900–1955. ja.Wikinews and ja.Wikipedia carry the register, under the same CC BY-SA-as-separate-asset pattern `jmdict_subset.xml` already uses (Phase 0 design §3.1) |
  | Reject unfiltered Aozora | Both segmenters flail on 旧字旧仮名, so the disagreement sampler returns disproportionately many pre-war sentences — a distribution ClipYomi never sees, which Phase 1 would then be tuned against (Phase 0 design §3.3) |
  | Expose `ent_seq` on `Entry` rather than proxy on headword and reading | Two entries can share a headword and reading, so the proxy passes silently exactly where entry top-1 is meant to catch a mis-ranking (Phase 0 design §5.4) |

- [ ] **Step 8: Run the full gate**

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo check -p jparser --no-default-features
cargo clippy --workspace -- -D warnings
cargo clippy -p jparser --features mecab -- -D warnings
```

Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add docs/superpowers/2026-08-28-engine-v2-phase0-handoff.md docs/superpowers/specs/
git commit -m "docs(engine-v2): record the Phase 0 baseline

Span accuracy and entry top-1 for the current engine, hints-on and
hints-off, over the committed gold corpus, plus the same pair over the
uncommitted real-visual-novel corpus. This is the contract Phase 1 and
Phase 2 are measured against.

Records the subset-equivalence check: the committed subset index scores
identically to the full 67 MB index on this corpus, character for
character, both ways. That is what makes 'reproducibly and offline'
true rather than argued.

Folds Phase 0's seven deviations back into the master plan and strikes
open question 1."
```

---

## Self-Review

**Spec coverage.** Every section of the design maps to a task:

| Spec | Task |
| --- | --- |
| §2.1 scope reading | 1, 2 (and §14 item 1 recorded in 9 Step 7) |
| §3.1 five tiers | 8 Steps 1, 3, 4, 8 |
| §3.2 `vn_data` as pool | 6 |
| §3.3 Aozora orthography trap | 6 Step 6 (rules), 8 Step 1 (applied) |
| §3.4 sizes and the 8 MB cap | 8 Steps 1, 3, 4, 6 |
| §4.1 gold format | 3 |
| §4.2 append-only, last-wins | 3 |
| §4.3 diff format | 3, 5 |
| §4.4 provenance manifest | 6 Step 6, 8 Steps 1, 3, 4 |
| §5.1 full-sentence gold | 7 Step 7, 8 Step 5 |
| §5.2 metric definitions | 4 |
| §5.3 output format | 4 |
| §5.4 `entry_id` | 2 |
| §6.1 the refinement predicate | 5 |
| §6.2 raw Vibrato | 5 |
| §6.3 diff is not the offline gate | 5 Step 5, 9 Step 4 |
| §7.1 two levels and the fast path | 7 |
| §7.2 `ent_seq` ordering | 7 |
| §7.3 line-buffered stdin | 7 |
| §8.1 offline contract | 9 Step 4 |
| §8.2 subset equivalence | 4 Step 7, 8 Step 7, 9 Step 3 |
| §8.3 the baseline pair | 9 Steps 1, 2, 6 |
| §9.1 bin as directory | 1 |
| §9.2 no new dependencies | Global Constraints |
| §9.3 the Python tools | 4 Step 7, 6 Step 1 |
| §11.1–11.5 verification | 3, 4, 5, 7 unit and integration tests; 9 Steps 3, 4 |
| §12 errors | 3 (`validate`, `CorpusError`), 4 Step 5 |
| §13 deferred | Not implemented, by design |
| §14 deviations | 9 Step 7 |

Two gaps found and closed while reviewing:

- §11.2's **manual `--all` check** — 200 pool lines read by hand to confirm the predicate hides no real conflict — had no step. It is the only evidence the sampler is not filtering out the bug the corpus exists to find. Added as Task 5 Step 5's second half; do it against `vn_data/pool.tsv` once Task 6 has built it, and record the finding in the Task 9 handoff.
- §5.3's `--json` had a CLI flag but no test. Added as `cli_score.rs`'s fourth test.

**Placeholder scan.** Two deliberate fill-ins, each with an explicit step that replaces it and a note that leaving it is a plan failure: `gold_sources.md`'s tier C1 coverage-intent comment (Task 6 Step 6) and Task 3's `#[allow(dead_code)]` (removed in Task 4 Step 3). One stray import in the Task 4 code block is called out and corrected in the same step rather than left to be discovered.

**Type consistency.** `GoldSpan`/`GoldRecord`/`DiffRecord` field names are identical in Task 3's definitions, Task 4's and Task 7's test helpers, and every JSONL literal in `cli_score.rs` and `cli_label.rs`. `Metrics` field names match between the struct, `render`, the `--json` assertion, and the accumulate tests. `conflicts`, `boundaries_from_segments`, `read_pool`, `spans_from_cuts`, `parse_cuts`, and `candidates` are each named identically in their Interfaces block, their tests, and their implementation. `Segment` literals in Tasks 4 and 5 carry all six fields plus `entry_id`, matching `lib.rs` after Task 2.
