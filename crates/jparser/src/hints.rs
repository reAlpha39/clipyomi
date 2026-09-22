// SPDX-License-Identifier: MIT OR Apache-2.0

//! Vibrato's token boundaries as [`crate::engine::Boundaries`]:
//! [`BoundarySet`], which [`VibratoTokenizer::boundaries`] returns, is one of
//! two production implementors; [`crate::engine::relax::Relaxed`] is the
//! other, and `engine::parse` wraps every boundary source in it.

use std::path::{Path, PathBuf};

/// Token boundaries as the DP wants them: `set[p]` is true when a token ends
/// at character `p` and the next begins there.
///
/// The advisory pair this replaced answered "is this position interior to a
/// token", which loses single-character tokens entirely — master plan §4.3
/// cause 1, and the reason `くるまではこう` merged `は`+`こう` for free. This
/// answers the question the constraint actually asks.
pub struct BoundarySet(Vec<bool>);

impl crate::engine::Boundaries for BoundarySet {
    fn is_boundary(&self, pos: usize) -> bool {
        self.0.get(pos).copied().unwrap_or(false)
    }
}

/// A loaded Vibrato dictionary, ready to tokenize.
///
/// Loading is expensive and reading the dictionary is not, so the two are
/// separate: a caller loads once and calls [`VibratoTokenizer::boundaries`]
/// per text.
pub struct VibratoTokenizer {
    tokenizer: vibrato::Tokenizer,
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
        // **A rejected lexicon is not an error.** Its connection ids are
        // IPADIC's, and Vibrato verifies them against whatever dictionary was
        // actually loaded, so someone pointing `TA_HINTS_DICT` at a different
        // one would otherwise lose boundary hints altogether — far worse than
        // losing eighty words. The re-read is the price of that fallback and
        // is paid only on the failing path.
        let dict = match with_user_lexicon(dict) {
            Ok(d) => d,
            Err(()) => {
                let file = std::fs::File::open(path).map_err(|source| HintsError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
                vibrato::Dictionary::read(std::io::BufReader::new(file))
                    .map_err(|e| HintsError::Dictionary(e.to_string()))?
            }
        };
        Ok(Self {
            tokenizer: vibrato::Tokenizer::new(dict),
        })
    }

    /// The connection ids the loaded dictionary gives `probe`, when it
    /// tokenizes it as exactly one token.
    ///
    /// Only `examples/shatter_sweep.rs` uses this, to generate
    /// [`USER_LEXICON`] with ids the connection matrix will accept rather than
    /// ids invented by hand.
    pub fn ids_of(&self, probe: &str) -> Option<(u16, u16)> {
        let mut w = self.tokenizer.new_worker();
        w.reset_sentence(probe);
        w.tokenize();
        (w.num_tokens() == 1).then(|| {
            let t = w.token(0);
            (t.left_id(), t.right_id())
        })
    }

