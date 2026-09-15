# Engine v2 Phase 4 — handoff

**Date:** 2026-09-16
**Design:** [`specs/2026-09-15-engine-v2-phase4-design.md`](specs/2026-09-15-engine-v2-phase4-design.md),
with §9 written by this session.
**Branch:** `feat/engine-v2`
**Abandoned:** `spike/phase4-escape` (`cbd68f9`), the unconditional escape
that failed Gate 1. Kept for its measurement; nothing on it is merged.

## 1. What landed

Three commits stand between `dce356b`, where the design's §8 recorded the
failure, and this handoff: `14970cb`, `77cb1c1`, `bc3e607`.

`14970cb` adds `crates/jparser/examples/span_dump.rs`, and it comes first
because the rest of this phase is unreadable without it. `score`'s `strict`
counter credits a segment sitting exactly on a gold span **whether or not it
matched anything**, so a change that turns an unmatched-but-exact hit into a
matched span that shatters reads as a pure loss, and one that turns a fragment
into a real match reads as nothing at all. The probe dumps one segmentation
per input line as JSONL — segments, tokenizer boundaries, and the positions
the escape reopened — so two builds are compared span by span and token by
token. Every number below came out of it.

`77cb1c1` is the phase. `engine::relax::Relaxed` reopens the interior of a
hint token that no candidate can cover **and that carries at least one
hiragana character**; `Boundaries` gains a defaulted `is_usable_endpoint` that
`cost::is_allowed` asks instead of `is_boundary`; `engine::parse` wires the
two together in three lines. `kana` gains `is_hiragana`, the hiragana leg of
`is_kana` on its own. Nothing outside `crates/jparser/src/engine/` changes, no
index format moves, and no install rebuilds.

`bc3e607` is design §9 — the narrowing, the three §8.4 candidates measured
against it, and what it costs.

The design's §2 rule is implemented as written. The hiragana condition is the
one thing on top of it, and §3 below is why.

## 2. The gates

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
into scratch — 5,402 entries, 9,930 keys, 13,898 records, identical to Phase
3a's figures and every phase since:

```
entries:             5402
skipped entries:     0
keys:                9930
records:             13898
kanji:               0
skipped kanji:       0
```

Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.
Release build of `jparser-cli` with `--features mecab`. The eighteen-sentence
cell is the only one measured against the real 218,449-entry index
(`%APPDATA%\dev.jparser.clipyomi\dict\gen-3`), as §1.1 measured it.

### Gate 1 — the baseline cells, both hint states

Hints off:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7342  (1588 / 2163)
entry top-1:    0.9093  (1123 / 1235)
  by decision:  e 0.7342 (1588 / 2163)
span tolerant:  0.7924  (1714 / 2163)
  coarsening:   126 gold spans inside 62 dictionary merges
compounds:      0.5860  (109 / 186)
  kept whole:   44 / 106     split apart: 65 / 80
  by class:     lexical 46 / 112  false-match 24 / 32  transparent 39 / 42   (8 ambiguous, excluded)
```

Every cell **identical to the digit**, secondary lines included, as it must
be: `bounds == None` stays `None`, so no position is relaxed and there is
nothing for the escape to do. Checked harder than the cells, too — the 150
hints-off segmentations are byte-identical to the pre-phase build's,
segment for segment, via `span_dump`.

Hints on:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8285  (1792 / 2163)
entry top-1:    0.9125  (1230 / 1348)
  by decision:  e 0.8285 (1792 / 2163)
span tolerant:  0.8645  (1870 / 2163)
  coarsening:   78 gold spans inside 39 dictionary merges
compounds:      0.6129  (114 / 186)
  kept whole:   36 / 106     split apart: 78 / 80
  by class:     lexical 42 / 112  false-match 30 / 32  transparent 42 / 42   (8 ambiguous, excluded)
```

| cell | before | required | measured |
| --- | --- | --- | --- |
| strict, hints on | 0.8174 (1768) | must not fall | **0.8285 (1792)** |
| tolerant, hints on | 0.8525 (1844) | recorded | 0.8645 (1870) |
| entry top-1, hints on | 0.9092 | recorded | 0.9125 |
| compounds, hints on | 0.6129 (114 / 186) | recorded | 0.6129 (114 / 186) |
| `lexical` / `false-match` / `transparent` | 42/112, 30/32, 42/42 | recorded | identical |
| strict, hints off | 0.7342 (1588) | identical | 0.7342 (1588) |
| tolerant, hints off | 0.7924 (1714) | identical | 0.7924 (1714) |
| entry top-1, hints off | 0.9093 | identical | 0.9093 |
| compounds, hints off | 0.5860 (109 / 186) | identical | 0.5860 (109 / 186) |

