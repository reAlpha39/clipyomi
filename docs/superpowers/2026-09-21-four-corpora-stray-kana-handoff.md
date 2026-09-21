# Four more corpora, and the class the probe could not rank — handoff

**Date:** 2026-09-21
**Branch:** `feat/corpus-disagree`, tip `a0f4ac6`
**Predecessor:** [`2026-09-18-three-corpus-adjudication-handoff.md`](2026-09-18-three-corpus-adjudication-handoff.md)

The previous session adjudicated three corpora side by side and left a list of
open classes, two of them unmeasured. This session ran the same probe on four
further corpora — 75,782 sentences, none of them seen before — to settle which
of those classes are Japanese and which were one writer's habits, and found one
class that is larger than everything on the list and that the probe's ranking
cannot show.

§1 to §8 are the measurement, written before anything was changed: no code, no
gate, and every artifact throwaway. §9 is the one change the measurement
justified, shipped the same day — `640074c`, the 〜てくる / 〜ておく / 〜てみる
rows §4 priced.

## 1. The corpora, and how to rebuild the run

The four are in `D:\Code\oss\vn-text`, outside the repository, alongside the two
the predecessor used. Same recipe as before: table `artificialtrans`, column
**`source`**, distinct non-empty rows with whitespace stripped. Re-dumping
`恋愛、はじめまして` with it returns exactly **8,334**, the predecessor's number,
which is how the recipe was re-validated before anything else ran.

| corpus | file | distinct sentences |
| --- | --- | --- |
| start | `Start_1766545408…sqlite` | 22,515 |
| amakano | `amakanoPE_1767959437…sqlite` | 15,995 |
| kf3 | `kf3FHD_1758285994…sqlite` | 21,224 |
| triple | `トリプルペアリング_Crack_1780067045…sqlite` | 16,048 |

75,782 in total, against the predecessor's 75,767, so per-10k rates are directly
comparable between the two sessions. **`kf2` is not in that directory** — it is
still `vn_data/KF2ifFHD_…sqlite`, and `kf3FHD` / `kf3if_sFHD` are different
titles, not a rename.

Copyrighted game script: local probe input only, and nothing derived from it is
in this repository.

The v1 side is produced out of tree as before. The detached worktree at
`D:\Code\oss\clipyomi-v1` survived at `361a77f` with its untracked
`examples/v1_dump.rs`. **No v1 index was on disk** — `dict/gen-2` is format 5 and
`gen-3` format 6, both v2 — so it was rebuilt from a scratch copy of
`%APPDATA%\dev.jparser.clipyomi\source\JMdict_e.gz` with
`build-index --engine v1`: 486,553 keys, 619,717 records, 121,048 exact stems,
398 v5-fallback, 5 empty. That index lives in the session scratchpad and is gone
with it; whoever runs this next rebuilds it, which takes about a minute.

`corpus_disagree` was run at `--top 6000`, not 2,000, for the reason the
predecessor's §9 gives: at 2,000 the entire zero-signal tail is invisible. Runs
took 39 to 66 seconds per corpus and state.

## 2. The merge direction holds on four more writers

| corpus | state | we merge, v1 splits | we split, v1 merges | ratio |
| --- | --- | --- | --- | --- |
| start | on | 2,748 | 6,479 | 2.36× |
| amakano | on | 1,598 | 3,507 | 2.19× |
| kf3 | on | 1,837 | 4,194 | 2.28× |
| triple | on | 2,056 | 5,197 | 2.53× |
| start | off | 2,099 | 11,839 | 5.64× |
| amakano | off | 1,446 | 5,788 | 4.00× |
| kf3 | off | 1,309 | 7,254 | 5.54× |
| triple | off | 1,597 | 9,195 | 5.76× |

Regions, not occurrences. The predecessor measured 2.5–3.5× over the first three;
these four read 2.2–2.5× hints-on and 4.0–5.8× hints-off. Over seven corpora and
roughly 151,000 sentences the sign has never changed, so the ranking is still
reading the dominant direction and the blind side is still not small.

