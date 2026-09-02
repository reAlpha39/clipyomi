# Engine v2 Phase 1b-ii — Handoff

**Date:** 2026-09-02
**Design:** [`specs/2026-09-02-engine-v2-phase1b-ii-design.md`](specs/2026-09-02-engine-v2-phase1b-ii-design.md)
**Plan:** [`plans/2026-09-02-engine-v2-phase1b-ii.md`](plans/2026-09-02-engine-v2-phase1b-ii.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §1, §3.2, §7 Phase 1, §7 Phase 1b, §9.2
**Status:** Closed. Design §10's six done-when conditions are met (§3 below).
Phase 1's own done-when is settled: its relicence clause met, its metric
clause superseded and recorded as such.

Phase 1b-ii made v2 the only engine, deleted the nine hard-taint files master
plan §3.2 names, and relicensed the tree from `GPL-2.0-only` to
`MIT OR Apache-2.0`. Master plan §1's goal is discharged.

---

## 1. An open question for the maintainer — the frontend's provenance

**This is not a defect, and not a decision this phase took.** It is a question
the phase surfaced and deliberately did not answer, because it belongs to the
copyright holder.

Seven comments in the frontend cite the upstream GPL-2 C++ by source file and
line range. They are, exactly as written:

| Site | Citation |
| --- | --- |
| `src/render/tooltip-colour.test.ts:11` | `MyToolTip.cpp:154-160` |
| `src/render/tooltip-colour.test.ts:41` | `MyToolTip.cpp:214-217` |
| `src/render/tooltip-colour.test.ts:50` | `MyToolTip.cpp:224` |
| `src/render/popover.ts:45` | `MyToolTip.cpp:518-523` |
| `src/render/popover.test.ts:28` | `MyToolTip.cpp:518-523` |
| `src/render/tooltip-text.test.ts:66` | *"`DictionaryUtil.cpp:46` sets `temp[0] = 1` for exactly this purpose."* |
| `src/styles/tooltip.css:50` | `MyDrawText`'s `xOffset = 10` |

`README.md:5` names Translation Aggregator outright as the project's lineage —
deliberate attribution, and the eighth site if one is counting them all.

**One of these rows is gone since this table was first written.**
`c827ef2` reimplemented `src/render/tooltip-colour.ts`'s `colourLine`
clean-room: the implementer worked only from
`src/render/tooltip-colour.test.ts` as the specification, was forbidden from
reading the previous implementation or any upstream source, and confirmed
that constraint was honoured. The result is structurally independent — 157
lines against 97, six private helpers against three, iterating code points
where the old version indexed the string by UTF-16 code unit — and carries no
derivation claim at all. That is why the table above no longer has rows for
the former `src/render/tooltip-colour.ts:47` and `:58`; the citation
inventory dropped from nine sites to seven. The rewrite also fixed a latent
bug the index-based version had: a supplementary-plane ideograph (a
UTF-16 surrogate pair) would have been mishandled, and a new test now pins
U+20000 through the code-point-iterating version.

**Why these survived, and why that is not evidence they are fine.** They
passed this phase's done-when greps **by construction, not by judgment**: the
binding checks counted two literal strings (`ta-old` and `ported from
Translation Aggregator`), and a C++ filename is neither. The phase's
clean-room rebuild covered the **parsing engine only**. The frontend tooltip
renderer was never in scope, never behind the fence, and never reassessed, so
nothing this phase produced asserts it is clean.

**What actually relabelled it.** No file under `src/` carries a licence header
at all — the clean-room SPDX headers exist only on `crates/jparser/src/engine/*`,
the Phase 0 harness files and two engine test files. So the act that newly
declares the frontend `MIT OR Apache-2.0` is `package.json`'s `license` field
together with `README.md` and the `LICENSE-MIT`/`LICENSE-APACHE` pair, not a
per-file assertion.

**What the citations actually record — and what changed.** This section
originally singled out two citations as going beyond uncopyrightable
behavioural rules: `src/render/tooltip-colour.ts:58`, whose author
characterised the work as *"a port of the rule rather than of its output"*,
and `src/render/tooltip-text.test.ts:66`, which quotes an upstream *source
statement* rather than a line range. **The first of those two is now gone** —
dissolved by the clean-room rewrite above, not merely reworded. Everything
that remains is one of three kinds: a test comment, a CSS comment, or the one
implementation comment left, `src/render/popover.ts:45`, which describes a
window-placement rule (flip sideways, right first). Comments of that kind are
recording *which rules were adopted* — which characters colour how, a 2px
gap, a right-first flip, `xOffset = 10` — and a rule of that kind is largely
uncopyrightable as such. That argument, which this section already made for
most of the tree, now applies to everything left in it.

The one exception worth naming on its own is **`src/render/tooltip-text.test.ts:66`**,
the surviving strong case: it quotes an upstream *source statement*, not a
behavioural rule, and it is a comment in a *test* file about a `\x01`
sentinel value, not implementation code. That distinction may matter to the
analysis, but it does not settle it — the citation still names a specific
upstream line.

**This narrows the open question; it does not close it.** The maintainer
still owns the decision about the remaining citations and about
`README.md:5`, and the merge-versus-release guidance below still stands.

**The ruling this phase took: leave every one of them exactly as written.**
The citations *are* the provenance evidence. Rewording them would strip the
maintainer's ability to assess the question while changing nothing about the
code's actual derivation — the worst of both worlds, and the one action that
is certainly wrong. A tree carrying honest citations a licence reviewer will
ask about is strictly better than the same derivation with its citations
scrubbed. Task 8's stale-prose sweep was explicitly told to leave them alone,
and did.

**What this means for merging versus releasing.** This branch does not
*create* the frontend's derivation. If `src/render/tooltip-colour.ts` is a
derivative work of the upstream C++, it already was one before this phase,
under `GPL-2.0-only`, where that derivation was compatible by construction —
a GPL-2 work deriving from GPL-2 raises no licence question at all. What this
branch changes is the licence the *same code* now ships under. Under
`MIT OR Apache-2.0`, relicensing a derivative work is not unilaterally the
maintainer's to make — but that consequence attaches at **distribution**, not
at a merge into `main` in the maintainer's own repository. Nobody outside this
repository receives anything different the moment this branch merges. So the
recommendation is narrower than "resolve this before merging": merging is
fine, and it is the **first release** cut under the new licence that should
wait on the maintainer's answer. This is guidance on where the risk actually
sits, not legal advice — the maintainer should still get a real answer before
that release ships.

**A third option, beside yes and no — and this one has now been taken.** The
question above was framed as binary — is this code clean, or isn't it — but
there was a cheap engineering answer available too, and `c827ef2` took it.
`colourLine`, the function that had carried *"Ported from `MyDrawText`"*, the
strongest of the original nine citations, was reimplemented clean-room in
that one commit, working only from `src/render/tooltip-colour.test.ts` as the
specification. That converted the highest-risk item in the tree from an open
licence question into a closed engineering task: the same manoeuvre this
phase performed, at far larger scale, on the parsing engine itself. The same
move remains available for any of the seven remaining citations the
maintainer judges needs it.

**One thing that is now available and was not before.** "The original
tooltip" is the de-naming circumlocution in **ten** places
(`src-tauri/src/popover.rs:17`, `src/main.ts:782,974`,
`src/render/popover.ts:1,82`, `src/render/popover.test.ts:16,70`,
`src/render/tooltip-text.ts:6`, `src/styles/tooltip.css:21`,
`src/surface-alpha.ts:6`). Under GPL that phrasing had a purpose: the phase's
done-when counted the literal upstream name. Under `MIT OR Apache-2.0` there
is **no licence reason to avoid naming an inspiration outright**, so if the
maintainer's provenance review would rather name the project, that is one
mechanical find-and-replace.

---

## 2. What exists now

**One engine.** `crates/jparser/src/engine/` is the parser: 13 files
(`assemble`, `build`, `chain`, `cost`, `flags`, `kana`, `label`, `lattice`,
`lex`, `mod`, `romaji`, `table`, `table_data`). 25 tracked files under
`crates/jparser/src/` totalling 9,864 lines; 10 integration test files under
`crates/jparser/tests/`; 12 files under `src-tauri/src/`. There is no `--engine`
flag, no `parse_v2`, no `xtask` crate, and no `assets/conjugations.json`.

**The public API**, from `crates/jparser/src/lib.rs`:

```rust
pub fn parse(
    index: &Index,
    table: &engine::table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError>

pub use crate::engine::flags::WordFlags;   // WordFlags(pub u16): contains / insert / names
pub use crate::engine::Boundaries;         // is_boundary(pos) -> bool
pub use crate::index::SenseData as Sense;
// plus ParseOptions, ParseResult, Segment, Entry, ParseError defined in lib.rs
```

`engine::table::Table::new()` is infallible, which is why `StartupError::Conjugation`
and the `ConjugationError` it wrapped are gone with `ConjugationTable`.

**Gate results at the final commit** — all seven green, all four counts
unchanged across the last three tasks of the phase:

| gate | result |
| --- | --- |
| `cargo test -p jparser` | 260 passed / 0 failed / 0 ignored |
| `cargo test -p jparser --features mecab` | 286 passed / 0 failed / 1 ignored |
| `cargo clippy -p jparser --all-targets -- -D warnings` | clean |
| `cargo clippy -p jparser --all-targets --features mecab -- -D warnings` | clean |
| `cargo check -p jparser --no-default-features` | clean |
| `cargo test -p clipyomi` | 149 passed / 0 failed / 0 ignored |
| `cargo test -p jmdict-source` | 37 passed / 0 failed / 1 ignored |

The one ignored `jparser` test is `the_particle_absorbing_merges_are_gone`
(Phase 1a handoff §5's §10.3 reading; still ignored, still for a fixture-gap
reason). `cargo fmt` ran once at the end of the phase over both `jparser` and
`clipyomi`; both crates are rustfmt-clean.

**The test-count drop at the switchover reconciles, and is worth recording
because 173 vanishing tests is exactly where a real loss would hide.**
`jparser` went 433/0 → 260/0 default and 468/1 → 286/1 with `mecab`: −173 and
−182, an asymmetry of exactly 9, which is `hints.rs`'s nine deleted
`BoundaryFlags`-typed tests (they exist only under `--features mecab`). The
dominant term is ~155 lib tests leaving with the nine orphaned `mod`
declarations — each of those files carried its own `#[cfg(test)] mod tests`,
which stops compiling the moment the module is gone. Every one of them tested
v1 code the next commit deleted.

---

## 3. Design §10's done-when, one condition at a time

**1. Both banned strings grep to zero.** Scoped to the source tree, per the
plan's own Ruling 8 — `docs/` holds 47 files mentioning `ta-old` and this
phase's own design and plan among the historical record:

```
git grep -n "ta-old" -- ':!docs'                                  → nothing
git grep -n "ported from Translation Aggregator" -- ':!docs'      → nothing
```

**2. The GPL grep returns nothing outside `LICENSE-APACHE`'s own text.**

```
git grep -n "GNU General Public License" -- ':!docs' ':!LICENSE-APACHE'  → nothing
git grep -n "GPL" -- ':!docs' ':!LICENSE-APACHE'                          → nothing
```

`LICENSE` is gone; `LICENSE-MIT` and the canonical `LICENSE-APACHE` are in
place (verified by hash — 11,358 bytes, 202 newlines, sections 1–9 at their
canonical offsets, appendix placeholder unmodified — rather than accepted as a
paraphrase). 59 files were re-headered. `Cargo.toml:7` and `package.json:3`
both read `MIT OR Apache-2.0`.

Two provenance items are worth naming here because they were done by hand and
are the kind of thing that silently regresses. `tools/gen_conjugation_table.py`
*emits* the header of `engine/table_data.rs`, so its emitted string was
updated too and proved byte-identical to the committed file
(`sha256(HEADER) == sha256(table_data.rs[:927])`); a regeneration therefore
cannot reintroduce the GPL header. And the CI TLS-backend gate's grep pattern
is byte-identical and unweakened — only its GPL *rationale* was reworded. Four
files carry no SPDX line because they never carried a GPL header
(`crates/jparser/examples/boundary_diag.rs`, `src-tauri/src/mouse_tracker.rs`,
`src-tauri/src/popover.rs`, `tools/gen_conjugation_table.py`); that is the
only header inconsistency left **among files that carry an SPDX line at
all** — §1 documents a separate and larger gap, where no file under `src/`
carries a licence header at all — and this phase did not create it.

**3. The six gates green at every commit.** They were, and a seventh joined
them — see §6 below.

**4. The startup path.** Verified by the simpler of the two checks the plan
offered, rather than by hex-editing an index header:
`state::tests::a_stale_index_is_the_download_screens_to_fix` passes (confirmed
by name with a filtered run), and `index/mod.rs`'s own
`a_version_mismatch_rebuilds` and `a_fingerprint_mismatch_rebuilds` pass by
name. Together: a stale generation — wrong `engine::build::fingerprint` or an
older `INDEX_FORMAT_VERSION` — routes to the download screen rather than a
fatal, and `ensure_dictionary` rebuilds on exactly those two errors.

**5. The eight `score` cells, reproduced exactly.** Against the gold corpus,
no `--engine` argument, `--features mecab`, hints from the 45.6 MB IPADIC
system dictionary. 150 sentences, 2,163 gold spans, 0 superseded in both runs.

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573/2163) | 0.7975 (1725/2163) | 0.9075 (1108/1221) | 75 |
| hints on | 0.8031 (1737/2163) | 0.8521 (1843/2163) | 0.9063 (1170/1291) | 53 |

