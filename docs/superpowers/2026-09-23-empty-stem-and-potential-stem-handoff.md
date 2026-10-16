# The empty stem, the potential's stem, and four refusals — handoff

**Date:** 2026-09-23
**Branch:** `feat/corpus-disagree`, from `b9210af` through `5194b8a`, one commit per shipped change, each fast-forwarded from its own spike branch
**Predecessor:** [`2026-09-23-godan-potential-handoff.md`](2026-09-23-godan-potential-handoff.md)

The predecessor shipped the godan potential and left five follow-ups it had
measured, in priority order. This session took each on its own spike branch,
measured it over the twelve corpora in both hint states, adjudicated every
changed span by key, and then shipped or refused it.

Three changes shipped: the empty-stem exemption on `PRONOUNCE_CROSS` (task 1),
the potential's stem before そう (task 2), and the suffix-only refusal of the
potential (task 4a). Four were refused and keep their spike branches with the
measurement in the commit: counting only uninflected pieces in a grammatical
stack (task 3), withholding COMMON from potentials (task 4b), the narrow
katakana fold (task 5), and one change the brief did not ask for, blocking
たい's 未然ヌ接続. Over the pool, **3,983 of 151,225 sentences change hints on
(2.63%) and 409 hints off (0.27%)**. Gold moves by one span, a repair.

## 1. Controls

- **The tip's dumps.** `span_dump` rebuilt at `b9210af` with the label patch
  reproduces `spans-v6` byte for byte on `onikiss`, both states. Every A/B
  below is against `spans-v6` or against the previous shipped change's dumps.
- **Gold at the tip**, both states: 1699 / 1953 / 0.9585 (1200/1252) / 152
  hints on and 1589 / 1786 / 0.9569 (1178/1231) / 118 hints off, the
  predecessor's figures exactly.
- **One instrument failure, caught.** The spike worktrees shared one cargo
  target directory, and cargo's fingerprint does not tell worktrees apart: it
  compares the current worktree's source times with the last build of *any*
  worktree. One candidate (`t2b`) therefore came out as the previous
  worktree's binary, byte for byte, and its "results" were another change's.
  It was caught because its A/B was identical to that change's; every other
  candidate showed its own distinct behaviour. The build script now touches
  the worktree's sources before building and fails unless `jparser`
  recompiles.

## 2. Task 1: the cut at an empty stem (`754a7bf`)

`やめて` hints on read やめ | て, the lone て glossed "you said"; `できた`
read でき | た with た glossed たい. 一段's 連用形 and 未然形 are empty, so
IPADIC's stem|auxiliary cut lands exactly on the chain's start rather than
strictly inside it, and `PRONOUNCE_CROSS` charged 25 for it.

**The hypothesis as briefed, measured first.** Exempting the chain-start cut
whenever the innermost step is zero-length changed 5,276 sentences hints on.
The repairs were the class. The collateral was the one-kana stems that collide
with particles: `だけでなく` and `完璧でない` read as 出る (`でなく` 141,
`でない` 100), `わけない` as 分ける (114), `ことにな`, `滅多にない`, `確かにね`,
`女の子にさせて` and `気にせず` as 似る or 似せる (about 300 together), `でね`
as 出る (71). Gold read 1704 / 1958 / 0.9578 (1204/1257).

**What ships adds two halves, each measured.**

- **Only for a word JMdict writes in kana**: `uk` on a sense that is not
  `aux-v` (`Candidate::usually_kana`), the test `lex` already applies to the
  potential. 居る, 止める, 出来る, 上げる, 過ぎる, 苛める and 溢れる are `uk`;
  出る, 似る, 分ける, 生きる and 見る (whose only `uk` sense is 〜てみる) are
  not. This alone took the change to 3,678 sentences and removed every
  particle collision above. It gives back the non-`uk` kana verbs:
  `ふるえ|て`, `つづけ|て`, `はじめ|た` and `ふれ|て` stay split.
- **Not for a chain that ends at ガル接続 or at ぬ's 仮定形.** ない there is な
  and ぬ is ね, the one-kana pair `CHAIN_BLOCK` already refuses on every empty
  stem it names. Without it `それな`, `エロいな` and `やめな` (やめなさい) read
  as a Negative (33 spans newly wrong); with it `できなっ` (11) and `者はいな`
  (2) go back to the split. The ね half came from the independent review; on
  the corpus it swaps `それね` (2, repaired) for `いじめておかねば` (2, newly
  wrong).

