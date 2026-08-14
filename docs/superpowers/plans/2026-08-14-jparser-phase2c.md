# JParser Phase 2C — MeCab Boundary Hints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `jparser`'s `BoundaryHints` trait its first real implementation, backed by the Vibrato tokenizer, so the segmentation DP can be told where not to split.

**Architecture:** A new `crates/jparser/src/hints.rs`, gated behind an optional `mecab` feature. `VibratoTokenizer::load(path)` reads a compiled dictionary once; `hints(text)` tokenizes and returns `BoundaryFlags`, which implements `BoundaryHints` over two `Vec<bool>` indexed by char position. The derivation is a literal port of ta-old: for each token whose IPADIC feature field 7 (the reading) is present and not `*`, mark the token's **interior** positions only. `jparser-cli parse` gains `--hints <dict>`.

**Tech Stack:** Rust 2021, **MSRV 1.85 (raised from 1.75 by this phase)**, `vibrato 0.5` with `default-features = false`. No new dependency beyond `vibrato`.

**Reference:** `docs/superpowers/specs/2026-08-14-jparser-phase2c-design.md` (authoritative), with `docs/superpowers/2026-08-13-jparser-phase2b-handoff.md` for the invariants this phase must not break. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **License GPL v2.** Every new source file gets the standard header comment, copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **MSRV 1.85 after Task 1.** The gate is `cargo +1.85 check --workspace`. Do not run `cargo +1.75` anything after Task 1 — the floor has moved.
- **`crates/jparser`'s library stays pure.** `vibrato` is `optional` and `mecab` is **not** in `default`. The gate is `cargo check -p jparser --no-default-features --all-targets` plus `cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"` returning **0**.
- **No new dependency beyond `vibrato`.** No `tempfile`, no mock framework, no zstd/xz/tar.
- **No test touches the network**, and no test loads a real 7.7 MB dictionary. Tests build a tiny dictionary in memory (Task 2).
- **Errors are explicit:** no `unwrap()`, `expect()`, or `unreachable!()` in library code outside `#[cfg(test)]`. Never swallow an error without a comment naming the reason.
- **No magic numbers, no bare literals:** the feature field index `7` is a named constant citing ta-old's line.
- **Naming, frozen:** `VibratoTokenizer`, `BoundaryFlags`, `HintsError`, `load`, `hints`, `READING_FIELD`.
- **File size** 200–400 lines typical, **800 hard maximum including `#[cfg(test)] mod tests`**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never `cargo fmt`, never `cargo fmt -p jparser`** — it reformats `conjugation.rs`, `kana.rs`, and `romaji.rs`, which this phase must leave untouched. `conjugation.rs` is deliberately not rustfmt-clean; "fixing" it is a defect. After formatting run `git diff --stat` and confirm only intended files moved.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` clean at the end of every task.
- **`crates/jparser/src/segment.rs` is at 778/800 lines and must not be edited.**

**Invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format; a published `gen-N` is immutable; directory knowledge lives only in `generations.rs` and `ensure_dictionary`; the staging filename stays process-unique; a `.partial` file is never resolved. This phase touches none of them.

---

## Resolved facts — do not re-derive these

Measured 2026-08-14 against the live registry and a compile probe. Spec §9 records them.

| Fact | Value |
|---|---|
| `vibrato` latest | 0.5.2 |
| License | MIT OR Apache-2.0 (GPL-v2 compatible) |
| Default features | `["train"]` → pulls `rucrf`. Use `default-features = false` |
| vibrato 0.5.0 | uses `bincode 2.0.0-rc.2`, builds on 1.75 |
| vibrato 0.5.1 / 0.5.2 | use `bincode 2.0.1`, **require rustc 1.85** |
| `jparser` already uses | `bincode 1.3` — the tree will carry both majors, which cargo permits |

**API verified to compile** — do not consult docs, use these:

```rust
SystemDictionaryBuilder::from_readers(lexicon, matrix, char_def, unk_def) -> Result<Dictionary>
Tokenizer::new(dict);  tokenizer.new_worker();
worker.reset_sentence(text);  worker.tokenize();  worker.num_tokens();
worker.token(i).surface()    -> &str
worker.token(i).feature()    -> &str
worker.token(i).range_char() -> Range<usize>   // CHAR units — no conversion needed
```

**The minimal in-memory dictionary, verified to build and tokenize.** Copy verbatim:

```text
LEX:    東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー
        都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト
MATRIX: 1 1
        0 0 0
CHAR:   DEFAULT 0 1 0
        KANJI 0 0 2
        0x4E00..0x9FFF KANJI
UNK:    DEFAULT,0,0,5000,記号,*,*,*,*,*,*
        KANJI,0,0,5000,名詞,一般,*,*,*,*,*
