# JParser Phase 5A — Vibrato Dictionary Acquisition (Design)

**Date:** 2026-08-27
**Status:** Designed, not started.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§4.4, §11, §12)
**Precedent:** 2B (`crates/jmdict-source` — download, stage, verify, publish),
2A (immutable publish), 2C (`hints.rs`, `VibratoTokenizer`, the `mecab` feature)

---

## 1. Goal

2C shipped MeCab boundary hints and left them reachable only from a terminal:
`TA_HINTS_DICT` must name an already-downloaded, already-extracted,
already-decompressed dictionary that the user obtained by hand. `parse --hints`'
own doc comment currently instructs them to run `wget`, `tar` and `zstd`.

5A removes those three commands. It adds the acquisition path — download the
pinned dictionary, prove it is the right bytes, unwrap two layers of
compression, and publish the result where a later phase can find it.

It stops there. 5B owns the toggle, the live `watch` into the worker, and the
"absent dictionary means hints off, not a fatal" rule. This phase ships a
library and a CLI driver, exactly as 2B did for JMdict four sub-phases before
2F wired it to a button.

---

## 2. Scope

**In:**

- `crates/vibrato-source`: a new crate that downloads, verifies, extracts and
  publishes the compiled Vibrato dictionary.
- A SHA-256 pin on the downloaded archive, checked before anything is unpacked.
- Directory-granularity atomic publish, so a killed process never leaves a
  half-written dictionary at the resolved name.
- `jparser-cli fetch-hints DIR`, behind a new non-default feature.
- The CI rules that keep this phase's dependencies from leaking into the parser
  library or re-introducing a GPL-incompatible TLS backend.

**Out:**

- **The MeCab toggle**, the `watch` into the worker, and every line of
  `src-tauri`. Port design §11 gives those to 5B, and §4.4's "paid for only when
  MeCab is switched on" is a statement about that toggle, not about this crate.
- **`TA_HINTS_DICT`'s behaviour.** It stays the env override it is today,
  including 2C's decision that a set-but-unreadable value is fatal.
- **Startup resolution.** `state.rs::load_state` is untouched; nothing looks for
  a published dictionary until 5B does.
- **Updating a published dictionary.** The asset is pinned by URL and hash. A
  later version is a code change, and §4.1 explains why that costs nothing on
  disk.
- **Sweeping leftover staging directories.** The same gap `jmdict-source`
  documents on `PARTIAL_SUFFIX`, for the same reason, recorded again in §10.

---

## 3. The artifact

### 3.1 What is pinned

Vibrato publishes precompiled dictionaries as release assets. The one this port
wants is IPADIC — 2C's `READING_FIELD` is IPADIC's field 7, and `hints.rs` is
written against IPADIC's feature layout, so no other resource on that page is a
drop-in.

| Fact | Value |
| ---- | ----- |
| URL | `https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz` |
| Size | 7,704,680 bytes |
| SHA-256 | `4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9` |
| Members | `ipadic-mecab-2_7_0/{COPYING, NOTICE, system.dic.zst}` |
| `system.dic.zst` | 8,045,952 bytes, SHA-256 `82a6da70bb4a17be70f20ff44f650f9ad1d2b0b4fcb2f39c17fc797f92d0ab75` |
| `system.dic` | 47,788,814 bytes |

Every number above was measured against the real asset while this design was
written, not read off a page. `system.dic`'s size in particular decides §3.3's
publish form and is not documented anywhere upstream — the `.zst` frame header
carries no content size, because it was produced by a streaming compressor.

The transport is HTTPS to `github.com`, which redirects to
`objects.githubusercontent.com`. This is the first HTTPS in the tree:
`jmdict-source` is deliberately plain HTTP because EDRDG serves no usable
certificate, and its Cargo comment says not to add a TLS feature back. That
comment governs *that* crate; §6 is this crate's answer.

### 3.2 Two layers, and why vibrato will not unwrap either

`vibrato::Dictionary::read` takes any `Read` and deserializes a plain compiled
dictionary. It has no zstd support, and vibrato's own README says so: "The
distributed models are compressed in zstd format. If you want to load these
compressed models with the `vibrato` API, you must decompress them outside of
the API." So xz and zstd are this phase's problem, not a dependency's.

### 3.3 Publishing the uncompressed dictionary

The published artifact is the **decompressed** `system.dic`, 47,788,814 bytes.

