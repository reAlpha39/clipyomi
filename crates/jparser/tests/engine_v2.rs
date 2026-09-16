// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.

//! Integration tests for the Engine v2 path.

use std::io::BufReader;
use std::path::{Path, PathBuf};

use jparser::WordFlags;

fn open_v2(dir: &Path) -> jparser::index::load::Index {
    jparser::index::load::Index::open(dir).expect("open v2 index")
}

const FIXTURE: &str = include_str!("fixtures/jmdict_subset.xml");
const SENTENCES: &str = include_str!("fixtures/parse_sentences.txt");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("jparser-engine-v2").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn build_v2(dir: &Path) {
    jparser::engine::build::build_from_reader(BufReader::new(FIXTURE.as_bytes()), dir)
        .expect("v2 build");
}

/// An index over the gold corpus's own JMdict subset rather than the small
/// curated one, for the one test whose subject is a gold sentence.
///
/// `FIXTURE` is 1,472 hand-picked entries and covers isolated probe words; a
/// gold sentence needs the 5,402 that `score` measures against, or a row
/// passes because nothing in it matched at all.
#[cfg(feature = "mecab")]
fn build_gold_v2(dir: &Path) {
    const GOLD_FIXTURE: &str = include_str!("fixtures/gold_jmdict_subset.xml");
    jparser::engine::build::build_from_reader(BufReader::new(GOLD_FIXTURE.as_bytes()), dir)
        .expect("gold v2 build");
}

#[test]
fn a_v2_index_opens_under_the_engines_fingerprint() {
    let dir = scratch("build");
    build_v2(&dir);
    assert!(open_v2(&dir).entry_count() > 0);
}

#[test]
fn an_index_built_under_another_fingerprint_is_rejected() {
    // The fingerprint guard, doing exactly what it exists for: a table
    // regeneration changes every class id an index stores, and `Index::open`
    // must refuse the stale index rather than misresolve classes silently.
    let dir = scratch("wrong-fingerprint");
    build_v2(&dir);
    let wrong = jparser::engine::build::fingerprint() ^ 1;
    let err = jparser::index::load::Index::open_with(&dir, wrong)
        .expect_err("a different fingerprint must not open this index");
    assert!(
        matches!(err, jparser::index::IndexError::ConjugationMismatch { .. }),
        "expected a fingerprint mismatch, got {err:?}"
    );
    open_v2(&dir);
}

fn cands(dir: &Path, text: &str) -> Vec<Vec<jparser::engine::lex::Candidate>> {
    let index = open_v2(dir);
    let table = jparser::engine::table::Table::new();
    let chars: Vec<char> = text.chars().collect();
    jparser::engine::lex::candidates(&index, &table, &chars).expect("candidates")
}

#[test]
fn a_plain_headword_is_found_at_its_position() {
    let dir = scratch("lex-plain");
    build_v2(&dir);
    let c = cands(&dir, "東京都に住む");
    assert!(c[0].iter().any(|x| x.headword == "東京" && x.len == 2));
}

#[test]
fn a_conjugated_verb_is_found_with_its_chain() {
    let dir = scratch("lex-conj");
    build_v2(&dir);
    let c = cands(&dir, "書いた");
    let hit = c[0]
        .iter()
        .find(|x| x.headword == "書く")
        .expect("書いた -> 書く");
    assert_eq!(
        hit.len, 3,
        "the candidate spans the whole conjugated surface"
    );
    assert!(!hit.steps.is_empty());
}

#[test]
fn a_kana_unified_match_is_marked_inexact() {
    // No deinflection fires here at all (see the gap test below) -- what
    // this exercises is a plain headword collision. The span "ハイ" folds to
    // the same FST key as the existing headword "はい" (a reading of 杯/盃,
    // ent_seq 2019640), and that stored surface differs from the raw
    // katakana query, so the match must come back marked inexact.
    let dir = scratch("lex-inexact");
    build_v2(&dir);
    let c = cands(&dir, "ハイタ");
    let hit = c[0]
        .iter()
        .find(|x| x.headword == "はい")
        .expect("ハイ -> はい (杯/盃's reading) is the kana-unified match");
    assert_eq!(
        hit.len, 2,
        "the match covers ハイ, not the whole katakana span"
    );
    assert!(!hit.exact, "はい (hiragana) != the raw katakana query ハイ");
}

/// Opens a fresh index, builds a fresh `Table`, and runs `parse` with
/// whatever boundary source `bounds` names.
fn parse_v2_with_bounds(
    dir: &Path,
    text: &str,
    bounds: Option<&dyn jparser::engine::Boundaries>,
) -> jparser::ParseResult {
    let index = open_v2(dir);
    let table = jparser::engine::table::Table::new();
    jparser::parse(
        &index,
        &table,
        text,
        &jparser::ParseOptions::default(),
        bounds,
    )
    .expect("parse")
}

/// `parse_v2_with_bounds` with no boundary source -- the shape every assemble
/// test below needs and nothing more.
fn parse_v2_over(dir: &Path, text: &str) -> jparser::ParseResult {
    parse_v2_with_bounds(dir, text, None)
}

/// Locates the real Vibrato IPADIC dictionary: `TA_HINTS_DICT` if set (the
/// same override `hints.rs`'s doc comments and the CLI's `--hints`/startup
/// path already use), else the path `jparser-cli fetch-hints` publishes to by
/// default on this machine.
///
/// `TA_HINTS_DICT` is the portable route and the only one anywhere but
/// Windows: the fallback is a `%APPDATA%` location, and the panic below used
/// to read "APPDATA must be set on Windows" — true, unhelpful, and reached
/// only when the reader is *not* on Windows. CI is the case that matters
/// (`.github/workflows/ci.yml` runs the `mecab` jobs on `ubuntu-latest`), so
/// the message names the fetch that fixes it.
#[cfg(feature = "mecab")]
fn dictionary_path() -> PathBuf {
    if let Ok(path) = std::env::var("TA_HINTS_DICT") {
        return PathBuf::from(path);
    }
    let Ok(appdata) = std::env::var("APPDATA") else {
        panic!(
            "no boundary-hint dictionary: TA_HINTS_DICT is unset and there is no APPDATA to \
             fall back to. Publish one with `cargo run -p jparser --features hints-source \
             --bin jparser-cli -- fetch-hints <dir>` and point TA_HINTS_DICT at its \
             `current/system.dic`."
        )
    };
    PathBuf::from(appdata)
        .join("dev.jparser.clipyomi")
        .join("hints")
        .join("current")
        .join("system.dic")
}

/// Real Vibrato boundaries for `text`, needed only by the two tests that pin
/// master plan success criterion 3 against genuine IPADIC tokenization rather
/// than a hand-built stand-in dictionary.
#[cfg(feature = "mecab")]
fn boundaries_over(text: &str) -> jparser::hints::BoundarySet {
    let path = dictionary_path();
    let tokenizer = jparser::hints::VibratoTokenizer::load(&path)
        .unwrap_or_else(|e| panic!("load the vibrato dictionary at {}: {e}", path.display()));
    tokenizer.boundaries(text)
}

#[cfg(feature = "mecab")]
#[test]
fn kurumadehakou_parses_as_four_words() {
    // Master plan success criterion 3, and the sentence this whole phase
    // exists for. Vibrato tokenizes it correctly and the v1 engine discards
    // that, producing くる / まで / はこう (§4.3).
    let dir = scratch("kurumadehakou");
    build_v2(&dir);
    let bounds = boundaries_over("くるまではこう");
    let r = parse_v2_with_bounds(&dir, "くるまではこう", Some(&bounds));
    let cuts: Vec<&str> = r.segments.iter().map(|s| s.surface.as_str()).collect();
    assert_eq!(cuts, ["くる", "まで", "は", "こう"]);
}

