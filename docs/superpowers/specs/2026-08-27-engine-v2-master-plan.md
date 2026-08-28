# Engine v2 — Master Plan

**Date:** 2026-08-27
**Supersedes (eventually):** [`2026-08-12-jparser-port-design.md`](2026-08-12-jparser-port-design.md)
**Status:** Design draft, not approved, not implemented

---

## 1. Goal

Replace the ta-old-derived parser core with an independently written engine that
is **permissively licensable** and **measurably better** than the port it
replaces.

Two goals, not one, and they constrain each other. The licence goal forbids
reusing ta-old's cost model; the quality goal forbids shipping something worse
than what that cost model produces today. Neither is achievable without the
other being deliberately handled, which is why §6 puts measurement before
rewriting.

### 1.1 Motive

`crates/jparser` is a line-level port of Translation Aggregator's JParser.
Translation Aggregator is GPL v2 with no "or later" grant (§3.1), so ClipYomi is
GPL-2.0-only and cannot be relicensed while that code remains. The practical
costs already visible in the tree:

- `src-tauri/Cargo.toml:36` and `:49` both carry comments taking a dependency
  under MIT specifically because "Apache-2.0 alone is incompatible with this
  project's GPL-2.0-only". That constraint will keep recurring.
- Permissively-licensed contribution and any future dual-licensing are closed.

### 1.2 What this does not buy

**Mac App Store distribution stays closed.** `src-tauri/Cargo.toml:24` enables
`macos-private-api`, which rules out App Store distribution independently of the
licence. Relicensing alone does not open that door, and this plan does not
claim it.

---

## 2. Success criteria

The plan succeeds when all four hold:

1. `crates/jparser` contains no code or data derived from ta-old, and the
   repository can be relicensed under a permissive licence at the author's
   discretion.
2. **Span accuracy** and **entry top-1** (§5.2) on the Phase 0 gold corpus are
   each no lower than the current engine's, measured by the same harness.
3. `くるまではこう` and the wider class of kana-heavy ambiguity it represents
   parse correctly — the case that motivated this work (§4.3).
4. Every deliberate shortcut is recorded, not discovered later.

Criterion 2 is the one that can fail quietly. It is why Phase 0 exists and why
it runs first.

---

## 3. Licence constraints

### 3.1 Upstream is GPL-2.0-only

Translation Aggregator's README states: *"Source in this repository is available
under the GPL v2."* No "or later" grant appears there, and the source files
carry no per-file licence headers (`exe/util/Dictionary.cpp` begins directly at
`#include`). Absent an explicit "or later" statement, GPL version 2 **only** is
the correct conservative reading, and it is what this repository's own headers
already assert.

Relicensing ta-old-derived code would require permission from Translation
Aggregator's copyright holders — multiple contributors across many years.
Treat that as unavailable.

### 3.2 Taint map

Measured by `grep -rc "ta-old"` plus reading the citation at each site. Line
counts from `wc -l`.

| Tier | Files | Lines | Evidence |
| --- | --- | --- | --- |
| **Hard — line-level port** | `segment.rs`, `matcher.rs`, `matcher/verb.rs`, `stem.rs`, `rank.rs`, `record.rs`, `kana.rs`, `romaji.rs`, `conjugation.rs` | ~4,100 | Cite `Dictionary.cpp:1179-1235`, `StringUtil.cpp:23-450`, `Shrink.h:180`. `segment.rs:182`: *"The clause order is load-bearing."* `kana.rs:10`: *"a direct port of `unify()`"* |
| **Hard — copied data** | `assets/conjugations.json` | 32 types / 734 conjugations | `xtask/src/main.rs:15` converts `../ta-old/dictionaries/Conjugations.txt` verbatim |
| **Soft — new code that replaces ta-old** | `jmdict.rs`, `hints.rs`, `index/*`, `lib.rs` | ~3,300 | `jmdict.rs:10`: *"Replaces ta-old's EDICT2 line parser"* — different design, not a port. `index/*` is this project's FST/mmap design |

`hints.rs` is the one borderline entry. Most of it is original Vibrato
integration, but `flags_from_worker` is documented as a port of
`Dictionary.cpp:1115-1126`. Phase 1 dissolves the question rather than arguing
it: tokenizer output stops being an advisory "hint" and becomes a DP constraint
(§4.3), so that function has no successor.
| **Clean** | `crates/jmdict-source`, `crates/vibrato-source` | — | Zero `ta-old` references. Verified by grep |
| **Clean** | `src/**.ts`, `src/**.css`, `src-tauri/src/**` | — | 25 references, all behavioural: *"ta-old's rule"*, *"the same shape"*. Describes conventions, not copied expression |

