# JParser Phase 2A — Dictionary Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish dictionary indexes as immutable numbered generations so that an
interrupted or concurrent rebuild can never serve well-formed wrong data, and
give the crate a headless `ensure_dictionary` plus a CLI to drive it.

**Architecture:** A new `index/generations.rs` owns three primitives — `latest`,
`build_new`, `sweep` — over a `<root>/gen-<N>/` layout whose directories are
immutable once created. `build_new` builds into `<root>/.build-<pid>-<nanos>/`
and publishes by `fs::rename`, so a reader resolving a generation name either
gets a wholly-valid index or `ENOENT`, never a splice of two builds.
`ensure_dictionary` in `index/mod.rs` composes them behind a lazy closure, so the
~60 MB source is never obtained on the steady-state path.

**Tech Stack:** Rust 2021 (MSRV **1.75**, toolchain 1.97.1), `std::fs` only —
`fst`, `memmap2`, `bincode`, `serde`, `thiserror`, `clap` are already present and
no new dependency is added.

**Reference:** `docs/superpowers/specs/2026-08-13-jparser-phase2a-design.md`
(authoritative), with `docs/superpowers/2026-08-13-jparser-phase1a-handoff.md`
for the empirical filesystem facts and `docs/superpowers/2026-08-13-jparser-phase1b-handoff.md`
for the invariants Phase 1 leaves behind. The C++ original in `ta-old/` is
**read-only — never modify it**; it has no equivalent of this layout and is not
a reference for this phase.

## Global Constraints

- **License:** GPL v2. Every new source file gets the standard GPL v2 header
  comment, copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **MSRV is 1.75.** `Option::is_none_or` (1.82) and
  `io::ErrorKind::DirectoryNotEmpty` (1.83) are **not available**. Use
  `map_or(true, …)` and an `exists()` probe respectively. This is the single
  most likely way to break the build in this phase.
- **Crate purity:** `crates/jparser` must not depend on Tauri, any UI crate, or
  any HTTP client, and **no new dependency may be added**. No gzip, no `libc`,
  no `tempfile` — temp dirs use the existing
  `std::env::temp_dir().join(format!("jparser-test-{name}"))` +
  `let _ = std::fs::remove_dir_all(&dir);` pattern from
  `crates/jparser/tests/index_roundtrip.rs:20`.
- **Errors are explicit:** no `unwrap()` or `expect()` in library code outside
  tests. Every fallible path returns `Result` with an `IndexError` variant.
  Never silently skip data without counting it.
- **Immutability:** all public types are owned and immutable.
- **No magic numbers:** `GENERATION_PREFIX`, `BUILD_PREFIX`, and
  `DEFAULT_KEEP_GENERATIONS` are named `const`s. No string literal `"gen-"` or
  `".build-"` appears anywhere outside their definitions.
- **File size:** 200–400 lines typical, **800 hard maximum including the
  `#[cfg(test)] mod tests` block**.
- **Naming:** types `PascalCase`, functions and variables `snake_case`,
  constants `UPPER_SNAKE_CASE`. One name per concept, frozen: `latest`,
  `build_new`, `sweep`, `publish`, `generation_number`, `ensure_dictionary`,
  `GenerationExists`, `DEFAULT_KEEP_GENERATIONS`.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never
  `cargo fmt -p jparser`, and never `rustfmt` `src/lib.rs`** — both cascade into
  `conjugation.rs`, `kana.rs`, and `romaji.rs`, which this phase must leave
  alone. `conjugation.rs` is deliberately not rustfmt-clean; "fixing" it is a
  defect. After formatting, run `git diff --stat` and confirm only intended
  files moved.
- **Coverage target:** 80% line coverage on `crates/jparser`, measured by
  `cargo llvm-cov -p jparser --summary-only --fail-under-lines 80`. Phase 1B
  finished at 96.37%.
- **Clippy:** `cargo clippy -p jparser --all-targets -- -D warnings` must pass
  clean at the end of **every** task in this phase. Unlike Phase 1B, there is no
  sanctioned window of dead-code failures — each task's deliverable is consumed
  by its own tests.

**Phase 1 invariants this phase must not break** (Phase 1B handoff):

- **`INDEX_FORMAT_VERSION` is 3 and does not change here.** `EntryData`'s field
  order is `id, readings, senses` and bincode is positional, so field order is
  wire format.
- **`parse` still takes `&Index` and no parser type learns a directory path.**
  2A introduces the first code that legitimately knows about `root`; it lives in
  `generations.rs` and `ensure_dictionary` only.
