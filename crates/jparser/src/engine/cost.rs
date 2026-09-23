// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.

//! The cost model: which span of the input a candidate should be priced at,
//! and whether a tokenizer's boundaries allow it at all.
//!
//! `is_allowed` and `cost` are deliberately separate. A span either may or
//! may not exist (`is_allowed`); among the spans that may, `cost` says which
//! the DP should prefer. Master plan §4.3's asymmetry lives in that split: a
//! span may never start or end inside a tokenizer token, but merging two
//! tokens into one span is always allowed and only ever priced.
//!
//! Design §7.2's cost formula also names a `particle-in-context` evidence
//! term, alongside `common` (`COMMON` below) and `exact` (`INEXACT` below).
//! That phase implemented the latter two only, and this paragraph used to end
//! "not implemented, not forgotten". **The particle term shipped after all**:
//! `FUNCTION_STACK` below *is* a particle-in-context term. `lex` computes
//! `Candidate::function_stack` by asking whether a span partitions into
//! candidates that each carry `COMMON` and `PARTICLE`, and `cost` prices
//! exactly that. The narrow claim that nothing in *this module* reads the flag
//! is still true, and only because the flag is read one module upstream on
//! this term's behalf. `assemble::order` reads it too, as of 2026-09-16.

use crate::engine::flags::WordFlags;
use crate::engine::kana;
use crate::engine::lex::{is_copula_desu_chain, Candidate};
use crate::engine::table::CLASSES;
use crate::engine::Boundaries;

/// Cost of using a match at all, charged once per span regardless of length.
///
/// **Master plan §4.3 removed a constant of this shape and this reintroduces
/// it, at a magnitude the guards it lacked can hold.** The original made
/// `はこう` (one match) beat `は` + `こう` (two), because a per-match price
/// always favours fewer, longer spans. Since then the engine gained
/// `PRONOUNCE_CROSS`, `is_allowed`'s boundary constraint, `FUNCTION_STACK`,
/// and `spurious_merges.txt` as a live 26-row gate — so the question was
/// worth re-asking with a measurement rather than assuming the old answer.
///
/// **Why it is needed.** Every other term is per character, and `COMMON` is
/// -2 against `COVER`'s +1, so a COMMON span costs -1 a character and a
/// non-COMMON one +1: *a tiling of COMMON fragments beats a longer
/// non-COMMON dictionary word at every magnitude*. Nothing per-character can
/// fix that, and the effect is not occasional — it is the dominant ordering.
/// On `ある日起きたら、英国海軍第一卿になって...` the engine ranked the cover
/// that keeps all four of `ある日`, `起きたら`, `第一` and `なって` whole
/// **eighteenth**, behind every cover that refuses at least one of them.
///
/// **Why 2.** Swept against the gold corpus, hints on, with the compound
/// metric's class lines read rather than its headline:
///
/// | M | strict | compounds | lexical | false-match | transparent | merge gate |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | 0 | 1825 | 114 | 42 | 30 / 32 | 42 / 42 | 26 / 26 |
/// | **2** | **1823** | **118** | **46** | **30 / 32** | **42 / 42** | **26 / 26** |
/// | 4 | 1744 | 106 | — | — | — | 26 / 26 |
/// | 6 | 1694 | 123 | 75 | 27 / 32 | 30 / 42 | 25 / 26 |
///
/// 6 is where `ある日` is finally kept whole, and it is refused: its compound
/// headline rises on the `lexical` class while `transparent` collapses 42 to
/// 30 and `false-match` 30 to 27 — it merges twelve compounds that *should*
/// split. §5.2 reports those classes separately for exactly this reason, and
/// the headline alone would have hidden it. 2 gains 4 compounds and breaks
/// neither split class.
///
/// **What it costs and buys.** Hints on, 2 spans (1825 -> 1823); hints off,
/// it *gains* 11 (1588 -> 1599). Compounds 114 -> 118 and 109 -> 112. Entry
/// top-1 up in both states. And it fixes `ミスった`, which the Phase 4 design
/// scoped out as a ranking defect its own change could not reach.
///
/// A symmetric `PARTICLE_CROSS` was built and measured alongside this — a
/// per-crossing surcharge for a PARTICLE match, the one shape neither
/// `PRONOUNCE_CROSS` nor `FUNCTION_STACK` catches (`それを` as "even so").
/// It is needed at M >= 6 and **not** at 2, where it reads -2 strict for +1
/// compound, so it is not here.
const MATCH: i32 = 2;

/// Extra `MATCH`, charged only to a span that is **not** a particle.
///
/// **What it separates, and why `MATCH` cannot.** `LEN_GAIN`'s square makes
/// the merge-versus-tiling comparison length-*sensitive*, but only where the
/// tiling is balanced. Peel a single character off either end and the squares
/// cancel exactly: for a non-COMMON headword of any length `n` split into a
/// COMMON piece of `n-1` and a COMMON piece of `1`, the tiling beats the
/// merge by `3 - MATCH` — the same one point at `n = 2` and at `n = 5`. That
/// is the whole of the corpus probe's top class, and the gold corpus's too:
/// `呼び|方`, `発売|所`, `副|大統領`, `部屋|着`, `脳|内`, `危険|日`,
/// `キャンプ|場`, `月|平均`, `総|売り上げ`, `一歩|先`.
///
/// `MATCH = 3` closes that gap and was measured **on today's engine, the one
/// with `LEN_GAIN`** (hints on): `lexical` 72 → 83. The sokuon-tail handoff
/// §3c records `M = 3` as inert — 1823 / 118 / 46 / 30 / 42, identical to
/// M = 2 — and that sweep is not wrong, it predates `LEN_GAIN`. The square is
/// what gives a per-span constant a class to reach. It also merges
/// `彼の`, `名の`, `煙が`, `今も` and `側から` —
/// `transparent` 40 → 35 and `false-match` 30 → 28 — because a blanket
/// per-span constant cannot see that the character being peeled off `彼の` is
/// a **particle** and the one being peeled off `呼び方` is not. That is the
/// same length-invariance argument `LEN_GAIN`'s comment makes, one level
/// down: `MATCH` prices spans, not what is in them.
///
/// So the surcharge is withheld from particles. A particle standing as its own
/// span is not a failure to find a longer word — it is the analysis success
/// criterion 3 and `spurious_merges.txt` exist to protect — so it pays
/// `MATCH` and nothing more, while two content words that a single dictionary
/// entry spans pay twice. Both split classes are unmoved on the gold corpus,
/// 30 / 32 and 40 / 42 before and after, which is the check that the five
/// regressions above did not happen.
///
/// **The general rule is wider than "withheld from particles", and is stated
/// here because the narrow phrasing hides it.** The term favours the *merge*
/// by (content pieces in the tiling − 1), and favours the *split* only where a
/// content merge tiles entirely into particles. A two-piece tiling with one
/// particle (`彼` + `の`) is therefore exactly neutral — the designed case —
/// while a three-piece tiling with two content pieces hands the merge a point.
/// Two consequences worth naming:
///
/// - **That second mode is what moves hints off**, where `LEN_GAIN` is absent,
///   the one-character-peel gap is `2n − MATCH − CONTENT_MATCH` and grows with
///   `n` instead of cancelling — so the class above is not reached there at any
///   length. What is reached is the three-piece all-content shatter: `罪|悪|感`
///   and `解|像|度` become whole, `林|間|学校` does not.
/// - **It spends margin on criterion 3's weakest row.** `ことになっている` is
///   `exp,v1`, so it is content, and it tiles `こと` + `に` + `なっている` —
///   two content pieces, so the merge gains a point. Handoff §3c records that
///   row as the one that took the spurious-merge gate 26 → 25 under
///   `M=6 FS=8 PC=12`. The gate is 26 of 26 here and `grammatical_stack` still
///   withholds `LEN_GAIN` from that merge, but how much margin is left is
///   unmeasured.
///
/// **The flag is per entry and `any`-over-senses** (`flags.rs`'s
/// `PARTICLE_POS = ["prt", "conj"]`), so one `conj` sense anywhere withholds
/// the surcharge from every headword that entry produces, kanji forms
/// included: 41 of the 104 PARTICLE entries in `gold_jmdict_subset.xml` are
/// kanji-bearing or carry a content POS (`間` `adv,conj,n`, `程` `adv,n,prt`,
/// `又` `adv,conj,pref`, `プラス`). That is a **coverage ceiling, not a risk** —
/// withholding only ever makes a span cheaper, and a merge is one span against
/// a tiling of many, so the term cannot fire the wrong way from it. A compound
/// whose peeled character is one of those simply stays in the failing class.
///
/// 1, because the gap it closes is exactly one point and the term's whole
/// argument is that it should reach that class and no further. The tie it
/// creates is resolved toward the merge by `lattice::beats`' last-step-length
/// tie-break — a merge's single step is always longer than the tiling's last
/// step, which is a proper suffix of it — the same way `COMMON`'s and
/// `FUNCTION_STACK`'s ties already are.
const CONTENT_MATCH: i32 = 1;

