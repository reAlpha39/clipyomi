# Finding defects by counting — handoff

**Date:** 2026-09-18
**Branch:** `feat/corpus-disagree`, off `feat/sokuon-tail` tip `e3d75e2`
**Predecessor:** [`2026-09-16-sokuon-tail-handoff.md`](2026-09-16-sokuon-tail-handoff.md)

Every defect the previous session fixed came from one screenshot pair at a
time — ClipYomi against Translation Aggregator, whatever the reviewer
happened to paste. Five screenshots, five defects, and no way to know what
the sixth would have found. This session replaced that with a corpus.

## 1. What shipped

| commit | what it is |
| --- | --- |
| `1bb166f` | `examples/corpus_disagree.rs` — ranks two engines' disagreements over a corpus by how many of the engine's own signals agree |
| `a3279ca` | `cost.rs`'s `CONTENT_MATCH` — a `MATCH` surcharge withheld from particles, which is the probe's whole top class |

## 2. The probe

`corpus_disagree.rs` parses a corpus with today's engine, reads a
segmentation dump of the same corpus from the ta-old port, and groups every
place the two cut differently into a **region**: a maximal interval whose
endpoints both engines agree are boundaries and whose interior cut sets
differ. Regions are keyed by their surface, counted by distinct sentence, and
ranked.

**v1 is triage, not an oracle**, and the ranking is built so that it cannot
be mistaken for one. Its own scores are criterion 2's floors (strict 0.6810 /
0.6963, entry top-1 0.9159 / 0.9155) and every one is below v2's, so a
disagreement is a place worth adjudicating, never evidence that v1 is right.
What ranks a region is how many **independent signals** fire on it:

| signal | what it means |
| --- | --- |
| `alt` | reading 1 differs from reading 2..3 here — the right cover ranked second, which is how `物思いにふける` and `なのに` were found |
| `rdg` | one entry covers the region and its reading is not the concatenation of the parts' readings — `唐揚げ` is `からあげ`, never `とう` + `あげ` |
| `sht` | one entry covers the region and the engine splits it anyway — `shatter_sweep.rs`'s question, asked per occurrence so frequency can rank it |
| `prt` | a lone `は`/`へ` inside the region carries `PARTICLE`, so `src/render/furigana.ts` romanises it `wa`/`e` — `へへ` read as `e e` |

Two filters do most of the work of keeping the head readable. A region whose
covering entry is a `lex::is_grammatical_stack` is reported **apart from the
ranking**: JMdict carries `には`, `とは`, `それは`, `ではない` as headwords and
success criterion 3 plus the 26-row `spurious_merges.txt` gate exist to refuse
exactly those merges, so they are the standard rather than defects. And a
cover must be `PRIMARY`, exact and uninflected: a `PRONOUNCE`-only record is a
*reading* of some kanji entry, so without that clause `はは` is covered by 義母
(`ぎぼ`), `みた` by 蒙 (`むた`), and `はもう` by 食む through a deinflection
chain. Those two filters took the four-signal count from 13 rows of noise to
one real row.

**The corpus is not in the repository and nothing derived from it is.** It is
8,334 lines of visual-novel dialogue from a local SQLite capture — copyrighted
game script, used as probe input only, the same judgment the frequency-layer
refusal already made for this project. The probe takes a path.

**The v1 side is produced out-of-tree.** `crates/jparser` must contain nothing
derived from ta-old (criterion 1), so the old engine is built in a detached
worktree at `fb450f6^` and dumps `text<TAB>start:len:matched,...`. Nothing in
this repository links it, and the probe reads a file.

## 3. What it found, and what was fixed

Hints on, 8,334 sentences, 5,153 distinct regions, 6,601 sentences where the
two engines disagree at all. 77 regions at three signals, 238 at two — and
the head was **one class**, adjudicated uniform `lexical`:

`呼び|方`, `部屋|着`, `脳|内`, `危険|日`, `キャンプ|場`, `同盟|者`, `持久|力`,
`現代|人`, `不審|者`, `効果|音`, `シャッター|音`, `チェーン|店`, `専門|外`,
`最終|日`, `攻略|本`, `創造|主`, `男性|器`, `未|開封`, `小|悪魔`, `汚|部屋`.

