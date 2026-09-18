# The LEN_GAIN refusal, resampled — handoff

**Date:** 2026-09-18
**Branch:** `feat/corpus-disagree`
**Predecessor:** [`2026-09-18-three-corpus-adjudication-handoff.md`](2026-09-18-three-corpus-adjudication-handoff.md)

The previous session refused ungating `LEN_GAIN` hints-off on a corpus-scale
number: 9,849 coarsening against 9,737 invented over 18,000 sentences, a 1:1
ratio against the gold corpus's 3:1. Every one of those 18,000 sentences came
from `head -6000` of three corpora — the **opening chapters** of three visual
novels. This session drew a different sample to find out whether the ratio was
an artefact of where it was taken.

**It was not. The sample was fine. The measurement was wrong.**

## 1. What this session did

Four corpora, three disjoint slices each, two refused variants against today's
`HEAD`, hints off throughout. Plus a reproduction control, which is what made
the rest of it readable.

| | | |
| --- | --- | --- |
| Phase 0 | why `mecab_hints` defaults off | `ac2bd53` |
| Phase 1 | resample: 4 corpora × (head-4000, tail-4000, random-4000) + 3 control pools | — |
| Phase 2 | control, then the 2 × 3 × 4 table, then sixty sentences read by eye | — |
| Phase 4 | verdict | this document |

## 2. Phase 0 — `mecab_hints` defaults off, and a bare flip makes it worse

A fresh install runs hints off, which is the worse engine on every cell:
strict 1593 against 1698, tolerant 1786 against 1948, compounds 118 against
152. The default looked like an oversight. It is not a quality preference at
all — **it records whether the 7.7 MB Vibrato dictionary has been acquired.**

`state::load_hints` never downloads; it only opens an already-published
`hints/current/system.dic`. `commands::hints::set_mecab_hints` is the sole
fetcher. So `mecab_hints: true` in `Settings::default()` makes a fresh install
boot with `hints_off_warning("no dictionary in …")` and hints still off —
strictly worse than today. Turning hints on for a first run needs an
*acquisition* on the first-run path; the download screen in `src/main.ts`
already owns the one gesture that could carry it. Recorded at the declaration
in `settings.rs`.

## 3. Phase 1 — the pools

Dump recipe unchanged: `artificialtrans`, column `source`, distinct non-empty
rows in rowid order with newlines stripped. It reproduces all four surveyed
counts and averages exactly, which is how it is validated.

| corpus | distinct | avg chars | pools |
| --- | --- | --- | --- |
| renai | 8,334 | 28.2 | head/tail/random-4000 + head-6000 control |
| kf2 | 31,734 | 23.9 | head/tail/random-4000 + head-6000 control |
| riruka | 35,699 | 32.3 | head/tail/random-4000 + head-6000 control |
| **ikenaiDX** | 9,559 | **50.4** | head/tail/random-4000 (no control — nothing to reproduce) |

`random-4000` seed **20260918**, `random.Random(seed).sample`. N = 4,000 so
that head and tail are disjoint in renai (8,334 rows). The two SEALED
databases were not opened.

## 4. The control reproduces to the digit, and that is what made the rest legible

At `7722c2c`, the variants' own baseline, head-6000 over the three MEASURE
corpora:

| | sentences | changed | merge | cut |
| --- | --- | --- | --- | --- |
| measured here | 18,000 | **9,370** | **9,849** | **9,737** |
| handoff §8 recorded | 18,000 | 9,370 | 9,849 | 9,737 |

Identical. The same control at today's `HEAD` reads 9,327 / 9,808 / 9,639 —
the small shift is entirely `e2ed7a3`, the second `ていく` attachment site,
which merges those forms at `HEAD` so `LEN_GAIN` no longer has to.

## 5. The ratio is stable across slices — "different chapter" is dead

Ungated `LEN_GAIN`, merge-vs-cut, per 4,000 sentences:

| corpus | head | tail | random | spread |
| --- | --- | --- | --- | --- |
| renai | 0.82 | 0.85 | 0.84 | 0.03 |
| kf2 | 1.65 | 1.62 | 1.70 | 0.08 |
| riruka | 0.94 | 1.03 | 1.01 | 0.09 |
| ikenaiDX | 1.68 | 1.82 | 1.80 | 0.14 |