The clean tiers are roughly half the repository and carry forward untouched.

### 3.3 Permissive replacement sources

**IPADIC is permissive**, and the app already ships it. From the `COPYING` file
published beside the dictionary in `<app_config_dir>/hints/current/`:

> Copyright 2000, 2001, 2002, 2003 Nara Institute of Science and Technology.
> Copyright 2023, LegalOn Technologies, Inc. All Rights Reserved.
> **Use, reproduction, and distribution of this software is permitted.** Any
> copy of this software, whether in its original form or modified, must include
> both the above copyright notice and the following paragraphs.

Notice-retention only. Compatible with any permissive relicence.

IPADIC also carries the grammatical data `conjugations.json` was hand-built to
supply. Every token exposes conjugation class, conjugation form, and dictionary
form:

```
"くる" 動詞,自立,*,*,カ変・クル,基本形,くる,クル,クル
                    │         │      └─ dictionary form
                    │         └─ 活用形  (conjugation form)
                    └─ 活用型  (conjugation class)
```

**Colloquial contractions are lexicalised too**, which is better than this plan
first assumed. Measured against the installed `hints/current/system.dic`:

```
食べちゃった   食べ | ちゃっ 動詞,非自立,五段・ワ行促音便,連用タ接続,ちゃう | た
やっとかなきゃ やっ | とか  動詞,非自立,五段・カ行イ音便,未然形,とく    | なきゃ 助動詞,特殊・ナイ,仮定縮約２,ない
```

〜ちゃう and 〜とく arrive as ordinary 非自立 verbs with a conjugation class, and
なきゃ has its own IPADIC form, 仮定縮約２. The informal register does not need a
separate hand-written contraction layer on top of the table — it needs the same
class-plus-form mapping as everything else. This is evidence for the §7.2 size
estimate, and it was measured rather than assumed.

**What IPADIC does not carry is semantics.** Fields 2–4 are `*` on every
auxiliary; there is no 使役 / 受身 / 丁寧 / 否定 tag anywhere in the feature
string. Those readings are recoverable only from the dictionary-form field
(せる / られる / ます / ん / です / た). §7.3 is therefore a lemma-to-label
mapping, not a tag-to-label mapping, and no choice of dictionary changes that.

Other data already in use:

| Asset | Licence | Effect |
| --- | --- | --- |
| JMdict (EDRDG) | CC BY-SA 4.0 | Data licence. Constrains the *index*, not the code |
| IPADIC via Vibrato | BSD-style, above | Notice retention only |
| Vibrato, Lindera | MIT / MIT OR Apache-2.0 | Free |
| **MeCab** | **GPL *or* LGPL *or* BSD** | Take under BSD. Permissive, and it has the two features Vibrato lacks (§4.1) |
| **UniDic** (NINJAL) | **GPL-2 *or* LGPL-2.1 *or* BSD New** | Take under BSD. Better readings than IPADIC, which matters for furigana |

MeCab's own documentation states the tri-licence directly:

> MeCab はフリーソフトウェアです．GPL(the GNU General Public License),
> LGPL(Lesser GNU General Public License), または BSD ライセンスに従って
> 本ソフトウェアを使用,再配布することができます。

Neither was in this plan's first draft. Both were found by examining
LunaTranslator (§4.4), which uses MeCab with UniDic.

### 3.4 The ecosystem trap

Every mature Japanese deinflector descends from Rikaichan, and the whole lineage
is GPL:

```
Rikaichan ─┬─ Rikaikun ── Rikaichamp ── 10ten-ja-reader   GPL-3.0
           └─ Yomichan ── Yomitan                          GPL-3.0
JParser (Translation Aggregator)                           GPL-2.0-only  ← current
```

Consequences for candidate dependencies:

- **`yomichan_rs`** — GPL-3.0. Moving there makes the licence *more*
  restrictive, not less. Rejected.
- **`deinflector`** — labelled MIT on crates.io, but self-described as *"a 1 to
  1 reimplementation of Yomitan's MultiLanguageTransformer"*. An MIT label over
  GPL-3.0 derivation is a provenance risk this plan will not adopt. Rejected.
