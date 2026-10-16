# Engine v2 — Master Plan

**Date:** 2026-08-27
**Supersedes (eventually):** [`2026-08-12-jparser-port-design.md`](2026-08-12-jparser-port-design.md)
**Status:** **Implemented and merged.** Phases 0 through 4 shipped between
2026-08-28 and 2026-09-16; `develop` carries them as `cb7b994`, "Merge Engine
v2: the JParser rewrite, all four success criteria met". This line read
"Design draft, not approved, not implemented" until 2026-09-16, through every
one of those phases — read §2 for the criteria's status and §5.2's current
baseline block for the figures, and treat any *other* present-tense sentence
here as written in 2026-08-27's tense unless it carries a dated correction.

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

**Every sentence above is the 2026-08-27 position and none of it is true any
more (corrected 2026-09-16).** `crates/jparser` is the clean-room v2 engine;
the nine ported files were deleted at Phase 1b-ii. The root `Cargo.toml` and
`package.json` both declare `MIT OR Apache-2.0`, `LICENSE-MIT` and
`LICENSE-APACHE` are at the repo root, and `git grep "GPL-2.0-only" --
':!docs'` returns nothing. The two `src-tauri/Cargo.toml` comments now read
"MIT OR Apache-2.0, the same pair this project offers" — the quoted string
appears in no tracked file. Dual licensing is not closed; it is the current
state. The motive is kept as written because it is why the work was done.

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

   **That corpus contains no visual-novel dialogue** — 69 pastiche, 41 Aozora
   pre-war literary, 32 Wikinews/Wikipedia, 8 noise, and tier C2 excluded by
   design. This criterion is therefore a floor on literary and news prose, not
   on the register README's first paragraph names as the target. It is still a
   floor and it still binds; §5.2's "what register these numbers speak for"
   block says how to weigh it against corpus evidence when the two disagree.

   **Met 2026-09-16, and it was not met before that day.** Span accuracy had
   cleared the v1 baseline since Phase 1b; **entry top-1 had been sitting
   below it in both hint states for eight phases and no document said so.**
   Against v1 (Phase 1a handoff §2), same corpus, same 2,163 spans, same
   harness:

   | | v1 | v2 before | v2 now (2026-09-30, `a7a9f24`) |
   | --- | --- | --- | --- |
   | span accuracy, hints off | 0.6810 (1473) | 0.7342 (1588) | 0.7351 (1590) |
   | span accuracy, hints on | 0.6963 (1506) | 0.8285 (1792) | 0.7859 (1700) |
   | entry top-1, hints off | 0.9159 | **0.9093** ✗ | **0.9610** ✓ |
   | entry top-1, hints on | 0.9155 | **0.9125** ✗ | **0.9617** ✓ |

   Span accuracy hints-on rose again the same day, 0.8285 → **0.8437 (1825)**
   — a 2026-09-16 reading; later changes have moved it repeatedly and the
   table three lines above carries the current figure, which is the only one
   to quote —
   when `PRONOUNCE_CROSS` stopped charging for a boundary the tokenizer put
   *inside* an inflection. That change on its own pushed entry top-1 back
   under the floor — 30 newly-placed spans are harder and 15 resolved wrong —
   and the sense-count tiebreak is what recovered it, +41.

   The gap was pure ranking, not coverage: of 118 hints-on misses, 116 had the
   gold entry in the list and merely out-ranked, 61 of them at rank 2, and
   `entry_id` — a tiebreak §5.2 itself says carries no quality judgement — was
   deciding them. `assemble::order` gained two clauses, a `COUNTER` promoted
   only when the character before the span is a digit and a `PARTICLE`
   promoted among equals; both are facts JMdict or the text supplies, and
   neither costs a single span in either hint state. `examples/order_sweep.rs`
   is the measurement, and `order` runs after the DP, so nothing else moved.

   **Read "span accuracy" as §5.2 defines it — strict.** `span tolerant` is a
   later diagnostic and *is* below v1 (0.8465 → 0.8206 hints off), because v1
   earned it with 167 dictionary merges against v2's 70 and tolerant credits
   exactly the particle-absorbing merges criterion 3's fixture exists to kill.
   §5.2 already records that metric as over-forgiving by about 1.3 points for
   this reason. It is not this criterion's metric and is not a regression.
3. `くるまではこう` and the wider class of kana-heavy ambiguity it represents
   parse correctly — the case that motivated this work (§4.3).

   **Met, and the "wider class" half is now a live gate (2026-09-09).** The
   named sentence has been pinned since Phase 1b by
   `kurumadehakou_parses_as_four_words`. The class around it is
   `tests/fixtures/spurious_merges.txt`, 26 rows of handoff §9.5's
   particle-absorbing merges, and its test was **ignored** from Task 16b until
   now on 4 of 26 failures. All four were properties of the test's subject
   rather than of the engine: it measured each surface in isolation, and given
   two bare characters Vibrato calls `はこ`, `うん` and `んで` one token each,
   so `cost.rs`'s crossing count found no interior boundary and neither
   `CROSS` nor `PRONOUNCE_CROSS` could fire at any magnitude — which is why
   every trial from 1 to 25 measured those rows identically. The fourth,
   `いんじゃない`, needed `じゃ`, which is four real JMdict entries absent from
   the small curated subset.

   Each row now names a committed gold sentence that contains its surface and
   whose adjudication puts a boundary strictly inside it, and the index is
   built from `gold_jmdict_subset.xml` so no row can pass because nothing
   matched. **26 of 26 pass and nothing is absorbed**; the suite has no
   ignored test. Two rows (`はいた`, `タッとい`) cut inside the surface at a
   position gold does not agree with — ordinary strict-span misses already
   inside the strict figure, deliberately not this test's subject, and
   recorded in it.
   This matters because §5.2 records that neither span metric can catch this
   class: strict counts it as an ordinary miss and tolerant actively *credits*
   it, since the merge's endpoints are both gold boundaries. The fixture is
   the only guard there is, and it was not running.
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
the correct conservative reading. **The clause that used to follow — "and it
is what this repository's own headers already assert" — was true in 2026-08-27
and is false now (corrected 2026-09-16):** every source header in the tree
asserts `SPDX-License-Identifier: MIT OR Apache-2.0`. The paragraph is about
*Translation Aggregator's* licence, which is unchanged; only the claim about
this repository's headers had to go.

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
(§4.3), so that function has no successor. **Dissolved at Phase 1b-ii: deleted
with `BoundaryFlags`, no successor.**
| **Clean** | `crates/jmdict-source`, `crates/vibrato-source` | — | Zero `ta-old` references, verified by grep — but that grep tested the wrong string; see the correction below |
| **Clean** | `src/**.ts`, `src/**.css`, `src-tauri/src/**` | — | 25 references, all behavioural: *"ta-old's rule"*, *"the same shape"*. Describes conventions, not copied expression. **Now zero** — `git grep "ta-old" -- src src-tauri` is empty (2026-09-16); the "Discharged at Phase 1b-ii" note below covers only the header strings, not these |

The clean tiers are roughly half the repository and carry forward untouched.

**Discharged at Phase 1b-ii.** The hard tier is deleted; the soft tier's
citations are scrubbed (§9.3's assumption held); the clean tier's header
strings are gone.

**Correction, made at Phase 1a (design §11.1).** The grep behind the first
"Clean" row above tested only `ta-old`. Ten files across `crates/jmdict-source`
and `crates/vibrato-source` carry the header `Japanese text parser ported from
Translation Aggregator` — a string containing no `ta-old` — so the original
grep passed silently over them. The tier itself does not change: a boilerplate
provenance header is not protected expression, and both crates remain clean in
substance. But the evidence claim as originally written was wrong, and Phase
1's done-when (§7) is now written to sweep both strings, over this pair of
crates as well as `crates/jparser/` and `xtask/`.

**Second correction, made at the Phase 1a whole-branch review.** The same
header sits on 11 files under `src-tauri/` (10 under `src/` plus `build.rs`)
and on `tools/extract_jmdict_subset.py` — the second "Clean" row above lists
`src-tauri/src/**`'s 25 behavioural `ta-old` mentions but never checked it, or
`tools/`, for the header string at all. A phase that satisfies only the
four-crate wording could ship an `MIT OR Apache-2.0` `Cargo.toml` (§9 open
question 2) while `src-tauri/build.rs` still opens with a GPL-2.0 notice
declaring itself "ported from Translation Aggregator." Both directories are
folded into Phase 1's done-when below, worded as a tree-wide sweep this time
so a directory list can't fall behind a third time.

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
`token(i)` — one path only. (The enumeration is incomplete: `token_iter`,
`init_connid_counter`, `update_connid_counts` and `compute_connid_probs` are
public too. None is an N-best path, so the conclusion stands and only the list
was short.) No `nbest` symbol exists anywhere in the crate.

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

**Route taken at Phase 2: own k-shortest-path over the Phase 1 DP.** The
other two routes enumerate tokenizer morphemes over IPADIC, and since Phase
1a's design §6.1 hints supply boundaries only, there is no tokenizer lattice
in the engine to take an N-best from. The lattice module doc had promised the
shape — swap the single winner for a bounded list — and that is what landed
(Phase 2 design §2.3, §3).

### 4.2 The DP is already most of a k-best solver

`segment.rs` is a min-cost path over a `best[]` array. **(v1's file, deleted
at Phase 1b-ii; `engine::lattice` plus `engine::cost` are its successors.
Unlike §3.2 and §4.3 this sentence carried no marker until 2026-09-16.)** K-shortest-path is a
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

**Delivered at Phase 1a, and the advisory mechanism is gone at 1b-ii.** Cost
accrues per character and the tokenizer's cuts reach the DP as the
`engine::Boundaries` trait — a question about where a token ends, not a
per-position "do not split inside this" nudge. §3.2's `flags_from_worker`, the
one borderline entry in the taint map, was deleted along with `BoundaryFlags`
at Phase 1b-ii with no successor; `hints::VibratoTokenizer::boundaries` is
what remains.

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

  The escape hatch has a measured limit: hints on, 63 gold spans in the Phase
  0 corpus are suppressed by `is_allowed` itself — they exist as candidates
  and no path may use them — and k-best over the constrained lattice reaches
  none of them. Phase 2 kept alternates on the constrained lattice and
  recorded the 63 as the constraint's price (Phase 2 design §2.3).

  **Phase 4 changed this and both paragraphs above are now too strong
  (2026-09-16).** `engine::relax::Relaxed` reopens the positions strictly
  inside a hint token that no candidate can cover *and* that carries a
  hiragana character, and `engine::parse` wires it in whenever `bounds` is
  `Some` — so where the tokenizer and the dictionary disagree about whether a
  unit exists at all, the DP now *can* recover. It gained 29 gold spans
  hints-on against 5 lost. The paragraph's own escape route, "the constraint
  softens to a large-but-finite penalty", is explicitly **not** what shipped:
  Phase 4 design §6 records why a price cannot express structural absence, and
  the veto still holds everywhere it held before.

  **The 63-span figure has not been re-measured since Phase 2 and should not
  be quoted as current.** Phase 4 neither restated it nor invalidated it on
  purpose; whatever the constraint now suppresses, 63 is a 2026-09-02 number
  about a rule that has since gained an escape.

If Phase 0 shows tokenizer-wrong cases are common enough to hurt span accuracy,
the constraint softens to a large-but-finite penalty and the tuning happens
against the corpus. That is a cost-model parameter, not a redesign.

#### What Phase 1a found and measured

**Hard constraints alone do not fix `くるまではこう` (design §11.2, §7.1).**
The arbitration rule above forbids splitting *inside* a token but never forbids
merging *across* one — that asymmetry is what lets `東京都` survive a tokenizer
that cuts it as `東京 | 都`. But Vibrato tokenizes the motivating sentence as
`は` at 4..5 and `こう` at 5..7, so `はこう` at 4..7 merges *across* a boundary
and is legal under any strength of interior constraint. It has to lose on
cost, not be forbidden by a constraint. Removing the fewer-words-wins bias
itself is therefore not an optional second half of the fix: with a per-match
constant retained, success criterion 3 fails with every constraint in place.

**The asymmetry's price, measured (design §11.3, §7.5).** From the committed
diagnostic (`examples/boundary_diag.rs`): of 2,013 interior gold boundaries,
jparser holds 1,734 and Vibrato 1,954, with 266 held by Vibrato alone. 1,688
are held by both, and **46 are held by jparser alone** — gold boundaries the
current engine finds and Vibrato does not, which is exactly what a hard
interior constraint forbids. At an absolute ceiling that is worth about two
points of strict span accuracy, and less in practice, since a gold span needs
both of its edges. This is the number the softening clause above should be
judged against, not a guess.

**Phase 1a measured the constraint's actual cost and declined to soften it
(Phase 1a handoff, 2026-09-01, §5, §8).** Over every gold span with a v2
dictionary candidate at its exact `(start, len)`, hints on: of 2,163 gold
spans, 1,561 have such a candidate, and **64 of those (4.1% of the 1,561,
2.96% of all gold spans) are hard-forbidden by the interior constraint even
though the candidate exists** — a rejection, not a cost comparison the DP
lost. This is a different unit from the 46-boundary figure above (boundary
positions versus dictionary-candidate spans) and is not directly comparable to
it, but it is real and larger in raw count. Tier breakdown: 11 tier A
(literary prose), 15 tier B (news register), 38 tier C1 (hand-written
pastiche) — so 26 of the 64, about 41%, fall in the ordinary-prose tiers
rather than the mimetic/onomatopoeia class the corpus's weakest tokenizer
signal would predict.

Judgment: the softening clause's own precondition — the constraint costing
enough to hurt span accuracy — is not met. Hints-on strict span accuracy
passed the v1 baseline by +0.1068 (231 spans), an order of magnitude above
the 64-span ceiling even if every forbidden span flipped to a win; and
turning the constraint from a hard filter into a scored penalty is a genuine
architectural change — it touches the existence/pricing split the cost
model's own module documentation states, and three dedicated tests assert the
current hard-forbid behaviour directly — not a constant to tune and revert.
Not attempted in Phase 1a; recorded here, with its tier breakdown, so the
next phase can decide with the number in hand rather than a guess.

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

Two, because the failures are different and a combined number would hide
both. Every metric here is measured at corpus sentence lengths, and nothing
in the harness measures a clipboard-length input — which is why a
superlinearity that made the app's own 10,000-character ceiling unusable
survived four phases (Phase 2c). Latency at that ceiling is now recorded
outside the harness, at the `k` the app actually runs: 0.897 s hints off and
0.879 s hints on for 9,987 characters at `--readings 3`, against 0.501 s at
k = 1, so the k-best pool costs ×1.79 and not the ×12 its size suggests
(Phase 2c handoff §5).

**A timing figure inside the harness now exists: `jparser-cli bench`
(`src/bin/jparser-cli/bench.rs`).** It is deliberately not a timer on `score`
— timing the 150-sentence corpus would reproduce the exact blindness above,
since Phase 2c's defect was invisible at corpus sentence lengths and fatal at
paste lengths. Instead it parses a paste built by cycling the gold sentences
(deterministic from the committed fixture, so no paste file has to be kept) at
two lengths: 1,000 characters and the app's own 10,000-character ceiling.

**The recorded cell is `scaling`, not the milliseconds** — the time ratio
divided by the length ratio, which is 1.00 for linear behaviour on any
machine, while absolute times are not comparable across hardware or across
indexes. Phase 2c's once-per-position index walk would have read near 10.

Baseline, gold index rebuilt from `gold_jmdict_subset.xml`, 5 runs per point,
median (the absolute figures are gold-index numbers and are **not** comparable
to the full-index paragraph above; the scaling column is what carries across):

| | k | 1,005 chars | 10,040 chars | scaling |
| --- | --- | --- | --- | --- |
| hints on | 1 | 24 ms | 263 ms | **1.09x** |
| hints off | 1 | 26 ms | 260 ms | **1.00x** |
| hints on | 3 | 46 ms | 518 ms | **1.12x** |
| hints off | 3 | 58 ms | 614 ms | **1.05x** |

Linear in all four states, which is what Phase 2c's fix claimed and nothing in
the harness could previously check. The scaling cells are ratios of timing
medians and are read to about ±0.1x — Phase 3a's untouched engine read 1.04x
where 1.05x is recorded — while the accuracy cells are read to the digit. The
k = 3 pool costs ×1.97 over k = 1
hints-on here against the full index's ×1.79, which is the instrument agreeing
with the out-of-harness measurement it was built to replace. `bench` reads the
index and the tokenizer and writes nothing; `score`'s output is untouched, so
the eight recorded baseline cells and the test that pins them are unaffected.

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

**Span accuracy inherited that defect, and a third metric answers it.** The
rejection above was written about boundary F1 and stopped there. But the gold
corpus of §5.3 is disagreement-sampled against a tokenizer, so it was
adjudicated at the tokenizer's short-unit granularity — and strict span
accuracy against a short-unit corpus penalises compound preservation exactly
as boundary F1 would. Measured: it costs **16.5 points**.

**Neither span metric can reward compound preservation, and a third
answers that.** Both are scored against a corpus adjudicated at short-unit
granularity, so strict *punishes* keeping `東京都` whole and tolerant only
declines to punish it. Measured 2026-09-09: a signal giving an
inverted-priority compound parity with its parts costs about 6.9 points
strict for at most 0.8 points tolerant, so the Phase 3 frequency layer below
could never have passed a gate built on this pair. **Compound accuracy**, the
third metric, is the fraction of hand-adjudicated compound decisions the
parse handles as adjudicated — 194 windows over this corpus, each classed
`lexical` (should merge), `transparent` or `false-match` (should split), or
`ambiguous` (excluded). It needs its own file (`score --compounds`), and it is
reported separately from the two below like everything else here. See
`docs/superpowers/2026-09-09-frequency-signal-candidates.md` §10-§11 and
`src/bin/jparser-cli/compounds.rs`.

**THE CURRENT BASELINE, and the one place to read it from.** Every table below
this point in §5.2 and in §7 is a dated record of what a given phase measured,
and none of them is rewritten when the engine moves — so this block is the
authoritative current figure and those are history. Gold index rebuilt from
`gold_jmdict_subset.xml`, 150 sentences, 2,163 gold spans.

| | strict | tolerant | entry top-1 | compounds | coarsening |
| --- | --- | --- | --- | --- | --- |
| hints off | 0.7351 (1590) | 0.8262 (1787) | 0.9610 (1184 / 1232) | 0.6344 (118 / 186) | 197 spans in 96 merges |
| hints on | **0.7859 (1700)** | **0.9034 (1954)** | **0.9617 (1205 / 1253)** | **0.8172 (152 / 186)** | 254 spans in 118 merges |

Compounds by class, hints on: `lexical` 82 / 112, `false-match` 30 / 32,
`transparent` 40 / 42, 8 `ambiguous` excluded. Hints off: `lexical` 56 / 112,
`false-match` 23 / 32, `transparent` 39 / 42.

**Refreshed 2026-09-23 at `5194b8a`; the block had stood unchanged since
2026-09-18** while the cells moved under it — it read 1698 / 1948 / 0.9528
hints on and 1593 / 1786 / 0.9506 hints off. Each move is recorded in the
handoff of the session that made it, dated 2026-09-18 to 2026-09-23 in
`docs/superpowers/`; the last three are the twelve-corpora table fixes
(1696 / 1950 / 0.9528 -> 1698 / 1952 / 0.9584 hints on, 1588 / 1785 /
0.9504 -> 1589 / 1786 / 0.9569 off), the godan potential (+1 strict,
tolerant and entry hit hints on) and the empty-stem exemption on
`PRONOUNCE_CROSS` (+1 again, hints on only;
[`2026-09-23-empty-stem-and-potential-stem-handoff.md`](../2026-09-23-empty-stem-and-potential-stem-handoff.md)).
None of the three moves a compound class line.

**Refreshed again 2026-09-26 at `58a8a80`**, for one move: `FUNCTION_STACK`
no longer charges a `uk` word of three kana or more hints off, and gold gains
`やがて` (1589 / 1786 / 0.9569 -> 1590 / 1787 / 0.9570 off; hints on
unchanged, by construction). The display clause before ます committed with it
moves no cell. See
[`2026-09-26-reported-sentence-handoff.md`](../2026-09-26-reported-sentence-handoff.md).

**Refreshed again 2026-09-30 at `a7a9f24`**, for entry top-1 alone: 1201 ->
1205 hints on and 1179 -> 1184 hints off, all from counting a noun's `uk`
only on its first sense (`9bd5d33`). The two other order clauses committed
with it -- a verb left on an unfinished form, and サ変's し under れる --
move no cell, and no span moves in either state. See
[`2026-09-30-first-gloss-order-handoff.md`](../2026-09-30-first-gloss-order-handoff.md).

Latency at the app's ceiling (`jparser-cli bench`, gold index, k = 3, hints
on, 10,040 characters), interleaved A/B across the three changes of
2026-09-23: 644 / 687 / 624 ms before against 619 / 672 / 579 after the first,
565 / 544 / 540 / 573 / 551 against 571 / 562 / 564 / 577 / 561 for the
second, 579 / 562 / 568 against 563 / 549 / 547 for the third; scaling
0.91-1.07x throughout. Two builds that differ by one suffix check read 3 %
apart, which is this machine's layout noise; every figure is a reading from
the moment it was taken.

