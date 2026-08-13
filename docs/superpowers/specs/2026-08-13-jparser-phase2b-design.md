# JParser Phase 2B — Dictionary Acquisition Design

Getting JMdict bytes to the seam Phase 2A left open. Nothing else.

2A shipped `ensure_dictionary(root, table, opts, keep, source)` where
`source: FnOnce() -> io::Result<impl BufRead>`, and its only caller passes
`|| File::open(path).map(BufReader::new)`. This phase supplies the real producer:
a local file if the user has one, a download if not.

**Reference:** `docs/superpowers/specs/2026-08-13-jparser-phase2a-design.md` for
the seam and the generation layout, and
`docs/superpowers/2026-08-13-jparser-phase2a-handoff.md` for what 2A actually
shipped, which differs from its own spec in seven ruled places. The C++ original
in `ta-old/` is **read-only — never modify it**; it downloaded EDICT2 rather than
JMdict and is not a reference for this phase.

---

## 1. Scope

| In | Out |
|---|---|
| A new `crates/jmdict-source` crate | Anything Tauri |
| Resolving a local source file, or downloading one | The clipboard poll |
| gzip decode, and rejecting a corrupt archive | The latest-wins parse worker |
| Wiring both into `jparser-cli` so the path is exercisable by hand | Settings persistence, window, `RwLock<Arc<Index>>` |

**Deferred deliberately:** resume-on-retry. A failed download restarts from the
beginning. The archive is ~10 MB compressed; a full retry is cheap, and resume
requires either server `Range` support or a persistent partial artifact with its
own staleness and cleanup rules. Neither earns its place until someone reports
the retry hurting.

---

## 2. Supersessions

**2A's spec §1 scope table lists Phase 2B as "App shell: clipboard poll,
latest-wins parse worker, commands, settings, window" and separately assigns it
"HTTP, the EDRDG URL, resume-on-retry, gzip decode and integrity check." That is
three independent subsystems in one phase. This document takes only
acquisition.**

Acquisition is the piece that can be built and tested headlessly, under the
crate's 80% coverage rule, with no Tauri and no UI. The shell and the interaction
core are where 2A's own spec quotes the port design saying coverage is *"not
meaningful in `src-tauri`, which is glue and window plumbing"* — mixing the two
in one phase would put a network-I/O correctness path and untested glue behind
the same gate. They become their own phases, against a working acquisition path.

**2A's spec §4 sketches this phase's call as
`|| Ok(BufReader::new(GzDecoder::new(response)))` — decoding straight off the
HTTP response. This document does not do that.** Streaming the response into the
parser cannot satisfy the manual-file requirement (§3), and it gives the
integrity check nowhere to run before the index is built. The response is written
to disk, verified, and then read. §4's `DictSource`-trait rejection still stands
and is not revisited: the closure remains the seam.

**Resume-on-retry is dropped**, per §1.

---

## 3. Disk layout

The source directory is a **sibling** of 2A's generation root, not a child of it:

```
<data-dir>/
  dictionary/                    # 2A's generation root — derived, sweepable
    gen-1/
    gen-2/
  source/                        # input — never swept, may be hand-placed
    JMdict_e.gz                  # the resolved source (~10 MB)
    JMdict_e.gz.partial          # transient; ignored by `resolve`
```

Two reasons, the second load-bearing.

**Lifetimes differ.** A generation is derived data: disposable, regenerable from
source. A hand-placed source file is input, and is irreplaceable if EDRDG is
unreachable when the user next needs it. Putting irreplaceable input inside the
directory designed to be swept inverts which of the two is precious.

**Nesting would create an undeclared dependency.** `jmdict-source` has no
`jparser` in its `Cargo.toml`, which is the point. But a `resolve(root)` that
knew to look in a subdirectory *of the generation root* would depend on
`jparser`'s layout convention anyway — coupling the compiler cannot see, which
breaks silently if that layout ever changes. Taking its own directory, the crate
knows nothing about generations.

