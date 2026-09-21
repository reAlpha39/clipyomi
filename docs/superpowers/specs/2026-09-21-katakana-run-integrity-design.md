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

- **`engine/lex.rs`** — one more `Candidate` field, `inside_katakana_run`, set in
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

## 8. Measured 2026-09-21: the sweep, and refused

Six values — 40, 50, 60, 80, 100, 130 — were built and measured against §5's
protocol. Baseline (the charge absent, `KATAKANA_FRAGMENT = 0`) was
re-confirmed to match §5 exactly before the sweep ran: on strict 1694/2163,
tolerant 1948/2163, entry top-1 1188/1247 (0.9527), compounds 152/186; off
strict 1588/2163, tolerant 1785/2163, entry top-1 1169/1230 (0.9504),
compounds 118/186.

### 8.1 Gold, all eight cells

`↓` marks a cell that fell below baseline — §5's gate.

| value | on strict | on tolerant | on entry top-1 | on compounds | off strict | off tolerant | off entry top-1 | off compounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 1694 | 1948 | 0.9527 (1188/1247) | 152 | 1588 | 1785 | 0.9504 (1169/1230) | 118 |
| 40 | 1692 ↓ | 1948 | 0.9526 ↓ (1186/1245) | 153 | 1582 ↓ | 1783 ↓ | 0.9501 ↓ (1161/1222) | 120 |
| 50 | 1692 ↓ | 1948 | 0.9526 ↓ (1186/1245) | 153 | 1582 ↓ | 1783 ↓ | 0.9501 ↓ (1161/1222) | 120 |
| 60 | 1692 ↓ | 1948 | 0.9526 ↓ (1186/1245) | 153 | 1590 | 1791 | 0.9493 ↓ (1160/1222) | 120 |
| 80 | 1692 ↓ | 1948 | 0.9518 ↓ (1185/1245) | 153 | 1590 | 1791 | 0.9493 ↓ (1160/1222) | 120 |
| 100 | 1692 ↓ | 1948 | 0.9518 ↓ (1185/1245) | 153 | 1590 | 1791 | 0.9493 ↓ (1160/1222) | 120 |
| 130 | 1692 ↓ | 1948 | 0.9518 ↓ (1185/1245) | 153 | 1590 | 1791 | 0.9493 ↓ (1160/1222) | 120 |

Every value falls on-strict and on-entry-top-1. 60 and above additionally
recover off-strict and off-tolerant but still fall off-entry-top-1; 40 and 50
fall off-strict and off-tolerant too, on top of the two cells every value
loses. Nothing swept holds all eight.

### 8.2 Tests, katakana misses, and the corpus

| value | `katakana`-filtered tests | the 2 tests outside that filter | on misses (baseline 7) | off misses (of 26) | 2-corpus on: whole / short / ≥3 | 2-corpus off: whole / short / ≥3 |
| --- | --- | --- | --- | --- | --- | --- |
| baseline | — | — | 7 | 26 | 9094 / 1918 / 79 | 6830 / 4148 / 113 |
| 40 | 5/6 — `a_katakana_name_is_one_run_not_two_words` fails (too low, per §6, not a stop) | pass, pass | 8 | 27 | 9095 / 1915 / 81 | 7390 / 3578 / 123 |
| 50 | 5/6 — same failure | pass, pass | 8 | 27 | 9095 / 1915 / 81 | 7400 / 3568 / 123 |
| 60 | 6/6 | pass, pass | 8 | 19 | 10843 / 167 / 81 | 9276 / 1651 / 164 |
| 80 | 6/6 | pass, pass | 8 | 19 | 10843 / 167 / 81 | 9276 / 1651 / 164 |
| 100 | 6/6 | pass, pass | 8 | 19 | 10843 / 167 / 81 | 9276 / 1651 / 164 |
| 130 | 6/6 | pass, pass | 8 | 19 | 10843 / 167 / 81 | 9276 / 1651 / 164 |

40 and 50 recover none of the 26 hints-off katakana misses — the short-piece
corpus row barely moves (1918 → 1915, 4148 → 3568/3578) because the charge is
below the ユリナス end of §2.1's window and does not flip enough tilings to
matter. 60 through 130 are identical on every cell of *this* table — tests,
katakana misses and the corpus counts — once the charge clears whatever local
margin each of these decisions needs, more of it changes nothing further
here, all the way to the window's own upper edge. (§8.1's gold table does not
share that pattern: on-entry-top-1 keeps falling between 60 and 80, and §8.4
relies on exactly that difference.) They recover 7 of the 26 — short of §5's
"at least 20" bar on its own, before the gold gate is even considered.

