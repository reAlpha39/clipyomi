# Engine v2 Phase 2c — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2c-design.md`](specs/2026-09-09-engine-v2-phase2c-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2c.md`](plans/2026-09-09-engine-v2-phase2c.md)
**Finding:** [`2026-09-09-candidate-generation-blowup.md`](2026-09-09-candidate-generation-blowup.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Three commits change code or its rationale. Five stand between the plan
(`ef960be`) and `HEAD`: the other two are this handoff's own commit
(`e07fb82`), which cannot name itself, and a follow-up documentation fix
(`7d2d27d`) that lands after it.

`b6821d8` (`feat(engine-v2): expose the key FST's forward walk as a
resumable Walk`) adds `Index::walk` and the `Walk` type — a position that
`push` advances one folded character at a time, `records` reads the key
ending there, and `records_after` tests an extension without moving it —
then reimplements `prefixes_of_with` on top of it, unchanged in signature
and behaviour.

`1529bbf` (`docs(engine-v2): restore the walk's root-check and fold
rationale`) restores doc-comment rationale that the first commit's
replacement text had dropped: why the root node's finality is checked at
all, the v1-builder history behind the `key_chars == 0` branch, and why
`fold` is a runtime parameter rather than a constant. It also adds a
pointer from `Walk::records` back to that doc, since the root-finality
check itself moved there.

`2b8258e` (`fix(engine-v2): walk the index once per position instead of
once per span`) restructures `candidates`: chains are reindexed by stem end
rather than span end, one `index.walk()` runs per start position, each
conjugation class's remove-suffix is tested against the standing walk, and
the key `String` is built only for a span that already matched. No span
bound and no constant is introduced — the walk's own dead end is the bound.

## 2. The gate — byte-identical output

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`;
Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

```
--- hints off ---
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7272  (1573 / 2163)
entry top-1:    0.9075  (1108 / 1221)
  by decision:  e 0.7272 (1573 / 2163)
span tolerant:  0.7975  (1725 / 2163)
  coarsening:   152 gold spans inside 75 dictionary merges
```

```
--- hints on ---
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8031  (1737 / 2163)
entry top-1:    0.9063  (1170 / 1291)
  by decision:  e 0.8031 (1737 / 2163)
span tolerant:  0.8521  (1843 / 2163)
  coarsening:   106 gold spans inside 53 dictionary merges
```

```
--- readings 3, hints on ---
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

Spec §5 clause 1 asked for this triple in both hint states; the hints-off
side of it was not run before this handoff was written. Measured after the
fact, over the same gold index:

```
--- readings 3, hints off ---
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

That matches Phase 2b handoff §2's own recorded hints-off `oracle@3` --
1,650 / 1,775 / 1 -- exactly, not Phase 2's earlier, pre-`demote` 1,629 /
1,749 / 1 (Phase 2 handoff §2). The two are different measurements of
different code: 1,629 / 1,749 / 1 predates `lattice::demote`, which Phase 2b
landed and which moved both hint states' figures (hints on 1,846 → 1,867
tracks the same change); 1,650 / 1,775 / 1 is Phase 2b's own post-`demote`
gate. Phase 2c changes nothing downstream of candidate generation, so
Phase 2b's own figures are the correct reference, and this measurement
reproduces them.

All eight cells at `--readings 1` in both hint states, and all three
`oracle@3` figures at `--readings 3` in both hint states, match the recorded
Phase 2b values exactly. The candidate set did not move.

## 3. What it bought

Release build, full 218,449-entry index, hints off, `jparser-cli parse`,
k = 1:

| input chars | before | after |
| --- | --- | --- |
| 500 | 0.763 s | 0.055 s |
| 1,000 | 4.284 s | 0.077 s |
| 2,000 | 27.197 s | 0.124 s |
| 5,000 | — | 0.247 s |
| 10,000 | did not finish in 600 s | 0.506 s |

Time per character, against the 500-character row: 1.00, 0.70, 0.56, 0.45,
0.46 — every ratio is at or below 1.00, so the curve is sub-linear rather
than super-linear across the whole range. Design §5 clause 3 asked for
linear to within a factor of two as a guard against super-linearity; two of
the five ratios (0.45 and 0.46) are themselves more than a factor of two
*below* 1.00, which is outside that band on the fast side, not the slow one
— sub-linear is the direction clause 3 exists to allow, not the direction it
was watching for, and nothing here exceeds 1.00.

**That curve is sub-linear, and the sub-linearity is not a property of the
algorithm.** Time per character falls from 0.000110 s at 500 characters to
about 0.0000506 s at 10,000, a ratio of 0.46. Part of that is real: fixed
per-invocation cost — process spawn, `mmap`, `Table::new()` — amortises over
a longer input. But part of it is an artefact of the input: the paste files
are built by cycling a small fixed set of gold-corpus sentences up to the
target length, so a longer paste is a *more repetitive* one, and
`candidates`'s `class_cache` — a `HashMap` keyed by `entry_id`, shared
across one call — sees a rising hit rate as the same entries recur more
often as the input grows. What was measured is that the once-per-position
walk is linear or better on this input; a non-repetitive corpus of the same
lengths was not measured, so whether the curve stays this flat off a
repetitive paste is open.

## 4. Standing gates

Run at `2b8258e`: `cargo test -p jparser` (296 passed), the same with
`--features mecab` (322 passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` (279 passed), `npx tsc --noEmit` clean,
`cargo test --manifest-path src-tauri/Cargo.toml` (149 passed).

## 5. Open items

- **The canary asserts no timing.** `a_long_input_parses_in_reasonable_time`
  parses ~4,000 characters and checks the cover; if the blowup returns the
  suite stops finishing rather than failing an assertion. That is deliberate
  — a wall-clock threshold is flaky — but it means a 2× regression would
  pass unnoticed. A timing figure in the harness, compared against a
  recorded baseline, is the stronger version and is not built.
- **10,000 characters still takes seconds, not milliseconds.** The blowup is
  gone; the constant factor is whatever it is. Nothing here measured where
  the remaining time goes — entry assembly and JSON formatting both grow
  with the result, and neither was profiled.
- ~~**`MAX_CHAIN`'s and `MAX_UNMATCHED_RUN`'s adequacy is still unmeasured**
  against the full corpus, as the Phase 2 handoff's "three size/shape
  assumptions" bullet recorded. This phase did not touch either.~~
  **Closed 2026-09-09, after this phase, by sweeping each constant down until
  the output moved.** `MAX_CHAIN`: the corpus needs 4, every value from 4 to
  24 is byte-identical across all four `score` runs, 3 diverges — and the same
  sweep against the full 218,449-entry index over 2,000 characters of real
  text agrees, so the margin is not an artefact of the gold subset's smaller
  lexicon. `MAX_UNMATCHED_RUN`: real text needs 3, every value from 4 to 64 is
  byte-identical and 2 diverges; on a deliberately pathological 200-character
  single-script run every value changes the output instead, which is the
  constant's own doc comment seen directly — an all-one-script gap costs the
  same however it is split, so the hop count is exactly what the bound
  decides. Coverage is invariant, the segmentation of a pathological gap is
  not, and the comment already said so. Both figures now live in the
  constants' doc comments rather than in a handoff.
- ~~**The multiplier this phase set out to measure is still unmeasured.**
  §1 records that the intent was to time `with_readings(3)` at the app's
  10,000-character ceiling and that the k = 1 baseline never finished, so the
  restructure was measured at k = 1 only. The app runs at 3.~~
  **Closed 2026-09-09, after this phase.** `jparser-cli parse` gained
  `--readings`, which it had never had — the app ran at a `k` the CLI could
  not reproduce — and the §2 method was repeated at both values: full
  218,449-entry index built from the `JMdict_e.gz` on disk, paste files of
  gold-corpus sentences cycled to length (365 sentences at the full size,
  reproducing §2's 366), release build, stdout to a file.

  | input chars | k = 1 | k = 3 | ratio |
  | --- | --- | --- | --- |
  | 455 | 0.107 s | 0.121 s | ×1.13 |
  | 975 | 0.134 s | 0.167 s | ×1.25 |
  | 1,973 | 0.183 s | 0.247 s | ×1.35 |
  | 4,960 | 0.300 s | 0.515 s | ×1.72 |
  | 9,987 | **0.501 s** | **0.897 s** | **×1.79** |

  **The k = 1 column reproduces this phase's recorded 0.506 s at 10,000
  characters to within 0.005 s**, which is what makes the k = 3 column
  comparable rather than a fresh measurement on an unvalidated instrument.

  **The multiplier at the ceiling is ×1.79, not the ×12 the pool size would
  suggest.** `RAW_FACTOR` takes the DP's per-position list from 1 to 12 at
  k = 3, but the DP is not where the time goes — `lex::candidates` runs once
  per parse whatever `k` is, and it is still the dominant cost after this
  phase's fix. Hints on, which is the app's own configuration, the ceiling is
  0.879 s median of three runs (0.830 / 0.879 / 0.885) with stdout discarded,
  and 0.920 s with the segment dump written out.

  So the app's worst case is **under one second at its own input ceiling**,
  and the Phase 2b risk recorded as "no latency figure exists anywhere" is
  closed with figures rather than by argument. What the bullet above still
  says stands: 0.9 s is seconds rather than milliseconds, nothing here
  profiled where it goes, and the sub-linearity caveat of §3 applies to both
  columns equally — the paste files are repetitive by construction.