// Sentences reported from real use on 2026-09-15, all parsing badly with
// hints on. They fail for different reasons and are kept together so that a
// fix to one cannot be credited for another: the first is the engine's, the
// second is the dictionary's, and the third is the first again at a severity
// the first does not convey.
//
// A private fixture, for the reason `KURU_FIXTURE` gives — `jmdict_subset.xml`
// is extractor-generated and would drop hand-added entries on regeneration —
// and because the point of the second test is an entry that is *absent*, which
// a shared fixture could acquire at any time. Eight entries, hand-written
// rather than extracted, with glosses written here rather than copied.
//
// 良い is `adj-i` and not the `adj-ix` JMdict gives it. That is deliberate and
// it is a fixture simplification, not a claim: `adj-ix` maps to 形容詞・イイ,
// whose stem handling does not reach 良かった from the kanji stem, so an
// `adj-ix` fixture would make the hints-off control below fail for a reason
// that has nothing to do with hints. The real index carries an entry that does
// deinflect (measured: `良かった matched reading=よかった`), so `adj-i` is what
// reproduces the real index's behaviour here, and the defect under test is
// about spans the hint boundaries forbid rather than about which class found
// them.
#[cfg(feature = "mecab")]
const HINT_TOKEN_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
<!ENTITY adj-i "adjective (keiyoushi)">
<!ENTITY adj-pn "pre-noun adjectival (rentaishi)">
<!ENTITY adv "adverb (fukushi)">
<!ENTITY exp "expressions (phrases, clauses, etc.)">
<!ENTITY suf "suffix">
<!ENTITY v1 "Ichidan verb">
<!ENTITY prt "particle">
]>
<JMdict>
<entry><ent_seq>2000010</ent_seq>
<k_ele><keb>気持ち</keb></k_ele><r_ele><reb>きもち</reb></r_ele>
<sense><pos>&n;</pos><gloss>feeling</gloss></sense></entry>
<entry><ent_seq>2000020</ent_seq>
<k_ele><keb>良い</keb></k_ele><r_ele><reb>よい</reb></r_ele><r_ele><reb>いい</reb></r_ele>
<sense><pos>&adj-i;</pos><gloss>good</gloss></sense></entry>
<entry><ent_seq>2000030</ent_seq>
<r_ele><reb>この</reb></r_ele>
<sense><pos>&adj-pn;</pos><gloss>this</gloss></sense></entry>
<entry><ent_seq>2000040</ent_seq>
<r_ele><reb>くらい</reb></r_ele>
<sense><pos>&suf;</pos><gloss>about</gloss></sense></entry>
<entry><ent_seq>2000050</ent_seq>
<r_ele><reb>どう</reb></r_ele>
<sense><pos>&adv;</pos><gloss>how</gloss></sense></entry>
<entry><ent_seq>2000060</ent_seq>
<k_ele><keb>事</keb></k_ele><r_ele><reb>こと</reb></r_ele>
<sense><pos>&n;</pos><gloss>thing</gloss></sense></entry>
<entry><ent_seq>2000070</ent_seq>
<r_ele><reb>ない</reb></r_ele>
<sense><pos>&adj-i;</pos><gloss>nonexistent</gloss></sense></entry>
<entry><ent_seq>2000080</ent_seq>
<r_ele><reb>だろう</reb></r_ele>
<sense><pos>&exp;</pos><gloss>I guess</gloss></sense></entry>
<entry><ent_seq>2000090</ent_seq>
<r_ele><reb>させる</reb></r_ele>
<sense><pos>&v1;</pos><gloss>to make (someone) do</gloss></sense></entry>
<entry><ent_seq>2000100</ent_seq>
<r_ele><reb>あげる</reb></r_ele>
<sense><pos>&v1;</pos><gloss>to give</gloss></sense></entry>
<entry><ent_seq>2000110</ent_seq>
<r_ele><reb>から</reb></r_ele>
<sense><pos>&prt;</pos><gloss>because</gloss></sense></entry>
<entry><ent_seq>2000120</ent_seq>
<k_ele><keb>本当</keb></k_ele><r_ele><reb>ほんとう</reb></r_ele>
<sense><pos>&n;</pos><gloss>truth</gloss></sense></entry>
<entry><ent_seq>2000130</ent_seq>
<r_ele><reb>は</reb></r_ele>
<sense><pos>&prt;</pos><gloss>topic marker</gloss></sense></entry>
<entry><ent_seq>2000140</ent_seq>
<k_ele><keb>品</keb></k_ele><r_ele><reb>しな</reb></r_ele>
<sense><pos>&n;</pos><gloss>goods</gloss></sense></entry>
<entry><ent_seq>2000150</ent_seq>
<k_ele><keb>川</keb></k_ele><r_ele><reb>かわ</reb></r_ele>
<sense><pos>&n;</pos><gloss>river</gloss></sense></entry>
</JMdict>
"#;

#[cfg(feature = "mecab")]
fn build_hint_token_fixture(name: &str) -> PathBuf {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(HINT_TOKEN_FIXTURE.as_bytes()), &dir)
        .expect("hint-token fixture build");
    dir
}

/// Phase 4's first case, now fixed: **hints on and hints off agree.**
///
/// Reported 2026-09-15 from 「よ、良かったわよ......思ったよりも気持ち良かったし......」,
/// reduced here to the phrase that carries it. Before the escape, hints on
/// returned no matched span at all while hints off parsed it correctly — the
/// hint source, whose job is to supply boundary evidence to the DP, was
/// destroying a cover the DP had without it. Both halves are still asserted
/// in one test, because their agreeing is the property worth holding.
///
/// The mechanism, measured against the real index and dictionary:
///
/// ```text
/// vibrato tokens:  気持ち良かっ | た | し        boundaries 0, 6, 7, 8
/// longest key at 0: 気持ち                       3 chars
/// ```
///
/// IPADIC holds 気持ち良かっ as one token, because 気持ち良い is one word to it,
/// and no key in the index is long enough to cover that token. So the token is
/// unmatchable, [`jparser::engine::relax::Relaxed`] reopens its interior, and
/// 気持ち may end at 3 while 良かった starts there and ends on the real
/// boundary after た — one endpoint relaxed, one strict, which is why the
/// escape marks positions rather than spans.
#[cfg(feature = "mecab")]
#[test]
fn a_hint_token_longer_than_any_key_no_longer_hides_its_words() {
    const TEXT: &str = "気持ち良かったし";
    let dir = build_hint_token_fixture("hint-token-shatter");

    // Hints off: the cover the index really holds, and the answer a reader
    // wants. Asserted first so that a fixture that stopped reaching 良かった
    // fails here, naming its own cause, rather than making the defect below
    // look fixed.
    let off = parse_v2_over(&dir, TEXT);
    let off_spans: Vec<(&str, bool)> = off
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        off_spans,
        [("気持ち", true), ("良かった", true), ("し", false)],
        "hints off, this index covers the phrase"
    );

    // Hints on: the same answer, which is the whole point.
    let bounds = boundaries_over(TEXT);
    let on = parse_v2_with_bounds(&dir, TEXT, Some(&bounds));
    let on_spans: Vec<(&str, bool)> = on
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        on_spans, off_spans,
        "hints must not take away a cover the engine has without them"
    );
    assert_eq!(on.segments[1].reading.as_deref(), Some("よかった"));
}

/// The same defect as above, at the severity that makes it matter, and with
/// the trigger that makes it common: **an unknown word.**
///
/// The test above needs a hint token that happens to outrun the dictionary,
/// which reads like a corner case. This one does not. IPADIC groups a run it
/// does not recognise into a single unknown token, so one invented or
/// colloquial word — the app's whole input register is full of them — glues
/// itself to every ordinary word after it and takes them all down.
/// `どっぴゅん` is in no dictionary, and IPADIC answers
/// `どっ | ぴゅんさせてあげるから`: eleven characters in one unknown token,
/// holding させて, あげる and から, none of which can now be reached.
///
/// Hints off, the same index finds all three and leaves only the unknown word
/// unmatched, which is the right answer. Before the escape, hints on returned
/// the whole thirteen characters as one unmatched span — the two runs merged
/// and not one word survived. **Now the two hint states agree**, and the
/// unknown word is unmatched in both, which is what it should be: it is in no
/// dictionary.
///
/// Measured on the seven sentences this came from, against the real
/// 218k-entry index: hints match **194 of 299 characters, hints off 230**.
/// Four of the seven lose ground with hints on (−14, −11, −7, −4 characters)
/// and not one gains. That is the opposite sign to the gold corpus, where
/// hints take strict span accuracy 0.7342 → 0.8174, and the difference is
/// register: news and literature are in IPADIC, a visual novel's dialogue is
/// not. It is worth stating plainly, because §4.3 treats the hint source as
/// evidence and on this register it is closer to a veto.
///
/// The fix is the escape the test above names, and this was never a second
/// defect — it was the first one's blast radius.
///
/// An unknown word is the loudest trigger, not the only one, and reading this
/// test alone would narrow it too far: see
/// [`a_hint_token_that_is_not_a_key_no_longer_unmatches_its_words`], where the
/// token is three characters IPADIC knows perfectly well.
#[cfg(feature = "mecab")]
#[test]
fn an_unknown_word_run_no_longer_swallows_the_words_after_it() {
    const TEXT: &str = "どっぴゅんさせてあげるから";
    let dir = build_hint_token_fixture("unknown-word-run");

    // Hints off: the unknown word is unmatched, as it must be, and every word
    // behind it is found.
    let off = parse_v2_over(&dir, TEXT);
    let off_spans: Vec<(&str, bool)> = off
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        off_spans,
        [
            ("どっぴゅん", false),
            ("させて", true),
            ("あげる", true),
            ("から", true),
        ],
        "hints off, only the unknown word is unmatched"
    );

    // Hints on: the same answer. The unknown word keeps its own span and
    // takes nothing with it.
    let bounds = boundaries_over(TEXT);
    let on = parse_v2_with_bounds(&dir, TEXT, Some(&bounds));
    let on_spans: Vec<(&str, bool)> = on
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        on_spans, off_spans,
        "an unknown token must not reach past itself"
    );
}

