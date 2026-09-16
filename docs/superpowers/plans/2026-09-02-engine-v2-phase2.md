# Engine v2 Phase 2 — k-best readings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Return the three cheapest distinct segmentations of a sentence, measure whether readings 2 and 3 earn a UI (oracle@3 tolerant, hints on, at least 55 spans over top-1), and if they do, let the user cycle them from the main window.

**Architecture:** `lattice::best_path` generalises to `lattice::k_best` by holding a bounded, key-deduplicated list per position instead of one node; `parse` assembles every returned path with the existing `to_segment` and puts readings 2..k in a new additive `ParseResult.alternatives`; `jparser-cli score --readings K` ORs each gold span across readings for oracle@K; the webview renders whichever reading `readingIndex` names and offers the others as radio rows in the existing right-click menu. Top-1 never moves: Phase 1b-i's eight cells are re-run after every commit.

**Tech Stack:** Rust 2021 (`jparser` crate: serde, clap 4, thiserror; `insta` in tests), Tauri 2 backend in `src-tauri`, TypeScript + Vite frontend tested with Vitest under happy-dom.

**Spec:** `docs/superpowers/specs/2026-09-02-engine-v2-phase2-design.md`

## Global Constraints

- **Top-1 is byte-identical at every commit.** After each task's code lands, `jparser-cli score` at the default `--readings 1` must print exactly Phase 1b-i handoff §2's eight cells: hints off `0.7272 (1573/2163)` strict, `0.7975 (1725/2163)` tolerant, `0.9075 (1108/1221)` entry top-1, `75` merges; hints on `0.8031 (1737/2163)`, `0.8521 (1843/2163)`, `0.9063 (1170/1291)`, `53` merges. The commands are in "Measuring" below.
- **Standing gates**, run before every commit that touches `crates/jparser`: `cargo test -p jparser`, `cargo test -p jparser --features mecab`, `cargo clippy -p jparser --all-targets -- -D warnings`, `cargo clippy -p jparser --all-targets --features mecab -- -D warnings`, `cargo check -p jparser --no-default-features`, `cargo fmt -p jparser -- --check`. Before a commit that touches `src/`: `npm test`. Before a commit that touches `src-tauri/`: `cargo test --manifest-path src-tauri/Cargo.toml`.
- **k is 3 readings total**, top-1 included. No setting, no constant exposed past `src-tauri/src/parse.rs`.
- **No scores on the wire.** `alternatives` is `Vec<Vec<Segment>>`, cost order, nothing else.
- **Alternates come from the constrained lattice only.** `is_allowed` is not touched; `cost.rs` is not touched.
- **Every new Rust file-level comment states provenance** the way `lattice.rs` line 1–5 does; every new source file starts with `// SPDX-License-Identifier: MIT OR Apache-2.0`. This plan creates no new source files.
- **Commit 4 (the UI, Task 6) lands only if Task 5's gate passes.** On fail, Tasks 1–5 stay and Task 6 is not started; Task 7's handoff records the number.
- **Docs are written in normal prose**, not the session's terse style.

### Measuring

The gold index on disk under `corpus_staging/gold_index` is format 3 and no longer opens (`VersionMismatch { found: 3, expected: 4 }`). Build a fresh one once per session and reuse it:

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase2"   # any scratch dir outside the tree
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

Rebuild the release binary (`cargo build --release ...` above) before re-running; `score` reads the binary, not the source.

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `crates/jparser/src/engine/lattice.rs` | `Node` gains `from_rank` and `key`; `best[]` becomes `Vec<Vec<Node>>`; new `k_best`, `extend_key`, `beats`, `slot`, `insert`, `path_of`, `matched_spans`, `undominated`; `best_path` becomes the k = 1 wrapper; new unit tests | 1 |
| `crates/jparser/src/lib.rs` | `ParseOptions { pub readings }` with `with_readings`; `ParseResult.alternatives` | 2 |
| `crates/jparser/src/engine/mod.rs` | `parse` calls `k_best` and assembles every path | 2 |
| `crates/jparser/tests/engine_v2.rs` | struct literals gain `alternatives`; new `TOKYO_FIXTURE` tests; corpus invariance test; wire-shape extension | 2 |
| `crates/jparser/src/bin/jparser-cli/score.rs` | `parsed` helper gains `alternatives` (Task 2); `Oracle`, `fit`, `covers_exactly`, oracle lines, `run(.., readings, ..)`; unit tests | 2, 3 |
| `crates/jparser/src/bin/jparser-cli/main.rs` | `Score { readings }` | 3 |
| `crates/jparser/tests/cli_score.rs` | `--readings 3` text and JSON tests | 3 |
| `src-tauri/src/parse.rs` | test literal gains `alternatives` (Task 2); worker passes `with_readings(3)` (Task 6) | 2, 6 |
| `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md` | created with the measurement and the gate decision | 5, 7 |
| `src/types.ts` | `ParseResult.alternatives` | 6 |
| `src/main.ts` | `readingIndex`, `readings`, `displayedSegments`, `showReading`, `readingMenuEntries`; `display`, `segmentAt`, the `contextmenu` and `keydown` listeners | 6 |
| `src/main.test.ts` | six reading tests | 6 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | the five edits spec §11 owes | 7 |

---

### Task 1: `lattice::k_best`

**Files:**
- Modify: `crates/jparser/src/engine/lattice.rs` (whole non-test body lines 17–181; tests appended inside the existing `mod tests`)

**Interfaces:**
- Consumes: `cost::{cost, is_allowed, unmatched_cost}`, `lex::Candidate`, `engine::Boundaries` — all unchanged.
- Produces: `pub fn k_best(chars: &[char], by_start: &[Vec<Candidate>], bounds: Option<&dyn Boundaries>, k: usize) -> Vec<Vec<Step>>` — paths in cost order, at most `k`, always at least one, empty input yields `vec![Vec::new()]`. `pub fn best_path(..) -> Vec<Step>` keeps its signature and returns `k_best(.., 1)`'s first path. `Step` is unchanged.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `crates/jparser/src/engine/lattice.rs`, after `the_empty_input_yields_an_empty_path`:

```rust
    /// The spans each reading matches, in order -- what distinguishes one
    /// reading from another.
    fn spans_of(path: &[Step]) -> Vec<(usize, usize)> {
        path.iter()
            .filter_map(|s| match s {
                Step::Matched(c) => Some((c.start, c.len)),
                Step::Unmatched { .. } => None,
            })
            .collect()
    }

    #[test]
    fn k_best_with_k_one_is_best_path() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let cands = by_start(5, vec![c(0, 2, 1, WordFlags::PRIMARY), c(2, 3, 2, WordFlags::PRIMARY)]);
        assert_eq!(k_best(&chars, &cands, None, 1), vec![best_path(&chars, &cands, None)]);
    }

    #[test]
    fn readings_come_out_distinct_and_in_cost_order() {
        // あいう as one word, or あい + う. Nothing else matches, and every
        // path that merely un-matches one of those words is dominated.
        let chars: Vec<char> = "あいう".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| spans_of(p)).collect();
        // Equal cost (3 each); the longer last step wins the tie, as at k = 1.
        assert_eq!(spans, vec![vec![(0, 3)], vec![(0, 2), (2, 1)]]);
    }

    #[test]
    fn two_entries_at_one_span_are_one_reading() {
        let chars: Vec<char> = "あ".chars().collect();
        let cands = by_start(1, vec![c(0, 1, 1, WordFlags::PRIMARY), c(0, 1, 2, WordFlags::PRIMARY)]);
        let readings = k_best(&chars, &cands, None, 3);
        assert_eq!(readings.len(), 1, "the second entry is a sibling, not a reading");
        assert_eq!(spans_of(&readings[0]), vec![(0, 1)]);
    }

    #[test]
    fn an_unmatched_run_yields_no_fragment_variants() {
        let chars: Vec<char> = "」、「".chars().collect();
        let readings = k_best(&chars, &by_start(3, vec![]), None, 3);
        assert_eq!(readings.len(), 1, "every split of the run is the same reading");
        assert_eq!(readings[0].len(), 1, "and it is kept as one run");
    }

    #[test]
    fn a_reading_that_only_unmatches_a_word_is_dropped() {
        let chars: Vec<char> = "あい".chars().collect();
        let cands = by_start(2, vec![c(0, 1, 1, WordFlags::PRIMARY), c(1, 1, 2, WordFlags::PRIMARY)]);
        let readings = k_best(&chars, &cands, None, 3);
        assert_eq!(readings.len(), 1, "あ+い is the only reading; leaving either unmatched is a worse copy of it");
    }

    #[test]
    fn a_reading_that_leaves_an_unmatchable_character_unmatched_is_kept() {
        // あい, or いう -- each leaves the other's spare character unmatched,
        // and neither matches everything the other does.
        let chars: Vec<char> = "あいう".chars().collect();
        let cands = by_start(3, vec![c(0, 2, 1, WordFlags::PRIMARY), c(1, 2, 2, WordFlags::PRIMARY)]);
        let readings = k_best(&chars, &cands, None, 3);
        let spans: Vec<Vec<(usize, usize)>> = readings.iter().map(|p| spans_of(p)).collect();
        // Both cost 32; いう closes the sentence with a longer step than う.
        assert_eq!(spans, vec![vec![(1, 2)], vec![(0, 2)]]);
    }

    #[test]
    fn a_forbidden_candidate_appears_in_no_reading() {
        struct At(&'static [usize]);
        impl crate::engine::Boundaries for At {
            fn is_boundary(&self, p: usize) -> bool {
                self.0.contains(&p)
            }
        }
        let chars: Vec<char> = "東京都".chars().collect();
        let cands = by_start(
            3,
            vec![
                c(0, 3, 1, WordFlags::PRIMARY),
                c(1, 2, 9, WordFlags::PRIMARY), // 京都 starts inside the 東京 token
                c(0, 2, 2, WordFlags::PRIMARY),
                c(2, 1, 3, WordFlags::PRIMARY),
            ],
        );
        let readings = k_best(&chars, &cands, Some(&At(&[0, 2, 3])), 3);
        assert!(readings.len() >= 2);
        for path in &readings {
            assert!(!path.iter().any(|s| matches!(s, Step::Matched(x) if x.entry_id == 9)));
        }
    }

    #[test]
    fn every_reading_covers_the_input_exactly_once() {
        let chars: Vec<char> = "あいうえお".chars().collect();
        let cands = by_start(
            5,
            vec![
                c(0, 2, 1, WordFlags::PRIMARY),
                c(0, 3, 2, WordFlags::PRIMARY),
                c(3, 2, 3, WordFlags::PRIMARY),
                c(2, 3, 4, WordFlags::PRIMARY),
            ],
        );
        for path in k_best(&chars, &cands, None, 3) {
            let mut at = 0;
            for step in &path {
                let (s, l) = match step {
                    Step::Matched(x) => (x.start, x.len),
                    Step::Unmatched { start, len } => (*start, *len),
                };
                assert_eq!(s, at);
                at += l;
            }
            assert_eq!(at, 5);
        }
    }

    #[test]
    fn the_empty_input_yields_one_empty_reading() {
        assert_eq!(k_best(&[], &[], None, 3), vec![Vec::<Step>::new()]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --lib lattice`
Expected: compile error, `cannot find function `k_best` in this scope`.

- [ ] **Step 3: Replace the DP body**

Replace everything in `crates/jparser/src/engine/lattice.rs` from `const MAX_UNMATCHED_RUN` (line 31) through the end of `best_path` (line 181) with:

```rust
const MAX_UNMATCHED_RUN: usize = 64;

/// Raw readings the DP keeps per position for every reading `k_best` is
/// asked for. `undominated` runs after the backtrack and can only remove
/// readings, so the DP over-provisions: each matched span of a reading has a
/// variant that merely leaves it unmatched, about `UNMATCHED - COVER` dearer
/// and never a reading in its own right.
///
/// ponytail: fixed factor. A top-1 with more than `3 * k` matched spans has
/// more such variants than the raw list has spare slots, and a genuine
/// alternate priced above them (a PRONOUNCE crossing plus INEXACT, about
/// +33) can fall off the end; the upgrade path is dominance pruning inside
/// the lists, which is safe because domination at a position survives every
/// extension.
const RAW_FACTOR: usize = 4;

/// One step of a segmentation: either a dictionary match, or a run of input
/// no candidate covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Matched(Candidate),
    Unmatched { start: usize, len: usize },
}

impl Step {
    fn len(&self) -> usize {
        match self {
            Step::Matched(c) => c.len,
            Step::Unmatched { len, .. } => *len,
        }
    }
}

/// One reading reaching an end position: what it cost, which reading at
/// `from` it extends, the step that closed the gap, and its identity.
struct Node {
    cost: i32,
    from: usize,
    /// Index into `best[from]` of the reading this one extends. Position 0
    /// has no list -- it is the start of the sentence -- so a node with
    /// `from == 0` ignores this.
    from_rank: usize,
    step: Step,
    /// The reading's identity: a hash of every matched `(start, len)` on the
    /// path so far, in position order. Unmatched steps leave it unchanged,
    /// so two paths that differ only in how they split an unmatched run, or
    /// in which same-span entry they carry, are one reading and share a
    /// slot. A collision would merge two genuinely different readings; at
    /// tens of spans and 64 bits that is a non-event, and the failure is a
    /// missing alternate, never a wrong segment.
    key: u64,
}

/// `key` extended by `step`: unchanged for an unmatched run; for a match,
/// the span mixed in through a splitmix64 finaliser so the result depends on
/// the whole set of spans and not on the last one alone.
fn extend_key(key: u64, step: &Step) -> u64 {
    let Step::Matched(c) = step else {
        return key;
    };
    let span = ((c.start as u64) << 32) | c.len as u64;
    let mut x = (key ^ span).wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The k = 1 tie-break as an ordering: cheaper first, then the longer last
/// step first. A full tie is decided by insertion order -- `insert` places a
/// new node after every node it does not strictly beat -- which is what
/// `Some(_) => false` did when there was one slot: the same-script gap
/// `」、「` costs the same as one run or three, and the longer run wins so it
/// never fragments on iteration order, while a mixed-script run still splits
/// at its script change because that path is strictly cheaper.
fn beats(cost: i32, len: usize, other: &Node) -> bool {
    cost < other.cost || (cost == other.cost && len > other.step.len())
}

/// Whether a node of `cost`, last-step `len` and `key` would enter `list`
/// under `cap`: not if a node with the same key is already there and is not
/// beaten, and not if it would land at or past `cap`. Split from `insert` so
/// `relax` clones a `Step` only for nodes that are actually kept.
fn accepts(list: &[Node], cost: i32, len: usize, key: u64, cap: usize) -> bool {
    if let Some(same) = list.iter().find(|n| n.key == key) {
        if !beats(cost, len, same) {
            return false;
        }
    }
    let at = list.iter().position(|n| beats(cost, len, n)).unwrap_or(list.len());
    at < cap
}

/// Put `node` in rank order, replacing a same-key node it beats, and drop
/// whatever falls past `cap`. Callers check `accepts` first.
fn insert(list: &mut Vec<Node>, node: Node, cap: usize) {
    if let Some(i) = list.iter().position(|n| n.key == node.key) {
        list.remove(i);
    }
    let at = list
        .iter()
        .position(|n| beats(node.cost, node.step.len(), n))
        .unwrap_or(list.len());
    list.insert(at, node);
    list.truncate(cap);
}

/// Offer every reading at `from` one way to reach `to`: `step`, at
/// `step_cost` on top of what that reading already cost.
///
/// The single relaxation point the module doc promises. Position 0 is the
/// start of the sentence and holds one implicit reading at cost 0 with an
/// empty key. Any other position offers each node of its list in rank order,
/// so on a full tie the extension of a higher-ranked reading is inserted
/// first and stays ahead -- which is what keeps rank 0 at every position
/// equal to the single winner the k = 1 DP produced.
fn relax(best: &mut [Vec<Node>], from: usize, to: usize, step_cost: i32, step: &Step, cap: usize) {
    // `from < to` always, so `from` is in the left half and `to` heads the right.
    let (before, rest) = best.split_at_mut(to);
    let dst = &mut rest[0];
    let len = step.len();
    let mut offer = |from_rank: usize, from_cost: i32, from_key: u64| {
        let cost = from_cost + step_cost;
        let key = extend_key(from_key, step);
        if accepts(dst.as_slice(), cost, len, key, cap) {
            insert(
                dst,
                Node {
                    cost,
                    from,
                    from_rank,
                    step: step.clone(),
                    key,
                },
                cap,
            );
        }
    };
    if from == 0 {
        offer(0, 0, 0);
    } else {
        for (rank, node) in before[from].iter().enumerate() {
            offer(rank, node.cost, node.key);
        }
    }
}

/// The path that ends as `best[end][rank]`, walked back through
/// `(from, from_rank)` to position 0.
fn path_of(best: &[Vec<Node>], end: usize, rank: usize) -> Vec<Step> {
    let mut path = Vec::new();
    let (mut pos, mut rank) = (end, rank);
    while pos != 0 {
        let node = &best[pos][rank];
        path.push(node.step.clone());
        (pos, rank) = (node.from, node.from_rank);
    }
    path.reverse();
    path
}

fn matched_spans(path: &[Step]) -> Vec<(usize, usize)> {
    path.iter()
        .filter_map(|s| match s {
            Step::Matched(c) => Some((c.start, c.len)),
            Step::Unmatched { .. } => None,
        })
        .collect()
}

/// The first `k` of `readings` (already in cost order) that no cheaper kept
/// reading dominates. A reading is dominated when another, cheaper one
/// matches every span it matches: it only leaves words unmatched that the
/// other found, a worse copy of that reading rather than a different one.
/// Rank 0 is never dominated. Checking against the kept list alone is
/// enough, because every dominator of a dropped reading is cheaper than it
/// and so was either kept or dominated in turn by something kept.
fn undominated(readings: Vec<Vec<Step>>, k: usize) -> Vec<Vec<Step>> {
    let mut kept: Vec<Vec<Step>> = Vec::with_capacity(k);
    let mut kept_spans: Vec<Vec<(usize, usize)>> = Vec::with_capacity(k);
    for reading in readings {
        if kept.len() == k {
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

/// The `k` cheapest distinct contiguous covers of `chars`, cheapest first,
/// choosing among `by_start`'s candidates under the boundaries `bounds`
/// allows. Distinct means by the set of spans matched (see `Node::key`), and
/// a reading that only un-matches what a cheaper one matches is not offered
/// (see `undominated`). Always at least one path; empty input yields exactly
/// one empty path.
///
/// `by_start[i]` is every candidate starting at character offset `i`, the
/// shape [`lex::candidates`] produces; a position past the end of a short
/// `by_start` is treated as having no candidates. A forbidden candidate
/// (`is_allowed` false) is never relaxed, so it appears in no reading. Every
/// position up to [`MAX_UNMATCHED_RUN`] ahead of a reachable one is itself
/// reachable via an unmatched run, so position `chars.len()` always holds at
/// least one node and `path_of` never runs off the front.
///
/// Rank 0 is exactly what the single-slot DP produced: same comparator
/// (`beats`), extensions of a position's rank-0 reading inserted before its
/// lower ranks', and every lower-ranked extension at least as dear as the
/// rank-0 extension with the same step.
pub fn k_best(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
    k: usize,
) -> Vec<Vec<Step>> {
    let k = k.max(1);
    let n = chars.len();
    if n == 0 {
        return vec![Vec::new()];
    }
    let cap = k * RAW_FACTOR;
    let mut best: Vec<Vec<Node>> = (0..=n).map(|_| Vec::new()).collect();

    for from in 0..n {
        if from != 0 && best[from].is_empty() {
            continue;
        }
        let cands = by_start.get(from).map(Vec::as_slice).unwrap_or(&[]);

        for cand in cands {
            if !is_allowed(cand, bounds) {
                continue;
            }
            let to = cand.start + cand.len;
            let step_cost = cost(cand, bounds);
            relax(&mut best, from, to, step_cost, &Step::Matched(cand.clone()), cap);
        }

        let max_to = (from + MAX_UNMATCHED_RUN).min(n);
        for to in (from + 1)..=max_to {
            let len = to - from;
            let step_cost = unmatched_cost(chars, from, len);
            relax(&mut best, from, to, step_cost, &Step::Unmatched { start: from, len }, cap);
        }
    }

    let raw: Vec<Vec<Step>> = (0..best[n].len()).map(|rank| path_of(&best, n, rank)).collect();
    undominated(raw, k)
}

/// The min-cost contiguous cover of `chars`: [`k_best`] at k = 1.
pub fn best_path(
    chars: &[char],
    by_start: &[Vec<Candidate>],
    bounds: Option<&dyn Boundaries>,
) -> Vec<Step> {
    k_best(chars, by_start, bounds, 1).swap_remove(0)
}
```

Then update the module doc at lines 7–15 to read:

```rust
//! The k cheapest paths over the candidate lattice: given every candidate
//! span [`lex::candidates`](crate::engine::lex::candidates) produced and the
//! price [`cost`](crate::engine::cost) puts on each, choose the contiguous
//! covers of the sentence that pay the least.
//!
//! A forward DP over end positions `0..=chars.len()`, relaxed in the single
//! place [`relax`], holding a bounded list of distinct readings per position
//! (Phase 2 design §3). [`best_path`] is the k = 1 case and is what the
//! engine produced before Phase 2, node for node.
```

- [ ] **Step 4: Run the lattice tests**

Run: `cargo test -p jparser --lib lattice`
Expected: all 14 tests pass (the five pre-existing ones untouched).

- [ ] **Step 5: Run the standing gates**

Run each of: `cargo test -p jparser`, `cargo test -p jparser --features mecab`, `cargo clippy -p jparser --all-targets -- -D warnings`, `cargo clippy -p jparser --all-targets --features mecab -- -D warnings`, `cargo check -p jparser --no-default-features`, `cargo fmt -p jparser -- --check`.
Expected: all pass. If `fmt --check` reports a diff, run `cargo fmt -p jparser` and re-run.

- [ ] **Step 6: Re-run the eight cells**

Run the "Measuring" commands. Expected: byte-identical to the two blocks above. If any number moved, rank 0 is not today's winner: stop, do not commit, and compare `beats`/`accepts`/`insert` against the old `relax` — the two orderings must agree on ties.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/src/engine/lattice.rs
git commit -m "feat(engine-v2): generalise best_path to k_best over distinct readings