**WHAT REGISTER THESE NUMBERS SPEAK FOR, added 2026-09-18 because nothing said
it and a refusal was argued on it.** The 150 sentences behind every cell above
are, by `source` tier: **69 hand-written pastiche (C1), 41 Aozora Bunko pre-war
literary prose (A), 32 ja.Wikinews / ja.Wikipedia (B), 8 derived noise (D)**.
Visual-novel dialogue is **0 of 150, by design** —
`tests/fixtures/gold_sources.md` records that tier C2, real `vn_data` lines, "is
never committed and has no rows here", because the text is copyrighted and the
committed corpus is the reproducible contract.

That exclusion is a licensing decision and it is the right one. Naming its
*measurement* consequence is what was missing: README's first paragraph says
this program is "for visual novels, games, and immersion reading", so **every
gate in this section is measured on text the product does not primarily
target**, and the corpora the disagreement probe runs on are visual-novel
dialogue and nothing else. Neither side is "the truth". The rule, when they
disagree, is to **say which register each number speaks for** rather than
quoting whichever agrees with the change under test — and in particular a
`strict` fall on literary and news prose is weaker evidence against a change
than the same fall would be on the target register. This is a weighting, not
a licence to skip a gate; criterion 3's fixture and the compound class lines
stay absolute.

Worked example, [`2026-09-18-three-corpus-adjudication-handoff.md`](../2026-09-18-three-corpus-adjudication-handoff.md)
§8: ungating `LEN_GAIN` hints-off reads `compounds` 118 → 158 with `lexical`
56 → 94 and no split class regressing, against `strict` 1604 → 1525. It was
refused on independent VN-corpus evidence — the spans it invents sever
morphology, `話しかける` → `話しかけ` + `る` — and that refusal stands. But the
strict fall was quoted beside it as though the two carried equal weight, and
they do not.

**The fix for this is `tests/fixtures/gold_local.jsonl`**, which `.gitignore`
already reserves: "the local gold corpus over `vn_data` lines, scored by the
same `jparser-cli score` as the committed corpus but never committed (Phase 0
design §3.2)". Phase 0 built the mechanism and nothing has used it. A few
hundred hand-adjudicated VN-register spans would make a VN-class change
*approvable* rather than only detectable, which is the constraint that decided
the largest available change of 2026-09-18. It is hand adjudication, so it is
a phase of its own.

**Moved 2026-09-18 by `table.rs`'s `ていく`/`でいく` rows** (a dated record, like the tables below; the block above is current), the
directional auxiliary — the most frequent member of the 〜て family still
split in *both* hint states after the `ている` work. `歩いて` | `いく` and
`引きずって` | `いった` now read as one segment, `歩く` Directional and
`引きずる` Directional Past. Found by running the disagreement probe over two
corpora it had never seen: 1,189 lines of 75,767 carry one, a rate of 9.6 to
17.3 per **thousand** in each of the three separately, so it is a rate rather
than one writer's habit.

**It is not the last of them.** 〜てくる, 〜ておく and 〜てみる have no rows
either and still read as two segments — `歩いて` | `くる`, `買って` | `おく`,
`歩いて` | `みる` — and `持ってくる` merges only because JMdict carries it as a
headword. 〜てくる is the same directional concept in the opposite direction
and wants a label word of its own, not this one; `conjugation_labels.txt`
records why.

**Shipped in two commits, and the second is the one to read.** The first gave
`ていく` a single attachment site, 連用タ接続, which reaches 五段 onbin stems and
**leaves every 一段, サ変・スル and 特殊・デス verb behind** — `食べていく`,
`落ちていった`, `溶けていく`, `見ていく` all still split. That is the rule
`ATTACHES_TO`'s own doc already states for た/て: IPADIC names 連用タ接続 only
where a class distinguishes it from plain 連用形, so either row alone leaves
half the classes unreachable. `でいく` keeps one row, because で follows an
onbin stem and nothing else.

Hints on: `strict` 1714 → **1698**, `tolerant` **1948** unmoved, `entry top-1`
0.9534 → 0.9528, `compounds` 152 with all three class lines identical,
coarsening 234 spans in 108 merges → 250 in 116. Hints off: `strict` 1610 →
1593, `tolerant` 1791 → 1786, `entry top-1` 0.9513 → 0.9506, `compounds` 118
with all three class lines identical, coarsening 181 in 88 → 193 in 94.

**The strict fall is decomposed, not assumed.** Hints on, 8 gold spans are
lost and 0 gained, and **all 8 have both endpoints on gold boundaries** — they
are four intended merges and nothing else, `折れて`+`いった`, `溶けて`+`いく`
among them. Zero damage. Hints off, 11 are lost and 0 gained: 8 are the same
coarsening and **3 are real damage**, all in one sentence — `出ていったようだ`
reads `出ていっ` | `たよう` | `だ`, where `たよう` is 態様 "condition; manner"
and the auxiliary's own onbin stem has been taken for a word.

**That last one has a named cause and a priced fix, and the fix is refused
here.** `BOUND_FORMS` exists so that a chain *ending* at 連用タ接続 is not a
word — `優しいっ` must not read `優し` + `いっ` — but it is consulted only for a
headword's own inflection, never for an auxiliary's. Extending it to
auxiliaries was built and does fix it: `出ていったようだ` comes back whole,
hints-off `strict` 1593 → 1594, `tolerant` 1786 → **1789**, `compounds`
118 → **119** with `lexical` 56 → 57, and hints on is identical to the digit.
It is refused because it deletes **nine committed rows** from
`conjugation_labels.txt` — `食べなかっ`, `食べたかっ`, `読んだっ`, `買っちゃっ`,
`読んじゃっ`, `買っちまっ`, `読んじまっ` and this change's own two — and that
fixture declares itself authoritative over the code. Seven of the nine predate
this work, so the change is a fixture revision needing its own adjudication,
not a row to slip in beside a table addition. `BOUND_FORMS`'s own comment set
that precedent: each bound form "deletes candidates of its own and wants its
own score run".

**No `CHAIN_BLOCK` or `AUX_FORM_BLOCK` rows accompany these**, and that is a
statement about the class rather than a measurement. Both tables exist because
一段's 未然形 and 連用形 have *empty* suffixes, so `てる` carries a bare `て` on
which a one-kana auxiliary stacks and collides with a sentence-final particle.
五段・カ行促音便 has no empty form: its 未然形 is `ていか`, so the surfaces an
auxiliary could reach are `ていかね` and `ていかな`, which are `持っていかねば`
and `持っていかなきゃ`. The chain count is 73,731 → **82,573**.

Latency unmoved, interleaved A/B over three rounds each way: hints on, 10,040
characters read 567 / 543 / 544 ms before against 542 / 545 / 537 after, and
hints off 618 / 618 / 619 against 620 / 599 / 606. The scaling cell reads
0.99–1.02x before against 1.05–1.06x after in the hints-on state, and that
rise is the 1,005-character *denominator* getting faster (57 / 54 / 53 ms
against 51 / 51 / 51), not the paste getting slower.

