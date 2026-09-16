# Engine v2 Phase 2b — reading identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `lattice::k_best` sink readings that differ from an already-kept one only by a merge or split, re-measure Phase 2's unchanged gate (oracle@3 tolerant, hints on, at least 1,898 of 2,163) through the shipped path, and on pass land the reading-cycling UI Phase 2 withheld.

**Architecture:** `k_best` today ends `undominated(raw, k)`, which filters dominated readings and cuts to k in one loop. Phase 2b calls `undominated` with the raw pool's full bound instead, then a new `demote` reorders that pool — readings whose cut set is contained in an earlier fresh reading's, or contains it, sink below every reading that is not so related — and cuts to k. Rank 0 is the first reading the sweep sees, so it never moves and k = 1 is the identity. If the pool the shipped k = 3 already holds is too shallow for the reorder to clear the gate, a `DEMOTE_POOL` constant widens the DP's pool, with a latency clause. The UI is Phase 2 design §6 as written, plus a scroll-position guard in `display`.

**Tech Stack:** Rust 2021 (`jparser` crate: serde, clap 4, thiserror; `insta` in tests), Tauri 2 backend in `src-tauri`, TypeScript + Vite frontend tested with Vitest under happy-dom.

**Spec:** `docs/superpowers/specs/2026-09-09-engine-v2-phase2b-design.md` — read it first; this plan argues from it. Its §2 summarises the off-engine measurement in `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md` §7.

## Global Constraints

- **Top-1 is byte-identical at every commit.** After each task's code lands, `jparser-cli score` at the default `--readings 1` must print exactly Phase 1b-i handoff §2's eight cells: hints off `0.7272 (1573/2163)` strict, `0.7975 (1725/2163)` tolerant, `0.9075 (1108/1221)` entry top-1, `75` merges; hints on `0.8031 (1737/2163)`, `0.8521 (1843/2163)`, `0.9063 (1170/1291)`, `53` merges. The commands are in "Measuring" below.
- **The gate is Phase 2 design §8 clause 1, unchanged:** hints on, `--readings 3`, `oracle@3 tolerant:` numerator at least **1,898** of 2,163, i.e. at least 55 over top-1's 1,843. It is not renegotiated because the lever changed (spec §1).
- **Standing gates**, run before every commit that touches `crates/jparser`: `cargo test -p jparser`, `cargo test -p jparser --features mecab`, `cargo clippy -p jparser --all-targets -- -D warnings`, `cargo clippy -p jparser --all-targets --features mecab -- -D warnings`, `cargo check -p jparser --no-default-features`, `cargo fmt -p jparser -- --check`. Before a commit that touches `src/`: `npm test` and `npx tsc --noEmit`. Before a commit that touches `src-tauri/`: `cargo test --manifest-path src-tauri/Cargo.toml`.
- **k is 3 readings total**, top-1 included. No setting, no constant exposed past `src-tauri/src/parse.rs`.
- **No scores on the wire.** `alternatives` stays `Vec<Vec<Segment>>`; only its order changes, and its doc comment says so (spec §4).
- **Nothing in `cost.rs`, `is_allowed`, the lexicon, the index format or the wire shape changes.** `k_best`'s signature does not change; `parse` and `score` are not edited.
- **Task 2 runs only on a Task 1 shortfall; Task 3 lands only if the gate holds** after Task 1 or Task 2. On fail, Task 1 still lands (a better rank order at zero cost, wire field still empty) and Task 4 records the number.
- **Every new Rust file-level comment states provenance** the way `lattice.rs` lines 1–5 do. This plan creates no new source files.
- **Docs and commit messages are written in normal prose**, not the session's terse style.

### Measuring

The gold index on disk under `corpus_staging/gold_index` is format 3 and no longer opens (`VersionMismatch { found: 3, expected: 4 }`). Build a fresh one once per session and reuse it:

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase2b"   # any scratch dir outside the tree
mkdir -p "$S"
./target/release/jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$S/gold_index"
export DIC="$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"    # 47,788,814 bytes
```

The eight-cells check, run after every task's code:

```bash
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --hints "$DIC"
```

Expected, exactly:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7272  (1573 / 2163)
entry top-1:    0.9075  (1108 / 1221)
  by decision:  e 0.7272 (1573 / 2163)
span tolerant:  0.7975  (1725 / 2163)
  coarsening:   152 gold spans inside 75 dictionary merges
```

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8031  (1737 / 2163)
entry top-1:    0.9063  (1170 / 1291)
  by decision:  e 0.8031 (1737 / 2163)
span tolerant:  0.8521  (1843 / 2163)
  coarsening:   106 gold spans inside 53 dictionary merges
