# A Vibrato user lexicon, measured and not shipped

**Date:** 2026-09-16
**Branch:** `spike/user-lexicon`, not merged
**Question:** `いちゃいちゃ` is in JMdict and the engine shatters it. The
boundary constraint would protect it if IPADIC held it as one token. Does
adding the missing words to a Vibrato user lexicon pay?

**Answer: the mechanism works, the lists do not, and the safety argument I
made for it was wrong.**

## 1. The mechanism works

With a user lexicon loaded:

```
いちゃいちゃする       tokens: いちゃいちゃ | する     parse: いちゃいちゃ | する
しっちゃかめっちゃかだ  tokens: しっちゃかめっちゃか | だ  parse: しっちゃかめっちゃか | だ
```

Exactly as predicted from `美少女`, which survives hints-on only because
IPADIC holds it whole. Vibrato accepted the CSV, the connection ids lifted
from real IPADIC entries verified against the matrix, and the words came back
whole. Nothing about the route is impossible.

## 2. The dictionary-derived list is worthless

956 all-hiragana single lexical items that JMdict has, IPADIC cuts, and the
engine shatters into a one- or two-character piece.

**Not one of them occurs in 66,116 lines — 3.1M characters — of real visual
novel text.** `いちゃいちゃ` itself: zero occurrences. The control words in the
same file read fine (`する` 3,778, `ちゃん` 1,428, `好き` 400), so this is the
list and not the encoding.

The reason is structural and worth keeping: **IPADIC's gaps are rare words by
construction.** A list selected by "the tokenizer does not know it" selects
for obscurity — Okinawan dishes, niche greetings, dialect. Deriving a lexicon
from the dictionary is selecting on the wrong axis.

## 3. The text-derived list measures net-negative

Inverting it — rank the 12,643 shattered words by how often they actually
occur — gives 207 words with 1,737 occurrences, and the head of the list is
real vocabulary: `おまんこ` 190, `ですけど` 169, `みたいだ` 145, `おちんちん`
113, `それなりに` 79, `ぞわぞわ` 28.

Filtered to the 164 that are genuinely damaged (a piece of one or two
characters — the rest are served better by `relax::MIN_PIECE`, which already
splits `ダークエルフ` into two real words):

| | baseline | with the 164 |
| --- | --- | --- |
| span accuracy, hints on | 0.8437 (1825) | **0.8428 (1823)** |
| entry top-1 | 0.9341 | 0.9340 |
| tolerant | 0.8798 | 0.8798 |
| compounds | 0.6129 (114) | **0.6183 (115)** |
| eighteen sentences | 486 / 630 | 486 / 630 |
| spurious merges | 26 / 26 | 26 / 26 |

−2 gold spans, +1 compound, and **no change at all** on the eighteen reported
sentences. Every instrument that exists says "nothing, or slightly worse".

## 4. The safety argument was wrong

The sweep document claimed adding these entries was structurally safe: every
word is in JMdict by construction, so the new token is always matchable, and
adding a token only *removes* interior boundaries. **The second half is
false**, and the −2 above is the proof.

The two lost spans are `タイムリー` and `ヒット`, in
`この試合は2回に阿部慎之助選手がタイムリーヒットで先制。`:

```
before:  ... 選手 | が | タイムリー | ヒット
after :  ... 選手 | が | タイムリ | ーヒット
```

**None of `タイムリー`, `ヒット` or `タイムリーヒット` is in the lexicon.**
Adding 164 unrelated entries re-cut a region that contains none of them, into
a nonsense split before the prolonged sound mark. Vibrato's Viterbi is global:
new entries change path costs, and boundaries move where no entry matches. A
user lexicon does not only relax the veto locally — it perturbs tokenization
everywhere, and the fabricated connection ids and MeCab autocost heuristic
used here are the likeliest cause.

## 5. What would change the answer

- **Real dictionary compilation.** The ids and costs here were lifted from one
  probe word each and MeCab's `-400 × len^1.5` heuristic. A properly compiled
  user dictionary assigns costs against the connection matrix; that is a
  `mecab-dict-index` job, not a CSV a sweep can write.
- **A corpus that can see the benefit.** The gold corpus is news and
  literature and contains none of these words; the eighteen sentences contain
  none either. The benefit is asserted from 1,737 occurrences in a pool with
  no adjudication, so it cannot be scored. Until something can measure it, a
  change that costs 2 gold spans has nothing to weigh against it.
- **A smaller, hand-curated list.** 164 was already filtered twice. The
  perturbation in §4 suggests the risk scales with list size rather than with
  which words are on it, so a list of five or ten — the words a reader
  actually hits — may behave differently. That is a different experiment.

## 6. Standing conclusion

`いちゃいちゃ` remains shattered. The cost model cannot fix it
([`2026-09-16-per-match-cost-spike.md`](2026-09-16-per-match-cost-spike.md)),
and the tokenizer route works but cannot currently be shown to pay. Both
answers are negative and both are now measured rather than assumed.
