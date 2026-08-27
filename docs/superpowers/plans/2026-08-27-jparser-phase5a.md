# JParser Phase 5A — Vibrato Dictionary Acquisition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `jparser-cli fetch-hints DIR` downloads the pinned IPADIC dictionary over HTTPS, proves it is the expected bytes, unwraps xz and zstd, and publishes a `system.dic` that `VibratoTokenizer::load` opens — replacing the three manual commands `parse --hints` currently documents.

**Architecture:** A new crate, `crates/vibrato-source`, modelled line for line on `crates/jmdict-source`: constants and errors in `lib.rs`, the network half in `fetch.rs`, and the archive half in `extract.rs`. A download is staged under a PID-suffixed name, hashed, unpacked into a PID-suffixed staging **directory**, and published by renaming that directory — so nothing at the resolved path is ever partial. `crates/jparser` gains one non-default feature that carries the CLI driver; `src-tauri` is untouched.

**Tech Stack:** Rust 1.85 (workspace floor), `ureq` 3 with `rustls` + `rustls-graviola`, `lzma-rs`, `tar`, `ruzstd`, `sha2`, `thiserror`; `vibrato` 0.5 as a dev-dependency for fixtures.

**Spec:** `docs/superpowers/specs/2026-08-27-jparser-phase5a-design.md`

## Global Constraints

- Workspace floor is `rust-version = "1.85"`. `crates/vibrato-source` uses `rust-version.workspace = true`. Do not raise it.
- Project licence is **GPL-2.0-only**. Every new dependency must be usable under a GPL-2.0-compatible licence — MIT, or a dual licence taken under MIT/ISC. A licence containing `AND Apache-2.0` (as `ring` and `aws-lc-sys` do) offers no compatible option and must be refused. Record which licence was taken in a comment above each dependency, the way `tauri-plugin-clipboard-manager` and `tauri-plugin-global-shortcut` do in `src-tauri/Cargo.toml`.
- **No test may touch the real network.** Every fetch test drives a `std::net::TcpListener` bound to `127.0.0.1:0`, as `crates/jmdict-source/tests/download.rs` does. The real asset is fetched exactly once, by hand, in Task 7.
- **No binary fixture may be committed.** Fixtures are built in memory, following `hints.rs`' own precedent (`test_dictionary()`).
- Every file starts with the GPL header block used by every other file in `crates/`.
- Files stay under 800 lines. Doc comments explain *why*, in the prose style of the surrounding code. Tests are Arrange-Act-Assert with names that state the behaviour.
- Conventional Commits. Commit at the end of every task.
- The pinned asset, verbatim, used by Task 1 and Task 7:
  - URL: `https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz`
  - Size: `7_704_680` bytes
  - SHA-256: `4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9`
  - Inner member: `ipadic-mecab-2_7_0/system.dic.zst`, 8,045,952 bytes
  - Published `system.dic`: 47,788,814 bytes

---

## File Structure

| File | Responsibility |
| --- | --- |
| `Cargo.toml` (workspace) | Modify. One new member. |
| `crates/vibrato-source/Cargo.toml` | **Create.** Six dependencies, each with its licence recorded. |
| `crates/vibrato-source/src/lib.rs` | **Create.** Constants, `SourceError`, `published`, `resolve`, `publish`. The crate's whole public surface. |
| `crates/vibrato-source/src/extract.rs` | **Create.** xz → tar → zstd, whitelisted members, into a staging directory. No network. |
| `crates/vibrato-source/src/fetch.rs` | **Create.** The agent (TLS provider wiring), one download attempt, SHA-256 verification, the retry policy. |
| `crates/vibrato-source/tests/support/mod.rs` | **Create.** Fixture builders shared by the integration tests: a real two-entry dictionary, and a real `.tar.xz` around it. |
| `crates/vibrato-source/tests/extract.rs` | **Create.** Archive-half tests. No sockets. |
| `crates/vibrato-source/tests/download.rs` | **Create.** Fetch-half tests against a local listener. |
| `crates/jparser/Cargo.toml` | Modify. Optional `vibrato-source` dependency and the `hints-source` feature. |
| `crates/jparser/src/bin/jparser-cli.rs` | Modify. The `fetch-hints` subcommand, and the replacement of `parse --hints`' obsolete instructions. |
| `.github/workflows/ci.yml` | Modify. Purity grep gains the new crate; the TLS gate widens to `ring`/`aws-lc`. |
| `docs/superpowers/2026-08-27-jparser-phase5a-handoff.md` | **Create** in Task 7. |

---

## Task 0: Verify the dependencies before writing any code

**Files:** none. This task produces answers, not a diff.

**Interfaces:**
- Consumes: nothing.
- Produces: confirmation (or correction) of every API and licence later tasks assume, plus the baseline test counts Task 7's gate compares against.

Five of this plan's facts are expectations until they are run. None of these crates is in `Cargo.lock` today.

- [ ] **Step 1: Record the baseline test counts**

```bash
cargo test --workspace 2>&1 | grep -E "^test result:" 
npx vitest run 2>&1 | tail -5
```

Write both numbers down. Task 7 asserts the suite grew and nothing regressed.

- [ ] **Step 2: Create a throwaway probe crate outside the repo**

```bash
cd "$TMPDIR" 2>/dev/null || cd /tmp
cargo new --lib vibprobe && cd vibprobe
cargo add ureq@3 --no-default-features --features rustls-no-provider,platform-verifier
cargo add rustls-graviola@0.4 lzma-rs@0.3 tar@0.4 ruzstd@0.9 sha2@0.11
cargo add vibrato@0.5 --no-default-features
```

The probe stays outside the repo so a failed experiment never touches `Cargo.lock`.

- [ ] **Step 3: Confirm the licences of everything that resolved**

```bash
cargo metadata --format-version 1 | python -c "
import json,sys
for p in sorted(json.load(sys.stdin)['packages'], key=lambda p: p['name']):
    print(p['name'], '|', p.get('license'))
" | grep -iE "rustls|graviola|ring|aws-lc|webpki|lzma|ruzstd|^tar |sha2|ureq"
```

Expected: no `ring`, no `aws-lc-sys` anywhere in the tree. **If either appears, stop and report** — the crypto provider wiring in Step 4 has not taken effect, and the spec's §6 reasoning does not survive it.

- [ ] **Step 4: Confirm the TLS stack completes a real handshake**

```rust
// src/lib.rs of the probe
#[test]
fn graviola_reaches_github() {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .unversioned_rustls_crypto_provider(std::sync::Arc::new(
            rustls_graviola::default_provider(),
        ))
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(120)))
            .tls_config(tls)
            .build(),
    );
    let mut res = agent
        .get("https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz")
        .call()
        .expect("the handshake and the redirect must both succeed");
    let bytes = res.body_mut().with_config().limit(32 * 1024 * 1024).read_to_vec().unwrap();
    assert_eq!(bytes.len(), 7_704_680);
    std::fs::write("asset.tar.xz", &bytes).unwrap();
}
```

Run: `cargo test graviola_reaches_github -- --nocapture`
Expected: PASS. This proves the handshake, the redirect to `objects.githubusercontent.com`, and the size in one shot, and leaves `asset.tar.xz` for Step 5.

**If `RootCerts::PlatformVerifier` fails to build a config** (it may want the provider installed process-wide), first try adding `let _ = rustls_graviola::default_provider().install_default();` before building the agent. If it still fails, fall back to `RootCerts::WebPki` with ureq's `rustls-webpki-roots` feature — `webpki-roots` is CDLA-Permissive-2.0, which is permissive and carries no Apache-2.0 term — and **correct §6 of the design spec** before continuing.

- [ ] **Step 5: Confirm the unpacking stack, and time it**

