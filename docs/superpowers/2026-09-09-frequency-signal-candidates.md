# Phase 3's frequency layer — every candidate measured, and the layer itself ruled out

**Date:** 2026-09-09
**Question as it started:** master plan §7's Phase 3 note says a frequency-rank
layer "stopped being a nice-to-have at Phase 1a" and that its licensing is
unresolved. Is the signal already on disk under a licence the plan accepts?
**Question as it ended:** §8 narrowed the search to three corpora and §11 built
the instrument that could judge one. Does a table built from either buildable
corpus rank the compounds the engine should merge above the ones it should
split?
**Answer:** no, and not for want of corpus or register — §12. Every
count-derived ranking scores at or below chance on a held-out register, two
signals needing no corpus at all beat all of them, and the discriminator the
metric asks for is wordhood rather than frequency. The layer as the master plan
specifies it is ruled out on measurement, so the licence question §9 left open
never has to be answered — including the one about the capture pool that only
the copyright holder could have settled.
**And the sequel, §13:** with frequency gone, §10.4's claim that "a derivation
cannot" tell the two groups apart had nothing behind it — and it had tested
exactly one derivation. A second one does separate them, shipped as
`cost.rs`'s `FUNCTION_STACK`: `transparent` 30 → 42 of 42, strict +31 spans,
tolerant +1, compounds +13 windows, no class regressed. Ungated it also
shatters `なりたいです` while improving every one of those numbers, which is
the more useful finding of the two.
**Probes:** [`derivation_probe.rs`](../../crates/jparser/examples/derivation_probe.rs) (§13),
[`freq_counts.rs`](../../crates/jparser/examples/freq_counts.rs) (§12),
[`common_inversion_probe.rs`](../../crates/jparser/examples/common_inversion_probe.rs) (§10),
[`ipadic_cost_probe.rs`](../../crates/jparser/examples/ipadic_cost_probe.rs) (§1-§6)

## 1. Why this was worth checking first

§3.3 already established IPADIC as permissive — notice retention only, the
`COPYING` beside the shipped dictionary says "Use, reproduction, and
distribution of this software is permitted" — and the app already ships it as
the boundary-hint dictionary. `vibrato` 0.5.2 makes `Token::word_cost()`
public. A MeCab-style word cost is a trained weight standing in for
`-log P(word)`, so on the face of it a frequency-derived prior was already
present, already licensed, and already downloaded: no new asset, no new
licence question, nothing to establish in writing.

That would have satisfied Phase 3's gate for free, which is why it was the
first thing to test rather than the last.

## 2. What the costs actually are

Measured against the installed `system.dic`
(`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`), the probe
tokenises each surface on its own and prints the cost IPADIC assigns.

**Compound against its own parts** — what a segmentation prior has to get
right, since the finding it would fix is that JMdict's priority marker
rewards splitting the compounds this plan exists to preserve:

| Compound | One IPADIC entry? | Cost | Parts |
| --- | --- | --- | --- |
| 日本語 | yes | **276** | 日本 3490 + 語 7198 |
| 情報処理 | yes | **4337** | 情報 4935 + 処理 2886 |
| 東京都 | **no** | — | 東京 3003 + 都 9428 |
| 研究者 | **no** | — | 研究 −413 + 者 7258 |
| 高等学校 | **no** | — | 高等 924 + 学校 1624 |

**Common against rare** — what a frequency rank has to get right:

| Common | Cost | Rare | Cost |
| --- | --- | --- | --- |
| 食べる | 7169 | 邂逅 | **4302** |
| 見る | 7001 | 逍遥 | **4645** |
| 言う | 6908 | 膏薬 | **5442** |
| 人 | 7118 | | |
| 時間 | 1342 | | |

## 3. What that decides

**As a frequency rank it is not one, and the counterexample is flat.** 人 —
among the most common nouns in the language — costs 7118, while 邂逅, a rare
literary word for a chance encounter, costs 4302. 食べる, 見る and 言う all cost
more than all three rare words tested. And 研究 costs **−413**: a negative
number, which no probability rank produces. These are discriminative weights
trained for segmentation, mixing word likelihood with part-of-speech
connection behaviour, and they are not ordered by corpus frequency even
approximately. Using them to tell a reader "this word is common" would be
actively wrong, not merely noisy.

**As a segmentation prior it points the right way but covers the wrong
words.** Where IPADIC lexicalises a compound the signal is exactly what the
inverted-priority finding wants: 日本語 at 276 against 3490 + 7198 for its
parts is an overwhelming vote for the compound, the opposite of what JMdict's
marker does. But IPADIC is a short-unit dictionary, so it lexicalises only
some compounds — and three of the five tested, including 東京都, the example
the master plan and every phase document uses, are not entries at all. The
coverage is biased against precisely the cases that motivated the layer.

**So Phase 3's gate is still unmet.** The layer still needs a real
frequency source and that source's licence still has to be established in
writing. What changed is that the cheapest candidate is closed with numbers
instead of standing open as a maybe.

## 4. A licence nuance worth recording before anyone leans harder on this

Today the app uses this dictionary at runtime, for boundary hints, downloaded
by the user's own installation. Reading `word_cost` at runtime keeps that
posture exactly. **Baking a derived table into a shipped index would not** —
that turns runtime use of a downloaded asset into redistribution of a
derived work, which is a different question even under a notice-retention
licence. The shipped `NOTICE` also records that the dictionary's connection
ids "are remapped using CORE data in BCCWJ", and BCCWJ is NINJAL data that is
not itself freely licensed. That affects the connection matrix rather than the
word costs, and it is already accepted for the current runtime use, but anyone
proposing to redistribute anything derived from this file should read that
notice first.

