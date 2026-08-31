# Engine v2 Phase 1a — Clean-room segmentation core

**Date:** 2026-09-01
**Master plan:** [`2026-08-27-engine-v2-master-plan.md`](2026-08-27-engine-v2-master-plan.md) §3.2, §3.5, §4.1, §4.3, §7 Phase 1, §9.2, §9.3, §9.5, §9.6, §9.7, §10
**Phase 0 handoff:** [`../2026-08-28-engine-v2-phase0-handoff.md`](../2026-08-28-engine-v2-phase0-handoff.md) §9, §11
**Branch:** `feat/engine-v2`, from `b9c35be`
**Status:** Design, approved in session, not implemented

---

## 1. Scope

Master plan §7 describes Phase 1 as one phase. This design splits it in two and
covers the first half.

**Phase 1a — in.** Independently written replacements for the segmentation core:
the lattice and its cost model, candidate generation, deinflection, the
conjugation class-and-form table regenerated from IPADIC, headword flags, kana
folding, romaji. A second engine, reachable behind a flag, measured against the
Phase 0 baseline.

**Phase 1a — out.** Naming a conjugation chain in English (§2). The switchover
that makes the new engine the default and deletes the old files (§4.1). k-best
(Phase 2). Any UI change.

**Phase 1b — in.** The auxiliary-chain formatter, its own gate, and the
switchover: the nine hard-taint files deleted, `assets/conjugations.json`
deleted, `xtask`'s converter deleted, every `ta-old` citation and every
`ported from Translation Aggregator` header gone, the provenance headers of
master plan §9.2 written.

**Phase 1's done-when is unchanged and lands whole**, at the end of 1b.

---

## 2. Where the split line falls, and why it moved

The proposal that opened this session put the auxiliary-chain recombiner and the
English labels together in 1b, on the argument that the Phase 0 harness cannot
see either: span accuracy measures boundaries, entry top-1 measures which JMdict
entry won, and neither can tell whether *"Causative-Passive Polite Negative
Past"* is named correctly. That argument is right about the labels and wrong
about the recombiner, for a reason worth stating precisely.

### 2.1 The recombiner has two outputs and the harness sees one of them

Recombining `食べ|させ|られ|ませ|ん|でし|た` produces a span shape and a string.
Whether those seven IPADIC tokens become one clickable segment or stay seven is
a decision about which characters fall in which segment, and span accuracy
measures exactly that. Worse, compound-tolerant accuracy scores it as a *merge*,
which is the class handoff §9.6 measured as over-forgiving by ~1.3 points. So
the segmentation half of the recombiner is not merely visible to the gate — it
is visible in the metric least able to judge it honestly.

There is a stronger version of the same point. Master plan §4.3 states the
arbitration rule as "constraints forbid splitting inside a token, never merging
across one", so merging across `させ|られ` is *permitted by the constraint* and
must be decided on cost. The recombination is therefore not a separate component
downstream of the DP; it is the DP's merge decision, which is 1a's cost model.

**Consequently 1a owns the chain's span shape and the structure that carries its
steps; 1b owns only the mapping from those steps to English.** Master plan
§7.3's "~300 lines, no predecessor, auxiliary-chain recombiner" is one row
covering two sub-phases, and §9 of this document splits it.

This also settles the constraint that 1a must fix `record.rs`'s token and flag
shape with 1b's needs in view. It is no longer a hazard to be managed by
foresight: the carrying structure is a 1a deliverable, populated by 1a, read by
1b. 1b's rework risk drops from "1a redesign" to "1b adds a formatter".

### 2.2 1b's gate must not be written beside 1b's code

Enumerating verb class × auxiliary chain against a hand-written expectation
table is the right gate and it is far cheaper than Phase 0's corpus, because
conjugation is finite and generative. The trap is the tautology master plan
§7.2 names in another context: a table written alongside the recombiner agrees
with it by construction, exactly as a rank-ordered candidate list would have made
entry top-1 measure the parser against itself (§5.4).

The fix costs nothing. Generate the class × chain grid mechanically, so coverage
is machine-complete and cannot quietly omit the case the code gets wrong; write
the expected English column by hand from a grammar reference **before** the
formatter exists; commit it as a fixture. It is finite and it is in English, so
it is reviewable by the author in a way 4,000 span decisions were not.