Within every corpus the three slices agree. The head-6000 sample was
representative and the sampling question is closed. What the table also shows
is that the ratio is **corpus-dependent by a factor of two**, so the
three-corpus pooled 1:1 was an average over corpora that disagree.

## 6. But merge-vs-cut is not measuring quality

Sixty changed sentences were read side by side, fifteen from each corpus, on
the new slices. Gains outnumbered damage roughly four to one — the opposite of
what the counts said. The counts are wrong in a specific, mechanical way.

`merge` vs `cut` classifies a new span by whether both its endpoints were
boundaries in the other build's parse. That is agreement with the previous
engine, not truth, and it fails in **both** directions:

- A correct re-tiling files as damage. `中` | `出` | `しする` → `中出し` |
  `する` ends inside `しする`, so it counts as an invented boundary. So do
  `絵` | `に` | `描いた` | `よ` | `うな` → `絵に描いたよう` | `な`, and
  `あり` | `ま` | `せん` | `で` | `した` → `ありませんでした`.
- A bad merge of an already-mis-cut region files as coarsening.
  `なんでも` | `な` | `いよ` → `なんでもな` | `いよ` has both endpoints on base
  boundaries, so it scores as a merge.
- **One severing is counted twice.** `抱え` | `ながら` → `抱えな` | `がら`
  produces two new spans, both with an off-base endpoint, and zero merges.

Decomposing the "invented" bucket over all twelve slices, 27,475 spans:

| what it actually is | count | share |
| --- | --- | --- |
| a dictionary word re-tiling across a base cut | 11,730 | 42.7% |
| a leftover piece inside one base span (the other half of a severing) | 15,738 | 57.3% |
| **a non-word merged across a base boundary** | **7** | **0.0%** |

The proxy's damage bucket contains **seven** genuine invented compounds in
16,000 sentences. Everything else in it is a recovered word or a double count.

## 7. Re-measured against the dictionary rather than against the old engine

Two questions that do distinguish them, both using each build's own `matched`
flag:

- **severed** — a span `HEAD` matched as a dictionary word that the variant cut
  through. Destroyed morphology.
- **gained** — a span the variant matches as a dictionary word, absent from
  `HEAD`, covering at least one `HEAD` boundary. A word recovered.
- **clean gain** — a `gained` span that severs nothing itself. The conservative
  count, because a span like `浮かべな` (199 occurrences) is matched, and is the
  `ながら` severing wearing a gain's clothes.

Ungated `LEN_GAIN`, all twelve slices, hints off:

| corpus | avg chars | clean gains | entangled | severed | **clean : severed** |
| --- | --- | --- | --- | --- | --- |
| renai | 28.2 | 6,282 | 3,155 | 3,803 | **1.65** |
| kf2 | 23.9 | 6,738 | 1,601 | 2,047 | **3.29** |
| riruka | 32.3 | 6,691 | 2,703 | 3,343 | **2.00** |
| **ikenaiDX** | **50.4** | 16,292 | 4,219 | 4,633 | **3.52** |
| all four | — | 36,003 | 11,678 | 13,826 | **2.60** |

Per-slice spread within a corpus is at most 0.27. Every corpus is above 1.0 on
the most conservative count available. The headline figure including entangled
gains is 3.45:1.

**2.60 is a floor, not an estimate.** `entangled` catches any gain that cuts a
base-*matched* span, and a base match is not always a base word: `中` | `出` |
`しする` → `中出し` | `する` files as entangled because `HEAD` matched `しする`,
which is not a word. Every such repair is counted against the change. The true
figure is somewhere between 2.60 and 3.45 and nothing here pins it closer.

### 7.1 The kana third of the gains, adjudicated

69% of the gained spans contain kanji or katakana and are plainly lexical
(`中出し`, `亀頭`, `子宮口`, `性感`, `愛液`, `魔族`, `種付け`, `先走り汁`,
`お尻`, `あの子`, `おまえさん`). Only 2.7% of those — 831 of 30,568 — take the
`十` \| `時に` shape of a kanji stem plus a single kana particle, led by `所で`
94, `時に` 61, `次に` 22.

The kana third was adjudicated separately, against the `からな` class as the
thing to test for: 90 surfaces by `lookup` flags and real corpus sentences,
plus a 901-sentence resample to reach the half the top-90 does not cover.
Weighted over all 9,735 kana-only occurrences:

| class | share | example |
| --- | --- | --- |
| **false-match, the `からな` shape** | **~34%** | `いんだ` claimed as `いぬ` 往ぬ — **0 occurrences of 往ぬ/去ぬ in 85,326 corpus lines**, verified here |
| boundary overrun | ~5% | `させ` \| `ながら` → `させな` \| `がら` |
| **entry-less vocalization** | ~7% | `うう` 423 claimed as `植う` "to plant"; `ずず` 96 as 数珠; `くす` 64 as 楠 |
| real or coarsening | ~54% | `おちんちん` 465, `おまえさん` 198, `このまま` 143, `ありません` 46 |

Zero contamination rows: every single-corpus surface checked is a register or
SFX habit, not a character name.

**Two findings from that pass are worth more than the percentages.**

**The lemma-absence grep under-detects the class by about a third.** It
convicts `いんだ` (往ぬ: 0 hits), `りする` (利する: 5, kanji only) and `のせ`
(のせる in kana: 0) outright, but 祈る, 異なる, 絡む, 歩く, 泣く, 行く, 逸れる
and 唸る all *do* occur, so it acquits `いの`, `ことな`, `からも`, `あるか`,
`なこ`, `いって`, `それな` and `うなら` falsely. Those eight had to be convicted
by parsing the sentence. The test that convicted the `からな` family is
necessary and not sufficient.

**The bad kana gains are the left half of a residue-1 severing, not a separate
class.** Verified directly on the two pinned binaries:

| | `HEAD` | ungated |
| --- | --- | --- |
| それならいい | `それなら` \| `いい` | `それな` \| `らいい` |
| 感じやすいんだ | `感じ` \| `やすい` \| `んだ` | `感じやす` \| `いんだ` |
| 殺すんじゃないの | `殺す` \| `ん` \| `じゃない` \| `の` | `殺す` \| `ん` \| `じゃな` \| `いの` |

Every one takes one kana too many and books the stub as a match. 28 of the 60
most-severed words — 2,593 of 9,119 severed occurrences in that sample — have a
top-90 kana gain as their own residue, so **the gain and the severing are the
same cut counted twice, once on each side of the ledger.** `entangled` above
catches most of this and is why 2.60 is the conservative figure, but not all of
it: a residue that happens to span a base boundary lands in the clean bucket.

Striking the false-match and boundary-overrun share takes the optimistic ratio
from 3.45:1 to **3.03:1** and the conservative one from 2.60:1 to about
**2.3:1**. The sign does not change. What changes is the reading of the fix:
because the bogus gains *are* the severings, a residue term removes both at
once rather than trading one for the other.

**The gold corpus agrees, and the handoff's claim that it does not is wrong.**
Decomposed on today's `HEAD`, the ungated variant loses 125 gold spans and
gains 53; of the losses, **96 are coarsening** (swallowed by a merge with both
ends on gold boundaries) against **29 damage**. That is 3.3:1 — the same ratio
the corpus gives. The finding recorded last session, that "the gold corpus's
3:1 coarsening-to-damage ratio does not survive 18,000 real sentences", does
not hold. It survives; the proxy did not.

## 8. The contrast arm answered its own question

`ikenaiDX` was added for length, not volume: 50.4 characters a line against the
others' 24–32. `LEN_GAIN` is the one superlinear term in the cost model,
charged as `(len−1)²`, so the prediction was that its effect grows with span
length.

It does. `ikenaiDX` reads **3.52** clean-gains-to-severed against renai's 1.65,
and it is the only corpus where the merge rate per 10,000 characters (250–285)
runs far above every other corpus's. The register this engine primarily
targets, and its long-narration half especially, is where the change is worth
most.

The reverse reading also holds: the biggest single damage class, `ながら`, is
949 severings across the new slices and **760 of them are `ikenaiDX`'s**. The
long-sentence arm carries both sides of the trade, not just the good half.

## 9. The damage is one shape, and it is the shape nothing expresses

Of 13,826 severings across all twelve slices:

| residue left of the severed word | count | share | cumulative |
| --- | --- | --- | --- |
| **1 character** | 12,644 | **91.5%** | 91.5% |
| 2 characters | 1,130 | 8.2% | 99.6% |
| 3 characters | 44 | 0.3% | 99.9% |
| 4 characters | 8 | 0.1% | 100.0% |

