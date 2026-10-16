# The residue, adjudicated — handoff

**Date:** 2026-09-22
**Branch:** `feat/corpus-disagree`, tip `c3cf700`
**Predecessor:** [`2026-09-22-uk-kana-shatter-handoff.md`](2026-09-22-uk-kana-shatter-handoff.md)

The predecessor shipped the `uk` exemption and left a named residue it had not
adjudicated: 302 shattered occurrences hints-on, 1,970 hints-off, and two
families (`できる`, `はいない`) it attributed to the chain condition. This
session re-ran the whole probe at the current tip, adjudicated that residue
surface by surface, shipped the half that is fixable, and **refused** the
hints-off half on a measurement that the gold corpus alone would have passed.

Three commits, all of them user-lexicon rows: `1798a9f` adds three for the
`もしか` family, `06e9158` five more for the family §6 had called blocked, and
`c3cf700` one protective row that cancels a collateral of those five. §10 is
that correction and it is the larger change. The hints-off candidate was built,
measured and reverted; §5 is its record.

## 1. Controls first

Three, all of them cheap and all of them paid before anything was read.

- **The index.** `build-index` at this tip, from the same `JMdict_e.xml`, is
  **byte-identical in all six files** to the predecessor's `idx-uk2`. So its
  span dumps are this tip's span dumps and did not need re-running — the
  session's largest saving, and the reason the numbers below are comparable to
  the predecessor's cell for cell.
- **The gold cells.** 1694 / 1948 / 0.9527 / 152 hints-on and 1588 / 1785 /
  0.9504 / 118 hints-off, reproducing the predecessor's baseline exactly.
- **The probe.** `corpus_disagree` re-run at this tip over ten corpora and both
  hint states, 131,847 sentences, against the same v1 dumps. The six corpora
  the four-corpus handoff measured read 8,258 merge-direction regions against
  its 8,260 — the `uk` fix moved the split direction (14,651 → 14,521) and left
  the merge direction where it was.

| state | merge-direction regions | split-direction regions | ratio | cuts move |
| --- | --- | --- | --- | --- |
| on | 18,164 | 33,458 | 1.84× | 8,745 |
| off | 14,321 | 57,812 | 4.04× | 14,207 |

## 2. The ranked head is policy, in both directions

Re-ranked at this tip and aggregated over the ten corpora, with the surfaces
already named in the documents removed, the head is entirely deliberate
behaviour:

| direction | head |
| --- | --- |
| we split, v1 merges | `そう\|な` 620, `に\|なった` 405, `すぎ\|て` 329, `みたい\|に` 327, `こと\|で` 287, `射精\|して` 281 |
| we merge, v1 splits | `なってる` 287, `くれてる` 227, `わかってる` 155, `くれている` 131, `思ってた` 114 |

The first is success criterion 3 working; the second is the shipped auxiliary
policy. **No defect reaches the head of either direction.** That is the third
session in a row to report it, and it is why the rest of this document is about
the class the ranking cannot see.

## 3. The residue, surface by surface

`uk_sweep5.py` counts the places the ta-old port emits a `uk` content word
whole and this engine's tiling is not a grammatical stack. At this tip:

| state | TA emits whole | we keep | stack split | **shattered** |
| --- | --- | --- | --- | --- |
| on | 30,389 | 26,388 | 3,217 | **302** |
| off | 30,013 | 23,591 | 3,378 | **1,970** |

Every surface above two occurrences hints-on was read in context. The verdicts:

| surface | n | verdict |
| --- | --- | --- |
| `もしかして` | 76 | **defect, fixed** — the adverb in every context read |
| `なんで` | 43 | **TA is wrong.** Half its contexts are `だけなんです`, where `な` + `んで` is the copula and `なんで`(何で) is not the word at all |
| `はなし` | 20 | **TA is wrong.** `手抜きはなし`, `占うのはなし` — `は` + `なし`, and 放し is not what is written |
| `わかりやすい` | 17 | defect, small; 分かり易い whole against `わかり` + `やすい`, which is itself decomposable |
| `してやる` | 17 | **TA is wrong by this project's own standard.** `gold` annotates 〜て + auxiliary as two spans, 34 of 34 |
| `もしかすると` | 17 | **defect, fixed** |
| `おそば` | 16 | gray — お + そば is a defensible reading of the honorific, and one occurrence really is 蕎麦 |
| `シゴく`, `ふともも`, `おまた` | 13, 8, 7 | defects, one lexeme each, two corpora each |

`もしかしたら` does not appear in that table because the sweep classes it as a
stack split, but it is the same family and the worst member: **59 occurrences,
0 kept whole.** Counted over every occurrence rather than only where TA agrees,
the family is 334 / 59 / 17 occurrences and 153 splits.

## 4. What shipped

`1798a9f` puts `もしかして`, `もしかしたら` and `もしかすると` in
`assets/hint_user_lexicon.csv`, cap 85 → 88.

