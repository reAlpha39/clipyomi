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
use crate::engine::lex::{potential_allowed, Candidate};
use crate::engine::table::{
    classes_for_pos, is_godan_potential, potential_stem_on, Class, CLASSES, KURU_KANJI_CLASS,
};
use crate::engine::ConjStep;
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
    // `1969|年` and `楽し|そう`: the two pieces of context the ordering uses,
    // one character either side. Computed here because this is where the
    // span's neighbours are in hand.
    let prev_is_digit = start > 0 && kana::is_digit(chars[start - 1]);
    // A stem that ends in kanji is left alone: every repair `is_bare_stem`
    // exists for ends in kana (した, 愛し, 欲し), and hints-off the kanji ones
    // are compounds already cut (内太|腿, 重|低音, 鈍|痛), 220 spans where
    // 太る, 重る and 鈍る are no better a gloss than the adjective.
    let stem_attached =
        attaches_to_a_stem(&chars[start + len..]) || !kana::is_kana(chars[start + len - 1]);
    // `帰れ|そう`: see `attaches_to_a_masu_stem`.
    let masu_stem_next = attaches_to_a_masu_stem(&chars[start + len..]);
    // `ご一緒|し|ませんか`: see `attaches_to_masu`.
    let masu_next = attaches_to_masu(&chars[start + len..]);

    // Entries are resolved *before* ordering, because the last clause of the
    // key is the entry's own sense count and every candidate in the group was
    // going to be resolved anyway -- so this costs nothing it did not already
    // pay.
    let mut built: Vec<(Candidate, Entry)> = Vec::with_capacity(group.len());
    for mut c in group {
        let mut entry = build_entry(index, chars, &c)?;
        if masu_stem_next {
            // The steps and the label change; the reading is rebuilt from the
            // first step, which does not, so it needs no second look-up.
            if let Some(stem) = potential_stem(&c, &entry) {
                c.steps.push(stem);
                entry.steps = chain::steps_for(&c);
                entry.conjugation = label::fmt(&entry.steps);
            }
        }
        built.push((c, entry));
    }
    order(
        &mut built,
        prev_is_digit,
        stem_attached,
        masu_stem_next,
        masu_next,
    );
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
/// had to add before the gates held. `cost` reads a third form as well, for
/// the cut at an empty stem: the tag on a sense that is not `aux-v`, which is
/// `lex::Candidate::usually_kana`.
fn usually_kana(entry: &Entry) -> bool {
    entry
        .senses
        .iter()
        .any(|s| s.misc.iter().any(|m| m == "uk"))
}

/// Decide which same-span candidate a caller sees first.
///
/// In sequence: an exact surface match before one only reached by folding
/// kana together; any other reading before a godan potential, with one
/// exception (see [`potential_yields`]) -- and, **when the text after the span
/// is what a 連用形 takes**, any verb before a potential and any reading
/// before a bare 仮定形 (see [`stem_yields`]); **when the text after the span
/// is the polite ます**, a verb before anything that is not one (see
/// [`attaches_to_masu`]); a `COMMON`-priority headword before
/// a rarer one; a
/// `COUNTER` entry before the rest **when the character before the span is a
/// digit**; a `PARTICLE` entry before the rest; a `PRIMARY` headword (the
/// word itself) before a `PRONOUNCE` one (a kanji entry's own reading,
/// offered only as a fallback) **unless JMdict tags the entry `uk`**, where
/// the kana spelling is the normal one and demoting it is simply wrong; any
/// reading before an adjective's bare ガル接続 stem **unless the text after
/// the span is what attaches to one** (see [`is_bare_stem`]); a
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
/// **The potential clause sits above `COMMON`, and every position below it
/// was measured first** over twelve visual-novel corpora (151,225 sentences).
/// A potential is derived, and the words it collides with are JMdict's own:
/// with no clause, `弾ける` read 弾く Potential, `擦れて` 擦る, `塗れた` 塗る,
/// `解れている` 解る, `いけない` 行く Potential Negative (about 400 times) and
/// `とけちゃう` 解く -- each because the godan is `COMMON` and the other word
/// is not, or is a reading `PRONOUNCE` demotes. Placed below `COMMON` it still
/// left `こすれる`, `めくれる`, `つなげる` and `かすめる` reading as the
/// transitive's potential. Every clause, placed anywhere, moves only
/// potentials: in a group without one it is false throughout.
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
pub fn order(
    built: &mut [(Candidate, Entry)],
    prev_is_digit: bool,
    stem_attached: bool,
    masu_stem_next: bool,
    masu_next: bool,
) {
    let non_verb_here = built
        .iter()
        .any(|(c, e)| !is_godan_potential(&c.steps) && !is_verb(e));
    // Asked only where `stem_yields` reads it: `is_verb` walks every sense.
    let verb_here = masu_stem_next
        && built
            .iter()
            .any(|(c, e)| !is_godan_potential(&c.steps) && !is_bare_conditional(c) && is_verb(e));
    let masu_verb = |c: &Candidate, e: &Entry| takes_masu(c) && is_verb(e);
    let verb_before_masu = masu_next && built.iter().any(|(c, e)| masu_verb(c, e));
    built.sort_unstable_by_key(|(c, e)| {
        (
            !c.exact,
            if masu_stem_next {
                stem_yields(c, e, verb_here)
            } else {
                potential_yields(c, e, non_verb_here)
            },
            verb_before_masu && !masu_verb(c, e),
            !c.flags.contains(WordFlags::COMMON),
            !(prev_is_digit && c.flags.contains(WordFlags::COUNTER)),
            !c.flags.contains(WordFlags::PARTICLE),
            c.flags.contains(WordFlags::PRONOUNCE) && !usually_kana(e),
            !stem_attached && is_bare_stem(c),
            c.steps.len(),
            // More senses first. JMdict's own editorial content, not a corpus
            // count -- and placed last, where `entry_id` was about to decide
            // arbitrarily, so it never overrules a clause that means anything.
            std::cmp::Reverse(e.senses.len()),
            c.entry_id,
        )
    });
}

