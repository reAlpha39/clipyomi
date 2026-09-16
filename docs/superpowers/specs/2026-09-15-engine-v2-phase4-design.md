# Engine v2 Phase 4 — the hint constraint's escape

**Date:** 2026-09-15
**Branch:** `feat/engine-v2`
**Answers:** the five characterization tests committed 2026-09-15
(`4a8a24f`, `52c6513`, `4db0b2e`, `7ca8168`), all pinning today's behaviour
so that a fix fails them.
**Takes the Phase 4 slot**, which the master plan had held open for compound
preservation until
[the long-unit spike](../2026-09-15-long-unit-separation-spike.md) closed that
route on 2026-09-15.

## 1. What it does, and what it deliberately does not

`cost::is_allowed` forbids a span from starting or ending strictly inside a
tokenizer token. That rule is right and this phase keeps it. What it lacks is
an escape for the case where it forbids *everything*: when no candidate can
cover a token, the rule leaves the token's characters with no legal match at
all, and they fall to unmatched.

**This phase adds that escape and changes nothing else.** A position strictly
inside a token that no candidate can cover becomes usable as a span endpoint.
Every other position behaves exactly as it does today, so wherever the hint
source and the dictionary agree about where words are — which is most of the
gold corpus — the parse does not move.

Not in scope, stated so the phase cannot quietly grow into them:

- **The ranking defects.** `ミスった` splitting into `ミス` + `った`, and
  `ミスる` losing to `ミス` + `る` without hints (`52c6513`), are cost-model
  ordering among *allowed* spans. This phase changes which spans are allowed
  and touches no constant. Those two tests are expected to stay green, and if
  either moves, that is a finding to explain rather than a bonus.
- **Turning the constraint into a price.** §6 records why not.
- **The dictionary gaps.** `ていう` has no key; no segmentation change reaches
  it (`4a8a24f`).
- **Any new data, index format, wire shape, setting or stylesheet.** Nothing
  outside `crates/jparser/src/engine/` changes, and no install rebuilds.

### 1.1 Why this is worth a phase

Measured 2026-09-15 across the eighteen sentences reported from real use,
against the real 218,449-entry index:

| | characters | matched, hints on | matched, hints off |
| --- | --- | --- | --- |
| eighteen sentences | 630 | **426** | **487** |

Nine of the eighteen lose ground with hints on and **not one gains**. On the
gold corpus the same hints take strict span accuracy 0.7342 → 0.8174, so the
two registers disagree about the sign. The difference is vocabulary: news and
literature are in IPADIC, a visual novel's dialogue is not, and where IPADIC
holds a unit JMdict lacks the constraint turns evidence into a veto.

The smallest case is three characters. IPADIC holds `本当は` as one entry and
JMdict does not hold it as a key, so `本当` ends inside the token, `は` starts
inside it, and both are forbidden while nothing is long enough to cover the
token. Hints off the same index returns `本当` (ほんとう) and `は`, both
matched. Hints on, **both unmatched, with the boundaries still correct** — so
nothing downstream looks wrong and the reader gets a phrase with no
definitions under it.

## 2. The rule

Today, for a candidate `c` and boundary set `B`:

```text
allowed(c)  ≡  B.is_boundary(c.start) ∧ B.is_boundary(c.start + c.len)
```

This phase replaces `is_boundary` in *that* test — and only there — with a
usability test:

```text
usable(p)   ≡  B.is_boundary(p) ∨ inside_an_unmatchable_token(p)
allowed(c)  ≡  usable(c.start) ∧ usable(c.start + c.len)
```

A token `T = [a, b)` is **matchable** when some candidate covers it whole
between two real boundaries:

```text
matchable(T) ≡ ∃ candidate c :  B.is_boundary(c.start)
                              ∧ B.is_boundary(c.start + c.len)
                              ∧ c.start ≤ a  ∧  c.start + c.len ≥ b
```

and **unmatchable** otherwise. `inside_an_unmatchable_token(p)` holds when `p`
is strictly interior to some unmatchable token.

