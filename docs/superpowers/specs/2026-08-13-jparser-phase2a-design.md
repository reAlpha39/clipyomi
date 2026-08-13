# JParser Phase 2A — Dictionary Lifecycle Design

**Date:** 2026-08-13
**Status:** Approved, ready for implementation planning
**Amends:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md`
**Follows:** `docs/superpowers/2026-08-13-jparser-phase1b-handoff.md`

This is an addendum, not a replacement. The port design already specifies the
app shell's module split (§6), the error-handling policy at each boundary (§9),
and the testing posture (§10). The Phase 1A handoff already specifies the
generation-directory layout, the empirical filesystem facts behind it, and why
no swap-based scheme works. **Those stand as written and are not restated here**
beyond what is needed to fix a signature.

This document records only what those left open, plus the decisions taken during
brainstorming and two supersessions.

---

## 1. Scope

Phase 2 as the port design writes it — Tauri shell, first-run download,
clipboard monitor, sentence and definition panes, theming, always-on-top — is
larger than Phase 1B, which took nine tasks. It is therefore split into three
sub-phases, each with its own spec, plan, and execution cycle:

| | Subsystem | Character |
|---|---|---|
| **2A** | Dictionary lifecycle: generation publish, retention, `ensure_dictionary` | Headless Rust. High correctness risk. **This document.** |
| **2B** | App shell: clipboard poll, latest-wins parse worker, commands, settings, window | Tauri. Medium risk. |
| **2C** | UI: sentence and definition panes, theming, header controls, EDRDG attribution | Frontend. Low risk. |

2A goes first because it is fully designed already, testable without any network
or GUI, and is where a defect is invisible rather than obvious — a torn rebuild
that serves well-formed wrong answers.

**Explicitly out of 2A:**

| Item | Owner |
|---|---|
| HTTP, the EDRDG URL, resume-on-retry, gzip decode and integrity check | 2B |
| `ensure_dictionary` as a Tauri command, and the `RwLock<Arc<Index>>` that holds the result | 2B |
| Any UI, including download progress and the EDRDG attribution the licence requires | 2C |
| `fsync` before publish | Deferred — see §7 |

---

## 2. Supersessions

**Port design §6 places `ensure_dictionary` in `src-tauri/commands.rs`. This
document moves it to `crates/jparser`, in `index/mod.rs`.**

The reason is a conflict inside the port design itself. §9 gives
`ensure_dictionary` decision logic with a correctness consequence — *"Index
version mismatch ⇒ rebuild from source; never attempt a read"* — while §10 states
coverage is *"not meaningful in `src-tauri`, which is glue and window
plumbing."* As §6 writes it, the one rule that decides whether the app reads a
stale index lands in the one layer the design declines to test. Moving the
orchestrator into `jparser` puts it under the 80% line. `src-tauri` keeps a
`parse_text`-style command that calls it, which is genuine glue.

This is the same pattern as Phase 1B superseding contract §7's module map, and
is flagged here rather than discovered during implementation.

**The Phase 1A handoff sketches `build_new(root, ..)` without fixing its input
type.** §4 below fixes it, and in doing so removes the `DictSource` trait an
earlier draft of this design proposed.

---

## 3. Disk layout

Unchanged from the Phase 1A handoff; restated only because every signature below
refers to it.

```
<root>/
├── .build-<pid>-<nanos>/     transient; no reader ever resolves this name
├── gen-1/                    immutable once created
├── gen-2/
└── gen-7/                    readers take the highest N
```

Each `gen-<N>` holds the five files `build_from_reader` writes:
`keys.fst`, `records.bin`, `entries.bin`, `entries.idx`, `header.bin`.

**Why immutable directories and not a swap.** `Index::open` is a five-step
sequence against a directory *name*: read `header.bin`, read `entries.idx`, then
mmap three files. Any scheme where readers resolve a **mutable** name —
rename-over-the-directory, symlink flip, or a `CURRENT` pointer file — can
splice generation N's `entries.idx` onto generation N+1's `entries.bin` if a
reader opens mid-swap, which is exactly the silent-wrong-answer class the layout
exists to prevent. Atomicity of the pointer swap does not help, because the
*open* is not atomic. A `gen-N` path's contents never change after creation, so
a straddling open either succeeds wholly or gets `ENOENT`.

**Generation numbers** are `u64`, formatted without padding. `latest` parses the
suffix and ignores any directory whose suffix does not parse — `gen-`, `gen-abc`,
`gen-1x`, and `gen-01` are all ignored rather than treated as `1`, so a
hand-created directory can never shadow a real generation.

---

## 4. The lazy source seam

`ensure_dictionary` must not obtain the dictionary bytes unless a rebuild is
actually needed — the real source is a ~60 MB download. Laziness is the only
property required, so it is expressed as a closure rather than a trait:

```rust
pub fn ensure_dictionary<R, F>(
    root: &Path,
    keep: usize,
    source: F,
) -> Result<Index, IndexError>
where
    F: FnOnce() -> std::io::Result<R>,
    R: std::io::BufRead;