/// Discount for a matched span's length, charged as `LEN_GAIN * (len - 1)²`.
///
/// **The one term here that is not linear, and the reason it exists.** Every
/// other term is per character or per span, so the comparison between a merge
/// and a tiling of its parts is *length-invariant*: whatever separates
/// `物思いにふける` from its three parts separates `には` from `に` + `は` by the
/// same ratio. That is why no value of `MATCH` reaches this class — swept
/// 2026-09-16 at 2/3/4/6 against four `FUNCTION_STACK` values, three
/// `PARTICLE_CROSS` values and three stack definitions, and every
/// configuration that merges the seven-character idiom also merges the
/// two-character particle stack. A superlinear term is the only shape that
/// can tell a long dictionary word from a short one.
///
/// The form is taken from Ichiran (`dict.lisp:928`, coefficient sequences at
/// `:681-700`), which multiplies a per-entry score by a superlinear length
/// coefficient; it is MIT and JMdict-based, the closest architectural peer
/// this project has. Expressed additively here because this cost model sums
/// rather than multiplies.
///
/// **Gated on `bounds`, which is load-bearing and was measured.** Without a
/// tokenizer nothing constrains where a span may begin or end, and a reward
/// that grows with length then buys spurious long spans outright. Measured
/// hints-off on the gold corpus: 48 gold spans shattered and 20 swallowed
/// past a gold boundary — `こと` became `きのこ` + `と`, `いつも` became `はい` +
/// `つも`, `また` became `つつま` + `た`. Real words invented across real
/// boundaries. With the gate, every hints-off cell is identical to the digit
/// and only the hints-on half moves.
///
/// **Re-measured 2026-09-18 and refused again, and the second refusal is the
/// one to read** — the paragraph above predates `CONTENT_MATCH`,
/// `KATAKANA_MIN_PIECE`, the `uk` clause and the 〜てる/〜ている/〜ていく rows,
/// so it was a statement about an engine that no longer exists. Ungating is
/// the *only* shape that can reach the largest defect class three corpora
/// agree on: hints-off, a non-COMMON entry of length `n` tiling into `k`
/// COMMON content pieces loses by `2n + 3 − 3k`, which at `k = 2` is `2n − 3`
/// and **grows with `n`**, so no value of `MATCH` or `CONTENT_MATCH` reaches
/// it at any length. Only the square can. **On the gold corpus every cell it
/// moves improves except strict** — strict 1604 → 1525, tolerant 1789 →
/// **1811**, `compounds` 118 → **158** with `lexical` 56 → **94**,
/// `false-match` 23 → 25 and `transparent` 39 unmoved, so no split class
/// regresses, the 26-row spurious-merge gate holds at 26, and 99 of the 132
/// lost gold spans are coarsening against 33 of real damage. Strict is
/// criterion 2's own metric and it falls, so "gold says ship it" would be too
/// strong; what gold says is that the coarsening explains almost all of the
/// fall.
///
/// **The corpus says it is a wash, and a wash is not worth the risk.** Both
/// builds were dumped over 18,000 sentences of visual-novel dialogue with
/// `examples/span_dump.rs`, hints off, and every new span classified by
/// whether **both its endpoints were boundaries in the other build's own
/// parse**: 9,849 that merge adjacent base spans against **9,737 that cut
/// across one**, over about half the sentences. Read that proxy for what it
/// is — it measures agreement with the *previous* engine, not with truth, so
/// a genuine boundary correction files under "cuts across one" too. It
/// establishes that the conservative and the boundary-moving changes are
/// equally numerous, not that harm equals good. What decides it is reading
/// the changed sentences: the gains are real (`あの頃`, `居座る`, `人魚族`,
/// `州長官`, `金貸し`, `おまえさん`, `待ち合わせ`, `後にする`) and so are the
/// losses, and the losses destroy *morphology*, which is worse than an
/// unmerged compound — `話しかける` → `話しかけ` + `る`, `やってくる` →
/// `やってく` + `る`, `言いつつ` → `言`("word") + `いつつ`, `見つめながら` →
/// `見つめな` + `がら`, `もしかして` → `も` + `し` + `かして`, and
/// `かもしれない` splits again. The gold corpus's 3:1 coarsening-to-damage
/// ratio does not survive contact with 18,000 real sentences, and that is the
/// finding worth carrying more than either count.
///
/// **Narrowing it to uninflected spans does not help.** Gating hints-off on
/// `steps.is_empty()` reads better on every gold cell than ungating (strict
/// 1576, tolerant 1816, `lexical` 87) and worse on the corpus: 7,248
/// coarsening against **9,537 invented**. The mechanism is specific and is
/// the reason to expect no better from that particular predicate — an
/// inflected span gets no gain while a shorter uninflected one does, so
/// `来ていない` shatters into `来` + `ていない` (庭内, "grounds"). **Two
/// predicates have been tried and both are refused; no narrower one has been
/// found**, which is not the same as proving none exists. The shape nothing
/// here expresses is a term that sees the *residue* of a cut rather than the
/// length of a span, the way `relax::MIN_PIECE` and `KATAKANA_MIN_PIECE`
/// already do — and `cost` is per candidate and cannot see the tiling, so
/// that would be a `lattice` change rather than a constant here.
///
/// **Withheld from grammatical stacks**, `lex::is_grammatical_stack`, or the
/// gain would merge exactly what success criterion 3 forbids: at `LEN_GAIN`
/// ungated by that test, `ことになっている` swallows its `に` and the 26-row
/// `spurious_merges.txt` gate goes to 25.
///
/// **1, and magnitude is not free here** — unlike `FUNCTION_STACK` and
/// `PRONOUNCE_CROSS`, whose cells are flat across their whole ranges, this
/// term's square makes every increment a different engine. 1 is the smallest
/// value that separates the class and was the only one measured; a floor
/// variant (no gain at or below three characters) was tried and gives the
/// gain back entirely — compounds 117 against 142.
///
/// **What it buys, hints on** (hints off is unmoved by construction):
/// strict 1826 -> 1754, tolerant 1912 -> 1919, entry top-1 0.9341 -> 0.9374,
/// compounds 118 -> **142**, whose class lines are `lexical` 46 -> 72,
/// `false-match` 30 -> 30 and `transparent` 42 -> 40. The strict fall is
/// coarsening, not damage: 86 gold spans inside 43 dictionary merges becomes
/// 165 inside 74, and §5.2 records strict as punishing compound preservation
/// against a short-unit corpus by about 16.5 points. The spurious-merge gate
/// stays 26 of 26 and the snapshot is byte-identical.
const LEN_GAIN: i32 = 1;

/// Cost of one covered character in a matched span.
///
/// Per character, not per match. A per-match constant is what makes `はこう`
/// (one match) beat `は`+`こう` (two) — master plan §4.3 cause 2 — and the
/// interior constraint does not forbid that merge, because `は` and `こう`
/// are separate tokens and merging across a boundary is always permitted
/// (§4.3's asymmetry, the rule that saves `東京都`). Removing the bias
/// structurally is the only form of the fix that satisfies success criterion 3.
///
/// Task 16b considered this constant and left it untouched: `COVER`'s
/// per-character shape is what the other terms' own length-proportionality
/// (`COMMON`, this task's own fix) is built to stay neutral against, and no
/// trial found evidence bearing on its magnitude specifically.
const COVER: i32 = 1;

/// Cost of one character in an unmatched run.
///
/// Far above COVER, so a fallback run never outbids a dictionary reading.
/// Handoff §9.1 measured the opposite failure: 36 of 266 lost gold boundaries
/// were runs of punctuation lumped into one unmatched segment (`」、`, `、「喜`),
/// the one unambiguous defect in that decomposition.
///
/// Task 16b considered this constant and left it untouched: no trial in this
/// task's cost-model tuning pass touched split-vs-merge decisions between two
/// *matched* candidates, which is where every measured defect lived, so
/// nothing bore on `UNMATCHED`'s own magnitude. It remains exactly the
/// Task 9-era value, unmeasured against the gold corpus.
const UNMATCHED: i32 = 30;