Three properties of that definition, each load-bearing:

1. **A single covering candidate is the only way.** A token has no interior
   boundaries by construction, so under today's rule every allowed span either
   covers a token wholly or does not touch it. There is no chain of allowed
   spans that tiles a token, and the definition does not have to look for one.
2. **Coverage is computed under the strict rule**, not the relaxed one, so the
   definition is not circular and one pass settles it.
3. **The escape is per position, not per span.** `気持ち良かったし` needs
   `良かった` to start inside the token `気持ち良かっ` and end on the real
   boundary after `た` — a span that lies *within* the token would not do. So
   the relaxation marks positions, and a span may have one endpoint relaxed
   and the other strict.

### 2.1 The rule on the pinned cases

| input | token | matchable? | what the escape permits |
| --- | --- | --- | --- |
| `本当は` | `本当は` | no — longest key is `本当` | `本当` ends at 2, `は` starts at 2 |
| `気持ち良かったし` | `気持ち良かっ` | no — longest key is `気持ち` | `気持ち` ends at 3, `良かった` starts at 3 and ends on a real boundary |
| `どっぴゅんさせてあげるから` | `ぴゅんさせてあげるから` | no | `どっぴゅん` ends at 5, `させて`, `あげる`, `から` follow |
| `くるまではこう` | every token | yes | nothing — unchanged, and `kurumadehakou_parses_as_four_words` says so |

The last row is the point of the design. Where the tokenizer and the
dictionary agree, no position is relaxed and the parse is bit-identical.

## 3. Where it goes

### 3.1 `Boundaries` gains a defaulted method

```rust
pub trait Boundaries {
    fn is_boundary(&self, pos: usize) -> bool;

    /// Whether a span may start or end at `pos`. Defaults to `is_boundary`,
    /// so every existing implementation keeps today's behaviour.
    fn is_usable_endpoint(&self, pos: usize) -> bool {
        self.is_boundary(pos)
    }
}
```

`cost::is_allowed` calls `is_usable_endpoint`; `cost::cost` keeps
`is_boundary` for its `CROSS` and `PRONOUNCE_CROSS` crossing count. **Those
two must not be conflated:** `CROSS` is 0 today so the difference is currently
invisible, but `PRONOUNCE_CROSS` is 25 per crossing, and feeding a widened
boundary set to `cost` would reprice every PRONOUNCE-only match for a reason
that has nothing to do with this phase.

A defaulted method is what keeps the diff small: every `Boundaries`
implementation in the tree and in the tests compiles unchanged, `k_best` keeps
its signature, and its several dozen call sites are untouched.

### 3.2 One new module, `engine::relax`

```rust
/// A `Boundaries` that additionally allows span endpoints strictly inside
/// any token the candidate set cannot cover.
pub struct Relaxed<'a> { inner: &'a dyn Boundaries, usable: Vec<bool> }

impl<'a> Relaxed<'a> {
    pub fn over(n: usize, by_start: &[Vec<Candidate>], inner: &'a dyn Boundaries) -> Self
}
```

`over` is one left-to-right pass:

1. Walk `0..=n` asking `inner.is_boundary`, which yields the token edges.
2. For each boundary `i`, take `reach[i]` = the greatest `i + c.len` over
   candidates `c` in `by_start[i]` whose end is also a boundary.
3. Carry a running maximum of `reach` over boundaries seen so far. A token
   `[a, b)` is matchable exactly when that maximum at `a` is `≥ b`.
4. For each unmatchable token, mark its interior positions usable.

`O(n + |candidates|)` time and `O(n)` space, against a parse that already
walks every candidate several times. The 10,000-character bench cell is the
guard, not an argument (§5).

### 3.3 `engine::parse` wires it

```rust
let by_start = lex::candidates(index, table, &chars)?;
let relaxed = bounds.map(|b| relax::Relaxed::over(chars.len(), &by_start, b));
let bounds = relaxed.as_ref().map(|r| r as &dyn Boundaries).or(bounds);
let mut paths = lattice::k_best(&chars, &by_start, bounds, opts.readings);
```

