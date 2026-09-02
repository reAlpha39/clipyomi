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

## 5. Standing gates at the final commit

Run at `428fac4`: `cargo test -p jparser` (283 passed), the same with
`--features mecab` (309 passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` (269 passed), `cargo test --manifest-path
src-tauri/Cargo.toml` (149 passed). Eight cells byte-identical at
`--readings 1`.

## 6. Open items

- `RAW_FACTOR` in `lattice.rs` is a fixed over-provision for the dominance
  filter; a top-1 with more than nine matched spans can push a genuine
  alternate priced above +29 out of the raw list. Upgrade path: dominance
  pruning inside the DP lists (design §3.2a).
- `display` resets the sentence pane's scroll on a reading switch, as it
  does on a furigana change. Harmless on short clipboard text; a long parse
  loses its place.
- The 9 gold spans with no candidate at all (`嫌な`, `完成する`, `見ないで`
  and kin) and the 3 with the span but not the entry (`あり` wanting
  1296400) are lexicon or index findings for a later phase, not k-best ones.