## 5. What to check next, if Phase 3 is wanted

**UniDic.** §3.3 established it as tri-licensed GPL-2 / LGPL-2.1 / **BSD
New**, takeable under the BSD leg, and noted it has better readings than
IPADIC. It is also built from BCCWJ, and some editions ship corpus frequency
columns alongside the segmentation weights — which, unlike a trained cost,
would be an actual frequency. That is the next candidate, and unlike this one
it is not free: it is a different dictionary from the one on disk, so it
brings a download, a size cost, and a notice obligation to establish.

Two things this probe says about how to evaluate it. Ask for the *frequency*
column specifically and do not accept a cost column as a proxy — §3 is what
that substitution looks like when measured. And check compound coverage
against the same five surfaces above, because a short-unit dictionary will
miss the same compounds however good its numbers are.

## 6. What not to do

Do not start Phase 3's implementation on IPADIC costs. They would improve
nothing the layer exists for, and a display-facing "common word" marker built
on them would be wrong in a way users would notice — 邂逅 marked commoner
than 人.

## 7. UniDic, checked the same day — ruled out on two grounds, licence not among them

§5 named UniDic as the next candidate. It does not survive either.

> **This section originally gave three grounds and one of them was wrong.**
> It claimed UniDic's licence was "not established where the gate requires",
> on the basis that NINJAL's own pages did not state it — `about_unidic_en.html`
> states no licence and `unidic.ninjal.ac.jp` did not resolve, so the only
> attestation found was a third-party wrapper's README. That was a failure to
> find the page, not an absence. The maintainer supplied
> `https://clrd.ninjal.ac.jp/unidic/en/`, which states the licence plainly and
> links its texts. **The licence ground is withdrawn.** The two below stand,
> and the corrected licence finding is recorded first because it is the one a
> later reader is most likely to need.

**Licence: established, and permissive.** NINJAL's own page states "Licence:
GPL v2.0/LGPL v2.1/BSD New triple license" and links each text.
`clrd.ninjal.ac.jp/unidic/copying/BSD` is a 3-clause New BSD licence,
"Copyright (c) 2011-2021, The UniDic Consortium", with the usual
notice-retention pair plus the no-endorsement clause. That is compatible with
`MIT OR Apache-2.0` and it is the copyright holder's own written statement, so
master plan §7's "established in writing first" is satisfiable for UniDic.
Worth knowing for any future use of the dictionary itself — its readings are
better than IPADIC's (§3.3) — even though it fails this layer's purpose below.
One wrinkle for a redistributor: the archive bundles a `ChaMame` UI installer
that its README marks as separately licensed.

**Size: 576 MB at minimum**, from NINJAL's own download table — 576 MB for
Contemporary Written Japanese without `model.def`/`matrix.def` and 2.1 GB
with, 609 MB and 2.2 GB for the Spoken edition. Against the roughly 47 MB
compiled `system.dic` the app ships today, paid on every user's first run.
(An earlier draft of this section said 770 MB, which was a wrapper's installed
footprint rather than the distribution.)

**And it carries no frequency data.** UniDic's `lex.csv` columns are
`pos1`–`pos4`, `cType`, `cForm`, `lForm`, `lemma`, `orth`, `pron`,
`orthBase`, `pronBase`, `goshu`, `iType`, `iForm`, `fType`, `fForm`, with
`lemma_id` last — every one morphological, orthographic or phonological. The
archive's own README confirms it: it reports lexicon statistics (228,315
lexemes, and short-unit counts up to 879,222 under different normalisations)
and documents no frequency or occurrence-count column anywhere. The only
numeric weight is MeCab's `cost` field — the same trained segmentation weight
§3 measured and rejected, one dictionary over.

**A note for a different question.** NINJAL also publishes UniDic for
Contemporary *Spoken* Japanese, whose register is far closer to the dialogue
this app is mostly pointed at than either IPADIC or the written edition. It
carries no frequency data either, so it is irrelevant to this layer — but if
boundary-hint quality is ever revisited, it is the candidate §3.3 did not
know about.

## 8. The reframing this forces

Two candidates in, the pattern is not about which dictionary to pick. **A
tokenizer dictionary is the wrong kind of artifact to look for frequency in.**
The MeCab family carries trained segmentation costs because that is what a
segmenter needs; corpus counts live in *frequency lists*, which are separate
publications. Searching dictionaries for a frequency column was the wrong
search, and two measurements were the cheapest way to find that out.

For Japanese the authoritative frequency lists are BCCWJ-derived NINJAL
products — the 短単位語彙表 family — which are their own publications with
their own terms, and those terms are the thing the gate would have to
establish. That is a materially different licensing question from the
dictionaries, and it is the one Phase 3 actually faces.

Three candidates remain, and they differ in what they cost rather than in
whether they are available:

1. **Derived from public-domain text.** Aozora Bunko, 新字新仮名 only, is
   already tier A of the gold corpus and `corpus_staging/fetch_aozora.py`
   already fetches it. Counts derived from public-domain text carry no
   encumbrance at all, so the licence gate is satisfied by construction — the
   cheapest gate of the three. The cost is register: the master plan already
   records that Aozora is literary prose from roughly 1900–1955 and "carries
   no news register at all", and it carries no contemporary or colloquial
   register either. A frequency rank built on it would be a frequency rank
   for a language nobody now writes.