### 2.3 1a's gate is weaker than the evidence Phase 0 handed it

"Both Phase 0 metrics at or above baseline, and `くるまではこう` parses
correctly" can be met while handoff §9.5's defect class survives untouched,
because compound-tolerant accuracy **credits** those merges: they are matched
segments whose endpoints are both gold boundaries. §9.6 concluded that no
mechanical discriminator should be added to the metric, and that conclusion
stands. But the metric is not the only place to look — see §7.3. §10 adds two
gate conditions accordingly.

---

## 3. Clean-room discipline for this phase

Master plan §3.5 is binding. This section records how it was kept, so a reviewer
can establish provenance without asking.

**Precondition, verified.** `../ta-old` does not exist and no Translation
Aggregator source is present in or beside the workspace. Nothing in this phase
fetches it, and per master plan §10 no CI check may introduce it.

**What was read before the design freeze.** Interfaces only, and they are named
here so the boundary is auditable:

| Read | Why it is on the right side of §3.5 |
| --- | --- |
| `lib.rs`'s public types — `ParseResult`, `Segment`, `Entry`, `ParseError`, `parse`'s signature | The output contract the TypeScript layer consumes. Preserving an API is not transcribing an algorithm |
| `index/mod.rs`, `index/load.rs` public API — `StoredRecord`, `EntryData`, `PrefixHit`, `prefixes_of`, `entry` | Soft tier per §3.2, and the new engine must read the same index |
| `record.rs`'s eight flag names and their wire strings | Public API pinned by tests and read by the webview |
| `conjugation.rs`'s type signatures — not its data, not its loader | To size the seam. `assets/conjugations.json` was **not** opened |
| `hints.rs`'s `BoundaryHints` trait and `bad_start` / `bad_end` | The constraint interface §4.3 redefines |
| Test fixtures and their READMEs | Hand-written, no ta-old provenance |

**What is not opened during 1a.** The bodies of `segment.rs`, `matcher.rs`,
`matcher/verb.rs`, `stem.rs`, `rank.rs`, `kana.rs`, `romaji.rs`, and the
conjugation loader and asset. The side-by-side staging of §4 exists partly so
that this costs nothing: no edit is ever made inside a file being replaced.

**Constants.** Every cost term in §7 carries a written justification naming the
Phase 0 measurement it derives from. A term with no such justification does not
ship. This is the operative form of §3.5's prohibition on arriving at ta-old's
numbers "via the same numbers happened to win".

**Behaviour may be diverged from, and the old engine is a sampling source, not
an oracle.** Master plan §5.1 makes v1-versus-v2 disagreement the Phase 1
sampler pair. A disagreement is a sentence worth looking at; it is never a
failure. The existing `parse_snapshots.rs` snapshot pins v1's behaviour and is
**not** promoted into v2's gate — see §10.4.

---

## 4. Staging in the tree

### 4.1 Side-by-side, and the switchover belongs to 1b

The new engine lands as new modules under new names. The nine hard-taint files
keep compiling, untouched, and remain the default path for the whole of 1a.

Three reasons, in order of weight. Master plan §5.1 already assumes both engines
are alive from Phase 1 on — "the pair becomes jparser against its replacement" —
and no other staging provides that. Clean-room is then enforced by construction
rather than by willpower, because no edit is ever made inside a file being
replaced. And an in-place file-by-file replacement has intermediate commits that
cannot parse, which discards the incremental measurement Phase 0 was built to
provide.

**The switchover commit is 1b's, not 1a's.** Making v2 the default at the end of
1a would leave a window in which the shipped path emits `conjugation: None` —
the app without its distinguishing feature (master plan §4.4, §7.3). Deferring
it means the relicence does not slip either: "no `ta-old` reference remains
under `crates/jparser/`" stays Phase 1's done-when, discharged in one commit at
the end of 1b, with nothing shipped in between.

### 4.2 Layout

