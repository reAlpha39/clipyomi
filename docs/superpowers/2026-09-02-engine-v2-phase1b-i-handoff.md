# Engine v2 Phase 1b-i — Handoff

**Date:** 2026-09-02
**Design:** [`specs/2026-09-02-engine-v2-phase1b-i-design.md`](specs/2026-09-02-engine-v2-phase1b-i-design.md)
**Plan:** [`plans/2026-09-02-engine-v2-phase1b-i.md`](plans/2026-09-02-engine-v2-phase1b-i.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §7 Phase 1b, §7.3
**Status:** Closed. Design §8's four done-when conditions are met (§2 below).

## 1. What exists now

`Entry::conjugation` is now filled on the v2 path. `engine/label.rs` reads
each candidate's `Entry::steps` (Phase 1a's `ConjStep` chain) and renders it
into a learner-standard English string — Causative, Passive/Potential,
Negative, Volitional, Polite, Want-to, Past, Te-form, Conditional,
Imperative, Adverbial — using three lookup tables (`AUX_WORD`, `FORM_WORD`,
`OVERRIDE`) and the four composition rules of design §3. `ConjStep` gained
one field, `aux: Option<&'static str>`, carrying the `ATTACHES_TO` lemma for
every step but the first, because `(class, form)` alone cannot distinguish
せる from られる (both 一段) or ん from て (both 不変化型) — design §2. The
gate is `crates/jparser/tests/fixtures/conjugation_labels.txt` and
`crates/jparser/tests/conjugation_labels.rs`, backed by a coverage test in
`engine/table.rs`. The v2 sentence snapshot is regenerated; the inverted unit
test `the_v2_path_labels_a_conjugated_entry_from_its_steps` in
`tests/engine_v2.rs` pins the wiring itself.

Files touched, with their actual size against design §5's table:

| File | Change | Lines |
| --- | --- | ---: |
| `engine/label.rs` | new — the three tables, `fmt`, unit tests | 262 |
| `tests/conjugation_labels.rs` | new — the gate test | 139 |
| `engine/table.rs` | `Continuation.lemma`, `search` writes it, coverage test | +207 |
| `engine/assemble.rs` | `conjugation: label::fmt(&steps)` in `build_entry`, module doc | small |
| `engine/mod.rs` | `pub mod label;`, `ConjStep.aux` | small |
| `engine/chain.rs` | one contract bullet | small |
| `tests/engine_v2.rs` | one test inverted | small |
| `tests/fixtures/conjugation_labels.txt` | new — 221 rows | 280 |

Design §5 estimated "about 500 lines" for 1b-i's code; the measured total is
nearer 660 (`label.rs` + `conjugation_labels.rs` + `table.rs`'s addition
alone sum to 608, before the small edits and the 280-line fixture). Stated
plainly rather than repeated as the old estimate: the excess is the coverage
test's walk and the three extra fixture rows the review added to pin dedup
and step-order on chains other than ます+です, both of which earned their
place rather than being scope creep. Measured against the plan's base commit
(`78a187b`) through the commit that finished the code (`4db86bd`) — the
*code range*, not the whole phase — this is **9 files changed, 956
insertions(+), 55 deletions(-)**. The whole phase, through this handoff's own
closing commits, is larger: **13 files changed, 1,343 insertions(+), 66
deletions(-)**. The 9-file code-range figure omits `lib.rs`'s doc-comment
change, which is a source file, not only the two closing docs commits this
handoff itself is part of — a handoff cannot count its own commit, which is
why the two figures are reported separately rather than folded into one.