**Moved 2026-09-18 by `table.rs`'s `ている`/`でいる` rows**, the
uncontracted form of the same auxiliary, and it is the largest granularity
change this engine has made. `待っていた` read `待って` | `い` | `た` with `い`
glossed "the Boar (twelfth sign of the Chinese zodiac)" — 218 corpus regions,
the largest single count the disagreement probe found — and now reads as one
segment, `待つ` Progressive Past. So do `思っている`, `食べている`,
`している`, `遊んでいる`, `来ていない` and `遅れています`.

**〜ている is an auxiliary and every other auxiliary this engine knows already
merges** — 〜た, 〜ない, 〜ます, 〜たい, 〜ちゃう, and as of the rows below
〜てる. Splitting it was not a policy; it was a missing row, and keeping it
split while the contraction merged would have made the same construction two
granularities apart by which spelling the speaker chose.

Hints on: `strict` 1746 → 1714 and `tolerant` 1935 → **1948**, `entry top-1`
0.9507 → **0.9534**, `compounds` 152 with all three class lines identical.
Hints off: `strict` 1634 → 1610, `tolerant` 1778 → **1791**, `entry top-1`
0.9506 → **0.9513**, `compounds` 113 → **118** with `lexical` 52 → 56.
Coarsening 189 spans in 86 merges → 234 in 108.

**The strict fall is decomposed, not assumed.** Of the 25 new spans on the
gold corpus hints on, **23 have both endpoints on gold boundaries** — the
coarsening this section prices at about 16.5 points — and 2 do not:
`当惑|していた` where gold cuts `当惑して|いた`, and `行き|あっていました` where gold
cuts `行きあって|いました`. In both the offending left cut is **pre-existing** —
the engine already read `当惑|して|い|た` and `行き|あって|い|ました` — so the
merge coarsens the auxiliary tail of an already-misplaced boundary rather
than inventing a word across a good one. `tolerant` rising in **both** hint
states is the same finding from the other side.

**Over 8,334 lines of dialogue**: 652 sentences change, 754 new spans, **336
stray `い` segments and 239 stray `た` segments disappear**, and **zero** new
spans end in a sentence-final particle — the failure mode the `てる` rows
below had to be gated against does not arise here.

**The one gate row that moved, and why it was re-pointed rather than
weakened.** `spurious_merges.txt`'s `してい` row asserts a cut inside that
surface in `b:wn25517-3`; with `ている` the sentence reads `予定` | `に` |
`している`, so there is none. The row exists to stop 1308000 (指定) or the
kana-unified 子弟/師弟 taking that span, and neither does: the `に` is its own
segment, nothing is absorbed, and the covering entry is `する` Progressive.
Verified before touching it — across eight readings of that sentence 指定 never
takes the span, and `してい` parsed alone now leads with `する` Progressive where
it used to lead with 子弟 "children; sons". The test now accepts a **strictly
longer** matched word covering the surface **only when its top entry is not
the row's own `ent_seq`** — which is what the row was always about — and a
segment sitting exactly on the surface still fails whatever it matched.
Column 2 of that fixture was read by nothing until now.

**Three later changes on 2026-09-18 moved no cell at all and are recorded in
the handoff's §9 and §10**: the gate acceptance above tightened from
`s.start <= at` to `s.start == at`, which is what restores the `uk`-flag
refusal to a running check; `table.rs`'s `CHAIN_BLOCK`, which stops `ぬ` at
仮定形 and `ない` at ガル接続 stacking on the `てる` family so `してね` is a
request again rather than `する` Progressive Negative Conditional; and
`cost.rs`'s `KATAKANA_MIN_PIECE`, the mirror of `relax::MIN_PIECE`, which
stops `PRONOUNCE_CROSS` charging for a tokenizer cut inside a katakana run
that leaves a piece shorter than three — `センパイ` read `セン` "line" +
`パイ` "pie" in 70 corpus lines and correctly in 5, and the only thing that
varied was whether IPADIC called it one token.

**Set earlier the same day by `table.rs`'s `てる`/`でる` rows**, the ている /
でいる contraction: `思ってる` read `思` | `っ` | `てる` with `てる` glossed
照る "to shine", in 122 region surfaces and 300 corpus sentences, and now
reads as one segment labelled `思う` Progressive. The rows are the same shape
as the ちゃう family's — a contraction IPADIC tokenizes on its own. JMdict
does carry `てる`/`でる` as headwords, but reaching that entry needs a separate
span and the only span that could precede one is the one `is_allowed` refuses,
so the row is what puts the contraction inside a span rather than beside it.
They sit **below** the `て`/`で` rows because 一段's
empty-suffix 未然形 and 連用形 make `てる`'s own surfaces include a bare `て`;
inserted first they relabelled every plain te-form "Progressive", which
`parse_sentences.txt`'s snapshot caught.

**The rows also needed `AUX_FORM_BLOCK`, and that is where the real risk
was.** 一段 gives both 命令ｙｏ and 未然ウ接続 the suffix よ, so `てる` and
`でる` carried the surfaces `てよ`/`でよ` — which is the sentence-final
particle. Ungated, 22 occurrences over 8,334 lines absorbed it (`待ってよ`,
`返してよ`, `呼んでよ`), and five were far worse: `なく` is 五段・カ行イ音便,
so `な`+`い` is a 連用タ接続 site and `忘れないでよね` parsed as `忘れ` +
`ないでよ` + `ね`, the negation destroyed and the span glossed `なく` "to
cry". The contraction's imperative is `待ってろ`, never `待ってよ`, so the two
よ-forms are blocked; the cost is the corpus's single `てよう`.

Cells at the time: `strict` 1745 → 1746 hints on and 1633 → 1634 off,
`tolerant` 1932 → 1935 and 1775 → 1778, `compounds` and every class line
identical in both states. The reachable-chain count went 50,419 → 60,926 here
and then 72,049 for the `ている` rows above; `bench` says the runtime cost is
nothing, as it did for ちゃう — 1.04x hints on and 0.98x off after both, with
interleaved spreads overlapping on a loaded machine.

**What it still costs, measured before and after over the same 8,334
lines**: 1,225 sentences change and 1,358 new spans appear, of which **34
absorb a sentence-final ね or な** — `してね` reads `する` Progressive Negative
where the text is a request, because 特殊・ナイ's 体言接続特殊２ *is* `ね` and
`してねえ` really is that contraction — and **3 eat a kanji adverb**
(`決してない` as `決す` Progressive Negative, hints-on only). 37 against
~1,320 correct contractions. The fix for the first is a chain-level block
(`ない` may not attach to `てる` at 体言接続特殊２), which the current
`(lemma, form)` shape cannot express; it is named, not built.

**Entry top-1's two cells were set by one clause in
`assemble::order`**: the `PRONOUNCE` demotion no longer applies to an entry
JMdict tags `uk`. A kana spelling of a kanji word is a fallback and deserves
the demotion; a `uk` entry — 9,580 of JMdict's 218,449 — is one whose kana
*is* the normal spelling, and demoting it is simply wrong. `待っていた` is
where it was found: `いる` is `uk`, and its `いた` lost to `い`, "the Boar
(twelfth sign of the Chinese zodiac)". **0.9400 → 0.9500 hints on and 0.9358 →
0.9507 hints off, +13 and +19 hits on unchanged denominators, and every other
cell in this table was identical to the digit at the time** — `order` runs
after the DP, so no boundary can move; the `てる` rows above have since moved
`tolerant` and one hints-off span. The snapshot moved in nine places and every one is a
gloss list reordering, `時間がない` now leading with 無い "nonexistent" rather
than the negative auxiliary.

**It is deliberately not a `WordFlags` bit.** That was built and measured, and
it carries `uk` into `cost::PRONOUNCE_CROSS` as well: entry top-1 rises about
as far, `lexical` goes 82 → 87 and `strict` 1745 → 1737, but `ことになって`
swallows its `に` and the 26-row `spurious_merges.txt` gate goes to 25, because
`なる` is `uk` too. Criterion 3 refuses it. The same refusal covers widening
`PRONOUNCE_CROSS`'s chain exemption to the chain's *start*, which fixes
`待っていた`'s span as well as its gloss and breaks `はこう` — criterion 3's own
named sentence — for `false-match` 30 → 26 and `transparent` 40 → 37.

**The span half of that defect was still open when this paragraph was
written and is closed by the `ている` rows above**, later the same day.
`待っていた` is one segment. `いた` standing outside the auxiliary is not:
「猫がいた」 still reads `猫`|`が`|`い`|`た`.

**The span cells were set by `cost.rs`'s `CONTENT_MATCH`** — a `MATCH`
surcharge withheld from particles. It exists because `LEN_GAIN`'s square
cancels exactly when a tiling peels a *single* character off either end of a
headword: a non-COMMON entry of any length split into a COMMON piece of
`n-1` and a COMMON piece of `1` loses by the same one point at `n = 2` as at
`n = 5`, which is `呼び|方`, `発売|所`, `副|大統領`, `月|平均`, `一歩|先`,
`総|売り上げ` and `年|ぶり`. Raising `MATCH` to 3 closes that gap and also
merges `彼の`, `名の`, `煙が`, `今も` and `側から` — because a per-span
constant cannot see that the peeled character is a particle in one set and a
bound morpheme in the other. Withholding the surcharge from `PARTICLE` is
that distinction, and it costs neither split class anything.

**It was found by corpus, not by screenshot.**
`examples/corpus_disagree.rs` ranks the places this engine and the ta-old
port cut differently over 8,334 lines of visual-novel dialogue, by how many
of the engine's own signals agree. This class was the whole head of that
ranking: 77 regions at three signals and 238 at two, hints on. After the
term, 7 and 36 — **and that figure is a split-detector reporting on a
merge-increasing change**, so read it as confirmation that the intended class
closed, never as evidence that nothing else opened. Three of the four signals
can only see over-splitting; the probe's own module doc names the blind spot.
The merge direction was counted separately and is flat: regions where this
engine has fewer segments than v1 go 525 to 529 and 1,228 sentences to 1,232.
**The corpus is copyrighted game script and is not in the repository**, so
this figure cannot be re-checked in-tree — the same standing as §4's "gate 6
was not run". What is re-checkable is every gold cell above and the arithmetic
contract `a_one_character_peel_ties_unless_the_peeled_piece_is_a_particle`
pins.

Hints on: `compounds` 142 -> **152** with `lexical` 72 -> 82 and **both
split classes unmoved** — `false-match` 30 / 32 and `transparent` 40 / 42.
`strict` falls 1754 -> 1745 while coarsening rises 165 spans in 74 merges to
187 in 85, which is this section's own 16.5-point measurement again: ten more
compounds kept whole cost their own short-unit gold spans. `tolerant` rises
1919 -> 1932 and `entry top-1` 0.9374 -> 0.9400.

**Hints off moves for once, and upward**: `strict` 1611 -> 1633, `tolerant`
1749 -> 1775, `entry top-1` 0.9347 -> 0.9358. That matters more than the
hints-on half: `mecab_hints` is `#[serde(default)]` on a `bool`, so a fresh
install runs hints **off**.

**It is not the class above that moves there, and the term's own arithmetic
says so.** `LEN_GAIN` is gated on `bounds`, so hints off the squares are not
present to cancel and the one-character-peel gap is `2n − MATCH −
CONTENT_MATCH`, which *grows* with `n` — 1 point at `n = 2`, 3 at `n = 3`, 7
at `n = 5`. What moves is the term's second mode: it favours a merge by
(content pieces in the tiling − 1), so a three-piece all-content shatter gains
two. Verified — `罪|悪|感` and `解|像|度` become whole hints off, `林|間|学校`
does not. This term is ungated where `LEN_GAIN` is gated because its magnitude
is bounded at one point per span-count difference, so its worst case is one
boundary; `LEN_GAIN`'s reward grows as `n²` and ungated it buys arbitrarily
long spurious spans. "A constant cannot invent a span" would have been the
wrong reason: `LEN_GAIN` only repriced spans that already existed too.

The one cost is one hints-off `false-match` window, `はいた` in
`d:a:003521-6` (23 / 32 to 22 / 32), the §9.5 shape in the state where
`PRONOUNCE_CROSS` is bounds-gated and cannot reach it. The spurious-merge
gate holds at 26 of 26. The snapshot moved once and it is a correction:
`日本語を話せますか。` read `話` + `せま`("narrow") + `すか`("disappointment")
and now reads `話せ` + `ます` + `か`.

**This block was stale for two phases and the staleness had a cost.** It read
`0.8174 / 0.8525 / 0.9092` hints on and `0.9093` entry top-1 hints off until
2026-09-16 — figures Phase 4 superseded and today's ranking change superseded
again — while telling every reader it was "the authoritative current figure".
That is the direct mechanism behind §2 criterion 2 going unmet for eight
phases without anyone noticing: the one block a reader is told to trust was
not being updated, so nothing ever compared itself to the v1 baseline. **A
phase that moves any cell here must rewrite this table in the same commit**,
and the audit of 2026-09-16 exists because that rule was not written down.

**Set 2026-09-09 by `cost.rs`'s `FUNCTION_STACK`, the first deliberate
cost-model change since Task 16b.** It replaces the eight cells every earlier
document quotes — hints on 0.8031 / 0.8521 / 0.9063 with 53 merges and
compounds 0.5430; hints off 0.7272 / 0.7975 / 0.9075 with 75 merges and
compounds 0.5161. All four hints-on metrics rose and no compound class
regressed; hints-off tolerant fell 11 spans against a merge count falling 75
to 62, which is coarsening the tolerant metric was crediting. Details and the
trial log in doc §13, including the `steps.is_empty()` gate without which the
same term shatters `なりたいです` while *improving* every number in this
table.

**One further candidate was re-priced the same day and reverted (doc §13.5).**
With `transparent` at 42 of 42, §10.4's blanket COMMON derivation looked as
though its −22 wrong merges were already prevented. Measured, it takes
`lexical` from 42 to **82 of 112** and compounds to 0.7688 — the largest move
on that metric anything has produced — for **−104 strict spans**, and it fails
`the_particle_absorbing_merges_are_gone`, merging `これは` as one segment. The
−22 turned out not to be one class: `FUNCTION_STACK` blocks *particle*
stacking, while that discount creates *noun*-plus-particle absorption, and
raising `FUNCTION_STACK` to 12 changed nothing. Compound preservation stays
the open half of Phase 3, with three ruled-out candidates instead of two.

