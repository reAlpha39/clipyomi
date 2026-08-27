# JParser Phase 5B — Handoff

**Date:** 2026-08-27
**Spec:** [`docs/superpowers/specs/2026-08-27-jparser-phase5b-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-27-jparser-phase5b-design.md)
**Commits:** `6685b61..772856a` on `develop` (eight commits across five tasks, unpushed), plus this task's own doc commit closing the phase, plus the post-close fix in §10.

This is Phase 5B of Phase 5 (MeCab). 5A shipped acquisition only — download,
verify, unpack, publish, headless — as a library and a CLI driver. 5B is the
toggle: a persisted setting, `set_mecab_hints` acquiring the dictionary on
demand and publishing the loaded tokenizer to the running parse worker over a
second `watch`, and startup resolution that turns an absent or unloadable
dictionary into a warning instead of 2C's fatal. Phase 5 is now complete.

---

## 1. What Shipped

1. **One version-blind slot and a hand-placed archive route** (`6685b61`,
   fixed by `4db1c0b`). `crates/vibrato-source`: `DICT_DIR` becomes `current`
   (row 19); `hand_placed(hints_dir) -> Option<PathBuf>` finds the newest
   `.tar.xz` directly in the hints directory, ignoring dotfiles and staging
   names; `publish_replacing` swaps a new dictionary in behind a rename-based
   backup, restoring it on a failed second rename; `resolve_from` gained the
   two-step hand-placed branch ahead of its existing publish-then-fetch
   fallback, deliberately unhashed (row 20). `4db1c0b` fixed a review-round
   Critical: `crates/jparser/tests/cli_hints.rs` hardcoded the old
   `ipadic-mecab-2_7_0` slot name, which after the `DICT_DIR` rename would
   have fallen through to a real 7.7 MB download on every CI run of the
   `hints` job — repointed at `vibrato_source::DICT_DIR`/`SYSTEM_DIC`.
   `vibrato-source`'s own suite: **5 unit + 17 extract + 13 download = 35
   tests**, real counts corrected from the plan's stale prediction (§5 below).

2. **`write_test_dictionary` exposed for downstream tests** (`f746f6c`).
   `crates/jparser/src/hints.rs` lifts the two-entry in-memory IPADIC fixture
   its own tests already built via `vibrato::SystemDictionaryBuilder` into a
   `#[doc(hidden)] pub fn write_test_dictionary(path) -> Result<(),
   HintsError>` behind the `mecab` feature — what lets `src-tauri`'s tests
   exercise the real `VibratoTokenizer::load` without a 46 MB file or a
   network call. One new test, `the_written_test_dictionary_loads_and_tokenizes`
   (jparser lib: 236 → 237).

3. **Hints carried on their own `watch` channel** (`7214b0e`, relocated by
   `b8478d8`). `src-tauri`: `Settings::mecab_hints` (serde, defaults off);
   `HintsSender(watch::Sender<Option<Arc<VibratoTokenizer>>>)`; `HintsWarning`
   as the shared string both windows read; `state::load_hints`'s five
   branches (`TA_HINTS_DICT` fatal, setting off, no published dictionary,
   published-and-loads, published-but-unloadable); `AppState.hints` deleted;
   `run_worker` takes the hints receiver as its third `watch` alongside the
   index and settings channels it already had; `current_hints` reads it the
   same way `current_index` reads the index channel.
   `b8478d8` relocated `settings.rs`'s `#[cfg(test)] mod tests` into
   `src-tauri/src/settings/tests.rs` — a pure move, same test count — after a
   review round found `settings.rs` had crossed the phase's 800-line ceiling
   on this task's own addition (settings.rs 265, settings/tests.rs 554).
   clipyomi: **134 tests**, clippy warning count 6 (baseline).

4. **`set_mecab_hints` acquires and publishes the tokenizer** (`ea4dcd8`,
   reworded by `c1543ee`). The command: off is synchronous (`hints_tx.send
   (None)`, persist `false`, reparse, emit `off`); on claims `HintsInFlight`,
   picks the status label (`unpacking` if `hand_placed` finds an archive,
   else `downloading`) with one `read_dir` before calling `resolve` (the
   label is cosmetic — a wrong guess costs a mislabelled phase, not a wrong
   outcome), runs `resolve` + `VibratoTokenizer::load` on `spawn_blocking`,
   and on success sends the tokenizer, persists `true` (only on success —
   never claims hints it does not have), reparses what is on screen, and
   emits `ready`. `hints_warning` and `hints_dir` commands added, both reading
   state the two windows need. `c1543ee` remapped `claim_download`'s shared
   "a dictionary download is already running" text to a hints-specific
   message at the call site, since the helper's wording was written for the
   JMdict slot it also guards. clipyomi: **138 tests**, clippy 6.

5. **The MeCab boundary hints toggle and its warning** (`772856a`). Frontend:
   the settings-window checkbox and its status line (`#setting-mecab-hints-
   status`); `HINTS_PHASES`/`applyHintsStatus` mapping the four `hints-status`
   event payloads (`downloading`/`unpacking`/`ready`/`off`) to copy, with any
   other payload shown verbatim as the failure message; the checkbox
   disabled from click to a terminal event; `showHintsWarning` appending to
   `#parse-error` alongside the two warnings already asserted there, and
   seeding the settings window's status line from `hints_warning` on load.
   `types.ts` gains `mecab_hints?: boolean`, read-only — `merge_saved` never
   writes it, matching the constraint that the backend owns this key
   outright. vitest: **263** (up from Task 0's baseline 252), tsc clean.

---

## 2. The Measured Facts

Steps 2–7's manual checks need a running GUI, which this task cannot drive
(see the controller's ruling, §6.1 item 12). In its place, Task 6 ran the
substitute the controller specified: the 5A CLI driver, against scratch
directories, never the user's app config dir
(`C:\Users\realpha39\AppData\Roaming\dev.jparser.clipyomi`, untouched
throughout).

### 2.1 The substitute acquisition check — real asset, real network

```
SCRATCH=$(mktemp -d)/hints   # /tmp/tmp.FRGH2ZXoSY/hints
cargo run -p jparser --features hints-source --bin jparser-cli -- fetch-hints "$SCRATCH"
```

- **First fetch, into a directory that had never held it: 5.478 s wall**
  (already-built binary; the real HTTPS GET against
  `github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz`,
  SHA-256 verify, xz → tar → zstd unpack, staging-directory publish, all
  included). `system.dic` landed at exactly **47,788,814 bytes**, `COPYING`
  at **3,839 bytes**, `NOTICE` at **292 bytes**, all beside it at
  `$SCRATCH/current/`. `find "$SCRATCH" -iname "*.partial*" -o -iname
  "*.backup*"` returned nothing.
- **Second run, same directory: 0.303 s wall** — about 18x faster than the
  first, the observable signal `published()` short-circuited rather than
  re-hitting the network. Directory contents unchanged.

Both numbers are consistent with 5A's own Task 6 measurement of the same
pipeline (2.643 s / 0.252 s, on a different run of the same machine) — the
absolute times differ with network conditions, the ratio does not.

### 2.2 The hand-placed-archive branch — attempted, and why it could not be driven through the CLI

A real archive was obtained without inventing one: `ASSET_URL` was fetched a
second time directly (`curl`), landing 7,704,680 bytes whose SHA-256,
`4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9`, matches
`vibrato_source::ASSET_SHA256` exactly — this is the same pinned asset 5A's
handoff records, downloaded again rather than fabricated.

Following the brief's fallback procedure: `$SCRATCH/current/system.dic` was
touched to `2020-01-01`, the archive was copied into `$SCRATCH/`, and
`fetch-hints "$SCRATCH"` was run again. It returned in **0.238 s** and
`$SCRATCH/current/`'s own directory metadata (`birth`/`change` times, checked
with `stat`) was unchanged from the very first fetch — the unpack-and-publish
path did not run.

Tracing why: `crates/jparser/src/bin/jparser-cli.rs`'s `Command::FetchHints`
arm (lines 216–223) is

```rust
let path = match vibrato_source::published(&dir) {
    Some(path) => path,
    None => vibrato_source::fetch::fetch(&dir)?,
};
```

— `published()` then, on a miss, the plain network `fetch()`. It never calls
`vibrato_source::resolve()` / `resolve_from()`, which is the only function
that implements the hand-placed-archive branch (`crates/vibrato-source/src/
lib.rs:399–431`, steps 1–2 of `resolve_from`). Grepping the tree for callers
of `vibrato_source::resolve` finds exactly one in production code:
`set_mecab_hints` at `src-tauri/src/commands.rs:1600` — reachable only
through the running app's settings-window toggle, the same GUI surface this
task is barred from driving.

So **this sub-check cannot be performed through `jparser-cli fetch-hints` at
all** — not for lack of a real, hash-verified archive (one was obtained), but
because the CLI driver was never wired to the code path under test. This is
recorded as unrun via the CLI, not fabricated as passing. The branch's logic
itself is exercised — with fixture archives, not the real asset — by
`vibrato-source`'s own suite, which passed as part of the Step 1 gate below:
`a_newer_hand_placed_archive_wins_without_touching_the_network`,
`an_older_hand_placed_archive_loses_to_the_published_slot`,
`a_hand_placed_archive_is_published_without_a_hash_check`,
`the_newest_hand_placed_archive_wins`,
`dotfiles_and_staging_archives_are_not_hand_placed`,
`an_absent_hints_directory_has_no_hand_placed_archive`. See §5 and §8.

### 2.3 The six GUI manual checks — one run post-close, five outstanding

Item 1 has since been run by the user, and it **failed** — see §10. The other
five did not run. They are handed to a human with a window to click, phrased
so they can be executed directly. All assume a fresh `<app_config_dir>/hints`
(move any existing one aside first) and `npm run tauri dev`.

1. **Fresh install downloads and takes effect live. — RUN, failed, fixed;
   see §10.** Copy `すもももももももものうち` to the clipboard and note the
   segmentation. Open Settings, tick **Boundary hints (MeCab)**. Expect the
   status line to read "Downloading the dictionary…", then clear with the box
   checked, and the on-screen sentence to re-segment **without a restart**.

   **Do not use `東京都に住んでいます` for this check**, which is what this
   section originally named. Measured against the real `gen-1` index, it
   parses identically with and without hints: JMdict carries `東京都` as a
   single headword ("Tokyo Metropolis"), so the three-character match wins
   outright either way and the check cannot fail. The 5A handoff's use of it
   was to show hints *working*, not to discriminate. Nine candidate sentences
   were compared through `jparser-cli parse` with and without `--hints`
   against the real index; **seven were identical** — including
   `外国人参政権`, `東京特許許可局`, `ここではきものを脱ぐ` and
   `私は毎日新聞を読んでいます`, because JMdict already holds those compounds
   as entries and longest-match wins regardless. Two differed:

   | Text | Hints off | Hints on |
   | --- | --- | --- |
   | `すもももももももものうち` | すもも / もも / もも / もも / のうち | すもも / も / もも / も / もも / のうち |
   | `くるまではこう` | くるま / では / こう | くる / まで / はこう |

   The `すもも` row is the better demo — hints recover the `も` particles — and
   is what this check should use. The wider result is worth carrying forward
   on its own: **on real JMdict, hints rarely change kanji-compound
   segmentation**; they earn their keep on kana-heavy ambiguity. 6A's
   differential run should expect that distribution rather than the kanji
   examples the design documents lead with.
2. **The second toggle is offline and instant.** Untick, then re-tick.
   Expect `ready` effectively immediately, "Unpacking the archive…" never
   shown, and no network activity — `published()` short-circuited inside
   `resolve`.
3. **A hand-placed archive replaces the dictionary.** With the app running,
   copy the 5A asset (or another IPADIC-sized Vibrato `.tar.xz` — the
   extractor's caps refuse UniDic/NEologd-sized ones, see §5) into
   `<app_config_dir>/hints/`, then untick and re-tick. Expect "Unpacking the
   archive…", `<hints>/current/` afterwards holding the archive's dictionary,
   and `ls -a <hints>` showing no `.partial` or `.backup` leftovers.
4. **A missing dictionary is a warning, not a fatal.** Quit. Delete
   `<app_config_dir>/hints/current/` while `settings.json` still has
   `"mecab_hints": true`. Relaunch. Expect the app to start normally, the
   warning to appear in **both** the main window's `#parse-error` and the
   settings window's status line, and the checkbox to still read on.
5. **An unloadable dictionary names the format.** Quit. Write junk over
   `<hints>/current/system.dic`. Relaunch, open Settings, untick and re-tick.
   Expect the failure message to name the v0.5.0 format, the checkbox to
   fall back to unchecked, and `settings.json` to still hold
   `"mecab_hints": false` afterwards.
6. **The env override is still fatal.** Quit. Launch with `TA_HINTS_DICT`
   pointing at a path that does not exist. Expect a startup failure in
   `#output`, exactly as 2C shipped it. **5A's caveat applies unchanged**:
   the desktop app surfaces startup failures only into the managed
   `StartupFailure` state for the webview, never to stderr — "it didn't
   print anything" is not evidence either way. The evidence has to be the
   message rendered in the window.

---

## 3. What 6A/6B Inherits

- **The `hints-status` event contract**: `downloading` / `unpacking` /
  `ready` / `off`, plus any other string shown verbatim as the failure
  message. 6B's Playwright coverage, when it reaches the settings window,
  drives the same four labels this phase's vitest suite already asserts.
- **`HintsWarning`**, read by `hints_warning` and shown identically in both
  windows — the main window's `#parse-error`, the settings window's status
  line on load.
- **The settings window is still absent from the Playwright suite.** No
  Playwright test was added this phase (design spec §10.3: "adding it is
  6B's scope, not this phase's"). The 45 Playwright tests that pass today
  cover the main window only; the toggle's full lifecycle is proven only by
  vitest and, once run, the manual checks in §2.3.

---

## 4. What Is Deliberately Absent

Copied from the design spec's §12, unchanged by implementation:

- **Detecting a non-IPADIC dictionary.** `hints.rs:22` hardcodes
  `READING_FIELD = 7` as IPADIC's reading column. A UniDic or NEologd
  dictionary dropped into the slot loads without error and puts something
  else in field 7, so hints degrade silently rather than failing. Detection
  means probing a known token's feature layout at load — a guess with a
  false-positive cost that would refuse a working dictionary. The settings
  note naming IPADIC is the honest fix for now.
- **Installing a hand-placed archive at startup.** Only the toggle installs
  one; a dropped-in archive is inert until the next off-and-on.
- **A progress percentage.** The status line shows a phase label, not a
  number.
- **Sweeping staging directories.** Inherited from 5A unchanged, for the
  reason `jmdict-source` already documents: a process cannot distinguish
  another's live staging directory from a dead one's leftovers.
- **Layout slots capturing `mecab_hints`.** Out of scope for this phase.
- **Dictionary updates as a first-class action.** Dropping a newer archive in
  is the whole mechanism. Nothing checks for a new release.

---

## 5. Findings From Implementation, Beyond the Design Spec

- **`reparse` and the history dedupe encode contradictory invariants, and
  nothing in either phase could see it.** Found post-close by the user, on
  the first run of §2.3's check 1; fixed in §10. Design §6.2 requires the
  re-parse to reach the screen; Phase 3's `src/history.ts` dedupes on the
  surface text — "what the user copied, independent of how it segmented" — so
  it dropped exactly the result §6.2 exists to deliver. Each invariant is
  correct in isolation, each had passing tests, and no review read both:
  5B's task reviews were scoped to 5B's diff, and `history.ts` is not in it.
  **The lesson for later phases is about the review's scope, not the code**:
  a change whose whole purpose is to make an existing surface behave
  differently has to be reviewed against that surface's existing rules, which
  a diff-scoped review by construction cannot see.

- **The plan's predicted per-file test counts were stale.** The plan
  (`docs/superpowers/plans/2026-08-27-jparser-phase5b.md`) derived its
  step-16-style predictions from 5A's handoff, but the tree had already
  moved by the time this phase started — Task 0 measured 464 workspace
  tests where 5A's own handoff had recorded 460 as its close-of-phase
  number (464 was 5A's own post-close fix-wave total). Task 1's real counts
  for `vibrato-source` came out as **5 + 17 + 13**, not the plan's stale
  prediction. The deltas per file match exactly the tests the brief itself
  adds; nothing extra snuck in. Recorded as a plan defect, not an
  implementer deviation (Task 1 ledger entry).

- **Three plan snippets, as written, would have broken this phase's own
  clippy gate**, and were corrected inside the tasks that hit them rather
  than deferred to a separate fix task:
  1. `crates/jparser/tests/cli_hints.rs` hardcoded the old published-slot
     name `ipadic-mecab-2_7_0`. After `DICT_DIR` became `current`, a CI run
     of that test would have fallen through to a real 7.7 MB GitHub
     download. Fixed in Task 1 (`4db1c0b`) by pointing the test at
     `vibrato_source::DICT_DIR`/`SYSTEM_DIC`.
  2. The brief's literal `let mut settings = Settings::default(); settings.
     mecab_hints = true;` in the settings round-trip test trips
     `clippy::field_reassign_with_default`. Task 3 substituted struct-update
     syntax instead — same round trip asserted, clean under `-D warnings`.
  3. The brief's `let (tx, mut rx) = watch::channel(...)` in
     `reparse_skips_an_empty_channel` trips `clippy::unused_mut`, because
     `rx.has_changed()` takes `&self`. Task 4 dropped the `mut` — the
     assertion is unchanged.

  All three are plan-snippet defects, not implementer deviations from the
  brief's intent; each is recorded as its own ruling in §6.1.

- **`jparser-cli fetch-hints` has no route to the hand-placed-archive
  branch**, discovered while attempting Task 6's substitute check — see §2.2
  for the full trace. The CLI's `FetchHints` arm calls `published()` then
  `fetch()`, never `resolve()`/`resolve_from()`; only `set_mecab_hints`
  (GUI-only) calls `resolve()`. This is not a regression this phase
  introduced — the design spec's own §9 notes "`jparser`'s `hints-source`
  feature is not touched... it stays what 5A made it — the CLI driver's
  gate" — but it means **no automated or scriptable surface in this tree
  drives the hand-placed-archive branch against the real 46 MB asset**;
  only fixture-sized archives are exercised there (§2.2's test list), and
  the real-asset path is provable only by the manual check in §2.3 item 3.
  Worth carrying forward: a `jparser-cli` command that calls `resolve()`
  the way `set_mecab_hints` does would close this gap without needing the
  GUI, but adding one is a code change outside this task's scope.

- **The "accepts any Vibrato dictionary" claim, repeated in this spec's §2,
  the port-design §11 5B row and this handoff, was a spec defect.**
  `extract.rs`'s caps — `MAX_TAR_BYTES` = 12 MiB, `MAX_DICT_BYTES` = 64 MiB —
  are sized at ~1.5x/1.4x the pinned IPADIC asset. Every other Vibrato
  release asset (UniDic, BCCWJ, NEologd) exceeds both by a large multiple, so
  the hand-placed route in practice accepts IPADIC-sized archives and
  nothing else, and a too-large archive fails with a message that reads like
  corruption rather than a size mismatch. Found by the phase's final
  whole-branch review, not by this task. Corrected in this spec, the design
  spec's §2 and §12, and the port-design §11 row, without raising the caps —
  they are a real disk-fill guard, and §12's deferred non-IPADIC detection
  already names the settings note as the honest mitigation for what a
  correctly-sized non-IPADIC dictionary would still do silently.

---

## 6. The Verification Gate

Run and personally verified at commit `772856a` (tip of `develop` before this
task's own doc commit), on Windows, `rustc 1.97.1`, tree clean before and
after every command:

| Command | Result | Baseline | |
| --- | --- | --- | --- |
| `cargo test --workspace` | **487 passed, 0 failed, 1 ignored** | 464 / 0 / 1 | at/above baseline |
| `cargo test -p jparser --features hints-source` | **278 passed, 0 failed** (237 lib + 0 bin + 13 + 1 + 2 + 2 + 15 + 4 + 4) | 277, per 5A's own final gate at the same starting commit `450bc8d` | at/above |
| `cargo check -p jparser --no-default-features --all-targets` | clean | clean | matches |
| `cargo clippy --workspace --all-targets --exclude clipyomi -- -D warnings` | clean | clean | matches |
| `cargo clippy -p clipyomi --all-targets \| grep -c "^warning"` | **6** | 6 | exactly equal |
| `npx vitest run` | **263 passed, 13 files** | 252 in 13 files | at/above |
| `npx playwright test` | **45 passed** | 45 | exactly equal (unchanged — this phase touches no Playwright-covered surface) |
| `cargo tree --workspace \| grep -E "openssl v\|openssl-sys\|native-tls\|\bring v\|aws-lc"` | clean | clean | matches |
| `cargo tree --workspace --target x86_64-unknown-linux-gnu \| grep -E "..."` | clean | clean | matches |

These numbers are as of `772856a` and stand. The post-close fix in §10 moves
`npx vitest run` to **265 passed, 13 files**; nothing else in this table
changed with it, because that fix touches no Rust.

`cargo test --workspace`'s +23 over baseline (464 → 487) is accounted for as
follows: `vibrato-source` contributes a verified **+9** (26 → 35, Task 1's own
suite growth, the same 22 → 26 → 35 progression the 5A and 5B ledgers both
record), and `jparser` lib contributes a verified **+1** (236 → 237, traced
directly to Task 2's single new test, `the_written_test_dictionary_loads_and_
tokenizes`). The remaining **+13** belongs to `clipyomi`, whose growth across
Tasks 3–4 the ledger records reaching 134 tests after Task 3 and 138 after
Task 4's fix round — but the ledger never captured a Task-0 baseline for
`clipyomi` specifically, only the workspace-wide total, so 138 minus a
verified per-crate baseline cannot be shown here; only the workspace-wide
arithmetic (464 + 9 + 1 + 13 = 487) is asserted. No count fell below
baseline in any crate this task could observe directly.

**Note on feature unification.** `cargo test --workspace`'s plain invocation
(no `--features`) still shows `jparser` lib at 237 — the same count as the
`hints-source` run — because `clipyomi` (a workspace member) depends on
`jparser` with `mecab` enabled, and Cargo unifies that feature across the
whole `--workspace` build even though `jparser` itself defaults to `cli`
only. `cli_hints` still does not appear in the plain run's binary list,
because `hints-source` is not implied by `mecab` alone — confirming the
`hints-source`/`mecab` separation the design spec requires still holds.

**The known-inherited failure.** `cargo clippy --workspace --all-targets --
-D warnings` (including `clipyomi`) was **not run** as a gate, per this
task's own instructions: it fails on Windows for four pre-existing
`clippy::upper_case_acronyms` errors on `RECT`, `STYLESTRUCT`, `WINDOWPOS`
and `SUBCLASSPROC` in `src-tauri/src/commands.rs`, behind Windows-only
`#[cfg]` blocks. These are the same four warnings the 4D and 5A handoffs both
record as predating those phases, and CI runs this gate on `ubuntu-latest`
where those `#[cfg]` blocks never compile — which is why the errors have
never actually failed a CI run. **This phase neither fixed them nor added to
them**, confirmed by the `--exclude clipyomi` run above being clean and by
the `clipyomi`-only warning count being exactly 6, matching baseline.

### 6.1 The controller's rulings, taken during implementation

Every decision made on the user's behalf, from the phase ledger
(`.superpowers/sdd/2026-08-27-jparser-phase5b/progress.md`), with what was
decided and why:

1. **Task 1 — `cli_hints.rs`'s hardcoded slot name is a real, plan-level
   defect, not a reviewer misread.** Verified by hand (grep + reading the
   test + `jparser-cli.rs`'s `FetchHints` arm): after `DICT_DIR` became
   `current`, the test's old `ipadic-mecab-2_7_0` path would fall through to
   a real 7.7 MB GitHub download on every CI run. Decided: fix inside Task
   1's own commit range by pointing the test at `vibrato_source::DICT_DIR`/
   `SYSTEM_DIC`, rather than opening a separate task. Cost if wrong: a
   one-line test edit lands in the wrong task's commit range — no
   behavioural risk.

2. **Task 1 — the plan's predicted per-file test counts (steps 2/16) are
   stale, derived from 5A's handoff while the tree had already moved.** Real
   baseline was 11 extract / 10 download, not 8 / 9. Recorded as a minor,
   deferred — no ruling needed, no code affected.

3. **Task 1 — `publish_replacing`'s doc overstates its own guarantee** ("no
   window in which the user has no dictionary"; strictly there is a
   crash-only window between the two renames). Deferred: the brief mandates
   the code, not the doc's exact wording.

4. **Task 3 — the settings round-trip test's struct-update syntax stands in
   for the brief's literal `let mut settings = Settings::default();
   settings.mecab_hints = true;`.** The brief's literal form trips
   `clippy::field_reassign_with_default`, which would have broken this
   phase's own warning-count gate. Decided: the plan snippet was wrong, not
   the implementer; accept the substitution. Cost if wrong: none
   behavioural — the test asserts the identical round trip.

5. **Task 3 — `#[allow(dead_code)]` accepted on `HintsSender`,
   `DictionaryPaths.hints_dir` and `HintsWarning` for Task 3's duration
   only, carried into Task 4 as a removal requirement.** All three exist in
   Task 3 with their only readers arriving in Task 4, and `src-tauri` is a
   binary crate where `dead_code` fires on unread `pub` fields. Task 4
   removed all three allows once the readers existed (confirmed in Task 4's
   ledger entry and by this task's own reading of `state.rs`). Cost if
   wrong: a stale allow could mask genuinely dead code later — mitigated by
   making the removal a hard requirement of Task 4's own dispatch and
   review.

6. **Task 3 (review) — `settings.rs` crossed the phase's 800-line ceiling on
   this task's own addition (777 → 809 lines).** The finding stands: the
   ceiling is this plan's own constraint, and the file was under it before
   this task. Fix chosen: relocate the whole `#[cfg(test)] mod tests` block
   to `src-tauri/src/settings/tests.rs`, mirroring the existing `layout.rs` +
   `layout/geometry.rs` pattern — a verbatim move, no test rewritten. Cost if
   wrong: a ~560-line relocation diff mid-phase, mitigated by requiring the
   same test count to pass after the move and by no later task touching
   `settings.rs` again. Confirmed: settings.rs 265 lines, settings/tests.rs
   554 lines, both well under the ceiling.

7. **Task 4 — dropping the `mut` from the brief's `let (tx, mut rx) =
   watch::channel(...)` in `reparse_skips_an_empty_channel`.** `rx.
   has_changed()` takes `&self`, so the brief's snippet trips
   `clippy::unused_mut` and would have broken the warning-count gate.
   Another plan-snippet defect, not an implementer deviation. Cost if wrong:
   none — the assertion is unchanged.

8. **Task 4 — `state.rs` appearing in Task 4's diff although the dispatch
   named two files.** The third `#[allow(dead_code)]` from ruling 5 lives
   there, and this task's own cleanup instruction required its removal.
   Cost if wrong: none — the edit is the deletion of one attribute and its
   comment.

9. **Task 4 (review) — `claim_download`'s "a dictionary download is already
   running" is genuinely misleading when it guards the hints slot.** The
   design spec's "verbatim reuse" instruction is about the mechanism
   (one shared in-flight flag), not the exact wording shown to the user.
   Decided: keep the shared helper unchanged and map the error to a
   hints-specific message at the hints call site only (`c1543ee`). Cost if
   wrong: one line, trivially revertible; `download_dictionary`'s own
   message is untouched.

10. **Task 4 (review, parked) — the off path does not consult
    `HintsInFlight`, so an "on" acquisition completing after a user's "off"
    could republish the tokenizer and persist `true`.** Ruling: real but
    unreachable through the shipped UI — the settings window is the only
    caller, it disables the checkbox from click to a terminal `ready`/`off`/
    error event, and the off path itself is synchronous. Carried into Task
    5's review as a thing to confirm, not fixed with a backend interlock.
    Cost if wrong: a double-toggle at exactly the wrong instant could leave
    the setting on when the user asked for off; recoverable by toggling
    again. **Confirmed unreachable** by Task 5's review: the only early
    `disabled = false` on the checkbox is the invoke-rejection path, where
    no `hints-status` event can arrive to race it.

11. **Task 5 — two minors deferred, not fixed.** The status note's base
    sentence is duplicated between the static HTML template and the
    `hints_dir` callback in `src/settings.ts`. And "an off event unchecks
    the box and re-enables it" does not also assert the status line clears,
    unlike its `ready` sibling — a brief-specified omission, not a gap the
    implementer introduced.

12. **Task 6 (this task) — the six manual checks (design spec §10.4) need a
    running GUI a subagent cannot drive.** Decided: run the full automated
    gate, plus a real-asset acquisition check through `jparser-cli
    fetch-hints` into a scratch directory (proving the live download and
    attempting the hand-placed-archive replacement against the real 7.7 MB
    asset, without touching the user's app config dir), and write the docs.
    The GUI-dependent halves are recorded as unrun in §2.3 and handed to the
    user. Cost if wrong: the toggle's UI lifecycle is proven only by vitest
    and the design, not by a human watching the window — which is exactly
    what this handoff says.

---

## 7. Deviations Folded Into the Master Spec

Rows 19 and 20 (verbatim from the design spec's §13), appended to
`docs/superpowers/specs/2026-08-12-jparser-port-design.md`'s §12 table. §11's
Phase 5 table's 5B row marked shipped. See §8 below for the exact edits.

| # | Deviation | Why |
| - | --------- | --- |
| 19 | One version-blind slot, `<hints>/current/`, amending row 17's version-named directory | Row 17's directory was named for the pinned release on the reasoning that a later one would land beside it. Nothing ever reads a second slot, and 5B accepts dictionaries the pin does not describe, so a name derived from the pin would misdescribe its own contents. §3.1 |
| 20 | A hand-placed archive is unpacked without a hash check, while the download keeps `ASSET_SHA256` and `ASSET_BYTES` | The pin proves a *network response* is the artifact this design chose — against a repointed URL, a truncating proxy, a tampered mirror. A file the user placed in their own config directory is already their choice, and hashing it could only refuse what they asked for. `jmdict-source`'s hand-placed file has no hash either. §3.3 |

---

## 8. Master Spec Changes Made at This Close

`docs/superpowers/specs/2026-08-12-jparser-port-design.md`:

- **§11's 5B row**: rewritten to past tense, in the voice of the 5A row
  above it — states what shipped (`Settings::mecab_hints` and its
  settings-window control, `set_mecab_hints` acquiring on demand and
  publishing to the live worker over a second `watch`, `AppState.hints`
  deleted, an absent dictionary at startup meaning hints off plus a warning
  rather than a fatal, the hand-placed-archive route accepting an
  IPADIC-format, IPADIC-sized Vibrato dictionary, `TA_HINTS_DICT` keeping its
  fatal) and names §12 rows 19–20.
- **§12**: gained rows 19 and 20, copied verbatim from the design spec's
  §13.

No other text in that file was touched at this close.

---

## 9. What This Phase Did Not Clear

**Nothing from the 4D handoff's §4/§5 was touched.** 5B, like 5A before it,
does not run through any window/chrome/layout/shortcut code — none of 4D's
seven unrun manual checks, its unrun macOS surface, 4A's own checklist, 4B's
manual checks, the Windows title-bar test plan, `set_chrome_mode`'s persist
ordering, or the peeked-band cosmetic gap are addressed here. They remain
exactly as the 4D handoff recorded them.

**5A's own unrun item is still unrun.** 5A's handoff §9 named two things
beyond its own Task 6: no macOS run of any kind, and no verification that a
killed fetch mid-download actually leaves a `.partial` directory and no
half-written `system.dic` at the resolved name under a real interruption
(asserted only by controlled fixtures). 5B did not run on macOS either, and
did not attempt a real-network interruption test — both remain outstanding
for a later phase.

**This phase's own new unrun items:**

- **Five of the six GUI manual checks in §2.3** — the offline second toggle,
  a hand-placed-archive replacement, a missing-dictionary warning in both
  windows, an unloadable-dictionary message naming the format, and the
  env-override's fatal. Check 1 has since been run: it failed, the defect is
  fixed, and the re-run passed (§10). Checks 3 and 5 are the two that
  exercise the destructive replace-a-published-slot path and are the ones
  worth running before any release.
- The hand-placed-archive branch has **no automated or scriptable coverage
  against the real 46 MB asset** — see §2.2 and §5. The branch's logic is
  unit-tested with fixtures; the real-asset path is provable only by the
  manual check in §2.3 item 3, or by adding a `jparser-cli` command that
  calls `resolve()` — a code change this task did not make, being out of
  scope.
- 6B's Playwright coverage of the settings window, named in the design spec
  and in this phase's own §10.3, has not started; the toggle's full
  lifecycle is proven only by vitest until it does.

---

## 10. Post-Close Defect and Fix

The phase closed with §2.3's manual checks unrun. The user ran check 1 first,
and it failed: **toggling the hints checkbox changed nothing on screen.**
Copying a different sentence afterwards *did* show the new hints state, so
acquisition, persistence and the `watch` publication were all working.

**Root cause — not in this phase's code.** `src/history.ts`'s `push` dedupes
on `surfaceText(result)`, the segments joined back together, documented as
"what the user copied, independent of how it segmented". `commands::reparse`
re-parses the text already on screen, so its result carries an **identical
surface with a different segmentation** — the exact shape that dedupe key
calls a duplicate. `push` dropped it, and `src/main.ts`'s `show()` then
re-displayed `history.current()`: the stale parse. `すもも/もも/もも/もも` and
`すもも/も/もも/も/もも` both join to `すもももももももものうち`, so the
result was provably discarded rather than merely suspected.

**Fix** (`c330288`): a same-surface parse now **replaces** the newest history
entry instead of being discarded. Still one entry per copied text, and the
cursor is left alone exactly as skipping did, so a user reading an older parse
is not yanked forward. All sixteen pre-existing `history.test.ts` tests pass
unchanged, including the four that pin the dedupe's other guarantees.

**Tests added**, both verified to fail with the fix reverted:

- `src/history.test.ts` — `a re-segmentation of the newest parse replaces it`,
  the unit contract.
- `src/main.test.ts` — `a re-segmentation of the same text replaces what is on
  screen`, which drives two `parse-result` events through the real
  show/push/display path. Reverted, it fails with
  `expected [ '東京', '都' ] to deeply equal [ '東京都' ]` — the user's symptom
  exactly. This is the one that matters: the defect lived in the *interaction*,
  and a history-only test would let a future `show()` refactor reintroduce it.

**Gate after the fix:** `npx vitest run` **265 passed, 13 files** (263 before),
`npx tsc --noEmit` clean, `npx playwright test` **45 passed**. No Rust changed,
so §6's Rust numbers stand.

**Check 1 then passed on re-run**, confirmed by the user: the download, the
live publication to the running worker, and re-segmentation without a restart.
That makes it the first end-to-end GUI evidence in the phase — everything in
§2 before it was headless.

**What this says about the phase's testing.** The vitest suite covered
`reparse`'s backend half and the checkbox lifecycle, and the Rust suite covered
the channel and the worker. Nothing covered a second `parse-result` arriving
for the same text, which is the only new event shape 5B introduced to the
frontend. That gap is now closed, but it is the kind worth looking for
directly: when a phase makes an existing surface receive an input it never
received before, test that surface with that input.