/// Extra cost for each script transition inside a single unmatched run.
///
/// `UNMATCHED` alone prices a run only by its length, so a mixed run like
/// `丨FIFA` costs exactly the same whether it is kept whole or split at its
/// kanji/roman boundary — nothing about a per-character price prefers the
/// split. This surcharge is what makes leaving a script change inside one
/// span cost more than breaking there, which a flat per-character price
/// cannot do on its own (see `an_unmatched_run_prefers_splitting_at_its_
/// script_change` below).
///
/// Whole-branch review correction: an earlier version of this comment cited
/// handoff §9.1's `」、`/`、「喜` punctuation runs as this term's measured
/// motivation. `classify` below has no punctuation class — `」`, `、` and
/// `「` all classify as `Script::Other` — so a pure-punctuation pair like
/// `」、` earns no surcharge whole or split; the citation was false and is
/// removed rather than repeated. Priced well under `UNMATCHED` so it only
/// ever tips a tie between equal-length partitions, never competes with
/// `UNMATCHED` itself. Task 16b (the tuning pass, split from the original
/// Task 16) found no corpus evidence bearing on this term either way -- the
/// two diagnosed defects (master plan §3.5, handoff §9) were both in
/// `COMMON`/`CROSS`, not here — so it was left untouched. Unlike `CROSS`
/// and `PRONOUNCE_CROSS`, no corpus measurement backs this specific value
/// — chosen for sign and rough magnitude only, and still open for a future
/// pass with evidence to spend.
const SCRIPT_TRANSITION: i32 = 10;

/// Cost of crossing one tokenizer boundary.
///
/// Priced, never forbidden: master plan §4.3's asymmetry requires `東京都` to
/// merge across the `東京|都` boundary and win. Task 16b measured this down
/// from 4 to 0 against the 2,163-span gold corpus (trials 2 and 11): with
/// `COMMON` length-proportional (below), a real compound overwhelmingly ties
/// its split alternative exactly on `COVER`+`COMMON` alone, so any positive
/// `CROSS` disqualified it outright rather than merely taxing it -- trial 11
/// (`CROSS = 1`) collapsed hints-on span tolerant from 0.8521 to 0.8049 and
/// the merge count from 53 to 19. At `CROSS = 0` the tie is real, and
/// `lattice::relax`'s longer-run tie-break resolves it toward the compound.
/// `PRONOUNCE_CROSS` (below) is what still keeps a spurious PRONOUNCE-only
/// crossing (`はこう`-shaped) out even though this term no longer taxes it.
const CROSS: i32 = 0;

/// Extra cost, per boundary crossed, when a PRONOUNCE-only match crosses a
/// tokenizer boundary.
///
/// Handoff §9.5's ~28 spurious merges are every one of them a match against a
/// kanji headword's *reading*: `はいた`→履く via はく, `ももう`→揉む via もむ,
/// `がい`→甲斐, `してい`→指定, `どうし`→同士, `ことし`→今年, `になって`→荷なう.
/// **The sentence that used to follow was wrong twice and is corrected here
/// (2026-09-16).** It read "every legitimate merge in §9.1 is kanji-bearing:
/// `音がする`, `ことになっている`, `首を振る`, `これまでに`, `東京都`, `2月`" —
/// but `ことになっている` and `これまでに` carry no kanji at all, so two of its
/// own six examples falsify it, and `東京都` is not in §9.1 (the handoff's list
/// is `音がした`, `ことになっている`, `首を振った`, `これまでに`, `天と地`, `2月`
/// and `18日`). The generalisation was never load-bearing. **What actually
/// keeps this term off ordinary particles is the flag, not the script:** a
/// kana-only word is PRIMARY by `WordFlags`' own definition, so `は`, `こう`
/// and `もう` are not caught whatever they are made of.
///
/// Handoff §9.6's refusal to add a heuristic to the *metric* is unaffected:
/// there the object is a merged span the corpus carries no label for. Here the
/// flag is already computed.
///
/// Task 16b measured this term's magnitude as flat across the whole safe
/// range: 10, 5, 2 and 1 (trials 5, 7, 8, 9) all reproduced the
/// `spurious_merges.txt` fixture's exact 4-of-26 failures, no different from
/// the original 25. **"Exact corpus numbers" overstated the trial log and is
/// corrected here (2026-09-16):** only trial 5 reproduced trial 2 cell for
/// cell; trial 7 read 0.8035 (1738) / 0.8525 (1844) and trial 9 0.8040 (1739)
/// / 0.8530 (1845), and trial 8 recorded no corpus figures at all. The
/// handoff's own word was "measurement-identical", which is the honest one:
/// the spread is three spans across a 25-fold change in magnitude. Only the sign matters here,
/// not the size: trial 6 (`PRONOUNCE_CROSS = 0`) broke 6 additional fixture
/// rows (`はいた`, `はこう`, `ももう`, `になって`, `になった`, `してい`) for a
/// corpus tolerant gain (0.8521 → 0.8682 hints-on) now attributable to that
/// same spurious class reappearing, not to real compounds -- see
/// handoff §10. Left at the original value since nothing measured
/// argues for changing it, and it carries a larger margin against any
/// spurious-merge shape this corpus and fixture do not already cover.
///
/// **The 4-of-26 those trials held constant were an artefact of the fixture's
/// subject, not of this term (2026-09-09).** The test measured each surface in
/// isolation, and given two bare characters Vibrato calls `はこ`, `うん` and
/// `んで` one token each -- so the crossing count below, which looks strictly
/// *between* a span's endpoints, found nothing to price and this term could
/// not fire at any magnitude. That is why every trial in the range measured
/// identically on those rows. Re-pointed at the gold sentence each surface
/// occurs in, the fixture passes 26 of 26 and the test is no longer ignored;
/// the flatness finding above stands, since it was measured on the corpus as
/// well.
const PRONOUNCE_CROSS: i32 = 25;

/// Shortest `uk` kana spelling the exemption above trusts.
///
/// Two characters is where the reading collisions live — `はは`→義母,
/// `がい`→甲斐, `みた`→蒙, `とが`, `のか` are every one of them two — and the
/// `uk` tag does not separate them, because a two-kana word that is normally
/// written in kana is also the likeliest thing for two grammatical morphemes
/// to spell. Measured over ten corpora at no floor: `冬|だ|し` became
/// `冬|だし` glossed 出汁 "soup stock" 105 times and `愛おし|げ|に` became
/// `愛おし|げに` glossed 実に 32 times, both of them a copula or a suffix
/// swallowing the particle after it, and neither caught by
/// `grammatical_stack`, whose one-character pieces must be PARTICLE and
/// `だ`(cop) and `げ`(suf) are not. Three loses `いつ`(何時) and `あだ` and
/// keeps every longer win. `KATAKANA_MIN_PIECE` draws its floor in the same
/// place for the same reason.
const UK_MIN_LEN: usize = 3;

/// Discount per character for a match whose entry carries a priority marker.
///
/// A frequency signal JMdict supplies directly, and the only evidence
/// available for choosing between two equally-covering readings.
///
/// Task 16b trial 1: changed from a flat per-match `-2` (applied once
/// regardless of length) to `COMMON * cand.len` in `cost` (per character).
/// The flat form is exactly master plan §4.3 cause 2's bug: 東京 as one
/// COMMON match cost `2 - 2 = 0`, but 東(COMMON) + 京(COMMON) as two COMMON
/// matches cost `(1-2) + (1-2) = -2`, so the split always won. Scaled by
/// length, both cost `-2` -- a tie `lattice::relax`'s longer-run tie-break
/// resolves toward the compound. Measured against the 2,163-span gold
/// corpus: hints-off span tolerant rose from 0.6015 (1301/2163) to 0.7975
/// (1725/2163) and hints-off strict, which was not expected to move, rose
/// too (0.5895 → 0.7272) -- both directions confirm this length-scaling was
/// blocking whole-word matches from shattering into common single-character
/// fragments, not only multi-word compounds.
///
/// Magnitude was then tested separately: trial 3 (`-1`) reproduced trial 1's
/// numbers exactly on this corpus, and trial 4 (`0`, disabling the term
/// entirely) *regressed* hints-off strict below the v1 baseline (0.6657 vs
/// baseline's 0.6810) while only partially recovering tolerant, because
/// removing all evidence let some genuinely wrong merges win the length
/// tie-break too. Kept at `-2`: the sign and length-proportionality are what
/// the corpus rewards, not the specific magnitude (handoff §10's trial log
/// has both directions).
///
/// What this term structurally *cannot* fix, at any magnitude: an entry like
/// 東京都 (ent_seq 1447740 in `gold_jmdict_subset.xml`) that carries no
/// `ke_pri`/`re_pri` of its own while its parts (東, 京都) individually do.
/// The split's aggregate COMMON discount is then strictly larger than the
/// compound's zero, by an amount proportional to whatever `COMMON` is set
/// to -- the sign of that comparison does not depend on the magnitude. This
/// is JMdict's own priority-tagging gap (a well-known place name is simply
/// untagged), not something this cost term has the information to see; it
/// is part of why hints-off/on tolerant still trail baseline after tuning.
const COMMON: i32 = -2;

