# JParser Phase 2B — Dictionary Acquisition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Supply the JMdict bytes that Phase 2A's `ensure_dictionary` seam expects
— from a hand-placed file if one exists, otherwise by downloading, verifying, and
publishing one — without `crates/jparser` gaining an HTTP client or a
decompressor.

**Architecture:** A new `crates/jmdict-source` crate owns three layers:
`open_local` sniffs a file's first two bytes and wraps it in a `GzDecoder` or
passes it through; `fetch` downloads to a `.partial`, decodes it once to prove it
is a valid archive, then `fs::rename`s it into place; `resolve` composes them —
open what is there, or fetch first. The rename is the publish, for the same reason
2A's generations use one: a file at the resolved name must never be anything but a
complete, valid archive. The crate does **not** depend on `jparser`; the caller
wires them through 2A's closure.

**Tech Stack:** Rust 2021 (MSRV **1.75**, installed toolchain 1.97.1),
`ureq ~3.2` with rustls, `flate2 1`, `thiserror` (already in the workspace). No
async runtime, no `tempfile`, no mock framework.

**Reference:** `docs/superpowers/specs/2026-08-13-jparser-phase2b-design.md`
(authoritative), with `docs/superpowers/2026-08-13-jparser-phase2a-handoff.md`
for the seam this plugs into and the invariants it must not break. The C++
original in `ta-old/` is **read-only — never modify it**; it fetched EDICT2, not
JMdict, and is not a reference for this phase.

## Global Constraints

- **License GPL v2.** Every new source file gets the standard header comment,
  copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **MSRV is 1.75, and `resolver = "2"` does not enforce it.** This is the single
  most likely way to ship a violation in this phase. `ureq = "3"` and even
  `ureq = "3.2"` both resolve to **3.4.0, whose `rust-version` is 1.85** — and it
  compiles anyway, because the installed toolchain is 1.97.1. Task 1 addresses
  this structurally. Do not "simplify" the version requirement.
- **`crates/jparser` stays pure.** No Tauri, no UI crate, no HTTP client, no
  decompression dependency in the **library**. `jmdict-source` enters its
  manifest only as an `optional` dependency behind a default-on `cli` feature,
  and `cargo check -p jparser --no-default-features` is the gate that proves it.
- **No `tempfile`, no mock framework, no new dependency beyond `ureq` and
  `flate2`.** Test temp dirs use
  `std::env::temp_dir().join(format!("jmdict-source-test-{name}"))` +
  `let _ = std::fs::remove_dir_all(&dir);`.
- **No test touches the network.** The download path runs against a
  `std::net::TcpListener` on port 0.
- **Errors are explicit:** no `unwrap()`, `expect()`, or `unreachable!()` in
  library code outside `#[cfg(test)]`. Never swallow an error without a comment
  naming the reason.
- **No magic numbers, no bare literals:** `SOURCE_DIR`, `SOURCE_FILE`,
  `PARTIAL_SUFFIX`, `DOWNLOAD_ATTEMPTS`, `RETRY_BACKOFF`, and `JMDICT_URL` are
  named consts. The strings `"source"`, `"JMdict_e.gz"`, `".partial"`, and the
  URL appear nowhere else.
- **Naming, frozen:** `resolve`, `fetch`, `fetch_from`, `fetch_with_retry`,
  `open_local`, `verify_archive`, `SourceError`, `SOURCE_FILE`, `PARTIAL_SUFFIX`,
  `DOWNLOAD_ATTEMPTS`, `RETRY_BACKOFF`.
- **File size** 200–400 lines typical, **800 hard maximum including
  `#[cfg(test)] mod tests`**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never
  `cargo fmt`, never `cargo fmt -p jparser`, never rustfmt `src/lib.rs`** — those
  cascade into `jparser`'s `conjugation.rs`, `kana.rs`, and `romaji.rs`, which
  this phase must leave untouched. `conjugation.rs` is deliberately not
  rustfmt-clean; "fixing" it is a defect. After formatting run `git diff --stat`
  and confirm only intended files moved.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` clean at
  the end of every task.
- **Coverage:** 80% lines on the new crate —
  `cargo llvm-cov -p jmdict-source --summary-only --fail-under-lines 80`.

**Phase 2A invariants this phase must not break** (2A handoff):
`INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format; a
published `gen-N` is immutable; directory knowledge lives only in
`generations.rs` and `ensure_dictionary`. This phase touches none of them — it
produces a reader and knows nothing about generations.
`crates/jparser/src/segment.rs` sits at 778/800 lines and must not be edited.

---

## Resolved facts — do not re-derive these

Measured 2026-08-13 against the live registry and the live host. Spec §10 records
them.

| Fact | Value |
|---|---|
| URL | `http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz` |
| HTTPS | **Unavailable** — certificate subject-name mismatch. Plain HTTP only. |
| Size | 10,545,887 bytes compressed |
| Filename | `JMdict_e.gz`, **not** `JMdict_e.xml.gz` |
| `ureq` | 3.2.1 has `rust-version = 1.71.1`; **3.4.0 has 1.85** |
| `flate2` | 1.1.9, `rust-version = 1.67.0` |
| `ureq` API (verified to compile) | `ureq::get(url).call()?` → `res.status().as_u16()` → `res.body_mut().as_reader()` |
| TLS backend | rustls 0.23; `cargo tree` shows no `openssl-sys` |

`flate2`'s rejection behavior, measured — Task 2 asserts all of it:

| Input | Result |
|---|---|
| intact | `Ok` |
| corrupt CRC32 trailer | `Err`, `ErrorKind::InvalidInput` |
| corrupt ISIZE length | `Err`, `ErrorKind::InvalidInput` |
| truncated, trailer removed | `Err`, `ErrorKind::UnexpectedEof` |
| truncated mid-deflate-stream | `Err`, `ErrorKind::UnexpectedEof` |
| HTML body, not gzip | `Err`, `ErrorKind::InvalidInput` ("invalid gzip header") |

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | *(modified)* add `crates/jmdict-source` member; adopt `resolver = "3"` |
| `crates/jmdict-source/Cargo.toml` | *(new)* manifest — `ureq ~3.2`, `flate2 1`, `thiserror` |
| `crates/jmdict-source/src/lib.rs` | *(new)* constants, `SourceError`, `open_local`, `resolve` |
| `crates/jmdict-source/src/fetch.rs` | *(new)* `fetch`, `fetch_from`, `fetch_with_retry`, `verify_archive` |
| `crates/jmdict-source/tests/download.rs` | *(new)* `TcpListener`-driven download and retry tests |
| `crates/jmdict-source/tests/seam.rs` | *(new)* end-to-end: `resolve` feeds `ensure_dictionary` |
| `crates/jparser/Cargo.toml` | *(modified)* optional dep + `cli` feature + `required-features` |
| `crates/jparser/src/bin/jparser-cli.rs` | *(modified)* `--source-dir`, `fetch-source` |
| `crates/jparser/tests/cli_generations.rs` | *(modified)* argument-group tests |

