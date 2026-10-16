# The katakana sokuon, and the fold that was refused with it — handoff

**Date:** 2026-09-22
**Branch:** `feat/corpus-disagree`, tip `3fe232b`
**Predecessor:** [`2026-09-22-ta-rerun-handoff.md`](2026-09-22-ta-rerun-handoff.md)

The predecessor named the largest defect class the probe has found — 1,207
occurrences of a lone katakana `イ` — located its cause at one character, and
stopped there: "It was **not** opened at the end of this session." This
session opened it.

Three things came out of it. The cause was in a different file than the
predecessor guessed, and the fix is two lines. The class is **1,067
occurrences at this tip and 420 after**, with no new false gloss anywhere in
the corpus. And the wider change that would have closed more of it — folding
the whole conjugation trie, the trade Phase 1a wrote down and could not price
— was built, measured against the same ten corpora, and **refused**, which is
the part worth carrying forward.

## 1. Controls

- **The v1 index.** Rebuilt in the detached worktree at
  `D:\Code\oss\clipyomi-v1` (`361a77f`, untracked `examples/v1_dump.rs`) from
  the same `JMdict_e.xml`: **486,553 keys, 619,717 records, 121,048 exact
  stems, 398 v5-fallback, 5 empty** — the fourth time that figure reproduces.
- **The pool.** The ten corpora re-dumped from `D:\Code\oss\vn-text`, every
  per-corpus count matching its documented figure, **131,847** in total.
- **Gold at the tip**, both hint states, all four cells:
  1696 / 1950 / 0.9528 / 152 hints-on and 1588 / 1785 / 0.9504 / 118
  hints-off. Identical after the change, which §3 returns to.
- **`cargo test --workspace --features mecab`**: 674 passed, 0 failed, 1
  ignored (the pre-existing `jmdict_source` doc-test) — 673 before, plus the
  new `kana` test. `cargo fmt --all --check` and `cargo clippy --workspace
  --features mecab --all-targets` clean.

## 2. The cause was the conjugation trie, not the FST walk

The predecessor wrote that this is "an engine question — `engine/build.rs`'s
stem generation and the FST walk in `engine/lex.rs`". Both of those were
already right: `lex::candidates` pushes every character through `kana::fold`
before asking the key FST, which is why a katakana stem reaches a hiragana
headword at all.

The holdout is `engine/table.rs`. `Table::search` walked the reverse suffix
trie with **raw** characters, and `insert` built that trie out of IPADIC's
suffixes, which are hiragana without exception. So a katakana small `ッ` found
no child one character into the tail, the walk broke, and no chain — and
therefore no candidate — existed for the whole word. The minimal pair is
exact, and the stem's own script is not what matters:

```text
そこへ行った。   → 行った   ✓        そこへ行ッた。   → 行|ッた    ✗
そこへいった。   → いった   ✓        そこへいッた。   → い|ッた    ✗
そこへイった。   → イった   ✓        そこへイッた。   → イッ|た    ✗
                                    ワラッた        → ワラッ|た  ✗
```

`ヤッた` is the control the predecessor found and it still holds: `やった` is
a stored record, so it is reached by lookup and never walks the trie.

In the ten-corpus pool the shape `[katakana]*ッ[hiragana]+` occurs **1,860
times over 674 distinct surfaces**, headed by `イッちゃう` 240, `イッて` 166,
`ギュッと` 50, `イッてぇ` 45.

## 3. The fix: the sokuon only

`3fe232b`. `Table::search` now passes each input character through
`hiragana_sokuon` — katakana `ッ` read as hiragana `っ`, everything else
unchanged — before asking the trie for a child.

Only the sokuon, and the reason is not caution. **The gemination mark takes
the script of the character it geminates rather than of the suffix it belongs
to**, which is exactly why it is the one character that leaks across the
boundary: `イッちゃう` writes the sokuon in katakana and leaves `ちゃう` in
hiragana. Nothing else in an inflected tail does that. §4 is what happens when
you assume otherwise.

Ten corpora, hints-on, against the tip: **1,005 of 131,847 sentences change
(0.76%)**.

| | count | distinct |
| --- | --- | --- |
| spans gained holding a katakana `ッ` and a hiragana tail | **686** | 37 |
| other spans gained | 476 | 14 |
| matched spans lost outright | 723 | 45 |

The head of the first is `イッちゃう` 359, `イッた` 114, `イッてる` 37,
`イッたら` 29, `イッちゃい` 26, `イッちゃった` 26, `イッちゃえ` 23,
`イッちゃいます` 18. Of the second, **447 are `イッて`** — a span that was
already the right shape and merely unmatched, which now carries its entry. The
losses are the debris those merges absorbed (`ちゃう` 361, `た` 112, `てる`
36); what is left after subtracting them is about 18 spans of neighbours
re-tiling around a new merge (`から` 8, `か` 5, `オッパイ` 3).

**Gold is identical in both hint states, all four cells.** So is `span_dump`
over `onikiss` against the same build without the reading half of the change,
byte for byte.

Against the ta-old port, per occurrence over the pool:

| surface | occurrences | TA emits whole | tip | this change |
| --- | --- | --- | --- | --- |
| `イッちゃう` | 381 | 377 | **0** | **365** |
| `イッた` | 153 | 114 | **0** | **115** |
| `イッてる` | 37 | 0 | 0 | 37 |