```

2A's callers pass `|| File::open(path).map(BufReader::new)`. 2B passes
`|| Ok(BufReader::new(GzDecoder::new(response)))`. The closure is not invoked on
the steady-state path.

**A `DictSource` trait was considered and rejected.** It would have exactly one
implementation in 2A and one in 2B, adds a file and a vtable, and buys nothing
the closure does not already provide. If a future phase needs to enumerate or
configure sources, that is when the trait earns its place.

**gzip stays in 2B.** `jparser` gains no decompression dependency. 2A reads
plain XML; the `.gz` integrity check the port design's §9 requires is a property
of the download, and belongs with the code that performs it.

---

## 5. Module surface

### `crates/jparser/src/index/generations.rs` (new)

```rust
/// Highest-numbered generation, or None on an empty or absent root.
pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError>;

/// Build into a fresh nonce directory, then publish by rename.
/// Returns the published `<root>/gen-<N>` path.
pub fn build_new(root: &Path, xml: impl BufRead) -> Result<PathBuf, IndexError>;

/// Remove `.build-*` orphans and all but the `keep` highest generations.
/// Returns the number of directories removed.
pub fn sweep(root: &Path, keep: usize) -> Result<usize, IndexError>;
```

`build_new` uses `fs::create_dir`, **not** `create_dir_all`: the nonce directory
must not already exist, and a collision is a signal worth surfacing rather than
absorbing. It calls the existing `build_from_reader` unchanged — that function
needs no modification, because it always receives a fresh empty directory. It
then computes `N = latest + 1` and `fs::rename`s into place.

A builder that loses a race gets `ENOTEMPTY` from the rename. Its temp directory
**survives** so it can retry; the failure is loud and no update is lost.

### `crates/jparser/src/index/mod.rs` (modified)

`ensure_dictionary`, per §4's signature. Its decision table is §6.

### `crates/jparser/src/bin/jparser-cli.rs` (modified)

```
jparser-cli ensure-dictionary <root> <xml> [--keep N]
jparser-cli gen list <root>
jparser-cli gen sweep <root> [--keep N]
```

2A is headless by construction, so without these there is no way to exercise it
by hand — every check would be a `cargo test` away from a human. Phase 1's CLI
harness is what made that phase inspectable, and the same argument applies here.
`gen list` prints each generation, its format version, and whether it opens.

---

## 6. `ensure_dictionary` decision table

`IndexError` already carries exactly the variants this needs, so no new one is
required for the dispatch:

| `latest(root)` | `Index::open` result | Action |
|---|---|---|
| `None` | — | build, sweep, open |
| `Some(p)` | `Ok` | return it; the closure is never called |
| `Some(p)` | `VersionMismatch` or `ConjugationMismatch` | build, sweep, open |
| `Some(p)` | `Io`, `Fst`, `Encoding`, `Jmdict` | **return the error** |

The match is on those two named variants specifically, not on a catch-all —
adding a future variant must force a compile error at this site rather than
silently falling into either arm.

The last row is the one worth stating explicitly. A version or fingerprint
mismatch is *expected* after an app upgrade or a changed `conjugations.json`,
and rebuilding silently is right. Any other failure — a truncated payload, a
bincode error, an I/O fault — means a generation that was published as complete
is not, which is a bug or a failing disk. Rebuilding on it would re-download
60 MB and hide the fault. This applies the crate's existing "never silently skip
data without counting it" constraint to the dictionary as a whole.

Order matters: **sweep runs after the successful rename and before the open** of
the new generation, never before the build.

---

## 7. Retention, sweep, and the Windows precondition

**`DEFAULT_KEEP_GENERATIONS = 2`**, a named constant in `generations.rs`. `keep`
stays an explicit parameter on `sweep` and `ensure_dictionary` — the constant is
the value the CLI's `--keep` defaults to and the value 2B is expected to pass,
not a hidden default inside the library.

The value is 2 rather than 1, and not for rollback — nothing reads an older
generation once a newer one exists — but as slack for a sweep that fails.

Phase 1A verified on Darwin that an established mmap survives `unlink`,
`rename`, and `remove_dir_all` on its parent, because the mapping holds its own
reference to the inode. **Windows does not generally permit deleting a mapped
file.** Sweep must therefore run at application startup, before any `Index` has
been opened, and the port design's Windows-as-first-class-target commitment
means a sweep that fails there must not be load-bearing. `keep = 2` guarantees
that: a failed sweep leaves extra directories on disk and nothing else.

This makes "sweep before any mapping exists" a **documented precondition of
`sweep`**, not an informal note. It is stated on the function.

Phase 1A's four-operation Windows budget still applies and is still unspent:
`create_dir` on a fresh name, `rename` to an absent target, `read_dir`, and
`remove_dir_all` when nothing is mapped. The layout deliberately never renames
or deletes a populated-and-mapped directory.

**No `fsync` before publish.** Decided deliberately. Without it, a power cut can
make the `gen-N` directory entry durable before its contents are, producing a
directory readers will trust. The cost of closing that is not the ~12 lines: it
is that directory-entry sync has no `std` equivalent on Windows, so the fix buys
a Unix-only guarantee at the price of a `#[cfg]` fork in the most
correctness-sensitive code in the project. Recorded as a **known residual**;
revisit if a user reports garbage lookups after a hard reboot.

