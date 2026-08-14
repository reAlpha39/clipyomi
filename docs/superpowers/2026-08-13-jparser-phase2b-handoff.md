# JParser Phase 2B — Handoff

Phase 2B supplies the JMdict bytes that 2A's `ensure_dictionary` expects — from a
hand-placed file if one exists, otherwise by downloading, verifying, and
publishing one — without `crates/jparser`'s library gaining an HTTP client or a
decompressor.

Executed from `docs/superpowers/plans/2026-08-13-jparser-phase2b.md` against
`docs/superpowers/specs/2026-08-13-jparser-phase2b-design.md`. **The spec has
since been amended** (`fa01123`) with seven corrections marked at their sites;
read the amended spec, not the original text, and prefer it over the plan
wherever they disagree.

**Branch:** `worktree-jparser-phase2b`, 11 commits off `master` @ `2571d68`,
merged at `e6b1edf`. Four follow-ups on master: `2d6a9f5` (clap MSRV),
`fa01123` (spec amendment), `6174dde` (CI), `63c9d38` (residual minors).

**Verification at close:** 292 tests passing + 1 pre-existing ignored,
`cargo clippy --workspace --all-targets -- -D warnings` clean,
`cargo +1.75 check --workspace` passes, `cargo llvm-cov` → **98.57%** lines on
`jmdict-source` and **96.37%** on `jparser` (floor 80), purity grep **0**, no
`openssl`/`native-tls` in the tree.

---

## The public surface the next phase consumes

In `jmdict_source`:

```rust
pub const SOURCE_DIR: &str = "source";        // convention, never enforced
pub const SOURCE_FILE: &str = "JMdict_e.gz";  // the ONLY resolved name
pub const PARTIAL_SUFFIX: &str = ".partial";  // staging is `<FILE><SUFFIX>.<pid>`
pub const DOWNLOAD_ATTEMPTS: usize = 3;
pub const RETRY_BACKOFF: Duration = Duration::from_secs(2);  // doubles

pub fn open_local(path: &Path) -> Result<Box<dyn BufRead>, SourceError>;
pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>>;
pub fn resolve_from(url: &str, source_dir: &Path, backoff: Duration)
    -> std::io::Result<Box<dyn BufRead>>;

pub enum SourceError { Io, Transport(String), Corrupt(String),
                       Http { status, url, source_dir }, TooManyAttempts { .. } }
```

In `jmdict_source::fetch`:

```rust
pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError>;
pub fn fetch_from(url: &str, source_dir: &Path) -> Result<PathBuf, SourceError>;
pub fn fetch_with_retry(url: &str, source_dir: &Path, backoff: Duration)
    -> Result<PathBuf, SourceError>;
```

`resolve` returns `io::Result` specifically so it drops into 2A's
`FnOnce() -> io::Result<impl BufRead>` with no mapping at the call site:

```rust
let index = jparser::index::ensure_dictionary(&root, &table, &opts, keep,
    || jmdict_source::resolve(&source_dir))?;
```

The typed error survives underneath via `io::Error::other`, reachable through
`get_ref()` and `downcast_ref::<SourceError>()`.

**`resolve_from`, `fetch_from`, and `fetch_with_retry` are `pub` only because
integration tests are separate crates.** Their doc comments say they are not the
supported surface. Production callers use `resolve` and `fetch`. `JMDICT_URL` is
`pub(crate)` on purpose — exposing it invites a caller to fetch directly and skip
the staging and verification that are the entire point.

---

## Decisions that departed from the plan and the spec

Nine, each ruled on by the human partner during execution. The reasoning and the
measurements behind them are in the amended spec; this is the index.

