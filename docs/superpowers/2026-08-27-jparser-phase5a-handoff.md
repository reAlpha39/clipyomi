# JParser Phase 5A — Handoff

**Date:** 2026-08-27
**Spec:** [`docs/superpowers/specs/2026-08-27-jparser-phase5a-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-27-jparser-phase5a-design.md)
**Commits:** `ac3773a..54db839` on `develop` (five commits, unpushed)

This is Phase 5A of Phase 5 (MeCab). 5A ships acquisition only — download,
verify, unpack, publish — as a library and a CLI driver, exactly as 2B did for
JMdict four sub-phases before 2F wired it to a button. 5B (the toggle, the
live `watch` into the parse worker, and startup resolution) has not started;
nothing in `src-tauri` changed in this phase.

---

## 1. What Shipped

1. **The crate skeleton, the pin, `SourceError`, `published`, `staging_names`**
   (`ac3773a`). `crates/vibrato-source` joins the workspace. Eleven constants
   pin the v0.5.0 IPADIC release asset (`ASSET_URL`, `ASSET_SHA256`,
   `ASSET_BYTES`, `DICT_DIR`, `SYSTEM_DIC`, and the rest), each value taken
   from Task 0's direct measurement of the real asset rather than an upstream
   page. `SourceError` mirrors `jmdict_source::SourceError`'s shape,
   including naming the target directory in the messages a user acts on.
   `published(hints_dir) -> Option<PathBuf>` never touches the network, and
   `staging_names` derives the `.partial.<pid>` names §4.2 relies on. Every
   dependency's licence was resolved before this commit: `rustls-graviola`
   under ISC, `lzma-rs`/`ruzstd`/`tar` under MIT — chosen so no crate in the
   tree carries an `AND Apache-2.0` term, which would collide with this
   project's GPL-2.0-only. 5 tests, clippy clean.

2. **`extract::unpack_into` and `publish`** (`94a0cdb`). The archive half,
   exercised with no network at all: xz-decoded bytes in, a tar walk that
   trusts only three whitelisted basenames (`COPYING`, `NOTICE`,
   `system.dic.zst`) written into a staging directory, `system.dic.zst`
   streamed through `ruzstd::decoding::StreamingDecoder` into the
   uncompressed `system.dic`, and `publish` renaming the whole staging
   directory onto the resolved name — so nothing at the resolved path exists
   until the archive is completely unpacked. A traversal fixture (a raw GNU
   header naming `../escaped.txt`, since `tar` 0.4.46 refuses to *write* a
   `..` path but does not restrict what it *reads*) proves the basename
   whitelist holds against a hand-crafted malicious archive, not just a
   well-formed one. 13 tests (5 + 8), clippy clean. See §4 below for the
   zstd content-checksum addition this commit made beyond the design spec.

3. **`fetch.rs`: the graviola TLS agent, SHA-256 verification, the retry
   policy, `resolve`** (`bfa7746`). `agent()` builds a `ureq::Agent` with
   `TlsProvider::Rustls`, `rustls_graviola::default_provider()` injected as
   the crypto provider, and `RootCerts::PlatformVerifier` — Task 0 had
   already confirmed this handshakes against `github.com` and its redirect
   to `objects.githubusercontent.com` with no fallback needed.
   `download_and_verify` streams to a staging file under a 32 MiB cap and
   hashes it before any decompressor sees a byte. The retry policy is copied
   from `jmdict-source`: `DOWNLOAD_ATTEMPTS` = 3, 2 s doubling backoff, a 4xx
   or a local `Io` failure returned immediately, everything else — including
   a hash mismatch, which a truncated or proxy-mangled body would produce —
   retried. `resolve(hints_dir)` tries `published` first and only reaches the
   network on a miss. 22 tests total (5 unit + 8 extract + 9 download, the
   download suite against a local `TcpListener`, never the real network),
   clippy clean.

4. **The `hints-source` feature and `jparser-cli fetch-hints`** (`53ee313`).
   `hints-source = ["dep:vibrato-source", "mecab"]` on `crates/jparser`,
   deliberately not implied by `cli` or `mecab` alone, so `src-tauri`
   enabling `mecab` today pulls in no TLS stack. `jparser-cli fetch-hints
   DIR` prints the published path, mirroring `fetch-source`. `parse --hints`'
   doc comment, which previously instructed `wget`/`tar`/`zstd -d` by hand,
   now points at `fetch-hints`. Two `cargo tree` checks confirmed the gate
   before it existed as CI: `--no-default-features` pulls in neither
   `vibrato-source` nor `rustls`/`ureq`, and `--features mecab` alone (no
   `hints-source`) pulls in neither `vibrato-source` nor `rustls` — `ureq`
   does appear there, but only via the pre-existing `jmdict-source` path
   (plain HTTP, no TLS feature), not this phase's. 236 lib tests + 6
   integration binaries including the new `cli_hints` (1/1), clippy clean.

5. **The two CI gates** (`54db839`). The library-purity grep gains
   `vibrato-source`, so `jparser --no-default-features` can never pull it in.
   The TLS gate widens from `openssl|native-tls` to also reject `ring` and
   `aws-lc`, with a comment naming `rustls-graviola` as the sanctioned
   provider — closing the gap where enabling ureq's default features would
   silently take `ring` back with nothing to notice. Both greps verified
   locally against the real dependency tree; `.github/workflows/ci.yml` is
   the only file this commit touches.

---

## 2. The Measured Facts

**Task 0's decode timings**, against the real 7.7 MB asset, no network
included:

- `lzma_rs::xz_decompress`: **156.9 ms** → 8,058,880 bytes (the intermediate
  tar).
- `ruzstd::decoding::StreamingDecoder`: **806.3 ms**.
- `vibrato::Dictionary::read`: **522.5 ms**.

None of the three is remotely near a pathological decode — the whole
pipeline, download excluded, is well under 1.5 s combined. `vibrato` 0.5.2 —
the version this tree pins — reads the dictionary published under the
upstream `v0.5.0` tag with no error and no size drift: exactly 47,788,814
bytes, matching the pinned constant.

**Task 6's end-to-end run**, on Windows, against the real asset:

- First fetch, into a directory that had never held it: **2.643 s wall**
  (network GET, SHA-256 verify, xz → tar → zstd unpack, staging-directory
  publish, all included).
- Second run, same directory: **0.252 s wall** — about 10.5x faster, the
  observable signal that `published` short-circuited the network path rather
  than a literal zero-cost no-op (the floor is `cargo run`'s own overhead).
- `system.dic` landed at exactly 47,788,814 bytes, with `COPYING` (3,839 B)
  and `NOTICE` (292 B) beside it, no `.partial` leftovers anywhere under the
  target directory.
- `parse --hints` against the real `gen-1` index, on the published
  dictionary, produced a clean 4-segment segmentation (東京都 / に / 住んで /
  います) with no `VibratoTokenizer::load` error.

**Asset identity**, pinned in `crates/vibrato-source/src/lib.rs` and verified
against the live download: 7,704,680 bytes, SHA-256
`4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9`, from the
vibrato v0.5.0 GitHub release. Inner member `system.dic.zst` is 8,045,952
bytes.

---

## 3. What 5B Inherits

- **`published(hints_dir) -> Option<PathBuf>`** as the network-free
  availability check. 5B's toggle needs to know whether MeCab is available
  before it can render, without a stray network call on every settings-window
  open; this function exists for exactly that caller.
- **`resolve(hints_dir) -> Result<PathBuf, SourceError>`** as the toggle's
  entry point: publish-first, network only on a miss. 5B calls this, not
  `fetch::fetch` or `fetch_with_retry` directly — those are test seams, named
  as such in the design spec §5.1.
- **`src-tauri` has no dependency on `vibrato-source` yet.** Nothing in this
  phase touched `src-tauri/Cargo.toml`, `main.rs`, `state.rs`, or any other
  file under `src-tauri`. 5B adds `vibrato-source` to `src-tauri` directly,
  the way 2F added `jmdict-source`.
- **`TA_HINTS_DICT` is unchanged** — still the env override 2C shipped,
  including the set-but-unreadable-is-fatal rule. 5B decides whether an
  absent published dictionary should mean "hints off" rather than a fatal;
  5A does not touch `state.rs::load_state` at all.

---

## 4. What Is Deliberately Absent

- **The MeCab toggle, the `watch` into the worker, and startup resolution.**
  Port design §11 gives these to 5B; §4.4's "paid for only when MeCab is
  switched on" describes that toggle, not this crate. Nothing in this phase
  decides *when* a dictionary gets fetched — only *how*.
- **Progress reporting.** Design spec §10: the download is 7.7 MB behind a
  toggle nobody has built yet, and 5B decides whether its UI needs a
  percentage.
- **Staging-directory sweeping.** Design spec §10, same reasoning
  `jmdict-source` already documents for its own `PARTIAL_SUFFIX`: a process
  cannot distinguish another's live staging directory from a dead one's
  leftovers, so sweeping by pattern would reintroduce the race the PID
  prevents. A killed fetch leaves up to ~46 MB of inert, never-resolved
  disk behind — a cost in disk, not correctness.
- **A hand-placed-archive path.** Design spec §10: `jmdict-source`'s exists
  because EDRDG's download fails in ways a user must be able to route
  around. GitHub over HTTPS has not earned that escape hatch, and §5.4's
  basename whitelist keeps the extractor safe if a later phase adds one
  anyway.

---

## 5. Findings From Implementation, Beyond the Design Spec

Three things surfaced while building this phase that are not in
`2026-08-27-jparser-phase5a-design.md` and need to travel with the code.

1. **`ruzstd` 0.9's `StreamingDecoder` never verifies the zstd frame's
   content checksum.** The design spec's §5.3 pipeline describes
   `ruzstd::decoding::StreamingDecoder -> system.dic` as if the decoder alone
   were sufficient. It is not: `StreamingDecoder`'s `Read` implementation
   calls `FrameDecoder::decode_blocks` and returns whatever bytes come out,
   but never itself compares the frame's trailing content checksum against
   what was actually decoded — the crate never references its own
   `get_checksum_from_data`/`get_calculated_checksum` pair anywhere in the
   streaming path. Task 2 found this the hard way: a corruption fixture that
   flips one byte in a tiny zstd block often lands in a raw/uncompressed
   block, which decodes "successfully" to the wrong byte with no structural
   failure at all. `extract.rs` now compares the two checksum values itself
   after `std::io::copy` completes and returns `SourceError::Archive` on a
   mismatch. This is a behavioural addition beyond the spec's §5.3, not an
   implementation detail of it.

   **Scoped by the fix wave (§10): the pinned asset carries no content
   checksum, so this check is inert on the real download.** The frame's
   content-checksum flag is optional, and it is clear on today's pin.
   Measured on the real archive: `ipadic-mecab-2_7_0/system.dic.zst` opens
   `28 b5 2f fd 00` — the magic, then a Frame_Header_Descriptor of `0x00`,
   whose bit 2 (`Content_Checksum_flag`) is 0. The `if let (Some, Some)`
   therefore never fires on the production asset. What the check does protect
   is the test fixtures (`ruzstd`'s encoder emits a checksum) and any future
   asset built with one. What protects today's download from a corrupt or
   truncated dictionary is `ASSET_SHA256` over the whole `.tar.xz`, verified
   before a decompressor sees a byte, plus xz's own integrity check. The
   original wording here stated the protection unconditionally; do not restate
   it that way.

2. **`ring v0.17.14` appears in `Cargo.lock`** as an optional, never-enabled
   dependency of `rustls-webpki`. `cargo metadata` lists it because Cargo
   records every package resolvable across all platforms and features, not
   just the ones active for this build; `cargo build -v`'s actual compile
   list never includes it, and `cargo tree -i ring` prints nothing. Task 5's
   CI gate greps `cargo tree --workspace`, which omits disabled optional
   dependencies, precisely so this phantom entry cannot false-positive it —
   worth recording so a future maintainer does not "fix" the gate into a
   `cargo metadata` grep on the theory that it is more thorough, and get a
   false positive on a crate that never links.

3. **The `TA_HINTS_DICT` verification in Task 6 is narrower than the design
   spec's §9.4 implies.** The desktop app surfaces every startup failure —
   including a `state::load_state` failure — only into the managed
   `StartupFailure` state for the webview to render; nothing is ever
   `eprintln!`'d to stdout or stderr. Launching `clipyomi.exe` headless with
   `TA_HINTS_DICT` set and watching it survive 30 s without crashing proves
   the app didn't panic, but an absence of output is not proof the
   dictionary loaded — a real load failure would be just as silent on that
   channel. The actual evidence for the env-override path is Task 6 Step 4's
   direct `parse --hints` run: the same `VibratoTokenizer::load` call, on the
   same `system.dic` file, from the CLI, which produced a real 4-segment
   parse with no error. The headless app launch corroborates (it ran; the
   only line it printed was the pre-existing, unrelated
   `frontend_ready`-timeout warning) but does not, on its own, prove the
   dictionary loaded.

---

## 6. The Verification Gate

Run and personally verified at commit `54db839` (tip of `develop`), on
Windows, tree clean before and after:

- **`cargo test --workspace`: 460 passed, 0 failed, 1 ignored** — up from
  Task 0's baseline of 438 / 0 / 1. The delta is exactly +22, matching
  `vibrato-source`'s own suite (5 unit + 8 extract + 9 download) newly
  present in the default workspace run; no other crate's count moved and
  nothing failed. No drop below baseline — not a regression.
- **`cargo test -p jparser --features hints-source`: 236 + 13 + 1 + 2 + 2 +
  15 + 4 + 4 = 277 passed, 0 failed** across the lib and every integration
  binary, including the new `cli_hints` (1/1).
- **`cargo test -p jparser --features mecab`: 236 + 13 + 2 + 2 + 15 + 4 + 4 =
  276 passed, 0 failed** — the same suite minus `cli_hints`, which requires
  `hints-source` specifically (`mecab` alone does not pull in
  `vibrato-source`, confirmed again by this run).
- **`cargo check -p jparser --no-default-features --all-targets`: clean.**
- **`npx vitest run`: 252 passed, 13 files** — unchanged from Task 0's
  baseline, exactly as expected: this phase touches no frontend file.
- **`npx playwright test`: 45 passed.** No pre-phase baseline exists for
  this number (Task 0's own plan omitted capturing one); recorded as
  observed rather than diffed, per the phase's ruling. `git status --short`
  showed no screenshot baseline touched by the run.
- **`cargo clippy --all-targets 2>&1 | grep -c "^warning"`: 6** — unchanged
  from Task 0's baseline. `vibrato-source` contributes zero.
- **`cargo clippy --workspace --all-targets -- -D warnings`: fails on
  Windows, for a reason that predates this phase and is out of its scope.**
  See below.

**On the `-D warnings` workspace run.** `.github/workflows/ci.yml` runs
this exact command on `ubuntu-latest`, where it is *expected* to pass.
That is an inference from the `#[cfg]` reasoning below, not an
observation: this repository has no git remote, so no run of this
workflow has ever been seen, and the fix wave in §10 found that the
`purity` job's TLS grep would in fact have failed on that runner. Read
every claim about CI in this section as an expectation.

Run locally on Windows it fails with four errors, all
`clippy::upper_case_acronyms` on `RECT`, `STYLESTRUCT`, `WINDOWPOS` and
`SUBCLASSPROC` in
`src-tauri/src/commands.rs` — the same four warnings Task 0 and Step 2 above
already count as the pre-existing baseline of 6 warning lines, and the same
four the Phase 4D handoff §3 already recorded as predating that phase too.
They are behind Windows-specific `#[cfg]` blocks that never compile on
Linux, which is exactly why CI runs this gate on `ubuntu-latest` rather than
a Windows runner. `-D warnings` simply promotes an existing warning to a
hard error; it does not introduce a new one. To confirm this phase
contributes none of it, the same command was re-run excluding `clipyomi`
(the `src-tauri` package):

```
cargo clippy --workspace --all-targets --exclude clipyomi -- -D warnings
    Checking xtask v0.1.0
    Checking jparser v0.1.0
    Checking vibrato-source v0.1.0
    Checking jmdict-source v0.1.0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.29s
```

Clean. `vibrato-source`, `jparser`, `jmdict-source` and `xtask` are all
clippy-clean under `-D warnings`; the failure is entirely `src-tauri`, which
this phase does not touch and which 5B — the phase that does touch
`src-tauri` — inherits unchanged.

---

## 7. Deviations to Fold Into the Master Spec

Rows 16, 17 and 18 (verbatim from the design spec's §11), appended to
`docs/superpowers/specs/2026-08-12-jparser-port-design.md`'s §12 table. §11's
Phase 5 table 5A row marked shipped. See §8 below for the exact edits.

---

## 8. Master Spec Changes Made at This Close

`docs/superpowers/specs/2026-08-12-jparser-port-design.md`:

- **§11's 5A row**: rewritten to past tense, in the voice of the Phase 4
  rows above it — states what shipped (the crate, the hash-pinned download,
  the xz/tar/zstd unpack, the atomic directory-rename publish, the CLI
  driver behind `hints-source`) and that `TA_HINTS_DICT` survives unchanged.
- **§12**: gained rows 16, 17 and 18, copied verbatim from the design spec's
  §11 — the SHA-256 pin JMdict lacks, the version-named directory instead of
  a numbered generation, and `rustls` + `rustls-graviola` as the first HTTPS
  in the tree.

No other text in that file was touched at this close.

---

## 9. What This Phase Did Not Clear

Nothing in this phase re-opens or touches any item from the 4D handoff's §4
or §5 — none of that work runs through `crates/vibrato-source` or
`crates/jparser`, and `src-tauri` is untouched. Everything 4D left
outstanding (the manual checklists, `docs/superpowers/2026-08-18-windows-
titlebar-test-plan.md`, `set_chrome_mode`'s persist ordering, and the rest)
remains exactly as that handoff recorded it, for 5B or a later phase to pick
up.

This phase's own unrun item: the six items in design spec §9.4 beyond the
one performed in Task 6 — specifically, no macOS run of any kind, and no
verification that a killed fetch mid-download actually leaves a `.partial`
directory and no half-written `system.dic` at the resolved name (asserted
only by the unit/integration tests' controlled fixtures, never against the
real 46 MB asset under a real interruption).

---

## 10. Post-Close Fix Wave

A whole-branch review after the close raised fifteen findings against the code
above. All fourteen code/doc items were fixed in one pass; nothing new was
built and no dependency was added. What changed, and why it is not visible in
§1:

- **`ci.yml`'s TLS grep failed on the runner it runs on.** The pattern was
  case-insensitive and unanchored, so `openssl` matched `openssl-probe
  v0.2.1`, which `rustls-native-certs` pulls in on Linux under
  `rustls-platform-verifier`. That crate links nothing and is MIT OR
  Apache-2.0 — a false positive, but a fatal one, since the `purity` job only
  ever runs on `ubuntu-latest`. Now anchored to `openssl v|openssl-sys|
  native-tls|\bring v|aws-lc` with `-i` dropped, and commented so it is not
  re-broadened. Confirmed clean for both `--target x86_64-unknown-linux-gnu`
  and the Windows host, and confirmed still to catch `openssl v`,
  `openssl-sys`, `native-tls`, `ring v`, `aws-lc-rs` and `aws-lc-sys`.
- **`vibrato-source` declared a Rust floor it cannot keep.** It inherited the
  workspace's 1.85; `graviola 0.4.1` (under `rustls-graviola`) declares 1.89
  and `ruzstd 0.9.0` declares 1.87, both read from the resolved packages, and
  Cargo treats that as a hard error rather than a warning. Now an explicit
  `rust-version = "1.89"` with the same reasoning `src-tauri/Cargo.toml`
  already carries. The `msrv` job is unaffected: it never compiles this crate.
- **`staging_names` keyed on the PID alone**, so two calls in one process
  shared both names. With 5B calling `resolve` from a UI toggle, that is a
  real shape: one thread publishes, the second `File::create`s what is now the
  published dictionary and truncates 46 MB to zero while `published()` still
  reports it present. A process-wide `AtomicU64` now uniquifies both names per
  call, and the doc comments that claimed the protection say what actually
  provides it.
- **Neither decompression layer bounded its output.** `lzma-rs` 0.3 has no
  bounded xz entry point, so the bound is a limiting `Write` adapter used on
  both layers, capped at 12 MiB (tar; measured 8,058,880) and 64 MiB
  (dictionary; measured 47,788,814). An overrun is `SourceError::Archive`.
- **A full disk was reported as a corrupt archive and retried.** `io::copy`
  collapses a decode failure, a cap overrun and a failed write into one
  `io::Error`, and all three were mapped to `Archive` — so an ENOSPC
  re-downloaded 7.7 MB and re-decompressed it twice more onto a still-full
  disk. The limiting writer now remembers which of the two *it* produced, and
  a write failure surfaces as `SourceError::Io` (immediate return). The
  `COPYING`/`NOTICE` branch is classified the same way, so a read failure
  there is `Archive` like the dictionary branch, not `Io`.
- **A rename failure discarded 46 MB and named neither path.** The realistic
  trigger is a `<hints>/ipadic-mecab-2_7_0/` left by the `tar xf` instructions
  this phase deleted — the resolved name, non-empty, no `system.dic` — which
  made every run fail identically with `hints io failed: Directory not empty
  (os error 39)`. Now `SourceError::Publish`, carrying both paths and telling
  the user to remove or rename the directory. Non-retrying, like `Io`.
- **No job compiled `hints-source`**, so `tests/cli_hints.rs` and the whole
  `Command::FetchHints` arm were type-checked nowhere in CI. Design spec §9.5
  lists this gate; a `hints` job mirroring `mecab` now runs
  `cargo test -p jparser --features hints-source` and the matching
  `clippy ... -D warnings`.
- **`hints_dir` was carried in two error variants and rendered in neither.**
  Both now end with `nothing was written to <dir>` / `nothing was published in
  <dir>`, the way `jmdict_source::SourceError` renders its `source_dir`.
- **The `resolve` test stood up no server**, despite its own doc claiming the
  guarantee was "asserted by the server never being connected to". It passed
  only because `published()` short-circuits — a regression there would have
  sent the suite to the live `ASSET_URL`. A `resolve_from(url, sha, dir,
  backoff)` injection point was added (the shape `jmdict_source::resolve_from`
  already has), and the test now drives it against a local listener and
  asserts zero connections before touching `resolve` itself.
- **`ASSET_BYTES` was dead.** It is now checked in `download_and_verify`
  before the hash, gated on the hash being the pinned one, and reported as
  `SourceError::Size`. The tautological `assert_eq!(ASSET_BYTES, 7_704_680)`
  in the unit tests is gone; a served body of the wrong length under the
  pinned hash proves it instead.
- **The traversal test passed for the wrong reason.** `../escaped.txt` has an
  un-whitelisted basename, so it was skipped as an unrecognised member — a
  path another test already covers. The fixture is now `../COPYING`: a
  whitelisted basename behind a `..`, which is the case where basename
  extraction is actually load-bearing.
- **`[[test]] cli_hints` needed `required-features = ["hints-source", "cli"]`.**
  The stated mechanism was slightly off — measured, the test still *compiles*
  under `--no-default-features --features hints-source`, because Cargo sets
  `CARGO_BIN_EXE_jparser-cli` regardless; what fails is the run, with
  `Os { code: 2, kind: NotFound }`, because the binary is
  `required-features = ["cli"]` and was never built. Either way the fix is the
  one named, and Cargo now says "requires the features: `hints-source`, `cli`"
  instead.
- **The zstd content-checksum claim was scoped to reality** — see §5.1. The
  pinned asset's frame carries no checksum.
- **§6's CI claim was restated as an expectation**, since this repo has no
  remote and the `purity` job would in fact have failed.

One finding produced no test. A read failure from the tar cannot be provoked
in-process: `unpack_into` reads the tar from a `std::io::Cursor<Vec<u8>>`,
whose reads never fail, and a member truncated at end-of-data returns `Ok(0)`
through `io::Take` rather than an error. The classification fix stands (it is
correct for a sparse-entry or long-name failure, which `tar` *can* raise), but
it is unreachable from a fixture, so no test asserts it.

Gate after the wave, on Windows, `rustc 1.97.1`:

- `cargo test --workspace`: **464 passed, 0 failed, 1 ignored** — up from the
  close's 460 / 0 / 1. The +4 is `vibrato-source`'s own suite going 22 -> 26
  (two cap tests, one publish-blocked test, one size-pin test; the traversal
  test was rewritten rather than added).
- `cargo test -p jparser --features hints-source`: **277 passed, 0 failed** —
  unchanged.
- `cargo clippy -p vibrato-source --all-targets -- -D warnings`: clean.
- `cargo clippy -p jparser --features hints-source --all-targets -- -D warnings`:
  clean (the new CI job, run locally).
- `cargo check -p jparser --no-default-features --all-targets`: clean.
- Both `cargo tree` TLS greps: clean, Linux target and Windows host.