/// Whether a godan potential yields to the span's other readings: always,
/// with one exception. The kana spelling of a JMdict `aux-v` verb's potential
/// -- 成る, 行く, 貰う, the grammaticalized verbs whose potential is common in
/// running text -- does not yield to another *verb*: `なれた` as 成る
/// Potential stays level with 慣れる, and `PRONOUNCE` then demotes 慣れる,
/// which JMdict writes in kanji. Yielding unconditionally read about 300
/// `なれ-` spans as 慣れる where the text says "can become".
///
/// The exception is narrow on both sides, and each side was measured. It
/// still yields to a word that is not a verb -- `いけない` is the expression,
/// not 行く Potential Negative. And it is only for the kana reading of a kanji
/// verb: a potential spelled as its entry's own headword yields regardless,
/// because JMdict's kana-only `とく` (the ておく contraction) and 捲る (whose
/// まくる sense is `aux-v`) are `aux-v` too, and exempting them read `とける`
/// as とく (55 spans) and `捲れる` as 捲る Potential. Every other potential
/// yields: とけた is 溶ける before 解く Potential.
fn potential_yields(c: &Candidate, e: &Entry, non_verb_here: bool) -> bool {
    is_godan_potential(&c.steps) && (non_verb_here || !is_kana_aux_verb(c, e))
}

/// [`potential_yields`] for a span before text that only a 連用形 or 未然形
/// takes ([`attaches_to_a_masu_stem`]), where two things differ.
///
/// A bare 仮定形 is no reading there at all -- it stands only before ば -- so
/// it yields to everything, whichever class it is: `くれ|そう` is くれる, not
/// 来る "Conditional". Its godan siblings have become the potential's stem
/// ([`potential_stem`]) and are ranked as potentials. And a word that is not a
/// verb cannot take そう or ながら either, so there a potential yields only to
/// another verb: `帰れ|そう` is 帰る Potential rather than JMdict's `帰れ`,
/// "go home", which `potential_yields` would put first, while `赤らめ|ながら`
/// is still 赤らめる and not 赤らむ Potential. The `aux-v` exception stands.
fn stem_yields(c: &Candidate, e: &Entry, verb_here: bool) -> bool {
    is_bare_conditional(c) || (is_godan_potential(&c.steps) && verb_here && !is_kana_aux_verb(c, e))
}

/// The kana reading of a JMdict `aux-v` verb: [`potential_yields`]' exception.
fn is_kana_aux_verb(c: &Candidate, e: &Entry) -> bool {
    c.flags.contains(WordFlags::PRONOUNCE)
        && e.senses.iter().any(|s| s.pos.iter().any(|p| p == "aux-v"))
}

/// Whether `cand`'s whole chain is one bare 仮定形, the form that stands
/// only before ば.
///
/// Not the imperative, although a godan's shares its kana: `lex` keeps one
/// chain per `(len, entry_id)`, and where a 連用形 and an imperative spell the
/// same kana the survivor can be the imperative -- 呉れる's `くれ` is 一段・クレル
/// 命令ｅ -- so demoting it demoted the only reading くれる had, and `くれ|そう`
/// read 暮れ "sunset" first. A godan's 命令ｅ never survives over its 仮定形.
fn is_bare_conditional(cand: &Candidate) -> bool {
    matches!(cand.steps.as_slice(), [only]
        if only.aux.is_none()
            && CLASSES[only.class as usize].forms[only.form as usize].name == "仮定形")
}