/// The smallest form of the same defect, and the one that said how often it
/// could fire: **three characters, two of the commonest words in the
/// language, and a hint token IPADIC knows perfectly well.**
///
/// From 2026-09-15's fourth report, `......本当はちょっとしたいけど。`. IPADIC
/// holds 本当は as a single entry; JMdict does not hold it as a key, so the
/// token is 本当は and nothing in the index is long enough to cover it. Before
/// the escape, `本当` ended inside the token and `は` started inside it, both
/// were forbidden, and **the boundaries stayed right while only the matching
/// died** — the parse still cut 本当 | は, so nothing downstream looked wrong
/// and the reader simply got a phrase with no definitions under it. That is
/// what made this case worth its own test rather than a line in the one
/// above, and it is why the assertion below is on the pair rather than on the
/// cut.
///
/// It also corrects the shape of the problem. The unknown-word case reads as
/// a tokenizer failing on strange input; this one is two well-formed
/// dictionaries disagreeing about what a word is, which happens wherever
/// IPADIC's inventory holds a unit JMdict lacks — 本当は, 狙える,
/// ロイヤルネイビーガール and 気持ち良かっ are four from these reports alone.
/// The trigger is the vocabulary mismatch, and the unknown word is only its
/// loudest case.
///
/// The mismatch runs both ways, and only one way hurts. Where JMdict has the
/// longer unit — ということは, になって, both in these sentences — hints merely
/// split what the engine would have joined, and the words still match. It is
/// the other direction that unmatches them, because a span may swallow whole
/// hint tokens but may not start or end inside one.
///
/// Measured across the eighteen sentences reported on 2026-09-15 against the
/// real index, before the escape: hints matched **426 characters of 630,
/// hints off 487**. Nine of the eighteen lost ground with hints on and not
/// one gained. The Phase 4 handoff records the figure after.
#[cfg(feature = "mecab")]
#[test]
fn a_hint_token_that_is_not_a_key_no_longer_unmatches_its_words() {
    const TEXT: &str = "本当は";
    let dir = build_hint_token_fixture("hint-token-common-words");

    let off = parse_v2_over(&dir, TEXT);
    let off_spans: Vec<(&str, bool)> = off
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        off_spans,
        [("本当", true), ("は", true)],
        "hints off, both words are found"
    );
    assert_eq!(off.segments[0].reading.as_deref(), Some("ほんとう"));

    let bounds = boundaries_over(TEXT);
    let on = parse_v2_with_bounds(&dir, TEXT, Some(&bounds));
    let on_spans: Vec<(&str, bool)> = on
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        on_spans,
        [("本当", true), ("は", true)],
        "the boundaries were always right; now the matches survive too"
    );
    assert_eq!(on.segments[0].reading.as_deref(), Some("ほんとう"));
}

/// The other half of the escape, and the one that decides its shape: **a hint
/// token with no hiragana in it stays shut, and here the two hint states must
/// *disagree*.**
///
/// `品川` is the test case because the escape's first implementation failed
/// its gate on it. IPADIC holds the place as one token; JMdict does not hold
/// it as a key at all, so it is unmatchable by exactly the same test `本当は`
/// is, and its interior is full of one-character entries waiting to match.
/// Relaxed unconditionally, a reader who hovered 品川 got "goods" and
/// "river" — and across the gold corpus that shape cost 88 gold spans
/// (42 all-kanji tokens, 25 all-katakana, 21 all-digit) against 8 gained,
/// taking hints-on strict span accuracy from 1768 to 1713.
///
/// So the escape asks for one hiragana character anywhere in the token, which
/// is the only evidence the engine has that the token glues a content word to
/// something else rather than being one word. Hints off, nothing constrains
/// the DP and it takes the two single-character entries, because `UNMATCHED`
/// is 30 a character against `COVER`'s 1; hints on, the veto holds and the
/// place comes back whole and unglossed. **That asymmetry is the correct
/// answer, not a residual defect** — the tokenizer knows something here that
/// the dictionary does not, which is the same fact that makes hints worth
/// having.
#[cfg(feature = "mecab")]
#[test]
fn a_hint_token_with_no_hiragana_keeps_its_veto() {
    const TEXT: &str = "品川";
    let dir = build_hint_token_fixture("hint-token-proper-noun");

    // Hints off: the constraint does not exist, so both entries win a span.
    let off = parse_v2_over(&dir, TEXT);
    let off_spans: Vec<(&str, bool)> = off
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        off_spans,
        [("品", true), ("川", true)],
        "hints off, nothing forbids the two one-character matches"
    );

    // Hints on: one token, no hiragana, no escape.
    let bounds = boundaries_over(TEXT);
    let on = parse_v2_with_bounds(&dir, TEXT, Some(&bounds));
    let on_spans: Vec<(&str, bool)> = on
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        on_spans,
        [("品川", false)],
        "the place stays whole and unglossed rather than shattering"
    );
}

/// The contrast case — and the one place Phase 4's design guessed wrong.
///
/// Reported in the same breath as the sentence above, from
/// 「このくらいどうていうことないだろう」. The hints are followed exactly and
/// every boundary is right; `ていう` is not a key in the index, which holds
/// `っていう`, `て` and `てい` but not this reading. The source text is most
/// likely どうっていうこと with the small つ lost, so the reading may never be
/// worth an entry of its own.
///
/// The Phase 4 design listed this test under "expected not to move", on the
/// reasoning that a missing key is a data gap no segmentation change can
/// reach. **That was wrong, and the escape improved this case too.** `ていう`
/// is a token nothing covers, so its interior reopens, and inside it sits
/// `いう` — a real entry, and the right reading of this text, since the
/// underlying phrase is と + いう. What is left unmatched is `て`, the half
/// that lost its small つ. So the parse went from three characters with no
/// gloss to one, and the reader now gets a definition for the verb.
///
/// The lesson is narrow and worth keeping: a span can be unmatchable for want
/// of a key *and* be inside a token that hides a shorter key, and "the
/// dictionary is missing this" does not imply "nothing in the engine can
/// help". The design's §4.2 is corrected rather than deleted, because the
/// reasoning that produced the wrong prediction is the interesting part.
#[cfg(feature = "mecab")]
#[test]
fn a_reading_the_index_lacks_still_yields_the_word_inside_it() {
    const TEXT: &str = "このくらいどうていうことないだろう";
    let dir = build_hint_token_fixture("hint-token-gap");

    let bounds = boundaries_over(TEXT);
    let r = parse_v2_with_bounds(&dir, TEXT, Some(&bounds));
    let spans: Vec<(&str, bool)> = r
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        spans,
        [
            ("この", true),
            ("くらい", true),
            ("どう", true),
            ("て", false),
            ("いう", true),
            ("こと", true),
            ("ない", true),
            ("だろう", true),
        ],
        "the escape reaches the verb inside the token the index cannot cover"
    );
}

/// The control for the two tests below, and the reason they can name a cause.
///
/// One entry, nothing to compete with it. If `ミスった` comes out whole here —
/// and it does — then deinflecting a katakana-stemmed godan verb works, the
/// conjugation trie reaches った, and whatever goes wrong against the real
/// index is about ranking rather than about reach.
const LONE_VERB_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY v5r "Godan verb with 'ru' ending">
]>
<JMdict>
<entry><ent_seq>3000010</ent_seq><r_ele><reb>ミスる</reb></r_ele>
<sense><pos>&v5r;</pos><gloss>to make a mistake</gloss></sense></entry>
</JMdict>
"#;

/// A second known defect, from 2026-09-15's third reported sentence:
/// 「(場所選択ミスったかもな......普通に空き教室にすれば良かった)」. Every
/// other word in it parses correctly in both hint states; `ミスった` does not.
///
/// **This is not the katakana gap.** `katakana_conjugation_is_not_deinflected_yet`
/// below documents a deliberate one — the conjugation trie is hiragana-keyed,
/// so a wholly katakana surface like ハイタ cannot reach a suffix match — and
/// it is easy to assume this is that. It is not: the suffix here is った,
/// which is hiragana, and the control below proves the deinflection succeeds
/// when nothing competes with it. What fails is the ranking.
///
/// Against the curated subset the engine prefers **ミス + った**, two short
/// matches, over the one conjugated match that explains the same four
/// characters. `ミス` is a common noun and `った` reaches ったい, so the pair
/// is cheap; `ミスる` in its 連用形-促音便 past is one candidate and loses.
/// The correct parse is one span, `ミスる` Past.
///
/// Measured against the real 218k-entry index too, where it is the same
/// answer with different debris: hints off `ミス` + `った`, hints on `ミスっ`
/// + `た`. So this is not an artefact of the subset's size.
///
/// A fix belongs with the cost model rather than with `lex`, and like the
/// hint-token defect above it moves §5.2's cells, so it wants a phase. When
/// one lands this test fails, which is the signal to re-measure.
#[test]
fn a_conjugated_katakana_verb_loses_to_its_shorter_parts() {
    // Control first: with nothing to compete, the deinflection is found.
    let lone = scratch("lone-verb");
    jparser::engine::build::build_from_reader(BufReader::new(LONE_VERB_FIXTURE.as_bytes()), &lone)
        .expect("lone-verb build");
    let whole = parse_v2_over(&lone, "ミスった");
    assert_eq!(whole.segments.len(), 1, "control: {:?}", whole.segments);
    assert!(whole.segments[0].matched);
    assert_eq!(whole.segments[0].surface, "ミスった");
    assert_eq!(whole.segments[0].entries[0].headword, "ミスる");
    assert_eq!(
        whole.segments[0].entries[0].conjugation.as_deref(),
        Some("Past"),
        "control: the past form is reached through a real conjugation step"
    );

    // The defect: the same word, the same entry present, against a fixture
    // that also holds ミス and ったい.
    let dir = scratch("katakana-verb-split");
    build_v2(&dir);
    let parsed = parse_v2_over(&dir, "ミスった");
    let split: Vec<(&str, bool)> = parsed
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        split,
        [("ミス", true), ("った", true)],
        "known defect: two short matches beat the one conjugated match"
    );
}