Both feature states of the standing test suite pass at the counts every task
in this phase reported and this handoff reconfirmed independently at
`4db86bd` (the commit that finished Task 5, immediately before this task's
two doc-comment corrections — see §3 of the task 6 report for the
unchanged re-run after those edits): `cargo test -p jparser` is **433
passed, 0 failed, 0 ignored**; `cargo test -p jparser --features mecab` is
**468 passed, 0 failed, 1 ignored** (`the_particle_absorbing_merges_are_gone`,
Phase 1a's open item, untouched by this phase). Both clippy gates and
`cargo check -p jparser --no-default-features` are clean. §3 below covers
what the fixture confirmed that the design only proposed; §4 is the set of
findings the grid exposed; §5 is what Phase 1b-ii inherits.

## 2. Design §8's done-when, one condition at a time

**§8.1 — met.** All 221 rows of `conjugation_labels.txt` pass through the
real trie and label equal. The mechanical coverage walk in `engine/table.rs`
reaches **21,760** chains to depth 2 (170 at depth 0, 1,860 at depth 1,
19,730 at depth 2) and confirms every depth-0 `(is_adjective, form)` tuple
and every depth-1 `(aux lemma, aux form)` tuple that walk reaches already has
a fixture row — **zero rows needed to be added** by the coverage test. Before
`label.rs` existed, `every_fixture_row_labels_as_expected` failed on exactly
**183** rows against the always-`None` stub — precisely the fixture's
non-`-` rows — which is the gate proving it can fail before it is trusted to
pass.

**§8.2 — met in substance; the design's own example sentence does not work,
for reasons outside this phase.** Running `jparser-cli parse --engine v2`
against a real word does exercise the label end to end: `書いた` returns
`書く (Past)`, so `Entry::conjugation` plainly reaches the user-visible CLI
output, and the v2 sentence snapshot was regenerated and reviewed line by
line (see below). But the design's chosen example,
食べさせられませんでした, was verified — by running the release CLI
directly against the curated `jmdict_subset.xml` — not to decompose as one
chain at all: the segmenter splits it into two spans, `食べ` matched alone
and `させられませんでした` matched separately, so no single entry ever
carries the full label the design predicted. And the second span's own tail
renders `…Negative Want-to` rather than `…Negative Past`, which is finding 5
below (the た/たい collision), not a formatter defect. Both causes are
outside this phase's files — the split is the DP's (`cost.rs`/`lattice.rs`,
Phase 1a's metric-gated territory) and the Want-to mislabelling is
`lex::dedup_shallowest`'s equal-depth tie (finding 5 below), not
`assemble::order`'s ranking. `label::fmt` itself renders that exact chain
correctly: the fixture carries the row for it (`書かせられた` at depth 3,
`Causative Passive/Potential Past`, and `食べました` at depth 2, `Polite
Past`) and both pass. This is recorded as a correction to the design's
example, not as an open defect in this phase's own deliverable — see finding
8 in §4 for the full detail, including the documentation defect (finding 9)
about what `jparser-cli parse` actually emits.

The snapshot regeneration itself is clean: **36** `(-)` slots gained a label
across the 30 hand-written sentences, and every one of those 36 changed
line-pairs is, byte for byte, `(-)` → `(Label)` with the headword, reading
and gloss identical on both sides — no `start=`, `len=`, `matched`,
`reading=`, or sentence-header line changed anywhere in the diff. Six of the
36 read wrong on their sentence when checked against what a fluent reader
would expect; three are the known た/たい collision (finding 5) and three are
correctly-derived labels on implausible sibling parses (finding 6) — both
are recorded in §4, and neither is a defect in `label.rs`.

**§8.3 — met.** The five standing gates, run at `4db86bd`: `cargo test -p
jparser` (433/0/0), `cargo test -p jparser --features mecab` (468/0/1
ignored, the pre-existing Phase 1a item), `cargo clippy -p jparser
--all-targets -- -D warnings` (clean), the same with `--features mecab`
(clean), `cargo check -p jparser --no-default-features` (clean). Re-run
after this task's own doc-comment fix (§3 of the task 6 report) at the exact
same counts.

**§8.4 — met, and provable rather than merely observed.** `jparser-cli score
--engine v2` reproduces every one of Phase 1a's eight measured cells exactly:

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573/2163) | 0.7975 (1725/2163) | 0.9075 (1108/1221) | 75 |
| hints on | 0.8031 (1737/2163) | 0.8521 (1843/2163) | 0.9063 (1170/1291) | 53 |

These are byte-identical to the Phase 1a handoff §2 table's "this handoff"
rows in both hint states, on every field. The review that closed this task
established *why* this equality is guaranteed rather than coincidental:
`crates/jparser/src/bin/jparser-cli/score.rs`'s `accumulate` — read in full
for this handoff — touches only `gold.spans`, and from the parsed result
only `segments[].start`, `.len`, `.matched`, and
`seg.entries.first().entry_id`. It never reads `conjugation`, `steps`,
`reading`, `pos`, `senses`, or `flags` anywhere in the function. `build_entry`
in `assemble.rs`'s only behavioural change this phase is one added pure call,
`label::fmt(&steps)`, over the same `steps` value the module already computed
before this task touched it. A field the scorer never reads cannot move a
score no matter what fills it, so exact equality across all eight cells is a
structural consequence of the diff's shape — the measurement corroborates
that reasoning rather than standing in for it.

## 3. What the fixture decided that design §3 did not say in advance

The fixture's English column is not evidence of an independent check on the
formatter: `AUX_WORD`, `FORM_WORD`, `OVERRIDE` and the whole body of `fmt`
are printed verbatim in the plan (`docs/superpowers/plans/
2026-09-02-engine-v2-phase1b-i.md`), as is the entire fixture, and both were
committed together in `78a187b` by one author, both derived from design §3.
They share an ancestor, so their agreement cannot be evidence that the
vocabulary or the rules are correct. What the phase actually earned is four
narrower, verified claims:

1. **No fixture row's `expected` value was ever edited toward observed
   output.** The fixture is touched by exactly two commits, `aead911` and
   `c37e971`, both before the formatter commit `8cfe4c0`; `c37e971` added
   three rows and corrected comments only.
2. **Design §3's rules were complete over the whole reachable input space.**
   221 hand-applied rows and 21,760 walked chains produced **zero**
   additions to the three lookup tables. A rule set retrofitted to a
   fixture always grows; this one did not.
3. **The transcription from design to code is faithful** — verified
   byte-for-byte against the plan by review.
4. **The gate could fail before it was trusted to pass**: 183 rows red
   against the `None`-returning stub, exactly the fixture's non-`-` count.

The honest caveat the fixture header already half-admits: for roughly thirty
rows describing surfaces that are not natural Japanese — 読んな, 読んなれ,
食べよよう, 食べるじゃ, and 食べた read as `たい:ガル接続` — the `expected`
column is the rule's output by construction, so those rows verify
*consistency*, not correctness. The gate's rule-grading power sits in the
naturally-occurring surfaces (書いた, 食べさせられませんでした, 高く,
食べなかった, ませんでした and the rest), and there the review checked the
English on every row and found none wrong.

Two corrections were nonetheless made *before* the formatter was written,
found by enumerating the grid rather than by any fixture row failing:

- **連用デ接続 contributes no word.** 連用デ接続's surface is ない itself —
  the で the form is named for is absent from what actually attaches — so an
  earlier draft of design §3.2's table that assigned it "Te-form" was wrong
  and was corrected to *(none)* before implementation.
- **`(だ, 仮定形)` needs the same override `(た, 仮定形)` already has.**
  Without it, 読んだら would render "Past Conditional" instead of
  "Conditional" — the same double-counting rule 2 of design §3.3 exists to
  prevent for た, applying identically to its voiced counterpart だ.

## 4. Table findings — span shape, not fixed here

These are span-shape and ranking questions the grid exposed. Every one is
outside this phase's scope: changing any of them alters candidate generation
or the DP's ranking, and both are Phase 1a's metric-gated territory. Nine
are recorded below, with the surfaces that expose each.

1. **No `ATTACHES_TO` row requires 連用テ接続.** て's rows require 連用形 or
   連用タ接続, and ない requires 未然形, so 高くて, 食べなくて, 高くない and
   食べたくない never decompose as one chain.
2. **う attaches only to 未然ウ接続.** た, だ and です carry their
   presumptive stems as 未然形 (たろ, だろ, でしょ), so だろう, たろう and
   でしょう never decompose. よう, which also requires 未然ウ接続, only ever
   produces the spurious 食べよよう — the natural 食べよう is reached through
   う instead.
3. **The voiced past だ row names the copula class.** `("だ", "特殊・ダ",
   "連用タ接続")` gives だ the copula's forms (なら, な, なれ, だ, だろ, だっ,
   で) rather than the 特殊・タ shape た has (たら, た, たろ). So 読んだら is
   unreachable, and 読んなら, 読んな, 読んなれ and 読んだっ are generated.
   読んだ, 読んだろ and 読んで are real.
