# The sentence-final っ — handoff

**Date:** 2026-09-16
**Branch:** `feat/sokuon-tail`, off `develop` tip `0eadc7f`
**Predecessor:** [`2026-09-16-post-merge-handoff.md`](2026-09-16-post-merge-handoff.md)

One screenshot pair, ClipYomi against Translation Aggregator, on
`「ええっ、いいの!? 忍くん優しいっ!」`. Three differences, two of them ours.

## 1. What shipped

| commit | what it fixed |
| --- | --- |
| `0a0c104` | a sokuon with nothing to double no longer romanizes as `tsu` — `ええっ` was `eetsu`, and `っあ`/`っん` doubled the vowel and the `n` |
| `3dc632d` | `連用タ接続` is not a word on its own — `優しいっ` was `優し` + `いっ`, a bare 五段 onbin stem beating `優しい` plus an emphatic `っ` |
| `a286fd3` | `なのに` joins the user lexicon — second screenshot, `見た目はギャルなのになー`, where the right cover ranked second |
| `a9a6898` | `へへ` and `あたし達` join it — third screenshot; `へへ` was also romanizing as `e e` |
| `94eceeb` | `唐揚げ` joins it — fourth screenshot; the split read `とう` + `あげ`, not `からあげ` |

The romaji half is the same class of defect as `d29360d`: the frontend
keeps its own kana table and it had drifted from `crates/jparser`'s
romanizer. Every new expectation in `furigana.test.ts` was read off
`jparser-cli romaji` first. The one place the two still disagree is `いっち`
— frontend `itchi`, engine `icchi` — and the frontend is the correct one
there (traditional Hepburn `tch`), so the engine is what wants fixing.

The engine half is `BOUND_FORMS`, a one-name list in `table.rs`. Every
`CLASSES` form was inserted as a `Terminal` trie hit, so `search` would end
a chain at `連用タ接続`, a form that only exists before `た/て/だ/で`. A
`Terminal` that is bound is now emitted only when `required` is `Some` —
which is exactly when an auxiliary asked for it, so `行った` is untouched.

`ちゃう/じゃう/ちまう/じまう` had no `ATTACHES_TO` rows, and they are the only
other consumers of `連用タ接続`, so the refusal on its own left `買っちゃった`
with no chain through `買っ` at all. Eight rows (`連用タ接続` and `連用形`,
the pair `た/て` each need) restore it and take `食べちゃう` to one span.
They are free on the corpus: every cell below is identical with and
without them.

## 2. The numbers

