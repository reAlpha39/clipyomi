# Engine v2 Phase 0 — Harness and Baseline (Design)

**Date:** 2026-08-28
**Master plan:** [`2026-08-27-engine-v2-master-plan.md`](2026-08-27-engine-v2-master-plan.md) §5, §7 Phase 0, §9.1
**Status:** Design draft, not approved, not implemented

---

## 1. Goal

Build the instrument that tells "different" from "better", and take its first
reading.

Every phase after this one produces a different-by-construction result: the
clean-room rule forbids reproducing the old cost model, so "no regressions
against jparser" is not available as a gate (master plan §5.1). What replaces it
is a labelled corpus and two numbers measured over it. Phase 0 delivers the
corpus, the measurement, and the current engine's score. Nothing else.

The recorded baseline is the contract Phase 1 and Phase 2 are held to. If it is
wrong, optimistic, or irreproducible, every later phase is unfalsifiable — which
is why §5 argues subset-index equivalence explicitly and §8.4 gates it with an
empirical check rather than the argument alone.

## 2. Scope

**In:**

- `jparser-cli diff | label | score`, per master plan §5.4.
- The gold JSONL label format (§4) and the provenance manifest beside it (§4.4).
- A genre-extended gold corpus, disagreement-sampled against Vibrato (§3, §6).
- A committed JMdict subset for the new corpus, so `score` runs offline from a
  fresh clone (§8.2).
- A recorded baseline of span accuracy and entry top-1 for the current engine,
  hints-on and hints-off (§8.3).
- One additive field on `jparser::Entry` (§5.4), and the bin's move from a file
  to a directory (§9.1).

**Out:**

- Any change to parsing behaviour. No edit to `segment.rs`, `matcher*`,
  `rank.rs`, `stem.rs`, `conjugation.rs`, `hints.rs`, `index/*`, `jmdict.rs`,
  `record.rs`, `kana.rs`, `romaji.rs`.
- Any change to `tests/fixtures/parse_sentences.txt`,
  `tests/fixtures/jmdict_subset.xml`, or `tests/parse_snapshots.rs`. The
  existing snapshot suite is left exactly as it is; the gold corpus is a second,
  independent artefact (§8.2).
- Oracle@k. Meaningless before k-best exists (master plan §5.2).
- IPADIC-versus-UniDic evaluation (master plan §9.6) and the out-of-tree
  sudachi.rs measurement (§9.7). Both are named as Phase 0-adjacent in the
  master plan; both are *uses* of the harness, not parts of it, and both are
  cheaper once `score` exists. Deferred to the Phase 0/1 boundary (§13).
- Any UI. Any network call inside `score`.

### 2.1 Reading of "no code under `crates/jparser/src/`"

Master plan §7 says Phase 0 writes no code under `crates/jparser/src/`. Read
literally that forbids Phase 0's own deliverable: §5.4 specifies the harness as
`jparser-cli` subcommands, and `jparser-cli` lives at
`crates/jparser/src/bin/jparser-cli.rs`.

The rule is therefore read as **no change to the parser library** — the file
list in §2 above. Two amendments are requested against that reading, both
recorded in §14:

1. `src/bin/jparser-cli*` is where the harness goes, as §5.4 requires.
2. One additive, read-only field on `jparser::Entry` in `src/lib.rs` (§5.4).

No third amendment. If either is refused, §5.4 and §9.1 name the fallbacks and
what they cost.

---

## 3. Corpus sourcing — master plan §9.1, answered

### 3.1 Five tiers, four committed

| Tier | Source | Committed | Genre it covers |
| --- | --- | --- | --- |
| A | Aozora Bunko, 新字新仮名 editions only | yes — public domain | literary prose, long sentences, 「」 speech |
| B | ja.Wikinews (CC BY 2.5), ja.Wikipedia (CC BY-SA 4.0) | yes — own notice | news register, proper nouns, numerals, katakana loans |
| C1 | hand-written pastiche | yes — project licence | visual-novel dialogue, casual and contracted speech |
| C2 | real lines from `vn_data/` | **no** | the true input distribution |
| D | capture and OCR noise, derived from A–C1 | yes — follows its source line | OCR noise, vertical-text artefacts |

The master plan's §9.1 options were author's-reading-history, Aozora, or
pastiche. This design takes all three and adds two sources the plan did not
consider, for two specific gaps.

**Aozora carries no news register.** It is literary prose, roughly 1900–1955.
The plan's §5.3 asks for news register as a distinct genre, and answering it
with 1920s fiction would leave that genre unmeasured under a label claiming it
was covered. ja.Wikinews and ja.Wikipedia carry it in modern orthography, with
the proper nouns, numerals, and katakana loanword density that go with it. The
licence pattern is already in the tree and needs no new decision:
`tests/fixtures/jmdict_subset.xml` is CC BY-SA 4.0 data carrying its own notice
header, not relicensed, kept as a separately-licensed asset because the crate is
GPL-2.0-only and CC BY-SA 4.0 is one-way compatible with GPL v3 only. Tier B
files follow that pattern exactly.