/// Penalty for a match whose span is nothing but stacked common function
/// words -- `には` where `に` + `は` are both there.
///
/// A tie-breaker, not a tax. `には` as one COMMON entry of length 2 costs
/// `COVER*2 + COMMON*2 = -2`; `に` + `は` as two COMMON entries of length 1
/// cost `(1-2) + (1-2) = -2` as well, and `lattice::relax`'s longer-run
/// tie-break then hands the tie to the merge. Any positive value here hands
/// it back to the split, which is why this is flat rather than
/// length-proportional: `COMMON`'s proportionality exists to stay neutral
/// across lengths (see its comment), while this term is aimed at one shape.
///
/// Measured 2026-09-09 before it was written, off-engine, over the 139
/// adjudicated compound types (`examples/derivation_probe.rs`): the property
/// holds for 7 of 22 `transparent` types, 3 of 23 `false-match`, and 2 of 87
/// `lexical`. Cross-referenced against what the engine does today, that is
/// **+12 windows** it can fix (`には` in 8 sentences, `にも` in 4 -- every one
/// of the `transparent` class's misses) against **-3** it breaks (`として` in
/// 2, `となり` in 1, all three currently kept whole).
///
/// Unusually for a Phase 3 candidate, the two span metrics point the same
/// way: the short-unit gold corpus splits all four surfaces, so the merges
/// this blocks currently destroy 30 gold spans (16 + 8 + 4 + 2) that strict
/// accuracy scores as misses today. Frequency could not manage that -- doc
/// §10.3 priced it at about -6.9 points strict for the compound gain -- and
/// it is why this term was worth trying where a blanket COMMON derivation
/// (doc §10.4) was not.
///
/// **What it measured on-engine 2026-09-09, hints on — these are the cells as
/// they stood that day, and the hints-on half has since moved twice (Phase 4's
/// escape and `order`'s two new clauses, and twice more since for `LEN_GAIN`
/// and `CONTENT_MATCH`): "strict is 1792, tolerant 1870 and entry top-1
/// 0.9236 today" was a 2026-09-16 reading, and the cells moved four more
/// times on 2026-09-18 alone. The hints-off half below stopped being exact on 2026-09-18 for
/// the same reason. Master plan §5.2 carries the current figures and this
/// paragraph is a dated record.** strict 1737 -> 1768 spans
/// (+31), tolerant 1843 -> 1844 (+1), entry top-1 0.9063 -> 0.9092,
/// compounds 101 -> 114 windows, `transparent` 30 -> 42 of 42 with
/// `lexical` 41 -> 42 and `false-match` 30 -> 30 -- no class regressed.
/// Hints off: strict 1573 -> 1588, compounds 96 -> 109, tolerant 1725 ->
/// 1714 (-11, against 75 -> 62 merges, so it is coarsening the tolerant
/// metric was crediting; §5.2 records that metric as over-forgiving by about
/// 1.3 points for exactly this reason). Latency unmoved: `bench`'s scaling
/// cell reads 1.10x hints on against 1.12x before.
///
/// **Magnitude is flat from 1 to 50 in both hint states** -- identical cells
/// throughout -- so only the sign matters here, as with `PRONOUNCE_CROSS`
/// above. 4 is the small end: this term suppresses a *shape* that overlaps a
/// legitimate class, so unlike `PRONOUNCE_CROSS` there is no argument for
/// extra margin, and less of any lattice this corpus does not cover is
/// perturbed.
///
/// **The `steps.is_empty()` gate below is load-bearing and was measured, not
/// assumed.** Ungated, this term shattered `なりたいです` into `なり` +
/// `たいです` and `して` into `し` + `て`, because `なり` and `し` are
/// themselves COMMON particle entries -- the §4.3 shattering class this whole
/// rewrite exists to remove, and a worse defect than the merge it cures. The
/// gold corpus's own metrics *improved* through that damage; the curated
/// `parse_sentences.txt` snapshot is what caught it. With the gate the
/// snapshot is byte-identical.
const FUNCTION_STACK: i32 = 4;

/// Penalty for a span that starts where a counter starts, after a number,
/// carries no `COUNTER` itself, and loses to a number-plus-counter the
/// dictionary actually has.
///
/// `4人目` is the shape. `人目` is `[primary, common, common-line]` — a noun
/// meaning "public gaze", `ひとめ` — and it took the span, so a reader was
/// shown "four public-gaze" where the text says "the fourth person". `人` is a
/// counter starting in the same place and `4人` is an entry, so there is a
/// counted number for it to lose to.
///
/// **The "actually has" clause is what keeps this honest**, and it was added
/// after measurement rather than foresight. `1969年度` looks identical —
/// `年` is a counter, `年度` is not — but `1969年` is no entry, so nothing is
/// on offer and the compound is left alone. Without that clause the rule split
/// `年度` into `年` + `度` and `点差` into `点` + `差`, costing 3 gold spans.
/// With it, every gold cell is identical in both hint states.
///
/// It also covers what [`DIGIT_COUNTER`] cannot. That term fires only where
/// the merged reading differs from its parts, so it fixed `4人目` (`よにん`)
/// and left `3人目` and `8人目` alone, `さんにん` and `はちにん` being exactly
/// what the split says. This one is about the *entry* being wrong rather than
/// the merge being right, so the number does not matter.
///
/// 3 is the smallest magnitude that moves the cases; 4 and 6 measured
/// identically, so only the sign and clearing the gap matter.
const DIGIT_NONCOUNTER: i32 = 3;

/// Discount for a span that opens with a number and whose reading carries the
/// sound change that number makes — `1週間` as `いっしゅうかん`.
///
/// **Why it needs a term at all.** Number and counter are one phonological
/// word in Japanese and JMdict records the merged reading, but the merged
/// entry is usually `[primary, common-line]` while the counter alone is
/// `COMMON` — so `週間` earns −2 a character and `１週間` earns nothing, and
/// the split wins by 2. The reader is then shown `ichi shuukan` for a word
/// pronounced `isshuukan`: a mispronunciation, not merely a different cut.
///
/// **Why it is gated on the reading actually changing**, in
/// [`lex::Candidate::digit_counter`]: that is exactly where the merge earns
/// its keep. `一週間` reads `いっしゅうかん` and `四人` `よにん`, which
/// `いち` + `しゅうかん` and `よん` + `にん` cannot produce, so splitting
/// mispronounces them. `三回` reads `さんかい` and `二番` `にばん`, which is
/// what the split already says. Measured over 2,743 lines of real text, an
/// **ungated** discount re-cut `３|回目` as `３回|目` **49 times**, breaking a
/// genuine counter suffix to fix readings that were never wrong. Gated, 129
/// spans change and they are euphonic merges — `１週間` 62, `２人` 16 (`ふたり`,
/// not `に` + `にん`), `1週間` 15, `1ヶ月` 4.
///
/// **Magnitude.** −3 is the smallest that flips the 2-point gap above; −5 and
/// −8 measured identically on every cell, so only the sign and clearing the
/// gap matter. Gold is untouched in both hint states — every cell and every
/// compound class line identical. `1969年` is unaffected: an all-digit span
/// fails the "continues past it" half of the shape test, which is what keeps
/// years out.
///
/// **What it does not reach.** `8人目` stays `8 | 人目`, because `はちにん` is
/// exactly `はち` + `にん` and the gate correctly declines — so the same
/// construction is fixed at 4 and not at 8. The underlying fault there is
/// that `人目` is a different word (`ひとめ`, "public gaze") winning after a
/// digit, which is an entry-selection problem rather than a merge one, and
/// JMdict's `COUNTER` flag cannot separate it: `週間`, `時間` and `匹` are
/// counters and carry no flag.
const DIGIT_COUNTER: i32 = -3;

/// Shortest piece a tokenizer cut inside an all-katakana span may leave
/// before that cut counts as evidence at all.
///
/// **The mirror of `relax::MIN_PIECE`, at the same value and for the same
/// reason.** That constant asks whether an unmatchable katakana token may be
/// *reopened*: only when it tiles into candidates of three characters or
/// more, because "it decomposes into real words" and not "it is katakana" is
/// the test. This one asks the converse: when the tokenizer cuts *inside* a
/// katakana run and leaves a piece shorter than that, it has failed on a word
/// it does not know rather than proposed a boundary, so `PRONOUNCE_CROSS`
/// must not charge 25 for crossing it.
///
/// **`センパイ` is the case, and it is decided by that term alone.** JMdict has
/// no `センパイ` headword; the span reaches 先輝 through `kana::fold`, so the
/// record is the hiragana reading and is PRONOUNCE-only. Merged it costs
/// `4 + 2 + 1 - 9 + 8 - 8 = 8` against `セン` + `パイ`'s 8 -- and then pays 25
/// for IPADIC's interior cut, so the reader is shown "line" and "pie" where
/// the word is 先輝. The same word in the same corpus is kept whole in the 5
/// lines where IPADIC happens to call it one token, and split in 70 where it
/// does not.
///
/// **Swept over every katakana run in 8,334 lines of dialogue**: 1,110
/// distinct runs of three characters or more, of which the engine splits 110
/// hints-on. Eight are real words shattered (`センパイ`, `アブナイ`,
/// `サイアク`, `ヘンテコ`, `シロクロ`, `ハンパ`, `モンモン`,
/// `ナイナイ`) and **every one leaves a piece shorter than three**; 63 are
/// correct compound decompositions (`オンライン|フレンド`,
/// `レトルト|カレー`) and **every one of those has both pieces at three or
/// more**. Zero overlap on this corpus.
///
/// Safe against criterion 3 by construction: all 26 rows of
/// `spurious_merges.txt` are hiragana or kanji, and the one mixed-script row
/// (`タッとい`) is not an all-katakana run, so no row can reach this clause.
const KATAKANA_MIN_PIECE: usize = 3;

