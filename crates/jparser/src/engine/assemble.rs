// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! Candidates to entries: turning the winning path's [`Step`]s into the
//! public [`Segment`]/[`Entry`] shape the application consumes.
//!
//! `Entry::conjugation` is `label::fmt` over the chain `chain::steps_for`
//! hands back (Phase 1b-i). What this module owns beyond that is reading
//! reconstruction: `EntryData::readings` holds a reading only for a kanji
//! entry, and only in its dictionary form, so a conjugated match has to
//! strip that dictionary form's own suffix and rebuild it from the matched
//! surface's tail. A reading that does not strip cleanly is never guessed
//! at -- `None` is what goes out, because a wrong reading renders as
//! furigana the reader has no reason to doubt.

use crate::engine::chain;
use crate::engine::flags::WordFlags;
use crate::engine::kana;
use crate::engine::label;
use crate::engine::lattice::Step;
use crate::engine::lex::Candidate;
use crate::engine::table::{Class, CLASSES, KURU_KANJI_CLASS};
use crate::index::load::Index;
use crate::index::{EntryData, IndexError, SenseData};
use crate::{Entry, Segment};

/// Turn one step of the winning path into the [`Segment`] the application
/// consumes: an unmatched run as-is, a match resolved through `index` into
/// one [`Entry`] per same-span sibling candidate.
///
/// `by_start` is the whole per-start candidate table `lex::candidates`
/// produced -- not just `step`'s own span -- because siblings sharing
/// `step`'s span are found by indexing into it directly. Guarded with
/// `by_start.get(start)` rather than trusted to cover `step`'s start: a
/// caller may reasonably pass a table shorter than `chars`.
///
/// `Segment::entries`' order comes from [`order`], not from the DP's own
/// pick or from `by_start`'s incidental order: `entries[0]` is whichever
/// same-span candidate `order` ranks highest, which need not be `cand`
/// itself. `cand` still decides the span the DP wrote into `step` -- its
/// `start`/`len` set `Segment::start`/`len`/`surface`/`matched` -- only
/// which entry a caller sees *first* for that span is `order`'s call.
pub fn to_segment(
    index: &Index,
    chars: &[char],
    step: &Step,
    by_start: &[Vec<Candidate>],
) -> Result<Segment, IndexError> {
    let (start, len) = match step {
        Step::Matched(cand) => (cand.start, cand.len),
        Step::Unmatched { start, len } => (*start, *len),
    };
    let surface: String = chars[start..start + len].iter().collect();

    let Step::Matched(cand) = step else {
        return Ok(Segment {
            start,
            len,
            surface,
            reading: None,
            matched: false,
            entries: Vec::new(),
        });
    };

    // Every same-span candidate, `cand` included, gathered and ordered
    // *before* any `Entry` is built -- `entries[0].reading` becomes
    // `Segment::reading` below, so the order has to be settled first for
    // that to mean what it says.
    let siblings = by_start.get(start).map(Vec::as_slice).unwrap_or(&[]);
    let mut group = Vec::with_capacity(1);
    group.push(cand.clone());
    for sibling in siblings {
        if sibling.len == cand.len && sibling != cand {
            group.push(sibling.clone());
        }
    }
    // `1969|年`: the one piece of context the ordering uses. Computed here
    // because this is where the span's neighbours are in hand.
    let prev_is_digit = start > 0 && kana::is_digit(chars[start - 1]);

    // Entries are resolved *before* ordering, because the last clause of the
    // key is the entry's own sense count and every candidate in the group was
    // going to be resolved anyway -- so this costs nothing it did not already
    // pay.
    let mut built: Vec<(Candidate, Entry)> = Vec::with_capacity(group.len());
    for c in group {
        let entry = build_entry(index, chars, &c)?;
        built.push((c, entry));
    }
    order(&mut built, prev_is_digit);
    let entries: Vec<Entry> = built.into_iter().map(|(_, e)| e).collect();

    let reading = entries[0].reading.clone();
    Ok(Segment {
        start,
        len,
        surface,
        reading,
        matched: true,
        entries,
    })
}

