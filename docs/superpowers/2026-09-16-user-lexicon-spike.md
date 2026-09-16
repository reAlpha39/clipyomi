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

## 6. Resolved the same day: the size was the problem, not the route

§5's last bullet guessed that the perturbation scales with list size rather
than with which words are on it. **It does**, and the threshold is sharp:

| entries | gold strict | compounds | occurrences covered |
| --- | --- | --- | --- |
| 2, 6, 11, 26, 41, 61 | 1825 | 114 | up to 1,240 |
| **81** | **1825** | **114** | **1,294 of 1,393** |
| 121 | 1823 | 115 | 1,349 |
| 165 | 1823 | 115 | 1,393 |

At 81 entries **every gold cell is identical to the digit** — span accuracy,
entry top-1, tolerant, the coarsening counts and every compound class line, in
both hint states — the eighteen sentences do not move, the spurious-merge gate
is 26 of 26, the snapshot is unchanged and bench reads 1.03x. And
`いちゃいちゃする` parses as `いちゃいちゃ | する`.

So the route ships, capped where the measurement caps it:
`crates/jparser/assets/hint_user_lexicon.csv`, applied in `hints.rs` and
falling back to no user lexicon if the loaded dictionary rejects the
connection ids. §4's warning stands and is why the cap exists rather than a
round number.

The two negative results above are unchanged and still worth having: the
dictionary-derived list really is worthless, and the cost model really cannot
do this.


## 7. `ある日`, and two corrections to the sweep

Asked the same day: `ある日` parses as `ある` + `日` and is a JMdict headword,
so why is it not in the list?

**Because JMdict tags it `exp`.** Its entry is `exp,n`, "one day; (on) a
certain day" — the lexicographers classify it as a phrase, and §2's filter
drops phrases on purpose, since a phrase should come apart into its component
words. Here that is also the better reading: `ある` and `日` are both glossable
and the composition is transparent. `いちゃいちゃ` is `adv,vs` — a single
lexical item whose pieces are not words at all. That tag is the whole
distinction, and it is JMdict's rather than ours.

The arithmetic is identical (`ある日` +3 against `ある` −2 plus `日` −1), so if
the merged reading is ever judged better the route is there. It occurs once in
3.1M characters of the pool, so frequency does not argue for it.

**Two real defects in the sweep came out of the question.**

1. **It only considered kana-only headwords**, so every mixed-script word was
   invisible to it — `ある日` among them. The first published figures (956
   damaged, 12,643 shattered) are the kana-only slice, not the whole class.
2. **Fixing that naively made it much worse.** Taking every reading as a
   surface pulls in the readings of *kanji* entries, which are pronunciations
   rather than ways a word is written, and the frequency-ranked head became
   `してい` 1906 (指定), `どうし` 755 (同士), `それを` 469 — handoff §9.5's
   spurious-merge class, item for item. Forcing those as tokens would be
   actively harmful.

The rule the tool now uses is kanji forms always, readings only where the
entry has no kanji form. At three characters and up that gives 116,684
shattered words of which 1,329 are all-hiragana and damaged — against the
213,400 the broken version reported.

**The shipped 81 entries are unchanged.** They were generated under the
kana-only rule and measured under it; regenerating with the corrected rule is
a different list and would need the gates run again.