**What the brief asked to be measured explicitly.** The five sentences that
refused the 2026-09-18 widening are unmoved in gold: `はこう` (a:004842-3,
は | こう), `はいた` (d:a:003521-6, はい | たが, the pre-existing strict miss),
`いたから` (a:003521-2), `とが` (c1:cas-03, a:003521-2) and `のか` ×3
(d:a:048085-2, a:048085-2, c1:cas-10). Every one has a non-empty innermost
step. **いた → いる**: 334 spans now read 居る Past where 5 did; a lone `い`
was already glossed 居る rather than "the Boar" (the 2026-09-18 `uk` order
clause), and falls 2,692 → 1,788 because `いた` and `いて` became whole.

**Gates.** Gold hints on 1700 / 1954 / 0.9585 (1201/1253) / 152, class lines
82/30/40; one sentence moves, c1:elo-08, where `やめて` becomes one span. Hints
off identical in every cell, and the twelve corpora hints off are
byte-identical. `spurious_merges.txt` 26/26. Tests 688 passed, 1 ignored (two
new: an arithmetic test per half in `cost.rs`, an engine test with the real
tokenizer on the gold index; all six halves mutation-checked). Bench,
interleaved: 644 / 687 / 624 ms before against 619 / 672 / 579 after.

**Twelve corpora hints on**: 3,641 sentences change. Adjudicated by key:

| | repaired | neutral | wrong → wrong | newly wrong |
| --- | --- | --- | --- | --- |
| hints on | 339 keys, 3,837 spans | 5, 20 | 29, 74 | 16, 40 |

The newly wrong: a katakana verb stem severed, `シゴいて` as シゴ + いて (居る),
29 over eight keys; `〜たから` read as the want-to form, 5 (§7);
`それじゃお姉ちゃん` as 逸れる + 姉ちゃん, 2; `いなかったかって`, 2; and
`いじめておかねば`, 2. The adjudicator classed `いて → いる` (225) wrong for
wrong from its first three examples, all `シゴいて`; every occurrence was read,
and 215 follow に, で or が and are repaired, 10 follow a katakana stem. Short
spans: a lone `た` glossed たい falls 1,347 → 537, a lone `て` 5,333 → 3,904.

## 3. Task 2: the potential's stem before そう (`c0c205e`)

`帰れそう` showed 帰る "Conditional". The same spans showed JMdict's 一段
verbs after a godan homograph's bare 仮定形 — `赤らめ|ながら` as 赤らむ,
`照れ|ながら` as 照る, `とけ|そう` as とく, `くれ|そう` as 来る — because the
two tied on every order clause down to the sense count. A tally of every bare
godan 仮定形 by the text after it found a godan "Conditional" shown first in
159 of the 212 before そう, 128 of the 727 before ながら, and 44 before なく.

**The choice, and why.** The brief offered an order/label rule keyed on the
next text, or admitting the bound chain in `search` with a dedup exception.
The bound chain has the same span, flags and interior as the bare 仮定形, so
`cost` prices the two identically and no boundary could move; it would
collapse into the 仮定形 in `dedup_shallowest`; and a table change cannot
reorder the 一段 readings, which are half the class. So it is display, keyed
on the next text: before that text (`attaches_to_a_masu_stem`), `to_segment`
gives a godan's bare 仮定形 the potential's zero-width 連用形 wherever `lex`
would allow the potential, and `order` ranks by `stem_yields` — a bare 仮定形
of any class yields, and a potential yields only to another verb, keeping the
kana `aux-v` exception. **JMdict's `帰れ` "go home"** is an expression, not a
verb; `potential_yields` would have put it first, and a noun cannot take そう,
so there the potential does not yield to it.

**Built and dropped on the way.**

- A bare-form clause that also covered 命令ｅ demoted 呉れる's only reading:
  `lex` keeps one chain per `(len, entry_id)`, and for 一段・クレル the
  survivor is 命令ｅ, so `くれ|そう` read 暮れ "sunset". It covers 仮定形 only.