/// Whether JMdict tags the entry `uk`, "word usually written using kana
/// alone".
///
/// Read off the senses rather than [`WordFlags::UK`] on purpose, and the two
/// are deliberately not the same question. `order` already has the assembled
/// [`Entry`] and `SenseData::misc` is already stored, so this costs nothing,
/// and this half wants the plain JMdict tag: it runs after the DP and moves no
/// boundary, so `様に` and `其れに` being `uk` is harmless here.
///
/// The flag bit is narrower — `uk` **and** no grammatical POS — because
/// `cost::PRONOUNCE_CROSS` reads it and a boundary moves. The earlier refusal
/// recorded here, that a `uk` bit takes the 26-row `spurious_merges.txt` gate
/// to 25 by letting `ことになって` swallow its `に`, was of the plain tag; see
/// `flags::GRAMMATICAL_POS` and `cost::UK_MIN_LEN` for what the conjunction
/// had to add before the gates held.
fn usually_kana(entry: &Entry) -> bool {
    entry
        .senses
        .iter()
        .any(|s| s.misc.iter().any(|m| m == "uk"))
}

/// Decide which same-span candidate a caller sees first.
///
/// In sequence: an exact surface match before one only reached by folding
/// kana together; a `COMMON`-priority headword before a rarer one; a
/// `COUNTER` entry before the rest **when the character before the span is a
/// digit**; a `PARTICLE` entry before the rest; a `PRIMARY` headword (the
/// word itself) before a `PRONOUNCE` one (a kanji entry's own reading,
/// offered only as a fallback) **unless JMdict tags the entry `uk`**, where
/// the kana spelling is the normal one and demoting it is simply wrong; a
/// shallower deinflection chain before a
/// deeper one, since an uninflected reading of the same surface is the
/// likelier one; and, last, ascending `entry_id` -- JMdict's own `ent_seq` --
/// as a tiebreak. Every earlier clause is a fact JMdict, the match itself or
/// the surrounding text supplies, never a frequency table this project does
/// not have; the last clause carries no quality judgement of its own, it
/// exists only to make the order total.
///
/// **The `COUNTER` clause is gated on the digit and must stay gated.**
/// Measured over the 150-sentence gold corpus with
/// `examples/order_sweep.rs`, hints on: gated it is **+12 entry top-1 hits
/// and costs nothing**, every one of them `年` after a year like `1969`.
/// *Ungated* -- "a counter outranks the rest, always" -- it is **-45**, because
/// `が` has a `suf` sense and `COUNTER_POS` includes `suf`, so the particle
/// loses its own span 51 times. A counter is not more likely than a noun in
/// general; it is more likely *after a number*, and that is a fact about the
/// text rather than a prior about the language. That distinction is the bar
/// the Phase 1a task brief set for this key -- "a fact JMdict or the match
/// itself supplies, never a frequency table this project does not have"
/// (`plans/2026-09-01-engine-v2-phase1a.md`). It was first attributed here to
/// master plan §5.2, which defines the metrics and sets no such bar.
///
/// The `PARTICLE` clause needs no gate: measured on its own it is **+3 and
/// costs nothing** in both hint states, since it only ever separates
/// candidates that were about to be decided by ascending `entry_id`.
///
/// Together, entry top-1 goes **0.9125 -> 0.9236 hints on and 0.9093 ->
/// 0.9215 hints off, with no span losing a hit in either state** -- which is
/// what takes master plan §2's criterion 2 above the v1 baseline it had been
/// below (0.9155 / 0.9159). Two near misses that were tried and rejected are
/// recorded in `order_sweep.rs`: folding `COMMON_LINE` into the `COMMON`
/// clause is an exact wash (+17 / -17), and putting counters and particles in
/// one ungated tier scores +22 but buys it with 13 real losses.
///
/// `entry_id` sorting last is exactly what makes the key total: two
/// distinct entries never share one, so no two candidates from different
/// entries can ever tie all the way down. A literal duplicate candidate
/// (same entry, same chain) ties on every field including `entry_id`, but
/// is then indistinguishable from itself in the output either way -- so
/// `sort_unstable_by_key` never has a real tie left for stability to matter
/// to, and is safe here. That safety leans on a precondition this function
/// does not itself enforce: `lex::dedup_shallowest` has already collapsed
/// every candidate sharing a `(len, entry_id)` down to one per shallowest
/// chain before `order` ever sees them. Without that upstream collapse, two
/// candidates with the same entry but *different* chains -- and therefore
/// different `Entry::conjugation` labels -- could tie on every key above and
/// still be visibly different in the output.
pub fn order(built: &mut [(Candidate, Entry)], prev_is_digit: bool) {
    built.sort_unstable_by_key(|(c, e)| {
        (
            !c.exact,
            !c.flags.contains(WordFlags::COMMON),
            !(prev_is_digit && c.flags.contains(WordFlags::COUNTER)),
            !c.flags.contains(WordFlags::PARTICLE),
            c.flags.contains(WordFlags::PRONOUNCE) && !usually_kana(e),
            c.steps.len(),
            // More senses first. JMdict's own editorial content, not a corpus
            // count -- and placed last, where `entry_id` was about to decide
            // arbitrarily, so it never overrules a clause that means anything.
            std::cmp::Reverse(e.senses.len()),
            c.entry_id,
        )
    });
}