- `Match::chain` empty ⇔ non-verb; `VerbTypeId` is 0-based; the DP's skip `>`
  and match `>=` tie-breaks differ deliberately. Nothing in this phase touches
  any of them — `segment.rs` sits at 778/800 and must not be edited.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/jparser/src/index/generations.rs` | *(new)* `GENERATION_PREFIX`, `BUILD_PREFIX`, `DEFAULT_KEEP_GENERATIONS`, `generation_number`, `latest`, `latest_number`, `build_new`, `publish`, `sweep` |
| `crates/jparser/src/index/mod.rs` | *(modified)* `pub mod generations;`, `IndexError::GenerationExists`, `ensure_dictionary` |
| `crates/jparser/src/index/load.rs` | *(modified)* `Index::entry_count` |
| `crates/jparser/src/bin/jparser-cli.rs` | *(modified)* `ensure-dictionary`, `gen-list`, `gen-sweep` subcommands |
| `crates/jparser/tests/generations_torn_build.rs` | *(new)* the eleven-state crash-point suite plus the hazard reproduction |
| `crates/jparser/tests/cli_generations.rs` | *(new)* CLI round trip over the three new subcommands |

`generations.rs` is projected at ~430 lines post-rustfmt (~180 impl, ~250 test)
and `index/mod.rs` grows from 119 to ~300. Both are comfortably under the cap.
`ensure_dictionary` lives in `mod.rs` rather than `generations.rs` because the
design spec §2 places it there, and because `generations.rs` owns filesystem
primitives while `ensure_dictionary` owns policy.

---

## Task 1: Generation naming and `latest`

**Files:**
- Create: `crates/jparser/src/index/generations.rs`
- Modify: `crates/jparser/src/index/mod.rs` (add `pub mod generations;`)

**Interfaces:**
- Consumes: `IndexError` from `crate::index` (`index/mod.rs`, existing).
- Produces:
  - `pub const GENERATION_PREFIX: &str = "gen-";`
  - `pub const BUILD_PREFIX: &str = ".build-";`
  - `pub const DEFAULT_KEEP_GENERATIONS: usize = 2;`
  - `fn generation_number(name: &str) -> Option<u64>` *(private)*
  - `pub(crate) fn latest_number(root: &Path) -> Result<Option<(u64, PathBuf)>, IndexError>`
  - `pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError>`

**Why `gen-01` must not parse.** The layout's whole guarantee is that a
generation name resolves to exactly one immutable directory. A permissive parse
lets a hand-created `gen-01` and a real `gen-1` both claim generation 1, which
reintroduces the ambiguity immutable names exist to remove. Design spec §3 and
Risk #2.

- [ ] **Step 1: Write the failing tests**

Create `crates/jparser/src/index/generations.rs` containing **only** the GPL
header, the module doc, the `use` block, and the test module below. The
implementation arrives in Step 3, so this file will not compile yet — that is
the intended RED.

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Immutable generation directories.
//!
//! An index is published as `<root>/gen-<N>/`, built first into
//! `<root>/.build-<pid>-<nanos>/` and moved into place with a single
//! `fs::rename`. Readers resolve the highest `N`.
//!
//! The layout exists because `Index::open` is a five-file sequence against a
//! directory *name*: `header.bin`, `entries.idx`, then three mmaps. Any scheme
//! where readers resolve a **mutable** name — rename-over-the-directory, a
//! symlink flip, or a `CURRENT` pointer file — can splice one generation's
//! `entries.idx` onto another's `entries.bin` when a reader opens mid-swap,
//! which yields well-formed wrong answers rather than an error. Atomicity of
//! the pointer swap does not help, because the open is not atomic. A `gen-N`
//! directory's contents never change after creation, so a straddling open
//! either succeeds wholly or gets `ENOENT`.

use std::path::{Path, PathBuf};

use crate::index::IndexError;

#[cfg(test)]
mod tests {
    use super::*;

    /// Per the crate's no-`tempfile` rule; mirrors `tests/index_roundtrip.rs:20`.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn mkdir(root: &Path, name: &str) {
        std::fs::create_dir_all(root.join(name)).expect("mkdir");
    }

    #[test]
    fn an_absent_root_is_not_an_error() {
        let root = std::env::temp_dir().join("jparser-test-gen-absent");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(latest(&root).expect("latest"), None);
    }

    #[test]
    fn an_empty_root_has_no_generation() {
        let root = scratch("gen-empty");
        assert_eq!(latest(&root).expect("latest"), None);
    }

    #[test]
    fn the_highest_generation_wins() {
        let root = scratch("gen-highest");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-2");
        mkdir(&root, "gen-7");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-7")));
    }

    /// Lexicographic ordering would pick gen-9. The comparison is numeric.
    #[test]
    fn generations_order_numerically_not_lexicographically() {
        let root = scratch("gen-numeric");
        mkdir(&root, "gen-9");
        mkdir(&root, "gen-10");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-10")));
    }

    #[test]
    fn malformed_generation_names_are_ignored() {
        let root = scratch("gen-malformed");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-");
        mkdir(&root, "gen-abc");
        mkdir(&root, "gen-1x");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    /// A hand-made `gen-01` must not shadow the real `gen-1`, and must not be
    /// mistaken for a *higher* generation than `gen-1` either.
    #[test]
    fn a_zero_padded_generation_is_ignored() {
        let root = scratch("gen-padded");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-01");
        mkdir(&root, "gen-007");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    #[test]
    fn build_directories_are_never_returned() {
        let root = scratch("gen-buildskip");
        mkdir(&root, ".build-123-456");
        assert_eq!(latest(&root).expect("latest"), None);
    }

    /// A *file* named like a generation is not a generation.
    #[test]
    fn a_file_named_like_a_generation_is_ignored() {
        let root = scratch("gen-file");
        std::fs::write(root.join("gen-9"), b"not a directory").expect("write");
        mkdir(&root, "gen-1");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    #[test]
    fn latest_number_reports_the_number_beside_the_path() {
        let root = scratch("gen-number");
        mkdir(&root, "gen-4");
        let (n, path) = latest_number(&root).expect("latest_number").expect("some");
        assert_eq!(n, 4);
        assert_eq!(path, root.join("gen-4"));
    }
}
```

- [ ] **Step 2: Wire the module in, then run the tests to verify they fail**

Add `pub mod generations;` to `crates/jparser/src/index/mod.rs` immediately
after the existing `pub mod load;` line. Without it the new file is not part of
the crate and the test command below matches nothing, which is a *false* green.

Run: `cargo test -p jparser --lib generations`

Expected: FAIL to compile, with `cannot find function 'latest' in this scope`
and `cannot find function 'latest_number' in this scope`. That is the intended
RED.

- [ ] **Step 3: Implement the parser and the scan**

Insert into `crates/jparser/src/index/generations.rs`, between the `use` block
and `#[cfg(test)]`:

```rust
/// Directory-name prefix for a published generation.
pub const GENERATION_PREFIX: &str = "gen-";

/// Directory-name prefix for an in-progress build. Dotted so it sorts and
/// displays as hidden, and so it can never collide with `GENERATION_PREFIX`.
pub const BUILD_PREFIX: &str = ".build-";

/// Generations retained by `sweep`. Two rather than one: `sweep` cannot delete
/// a mapped file on Windows, so a failed sweep must never be load-bearing.
/// See the design spec §7.
pub const DEFAULT_KEEP_GENERATIONS: usize = 2;

/// Parse `gen-<N>` into `N`, rejecting everything else.
///
/// Deliberately strict. `gen-01` is rejected rather than read as 1: a
/// permissive parse would let a hand-created directory shadow a real
/// generation, which is precisely the ambiguity immutable names remove.
fn generation_number(name: &str) -> Option<u64> {
    let digits = name.strip_prefix(GENERATION_PREFIX)?;
    if digits.is_empty() {
        return None;
    }
    // Rejects '+', '-', whitespace, and any non-ASCII digit `u64::from_str`
    // would otherwise accept or refuse on its own terms — checked explicitly
    // so the rule is visible rather than implied by the parser.
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Leading zeros: `gen-01` and `gen-1` would both claim generation 1.
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    digits.parse::<u64>().ok()
}

/// Highest generation in `root`, as `(number, path)`.
///
/// An absent `root` yields `Ok(None)` — that is the first-run "no dictionary
/// yet" signal, not an error.
pub(crate) fn latest_number(root: &Path) -> Result<Option<(u64, PathBuf)>, IndexError> {
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut best: Option<(u64, PathBuf)> = None;
    for entry in read {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(number) = generation_number(name) else {
            continue;
        };
        // MSRV 1.75: `Option::is_none_or` is 1.82. Do not "simplify" this.
        if best.as_ref().map_or(true, |(best, _)| number > *best) {
            best = Some((number, entry.path()));
        }
    }
    Ok(best)
}

/// Path of the highest generation in `root`, or `None` if there is none.
pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError> {
    Ok(latest_number(root)?.map(|(_, path)| path))
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --lib generations`
Expected: PASS, 9 tests.

Run: `cargo clippy -p jparser --all-targets -- -D warnings`
Expected: clean. If clippy suggests `is_none_or` on the `map_or` line, **do not
take the suggestion** — it does not compile on MSRV 1.75. Add no `#[allow]`.