The alternative — publish the 8 MB `.zst` and teach `VibratoTokenizer::load` to
sniff zstd magic the way `jmdict_source::open_local` sniffs gzip — saves 38 MiB
of disk and costs three things this phase would rather not spend:

1. A zstd decoder inside `crates/jparser`, under `mecab`. The parser library's
   dependency list is guarded by a CI grep for exactly this class of crate.
2. A 46 MB decode on every launch with hints enabled, on top of the bincode
   deserialize that already dominates it, using a pure-Rust decoder chosen for
   its licence rather than its speed.
3. A second thing `load` can mean, in a function whose contract — "an
   **uncompressed** compiled Vibrato dictionary" — is repeated in
   `TA_HINTS_DICT`'s doc comment and in `parse --hints`' help text.

Disk is the cheap side of that trade here: a published index generation on a
development machine already measures 67 MB, beside 11 MB of retained JMdict
source. Decompressing once at fetch keeps `jparser` and its env override
untouched by this phase.

### 3.4 Why a hash here, when JMdict has none

`jmdict-source` cannot pin a hash: EDRDG rebuilds `JMdict_e.gz` daily, so the
only verification available to it is structural — decode the gzip, and let the
XML prefix check reject a saved error page. A GitHub release asset is the
opposite kind of artifact: one immutable file, already downloaded and measured,
pinned in source next to the URL that names it.

The hash also buys independence from how rigorously `lzma-rs` validates xz's
check field, which is a question this design then does not have to answer. It is
checked over the staged file **before** any decompressor sees a byte, so a
substituted or corrupted archive fails at the cheapest possible point.

`COPYING` and `NOTICE` are published beside `system.dic` because IPADIC's
licence requires attribution and 6C owes an attribution check. Copying two small
files while the tar is already open costs nothing; re-fetching them later would
not.

---

## 4. On-disk layout and publish

### 4.1 Where it goes

```
<hints_dir>/
  ipadic-mecab-2_7_0/
    COPYING
    NOTICE
    system.dic
  .ipadic-mecab-2_7_0.partial.<pid>/   # staging, never resolved
```

`hints_dir` is conventionally `<app_config_dir>/hints`, beside 2A's `dict/` and
2B's `source/`. The crate takes the directory it is given and never derives it
from an app directory — the independence rule `jmdict-source`'s module doc
states, for the reason it states: a `resolve(app_dir)` would depend on a layout
it does not own.

The version-named subdirectory is what makes §2's "no update affordance" free. A
later vibrato release, or a dictionary format bump, lands beside this one under
its own name rather than overwriting it, and the code pins which name it reads.

### 4.2 Directory-granularity staging

2B stages a file and renames it, so a file at the resolved name is always a
complete archive. This phase publishes three files, so the same guarantee needs
the rename to happen one level up: extract everything into
`.ipadic-mecab-2_7_0.partial.<pid>/`, then `fs::rename` that directory onto the
target name. Until that rename, nothing at the resolved path exists at all.

The PID is there for the reason it is there in 2B: two processes racing against
one `hints_dir` — two CLI invocations, or 5B's toggle against a CLI run — must
not share a staging path, or one truncates the other's write.

Two platform details the plan must handle rather than discover:

- `fs::rename` onto an **existing** directory fails on Windows. That is the
  benign race (another process published first), so it is resolved by
  re-checking the target: present and complete means success, and the staging
  directory is removed.
- The staging directory lives inside `hints_dir`, so the rename never crosses a
  filesystem. `fs::rename` returns `EXDEV` across devices and does not fall back
  to copying.

### 4.3 Not a numbered generation

§11's row for 5A says "verified and published like an index generation". What
2A's generations buy is that a torn build can never be served: numbered
directories, `latest`, and a sweep. Two of those three exist to manage
*rebuilds* — a rebuild happens when JMdict changes or the index format does, and
the old generation must keep serving until the new one is complete.

Nothing here rebuilds. There is one pinned artifact, and the property actually
wanted from 2A — never serve a partially written dictionary — is delivered by
§4.2's staging directory alone. Generalizing `jparser::index::generations`,
which is shaped around index rebuilds and lives inside the index module, to
carry an artifact with one immutable version would be more machinery than the
guarantee needs. Recorded as a deviation in §11.

---

## 5. The crate

### 5.1 Surface