**Compound-tolerant span accuracy.** A gold span strict accuracy scored as a
miss counts as a hit when a single *matched* segment swallows it whole and that
segment's own endpoints are both gold boundaries — so it merged whole gold
spans rather than cutting one. This is §4.2's benign-coarsening test applied to
gold instead of to Vibrato. Three conditions, all load-bearing: unmatched
fallback runs are not forgiven, a merge that ends inside a gold span is not
forgiven, and the strict number is still reported beside it.

Measured on the 150-sentence corpus:

| | span accuracy | compound-tolerant | coarsening |
| --- | --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | **0.8465** (1831 / 2163) | 358 spans, 167 merges |
| hints on | 0.6963 (1506 / 2163) | **0.8678** (1877 / 2163) | 371 spans, 173 merges |
| delta | +0.0153 | **+0.0213** | |

Two things follow. The engine's real span behaviour on a deliberately hard
corpus is 0.85, not 0.68 — the missing 16.5 points were `音がする`,
`ことになっている`, `首を振る`, `2月` and their kind. And **the hint benefit is
larger than the strict metric reported**, +2.13 points rather than +1.53, so
§8.3's conclusion is strengthened rather than revised.

Never combined, and never used alone. Strict moving without tolerant moving
means granularity changed, not correctness; the pair localises a regression a
single number would only average away.

**The tolerant metric over-forgives by about 1.3 points, and no heuristic
should be added to fix it.** Roughly 28 of the 167 merges it credits are
spurious — jparser absorbing a particle into the next word and landing on a
kana-unified match that is absurd (`は`+`いた` as 履く, `も`+`もう` as 揉む,
`が`+`いった` as 甲斐). They pass the test because they *are* matched segments
spanning gold boundaries. Separating them from `音がする` needs a label for the
merged span, which the corpus by definition does not carry; every mechanical
discriminator tried cuts the wrong way. The guard is the `coarsening: N spans
in M merges` line printed beside both accuracies: a change that lifts tolerant
accuracy by inflating M has probably found more spurious matches, not more
compounds.

**Phase 1a measured why tolerant accuracy and entry top-1 stay hard to close,
even after tuning (2026-09-01 Phase 1a handoff §4).** JMdict's priority
tagging is inverted on exactly the compounds this section names as the reason
compound preservation matters. `東京都` (ent_seq 1447740 in
`gold_jmdict_subset.xml`) — Tokyo Metropolis, an extremely common word —
carries no `ke_pri`/`re_pri` of its own, while its would-be parts 東 and 京都
each do; `音がする` and `首を振る` are confirmed present as headwords in the
same fixture and show the same shape. Since the engine's only
frequency-shaped evidence signal is JMdict's priority marker, that signal
actively rewards splitting the compounds this section exists to preserve —
no magnitude of the signal can fix an inverted one. This is a real, measured
structural limit, not a magnitude Phase 1a failed to find, and it is a real
share of why tolerant accuracy and entry top-1 miss baseline in Phase 1a's
own measurement (§7 Phase 1a). Phase 3's frequency-rank layer, below, is the
signal that would close this gap.

A second signal for the same class was tried and failed. `音がする` and
`首を振る` both carry JMdict's `exp` tag — real, already-parsed evidence — so
it was priced the same length-proportional way as the priority signal, at the
magnitude needed to tie those compounds against their split. Measured
against the gold corpus: strict span accuracy fell by 91 spans hints-off
(1573→1482) and 92 spans hints-on (1737→1645), for at most 4 spans of
tolerant gain either way, plus one false positive — an archaic `exp`-tagged
copula matched through an unrelated conjugation chain. Gating the term to
direct, unconjugated matches removed that false positive but not the shape:
strict still fell by roughly 60 spans in both hint states for roughly 4 spans
of tolerant gain. It was reverted rather than kept losing. `exp` is real
evidence for the target class, but it is tagged far more broadly than "the
multi-word phrase headwords this corpus's compound-preservation cases need" —
short archaic auxiliaries and incidental grammatical constructions carry it
too, at a rate that outweighs the target class on this corpus. The ceiling
this leaves is scoped, not total: `東京都` is tagged `&n;`, not `&exp;`, so
this signal was never going to reach it — the noun-compound subclass needs a
genuinely different signal.

**Oracle@k** joined in Phase 2. Per gold span, a hit if *any* of the top-k
readings reproduces it — strict, or tolerant with the same benign-merge rule
— because the gate compares against span accuracy and a user cycles per
word; the whole-gold-cover-in-top-k count is printed beside it as context
(Phase 2 design §5). Before the DP existed, Phase 2 measured the bound that
needs none, **oracle@∞**: does the lattice hold a candidate at the gold span
at all? Fallback held at top-1, tolerant adding candidates that could
forgive the span:

| | strict top-1 | strict ceiling | tolerant top-1 | tolerant ceiling | exact-entry ceiling |
| --- | --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573) | 0.8807 (1905) | 0.7975 (1725) | 0.8858 (1916) | 0.9923 (1542 / 1554) |
| hints on | 0.8031 (1737) | 0.8937 (1933) | 0.8521 (1843) | 0.9020 (1951) | 0.9517 (1479 / 1554) |

The lexicon is not the bottleneck — 99% of gold entries are in the lattice
hints off — and the whole strict gap is the DP's choice plus the hard
constraint (Phase 2 design §2.2).

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

**The denominator is the whole sentence, and `ent_seq: null` is a positive
label.** Phase 0 sharpened this: gold covers every span of a sampled sentence,
not only the spans the two segmenters disagreed on, and "no JMdict entry belongs
first here" is a claim the corpus records rather than a gap it leaves. Without
both, span accuracy has no stable denominator and a parser that emits fewer
entries scores better by emitting less.

**§9.1 is answered — five tiers, four committed.** Aozora Bunko (新字新仮名
editions), ja.Wikinews under CC BY 2.5, hand-written pastiche, and noise derived
from those three are committed; real `vn_data` lines stay local as tier C2 and
are never committed. Two sources this plan did not list were adopted for
specific gaps: Wikinews carries the news register Aozora has none of, and
hand-derived noise covers the OCR and vertical-text genres this plan proposed
deferring. The Phase 0 design §3 carries the argument and the derivation table.

**Two sampling hazards found while building it, both worth carrying forward:**

- **Aozora's 新字新仮名 flag describes the edition, not the content.** Classical
  passages quoted inside a modern edition and kana iteration marks (`慌てゝ`)
  both leak through, and both segmenters flail on pre-war orthography — so an
  unfiltered Aozora sample returns disproportionately many of them, a
  distribution ClipYomi never sees and would then be tuned against. A work
  usually has several catalogue rows and the 旧字旧仮名 edition often sorts
  first, so resolving a title to "the first matching row" silently violates the
  filter.
- **"Cover class X" is not "write sentences containing X", and only the sampler
  tells you which you did.** The first pastiche draft aimed at reduplicated
  mimetics using ordinary ones (`どきどき`, `ぐるぐる`) and sampled at 12%,
  because those are JMdict headwords and both segmenters agree on them. The
  conflicting shape is a short mora unit repeated many times with little kanji
  anchor, ideally one JMdict lacks. Rewriting took the tier from 12% to 59%.

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

**The predicate is refinement, not equality** — sample a sentence when jparser
proposes a boundary Vibrato does not, *Bj* \ *Bv* ≠ ∅. Phase 0 sharpened this
from plain boundary disagreement, which selects nearly the whole pool for the
same reason §5.2 rejects boundary F1: jparser merging a Vibrato token pair into
one JMdict entry is compound preservation working, and sampling on it fills the
corpus with sentences where jparser is right. Measured over 66k captured lines,
the sharpened predicate selects 36% where equality selects almost everything,
and the 36% is where the real conflicts are.

`label` prints both parses and **reads a line**, not a keystroke — so the
adjudicator is testable without a TTY, which is what let the whole flow be
integration-tested. Output is append-only JSONL so an interrupted session loses
nothing.

**`label` lists candidate entries ordered by `ent_seq`, never by the ranker's
own score.** Showing the adjudicator a rank-ordered list makes entry top-1
measure the parser against itself: the label is anchored on the answer being
graded. Sorting by a stable id that carries no quality signal is what keeps the
metric independent, and it costs nothing.

---

## 6. Phase order

```
Phase 0  Harness + gold corpus + baseline      ← proves everything after it
Phase 1  Clean-room core, k = 1                ← the relicence
Phase 2  k-best readings + UI                  ← the quality win
         2   instrument only, gate failed at +40 spans
         2b  reading identity, gate cleared at +65 on-engine, UI shipped
Phase 3  Extra linguistic layers               ← independent, per-layer licensing
Phase 4  The hint constraint's escape          ← added 2026-09-15, not in the original order
```

**Phase 4 was added after this block was written** and took a slot the plan
had held open for compound preservation, once the long-unit spike closed that
route (`docs/superpowers/2026-09-15-long-unit-separation-spike.md`). It is the
only phase here that changes §4.3's core mechanism rather than adding a layer,
which is why §4.3 and §5.2 both carry 2026-09-16 corrections. See
`specs/2026-09-15-engine-v2-phase4-design.md` and the 2026-09-16 handoff.

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

### Phase 0 — Harness and baseline — **COMPLETE**

**In:** the three CLI subcommands (§5.4); the JSONL label format; genre
extension of the sentence corpus, sampled by jparser-against-Vibrato
disagreement (§5.1); a recorded baseline of both metrics for the current engine,
with and without hints.

**Out:** any change to parsing behaviour. **Amended:** the rule reads *no change
to the parser library*, not "no code under `crates/jparser/src/`" — read
literally the latter forbids Phase 0's own deliverable, since §5.4 specifies the
harness as `jparser-cli` subcommands and the bin lives under that path. Two
exceptions are named and no others: `src/bin/jparser-cli*`, and one additive
read-only `entry_id` field on `jparser::Entry` that `score` needs to compare
entry decisions at all. Parsing behaviour is untouched.

**Done when:** `jparser-cli score` prints span accuracy and entry top-1 for the
current engine over a committed gold corpus, reproducibly and offline. **Met.**

**The baseline, measured over the committed 150-sentence corpus:**

| | span accuracy | compound-tolerant | entry top-1 |
| --- | --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | 0.8465 (1831 / 2163) | 0.9159 (1046 / 1142) |
| hints on | 0.6963 (1506 / 2163) | 0.8678 (1877 / 2163) | 0.9155 (1072 / 1171) |
| delta | +0.0153 | +0.0213 | −0.0004 |

That pair is the contract every later phase is measured against. Three things
constrain how it may be read, and Phase 1 should not quote a number without
them:

1. **Quote both span metrics, never the strict one alone.** Against a corpus
   adjudicated at short-unit granularity, strict accuracy rewards splitting the
   JMdict compounds §5.2 exists to preserve — so optimising it directly
   optimises against the product. The tolerant metric in turn over-forgives by
   roughly 1.3 points, because ~28 of its 167 merges are spurious. A rising
   merge count is a warning, not a win.
2. **The corpus is model-adjudicated and author-reviewed on entry decisions
   only.** `crates/jparser/tests/fixtures/gold_sources.md` states exactly what
   was checked and what was not; 54 of the 150 sentences carry a cut neither
   segmenter proposed and no mechanical check covers them.
3. **Absolute values are not a quality verdict.** Every sentence was sampled
   because the two segmenters disagreed on it. The numbers exist to be
   comparable across phases.

**Two items stay open** and neither blocks Phase 1: tier C2's local baseline
(`gold_local.jsonl`, never committed) was not built, so there is no check that
the committed corpus has not drifted from the real capture distribution; and 96
of the 246 sampled sentences remain unadjudicated in staging.

### Phase 1 — Clean-room core

**Split into Phase 1a and Phase 1b (design §1, §2, added 2026-09-01).**
§7.3 below — recombining IPADIC's auxiliary tokens into a labelled chain — has
two outputs: a span shape and an English string. The span shape is the DP's
merge decision and is visible to span accuracy — worse, compound-tolerant
accuracy scores it as a merge, the class it is measured to over-forgive by
about 1.3 points (§5.2). Only the English string is invisible to both Phase 0
metrics. So **1a owns the chain's span shape; 1b owns only the mapping from
that shape to English** (design §2.1). And 1b's own gate — a class × chain
grid generated mechanically for coverage, its expected English column written
by hand from a grammar reference *before* the formatter exists — must not be
written beside the code it grades: a table written alongside the recombiner
would agree with it by construction, the same trap §5.4's rule against a
rank-ordered candidate list exists to avoid (design §2.2).

**Phase 1's own done-when is unchanged in substance and lands whole, at the
end of 1b:** no `ta-old` reference and no `ported from Translation
Aggregator` header remains **anywhere in the tree** — not a directory list,
which §3.2's two corrections above show has now fallen behind twice (first
missing `crates/jmdict-source`/`crates/vibrato-source`, then missing
`src-tauri/`, including `build.rs`, and `tools/`), but both strings, grepped
tree-wide with zero hits. Corrected counts of the header string alone, as of
Phase 1a: 24 files under `crates/jparser/`, 1 under `xtask/`, 4 under
`crates/jmdict-source`, 6 under `crates/vibrato-source`, 11 under
`src-tauri/` (10 under `src/` plus `build.rs`), 1 under `tools/`
(`extract_jmdict_subset.py`) — 47 files total.

**Relicence clause met** (Phase 1b-ii): both strings grep to zero tree-wide.
**Scope correction (2026-09-16): not tree-wide — outside `docs/`.**
`git grep -l "ta-old"` still returns about forty tracked files and
`"ported from Translation Aggregator"` fourteen, this document among them,
because the phase records name the strings they removed. The measurement that
matters is
`git grep -l -e "ta-old" -e "ported from Translation Aggregator" -- ':!docs'`,
which is empty. The done-when insisted the scope was deliberately not a
directory list; that was the wrong call and the wording claimed more than it
measured.
**Metric clause superseded**, not met: the tolerant gap is the structural
JMdict-priority ceiling §7 Phase 3 already assigns to the frequency layer, and
no Phase 1 tool reaches it. In the shipped configuration (hints on) v2 is
+10.7 points strict, −1.6 tolerant, −0.9 entry top-1 against v1 — strict
measures the particle-absorbing defect this project exists to fix; tolerant
measures compound preservation, which Phase 3 owns. `くるまではこう` parses as
`くる / まで / は / こう`.

