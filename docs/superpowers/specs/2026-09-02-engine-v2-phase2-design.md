# Engine v2 Phase 2 — k-best readings behind a tolerant oracle gate

**Date:** 2026-09-02
**Master plan:** [`2026-08-27-engine-v2-master-plan.md`](2026-08-27-engine-v2-master-plan.md) §4.1, §4.2, §4.3, §5.2, §7 Phase 2, §10
**Phase 1a design:** [`2026-09-01-engine-v2-phase1a-design.md`](2026-09-01-engine-v2-phase1a-design.md) §6.1, §7, §7.5
**Phase 1a handoff:** [`../2026-09-01-engine-v2-phase1a-handoff.md`](../2026-09-01-engine-v2-phase1a-handoff.md) §5
**Phase 1b-i handoff:** [`../2026-09-02-engine-v2-phase1b-i-handoff.md`](../2026-09-02-engine-v2-phase1b-i-handoff.md) §2
**Branch:** `feat/engine-v2`, from `a8ac745`
**Status:** Design, approved in session, not implemented

---

## 1. Scope

Phase 2 is master plan §7's "k-best readings + UI": the engine returns the
three cheapest distinct segmentations of a sentence instead of one, the
harness measures whether the second and third are worth showing, and the
main window lets the user cycle between them. Master plan §4.3 names the
purpose: when the engine picks wrong, surface the other reading to the user
instead of guessing on their behalf.

**In.**

- `lattice::k_best`, an exact k-best over the Phase 1 lattice, of which
  today's `best_path` becomes the k = 1 case (§3).
- `ParseOptions::with_readings(k)`; `ParseResult.alternatives`, readings 2..k as
  full covers (§4).
- `jparser-cli score --readings K`, adding oracle@K lines (§5).
- The main window: `Reading n of k` radio rows in the existing right-click
  menu, a key pair to cycle, and the hover path following the displayed
  reading (§6). Lands only if the gate passes (§7, §8).

**Out.**

- Per-region alternates ("the other ways this stretch could split"). A
  second reading-selection model beside the one oracle@k measures; not
  until the whole-sentence one has earned its place.
- Scores on the wire. The order is the rank; the `i32` cost has no meaning a
  user could act on. Master plan §7's "reading list with scores" is amended
  in §11.
- Softening `is_allowed` into a penalty, or any other change to the cost
  model. Alternates come from the same constrained lattice as top-1 (§2.3).
- A setting for k. Three readings, fixed.
- Any change to what `k = 1` produces.

**Invariant, held at every commit.** Top-1 does not move: Phase 1b-i's
eight measured cells are byte-identical after each commit of §7, on both
hint states. k = 1 through the new code is the old DP, and the same `score`
run that measured Phase 1 proves it.

## 2. Why now: the oracle@∞ ceiling

### 2.1 What was measured

Master plan §7's gate for this phase is oracle@k, which cannot be measured
until the k-best DP exists. Before building it, a throwaway spike measured
the bound that needs no DP: for each of the gold corpus's 2,163 spans, does
`lex::candidates` already produce a candidate at the gold `(start, len)`, and
one carrying the gold `ent_seq`? No path over the lattice can reproduce a
span the lattice never offered, so this bounds oracle@k for every k. Hints
on means the candidate also passes `cost::is_allowed` under Vibrato's
boundaries, the only way hints touch candidate existence.

Two conventions, both so the ceiling sits beside the eight cells rather than
beside a different denominator:

- **Fallback held at top-1.** An unmatched run can cover any span, so the
  literal k = ∞ span ceiling is 1.0 and says nothing. A gold span top-1
  already reproduces counts as reachable; otherwise a candidate must exist.
- **Tolerant ceiling** adds spans some allowed candidate could forgive: a
  candidate containing the span with both its endpoints on gold boundaries,
  the same rule `score`'s `accumulate` applies to the parse.

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
(`corpus_staging/gold_index` is format 3 and no longer opens); Vibrato
dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.
The spike's own top-1 tallies equal `score`'s on every cell.

