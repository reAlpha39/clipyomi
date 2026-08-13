# JParser Phase 1B — Handoff

Phase 1B is complete, and with it Phase 1: dictionary matching with verb
conjugation recursion, the min-cost segmentation DP, conjugation-label
rendering, reading reconstruction, and the public `parse()` surface —
verifiable through `jparser-cli parse` and through insta snapshots over real
JMdict vocabulary. 210 tests, 96.37% line coverage,
`cargo clippy -p jparser --all-targets -- -D warnings` clean.

This document records what Phase 2 needs and what Phase 1B deliberately left
undone. It exists because these decisions are not recoverable from the diff.

Read `docs/superpowers/2026-08-13-jparser-phase1a-handoff.md` first if you have
not. Its index-build section is still live and still unimplemented, and Phase 2
is the phase that triggers it — see "What Phase 2 inherits" below.

**Plan:** `docs/superpowers/plans/2026-08-13-jparser-phase1b.md`. Treat the plan
as a historical artifact **with two exceptions that are still live**: its
Self-Review §5 (fidelity divergences from ta-old) and §6 (residual gaps). Those
two lists are Phase 6's differential-run budget and the open-questions register
respectively, and both were corrected during the final review. Everything else
in the plan describes work already executed; where it disagrees with the code,
trust the code.

**Layout note.** The Rust workspace moved from `ta/` to the repo root after
Phase 1B merged. The plan and the Phase 1A plan still say `ta/crates/jparser/…`
throughout; the path is now `crates/jparser/…`. Nothing else about them changed.

---

## The public surface Phase 2 consumes

```rust
pub fn parse(
    index: &Index,
    table: &ConjugationTable,
    text: &str,
    opts: &ParseOptions,
    hints: Option<&dyn BoundaryHints>,
) -> Result<ParseResult, ParseError>
```

Four things about this signature that matter for the Tauri layer:

- **It takes `&Index` *and* `&ConjugationTable`.** They are separate, and both
  must outlive the call. The Tauri state holds both. The index is bound to its
  asset by `conjugation_fingerprint`, so a mismatched pair fails at
  `Index::open`, not here.
- **`ParseOptions` is deliberately empty.** It exists so the first real option
  is not a breaking change. Do not delete it to tidy up.
- **`hints: Option<&dyn BoundaryHints>` is the Vibrato seam.** Phase 1B ships
  the trait and test stubs only; there is no `morph.rs`. `BoundaryHints` lives
  in `segment.rs` and is re-exported from the crate root. `None` behaves
  *exactly* like an implementation returning `false` everywhere, and there is a
  test asserting that equality — so Phase 5 can land a real implementation
  without changing a single Phase 2 call site.
- **`ParseError` has one variant, `Index`.** Reading the mmap'd payload is the
  only fallible step in the whole phase.

`ParseResult::segments` is a **contiguous cover**: every character belongs to
exactly one segment, matched or not, in ascending `start` order, empty iff the
input is empty. Unmatched runs are emitted as segments with `matched: false`,
`entries: []`, `reading: None`. This diverges from ta-old, which emitted
nothing for them — it is what makes the cover contiguous, and the UI can rely
on it rather than reconstructing gaps.

**Every offset is a char offset.** `Segment::start`, `Segment::len`, and every
internal offset are counted in `char`s, never bytes. The single
`&str` → `Vec<char>` conversion in the entire phase is at the top of `parse`.
The UI will want byte or UTF-16 offsets for DOM ranges; that conversion is
Phase 2's to write, and it is the most likely place for an off-by-one to enter
the system.

`Segment::reading` is `entries[0].reading` — see the `entry_id` ordering
divergence below, because that is what decides which entry is `entries[0]`.

---

## Decisions that departed from the plan

**`entry_data` uses `HashMap::entry`, not the plan's `contains_key` + `insert`.**
The literal form the plan specified trips `clippy::map_entry`, a hard error
under the `-D warnings` gate the same task runs. Behaviour is identical on both
the hit and miss paths. Documented at the site.

