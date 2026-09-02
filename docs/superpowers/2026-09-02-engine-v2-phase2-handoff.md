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
