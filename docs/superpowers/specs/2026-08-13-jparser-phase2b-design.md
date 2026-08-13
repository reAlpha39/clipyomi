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
    JMdict_e.gz.partial.<pid>    # transient; ignored by `resolve`
```

> **Amended after implementation (2026-08-14).** The staging file was originally
> specified as a fixed `JMdict_e.gz.partial`. That is unsafe under concurrency,
> which this design never considered: `File::create` truncates, so two processes
> against one source directory share an inode. Two orderings break §6's central
> guarantee — one process verifies, the second truncates and begins writing, and
> the first's `fs::rename` then publishes the second's half-written bytes; and
> separately, `rename` does not invalidate the second's open descriptor, so after
> any successful publish it keeps writing into the *published* archive. Both
> poison the resolved name, which `resolve` deliberately never re-verifies, so
> every later run fails identically with no automatic recovery. Appending
> `std::process::id()` confines each process to its own path, and the worst case
> degrades to "last writer wins with a valid archive." The cost is that an
> interrupted download now leaves an orphan no later run reuses; previously the
> next run truncated the one fixed name. A blind sweep of `*.partial.*` would
> reintroduce the race, so the orphans are left alone.

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

/// Suffix for an in-progress or unverified download. Never resolved. The
/// staging file is `<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>` — see §3.
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

> **Amended after implementation (2026-08-14).** Three corrections to this block.
>
> **`resolve` needed the same seam `fetch` has.** A `resolve_from(url,
> source_dir, backoff)` was added, with `resolve` delegating to it using the
> production URL and `RETRY_BACKOFF`. Without it two of §8's required tests were
> untestable offline: both expect `resolve` to *fail*, and reached that failure
> only by falling through to `fetch` and assuming no route to the host. On a
> connected machine the fall-through **succeeds** — measured, it downloaded the
> real 10,545,887-byte archive in 35.79 s and the test then failed because
> `resolve` returned `Ok`. So the suite passed only where `ftp.edrdg.org` was
> unreachable, and violated §9's no-network rule everywhere else. The seam also
> bought coverage the design never had: `resolve`'s *download* branch had no
> offline test at all.
>
> **`fetch_from` and `fetch_with_retry` are `pub`, not private.** Integration
> tests are separate crates and cannot reach a private item. Their doc comments
> say they are not part of the supported surface; the alternative — moving those
> tests into `#[cfg(test)]` units — would lose real-process coverage.
>
> **A request timeout and a body cap were added**, as `DOWNLOAD_TIMEOUT` (120 s,
> via `timeout_global`) and `MAX_ARCHIVE_BYTES` (32 MiB, via
> `body_mut().with_config().limit()`). `ureq` 3.2.1 leaves every `Timeouts` field
> `None` but `await_100`, so a peer that completes the handshake and then stalls
> mid-body hung `fetch` **forever** — inside 2A's lazy closure, which means a
> later phase calling this at startup would hang on launch rather than fail.
> `as_reader()` is likewise unbounded. A stall now surfaces as `Transport`, which
> the §6 table already retries. Worst-case block is bounded but long:
> 3 × 120 s + 2 s + 4 s ≈ 366 s, with no progress signal.

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
2. Otherwise download to `<source_dir>/JMdict_e.gz.partial.<pid>`, verify it
   (§6), rename it to `JMdict_e.gz`, and open that.

A `.partial` file is **never** resolved and never verified in place. Its presence
does not satisfy step 1.

> **Amended after implementation (2026-08-14).** This paragraph originally ended
> "and step 2 overwrites it." It no longer does: since the staging name now
> carries the process id (§3), step 2 overwrites only *this* process's own
> staging file. Another process's — or a dead process's — is left untouched.
> Neither is ever resolved, so the guarantee this paragraph exists to state is
> unchanged; only the cleanup behavior differs.

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
    #[error(
        "the dictionary server returned HTTP {status} for {url}; \
         place a JMdict_e.gz in {source_dir} manually to bypass the download"
    )]
    Http {
        status: u16,
        url: String,
        source_dir: PathBuf,
    },
    /// A downloaded body that is not a well-formed, complete gzip archive.
    /// Distinct from `Io` because the retry policy inverts between them: bad
    /// bytes are worth another attempt, a failed local write is not.
    #[error("the downloaded dictionary was not a valid archive: {0}")]
    Corrupt(String),
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