The cost model cannot reach them. All three are `adv,uk`, so `flags.rs` gives
them `WordFlags::UK`, but `cost.rs`'s exemption also requires
`!grammatical_stack`, and `lex::is_grammatical_stack` sees a span tiling into
short COMMON pieces (`もしか` + `して`) and refuses. The tiling test cannot see
that the whole is an adverb. The tokenizer is the place, exactly as it was for
`いちゃいちゃ`.

| | gold, both states | corpus hints-on |
| --- | --- | --- |
| effect | every cell identical to the digit | 155 of 131,847 sentences change, 153 occurrences recovered, 3 lost |

The three lost are `ナスカ` → `ナ|スカ` twice and `早百合` → `早|百合` once,
each a name standing next to the new token: Vibrato's Viterbi is global, which
is the whole reason the cap test exists. 153 against 3 is the trade `なのに`
took. Word cost `-4472` and `-2078` were both measured and differ by one
`もしかして` occurrence, so the magnitude is flat here the way
`PRONOUNCE_CROSS`'s is.

## 5. The hints-off half, built and refused

Hints-off is 1,970 occurrences — six times the hints-on residue — and its head
is not gray at all: `かなり` 137, `やがて` 99, `おねだり` 97, `まみれ` 74,
`もはや` 72, `ケータイ` 51, `カバン` 51, `おかげ` 41, `もともと` 36,
`だいたい` 32, `かつて` 27. Real words, cut into meaningless pieces
(`か|なり`, `カ|バン`, `もと|もと`).

The cause is arithmetic: `LEN_GAIN` is gated on `bounds`, so hints-off a
3-character `uk` word costs `+6` against `+2` for its two COMMON pieces. The
candidate was the narrowest possible widening — let `LEN_GAIN` apply with
`bounds == None` **only** under the predecessor's own conjunction (`UK`,
`len >= 3`, exact, empty chain, not a grammatical stack), which is far narrower
than the blanket ungating measured and refused when `LEN_GAIN` was introduced.

**Every gold cell moved the right way:**

| hints-off | strict | tolerant | entry top-1 | compounds |
| --- | --- | --- | --- | --- |
| baseline | 1588 | 1785 | 0.9504 | 118 |
| candidate | **1592** | **1791** | **0.9505** | **119** |

Hints-on is untouched by construction. On the gold corpus this is a clean win,
and **it is wrong.** The ten-corpus A/B says so: 4,836 sentences change (3.67%),
8,049 spans are gained and 9,826 lost, and of the gained spans the ta-old port
emits only **2,919 (36%)**. The 64% it does not contains the reason:

```text
そう言いながら…   言い|ながら  →  言|いながら   253 occurrences
歯の先端を覆い…   覆い|ながら  →  覆|いながら   314 occurrences
```

`いながら` is 居ながら, `adv,uk`, four characters, exact, no chain, not a
stack — it satisfies every condition — so with the gain it steals the `い` off
the preceding 連用形. That is the same shape as the `きのこ` + `と` invention
the blanket ungating produced, and the narrow gate does not exclude it, because
the property that makes it wrong is not in JMdict.

**The gold corpus could not see it**: 150 sentences, and the class costs it
nothing. The instrument that sees it is the corpus A/B, and this is the third
time in this project a change has passed the fixture-sized gate and failed the
corpus (`KATAKANA_FIXTURE` and `ラストラン`; the `uk` bit without the POS test
and its 16 swallowed particles).

**Refused and reverted.** The 1,970 stands. Any future attempt needs a signal
that separates a word from a 連用形 + auxiliary, which is the same missing
discriminator §6 names.

## 6. `できる` and `はいない` are one defect — blocked in the cost model, fixed in the tokenizer

**Read §10 first: this section's "blocked" is true only of `cost.rs`.**

The predecessor listed them apart. They are the same arithmetic: a `uk` entry's
**inflected** form is PRONOUNCE-only, the chain condition excludes it from the
exemption, and `PRONOUNCE_CROSS` then charges 25 for the cut IPADIC makes
inside it.

- `はいない`: 83 occurrences, **82 read `はい` + `ない`** — the `prt` blind
  spot, "yes" swallowing the topic particle. `は` + `いない` costs 22 against
  `はい` + `ない`'s 0, and 21 of those 22 points are the crossing charge on
  `いない`(居ない).
- The `できる` family: 506, measured by the predecessor.
- `はいる` is the control that shows the charge is what decides it: 38
  occurrences, **35 correctly cut `は` + `いる`**, because `いる` is one token
  and there is no interior boundary to charge.

Extending the exemption to inflected forms is the widening refused on
2026-09-18 (`はこう`→履く, `ことし`→今年). Gating that widening on `UK` does
**not** rescue it: `ももう`→揉む, `はもう`→食む, `のか`→退く, `とかな`→解く
and `たまん`→溜まる are all `uk`, all inflected, and all three characters, so
neither the flag nor `UK_MIN_LEN` separates them from `いない`. Not attempted;
the separation is not in the dictionary — **and it is not needed, because the
tokenizer does not have to separate anything. See §10.**

## 7. The shipped `uk` fix, audited from the other side