**OCR noise is not deferred.** The plan's §5.3 lists it and §9.1's lean accepted
thin coverage until real clipboard capture exists. Two reasons not to: it is
the genre most likely to break silently, and it is the cheapest to cover. Tier D
is ~40 lines derived by hand from already-committed clean lines in A–C1, with
the derivation table documented in the corpus manifest header:

- **Real capture artefacts**, observed in `vn_data/`: ASCII `......` where
  `……` belongs, halfwidth `?` and `!`, U+3000 ideographic space mid-sentence,
  speaker-name prefixes (`テスト太郎「…」`), `――` em-dash runs, halfwidth digits in
  Japanese context (`1つ`).
- **Synthetic OCR confusables**, which text hooking cannot produce and only real
  OCR does: 力/カ, 口/ロ, 一/ー/丨, ニ/二, 八/ハ, 卜/ト, dropped dakuten
  (`ば`→`は`), full/half-width mixing, spurious intra-word space.
- **Vertical-text artefacts**: `ー` rendered as `｜` or `丨`, `〜` as `～`.

No tooling. Forty hand-derived lines is a smaller artefact than a corruption
script plus its output, and the committed lines *are* the reproducible artefact
either way. A script arrives only if tier D ever needs regenerating at scale
(§13).

A tier D line derived from a tier B source is a derivative of CC BY-SA material
and stays CC BY-SA, attributed to the same source in the manifest. Derived from
tier A it is public domain; from C1 it is project-licensed.

### 3.2 `vn_data/` is a sampling pool, not a corpus

`vn_data/` holds two LunaTranslator caches — `artificialtrans(id, source,
machineTrans, origin)`, UTF-8 — with **67,469 source lines** of real
visual-novel dialogue. The directory is already in `.gitignore`. Read the
`source` column; `origin` stores each line doubled and is unusable.

That text is copyrighted and cannot be committed, which is exactly the
constraint §9.1 identified. It is used two ways, neither of which commits it:

1. **As the disagreement pool for tier C1.** Run `diff` over all 67k lines
   first, read which register classes actually generate conflicts — contracted
   verb forms, sentence-final particle stacks, elongation marks, speaker
   prefixes — then hand-write tier C1 to cover *those* classes. Register and
   style are not copyrightable; transcribed lines are. This gets
   real-distribution coverage into a committed corpus without committing a word
   of it.
2. **As tier C2, a local gold file.** Same JSONL format, same `score` command,
   `crates/jparser/tests/fixtures/gold_local.jsonl` added to `.gitignore`. Two
   baselines get recorded (§8.3); the committed one is the reproducible
   contract, the local one is the sanity check that the committed corpus has not
   drifted away from the real distribution.

### 3.3 The Aozora orthography trap

Not in the master plan, and it would have quietly poisoned the sample.

Aozora's pre-war editions use 旧字体 and 旧仮名遣い. Both segmenters flail on
them, so a disagreement sampler run over unfiltered Aozora returns
disproportionately many 旧仮名 sentences — a distribution ClipYomi never sees.
The corpus would then be measuring the engine on orthography no user pastes,
and Phase 1 would be tuned against it.

Two constraints on tier A:

- **新字新仮名 editions only.** Aozora publishes both variants for many works;
  the card page states which. Reject 旧字旧仮名 outright.
- **Cap sentences per work**, at 8. Ten-plus works rather than two, so no single
  author's idiolect dominates a tier.

Aozora ruby (`《…》`), reading marks (`｜`), and editorial annotations
(`［＃…］`) are stripped before a line enters the pool. Note the collision:
`｜` is both an Aozora ruby delimiter and a tier D vertical-text artefact.
Stripping happens at pool-construction time, before tier D derivation, so a
`｜` surviving into a committed line is a deliberate tier D artefact and never
residue.

### 3.4 Sizes

Target **250 committed sentences**: A 80, B 50, C1 80, D 40. Plus ~100 local
tier C2 lines.

That is the low end of §5.3's "a few hundred", chosen against labelling cost:
full-sentence gold (§5.1) at roughly ten spans per sentence is ~2,500 span
decisions, which is a few hours with the fast path in §7.1. Growing the corpus
later is additive — append to the JSONL, re-run the extractor, re-record the
baseline — so there is no reason to over-buy up front.

**The committed subset's size needs measuring, not estimating.** 30 sentences
produce a 795 KB `jmdict_subset.xml` today, but the extractor keeps every entry
whose surface is any substring of the corpus, and that grows with vocabulary
coverage rather than with sentence count. Measure it at the first 100 labelled
sentences (task 8, §10). If the 250-sentence projection exceeds **8 MB**, cap the corpus at
whatever holds under 8 MB and record the cap in the manifest. Do not compress
it: a gzipped fixture buys a dependency and loses reviewability, and a corpus
small enough to be reviewed is worth more than one large enough to need
compressing.