556 surfaces appear in all four hints-on heads and 370 in all four hints-off.
Those totals are not comparable with the predecessor's 300 and 232, which were
taken over three corpora at `--top 2000`.

## 3. The class the ranking cannot show: stray small kana

**The engine emits a segment that is one sokuon, small vowel, small ya-row kana
or long mark in 16.5% of sentences hints-on** — 26,885 such segments over the
four corpora, 3,547.7 per 10k — and 17.4% hints-off (27,603, 3,642.4 per 10k).
Composition hints-on: `っ` 12,143, `ぅ` 3,783, `ぁ` 3,094, `ゅ` 2,528, `ぇ` 1,869,
`ぉ` 1,195, `ー` 786, `ぃ` 767.

This is not on any previous list, and the reason is structural rather than an
oversight. **The probe ranks per surface**, and every one of these spells
differently — `あんっ`, `ああっ`, `よぉ`, `ですぅ`, `ちゅ`, `ごきゅ` — so the class
scatters across thousands of rows with small counts each. Only `ああっ` (57.7 per
10k) and `あんっ` reached a head at all. Aggregating by *shape* rather than by
surface is what makes it visible, and that took `span_dump`, not the probe.

Two sub-shapes, and they want different answers:

- **74% attach to a preceding kana run.** `ち|ゅ` 1,113, `あぁ|っ` 659, `ん|ぅ` 615,
  `ふ|ぅ` 436, `ごき|ゅ` 367, `ひ|ゃ` 351, `はぁ|ぁ` 315, `ぐ|っ` 307, `あん|っ` 293,
  `よ|ぉ` 256, `です|ぅ`. Onomatopoeia and emotive lengthening, where the whole
  sound run is one span and the cut is pure damage. `だら|っ|とし` is the clearest
  case: `だらっと` is a JMdict adverb.
- **26% follow punctuation or open a line**, and for those **no correct merge
  exists** — `、|っ|と` is a stray the writer put there. Any fix must leave them
  alone or it will invent spans.

**Two of them carry a false gloss rather than nothing.** The stray `ぇ` is shown
with the entry `ェ` **1,869 times** and `ー` with `ー` 785 times; the `っ` strays
are unmatched. So the class is not merely noise in the segmentation — a sixth of
all sentences hands the reader at least one segment to skip, and some of those
open a dictionary entry for a sound.

**v1 does not fix it, which is why it is here and not in the predecessor.** v1
cuts the same way in 24% of cases hints-on, so 76% do enter the probe as
disagreements — but its merges are junk of a different kind, gluing the stray to
punctuation or to the wrong neighbour (`だ|らっと`, `ですぅ～`, `、っ`). Read against
the predecessor's §9 note, this is the half-visible case: the probe emits the
region, the ranking buries it, and the second opinion is no better than ours.

The register matters. These are eroge scripts and the gold corpus has almost
none of this shape, so **no gate in the repository moves when this class moves**.
That is the first thing whoever takes it has to build.

**What the 16.5% is not.** It counts segments, not moments a reader was hurt,
and the two numbers are not close. A quarter of the class has no correct merge
at all, and a large part of the remainder is `ちゅ`, `ごきゅ`, `んぅ` — a reader
scrolling past a moan does not need it lemmatised, and a one-character span
there costs them nothing they would notice. The defensible core is the rest:
`だらっと` (a JMdict adverb cut in half), `ですぅ`, `よぉ` — a real word, or a
real word plus a lengthening, severed. **Anyone pricing a fix should price that
core, not the headline**, and should weigh the cheapest candidate first: never
segment inside a run of kana that carries no word, and show the run as one
unglossed span. That takes most of the value without a cost-model change, and
it leaves the 26% alone by construction.

## 4. 〜てくる, measured at last, and bigger than 〜ていく was