/// Penalty for a match whose source characters differ from the stored surface
/// before folding.
///
/// Every §9.5 absurdity is inexact against its headword; every §9.1
/// legitimate merge is exact modulo conjugation. Independent of
/// PRONOUNCE_CROSS.
///
/// Task 16b trial 10 halved this to 4 and measured a small regression on
/// both span metrics hints-off (strict 1573→1572, tolerant 1725→1722 spans),
/// not the gain the flat-per-match reasoning that motivated `COMMON`'s
/// change would predict for this term too (a flat penalty already
/// *disadvantages* merges relative to splits, the opposite bias from
/// `COMMON`, so halving it should have made merges relatively more
/// attractive -- it did not move the corpus meaningfully in that direction).
/// Reverted to 8; nothing measured argues for changing it.
const INEXACT: i32 = 8;

/// Whether a tokenizer allows `cand`'s span to exist at all.
///
/// A span may not *start or end* inside a token: `bounds` must call both
/// `cand.start` and `cand.start + cand.len` usable endpoints. Nothing forbids
/// a span from *crossing* boundaries in between — that is `cost`'s job, not
/// this one — which is the asymmetry that lets `東京都` survive a tokenizer
/// that cuts it as `東京|都`.
///
/// The question is [`Boundaries::is_usable_endpoint`] and not `is_boundary`,
/// and the two differ in exactly one place: a token that no candidate can
/// cover *and* that carries a hiragana character has no legal span at all
/// under the strict reading, so [`crate::engine::relax::Relaxed`] reopens its
/// interior (Phase 4 design §2 and §9). Both halves are load-bearing — an
/// all-kanji, all-katakana or all-digit token nothing covers is a proper noun
/// or a number, and stays shut. Everywhere the tokenizer and the dictionary
/// agree about where words are, the two questions have the same answer.
///
/// With `bounds == None` every constraint lifts: the engine must still work
/// with no tokenizer at all.
///
/// **One exception needs the tokenizer rather than being lifted by its
/// absence: the copula conjugating as its own headword** (`でした`, see
/// `lex::is_copula_desu_chain`). `で` + `した` is also the particle and する's
/// past -- `ここでした`, "did it here" -- and nothing in the dictionary tells
/// them apart, but IPADIC does: over twelve corpora it keeps `でし` one token in
/// 504 `でした` and cuts `で|し` in 20, and those 20 are the particle. So the
/// chain is allowed only where a tokenizer ran and did not cut there, and
/// hints-off it never exists.
pub fn is_allowed(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> bool {
    let copula = is_copula_desu_chain(cand);
    let Some(bounds) = bounds else {
        return !copula;
    };
    if copula && bounds.is_boundary(cand.start + 1) {
        return false;
    }
    bounds.is_usable_endpoint(cand.start) && bounds.is_usable_endpoint(cand.start + cand.len)
}

/// The price of matching `cand`, given where a tokenizer put its boundaries.
///
/// Sums `COVER` per character, `INEXACT` for a folded-not-exact match,
/// `COMMON` per character for a priority entry, `FUNCTION_STACK` for an
/// unconjugated stack of common function words, and `CROSS` for each interior
/// tokenizer boundary the span swallows, with `PRONOUNCE_CROSS` added on top
/// of `CROSS` per boundary when the match is PRONOUNCE-only. With
/// `bounds == None` there is nothing to cross, so only the per-character and
/// per-match terms apply.
pub fn cost(cand: &Candidate, bounds: Option<&dyn Boundaries>) -> i32 {
    let mut total = COVER * cand.len as i32 + MATCH;
    if !cand.flags.contains(WordFlags::PARTICLE) {
        total += CONTENT_MATCH;
    }
    // See `LEN_GAIN` for why this is the one superlinear term, why it is gated
    // on `bounds` the way `CROSS` and `PRONOUNCE_CROSS` below are, and why a
    // grammatical stack is excluded. `steps.is_empty()` mirrors
    // `FUNCTION_STACK`'s own gate: a conjugated form is not function-word
    // stacking, and `物思いにふけった` needs the gain its citation form gets.
    if bounds.is_some() && !(cand.grammatical_stack && cand.steps.is_empty()) {
        let l = cand.len as i32 - 1;
        total -= LEN_GAIN * l * l;
    }

    if !cand.exact {
        total += INEXACT;
    }
    if cand.flags.contains(WordFlags::COMMON) {
        total += COMMON * cand.len as i32;
    }
    // Gated on an unconjugated match. A conjugated form is not function-word
    // stacking, and without this gate the term shatters exactly the class
    // §4.3 exists to protect: measured, `なりたいです` (`なる` Want-to Polite,
    // one segment) split into `なり` + `たいです`, and `して` into `し` + `て`,
    // because `なり` and `し` are themselves COMMON particle entries. Both are
    // in `parse_sentences.txt`, so the curated snapshot caught what the gold
    // corpus's own metrics did not -- they improved through it.
    if cand.function_stack && cand.steps.is_empty() {
        total += FUNCTION_STACK;
    }
    if cand.digit_counter {
        total += DIGIT_COUNTER;
    }
    if cand.after_digit_noncounter {
        total += DIGIT_NONCOUNTER;
    }

    if let Some(bounds) = bounds {
        // Interior positions of `start..start+len`: every `p` strictly
        // between the span's own two endpoints. `start` and `start+len` are
        // excluded because crossing *those* is `is_allowed`'s concern, not a
        // price this span pays for merely existing.
        let crossings = ((cand.start + 1)..(cand.start + cand.len))
            .filter(|&p| bounds.is_boundary(p))
            .count() as i32;
        total += crossings * CROSS;
        // A crossing **strictly** inside the characters the deinflection chain
        // consumed is
        // the tokenizer cutting an *inflection*, not proposing a word
        // boundary, so it is not evidence of a spurious merge and pays
        // nothing. Everything else pays in full. The range is the chain's as a
        // whole, not each step's: `なった` decomposes into two steps, [1,2) for
        // the っ and [2,3) for the た, and IPADIC's cut lands exactly on the
        // junction between them -- inside the inflection, but inside neither
        // step. See `PRONOUNCE_CROSS`'s comment for the measurement.
        //
        // **Widening it to the chain's start was measured and refused
        // (2026-09-18).** `a <= p` exempts the stem/suffix junction, which is
        // the cut this exemption most obviously describes, and it does fix
        // `待っていた` — IPADIC cuts `待っ|て|い|た`, `いた` is `いる` Past and
        // PRONOUNCE-only because 居る carries kanji, so the boundary at the
        // `い`/`た` junction cost 25 and the engine preferred `い` + `た`,
        // glossing `い` as "the Boar". 218 corpus sentences.
        //
        // It is refused because the junction cannot distinguish that from the
        // spurious class: `はこう` is `履く` Volitional with the stem `は` and
        // the chain on `こう`, structurally identical, and the cut at `は`/`こう`
        // is the chain's start too. `くるまではこう` is criterion 3's own named
        // sentence. Measured hints on: `false-match` 30 → 26 and `transparent`
        // 40 → 37, breaking `はこう`, `はいた`, `いたから`, `とが` and `のか` ×3,
        // with entry top-1 0.9400 → 0.9314. The engine has no term here that
        // separates a stem from a particle; see the handoff for the `uk` route.
        //
        // **The narrow form, shipped 2026-09-23: an empty stem of a word JMdict
        // writes in kana.** Every one of those five breaks has a *non-empty*
        // innermost step (は|こう is 履く's 未然ウ接続 こ, の|か 退く's 未然形 か).
        // A 一段 verb's 連用形 and 未然形 are empty, so for `やめて` the chain
        // starts exactly where IPADIC cuts `やめ|て`, the cut is not strictly
        // inside it, and the kana verb split at the stem with the lone て glossed
        // "you said": `やめ|て`, `でき|た`, `あげ|ます`, `い|た`, about 4,000
        // spans over twelve visual-novel corpora hints on. So the cut is exempt
        // where the innermost step is zero-length, and only for an entry JMdict
        // tags `uk` on a sense that is not `aux-v` (`Candidate::usually_kana`,
        // the test `lex` applies to the potential). Without that second half the
        // one-kana stems collide with particles: `ことにな`, `滅多にない`,
        // `気にせず` read as 似る, `完璧でない` and `だけでなく` as 出る, and
        // `わけない` as 分ける -- none of them `uk`, while 居る, 止める, 出来る,
        // 上げる and 過ぎる are. And not for a chain that ends at ガル接続, the
        // bound form さ and そう attach to, or at `ぬ`'s 仮定形: `ない` there is
        // `な` and `ぬ` is `ね`, the one-kana pair `table.rs`'s CHAIN_BLOCK
        // refuses on every empty stem it names, because both are commoner as a
        // sentence-final particle. `それな`, `エロいな` and `やめな` (やめなさい)
        // read as 逸れる, 居る and やめる **Negative**, and `それね` as 逸れる
        // Negative Conditional.
        //
        // Measured hints on over twelve corpora (151,225 sentences): 3,641
        // change, and adjudicated by key 3,837 spans are repaired, 20 neutral,
        // 74 wrong for wrong and 40 newly wrong, among them a katakana stem
        // severed (`シゴ|いて`, 30) and `〜たから` read as the want-to form (5).
        // The five sentences above are unmoved, gold's one mover is `やめて`,
        // and hints off nothing can change, since this charge needs a
        // tokenizer.
        //
        // **The `uk` exemption (2026-09-22, experimental).** An entry JMdict
        // tags `uk` is normally written in kana, and it keeps its kanji
        // headword, so the spelling JMdict itself calls normal is a
        // PRONOUNCE-only record and pays this charge. Measured over ten
        // corpora: 918 occurrences hints-on where the ta-old port emits such a
        // word whole and this engine shatters it — `おねだり` into `お|ねだり`
        // 115 times, against `お尻` merging across the same boundary because
        // 尻 is kanji.
        //
        // The exemption is a conjunction, and the other two terms are what
        // make it safe. Every false match this charge exists to refuse is
        // either not `uk` — `はこう`→履く, `になって`→荷なう, `ことし`→今年,
        // `どうし`→同士, `してい`→指定, `がい`→甲斐, `はは`→義母 — or
        // reached through a deinflection chain: `ももう`→揉む, `はもう`→食む,
        // `のか`→退く, `とかな`→解く, `たまん`→溜まる are all `uk` and all
        // inflected. `assemble.rs`'s refusal of a `uk` bit in `WordFlags`
        // names `ことになって` swallowing its `に` through `なる`, and that is
        // the same shape: `なって` is a chain, so the conjunction never fires
        // on it.
        let uk_spelling = cand.flags.contains(WordFlags::UK)
            && cand.len >= UK_MIN_LEN
            && cand.exact
            && cand.steps.is_empty()
            && !cand.grammatical_stack;
        if cand.flags.contains(WordFlags::PRONOUNCE) && !uk_spelling {
            let chain = cand
                .steps
                .iter()
                .fold(None, |acc: Option<(usize, usize)>, s| {
                    Some(match acc {
                        None => (s.range.start, s.range.end),
                        Some((a, b)) => (a.min(s.range.start), b.max(s.range.end)),
                    })
                });
            // See the narrow form above.
            let ends_bound = |s: &crate::engine::ConjStep| {
                let form = CLASSES[s.class as usize].forms[s.form as usize].name;
                form == "ガル接続" || (s.aux == Some("ぬ") && form == "仮定形")
            };
            let empty_stem_end = cand
                .steps
                .first()
                .filter(|s| cand.usually_kana && s.range.is_empty())
                .filter(|_| !cand.steps.last().is_some_and(ends_bound))
                .map(|s| s.range.start);
            let charged = ((cand.start + 1)..(cand.start + cand.len))
                .filter(|&p| bounds.is_boundary(p))
                // See `KATAKANA_MIN_PIECE`.
                .filter(|&p| {
                    !(cand.all_katakana
                        && (p - cand.start < KATAKANA_MIN_PIECE
                            || cand.start + cand.len - p < KATAKANA_MIN_PIECE))
                })
                .filter(|&p| !matches!(chain, Some((a, b)) if a < p && p < b))
                .filter(|&p| empty_stem_end != Some(p))
                .count() as i32;
            total += charged * PRONOUNCE_CROSS;
        }
    }

    total
}

/// A character's script class, for `unmatched_cost`'s transition surcharge.
#[derive(PartialEq, Eq)]
enum Script {
    Kanji,
    Kana,
    Digit,
    Other,
}

fn classify(c: char) -> Script {
    if kana::is_kanji(c) {
        Script::Kanji
    } else if kana::is_kana(c) {
        Script::Kana
    } else if kana::is_digit(c) {
        Script::Digit
    } else {
        Script::Other
    }
}

/// The price of leaving `chars[start..start+len]` unmatched.
///
/// `UNMATCHED` per character, plus `SCRIPT_TRANSITION` for each adjacent pair
/// inside the run whose script class differs — the surcharge that prefers
/// breaking a mixed run like `丨FIFA` at its script change over lumping it
/// into one unmatched segment.
///
/// # Panics
///
/// Panics if `start + len` exceeds `chars.len()`; the segmentation DP is
/// expected to construct only valid ranges.
pub fn unmatched_cost(chars: &[char], start: usize, len: usize) -> i32 {
    let mut total = UNMATCHED * len as i32;
    for pair in chars[start..start + len].windows(2) {
        if classify(pair[0]) != classify(pair[1]) {
            total += SCRIPT_TRANSITION;
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ConjStep;

    struct At(&'static [usize]);
    impl crate::engine::Boundaries for At {
        fn is_boundary(&self, pos: usize) -> bool {
            self.0.contains(&pos)
        }
    }

    fn cand(start: usize, len: usize, flags: WordFlags, exact: bool) -> Candidate {
        Candidate {
            start,
            len,
            entry_id: 1,
            flags,
            steps: Vec::new(),
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

    /// The narrow exemption on `PRONOUNCE_CROSS`, one row per half of it.
    /// IPADIC cuts `やめ|て` where the chain starts, because 一段's 連用形 is
    /// empty; the cut is the inflection's only for a stem that is empty, of a
    /// word JMdict writes in kana, whose chain does not end at ガル接続 or at
    /// ぬ's 仮定形. Spans start at 4, so every position is absolute.
    #[test]
    fn a_cut_at_an_empty_stem_of_a_kana_word_is_its_inflections() {
        let step = |class: &str, form: &str, range: std::ops::Range<usize>, aux| {
            let c = CLASSES.iter().position(|c| c.name == class).expect("class");
            ConjStep {
                class: c as crate::engine::ClassId,
                form: CLASSES[c]
                    .forms
                    .iter()
                    .position(|f| f.name == form)
                    .expect("form") as crate::engine::FormId,
                range,
                aux,
            }
        };
        let none = At(&[]);
        let charge = |c: &Candidate, at: &At| cost(c, Some(at)) - cost(c, Some(&none));

        // やめて at 4: 止める, `uk`, over an empty 連用形 at 6.
        let mut yamete = cand(4, 3, WordFlags::PRONOUNCE, true);
        yamete.steps = vec![
            step("一段", "連用形", 6..6, None),
            step("不変化型", "基本形", 6..7, Some("て")),
        ];
        yamete.usually_kana = true;
        assert_eq!(
            charge(&yamete, &At(&[6])),
            CROSS,
            "やめ|て is the inflection's cut"
        );
        // A cut inside the stem is no inflection's.
        assert_eq!(
            charge(&yamete, &At(&[5])),
            CROSS + PRONOUNCE_CROSS,
            "や|めて still pays"
        );

        // The same chain for an entry JMdict writes in kanji: 出る's で is also
        // the copula, 似る's に the particle.
        let mut kanji_word = yamete.clone();
        kanji_word.usually_kana = false;
        assert_eq!(
            charge(&kanji_word, &At(&[6])),
            CROSS + PRONOUNCE_CROSS,
            "で|ない still pays"
        );

        // はこう: 履く's stem は and its 未然ウ接続 こ, not empty. `usually_kana`
        // is set, although 履く is not `uk`, so that only the empty-stem half
        // can refuse it.
        let mut hakou = cand(4, 3, WordFlags::PRONOUNCE, true);
        hakou.steps = vec![
            step("五段・カ行イ音便", "未然ウ接続", 5..6, None),
            step("不変化型", "基本形", 6..7, Some("う")),
        ];
        hakou.usually_kana = true;
        assert_eq!(
            charge(&hakou, &At(&[5])),
            CROSS + PRONOUNCE_CROSS,
            "は|こう still pays"
        );

        // それな and それね: a chain ending at ガル接続 is a stem waiting for さ or
        // そう, and ぬ's 仮定形 is the particle ね.
        for (form, aux, class, what) in [
            ("ガル接続", "ない", "特殊・ナイ", "それ|な still pays"),
            ("仮定形", "ぬ", "特殊・ヌ", "それ|ね still pays"),
        ] {
            let mut sore = yamete.clone();
            sore.steps = vec![
                step("一段", "未然形", 6..6, None),
                step(class, form, 6..7, Some(aux)),
            ];
            assert_eq!(charge(&sore, &At(&[6])), CROSS + PRONOUNCE_CROSS, "{what}");
        }
    }

    #[test]
    fn a_function_stack_pays_only_while_it_is_unconjugated() {
        // The gate `FUNCTION_STACK`'s comment calls load-bearing, as an
        // arithmetic contract rather than as a corpus number. Ungated the
        // same term shattered `なりたいです`, because a conjugated match whose
        // span happens to decompose into common particles is not stacking.
        let plain = cand(0, 2, WordFlags::COMMON, true);
        let mut stacked = plain.clone();
        stacked.function_stack = true;
        let mut conjugated = stacked.clone();
        conjugated.steps = vec![ConjStep {
            class: 0,
            form: 0,
            range: 0..2,
            aux: None,
        }];

        assert_eq!(
            cost(&stacked, None) - cost(&plain, None),
            FUNCTION_STACK,
            "an unconjugated function stack pays the term exactly once"
        );
        assert_eq!(
            cost(&conjugated, None),
            cost(&plain, None),
            "a conjugated match pays nothing, however its span decomposes"
        );
    }

    /// `CONTENT_MATCH`'s whole claim, as an arithmetic contract rather than a
    /// corpus number — the shape
    /// `a_function_stack_pays_only_while_it_is_unconjugated` above uses for
    /// `FUNCTION_STACK`'s gate.
    ///
    /// The claim is that a non-COMMON headword under bounds now **ties** its
    /// COMMON `(n-1)` + COMMON `1` tiling at every `n`, because `LEN_GAIN`'s
    /// squares cancel for that split, and still **loses by one** when the
    /// peeled character is a PARTICLE. Without this the length-invariance in
    /// the comment is unguarded: nothing else here would fail if the term
    /// became per-character, or if the particle case stopped being withheld.
    #[test]
    fn a_one_character_peel_ties_unless_the_peeled_piece_is_a_particle() {
        /// Every position is a boundary, which is what a merge and its tiling
        /// both need to be legal; the term under test is gated on `bounds`
        /// being present at all.
        struct At;
        impl Boundaries for At {
            fn is_boundary(&self, _: usize) -> bool {
                true
            }
        }
        let b: Option<&dyn Boundaries> = Some(&At);
        for n in 2..=5usize {
            let whole = cost(&cand(0, n, WordFlags::PRIMARY, true), b);
            let head = cost(&cand(0, n - 1, WordFlags::COMMON, true), b);
            assert_eq!(
                whole,
                head + cost(&cand(n - 1, 1, WordFlags::COMMON, true), b),
                "at n = {n} a content peel ties, and `lattice::beats` gives \
                 the tie to the merge"
            );
            let particle = WordFlags(WordFlags::COMMON.0 | WordFlags::PARTICLE.0);
            assert_eq!(
                head + cost(&cand(n - 1, 1, particle, true), b) + 1,
                whole,
                "at n = {n} a particle peel still beats the merge by one"
            );
        }
    }

    /// `MATCH` is charged once per span, whatever its length — that is the
    /// whole point, and it is what makes a four-character headword able to
    /// beat a stem plus its tail. The companion below pins the *other* half:
    /// per-character coverage must stay neutral, so the two terms together
    /// prefer the longer match without ever rewarding a merge that covers
    /// more ground than its parts.
    ///
    /// The per-span constant is `MATCH + CONTENT_MATCH` for a content word
    /// and `MATCH` alone for a particle; both are flat in length, which is
    /// the property this test is about, so it pins each separately rather
    /// than either alone.
    #[test]
    fn the_match_cost_is_flat_and_the_longer_span_wins_on_it() {
        let one = cost(&cand(0, 4, WordFlags::PRIMARY, true), None);
        let two = cost(&cand(0, 2, WordFlags::PRIMARY, true), None)
            + cost(&cand(2, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(
            two - one,
            MATCH + CONTENT_MATCH,
            "two content matches pay it twice, one pays it once"
        );
        assert!(one < two, "so the whole span is preferred to the split");

        let one = cost(&cand(0, 4, WordFlags::PARTICLE, true), None);
        let two = cost(&cand(0, 2, WordFlags::PARTICLE, true), None)
            + cost(&cand(2, 2, WordFlags::PARTICLE, true), None);
        assert_eq!(
            two - one,
            MATCH,
            "a particle pays `MATCH` and no surcharge, which is what lets \
             `彼` + `の` go on beating `彼の` while `呼び` + `方` no longer \
             beats `呼び方`"
        );
    }

    #[test]
    fn coverage_cost_does_not_reward_using_fewer_matches() {
        // §4.3's fix, which used to be a plain equality: `whole == split`,
        // because a per-match price is what made `はこう` beat `は` + `こう`.
        // `MATCH` reintroduced that price deliberately and at a measured
        // magnitude, so the equality now holds **once the per-match term is
        // taken out** -- which is the property §4.3 actually needed. `COVER`
        // still contributes nothing to the preference between a span and its
        // parts; only `MATCH` does, and only by its own size.
        //
        // `CONTENT_MATCH` joined `MATCH` as a per-span term in 2026-09-18 and
        // this test is restated rather than deleted: it is the same property,
        // over the same two terms, and taking *both* out is what leaves the
        // per-character equality §4.3 needed.
        let per_span = MATCH + CONTENT_MATCH;
        let whole = cost(&cand(4, 3, WordFlags::PRIMARY, true), None);
        let split = cost(&cand(4, 1, WordFlags::PRIMARY, true), None)
            + cost(&cand(5, 2, WordFlags::PRIMARY, true), None);
        assert_eq!(
            whole - per_span,
            split - 2 * per_span,
            "per-character cost is flat"
        );
        assert_eq!(
            split - whole,
            per_span,
            "and the only difference is one match"
        );
    }

    /// The `uk` exemption's four conditions, as an arithmetic contract.
    ///
    /// Each one is a measured refusal rather than a taste: without the POS
    /// half of `WordFlags::UK` the engine lost 16 gold spans hints-on, every
    /// one a particle swallowed (`様に`, `様だ`, `其れに` are all `uk`);
    /// without `UK_MIN_LEN` it merged `冬|だ|し` into 出汁 333 times over ten
    /// corpora; `exact` and an empty chain are what keep `ももう`→揉む and
    /// `のか`→退く paying, since those entries are `uk` too and reached
    /// through a deinflection chain.
    #[test]
    fn only_an_uninflected_uk_spelling_of_word_length_crosses_free() {
        let b = At(&[0, 2, 5]);
        let pronounce = WordFlags(WordFlags::PRONOUNCE.0 | WordFlags::UK.0);
        // `おねだり`-shaped: the kana spelling of a kanji headword, crossing
        // the one boundary IPADIC puts inside it.
        let free = cand(0, 5, pronounce, true);
        let charged = cand(0, 5, WordFlags::PRONOUNCE, true);
        assert_eq!(
            cost(&charged, Some(&b)) - cost(&free, Some(&b)),
            PRONOUNCE_CROSS,
            "a uk spelling crosses free; the same span without the bit pays"
        );

        let short = cand(0, 2, pronounce, true);
        let mut short_charged = short.clone();
        short_charged.flags = WordFlags::PRONOUNCE;
        let b2 = At(&[0, 1, 2]);
        assert_eq!(
            cost(&short, Some(&b2)),
            cost(&short_charged, Some(&b2)),
            "under UK_MIN_LEN the bit buys nothing -- `だし` keeps paying"
        );

        let mut inflected = free.clone();
        inflected.steps = vec![ConjStep {
            class: 0,
            form: 0,
            range: 3..5,
            aux: None,
        }];
        let mut inflected_charged = charged.clone();
        inflected_charged.steps = inflected.steps.clone();
        assert_eq!(
            cost(&inflected, Some(&b)),
            cost(&inflected_charged, Some(&b)),
            "a deinflected uk match pays as before"
        );

        let mut stacked = free.clone();
        stacked.grammatical_stack = true;
        let mut stacked_charged = charged.clone();
        stacked_charged.grammatical_stack = true;
        assert_eq!(
            cost(&stacked, Some(&b)),
            cost(&stacked_charged, Some(&b)),
            "a grammatical stack pays as before, whatever JMdict tags it"
        );

        let mut folded = free.clone();
        folded.exact = false;
        let mut folded_charged = charged.clone();
        folded_charged.exact = false;
        assert_eq!(
            cost(&folded, Some(&b)),
            cost(&folded_charged, Some(&b)),
            "a kana-folded match is not the spelling JMdict called normal"
        );
    }

    #[test]
    fn crossing_a_boundary_is_allowed_and_never_a_discount() {
        // Renamed from `..._costs_something_but_is_allowed`: at Task 16b's
        // measured CROSS=0, a PRIMARY match's crossing is cost-*neutral*, not
        // cost-*positive* -- `CROSS` no longer strictly separates the two.
        // Trial 2 (CROSS=4 -> 0) is why: the gold corpus's real compounds
        // overwhelmingly tie their split alternative on COVER+COMMON alone,
        // so any positive CROSS priced them out categorically (trial 11,
        // CROSS=1, collapsed hints-on tolerant from 0.8521 to 0.8049 and
        // merges from 53 to 19) rather than merely taxing them. What must
        // still hold -- and is asserted below -- is that a crossing is never
        // *rewarded*: `is_allowed` still permits it, and `cost` never prices
        // it below the no-crossing case. A PRONOUNCE-flagged crossing paying
        // strictly more is `a_pronounce_match_that_crosses_is_priced_far_above_a_primary_one`'s
        // job, not this test's.
        let b = At(&[0, 4, 5, 7]);
        let c = cand(4, 3, WordFlags::PRIMARY, true);
        assert!(
            is_allowed(&c, Some(&b)),
            "merging across a boundary is legal"
        );
        // Both sides carry bounds. They used to be `Some(&b)` against `None`,
        // which no longer isolates crossing from anything else: `LEN_GAIN`
        // applies only where a tokenizer exists, so a with-bounds cost is
        // legitimately below its without-bounds self by the length gain, for
        // a span that crosses nothing at all. The invariant this test is for
        // -- a crossing is never *rewarded* -- is the comparison below: the
        // same candidate under bounds that cut it, against bounds that do
        // not.
        let uncut = At(&[0, 4, 7]);
        assert!(
            cost(&c, Some(&b)) >= cost(&c, Some(&uncut)),
            "never a discount for crossing"
        );
    }

    #[test]
    fn a_pronounce_match_that_crosses_is_priced_far_above_a_primary_one() {
        // はいた as 履く against a real は + いた.
        let b = At(&[0, 1, 3]);
        let reading = cost(&cand(0, 3, WordFlags::PRONOUNCE, true), Some(&b));
        let kanji = cost(&cand(0, 3, WordFlags::PRIMARY, true), Some(&b));
        assert!(reading > kanji + 20, "reading {reading} kanji {kanji}");
    }

    #[test]
    fn a_kana_only_word_is_not_caught_by_the_pronounce_term() {
        // は, こう and もう are PRIMARY by that flag's own definition — the
        // hiragana of a kana-only entry — so the PRONOUNCE term must not fire
        // on them. If it ever did, the term would penalise ordinary particles
        // and make the engine worse rather than better.
        let b = At(&[0, 1, 3]);
        let kana_only = cand(0, 3, WordFlags::PRIMARY, true);
        let crossing = cost(&kana_only, Some(&b));
        // Against the same span under bounds that do not cut it, not against
        // the no-bounds case: `LEN_GAIN` is bounds-gated and would otherwise
        // show up in this difference as if it were a crossing price.
        let not_crossing = cost(&kana_only, Some(&At(&[0, 3])));
        assert_eq!(
            crossing - not_crossing,
            CROSS,
            "a PRIMARY span pays only the plain crossing price"
        );

        let reading = cand(0, 3, WordFlags::PRONOUNCE, true);
        assert_eq!(
            cost(&reading, Some(&b)) - crossing,
            PRONOUNCE_CROSS,
            "and the PRONOUNCE variant of the same span pays exactly the extra"
        );
    }

    #[test]
    fn pronounce_cross_scales_with_each_boundary_crossed() {
        // Every other test in this file uses a fixture with exactly one
        // interior boundary, so none of them can tell per-crossing scaling
        // apart from a flat one-time surcharge. This pins the current
        // choice explicitly: PRONOUNCE_CROSS is charged once per boundary
        // crossed, same as CROSS itself.
        //
        // The defect class it targets (Handoff §9.5) is entirely
        // single-crossing, so per-crossing scaling does no known harm today.
        // But it does not come free: at two crossings the term alone adds
        // 2 * PRONOUNCE_CROSS = 50, more than UNMATCHED charges for two whole
        // fallback characters, so a legitimate multi-boundary reading-form
        // merge could in principle lose to an unmatched run rather than
        // merely cost more. Not yet seen happening against the corpus, and
        // no task currently owns revisiting it -- see handoff §8's "three
        // size/shape assumptions" bullet.
        let b = At(&[0, 1, 3, 5]);
        let primary = cost(&cand(0, 5, WordFlags::PRIMARY, true), Some(&b));
        let pronounce = cost(&cand(0, 5, WordFlags::PRONOUNCE, true), Some(&b));
        assert_eq!(
            pronounce - primary,
            2 * PRONOUNCE_CROSS,
            "two boundaries crossed, so the PRONOUNCE surcharge applies twice"
        );
    }

    #[test]
    fn a_span_starting_or_ending_inside_a_token_is_forbidden() {
        let b = At(&[0, 2, 3]);
        assert!(!is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), Some(&b)));
        assert!(!is_allowed(&cand(0, 1, WordFlags::PRIMARY, true), Some(&b)));
        assert!(is_allowed(&cand(0, 2, WordFlags::PRIMARY, true), Some(&b)));
    }

    #[test]
    fn every_constraint_lifts_when_there_is_no_tokenizer() {
        assert!(is_allowed(&cand(1, 2, WordFlags::PRIMARY, true), None));
    }

    /// `雨でした`: the copula's own chain exists only where a tokenizer ran
    /// and kept でし one token -- IPADIC's call between the copula and
    /// `で` + する, which nothing in the dictionary can make.
    #[test]
    fn the_copula_chain_needs_the_tokenizer_to_keep_deshi_whole() {
        use crate::engine::table::CLASSES;
        let class = |name: &str| {
            CLASSES.iter().position(|c| c.name == name).expect("class") as crate::engine::ClassId
        };
        let form = |c: crate::engine::ClassId, name: &str| {
            CLASSES[c as usize]
                .forms
                .iter()
                .position(|f| f.name == name)
                .expect("form") as crate::engine::FormId
        };
        let (desu, ta) = (class("特殊・デス"), class("特殊・タ"));
        let mut copula = cand(1, 3, WordFlags::PRIMARY, true);
        copula.steps = vec![
            ConjStep {
                class: desu,
                form: form(desu, "連用形"),
                range: 2..3,
                aux: None,
            },
            ConjStep {
                class: ta,
                form: form(ta, "基本形"),
                range: 3..4,
                aux: Some("た"),
            },
        ];
        assert!(
            is_allowed(&copula, Some(&At(&[0, 1, 3, 4]))),
            "雨|でし|た: IPADIC kept でし whole, the copula"
        );
        assert!(
            !is_allowed(&copula, Some(&At(&[0, 1, 2, 3, 4]))),
            "雨|で|し|た: the particle and する"
        );
        assert!(!is_allowed(&copula, None), "no tokenizer, no copula chain");
        assert!(
            is_allowed(&cand(1, 3, WordFlags::PRIMARY, true), None),
            "every other candidate still lifts without one"
        );
    }

    #[test]
    fn an_unmatched_character_costs_far_more_than_a_matched_one() {
        let chars: Vec<char> = "」、".chars().collect();
        assert!(
            unmatched_cost(&chars, 0, 2) > cost(&cand(0, 2, WordFlags::PRIMARY, true), None) * 10
        );
    }

    /// Design §7.2/`SCRIPT_TRANSITION`'s own worked example: a genuinely
    /// mixed-script run (kanji/CJK-radical followed by roman letters, the
    /// OCR-noise shape Phase 0 handoff §9.1 names -- not §7.2, which carries only
    /// the cost formula) prices splitting at the script change
    /// strictly cheaper than keeping it whole, by exactly `SCRIPT_TRANSITION`
    /// -- the one script pairing `classify` actually distinguishes, unlike
    /// the punctuation-only example the doc comment above no longer cites.
    #[test]
    fn an_unmatched_run_prefers_splitting_at_its_script_change() {
        let chars: Vec<char> = "丨FIFA".chars().collect();
        let whole = unmatched_cost(&chars, 0, chars.len());
        let split = unmatched_cost(&chars, 0, 1) + unmatched_cost(&chars, 1, chars.len() - 1);
        assert_eq!(
            whole - split,
            SCRIPT_TRANSITION,
            "the only difference between keeping `丨FIFA` whole and splitting \
             it at the kanji/roman boundary must be the one script-transition \
             surcharge the whole run pays and the split does not"
        );
    }
}