```
crates/jparser/src/
  segment.rs matcher.rs matcher/verb.rs stem.rs rank.rs
  record.rs kana.rs romaji.rs conjugation.rs      <- default path through all of 1a
  engine/
    mod.rs      parse_v2, same signature and same ParseResult as parse
    table.rs    conjugation classes and forms, generated from IPADIC; deinflection lookup
    lex.rs      candidate generation over the FST
    cost.rs     the cost terms of §7
    lattice.rs  the DP, k = 1, shaped so k follows (§4.2 of the master plan)
    chain.rs    ConjStep and the span-shape rule (1a); steps -> English is a 1b stub
    kana.rs     folding and unification, written from a chart
    romaji.rs   the romaji table, written from a chart
    flags.rs    headword records and flag derivation
```

`lib.rs` gains `pub fn parse_v2` beside `parse`, identical signature. The three
CLI subcommands that parse — `score`, `diff`, `parse` — gain `--engine v2`,
defaulting to v1. That is the entire selector; there is no trait, no registry
and no dynamic dispatch, because there are exactly two engines and one of them
is scheduled for deletion.

---

## 5. Index consequences

`StoredRecord` carries `verb_type: Option<VerbTypeId>`, an id into
`conjugations.json`, and `IndexHeader.conjugation_fingerprint` makes
`Index::open` reject an index whose ids came from a different table. v2's table
assigns different ids, so **v2 needs its own index build.**

This is the existing guard working, not friction to route around: it makes it
impossible for v1 to read a v2 index or the reverse, in either direction,
silently. During 1a there are two index builds from the same committed subset
XML, both offline, both reproducible.

Three properties keep the cost small.

**The on-disk format does not change.** v2 deinflects at parse time (§6), so its
build emits headwords only and never a generated stem. `verb_type` is therefore
always `None` in a v2 index — a field that is present and unused, not a format
difference. No version bump, and `index/*` needs no rewrite beyond the citation
scrub master plan §9.3 already assumes.

**One additive method on `index/load.rs`.** `prefixes_of` folds each character
through `crate::kana::unify` inside the FST walk. `unify` is `char -> char`, so
v2 needs the same walk parameterised by its own fold function. One method,
soft-tier code, removed again at the 1b switchover.

**Every user rebuilds their index once, and the machinery for that exists.** A
new conjugation table means a new fingerprint means every `gen-N` is invalid.
`index/mod.rs:142` names this as expected "after an application upgrade or a
changed `conjugations.json`", and the generations directory layout was built for
it. It is a consequence to record, not a risk to mitigate.

---

## 6. Deinflection moves from build time to parse time

Today deinflection happens twice: `stem.rs` generates stem keys into the FST at
build time, and `matcher/verb.rs` recurses conjugation suffixes at parse time.
v2 does it once.

At a span's end, a reverse trie over the table's conjugation suffixes yields
`(stem_len, class, form)` candidates; the stem plus its class's remove-suffix is
then one exact FST lookup. One reverse-trie walk per position, not a trial of
every rule.

**Why this rather than keeping the two-stage shape.** One mechanism instead of
two that must be kept in step. One place for 1b's chain to originate, which is
what makes the §8 seam a single structure rather than a union of two. A smaller
index. And the `する` / `来る` wart — an irregular verb whose whole surface is
its own remove-suffix generates an empty-string stem, which `prefixes_of`
documents as a special case and `parse_snapshots.rs` carries two targeted
assertions about — does not arise at all, because there is no generated stem to
be empty.

**What it costs.** A larger deviation from master plan §7's size table than the
phrase "still walks the FST over spans" anticipates, and the deviation is
recorded in §9's budget rather than hidden inside it.

**A standing constraint decided this, not a preference.** `parse` takes
`hints: Option<&dyn BoundaryHints>` and the whole `hints` module sits behind
`#[cfg(feature = "mecab")]`, with `cargo check -p jparser --no-default-features`
as a gate the repository already enforces. The engine must therefore deinflect
with no tokenizer present at all. IPADIC can never be the deinflection path; it
can only ever be an accelerator. The reading of master plan §7.2 in which
`stem.rs` shrinks to ~250 lines "because IPADIC gives dictionary forms directly"
does not survive that constraint, and §11 records the correction.

### 6.1 Hints supply boundaries only

A decision taken explicitly, because the alternative is attractive and its cost
is easy to miss.