---

## 4. Formats

### 4.1 Gold corpus — `gold.jsonl`

One JSON object per sentence, one line each:

```jsonl
{"text":"くるまではこう","source":"c1:pastiche-014","spans":[{"start":0,"len":3,"ent_seq":1207610},{"start":3,"len":1,"ent_seq":2028980},{"start":4,"len":1,"ent_seq":2028920},{"start":5,"len":2,"ent_seq":1288860}],"decision":"e"}
```

- `text` — the sentence verbatim, and the record's identity.
- `source` — `tier:id`, resolved through the manifest (§4.4).
- `spans` — a contiguous cover of `text` in ascending `start` order: every
  character belongs to exactly one span. The same invariant
  `jparser::ParseResult` documents, so a gold record and a parse result are
  directly comparable without normalisation. Offsets are **character** offsets,
  never bytes, matching `Segment::start` and `Segment::len`.
- `ent_seq` — the JMdict entry the span should resolve to, or `null`.
- `decision` — which key the adjudicator pressed to settle this record's spans
  in `label`: `"j"` (blessed jparser's), `"v"` (blessed Vibrato's), `"e"`
  (retyped the cuts). Absent (not `null`) on a hand-authored record that no
  adjudicator produced. See §7.1 for why it is recorded.

`null` means *no JMdict entry belongs first here* — the span is a function word
with no useful entry, or an unmatched run, or the adjudicator ruled that nothing
in the dictionary fits. It is a positive label, not missing data. A `null` span
counts in span accuracy and is excluded from entry top-1 (§5.2).

Entry IDs rather than boundary conventions, per master plan §5.3: `ent_seq` is
stable and unambiguous, and jparser's internal `entry_id` *is* `ent_seq`
(`jmdict.rs:280` reads it straight off the element).

### 4.2 Append-only, last-wins

`label` only ever appends. A later record for the same `text` supersedes every
earlier one; readers take the last. So an interrupted session loses nothing, a
correction is another append, and no reader ever rewrites the file.

`score` reports the count of superseded records it dropped. A file that is 30%
supersessions is a signal the labelling pass was confused, and silence about
that would hide it.

Sentences are deduplicated by `text` at pool-construction time, so two distinct
sources never produce two records that look like a supersession.

### 4.3 `diff` output — `disagreements.jsonl`

```jsonl
{"text":"くるまではこう","source":"c2:riruka-018233","jparser":[0,3,4,7],"vibrato":[0,2,4,5,7],"conflicts":[3],"unknown_tokens":0}
```

Boundary offsets, not spans: that is what the sampler compares (§6.1). Both
arrays start at 0 and end at the sentence's char length, so a cover is implied
and the record stays short.

- `conflicts` — the jparser cuts that are not Vibrato cuts. Non-empty by
  construction; this is the sampling signal.
- `unknown_tokens` — Vibrato tokens whose IPADIC reading field is `*`, absent,
  or empty. The count, not the positions. It tells the adjudicator how much of
  the "disagreement" is the tokenizer guessing (§6.2), which matters most on
  tier D where guessing is the normal case.

`label` reads this file and writes `gold.jsonl`. The two formats are separate
because they answer different questions and the diff file is disposable.

### 4.4 Provenance manifest — `gold_sources.md`

One Markdown table beside the corpus, one row per `source` id: tier, work or
article title, author, Aozora card number or article URL and revision id,
licence, retrieval date. Plus the tier D derivation table (§3.1) in the header,
and the tier A stripping rules (§3.3).

Markdown, not JSON: nothing parses it, a reviewer reads it, and it carries the
CC BY-SA and CC BY attribution that tier B legally requires. `tests/fixtures/
README.md` is the precedent and gains a pointer to it.

No fetcher is written for tiers A and B. Selection is a one-time manual job and
the manifest is what makes it auditable — a fetch script for a job run once is
the definition of speculative tooling. The one extraction that *is* repeated is
`vn_data/` (it grows with reading), so that gets `tools/extract_vn_pool.py`
(§9.3).

---

## 5. Metrics

Two, reported separately and always. Master plan §5.2 fixes the definitions;
this section fixes the denominators, which it left open.

### 5.1 Full-sentence gold, not disagreement-spans-only

Disagreement sampling selects **sentences**. Every span in a selected sentence
is then labelled, and span accuracy is measured over all of them.

The alternative — label only the conflicting spans — is cheaper and produces a
number that cannot serve as a cross-phase contract. Its denominator is "the
hard spans in the corpus", so its difficulty shifts every time the corpus grows,
and a Phase 1 result would not be comparable to a Phase 0 result measured over
a different mix. Full-sentence gold has a fixed denominator and a stable
meaning: *this fraction of the words in this corpus is delimited correctly*.