Three lines in `engine/mod.rs`. `bounds == None` stays `None`, so the
no-tokenizer path is untouched by construction.

## 4. What this will move, and what must not

### 4.1 Expected to move

The five characterization tests. Three of them must go from pinning the defect
to pinning the fix, and their doc comments say so already:

- `a_hint_token_longer_than_any_key_leaves_its_region_unmatched` →
  `気持ち` + `良かった` + `し`.
- `an_unknown_word_run_swallows_every_word_after_it` → `どっぴゅん` unmatched,
  `させて` + `あげる` + `から` matched.
- `a_hint_token_that_is_not_a_key_unmatches_two_common_words` → `本当` +
  `は`, both matched.

Rewriting a test whose comment says "when that lands, this test fails" is the
intended flow, not a gate failure. Each rewrite keeps the hints-off half
untouched, because the hints-off answer is what the hints-on answer should
become.

### 4.2 Expected not to move

- **`a_reading_the_index_lacks_is_unmatched_between_correct_boundaries`.**
  `ていう` is a missing key; if this test changes, the escape is doing
  something it was not designed to do.
- **The two ranking tests** (`52c6513`), for the reason §1 gives.
- **`kurumadehakou_parses_as_four_words`**, the master plan's success
  criterion 3 — every token there is matchable, so no position is relaxed.
- **`v2_snapshots_every_sentence`**, which runs hints-off.

### 4.3 The gate that could genuinely fail

`the_particle_absorbing_merges_are_gone` — the spurious-merge gate, 26 rows,
green since it was repointed at gold sentences. The escape only ever *adds*
allowed spans, and an absorbing merge that happens to lie inside an
unmatchable token would become legal where it is structurally impossible
today. That is the one place this phase can regress a class it was not aiming
at, and it is a gate rather than a metric: **26 of 26 or the phase does not
ship in this form.**

If it fails, the narrowing to try first is requiring the relaxed span to be
*matched* and to reach at least one real boundary, which the absorbing merges
by construction do not.

## 5. Gates

The Phase 3c set, unchanged, plus the eighteen-sentence measurement this phase
exists for.

1. **The eight baseline cells**, both hint states, gold index rebuilt from
   `gold_jmdict_subset.xml` (5,402 entries, 9,930 keys, 13,898 records).
   Hints-off must be **identical to the digit** — no position is relaxed when
   `bounds` is `None`, so any movement there is a bug in the wiring rather
   than a result. Hints-on is expected to move up and is recorded, not
   thresholded, with one exception: **strict span accuracy must not fall**
   below 0.8174. The escape adds matches where there were none, so a fall
   would mean it is also taking matches away.
2. **The compound metric**, hints on, `114 / 186` today. Recorded. A rise is
   plausible — a compound inside an unmatchable token is unreachable now —
   and a fall needs explaining.
3. **The spurious-merge gate**, 26 of 26. §4.3.
4. **`v2_snapshots_every_sentence`**, hints off, byte-identical.
5. **The bench scaling cell**, both hint states, read as a band as Phases 3b
   and 3c read it: hints off 0.98x–1.07x, hints on 1.07x–1.12x. §3.2's pass is
   linear, but "linear" is an argument and the cell is a measurement.
6. **The eighteen reported sentences**, matched characters hints-on against
   hints-off, today 426 against 487. The phase's own claim is that the gap
   closes; the figure is recorded rather than thresholded, because the
   remainder is dictionary gaps and ranking, neither of which this phase
   touches.
7. `cargo test -p jparser --features mecab` green, 0 ignored;
   `clippy --all-targets --features mecab` and `fmt` clean.

## 6. Why not the simpler change

The lazier fix is to delete the boundary test from `is_allowed` and price it
in `cost` instead — a large penalty for starting or ending inside a token,
letting the DP shatter only when nothing better exists. It is a much smaller
diff and it matches §4.3's own framing, that hints are evidence rather than
law.