**Byte-identical to Phase 1b-i's table on every field**, which is in turn
byte-identical to Phase 1a's. Measured three times independently in this
phase: after the table fixes, after the switchover, and by a reviewer who
rebuilt the gold index from scratch in a throwaway worktree and re-ran both
hint states. The secondary numbers also reproduce: 152 gold spans coarsened
inside 75 merges hints-off, 106 inside 53 hints-on.

**6. The two CLI observables**, through the switched-over binary:

- `parse 読みました` → 読む, label `Polite Past`.
- `parse 来た` → **one** span, headword 来る, reading きた, label `Past`. Two
  further same-span candidates (来た, 来たる) follow, correctly ranked below.

The second is §4's kanji-来る fix visible end to end, and it is the observable
that would have been a shipped regression had the fix not been bundled.

---

## 4. The two table fixes

Both were landed in the prep stage while v1 was still the default, measured
one at a time, and both proved metric-neutral. The design's standing rule was
that a cell moving *down* is a finding rather than a number to record; none
did.

### 4.1 The たい row reorder (design §9.2)

The `("たい", "特殊・タイ", "連用形")` row in `ATTACHES_TO` moved below the two
`("た", …)` rows. That changes which of two equal-depth chains survives
`lex::dedup_shallowest` and nothing else — the survivor has the same span, the
same `len` and the same `entry_id`, which is the whole of what
`score.rs::accumulate` reads.