    /// Character-offset boundary set for `text`.
    ///
    /// A fresh worker per call: workers are mutable scratch space, and sharing
    /// one would force `&mut self` on a method that is otherwise read-only.
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

/// Words JMdict holds that IPADIC does not, so the tokenizer cuts inside them
/// and the cost model then prefers a tiling of `COMMON` fragments —
/// `いちゃいちゃ` came back as `い` + `ちゃい` + `ちゃ`. The constraint in
/// `cost::is_allowed` already protects any word the tokenizer holds whole,
/// which is why `美少女` survives hints-on and shatters hints-off; these
/// entries give the tokenizer the rest.
///
/// See `docs/superpowers/2026-09-16-user-lexicon-spike.md` for why no
/// cost-model term can do this instead, and why the list is capped at 81.
const USER_LEXICON: &str = include_str!("../assets/hint_user_lexicon.csv");

/// [`USER_LEXICON`] with its comment lines removed — Vibrato's CSV reader has
/// no comment syntax, so the provenance lives in the file and is stripped
/// here.
fn user_lexicon_body() -> String {
    USER_LEXICON
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(
            "
",
        )
}

/// Apply [`USER_LEXICON`] to `dict`.
///
/// `Err` gives nothing back, because `reset_user_lexicon_from_reader` takes
/// the dictionary by value and drops it on failure — hence [`load`] re-reads
/// the file rather than trying to recover one here.
fn with_user_lexicon(dict: vibrato::Dictionary) -> Result<vibrato::Dictionary, ()> {
    dict.reset_user_lexicon_from_reader(Some(user_lexicon_body().as_bytes()))
        .map_err(|_| ())
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
    /// The asset is data, so its shape is worth pinning: Vibrato's CSV reader
    /// has no comment syntax and would take a `#` line as a surface.
    #[test]
    fn the_user_lexicon_strips_comments_and_keeps_well_formed_rows() {
        let body = user_lexicon_body();
        assert!(!body.is_empty(), "the bundled lexicon is not empty");
        assert!(
            !body.lines().any(|l| l.trim_start().starts_with('#')),
            "no comment survives into what Vibrato parses"
        );
        for line in body.lines() {
            let f: Vec<&str> = line.split(',').collect();
            assert!(f.len() >= 5, "surface,left,right,cost,feature...: {line}");
            assert!(!f[0].is_empty(), "a surface is present: {line}");
            f[1].parse::<u16>().expect("left id");
            f[2].parse::<u16>().expect("right id");
            f[3].parse::<i16>().expect("word cost");
        }
    }

    /// The cap is measured, not stylistic. At 81 entries every gold cell is
    /// identical to the digit; at 121 the corpus loses 2 spans, because
    /// Vibrato's Viterbi is global and a bigger user lexicon moves boundaries
    /// in regions where none of its entries appear. If this fails, re-run the
    /// gates in `docs/superpowers/2026-09-16-user-lexicon-spike.md` rather
    /// than raising the number.
    ///
    /// 83 -> 87 on 2026-09-16 for `なのに`, `へへ`, `あたし達` and `唐揚げ`, all
    /// the same shape as `ある日`/`その日`. Gates re-run at each step, every cell
    /// in both hint states identical to the digit.
    ///
    /// **87 -> 85 on 2026-09-18, and the direction is the point.** That
    /// comment used to end "the number will keep climbing until something
    /// structural closes it"; `cost.rs`'s `CONTENT_MATCH` closed half of it.
    /// `あたし達` and `唐揚げ` are the one-character-peel shape that term
    /// exists for and both now merge unaided -- measured by deleting the rows
    /// and re-running, every gold cell identical to the digit in both hint
    /// states. `なのに` and `へへ` stay, because both tile into COMMON
    /// *particles* and that is exactly the tiling the surcharge is withheld
    /// from. A dead row is not free: the cap below exists because Vibrato's
    /// Viterbi is global.
    ///
    /// **85 -> 88 on 2026-09-22** for `もしかして`, `もしかしたら` and
    /// `もしかすると`, the residue the `uk` cover exposed. Gates re-run: every
    /// gold cell in both hint states identical to the digit, and the ten-corpus
    /// A/B reads 153 occurrences recovered against 3 lost, all three a name
    /// beside the new token. The globality this cap exists for is what those
    /// three are.
    ///
    /// **88 -> 93 the same day** for `いない`, `できない`, `できなかった`,
    /// `できず` and `よさそう` -- the first inflected forms here, and the first
    /// rows to move a gold cell rather than hold it: hints-on strict 1694 ->
    /// 1696 and tolerant 1948 -> 1950, hints-off identical, snapshot identical.
    /// The ten-corpus A/B reads 832 occurrences recovered against about a dozen
    /// lost. The cap is still a measured number, not a budget: the sixth row
    /// considered (a protective `もったいない`) was left out precisely because
    /// it had not been through that A/B.
    #[test]
    fn the_user_lexicon_stays_within_its_measured_cap() {
        assert!(
            user_lexicon_body().lines().count() <= 93,
            "93 is the largest list measured against every gold cell"
        );
    }

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
    }

    /// A `VibratoTokenizer` whose dictionary recognizes exactly `words`, built
    /// the same way [`test_dictionary`] is. `VibratoTokenizer::boundaries`
    /// never consults the reading field, so the feature columns after 名詞
    /// don't matter here.
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

    /// Task 3's `load_hints` tests need a file `VibratoTokenizer::load` accepts, in
    /// a crate that cannot reach this module's private fixture. The proof that the
    /// helper is worth anything is that its output loads and tokenizes.
    #[test]
    fn the_written_test_dictionary_loads_and_tokenizes() {
        use crate::engine::Boundaries;

        let dir = scratch("write-test-dictionary");
        let path = dir.join("system.dic");

        write_test_dictionary(&path).expect("write");

        let tokenizer = VibratoTokenizer::load(&path).expect("load what we just wrote");
        let b = tokenizer.boundaries("東京都");
        assert!(b.is_boundary(2), "東京 ends and 都 begins at 2");
        assert!(!b.is_boundary(1), "1 is interior to 東京");
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
}