- **`bunpo`** — MIT, and the cleanest of the three, but its README credits
  *"10ten Japanese Reader: for their deinflector implementation, which inspired
  `bunpo`"*, and 10ten is GPL-3.0. "Inspired by" is materially weaker than
  "reimplementation of", but it is still downstream of the same lineage. 875
  total downloads, v0.1.7, single maintainer. Not adopted; reconsider only if
  writing the table proves harder than §7.2 estimates.

**Therefore the deinflection table is written here, from IPADIC data and a
grammar reference.** That is the plan's single largest piece of genuinely new
work, and §7.3 governs how it is written.

### 3.5 Clean-room discipline

Binding for all of Phase 1.

- The new conjugation table is derived from IPADIC's conjugation data and a
  published grammar reference. **`assets/conjugations.json` is not open, not
  consulted, and not diffed against while that work is done.** Reading it and
  retyping the contents produces a derivative regardless of the output format.
- The new cost model is designed from the measured corpus, not from ta-old's
  constants. `MATCH_BASE = 10`, `PARTICLE_BONUS = -2` and the rest are ta-old's
  expression and do not carry over — including via "the same numbers happened to
  win".
- ta-old's *observable behaviour* may be described, tested against, and
  deliberately diverged from. Its source and data files may not be transcribed.
  The existing `src/**` comments are on the right side of this line; the
  `crates/jparser/src/**` citations are not.
- Each replaced file's new header states what it was written from. A reviewer
  should be able to establish provenance without asking.

---

## 4. Technical findings

### 4.1 Vibrato has no N-best

Verified against the installed source,
`vibrato-0.5.2/src/tokenizer.rs:2`:

```rust
pub(crate) mod lattice;   // crate-private
```

`Worker`'s public surface is `reset_sentence`, `tokenize`, `num_tokens`,
`token(i)` — one path only. No `nbest` symbol exists anywhere in the crate.

Consequence: **k-best readings cannot come from Vibrato as it ships.** Three
routes exist, and the choice is deferred to Phase 2 (§9.5):

| Route | Cost | Notes |
| --- | --- | --- |
| **Fork Vibrato** | ~100 lines | MIT, so forking is free. The lattice is already written and correct — it is only `pub(crate)`. Cheapest by a wide margin, and upstreamable |
| **Own k-shortest-path** | ~400 lines | Full control over the JMdict-aware cost model. §4.2 |
| **Swap to MeCab** | build system | Native `-N` up to 512 *and* `Lattice::set_boundary_constraint` (0.995+), under BSD (§3.3). Cost is C++ in a Rust/Tauri cross-platform build |

Note that a tokenizer's N-best is not the same object as this tool's k-best: it
enumerates morpheme sequences over IPADIC or UniDic, not segmentations scored
against JMdict. Whichever route is taken, tokenizer N-best is a **candidate
generator** feeding this project's ranking, not the answer itself. That is why
§7 keeps a DP under all three.

### 4.2 The DP is already most of a k-best solver

`segment.rs` is a min-cost path over a `best[]` array. K-shortest-path is a
known, contained extension of exactly that structure. The rewrite is not
"add k-best to a Viterbi tokenizer"; it is "write a fresh DP, shaped so k
follows".

### 4.3 Hints are advisory where they should be decisive

Measured against the real `gen-1` index. Vibrato tokenizes `くるまではこう`
correctly:

```
"くる" 0..2  動詞      "まで" 2..4  助詞,副助詞
"は"   4..5  助詞,係助詞  "こう" 5..7  副詞
```

The current engine discards it, producing `くる / まで / はこう`. Two causes,
both in `segment.rs`:

1. **Hints only mark token interiors.** `hints.rs:47-50`: *"the hint says 'do
   not split inside this,' not 'split here.'"* A single-char token like `は`
   marks nothing, so merging *across* the `は|こう` boundary is free.
2. **Cost is per match**, `MATCH_BASE = 10` (`segment.rs:146`), so fewer words
   win: `はこう` costs 10 against `は`+`こう` at 18.

Flags for the sentence are `bad_start = {1,3,6}`, `bad_end = {0,2,5}` — neither
touches `はこう`'s start (4) or end (6).

This is the class of bug success criterion 3 names. The fix is a cost model in
which tokenizer boundaries are evidence rather than a ±10 nudge — designed in
Phase 1, tuned against Phase 0's corpus.

#### Why two objectives, and what arbitrates between them