---

## 8. Error handling

**Exactly one new `IndexError` variant**, because a lost publish race arrives as
a bare `ENOTEMPTY` that `IndexError::Io` would flatten into "an I/O error" —
and "another builder won, retry" and "the disk is full" are different operator
actions:

```rust
#[error("index generation {generation} already exists; another builder \
         published first — retry (partial build kept at {build_dir})")]
GenerationExists { generation: u64, build_dir: PathBuf },
```

Naming `build_dir` in the message is what makes the retry actionable and the
orphan findable. Every other boundary below reuses an existing variant.

| Boundary | Policy |
|---|---|
| Nonce directory already exists | Error. `create_dir`, not `create_dir_all`. |
| Rename onto an existing `gen-N` | `ENOTEMPTY`; temp directory survives; error names the retry. |
| Rename across filesystems | `EXDEV`. `fs::rename` never falls back to copying, so `root` and its build directory must be on one filesystem. Stated as a precondition. |
| Malformed `gen-*` directory name | Ignored by `latest`, removed by `sweep` only if it matches `.build-*`. Never guessed at. |
| Corrupt published generation | Returned, not rebuilt (§6). |
| Sweep failure | Non-fatal. Logged and counted; `keep = 2` makes it non-load-bearing. |

---

## 9. Testing

**The crash-point suite is the deliverable that distinguishes "built it" from
"know it works,"** and is the whole justification for the layout existing. Phase
1A reproduced the hazard across 11 reconstructed crash points, several of which
returned well-formed wrong answers with **no error at all** — `entry(2000010)`
returning `EntryData { id: 1000010, .. }`, and `prefixes_of` returning a
`StoredRecord` belonging to a different key.