The lone katakana-`イ` class the predecessor measured goes from **1,067
occurrences at this tip to 420**. What survives is `イ|かせ` 57, `イ|く` 38,
`イッ|ちゃ` 36, `イ|っ` 31, `イ|きそう` 23 — the `イか`/`イき`/`イけ` half,
which has no sokuon in it and is the `ミスった` ranking family rather than
this. `イッ|ちゃ` is a third thing again: `いっちゃダメ` splits the same way in
hiragana, so the `〜ては` contraction is missing from the table in both
scripts.

## 4. The full fold, built and refused

Phase 1a's `katakana_conjugation_is_not_deinflected_yet` wrote the trade down:
folding the trie would reach katakana inflections and "would also turn every
katakana loanword ending in タ (データ, メーター, カルタ) into a spurious
past-tense candidate", and Tier D's 8 sentences could not price that. It is
priced now. The full `kana::fold` version is on
**`spike/katakana-sokuon-deinflect`, `eac76b0`**.

Ten corpora, hints-on, against the tip: **2,030 sentences change (1.54%)**,
708 target spans gained, 1,593 other, 1,897 matched spans lost. Gold is
identical in both hint states here too — the gold corpus has almost none of
this register's katakana, so **gold cannot see this trade at all**, in either
direction.

Diffed directly against the sokuon-only fold that shipped, the disputed set is
**1,076 sentences and 1,149 spans**, and it comes out about one good to one
bad.

What it gets right, and the shipped fold does not:

`イケナイ` 130, `キツク` 107, `イカせて` 102, `イキたい` 55, `シて` 29,
`イカせちゃう` 18, `イカせる` 15, `イキます` 14, `イキたく` 11, `キてる` 10,
`イカない` 9. All real words, all correctly glossed. `キツク` is the
four-corpora handoff's own open item — the `INEXACT` fold penalty, 115
occurrences — closed as a side effect, and `イカせて` is a merge the ta-old
port makes 115 times out of 116.

What it gets wrong:

`タツヤ` glossed 立つ **104**, `ズン` as ずる 48, `トコ` as とく 31, `コレ` as
来る 28, `はロ` as はる 18, `ユリナスです` severed into `ユリ` + `ナスです`
14, `ネロ` as 寝る 11, `クリトリ` as くりとる 9, then a long tail of one and
two — `エッダ` as えう, `クレ` as 来る, `サ` as する, `アンタ` as あむ,
`スミマセンでした` as すむ. Plus a class of over-merges across a `〜て`
junction: `されてイク` as される 6, `飲まれてイク` as 飲む 6, `吸われてイク`
as 吸う 5.

**Most of those spans were already whole and merely unmatched.** `タツヤ` is
104 spans at this tip too; what the fold changes is that a character's name
stops being unglossed and starts being confidently wrong. For a reading aid
that is a worse state, and it is the same judgment the predecessor made when
it moved `達した` from `-3200` to `-1500`: a new defect outweighs an old
missing merge.

Refused. The numbers are in `hiragana_sokuon`'s doc comment so the next person
to reach for the fold finds them before building it, and Phase 1a's test keeps
its name and its assertion with the price written into it.

## 5. The defect the fix exposed

`assemble::reading_for` builds a conjugated reading by pasting the inflected
tail on **as the input wrote it** — only the stem is canonical, taken from the
entry. That was invisible while no katakana tail could ever reach a chain. The
moment one could, `行ッた` read `いッた` and `笑ッてる` read `わらッてる`, and
`src/render/furigana.ts` renders a reading verbatim in its hiragana mode, so
that is what a reader would have seen.

`kana::to_hiragana` — `fold`'s target script inverted — now folds the tail
back. It is identity on hiragana, on `ー`, and on everything that is not kana,
so no reading that was already right can move; the `カ変・来ル` path takes the
same treatment and `出て来た` still reads `でてきた`. No scoring path reads an
assembled reading (`score`, `bench` and `diff` compare boundaries and entry
ids), so this half cannot move a gold cell, and the `span_dump` control above
confirms it moves no span either.

There was no test anywhere for a reading whose tail is not hiragana. There is
one now.

## 6. Not done

- **The `イか`/`イき`/`イけ` half, 420 occurrences.** `イく` and `イかせ` lose
  to their parts on the `INEXACT` penalty; this is the `ミスった` family, a
  cost-model item, and §4 shows the shape of what fixing it by folding costs.
- **The `〜ては` contraction.** `イッ|ちゃ` 36, and `いっちゃダメ` splits the
  same way, so it is a table gap and not a script one.
- **`キツク` and the `INEXACT` penalty, 115 occurrences.** Still open. §4
  closes it only as part of a trade that was refused; it wants its own change.
- **Everything the predecessor left**: the 1,970 hints-off, the particle →
  godan family, the hints-off atomisation class, `達した`'s siblings in the
  `〜し|た` junction, the stray-kana gate (and the predecessor's §6: subtract
  the clause-final emphatic sokuon first), and the six darwin screenshot
  baselines.
- **`vn-text` is still exhausted as a source of new registers.** This session,
  like the last, found what it found by asking a different question of the
  same corpus — a minimal pair against the engine's own source, rather than a
  ranking.