IPADIC lexicalises the colloquial contractions master plan §3.3 measured —
`食べちゃった` as `食べ | ちゃっ` with `ちゃう` as a 非自立 verb, `やっとかなきゃ`
with `なきゃ` carrying its own 仮定縮約２ form — so letting a token's 基本形
field become a JMdict lookup candidate would buy the informal register almost
free.

It is not taken. Today hints change **where** cuts fall and nothing else, which
is precisely what the Phase 0 baseline pair measures. Base-form candidates would
make hints change **which entries can match**, so entry top-1 would acquire a
hint dependency it does not have today and the hints-on and hints-off numbers
would stop being two measurements of one engine. It would also make 1b's input
non-uniform — sometimes table steps, sometimes an IPADIC token run — which is
the exact shape §2.1 exists to prevent.

The conjugation table is the single deinflection path, present with or without a
tokenizer. Revisit in Phase 2 only if 1a's measured error analysis shows
contracted forms surviving as a residual class.

### 6.2 The table's source

IPADIC's conjugation data is not in the tree. `crates/vibrato-source` pins the
**compiled** `ipadic-mecab-2_7_0.tar.xz`, and Vibrato exposes `tokenize`, not
lexicon iteration, so the shipped asset cannot yield the full table — only the
forms a corpus happens to exercise.

Deriving it mechanically means the raw `mecab-ipadic-2.7.0-20070801` CSVs,
pivoted on (活用型, 活用形) with the surface suffix taken relative to the base
form. That is a new out-of-tree fetch, handled the way the UniDic and sudachi
measurements of master plan §9.6 and §9.7 were: fetched outside the repository,
used to generate, not vendored.

The generator replaces `xtask`'s ta-old converter rather than merely deleting
it. Its output — the table — is committed, with the IPADIC notice retained per
master plan §3.3, which permits reproduction and distribution on exactly that
condition. The generator itself is committed too, so the table is regenerable
from its inputs rather than being a hand-edited artefact; that is the property
handoff §11.1 established for `gold.jsonl` and it is worth having here for the
same reason.

---

## 7. The cost model

### 7.1 Hard constraints alone do not fix `くるまではこう`

Master plan §4.3 names two causes for the motivating bug and proposes a cost
model "in which tokenizer boundaries are evidence rather than a ±10 nudge". Read
together with §4.3's own arbitration rule, the constraint half is insufficient
for the example, and this needs stating because it is easy to believe otherwise.

The rule is that constraints forbid splitting inside a token and never forbid
merging across one — the asymmetry that lets `東京都` survive a tokenizer that
cuts it as `東京 | 都`. But Vibrato tokenizes the sentence as `は` at 4..5 and
`こう` at 5..7, so `はこう` at 4..7 **merges across** and is legal under any
strength of interior constraint. It has to lose on cost.

So removing the fewer-words-wins bias is not an optional second half of the fix.
With a per-match constant retained, success criterion 3 fails with every
constraint in place.

### 7.2 The shape

Cost accrues per **character covered**, not per match. `はこう` and `は` + `こう`
therefore start level on the coverage term, and the decision is made by evidence
terms rather than by an arithmetic accident of how many matches a reading uses.

```
cost(span) = c_cover · len
           + c_cross · (tokenizer boundaries strictly inside span)
           + c_pronounce_cross     if the match is PRONOUNCE-only and crosses
           − evidence(common, exact, particle-in-context)

unmatched character  ≫  matched character
a span that starts or ends inside a token  →  forbidden
```

### 7.3 The PRONOUNCE term, and why it does not contradict §9.6

Handoff §9.6 concluded that no mechanical discriminator separates a spurious
merge from a real compound, and that none should be added. That conclusion is
about the **metric**, where the object being judged is a merged span the corpus
carries no label for — which is what makes it merged. It stands.

The cost model is a different position with more to look at, and there the
discriminator is already computed and already on the wire. Every one of handoff
§9.5's ~28 spurious merges is a match against a kanji headword's **reading**:
`はいた` → 履く via はく, `ももう` → 揉む via もむ, `がい` → 甲斐, `してい` →
指定, `どうし` → 同士, `ことし` → 今年, `になって` → 荷なう, and the katakana runs
`クで` / `タは` / `ヌの`. Every legitimate merge in handoff §9.1 is kanji-bearing:
`音がする`, `ことになっている`, `首を振る`, `これまでに`, `天と地`, `東京都`,
`2月`.