`lib.rs` is projected at ~330 lines post-rustfmt and `fetch.rs` at ~380. Both are
under the cap. `verify_archive` lives in `fetch.rs` rather than `lib.rs` because
it exists to gate the rename, and the rename is `fetch`'s job.

---

## Task 1: Crate skeleton, MSRV enforcement, and `open_local`

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/jmdict-source/Cargo.toml`, `crates/jmdict-source/src/lib.rs`,
  `crates/jmdict-source/src/fetch.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub const SOURCE_DIR: &str = "source";`
  - `pub const SOURCE_FILE: &str = "JMdict_e.gz";`
  - `pub const PARTIAL_SUFFIX: &str = ".partial";`
  - `pub enum SourceError { Io, Transport(String), Http { status: u16 }, TooManyAttempts { attempts: usize, source_dir: PathBuf, last: String } }`
  - `pub fn open_local(path: &Path) -> Result<Box<dyn BufRead>, SourceError>`

**Why the resolver changes.** `resolver = "2"` ignores `rust-version` when
choosing versions, so `ureq = "3.2"` resolves to 3.4.0 — MSRV 1.85 — and builds
clean on the installed 1.97.1 toolchain. `resolver = "3"` makes Cargo prefer
versions compatible with the declared floor, which fixes the whole class rather
than this one instance. The `~3.2` requirement stays as a second line of defence,
because a resolver setting is easy to revert by accident.

- [ ] **Step 1: Install a 1.75 toolchain and confirm MSRV is checkable**

MSRV 1.75 has never been compile-verified in this repo — only reviewed by eye.
That is why this hazard survived three phases.

```bash
rustup toolchain install 1.75
cargo +1.75 check -p jparser --quiet
```

Expected: succeeds. If `jparser` itself fails on 1.75, **stop and report** — that
is a pre-existing violation and a separate decision, not this task's to fix.

- [ ] **Step 2: Adopt `resolver = "3"` and add the workspace member**

In the root `Cargo.toml`, change `resolver = "2"` to `resolver = "3"` and add the
new member:

```toml
[workspace]
members = ["crates/jparser", "crates/jmdict-source", "xtask"]
resolver = "3"
```

Then confirm nothing already in the tree moved:

```bash
cargo update --dry-run 2>&1 | grep -iE "downgrad|updat" || echo "no version changes"
cargo test --workspace --quiet 2>&1 | grep -E "^test result|^error"
```

Expected: the existing 255 tests still pass. If a dependency downgrades in a way
that breaks a build, **stop and report** rather than working around it.

- [ ] **Step 3: Create the crate manifest**

`crates/jmdict-source/Cargo.toml`:

```toml
[package]
name = "jmdict-source"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
# `~3.2` not `3.2`: a caret requirement resolves to 3.4.0, whose rust-version is
# 1.85 and breaks the workspace MSRV of 1.75. Do not loosen this.
ureq = { version = "~3.2", default-features = false, features = ["rustls"] }
flate2 = "1"
thiserror = "1"
```

If `crates/jparser/Cargo.toml` declares `thiserror` differently — a workspace
dependency, or a 2.x version — match it exactly rather than introducing a second
major version into the tree.

Then:

```bash
cargo tree -p jmdict-source | grep -iE "^jmdict-source|ureq|rustls|openssl|native-tls"
```

Expected: `ureq v3.2.x`, `rustls` present, **no `openssl-sys` and no
`native-tls`**. A `native-tls` line is a GPL-v2 licensing problem, not a
preference — stop and report.

- [ ] **Step 4: Write the failing tests**

Create `crates/jmdict-source/src/lib.rs` with the GPL v2 header, the module doc,
the `use` block, and **only** the test module below. It will not compile — that is
the intended RED.

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Obtaining the JMdict source archive.
//!
//! Phase 2A's `ensure_dictionary` takes a lazy `FnOnce() -> io::Result<impl
//! BufRead>` so that the ~10 MB archive is never obtained on the steady-state
//! path. This crate is that closure's body: it opens a hand-placed file if one
//! exists, and otherwise downloads, verifies, and publishes one.
//!
//! Deliberately independent of `jparser`. Taking a source directory rather than
//! a generation root keeps it ignorant of 2A's layout — a `resolve(root)` that
//! looked inside the generation root would depend on that layout without
//! declaring it.

use std::io::BufRead;
use std::path::{Path, PathBuf};

pub mod fetch;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// gzip `data`, so tests never ship a binary fixture.
    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).expect("gz write");
        e.finish().expect("gz finish")
    }

    const XML: &[u8] = b"<?xml version=\"1.0\"?><JMdict></JMdict>";

    fn read_all(mut r: Box<dyn BufRead>) -> Vec<u8> {
        let mut out = Vec::new();
        r.read_to_end(&mut out).expect("read");
        out
    }

    #[test]
    fn a_gzipped_file_is_decompressed() {
        let dir = scratch("open-gz");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(XML)).expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), XML);
    }

    /// The extension says `.gz`, but the bytes decide. A user who decompressed
    /// the archive by hand and kept the name must still work.
    #[test]
    fn a_plain_xml_file_is_passed_through() {
        let dir = scratch("open-plain");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, XML).expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), XML);
    }

    /// Shorter than the two magic bytes. Must not panic on the slice.
    #[test]
    fn a_one_byte_file_takes_the_plain_path() {
        let dir = scratch("open-tiny");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<").expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), b"<");
    }

    #[test]
    fn an_empty_file_takes_the_plain_path() {
        let dir = scratch("open-empty");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"").expect("write");
        assert!(read_all(open_local(&path).expect("open")).is_empty());
    }

    #[test]
    fn an_absent_file_is_an_io_error() {
        let dir = scratch("open-absent");
        let err = open_local(&dir.join(SOURCE_FILE)).expect_err("must fail");
        assert!(matches!(err, SourceError::Io(_)), "got {err:?}");
    }

    #[test]
    fn the_source_file_name_is_the_edrdg_name() {
        // EDRDG publishes `JMdict_e.gz`. An earlier draft of the design said
        // `JMdict_e.xml.gz`, which does not exist.
        assert_eq!(SOURCE_FILE, "JMdict_e.gz");
    }
}
```

- [ ] **Step 5: Run the tests to verify they fail**

Run: `cargo test -p jmdict-source`

Expected: FAIL to compile, with `cannot find function 'open_local'`, `cannot find
value 'SOURCE_FILE'`, `cannot find type 'SourceError'`, and an unresolved
`pub mod fetch`. That is the intended RED.

- [ ] **Step 6: Implement the constants, the error, and `open_local`**

Insert into `lib.rs` between the `use` block and `#[cfg(test)]`:

```rust
/// Directory name callers conventionally use for the source archive. Not
/// enforced — every function here takes the directory it is given.
pub const SOURCE_DIR: &str = "source";

/// The archive's name, as EDRDG publishes it. The extension is a location, not
/// a claim: [`open_local`] decides compression from the bytes.
pub const SOURCE_FILE: &str = "JMdict_e.gz";

/// Suffix for a download that is in progress or not yet verified. Never
/// resolved, so a killed download cannot be mistaken for a hand-placed file.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// The two-byte gzip magic, `1f 8b`.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloading the dictionary failed: {0}")]
    Transport(String),
    #[error("the dictionary server returned HTTP {status}")]
    Http { status: u16 },
    #[error(
        "could not obtain the dictionary after {attempts} attempts ({last}); \
         place a {file} in {source_dir} manually to bypass the download",
        file = SOURCE_FILE,
    )]
    TooManyAttempts {
        attempts: usize,
        source_dir: PathBuf,
        /// The final attempt's failure, rendered. Attempts can be exhausted by
        /// a transport error, a 5xx, or a corrupt archive, and those are three
        /// different things for a user to act on.
        last: String,
    },
}

/// Open `path`, decompressing it when its bytes say it is gzipped.
///
/// The file name is not consulted. EDRDG publishes `JMdict_e.gz`, but a user who
/// decompressed it by hand and kept the name is a case worth supporting, and
/// exactly one resolved name — rather than one per encoding — avoids needing a
/// precedence rule between two copies that disagree.
pub fn open_local(path: &Path) -> Result<Box<dyn BufRead>, SourceError> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    // `fill_buf` peeks without consuming, so the magic bytes remain available
    // to whichever reader is constructed below.
    let gzipped = reader.fill_buf()?.starts_with(&GZIP_MAGIC);
    if gzipped {
        Ok(Box::new(std::io::BufReader::new(
            flate2::read::GzDecoder::new(reader),
        )))
    } else {
        Ok(Box::new(reader))
    }
}
```

Create `crates/jmdict-source/src/fetch.rs` containing only the GPL v2 header and
this module doc, so `pub mod fetch;` resolves. Task 2 fills it in.

```rust
//! Downloading the archive, and proving it is one before publishing it.
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p jmdict-source`
Expected: PASS, 6 tests.

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

Run: `cargo +1.75 check -p jmdict-source --quiet`
Expected: succeeds. **This is the check that makes MSRV real.** If it fails, the
dependency versions are wrong — fix them, do not raise the floor.

- [ ] **Step 8: Format and commit**

```bash
rustfmt --edition 2021 crates/jmdict-source/src/lib.rs
rustfmt --edition 2021 crates/jmdict-source/src/fetch.rs
git diff --stat
git add Cargo.toml Cargo.lock crates/jmdict-source
git commit -m "feat: open a local JMdict archive, compressed or not"
```

`git diff --stat` must show only the root `Cargo.toml`, `Cargo.lock`, and the new
crate. If `conjugation.rs`, `kana.rs`, or `romaji.rs` appear, you ran rustfmt on a
crate root — `git checkout --` them before committing.

---

## Task 2: `verify_archive`

**Files:**
- Modify: `crates/jmdict-source/src/fetch.rs`

**Interfaces:**
- Consumes: `SourceError` (Task 1).
- Produces: `pub(crate) fn verify_archive(path: &Path) -> Result<(), SourceError>`

**Why this exists.** The transport is plain HTTP with no authenticity guarantee
(spec §10), so a transparent proxy's HTML login page arrives indistinguishable
from the archive until something decodes it. Verifying *before* the rename is
what stops those bytes landing at the resolved name, where `resolve` cannot tell
them from a hand-placed file and every future run fails identically. A
`Content-Length` check was considered and rejected: it passes exactly that case.

- [ ] **Step 1: Write the failing tests**

Add to `crates/jmdict-source/src/fetch.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).expect("gz write");
        e.finish().expect("gz finish")
    }

    /// Big enough that a half-truncation lands mid-deflate-stream.
    fn payload() -> Vec<u8> {
        b"<?xml version=\"1.0\"?><JMdict></JMdict>".repeat(50)
    }

    fn write(dir: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join("archive");
        std::fs::write(&path, bytes).expect("write");
        path
    }

    #[test]
    fn an_intact_archive_verifies() {
        let dir = scratch("verify-ok");
        let path = write(&dir, &gz(&payload()));
        assert!(verify_archive(&path).is_ok());
    }

    #[test]
    fn a_corrupt_checksum_fails() {
        let dir = scratch("verify-crc");
        let mut bytes = gz(&payload());
        let n = bytes.len();
        bytes[n - 8] ^= 0xff; // CRC32 field
        let path = write(&dir, &bytes);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_corrupt_length_fails() {
        let dir = scratch("verify-isize");
        let mut bytes = gz(&payload());
        let n = bytes.len();
        bytes[n - 4] ^= 0xff; // ISIZE field
        let path = write(&dir, &bytes);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_missing_trailer_fails() {
        let dir = scratch("verify-notrailer");
        let bytes = gz(&payload());
        let path = write(&dir, &bytes[..bytes.len() - 8]);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_mid_stream_truncation_fails() {
        let dir = scratch("verify-midstream");
        let bytes = gz(&payload());
        let path = write(&dir, &bytes[..bytes.len() / 2]);
        assert!(verify_archive(&path).is_err());
    }

    /// The captive-portal case: a proxy answers 200 with an HTML login page.
    /// This is the scenario that decides the design against a Content-Length
    /// check, and it fails at the gzip header, so rejection is nearly free.
    #[test]
    fn an_html_body_fails() {
        let dir = scratch("verify-html");
        let path = write(&dir, b"<html><body>Login required</body></html>");
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn an_empty_file_fails() {
        let dir = scratch("verify-empty");
        let path = write(&dir, b"");
        assert!(verify_archive(&path).is_err());
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jmdict-source --lib fetch`
Expected: FAIL to compile, `cannot find function 'verify_archive'`.

- [ ] **Step 3: Implement `verify_archive`**

Insert into `fetch.rs` above the test module, with its `use` block:

```rust
use std::path::Path;

use crate::SourceError;

/// Prove `path` is a complete, well-formed gzip archive by decoding all of it
/// and discarding the output.
///
/// Runs on a **downloaded** archive before it is renamed into place, never on a
/// hand-placed one — pre-verifying a user's own file would double the read on
/// every rebuild to guard against a mistake they made deliberately, and that
/// failure already surfaces correctly through the parser.
///
/// gzip carries a CRC32 and an uncompressed length in its trailer, both checked
/// at end of stream, so this rejects a corrupt checksum, a corrupt length, a
/// truncation, and a body that is not gzip at all. The measured behavior is
/// tabulated in the plan's "Resolved facts".
pub(crate) fn verify_archive(path: &Path) -> Result<(), SourceError> {
    let file = std::fs::File::open(path)?;
    let mut decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    // `io::sink` discards the output; only the error matters.
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jmdict-source`
Expected: PASS, 13 tests (6 from Task 1 + 7 new).

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 5: Prove the suite can fail**