**K = 3 flipped labels**, exactly as predicted: 読みました, しました and
書かせられた, all *Want-to* → *Past*, with headword, reading and gloss
unchanged on every one. The coverage walk's 21,760-chain literal was untouched
(same rows, different order). All eight `score` cells unchanged.

This closes Phase 1b-i handoff §4's finding 5, which the final review of that
phase called a blocker for the moment v2 became the default: *"Polite Want-to"*
on ました is wrong on the most common polite form in the language.

**The principled fix is named and not taken.** Making `たい:ガル接続`
non-terminal — no `ATTACHES_TO` row requires that form — would delete
candidates and can move the DP, so it is a measured Phase 1a follow-up. The
same goes for the godan 仮定形/命令ｅ tie (書け as Conditional or Imperative),
which the row reorder does not touch.

### 4.2 Kanji 来る deinflects (design §9.1)

`JMDICT_POS_CLASS` became a multimap with a second row `("vk", "カ変・来ル")`,
and `class_for_pos(pos) -> Option<ClassId>` became
`classes_for_pos(pos) -> impl Iterator<Item = ClassId> + '_`. Before the fix,
`parse 来た` produced 来 and た both **unmatched**, and 来ない, 来ます and every
other kanji-written form failed the same way — because `vk` mapped only to
`カ変・クル`, whose suffixes are the whole kana word (き, こ, くる).