The predecessor reported that of the 1,699 spans its fix newly emits, the
ta-old port emits 1,203. The other **496** were never read. They are:

- correct merges TA simply does not make — `もみもみ` 65, `えっち` 44,
  `いっぱい` 26, `ぶっかけ` 12;
- the gray adverbs `そのよう` 89, `このよう` 42, `あのよう` 9, which §6 of the
  predecessor already named;
- and **12 false merges**: `だいじょ` 6 (from `だいじょぶ`), `まだい` 3,
  `くない` 3. That is 2.4% of the 496 and matches the "roughly 2%" the
  predecessor estimated from the other direction.

No action. The audit is here so the estimate is a count.

## 8. Gates

`cargo test --workspace --features mecab`: **673 passed, 0 failed, 1 ignored**
(the pre-existing `jmdict_source` doc-test). `cargo fmt --all --check` and
`cargo clippy --workspace --features mecab --all-targets` clean. Gold cells
identical in both hint states. No index format change, so no user-visible
rebuild.

## 9. Not done

- **`わかりやすい` (17), `シゴく` (13), `ふともも` (8), `おまた` (7)** — the
  rest of the hints-on residue worth fixing, one lexeme each. They are lexicon
  rows, but the cap is a measured number and four more rows is a fourth A/B;
  the family shipped here was worth its measurement at 153 occurrences and
  these are not, individually.
- ~~The 589 of §6~~ — **fixed in §10.**
- The 1,970 hints-off, per §5.
- Everything the predecessor left: the particle → godan family, the
  hints-off atomisation class, `やだ`, the stray-kana gate, `達した`, `イって`,
  and the six darwin screenshot baselines.
- **`vn-text` is exhausted, and this session did not need a new corpus** — the
  ranked head was already policy. What the next unknown needs is a different
  register, or a different instrument, not a thirteenth script.

## 10. Postscript: the tokenizer route, and 832 occurrences

`06e9158`, the same day, after §6 had already called the class blocked.

**The mistake §6 made was reaching for the cost model.** The class is a
tokenizer artifact: IPADIC holds `いる` whole and splits `い|ない`, which is why
`は|いる` is cut correctly 35 times in 38 while `はい`(yes) wins 82 times in 83.
Nothing about that asymmetry is linguistic, and the user lexicon is exactly the
tool for it — the same tool `いちゃいちゃ` needed, applied for the first time to
an inflected form.

Five rows: `いない`, `できない`, `できなかった`, `できず`, `よさそう`.
`できなくて` was tried and dropped — the forced token did not survive the
engine's own `て` split. `よさそう` is at `-5878` because `-3200` loses
`気持ちよさそう`, which IPADIC cuts `気持ちよ|さ|そう`.

| | |
| --- | --- |
| gold hints-on | strict 1694 → **1696**, tolerant 1948 → **1950**, entry top-1 0.9527 → **0.9528**, compounds 152 |
| gold hints-off | identical, by construction |
| snapshot | byte-identical |
| ten corpora, hints-on | 847 of 131,847 sentences change (0.64%), **832 occurrences recovered** |

`いない` 333, `できない` 295, `よさそう` 122, `できなかった` 60, `できず` 22,
and `はい` is gone from all 83 of its swallows. **These are the first lexicon
rows to move a gold cell rather than hold it**, which is worth noticing: the
class was large enough to reach a 150-sentence corpus.

The price is about a dozen occurrences — `もったいないでしょ` → `もった|いない
でしょ` 3, `のに` split twice beside it, `イっていない` twice, `気持ちよさそうな
お顔` → `なお|顔` twice.

**The protective row, `c3cf700`.** `もったいない` was held out of `06e9158`
because it had not been through the A/B; it has now, and it ships. Ten corpora,
hints-on, against `06e9158`: **8 sentences change (0.01%)**, gold and snapshot
identical in both states. Three are the repair — the shatter, and the two `のに`
that stopped splitting beside it. Five are `もったいないです` ×4 and
`もったいないじゃ` ×1 merging back into the polite span the engine emitted
before today, which is §2's own merge-direction policy rather than a defect.
Its own gain is three occurrences, which is small; what makes it worth a row is
that the footprint is eight sentences and it cancels a collateral this session
introduced. Cap 93 → 94.

**The hazard worth carrying forward:** `いない` as a forced token breaks a kana
な-adjective ending in い before a word starting with い — `きれいないろ` reads
`きれ|いない|ろ`. It occurs **zero times in 131,847 sentences** of this
register, and `けんめいないのり` and `しずかないい人` both stay correct, but a
different register could carry it.

**And the probe inputs were purged mid-session** — the temp tree holding the
corpora, the v1 dumps and every span dump was reaped for disk space. The pool
was re-dumped from `D:\Code\oss\vn-text` with §1's recipe and **every one of
the ten counts reproduced exactly** (131,847 total), which validates that
recipe a fourth time. The v1 side was not rebuilt: a control-against-candidate
A/B does not need it, and the adjudication above is by reading contexts.
