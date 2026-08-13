# JParser Phase 1B — Design Addendum

**Date:** 2026-08-13
**Status:** Approved, ready for implementation planning
**Amends:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md`
**Follows:** `docs/superpowers/2026-08-13-jparser-phase1a-handoff.md`

This is an addendum, not a replacement. The port design already specifies Phase
1B's algorithms in detail — the matcher's recursion rules (§5.3), the DP's
twelve scoring constants verbatim (§5.4), conjugation-label rendering (§5.5),
and the `BoundaryHints` contract (§5.7). Those sections stand as written and are
not restated here.

This document records only what the port design left open, plus two corrections
to statements made during Phase 1A.

---

## 1. Scope

Phase 1B completes **Phase 1 of the port design's §11 phasing**. Phase 1A
delivered the conjugation table, kana/romaji, JMdict streaming, index build with
stem generation, and the FST. Phase 1B delivers the rest:

- `matcher.rs` — `matches_at()` with verb-conjugation recursion (§5.3)
- `segment.rs` — the DP, the backtrack pass, `SortMatches` (§5.4)
- Conjugation label rendering (§5.5)
- The `BoundaryHints` trait, without its Vibrato implementation (§5.7)
- The `parse()` public surface: `ParseResult`, `Segment`, `Entry`, `ParseError`
  (§5.1), including reading reconstruction with the `kuruHack` kanji/kana twin
  pairing for irregular verbs
- A `parse` subcommand on `jparser-cli` — §11's "verifiable via a CLI harness"
- Unit tests, cost assertions, and insta snapshots (§10)

### Out of scope, with the phase that owns each

| Item | Phase |
|---|---|
| Vibrato / `morph.rs` — the `BoundaryHints` implementation | 5 |
| Furigana display modes, and the `to_katakana` conflict in §3 below | 3 |
| Differential run against ta-old | 6 |
| Tauri shell, clipboard, `ensure_dictionary` | 2 |
| Generation-directory index layout (see the handoff doc) | 2 |
| JMnedict — `NAME_DICT_*` implemented but dormant, nothing sets `IS_NAME` | deferred |
| Half-width katakana offset map — `ponytail:` comment only | deferred |

---

## 2. Corrections to Phase 1A statements

**The differential run is Phase 6, not Phase 1B.** The Phase 1A plan's
self-review listed it as deferred to Phase 1B, and the Phase 1A handoff repeated
that. Port design §11 places it in "Phase 6 — Verification", alongside the
Playwright suite and the per-platform checklist. Phase 1B does not include it.

**MeCab is Phase 5, deliberately late.** §11: it is "a ±10 tiebreaker on a
100/500 baseline, so it cannot be validated until the DP it nudges is
known-good." Phase 1B defines the trait so the DP can take
`Option<&dyn BoundaryHints>` and be tested against a stub; it writes no Vibrato
code.

---

## 3. A spec-vs-implementation conflict, deferred to Phase 3

Port design §5.6 specifies that katakana furigana mode should "bail and return
the raw reading if any char is `>= 0x3097`". Phase 1A shipped `kana::to_katakana`
with pass-through instead: the Phase 1A plan's own self-review argued the bail
would wrongly fail on inputs like `to_katakana("ヴ")`.

One of the two is wrong, and it is a user-visible furigana behaviour. It does
not block Phase 1B — §11 puts the four furigana display modes in Phase 3 — so
it is recorded here and left for whoever implements them. Phase 1B needs reading
*reconstruction*, not reading *display*.

---

## 4. The matcher/segmenter seam

The port design fixes the module split and the public types but not the internal
interface between them. Phase 1B uses a **precomputed match table**:

```rust
// matcher.rs
pub(crate) fn matches_at(
    index: &Index,
    table: &ConjugationTable,
    text: &[char],
    i: usize,
) -> Result<Vec<Match>, ParseError>;

// segment.rs — takes no Index and no ConjugationTable
pub(crate) fn segment(
    text: &[char],
    matches: &[Vec<Match>],
    hints: Option<&dyn BoundaryHints>,
) -> Segmentation;

pub(crate) struct Segmentation {
    spans: Vec<Span>,
    total_cost: i32,
}