```rust
pub const ASSET_URL: &str;      // the v0.5.0 IPADIC asset
pub const ASSET_SHA256: &str;   // lowercase hex, 64 chars
pub const ASSET_BYTES: u64;     // 7_704_680 — a cheap pre-hash mismatch signal
pub const DICT_DIR: &str;       // "ipadic-mecab-2_7_0"
pub const SYSTEM_DIC: &str;     // "system.dic"

/// The published dictionary, or `None`. Never touches the network.
pub fn published(hints_dir: &Path) -> Option<PathBuf>;

/// The published dictionary, downloading and publishing it if absent.
pub fn resolve(hints_dir: &Path) -> Result<PathBuf, SourceError>;

/// `resolve`'s fetch half, with the URL, hash and backoff injected.
/// Not part of the supported surface — a test seam, as in `jmdict-source`.
pub fn fetch_with_retry(
    url: &str,
    sha256: &str,
    hints_dir: &Path,
    backoff: Duration,
) -> Result<PathBuf, SourceError>;
```

`published` exists for 5B, which needs a network-free answer to "is MeCab
available?" before it can render a toggle. Shipping it now costs one function
and keeps 5B from reaching around this crate to stat a path itself.

### 5.2 Errors

```rust
pub enum SourceError {
    Io(std::io::Error),
    Transport(String),
    Http { status: u16, url: String, hints_dir: PathBuf },
    Hash { expected: String, actual: String },
    Archive(String),                 // xz, tar or zstd refused the bytes
    MissingMember { name: String },  // the tar carried no system.dic.zst
    TooManyAttempts { attempts: usize, hints_dir: PathBuf, last: String },
}
```

Modelled on `jmdict_source::SourceError`, including the habit of naming the
directory in the messages a user is expected to act on. `Archive` is one variant
rather than three because a user cannot act differently on a bad xz stream than
on a bad zstd frame; the rendered string says which layer failed.

The retry policy is copied outright: `DOWNLOAD_ATTEMPTS` = 3, `RETRY_BACKOFF` =
2 s doubling, a 4xx and a local `Io` failure returned immediately rather than
slept on, everything else retried. A `Hash` mismatch **is** retried — a
truncated or proxy-mangled body is exactly what it detects, and that is worth a
second attempt.

Timeouts and caps follow 2B's shape and reasoning: a 120 s `timeout_global` so a
stalled server cannot hang the call forever, and a 32 MiB body cap — a little
over 4x the measured asset — so a misbehaving response cannot stream unbounded
bytes to disk before the hash is checked.

### 5.3 The pipeline

```
GET (HTTPS, redirect followed)
  -> staging file `<hints_dir>/.ipadic-mecab-2_7_0.tar.xz.partial.<pid>`
  -> SHA-256 over the staged file            (mismatch: retry)
  -> lzma_rs::xz_decompress into a Vec       (~8 MB, the tar)
  -> tar walk, whitelisted members only
       COPYING, NOTICE   -> written verbatim into the staging directory
       system.dic.zst    -> ruzstd::decoding::StreamingDecoder
                            -> `system.dic` in the staging directory
  -> fs::rename(staging directory, `<hints_dir>/ipadic-mecab-2_7_0`)
  -> remove the staging archive
```

Peak memory is the ~8 MB decompressed tar; the 46 MB dictionary is streamed to
disk and never held. The xz layer is decoded to memory rather than streamed
because `lzma_rs::xz_decompress` takes a `BufRead` and a `Write`, and 8 MB is
smaller than the machinery that would avoid it.

### 5.4 Tar entries are a trust boundary

The archive is pinned and hashed, so its entry names are known. The extractor
still refuses to trust them: it matches each entry against the three expected
**basenames** and writes only those, into the staging directory, by a path this
crate constructs. Nothing derived from the archive reaches the filesystem.

This is not defensive dressing on a verified artifact — it is what keeps the
verification and the extraction independent. A future phase that repoints
`ASSET_URL`, relaxes the pin, or adds a hand-placed-archive path inherits a tar
extractor that cannot write outside its staging directory, rather than one whose
safety was an argument about a constant elsewhere in the file.

An entry that is not one of the three is skipped, not an error: a future release
adding a README must not fail the fetch. A **missing** `system.dic.zst` is
`MissingMember`.

---

## 6. Dependencies

Every crate below is taken under a GPL-2.0-only-compatible licence, and the
choice of TLS backend is constrained by that rather than by preference.