```rust
#[test]
fn the_real_asset_unpacks_and_loads() {
    let t0 = std::time::Instant::now();
    let mut tar_bytes = Vec::new();
    let mut input = std::io::BufReader::new(std::fs::File::open("asset.tar.xz").unwrap());
    lzma_rs::xz_decompress(&mut input, &mut tar_bytes).expect("xz");
    eprintln!("xz: {:?} -> {} bytes", t0.elapsed(), tar_bytes.len());

    let mut names = Vec::new();
    let mut dic = Vec::new();
    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let name = entry.path().unwrap().to_string_lossy().into_owned();
        if name.ends_with("system.dic.zst") {
            let t1 = std::time::Instant::now();
            let mut d = ruzstd::decoding::StreamingDecoder::new(&mut entry).expect("zstd");
            std::io::copy(&mut d, &mut dic).unwrap();
            eprintln!("zstd: {:?}", t1.elapsed());
        }
        names.push(name);
    }
    eprintln!("members: {names:?}");
    assert_eq!(dic.len(), 47_788_814);

    let t2 = std::time::Instant::now();
    vibrato::Dictionary::read(std::io::Cursor::new(&dic)).expect("vibrato 0.5.2 must read it");
    eprintln!("Dictionary::read: {:?}", t2.elapsed());
}
```

Run: `cargo test the_real_asset_unpacks_and_loads -- --nocapture`
Expected: PASS, printing three timings and the three member names. **If `Dictionary::read` fails**, the v0.5.0 asset is not readable by the 0.5.2 crate in this tree — stop and report; the spec's §3.1 pin has to change.

Record the xz and zstd timings. If xz takes more than ~30 s, say so in the commit body: the spec chose `lzma-rs` on licence, and a pathological decoder is worth knowing about before Task 2's tests are written around it.

- [ ] **Step 6: Confirm the fixture stack round-trips**

```rust
#[test]
fn ruzstd_encodes_what_ruzstd_decodes() {
    let plain = b"round trip me".repeat(500);
    let packed = ruzstd::encoding::compress_to_vec(&plain[..], ruzstd::encoding::CompressionLevel::Fastest);
    let mut out = Vec::new();
    let mut d = ruzstd::decoding::StreamingDecoder::new(&packed[..]).unwrap();
    std::io::copy(&mut d, &mut out).unwrap();
    assert_eq!(out, plain);
}
```

Run: `cargo test ruzstd_encodes_what_ruzstd_decodes`
Expected: PASS. If `compress_to_vec`'s signature differs (a level argument, or a `Vec` sink), record the real one — Task 1's fixture builder uses it. If `ruzstd` has no usable encoder, add `zstd = "0.13"` (MIT) as a **dev-dependency only** and note the change in the spec's §9.1.

- [ ] **Step 7: Record the answers and delete the probe**

Write one line per confirmed fact into the Task 1 commit body. Delete the probe directory. Nothing from it is committed.

---

## Task 1: The crate, its constants, and `published`

**Files:**
- Modify: `Cargo.toml` (workspace members)
- Create: `crates/vibrato-source/Cargo.toml`
- Create: `crates/vibrato-source/src/lib.rs`

**Interfaces:**
- Consumes: Task 0's confirmed licences.
- Produces: `ASSET_URL`, `ASSET_SHA256`, `ASSET_BYTES: u64`, `DICT_DIR`, `SYSTEM_DIC`, `SYSTEM_DIC_ZST`, `COPYING`, `NOTICE`, `PARTIAL_SUFFIX`, `DOWNLOAD_ATTEMPTS`, `RETRY_BACKOFF`, `enum SourceError`, `fn published(&Path) -> Option<PathBuf>`, `fn staging_names(&Path) -> (PathBuf, PathBuf)`.

- [ ] **Step 1: Add the crate to the workspace**

`Cargo.toml`:

```toml
[workspace]
members = ["crates/jparser", "crates/jmdict-source", "crates/vibrato-source", "xtask", "src-tauri"]
resolver = "2"
```

- [ ] **Step 2: Write the crate manifest**

`crates/vibrato-source/Cargo.toml`:

```toml
[package]
name = "vibrato-source"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
# GitHub serves no plain HTTP, so unlike `jmdict-source` this crate needs TLS.
# `rustls` is MIT OR Apache-2.0 OR ISC, taken under MIT — but its default crypto
# providers are NOT usable here: `ring` is "Apache-2.0 AND ISC" and `aws-lc-sys`
# carries an "AND Apache-2.0" term, and an AND offers no compatible option under
# this project's GPL-2.0-only. Hence `rustls-no-provider` plus the provider below.
ureq = { version = "3", default-features = false, features = [
    "rustls-no-provider",
    "platform-verifier",
] }
# Apache-2.0 OR ISC OR MIT-0, taken under ISC. Pure Rust, x86_64 and aarch64.
rustls-graviola = "0.4"
# MIT. Pure-Rust xz, so no C toolchain enters any platform's build.
lzma-rs = "0.3"
# MIT OR Apache-2.0, taken under MIT.
tar = "0.4"
# MIT. Pure-Rust zstd.
ruzstd = "0.9"
# MIT OR Apache-2.0, taken under MIT.
sha2 = "0.11"
thiserror = "1"

[dev-dependencies]
# Fixtures are built, never committed: `SystemDictionaryBuilder` makes the same
# two-entry dictionary `jparser`'s `hints.rs` uses in its own tests.
# `default-features = false` drops vibrato's `train` feature, as jparser does.
vibrato = { version = "0.5", default-features = false }
```

- [ ] **Step 3: Write the failing test**

