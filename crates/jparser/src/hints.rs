// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Boundary hints derived from Vibrato tokenization.
//!
//! `segment.rs` weights `BoundaryHints` into the segmentation DP but has had no
//! implementation since Phase 1B. This module is that implementation: it
//! tokenizes with Vibrato and marks the *interior* positions of each token, so
//! the DP is discouraged from splitting inside a word the tokenizer recognized.

use std::path::{Path, PathBuf};

use crate::BoundaryHints;

/// Index of the reading in an IPADIC feature string. ta-old skips a token whose
/// reading is `*`, absent, or empty — "If katakana is '*' or does not exist,
/// not real word, so don't penalize" (`ta-old/exe/util/Dictionary.cpp:1115-1121`).
const READING_FIELD: usize = 7;

/// Environment override for [`READING_FIELD`], for the §9.6 IPADIC-versus-UniDic
/// measurement at the Phase 0/1 boundary.
///
/// The index is a property of the dictionary, not of this code, and 7 is
/// IPADIC's. Measured against unidic-mecab-2.1.2: its field 7 is the **lemma**
/// (書字形), not a reading, and its surface pronunciation lives at 9. So a
/// UniDic dictionary read with 7 finds a non-empty, non-`*` value on every
/// token and the unknown-word guard below never fires — silent degradation with
/// no error anywhere, which is the hazard the master plan §9.6 predicted.
///
/// A constant plus an override rather than a proper per-dictionary property:
/// deciding where that property belongs is Phase 1's cost-model work, and this
/// exists to produce the number that informs it.
const READING_FIELD_ENV: &str = "TA_HINTS_READING_FIELD";

