# Engine v2 Phase 2c — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2c-design.md`](specs/2026-09-09-engine-v2-phase2c-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2c.md`](plans/2026-09-09-engine-v2-phase2c.md)
**Finding:** [`2026-09-09-candidate-generation-blowup.md`](2026-09-09-candidate-generation-blowup.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Three commits.

`7468d6b` (`feat(engine-v2): expose the key FST's forward walk as a
resumable Walk`) adds `Index::walk` and the `Walk` type — a position that
`push` advances one folded character at a time, `records` reads the key
ending there, and `records_after` tests an extension without moving it —
then reimplements `prefixes_of_with` on top of it, unchanged in signature
and behaviour.

`cbd7604` (`docs(engine-v2): restore the walk's root-check and fold
rationale`) restores doc-comment rationale that the first commit's
replacement text had dropped: why the root node's finality is checked at
all, the v1-builder history behind the `key_chars == 0` branch, and why
`fold` is a runtime parameter rather than a constant. It also adds a
pointer from `Walk::records` back to that doc, since the root-finality
check itself moved there.

`9fcab9b` (`fix(engine-v2): walk the index once per position instead of
once per span`) restructures `candidates`: chains are reindexed by stem end
rather than span end, one `index.walk()` runs per start position, each
conjugation class's remove-suffix is tested against the standing walk, and
the key `String` is built only for a span that already matched. No span
bound and no constant is introduced — the walk's own dead end is the bound.

## 2. The gate — byte-identical output

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

All eight cells at `--readings 1` in both hint states, and all three
`oracle@3` figures at `--readings 3 --hints`, match the recorded Phase 2b
values exactly. The candidate set did not move.

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
0.46 — design §5 clause 3 asked for linear to within a factor of two, and
every ratio is within it.

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

Run at `9fcab9b`: `cargo test -p jparser` (296 passed), the same with
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
- **`MAX_CHAIN`'s and `MAX_UNMATCHED_RUN`'s adequacy is still unmeasured**
  against the full corpus, as the Phase 2 handoff's "three size/shape
  assumptions" bullet recorded. This phase did not touch either.
- **The reindex's non-wrapping invariant lost its comment.** The restructure
  in `crates/jparser/src/engine/lex.rs`, around the `chains_by_stem_end`
  reindex loop (`chains_by_stem_end[end - hit.consumed].push(...)`),
  performs the same `end - hit.consumed` subtraction the old loop did, but
  the old code carried an explicit comment noting the subtraction cannot
  wrap because `Table::search` guarantees `consumed <= end`. That comment
  was not carried over. The invariant still holds — the task review
  re-proved it against `Table::search`'s unchanged `len >= end` break — but
  a future reader auditing that line for panic-safety no longer has the
  pointer. Documentation debt, not a correctness gap.