The cost is paid down by `label`'s fast path (§7.1), not by shrinking the
denominator.

### 5.2 Definitions

Let *G* be the gold spans of a sentence and *P* the parse's segments.

**Span accuracy** = |{g ∈ G : ∃ p ∈ P with p.start = g.start and p.len =
g.len}| / |G|, summed over the corpus. Includes `ent_seq: null` spans — whether
a function word is delimited correctly is exactly as much a span question as
whether a noun is.

**Entry top-1** = |{g ∈ G : g exactly spanned, g.ent_seq ≠ null, and the
matching segment's `entries[0].entry_id` = g.ent_seq}| / |{g ∈ G : g exactly
spanned, g.ent_seq ≠ null}|.

Conditioned on correct spanning because a wrong span has no meaningful entry
list, and restricted to non-`null` gold because a `null` gold span asserts
nothing about ranking.

An exactly-spanned, non-`null` gold span whose matching segment has
`matched: false` or an empty `entries` list counts in the denominator and fails
the numerator. It is a real miss and hiding it would flatter the baseline.

### 5.3 Output

```
$ jparser-cli score <index> <gold.jsonl>
sentences:      250
gold spans:     2481
superseded:     12
span accuracy:  0.8734  (2167 / 2481)
entry top-1:    0.7914  (1502 / 1898)
  by decision:  j 1.0000 (980 / 980)  v 0.7421 (612 / 825)  e 0.8506 (575 / 676)
```

Numerator and denominator beside every ratio, so a corpus change is visible as a
denominator change rather than showing up as unexplained metric movement. No
combined score — master plan §5.2 forbids it, because a change that improves one
metric and harms the other is a finding, not noise.

The `by decision:` line is additive: span accuracy split by the `label`
decision (`j`, `v`, or `e`, §4.1) that produced each gold record, printed only
when at least one record carries one. Ordered `j`, `v`, `e`, then anything else
alphabetically. This is §7.1's fast-path bias made measurable rather than
removed — see that section for why `j`'s number in particular should not be
read as "the parser is right".

`--json` emits the same numbers as one object, including the decision
breakdown and the run's provenance (§8.3: whether `--hints` was used, and the
index generation), for recording baselines into the phase's handoff without
transcribing them by hand.

### 5.4 Why `Entry` must expose `entry_id`

`jparser::Entry` (`lib.rs:94`) carries `headword`, `reading`, `conjugation`,
`pos`, `senses`, `flags` — and no id. `assemble_entry` has `Match::entry_id` in
hand and drops it. Entry top-1 is unmeasurable as the API stands.

Requested amendment: `pub entry_id: u32` on `Entry`, set from `m.entry_id` in
`assemble_entry`. Two lines, additive, read-only, no behaviour change, and it
does not alter what `parse` computes — only what it admits to having computed.

The alternative needs no `src/` change and was rejected: resolve the gold
`ent_seq` through `Index::entry` and compare `(headword, reading)` against
`entries[0]`. Two entries can share a headword and reading — homographs whose
senses JMdict splits — so the comparison silently passes when the ranker put the
wrong one first. That is precisely the failure entry top-1 exists to catch, and
a metric that cannot see its own target case is worse than no metric.

If the amendment is refused, the fallback is the `(headword, reading)` proxy with
its ambiguity recorded in the baseline as a known optimistic bias, and the
affected span count printed alongside. It is strictly worse and should not be
chosen.

---

## 6. `diff`

```
jparser-cli diff <index> <pool.txt> --hints <dic> [--all] [--limit N]
```

`--hints` is required, per master plan §5.4: Vibrato is the second segmenter, so
there is nothing to diff against without it.

### 6.1 Refinement, not equality — the sampler that works

The obvious predicate is "the boundary sets differ". It is unusable, and this is
the section that keeps Phase 0 to hundreds of labels instead of thousands.

jparser emits JMdict long units; Vibrato emits IPADIC short-unit morphemes. They
disagree on most sentences *legitimately*: `東京都` is one JMdict headword and
two IPADIC tokens. Master plan §5.2 rejects boundary F1 for exactly this reason —
it punishes compound preservation, which is a feature. A sampler keyed on
inequality makes the same mistake and selects nearly the whole pool.

The usable predicate is one-directional. Write *Bj* for jparser's cut offsets
(including 0 and *n*) and *Bv* for Vibrato's. Sample the sentence when

> **Bj \ Bv ≠ ∅** — jparser has at least one cut that is not a Vibrato cut.

Three cases, and this predicate splits them correctly:

- **Coarsening**, *Bj* ⊆ *Bv*: jparser merged whole tokens. `東京都` — *Bv* =
  {0,2,3}, *Bj* = {0,3}, difference empty. Benign, excluded, and this is the
  bulk of the pool.