Decomposed span by span against a before-binary, because the total alone would
not say which way this went:

```
BEFORE  strict 1768  =  1332 matched  +  436 unmatched-but-exact
AFTER   strict 1792  =  1361 matched  +  431 unmatched-but-exact
                         +29             −5
```

29 gold spans gained a strict hit and every one of them is a *matched* hit;
5 lost one, 3 of those having been unmatched-but-exact. Both halves move the
right way, which is what §8's failure did not do.

### Gate 2 — the compound metric

`0.6129 (114 / 186)` hints on, `kept whole 36 / 106`, `split apart 78 / 80`,
`lexical 42 / 112`, `false-match 30 / 32`, `transparent 42 / 42`. **Unchanged
to the digit**, every class line included, as it was under the unconditional
escape. The compounds this metric adjudicates live inside tokens the
tokenizer and the dictionary already agree about, so the escape never reaches
them.

### Gate 3 — the spurious-merge gate

`the_particle_absorbing_merges_are_gone`, **26 of 26**. Design §4.3 named
this as the gate that could genuinely fail — the escape only ever adds allowed
spans, and an absorbing merge inside a relaxed token would become legal where
it is structurally impossible today. It did not fail under the unconditional
escape and it does not fail under the narrowed one, which is the weaker of the
two facts.

### Gate 4 — the snapshot

`v2_snapshots_every_sentence` passes and
`git status --short crates/jparser/tests/snapshots` is empty. It runs hints
off, so this is the same fact as Gate 1's hints-off half, taken a second way.

### Gate 5 — the bench scaling cell

Owed since this phase opened and never run before today.

```
--- hints off ---           --- hints on ---
     chars   median ms           chars   median ms
      1005          57            1005          51
     10040         579           10040         548
scaling:  1.01x              scaling:  1.07x
```

Bands are hints off 0.98x–1.07x and hints on 1.07x–1.12x. Both readings are
inside on the first run. The hints-on cell landed on its lower edge, so a
second reading was taken and came back **1.02x** — below the band, which is
the safe direction: a lower ratio is less superlinearity, not more. The
guarded risk is a cell above 1.12x and neither reading is near it.

### Gate 6 — the eighteen reported sentences

Matched characters, hints on against hints off, against the real index:

| | characters | hints on | hints off |
| --- | --- | --- | --- |
| before Phase 4 | 630 | 426 | 487 |
| unconditional escape (`cbd68f9`) | 630 | 486 | 487 |
| **this phase** | 630 | **475** | 487 |

Sixteen of the eighteen are now character-for-character identical to their
hints-off parse. The remaining 12 characters are two sentences: `w1` loses 1,
and `w7` — 「お、『ロイヤルネイビーガール』の新刊出てたんだな」 — loses 11,
its katakana title coming back whole and unglossed rather than as
`ロイヤル` + `ネイビー` + `ガール`. That one sentence is the entire difference
between this phase and the version that failed Gate 1, and §4 below says why
it is the right side of the trade.

### Gate 7 — the suites

`cargo test -p jparser --features mecab`: **388 passed across thirteen
binaries, 0 failed, 0 ignored.** `engine_v2` is 47, up from 46 before the
phase — the four rewritten characterization tests keep their slots and
`a_hint_token_with_no_hiragana_keeps_its_veto` is the new one. `cargo fmt
--all` and `cargo clippy --all-targets --features mecab` are clean.

## 3. Where the implementation departs from the design

One place, and it is the whole phase.

Design §2 relaxes the interior of **any** unmatchable token. §8 measured that
and it failed Gate 1: hints-on strict fell 1768 → 1713, because the rule fires
on proper nouns and numbers IPADIC holds whole and JMdict does not hold at
all, whose interiors are full of one-character entries. `品川` became
`品` + `川` and a reader hovering it got "goods" and "river".

`relax::opens_to_a_span` adds the missing condition: **one hiragana character
anywhere in the token.** Japanese writes its inflection and its particles in
hiragana, so hiragana inside a token is the only evidence the engine has that
the token is a *concatenation* — a content word glued to something else —
rather than one word. A token with no hiragana in it is a run of kanji,
katakana or digits that IPADIC declined to cut, and that is a single lexical
unit by construction.

Per token over the 150 gold sentences, hints on, that separation is not
marginal — the two halves have opposite signs:

| token script | tokens | gold spans gained | gold spans lost |
| --- | --- | --- | --- |
| **hiragana-bearing** | **47** | **29** | **3** |
| kanji only | 68 | 5 | 42 |
| katakana only | 36 | 3 | 25 |
| digits only | 25 | 0 | 21 |
| Latin, punctuation | 13 | 0 | 0 |