> **Amended after implementation (2026-08-14).** Two corrections, both forced by
> §6's retry table.
>
> **`Corrupt` was added, and it is not optional.** As originally written,
> `verify_archive` reported a decode failure as `Io` through `#[from]`, while the
> table makes `Io` non-retryable ("retrying will not free disk") and verification
> failures retryable ("the bytes were bad, the URL was not"). Those contradict
> outright: a corrupt body would have returned immediately instead of retrying.
> Splitting the variant resolves it and leaves `Io` meaning genuinely local
> failure — every remaining `?`-to-`Io` is `File::open`, `create_dir_all`,
> `File::create`, `flush`, or `rename`.
>
> **`Http` carries the URL and the source directory.** Only `TooManyAttempts`
> originally named the hand-placement escape hatch, but §6 fails a 4xx
> *immediately* — and a 404 is the likeliest permanent real failure, since EDRDG
> can rename or move the archive, which this project has already been bitten by
> once. The user saw `the dictionary server returned HTTP 404` with no URL, no
> directory, and no recovery route: the design's declared recovery path was
> invisible on the failure most likely to need it. One known wart: when attempts
> are exhausted by 5xx, `last` captures a rendered `Http` that already contains
> the escape-hatch clause, so `TooManyAttempts` prints it twice.

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
| An immediate 4xx also names the URL and the source directory | §7: a 404 never reaches `TooManyAttempts`, so without this the likeliest permanent failure carries no recovery route |
| `ensure-dictionary --source-dir` builds an index end to end **through the CLI** | The route this phase exists to add; the two rejection tests die inside `clap` before `main` runs, and the seam test bypasses `clap` entirely, so nothing else covers it |
| The rejection tests assert on stderr, not just a non-zero exit | Otherwise they pass against a binary that never learned `--source-dir`, for the wrong reason |
| End to end: `resolve` feeds `ensure_dictionary` and an index opens | The seam is the deliverable; this is the only test that proves the two crates compose |

> **Amended after implementation (2026-08-14).** Four notes on this table.
>
> **Two rows were untestable as written.** "A present source file ⇒ zero HTTP
> requests" and "a `.partial` alone does not satisfy `resolve`" both reached
> their conclusion by falling through to `fetch` and assuming the host was
> unreachable — see §4's amendment for the measured 10.5 MB download that proved
> otherwise. Both now drive `resolve_from` against a `TcpListener`.
>
> **Three rows were added** (marked above): the 4xx message, the `--source-dir`
> CLI route, and the stderr assertions. The first two are gaps this section
> simply missed; the CLI route is the phase's headline capability and had no
> coverage at all until it was caught in review.
>
> **The end-to-end test lives in `crates/jmdict-source/tests/seam.rs`**, not in
> `crates/jparser`, with `jparser` as a `dev-dependency` carrying
> `default-features = false`. The direction matters: `jmdict-source` must stay
> usable without `jparser`, and the flag keeps `jparser`'s `cli` feature from
> pulling `jmdict-source` back in.
>
> **The fixture is ~60 bytes, not 60 MB** — one entry, gzipped in-test.

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
  (§10). Verify `cargo tree` shows no `openssl-sys`.

  > **Amended after implementation (2026-08-14).** This bullet originally kept
  > `rustls` "so that an EDRDG certificate fix, an HTTPS mirror, or an HTTP→HTTPS
  > redirect does not become a dependency decision under time pressure." The
  > feature is now **off** (`default-features = false`, no TLS feature at all).
  > It was not paying for itself: `rustls` pulled `zeroize`, whose 1.9.0 requires
  > edition2024 and therefore Rust 1.85, breaking the MSRV floor below. Pinning
  > `zeroize` in `Cargo.lock` would have held it only until the next bare
  > `cargo update`, and pinning it in a manifest means an unused direct
  > dependency. Dropping the feature removes `rustls`, `ring`, and `zeroize` from
  > the graph outright — no pin needed anywhere — and costs only a capability
  > this crate cannot currently use. Restoring TLS is one feature flag if EDRDG
  > ever fixes its certificate.