- **Interior split**, *Bv* ⊂ *Bj*: jparser cut inside a token Vibrato
  recognised. Kept — that is the failure the hint mechanism exists to prevent,
  so a sample of it is worth labelling.
- **Crossing**, neither contains the other: kept.

Checked against the case master plan §5.1 says the sampler must surface.
`くるまではこう`, from §4.3's recorded flags: *Bv* = {0,2,4,5,7} —
`くる|まで|は|こう`. jparser produces `くるま|で|はこう`, so *Bj* = {0,3,4,7}. The
cut at 3 sits strictly inside Vibrato's `まで`, so *Bj* \ *Bv* = {3} and the
sentence surfaces. Note also that Vibrato is not right here either — it read
`くる` as a verb. The adjudicator rules for neither, which master plan §5.4
explicitly permits; the pair's job is to raise the sentence, not to answer it.

Output is ranked by |*Bj* \ *Bv*| descending, so the densest conflicts come
first and `--limit N` is a useful stopping point rather than an arbitrary
truncation. `--all` drops the predicate and emits every sentence where the sets
differ at all — kept for one purpose, verifying against a hand-checked sample
that the predicate is not filtering out real conflicts (§11.2).

### 6.2 Raw Vibrato, not `BoundaryFlags`

`diff` constructs its own `vibrato::Tokenizer` and reads `token(i).range_char()`
directly. It does not call `VibratoTokenizer::hints`.

Two reasons. `BoundaryFlags` stores only interior-position booleans and throws
the token ranges away, so token boundaries are not recoverable from it. And
`flags_from_worker` deliberately *skips* tokens whose reading field is `*` — the
unknown-word guard — which is right for hints and wrong for sampling: a sentence
where the tokenizer recognised nothing is a sentence worth looking at.

So `hints.rs` is not touched. The cost is that `diff` counts reading-less tokens
itself and reports `unknown_tokens` (§4.3), so the adjudicator can tell a real
conflict from two systems both guessing — which is the normal case on tier D and
on proper nouns.

`diff` needs the `mecab` feature, and is `#[cfg(feature = "mecab")]`-gated the
way `parse --hints` already is.

### 6.3 `diff` is not part of the offline gate

`diff` needs a full JMdict index and the 46 MB Vibrato dictionary. It is a
corpus-construction tool, run when the corpus grows, and its output is
disposable. Only `score` carries the reproducible-and-offline requirement (§8.1).

Both inputs are already on this machine — `dict/gen-1` and
`hints/current/system.dic` under the app config directory — so no download is
needed to start.

---

## 7. `label`

```
jparser-cli label <index> <disagreements.jsonl> <gold.jsonl>
```

Reads a diff record, prints both parses, takes a decision, appends a gold
record. Explicit output path rather than in-place: the input is disposable and
the output is the artefact.

### 7.1 Two levels, and the fast path

Per sentence, print jparser's spans and Vibrato's tokens aligned under the text,
with the conflicting cuts marked, then:

```
[j] jparser spans   [v] vibrato spans   [e] edit spans   [s] skip sentence
```

Blessing a span set at sentence level is one keystroke, which is what makes
full-sentence gold (§5.1) affordable. `e` drops into per-span editing: split,
merge, adjust a boundary. `s` appends nothing — a sentence the adjudicator
cannot rule on stays unlabelled rather than becoming a guess in the gold file.

Then, per span, the entry choice: a numbered list, `n` for `null`, `b` to go
back. No default. Enter alone is rejected rather than accepting anything.

**The sentence-level decision (`j`, `v`, or `e`) is recorded on the gold
record**, per §4.1. It has to be: `diff` runs with `hints = None` (§6.2), so
`j` blesses the *hints-off* segmentation verbatim. A `j`-blessed sentence
therefore scores 1.0 on span accuracy under `score` hints-off by construction,
before the parser has done anything `hints` should get credit for. That biases
§8.3's hints-on-minus-hints-off comparison — the one number this whole harness
exists to produce — against the hint mechanism, silently, unless the bias is
visible. The fix is not to make `j` blessing rarer or to run `diff` hints-on
(§8.3's per-decision breakdown is worthless if `j` itself is redefined every
time hints change); it is to record which decision produced each record and
report span accuracy split by it, so a corpus that is mostly `j` reads as
mostly rubber-stamped rather than as a strong hints-off baseline. A high `j`
share is evidence about the labelling pass, not about the parser.

### 7.2 Entries are listed by `ent_seq`, not by rank

Non-obvious and load-bearing.

If `label` shows the entry list in the parser's ranked order, the adjudicator
reads top-to-bottom, the plausible first candidate is usually acceptable, and
gold `ent_seq` becomes a copy of the parser's own answer. Entry top-1 then reads
near 1.0 by construction and measures nothing. The whole baseline would be a
tautology, and it would look like a good result.

