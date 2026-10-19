# The class the probe could not see — handoff

**Date:** 2026-09-22
**Branch:** `feat/corpus-disagree`, tip `dd16e26`
**Predecessor:** [`2026-09-21-six-corpora-handoff.md`](2026-09-21-six-corpora-handoff.md)

The predecessor exhausted `vn-text` and left the ranked head mined out. This
session re-ran the whole probe at the current tip against the ta-old port,
confirmed the head really is exhausted, and then found a class of 918
occurrences hints-on that **no probe run could ever have reported**, because
`corpus_disagree`'s own cover filter refused it by construction.

Three commits: `b550b7f` and `586dc6d` teach the probe to see the class,
`dd16e26` fixes it in the engine at index format 7. The three commits below
them on this branch (`c2d2df6`, `419b3f7`, `f1a2fda`) are window and UI work
from another session and are unrelated.

## 1. The re-run, and the control

Ten corpora, 131,847 sentences, both hint states, current engine against v1
dumps produced from the detached worktree at `D:\Code\oss\clipyomi-v1`. The
corpora, their text dumps and the v1 side are the predecessor's, reused
unchanged; nothing derived from them is in this repository.

| state | corpora | we merge, v1 splits | we split, v1 merges | ratio |
| --- | --- | --- | --- | --- |
| on | all ten | 10,167 | 18,120 | 1.78× |
| off | all ten | 6,623 | 24,173 | 3.65× |

The predecessor's six re-measure at 8,257 / 14,651 against its 8,260 / 14,651,
so **`f750bd1` did not move the ratio** — the control its §2 asks for, paid
this time at no extra cost because the re-run was happening anyway. Twelve
corpora and the sign has still never changed.

**The head is exhausted, and that is the finding.** Hints-on, the split
direction is criterion 3 working (`すぐ|に`, `こと|で`, `好き|な`), the merge
direction is the shipped auxiliary policy (`なってる`, `くれている`,
`思ってた`), and `sht` fires on 27 surfaces in total, most of them the correct
particle-tail split. Nothing at the head is a defect.

## 2. What the head could not contain

`corpus_disagree.rs:361` required `WordFlags::PRIMARY` of a covering
candidate. The reason is sound — a PRONOUNCE-only record is a kanji entry's
*reading*, so without it `はは` is covered by 義母(ぎぼ), `ことし` by 今年 and
`みた` by 蒙(むた), which is `shatter_sweep.rs`'s own §9.5 absurdity.

But JMdict tags an entry `uk` when kana is how the word is normally written,
**and such an entry still keeps its kanji headword**. `おねだり` is お強請り,
`もしかして` is 若しかして, `よさそう` is 良さそう. Their everyday spelling is
PRONOUNCE-only by construction, so it could never be a cover: every one of
them printed `-` in the cover column and read as v1 noise. `おねだり` sat at
rank 131 of pxc2's ranking, nine occurrences, cover `-`.

`b550b7f` exempts `uk` from the PRIMARY clause, and takes the stack partition
with it. `lex::is_grammatical_stack` asks whether a span tiles into short
COMMON pieces, which is the only question an engine with no part of speech can
ask, and every kana adverb answers yes: `もしかして` tiles `もし|か|し|て`.
Partitioning those away buried exactly the rows the exemption exists to show —
measured, `もしかして` ranked 163 before and vanished after. A probe holding
the assembled entry reads the POS instead. kf3if hints-on, before and after:
two-signal rows 7 → 38, grammatical stacks 284 → 468, 40 rows with a `[uk]`
cover.

## 3. The class, and its cause

`cost::PRONOUNCE_CROSS` is 25 per tokenizer boundary interior to a
PRONOUNCE-only match. A `uk` word written in kana is PRONOUNCE-only, so
wherever IPADIC cuts inside it, its own pieces win. The minimal pair, hints-on,
same shape and same boundary:

```text
お尻を見る。      → お尻|を|見る      kanji, PRIMARY, merges across お|尻
おねだりする。    → お|ねだり|する    kana,  PRONOUNCE, pays 25
```

Ten corpora, counting only places the ta-old port emits a `uk` content word
whole and our tiling is not a grammatical stack:

| state | TA emits whole | we keep | stack split (criterion 3) | **shattered** |
| --- | --- | --- | --- | --- |
| on | 30,389 | 25,314 | 3,631 | **918** |
| off | 30,013 | 23,591 | 3,378 | **1,970** |

`おねだり` 115 of 116, `もしかして` 76, `もみもみ` 49, `すごい`→`すご|い` 39,
`ちっこい` 28, `いっぱい`→`い|っぱ|い` 23, `すぐそば` 18.

**Inflected forms of `uk` entries are the same class and larger.** `できる` is
出来る, `uk`:

| form | occurrences | cut hints-on | cut hints-off |
| --- | --- | --- | --- |
| できない | 389 | 374 (96.1%) | 13 (3.3%) |
| できなかった | 67 | 66 | 0 |
| できなくて | 38 | 38 | 0 |
| できず | 22 | 22 | 0 |
| できません | 58 | 6 | 0 |
| よさそう | 122 | 122 (100%) | 103 |

`出来ない`, `食べない`, `見えない`, `信じられない` — anything kanji-bearing —
merge in both states, and gold merges 一段+ない (`食べない`, `いない`), so the
split is against this project's own standard and not merely against TA.