```

Tokenizing `"東京都"` with it produces, measured:

```text
tok 0: surface="東京" range_char=0..2 feature="名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー"
tok 1: surface="都"   range_char=2..3 feature="名詞,接尾,地域,*,*,*,都,ト,ト"
```

Field index 7 of `feature` is the reading (`トウキョウ`), confirming IPADIC layout.

**The seam being filled**, `crates/jparser/src/segment.rs:37-42`, re-exported at `lib.rs:47` as `jparser::BoundaryHints`:

```rust
pub trait BoundaryHints {
    fn bad_start(&self, pos: usize) -> bool;
    fn bad_end(&self, pos: usize) -> bool;
}
```

**Worked example of the derivation** on `"東京都"`: `東京` has char range `0..2`, length 2, so `i` runs over `0..1` → `bad_end[0] = true`, `bad_start[1] = true`. `都` has length 1, so its loop is empty and it marks nothing. Position 2 stays free in both vectors.

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | *(modified)* `rust-version = "1.85"` |
| `crates/jparser/Cargo.toml` | *(modified)* unpin `clap`; add optional `vibrato` + `mecab` feature |
| `.github/workflows/ci.yml` | *(modified)* MSRV matrix → `1.85`; add `vibrato` to the purity grep |
| `docs/superpowers/specs/2026-08-13-jparser-phase2b-design.md` | *(modified)* amend §9/§10's 1.75 claims |
| `docs/superpowers/2026-08-13-jparser-phase2b-handoff.md` | *(modified)* amend its MSRV invariant |
| `crates/jparser/src/hints.rs` | *(new)* `READING_FIELD`, `HintsError`, `BoundaryFlags`, `VibratoTokenizer`, and the test dictionary |
| `crates/jparser/src/lib.rs` | *(modified)* `pub mod hints;` gated on `mecab` |
| `crates/jparser/src/bin/jparser-cli.rs` | *(modified)* `--hints` on `parse` |
| `crates/jparser/tests/cli_generations.rs` | *(modified)* CLI flag test |

`hints.rs` is projected at ~330 lines post-rustfmt including its test module — under the cap.

---

## Task 1: Raise the MSRV floor to 1.85

**Files:**
- Modify: `Cargo.toml`, `crates/jparser/Cargo.toml`, `.github/workflows/ci.yml`, `docs/superpowers/specs/2026-08-13-jparser-phase2b-design.md`, `docs/superpowers/2026-08-13-jparser-phase2b-handoff.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a workspace whose floor is 1.85, with `clap` unpinned. Task 2 depends on this — `vibrato 0.5.2` cannot build below it.

This task adds no Rust code. It is separate because it changes a project-wide constraint that every later task inherits, and because a reviewer should be able to reject the floor change without rejecting the feature.

- [ ] **Step 1: Install the 1.85 toolchain and confirm the current floor still holds**

```bash
rustup toolchain install 1.85 --profile minimal
cargo +1.75 check --workspace --quiet && echo "1.75 currently passes"
```

Expected: both succeed. The second confirms you are starting from a green floor, so any later failure is yours.

- [ ] **Step 2: Raise the workspace floor**

In the root `Cargo.toml`, change `rust-version = "1.75"` to `rust-version = "1.85"` under `[workspace.package]`. Leave `resolver = "2"` alone — it is unrelated, and changing it is out of scope.

- [ ] **Step 3: Unpin `clap`**

In `crates/jparser/Cargo.toml`, replace the pinned entry and its comment:

```toml
# Pinned exactly, not `4`: clap_builder moved to clap_lex 1.x partway through the
# 4.5 line (4.5.61 already requires it), and clap_lex 1.1 needs edition2024, i.e.
# Rust 1.85 — above the workspace MSRV of 1.75. A range does not help, because
# `resolver = "2"` ignores rust-version when choosing versions and would take the
# newest match. 4.5.51 is the version compile-verified against the 1.75 toolchain.
# Constraining clap_lex directly does not work either: cargo permits the 0.7 and
# 1.x majors side by side, so clap_builder keeps its own copy.
clap = { version = "=4.5.51", features = ["derive"] }
```

with:

```toml
clap = { version = "4", features = ["derive"] }
```

The entire comment goes — the constraint it documents no longer exists.

- [ ] **Step 4: Re-resolve and verify**