Read the losing side by name and it is a gazetteer: `品川`, `岡本`, `松久`,
`浦和`, `新大阪`, `大阪環状線`, `百合子`, `仁兵衛`, `ディオニス`, `マルヴィナ`,
`ルクセイン`, `サフィレーヌ`, `2014`, `1969`, `9518`.

Design §9.3 records the other three §8.4 candidates, all implemented behind a
switch and measured on the same gates. The two findings worth carrying
forward:

- **A minimum length on doubly-relaxed spans is inert here.** At 3 it
  reproduces the unconditional escape's gold cells *exactly*. Blocking a
  relaxed span does not restore the gold span: `UNMATCHED` is 30 a character
  against `COVER`'s 1, so the DP takes whatever shorter match remains and pads
  the rest with a fallback run — `羽田空港` becomes `羽田`(unmatched) + `空港`
  rather than going back to one unmatched span. Any partial decomposition
  destroys the exact hit as thoroughly as a full one, which is worth
  remembering the next time a length rule looks like it would help.
- **Script-run awareness (§8.4.2) fails Gate 1 too**, at 1753. It reaches the
  katakana and digit tokens by construction and leaves all 68 all-kanji ones
  open, and 42 of the 88 losses are there.

§8.4.3's part-of-speech condition was not implemented, and is recorded as
not-needed rather than untried: it was proposed to separate a grammatical
tail from a lexical unit, and the script test does that with information the
engine already has, without a second inventory or a conjugation special case.

## 4. What this gives up, and why it is the right side

Stated here rather than buried, because all three are real.

- **Four multi-word compounds stay glued.** `東京厚生年金会館` (3 gold spans),
  `クライシスマッピングパーティー` (3), `田町駅` (1), `冶爺` (1) — the whole +8
  on the no-hiragana side of the table. They are genuinely several words each,
  and the escape cannot tell one from `羽田空港` without information it does
  not have. Eight spans against eighty-eight is not a close call.
- **`w7`'s katakana title.** On a visual novel's register, shattering
  `ロイヤルネイビーガール` into three katakana words would have helped a
  reader. On the gold corpus the identical shape is `ニュルンベルク` → six
  pieces. The two registers disagree about the sign, the gold corpus is the
  gate, and Gate 6 is recorded rather than thresholded for exactly this
  reason.
- **Three hiragana runs that are themselves one unit still shatter** —
  `ぱたぱたぱたぱた`, `ぇぇぇ`, `ぁぁぁ`. They are indistinguishable from
  `ぱひゅぱひゅぱひゅと`, which is the same shape with a particle on the end
  and accounts for 26 of the 29 gains. A ceiling of the condition, not an
  oversight, and not worth a second rule for −3.

## 5. Two things the design got wrong, both usefully

**§4.2 listed `a_reading_the_index_lacks_is_unmatched_between_correct_boundaries`
as "expected not to move."** It moved and improved, and it still does under
the narrowing: `ていう` is a token nothing covers and it is all hiragana, so
its interior reopens and `いう` matches — the right reading, since the phrase
is と + いう — leaving only the `て` that lost its small つ. "The dictionary is
missing this" does not imply "nothing in the engine can help". The test is
renamed `a_reading_the_index_lacks_still_yields_the_word_inside_it`.

**§7's first open item anticipated the Gate 1 failure's mechanism and
misjudged it.** It said a token nothing covers would be relaxed harmlessly,
"no candidate exists there either way". Candidates do exist there, and they
are the worst kind: one-character entries under a proper noun. The open item
named the right thing to narrow and drew the opposite conclusion about its
urgency.

## 6. Open items

- **The eighteen sentences are still not a corpus.** They are what was
  reported on one day from one register, chosen by what looked wrong. Gate 6
  records a figure over them; it does not license quoting 475/630 as the
  engine's accuracy on anything. This was true when §7 said it and the
  narrowing does not change it.
- **The gold corpus has no proper-noun decision to appeal to.** The 88 losses
  and the 8 gains on the no-hiragana side are both real, and the only thing
  separating them is whether JMdict happens to hold the unit. If compound
  preservation ever gets a second look, `東京厚生年金会館` is where the
  evidence for it is.
- **`opens_to_a_span` is a script test, not a linguistic one.** A Latin or
  punctuation token never opens, which is right today only because no
  candidate lives there either way — §7's first open item, narrowed but not
  closed. If the index ever grows Latin keys, that arm needs measuring rather
  than assuming.
- **The darwin e2e baselines are still owed from Phases 3a and 3b.** Untouched
  by this phase and still outstanding.
- **CI has still never run.** There is no git remote, so a green local run
  remains the only real gate, and the four jobs that want the IPADIC
  dictionary are still dead on arrival.