That distinction is `WordFlags::PRONOUNCE` against `WordFlags::PRIMARY`, derived
in `record.rs` today and serialised to the webview by name. Kana-only words are
not caught by it: `PRIMARY` is defined as "kanji spelling of a kanji word, **or
the hiragana of a kana-only word**", so `は`, `こう`, `こと` and `もう` are
`PRIMARY` and only the reading-form-of-a-kanji-word case is priced.

The claim this design makes is therefore narrow and testable: **a PRONOUNCE-only
match that crosses a tokenizer boundary is the §9.5 defect class.** If it is
wrong, §10's third gate condition fails visibly and the term is removed rather
than tuned into invisibility.

### 7.4 Every term's justification

| Term | Justified by |
| --- | --- |
| per-character coverage, matched | Master plan §4.3 cause 2. A per-match constant is what makes `はこう` beat `は` + `こう` |
| per-character coverage, unmatched ≫ matched | Handoff §9.1's 36-boundary fallback bucket — runs of punctuation lumped into one unmatched segment, the one unambiguous defect in the decomposition |
| crossing a tokenizer boundary, priced rather than forbidden | Master plan §4.3's asymmetry. `東京都` must be able to pay it and still win |
| PRONOUNCE-only match that crosses | Handoff §9.5, §7.3 above |
| splitting inside a token, forbidden | Master plan §4.3, at the measured price of §7.5 |
| common and exact evidence | Handoff §9.5's absurd matches are all inexact against their headword; §9.1's legitimate merges are exact modulo conjugation |

Tuning is against the gold corpus and the §9.5 list. There is no target set of
constants and no comparison against the old model's numbers.

### 7.5 What the hard interior constraint costs, measured

From the committed diagnostic (handoff §9, `examples/boundary_diag.rs`): of
2,013 interior gold boundaries, jparser holds 1,734 and Vibrato 1,954, with 266
held by Vibrato alone. So 1,688 are held by both and **46 are held by jparser
alone** — gold boundaries the current engine finds and Vibrato does not.

Those 46 are what a hard interior constraint forbids. At an absolute ceiling
that is worth about two points of strict span accuracy, and in practice less,
since a gold span needs both of its edges. That is the price of the asymmetry,
and it is the number master plan §4.3's softening clause — "the constraint
softens to a large-but-finite penalty" — should be judged against, rather than a
guess. If measurement shows the constraint costing more than this, the softening
is a cost-model parameter, as §4.3 says, not a redesign.

---

## 8. The 1a/1b seam

One additive field on `Entry`. `conjugation: Option<String>` keeps its type and
meaning exactly.

```rust
/// One step of a deinflection chain: which class was applied, in which form,
/// over which characters of the span. Filled by 1a; named by 1b.
///
/// `ClassId` and `FormId` are `table.rs`'s own ids, assigned by the IPADIC
/// generator of §6.2. They are not `conjugation.rs`'s `VerbTypeId`/`TenseId`
/// and no mapping between the two exists or is wanted.
pub struct ConjStep {
    pub class: ClassId,
    pub form: FormId,
    pub range: std::ops::Range<usize>,
}

pub struct Entry {
    // ..
    /// 1a leaves this `None`. 1b writes the formatter that fills it.
    pub conjugation: Option<String>,
    pub steps: Vec<ConjStep>,
    // ..
}
```

**1b changes no TypeScript.** `src/render/tooltip-text.ts:17` prefixes
`entry.conjugation` with `CONJ_MARKER` and `tooltip-colour.ts:64` colours the
line; neither inspects the string. The label is opaque to the whole frontend, so
1b's surface is `fmt(&[ConjStep]) -> Option<String>` and nothing else on this
path.

**The span-shape rule is 1a's.** Whether `食べさせられませんでした` is one segment
or seven is decided by §7's cost model and gated by §10's span accuracy. 1b
cannot move it without moving a number 1a has already fixed.