### 2.2 What it found

2,163 gold spans: 1,554 with an `ent_seq`, 609 `null`.

| | strict top-1 | strict ceiling | tolerant top-1 | tolerant ceiling | exact-entry ceiling |
| --- | --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573) | 0.8807 (1905, +332) | 0.7975 (1725) | 0.8858 (1916, +191) | 0.9923 (1542 / 1554) |
| hints on | 0.8031 (1737) | 0.8937 (1933, +196) | 0.8521 (1843) | 0.9020 (1951, +108) | 0.9517 (1479 / 1554) |

The exact-entry column is over non-null spans only: a candidate with the
gold `(start, len, ent_seq)` exists and is allowed. Hints on, the 75 non-null
spans without one decompose as 63 forbidden by `is_allowed`, 9 with no
candidate at the span at all (`嫌な`, `完成する`, `開始する`, `見ないで`,
`来ないで`: adjective + な and noun + する gold units, and the ないで chain),
and 3 where the span exists but not the entry (`あり` wanting 有り 1296400).
Hints off the 63 return and the residue is 12.

So the lexicon is not the bottleneck. Ninety-nine percent of gold entries
are in the lattice hints off, ninety-five hints on. The whole strict gap is
the DP's choice plus the hard constraint.

The number the gate is built on is narrower than the strict headroom. Of
the 196 hints-on spans k-best could newly reach, 92 are compounds the
corpus splits and the tolerant metric already forgives; winning them is
re-splitting what master plan §5.2 wants kept. The other **100 spans, in 66
sentences,** are unforgiven top-1 misses with the exact candidate reachable.
Per sentence: 84 need no flip, 45 need one, 13 need two, 8 need three or
more (maximum five). A whole-sentence reading must flip every one at once,
so the multi-flip sentences sit deep in any ranking; the 45 single-flip
sentences are what k = 2 and k = 3 can plausibly hold.

### 2.3 What it decides

**Phase 2 proceeds.** The ceiling is not small: up to +9 strict and +5
tolerant points hints on. It also sets the bar: oracle@3 is bounded by those
numbers, and every span in the headroom is reachable at k = 1 too, by a
different cost. The design must therefore say why k-best rather than another
cost-model pass. The answer is the shape of the residue. Phase 1a's cost
model already spent its evidence terms (per-character cover, `COMMON`,
`INEXACT`, the `PRONOUNCE` crossing price) and measured `exp` as a net loss;
what remains ties on evidence. A cost model must break a tie one way for
every sentence; a reading list hands the tie to the reader, who has the
context the lattice does not. The gate in §8 asks whether three readings
capture half of what is reachable. If they do not, the tie was not the
problem and Phase 2 ships nothing, as master plan §7 allows.

**Alternates come from the constrained lattice.** Master plan §4.3 calls
k-best "the escape hatch that surfaces the suppressed reading", and hints
on, 63 gold spans are suppressed by `is_allowed` itself: they exist as
candidates and no path may use them. Three routes were weighed. Running
alternates over the unconstrained lattice with hints as a penalty reaches
all 63 but reopens the softening clause Phase 1a §7.5 declined, touches
`cost.rs`'s documented existence/pricing split and three tests asserting
hard-forbid, and changes k = 1 unless separately gated. Appending the
hints-off top-1 as an extra reading reaches some of them for one more DP run
and a second oracle line, at the cost of two lattices to reason about in the
UI. Constrained only keeps one DP, one oracle number, and k = 1 provably
unchanged. The 63 are recorded as the constraint's price, the judgment Phase
1a §7.5 already made with the number in hand.