**Two size-driven module splits the frozen contract §7 does not have.**
`recurse` lives in `src/matcher/verb.rs` (a *child* module, which is the only
way it reaches `matcher`'s private `strict_eq` and `commit` — do not flatten
it), and `sort_matches` lives in `src/rank.rs`. Both were forced by the
800-line-per-file cap once in-module tests are counted, because `pub(crate)`
items are unreachable from `tests/`. The plan's File Structure table is
authoritative over contract §7. Contract §7 is now stale in four rows and
should be folded back — see "Known issues" below.

**The `lib.rs` split was deliberately NOT taken.** `lib.rs` finishes at 673
lines, over contract §7's "~400 → split to `entry.rs`" advisory but well under
the 800 hard cap. The contract's own test-home table puts the
reading-reconstruction and kuruHack tests in `lib.rs`'s `mod tests`, which only
makes sense if the code under test lives there. `entry.rs` and `label.rs` do
not exist by contract. Headroom is ~130 lines.

**The plan's stated rationale for the `entry_id` ordering was factually wrong,
and is now corrected.** It claimed ta-old's `firstJString` key "was a heap
address and was never reproducible across runs." It was not: the pointer is
inside `dict->strings`' single contiguous allocation, and the comparison only
ever runs after `dictIndex` has already tied — i.e. within one allocation — so
relative order was stable within a run and equalled JMdict document order. The
port's ascending sort is a **deliberate direction flip**, kept because lower
`ent_seq` is generally the more established entry and makes a better default
for `Segment::reading`. The code was not changed; §5 now records it accurately.

---

## Known issues carried into Phase 2

**The JMdict subset's licence is an open decision, not a resolved one.**
`crates/jparser/tests/fixtures/jmdict_subset.xml` is CC BY-SA 4.0 (© EDRDG),
committed as a separately-licensed data asset with the EDRDG notice inline and
mirrored in `tests/fixtures/README.md`. The crate is `GPL-2.0-only`, and CC
BY-SA 4.0 is one-way compatible with GPL **v3**, not v2. Shipping it this way is
standard practice for projects that bundle JMdict, and it was implemented that
way — but it was explicitly *not* accepted when raised. **Settle it before the
first public release**, not after.

**`to_katakana` still has no library caller.** Phase 1A predicted the first real
consumer would want a different signature from at least one dormant public item,
and named `to_katakana`'s `Option` return as the likely candidate. Phase 1B did
not exercise it — it remains test-only. Phase 3 (furigana display modes) is its
first consumer, and the plan's Scope table already flags a
`to_katakana` `>= 0x3097` bail conflict waiting there. Expect to change the
signature; nothing outside `kana.rs` depends on it yet, so it is free to do now.

**`Segmentation::total_cost` never reaches `ParseResult`.** Port design §10's
"assert the cost, not just the winning segmentation" is satisfied only by
`segment.rs`'s in-module tests. The snapshots pin the winner and the
alternatives list, not the cost. Catching cost regressions end to end needs a
`total_cost` field on `ParseResult` — a contract change, deliberately not taken.
The field carries the phase's single narrowest `#[allow(dead_code)]`, on the
field itself; without it the clippy gate fails on "field is never read".

**A duplicate-looking alternative in the snapshots is faithful, not a bug.**
`昨日は宿題をしました。` renders `しま (Past) [-] conjecture` three times: one
entry (`ent_seq` 2854156, 揣摩/しま) reached by three distinct chains through
the duplicate `vs` type pair. ta-old keeps all three too. **Phase 3's display
layer needs a per-`(entry_id, label)` dedupe** — otherwise the next person to
see this files it as a parser bug and "fixes" correct code.

**`matcher/verb.rs`'s Stem-skip arm advances neither `depth` nor the cap.** A
zero-width `Stem`/form-0 *cycle* in a conjugation asset would recurse until the
stack overflows. The shipped asset has exactly six zero-width stem-skip edges
and is acyclic (verified by enumeration), and `Index::open`'s fingerprint check
binds an index to its asset — so there is no hazard today. But
`ConjugationTable::from_json` accepts arbitrary JSON, and nothing guards or
tests it. A `ponytail:` comment names the ceiling and the upgrade path. This was
raised and the decision was to ship the comment; **if Phase 2 ever loads a
user-supplied or downloaded conjugation asset, revisit it before that ships.**