The cause was proved rather than inferred: `PRONOUNCE_CROSS = 0` merges
`おねだり` and `できない` and simultaneously re-breaks `のか`→退く, which is
what the term is for.

## 4. Four conditions, three of them refusals

`dd16e26` exempts a candidate from `PRONOUNCE_CROSS` when it carries
`WordFlags::UK`, is `exact`, has an empty chain, is not a `grammatical_stack`
and is at least `UK_MIN_LEN` = 3 characters. Every condition was measured.

- **`uk` alone is not enough, and the chain is why.** `ももう`→揉む,
  `はもう`→食む, `のか`→退く, `とかな`→解く, `たまん`→溜まる are all `uk` and
  all reached through a deinflection chain; `はこう`→履く, `になって`→荷なう,
  `ことし`→今年, `どうし`→同士, `してい`→指定, `がい`→甲斐, `はは`→義母 are
  none of them `uk`. The two halves of the noise class partition cleanly,
  which is what made the exemption thinkable.
- **A `uk` bit that ignores POS costs 16 gold spans hints-on**, every one a
  particle swallowed: `よう|だ`→`ようだ`, `どう|か`, `よう|に`, `それ|に`. So
  the bit itself means "`uk` **and** vocabulary", decided in `flags.rs` where
  the senses are in hand. `spurious_merges.txt` stayed 26 of 26 throughout,
  which is the lesson: the fixture-sized gate could not see this, exactly as
  `KATAKANA_FIXTURE` could not see `ラストラン`.
- **No length floor costs 333 false merges.** `冬|だ|し` became `冬|だし`
  glossed 出汁 "soup stock" and `愛おし|げ|に` became `愛おし|げに` glossed
  実に. Neither is caught by `grammatical_stack`, whose one-character pieces
  must be PARTICLE, and `だ`(cop) and `げ`(suf) are not. Three loses `いつ`
  (何時) and `あだ` and keeps every longer win.
- **The stack test is still needed on top of the POS test**, for the shapes
  JMdict tags as vocabulary but the tiling calls grammar: `どうか`(adv,vs) and
  `もしかして`(adv) both survive the POS gate and are refused here.

## 5. Gates

Measured control-against-candidate on one freshly built format-7 index, the
control reproducing the recorded baseline exactly, which is what makes the A/B
mean anything.

| | on strict | on tolerant | on entry top-1 | on compounds | off strict | off tolerant | off entry top-1 | off compounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 1694 | 1948 | 0.9527 | 152 | 1588 | 1785 | 0.9504 | 118 |
| shipped | 1694 | 1948 | 0.9527 | 152 | 1588 | 1785 | 0.9504 | 118 |

**Byte-identical in both states.** Hints-off is unchanged by construction — no
boundaries, no crossing — and the corpus diff confirms it: 0 sentences of
131,847 change. `cargo test --workspace --features mecab` is 673 passed, 0
failed, 1 ignored (the pre-existing `jmdict_source` doc-test); fmt and clippy
clean; `parse_sentences.txt` and the snapshots byte-identical. `bench`
interleaved four rounds, median candidate/baseline 0.98 at 1,005 characters and
1.01 at 10,040 — inside session noise.

Corpus effect, hints-on: 1,526 sentences change (1.16%), the shatter class
falls **918 → 302**, and of the 1,699 spans that are new the ta-old port emits
**1,203 (70.8%)** of them.

Index format 6 → 7. `ensure_dictionary` rebuilds a generation on a version
mismatch, so the user cost is one rebuild from the source already on disk.

## 6. What it does not fix

- **`もしかして`, 76 occurrences.** Vocabulary by POS, a stack by tiling, and
  the tiling test wins. Fixing it needs a discriminator neither test has.
- **The `できる` family, 506 occurrences.** The chain condition excludes it by
  design; reaching it means exempting the chain's start, which is the widening
  measured and refused on 2026-09-18 for breaking `はこう`.
- **`よさそう`, 122.** A second cause sits under this one: at
  `PRONOUNCE_CROSS = 0` it still comes out `よさ|そうだ`.
- **Hints-off, 1,970 occurrences.** Untouched by construction. That state has
  no boundaries, so its shatter is a different mechanism and needs its own
  measurement.
- **A residue of three-kana coincidences**, `だいじょ` 10 and `まだい` 3 among
  them, roughly 2% of what the change recovers. They are the class the floor is
  drawn against, surviving just above it.
- **The gray merges**: `そのよう` 89 and `このよう` 42 now merge, which TA does
  not. Both are 其の様/此の様, `uk` adverbs, and criterion 3 has no row for
  them either way.

## 7. Not done

- The predecessor's open list is untouched: the particle → godan family, the
  hints-off atomisation class, `やだ`, the stray-kana gate, `達した`, `イって`,
  and the six darwin screenshot baselines.
- **`はいない` 83 occurrences, 0% correctly cut hints-on** (72.3% hints-off) —
  the `はい` swallow the 2026-09-18 handoff named as `prt`'s blind spot, now
  counted on ten corpora and still open.
- The probe's `uk` cover brings short kana coincidences back at the tail
  (`あだ`, `いつ`, `もみ`). They are adjudicated the same way as PRIMARY's own
  noise class, per occurrence.
- `vn-text` is still exhausted. The next unknown is a different register, not a
  thirteenth eroge script.
