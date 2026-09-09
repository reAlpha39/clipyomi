# Phase 3's frequency layer — the cheapest candidate, measured and ruled out

**Date:** 2026-09-09
**Question:** master plan §7's Phase 3 note says a frequency-rank layer
"stopped being a nice-to-have at Phase 1a" and that its licensing is
unresolved. Is the signal already on disk under a licence the plan accepts?
**Answer:** the licence question for IPADIC is settled and favourable, and the
data still does not do the job. Ruled out with evidence rather than left open.
**Probe:** [`crates/jparser/examples/ipadic_cost_probe.rs`](../../crates/jparser/examples/ipadic_cost_probe.rs)

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