Strengthening boundaries raises the obvious question the fix does not answer:
why keep a separate JMdict scoring pass at all, rather than one objective that
scores morphemes and JMdict entries in a single lattice?

Because the two vocabularies are not commensurable. A tokenizer's cost model is
a bigram over context IDs assigned at dictionary build time — IPADIC's, and only
IPADIC's. JMdict entries have no context IDs and no transition statistics; their
POS vocabulary (`n`, `v5k`, `adj-i`, `exp`) has no defined relationship to
IPADIC's. Putting a JMdict compound node into the tokenizer's lattice requires
synthesising a `(left_id, right_id)` pair for every entry, which is a large
hand-authored judgement table with no permissive upstream and no way to validate
it short of the Phase 0 corpus. A single objective does not remove the second
scoring model; it hides it inside a mapping table and makes it harder to measure.

So the engine keeps two objectives. The arbitration rule between them:

- **The tokenizer decides where words may not begin or end.** Its boundaries are
  hard constraints on the DP, not scored terms. This is the §4.3 fix.
- **JMdict decides which span, among those the constraints allow, is a word.**
  Compound preservation lives here: `東京都` is one JMdict headword spanning a
  boundary IPADIC puts between `東京` and `都`, so **constraints forbid splitting
  inside a token, never merging across one.** That asymmetry is deliberate and it
  is what makes both `くるまではこう` and `東京都` come out right.
- **When the tokenizer is wrong, the constraint is wrong, and the DP cannot
  recover.** This is the accepted cost of the asymmetry, and it is the failure
  mode Phase 0 must measure rather than assume: the corpus sampler (§5.1) exists
  to find these cases, and Phase 2's k-best is the escape hatch that surfaces the
  suppressed reading to the user instead of guessing on their behalf.

If Phase 0 shows tokenizer-wrong cases are common enough to hurt span accuracy,
the constraint softens to a large-but-finite penalty and the tuning happens
against the corpus. That is a cost-model parameter, not a redesign.

### 4.4 Prior art — LunaTranslator

The closest actively-maintained peer: a visual-novel translator with Japanese
segmentation and furigana, GPL-3.0, C++ and Python, ~12.9k stars, and commits
landing daily. **GPL-3.0 means no code may be reused here** — this section is
architectural study only, and nothing in it was copied.

**Its entire parser is roughly forty lines.**
`src/NativeImpl/NativeUtils/_/simplemecab.cpp` loads a vendored MeCab 0.996 by
`LoadLibraryW`, constructs a tagger with `-C -r nul -d <dict> -Owakati`, calls
`mecab_sparse_tonode`, walks `node->next`, and hands each `(surface, feature)`
pair to a callback. `libmecab.cmake` fetches and patches MeCab from source. That
is the whole implementation.

**Everything else is delegated.** `src/LunaTranslator/cishu/` holds `mdict.py`,
`jisho.py`, `weblio.py`, `japandict.py`, `jpdb.py`, `mojidict.py`, `youdao.py`,
and `chatgptlike.py` — offline MDX files, online lookup services, and an LLM.
Yomitan runs in an embedded browser for the rest. There is **no deinflector, no
JMdict engine, no sense ranking, and no conjugation chain** anywhere in the
project.

Four things worth carrying:

1. **The tri-licences (§3.3).** MeCab and UniDic were found here. That is this
   section's largest contribution.
2. **A thin tokenizer never invents the §4.3 bug.** LunaTranslator would parse
   `くるまではこう` correctly for the simple reason that it has no JMdict cost
   model able to override the tokenizer. Independent evidence for the direction
   §4.3 already takes: boundaries should be strong.
3. **It uses one-best anyway.** `mecab_sparse_tonode`, not `mecab_nbest_*`,
   despite N-best being available to it. Nobody in this space appears to have
   shipped k-best readings — §9.5 is genuinely open ground, not a solved problem
   with a known answer.
4. **Runtime `LoadLibraryW` sidesteps static-link licence questions**, and is
   Windows-only as written. ClipYomi targets macOS as well, so that particular
   trick does not transfer.

What does **not** transfer is the architecture itself. LunaTranslator's premise
is bring-your-own-dictionary; ClipYomi's is owning the pipeline end to end —
deinflection, sense ranking, and the conjugation chain are the product (§7.3).
Adopting the thin-tokenizer design would delete the reason the app exists.

---

## 5. The harness (Phase 0 output)

### 5.1 Why jparser is not the reference

