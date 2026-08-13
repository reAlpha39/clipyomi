# JParser Phase 2A — Handoff

Phase 2A published dictionary indexes as immutable numbered generations so that
an interrupted or concurrent rebuild can never serve well-formed wrong data, and
gave the crate a headless `ensure_dictionary` plus a CLI to drive it.

Executed from `docs/superpowers/plans/2026-08-13-jparser-phase2a.md` against
`docs/superpowers/specs/2026-08-13-jparser-phase2a-design.md`.

**Branch:** `phase2a-dictionary-generations`, 23 commits off `master` @ `749d6cf`.

**Verification at close:** 255 tests passing, `cargo clippy -p jparser
--all-targets -- -D warnings` clean, `cargo doc -p jparser --no-deps`
warning-free, `cargo llvm-cov -p jparser --summary-only --fail-under-lines 80`
→ 96.50% lines (Phase 1B baseline 96.37%, floor 80).

---

## The public surface Phase 2B consumes

In `jparser::index::generations`:

```rust
pub const GENERATION_PREFIX: &str = "gen-";
pub const BUILD_PREFIX: &str = ".build-";
pub const DEFAULT_KEEP_GENERATIONS: usize = 2;

pub fn generation_number(name: &str) -> Option<u64>;
pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError>;
pub fn build_new(
    root: &Path,
    xml: impl BufRead,
    table: &ConjugationTable,
    opts: &StemOptions,
) -> Result<(PathBuf, BuildReport), IndexError>;
pub fn sweep(root: &Path, keep: usize) -> Result<usize, IndexError>;
```

In `jparser::index`:

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

