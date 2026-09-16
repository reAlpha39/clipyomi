// SPDX-License-Identifier: MIT OR Apache-2.0
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
    /// Which key the adjudicator pressed to settle this sentence's spans: `"j"`
    /// (blessed jparser's), `"v"` (blessed Vibrato's), `"e"` (retyped the cuts).
    ///
    /// Recorded because `j` blesses the *hints-off* parse verbatim (`diff` runs
    /// with no hints), so a `j`-blessed sentence scores 1.0 under `score`
    /// hints-off by construction. That biases §8.3's hints-on-minus-hints-off
    /// comparison, and the bias is only visible if the decision is recorded.
    /// A high `j` rate is evidence of rubber-stamping, not of a good parser.
    ///
    /// `None` for a hand-authored record that no adjudicator produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
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
            decision: None,
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

    /// A hand-written gold line predating the `decision` field must still
    /// parse, with `decision: None` — `serde(default)` is load-bearing here.
    #[test]
    fn a_record_with_no_decision_field_reads_as_none() {
        let dir = scratch("no-decision-field");
        let path = dir.join("gold.jsonl");
        std::fs::write(
            &path,
            "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}\n",
        )
        .expect("write");

        let (records, _) = read_gold(&path).expect("read");

        assert_eq!(records[0].decision, None);
    }

    /// `decision` round-trips, and a record with a decision does not emit
    /// `"decision":null` for one without — `skip_serializing_if` covers it.
    #[test]
    fn a_decision_round_trips_and_is_omitted_when_absent() {
        let dir = scratch("decision-round-trip");
        let path = dir.join("gold.jsonl");
        let mut with_decision = gold("言った", &[(0, 3, Some(1000010))]);
        with_decision.decision = Some("j".to_string());
        append(&path, &with_decision).expect("append");
        append(&path, &gold("は", &[(0, 1, None)])).expect("append");

        let (records, _) = read_gold(&path).expect("read");

        assert_eq!(records[0].decision.as_deref(), Some("j"));
        assert_eq!(records[1].decision, None);
        let raw = std::fs::read_to_string(&path).expect("read raw");
        let lines: Vec<&str> = raw.lines().collect();
        assert!(lines[0].contains("\"decision\":\"j\""), "got {}", lines[0]);
        assert!(!lines[1].contains("\"decision\""), "got {}", lines[1]);
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

    // `.err().expect(...)` rather than `expect_err(...)`: kept verbatim per
    // the brief; the allow silences a clippy lint the brief predates.
    #[allow(clippy::err_expect)]
    #[test]
    fn a_malformed_line_names_its_line_number() {
        let dir = scratch("malformed");
        let path = dir.join("gold.jsonl");
        std::fs::write(
            &path,
            "{\"text\":\"あ\",\"source\":\"t\",\"spans\":[]}\nnot json\n",
        )
        .expect("write");

        let err = read_gold(&path).err().expect("must fail");
        assert!(err.to_string().contains("line 2"), "got {err}");
    }

    #[allow(clippy::err_expect)]
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
