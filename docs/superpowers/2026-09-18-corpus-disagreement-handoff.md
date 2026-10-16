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
| `bdfe9c6` | the review's findings: the term's second mode, its arithmetic test, two dead lexicon rows, and the probe's ranking |
| `30726c4` | `assemble::order` stops demoting a kana spelling JMdict calls normal — §6 |
| `0983fd7` | `table.rs`'s `てる`/`でる` rows, the ている contraction — §7 |
| `79ccb71` | `table.rs`'s `ている`/`でいる` rows and the re-pointed gate row — §8 |
| this one | the gate acceptance tightened, `CHAIN_BLOCK`, `KATAKANA_MIN_PIECE` — §9, §10 |

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

To re-run the whole thing:

```text
git worktree add <path-outside-repo> fb450f6^
# in that worktree, with a dumper of your own beside its examples/:
cargo build --release -p jparser --features mecab
jparser-cli build-index --engine v1 <JMdict_e.xml> <v1-index>   # format 3
# in this repository:
jparser-cli build-index <JMdict_e.xml> <v2-index>               # format 6
cargo run --release -p jparser --features mecab --example corpus_disagree -- \
    <v2-index> <corpus.txt> <v1-dump.tsv> [--hints <system.dic>] [--top N]
```

The dumper in the worktree is forty lines around `jparser::parse` and is
deliberately not described further here: it is GPL-2.0-only code and this
document is not the place to carry it.

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

Gates: spurious merges **26 of 26**, 658 tests 0 failed 1 ignored, `fmt` and
`clippy --all-targets --features mecab` clean, `bench` interleaved three
rounds each way (hints on 536 / 560 / 531 ms before against 524 / 529 / 531
after; hints off 579 / 609 / 587 against 589 / 596 / 596 — spreads overlap),
scaling unmoved within its read noise — two readings of the same binary gave
1.00x and 1.05x hints on, against 1.05x before, and 0.98x off against 0.97x.

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
occurrences. FIXED, §7.** `思ってる` → `思` + `っ` + `てる`, where `てる` glosses 照る
"to shine"; `言ってる` → `言`("word") + `っ` + `てる`; `遊んでる` → `遊` + `ん`
("yes; yeah") + `でる`(出る). Shape: a lone `っ`/`ん` segment followed by a
segment opening `て`/`で`. It is a hint artefact — IPADIC cuts `言っ`+`てる`,
JMdict wants `言って`+`る`, `is_allowed` forbids the JMdict span, and the
engine falls back to a bare kanji. Hints off: `言って`|`る`, 3 rows and 9
occurrences. **No signal can see it**: no single entry covers `思ってる`, so
`sht` and `rdg` are structurally impossible, and readings 2–3 do not differ
there, so `alt` is quiet too. 75 of the 122 fire nothing at all.

**2. `いた`, 218 sentences — the largest single count in the corpus. Fixed:
the gloss half in §6, the span half in §8.** `待っていた` → `待って` + `い` + `た`, where `い` glossed "the
Boar (twelfth sign of the Chinese zodiac)". Hints off it is `待って`|`いた`.
Signals: `alt=v1` only, which put it at rank 316 under the old ordering and
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

## 6. The `uk` clause, and the two routes refused beside it

`PRONOUNCE_CROSS` charges 25 per boundary a kana match of a kanji entry
crosses, and `assemble::order` demotes such a match below a `PRIMARY` one.
Both are right for a kana *spelling* of a kanji word. Both are wrong for an
entry JMdict tags `uk`, "word usually written using kana alone" — 9,580 of
JMdict's 218,449 entries, `居る`, `見る`, `有る`, `成る` among them — where
the kana **is** the normal spelling. `履く` and `穿く` are not `uk`, which is
what makes the tag a discriminator rather than a blanket.

**Shipped: the ordering half.** `order`'s `PRONOUNCE` clause is exempted when
any sense carries `uk`, read straight off `SenseData::misc` rather than
through a new flag bit. Entry top-1 **0.9400 → 0.9500 hints on and 0.9358 →
0.9507 hints off**, +13 and +19 hits on unchanged denominators, and **every
other cell in both states is identical to the digit** — `order` runs after the
DP, so no boundary can move. `待っていた`'s `い` now glosses "to be (of animate
objects); to exist". The snapshot moved in nine places, all gloss list
reorderings, and `時間がない` now leads with 無い "nonexistent" instead of the
negative auxiliary.