**The reading rule, and why it is table-derived.** Under `カ変・来ル` the
strip *succeeds* (くる ends in る) where `assemble::reading_for` had assumed it
fails, and would have yielded く + た = **くた**: wrong furigana, which that
function's own contract forbids. The rule added is that a `カ変・来ル` match
takes its stem reading from `カ変・クル`'s **same-named form**, whose suffix
over an empty stem *is* that reading — 連用形 き, 未然形 こ, 仮定形 くれ,
命令ｉ こい — followed by the surface after the step's own range. So 来た →
きた, 来ない → こない, 来れ → くれ, 来い → こい. A test asserts the two classes
carry the same ten form names so the lookup cannot silently miss. Nothing
hardcodes き, こ or くれ, which is the structural evidence that this is not a
port of v1's `kuruHack`.

**Snapshot movement**, both changes reviewed line by line:

- 来ます went from two junk spans (来 = "next year", ます = "trout") to one
  span `来る (Polite) [きます] to come`. The fix delivering outright.
- A new low-ranked *sibling* candidate `来る (-) [く]` appeared under an
  already-matched bare 来 in 彼はまだ来ていない。 Accepted: it moves no span
  boundary, changes no pre-existing entry and no ranking, and its reading く is
  `カ変・クル`'s 体言接続特殊２ suffix — verified directly in `table_data.rs`
  alongside 未然形 こ and 連用形 き, so it is produced by the same rule the new
  unit test pins rather than invented. Revertible by making 体言接続特殊２
  non-terminal, the same class of deferred fix design §9.2 already names.