Clean-room requires an independently designed cost model, so the new engine
**will not** reproduce the old engine's output. "No regressions against jparser"
is therefore not a gate — it contradicts the goal.

jparser's role changes: it is a **disagreement source**, not an oracle. Run two
segmenters over unlabelled text; the sentences where they differ are the
sentences worth a human's labelling time. Agreements are cheap and near-always
right. This is what makes a gold corpus affordable — hundreds of labels, not
thousands.

**In Phase 0 the second segmenter is Vibrato, not a second engine.** The new
engine does not exist yet, so the pair is jparser's spans against Vibrato's token
boundaries. From Phase 1 the pair becomes jparser against its replacement.

Getting this pair right matters more than it looks. The obvious Phase 0 pairing —
jparser with hints against jparser without hints — **cannot find the bug this
project exists to fix.** For `くるまではこう` the flags are `bad_start = {1,3,6}`,
`bad_end = {0,2,5}`; neither touches `はこう` (§4.3), so hints-on and hints-off
produce the *same wrong* parse and the sentence never surfaces. Diffing against
Vibrato surfaces it immediately, because Vibrato is right and jparser is not.

Hints-on and hints-off remain the **baseline pair** (§7, Phase 0) — two numbers
measuring what the hint mechanism is currently worth. They are not the corpus
sampler.

### 5.2 Metrics

Two, because the failures are different and a combined number would hide both.

**Span accuracy.** Fraction of gold spans the parser reproduces exactly (same
start, same length). Measures whether the word is correctly delimited — whether
clicking it selects the right thing.

**Entry top-1.** Of correctly-spanned tokens, the fraction where the gold
JMdict `ent_seq` is ranked first. Measures whether `rank.rs`' successor puts the
right sense in front of the user.

Reported separately, always. A change that improves one and harms the other is a
real finding, not noise.

**Boundary F1 is rejected.** It punishes JMdict compound preservation, which is
a feature: `東京都` is a single JMdict headword, and short-unit academic gold
splits it as `東京`+`都`. Scoring against that standard would penalise the
correct behaviour for this tool.

**Oracle@k** joins in Phase 2 — is the gold parse present anywhere in the top-k
whole-sentence readings? It measures whether k-best earns its complexity, and it
is meaningless before k-best exists.

### 5.3 Gold corpus

Extends `crates/jparser/tests/fixtures/parse_sentences.txt`, which already holds
30 curated sentences with documented per-group coverage intent and is
hand-written (no ta-old provenance).

Genres to cover, because the current corpus is grammar-driven rather than
source-driven: visual-novel dialogue, casual and contracted speech, news
register, OCR noise, and vertical-text artefacts. Target a few hundred labelled
sentences, disagreement-sampled per §5.1.

Labels are `(span, ent_seq)` pairs. Entry IDs are stable and unambiguous;
boundary positions would require first settling a segmentation convention, which
§5.2 explicitly declines to do.

**Open question (§9.1): corpus sourcing.** Visual-novel text is copyrighted.
Options are the author's own reading history kept local and uncommitted, Aozora
Bunko for public-domain prose, or hand-written pastiche. This is unresolved and
gates Phase 0's size, not its design.

### 5.4 Shape

A `jparser-cli` subcommand plus a JSONL corpus file. No framework, no TUI
library, no web UI:

```
jparser-cli diff  <index> <corpus.txt> --hints <dic>     → spans jparser and Vibrato disagree on
jparser-cli label <index> <disagreements.jsonl>          → adjudicate, append labels
jparser-cli score <index> <gold.jsonl> [--hints <dic>]   → span accuracy, entry top-1
```

`--hints` is **required** on `diff` — Vibrato is the second segmenter (§5.1), so
there is nothing to diff against without it. On `score` it is optional, because
that is where the hints-on and hints-off baseline pair is produced.

`diff` compares jparser's span boundaries against Vibrato's token boundaries and
emits the sentences where the two disagree. Boundary-level disagreement is the
sampling signal only; the labels themselves stay `(span, ent_seq)` pairs per
§5.3, and the adjudicator is free to rule that neither segmenter was right.

`label` prints both parses and reads a keystroke. Output is append-only JSONL so
an interrupted session loses nothing.

---

## 6. Phase order

```
Phase 0  Harness + gold corpus + baseline      ← proves everything after it
Phase 1  Clean-room core, k = 1                ← the relicence
Phase 2  k-best readings + UI                  ← the quality win
Phase 3  Extra linguistic layers               ← independent, per-layer licensing
```

