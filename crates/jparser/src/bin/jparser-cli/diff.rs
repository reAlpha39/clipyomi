// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

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

use std::error::Error;
use std::path::Path;

use jparser::index::load::Index;
use jparser::{ParseOptions, Segment};

use crate::corpus::DiffRecord;

/// Index of the reading in an IPADIC feature string, for the unknown-token
/// count. Once duplicated from a private constant in `hints.rs` rather than
/// made public, because `hints.rs` was out of scope for Phase 0 and one
/// integer was a smaller cost than an API change. **That constant went with
/// the v1 engine and this is the only copy in the tree now (2026-09-16):**
/// `hints::VibratoTokenizer::boundaries` never consults the reading field at
/// all, so there is nothing left to keep in step with.
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

/// Clamp a raw Vibrato cut list to the sentence's real bounds: `0` at the
/// front and `chars` at the back.
///
/// Vibrato does not guarantee its first token starts at char 0 or its last
/// token ends at the sentence's char length — it can skip a character it
/// never tokenizes (an interior space, U+3000) without leaving a boundary
/// there. An unclamped list would then make [`conflicts`] report `0` or
/// `chars` as a jparser-only cut even when the two segmentations agree on
/// everything else, over-sampling into the noise tier `diff --all` exists to
/// audit. A no-op when the list already covers the full range, and left as an
/// empty list when there was nothing to tokenize at all — see
/// `no_sampling_signal`.
fn clamp_bounds(mut cuts: Vec<usize>, chars: usize) -> Vec<usize> {
    if cuts.is_empty() {
        return cuts;
    }
    if cuts[0] != 0 {
        cuts.insert(0, 0);
    }
    if *cuts.last().expect("checked non-empty above") != chars {
        cuts.push(chars);
    }
    cuts
}

/// True when Vibrato tokenized nothing at all. `conflicts` would then treat
/// every jparser cut as a disagreement — the densest possible signal — while
/// `unknown_tokens: 0` tells the adjudicator the tokenizer was confident. That
/// is a tokenizer failure, not sampling signal, so `run` skips these records
/// rather than letting them consume `--limit`'s budget.
fn no_sampling_signal(vibrato_cuts: &[usize]) -> bool {
    vibrato_cuts.is_empty()
}

/// Tokenize `text` and return its cut offsets plus the count of tokens whose
/// IPADIC reading field is `*`, absent, or empty.
///
/// Deliberately **not** `VibratoTokenizer::boundaries`. That returns a
/// `BoundarySet` and nothing else, so the reading-less token count is not
/// recoverable from it — and that count is the whole point here, because a
/// sentence the tokenizer recognised nothing in is a sentence worth looking
/// at. Driving a worker directly is the only way to read both off one
/// tokenization.
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
        cuts = clamp_bounds(cuts, text.chars().count());
    }
    (cuts, unknown)
}

/// Order `records` for the run's audience. See [`rank`].
///
/// Normal runs sort by conflict count descending, so `--limit N` is a
/// stopping point rather than an arbitrary truncation. `--all` drops the
/// predicate and emits every sentence whose boundary sets differ at all —
/// kept only to verify by hand that the predicate is not filtering out real
/// conflicts (design §11.2). Its extra records are exactly the
/// predicate-excluded ones (empty `conflicts`), so `--all` sorts ascending
/// instead: those records come first, where `--limit N` can actually see
/// them, rather than last under the normal ordering.
fn rank(records: &mut [DiffRecord], all: bool) {
    records.sort_by(|a, b| {
        let by_count = a.conflicts.len().cmp(&b.conflicts.len());
        let by_count = if all { by_count } else { by_count.reverse() };
        by_count.then_with(|| a.text.cmp(&b.text))
    });
}