- A table guard refusing せる on 一段's empty 未然形 (`出せ|そう` reads 出る
  "Causative", which the language does not have). It is right in the table's
  terms, and it cost more than it bought: kana `みせた` (〜てみせる) keeps its
  span only through 見る + せる, since 見せる is not `uk`, and without it
  about 70 spans read みせ as 店 "shop" + た glossed たい. Measured at 651
  sentences hints on, 233 spans gained and 110 lost.

**Gates.** Gold identical in both states. No span moves in either state;
440 sentences change their first entry or label hints on and 494 hints off.
Tests 691 passed (three new, every mechanism mutation-checked). Bench, five
rounds interleaved: 565 / 544 / 540 / 573 / 551 ms before against 571 / 562 /
564 / 577 / 561 after; after `verb_here` was gated on the context, 550 / 557 /
542 against 565 / 564 / 557, minima 537 both. Task 4a's builds, which differ
by one suffix check, read 3 % the other way, so this is the machine's layout
noise.

| | repaired | neutral | wrong → wrong | newly wrong |
| --- | --- | --- | --- | --- |
| hints on | 87 keys, 379 spans | 1, 2 | 9, 49 | 3, 14 |
| hints off | 92, 398 | 0, 0 | 19, 79 | 7, 23 |

The newly wrong are all one pattern, a potential giving way to another verb's
chain or a noun where the potential was meant: `出せ` as 出る Causative (15,
the table's 一段 + せる), `止まれ` as 止む Passive/Potential (5), `擦れ|まくって`
as the noun 擦れ (5), `抜け出せ` as 抜け出る (4), `引け` as the noun (3),
`息を切らせ` (2), `開け`, `即け`, `逸らせ` (1 each). The largest wrong for
wrong is `くれ|そう`: くれる now first, labelled "Imperative" (27 hints on,
30 off).

## 4. Task 3: a grammatical stack of uninflected pieces — refused (`spike/stack-uninflected`, `2b1ac35`)

`なんとも言えない` splits because the COMMON expression tiles into short
COMMON pieces, one of them the conjugated `言え`, so `grammatical_stack`
withholds `LEN_GAIN`, while the COMMON potential `言えない` earns its own.
Counting only uninflected pieces makes the expression a word.

Gold identical in both states, class lines unmoved, `spurious_merges.txt`
26/26 — the refusal the `LEN_GAIN` comment records for `ことになっている` did
not recur. Hints off byte-identical by construction. Hints on, 1,729
sentences change, 1,874 spans gained over 249 keys: 312 repaired, 1,440
neutral, 13 wrong for wrong, 109 newly wrong. The target moves about 21 spans
(`なんとも言えない` 15, `何とも言えない` 6). The rest is a wholesale change of the
stack policy — `でしょうか` 373, `言いながら` 280, `どうした` 224, `よろしくお願い
します` 88 become single expression spans — and the newly wrong are confident
wrong glosses: `ここに来て` read "at this point" where it means "came here"
(35), `なければならない` leaving a bare `し` (20, and 19 more bare `し`),
`とすれば` read "supposing" (13), `言いようのない` read "indescribable" where
the text says "can only be called" (8). No narrower predicate separates the
two: `ことになっている` and `なんとも言えない` both tile with exactly one
conjugated two-character piece.

## 5. Task 4: the potential's hints-off regressions

**4a, the suffix-only refusal, ships (`5194b8a`).** がる (1631750) is `suf`
and `v5r` and nothing else; `takes_a_potential` now refuses an entry every
one of whose senses is a suffix. Hints on byte-identical; hints off 20
sentences: `焦がれる`, `削がれる` and `恋い焦がれる` repaired (3 spans), 36
wrong for wrong (`待ち焦がれ` stays cut, がれ now "Conditional" with a lone た
glossed たい), none newly wrong. Gold identical.

**4b, COMMON withheld from potentials, refused (`spike/potential-no-common`,
`b620c4d`).** The principle is sound — the priority tag marks the base verb,
not its potential — and it is what closes `顔をしかめる`: with the discount
`かめる` (擤む, COMMON through its reading) costs 0 against the expression's 9,
and し + かめる wins 5 to 9 hints off. Without it the potential's own repairs
go: 978 sentences hints on, 3,077 hints off, the potential class 752 → 1,498,
`もらえ|た` 161, `払え|る` 463, `もら|える` 309.

**Left open**: `顔をしかめる` (56), `気が引ける` (27) and `差しのべる` as 忍ぶ
(6), the COMMON arithmetic no tokenizer constrains.

## 6. Task 5: the narrow katakana fold — refused (`spike/katakana-stem-fold`, `dd4a666`)

`search` folds katakana to hiragana only under an auxiliary, only for the
headword's own form, never for 基本形 (`ナスです` is not 為す), and never
through a katakana auxiliary. Gold identical. **Hints on it works**: 273
sentences, 276 spans repaired (`イキたい` → いく 85, `シて` → する 43,
`キてる` → 来る), 10 newly wrong. **Hints off it does not**: 240 sentences,
104 repaired against 58 newly wrong, most of them confident wrong glosses —
`ココなら` as コ + 来る (7), `マシな` as マ + する, `デキちゃう` as デ + 来る,
`いいよ` as い + いる. A change that measures well only with a tokenizer does
not ship. And **`イカせて` does not move** either way: 行く's causative becomes
a candidate, but it is inexact, and `order`'s first clause puts the exact
katakana headword イカす first.

## 7. An extra spike: たい's 未然ヌ接続 — refused (`spike/tai-mizen-nu`, `b569551`)

Found adjudicating task 1: たい's 未然ヌ接続 is `たから`, spelled like the past
`た` + `から`, so `くれたから` reads くれる **Want-to** — 458 spans at the tip,
every one the past. Blocking the form (walk 133,802 → 133,219, one fixture
row removed with a note) frees `から` (252) but splits the span into worse
pieces, because JMdict's `たい` is itself an adjective headword whose own
未然ヌ接続 is `たから`: `くれたから` becomes くれ → 来る Conditional + たから →
たい (45), and `〜たからな`/`からね` read as かる (26). Closing it needs the
headword's own form refused in `lex` as well.

## 8. Found, measured, not fixed

- **The table's 一段 + せる** (`ATTACHES_TO` names 未然形 by name): `出せ|そう`
  as 出る Causative (15), `抜け出せ` (4). Refusing it costs kana `みせた`
  (§3), which rides on the same chain because 見せる is not `uk`.
- **〜たから as the want-to form**, 458 + 5 (§7).
- **A katakana verb stem severed by 居る**, `シゴいて` (29, task 1).
- **Non-`uk` kana 一段 verbs still split at the stem**: `ふるえ|て`,
  `つづけ|て`, `はじめ|た`, `ふれ|て`.
- **`くれ|そう` labelled "Imperative"** (27 / 30): `dedup_shallowest` keeps
  一段・クレル's 命令ｅ over its 連用形, which spell the same kana.
- **A godan imperative at the end of a line reads "Conditional"**: the bare
  仮定形 survives dedup over 命令ｅ, so `頑張れ！` and `くれ！` show
  "Conditional" first (about 60 before `！」` alone).
- **`おれ|たち` reads おる "Conditional"** first, 140 hints on: 俺 is kanji and
  PRONOUNCE demotes it.
- **`イカせて`** (150): the order question in §6, not the table.
- **`なんとも言えない`** (42) and the hints-off COMMON arithmetic (§5).

## 9. State of the branch

| commit | change | § |
| --- | --- | --- |
| `754a7bf` | the cut at an empty stem of a word JMdict writes in kana | 2 |
| `c0c205e` | a godan's bare 仮定形 before そう is the potential's stem | 3 |
| `5194b8a` | the potential refused for an entry that is only a suffix | 5 |
| *(this document)* | the handoff, and the master plan's §2 and §5.2 figures refreshed | — |

Refused spikes, each with its measurement in the commit: `spike/stack-uninflected`,
`spike/potential-no-common`, `spike/katakana-stem-fold`, `spike/tai-mizen-nu`.
`README.md` and `src-tauri/Cargo.toml` carry the user's own uncommitted edits
and were not touched. The dumps (`spans-v7` is this tip's), the adjudication
TSVs and the scripts are in the session scratchpads, not the repository.