**Route (master plan §4.1).** Own k-shortest-path over the Phase 1 DP. The
other two routes enumerate tokenizer morphemes over IPADIC, and since Phase
1a §6.1 hints supply boundaries only, there is no tokenizer lattice in the
engine to take an N-best from. `lattice.rs`'s module doc already promised
this shape: "Phase 2's k-best swaps the single winner `best[]` holds for a
bounded list without moving anything else in this file."

## 3. `lattice::k_best`

### 3.1 Shape

`best[to]` becomes a bounded list of at most k nodes, sorted by the
comparator `relax` uses today: cost ascending, then longer step first, then
insertion order. A node is `(cost, from, from_rank, step, key)`; backtrack
follows `(from, from_rank)` instead of `from` alone.

```rust
pub fn k_best(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
    k: usize,
) -> Vec<Vec<Step>>;

pub fn best_path(chars, by_start, bounds) -> Vec<Step> {
    k_best(chars, by_start, bounds, 1).remove(0)
}
```

`k_best` returns paths in cost order, at most k of them and always at least
one, each a contiguous cover of `0..chars.len()`. Empty input yields exactly
one empty path, so the wrapper's `remove(0)` never panics and `best_path`'s
existing empty-input test holds unchanged. `parse` calls
`k_best`; nothing else calls `best_path` in production, so the wrapper
exists for the existing unit tests and for the invariant's statement.

### 3.2 The reading key

`key` is a `u64` hash of the path's matched span sequence: updated from the
prefix's key on every `Step::Matched` by mixing `(start, len)`, carried
unchanged across `Step::Unmatched`. Two nodes at the same position with the
same key are the same reading; the list keeps one, the cheaper, or on a full
tie the earlier inserted. One rule, three consequences:

- **Same span, different entries is one reading.** `assemble::to_segment`
  already gathers every sibling at the span and orders them with `order`;
  the DP's chosen candidate only ever set the span. Without the key, k = 3
  on `東京` would return the same segmentation three times with a different
  entry pointer each.
- **Unmatched fragmentation is one reading.** A ten-character unmatched run
  has 512 equal-cost splits. Today `relax`'s longer-step tie-break hides them
  at k = 1; the key hides them at any k. A mixed-script split is strictly
  cheaper and still wins, as today.
- **Nothing forbidden appears in any reading.** Forbidden candidates are
  never relaxed. Same line of code as today.

Hash collisions merge two genuinely different readings into one. At tens of
spans per sentence and a 64-bit mixer that is a non-event, and the failure
is a missing alternate, never a wrong segment.

### 3.2a Dominated readings

The key leaves one class of near-optimal path standing: a reading that is
top-1 with one matched word left unmatched. `UNMATCHED` is 30 per character
against at most `COVER + INEXACT` = 9 for the costliest one-character match,
so such a variant is always at least 21 dearer than the reading it degrades
and never wins at k = 1 — but on a short or unambiguous sentence it is what
fills readings 2 and 3, and "東京 with 都 unmatched" is not a second reading
of 東京都, only a worse copy of the first.

A reading is **dominated** when a cheaper reading matches every span it
matches. `k_best` keeps `4k` raw readings per position, and after the
backtrack returns the first k that no cheaper kept reading dominates. Rank 0
is never dominated, so k = 1 is untouched. A reading that leaves a character
unmatched because nothing matches it (東 in a dictionary that lacks it, read
against 京都) has a span the cheaper readings do not, and survives.

The factor is a `ponytail:` ceiling: a top-1 with more than `3k` matched
spans has more one-word-unmatched variants than the raw list holds spare
slots, and a genuine alternate priced above them (a PRONOUNCE crossing plus
INEXACT, about +33) can fall off the end. The upgrade path is dominance
pruning inside the DP lists, which is safe because domination at a position
survives every extension.

### 3.3 Exactness, and why rank 0 is today's answer

A reading in the top-k at `to` is a reading at some `from` plus one step. If
that prefix were outside `from`'s top-k, the k cheaper distinct readings at
`from` extended by the same step would be k cheaper distinct readings at
`to`, displacing it. So k per position suffices for exact top-k over
distinct readings.