#### Phase 1a — Clean-room segmentation core

**In:** independently written replacements for the segmentation core: the
lattice and its cost model, candidate generation, deinflection, the
conjugation class-and-form table regenerated from IPADIC, headword flags,
kana folding, romaji. A second engine (`parse_v2`), reachable behind
`--engine v2` on `build-index`/`parse`/`score`, measured against the Phase 0
baseline. v1 stays the default parser and is untouched throughout.

**Out:** naming a conjugation chain in English (Phase 1b, below). The
switchover that makes the new engine the default and deletes the old files.
k-best (Phase 2). Any UI change.

**Done when (design §10.1–§10.3, §10.5 — four conditions):**

1. Both Phase 0 metrics, in both hint states, at or above baseline.
2. `くるまではこう` parses as `くる / まで / は / こう`.
3. The particle-absorbing merge class this section's decision log records
   (handoff §9.5) is gone, checked against a committed fixture.
4. The standing gates pass (`cargo test -p jparser`, with and without
   `--features mecab`; `cargo clippy` with `-D warnings`, both feature states;
   `cargo check --no-default-features`), plus an offline check that a
   full-index build and the committed subset-index build score identically
   (design §10.5).

**Size, restated per module (design §9).**

| New module | Replaces | Master plan est. | 1a lines |
| --- | --- | ---: | ---: |
| `lattice.rs` + `cost.rs` | `segment.rs`, `rank.rs` | ~650 | ~650 |
| `lex.rs` | `matcher.rs`, `matcher/verb.rs` | ~600 | ~350 |
| `table.rs` | `stem.rs`, `conjugation.rs`'s table | ~850 | ~550 |
| `chain.rs` (span shape only) | *(new)* — §7.3's recombiner | ~300 (shared with 1b) | ~120 |
| `flags.rs` | `record.rs` | ~350 | ~350 |
| `kana.rs` + `romaji.rs` | same | ~560 | ~560 |
| **1a total** | | | **~2,580** |

Two deviations from the line counts an earlier draft of this plan estimated.
`lex.rs` is smaller and `table.rs` larger than their old estimates, because
deinflection moved from build time (`stem.rs`, recursed at parse time in
`matcher/verb.rs`) to a single parse-time reverse-suffix-trie lookup in
`table.rs` — one mechanism instead of two kept in step, and the `する`/`来る`
empty-stem special case disappears with it (design §6). And **`stem.rs`'s
work does not shrink because IPADIC supplies dictionary forms — it moves into
the table** (design §11.5; see also §7.2 below): `hints` is optional and
feature-gated, with `cargo check -p jparser --no-default-features` an
enforced gate, so the engine must deinflect with no tokenizer present at all.
IPADIC can never be the deinflection path, only ever an accelerator.

**Measured outcome (Phase 1a handoff, 2026-09-01).** `jparser-cli score
--engine v2` over the committed `gold.jsonl` (150 sentences, 2,163 spans)
against a v2 index built from the committed `gold_jmdict_subset.xml`:

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| baseline, hints off | 0.6810 (1473/2163) | 0.8465 (1831/2163) | 0.9159 (1046/1142) | ≤167 |
| **v2, hints off** | **0.7272 (1573/2163) PASS** | 0.7975 (1725/2163) MISS | 0.9075 (1108/1221) MISS | 75, pass |
| baseline, hints on | 0.6963 (1506/2163) | 0.8678 (1877/2163) | 0.9155 (1072/1171) | ≤173 |
| **v2, hints on** | **0.8031 (1737/2163) PASS** | 0.8521 (1843/2163) MISS | 0.9063 (1170/1291) MISS | 53, pass |

**Done-when condition 1 is not met — 4 of the 8 cells miss.** Both hint
states pass strict span accuracy by a wide margin (+0.0462 hints off,
+0.1068 hints on) and hold the merge ceiling; both miss tolerant span
accuracy and entry top-1, by single digits of percentage points after
tuning — §5.2 above explains the structural reason, found by measurement,
not assumed. **Condition 2 is met** — `くるまではこう` parses as
`くる / まで / は / こう` under the tuned constants. **Condition 3 is not
met** — the particle-absorbing merge fixture passes 22 of its 26 rows; the
test stays ignored. **Condition 4 is met** — the standing gates and the
full-index/subset-index equivalence check both pass.

**Two items deferred, left open with their reasons rather than attempted:**

- The conjugation trie is hiragana-keyed with no folding, so a fully
  katakana-written conjugated form never deinflects. Tier D (noise) is only
  8 sentences / 169 spans in the gold corpus — too thin a sample to settle
  whether this costs anything.
- `ATTACHES_TO` lets `た` attach to `連用形` unconditionally, which licenses
  the over-generation `書きた → 書く`. The candidate fix is making the
  required form a predicate rather than a fixed form — `連用タ接続` where a
  class has that form, `連用形` only where it does not.

#### Phase 1b — Formatter, class × chain gate, and the switchover

**Split into Phase 1b-i and Phase 1b-ii (design
[`2026-09-02-engine-v2-phase1b-i-design.md`](2026-09-02-engine-v2-phase1b-i-design.md)
§1, added 2026-09-02).** 1b-i is the formatter and its gate, landed behind
the existing `--engine v2` flag; 1b-ii is the switchover, with the In/Out/
Done-when below carried verbatim. The split exists because Phase 1a's metric
gate is open on four of eight cells and the switchover decision wants the
formatter working and those numbers in hand, and because §7.3 names the
formatter the highest-risk item — de-risked first, it leaves the switchover
a smaller commit. Phase 1's own done-when lands at the end of 1b-ii — its
relicence clause met, its metric clause superseded, as recorded above.

**Phase 1b-i — complete (handoff
[`../2026-09-02-engine-v2-phase1b-i-handoff.md`](../2026-09-02-engine-v2-phase1b-i-handoff.md)).**
`Entry::conjugation` is filled on the v2 path by `engine/label.rs` from
`Entry::steps`, in the learner-standard register (Causative,
Passive/Potential, Negative, Volitional, Polite, Want-to, Past, Te-form,
Conditional, Imperative, Adverbial), with the English Wikipedia article
*Japanese verb conjugation* as the named reference. The gate is
`tests/fixtures/conjugation_labels.txt` — 221 hand-written rows (59 at depth
0, 101 at depth 1, 61 curated deeper): every `(adjective?, form)` at depth 0,
every `(auxiliary, form)` at depth 1, a curated set to depth 6 — written
before the formatter and run through the real trie, plus a mechanical
coverage walk to depth 2 that asserts the fixture omits nothing and asserts
no English. Both Phase 0 metrics are byte-identical to Phase 1a's, as they
must be. The grid exposed five table findings (design §7, handoff §4) that
are span-shape questions and are left for a measured 1a follow-up or
1b-ii — four are `ATTACHES_TO` rows (a missing row, rows pointing at the
wrong class, a missing restriction); the fifth is a collision between two
`CLASSES` shapes (特殊・タ and 特殊・タイ both emit a bare `た` surface), not
an `ATTACHES_TO` problem.

**In:** the auxiliary-chain formatter — `fmt(&[ConjStep]) -> Option<String>`,
filling `Entry::conjugation`, which 1a leaves `None`; its own gate, a class ×
chain grid generated mechanically for coverage with the expected English
column written by hand from a grammar reference before the formatter exists,
committed as a fixture; the switchover — the nine hard-taint files of §3.2
deleted, `assets/conjugations.json` deleted, `xtask`'s ta-old converter
deleted, every `ta-old` citation and every `ported from Translation
Aggregator` header gone tree-wide — `crates/jparser/`, `xtask/`,
`crates/jmdict-source`, `crates/vibrato-source`, `src-tauri/` (including
`build.rs`), and `tools/` included, checked by grepping both strings over the
whole repository rather than an enumerated directory list; the provenance
headers of §9 open question 2 written on the replacement files.

**Out:** k-best (Phase 2). Extra token layers (Phase 3). Any UI change.

**Done when:** the class × chain gate passes, and the switchover lands with
nothing shipped in between (design §4.1) — which is also when Phase 1's own
done-when above is settled. The 1b-ii design §1 refined "one commit" into the
four-stage sequence recorded below: the rewiring is one indivisible commit,
but the deletion of the ported sources and the licence flip are deliberately
separate from it, so that a commit exists in which every ported line is gone
and the tree is still `GPL-2.0-only`.

**Size, restated per module (design §9).**

| New module | Replaces | 1b lines |
| --- | --- | ---: |
| `chain.rs` (English formatting) | *(new)* — §7.3's recombiner, formatter half | ~180 |
| `conjugation.rs`'s label mapping | *(new)* — §7.3 | ~300 |
| **1b total** | | **~480** |

Plus 1b's own expectation fixture. 1a's ~2,580 and 1b's ~480 total ~3,060, a
little under the original ~3,300 estimate; the difference is rounding across
rows, not a claim of a smaller phase.

**Phase 1b-ii — complete (design
[`2026-09-02-engine-v2-phase1b-ii-design.md`](2026-09-02-engine-v2-phase1b-ii-design.md),
handoff
[`../2026-09-02-engine-v2-phase1b-ii-handoff.md`](../2026-09-02-engine-v2-phase1b-ii-handoff.md)).**
Landed in four stages, in the one order that builds: **prep** (the startup
fix, the two table fixes measured one at a time, v2's fingerprint pinned,
`parse_snapshots.rs` retired with one test ported forward), **switchover**
(one indivisible commit of about forty files — the relocations, index format
4, the engine-agnostic lifecycle, `parse` becoming v2, `src-tauri` and
`jparser-cli` following, and the nine v1 files orphaned but untouched on
disk), **delete** (`git rm` of those nine plus `matcher/verb.rs`,
`assets/conjugations.json`, the `xtask` crate and `.gitignore`'s `ta-old/`
line — 8,980 deletions against 8 insertions, and the tree still
`GPL-2.0-only` at that commit, which is why the deletion is separate from the
flip), and **relicense** (59 files re-headered, `LICENSE` swapped for
`LICENSE-MIT` plus the canonical `LICENSE-APACHE`, both banned strings at
zero, and `cargo fmt` run once). The hazard §3 found is the one the prep
stage exists for: `state.rs::load_state` opens the newest generation with
`Index::open`, which after the fingerprint bump returns
`IndexError::ConjugationMismatch` for every user upgrading with a v1-built
index — and `is_missing_dictionary` mapped `StartupError::Index(_)` to
`false`, so the webview would have shown a fatal error screen for exactly the
condition the download screen already repairs. One match arm routing
`ConjugationMismatch` and `VersionMismatch` to `true` fixes it, and those are
precisely the two errors `ensure_dictionary` already rebuilds on. The two
bundled table fixes both landed measured and both proved metric-neutral: the
`たい:連用形` `ATTACHES_TO` row moved below the two `た` rows, flipping three
snapshot labels (読みました, しました, 書かせられた) from *Want-to* to *Past*;
and `JMDICT_POS_CLASS` became a multimap so `vk` also maps `カ変・来ル`, which
makes kanji-written 来る deinflect at last (来ます went from two junk spans to
one `来る (Polite) [きます]`) under a table-derived reading rule that takes the
stem reading from `カ変・クル`'s same-named form. The coverage walk re-pinned
from 21,760 chains to 22,783, and all eight `score` cells reproduce Phase
1b-i's exactly in both hint states: hints off 0.7272 strict / 0.7975 tolerant
/ 0.9075 entry top-1 / 75 merges, hints on 0.8031 / 0.8521 / 0.9063 / 53.

**§7.2 — Table size.** ta-old's table holds 32 types and 734 conjugations. The
replacement need not match that count — it must cover what the corpus exercises
plus the standard forms — but the order of magnitude is the planning number.
IPADIC's own conjugation data supplies surface forms mechanically; the
learner-facing English labels do not come from it. As design §11.5 records,
this table's size does not shrink to ~250 lines the way an earlier reading of
this section assumed — see Phase 1a's Size note above.

**§7.3 — The label mapping is the product.** IPADIC yields 未然形 / 連用形 /
基本形 — linguistic form names — and splits auxiliaries into separate tokens
(`させ|られ|ませ|ん|でし|た`). JParser's distinguishing feature is the
learner-facing chain, *"Causative-Passive Polite Negative Past"*. Recombining
IPADIC's auxiliary tokens into that chain, and naming the result in English, is
new work with no permissive upstream. It is the single highest-risk item in this
plan and the one most likely to need its own spec — which is why, per Phase
1b's In above, its gate is written before its code (design §2.2).

### Phase 2 — k-best readings

**In:** the §4.1 route decision, taken with Phase 1's measured cost model in
hand; k-best paths over the Phase 1 DP; a whole-sentence reading list in
cost order (no scores on the wire — the order is the rank, and the `i32`
cost has no user meaning; Phase 2 design §1); UI to cycle readings; oracle@k
added to `score`.

**Out:** automatic disambiguation beyond ranking. The user picks.

**Done when:** oracle@3 *tolerant* span accuracy, hints on, exceeds top-1
tolerant by at least 55 spans of 2,163 (2.5 points rounded up: at least
1,898 against 1,843), measured on the Phase 0 corpus. Tolerant rather than
strict because strict would credit re-splitting the compounds the tolerant
metric exists to forgive; half the measured ceiling headroom because below
that a k = 1 cost-model pass plausibly recovers as much without a UI (Phase
2 design §2.3, §8). **If it does not, Phase 2 ships nothing** — the k-best
lattice and the `score --readings` instrument stay, the wire field stays
empty, no UI lands. That outcome is a valid result, not a failure to be
worked around.

**Measured 2026-09-02 and not met:** oracle@3 tolerant hints on reached
1,883 of 2,163, 40 spans over top-1, against the 55 required. Phase 2 shipped the
measurement only; see the Phase 2 handoff §3.