/// One [`Entry`] for `cand`: its dictionary data resolved through `index`,
/// its reading reconstructed per this module's contract, and its
/// conjugation label rendered from the chain by `label::fmt`.
fn build_entry(index: &Index, chars: &[char], cand: &Candidate) -> Result<Entry, IndexError> {
    // A candidate's `entry_id` always names an entry in the same index it
    // was matched from -- `lex::candidates` reads both from `index` in the
    // same call. A well-formed index never disagrees with itself.
    let entry_data = index.entry(cand.entry_id)?.expect(
        "invariant: lex::candidates only ever emits a Candidate whose entry_id it just \
         read out of this same `index`, so `index.entry(cand.entry_id)` resolving to None \
         here means the index disagreed with itself between the two calls",
    );

    let reading = reading_for(&entry_data, cand, chars);
    let pos = union_pos(&entry_data.senses);
    let steps = chain::steps_for(cand);

    Ok(Entry {
        headword: cand.headword.clone(),
        reading,
        conjugation: label::fmt(&steps),
        pos,
        senses: entry_data.senses,
        flags: cand.flags,
        entry_id: cand.entry_id,
        steps,
    })
}

/// Every sense's POS code, in first-seen order, deduplicated globally --
/// not just between neighbours, since the same code can legitimately recur
/// in a later sense.
fn union_pos(senses: &[SenseData]) -> Vec<String> {
    let mut pos = Vec::new();
    for sense in senses {
        for code in &sense.pos {
            if !pos.contains(code) {
                pos.push(code.clone());
            }
        }
    }
    pos
}

/// The kana spelling's class, and the reason [`KURU_KANJI_CLASS`] needs
/// special handling here: 来's stem reading alternates with the form (来た =
/// きた, 来ない = こない, 来る = くる), so stripping the kanji class's
/// remove-suffix from the dictionary reading -- right for every other class
/// -- would produce くた. This class conjugates the same ten form names over
/// an empty stem, so each form's suffix *is* the kanji stem's reading in that
/// form.
const KURU_KANA_CLASS: &str = "カ変・クル";

/// The kana spelling's class itself, for its `remove` as well as its forms.
fn kuru_kana_class() -> Option<&'static Class> {
    CLASSES.iter().find(|c| c.name == KURU_KANA_CLASS)
}

/// `カ変・クル`'s suffix for the form named `form`: the reading of 来's stem
/// in that form. `None` only if the two classes' form sets have drifted
/// apart, which `kuru_classes_share_every_form_name` guards against.
fn kuru_stem_reading(form: &str) -> Option<&'static str> {
    kuru_kana_class()?
        .forms
        .iter()
        .find(|f| f.name == form)
        .map(|f| f.suffix)
}