The reading is what settles it. In 52 of the top 60 the entry's reading is
what the sentence says and the split's is not: 者 comes out `もの` where the
word needs `しゃ`, 日 `ひ` where it needs `び`, 音 `おと` where it needs `おん`,
場 `ば` where it needs `じょう`, 内 `うち` where it needs `ない`. The engine
was picking the free-standing kun reading of a morpheme that is bound in the
compound — a wrong gloss and wrong furigana, not merely a different cut.

**The same class is in the gold corpus's own `lexical` misses**, which is the
confirmation that mattered: `発売所`, `副大統領`, `月平均`, `総売り上げ`,
`一歩先`, `年ぶり`, `その頃`.

### 3.1 The arithmetic, and why no `MATCH` value reaches it

`LEN_GAIN` made the merge-versus-tiling comparison length-sensitive, and the
previous handoff's §3e is right that this is the only shape that can separate
a seven-character idiom from `には`. But it is length-sensitive only where the
tiling is **balanced**. Peel a single character off either end and the squares
cancel exactly.

For a non-COMMON headword of length `n` split into a COMMON piece of `n-1` and
a COMMON piece of `1`, hints on:

```
whole  = n + MATCH - (n-1)²
split  = [(n-1) + MATCH - (n-2)² - 2(n-1)] + [1 + MATCH - 0 - 2]
gap    = whole - split = 3 - MATCH
```

One point at `n = 2` and one point at `n = 5`. That is the entire class.

`MATCH = 3` closes it and was measured, hints on: `lexical` 72 → 83,
compounds 142 → 146, strict 1754 → 1729, tolerant 1919 → 1932, entry top-1
0.9374 → 0.9393. It also merges five windows that should split —
`transparent` 40 → 35 (`彼の` ×3, `側から`, `今も`) and `false-match` 30 → 28
(`名の`, `煙が`) — because a per-span constant cannot see that the character
being peeled off `彼の` is a **particle** and the one being peeled off `呼び方`
is a bound morpheme. That is `LEN_GAIN`'s own argument one level down: `MATCH`
prices spans, not what is in them.

### 3.2 `CONTENT_MATCH`

So the surcharge is withheld from `PARTICLE`. A particle standing as its own
span is not a failure to find a longer word — it is the analysis criterion 3
exists to protect. `CONTENT_MATCH = 1`, charged to every span that does not
carry the flag.

| | strict | tolerant | entry top-1 | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- | --- |
| on, before | 1754 | 1919 | 0.9374 | 142 | 72 | **30 / 32** | **40 / 42** |
| on, after | 1745 | **1932** | **0.9400** | **152** | **82** | **30 / 32** | **40 / 42** |
| off, before | 1611 | 1749 | 0.9347 | 113 | 51 | 23 / 32 | 39 / 42 |
| off, after | **1633** | **1775** | **0.9358** | 113 | 52 | **22 / 32** | 39 / 42 |

Hints on, ten `lexical` windows gained and **neither split class moved**. The
strict fall is coarsening, not damage — 165 gold spans in 74 merges becomes
187 in 85 — which is §5.2's own 16.5-point measurement: the corpus is
adjudicated at short-unit granularity, so every compound now kept whole costs
its own gold spans. `tolerant` and `entry top-1` both rise.

**Hints off moves for once, and upward**, which matters more than the
hints-on half: that state is the one a fresh install runs, `mecab_hints` being
`#[serde(default)]` on a `bool`. §3.3 says what actually moves there, because
it is **not** the class above. This term is ungated where `LEN_GAIN` is gated
because its magnitude is bounded at one point per span-count difference, so
its worst case is one boundary; `LEN_GAIN`'s reward grows as `n²` and ungated
it buys arbitrarily long spurious spans.

The one cost is one hints-off `false-match` window: `はいた` in
`d:a:003521-6`, §9.5's shape in the state where `PRONOUNCE_CROSS` is
bounds-gated and cannot reach it.

Gates: spurious merges **26 of 26**, 657 tests 0 failed 1 ignored, `fmt` and
`clippy --all-targets --features mecab` clean, `bench` interleaved three
rounds each way (hints on 536 / 560 / 531 ms before against 524 / 529 / 531
after; hints off 579 / 609 / 587 against 589 / 596 / 596 — spreads overlap),
scaling 1.00x on and 0.98x off.

