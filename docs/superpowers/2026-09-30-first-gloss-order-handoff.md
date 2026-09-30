# The first gloss against the ta-old port, and three order fixes — handoff

**Date:** 2026-09-30
**Branch:** `fix/first-gloss-order`, from `e22f398` (the unmerged `fix/reported-sentence-glosses`): `de3b65a`, `45a3f49`, `9bd5d33`, `a7a9f24`, `cbfe1e3` (review follow-ups: docs and tests only), and this document
**Predecessor:** [`2026-09-26-reported-sentence-handoff.md`](2026-09-26-reported-sentence-handoff.md)

Every earlier session compared the two engines' **spans**. This one compared
the gloss a reader sees first, on the spans both engines cut the same way,
and found that the defects left there come from `assemble::order`, not from
segmentation. Three order clauses shipped. None moves a boundary, and gold
entry top-1 rises in both hint states.

## 1. Controls

- **This engine.** The branch base `e22f398` was rebuilt, and its dumps over the twelve visual-novel corpora (151,225 sentences) are byte-identical to the last session's final candidate.
- **The port.** The port is v1, run hints off, the state a user of it ran ([`2026-09-26`](2026-09-26-reported-sentence-handoff.md) §1). It now also dumps each span's first entries, through an untracked example beside the untracked `v1_dump` in the detached worktree. Its segmentation matches the 2026-09-25 hints-off dump on every one of the 151,225 lines.
- **Gold at the base.** 1700 / 1954 / 0.9585 (1201 / 1253) / 152 hints on; 1590 / 1787 / 0.9570 (1179 / 1232) / 118 hints off.

## 2. What differs

- **Spans, the port hints off against this engine hints on.** There are 224,811 disagreement regions: 128,595 where we split, 61,475 where we merge and 34,741 crossing. The head is the policy four earlier sessions recorded: criterion 3's particle stacks, names we merge, and the auxiliary policy. Only two items were new:
  - `かも|しれない`, whose しれない is §3.1.
  - `だんな|さま`, 1,161, of which 1,160 are in one corpus. It is compositional, both halves are glossed right, and it matches the short units gold is adjudicated at. **No action.**
- **The first gloss on a shared span**, which was never measured before. It differs on 113,731 spans hints on and 125,770 off. Ours is the right one in the large majority: the port reads くれる as 繰る "to reel", いる as 射る "to shoot" and おれ as 織る "to weave". Ranks 0 to 260 by count were adjudicated by hand and ranks 260 to 800 by a triage pass, which together cover 95% of the mass hints on. Where ours is wrong, the cause is one of a few `order` clauses, and §3 fixes three of them.

## 3. Shipped

Every commit builds and passes the workspace suite and both gold states on
its own. Each was measured by **entry id**, not headword
([`2026-09-26`](2026-09-26-reported-sentence-handoff.md) §3 is why).

### 3.1 サ変's し under れる/せる ranks last (`de3b65a`)

The table's れる/せる row names its site as 未然形 by form alone, so it
attaches to サ変's 未然形 し, and `しれない` read する "Passive/Potential
Negative" ahead of 知れる. `sahen_shi_voice` sends that chain last.

**The table refusal was measured first and splits the span.** Refusing the
pair in `table::search` cut it in 968 sentences hints on. IPADIC cuts
しれ|ない; the cut falls inside する's inflection, which `PRONOUNCE_CROSS`
exempts, but at the end of 知れる's stem, where it charges (−10 against 15).
The する chain is what holds the span.

| | first entries changed | to 知れる | other |
| --- | --- | --- | --- |
| hints on | 978 | 945 | `しれん` → 試練 26 (one false gloss for another), truncated `しれ` → 知る 7 |
| hints off | 115 | 104 | 知る 10, 試練 1 |

### 3.2 A verb left on an unfinished form ranks last (`45a3f49`)

Some forms stand only before a particular continuation. `bound_unmet` sends a
bare chain on such a form below every other reading when the text after the
span is not that continuation, and `order` takes that text as a new argument.
It is the verb half of `is_bare_stem`, and the general case of what
`stem_yields` did for 仮定形 before そう.

The forms, and the exception each one needed:

| form | stands before | exception |
| --- | --- | --- |
| 仮定形 | ば, ど | a godan's is also its imperative, so it yields only before は, の, が, を, に, も, へ, や, で; other verbs also keep the ら抜き る, な, た, て, ま |
| godan 未然形 | な, ず, ぬ, ん, れ, せ, さ, ね, ざ, へ | godan only: 一段's and サ変's 未然形 spell their 連用形 |
| 未然特殊 (一段・クレル's くん) | な, ね | — |
| 体言接続特殊 (the ん of `くんだ`) | な, の, だ, で, じ, ね | not 一段, whose ん also spells the negative that dedup dropped |

| | changed | repaired | newly wrong | head |
| --- | --- | --- | --- | --- |
| hints on | 5,866 | ~5,500 | ~10 | `おれ` 居る "Conditional" → 俺 4,124, `くん` 来る → 君 718, `くれ` 来る → くれる 439 |
| hints off | 10,248 | ~8,500 | ~10 | `おれ` 4,029, `のか` 退く → the particle 2,731, `くん` 818, `くれ` 452 |

The rest trade one wrong reading for another on noise or a span already cut
wrong.

Refused on the way:
- **Every 仮定形, godan included**, read about 230 imperatives (`やれ`, `行け`, `出せ`) as nouns.
- **五段・ラ行's empty 体言接続特殊２** read kanji stems cut from their okurigana (`居|て`, `触|った`, ~70) as bare nouns.
- **一段's ん** read `できん」` as 出禁 "ban".

### 3.3 A noun's `uk` counts only on its first sense (`9bd5d33`, `a7a9f24`)

`order`'s PRONOUNCE clause spares a kanji entry's kana reading when JMdict
calls the word usually kana, and `usually_kana` counted `uk` on any sense.
仕手's is on "shite", the noh role written シテ, and it let "doer" lead every
`して` ahead of する's te-form. The snapshot's own `説明してください` showed it.

A word that does not inflect now counts only its first sense. Verbs and
i-adjectives keep any sense. Narrowing verbs read `きた` as 北 "north",
because 来る's tag is on "to come back", and 欲しい's is on its auxiliary
sense. `a7a9f24` makes the POS walk lazy, since it runs inside the sort key.

| | changed | repaired | newly wrong |
| --- | --- | --- | --- |
| hints on | 10,016 | ~8,900 (`して` 6,747, `いや` → 否 ~1,380, `イキ` 575) | ~330 |
| hints off | 10,282 | ~9,250 | ~500 |

**Gold entry top-1: 1201 → 1205 hints on, 1179 → 1184 hints off.**

**The cost, named, because no JMdict field avoids it.** None of 内, 付き or
乗り carries a `stagr` or anything else that separates its tagged sense from
仕手's:
- `〜うちに` "while" reads 家 (~150 per state): 内's tag is on "while" and "among".
- `手つき` reads 吐く (~190 hints off): 吐く is `uk` on its first sense.
- `いやだ` reads 否 (~140).
- `ノリ` reads 海苔 (14).

The user accepted the trade for the ~9,000 repairs.

## 4. Refused, with the measurement

- **PARTICLE above COMMON**, for `もん` → 門 (430). Gold entry top-1 falls 13 hints on and 5 off, with こと → the "command" particle 10,186, よう → よー "hey" 6,067 and して → the particle 6,747.
- **An `aux-v` narrowing of §3.3 for verbs.** 来る's tagged senses are not `aux-v`, and 過ぎる's tag is on its sixth sense.

## 5. Found, measured, not fixed

- **わたし → 渡し "ferry"**, 1,544 hints on. 私 and 渡し differ only in priority grade: ichi1 / news1 / nf01 against news1 / nf21. The index keeps one COMMON bit per form. The nf bands were built at format 7 on `feat/nf-band` (`6e1011f`) and refused as a cost signal. An order-only tiebreak on them would need that index change, and it has not been measured.
- **とる → 摂る and つく → 吐く** where 取る and 付く are meant (89 + 76 in the triage). JMdict tags 摂る and 吐く `uk` on their first sense, so the tag itself decides.
- **とき / とこ → 解く**, 432. 解く's tag is on 梳く "to comb", and the verb any-sense rule must stay for 来る.
- **もん → 門**, 430. The particle's もん reading carries no priority tag, and §4 is what fixing it costs.
- **Single-kanji homographs** decided by sense count: 本 → 元 (119), 金 → きん, 風 → ふう, 数 → すう.
- **Known table classes**, re-counted:
  - 一段 + せる, `出せる` → 出る Causative, 66
  - 見せる as 見る Causative, 62
  - a verb's 基本形 + です, `そうでした` → 添う, `ようです` → 酔う
- **A stem noun shown where the verb is meant** (`向け`, `眺め`, `近づき`, 291 in ranks 260–800), and sense order inside the right entry (`眠れる` leads with the pre-noun sense).
- **§3.2's godan 仮定形** keeps the "Conditional" label on a real imperative (`頑張れ！`). Relabelling it is a separate change.
- **§3.2 reads the continuation in hiragana only**, as `attaches_to_a_stem` does, so a katakana verb before a katakana `ナイ` is not recognised. This was not measured. Folding the continuation would move display order and need its own A/B.
- **The sort key is at twelve fields**, the most a tuple implements `Ord` for. The next clause joins an existing tier or turns the key into a struct.

## 6. Gates

- **Workspace suite:** `cargo test --workspace --features mecab --no-fail-fast`, 699 passed, 0 failed, 1 ignored (the pre-existing `jmdict_source` doc-test).
  - **Use `--no-fail-fast`.** Without it the first failing binary stops the run, and this branch's rewritten unit test would have hidden the snapshot change.
  - The snapshot changes three sentences, all in its order: `して` → する (§3.3), and the tail under `これ` and `から` (§3.2).
- **Lint:** `cargo fmt --all --check` and `cargo clippy --workspace --features mecab --all-targets` are clean.
- **Gold:** 1700 / 1954 / **0.9617 (1205 / 1253)** / 152 hints on, 1590 / 1787 / **0.9610 (1184 / 1232)** / 118 hints off. Every class line is unmoved.
- **Corpus:** the tip's dumps are byte-identical to the measured combination, across all 24 files. No span moves in either state.
- **Bench:** interleaved over five rounds, hints on: 631 ms median at `e22f398` against 638 ms, inside the ~3% layout noise.
- **Gate 6 was not run.** Its eighteen sentences are still not in the repository.