`label` therefore sorts the candidate list by `ent_seq` ascending. That order is
uncorrelated with the ranking under measurement, so position in the list carries
no signal about what the parser preferred. One `sort_by_key`, and it is the
difference between a baseline and a number that flatters itself.

Nothing in the printed line reveals rank either — no marker, no reordering, no
"(top)" annotation.

### 7.3 Line-buffered stdin, not raw keystrokes

Master plan §5.4 says `label` "reads a keystroke". This reads a line.

A raw-mode terminal reader needs a TTY, and a TTY cannot be driven from a test.
Line-buffered stdin costs one Enter per decision and makes the whole adjudicator
testable by piping a decision script — which is how §11.3 pins the
append-only, last-wins, and `null` behaviours. No `crossterm`, no
`termion`, no new dependency. Recorded as a deviation in §14.

---

## 8. `score`

```
jparser-cli score <index> <gold.jsonl> [--hints <dic>] [--json]
```

`--hints` optional, per master plan §5.4: this is where the baseline pair is
produced.

### 8.1 The offline contract

From a fresh clone, with no network and no downloaded asset:

```
cargo run -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml /tmp/gold-index
cargo run -p jparser --bin jparser-cli -- \
  score /tmp/gold-index crates/jparser/tests/fixtures/gold.jsonl
```

Two committed text files and no binary asset. That is the master plan's "done
when", and it holds because of §8.2.

The hints-on half of the baseline pair needs `hints/current/system.dic`, which
is 46 MB and not committed. So: **hints-off is reproducible from a fresh clone;
hints-on is reproducible from a fresh clone plus one `fetch-hints` run.** Both
numbers are recorded, and the distinction is recorded with them.

### 8.2 Why a subset index scores the same as a full one

`score` runs against `gold_jmdict_subset.xml`, produced by the existing
`tools/extract_jmdict_subset.py`. The claim is that this scores identically to
the full 67 MB index, and it rests on three properties of code Phase 0 does not
touch:

1. **The extractor's closure is a superset of what can match.** `matcher` finds
   candidates at position *i* by `index.prefixes_of(text[i..])`, so any key that
   matches anywhere is a substring of the corpus. Index keys are exactly
   headword surfaces — every `keb` and every `reb` (`record.rs::headwords`) —
   plus generated stems (`index/build.rs`). The extractor keeps an entry if any
   `keb`/`reb` is a corpus substring (rule 1, covering surface keys) or if the
   entry is conjugable and some surface equals *substring + remove-suffix*
   (rule 2, covering stem keys). Every entry that can produce a match at any
   corpus position is kept.
2. **Flags are entry-local.** `PRIMARY` is form position within its entry;
   `COMMON`/`COMMON_LINE` come from that entry's own priority markers;
   `PARTICLE` and `COUNTER` from its own POS and misc. No corpus-wide frequency,
   no global statistic. Removing unrelated entries cannot change a kept entry's
   flags, so the DP's inputs are identical.
3. **Ranking is per-span and entry-local.** `rank::sort_matches` is called per
   span, and its identity is `entry_id` alone (`rank.rs`). Unrelated entries
   absent from the subset were never candidates on that span.

Identical candidate buckets, identical DP input, identical per-span ranking.
Therefore identical spans and identical `entries[0]`.

The argument is stated because it is the load-bearing claim under "reproducibly
and offline". It is not trusted on its own: §11.4 gates it with a measurement.

**A second subset, not an edit of the existing one.** `jmdict_subset.xml` and
`parse_snapshots.rs` are left untouched — the committed snapshots are a
regression asset from Phase 1A, and regenerating them to serve a Phase 0 corpus
would put existing coverage at risk for no gain. The extractor gains the ability
to read `.jsonl` corpora (~8 lines, sniffed on extension) and is run a second
time into `gold_jmdict_subset.xml`. The two subsets overlap; that is cheaper
than the alternative.

### 8.3 The baseline pair

Four numbers recorded, in the phase handoff and in §14's master-plan update.
The table is filled by task 9 and is deliberately empty here — a design cannot
predict its own baseline:

| | span accuracy | entry top-1 |
| --- | --- | --- |
| hints off | | |
| hints on | | |

Hints-on minus hints-off is what the hint mechanism is currently worth — two
numbers, not a corpus sampler (master plan §5.1). Recorded against the committed
corpus. Tier C2's local baseline is recorded in the same table, marked
irreproducible, as the check that the committed corpus has not drifted from the
real distribution.

The index generation and the corpus commit hash are recorded beside them. JMdict
is rebuilt daily and `ent_seq` values are stable but entries do get added; the
committed subset is what freezes the measurement, and naming the artefacts is
what makes a later "the number moved" answerable.

---

## 9. Layout and dependencies