2. **Derived from CC BY-SA text.** ja.Wikinews is already tier B and
   `fetch_wikinews.py` already fetches it. A derived table inherits
   share-alike, which is an obligation to accept knowingly rather than a
   blocker — the project already ships CC BY-SA data, since JMdict is CC BY-SA
   4.0 and §3.3 records that as constraining the index and not the code. News
   register, which is a real improvement on Aozora and still not the app's
   input distribution.
3. **Derived from the project's own capture pool.** The 66,114 cleaned
   visual-novel lines under `vn_data` are the true input distribution — §9.1
   says so in those words — and they are deliberately never committed because
   the transcribed lines are copyrighted. A frequency table over them is
   counts, not text, and §9.1's own reasoning is that "register and style are
   not copyrightable; transcribed lines are." That is the most on-target
   register available and the only candidate that matches what users actually
   paste. It is also the one that turns on a judgment about whether a
   statistical summary of copyrighted text may be distributed — which is the
   copyright holder's call, not a measurement, and is where this document
   stops.

## 9. What the next step is, and what it is not

It is not code, and it is no longer a dictionary hunt. It is a decision
between the three above, and each carries a different question: for Aozora,
whether a pre-war literary frequency rank is worth having at all; for
Wikinews, whether share-alike on a shipped table is acceptable; for the
capture pool, whether counts over copyrighted text may be shipped.

Only the third needs a judgment nobody can make on the copyright holder's
behalf. The first is buildable today with tooling already in the repo, and its
weakness is measurable rather than legal — which means it could be built, its
rank compared against the corpus's own compound decisions, and kept or
discarded on evidence, exactly as the two candidates above it were.

## 10. Before choosing a corpus: the inversion sized on the gold corpus itself

§9 ended at a choice between three corpora. Before paying for any of them
the premise was sized: master plan §5.2 and §7 justify the layer by the
claim that JMdict's priority marker is inverted on the compounds the engine
exists to preserve. How much of the corpus's compound behaviour is actually
that? **Probe:**
[`crates/jparser/examples/common_inversion_probe.rs`](../../crates/jparser/examples/common_inversion_probe.rs),
hints on, gold index rebuilt from `gold_jmdict_subset.xml`. The eight-cell
baseline was reproduced before and after (0.8031 / 0.8521 / 0.9063, 53
merges forgiving 106 spans) and the probe touches no engine code.

### 10.1 The coarsening count cannot answer it

The natural place to look is `score`'s own line -- "coarsening: 106 gold
spans inside 53 dictionary merges" -- since those 53 are the compounds the
engine preserved. Of them, **0 are inverted** (compound without COMMON while
a part carries it). That is structural rather than empirical. With
`CROSS = 0`, a compound and its exact split tie on `COVER`, and `relax`'s
tie-break gives an equal-cost tie to the longer step; a compound therefore
wins only when it is evidenced at least as well as its parts. 48 of the 53
are COMMON-against-COMMON ties, 3 are numeral+counter headwords (18日, 2月)
whose numeral part has no COMMON, and 2 (タイムリーヒット, ノーベル化学賞) beat
parts that had no candidate at all. A compound that lacks a discount its
parts carry is priced out before it can become a merge, so the coarsening
count says nothing about the inversion in either direction and should not be
quoted as evidence about it.

### 10.2 The population that can: compounds available but declined

Every window between two gold boundaries, two or more gold spans wide, at
which an allowed candidate exists and the parse placed no segment. There are
**141**, and **104 are inverted** (88 with every part COMMON). The other 37
carry COMMON on both sides and lost to `PRONOUNCE_CROSS` -- the はく-shaped
reading-only merges the cost model keeps out on purpose -- not to the marker.
Every master-plan example present in the corpus is in the inverted set and is
split today for exactly that reason: 音がする (five sentences), 首を振る,
ことになっている, 音を立てる, 風を切る. 東京都 itself does not occur in the
corpus. So the claim holds: the inversion is real, and it is the mechanism.

What the parse did instead across the 141: the gold split in 105, a smaller
merge in 6, a real miss in 30.

### 10.3 What fixing it would score, and why that is the finding

For each declined window the probe asks what a COMMON-sized discount on the
compound would have done -- any signal that gives the compound parity with
its parts, a frequency rank included, behaves the same way at the DP. Of 99
tiled inverted windows, **76 flip**. 69 of those replace a *gold split* --
spans that are strict hits today -- so they would become tolerant-only: 150
distinct strict spans at risk, about -6.9 points strict with tolerant flat.
7 replace real misses: 18 distinct spans could be newly forgiven, about +0.8
points tolerant at the very most.

The gold corpus is adjudicated at short-unit granularity (§5.2 of the master
plan already records this), so preserving 音がする as one segment can never
register as a gain on it; it shows only as strict loss. **No frequency
source, whatever its licence, can be gated by the current metrics for the
problem that motivates the layer.** The blocker in front of Phase 3 is
therefore not which corpus to choose but that the gate has no instrument
that can reward a compound. The 76 flip windows are printed by the probe and
are one sitting's worth of hand adjudication -- merge or split, per window --
which would be that instrument, and needs no licence.

### 10.4 Why a build-time COMMON derivation is not the free fix

