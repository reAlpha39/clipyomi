# JParser Phase 1A — Handoff

Phase 1A is complete: parser foundations plus a memory-mapped FST dictionary
index, verifiable through `jparser-cli`. 113 tests, 93.85% line coverage,
`cargo clippy -- -D warnings` clean.

This document records what Phase 1B needs and what Phase 1A deliberately left
undone. It exists because these decisions are not recoverable from the diff.

**Phase 1B is now complete;** its handoff continues this one at
`docs/superpowers/2026-08-13-jparser-phase1b-handoff.md`. Read that for the
public `parse()` surface and the invariants Phase 1B added. **This document is
not superseded:** the index-build section below is still live and still
unimplemented, and its stated trigger — the first process holding an `Index`
across a rebuild, i.e. `ensure_dictionary` — lands in Phase 2. Phase 1B's only
deliverable there was the corrected guidance now in `load.rs`.

Note also that the Rust workspace moved from `ta/` to the repo root after
Phase 1B merged. Every path in this document was already root-relative, so all
of them still resolve; the Phase 1A *plan* still says `ta/crates/jparser/…`.

**Plan:** `docs/superpowers/plans/2026-08-12-jparser-phase1a-index.md`.
Note the plan is a historical artifact and is now contradicted by the code in
two places — it specifies `IS_NAME = 0x0040` and `INDEX_FORMAT_VERSION = 1`,
both of which changed for reasons recorded below. Trust the code.

---

## Decisions that departed from the plan

**`v5` fallback tests were re-paired (plan defect).** The plan's tests asserted
`v5_fallback_stems > 0` using `言う` tagged `v5u` — a *correctly* annotated verb,
for which the counter is structurally pinned at zero. The nine length-3 `v5*`
types have mutually exclusive single-kana remove suffixes (`v5b` ぶ, `v5g` ぐ,
`v5k` く, `v5m` む, `v5n` ぬ, `v5r` る, `v5s` す, `v5t` つ, `v5u` う), so a
surface ending in う can only ever match `v5u`. The fallback only fires on
*mis*-annotated verbs — which the plan's own module prose states correctly. The
tests now use `言う` tagged `v5s`. The implementation was not changed.

**`WordFlags::IS_NAME` moved from `0x0040` to `0x0080`.** ta-old's `0x0040` is
`JAP_WORD_TOP`, so the plan's claim that flag values "mirror `JAP_WORD_*`
exactly" was false for that value. `TOP = 0x0040` is now reserved for the
Phase 1B scorer port. The other six flags do match ta-old exactly, which is what
the differential run depends on.

**`INDEX_FORMAT_VERSION` is 2**, because `IndexHeader` gained
`conjugation_fingerprint`.

**quick-xml has no `Event::GeneralRef` in 0.36.** The plan guessed it did. DTD
entity references surface inside `Event::Text` as literal `&name;` bytes;
`jmdict::decode_text` handles them. The `SPIKE RESULT` comment in `jmdict.rs`
records the measured event sequence.

---

## Known issues carried into Phase 1B

**`prefixes_of` returns the empty-key hit on every call.** Correct — empty stems
are legitimate and `する`/`来る` produce nothing else — but the scorer must
special-case `key_chars == 0`, and each call bincode-deserializes the entire
empty-key record vector. On a real corpus that vector holds one record per verb
whose whole surface is its own remove-suffix. If it is large, load it once
rather than per call; Phase 1B's parse loop calls this per character position.

**The index build is neither atomic nor crash-safe.** See the dedicated section
below — this is the largest open item, and the fix is not the one an earlier
draft of this document sketched.

**`PRIORITY_MARKERS` includes `spec2` on documentation, not evidence.** The
EDICT `(P)` rule is documented as firing on `ichi1`, `news1`, `spec1`, `spec2`,
`gai1`. This could not be verified from anything in the repo — there is no
`JMdict_e.xml`, and ta-old read EDICT2 where `(P)` was already baked in.
**Confirm against the JMdict DTD's `ke_pri`/`re_pri` documentation before
interpreting the differential run**, or a few thousand entries' `COMMON` flag
will differ for a reason that has nothing to do with the port.

**The `TOP` doc comment overstates the flag's status.** It cites
`Dictionary.cpp:1010` and `:1239` without noting that those read sites — and the
write sites at `:306`/`:388` — are all inside `#ifdef SETSUMI_CHANGES`, which
nothing in ta-old ever defines. The `(T)` tagging feature is dead in the shipped
build. The bit reservation is still right; the comment will mislead whoever
ports `FindBestMatches`.

**`StemStats` can misattribute exact vs fallback stems.** For a headword
carrying both a correct and a same-length mis-annotated `v5*` tag, whichever POS
tag comes first in source order wins, and dedup drops the second before its
counter fires. The authoritative measurement of whether the fallback earns its
keep is the `--no-v5-fallback` A/B, not these counters — on the test fixture,
11 records with it versus 8 without.