`≤2` holds at 99.4%, 99.8%, 99.5% and 99.8% in the four corpora
**independently**. The class is 89.6% kana and named by its own frequency
list: `ながら` 949, `じゃない` 347, `んだ` 265, `かもしれない` 236, `こんなに`
182, `じゃないか` 148, `やる` 139, `たり` 126.

**Every severing is a merge that takes all but one kana of a word the engine
had already matched.** That is exactly what `relax::MIN_PIECE` and
`KATAKANA_MIN_PIECE` already encode on the split side — "a piece shorter than
three is not evidence" — stated for the merge side, which the previous
handoff's CLOSED list named as the one shape nobody has expressed.

**It is now priced.** A rule rejecting a merge that leaves a `≤2`-character
residue of an otherwise-matched word would keep **36,003 of 47,681 gains
(75.5%)** while reaching **99.6% of the damage**. Read that as an upper bound
from the dumps, not a measurement of a built term: the Viterbi path is global,
so blocking a candidate does not leave the clean gains untouched. `cost()` is
per candidate and cannot see the tiling, so this is a `lattice` change.

## 10. What bare ungating costs on the gates

Run on a worktree at `HEAD` with the one-line predicate change:

Run to completion with `--no-fail-fast` on a worktree at `HEAD` with the
one-line predicate change, against `HEAD`'s 658 passed / 0 failed / 1 ignored.
**The variant reads 653 passed / 5 failed / 1 ignored, and all five are below.**

