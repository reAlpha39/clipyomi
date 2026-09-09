# Engine v2 Phase 2b — reading identity, and the UI Phase 2's gate withheld

**Date:** 2026-09-09
**Branch:** `feat/engine-v2`
**Prior:** [Phase 2 design](2026-09-02-engine-v2-phase2-design.md) ·
[Phase 2 handoff](../2026-09-02-engine-v2-phase2-handoff.md), §6 and §7 ·
[master plan](2026-08-27-engine-v2-master-plan.md) §7

## 1. Scope

Phase 2 built the k-best lattice, the `alternatives` wire field and the
`score --readings K` instrument, measured its own gate, and shipped no UI:
oracle@3 tolerant hints on reached +40 spans over top-1 against the +55
required. Its handoff §6 named the lever — reading identity, not list
capacity — and its handoff §7 measured it: demoting readings that differ from
a kept one only by a merge or split clears the same gate, +65 at depth 12 and
+68 at depth 64.

Phase 2b changes what `lattice::k_best` calls a *rank* and then lands Phase 2
design §6's UI unchanged. Specifically:

**In.** A demotion pass inside `k_best`, between `undominated` and the
truncation to k: a reading whose boundary set is contained in an already-kept
reading's, or contains it, sinks below every reading that is not so related.
Nothing is deleted. The gate is re-measured through the shipped path. On
pass, Phase 2 design §6's reading-cycling UI lands as written, plus the
scroll decision Phase 2 handoff §6 left open.

**Out.** Any change to the cost model, the candidate lexicon, `is_allowed`,
the wire shape, or the public API. Per-region alternates (Phase 2 design §12
holds them out and this design does not reopen it). Automatic disambiguation.
A second lattice.

**Unchanged bar.** The gate is Phase 2 design §8 clause 1 verbatim: oracle@3
tolerant, hints on, at least 1,898 of 2,163. The number is not renegotiated
because the lever changed — that is the whole point of re-measuring against
it.

## 2. What the spike settled, and what it left

[`crates/jparser/examples/reading_identity.rs`](../../../crates/jparser/examples/reading_identity.rs)
(`6145497`, refinement arm `ad73d2d`) measured this outside the engine: it
asks `parse` for a deeper reading list, filters that, keeps three, and prints
`oracle@3` over the engine's own unfiltered first three beside it as a
control. The control reproduces Phase 2's recorded gate exactly at every
depth (+40 tolerant, +109 strict, 1,883 of 2,163), so the filtered rows are
readable. Full table in Phase 2 handoff §7.

**Settled.**

- **Demote, do not collapse.** Deleting the equivalent readings reaches +54,
  one span short of the gate, and pays for it by removing readings the
  tolerant metric is defined not to see — the same shape of error master plan
  §10 records for compound preservation. Demotion reaches +68 and deletes
  nothing.