**Reopened 2026-09-09 as Phase 2b.** The lever the Phase 2 handoff §6 named —
reading identity, not list capacity — clears the same gate when measured
*outside* the engine: demoting readings that differ from a kept one only by a
merge or split reaches 1,908 of 2,163 (+65) at reading depth 12 and 1,911
(+68) at depth 64, against the 55 required, with top-1 unmoved. Deleting them
instead reaches only +54. The gate itself is unmoved and is re-measured
through the shipped path in Phase 2b, whose first task is to find out whether
the reorder pays over the reading pool k = 3 already holds — which would cost
the DP nothing — or needs a deeper one. See the Phase 2 handoff §7 for the
measurement and its two recorded costs (one whole-sentence cover lost of 150,
and an unresolved product question about the readings demotion sinks), and
[the Phase 2b design](2026-09-09-engine-v2-phase2b-design.md) for what ships.

**Measured 2026-09-09 and met:** oracle@3 tolerant hints on reached 1,908 of
2,163 through the shipped path, 65 spans over top-1, against the 55 required,
over the pool k = 3 already holds. Phase 2b shipped the reading-cycling UI;
see the Phase 2b handoff §3 and §4. Whole-sentence cover in the top three is
23 of 150 (Phase 2: 25), recorded and unthresholded.

### Phase 2c — Candidate generation walks the index once per position — **COMPLETE**

Not a quality change: the candidate set is byte-identical, and so are the
eight cells and Phase 2b's oracle@3 figures — measured and confirmed in both
hint states (Phase 2c handoff §2), including the hints-off `--readings 3`
triple, which the handoff had not run when this entry was first written.
`lex::candidates` enumerated
every `(start, end)` span pair and built a key per pair, making generation
superlinear in input length — a 10,000-character paste, the app's own
clipboard ceiling, did not finish in ten minutes. It now walks the key FST
once per start position, with the walk's own dead end bounding the span. See
the Phase 2c handoff for the before-and-after timings.

### Phase 3a — The KANJIDIC layer — **COMPLETE**

One block at the foot of the popover, one line per distinct kanji in the
hovered surface: on and kun readings and up to four English meanings from
KANJIDIC2. Provisioned through the dictionary build (a sixth index file,
format 4 → 5, so existing installs rebuild once), fetched beside JMdict by
the same source crate, and reached lazily on hover through one command and a
page-lifetime cache, so the parse payload did not grow. `hide_kanji` joins
the gloss filters; the settings window gains the app's first persistent
attribution. Design: `specs/2026-09-09-engine-v2-phase3a-design.md`; record:
`2026-09-11-engine-v2-phase3a-handoff.md`. The engine was not touched, and
§5.2's baseline block is unchanged to the cell.

### Phase 3b — The KANJIDIC misc fields — **COMPLETE**

The four `<misc>` numbers Phase 3a's parser read and discarded — school
grade, stroke count, newspaper frequency rank, former JLPT level — kept on
the same record and appended to the same kanji line as one parenthesised
aside, `(grade 2, 8 strokes, freq 37, old JLPT 4)`, which `colourLine`
paints as a paren run with no renderer or stylesheet change. Index format
5 → 6 with the header untouched, so existing installs rebuild once from the
archives they already hold. The JLPT number is labelled as the former
four-level scale and never mapped to N-levels; the Phase 3 table's JLPT row
is settled for kanji and declined for words, with the reasons in the design.
Design: `specs/2026-09-11-engine-v2-phase3b-design.md`; record:
`2026-09-11-engine-v2-phase3b-handoff.md`. Nothing under `engine/` was
touched, and §5.2's baseline block is unchanged to the cell.

### Phase 3c — The KANJIDIC layer close-out — **COMPLETE**

The three loose ends Phases 3a and 3b left, closed together and nothing
else. Kun readings drop KANJIDIC2's affix hyphen at render time and keep
the okurigana dot (`-ぐ.い` renders `ぐ.い`, `た.べる` is unchanged); the
record and the wire keep the notation. The source sniff is root-aware on
both the plain and the gzip path: each `Source` names its root element, and
a JMdict file under the KANJIDIC2 name — declaration intact, or gzipped — is
refused instead of building a kanji-less index. The dictionary commands left
`src-tauri/src/commands.rs` for `commands/dictionary.rs` and
`commands/hints.rs` by a pure move. No engine, index format, wire, settings
or CSS change, so no install rebuilds and §5.2's baseline block is unchanged
to the cell. Design: `specs/2026-09-12-engine-v2-phase3c-design.md`; record:
`2026-09-12-engine-v2-phase3c-handoff.md`. The Phase 3 table is not changed
by this phase; the two research items it leaves — the long-unit token
inventory and a pitch-accent source hunt — are spikes, not phases.
**The first ran on 2026-09-15 and closed the route**
(`docs/superpowers/2026-09-15-long-unit-separation-spike.md`) and the second
ran the same day (`docs/superpowers/2026-09-15-pitch-accent-source-hunt.md`),
leaving UniDic as the one surviving accent source and a coverage measurement
as what stands between it and a phase.

### Phase 3 — Extra linguistic layers

Independent of segmentation entirely. Each layer is its own increment with its
own licensing gate, adopted only if its data source is permissive:

| Layer | Source | Licence status |
| --- | --- | --- |
| Frequency rank | corpus-derived | **Moot** — the layer is ruled out on measurement, so no licence needs establishing |
| JLPT level | KANJIDIC2 `<jlpt>`, kanji only, former four-level scale | **Kanji half shipped, Phase 3b (2026-09-11)**, labelled as the former scale. Word-level JLPT declined: no official N1–N5 list exists, so no source can meet the written-provenance gate |
| Kanji readings | KANJIDIC2 (EDRDG) | **Shipped, Phase 3a (2026-09-11).** CC BY-SA 4.0, same publisher and licence as JMdict; attribution in the settings footer. Phase 3b added grade, stroke count, frequency rank and former JLPT level to the same line |
| Pitch accent | UniDic (NINJAL) `aType`, under the modified-BSD leg | **Source hunt done 2026-09-15** (`docs/superpowers/2026-09-15-pitch-accent-source-hunt.md`). UniDic passes — the triple licence is real, the BSD leg is selectable, and the accent columns are columns of the same lexicon §3.3 already chose. **Wadoku fails and its CC BY-SA 3.0 premise was wrong**: both licences in force restrict commercial use behind written permission. `kanjium` declares CC BY-SA 4.0 over undeclared provenance and is refused under §3.4. Gated on an unmeasured coverage figure before any phase opens |

No layer ships without its licence established in writing first.

**The premise was sized on the corpus 2026-09-09, and the blocker turned
out to be the gate rather than the corpus.** The inversion is real and is the
mechanism: of 141 windows where a compound was available and the parse
declined it, 104 are inverted, and every example this plan names that occurs
in the corpus — `音がする` in five sentences, `首を振る`, `ことになっている` —
is among them. But `score`'s coarsening count cannot show that (0 of its 53
merges are inverted, structurally: with `CROSS = 0` an under-evidenced
compound is priced out before it can become a merge), and neither span metric
above can score fixing it as a gain. §5.2's third metric was built for that
reason, and on it the same fix reads +25 windows where strict read −6.9
points. It also prices the free alternative: a build-time COMMON derivation
takes the `lexical` class from 41 to 88 of 112 and pays 22 wrong merges,
because it cannot tell `副大統領` from `名の` — which is exactly what a
frequency rank does, and is the sharpest argument for this layer that exists.
Details in `docs/superpowers/2026-09-09-frequency-signal-candidates.md`
§10-§11; the licence question below is untouched by any of it.

**Frequency rank stopped being a nice-to-have at Phase 1a (2026-09-01
handoff §4).** It is the signal §5.2's compound-preservation finding shows is
missing: JMdict's own priority marker is inverted on exactly the compounds
this plan exists to preserve, so it actively rewards splitting them, and no
cost-model magnitude can fix a signal that points the wrong way. A genuine
frequency-rank layer was the candidate fix for that specific gap.

**It is now ruled out on measurement, and the licence question with it
(2026-09-09, doc §12).** Both buildable corpora of §8 were built — 400 Aozora
works, 12.1 M characters, and the whole `jawikinews` dump, 2.7 M — and each was
scored on the adjudicated type list held out to the *other* register, since
Aozora carries no news words and a Wikinews table would otherwise score on
windows the gold corpus itself sampled from Wikinews. Both traps measured out
as predicted (Aozora is blank on 26 of 39 news-sourced compound types;
Wikinews is blank on 0 of 39 of its own). On the held-out sets **every
count-derived ranking scores AUC ≤ 0.51 and none beats a blanket discount by
more than two windows**, while two signals needing no corpus at all — surface
length and part count — beat all of them. The volume rankings are worse than
chance: `には` occurs 2,585 times on token boundaries in the news corpus, between
`である` (2,569) and `として` (4,852). The reason is structural rather than a
matter of corpus or size: what the metric asks for is **wordhood, which
frequency does not measure**, and under a short-unit inventory only 5 of the 87
should-merge types are a token at all. So the third candidate — the `vn_data`
capture pool, whose licence turns on a judgment only the copyright holder could
make — is settled too, without anyone making that judgment. The route the
measurements leave open was a **long-unit** token inventory rather than another
corpus — **and that was tested on 2026-09-15 and closed too**
(`docs/superpowers/2026-09-15-long-unit-separation-spike.md`). SudachiDict
Mode C, the concrete long-unit article, never emits a `transparent` or a
`false-match` span as a token (0 / 42 and 0 / 32) but emits only 9 of 112
`lexical` windows as one, so every reading's best threshold is the blanket
itself at AUC 0.546. The nine are noun-noun compounds; the other 103 are
constructions, and no word inventory holds a construction at any unit length.
**Phase 3's data thread is finished in every direction**: not the corpus, not
the dictionary, not the unit. What is left of compound preservation is a
lattice-and-cost question inside the engine, which is a different premise and
is not scoped here.

---

### Phase 4 — The hint constraint's escape — **COMPLETE**

Added 2026-09-15, outside the original phase order (§6). `cost::is_allowed`
forbids a span from starting or ending strictly inside a tokenizer token,
which is what saves `東京都`; it had no escape for the case where it forbids
*everything*, so wherever IPADIC held a unit JMdict lacks, the characters fell
to unmatched with the boundaries still correct — a phrase with no definitions
under it. `engine::relax::Relaxed` reopens the interior of such a token **when
the token carries a hiragana character**; without that condition the same rule
shatters `品川` into 品 + 川 and cost 88 gold spans against 8 gained, which is
why the first implementation (`spike/phase4-escape`) did not ship.

Hints-on strict 0.8174 → 0.8285, tolerant 0.8525 → 0.8645, compounds
unchanged, spurious merges 26 of 26, hints-off identical to the digit. On the
eighteen sentences reported from real use, matched characters went 426 → 475
of 630 against hints-off's 487.

A follow-on the same week took `assemble::order` two clauses further — a
`COUNTER` promoted only after a digit and a `PARTICLE` promoted among equals —
which is what finally put §2's criterion 2 above the v1 baseline.

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

1. ~~**Corpus sourcing (§5.3).**~~ **ANSWERED — all three, plus two sources this
   plan did not list.** Visual-novel text is copyrighted, so it is used as a
   *sampling pool* and never committed: `diff` runs over the 66,114 cleaned
   capture lines to find which register classes actually generate conflicts, and
   the pastiche tier is then written to cover those classes. Register and style
   are not copyrightable; transcribed lines are. That gets real-distribution
   coverage into a committed corpus without committing a word of it.

   Five tiers, four committed:

   | Tier | Source | Committed | Covers |
   | --- | --- | --- | --- |
   | A | Aozora Bunko, 新字新仮名 only | yes — public domain | literary prose, long sentences, 「」 speech |
   | B | ja.Wikinews | yes — CC BY 2.5, own notice | news register, proper nouns, numerals, katakana loans |
   | C1 | hand-written pastiche | yes — project licence | visual-novel dialogue, casual and contracted speech |
   | C2 | real `vn_data` lines | **never** | the true input distribution, locally only |
   | D | noise derived from A–C1 | yes — follows its parent | OCR noise, vertical-text artefacts |

   The two additions are B and D. Aozora is literary prose from roughly
   1900–1955 and carries no news register at all, so answering §5.3's request
   for that genre with 1920s fiction would leave it unmeasured under a label
   claiming coverage. And OCR noise, which §9.1's original lean proposed
   deferring until real clipboard capture exists, is both the genre most likely
   to break silently and the cheapest to cover — it is derived by hand from
   lines already committed.

   **What it actually cost, against the estimate.** The design assumed ~10 spans
   per sentence; the measured median is 14 and the mean 16.3, so the sampled 246
   sentences are ~4,000 span decisions rather than 2,500 — 60% more. The
   committed corpus stops at the top 150 by conflict density, which still covers
   every tier. Anyone sizing a corpus extension should use the measured figure.
2. ~~**Target licence.**~~ **ANSWERED — `MIT OR Apache-2.0`.** The author's
   decision, taken at the Phase 0/1 boundary so Phase 1 writes the nine
   provenance headers once.

   Three properties decided it. The dual licence is the Rust ecosystem's
   default, so every crate this engine will sit beside already carries it and
   no downstream has to reason about an unusual combination. The Apache leg
   carries an express patent grant, which neither MIT alone nor any of the
   sources in §3.3 provides. And it is the only option that leaves §9.7 open:
   sudachi.rs is Apache-2.0 with no MIT leg, so an MIT-only target would
   permanently foreclose adopting it while an Apache-2.0-only target would
   reproduce, in the opposite direction, exactly the GPL-2.0-only
   incompatibility `src-tauri/Cargo.toml` records twice.

   Header form for Phase 1, on each of the nine files:

   ```rust
   // SPDX-License-Identifier: MIT OR Apache-2.0
   //
   // Written for ClipYomi without reference to Translation Aggregator's
   // JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
   // §3.5 for the clean-room discipline this file was written under.
   ```

   Note this decision does **not** relicense anything on its own. The tree is
   GPL-2.0-only until the nine hard-taint files of §3.2 are gone, which is
   Phase 1's output. Until then the header states the intended licence of the
   new file, and the repository's licence is unchanged.

   **Written at Phase 1b-ii. The tree is `MIT OR Apache-2.0`.** 59 files
   re-headered, `Cargo.toml` and `package.json` updated, `LICENSE` replaced by
   `LICENSE-MIT` plus the canonical `LICENSE-APACHE`. One open item goes with
   it, and it is the maintainer's to settle rather than a phase's: ten
   line-range citations into the upstream GPL-2 C++ survive in
   `src/render/*` and `src/styles/tooltip.css`, files this phase relabelled
   without ever having fenced or reassessed them — the clean-room rebuild
   covered the parsing engine only. See the 1b-ii handoff §1.

   **Resolved 2026-09-09, and the count was wrong (corrected 2026-09-16).**
   There were **seven** sites, not ten —
   `docs/superpowers/2026-09-09-frontend-provenance-inventory.md` §2 enumerates
   them §2.1 to §2.7 — and its §9 records them settled as attribution rather
   than derivation: one reauthored clean-room (`b8035a3`), six restated, 93 of
   93 files stamped. `grep -rn "MyToolTip\|DictionaryUtil\|MyDrawText\|Dictionary.cpp" src/`
   now returns nothing, so none of them "survive".