The cheapest response to an inverted marker is to derive COMMON at build time
for any headword whose parts all carry it. The flip set shows what that ships:
alongside the wanted 副大統領, 月平均, 名誉教授, 発売所, 大統領選挙 and
音がする it flips 名の into 名のる, 煙が into 煙がる, and それは, 彼の, のか,
かよ -- particle absorption and false verb matches, the §5.2 spurious-match
class. A frequency rank is exactly what separates those two groups; a
derivation cannot, and the current harness scores both groups identically.
That is the argument for frequency over derivation, and it is also why
neither can be adopted until 10.3's instrument exists.

## 11. The gate instrument §10.3 asked for, built and read

§10.3 concluded that the blocker in front of Phase 3 is not which corpus to
choose but that the gate has no instrument able to reward a compound. That
instrument now exists: `score --compounds`, backed by
[`crates/jparser/tests/fixtures/compound_gold.jsonl`](../../crates/jparser/tests/fixtures/compound_gold.jsonl),
built by
[`tools/adjudicate_compounds.py`](../../tools/adjudicate_compounds.py)
from the probe's `--emit` output, and implemented in
[`crates/jparser/src/bin/jparser-cli/compounds.rs`](../../crates/jparser/src/bin/jparser-cli/compounds.rs).

### 11.1 What was adjudicated, and by what rule

All 194 windows of both §10 populations — the 53 merges the parse made and the
141 compounds it declined — so the metric can register a preserved compound
*and* a spurious merge with one rule rather than only the half a change is
trying to move. Judged once per distinct `(surface, headword)` type, 139 of
them, into four classes:

| class | verdict | n windows | what it is |
| --- | --- | --- | --- |
| `lexical` | should merge | 112 | a dictionary unit a reader looks up whole: compound noun (`副大統領`), expression (`首を振る`), numeral + counter (`16日`), a conjugated single verb (`行かなきゃ`), or a fixed construction whose force is not its parts stacked (`である`, `によると`, `ことになっている`) |
| `transparent` | should split | 42 | function-word stacking whose parts are each what the reader needs: `には`, `にも`, `での`, `との`, `のか`, `彼の` |
| `false-match` | should split | 32 | the headword is a different word; merging shows a wrong gloss: `がした` as `がす`, `名の` as `名のる`, `はこう` as `はく` |
| `ambiguous` | excluded | 8 | context-dependent at type level: `では`, `でも`, `時に`, `にして`, `天と地`, and the `どうか`/`かなあ` overlap in 「それはどうかなあ」 |

Overlaps are resolved per sentence, not per type: two windows cannot both be
segments, so among overlapping `lexical` windows the longest is the one
expected to merge and the rest carry `expect: split` with a note naming the
winner. Six windows are resolved that way, and one of the six needed a hand
override — `服を着る` is a real expression but in 「立派な黒服を着て」 it
straddles the noun `黒服`, so longest-wins was wrong there.

**Excluding rather than guessing the `ambiguous` eight is deliberate**, on
§5.2's own precedent: it records that every mechanical discriminator tried for
the spurious-merge class cut the wrong way, and a metric that guesses would
inherit exactly that. The count is printed beside the ratio so the
denominator is never quietly smaller than the file.

**The judgment most open to dispute is the `transparent` class.** `には` and
`にも` are JMdict headwords and the engine merges them today; they are
adjudicated *split* here on the ground that merging hides the particle doing
the work — a reader's difficulty with `には` is knowing that `は` is the topic
marker attaching after `に`, and a merged segment puts neither particle within
reach. That call alone accounts for 12 of the metric's misses. Because the
class is stored per record, reversing it is a filter over the file rather than
a re-adjudication, and the by-class line makes its contribution visible
without recomputing anything.

### 11.2 What the engine scores

Baseline as it stood when §11 was written, and unchanged by anything in §10 or
§11 — the eight recorded cells were byte-identical throughout and `score`
without `--compounds` printed exactly what it printed before. **§13.2 has since
replaced these cells deliberately**, so read the table below as the figure
every measurement in §10 through §12 was taken against, and master plan §5.2's
current-baseline block for what the engine scores now:

| | span strict | span tolerant | entry top-1 | **compounds** | kept whole | split apart |
| --- | --- | --- | --- | --- | --- | --- |
| hints off | 0.7272 | 0.7975 | 0.9075 | **0.5161** (96 / 186) | 43 / 106 | 53 / 80 |
| hints on | 0.8031 | 0.8521 | 0.9063 | **0.5430** (101 / 186) | 35 / 106 | 66 / 80 |

By class, hints on: `lexical` 41 / 112, `transparent` 30 / 42,
`false-match` 30 / 32.

Two readings the span metrics could not give.

**The engine keeps 35 of 106 adjudicated compounds whole — 33%.** That is the
size of the gap Phase 3 exists to close, stated as a number for the first
time, and §10.2's finding is what it is made of: the compounds it declines are
overwhelmingly the ones whose priority marker is inverted. (36 of 106 as of
§13.2, so the gap this sentence names is almost exactly where it was: what
§13 closed was the `transparent` class, not this.)

**And the hint benefit is not what the span metrics reported.** Hints improve
the compound metric overall (0.5161 → 0.5430) but they do it *entirely* by
suppressing bad merges — `false-match` 24 → 30 of 32, `transparent` 27 → 30 of
42 — while compound preservation gets *worse*, `lexical` 45 → 41 of 112.
Vibrato's boundaries are a constraint, so this is the shape one should expect
and neither span metric could show it: §5.2 recorded the hint benefit as
+1.53 strict / +2.13 tolerant with no sign that the two halves move in
opposite directions.