**The `kuru_hack` `want > 3` branch is uncovered** and has no test in any task.
Accepted knowingly.

**Contract §7's module map and test-home table are stale in four rows** —
`recurse` → `matcher/verb.rs`, `sort_matches` → `rank.rs`, matcher tests split
across two files, `tests/parse_irregular.rs` added. Fold them back if the
contract is going to outlive the plan.

**`segment.rs` is at 778 of the 800-line cap** — the tightest file in the crate,
with 22 lines of headroom. Nothing in Phase 2 should need to touch it. If
something does, the sanctioned next split is moving `counter_after_number` and
`isolated_katakana_run` out; take it deliberately and flag it, as the two
existing splits were.

---

## Fidelity divergences from ta-old

Nine documented, each deliberate, each commented at the site, each a
pre-approved delta for Phase 6's differential run. **Do not re-derive this list
— read plan Self-Review §5.** Two were added during the final review and are
easy to miss if you only read the original draft:

- the `entry_id` ordering direction flip (systematic across every
  multi-alternative span — the single largest expected diff);
- names-inexact suppression applied on the verb path as well as the non-verb
  path in `matcher::commit`, where ta-old gates only the non-verb branch
  (dormant until JMnedict is wired in, at which point it becomes live).

The other seven: the Potential-Potential `retain` vs swap-remove, `inexactMatch`
narrowed to `bool`, `dictionary_form` using the first remove-tense/form-0
conjugation rather than the first that strips, `strict_eq` folding ASCII case
only, unmatched runs emitted as segments, the zero-length match drop, and
`dictIndex` being dropped entirely.

Phase 6 without this list is "investigate every difference." With it, it is
"check these nine, investigate the rest."

---

## What Phase 2 inherits

**The index-build hazard is now Phase 2's.** The Phase 1A handoff's dedicated
section stands unchanged and is still the authoritative design: build into
`<root>/.build-<nonce>/`, `fs::rename` to `<root>/gen-<N>/`, readers take the
highest generation. Phase 1B's only deliverable there was the corrected
guidance in `load.rs`'s SAFETY block and `Index::open` doc, which is in place —
including *why* a rename-over-a-live-directory swap is the broken answer
(`Index::open` reads five files in sequence, so a straddling open can splice one
generation's `entries.idx` onto another's `entries.bin`).

**The trigger has now arrived.** Phase 1A named it precisely: "the first commit
where a single process keeps an `Index` alive across a rebuild — concretely,
when `ensure_dictionary` is written in the Tauri layer." That is Phase 2's first
week. Estimated cost when it lands, from the 1A handoff: a new
`index/generations.rs` (~60 LOC + ~60 LOC tests), one line in `index/mod.rs`,
comments only in `load.rs`, and ~15 LOC of Tauri-side `RwLock<Arc<Index>>` plus
a mutex held across the whole rebuild. Two open judgment calls there — `fsync`
before the rename, and true hot-swap versus a brief "reloading dictionary…"
pause — are product decisions, not engineering ones. **The second one collapses
the in-memory piece to about five lines if you can accept the pause.**

Phase 1B kept this seam clean on purpose: **no Phase 1B type stores the index
directory path.** `parse` takes `&Index`. The generation layout can land without
touching a line of parser code.

**Windows is still unexercised.** The 1A handoff budgets an afternoon on a
Windows box running the four filesystem operations the generation layout needs
before Phase 2 ships. Nothing in Phase 1B changed that.

---

## Invariants Phase 2 must not break

Phase 1A's invariants all still hold. These are the ones Phase 1B added or
made load-bearing:

- **`Match::chain` empty ⇔ non-verb.** This is the port's encoding of ta-old's
  `conj[0].verbType == 0`. Nothing anywhere may encode "not a verb" as
  `verb_type == 0`; that was ta-old's 1-based scheme and Phase 6 re-adds the
  `+ 1` only for comparison. The invariant holds at seven consumers.