`crates/vibrato-source/src/lib.rs`, tests module (the rest of the file follows in Step 5):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vibrato-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn an_absent_directory_is_not_published() {
        let dir = scratch("published-absent");
        assert_eq!(published(&dir), None);
    }

    /// The shape a killed unpack leaves: the directory exists, the dictionary
    /// does not. Treating that as published is how a truncated file reaches
    /// `VibratoTokenizer::load`.
    #[test]
    fn a_directory_without_the_dictionary_is_not_published() {
        let dir = scratch("published-empty");
        std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
        assert_eq!(published(&dir), None);
    }

    #[test]
    fn a_complete_directory_is_published() {
        let dir = scratch("published-complete");
        std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
        std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"not really a dictionary")
            .expect("write");
        assert_eq!(published(&dir), Some(dir.join(DICT_DIR).join(SYSTEM_DIC)));
    }

    /// A staging directory is never resolved — it is either an unpack in
    /// progress or one that failed, and `published` must not see either.
    #[test]
    fn a_staging_directory_is_not_published() {
        let dir = scratch("published-staging");
        let (_, staging_dir) = staging_names(&dir);
        std::fs::create_dir_all(&staging_dir).expect("staging");
        std::fs::write(staging_dir.join(SYSTEM_DIC), b"partial").expect("write");
        assert_eq!(published(&dir), None);
    }

    #[test]
    fn the_pin_matches_the_measured_asset() {
        assert_eq!(ASSET_BYTES, 7_704_680);
        assert_eq!(ASSET_SHA256.len(), 64);
        assert!(ASSET_SHA256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(ASSET_URL.starts_with("https://"), "GitHub serves no plain HTTP");
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p vibrato-source`
Expected: FAIL to compile — `published`, `staging_names` and the constants do not exist.

- [ ] **Step 5: Write the module**

`crates/vibrato-source/src/lib.rs`, above the tests module:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Obtaining the compiled Vibrato dictionary that backs MeCab boundary hints.
//!
//! 2C's `VibratoTokenizer::load` wants one uncompressed `system.dic`. Vibrato
//! distributes it as a zstd-compressed member of a `.tar.xz`, and vibrato's own
//! API refuses compressed input, so unwrapping both layers is this crate's job.
//!
//! Modelled on `jmdict-source`, and deliberately independent of `jparser` for
//! the same reason: it takes the directory it is given and knows nothing about
//! how an application lays out its data. One difference of substance — the
//! asset is a single immutable GitHub release file, so it is pinned by
//! SHA-256, where JMdict (rebuilt daily) can only be checked structurally.

use std::path::{Path, PathBuf};

pub mod extract;
pub mod fetch;

/// The pinned release asset. HTTPS is not a preference: GitHub does not serve
/// this file over plain HTTP, which is why this crate carries a TLS stack and
/// `jmdict-source` deliberately does not.
pub const ASSET_URL: &str =
    "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz";

/// SHA-256 of [`ASSET_URL`]'s body, lowercase hex. Checked before any
/// decompressor sees a byte, so a substituted or truncated archive fails at
/// the cheapest point rather than somewhere inside three nested formats.
pub const ASSET_SHA256: &str =
    "4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9";

/// Measured size of the pinned asset. Not a security control — [`ASSET_SHA256`]
/// is — but a mismatch identifies a wrong-file download in the error message
/// without making the reader compare two hex strings.
pub const ASSET_BYTES: u64 = 7_704_680;

/// The published directory's name, inside the caller's hints directory. Named
/// for the asset so a future dictionary lands beside this one instead of
/// overwriting it.
pub const DICT_DIR: &str = "ipadic-mecab-2_7_0";

/// The published dictionary: decompressed, exactly what `VibratoTokenizer::load`
/// and `TA_HINTS_DICT` expect.
pub const SYSTEM_DIC: &str = "system.dic";

/// The archive member [`SYSTEM_DIC`] is decompressed from.
pub const SYSTEM_DIC_ZST: &str = "system.dic.zst";

/// IPADIC's licence and attribution files, published beside the dictionary
/// because IPADIC requires attribution and the release check in Phase 6C will
/// want them. Copying them while the tar is open costs nothing.
pub const COPYING: &str = "COPYING";
/// See [`COPYING`].
pub const NOTICE: &str = "NOTICE";

/// Marks a download or an unpack that is in progress or unverified. The
/// writing process's PID follows it, so two concurrent runs against one hints
/// directory stage into different names instead of one truncating the other.
///
/// **These accumulate.** A killed unpack leaves a staging directory holding up
/// to ~46 MB that nothing sweeps — deliberately, for the reason
/// `jmdict-source`'s own `PARTIAL_SUFFIX` gives: a process cannot tell
/// another's live staging area from a dead one's leftovers. They are inert,
/// never resolved, so the cost is disk rather than correctness.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Download attempts before [`fetch::fetch_with_retry`] gives up.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Delay before the second attempt; doubles for each attempt after it.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("hints io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloading the vibrato dictionary failed: {0}")]
    Transport(String),
    #[error("the dictionary server returned HTTP {status} for {url}")]
    Http {
        status: u16,
        /// The URL that was requested. A 4xx means the pinned release asset
        /// moved or was deleted, and a report without the URL leaves nothing
        /// to check by hand.
        url: String,
        /// Where the dictionary would have been published. Named for the same
        /// reason `jmdict-source` names its source directory: the message is
        /// shown to someone who has to decide what to do next.
        hints_dir: PathBuf,
    },
    #[error(
        "the downloaded archive is not the pinned dictionary: expected SHA-256 \
         {expected}, got {actual}"
    )]
    Hash { expected: String, actual: String },
    /// One of the three nested formats refused the bytes. A single variant
    /// rather than three, because no user acts differently on a bad xz stream
    /// than on a bad zstd frame; the string says which layer failed.
    #[error("the archive could not be unpacked: {0}")]
    Archive(String),
    #[error("the archive carried no {name}")]
    MissingMember { name: String },
    #[error(
        "could not obtain the vibrato dictionary after {attempts} attempts ({last})"
    )]
    TooManyAttempts {
        attempts: usize,
        hints_dir: PathBuf,
        /// The final attempt's failure, rendered. Attempts can be exhausted by
        /// a transport error, a 5xx, a hash mismatch or a corrupt archive, and
        /// those are four different things for a user to act on.
        last: String,
    },
}

/// The published dictionary in `hints_dir`, or `None`. Never touches the
/// network.
///
/// Phase 5B calls this to decide whether the MeCab toggle can be offered at
/// all, which is why it answers without a `Result`: "not there" is not a
/// failure, it is the first-run state.
pub fn published(hints_dir: &Path) -> Option<PathBuf> {
    let path = hints_dir.join(DICT_DIR).join(SYSTEM_DIC);
    path.is_file().then_some(path)
}

/// This process's staging archive and staging directory inside `hints_dir`.
///
/// Both carry the PID, so two concurrent runs never share either one. Both are
/// hidden behind a leading dot and neither is ever resolved by [`published`].
pub fn staging_names(hints_dir: &Path) -> (PathBuf, PathBuf) {
    let pid = std::process::id();
    (
        hints_dir.join(format!(".{DICT_DIR}.tar.xz{PARTIAL_SUFFIX}.{pid}")),
        hints_dir.join(format!(".{DICT_DIR}{PARTIAL_SUFFIX}.{pid}")),
    )
}
```

Add empty `pub mod extract;` / `pub mod fetch;` files for now so the crate compiles:

```bash
printf '' > crates/vibrato-source/src/extract.rs
printf '' > crates/vibrato-source/src/fetch.rs
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vibrato-source`
Expected: PASS, 5 tests.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock crates/vibrato-source
git commit -m "feat(5a): add the vibrato-source crate skeleton and its pin"
```

The commit body carries Task 0's confirmed facts: the licences that resolved, the two decode timings, and that `vibrato` 0.5.2 read the v0.5.0 dictionary.

---

## Task 2: Unpack and publish, with no network at all

**Files:**
- Create: `crates/vibrato-source/src/extract.rs`
- Modify: `crates/vibrato-source/src/lib.rs` (add `publish`)
- Create: `crates/vibrato-source/tests/support/mod.rs`
- Create: `crates/vibrato-source/tests/extract.rs`

**Interfaces:**
- Consumes: Task 1's constants and `SourceError`.
- Produces: `extract::unpack_into(archive: &Path, staging: &Path) -> Result<(), SourceError>` and `publish(staging: &Path, target: &Path) -> Result<(), SourceError>`.

This task is the whole archive half. It never opens a socket, so it can be reviewed and tested on its own.

- [ ] **Step 1: Write the fixture builders**

`crates/vibrato-source/tests/support/mod.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Fixtures, built in memory.
//!
//! Nothing here is committed as a file. The dictionary is the same two-entry
//! IPADIC-shaped one `jparser`'s `hints.rs` builds in its own tests, pushed
//! back out through the exact stack the production code reads: vibrato →
//! zstd → tar → xz. A fixture that is a real archive of a real dictionary is
//! the only kind that can prove the unpacker end to end.

use std::io::Write;
use std::path::{Path, PathBuf};

/// A compiled two-entry dictionary, serialized as vibrato writes it.
pub fn dictionary_bytes() -> Vec<u8> {
    const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                       都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
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
    .expect("the fixture dictionary must build");

    let mut out = Vec::new();
    dict.write(&mut out).expect("the fixture dictionary must serialize");
    out
}

/// zstd-compress `plain` the way the release asset's `system.dic.zst` is.
pub fn zstd(plain: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(plain, ruzstd::encoding::CompressionLevel::Fastest)
}

/// A `.tar.xz` holding `members`, each `(path_in_archive, bytes)`.
///
/// Paths are written verbatim, including ones that try to escape — that is what
/// makes the traversal test meaningful.
pub fn tar_xz(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, bytes) in members {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, bytes.as_slice())
            .expect("append");
    }
    let tar_bytes = builder.into_inner().expect("finish tar");

    let mut xz = Vec::new();
    lzma_rs::xz_compress(&mut tar_bytes.as_slice(), &mut xz).expect("xz compress");
    xz
}