1. **`resolver = "3"` was not adopted** (the plan's Task 1 Step 2). Cargo 1.75
   rejects the manifest outright — `` `resolver` setting `3` is not valid `` — so
   the resolver setting and the plan's own `cargo +1.75 check` gate are mutually
   exclusive. The compile check is the stronger of the two and won.
2. **Work happened in a worktree**, not on `master`.
3. **The two constant-echo tests were kept** (`SOURCE_FILE == "JMdict_e.gz"`,
   `DOWNLOAD_ATTEMPTS == 3`). Deliberate: an earlier design draft used the wrong
   filename `JMdict_e.xml.gz`.
4. **The MSRV gate ran as `-p jmdict-source` for the whole phase**, because
   `jparser` did not compile on its own declared floor. Fixed afterwards in
   `2d6a9f5`; `--workspace` is now the correct gate.
5. **`ureq` carries no TLS feature.** `rustls` pulled `zeroize`, whose 1.9.0
   needs edition2024 and so Rust 1.85. Dropping the feature removes `rustls`,
   `ring`, and `zeroize` outright — no lockfile pin needed anywhere.
6. **An unreachable non-2xx guard was deleted.** `ureq` 3.2.1's default config
   has `http_status_as_error: true`, so any `Ok(response)` is already 2xx.
7. **`resolve_from` was added.** Two of the spec's required tests assumed the
   fall-through to `fetch` would fail for lack of a route; on a connected machine
   it *succeeded*, downloading the real 10,545,887-byte archive in 35.79 s and
   failing the test because `resolve` returned `Ok`.
8. **Two CLI tests were added** that the spec's test list omitted: the
   `--source-dir` success route, and stderr assertions on the rejection tests.
9. **The staging filename carries the PID.** See invariants below.

`SourceError::Corrupt` was the implementer's call, not a ruling — the plan
explicitly left the choice open. It exists because `verify_archive` reported
decode failures as `Io` while the retry policy makes `Io` non-retryable and
verification failures retryable, a flat contradiction.

---

## What the mutation checks proved

Every "prove this is load-bearing" step in the plan was run, and all of them
caught their target. Recorded because a check that cannot fail is not evidence:

| Check | Result |
|---|---|
| `verify_archive` body → `Ok(())` | 6 of 7 tests failed (the six failure modes; intact still passed) |
| `fs::rename` hoisted above verification | Both "never reaches the resolved name" tests failed on their `exists()` assertion |
| `resolve` fetches unconditionally | `a_present_archive_is_opened_without_downloading` failed |
| `last = e.to_string()` (escape-hatch de-duplication) | Exhaustion test failed on the duplicated clause |

---

## Known issues carried forward

- **Integrity is verified; authenticity is not.** The transport is plain HTTP
  with no digest — EDRDG's certificate fails subject-name validation, so HTTPS is
  not available. An on-path attacker can substitute a well-formed archive.
  Documented, not papered over. The natural fix when revisited is a pinned
  SHA-256 checked in `verify_archive` before the rename; it subsumes the
  corrupt-download case at no extra read, since the decode already streams every
  byte.
- **Staging orphans accumulate.** An interrupted download leaves
  `JMdict_e.gz.partial.<pid>` under a name no later run reuses, up to
  `MAX_ARCHIVE_BYTES` each. Nothing sweeps them deliberately — see invariants.
  Inert: never resolved, never verified. The cost is disk, not correctness.
- **Worst-case block is ~366 s** inside `ensure_dictionary`'s closure
  (3 × `DOWNLOAD_TIMEOUT` + backoff), with no progress signal. Strictly better
  than the unbounded hang before the timeout was added, but a phase that calls
  this at app start should show progress or move it off the startup path.
- **`DOWNLOAD_TIMEOUT` is end-to-end, not idle.** 120 s caps total download
  duration, implying a sustained-throughput floor of ~0.70 Mbit/s. A genuinely
  slow link fails every attempt. Raise it before blaming the network.
- **`clap` is pinned `=4.5.51`.** A range does not work: `clap_builder` adopted
  `clap_lex 1.x` partway through the 4.5 line. Constraining `clap_lex` directly
  does not work either — cargo permits the 0.7 and 1.x majors side by side, so
  `clap_builder` keeps its own copy. Raising the workspace MSRV to 1.85 is what
  frees `clap` to float again.
- **CI has never run.** `.github/workflows/ci.yml` exists (`6174dde`) and every
  command in it was dry-run locally, but the repo has no `origin` remote, so the
  workflow has not executed once. Confirm the first run is green.
- **`jparser-cli`'s coverage is 66.91%**, and ~35 of its missed lines are
  pre-existing Phase 2A gaps in `Lookup`/`Romaji`/`GenList`/`GenRemove`/
  `BuildIndex`, not 2B's. The crate total clears the floor comfortably.
- **The SDD ledger is git-ignored.** `.superpowers/sdd/2026-08-13-jparser-phase2b/`
  holds all nine rulings with their evidence plus eight agent reports. This
  handoff and the amended spec are the versioned record; the ledger is not.

---

## Invariants the next phase must not break

- **A file at the resolved name must never be anything but a complete, valid
  archive.** `resolve` cannot re-verify it — by design, since it must not be able
  to tell a downloaded file from a hand-placed one — so a bad publish is sticky
  and fails identically forever. Stage → verify → rename, in that order. The
  rename is the publish.
- **The staging name must stay process-unique**
  (`<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>`). With a fixed name, `File::create`
  truncates, so two processes share an inode: one can verify while the other
  truncates and rewrites, and the first's rename then publishes half-written
  bytes. `rename` also does not invalidate the other process's descriptor, so it
  keeps writing into the *published* archive. Do not simplify this back, and **do
  not add a `*.partial.*` sweep** — a process cannot distinguish another's live
  staging file from a dead one's leftovers, so deleting by pattern reintroduces
  exactly this race.
- **Staging stays inside `source_dir`.** `fs::rename` returns `EXDEV` across
  filesystems and never falls back to copying.
- **A `.partial` file is never resolved**, never verified in place, and never
  satisfies step 1 of resolution.
- **Exactly one filename is resolved, and its extension is not trusted.**
  `open_local` sniffs the gzip magic via `fill_buf`, which peeks without
  consuming. Two accepted names would need a precedence rule between a compressed
  and an uncompressed copy that disagree; there is no good answer.
- **`crates/jparser`'s library gains no HTTP client and no decompressor.**
  `jmdict-source` is `optional` behind the default-on `cli` feature, with
  `required-features = ["cli"]` on both the `[[bin]]` and the `[[test]]`. The
  gate is `cargo check -p jparser --no-default-features --all-targets` plus a
  `cargo tree` grep that must return 0. CI enforces both.
- **No dependency may link `native-tls`/OpenSSL.** GPL v2: OpenSSL is Apache-2.0,
  which the FSF holds GPLv2-incompatible. This binds every later phase.
- **MSRV is 1.75 and is now compile-verified.** Under `resolver = "2"` a
  dependency's `rust-version` is documentation, not a constraint — every direct
  dependency whose upstream may raise its floor needs an explicit bound, and only
  `cargo +1.75 check --workspace` proves the floor holds. `#[expect(...)]` is
  unavailable (stabilized 1.81); use `#[allow(...)]`.
- **Never `cargo fmt`.** `crates/jparser/src/conjugation.rs` is deliberately not
  rustfmt-clean and "fixing" it is a defect. Use `rustfmt --edition 2021 <file>`
  on individual files, then check `git diff --stat`. CI has no formatting job for
  this reason, with a comment at the top of the workflow saying so.
- **`crates/jparser/src/segment.rs` is at 778/800 lines.** Do not edit casually.
- **2A's invariants still hold unchanged:** `INDEX_FORMAT_VERSION` is 3;
  `EntryData`'s field order is wire format; a published `gen-N` is immutable;
  directory knowledge lives only in `generations.rs` and `ensure_dictionary`.
  This phase touched none of them — it produces a reader and knows nothing about
  generations.
- **The source directory is a sibling of the generation root, never a child.**
  Nesting would make `jmdict-source` depend on `jparser`'s layout convention —
  coupling the compiler cannot see. Safety was never the argument; coupling was.