3. ~~**`jmdict.rs` and `index/*` disposition (§3.2 soft tier).**~~ **ANSWERED
   — scrubbed, at Phase 1b-ii.** Rewriting was cleanly safe; scrubbing the
   citations was cheaper and is what happened. Both carry
   `SPDX-License-Identifier: MIT OR Apache-2.0` headers and no ta-old
   citations, and Phase 1 did not grow by 1,300 lines. Left unstruck until
   2026-09-16 while every other answered item here was struck.
4. ~~**Does k-best justify its UI?**~~ **ANSWERED — no; the gate was not met.**
   oracle@3 tolerant span accuracy, hints on, reached 1,883 of 2,163, +40
   spans over top-1's 1,843, against the 55 required. No interface was built
   (§7 Phase 2; Phase 2 handoff).
5. ~~**Which N-best route (§4.1)?**~~ **ANSWERED — own k-shortest-path over
   the Phase 1 DP.** Not the Vibrato fork this item's original lean named:
   Phase 1a's hints supply boundaries only, so there is no tokenizer lattice
   in the engine to take an N-best from in the first place (§4.1).
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

   **None of that sentence is true of the tree (corrected 2026-09-16).**
   `hints.rs` reads no feature column at all: `VibratoTokenizer::boundaries`
   uses `worker.token(i).range_char()` and returns a `BoundarySet`, nothing
   more. There is no reading field, no override and no flag derivation, and
   `TA_HINTS_READING_FIELD` appears nowhere outside this document and the
   Phase 0 handoff. The concern was dissolved rather than guarded — Phase
   1b-ii replaced the advisory flag pair with the boundary set — so the
   question is closed, but not by the mechanism described here. §10's decision
   row "Treat the tokenizer's reading column as a per-dictionary property"
   describes the same non-existent guard.
7. ~~**Does sudachi.rs change the answer to question 6?**~~ **ANSWERED — no.
   Stay on Vibrato for Phase 1.** Measured out-of-tree at the Phase 0/1
   boundary against the 150-sentence corpus.

   Pure Rust, zero FFI, and its Mode A/B/C multi-granularity is aimed at exactly
   the short-unit-versus-long-unit tension §5.2 describes: Mode C keeps `東京都`
   whole, Mode A splits it. Two caveats kept it out of Phase 1 in advance. **It
   is Apache-2.0 with no MIT leg**, and `src-tauri/Cargo.toml` records twice
   that Apache-2.0 alone is incompatible with this repository's GPL-2.0-only —
   so it cannot enter the tree until the relicence has landed, which is Phase
   1's output, not its input. (§9.2's answer, `MIT OR Apache-2.0`, removes that
   obstacle from Phase 1 onward.) And Mode C emits *SudachiDict* long units, not
   JMdict headwords.

   **How it was measured.** §9.6 swapped one Vibrato dictionary for another and
   re-scored, which is a file path. Sudachi is a different engine with a
   different dictionary format, so the equivalent experiment means writing a
   sudachi-backed hints path — Phase 1 work, on a crate that cannot enter the
   tree yet. What answers the question without any of that is the quality of the
   boundaries themselves, since §4.3's whole claim is that a hint source's job
   is to supply boundary evidence to the DP. **A hint source can only help where
   it has the boundary**, so boundary recall bounds what any wiring could
   achieve. Interior boundaries only; 0 and `len` are excluded because every
   tokenizer emits them. `corpus_staging/measure_sudachi.py`, against 2,013
   interior gold boundaries and 1,042 gold spans of two or more characters.

   | source | recall | precision | F1 | gold span exact | gold span uncut |
   | --- | ---: | ---: | ---: | ---: | ---: |
   | jparser (hints off) | 0.8614 | 0.8206 | 0.8405 | 0.5902 | 0.7879 |
   | **Vibrato / IPADIC** | 0.9707 | **0.8426** | **0.9021** | 0.6708 | **0.7015** |
   | sudachi Mode A | **0.9886** | 0.8073 | 0.8888 | 0.6478 | 0.6603 |
   | sudachi Mode B | 0.9856 | 0.8232 | 0.8971 | **0.6727** | 0.6910 |
   | sudachi Mode C | 0.9796 | 0.8254 | 0.8960 | 0.6603 | 0.6996 |

   *exact* is the share of multi-character gold spans emitted as exactly one
   token — the §5.2 compound question. *uncut* is the share with no tokenizer
   boundary strictly inside, which is the §4.2 benign-coarsening distinction
   applied to the hint source: a tokenizer that merges two gold spans is still a
   usable constraint, one that cuts inside a gold span is not.

   **Sudachi has better recall and worse precision, and loses on F1.** Every
   mode finds more of the gold boundaries than Vibrato — Mode A misses only 23
   of 2,013 — and every mode pays for it in spurious boundaries, emitting 2,389
   to 2,465 against Vibrato's 2,319. Vibrato wins F1 outright.

   **Mode C is not the best mode, and the reason is the one this question
   anticipated.** Its exact-match rate (0.6603) is *below* Mode B's and below
   Vibrato's. Mode C's long units are SudachiDict named entities, and they
   swallow JMdict headwords whole: 41 gold spans disappear inside a longer Mode C
   unit, including `武田`/`信玄` inside 武田信玄, `東京都`/`新宿` inside 東京都新宿区,
   `JR`/`東日本`, `湘南`/`新宿` inside 湘南新宿ライン, and `17`/`290` inside
   17万290. The long-unit vocabulary is real and it is not JMdict's.

   **The two engines fail on disjoint classes, which is the finding worth
   keeping.** Mode C matches 55 gold spans exactly that Vibrato misses and
   misses 43 that Vibrato matches. The 55 are dominated by unregistered
   reduplicated mimetics — `ぱひゅぱひゅぱひゅ`, `どぷんどぷんどぷん`,
   `ぬちゃぬちゃぬちゃ` — which sudachi's OOV handling groups into one unknown
   token where Vibrato splits them. That is not a marginal class for this
   project: §5.3's measurement over the whole `vn_data` pool found reduplicated
   mimetics are ~65% of real jparser-versus-Vibrato conflicts. The 43 are the
   named-entity compounds above. So the honest summary is that neither engine
   dominates; sudachi is better at unknown-word grouping and worse at not
   inventing compound boundaries.

   **What decides it is a number in the same table that is not about sudachi at
   all.** jparser's own boundary recall is 0.8614 while every tokenizer sits at
   0.97 or above. jparser is missing 279 gold boundaries that the hint source
   already has. Switching hint sources is worth at most 1.8 points of recall;
   using the existing hints as §4.3 describes is worth eleven. Phase 1 should
   spend its effort there.

   **Revisit in Phase 2, and only on evidence.** If Phase 1's measured error
   analysis shows unregistered mimetic runs surviving as a residual class, the
   sudachi comparison is the place to look, and by then the relicence has landed
   so the Apache-2.0 leg is no longer an obstacle.

   **Caveats, both of which strengthen the conclusion rather than weaken it.**
   This measures boundary quality, not end-to-end span accuracy; a
   sudachi-backed hints path could still score differently, though it cannot
   recover a boundary the tokenizer never emitted. And the corpus is
   disagreement-sampled on `Bj \ Bv ≠ ∅`, which selects sentences where jparser
   cut somewhere Vibrato did not — so Vibrato's recall is measured on ground
   chosen adversarially to it, while sudachi played no part in the sampling and
   is measured neutrally. Vibrato still wins.

   **Provenance.** SudachiPy 0.6.11 with SudachiDict-core, in a venv outside the
   repository. SudachiPy 0.6.x is not a second implementation — its home page is
   `github.com/WorksApplications/sudachi.rs/tree/develop/python` and it is the
   Python binding over the Rust crate, so this measures sudachi.rs itself with no
   port-parity assumption. Nothing was distributed and no sudachi code entered
   the tree. Raw output in `corpus_staging/sudachi_result.txt`.

---

## 10. Decision log

