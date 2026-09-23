# Twelve corpora, the conjugation table's over-generation, and the gloss nobody measured — handoff

**Date:** 2026-09-23
**Branch:** `feat/corpus-disagree`, from `a849d7f` through `37460aa`, one commit per change
**Predecessor:** [`2026-09-22-katakana-sokuon-handoff.md`](2026-09-22-katakana-sokuon-handoff.md)

Every handoff since 2026-09-21 ended with "`vn-text` is still exhausted". It was
not: two of its fourteen files had never been run. This session ran them, then
asked the pool three questions no earlier session had asked — where does the
ta-old port keep *any* JMdict content word whole (not only `uk`), where does
this engine swallow a particle the port leaves alone, and what does a reader
actually see first on the short spans the engine leaves behind.

The answers were mostly not lexicon rows. They were four places where
`table.rs`'s attachment rules generate chains Japanese does not have, one
place where `order` ranks a bare adjective stem above a verb, and a copula
that could not conjugate. Seven changes shipped, one commit each; two more
were built, measured and refused; the largest class found — the godan
potential — is measured and left for its own phase.

## 1. Controls

- **The dump recipe, corrected.** The documented recipe ("whitespace stripped,
  internal newlines removed") does not reproduce the pool: `onikiss` differs
  from line 12, `kf3` from line 4,128. What does is **every whitespace run —
  newlines and U+3000 included — collapsed to one ASCII space**
  (`' '.join(source.split())`), distinct non-empty rows, in rowid order. That
  reproduces six corpora byte for byte (onikiss, kf3, enkou, hitoduma2, start,
  pxc2).
- **The tip's span dumps.** `span_dump` rebuilt at `a849d7f` over `onikiss`
  is byte-identical to the predecessor's shipped-fix dump, so its ten-corpus
  dumps are this tip's.
- **The v1 side.** `v1_dump` re-run over `onikiss` against the predecessor's
  format-3 index is byte-identical to its dump.
- **Gold at the tip**, both states: 1696 / 1950 / 0.9528 / 152 hints-on and
  1588 / 1785 / 0.9504 / 118 hints-off, reproducing the predecessor exactly.

## 2. Two corpora nobody had run

`amanee` (9,279 distinct sentences) and `torawase` (10,099) sit in `vn-text`
beside the ten and appear in no document. Neither overlaps the others beyond
40 lines of bare interjections. The pool is **151,225** sentences.

| corpus | merge direction | split direction | ratio |
| --- | --- | --- | --- |
| amanee | 1,164 | 2,271 | 1.95× |
| torawase | 1,553 | 2,817 | 1.81× |

The sign holds on two more writers. Their heads are the known policy
(`じゃ|ない`, `んだ|けど`, `くれてる`) plus two classes the ten corpora had
under-weighted: `気持ち|よ|さ` and `い|た`.

## 3. Three questions the ranking cannot ask

- **General lexeme sweep**: every span the port keeps whole whose surface is
  a JMdict content headword or reading, which this engine cuts into pieces
  that are not a particle stack. **11,176 occurrences over 1,303 surfaces** at
  the tip. Most of the head is policy or the port's own error — `から|だ`
  "because" (the port reads 体 "body"), `〜に|なる|ほど` (the port reads
  なるほど), pronoun + たち. The false glosses in it are §5.2's rows and §7's
  `-さ` class.
- **Particle-swallow sweep**: our span ends in a particle, and the port joins
  our left neighbour to its head and leaves the particle alone. 1,361
  occurrences. Most are the auxiliary policy (`ですか`, `ませんか`); the real
  ones are `兄|様に` "like; as" 44, `中出|しの` 死ぬ 16, `姉|上は` "now that" 11.
- **What a reader sees first on a short span.** This is the question that
  found the most. Every lone `した` in the pool — **3,942 of 3,942** — led
  with `したい` "want to do"; every lone `た` — 2,023 — with `たい`; a lone `う`
  after `でしょ` with 卯 "the Rabbit (zodiac)". None of it is visible to span
  accuracy, and the port's spans cannot show it either.

## 4. The table generates chains Japanese does not have

`ATTACHES_TO` names an attachment site by **form name alone**, so every class
with that form satisfies every row that asks for it. Four rows were wrong in
ways that one rule or one class change fixes.

### 4.1 Voicing at the onbin — `onbin_voicing_agrees`

`で` and `だ` attached to *any* 連用タ接続, and 五段・カ行イ音便's is a bare `い`.
`ない` is な + い, and な + く is 泣く, so **`しないでください` read `ないで` as
泣く Te-form and `わけないだろう` read `ないだろ` as 泣く Past**. Over the pool:
645 such spans in 641 sentences — `ないで` 317, `ないだろ` 226, `せいだ` as
せく "to hurry" 26, `おいで` as 置く 23. The voiced auxiliaries now attach only
after 五段 ガ/ナ/バ/マ行, and the unvoiced ones only elsewhere, keyed on the
auxiliary's first kana. 890 sentences change hints-on; `ついた` reads つく
instead of つぐ 62 times, and `かいでる` reaches かぐ.

### 4.2 The voiced past is 特殊・タ

The `だ` row named the copula's class 特殊・ダ, whose forms are なら/な/なれ/だ/
だろ/だっ/で. IPADIC itself tags 読ん|だ and 読ん|だら as **特殊・タ**. So
**`読んだら` was 読|ん|だら with 読 unmatched** — `飲んだら` 45, `死んだら` 18,
every ガ/ナ/バ/マ-row conditional — while `読んなら`, `読んな`, `読んなれ` and
`読んだっ` were generated, and `もんな` read 揉む 57 times. The Phase 1b-i label
fixture had recorded this as a "table finding" and left it. 343 more sentences
hints-on; the fixture's five spurious rows are removed with a note, and
`遊んでた` now labels Progressive Past through the `でる` route.

### 4.3 `でしょ` is blocked on the `です` row

IPADIC's `う` attaches to 特殊・デス's 未然形 (でしょ|う), which no row could
follow, so every `[verb]でしょう` parsed `行くでしょ | う` with 卯 "the Rabbit".
Blocking the form lets `でしょう` be JMdict's own headword, the way `だろう`
already is: 791 sentences hints-on, 310 lone `う` gone, gold identical.

**Refused, measured twice: letting `う` follow instead.** With 特殊・デス, ダ and
タ as IPADIC has it, `ないだろう` became one span read 凪ぐ 100 times and hints-off
lost a gold span. With 特殊・デス alone, `大変だったでしょう` became **だる** — JMdict
2867372, the 〜てやる contraction, `v5r` — Past Polite Volitional where the split
had been right, and gold hints-off still lost a span. The cause is `LEN_GAIN`:
it is quadratic in length, so every chain a table change makes *longer* is
paid for far more than the common pieces it replaces. **Any future change that
extends chain reach needs the corpus A/B before anything else.**

### 4.4 The じゃ/や placeholder rows

The doc said they named 基本形 "as a placeholder … without pretending either
row will ever actually fire". Both fired after every 基本形, the te-form's
`て` included: `そうやって` read そう **Copula Te-form** and `こうやって` the same
(224 spans), `みたいじゃない` 見る Want-to Copula, `臭いやベトつき` swallowed the
particle `や`, `ましてや` read ます. Removed, with the coverage test exempting
the three copula classes. **The one change this session that moves gold:
+2 strict, +2 tolerant, +2 entry hits hints-on, +1 each hints-off.** 891
sentences hints-on.

## 5. The other three

### 5.1 A bare adjective stem ranks after a real reading — `order`

`したい` is an `adj-i` headword, its ガル接続 stem is `した`, and `order`'s
shallower-chain clause read one step as likelier than する's two. A new clause
ranks a bare ガル接続 stem after every other reading **unless the next text is
what attaches to one** (さ, そう, げ, すぎ, め, み, がる's forms, and the
adjective's own い/かっ) and **unless the stem ends in kanji**. Order runs after
the DP, so no span can move. 3,898 sentences hints-on, `した` → する 3,903,
`愛し続けて` → 愛する 34; gold entry top-1 **+6 hints-on, +7 hints-off**.

Refused on the way: the unconditional clause (≈220 genuine stems lost —
`苦し|そう` read 苦い 82, `くすぐった|さ` くすぐる 38), and the version without
the kanji gate (hints-off, 220 single-kanji stems inside cut compounds —
`内太|腿` read 太る 155 — no better than before).

### 5.2 Four user-lexicon rows, cap 111 → 115

`きもち` (き|もち "spirit + rice cake", 103 of 121), `兄様`, `姉上`, `兄上`
(the particle swallow of §3). 181 sentences hints-on, 203 occurrences
recovered, 3 lost (`不思議とお兄様` → `とお`), gold identical, hints-off
untouched by construction. **`よさ` was built and refused**: the span comes
right and leads with 止す "to stop", a `uk` verb's 未然形 the PRONOUNCE clause
does not demote.

### 5.3 The copula conjugates as `でした`

`cop` has no class, so `雨でした` read 雨 | でし "pupil; disciple" | た "want to
do", 231 times. `です` now conjugates in 特殊・デス as its own headword **only**
as でし + た, **only** hints-on, and only where IPADIC kept `でし` one token
(`cost::is_allowed`) — IPADIC separates the copula from particle + する 504 to
20. `でして` and `でしたい` stay refused: IPADIC tokenizes `みんなでしたい` and
`口でして` as the copula too. 346 sentences hints-on, 253 `でした` → です Past;
hints-off byte-identical.

## 6. Gates

| | strict | tolerant | entry top-1 | compounds |
| --- | --- | --- | --- | --- |
| tip, hints-on | 1696 | 1950 | 0.9528 (1190/1249) | 152 |
| **now, hints-on** | **1698** | **1952** | **0.9584 (1199/1251)** | 152 |
| tip, hints-off | 1588 | 1785 | 0.9504 (1169/1230) | 118 |
| **now, hints-off** | **1589** | **1786** | **0.9569 (1178/1231)** | 118 |

`cargo test --workspace --features mecab`: **680 passed, 0 failed, 1 ignored**
(the pre-existing `jmdict_source` doc-test; 674 before, plus six new: the
voicing rule, the でしょ block, two for the bare-stem clause and two for the
copula gate); `cargo fmt --all --check`, `cargo
clippy --workspace --features mecab --all-targets` and `cargo check -p jparser
--no-default-features` clean. The curated snapshot moved in one line:
`からい` "spicy" leaves the top five of `東京から`'s `から`, whose first entry is
unchanged. `the_label_fixture_covers_every_reachable_tuple` reads 127,432
chains (from 145,482), each step's delta written into the constant's comment.

Over the pool, tip against now: **7,219 of 151,225 sentences change hints-on
(4.77%), 6,690 hints-off (4.42%)**. Every change was A/B'd on its own, both
states; the head of each A/B was read, and the losses named above are all of
them that were not already wrong at the tip. Hints-off carries a handful of
wrong-for-wrong swaps — `こ|うなったら` now 唸る where it was 請う, `こうや|って`
where it was こう Copula, `わけない` 分ける where it was 泣く — in a state whose
kana regions were already garbage.

## 7. Found, measured, not fixed

- **The godan potential — the largest class found.** `読めた` is 読め
  "Conditional" + た, `書けない` 書け + ない, `呼べる` 呼べ + る: IPADIC files
  可能動詞 as their own 一段 lexemes, so no chain reaches 読む. **3,393
  occurrences in 3,273 sentences**; the port keeps 2,829 whole; **1,535 have no
  JMdict entry** (`会え|なかった`, `戻れ|ない`, `帰れ|る`), and the rest have one
  and lose anyway (`読める` is in JMdict, `読めた` still reads 読め|た). The
  shape that fits `table.rs` is a zero-width potential auxiliary on 五段 仮定形,
  godan only. It is its own phase because §4.3's lesson applies at full
  strength: `イカせて` (127 in the pool) would reach JMdict's `イカす` "cool"
  Potential, and every 一段/五段 pair whose 一段 is not COMMON (`擦れて`,
  `塗れた`) would read the godan's potential.
- **Lone `た` → `たい`, 2,023.** The past auxiliary (2654250) carries no
  priority tag, so COMMON puts たい first; §5.1 cannot reach it. It is a symptom:
  379 follow a potential stem, 336 `い|た`, 184 `でき|た`.
- **`ないだろ` → 凪ぐ, 93 hints-on.** It was 泣く at the tip; the chain's
  interior boundary is inside the inflection, so `PRONOUNCE_CROSS` is exempt.
- **`くん` → 来る (体言接続特殊)**, 14 hints-on, exposed where §4.2 split
  `くんな`. The honorific 君 is PRONOUNCE and demoted.
- **The `-さ` nominal**: `柔らか|さ` 94, `熱|さ` 68, `美し|さ` 16. The glosses are
  right (`さ` leads with "-ness") except `熱`, which reads ねつ.
- **Rows not attempted**: `中出し` (154 cut, 中出 unmatched, `しの` 死ぬ 16),
  `せんぱい` (33, one corpus), `けっきょく` 25, `あたま` 13, `ちくび` 13,
  `わしづかみ` 18, `ぞくり` 21.
- Everything the predecessor left, untouched.

## 8. State of the branch

One commit per change, in the order they were measured, and every commit
builds and passes the full suite and both gold states on its own — each was
re-gated from its own tree before it was made:

| commit | change | § |
| --- | --- | --- |
| `88f5557` | voicing at the onbin | 4.1 |
| `62b372f` | the voiced past in 特殊・タ | 4.2 |
| `337f2e8` | `でしょ` blocked on the `です` row | 4.3 |
| `39d08a9` | a bare adjective stem ranks below a real reading | 5.1 |
| `2683679` | four user-lexicon rows | 5.2 |
| `c929c91` | the copula conjugates as `でした` | 5.3 |
| `37460aa` | the じゃ/や rows removed | 4.4 |

`README.md` and `src-tauri/Cargo.toml` carry the user's own uncommitted edits
and were not touched. The sweeps, the A/B tool and every build's dumps are in
the session scratchpad, not the repository.
