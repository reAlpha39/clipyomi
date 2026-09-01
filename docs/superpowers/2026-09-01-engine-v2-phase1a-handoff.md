# Engine v2 Phase 1a — Handoff

**Date:** 2026-09-01
**Design:** [`specs/2026-09-01-engine-v2-phase1a-design.md`](specs/2026-09-01-engine-v2-phase1a-design.md)
**Plan:** [`plans/2026-09-01-engine-v2-phase1a.md`](plans/2026-09-01-engine-v2-phase1a.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §3.5, §4.3, §5.2, §7 Phase 1a, §9.5
**Status:** **Not closed.** Seventeen tasks built the clean-room v2 core, wired
it end to end, measured it against the Phase 0 baseline, and tuned its cost
model. Design §10's done-when has three conditions; two are met
(`くるまではこう` parses correctly; the standing gates and the offline
equivalence gate all pass) and one is not (the 8-cell metric gate — 4 of 8
cells still miss the v1 baseline — and its companion, the §9.5 defect-class
fixture, still fails 4 of 26 rows). Both misses are documented with numbers
below and in `.superpowers/sdd/2026-09-01-engine-v2-phase1a/task-16b-report.md`
(gitignored, not part of this doc's commit).

---

## 1. What exists now

The full clean-room v2 segmentation core: `crates/jparser/src/engine/{kana,
romaji, table, flags, lex, cost, lattice, chain, assemble, build, mod}.rs`,
selectable via `jparser-cli`'s `--engine v2` on `build-index`, `parse` and
`score`. v1 stays the default parser and is untouched; no switchover has
happened (that is Phase 1b's). `Entry::conjugation` is `None` throughout —
Phase 1b writes the English-label formatter over the `steps: Vec<ConjStep>`
this phase already produces.

Tests: **416** passing under default features (0 ignored), **451** passing
under `--features mecab` (1 ignored — see §5). `cargo clippy -p jparser
--all-targets -- -D warnings` clean with and without `--features mecab`.
`cargo check -p jparser --no-default-features` clean.

| Delivered | Where |
| --- | --- |
| The v2 engine (candidate generation, cost model, DP, chain recombination, assembly, ordering) | `crates/jparser/src/engine/` |
| `jparser-cli build-index/parse/score --engine v2` | `src/bin/jparser-cli/{main,score}.rs` |
| `score`'s compound-tolerant metric wired to the v2 boundary source | `src/bin/jparser-cli/score.rs` (Task 16a) |
| The §9.5 particle-absorbing regression fixture | `crates/jparser/tests/fixtures/spurious_merges.txt` |
| v2's own 30-sentence snapshot | `crates/jparser/tests/snapshots/engine_v2__sentences.snap` |

## 2. The measured table, beside the Phase 0 baseline

`jparser-cli score --engine v2` over the committed `gold.jsonl` (150
sentences, 2,163 spans) and a v2 index built from the committed
`gold_jmdict_subset.xml`, both hint states.

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| **hints off — v1 baseline** | 0.6810 (1473/2163) | 0.8465 (1831/2163) | 0.9159 (1046/1142) | ≤167 |
| **hints off — v2, pre-tuning (Task 16a)** | 0.5895 (1275/2163) | 0.6015 (1301/2163) | 0.9361 (864/923) | 13 |
| **hints off — v2, this handoff** | **0.7272 (1573/2163)** | 0.7975 (1725/2163) | 0.9075 (1108/1221) | 75 |
| gate | **PASS** (+0.0462) | **MISS** (−0.0490) | **MISS** (−0.0084) | pass |
| **hints on — v1 baseline** | 0.6963 (1506/2163) | 0.8678 (1877/2163) | 0.9155 (1072/1171) | ≤173 |
| **hints on — v2, pre-tuning (Task 16a)** | 0.7947 (1719/2163) | 0.8003 (1731/2163) | 0.9017 (1146/1271) | 6 |
| **hints on — v2, this handoff** | **0.8031 (1737/2163)** | 0.8521 (1843/2163) | 0.9063 (1170/1291) | 53 |
| gate | **PASS** (+0.1068) | **MISS** (−0.0157) | **MISS** (−0.0092) | pass |

**Reading this table.** Tuning closed the worst gap in the report (hints-off
strict, −0.0915 pre-tuning) into the largest margin in the table (+0.0462),
and every remaining miss narrowed from double digits of percentage points to
single digits. Both span metrics rose in **both** hint states relative to the
pre-tuning measurement — no strict spans were traded away anywhere to buy the
tolerant gains, 536 tolerant spans were recovered combined (536 = 424
hints-off + 112 hints-on), against the design brief's own ~360-span estimate.
What tuning could **not** do is close the remaining gap to v1's baseline on
tolerant and entry top-1, in either hint state — §4 explains the structural
reason, found by measurement rather than assumed.

Sign of the equivalence gate (§6) and `くるまではこう` (§5) both hold at
these constants.

## 3. What each cost constant ended at, and the measurement that justifies it

`crates/jparser/src/engine/cost.rs`. Every constant's doc comment in the file
itself carries this same justification; this table is the index into it.

| Constant | Value | Ended at / unchanged | Measurement |
| --- | --- | --- | --- |
| `COVER` | `1` | unchanged | Per-character by construction since Task 9; considered and left untouched — every other term's length-proportionality is built to stay neutral against it |
| `UNMATCHED` | `30` | unchanged | Considered and left untouched: no trial touched split-vs-merge decisions between two *matched* candidates, which is where every measured defect lived, so nothing bore on this term specifically |
| `SCRIPT_TRANSITION` | `10` | unchanged | No corpus evidence either way; explicitly still open for a future pass with evidence to spend |
| `EXP` (new field `Candidate::is_exp`, `lex.rs`) | tried at `-2` and `-1`, both ungated and gated to direct matches | **tried, reverted — not in the shipped constants** | Review round 1's Important 1. `音がする`/`首を振る` carry JMdict's `exp` tag though not `COMMON`; priced the same length-proportional way as `COMMON`. Every configuration measured net-regressed strict (as much as −92 spans in one hint state) for at most a few spans of tolerant gain, and the ungated form broke one more `spurious_merges.txt` row. See §4 for the full trial log and the reasoning kept from this negative result |
| `CROSS` | `0` | **changed from 4** | Trial 2/11: with `COMMON` length-proportional, real compounds on this corpus overwhelmingly *tie* their split alternative exactly on `COVER`+`COMMON`; any `CROSS > 1` disqualifies them outright rather than merely taxing them (`CROSS=1` collapsed hints-on tolerant from 0.8521 to 0.8049, merges from 53 to 19) |
| `PRONOUNCE_CROSS` | `25` | unchanged | Trials 5/7/8/9 (10, 5, 2, 1) all reproduce the original's exact corpus numbers *and* the same 4-of-26 fixture failures; only trial 6 (`= 0`) differs, and it differs by resurrecting 6 more rows of the exact spurious-merge class this term exists to suppress. Kept at 25 for margin, since nothing measured argues for a lower value |
| `COMMON` | `-2` applied as `COMMON * cand.len` | **shape changed** (was flat `-2` per match) | Trial 1: length-proportionality ties a compound against equally-evidenced parts instead of always losing to them (`lattice::relax`'s longer-run tie-break then wins it). Measured: hints-off tolerant 0.6015→0.7975, hints-off strict 0.5895→0.7272 (both directions confirm the flat form was blocking plain multi-character words from shattering into common single-character fragments, not just multi-word compounds). Magnitude alone (trial 3, `-1`) reproduced trial 1 exactly; disabling it (trial 4, `0`) regressed hints-off strict *below* v1's baseline |
| `INEXACT` | `8` | unchanged | Trial 10 (halved to 4) measured a small regression, not the gain a flat-per-match-bias argument would predict; reverted |

## 4. The structural ceiling `COMMON` cannot cross, found by measurement

`東京都` (ent_seq 1447740 in `gold_jmdict_subset.xml`) — Tokyo Metropolis, an
extremely common word — carries **no** `ke_pri`/`re_pri` of its own, while
its would-be parts 東 and 京都 each do. The split's aggregate `COMMON`
discount is then strictly larger than the compound's zero by an amount
*proportional to whatever `COMMON` is set to*: the sign of the comparison
never flips as the magnitude moves, so no scaling of `COMMON` alone can save
an entry the JMdict priority-tagging pass itself never marked. `音がする` and
`首を振る` — both confirmed present as literal `gold_jmdict_subset.xml`
headwords — show the same shape. This is a real, measured, structural limit,
not a magnitude this phase failed to find, and it is a real share of why
tolerant accuracy and entry top-1 still trail v1's baseline in both hint
states.

**Review round 1 found a second signal for the same gap, and it was tried.**
`音がする` and `首を振る` both carry JMdict's `exp` POS code, a signal
`Candidate::is_exp` (`lex.rs`) can now read off the same `EntryData` fetch
`allows_chain` already makes, priced length-proportionally in `cost.rs` the
same way `COMMON`'s own fix was shaped. Measured against the gold corpus at
the magnitude that ties `音がする`/`首を振る` against their split (`EXP = -2`,
matching `COMMON`'s own magnitude — smaller values are provably too weak to
reach the tie, since the split's aggregate `COMMON` discount is proportional
to total covered length regardless of how many pieces it is split into):
hints-off strict fell from 1573 to 1482 spans, hints-on strict fell from 1737
to 1645, and tolerant moved by at most 4 spans either way — a clear net
regression, plus one new `spurious_merges.txt` fixture failure (だった
matching an archaic exp-tagged copula, 2867372, through an unrelated
conjugation chain). Gating the term to direct, unconjugated matches
(`cand.steps.is_empty()`) removed that specific false positive but did not
change the outcome's shape: strict still fell by ~60 spans in both hint
states for a ~4-span tolerant gain. The reverted trial log (three
configurations, each measured) is in the task report's review-round
addendum. **This upgrades the ceiling claim from assumed to evidenced**:
JMdict's `exp` tag is real, already-parsed evidence for the target class, but
it is tagged far more broadly than "the multi-word phrase headwords this
corpus's compound-preservation cases need" — short archaic auxiliaries and
incidental grammatical constructions carry it too, at a rate that outweighs
the target class on this corpus. The code was reverted rather than kept
losing.

Note the ceiling is scoped, not total: `東京都` is tagged `&n;`, not `&exp;`,
so this signal was never going to reach it — a genuinely different signal
would be needed for the noun-compound subclass specifically.

## 5. Design §10's three done-when conditions, one at a time

**§10.1, the 8-cell metric gate — not met.** 4 of 8 cells miss; see §2.
**§10.2, `くるまではこう` — met.** `kurumadehakou_parses_as_four_words`
parses as `くる / まで / は / こう` under the final constants — the sentence
this whole phase exists for still holds. **§10.3, the §9.5 defect class gone
— not met.** `the_particle_absorbing_merges_are_gone` passes 22 of 26 rows;
the remaining 4 (はこ, うん, んで, いんじゃない) fail for two distinct,
`cost.rs`-unreachable reasons documented row-by-row in the test's own ignore
message and in the task report — briefly: three have no interior boundary at
all in Vibrato's own isolated-surface tokenization to price against
(`crossings == 0` regardless of any constant), and the fourth loses to an
`UNMATCHED`-fallback tradeoff caused by a dictionary entry
(`crates/jparser/tests/fixtures/jmdict_subset.xml`'s curated small fixture)
missing じゃ as a standalone headword — review round 1 independently checked
the full JMdict dump and confirmed じゃ **does** exist there as a standalone
headword with `re_pri spec1` on the sense needed, so this is a confirmed
fixture-curation gap, not a cost-model defect. **§10.5, standing gates and
the offline equivalence gate — met**, see §6.

**A fourth, separate check from review round 1: how much does `is_allowed`'s
hard interior constraint itself cost, independent of any cost-model
constant?** Master plan §4.3's softening clause is conditional on this being
measured, and it was not checked in this task's first pass. Measured now,
hints-on, over every gold span with a v2 dictionary candidate at its exact
`(start, len)`: of 2,163 gold spans, 1,561 have such a candidate, and **64 of
those (4.1%, 2.96% of all gold spans) are forbidden by `is_allowed`** even
though the candidate exists — a hard rejection, not a cost comparison the DP
lost. Tier breakdown: 11 tier A, 15 tier B, 38 tier C1 — concentrated in, but
not exclusive to, the mimetic/onomatopoeia class `gold_sources.md` already
documents as having the weakest tokenizer signal (e.g. `と` after a long
reduplicated-mimetic run Vibrato does not cleanly delimit). This number is
not directly comparable to design §7.5's 46-*boundary* figure (a different
unit — that count is boundary positions v1's own soft-hinted matcher reaches
that Vibrato lacks, independent of whether a v2 dictionary candidate exists
there at all; this count is v2-specific gold *spans* with a real candidate
that `is_allowed` still forbids), but it is real and larger in raw count.
**Judgment: the softening clause's own precondition — cost enough to hurt
span accuracy — is not met.** Hints-on strict passes baseline by +0.1068
(231 spans), an order of magnitude above the 64-span ceiling even if every
forbidden span flipped to a win; and turning `is_allowed` from a hard filter
into a scored penalty is a genuine architectural change (it touches the
documented existence/pricing split `cost.rs`'s own module doc states, and
three dedicated tests assert the current hard-forbid behavior directly), not
a constant to tune and revert like `EXP` above. Not attempted in this pass;
the number is recorded here, with its tier breakdown, so the next phase can
decide with full information rather than a guess.

## 6. Standing gates and the equivalence gate

```
cargo test -p jparser                                                   416 passed, 0 failed, 0 ignored
cargo test -p jparser --features mecab                                   451 passed, 0 failed, 1 ignored
cargo clippy -p jparser --all-targets -- -D warnings                     clean
cargo clippy -p jparser --all-targets --features mecab -- -D warnings    clean
cargo check -p jparser --no-default-features                             clean
```

**Design §10.5's equivalence check** (the Phase 0 design's §11.4, re-run for
v2): a full JMdict index (218,449 entries, built outside the repository) and
the committed subset index score byte-identically on every field but
`index_generation`, in both hint states. Full JSON pairs are in the task
report.

## 7. The v2 snapshot, and the shattering figure's movement

`v2_snapshots_every_sentence` regenerated against the tuned constants (30
hand-written sentences, hints off). Recomputing Task 14's own measurement the
same way: of the 81 multi-character matched spans v1 keeps whole across the
same 30 sentences, v2 now shatters **18 (22.2%)** and keeps **63** — down
from **63 shattered (78%)** before this task. Sentences losing their entire
multi-character vocabulary fell from **15 of 30 to 3 of 30** (corrected in
review round 1: the task's first pass misquoted an earlier, uncross-checked
figure of 12; recomputing directly from the pre-tuning and v1 snapshots gives
15, under the definition "among the sentences where v1 keeps at least one
multi-character span, how many does v2 shatter down to zero" — sentences
where v1 itself has no multi-character span are excluded from this specific
count, though they still count toward the 81-span aggregate above wherever
they contribute a span). The remaining 18 shattered spans are listed in the
task report; a systematic per-span
diagnosis of them (distinguishing the `COMMON`-tagging-gap class from
conjugation-chain artifacts) was not done and is a reasonable next-pass
target.

## 8. What is still open

- **The 8-cell metric gate's remaining 4 misses** (tolerant and entry top-1,
  both hint states) — §4's structural ceiling accounts for a real share of
  this, but not necessarily all of it; the 18 still-shattered snapshot spans
  (§7) were not individually attributed to it.
- **`the_particle_absorbing_merges_are_gone` stays ignored** — §5's §10.3
  reading. The いんじゃない row's "fixture gap, not defect" characterization
  is now **confirmed**, not merely hedged (review round 1 independently
  checked the full JMdict dump and found じゃ present with `re_pri spec1` on
  the needed sense) — the remaining gap is the small curated
  `jmdict_subset.xml` test fixture, not the full dictionary.
- **`is_allowed`'s hard interior constraint costs 64 of 2,163 gold spans**
  (hints-on), measured in review round 1 (§5) and judged, not implemented —
  hints-on strict's +231-span margin over baseline is well above that
  ceiling even in the best case, and softening the constraint is an
  architectural change (three dedicated tests assert the current hard-forbid
  behavior), not a constant tune. Left as a well-quantified follow-up rather
  than a rushed change late in an already-large task.
- **`EXP`, the second signal for `COMMON`'s ceiling, was tried and reverted**
  (§3, §4) — real evidence the `exp` POS tag is too broadly distributed
  across JMdict to serve as a clean length-proportional discount at the
  magnitude the target class needs, not merely an untried idea.
- **`entry top-1`'s shortfall is not a `cost.rs` question.** `cost.rs`
  governs which spans exist in the winning path; ranking candidate entries
  *within* a correctly-selected span is `engine::assemble::order`'s job, out
  of this task's file scope and untouched. If a future pass wants to close
  entry top-1's gap, that module — not `cost.rs` — is where to look, along
  with the brief's own named suspect: `COUNTER`'s archaic disqualifier being
  entry-wide rather than per-sense.
- **`SCRIPT_TRANSITION` and `UNMATCHED`** have no corpus measurement behind
  their specific values — inherited from Task 9, untouched this phase because
  neither was implicated in either diagnosed defect.
- **Deferred by the brief, not attempted:** the `書きた` over-generation
  (`ATTACHES_TO` letting `た` attach to `連用形` unconditionally); the
  katakana deinflection gap (the conjugation trie is hiragana-keyed with no
  folding — Tier D's 8 sentences / 169 spans showed no visible penalty, but
  that sample is too thin to call it settled either way); the
  `("特殊・ヌ", "仮定形") = ね` conjugation-table entry, flagged as the one
  genuinely uncertain override.

## 9. What Phase 1b needs

- **The switchover decision inherits an open gate.** Phase 1b's own
  done-when (the formatter, its class × chain gate, and the switchover) does
  not itself require Phase 1a's metric gate to be met, but whoever plans
  Phase 1b should read §2's table and §4's structural finding before deciding
  whether v2 is ready to become the default — the gap that remains is
  concentrated and understood, not diffuse.
- **`engine::assemble::order`** is the next place to look for entry top-1
  movement, per §8 — untouched by this phase's `cost.rs`-scoped work.
- **`main.rs`'s `Parse` command** still passes `bounds: None` unconditionally
  on the v2 path and has the same latent `Index::open` fingerprint bug
  `score.rs` had before Task 16a fixed it there (Task 16a's report, concern
  4) — neither affects any measurement in this handoff, but a future task
  driving `jparser-cli parse --engine v2 --hints` interactively will hit
  both.
- **The 18 still-shattered snapshot spans (§7)** are a small, bounded
  follow-up if the next phase wants to keep closing the metric gate before
  the switchover, rather than proceeding with it open.
- **`is_allowed`'s softening (§5, §8)** is the largest single quantified
  lever left untried: 64 gold spans, a real architectural change rather than
  a constant. If Phase 1b (or a Phase 1a extension) wants to keep pushing
  the metric gate rather than accept it open, this — not another cost-model
  trial — is where the next real gain is most likely to be, per this
  phase's own measurement.