**Refused, with numbers: `uk` as a `WordFlags` bit.** Built by flagging a `uk`
entry's reading `PRIMARY` instead of `PRONOUNCE`, which carries the tag into
`cost::PRONOUNCE_CROSS` too. It is better on nearly every cell — entry top-1
0.9497 / 0.9507, `lexical` 82 → 87, `strict` 1745 → 1737, `tolerant` 1932 →
1964 — and it **fails success criterion 3**: `ことになって` swallows its `に`
and the 26-row `spurious_merges.txt` gate goes to 25, because `なる` is `uk`
as well. `transparent` 40 → 37 and `false-match` 30 → 29 besides.

**Refused, with numbers: widening `PRONOUNCE_CROSS`'s chain exemption to the
chain's start.** The exemption is `a < p && p < b` over the characters the
deinflection chain consumed, so the one cut it most obviously describes — the
junction between a stem and the inflection after it — pays in full. `a <= p`
fixes `待っていた`'s span outright. It is refused because the junction cannot
tell that from the spurious class: `はこう` is `履く` Volitional with the stem
`は` and the chain on `こう`, structurally identical, and `くるまではこう` is
criterion 3's own named sentence. Measured hints on: `false-match` 30 → 26,
`transparent` 40 → 37, entry top-1 0.9400 → 0.9314, breaking `はこう`,
`はいた`, `いたから`, `とが` and `のか` ×3.

So `待っていた` still reads `待って`|`い`|`た` hints on. The gloss is right and
the span is not, and nothing in the current term set separates a verb stem
from a particle.

## 7. The てる / でる rows

思ってる read 思 | っ | てる, with てる glossed 照る "to shine"; 遊んでる read
遊 | ん | でる, with でる glossed 出る. 122 region surfaces, 300 corpus
sentences, and **no signal could see any of it**: no single JMdict entry
covers 思ってる, so `sht` and `rdg` are structurally impossible, and readings
2–3 do not differ there either.

**Why nothing could cover it.** IPADIC tokenizes 思っ|てる. JMdict's
思って ends strictly inside the てる token, so `cost::is_allowed` refuses it;
`relax::Relaxed` reopens only a token no candidate can cover and 照る covers
this one; and 思っ alone is 連用タ接続, which `BOUND_FORMS` stopped from
standing alone when the sokuon-tail branch fixed 優しいっ. Three constraints,
each right on its own, leaving the region uncoverable.

**JMdict does carry てる and でる** — `[primary]` "to be ...-ing", and they
conjugate, so `思ってろ` already matched `てる` Imperative. The entry cannot
help here: reaching it needs a *separate* span, and the only span that could
precede one is `思って`, which is the span `is_allowed` refuses. An earlier
draft of this document and of `table.rs` said JMdict had no headword; that was
wrong and is corrected here.

**The fix is three `ATTACHES_TO` rows**, the same shape the ちゃう family
already has — a contraction IPADIC tokenizes on its own — which put the
contraction *inside* one span rather than beside it:

```
("てる", "一段", "連用タ接続"),   # 思ってる, 待ってる
("てる", "一段", "連用形"),         # 食べてる, 見てる, してる
("でる", "一段", "連用タ接続"),   # 遊んでる, 読んでる
```

一段 because the contraction conjugates as one: てる / てた / てて / てない /
てます. でる mirrors the existing で row and takes 連用タ接続 only, for the
same reason で does — it follows an onbin stem and nothing else. It reaches
`遊んでる` and `遊んでない`; `遊んでた` and `遊んでます` keep the `だ` 連用形
route that was already there and are still labelled Past, **a pre-existing
asymmetry between て and で** measured on the previous binary and left alone —
fixing it means inserting `でる` above `だ`, which relabels bare `遊んで`
"Progressive", the same trap as below.

The label is **Progressive**, and it is the **second** word in
`conjugation_labels.txt` its Wikipedia terminology list does not carry.
An earlier draft claimed the opposite. That article discusses ている under its
gerund and infinitive sections with no one-word English gloss and uses
"continuative" only as an alias for 連用形, so Progressive has exactly
Completive's status: the standard term for the aspect, from Wikipedia's
"Grammatical aspect". The aspect is genuinely ambiguous — progressive for
`走ってる`, resultative for `知ってる` — but unlike れる/られる the readings are
not balanced, so the primary one is used unqualified, which is the Completive
precedent rather than the Passive/Potential one.

**Row order is load-bearing, and it was measured the hard way.** 一段's
未然形 and 連用形 have empty suffixes, so てる's own surfaces include a bare
`て` — the same surface as the Te-form auxiliary, at the same trie node, at
the same depth. `lex::dedup_shallowest` keeps whichever `Table::new` inserted
first. Inserted **above** the て/で rows, every plain te-form in the corpus
relabelled "Progressive": 走って, 遅れて, 開けて, 遊んで, して, 待って. The gold
corpus did not register it; `parse_sentences.txt`'s snapshot did, which is the
third time that fixture has caught what the corpus could not. Below the て/で
rows, `て` wins the bare node and `てる` keeps every node `て` does not reach.