**It is not proposed, for three reasons.** It moves every cell in both hint
states rather than only the cases that are broken today, so the gold-corpus
gates stop being a guard and become a negotiation. It reopens the class
`is_allowed` was written to close — the spurious merges are currently
impossible by construction, and under a price they become merely expensive,
with the magnitude to be swept the way Task 16b swept `INEXACT` and `COMMON`.
And the magnitude has no natural value: the defect this phase fixes is
*structural absence* of any legal span, which a price cannot express, so a
penalty large enough to preserve today's behaviour is also large enough to
leave `本当は` broken.

The escape is the larger diff and the smaller change. If it fails its gates,
the price version is where to go next, with the failure telling us what to
sweep.

## 7. Open items

- **A token no candidate covers is relaxed even when nothing could ever match
  inside it** — `......`, a run of punctuation, a Latin word. Harmless today
  (no candidate exists there either way) but it widens the permitted set for
  no benefit, and if the bench cell moves this is the first thing to narrow.
- **The rule says nothing about *why* a token is unmatchable.** An unknown
  word, a vocabulary mismatch and a dictionary gap are all the same to it.
  That is deliberate — the engine cannot tell them apart and does not need
  to — but it means the escape fires on `......` as readily as on `本当は`.
- **The hints-on strict floor in §5.1 is a judgment, not a measurement.**
  0.8174 is today's figure and the phase should not fall below it; whether a
  small fall would be acceptable in exchange for a large gain on the reported
  register is a question for whoever reads the numbers, and this design takes
  the conservative side of it.
- **The eighteen sentences are not a corpus.** They are what was reported on
  one day, from one register, chosen by what looked wrong rather than sampled.
  Gate 6 records a figure over them; it does not license quoting that figure
  as the engine's accuracy on anything.

---

## 8. Measured 2026-09-15: the first implementation, and why it does not ship

The escape above was built as specified and measured the same day. It is on
`spike/phase4-escape` (`cbd68f9`), not on this branch, because **it fails
Gate 1.** What follows is the result, kept here because the design's
reasoning is what produced the failure and both halves are worth reading
together.

### 8.1 What held

| gate | result |
| --- | --- |
| 1, hints off | **identical to the digit** — 0.7342 (1588) / 0.7924 (1714) / 0.9093 / compounds 0.5860 (109), every class line unchanged |
| 2, compounds hints on | **unchanged**: 0.6129 (114 / 186), `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42 |
| 3, spurious merges | **26 of 26.** §4.3 named this as the gate that could genuinely fail and it did not |
| 4, snapshot | byte-identical |
| 6, eighteen sentences | **426 → 486 matched characters of 630**, against 487 with hints off. The gap this phase exists to close went from 61 characters to 1 |
| 7, suite | 46 in `engine_v2`, 0 ignored; `fmt` and `clippy --all-targets` clean |

### 8.2 What failed

Hints-on strict span accuracy **fell 1768 → 1713**, and §5.1 says it must not
fall. Decomposed span by span over all 150 sentences against a before-binary:

```
BEFORE  strict 1768  =  1332 matched  +  436 unmatched-but-exact
AFTER   strict 1713  =  1369 matched  +  344 unmatched-but-exact
                         +37             −92
