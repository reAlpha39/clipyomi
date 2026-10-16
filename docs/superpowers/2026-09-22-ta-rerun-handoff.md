# The re-run against the ta-old port, and the class no lexicon row reaches — handoff

**Date:** 2026-09-22
**Branch:** `feat/corpus-disagree`, tip `cfe1220`
**Predecessor:** [`2026-09-22-residue-adjudication-handoff.md`](2026-09-22-residue-adjudication-handoff.md)

The predecessor shipped nine user-lexicon rows and then measured nothing
against the ta-old port afterwards — every A/B it ran was engine against
engine. This session rebuilt the whole v1 side, re-ran the probe at the current
tip, and asked the corpus what the nine rows had actually done and what was
left.

Three things came out of it. The ranked head is **still** free of defects, for
a fourth session. Seventeen rows shipped, the largest batch so far, recovering
**591 occurrences against 8 lost**. And the largest single defect class in the
corpus — **1,207 occurrences of a lone katakana `イ`** — turns out not to be a
lexicon problem at all, and is left named, measured and minimal-paired rather
than half-fixed.

## 1. Four controls, all paid before anything was read

The predecessor's temp tree survived, which is the reason this session could
afford a full v1 rebuild. Everything below was checked against a number an
earlier document had already written down.

- **The v1 index.** Rebuilt from the same `JMdict_e.xml` in the detached
  worktree at `D:\Code\oss\clipyomi-v1` (`361a77f`, untracked
  `examples/v1_dump.rs`) with `build-index --engine v1`: **486,553 keys,
  619,717 records, 121,048 exact stems, 398 v5-fallback, 5 empty** — identical
  to the six-corpora handoff's rebuild, the third time that figure reproduces.
- **The pool.** The ten corpora re-dumped from `D:\Code\oss\vn-text` with the
  `artificialtrans` / `source` recipe: 8,054 / 5,253 / 6,517 / 10,499 / 9,559 /
  16,183 / 15,995 / 16,048 / 21,224 / 22,515 = **131,847**, every one matching
  its documented count. Fifth validation of that recipe.
- **The tip's span dumps.** `span_dump` rebuilt at this tip and run over
  `onikiss` is **byte-identical** to the predecessor's `spans-prot`, so its
  dumps are this tip's dumps and the hints-on side did not need re-running.
- **The ratio.** `corpus_disagree` at `--top 6000`, ten corpora, hints-on:

| | merge direction | split direction | ratio | cuts move |
| --- | --- | --- | --- | --- |
| predecessor | 18,164 | 33,458 | 1.84× | 8,745 |
| **this tip** | **18,167** | **33,386** | **1.84×** | **8,733** |

  Three regions moved one way and seventy-two the other. Twelve corpora and
  the sign has still never changed.

**Hints-off was not re-run, and that is a decision rather than an omission.**
Every commit since the last hints-off measurement is a user-lexicon row, and
the user lexicon feeds the Vibrato tokenizer, which only runs hints-on. The
hints-off column is byte-identical to the column the predecessor adjudicated,
including its refused 1,970.

## 2. The ranked head, fourth session running

Aggregated over the ten corpora, the head is proper nouns we merge and v1
splits (`早百合` 2,898, `達哉` 1,820, `香奈` 1,805, `アジュラ` 1,067), success
criterion 3 splitting what v1 merges (`そう|な` 620, `こと|で` 287,
`好き|な` 242), and the shipped auxiliary policy (`してる` 310, `なってる` 287,
`くれてる` 227). **No defect reaches the head in either direction.**

Two rows in the head are ours and wrong, and both belong to a class named
below rather than to the ranking: `ああっ` → `あ|あっ` 727 and `あんっ` →
`あん|っ` 562 are the stray-kana class, and `イッちゃう` → `イッ|ちゃう` 323
is §5.

## 3. The sweep the ranking cannot do

`corpus_disagree` ranks regions; it cannot answer "where does the ta-old port
emit a whole word and we do not". That question was asked directly, over every
JMdict entry tagged `uk`, with phrase entries (`exp`) and any tiling that
produces a lone particle or auxiliary kana excluded — those are the stacks
criterion 3 exists to split.

| | uk content spans TA emits whole | we keep | we shatter | distinct |
| --- | --- | --- | --- | --- |
| this tip | 42,819 | 41,455 | **1,364** | 167 |
| after §4 | 42,819 | 41,585 | **1,234** | 159 |

The `もしか` family is gone from the list, which is the predecessor's three
rows confirmed from the outside. What was left splits into three:

- **demonstrative or interrogative + particle** — `いつでも` 139,
  `いつもより` 95, `それだけ` 90, `これだけ` 81, `それこそ` 50, `どこまで` 33.
  Criterion 3 working. No action.
- **our merge is the larger one** — `やろう`, `なくなる`, `ほしい`,
  `してやる`, `くれる`. The auxiliary policy. No action.
- **single lexemes we shatter** — the rest, and §4's subject.

Three named-but-never-attempted items from earlier documents were re-measured
at this tip while the dumps were open, because the numbers in the register were
from older tips and smaller corpora:

| | occurrences | we emit whole | TA emits whole |
| --- | --- | --- | --- |
| `やだ` | 275 | **0** | 215 |
| `達した` | 111 | **0** | 90 |
| `イって` + `イった` | 174 | **0** | 155 |

## 4. Seventeen rows, 591 occurrences

Thirteen for the defect, four protective. `cargo test --workspace --features
mecab`: **673 passed, 0 failed, 1 ignored** (the pre-existing `jmdict_source`
doc-test); `cargo fmt --all --check` and `cargo clippy --workspace --features
mecab --all-targets` clean.