```toml
[dependencies]
# rustls under MIT (Apache-2.0 OR ISC OR MIT). Its default crypto providers are
# NOT usable here: `ring` is "Apache-2.0 AND ISC" and `aws-lc-sys` carries an
# "AND Apache-2.0" term — an AND offers no compatible option, and Apache-2.0
# alone is incompatible with this project's GPL-2.0-only.
ureq = { version = "3", default-features = false, features = [
    "rustls-no-provider",
    "platform-verifier",
] }
# The provider, taken under ISC (Apache-2.0 OR ISC OR MIT-0). Pure Rust,
# x86_64 and aarch64 — which is every target this project builds for.
rustls-graviola = "0.4"
lzma-rs = "0.3"      # MIT, pure Rust xz
tar = "0.4"          # MIT OR Apache-2.0, taken under MIT
ruzstd = "0.9"       # MIT, pure Rust zstd
sha2 = "0.11"        # MIT OR Apache-2.0, taken under MIT
thiserror = "1"

[dev-dependencies]
# Fixtures are built, never committed: `SystemDictionaryBuilder` makes the
# two-entry dictionary `hints.rs` already uses in its own tests.
vibrato = { version = "0.5", default-features = false }
```

### 6.1 Why not `native-tls`

`native-tls` would be one crate instead of three, and on the two platforms this
project ships — SChannel on Windows, Security.framework on macOS — it links no
OpenSSL at all. It is still rejected: `ci.yml` fails any build whose
`cargo tree --workspace` mentions it, and the comment above that gate says "no
dependency may link native-tls/OpenSSL — now or in any later phase". The gate
runs on Linux, where `native-tls` **does** mean OpenSSL, and §13's deferred
Linux build would inherit the problem the gate exists to prevent.

`rustls` + `rustls-graviola` keeps that invariant intact with no C toolchain on
any platform. ureq 3.4 accepts an injected provider through
`unversioned_rustls_crypto_provider`, and `RootCerts::PlatformVerifier` (the
`platform-verifier` feature) takes roots from the OS trust store rather than a
vendored bundle.

### 6.2 What CI must learn

Two edits to `.github/workflows/ci.yml`:

1. The purity grep gains `vibrato-source`, so `jparser --no-default-features`
   can never pull this crate in.
2. The TLS gate's pattern widens from `openssl|native-tls` to also reject `ring`
   and `aws-lc`, and its comment names `rustls-graviola` as the sanctioned
   provider. Today nothing stops a later phase from "fixing" a build error by
   enabling ureq's default features and silently taking `ring` back — that grep
   is the only place the invariant is enforced, and it currently spells out only
   half of it.

---

## 7. Feature gating and the CLI

```toml
# crates/jparser/Cargo.toml
[features]
hints-source = ["dep:vibrato-source", "mecab"]
```

Deliberately **not** implied by `cli`. `cli` is on by default, and `src-tauri`
enables `mecab` on top of default features — so folding this into either one
would put a TLS stack and an archive-unpacking stack into the shell's build months
before 5B wants them there. 5B adds `vibrato-source` to `src-tauri` directly,
the way 2F added `jmdict-source`.

```
jparser-cli fetch-hints <DIR>
```

Gated on `hints-source`, printing the published path. Mirrors `fetch-source`,
2B's driver for the same job.

`parse --hints`' doc comment — currently four lines instructing the user to
`wget`, `tar xf` and `zstd -d` by hand — is replaced by a pointer to
`fetch-hints`. Leaving it would document the workflow this phase exists to
delete.

---

## 8. Task order

**Task 0 — prove the dependencies before writing the crate.** Four facts that
change the design if any is false, each cheap to check and expensive to discover
late:

1. `ureq` + `rustls-no-provider` + `rustls-graviola` completes a TLS handshake
   against `github.com` on Windows with `RootCerts::PlatformVerifier`, including
   the redirect to `objects.githubusercontent.com`. The provider must be
   installed as the process default for `rustls-platform-verifier` to build a
   config.
2. `lzma_rs::xz_decompress` decodes the real 7.7 MB asset, and how long it
   takes. The pure-Rust decoder was chosen on licence, not speed; if it is
   pathological on this input the plan needs to know before tests are written
   around it.
3. `ruzstd` decodes `system.dic.zst` to exactly 47,788,814 bytes, and `vibrato`
   0.5.2 — the version in this tree — loads the result. The asset was published
   under the v0.5.0 tag, and nothing upstream promises cross-version dictionary
   compatibility.
4. `ruzstd::encoding::compress_to_vec` produces a frame `ruzstd`'s own decoder
   reads, so §9.1's fixture needs no second zstd crate. (It exists in 0.9.0;
   this confirms it round-trips.)