/// One chosen span. `matched` is false for a skipped run.
pub(crate) struct Span {
    start: usize,        // char offset
    len: usize,          // in chars
    matched: bool,
    matches: Vec<Match>, // every match aligning to this span, sorted
}
```

Note the asymmetry: `matches_at` returns `Result` because it reads the mmap'd
payload and can hit a corrupt index; `segment` is infallible by construction,
operating only on an in-memory table it was handed. That is deliberate, not an
omission — if `segment` ever needs to fail, the DP has grown an I/O dependency
it should not have.

**Why this shape.** §10 requires the DP tests to assert *the cost*, "not just
the winning segmentation, so a scoring regression is caught even when the winner
does not change". Making `segment()` a pure function of a match table plus hints
means those tests need no dictionary, no index, and no fixture — a hand-built
`Vec<Vec<Match>>` is enough, and `total_cost` is directly assertable.

A lazy `MatchSource` trait was considered and rejected: the DP visits every
position regardless, so laziness saves nothing real, and it adds a trait plus
lifetimes for no gain. A fused walk-and-score was rejected because it makes the
DP untestable without a real index and contradicts §3's module list.

---

## 5. Module homes for things §3 does not place

§3's architecture lists no `label.rs` and no `entry.rs`, so:

- **Conjugation label rendering** lives in `matcher.rs`, next to the `Match`
  chain it consumes.
- **The `BoundaryHints` trait** lives in `segment.rs`, its only consumer, and is
  re-exported from `lib.rs`.
- **Entry assembly** (resolving `entry_id` through `Index::entry()`, applying
  `kuruHack`) lives in `lib.rs` beside `parse()`.
- **`Sense`** re-exports `index::SenseData` rather than defining a parallel
  owned copy of the same five fields.

If `lib.rs` passes ~400 lines, entry assembly splits into its own module. That
is a deviation from §3's module list and should be flagged in the plan rather
than taken silently.

---

## 6. Test fixtures

Two distinct dictionaries, for two distinct purposes:

- **Matcher golden tests** use a hand-written ~20-entry dictionary, per §10 —
  "fast and immune to dictionary version drift".
- **DP tests** use no dictionary at all; hand-built match tables, run with
  `None` hints and with a stub `BoundaryHints`.
- **insta snapshots** over ~30 real sentences run against a **committed curated
  JMdict subset**: JMdict is fetched once, and only the entries those sentences
  need are extracted into a fixture of a few hundred KB. This keeps snapshots
  deterministic, runnable offline from a fresh clone, and immune to JMdict
  version drift, while still exercising real vocabulary.

The curation step needs a real `JMdict_e.xml`, which is not in the repo. The plan
must treat obtaining it as an explicit task with the fetch called out, not assume
it is present.

Snapshot sentences must include する or 来る. Those are the irregular verbs whose
stems are the empty string, and the empty-key path was broken and fixed late in
Phase 1A — it deserves a standing regression.

Coverage stays at **80% on `crates/jparser`**, per §10.

---

## 7. Risks the plan must address explicitly

**The subtle matcher rules will pass by accident if not tested directly.**
§5.3 requires that informal `Stem` conjugations at depth > 0 neither consume
depth nor get added to the match list, and that `Potential Potential` duplicates
are dropped. Both are easy to implement approximately and hard to notice when
wrong. Each needs its own test asserting the resulting chain.

**Cost, not just the winner.** A scoring constant applied in the wrong branch
often leaves the winning segmentation unchanged on short inputs. Assert cost.

**Offsets stay char-based everywhere**, including `BoundaryHints::pos` and
`Segment.start`/`len`. Mixing in a byte offset corrupts alignment on every
multi-byte character, which is all of them.

**The backtrack pass is not optional.** §5.4: collect every match aligning to a
chosen span, not only the winners. Skipping it yields a single guess per segment
and silently removes the alternative readings the definition list exists to show.

---

## 8. Invariants inherited from Phase 1A

Listed in full in the Phase 1A handoff; the ones Phase 1B can actually break:

- No Phase 1B type stores the index directory path — pass `&Index` or
  `Arc<Index>`. This keeps Phase 2's generation-directory work cheap.
- `kana::unify` is prefix-stable and `kana::strip_suffix_unified` is shared by
  conjugation resolution and stem generation. Changing kana folding changes both.
- A stem carries exactly one verb type; `StoredRecord.verb_type` is `None` for a
  plain headword and `Some` for a stem.
- ta-old stored `verbType` as `vt + 1` with `0` meaning "not a verb"; this port
  uses a 0-based id in an `Option`. Phase 6's differential run compares against
  the old encoding.