| | gold hints-on | gold hints-off |
| --- | --- | --- |
| tip | 1696 / 1950 / 0.9528 / 152 | 1588 / 1785 / 0.9504 / 118 |
| candidate | **identical** | **identical** |

Ten corpora, hints-on, against the tip: **545 of 131,847 sentences change
(0.41%)**, 591 occurrences recovered, 8 lost.

`やだ` 224, `イって` 107 (plus `イった` 17, `イったら` 4, `イっている` 3,
`イってる` 3, `イっていない` 2, `イってない` 1), `達した` 84, `マズい` 27,
`ありえない` 23, `わかりやすい` 21, `おかわり` 16, `シゴく` 15, `こやつ` 11,
`ちきしょう` 9, `ふともも` 8, `よだれ` 6.

`やだ` is the largest single row this project has measured — 275 occurrences
and not one of them emitted whole before it. `達した` is the first row with a
kanji surface, and it is here because `達し` (official notice) is a real
JMdict entry, so `達し|た` is a **false gloss** rather than an over-split.

**The four protective rows** were measured as their own A/B, 44 sentences
changing (0.03%):

- `いやだ` — `やだ` steals the `い` out of `もういやだ` → `も|うい|やだ`.
- `カワイイ`, `マズイ` — `イって` steals the katakana `イ` out of
  `カワイイって` and `マズイって`.
- `熟達した` — `達した` severs `熟達`. `上達` and `発達` come back for free
  once `達した` moves from `-3200` to `-1500`, which costs 17 of the bare
  `達した` merges and saves 8 severed compounds. That trade was taken because a
  severed compound is a **new** defect and a missed merge is an **old** one;
  the other way round is +101/−10 and was measured too.

**The 8 that stand**: `さや` (a character name) reads `さ|やだ` 5 times,
`上達した` still severs twice, and `ヤバイって` reads `ヤバ|イって` once. No
protective row: one is a name, and the other two are one occurrence each.

Cap 94 → 111, with the per-row counts in the CSV's own comment block.

## 5. The katakana `イ` class — 1,207 occurrences, and no row reaches it

This is the largest defect class the probe has ever found in this corpus and
the finding worth carrying forward.

Counting every place this engine leaves a lone `イ` or `イッ` as its own span,
ten corpora, hints-on: **1,207 occurrences.** The head is `イッ|ちゃう` 357,
`イ|って` 115, `イッ|た` 109, `イ|かせ` 57, `イッ|ちゃ` 38, `イ|く` 38,
`イッ|てる` 36, `イッ|たら` 29, `イッ|ちゃい` 27, `イッ|ちゃった` 27. One
lexeme — `イく`, the katakana spelling of 行く/逝く — across its paradigm.

§4's two rows take 136 of those. The rest do not respond to a lexicon row at
all: six were tried (`イッちゃう`, `イッた`, `イッて`, `イかせ`, `イきそう`,
`イく`) and **three lost outright**, `イッちゃう` coming back as `イ|ッ|ちゃう`,
which is worse than the `イッ|ちゃう` it replaced. A forced token with no cover
behind it is not an improvement.

The cause is one character, and the minimal pair is exact — **hints on and
hints off alike**:

```text
そこへ行った。     → 行った      ✓        先にイっちゃう。  → イっちゃう   ✓
そこへいった。     → いった      ✓        先にイッちゃう。  → イッ|ちゃう  ✗
そこへイった。     → イった      ✓
そこへイッた。     → イッ|た     ✗
```

The stem's script does not matter — `イった` parses whole. **A katakana small
`ッ` inside the inflected form does.** `ヤッた` is the control that shows it is
not a blanket katakana failure: it parses whole, and `lookup` finds `やった` as
a stored record, so it never goes through the same path.

This is an engine question — `engine/build.rs`'s stem generation and the FST
walk in `engine/lex.rs`, which folds with `kana::fold` per character — and not
a cost-model or lexicon one. It was **not** opened at the end of this session.
The katakana-`ッ` shapes alone are ≈668 occurrences, which makes it the single
largest thing on the open list with a one-character cause.

## 6. The stray-kana class, re-measured, and a correction to its framing

Ten corpora, hints-on: **45,137 stray small-kana segments**, 3,423.4 per 10k
sentences, in 16.3% of sentences. `っ` alone is **20,766 of them (46%)**.

The four-corpora handoff frames this class as a defect needing a gate. For its
largest member that framing is wrong: of the 20,766 lone `っ`, the **following
span is punctuation in 16,056 (77%)** — `、` 6,259, `」` 2,285, `！」` 1,669,
`！` 1,362, `……` 1,145 — and what precedes is `……` 3,961, `あぁ` 1,004,
`あん` 647. It is the clause-final emphatic sokuon of this register, and there
is no word for it to attach to. **Whatever gate is built for the stray-kana
class must subtract it first**, or it will measure the register rather than the
engine.

## 7. Not done

- **The katakana `イ` class, §5** — 1,207, the largest, and the one with a
  cause worth a phase.
- **The rest of §3's lexeme tail** — `よくぞ` 12, `どれどれ` 9, `だめだめ` 8,
  `おまた` 7, `おそば` 16. Each is gray for its own reason: adverb + particle,
  reduplication, or an honorific `お` that is defensible either way.
- **`ヤバイ`, `さや`, `上達した`** — §4's own 8, one occurrence per row.
- **Everything the predecessor left**: the 1,970 hints-off, the particle →
  godan family, the hints-off atomisation class, `達した`'s siblings in the
  `〜し|た` junction, the stray-kana gate (per §6), and the six darwin
  screenshot baselines.
- **`vn-text` is still exhausted as a source of new registers.** What this
  session found, it found by asking a different question of the same corpus —
  a per-occurrence sweep rather than a ranking — not by adding an eleventh
  script.
