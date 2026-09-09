# Engine v2 Phase 2b — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2b-design.md`](specs/2026-09-09-engine-v2-phase2b-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2b.md`](plans/2026-09-09-engine-v2-phase2b.md)
**Branch:** `feat/engine-v2`

## 1. What landed before the gate

One commit, re-measured at `--readings 1` against Phase 1b-i handoff §2's
eight cells and byte-identical to them: `lattice::demote`, the reorder
between `undominated` and the cut to k (design §3.2), over the raw pool the
shipped k = 3 already holds — cap 12, no DP widening. `k_best`'s signature,
`parse`, `score` and the wire shape are unchanged; `ParseResult::alternatives`'s
doc comment now says rank is not cost order alone.

## 2. The gate measurement, over the existing pool

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`;
Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

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
oracle@3 strict:    0.7628  (1650 / 2163)
oracle@3 tolerant:  0.8206  (1775 / 2163)
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
oracle@3 strict:    0.8632  (1867 / 2163)
oracle@3 tolerant:  0.8821  (1908 / 2163)
oracle@3 sentences: 0.1533  (23 / 150)
```

Phase 2 recorded, at the same `--readings 3` before this phase: hints on
oracle@3 strict 1846, tolerant 1883, sentences 25; hints off 1629, 1749, 1.

## 3. Verdict

Design §8 clause 1, which is Phase 2 design §8 clause 1 unchanged: oracle@3
tolerant hints on must be at least 1,898 of 2,163, 55 spans over top-1's
1,843. Measured over the pool k = 3 already holds: **1,908 of 2,163, 1,908 −
1,843 = 65 spans.** The gate **holds** at this pool.

Strict oracle@3 hints on: 1,867 of 2,163 (Phase 2: 1,846). Whole-sentence gold
cover in the top three: 23 of 150 (Phase 2: 25). Neither is thresholded
(design §8 clause 3); both are recorded here rather than averaged away, and
the sentence count is the one the off-engine spike saw regress, 25 → 24.