/// The archive the real release publishes: three members under one directory.
pub fn real_shaped_archive() -> Vec<u8> {
    tar_xz(&[
        ("ipadic-mecab-2_7_0/COPYING", b"IPADIC licence text".to_vec()),
        ("ipadic-mecab-2_7_0/NOTICE", b"IPADIC notice text".to_vec()),
        (
            "ipadic-mecab-2_7_0/system.dic.zst",
            zstd(&dictionary_bytes()),
        ),
    ])
}

/// A clean scratch directory, removed and recreated per test.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vibrato-source-it-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Write `bytes` to `dir/name` and return the path.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("create");
    file.write_all(bytes).expect("write");
    path
}
```

`support/mod.rs` needs `tar`, `lzma-rs`, `ruzstd` and `vibrato` at test time. `tar`, `lzma-rs` and `ruzstd` are already normal dependencies; `vibrato` is the dev-dependency Task 1 added.

- [ ] **Step 2: Write the failing tests**

`crates/vibrato-source/tests/extract.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The archive half: unpacking and publishing, with no socket in sight.

mod support;

use support::*;
use vibrato_source::{extract, publish, SourceError, COPYING, DICT_DIR, NOTICE, SYSTEM_DIC};

/// The central proof of this task: what comes out of the unpacker is a file
/// vibrato itself will load.
#[test]
fn a_real_shaped_archive_yields_a_loadable_dictionary() {
    let dir = scratch("unpack-happy");
    let archive = write(&dir, "asset.tar.xz", &real_shaped_archive());
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    let dic = staging.join(SYSTEM_DIC);
    assert!(dic.is_file(), "the dictionary must be written decompressed");
    let file = std::fs::File::open(&dic).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file))
        .expect("vibrato must read what the unpacker wrote");

    assert!(staging.join(COPYING).is_file(), "attribution must survive");
    assert!(staging.join(NOTICE).is_file(), "attribution must survive");
}

#[test]
fn bytes_that_are_not_xz_are_an_archive_error_not_an_io_error() {
    let dir = scratch("unpack-notxz");
    let archive = write(&dir, "asset.tar.xz", b"<html><body>404</body></html>");

    let err = extract::unpack_into(&archive, &dir.join("staging"))
        .err()
        .expect("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
}

#[test]
fn an_archive_without_the_dictionary_names_the_missing_member() {
    let dir = scratch("unpack-nomember");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[("ipadic-mecab-2_7_0/COPYING", b"licence".to_vec())]),
    );

    let err = extract::unpack_into(&archive, &dir.join("staging"))
        .err()
        .expect("must fail");
    match err {
        SourceError::MissingMember { name } => assert_eq!(name, "system.dic.zst"),
        other => panic!("got {other:?}"),
    }
}

/// The trust boundary. The archive is pinned and hashed, but the extractor
/// must not depend on that: nothing derived from an entry's own path may reach
/// the filesystem, or a later phase that relaxes the pin inherits a hole.
#[test]
fn a_traversing_entry_writes_nothing_outside_the_staging_directory() {
    let dir = scratch("unpack-traversal");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[
            ("../escaped.txt", b"nope".to_vec()),
            ("ipadic-mecab-2_7_0/system.dic.zst", zstd(&dictionary_bytes())),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(!dir.join("escaped.txt").exists(), "an entry escaped staging");
    assert!(staging.join(SYSTEM_DIC).is_file(), "the dictionary must still land");
}

/// A future release adding a README must not fail the fetch.
#[test]
fn an_unexpected_member_is_skipped_not_fatal() {
    let dir = scratch("unpack-extra");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[
            ("ipadic-mecab-2_7_0/README.md", b"hello".to_vec()),
            ("ipadic-mecab-2_7_0/system.dic.zst", zstd(&dictionary_bytes())),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(!staging.join("README.md").exists(), "only the three known names");
    assert!(staging.join(SYSTEM_DIC).is_file());
}

/// A member whose zstd frame is damaged. The dictionary is the one member
/// whose content is decompressed, so this is the only place a corrupt frame
/// can surface.
#[test]
fn a_corrupt_zstd_member_is_an_archive_error() {
    let dir = scratch("unpack-badzstd");
    let mut packed = zstd(&dictionary_bytes());
    let n = packed.len();
    packed[n / 2] ^= 0xff;
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[("ipadic-mecab-2_7_0/system.dic.zst", packed)]),
    );

    let err = extract::unpack_into(&archive, &dir.join("staging"))
        .err()
        .expect("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
}

#[test]
fn publishing_renames_the_staging_directory_into_place() {
    let dir = scratch("publish-happy");
    let staging = dir.join("staging");
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"dictionary").expect("write");

    publish(&staging, &dir.join(DICT_DIR)).expect("publish");

    assert!(dir.join(DICT_DIR).join(SYSTEM_DIC).is_file());
    assert!(!staging.exists(), "the staging directory must not survive");
}

/// The benign race: another process published first. The rename fails on
/// Windows against an existing directory, and that must read as success, not
/// as a failed download.
#[test]
fn publishing_over_an_existing_dictionary_succeeds_and_clears_staging() {
    let dir = scratch("publish-race");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the winner").expect("write");

    let staging = dir.join("staging");
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the loser").expect("write");

    publish(&staging, &target).expect("a published rival is not a failure");

    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("read"),
        b"the winner",
        "the already-published copy must win"
    );
    assert!(!staging.exists(), "the losing staging directory must be removed");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p vibrato-source --test extract`
Expected: FAIL to compile — `extract::unpack_into` and `publish` do not exist.

- [ ] **Step 4: Write the extractor**

`crates/vibrato-source/src/extract.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Unwrapping the archive: xz, then tar, then zstd.
//!
//! `vibrato::Dictionary::read` takes a plain compiled dictionary, and vibrato's
//! README is explicit that its API will not decompress one ("you must
//! decompress them outside of the API"). Both layers are therefore unwrapped
//! here, and the dictionary is written out decompressed so nothing downstream —
//! `VibratoTokenizer::load`, `TA_HINTS_DICT`, `parse --hints` — has to learn a
//! second thing a dictionary file can be.

use std::io::Write;
use std::path::Path;

use crate::{SourceError, COPYING, NOTICE, SYSTEM_DIC, SYSTEM_DIC_ZST};