**`WordFlags` keeps its eight wire names.** They are public API, pinned by tests
in `record.rs` and read by the webview to pick a chip's content class. `TOP` and
`IS_NAME` are set by nothing today — `TOP` was reserved for a scorer port that
this phase deletes, `IS_NAME` for JMnedict. v2 keeps both names on the wire and
sets neither, so no TypeScript moves and nothing is carried forward for a
component that will not exist.

---

## 9. Module budget

Master plan §7's size table is per replaced file and assumes Phase 1 is one
phase. Restated per sub-phase, with the two structural deviations of §6 and §2.1
visible rather than absorbed:

| New module | Replaces | Master plan est. | 1a | 1b |
| --- | --- | ---: | ---: | ---: |
| `lattice.rs` + `cost.rs` | `segment.rs`, `rank.rs` | ~650 | ~650 | — |
| `lex.rs` | `matcher.rs`, `matcher/verb.rs` | ~600 | ~350 | — |
| `table.rs` | `stem.rs`, `conjugation.rs`'s table | ~850 | ~550 | — |
| `chain.rs` | *(new)* — master plan §7.3's recombiner | ~300 | ~120 | ~180 |
| `conjugation.rs`'s label mapping | *(new)* — master plan §7.3 | *(in the 600)* | — | ~300 |
| `flags.rs` | `record.rs` | ~350 | ~350 | — |
| `kana.rs` + `romaji.rs` | same | ~560 | ~560 | — |
| **Total** | | **~3,300** | **~2,580** | **~480** |

Plus the generated table data, which is generator output rather than
hand-written lines, and 1b's expectation fixture.

Two deviations are deliberate. `lex.rs` is smaller and `table.rs` larger than
the master plan's rows, because §6 moves deinflection out of the matcher and
into the table. And master plan §7.3's single ~300-line recombiner row is split
across both sub-phases per §2.1.

---

## 10. The gate

### 10.1 Metrics

`jparser-cli score --engine v2` over the committed `gold.jsonl` and a v2 index
built from the committed `gold_jmdict_subset.xml` (§5 — the XML is committed,
the index is built from it, and v2 needs its own), in both hint states. Baseline
from master plan §7 Phase 0 and handoff §11.2.

| | strict span | compound-tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | ≥ 0.6810 | ≥ 0.8465 | ≥ 0.9159 | ≤ 167 |
| hints on | ≥ 0.6963 | ≥ 0.8678 | ≥ 0.9155 | ≤ 173 |

**Both span metrics are quoted, always, and never the strict one alone.**
Master plan §7 Phase 0 constraint 1 gives the reason: against a corpus
adjudicated at short-unit granularity, strict accuracy rewards splitting the
JMdict compounds §5.2 exists to preserve, so optimising it directly optimises
against the product. Strict moving without tolerant moving means granularity
changed, not correctness.

**Merge count is a ceiling, not a floor.** Handoff §9.6: a change that lifts
tolerant accuracy by inflating the merge count has probably found more spurious
matches, not more compounds. §7.3's whole claim is that the PRONOUNCE term
should make this number *fall*.

### 10.2 `くるまではこう`

Parses as `くる / まで / は / こう`. Master plan success criterion 3, committed
as a test.

### 10.3 The §9.5 defect class is gone

`examples/boundary_diag.rs` is re-run against v2 and the ~28 particle-absorbing
merges of handoff §9.5 are absent. The surfaces are committed as a fixture so
the check runs from a clean clone: they are this project's own measurement,
contain no ta-old expression and no captured text — `はいた`, `ももう`, `がい`,
`してい` and their kind are ordinary Japanese strings — and the promotion is the
same one `gold.jsonl` received in handoff §11.1.

This is the condition §2.3 adds, and it is the one that cannot be satisfied by
the metrics alone.

### 10.4 What is not a gate

**v1's snapshot.** `parse_snapshots.rs` pins the current engine's output over
the 30 hand-written sentences of `parse_sentences.txt`. v2 gets a **new**
snapshot, reviewed on its own terms. Promoting v1's would import the port's
behaviour, including its bugs, as the specification — and master plan §5.1 is
explicit that jparser is a disagreement source, not an oracle. A v1/v2
difference is a sentence worth reading; it is never a failure.