The snapshot moved once and it is a correction: `日本語を話せますか。` read
`話` + `せま`("narrow") + `すか`("disappointment") and now reads `話せ` +
`ます` + `か`.

**The term's own claim now has a test.**
`a_one_character_peel_ties_unless_the_peeled_piece_is_a_particle` asserts the
arithmetic directly over `n = 2..=5`: a non-COMMON span under bounds ties its
COMMON `(n-1)` + COMMON `1` tiling, and loses by exactly one when the peeled
piece is a `PARTICLE`. It is the shape
`a_function_stack_pays_only_while_it_is_unconjugated` already uses for
`FUNCTION_STACK`'s gate — an arithmetic contract rather than a corpus number —
and without it the length-invariance claim was unguarded: nothing else would
fail if the term became per-character.

**Two `cost.rs` invariants were restated rather than deleted.** Both pinned
the per-span difference as exactly `MATCH`; the per-span term is now
`MATCH + CONTENT_MATCH` for a content word and `MATCH` alone for a particle,
and `the_match_cost_is_flat_and_the_longer_span_wins_on_it` now pins both
halves, the particle one with the two cases the term exists to separate.

**After the term, the probe's head is gone**: three-signal regions 77 → 7,
two-signal 238 → 36. That is a split-detector reporting on a merge-increasing
change and is not by itself evidence that nothing else opened, so the merge
direction was counted separately: regions where v2 has fewer segments than v1
go 525 to 529, and 1,228 sentence-occurrences to 1,232.

### 3.3 The term has a second mode, and it is where hints off moved

"Withheld from particles" is the narrow phrasing and it hides the general
rule: **the term favours the merge by (content pieces in the tiling − 1)**,
and favours the split only where a content merge tiles entirely into
particles. A two-piece tiling with one particle (`彼` + `の`) is exactly
neutral, which is the designed case; a three-piece tiling with two content
pieces hands the merge a point.

That matters twice.

**It is why hints off moved, and the class in §3.1 is not the reason.**
`LEN_GAIN` is gated on `bounds`, so hints off there are no squares to cancel
and the one-character-peel gap is `2n − MATCH − CONTENT_MATCH`, which *grows*
with `n` — 1 at `n = 2`, 3 at `n = 3`, 7 at `n = 5`. The class is unreachable
there at any length. What is reachable is the three-piece all-content shatter,
verified: `罪|悪|感` and `解|像|度` become whole hints off, `林|間|学校` does
not. §5.2 said the wrong thing about this for one draft and now says this.

**It spends margin on criterion 3's weakest row.** `ことになっている` is
`exp,v1`, so it is content, and it tiles `こと` + `に` + `なっている` — two
content pieces, so the merge gains a point. §3c of the previous handoff
records that exact row as the one that took the spurious-merge gate 26 → 25
under `M=6 FS=8 PC=12`. The gate is 26 of 26 here and `grammatical_stack`
still withholds `LEN_GAIN` from that merge, but **how much margin is left is
unmeasured** and the next change in this area should measure it first.

### 3.4 Two user-lexicon rows became dead and were deleted

`hint_user_lexicon.csv` went 87 → 85. `あたし達` and `唐揚げ` are the
one-character-peel shape exactly, and both now merge unaided — measured by
deleting the rows and re-running, whole in both cases with every gold cell
identical to the digit in both hint states.

`なのに` and `へへ` stay, and the reason is the general rule above: both tile
into COMMON **particles**, which is precisely the tiling the surcharge is
withheld from. The CSV's "this list is not converging" block now reads
"it converged by half" and names what is left.

That comment used to end "the number will keep climbing until something
structural closes it". Something structural closed half of it. A dead row is
not free — the cap exists because Vibrato's Viterbi is global and a bigger
user lexicon moves boundaries where none of its entries appear.

## 4. What the audit of the probe found, and what was done about it

The probe was itself audited against the corpus, and three of its four signals
and its ordering were all weaker than they looked. **Every finding below is
now applied**, in `1bb166f`'s follow-up.