/// Emit the pool sentences worth labelling, as JSONL on stdout.
pub fn run(
    index_dir: &Path,
    pool: &Path,
    hints: &Path,
    all: bool,
    limit: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    let table = jparser::engine::table::Table::new();
    let index = Index::open(index_dir)?;
    let file = std::fs::File::open(hints)
        .map_err(|e| format!("opening the vibrato dictionary at {}: {e}", hints.display()))?;
    let dict = vibrato::Dictionary::read(std::io::BufReader::new(file)).map_err(|e| {
        format!(
            "the vibrato dictionary at {} could not be loaded: {e}",
            hints.display()
        )
    })?;
    let raw = vibrato::Tokenizer::new(dict);

    let mut records = Vec::new();
    let mut skipped_no_signal = 0;
    for (source, text) in read_pool(pool)? {
        let parsed = jparser::parse(&index, &table, &text, &ParseOptions::default(), None)?;
        let j = boundaries_from_segments(&parsed.segments);
        let (v, unknown_tokens) = tokenize(&raw, &text);
        if no_sampling_signal(&v) {
            skipped_no_signal += 1;
            continue;
        }
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
    if skipped_no_signal > 0 {
        eprintln!("note: skipped {skipped_no_signal} sentence(s) where vibrato produced no tokens");
    }

    rank(&mut records, all);
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

    /// A leading character Vibrato never tokenizes (e.g. a skipped space)
    /// leaves the raw cut list missing its 0.
    #[test]
    fn clamp_bounds_prepends_a_missing_zero() {
        assert_eq!(clamp_bounds(vec![1, 2, 3], 3), vec![0, 1, 2, 3]);
    }

    /// Symmetric case at the end of the sentence.
    #[test]
    fn clamp_bounds_appends_a_missing_final_boundary() {
        assert_eq!(clamp_bounds(vec![0, 1, 2], 3), vec![0, 1, 2, 3]);
    }

    #[test]
    fn clamp_bounds_leaves_a_correct_cover_untouched() {
        assert_eq!(clamp_bounds(vec![0, 1, 3], 3), vec![0, 1, 3]);
    }

    #[test]
    fn clamp_bounds_leaves_an_empty_list_empty() {
        assert_eq!(clamp_bounds(Vec::new(), 3), Vec::<usize>::new());
    }

    #[test]
    fn a_sentence_with_no_vibrato_tokens_has_no_sampling_signal() {
        assert!(no_sampling_signal(&[]));
    }

    #[test]
    fn a_sentence_with_vibrato_tokens_has_sampling_signal() {
        assert!(!no_sampling_signal(&[0, 2, 4]));
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

    /// Minimal `DiffRecord` for ordering tests: only `text` and `conflicts`
    /// matter to `rank`.
    fn record(text: &str, conflicts: Vec<usize>) -> DiffRecord {
        DiffRecord {
            text: text.to_string(),
            source: "test:1".to_string(),
            jparser: Vec::new(),
            vibrato: Vec::new(),
            conflicts,
            unknown_tokens: 0,
        }
    }

    fn texts(records: &[DiffRecord]) -> Vec<&str> {
        records.iter().map(|r| r.text.as_str()).collect()
    }

    /// `--all`'s whole point is surfacing the predicate-excluded records
    /// (empty `conflicts`), so under `all == true` they must sort first, not
    /// last.
    #[test]
    fn all_sorts_conflict_count_ascending() {
        let mut records = vec![
            record("two", vec![1, 2]),
            record("zero", vec![]),
            record("one", vec![9]),
        ];

        rank(&mut records, true);

        assert_eq!(texts(&records), vec!["zero", "one", "two"]);
    }

    #[test]
    fn normal_run_sorts_conflict_count_descending() {
        let mut records = vec![
            record("two", vec![1, 2]),
            record("zero", vec![]),
            record("one", vec![9]),
        ];

        rank(&mut records, false);

        assert_eq!(texts(&records), vec!["two", "one", "zero"]);
    }

    /// Two records with equal conflict counts must not swap between runs.
    #[test]
    fn tied_conflict_counts_break_on_text_ascending_when_all() {
        let mut records = vec![record("b", vec![1]), record("a", vec![1])];

        rank(&mut records, true);

        assert_eq!(texts(&records), vec!["a", "b"]);
    }

    #[test]
    fn tied_conflict_counts_break_on_text_ascending_when_not_all() {
        let mut records = vec![record("b", vec![1]), record("a", vec![1])];

        rank(&mut records, false);

        assert_eq!(texts(&records), vec!["a", "b"]);
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