```

The gate measurement, after Task 1 (and again after Task 2 if it runs):

```bash
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3 --hints "$DIC"
```

Each prints the eight cells' lines unchanged, then three lines `oracle@3 strict:`, `oracle@3 tolerant:`, `oracle@3 sentences:`. Phase 2's recorded values at `--readings 3`, before this phase, are the *before*: hints off `0.7531 (1629 / 2163)` / `0.8086 (1749 / 2163)` / `0.0067 (1 / 150)`; hints on `0.8534 (1846 / 2163)` / `0.8706 (1883 / 2163)` / `0.1667 (25 / 150)`.

Rebuild the release binary (`cargo build --release ...` above) before re-running; `score` reads the binary, not the source.

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `crates/jparser/src/engine/lattice.rs` | `use std::collections::BTreeSet`; new `cuts`, `demote`; `undominated`'s parameter renamed `bound` and its doc adjusted; `k_best`'s tail becomes `demote(undominated(raw, cap), k)`; two `k_best` doc paragraphs amended; five unit tests | 1 |
| `crates/jparser/src/lib.rs:71-75` | `ParseResult::alternatives` doc: rank is no longer cost order alone | 1 |
| `crates/jparser/tests/engine_v2.rs` | comment on the existing `東京都` test; new `TOSHI_FIXTURE`, `parse_toshi`, one test | 1 |
| `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md` | created with §1–§3 at Task 1; completed at Task 4 | 1, 4 |
| `crates/jparser/src/engine/lattice.rs` | `DEMOTE_POOL` and the `cap` expression — **only on a Task 1 shortfall** | 2 |
| `src-tauri/src/parse.rs:111` | `ParseOptions::with_readings(READINGS)` | 3 |
| `src/types.ts:41-43` | `ParseResult.alternatives` | 3 |
| `src/main.ts` | `readingIndex`, `readings`, `displayedSegments`, `showReading`, `readingMenuEntries`; `segmentAt`, `display`, the `contextmenu` and `keydown` listeners | 3 |
| `src/main.test.ts` | nine tests appended inside the first `describe` | 3 |
| `src/history.test.ts:6-22`, `src/render/sentence.test.ts:37` | `alternatives: []` on typed `ParseResult` literals | 3 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | §6 phase order line; §7 Phase 2 outcome paragraph | 4 |

---

### Task 1: `demote` in `lattice::k_best`, over the pool k = 3 already holds

**Files:**
- Modify: `crates/jparser/src/engine/lattice.rs` — imports at 17–19; `undominated` at 203–231; `k_best` doc at 233–263 and tail at 322–326; tests module (ends at 648)
- Modify: `crates/jparser/src/lib.rs:71-75`
- Modify: `crates/jparser/tests/engine_v2.rs` — comment at 944–957; new fixture and test after `every_alternative_is_a_contiguous_cover` (ends near line 980)
- Create: `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md`

**Interfaces:**
- Consumes: `undominated(readings: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>>` and `k_best`'s `cap` (both existing), `Step::len` (existing, private).
- Produces: `fn cuts(path: &[Step]) -> BTreeSet<usize>` and `fn demote(pool: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>>`, both private to `lattice`; `k_best`'s public signature is unchanged, its output order is what Task 3 renders and Task 2 may re-measure.

- [ ] **Step 1: Write the failing lattice tests**

In `crates/jparser/src/engine/lattice.rs`, inside `mod tests`, after the test `a_huge_k_neither_panics_nor_overflows` (the last test, ending just before the module's closing `}` at line 648), add:

```rust
    // Phase 2b (design §3): rank is cost order among readings not merge/split
    // equivalent to an earlier kept one, then the equivalent ones in cost
    // order. Two readings are equivalent when one's cut set is contained in
    // the other's, in either direction.

    /// 東京都市 over five candidates: 東京 and 都市 COMMON, 東, 京 and 京都市
    /// plain. Exactly three undominated readings, at distinct costs, so no
    /// assertion below rides on a tie-break: 東京+都市 at (2-4)+(2-4) = -4,
    /// 東+京+都市 at 1+1+(2-4) = 0, 東+京都市 at 1+3 = 4. Every other cover
    /// leaves a word unmatched that one of these three matches, and is
    /// dropped by `undominated` before `demote` sees it.
    fn tokyo_toshi() -> (Vec<char>, Vec<Vec<Candidate>>) {
        let mut common = WordFlags::PRIMARY;
        common.insert(WordFlags::COMMON);
        let chars: Vec<char> = "東京都市".chars().collect();
        let cands = by_start(
            4,
            vec![
                c(0, 2, 1, common),             // 東京
                c(2, 2, 2, common),             // 都市
                c(0, 1, 3, WordFlags::PRIMARY), // 東
                c(1, 1, 4, WordFlags::PRIMARY), // 京
                c(1, 3, 5, WordFlags::PRIMARY), // 京都市
            ],
        );
        (chars, cands)
    }

    #[test]
    fn a_reading_unrelated_to_rank_zero_outranks_a_re_split_of_it() {
        let (chars, cands) = tokyo_toshi();
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // Cost order is 東京+都市, 東+京+都市, 東+京都市. The middle one cuts
        // at {0,1,2,4}, a superset of rank 0's {0,2,4} -- the same reading
        // re-split -- so it sinks. 東+京都市 cuts at {0,1,4}, comparable with
        // rank 0's set in neither direction, so it is fresh and now rank 1.
        //
        // {0,1,4} *is* a subset of the sunk reading's {0,1,2,4}. Were readings
        // compared against sunk ones too, 東+京都市 would sink behind it; it
        // is compared against the fresh list only, and does not. A refactor
        // that compares against every earlier reading fails here.
        assert_eq!(
            spans,
            vec![
                vec![(0, 2), (2, 2)],
                vec![(0, 1), (1, 3)],
                vec![(0, 1), (1, 1), (2, 2)]
            ]
        );
    }

    #[test]
    fn demotion_frees_a_rank_the_cut_would_have_spent() {
        // The point of the phase: at k = 2 the re-split no longer occupies the
        // second slot, so a reading the old cut never returned now does.
        let (chars, cands) = tokyo_toshi();
        let readings = k_best(&chars, &cands, None, 2);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        assert_eq!(spans, vec![vec![(0, 2), (2, 2)], vec![(0, 1), (1, 3)]]);
    }

    #[test]
    fn demotion_never_moves_rank_zero() {
        let (chars, cands) = tokyo_toshi();
        assert_eq!(
            k_best(&chars, &cands, None, 3)[0],
            best_path(&chars, &cands, None)
        );
    }

    #[test]
    fn readings_all_equivalent_to_rank_zero_keep_their_cost_order() {
        // 東京都 as one word, or 東京+都, or 東+京都. Every full cover cuts at
        // a superset of the single word's {0,3}, so every alternative sinks,
        // and sinking together they keep the order they arrived in: demotion
        // reorders between the fresh group and the sunk group, never inside
        // either. 東+京都 and 東京+都 cut at {0,1,3} and {0,2,3}, comparable
        // with neither each other -- the relation is not transitive, and the
        // sweep does not pretend it is.
        let chars: Vec<char> = "東京都".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
                c(0, 1, 4, WordFlags::PRIMARY),
                c(1, 2, 5, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| matched_spans(p)).collect();
        // All three cost 3; the longer last step wins each tie, as at k = 1.
        assert_eq!(
            spans,
            vec![vec![(0, 3)], vec![(0, 1), (1, 2)], vec![(0, 2), (2, 1)]]
        );
    }

    #[test]
    fn cuts_are_zero_and_every_step_end() {
        let path = vec![
            Step::Matched(c(0, 2, 1, WordFlags::PRIMARY)),
            Step::Unmatched { start: 2, len: 1 },
            Step::Matched(c(3, 2, 2, WordFlags::PRIMARY)),
        ];
        assert_eq!(cuts(&path), BTreeSet::from([0, 2, 3, 5]));
        assert_eq!(cuts(&[]), BTreeSet::from([0]));
    }
```

- [ ] **Step 2: Run the new tests to verify they fail**

Run: `cargo test -p jparser --lib lattice::tests -- --nocapture 2>&1 | tail -30`
Expected: compile error, `cannot find function `cuts` in this scope` (and `BTreeSet` not in scope). That is the failure: the function does not exist yet.

- [ ] **Step 3: Add the import, `cuts` and `demote`**

At the top of `crates/jparser/src/engine/lattice.rs`, change the imports (lines 17–19) to:

```rust
use std::collections::BTreeSet;

use crate::engine::cost::{cost, is_allowed, unmatched_cost};
use crate::engine::lex::Candidate;
use crate::engine::Boundaries;
```

Replace the whole of `undominated` (its doc comment from `/// The first `k` of `readings`` through the function's closing `}`, lines 203–231) with:

```rust
/// The first `bound` of `readings` (already in cost order) that no cheaper
/// kept reading dominates. A reading is dominated when another, cheaper one
/// matches every span it matches: it only leaves words unmatched that the
/// other found, a worse copy of that reading rather than a different one.
/// Rank 0 is never dominated. Checking against the kept list alone is
/// enough, because every dominator of a dropped reading is cheaper than it
/// and so was either kept or dominated in turn by something kept.
///
/// The kept list is built in cost order and stops when full, so its first
/// `n` entries are the same for every `bound >= n`: a larger bound extends
/// the list and never disturbs its prefix. `k_best` relies on that to pass
/// the whole raw pool's bound here and cut to `k` only after `demote`.
fn undominated(readings: Vec<Vec<Step>>, bound: usize) -> Vec<Vec<Step>> {
    // No `Vec::with_capacity(bound)`: `bound` derives from a caller-controlled
    // `k` (see `k_best`) and must not let a huge request pre-allocate an
    // absurd amount up front.
    let mut kept: Vec<Vec<Step>> = Vec::new();
    let mut kept_spans: Vec<Vec<(usize, usize)>> = Vec::new();
    for reading in readings {
        if kept.len() == bound {
            break;
        }
        let spans = matched_spans(&reading);
        let dominated = kept_spans
            .iter()
            .any(|other| spans.iter().all(|s| other.contains(s)));
        if dominated {
            continue;
        }
        kept_spans.push(spans);
        kept.push(reading);
    }
    kept
}

/// Where a reading cuts: 0, then the end of every step. A path is a
/// contiguous cover from 0, so the step ends alone determine it.
fn cuts(path: &[Step]) -> BTreeSet<usize> {
    let mut at = 0;
    let mut set = BTreeSet::from([0]);
    for step in path {
        at += step.len();
        set.insert(at);
    }
    set
}

/// `pool`, in cost order, reordered so that a reading merge/split equivalent
/// to an earlier *fresh* reading sinks below every reading that is not, then
/// cut to `k`. Two readings are merge/split equivalent when one's cut set is
/// contained in the other's -- `東京都` at {0,3} against `東京`+`都` at
/// {0,2,3} -- in either direction, because merge and split are one relation
/// read from opposite ends (Phase 2b design §3.1; the asymmetric variant was
/// measured and loses, Phase 2 handoff §7).
///
/// Rank 0 meets an empty fresh list, so it is always fresh and always first:
/// at k = 1 this is the identity, which is what keeps `best_path` and the
/// eight cells untouched by construction. A reading is compared against the
/// fresh list only, never against sunk ones -- otherwise one sunk reading
/// would drag unrelated readings down behind it. Containment in either
/// direction is not transitive, so this is a greedy sweep whose result
/// depends on the order it sweeps; cost order is the only order there is,
/// and rank 0 is the reading the engine chose.
///
/// Nothing is deleted. A sunk reading past `k` is not returned, as any
/// reading past `k` never was; what changes is which readings the cut
/// keeps.
fn demote(pool: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>> {
    // No `Vec::with_capacity(k)`, for `undominated`'s reason.
    let mut fresh: Vec<Vec<Step>> = Vec::new();
    let mut fresh_cuts: Vec<BTreeSet<usize>> = Vec::new();
    let mut sunk: Vec<Vec<Step>> = Vec::new();
    for reading in pool {
        let c = cuts(&reading);
        if fresh_cuts.iter().any(|f| f.is_subset(&c) || c.is_subset(f)) {
            sunk.push(reading);
        } else {
            fresh_cuts.push(c);
            fresh.push(reading);
        }
    }
    fresh.extend(sunk);
    fresh.truncate(k);
    fresh
}
```

- [ ] **Step 4: Rewire `k_best`'s tail and amend its doc**

In `k_best`, replace the tail (lines 322–326):

```rust
    let raw: Vec<Vec<Step>> = (0..best[n].len())
        .map(|rank| path_of(&best, n, rank))
        .collect();
    undominated(raw, k)
}
```

with:

```rust
    let raw: Vec<Vec<Step>> = (0..best[n].len())
        .map(|rank| path_of(&best, n, rank))
        .collect();
    // Filter the whole raw pool, not the first `k`: `demote` can only free a
    // rank if it runs before the list is cut (Phase 2b design §3.2).
    demote(undominated(raw, cap), k)
}
```

In `k_best`'s doc comment, replace the first paragraph (lines 233–238):

```rust
/// The `k` cheapest distinct contiguous covers of `chars`, cheapest first,
/// choosing among `by_start`'s candidates under the boundaries `bounds`
/// allows. Distinct means by the set of spans matched (see `Node::key`), and
/// a reading that only un-matches what a cheaper one matches is not offered
/// (see `undominated`). Always at least one path; empty input yields exactly
/// one empty path.
```

with:

```rust
/// The `k` best distinct contiguous covers of `chars`, choosing among
/// `by_start`'s candidates under the boundaries `bounds` allows. Distinct
/// means by the set of spans matched (see `Node::key`), and a reading that
/// only un-matches what a cheaper one matches is not offered (see
/// `undominated`). Rank 0 is the cheapest; below it the order is cost order
/// among readings not merge/split equivalent to an earlier one, then the
/// equivalent ones in cost order (see `demote`) -- so a re-split of the top
/// reading does not spend a rank a genuinely different reading could hold.
/// Always at least one path; empty input yields exactly one empty path.
```

and replace the last paragraph (lines 258–263):

```rust
/// That ceiling is measured, not merely bounded: at k = 3 the shipped cap of
/// 12 returns byte-identical reading lists to a cap of 96 on all 150
/// gold-corpus sentences in both hint states, because with `CROSS = 0` the
/// interesting alternates are exact-cost ties while the one-word-unmatched
/// variants they compete with cost about +30.
```

with:

```rust
/// That ceiling was measured before `demote`, not merely bounded: at k = 3
/// the shipped cap of 12 returned byte-identical top-three lists to a cap of
/// 96 on all 150 gold-corpus sentences in both hint states, because with
/// `CROSS = 0` the interesting alternates are exact-cost ties while the
/// one-word-unmatched variants they compete with cost about +30. With
/// `demote` the top three depend on the whole pool, not its first three, so
/// that measurement no longer covers them; the Phase 2b handoff §2 records
/// what the pool at this cap reaches.
```

- [ ] **Step 5: Run the lattice tests**

Run: `cargo test -p jparser --lib lattice::tests`
Expected: all 20 tests pass (15 existing, 5 new), including `readings_come_out_distinct_and_in_cost_order` and `a_reading_that_leaves_an_unmatchable_character_unmatched_is_kept` unchanged — in both, every returned reading is equivalent to rank 0 or there are only two, so the order is what it was.

- [ ] **Step 6: The `alternatives` doc and the `engine_v2` tests**

In `crates/jparser/src/lib.rs`, replace the doc comment on `alternatives` (lines 71–74):

```rust
    /// Readings 2..k in cost order, each a contiguous cover like `segments`
    /// and distinct from it and from each other by the spans they match
    /// (Phase 2 design §3, §4). Empty unless the caller asked for more than
    /// one reading through [`ParseOptions::with_readings`].
```

with:

```rust
    /// Readings 2..k, each a contiguous cover like `segments` and distinct
    /// from it and from each other by the spans they match (Phase 2 design
    /// §3, §4). The order is the rank, and rank is not cost order alone:
    /// readings that differ from `segments` or an earlier alternative only
    /// by a merge or split come after every reading that does not, so the
    /// first alternatives are genuinely different analyses rather than
    /// re-splits of the top one (Phase 2b design §3, §4). Empty unless the
    /// caller asked for more than one reading through
    /// [`ParseOptions::with_readings`].
```

In `crates/jparser/tests/engine_v2.rs`, replace the test `three_readings_come_out_in_cost_order_with_dominated_ones_dropped` (lines 944–957) with:

```rust
#[test]
fn three_readings_come_out_in_cost_order_with_dominated_ones_dropped() {
    let r = parse_tokyo("tokyo-three", 3);
    assert_eq!(surfaces(&r.segments), [("東京都", true)]);
    assert_eq!(r.alternatives.len(), 2);
    // Phase 2b's demotion leaves this order alone, and not by luck: top-1 is
    // the single word 東京都, so every full cover cuts at a superset of its
    // {0,3} and every alternative sinks together, keeping the cost order they
    // arrived in. Demotion reorders between fresh and sunk readings, never
    // within a group; `a_reading_unrelated_to_the_top_one_outranks_a_re_split_of_it`
    // below is the case where the groups differ.
    assert_eq!(surfaces(&r.alternatives[0]), [("東京", true), ("都", true)]);
    // 東 has no entry here, so leaving it unmatched is the only way to read
    // 京都. 東京 + unmatched 都 is absent: 東京 + 都 is cheaper and matches
    // everything it does.
    assert_eq!(
        surfaces(&r.alternatives[1]),
        [("東", false), ("京都", true)]
    );
}
```

After the test `every_alternative_is_a_contiguous_cover` (its closing `}` is near line 980, before the doc comment `/// Global constraint: asking for more readings never moves the first one.`), add:

```rust
// Phase 2b: a private five-entry fixture on which 東京都市 has exactly three
// undominated readings at distinct costs -- 東京+都市 (both COMMON through
// `news1`), 東+京+都市, 東+京都市 -- so `demote`'s reorder is visible through
// `parse` with no tie-break in play. Private for the reason `KURU_FIXTURE` is.
const TOSHI_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry><ent_seq>1000200</ent_seq><k_ele><keb>東京</keb><ke_pri>news1</ke_pri></k_ele><r_ele><reb>とうきょう</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo</gloss></sense></entry>
<entry><ent_seq>1000210</ent_seq><k_ele><keb>都市</keb><ke_pri>news1</ke_pri></k_ele><r_ele><reb>とし</reb></r_ele><sense><pos>&n;</pos><gloss>city</gloss></sense></entry>
<entry><ent_seq>1000220</ent_seq><k_ele><keb>東</keb></k_ele><r_ele><reb>ひがし</reb></r_ele><sense><pos>&n;</pos><gloss>east</gloss></sense></entry>
<entry><ent_seq>1000230</ent_seq><k_ele><keb>京</keb></k_ele><r_ele><reb>きょう</reb></r_ele><sense><pos>&n;</pos><gloss>capital</gloss></sense></entry>
<entry><ent_seq>1000240</ent_seq><k_ele><keb>京都市</keb></k_ele><r_ele><reb>きょうとし</reb></r_ele><sense><pos>&n;</pos><gloss>Kyoto City</gloss></sense></entry>
</JMdict>
"#;

fn parse_toshi(name: &str, readings: usize) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(TOSHI_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    jparser::parse(
        &index,
        &table,
        "東京都市",
        &jparser::ParseOptions::with_readings(readings),
        None,
    )
    .expect("parse")
}

/// Phase 2b design §3: a reading that is not a merge or split of the top one
/// outranks one that is, even when the re-split is cheaper.
#[test]
fn a_reading_unrelated_to_the_top_one_outranks_a_re_split_of_it() {
    let r = parse_toshi("toshi-three", 3);
    assert_eq!(surfaces(&r.segments), [("東京", true), ("都市", true)]);
    assert_eq!(r.alternatives.len(), 2);
    // By cost, 東+京+都市 (0) precedes 東+京都市 (4). The first cuts at
    // {0,1,2,4}, a superset of top-1's {0,2,4} -- the same reading
    // re-split -- and sinks; the second cuts at {0,1,4}, comparable with
    // neither, and is the first alternative a reader is offered.
    assert_eq!(
        surfaces(&r.alternatives[0]),
        [("東", true), ("京都市", true)]
    );
    assert_eq!(
        surfaces(&r.alternatives[1]),
        [("東", true), ("京", true), ("都市", true)]
    );
}
```

- [ ] **Step 7: Run every standing gate**

Run, in order:

```bash
cargo fmt -p jparser
cargo test -p jparser
cargo test -p jparser --features mecab
cargo clippy -p jparser --all-targets -- -D warnings
cargo clippy -p jparser --all-targets --features mecab -- -D warnings
cargo check -p jparser --no-default-features
cargo fmt -p jparser -- --check
```

Expected: `cargo test -p jparser` reports 289 passed (283 + 5 lattice + 1 engine_v2); with `--features mecab`, 315 passed and 1 ignored; every clippy, check and fmt invocation clean. If `cargo fmt` reflowed the long `assert_eq!` vectors in the new tests, that is fine — commit the reflowed form.

- [ ] **Step 8: Eight cells**

Run the two `score` commands from "Measuring" at the default `--readings 1` (rebuild the release binary first). Expected: byte-identical to the two blocks shown there. If any cell differs, stop: `demote` is not the identity at k = 1 and the tail is wrong — do not proceed to the gate.

- [ ] **Step 9: Commit the code**

```bash
git add crates/jparser/src/engine/lattice.rs crates/jparser/src/lib.rs crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine-v2): demote merge/split re-splits below genuinely different readings

k_best now filters the whole raw pool for dominated readings and then
reorders it before the cut to k: a reading whose cut set is contained in
an earlier fresh reading's, or contains it, sinks below every reading that
is not so related. Rank 0 meets an empty fresh list and so never moves,
which keeps best_path and the eight cells untouched by construction.

Nothing is deleted and no signature changes. What changes is which
readings the cut to k keeps: a re-split of the top reading no longer
spends a rank a genuinely different reading could hold. The relation is
symmetric because the asymmetric variant was measured and loses (Phase 2
handoff §7); the reorder runs over the pool k = 3 already holds, so the DP
is not widened."
```

- [ ] **Step 10: Measure the gate through the shipped path**

Rebuild and run the two `--readings 3` commands from "Measuring". Copy both outputs verbatim. From the hints-on output take the `oracle@3 tolerant:` numerator `T`. Compute `T − 1843`. The gate holds if `T ≥ 1898`.

Also record, from the hints-on output, the `oracle@3 strict:` numerator and the `oracle@3 sentences:` numerator (Phase 2's were 1846 and 25), and from the hints-off output all three oracle numerators (Phase 2's: 1629, 1749, 1).

- [ ] **Step 11: Create the handoff with the measurement**

Create `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md` with this content, the two `score` outputs pasted verbatim into the fenced blocks, and every `T`, `D`, `S`, `N` replaced by the numbers from Step 10:

```markdown
# Engine v2 Phase 2b — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2b-design.md`](specs/2026-09-09-engine-v2-phase2b-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2b.md`](plans/2026-09-09-engine-v2-phase2b.md)
**Branch:** `feat/engine-v2`

## 1. What landed before the gate

One commit, re-measured at `--readings 1` against Phase 1b-i handoff §2's
eight cells and byte-identical to them: `lattice::demote`, the reorder
between `undominated` and the cut to k (design §3.2), over the raw pool the
shipped k = 3 already holds — cap 12, no DP widening. `k_best`'s signature,
`parse`, `score` and the wire shape are unchanged; `ParseResult::alternatives`'s
doc comment now says rank is not cost order alone.

## 2. The gate measurement, over the existing pool

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`;
Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

Hints off:

```
(paste the hints-off output here)
```

Hints on:

```
(paste the hints-on output here)
```

Phase 2 recorded, at the same `--readings 3` before this phase: hints on
oracle@3 strict 1846, tolerant 1883, sentences 25; hints off 1629, 1749, 1.

## 3. Verdict

Design §8 clause 1, which is Phase 2 design §8 clause 1 unchanged: oracle@3
tolerant hints on must be at least 1,898 of 2,163, 55 spans over top-1's
1,843. Measured over the pool k = 3 already holds: **T of 2,163, T − 1,843 =
D spans.** The gate **(holds | does not hold)** at this pool.

Strict oracle@3 hints on: S of 2,163 (Phase 2: 1,846). Whole-sentence gold
cover in the top three: N of 150 (Phase 2: 25). Neither is thresholded
(design §8 clause 3); both are recorded here rather than averaged away, and
the sentence count is the one the off-engine spike saw regress, 25 → 24.
```

If the gate does not hold, keep the wording "does not hold at this pool" — Task 2 appends §2b and a second verdict; do not write "fails" yet.

- [ ] **Step 12: Commit the handoff**

```bash
git add docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md
git commit -m "docs(engine-v2): record the Phase 2b gate measurement over the existing pool"
```

---

### Task 2: Gate check, and `DEMOTE_POOL` only on a shortfall

**Files:**
- Modify (only on a shortfall): `crates/jparser/src/engine/lattice.rs` — after `RAW_FACTOR` (line 45); `cap` in `k_best` (line 283)
- Modify (only on a shortfall): `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md`

**Interfaces:**
- Consumes: `k_best`'s `cap` expression and `RAW_FACTOR` (existing).
- Produces: `const DEMOTE_POOL: usize`, private to `lattice`, and a wider raw pool at every k > 1. Nothing else reads it.

- [ ] **Step 1: Read the verdict in the handoff's §3**

If `T ≥ 1898`: this task is done. Skip every step below and proceed to Task 3.

If `T < 1898`: continue.

- [ ] **Step 2: Keep the Task 1 binary for the latency comparison**

```bash
cp ./target/release/jparser-cli.exe "$S/jparser-cli-pool-k.exe"
```

- [ ] **Step 3: Write the failing test**

In `crates/jparser/src/engine/lattice.rs`, inside `mod tests`, after `cuts_are_zero_and_every_step_end`, add:

```rust
    #[test]
    fn the_pool_is_at_least_demote_pool_wide_above_k_one() {
        // `DEMOTE_POOL` exists only because the pool `k` alone provides was
        // measured too shallow for `demote` to pay (Phase 2b handoff §2b).
        // The constant is a floor on the pool, not a change to `k`: the list
        // is still cut to `k`, and k = 1 still runs a single slot.
        assert!(DEMOTE_POOL >= 3, "the floor must not be below the shipped k");
        assert_eq!(pool_cap(1), 1);
        assert_eq!(pool_cap(3), DEMOTE_POOL * RAW_FACTOR);
        assert_eq!(pool_cap(DEMOTE_POOL + 1), (DEMOTE_POOL + 1) * RAW_FACTOR);
    }
```

- [ ] **Step 4: Run it to verify it fails**

Run: `cargo test -p jparser --lib lattice::tests::the_pool_is_at_least_demote_pool_wide_above_k_one`
Expected: compile error, `cannot find value `DEMOTE_POOL`` and `cannot find function `pool_cap``.

- [ ] **Step 5: Add `DEMOTE_POOL` and `pool_cap`**

After the `RAW_FACTOR` constant (line 45) add:

```rust
/// Readings the raw pool is sized for when the caller asks for fewer. `demote`
/// reorders the pool before the cut to `k`, and the Phase 2b handoff §2
/// measured the pool `k = 3` alone provides as too shallow for that reorder
/// to clear the gate. The off-engine sweep (Phase 2 handoff §7) cleared it
/// from reading depth 12 (+65 of the +55 required) and not from 6 (+48), so
/// 12 is the shallowest floor with a measurement behind it. Handoff §2b
/// records what it costs in wall time.
const DEMOTE_POOL: usize = 12;

/// Raw readings the DP keeps per position for `k` requested readings: one at
/// k = 1, where `demote` is the identity and a single slot is the pre-Phase-2
/// DP exactly; otherwise `RAW_FACTOR` times the larger of `k` and
/// `DEMOTE_POOL`.
fn pool_cap(k: usize) -> usize {
    if k == 1 {
        1
    } else {
        k.max(DEMOTE_POOL) * RAW_FACTOR
    }
}
```

In `k_best`, replace (line 283):

```rust
    let cap = if k == 1 { 1 } else { k * RAW_FACTOR };
```

with:

```rust
    let cap = pool_cap(k);
```

and in the comment block just above it, change the sentence `The `RAW_FACTOR` over-provision only earns its keep once there is more than one rank to protect from domination.` to `The `RAW_FACTOR` over-provision, and the `DEMOTE_POOL` floor beneath it, only earn their keep once there is more than one rank to protect from domination and reorder.`

In `crates/jparser/src/lib.rs`, the `MAX_READINGS` doc (lines 27–29) says the DP list is `proportional to `readings` (`engine::lattice::RAW_FACTOR`)`; change that parenthetical to `(`engine::lattice::RAW_FACTOR`, with a floor of `engine::lattice::DEMOTE_POOL` readings)`.

- [ ] **Step 6: Standing gates and eight cells**

Run the seven commands from Task 1 Step 7. Expected: 290 / 316 passed, everything else clean. Then rebuild the release binary and run the eight-cells check. Expected: byte-identical — `pool_cap(1)` is 1, so k = 1 is untouched.

- [ ] **Step 7: Re-measure the gate**

Run the two `--readings 3` commands from "Measuring" and take `T` again. The gate holds if `T ≥ 1898`.

- [ ] **Step 8: Measure latency**

In bash, five timed runs per binary per hint state, discarding the first (cold cache) and taking the median of the remaining four:

```bash
TIMEFORMAT=%R
for bin in "$S/jparser-cli-pool-k.exe" ./target/release/jparser-cli.exe; do
  echo "== $bin, hints off"
  for i in 1 2 3 4 5; do time "$bin" score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3 > /dev/null; done
  echo "== $bin, hints on"
  for i in 1 2 3 4 5; do time "$bin" score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3 --hints "$DIC" > /dev/null; done
done
```

Design §8 clause 4: the new binary's median must be within 10% of the Task 1 binary's, in both hint states. The hints-on figure includes loading the 46 MB Vibrato dictionary in both binaries, so the hints-off pair is the sharper comparison; both must pass.

- [ ] **Step 9: Record §2b in the handoff**

In `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md`, after §2's hints-on block and before `## 3. Verdict`, add:

```markdown
## 2b. The gate measurement, with the pool floored at `DEMOTE_POOL` = 12

Hints off:

```
(paste the hints-off output here)
```

Hints on:

```
(paste the hints-on output here)
```

Wall time, median of four warm runs, seconds — pool k (cap 12) against pool
12 (cap 48):

| | hints off | hints on |
| --- | --- | --- |
| Task 1 binary | X.XX | X.XX |
| Task 2 binary | X.XX | X.XX |
| change | +X% | +X% |
```

and rewrite §3 to give both verdicts: the pool-k figure from Task 1 and the pool-12 figure here, each against the same 1,898, and whether the latency clause held.

- [ ] **Step 10: Commit**

```bash
git add crates/jparser/src/engine/lattice.rs crates/jparser/src/lib.rs docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md
git commit -m "feat(engine-v2): floor the raw pool at DEMOTE_POOL readings so demotion has room to reorder

The pool k = 3 alone provides was measured too shallow for demote to clear
the gate (Phase 2b handoff §2). The pool is now sized for the larger of k
and 12 readings above k = 1, which the off-engine sweep found to be the
shallowest depth that clears it. k = 1 still runs a single slot, so the
eight cells are untouched; the wall-time cost is recorded in handoff §2b."
```

If the gate still does not hold, or the latency clause fails, the pool change still lands only if the gate holds *and* latency is within 10%; otherwise revert Step 5's code change (`git checkout -- crates/jparser/src/engine/lattice.rs crates/jparser/src/lib.rs`), keep the handoff §2b as the record, commit the handoff alone, and skip Task 3.

---

### Task 3: Application and UI (only if the gate holds)

**Files:**
- Modify: `src-tauri/src/parse.rs:111`
- Modify: `src/types.ts:41-43`
- Modify: `src/main.ts` — the `contextmenu` listener (486–489), new declarations after `lastResult` (698), `segmentAt` (703–706), the `keydown` listener (1053–1057), `display` (1193–1209)
- Modify: `src/main.test.ts` — nine tests appended inside the first `describe`, before its closing `});` at line 172
- Modify: `src/history.test.ts:6-22`, `src/render/sentence.test.ts:37`

**Interfaces:**
- Consumes: `ParseResult.alternatives` on the `parse-result` event (Phase 2 Task 2, already shipped; Task 1 fixed its order); `MenuEntry` (`{ kind: 'radio'; label; checked; onSelect } | { kind: 'action'; label; onSelect } | { kind: 'separator' }`) and `openContextMenu(entries, x, y)` from `src/context-menu.ts` unchanged; `renderSentence(result: ParseResult, mode)` from `src/render/sentence.ts` unchanged; `panes` (`app.querySelector('.panes')`, line 86) and `lastResult` in `main.ts`.
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing frontend tests**

In `src/main.test.ts`, inside `describe('the event-driven render path', ...)`, after the test `a parse-error event leaves the previous result on screen` (ends at line 171) and before the describe's closing `});` (line 172), add:

```ts
  // Phase 2 design §6, landed by Phase 2b: readings 2..k ride on the same
  // event as `alternatives`, in the order Phase 2b's `demote` gives them.
  const readingSeg = (start: number, surface: string, matched = true) => ({
    start,
    len: surface.length,
    surface,
    reading: null,
    matched,
    entries: matched
      ? [
          {
            headword: surface,
            reading: null,
            conjugation: null,
            pos: ['n'],
            senses: [{ pos: ['n'], glosses: [surface], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          },
        ]
      : [],
  });
  /** Every rendered segment in order, chip or unmatched run. */
  const rendered = () => [...document.querySelectorAll('.sentence > *')].map((el) => el.textContent);
  const menuLabels = () => {
    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    return [...document.querySelectorAll('.context-menu button')].map((b) => b.textContent);
  };
  const press = (key: string, init: KeyboardEventInit = {}) =>
    document.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...init }));
  const panes = () => document.querySelector<HTMLElement>('.panes')!;
  const tokyo = () => ({
    segments: [readingSeg(0, '東京都')],
    alternatives: [
      [readingSeg(0, '東京'), readingSeg(2, '都')],
      [readingSeg(0, '東', false), readingSeg(1, '京都')],
    ],
  });

  test('alternates appear as reading rows and a row switches the sentence', () => {
    emit('parse-result', tokyo());
    expect(rendered()).toEqual(['東京都']);

    expect(menuLabels().slice(0, 3)).toEqual(['Reading 1 of 3', 'Reading 2 of 3', 'Reading 3 of 3']);
    [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
      .find((b) => b.textContent === 'Reading 3 of 3')!
      .click();

    expect(rendered()).toEqual(['東', '京都']);
    expect(document.querySelector('.unmatched')?.textContent).toBe('東');
    // Reopened, the menu marks the reading now on screen.
    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const checked = [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
      .filter((b) => b.getAttribute('aria-checked') === 'true')
      .map((b) => b.textContent);
    expect(checked).toContain('Reading 3 of 3');
  });

  test('a result with no alternates offers no reading rows', () => {
    emit('parse-result', { segments: [readingSeg(0, '東京都')], alternatives: [] });
    expect(menuLabels().some((label) => label?.startsWith('Reading'))).toBe(false);
  });

  test('] and [ cycle the readings and wrap at both ends', () => {
    emit('parse-result', tokyo());
    press(']');
    expect(rendered()).toEqual(['東京', '都']);
    press(']');
    press(']');
    expect(rendered()).toEqual(['東京都']);
    press('[');
    expect(rendered()).toEqual(['東', '京都']);
  });

  test('a modified bracket is not a reading step', () => {
    // Alt+] and Alt+[ are history navigation, handled on the Rust side; the
    // webview still sees the keydown and must not also cycle readings.
    emit('parse-result', tokyo());
    press(']', { altKey: true });
    press('[', { ctrlKey: true });
    expect(rendered()).toEqual(['東京都']);
  });

  test('a new result starts at reading 1 again', () => {
    emit('parse-result', tokyo());
    press(']');
    emit('parse-result', {
      segments: [readingSeg(0, '東京'), readingSeg(2, '都')],
      alternatives: [[readingSeg(0, '東京都')]],
    });
    expect(rendered()).toEqual(['東京', '都']);
  });

  test('the popover reads the displayed reading, not the top one', () => {
    emit('parse-result', tokyo());
    press(']');
    document
      .querySelector<HTMLElement>('.chip[data-start="2"]')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));

    const content = emitted.find(([event]) => event === 'popover-content');
    expect(content).toBeDefined();
    const payload = content![1] as { entries: { headword: string }[] };
    expect(payload.entries[0].headword).toBe('都');
  });

  test('a furigana change re-renders the reading on screen, not reading 1', () => {
    emit('parse-result', tokyo());
    press(']');
    emit('settings-changed', { furigana_mode: 'hiragana' });
    expect(rendered()).toEqual(['東京', '都']);
  });

  test('cycling keeps the scroll position; a new result returns to the top', () => {
    // Phase 2b design §6: a reading switch is made while reading one word,
    // and must not lose the reader's place. A different result is a new
    // sentence and starts at the top, as every render did before.
    emit('parse-result', tokyo());
    panes().scrollTop = 120;
    press(']');
    expect(panes().scrollTop).toBe(120);
    emit('parse-result', { segments: [readingSeg(0, '東京')], alternatives: [] });
    expect(panes().scrollTop).toBe(0);
  });

  test('a furigana change keeps the scroll position too', () => {
    emit('parse-result', tokyo());
    panes().scrollTop = 120;
    emit('settings-changed', { furigana_mode: 'hiragana' });
    expect(panes().scrollTop).toBe(120);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts -t "reading|bracket|scroll"`
Expected: eight of the nine fail — no `Reading` rows, `]` does nothing, the popover shows `東京都`'s entry, `scrollTop` is reset to 0 on the cycle and on the furigana change. `a modified bracket is not a reading step` passes vacuously (nothing cycles yet); it earns its keep once Step 4 lands.

- [ ] **Step 3: `types.ts`, and the typed literals `tsc` then flags**

In `src/types.ts`, change `ParseResult` (lines 41–43) to:

```ts
export interface ParseResult {
  segments: Segment[];
  /**
   * Readings 2..k, each a full cover like `segments`. The order is the rank:
   * readings that differ from an earlier one only by a merge or split come
   * after every reading that does not (Phase 2b design §4).
   */
  alternatives: Segment[][];
}
```

Run `npx tsc --noEmit`. Expected: two errors, both `Property 'alternatives' is missing`, at `src/history.test.ts` (the `parse` helper's return) and `src/render/sentence.test.ts:37`. Fix them:

In `src/history.test.ts`, inside `function parse(...surfaces: string[]): ParseResult`, change the returned object so it reads:

```ts
  return {
    segments: surfaces.map((surface) => {
      const segment = {
        start,
        len: surface.length,
        surface,
        reading: null,
        matched: true,
        entries: [],
      };
      start += surface.length;
      return segment;
    }),
    alternatives: [],
  };
```

In `src/render/sentence.test.ts`, line 37, change `renderSentence({ segments: [] })` to `renderSentence({ segments: [], alternatives: [] })`.

`main.ts` compiles unchanged at this point because it only reads `segments`; the untyped payloads in `main.test.ts` are `unknown` and are not checked. Run `npx tsc --noEmit` again. Expected: clean.

- [ ] **Step 4: `main.ts`**

After the declaration `let lastResult: ParseResult | null = null;` (line 698) and its doc comment, add:

```ts
/**
 * Which of `lastResult`'s readings is on screen: 0 is `segments`, n ≥ 1 is
 * `alternatives[n - 1]` (Phase 2 design §6). `display` resets it whenever a
 * different result arrives; a re-render of the same result — a furigana
 * change, a reading switch — keeps it.
 */
let readingIndex = 0;

/** Every reading of `result`, top-1 first. */
function readings(result: ParseResult): Segment[][] {
  return [result.segments, ...result.alternatives];
}

/** The reading currently rendered, or `[]` before the first parse. */
function displayedSegments(): Segment[] {
  if (lastResult === null) return [];
  return readings(lastResult)[readingIndex] ?? lastResult.segments;
}

/** Show reading `index` of the current result, wrapping at either end. */
function showReading(index: number): void {
  if (lastResult === null) return;
  const count = readings(lastResult).length;
  if (count < 2) return;
  readingIndex = ((index % count) + count) % count;
  display(lastResult);
}

/**
 * `Reading n of k` radio rows for the right-click menu, then a separator —
 * or nothing at all when the parse has one reading, so the menu is exactly
 * what it was before Phase 2.
 */
function readingMenuEntries(): MenuEntry[] {
  if (lastResult === null) return [];
  const count = readings(lastResult).length;
  if (count < 2) return [];
  const entries: MenuEntry[] = [];
  for (let i = 0; i < count; i += 1) {
    entries.push({
      kind: 'radio',
      label: `Reading ${i + 1} of ${count}`,
      checked: i === readingIndex,
      onSelect: () => showReading(i),
    });
  }
  entries.push({ kind: 'separator' });
  return entries;
}
```

Change `segmentAt` (lines 703–706) to:

```ts
function segmentAt(start: string | undefined): Segment | undefined {
  if (start === undefined || lastResult === null) return undefined;
  return displayedSegments().find((segment) => String(segment.start) === start);
}
```

Replace `display` (lines 1193–1209, from `function display(result: ParseResult | null): void {` through its closing `}`; its doc comment above it stays) with:

```ts
function display(result: ParseResult | null): void {
  if (result === null) return;
  // Before anything is replaced: a popover left open would be anchored to a
  // chip from the previous sentence that is about to leave the document.
  closePopover();
  // The same result re-rendered — a reading switch, a furigana change — keeps
  // the reading on screen and the reader's place in the pane: both happen
  // while reading one word, and losing the scroll position would make a
  // reading switch useless on exactly the long sentences it is for (Phase 2b
  // design §6). A different result is a new sentence: reading 1, top of the
  // pane.
  const same = result === lastResult;
  if (!same) readingIndex = 0;
  const scrollTop = same ? panes.scrollTop : 0;
  lastResult = result;

  const sentence = renderSentence(
    { segments: displayedSegments(), alternatives: [] },
    currentFuriganaMode,
  );
  output.replaceChildren(sentence);
  // `.panes` is the scroll container (up to 10,000 clipboard characters can
  // far exceed a viewport), and replacing `#output`'s children never touches
  // it on its own — so it is set explicitly: back to where it was for a
  // re-render, to the top for a new result rather than wherever a previous,
  // longer parse happened to leave it.
  panes.scrollTop = scrollTop;
  syncHistoryButtons();
}
```

Change the `contextmenu` listener (lines 486–489) to:

```ts
document.addEventListener('contextmenu', (event) => {
  event.preventDefault();
  openContextMenu([...readingMenuEntries(), ...chromeMenuEntries()], event.clientX, event.clientY);
});
```

Change the `keydown` listener (lines 1053–1057) to:

```ts
document.addEventListener('keydown', (e) => {
  // Focus is deliberately not moved: the user is mid-sentence, and Escape
  // dismissing a tooltip should not cost them their place in the tab order.
  if (e.key === 'Escape') closePopover();
  // `]` / `[` step through the current parse's readings (Phase 2 design §6).
  // Bare keys: this window has no text input, both exist on JIS and US
  // layouts, and neither scrolls `.panes`. The Alt-modified pair is history
  // (`history-step`, Rust-side) and the webview sees that keydown too, so a
  // modifier means this is not a reading step.
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  if (e.key === ']') showReading(readingIndex + 1);
  if (e.key === '[') showReading(readingIndex - 1);
});
```

- [ ] **Step 5: `parse.rs`**

In `src-tauri/src/parse.rs`, before `pub async fn run_worker(` (line 91) and after `current_hints`'s closing `}`, add:

```rust
/// Readings the app asks the engine for, top-1 included: Phase 2 design §1
/// fixes it at three. The webview shows `segments` and offers the two
/// `alternatives` from its right-click menu, in the order Phase 2b's
/// `lattice::demote` gives them.
const READINGS: usize = 3;
```

and change `&ParseOptions::default(),` inside `run_worker` (line 111) to `&ParseOptions::with_readings(READINGS),`. The other `ParseOptions::default()` at line 270 is in a test and stays.

- [ ] **Step 6: Run the tests**

Run: `npm test`, then `npx tsc --noEmit`, then `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: `npm test` reports 278 passed (269 + 9); `tsc` clean; `src-tauri` 149 passed.

- [ ] **Step 7: Commit**

```bash
git add src/types.ts src/main.ts src/main.test.ts src/history.test.ts src/render/sentence.test.ts src-tauri/src/parse.rs
git commit -m "feat(app): offer the engine's second and third readings

The worker asks for three readings; the main window renders the chosen
one, lists the others as 'Reading n of 3' radio rows in the right-click
menu, and steps through them with ] and [. Hover resolves against the
reading on screen; a new parse starts at reading 1 and at the top of the
pane, while a reading switch or a furigana change keeps the reader's
scroll position. A modified bracket is left to history navigation."
```

---

### Task 4: Handoff completion and master plan edits

**Files:**
- Modify: `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md`
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — §6 phase order (the fenced block at lines 682–690); §7 Phase 2, the paragraph beginning `**Reopened 2026-09-09 as Phase 2b.**`

**Interfaces:** none.

- [ ] **Step 1: Complete the handoff**

Append to `docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md`, after §3. If the gate held, use the first `## 4`; if it did not, the second. Replace every bracketed placeholder with the real figure or hash.

```markdown
## 4. What landed after the gate

`src-tauri/src/parse.rs` asks for three readings; `src/main.ts` renders the
chosen one, offers the others as `Reading n of 3` radio rows in the
right-click menu, steps through them with `]` and `[`, and keeps the
sentence pane's scroll position across a reading switch and a furigana
re-render (design §6). Commit `[hash]`.
```

or

```markdown
## 4. What landed after the gate

Nothing. Task 1's reorder stays — a better rank order at zero cost, with
`alternatives` still empty on the wire — and no UI lands, as design §7 and
master plan §7 allow. The instrument is unchanged and the next attempt
starts from handoff §2 (and §2b, if it ran).
```

Then, in both cases:

```markdown
## 5. Standing gates at the phase's final code commit

Run at `[hash]`: `cargo test -p jparser` ([n] passed), the same with
`--features mecab` ([n] passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` ([n] passed), `npx tsc --noEmit` clean,
`cargo test --manifest-path src-tauri/Cargo.toml` (149 passed). Eight cells
byte-identical at `--readings 1`.

## 6. Open items

- **The product question is now live, not hypothetical.** `くるまで` against
  `くる` + `まで` is exactly the relation `demote` sinks (design §13). The
  order that ships is the one the gate can see; whether it is the one a
  reader wants is what the UI now makes observable for the first time. The
  whole-sentence cover count in §3 is the measured footprint so far.
- **k stays at 3 on the wire**, so the sunk merge/split variants do not reach
  the reader at all. Raising k for the app while the gate stays at 3 is the
  named follow-up once there is UI evidence to argue from (design §13).
- **The scroll guard covers furigana re-renders too**, because both paths
  re-render `lastResult`. If that turns out to change furigana behaviour
  someone relies on, the guard narrows to the reading-switch path
  (design §13).
- The 9 gold spans with no candidate and the 3 with the span but not the
  entry (Phase 2 handoff §6) are unchanged by this phase and remain lexicon
  or index findings.
```

- [ ] **Step 2: Master plan §6**

In the fenced phase-order block (lines 682–690), change the line

```
         2b  reading identity, gate cleared at +65 off-engine
```

to, if the gate held,

```
         2b  reading identity, gate cleared at +D on-engine, UI shipped
```

or, if it did not,

```
         2b  reading identity, +65 off-engine did not survive on-engine (+D)
```

with `D` the final handoff §3 figure.

- [ ] **Step 3: Master plan §7**

Immediately after the paragraph beginning `**Reopened 2026-09-09 as Phase 2b.**` (which ends `...for what ships.`), add one paragraph. If the gate held:

```markdown
**Measured [date] and met:** oracle@3 tolerant hints on reached T of 2,163
through the shipped path, D spans over top-1, against the 55 required, over
[the pool k = 3 already holds | a pool floored at 12 readings, at +X% wall
time]. Phase 2b shipped the reading-cycling UI; see the Phase 2b handoff §3
and §4. Whole-sentence cover in the top three is N of 150 (Phase 2: 25),
recorded and unthresholded.
```

If it did not:

```markdown
**Measured [date] and not met:** oracle@3 tolerant hints on reached T of
2,163 through the shipped path, D spans over top-1, against the 55 required,
[over the existing pool and over a pool floored at 12 readings]. The
off-engine +65 did not survive the engine's own pool. The reorder stays as a
better rank order at zero cost; no UI lands. See the Phase 2b handoff §3.
```

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/2026-09-09-engine-v2-phase2b-handoff.md docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
git commit -m "docs(engine-v2): close Phase 2b; record the on-engine gate result in the handoff and master plan"
```

---

## Self-review

**Spec coverage.** §1 scope and unchanged bar → Global Constraints (gate 1,898; nothing in `cost.rs`, `is_allowed`, wire). §2 settled decisions → Task 1's `demote` is symmetric containment and deletes nothing; the doc comments cite the measurement. §3.1 relation and its three properties → `cuts`/`demote` and the tests `a_reading_unrelated_to_rank_zero_outranks_a_re_split_of_it` (fresh-list-only, non-transitivity in its comment), `readings_all_equivalent_to_rank_zero_keep_their_cost_order` (within-group order, rank 0 first), `demotion_never_moves_rank_zero`. §3.2 placement → Task 1 Step 4's tail and `undominated`'s prefix note. §3.3 free pool first, `DEMOTE_POOL` only on shortfall → Task 1 measures at cap 12; Task 2 is conditional and carries the latency clause. §4 API doc → Task 1 Step 6 (`lib.rs`) and Task 3 Step 3 (`types.ts`). §5 harness unchanged → no task edits `score`. §6 UI and scroll guard → Task 3 Step 4 (`display`, modifier guard added beside the spec's keys). §7 staging → Tasks 1, 2, 3 in order, gate written into the handoff at Task 1 Step 11 before Task 3. §8 clauses 1–6 → Task 1 Steps 8, 10; Task 2 Steps 6–8; Task 3 Step 6; Task 4 §5. §9 tests → each named test appears above; the `東京都` test gains its comment. §10 budget → `lattice.rs` grows by `cuts`, `demote`, the `undominated` doc and five tests, within +45 code / +90 tests; no new file. §11 → already applied; Task 4 adds the outcome paragraph. §13 → Task 4 §6 open items.

**Placeholder scan.** The handoff templates carry `T`, `D`, `S`, `N`, `[hash]`, `[date]`, `(paste ... here)` deliberately — they are the measurement's output and cannot be known before Task 1 runs; each is named and its source step is given. No other placeholders.

**Type consistency.** `cuts(path: &[Step]) -> BTreeSet<usize>` and `demote(pool: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>>` are used with those signatures in `k_best`'s tail and the tests; `undominated`'s parameter is `bound` in its body and doc. `pool_cap(k: usize) -> usize` and `DEMOTE_POOL: usize` appear only in Task 2 and agree between test and implementation. TypeScript: `readingIndex`, `readings(result)`, `displayedSegments()`, `showReading(index)`, `readingMenuEntries()` are declared once in Task 3 Step 4 and used with those names in `segmentAt`, `display`, the two listeners and the tests; `panes` is the existing `const` at `main.ts:86`. `READINGS: usize = 3` matches the Global Constraint.
