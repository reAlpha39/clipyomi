# Engine v2 Phase 2c — one index walk per position Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `lex::candidates` walk the key FST once per start position instead of once per `(start, end)` span pair, so candidate generation is linear in input length rather than `n^2.6`, while producing a byte-identical candidate set.

**Architecture:** `Index::prefixes_of_with` is already a forward FST walk that dies where the index has no transition; `candidates` re-runs it from each `start` for every `end` and throws away every prefix hit but one. Task 1 extracts that walk as a resumable `Walk` on `Index` and reimplements `prefixes_of_with` on top of it, changing no behaviour. Task 2 restructures `candidates` around one `Walk` per start: chains are reindexed by where the *stem* ends rather than where the span ends, candidates are collected as the walk finds them and emitted in the span-end-descending order the old loop produced, and the key `String` is built only for records that actually hit. No span bound and no constant is introduced — the walk's dead end is the bound.

**Tech Stack:** Rust 2021 (`jparser` crate: `fst` 0.4, `memmap2`, serde, clap 4; `insta` in tests), Tauri 2 backend in `src-tauri`, TypeScript + Vite frontend tested with Vitest.

**Spec:** `docs/superpowers/specs/2026-09-09-engine-v2-phase2c-design.md` — read it first; this plan argues from it. Its companion finding, with the measurements, is `docs/superpowers/2026-09-09-candidate-generation-blowup.md`.

## Global Constraints

- **The candidate set is byte-identical.** After Task 2, `jparser-cli score` at `--readings 1` must print exactly Phase 1b-i handoff §2's eight cells: hints off `0.7272 (1573/2163)` strict, `0.7975 (1725/2163)` tolerant, `0.9075 (1108/1221)` entry top-1, `75` merges; hints on `0.8031 (1737/2163)`, `0.8521 (1843/2163)`, `0.9063 (1170/1291)`, `53` merges. And at `--readings 3`, hints on, the Phase 2b figures: `oracle@3 strict: 0.8632 (1867 / 2163)`, `oracle@3 tolerant: 0.8821 (1908 / 2163)`, `oracle@3 sentences: 0.1533 (23 / 150)`. The order of `alternatives` is a function of the candidate set, so the oracle figures are part of the gate, not context beside it.
- **Standing gates**, before every commit that touches `crates/jparser`: `cargo test -p jparser`, `cargo test -p jparser --features mecab`, `cargo clippy -p jparser --all-targets -- -D warnings`, `cargo clippy -p jparser --all-targets --features mecab -- -D warnings`, `cargo check -p jparser --no-default-features`, `cargo fmt -p jparser -- --check`. Measured baselines at `9945ec8`: 290 passed; 316 passed and 1 ignored; everything else clean. `npm test` (279) and `cargo test --manifest-path src-tauri/Cargo.toml` (149) are unaffected by this phase but must still pass before the final commit.
- **No new dependency, no new source file, no index format change.** `Index`'s only new public item is `walk` and the `Walk` type it returns.
- **`prefixes_of_with` keeps its signature, its return type, its `key_chars` semantics and its early return.** It has callers in `crates/jparser/src/bin/jparser-cli/main.rs:300`, `crates/jparser/tests/index_roundtrip.rs` (7 call sites) and `crates/jparser/tests/generations_torn_build.rs:113`. None of those files is edited by this plan.
- **Nothing in `cost.rs`, `lattice.rs`, `assemble.rs`, `table.rs`, `is_allowed`, `ParseOptions`, or the wire shape changes.**
- **Every new Rust file-level comment states provenance** the way `lex.rs` lines 1–5 do. This plan creates no new source files, so no new header is owed.
- **Docs and commit messages are written in normal prose**, not the session's terse style.

### Measuring

The gold index for the eight cells and the oracle figures:

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase2c"
mkdir -p "$S"
./target/release/jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$S/gold_index"
export DIC="$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"
```

```bash
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --hints "$DIC"
./target/release/jparser-cli score "$S/gold_index" crates/jparser/tests/fixtures/gold.jsonl --readings 3 --hints "$DIC"
```

The **full** index, for the timing table. The installed `dict/gen-1` is format 3 and will not open; build a current one from the `JMdict_e.gz` already on disk (this reads the user's source directory and writes only into `$S`):

```bash
./target/release/jparser-cli ensure-dictionary "$S/full_root" --source-dir "$APPDATA/dev.jparser.clipyomi/source"
# prints: generation: gen-1 / entries: 218449
```

The timing inputs are prefixes of a realistic paste — gold-corpus sentences concatenated to 10,000 characters:

```bash
python - <<'PY'
import json, os
S = os.path.expandvars(r"%LOCALAPPDATA%\Temp\claude\clipyomi-phase2c")
seen, texts = set(), []
for line in open("crates/jparser/tests/fixtures/gold.jsonl", encoding="utf-8"):
    if line.strip():
        t = json.loads(line)["text"]
        if t not in seen:
            seen.add(t); texts.append(t)