/// Unpack the `.tar.xz` at `archive` into `staging`.
///
/// Only three members are written, matched by **basename**, to paths this
/// function builds. Nothing derived from an entry's own path reaches the
/// filesystem: the archive is pinned and hashed today, but the extractor's
/// safety must not depend on a constant somewhere else in the crate, or a
/// later phase that repoints the pin or adds a hand-placed archive inherits a
/// hole rather than a guard.
///
/// An unrecognised member is skipped rather than refused — a future release
/// that adds a README must not fail the fetch. A missing [`SYSTEM_DIC_ZST`] is
/// [`SourceError::MissingMember`].
pub fn unpack_into(archive: &Path, staging: &Path) -> Result<(), SourceError> {
    // The xz layer is decoded to memory rather than streamed: `xz_decompress`
    // wants a `BufRead` and a `Write`, the decompressed tar is ~8 MB, and the
    // 46 MB dictionary inside it is still streamed to disk below.
    let mut tar_bytes = Vec::new();
    {
        let mut input = std::io::BufReader::new(std::fs::File::open(archive)?);
        lzma_rs::xz_decompress(&mut input, &mut tar_bytes)
            .map_err(|e| SourceError::Archive(format!("xz: {e}")))?;
    }

    std::fs::create_dir_all(staging)?;

    let mut found = false;
    let mut tar = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = tar
        .entries()
        .map_err(|e| SourceError::Archive(format!("tar: {e}")))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| SourceError::Archive(format!("tar: {e}")))?;
        // The borrow of `entry` for its path ends with this block, so the entry
        // can be read from below.
        let name = {
            let path = entry
                .path()
                .map_err(|e| SourceError::Archive(format!("tar: {e}")))?;
            match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_owned(),
                None => continue,
            }
        };

        match name.as_str() {
            SYSTEM_DIC_ZST => {
                let mut out = std::io::BufWriter::new(std::fs::File::create(
                    staging.join(SYSTEM_DIC),
                )?);
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut entry)
                    .map_err(|e| SourceError::Archive(format!("zstd: {e}")))?;
                std::io::copy(&mut decoder, &mut out)
                    .map_err(|e| SourceError::Archive(format!("zstd: {e}")))?;
                out.flush()?;
                found = true;
            }
            COPYING | NOTICE => {
                let mut out =
                    std::io::BufWriter::new(std::fs::File::create(staging.join(&name))?);
                std::io::copy(&mut entry, &mut out)?;
                out.flush()?;
            }
            _ => continue,
        }
    }

    if !found {
        return Err(SourceError::MissingMember {
            name: SYSTEM_DIC_ZST.to_string(),
        });
    }
    Ok(())
}
```

- [ ] **Step 5: Write `publish`**

Append to `crates/vibrato-source/src/lib.rs`, above the tests module:

```rust
/// Move a completed staging directory to its published name.
///
/// This is the only moment anything appears at the resolved path, which is what
/// makes a killed process harmless: until the rename, `published` sees nothing.
///
/// A rename onto an existing directory fails — on Windows always, on Unix
/// unless the target is empty — and that failure is the benign race: another
/// process finished first. A target holding a dictionary therefore reads as
/// success, and this process's staging directory is removed. The worst case is
/// last-writer-wins between two copies of the same pinned artifact, not a
/// corrupt one.
///
/// The staging directory always lives inside the same hints directory, so this
/// rename never crosses a filesystem — `fs::rename` returns `EXDEV` across
/// devices and does not fall back to copying.
pub fn publish(staging: &Path, target: &Path) -> Result<(), SourceError> {
    match std::fs::rename(staging, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            if target.join(SYSTEM_DIC).is_file() {
                // Best-effort: a leftover staging directory is inert, and
                // failing here would report a download failure that did not
                // happen.
                let _ = std::fs::remove_dir_all(staging);
                Ok(())
            } else {
                Err(SourceError::Io(e))
            }
        }
    }
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vibrato-source`
Expected: PASS — 5 unit tests from Task 1, 8 integration tests here.

- [ ] **Step 7: Commit**

```bash
git add crates/vibrato-source
git commit -m "feat(5a): unpack the vibrato archive and publish it atomically"
```

---

## Task 3: The download, the hash, and the retry policy

**Files:**
- Create: `crates/vibrato-source/src/fetch.rs`
- Modify: `crates/vibrato-source/src/lib.rs` (add `resolve`)
- Create: `crates/vibrato-source/tests/download.rs`

**Interfaces:**
- Consumes: Task 1's constants and errors, Task 2's `extract::unpack_into` and `publish`.
- Produces: `fetch::fetch_with_retry(url: &str, sha256: &str, hints_dir: &Path, backoff: Duration) -> Result<PathBuf, SourceError>`, `fetch::fetch_from(url, sha256, hints_dir)`, and `resolve(hints_dir: &Path) -> Result<PathBuf, SourceError>`.

- [ ] **Step 1: Write the failing tests**

`crates/vibrato-source/tests/download.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The fetch half, against a local listener. Nothing here reaches the internet:
//! the real asset is fetched once, by hand, and recorded in the handoff.

mod support;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use support::*;
use vibrato_source::{fetch, published, resolve, SourceError, DICT_DIR, SYSTEM_DIC};

/// A one-shot server that answers every request with `status` and `body`,
/// counting the connections it accepted.
fn serve(status: u16, body: Vec<u8>, requests: Arc<AtomicUsize>) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            requests.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let head = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}/asset.tar.xz"), handle)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The phase's central proof: bytes off a socket become a dictionary vibrato
/// loads, published under the resolved name.
#[test]
fn a_served_archive_is_downloaded_unpacked_and_published() {
    let dir = scratch("fetch-happy");
    let archive = real_shaped_archive();
    let hash = sha256_hex(&archive);
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, archive, requests.clone());

    let path = fetch::fetch_with_retry(&url, &hash, &dir, Duration::from_millis(1))
        .expect("fetch");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file)).expect("vibrato must read it");
    assert_eq!(requests.load(Ordering::SeqCst), 1, "one attempt, no retries");
}

/// Nothing at the resolved name may be a staging leftover.
#[test]
fn a_failed_fetch_leaves_nothing_published() {
    let dir = scratch("fetch-failed");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(&url, &sha256_hex(b"whatever"), &dir, Duration::from_millis(1))
        .err()
        .expect("must fail");

    assert!(matches!(err, SourceError::TooManyAttempts { .. }), "got {err:?}");
    assert_eq!(published(&dir), None, "a failure must publish nothing");
}