### 11.3 What it says about the three candidates

The flip test of §10.3, rescored on this metric: a COMMON-sized discount on
the compound — how any parity signal, frequency rank included, reaches the DP
— moves the metric **0.5430 → 0.6774, +25 windows**, where the strict span
metric scored the same change at about −6.9 points. That is the instrument
doing its job: the change Phase 3 wants is now measurable as a gain.

The per-class split is the finding that chooses between a frequency layer and
the free alternative:

| class | now | after a blanket COMMON discount |
| --- | --- | --- |
| `lexical` | 41 / 112 | **88 / 112** (+47) |
| `transparent` | 30 / 42 | 10 / 42 (−20) |
| `false-match` | 30 / 32 | 28 / 32 (−2) |

A build-time derivation cannot tell `副大統領` from `名の`, so it buys the +47
and pays the −22; net +25 is real but it ships 22 wrong merges, two of them
displaying an outright wrong word. **Separating those two groups is precisely
what a frequency rank does and a derivation cannot** — which is the argument
for Phase 3's layer, now with a number on both the prize and the price.

### 11.4 What is still open

The metric is 194 windows from 150 sentences, so it is small, and it is a
model's adjudication rather than the author's — `corpus_staging/build_gold.py`
draws that distinction for the gold corpus and it holds here. Neither is a
reason to wait: the corpus choice of §8 can now be made on evidence, since a
candidate frequency table can be built, priced into `cost.rs`, and scored on
this metric without asking anyone to accept a −6.9-point strict regression as
progress.

What has *not* changed is §9's licence question. Aozora remains buildable
today with tooling already in `corpus_staging` and its weakness remains
register; the capture pool remains the only candidate turning on a judgment
about distributing counts over copyrighted text. The instrument decides which
is worth having, not whether it may be shipped.

## 12. The separation test: both buildable corpora built, counted and read

§11.4 said the corpus choice of §8 "can now be made on evidence". This is that
evidence, and it cost less than pricing a table into `cost.rs` would have.
**Probe:**
[`crates/jparser/examples/freq_counts.rs`](../../crates/jparser/examples/freq_counts.rs);
scratch tooling in `corpus_staging/fetch_freq_corpora.py` and
`corpus_staging/separation.py`, gitignored like the fetchers beside them.

### 12.1 The question a table can be asked before any engine change

§11.3 priced the ceiling: a blanket COMMON discount takes `lexical` from 41 to
88 of 112 (+47) and pays `transparent` 30 → 10 and `false-match` 30 → 28
(−22), net +25. So a frequency table is only worth having if it earns the +47
without the −22 — and the adjudicated fixture is a lookup table of 139 distinct
`(surface, headword)` types with a class on each. Whether a candidate corpus
can do that is therefore a *ranking* question over those 139 types: does the
table put the `lexical` types above the `transparent` and `false-match` ones?
No cost model, no `cost.rs` edit, and no licence settled to ask it.

**Held-out register, because each candidate is compromised on its own tier.**
Aozora carries no news register at all, so it predictably has nothing to say
about `副大統領`, `大統領選挙` or `日本郵政公社`; and a Wikinews table would score
on those same windows partly because a third of the gold corpus *is* Wikinews,
which is circular. Both tables were therefore built and each scored only on the
types none of whose windows come from its own tier. Tier D is derived noise and
its source string carries its parent tier (`d:a:…`), so it resolves to that.
Tier C1 is hand-written pastiche and is held out from neither.

| table | corpus | chars counted | tokens | distinct tokens | held-out types |
| --- | --- | --- | --- | --- | --- |
| Aozora | 400 works, catalogue-picked one per author, 新字新仮名 public domain | 12,119,277 | 7,899,934 | 91,958 | 76 (tier B + C1) |
| Wikinews | the whole `jawikinews` dump, ns0, 4,118 articles | 2,720,659 | 1,546,666 | 42,934 | 84 (tier A + C1) |

The counter uses the engine's own tokenizer — the shipped `system.dic` that
`hints::VibratoTokenizer` loads — so "as a word" here means what the shipped
boundary hints already mean and no second segmenter's opinion enters. It emits
every count a ranking could plausibly key on from one corpus pass: the surface
as a substring (`raw`), the surface where both its endpoints are token
boundaries (`aligned`), the surface as exactly one token (`tok_exact`), tokens
whose IPADIC base form is the surface or the headword, and the parts the
surface tokenizes into with each part's own count.

### 12.2 Both traps are real, and are the same number twice

Zero-count share of the 87 `lexical` types, by the tier their windows come
from:

| table | tier A types | tier B types | tier C1 types |
| --- | --- | --- | --- |
| Aozora | **0 / 36** zero | **26 / 39** zero | 0 / 13 zero |
| Wikinews | 17 / 36 zero | **0 / 39** zero | 8 / 13 zero |

Aozora cannot see two-thirds of the news-sourced compounds, exactly as
predicted. And Wikinews sees **every one** of the 39 news-sourced compound
types — 0 zeroes out of 39, against 17 of 36 on Aozora's tier. That is the
circularity, measured: a Wikinews table's apparent strength on those windows is
the gold corpus's own sampling showing through. Held-out scoring is not a
refinement here; without it the two tables would have swapped ranks for a
reason that has nothing to do with either being better.

### 12.3 What the separation test decides: no