- **MSRV 1.75, compile-verified.** `io::Error::other` (1.74) is in bounds.

  > **Amended after implementation (2026-08-14).** This bullet originally said
  > "only review catches violations," and that assumption was exactly wrong —
  > review had already missed one for three phases. Installing a 1.75 toolchain
  > and running `cargo +1.75 check` found that `crates/jparser` itself did not
  > compile on its own declared floor: `clap 4.6.6` pulls `clap_lex 1.1.0`, which
  > requires edition2024 and so Rust 1.85. It built fine only because the
  > installed toolchain is 1.97.1. `clap` is now pinned `=4.5.51` and
  > `cargo +1.75 check --workspace` passes. **Adopting `resolver = "3"` is not an
  > alternative** — Cargo 1.75 rejects the manifest outright (`` `resolver`
  > setting `3` is not valid ``), so the resolver setting and the compile check
  > are mutually exclusive. The compile check is the stronger of the two and wins.

- **`crates/jparser` stays pure.** No Tauri, no UI crate, no HTTP client, no
  decompression dependency **in the library**.

  > **Amended after implementation (2026-08-14).** "This phase adds nothing to
  > its `Cargo.toml`" was unachievable: `jparser-cli` lives inside
  > `crates/jparser`, so §4's `--source-dir` wiring necessarily puts
  > `jmdict-source` in that manifest. Resolved with an `optional` dependency
  > behind a default-on `cli` feature plus `required-features` on the `[[bin]]`
  > and the `[[test]]` — which turns the rule from a promise into a
  > compile-checked property: `cargo check -p jparser --no-default-features
  > --all-targets` succeeds and `cargo tree -p jparser --no-default-features`
  > shows no `jmdict-source`, `ureq`, or `flate2`.
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

**1. Dependency versions and MSRV — pinning *is* needed.**

| Crate | Version | `rust-version` | License |
|---|---|---|---|
| `ureq` | 3.2.1 | 1.71.1 | MIT OR Apache-2.0 |
| `flate2` | 1.1.9 | 1.67.0 | MIT OR Apache-2.0 |

Both are under the workspace's 1.75 floor. Confirm `cargo tree` shows no
`openssl-sys`.

> **Amended after implementation (2026-08-14).** This item was the most
> consequential wrong call in the design, and its own evidence was
> misinterpreted. It originally read "no pinning needed" and concluded, from
> `cargo` having selected `ureq` 3.2.1 over the newer 3.4.0, that "MSRV-aware
> resolution is doing this, so the plan must **not** pin a version by hand."
>
> MSRV-aware resolution was *not* doing it — this workspace is `resolver = "2"`,
> which ignores `rust-version` entirely when choosing versions. 3.2.1 was in the
> lockfile for unrelated reasons. A caret `ureq = "3"` or `"3.2"` resolves
> straight to 3.4.0, whose floor is 1.85, and compiles clean on the installed
> 1.97.1 toolchain — so nothing would have reported the violation. The
> requirement is pinned `~3.2` for exactly this reason.
>
> The same mistake was already latent elsewhere in the tree: `clap` had drifted
> to 4.6.6, pulling `clap_lex 1.1.0` (edition2024, Rust 1.85), so `crates/jparser`
> had not compiled on its declared floor for three phases. Now pinned `=4.5.51`.
> A range does not suffice — `clap_builder` adopted `clap_lex 1.x` partway
> through the 4.5 line — and constraining `clap_lex` directly does not either,
> since cargo permits the 0.7 and 1.x majors side by side and `clap_builder`
> simply keeps its own copy.
>
> **The general rule this replaces:** under `resolver = "2"`, a dependency's
> `rust-version` is documentation, not a constraint. Every direct dependency
> whose upstream may raise its floor needs an explicit bound, and the only thing
> that actually proves the floor holds is `cargo +<MSRV> check --workspace`.

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