**A correction to design §9.1.** Its claim that the `vk` multimap "only adds
candidates for surfaces unmatched today" is **imprecise**: it also adds
candidates to *matched* surfaces containing 来. The sibling entry above is the
observed instance. The claim's spirit holds — nothing legitimate was displaced,
and all eight score cells were unchanged — but a future reader should not rely
on the "unmatched only" wording.

**The coverage walk re-pinned from 21,760 chains to 22,783** (180 depth-0 +
1,953 depth-1 + 20,650 depth-2 over 21 mapped classes), re-derived deliberately
as its own failure message instructs. Depth-0 moving 170 → 180 is exactly
`カ変・来ル`'s ten forms arriving as the 21st mapped class.

### 4.3 Why the score did not move — the correct reason

The task report written at the time gave a **wrong** explanation for the
score's neutrality, and it must not be repeated. It claimed no `gold.jsonl`
line has a standalone conjugated kanji-来る span glued to an auxiliary. One
does.

`gold.jsonl` line 113 — 「きゃーーーっ、こっちに来ないでよ！」, `ent_seq`
1547720 — carries a gold span at `start=12 len=4`, which is 来ないで. Parsed
both ways: **before** the fix, 来 splits off alone and matches the unrelated
homograph 来/らい; **after** it, 来ない is correctly recovered as one 3-character
span reading こない, entry 来る, label Negative. The score does not move
because **gold wants 4 characters and the fix recovers 3** — a boundary miss
either way, for the separate reason that で is not pulled into the same chain.

That is a real and specific lead for whoever tunes chain depth: the fix is
working on this sentence and the metric cannot see it, because one more
character of the auxiliary chain is needed to score it.

---

## 5. What the switchover dropped

Removals were confirmed by **running the release CLI**, not merely by grep:

- **`--engine`** on `build-index` / `parse` / `score`.
  `parse … --engine v2` → `error: unexpected argument '--engine' found`.
- **`--no-v5-fallback`** on `build-index`; `--help` shows only `<XML> <OUT>`.
- **`romaji --particle`**; `--help` shows only `<TEXT>`. The flag existed to
  expose v1's particle-aware romaji path, which `engine::romaji` does not have.
- **`lookup`'s `type=` column** — it printed `verb_type`, which format 4 no
  longer stores.
- **`Index::prefixes_of`** — the fold-less variant. `prefixes_of_with(text, fold)`
  is the surviving read path; the fold is a parameter because an index is built
  with the fold its builder used and the walk has to agree with it.