Positives `lexical`, negatives `transparent` + `false-match`, `ambiguous`
excluded as always. AUC is Mann-Whitney with ties at half. "Best threshold" is
the cut maximising should-merge minus should-split above it, reported in
§11.3's own currency. The last row is the reference every scalar has to beat:
a blanket discount with no signal at all, which merges everything on the same
held-out subset.

| scalar | Aozora AUC | net windows | Wikinews AUC | net windows |
| --- | --- | --- | --- | --- |
| `raw` | 0.173 | +43 | 0.297 | +14 |
| `aligned` | 0.221 | +43 | 0.319 | +14 |
| `tok_exact` | 0.457 | +43 | 0.510 | +14 |
| `lemma_surface` | 0.432 | +43 | 0.510 | +14 |
| `lemma_headword` | 0.433 | +43 | 0.468 | +14 |
| PMI against the parts, on `aligned` | 0.449 | +43 | 0.458 | +16 |
| PMI against the parts, on `tok_exact` | 0.456 | +43 | 0.510 | +14 |
| compound against its rarest part | 0.288 | +43 | 0.369 | +14 |
| `aligned / raw` | 0.429 | +43 | 0.353 | +14 |
| *control:* number of IPADIC parts | 0.658 | +44 | 0.709 | +21 |
| *control:* surface length in characters | **0.779** | **+45** | **0.733** | **+29** |
| **blanket, no signal** | — | **+43** | — | **+14** |

Every count-derived scalar lands at or below 0.51, and on the Aozora table
every one of them has its best threshold at "merge everything" — the +43 is
the blanket's own number, meaning no cut through those counts is worth making.
On Wikinews one scalar clears the blanket by two windows, and does it by
merging 24 of 56 `lexical` windows instead of 56, which is not the +47 §11.3
asked for under any reading.

**The volume scalars do not merely fail to separate; they separate backwards.**
`raw` at 0.173 and `aligned` at 0.221 on Aozora mean the should-*split* types
rank above the should-merge ones far more often than not. `には` occurs 2,585
times on token boundaries in the news corpus, which puts it between `である`
(2,569) and `として` (4,852) — two `lexical` types. A threshold cannot be placed
between them because there is nothing between them.

**Two controls that need no corpus at all beat every corpus scalar on both
tables.** Surface length reaches 0.779 / 0.733 and net +45 / +29; the number of
IPADIC parts reaches 0.658 / 0.709. This is a floor, not a proposal — both are
build-time derivations of exactly the kind §10.4 rules out, and length
separating this fixture is partly the adjudication's own shape (`transparent`
is two-character particle stacking by definition). The point is that a
candidate signal which cannot clear a free one has not earned a licence
question, a shipped table, or a `cost.rs` term.

An earlier version of the PMI scalar smoothed unseen surfaces with add-one and
scored AUC 0.883, which looked like the answer. It was the arity control in
disguise: `二重三重`, with **zero** occurrences and four parts, ranked first of
139. The controls are in the table above so that mistake cannot be made twice.

### 12.4 Why it fails, and what that settles about candidate 3

The two corpora are 4.7× apart in size and as far apart in register as the
three candidates go, and they return the same verdict by the same mechanism.
The mechanism is neither coverage nor register:

**What the metric asks for is wordhood, and frequency does not measure
wordhood.** `には` and `副大統領` are both sequences whose endpoints fall on token
boundaries; counted as sequences, `には` wins by three orders of magnitude, so
volume ranks them backwards. Counted as words under the shipped short-unit
tokenizer, *neither* is a token, so token and lemma counts are zero for both
and rank nothing at all: **of the 87 `lexical` types, 5 exist as a single
IPADIC token** in 2.9 M characters of news — `として`, `による`, `タイムリーヒット`,
`となり`, `五日`. A word-frequency list, keyed the way every real frequency list
is keyed, can price 5 of 87 targets.

That settles §8's candidate 3 without needing the judgment it turns on. The
`vn_data` capture pool is the best register available and the only candidate
requiring a decision about distributing counts over copyrighted text — and it
would fail this test for a reason that has nothing to do with register.
**Nobody has to make that call.** Two corpora that cost nothing but a fetch
retired the question, which is the same shape of result as §2 and §7: the
cheapest measurement closed the expensive one.

### 12.5 The one route this leaves, and it is not a corpus

The counts do say what *would* separate the two groups: a frequency table over
a **long-unit** token inventory — one in which `副大統領` is a token and `には` is
not. Then `tok_exact` is positive for the compounds and zero for the particle
stacks, which is the separation the metric wants, and it is the same table
these two corpora could not provide only because the inventory that segmented
them is short-unit.

So §8's reframing holds one level up. It said a tokenizer dictionary is the
wrong artifact to look for *frequency* in; what this adds is that the corpus
was never the missing piece either. The missing piece is the **unit**, and a
unit is a property of a segmenter's inventory, not of text. That is a different
question from Phase 3's as written — it would mean adopting a second segmenter
whose long-unit inventory is itself the signal, with its own size and licence
gates — and it is **untested here**. It is recorded as the only route the
measurements leave open, not as a recommendation, and not as licence to resume
the dictionary hunt §8 closed.

### 12.6 The `transparent` ruling, settled

§11.1 flagged the `transparent` class (`には`, `にも`, `での`, `との`, `のか`,
`彼の`) as the adjudication most open to dispute, worth 12 of the metric's
misses. **It stands as should-split**, and the separation test is what decides
it rather than taste. Rerun with the class moved to the positives, the best
threshold for every scalar tested — corpus-derived and control alike — becomes
"merge everything": blanket +52 held-out windows, and nothing beats it. So
reversing the class does not make a frequency layer easier to justify, it
removes the justification, because §11.3's argument was precisely that a rank
could take the +47 while declining the −22. A ruling that deletes the −22
decides Phase 3 by adjudication instead of by measurement, which is the one
thing the third metric was built to avoid.