**What it costs.**

**The rows alone were not shippable, and a before/after over the corpus is
what showed it.** 一段 gives both 命令ｙｏ and 未然ウ接続 the suffix よ, so
`てる` and `でる` carried `てよ`/`でよ` — the sentence-final particle. 22
occurrences absorbed it (`待ってよ`, `返してよ`, `答えてよ`, `呼んでよ`), and
five were much worse: `なく` is 五段・カ行イ音便, so `な` + `い` is a 連用タ接続
site, and `忘れないでよね` parsed as `忘れ` + `ないでよ` + `ね` — the negation
destroyed and the span glossed `なく` "to cry; to shed tears". A wrong
headword stated confidently, which is worse than a missed merge.

`AUX_FORM_BLOCK` is the answer: four `(lemma, form)` rows that stop the table
generating a form at all. The contraction's imperative is `待ってろ`, never
`待ってよ`, so blocking both よ-forms is right on the language as well as on
the corpus. It costs the one `てよう` in 8,334 lines.

| | strict | tolerant | entry top-1 | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- | --- |
| on, before | 1745 | 1932 | 0.9500 | 152 | 82 | 30 / 32 | 40 / 42 |
| on, after | **1746** | **1935** | 0.9507 | 152 | 82 | **30 / 32** | **40 / 42** |
| off, before | 1633 | 1775 | 0.9507 | 113 | 52 | 22 / 32 | 39 / 42 |
| off, after | **1634** | **1778** | 0.9506 | 113 | 52 | **22 / 32** | 39 / 42 |

Nothing falls. The reachable-chain constant goes 50,419 → 60,926 — test-walk
only, and `bench` reads 1.02x in both states with interleaved spreads that
overlap, the same answer the ちゃう rows got. 658 tests, 0 failed, 1 ignored;
spurious merges 26 of 26; `tsc` clean and `vitest` 375 passed.

**What it still costs.** Before/after over the same 8,334 lines: 1,225
sentences change, 1,358 new spans appear, and of those

- **34 absorb a sentence-final ね or な.** `してね` reads `する` Progressive
  Negative where the text is a request, because 特殊・ナイ's 体言接続特殊２ *is*
  `ね` and `してねえ` really is that contraction. The fix is a **chain-level**
  block — `ない` may not attach to `てる` at 体言接続特殊２ — which the current
  `(lemma, form)` shape cannot express. Named, not built.
- **3 eat a kanji adverb**: `決してない` as `決す` Progressive Negative, because
  `決す` is サ変・ースル with 連用形 `決し`. Hints-on only.

37 against about 1,320 correct contractions, and the gate is 26 of 26
throughout. **An earlier draft of this section said "of 428 newly merged
regions exactly one has v1 splitting a lone particle inside it".** That number
came from the v1-comparison probe, which is not a before/after of this change
at all; the figures above are, and they are the ones to trust.

The homographs the rows could have broken are untouched. All 441 JMdict
headwords ending てる/でる were swept before and after: **0 gain a span**, 9
lose one (`付いてる`, `来てる`, `変わってる`, `言えてる`, `背負ってる` — all
lexicalised contractions, all wins). `捨てる`, `育てる`, `撫でる`, `照る`,
`出てる` are byte-identical, because `lex::allows_chain` requires the
reconstructed key's entry to carry a POS matching the chain's class: `捨る`
and `育る` are not entries at all, and `当る` is `v5r`, not 一段.

## 8. `ている` / `でいる`, and the gate row that was re-pointed

`待っていた` read `待って` | `い` | `た`, with `い` glossed "the Boar (twelfth
sign of the Chinese zodiac)". §6 fixed the gloss by exempting `uk` entries
from `order`'s PRONOUNCE demotion; the span stayed wrong, and two routes to it
were measured and refused there.

The third route is the one the `てる` rows had already taken: **`ている` is an
auxiliary, and every other auxiliary this engine knows already merges** —
〜た, 〜ない, 〜ます, 〜たい, 〜ちゃう, and as of §7 〜てる. Splitting it was never
a policy, it was a missing row, and after §7 the same construction had two
granularities depending on which spelling the speaker used.

```
("ている", "一段", "連用タ接続"),   待っている, 思っていた
("ている", "一段", "連用形"),        食べている, 見ていない, している
("でいる", "一段", "連用タ接続"),   遊んでいる, 読んでいた
```

