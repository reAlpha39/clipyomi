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
