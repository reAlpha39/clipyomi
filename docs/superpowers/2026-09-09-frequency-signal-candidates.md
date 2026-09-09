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