```bash
cargo update
cargo +1.85 check --workspace --quiet
cargo test --workspace 2>&1 | grep -E "^test result|^error"
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: `clap` moves back up to 4.6.x, `cargo +1.85 check --workspace` succeeds, all tests still pass (292 passing + 1 ignored before this phase adds any), clippy clean. If `cargo +1.85 check --workspace` fails, **stop and report** — the floor is wrong, not the code.

- [ ] **Step 5: Update CI**

In `.github/workflows/ci.yml`, change the MSRV matrix from `rust: ["1.75"]` to `rust: ["1.85"]`, and update the comment above it so the clap anecdote reads as history rather than a live constraint. Add `vibrato` to the purity job's grep now, so Task 2 does not have to touch CI:

```yaml
          if cargo tree -p jparser --no-default-features \
             | grep -E "jmdict-source|ureq|flate2|vibrato"; then
```

- [ ] **Step 6: Amend the Phase 2B spec and handoff**

Both assert MSRV 1.75 as a live fact and will now be wrong. Use the house style already in those files — a `> **Amended after implementation (2026-08-14).**` blockquote at the site, saying what changed and why.

In `docs/superpowers/specs/2026-08-13-jparser-phase2b-design.md`, add a note to §9's MSRV bullet and §10's item 1 recording that Phase 2C raised the floor to 1.85, that the `clap` pin was retired with it, and that the cause was `vibrato`'s transitive `bincode 2.0.1`.

In `docs/superpowers/2026-08-13-jparser-phase2b-handoff.md`, amend the "MSRV is 1.75 and is now compile-verified" invariant and the `clap` bullet under "Known issues carried forward" the same way.

Keep both short — three or four sentences each. Do not restate this plan.

- [ ] **Step 7: Format check and commit**

```bash
git diff --stat
git add Cargo.toml Cargo.lock crates/jparser/Cargo.toml .github/workflows/ci.yml docs/superpowers
git commit -m "chore: raise the workspace MSRV to 1.85 and unpin clap"
```

`git diff --stat` must show only those files. No `.rs` file should appear.

---

## Task 2: The `mecab` feature and the in-memory test dictionary

**Files:**
- Modify: `crates/jparser/Cargo.toml`, `crates/jparser/src/lib.rs`
- Create: `crates/jparser/src/hints.rs`

**Interfaces:**
- Consumes: MSRV 1.85 (Task 1).
- Produces:
  - a `mecab` feature enabling an optional `vibrato` dependency
  - `crates/jparser/src/hints.rs` with the GPL header and its test-dictionary constants
  - `pub(crate) fn test_dictionary() -> vibrato::Dictionary` under `#[cfg(test)]`, which Tasks 3–5 use

The deliverable is: a test can build a dictionary and tokenize with it, and the library still builds without any of it.

- [ ] **Step 1: Add the optional dependency and the feature**

In `crates/jparser/Cargo.toml`, add to `[features]`:

```toml
# Vibrato-backed BoundaryHints. Off by default: the parser library must build
# without a tokenizer, and `cargo check -p jparser --no-default-features` is the
# gate that proves it.
mecab = ["dep:vibrato"]
```

and to `[dependencies]`:

```toml
# `default-features = false` drops vibrato's default `train` feature and the
# `rucrf` training stack with it — nothing here trains a model.
vibrato = { version = "0.5", default-features = false, optional = true }
```

- [ ] **Step 2: Verify the purity gate before writing any code**

```bash
cargo check -p jparser --no-default-features --all-targets --quiet
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
cargo tree -p jparser --features mecab | grep -E "^jparser|vibrato|rucrf"
```

Expected: the check succeeds; the grep count is **0**; the third shows `vibrato v0.5.2` and **no `rucrf`**. If `rucrf` appears, `default-features = false` is missing or misspelled.

- [ ] **Step 3: Write the failing test**

Create `crates/jparser/src/hints.rs` with the GPL v2 header copied verbatim from `crates/jparser/src/index/mod.rs:1-6`, then the module doc and **only** this:

```rust
//! Boundary hints derived from Vibrato tokenization.
//!
//! `segment.rs` weights `BoundaryHints` into the segmentation DP but has had no
//! implementation since Phase 1B. This module is that implementation: it
//! tokenizes with Vibrato and marks the *interior* positions of each token, so
//! the DP is discouraged from splitting inside a word the tokenizer recognized.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_dictionary_tokenizes_a_known_sentence() {
        let dict = test_dictionary();
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("東京都");
        worker.tokenize();

        assert_eq!(worker.num_tokens(), 2, "expected 東京 + 都");
        assert_eq!(worker.token(0).surface(), "東京");
        assert_eq!(worker.token(0).range_char(), 0..2);
        assert_eq!(worker.token(1).surface(), "都");
        assert_eq!(worker.token(1).range_char(), 2..3);
        // Field 7 is IPADIC's reading. The derivation's guard depends on it.
        let reading = worker.token(0).feature().split(',').nth(READING_FIELD);
        assert_eq!(reading, Some("トウキョウ"));
    }
}
```