/// The potential's zero-width 連用形, to push onto `cand` when it is a bare
/// godan 仮定形 whose entry `lex` would let the potential reach
/// ([`potential_allowed`]); `None` otherwise.
fn potential_stem(cand: &Candidate, entry: &Entry) -> Option<ConjStep> {
    let [only] = cand.steps.as_slice() else {
        return None;
    };
    if !potential_allowed(&entry.senses, cand.flags) {
        return None;
    }
    potential_stem_on(only)
}

/// What follows a verb's 連用形 or 未然形 and never its 仮定形, by the text
/// right after the span, among what the table does not chain: 様態 そう, ながら,
/// つつ, すぎる, まくる, 始める and 続ける, the particle たり, ちゃ (ては), and
/// なく where なくなる is a span of its own.
///
/// The godan potential's 連用形 and 未然形 are empty, and `table::search` never
/// ends a chain on an empty step, so `帰れそう` reaches 帰る only as its bare
/// 仮定形 and read "Conditional". Before this text the e-row kana is the
/// potential's stem and nothing else. The same span is where JMdict's 一段
/// verbs were shown *after* a godan homograph's bare 仮定形 -- `赤らめ|ながら`
/// read 赤らむ "Conditional" and `照れ|ながら` 照る -- because the two tied on
/// every clause down to the sense count.
///
/// **Why here and not in the table.** Admitting the bound chain -- letting
/// `search` end on the zero-width step when this text follows -- was the
/// other way, and it would change nothing but the label: the chain has the
/// same span, the same flags and the same interior as the bare 仮定形, so
/// `cost` prices the two identically and no boundary can move. It would also
/// collapse into the bare 仮定形 in `lex::dedup_shallowest`, which keeps the
/// shallower chain for a `(len, entry_id)`, so it needs an exception there
/// too, and it still could not reorder the 一段 readings above. The judgment
/// is one about display, keyed on the text after the span, which is
/// [`is_bare_stem`]'s shape.
///
/// **Measured 2026-09-23** over twelve visual-novel corpora (151,225
/// sentences): no span moves in either state; 440 sentences change their first
/// entry or label hints on and 494 hints off. Adjudicated by key, 379 spans
/// are repaired hints on and 398 hints off, against 14 and 23 newly wrong --
/// each a potential giving way to another verb's chain or a noun where the
/// potential was meant: `出せ|そう` as 出る "Causative" (the table lets せる
/// attach to 一段's empty 未然形, which the language does not do), `止まれ` as
/// 止む, `擦れ|まくって` as the noun 擦れ.
fn attaches_to_a_masu_stem(rest: &[char]) -> bool {
    matches!(
        rest,
        ['そ', 'う', ..]
            | ['な', 'が', 'ら', ..]
            | ['つ', 'つ', ..]
            | ['す' | '過', 'ぎ', ..]
            | ['ま', 'く', ..]
            | ['始', 'め', ..]
            | ['続', 'け', ..]
            | ['た', 'り', ..]
            | ['ち', 'ゃ', ..]
            | ['な', 'く', ..]
    )
}

/// Whether the text after the span is the polite ます, which only a verb's
/// 連用形 takes -- so before it, a reading that is not a verb is no reading.
///
/// `ご一緒|し|ませんか` and `どう|し|ましょうか` are the shape. JMdict lists
/// `ませんか` and `ましょうか` as expressions, and the lone `し` in front of
/// them led with the conjunctive particle し, "and; besides", ahead of する,
/// because `order`'s `PARTICLE` clause puts a particle first wherever the two
/// tie above it. The same span's 氏, 詩 and 市 were no better.
///
/// **Separate from [`attaches_to_a_masu_stem`] on purpose.** That list's そう
/// and ながら follow an adjective's stem and a noun as well (重|そう,
/// 残念|ながら), so "a non-verb yields" would be wrong there; ます follows
/// nothing but a verb.
///
/// **What it leaves out, and why.** ますます is the adverb, after a particle
/// as often as not (`でますます`, `にますます` -- で and に are also 出る's and
/// 似る's 連用形, which this would put first). A bare まし is 増し, "better"
/// (`よりまし`, where より is also 寄る's), and まして is "let alone". So ます
/// counts unless it doubles, and まし only as ました or ましょ.
///
/// **Only a verb ending where ます attaches counts**, [`takes_masu`]: a
/// reading whose chain ends in any other form is no more possible before ます
/// than a noun is. Counting every verb read `いた|だけ|ませんか` as 抱く
/// "Conditional", and `イキ|ます` as イキる and `発し|て|ます` as 照る "to
/// shine", because 五段・ラ行's 体言接続特殊２ has an empty suffix and so every
/// ラ行 verb spells its own bare stem.
///
/// **Measured 2026-09-26** over twelve visual-novel corpora (151,225
/// sentences), by entry id rather than headword, since `あり`'s repair keeps
/// its headword: no span moves in either state and gold is identical in both.
/// Hints on 103 spans change their first entry and every one is a repair --
/// `あり` 蟻 → 在り 43, `し` → する 36, `ありえ|ません`'s `え` → 得る 9, `て|ます`
/// → the ている contraction てる 5. Hints off 732 change, 725 repairs (`あり`
/// 646, `し` 27, `違い` 10, `頑張り` 8, `居` おる → いる 4) and 7 trading one
/// wrong reading for another on a span already cut wrong (`しゃ|ぶり`,
/// `ます|ます`). None newly wrong.
fn attaches_to_masu(rest: &[char]) -> bool {
    match rest {
        ['ま', 'す', 'ま', 'す', ..] => false,
        ['ま', 'せ' | 'す', ..] | ['ま', 'し', 'た' | 'ょ', ..] => true,
        _ => false,
    }
}

