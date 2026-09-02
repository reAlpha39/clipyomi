# Engine v2 Phase 1b-i — Conjugation-label formatter and its gate

**Date:** 2026-09-02
**Master plan:** [`2026-08-27-engine-v2-master-plan.md`](2026-08-27-engine-v2-master-plan.md) §3.5, §7 Phase 1b, §7.3
**Phase 1a design:** [`2026-09-01-engine-v2-phase1a-design.md`](2026-09-01-engine-v2-phase1a-design.md) §2.1, §2.2, §8, §9
**Phase 1a handoff:** [`../2026-09-01-engine-v2-phase1a-handoff.md`](../2026-09-01-engine-v2-phase1a-handoff.md) §8, §9
**Branch:** `feat/engine-v2`, from `03334f0`
**Status:** Design, approved in session, not implemented

---

## 1. Scope

Master plan §7 describes Phase 1b as one sub-phase: the formatter, its gate,
and the switchover. This design splits it and covers the first half.

**Phase 1b-i — in.** The auxiliary-chain formatter,
`fmt(&[ConjStep]) -> Option<String>`, filling `Entry::conjugation` on the v2
path; its gate — a class × chain fixture whose English column is written by
hand from a named grammar reference before the formatter exists (Phase 1a
design §2.2); the one additive field the formatter needs on `ConjStep` (§2).