/// The sharper half of the defect above, and the one that is hard to argue is
/// anything but wrong: **the engine declines an exact dictionary-form match.**
///
/// `ミスる` is an entry. Parsed on its own against the curated subset with no
/// hints, it comes back as `ミス` + `る` — a common noun plus a verb-forming
/// suffix — rather than as the single headword the index holds verbatim. No
/// conjugation, no deinflection, no ambiguity about where the word ends: the
/// ranking simply prefers the pair.
///
/// With hints it is correct, and that is worth pinning too, because it says
/// where the missing evidence is rather than only that some is missing:
/// IPADIC holds ミスる as one token, `cost::is_allowed` then forbids a span
/// ending inside it, and the split becomes illegal rather than merely
/// expensive. The engine is relying on the hint source to rule out a
/// segmentation its own cost model ranks above the right one.
#[cfg(feature = "mecab")]
#[test]
fn the_dictionary_form_itself_loses_to_its_parts_without_hints() {
    let dir = scratch("dictionary-form-split");
    build_v2(&dir);

    let bare = parse_v2_over(&dir, "ミスる");
    let off: Vec<(&str, bool)> = bare
        .segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect();
    assert_eq!(
        off,
        [("ミス", true), ("る", true)],
        "known defect: an exact headword loses to noun + suffix"
    );

    let bounds = boundaries_over("ミスる");
    let on = parse_v2_with_bounds(&dir, "ミスる", Some(&bounds));
    assert_eq!(on.segments.len(), 1, "hints on: {:?}", on.segments);
    assert_eq!(on.segments[0].surface, "ミスる");
    assert!(on.segments[0].matched);

    // And the past form is *not* rescued the same way -- the hints move it
    // from one wrong answer to another, which is why the two cases are
    // pinned apart.
    let past_bounds = boundaries_over("ミスった");
    let past_result = parse_v2_with_bounds(&dir, "ミスった", Some(&past_bounds));
    let past: Vec<&str> = past_result
        .segments
        .iter()
        .map(|s| s.surface.as_str())
        .collect();
    assert_eq!(
        past,
        ["ミスっ", "た"],
        "known defect: hints on, the stem and its tail stay apart"
    );
}

/// The cut offsets a `ParseResult`'s own segments imply: every `start`, plus
/// the final segment's `start + len`. The intermediate ends need no entry of
/// their own -- a `ParseResult` is a contiguous cover, so every segment's end
/// is the next one's start.
#[cfg(feature = "mecab")]
fn segment_cuts(segments: &[jparser::Segment]) -> Vec<usize> {
    let mut out: Vec<usize> = segments.iter().map(|s| s.start).collect();
    if let Some(last) = segments.last() {
        out.push(last.start + last.len);
    }
    out
}

/// One gold record, reduced to what
/// [`the_particle_absorbing_merges_are_gone`] asks of it.
#[cfg(feature = "mecab")]
struct GoldSentence {
    source: String,
    text: String,
    /// Every adjudicated boundary: each span's `start`, plus each span's end.
    cuts: Vec<usize>,
}

/// The committed gold corpus, as sentences with their adjudicated boundaries.
///
/// Read rather than transcribed: the fixture names a `source` and the
/// sentence itself stays in exactly one place, so a corpus edit cannot leave
/// this test asserting against a sentence the corpus no longer holds. A
/// missing `source` is a panic below, never a skip -- the same rule
/// `jparser-cli score` applies to an adjudicated sentence absent from the
/// corpus.
#[cfg(feature = "mecab")]
fn gold_records() -> Vec<GoldSentence> {
    const GOLD: &str = include_str!("fixtures/gold.jsonl");
    GOLD.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).expect("gold.jsonl is JSONL");
            let spans = v["spans"].as_array().expect("spans");
            let mut cuts: Vec<usize> = Vec::with_capacity(spans.len() + 1);
            for span in spans {
                let start = span["start"].as_u64().expect("start") as usize;
                let len = span["len"].as_u64().expect("len") as usize;
                cuts.push(start);
                cuts.push(start + len);
            }
            cuts.sort_unstable();
            cuts.dedup();
            GoldSentence {
                source: v["source"].as_str().expect("source").to_string(),
                text: v["text"].as_str().expect("text").to_string(),
                cuts,
            }
        })
        .collect()
}

/// `needle`'s first occurrence in `text` as a **character** offset, which is
/// the unit every boundary in this crate is expressed in.
#[cfg(feature = "mecab")]
fn char_index_of(text: &str, needle: &str) -> Option<usize> {
    text.find(needle).map(|b| text[..b].chars().count())
}

#[cfg(feature = "mecab")]
#[test]
fn the_particle_absorbing_merges_are_gone() {
    // Handoff §9.5's defect class: jparser absorbing a preceding particle
    // into the word after it and landing on a kana-unified headword that is
    // semantically absurd -- `は`+`いた` as 履く, `も`+`もう` as 揉む,
    // `が`+`いった` as 甲斐. Neither span metric can catch it. Strict scores
    // it as an ordinary miss, and tolerant actively *credits* it, because the
    // merge's endpoints are both gold boundaries -- master plan §5.2 records
    // the ~1.3 points it over-forgives for exactly this class, and refuses to
    // add a heuristic to the metric to fix it. This fixture is the only guard
    // that exists.
    //
    // Each row is measured inside a committed gold sentence that contains the
    // surface and whose adjudication puts a boundary strictly inside it; the
    // fixture's fourth column names it, and all 26 rows have one. The
    // sentences are the corpus's, not invented here.
    //
    // Until 2026-09-09 the subject was the bare surface, and that could not
    // measure the class. Given two characters and no context, Vibrato calls
    // はこ, うん and んで one token each, so `cost::cost`'s crossing count --
    // which looks strictly *between* a span's endpoints -- finds no interior
    // boundary and neither CROSS nor PRONOUNCE_CROSS can fire at any
    // magnitude (measured flat at 0, 1, 2, 5, 10 and 25; Task 16b, handoff
    // §10). Those three rows plus いんじゃない, whose じゃ is absent from the
    // small curated `jmdict_subset.xml`, were the four failures this test was
    // ignored for, and all four were properties of the subject rather than
    // defects in the engine. The index is built from `gold_jmdict_subset.xml`
    // for both reasons: it carries じゃ (four real JMdict entries) and it
    // covers the gold sentences, so a row cannot pass because nothing
    // matched.
    //
    // Two assertions, because either alone is weak. The absorption check is
    // §9.5's literal signature. The interior-cut check is its positive form
    // and is what stops the first passing vacuously.
    //
    // The interior-cut check asks for *a* boundary inside the surface, not
    // for the gold one. That is deliberate and it is not free: measured
    // 2026-09-09, 24 of the 26 rows land on the gold boundary exactly, and
    // はいた (`d:a:003521-6`) and タッとい (`a:045670-4`) cut inside the
    // surface at a position gold does not agree with. Those two are ordinary
    // span misses, already counted in strict span accuracy's 0.8031, and
    // requiring the exact offset here would fail this test for a reason it
    // does not exist to measure. What it exists to measure is whether the
    // particle got absorbed, and the answer for all 26 is no.
    //
    // Vibrato's own cut set is deliberately no longer the standard: a gold
    // boundary is adjudicated where a tokenizer's is another opinion, and
    // `cost::is_allowed` already forbids a span starting or ending inside a
    // Vibrato token structurally, so asserting it here measured the
    // constraint rather than the engine.
    const FIXTURE_SPURIOUS: &str = include_str!("fixtures/spurious_merges.txt");
    let dir = scratch("spurious");
    build_gold_v2(&dir);
    let gold = gold_records();

    let mut wrong = Vec::new();
    let mut rows = 0;
    for line in FIXTURE_SPURIOUS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let mut cols = line.split('\t');
        let surface = cols.next().expect("column 1 is the surface");
        let source = cols
            .nth(2)
            .unwrap_or_else(|| panic!("{surface}: no fourth column naming a gold sentence"));
        rows += 1;

        let record = gold.iter().find(|r| r.source == source).unwrap_or_else(|| {
            panic!("{surface}: gold.jsonl carries no sentence with source {source}")
        });
        let at = char_index_of(&record.text, surface)
            .unwrap_or_else(|| panic!("{surface} does not occur in {source}"));
        let len = surface.chars().count();

        // Re-derived rather than trusted: if a corpus edit moved the boundary
        // out of the surface, this row would assert nothing, and a row that
        // asserts nothing must fail loudly instead of passing quietly.
        let gold_inside: Vec<usize> = record
            .cuts
            .iter()
            .copied()
            .filter(|&c| c > at && c < at + len)
            .collect();
        assert!(
            !gold_inside.is_empty(),
            "{surface} at {at} in {source}: the gold segmentation no longer puts a \
             boundary inside it, so this row cannot assert a split -- repoint the \
             fixture's fourth column"
        );

        let bounds = boundaries_over(&record.text);
        let r = parse_v2_with_bounds(&dir, &record.text, Some(&bounds));
        let absorbed = r
            .segments
            .iter()
            .any(|s| s.matched && s.start == at && s.len == len);
        let cuts = segment_cuts(&r.segments);
        let interior = cuts.iter().any(|&c| c > at && c < at + len);

        if absorbed || !interior {
            let surfaces: Vec<&str> = r.segments.iter().map(|s| s.surface.as_str()).collect();
            let reason = if absorbed {
                "absorbed into one matched segment"
            } else {
                "no segment boundary inside it"
            };
            wrong.push(format!(
                "{surface} at {at} in {source}: {reason}; gold cuts inside at \
                 {gold_inside:?}, parse cut at {cuts:?} {surfaces:?}"
            ));
        }
    }

    // The fixture's own count, asserted so a truncated or mis-parsed file
    // cannot make this test pass by measuring nothing. The fixture header
    // explains why 26 is a lower bound on the class rather than a closed
    // count; it is still an exact count of this file.
    assert_eq!(rows, 26, "the fixture should hold 26 rows");
    assert!(
        wrong.is_empty(),
        "particle-absorbing merges are back:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn the_result_is_a_contiguous_cover_in_ascending_order() {
    let dir = scratch("cover");
    build_v2(&dir);
    for text in ["東京都に住む", "」、「", "", "書いた本を読んだ"] {
        let r = parse_v2_over(&dir, text);
        let mut at = 0;
        for s in &r.segments {
            assert_eq!(s.start, at);
            at += s.len;
        }
        assert_eq!(at, text.chars().count());
    }
}