- **Symmetric, not refinement-only.** The tolerant rule is asymmetric — a
  matched segment swallowing gold spans whole at gold boundaries is forgiven,
  a finer segment is not — which argues for sinking only refinements and
  keeping coarser alternatives, since only a coarse reading can earn credit
  where gold merges and the kept reading splits. Measured, that variant is a
  wash on tolerant (+67 against +68) and a collapse on strict (+87, *below*
  plain's +109), because the finer readings it protects the coarse ones by
  sinking are exactly what a short-unit corpus credits strictly. Symmetric
  wins both.
- **Depth 12 is where it stops paying.** +65 at 12, +67 at 24, +68 at 64. At
  depth 3 demotion is exactly plain at both metrics: the gain is entirely in
  reaching ranks the shipped k never returns.

**Left open, and §3.3 is how this design closes it.** The spike reads depth
off `parse`, which sets the DP's per-position cap to `k * RAW_FACTOR`, so its
depth-12 arm widened every position's list from 12 to 48. Whether the reorder
can instead run over the pool the shipped k = 3 *already holds* is
unmeasured, and that is the difference between a free change and a 16× term
in the DP.

## 3. The change

### 3.1 The relation

Two readings are **merge/split equivalent** when one's boundary set is
contained in the other's. A reading's boundary set is every step's start plus
the input length: `東京都` cuts at {0, 3} and `東京` + `都` at {0, 2, 3}, so
the first is the second with one boundary removed. Containment in *either*
direction, because merge and split are one relation read from opposite ends,
and because §2 measured the asymmetric alternative and it loses.

Three properties that matter for the implementation and the tests:

- **It is not an equivalence relation.** Containment in either direction is
  reflexive and symmetric but not transitive: {0,4} ⊆ {0,2,4} and
  {0,4} ⊆ {0,3,4}, while {0,2,4} and {0,3,4} are incomparable. So the pass is
  a greedy sweep, not a partition, and its output depends on the order it
  sweeps — cost order. That is a property to document and test, not a defect
  to engineer around: cost order is the only order available, and rank 0 is
  the reading the engine actually chose.
- **A reading is compared against the kept-fresh list only**, never against
  readings already sunk. Otherwise one sunk reading would drag unrelated
  readings down after it.
- **Rank 0 never moves.** It is the first reading the sweep sees, the fresh
  list is empty, so it is always kept fresh and always emitted first. At
  k = 1 the pass is the identity, so `best_path` and the eight cells are
  untouched by construction rather than by measurement.

### 3.2 Where it goes

`k_best` today ends with `undominated(raw, k)`, which both filters dominated
readings and truncates to k in one loop. Phase 2b splits those:

```rust
let pool = undominated(raw, cap);   // filter only; cap, not k
demote(pool, k)                     // reorder, then truncate to k
```

`undominated` builds its kept list in cost order and breaks when it is full,
so its first k entries are identical whether it is called with `k` or with
`cap` — passing the larger bound extends the list and cannot disturb its
prefix. The truncation to k moves from `undominated` into `demote`, which is
the only reason the split is needed at all: to free a rank the reorder has to
happen before the list is cut, which is exactly why the spike could not
filter `parse`'s three returned readings.

`demote` is `undominated`'s shape: one pass in cost order, a `Vec` of kept
boundary sets, a linear scan per reading. `Vec<BTreeSet<usize>>` for the
boundary sets, `is_subset` in both directions — the same two lines the spike
ran. At `cap` = 12 the scan is at most 12 × 12 set comparisons per sentence,
against a DP that has already relaxed every edge in the lattice.

### 3.3 Depth: measure the free variant first

`cap` is `k * RAW_FACTOR` = 12 at the shipped k = 3. The reorder over that
pool costs the DP nothing — no wider lists, no extra relaxation, no latency
clause owed. Whether it clears the gate is unmeasured: the spike's +65 came
from 12 undominated readings drawn from a cap-48 pool, and the free variant's
12 come from a cap-12 pool. The answer is somewhere in [+40, +65] and only
the shipped path can say where.

**So task 1 measures the free variant and task 2 exists only if it falls
short.** If it does, the fallback is to decouple the DP's pool from the
caller's k — `cap = max(k, DEMOTE_POOL) * RAW_FACTOR`, with `DEMOTE_POOL` at
the shallowest depth the sweep shows sufficient — and it carries a latency
clause (§8 clause 4), because `k_best` is `O(E · cap²)` and 12 → 48 is a 16×
term. Not a speculative knob: it is one `const` and one `max`, added only on
a measured shortfall, and named here so the shortfall does not become a
redesign.

## 4. Public API

No signature changes. One doc change, which is a real semantic change and is
called out rather than smuggled: `ParseResult::alternatives`
(`crates/jparser/src/lib.rs:71`) currently reads "readings 2..k in cost
order". Rank is no longer cost order alone — it is cost order among readings
not merge/split equivalent to a cheaper one, then the equivalent ones in cost
order. Phase 2 design §1's "the order is the rank" still holds; what a rank
*means* is what this phase changes. `segments` stays top-1, the wire shape is
untouched, and nothing downstream needs a version bump.

## 5. Harness

Nothing. `score --readings 3` measures the new order through the same code
path the app uses, and Phase 2's recorded 1,883 is the before. The spike is
already committed as the A/B instrument, so no `--no-demote` flag is owed — a
flag would be a second code path to keep honest for one measurement that
already exists.

## 6. Application and UI

Phase 2 design §6 as written, unchanged: `with_readings(3)` in the worker,
`alternatives: Segment[][]` in `types.ts`, `readings(result)` and
`readingIndex` in `main.ts`, right-click radio rows only when alternates
exist, `]` and `[` to cycle with wrapping. It was designed against a wire
field that already exists and a `renderSentence` that never knew which
reading it was drawing; none of that moved.

One addition, from Phase 2 handoff §6: **cycling preserves the sentence
pane's scroll position.** `display` resets it on every render, which is why a
furigana change already returns a long parse to the top. A reading switch
would inherit that, and unlike a furigana switch it is a navigation the user
makes *while reading a specific word* — losing their place is the one thing
that makes cycling useless on exactly the long sentences it is for. The fix
sits in `display`: capture `scrollTop` before the re-render and restore it
after, when the result is `lastResult`. Whether that also improves the
furigana case is a question this design does not answer and does not need
to — the same guard covers both, because both re-render `lastResult`.

## 7. Staging

1. **`demote` in `lattice::k_best`, over the existing pool.** `undominated`
   called with `cap`, `demote` truncating to k, unit tests (§9), the
   `alternatives` doc corrected. Eight cells re-run at `--readings 1`.
   **The gate is measured here and its verdict written into the Phase 2b
   handoff before task 3 is started.**
2. **Only on a shortfall:** `DEMOTE_POOL`, plus the latency measurement §8
   clause 4 requires. Re-measure the gate. Eight cells re-run.
3. **App and UI.** §6, only on pass.

On fail — the free pool short of +55 and the deeper pool either short or too
slow — task 1 still lands: it is a strictly better rank order at zero cost,
tested, with the wire field still empty. Phase 2's "ships nothing" precedent
applies again, and it applies to the UI, not to the instrument.

## 8. Done when

Measured hints on, gold corpus, K = 3, `score`'s per-span union:

1. **oracle@3 tolerant is at least 1,898 of 2,163** — Phase 2 design §8
   clause 1 verbatim, +55 over top-1's 1,843.
2. **The eight cells are byte-identical** to Phase 1b-i handoff §2's table at
   the default `--readings 1`, after every task. Demotion is the identity at
   k = 1 (§3.1), so this is a check on the refactor, not on the idea.
3. **Strict oracle@3, the hints-off pair and the sentence-level count are
   printed beside it, unthresholded** — and the sentence count is *recorded
   even though it regresses*: the spike measured whole-sentence gold cover in
   the top three falling 25 → 24 of 150 while tolerant spans gain, because a
   full-cover reading equivalent to rank 0 can sink past rank 3. It was
   context beside Phase 2's gate and it stays context, but it is a real
   regression and the handoff states it rather than averaging it away.
4. **Latency, only if task 2 runs:** `score` wall time over the 150-sentence
   corpus, both hint states, within 10% of task 1's. A wider DP pool is worth
   a measurable slowdown only if the readings it buys are the ones the user
   cycles to, and nothing measured so far says they are.
5. **Standing gates:** `cargo test -p jparser`, the same with `--features
   mecab`, `cargo clippy -p jparser --all-targets -- -D warnings` in both
   feature states, `cargo check -p jparser --no-default-features`, `cargo fmt
   -p jparser -- --check`, `npm test`, `cargo test --manifest-path
   src-tauri/Cargo.toml`.
6. **Task 3 lands only if clause 1 holds**, and its own gate is Phase 2
   design §9's Vitest rows plus one for scroll preservation.

## 9. Tests

- **`lattice`**: a merge/split pair sinks below an unrelated reading that was
  cheaper than neither; rank 0 is emitted first in every case, including when
  every other reading is equivalent to it; a reading sharing no containment
  relation is not demoted; two readings equivalent to rank 0 but not to each
  other keep their cost order among themselves; the greedy sweep's
  order-dependence is asserted rather than left implicit; output is still at
  most k long and every path still a contiguous cover; every existing
  `best_path` and `k_best` test passes unchanged.
- **`engine_v2`**:
  `three_readings_come_out_in_cost_order_with_dominated_ones_dropped`
  survives as written — on `東京都` every alternative is a refinement of the
  single-segment top-1, so all of them sink together and their relative order
  is preserved. That deserves an explicit comment in the test rather than
  passing by luck: it is the case that shows demotion reorders between groups
  and never reshuffles within one. A new test needs a sentence with a
  genuinely unrelated second reading, to show one promoted past an
  equivalent.
- **`score` unit**: unchanged; K = 1 output byte-identical.
- **Vitest**: Phase 2 design §9's rows, plus scroll position preserved across
  a cycle and across a furigana re-render of the same result.

## 10. Budget

`lattice.rs` +45 (`demote`, its doc, the `k_best` tail) on 649 today, tests
+90. `lib.rs` doc only. `main.ts` +55 including the scroll guard, `types.ts`
+1. No new file, no new dependency, no new public item.

## 11. Master plan edits this design owes

Both already applied at the commit that adds this file:

- **§6 Phase order.** Phase 2 split into `2` (instrument only, gate failed at
  +40) and `2b` (reading identity, gate cleared at +65).
- **§7 Phase 2.** The 2026-09-02 "measured and not met" note keeps its
  wording and gains the 2026-09-09 reopening beside it, with both recorded
  costs and a pointer to handoff §7.

Nothing in §5.2's metric definitions changes: the gate, the metrics and the
corpus are Phase 2's.

## 12. Decision log

| Decision | Rationale |
| --- | --- |
| Demote, never delete | Collapse reaches +54 against the +55 gate and buys that by deleting readings the tolerant metric cannot see, the error shape master plan §10 already records. Demotion reaches +68 with nothing removed (§2) |
| Symmetric containment, not refinement-only | The tolerant rule's asymmetry argues for keeping coarser alternatives, and the measurement rejects it: +67 tolerant against +68, but strict falls to +87 from plain's +109 (§2) |
| Reorder between `undominated` and the truncation | To free a rank the reorder must precede the cut — filtering the three returned readings can only reduce them, which is why the spike could not touch `parse`. `undominated`'s prefix is bound-independent, so passing `cap` extends its list without disturbing it (§3.2) |
| Free pool first, `DEMOTE_POOL` only on a shortfall | The reorder over the pool k = 3 already holds costs the DP nothing; a deeper pool is a 16× term in `O(E · cap²)`. The spike measured depth on `parse`'s k, not on the pool, so the free variant's gain is genuinely unknown and worth one measurement before a knob (§3.3) |
| Same gate number, not a renegotiated one | Phase 2 chose +55 tolerant hints on before any of these figures existed, and that is what makes this measurement mean anything. A lever that clears the original bar is a result; a bar moved to fit the lever is not (§1, §8) |
| The sentence-cover regression is recorded, not thresholded | It was context beside Phase 2's gate and the metric has not changed. But it is a loss (25 → 24 of 150) and the honest place for it is the handoff, stated, not a footnote in a spike (§8 clause 3) |
| Scroll preservation is in scope, per-region alternates are not | Cycling is a navigation made while reading one word; resetting scroll makes it useless on the long sentences it exists for, and the guard is a few lines in `display`. Per-region alternates are a second selection model and Phase 2 design §12 already held them out (§6) |
| `alternatives`'s doc changes, its type does not | Rank stops meaning cost order alone. That is a semantic change to a shipped field and it gets stated in the doc comment rather than discovered by a reader (§4) |

## 13. Open questions

- **The product question the metric cannot answer.** `くるまで` against
  `くる` + `まで` is exactly the relation demotion sinks, and `engine_v2.rs`
  already uses that sentence as the canonical second reading a human would
  want. The tolerant metric cannot distinguish that genuine ambiguity from a
  short-unit re-split of a compound the engine correctly keeps merged, and
  boundary containment cannot either. The spike's filter fired on 90 of 150
  sentences and sank 461 readings, so the question is large. This design
  ships the metric-improving order because the gate is the only instrument
  the plan has; it does not claim the order is what a reader wants, and the
  first honest test of that is the UI itself.
- **Whether k should rise for the UI.** Demotion deletes nothing, but `parse`
  still truncates to 3, so the sunk merge/split variants do not reach the
  wire at all — the reader loses access to precisely the alternatives the
  bullet above says they might want. Raising k to 4 or 5 for the app while
  the gate stays at 3 would return them, at the cost of a longer menu and a
  wider DP pool. Out of scope here, and a candidate for the phase that has UI
  evidence to argue from.
- **Whether the scroll guard belongs to furigana too.** §6 fixes both cases
  with one guard because both re-render `lastResult`. If that turns out to
  change furigana behaviour someone relies on, the guard narrows to the
  reading-switch path.
