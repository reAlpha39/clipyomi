# Katakana run integrity — the name that reads "lily eggplant"

**Date:** 2026-09-21
**Branch:** `feat/corpus-disagree`
**Measured in:**
[`2026-09-21-six-corpora-handoff.md`](../2026-09-21-six-corpora-handoff.md),
whose §8 postscript closed the previous defect in this area and whose katakana
figures below were taken after it shipped (`4bc74b4`).

## 1. What it does, and what it deliberately does not

A maximal run of katakana is cut wherever its *pieces* happen to be dictionary
words, whether or not the run is one:

```
ユリナス  ->  ユリ ("lily") | ナス ("eggplant")     1,721x hints-on, 1,722x off
アジュラ  ->  ア | ジュ | ラ                          663x hints-off
```

Both are characters' names. This phase adds one cost term that charges a span
for being a **short piece of a longer katakana run**, so a run whose only
tilings are short pieces loses to leaving the run whole, while a run that
tiles into real words keeps its cut.

**It changes nothing else.** No new constraint, no change to `is_allowed`, no
index format, wire shape, setting or stylesheet, and nothing outside
`crates/jparser/src/engine/`.

Not in scope, stated so the phase cannot quietly grow into them:

- **`イ|って`** (109 hints-on). The katakana run there is one character, so
  there is no run to be inside of. `イく` is absent from JMdict; that is a
  lexicon question, not this one.
- **The `exact`/fold penalty.** `きつく締め付け` parses correctly and
  `キツク締め付け` does not, because the folded candidate pays `INEXACT`. This
  phase may fix `キツク` as a side effect — once the short pieces are charged,
  the whole-run candidate is the only one left — but tuning `INEXACT` is not
  in scope and no gate here depends on that case.
- **Proper-noun recognition.** Nothing learns that ユリナス is a name. The run
  is emitted unglossed, which is the honest output for a word the dictionary
  does not have.
- **The gold corpus's own merges.** Gold splits `タイムリー`|`ヒット` where the
  engine merges; that is the opposite direction and this phase does not touch
  it.

### 1.1 Why this is worth a phase

Measured over the six corpora of the handoff (56,065 sentences), runs of three
or more katakana characters:

| | runs | split | sentences carrying a split run |
| --- | --- | --- | --- |
| hints-on | 25,648 | 2,320 (9.0%) | 3.7% |
| hints-off | 25,648 | 8,050 (31.4%) | 12.6% |

And it is gated, which the other open classes are not. Gold carries **58**
katakana runs of three or more, and the engine misses **7** of their spans
hints-on and **26** hints-off — ニュルンベルク, ゼップ, ブラッター,
リュミエスタ, エルスウェイル, テオドリック and the rest, all names shattered.
Those 26 are part of the 106-span gap between hints-on strict (1694) and
hints-off (1588).

## 2. The rule

A candidate is a **katakana fragment** when both hold:

1. Its span lies strictly inside one maximal katakana run — every character of
   the span is katakana, and the run extends beyond the span on at least one
   side. (A span that *is* the whole run is not a fragment; nor is one that
   starts inside a run and ends outside it.)
2. Its length is below `KATAKANA_MIN_PIECE`, the constant already in
   `cost.rs`, which is 3.

A katakana fragment pays a flat `KATAKANA_FRAGMENT` charge.

"Maximal katakana run" means a maximal span of characters in U+30A1..U+30FA
plus the chouon mark U+30FC — the same class the measurement used.

### 2.1 Why a charge and not a constraint

A constraint — "no piece shorter than three inside a run" — is simpler and is
**refused by the gate in §5**. Gold cuts five of its 58 runs and two of those
cuts leave a short piece: `ソロ` in ソロホームラン and `ラン` in ラストラン.
The engine gets both right today, in both hint states. A constraint deletes
those tilings and costs four gold spans by construction.

A charge does not, because the alternative to a split is leaving the run
unmatched, and `unmatched_cost` is `UNMATCHED` (30) **per character**. The
whole-run alternative therefore gets more expensive the longer the run is,
while the charge does not:

| run | unmatched whole | split + charge | outcome |
| --- | --- | --- | --- |
| ユリナス (4, two 2-char pieces) | 120 | ~16 + 2P | merges once P ≳ 52 |
| アジュラ (4, pieces 1/2/1) | 120 | ~16 + 3P | merges once P ≳ 35 |
| ラストラン (5, pieces 3/2) | 150 | ~16 + 1P | stays split below P ≈ 134 |
| ソロホームラン (7, pieces 2/5) | 210 | ~16 + 1P | stays split below P ≈ 190 |

The split figures are approximations — they assume two COMMON inexact matches
and ignore `LEN_GAIN`, which applies hints-on only — so read the window as
roughly **P ∈ [52, 134]** and not to the unit. §4 measures it rather than
trusting this table.

### 2.2 Why the piece length is the right signal

It is the signal the corpus separates on, and `cost.rs` found the same thing
on a different corpus when `KATAKANA_MIN_PIECE` was written: of 1,110 runs
there, the eight shattered real words all left a piece shorter than three and
all 63 correct decompositions had both pieces at three or more.