A bounded list per position keyed by the set of matched spans, so two
paths that differ only in an unmatched split or a same-span entry are one
reading; dominated readings (only un-matching a cheaper reading's word)
dropped after the backtrack. best_path is k = 1 and the eight cells are
byte-identical."
```

---

### Task 2: `ParseOptions::with_readings` and `ParseResult.alternatives`

**Files:**
- Modify: `crates/jparser/src/lib.rs:27-43`
- Modify: `crates/jparser/src/engine/mod.rs:62-91`
- Modify: `crates/jparser/tests/engine_v2.rs` (literals at 837 and 880; new tests appended after `an_unmatched_segment_serializes_with_null_reading_and_no_entries`)
- Modify: `crates/jparser/src/bin/jparser-cli/score.rs:419-421`
- Modify: `src-tauri/src/parse.rs:164`

**Interfaces:**
- Consumes: `lattice::k_best` from Task 1.
- Produces: `ParseOptions { pub readings: usize }` (default 1) with `ParseOptions::with_readings(k: usize) -> Self` (`k.max(1)`); `ParseResult { segments, alternatives: Vec<Vec<Segment>> }`. Tasks 3 and 6 read both.

- [ ] **Step 1: Write the failing tests**

In `crates/jparser/tests/engine_v2.rs`, append after `an_unmatched_segment_serializes_with_null_reading_and_no_entries`:

```rust
// Phase 2: a private four-entry fixture on which 東京都 has exactly three
// readings, so `k_best`'s order and its dominance filter are both visible
// through `parse`. Private for the reason `KURU_FIXTURE` is.
const TOKYO_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry><ent_seq>1000100</ent_seq><k_ele><keb>東京都</keb></k_ele><r_ele><reb>とうきょうと</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo Metropolis</gloss></sense></entry>
<entry><ent_seq>1000110</ent_seq><k_ele><keb>東京</keb></k_ele><r_ele><reb>とうきょう</reb></r_ele><sense><pos>&n;</pos><gloss>Tokyo</gloss></sense></entry>
<entry><ent_seq>1000120</ent_seq><k_ele><keb>京都</keb></k_ele><r_ele><reb>きょうと</reb></r_ele><sense><pos>&n;</pos><gloss>Kyoto</gloss></sense></entry>
<entry><ent_seq>1000130</ent_seq><k_ele><keb>都</keb></k_ele><r_ele><reb>みやこ</reb></r_ele><sense><pos>&n;</pos><gloss>capital</gloss></sense></entry>
</JMdict>
"#;

fn parse_tokyo(name: &str, readings: usize) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(TOKYO_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    jparser::parse(
        &index,
        &table,
        "東京都",
        &jparser::ParseOptions::with_readings(readings),
        None,
    )
    .expect("parse")
}

fn surfaces(segments: &[jparser::Segment]) -> Vec<(&str, bool)> {
    segments.iter().map(|s| (s.surface.as_str(), s.matched)).collect()
}

#[test]
fn three_readings_come_out_in_cost_order_with_dominated_ones_dropped() {
    let r = parse_tokyo("tokyo-three", 3);
    assert_eq!(surfaces(&r.segments), [("東京都", true)]);
    assert_eq!(r.alternatives.len(), 2);
    assert_eq!(surfaces(&r.alternatives[0]), [("東京", true), ("都", true)]);
    // 東 has no entry here, so leaving it unmatched is the only way to read
    // 京都. 東京 + unmatched 都 is absent: 東京 + 都 is cheaper and matches
    // everything it does.
    assert_eq!(surfaces(&r.alternatives[1]), [("東", false), ("京都", true)]);
}

#[test]
fn the_default_options_return_no_alternatives_and_the_same_top_reading() {
    let one = parse_tokyo("tokyo-one", 1);
    let three = parse_tokyo("tokyo-three-again", 3);
    assert!(one.alternatives.is_empty());
    assert_eq!(one.segments, three.segments);
}

#[test]
fn every_alternative_is_a_contiguous_cover() {
    let r = parse_tokyo("tokyo-cover", 3);
    assert!(!r.alternatives.is_empty());
    for alt in &r.alternatives {
        let mut at = 0;
        for s in alt {
            assert_eq!(s.start, at);
            at += s.len;
        }
        assert_eq!(at, 3);
    }
}

/// Global constraint: asking for more readings never moves the first one.
/// The eight cells prove it on the gold corpus; this proves it on every
/// sentence the crate's own corpus holds, without a release build.
#[test]
fn asking_for_readings_never_moves_the_top_reading_on_the_corpus() {
    let dir = scratch("readings-invariant");
    build_v2(&dir);
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();
    for text in sentences() {
        let one = jparser::parse(&index, &table, text, &jparser::ParseOptions::default(), None)
            .expect("parse");
        let three = jparser::parse(
            &index,
            &table,
            text,
            &jparser::ParseOptions::with_readings(3),
            None,
        )
        .expect("parse");
        assert_eq!(one.segments, three.segments, "{text}");
    }
}

/// The wire shape of `alternatives`: an array of segment arrays, the same
/// segment shape `segments` uses, so `src/types.ts` needs one line.
#[test]
fn alternatives_serialize_as_an_array_of_segment_arrays() {
    let seg = |start: usize, len: usize, surface: &str| jparser::Segment {
        start,
        len,
        surface: surface.to_string(),
        reading: None,
        matched: true,
        entries: vec![],
    };
    let result = jparser::ParseResult {
        segments: vec![seg(0, 3, "東京都")],
        alternatives: vec![vec![seg(0, 2, "東京"), seg(2, 1, "都")]],
    };
    let json = serde_json::to_value(&result).expect("serialize");
    assert_eq!(json["alternatives"].as_array().map(Vec::len), Some(1));
    assert_eq!(json["alternatives"][0][1]["surface"], "都");
    assert_eq!(json["alternatives"][0][1]["start"], 2);
}
```

Also extend `parse_result_serializes_to_the_documented_wire_shape` (line 836): add `alternatives: Vec::new(),` after `segments: vec![...]` in its literal, and add at its end:

```rust
    // Present and empty at the default: the webview reads it unconditionally.
    assert_eq!(json["alternatives"].as_array().map(Vec::len), Some(0));
```

And add `alternatives: Vec::new(),` to the literal in `an_unmatched_segment_serializes_with_null_reading_and_no_entries` (line 880).

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --test engine_v2 tokyo`
Expected: compile error, `no function or associated item named `with_readings`` and `struct `ParseResult` has no field named `alternatives``.

- [ ] **Step 3: Change `lib.rs`**

Replace lines 27–43 of `crates/jparser/src/lib.rs` (`/// Parse-time options.` through `ParseResult`'s closing brace) with:

```rust
/// Parse-time options.
///
/// `readings` is how many distinct segmentations `parse` returns: the best
/// as `ParseResult::segments`, the rest as `ParseResult::alternatives`. The
/// default is one, so a caller that never asks sees exactly what it always
/// did. Gloss filters and furigana modes are Phase 3 display concerns above
/// this crate. `#[non_exhaustive]` so adding a field later is not a breaking
/// change; construct with `ParseOptions::default()` or
/// [`ParseOptions::with_readings`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ParseOptions {
    /// Distinct readings to return, at least 1.
    pub readings: usize,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self { readings: 1 }
    }
}