buf = ""
i = 0
while len(buf) < 10000:
    buf += texts[i % len(texts)]; i += 1
buf = buf[:10000]
for n in (500, 1000, 2000, 5000, 10000):
    open(os.path.join(S, f"paste{n}.txt"), "w", encoding="utf-8").write(buf[:n])
PY
```

```bash
TIMEFORMAT=%R
for n in 500 1000 2000 5000 10000; do
  T=$(cat "$S/paste$n.txt")
  printf "%5d chars: " "$n"
  { time ./target/release/jparser-cli parse "$S/full_root/gen-1" "$T" > /dev/null; } 2>&1 | tr -d '\n'
  echo
done
```

Pass the text with `$(cat ...)`, not a `python -c print`, which fails on this console's cp932 encoding. 29,104 bytes of argument passes fine.

**Today's numbers, for the handoff's before column** (release, full index, hints off, k = 1): 500 → 0.763 s, 1,000 → 4.284 s, 2,000 → 27.197 s, 10,000 → did not finish in 600 s. Ratios per doubling 2.2, 3.6, 5.6, 6.3.

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `crates/jparser/src/index/load.rs` | new `Walk<'a>` type with `push`, `records`, `records_after`; new `Index::walk`; `prefixes_of_with` reimplemented on it | 1 |
| `crates/jparser/tests/index_roundtrip.rs` | four new tests for `Walk` | 1 |
| `crates/jparser/src/engine/lex.rs` | `candidates` restructured; `chains_by_stem_end`; collect-and-emit; module doc note | 2 |
| `crates/jparser/tests/engine_v2.rs` | the long-input canary | 2 |
| `docs/superpowers/2026-09-09-engine-v2-phase2c-handoff.md` | created | 3 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | §7 new Phase 2c entry; §5.2 sentence | 3 |
| `docs/superpowers/2026-09-09-candidate-generation-blowup.md` | status line pointing at the handoff | 3 |

---

### Task 1: `Index::walk`, and `prefixes_of_with` reimplemented on it

**Files:**
- Modify: `crates/jparser/src/index/load.rs` — `prefixes_of_with` at 151–187; new items after it, before `records_at` at 189
- Modify: `crates/jparser/tests/index_roundtrip.rs` — four tests appended

**Interfaces:**
- Consumes: `Index`'s private `fst: Fst<Mmap>` and `records_at(offset) -> Result<Vec<StoredRecord>, IndexError>` (same module, so private access is fine); `fst::raw::Node`, which is `Copy` — the crate's own doc says "Nodes are very cheap to construct. Notably, they satisfy the `Copy` trait."
- Produces: `pub struct Walk<'a>` with `push(&mut self, char, fn(char) -> char) -> bool`, `records(&self) -> Result<Option<Vec<StoredRecord>>, IndexError>`, `records_after(&self, &str, fn(char) -> char) -> Result<Option<Vec<StoredRecord>>, IndexError>`; and `Index::walk(&self) -> Walk<'_>`. Task 2 calls exactly these.

- [ ] **Step 1: Write the failing tests**

In `crates/jparser/tests/index_roundtrip.rs`, append at the end of the file:

```rust
/// Phase 2c: `Walk` is the walk `prefixes_of_with` performs, exposed so one
/// start position can be walked once instead of re-walked per span end.
#[test]
fn a_walk_finds_the_key_that_ends_where_it_stands() {
    let dir = tmpdir("walk-exact");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(walk.records().expect("records").is_none(), "no key ends at the root");
    for ch in "言う".chars() {
        assert!(walk.push(ch, fold), "言う is in the fixture");
    }
    let records = walk.records().expect("records").expect("言う ends here");
    assert!(records.iter().any(|r| r.surface == "言う"));
}

#[test]
fn a_walk_that_leaves_the_index_is_spent() {
    let dir = tmpdir("walk-dead");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(!walk.push('z', fold), "no fixture key starts with z");
    // Spent, and it stays spent: a caller enumerating spans from one start
    // stops here rather than testing longer ones, which is the whole point.
    assert!(!walk.push('言', fold));
    assert!(walk.records().expect("records").is_none());
}

#[test]
fn records_after_reaches_a_key_the_walk_itself_has_not() {
    let dir = tmpdir("walk-after");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(walk.push('言', fold));
    // 言 alone is not the key; 言う is. `records_after` asks about the
    // extension without committing the walk to it -- which is how a
    // deinflected key (stem plus the class's remove-suffix) is tested.
    let records = walk
        .records_after("う", fold)
        .expect("records_after")
        .expect("言 + う is a key");
    assert!(records.iter().any(|r| r.surface == "言う"));
    assert!(
        walk.records_after("zz", fold).expect("records_after").is_none(),
        "an extension that leaves the index yields nothing"
    );
}

#[test]
fn records_after_does_not_move_the_walk_it_was_called_on() {
    let dir = tmpdir("walk-immutable");
    build(&dir);
    let index = Index::open(&dir).unwrap();

    let mut walk = index.walk();
    assert!(walk.push('言', fold));
    let _ = walk.records_after("う", fold).expect("records_after");
    // Still at 言, so pushing う still arrives at 言う.
    assert!(walk.push('う', fold));
    let records = walk.records().expect("records").expect("言う ends here");
    assert!(records.iter().any(|r| r.surface == "言う"));
}
```

These use the file's existing helpers, verified present: `tmpdir(name)`, `build(dir)`, `Index::open(&dir).unwrap()`, and `fold` imported at the top from `jparser::engine::kana::fold`. The fixture is `fixtures/jmdict_mini.xml`, which contains `言う` (readings `いう`, `ゆう`), `は` and `高い`, so `言う` is a real key and `z` starts none.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --test index_roundtrip 2>&1 | tail -20`
Expected: compile error, `no method named 'walk' found for struct 'Index'`.

- [ ] **Step 3: Add `Walk` and `Index::walk`, and reimplement `prefixes_of_with`**

In `crates/jparser/src/index/load.rs`, replace the whole of `prefixes_of_with` (lines 151–187, from `    pub fn prefixes_of_with(` through its closing `    }`) with:

```rust
    /// Every dictionary key that is a prefix of `text`, folded by `fold`,
    /// each tagged with its length in characters of `text`.
    ///
    /// One forward walk, stopping where the index has no transition: a
    /// caller that wants the walk itself, to test several keys sharing a
    /// prefix without repeating it, wants [`Index::walk`] instead. This is
    /// that walk with every key it passes collected.
    pub fn prefixes_of_with(
        &self,
        text: &str,
        fold: fn(char) -> char,
    ) -> Result<Vec<PrefixHit>, IndexError> {
        let mut walk = self.walk();
        let mut hits = Vec::new();
        if let Some(records) = walk.records()? {
            hits.push(PrefixHit { key_chars: 0, records });
        }
        for (consumed, ch) in text.chars().enumerate() {
            if !walk.push(ch, fold) {
                return Ok(hits);
            }
            if let Some(records) = walk.records()? {
                hits.push(PrefixHit {
                    key_chars: consumed + 1,
                    records,
                });
            }
        }
        Ok(hits)
    }

    /// A walk at the FST root, before any character.
    pub fn walk(&self) -> Walk<'_> {
        Walk {
            index: self,
            node: self.fst.root(),
            output: 0,
            alive: true,
        }
    }
```