- [ ] **Step 4: Run the test to verify it fails**

Run: `cargo test -p jparser --features mecab --lib hints`

Expected: FAIL to compile — `cannot find function 'test_dictionary'`, `cannot find value 'READING_FIELD'`, and `file not found for module 'hints'` until Step 5 wires it. That is the intended RED.

- [ ] **Step 5: Wire the module and implement the fixture**

In `crates/jparser/src/lib.rs`, beside the other `mod` declarations (they are alphabetical — put it after `pub mod conjugation;`):

```rust
#[cfg(feature = "mecab")]
pub mod hints;
```

In `hints.rs`, insert above the test module:

```rust
/// Index of the reading in an IPADIC feature string. ta-old skips a token whose
/// reading is `*` or absent — "If katakana is '*' or does not exist, not real
/// word, so don't penalize" (`ta-old/exe/util/Dictionary.cpp:1115-1121`).
const READING_FIELD: usize = 7;
```

and inside `mod tests`, above the test:

```rust
    /// A two-entry IPADIC-shaped dictionary, built in memory.
    ///
    /// Deliberately not a fixture file: the real dictionary is 7.7 MB and the
    /// tests must not touch the network or the filesystem. These four readers
    /// are the same inputs vibrato's own `compile` binary takes.
    pub(crate) fn test_dictionary() -> vibrato::Dictionary {
        const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                           都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        vibrato::SystemDictionaryBuilder::from_readers(
            LEX.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("the built-in test dictionary must build")
    }
```

**Note the line continuations.** `\n\` followed by indentation continues a Rust string literal without embedding the indentation, because the backslash eats the leading whitespace of the next line. Verify the built dictionary tokenizes as expected rather than assuming the literal is well formed — that is what Step 3's test does.

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p jparser --features mecab --lib hints`
Expected: PASS, 1 test.

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.
Run: `cargo check -p jparser --no-default-features --all-targets --quiet` — succeeds.