/// Whether `cand` could stand before ます: uninflected, or its chain ends in
/// the 連用形 ます takes.
///
/// Or in the 未然形, because `lex::dedup_shallowest` keeps one chain per
/// `(len, entry_id)` and 一段 and サ変 spell the two forms the same, so the
/// chain that survives for `し` or `あけ` is the 未然形 -- counting only the
/// 連用形 undid the whole repair. A godan's 未然形 is a different kana (行か)
/// and does not stand before ます to be let in. Uninflected counts because
/// JMdict lists 在り, whose headword is the classical verb's own form, and
/// `あり|ません` led with 蟻, "ant".
fn takes_masu(cand: &Candidate) -> bool {
    cand.steps.last().is_none_or(|s| {
        matches!(
            CLASSES[s.class as usize].forms[s.form as usize].name,
            "連用形" | "未然形"
        )
    })
}

/// Whether some sense of `e` is a verb: a POS that conjugates in a class that
/// is not an adjective's, or `adj-f`, JMdict's tag for a verb form listed on
/// its own to modify a noun -- `慣れた` is 慣れる's past, not a word the
/// potential could be, and counting it a non-verb read なれた as 慣れる again.
fn is_verb(e: &Entry) -> bool {
    e.senses
        .iter()
        .flat_map(|s| &s.pos)
        .any(|p| p == "adj-f" || classes_for_pos(p).any(|c| !CLASSES[c as usize].is_adjective))
}

/// Whether `cand`'s whole chain is one adjective's bare ガル接続 stem.
///
/// IPADIC gives an adjective that form only as the site がる, さ and そう
/// attach to (楽し|そう, 寂し|さ), so on its own it is a stem rather than a
/// word, and `order`'s shallower-chain clause, which reads one step as the
/// likelier parse, gets it backwards. `したい` is an `adj-i` headword whose
/// stem is `した`, so every standalone `した` -- 承諾|した, 後悔|した -- led with
/// "want to do" instead of する Past: **3,942 spans of 3,942** over twelve
/// visual-novel corpora, hints on, and `愛し続けて`'s `愛し` led with 愛しい
/// rather than 愛する. Where the stem is the only reading -- 恥ずかし|そう,
/// 楽し|そう -- there is nothing for it to lose to and nothing moves.
///
/// **Only demoted when nothing attaches to it** ([`attaches_to_a_stem`]). The
/// unconditional clause was measured first and it is wrong in the other
/// direction: where a stem *is* the word, the one it loses to is a stranger
/// that happens to share the surface -- 苦し|そう read 苦い 82 times,
/// くすぐった|さ read くすぐる 38, 太|さ 太る 24, 重|そう 重る 18, 難し|そう
/// 難い 11 -- about 220 spans against 4,000 repaired.
fn is_bare_stem(cand: &Candidate) -> bool {
    matches!(cand.steps.as_slice(), [only]
        if only.aux.is_none()
            && CLASSES[only.class as usize].forms[only.form as usize].name == "ガル接続")
}