#[test]
fn a_matched_segment_carries_its_entry_and_senses() {
    // Not 東京 (the brief's own example, and `a_compound_of_two_common_kanji_
    // should_not_shatter` below's): a distinct fixture word so this test's
    // entry/senses assertions don't just duplicate that test's shattering
    // assertion. 学校 has no competing split either way: 学 alone is COMMON
    // but 校 alone is not, so the one-match/two-match costs tie under
    // `cost.rs`'s length-proportional COMMON (`COMMON * cand.len`, Task 16b)
    // and `lattice::relax`'s longer-run tie-break keeps it whole --
    // confirmed against this fixture directly rather than assumed. (Before
    // Task 16b's fix, COMMON was a flat per-match discount, and 東京 would
    // have shattered here for the same reason it once did in the other
    // test -- that historical reason for avoiding 東京 no longer applies,
    // but 学校 stays for the reason above.)
    let dir = scratch("assemble-basic");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "学校");
    let seg = &r.segments[0];
    assert!(seg.matched);
    assert_eq!(seg.surface, "学校");
    assert!(!seg.entries.is_empty());
    assert!(!seg.entries[0].senses.is_empty());
    assert!(!seg.entries[0].pos.is_empty());
    // ent_seq 1206730 in tests/fixtures/jmdict_subset.xml, <keb>学校</keb>.
    assert_eq!(
        seg.entries[0].entry_id, 1206730,
        "the ent_seq entry top-1 is scored on"
    );
}

// The general shape, not just this one word: with `COVER` priced per
// character, a flat *per-match* discount (`COMMON`, -2) rewards more matches
// over fewer, and a flat per-match penalty (`INEXACT`, +8) rewards fewer
// over more -- exactly the "fewer words wins" bug this engine exists to
// remove, reintroduced from the discount side. The model is
// granularity-neutral only when every candidate on both sides of a split
// carries identical per-match flags, which real text never guarantees.
// Common single kanji are everywhere in Japanese, so this generalises well
// past 東京.
//
// Task 16b's fix: `COMMON` is now applied as `COMMON * cand.len` in
// `cost::cost` (length-proportional, not flat). 東京 as one COMMON match now
// costs `2*1 - 2*2 = -2`; 東(COMMON) + 京(COMMON) as two COMMON matches costs
// `(1-2) + (1-2) = -2` too -- a tie, and `lattice::relax`'s longer-run
// tie-break keeps 東京 whole. See `cost.rs`'s `COMMON` doc comment and
// handoff §10 for the full trial log against the 2,163-span gold
// corpus.
#[test]
fn a_compound_of_two_common_kanji_should_not_shatter() {
    let dir = scratch("assemble-compound-shatter");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "東京");
    assert_eq!(r.segments.len(), 1, "東京 must not shatter into 東 + 京");
    assert_eq!(r.segments[0].surface, "東京");
}

#[test]
fn the_v2_path_labels_a_conjugated_entry_from_its_steps() {
    let dir = scratch("assemble-steps");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "書いた");
    let e = &r.segments[0].entries[0];
    assert_eq!(e.headword, "書く");
    assert!(
        !e.steps.is_empty(),
        "Phase 1a hands the formatter the steps"
    );
    assert_eq!(
        e.conjugation.as_deref(),
        Some("Past"),
        "Phase 1b-i names them"
    );
}

#[test]
fn an_unmatched_run_has_no_entries_and_no_reading() {
    let dir = scratch("assemble-unmatched");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "」、「");
    assert!(r
        .segments
        .iter()
        .any(|s| !s.matched && s.entries.is_empty() && s.reading.is_none()));
}

#[test]
fn pos_is_the_union_of_every_sense_in_first_seen_order() {
    let dir = scratch("assemble-pos");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "学校"); // see the fragmentation note above -- 東京 splits
    let pos = &r.segments[0].entries[0].pos;
    let mut seen: Vec<&String> = Vec::new();
    for code in pos {
        assert!(
            !seen.contains(&code),
            "{code} appears twice in {pos:?}, not just adjacently deduplicated"
        );
        seen.push(code);
    }
}

// The candidate at `start` whose span reaches the end of `text` -- what
// `chain.rs`'s tests need to pin `steps_for`'s contract. Takes `dir` (unlike
// the brief's two-argument sketch) so each test can build into its own
// scratch subdirectory, the same reason `cands` above does: `scratch` wipes
// its directory on every call, and cargo runs tests in parallel.
fn candidate_for(dir: &Path, text: &str, start: usize) -> jparser::engine::lex::Candidate {
    let total = text.chars().count();
    cands(dir, text)[start]
        .iter()
        .find(|c| c.start + c.len == total)
        .unwrap_or_else(|| panic!("no candidate at {start} spanning to the end of {text:?}"))
        .clone()
}

#[test]
fn steps_are_innermost_first() {
    // 食べさせられませんでした: the verb's own inflection first, then each
    // auxiliary outward. Phase 1b renders left to right from this order.
    let dir = scratch("chain-innermost");
    build_v2(&dir);
    let cand = candidate_for(&dir, "食べさせられませんでした", 0);
    let steps = jparser::engine::chain::steps_for(&cand);
    assert!(steps.len() >= 5);
    assert!(steps[0].range.start <= steps[1].range.start);
    assert_eq!(steps[0].range.start, 2, "食べ is the stem");
    assert_eq!(steps.last().unwrap().range.end, 12);
}

#[test]
fn the_ranges_tile_the_conjugated_tail_without_gaps() {
    let dir = scratch("chain-tiling");
    build_v2(&dir);
    let cand = candidate_for(&dir, "食べさせられませんでした", 0);
    let steps = jparser::engine::chain::steps_for(&cand);
    for pair in steps.windows(2) {
        assert_eq!(pair[0].range.end, pair[1].range.start, "no gap, no overlap");
    }
}

#[test]
fn an_uninflected_match_has_no_steps() {
    let dir = scratch("chain-uninflected");
    build_v2(&dir);
    let cand = candidate_for(&dir, "東京", 0);
    assert!(jparser::engine::chain::steps_for(&cand).is_empty());
}

#[test]
fn the_ranges_are_absolute_not_relative_to_the_spans_start() {
    // The brief's original name for this test ("...offset by the span's
    // start") assumed `steps_for` shifts each range by `cand.start`. It
    // doesn't -- see `chain.rs`'s contract doc. 食べ sits at absolute
    // positions 2..4 regardless of where the candidate's own span begins,
    // so た's step starts at 4, not at 4 + 2.
    let dir = scratch("chain-absolute");
    build_v2(&dir);
    let cand = candidate_for(&dir, "私は食べた", 2);
    let steps = jparser::engine::chain::steps_for(&cand);
    assert_eq!(steps[0].range.start, 4, "食べ starts at 2, た at 4");
}