| Decision | Rationale |
| --- | --- |
| Keep a DP under every engine option | Compound preservation (`東京都`) requires scoring JMdict matches, not tokenizer output. A tokenizer's N-best enumerates morphemes over IPADIC/UniDic, not JMdict segmentations — it is a candidate generator, not the answer (§4.1) |
| Study LunaTranslator, adopt nothing | GPL-3.0, so no reuse. Its thin-tokenizer + delegated-dictionary design would delete the conjugation chain and sense ranking that are ClipYomi's reason to exist (§4.4) |
| Reject cross-engine consensus voting | JMdict longest-match and a deinflector share one dictionary — correlated voters echo rather than vote. Ties need a tiebreak, which rebuilds a cost model. k-best surfaces the same disagreements with a principled ranking |
| Reject boundary F1 | Punishes JMdict compound preservation (§5.2) |
| **Span accuracy inherits boundary F1's defect, and it was not noticed** | Measured by `examples/boundary_diag.rs` over the gold corpus: of the 266 gold boundaries Vibrato supplies and jparser drops, ~45% are jparser merging into a real JMdict multi-word expression or vs-noun headword (音がする, ことになっている, 首を振る, これまでに) and another 7% into a numeral+counter date headword (2月, 18日). The corpus was adjudicated at the tokenizer's short-unit granularity, so the metric penalises exactly the behaviour §5.2 calls the reason the DP exists. Reading the full list rather than a sample then found a second defect class inside the merges: ~10.5% are jparser absorbing a particle into the next word and matching a kana-unified absurdity (`は`+`いた` as 履く, `も`+`もう` as 揉む, `に`+`なる` as 荷なう). With the 13.5% fallback bucket that is ~64 boundaries of real defect, not 36 |
| **Make Vibrato boundaries decisive, and treat this as §4.3's evidence** | The particle-absorbing merges above all have a Vibrato boundary at exactly the place jparser overrode. That is the measured case for §4.3 — the strongest Phase 1 evidence Phase 0 produced, and a concrete example list to test a new cost model against (handoff §9.5) |
| **Answered: add compound-tolerant span accuracy, keep strict beside it** | Costs 16.5 points on this corpus (0.6810 strict against 0.8465 tolerant, hints off). Chosen over re-adjudicating the corpus at JMdict granularity, which is expensive and edges toward the §7.2 tautology of a corpus that agrees with the engine by construction; and over simply accepting the strict number, which leaves Phase 1's done-when clause rewarding the removal of compound preservation. It also revealed the hint benefit is understated: +2.13 points tolerant against +1.53 strict (§5.2) |
| Reject `bunpo`, `deinflector`, `yomichan_rs` | GPL-3.0 lineage or provenance risk (§3.4) |
| Write the conjugation table rather than port or depend | No permissive upstream exists (§3.4); conjugation rules are facts, and IPADIC supplies the forms permissively (§3.3) |
| Phase 0 before any rewrite | A clean-room rewrite is different-by-construction; without a baseline, "better" is unfalsifiable (§5.1) |
| Reject the unified single-stage lattice (ULLE) | Proposed in [`2026-08-27-engine-v2-ulle-review.md`](2026-08-27-engine-v2-ulle-review.md): merge IPADIC morphemes, JMdict entries and deinflected spans into one lattice scored by the tokenizer's bigram matrix. The matrix is indexed by IPADIC context IDs and JMdict entries have none, so every entry needs a synthesised `(left_id, right_id)` — a large hand-authored table with no permissive upstream, which is the artefact the proposal exists to avoid. Without it the design collapses back into the two-objective engine it replaces. §4.3 states the arbitration rule instead |
| Reject Eppstein's algorithm for k-best | Same review. The lattice is a DAG, so exact k-best is a backward-Viterbi potential plus a priority queue over partial paths — about fifty lines, and what MeCab's N-best already does. Eppstein's persistent 4-ary heap buys asymptotics that never bind at tens of nodes and k≈3, at a much higher chance of being wrong |
| Reject CI provenance auditing that diffs against ta-old | Same review proposed continuous AST comparison against the upstream repository. That requires the protected expression present in CI and in the workspace, which is the one fact §3.5 must be able to deny. If a mechanised check is wanted, fingerprint the upstream once from outside the clean room and compare against the fingerprints |
| Target `MIT OR Apache-2.0` | The Rust ecosystem default, so no downstream has to reason about an unusual combination; the Apache leg adds a patent grant no §3.3 source supplies; and it is the only option that keeps §9.7 open, since MIT-only would foreclose Apache-2.0-only sudachi.rs forever and Apache-2.0-only would recreate the GPL-2.0-only incompatibility in the other direction (§9.2) |
| Stay on Vibrato; do not adopt sudachi.rs in Phase 1 | Measured out-of-tree on the gold corpus: sudachi has higher boundary recall in every mode and lower precision in every mode, and loses on F1. Mode C is worse than Mode B because its SudachiDict long units swallow JMdict headwords — 41 gold spans vanish inside them. The two engines fail on disjoint classes, so neither dominates. The number that decides it is jparser's own 0.8614 recall against every tokenizer's 0.97+: the headroom is in using the hints, not in changing them (§9.7) |
| Correct the model-adjudicated corpus rather than discard it | The author's review found 14 wrong spans in 2,163 (0.6%), from two causes that are both mechanical and both fixable in the scratch tables: null reasons written against a truncated candidate list, and one per-surface entry decision applied to a surface whose correct answer varies by context. Neither is a reason to distrust the other 2,149 (§5.3 note) |
| Stay on IPADIC; do not adopt UniDic | Measured at the Phase 0/1 boundary on the gold corpus: UniDic scores one span worse and slightly worse on entry top-1, for 199 MB against 46 MB and a per-dictionary reading-column property IPADIC does not need (§9.6) |
| Treat the tokenizer's reading column as a per-dictionary property, not a constant | IPADIC's reading is at field 7 and UniDic's at 9, and reading IPADIC at 9 silently disables every hint and scores exactly the no-hints baseline. A wrong constant is invisible in both metrics (§9.6) |
| Sample the gold corpus against Vibrato, not against hints-on/hints-off | The hints pair produces the same wrong parse on `くるまではこう`, so it cannot surface the bug success criterion 3 names (§5.1). Vibrato disagrees with jparser there, which is exactly the signal wanted |
| **Commit the model-adjudicated corpus, labelled for exactly what it is** | The alternative to promoting it was not a better corpus but no baseline, and Phase 1's done-when clause cannot be evaluated without one. What makes it safe to promote is that the risk is stated where the number is read rather than hidden: `gold_sources.md` records that entry decisions were author-reviewed and mechanically audited (262 decisions, 0 POS flags) while the span decisions were not, and names the 54 sentences carrying a cut neither segmenter proposed. `gold.jsonl` is append-only and last-wins, so a correction is one appended record plus a re-run — the corpus is frozen for comparability, not permanently (§7 Phase 0) |
| Sample on refinement, not on boundary inequality | Sample a sentence when jparser proposes a boundary Vibrato does not (*Bj* \ *Bv* ≠ ∅). Plain inequality selects nearly the whole pool, because jparser merging a Vibrato token pair into one JMdict entry is compound preservation working — sampling on it fills the corpus with sentences where jparser is right. Measured over 66k capture lines: 36% selected, and it is the 36% carrying the real conflicts (§5.4) |
| List candidate entries by `ent_seq`, never by the ranker's own score | A rank-ordered candidate list anchors the adjudicator on the answer being graded, so entry top-1 would measure the parser against itself. Sorting by a stable id that carries no quality signal costs nothing and keeps the metric independent (§5.4) |
| Label whole sentences, and treat `ent_seq: null` as a positive label | Labelling only the disputed spans leaves span accuracy without a stable denominator, and treating "no entry belongs here" as an unlabelled gap rewards a parser for emitting fewer entries. Both are recorded claims instead (§5.3) |
| Read Phase 0's "no code under `crates/jparser/src/`" as "no change to the parser library" | Read literally the rule forbids Phase 0's own deliverable, since §5.4 specifies the harness as `jparser-cli` subcommands and the bin lives under that path. Two exceptions, no others: `src/bin/jparser-cli*`, and one additive read-only `entry_id` field on `Entry` without which `score` cannot compare entry decisions at all. Parsing behaviour is untouched (§7 Phase 0) |
| Filter Aozora on content, not on the 新字新仮名 catalogue flag | The flag describes the edition. Classical passages quoted inside a modern edition and kana iteration marks both leak through, both segmenters flail on pre-war orthography, and an unfiltered sample therefore returns disproportionately many of them — a distribution ClipYomi never sees and would then be tuned against. A work usually has several catalogue rows and the 旧字旧仮名 edition often sorts first, so "the first matching row" silently violates the filter (§5.3) |
| Split Phase 1 into 1a and 1b | §7.3's label mapping is invisible to both Phase 0 metrics, and ~900 ungated lines inside a 3,300-line phase is the silent-degradation shape handoff §9.6 already caught once. 1b needs a gate of its own, and conjugation is finite and generative, so it is far cheaper than a corpus (design §1, §2) |
| 1a owns the chain's span shape; 1b owns only the English | The recombiner has two outputs. Its span shape is the DP's merge decision and is measured by span accuracy — and by the tolerant metric, which over-forgives exactly that shape. Only the string is ungated (design §2.1) |
| 1b's expectation table is written before the formatter, from a grammar reference | A table written beside the code agrees with it by construction — the same trap §5.4 avoids by never showing the adjudicator a rank-ordered candidate list. Generate the class × chain grid mechanically for coverage; hand-write only the English column (design §2.2) |
| 1a's gate adds the §9.5 merge conditions | "Both metrics at or above baseline" can pass while the defect class survives, because compound-tolerant accuracy credits those merges. Merge count becomes a ceiling and the named surfaces must be gone (design §2.3, §10.3) |
| Side-by-side staging: the new engine lands as new modules under new names, the nine hard-taint files keep compiling untouched | Master plan §5.1 already needs both engines alive from Phase 1 on, clean-room is then enforced by construction rather than willpower, and incremental measurement survives (design §4.1) |
| The switchover commit belongs to 1b | Switching at the end of 1a leaves a window where the engine emits no conjugation label — the app without its distinguishing feature. Deferring keeps Phase 1's done-when whole and lands the relicence in one commit (design §4.1) |
| v2 builds its own index; the format does not change | `verb_type` ids are table-relative and the header fingerprint already rejects a mismatch, so the existing guard prevents a cross-read for free. v2 emits no generated stems, so the field is present-and-unused rather than a format difference (design §5) |
| Deinflection at parse time, via a reverse suffix trie | One mechanism instead of a build-time generator plus a parse-time recursion; one origin for 1b's chain; a smaller index; and the `する`/`来る` empty-stem special case disappears. Costs a deviation from this section's original Size table, recorded in Phase 1a's Size note above (design §6) |
| Hints supply boundaries only, never match candidates | `parse` takes `hints: Option` behind a feature gate with `--no-default-features` enforced, so the table is the deinflection path regardless. Base-form candidates would make entry top-1 hint-dependent and 1b's input non-uniform, for a contraction win that can be revisited on evidence in Phase 2 (design §6.1) |
| Cost accrues per character, not per match | A per-match constant is what makes `はこう` beat `は` + `こう`, and the interior constraint does not forbid that merge (§4.3). Removing the bias structurally is the only form of the fix that satisfies success criterion 3 (design §7.1, §7.2) |
| Price a `PRONOUNCE`-only match that crosses a tokenizer boundary | Every one of handoff §9.5's ~28 spurious merges is a reading-form match of a kanji headword; every legitimate merge in handoff §9.1 is kanji-bearing. The flag is already computed and already on the wire, and kana-only words are `PRIMARY` so they are not caught. Handoff §9.6's refusal of a metric heuristic is unaffected (design §7.3) |
| Commit the §9.5 surfaces as a fixture | The gate must run from a clean clone. They are this project's own measurement, carry no ta-old expression and no captured text, and the promotion is the one `gold.jsonl` already received (design §10.3) |
| v1's snapshot is not promoted into v2's gate | It pins the port's behaviour, bugs included, as a specification. §5.1 makes v1 a disagreement source, not an oracle (design §10.4) |
| The IPADIC CSVs are fetched out of tree; the generated table is committed | The shipped Vibrato asset is a compiled dictionary and yields no lexicon. IPADIC's licence permits reproduction with notice retention, and committing generator plus output keeps the table regenerable rather than hand-edited (design §6.2) |
| **Compound-tolerant span accuracy is vindicated by Phase 1a's own numbers** | Before tuning, v2 hints-on scored 0.7947 strict against baseline's 0.6963 — a 9.8-point apparent improvement — while tolerant *fell* (0.8678→0.8003) and the merge count collapsed from 173 to 6. Without the second metric and the merge-count guard, that would have read as a large win while compound preservation had been destroyed — exactly the trap the row above ("Answered: add compound-tolerant span accuracy") exists to catch, now observed in a real phase rather than merely anticipated (Phase 1a handoff, 2026-09-01, §2) |
| Phase 1b split into 1b-i (formatter + gate) and 1b-ii (switchover) | 1a's metric gate is open on four cells; the switchover decision wants the formatter working and the numbers in hand, not 480 ungated lines in the relicence commit (1b-i design §1) |
| Label register is learner-standard; られる/れる is "Passive/Potential" | The product is a learner tool and §7.3's own example is in this register. Ichidan られる is ambiguous and the parser cannot resolve it, so the label says so (1b-i design §3) |
| `ConjStep.aux` records the auxiliary lemma | 一段 is shared by せる/させる/れる/られる and 不変化型 by ん/う/よう/て/で; class and form cannot name the auxiliary (1b-i design §2) |
| The label fixture is hand-written to depth 1 in full, curated deeper, with a mechanical completeness test that asserts no English | Depth 2 is 1,028 distinct tuples — past what a reviewer reads rather than skims; asserting compositional English at depth 2 would test the formatter against its own rule (1b-i design §4) |
| **1b-ii lands in four stages — prep, switchover, delete, relicense — not one commit** | The rewiring is genuinely indivisible (dropping `verb_type` breaks v1's builder; v2's fingerprint fails every v1-built test index; a fresh `WordFlags` collides with `record.rs`'s on the shared `Entry.flags`), but the deletion and the licence flip are not. Separating them buys a commit in which every ported line is gone and the tree is still `GPL-2.0-only` — the licence invariant made observable rather than asserted (1b-ii design §1) |
| Re-seat the prep/switchover seam at what v1 tolerates | An earlier draft put the relocations in the prep commit "with v1 still the default and still compiling", which cannot be built. The alternative was a throwaway `WordFlags` conversion layer with bit values reverse-engineered from v1 at run time, purely to keep a commit boundary — code the next commit would immediately remove, and reverse engineering the fence forbids (1b-ii design §1) |
| Route a stale-index startup error to the download screen | After the fingerprint bump every user upgrading with a v1-built index gets `IndexError::ConjugationMismatch`, and `is_missing_dictionary` mapped `StartupError::Index(_)` to `false` — a fatal error screen for the one condition the download path already repairs. One match arm over `ConjugationMismatch \| VersionMismatch`, which are exactly the errors `ensure_dictionary` rebuilds on, so no new UI and no new command (1b-ii design §3) |
| Index format 4: `StoredRecord` loses `verb_type` | v2 emits no generated stems, so the field was written `None` on every record. The header fingerprint already rejects a cross-read, so dropping it costs no new guard (1b-ii design §2) |
| `flags_from_worker` dissolved rather than argued | §3.2's one borderline taint entry, documented as a port of `Dictionary.cpp:1115-1126`. Deleted with `BoundaryFlags` at 1b-ii; `Boundaries` asks where a token ends instead, so there is nothing left to relicense or defend (§3.2, §4.3, 1b-ii design §5) |
| `vk` maps a second class, `カ変・来ル`, with a table-derived reading rule | Kanji-written 来る did not deinflect at all under v2 — `来た` came out as two unmatched spans — because `JMDICT_POS_CLASS` mapped `vk` only to `カ変・クル`, whose suffixes are the whole kana word. Shipping that as the default would regress one of the language's two irregular verbs. The stem reading comes from `カ変・クル`'s same-named form, whose suffix over an empty stem *is* that reading (連用形 き, 未然形 こ, 仮定形 くれ), written from the observable alternation rather than from v1's `kuruHack`. Metric-neutral on all eight cells (1b-ii design §9.1) |
| The たい row reorder is the cheap fix, and the principled one is named not taken | 特殊・タ and 特殊・タイ both emit a bare `た`, the chains tie in `lex::dedup_shallowest`, and `ATTACHES_TO`'s row order decided that *Want-to* survived — so ました labelled "Polite Want-to", wrong on the most common polite form in the language. Moving one row changes which of two equal-depth chains survives dedup and nothing else. Making `たい:ガル接続` non-terminal would delete candidates and can move the DP, so it stays a measured Phase 1a follow-up, as does the godan 仮定形/命令ｅ tie (1b-ii design §9.2) |
| `romaji --particle` removed with the switchover | The flag existed to expose v1's particle-aware romaji path. v2's `engine::romaji` has no such mode, so the flag could only have reported a value it no longer computes (1b-ii design §4) |
| **Phase 1's metric clause is superseded, not met** | The tolerant gap is the structural JMdict-priority ceiling §7 Phase 3 already assigns to the frequency layer, so no tool inside Phase 1 can satisfy the clause as written; leaving it open indefinitely would be the same silent-degradation shape the phase split exists to avoid. The numbers stand beside the amendment: hints on, v2 is +10.7 points strict, −1.6 tolerant, −0.9 entry top-1 against v1. Strict measures the particle-absorbing defect class this project exists to fix (1b-ii design §10) |
| `cargo fmt` runs once, in the last commit of the phase | The clean-room quarantine forbade it while the hand-laid conjugation table was still in the tree, and running it earlier would also have destroyed the evidence that it *had not* run — 12 of the drift sites that vanished at the switchover did so because their files became orphaned, which is a fact only a never-formatted tree can show. 49 sites across 15 files, absorbed in one mechanical commit (1b-ii design §1, §8) |
| Measure the oracle@∞ ceiling before building the k-best DP | oracle@k needs the DP; the ceiling needs none and bounds it for every k. Had it been small, Phase 2 would have shipped nothing without a line of engine code (Phase 2 design §2) |
| Phase 2 alternates come from the constrained lattice only | One DP, one oracle number, k = 1 provably unchanged; the 63 `is_allowed`-suppressed spans are the constraint's price, the judgment Phase 1a §7.5 already made (Phase 2 design §2.3) |
| Phase 2's gate is tolerant, +55 spans, hints on | 92 of the 196 reachable strict spans are compounds the corpus splits; strict would reward re-splitting them. Half the tolerant headroom is the line below which a cost-model pass plausibly does as well without a UI (Phase 2 design §8) |
| Reading identity is the matched span set; dominated readings are dropped | Same-span entries and unmatched splits collapse into one reading; a reading that only un-matches a cheaper reading's word is a worse copy, not an alternate (Phase 2 design §3.2, §3.2a) |
| **Phase 2's tolerant gate is vindicated by its own measurement** | Strict oracle@3 hints on gained +109 spans over strict top-1 (1737→1846); tolerant gained only +40 (1843→1883). The strict number alone would have cleared the same 55-span bar, but the gain is dominated by re-splitting the JMdict compounds the corpus adjudicates at short-unit granularity and the engine correctly keeps merged — exactly what the tolerant choice was made to avoid crediting, decided before either number existed (Phase 2 design §8; handoff §3) |