- **`prt` is dead.** It fires **0 times** hints-on across 4,838 ranked rows,
  and 8 times hints-off, every one of which already carries `sht`. It also
  cannot see the direction that matters: it requires `は` to be a *separate*
  segment carrying the flag, so when the engine swallows `は` into a bogus
  word — `今の俺はいない` → `今|の|俺|はい|ない`, `はい` "yes" — it is silent.
  It belongs as an annotation on a `sht` row, not as a ranking term.
- **`rdg` is a strict subset of `sht`** — 82 of 82 rows also fire `sht`, none
  without. It adds no row; it double-weights 82 of 322, and that is how
  `誰が`(たが) reached rank 1 on a surface whose 28 corpus sentences are all
  `だれが`.
- **`sht` needs a particle-tail exclusion.** 33 rows / 287 occurrences have a
  bare particle as the last v2 segment, and only two are worth fixing. The
  class holds every archaic-reading trap — `誰が`(たが), `天の`(あめの),
  `外に`(ほかに), `本の`(ほんの), `並に`(ならびに) — where the split is
  correct, plus `みたいな` at n = 128. `is_grammatical_stack` does not catch
  them because it requires *every* part to be COMMON + PARTICLE; the probe's
  test should be that the **last part alone** is. Commonness is not the
  discriminator and reaching for it is backwards: `外に`, `本の`, `並に` are
  `[primary,common-line]` while `呼び方`, `部屋着`, `危険日`, `キャンプ場` —
  the real defects — are bare `[primary]`.
- **Ranking by signal count is the biggest single error.** `alt` fires on 53%
  of rows and `sht` on 6.7%, so "how many signals" ranks almost identically to
  "does an entry happen to exist", with frequency only a tiebreak inside it.
  Sorting by `n` first among rows with at least one signal costs nothing and
  reorders the head completely.
- **Surface keying hides homographs.** One adjudication is recorded per
  surface where the right answer is per sentence: `端から` is two occurrences
  of `はなから` (merge right) and four of `初っ端から` / `先端から` (merge
  wrong) in one row. The signal booleans are OR'd across occurrences and the
  printed covers come from the first occurrence only, so a row's cover is a
  sample of size one against a count of `n`.
- **The punctuation guard had a hole.** `any(is_kana || is_kanji)` passed 146
  zero-signal rows of small kana plus punctuation (`ぅ……`, `ゅ、`, `っ、`)
  because `ぅ` is kana. The guard now requires a kanji or a non-small kana.
- **Three of the four signals only detect over-splitting**, which the module
  doc now says outright. `sht` requires that we cut a covered region, `rdg`
  compares the parts' readings so a merge makes it trivially equal, and `prt`
  needs the `は` to still be its own segment — so when the engine swallows one
  into a bogus word, every one of them is silent. Only `alt` is symmetric.
- **`rdg` depended on which reading came first.** A multi-reading entry's
  `readings[0]` is document order, not a judgment, so the signal could fire
  for a reason unrelated to the region. It now compares against *any* reading
  of the covering entry.
- **`cuts_in` compared ordered `Vec`s built from a foreign file.** Nothing in
  the dump's format contract says the v1 spans arrive sorted; they are sorted
  now. The sentence-level `b1`/`b2` sets are built from both span *starts* and
  ends, and a sentence whose v1 spans do not tile `0..n` is skipped and
  counted — every downstream filter assumes no span straddles a region edge.
  Measured on this dump: 0 such sentences, so the guard is insurance.

The corrected ranking is what produced §5.

## 5. What the corrected ranking puts at the top, all live

Frequency-first, hints on, after `CONTENT_MATCH`. The head is no longer a
compound-noun class at all:

| n | region | v1 | v2 (ours) |
| --- | --- | --- | --- |
| 218 | `いた` | `いた` | `い`\|`た` |
| 138 | `じゃない` | `じゃない` | `じゃ`\|`ない` |
| 115 | `ような` | `ような` | `よう`\|`な` |
| 91 | `んだけど` | `んだ`\|`けど` | `ん`\|`だけど` |
| 71 | `かもしれない` | `かもしれない` | `かも`\|`しれない` |
| 70 | `センパイ` | `センパイ` | `セン`\|`パイ` |
| 62 | `くせに` | `くせに` | `くせ`\|`に` |
| 56 | `ああっ` | `ああっ` | `あ`\|`あっ` |
| 54 | `いない` | `いない` | `い`\|`ない` |
| 44 | `からね` | `から`\|`ね` | `からね` |