Rank 0 at every position is today's single winner: the comparator is the
one `relax` already applies, extensions from a position's rank-0 node are
inserted before extensions from its lower ranks, and a same-cost same-key
later insert loses on insertion order exactly as `Some(_) => false` loses it
now. Complexity is `O(E · k²)` over the lattice's edges; at k = 3 it is the
same loop with a three-element list.

## 4. Public API

`ParseOptions`, `#[non_exhaustive]` and `Default` since Phase 1b-ii for
exactly this, gains its first field:

```rust
pub struct ParseOptions { pub readings: usize }        // Default: 1
impl ParseOptions { pub fn with_readings(k: usize) -> Self }  // k.max(1)
```

`ParseResult` gains `alternatives: Vec<Vec<Segment>>`: readings 2..k in cost
order, each a full cover, empty with the default options. `segments` stays
top-1, so every consumer and the wire-shape test keep meaning what they say.
`parse` runs `k_best` once and the existing `to_segment` loop per path; k = 3
costs two more assemblies of a sentence that was already parsed.

The struct literal `ParseResult { segments }` appears in a handful of tests
across `engine_v2.rs`, `score.rs`, and `src-tauri/src/parse.rs`; each gains
`alternatives: Vec::new()`. Mechanical, in the same commit.

## 5. Harness: oracle@K

`score` gains `--readings K`, default 1, and passes `ParseOptions::with_readings(K)`
to `parse`. With K = 1 every output byte is unchanged, which is how §1's
invariant is measured.

With K > 1, `accumulate` classifies each gold span against each reading with
the rule it applies today, strict hit or forgiven merge, and ORs across
readings. Per span, not per sentence, because the gate compares against
top-1 span accuracy and a user cycles per word: if the chip they hover is
wrong in reading 1 and right in reading 2, the list delivered. The
whole-gold-parse-in-top-K count is the master plan's literal definition and
is printed beside it, so a reader can see how often cycling once fixes the
whole sentence rather than one word.

Three added lines, additive after `coarsening:`, printed only when K > 1:

```
oracle@3 strict:    0.xxxx  (n / 2163)
oracle@3 tolerant:  0.xxxx  (n / 2163)
oracle@3 sentences: 0.xxxx  (n / 150)
```

JSON gains one block, `"oracle":{"readings":K,"span_hits":n,
"tolerant_hits":n,"sentence_hits":n}`, absent at K = 1, so the recorded
baselines stay byte-identical too.

## 6. Application and UI (commit 4 only)

`src-tauri/src/parse.rs`'s worker passes `ParseOptions::with_readings(3)`.
Nothing else in `src-tauri` changes: the `parse-result` event already
carries the whole `ParseResult`, and `history` stores it, so alternates ride
along into back and forward for free.

`src/types.ts` adds `alternatives: Segment[][]`. `src/main.ts`:

- `readings(result)` is `[result.segments, ...result.alternatives]`. One new
  piece of state, `readingIndex`, reset to 0 when `display` receives a
  result that is not `lastResult`. A furigana-mode re-render passes
  `lastResult` and keeps the index.
- `display` renders `readings(result)[readingIndex]` through `renderSentence`
  unchanged. `segmentAt` searches that same array, so hover, dwell, popover
  and clip region need no change: they never knew which reading they were
  looking at.
- Right-click menu: when `alternatives.length > 0`, `chromeMenuEntries` is
  preceded by radio rows `Reading 1 of 3` … `Reading 3 of 3` and a
  separator. With no alternates, no rows; the menu is today's.
- Keys: `]` next, `[` previous, wrapping, in the existing `keydown` handler
  beside Escape. Cycling closes the popover, as `display` already does.
  Chosen because the window has no text input, both keys exist on JIS and
  US layouts, and neither scrolls `.panes`.

## 7. Staging

Four commits, in this order. The gate sits between the third and the fourth.