#[test]
fn katakana_conjugation_is_not_deinflected_yet() {
    // Deliberate gap, not a bug, kept for Phase 1a: `Table`'s conjugation
    // trie is hiragana-keyed with no folding in `Table::search`, so a fully
    // katakana surface can never reach a Terminal/Continuation node through
    // an actual suffix match. Folding the trie too would fix that, but it
    // would also turn every katakana loanword ending in タ (データ, メーター,
    // カルタ) into a spurious past-tense candidate -- exactly the
    // kana-unified false-match class handoff §9.5 already measured as this
    // engine's worst defect. Handoff §8's deferred katakana-deinflection
    // item is what would reopen this trade-off: Tier D's 8 sentences / 169
    // spans showed no visible penalty from leaving it unfolded, but that
    // sample is too thin to call it settled either way.
    //
    // `chains_ending_at` separately always offers a zero-consumption
    // substitution for any class whose own form has an empty suffix (an
    // ichidan-family bare 連用形 stem, e.g.) -- that fires for every input
    // regardless of script, since it never compares a character at all, so
    // it is unrelated to this gap. The check below is scoped to steps that
    // actually consumed a character, which is what "deinflected" means here.
    let dir = scratch("lex-katakana-gap");
    build_v2(&dir);
    let c = cands(&dir, "ハイタ");
    assert!(
        c.iter()
            .flatten()
            .all(|x| x.steps.iter().all(|s| s.range.is_empty())),
        "no candidate should explain any of ハイタ's characters through a real conjugation ending"
    );
}

// Not 東京 (the brief's own example): under the tuned cost model (Task 16b),
// 東京 no longer shatters in this fixture -- `a_compound_of_two_common_kanji_
// should_not_shatter` now passes rather than staying ignored -- so
// `segments[0]` for it is 東京 as one two-character match, not 東 alone.
// That still does not exercise what this test means to: it needs a single
// *one-character* span with two colliding candidates of different flags,
// not a whole compound. る is used instead:
// ent_seq 2249020 (kanji 流, reading る -- a PRONOUNCE candidate) and ent_seq
// 2849996 (kana-only suffix る -- a PRIMARY candidate) collide on this exact
// one-character span, neither carries a priority tag, and both are reached
// by a direct, unfolded lookup, so `order`'s COMMON tier never interferes
// with the PRIMARY/PRONOUNCE clause this test is about.
//
// This is an end-to-end check against a real fixture; what actually proves
// the PRIMARY-before-PRONOUNCE clause in isolation (a hand-built trap where
// a lower-priority signal favors the wrong candidate) is
// `assemble::tests::order_puts_primary_before_pronounce_regardless_of_id`.
#[test]
fn an_exact_match_outranks_an_inexact_one() {
    let dir = scratch("order-exact");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "る");
    let e = &r.segments[0].entries;
    let has_primary = e.iter().any(|x| x.flags.contains(WordFlags::PRIMARY));
    let has_pronounce = e.iter().any(|x| x.flags.contains(WordFlags::PRONOUNCE));
    assert!(
        has_primary && has_pronounce,
        "fixture must offer both kinds or this test proves nothing"
    );
    let first_inexact = e
        .iter()
        .position(|x| x.flags.contains(WordFlags::PRONOUNCE));
    let last_exact = e.iter().rposition(|x| x.flags.contains(WordFlags::PRIMARY));
    assert!(
        last_exact.expect("checked above") < first_inexact.expect("checked above"),
        "every PRIMARY entry precedes every PRONOUNCE one"
    );
}

#[test]
fn the_order_is_stable_and_total() {
    let dir = scratch("order-stable");
    build_v2(&dir);
    let a = parse_v2_over(&dir, "東京都に住む");
    let b = parse_v2_over(&dir, "東京都に住む");
    assert_eq!(a, b, "two identical parses order identically");
}

// Not は (the brief's own example): this fixture carries exactly one
// PARTICLE-flagged entry for は (ent_seq 2028920), so filtering on
// `WordFlags::PARTICLE` there leaves a single-element list and the sort
// assertion below would pass vacuously. で offers four -- ent_seq 2028980
// (COMMON), 2654270 and 2844355 (neither COMMON, both an uninflected
// zero-step match on the exact surface), and 2854413 (inexact -- its
// stored surface is デ, reached only through kana folding). 2654270 and
// 2844355 tie on every earlier clause (exact, COMMON, PRIMARY, chain
// length), so ascending `entry_id` is the only thing deciding 2654270
// before 2844355 between them.
//
// This fixture's per-key candidate order already happens to run ascending
// by `entry_id` before `order` touches it (JMdict's source document is
// itself `ent_seq`-ordered), so this test passes whether or not the
// tiebreak clause runs at all -- it is an end-to-end sanity check, not
// proof. What actually proves the clause is
// `assemble::tests::order_breaks_ties_on_entry_id_ascending`, a unit test
// against `order` directly with hand-built candidates handed in descending
// id order (and identical on every other field), which can only pass if
// the ascending-`entry_id` tiebreak is really being applied.
#[test]
fn ties_break_on_entry_id_ascending() {
    let dir = scratch("order-ties");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "で");
    let ids: Vec<u32> = r.segments[0]
        .entries
        .iter()
        .filter(|e| e.flags.contains(WordFlags::PARTICLE))
        .map(|e| e.entry_id)
        .collect();
    assert!(
        ids.len() >= 2,
        "fixture must offer several particle entries or this test proves nothing"
    );
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "particles with equal evidence sort by ent_seq");
}

// で is where the duplication was actually observed while building the
// ordering tests above: `lex::candidates` independently reconstructed
// ent_seq 1338240/2831062/2871509 (all headword でる) twice each at start 0,
// through two different empty-consumption chain hits landing on the same
// key. Without `lex::dedup_shallowest`, `Segment::entries` for で carries
// each of those three ent_seq values twice -- the same dictionary entry
// showing up twice in what a caller reads as the candidate list.
#[test]
fn no_two_entries_in_a_segment_share_an_entry_id() {
    let dir = scratch("lex-no-duplicate-entries");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "で");
    let ids: Vec<u32> = r.segments[0].entries.iter().map(|e| e.entry_id).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        ids.len(),
        unique.len(),
        "each entry_id must appear at most once in a segment's entries, got {ids:?}"
    );
}

// The dedup above keys on `(len, entry_id)`, not `(start, entry_id)` alone,
// precisely so this still holds: 書いた's onbin fragment 書い (len 2) and
// the full chain 書いた (len 3) both reconstruct headword 書く (the same
// entry_id) from the same start, and both must survive as distinct
// candidates -- collapsing the key down to `(start, entry_id)` would drop
// one of them silently.
#[test]
fn the_same_headword_at_two_different_lengths_is_not_deduplicated_away() {
    let dir = scratch("lex-keeps-both-lengths");
    build_v2(&dir);
    let c = cands(&dir, "書いた");
    let lens: Vec<usize> = c[0]
        .iter()
        .filter(|x| x.headword == "書く")
        .map(|x| x.len)
        .collect();
    // Asserted on the emitted order, not just membership: both 書いた's full
    // chain (len 3) and its 書い fragment (len 2) must survive the dedup,
    // longest span first.
    assert_eq!(
        lens,
        vec![3, 2],
        "both 書いた's full chain (len 3) and its 書い fragment (len 2) must \
         survive the dedup in longest-first order, got lens {lens:?}"
    );
}

// v2's own snapshot over the curated 30-sentence corpus, reviewed on its own
// terms (master plan §5.1: v1 is a disagreement source, not an oracle, so
// this is a fresh snapshot rather than a copy of
// `parse_snapshots__sentences.snap`). The rendering below was written to
// mirror the now-deleted `parse_snapshots.rs`'s `render`, so that while both
// existed a human could diff the two snapshots by eye. It stayed a separate
// copy rather than a shared one because two integration test binaries cannot
// share non-`pub` code, and making this rendering `pub` in the library for
// fifteen lines would leak a display concern into the API. Phase 1b-ii
// deleted the v1 snapshot test, so this is now the only copy.

/// Alternatives printed per span.
const MAX_ALTERNATIVES: usize = 5;

/// Printed wherever a reading or conjugation is `None`.
const NONE_LABEL: &str = "-";