Then, immediately after the `impl Index` block that contains `entry_count` (its closing `}` is at line 209 before this task's edits), add:

```rust
/// A position in the key FST: the node some folded prefix of the input
/// reached, and the transducer output accumulated on the way to it.
///
/// Exists so one input position can be walked once rather than re-walked
/// for every span that starts there. `lex::candidates` used to build a key
/// per `(start, end)` pair and hand each to [`Index::prefixes_of_with`],
/// which walks from the root every time and discards every prefix it passes
/// but one; that made candidate generation superlinear in input length
/// (Phase 2c design §2).
pub struct Walk<'a> {
    index: &'a Index,
    node: fst::raw::Node<'a>,
    output: u64,
    /// False once a `push` found no transition. A spent walk stays spent:
    /// no key has the prefix it stands on, so no longer prefix has it
    /// either, and a caller enumerating spans from one start stops.
    alive: bool,
}

impl<'a> Walk<'a> {
    /// Advance over `ch`, folded by `fold`. `false` when the index has no
    /// such transition, after which this walk is spent.
    pub fn push(&mut self, ch: char, fold: fn(char) -> char) -> bool {
        if !self.alive {
            return false;
        }
        let mut buf = [0u8; 4];
        for &byte in fold(ch).encode_utf8(&mut buf).as_bytes() {
            let Some(i) = self.node.find_input(byte) else {
                self.alive = false;
                return false;
            };
            let transition = self.node.transition(i);
            self.output += transition.out.value();
            self.node = self.index.fst.node(transition.addr);
        }
        true
    }

    /// The records of the key ending exactly where this walk stands, or
    /// `None` when no key ends here.
    pub fn records(&self) -> Result<Option<Vec<StoredRecord>>, IndexError> {
        if !self.alive || !self.node.is_final() {
            return Ok(None);
        }
        let offset = self.output + self.node.final_output().value();
        Ok(Some(self.index.records_at(offset)?))
    }

    /// The records of the key that ends `suffix` past where this walk
    /// stands, leaving this walk where it is. `None` when the extension
    /// leaves the index, or when no key ends there.
    ///
    /// This is what lets a deinflected key be tested without repeating the
    /// stem: the walk holds the stem, and `suffix` is the conjugation
    /// class's remove-suffix.
    pub fn records_after(
        &self,
        suffix: &str,
        fold: fn(char) -> char,
    ) -> Result<Option<Vec<StoredRecord>>, IndexError> {
        // `fst::raw::Node` is `Copy`, so this is a cheap independent
        // position rather than a second traversal from the root.
        let mut probe = Walk {
            index: self.index,
            node: self.node,
            output: self.output,
            alive: self.alive,
        };
        for ch in suffix.chars() {
            if !probe.push(ch, fold) {
                return Ok(None);
            }
        }
        probe.records()
    }
}
```

`fst::raw::Node` needs naming; `use fst::raw::Fst;` is already at the top of the file, so either widen it to `use fst::raw::{Fst, Node};` and write `Node<'a>`, or leave the import alone and write `fst::raw::Node<'a>` as above. Either is fine — pick one and be consistent.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p jparser --test index_roundtrip`
Expected: all pass — the four new ones and every pre-existing `prefixes_of_with` assertion in that file, which is the regression net for the reimplementation.

- [ ] **Step 5: Run the standing gates**

```bash
cargo fmt -p jparser
cargo test -p jparser
cargo test -p jparser --features mecab
cargo clippy -p jparser --all-targets -- -D warnings
cargo clippy -p jparser --all-targets --features mecab -- -D warnings
cargo check -p jparser --no-default-features
cargo fmt -p jparser -- --check
```

Expected: 294 passed (290 + 4); with `--features mecab`, 320 passed and 1 ignored; everything else clean.

- [ ] **Step 6: Eight cells**

Build the release binary and the gold index per "Measuring", then run the two `--readings 1` commands. Expected: byte-identical to the eight cells in Global Constraints. `candidates` is untouched by this task, so a difference here means `prefixes_of_with` was not reimplemented faithfully — stop and fix it rather than proceeding.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/src/index/load.rs crates/jparser/tests/index_roundtrip.rs
git commit -m "feat(engine-v2): expose the key FST's forward walk as a resumable Walk

prefixes_of_with already walks the FST one folded character at a time and
stops where there is no transition. That walk is now a type: Index::walk
returns a position, push advances it, records reads the key ending there,
and records_after tests an extension without moving it -- which is what
lets a stem be walked once and several suffixes tried against it.

prefixes_of_with is reimplemented on top of it and keeps its signature,
its key_chars semantics and its early return, so its existing tests are
the regression net. No behaviour changes and no caller moves."
```

---

### Task 2: `candidates` walks once per start

**Files:**
- Modify: `crates/jparser/src/engine/lex.rs` — `candidates` at 78–148; module doc at 7–14
- Modify: `crates/jparser/tests/engine_v2.rs` — the canary, appended

**Interfaces:**
- Consumes: Task 1's `Index::walk`, `Walk::push`, `Walk::records`, `Walk::records_after`; existing `MAX_CHAIN`, `allows_chain`, `dedup_shallowest`, `CLASSES`, `ChainHit { consumed, steps, stem_class }`, `Table::chains_ending_at`, `kana::fold`.
- Produces: nothing new. `candidates`'s signature and its output are unchanged.

- [ ] **Step 1: Write the failing canary and the order test**

In `crates/jparser/tests/engine_v2.rs`, append:

```rust
/// Phase 2c: candidate generation used to be superlinear in input length —
/// it walked the index from each start for every span end, so a
/// 10,000-character paste, which is the app's own clipboard ceiling, did not
/// finish in ten minutes. This asserts nothing about the clock: a wall-time
/// threshold is flaky on shared hardware. It is a canary. If the blowup
/// returns, this test stops finishing and the suite hangs, which is the
/// loud signal whose absence let the defect survive four phases.
#[test]
fn a_long_input_parses_in_reasonable_time() {
    let dir = scratch("long-input");
    build_v2(&dir);
    let index = open_v2(&dir);
    let table = jparser::engine::table::Table::new();

    // Roughly 4,000 characters of the crate's own corpus sentences, which is
    // far past where the old shape became unusable and well short of the
    // clipboard ceiling.
    let mut text = String::new();
    while text.chars().count() < 4000 {
        for sentence in sentences() {
            text.push_str(sentence);
            if text.chars().count() >= 4000 {
                break;
            }
        }
    }

    let result = jparser::parse(
        &index,
        &table,
        &text,
        &jparser::ParseOptions::default(),
        None,
    )
    .expect("parse");

    // The cover is contiguous and complete, as for any input.
    let mut at = 0;
    for segment in &result.segments {
        assert_eq!(segment.start, at);
        at += segment.len;
    }
    assert_eq!(at, text.chars().count());
}
```

`crates/jparser/src/engine/lex.rs` has no `mod tests` — verified — so do not create one. The zero-length guard gets its own integration test instead, appended to `crates/jparser/tests/engine_v2.rs`, which already reaches `candidates` through its own `cands` helper:

```rust
/// Phase 2c design §3.2: the old `(start + 1..=len)` loop bound made a
/// zero-length span unreachable. Indexed by stem end, the identity chain hit
/// (`consumed: 0`) at `stem_end == start` implies `end == start`, so the
/// restructure has to exclude it explicitly. A candidate of length zero
/// would break every consumer of a contiguous cover.
#[test]
fn no_candidate_has_zero_length() {
    let dir = scratch("zero-length");
    build_v2(&dir);
    for text in sentences() {
        for bucket in cands(&dir, text) {
            for candidate in bucket {
                assert!(
                    candidate.len >= 1,
                    "zero-length candidate at {} in {text}",
                    candidate.start
                );
            }
        }
    }
}
```

`cands(dir, text)` rebuilds the index per call, so if this is noticeably slow, hoist the build: `build_v2` once, then call `jparser::engine::lex::candidates` directly the way `cands` does. Either is acceptable; say which you did.

- [ ] **Step 2: Run them to verify the canary is slow or the order test fails**

Run: `cargo test -p jparser --test engine_v2 a_long_input_parses_in_reasonable_time -- --nocapture`
Expected before the fix: it completes, but slowly — at 4,000 characters against the small fixture index the old shape is tolerable, which is exactly why the corpus never caught this. Record the wall time from `cargo test`'s output as the before figure; it is the canary's baseline, not a gate.

- [ ] **Step 3: Restructure `candidates`**

In `crates/jparser/src/engine/lex.rs`, replace the whole of `candidates` (lines 78–148, from `pub fn candidates(` through the `Ok(out)` and its closing `}`) with:

```rust
pub fn candidates(
    index: &Index,
    table: &Table,
    chars: &[char],
) -> Result<Vec<Vec<Candidate>>, IndexError> {
    let len = chars.len();
    let mut out: Vec<Vec<Candidate>> = vec![Vec::new(); len];

    // `chains_ending_at(chars, end, ..)` depends only on where a span ends,
    // never on where it starts, so it is computed once per end position and
    // shared by every start that reaches it instead of once per (start, end)
    // pair.
    let chains_by_end: Vec<Vec<ChainHit>> = (1..=len)
        .map(|end| table.chains_ending_at(chars, end, MAX_CHAIN))
        .collect();

    // The same chains, indexed by where the *stem* ends rather than where
    // the span does. One walk from `start` arrives at stem ends ascending,
    // so that is the order the lookups need; each entry carries the span end
    // it implies and its position in its own `chains_by_end` bucket, which
    // the emission order below needs (Phase 2c design §3.2, §3.3).
    let mut chains_by_stem_end: Vec<Vec<(usize, usize, &ChainHit)>> = vec![Vec::new(); len + 1];
    for (i, bucket) in chains_by_end.iter().enumerate() {
        let end = i + 1;
        for (chain_index, hit) in bucket.iter().enumerate() {
            chains_by_stem_end[end - hit.consumed].push((end, chain_index, hit));
        }
    }

    let mut class_cache: HashMap<u32, Vec<ClassId>> = HashMap::new();

    for start in 0..len {
        // Collected as the walk finds them -- stem-end ascending -- and
        // emitted longest span first below, which is the order the old
        // `(start + 1..=len).rev()` loop pushed in and `dedup_shallowest`
        // depends on.
        let mut found: Vec<(usize, usize, Candidate)> = Vec::new();
        let mut walk = index.walk();
        let mut stem_end = start;

        loop {
            for &(end, chain_index, hit) in &chains_by_stem_end[stem_end] {
                // The old loop ran `end` from `start + 1`, so a zero-length
                // span was unreachable; indexed by stem end, the identity
                // chain at `stem_end == start` implies `end == start`
                // (design §3.2).
                if end <= start {
                    continue;
                }
                let remove = match hit.stem_class {
                    Some(class) => CLASSES[class as usize].remove,
                    None => "",
                };
                // Ask the index whether anything is there before building a
                // key to ask with: the walk already holds the stem.
                let found_records = if remove.is_empty() {
                    walk.records()?
                } else {
                    walk.records_after(remove, kana::fold)?
                };
                let Some(records) = found_records else {
                    continue;
                };
                // Built only for a span that matched, because `exact`
                // compares against it.
                let key: String = chars[start..stem_end]
                    .iter()
                    .copied()
                    .chain(remove.chars())
                    .collect();
                for record in &records {
                    if !allows_chain(index, &mut class_cache, record, hit.stem_class)? {
                        continue;
                    }
                    found.push((
                        end,
                        chain_index,
                        Candidate {
                            start,
                            len: end - start,
                            entry_id: record.entry_id,
                            flags: WordFlags(record.flags),
                            steps: hit.steps.clone(),
                            exact: record.surface == key,
                            headword: record.surface.clone(),
                        },
                    ));
                }
            }

            // The walk's own dead end bounds the span: no key has this
            // prefix, so none has a longer one, and nothing further from
            // this start can match. That is the bound the old shape lacked.
            if stem_end == len || !walk.push(chars[stem_end], kana::fold) {
                break;
            }
            stem_end += 1;
        }

        // Longest span first, and within one span end the order that end's
        // own chain bucket had. `sort_by` is stable, so record order within
        // one chain hit survives too -- which is what makes this reproduce
        // the old push order term for term (design §3.3).
        found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        out[start] = dedup_shallowest(found.into_iter().map(|(_, _, c)| c).collect());
    }

    Ok(out)
}
```

Then amend the module doc (lines 7–14) by appending one paragraph to it:

```rust
//! Generation walks the key FST once per start position, not once per span:
//! the walk holds the stem, each conjugation class's remove-suffix is tried
//! against it without repeating the walk, and the walk's own dead end is
//! what bounds how far a span can reach. Before Phase 2c this enumerated
//! every `(start, end)` pair and built a key per pair, which was superlinear
//! in input length.
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p jparser` then `cargo test -p jparser --features mecab`
Expected: 296 passed (294 after Task 1, plus the canary and the zero-length test) and 322 passed with 1 ignored. Every pre-existing `lex` and `engine_v2` assertion must pass unchanged — those are the correctness net for this restructure. If any fails, the restructure is wrong; do not adjust the test.

- [ ] **Step 5: Standing gates**

Run the seven commands from Task 1 Step 5. Expected: all clean.

- [ ] **Step 6: The gate — byte-identical output**

Rebuild the release binary and run all three `score` commands from "Measuring". Expected, exactly:

- both `--readings 1` runs byte-identical to the eight cells in Global Constraints;
- the `--readings 3 --hints` run's oracle lines exactly `oracle@3 strict: 0.8632 (1867 / 2163)`, `oracle@3 tolerant: 0.8821 (1908 / 2163)`, `oracle@3 sentences: 0.1533 (23 / 150)`.

If any figure differs, the candidate set moved. Stop and diagnose — the design's §9 fallback exists for this outcome and is a decision for the controller, not a thing to work around by adjusting a test or a threshold.

- [ ] **Step 7: The timing table**

Build the full index and the paste files per "Measuring", then run the timing loop. Record all five figures. Design §5 clause 2 requires 10,000 characters under 30 seconds; clause 3 requires the curve to be linear to within a factor of two across the five lengths (compare each row's time-per-character against the 500-character row's).

- [ ] **Step 8: Commit**

```bash
git add crates/jparser/src/engine/lex.rs crates/jparser/tests/engine_v2.rs
git commit -m "fix(engine-v2): walk the index once per position instead of once per span

candidates enumerated every (start, end) pair, built a stem String of up
to the input's length for each, and handed it to prefixes_of_with -- which
walks from the root every time and discards every prefix it passes but
one. Generation was therefore superlinear in input length: a
10,000-character paste, the app's own clipboard ceiling, did not finish in
ten minutes.

It now walks once per start. Chains are reindexed by where the stem ends
rather than where the span does, since a walk arrives at stem ends
ascending; each class's remove-suffix is tested against the standing walk
without repeating it; and the key String is built only for records that
actually hit. No span bound and no constant is introduced -- the walk's
dead end is the bound, and it is tighter than any constant could be.

Candidates are collected as the walk finds them and emitted span-end
descending, stably, which reproduces the old push order term for term.
That is what lets the gate be byte-identical output rather than merely
equivalent output: the eight cells and Phase 2b's oracle@3 figures are
unchanged in both hint states."
```

---

### Task 3: Handoff and master plan

**Files:**
- Create: `docs/superpowers/2026-09-09-engine-v2-phase2c-handoff.md`
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — §5.2, and §7 between the Phase 2 entry and `### Phase 3`
- Modify: `docs/superpowers/2026-09-09-candidate-generation-blowup.md` — the `**Status:**` line at the top

**Interfaces:** none.

- [ ] **Step 1: Write the handoff**

Create `docs/superpowers/2026-09-09-engine-v2-phase2c-handoff.md`:

```markdown
# Engine v2 Phase 2c — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2c-design.md`](specs/2026-09-09-engine-v2-phase2c-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2c.md`](plans/2026-09-09-engine-v2-phase2c.md)
**Finding:** [`2026-09-09-candidate-generation-blowup.md`](2026-09-09-candidate-generation-blowup.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Two commits. `Index::walk` and the `Walk` type, with `prefixes_of_with`
reimplemented on it and unchanged in signature and behaviour; then
`candidates` restructured to walk once per start position.

## 2. The gate — byte-identical output

(paste the two `--readings 1` outputs and the `--readings 3 --hints` output
verbatim)

All eight cells and all three `oracle@3` figures match the recorded values.
The candidate set did not move.

## 3. What it bought

Release build, full 218,449-entry index, hints off, `jparser-cli parse`,
k = 1:

| input chars | before | after |
| --- | --- | --- |
| 500 | 0.763 s | X.XXX s |
| 1,000 | 4.284 s | X.XXX s |
| 2,000 | 27.197 s | X.XXX s |
| 5,000 | — | X.XXX s |
| 10,000 | did not finish in 600 s | X.XXX s |

Time per character, against the 500-character row: (fill in) — design §5
clause 3 asked for linear to within a factor of two.

## 4. Standing gates

Run at `<hash>`: `cargo test -p jparser` (N passed), the same with
`--features mecab` (N passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` (279 passed), `cargo test --manifest-path
src-tauri/Cargo.toml` (149 passed).

## 5. Open items

- **The canary asserts no timing.** `a_long_input_parses_in_reasonable_time`
  parses ~4,000 characters and checks the cover; if the blowup returns the
  suite stops finishing rather than failing an assertion. That is deliberate
  — a wall-clock threshold is flaky — but it means a 2× regression would
  pass unnoticed. A timing figure in the harness, compared against a
  recorded baseline, is the stronger version and is not built.
- **10,000 characters still takes seconds, not milliseconds.** The blowup is
  gone; the constant factor is whatever it is. Nothing here measured where
  the remaining time goes — entry assembly and JSON formatting both grow
  with the result, and neither was profiled.
- **`MAX_CHAIN`'s and `MAX_UNMATCHED_RUN`'s adequacy is still unmeasured**
  against the full corpus, as the Phase 2 handoff's "three size/shape
  assumptions" bullet recorded. This phase did not touch either.
```

- [ ] **Step 2: Master plan §7**

Immediately before `### Phase 3 — Extra linguistic layers`, insert:

```markdown
### Phase 2c — Candidate generation walks the index once per position — **COMPLETE**

Not a quality change: the candidate set is byte-identical, and so are the
eight cells and Phase 2b's oracle@3 figures. `lex::candidates` enumerated
every `(start, end)` span pair and built a key per pair, making generation
superlinear in input length — a 10,000-character paste, the app's own
clipboard ceiling, did not finish in ten minutes. It now walks the key FST
once per start position, with the walk's own dead end bounding the span. See
the Phase 2c handoff for the before-and-after timings.
```

- [ ] **Step 3: Master plan §5.2**

Find the paragraph introducing §5.2's metrics and append one sentence to it:

```markdown
Every metric here is measured at corpus sentence lengths, and nothing in the
harness measures a clipboard-length input — which is why a superlinearity
that made the app's own 10,000-character ceiling unusable survived four
phases (Phase 2c).
```

- [ ] **Step 4: The finding document's status line**

In `docs/superpowers/2026-09-09-candidate-generation-blowup.md`, change

```markdown
**Status:** diagnosed and reproduced, fix shape proven, not fixed
```

to

```markdown
**Status:** fixed in Phase 2c — see its handoff. The restructure there
supersedes §4's experimental bound and §5's four ways of sourcing a
constant; neither was needed.
```

- [ ] **Step 5: Commit**

```bash
git add docs/
git commit -m "docs(engine-v2): close Phase 2c; record the before-and-after and the missing gate"
```

---

## Self-review

**Spec coverage.** §1 scope and the "explicitly not a bound" clause → Task 2 Step 3's code introduces no constant; Global Constraints forbid the format change. §2 rationale → the module doc paragraph and the commit body. §3.1 `Walk` API → Task 1 Step 3, with the exact three methods §3.1 names. §3.2 reindex, the walk loop, the `end <= start` guard, the key built only on a hit → Task 2 Step 3, each with the design's own reasoning in a comment. §3.3 order preservation → the stable `sort_by` and its comment; the gate in §5 clause 1 is what verifies it. §4 complexity → Task 2 Step 7's per-character comparison. §5 clauses 1–5 → Task 2 Steps 4–7 and Task 1 Steps 4–6. §6 tests → Task 1 Step 1's four `Walk` tests, Task 2 Step 1's canary and zero-length assertion; the "conjugated form still resolves" and "longest-span-first" cases §6 names are covered by the pre-existing `lex` and `engine_v2` suites, which Task 2 Step 4 requires to pass unchanged — stated there rather than duplicated as new tests. §7 budget → the file map. §8 master plan edits → Task 3 Steps 2 and 3. §9 fallback → Task 2 Step 6 routes a gate failure to the controller rather than to a workaround. §10 decision log → no task contradicts a row.

**Placeholder scan.** The handoff template carries `X.XXX`, `N` and `<hash>` deliberately — they are Task 2 Step 7's and Task 3's measured output and cannot be known earlier; each is named with the step that fills it. Task 1's and Task 2's test helpers were verified against the real files before this plan was committed: `index_roundtrip.rs` has `tmpdir`/`build`/`fold` and opens with `Index::open(&dir).unwrap()`; its fixture contains `言う`; `lex.rs` has no `mod tests`, so the zero-length test is an integration test using `engine_v2.rs`'s own `scratch`/`build_v2`/`cands`/`sentences` helpers. No other placeholders.

**Type consistency.** `Walk<'a>` is declared in Task 1 with `push(&mut self, char, fn(char) -> char) -> bool`, `records(&self) -> Result<Option<Vec<StoredRecord>>, IndexError>` and `records_after(&self, &str, fn(char) -> char) -> Result<Option<Vec<StoredRecord>>, IndexError>`, and Task 2 calls exactly those three with those shapes. `chains_by_stem_end: Vec<Vec<(usize, usize, &ChainHit)>>` is built and consumed with the same triple order `(end, chain_index, hit)` in both places. `found: Vec<(usize, usize, Candidate)>` sorts on `.0` then `.1`, matching that order. `CLASSES[class as usize].remove` is a `&'static str`, so `remove.is_empty()` and `remove.chars()` are both available.