#[test]
fn a_body_with_the_wrong_hash_is_rejected() {
    let dir = scratch("fetch-badhash");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let err = fetch::fetch_with_retry(
        &url,
        "0000000000000000000000000000000000000000000000000000000000000000",
        &dir,
        Duration::from_millis(1),
    )
    .err()
    .expect("must fail");

    // A wrong hash is retried — a truncated or proxy-mangled body is exactly
    // what it detects, and that is worth a second attempt — so the terminal
    // error is `TooManyAttempts` carrying the hash failure as its `last`.
    match err {
        SourceError::TooManyAttempts { last, attempts, .. } => {
            assert_eq!(attempts, 3);
            assert!(last.contains("SHA-256"), "got {last}");
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(requests.load(Ordering::SeqCst), 3, "a hash mismatch is retried");
}

#[test]
fn a_404_fails_immediately_without_retrying() {
    let dir = scratch("fetch-404");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(404, b"gone".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(&url, &sha256_hex(b"x"), &dir, Duration::from_millis(1))
        .err()
        .expect("must fail");

    match err {
        SourceError::Http { status, url: reported, .. } => {
            assert_eq!(status, 404);
            assert_eq!(reported, url, "the message must name what was requested");
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(requests.load(Ordering::SeqCst), 1, "a 4xx will not change on a retry");
}

#[test]
fn a_5xx_exhausts_the_attempts() {
    let dir = scratch("fetch-500");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(&url, &sha256_hex(b"x"), &dir, Duration::from_millis(1))
        .err()
        .expect("must fail");

    assert!(matches!(err, SourceError::TooManyAttempts { attempts: 3, .. }), "got {err:?}");
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

/// `resolve` must answer from disk when the dictionary is already published —
/// asserted by the server never being connected to, not by timing.
#[test]
fn resolve_returns_a_published_dictionary_without_downloading() {
    let dir = scratch("resolve-published");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");

    let path = resolve(&dir).expect("resolve");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
}

/// A staging directory from a killed run is not a published dictionary, and a
/// fresh fetch must publish over it rather than resolve it.
#[test]
fn a_staging_directory_does_not_satisfy_resolve() {
    let dir = scratch("resolve-staging");
    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"partial").expect("write");

    assert_eq!(published(&dir), None);

    let archive = real_shaped_archive();
    let hash = sha256_hex(&archive);
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, archive, requests.clone());

    fetch::fetch_with_retry(&url, &hash, &dir, Duration::from_millis(1)).expect("fetch");
    assert!(published(&dir).is_some());
}

/// Exercises the real `ureq` limiting mechanism against a tiny limit rather
/// than moving 32 MiB through a test socket. Asserting only `is_err()` would
/// not distinguish a rejected body from a silently truncated one.
#[test]
fn a_body_over_the_limit_is_rejected_not_truncated() {
    let dir = scratch("fetch-limit");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, vec![b'x'; 4096], requests.clone());

    let err = fetch::download_and_verify_for_test(
        &url,
        &dir.join("staging.tar.xz"),
        &sha256_hex(b"x"),
        Duration::from_secs(30),
        8,
    )
    .err()
    .expect("must fail");

    assert!(matches!(err, SourceError::Transport(_)), "got {err:?}");
}

/// Before this, ureq's defaults leave every timeout unset except `await_100`,
/// so a server that accepts and never answers hangs the call forever.
#[test]
fn a_stalled_server_is_rejected_by_the_timeout() {
    let dir = scratch("fetch-stall");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            std::thread::sleep(Duration::from_millis(300));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        }
    });

    let err = fetch::download_and_verify_for_test(
        &format!("http://{addr}/asset.tar.xz"),
        &dir.join("staging.tar.xz"),
        &sha256_hex(b"x"),
        Duration::from_millis(50),
        32 * 1024 * 1024,
    )
    .err()
    .expect("a stalled server must not hang forever");

    assert!(!matches!(err, SourceError::Http { .. }), "got {err:?}");
    handle.join().expect("server thread");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vibrato-source --test download`
Expected: FAIL to compile — `fetch::fetch_with_retry`, `fetch::download_and_verify_for_test` and `resolve` do not exist.

- [ ] **Step 3: Write the fetcher**

`crates/vibrato-source/src/fetch.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Downloading the archive, and proving it is the pinned one before unpacking.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::Digest;

use crate::{
    extract, publish, staging_names, SourceError, ASSET_SHA256, ASSET_URL, DICT_DIR,
    DOWNLOAD_ATTEMPTS, SYSTEM_DIC,
};

/// Global timeout for one download attempt, via `ureq`'s `timeout_global` —
/// "end-to-end, from DNS lookup to finishing reading the response body". Every
/// field of `ureq`'s `Timeouts` defaults to `None` except `await_100`, so
/// without this a server that completes the handshake and then stalls hangs
/// the call forever, with no error and no retry.
///
/// 120 seconds bounds the **whole call**, not idle time: the measured asset is
/// 7,704,680 bytes = ~62 Mbit, so this sets an effective sustained-throughput
/// floor of ~0.51 Mbit/s. Raise it before assuming a slow link is at fault.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound on the response body, via `ureq`'s `BodyWithConfig::limit`.
/// 32 MiB is a little over 4x the measured asset — headroom for a future
/// dictionary, while bounding a runaway body to tens of megabytes.
///
/// This is a first line, not the verification: the SHA-256 check below is what
/// decides whether the bytes are the pinned archive.
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;

/// Bytes hashed per read. Large enough that a 7.7 MB file is ~118 reads.
const HASH_CHUNK: usize = 64 * 1024;

/// An agent carrying this crate's TLS decision.
///
/// `rustls` with an **injected** provider: its defaults are `ring`
/// ("Apache-2.0 AND ISC") or `aws-lc-rs` (whose `-sys` crate carries an "AND
/// Apache-2.0" term), and an AND leaves no option compatible with this
/// project's GPL-2.0-only. `rustls-graviola` is taken under ISC. Roots come
/// from the OS trust store rather than a vendored bundle.
///
/// Do not "simplify" this to ureq's default features: that silently takes
/// `ring` back, and `ci.yml`'s TLS gate is the only thing that would notice.
fn agent(timeout: Duration) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .unversioned_rustls_crypto_provider(Arc::new(rustls_graviola::default_provider()))
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();

    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .tls_config(tls)
            .build(),
    )
}

/// SHA-256 of `path`, lowercase hex.
///
/// Read in chunks rather than through `io::copy` into the hasher: the digest
/// crates' `io::Write` impl is feature-gated, and a 46 MB read has no business
/// depending on which features happened to resolve.
fn sha256_hex(path: &Path) -> Result<String, SourceError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Download `url` to `staging`, then prove it is the archive `sha256` names.
///
/// `timeout` and `max_bytes` are parameters rather than reads of the constants
/// above purely so tests can exercise the real timeout and limiting mechanisms
/// against a tiny stall and a tiny body.
fn download_and_verify(
    url: &str,
    staging: &Path,
    sha256: &str,
    timeout: Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    let mut response = match agent(timeout).get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(SourceError::Http {
                status,
                url: url.to_string(),
                hints_dir: staging
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
            })
        }
        Err(e) => return Err(SourceError::Transport(e.to_string())),
    };

    {
        let mut body = response.body_mut().with_config().limit(max_bytes).reader();
        let mut file = std::io::BufWriter::new(std::fs::File::create(staging)?);
        // A mid-transfer disconnect, a stall past `timeout`, and a body over
        // `max_bytes` all surface here as an `io::Error`. All three are
        // transport failures rather than a bad archive, and the message should
        // not lie about which layer failed.
        std::io::copy(&mut body, &mut file).map_err(|e| SourceError::Transport(e.to_string()))?;
        std::io::Write::flush(&mut file)?;
    }

    let actual = sha256_hex(staging)?;
    if actual != sha256 {
        return Err(SourceError::Hash {
            expected: sha256.to_string(),
            actual,
        });
    }
    Ok(())
}

/// [`download_and_verify`] with the constants injected.
///
/// **Not part of the supported surface.** It exists so the timeout and body-cap
/// tests can drive the real `ureq` mechanisms against tiny values instead of
/// moving 32 MiB through a test socket.
#[doc(hidden)]
pub fn download_and_verify_for_test(
    url: &str,
    staging: &Path,
    sha256: &str,
    timeout: Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    download_and_verify(url, staging, sha256, timeout, max_bytes)
}

/// One attempt: download, verify, unpack, publish. [`fetch_with_retry`] adds
/// the retry policy.
///
/// Both staging names carry this process's PID, so two concurrent runs against
/// one hints directory never share a path. Neither is ever resolved by
/// [`crate::published`], so a killed run cannot be mistaken for a finished one.
pub fn fetch_from(url: &str, sha256: &str, hints_dir: &Path) -> Result<PathBuf, SourceError> {
    std::fs::create_dir_all(hints_dir)?;
    let (staging_archive, staging_dir) = staging_names(hints_dir);

    let result = download_and_verify(
        url,
        &staging_archive,
        sha256,
        DOWNLOAD_TIMEOUT,
        MAX_ARCHIVE_BYTES,
    )
    .and_then(|()| extract::unpack_into(&staging_archive, &staging_dir))
    .and_then(|()| publish(&staging_dir, &hints_dir.join(DICT_DIR)));

    // Best-effort either way: the archive has served its purpose on success,
    // and on failure a leftover is inert — never resolved — so a cleanup
    // failure changes nothing a caller can act on.
    let _ = std::fs::remove_file(&staging_archive);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    result.map(|()| hints_dir.join(DICT_DIR).join(SYSTEM_DIC))
}

/// Download the dictionary into `hints_dir`, retrying transient failures.
pub fn fetch(hints_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(ASSET_URL, ASSET_SHA256, hints_dir, crate::RETRY_BACKOFF)
}