Temporarily replace `verify_archive`'s body with `Ok(())` and re-run. **All six
failure tests must fail.** Revert and re-run to green. Record the output. A
verification function that cannot be caught doing nothing is not evidence, and
this phase's whole argument for the extra decode pass rests on it.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jmdict-source/src/fetch.rs
git diff --stat
git add crates/jmdict-source/src/fetch.rs
git commit -m "feat: reject a corrupt or non-gzip archive before it is published"
```

---

## Task 3: `fetch_from` — download, stage, verify, rename

**Files:**
- Modify: `crates/jmdict-source/src/fetch.rs`
- Create: `crates/jmdict-source/tests/download.rs`

**Interfaces:**
- Consumes: `SourceError`, `SOURCE_FILE`, `PARTIAL_SUFFIX` (Task 1);
  `verify_archive` (Task 2).
- Produces: `pub fn fetch_from(url: &str, source_dir: &Path) -> Result<PathBuf, SourceError>`
  — one attempt, no retry. Task 4 wraps it.

This task does a **single attempt**. Retry is Task 4, so the happy path and the
policy are reviewable separately. `fetch_from` is `pub` rather than `pub(crate)`
because integration tests are external crates and must reach it; its doc comment
says it is not part of the supported surface.

- [ ] **Step 1: Write the failing integration tests**

Create `crates/jmdict-source/tests/download.rs` with the GPL v2 header, then:

```rust
//! The download path, driven against a real socket.
//!
//! No mock framework and no network: a `TcpListener` on port 0 answers queued
//! requests with hardcoded HTTP/1.1 responses.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use jmdict_source::{SourceError, PARTIAL_SUFFIX, SOURCE_FILE};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn gz(data: &[u8]) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(data).expect("gz write");
    e.finish().expect("gz finish")
}

fn http(status: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

/// Serve `responses` in order, one per connection, then stop. Returns the URL
/// and a handle yielding the number of requests actually received.
fn serve(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        let mut served = 0usize;
        for response in responses {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // Drain the request head so the client sees a clean reply
                    // rather than a reset.
                    let mut buf = [0u8; 2048];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    served += 1;
                }
                Err(_) => break,
            }
        }
        served
    });
    (format!("http://{addr}/{SOURCE_FILE}"), handle)
}

/// Accept one connection and drop it without replying.
fn serve_reset() -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || match listener.accept() {
        Ok((stream, _)) => {
            drop(stream);
            1
        }
        Err(_) => 0,
    });
    (format!("http://{addr}/{SOURCE_FILE}"), handle)
}

fn partial(dir: &Path) -> PathBuf {
    dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}"))
}

const XML: &[u8] = b"<?xml version=\"1.0\"?><JMdict></JMdict>";