The consequence is carried rather than hidden: the engine merges `には` and
`にも` today, so those 12 windows are a real Phase 3 gap and the by-class line
(`transparent 30 / 42`) is where it shows. The headline stays 0.5430 and is
still never to be quoted without the class line beside it.

### 12.7 Baseline

Unchanged and reproduced before reporting — all eight cells byte-identical
(hints on 0.8031 / 0.8521 / 0.9063, 53 merges, compounds 0.5430 with `lexical`
41 / 112, `false-match` 30 / 32, `transparent` 30 / 42; hints off 0.7272 /
0.7975 / 0.9075, 75 merges, 0.5161). The probe reads the index and the
tokenizer and touches no engine code. Gates: jparser 333 with `mecab`, 1
ignored; `clippy --all-targets --features mecab` and `fmt` clean.

## 13. A derivation that does separate the classes — and the damage it does ungated

§12 ruled out frequency on both buildable corpora. That left §10.4's claim
standing on its own: "A frequency rank is exactly what separates those two
groups; a derivation cannot." With frequency gone, that sentence had nothing
behind it — and §10.4 had tested exactly **one** derivation, COMMON-if-all-
parts-COMMON. So the question was reopened for the rest of them. **Probe:**
[`crates/jparser/examples/derivation_probe.rs`](../../crates/jparser/examples/derivation_probe.rs).

### 13.1 The question, and the bound before any code

The probe asks, per adjudicated type, whether the surface admits a partition
into two or more index entries all sharing some property. A property is only
interesting if it holds for `transparent` and `false-match` and *not* for
`lexical`. Over the 139 types:

