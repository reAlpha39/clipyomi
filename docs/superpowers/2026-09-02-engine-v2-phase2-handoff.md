# Engine v2 Phase 2 — handoff

**Date:** 2026-09-02
**Design:** [`specs/2026-09-02-engine-v2-phase2-design.md`](specs/2026-09-02-engine-v2-phase2-design.md)
**Plan:** [`plans/2026-09-02-engine-v2-phase2.md`](plans/2026-09-02-engine-v2-phase2.md)
**Branch:** `feat/engine-v2`

## 1. What landed before the gate

Three commits, each re-measured at `--readings 1` against Phase 1b-i handoff
§2's eight cells and byte-identical to them:

1. `lattice::k_best` — the bounded, key-deduplicated list per position;
   `best_path` is its k = 1 case.
2. `ParseOptions::with_readings` and `ParseResult.alternatives`, empty by
   default.
3. `jparser-cli score --readings K` with the oracle@K lines.

## 2. The gate measurement

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
(the copy under `corpus_staging/gold_index` is format 3 and no longer
opens); Vibrato dictionary at
`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

Hints off:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7272  (1573 / 2163)
entry top-1:    0.9075  (1108 / 1221)
  by decision:  e 0.7272 (1573 / 2163)
span tolerant:  0.7975  (1725 / 2163)
  coarsening:   152 gold spans inside 75 dictionary merges
oracle@3 strict:    0.7531  (1629 / 2163)
oracle@3 tolerant:  0.8086  (1749 / 2163)
oracle@3 sentences: 0.0067  (1 / 150)
```

Hints on:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8031  (1737 / 2163)
entry top-1:    0.9063  (1170 / 1291)
  by decision:  e 0.8031 (1737 / 2163)
span tolerant:  0.8521  (1843 / 2163)
  coarsening:   106 gold spans inside 53 dictionary merges
oracle@3 strict:    0.8534  (1846 / 2163)
oracle@3 tolerant:  0.8706  (1883 / 2163)
oracle@3 sentences: 0.1667  (25 / 150)
```

## 3. Verdict

Design §8 clause 1: oracle@3 tolerant hints on must be at least 1,898 of
2,163, 55 spans over top-1's 1,843. Measured: **1,883 of 2,163, 1,883 − 1,843 = 40
spans.** The gate **fails**.

Commit 4 does not land. `k_best`, `with_readings` and
`score --readings` stay as the measurement instrument, `alternatives` stays
empty on the wire, and Phase 2 ships no UI, as master plan §7 allows.

Context beside the gate, none of it thresholded: strict oracle@3 hints on,
the hints-off pair, and the sentence-level count are in the outputs above.
The oracle@∞ ceiling the design was built on (design §2.2) was 1,951
tolerant hints on, so oracle@3 reached (1,883 − 1,843 =) 40 of the 108 reachable spans.

### Strict versus tolerant

Strict oracle@3 hints on gained +109 spans over strict top-1 (1737 → 1846);
tolerant gained only +40 (1843 → 1883). Had the gate been written on the
strict number, +109 would have cleared the same 55-span bar comfortably —
and the gain would have been dominated by re-splitting the JMdict compounds
the corpus splits at short-unit granularity and the engine correctly keeps
merged. That is precisely what design §8 chose the tolerant metric to avoid
crediting, and the choice was made before any of these numbers existed. This
measurement is what vindicates it, not what motivated it.

## 4. What landed after the gate

Nothing. The wire field is empty and the menu is unchanged.

## 5. Standing gates at the phase's final code commit

Run at `428fac4`: `cargo test -p jparser` (283 passed), the same with
`--features mecab` (309 passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` (269 passed), `cargo test --manifest-path
src-tauri/Cargo.toml` (149 passed). Eight cells byte-identical at
`--readings 1`.

## 6. Open items