1. **`lattice::k_best`.** The list, the key, the backtrack, `best_path` as
   the k = 1 wrapper, unit tests (§9). Eight cells re-run.
2. **`ParseOptions::readings`, `ParseResult.alternatives`.** Default 1, so
   nothing downstream moves. Wire-shape test extended. Eight cells re-run.
3. **`score --readings`.** Oracle lines, unit and CLI tests. Eight cells
   re-run at the default; oracle@3 measured on both hint states. **The gate
   decision is written into the Phase 2 handoff here, before commit 4 is
   started.**
4. **App and UI.** Only on pass.

On fail, commits 1–3 stay: zero behaviour change, tested, and `score
--readings` is the instrument the next attempt would need. Commit 4 does not
land and the wire field stays empty. That is master plan §7's "Phase 2 ships
nothing", with the measurement kept.

## 8. Done when

All measured hints on, gold corpus, K = 3, per §5's per-span union:

1. **oracle@3 tolerant exceeds top-1 tolerant by at least 55 spans**, 2.5
   points of 2,163 rounded up: at least 1,898 of 2,163 (0.8775) against
   today's 1,843 (0.8521). The tolerant ceiling is 1,951, so the bar is half
   the reachable headroom. Below half, a cost-model pass plausibly recovers
   as much without a UI; above it, three readings earn the control.
   Tolerant rather than strict because strict would credit re-splitting the
   compounds §2.2 shows make up 92 of the 196 reachable spans.
2. **Strict oracle@3, the hints-off pair, and the sentence-level count** are
   printed beside it. No threshold: hints off is not the shipped
   configuration, and the sentence count is context, not the metric.
3. **The eight cells are byte-identical** to Phase 1b-i handoff §2's table at
   the default `--readings 1`, after every commit.
4. **Standing gates**: `cargo test -p jparser`, the same with `--features
   mecab`, `cargo clippy -p jparser --all-targets -- -D warnings` in both
   feature states, `cargo check -p jparser --no-default-features`, `npm test`.
5. **Commit 4 lands only if 1 holds.** Its own gate is §9's Vitest rows.

## 9. Tests

- **`lattice`**: every existing `best_path` test passes through the wrapper
  unchanged; k = 3 returns readings distinct by span sequence; two entries at
  one span return one reading; an unmatched run yields no fragment variant;
  a reading that only un-matches a cheaper reading's word is dropped while
  one that leaves an unmatchable character unmatched is kept; a forbidden
  candidate appears in no reading; output is cost-ordered, at most k long,
  and each path is a contiguous cover.
- **`engine_v2`**: a sentence with a known second segmentation
  (`くるまではこう`, which `engine_v2.rs` already gates at k = 1) yields it as
  `alternatives[0]`; every alternate is a full cover; `ParseOptions::default()`
  yields empty `alternatives`. The wire-shape test asserts `alternatives`
  serializes as an array of segment arrays.
- **`score` unit**: the union credits a span reading 2 gets right and
  reading 1 misses; the tolerant union credits a merge; K = 1 output is
  byte-identical to today's.
- **`cli_score`**: the frozen first five lines unchanged; `--readings 3`
  adds exactly three lines.
- **Vitest**: menu rows appear only with alternates and are absent without;
  `]` and `[` cycle and wrap; `segmentAt` resolves against the displayed
  reading; a new result resets the index; a furigana re-render keeps it.

## 10. Budget

`lattice.rs` +90, `engine/mod.rs` and `lib.rs` +40, `score.rs` +70,
`main.ts` +50, `types.ts` +1, tests about 200. No new files under `src/`, no
new dependency. `lattice.rs` stays under 450 lines.

## 11. Master plan edits this design owes

- **§7 Phase 2 "In".** "a whole-sentence reading list with scores" becomes
  "a whole-sentence reading list in cost order"; scores do not go on the
  wire (§1).