For the record, nesting would have been *safe* against 2A as shipped: `sweep`
collects only names `generation_number` accepts plus `.build-*` orphans and
leaves everything else alone (`sweep_ignores_malformed_names` asserts this with
an `unrelated/` directory), and `latest` and `gen-list` both filter on
`GENERATION_PREFIX`. Safety was never the deciding argument; coupling was.

---

## 4. Module surface

### `crates/jmdict-source` (new crate)

Depends on `ureq` and `flate2`. Does **not** depend on `jparser`, and `jparser`
does not depend on it. The caller wires them.

```rust
/// Directory name callers conventionally use. Not enforced — `resolve` takes
/// whatever directory it is given.
pub const SOURCE_DIR: &str = "source";

/// Resolved source file within the source directory.
pub const SOURCE_FILE: &str = "JMdict_e.gz";

/// Suffix for an in-progress or unverified download. Never resolved.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Attempts before a download is abandoned.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Base backoff between attempts; doubles each time.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);

/// Open the JMdict source in `source_dir`, downloading it first if absent.
///
/// Returns decompressed XML regardless of whether the file on disk is gzipped.
pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>>;

/// Download only, to `<source_dir>/JMdict_e.gz`. Public so the CLI can
/// pre-fetch without building an index.
pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError>;

// Private. `fetch` delegates with the production URL; tests call this with a
// `TcpListener`'s address (§8). The seam is a parameter rather than an env var
// or a `OnceCell` override, so no test can perturb another's configuration and
// nothing about the production path is reachable only at runtime.
fn fetch_from(url: &str, source_dir: &Path) -> Result<PathBuf, SourceError>;
```

The production URL lives in one private constant that `fetch` passes to
`fetch_from`. It is deliberately **not** a public constant: exposing it would
invite a caller to fetch it themselves and bypass the staging and verification
in §6, which are the entire point.

`resolve` returns `io::Result` so it drops into 2A's closure with no mapping at
the call site. A `SourceError` is wrapped with `io::Error::other` (stable 1.74,
inside MSRV) and stays reachable through `Error::source()`, so the typed error is
not lost.