The predecessor's §7 shipped the `ていく` / `でいく` rows and named 〜てくる, 〜ておく
and 〜てみる as the same defect, unmeasured. Counting literal windows over these
four corpora and asking whether the engine cuts at the `て|` junction:

| class | windows | cut hints-on | cut hints-off | split per 10k, on |
| --- | --- | --- | --- | --- |
| てくる | 3,232 | 2,368 (73.3%) | 74.4% | **312.5** |
| てみる | 425 | 422 (99.3%) | 100% | 55.7 |
| ておく | 241 | 223 (92.5%) | 100% | 29.4 |
| ていく | 1,670 | **44 (2.6%)** | 4.7% | 5.8 |

Two findings in one table. **The `ていく` fix holds at corpus scale** on four
corpora it was not tuned against, in both hint states — 2.6% residue hints-on,
and the predecessor's own §7 example class is gone. And **〜てくる splits at about
twice the rate 〜ていく was reported at** (157 per 10k there against 312.5 per 10k
split occurrences here), which makes it the largest named segmentation defect
with a known fix.

`寄せて|くる`, `強調して|くる`, `して|おく`, `やって|おく`, `探して|みる`,
`訊ねて|みる`, `見渡して|みる`. The 26.7% of `てくる` windows that do not split are
mostly JMdict headwords like `持ってくる`, exactly as §7 predicted.

Anyone writing these rows should re-read §7 first: **both attachment sites or
half the verb classes stay unreachable**, and 〜てくる needs a *second* label word
because `label::fmt` drops a repeat from a chain carrying both directions.

## 5. The particle → godan class splits by hint state

The predecessor recorded this family as firing in both states. Measured as
whole-segment false merges — the surface emitted as one segment, which is the
defect — it divides:

| surface | whole hints-on | whole hints-off | what it displays |
| --- | --- | --- | --- |
| からな | 154 | 148 | `かる` "to spur on" |
| ないだろ | 141 | 94 | `なく` "to weep" |
| からね | 133 | 122 | `かる` |
| もんな | 44 | 42 | `もむ` "to rub" |
| のも | 1 | **472** | `のむ` "to drink" |
| のかな | 1 | **148** | `のく` "to step aside" |

474 wrong merges hints-on (62.5 per 10k) against **1,026 hints-off** (135 per
10k). `のも` and `のかな` are hints-off only in this register; the other four
survive the hint dictionary and are the ones a fix has to reach. The control
holds: grepping for `刈らな` or `駆らな` returns 0 on all four corpora, so the class
remains 100% false-match by construction.

Raw literal frequency, for the denominator: `のも` 73.9 per 10k, `からな` 65.8,
`ないだろ` 27.8, `からね` 26.7, `のかな` 25.3, `もんな` 7.4, `とかな` 4.9, `ぐぐ` 3.2.

### 5a. The particle class is in the gold corpus too

Building the gate turned one up where the corpus work had never looked.
`従っていたから` (`a:003521-2`) parses as **one seven-character span glossed
`従う` "Progressive Want-to"** — the `から` absorbed into the verb chain and
re-lemmatised, which is exactly §5's clause-final-particle class, the one
measured at ~237 occurrences per 10k on visual-novel dialogue.

It is **byte-identical before and after §9's rows**, so it is neither caused nor
fixed by them, and it is not new — it has presumably been there since the
corpus was adjudicated. What is new is knowing that the class is not a
register artefact of eroge dialogue: it reaches ordinary prose, in the very
corpus every gate is measured against. Whoever prices the fix for §5 now has a
gold sentence to test it on, which the class did not have before.

## 6. Smaller confirmations, and one small defect worth a row

- **`やだ` never survives.** 84 occurrences, **0** emitted whole in either state,
  displayed as `や` + `だ`. JMdict covers it, the probe scores it `alt=v1+sht`,
  and it heads the two-signal rows in all four corpora. Small (11 per 10k) and
  unambiguous.