Phase 0 is first because it is the only thing that can distinguish "different"
from "better", and every later phase produces a different-by-construction
result.

Phase 2 follows Phase 1 rather than merging into it: the cost model must first
be shown at parity on k=1, or a k-best regression and a cost-model regression
become indistinguishable.

Phases were considered for parallel execution and rejected. Four concurrent
workstreams on top of a clean-room rewrite is five simultaneous risk surfaces;
sequenced, each phase is verified by the one before it.

---

## 7. Phase detail

### Phase 0 — Harness and baseline

**In:** the three CLI subcommands (§5.4); the JSONL label format; genre
extension of the sentence corpus, sampled by jparser-against-Vibrato
disagreement (§5.1); a recorded baseline of both metrics for the current engine,
with and without hints.

**Out:** any change to parsing behaviour. Phase 0 writes no code under
`crates/jparser/src/`.

**Done when:** `jparser-cli score` prints span accuracy and entry top-1 for the
current engine over a committed gold corpus, reproducibly and offline.

That baseline number is the contract every later phase is measured against.

### Phase 1 — Clean-room core

**In:** independently written replacements for the nine hard-taint files
(§3.2); a conjugation table regenerated from IPADIC and a grammar reference; a
new cost model in which Vibrato boundaries are strong evidence (§4.3); removal
of `xtask`'s ta-old converter and of `assets/conjugations.json`; new file
headers stating provenance (§3.5); scrubbing the soft-taint citations in
`index/*`, `jmdict.rs`, and `lib.rs`; dissolving `hints.rs`' ported
`flags_from_worker` into the DP's constraint handling (§3.2 note).

**Out:** k-best (Phase 2). Extra token layers (Phase 3). Any UI change.

**Done when:** no `ta-old` reference remains under `crates/jparser/`; both
Phase 0 metrics are at or above baseline; `くるまではこう` parses as
`くる / まで / は / こう`.

**Size.** An earlier draft of this plan estimated ~1,400 lines on the assumption
that Vibrato's Viterbi would absorb segmentation wholesale. §4.1 killed that
assumption — the DP stays — so the honest figure is larger:

| Replaces | Old | New | Why it moves |
| --- | ---: | ---: | --- |
| `segment.rs` | 778 | ~400 | Simpler cost model; boundaries are constraints rather than a scoring leg |
| `matcher.rs` + `matcher/verb.rs` | 1,107 | ~600 | Still walks the FST over spans; conjugation recursion shrinks where IPADIC supplies the base form |
| `stem.rs` | 457 | ~250 | IPADIC gives dictionary forms directly; stems remain only for headwords it does not know |
| `conjugation.rs` | 463 | ~600 | **Grows.** The §7.3 label mapping lives here |
| `record.rs` | 409 | ~350 | Flag bookkeeping; new flag values, same shape |
| `rank.rs` | 282 | ~250 | Little changes but the constants |
| `kana.rs` + `romaji.rs` | 589 | ~560 | Kana and romaji tables are facts; rewritten from a chart |
| *(new)* auxiliary-chain recombiner | — | ~300 | No predecessor. §7.3 |
| **Total** | **4,085** | **~3,300** | |

Plus the conjugation table itself (§7.2). §7.2 and §7.3 carry the two risks.

**§7.2 — Table size.** ta-old's table holds 32 types and 734 conjugations. The
replacement need not match that count — it must cover what the corpus exercises
plus the standard forms — but the order of magnitude is the planning number.
IPADIC's own conjugation data supplies surface forms mechanically; the
learner-facing English labels do not come from it.

**§7.3 — The label mapping is the product.** IPADIC yields 未然形 / 連用形 /
基本形 — linguistic form names — and splits auxiliaries into separate tokens
(`させ|られ|ませ|ん|でし|た`). JParser's distinguishing feature is the
learner-facing chain, *"Causative-Passive Polite Negative Past"*. Recombining
IPADIC's auxiliary tokens into that chain, and naming the result in English, is
new work with no permissive upstream. It is the single highest-risk item in this
plan and the one most likely to need its own spec.

### Phase 2 — k-best readings

**In:** the §4.1 route decision, taken with Phase 1's measured cost model in
hand; k-best paths over the Phase 1 DP; a whole-sentence reading list with
scores; UI to cycle readings; oracle@k added to `score`.

**Out:** automatic disambiguation beyond ranking. The user picks.

