# The godan potential, deinflected — handoff

**Date:** 2026-09-23
**Branch:** `spike/godan-potential`, from `7b9eee6` (`feat/corpus-disagree`); one feature commit, `68c3bd1`, then this document
**Predecessor:** [`2026-09-23-twelve-corpora-table-handoff.md`](2026-09-23-twelve-corpora-table-handoff.md)

The predecessor's §7 named the godan potential the largest class the probe had
found and left it for a phase of its own: `読めた` parsed 読め "Conditional" +
た glossed たい, `会えなかった` 会え + なかった, because IPADIC files 可能動詞 as
一段 lexemes and no chain reached the godan lemma. It predicted the hazards —
`LEN_GAIN` rewarding longer chains, カ変's `くれ`, `イカせて`, the 一段/五段
pairs — and asked that each be measured rather than reasoned about.

This session built it on a spike branch and measured every stage over the
twelve corpora in both hint states. The mechanism is a zero-width table row.
On its own it repaired thousands of spans and also produced a few hundred
confident new wrong glosses, and every one of those was traced to a cause and
closed by a rule that was itself measured: two refusals in `lex` and one
ordering clause in `assemble`. What ships changes **3,700 of 151,225 sentences
hints on and 3,430 hints off**. Every changed span was adjudicated. **Hints on,
3,501 spans are repaired and 57 are newly wrong; hints off, 3,475 against
115.** Gold moves by one span, a repair. The class itself falls from **3,403
occurrences to 752**, and most of the 752 are not potentials.

## 1. Controls

- **The tip's dumps.** `span_dump` rebuilt at `7b9eee6` over all twelve
  corpora, both states, is byte-identical to the predecessor's `spans-v5`, all
  24 files. Every A/B below is against those dumps.
- **Gold at the tip**, both states: 1698 / 1952 / 0.9584 (1199/1251) / 152
  hints on and 1589 / 1786 / 0.9569 (1178/1231) / 118 hints off, the
  predecessor's figures exactly.
- **The class at the tip.** The predecessor's `pot.py` over the tip's dumps
  reads 3,403 occurrences in 3,280 sentences. The predecessor wrote 3,393 in
  3,273; that came from a slightly different dump set run through the same
  script.

## 2. The mechanism, and why it is a table row

The potential is an `ATTACHES_TO` row, `("る", "一段", "仮定形")`. The godan's
own 仮定形 already supplies the e-row kana (読め), so what the potential adds
conjugates exactly like 一段 over an empty stem: `る`, `れ`, `りゃ`, and **an
empty 未然形 and 連用形**. The empty forms carry the class: 読め + ∅ + た,
読め + ∅ + ない. `Table::new` used to skip an empty surface. It now inserts one
at the trie's root as a zero-width hit. `search` takes a zero-width hit only
when an auxiliary required that form, so it never ends a chain, and never
twice in a row: the recursion it starts passes `zero_width = false`. Every
level therefore either consumes a character or makes one zero-width hop that
cannot be followed by another, and the walk terminates whatever rows are
added. The independent review checked that argument against the code and
agreed.

**Why a row and not the brief's other option**, a fallback in `lex` from a
一段 key ending in e-row + る to the godan lemma. The chain is what `label.rs`
reads, what `cost` uses to exempt an inflection-internal cut, and what the
fixture walk enumerates. A `lex` fallback would put a godan entry under 一段
steps, a candidate `allows_chain` refuses by construction, and every reader
downstream would need a special case for it.