plus the same two よ-forms blocked, for the same reason as §7.

| | strict | tolerant | entry top-1 | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- | --- |
| on, before | 1746 | 1935 | 0.9507 | 152 | 82 | 30 / 32 | 40 / 42 |
| on, after | 1714 | **1948** | **0.9534** | 152 | 82 | **30 / 32** | **40 / 42** |
| off, before | 1634 | 1778 | 0.9506 | 113 | 52 | 22 / 32 | 39 / 42 |
| off, after | 1610 | **1791** | **0.9513** | **118** | **56** | **23 / 32** | 39 / 42 |

**The strict fall is decomposed, not assumed**, which §5.2's own rule
requires. Of 25 new spans on the gold corpus hints on, **23 have both
endpoints on gold boundaries** and 2 do not: `当惑|していた` against gold's
`当惑して|いた`, and `行き|あっていました` against gold's `行きあって|いました`. In
both the offending left cut is **pre-existing** — the engine already read
`当惑|して|い|た` — so the merge coarsens the auxiliary tail of a boundary that
was already misplaced rather than inventing a word across a good one.
`tolerant` rising in both hint states says the same thing from the other side.

**On the corpus**: 652 sentences change, 754 new spans, **336 stray `い`
segments and 239 stray `た` segments disappear**, and **zero** new spans end in
a sentence-final particle — the failure mode §7's rows had to be gated against
does not arise here. Top new spans are `している` 95, `していた` 48, `思っていた`
19, `見ている` 18, `しまっている` 16.

**The gate row, and why it was re-pointed rather than weakened.**
`spurious_merges.txt`'s `してい` row asserts a cut inside that surface in
`b:wn25517-3`; with `ている` the sentence reads `予定` | `に` | `している`, so
there is none. The row exists to stop 1308000 (指定) or the kana-unified
子弟/師弟 taking that span. Neither does: the `に` is its own segment, nothing
is absorbed, and the covering entry is `する` Progressive. **Verified before
touching the fixture** — across eight readings of that sentence 指定 never
takes the span, and `してい` parsed alone now leads with `する` Progressive
where it used to lead with 子弟 "children; sons". The test now accepts a
*strictly longer* matched word covering the surface **only when its top entry
is not the row's own `ent_seq`**, and a segment sitting exactly on the surface
still fails whatever it matched. Column 2 of that fixture was read by nothing
until now.

This is the third change this session to be weighed against the 26-row gate
and the first to be allowed past it. The other two — widening
`PRONOUNCE_CROSS`'s chain exemption and `uk` as a `WordFlags` bit — were
refused because the merge **was** the guarded class: `ことになって` swallowing
its `に`, and `はこう`. Here it is not, and the difference is checkable rather
than a matter of taste.

`いた` standing alone is still `い` | `た` — 「猫がいた」 reads `猫`|`が`|`い`|`た`.
Only the auxiliary use after a te-form is fixed.

## 9. Three corrections the review and the design pass produced

None of these moves a gold cell. All three were found by reading, not by the
corpus.

### 9.1 The gate acceptance §8 added was too loose

§8 widened `the_particle_absorbing_merges_are_gone` to accept a strictly
longer word covering the surface when its top entry is not the row's
`ent_seq`. The predicate was `s.start <= at`, and **that let the `になって` row
pass while `ことになって` swallowed its `に`** — the exact parse the
`uk`-as-`WordFlags` route was refused over in §6. A 7-character segment
starting at 37 covers 39..43, and its top entry is 事になる, not 1599900.

`s.start == at`. The class is *a preceding particle absorbed into the
following word*: a covering segment that starts **left** of the surface has
absorbed something, one that starts *at* it has not. §8's own case is
untouched — `してい` is at 52 and `している` starts at 52.

**Verified by re-running the refused variant.** With `<=`, the `uk` flag bit
passed 26 of 26; with `==` it fails on `になって` again. The claim in §6 and in
§5.2 that criterion 3 is what refuses it is backed by a running check once
more — it was not, for one commit.

### 9.2 The よ-block does not transfer to `ている`

§8 blocked 命令ｙｏ and 未然ウ接続 on `ている`/`でいる` by analogy with §7.
The analogy is false: `てよ` collides with the sentence-final particle because
`待って` is a word and `よ` is a particle, and `ていよ` cannot decompose that way
because `待ってい` is not a standalone anything. What the block actually cost
was `待っていよう` and `遊んでいよう`, ordinary Japanese. Unblocked: 2 corpus
sentences change, both gains, no particle absorbed. Four fixture rows added,
chain constant 72,049 → 73,731.