- [ ] **Step 5: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/index/generations.rs
rustfmt --edition 2021 crates/jparser/src/index/mod.rs
git diff --stat
git add crates/jparser/src/index/generations.rs crates/jparser/src/index/mod.rs
git commit -m "feat: resolve the highest index generation in a root"
```

`git diff --stat` must show exactly those two files. If `conjugation.rs`,
`kana.rs`, or `romaji.rs` appear, you ran rustfmt on a crate root — revert them
with `git checkout --` before committing.

---

## Task 2: `build_new` and the publish race

**Files:**
- Modify: `crates/jparser/src/index/generations.rs`
- Modify: `crates/jparser/src/index/mod.rs` (add `IndexError::GenerationExists`)

**Interfaces:**
- Consumes: `latest_number`, `GENERATION_PREFIX`, `BUILD_PREFIX` (Task 1);
  `build_from_reader(xml: R, table: &ConjugationTable, opts: &StemOptions, out_dir: &Path) -> Result<BuildReport, IndexError>`
  from `crate::index::build`; `BuildReport` from `crate::index`;
  `ConjugationTable` from `crate::conjugation`; `StemOptions` from `crate::stem`.
- Produces:
  - `IndexError::GenerationExists { generation: u64, build_dir: PathBuf }`
  - `fn publish(build_dir: &Path, root: &Path, generation: u64) -> Result<PathBuf, IndexError>` *(private)*
  - `pub fn build_new(root: &Path, xml: impl BufRead, table: &ConjugationTable, opts: &StemOptions) -> Result<(PathBuf, BuildReport), IndexError>`

**Signature correction against the design spec.** Spec §5 writes
`build_new(root, xml)`. The real `build_from_reader` also requires
`&ConjugationTable` and `&StemOptions`, so both must be threaded through.
`build_new` additionally returns the `BuildReport` so the CLI can keep printing
the statistics `build-index` prints today. This plan's signature is
authoritative.

**Why `publish` is a separate function.** The lost-race branch is otherwise
untestable: `build_new` computes `N = latest + 1`, so a pre-existing `gen-N`
would simply push it to `N+1` rather than collide. Extracting the rename lets a
test drive the collision directly.

- [ ] **Step 1: Add the error variant**

In `crates/jparser/src/index/mod.rs`, add to `pub enum IndexError`, after the
`ConjugationMismatch` variant:

```rust
    #[error(
        "index generation {generation} already exists; another builder \
         published first — retry (partial build kept at {build_dir})"
    )]
    GenerationExists {
        generation: u64,
        build_dir: std::path::PathBuf,
    },