#[test]
fn a_good_download_lands_at_the_resolved_name() {
    let dir = scratch("dl-ok");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let path = jmdict_source::fetch::fetch_from(&url, &dir).expect("fetch");

    assert_eq!(path, dir.join(SOURCE_FILE));
    assert!(path.exists(), "archive missing");
    assert!(!partial(&dir).exists(), "the .partial survived a success");
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_404_fails_and_leaves_nothing_behind() {
    let dir = scratch("dl-404");
    let (url, server) = serve(vec![http("404 Not Found", b"nope")]);

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(matches!(err, SourceError::Http { status: 404 }), "got {err:?}");
    assert!(!dir.join(SOURCE_FILE).exists(), "a 404 body was published");
    assert!(!partial(&dir).exists(), "the .partial survived a failure");
    assert_eq!(server.join().expect("join"), 1);
}

/// The captive-portal case, end to end: HTTP 200, correct Content-Length,
/// HTML body. Verification must stop it before the rename.
#[test]
fn an_html_200_never_reaches_the_resolved_name() {
    let dir = scratch("dl-html");
    let body = b"<html><body>Login required</body></html>";
    let (url, server) = serve(vec![http("200 OK", body)]);

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(!matches!(err, SourceError::Http { .. }), "got {err:?}");
    assert!(
        !dir.join(SOURCE_FILE).exists(),
        "an HTML login page was published as the dictionary"
    );
    assert!(!partial(&dir).exists(), "the .partial survived a failure");
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_truncated_archive_never_reaches_the_resolved_name() {
    let dir = scratch("dl-trunc");
    let full = gz(&XML.repeat(50));
    let (url, server) = serve(vec![http("200 OK", &full[..full.len() / 2])]);

    jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(!dir.join(SOURCE_FILE).exists());
    assert!(!partial(&dir).exists());
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_dropped_connection_is_a_transport_error() {
    let dir = scratch("dl-reset");
    let (url, server) = serve_reset();

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(matches!(err, SourceError::Transport(_)), "got {err:?}");
    assert!(!dir.join(SOURCE_FILE).exists());
    let _ = server.join();
}

#[test]
fn the_source_directory_is_created_if_absent() {
    let dir = scratch("dl-mkdir");
    let nested = dir.join("does").join("not").join("exist");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let path = jmdict_source::fetch::fetch_from(&url, &nested).expect("fetch");

    assert!(path.exists());
    assert_eq!(server.join().expect("join"), 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jmdict-source --test download`
Expected: FAIL to compile, `cannot find function 'fetch_from'`.

- [ ] **Step 3: Implement `fetch_from`**

Widen `fetch.rs`'s `use` block and insert after `verify_archive`:

```rust
use std::path::PathBuf;

use crate::{PARTIAL_SUFFIX, SOURCE_FILE};

/// Download `url` into `source_dir` and publish it as [`crate::SOURCE_FILE`].
///
/// One attempt. [`fetch`] adds the retry policy.
///
/// Stages into `<SOURCE_FILE><PARTIAL_SUFFIX>`, verifies it, and only then
/// renames — so a file at the resolved name is always a complete, valid
/// archive. Two failures make that necessary: a killed process leaves a
/// truncation, and a proxy can answer 200 with an HTML page of the right
/// length. Either one, published, is indistinguishable from a hand-placed file
/// and fails identically on every subsequent run.
///
/// The staging file is always in `source_dir`, so the rename never crosses a
/// filesystem — `fs::rename` returns `EXDEV` across devices and never falls
/// back to copying.
///
/// **Not part of the supported surface.** It is separate from [`fetch`] so tests
/// can point it at a local listener; production callers use [`fetch`].
pub fn fetch_from(url: &str, source_dir: &Path) -> Result<PathBuf, SourceError> {
    std::fs::create_dir_all(source_dir)?;
    let target = source_dir.join(SOURCE_FILE);
    let staging = source_dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}"));

    match download_and_verify(url, &staging) {
        Ok(()) => {
            std::fs::rename(&staging, &target)?;
            Ok(target)
        }
        Err(e) => {
            // Best-effort cleanup: the download already failed, and a leftover
            // `.partial` is never resolved, so a failure here changes nothing
            // a caller can act on.
            let _ = std::fs::remove_file(&staging);
            Err(e)
        }
    }
}

/// Write `url`'s body to `staging`, then prove it is a valid archive.
fn download_and_verify(url: &str, staging: &Path) -> Result<(), SourceError> {
    let mut response = match ureq::get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => return Err(SourceError::Http { status }),
        Err(e) => return Err(SourceError::Transport(e.to_string())),
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(SourceError::Http { status });
    }

    {
        let mut body = response.body_mut().as_reader();
        let mut file = std::io::BufWriter::new(std::fs::File::create(staging)?);
        // A mid-transfer disconnect surfaces here, and it is a transport
        // failure rather than a corrupt archive — the retry policy in Task 4
        // treats the two the same way, but the message should not lie.
        std::io::copy(&mut body, &mut file)
            .map_err(|e| SourceError::Transport(e.to_string()))?;
        std::io::Write::flush(&mut file)?;
    }

    verify_archive(staging)
}
```

**If `ureq::Error::StatusCode` does not exist under the resolved 3.2.x**, find the
actual variant (`cargo doc -p ureq --no-deps --open`, or read the crate docs) and
match it — then **report the difference in your report**. This plan's `ureq` API
shape was verified to compile against 3.4.0; only the three calls in the
Resolved-facts table were confirmed, and error-variant names were not.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jmdict-source`
Expected: PASS, 19 tests (13 + 6 new).

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.
Run: `cargo +1.75 check -p jmdict-source --quiet` — succeeds.

- [ ] **Step 5: Prove the staging discipline is load-bearing**

Temporarily move the `std::fs::rename` call **above** the `match`, so the file is
published before verification. Re-run: `an_html_200_never_reaches_the_resolved_name`
and `a_truncated_archive_never_reaches_the_resolved_name` must both fail on their
`!dir.join(SOURCE_FILE).exists()` assertion. Revert and re-run to green. Record
the output — this is the check showing stage-then-verify keeps bad bytes out of
the resolved name rather than being ceremony.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jmdict-source/src/fetch.rs
rustfmt --edition 2021 crates/jmdict-source/tests/download.rs
git diff --stat
git add crates/jmdict-source
git commit -m "feat: download the archive and publish it by rename after verifying"
```

---

## Task 4: The retry policy

**Files:**
- Modify: `crates/jmdict-source/src/lib.rs`, `crates/jmdict-source/src/fetch.rs`,
  `crates/jmdict-source/tests/download.rs`

**Interfaces:**
- Consumes: `fetch_from` (Task 3), `SourceError::TooManyAttempts` (Task 1).
- Produces:
  - `pub const DOWNLOAD_ATTEMPTS: usize = 3;`
  - `pub const RETRY_BACKOFF: std::time::Duration` — 2 s, doubling
  - `pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError>`
  - `pub fn fetch_with_retry(url: &str, source_dir: &Path, backoff: Duration) -> Result<PathBuf, SourceError>`

**The policy** (spec §6):

| Outcome | Action |
|---|---|
| Transport error | Retry |
| 5xx | Retry |
| Verification failure | Retry — the bytes were bad, the URL was not |
| 4xx | Fail immediately. Retrying a 404 only waits. |
| Local write error | Fail immediately. Retrying will not free disk. |

**One tension to resolve, deliberately not pre-decided.** `verify_archive`
returns its decode failure as `SourceError::Io` via `#[from]`, but the policy
above makes `Io` non-retryable and verification failures retryable. As written
those contradict. Resolve it one of two ways, and **state which in your report**:

- add a `Corrupt(String)` variant to `SourceError` and have `verify_archive` map
  its decode error into it; or
- keep `Io` for local write failures only, and have `verify_archive` return
  `Transport`-adjacent semantics under a new variant of its own.

Either is defensible; both change `SourceError`'s public shape, which is why the
choice is yours to make and to record.

- [ ] **Step 1: Add the constants**

In `lib.rs`, after `PARTIAL_SUFFIX`:

```rust
/// Download attempts before [`fetch::fetch`] gives up.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Delay before the second attempt; doubles for each attempt after it.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);
```

- [ ] **Step 2: Write the failing tests**

`RETRY_BACKOFF` is 2 s, so an exhaustion test would sleep 6 s. `fetch_with_retry`
takes the backoff as a parameter and `fetch` passes `RETRY_BACKOFF`, so tests pass
`Duration::ZERO`. **Do not shorten the constant for testability** — the production
value is a product decision, and a test that sleeps for seconds gets skipped or
deleted by whoever is in a hurry.

Add to `crates/jmdict-source/tests/download.rs`:

```rust
use std::time::Duration;

#[test]
fn a_transient_failure_is_retried_and_can_succeed() {
    let dir = scratch("retry-recover");
    let (url, server) = serve(vec![
        http("503 Service Unavailable", b"busy"),
        http("200 OK", &gz(XML)),
    ]);

    let path = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO)
        .expect("fetch");

    assert!(path.exists());
    assert_eq!(server.join().expect("join"), 2, "the 503 was not retried");
}

#[test]
fn a_404_is_not_retried() {
    let dir = scratch("retry-404");
    // Three responses queued, but only one may be consumed.
    let (url, server) = serve(vec![
        http("404 Not Found", b"nope"),
        http("200 OK", &gz(XML)),
        http("200 OK", &gz(XML)),
    ]);

    let err = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO)
        .expect_err("must fail");

    assert!(matches!(err, SourceError::Http { status: 404 }), "got {err:?}");
    assert!(!dir.join(SOURCE_FILE).exists());
    // The server thread is still blocked in accept(); dropping the handle
    // avoids a timing-dependent join. What matters is that the client stopped
    // after one request, which the error variant already proves.
    drop(server);
}

#[test]
fn exhausting_the_attempts_reports_the_last_failure_and_the_escape_hatch() {
    let dir = scratch("retry-exhaust");
    let responses: Vec<Vec<u8>> = (0..jmdict_source::DOWNLOAD_ATTEMPTS)
        .map(|_| http("500 Internal Server Error", b"x"))
        .collect();
    let (url, server) = serve(responses);

    let err = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO)
        .expect_err("must fail");

    let rendered = err.to_string();
    match err {
        SourceError::TooManyAttempts {
            attempts,
            source_dir,
            last,
        } => {
            assert_eq!(attempts, jmdict_source::DOWNLOAD_ATTEMPTS);
            assert_eq!(source_dir, dir);
            assert!(!last.is_empty(), "the last failure was not recorded");
        }
        other => panic!("expected TooManyAttempts, got {other:?}"),
    }
    assert!(rendered.contains(SOURCE_FILE), "got: {rendered}");
    assert!(
        rendered.contains(&dir.display().to_string()),
        "the message must name the directory: {rendered}"
    );
    assert_eq!(
        server.join().expect("join"),
        jmdict_source::DOWNLOAD_ATTEMPTS,
        "wrong number of attempts"
    );
}

/// A corrupt body is retried, because the bytes were bad and the URL was not.
#[test]
fn a_corrupt_archive_is_retried() {
    let dir = scratch("retry-corrupt");
    let (url, server) = serve(vec![
        http("200 OK", b"<html>not gzip</html>"),
        http("200 OK", &gz(XML)),
    ]);

    let path = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO)
        .expect("fetch");

    assert!(path.exists());
    assert_eq!(server.join().expect("join"), 2, "the corrupt body was not retried");
}

#[test]
fn the_default_attempt_count_is_three() {
    assert_eq!(jmdict_source::DOWNLOAD_ATTEMPTS, 3);
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p jmdict-source --test download`
Expected: FAIL to compile, `cannot find function 'fetch_with_retry'`.

- [ ] **Step 4: Implement the retry**

Insert into `fetch.rs`:

```rust
/// The published archive. Private: exposing it would invite a caller to fetch
/// it directly and skip the staging and verification in [`fetch_from`], which
/// are the only things standing between a proxy's error page and the resolved
/// name. EDRDG serves no usable HTTPS — the certificate fails subject-name
/// validation — so this is plain HTTP by necessity, which is precisely why
/// verification is mandatory rather than defensive.
const JMDICT_URL: &str = "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz";

/// Download the archive into `source_dir`, retrying transient failures.
pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(JMDICT_URL, source_dir, crate::RETRY_BACKOFF)
}

/// [`fetch`] with the URL and backoff injected, so tests can point at a local
/// listener without sleeping.
pub fn fetch_with_retry(
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> Result<PathBuf, SourceError> {
    let mut delay = backoff;
    let mut last = String::new();

    for attempt in 1..=crate::DOWNLOAD_ATTEMPTS {
        match fetch_from(url, source_dir) {
            Ok(path) => return Ok(path),
            // A 4xx will not change on a retry, and neither will a local write
            // failure. Surface both immediately rather than sleeping first.
            Err(e @ SourceError::Http { status: 400..=499 }) => return Err(e),
            Err(e @ SourceError::Io(_)) => return Err(e),
            Err(e) => {
                last = e.to_string();
                if attempt < crate::DOWNLOAD_ATTEMPTS {
                    std::thread::sleep(delay);
                    delay = delay.saturating_mul(2);
                }
            }
        }
    }

    Err(SourceError::TooManyAttempts {
        attempts: crate::DOWNLOAD_ATTEMPTS,
        source_dir: source_dir.to_path_buf(),
        last,
    })
}
```

Apply your resolution of the `Io`-vs-verification tension from this task's header
before running the tests — `a_corrupt_archive_is_retried` is the test that fails
if verification failures arrive as `Io`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p jmdict-source`
Expected: PASS, 24 tests (19 + 5 new).

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.
Run: `cargo +1.75 check -p jmdict-source --quiet` — succeeds.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jmdict-source/src/lib.rs
rustfmt --edition 2021 crates/jmdict-source/src/fetch.rs
rustfmt --edition 2021 crates/jmdict-source/tests/download.rs
git diff --stat
git add crates/jmdict-source
git commit -m "feat: retry transient download failures but not a 4xx"
```

---

## Task 5: `resolve`

**Files:**
- Modify: `crates/jmdict-source/src/lib.rs`

**Interfaces:**
- Consumes: `open_local` (Task 1), `fetch::fetch` (Task 4).
- Produces: `pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>>`

`io::Result`, not `Result<_, SourceError>`, so it drops into 2A's
`FnOnce() -> io::Result<impl BufRead>` with no mapping at the call site. The typed
error survives underneath via `io::Error::other`, reachable through
`Error::source()`.

- [ ] **Step 1: Write the failing tests**

Add to `lib.rs`'s `mod tests`:

```rust
    #[test]
    fn a_present_archive_is_opened_without_downloading() {
        let dir = scratch("resolve-local");
        std::fs::write(dir.join(SOURCE_FILE), gz(XML)).expect("write");
        assert_eq!(read_all(resolve(&dir).expect("resolve")), XML);
    }

    #[test]
    fn a_plain_archive_at_the_resolved_name_is_opened_too() {
        let dir = scratch("resolve-plain");
        std::fs::write(dir.join(SOURCE_FILE), XML).expect("write");
        assert_eq!(read_all(resolve(&dir).expect("resolve")), XML);
    }

    /// A killed download leaves a `.partial`. It must not satisfy `resolve`, or
    /// a truncation gets fed to the parser as though hand-placed. There is no
    /// listener here, so the fall-through to `fetch` fails and the error proves
    /// the `.partial` was not opened.
    #[test]
    fn a_partial_alone_does_not_satisfy_resolve() {
        let dir = scratch("resolve-partial");
        std::fs::write(dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}")), gz(XML))
            .expect("write");

        let err = resolve(&dir).expect_err("a .partial must not resolve");
        assert!(
            err.get_ref().is_some(),
            "the SourceError was lost inside the io::Error"
        );
    }

    #[test]
    fn the_typed_error_survives_the_io_wrapper() {
        let dir = scratch("resolve-typed");
        let err = resolve(&dir).expect_err("no source and no network");
        let inner = err.get_ref().and_then(|e| e.downcast_ref::<SourceError>());
        assert!(inner.is_some(), "expected a SourceError inside: {err:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jmdict-source --lib`
Expected: FAIL to compile, `cannot find function 'resolve'`.

- [ ] **Step 3: Implement `resolve`**

Insert into `lib.rs` after `open_local`:

```rust
/// Open the JMdict source in `source_dir`, downloading it first if absent.
///
/// Built to be the body of Phase 2A's lazy `source` closure:
///
/// ```ignore
/// let index = jparser::index::ensure_dictionary(
///     &generation_root, &table, &opts, keep,
///     || jmdict_source::resolve(&source_dir),
/// )?;
/// ```
///
/// 2A guarantees the closure runs only when a rebuild is actually needed, so a
/// steady-state start never touches the network or the ~10 MB archive.
///
/// A `<SOURCE_FILE><PARTIAL_SUFFIX>` file is never resolved: it is either a
/// download in progress or one that failed verification, and treating it as a
/// hand-placed archive is how a truncation reaches the parser.
pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>> {
    let target = source_dir.join(SOURCE_FILE);
    let path = if target.exists() {
        target
    } else {
        fetch::fetch(source_dir).map_err(std::io::Error::other)?
    };
    open_local(&path).map_err(std::io::Error::other)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p jmdict-source`
Expected: PASS, 28 tests.

Run: `cargo clippy --workspace --all-targets -- -D warnings` — clean.
Run: `cargo +1.75 check -p jmdict-source --quiet` — succeeds.

- [ ] **Step 5: Prove the local path really skips the download**

`a_present_archive_is_opened_without_downloading` passes partly because no server
is running, which is indirect evidence. Make it direct: temporarily change
`resolve` to call `fetch::fetch` unconditionally, ignoring `target.exists()`. That
test must fail. Revert and re-run to green, and record the output. A "no download
happened" claim that only holds because the network was unavailable anyway is not
evidence.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jmdict-source/src/lib.rs
git diff --stat
git add crates/jmdict-source/src/lib.rs
git commit -m "feat: resolve a local archive, downloading only when absent"
```

---

## Task 6: CLI wiring, the purity gate, and the seam

**Files:**
- Modify: `crates/jparser/Cargo.toml`, `crates/jparser/src/bin/jparser-cli.rs`,
  `crates/jparser/tests/cli_generations.rs`, `crates/jmdict-source/Cargo.toml`
- Create: `crates/jmdict-source/tests/seam.rs`

**Interfaces:**
- Consumes: `resolve`, `fetch::fetch`, `SOURCE_DIR` (Tasks 1, 4, 5);
  `jparser::index::ensure_dictionary` and `jparser::index::generations::latest`
  (Phase 2A).
- Produces: two CLI subcommand shapes and one compile-enforced purity property.

**Why the feature flag.** The design requires that `jparser`'s library gain no
HTTP client or decompressor, but `jparser-cli` lives inside that same crate, so
wiring it needs `jmdict-source` in `jparser`'s manifest. An `optional` dependency
behind a default-on `cli` feature resolves the contradiction and, more usefully,
turns the purity rule from a promise into a compile-checked property.

- [ ] **Step 1: Add the optional dependency and the feature**

In `crates/jparser/Cargo.toml`:

```toml
[features]
default = ["cli"]
# Pulls in the dictionary-source crate for `jparser-cli`. The library must build
# without it: `cargo check -p jparser --no-default-features` is the gate that
# keeps an HTTP client and a decompressor out of the parser.
cli = ["dep:jmdict-source"]

[dependencies]
jmdict-source = { path = "../jmdict-source", optional = true }

[[bin]]
name = "jparser-cli"
path = "src/bin/jparser-cli.rs"
required-features = ["cli"]
```

Verify the gate works **before** writing any CLI code:

```bash
cargo check -p jparser --no-default-features --quiet
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2"
```

Expected: the check succeeds and the grep count is **0**. If `jmdict-source`
appears with default features off, the `optional`/`dep:` wiring is wrong.

- [ ] **Step 2: Add the subcommands**

In `crates/jparser/src/bin/jparser-cli.rs`, widen the imports:

```rust
use clap::{ArgGroup, Parser, Subcommand};
use jmdict_source::SOURCE_DIR;
```

Make the existing positional optional, add `--source-dir` beside it, and require
exactly one:

```rust
    /// Open the newest usable index in ROOT, building from a source if needed.
    #[command(group(
        ArgGroup::new("source").required(true).args(["xml", "source_dir"])
    ))]
    EnsureDictionary {
        /// Generation root directory.
        root: PathBuf,
        /// Path to an uncompressed JMdict XML file, read only if a build is
        /// needed. Kept positional so existing invocations still work.
        xml: Option<PathBuf>,
        /// Directory holding the source archive, downloading it if absent.
        #[arg(long)]
        source_dir: Option<PathBuf>,
        /// Generations to retain after a rebuild. Must be at least 1.
        #[arg(long, default_value_t = DEFAULT_KEEP_GENERATIONS,
              value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
        keep: usize,
    },
    /// Download the JMdict archive into DIR without building an index.
    FetchSource {
        /// Directory to download into. Conventionally named `source`.
        dir: PathBuf,
    },
```

Replace the `EnsureDictionary` match arm's body and add the new arm:

```rust
        Command::EnsureDictionary {
            root,
            xml,
            source_dir,
            keep,
        } => {
            let table = ConjugationTable::load_embedded()?;
            let opts = StemOptions::default();
            // The ArgGroup guarantees exactly one is set, so both real branches
            // are reachable and the third exists only to satisfy the compiler.
            let index = match (&xml, &source_dir) {
                (Some(xml), _) => ensure_dictionary(&root, &table, &opts, keep, || {
                    std::fs::File::open(xml).map(BufReader::new)
                })?,
                (None, Some(dir)) => {
                    ensure_dictionary(&root, &table, &opts, keep, || {
                        jmdict_source::resolve(dir)
                    })?
                }
                (None, None) => return Err("no source given".into()),
            };
            let current = latest(&root)?;
            let name = current
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| NONE_LABEL.to_string());
            println!("generation: {name}");
            println!("entries:    {}", index.entry_count());
        }
        Command::FetchSource { dir } => {
            let path = jmdict_source::fetch::fetch(&dir)?;
            println!("archive:    {}", path.display());
            println!("convention: keep it in a directory named {SOURCE_DIR}");
        }
```

- [ ] **Step 3: Write the failing CLI and seam tests**

Add to `crates/jparser/tests/cli_generations.rs`:

```rust
/// The xml positional must keep working — 2A's other tests pass it that way,
/// and breaking it would make this phase edit tests about generations.
#[test]
fn ensure_dictionary_still_accepts_a_positional_xml() {
    let dir = scratch("cli-positional");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);

    assert!(out.contains("generation: gen-1"), "got: {out}");
}

/// The ArgGroup is what makes "exactly one source" true rather than intended.
#[test]
fn ensure_dictionary_rejects_both_sources_at_once() {
    let dir = scratch("cli-bothsources");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["ensure-dictionary", "dict", "mini.xml", "--source-dir", "src"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "both sources must be rejected");
}

#[test]
fn ensure_dictionary_rejects_no_source_at_all() {
    let dir = scratch("cli-nosource");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["ensure-dictionary", "dict"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "a missing source must be rejected");
}
```

Add to `crates/jmdict-source/Cargo.toml`:

```toml
[dev-dependencies]
# `default-features = false` so this does not pull `jmdict-source` back in
# through jparser's `cli` feature, which would be a dependency cycle.
jparser = { path = "../jparser", default-features = false }
```

Create `crates/jmdict-source/tests/seam.rs` with the GPL v2 header, then:

```rust
//! The deliverable: `resolve` feeding Phase 2A's `ensure_dictionary`.
//!
//! The only place both crates appear together, and the only test that proves
//! they compose. `jparser` is a dev-dependency here, not a dependency — the
//! direction matters, because `jmdict-source` must stay usable without it.

use std::io::Write;
use std::path::PathBuf;

use jmdict_source::SOURCE_FILE;
use jparser::conjugation::ConjugationTable;
use jparser::stem::StemOptions;

const XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn a_gzipped_archive_builds_an_index_through_ensure_dictionary() {
    let dir = scratch("seam");
    let source_dir = dir.join("source");
    let root = dir.join("dictionary");
    std::fs::create_dir_all(&source_dir).expect("mkdir");

    let mut e =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(XML.as_bytes()).expect("gz write");
    std::fs::write(source_dir.join(SOURCE_FILE), e.finish().expect("gz"))
        .expect("write");

    let table = ConjugationTable::load_embedded().expect("table");
    let opts = StemOptions::default();

    let index = jparser::index::ensure_dictionary(&root, &table, &opts, 2, || {
        jmdict_source::resolve(&source_dir)
    })
    .expect("ensure_dictionary");

    assert_eq!(
        index.entry(1000010).expect("entry").expect("present").id,
        1000010
    );
    assert!(root.join("gen-1").exists());
    // The source directory is a sibling of the generation root, so 2A's
    // sweep-and-list machinery never sees it, and the archive is not consumed.
    assert!(source_dir.join(SOURCE_FILE).exists(), "the source vanished");
}
```

`flate2` is needed by this test; add it under `[dev-dependencies]` too if the
compiler asks for it (it is already a normal dependency, so it should resolve).

- [ ] **Step 4: Run to verify failure, then success**

Before Steps 1–2 land, `cargo test -p jparser --test cli_generations` fails with
clap's `unexpected argument '--source-dir'`. After them:

Run: `cargo test -p jparser --test cli_generations`
Expected: PASS, 8 tests (5 from 2A + 3 new).

Run: `cargo test -p jmdict-source --test seam`
Expected: PASS, 1 test.

- [ ] **Step 5: The full gate**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p jparser --no-default-features --quiet
cargo +1.75 check --workspace --quiet
cargo llvm-cov -p jmdict-source --summary-only --fail-under-lines 80
cargo llvm-cov -p jparser --summary-only --fail-under-lines 80
cargo tree --workspace | grep -iE "openssl|native-tls" || echo "no openssl: good"
```

Expected: all pass. `jparser`'s coverage was 96.50% at the end of 2A and must not
fall below 80. If the new crate lands under 80, **stop and report** — do not add
tests solely to inflate it.

- [ ] **Step 6: Format and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/bin/jparser-cli.rs
rustfmt --edition 2021 crates/jparser/tests/cli_generations.rs
rustfmt --edition 2021 crates/jmdict-source/tests/seam.rs
git diff --stat
git add crates/jparser/Cargo.toml crates/jparser/src/bin/jparser-cli.rs \
        crates/jparser/tests/cli_generations.rs crates/jmdict-source Cargo.lock
git commit -m "feat: drive dictionary acquisition from jparser-cli"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 new `jmdict-source` crate, scope boundary | 1 |
| §3 sibling source directory; `.partial` never resolved | 5, 3 |
| §4 `SOURCE_DIR`/`SOURCE_FILE`/`PARTIAL_SUFFIX`/`DOWNLOAD_ATTEMPTS`/`RETRY_BACKOFF` | 1, 4 |
| §4 `resolve` returning `io::Result<Box<dyn BufRead>>` | 5 |
| §4 `fetch`, private URL, injectable seam | 4 |
| §4 CLI keeps both source forms via `ArgGroup` | 6 |
| §5 resolution order, `fill_buf` sniff, exactly one filename | 1, 5 |
| §6 stage → verify → rename; the retry table | 2, 3, 4 |
| §7 `SourceError` variants and the escape-hatch message | 1, 4 |
| §8 every required assertion, incl. the HTML-200 case and the seam test | 1–6 |
| §9 GPL header, MSRV, purity, formatting, clippy, coverage | 1, 6 |
| §10 resolved facts consumed, not re-derived | "Resolved facts" |

**2. Two spec problems this plan fixes.**

- **§9's purity rule contradicted §4's CLI change.** `jparser-cli` lives inside
  `crates/jparser`, so wiring `--source-dir` requires `jmdict-source` in that
  manifest — which §9 forbids outright. Task 6 resolves it with an optional
  dependency behind a default-on `cli` feature and turns the rule into a checked
  property via `cargo check -p jparser --no-default-features`.
- **The spec did not notice that `resolver = "2"` ignores `rust-version`.** `ureq`
  resolves to 3.4.0 (MSRV 1.85) and compiles clean on the installed 1.97.1
  toolchain, so review is the only thing that would catch it — and review missed
  it for three phases. Task 1 adopts `resolver = "3"`, pins `~3.2`, and installs a
  1.75 toolchain so MSRV is compile-checked for the first time in this repo.

**3. Placeholder scan.** No `TBD`, no `TODO`, no "implement later", no "similar to
Task N". Every code step carries runnable code and every test step a concrete
expected value. Three steps deliberately instruct the implementer to *decide and
report* rather than guess — Task 3 Step 3 on `ureq::Error::StatusCode` (the API
was verified against 3.4.0, not 3.2.x), Task 4's header on `verify_archive`'s
error variant, and Task 1 Step 1 on a pre-existing MSRV failure. Each names the
exact decision and its consequence, which is the opposite of a placeholder.

**4. Type consistency across task boundaries.** Checked:

- `open_local -> Result<Box<dyn BufRead>, SourceError>` (Task 1) is consumed by
  `resolve` (Task 5) with `.map_err(io::Error::other)` — matches.
- `verify_archive(&Path) -> Result<(), SourceError>` (Task 2) is the final
  expression of `download_and_verify` (Task 3) — matches.
- `fetch_from(&str, &Path) -> Result<PathBuf, SourceError>` (Task 3) is called by
  `fetch_with_retry` (Task 4) — matches.
- `fetch(&Path) -> Result<PathBuf, SourceError>` (Task 4) is called by `resolve`
  (Task 5) and the CLI's `FetchSource` arm (Task 6) — matches.
- `TooManyAttempts { attempts, source_dir, last }` is declared in Task 1 and
  destructured with exactly those names in Task 4's test — matches.
- `DOWNLOAD_ATTEMPTS: usize` (Task 4) is compared against a request count in the
  same task and asserted `== 3` — matches.
- `SOURCE_FILE` has one definition and is read by Task 1's tests, Task 3's
  staging, Task 5's `resolve`, and Task 6's seam test — no bare literal anywhere.

**5. Residual gaps a human should look at.**

- **No 1.75 toolchain is installed today**, so Task 1 Step 1 may reveal that
  `jparser` already violates its own stated MSRV. The plan says stop and report
  rather than fix, because that is a separate decision about a three-phase-old
  claim.
- **`ureq`'s API was verified against 3.4.0 while the plan pins `~3.2`.** The
  three calls in the Resolved-facts table are confirmed; error-variant names are
  not. Task 3 anticipates the difference and asks for a report.
- **Adopting `resolver = "3"` could move existing dependency versions.** Task 1
  Step 2 checks and stops on a breaking downgrade, but the blast radius is
  workspace-wide and deserves a human glance.
- **Integrity is verified; authenticity is not.** Plain HTTP with no digest means
  a determined on-path attacker can substitute a well-formed archive. Spec §10
  records the reasoning; a later phase may revisit it.
- **`a_404_is_not_retried` drops its server handle instead of joining it,**
  because the thread is still blocked in `accept()`. That is a deliberate trade to
  avoid a timing-dependent join; the error variant is what actually proves no
  retry happened. If it proves flaky, the fix is a bounded `set_nonblocking` poll,
  not a sleep.
- **`fetch_from` and `fetch_with_retry` are `pub` only because integration tests
  are external crates.** Their doc comments say they are not supported surface.
  If that bothers a reviewer, the alternative is moving those tests into
  `#[cfg(test)]` units inside `fetch.rs`, which loses the real-process coverage.

---

## Execution Handoff

Plan complete and saved to
`docs/superpowers/plans/2026-08-13-jparser-phase2b.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between
tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans,
batch execution with checkpoints.

Which approach?