/// [`fetch`] with the URL, hash and backoff injected, so tests can point at a
/// local listener without sleeping at the production backoff.
pub fn fetch_with_retry(
    url: &str,
    sha256: &str,
    hints_dir: &Path,
    backoff: Duration,
) -> Result<PathBuf, SourceError> {
    let mut delay = backoff;
    let mut last = String::new();

    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        match fetch_from(url, sha256, hints_dir) {
            Ok(path) => return Ok(path),
            // A 4xx means the pinned asset moved or was deleted, and a local
            // I/O failure will not free disk or fix permissions. Surface both
            // immediately rather than sleeping first.
            Err(e @ SourceError::Http { status: 400..=499, .. }) => return Err(e),
            Err(e @ SourceError::Io(_)) => return Err(e),
            Err(e) => {
                last = e.to_string();
                if attempt < DOWNLOAD_ATTEMPTS {
                    std::thread::sleep(delay);
                    delay = delay.saturating_mul(2);
                }
            }
        }
    }

    Err(SourceError::TooManyAttempts {
        attempts: DOWNLOAD_ATTEMPTS,
        hints_dir: hints_dir.to_path_buf(),
        last,
    })
}
```

- [ ] **Step 4: Write `resolve`**

Append to `crates/vibrato-source/src/lib.rs`, above the tests module:

```rust
/// The published dictionary in `hints_dir`, downloading it if absent.
///
/// The steady-state path is a single `is_file` — nothing here touches the
/// network once the dictionary exists, which is what lets Phase 5B call it on
/// a toggle without thinking about cost.
pub fn resolve(hints_dir: &Path) -> Result<PathBuf, SourceError> {
    match published(hints_dir) {
        Some(path) => Ok(path),
        None => fetch::fetch(hints_dir),
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p vibrato-source`
Expected: PASS — 5 unit, 8 extract, 9 download.

- [ ] **Step 6: Check clippy**

Run: `cargo clippy -p vibrato-source --all-targets -- -D warnings`
Expected: clean. Do not add `#[allow]`s to get there.

- [ ] **Step 7: Commit**

```bash
git add crates/vibrato-source Cargo.lock
git commit -m "feat(5a): download and verify the pinned vibrato dictionary"
```

---

## Task 4: `jparser-cli fetch-hints`

**Files:**
- Modify: `crates/jparser/Cargo.toml`
- Modify: `crates/jparser/src/bin/jparser-cli.rs:123-133` (the `Parse` variant's `hints` doc) and the `Command` enum

**Interfaces:**
- Consumes: `vibrato_source::{fetch::fetch, published}`.
- Produces: the `hints-source` feature, and a `fetch-hints` subcommand that prints the published path.

- [ ] **Step 1: Add the optional dependency and the feature**

`crates/jparser/Cargo.toml`:

```toml
[features]
default = ["cli"]
cli = ["dep:jmdict-source"]
mecab = ["dep:vibrato"]
# The `fetch-hints` driver. Deliberately NOT implied by `cli`, which is on by
# default, nor by `mecab`, which `src-tauri` enables: either would put a TLS
# stack and an archive-unpacking stack into the shell's build months before 5B
# wants them there. 5B adds `vibrato-source` to `src-tauri` directly, the way
# 2F added `jmdict-source`.
hints-source = ["dep:vibrato-source", "mecab"]

[dependencies]
vibrato-source = { path = "../vibrato-source", optional = true }
```

(Keep the existing dependency lines; add only the one above, in alphabetical position.)

- [ ] **Step 2: Write the failing test**

`crates/jparser/tests/cli_hints.rs`:

```rust
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `fetch-hints` reaches the crate that does the work.
//!
//! The download itself is covered in `vibrato-source`'s own suite against a
//! local listener; what is unproven here is the wiring — that the subcommand
//! exists under its feature, resolves the directory it is given, and reports a
//! failure rather than panicking. So this drives the binary against an
//! already-published dictionary, which needs no network at all.

use std::path::Path;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-cli-hints-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn fetch_hints_reports_an_already_published_dictionary_without_downloading() {
    let dir = scratch("published");
    let published = dir.join("ipadic-mecab-2_7_0");
    std::fs::create_dir_all(&published).expect("dict dir");
    std::fs::write(published.join("system.dic"), b"already here").expect("write");

    let out = cli()
        .args(["fetch-hints", dir.to_str().expect("utf-8 path")])
        .output()
        .expect("run");

    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("system.dic"),
        "the published path must be printed, got {stdout}"
    );
    assert!(
        Path::new(stdout.trim()).is_file(),
        "what is printed must be a real path, got {stdout}"
    );
}
```

Register it in `crates/jparser/Cargo.toml`, beside the existing `cli_generations` block:

```toml
[[test]]
name = "cli_hints"
path = "tests/cli_hints.rs"
required-features = ["hints-source"]
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p jparser --features hints-source --test cli_hints`
Expected: FAIL — `error: unrecognized subcommand 'fetch-hints'`.

- [ ] **Step 4: Add the subcommand**

In `crates/jparser/src/bin/jparser-cli.rs`, in `enum Command`, immediately after `FetchSource`:

```rust
    /// Download the compiled Vibrato dictionary into DIR, if it is not there
    /// already, and print the path to the published `system.dic`.
    ///
    /// The result is what `parse --hints` and `TA_HINTS_DICT` want:
    /// uncompressed, ready for `VibratoTokenizer::load`.
    #[cfg(feature = "hints-source")]
    FetchHints {
        /// Directory to publish into. Conventionally named `hints`.
        dir: PathBuf,
    },
```

And in the `match` in `main`, beside the `FetchSource` arm:

```rust
        #[cfg(feature = "hints-source")]
        Command::FetchHints { dir } => {
            let path = match vibrato_source::published(&dir) {
                Some(path) => path,
                None => vibrato_source::fetch::fetch(&dir)?,
            };
            println!("{}", path.display());
        }
```

- [ ] **Step 5: Replace `parse --hints`' obsolete instructions**

In the same file, the `hints` field of the `Parse` variant currently tells the
user to download, `tar xf` and `zstd -d` by hand. Replace that doc comment with:

```rust
        /// Path to an uncompressed compiled Vibrato dictionary. When given,
        /// tokenization supplies boundary hints to the segmenter.
        ///
        /// `jparser-cli fetch-hints <DIR>` (feature `hints-source`) downloads
        /// and publishes one; pass the `system.dic` it prints.
        #[cfg(feature = "mecab")]
        #[arg(long)]
        hints: Option<PathBuf>,
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p jparser --features hints-source`
Expected: PASS, including the new `cli_hints` test.

- [ ] **Step 7: Prove the feature gate holds both ways**

```bash
cargo tree -p jparser --no-default-features | grep -E "vibrato-source|ureq|rustls" && echo "LEAK" || echo "clean"
cargo tree -p jparser --features mecab | grep -E "vibrato-source|rustls" && echo "LEAK" || echo "clean"
```

Expected: `clean` from both. The second matters most: `src-tauri` builds `jparser` with `mecab` on top of default features, and this phase must not put a TLS stack in the shell.

- [ ] **Step 8: Commit**

```bash
git add crates/jparser Cargo.lock
git commit -m "feat(5a): add jparser-cli fetch-hints behind hints-source"
```

---

## Task 5: The CI gates

**Files:**
- Modify: `.github/workflows/ci.yml:108-123`

**Interfaces:**
- Consumes: nothing.
- Produces: two grep invariants that outlive this phase.

- [ ] **Step 1: Widen the purity grep**

The `no http client or decompressor in the library` step currently greps
`jmdict-source|ureq|flate2|vibrato`. Add the new crate:

```yaml
      - name: no http client or decompressor in the library
        run: |
          if cargo tree -p jparser --no-default-features \
             | grep -E "jmdict-source|vibrato-source|ureq|flate2|vibrato"; then
            echo "::error::jparser's library pulled in a cli-only dependency"
            exit 1
          fi
```

- [ ] **Step 2: Widen the TLS gate to the licence invariant it actually means**

```yaml
      # GPL v2: OpenSSL is Apache-2.0, which the FSF holds GPLv2-incompatible,
      # so no dependency may link native-tls/OpenSSL — now or in any later phase.
      #
      # `ring` ("Apache-2.0 AND ISC") and `aws-lc-sys` (which carries its own
      # "AND Apache-2.0" term) are refused for the same reason: an AND offers no
      # compatible option to take. Phase 5A's HTTPS therefore runs on `rustls`
      # with `rustls-graviola` (ISC) injected as the provider — if a build
      # error here tempts you to enable ureq's default features, that silently
      # takes `ring` back, and this grep is the only thing that would notice.
      - name: no GPL-incompatible TLS backend
        run: |
          if cargo tree --workspace | grep -iE "openssl|native-tls|\bring v|aws-lc"; then
            echo "::error::a GPLv2-incompatible TLS dependency entered the tree"
            exit 1
          fi
```

The `\bring v` form matches `cargo tree`'s `ring v0.17.14` rendering without
also matching every crate whose name merely contains the letters (`ring` appears
inside `string_cache`, which is in this tree).

- [ ] **Step 3: Prove both greps locally**

```bash
cargo tree -p jparser --no-default-features | grep -E "jmdict-source|vibrato-source|ureq|flate2|vibrato" && echo FAIL || echo pass
cargo tree --workspace | grep -iE "openssl|native-tls|\bring v|aws-lc" && echo FAIL || echo pass
```

Expected: `pass` from both. **If the second prints a `ring` line**, the provider
injection in Task 3 did not take effect — stop and fix `fetch::agent` before
continuing; the spec's §6 does not survive `ring` in the tree.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(5a): gate the new crate's purity and the crypto provider"
```

---

## Task 6: The real asset, once, by hand

**Files:** none. This task produces evidence.

**Interfaces:**
- Consumes: everything above.
- Produces: the measured numbers Task 7's handoff records.

CI never downloads the dictionary. This is the one run that proves the pinned
constants describe reality, and it happens on Windows because that is what the
project ships.

- [ ] **Step 1: Fetch it**

```bash
cargo run -p jparser --features hints-source -- fetch-hints "$LOCALAPPDATA/clipyomi-hints-check"
```

Expected: a printed path ending `ipadic-mecab-2_7_0/system.dic`. Time the run.

- [ ] **Step 2: Confirm what landed**

```bash
ls -l "$LOCALAPPDATA/clipyomi-hints-check/ipadic-mecab-2_7_0"
```

Expected: `system.dic` at exactly 47,788,814 bytes, plus `COPYING` and `NOTICE`.
No `.partial` leftovers.

- [ ] **Step 3: Prove the second run costs nothing**

```bash
cargo run -p jparser --features hints-source -- fetch-hints "$LOCALAPPDATA/clipyomi-hints-check"
```

Expected: the same path, returned immediately, with no network traffic.

- [ ] **Step 4: Prove the dictionary works where it has to**

```bash
cargo run -p jparser --features hints-source -- parse <index-dir> "東京都に住んでいます" \
  --hints "$LOCALAPPDATA/clipyomi-hints-check/ipadic-mecab-2_7_0/system.dic"
```

Use the index at `%APPDATA%/dev.jparser.clipyomi/dict/gen-<n>`. Expected: a
segmentation, with no error from `VibratoTokenizer::load`.

- [ ] **Step 5: Prove the env override still works**

```bash
TA_HINTS_DICT="$LOCALAPPDATA/clipyomi-hints-check/ipadic-mecab-2_7_0/system.dic" \
  cargo run -p clipyomi
```

Expected: the app starts and parses. 5A changed nothing about `TA_HINTS_DICT`,
and this is what proves it.

- [ ] **Step 6: Record**

Write down: the fetch wall time, the published sizes, and whether steps 3–5
behaved. These go into the handoff.

---

## Task 7: The gate and the handoff

**Files:**
- Create: `docs/superpowers/2026-08-27-jparser-phase5a-handoff.md`

**Interfaces:**
- Consumes: Task 6's measurements, Task 0's baseline counts.
- Produces: the phase's closing record.

- [ ] **Step 1: Run the whole gate**

```bash
cargo test --workspace
cargo test -p jparser --features hints-source
cargo test -p jparser --features mecab
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p jparser --no-default-features --all-targets
npx vitest run
npx playwright test
```

Expected: all green. The Vitest and Playwright counts must equal Task 0's
baselines — this phase touches no frontend file, so a change there is a
regression, not progress.

- [ ] **Step 2: Confirm the clippy debt did not move**

Run: `cargo clippy --all-targets 2>&1 | grep -c "^warning"`
Expected: unchanged from Task 0's baseline. `crates/vibrato-source` contributes
zero warnings; if it contributes any, fix them rather than allowing them.

- [ ] **Step 3: Write the handoff**

`docs/superpowers/2026-08-27-jparser-phase5a-handoff.md`, following the shape of
`docs/superpowers/2026-08-26-jparser-phase4d-handoff.md`:

1. **What shipped** — one numbered entry per task, with the commit hash.
2. **The measured facts** — Task 0's decode timings, Task 6's wall time and
   sizes, and the confirmation that vibrato 0.5.2 reads the v0.5.0 asset.
3. **What 5B inherits** — `published()` as the network-free availability check,
   `resolve()` as the toggle's entry point, and the fact that `src-tauri` has no
   dependency on `vibrato-source` yet.
4. **What is deliberately absent** — the toggle, startup resolution, progress
   reporting, staging-directory sweeping, and a hand-placed-archive path, each
   with the spec section that argues for the absence.
5. **Deviations to fold into the master spec** — rows 16, 17 and 18 from the
   design's §11, copied into
   `docs/superpowers/specs/2026-08-12-jparser-port-design.md`'s §12 table, and
   the §11 Phase 5 table's 5A row marked shipped.

- [ ] **Step 4: Update the master spec**

In `docs/superpowers/specs/2026-08-12-jparser-port-design.md`:
- §11's Phase 5 table: mark 5A shipped, with a one-line summary in the voice of
  the Phase 4 rows.
- §12's table: append rows 16, 17 and 18 verbatim from the 5A design's §11.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers
git commit -m "docs(phase5a): record the vibrato acquisition close"
```

---

## Self-Review Notes

**Spec coverage.** Every section of the design maps to a task: §3.1's pin →
Task 1's constants and `the_pin_matches_the_measured_asset`; §3.3's uncompressed
publish → Task 2's extractor writing `system.dic`; §3.4's hash → Task 3's
`sha256_hex` and `a_body_with_the_wrong_hash_is_rejected`; §4.1–4.2's layout and
staging → Task 1's `staging_names` and Task 2's `publish`; §4.3's non-use of
generations → nothing to build, argued in the spec; §5.1's surface → Tasks 1–3;
§5.2's errors and retry policy → Task 3; §5.3's pipeline → Tasks 2 and 3;
§5.4's trust boundary → Task 2's basename whitelist and traversal test; §6's
dependencies → Task 1's manifest and Task 0's licence probe; §6.2's CI → Task 5;
§7's feature and CLI → Task 4; §8's Task 0 → Task 0; §9's verification → the
tests in Tasks 1–4 plus Tasks 6 and 7; §10's deferrals → documented, not built;
§11's deviations → Task 7 Step 4.

**Two facts this plan takes on trust from Task 0**, both with a stated fallback
rather than a guess: `RootCerts::PlatformVerifier` building against an injected
graviola provider (fallback: `RootCerts::WebPki`, and correct the spec), and
`ruzstd::encoding::compress_to_vec`'s exact signature (fallback: a dev-only
`zstd`, and correct the spec). Task 0 exists so both are settled before any
committed line depends on them.

**Naming consistency check.** `published`, `resolve`, `publish`,
`staging_names`, `extract::unpack_into`, `fetch::fetch`, `fetch::fetch_from`,
`fetch::fetch_with_retry`, `fetch::download_and_verify_for_test` — each is
defined in exactly one task and used with the same signature everywhere after
it. `SYSTEM_DIC` is the published file, `SYSTEM_DIC_ZST` the archive member;
they are never interchanged.