/// What IPADIC attaches to an adjective's ガル接続 stem, by the text right
/// after the span: the nominal さ (高さ), 様態 そう (高そう), げ (楽しげ),
/// すぎる (高すぎ), め and み (高め, 甘み), and がる in its own forms -- が
/// followed by る, っ, ら, り, れ or ろ, so that the particle `が` after a past
/// (`したが`) does not count.
///
/// Plus the adjective's own `い` and `かっ`: a span that stops at the stem
/// with those next is the adjective cut in two (鈍|い, 難し|かった), which
/// happens hints-off, and there too the stem is the word. Every entry here
/// can only hand a span back to the order it had before the clause existed.
fn attaches_to_a_stem(rest: &[char]) -> bool {
    matches!(
        rest,
        ['さ' | 'げ' | 'め' | 'み' | 'い', ..]
            | ['そ', 'う', ..]
            | ['す', 'ぎ', ..]
            | ['か', 'っ', ..]
            | ['が', 'る' | 'っ' | 'ら' | 'り' | 'れ' | 'ろ', ..]
    )
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
        let tail: String = chars[first_step.range.end..end]
            .iter()
            .copied()
            .map(kana::to_hiragana)
            .collect();
        return Some(format!("{prefix}{stem}{tail}"));
    }
    let stem = dict_reading.strip_suffix(class.remove)?;
    // Hiragana, whatever the input wrote: a reading is spelled in hiragana and
    // `table::search` no longer requires the tail to be. See `kana::to_hiragana`.
    let tail: String = chars[first_step.range.start..end]
        .iter()
        .copied()
        .map(kana::to_hiragana)
        .collect();
    Some(format!("{stem}{tail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
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
            usually_kana: false,
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

    // `order`'s clauses, isolated one at a time against hand-built
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
            usually_kana: false,
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
        order(&mut cands, false, false, false, false);
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
        order(&mut after_digit, true, false, false, false);
        assert_eq!(
            after_digit[0].0.entry_id, 2,
            "after a digit the counter sorts first"
        );

        // `去|年`: no digit, so the clause must not fire and the id decides.
        let mut elsewhere = build();
        order(&mut elsewhere, false, false, false, false);
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
        order(&mut cands, false, false, false, false);
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
        order(&mut cands, true, false, false, false);
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
        order(&mut cands, false, false, false, false);
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
        order(&mut mixed, false, false, false, false);
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
        order(&mut cands, false, false, false, false);
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
        order(&mut plain, false, false, false, false);
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
        order(&mut cands, false, false, false, false);
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
        order(&mut cands, false, false, false, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "the PRIMARY match sorts first despite the higher entry_id"
        );
    }

    /// `した`: したい's bare ガル接続 stem (one step) against する Past (two).
    /// The shallower-chain clause alone would pick the stem, and entry_id
    /// agrees with it, so this passes only if the bare-stem clause fires.
    #[test]
    fn order_puts_a_real_reading_before_a_bare_adjective_stem() {
        let class_id = |name: &str| {
            CLASSES.iter().position(|c| c.name == name).expect("class") as crate::engine::ClassId
        };
        let form_id = |class: &str, form: &str| {
            CLASSES[class_id(class) as usize]
                .forms
                .iter()
                .position(|f| f.name == form)
                .expect("form") as crate::engine::FormId
        };
        let step = |class: &str, form: &str, aux: Option<&'static str>| ConjStep {
            class: class_id(class),
            form: form_id(class, form),
            range: 0..0,
            aux,
        };
        let (mut stem, stem_entry) = order_candidate(1, true, WordFlags::COMMON, 0);
        stem.steps = vec![step("形容詞・アウオ段", "ガル接続", None)];
        let (mut past, past_entry) = order_candidate(2, true, WordFlags::COMMON, 0);
        past.steps = vec![
            step("サ変・スル", "連用形", None),
            step("特殊・タ", "基本形", Some("た")),
        ];
        let mut cands = vec![
            (stem.clone(), stem_entry.clone()),
            (past.clone(), past_entry.clone()),
        ];
        order(&mut cands, false, false, false, false);
        assert_eq!(
            cands[0].0.entry_id, 2,
            "する Past sorts before したい's bare stem despite the deeper chain and higher id"
        );
        // したそう / したがる: the stem is the word, and the clause stands down.
        let mut attached = vec![(past, past_entry), (stem, stem_entry)];
        order(&mut attached, false, true, false, false);
        assert_eq!(
            attached[0].0.entry_id, 1,
            "before そう the bare stem is the reading"
        );
    }

    #[test]
    fn only_what_ipadic_attaches_to_a_stem_counts() {
        let c = |s: &str| s.chars().collect::<Vec<char>>();
        for yes in [
            "さ",
            "そうだ",
            "げに",
            "すぎる",
            "め",
            "み",
            "がる",
            "がって",
            "い",
            "かった",
        ] {
            assert!(attaches_to_a_stem(&c(yes)), "{yes}");
        }
        // The particle が after a past, その after した, and the end of text.
        for no in ["が", "がいい", "その", "す", "と", ""] {
            assert!(!attaches_to_a_stem(&c(no)), "{no:?}");
        }
    }

    #[test]
    fn only_the_polite_masu_counts_as_masu() {
        let c = |s: &str| s.chars().collect::<Vec<char>>();
        for yes in ["ませんか", "ます", "ました", "ましょうか", "ませ"] {
            assert!(attaches_to_masu(&c(yes)), "{yes}");
        }
        // The adverb ますます, 増し and まして, and the end of text.
        for no in ["ますます", "まし", "ましだ", "まして", "ま", "まだ", ""] {
            assert!(!attaches_to_masu(&c(no)), "{no:?}");
        }
    }

    /// `ご一緒|し|ませんか`: before ます the lone し is する, not the particle
    /// し. The particle gets the lower `entry_id` and both are `COMMON`, so
    /// only the new clause can decide it; without ます next, the particle
    /// clause still puts the particle first. する carries the 未然形 chain,
    /// because that is the one `lex` keeps for し.
    #[test]
    fn before_masu_a_verb_goes_ahead_of_a_particle() {
        let step = |class_name: &str, form_name: &str| {
            let class = CLASSES
                .iter()
                .position(|c| c.name == class_name)
                .expect("class");
            let form = CLASSES[class]
                .forms
                .iter()
                .position(|f| f.name == form_name)
                .expect("form");
            ConjStep {
                class: class as crate::engine::ClassId,
                form: form as crate::engine::FormId,
                range: 0..0,
                aux: None,
            }
        };
        let reading = |id: u32, flags: WordFlags, steps: Vec<ConjStep>, pos: &str| {
            let (mut c, mut e) = order_candidate(id, true, flags, 0);
            c.steps = steps;
            e.senses[0].pos = vec![pos.to_string()];
            (c, e)
        };
        let particle = reading(
            1,
            WordFlags(WordFlags::COMMON.0 | WordFlags::PARTICLE.0),
            Vec::new(),
            "conj",
        );
        let mut suru = reading(
            2,
            WordFlags(WordFlags::COMMON.0 | WordFlags::PRONOUNCE.0),
            vec![step("サ変・スル", "未然形")],
            "vs-i",
        );
        suru.1.senses[0].misc = vec!["uk".to_string()];
        let winner = |masu_next: bool| {
            let mut cands = vec![particle.clone(), suru.clone()];
            order(&mut cands, false, false, false, masu_next);
            cands[0].0.entry_id
        };
        assert_eq!(winner(true), 2, "し before ませんか is する");
        assert_eq!(winner(false), 1, "elsewhere the particle still leads");

        // A verb that does not end where ます attaches is no reading there:
        // `いた|だけ|ませんか` is not 抱く's bare 仮定形 and `発し|て|ます` is not
        // 照る's empty 体言接続特殊２, so neither sends the particle down. Each
        // gets the lowest id.
        for (class, form, pos) in [
            ("五段・カ行イ音便", "仮定形", "v5k"),
            ("五段・ラ行", "体言接続特殊２", "v5r"),
        ] {
            let other = reading(0, WordFlags::COMMON, vec![step(class, form)], pos);
            let mut cands = vec![other, particle.clone()];
            order(&mut cands, false, false, false, true);
            assert_eq!(cands[0].0.entry_id, 1, "{form} is no reading before ます");
        }
    }

    /// A godan potential against the readings it collides with, one pair per
    /// clause. Each trap gives the wrong candidate the lower `entry_id` and,
    /// where it matters, the only `COMMON`, so a pair passes only if the
    /// clause under test decides it.
    #[test]
    fn order_puts_the_dictionarys_word_before_a_godan_potential() {
        let class_id = |name: &str| {
            CLASSES.iter().position(|c| c.name == name).expect("class") as crate::engine::ClassId
        };
        let step = |class: &str, form: &str, aux: Option<&'static str>| ConjStep {
            class: class_id(class),
            form: CLASSES[class_id(class) as usize]
                .forms
                .iter()
                .position(|f| f.name == form)
                .expect("form") as crate::engine::FormId,
            range: 0..0,
            aux,
        };
        let potential = vec![
            step("五段・カ行イ音便", "仮定形", None),
            step("一段", "連用形", Some("る")),
            step("特殊・タ", "基本形", Some("た")),
        ];
        let ichidan = vec![
            step("一段", "連用形", None),
            step("特殊・タ", "基本形", Some("た")),
        ];
        let reading = |id: u32, flags: WordFlags, steps: &[ConjStep], pos: &[&str], uk: bool| {
            let (mut c, mut e) = order_candidate(id, true, flags, 0);
            c.steps = steps.to_vec();
            e.senses[0].pos = pos.iter().map(|p| p.to_string()).collect();
            if uk {
                e.senses[0].misc = vec!["uk".to_string()];
            }
            (c, e)
        };
        let common = |f: WordFlags| WordFlags(f.0 | WordFlags::COMMON.0);
        let winner = |mut cands: Vec<(Candidate, Entry)>| {
            order(&mut cands, false, false, false, false);
            cands[0].0.entry_id
        };

        // 弾けた in kanji: 弾く's potential is COMMON and 弾ける is not, and the
        // text spells 弾ける, so the dictionary's word goes first anyway.
        let kanji_potential = reading(1, common(WordFlags::PRIMARY), &potential, &["v5k"], false);
        let hajikeru = reading(2, WordFlags::PRIMARY, &ichidan, &["v1"], false);
        assert_eq!(
            winner(vec![kanji_potential, hajikeru]),
            2,
            "弾けた is 弾ける"
        );

        // In kana, a word that is not a verb goes first: いけない, not 行く
        // Potential Negative, although 行く is `aux-v`.
        let pronounce = common(WordFlags::PRONOUNCE);
        let iku = reading(1, pronounce, &potential, &["v5k-s", "aux-v"], true);
        let ikenai = reading(2, pronounce, &[], &["adj-i", "exp"], false);
        assert_eq!(winner(vec![iku.clone(), ikenai]), 2, "いけない is いけない");

        // Another verb, against the potential of an `aux-v` verb, is level
        // with it, and `PRONOUNCE` then demotes the homograph JMdict writes in
        // kanji: なれた is 成る Potential, not 慣れる.
        let nareru = reading(2, pronounce, &ichidan, &["v1"], false);
        assert_eq!(
            winner(vec![nareru.clone(), iku.clone()]),
            1,
            "なれた is 成る Potential"
        );

        // `adj-f` is a verb form for this purpose: 慣れた is listed on its own
        // as `adj-f`, and counted a non-verb it would make 成る yield again.
        let nareta = reading(2, pronounce, &[], &["adj-f"], false);
        assert_eq!(winner(vec![nareta, iku.clone()]), 1, "なれた is still 成る");

        // Against any other potential the verb goes first: とけた is 溶ける,
        // not 解く Potential, although 解く carries `uk`.
        let toku = reading(1, pronounce, &potential, &["v5k"], true);
        assert_eq!(winner(vec![toku, nareru.clone()]), 2, "とけた is 溶ける");
        // And an `aux-v` verb spelled as its own headword yields too: JMdict's
        // kana-only とく, the ておく contraction, is `aux-v`, and its potential
        // must not read とけた where 溶ける is there.
        let toku_contraction = reading(
            1,
            common(WordFlags::PRIMARY),
            &potential,
            &["v5k", "aux-v"],
            false,
        );
        assert_eq!(
            winner(vec![toku_contraction, nareru]),
            2,
            "とけた is 溶ける, not とく"
        );
    }

    /// `帰れ|そう`. Before text only a 連用形 takes, a godan's bare 仮定形
    /// becomes the potential's stem wherever `lex` would allow the potential,
    /// a bare 仮定形 of any class yields to every other reading, and a
    /// potential yields only to a verb. Each trap gives the wrong candidate
    /// the lower `entry_id` and, where it matters, the only `COMMON`.
    #[test]
    fn before_a_masu_stem_the_e_row_kana_is_the_potentials_stem() {
        let class_id = |name: &str| {
            CLASSES.iter().position(|c| c.name == name).expect("class") as crate::engine::ClassId
        };
        let step = |class: &str, form: &str| ConjStep {
            class: class_id(class),
            form: CLASSES[class_id(class) as usize]
                .forms
                .iter()
                .position(|f| f.name == form)
                .expect("form") as crate::engine::FormId,
            range: 0..0,
            aux: None,
        };
        let reading = |id: u32, flags: WordFlags, steps: &[ConjStep], pos: &[&str], uk: bool| {
            let (mut c, mut e) = order_candidate(id, true, flags, 0);
            c.steps = steps.to_vec();
            e.senses[0].pos = pos.iter().map(|p| p.to_string()).collect();
            if uk {
                e.senses[0].misc = vec!["uk".to_string()];
            }
            (c, e)
        };
        let common = |f: WordFlags| WordFlags(f.0 | WordFlags::COMMON.0);
        let winner = |mut cands: Vec<(Candidate, Entry)>, masu_stem_next: bool| {
            order(&mut cands, false, false, masu_stem_next, false);
            cands[0].0.entry_id
        };
        let kateikei = step("五段・ラ行", "仮定形");

        // The relabel, and the refusal it shares with `lex`: 帰る in kanji takes
        // the potential; the kana reading of a verb JMdict writes in kanji does
        // not, and stays a bare 仮定形.
        let (kaeru, kaeru_entry) = reading(
            3,
            common(WordFlags::PRIMARY),
            std::slice::from_ref(&kateikei),
            &["v5r"],
            false,
        );
        let stem = potential_stem(&kaeru, &kaeru_entry).expect("帰れ|そう is 帰る's potential");
        let potential = vec![kateikei.clone(), stem];
        assert_eq!(label::fmt(&potential).as_deref(), Some("Potential"));
        let (kana, kana_entry) = reading(
            3,
            WordFlags::PRONOUNCE,
            std::slice::from_ref(&kateikei),
            &["v5r"],
            false,
        );
        assert!(
            potential_stem(&kana, &kana_entry).is_none(),
            "no uk, no potential"
        );

        // 帰れ|そう: JMdict's `帰れ` "go home" is not a verb and cannot take そう,
        // so the potential does not yield to it there -- and still does
        // anywhere else.
        let kaeru_potential = reading(3, common(WordFlags::PRIMARY), &potential, &["v5r"], false);
        let kaere = reading(1, WordFlags::PRIMARY, &[], &["exp"], false);
        assert_eq!(
            winner(vec![kaere.clone(), kaeru_potential.clone()], true),
            3,
            "帰れそう is 帰る"
        );
        assert_eq!(
            winner(vec![kaere, kaeru_potential], false),
            1,
            "elsewhere the potential yields"
        );

        // 赤らめ|ながら: a verb is there, so the potential yields to it.
        let akaramu = reading(1, common(WordFlags::PRIMARY), &potential, &["v5m"], false);
        let akarameru = reading(
            2,
            WordFlags::PRIMARY,
            &[step("一段", "連用形")],
            &["v1"],
            false,
        );
        assert_eq!(
            winner(vec![akaramu, akarameru.clone()], true),
            2,
            "赤らめながら is 赤らめる"
        );

        // くれ|そう: カ変's 仮定形 takes no potential, and yields as it is.
        let kuru = reading(
            1,
            common(WordFlags::PRONOUNCE),
            &[step("カ変・クル", "仮定形")],
            &["vk"],
            true,
        );
        assert!(
            potential_stem(&kuru.0, &kuru.1).is_none(),
            "カ変's くれ takes none"
        );
        let kureru = reading(
            2,
            WordFlags::PRONOUNCE,
            &[step("一段", "連用形")],
            &["v1"],
            true,
        );
        assert_eq!(
            winner(vec![kuru.clone(), kureru.clone()], true),
            2,
            "くれそう is くれる"
        );
        assert_eq!(
            winner(vec![kuru, kureru], false),
            1,
            "elsewhere nothing moves"
        );
        // 呉れる's くれ can survive `lex`'s dedup as 一段・クレル 命令ｅ. It is a
        // verb, not a bare 仮定形, and must not yield to 暮れ "sunset".
        let kureru_imperative = reading(
            2,
            common(WordFlags::PRONOUNCE),
            &[step("一段・クレル", "命令ｅ")],
            &["v1-s"],
            true,
        );
        let kure = reading(1, common(WordFlags::PRONOUNCE), &[], &["n"], false);
        assert_eq!(
            winner(vec![kure, kureru_imperative], true),
            2,
            "くれそう is not 暮れ"
        );

        // なれ|そう: the kana potential of an `aux-v` verb keeps its exception.
        let naru = reading(
            3,
            common(WordFlags::PRONOUNCE),
            &potential,
            &["v5r", "aux-v"],
            true,
        );
        let nareru = reading(
            2,
            common(WordFlags::PRONOUNCE),
            &[step("一段", "連用形")],
            &["v1"],
            false,
        );
        assert_eq!(
            winner(vec![nareru, naru], true),
            3,
            "なれそう is 成る Potential"
        );
    }

    #[test]
    fn only_what_a_masu_stem_takes_counts() {
        let c = |s: &str| s.chars().collect::<Vec<char>>();
        for yes in [
            "そうだ",
            "ながら",
            "つつ",
            "すぎる",
            "過ぎた",
            "まくって",
            "始めた",
            "続ける",
            "たり",
            "ちゃう",
            "なくなる",
        ] {
            assert!(attaches_to_a_masu_stem(&c(yes)), "{yes}");
        }
        // ば is the conditional's own, よ and って follow an imperative, and
        // what the table chains (ない, た, ます) never reaches here as text.
        for no in ["ば", "よ", "って", "そ", "な", "ない", ""] {
            assert!(!attaches_to_a_masu_stem(&c(no)), "{no:?}");
        }
    }

    #[test]
    fn order_puts_a_shorter_chain_before_a_longer_one_regardless_of_id() {
        let mut cands = vec![
            order_candidate(1, true, WordFlags::PRIMARY, 2), // two-step chain, smaller id
            order_candidate(2, true, WordFlags::PRIMARY, 0), // uninflected, larger id
        ];
        order(&mut cands, false, false, false, false);
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
        order(&mut cands, false, false, false, false);
        let ids: Vec<u32> = cands.iter().map(|(c, _)| c.entry_id).collect();
        assert_eq!(ids, vec![10, 20, 30]);
    }
}