| property | `lexical` | `transparent` | `false-match` |
| --- | --- | --- | --- |
| every part `PARTICLE` | 9 / 87 | 12 / 22 | 10 / 23 |
| every part `COMMON` (§10.4's own) | **72 / 87** | 21 / 22 | 21 / 23 |
| every part exact | 75 / 87 | 22 / 22 | 21 / 23 |
| every part `PARTICLE` **and** `COMMON` | **2 / 87** | 7 / 22 | 3 / 23 |

The middle two rows are §10.4's finding restated: they hold for nearly
everything, which is exactly why a derivation "cannot tell `副大統領` from
`名の`". The last row is not like them. **2 of 87 `lexical` types against 10
of 45 should-split ones** is a separation, and §10.4 never tested it.

Cross-referenced against what the engine actually does today — most of those
windows are already split, so the property holding changes nothing there — the
achievable move was **+12 windows** (`には` in 8 sentences, `にも` in 4: every
one of the `transparent` class's 12 misses) against **−3** (`として` in 2,
`となり` in 1, all three currently kept whole).

And then the finding that made it worth building: **the short-unit gold corpus
splits all four surfaces.** So the merges this would block are currently
destroying 30 gold spans (16 + 8 + 4 + 2) that strict accuracy scores as
misses. Every earlier Phase 3 candidate traded strict spans for compound
windows — §10.3 priced frequency at about −6.9 points strict. This one is the
first where the compound adjudication and the short-unit gold **agree**.

### 13.2 What it measures on-engine

`FUNCTION_STACK` in `cost.rs`, a flat penalty on a match whose span also
partitions into two or more entries each carrying `COMMON` and `PARTICLE`.
The property is a fact about a span against the rest of the lattice rather
than about an entry, so it cannot be a build-time `WordFlags` bit: it is
computed by a second pass in `lex::candidates` and carried on `Candidate`.

| | strict | tolerant | entry top-1 | compounds | lexical | false-match | transparent |
| --- | --- | --- | --- | --- | --- | --- | --- |
| hints on, before | 1737 | 1843 | 0.9063 | 101 | 41 | 30 | 30 |
| hints on, after | **1768** | **1844** | **0.9092** | **114** | **42** | 30 | **42 / 42** |
| hints off, before | 1573 | 1725 | 0.9075 | 96 | 45 | 24 | 27 |
| hints off, after | **1588** | 1714 | **0.9093** | **109** | **46** | 24 | **39** |

Hints on, all four metrics move up and **no class regresses** — the
`transparent` class goes to 42 of 42. Hints off, the tolerant metric loses 11
spans against a merge count falling 75 → 62, so what it lost is coarsening it
was crediting; §5.2 records that metric as over-forgiving by about 1.3 points
for exactly this reason. That reading is an interpretation of the coarsening
line, not a separately proven claim, and it is the one cell in the table
pointing the wrong way.

Magnitude is **flat from 1 to 50 in both hint states**, identical cells
throughout, so only the sign matters — as with `PRONOUNCE_CROSS`. 4 is the
small end of that range, chosen because this term suppresses a *shape* that
overlaps a legitimate class, so there is no argument for the extra margin
`PRONOUNCE_CROSS`'s own comment makes.

Latency unmoved: `bench`'s scaling cell reads 1.10x hints on against 1.12x
before, and the second pass in `lex::candidates` is bounded by the span width
the FST walk already bounds rather than by a whole-text table — which is the
shape Phase 2c removed from that function.

### 13.3 The gate, which is the real finding

Ungated, this term is a defect. It shattered `なりたいです` (one segment,
`なる` Want-to Polite) into `なり` + `たいです`, and `して` into `し` + `て`,
because `なり` and `し` are themselves COMMON particle entries — so a
conjugated form looks like stacked function words. That is master plan §4.3's
shattering class, the thing this whole rewrite exists to remove, and it is a
worse defect than the merge the term cures.

**The gold corpus's own metrics improved straight through that damage.** With
the ungated term, strict rose 30 spans and compounds rose 11 windows while
`なりたいです` was being destroyed, because those sentences are in
`parse_sentences.txt` and not in the 150-sentence gold corpus. What caught it
was the curated snapshot — 30 hand-written sentences with documented coverage
intent, whose whole purpose is to be read rather than scored.

Gating the term on `steps.is_empty()` — a conjugated match is not function-word
stacking — leaves the snapshot **byte-identical** and keeps every gain in
§13.2.

Two things to carry from that. A metric that improves is not evidence a change
is correct; it is evidence about the sentences the metric contains. And the
`parse_sentences.txt` snapshot earned its place here: it is the only gate in
the harness that would have failed.

### 13.4 What this does and does not settle

It closes the `transparent` class, which §11.1 flagged as the metric's most
disputable call and §12.6 confirmed as a real Phase 3 gap: 42 of 42, and the
12 `には`/`にも` merges are gone. It does **not** touch the compound
preservation gap that motivated Phase 3 — `lexical` moves 41 → 42 of 112, so
the engine still declines 70 of the compounds the corpus says it should keep,
and `副大統領` is still split for the inverted-priority reason §10.2 measured.

So §10.4's sentence needs its correction stated plainly: a build-time
derivation **can** separate function-word stacking from lexical compounds, and
the conjunction that does it was simply not among the ones tested. What no
derivation and no frequency table has done is the other half — telling
`副大統領` from its parts — and that half is still open with no candidate.

### 13.5 Re-pricing the derivation §10.4 rejected, and why it still fails

§13.4 left the other half open: `lexical` at 42 of 112, `副大統領` still split
by the inverted marker §10.2 measured. §10.4's blanket derivation — grant
COMMON to an entry whose parts all carry it — was rejected at +47 `lexical`
windows against −22 wrong merges, and **20 of that −22 was the `transparent`
class**. §13.2 has just taken `transparent` to 42 of 42 with a term that fires
on evidence of its own, whether or not a discount was granted. So the −22
looked largely pre-paid, and a rejected option was worth re-pricing rather
than left closed on an arithmetic that had changed.

It was implemented as `DERIVED_COMMON = COMMON`, granted per character to an
entry carrying no marker whose span partitions into parts that all carry one —
exactly §10.3's "COMMON-sized discount", reusing `lex::candidates`' second
pass for a second property.

**It buys what §10.4 said it would and costs more than §10.4 could see.**
Hints on: `lexical` 42 → **82 of 112**, kept-whole 36 → 77 of 106, compounds
0.6129 → 0.7688 (+29 windows), tolerant 1844 → 1870 (+26). And strict 1768 →
**1664, −104 spans**, which is §10.3's own prediction (it priced this at "about
−6.9 points strict", and −104 spans is −4.8) arriving as measured.

Three further findings, and the last of them is what decides it.

**The hypothesis was wrong, and the reason is worth keeping.** `transparent`
fell 42 → 33 and `false-match` 30 → 28, so the −22 was *not* pre-paid.
`FUNCTION_STACK` blocks *particle* stacking — `に` + `は`, both `PARTICLE` —
while the merges this discount creates are `これは`, `時間が`: **noun plus
particle**, which that predicate never covers. The −22 was never one class,
and fixing the particle-stack half does nothing for the other.

**It is not a sizing conflict.** `DERIVED_COMMON` gives a two-character stack
−4 where `FUNCTION_STACK` adds +4, so the two appeared to cancel exactly.
Raising `FUNCTION_STACK` to 12 changed **nothing** — every cell identical,
`transparent` still 33 — so the interaction is not arithmetic and no
re-sizing recovers the class.

**It re-breaks success criterion 3.** `the_particle_absorbing_merges_are_gone`
fails with this term in place, and the curated snapshot shows `これは` merged
as one segment where it was `これ` + `は`. That fixture is the only guard for a
class §5.2 records that *neither* span metric catches — strict scores it as an
ordinary miss and tolerant actively credits it. A change that lifts tolerant
26 spans while that gate fails is the §5.2 coarsening warning happening in
front of us: merges went 38 → 93.

**Reverted.** Not because the compound gain is not real — it is the largest
move on that metric anything has produced, and Phase 3 exists for exactly it —
but because it is bought by reintroducing the defect the branch's own success
criterion names, and no magnitude separates the two. §10.4's rejection stands,
now with the number it lacked and with its one stated reason corrected: the
obstacle is not that a derivation cannot tell `副大統領` from `名の`, since
§13.1 found one that separates function stacking cleanly. The obstacle is that
the discount which reaches `副大統領` also reaches `これは`, and nothing
measured so far distinguishes those two.

**What that leaves.** Compound preservation is still the open half of Phase 3,
still at 42 of 112, and it now has three ruled-out candidates rather than two:
frequency (§12), the blanket derivation (§10.4, re-priced here), and the
COMMON+PARTICLE conjunction (§13.2, which solves a different problem well).
The next candidate would have to price a *noun*-plus-particle absorption apart
from a noun-plus-noun compound, and this document has no evidence bearing on
that.