### 9.1 The bin becomes a directory

`src/bin/jparser-cli.rs` is 350 lines with eleven subcommands. Three more
subcommands, JSONL IO, a metric pass, and an interactive adjudicator do not
belong in one file.

```
crates/jparser/src/bin/jparser-cli/
    main.rs      clap definitions and dispatch — the current file plus three
                 subcommands, with no logic of their own
    corpus.rs    gold and diff record types, append-only read/write, last-wins
    diff.rs      the refinement predicate, vibrato tokenization
    label.rs     the adjudicator
    score.rs     the two metrics
```

`path` in `[[bin]]` changes to `src/bin/jparser-cli/main.rs`. A `git mv` plus
one Cargo.toml line; every existing invocation, and both existing CLI test
suites, are unaffected. Files stay under 400 lines.

Rejected: a separate `crates/jparser-harness` crate. It honours master plan §7's
literal wording, but master plan §5.4 names `jparser-cli diff | label | score`
as the deliverable and a second binary would contradict the approved spec to
satisfy a rule §2.1 already reads correctly.

### 9.2 No new Rust dependencies

`serde`, `serde_json`, `clap`, and `vibrato` are all present. JSONL is
`serde_json::to_string` per line and `from_str` per line; nothing needs a
streaming parser at 250 records. `diff` and `label` are gated on the existing
`mecab` feature; `score` is not, so the offline gate does not pull in a
tokenizer.

### 9.3 One new Python tool

`tools/extract_vn_pool.py` — the two `vn_data/*.sqlite` files to a
newline-delimited pool: read `source`, drop `origin`, dedupe by exact text,
drop lines under 6 characters and over 120, strip speaker-name prefixes into a
separate column so the prefix is available for tier D and absent from the parsed
text. Output is gitignored. Python and `tools/`, matching
`extract_jmdict_subset.py`, with `sqlite3` from the standard library.

`extract_jmdict_subset.py` gains `.jsonl` corpus sniffing (§8.2). No other
change; its rules are already correct for this use.

Tiers A and B get no fetcher (§4.4).

---

## 10. Task order

Each task ends green and committed.

1. **`git mv` the bin to a directory.** Cargo.toml `path`, no behaviour change.
   `cli_parse`, `cli_hints`, `cli_generations` pass unchanged.
2. **`Entry::entry_id`.** Two lines in `lib.rs`, one assertion that `parse`
   returns the `ent_seq` the fixture declares.
3. **`corpus.rs`.** Record types, append-only writer, last-wins reader,
   supersession count. Unit tests only — no CLI surface yet.
4. **`score` and the extractor's JSONL sniffing.** Prove it on a hand-written
   10-sentence gold file whose sentences are drawn from the existing
   `parse_sentences.txt`, so the *existing* `jmdict_subset.xml` already covers
   the vocabulary and no regeneration is needed — read-only, per §2. This
   produces a working, tested `score` before any corpus work begins.
5. **`diff`.** The refinement predicate with its three cases unit-tested against
   constructed boundary sets, then run over the `vn_data/` pool.
6. **`extract_vn_pool.py`** and the pool. Then read the tier C1 register classes
   out of task 5's output.
7. **`label`.** Piped-stdin tests for bless, edit, skip, `null`, supersession.
8. **The corpus.** Tier A and B selection with the manifest, tier C1 written
   against task 6's findings, tier D derived, tier C2 sampled locally.
   Adjudicate. Regenerate `gold_jmdict_subset.xml`.
9. **Record the baseline.** Four numbers, plus tier C2's, plus the equivalence
   check of §11.4.

Tasks 1–5 are all verifiable before any labelling happens, which is deliberate:
the corpus is the expensive artefact and it should not be built against an
unproven harness.

---

## 11. Verification

### 11.1 Unit — `corpus.rs`, `score.rs`

- A gold record round-trips through JSONL.
- Two records for one `text`: the reader keeps the later, and reports one
  supersession.
- Span accuracy is 1.0 when gold equals the parse, and drops by exactly one
  span's worth when one boundary is moved.
- Entry top-1 excludes `null` gold spans from both numerator and denominator.
- Entry top-1 counts an exactly-spanned non-`null` gold span with
  `matched: false` as a miss, in the denominator.
- A gold span whose `start + len` exceeds the sentence is a hard error naming
  the record, not a silent skip.

### 11.2 Unit — the refinement predicate

Constructed boundary sets, no tokenizer:

- Coarsening (*Bj* ⊆ *Bv*) is excluded. The `東京都` sets specifically.
- Interior split (*Bv* ⊂ *Bj*) is kept.
- Crossing is kept.
- The `くるまではこう` sets from master plan §4.3 are kept, with `conflicts` =
  [3].
- Identical sets are excluded.