- **`StemOptions`, `StemStats`, `BoundaryFlags`, `BoundaryHints`,
  `flags_from_worker`, `verb_type`, `VerbTypeId`, `ConjugationTable`,
  `ConjugationError`, `StartupError::Conjugation`** — all gone with their
  owners. `hints::VibratoTokenizer::boundaries(&str) -> BoundarySet` is what
  remains of the hints layer.
- **Index format 3 → 4:** `StoredRecord` lost `verb_type`. v2 emits no
  generated stems, so the field was `None` on every record. The header
  fingerprint already rejects a cross-read, so dropping it needed no new guard;
  v2's fingerprint is pinned at `0xfeae_49c6_63b9_fb24` by a stability test.
- **`jmdict_mini.xml`'s corrected tag** — the fixture's `vs-i` entry needed its
  POS tag corrected for v2's mapping.

**Test dispositions worth knowing about**, because two are not plain deletions:

- `tests/parse_snapshots.rs` held **four** tests, not one. Three were correctly
  deleted (one owned by `v2_snapshots_every_sentence`; one whose observable was
  ported into `engine_v2.rs`; one testing v1's generated-stem mechanism, whose
  subject no longer exists). The fourth,
  `corpus_has_thirty_sentences_covering_both_irregulars`, was **ported** rather
  than deleted: its subject is the *fixture corpus*, not either engine, and it
  became more load-bearing after the deletion, since `v2_snapshots_every_sentence`
  is now the crate's only coverage of する at all.
- `tests/index_roundtrip.rs::prefix_walk_returns_distinct_ascending_lengths` got
  a new two-entry inline fixture rather than a plain repoint. The original's
  premise was that 言 is 言う's *generated stem*, which the v2 builder cannot
  produce (it stores headwords only). The replacement uses two real headwords
  where one is genuinely a prefix of the other (言 / 言葉), walks 言葉狩り and
  asserts the exact lengths `vec![1, 2]` — stronger than the original's
  `len() >= 2` plus an ascending-order pair. The contract under test is
  unchanged.
- **One surviving code path lost its only test**, deliberately.
  `index/load.rs`'s `prefixes_of_with` has a root-node finality check that
  yields a `key_chars == 0` hit for an indexed empty key. `engine::build`
  stores headwords only and no JMdict entry has an empty headword, so nothing
  can now produce such an index; the v1 builder could, because one verb type
  stripped its whole surface. **The branch stays and its doc was amended** to
  read as unreachable-by-construction rather than as a live guarantee. Deleting
  a branch of a surviving public API is a behaviour change and outside this
  phase's scope; a later phase can remove branch and doc together once it
  decides the empty key will never be indexed.

---

## 6. What Phase 2 inherits

### 6.1 The metric clause, as amended

Phase 1's metric clause — both Phase 0 metrics at or above baseline in both
hint states — is **superseded, not met**. The tolerant gap is the structural
JMdict-priority ceiling master plan §7 Phase 3 already assigns to the
frequency layer, and no tool inside Phase 1 reaches it. The numbers stand
beside the amendment: hints on, v2 is **+10.7 points strict, −1.6 tolerant,
−0.9 entry top-1** against v1. Strict measures the particle-absorbing defect
class this project exists to fix; tolerant measures compound preservation,
which Phase 3 owns.

### 6.2 `tools/extract_jmdict_subset.py` needs a port

The extractor took the deleted `crates/jparser/assets/conjugations.json` as a
**required positional argument**, and it is the tool that regenerates the
JMdict fixture subsets (`jmdict_subset.xml`, `gold_jmdict_subset.xml`). It does
not run as written today. Its conjugation source needs rebuilding from
`crates/jparser/src/engine/table.rs`; until that port lands, the deleted asset
is recoverable from git history if a regeneration is genuinely needed. A note
to this effect is in the script's own docstring and in
`crates/jparser/tests/fixtures/README.md`.

This was a deliberate deferral, not an oversight: the asset is the copied
upstream data whose removal is the licence invariant's whole point, so keeping
it was never an option, and the extractor is a developer-only fixture
regenerator whose outputs are committed. **No regeneration is pending** — the
committed subsets are the reproducible artifacts.