**The CLI is the only place all seven modules meet, and it has 0% coverage.**
A single process round-trip test buys more than another unit test anywhere else.

**~13 public items have no non-test caller yet**: `to_katakana`, the `kana`
classifiers other than `unify`/`unify_str`, `tense_name`, `MAX_CONJ_DEPTH`,
`Form::is_formal`/`is_negative`, `WordFlags::remove`, `IS_NAME`,
`JmdictError::BadEntry`. They are tested, not exercised. Expect the first real
consumer to want a different signature from at least one — `to_katakana`'s
`Option` return is the likely candidate.

---

## The index build: hazards, verified facts, and the recommended fix

This section supersedes an earlier draft of this document, which claimed the fix
was "~10 lines: build into a sibling directory and `fs::rename` the *directory*."
**That is wrong** — see verified fact 2. The sketch below is what the evidence
actually supports.

### Two distinct hazards

**1. Rebuild-into-a-live-directory is undefined behaviour.** `build_from_reader`
writes all five files directly into the target directory with `File::create`,
which truncates in place, while `load.rs` mmaps three of them. Soundness rests on
an unenforced caller obligation documented on `Index::open`. This goes live the
moment one process holds an `Index` across a rebuild.

**2. An interrupted rebuild can serve silently wrong data.** Independent of any
concurrency. `build.rs` writes `keys.fst`, `records.bin`, `entries.bin`,
`entries.idx`, then `header.bin` **last**; `Index::open` validates only
`version` and `conjugation_fingerprint`. After a torn rebuild both match — the
header is the old one, and a new build from the same binary has the same version
and fingerprint anyway — so `open` always succeeds. Reproduced across 11
reconstructed crash points: some yield a clean `IndexError` (bincode EOF, or
`slice_at`'s bounds checks), and some return well-formed wrong answers with no
error at all — `entry(2000010)` returning `EntryData { id: 1000010, .. }`, and
`prefixes_of` returning a `StoredRecord` belonging to a different key. No panics
in any scenario. Because the index is built on the user's machine from a
downloaded JMdict, a power cut mid-update is a field scenario, not a lab one.

### Verified empirically (Darwin 25.3.0 / APFS / arm64, Rust 1.97.1)

1. **An established mmap survives everything except in-place truncation.**
   `unlink`, `rename` over the mapped path, renaming the parent directory, and
   `remove_dir_all` on the parent all leave a live mapping intact and readable
   with its original bytes — no `SIGBUS` — because the mapping holds its own
   reference to the inode. The one thing that *does* break it is `File::create`
   (`O_TRUNC`) on the same inode: reads past the new EOF raise a real `SIGBUS`
   (signal 10, confirmed). That is precisely what the current builder does.
2. **You cannot atomically replace a non-empty directory.**
   `fs::rename` onto a non-empty directory fails with `DirectoryNotEmpty`
   (`ENOTEMPTY`, OS error 66); onto an empty directory or an absent path it
   succeeds; onto a file it fails with `NotADirectory` (`ENOTDIR`, 20);
   across filesystems it fails with `CrossesDevices` (`EXDEV`, 18) and never
   falls back to copying. Directory rename is O(1) metadata — 103–120µs for a
   directory holding 100MB across 10 files.
3. **Symlink-over-symlink rename is atomic and does work** — but see below for
   why that is not enough here.

### Why generation directories, and why not any swap

`Index::open` is a five-step sequence against a directory *name*: read
`header.bin`, read `entries.idx`, then mmap three files. Any scheme where readers
resolve a **mutable** name — rename-over-the-directory, symlink flip, or a
`CURRENT` pointer file — can splice generation N's `entries.idx` onto generation
N+1's `entries.bin` if a reader opens mid-swap. That produces exactly hazard 2's
silent-wrong-answer class. Atomicity of the pointer swap does not help, because
the *open* is not atomic.

Generation directories are immune by construction: a `gen-N` path's contents
never change after creation, so a straddling open either succeeds wholly or gets
`ENOENT`.

### Recommended shape

```
<root>/.build-<pid>-<nanos>/   build here (fs::create_dir, NOT create_dir_all)
<root>/gen-<N>/                fs::rename into place when complete
```

Readers scan `read_dir(root)`, take the highest `gen-N`, and `Index::open` that
immutable path. `Ok(None)` from the scan is the first-run "no dictionary yet"
signal. A crash leaves a `.build-*` orphan no reader will ever resolve. A
concurrent builder that loses the race gets `ENOTEMPTY` and its temp directory
survives, so it can retry — loud, not silent, and no lost update. Sweep old
generations and orphans at app startup only, before any mapping exists.

This deliberately does **not** solve: cross-process writers (a second instance
can still add generations — it just cannot corrupt one), power-loss durability
(no `fsync`; see judgment calls), or payload bit rot.

`build_from_reader` itself needs **no change** — it just always receives a fresh
nonce directory.

### Timing

**Write no code for this in Phase 1B.** The only Phase 1B deliverable is ~12
lines of corrected comment in `load.rs`: rewrite the `map()` SAFETY block and
`Index::open`'s doc to record that the fix is a fresh directory per build, never
a swap onto a live path, **and why** — because `open` reads five files in
sequence. Without the "why", the next implementer will build the rename-over
swap this document previously sketched, which is the broken one.

**Trigger for implementing it:** the first commit where a single process keeps an
`Index` alive across a rebuild — concretely, when `ensure_dictionary` is written
in the Tauri layer. That is Phase 2, not Phase 1B. Phase 1B's parser takes an
`&Index` and never learns a directory path, so it cannot make this harder later.

Estimated cost when it lands: a new `index/generations.rs` (~60 LOC plus ~60 LOC
of tests) exposing `latest(root)`, `build_new(root, ..)`, and `sweep(root, keep)`;
one line in `index/mod.rs`; comments only in `load.rs`; and ~15 LOC on the Tauri
side for an `RwLock<Arc<Index>>` plus a mutex held across the whole rebuild so
two update clicks cannot overlap.

### Open judgment calls

- **`fsync` before the rename** (+~12 LOC in `build.rs`). Without it a power cut
  can make the `gen-N` directory entry durable before its contents are, producing
  hazard 2's silent-wrong-data class inside a directory readers *will* trust.
  Adding it means a `write_and_sync` helper, building the FST to a `Vec<u8>`
  rather than a `File`, and `#[cfg(unix)]` for the directory-entry sync (no std
  equivalent on Windows). Suggested default: skip it, add it if a user ever
  reports garbage lookups after a hard reboot. No power-loss experiment was run
  either way.
- **True hot-swap versus a brief "reloading dictionary…" pause.** If a
  teardown-and-reopen of the `Index` after an update is acceptable, the Phase 2
  in-memory piece collapses to about five lines and the `RwLock`/`Arc` design
  disappears — the disk layout alone satisfies the design spec's requirement that
  the previous index stay live through a failed rebuild. Product decision.
- **Whether the CLI adopts the layout.** It does not need to;
  `build-index <out>` / `lookup <index>` keep working against a bare generation
  directory. Leaving the CLI alone is also why `generations.rs` should not be
  written until Phase 2 — it would have no caller.

### Platform note

Windows is a first-class target (Windows + macOS via Tauri; Linux deferred), so
POSIX-only behaviour cannot be relied on. The layout above was chosen so Windows
only ever needs four uncontroversial operations: `create_dir` on a fresh name,
`rename` to an absent target, `read_dir`, and `remove_dir_all` at startup when
nothing is mapped. It never renames or deletes a populated or mapped directory,
and it never depends on fact 1's unlink-survival behaviour. **None of this was
exercised on Windows** — budget an afternoon on a Windows box running those four
operations before Phase 2 ships.

---

## Invariants Phase 1B must not break

- **`kana::unify` is character-wise and therefore prefix-stable.** The entire
  FST design depends on folding a prefix equalling the prefix of a folded
  string. `kana.rs` tests this over every char boundary.
- **`kana::strip_suffix_unified` is shared** by conjugation-chain resolution and
  stem generation. They must agree or stems stop lining up with chains. If you
  teach kana folding about long-vowel marks, small kana, or halfwidth katakana
  (which `unify` deliberately does *not* fold, faithfully to ta-old), it changes
  both.
- **A stem carries exactly one verb type.** `build.rs` maps it to storage via
  `verb_types.first()`, lossless only because of this.
- **Duplicate type names are deliberate.** `vk`, `vs`, `v5r-i`, `v5uru` each
  appear twice — one twin carries kanji-form suffixes, the other kana-form —
  which is how readings are reconstructed for irregular verbs. `types_named`
  returns all matches; the stem candidate loop scans by name, not id.
- **Fixed tense discriminants** `Remove = 0`, `NonPast = 1`, `Stem = 2`,
  `Potential = 3` are special-cased and must not be reordered.
- **`verb_type` ids are indices into the conjugation table.** An index is bound
  to its asset by `conjugation_fingerprint`; a changed `conjugations.json` makes
  `Index::open` refuse rather than silently resolve the wrong verb.
- **ta-old stored `verbType` as `vt + 1`** with `0` meaning "not a verb". This
  port uses a 0-based id in an `Option`. The differential run compares against
  the old encoding.
- **No Phase 1B type stores the index directory path.** Pass `&Index` or
  `Arc<Index>`. The parser has no need for a path, and keeping it that way is
  what lets the generation layout land in Phase 2 without touching Phase 1B code.

---

## Verification

```
cargo test -p jparser                              # 113 tests
cargo clippy -p jparser --all-targets -- -D warnings
cargo llvm-cov -p jparser --summary-only           # 93.85%

cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/idx
cargo run -q -p jparser --bin jparser-cli -- lookup /tmp/idx "言うから"
```

The fixture yields keys=11, records=11, entries=3, skipped=0, exact_stems=2,
v5_fallback_stems=3, empty_stems=0. With `--no-v5-fallback`: 8 keys, 8 records,
0 fallback stems.