`Box<dyn BufRead>` rather than a generic, because the returned type genuinely
differs between the gzip and plain cases and no single generic parameter covers
both. `std` implements `BufRead for Box<B> where B: BufRead + ?Sized`, so it
satisfies 2A's `R: BufRead` bound directly. Every reader it boxes owns its
source — a `File`, or `ureq`'s response body — so the default `'static` bound
holds. One dynamic dispatch against a ~60 MB parse is not measurable.

### `crates/jparser/src/bin/jparser-cli.rs` (modified)

```
jparser-cli ensure-dictionary <root> [xml] [--source-dir <dir>] [--keep N]
jparser-cli fetch-source <dir>
```

**Both source forms stay, and `<xml>` keeps its existing position.** It becomes
optional, `--source-dir` is added beside it, and a clap `ArgGroup` requires
exactly one — so every invocation 2A accepts still works unchanged.

An earlier draft replaced the positional. That was wrong: 2A's two committed CLI
tests (`ensure_dictionary_builds_once_and_then_reuses` and
`ensure_dictionary_rejects_a_zero_keep`) pass it positionally, so replacing it
would make a phase about downloading edit tests about generations as collateral.
Both forms also earn their keep — `--source-dir` is the product behavior, and an
explicit `xml` is what you want when pointing at a two-entry fixture instead of a
~10 MB download.

`fetch-source` downloads without building, so the download path is exercisable on
its own.

---

## 5. Resolution order

`resolve(source_dir)`:

1. If `<source_dir>/JMdict_e.gz` exists, open it.
2. Otherwise download to `<source_dir>/JMdict_e.gz.partial`, verify it
   (§6), rename it to `JMdict_e.gz`, and open that.

A `.partial` file is **never** resolved and never verified in place. Its presence
does not satisfy step 1, and step 2 overwrites it.

**Exactly one filename is resolved — `SOURCE_FILE` — and its extension is not
trusted.** An earlier draft accepted `JMdict_e.xml` as a second name; that is
rejected. Two accepted names require a precedence rule, and a precedence rule
between a compressed and an uncompressed copy of the same dictionary is a
question with no good answer when both exist and disagree. One name plus content
sniffing has neither problem: a user who decompressed the archive by hand keeps
the `.gz` name and it still works. The name is a location, not a claim about
encoding — `resolve`'s doc comment says so, since it is mildly surprising.

Opening therefore sniffs rather than trusting the extension:

```rust
let mut reader = BufReader::new(File::open(path)?);
let is_gzip = matches!(reader.fill_buf()?, [0x1f, 0x8b, ..]);
```

`fill_buf` peeks without consuming, so the bytes remain available to whichever
reader is constructed. A file shorter than two bytes is not gzip and falls
through to the plain path, where the XML parser rejects it.

---

## 6. Download, verification, and the rename

The download stages and publishes by rename, for the same reason 2A's generations
do: **a file at the resolved name must never be anything but a complete, valid
archive.**

Two distinct failures make this necessary, and only the first is obvious.

**A killed process** leaves a truncated file. Without staging, that truncation
sits at the resolved name, is indistinguishable from a manual drop, and is opened
on every subsequent run.

**A complete response that is not the archive** is not caught by staging alone.
The realistic case is not bit-rot: it is a **proxy or captive portal returning an
HTML error page with HTTP 200 and a correct `Content-Length`** — ordinary on hotel
and corporate networks, and EDRDG is a plain academic mirror with no
authentication to fail loudly. A server replacing its own copy mid-publish
produces the same shape. Those bytes would be renamed into place, fail the build,
and fail identically forever, because `resolve` cannot tell them from a
hand-placed file.

**A `Content-Length` check was considered and rejected as the primary guard.** It
is nearly free and it does catch truncation, but it passes the captive-portal case
above — the very scenario most likely to occur. So the archive is **verified
before the rename**, by decoding it fully and discarding the output:

```rust
// After the download completes, before the rename.
let mut sink = std::io::sink();
std::io::copy(&mut GzDecoder::new(File::open(&partial)?), &mut sink)?;
```

gzip carries a CRC32 and an uncompressed length in its trailer, which
`flate2::read::GzDecoder` checks at end of stream. A mismatch or a truncation
surfaces as an `io::Error` here, the `.partial` is removed, and the attempt counts
as a failure — turning a permanent wedge into a retry. The cost is one extra
inflate pass over ~10 MB, well under a second, once per rebuild.

Verification runs only on a **downloaded** archive. A hand-placed file is not
pre-verified: doing so would double the read on every rebuild to guard against a
mistake the user made deliberately, and the failure mode is already correct —
`build_from_reader` returns the decode error, `build_new` publishes nothing, and
2A's `sweep` reclaims the partial build directory.

### Retry policy

Up to `DOWNLOAD_ATTEMPTS`, with `RETRY_BACKOFF` doubling between them. Each
attempt restarts from byte zero.

| Outcome | Action |
|---|---|
| Transport error (connect, reset, timeout) | Retry |
| 5xx | Retry |
| Verification failure (§6) | Retry — the bytes were bad, the URL was not |
| 4xx | Fail immediately. Retrying a 404 only waits. |
| Local write error (disk full, permissions) | Fail immediately. Retrying will not free disk. |

---

## 7. Error handling

```rust
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
         place a JMdict_e.gz in {source_dir} manually to bypass the download"
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
```

`Transport` carries a `String` rather than the client's error type, so
`SourceError` does not leak `ureq` into this crate's public API — the client
becomes swappable without a breaking change.

`TooManyAttempts` names the directory and the manual escape hatch, because that is
the one action a user can take when the network is the problem. This follows 2A's
`GenerationExists` precedent: an error that names the operator's next move rather
than only the failure. It carries `last` because §6's retry table exhausts
attempts on three unrelated causes — a message that named only one of them would
misdirect the user on the other two.

No new `IndexError` variant. `jparser` does not learn that downloads exist.

---

## 8. Testing

80% line coverage on `crates/jmdict-source`, measured the same way as `jparser`:
`cargo llvm-cov -p jmdict-source --summary-only --fail-under-lines 80`. This is a
library crate, not glue, so the rule applies in full.

**No test touches the network.** The download path is driven against a one-shot
`std::net::TcpListener` bound to port 0, speaking a hardcoded HTTP/1.1 response
from a thread — no mock framework and no new dependency. The URL under test is a
parameter so the listener's address can be injected; the production constant is
asserted separately for shape, not fetched.

Temp directories use
`std::env::temp_dir().join(format!("jmdict-source-test-{name}"))` +
`let _ = std::fs::remove_dir_all(&dir);`. There is no `tempfile` crate and none is
coming.

Required assertions:

| Behavior | Why it needs a test |
|---|---|
| gzip magic ⇒ decoded; plain XML ⇒ passed through; 1-byte file ⇒ plain path | The sniff is the only thing standing between a mislabeled file and a confusing parse error |
| A present source file ⇒ **zero** HTTP requests | The download must not fire when a local copy exists; assert a request counter, as 2A asserted its `source` call counter |
| A `.partial` alone does **not** satisfy `resolve` | This is the killed-download case; without it, step 1 would resolve a truncation |
| A corrupt archive fails verification, is removed, and does not land at the resolved name | This is §6's whole argument, and it is the claim this design could not verify offline |
| An HTTP 200 whose body is HTML with a correct `Content-Length` fails verification and leaves nothing at the resolved name | The captive-portal case, and the single scenario that decides §6 against the cheaper `Content-Length` guard. If this passes without the inflate pass, §6's cost is unjustified and should be revisited |
| `ensure-dictionary` still accepts `<xml>` positionally, and rejects being given both `<xml>` and `--source-dir` | §4 keeps both forms specifically so 2A's committed CLI tests are untouched; the `ArgGroup` is what makes "exactly one" true rather than intended |
| A truncated archive (valid header, missing trailer) fails the same way | CRC and length are checked at EOF, so a truncation that never reaches EOF must not pass silently |
| 4xx fails without retrying; a transport error retries up to `DOWNLOAD_ATTEMPTS` | The two halves of §6's table; collapsing them is the likely slip |
| `TooManyAttempts` names the source directory | The message is the user's only instruction when the network is down |
| End to end: `resolve` feeds `ensure_dictionary` and an index opens | The seam is the deliverable; this is the only test that proves the two crates compose |

The end-to-end test lives in `crates/jparser`'s integration tests or a new
workspace-level test, because it is the one place both crates appear — and it uses
a tiny XML fixture, gzipped in-test, never the real 60 MB file.

---

## 9. Constraints inherited

From the Phase 1B and 2A handoffs and the crate's standing rules:

- **GPL v2.** Every new source file carries the standard header, verbatim from
  `crates/jparser/src/index/mod.rs:1-6`. **This constrains the dependency tree as
  a standing rule:** OpenSSL is Apache-2.0, which the FSF holds
  GPLv2-incompatible, so no dependency may link `native-tls`/OpenSSL — now or in
  any later phase. `ureq` uses `rustls` (MIT/Apache-2.0/ISC) for this reason.
  Note the rule is *not* load-bearing for the current URL, which is plain HTTP
  (§10); rustls is kept anyway so that an EDRDG certificate fix, an HTTPS mirror,
  or an HTTP→HTTPS redirect does not become a dependency decision under time
  pressure. Verify `cargo tree` shows no `openssl-sys`.
- **MSRV 1.75.** The installed toolchain is 1.97.1 and will accept newer APIs, so
  only review catches violations. `io::Error::other` (1.74) is in bounds.
- **`crates/jparser` stays pure.** No Tauri, no UI crate, no HTTP client, no
  decompression dependency. This phase adds nothing to its `Cargo.toml`.
- **Errors are explicit.** No `unwrap()`/`expect()`/`unreachable!()` in library
  code outside `#[cfg(test)]`. Never swallow an error without a comment naming the
  reason.