```

`score`'s `fit` counts a segment sitting exactly on a gold span **whether or
not it matched anything** (`score.rs`'s `strict`, which looks only at
`start`/`len`). So part of that −55 is the metric withdrawing credit it was
giving to fragments that showed the reader nothing, and the figure that
tracks what a reader gets — gold spans resolving to a dictionary entry —
went **up** by 37.

**Only part, though, and the rest is a real defect in the escape.** Of the 92
spans that lost an unmatched-but-exact hit, **90 are now covered by a matched
span, and the matches are junk:**

```
品川        →  品 + 川
羽田空港     →  羽 + 田 + 空港
1969        →  19 + 6 + 9
ニュルンベルク →  ニ + ュ + ル + ン + ベル + ク
```

By script: 42 kanji, 25 katakana, 21 digits, 3 hiragana. These are proper
nouns and numbers that IPADIC holds as single tokens and JMdict does not hold
at all — so they are unmatchable by §2's definition, the escape fires on them
exactly as written, and their interiors are full of one-character entries
waiting to match. A reader who hovers 品川 now gets "goods" and "river".

**§7's first open item anticipated the mechanism and misjudged it.** It said
a token nothing covers would be relaxed harmlessly, "no candidate exists
there either way". Candidates do exist there, and they are the worst kind.

### 8.3 §4.2 was wrong about one test, in the useful direction

`a_reading_the_index_lacks_is_unmatched_between_correct_boundaries` was
listed as "expected not to move", on the reasoning that a missing key is a
data gap no segmentation change can reach. It moved, and improved: `ていう`
is a token nothing covers, so its interior reopens and `いう` matches — the
right reading, since the phrase is と + いう — leaving only the `て` that lost
its small つ. "The dictionary is missing this" does not imply "nothing in the
engine can help".

### 8.4 Where a narrowing has to bite

The failure is not that the escape fires too often but that it licenses
**one-character matches inside a token that is a single lexical unit**. Three
narrowings suggest themselves and none is measured:

1. **A minimum length on relaxed spans.** Kills 品 + 川 and 19 + 6 + 9
   directly. But `本当は` needs `は`, one character, so a flat minimum breaks
   the case this phase exists for; it would have to apply only to spans with
   *two* relaxed endpoints, and `させて` in `どっぴゅんさせてあげるから` has two.
2. **Script-run awareness.** Do not relax inside a token that is a single
   homogeneous run of digits or katakana. Principled — such a token has no
   interior word boundary — and it reaches 46 of the 92. It does nothing for
   the 42 kanji ones.
3. **A part-of-speech condition on the tail.** The mismatch that hurts is
   IPADIC gluing a grammatical suffix to a content word (`本当は`); the
   mismatch that does not is IPADIC holding a lexical unit (`品川`). The
   engine already knows `PARTICLE`, and `FUNCTION_STACK` already reasons
   about partitions this way. But `気持ち良かっ` is a conjugated adjective
   rather than a particle stack, so this narrowing has to admit conjugation
   too, and then it is no longer obviously narrow.

§6 still holds: turning the constraint into a price is not the answer, and
the failure here is an argument for a *sharper* condition on when the veto
lifts, not for softening the veto everywhere.


---

## 9. Measured 2026-09-16: the narrowing, and what it costs

§8.4 asked for a condition that keeps `本当は`, `気持ち良かったし` and
`どっぴゅんさせてあげるから` working while leaving `品川`, `1969` and
`ニュルンベルク` alone. **The condition is one hiragana character.**

```text
opens(T)  ≡  unmatchable(T)  ∧  T contains a hiragana character
```

Everything else in §2 is unchanged: coverage is still judged under the strict
rule, the escape still marks positions rather than spans, and a token that
passes still opens its whole interior.

### 9.1 Why hiragana

Japanese writes its inflection and its particles in hiragana. A hiragana
character inside a token is therefore the only evidence the engine has that
the token is a **concatenation** — a content word glued to something else —
rather than one word, and concatenation is the entire case §1 describes:

| token | what the hiragana is |
| --- | --- |
| `本当は` | the particle `は` |
| `気持ち良かっ` | okurigana, then a conjugated adjective |
| `ぴゅんさせてあげるから` | three ordinary words after an unknown one |

A token with no hiragana in it is a run of kanji, katakana or digits that
IPADIC declined to cut, and that is a single lexical unit by construction.
Reopening one buys nothing, because JMdict has no entry for the unit either —
it only lets the one-character entries inside it win a span each, which is
§8.2's whole failure.

This is a **narrowing of the veto's exception**, in §6's sense: the veto still
holds everywhere it held before, and the set of positions where it lifts is
smaller than §2's, never larger.

### 9.2 What the corpus says, per token

The decomposition §8 used, re-cut by hint token rather than by gold span.
Every unmatchable token the unconditional escape reopened over the 150 gold
sentences, hints on, classified by the scripts it contains, against the
gold spans that gained or lost a strict hit inside it:

| token script | tokens | gold spans gained | gold spans lost |
| --- | --- | --- | --- |
| hiragana only | 40 | 29 | 3 |
| hiragana + kanji | 6 | 0 | 0 |
| hiragana + katakana | 1 | 0 | 0 |
| **hiragana-bearing** | **47** | **29** | **3** |
| kanji only | 68 | 5 | 42 |
| katakana only | 36 | 3 | 25 |
| digits only | 25 | 0 | 21 |
| Latin, punctuation | 13 | 0 | 0 |
| **no hiragana** | **142** | **8** | **88** |

The separation is not marginal and it is not a threshold: the two halves have
opposite signs. Read the losing side by name and it is a gazetteer —
`品川`, `岡本`, `松久`, `浦和`, `新大阪`, `大阪環状線`, `百合子`, `仁兵衛`,
`ディオニス`, `マルヴィナ`, `ルクセイン`, `サフィレーヌ`, `2014`, `1969`,
`9518` — and the four exceptions on it are real multi-word compounds IPADIC
glued: `東京厚生年金会館` (3 spans), `クライシスマッピングパーティー` (3),
`田町駅` (1) and `冶爺` (1), which is the whole +8. Those four are what the
narrowing gives up, and §9.4 lists them again with the same figures.

### 9.3 The other three candidates, measured

§8.4 named three and measured none. All three were implemented behind a
switch and run against the same gates, so the choice is between measurements
rather than between arguments. Hints on, gold corpus, 2,163 spans:

| narrowing | strict | matched | unmatched-but-exact | 18 sentences |
| --- | --- | --- | --- | --- |
| *(before Phase 4)* | 1768 | 1332 | 436 | 426 / 630 |
| §2 unconditional (§8) | 1713 | 1369 | 344 | 486 |
| §8.4.1, min length 3 on doubly-relaxed spans | 1713 | 1369 | 344 | 478 |
| §8.4.1, min length 2 | 1714 | 1370 | 344 | 483 |
| §8.4.2, script-run: no all-digit or all-katakana token | 1753 | 1366 | 387 | 475 |
| **§9, hiragana-bearing token** | **1792** | **1361** | **431** | **475** |

Three findings in that table, none of them predictable from §8.4's prose:

- **A minimum length is inert on the gold corpus.** At 3 it reproduces the
  unconditional version's three *gold* cells exactly — 1713 / 1369 / 344 — and
  moves only the eighteen sentences, 486 down to 478. The reason is that blocking a
  relaxed span does not restore the gold span: `UNMATCHED` is 30 a character
  against `COVER`'s 1, so the DP takes any remaining shorter match and pads
  the rest with a fallback run — `羽田空港` becomes `羽田`(unmatched) + `空港`
  rather than going back to being one unmatched span. Every partial
  decomposition destroys the exact hit just as thoroughly as a full one.
  §8.4.1 was right that `は` breaks a flat minimum and wrong that the
  doubly-relaxed form would help.
- **Script-run awareness recovers about half of what it should.** It reaches
  the katakana and digit tokens by construction and leaves all 68 all-kanji
  ones open, which is where 42 of the 88 losses are. It ends up 15 spans
  *below* the pre-phase baseline, so it fails Gate 1 too.
- **The hiragana condition wins on gold and loses on the reported
  sentences.** At 475 it is the *lowest* row of the five in that column —
  tied with script-run awareness, below the other three, 11 below the
  unconditional escape. It is also the only one of the four that takes strict
  span accuracy *above* where Phase 4 found it. Gate 1 is a gate and Gate 6 is
  recorded, so that is the trade the phase takes; §9.4 names the eleven
  characters and the one sentence they are in. Reading this row as a win on
  both columns would invert the table.

§8.4.3, the part-of-speech condition, was not implemented. The measurement
above removes its motivation: it was proposed to separate a grammatical tail
from a lexical unit, and the script test does that with information the engine
already has at hand, without a second inventory to consult or a conjugation
case to special-case. It is recorded as not-needed rather than as untried.

### 9.4 What the narrowing costs, stated plainly

- **Four real compounds stay glued.** They are the whole +8: `東京厚生年金会館`
  (3 spans), `クライシスマッピングパーティー` (3), `田町駅` (1), `冶爺` (1).
  A kanji or katakana token that genuinely is several words is now
  unreachable, and the escape cannot tell one from `羽田空港` without
  information it does not have.
- **Eleven characters on the eighteen reported sentences**, all of them one
  sentence: `w7`, 「お、『ロイヤルネイビーガール』の新刊出てたんだな」, whose
  katakana title now comes back whole and unglossed rather than as
  `ロイヤル` + `ネイビー` + `ガール`. On this register that shattering would
  arguably have helped a reader; on the gold corpus the identical shape is
  `ニュルンベルク` → six pieces. The registers disagree and the gold corpus is
  the gate.
- **Three hiragana runs that are themselves one unit** — `ぱたぱたぱたぱた`,
  `ぇぇぇ`, `ぁぁぁ` — still shatter, because they are indistinguishable from
  `ぱひゅぱひゅぱひゅと`, which is the same shape with a particle on the end
  and is 26 of the 29 gains. This is a ceiling of the condition, not an
  oversight.

### 9.5 The gates

Run 2026-09-16 at the same gold index (5,402 entries, 9,930 keys, 13,898
records) and the same Vibrato dictionary.

| gate | required | measured |
| --- | --- | --- |
| 1, hints off | identical to the digit | **identical**, every printed line — the four headline metrics, the coarsening counts and every compound class line |
| 1, hints on, strict | ≥ 0.8174 (1768) | **0.8285 (1792)**, +24 |
| 1, hints on, tolerant | recorded | 0.8645 (1870), was 0.8525 (1844) |
| 1, hints on, entry top-1 | recorded | 0.9125, was 0.9092 |
| 2, compounds hints on | recorded | **0.6129 (114 / 186), unchanged**, every class line identical |
| 3, spurious merges | 26 of 26 | **26 of 26** |
| 4, snapshot | byte-identical | byte-identical |
| 5, bench scaling | off 0.98–1.07x, on 1.07–1.12x | off **1.01x**, on **1.07x** then 1.02x on a second reading |
| 6, eighteen sentences | recorded | **475 of 630** against hints-off's 487, from 426 |
| 7, suite | green, 0 ignored | 47 in `engine_v2`, 0 ignored; `fmt` and `clippy --all-targets` clean |

Gate 1 hints-on is the one §8 failed, and it does not merely recover: strict
span accuracy ends 24 spans above where Phase 4 found it, with the matched
half up 29 and the unmatched-but-exact half down only 5.

### 9.6 What §3 no longer describes

§9 said "everything else in §2 is unchanged", which is true of §2 and leaves
§3 stale. Recorded rather than rewritten, because §3 is what the phase was
planned against:

- **§3.2 and §3.3 give `Relaxed::over(n: usize, ...)`.** The shipped signature
  is `over(chars: &[char], ...)`: the hiragana test needs the characters of
  the token, not just how many there are. `parse` already had `chars` in hand,
  so §3.3's three lines are still three lines.
- **§2.1's last-but-one row promises less than it looks like.** Its column is
  "what the escape permits", and that is exactly right — measured on the real
  index, `気持ち良かったし` relaxes positions 1–5 and 7 is a real boundary, so
  `良かった` at `[3, 7)` *is* permitted. **§4.1 is the one that states an
  outcome**, "→ `気持ち` + `良かった` + `し`", and that outcome is the synthetic
  fixture's: it holds only `気持ち` and `良い`. Against the real 218,449-entry
  index the parse is `気持ち` + `良かっ` + `たし`, **identically with hints on
  and hints off**, so the phase's own claim holds and the split is the ranking
  class §1 puts out of scope. A reader taking §4.1 for a statement about the
  real index would be misled.