/// Reconstruct `cand`'s reading from `entry`'s stored data, or `None` when
/// no reading can be attached without inventing one.
///
/// - A [`WordFlags::PRONOUNCE`] match already *is* a reading (it is a kana
///   `<reb>` form of a kanji entry) -- it gets none of its own.
/// - `entry.readings` is empty for a kana-only entry, where the surface
///   already is the reading -- also none.
/// - An uninflected match (`cand.steps` empty) is a plain kanji headword:
///   `entry.readings[0]` applies as stored.
/// - A conjugated match's stored reading is the *dictionary* form's. The
///   innermost step (`cand.steps[0]`, per `chain`'s contract) names the
///   class the entry itself conjugates in; stripping that class's
///   `remove` suffix off the dictionary reading recovers the reading's own
///   stem, and the matched surface's tail from that same point onward
///   (`chars[step.range.start..cand.start + cand.len]`) is already kana in
///   the ordinary case, so appending it verbatim reconstructs the
///   conjugated reading (書いた -> stem "か" + tail "いた" -> かいた).
///   Kanji 来る is the exception: under `カ変・来ル` the strip *succeeds* (くる
///   ends in る) and would give くた, so that class takes its stem reading
///   from `カ変・クル`'s same-named form instead -- 連用形 き, 未然形 こ,
///   仮定形 くれ -- followed by the surface after the step's own range. Written
///   from the observable alternation; nothing here derives from v1.
fn reading_for(entry: &EntryData, cand: &Candidate, chars: &[char]) -> Option<String> {
    if cand.flags.contains(WordFlags::PRONOUNCE) {
        return None;
    }
    let dict_reading = entry.readings.first()?;

    let Some(first_step) = cand.steps.first() else {
        return Some(dict_reading.clone());
    };

    let class = &CLASSES[first_step.class as usize];
    let end = cand.start + cand.len;
    if class.name == KURU_KANJI_CLASS {
        let form = class.forms[first_step.form as usize].name;
        let stem = kuru_stem_reading(form)?;
        // Everything before the 来, which the kana class's suffix does not
        // carry: 出て来た is でてきた, not きた, and the reading is what the
        // furigana renders. Bare 来る's own reading is the suffix and nothing
        // else, so its prefix is empty and its output is unchanged.
        let prefix = kuru_kana_class()
            .and_then(|c| dict_reading.strip_suffix(c.remove))
            .unwrap_or("");
        // The step's own range is the kanji form's suffix (empty for 連用形,
        // れ for 仮定形), already accounted for by the kana form's suffix; the
        // auxiliaries after it are kana as written.
        let tail: String = chars[first_step.range.end..end].iter().collect();
        return Some(format!("{prefix}{stem}{tail}"));
    }
    let stem = dict_reading.strip_suffix(class.remove)?;
    let tail: String = chars[first_step.range.start..end].iter().collect();
    Some(format!("{stem}{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::table::classes_for_pos;
    use crate::engine::ConjStep;

    fn entry(readings: &[&str]) -> EntryData {
        EntryData {
            id: 1,
            readings: readings.iter().map(|s| s.to_string()).collect(),
            senses: Vec::new(),
        }
    }

    fn candidate(start: usize, len: usize, flags: WordFlags, steps: Vec<ConjStep>) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: 1,
            flags,
            steps,
            exact: true,
            all_katakana: false,
            inside_katakana_run: false,
            function_stack: false,
            grammatical_stack: false,
            digit_counter: false,
            after_digit_noncounter: false,
            headword: String::new(),
        }
    }

    #[test]
    fn a_pronounce_match_gets_no_reading_of_its_own() {
        let e = entry(&["かく"]);
        let c = candidate(0, 2, WordFlags::PRONOUNCE, Vec::new());
        assert_eq!(reading_for(&e, &c, &['か', 'く']), None);
    }

    #[test]
    fn a_kana_only_entry_has_no_separate_reading() {
        let e = entry(&[]); // no kanji forms -> no stored readings
        let c = candidate(0, 2, WordFlags::PRIMARY, Vec::new());
        assert_eq!(reading_for(&e, &c, &['は', 'く']), None);
    }

    #[test]
    fn an_uninflected_primary_match_takes_the_first_stored_reading() {
        let e = entry(&["とうきょう", "とうけい"]);
        let c = candidate(0, 2, WordFlags::PRIMARY, Vec::new());
        assert_eq!(
            reading_for(&e, &c, &['東', '京']),
            Some("とうきょう".to_string())
        );
    }

    #[test]
    fn a_conjugated_match_strips_the_dictionary_form_and_appends_the_tail() {
        // 書いた -> 書く (かく). The godan-ka class's `remove` is "く"; かく
        // strips to か, then いた (the matched surface's tail from the
        // stem's end onward) is appended: かいた.
        let e = entry(&["かく"]);
        let class = classes_for_pos("v5k").next().expect("v5k maps to a class");
        let chars: Vec<char> = "書いた".chars().collect();
        let steps = vec![ConjStep {
            class,
            form: 0,
            range: 1..3,
            aux: None,
        }];
        let c = candidate(0, 3, WordFlags::PRIMARY, steps);
        assert_eq!(reading_for(&e, &c, &chars), Some("かいた".to_string()));
    }

    fn sense(pos: &[&str]) -> SenseData {
        SenseData {
            pos: pos.iter().map(|s| s.to_string()).collect(),
            glosses: Vec::new(),
            xrefs: Vec::new(),
            misc: Vec::new(),
            info: Vec::new(),
        }
    }

    #[test]
    fn union_pos_removes_a_duplicate_even_when_it_is_not_adjacent() {
        // sense 1 and sense 3 both carry "n", with "vs" in between --
        // `Vec::dedup` alone would miss this, since it only collapses
        // adjacent runs.
        let senses = [sense(&["n"]), sense(&["vs"]), sense(&["n"])];
        assert_eq!(union_pos(&senses), vec!["n".to_string(), "vs".to_string()]);
    }

    #[test]
    fn a_reading_that_does_not_end_in_the_classs_suffix_is_never_invented() {
        // A synthetic mismatch: readings[0] does not end in the ichidan
        // class's "る" at all, so the strip fails outright. `None` is the
        // only correct answer -- a guess here would render as wrong
        // furigana with no visible sign it was fabricated.
        let e = entry(&["かく"]);
        let class = classes_for_pos("v1").next().expect("v1 maps to a class");
        let chars: Vec<char> = "かく".chars().collect();
        let steps = vec![ConjStep {
            class,
            form: 0,
            range: 0..2,
            aux: None,
        }];
        let c = candidate(0, 2, WordFlags::PRIMARY, steps);
        assert_eq!(reading_for(&e, &c, &chars), None);
    }

    fn kuru_step(form: &str, range: std::ops::Range<usize>) -> ConjStep {
        let class = CLASSES
            .iter()
            .position(|c| c.name == KURU_KANJI_CLASS)
            .expect("カ変・来ル") as u16;
        let form = CLASSES[class as usize]
            .forms
            .iter()
            .position(|f| f.name == form)
            .expect("form") as u16;
        ConjStep {
            class,
            form,
            range,
            aux: None,
        }
    }

    #[test]
    fn kuru_in_kanji_reads_its_stem_from_the_kana_class() {
        let e = entry(&["くる"]);
        // 来た: 来 is 連用形 (empty suffix, range 1..1); た follows.
        let chars: Vec<char> = "来た".chars().collect();
        let c = candidate(0, 2, WordFlags::PRIMARY, vec![kuru_step("連用形", 1..1)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("きた".to_string()));
        // 来ない: 未然形, also empty.
        let chars: Vec<char> = "来ない".chars().collect();
        let c = candidate(0, 3, WordFlags::PRIMARY, vec![kuru_step("未然形", 1..1)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("こない".to_string()));
        // 来れ: 仮定形's suffix れ is the step's own range, so nothing follows.
        let chars: Vec<char> = "来れ".chars().collect();
        let c = candidate(0, 2, WordFlags::PRIMARY, vec![kuru_step("仮定形", 1..2)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("くれ".to_string()));
    }

    #[test]
    fn kuru_classes_share_every_form_name() {
        // The reading rule looks each カ変・来ル form up by name in カ変・クル.
        // A regeneration that renamed a form in one class only would make the
        // lookup fail and the reading silently vanish; this catches it.
        let kanji = CLASSES
            .iter()
            .find(|c| c.name == KURU_KANJI_CLASS)
            .expect("kanji class");
        for form in kanji.forms {
            assert!(
                kuru_stem_reading(form.name).is_some(),
                "{} has no カ変・クル twin",
                form.name
            );
        }
    }

    // `order`'s seven clauses, isolated one at a time against hand-built
    // candidates rather than against whatever a fixture's FST happens to
    // hand back. This matters concretely for the lowest-priority clause:
    // `engine_v2.rs`'s `ties_break_on_entry_id_ascending` integration test
    // turned out to pass even with `order` disabled entirely, because that
    // fixture's per-key candidate order already comes out ascending by
    // `entry_id` on its own (JMdict's source XML is itself `ent_seq`-order,
    // and the index preserves that). These tests exist so each clause is
    // proven by construction, not by coincidence: every test below sets up
    // a "trap" where a *lower*-priority signal (often `entry_id` itself)
    // favors the wrong candidate, so the test only passes if the clause
    // under test is actually what `order` is applying.
    /// One sense, so every trap below still isolates the clause it names --
    /// the sense-count clause cannot break a tie it is not part of.
    fn order_candidate(
        entry_id: u32,
        exact: bool,
        flags: WordFlags,
        step_count: usize,
    ) -> (Candidate, Entry) {
        with_senses(entry_id, exact, flags, step_count, 1)
    }

    fn with_senses(
        entry_id: u32,
        exact: bool,
        flags: WordFlags,
        step_count: usize,
        senses: usize,
    ) -> (Candidate, Entry) {
        let entry = Entry {
            headword: String::new(),
            reading: None,
            conjugation: None,
            pos: Vec::new(),
            senses: vec![
                crate::Sense {
                    pos: Vec::new(),
                    glosses: Vec::new(),
                    xrefs: Vec::new(),
                    misc: Vec::new(),
                    info: Vec::new(),
                };
                senses
            ],
            flags,
            entry_id,
            steps: Vec::new(),
        };
        (candidate_only(entry_id, exact, flags, step_count), entry)
    }

    fn candidate_only(
        entry_id: u32,
        exact: bool,
        flags: WordFlags,
        step_count: usize,
    ) -> Candidate {
        Candidate {
            start: 0,
            len: 1,
            entry_id,
            flags,
            steps: vec![
                ConjStep {
                    class: 0,
                    form: 0,
                    range: 0..0,
                    aux: None,
                };
                step_count
            ],
            exact,
            all_katakana: false,
            inside_katakana_run: false,
            function_stack: false,
            grammatical_stack: false,
            digit_counter: false,
            after_digit_noncounter: false,
            headword: String::new(),
        }
    }

    #[test]
    fn order_puts_an_exact_match_before_an_inexact_one_regardless_of_id() {
        // The inexact candidate carries the *smaller* entry_id -- if `order`
        // fell through to the entry_id tiebreak instead of applying the
        // exact/inexact clause, this would sort backwards.
        let mut cands = vec![
            order_candidate(1, false, WordFlags::PRIMARY, 0),
            order_candidate(2, true, WordFlags::PRIMARY, 0),
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the exact match sorts first despite the higher entry_id"
        );
    }

    /// The counter clause and its gate, as one test, because the gate is the
    /// whole finding: ungated this same clause is **-45** entry top-1 hits on
    /// the gold corpus, `が` alone losing its own span 51 times to a `suf`
    /// sense. A counter is likelier *after a number*, not in general.
    #[test]
    fn order_puts_a_counter_first_only_after_a_digit() {
        let mut counter = WordFlags::PRIMARY;
        counter.insert(WordFlags::COUNTER);
        let build = || {
            vec![
                order_candidate(1, true, WordFlags::PRIMARY, 0), // noun, smaller id
                order_candidate(2, true, counter, 0),            // counter, larger id
            ]
        };

        // `1969|年`: the digit is there, so the counter wins despite its id.
        let mut after_digit = build();
        order(&mut after_digit, true);
        assert_eq!(
            after_digit[0].0.entry_id, 2,
            "after a digit the counter sorts first"
        );

        // `去|年`: no digit, so the clause must not fire and the id decides.
        let mut elsewhere = build();
        order(&mut elsewhere, false);
        assert_eq!(
            elsewhere[0].0.entry_id, 1,
            "without the digit the counter has no claim and entry_id decides"
        );
    }

    /// The particle clause, which needs no gate: measured on its own it is
    /// +3 entry top-1 hits and costs nothing, because it only ever separates
    /// candidates that `entry_id` was about to decide arbitrarily.
    #[test]
    fn order_puts_a_particle_before_a_plain_entry_regardless_of_id() {
        let mut particle = WordFlags::PRIMARY;
        particle.insert(WordFlags::PARTICLE);
        let mut cands = vec![
            order_candidate(1, true, WordFlags::PRIMARY, 0), // not a particle, smaller id
            order_candidate(2, true, particle, 0),           // particle, larger id
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the particle sorts first despite the higher entry_id"
        );
    }

    /// `COMMON` outranks both new clauses, which is what keeps them
    /// tiebreakers rather than a second priority signal.
    #[test]
    fn common_still_outranks_the_counter_and_particle_clauses() {
        let mut common = WordFlags::PRIMARY;
        common.insert(WordFlags::COMMON);
        let mut counter = WordFlags::PRIMARY;
        counter.insert(WordFlags::COUNTER);
        counter.insert(WordFlags::PARTICLE);
        let mut cands = vec![
            order_candidate(9, true, counter, 0), // counter *and* particle, not COMMON
            order_candidate(1, true, common, 0),  // COMMON only
        ];
        order(&mut cands, true);
        assert_eq!(
            cands[0].0.entry_id, 1,
            "a COMMON headword outranks a counter even after a digit"
        );
    }

    /// The sense-count clause, and why it sits last.
    ///
    /// `いった` is five candidates that tie on every clause above this one --
    /// all exact, all COMMON, all PRONOUNCE, all two-step chains -- so
    /// `entry_id` ascending was picking 炒る "to roast" (1391500) over 行く
    /// "to go" (1578850) purely because its number is smaller. JMdict records
    /// 12 senses for 行く and 1 for 炒る, which is the dictionary's own
    /// editorial content rather than a corpus count this project does not
    /// have (master plan §5.2's bar). Measured over the gold corpus with
    /// `examples/order_sweep.rs`, hints on: **+41 entry top-1 hits**, and the
    /// two earlier placements are both worse -- ahead of the chain depth it is
    /// +30 with 31 losses, ahead of PRONOUNCE +27 with 43.
    #[test]
    fn order_prefers_more_senses_only_where_entry_id_would_have_decided() {
        // The trap: the *fewer*-sense entry carries the smaller id, so this
        // passes only if the clause fires.
        let mut cands = vec![
            with_senses(1391500, true, WordFlags::PRONOUNCE, 2, 1),
            with_senses(1578850, true, WordFlags::PRONOUNCE, 2, 12),
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 1578850,
            "the 12-sense entry sorts first despite the higher entry_id"
        );

        // And it must not outrank a clause that means something: a COMMON
        // single-sense entry still beats a rarer twelve-sense one.
        let mut common = WordFlags::PRONOUNCE;
        common.insert(WordFlags::COMMON);
        let mut mixed = vec![
            with_senses(2, true, WordFlags::PRONOUNCE, 0, 12),
            with_senses(1, true, common, 0, 1),
        ];
        order(&mut mixed, false);
        assert_eq!(
            mixed[0].0.entry_id, 1,
            "COMMON still outranks the sense count"
        );
    }

    /// The `uk` exemption on the `PRONOUNCE` clause.
    ///
    /// `PRONOUNCE` demotes a kanji entry's own reading because a kana
    /// spelling of a kanji word is a fallback. For an entry JMdict tags `uk`
    /// — "usually written using kana alone" — that premise is false: the kana
    /// *is* how the word is written, and 9,580 of JMdict's 218,449 entries
    /// say so. `待っていた` is the case it was found on: `いる` is `uk`, its
    /// `いた` lost the clause to `い` "the Boar (twelfth sign of the Chinese
    /// zodiac)", and the corpus probe counts `いた` in 218 sentences.
    ///
    /// Measured over the gold corpus with `score`: entry top-1 **0.9400 →
    /// 0.9500 hints on and 0.9358 → 0.9507 hints off, +13 and +19 hits on an
    /// unchanged denominator, with no span moving in either state** — `order`
    /// runs after the DP, so it cannot. Every other cell is identical to the
    /// digit.
    ///
    /// **It is deliberately not a `WordFlags` bit**, which would carry `uk`
    /// into `cost::PRONOUNCE_CROSS`. That was measured: entry top-1 rises
    /// further (0.9497 / 0.9507) and `lexical` goes 82 → 87, but
    /// `ことになって` swallows its `に` and the 26-row `spurious_merges.txt`
    /// gate goes to 25, because `なる` is `uk` as well. Success criterion 3
    /// refuses it.
    #[test]
    fn order_does_not_demote_a_kana_spelling_the_dictionary_calls_normal() {
        let uk = |id: u32, senses: usize| {
            let (c, mut e) = with_senses(id, true, WordFlags::PRONOUNCE, 0, senses);
            e.senses[0].misc.push("uk".to_string());
            (c, e)
        };
        // The exemption makes the two *tie* on this clause, so the test has
        // to be visible one clause further down: the `uk` entry carries more
        // senses, which is only ever consulted once PRONOUNCE has tied.
        let mut cands = vec![with_senses(1, true, WordFlags::PRIMARY, 0, 1), uk(2, 2)];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "a `uk` reading is not a fallback, so the clause ties and the sense count decides"
        );

        // The same shape without `uk`: the clause fires and the PRIMARY
        // entry wins despite having fewer senses.
        let mut plain = vec![
            with_senses(1, true, WordFlags::PRIMARY, 0, 1),
            with_senses(2, true, WordFlags::PRONOUNCE, 0, 2),
        ];
        order(&mut plain, false);
        assert_eq!(
            plain[0].0.entry_id, 1,
            "without `uk` the clause still demotes the reading"
        );
    }

    #[test]
    fn order_puts_common_before_not_regardless_of_id() {
        let mut common = WordFlags::PRIMARY;
        common.insert(WordFlags::COMMON);
        let mut cands = vec![
            order_candidate(1, true, WordFlags::PRIMARY, 0), // not COMMON, smaller id
            order_candidate(2, true, common, 0),             // COMMON, larger id
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the COMMON match sorts first despite the higher entry_id"
        );
    }

    #[test]
    fn order_puts_primary_before_pronounce_regardless_of_id() {
        let mut cands = vec![
            order_candidate(1, true, WordFlags::PRONOUNCE, 0), // smaller id
            order_candidate(2, true, WordFlags::PRIMARY, 0),   // larger id
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the PRIMARY match sorts first despite the higher entry_id"
        );
    }

    #[test]
    fn order_puts_a_shorter_chain_before_a_longer_one_regardless_of_id() {
        let mut cands = vec![
            order_candidate(1, true, WordFlags::PRIMARY, 2), // two-step chain, smaller id
            order_candidate(2, true, WordFlags::PRIMARY, 0), // uninflected, larger id
        ];
        order(&mut cands, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the shallower chain sorts first despite the higher entry_id"
        );
    }

    #[test]
    fn order_breaks_ties_on_entry_id_ascending() {
        // Every field but entry_id is identical, and the input is handed in
        // descending id order -- only the tiebreak clause can produce
        // ascending output here.
        let mut cands = vec![
            order_candidate(30, true, WordFlags::PRIMARY, 0),
            order_candidate(10, true, WordFlags::PRIMARY, 0),
            order_candidate(20, true, WordFlags::PRIMARY, 0),
        ];
        order(&mut cands, false);
        let ids: Vec<u32> = cands.iter().map(|(c, _)| c.entry_id).collect();
        assert_eq!(ids, vec![10, 20, 30]);
    }
}