- **`VerbTypeId` is 0-based everywhere.** `rank::group_key` adds 1 *only* inside
  its sort key, to reproduce ta-old's ordering. Nothing else does, ever.
- **The DP's two tie-breaks differ on purpose.** The skip transition uses a
  strict `>`; the match transition uses `>=`. Harmonizing them silently changes
  which segmentation wins. Both are pinned by tests that name the counterfactual
  *shape*, not just the cost — which is the only way that regression is
  catchable.
- **Emission order is load-bearing** — for `sort_matches`' stable Pass C and the
  matcher's dedup. `matches_at` emits in ascending `key_chars` then stored
  order; preserve it.
- **`INDEX_FORMAT_VERSION` is 3**, because `EntryData` gained `readings`.
  `EntryData`'s field order is `id, readings, senses` and **bincode is
  positional, so field order is wire format**. The version bump is what makes an
  old on-disk index refuse to load rather than mis-decode. `Index::open`
  validates the version before touching any positional payload.
- **`kana::strip_suffix_unified` is shared** by conjugation-chain resolution,
  stem generation, *and now* `reconstruct_reading`'s `strip_remove_suffix`. All
  three must use the same expression, not a re-derived equivalent — otherwise a
  kana stem exists at parse time that did not exist at build time.
- **`types_named` returns `Vec<VerbTypeId>` and every caller must handle more
  than one.** The duplicate type names (`vk`, `vs`, `v5r-i`, `v5uru`) are
  deliberate, and that pairing *is* the kuruHack. Never index `[0]`.
- **All public types are owned and immutable.** `segment()` takes
  `&[Vec<Match>]` and must not mutate it; the stale-`COUNTER` clear operates on
  the span's clones.
- **`crates/jparser` has no Tauri, UI, or HTTP dependency, and no new
  dependency was added in Phase 1B.** `insta` was already a dev-dependency.
  There is no `tempfile` crate — tests use
  `std::env::temp_dir().join(format!("jparser-test-{name}"))` plus
  `let _ = std::fs::remove_dir_all(&dir);`.
- **Never run `cargo fmt -p jparser`.** It reformats `conjugation.rs`,
  `kana.rs`, and `romaji.rs`, which this phase deliberately left alone —
  `conjugation.rs` is not rustfmt-clean and "fixing" it is a defect. Running
  `rustfmt` on `src/lib.rs` cascades the same way, because it is the crate root.
  Format individual leaf files, then check `git diff --stat`.

---

## Verification

```
cargo test -p jparser                                 # 210 tests
cargo clippy -p jparser --all-targets -- -D warnings
cargo llvm-cov -p jparser --summary-only --fail-under-lines 80   # 96.37%

cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_subset.xml /tmp/idx
cargo run -q -p jparser --bin jparser-cli -- parse /tmp/idx "昨日は宿題をしました。"
```

The snapshot suite (`tests/parse_snapshots.rs`) is the highest-value regression
in the crate — it answers "did my refactor change the parse?" over 30 sentences
of real vocabulary. It runs offline from a fresh clone against the committed
1471-entry curated subset, so it is immune to JMdict's daily rebuilds.

To regenerate the subset after changing the corpus:

```
python3 tools/extract_jmdict_subset.py \
  /tmp/jmdict/JMdict_e.xml \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  crates/jparser/assets/conjugations.json \
  crates/jparser/tests/fixtures/jmdict_subset.xml
```

`JMdict_e.xml` is not in the repo and must be fetched from
`http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz`. Update the `**Retrieved:**`
date in `tests/fixtures/README.md` when you do — a test greps for the
unfilled placeholder and refuses to commit around it. The tool copies entries
**verbatim by text slicing and never uses an XML parser**, because JMdict's POS
codes arrive as entity references (`<pos>&v5r;</pos>`) and any DTD-aware parser
expands them into English prose — destroying exactly the field
`record::headwords` reads.