Gold index rebuilt from `gold_jmdict_subset.xml`, 150 sentences, 2,163
spans. Release build, `--features mecab`, Vibrato dictionary at
`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

| | strict | tolerant | entry top-1 | compounds |
| --- | --- | --- | --- | --- |
| off, before | 0.7393 (1599) | 0.8003 (1731) | 0.9374 (1168 / 1246) | 0.6129 (114) |
| off, after | **0.7448 (1611)** | **0.8086 (1749)** | 0.9347 (1174 / 1256) | 0.6075 (113) |
| on, before | 0.8428 (1823) | 0.8826 (1909) | 0.9347 (1289 / 1379) | 0.6344 (118) |
| on, after | **0.8442 (1826)** | **0.8840 (1912)** | 0.9341 (1289 / 1380) | 0.6344 (118) |

Hints-on compound class lines do not move: `lexical` 46 / 112,
`false-match` 30 / 32, `transparent` 42 / 42. Read them, per the previous
handoff's §6, and not the ratio.

**Entry top-1's ratio falls in both states while its numerator holds or
rises.** The denominator is correctly-spanned tokens, and the change adds
spans: hints off, +6 correct out of +10 newly scored. Both v1 floors
(0.9159 / 0.9155) clear with margin, so criterion 2 still holds.

**The one real cost is one hints-off compound**, and `--compounds-detail`
names all three moved windows: `なきゃいけない` now merges as adjudicated
(`lexical` +1), `になって` and `いたから` now merge where the adjudication
splits them (`false-match` −2).

Bench, 10,040 chars, k = 3: 607 → 596 ms hints off, 539 → 543 ms hints on;
scaling 1.01x and 1.00x. Spurious merges 26 of 26. 656 workspace tests, 0
failed, 1 ignored (a doctest). `clippy --all-targets --features mecab` and
`fmt` clean. `vitest` 375 passed, `tsc --noEmit` clean.

The snapshot moved twice and both are corrections:
`待っ|ていた` ("severe") became `待って|いた` (`待つ` Te-form, `いる` Past), and
`ない` stopped offering `泣く`.

## 3. What this cost elsewhere

- **Three fixture rows deleted** from `conjugation_labels.txt`: `書い`,
  `行っ`, `高かっ`, the three bare `連用タ接続` stems, each labelled `-`. The
  table no longer generates them standalone, and the fixture's own rule is
  that rows may be added but never *quietly* dropped — the file carries a
  comment naming them and why.
- **28 rows added** for the `ちゃう` family, and one new label word,
  `Completive`. It is the only word in `label.rs` the fixture's Wikipedia
  terminology list does not carry; the fixture's header now says where it
  came from and why not "Regrettably".
- **The reachable-chain constant went 22,783 → 50,419.** Four lemmas
  attaching at two forms each, whose own seven forms are then attachment
  sites, is where the depth-2 term more than doubles. Test-walk only; bench
  says the runtime cost is nothing.
- **`the_same_headword_at_two_different_lengths_is_not_deduplicated_away`
  moved off `書いた` to `食べた`.** `書い` is `連用タ接続` and is no longer a
  candidate, so the pair the test protects would have been a single
  candidate and the assertion vacuous. `食べ` is `連用形` — a form that does
  end words — so it is still two.

## 3a. The second screenshot

Same pair, `「(ほんと、見た目はギャルなのになー。こうして話してると、中身は完全にオタクなんだな)」`.
Three differences, and `lookup` settles which are ours:

| | JMdict | verdict |
| --- | --- | --- |
| `完全に` | no headword (`[完]`, `[完全]` only) | TA merges what we cannot; correct as split |
| `話してる` | no headword (`[話]`, `[話し]` only) | same |
| `なのに` | **`[primary,particle]`, 3 chars** | ours — the right cover ranked *second* |

`なー` romanizing as `naa` against TA's `nā` is house style, pinned both
sides (`ラーメン` → `raamen`), not a defect.

`なのに` is the `ある日` shape exactly: `[primary]` with no `common`, so it
costs +5 against `な`(+1) + `のに`(0) and loses by 4. `FUNCTION_STACK` is
charged to whichever side takes the span and cancels out. The only cost term
that reaches it is `MATCH`, which would have to be 6, and 6 is refused on the
compound class lines. The user lexicon is where the spike document already
put this class, and the cap's own comment asks for the gates rather than a
raised number — they were re-run and every cell is identical to the digit in
both hint states, so 83 became 84.

**What it costs:** a forced token also wins where `なの` + `に` is right —
`「原因なのに気づいた」` now reads `なのに`. The engine did not reach `なの`|`に`
before either (it read `な`|`の`|`に`), and the gold corpus has neither shape.

## 3b. The third screenshot

`「お、忍くん食いついた。へへ、意外とキャラの好み似てるんだねあたし達！」`. Two more of the
same shape, and one already-known non-defect:

| | JMdict | cost |
| --- | --- | --- |
| `へへ` | `[primary]` "heh-heh" | +4, +4 more from FUNCTION_STACK, against `へ`(+1) + `へ`(+1) |
| `あたし達` | `[primary]` "we; us" | +6 against `あたし`(−1) + `達`(+1) |
| `忍` → `shinobi` | — | JMnedict, not shipped; §4 |

`へへ` split was also a **romaji** defect, not only a segmentation one: a
lone `へ` takes the particle entry, and `furiganaFor` reads `e` off that
flag, so the laugh rendered `e e`. Merged, it is `hehe` from the laugh's own
entry — one fix for both.

`FUNCTION_STACK` charging `へへ` is worth naming: the laugh is tileable by a
COMMON particle, which is the term's trigger, and it is the one place that
term prices a word it should not. It did not have to be touched here, because
the forced token settles the span before cost sees it.

Neither entry was reachable by `shatter_sweep.rs` as it stands — `あたし達`
has a kanji and the sweep is kana-only; `へへ` it did see, below the cutoff.
Gates re-run at 86 entries, every cell identical to the digit in both hint
states, and `東京へ行く`/`学校へ` confirm the directional particle is untouched.

**The `[noun]達` class is open.** `私達` is its own JMdict headword, `君達`
and `侺達` are others. One row each does not scale; if more turn up the class
wants a suffix rule.

## 3c. The fourth screenshot, and the thing all four have in common

`「もう、ネタバレは本当に駄目だから、邪知暴虐の極みっ！唐揚げに無断でレモンを掛けるがごとき暴挙！」`

`唐揚げ` is `[primary]`, +5, against `唐`(+1) + `揚げ`(0), both COMMON. The
reading is what settles that it is lexical and not compositional: `からあげ`,
where the split rendered `とう` + `あげ`. Kanji-bearing, so the sweep never
saw it; `唐揚` without the `げ` already merges unaided.

`無断で` is in the same sentence and is the same `[primary]`-without-`common`
shape, and is deliberately **not** added. It is compositional, so the
compound adjudication's `transparent` class says split it and the engine
already does. TA merges it; that is not this project's standard, and the
distinction is the whole reason §5.2 reports the three classes separately.

**Four screenshots, four instances of one class.** `なのに`, `へへ`, `あたし達`,
`唐揚げ` — every one a `[primary]` headword with no `common` tag, losing to
its own COMMON parts by the arithmetic in §2 of the previous handoff. A row
each is a patch. The list went 83 → 87 in one session and there is no reason
to think the next screenshot is different.

**The structural candidate was measured, and it is worse than its own
record.** `cost.rs` names `MATCH = 6` as what `ある日` needs unaided and
refuses it on `transparent` 42 → 30. Re-measured on today's engine, hints
on, with `BOUND_FORMS` and the 87-entry lexicon in place:

| M | strict | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- |
| **2** | **1826** | **118** | **46 / 112** | **30 / 32** | **42 / 42** |
| 3 | 1823 | 118 | 46 / 112 | 30 / 32 | 42 / 42 |
| 4 | 1749 | 106 | 54 / 112 | 28 / 32 | 24 / 42 |
| 6 | 1698 | 124 | 76 / 112 | 27 / 32 | **21 / 42** |

`物思いにふける` needs M >= 4, which already costs 77 strict spans and half
the `transparent` class.

**The rest of the route was then measured rather than argued.** `MATCH`
alone over-merges `には`-shaped spans, and `FUNCTION_STACK` is precisely the
flag that names them, so it was swept upward to compensate; `PARTICLE_CROSS`
was written back into `cost.rs` (it mirrors `PRONOUNCE_CROSS`, gated on
`WordFlags::PARTICLE`) for the one shape neither catches. Hints on:

| config | strict | tolerant | entry top-1 | compounds | lexical | false-match | transparent | merge gate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **M=2 FS=4 (shipped)** | **1826** | 1912 | 0.9341 | 118 | 46 | **30 / 32** | **42 / 42** | **26 / 26** |
| M=6 FS=4 | 1698 | 1928 | 0.9387 | 124 | 76 | 27 | 21 | — |
| M=6 FS=8 | 1730 | 1928 | 0.9402 | 136 | 76 | 27 | 33 | fails |
| M=6 FS=8 PC=12 | 1733 | 1927 | 0.9411 | **137** | **76** | 27 | 34 | **25 / 26** |

Both new terms are flat in magnitude the way the existing ones are — FS at 8,
10 and 14 give identical cells, PC at 6, 12 and 25 likewise — so only their
signs were ever in question. `FUNCTION_STACK = 8` does its job: `私には分かる`
still splits `に` and `は`. `PARTICLE_CROSS` does its job: `それを聞いて` goes
back to `それ`|`を`|`聞いて`. All five reported cases merge, and the last four
lexicon rows would become unnecessary.

**And it is still refused, on success criterion 3.**
`the_particle_absorbing_merges_are_gone` goes 26 → 25: `ことになっている`
swallows the `に` in `b:wn28412-1`. JMdict carries that phrase as a headword,
so the merge is defensible in the abstract — and the gate exists precisely
because "JMdict carries it" is not the standard this engine is held to. On
top of that: −93 strict spans, `transparent` 42 → 34, `false-match` 30 → 27.

**Then: can the strict loss be avoided?** The −93 is not damage — coarsening
rises 86 spans in 43 merges → 194 in 87, against 93 strict lost, and tolerant
*rises* 15. Strict is scored against a short-unit gold corpus, so every
compound kept whole costs its own gold spans; §5.2 prices that at about 16.5
points. The loss is the merging, one for one.

So the question became whether a sharper *suppressor* could merge the
reported class while merging less of everything else. `FUNCTION_STACK`'s
trigger is "tileable by pieces that are COMMON **and** PARTICLE", and two
looser readings were tried, both at M=6 FS=8 PC=12:

| suppressor tiling | strict | coarsening | compounds | transparent | `物思いにふける` | gates |
| --- | --- | --- | --- | --- | --- | --- |
| baseline M=2 FS=4 | **1826** | 86 / 43 | 118 | **42 / 42** | splits | **all green** |
| COMMON + PARTICLE (today's) | 1733 | 194 / 87 | **137** | 34 / 42 | merges | particle gate 25 / 26 |
| COMMON only | 1776 | 135 / 60 | 125 | 40 / 42 | merges | gate fails, **plus two §4.3 shattering regressions** |
| all-COMMON, ≥ 1 PARTICLE | 1787 | 141 / 63 | 128 | 40 / 42 | **splits again** | gate fails, plus two others |

Each sharpening does cut the strict loss — −93, −50, −39 — and each one also
starts protecting the word we are trying to merge, because the features that
decide whether `ことになっている` is suppressible are the same features that decide
it for `物思いにふける`. The COMMON-only reading re-opens §4.3's shattering
class outright (`聴聞会` → `聴聞` + `会`), which is the defect the whole v2
rewrite exists to remove. The third reading is sharp enough to keep §4.3
intact and then no longer fixes the case.

**The route is closed with numbers, across four `MATCH` values, four
`FUNCTION_STACK` values, three `PARTICLE_CROSS` values and three tiling
definitions.** No configuration in this family both merges the reported class
and keeps the gates. `cost.rs` and `lex.rs` are back at `MATCH = 2`,
`FUNCTION_STACK = 4`, no `PARTICLE_CROSS`, COMMON+PARTICLE tiling, and the
baseline cells reproduce exactly: strict 1826, compounds 118, 46 / 30 / 42,
coarsening 86 in 43, 0 test failures.

**Why the user lexicon costs nothing where this costs 39 to 93 spans.** Not
because it is better — because it is *targeted*. The gold corpus contains
none of `なのに`, `へへ`, `あたし達`, `唐揚げ`, so forcing those four tokens
moves no cell. Any change to the cost model moves every sentence.

**Bench note, so the numbers in this document are read correctly.** The
machine drifted over the session: the same binary read 522 ms at 10,040
chars early and ~550 ms late, and one reading came back 1.29x on pure noise.
Only interleaved A/B is meaningful. Interleaved, three rounds each, 83
entries against 87: 577 / 541 / 549 ms against 586 / 572 / 541 — the spreads
overlap, so four lexicon rows cost nothing measurable. Every absolute
millisecond figure above is a reading from the moment it was taken.

## 3d. The fifth screenshot, and where the lexicon stops

`やることのなくなった俺は、再度物思いにふける。` — TA keeps `物思いにふける`
whole, a JMdict headword at 7 characters, `[primary]` with no `common`. Same
arithmetic as the other four: +9 whole against `物思い`(+5) + `に`(+1) +
`ふける`(−1) = +5, so it loses by 4, and the engine ranks it **second**.

**No row was added, and the reason is not the arithmetic.**

1. **A forced token is form-specific.** The lexicon pins one surface. The
   engine already reaches `物思いにふけった` and `物思いにふけって` as whole
   candidates at rank 2 through its own deinflection — a row for the citation
   form would fix one form of three and make the other two inconsistent
   with it.
2. **It absorbs a particle.** Success criterion 3 and the 26-row
   `spurious_merges.txt` gate exist to stop the engine swallowing `に` into
   the word before it. Forcing this merge is that shape, deliberately.
3. **Nothing is mis-read.** `唐揚げ` earned its row because the split
   rendered `とう` + `あげ` for `からあげ`. Here the reading is the sum of
   its parts, and the user sees `物思い`, `に` and `ふける` each glossed
   correctly. The only thing lost is the *idiom's own gloss*.

Which points at a display fix rather than a segmentation one: a span that is
a prefix of a longer dictionary entry the parse declined could say so in the
popover, without moving a boundary. Not built, not scoped here — named
because it is the shape of the answer for every idiom in this class, and
there are many.

## 4. Not done

- **Gate 6, the eighteen reported sentences, was not run.** The sentence
  set is not in the repository: `docs/` quotes two of the eighteen and
  records 486 / 630, but nothing holds the list. Whoever has it should
  re-run the cell, or commit the list so the gate stops depending on one
  session's context.
- **`忍` still reads `しのび`, not `しのぶ`.** Third difference in the
  screenshot pair, and not a defect this branch can fix: JMdict_e has no
  name readings, that is JMnedict, which this project does not ship. The
  previous handoff's §4 already lists it.
- **A term for this class that is not `MATCH`.** §3c's table closes the
  `MATCH` route. What the class actually needs is something that can tell a
  `[primary]` headword the tokenizer shattered from a COMMON function-word
  tiling, and `MATCH` cannot see the difference — it rewards *every* longer
  span. Unspecified; it is the open problem, not a queued task.
- **The other bound forms.** `未然形`, `ガル接続` (the `たい` row's comment has
  wanted this since Phase 1a) and the rest of the `接続` family are the same
  one-line change. Each deletes candidates of its own and wants its own
  score run — one at a time, cheapest defect first.
- **The engine's `いっち` → `icchi`.** See §1.