### 9.3 `CHAIN_BLOCK`, for the debt §7 shipped

§7 knowingly shipped 34 occurrences where a sentence-final ね or な is read
as the negative auxiliary. **The handoff named the wrong form for it**: 特殊・ナイ
has no 体言接続特殊２. The `ね` is `ぬ` at 仮定形 — which is why the label read
*Conditional* — and the `な` is `ない` at ガル接続. Two defects, 25 and 9.

Neither auxiliary can lose the form: `行かねば` and `食べな` are committed
fixture rows. So the block is keyed on the **pair** — `(outer lemma, outer
form, inner lemma)`, eight rows — and consulted at one place in
`Table::search`, after the recursion returns and before the step is pushed,
where `steps.last().aux` is the inner lemma. `aux` is `None` for a headword's
own inflection, which is what keeps `食べな` and `行かねば` alive.

Ledger over 8,334 lines, hints on: 34 sentences change, **30 release the
particle correctly**, 1 is neutral, **3 regress** — the `〜てなさそう` shape
(`わかってなさそうな顔`), where the `な` really is the negative and the
disambiguator is the `さ` in the *next* span, which a chain search
structurally cannot see. No tighter key avoids them. `ねえ` is untouched:
`してねえ` and `知らねえ` are `ない` at 音便基本形, a different form, and 150
spans ending `ねえ` are byte-identical.

**The `(lemma, form)` alternative was priced and refused.** Neither half fixes
the 34 alone (9 and 25 left over); together they change 167 sentences instead
of 34, and 391 instead of 79 hints-off, and delete two adjudicated fixture
rows. They *also* clear about 115 pre-existing absurd merges — `からね` and
`からな` ×90, glossed `かる` "to spur on" — which is a real measured win and
belongs on its own branch with its own adjudication, not smuggled in here.

`決してない` stays broken: 2 occurrences, plus 1 `決してなく`. `決す` and `愛す`
are both サ変・ースル, so any class-keyed refusal kills `愛してる`. Nothing
cheap fixes it.

## 10. `KATAKANA_MIN_PIECE`, the mirror of `relax::MIN_PIECE`

`センパイ` read `セン` ("line") + `パイ` ("pie") in 70 corpus lines and correctly
in 5, and **the only thing that varied was whether IPADIC happened to call it
one token**. JMdict has no `センパイ` headword; the span reaches 先輝 through
`kana::fold`, so the record is the hiragana reading, is PRONOUNCE-only, and
pays 25 for the interior cut. Merged it costs 8 against the split's 8 and then
loses by the crossing — `PRONOUNCE_CROSS` alone decides it.

`relax::MIN_PIECE = 3` already answers the converse question: an unmatchable
katakana token may be *reopened* only when it tiles into pieces of three or
more, because "it decomposes into real words" and not "it is katakana" is the
test. The mirror is that a cut *inside* a katakana run leaving a piece shorter
than three is the tokenizer failing on a word it does not know, so it is not
evidence and pays nothing.

Swept over every katakana run in the corpus: 1,110 distinct runs of three or
more characters, 110 split hints-on. **Eight are real words shattered** —
`センパイ`, `アブナイ`, `サイアク`, `ヘンテコ`, `シロクロ`, `ハンパ`,
`モンモン`, `ナイナイ` — and every one leaves a piece shorter than three.
**63 are correct compound decompositions** — `オンライン|フレンド`,
`レトルト|カレー`, `クリスタル|ソーダ` — and every one has both pieces at
three or more. Zero overlap.

Result: 89 sentences change, 90 new spans, all of them the eight words plus
`テキトー`, `スジモン`, `ノリノリ`, `カツオブシ`. **Every gold cell is identical
to the digit in both hint states.** Safe against criterion 3 by construction:
all 26 rows of `spurious_merges.txt` are hiragana or kanji, and the one
mixed-script row is not an all-katakana run.

`ヘンタイ` (10) and `ヨシヨシ` (6) are **not** reached and are a different cause:
変態 carries a genuine katakana `r_ele` flagged `[reading, common-line]` with
no `common`, so the merge forfeits the COMMON discount its parts keep. That is
the `[primary]`-without-`[common]` class again.

## 11. Not done

- **The `〜てね` / `〜てな` absorption the `てる` rows introduced**, 34
  occurrences, and the chain-level block that would fix it (§7).
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
- **`からね` (44) and `からな` (39) are not fixed**, nor is `いた` standing
  alone outside the auxiliary (§8). §5 is a
  measurement, not a queued task; each wants its own adjudication and its own
  score run, and §6 records the two routes to `いた` that were tried and
  refused.
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