- [ ] **Step 7: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/hints.rs
git diff --stat
git add crates/jparser/Cargo.toml crates/jparser/src/hints.rs crates/jparser/src/lib.rs Cargo.lock
git commit -m "feat: add the mecab feature and an in-memory vibrato test dictionary"
```

`git diff --stat` must not show `conjugation.rs`, `kana.rs`, or `romaji.rs`. If it does, you ran rustfmt on a crate root — `git checkout --` them.

---

## Task 3: `BoundaryFlags` and the derivation

**Files:**
- Modify: `crates/jparser/src/hints.rs`

**Interfaces:**
- Consumes: `READING_FIELD`, `test_dictionary` (Task 2); `crate::BoundaryHints` (existing).
- Produces:
  - `pub struct BoundaryFlags` implementing `BoundaryHints`
  - `fn flags_from_worker(worker: &vibrato::tokenizer::worker::Worker, char_len: usize) -> BoundaryFlags`

This is the phase's core logic. Task 4 loads a dictionary from disk; Task 5 wires the CLI.

- [ ] **Step 1: Write the failing tests**

Add to `hints.rs`'s `mod tests`:

```rust
    /// Build flags for `text` using the built-in test dictionary.
    fn flags_for(text: &str) -> BoundaryFlags {
        let tokenizer = vibrato::Tokenizer::new(test_dictionary());
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        flags_from_worker(&worker, text.chars().count())
    }

    /// 東京 spans chars 0..2, so only its interior boundary is marked: a word
    /// may still end at 1 and start at 0, but not the reverse.
    #[test]
    fn a_multi_char_token_marks_only_its_interior() {
        let f = flags_for("東京都");

        assert!(f.bad_end(0), "0 is interior to 東京");
        assert!(f.bad_start(1), "1 is interior to 東京");

        assert!(!f.bad_start(0), "a word may start at the token's first char");
        assert!(!f.bad_end(1), "a word may end at the token's last char");
    }

    /// 都 is one char, so its loop body never runs. An off-by-one here would
    /// silently penalize every single-char token in the language.
    #[test]
    fn a_single_char_token_marks_nothing() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(2), "都 must not mark its own start");
        assert!(!f.bad_end(2), "都 must not mark its own end");
    }

    #[test]
    fn empty_input_yields_empty_flags() {
        let f = flags_for("");
        assert!(!f.bad_start(0));
        assert!(!f.bad_end(0));
    }

    /// `segment.rs` queries `m.start + m.len - 1`, which can exceed what the
    /// tokenizer saw. A panic here would crash the DP.
    #[test]
    fn out_of_range_positions_are_false_not_a_panic() {
        let f = flags_for("東京都");
        assert!(!f.bad_start(999));
        assert!(!f.bad_end(999));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --features mecab --lib hints`
Expected: FAIL to compile — `cannot find type 'BoundaryFlags'`, `cannot find function 'flags_from_worker'`.

- [ ] **Step 3: Implement the derivation**

Insert into `hints.rs` above the test module:

```rust
use crate::BoundaryHints;

/// Positions where a word should not begin or end, derived from tokenization.
///
/// Indexed by **char** position, matching [`BoundaryHints`] and `segment.rs`.
/// Vibrato reports char ranges directly, so nothing here converts from bytes.
pub struct BoundaryFlags {
    bad_start: Vec<bool>,
    bad_end: Vec<bool>,
}

impl BoundaryHints for BoundaryFlags {
    fn bad_start(&self, pos: usize) -> bool {
        // Out of range is not an error: `segment.rs` queries positions derived
        // from match lengths, which may exceed the tokenized text.
        self.bad_start.get(pos).copied().unwrap_or(false)
    }

    fn bad_end(&self, pos: usize) -> bool {
        self.bad_end.get(pos).copied().unwrap_or(false)
    }
}

/// Derive flags from a tokenized worker. Port of `ta-old/exe/util/
/// Dictionary.cpp:1115-1126`.
///
/// For each token, the **interior** positions are marked: a word should not end
/// before the token's last char, nor start after its first. The token's own
/// boundaries stay free — the hint says "do not split inside this," not "split
/// here." A single-char token therefore marks nothing.
///
/// A token whose reading ([`READING_FIELD`]) is absent or `*` is skipped
/// entirely. That is ta-old's unknown-word guard: penalizing splits inside a
/// word the tokenizer only guessed at would be worse than staying silent.
///
/// ta-old carries a second guard — a fuzzy re-match of the token against the
/// source, commented "I don't trust mecab all that much" — which is
/// deliberately **not** ported. It existed because ta-old drove MeCab through a
/// text pipe and had to re-find each token by scanning. Vibrato returns char
/// ranges into the exact string it was handed, so misalignment cannot occur and
/// the branch would be untestable.
fn flags_from_worker(
    worker: &vibrato::tokenizer::worker::Worker,
    char_len: usize,
) -> BoundaryFlags {
    let mut bad_start = vec![false; char_len];
    let mut bad_end = vec![false; char_len];

    for i in 0..worker.num_tokens() {
        let token = worker.token(i);
        let reading = token.feature().split(',').nth(READING_FIELD);
        if !matches!(reading, Some(r) if !r.is_empty() && r != "*") {
            continue;
        }

        let range = token.range_char();
        for pos in range.start..range.end.saturating_sub(1) {
            if pos < char_len {
                bad_end[pos] = true;
            }
            if pos + 1 < char_len {
                bad_start[pos + 1] = true;
            }
        }
    }

    BoundaryFlags { bad_start, bad_end }
}
```

**If `vibrato::tokenizer::worker::Worker` is not the right path** under 0.5.2, find the actual one (`cargo doc -p vibrato --no-deps`, or read `~/.cargo/registry/src/index.crates.io-*/vibrato-0.5.2/src/`) and use it — then **report the difference**. The API calls themselves were verified; this type path was not.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --features mecab --lib hints`
Expected: PASS, 5 tests.

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.

- [ ] **Step 5: Add the unknown-word guard test**

The guard has no test yet, because the test dictionary's two entries both carry readings. Add one whose reading is `*`:

```rust
    /// A token whose reading is `*` is a guess, and ta-old refuses to penalize
    /// splits inside a guess. Without this guard the DP would be pushed away
    /// from splitting inside anything the tokenizer failed to recognize.
    #[test]
    fn a_token_without_a_reading_marks_nothing() {
        const LEX: &str = "謎語,0,0,5000,名詞,一般,*,*,*,*,*,*,*\n";
        const MATRIX: &str = "1 1\n0 0 0\n";
        const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
        const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                           KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

        let dict = vibrato::SystemDictionaryBuilder::from_readers(
            LEX.as_bytes(),
            MATRIX.as_bytes(),
            CHAR.as_bytes(),
            UNK.as_bytes(),
        )
        .expect("dictionary");
        let tokenizer = vibrato::Tokenizer::new(dict);
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence("謎語");
        worker.tokenize();
        let f = flags_from_worker(&worker, 2);

        assert!(!f.bad_end(0), "a reading-less token must not be penalized");
        assert!(!f.bad_start(1), "a reading-less token must not be penalized");
    }
```

Note this lexicon's field 7 is `*`. Run the tests: **PASS, 6 tests.** If this test passes even with the guard removed, the lexicon's field 7 is not `*` — print `token.feature()` and count the commas before adjusting.

- [ ] **Step 6: Prove the guard and the interior rule are load-bearing**

Temporarily delete the `if !matches!(reading, ...) { continue; }` block and re-run: `a_token_without_a_reading_marks_nothing` must fail. Restore it.

Then temporarily change `range.end.saturating_sub(1)` to `range.end` and re-run: `a_multi_char_token_marks_only_its_interior` must fail on its `!f.bad_end(1)` assertion, and `a_single_char_token_marks_nothing` must fail too. Restore, re-run to green, and record both outputs.

A derivation whose guards cannot be caught doing nothing is not evidence, and both of these are one-character mistakes away from silently wrong.

- [ ] **Step 7: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/hints.rs
git diff --stat
git add crates/jparser/src/hints.rs
git commit -m "feat: derive boundary flags from vibrato tokens"
```

---

## Task 4: `VibratoTokenizer` and `HintsError`

**Files:**
- Modify: `crates/jparser/src/hints.rs`

**Interfaces:**
- Consumes: `BoundaryFlags`, `flags_from_worker` (Task 3).
- Produces:
  - `pub enum HintsError { Io, Dictionary }`
  - `pub struct VibratoTokenizer` with `load(&Path) -> Result<Self, HintsError>` and `hints(&self, &str) -> BoundaryFlags`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-hints-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn an_absent_dictionary_is_an_io_error() {
        let dir = scratch("load-absent");
        let err = VibratoTokenizer::load(&dir.join("system.dic"))
            .err()
            .expect("must fail");
        assert!(matches!(err, HintsError::Io(_)), "got {err:?}");
    }

    #[test]
    fn a_file_that_is_not_a_dictionary_is_a_dictionary_error() {
        let dir = scratch("load-garbage");
        let path = dir.join("system.dic");
        std::fs::write(&path, b"this is not a compiled dictionary").expect("write");

        let err = VibratoTokenizer::load(&path).err().expect("must fail");
        assert!(matches!(err, HintsError::Dictionary(_)), "got {err:?}");
    }

    /// The error must be actionable: it names the file it could not load.
    #[test]
    fn the_io_error_renders_usefully() {
        let dir = scratch("load-render");
        let err = VibratoTokenizer::load(&dir.join("system.dic"))
            .err()
            .expect("must fail");
        assert!(!err.to_string().is_empty());
    }
```

`load` is tested against a real filesystem rather than a mock, matching how Phase 2B tested `open_local`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --features mecab --lib hints`
Expected: FAIL to compile — `cannot find type 'VibratoTokenizer'`, `cannot find type 'HintsError'`.

- [ ] **Step 3: Implement the loader**

Insert into `hints.rs` above the test module:

```rust
use std::path::Path;

/// A loaded Vibrato dictionary, ready to tokenize.
///
/// Loading is expensive and reading the dictionary is not, so the two are
/// separate: a caller loads once and calls [`VibratoTokenizer::hints`] per text.
pub struct VibratoTokenizer {
    tokenizer: vibrato::Tokenizer,
}

impl VibratoTokenizer {
    /// Load an **uncompressed** compiled Vibrato dictionary from `path`.
    ///
    /// The distributed archive is `.tar.xz` containing a zstd-compressed
    /// `system.dic`; extracting it is deliberately out of scope (spec §5),
    /// which is what keeps `vibrato` this phase's only new dependency.
    pub fn load(path: &Path) -> Result<Self, HintsError> {
        let file = std::fs::File::open(path)?;
        let dict = vibrato::Dictionary::read(std::io::BufReader::new(file))
            .map_err(|e| HintsError::Dictionary(e.to_string()))?;
        Ok(Self {
            tokenizer: vibrato::Tokenizer::new(dict),
        })
    }

    /// Tokenize `text` and derive its boundary flags.
    ///
    /// A fresh worker per call: workers are mutable scratch space, and sharing
    /// one would force `&mut self` on a method that is otherwise read-only.
    pub fn hints(&self, text: &str) -> BoundaryFlags {
        let mut worker = self.tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        flags_from_worker(&worker, text.chars().count())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HintsError {
    #[error("reading the vibrato dictionary failed: {0}")]
    Io(#[from] std::io::Error),
    /// Vibrato's error, rendered. Carried as a `String` so `vibrato` does not
    /// become part of this crate's public API for anyone matching on it — the
    /// same reason `SourceError::Transport` holds a `String` rather than a
    /// `ureq` type.
    #[error("the vibrato dictionary could not be loaded: {0}")]
    Dictionary(String),
}
```

**If `vibrato::Dictionary::read` does not exist or takes a different argument** under 0.5.2, find the actual loader and use it — then **report the difference**. `SystemDictionaryBuilder::from_readers` was verified; the on-disk reader was not.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --features mecab --lib hints`
Expected: PASS, 9 tests.

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.
Run: `cargo check -p jparser --no-default-features --all-targets --quiet` — succeeds.

- [ ] **Step 5: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/hints.rs
git diff --stat
git add crates/jparser/src/hints.rs
git commit -m "feat: load a vibrato dictionary and produce hints per text"
```

---

## Task 5: The `--hints` flag and the end-to-end proof

**Files:**
- Modify: `crates/jparser/src/bin/jparser-cli.rs`, `crates/jparser/src/hints.rs`, `crates/jparser/tests/cli_generations.rs`

**Interfaces:**
- Consumes: `VibratoTokenizer`, `BoundaryFlags` (Task 4).
- Produces: `jparser-cli parse <index> <text> [--hints <dict>]`.

**This task carries the phase's only end-to-end proof.** Everything before it tests the derivation in isolation. Step 1 is the test that shows hints actually reach the DP and change its output — and per spec §10, **failure to construct that case is a stop-and-report, not a test to weaken.**

- [ ] **Step 1: Write the test that proves the derivation is not inert**

Add to `crates/jparser/src/hints.rs`'s `mod tests`:

```rust
    /// The phase's reason to exist: the derivation must actually produce flags
    /// for ordinary input. Everything else here tests individual rules, which a
    /// no-op implementation returning all-false would also satisfy.
    ///
    /// `AlwaysBad` in `segment.rs` already proves the DP *responds* to hints;
    /// this proves the hints we derive are non-empty and correctly placed.
    #[test]
    fn the_derivation_is_not_inert() {
        let text = "東京都";
        let f = flags_for(text);

        let any = (0..text.chars().count()).any(|p| f.bad_start(p) || f.bad_end(p));
        assert!(any, "the derivation produced no flags at all");

        assert!(f.bad_end(0) && f.bad_start(1), "東京's interior must be marked");
    }
```

- [ ] **Step 2: Add the CLI flag**

In `crates/jparser/src/bin/jparser-cli.rs`, extend the `Parse` variant:

```rust
    /// Parse TEXT against the index at INDEX.
    Parse {
        index: PathBuf,
        text: String,
        /// Path to an uncompressed compiled Vibrato dictionary. When given,
        /// tokenization supplies boundary hints to the segmenter.
        #[cfg(feature = "mecab")]
        #[arg(long)]
        hints: Option<PathBuf>,
    },
```

- [ ] **Step 3: Wire it into the match arm**

Replace the body at `crates/jparser/src/bin/jparser-cli.rs:263-268`. The existing comment says hints have no implementation "until Phase 5" — that is now wrong and must go.

```rust
        Command::Parse {
            index,
            text,
            #[cfg(feature = "mecab")]
            hints,
        } => {
            let table = ConjugationTable::load_embedded()?;
            let index = Index::open(&index)?;

            #[cfg(feature = "mecab")]
            let tokenizer = match &hints {
                Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
                None => None,
            };
            #[cfg(feature = "mecab")]
            let flags = tokenizer.as_ref().map(|t| t.hints(&text));
            #[cfg(feature = "mecab")]
            let hints: Option<&dyn jparser::BoundaryHints> =
                flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
            #[cfg(not(feature = "mecab"))]
            let hints: Option<&dyn jparser::BoundaryHints> = None;

            let result = jparser::parse(&index, &table, &text, &ParseOptions::default(), hints)?;
```

Leave the printing loop that follows unchanged.

A load failure propagates through `?` and exits non-zero. That is deliberate: a user who passed `--hints` asked for hints, and silently parsing without them would return a plausible result that is not what was requested.

- [ ] **Step 4: Verify the CLI compiles both ways**

```bash
cargo build -p jparser --features mecab --quiet
cargo build -p jparser --quiet
cargo check -p jparser --no-default-features --all-targets --quiet
```

Expected: all three succeed. The second is the default build with `mecab` off — the flag must simply not exist there, not fail to compile.

- [ ] **Step 5: Write the CLI test**

Add to `crates/jparser/tests/cli_generations.rs`:

```rust
/// The flag must be rejected when the dictionary is missing, rather than
/// silently parsing without hints.
#[test]
fn parse_rejects_an_absent_hints_dictionary() {
    let dir = scratch("cli-hints-absent");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    cli(&["build-index", "idx", "mini.xml"], &dir);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["parse", "idx", "東京", "--hints", "nope.dic"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "a missing dictionary must be rejected");
}
```

**Confirm the `build-index` subcommand name and argument order** against the existing tests in that file before using it — 2A's CLI is kebab-case and this plan has not verified this particular invocation. If the file's other tests build an index differently, copy their form. This test needs the `mecab` feature to be meaningful; if the default `cargo test -p jparser` build has `mecab` off, gate the test with `#[cfg(feature = "mecab")]` and note that it runs only under `cargo test -p jparser --features mecab`.

- [ ] **Step 6: Run the full gate**

```bash
cargo test --workspace
cargo test -p jparser --features mecab
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p jparser --no-default-features --all-targets --quiet
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
cargo +1.85 check --workspace --quiet
cargo llvm-cov -p jparser --summary-only --fail-under-lines 80
```

Expected: all pass, the grep count is **0**. Note `cargo llvm-cov -p jparser` does not enable `mecab`, so `hints.rs` is excluded from that run; report the number rather than adjusting the threshold. If coverage of the crate falls below 80, **stop and report** — do not add tests solely to inflate it.

- [ ] **Step 7: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/hints.rs
rustfmt --edition 2021 crates/jparser/src/bin/jparser-cli.rs
rustfmt --edition 2021 crates/jparser/tests/cli_generations.rs
git diff --stat
git add crates/jparser/src crates/jparser/tests
git commit -m "feat: supply boundary hints to the parser from jparser-cli"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 in-scope: load a dictionary | 4 |
| §1 in-scope: tokenize and derive flags | 3 |
| §1 in-scope: `--hints` flag | 5 |
| §1 in-scope: raise MSRV to 1.85 | 1 |
| §2 `mecab` feature, optional `vibrato`, purity gate | 2 |
| §3 the derivation, interior-only, field-7 guard | 3 |
| §3 fuzzy re-alignment guard deliberately not ported | 3 (doc comment) |
| §4 `VibratoTokenizer`, `BoundaryFlags`, `HintsError` | 3, 4 |
| §5 uncompressed dictionary only | 4 (doc comment) |
| §6 CLI shape, `None` unchanged | 5 |
| §7 MSRV consequences: clap unpin, CI, 2B doc amendments | 1 |
| §8 failure behavior table | 4, 5 |
| §9 resolved facts consumed, not re-derived | "Resolved facts" |
| §10 required assertions | 2, 3, 5 |
| §11 GPL header, purity, formatting, clippy, coverage | 1–5 |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Three steps direct the implementer to *verify and report* rather than guess — Task 3 Step 3 on the `Worker` type path, Task 4 Step 3 on `Dictionary::read`, and Task 5 Step 5 on the `build-index` invocation. Each names the exact uncertainty and how to resolve it, which is the opposite of a placeholder.

**3. Type consistency across task boundaries.** Checked:

- `READING_FIELD: usize` (Task 2) is read by `flags_from_worker` (Task 3) — matches.
- `test_dictionary() -> vibrato::Dictionary` (Task 2) is used by `flags_for` (Task 3) — matches.
- `flags_from_worker(&Worker, usize) -> BoundaryFlags` (Task 3) is called by `VibratoTokenizer::hints` (Task 4) — matches.
- `BoundaryFlags` implements `crate::BoundaryHints` (Task 3) and is coerced to `&dyn BoundaryHints` in the CLI (Task 5) — matches, and `BoundaryHints` is public at `jparser::BoundaryHints` via `lib.rs:47`.
- `HintsError` derives `thiserror::Error` and `Debug`; Task 4's tests use `{err:?}` and `to_string()` — both available.
- `VibratoTokenizer::load(&Path)` (Task 4) is called with `&PathBuf` deref in the CLI (Task 5) — matches.

**4. Residual risks a human should look at.**

- **The end-to-end proof is weaker than the spec asked for, and this is the plan's biggest deviation.** Spec §10's sixth row wants a sentence whose *segmentation* changes when hints are supplied. Task 5 Step 1 asserts only that the derivation produces correctly-placed, non-empty flags. A two-entry test dictionary cannot produce a genuinely ambiguous segmentation, and the real dictionary is out of scope. The implementer should attempt the stronger version against the CLI test's real index and **report if it cannot be built** — the phase would then have shown its derivation correct but never shown it changes an outcome.
- **Three API details are unverified**: the `Worker` type path, `Dictionary::read`, and the `build-index` CLI invocation. Each has a named fallback and a report obligation.
- **`cargo llvm-cov -p jparser` will not cover `hints.rs`** unless `--features mecab` is passed, so the phase's new code is invisible to the coverage gate as written. Consider whether CI's coverage job should gain the feature — that is a decision, not an oversight.
- **Raising the MSRV is reversible in git but not in practice.** Once dependencies resolve forward past 1.75, going back means another round of pinning.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-14-jparser-phase2c.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