- **§7 Phase 2 "Done when".** "oracle@k=3 exceeds top-1 span accuracy by a
  margin that justifies the UI" becomes the §8 clause: tolerant, hints on,
  at least 55 spans.
- **§4.1.** Record the route taken: own k-best over the Phase 1 DP, because
  §6.1 of the Phase 1a design left no tokenizer lattice in the engine.
- **§4.3.** The "escape hatch" sentence gains its measured limit: hints on,
  63 gold spans are suppressed by `is_allowed` itself and no reading over
  the constrained lattice reaches them (§2.3).
- **§5.2.** Add the oracle@∞ ceiling table (§2.2) beside oracle@k's
  definition, and the per-span union as oracle@k's operational form (§5).

## 12. Decision log

| Decision | Rationale |
| --- | --- |
| Measure the oracle@∞ ceiling before building the DP | oracle@k needs the k-best DP; the ceiling needs none and bounds it for every k. If the ceiling were small, Phase 2 would ship nothing without a line of engine code (§2) |
| Hold fallback at top-1 in the ceiling | An unmatched run covers any span, so the literal k = ∞ span ceiling is 1.0. Counting a null span reachable only when top-1 already reproduces it keeps the ceiling beside the eight cells (§2.1) |
| Alternates from the constrained lattice only | One DP, one oracle number, k = 1 provably unchanged. The 63 `is_allowed`-suppressed spans are the constraint's price, the judgment Phase 1a §7.5 made; unconstrained-with-penalty reopens that clause and appending the hints-off best adds a second lattice to the UI (§2.3) |
| Own k-best, not Vibrato N-best or MeCab | Hints supply boundaries only (Phase 1a §6.1), so there is no tokenizer lattice in the engine to enumerate; a tokenizer's N-best is morphemes over IPADIC, not JMdict segmentations (master plan §4.1, §2.3 here) |
| Bounded list per position, not backward-Viterbi plus a priority queue | Both are exact and about the same size; the list is what `lattice.rs`'s module doc promised and changes one function's body plus the backtrack. Master plan §10 already rejected Eppstein (§3) |
| Reading identity is the matched span sequence, hashed | Collapses same-span entry variants, which `to_segment` already gathers, and unmatched fragmentation, which the k = 1 tie-break already hides. Without it k = 3 returns one segmentation three times (§3.2) |
| Drop dominated readings after the DP, over a 4k raw list | A reading that only leaves a cheaper reading's word unmatched is a worse copy, not an alternate, and on short sentences it is what fills readings 2 and 3. Filtering after the backtrack keeps the DP exact and rank 0 untouched; pruning inside the lists is the named upgrade if the fixed factor ever binds (§3.2a) |
| `segments` stays top-1; alternates are an additive field | Every consumer and the wire-shape test keep their meaning; the frontend ignores an unknown field until commit 4 (§4) |
| No scores on the wire | Order is the rank; the `i32` cost has no user meaning. Master plan §7 amended (§1, §11) |
| oracle@k is a per-span union, with the sentence count beside it | The gate compares against top-1 span accuracy, a per-span metric, and a user cycles per word. The master plan's literal whole-parse definition is printed as context (§5) |
| Gate on tolerant, +55 spans, hints on | 92 of the 196 reachable strict spans are compounds the corpus splits and the tolerant metric forgives; strict would reward re-splitting them. Half the tolerant headroom is the line below which a k = 1 cost-model pass plausibly does as well without a UI. Hints on is the shipped configuration (§8) |
| Right-click radio rows plus `[` `]`, not an inline indicator or per-region alternates | Reuses `MenuEntry` and `renderSentence` unchanged and adds no element to the sentence flow; per-region alternates are a second selection model and stay out until the first has earned its place (§6) |
| Commit 4 only after the gate; commits 1–3 stay on fail | The measurement is the phase's product either way; "ships nothing" means no UI and an empty wire field, not a reverted instrument (§7) |