/// Resolved once per [`VibratoTokenizer::load`], not per sentence and not into
/// a global: a `OnceLock` would make the value untestable (the first test to
/// touch it would pin it for the whole process) and reading the environment per
/// token would cost a getenv per token.
fn reading_field_from_env() -> usize {
    std::env::var(READING_FIELD_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(READING_FIELD)
}

/// Positions where a word should not begin or end, derived from tokenization.
///
/// Indexed by **char** position, matching [`BoundaryHints`] and `segment.rs`.
/// Vibrato reports char ranges directly, so nothing here converts from bytes.
pub struct BoundaryFlags {
    bad_start: Vec<bool>,
    bad_end: Vec<bool>,
}

impl BoundaryHints for BoundaryFlags {
    fn bad_start(&self, pos: usize) -> bool {
        // Out of range is not an error: `segment.rs` queries positions derived
        // from match lengths, which may exceed the tokenized text.
        self.bad_start.get(pos).copied().unwrap_or(false)
    }

    fn bad_end(&self, pos: usize) -> bool {
        self.bad_end.get(pos).copied().unwrap_or(false)
    }
}

/// Derive flags from a tokenized worker. Port of `ta-old/exe/util/
/// Dictionary.cpp:1115-1126`.
///
/// For each token, the **interior** positions are marked: a word should not end
/// before the token's last char, nor start after its first. The token's own
/// boundaries stay free — the hint says "do not split inside this," not "split
/// here." A single-char token therefore marks nothing.
///
/// A token whose reading ([`READING_FIELD`]) is absent, `*`, or empty is
/// skipped entirely. That is ta-old's unknown-word guard: penalizing splits
/// inside a word the tokenizer only guessed at would be worse than staying
/// silent.
///
/// ta-old carries a second guard — a fuzzy re-match of the token against the
/// source, commented "I don't trust mecab all that much" — which is
/// deliberately **not** ported. It existed because ta-old drove MeCab through a
/// text pipe and had to re-find each token by scanning. Vibrato returns char
/// ranges into the exact string it was handed, so misalignment cannot occur and
/// the branch would be untestable.
///
/// The bounds checks below keep release builds total even if a future caller
/// breaks the "same text the worker was reset with" invariant; the
/// `debug_assert!` makes that mismatch loud in tests instead of silently
/// dropping flags.
fn flags_from_worker(
    worker: &vibrato::tokenizer::worker::Worker,
    char_len: usize,
    reading_field: usize,
) -> BoundaryFlags {
    let mut bad_start = vec![false; char_len];
    let mut bad_end = vec![false; char_len];

    for i in 0..worker.num_tokens() {
        let token = worker.token(i);
        let reading = token.feature().split(',').nth(reading_field);
        if !matches!(reading, Some(r) if !r.is_empty() && r != "*") {
            continue;
        }

        let range = token.range_char();
        debug_assert!(
            range.end <= char_len,
            "token range {range:?} exceeds char_len {char_len}"
        );
        for pos in range.start..range.end.saturating_sub(1) {
            if pos < char_len {
                bad_end[pos] = true;
            }
            if pos + 1 < char_len {
                bad_start[pos + 1] = true;
            }
        }
    }

    BoundaryFlags { bad_start, bad_end }
}

/// Token boundaries as the Engine v2 DP wants them: `set[p]` is true when a
/// token ends at character `p` and the next begins there.
///
/// `BoundaryFlags` answers "is this position interior to a token", which loses
/// single-character tokens entirely — master plan §4.3 cause 1, and the reason
/// `くるまではこう` merged `は`+`こう` for free. This answers the question the
/// constraint actually asks.
pub struct BoundarySet(Vec<bool>);

impl crate::engine::Boundaries for BoundarySet {
    fn is_boundary(&self, pos: usize) -> bool {
        self.0.get(pos).copied().unwrap_or(false)
    }
}

/// A loaded Vibrato dictionary, ready to tokenize.
///
/// Loading is expensive and reading the dictionary is not, so the two are
/// separate: a caller loads once and calls [`VibratoTokenizer::hints`] per text.
pub struct VibratoTokenizer {
    tokenizer: vibrato::Tokenizer,
    /// Which feature-string column carries the reading, for the unknown-word
    /// guard. Resolved at load time from [`READING_FIELD_ENV`].
    reading_field: usize,
}

impl VibratoTokenizer {
    /// Load an **uncompressed** compiled Vibrato dictionary from `path`.
    ///
    /// The distributed archive is `.tar.xz` containing a zstd-compressed
    /// `system.dic`; extracting it is deliberately out of scope (spec §5),
    /// which is what keeps `vibrato` this phase's only new dependency.
    pub fn load(path: &Path) -> Result<Self, HintsError> {
        let file = std::fs::File::open(path).map_err(|source| HintsError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let dict = vibrato::Dictionary::read(std::io::BufReader::new(file))
            .map_err(|e| HintsError::Dictionary(e.to_string()))?;
        Ok(Self {
            tokenizer: vibrato::Tokenizer::new(dict),
            reading_field: reading_field_from_env(),
        })
    }

    /// Tokenize `text` and derive its boundary flags.
    ///
    /// A fresh worker per call: workers are mutable scratch space, and sharing
    /// one would force `&mut self` on a method that is otherwise read-only.
    pub fn hints(&self, text: &str) -> BoundaryFlags {
        let mut worker = self.tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        flags_from_worker(&worker, text.chars().count(), self.reading_field)
    }

    /// Character-offset boundary set for `text`.
    pub fn boundaries(&self, text: &str) -> BoundarySet {
        let char_len = text.chars().count();
        let mut set = vec![false; char_len + 1];
        set[0] = true;
        set[char_len] = true;

        let mut worker = self.tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        for i in 0..worker.num_tokens() {
            let range = worker.token(i).range_char();
            set[range.start] = true;
            set[range.end] = true;
        }
        BoundarySet(set)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HintsError {
    #[error("reading the vibrato dictionary at {path} failed: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Vibrato's error, rendered. Carried as a `String` so `vibrato` does not
    /// become part of this crate's public API for anyone matching on it — the
    /// same reason `SourceError::Transport` holds a `String` rather than a
    /// `ureq` type.
    #[error("the vibrato dictionary could not be loaded: {0}")]
    Dictionary(String),
}

/// A compiled two-entry IPADIC-shaped dictionary, built in memory.
///
/// Deliberately not a fixture file: the real dictionary is 46 MB, and nothing
/// in this crate's tests may touch the network or ship a binary. These four
/// readers are the same inputs vibrato's own `compile` binary takes.
fn test_dictionary() -> vibrato::Dictionary {
    const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                       都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
    const MATRIX: &str = "1 1\n0 0 0\n";
    const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
    const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                       KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

    vibrato::SystemDictionaryBuilder::from_readers(
        LEX.as_bytes(),
        MATRIX.as_bytes(),
        CHAR.as_bytes(),
        UNK.as_bytes(),
    )
    .expect("the built-in test dictionary must build")
}

/// Write [`test_dictionary`] to `path`, for tests in crates that cannot reach
/// this module's private fixture.
///
/// `src-tauri` has to exercise the real [`VibratoTokenizer::load`] — its
/// startup resolution and its `watch` publication both carry a real tokenizer,
/// and a stub would prove neither — and the alternatives are a 46 MB file in
/// the repository or a network call inside the suite.
///
/// Serialized to memory first rather than through a `BufWriter`: a buffered
/// writer flushed only by its `Drop` swallows the error that matters here.
///
/// **Not part of the supported surface.** Nothing in a shipping build calls it;
/// it is `pub` because a `#[cfg(test)]` item is invisible to another crate.
#[doc(hidden)]
pub fn write_test_dictionary(path: &Path) -> Result<(), HintsError> {
    let mut bytes = Vec::new();
    test_dictionary()
        .write(&mut bytes)
        .map_err(|e| HintsError::Dictionary(e.to_string()))?;
    std::fs::write(path, &bytes).map_err(|source| HintsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_dictionary_tokenizes_a_known_sentence() {
        let dict = test_dictionary();
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("東京都");
        worker.tokenize();

        assert_eq!(worker.num_tokens(), 2, "expected 東京 + 都");
        assert_eq!(worker.token(0).surface(), "東京");
        assert_eq!(worker.token(0).range_char(), 0..2);
        assert_eq!(worker.token(1).surface(), "都");
        assert_eq!(worker.token(1).range_char(), 2..3);
        // Field 7 is IPADIC's reading. The derivation's guard depends on it.
        let reading = worker.token(0).feature().split(',').nth(READING_FIELD);
        assert_eq!(reading, Some("トウキョウ"));
    }

    /// Build flags for `text` using the built-in test dictionary, through
    /// [`VibratoTokenizer::hints`] rather than [`flags_from_worker`] directly,
    /// so every test using this helper also exercises the real method.
    fn flags_for(text: &str) -> BoundaryFlags {
        let tokenizer = VibratoTokenizer {
            tokenizer: vibrato::Tokenizer::new(test_dictionary()),
            reading_field: READING_FIELD,
        };
        tokenizer.hints(text)
    }

    /// A `VibratoTokenizer` whose dictionary recognizes exactly `words`, built
    /// the same way [`test_dictionary`] and
    /// [`a_token_without_a_reading_marks_nothing`]'s fixture are.
    /// `VibratoTokenizer::boundaries` never consults the reading field, so the
    /// feature columns after 名詞 don't matter here.
    fn tokenizer_over(words: &[&str]) -> VibratoTokenizer {
        let lex: String = words
            .iter()
            .map(|w| format!("{w},0,0,5000,名詞,一般,*,*,*,*,*,*,*\n"))
            .collect();
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        let dict = vibrato::SystemDictionaryBuilder::from_readers(
            lex.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("tokenizer_over dictionary must build");
        VibratoTokenizer {
            tokenizer: vibrato::Tokenizer::new(dict),
            reading_field: READING_FIELD,
        }
    }

    #[test]
    fn a_single_character_token_still_yields_both_its_boundaries() {
        // The failure master plan §4.3 names: `bad_start`/`bad_end` mark
        // interiors, so a one-character token marks nothing at all.
        let t = tokenizer_over(&["東京", "都"]);
        let b = t.boundaries("東京都");
        use crate::engine::Boundaries;
        assert!(b.is_boundary(0));
        assert!(b.is_boundary(2), "東京 ends and 都 begins at 2");
        assert!(b.is_boundary(3));
        assert!(!b.is_boundary(1), "1 is interior to 東京");
    }

    /// 東京 spans chars 0..2, so only its interior boundary is marked: a word
    /// may still end at 1 and start at 0, but not the reverse.
    #[test]
    fn a_multi_char_token_marks_only_its_interior() {
        let f = flags_for("東京都");

        assert!(f.bad_end(0), "0 is interior to 東京");
        assert!(f.bad_start(1), "1 is interior to 東京");

        assert!(
            !f.bad_start(0),
            "a word may start at the token's first char"
        );
        assert!(!f.bad_end(1), "a word may end at the token's last char");
    }

    /// 都 is one char, so its loop body never runs. An off-by-one here would
    /// silently penalize every single-char token in the language.
    #[test]
    fn a_single_char_token_marks_nothing() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(2), "都 must not mark its own start");
        assert!(!f.bad_end(2), "都 must not mark its own end");
    }

    #[test]
    fn empty_input_yields_empty_flags() {
        let f = flags_for("");
        assert!(!f.bad_start(0));
        assert!(!f.bad_end(0));
    }

    /// `segment.rs` queries `m.start + m.len - 1`, which can exceed what the
    /// tokenizer saw. A panic here would crash the DP.
    #[test]
    fn out_of_range_positions_are_false_not_a_panic() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(999));
        assert!(!f.bad_end(999));
    }

    /// A reading field pointing at a column the dictionary does not have makes
    /// the guard fire on every token, so no flags at all are derived. That is
    /// not hypothetical: `READING_FIELD` is 7 because that is IPADIC's reading
    /// column, and measured against unidic-mecab-2.1.2 at the Phase 0/1
    /// boundary (master plan §9.6) IPADIC read at field 9 disables every hint
    /// and scores exactly the no-hints baseline, silently. This pins the
    /// mechanism so that failure cannot be mistaken for "hints did not help".
    #[test]
    fn a_reading_field_the_dictionary_lacks_derives_no_flags_at_all() {
        let dict = test_dictionary();
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("東京都");
        worker.tokenize();

        let good = flags_from_worker(&worker, 3, READING_FIELD);
        assert!(
            good.bad_end(0) && good.bad_start(1),
            "the real field must derive 東京's interior"
        );

        let bad = flags_from_worker(&worker, 3, 99);
        let any = (0..3).any(|p| bad.bad_start(p) || bad.bad_end(p));
        assert!(
            !any,
            "a field the dictionary lacks must derive nothing, not partial flags"
        );
    }

    /// The override parses, and anything unparseable falls back rather than
    /// panicking — a typo in an env var must not take the parser down.
    #[test]
    fn the_reading_field_override_falls_back_on_garbage() {
        // Not via the environment: that is process-global and would race every
        // other test in this binary. The parse is the part worth pinning.
        let parse = |v: &str| v.parse::<usize>().ok().unwrap_or(READING_FIELD);
        assert_eq!(parse("9"), 9);
        assert_eq!(parse(""), READING_FIELD);
        assert_eq!(parse("seven"), READING_FIELD);
        assert_eq!(parse("-1"), READING_FIELD);
    }

    /// A token whose reading is `*` is a guess, and ta-old refuses to penalize
    /// splits inside a guess. Without this guard the DP would be pushed away
    /// from splitting inside anything the tokenizer failed to recognize.
    #[test]
    fn a_token_without_a_reading_marks_nothing() {
        const LEX: &str = "謎語,0,0,5000,名詞,一般,*,*,*,*,*,*,*\n";
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        let dict = vibrato::SystemDictionaryBuilder::from_readers(
            LEX.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("dictionary");
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("謎語");
        worker.tokenize();
        let f = flags_from_worker(&worker, 2, READING_FIELD);

        assert!(!f.bad_end(0), "a reading-less token must not be penalized");
        assert!(
            !f.bad_start(1),
            "a reading-less token must not be penalized"
        );
    }

    /// Task 3's `load_hints` tests need a file `VibratoTokenizer::load` accepts, in
    /// a crate that cannot reach this module's private fixture. The proof that the
    /// helper is worth anything is that its output loads and tokenizes.
    #[test]
    fn the_written_test_dictionary_loads_and_tokenizes() {
        let dir = scratch("write-test-dictionary");
        let path = dir.join("system.dic");

        write_test_dictionary(&path).expect("write");

        let tokenizer = VibratoTokenizer::load(&path).expect("load what we just wrote");
        let flags = tokenizer.hints("東京都");
        assert!(flags.bad_end(0) && flags.bad_start(1), "東京's interior");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-hints-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn an_absent_dictionary_is_an_io_error() {
        let dir = scratch("load-absent");
        let err = VibratoTokenizer::load(&dir.join("system.dic"))
            .err()
            .expect("must fail");
        assert!(matches!(err, HintsError::Io { .. }), "got {err:?}");
    }

    #[test]
    fn a_file_that_is_not_a_dictionary_is_a_dictionary_error() {
        let dir = scratch("load-garbage");
        let path = dir.join("system.dic");
        std::fs::write(&path, b"this is not a compiled dictionary").expect("write");

        let err = VibratoTokenizer::load(&path).err().expect("must fail");
        assert!(matches!(err, HintsError::Dictionary(_)), "got {err:?}");
    }

    /// The error must be actionable: it names the file it could not load.
    #[test]
    fn the_io_error_renders_usefully() {
        let dir = scratch("load-render");
        let err = VibratoTokenizer::load(&dir.join("system.dic"))
            .err()
            .expect("must fail");
        assert!(err.to_string().contains("system.dic"), "got {err}");
    }

    /// A unit check that the derivation produces non-empty, correctly-placed
    /// flags for ordinary input — individual rules are covered elsewhere, and a
    /// no-op implementation returning all-false would fail only this one.
    ///
    /// Superseded as the phase's central proof by
    /// `hints_change_which_segmentation_jparser_parse_returns` below, which
    /// shows the derivation actually reaches `parse`'s output rather than only
    /// being correct in isolation. Kept because it is cheaper to run and pins
    /// the derivation's shape directly.
    #[test]
    fn the_derivation_is_not_inert() {
        let text = "東京都";
        let f = flags_for(text);

        let any = (0..text.chars().count()).any(|p| f.bad_start(p) || f.bad_end(p));
        assert!(any, "the derivation produced no flags at all");

        assert!(
            f.bad_end(0) && f.bad_start(1),
            "東京's interior must be marked"
        );
    }

    /// The end-to-end proof, not just a unit check on the derivation: a real
    /// `jparser::parse` call whose *segmentation* changes when hints are
    /// supplied, versus not.
    ///
    /// A plain two-entry dictionary cannot force this: `score_match`'s
    /// `MATCH_BASE`/`SINGLE_CHAR_PENALTY` make "東京"+"都" and "東"+"京都" cost
    /// exactly the same on a bare noun dictionary, so `segment.rs`'s own
    /// tie-break ("last writer wins") already happens to land on the correct
    /// split with no hints at all — nothing would be proved. Giving 京都 a
    /// JMdict priority marker (`ke_pri`, which becomes the `COMMON`/
    /// `COMMON_LINE` bonus in `segment.rs`) breaks that tie in favor of the
    /// *wrong* split outright: a realistic failure mode, since dictionary
    /// frequency alone cannot tell "東"+"京都" from "東京"+"都". `flags_for`
    /// then supplies exactly the interior hint `the_derivation_is_not_inert`
    /// pins (`bad_end(0)`, `bad_start(1)`), which taxes both matches
    /// straddling that interior — "東" ending at 0 and "京都" starting at 1 —
    /// by ten points apiece, which is enough to flip the winner back.
    #[test]
    fn hints_change_which_segmentation_jparser_parse_returns() {
        const XML: &str = concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            "<JMdict>",
            "<entry><ent_seq>1</ent_seq><k_ele><keb>東</keb></k_ele>",
            "<r_ele><reb>ひがし</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>east</gloss></sense></entry>",
            "<entry><ent_seq>2</ent_seq><k_ele><keb>都</keb></k_ele>",
            "<r_ele><reb>みやこ</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>capital</gloss></sense></entry>",
            "<entry><ent_seq>3</ent_seq><k_ele><keb>東京</keb></k_ele>",
            "<r_ele><reb>とうきょう</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>Tokyo</gloss></sense></entry>",
            "<entry><ent_seq>4</ent_seq>",
            "<k_ele><keb>京都</keb><ke_pri>news1</ke_pri></k_ele>",
            "<r_ele><reb>きょうと</reb></r_ele>",
            "<sense><pos>&n;</pos><gloss>Kyoto</gloss></sense></entry>",
            "</JMdict>",
        );

        let table = crate::conjugation::ConjugationTable::load_embedded()
            .expect("the embedded conjugation table must load");
        let dir = scratch("e2e-segmentation-flip");
        let report = crate::index::build::build_from_reader(
            XML.as_bytes(),
            &table,
            &crate::stem::StemOptions::default(),
            &dir,
        )
        .expect("the fixture must build");
        assert_eq!(
            report.skipped_entries, 0,
            "the fixture must not be malformed"
        );
        let index = crate::index::load::Index::open(&dir).expect("open the built index");

        let text = "東京都";
        let opts = crate::ParseOptions::default();
        let without = crate::parse(&index, &table, text, &opts, None).expect("parse without hints");
        let flags = flags_for(text);
        let with =
            crate::parse(&index, &table, text, &opts, Some(&flags)).expect("parse with hints");

        let shape = |r: &crate::ParseResult| -> Vec<(usize, usize, String)> {
            r.segments
                .iter()
                .map(|s| (s.start, s.len, s.surface.clone()))
                .collect()
        };

        assert_eq!(
            shape(&without),
            vec![(0, 1, "東".to_string()), (1, 2, "京都".to_string())],
            "without hints, 京都's priority bonus must win the tie outright"
        );
        assert_eq!(
            shape(&with),
            vec![(0, 2, "東京".to_string()), (2, 1, "都".to_string())],
            "with hints, the interior penalty must flip the winner back"
        );
        assert_ne!(
            shape(&without),
            shape(&with),
            "the whole point: hints must change parse's own output, not just the derivation"
        );
    }
}