4. **せる/させる and れる/られる are not restricted by the inner class.**
   Every 未然形 accepts all four, so 食べせる and 書かられる are generated
   beside the natural 食べさせる and 書かれる.
5. **The most consequential finding — the た/たい collision.** 特殊・タ has
   `remove: ""` with 基本形 `suffix: ""`, so `Table::new` gives it stem `た`
   and 基本形 surface `た`. 特殊・タイ has `remove: "たい"` with ガル接続
   `suffix: "た"`, so `lemma.strip_suffix(class.remove)` leaves an **empty**
   stem and its ガル接続 surface is *also* a bare `た`. Both are real
   Continuation hits at the same trie node, both accept a preceding 連用形,
   so **every polite past also carries a Want-to reading over the identical
   span**. Which one the reader sees is decided by `lex::dedup_shallowest`
   (`engine/lex.rs:170`), not by `assemble::order` and not by the formatter.
   For 読みました the two competing chains — `…ます:連用形 た:基本形` and
   `…ます:連用形 たい:ガル接続` — have the same span, the same `entry_id`,
   and the same `steps.len()` (3; 4 for 書かせられた), so
   `dedup_shallowest`'s key `(cand.len, cand.entry_id)` ties and the existing
   (first-generated) candidate is kept, deleting the other before
   `assemble::order` is ever called. Every one of `order`'s own keys —
   `(!exact, !COMMON, PRONOUNCE, steps.len(), entry_id)` — ties for these two
   as well, so `order` neither can nor does make this choice. The real
   decision is `Table::search`'s hit order at the shared `た` trie node,
   which follows `ATTACHES_TO`'s row order: `("たい", …)` at `table.rs:268`
   precedes the two `("た", …)` rows at `table.rs:269-270`, which is why
   Want-to wins. Three instances appear in the regenerated snapshot: 読みました
   and しました render `Polite Want-to` where `Polite Past` is right, and
   書かせられた renders `Causative Passive/Potential Want-to` where the
   fixture's own row for that exact chain says `Causative
   Passive/Potential Past`. The fixture deliberately pins **both** sides of
   this collision (as separate `chain` rows), so this is documented,
   understood behaviour rather than an unknown.

   Two fixes exist, at different costs, and only one of them needs the metric
   gate. The **cheap fix** — reordering `ATTACHES_TO` so the `たい` row
   follows the two `た` rows — changes only which of two equal-depth chains
   survives `dedup_shallowest`; the survivor keeps the same `len`, the same
   `entry_id` and the same span, so `jparser-cli score`'s `accumulate`
   (§8.4 above: it reads only `gold.spans` plus `segments[].start`/`.len`/
   `.matched` and `entries.first().entry_id`) cannot observe the difference —
   the same neutrality argument §8.4 already makes. That neutrality is
   derived from the code here and should be confirmed by measurement the way
   §8.4 was, not assumed; the cost is a one-row reorder plus a snapshot
   regeneration, with the metric run as *confirmation* rather than as a
   gate. The **principled fix** — making `たい:ガル接続` non-terminal, since
   no `ATTACHES_TO` row actually requires that form — deletes a candidate
   outright and can move the DP's span choice elsewhere, so it is the one
   that is genuinely metric-gated, like the rest of this section.

   The same equal-depth dedup tie decides more than this one label. Every
   godan 仮定形/命令ｅ pair collides identically: 書け is both
   `五段・カ行イ音便:仮定形` and `五段・カ行イ音便:命令ｅ` — same surface,
   same entry, same `steps.len()` of 1 — so `Conditional` versus `Imperative`
   is settled by row order too; 読め (五段・マ行) and 話せ (五段・サ行) are
   the same shape. The fixture pins both readings as separate rows (書け at
   both); the 30-sentence snapshot corpus happens not to exercise them.

   This is deferrable only because v2 is not yet the default: `parse_v2` has
   exactly two callers in the tree, `bin/jparser-cli/main.rs` and
   `bin/jparser-cli/score.rs`; `src-tauri` never calls it and the frontend
   never sees it, so no end user sees a wrong label today. Were v2 becoming
   the default in this commit, "Polite Want-to" on ました — the most common
   polite form in the language — would be a blocker, not a deferral. Phase
   1b-ii owns the switchover and must settle this before flipping the
   default.

6. **A distinct, larger class: correct labels on implausible parses.** The
   formatter names whatever chain it is handed, so a label can be perfectly
   correct while describing a parse no reader would accept — this is a
   segmentation/candidate-ranking question, not a formatter question. The
   snapshot has at least twelve instances, itemised below: これ in
   これは面白い映画だ carries three low-ranked sibling candidates that parse
   it as a conjugated verb stem (こる twice as `Conditional`, くる once as
   `Passive/Potential`); `いる (Polite)` appears glossed "to shoot", "to
   cast" and "to calm down" in 電車が遅れています where only "to be (of
   animate objects)" is plausible; 遊む and 遊ぶ both render `Te-form`
   glossed "to grow wild" in 子供が公園で遊んでいる; `なる (Want-to Polite)`
   glossed "to sound" and "to bear fruit" and `なり (Want-to Polite)`
   glossed "to be" appear in 先生になりたいです where only "to become" fits;
   `しまる (Past)` appears twice in パソコンが壊れてしまった. **These are not
   formatter defects and must not be described as such** — they are
   `assemble::order`'s ranking and the DP's candidate set, recorded because
   they are what a reader actually sees and because Phase 1a's own handoff
   already flags `order`'s signal-free final tiebreak as load-bearing on
   display quality.
7. **An accidental equivalence that will stop holding.** Design §3.2
   specifies that one word comes from the **outermost** step's form. No
   chain the table can generate today distinguishes that rule from "any
   step's form", because every 活用形 an auxiliary attaches to
   (`ATTACHES_TO`'s `requires` column: 未然形, 未然レル接続, 未然ウ接続,
   連用形, 連用タ接続, 基本形) contributes no word at all under design §3.2's
   table. The equivalence is accidental, and finding 1 above is exactly what
   ends it: adding a て row requiring 連用テ接続 would let `ない:連用テ接続`
   sit as an *inner* step, at which point "outermost only" gives `Negative
   Te-form` while "any step" would give `Negative Adverbial Te-form`.
   `label.rs` implements outermost-only, and its unit test
   `only_the_outermost_steps_form_contributes_a_word` pins the decision with
   hand-built steps before it becomes reachable through the table. Whoever
   takes finding 1 must know this before adding that row.
8. **The design's own §8.2 example does not work, for two independent
   reasons.** Verified by running the release CLI directly.
   `parse --engine v2 書いた` gives `書く (Past)`, so the label plainly
   reaches the user-visible output. But `parse --engine v2
   食べさせられませんでした` yields two spans — `食べ` matched alone, then
   `させられませんでした` — so no chain ever spans the whole word against the
   curated `jmdict_subset.xml`; and the remaining span's tail renders
   `…Polite Negative Want-to` via finding 5. Neither cause is the
   formatter's: the split is the DP's (`cost.rs`/`lattice.rs`, Phase 1a's
   metric-gated territory) and the Want-to mislabelling is finding 5's
   `lex::dedup_shallowest` equal-depth tie, not a ranking question. The
   formatter renders that exact chain correctly —
   `conjugation_labels.txt` carries the row and it passes. Recorded here as a
   correction to the design, not as an open defect.
9. **A documentation defect worth recording.** The plan asserted
   `jparser-cli parse` emits JSON. It does not — only `score` has `--json`;
   `parse` has a frozen human-text format (pinned by `tests/cli_parse.rs`'s
   doc comment), which does display the label in parentheses. The question
   the step existed to answer — does the new `ConjStep.aux` field
   serialize — was confirmed separately and positively: `Entry`, `Segment`
   and `ParseResult` all derive `serde::Serialize`, and a throwaway
   integration test built the fixture index, called `parse_v2` directly, and
   printed `serde_json::to_string_pretty` over the result, confirming both
   `conjugation` and `steps[].aux` round-trip exactly as the design's
   interfaces section describes.

## 5. What Phase 1b-ii inherits

The switchover itself, carried verbatim from master plan §7 Phase 1b and
Phase 1a's handoff §9: the nine hard-taint files deleted, `assets/
conjugations.json` deleted, `xtask`'s ta-old converter deleted, every
`ta-old` citation and every `ported from Translation Aggregator` header gone
tree-wide (`crates/jparser/`, `xtask/`, `crates/jmdict-source`,
`crates/vibrato-source`, `src-tauri/` including `build.rs`, and `tools/`
included, checked by grepping both strings over the whole repository), the
SPDX flip of master plan §9.2, and the provenance headers of §9's open
question 2 written on the replacement files. The concrete relocation
problems Phase 1a's handoff §9 names still stand and are still unresolved:
where `WordFlags` and `StemStats` move to, with `WordFlags`'s eight wire
names and its persisted bit values preserved byte-for-byte (`engine/
build.rs` writes `flags: head.flags.0` straight into the index, so getting a
bit value wrong silently changes either an index's fingerprint or its flag
semantics), and what `fingerprint_differs_from_the_v1_conjugation_asset`
asserts against once `conjugation.rs`'s fingerprint function is deleted with
it. None of this changed in Phase 1b-i — v1 still owns all three files —
but it is real work the switchover commit has to land atomically alongside
the deletions, not discover partway through.

One item of this phase's own to add to that inheritance: **the crate is not
`cargo fmt` clean.** Checked directly (`cargo fmt -p jparser --check`) both
at this phase's own plan base (`78a187b`) and at its head (`4db86bd`): the
crate carries **39** formatting sites at head, of which **32** are
unchanged from before this phase started — spread across
`src/bin/jparser-cli/{corpus,diff,label,main,score}.rs`, `src/conjugation.rs`
(8 of them), `src/kana.rs`, `src/romaji.rs` (3), `src/engine/{assemble,
cost,kana,lattice}.rs` (lattice.rs alone carries 5), and `tests/
engine_v2.rs` (5) — and **7** were introduced during this phase itself: 4 in
`engine/table.rs`'s new coverage-test code, 2 in the new `tests/
conjugation_labels.rs` (both transcribed verbatim from the task brief per
that task's explicit instruction to transcribe exactly rather than
reformat), and 1 in `tests/engine_v2.rs` where the single renamed test
shifted a pre-existing wrap point. `conjugation.rs` is among the pre-existing
sites and this phase's clean-room rule forbade opening it, so a repo-wide
`cargo fmt` was unavailable in principle for the whole of Phase 1b-i, not
merely undone for lack of time. Phase 1b-ii deletes or rewrites every file
carrying a pre-existing site as part of the relicence sweep in any case, so
it is the natural, cost-free place to run the formatter once and land all of
them together rather than carry the drift further.

Also worth recording for whoever plans the switchover: Phase 1a's metric
gate is still open on four of the eight cells in §2's table above — hints-off
tolerant and entry top-1, and hints-on tolerant and entry top-1 all remain
below the v1 baseline in the Phase 1a handoff §2 comparison, while both
strict cells pass. Nothing in Phase 1b-i changed that, by design: §8.4's own
proof is that a label cannot move a span or an `entry_id`, so this phase
could not have closed that gap even incidentally. Five of the surfaces
behind that gap now have a named cause (§4's `ATTACHES_TO` and collision
findings above); whoever plans 1b-ii should decide whether to take any of
those fixes, measured against the Phase 0 metrics, before or alongside
making v2 the default.

One more item for whoever lands the switchover: `EngineChoice`'s doc comment
in `crates/jparser/src/bin/jparser-cli/main.rs:55-57` reads *"v1 is the
default for all of Phase 1a: the switchover is Phase 1b's, so no build ships
with `conjugation: None`."* Its *reason*, not merely its phase number, is now
obsolete — v2 no longer emits `None` for a conjugated entry, so that
sentence's stated justification for keeping v1 the default no longer holds.
The real reason v1 is still the default appears nowhere in the code: Phase
1a's metric gate is open on four of the eight cells above. Phase 1b-ii
touches this file to flip the default in any case, so it should correct both
the phase number and the reason in the same edit. Recorded here, not fixed —
this handoff's own scope is 1b-i, and `main.rs` is 1b-ii's file to change.