| gate | result |
| --- | --- |
| 26-row spurious-merge gate | **26 / 26**, unmoved |
| `conjugation_labels` | 3 passed |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets` | 0 findings |
| `bench`, interleaved A/B, three rounds each way | **unmoved**; hints off 598 / 608 / 597 ms before against 588 / 580 / 589 after, hints on 519 / 534 / 539 against 536 / 519 / 537, scaling 0.98–1.03x |
| `npx tsc --noEmit`, `npx vitest run` | clean, 375/375 — unaffected, the change is one Rust predicate |
| `engine_v2`, all other tests | 45 passed |
| `cost.rs` unit tests | **2 fail** — both assert hints-off cost length-invariance, the property the change deliberately removes |
| `a_conjugated_katakana_verb_loses_to_its_shorter_parts` | **fails because the variant fixes it**: `ミス` \| `った` → `ミスった` |
| `the_dictionary_form_itself_loses_to_its_parts_without_hints` | **fails because the variant fixes it**: `ミス` \| `る` → `ミスる` |
| `parse_sentences.txt` snapshot | **3 of 30 sentences move** |

The three snapshot moves, adjudicated:

| sentence | old | new | |
| --- | --- | --- | --- |
| 日本語を話せますか。 | `話せ` \| `ます` | `話せます` | **fix** — 〜ます already merges everywhere else |
| 出て来るのを待っていた。 | `出て` \| `来る` | `出て来る` | **fix** — 〜てくる is on the handoff's own open list |
| 会議は十時に始まる。 | `十` \| `時` \| `に` | `十` \| `時に` | **regression** — hides the counter and the particle |

That last class is small: only **2.7%** of kanji-containing gains end in a
single kana particle (831 of 30,568), led by `所で` 94, `時に` 61, `次に` 22.

Gold cells for the variant, hints off, against `HEAD`'s 1593 / 1786 / 118:

| | strict | tolerant | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- |
| `HEAD` | 1593 | 1786 | 118 | 56 / 112 | 23 / 32 | 39 / 42 |
| ungated | **1521** | **1815** | **158** | **94** | **25** | 39 |
| uninflected only | 1566 | 1816 | 152 | 88 | 25 | 39 |

Hints on is identical to the digit in both variants, by construction. **No
split class regresses**: `false-match` improves 23 → 25 and `transparent` is
unmoved. Both halves of the compound metric move the right way at once —
**kept whole 52 → 90 of 106, split apart 66 → 68 of 80** — so it is not a
merge-versus-split trade on that metric at all.

`entry top-1` hints-off reads 0.9506 → 0.9578, and that rise is a **selection
effect, not an improvement**: the denominator falls 1235 → 1162, and of the 73
gold spans that stop being scored, 61 were previously correct — 83.6% against
the population's 95.1%. Dropping a below-average subgroup raises the average.

**The narrower variant is dominated on the corpus too.** `uninflected only`
severs more and gains less than ungating in three corpora of four, and its
clean ratio is below ungating's everywhere. Both refused predicates stay
refused; ungating is the better of the two.

## 11. Verdict

**The refusal reopens** — but not for the reason the session was set up to
test.

The sample was never the problem. The ratio is stable across head, tail and
random inside every corpus, and the control reproduces the old number to the
digit. What reopens the question is that **merge-vs-cut was measuring agreement
with the previous engine, and its damage bucket held seven genuine invented
compounds out of 27,475 spans.** Re-measured against the dictionary, the same
twelve slices read 2.60:1 in favour on the most conservative count and 3.45:1
on the headline — and the gold corpus, decomposed, says 3.3:1, the ratio the
last handoff reported as not surviving. Adjudicating the kana third takes those
to **2.3:1 and 3.03:1**. Every reading of the evidence is between two and
three and a half to one in favour; none is 1:1.

Bare ungating is therefore net-positive on the register this engine primarily
targets, and most positive on its long-narration half. What it costs is
13,826 severed words across 16,000 sentences — roughly **0.86 broken words per
sentence** — plus 72 strict on a gold corpus with zero VN dialogue in it, two
fixture rows that state a property the change removes, and one snapshot
regression. `それならいい` reading `それな` \| `らいい` is what that looks like
on screen.

**The residue term was the obvious next move and it is refused** — see §11a,
which was written after building it three ways. It is expressible and it costs
roughly what §9 predicted, but it cannot be *aimed*: the words it must protect
(`ながら`, `それなら`, `じゃない`) and the stacks it must not (`ために`,
`くせに`, `ように`) carry identical JMdict flags, so no predicate over the
dictionary separates them. The discriminator has to come from outside it.

That leaves the decision where §10 puts it. Bare ungating is net-positive on
the target register and ships a visible defect with no available fix; the
alternative is to leave `LEN_GAIN` gated and spend the next phase on the
signal — the tokenizer's POS, or the hand adjudication `gold_local.jsonl` was
reserved for. **Nothing in this branch changes the engine**; every cell above
is `HEAD`'s, re-measured.

## 10a. Held out: three corpora the change was never measured against

`random-2000` from three corpora outside the measured set and outside the
SEALED pair, which stays shut. Every prediction replicates:

| corpus | avg chars | clean : severed | optimistic | `fragment` | residue ≤ 2 |
| --- | --- | --- | --- | --- | --- |
| amakanoPE | 24.3 | 1.60 | 2.32 | **0** | 99.5% |
| enkou | 28.1 | 1.82 | 2.61 | **0** | 99.3% |
| **hitoduma2** | **45.1** | **4.54** | **5.44** | **0** | 99.4% |
| pooled | — | **2.79** | 3.60 | **0** | — |

Three things this settles that the measured set could not.

**The length finding is out-of-sample.** `hitoduma2` at 45.1 characters reads
4.54, the highest figure measured anywhere, exactly as `ikenaiDX` at 50.4 read
highest of the original four. Two independent long-sentence corpora by
different writers, same direction, and neither was used to form the claim. The
short corpora land where the short corpora landed: 1.60 and 1.82 against
renai's 1.65 at 28.2 characters.

**`fragment` stays at zero.** Not one merge across a boundary that matches
nothing, in 6,000 further sentences. The proxy that refused this change counted
27,475 "invented" spans; on unseen text the genuine count is still 0.

**The residue shape is not an artefact of the corpora it was found in.** 84–93%
of severings lose exactly one character and 99.3–99.5% lose two or fewer, in
three corpora that had no part in discovering it.

The SEALED pair (`Start_`, `トリプルペアリング_Crack_`) was **not** opened. It
is the one clean post-ship confirmation this project can get and a pre-ship
test would have spent it.

## 11a. The residue term: built, measured, refused — and the reason is sharp

§9 priced it and §11 called it the largest reachable fix. It was then built, in
three formulations, and **it is refused.** Not on cost: on the discovery that
the signal it needs does not exist in the dictionary.

**Formulation 1 — a per-position mask.** Mark every position interior to a
`COMMON` candidate that would leave a `≤ RESIDUE_STUB` stub, and charge a step
that ends there. It works on the class it was built for and it also suppresses
the criterion-3 splits the engine exists to make: `transparent` **40/42 →
25/42** hints on, **39/42 → 25/42** hints off. `ように` is a `COMMON` candidate,
so cutting it to `よう` \| `に` was charged exactly like cutting `ながら`.

**Formulation 2 — the same mask, excluding `grammatical_stack` candidates.**
`is_grammatical_stack` already means "function-word stacking whose parts are
each what the reader needs", so excluding it should free `ように`. It does, and
it reads better than anything else tried:

| hints off | strict | tolerant | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- |
| `HEAD` | 1593 | 1786 | 118 | 56 | 23 | 39 |
| residue alone | 1581 | 1786 | 121 | 61 | 23 | 37 |
| residue + ungated | 1519 | 1817 | 155 | 93 | 23 | 39 |

and on the corpus it beats bare ungating on both sides at once — severings down
12.5%, clean gains up 9.6%, so `clean : severed` over the four `random-4000`
slices goes **2.62 → 3.29**:

| | renai | kf2 | riruka | ikenaiDX |
| --- | --- | --- | --- | --- |
| ungated alone | 1.67 | 3.39 | 2.02 | 3.57 |
| residue + ungated | **2.24** | **4.41** | **2.74** | **4.02** |

**But it fixes none of the named defects.** `抱えながら`, `それならいい`,
`感じやすいんだ` and `殺すんじゃないの` all still break, because `ながら`,
`それなら` and `じゃない` are themselves grammatical stacks and the exclusion
that frees `ように` frees them too. It costs hints-on 2 strict and 3 compounds
to buy a corpus gain, and leaves the visible defect untouched.

**Formulation 3 — charge the span that straddles, not the cut.** The diagnosis
was right: `よう` \| `に` leaves `に` standing as its own span while `抱えな` \|
`がら` leaves `な` *absorbed*, and only a span crossing the word's own end can
absorb. Implemented, it is degenerate. A one-character span can never straddle
anything, so shattering is a free escape from the penalty and the engine takes
it: `感じやすいんだ` → `感じ` \| `や` \| `す` \| `い` \| `ん` \| `だ`, and
`歩いていくように` — correct in **both** other builds — → `歩いていく` \| `よ` \|
`う` \| `に`. A term that punishes crossing a boundary rewards never spanning
one.

**Why no fourth formulation was tried.** The two classes are flag-identical in
JMdict, and that is checkable in one command:

| must **not** split | flags |
| --- | --- |
| `ながら`, `それなら` | `[reading, common, common-line, particle]` |
| `じゃない`, `かもしれない`, `やすい` | `[reading, common, common-line]` |

| must split (criterion 3) | flags |
| --- | --- |
| `ために`, `くせに`, `ところで` | `[reading, common, common-line, particle]` |
| `ように`, `たびに` | `[reading, common, common-line]` |

Every combination on the first list appears on the second. `COMMON`,
`PRIMARY`, `PARTICLE` and `is_grammatical_stack` each cut straight through the
middle of both sets. **No predicate over the dictionary's own flags separates
"a word that must not come apart" from "a stack that must".** This is the same
wall §6 of the predecessor hit from the other direction, where adjudicating 68
kana rows split the review — and it is the same wall in the same place.

So the CLOSED list gets a new entry, and it is stronger than the one it
replaces. It is no longer "a residue term is the one shape nobody has
expressed". It is: **the residue term is expressible, costs what §9 predicted,
and cannot be aimed, because the discriminator it needs is not in the
dictionary.** Whatever supplies it has to come from outside — the tokenizer's
POS, or hand adjudication of the kind `compound_gold.jsonl` already is, which
is what `gold_local.jsonl` was reserved for.

Formulation 2 is the one worth reviving if anything is. It is a net gain on the
corpus and a small loss on gold, it is twenty lines in `lattice.rs`, and it is
recorded here rather than committed because it buys a ratio and not a defect.

## 12. Also measured, and not previously on record

**`〜てくる` is 2.5× `〜ていく`**, the auxiliary the previous session spent two
commits on. Lines carrying one, per thousand, over the full corpora:

| | renai | kf2 | riruka | ikenaiDX | all |
| --- | --- | --- | --- | --- | --- |
| 〜てくる | 56.3 | 24.8 | 68.4 | **106.3** | **55.2** |
| 〜ていく (fixed) | 8.0 | 13.8 | 15.1 | 91.6 | 22.5 |
| 〜ておく | 7.8 | 2.5 | 5.1 | 6.0 | 4.6 |
| 〜てみる | 8.5 | 5.4 | 7.9 | 6.5 | 6.9 |

The method reproduces the handoff's own `ていく` rates (9.6 / 15.5 / 17.3 per
thousand) within 15%, so the two are comparable. This is a surface-regex count
and therefore an upper bound — `持ってくる` is a JMdict headword and already
merges — but the ordering is not in doubt. `〜てくる` is the largest `〜て`
auxiliary still split.

**It is not a copy of the `ていく` rows.** Its class is `カ変・クル`, whose
surfaces are `くる`, `こ`, `き`, `くれ`, `こい`, `こよ`, `くん`, `く` — so
`ATTACHES_TO`'s own transferable test, "does any of this auxiliary's surfaces
decompose as [standalone word] + [particle]?", has two answers `ていく` did not
have to give:

- **`てくれ`** is `てくる`'s 仮定形 *and* the imperative of the benefactive
  `てくれる`. `買ってくれ` is "buy it for me", not "go and buy". The two
  meanings share a surface and only one of them is this row's.
- **`てく`** is 体言接続特殊２ *and* the spoken contraction of `ていく`
  (`歩いてく`), which the `ていく` rows already own.

Add to that the `でくる` mirror for an onbin stem (`遊んでくる`), both
attachment sites (`ATTACHES_TO`'s rule: IPADIC names 連用タ接続 only where it
differs from 連用形, so either row alone leaves half the classes unreachable),
and a **second label word**, because `label::fmt`'s "each word appears at most
once" would silently drop one from a chain carrying both directions and
`ていく` already took "Directional". This is a phase, not a row.

## 13. How to rebuild this run

**The tooling is committed this time**, because the scratchpad does not survive
a session and re-deriving it was most of the setup cost:

```
python tools/cut_corpus_pools.py <out> <db> [<db> ...]     # four pools per corpus, seed 20260918
<bin>/jparser-cli.exe build-index <JMdict_e.xml> <out>/v2-index
<bin>/span_dump.exe <out>/v2-index <pool>.tsv > <pool>.<build>.jsonl
python tools/classify_spans.py <pool>.HEAD.jsonl <pool>.<variant>.jsonl
python tools/classify_spans.py --selfcheck
```

`cut_corpus_pools.py` reproduces this session's pools byte-identically, and
`classify_spans.py` reproduces its counts (ikenaiDX random-4000: 5,465 clean /
1,402 entangled / 1,530 severed). Pin a copy of every binary before measuring —
two builds compared against a binary rebuilt underneath them is the failure
this harness has already had once.

The variants are one line in `cost.rs`, built into separate `--target-dir`s:

```rust
// HEAD
if bounds.is_some() && !(cand.grammatical_stack && cand.steps.is_empty()) {
// ungated
if !(cand.grammatical_stack && cand.steps.is_empty()) {
// uninflected only
if (bounds.is_some() || cand.steps.is_empty()) && !(cand.grammatical_stack && cand.steps.is_empty()) {
```

The v1 index and `corpus_disagree` were **not** needed and were not built: the
measurement that decides this question compares two v2 builds.

## 14. Not done

- **`LEN_GAIN` hints-off itself.** Reopened, not shipped. The refusal that
  closed it is withdrawn; the decision to ship is open.
- **The signal the residue term needs.** §11a. Not in the dictionary, so it is
  the tokenizer's POS or hand adjudication, and that is the next phase.
- **`〜てくる` / `〜ておく` / `〜てみる` rows.** Rate measured here for the first
  time. §12.
- The zero-signal tail is still truncated by `--top 2000` and still unread.
- Everything §10 of the predecessor left open: the `からな` family, criterion 2
  on the split parts, `BOUND_FORMS`' nine fixture rows, the quotative
  `と言った`, `gold_local.jsonl`, the six darwin baselines, gate 6's eighteen
  sentences, `忍` reading `しのび`, the `[noun]達` class, and the romanizer's
  `icchi`.