**Done when:** oracle@k=3 exceeds top-1 span accuracy by a margin that justifies
the UI, measured on the Phase 0 corpus. **If it does not, Phase 2 ships
nothing** — that outcome is a valid result, not a failure to be worked around.

### Phase 3 — Extra linguistic layers

Independent of segmentation entirely. Each layer is its own increment with its
own licensing gate, adopted only if its data source is permissive:

| Layer | Source | Licence status |
| --- | --- | --- |
| Frequency rank | corpus-derived | Depends on corpus; unresolved |
| JLPT level | community lists | Unofficial, provenance varies |
| Kanji readings | KANJIDIC (EDRDG) | CC BY-SA, same family as JMdict |
| Pitch accent | — | **Highest risk.** NHK's dictionary is not free, and community accent datasets have murky provenance — precisely the trap this plan exists to escape |

No layer ships without its licence established in writing first.

---

## 8. Non-goals

- Matching ta-old's output. §5.1 — it is excluded by the licence goal.
- Machine translation. Unchanged from the original design.
- Mac App Store distribution. §1.2.
- Replacing the clean tiers of §3.2. `jmdict-source`, `vibrato-source`, and the
  whole TypeScript and Tauri layer carry forward.
- Adopting any GPL-3.0 dependency, or any MIT-labelled crate whose provenance
  traces to one. §3.4.

---

## 9. Open questions

1. **Corpus sourcing (§5.3).** Visual-novel text is copyrighted. Author's own
   reading history kept local, Aozora Bunko, or hand-written pastiche? Gates
   Phase 0's size, not its design.
2. **Target licence.** MIT, Apache-2.0, or MIT OR Apache-2.0? Does not change
   the work, but should be decided before Phase 1 so headers are written once.
3. **`jmdict.rs` and `index/*` disposition (§3.2 soft tier).** Rewriting is
   cleanly safe; scrubbing the citations is cheaper. This plan assumes
   scrubbing. If a reviewer disagrees, Phase 1 grows by ~1,300 lines.
4. **Does k-best justify its UI?** Answered by Phase 2's gate, not in advance.
5. **Which N-best route (§4.1)?** Fork Vibrato, write k-shortest-path, or move
   to MeCab. Deferred to Phase 2 deliberately: Phase 1's measured cost model
   should inform it. Current lean is the Vibrato fork — MIT, ~100 lines,
   upstreamable, and it keeps the build pure Rust.
6. ~~**IPADIC or UniDic (§3.3)?**~~ **ANSWERED — stay on IPADIC.** Measured at
   the Phase 0/1 boundary against the 150-sentence gold corpus, unidic-mecab-2.1.2
   versus the pinned ipadic-mecab-2.7.0:

   | hints | span accuracy | entry top-1 |
   | --- | --- | --- |
   | none | 0.6810 (1473 / 2163) | 0.9166 |
   | IPADIC, field 7 | **0.6963 (1506 / 2163)** | **0.9169** |
   | UniDic, field 9 (its true reading column) | 0.6958 (1505 / 2163) | 0.9151 |
   | UniDic, field 7 (the naive setting) | 0.6958 (1505 / 2163) | 0.9151 |

   UniDic is **one span worse** on span accuracy and slightly worse on entry
   top-1, for 199 MB unpacked against IPADIC's 46 MB, plus a per-dictionary
   reading-column property IPADIC does not need. No measurable benefit; the
   "better readings" argument does not show up in either metric this project
   scores on. Revisit only if furigana quality becomes a measured goal.

   **The silent-degradation hazard is real, and worse in the other direction than
   this question anticipated.** Measured feature layouts: IPADIC puts the surface
   reading at field 7 and gives unknown words a *short* 7-field string, so the
   guard fires via an absent column on 3.7% of tokens. UniDic's field 7 is the
   **lemma** and its surface pronunciation is at 9. So:

   - UniDic read at 7 drops the guard from 13.8% of tokens to 0.5% — degraded,
     silent, and as it turns out immaterial: the score is identical either way.
   - **IPADIC read at 9 disables every hint** (the guard fires on 100% of tokens,
     because IPADIC strings are only 9 fields wide) and scores *exactly* the
     no-hints baseline, 1473 spans, with no error anywhere. That direction costs
     the entire +1.5-point hint benefit silently, and it is the one to fear.

   `hints.rs` now resolves the column at load time with a
   `TA_HINTS_READING_FIELD` override, and carries a test pinning that a column
   the dictionary lacks derives no flags at all rather than partial ones.
