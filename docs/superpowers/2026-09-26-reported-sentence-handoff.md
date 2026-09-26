# A reported line against the ta-old port, and the two fixes it led to — handoff

**Date:** 2026-09-26
**Branch:** `fix/reported-sentence-glosses`, from `a395bda`: `92a9ccf` (display), `58a8a80` (cost), and this document
**Predecessor:** [`2026-09-23-empty-stem-and-potential-stem-handoff.md`](2026-09-23-empty-stem-and-potential-stem-handoff.md)

A user reported one line of visual-novel dialogue and asked how the ta-old port
(v1, the detached worktree at `361a77f`) and this engine differ on it. The line
is tier C2 by [`gold_sources.md`](../../crates/jparser/tests/fixtures/gold_sources.md)'s
rule, so it is **not** committed: it is the first row of the gitignored
`crates/jparser/tests/fixtures/gold_local.jsonl`, adjudicated at short units,
and scored locally with `jparser-cli score <full index> gold_local.jsonl`. Below,
it appears only as the fragments each finding needs.

Two changes shipped. One visible defect in the line is not reachable by any
change measured here, and it is named in §5 rather than half-fixed.

## 1. Which cells the user compared

The two engines were run in both hint states:
- v1 through the untracked `v1_dump` against its own format-3 index
- this engine through `jparser-cli parse` against a freshly built index and against the cached one, which agreed byte for byte.

| | v1 | this engine |
| --- | --- | --- |
| hints on | `それでも\|し` … `ご一緒しません\|か` | `それでも\|し` … `ご一緒\|し\|ませんか` |
| hints off | `それで\|もし良かったら` … `ご一緒しません\|か` | `それで\|も\|し` … `ご\|一緒\|し\|ませんか` |

**The comparison a user actually makes is v1 hints off against this engine
hints on.** v1's `Settings::default().mecab_hints` is false, and v1 fetched
the dictionary only when the toggle was turned on. This engine has fetched it
on first run since `c0912bc`.

## 2. What each difference is

- **`それでも|し`, hints on, both engines.** IPADIC tokenizes `それでも` (接続詞) + `し` (する 連用形). `cost::is_allowed` is hard, so no span may end at the cut IPADIC did not make. `relax` reopens only a token nothing covers, and `それでも` is covered. The sequence occurs **once** in the twelve corpora's 151,225 sentences.
- **`も|し`, hints off.** `もし` tiles as も + し, both COMMON particles (し is the 接続助詞), so it pays `FUNCTION_STACK`: 5 against 2 for the pair. Of the 292 places IPADIC calls `もし` one token, hints off cuts **all 292**; v1 cuts 7.
- **`ご一緒|し|ませんか` against `ご一緒しません|か`.** Two causes:
  - v1 composes a する-noun with する; this engine cannot, since `vs` is in `table::POS_WITHOUT_A_CLASS`. That is 22,004 spans hints on and 19,605 off where v1 merges and this engine splits. Gold merges 3 of its 2,163 spans that way (`完成する`, `開始する`, `当惑して`), so at the short units the corpus is adjudicated in, this is policy, not a defect.
  - `しません|か` and `し|ませんか` then tie exactly, at -9 hints on and 0 off, and `lattice::beats`' longer-last-step rule picks the expression `ませんか`. The tie exists only because the particle し (1) undercuts する's 連用形 (2).
- **`し` glossed "and; besides".** The lone し led with the conjunctive particle, because `order`'s `PARTICLE` clause puts a particle first wherever the two tie above it. That is 36 spans hints on and 27 off before `ませんか` or `ましょうか`, always the particle.

## 3. Shipped: the verb first before ます (`92a9ccf`)

`assemble::attaches_to_masu` recognizes the text after a span as the polite
ます. It accepts ません, ます, ました and ましょ, and rejects ますます, 増し and
まして. When it matches, a verb whose chain can stand before ます
(`takes_masu`) goes ahead of every other reading.

That means an uninflected match, or a chain ending in 連用形 or 未然形.
未然形 is included because `lex::dedup_shallowest` keeps one chain per span
and entry, and for 一段 and サ変 it keeps the 未然形: counting only 連用形
undid the whole repair (candidate `rs3`). Two other versions were each
refused on a measured class:
- **Any verb.** 五段・ラ行's `体言接続特殊２` has an empty suffix, so every ラ行 verb spells its own bare stem. `イキ|ます` read イキる, `発し|て|ます` 照る "to shine", `だけ|ませんか` 抱く "Conditional" (`rs1`).
- **Any verb but a bare 仮定形.** The ラ行 stems again (`rs2`).