**Phase 1b-i — out.** The switchover: the nine hard-taint files, `assets/conjugations.json`,
`xtask`'s converter, every `ta-old` citation and `ported from Translation
Aggregator` header, the SPDX flip of master plan §9.2, the `WordFlags`/`StemStats`
relocation the Phase 1a handoff §9 lists. All of it moves to **Phase 1b-ii**
unchanged. Also out: k-best (Phase 2), any TypeScript change, any change to
the cost model, `ATTACHES_TO`, or candidate generation — the span shape is
Phase 1a's and is metric-gated (§7).

**Why split here.** Phase 1a's metric gate is open on four of eight cells
(handoff §2). Whether v2 becomes the default is a decision to take with the
formatter working and those numbers in hand, not one to bundle into the same
commit as 480 lines of ungated new code. The formatter is also master plan
§7.3's "single highest-risk item"; de-risking it first, behind the existing
`--engine v2` flag, costs nothing and leaves the switchover a smaller commit.

---

## 2. The input, and the one field it is missing

Phase 1a hands the formatter `Entry::steps: Vec<ConjStep>`, innermost first,
ranges absolute and tiling (`chain.rs`'s contract). Each step is
`{ class: ClassId, form: FormId, range }`.

That is not enough to name the chain. `table.rs`'s `ATTACHES_TO` generates
every auxiliary's surfaces from its *lemma*, but the step records only the
auxiliary's *class* — and classes are shared:

| class | lemmas sharing it |
| --- | --- |
| 一段 | せる, させる, れる, られる (and every ordinary ichidan headword) |
| 不変化型 | ん, う, よう, て, で |

"Causative" and "Passive/Potential" are the same `(class, form)`; so are
"Negative", "Volitional" and "Te-form". The alternative — recovering the lemma
from the step's surface minus its form suffix plus `class.remove` — works on
today's 18 lemmas but makes the formatter depend on the text, leaves `Entry`
unable to re-label itself from `steps` alone, and breaks silently the day two
`ATTACHES_TO` rows share a `(class, surface)`. Rejected in favour of recording
what the table already knows at the moment it knows it.

**The change.** `ConjStep` gains `pub aux: Option<&'static str>`: the
`ATTACHES_TO` lemma for every step after the first, `None` on the first
(the main word's own inflection, a `Terminal` hit). `TrieHit::Continuation`
gains `lemma: &'static str`, copied from the `ATTACHES_TO` row at
`Table::new`; `Table::search` writes it into the step it pushes. `chain.rs`'s
contract gains one bullet stating the invariant. The three hand-built
`ConjStep` literals in `assemble.rs`'s tests gain `aux: None`.

**What it does not touch.** `steps` is not persisted — `engine/build.rs`
writes no step data, so no index fingerprint moves. `Entry` is `Serialize`,
so the field appears in `jparser-cli parse --engine v2` JSON, where nothing
reads `steps` (`src/types.ts` declares `conjugation` only). The v2 snapshot
prints neither steps nor their fields.

---

## 3. Vocabulary and composition

The register is learner-standard — the vocabulary of Genki and Tae Kim, and
of master plan §7.3's own example *"Causative-Passive Polite Negative Past"*.
The source is the English Wikipedia article *Japanese verb conjugation*
(`https://en.wikipedia.org/wiki/Japanese_verb_conjugation`), which glosses
every IPADIC 活用形 name; only terminology is taken from it. The choices below
were made in session; where the hand-written fixture (§4) and this section
disagree, **the fixture is authoritative** and this section is corrected to
match — never the other way round.

### 3.1 One word per auxiliary lemma (steps after the first, in order)

| lemma | word | note |
| --- | --- | --- |
| せる, させる | Causative | |
| れる, られる | Passive/Potential | ichidan られる is passive, potential or honorific and the parser cannot tell; godan potentials (書ける) are not decomposed by the table at all, so the ambiguity is exactly this one class |
| ない, ぬ, ん | Negative | |
| う, よう | Volitional | covers presumptive だろう/でしょう too; learner texts file both under volitional |
| ます, です | Polite | |
| たい | Want-to | |
| た, だ | Past | だ here is the voiced た of 読んだ (`("だ", "特殊・ダ", "連用タ接続")`), not the copula |
| て, で | Te-form | |
| じゃ, や | Copula | `ATTACHES_TO`'s placeholder rows; `table.rs` documents that they never fire |

### 3.2 One word from the outermost step's form

Applies to whichever step is last — the main word's own form when there is no
auxiliary, the outermost auxiliary's form otherwise. "Adjective" means
`Class::is_adjective` or an adjective-conjugating auxiliary (ない, たい).

| form | word |
| --- | --- |
| 仮定形, 仮定縮約１, 仮定縮約２ | Conditional |
| 命令ｅ, 命令ｉ, 命令ｒｏ, 命令ｙｏ | Imperative |
| 連用形 (adjective only), 連用テ接続 | Adverbial |
| 連用デ接続 | Te-form (食べないで) |
| 基本形, 未然形, 未然ウ接続, 未然レル接続, 未然ヌ接続, 未然特殊, 連用形 (verb), 連用タ接続, 連用ゴザイ接続, 連用ニ接続, 体言接続, 体言接続特殊, 体言接続特殊２, 文語基本形, 音便基本形, 現代基本形, 基本形-促音便, ガル接続 | *(none)* — a plain form, or a dangling attachment stem |

### 3.3 Rules

1. **Order and dedup.** Auxiliary words in step order, then the outermost
   form's word. Each word appears at most once, first occurrence wins:
   食べませんでした is ます → Polite, ん → Negative, です → Polite (dropped),
   た → Past: *"Polite Negative Past"*.
2. **Overrides.** A table keyed `(lemma, outermost form)` replaces the pair
   of words rules 3.1 and 3.2 would emit. Two are known now: `(た, 仮定形)`
   食べたら → *"Conditional"*, not "Past Conditional"; `(だ, 連用形)` 読んで
   → *"Te-form"*, not "Past". The table grows only where a fixture row
   demands it.
3. **Empty is `None`.** Zero words → `None`, matching the v1 path's observable
   behaviour for an all-stem chain. So 書か, 書い, 食べ, 高かっ → `None`;
   食べませ → *"Polite"* — dangling, but a real candidate the DP can pick.
4. **One surface, several chains.** 読んで is reachable through で (不変化型,
   基本形) and through だ (特殊・ダ, 連用形); ませ through ます 未然形 and ます
   命令ｅ. The formatter labels each chain it is given. The fixture pins each
   chain separately (its `chain` column, §4.1); which one the user sees is
   `assemble::order`'s decision and is unchanged by this phase.

---

## 4. The gate

Phase 1a design §2.2's argument stands: conjugation is finite and generative,
so the gate is a grid, and the grid's English column must be written before
the code it grades. The one refinement is *how much* of the grid is written by
hand, decided by measurement rather than assumed.

**The grid's size, measured.** A throwaway script walked `CLASSES` and
`ATTACHES_TO` forward — from each of the 20 classes `JMDICT_POS_CLASS` maps,
through every form, through every auxiliary row requiring that form, through
every form of that auxiliary — and counted:

| auxiliaries deep | rows (all 20 classes) | distinct class-independent tuples |
| --- | ---: | ---: |
| 0 | 170 | 170 `(class, form)`; ~35 once collapsed to `(adjective?, form)` |
| 1 | 1,860 | 101 `(aux lemma, aux form)` |
| 2 | 19,730 | 1,028 |
| 3 | 226,766 | 11,806 |

The label is class-independent from depth 1 on (書いた and 食べた are both
*"Past"*), so the hand-written column covers **every** tuple at depth 0 and 1
and a **curated** set deeper. Depth 2 in full (1,028 rows) is past the point
where a reviewer reads each row rather than skims it, which would defeat the
review the gate exists for. Depth ≥ 2 is instead covered by a mechanical test
that asserts the fixture's completeness and the formatter's safety, without
asserting English it could only have derived compositionally (§4.3).

### 4.1 The fixture

`crates/jparser/tests/fixtures/conjugation_labels.txt`. Tab-separated, `#`
comments, blank lines skipped — the same shape and parser convention as
`spurious_merges.txt`. Columns:

```
surface ⇥ chain ⇥ expected
```

- `surface` — a complete conjugated word on a real headword, for the
  reviewer's benefit; `Table::chains_ending_at` does not consult the
  dictionary, so the headword's identity does not affect the chain.
- `chain` — space-separated steps, innermost first: `class:form` for the
  first step, `lemma:form` for each auxiliary.
- `expected` — the label, or `-` for `None`.

```
食べさせられませんでした	一段:未然形 させる:未然形 られる:未然形 ます:未然形 ん:基本形 です:連用形 た:基本形	Causative Passive/Potential Polite Negative Past
読んで	五段・マ行:連用タ接続 で:基本形	Te-form
読んで	五段・マ行:連用タ接続 だ:連用形	Te-form
書か	五段・カ行イ音便:未然形	-
```

Three parts, about 190 rows:

- **Part A — depth 0.** One row per `(adjective?, form)` tuple, ~35, on
  representative stems: 書く (五段), 行く (五段・カ行促音便ユク, the only
  class with 基本形-促音便), 食べる (一段), する (サ変), 来る (カ変), あり
  (ラ変, the only class with 現代基本形), 高い (形容詞・アウオ段), いい
  (形容詞・イイ). More rows are welcome; these are the floor the coverage
  test enforces.
- **Part B — depth 1.** One row per `(aux lemma, aux form)` tuple, 101, on
  whichever representative stem reaches that auxiliary's attachment form
  (食べ- for 未然形/連用形, 書い- for 連用タ接続, さ- for 未然レル接続, 書こ-
  for 未然ウ接続).
- **Part C — curated, depth 2–6.** About 50 rows chosen to exercise every
  §3.3 rule: each override, the Polite dedup, ましょう, なかった, たくない,
  なければ, なきゃ, たら, たろう, でしょう, ないで, されました (through
  未然レル接続), 高くなかった, よかった, both 読んで chains, both ませ chains,
  and the corpus's deepest known chain させ|られ|ませ|ん|でし|た.

**Authorship and order.** The English column is written from the Wikipedia
article, reading each surface, and reviewed by the author, **before**
`label.rs` exists. It is committed first; the gate test (§4.2) lands red
against it; the formatter is then written to pass it. A row the formatter
cannot pass without a rule §3 does not describe is a finding — either the row
is wrong (fix the row, record why) or §3 is incomplete (add the rule, record
it in the decision log). Neither is resolved by editing the row to match the
code.

### 4.2 The gate test

`crates/jparser/tests/conjugation_labels.rs`, an integration test needing no
feature flag. For each fixture row:

1. `Table::new().chains_ending_at(chars, chars.len(), chars.len())` over the
   surface — the budget is the whole surface, so the test does not depend on
   `lex.rs`'s private `MAX_CHAIN`.
2. Render every returned `ChainHit`'s steps in the `chain` column's notation
   (`CLASSES[class].name:forms[form].name` for the first step, `aux:form` for
   the rest).
3. Assert **exactly one** hit renders equal to the row's `chain`. Zero means
   the table no longer generates that chain — a stale row or a table
   regression, either way a failure with the row named. More than one should
   be impossible — two hits rendering identically would need one inner form
   name to satisfy two different `requires` strings — and is asserted
   anyway, so a future table change that makes it possible is caught here.
4. Assert `label::fmt(&hit.steps) == expected` (`-` ↔ `None`).

Running through the real trie rather than hand-building steps means the
fixture also proves, row by row, that the table generates every chain the
formatter claims to name.

### 4.3 The coverage test

In `table.rs`'s `#[cfg(test)]` module — it needs the private `ATTACHES_TO` —
reading the fixture through `include_str!`. It re-runs the forward walk of
this section's opening measurement, in Rust, to depth 2, and asserts:

- every depth-0 `(is_adjective, form name)` tuple reachable from a
  `JMDICT_POS_CLASS`-mapped class has a Part A row;
- every depth-1 `(aux lemma, aux form)` tuple has a Part B row;
- for every reachable chain to depth 2 (19,730), `label::fmt` over a
  synthesised `Vec<ConjStep>` (ranges irrelevant to `fmt`, set to `0..0`)
  does not panic, returns `None` or a non-empty string, and emits no word
  twice.

It asserts **no English** beyond Parts A–C. Asserting a depth-2 label equal
to the concatenation of its depth-1 parts would test the formatter against
its own composition rule — Phase 1a design §2.2's tautology, one level up.

### 4.4 The snapshot

`v2_snapshots_every_sentence` (`tests/snapshots/engine_v2__sentences.snap`)
prints `(-)` in every entry's conjugation slot today. It regenerates; the
diff is 30 hand-written sentences and is reviewed line by line, as the one
place the labels are seen on running text rather than on isolated surfaces.
`phase_1a_emits_no_conjugation_label_but_does_emit_the_steps`
(`tests/engine_v2.rs`) inverts: 書いた's first entry labels *"Past"*.

---

## 5. Wiring and files

| file | change | est. lines |
| --- | --- | ---: |
| `engine/label.rs` | **new.** `AUX_WORD`, `FORM_WORD`, `OVERRIDE` tables; `pub fn fmt(&[ConjStep]) -> Option<String>`; unit tests for each §3.3 rule. Pure, infallible, allocates only the result | ~250 |
| `engine/mod.rs` | `pub mod label;` `ConjStep.aux` | ~5 |
| `engine/table.rs` | `Continuation.lemma`; `search` copies it; coverage test | ~110 |
| `engine/chain.rs` | one contract bullet | ~3 |
| `engine/assemble.rs` | `conjugation: label::fmt(&steps)` in `build_entry`; `aux: None` on three test literals | ~5 |
| `tests/fixtures/conjugation_labels.txt` | **new.** ~190 rows | — |
| `tests/conjugation_labels.rs` | **new.** gate test | ~120 |
| `tests/engine_v2.rs`, snapshot | one test inverted; snapshot regenerated | ~5 |

About 500 lines against Phase 1a design §9's ~480 for 1b's code; the
difference is the coverage test, which §9 did not budget.

Untouched, by construction: every v1 file (`conjugation.rs`, `matcher.rs`,
`assets/conjugations.json` — not opened, §6), all TypeScript, the index
format, `cost.rs`, `lattice.rs`, `lex.rs`, `ATTACHES_TO`. `jparser-cli score
--engine v2` reads spans and `entry_id`, never `conjugation`, so both Phase 0
metrics are unchanged by construction; done-when §8.4 checks that they are.

---

## 6. Clean-room provenance

Master plan §3.5 binds this phase. `label.rs`'s header carries Phase 1a's
GPL-2.0 notice and clean-room line (the SPDX flip is 1b-ii's) plus:

> English vocabulary from *Japanese verb conjugation*, English Wikipedia,
> `https://en.wikipedia.org/wiki/Japanese_verb_conjugation`, read 2026-09.
> Terminology only. `assets/conjugations.json`, `conjugation.rs` and
> `matcher.rs`'s label renderer were not opened while this file or
> `tests/fixtures/conjugation_labels.txt` was written.

The fixture's header says the same of itself. The one deliberate divergence
from v1's observable output is recorded rather than hidden: v1 renders ます as
*"Formal"* (`lib.rs:101`'s doc example *"Negative Formal Past"*); this phase
renders *"Polite"*, master plan §7.3's own word.

---

## 7. Findings recorded, not fixed

The grid enumeration already shows table gaps that are **span-shape**
questions — Phase 1a's cost-model-gated territory, not this phase's:

- **`ATTACHES_TO` has no て row for 連用テ接続.** IPADIC's 連用テ接続 (高く,
  なく, たく) is literally the form て attaches to, but the table's て rows
  require 連用形 or 連用タ接続 only, so 高くて, 食べなくて, 行きたくて never
  decompose. Adding the row changes candidate generation and therefore span
  accuracy; it belongs to a 1a follow-up or 1b-ii, measured.
- **Dangling attachment forms are live candidates** (食べませ, 書こ, 読んだっ).
  The formatter gives each a deterministic label or `None` (§3.3 rule 3);
  whether they should exist as candidates at all is `cost.rs`/`lex.rs`'s
  question.
- **Any further gap the coverage walk surfaces** is written into the 1b-i
  handoff with the surfaces that expose it, and nothing else.

---

## 8. Done when

1. The gate test passes on every row of `conjugation_labels.txt`; the
   coverage test passes.
2. `jparser-cli parse --engine v2` labels 食べさせられませんでした
   *"Causative Passive/Potential Polite Negative Past"*; the v2 snapshot is
   regenerated and its diff reviewed.
3. The standing gates: `cargo test -p jparser` with and without
   `--features mecab`; `cargo clippy -p jparser --all-targets -- -D warnings`
   in both feature states; `cargo check -p jparser --no-default-features`.
4. `jparser-cli score --engine v2`, both hint states, reproduces the Phase 1a
   handoff §2 table byte for byte on every field but `index_generation`. The
   label cannot move a span or an `entry_id`; this proves the wiring didn't.

---

## 9. Master plan edits this design owes

- §7 Phase 1b: split into 1b-i (this) and 1b-ii (the switchover, with the
  current In/Out/Done-when carried verbatim). Phase 1's own done-when is
  unchanged and lands at the end of 1b-ii.
- §7.3: note that the label mapping's gate is this design's §4, and that the
  register and reference are decided (§3).
- §10 decision log: the rows below.

---

## 10. Decision log

| Decision | Rationale |
| --- | --- |
| Split Phase 1b into 1b-i (formatter + gate) and 1b-ii (switchover) | 1a's metric gate is open on four cells; the switchover decision wants the formatter working and those numbers in hand, not 480 ungated lines bundled into the relicence commit. §7.3 names the formatter the highest-risk item; de-risk it first behind the existing flag (§1) |
| `ConjStep.aux` records the auxiliary lemma | 一段 is shared by せる/させる/れる/られる and 不変化型 by ん/う/よう/て/で; class+form cannot name the auxiliary. Inferring the lemma from the surface works today and breaks silently later (§2) |
| Learner-standard register | The product is a learner tool and master plan §7.3's own example is in this register; only ~5 words differ from textbook-linguistic terms anyway (§3) |
| られる/れる → *"Passive/Potential"* | Ichidan られる is ambiguous and the parser cannot resolve it; a plain *"Passive"* is wrong for every potential reading. Godan potentials are not decomposed, so the noise is confined to the ambiguous class (§3.1) |
| No hyphenated fusions (させられ stays two words) | One word per step, no fusion rules beyond what the fixture forces; a hyphen buys nothing and adds a special case (§3.3) |
| Wikipedia *Japanese verb conjugation* as the named reference | Public, stable URL, glosses every IPADIC form name; terminology is not protected expression, and §3.5 needs a reviewer to be able to establish provenance without asking (§6) |
| Hand-written English to depth 1 in full, deeper rows curated | Measured: depth 2 is 1,028 distinct tuples, past the point a reviewer reads rather than skims. Depth ≥ 2 gets a mechanical completeness-and-safety test that asserts no English, avoiding §2.2's tautology (§4) |
| Gate test runs the real trie, not hand-built steps | Each row then also proves the table generates the chain it names; a stale row fails loudly instead of testing a chain that no longer exists (§4.2) |
| Fixture is authoritative over §3 | §3 is the intent; the fixture is the check. A row the code cannot pass is a finding to record, never a row to edit toward the code (§4.1) |
| 連用テ接続's missing て row is recorded, not fixed | Adding an `ATTACHES_TO` row changes span shape, which is 1a's metric-gated territory (§7) |