**Absolute values.** Master plan §7 Phase 0 constraint 3: every sentence in the
corpus was sampled because two segmenters disagreed on it. The numbers exist to
be comparable across phases.

**The corpus's unreviewed axis.** Constraint 2, and `gold_sources.md`: entry
decisions were author-reviewed and mechanically audited; span decisions were
not, and 54 of the 150 sentences carry a cut neither segmenter proposed. If a 1a
result looks wrong, `corpus_staging/REVIEW.md` section 3 is the first place to
read — handoff §11.4 rates it the best risk-per-minute review available.

### 10.5 Standing gates

Unchanged from Phase 0's close and re-run on every commit:
`cargo test -p jparser` and `--features mecab`;
`cargo clippy -p jparser --all-targets -- -D warnings` with and without the
feature; `cargo check -p jparser --no-default-features`. Plus, for v2, the
equivalence check of the **Phase 0 design's** §11.4: `score --json --engine v2`
over a v2 full index and a v2 subset index must return identical metrics in both
hint states, every field but `index_generation`.

### 10.6 Test strategy

Written test-first, per the repository's workflow rules.

| Unit | Its check |
| --- | --- |
| `table.rs` | Generator output is deterministic and regenerable; spot forms verified against a grammar reference, not against `conjugations.json` |
| `kana.rs`, `romaji.rs` | Table tests from a published chart. Neither is reached by either metric — romaji is display-only, used by one CLI subcommand and a furigana mode — so unit tests are their only cover |
| `lattice.rs`, `cost.rs` | Unit tests over synthetic lattices for each cost term in isolation; `くるまではこう` and the §9.5 list as integration tests |
| `chain.rs` | The span-shape rule over multi-auxiliary chains |
| `lex.rs` | Reverse-trie candidate generation, including the irregulars that no longer produce an empty stem |

---

## 11. Findings that amend the master plan

Five, all measured against the tree at `b9c35be` or derived from committed Phase
0 artefacts. §12 folds them into the master plan.

### 11.1 §3.2's "Clean" tier was verified against the wrong string

`grep -rc "ta-old"` reports zero for `crates/jmdict-source` and
`crates/vibrato-source`, which is what §3.2 records. But **10 files across those
two crates carry the header `Japanese text parser ported from Translation
Aggregator`** — a string containing no `ta-old`. Under `crates/jparser/` the
same header sits on **24 files**, beside 136 `ta-old` references; `xtask` has
one, and `src-tauri` eleven.

This is not a taint reclassification: a boilerplate header claiming a port is
not protected expression, and the clean crates remain clean in substance. It
matters because Phase 1's done-when is written as "no `ta-old` reference remains
under `crates/jparser/`", and that clause passes a grep while every file in the
crate still declares itself a port of Translation Aggregator — which is the
first line a licence auditor reads. **Phase 1's done-when must name both
strings**, and the sweep must cover `xtask` and the two crates §3.2 calls clean.

### 11.2 Hard constraints alone do not fix `くるまではこう`

§7.1. The asymmetry that saves `東京都` is exactly what leaves `はこう` legal.

### 11.3 The asymmetry's price is 46 gold boundaries

§7.5, derived from the committed diagnostic rather than estimated.

### 11.4 §9.5's defect class has a discriminator, in the cost model

§7.3. Handoff §9.6's refusal to add a heuristic to the **metric** stands
unamended; the cost model is a different position with the flag already computed.

### 11.5 §7.2's reading of `stem.rs` does not survive an existing gate

§6. `hints` is optional and feature-gated, with `--no-default-features` enforced,
so the engine must deinflect with no tokenizer. IPADIC cannot be the
deinflection path; `stem.rs`'s work does not shrink because IPADIC supplies
dictionary forms — it moves into the table.

---

## 12. Master plan edits this design owes

To be made as part of implementing 1a, not deferred past it:

- **§7 Phase 1** split into **Phase 1a** and **Phase 1b**, each with its own
  In / Out / Done-when, and the size table restated per §9 above.
- **§7 Phase 1's done-when** amended to name both `ta-old` and
  `ported from Translation Aggregator`, over `crates/jparser/`, `xtask/` and the
  two crates of §3.2's clean tier.