The rest follows the pipeline: constants and errors; the staging-and-publish
half against a local fixture; the fetch half against a local listener; the CLI
subcommand; the CI edits; the `parse --hints` doc replacement.

---

## 9. Verification

### 9.1 The fixture

Built in memory, never committed, following `hints.rs`' own precedent — the
two-entry `SystemDictionaryBuilder` dictionary it already uses in its tests —
pushed back through the full stack: `Dictionary::write` →
`ruzstd::encoding::compress_to_vec` → `tar::Builder` → `lzma_rs::xz_compress`. A
few kilobytes, and a real archive of a real dictionary rather than a shape that
resembles one.

### 9.2 Unit

- A hash mismatch is rejected, and the error names both hashes.
- Bytes that are not xz are `Archive`, not `Io`.
- A tar with no `system.dic.zst` is `MissingMember`.
- A tar entry named `../escape` or `/abs/path` writes nothing outside the
  staging directory.
- An unexpected extra member is skipped without failing the fetch.
- `published` returns `None` for an absent directory, `None` when the directory
  exists without `system.dic`, and the path when it is complete.
- A staging directory left by a previous run does not satisfy `published`.

### 9.3 Seam

Against a local `TcpListener`, as `jmdict-source/tests/download.rs` does, so no
test touches the real network:

- `resolve` on an empty directory downloads, publishes, and the result loads
  through `vibrato::Dictionary::read`. This is the phase's central proof.
- `resolve` on a published directory returns immediately — asserted by the
  listener never being connected to, not by timing.
- A 404 fails without retrying; a 500 exhausts the attempts and reports
  `TooManyAttempts` naming the last failure.
- A body over the cap and a stalled server are rejected rather than left
  hanging, exercising the real `ureq` mechanisms against tiny injected values.

### 9.4 Manual, once

The real 7.7 MB asset fetched end to end on Windows, the published `system.dic`
loaded through `VibratoTokenizer::load`, and `parse --hints` run against it.
Recorded in the handoff with the measured timings from Task 0. CI never
downloads it.

### 9.5 Gate

`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D
warnings`, the purity job with its widened greps, and `cargo check -p jparser
--features hints-source`. Coverage's 80% line is scoped to `crates/jparser` and
is unaffected: this crate is I/O and archive plumbing, and the master spec's §10
already declines to make a repo-wide number mean anything.

---

## 10. Deferred

- **Sweeping staging directories.** A killed fetch leaves
  `.ipadic-mecab-2_7_0.partial.<pid>/` behind, costing up to ~46 MB of disk
  permanently. `jmdict-source` documents the identical gap and the identical
  reason: a process cannot distinguish another's live staging directory from a
  dead one's leftovers, so sweeping by pattern would reintroduce the race the
  PID prevents. They are inert — never resolved — so the cost is disk, not
  correctness.
- **Progress reporting.** The download is 7.7 MB behind a toggle nobody has
  built yet; 5B decides whether its UI needs a percentage.
- **A hand-placed archive path.** `jmdict-source`'s exists because EDRDG's
  download fails in ways a user must be able to route around. GitHub over HTTPS
  has not earned that escape hatch yet, and §5.4 keeps the extractor safe if a
  later phase adds one.
- **Dictionary updates.** New version, new pin, new directory name. §4.1.

---

## 11. Deviations to record in the master spec at phase close

| # | Deviation | Why |
| - | --------- | --- |
| 16 | The Vibrato dictionary is pinned by URL **and SHA-256**, where JMdict is pinned by neither | EDRDG rebuilds `JMdict_e.gz` daily, so `jmdict-source` can only verify structurally. A GitHub release asset is one immutable file, and hashing it before unpacking makes the design independent of how strictly `lzma-rs` validates xz's check field. §3.4 |
| 17 | Published into a version-named directory, not a numbered generation, despite §11's "published like an index generation" | Generations exist so an old index can serve while a new one builds. Nothing here rebuilds — there is one pinned artifact — and the only property actually wanted, never serving a partial dictionary, comes from renaming a staging directory into place. §4.3 |
| 18 | The first HTTPS in the tree, via `rustls` + `rustls-graviola` rather than `native-tls` or rustls' default providers | GitHub serves no plain HTTP. `ring` and `aws-lc-sys` carry `AND Apache-2.0` terms with no compatible alternative, and `ci.yml`'s TLS gate forbids `native-tls` in any phase because it means OpenSSL on the deferred Linux build. Graviola is ISC and pure Rust. §6 |