fn sentences() -> impl Iterator<Item = &'static str> {
    SENTENCES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn render_v2(text: &str, result: &jparser::ParseResult) -> String {
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
fn v2_snapshots_every_sentence() {
    let dir = scratch("v2-snapshot");
    build_v2(&dir);
    let mut out = String::new();
    for sentence in sentences() {
        out.push_str(&render_v2(sentence, &parse_v2_over(&dir, sentence)));
        out.push('\n');
    }
    insta::assert_snapshot!("sentences", out);
}

// Ported from the now-deleted parse_snapshots.rs: its subject is the fixture
// corpus itself, not either engine, so it moves rather than dies with that
// file. Guards the corpus itself: addendum §6 requires ~30 sentences and at
// least one する or 来る. Deleting a sentence to make a snapshot green should
// fail here first -- and with parse_snapshots.rs's own する-specific test
// gone too (it tested v1's generated-stem mechanism, which v2 does not have),
// this is now the only test in the crate that exercises する at all.
#[test]
fn corpus_has_thirty_sentences_covering_both_irregulars() {
    let all: Vec<&str> = sentences().collect();
    assert_eq!(all.len(), 30, "got {} sentences", all.len());
    assert!(all.iter().any(|s| s.contains("する")));
    assert!(all.iter().any(|s| s.contains("来る")));
}

// 来る in kanji: the one common verb whose kanji stem's reading changes with
// the form (来た = きた, 来ない = こない, 来る = くる). These are the observables
// `tests/parse_irregular.rs` pinned for v1, ported to v2. A private fixture,
// because `jmdict_subset.xml` is extractor-generated and would drop a
// hand-added entry on regeneration (`fixtures/README.md`).
const KURU_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY vk "Kuru verb - special class">
]>
<JMdict>
<entry>
<ent_seq>1000040</ent_seq>
<k_ele><keb>来る</keb></k_ele>
<r_ele><reb>くる</reb></r_ele>
<sense><pos>&vk;</pos><gloss>to come</gloss></sense>
</entry>
</JMdict>
"#;

fn parse_kuru(name: &str, text: &str) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(KURU_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    parse_v2_over(&dir, text)
}

#[test]
fn kuru_in_kanji_uninflected_keeps_the_dictionary_reading() {
    let r = parse_kuru("kuru-plain", "来る");
    assert_eq!(r.segments.len(), 1);
    let s = &r.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来る"));
    assert!(s.matched);
    assert_eq!(s.reading.as_deref(), Some("くる"));
    assert_eq!(s.entries[0].headword, "来る");
    assert_eq!(s.entries[0].conjugation, None);
}

#[test]
fn kuru_in_kanji_deinflects_and_reads_through_the_kana_class() {
    // Fails today at the first assertion: `vk` maps only to カ変・クル, whose
    // suffixes are whole kana words, so the kanji stem 来 matches nothing and
    // 来 + た are two unmatched spans.
    let r = parse_kuru("kuru-past", "来た");
    assert_eq!(
        r.segments.len(),
        1,
        "one span, not 来 + た: {:?}",
        r.segments
    );
    let s = &r.segments[0];
    assert!(s.matched);
    // Not くた: 来's reading in 連用形 is き, which カ変・クル's same-named
    // form supplies.
    assert_eq!(s.reading.as_deref(), Some("きた"));
    let e = &s.entries[0];
    assert_eq!(e.headword, "来る");
    assert_eq!(e.reading.as_deref(), Some("きた"));
    assert_eq!(e.conjugation.as_deref(), Some("Past"));
    assert_eq!(e.pos, vec!["vk"]);
    assert_eq!(e.senses[0].glosses, vec!["to come"]);
}

#[test]
fn kuru_in_kanji_leaves_an_unmatched_tail_alone() {
    // 。 is in no dictionary here; an unmatched run has no reading and no
    // entries, and the caller can rebuild the input verbatim from the spans.
    let r = parse_kuru("kuru-unmatched", "来た。");
    assert_eq!(r.segments.len(), 2);
    assert!(r.segments[0].matched);
    let tail = &r.segments[1];
    // Char offset 2, not byte offset 6: 来た。 is 3 chars and 9 bytes.
    assert_eq!((tail.start, tail.len, tail.surface.as_str()), (2, 1, "。"));
    assert!(!tail.matched);
    assert_eq!(tail.reading, None);
    assert!(tail.entries.is_empty());
}

#[test]
fn empty_text_parses_into_no_segments() {
    let r = parse_kuru("kuru-empty", "");
    assert!(r.segments.is_empty());
}

/// Pins the exact JSON the webview parses. Field names here are the
/// TypeScript property names in `src/types.ts`; changing either side alone
/// breaks rendering with no compiler error anywhere.
#[test]
fn parse_result_serializes_to_the_documented_wire_shape() {
    let result = jparser::ParseResult {
        segments: vec![jparser::Segment {
            start: 0,
            len: 2,
            surface: "東京".to_string(),
            reading: Some("とうきょう".to_string()),
            matched: true,
            entries: vec![jparser::Entry {
                headword: "東京".to_string(),
                reading: Some("とうきょう".to_string()),
                conjugation: None,
                pos: vec!["n".to_string()],
                senses: vec![],
                flags: WordFlags::PRIMARY,
                entry_id: 1000010,
                steps: Vec::new(),
            }],
        }],
        alternatives: Vec::new(),
    };

    let json = serde_json::to_value(&result).expect("serialize");
    let seg = &json["segments"][0];
    assert_eq!(seg["start"], 0);
    assert_eq!(seg["len"], 2);
    assert_eq!(seg["surface"], "東京");
    assert_eq!(seg["reading"], "とうきょう");
    assert_eq!(seg["matched"], true);

    let entry = &seg["entries"][0];
    assert_eq!(entry["headword"], "東京");
    assert_eq!(entry["conjugation"], serde_json::Value::Null);
    assert_eq!(entry["pos"][0], "n");
    assert_eq!(entry["flags"][0], "primary");
    // A nonzero id: `entry_id`'s type default is 0, so asserting against
    // that value would pass even if the field silently stopped
    // serializing and fell back to its default.
    assert_eq!(entry["entry_id"], 1000010);
    // Present and empty at the default. No frontend consumer exists: the
    // Phase 2 gate that would have justified a reading-cycling UI was not
    // met, so nothing reads this field yet (docs/superpowers/
    // 2026-09-02-engine-v2-phase2-handoff.md §3-4).
    assert_eq!(json["alternatives"].as_array().map(Vec::len), Some(0));
}

/// Pins the JSON `kanji_info` hands the webview. Field names here are the
/// TypeScript property names of `src/types.ts`'s `KanjiInfo`; `literal` is a
/// `char`, which serde_json writes as a one-character string. The four misc
/// numbers cross as a number or `null`, never as a missing key: bincode 1.x
/// cannot read back a struct whose fields were skipped on serialisation, so
/// `skip_serializing_if` is not an option (Phase 3b design §4.1).
#[test]
fn kanji_data_serializes_to_the_documented_wire_shape() {
    let data = jparser::index::KanjiData {
        literal: '東',
        on: vec!["トウ".to_string()],
        kun: vec!["ひがし".to_string()],
        meanings: vec!["east".to_string()],
        grade: Some(2),
        strokes: Some(8),
        freq: Some(37),
        jlpt: None,
    };
    let json = serde_json::to_value(&data).expect("serialize");
    assert_eq!(json["literal"], "東");
    assert_eq!(json["on"][0], "トウ");
    assert_eq!(json["kun"][0], "ひがし");
    assert_eq!(json["meanings"][0], "east");
    assert_eq!(json["grade"], 2);
    assert_eq!(json["strokes"], 8);
    assert_eq!(json["freq"], 37);
    let keys = json.as_object().expect("an object");
    assert!(
        keys.contains_key("jlpt"),
        "an absent field is a present key"
    );
    assert!(
        json["jlpt"].is_null(),
        "an absent field is null, got {:?}",
        json["jlpt"]
    );
    assert_eq!(keys.len(), 8, "exactly the eight documented fields");
}

/// An unmatched run must still be a well-formed segment: the sentence pane
/// renders these muted and unchipped, so it needs them present, not omitted.
#[test]
fn an_unmatched_segment_serializes_with_null_reading_and_no_entries() {
    let result = jparser::ParseResult {
        segments: vec![jparser::Segment {
            start: 0,
            len: 1,
            surface: "〜".to_string(),
            reading: None,
            matched: false,
            entries: vec![],
        }],
        alternatives: Vec::new(),
    };

    let json = serde_json::to_value(&result).expect("serialize");
    assert_eq!(json["segments"][0]["reading"], serde_json::Value::Null);
    assert_eq!(json["segments"][0]["matched"], false);
    assert!(json["segments"][0]["entries"]
        .as_array()
        .expect("array")
        .is_empty());
}

// Phase 2: a private four-entry fixture on which 東京都 has exactly three
// readings, so `k_best`'s order and its dominance filter are both visible
// through `parse`. Private for the reason `KURU_FIXTURE` is.
const TOKYO_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry><ent_seq>1000100</ent_seq><k_ele><keb>東京都</keb></k_ele><r_ele><reb>とうきょうと</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo Metropolis</gloss></sense></entry>
<entry><ent_seq>1000110</ent_seq><k_ele><keb>東京</keb></k_ele><r_ele><reb>とうきょう</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo</gloss></sense></entry>
<entry><ent_seq>1000120</ent_seq><k_ele><keb>京都</keb></k_ele><r_ele><reb>きょうと</reb></r_ele><sense><pos>&n;</pos><gloss>Kyoto</gloss></sense></entry>
<entry><ent_seq>1000130</ent_seq><k_ele><keb>都</keb></k_ele><r_ele><reb>みやこ</reb></r_ele><sense><pos>&n;</pos><gloss>capital</gloss></sense></entry>
</JMdict>
"#;