```

The `build_dir` is named in the message because that is what makes the retry
actionable and the orphan findable. A bare `IndexError::Io` carrying `ENOTEMPTY`
would flatten "another builder won, retry" and "the disk is full" into one
message with two different operator responses.

Note `IndexError` derives `Debug` already; `PathBuf` satisfies it. No new
derive is needed.

- [ ] **Step 2: Write the failing tests**

Add to `generations.rs`'s `mod tests`, extending its imports first:

```rust
    use crate::conjugation::ConjugationTable;
    use crate::stem::StemOptions;

    /// The smallest JMdict that produces a non-empty index.
    const MINI_XML: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "<JMdict>",
        "<entry><ent_seq>1000001</ent_seq>",
        "<k_ele><keb>本</keb></k_ele>",
        "<r_ele><reb>ほん</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
        "<entry><ent_seq>1000002</ent_seq>",
        "<k_ele><keb>山</keb></k_ele>",
        "<r_ele><reb>やま</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>mountain</gloss></sense></entry>",
        "</JMdict>",
    );

    fn build_into(root: &Path) -> PathBuf {
        let table = ConjugationTable::load_embedded().expect("table");
        let opts = StemOptions::default();
        let (path, _) =
            build_new(root, MINI_XML.as_bytes(), &table, &opts).expect("build_new");
        path
    }

    #[test]
    fn the_first_build_publishes_generation_one() {
        let root = scratch("gen-first");
        assert_eq!(build_into(&root), root.join("gen-1"));
    }

    #[test]
    fn successive_builds_increment_the_generation() {
        let root = scratch("gen-increment");
        assert_eq!(build_into(&root), root.join("gen-1"));
        assert_eq!(build_into(&root), root.join("gen-2"));
        assert_eq!(build_into(&root), root.join("gen-3"));
    }

    #[test]
    fn a_successful_build_leaves_no_temp_directory() {
        let root = scratch("gen-notemp");
        build_into(&root);
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(BUILD_PREFIX))
            .collect();
        assert!(leftovers.is_empty(), "temp directory survived a success");
    }

    #[test]
    fn a_published_generation_opens() {
        let root = scratch("gen-opens");
        let path = build_into(&root);
        let index = crate::index::load::Index::open(&path).expect("open");
        let entry = index.entry(1000001).expect("entry").expect("present");
        assert_eq!(entry.id, 1000001);
    }

    /// Two builds inside one process must not collide on the nonce.
    #[test]
    fn two_builds_in_one_process_use_distinct_temp_names() {
        let root = scratch("gen-nonce");
        build_into(&root);
        build_into(&root);
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-2")));
    }

    /// The lost-race branch, driven directly because `build_new` alone can
    /// never collide — it always targets `latest + 1`.
    #[test]
    fn publishing_onto_an_existing_generation_reports_the_race() {
        let root = scratch("gen-race");
        mkdir(&root, "gen-1");
        std::fs::write(root.join("gen-1").join("occupied"), b"x").expect("write");

        let build_dir = root.join(format!("{BUILD_PREFIX}test-1"));
        std::fs::create_dir(&build_dir).expect("create build dir");

        let err = publish(&build_dir, &root, 1).expect_err("must not overwrite");
        match err {
            IndexError::GenerationExists {
                generation,
                build_dir: kept,
            } => {
                assert_eq!(generation, 1);
                assert_eq!(kept, build_dir);
            }
            other => panic!("expected GenerationExists, got {other:?}"),
        }
        assert!(build_dir.exists(), "the loser's build must survive for a retry");
        assert!(root.join("gen-1").join("occupied").exists(), "winner untouched");
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p jparser --lib generations`

Expected: FAIL to compile, on `cannot find function 'build_new'` and
`cannot find function 'publish'`.

- [ ] **Step 4: Implement `publish` and `build_new`**

Widen `generations.rs`'s `use` block to:

```rust
use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::conjugation::ConjugationTable;
use crate::index::build::build_from_reader;
use crate::index::{BuildReport, IndexError};
use crate::stem::StemOptions;
```

Then insert after `latest`:

```rust
/// Move a completed build into `<root>/gen-<generation>`.
///
/// The rename is the publish: it is the single operation that makes a
/// generation visible, and it is atomic. On failure the build directory is
/// left in place so the caller can retry without rebuilding.
fn publish(build_dir: &Path, root: &Path, generation: u64) -> Result<PathBuf, IndexError> {
    let target = root.join(format!("{GENERATION_PREFIX}{generation}"));
    match std::fs::rename(build_dir, &target) {
        Ok(()) => Ok(target),
        Err(e) => {
            // MSRV 1.75: `ErrorKind::DirectoryNotEmpty` is 1.83, and the raw
            // errno differs per platform (ENOTEMPTY is 66 on Darwin, 39 on
            // Linux, and Windows reports something else entirely). Probing
            // the target is portable and says the same thing: if it exists,
            // somebody else published first.
            if target.exists() {
                Err(IndexError::GenerationExists {
                    generation,
                    build_dir: build_dir.to_path_buf(),
                })
            } else {
                Err(e.into())
            }
        }
    }
}

/// Build an index from `xml` and publish it as the next generation.
///
/// Builds into `<root>/.build-<pid>-<nanos>/` first, so `root` never contains a
/// partially-written generation. `root` and the build directory are therefore
/// always on one filesystem — `fs::rename` returns `EXDEV` across devices and
/// never falls back to copying.
pub fn build_new(
    root: &Path,
    xml: impl BufRead,
    table: &ConjugationTable,
    opts: &StemOptions,
) -> Result<(PathBuf, BuildReport), IndexError> {
    std::fs::create_dir_all(root)?;

    // A clock before the Unix epoch is not a reason to refuse to build a
    // dictionary; the pid half still separates concurrent processes.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let build_dir = root.join(format!("{}{}-{}", BUILD_PREFIX, std::process::id(), nanos));

    // `create_dir`, never `create_dir_all`: the nonce must not already exist,
    // and a collision is a signal worth surfacing rather than absorbing.
    std::fs::create_dir(&build_dir)?;

    let report = build_from_reader(xml, table, opts, &build_dir)?;
    let generation = latest_number(root)?.map_or(1, |(n, _)| n + 1);
    let path = publish(&build_dir, root, generation)?;
    Ok((path, report))
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p jparser --lib generations`
Expected: PASS, 15 tests.

Run: `cargo clippy -p jparser --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/index/generations.rs
rustfmt --edition 2021 crates/jparser/src/index/mod.rs
git diff --stat
git add crates/jparser/src/index/generations.rs crates/jparser/src/index/mod.rs
git commit -m "feat: publish a built index as an immutable generation"
```

---

## Task 3: The torn-build crash-point suite

**Files:**
- Create: `crates/jparser/tests/generations_torn_build.rs`

**Interfaces:**
- Consumes: `jparser::index::generations::{build_new, latest, BUILD_PREFIX}`,
  `jparser::index::build::build_from_reader`, `jparser::index::load::Index`,
  the five file-name constants from `jparser::index`,
  `jparser::conjugation::ConjugationTable`, `jparser::stem::StemOptions`.
- Produces: no library API. It produces the evidence that the layout works.

**This is the task the phase exists for.** Phase 1A reproduced the hazard across
eleven reconstructed crash points, several of which returned well-formed wrong
answers with **no error at all** — `entry(2000010)` returning
`EntryData { id: 1000010, .. }`, and `prefixes_of` returning a `StoredRecord`
belonging to a different key. A suite asserting only "no panic" would have
passed against the broken code.

`build_from_reader` writes five files in a fixed order — `keys.fst`,
`records.bin`, `entries.bin`, `entries.idx`, `header.bin`, header last
(`index/build.rs:103,113,127,129,140`). The eleven states are: for each of those
five, {absent, truncated}, plus all-five-written-but-not-renamed.

- [ ] **Step 1: Write the hazard reproduction**

Create `crates/jparser/tests/generations_torn_build.rs` with the GPL v2 header,
then:

```rust
//! The generation layout's reason to exist, as executable evidence.
//!
//! Phase 1A established that a rebuild interrupted *inside a live index
//! directory* can leave `Index::open` succeeding against a spliced set of
//! files and returning well-formed wrong answers. These tests pin both halves:
//! that the hazard is real when a build writes directly into a served
//! directory, and that publishing through `gen-N` removes it.

use std::path::{Path, PathBuf};

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::generations::{build_new, latest, BUILD_PREFIX};
use jparser::index::load::Index;
use jparser::index::{ENTRIES_FILE, ENTRIES_INDEX_FILE, FST_FILE, HEADER_FILE, RECORDS_FILE};
use jparser::stem::StemOptions;

/// One dictionary.
const XML_A: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

/// A different dictionary: different ids, different surfaces, different sizes.
const XML_B: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>2000010</ent_seq><k_ele><keb>山</keb></k_ele>",
    "<r_ele><reb>やま</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>mountain</gloss></sense></entry>",
    "<entry><ent_seq>2000020</ent_seq><k_ele><keb>川</keb></k_ele>",
    "<r_ele><reb>かわ</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>river</gloss></sense></entry>",
    "<entry><ent_seq>2000030</ent_seq><k_ele><keb>海</keb></k_ele>",
    "<r_ele><reb>うみ</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>sea</gloss></sense></entry>",
    "</JMdict>",
);

const FILES: [&str; 5] = [
    FST_FILE,
    RECORDS_FILE,
    ENTRIES_FILE,
    ENTRIES_INDEX_FILE,
    HEADER_FILE,
];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn table_and_opts() -> (ConjugationTable, StemOptions) {
    (
        ConjugationTable::load_embedded().expect("table"),
        StemOptions::default(),
    )
}

/// Build `xml` directly into `dir` — the pre-generations behaviour.
fn build_directly(dir: &Path, xml: &str) {
    let (table, opts) = table_and_opts();
    build_from_reader(xml.as_bytes(), &table, &opts, dir).expect("build");
}

/// Delete a file, or truncate it to half its length.
fn damage(dir: &Path, file: &str, absent: bool) {
    let path = dir.join(file);
    if absent {
        std::fs::remove_file(&path).expect("remove");
    } else {
        let len = std::fs::metadata(&path).expect("metadata").len();
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open for truncate");
        f.set_len(len / 2).expect("truncate");
    }
}

/// The hazard, reproduced. A rebuild interrupted inside a directory that is
/// already being served leaves `Index::open` reading a mix of two builds.
///
/// This test asserts the BAD outcome deliberately: it is the baseline the
/// generation layout is measured against. If a future change makes
/// `Index::open` validate its payload, this test starts failing for a good
/// reason — delete it and say so in the commit. Do not "fix" it.
#[test]
fn a_rebuild_into_a_live_directory_can_serve_data_from_neither_build() {
    let dir = scratch("torn-hazard");
    build_directly(&dir, XML_A);

    // Interrupt a rebuild after two of the five files have been replaced.
    let staging = scratch("torn-hazard-staging");
    build_directly(&staging, XML_B);
    for file in [FST_FILE, RECORDS_FILE] {
        std::fs::copy(staging.join(file), dir.join(file)).expect("copy");
    }

    // `header.bin` is still A's, so version and fingerprint both validate and
    // `open` succeeds. What it returns is a splice of two dictionaries.
    let index = Index::open(&dir).expect("open succeeds — that is the hazard");
    let a_present = index.entry(1000010).expect("entry").is_some();
    let b_present = index.entry(2000010).expect("entry").is_some();
    assert!(
        !(a_present && b_present),
        "a spliced index cannot coherently hold both dictionaries"
    );
}
```

- [ ] **Step 2: Run it to confirm the hazard is real**

Run: `cargo test -p jparser --test generations_torn_build`
Expected: PASS. The hazard reproduces.

If it *fails*, the splice happened to be self-consistent on this fixture — add a
fourth entry to `XML_B` so the payload sizes diverge further, and re-run. Do not
weaken the assertion.

- [ ] **Step 3: Write the eleven-state suite**

Append to the same file:

```rust
/// Copy a completed build into an orphaned `.build-*` directory, damaged at
/// `state`, and leave it in `root` the way a killed process would.
fn strand_interrupted_build(root: &Path, state: usize) {
    let staging = scratch(&format!("torn-staging-{state}"));
    build_directly(&staging, XML_B);

    let orphan = root.join(format!("{BUILD_PREFIX}9999-{state}"));
    std::fs::create_dir_all(&orphan).expect("orphan dir");
    for file in FILES {
        std::fs::copy(staging.join(file), orphan.join(file)).expect("copy");
    }

    // States 0..=9 damage one file: state/2 selects it, state%2 selects
    // absent-vs-truncated. State 10 leaves the build whole but unpublished,
    // which is the "died just before the rename" case.
    if state < 10 {
        damage(&orphan, FILES[state / 2], state % 2 == 0);
    }
}

#[test]
fn no_interrupted_build_is_ever_served() {
    let (table, opts) = table_and_opts();

    for state in 0..11 {
        let root = scratch(&format!("torn-state-{state}"));

        // A good generation is already published and being served.
        let (good, _) =
            build_new(&root, XML_A.as_bytes(), &table, &opts).expect("build_new");
        assert_eq!(good, root.join("gen-1"));

        strand_interrupted_build(&root, state);

        // 1. The interrupted build is never what a reader resolves.
        let resolved = latest(&root).expect("latest").expect("a generation");
        assert_eq!(resolved, good, "state {state}: latest resolved the orphan");

        // 2. No `.build-*` path is ever returned.
        let name = resolved
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(
            !name.starts_with(BUILD_PREFIX),
            "state {state}: latest returned a build directory"
        );

        // 3. The resolved generation opens and is internally coherent. This is
        //    the assertion that would have caught the original hazard.
        let index = Index::open(&resolved).expect("open");
        let entry = index.entry(1000010).expect("entry").expect("present");
        assert_eq!(entry.id, 1000010, "state {state}: cross-generation splice");
        assert!(
            index.entry(2000010).expect("entry").is_none(),
            "state {state}: data from the interrupted build leaked in"
        );

        // 4. A later build still succeeds despite the orphan.
        let (next, _) =
            build_new(&root, XML_A.as_bytes(), &table, &opts).expect("build after orphan");
        assert_eq!(
            next,
            root.join("gen-2"),
            "state {state}: the orphan blocked a rebuild"
        );
    }
}
```

- [ ] **Step 4: Run the suite, then prove it can fail**

Run: `cargo test -p jparser --test generations_torn_build`
Expected: PASS, 2 tests.

**Then verify it is asserting something.** Temporarily change assertion 1 to
compare against `root.join("gen-2")` and re-run: it must FAIL for every state.
Change it back and re-run to green. A suite that cannot be made to fail is not
evidence, and this is the check design spec Risk #1 asks for.

- [ ] **Step 5: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/tests/generations_torn_build.rs
git diff --stat
git add crates/jparser/tests/generations_torn_build.rs
git commit -m "test: prove no interrupted build is ever served"
```

---

## Task 4: `sweep`

**Files:**
- Modify: `crates/jparser/src/index/generations.rs`

**Interfaces:**
- Consumes: `generation_number`, `BUILD_PREFIX`, `DEFAULT_KEEP_GENERATIONS`
  (Task 1).
- Produces: `pub fn sweep(root: &Path, keep: usize) -> Result<usize, IndexError>`

- [ ] **Step 1: Write the failing tests**

Add to `generations.rs`'s `mod tests`:

```rust
    #[test]
    fn sweep_keeps_the_highest_generations() {
        let root = scratch("gen-sweep-keep");
        for n in 1..=5 {
            mkdir(&root, &format!("gen-{n}"));
        }
        assert_eq!(sweep(&root, 2).expect("sweep"), 3);
        assert!(!root.join("gen-3").exists());
        assert!(root.join("gen-4").exists());
        assert!(root.join("gen-5").exists());
    }

    #[test]
    fn sweep_removes_orphaned_builds() {
        let root = scratch("gen-sweep-orphan");
        mkdir(&root, "gen-1");
        mkdir(&root, ".build-1-2");
        mkdir(&root, ".build-3-4");
        assert_eq!(sweep(&root, 2).expect("sweep"), 2);
        assert!(root.join("gen-1").exists());
        assert!(!root.join(".build-1-2").exists());
    }

    #[test]
    fn sweep_keeps_everything_when_keep_exceeds_the_count() {
        let root = scratch("gen-sweep-few");
        mkdir(&root, "gen-1");
        assert_eq!(sweep(&root, 2).expect("sweep"), 0);
        assert!(root.join("gen-1").exists());
    }

    /// Malformed names are not generations, so `sweep` must not count them
    /// toward `keep` — nor delete them, since it did not create them.
    #[test]
    fn sweep_ignores_malformed_names() {
        let root = scratch("gen-sweep-malformed");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-01");
        mkdir(&root, "unrelated");
        assert_eq!(sweep(&root, 1).expect("sweep"), 0);
        assert!(root.join("gen-01").exists());
        assert!(root.join("unrelated").exists());
    }

    #[test]
    fn sweep_on_an_absent_root_is_not_an_error() {
        let root = std::env::temp_dir().join("jparser-test-gen-sweep-absent");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(sweep(&root, 2).expect("sweep"), 0);
    }

    #[test]
    fn the_default_retention_is_two() {
        assert_eq!(DEFAULT_KEEP_GENERATIONS, 2);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --lib generations`
Expected: FAIL to compile, `cannot find function 'sweep'`.

- [ ] **Step 3: Implement `sweep`**

Insert after `build_new`:

```rust
/// Remove `.build-*` orphans and all but the `keep` highest generations.
/// Returns the number of directories removed.
///
/// # Precondition
///
/// **Call only before any `Index` has been opened from `root`** — at
/// application startup, never during a session. Phase 1A verified on Darwin
/// that an established mmap survives `remove_dir_all` on its parent, but
/// Windows does not generally permit deleting a mapped file.
/// [`DEFAULT_KEEP_GENERATIONS`] exists so that a sweep which fails anyway is
/// never load-bearing.
///
/// Directories that are neither a valid generation nor a build orphan are left
/// alone: this function did not create them.
pub fn sweep(root: &Path, keep: usize) -> Result<usize, IndexError> {
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };

    let mut generations: Vec<(u64, PathBuf)> = Vec::new();
    let mut orphans: Vec<PathBuf> = Vec::new();
    for entry in read {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(number) = generation_number(name) {
            generations.push((number, entry.path()));
        } else if name.starts_with(BUILD_PREFIX) {
            orphans.push(entry.path());
        }
    }

    // Highest first, so the tail past `keep` is exactly what to drop.
    generations.sort_by(|a, b| b.0.cmp(&a.0));

    let mut removed = 0usize;
    for path in orphans
        .iter()
        .chain(generations.iter().skip(keep).map(|(_, path)| path))
    {
        std::fs::remove_dir_all(path)?;
        removed += 1;
    }
    Ok(removed)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --lib generations`
Expected: PASS, 21 tests.

Run: `cargo clippy -p jparser --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/index/generations.rs
git diff --stat
git add crates/jparser/src/index/generations.rs
git commit -m "feat: sweep stale generations and build orphans"
```

---

## Task 5: `ensure_dictionary`

**Files:**
- Modify: `crates/jparser/src/index/mod.rs`

**Interfaces:**
- Consumes: `generations::{build_new, latest, sweep}` (Tasks 1, 2, 4);
  `Index::open` from `crate::index::load`;
  `IndexError::{VersionMismatch, ConjugationMismatch, Io, Fst, Encoding, Jmdict, GenerationExists}`.
- Produces:
  ```rust
  pub fn ensure_dictionary<R, F>(
      root: &Path,
      table: &ConjugationTable,
      opts: &StemOptions,
      keep: usize,
      source: F,
  ) -> Result<Index, IndexError>
  where
      F: FnOnce() -> std::io::Result<R>,
      R: std::io::BufRead;
  ```

**Signature correction against the design spec.** Spec §4 writes
`ensure_dictionary(root, keep, source)`. `build_new` requires
`&ConjugationTable` and `&StemOptions`, so both are threaded through here too.
This plan's signature is authoritative.

**The decision table** (design spec §6):

| `latest(root)` | `Index::open` | Action |
|---|---|---|
| `None` | — | build, sweep, open |
| `Some(p)` | `Ok` | return it; `source` is never called |
| `Some(p)` | `VersionMismatch` or `ConjugationMismatch` | build, sweep, open |
| `Some(p)` | any other variant | **return the error** |

- [ ] **Step 1: Write the failing tests**

Add a `#[cfg(test)] mod tests` block at the end of
`crates/jparser/src/index/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;

    use super::*;
    use crate::conjugation::ConjugationTable;
    use crate::stem::StemOptions;

    const XML: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "<JMdict>",
        "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
        "<r_ele><reb>ほん</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
        "</JMdict>",
    );

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn table_and_opts() -> (ConjugationTable, StemOptions) {
        (
            ConjugationTable::load_embedded().expect("table"),
            StemOptions::default(),
        )
    }

    /// Rewrite `gen-N`'s header so the next open rejects it.
    fn corrupt_header(dir: &std::path::Path, f: impl FnOnce(&mut IndexHeader)) {
        let path = dir.join(HEADER_FILE);
        let mut header: IndexHeader =
            bincode::deserialize(&std::fs::read(&path).expect("read")).expect("decode");
        f(&mut header);
        std::fs::write(&path, bincode::serialize(&header).expect("encode")).expect("write");
    }

    #[test]
    fn an_empty_root_builds_the_first_generation() {
        let root = scratch("ensure-first");
        let (table, opts) = table_and_opts();
        let calls = Cell::new(0usize);

        let index = ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("ensure_dictionary");

        assert_eq!(calls.get(), 1);
        assert!(root.join("gen-1").exists());
        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
    }

    /// The steady-state path. A ~60 MB download must not happen just because
    /// the application started.
    #[test]
    fn a_valid_generation_is_reused_without_consulting_the_source() {
        let root = scratch("ensure-reuse");
        let (table, opts) = table_and_opts();

        let first = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            first.set(first.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("first");
        assert_eq!(first.get(), 1);

        let second = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            second.set(second.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("second");
        assert_eq!(second.get(), 0, "the source was consulted on a valid index");
        assert!(!root.join("gen-2").exists(), "a redundant generation was built");
    }

    #[test]
    fn a_version_mismatch_rebuilds() {
        let root = scratch("ensure-version");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        corrupt_header(&root.join("gen-1"), |h| {
            h.version = INDEX_FORMAT_VERSION - 1;
        });

        let calls = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("rebuild");
        assert_eq!(calls.get(), 1, "a stale version must rebuild");
        assert!(root.join("gen-2").exists());
    }

    #[test]
    fn a_fingerprint_mismatch_rebuilds() {
        let root = scratch("ensure-fingerprint");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        corrupt_header(&root.join("gen-1"), |h| {
            h.conjugation_fingerprint ^= 1;
        });

        let calls = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("rebuild");
        assert_eq!(calls.get(), 1);
        assert!(root.join("gen-2").exists());
    }

    /// A published-then-corrupt generation is a bug or a failing disk.
    /// Rebuilding would re-download ~60 MB and hide it.
    #[test]
    fn a_corrupt_generation_errors_rather_than_rebuilding() {
        let root = scratch("ensure-corrupt");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        // Truncate a file `open` reads eagerly, leaving the header intact so
        // version and fingerprint both still validate.
        std::fs::write(root.join("gen-1").join(ENTRIES_INDEX_FILE), b"\x00\x00")
            .expect("truncate");

        let calls = Cell::new(0usize);
        let err = ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect_err("corruption must surface");

        assert!(
            matches!(err, IndexError::Encoding(_) | IndexError::Io(_)),
            "expected a decode or io error, got {err:?}"
        );
        assert_eq!(calls.get(), 0, "a corrupt index must not trigger a rebuild");
        assert!(!root.join("gen-2").exists());
    }

    #[test]
    fn a_rebuild_sweeps_to_the_retention_limit() {
        let root = scratch("ensure-sweep");
        let (table, opts) = table_and_opts();

        // Four rounds, invalidating the head each time so every round rebuilds.
        for _ in 0..4 {
            ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes()))
                .expect("ensure");
            let head = generations::latest(&root).expect("latest").expect("head");
            corrupt_header(&head, |h| h.version = INDEX_FORMAT_VERSION - 1);
        }

        let remaining = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(generations::GENERATION_PREFIX)
            })
            .count();
        assert!(
            remaining <= 2,
            "retention was not applied: {remaining} generations"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --lib index::tests`
Expected: FAIL to compile, `cannot find function 'ensure_dictionary'`.

- [ ] **Step 3: Implement `ensure_dictionary`**

In `crates/jparser/src/index/mod.rs`, add these imports to the file's `use`
block:

```rust
use std::path::Path;

use crate::conjugation::ConjugationTable;
use crate::index::load::Index;
use crate::stem::StemOptions;
```

Then add, after the type definitions and before the `#[cfg(test)]` block:

```rust
/// Open the newest usable index in `root`, building one first if necessary.
///
/// `source` is a *lazy* producer of JMdict XML: it is invoked only when a
/// build is actually required, because the real source is a ~60 MB download
/// and the steady-state path must not pay for it.
///
/// A version or conjugation-fingerprint mismatch triggers a rebuild — both are
/// expected after an application upgrade or a changed `conjugations.json`. Any
/// **other** open failure is returned rather than rebuilt: a generation that
/// was published as complete but does not read back is a bug or a failing
/// disk, and silently rebuilding would hide it.
pub fn ensure_dictionary<R, F>(
    root: &Path,
    table: &ConjugationTable,
    opts: &StemOptions,
    keep: usize,
    source: F,
) -> Result<Index, IndexError>
where
    F: FnOnce() -> std::io::Result<R>,
    R: std::io::BufRead,
{
    if let Some(current) = generations::latest(root)? {
        match Index::open(&current) {
            Ok(index) => return Ok(index),
            // Expected after an upgrade or an asset change: rebuild.
            Err(IndexError::VersionMismatch { .. })
            | Err(IndexError::ConjugationMismatch { .. }) => {}
            // Listed exhaustively rather than with a catch-all so that a new
            // IndexError variant forces a decision here instead of silently
            // landing in one arm or the other.
            Err(e @ IndexError::Io(_))
            | Err(e @ IndexError::Fst(_))
            | Err(e @ IndexError::Encoding(_))
            | Err(e @ IndexError::Jmdict(_))
            | Err(e @ IndexError::GenerationExists { .. }) => return Err(e),
        }
    }

    let xml = source()?;
    let (published, _report) = generations::build_new(root, xml, table, opts)?;
    // Sweep after the publish, never before: sweeping first could delete the
    // generation the caller would have fallen back to had the build failed.
    let _swept = generations::sweep(root, keep)?;
    Index::open(&published)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jparser --lib index::tests`
Expected: PASS, 6 tests.

Run: `cargo test -p jparser`
Expected: PASS — the 210 pre-existing tests plus this phase's additions.

Run: `cargo clippy -p jparser --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/index/mod.rs
git diff --stat
git add crates/jparser/src/index/mod.rs
git commit -m "feat: open the newest usable index, building it on demand"
```

---

## Task 6: CLI subcommands

**Files:**
- Modify: `crates/jparser/src/index/load.rs` (add `Index::entry_count`)
- Modify: `crates/jparser/src/bin/jparser-cli.rs`
- Create: `crates/jparser/tests/cli_generations.rs`

**Interfaces:**
- Consumes: `jparser::index::ensure_dictionary` (Task 5),
  `jparser::index::generations::{latest, sweep, DEFAULT_KEEP_GENERATIONS,
  GENERATION_PREFIX}` (Tasks 1, 4), `jparser::index::load::Index`.
- Produces: `pub fn Index::entry_count(&self) -> usize`, plus three
  subcommands and their output format.

2A is headless by construction, so without these there is no way to exercise it
by hand. `gen-list` prints each generation, whether it opens, and why not when
it does not.

- [ ] **Step 1: Add `Index::entry_count`**

`gen-list` needs a cheap "does this index have content" signal, and `Index`
exposes no accessor. Add to `crates/jparser/src/index/load.rs`, inside
`impl Index`:

```rust
    /// Number of entries in the payload. Cheap: the offset table is already
    /// resident after `open`, so this touches no mmap page.
    pub fn entry_count(&self) -> usize {
        self.entry_offsets.len()
    }
```

`entry_offsets` is the field `open` assigns the deserialized `Vec<(u32, u64)>`
to at `load.rs:111`. If it carries a different name in the struct, use that
name — do not rename the field.

- [ ] **Step 2: Add the subcommands**

In `crates/jparser/src/bin/jparser-cli.rs`, widen the imports:

```rust
use jparser::index::generations::{latest, sweep, DEFAULT_KEEP_GENERATIONS, GENERATION_PREFIX};
use jparser::index::ensure_dictionary;
```

Add to `enum Command`, after the `BuildIndex` variant:

```rust
    /// Open the newest usable index in ROOT, building from XML if needed.
    EnsureDictionary {
        /// Generation root directory.
        root: PathBuf,
        /// Path to JMdict_e.xml (uncompressed), read only if a build is needed.
        xml: PathBuf,
        /// Generations to retain after a rebuild.
        #[arg(long, default_value_t = DEFAULT_KEEP_GENERATIONS)]
        keep: usize,
    },
    /// List the generations in ROOT, newest first.
    GenList {
        /// Generation root directory.
        root: PathBuf,
    },
    /// Remove build orphans and all but the newest generations.
    GenSweep {
        /// Generation root directory.
        root: PathBuf,
        /// Generations to retain.
        #[arg(long, default_value_t = DEFAULT_KEEP_GENERATIONS)]
        keep: usize,
    },
```

Add the match arms in `main`, after the `BuildIndex` arm:

```rust
        Command::EnsureDictionary { root, xml, keep } => {
            let table = ConjugationTable::load_embedded()?;
            let opts = StemOptions::default();
            let index = ensure_dictionary(&root, &table, &opts, keep, || {
                std::fs::File::open(&xml).map(BufReader::new)
            })?;
            let current = latest(&root)?;
            let name = current
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| NONE_LABEL.to_string());
            println!("generation: {name}");
            println!("entries:    {}", index.entry_count());
        }
        Command::GenList { root } => {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(&root)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            paths.sort();
            paths.reverse();
            for path in paths {
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned())
                else {
                    continue;
                };
                if !name.starts_with(GENERATION_PREFIX) {
                    continue;
                }
                match Index::open(&path) {
                    Ok(index) => println!("{name} ok entries={}", index.entry_count()),
                    Err(e) => println!("{name} unusable {e}"),
                }
            }
        }
        Command::GenSweep { root, keep } => {
            println!("removed: {}", sweep(&root, keep)?);
        }
```

`ensure-dictionary` prints the generation it settled on and the entry count, so
running it twice visibly builds only once.

- [ ] **Step 3: Write the failing CLI test**

Create `crates/jparser/tests/cli_generations.rs` with the GPL v2 header, then:

```rust
//! `jparser-cli` generation subcommands, end to end through a real process.

use std::path::{Path, PathBuf};
use std::process::Command;

const XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// `&Path`, never `&PathBuf` — the latter trips `clippy::ptr_arg`, which is a
/// hard error under this crate's `--all-targets -D warnings` gate.
fn cli(args: &[&str], cwd: &Path) -> String {
    let exe = env!("CARGO_BIN_EXE_jparser-cli");
    let out = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run jparser-cli");
    assert!(
        out.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

#[test]
fn ensure_dictionary_builds_once_and_then_reuses() {
    let dir = scratch("cli-gen");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    let root = dir.join("dict");

    let first = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);
    assert!(first.contains("generation: gen-1"), "got: {first}");
    assert!(first.contains("entries:    1"), "got: {first}");

    let second = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);
    assert!(second.contains("generation: gen-1"), "rebuilt: {second}");

    let listed = cli(&["gen-list", "dict"], &dir);
    assert_eq!(listed.lines().count(), 1, "got: {listed}");
    assert!(listed.starts_with("gen-1 ok entries=1"), "got: {listed}");

    assert!(root.join("gen-1").exists());
    assert!(!root.join("gen-2").exists());
}

#[test]
fn gen_sweep_reports_what_it_removed() {
    let dir = scratch("cli-sweep");
    let root = dir.join("dict");
    for name in ["gen-1", "gen-2", "gen-3", ".build-1-1"] {
        std::fs::create_dir_all(root.join(name)).expect("mkdir");
    }

    let out = cli(&["gen-sweep", "dict", "--keep", "1"], &dir);
    assert_eq!(out.trim(), "removed: 3");
    assert!(root.join("gen-3").exists());
    assert!(!root.join("gen-1").exists());
    assert!(!root.join(".build-1-1").exists());
}
```

The subcommand names on the command line are `ensure-dictionary`, `gen-list`,
and `gen-sweep`: clap derives kebab-case from the variant names.

- [ ] **Step 4: Run to verify it fails, then passes**

Before Steps 1–2 land, run: `cargo test -p jparser --test cli_generations`
Expected: FAIL — clap exits non-zero with
`error: unrecognized subcommand 'ensure-dictionary'`, and the `cli` helper's
`assert!(out.status.success())` fires.

After Steps 1–2, run it again.
Expected: PASS, 2 tests.

- [ ] **Step 5: Full gate**

```bash
cargo test -p jparser
cargo clippy -p jparser --all-targets -- -D warnings
cargo llvm-cov -p jparser --summary-only --fail-under-lines 80
```

Expected: all pass. Coverage should land near the Phase 1B baseline of 96.37%
and must never fall below 80.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/index/load.rs
rustfmt --edition 2021 crates/jparser/src/bin/jparser-cli.rs
rustfmt --edition 2021 crates/jparser/tests/cli_generations.rs
git diff --stat
git add crates/jparser/src/index/load.rs crates/jparser/src/bin/jparser-cli.rs \
        crates/jparser/tests/cli_generations.rs
git commit -m "feat: drive the generation layout from jparser-cli"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §3 `gen-<N>` layout, numeric ordering, strict name parsing | 1 |
| §3 `.build-<pid>-<nanos>` transient directory | 2 |
| §4 lazy closure source, no trait, no gzip in `jparser` | 5 |
| §5 `latest` | 1 |
| §5 `build_new`, `create_dir` not `create_dir_all` | 2 |
| §5 `sweep` | 4 |
| §5 CLI `ensure-dictionary`, `gen-list`, `gen-sweep` | 6 |
| §6 decision table, exhaustive match, sweep-after-publish | 5 |
| §7 `DEFAULT_KEEP_GENERATIONS = 2` | 1 (constant), 4 (doc + test) |
| §7 sweep precondition stated on the function | 4 |
| §7 no `fsync` | Not implemented, by decision. Recorded in the spec; no code. |
| §8 `GenerationExists` | 2 |
| §8 `EXDEV` precondition | 2 (doc on `build_new`) |
| §8 malformed names left alone | 1 (`latest`), 4 (`sweep`) |
| §9 eleven crash points, all four assertions | 3 |
| §9 concurrent builders | 2 (`publishing_onto_an_existing_generation_reports_the_race`) |
| §9 `latest` ignores `gen-`, `gen-abc`, `gen-01` | 1 |
| §9 version-mismatch vs corrupt as distinct outcomes | 5 |
| §9 CLI round trip | 6 |
| §11.1 suite must fail against direct-into-root | 3 (hazard reproduction + the Step 4 mutation check) |
| §11.5 nonce uniqueness within a process | 2 |
| §11.6 Windows unexercised | Stated below; no task can verify it on this machine. |

**2. Two corrections applied to the spec during planning.** Both are signature
errors the spec could not have caught without reading `build.rs`:

1. `build_new(root, xml)` → `build_new(root, xml, table, opts)`, returning
   `(PathBuf, BuildReport)`. `build_from_reader` requires a `&ConjugationTable`
   and a `&StemOptions`, and the CLI needs the report to keep printing the
   statistics `build-index` prints today.
2. `ensure_dictionary(root, keep, source)` → the same two parameters threaded
   through, since it calls `build_new`.

The spec should be amended to match, or read with this section beside it.

**3. Placeholder scan.** No `TBD`, no `TODO`, no "implement later", no "similar
to Task N", and no test that asserts nothing. Every code step carries runnable
code and every test step carries a concrete expected value. The one deliberate
assertion of a *bad* outcome — Task 3's hazard reproduction — is labelled as
such at the site, with an instruction not to "fix" it.

**4. Type consistency across task boundaries.** Checked:

- `latest_number` returns `Option<(u64, PathBuf)>` in Task 1 and is consumed as
  `.map_or(1, |(n, _)| n + 1)` in Task 2 — tuple shape matches.
- `build_new` returns `(PathBuf, BuildReport)` in Task 2 and is destructured as
  `let (published, _report) = …` in Task 5 — matches.
- `sweep` returns `usize` in Task 4, printed directly in Task 6 — matches.
- `GenerationExists { generation: u64, build_dir: PathBuf }` is declared in
  Task 2 Step 1 and matched with exactly those field names in Task 2's race test
  and Task 5's exhaustive `match` — matches.
- `DEFAULT_KEEP_GENERATIONS` is `usize` in Task 1 and used as clap's
  `default_value_t` for a `usize` field in Task 6 — matches.
- `GENERATION_PREFIX` is used by `generation_number` (Task 1), Task 5's
  retention test, and Task 6's `gen-list` filter — one definition, three
  readers, no literal `"gen-"` outside it.
- `Index::entry_count()` is introduced in Task 6 Step 1 and used in Task 6
  Step 2's two arms. It is the only new method on an existing Phase 1 type.
- `FILES` in Task 3 uses the five constants exported from `index/mod.rs`
  (`FST_FILE`, `RECORDS_FILE`, `ENTRIES_FILE`, `ENTRIES_INDEX_FILE`,
  `HEADER_FILE`) in `build.rs`'s write order, verified at
  `build.rs:103,113,127,129,140`.

**5. MSRV hazards, called out at each site.** `Option::is_none_or` (1.82) in
Task 1 Step 3 and `io::ErrorKind::DirectoryNotEmpty` (1.83) in Task 2 Step 4.
Both carry a comment naming the version and forbidding the "simplification".
These are the likeliest way to break the build in this phase.

**6. Residual gaps a human should look at.**

- **No `fsync`.** A power cut between the rename and the contents reaching disk
  can still publish a generation readers trust. Deliberate; see spec §7.
- **Windows is unexercised.** The four operations the layout needs —
  `create_dir` on a fresh name, `rename` to an absent target, `read_dir`,
  `remove_dir_all` when nothing is mapped — have never been run there. Nothing
  in this plan changes that, and no test in it can.
- **`sweep`'s precondition is documented, not enforced.** Nothing stops a
  caller sweeping while an `Index` is live. On Darwin that is harmless; on
  Windows it will fail, which `keep = 2` makes survivable but not invisible.
- **Task 3's hazard reproduction asserts a bad outcome deliberately.** If a
  future change makes `Index::open` validate its payload, that test starts
  failing for a good reason. It says so at the site, but a reviewer meeting it
  cold may still flag it.
- **`gen-list` sorts lexicographically**, so `gen-10` lists before `gen-9`. It
  is a human-facing listing, not a resolution path — `latest` is numeric and is
  what correctness depends on. Worth knowing before someone reports it as a bug.

---

## Execution Handoff

Plan complete and saved to
`docs/superpowers/plans/2026-08-13-jparser-phase2a.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between
tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans,
batch execution with checkpoints.

Which approach?