Plus one hand-checked sample: run `diff --all` over 200 pool lines, read the
excluded ones, confirm none is a real conflict. This is the check that the
predicate is not hiding the bug the corpus exists to find, and it is a manual
step because that judgement cannot be automated.

### 11.3 Integration — `label`

Drive the binary with piped stdin against a two-sentence diff file: bless
jparser, bless Vibrato, edit one span, skip one sentence, label one span `null`,
re-label a sentence and confirm last-wins. Assert on the resulting `gold.jsonl`.

### 11.4 The equivalence check — the gate that matters

```
score <full local index> gold.jsonl   ==   score <subset index> gold.jsonl
```

Both metrics, both ratios, both numerators and denominators, hints-on and
hints-off. Identical, or §8.2's argument is wrong and the offline gate is not
met.

Run manually, because the full index is not committed and cannot be. Recorded in
the handoff with the index generation named. This is the one check that cannot be
replaced by reasoning: everything else in Phase 0 is worth nothing if the
committed corpus scores differently from the real dictionary.

### 11.5 Gate

- `cargo test -p jparser` green, including the untouched `parse_snapshots`
  suite.
- `cargo check -p jparser --no-default-features` still green — the library must
  build without the CLI, and task 1 must not have leaked harness code into it.
- `cargo clippy --workspace -- -D warnings` clean.
- §11.4 identical.
- `score` on the committed corpus, run in a fresh clone with networking off,
  prints both metrics.

---

## 12. Errors

`score` and `diff` are batch tools and fail loudly. A malformed gold record, a
span exceeding its sentence, a non-contiguous cover, an `ent_seq` absent from
the index: each is an error naming the record's `text` and the offending field.
Skipping a bad record would move a denominator silently, which is the one thing
a baseline may not do.

`label` is interactive and fails softly. An unparseable diff record is reported
and skipped; the session continues. The output is append-only, so a crash costs
at most the sentence in hand.

An `ent_seq` in gold that is absent from the subset index is a hard error, not a
zero score — it means the extractor was not re-run after the corpus grew, and
`tests/fixtures/README.md` already records that failure mode for
`parse_sentences.txt` ("the snapshot will show unmatched spans instead of
failing loudly"). Phase 0 fails loudly instead.

`score` enforces this: every gold span's `ent_seq` is looked up against the
index before it can be scored (lookups cached, since the same id recurs across
a corpus), and the first absent one aborts the run naming both the record's
`text` and the offending id, with the likely cause (a stale subset — re-run
`tools/extract_jmdict_subset.py`). Silently scoring an absent id as a miss
would misattribute a fixture bug to the engine — the exact failure this
section forbids.

---

## 13. Deferred

- **IPADIC versus UniDic** (master plan §9.6). The harness makes this
  measurable; measuring it is not building the harness. `READING_FIELD = 7` is
  IPADIC-shaped and UniDic's layout differs, so the comparison needs a
  `hints.rs` change — which §2 puts out of scope. Run at the Phase 0/1
  boundary, with `score`.
- **sudachi.rs, out of tree** (master plan §9.7). Same boundary, same reason,
  and the licence argument for keeping it out of the tree is unchanged.
- **A tier D generator.** Forty hand-derived lines need no script (§3.1). Write
  one if tier D ever needs regenerating at scale.
- **Fetchers for tiers A and B** (§4.4).
- **Oracle@k** (master plan §5.2). Phase 2.
- **Raw-keystroke `label`** (§7.3). Only if the Enter per decision proves
  actually annoying across 2,500 of them.

---

## 14. Deviations to record in the master plan at phase close

1. **§7 Phase 0 "no code under `crates/jparser/src/`"** is amended to "no change
   to the parser library", with two exceptions named: `src/bin/jparser-cli*`
   (where §5.4's deliverable necessarily lives) and one additive read-only field
   on `Entry`. §2.1 and §5.4 carry the argument.
2. **§9.1 is answered** with five tiers, four committed (§3.1). Two sources the
   plan did not list are adopted: ja.Wikinews/ja.Wikipedia for news register,
   which Aozora does not carry, and hand-derived noise lines for the OCR and
   vertical-text genres the plan proposed deferring.
3. **§5.4's `diff` predicate is sharpened** from boundary disagreement to
   *Bj* \ *Bv* ≠ ∅ (§6.1). Boundary inequality would select nearly the whole
   pool, for the same reason §5.2 rejects boundary F1.
4. **§5.3's labels get a defined denominator**: full-sentence gold, `ent_seq:
   null` as a positive label (§4.1, §5.1).
5. **§5.4's "reads a keystroke" becomes "reads a line"** (§7.3), so the
   adjudicator is testable without a TTY.
6. **A new sampling hazard is recorded**: Aozora's pre-war orthography biases the
   disagreement sample away from the target distribution (§3.3).
7. **`label` lists candidates by `ent_seq`, not by rank** (§7.2). Without it,
   entry top-1 measures the parser against itself.