- **No magic numbers**, no bare string literals for names that have constants.
- **File size** 200-400 lines typical, 800 hard maximum including tests.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. Never `cargo fmt`,
  which cascades into `jparser`'s `conjugation.rs` — deliberately not
  rustfmt-clean, and "fixing" it is a defect.
- **Clippy:** `cargo clippy -p jmdict-source --all-targets -- -D warnings` clean.

**2A invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3;
`EntryData`'s field order is wire format; a published `gen-N` is immutable;
directory knowledge stays in `generations.rs` and `ensure_dictionary`. This phase
touches none of them — it produces a reader and knows nothing about generations.

---

## 10. Resolved facts

The three items this section originally listed as unknowns were measured on
2026-08-13, against the live registry and the live EDRDG host. They are recorded
here as facts, not assumptions.

**1. Dependency versions and MSRV — no pinning needed.**

| Crate | Version | `rust-version` | License |
|---|---|---|---|
| `ureq` | 3.2.1 | 1.71.1 | MIT OR Apache-2.0 |
| `flate2` | 1.1.9 | 1.67.0 | MIT OR Apache-2.0 |

Both are under the workspace's 1.75 floor, so MSRV 1.75 stands untouched. Note
that `cargo` selected `ureq` 3.2.1 over the newer 3.4.0 on its own —
MSRV-aware resolution is doing this, so the plan must **not** pin a version by
hand and must **not** raise the floor. Confirm `cargo tree` shows no
`openssl-sys`.