`build_from_reader` writes five files in a fixed order — `keys.fst`,
`records.bin`, `entries.bin`, `entries.idx`, `header.bin`, header last. The
suite reconstructs the same coverage by construction: for each of the five
files, {absent, truncated}, plus the case where all five are written but the
process dies before the rename. Eleven states.

For every state, assert:

1. `latest(root)` returns the previous good generation, or `None` — **never the
   interrupted build**;
2. no `.build-*` path is ever returned by `latest`;
3. for every generation `latest` does return, `Index::open` succeeds and
   `entry(id).id == id` for all ids — no cross-generation splicing;
4. a subsequent `build_new` succeeds despite the orphan.

Point 3 is the one that would have caught the original hazard. A suite asserting
only "no panic" would have passed against the broken code.

Additionally:

- concurrent builders: the loser gets `ENOTEMPTY`, its temp directory survives,
  and a retry succeeds;
- `sweep` keeps exactly the `keep` highest and removes `.build-*` orphans;
- `latest` ignores `gen-`, `gen-abc`, `gen-01`;
- version mismatch ⇒ rebuild, and corrupt ⇒ error, asserted as **distinct**
  outcomes — collapsing them is the likely implementation slip;
- CLI round trip: `ensure-dictionary` twice in a row builds once, and `gen list`
  reports one generation.

No test touches the network. Temp directories use the existing
`std::env::temp_dir().join(format!("jparser-test-{name}"))` +
`let _ = std::fs::remove_dir_all(&dir);` pattern — there is no `tempfile` crate
and none is coming.

---

## 10. Constraints inherited

From the Phase 1B handoff and the crate's standing rules, unchanged and binding:

- **No new dependency**, and no Tauri, UI, or HTTP dependency in `crates/jparser`.
- **GPL v2 header** on every new source file.
- **No `unwrap()`/`expect()`** in library code outside tests.
- **All public types owned and immutable.**
- **No magic numbers** — the retention default and the nonce format are named
  constants.
- **800-line hard cap per file** including its `#[cfg(test)] mod tests` block.
  `generations.rs` is a new file with room; `index/mod.rs` is at 119 lines and
  `segment.rs`, at 778, is untouched by this phase.
- **Never `cargo fmt -p jparser`**, and never `rustfmt` the crate root — both
  cascade into `conjugation.rs`, `kana.rs`, and `romaji.rs`, which are
  deliberately left alone. Format individual leaf files and check
  `git diff --stat`.
- **No Phase 1B type stores the index directory path.** 2A introduces the first
  code that legitimately knows about `root`; it lives in `generations.rs` and
  `ensure_dictionary`, and `parse` still takes `&Index`.
- **`INDEX_FORMAT_VERSION` is 3.** 2A does not change it. `EntryData`'s field
  order is wire format — bincode is positional.

---

## 11. Risks the plan must address explicitly

1. **The crash-point suite must fail against the current code.** If it passes
   before `generations.rs` exists, it is asserting the wrong thing. The plan
   should require demonstrating red first, against a direct-into-`root` build.
2. **`gen-01` must not resolve as generation 1.** A permissive parse here
   reintroduces the shadowing problem the immutable-name design eliminates.
3. **Version-mismatch and corrupt must not collapse into one arm.** They differ
   only in which `IndexError` variant comes back, and treating them alike is a
   two-character mistake with a 60 MB consequence.
4. **`sweep` must not run before the build.** Sweeping first can delete the
   generation the app is about to fall back to if the build then fails.
5. **The nonce must actually be unique.** `<pid>-<nanos>` is unique across
   processes and within one; the plan should pin the format as a constant and
   test that two builds in the same process do not collide.
6. **Windows remains unexercised.** Nothing in 2A can verify it on this machine.
   The plan should state that plainly rather than implying coverage.