impl ParseOptions {
    /// `readings` distinct segmentations, cheapest first; anything below 1
    /// reads as 1.
    pub fn with_readings(readings: usize) -> Self {
        Self {
            readings: readings.max(1),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ParseResult {
    /// A contiguous cover of the input in ascending `start` order: every
    /// character belongs to exactly one segment, matched or not. Empty iff
    /// the input is empty.
    pub segments: Vec<Segment>,
    /// Readings 2..k in cost order, each a contiguous cover like `segments`
    /// and distinct from it and from each other by the spans they match
    /// (Phase 2 design §3, §4). Empty unless the caller asked for more than
    /// one reading through [`ParseOptions::with_readings`].
    pub alternatives: Vec<Vec<Segment>>,
}
```

- [ ] **Step 4: Change `parse`**

Replace `parse` in `crates/jparser/src/engine/mod.rs` (lines 62–91, doc comment included) with:

```rust
/// Parse `text` against an already-open index — the crate's public entry
/// point, re-exported as `jparser::parse`.
///
/// `bounds` is a tokenizer's boundary set, applied as a hard constraint:
/// no span may start or end strictly inside a token. `None` lifts every
/// constraint.
///
/// Three stages, each already a separate module: [`lex::candidates`] finds
/// every dictionary match at every span, [`lattice::k_best`] chooses the
/// `opts.readings` cheapest distinct covers among them, and
/// [`assemble::to_segment`] resolves each step of each cover into the
/// public [`Segment`]/[`Entry`] shape. The cheapest cover is
/// `ParseResult::segments`; the rest are `alternatives`.
///
/// [`Segment`]: crate::Segment
/// [`Entry`]: crate::Entry
pub fn parse(
    index: &Index,
    table: &table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError> {
    let chars: Vec<char> = text.chars().collect();
    let by_start = lex::candidates(index, table, &chars)?;
    let mut paths = lattice::k_best(&chars, &by_start, bounds, opts.readings);
    let assemble = |path: &[lattice::Step]| -> Result<Vec<crate::Segment>, IndexError> {
        path.iter()
            .map(|step| assemble::to_segment(index, &chars, step, &by_start))
            .collect()
    };
    // `k_best` always returns at least one path, an empty one for empty
    // input. `remove`, not `swap_remove`: the rest must stay in cost order.
    let segments = assemble(&paths.remove(0))?;
    let mut alternatives = Vec::with_capacity(paths.len());
    for path in &paths {
        alternatives.push(assemble(path)?);
    }
    Ok(ParseResult {
        segments,
        alternatives,
    })
}
```

Add `use crate::index::IndexError;` next to the existing `use crate::index::load::Index;` at line 12.

- [ ] **Step 5: Fix the struct literals**

- `crates/jparser/src/bin/jparser-cli/score.rs` line 419–421: `ParseResult { segments, alternatives: Vec::new() }`.
- `src-tauri/src/parse.rs` line 164: `ParseResult { segments: vec![], alternatives: vec![] }`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p jparser` and `cargo test --manifest-path src-tauri/Cargo.toml parse::`
Expected: all pass, including the five new `engine_v2` tests. If `three_readings_come_out_in_cost_order_with_dominated_ones_dropped` fails on the order of the first two readings, the tie between 東京都 (cost 3, one step of length 3) and 東京 + 都 (cost 3, last step length 1) is not being broken toward the longer last step — re-check `beats`.

- [ ] **Step 7: Run the standing gates and the eight cells**

Standing gates as in Task 1 Step 5; "Measuring" commands as in Task 1 Step 6. Expected: all pass, eight cells byte-identical (`score` still passes `ParseOptions::default()`, so `alternatives` is empty and nothing it reads changed).

- [ ] **Step 8: Commit**

```bash
git add crates/jparser/src/lib.rs crates/jparser/src/engine/mod.rs crates/jparser/tests/engine_v2.rs crates/jparser/src/bin/jparser-cli/score.rs src-tauri/src/parse.rs
git commit -m "feat(engine-v2): parse returns alternatives when asked for more than one reading

ParseOptions gains readings (default 1, with_readings(k)); ParseResult
gains alternatives, readings 2..k as full covers, empty by default so no
consumer moves. Eight cells byte-identical."
```

---

### Task 3: `score --readings K` and oracle@K

**Files:**
- Modify: `crates/jparser/src/bin/jparser-cli/score.rs` (`Metrics`, `accumulate`, `render`, `run`, tests)
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs:142-157` and `365-377`
- Modify: `crates/jparser/tests/cli_score.rs` (two new tests)

**Interfaces:**
- Consumes: `ParseOptions::with_readings`, `ParseResult.alternatives` from Task 2.
- Produces: `score::run(index_dir, gold_path, [hints,] readings: usize, json: bool)`; `pub struct Oracle { readings, span_hits, tolerant_hits, sentence_hits }` on `Metrics::oracle: Option<Oracle>`; three rendered lines `oracle@K strict:`, `oracle@K tolerant:`, `oracle@K sentences:` padded to 20 columns; JSON block `"oracle":{...}` only when `readings > 1`.

- [ ] **Step 1: Write the failing unit tests**

Inside `score.rs`'s `mod tests`, replace the `parsed` helper (lines 419–421) with:

```rust
    fn parsed(segments: Vec<Segment>) -> ParseResult {
        parsed_with(segments, Vec::new())
    }

    fn parsed_with(segments: Vec<Segment>, alternatives: Vec<Vec<Segment>>) -> ParseResult {
        ParseResult {
            segments,
            alternatives,
        }
    }

    fn with_oracle(readings: usize) -> Metrics {
        Metrics {
            oracle: Some(Oracle {
                readings,
                ..Oracle::default()
            }),
            ..Metrics::default()
        }
    }
```

Append these tests after `accumulate_splits_span_accuracy_by_decision`:

```rust
    /// Phase 2 design §5: per gold span, any reading counts. Top-1 merged
    /// both gold spans into one segment on gold boundaries, so span accuracy
    /// is 0/2 and both are forgiven; reading 2 gets both exactly, so oracle
    /// strict is 2/2 and the whole gold cover is present as one reading.
    #[test]
    fn oracle_credits_a_span_only_a_later_reading_gets_right() {
        let mut m = with_oracle(2);
        accumulate(
            &gold("あいう", &[(0, 1, Some(10)), (1, 2, Some(20))]),
            &parsed_with(
                vec![seg(0, 3, &[30])],
                vec![vec![seg(0, 1, &[10]), seg(1, 2, &[20])]],
            ),
            &mut m,
        );

        assert_eq!(m.span_hits, 0, "top-1 still scores as before");
        assert_eq!(m.coarse_forgiven, 2, "and its merge is still forgiven");
        let o = m.oracle.expect("oracle on");
        assert_eq!((o.span_hits, o.tolerant_hits, o.sentence_hits), (2, 2, 1));
    }

    /// Top-1 cuts inside a gold span (あい|う against gold あ|いう), so
    /// nothing is forgiven; reading 2 merges the whole sentence on gold
    /// boundaries, which tolerant forgives and strict does not.
    #[test]
    fn oracle_tolerant_credits_a_benign_merge_in_an_alternate() {
        let mut m = with_oracle(2);
        accumulate(
            &gold("あいう", &[(0, 1, Some(10)), (1, 2, Some(20))]),
            &parsed_with(
                vec![seg(0, 2, &[40]), seg(2, 1, &[50])],
                vec![vec![seg(0, 3, &[60])]],
            ),
            &mut m,
        );

        assert_eq!((m.span_hits, m.coarse_forgiven), (0, 0));
        let o = m.oracle.expect("oracle on");
        assert_eq!((o.span_hits, o.tolerant_hits, o.sentence_hits), (0, 2, 0));
    }

    /// With `--readings 1` there is no oracle, and alternates -- which the
    /// engine does not even produce then -- would change nothing if present.
    #[test]
    fn without_oracle_alternates_change_nothing() {
        let mut m = Metrics::default();
        accumulate(
            &gold("あ", &[(0, 1, Some(10))]),
            &parsed_with(vec![seg(0, 1, &[99])], vec![vec![seg(0, 1, &[10])]]),
            &mut m,
        );

        assert_eq!((m.span_hits, m.top1_hits), (1, 0));
        assert!(m.oracle.is_none());
    }

    #[test]
    fn render_adds_the_oracle_lines_only_when_present() {
        let mut m = Metrics {
            sentences: 2,
            gold_spans: 4,
            superseded: 0,
            span_hits: 3,
            top1_hits: 1,
            top1_total: 2,
            ..Metrics::default()
        };
        assert!(!render(&m).contains("oracle@"), "nothing to show at one reading");

        m.oracle = Some(Oracle {
            readings: 3,
            span_hits: 4,
            tolerant_hits: 4,
            sentence_hits: 1,
        });
        assert!(
            render(&m).ends_with(
                "oracle@3 strict:    1.0000  (4 / 4)\n\
                 oracle@3 tolerant:  1.0000  (4 / 4)\n\
                 oracle@3 sentences: 0.5000  (1 / 2)\n"
            ),
            "got {}",
            render(&m)
        );
    }
```

- [ ] **Step 2: Write the failing CLI tests**

Append to `crates/jparser/tests/cli_score.rs`:

```rust
/// Phase 2 design §5: `--readings 3` adds the oracle@3 lines after
/// everything the default prints. 言った is one gold span top-1 already gets,
/// so every oracle ratio is 1.0 by construction -- this pins the format and
/// the plumbing, not the engine.
#[test]
fn readings_above_one_add_the_oracle_lines() {
    let dir = scratch("cli-score-readings");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .arg("--readings")
        .arg("3")
        .output()
        .expect("runnable");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "sentences:      1\n\
         gold spans:     1\n\
         superseded:     0\n\
         span accuracy:  1.0000  (1 / 1)\n\
         entry top-1:    1.0000  (1 / 1)\n\
         oracle@3 strict:    1.0000  (1 / 1)\n\
         oracle@3 tolerant:  1.0000  (1 / 1)\n\
         oracle@3 sentences: 1.0000  (1 / 1)\n"
    );
}

/// The JSON form nests the oracle numbers under one key, present only when
/// asked -- `the_json_form_carries_the_same_numbers` above pins that the
/// default output has no such key.
#[test]
fn the_json_form_nests_the_oracle_block_only_when_asked() {
    let dir = scratch("cli-score-readings-json");
    let index = index_dir(&dir);
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        "{\"text\":\"言った\",\"source\":\"test:1\",\"spans\":[{\"start\":0,\"len\":3,\"ent_seq\":1000010}]}\n",
    )
    .unwrap();

    let out = Command::new(BIN)
        .arg("score")
        .arg(&index)
        .arg(&gold)
        .arg("--readings")
        .arg("3")
        .arg("--json")
        .output()
        .expect("runnable");
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains(r#""oracle":{"readings":3,"span_hits":1,"tolerant_hits":1,"sentence_hits":1}"#),
        "got {stdout}"
    );
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p jparser --bin jparser-cli oracle` and `cargo test -p jparser --test cli_score readings`
Expected: compile errors — `Oracle` not found, `Metrics` has no field `oracle`; the CLI tests fail with `unexpected argument '--readings'`.

- [ ] **Step 4: Add `Oracle` and the field**

In `score.rs`, after `DecisionBreakdown` (line 40), add:

```rust
/// oracle@K (master plan §5.2; Phase 2 design §5). Per gold span, a hit if
/// *any* of the K readings reproduces it -- a user cycles per word, and the
/// gate compares against top-1 span accuracy, a per-span metric.
/// `sentence_hits` is the master plan's literal form, the whole gold cover
/// present as one reading, printed as context.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Oracle {
    pub readings: usize,
    pub span_hits: usize,
    pub tolerant_hits: usize,
    pub sentence_hits: usize,
}
```

Add to `Metrics`, after `index_generation`:

```rust
    /// Present only when `--readings` asked for more than one: how often the
    /// gold is reachable anywhere in the reading list, not just at the top.
    /// Absent at the default so the recorded baselines serialize unchanged.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oracle: Option<Oracle>,
```

- [ ] **Step 5: Refactor `accumulate`**

Replace `accumulate` (lines 139–191) with:

```rust
/// How one reading (a segment list) relates to one gold span.
struct Fit {
    /// A segment sits exactly on the span.
    strict: bool,
    /// Set only when not `strict`: the matched segment that swallowed the
    /// span whole, starting and ending on gold boundaries -- a merge of whole
    /// gold spans rather than a cut through one.
    merge: Option<(usize, usize)>,
    /// The exactly-placed segment's first entry, if it has one.
    first_entry: Option<u32>,
}

fn fit(segments: &[Segment], span: &GoldSpan, bounds: &BTreeSet<usize>) -> Fit {
    let seg = segments
        .iter()
        .find(|s| s.start == span.start && s.len == span.len);
    // Benign coarsening: a matched segment that contains this gold span and
    // whose own endpoints are gold boundaries merged whole gold spans rather
    // than cutting one. A linear scan over a sentence's segments; corpora
    // are hundreds of sentences, so an index is not worth its bookkeeping.
    let merge = match seg {
        Some(_) => None,
        None => segments
            .iter()
            .find(|j| {
                j.matched
                    && j.start <= span.start
                    && span.start + span.len <= j.start + j.len
                    && bounds.contains(&j.start)
                    && bounds.contains(&(j.start + j.len))
            })
            .map(|j| (j.start, j.len)),
    };
    Fit {
        strict: seg.is_some(),
        merge,
        first_entry: seg.and_then(|s| s.entries.first().map(|e| e.entry_id)),
    }
}

/// Whether `segments` is the gold cover itself, span for span.
fn covers_exactly(segments: &[Segment], gold: &GoldRecord) -> bool {
    segments.len() == gold.spans.len()
        && segments
            .iter()
            .zip(&gold.spans)
            .all(|(s, g)| s.start == g.start && s.len == g.len)
}

/// Fold one sentence's gold and parse into `m`.
pub fn accumulate(gold: &GoldRecord, parsed: &ParseResult, m: &mut Metrics) {
    m.sentences += 1;
    // Every gold boundary, so a merge can be tested for cutting one. Starts
    // plus the last span's end: gold spans are a contiguous cover, so that is
    // the whole set.
    let mut bounds: BTreeSet<usize> = gold.spans.iter().map(|s| s.start).collect();
    if let Some(last) = gold.spans.last() {
        bounds.insert(last.start + last.len);
    }
    // Per record, so a segment forgiving three spans counts as one merge.
    let mut merges: BTreeSet<(usize, usize)> = BTreeSet::new();

    for span in &gold.spans {
        m.gold_spans += 1;
        let top = fit(&parsed.segments, span, &bounds);
        if let Some(decision) = gold.decision.as_deref() {
            bump_decision(&mut m.by_decision, decision, top.strict);
        }
        if let Some(merge) = top.merge {
            m.coarse_forgiven += 1;
            merges.insert(merge);
        }
        if let Some(o) = &mut m.oracle {
            let mut strict = top.strict;
            let mut tolerant = top.strict || top.merge.is_some();
            for alt in &parsed.alternatives {
                let f = fit(alt, span, &bounds);
                strict |= f.strict;
                tolerant |= f.strict || f.merge.is_some();
            }
            o.span_hits += strict as usize;
            o.tolerant_hits += tolerant as usize;
        }
        if !top.strict {
            continue;
        }
        m.span_hits += 1;
        // A null gold span asserts nothing about ranking, so it leaves both
        // halves of entry top-1 untouched.
        let Some(want) = span.ent_seq else { continue };
        m.top1_total += 1;
        // `entries.first()` is None for an unmatched span, which is a miss
        // rather than an exclusion — design §5.2.
        if top.first_entry == Some(want) {
            m.top1_hits += 1;
        }
    }
    m.coarse_merges += merges.len();

    if let Some(o) = &mut m.oracle {
        let whole = std::iter::once(parsed.segments.as_slice())
            .chain(parsed.alternatives.iter().map(Vec::as_slice))
            .any(|segments| covers_exactly(segments, gold));
        o.sentence_hits += whole as usize;
    }
}
```

Change the two imports at the top of `score.rs`: line 26 becomes `use jparser::{ParseOptions, ParseResult, Segment};` and line 28 becomes `use crate::corpus::{read_gold, validate, GoldRecord, GoldSpan};`. The test module's own explicit imports of `GoldRecord`, `GoldSpan`, `ParseResult` and `Segment` now also arrive through `use super::*;`; an explicit import beside a glob import of the same item is allowed and needs no change.

- [ ] **Step 6: Render and run**

Add after `LABEL_WIDTH` (line 32):

```rust
/// Field width for the oracle lines. `oracle@3 sentences:` is 19 chars.
const ORACLE_LABEL_WIDTH: usize = 20;
```

In `render`, after the `if m.coarse_forgiven > 0 { ... }` block and before `out`:

```rust
    // Additive like the two blocks above, and absent at one reading.
    if let Some(o) = &m.oracle {
        let w = ORACLE_LABEL_WIDTH;
        let k = o.readings;
        out.push_str(&format!(
            "{:<w$}{}\n",
            format!("oracle@{k} strict:"),
            ratio(o.span_hits, m.gold_spans)
        ));
        out.push_str(&format!(
            "{:<w$}{}\n",
            format!("oracle@{k} tolerant:"),
            ratio(o.tolerant_hits, m.gold_spans)
        ));
        out.push_str(&format!(
            "{:<w$}{}\n",
            format!("oracle@{k} sentences:"),
            ratio(o.sentence_hits, m.sentences)
        ));
    }
```

Change `run`'s signature to:

```rust
pub fn run(
    index_dir: &Path,
    gold_path: &Path,
    #[cfg(feature = "mecab")] hints: Option<&Path>,
    readings: usize,
    json: bool,
) -> Result<(), Box<dyn Error>> {
```

and in its body: the `Metrics` literal gains `oracle: (readings > 1).then(|| Oracle { readings, ..Oracle::default() }),`; `let opts = ParseOptions::default();` becomes `let opts = ParseOptions::with_readings(readings);`. Update `run`'s doc comment with one sentence: "`readings` above 1 asks the engine for that many readings per sentence and adds the oracle@K lines (Phase 2 design §5); at 1 every byte of output is what it was before Phase 2."

- [ ] **Step 7: The CLI flag**

In `main.rs`'s `Score` variant (lines 144–157), after the `hints` field add:

```rust
        /// Readings per sentence to score. Above 1, the engine returns that
        /// many and the oracle@K lines are added (Phase 2 design §5).
        #[arg(
            long,
            default_value_t = 1,
            value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
        )]
        readings: usize,
```

And in the match arm (lines 365–377) destructure `readings` and pass it: `score::run(&index, &gold, #[cfg(feature = "mecab")] hints.as_deref(), readings, json)?`.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p jparser`
Expected: all pass, including the four new unit tests and two new CLI tests, and the pre-existing `the_json_form_carries_the_same_numbers` unchanged (no `oracle` key at the default).

- [ ] **Step 9: Standing gates and the eight cells**

Standing gates as in Task 1 Step 5. Rebuild the release binary and run the "Measuring" commands. Expected: byte-identical.

- [ ] **Step 10: Commit**

```bash
git add crates/jparser/src/bin/jparser-cli/score.rs crates/jparser/src/bin/jparser-cli/main.rs crates/jparser/tests/cli_score.rs
git commit -m "feat(engine-v2): score --readings K reports oracle@K

Per gold span, a hit if any of the K readings reproduces it (strict) or
forgives it as a benign merge (tolerant); sentence-level whole-cover count
beside them. Default 1 leaves every byte of output unchanged."
```

---

### Task 4: Measure oracle@3 on the gold corpus

**Files:**
- Create: `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md`

**Interfaces:**
- Consumes: the release `jparser-cli` from Task 3.
- Produces: the gate decision, written down before Task 6 starts.

- [ ] **Step 1: Run both hint states at three readings**

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3 --hints "$DIC"
```

Expected: the eight cells' lines exactly as in "Measuring", followed by three `oracle@3` lines in each run.

- [ ] **Step 2: Decide the gate**

From the hints-on run, take the `oracle@3 tolerant:` numerator `T`. The gate (spec §8) is `T ≥ 1898`, i.e. `T − 1843 ≥ 55`. Write down `T − 1843` and the verdict: **pass** if ≥ 55, **fail** otherwise.

- [ ] **Step 3: Write the handoff**

Create `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md` with this content, the two `score` outputs pasted verbatim into the fenced blocks and the verdict sentence filled from Step 2:

```markdown
# Engine v2 Phase 2 — handoff

**Date:** 2026-09-02
**Design:** [`specs/2026-09-02-engine-v2-phase2-design.md`](specs/2026-09-02-engine-v2-phase2-design.md)
**Plan:** [`plans/2026-09-02-engine-v2-phase2.md`](plans/2026-09-02-engine-v2-phase2.md)
**Branch:** `feat/engine-v2`

## 1. What landed before the gate

Three commits, each re-measured at `--readings 1` against Phase 1b-i handoff
§2's eight cells and byte-identical to them:

1. `lattice::k_best` — the bounded, key-deduplicated list per position;
   `best_path` is its k = 1 case.
2. `ParseOptions::with_readings` and `ParseResult.alternatives`, empty by
   default.
3. `jparser-cli score --readings K` with the oracle@K lines.

## 2. The gate measurement

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
(the copy under `corpus_staging/gold_index` is format 3 and no longer
opens); Vibrato dictionary at
`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

Hints off:

```
<paste the hints-off output here, all lines>
```

Hints on:

```
<paste the hints-on output here, all lines>
```

## 3. Verdict

Design §8 clause 1: oracle@3 tolerant hints on must be at least 1,898 of
2,163, 55 spans over top-1's 1,843. Measured: **T of 2,163, T − 1,843 = D
spans.** The gate **passes / fails** (delete one).

<If it passes:> Commit 4 (the application and UI, plan Task 6) proceeds.
<If it fails:> Commit 4 does not land. `k_best`, `with_readings` and
`score --readings` stay as the measurement instrument, `alternatives` stays
empty on the wire, and Phase 2 ships no UI, as master plan §7 allows.

Context beside the gate, none of it thresholded: strict oracle@3 hints on,
the hints-off pair, and the sentence-level count are in the outputs above.
The oracle@∞ ceiling the design was built on (design §2.2) was 1,951
tolerant hints on, so oracle@3 reached (T − 1,843) of the 108 reachable spans.
```

Replace every `<...>` marker with the real text before committing; the file must contain no angle-bracket placeholders.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md
git commit -m "docs(engine-v2): record the Phase 2 oracle@3 measurement and the gate verdict"
```

---

### Task 5: Gate check

This task has no code. Read the verdict in the handoff's §3.

- [ ] If **pass**: proceed to Task 6, then Task 7.
- [ ] If **fail**: skip Task 6. Proceed to Task 7 and, in its master plan edits, record under §7 Phase 2 that the gate was measured and not met, with the number, and that the phase shipped the measurement only.

---

### Task 6: Application and UI (only on pass)

**Files:**
- Modify: `src-tauri/src/parse.rs:104-116`
- Modify: `src/types.ts:41-43`
- Modify: `src/main.ts` — `segmentAt` (703–706), `display` (1193–1209), the `keydown` listener (1053–1057), the `contextmenu` listener (486–489); new declarations next to `lastResult` (698)
- Modify: `src/main.test.ts` — six tests appended inside the first `describe` block (before its closing `});`, which follows the test at line 208–215)

**Interfaces:**
- Consumes: `ParseResult.alternatives` on the `parse-result` event (Task 2); `MenuEntry`, `openContextMenu` from `src/context-menu.ts` unchanged; `renderSentence(result, mode)` from `src/render/sentence.ts` unchanged.
- Produces: nothing downstream.

- [ ] **Step 1: Write the failing frontend tests**

In `src/main.test.ts`, inside the first `describe('the event-driven render path', ...)`, after the test `a parse-error event leaves the previous result on screen` (ends near line 215) and before the describe's closing, add:

```ts
  // Phase 2 design §6: readings 2..k ride on the same event as `alternatives`.
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
  const press = (key: string) =>
    document.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts -t "reading"`
Expected: the six new tests fail — no `Reading` rows, `]` does nothing, the popover shows `東京都`'s entry.

- [ ] **Step 3: `types.ts`**

In `src/types.ts`, change `ParseResult` to:

```ts
export interface ParseResult {
  segments: Segment[];
  /** Readings 2..k in cost order, each a full cover like `segments`. */
  alternatives: Segment[][];
}
```

Run `npx tsc --noEmit`. Expected: errors only in `main.test.ts` payloads that lack `alternatives` are **not** reported (they are untyped `unknown` payloads); errors in `src/history.test.ts` or `src/render/*.test.ts` that build a `ParseResult` literal without `alternatives` are fixed by adding `alternatives: []` to each literal. `main.ts` compiles unchanged because it only reads `segments`.

- [ ] **Step 4: `main.ts`**

After `let lastResult: ParseResult | null = null;` (line 698) add:

```ts
/**
 * Which of `lastResult`'s readings is on screen: 0 is `segments`, n ≥ 1 is
 * `alternatives[n - 1]` (Phase 2 design §6). `display` resets it whenever a
 * different result arrives; a re-render of the same result — a furigana
 * change — keeps it.
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

In `display` (lines 1193–1209), after `closePopover();` and before `lastResult = result;` add:

```ts
  // A different result starts at its top reading; the same result re-rendered
  // (a furigana change, a reading switch) keeps the one on screen.
  if (result !== lastResult) readingIndex = 0;
```

and change the render line to:

```ts
  const sentence = renderSentence(
    { segments: displayedSegments(), alternatives: [] },
    currentFuriganaMode,
  );
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
  // (`history-step`, Rust-side); this is the finer step inside one parse.
  if (e.key === ']') showReading(readingIndex + 1);
  if (e.key === '[') showReading(readingIndex - 1);
});
```

`display` still resets `panes.scrollTop` on a reading switch, as it does on a furigana change today; that is accepted, not an oversight.

- [ ] **Step 5: `parse.rs`**

In `src-tauri/src/parse.rs`, before `run_worker` add:

```rust
/// Readings the app asks the engine for, top-1 included: Phase 2 design §1
/// fixes it at three. The webview shows `segments` and offers the two
/// `alternatives` from its right-click menu.
const READINGS: usize = 3;
```

and change `&ParseOptions::default(),` inside `run_worker` (line 111) to `&ParseOptions::with_readings(READINGS),`.

- [ ] **Step 6: Run the tests**

Run: `npm test` and `npx tsc --noEmit` and `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all pass, the six new tests included.

- [ ] **Step 7: Commit**

```bash
git add src/types.ts src/main.ts src/main.test.ts src-tauri/src/parse.rs
git commit -m "feat(app): offer the engine's second and third readings

The worker asks for three readings; the main window renders the chosen
one, lists the others as 'Reading n of 3' radio rows in the right-click
menu, and steps through them with ] and [. Hover resolves against the
reading on screen; a new parse starts at reading 1."
```

Also add `alternatives: []` to any other test file literal `tsc` flagged in Step 3 and include those files in the commit.

---

### Task 7: Master plan edits and handoff completion

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — §4.1 (after the table at lines 252–256), §4.3 (paragraph at 330–334), §5.2 (paragraph at 560–562), §7 Phase 2 (lines 967–977), §10 decision log (append rows)
- Modify: `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md`

**Interfaces:** none.

- [ ] **Step 1: §7 Phase 2**

Replace lines 969–977 (from `**In:**` through `not a failure to be worked around.`) with:

```markdown
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
```

If Task 5 was a **fail**, append one more paragraph:

```markdown
**Measured 2026-09-02 and not met:** oracle@3 tolerant hints on reached
T of 2,163, D spans over top-1, against the 55 required. Phase 2 shipped the
measurement only; see the Phase 2 handoff §3.
```

with `T` and `D` replaced by the handoff's numbers.

- [ ] **Step 2: §4.1**

After the sentence ending `...so that is why §7 keeps a DP under all three.` (line 262), add:

```markdown
**Route taken at Phase 2: own k-shortest-path over the Phase 1 DP.** The
other two routes enumerate tokenizer morphemes over IPADIC, and since Phase
1a's design §6.1 hints supply boundaries only, there is no tokenizer lattice
in the engine to take an N-best from. The lattice module doc had promised the
shape — swap the single winner for a bounded list — and that is what landed
(Phase 2 design §2.3, §3).
```

- [ ] **Step 3: §4.3**

After the bullet ending `...instead of guessing on their behalf.` (line 334), add:

```markdown
  The escape hatch has a measured limit: hints on, 63 gold spans in the Phase
  0 corpus are suppressed by `is_allowed` itself — they exist as candidates
  and no path may use them — and k-best over the constrained lattice reaches
  none of them. Phase 2 kept alternates on the constrained lattice and
  recorded the 63 as the constraint's price (Phase 2 design §2.3).
```

- [ ] **Step 4: §5.2**

Replace lines 560–562 (the `**Oracle@k**` paragraph) with:

```markdown
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
```

- [ ] **Step 5: §10 decision log**

Append these rows at the end of the table:

```markdown
| Measure the oracle@∞ ceiling before building the k-best DP | oracle@k needs the DP; the ceiling needs none and bounds it for every k. Had it been small, Phase 2 would have shipped nothing without a line of engine code (Phase 2 design §2) |
| Phase 2 alternates come from the constrained lattice only | One DP, one oracle number, k = 1 provably unchanged; the 63 `is_allowed`-suppressed spans are the constraint's price, the judgment Phase 1a §7.5 already made (Phase 2 design §2.3) |
| Phase 2's gate is tolerant, +55 spans, hints on | 92 of the 196 reachable strict spans are compounds the corpus splits; strict would reward re-splitting them. Half the tolerant headroom is the line below which a cost-model pass plausibly does as well without a UI (Phase 2 design §8) |
| Reading identity is the matched span set; dominated readings are dropped | Same-span entries and unmatched splits collapse into one reading; a reading that only un-matches a cheaper reading's word is a worse copy, not an alternate (Phase 2 design §3.2, §3.2a) |
```

- [ ] **Step 6: Finish the handoff**

Append to `docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md`:

```markdown
## 4. What landed after the gate

<On pass:> Commit 4: `src-tauri/src/parse.rs` asks for three readings;
`src/types.ts` carries `alternatives`; `src/main.ts` renders the chosen
reading, lists the others as `Reading n of 3` radio rows in the right-click
menu, steps through them with `]` and `[`, and resolves hover against the
reading on screen. Six Vitest rows pin it.
<On fail:> Nothing. The wire field is empty and the menu is unchanged.

## 5. Standing gates at the final commit

Run at `<hash>`: `cargo test -p jparser` (<n> passed), the same with
`--features mecab` (<n> passed), both clippy invocations clean, `cargo check
-p jparser --no-default-features` clean, `cargo fmt -p jparser -- --check`
clean, `npm test` (<n> passed), `cargo test --manifest-path
src-tauri/Cargo.toml` (<n> passed). Eight cells byte-identical at
`--readings 1`.

## 6. Open items

- `RAW_FACTOR` in `lattice.rs` is a fixed over-provision for the dominance
  filter; a top-1 with more than nine matched spans can push a genuine
  alternate priced above +29 out of the raw list. Upgrade path: dominance
  pruning inside the DP lists (design §3.2a).
- `display` resets the sentence pane's scroll on a reading switch, as it
  does on a furigana change. Harmless on short clipboard text; a long parse
  loses its place.
- The 9 gold spans with no candidate at all (`嫌な`, `完成する`, `見ないで`
  and kin) and the 3 with the span but not the entry (`あり` wanting
  1296400) are lexicon or index findings for a later phase, not k-best ones.
```

Replace every `<...>` marker with real text and numbers from the final gate run; the committed file must contain none.

- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md docs/superpowers/2026-09-02-engine-v2-phase2-handoff.md
git commit -m "docs(engine-v2): close Phase 2; record the route, the gate and the ceiling in the master plan"
```

---

## Self-review

**Spec coverage.** §1 scope and invariant → Global Constraints, Tasks 1–3 eight-cells steps. §2 ceiling → Task 7 §5.2 edit. §2.3 constrained lattice → Task 1 relaxes only `is_allowed` candidates; §4.3 edit in Task 7. §3.1 shape and signature → Task 1 Step 3. §3.2 key → `extend_key`, tests `two_entries_at_one_span_are_one_reading`, `an_unmatched_run_yields_no_fragment_variants`. §3.2a dominance → `undominated`, `RAW_FACTOR`, tests `a_reading_that_only_unmatches_a_word_is_dropped`, `a_reading_that_leaves_an_unmatchable_character_unmatched_is_kept`, engine test `three_readings_come_out_in_cost_order_with_dominated_ones_dropped`. §3.3 rank 0 → `beats`/`accepts`/`insert` ordering, eight-cells steps, `asking_for_readings_never_moves_the_top_reading_on_the_corpus`. §4 API → Task 2. §5 harness → Task 3. §6 UI → Task 6. §7 staging → Tasks 1, 2, 3, 6 in order with the gate at Tasks 4–5. §8 done-when → Task 4 Step 2 and Task 7. §9 tests → each named test appears above. §11 master plan edits → Task 7 Steps 1–5.

**Type consistency.** `k_best(chars: &[char], by_start: &[Vec<Candidate>], bounds: Option<&dyn Boundaries>, k: usize) -> Vec<Vec<Step>>` in Task 1 is what Task 2's `parse` calls. `ParseOptions::with_readings(usize)` and `ParseResult.alternatives: Vec<Vec<Segment>>` in Task 2 are what Task 3 (`score::run`, `accumulate`) and Task 6 (`types.ts`, `parse.rs`) use. `Oracle { readings, span_hits, tolerant_hits, sentence_hits }` in Task 3 matches the CLI test's JSON string and the render test's lines. `readings()`, `displayedSegments()`, `showReading()`, `readingMenuEntries()` in Task 6 are used only within `main.ts`.

**Placeholders.** The handoff templates in Tasks 4 and 7 carry angle-bracket markers for numbers that do not exist until the measurement runs; both tasks instruct that none may remain in the committed file.
