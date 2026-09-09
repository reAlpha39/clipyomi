# Candidate generation is superlinear in input length

**Date:** 2026-09-09
**Found:** while measuring the app-level latency the Phase 2b design left
unmeasured (its handoff §6, and the final review's pre-merge note)
**Status:** diagnosed and reproduced, fix shape proven, not fixed
**Severity:** the app's own input ceiling is unusable. A 10,000-character
clipboard paste — the maximum `src-tauri` accepts — does not return.

## 1. What was being measured, and what turned up instead

Phase 2b moved the app from `ParseOptions::default()` to
`with_readings(3)`, taking the DP's per-position list from 1 to 12, and no
latency figure existed anywhere in Phase 2's or 2b's records. The intent was
to time that multiplier at the input ceiling.

The k = 1 baseline never finished. The multiplier is not the problem;
`lex::candidates` is, and it has been since long before Phase 2.

## 2. Measurements

Release build. Full JMdict index of **218,449 entries**, built from the
`JMdict_e.gz` already on disk under `%APPDATA%\dev.jparser.clipyomi\source`
(the installed `dict/gen-1` is format 3 and no longer opens). Input is a
realistic paste: gold-corpus sentences concatenated to length, 366 of them at
the full size. Hints off, `jparser-cli parse`, so **k = 1** — Phase 2b's
change is not in play at all.

| input chars | time | ratio to previous |
| --- | --- | --- |
| 125 | 0.096 s | — |
| 250 | 0.213 s | ×2.2 |
| 500 | 0.763 s | ×3.6 |
| 1,000 | 4.284 s | ×5.6 |
| 2,000 | 27.197 s | ×6.3 |
| 10,000 | **did not finish in 600 s** | — |

A doubling of input costs ×6 and rising, so this is worse than quadratic —
roughly `n^2.6` over the measured range. Extrapolating the trend puts 10,000
characters near half an hour, consistent with zero of ten runs completing
inside ten minutes.

The parse runs in `spawn_blocking` and its result arrives by event, so the
window does not freeze. It simply never shows a result.

## 3. Cause

`crates/jparser/src/engine/lex.rs:103`:

```rust
for start in 0..len {
    for end in (start + 1..=len).rev() {
        for hit in &chains_by_end[end - 1] {
            let stem_end = end - hit.consumed;
            if stem_end < start { continue; }
            let stem: String = chars[start..stem_end].iter().collect();
            ...
            for prefix_hit in index.prefixes_of_with(&key, kana::fold)? {
```

`end` runs to the end of the **input**, not to the end of the longest span
that could possibly match a dictionary key. So the loop visits every one of
the `O(n²)` `(start, end)` pairs, and for each one it materialises a `String`
of up to `n` characters and hands it to the FST — a lookup that, for any span
longer than the longest dictionary key, can only fail. The `String` build is
what makes it worse than quadratic; the FST itself bails after a few
characters.

**The longest key bounds the useful work, and it is small.** Measured over
`JMdict_e.gz`, 498,294 keys: the longest is 37 characters (a reading),
5 exceed 32, none exceeds 48. `MAX_CHAIN` is 12, and `end = stem_end +
hit.consumed` with `consumed ≤ MAX_CHAIN`, so no span wider than
`37 + 12 = 49` characters can contribute a candidate. Every pair past that is
pure waste, and at 10,000 characters almost every pair is past it.

## 4. The fix shape, proven

Bounding the `end` loop at a width above the true 49 was applied as a
throwaway experiment and measured, then reverted:

```rust
for end in (start + 1..=(start + 64).min(len)).rev() {
```

| input chars | unbounded | 64-char bound |
| --- | --- | --- |
| 500 | 0.763 s | 0.493 s |
| 1,000 | 4.284 s | 1.136 s |
| 2,000 | 27.197 s | 2.390 s |
| 5,000 | — | 6.916 s |
| 10,000 | did not finish | **15.040 s** |

Roughly linear, and the input ceiling returns in fifteen seconds instead of
never. **The eight Phase 1b-i baseline cells are byte-identical** under the
experiment, in both hint states — as the derivation predicts, since 64 is
above the 49 any candidate could need.

Two things this does *not* claim. Fifteen seconds for a 10,000-character
paste is still slow; the bound removes the blowup, not the constant factor,
and the residual superlinearity at the top of that column is unexplained
(output assembly and JSON formatting both grow with the result). And 64 is a
guessed constant, not a derived one — see below.

## 5. Where the bound should come from

The experiment's hardcoded 64 is not shippable: it is correct for today's
JMdict by four characters of margin, and if a future release adds a 53-character
reading it silently stops finding it. A dropped candidate has no alarm.

Four options, in rough order of increasing cost:

1. **A constant, plus a check where the index is built** that no key exceeds
   it. Cheapest. Turns a silent cliff into a loud one, but a failing
   dictionary build is its own problem, and the check has to live where
   `jmdict-source` can reach it.
2. **Scan the FST once at `Index::open`** for the longest key and keep it on
   `Index`. Exact, no format change, no cliff. Costs one pass over ~500,000
   keys per open — negligible for the app, less so for a test suite that
   opens indexes constantly.
3. **Store it in the index header at build time.** Exact and free at open,
   but `header.bin` is a fixed 24 bytes, so this is a format bump and every
   existing index is invalidated. Format 3 → 4 already happened once.
4. **Restructure `candidates` as a forward trie walk.** Instead of
   enumerating spans and building a key per span, walk the FST forward from
   `start` and let it terminate where no edge matches. The dictionary's own
   content bounds the span exactly, with no constant anywhere and no wasted
   `String`. This is the shape the data structure is for, and it also
   removes the per-pair allocation the current loop pays. It is a rewrite of
   the function, and the deinflection chains make it harder than it sounds:
   `chains_by_end` is indexed by where a span *ends*, which is what the
   current loop's shape exists to serve.

Option 4 is the right answer and the expensive one. Option 2 is the smallest
change with no correctness cliff.

## 6. Why this was not caught

Nothing in the harness measures a long input. The gold corpus is 150
sentences with a median of 14 spans; the longest is nowhere near a paste. The
eight cells, both oracle figures and every standing gate run in well under a
minute, because per-sentence cost at corpus lengths is genuinely fine — the
term that dominates only appears when a single input is thousands of
characters long, which is exactly the case the clipboard produces and the
corpus never does.

A single long-input timing, anywhere in the standing gates, would have caught
it. That is the cheapest thing to add alongside the fix.