- `RAW_FACTOR`'s over-provision risk is retired on this corpus, not merely
  bounded. The final Phase 2 review measured it directly: at k = 3, the
  shipped cap of 12 raw readings per position returns byte-identical
  top-three reading lists to a cap of 96 on all 150 gold-corpus sentences, in
  both hint states, with no change to either oracle figure -- even though 70
  of the 150 sentences have more than nine matched spans (the point past
  which `RAW_FACTOR`'s doc comment warns a genuine alternate could fall off
  the raw list) and 58 have more than eleven. The reason is structural, not
  luck: with `CROSS = 0` the alternates that matter are exact-cost ties with
  the reading they compete against, while the dominated one-word-unmatched
  variants that would need to crowd them out cost roughly +30 more, so they
  never do. Dominance pruning inside the DP lists (design §3.2a) is not a
  live risk to close on this corpus; see below for where the real headroom
  is instead.
- The measurement above points at the actual lever for a future oracle@k
  gain: reading identity, not list capacity. With `CROSS = 0` a real
  compound ties its split alternative exactly, so ranks 1 and 2 of the
  reading list are routinely spent on re-splits the tolerant metric forgives
  by construction -- which is why strict oracle@3 gained +109 spans over
  strict top-1 while tolerant gained only +40 (§3, "Strict versus
  tolerant"). If `k_best`'s notion of "distinct" also collapsed readings
  differing only by a tolerant-equivalent merge or split -- the same rule
  `score.rs`'s `accumulate` already applies when scoring -- ranks 1 and 2
  would be freed for readings that could actually move the metric. Named
  next experiment for anyone revisiting Phase 2; not attempted here because
  it changes what `k_best` returns, and the gate had already reached its
  verdict.
- Whoever builds the reading-cycling interface should decide what happens to
  the sentence pane's scroll position. `display` resets it on every render,
  which is why a furigana change already returns a long parse to the top
  (`src/main.ts`); a reading switch would inherit that.
- The 9 gold spans with no candidate at all (`嫌な`, `完成する`, `見ないで`
  and kin) and the 3 with the span but not the entry (`あり` wanting
  1296400) are lexicon or index findings for a later phase, not k-best ones.

---

## 7. Addendum, 2026-09-09 — the named next experiment, measured

§6's second bullet named reading identity as the lever and did not pull it.
It has now been measured, by
[`crates/jparser/examples/reading_identity.rs`](../../crates/jparser/examples/reading_identity.rs)
(committed at `6145497`, refinement arm at `ad73d2d`), which touches no shipped code: it asks `parse` for
a deeper reading list, filters that, and keeps three. Same gold index rebuilt
from `gold_jmdict_subset.xml`, same Vibrato dictionary, hints on, 150
sentences, 2,163 gold spans.

Three filters. The first two share one relation — one reading's boundary set
contained in the other's, in **either** direction, because merge and split
are one relation read from opposite ends — and the third asks whether that
symmetry is right:

- **Collapse** deletes a reading equivalent to an earlier kept one.
- **Demotion** sinks it below every reading that is not equivalent, and
  deletes nothing.
- **Refinement-only demotion** sinks a reading only when it is *finer* than a
  kept one (its cut set a superset), leaving coarser readings fresh. Its
  motive is that the tolerant rule is itself asymmetric: a matched segment
  swallowing gold spans whole at gold boundaries is forgiven, a segment finer
  than a gold span is not, so a refinement cannot earn tolerant credit its
  kept reading lacks while a coarser reading can — wherever gold merges and
  the kept reading splits, only the coarse alternative is forgiven.

Depth swept, because every rank above the shipped 3 costs the DP a wider list
at every position, so the shallowest depth that clears the gate is the one
worth shipping. Gains are spans over top-1, out of 2,163:

```
       tolerant gain over top-1        strict gain over top-1
deep   plain  collaps  demote  refine    plain  collaps  demote  refine
3        +40         +3      +40      +40     +109         +2     +109     +109
4        +40        +16      +40      +42     +109        +27     +109     +106
6        +40        +31      +48      +49     +109        +52     +112     +102
12       +40        +49      +65      +62     +109        +99     +132      +98
24       +40        +53      +67      +65     +109       +109     +134      +87
64       +40        +54      +68      +67     +109       +110     +135      +87
```

At depth 64, absolute:

```
                      strict                 tolerant
top-1              0.8031 (1737 / 2163)   0.8521 (1843 / 2163)
oracle@3 plain     0.8534 (1846 / 2163)   0.8706 (1883 / 2163)
oracle@3 collapsed 0.8539 (1847 / 2163)   0.8770 (1897 / 2163)
oracle@3 demoted   0.8655 (1872 / 2163)   0.8835 (1911 / 2163)
oracle@3 refined   0.8433 (1824 / 2163)   0.8830 (1910 / 2163)
```

### 7.1 What this changes

**`oracle@3 plain` reproduces §2's recorded gate figure exactly at every
depth** — +40 tolerant, +109 strict, 1,883 of 2,163. That is the control:
the spike's own reimplementation of `score.rs`'s two hit rules agrees with
the shipped scorer span for span, so the rows below it are readable.

**Demotion clears design §8 clause 1 from depth 12.** The clause needs
oracle@3 tolerant hints on at 1,898 of 2,163, +55 over top-1's 1,843.
Demotion at depth 12 reaches 1,908 (+65) and at depth 64 reaches 1,911
(+68); the tolerant ceiling is 1,951, so +68 is 63% of the reachable
headroom against plain's 37%. **Collapse does not clear it**: +54 at depth
64, one span short, and it pays for that by deleting readings the tolerant
metric is defined not to see — the same shape of error master plan §10
records for compound preservation. Demotion has no such cost, which is why
it is the variant worth shipping and collapse is recorded here only as the
measurement that isolates the deletion's contribution.

**The symmetry is right, and this is what settles it.** Refinement-only
demotion reaches tolerant +67 against symmetric demotion's +68 — a wash — but
its strict figure falls to +87, *below plain's +109*. The finer readings it
protects the coarse ones by sinking are exactly the ones a short-unit
granularity corpus credits strictly, so keeping the coarse alternative costs
more strict credit than the tolerant metric returns for it. Symmetric
demotion wins both metrics; the asymmetric variant is recorded as the
measurement that rules it out, not as an option still open.

**Depth 3 is exactly plain, at both metrics.** Nothing to reorder inside
three slots when the equivalences are already occupying them: the gain is
entirely in reaching ranks the shipped k never returns. Between depth 12 and
64 the tolerant gain moves +3, so the useful readings are shallow — 12 is
where this stops paying for itself.

### 7.2 The costs, stated

- **Whole-sentence gold cover in the top three regresses, 25 → 24 of 150**
  (top-1 alone: 16). Demotion buys +28 tolerant span hits and loses one
  sentence's complete cover, because a full-cover reading that happens to be
  merge/split equivalent to rank 0 can be sunk past rank 3. §5.2's
  sentence-level count was context beside the gate, not a threshold, and
  this stays context — but it is a regression and it is recorded, not
  averaged away.
- **The product question is unresolved and is now sharper.** `くるまで`
  against `くる` + `まで` is exactly the relation demotion sinks, and
  `engine_v2.rs` already uses that sentence as the canonical second reading a
  human would want to cycle to. The tolerant metric cannot see the
  difference between that genuine ambiguity and a short-unit-granularity
  re-split of a compound the engine correctly keeps merged; boundary-subset
  cannot either. The filter fired on 90 of 150 sentences and sank 461
  readings, so the question is large, not marginal.
- **Depth is not free.** The spike reads depth off `parse`, which sets the
  DP's per-position cap to `k * RAW_FACTOR`; asking for 12 readings widens
  every position's list from 12 to 48. Whether the reorder can instead run
  over the raw pool the shipped k = 3 already holds — cap 12, before
  `undominated` truncates to 3 — is unmeasured and is the first thing worth
  measuring, because it would cost the DP nothing.

### 7.3 What it does not change

Nothing shipped. `k_best`, `parse` and the wire are untouched, `alternatives`
is still empty by default, and the eight cells are not disturbed — the spike
never runs at `--readings 1`, and demotion is the identity at k = 1 in any
case, since rank 0 is always the first reading kept. §3's verdict stands as
the record of what Phase 2 measured and shipped. Reopening it is a new
phase's decision, with its own design, not an edit to this handoff's
verdict.