7. **Does sudachi.rs change the answer to question 6?** Not considered in this
   plan's first draft. Pure Rust, zero FFI, and its Mode A/B/C multi-granularity
   is aimed at exactly the short-unit-versus-long-unit tension §5.2 describes:
   Mode C keeps `東京都` whole, Mode A splits it. Two caveats keep it out of
   Phase 1. **It is Apache-2.0 with no MIT leg**, and `src-tauri/Cargo.toml`
   records twice that Apache-2.0 alone is incompatible with this repository's
   GPL-2.0-only — so it cannot enter the tree until the relicence has landed,
   which is Phase 1's output, not its input. And Mode C emits *SudachiDict* long
   units, not JMdict headwords, so it narrows the compound-preservation gap
   without closing it; the JMdict-aware scoring layer stays either way (§4.3).
   **Measure it out-of-tree at the Phase 0/1 boundary**, where nothing is
   distributed and the licence question does not arise, and record the number
   here.

---

## 10. Decision log

| Decision | Rationale |
| --- | --- |
| Keep a DP under every engine option | Compound preservation (`東京都`) requires scoring JMdict matches, not tokenizer output. A tokenizer's N-best enumerates morphemes over IPADIC/UniDic, not JMdict segmentations — it is a candidate generator, not the answer (§4.1) |
| Study LunaTranslator, adopt nothing | GPL-3.0, so no reuse. Its thin-tokenizer + delegated-dictionary design would delete the conjugation chain and sense ranking that are ClipYomi's reason to exist (§4.4) |
| Reject cross-engine consensus voting | JMdict longest-match and a deinflector share one dictionary — correlated voters echo rather than vote. Ties need a tiebreak, which rebuilds a cost model. k-best surfaces the same disagreements with a principled ranking |
| Reject boundary F1 | Punishes JMdict compound preservation (§5.2) |
| Reject `bunpo`, `deinflector`, `yomichan_rs` | GPL-3.0 lineage or provenance risk (§3.4) |
| Write the conjugation table rather than port or depend | No permissive upstream exists (§3.4); conjugation rules are facts, and IPADIC supplies the forms permissively (§3.3) |
| Phase 0 before any rewrite | A clean-room rewrite is different-by-construction; without a baseline, "better" is unfalsifiable (§5.1) |
| Reject the unified single-stage lattice (ULLE) | Proposed in [`2026-08-27-engine-v2-ulle-review.md`](2026-08-27-engine-v2-ulle-review.md): merge IPADIC morphemes, JMdict entries and deinflected spans into one lattice scored by the tokenizer's bigram matrix. The matrix is indexed by IPADIC context IDs and JMdict entries have none, so every entry needs a synthesised `(left_id, right_id)` — a large hand-authored table with no permissive upstream, which is the artefact the proposal exists to avoid. Without it the design collapses back into the two-objective engine it replaces. §4.3 states the arbitration rule instead |
| Reject Eppstein's algorithm for k-best | Same review. The lattice is a DAG, so exact k-best is a backward-Viterbi potential plus a priority queue over partial paths — about fifty lines, and what MeCab's N-best already does. Eppstein's persistent 4-ary heap buys asymptotics that never bind at tens of nodes and k≈3, at a much higher chance of being wrong |
| Reject CI provenance auditing that diffs against ta-old | Same review proposed continuous AST comparison against the upstream repository. That requires the protected expression present in CI and in the workspace, which is the one fact §3.5 must be able to deny. If a mechanised check is wanted, fingerprint the upstream once from outside the clean room and compare against the fingerprints |
| Stay on IPADIC; do not adopt UniDic | Measured at the Phase 0/1 boundary on the gold corpus: UniDic scores one span worse and slightly worse on entry top-1, for 199 MB against 46 MB and a per-dictionary reading-column property IPADIC does not need (§9.6) |
| Treat the tokenizer's reading column as a per-dictionary property, not a constant | IPADIC's reading is at field 7 and UniDic's at 9, and reading IPADIC at 9 silently disables every hint and scores exactly the no-hints baseline. A wrong constant is invisible in both metrics (§9.6) |
| Sample the gold corpus against Vibrato, not against hints-on/hints-off | The hints pair produces the same wrong parse on `くるまではこう`, so it cannot surface the bug success criterion 3 names (§5.1). Vibrato disagrees with jparser there, which is exactly the signal wanted |