Measured by **entry id**, not headword. The ab scripts key on headword, and
the largest repair keeps its headword: `あり` 蟻 "ant" → 在り.

| | first entries changed | repaired | wrong for wrong | newly wrong |
| --- | --- | --- | --- | --- |
| hints on | 103 | 103 | 0 | 0 |
| hints off | 732 | 725 | 7 | 0 |

- **The 103 hints on** are `あり` 43, `し` → する 36, `ありえ|ません`'s `え` → 得る 9 and `て|ます` → the ている contraction てる 5, plus singles.
- **Hints off** adds `あり` 646, `違い` 10, `頑張り` 8 and `居` おる → いる 4.
- **The 7** sit on spans already cut wrong: `しゃ|ぶり`, `ます|ます`, `弄|び`, `りし`.

No span moves in either state; gold is identical in both.

## 4. Shipped: `FUNCTION_STACK` spares a kana adverb, hints off (`58a8a80`)

`もし` was the reported shape, but the class behind it is wider and three
characters long. The term shattered ordinary `uk` adverbs hints off: `と|て|も`
595 times, `とも|に` 350, `なかな|か` 229, `か|なり` 202, `や|が|て` 102.

A `uk` entry of `UK_MIN_LEN` kana or more, that is not a particle and matched
exactly, no longer pays the term, and only when no tokenizer ran. 1,961
sentences change hints off:
- **Repaired:** about 1,900 spans (the adverbs above, `ついでに`, `もはや`, `なぜか`, `だいたい`, `もともと`, `かつて`, `よほど`…).
- **Made worse:** about 90 — `もしかしたら` → `もしか|したら` 62, `もしもし` → `もしも|し` 17, `まとも|に` → `ま|ともに` 4.
- **Gold hints off** gains `やがて`: 1590 / 1787 / 0.9570 from 1589 / 1786 / 0.9569.
- **Hints on** is byte-identical, by construction.

Refused on the way:

| candidate | what it adds | hints off | hints on |
| --- | --- | --- | --- |
| `fs1` | no length floor | 4,386 sentences; repairs `もし` 388 and `もの` 1,639, but `たしか\|に` → `たし\|かに` "crab" 175 and `ワタ\|シ` 59 | `ワタシ` 60, `ものの` 27, `もしもし` 4 broken |
| `fs2` | the floor, not gated on the tokenizer | as shipped | `もしもし` and `信じがたい` split (10 spans) for 3 repairs |

## 5. Not fixed, and why

- **`それでも|し` hints on.** It happens once in 151,225 sentences. A user-lexicon row would either coarsen `もし|良かったら` against the short-unit reference or reprice `もし` everywhere IPADIC now gets it right (292 of 292). Every row costs a measured A/B under a capped list.
- **`も|し` hints off.** `もし` is two kana. The floor that keeps `かに`, `ワタ` and `もの` from colliding keeps it out too, and nothing in `WordFlags` separates an adverb from a noun.
- **する-noun + する.** Policy, per §2.
- **The `し|ませんか` tie.** Both segmentations are defensible and gold has no `ませんか`. With §3 the glosses are right.

On the reported line, the `し` before `ませんか` now reads する in both
states. The `し` IPADIC's cut leaves at the head still reads the particle,
since nothing like ます follows it. Strict on the local row stays 5 of 9 in
both states and entry top-1 3 of 3.

## 6. Gates

- **Workspace suite:** `cargo test --workspace --features mecab`, 694 passed, 0 failed, 1 ignored (the pre-existing `jmdict_source` doc-test). This includes the snapshot and the 26-row spurious-merge gate.
- **Lint:** `cargo fmt --all --check` and `cargo clippy --workspace --features mecab --all-targets` clean.
- **Gold:** both states with `--compounds`, every class line unmoved.
- **Bench:** interleaved, within the ~3% layout noise, scaling 1.0x.
- **Gate 6 was not run**, because its eighteen sentences are still not in the repository.

## 7. Open

- `イキ|ます` still leads with the noun イキ. The right reading, 行く through the kana fold, is inexact and sorts below every exact one. That is the katakana `イ` class of [`2026-09-22-ta-rerun-handoff.md`](2026-09-22-ta-rerun-handoff.md) §5.
- `ありません` hints off is `あり|ま|せん`, and ある's `あり` has no candidate at all. This is Phase 2's "span but not the entry" note, unchanged.
- The ~90 made worse in §4, of which `もしもし` (17) is the only false gloss; `もしか|したら` is coarse, not wrong.
- The measurement harness this used is the b57d997c scratchpad's (`cand.sh`, `ab.py`, `lab_keys.py`), plus an entry-id comparison written for this session. That comparison is the one that saw `あり`.