- **§3.2** corrected: the clean tier was verified against the wrong string
  (§11.1).
- **§4.3** amended with §11.2 and §11.3.
- **§7.2** amended with §11.5.
- **§10 decision log**, new rows: the 1a/1b line and its three sharpenings; the
  switchover belonging to 1b; hints supplying boundaries only; parse-time
  deinflection; the PRONOUNCE cost term and its relationship to handoff §9.6.

---

## 13. Decision log

| Decision | Rationale |
| --- | --- |
| Split Phase 1 into 1a and 1b | §7.3's label mapping is invisible to both Phase 0 metrics, and ~900 ungated lines inside a 3,300-line phase is the silent-degradation shape handoff §9.6 already caught once. 1b needs a gate of its own, and conjugation is finite and generative, so it is far cheaper than a corpus |
| 1a owns the chain's span shape; 1b owns only the English | The recombiner has two outputs. Its span shape is the DP's merge decision and is measured by span accuracy — and by the tolerant metric, which over-forgives exactly that shape. Only the string is ungated (§2.1) |
| 1b's expectation table is written before the formatter, from a grammar reference | A table written beside the code agrees with it by construction, which is master plan §7.2's tautology in a new place. Generate the class × chain grid mechanically for coverage; hand-write only the English column (§2.2) |
| 1a's gate adds the §9.5 merge conditions | "Both metrics at or above baseline" can pass while the defect class survives, because compound-tolerant accuracy credits those merges. Merge count becomes a ceiling and the named surfaces must be gone (§2.3, §10.3) |
| Side-by-side staging | Master plan §5.1 already needs both engines alive from Phase 1 on, clean-room becomes enforced by construction, and incremental measurement survives (§4.1) |
| The switchover commit belongs to 1b | Switching at the end of 1a leaves a window where the engine emits no conjugation label — the app without its distinguishing feature. Deferring keeps Phase 1's done-when whole and lands the relicence in one commit (§4.1) |
| v2 builds its own index; the format does not change | `verb_type` ids are table-relative and the header fingerprint already rejects a mismatch, so the guard prevents a cross-read for free. v2 emits no generated stems, so the field is present-and-unused rather than a format difference (§5) |
| Deinflection at parse time, via a reverse suffix trie | One mechanism instead of a build-time generator plus a parse-time recursion; one origin for 1b's chain; a smaller index; and the `する`/`来る` empty-stem special case disappears. Costs a deviation from §7's size table, recorded in §9 (§6) |
| Hints supply boundaries only, never match candidates | `parse` takes `hints: Option` behind a feature gate with `--no-default-features` enforced, so the table is the deinflection path regardless. Base-form candidates would make entry top-1 hint-dependent and 1b's input non-uniform, for a contraction win that can be revisited on evidence in Phase 2 (§6.1) |
| Cost accrues per character, not per match | A per-match constant is what makes `はこう` beat `は` + `こう`, and the interior constraint does not forbid that merge. Removing the bias structurally is the only form of the fix that satisfies success criterion 3 (§7.1, §7.2) |
| Price a PRONOUNCE-only match that crosses a boundary | Every one of handoff §9.5's ~28 spurious merges is a reading-form match of a kanji headword; every legitimate merge in §9.1 is kanji-bearing. The flag is already computed and already on the wire, and kana-only words are `PRIMARY` so they are not caught. Handoff §9.6's refusal of a metric heuristic is unaffected (§7.3) |
| Commit the §9.5 surfaces as a fixture | The gate must run from a clean clone. They are this project's own measurement, carry no ta-old expression and no captured text, and the promotion is the one `gold.jsonl` already received (§10.3) |
| v1's snapshot is not promoted into v2's gate | It pins the port's behaviour, bugs included, as a specification. Master plan §5.1 makes v1 a disagreement source, not an oracle (§10.4) |
| The IPADIC CSVs are fetched out of tree; the generated table is committed | The shipped asset is Vibrato's compiled dictionary and yields no lexicon. IPADIC's licence permits reproduction with notice retention, and committing generator plus output keeps the table regenerable rather than hand-edited (§6.2) |