**The new casualty, every value.** `ラスト` is the gold span at (32, 3) in
`b:wn21710-1`, half of the `ラスト` + `ラン` pair gold annotates inside that
sentence's `ラストラン`. It is matched at baseline (gold's own split) and
missed at every swept value, hints-on and hints-off alike, because the engine
returns the whole run as one span instead. It is the reason on-strict and
on-entry-top-1 fall even at the lowest value tried; nothing in this sweep
found a magnitude below it. §8.3 traces why.

**A row that was expected not to move, moved.** §2.2's ≥3 ("word-sized") row
was expected to hold, and at 60 and above it does not: 79 → 81 hints-on,
113 → 164 hints-off (this table's two-corpus subset). The mechanism is
visible in the whole-run row moving in step (9094 → 10843 hints-on,
6830 → 9276 hints-off): a run that used to split at a short piece does not
only fall back to unmatched when the charge blocks that split — a few runs
find a *different*, all-≥3 tiling instead, and get reclassified into the row
the charge should not have been able to touch, since nothing under 3
characters is ever a candidate there.

For an isolated run the class boundary in §2.2 is in fact clean: an all-≥3
tiling pays no new term and neither does the whole-run candidate, so a run
already in that row cannot leave it — the row can only gain. The six-corpus
counts (handoff §9.3) show exactly that, each summing to zero: hints-on
−1807 (short-piece) / +1783 (whole) / +24 (word-sized), hints-off −4927 /
+4836 / +91. The remaining caveat is that the Viterbi search is global, so a
span crossing a run boundary elsewhere in the sentence can perturb a run
indirectly — that is why the row moved by 24 (91 hints-off) rather than 0.
It is a second-order effect and not itself a gate failure, since §5 gates
gold, not this row.

### 8.3 The cause: `ラストラン` is a dictionary entry, and §2.1's table did not know

The whole of the hints-on gold damage traces to one sentence, `b:wn21710-1`:

> 産経新聞によると、この日、東京駅のホームには、東京-新大阪間でのラストランを
> 見る目的で、多くの鉄道ファンらが集まった。

Gold annotates two spans there, `ラスト` (start 32, len 3) and `ラン` (start
35, len 2). At baseline the engine agrees and returns the same two spans. At
every swept value — 40 included, the lowest tried — and in both hint states,
the engine instead returns `ラストラン` (start 32, len 5) as one span,
matched to entry 2843587. That single substitution, confirmed directly from
the `gold.on.jsonl` and `gold.off.jsonl` dumps in each of `$SCR/v40` through
`$SCR/v130`, is the entire −2 on hints-on strict span accuracy and the
matching −2/−2 on hints-on entry top-1 in §8.1's table; it is the only thing
the `ラスト` casualty named in §8.2 is.

The cause is an error in §2.1's table, not a defect in the charge. That
table prices ラストラン's whole-run alternative as `unmatched whole = 150`
(`UNMATCHED` × 5 characters) and concludes the split "stays split below
P ≈ 134" — but `crates/jparser/tests/fixtures/gold_jmdict_subset.xml:136897`
carries an entry whose `<reb>` is `ラストラン` (ent_seq 2843587, gloss "last
run (of a train being retired)"): the compound is itself a dictionary entry.
The whole-run candidate is therefore a cheap **matched** span, not a
150-point unmatched fallback, and any charge from 40 up — the lowest value
this sweep tried — makes it cheaper than the split. The window §2.1 derived
never had a lower edge for this particular run; the table's arithmetic was
right for a run the dictionary does not cover and wrong for one it does.

Task 2's fixture test, `a_five_character_katakana_compound_keeps_its_cut`,
cannot see this: `KATAKANA_FIXTURE` defines `ラスト` and `ラン` as entries but
never defines `ラストラン`, so inside the fixture the whole run really is
unmatched at 150 and the test passes at 60 exactly as §2.1 predicted. The
test is correct about the fixture and silent about the installed index — a
later session must not read its passing as evidence that this particular run
is safe against the real dictionary.

Read plainly: on this one sentence, gold and JMdict disagree about whether
`ラストラン` is one word or two, and the charge moves the engine to JMdict's
side. That is a data disagreement, not a bug in the implementation, and it is
not a reason to loosen §5 — the refusal in §8.4 stands regardless of which
side is "right" about this sentence, because the gate judges the whole
corpus, not one case.

### 8.4 The refusal

No value clears §5. Every one of the six loses hints-on strict span accuracy
and hints-on entry top-1 to the `ラスト`/`ラストラン` disagreement traced in
§8.3; 60 and above also lose hints-off entry top-1, and 40/50 lose hints-off
strict and tolerant besides. Even the two gold cells that do recover at 60
and above — off-strict and off-tolerant — plus the 7 of 26 katakana misses
recovered alongside them (a separate quantity, not a gold cell) fall short of
the "at least 20" bar §5 sets for the phase to have done its job. §4 named
this outcome in advance: *"If no value in the window clears §5, the phase is
refused and says so — the same way `LEN_GAIN` and the residue term were
refused."* This is that outcome.

`KATAKANA_FRAGMENT` is left at **60**, the least damaged of the six swept
values (three gold cells fall, against five at 40 and 50, and a smaller
entry-top-1 drop than 80/100/130's), but as an unvalidated placeholder, not a
shipped magnitude — see `cost.rs`'s doc comment. 80, 100 and 130 were not
chosen over 60 because they are identical to it on every cell above except a
strictly worse on-entry-top-1 (0.9518 against 0.9526); 40 and 50 were not
chosen because they recover nothing while damaging two more cells than 60
does. Tasks 1 and 2's mechanism — the `inside_katakana_run` marking and the
cost term itself — is unaffected; what is refused is trusting any value in
the §2.1 window to fix the hints-off katakana defect without a cost this
gate can see.

**The damage and benefit thresholds are ordered the wrong way round, so no
finer sweep can help.** The `ラストラン` damage (§8.3) is already present at
40, the lowest value tried, while nothing useful moves until above 50: at 50
the two-corpus short-piece row (§8.2) goes 1918 → 1915 and **zero** of the 26
hints-off katakana misses return; at 60 it goes to 167 and 7 return. There is
no magnitude between a live defect and a dead charge — both halves are in
§8.1/§8.2's tables above, but the ordering is worth stating plainly, because
without it the record invites a re-sweep between 40 and 60 that cannot
succeed.

The fixture test `a_katakana_name_is_one_run_not_two_words`
(`crates/jparser/tests/engine_v2.rs`) pins a floor under this from the test
suite's side: under `KATAKANA_FIXTURE`'s arithmetic it passes only when
`KATAKANA_FRAGMENT >= 55`. Reverting the charge to 0 is therefore not a
one-token edit — that test must be deleted or inverted as part of the
revert.

**Reverted 2026-09-22.** The user decided the trade above was not worth the
three gold cells it costs and reverted the charge rather than shipping it.
The implementation this section describes — `KATAKANA_FRAGMENT`, its `if` in
`cost()`, its unit test, and the four parse tests in `engine_v2.rs` — is
recoverable from `4007a45`, the commit that added it, and the sweep and
measurement it failed are unchanged above. Task 1's geometry (`kana.rs`'s
`is_katakana`, `lex.rs`'s `katakana_runs` and the `inside_katakana_run` field
and its assignment) was not part of the revert and remains in the tree, so a
later attempt at this problem starts from the geometry rather than from
nothing. Gold returns to baseline: on-strict 1692 → 1694, on-entry-top-1
0.9526 → 0.9527, off-entry-top-1 0.9493 → 0.9504.

### 8.5 Interaction with `PRONOUNCE_CROSS`'s `KATAKANA_MIN_PIECE` exemption

`cost.rs`'s older `KATAKANA_MIN_PIECE` exemption (in the `PRONOUNCE_CROSS`
filter chain) and `KATAKANA_FRAGMENT` do not double-charge — the exemption
withholds a cost from a *merged* PRONOUNCE-only span, and the new charge
prices the *split* pieces, so they never land on the same candidate. But
both fire on the same condition, a piece under three characters inside an
all-katakana span, and both push toward the merge. Hints-on, a short-piece
run gets the withheld `PRONOUNCE_CROSS` on the merge *and* the new charge on
the split pieces, stacking; hints-off, with no tokenizer boundaries to
exempt anything from, only the new charge applies. §2.1's window was derived
ignoring `PRONOUNCE_CROSS` entirely, so it describes the hints-off state
only — which is a likely explanation for why the measured collapse is so
asymmetric (split-with-short-piece falls 9.6x hints-on against 2.9x
hints-off, handoff §9.3), though nobody has isolated the two effects to
measure this directly.