**2. `flate2` rejects every corruption mode §6 depends on** — measured, not
asserted:

| Input | Result |
|---|---|
| intact archive | `Ok` |
| corrupt CRC32 trailer | `Err(InvalidInput)` — "corrupt gzip stream does not have a matching checksum" |
| corrupt ISIZE length | `Err(InvalidInput)` — same message |
| truncated, trailer removed | `Err(UnexpectedEof)` |
| truncated mid-deflate-stream | `Err(UnexpectedEof)` |
| HTML body, not gzip at all | `Err(InvalidInput)` — "invalid gzip header" |

The last row matters twice: it is the captive-portal case §6 exists for, and it
fails at the *header*, so verification costs almost nothing when it rejects.
§8's tests still assert all of this — a measurement in a document is evidence for
a decision, not a regression guard.

**3. The URL, filename, and scheme.**

```
http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz
```

- The archive is **`JMdict_e.gz`**, not `JMdict_e.xml.gz`. `SOURCE_FILE` is
  `"JMdict_e.gz"`.
- **10,545,887 bytes** compressed, decompressing to gzipped XML that begins
  `<?xml version="1.0" encoding="UTF-8"?>`. An earlier draft of this document
  said ~35 MB; that was wrong by 3.5×, and the corrected figure makes both a
  full-restart retry (§1) and the verification pass (§6) cheaper than argued.
- **There is no usable HTTPS.** `https://ftp.edrdg.org/...` fails certificate
  validation with a subject-name mismatch. The URL is plain HTTP.
- The server sends `Accept-Ranges: bytes`, so resume *would* be implementable.
  §1 still declines it: a 10 MB restart is cheaper than the partial-artifact
  lifecycle resume requires.

**The plain-HTTP transport is why §6 is a requirement rather than insurance.**
There is no authenticity guarantee on the wire at all, so a transparent proxy or
an on-path substitution is indistinguishable from a real response until the
archive is decoded. Verifying before the rename is the only thing standing
between that and a poisoned file at the resolved name.

**Known limitation, accepted:** this phase verifies *integrity* (the bytes are a
well-formed gzip stream) but not *authenticity* (the bytes came from EDRDG).
Pinning a digest was considered and rejected — EDRDG publishes no checksum and
the file changes daily, so any pin would need a second trusted source or manual
maintenance. Recorded so a later phase can revisit it deliberately.