- **The hints-off atomisation class reproduces exactly.** `肉棒` 426 whole
  hints-on against 0 hints-off, `気持ちよく` 530 against 0, `お尻` 318 against 0,
  `気がする` 136 against 0. Predecessor §4 row 1, unchanged, and still the largest
  hints-off class.
- **The quotative class is hints-on only here, and ten times rarer.** `と言った` is
  merged into the "such as" entry 41 times of 61 occurrences hints-on and **0
  times hints-off** (5.4 per 10k against the predecessor's ~56); `と言って` adds 69.
  `と言う` merges 61 / 80 and that is correct, not a defect.
- **The `ておく` contractions are nearly absent**: `といて` → `とく` 16 hints-on and
  19 hints-off, `言っとく` and `っとく` never merged. ~2 per 10k against ~22.

The last two are register-dependent, which is the point of running more writers:
a class can be real and still be rare in the text a given reader is parsing.

## 7. Method notes worth keeping

- **`--top 6000`, and even that truncates.** The predecessor's §9 warning applies
  to every run; 2,000 hides the whole zero-signal tail.
- **The probe ranks surfaces, so it cannot see a shape.** §3's class is the
  demonstration. When a defect's surface varies (onomatopoeia, names, numerals),
  the ranking is the wrong instrument — dump spans and aggregate by shape.
- **`span_dump` plus a literal window gives the denominator the probe cannot.**
  "Of the 3,232 `てくる` in this corpus, 2,368 are cut" is a claim the probe's
  per-region counts cannot make, and it is the number a decision needs.
- **A literal window is noisy and the noise is one-directional**, so read the
  rates in §4 and §9 as approximations rather than to the decimal. `てみな`
  matches inside `てみない`, which is correct Japanese, and `ておか` inside
  `すぎておかしく`, which is not the auxiliary at all — both were caught only by
  reading examples, and both inflate a count. The direction of every figure
  here survives the noise (73.3% → 1.6% is not a rounding question); the second
  digit does not. Read the examples, always, before believing a rate.
- **The `matched` flag on a one-character segment is worth auditing on its own.**
  It found the `ぇ` → `ェ` gloss, which no reading- or cover-based signal reports.
- On this shell, Python needs `PYTHONIOENCODING=utf-8` or every Japanese column
  prints mojibake and the tables look empty.

## 8. Not done

- **The heavier auxiliary metric**, if rates and sub-classes are ever wanted:
  §10's gate is a pass/fail list, not a number that can be tracked across runs.
- **The stray-kana class has no gate.** It is the largest thing measured here and
  the gold corpus contains almost none of it, so no cell in `score` moves when it
  moves. A gate comes first, then a fix, and the fix must not touch the 26% that
  follow punctuation.
- ~~**The 〜てくる / 〜ておく / 〜てみる rows.**~~ **Shipped the same day** —
  see §9.
- **`やだ`.** One row, 84 occurrences, both states.
- ~~**Six corpora in `vn-text` were not run**~~ — PxC2, enkou, hitoduma2,
  ikenaiDX, kf3if_sFHD, おにキスＨ — a deliberate scope choice, four being enough
  to separate a writer's habit from Japanese. **All six were run the same day**,
  and `vn-text` is now exhausted — see
  [`2026-09-21-six-corpora-handoff.md`](2026-09-21-six-corpora-handoff.md).
- Everything the predecessor's §10 left open that this session only re-measured:
  the particle → godan family, the hints-off atomisation class, criterion 2 on
  the split parts, `BOUND_FORMS` for an auxiliary's own 連用タ接続, and the six
  darwin screenshot baselines.

## 9. Postscript: the three rows, shipped

`640074c`, the same day, because §4 had already priced it and §7 of the
predecessor had already written the method down.

Nine `ATTACHES_TO` rows — `てくる`/`でくる` as カ変・クル, `ておく`/`でおく` as
五段・カ行イ音便, `てみる`/`でみる` as 一段, both attachment sites for the
て-forms and 連用タ接続 alone for the で-forms. Labels `Approaching`,
`Preparatory` and `Tentative`; `てくる` could not also be `Directional`,
because the fixture's own ていく note had reserved a second word for it.

| class | before, on | after, on | before, off | after, off |
| --- | --- | --- | --- | --- |
| てくる | 73.3% | **1.6%** | 74.4% | **2.9%** |
| ておく | 92.5% | **13.3%** | 100% | **11.6%** |
| てみる | 99.3% | **1.6%** | 100% | **2.8%** |
| ていく | 2.6% | 2.6% | 4.7% | 4.7% |

`ていく`'s own column is the control and it does not move. What is left of
`ておく` is mostly the window counter rather than the auxiliary — `すぎておかしく`
matches the literal `ておか` — plus the hint-boundary class that splits
`やめておく` hints-on and parses it correctly hints-off.

**Two collisions the ていく work never met**, both measured on the corpora
before any row was written, both priced in the commit:

- カ変's 仮定形 `くれ`, 体言接続特殊 `くん` and 体言接続特殊２ `く` all belong to
  something else in running text — **2,455 `〜てくれ`** (くれる) against 7
  `〜てくれば`, and **919 of the bare `〜てく` are `〜てください`**. All three are
  in `AUX_FORM_BLOCK`, and `〜てくれ` and `〜てください` come out byte-identical
  to before.
- 一段's empty 未然形 makes `てみ` a stacking site. `やってみな` read Tentative
  **Negative** where the な is なさい (14), and **all 99 `〜てみせ` read
  Tentative Causative** where the word is 見せる. `CHAIN_BLOCK` gains the
  ぬ/ない pair its own doc asks for and a `せる` row under a new `*` form,
  because every せる form produces the collision.

Gold cells: hints on strict 1698 → 1694 with tolerant unmoved at 1948 and
compounds 152 on all three class lines; hints off strict 1593 → 1588, tolerant
1786 → 1785, compounds 118. **Zero damage hints-on** — 4 spans lost, 0 gained,
every one of them an intended merge with both endpoints on gold boundaries —
and hints-off's fifth is `行ってきちゃえ` + `ば` against gold's `きちゃえば`,
the right word under a different grouping. Chain walk 90,359 → 140,947,
latency unmoved on an interleaved A/B.

Residue named rather than fixed: `待っておいで` (5) and one `守りぬいてみせます`
whose span now ends at `てみ`.

## 10. The gate cannot score this class, and that is measurable

§9's change improved the parse on 75,782 sentences and **every gold cell it
moved, moved down**. That is the second change in this family in a row to do
that, and the pattern is not noise — it is the gold corpus's annotation policy.

| change | strict on | tolerant on | entry on | strict off | tolerant off | compounds |
| --- | --- | --- | --- | --- | --- | --- |
| `てる`/`でる` | 1745 → **1746** | 1932 → **1935** | → 0.9507 | 1633 → **1634** | 1775 → **1778** | 152 / 113 |
| `ている`/`でいる` | 1746 → 1714 | 1935 → **1948** | → **0.9534** | 1634 → 1610 | 1778 → **1791** | 113 → **118** |
| `ていく`/`でいく` | 1714 → 1698 | 1948 → 1948 | → 0.9528 | 1610 → 1593 | 1791 → 1786 | 152 / 118 |
| `てくる`+2 (§9) | 1698 → 1694 | 1948 → 1948 | → 0.9527 | 1593 → 1588 | 1786 → 1785 | 152 / 118 |

The first bought a gain on every column it touched. The second traded strict
for tolerant, entry and compounds. **The last two have no positive cell at
all.**

**The mechanism, measured rather than assumed: of the 34 〜て+auxiliary sites in
`gold.jsonl`, 34 are annotated as two spans and 0 as one** — `引きずって`|`いった`,
`沈みこんで`|`いく`, `頬張って`|`いる`. So an auxiliary-merge row is a strict loss
**by construction**: it takes two gold spans and returns one. Tolerant can only
be neutral, because it credits the merge as coarsening rather than rewarding it.
The compound metric contains no auxiliaries at all. Entry top-1 moves on a
denominator that the merge itself changes. There is no cell in `score` that can
say "the auxiliary is attached to its verb now", and there never was.

That is not an argument that the gold corpus is wrong. Its short-unit policy is
deliberate and everything else in the engine is scored against it. It is an
argument that **this class of change has been running without a gate for four
commits**, and that the corpus evidence has quietly been doing the gate's job —
which is exactly the position §3's stray-kana class is in, arrived at from the
other direction.

**Built, in `420725b`.** `tests/fixtures/auxiliary_merges.txt` is the mirror of
`spurious_merges.txt` — that file refuses merges the engine must not make, this
one requires the merges it must — with 32 rows over the gold sentences that
carry them. Each row asserts that gold still splits the surface (so no row can
pass by agreeing with gold), that the engine emits exactly one matched segment
there, and that the winning entry's label carries the auxiliary's own word. It
was falsified before being trusted: dropping the two `てくる` rows from
`ATTACHES_TO` fails three rows, one of them on the label alone
(`行ってきちゃえ` degrades to "Completive Conditional").

The heavier option — an adjudicated file in `compound_gold.jsonl`'s shape, with
a metric line of its own in `score` — was not taken. It buys rates and
sub-classes, and it costs the plumbing plus a new adjudicated fixture; the
compound metric took a phase to get right. The cheap gate closes the hole that
mattered: **removing an `ATTACHES_TO` row now fails a test instead of improving
a number.**

Three gold pairs are excluded and the fixture says why for each. One of them is
a finding in its own right — see §5a.

## 11. The hints-off premise, audited

The `LEN_GAIN` refusal and the largest open class in §4 of the predecessor both
lean on hints-off being the state users are in. `fdbbb61` changed that on
2026-09-21 and the audit is worth writing down, because the answer is not the
simple one.

**What actually changed.** `mecab_hints` still defaults to `false`, deliberately
— it records whether the 7.7 MB Vibrato dictionary was ever acquired, not a
preference. What the commit added is an acquisition on the download screen,
armed only when that screen's own button started the download, no hints
dictionary is published (`hints_available`), and no `hints-status` has arrived
since.

**So the hints-off column is now a draining population, not a default.** A
fresh install that downloads a dictionary lands hints-on. An established user
who never acquired hints publishes no dictionary either, so `hints_available`
is false for them too — and `state::is_missing_dictionary` routes
`ConjugationMismatch` and `VersionMismatch` to the same screen, which means
**the next index-format bump arms the acquisition for them as well**. What
stays hints-off after that is three groups: users who acquired hints and turned
them off (correctly excluded by the guard), users whose fetch failed — GitHub
blocked where EDRDG is not is the realistic case — and installs that have not
yet met a rebuild.

**No refusal reverses.** Each was checked against its own reasoning rather than
its framing:

- **`LEN_GAIN`** was already re-argued after the acquisition, in
  `2026-09-18-len-gain-resample-handoff.md` §11b: it stays gated on the severing
  class (`それならいい` → `それな` | `らいい`, once a sentence), and that argument
  never used the fresh-install premise. Unaffected.
- **The residue term** is refused on separability — `ながら` and `ように` are
  flag-identical — which is a property of JMdict, not of a hint state.
- **`nf`-band grading** is refused for shattering inflections. Same.

**What changes is priority, and two documents now read wrong.** The three-corpus
handoff's §8 says "the broken state is the one a fresh install runs in"; that
sentence is stale and should be read against `fdbbb61`. And the hints-off-only
half of §5 above — `のも` 472 and `のかな` 148 — plus the ~1,640-per-10k
atomisation class describe a shrinking audience rather than the default one.
They are still real defects for the three groups named above; they are no
longer the first thing a new user meets. **Before another session is spent on
the hints-off column, that is the number to ask for: how many users are in it,
and for how long.**