Three of them are worth naming in full.

Every one reproduces on the shipped engine, hints on, and all three are
**hints-on only** — the hints-off parse of each is correct. Frequencies are
distinct corpus sentences out of 8,334.

**1. The te-form euphonic stem shatters, 122 region surfaces / 300
occurrences.** `思ってる` → `思` + `っ` + `てる`, where `てる` glosses 照る
"to shine"; `言ってる` → `言`("word") + `っ` + `てる`; `遊んでる` → `遊` + `ん`
("yes; yeah") + `でる`(出る). Shape: a lone `っ`/`ん` segment followed by a
segment opening `て`/`で`. It is a hint artefact — IPADIC cuts `言っ`+`てる`,
JMdict wants `言って`+`る`, `is_allowed` forbids the JMdict span, and the
engine falls back to a bare kanji. Hints off: `言って`|`る`, 3 rows and 9
occurrences. **No signal can see it**: no single entry covers `思ってる`, so
`sht` and `rdg` are structurally impossible, and readings 2–3 do not differ
there, so `alt` is quiet too. 75 of the 122 fire nothing at all.

**2. `いた`, 218 sentences — the largest single count in the corpus.**
`待っていた` → `待って` + `い` + `た`, where `い` glosses "the Boar (twelfth
sign of the Chinese zodiac)" and `た` "want to". Hints off it is
`待って`|`いた`. Signals: `alt=v1` only, which put it at rank 316 under the old ordering and
rank 1 under the corrected one.

**3. `からね` 44 and `からな` 39.** `そうだからね` → `そうだ` + `からね`, where
`からね` is `かる` Negative Conditional, "to spur on; to urge forward". `から`
is `[primary,common,common-line,particle]` at two characters and lost to it.
Same shape: `ないだろ` 15 (`なく` Past, "to weep"), `もんな` 5 (`もむ` Past,
"to rub"), `てるでしょ` 4, `てんじゃん` 2 (`テンジャン`, Korean miso).

These are the particle-absorbing class criterion 3 exists to stop, occurring
on a real corpus at frequencies the 26-row fixture cannot represent. Note the
direction: `CONTENT_MATCH` did not cause them — they reproduce on the
pre-change binary as well — and the over-merge counts above say the term did
not enlarge the class either.

## 6. Not done

- **The hints-off class is different and was not touched.** Its top 60 is
  full atomisation: `お姉ちゃん` → `お|姉|ちゃん` (39 sentences), `林間学校` →
  `林|間|学校` (19), `四足歩行` (8), `乙女`, `舎弟`, `魔性`, `お嬢様`,
  `心の中`. 24 of 60 are atomised to bare single kanji and 17 split into three
  or more pieces, against 8 and **zero** hints-on. `CONTENT_MATCH` does not
  reach the two-kanji shape there: with no tokenizer `LEN_GAIN` is off
  entirely, so the gap is `2n - MATCH - CONTENT_MATCH`, which grows with `n`
  instead of cancelling. 43 of that top 60 are that shape. **Gold cannot see
  it** — `jparser-cli diff`, the corpus's sampling signal, requires `--hints`,
  so hints-off-only disagreements were structurally unreachable by the
  sampler.
- **`いた`, the `ってる` family and `からね` are not fixed.** §5 is a
  measurement, not a queued task; each wants its own adjudication and its own
  score run.
- **How much spurious-merge margin `ことになっている` has left is unmeasured.**
  §3.3.
- **`夕陽` is corpus contamination and any gate built off this list must
  exclude it.** All 33 occurrences are one game's character name. The
  adjudication rule has no class for a proper noun, and this one is 11% of the
  hints-off top-60 volume.
- Everything the previous handoff's §4 left open: gate 6's eighteen sentences
  are still not in the repository, `忍` still reads `しのび`, the `[noun]達`
  class is still open, and the engine still romanises `いっち` as `icchi`
  where the frontend's `itchi` is correct.