Over the six corpora, split runs by their shortest piece:

| shortest piece | hints-on | hints-off |
| --- | --- | --- |
| 1 character | 46 | 4,558 |
| 2 characters | 1,971 | 3,015 |
| **under 3 — the defect** | **2,017 (87%)** | **7,573 (94%)** |
| 3 or more — real compounds | 303 | 477 |

The ≥3 rows are `チンポ|ミルク`, `ビクン|ビクン`, `ランニング|ウェア`,
`オッパイ|ビンタ` — correct decompositions, every one, and none of them is
charged under §2.

## 3. Where it goes

- **`engine/lex.rs`** — one more `Candidate` field, `katakana_fragment`, set in
  `candidates()` where `all_katakana`, `digit_counter` and
  `after_digit_noncounter` are already set from `chars`. The maximal run around
  each position is computed once per sentence, O(n), not per candidate.
- **`engine/cost.rs`** — `const KATAKANA_FRAGMENT: i32` with the doc comment
  this file's constants carry, and one `if` in `cost()` beside the other
  candidate-property terms.

Nothing else. `is_allowed`, `lattice`, `assemble` and the index are untouched,
so no install rebuilds and no snapshot of the wire shape moves.

## 4. Choosing the magnitude

Swept, not chosen. For each candidate value the binary is rebuilt and measured
on, in this order, stopping at the first failure:

1. **The four strings of §2.1**, parsed directly — a two-second pre-check that
   rejects a value before anything slower runs.
2. **Gold, both hint states** — `score` with `--compounds`. Any cell below
   baseline rejects the value (§5).
3. **The 26 hints-off katakana gold misses** — how many come back.
4. **The corpora**, two of the six for speed, re-measuring the table in §2.2:
   the under-3 rows should collapse and the ≥3 rows should not move.

The value that ships is the one with zero gold damage and the most misses
recovered; ties break **low**, because a smaller charge disturbs fewer
tilings that no gate here can see. If no value in the window clears §5, the
phase is refused and says so — the same way `LEN_GAIN` and the residue term
were refused.

## 5. Gates

Baseline, at `4bc74b4`:

| | hints-on | hints-off |
| --- | --- | --- |
| span strict | 1694 | 1588 |
| span tolerant | 1948 | 1785 |
| entry top-1 | 0.9527 | 0.9504 |
| compounds | 152 | 118 |

**Zero gold damage in either state**: no cell above may fall. This is the
gate the class needs rather than the gate it deserves — gold cannot show the
hints-on win at all, because its own katakana misses hints-on are merges it
wants split. A change that pays nothing on gold and costs nothing there is one
a later session can re-check without this session's corpus.

**Must hold**: `spurious_merges.txt` 26 of 26, `auxiliary_merges.txt` all rows,
`cargo test --workspace --features mecab` with no failures, `cargo fmt` and
`cargo clippy --workspace --all-targets --features mecab` clean, and
`parse_sentences.txt` byte-identical — its only katakana runs of three or more
are コーヒー and パソコン, both already matched whole in both states, so a
change there means the rule is reaching further than §2 says.

**Must move**: at least 20 of the 26 hints-off katakana gold spans, and the
under-3 corpus rows in §2.2 by at least an order of magnitude.

**Latency**: `jparser-cli bench`, interleaved A/B against the same baseline
binary, since the run scan is O(n) per sentence and should not be visible.
Record the ratio, not the milliseconds.

## 6. Tests

A private fixture in `tests/engine_v2.rs`, in the shape `KURU_COMPOUND_FIXTURE`
already uses, carrying ユリ, ナス, ソロ, ホームラン, ラスト, ラン and one
all-pieces-≥3 compound:

1. `ユリナス` is one unmatched span, not ユリ + ナス.
2. `ソロホームラン` keeps its cut — the long-run case the charge must not
   reach, and the reason this is a cost rather than a constraint.
3. `ラストラン` keeps its cut — same, at the window's other end.
4. A compound whose pieces are both ≥3 is untouched, and its candidates carry
   no charge.
5. A span that starts inside a run and ends outside it is not a fragment —
   the `キツい` shape, asserted on the flag rather than on the parse, so it
   cannot pass by accident of ranking.

Tests 2 and 3 are the ones that fail if the swept magnitude is too high, so
they double as the sweep's own regression check.

## 7. Open items

- **The magnitude's window is derived from approximate costs** (§2.1). If the
  sweep finds the window empty, that is a finding about the cost model, not a
  reason to loosen §5.
- **Runs whose interior cuts are all ≥3 but whose tiling still shatters a
  word** are not reached by this rule. None was found in the six corpora; the
  measurement says only that the predicate separates what is there.
- **`アジュラ` is whole hints-on in isolation and shattered in context.** The
  sweep's pre-check must therefore use corpus sentences, not bare strings, for
  anything it claims about hints-on.
- **The hints-off column is a draining population** (four-corpora handoff
  §11), so the gate that scores this phase measures the state fewest users
  will be in. The hints-on win is real and corpus-only; whoever ships this
  should say so in the commit rather than quoting the gold delta alone.