Related: `crates/jparser/tests/fixtures/jmdict_matcher.xml` is now **unused**.
Its only consumer was `src/matcher.rs`'s test module. It is kept rather than
deleted because the eight lexical shapes it covers are still the ones a
lookup-layer test wants and nothing else in the tree covers them together;
adopting or dropping it is a decision for whoever next touches that layer.

### 6.3 Still-open items from Phase 1a handoff §8

Carried forward unchanged — none of these were touched here:

- **`is_allowed`'s hard interior constraint costs 64 of 2,163 gold spans**
  (hints-on), measured and judged rather than implemented. Softening it is an
  architectural change — three dedicated tests assert the current hard-forbid
  behaviour — not a constant tune.
- **`assemble::order`, not `cost.rs`, is where entry top-1's shortfall lives.**
  `cost.rs` governs which spans exist in the winning path; ranking candidate
  entries *within* a correctly-selected span is `order`'s job. The named
  suspect is `COUNTER`'s archaic disqualifier being entry-wide rather than
  per-sense.
- **`order`'s signal-free final tiebreak is load-bearing on display.**
  `Segment::reading` is `entries[0].reading`, so when every real signal ties,
  ascending `ent_seq` decides which *reading* the user sees. The committed
  snapshot has a live example: 本を読みました gives 本 the reading もと
  ("origin") over ほん ("book").
- **The 18 still-shattered snapshot spans** were never individually attributed
  to the structural ceiling.
- **`EXP` as a second signal for `COMMON`'s ceiling was tried and reverted** —
  real evidence, not an untried idea.

### 6.4 Phase 1b-i handoff §4's table findings, not taken here

Only finding 5 (the た/たい collision) was fixed. The other eight remain, all
span-shape or ranking questions in Phase 1a's metric-gated territory:

1. No `ATTACHES_TO` row requires 連用テ接続, so 高くて, 食べなくて, 高くない and
   食べたくない never decompose as one chain.
2. う attaches only to 未然ウ接続, so だろう, たろう and でしょう never
   decompose; よう only ever produces the spurious 食べよよう.
3. The voiced past だ row names the copula class, so 読んだら is unreachable
   while 読んなら, 読んな, 読んなれ and 読んだっ are generated.
4. せる/させる and れる/られる are not restricted by the inner class, so 食べせる
   and 書かられる are generated beside the natural forms.

Plus the godan 仮定形/命令ｅ tie and the `たい:ガル接続` non-terminal fix named
in §4.1 above.

### 6.5 The copyright-holder line is a default, not a decision

`LICENSE-MIT` reads `Copyright (c) 2026 the ClipYomi contributors`. It was
chosen as a default explicitly because no `authors` field exists anywhere in
the tree, and it is the one line in this phase with the maintainer's name on
it. Changing it is one commit.

### 6.6 `cargo test -p jmdict-source` should be a standing gate

This phase had to edit `crates/jmdict-source/tests/seam.rs`, and **none of the
plan's six gates covers that crate** — it was found by grep, not by the
compiler. A seventh gate was added for the last three tasks of the phase:

```
cargo test -p jmdict-source
```

**Recommend it permanently.** The plan's gate set has a hole and this closes it
for the cost of one fast command.

### 6.7 Two plan/design defects, recorded so they are not inherited

- **The 1b-ii design §8 is wrong about CI.** It says the dependency-licence
  check is "not automated; there is no CI to hang it on yet".
  `.github/workflows/ci.yml` exists and already carries a licence gate, so a
  future phase could host the check there. No new CI job was added here —
  that is outside this phase's scope — but the statement should not be
  repeated.
- **The plan's `cargo tree` licence filter lacks `BSL-1.0`.** That is the only
  reason it surfaced two crates as apparent hits: `error-code` and
  `clipboard-win`, both Boost-licensed and permissive. The actual dependency
  read found **no GPL-family licence in 685 normal edges**. Add `BSL-1.0` to
  the allow-pattern before re-running it.

A third item, smaller: a `cargo fmt --check` job is now a safe addition to CI
and was not made. The comment at the top of `ci.yml` used to forbid it,
citing the hand-laid conjugation table whose layout a wholesale run would have
destroyed; that table and its module are gone and both crates are
rustfmt-clean, so the comment was rewritten to say so.