IndexError::GenerationExists { generation: u64, build_dir: PathBuf }
```

In `jparser::index::load`: `Index::entry_count(&self) -> usize`.

`source` is `FnOnce` deliberately — the compiler then guarantees it is invoked at
most once, which is what keeps the ~60 MB JMdict download off the steady-state
path. `ensure_dictionary` never calls it when the newest generation opens.

CLI: `ensure-dictionary <root> <xml> [--keep N]`, `gen-list <root>`,
`gen-sweep <root> [--keep N]`, `gen-remove <root> <generation>`.

---

## Why the layout is what it is

`Index::open` is a five-file sequence against a directory *name*. It validates
only the header's `version` and `conjugation_fingerprint` — no counts, no
payload. So any scheme where readers resolve a **mutable** name
(rename-over-the-directory, a symlink flip, a `CURRENT` pointer file) can splice
one generation's `entries.idx` onto another's `entries.bin` when a reader opens
mid-swap, yielding well-formed wrong answers rather than an error. Atomicity of
the pointer swap does not help, because the open is not atomic.

A `gen-N` directory's contents never change after creation, so a straddling open
either succeeds wholly or gets `ENOENT`.

Two consequences worth knowing before changing anything here:

- **`generation_number` is deliberately strict.** `gen-01`, `gen-`, `gen-abc`,
  `gen-1x` all fail to parse. A permissive parse would let a hand-created
  `gen-01` and a real `gen-1` both claim generation 1 — the exact ambiguity
  immutable names remove.
- **`DirEntry::file_type()` not following symlinks is intent, not omission.** A
  symlinked `gen-N` is a mutable name, so resolving it would reintroduce the
  hazard through the back door.

Monotonic numbering also buys a Windows property for free: a partially-deleted
`gen-K` can never become `latest` again, because numbers only increase.

---

## Decisions that departed from the plan and the spec

Seven, each because a review found the mandated version defective. All were
ruled on explicitly rather than silently changed.

1. **`build_new` retries the publish** (`PUBLISH_ATTEMPTS = 8`), recomputing
   `latest + 1` per attempt, so concurrent builders all succeed as `gen-1`,
   `gen-2`, … **This supersedes design spec §5's "the loser errors."** As
   specified, two processes starting from an empty root both targeted `gen-1` and
   the loser returned `GenerationExists` even though a valid `gen-1` then
   existed — a double-launched application failed to start for no reason. Spec §5
   should be amended.
2. **Spec §9's "a retry succeeds" clause was never testable as written,** because
   the plan made `publish` private and `build_new` always allocate a fresh nonce,
   so no public API could perform the retry the error message promised. The retry
   loop in (1) is what makes the clause meaningful. Spec §9 should be amended
   alongside §5.
3. **A failed `sweep` is non-fatal in `ensure_dictionary`,** and `sweep` treats
   `remove_dir_all`'s `NotFound` as already-done. Spec §8 already said sweep
   failure is non-fatal and `sweep`'s own doc repeated it, but the code used `?`
   on both — so two concurrent sweeps, or a Windows reader holding a generation
   past `keep`, became a hard startup failure. This was an implementation/spec
   mismatch, not a spec defect.
4. **`keep == 0` is clamped to 1 in `ensure_dictionary`** and rejected outright at
   the CLI. `sweep(root, 0)` would otherwise delete the generation
   `ensure_dictionary` just published. `sweep` itself still accepts `0` —
   deleting everything is coherent for a primitive.
5. **`gen-list` propagates `read_dir` errors** instead of the plan's
   `.into_iter().flatten().flatten()`, which dropped the `Err` so a mistyped root
   printed nothing and exited 0. The asymmetry with `latest`/`sweep` — which
   treat an absent root as the legitimate "no dictionary yet" signal — is
   deliberate: those are library calls on a first-run path, `gen-list` is a human
   diagnostic.
6. **`gen-list` sorts numerically,** which is why `generation_number` is `pub`.
   The plan's lexicographic sort contradicted the command's own `--help` text
   from the 10th rebuild on (`gen-9` before `gen-10`).
7. **`gen-remove <N>` was added** beyond the plan. Without it an unopenable
   newest generation could not be repaired: `ensure_dictionary` deliberately
   errors rather than rebuilding (spec §6, correct), `sweep` never deletes the
   newest, and `--keep 0` is rejected. `gen-list` could diagnose the state but
   nothing could act on it.

The build nonce also gained a third component: `.build-<pid>-<nanos>-<seq>`,
where `seq` is a process-wide `AtomicU64`. The pid separates processes and the
clock separates sequential builds, but two threads share a pid and can read the
same coarse clock value.

---

## Five plan-mandated tests that could not fail

Recorded because the pattern recurred and is worth watching for in 2B: **the
plan repeatedly damaged one thing and asserted on a read path the damage did not
touch.**

| Test | Damage applied | Assertion read | Why it could not fail |
|---|---|---|---|
| hazard reproduction | B's `keys.fst` + `records.bin` into A's dir | `entry()` | `entry()` reads only `entries.idx`/`.bin`, both still A's |
| `two_builds_in_one_process…` | none — first temp dir already renamed away | `latest() == gen-2` | a constant temp name would pass identically |
| eleven-state matrix | one of five files in the orphan | `latest()` | `latest` filters `.build-*` by *name*, never opens it |
| `ensure_dictionary` `keep` | — | `keep = 2` everywhere | equals `DEFAULT_KEEP_GENERATIONS`, so the parameter was decorative |
| `concurrent_sweeps…` | racing threads | `sweep` returns `Ok` | won the race ~95% of runs, so the `NotFound` arm was effectively uncovered |

All five were strengthened. The hazard test now asserts through `prefixes_of`,
which is the actually-spliced read path; the eleven-state matrix gained a
`state == 10` assertion that an *intact* unpublished build is still never served;
`concurrent_sweeps` was replaced by a deterministic unit test of an extracted
`remove_if_present`.

---

## Known issues carried into Phase 2B

- **No `fsync`.** A power cut between the `rename` and the contents reaching disk
  can still publish a generation readers trust. Deliberate; see spec §7. The
  cheap mitigation is the next item.
- **`Index::open` cross-checks no counts.** `header.bin` already carries `keys`,
  `records`, and `entries`. Asserting `entry_offsets.len() == header.entries` and
  `fst.len() == header.keys` would close most of the residual no-`fsync` window
  for a handful of lines. **This is Phase 1 surface, so it was out of scope for
  2A.** Note that it would correctly retire
  `a_rebuild_into_a_live_directory_can_serve_data_from_neither_build`, which says
  so at its own site — that test deliberately asserts the bad outcome as a
  baseline. Delete it and say so in the commit; do not "fix" it.
- **`sweep`'s precondition is documented, not enforced.** Call it only before any
  `Index` has been opened from `root`. Darwin lets an established mmap survive
  `remove_dir_all` on its parent; Windows does not generally permit deleting a
  mapped file. `DEFAULT_KEEP_GENERATIONS = 2` exists so a sweep that fails
  anyway is never load-bearing.
- **Spec §8 wants a sweep failure "logged and counted."** It is now neither —
  the crate has no logger, and `ensure_dictionary`'s signature gives the caller
  no way to learn a sweep failed. If 2B surfaces retention state in settings,
  this needs a return-shape change.
- **`ensure_dictionary` does not tell the caller which generation it opened.**
  The CLI re-resolves `latest(root)` to print the name, so the printed generation
  and the opened index can disagree under a concurrent publish. Returning
  `(PathBuf, Index)` would avoid re-asking a mutable question — which is this
  phase's whole theme.
- **`build_new`'s retry loop has only probabilistic test coverage.** Its
  collision window cannot be forced through the public API. The concurrent
  builders test detects loop removal only when the two threads actually contend;
  the lost-race test covers `publish` and `next_generation`, not the loop. Both
  say so in their doc comments.
- **The eleven-state suite is ten repetitions plus one signal.** States 0–9 are
  covered by construction — exclusion happens on the name before a byte is read
  — so the state-sensitive assertion is entirely in state 10. Collapsing 0–9 to
  two representatives would lose no signal, at a cost of ~33 mini-builds today.
  Recorded so a future collapse is not mistaken for a regression.
- **`next_generation`'s `n + 1` would overflow** on a hand-made
  `gen-18446744073709551615`. Pre-existing shape, carried forward.
- **Windows is entirely unexercised.** `create_dir` on a fresh name, `rename` to
  an absent target, `read_dir`, `remove_dir_all` with nothing mapped. A read
  through found nothing likely to differ beyond the two already named —
  `remove_dir_all` on a mapped file, and `rename` failing under an antivirus
  handle — and `build_from_reader` closes every handle before returning, so the
  `rename` has no open handle of its own to trip on.
- **The sweep-failure test probes its own preconditions.** It sets a parent to
  `0o555`, which root ignores, so it checks whether the restriction actually
  bites and skips the assertion when it does not. Detecting euid would need
  `libc`, which the crate forbids.

---

## Invariants Phase 2B must not break

- **`INDEX_FORMAT_VERSION` is 3** and did not change in 2A. `EntryData`'s field
  order is `id, readings, senses` and bincode is positional, so **field order is
  wire format**.
- **A published `gen-N` directory is immutable.** Never write into one. Build
  into `.build-*` and publish by `fs::rename` — that single rename is the only
  thing that makes a generation visible, and it is what the no-splice guarantee
  rests on.
- **`root` and the build directory must be on one filesystem.** `fs::rename`
  returns `EXDEV` across devices and never falls back to copying.
- **`publish` must keep leaving the build directory in place on failure** — the
  retry loop reuses it.
- **Directory knowledge stays in `generations.rs` and `ensure_dictionary`.**
  `parse` still takes `&Index` and no parser type knows a path.
- **`crates/jparser/src/segment.rs` is at 778/800 lines.** Do not edit it
  casually. `conjugation.rs` is deliberately not rustfmt-clean — "fixing" it is a
  defect, which is why this phase used `rustfmt --edition 2021 <file>` on
  individual files and never `cargo fmt`.
- **No new dependency** was added, and the crate still depends on no Tauri, UI,
  or HTTP crate. `keep == 0` reaching `ensure_dictionary` is clamped; do not
  remove that guard on the assumption the CLI is the only caller.