fn parse_tokyo(name: &str, readings: usize) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(TOKYO_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    jparser::parse(
        &index,
        &table,
        "東京都",
        &jparser::ParseOptions::with_readings(readings),
        None,
    )
    .expect("parse")
}

fn surfaces(segments: &[jparser::Segment]) -> Vec<(&str, bool)> {
    segments
        .iter()
        .map(|s| (s.surface.as_str(), s.matched))
        .collect()
}

#[test]
fn three_readings_come_out_in_cost_order_with_dominated_ones_dropped() {
    let r = parse_tokyo("tokyo-three", 3);
    assert_eq!(surfaces(&r.segments), [("東京都", true)]);
    assert_eq!(r.alternatives.len(), 2);
    // Phase 2b's demotion leaves this order alone, and not by luck: top-1 is
    // the single word 東京都, so every full cover cuts at a superset of its
    // {0,3} and every alternative sinks together, keeping the cost order they
    // arrived in. Demotion reorders between fresh and sunk readings, never
    // within a group; `a_reading_unrelated_to_the_top_one_outranks_a_re_split_of_it`
    // below is the case where the groups differ.
    assert_eq!(surfaces(&r.alternatives[0]), [("東京", true), ("都", true)]);
    // 東 has no entry here, so leaving it unmatched is the only way to read
    // 京都. 東京 + unmatched 都 is absent: 東京 + 都 is cheaper and matches
    // everything it does.
    assert_eq!(
        surfaces(&r.alternatives[1]),
        [("東", false), ("京都", true)]
    );
}

#[test]
fn the_default_options_return_no_alternatives_and_the_same_top_reading() {
    let one = parse_tokyo("tokyo-one", 1);
    let three = parse_tokyo("tokyo-three-again", 3);
    assert!(one.alternatives.is_empty());
    assert_eq!(one.segments, three.segments);
}

#[test]
fn every_alternative_is_a_contiguous_cover() {
    let r = parse_tokyo("tokyo-cover", 3);
    assert!(!r.alternatives.is_empty());
    for alt in &r.alternatives {
        let mut at = 0;
        for s in alt {
            assert_eq!(s.start, at);
            at += s.len;
        }
        assert_eq!(at, 3);
    }
}

// Phase 2b: a private five-entry fixture on which 東京都市 has exactly three
// undominated readings at distinct costs -- 東京+都市 (both COMMON through
// `news1`), 東+京+都市, 東+京都市 -- so `demote`'s reorder is visible through
// `parse` with no tie-break in play. Private for the reason `KURU_FIXTURE` is.
const TOSHI_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry><ent_seq>1000200</ent_seq><k_ele><keb>東京</keb><ke_pri>news1</ke_pri></k_ele><r_ele><reb>とうきょう</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo</gloss></sense></entry>
<entry><ent_seq>1000210</ent_seq><k_ele><keb>都市</keb><ke_pri>news1</ke_pri></k_ele><r_ele><reb>とし</reb></r_ele><sense><pos>&n;</pos><gloss>city</gloss></sense></entry>
<entry><ent_seq>1000220</ent_seq><k_ele><keb>東</keb></k_ele><r_ele><reb>ひがし</reb></r_ele><sense><pos>&n;</pos><gloss>east</gloss></sense></entry>
<entry><ent_seq>1000230</ent_seq><k_ele><keb>京</keb></k_ele><r_ele><reb>きょう</reb></r_ele><sense><pos>&n;</pos><gloss>capital</gloss></sense></entry>
<entry><ent_seq>1000240</ent_seq><k_ele><keb>京都市</keb></k_ele><r_ele><reb>きょうとし</reb></r_ele><sense><pos>&n;</pos><gloss>Kyoto City</gloss></sense></entry>
</JMdict>
"#;

fn parse_toshi(name: &str, readings: usize) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(TOSHI_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    jparser::parse(
        &index,
        &table,
        "東京都市",
        &jparser::ParseOptions::with_readings(readings),
        None,
    )
    .expect("parse")
}

/// Phase 2b design §3: a reading that is not a merge or split of the top one
/// outranks one that is, even when the re-split is cheaper.
#[test]
fn a_reading_unrelated_to_the_top_one_outranks_a_re_split_of_it() {
    let r = parse_toshi("toshi-three", 3);
    assert_eq!(surfaces(&r.segments), [("東京", true), ("都市", true)]);
    assert_eq!(r.alternatives.len(), 2);
    // By cost, 東+京+都市 (0) precedes 東+京都市 (4). The first cuts at
    // {0,1,2,4}, a superset of top-1's {0,2,4} -- the same reading
    // re-split -- and sinks; the second cuts at {0,1,4}, comparable with
    // neither, and is the first alternative a reader is offered.
    assert_eq!(
        surfaces(&r.alternatives[0]),
        [("東", true), ("京都市", true)]
    );
    assert_eq!(
        surfaces(&r.alternatives[1]),
        [("東", true), ("京", true), ("都市", true)]
    );
}

/// Global constraint: asking for more readings never moves the first one.
/// The eight cells prove it on the gold corpus; this proves it on every
/// sentence the crate's own corpus holds, without a release build.
#[test]
fn asking_for_readings_never_moves_the_top_reading_on_the_corpus() {
    let dir = scratch("readings-invariant");
    build_v2(&dir);
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    for text in sentences() {
        let one = jparser::parse(
            &index,
            &table,
            text,
            &jparser::ParseOptions::default(),
            None,
        )
        .expect("parse");
        let three = jparser::parse(
            &index,
            &table,
            text,
            &jparser::ParseOptions::with_readings(3),
            None,
        )
        .expect("parse");
        assert_eq!(one.segments, three.segments, "{text}");
    }
}

/// The wire shape of `alternatives`: an array of segment arrays, the same
/// segment shape `segments` uses, so `src/types.ts` needs one line.
#[test]
fn alternatives_serialize_as_an_array_of_segment_arrays() {
    let seg = |start: usize, len: usize, surface: &str| jparser::Segment {
        start,
        len,
        surface: surface.to_string(),
        reading: None,
        matched: true,
        entries: vec![],
    };
    let result = jparser::ParseResult {
        segments: vec![seg(0, 3, "東京都")],
        alternatives: vec![vec![seg(0, 2, "東京"), seg(2, 1, "都")]],
    };
    let json = serde_json::to_value(&result).expect("serialize");
    assert_eq!(json["alternatives"].as_array().map(Vec::len), Some(1));
    assert_eq!(json["alternatives"][0][1]["surface"], "都");
    assert_eq!(json["alternatives"][0][1]["start"], 2);
}

/// Phase 2c: candidate generation used to be superlinear in input length —
/// it walked the index from each start for every span end, so a
/// 10,000-character paste, which is the app's own clipboard ceiling, did not
/// finish in ten minutes. This asserts nothing about the clock: a wall-time
/// threshold is flaky on shared hardware. It is a canary. If the blowup
/// returns, this test stops finishing and the suite hangs, which is the
/// loud signal whose absence let the defect survive four phases.
#[test]
fn a_long_input_parses_in_reasonable_time() {
    let dir = scratch("long-input");
    build_v2(&dir);
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();

    // Roughly 4,000 characters of the crate's own corpus sentences, which is
    // far past where the old shape became unusable and well short of the
    // clipboard ceiling.
    let mut text = String::new();
    while text.chars().count() < 4000 {
        for sentence in sentences() {
            text.push_str(sentence);
            if text.chars().count() >= 4000 {
                break;
            }
        }
    }

    let result = jparser::parse(
        &index,
        &table,
        &text,
        &jparser::ParseOptions::default(),
        None,
    )
    .expect("parse");

    // The cover is contiguous and complete, as for any input.
    let mut at = 0;
    for segment in &result.segments {
        assert_eq!(segment.start, at);
        at += segment.len;
    }
    assert_eq!(at, text.chars().count());
}

/// Phase 2c design §3.2: the old `(start + 1..=len)` loop bound made a
/// zero-length span unreachable. Indexed by stem end, the identity chain hit
/// (`consumed: 0`) at `stem_end == start` implies `end == start`, so the
/// restructure has to exclude it explicitly. A candidate of length zero
/// would break every consumer of a contiguous cover.
///
/// The build is hoisted out of the per-sentence loop (unlike `cands`, which
/// rebuilds per call) -- the same shape
/// `asking_for_readings_never_moves_the_top_reading_on_the_corpus` above
/// already uses -- since this iterates every corpus sentence.
#[test]
fn no_candidate_has_zero_length() {
    let dir = scratch("zero-length");
    build_v2(&dir);
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    for text in sentences() {
        let chars: Vec<char> = text.chars().collect();
        let out = jparser::engine::lex::candidates(&index, &table, &chars).expect("candidates");
        for bucket in out {
            for candidate in bucket {
                assert!(
                    candidate.len >= 1,
                    "zero-length candidate at {} in {text}",
                    candidate.start
                );
            }
        }
    }
}