**Godan only.** `ATTACHES_TO` names a site by form alone, so
`potential_attaches` restricts the row to a headword's own 仮定形 in eleven
五段 classes. It leaves out 五段・ラ行アル (ある's `あれた` is 荒れた) and
五段・ラ行特殊 (`なされた` is なさる plus the honorific れる). The row never
lands on カ変's `くれ`, on 一段's `食べれ`, or on any auxiliary's own 仮定形.

**Blocked forms.** 命令ｙｏ and 未然ウ接続 are both `よ`, and `待てよ` is an
imperative plus the particle. 命令ｒｏ is the potential's own imperative,
which the language does not have. 体言接続特殊 `ん` would tie with the
negative `読めん` and win on depth, reading "can't" as "can". In `CHAIN_BLOCK`,
`ぬ` at 仮定形 and `ない` at ガル接続 are refused for the particle collisions
`てる` already had, and causative, passive and want-to are refused outright:
`読めさせる`, `読められる` and `読めたい` are not Japanese.

The label is **"Potential"**, not "Passive/Potential", because 読める cannot be
a passive. The fixture gains ten rows. The reachable-tuple walk goes from
127,432 to **133,802** (190 + 5,001 + 128,611). That is +105 depth-1, the
row's five surfaces at each of the 21 mapped classes with a 仮定形, and +6,265
depth-2 behind them.

## 3. What the bare mechanism did

The row alone changed 5,808 sentences hints on and 4,553 hints off. The gains
were exactly the class: `もらえた`, `会えなかった`, `帰れる`, `思い出せない`. So
were the newly wrong glosses. The worst of them follow.

- **Hazard 1 at full strength.** A kana potential crossed IPADIC's cut
  between the e-row kana and the auxiliary for nothing, because the cut lies
  strictly inside the chain and `cost` exempts such a cut, and then
  `LEN_GAIN` paid for the length. `わけない` became one span read 湧く Potential
  Negative, with 分ける shown first (122 hints on, where `わけ|ない` had been
  right). `していただけませんか` was cut `いた|だけません` and read as 抱く (19),
  and hints off `ありません` read ません as 増す "to increase" Potential Negative
  (317).
- **Expressions.** `口を開けて`, `腰を浮かせて` and `目を開ける` became the
  potential of 口を開く, 腰を浮かす and 目を開く (about 65).
- **The dictionary's own words shown second.** `弾ける` read 弾く Potential,
  `擦れて` 擦る, `掴まれた` 掴まる Potential, `いけない` 行く Potential Negative
  (about 400) and `とける` 解く.

## 4. Two refusals in `lex`

`EntryFacts::takes_a_potential` refuses the potential in two cases.

- **For an entry with an `exp` sense.** This removes the expression merges
  above. It also returns the few genuine ones to the split they had
  (`目を離せない`, `好きになれる`, `役に立てる`).
- **For the kana reading of an entry with no `uk` tag on a non-`aux-v`
  sense.** The kana verbs whose potential is common in running text — 貰う,
  行く, 成る, 頂く, 遣る — are tagged `uk` in JMdict and keep it. 湧く, 抱く, 貸す
  and 増す are not, and lose it. The flag `WordFlags::UK` is not used because
  it also drops every entry with an `aux-v` sense, and those five all have one.
  The `aux-v` clause was added last: 腐る's only `uk` sense is the 〜くさる
  suffix, and counting it read hints-off `良くされる` as 良 + 腐る Potential,
  about 40 sentences. Its cost is 置く, whose `uk` senses are all 〜ておく, so
  `放っておけない` goes back to the tip's split, 4 sentences hints on.

**What the second refusal gives back, and why it is still right.** Without
it, the potential of 止む crossed `やめ|て` for free, and so the 一段 verb やめる
was kept whole. About 400 spans of kana 一段 verbs were repaired this way
(`やめて`, `ふるえて`, `つづけて`): `PRONOUNCE_CROSS` splits these at the stem,
and the repairing candidate is one nobody displays. They go back to the tip's
split. The defect is in `cost` (§8), and a side door that also opened
`わけない` is not the way to fix it.

## 5. Which reading goes first

`order` gains one clause, placed above `COMMON`: `potential_yields` in
`crates/jparser/src/engine/assemble.rs`. It moves only potentials; in a group
without one it is false throughout, which the review confirmed. Each
placement below was built, dumped and A/B'd against the one before it. A
variant with the kanji clause and no kana clause was built as well; it read
`いけない` and `とける` as the potentials again.

| variant | what changed | measured |
| --- | --- | --- |
| no clause | — | `いけない` → 行く ×400, `とける` → 解く, `弾ける` → 弾く, `掴まれた` → 掴まる |
| after `PARTICLE` | a potential yields to every other reading | fixed those; `なれ-` → 慣れる, about 300, where the text says "can become" |
| + kanji spellings above `COMMON` | `弾けた`, `擦れて`, `塗れた`, `解れている` fixed | `満たせる`, `抜け出せない` back to the tip's wrong causative (about 15) |
| rank by verb / non-verb | `なれ-` fixed | **reordered non-potentials**: `くれない` showed 紅 over くれる, 202 |
| yield flag, `aux-v` exemption | only potentials move | `なれた` back to 慣れる, 38, via the `adj-f` entry 慣れた |
| `adj-f` counts as a verb | `なれた` fixed | kana intransitives (`こすれる`, `めくれる`, `かすめる`) still lose to a COMMON transitive |
| one clause above `COMMON`, exemption for kana readings only | `こすれ-` 28, `めくれ-` 11, `つなげ-` 6, `かすめ-` 5 fixed | the ておく contraction `とく` and 捲る are `aux-v` and spelled as their own headwords; exempting them read `とける` as とく (55), so the exemption is kana-reading only |

The rule that ships says a potential yields to the span's other readings. It
does not yield when it is the kana reading of an `aux-v` verb and every other
reading is a verb, `adj-f` included. Then `PRONOUNCE` decides, and it demotes
慣れる, which JMdict writes in kanji. Against `いけない`, which is not a verb, it
still yields.

## 6. The brief's four hazards, measured

1. **`LEN_GAIN` and longer chains.** This produced the newly wrong spans in
   §3, and §4's refusals closed them. One case remains open and is counted
   against the change. `なんとも言えない` (COMMON, 8 characters) tiles into
   COMMON pieces of two characters or fewer with a particle, so
   `grammatical_stack` withholds `LEN_GAIN` from it. The new COMMON potential
   `言えない` earns its own, and the compound splits: 42 spans hints on, all
   with correct pieces.
2. **カ変's `くれ`.** No chain puts the potential on カ変, 一段 or an
   auxiliary. A table test and the engine test cover this, and the table
   test fails with the guard removed. The corpus heads
   show no くれ → 来る Potential. The one `くれない` defect found (202) came from
   an ordering variant, not the table, and it is fixed (§5).
3. **The predicted false chains.**
   - `イカせて`, `イカせちゃう`, `イカせる` (150 hints on) are **wrong → wrong**.
     The tip read イカす "cool" Conditional; now it is イカす Potential. イク's
     causative cannot be reached, because the table is hiragana and the stem is
     katakana. The engine test pins the behaviour as known.
   - `擦れて` and `塗れた` are **repaired**. The tip split them and showed 擦る
     and 塗る; now they read as 擦れる and 塗れる, through the kanji clause.
   - `かえた` is refused. 買う carries no `uk`, so `かえた` is 変える.
4. **The mechanism** is written down in §2 and in the row's comment.

## 7. Gates

| | strict | tolerant | entry top-1 | compounds |
| --- | --- | --- | --- | --- |
| tip, hints on | 1698 | 1952 | 0.9584 (1199/1251) | 152 |
| **now, hints on** | **1699** | **1953** | **0.9585 (1200/1252)** | 152 |
| tip, hints off | 1589 | 1786 | 0.9569 (1178/1231) | 118 |
| **now, hints off** | 1589 | 1786 | 0.9569 (1178/1231) | 118 |

The compound class lines are identical in both states. One gold sentence
moves: in `やっとかなきゃいけないこと`, `いけ|ない` becomes `いけない`.

- **Tests.** `cargo test --workspace --features mecab` reports **686 passed,
  0 failed, 1 ignored**. That is 680 plus six new ones: three in `table.rs`
  (reach, godan-only, particles and impossible stacks), one each in `lex.rs`
  and `assemble.rs`, and the fixture-backed collision test in
  `tests/engine_v2.rs`.
- **Mutation checks.** Every mechanism was disabled in turn and a test
  failed: the godan guard, the four blocked forms, the two `CHAIN_BLOCK`
  groups, the zero-width top-level guard, both refusals, the ordering clause,
  its `aux-v` exemption, the kana-only half, the `adj-f` branch, and the
  rule that a `uk` tag on an `aux-v` sense does not count. The review had
  flagged the `adj-f` branch as untested.
- **Static checks.** `cargo fmt --all --check`, `cargo clippy --workspace
  --features mecab --all-targets` and `cargo check -p jparser
  --no-default-features` are clean.
- **Snapshot.** The curated snapshot moves in two places. `話せ|ます|か`
  becomes `話せます`, with 話せる Polite first, and `開けて` gains two 開く
  Potential siblings below 開ける.
- **Bench.** `jparser-cli bench` was run interleaved, four rounds, at 10,040
  characters. The tip read 626–647 ms and the final build 631–675 ms; scaling
  was 0.93–1.01× against 0.89–1.03×. The ranges overlap, and the figures are
  readings from the moment they were taken.

**The twelve-corpus A/B, tip against final.**

| | changed sentences | spans gained | spans lost | same span, new top entry |
| --- | --- | --- | --- | --- |
| hints on | 3,700 (2.45%) | 3,680 (974 keys) | 7,111 | 371 (13) |
| hints off | 3,430 (2.27%) | 3,590 (933) | 6,684 | 378 (44) |

**Every changed span was adjudicated**, by key (surface → top entry, with the
conjugation label), against three contexts each. Four agents covered the
heavy and tail halves of each state, and I classified the 43 keys the last
variants introduced. I read the heads myself.

| | repaired | neutral | wrong → wrong | newly wrong |
| --- | --- | --- | --- | --- |
| hints on | 902 keys, 3,501 spans | 30, 321 | 44, 214 | 11, 15 **+ 42** |
| hints off | 821, 3,475 | 14, 24 | 116, 354 | 26, 115 |

The +42 is `なんとも言えない` (§6.1). The adjudicator classed it neutral, since
the pieces are right; it is counted against the change here. The hints-on
newly wrong spans are listed below.

- Compound verbs JMdict lacks as 一段, now read as the godan's potential:
  `読み進める` (4), `突き進める` (3), `まき散らせる` (2), `焼き切れた` (1).
- `慣れていない` read as なる, 2.
- Two compound cuts next to a changed span: `抱きよせる`, `吐きかける`.

The hints-off 115 are listed below.

- `顔をしかめる` split into し + 擤む Potential, 56.
- `気が引ける` split, with 引ける "to close" first, 27.
- `焦がれる` and `削がれる` split through がる's potential, 8.
- `差しのべる` read as 忍ぶ, 6.
- `なくなる` swallowed into なり, なると or なるほど, 6.
- `いけなかった` and `いけねえ` read as "couldn't go" where they mean "must",
  3; `慣れていない` read as なる, 4; a stray `たし`, `たん` or `ろう` beside a
  changed span, 4; `あえて` read as 和える, 1.

All of the hints-off ones are the COMMON arithmetic that no tokenizer
constrains: a COMMON godan reading of a short piece makes a split cheaper
than a longer word, the defect `cost.rs`'s `COMMON` and `MATCH` comments
describe.

**First glosses on short spans.** Tallied per (surface, `entries[0]`) for
every span of two characters or fewer. Only fragments shrink: lone `た` → たい
goes 1,792 → 1,347 hints on and 1,177 → 931 hints off, `いけ` → 行く 437 → 15,
and `なれ` → なる 295 → 111. The only increases are particles and whole words
(`か` +33, `に` +25, `を` +21 hints on; `か` +60, `の` +54 hints off). No new
wrong first gloss appears.

## 8. Found, measured, not fixed

- **Kana 一段 verbs split at the stem**, about 400 spans (`やめ|て`,
  `ふるえ|て`, `つづけ|て`). An empty innermost 一段 step puts the stem|aux cut
  at the chain's start, where `PRONOUNCE_CROSS` charges it. The 2026-09-18
  widening to the chain's start was refused over `はこう`. The untried narrow
  form exempts that cut only when the innermost step is zero-length, and it
  has not been built.
- **A potential stem before `そう` or `なくなる`**: `帰れ|そう`, `出せ|そう`,
  `会え|なくなる`, about 250 of the 752 left. The zero-width step cannot end a
  chain, so `帰れ` keeps the tip's 帰る "Conditional".
- **`イカせて`**, 150, wrong → wrong (§6.3).
- **`なんとも言えない`**, 42 (§6.1). The interplay is between
  `grammatical_stack` and a COMMON potential piece, not the potential itself.
- **Compound verbs JMdict lacks as 一段** (`読み進める`, `突き進める`): the
  godan's potential is the only reading the span has.
- **Hints off**: the `顔をしかめる`, `気が引ける`, がる and 忍ぶ splits above. A
  refusal for suffix-only entries such as がる was considered and not built,
  at 8 spans.
- **`作れんの` and `頼れんの`**, 〜るの contracted, read as Potential Negative:
  3, the price of blocking 体言接続特殊.
- **`喜びあえる` and `向きあえない`**: 合う carries no `uk`, so its kana
  potential is refused and 和える or 敢えない shows (about 10, wrong → wrong).
- **JMdict lexemes whose first sense misleads**, now shown first where they
  are the dictionary's word: `聞ける` (archaic "to tell"), `行ける` ("to be good
  at"), `眠れる` ("sleeping"). The hints-off adjudicator classed these rows
  wrong → wrong or repaired. The fix belongs to the dictionary.

## 9. State of the branch

| commit | change |
| --- | --- |
| `68c3bd1` | the godan potential: table row, zero-width search, `lex` refusals, `order` clause, label, tests, fixture, snapshot |
| *(this document)* | the handoff |

`README.md` and `src-tauri/Cargo.toml` carry the user's own uncommitted edits
and were not touched. Every build's dumps, the adjudication TSVs, the A/B
outputs and the scripts (`gp_keys.py`, `gp_short.py`, `ledger.py`) are in the
session scratchpads, not the repository.
