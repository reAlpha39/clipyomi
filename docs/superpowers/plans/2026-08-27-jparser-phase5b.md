# JParser Phase 5B — MeCab Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A persisted "Boundary hints (MeCab)" checkbox in the settings window acquires the Vibrato dictionary on demand — downloading it, or unpacking an archive the user dropped in the hints directory — and publishes the loaded tokenizer to the running parse worker, so the text already on screen re-segments without a restart.

**Architecture:** `vibrato-source` gains one version-blind published slot (`<hints>/current/`), a hand-placed-archive route where the newest `.tar.xz` wins, and a replacing publish. `src-tauri` grows a second `watch` channel carrying `Option<Arc<VibratoTokenizer>>` beside the existing index channel — so `AppState` stays what its module doc claims it is, built once and read-only — plus a `set_mecab_hints` command that acquires on a `spawn_blocking` thread and reports phases as `hints-status` events. Startup calls `published()`, never `resolve()`: an absent dictionary means hints off plus a warning, not a fatal launch.

**Tech Stack:** Rust (workspace `jparser` 1.85, `vibrato-source` 1.89, `clipyomi` 1.88), `tokio::sync::watch`, Tauri 2, `vibrato` 0.5, TypeScript + Vite, vitest.

**Spec:** `docs/superpowers/specs/2026-08-27-jparser-phase5b-design.md`

## Global Constraints

- **Startup never acquires.** `state::load_hints` calls `vibrato_source::published`, never `resolve`. No network and no unpack at launch, ever (spec §5).
- **`TA_HINTS_DICT` keeps its fatal.** Set and unreadable is still `StartupError::Hints` and still ends the launch. The persisted setting is the thing that degrades to a warning (spec §5).
- **The persisted setting is written only on success.** A failed acquisition persists nothing, so the next launch does not claim hints it does not have (spec §6).
- **`merge_saved` stays a whitelist.** `mecab_hints` must never be added to it, never appear in `currentSnapshot()`, and never be sent by the settings window's `save_settings` call. The backend owns it outright (spec §7.1).
- **No test may touch the real network.** Every fetch test drives a `std::net::TcpListener` bound to `127.0.0.1:0`, as `crates/vibrato-source/tests/download.rs` already does. Archive-branch tests assert the connection counter is **0**.
- **No binary fixture may be committed.** Dictionaries and archives are built in memory — `crates/vibrato-source/tests/support/mod.rs` for that crate, `jparser::hints::write_test_dictionary` for `src-tauri`.
- Every new file starts with the GPL header block used by every other file in `crates/` and `src-tauri/src/`.
- Files stay under 800 lines. Doc comments explain *why*, in the prose style of the surrounding code. Tests are Arrange-Act-Assert with names that state the behaviour.
- Conventional Commits. Commit at the end of every task.
- The hand-placed archive is **not** hash-verified; the download keeps both `ASSET_BYTES` and `ASSET_SHA256` unchanged (spec §3.3).
- Known pre-existing failure, inherited not caused: `cargo clippy --workspace --all-targets -- -D warnings` fails **on Windows** with four `clippy::upper_case_acronyms` errors (`RECT`, `STYLESTRUCT`, `WINDOWPOS`, `SUBCLASSPROC`) in `src-tauri/src/commands.rs`, behind Windows-only `#[cfg]` blocks. This phase does not fix them and must not add to them. Use `--exclude clipyomi` for the clean gate and count `clipyomi`'s warnings against the Task 0 baseline.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/vibrato-source/src/lib.rs` | Modify. `HINTS_DIR`; `DICT_DIR` becomes `current`; `ARCHIVE_SUFFIX`; `hand_placed`; `publish_replacing`; the four-step `resolve_from`; `MissingMember` gains the archive path. |
| `crates/vibrato-source/src/extract.rs` | Modify. One return site: `MissingMember` carries the archive it walked. |
| `crates/vibrato-source/tests/support/mod.rs` | Modify. A `tick()` helper so two files written in one test compare by mtime. |
| `crates/vibrato-source/tests/extract.rs` | Modify. `hand_placed` selection rules and `publish_replacing`. |
| `crates/vibrato-source/tests/download.rs` | Modify. The archive branch never connects; newest wins; no hash check on a hand-placed archive. |
| `crates/jparser/src/hints.rs` | Modify. `write_test_dictionary`, so other crates can exercise the real `VibratoTokenizer::load` without a 46 MB file. |
| `src-tauri/Cargo.toml` | Modify. `vibrato-source` as a direct dependency, the way `jmdict-source` already is. |
| `src-tauri/src/settings.rs` | Modify. `Settings::mecab_hints`. |
| `src-tauri/src/state.rs` | Modify. `HintsWarning`; `load_hints`; `AppState.hints` and `load_state`'s env branch removed. |
| `src-tauri/src/parse.rs` | Modify. `current_hints`; `run_worker`'s third parameter; the parse closure reads flags off the channel. |
| `src-tauri/src/commands.rs` | Modify. `HintsSender`, `HintsInFlight`, `HINTS_STATUS_EVENT`, `set_mecab_hints`, `reparse`, `hints_warning`, `hints_dir`; `download_dictionary`'s hints branch deleted; `DictionaryPaths` gains `hints_dir`. |
| `src-tauri/src/main.rs` | Modify. The hints channel, `load_hints` ahead of `load_state`, the four new commands registered. |
| `src/types.ts` | Modify. `mecab_hints?: boolean`. |
| `src/settings.ts` | Modify. The `Segmentation` group and its lifecycle. |
| `src/settings.test.ts` | Modify. The checkbox lifecycle across all four outcomes. |
| `src/main.ts` | Modify. `showHintsWarning`. |
| `src/main.test.ts` | Modify. Two mock fallthroughs, plus a test for the new warning. |
| `docs/superpowers/specs/2026-08-12-jparser-port-design.md` | Modify in Task 6. §11's 5B row, §12 rows 19–20. |
| `docs/superpowers/2026-08-27-jparser-phase5b-handoff.md` | **Create** in Task 6. |

---

## Task 0: Capture the baseline

**Files:** none. This task produces numbers, not a diff.

**Interfaces:**
- Consumes: nothing.
- Produces: the baseline counts Task 6's gate compares against.

- [ ] **Step 1: Record the Rust and frontend test counts**

```bash
cargo test --workspace 2>&1 | grep -E "^test result:"
npx vitest run 2>&1 | tail -5
npx playwright test 2>&1 | tail -3
```

Write all three down. 5A's close recorded 460 passed / 0 failed / 1 ignored for `cargo test --workspace`, 252 vitest, 45 Playwright — confirm rather than assume; the tree may have moved.

- [ ] **Step 2: Record the clippy warning count**

```bash
cargo clippy --all-targets 2>&1 | grep -c "^warning"
cargo clippy --workspace --all-targets --exclude clipyomi -- -D warnings
```

5A recorded 6 for the first and a clean run for the second. The second **must** be clean before this phase starts; if it is not, stop and report — this plan's gate cannot otherwise distinguish inherited noise from its own.

- [ ] **Step 3: Note whether a 5A hints directory already exists**

```bash
ls -la "$APPDATA/com.clipyomi.app/hints" 2>/dev/null || echo "no hints dir yet"
```

The exact directory is whatever `app.path().app_config_dir()` resolves to on this machine. Task 1 renames the published slot from `ipadic-mecab-2_7_0/` to `current/`, so a directory left by a 5A `jparser-cli fetch-hints` run is simply ignored from then on — harmless, but it will confuse Task 6's "fresh config dir" check if nobody wrote it down.

No commit: this task changes nothing.

---

## Task 1: `vibrato-source` — one slot, and the archive route

**Files:**
- Modify: `crates/vibrato-source/src/lib.rs`
- Modify: `crates/vibrato-source/src/extract.rs` (the `if !found` return in `unpack_bounded`)
- Test: `crates/vibrato-source/tests/support/mod.rs`, `crates/vibrato-source/tests/extract.rs`, `crates/vibrato-source/tests/download.rs`

**Interfaces:**
- Consumes: `published`, `publish`, `staging_names`, `extract::unpack_into`, `fetch::fetch_with_retry` — all as 5A shipped them.
- Produces:
  - `pub const HINTS_DIR: &str = "hints"`
  - `pub const DICT_DIR: &str = "current"` (value change; name unchanged)
  - `pub const ARCHIVE_SUFFIX: &str = ".tar.xz"`
  - `pub fn hand_placed(hints_dir: &Path) -> Option<PathBuf>`
  - `pub fn publish_replacing(staging: &Path, target: &Path) -> Result<(), SourceError>`
  - `pub fn resolve(hints_dir: &Path) -> Result<PathBuf, SourceError>` — signature unchanged, four-step body
  - `SourceError::MissingMember { name: String, archive: PathBuf }` — gains a field

- [ ] **Step 1: Point the slot at `current` and name the hints directory**

In `crates/vibrato-source/src/lib.rs`, replace the `DICT_DIR` constant and its doc comment with these three:

```rust
/// The caller's hints directory, inside whatever config directory the
/// application owns. Named here for the same reason `jmdict_source::SOURCE_DIR`
/// is: the crate names its own conventional subdirectory without deriving an
/// app layout it does not own.
pub const HINTS_DIR: &str = "hints";

/// The published directory's name, inside the caller's hints directory.
///
/// Version-blind on purpose. 5A named it for the pinned release on the
/// reasoning that a later release would land beside it; nothing ever reads a
/// second slot, and this crate now accepts dictionaries the pin does not
/// describe — a hand-placed archive is not hash-checked — so a name derived
/// from the pin would be a lie about the contents.
pub const DICT_DIR: &str = "current";

/// The suffix a hand-placed archive must carry to be [`hand_placed`].
///
/// Staging archives are excluded twice over: they begin with a dot, and they
/// end in `.partial.<pid>-<n>` rather than this.
pub const ARCHIVE_SUFFIX: &str = ".tar.xz";
```

- [ ] **Step 2: Run the crate's tests — the rename must be inert**

```bash
cargo test -p vibrato-source
```

Expected: PASS, unchanged counts (5 unit + 8 extract + 9 download). Every test names the slot through `DICT_DIR`, so changing its value moves the directory without moving any behaviour. A failure here means a test hardcoded `ipadic-mecab-2_7_0` — fix that test, not the constant.

- [ ] **Step 3: Add the mtime helper and write the failing `hand_placed` tests**

In `crates/vibrato-source/tests/support/mod.rs`, beside `write`:

```rust
/// Sleep long enough that the next file written compares strictly newer.
///
/// The archive-versus-slot rule is an mtime comparison, and two files written
/// in the same tick can compare equal. Crude, but the alternative is a
/// `filetime` dependency for a test-only concern.
pub fn tick() {
    std::thread::sleep(std::time::Duration::from_millis(50));
}
```

Append to `crates/vibrato-source/tests/extract.rs`:

```rust
/// Two archives is not an error: the newest wins, which is the same rule
/// `resolve` applies against the published slot. One sentence to explain, and
/// no new error variant.
#[test]
fn the_newest_hand_placed_archive_wins() {
    let dir = scratch("handplaced-newest");
    write(&dir, "old.tar.xz", &real_shaped_archive());
    tick();
    write(&dir, "new.tar.xz", &real_shaped_archive());

    assert_eq!(
        vibrato_source::hand_placed(&dir),
        Some(dir.join("new.tar.xz"))
    );
}

/// A staging archive is a download in progress or a dead one's leftover, and
/// neither is a file the user placed. It is excluded twice over — by the
/// leading dot and by not ending in the suffix — and this pins both halves.
#[test]
fn dotfiles_and_staging_archives_are_not_hand_placed() {
    let dir = scratch("handplaced-staging");
    let (staging_archive, _) = vibrato_source::staging_names(&dir);
    write(&dir, ".hidden.tar.xz", b"not a candidate");
    std::fs::write(&staging_archive, b"mid download").expect("write");
    write(&dir, "notes.txt", b"not an archive");

    assert_eq!(vibrato_source::hand_placed(&dir), None);
}

/// First run: the hints directory does not exist yet. A `read_dir` failure
/// there is "no archive", not an error to report.
#[test]
fn an_absent_hints_directory_has_no_hand_placed_archive() {
    let dir = scratch("handplaced-absent");
    assert_eq!(vibrato_source::hand_placed(&dir.join("never-created")), None);
}
```

- [ ] **Step 4: Run them to verify they fail**

```bash
cargo test -p vibrato-source --test extract
```

Expected: FAIL to compile — ``cannot find function `hand_placed` in crate `vibrato_source` ``.

- [ ] **Step 5: Implement `hand_placed`**

In `crates/vibrato-source/src/lib.rs`, directly after `published`:

```rust
/// The newest hand-placed archive directly in `hints_dir`, or `None`.
///
/// A file qualifies when its name ends in [`ARCHIVE_SUFFIX`] and does not begin
/// with a dot. Subdirectories are not descended into: this is a drop box, not a
/// search path.
///
/// A `read_dir` that fails — on a directory that does not exist yet — is `None`
/// rather than an error. First run is not a failure.
///
/// Public rather than private to [`resolve`] because the caller picks the phase
/// label it shows the user *before* calling `resolve`: `resolve` returns a
/// path, not which branch produced it, and by the time it returns the label is
/// no longer useful. Being wrong costs a mislabelled phase, not a wrong
/// outcome, which is not worth a second return channel.
pub fn hand_placed(hints_dir: &Path) -> Option<PathBuf> {
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;

    for entry in std::fs::read_dir(hints_dir).ok()?.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || !name.ends_with(ARCHIVE_SUFFIX) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(mtime) = metadata.modified() else {
            continue;
        };
        let better = match &newest {
            Some((best, _)) => mtime > *best,
            None => true,
        };
        if better {
            newest = Some((mtime, entry.path()));
        }
    }

    newest.map(|(_, path)| path)
}
```

- [ ] **Step 6: Run the tests to verify they pass**

```bash
cargo test -p vibrato-source --test extract
```

Expected: PASS, 11 tests (8 existing + 3 new).

- [ ] **Step 7: Write the failing tests for `publish_replacing`**

Append to `crates/vibrato-source/tests/extract.rs`:

```rust
/// The whole point of the second entry point: `publish` treats an occupied
/// target as a benign race and throws the new dictionary away, which is right
/// for two processes fetching the same pinned artifact and wrong for a
/// deliberate replacement.
#[test]
fn publish_replacing_overwrites_an_existing_dictionary() {
    let dir = scratch("replace-happy");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the old one").expect("write");

    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the new one").expect("write");

    vibrato_source::publish_replacing(&staging, &target).expect("replace");

    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("read"),
        b"the new one"
    );
    assert!(!staging.exists(), "the staging directory must not survive");
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".partial"))
        .collect();
    assert!(leftovers.is_empty(), "backup left behind: {leftovers:?}");
}

/// The failure the backup exists for: the second rename does not happen, and
/// the user must not be left with no dictionary at all. A staging directory
/// that is not there is the portable way to make that rename fail.
#[test]
fn a_failed_replacement_puts_the_old_dictionary_back() {
    let dir = scratch("replace-restore");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the old one").expect("write");

    let (_, staging) = vibrato_source::staging_names(&dir);

    let err = vibrato_source::publish_replacing(&staging, &target).expect_err("must fail");
    assert!(matches!(err, SourceError::Publish { .. }), "got {err:?}");
    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("the old dictionary must survive"),
        b"the old one"
    );
}

/// No dictionary to replace is not a special case for the caller to handle.
#[test]
fn publish_replacing_onto_nothing_is_a_plain_publish() {
    let dir = scratch("replace-empty");
    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the only one").expect("write");

    vibrato_source::publish_replacing(&staging, &dir.join(DICT_DIR)).expect("publish");

    assert!(vibrato_source::published(&dir).is_some());
}
```

- [ ] **Step 8: Run them to verify they fail**

```bash
cargo test -p vibrato-source --test extract
```

Expected: FAIL to compile — ``cannot find function `publish_replacing` ``.

- [ ] **Step 9: Implement `publish_replacing`**

In `crates/vibrato-source/src/lib.rs`, directly after `publish`:

```rust
/// Appended to a staging path to name this call's backup. Begins with a dot
/// because the staging name does, so a backup is neither [`published`] nor
/// [`hand_placed`].
const BACKUP_SUFFIX: &str = ".backup";

/// Publish `staging` onto `target`, replacing a dictionary already there.
///
/// [`publish`] treats an occupied target as a benign race and discards the
/// staging directory. That is right for two processes fetching the same pinned
/// artifact and wrong for a deliberate replacement — dropping a newer archive
/// in and toggling is how a user replaces their dictionary, and it must not
/// silently keep the old one.
///
/// The existing target is renamed aside first and put back if the second rename
/// fails, so there is no window in which the user has no dictionary. The backup
/// name is derived from `staging`, which already carries this process's PID and
/// [`STAGING_SEQ`], so it is unique to this call for the same reason the
/// staging directory is. All three paths live inside the same hints directory,
/// so no rename crosses a filesystem.
pub fn publish_replacing(staging: &Path, target: &Path) -> Result<(), SourceError> {
    if !target.exists() {
        return publish(staging, target);
    }

    let mut backup = staging.as_os_str().to_owned();
    backup.push(BACKUP_SUFFIX);
    let backup = PathBuf::from(backup);

    let failed = |cause: String| SourceError::Publish {
        staging: staging.to_path_buf(),
        target: target.to_path_buf(),
        cause,
    };

    std::fs::rename(target, &backup).map_err(|e| failed(e.to_string()))?;

    match std::fs::rename(staging, target) {
        Ok(()) => {
            // Best-effort: a leftover backup is inert — never resolved, never
            // hand-placed — so failing here would report a failure that did not
            // happen.
            let _ = std::fs::remove_dir_all(&backup);
            Ok(())
        }
        Err(e) => {
            // The dictionary the user already had is the one thing that must
            // survive this.
            let _ = std::fs::rename(&backup, target);
            Err(failed(e.to_string()))
        }
    }
}
```

- [ ] **Step 10: Run the tests to verify they pass**

```bash
cargo test -p vibrato-source --test extract
```

Expected: PASS, 14 tests.

- [ ] **Step 11: Make `MissingMember` name its archive**

Once more than one archive can be present, "the archive carried no system.dic.zst" no longer says which one. In `crates/vibrato-source/src/lib.rs`, replace that variant:

```rust
    #[error("the archive at {archive} carried no {name}", archive = archive.display())]
    MissingMember { name: String, archive: PathBuf },
```

In `crates/vibrato-source/src/extract.rs`, the `if !found` return at the end of `unpack_bounded`:

```rust
    if !found {
        return Err(SourceError::MissingMember {
            name: SYSTEM_DIC_ZST.to_string(),
            archive: archive.to_path_buf(),
        });
    }
```

And update the existing test in `crates/vibrato-source/tests/extract.rs`:

```rust
#[test]
fn an_archive_without_the_dictionary_names_the_missing_member() {
    let dir = scratch("unpack-nomember");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[("ipadic-mecab-2_7_0/COPYING", b"licence".to_vec())]),
    );

    let err = extract::unpack_into(&archive, &dir.join("staging")).expect_err("must fail");
    match err {
        SourceError::MissingMember { name, archive: named } => {
            assert_eq!(name, "system.dic.zst");
            assert_eq!(named, archive, "the message must say which archive");
        }
        other => panic!("got {other:?}"),
    }
}
```

- [ ] **Step 12: Run the tests to verify they pass**

```bash
cargo test -p vibrato-source
```

Expected: PASS. A compile error anywhere else would mean a second construction site of `MissingMember`; there is exactly one.

- [ ] **Step 13: Write the failing tests for the four-step `resolve`**

Append to `crates/vibrato-source/tests/download.rs`:

```rust
/// The archive branch's central proof: an archive newer than the slot is
/// unpacked and published, and no socket is opened doing it.
#[test]
fn a_newer_hand_placed_archive_wins_without_touching_the_network() {
    let dir = scratch("resolve-archive-newer");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"the old one").expect("write");
    tick();
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let path = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect("resolve");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file))
        .expect("the archive's dictionary must have replaced the old file");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        0,
        "the archive branch must not reach the network"
    );
}

/// The other direction, and the reason the rule is an mtime comparison rather
/// than "an archive present wins": the archive a user installed last month must
/// not re-unpack on every toggle.
#[test]
fn an_older_hand_placed_archive_loses_to_the_published_slot() {
    let dir = scratch("resolve-archive-older");
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());
    tick();
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let path = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect("resolve");

    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"already here",
        "an older archive must not replace the published slot"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}

/// A file the user placed in their own config directory is already their
/// choice; hashing it could only refuse what they explicitly asked for. The
/// hash passed here matches nothing, and the archive is published anyway.
#[test]
fn a_hand_placed_archive_is_published_without_a_hash_check() {
    let dir = scratch("resolve-archive-nohash");
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let path = resolve_from(
        &url,
        "0000000000000000000000000000000000000000000000000000000000000000",
        &dir,
        Duration::from_millis(1),
    )
    .expect("an unhashed hand-placed archive must publish");

    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file)).expect("vibrato must read it");
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}
```

Then give 5A's zero-connection test its archive-branch assertion. In `resolve_returns_a_published_dictionary_without_downloading`, the arrange block becomes — an archive present but *older* than the slot, so the hand-placed check runs on the way to the short-circuit and changes nothing:

```rust
    let dir = scratch("resolve-published");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    // An archive older than the slot. `resolve` looks for one on every call, so
    // this proves that lookup neither reaches the network nor clobbers a newer
    // slot on its way to the published short-circuit below.
    write(&dir, "stale.tar.xz", &real_shaped_archive());
    tick();
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");
```

The rest of that test — the server, the `resolve_from` call, the two assertions, the trailing production-`resolve` line — is unchanged. Add an assertion after the existing ones:

```rust
    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"already here",
        "a stale archive must not have replaced the slot"
    );
```

- [ ] **Step 14: Run them to verify they fail**

```bash
cargo test -p vibrato-source --test download
```

Expected: FAIL. `a_newer_hand_placed_archive_wins_without_touching_the_network` fails its `vibrato::Dictionary::read` assertion (today's `resolve` returns the stale slot untouched), and `a_hand_placed_archive_is_published_without_a_hash_check` fails with `TooManyAttempts`.

- [ ] **Step 15: Implement the four-step `resolve_from`**

In `crates/vibrato-source/src/lib.rs`, add the mtime comparison above `resolve_from` and replace that function's body and doc:

```rust
/// Whether `archive` is newer than the published dictionary at `dic`.
///
/// A missing dictionary is older than anything: an archive with no slot beside
/// it wins. An unreadable mtime on either side is "not newer" — keeping a
/// dictionary that works beats replacing it on a failed `stat`.
fn archive_is_newer(archive: &Path, dic: &Path) -> bool {
    let Ok(archive_mtime) = std::fs::metadata(archive).and_then(|m| m.modified()) else {
        return false;
    };
    match std::fs::metadata(dic).and_then(|m| m.modified()) {
        Ok(dic_mtime) => archive_mtime > dic_mtime,
        Err(_) if !dic.exists() => true,
        Err(_) => false,
    }
}

/// [`resolve`] with the URL, hash and backoff injected, so a test can prove
/// both short-circuits — the published slot and the hand-placed archive — by
/// pointing the fall-through at a local listener and asserting it was never
/// connected to. The same shape as `jmdict_source::resolve_from`.
///
/// **Not part of the supported surface.** Production callers use [`resolve`].
#[doc(hidden)]
pub fn resolve_from(
    url: &str,
    sha256: &str,
    hints_dir: &Path,
    backoff: std::time::Duration,
) -> Result<PathBuf, SourceError> {
    let target = hints_dir.join(DICT_DIR);
    let dic = target.join(SYSTEM_DIC);

    // Steps 1 and 2: a hand-placed archive newer than the slot — or with no
    // slot beside it at all — is what the user just asked for. Deliberately not
    // hashed; see this module's own docs and design §3.3. This is also how a
    // dictionary is *replaced*: drop the new archive in, toggle off and on.
    if let Some(archive) = hand_placed(hints_dir) {
        if archive_is_newer(&archive, &dic) {
            let (_, staging) = staging_names(hints_dir);
            let result = extract::unpack_into(&archive, &staging)
                .and_then(|()| publish_replacing(&staging, &target));
            if result.is_err() {
                // Inert either way, but a failed unpack can leave tens of
                // megabytes behind and nothing else will ever remove it.
                let _ = std::fs::remove_dir_all(&staging);
            }
            result?;
            return Ok(dic);
        }
    }

    // Steps 3 and 4.
    match published(hints_dir) {
        Some(path) => Ok(path),
        None => fetch::fetch_with_retry(url, sha256, hints_dir, backoff),
    }
}
```

- [ ] **Step 16: Run the whole crate's tests and clippy**

```bash
cargo test -p vibrato-source
cargo clippy -p vibrato-source --all-targets -- -D warnings
```

Expected: PASS — 5 unit + 14 extract + 12 download; take the arithmetic from the real output, not from this line. Clippy clean.

- [ ] **Step 17: Update the crate's module doc for the second source**

`lib.rs`'s header still describes one immutable pinned asset. Add after that paragraph:

```rust
//! Since 5B there is a second source: a `.tar.xz` the user drops in the hints
//! directory themselves, which is unpacked without a hash check because a file
//! they placed is already their choice — the same asymmetry `jmdict-source` has
//! for its own hand-placed file. The extractor's guards do not depend on the
//! pin, so this route inherits no hole: see [`extract::unpack_into`].
```

- [ ] **Step 18: Commit**

```bash
git add crates/vibrato-source
git commit -m "feat(vibrato-source): one version-blind slot and a hand-placed archive route"
```

---

## Task 2: `jparser` — a real dictionary other crates' tests can write

**Files:**
- Modify: `crates/jparser/src/hints.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `#[doc(hidden)] pub fn jparser::hints::write_test_dictionary(path: &Path) -> Result<(), HintsError>`, available whenever the `mecab` feature is on — which `src-tauri` already enables.

- [ ] **Step 1: Write the failing test**

Append to `crates/jparser/src/hints.rs`'s `mod tests`:

```rust
/// Task 3's `load_hints` tests need a file `VibratoTokenizer::load` accepts, in
/// a crate that cannot reach this module's private fixture. The proof that the
/// helper is worth anything is that its output loads and tokenizes.
#[test]
fn the_written_test_dictionary_loads_and_tokenizes() {
    let dir = scratch("write-test-dictionary");
    let path = dir.join("system.dic");

    write_test_dictionary(&path).expect("write");

    let tokenizer = VibratoTokenizer::load(&path).expect("load what we just wrote");
    let flags = tokenizer.hints("東京都");
    assert!(flags.bad_end(0) && flags.bad_start(1), "東京's interior");
}
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cargo test -p jparser --features mecab the_written_test_dictionary
```

Expected: FAIL to compile — ``cannot find function `write_test_dictionary` ``.

- [ ] **Step 3: Lift the fixture out of the test module**

Move `test_dictionary()` and its four `const` inputs from `mod tests` up to module scope in `crates/jparser/src/hints.rs`, directly below `HintsError`. Only its visibility and doc change:

```rust
/// A compiled two-entry IPADIC-shaped dictionary, built in memory.
///
/// Deliberately not a fixture file: the real dictionary is 46 MB, and nothing
/// in this crate's tests may touch the network or ship a binary. These four
/// readers are the same inputs vibrato's own `compile` binary takes.
fn test_dictionary() -> vibrato::Dictionary {
    const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                       都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
    const MATRIX: &str = "1 1\n0 0 0\n";
    const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
    const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                       KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

    vibrato::SystemDictionaryBuilder::from_readers(
        LEX.as_bytes(),
        MATRIX.as_bytes(),
        CHAR.as_bytes(),
        UNK.as_bytes(),
    )
    .expect("the built-in test dictionary must build")
}
```

Delete the `pub(crate) fn test_dictionary` that was inside `mod tests`. Its two call sites there (`the_test_dictionary_tokenizes_a_known_sentence`, `flags_for`) resolve through the module's existing `use super::*` unchanged.

- [ ] **Step 4: Write `write_test_dictionary` beside it**

```rust
/// Write [`test_dictionary`] to `path`, for tests in crates that cannot reach
/// this module's private fixture.
///
/// `src-tauri` has to exercise the real [`VibratoTokenizer::load`] — its
/// startup resolution and its `watch` publication both carry a real tokenizer,
/// and a stub would prove neither — and the alternatives are a 46 MB file in
/// the repository or a network call inside the suite.
///
/// Serialized to memory first rather than through a `BufWriter`: a buffered
/// writer flushed only by its `Drop` swallows the error that matters here.
///
/// **Not part of the supported surface.** Nothing in a shipping build calls it;
/// it is `pub` because a `#[cfg(test)]` item is invisible to another crate.
#[doc(hidden)]
pub fn write_test_dictionary(path: &Path) -> Result<(), HintsError> {
    let mut bytes = Vec::new();
    test_dictionary()
        .write(&mut bytes)
        .map_err(|e| HintsError::Dictionary(e.to_string()))?;
    std::fs::write(path, &bytes).map_err(|source| HintsError::Io {
        path: path.to_path_buf(),
        source,
    })
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cargo test -p jparser --features mecab
cargo clippy -p jparser --features mecab --all-targets -- -D warnings
```

Expected: PASS, one test more than the baseline for this feature set. Clippy clean.

- [ ] **Step 6: Confirm the purity gate still holds**

```bash
cargo check -p jparser --no-default-features --all-targets
```

Expected: clean. `write_test_dictionary` lives inside `#[cfg(feature = "mecab")] pub mod hints`, so a no-features build never sees it.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/src/hints.rs
git commit -m "feat(jparser): expose write_test_dictionary for downstream tests"
```

---

## Task 3: `src-tauri` state — the setting, the second channel, startup

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/settings.rs` (the `Settings` struct, its `Default`, its tests)
- Modify: `src-tauri/src/state.rs` (module doc, `AppState`, `load_state`, new `HintsWarning` and `load_hints`, tests)
- Modify: `src-tauri/src/parse.rs` (`current_hints`, `run_worker`, `catch_parse`'s doc, tests)
- Modify: `src-tauri/src/commands.rs` (`HintsSender`, `DictionaryPaths.hints_dir`, `download_dictionary`'s hints branch deleted)
- Modify: `src-tauri/src/main.rs` (the hints channel, `load_hints` ahead of `load_state`)

**Interfaces:**
- Consumes: `vibrato_source::{HINTS_DIR, published}`; `jparser::hints::{VibratoTokenizer, write_test_dictionary}` from Task 2.
- Produces:
  - `settings::Settings { .., pub mecab_hints: bool }`
  - `state::HintsWarning(pub String)`
  - `state::load_hints(hints_dir: &Path, enabled: bool) -> Result<(Option<VibratoTokenizer>, String), StartupError>`
  - `state::AppState { pub index: Index, pub table: ConjugationTable }` — `hints` removed
  - `parse::current_hints(&watch::Receiver<Option<Arc<VibratoTokenizer>>>) -> Option<Arc<VibratoTokenizer>>`
  - `parse::run_worker(app, index, hints, rx)` — three channels
  - `commands::HintsSender(pub watch::Sender<Option<Arc<VibratoTokenizer>>>)`
  - `commands::DictionaryPaths { root, source_dir, hints_dir }`

- [ ] **Step 1: Add `vibrato-source` to the shell**

In `src-tauri/Cargo.toml`, below the `jmdict-source` line:

```toml
# Added directly, the way `jmdict-source` is, rather than through `jparser`'s
# `hints-source` feature: that feature is the CLI driver's gate, and routing the
# shell's acquisition through it would put a TLS stack behind a library feature
# flag instead of in the binary that actually downloads. `ci.yml`'s purity gate
# still proves `jparser --no-default-features` pulls in neither source crate.
vibrato-source = { path = "../crates/vibrato-source" }
```

- [ ] **Step 2: Verify it builds and the TLS gate still passes**

```bash
cargo check -p clipyomi
cargo tree --workspace | grep -E "openssl v|openssl-sys|native-tls|\bring v|aws-lc" || echo "TLS gate clean"
```

Expected: clean build, and `TLS gate clean`. `src-tauri` now reaches `rustls` + `rustls-graviola` through `vibrato-source`, which is the sanctioned provider the gate's own comment names.

- [ ] **Step 3: Write the failing test for the persisted setting**

Append to `src-tauri/src/settings.rs`'s `mod tests`:

```rust
/// Owned by the backend, not the settings window: `merge_saved` is a whitelist
/// that does not carry this key, so the only writer is `set_mecab_hints`. Off
/// by default — a first launch must not download 8 MB nobody asked for.
#[test]
fn mecab_hints_defaults_off_and_survives_a_round_trip() {
    let dir = scratch("mecab-hints-roundtrip");
    let path = dir.join(SETTINGS_FILE);
    assert!(!Settings::default().mecab_hints);

    let mut settings = Settings::default();
    settings.mecab_hints = true;
    save(&path, &settings).expect("save");

    let (reloaded, warning) = load(&path);
    assert!(reloaded.mecab_hints);
    assert!(warning.is_none(), "got {warning:?}");
}
```

Use whatever scratch-directory helper that test module already has; if the name differs from `scratch`, match it rather than adding a second one.

- [ ] **Step 4: Run it to verify it fails**

```bash
cargo test -p clipyomi mecab_hints_defaults_off
```

Expected: FAIL to compile — `no field 'mecab_hints' on type 'Settings'`.

- [ ] **Step 5: Add the field**

In `src-tauri/src/settings.rs`, in `Settings`, directly below `clipboard_monitoring`:

```rust
    /// Whether MeCab boundary hints are switched on (§7.1).
    ///
    /// Owned by the backend outright: `commands::merge_saved` is a whitelist
    /// and does not carry this key, the settings window never sends it, and
    /// `set_mecab_hints` is the only writer. The alternative — a snapshot
    /// written by an unrelated font-box change clobbering an in-flight toggle —
    /// is silent when it happens.
    #[serde(default)]
    pub mecab_hints: bool,
```

And in `impl Default for Settings`, below `clipboard_monitoring: true,`:

```rust
            mecab_hints: false,
```

- [ ] **Step 6: Run it to verify it passes**

```bash
cargo test -p clipyomi settings::
```

Expected: PASS.

- [ ] **Step 7: Write the failing tests for `load_hints`**

Append to `src-tauri/src/state.rs`'s `mod tests`. They reuse the `ENV_LOCK` that module already declares:

```rust
/// Write a real, loadable dictionary into `hints_dir`'s published slot.
fn publish_fixture_dictionary(hints_dir: &std::path::Path) {
    let dir = hints_dir.join(vibrato_source::DICT_DIR);
    std::fs::create_dir_all(&dir).expect("slot");
    jparser::hints::write_test_dictionary(&dir.join(vibrato_source::SYSTEM_DIC))
        .expect("fixture dictionary");
}

/// The setting off is the ordinary state, and it must cost nothing: no
/// tokenizer, no warning, no look at the disk.
#[test]
fn hints_off_loads_nothing_and_warns_about_nothing() {
    let dir = scratch("hints-off");
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let (tokenizer, warning) = load_hints(&dir, false).expect("must not fail");

    assert!(tokenizer.is_none());
    assert!(warning.is_empty(), "got {warning:?}");
}

/// The happy path: the setting is on, a dictionary is published, and it loads.
#[test]
fn hints_on_loads_the_published_dictionary() {
    let dir = scratch("hints-published");
    publish_fixture_dictionary(&dir);
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let (tokenizer, warning) = load_hints(&dir, true).expect("must not fail");

    assert!(tokenizer.is_some(), "a published dictionary must load");
    assert!(warning.is_empty(), "got {warning:?}");
}

/// Master spec §11's 5B rule: an absent dictionary means hints off, not a
/// fatal. The setting stays on, so the mismatch has to be named somewhere the
/// user can see it.
#[test]
fn hints_on_without_a_dictionary_is_a_warning_not_a_failure() {
    let dir = scratch("hints-absent");
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

    assert!(tokenizer.is_none());
    assert!(!warning.is_empty(), "the mismatch must be named");
    assert!(
        warning.contains("Settings"),
        "the warning must name the gesture that fixes it, got {warning}"
    );
}

/// `published` only asks whether the file is there. A truncated or corrupt one
/// answers yes and then fails to load, and that is the same user-visible state
/// as an absent one.
#[test]
fn a_published_dictionary_that_will_not_load_is_a_warning_too() {
    let dir = scratch("hints-corrupt");
    let slot = dir.join(vibrato_source::DICT_DIR);
    std::fs::create_dir_all(&slot).expect("slot");
    std::fs::write(slot.join(vibrato_source::SYSTEM_DIC), b"not a dictionary").expect("write");
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

    assert!(tokenizer.is_none());
    assert!(!warning.is_empty(), "got an empty warning");
}

/// Startup must never acquire: no network, and no unpack of a hand-placed
/// archive either. An archive dropped in while the app was closed is installed
/// by the toggle, not by the next launch.
#[test]
fn startup_never_installs_a_hand_placed_archive() {
    let dir = scratch("hints-archive-ignored");
    std::fs::create_dir_all(&dir).expect("hints dir");
    std::fs::write(dir.join("ipadic.tar.xz"), b"not unpacked at startup").expect("write");
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let (tokenizer, warning) = load_hints(&dir, true).expect("must not be fatal");

    assert!(tokenizer.is_none(), "startup must not unpack");
    assert!(!warning.is_empty());
    assert!(
        !dir.join(vibrato_source::DICT_DIR).exists(),
        "nothing may be published by a launch"
    );
}
```

Then **move** `a_missing_hints_dictionary_is_reported_as_startup_error_hints` off `load_state` and onto `load_hints`, keeping its `ENV_LOCK` guard and both `unsafe` blocks, and adding the readable-env case beside it:

```rust
/// `TA_HINTS_DICT` keeps 2C's fatal because it keeps 2C's meaning: an explicit
/// request, made outside the app, with no UI to recover in. Unlike the setting,
/// there is no control to show and no warning surface to explain itself.
///
/// No index is built here any more: `load_hints` is independent of
/// `load_state`, which is the whole point of it being a separate function.
#[test]
fn a_missing_hints_dictionary_is_reported_as_startup_error_hints() {
    let dir = scratch("hints-env-missing");
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: `ENV_LOCK` above is the only writer of `TA_HINTS_DICT` in this
    // binary, held for the full set/read/clear cycle, so no other thread can
    // observe this variable mid-write.
    unsafe { std::env::set_var(HINTS_ENV, "/nonexistent/ta-hints-dict.dic") };
    let result = load_hints(&dir, false);
    unsafe { std::env::remove_var(HINTS_ENV) };

    match result {
        Err(StartupError::Hints(_)) => {}
        Ok(_) => panic!("expected StartupError::Hints, got Ok(_)"),
        Err(other) => panic!("expected StartupError::Hints, got a different error: {other}"),
    }
}

/// The env var outranks the setting in both directions: it loads even with the
/// setting off, which is what makes it an override rather than a second toggle.
#[test]
fn the_env_override_loads_even_with_the_setting_off() {
    let dir = scratch("hints-env-present");
    let path = dir.join("system.dic");
    jparser::hints::write_test_dictionary(&path).expect("fixture dictionary");

    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: as above.
    unsafe { std::env::set_var(HINTS_ENV, &path) };
    let result = load_hints(&dir, false);
    unsafe { std::env::remove_var(HINTS_ENV) };

    let (tokenizer, warning) = result.expect("a readable override must load");
    assert!(tokenizer.is_some());
    assert!(warning.is_empty(), "got {warning:?}");
}
```

Every `load_hints` test takes `ENV_LOCK`, not just the two that write it: an unguarded reader can observe another test's half-written variable, and this module's own comment on that lock already says to take it rather than adding a second one.

- [ ] **Step 8: Run them to verify they fail**

```bash
cargo test -p clipyomi state::
```

Expected: FAIL to compile — ``cannot find function `load_hints` in this scope``.

- [ ] **Step 9: Implement `HintsWarning` and `load_hints`**

In `src-tauri/src/state.rs`, after `ShortcutWarning`:

```rust
/// Why hints are off while the setting says they are on, or empty when they
/// agree.
///
/// Its own managed string on `ShortcutWarning`'s stated precedent: one per
/// warning kind, one command each, because the messages are independent and any
/// of them can be present without the others.
pub struct HintsWarning(pub String);
```

After `load_state`:

```rust
/// The warning shown when the setting asks for hints and there are none.
///
/// It exists because §5's rule creates a visible mismatch: the setting stays on
/// while hints are off, so the checkbox reads on and the parse has no hints.
/// The sentence names that state and the gesture that fixes it.
fn hints_off_warning(detail: &str) -> String {
    format!(
        "MeCab boundary hints are switched on but not loaded ({detail}). \
         Open Settings and toggle Boundary hints off and on to install them."
    )
}

/// The tokenizer this launch should parse with, and the warning to show if the
/// setting asks for one that is not there.
///
/// Deliberately **not** part of [`load_state`]. That function returns
/// `Err(NoIndex)` on first run and never reaches its own tail, so hints folded
/// into it would go unloaded on exactly the run that downloads JMdict, and stay
/// unloaded until a restart.
///
/// [`HINTS_ENV`] keeps 2C's fatal because it keeps 2C's meaning: an explicit
/// request, made outside the app, with no UI to recover in. That reasoning does
/// not describe a persisted setting, which has a control the user can see and a
/// warning surface to explain itself — so an absent dictionary there is hints
/// off, not a failed launch.
///
/// **Never acquires.** It calls `published`, never `resolve`: no network, and no
/// unpack of a hand-placed archive either. An unpack is a second of work with no
/// gesture behind it and its own ways to fail. The consequence is worth stating
/// — an archive dropped in while the app was closed is not installed by the next
/// launch. The toggle installs it, and the settings note says so.
pub fn load_hints(
    hints_dir: &Path,
    enabled: bool,
) -> Result<(Option<VibratoTokenizer>, String), StartupError> {
    if let Some(path) = std::env::var_os(HINTS_ENV) {
        return Ok((
            Some(VibratoTokenizer::load(Path::new(&path))?),
            String::new(),
        ));
    }

    if !enabled {
        return Ok((None, String::new()));
    }

    let Some(path) = vibrato_source::published(hints_dir) else {
        return Ok((
            None,
            hints_off_warning(&format!("no dictionary in {}", hints_dir.display())),
        ));
    };

    match VibratoTokenizer::load(&path) {
        Ok(tokenizer) => Ok((Some(tokenizer), String::new())),
        // A file that is there but will not load is the same user-visible state
        // as one that is not there, and the same gesture fixes both.
        Err(e) => Ok((None, hints_off_warning(&e.to_string()))),
    }
}
```

- [ ] **Step 10: Run the tests to verify they pass**

```bash
cargo test -p clipyomi state::
```

Expected: PASS. `load_state` still compiles with its own hints branch in place — that comes out next.

- [ ] **Step 11: Take hints out of `AppState`**

In `src-tauri/src/state.rs`:

- Module doc: `//! Startup state: the index, the conjugation table, and optional hints.` becomes

```rust
//! Startup state: the index and the conjugation table.
//!
//! Everything here is built once at launch and then read-only, which is what
//! lets `AppState` live in Tauri's managed state behind a shared reference —
//! and what `parse::catch_parse`'s unwind-safety argument rests on. Hints are
//! deliberately not here: they change while the app runs, and they travel to
//! the worker on their own `watch` channel instead.
```

- `AppState` loses its field:

```rust
pub struct AppState {
    pub index: Index,
    pub table: ConjugationTable,
}
```

- `load_state` loses the env branch and its `Ok` shrinks:

```rust
    let index = Index::open(&generation)?;
    let table = ConjugationTable::load_embedded()?;

    Ok(AppState { index, table })
```

`StartupError::Hints` stays exactly as it is — `load_hints` is its construction site now — so `every_startup_error_variant_renders_a_non_empty_string` and `every_other_startup_error_stays_fatal` need no edit.

In `src-tauri/src/commands.rs`, in `download_dictionary`'s `spawn_blocking` closure, delete the whole `// mirrors state::load_state's hints branch` block and shrink the return:

```rust
        Ok::<AppState, String>(AppState { index, table })
```

2F's download stops knowing about MeCab.

- [ ] **Step 12: Write the failing tests for `current_hints`, then wire the worker**

Append to `src-tauri/src/parse.rs`'s `mod tests`:

```rust
/// Before the toggle is ever pressed there is no tokenizer, and the worker
/// must parse without one rather than wait for it.
#[test]
fn no_hints_yet_yields_no_tokenizer() {
    let (_tx, rx) = watch::channel(None::<Arc<VibratoTokenizer>>);
    assert!(current_hints(&rx).is_none());
}

/// The hints counterpart of `a_published_index_becomes_visible_without_a_restart`,
/// and the mechanism the toggle's "no restart" promise rests on. Writable for
/// real because `write_test_dictionary` makes a dictionary vibrato loads
/// without a 46 MB file or a network call.
#[test]
fn a_published_tokenizer_becomes_visible_without_a_restart() {
    let dir = crate::test_support::scratch("worker-late-hints");
    let path = dir.join("system.dic");
    jparser::hints::write_test_dictionary(&path).expect("fixture dictionary");
    let tokenizer = VibratoTokenizer::load(&path).expect("load");

    let (tx, rx) = watch::channel(None);
    assert!(current_hints(&rx).is_none(), "starts empty");
    tx.send(Some(Arc::new(tokenizer))).expect("send");
    assert!(current_hints(&rx).is_some(), "publishing must be visible");
}
```

And fix `a_published_index_becomes_visible_without_a_restart`, whose `AppState` literal no longer compiles:

```rust
        let state = AppState {
            index: Index::open(&generation).expect("open"),
            table: ConjugationTable::load_embedded().expect("table"),
        };
```

- [ ] **Step 13: Run them to verify they fail**

```bash
cargo test -p clipyomi parse::
```

Expected: FAIL to compile — ``cannot find function `current_hints` in this scope``.

- [ ] **Step 14: Implement `current_hints` and give the worker its third channel**

In `src-tauri/src/parse.rs`, add the import and the accessor beside `current_index`:

```rust
use jparser::hints::VibratoTokenizer;
```

```rust
/// The tokenizer hints are currently derived from, or `None` when they are off.
///
/// Separated from `run_worker` for the reason `current_index` is, and with the
/// same shape: the `watch` borrow is released before the value is returned, so
/// no guard is ever held across an `.await`.
pub fn current_hints(
    hints: &watch::Receiver<Option<Arc<VibratoTokenizer>>>,
) -> Option<Arc<VibratoTokenizer>> {
    hints.borrow().clone()
}
```

Extend `catch_parse`'s doc with one sentence, so the unwind-safety argument still describes the code:

```rust
/// `AssertUnwindSafe` is sound here: the managed state is read-only after
/// startup and no `&mut` crosses this boundary, so there is no invariant a
/// half-completed parse could leave broken. The tokenizer now arrives by `Arc`
/// off a `watch` channel rather than out of `AppState`, and is equally
/// read-only, so nothing about that argument changes.
```

And `run_worker`:

```rust
/// Parse each new input and emit the outcome to the webview.
///
/// The index arrives through `index` rather than being passed by value, because
/// the worker is spawned before startup knows whether there is one: on a first
/// run it starts empty and begins parsing when `commands::download_dictionary`
/// publishes. Input arriving before then is dropped — the download screen is on
/// top at that point, so there is nothing a message could usefully tell the user.
///
/// `hints` is the same shape for a different reason: the tokenizer is switched
/// on and off while the app runs, and read once per parse so a toggle takes
/// effect on the next one without a restart.
pub async fn run_worker(
    app: AppHandle,
    index: watch::Receiver<Option<Arc<AppState>>>,
    hints: watch::Receiver<Option<Arc<VibratoTokenizer>>>,
    mut rx: watch::Receiver<String>,
) {
    while let Some(text) = next_input(&mut rx).await {
        let Some(state) = current_index(&index) else {
            continue;
        };
        let tokenizer = current_hints(&hints);
        let len = text.chars().count();

        let outcome = tauri::async_runtime::spawn_blocking(move || {
            catch_parse(len, || {
                let flags = tokenizer.as_ref().map(|t| t.hints(&text));
                jparser::parse(
                    &state.index,
                    &state.table,
                    &text,
                    &ParseOptions::default(),
                    flags.as_ref().map(|f| f as &dyn BoundaryHints),
                )
                .map_err(|e| e.to_string())
            })
        })
        .await;
```

The rest of the loop — the `match outcome` and the emit — is untouched.

- [ ] **Step 15: Add `HintsSender` and the hints directory to the managed paths**

In `src-tauri/src/commands.rs`, beside `IndexSender`:

```rust
/// The sending half of the hints channel, managed so `set_mecab_hints` can
/// publish a freshly loaded tokenizer to the already-running worker.
///
/// A second channel rather than a rebuilt `AppState` republished over the index
/// one: that path reopens the index on every toggle, and gives a mid-rebuild
/// failure a way to leave the worker holding no index at all.
pub struct HintsSender(pub watch::Sender<Option<Arc<VibratoTokenizer>>>);
```

with `use jparser::hints::VibratoTokenizer;` added to that module's imports, and `DictionaryPaths` gaining its third path:

```rust
/// Where the dictionaries live. Resolved once in `main`'s `setup`, so no command
/// needs the app config dir, and so `source/` stays a sibling of `dict/` rather
/// than living inside a published generation, which is immutable.
pub struct DictionaryPaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
    /// The MeCab hints directory: `<app_config_dir>/hints`, holding the single
    /// published slot and any archive the user drops in beside it.
    pub hints_dir: PathBuf,
}
```

- [ ] **Step 16: Wire it all in `main.rs`**

In `src-tauri/src/main.rs`'s `setup`, after the index channel is created:

```rust
            // Hints travel on their own channel rather than inside `AppState`:
            // that state is built once and then read-only — which is what
            // `parse::catch_parse`'s unwind-safety argument rests on — and a
            // toggle must not rebuild the index to change one flag.
            let (hints_tx, hints_rx) =
                tokio::sync::watch::channel(None::<Arc<jparser::hints::VibratoTokenizer>>);
            app.manage(commands::HintsSender(hints_tx.clone()));
```

The `DictionaryPaths` construction gains its third field, from a directory resolved beside `source_dir`:

```rust
            let source_dir = config_dir.join(jmdict_source::SOURCE_DIR);
            let hints_dir = config_dir.join(vibrato_source::HINTS_DIR);
            app.manage(commands::DictionaryPaths {
                root: root.clone(),
                source_dir,
                hints_dir: hints_dir.clone(),
            });
```

The worker spawn gains its third channel:

```rust
            tauri::async_runtime::spawn(parse::run_worker(handle, index_rx, hints_rx, rx));
```

And the startup match is wrapped, with the existing three-arm `load_state` match moved inside the `Ok` arm **unchanged**:

```rust
            // Hints first, and outside `load_state`: that function returns
            // `Err(NoIndex)` on first run and never reaches its own tail, so
            // hints folded into it would go unloaded on exactly the run that
            // downloads JMdict, and stay unloaded until a restart.
            match state::load_hints(&hints_dir, settings.snapshot().mecab_hints) {
                Ok((tokenizer, warning)) => {
                    // Also kept for developers running from a terminal, the way
                    // the settings warning above already is.
                    if !warning.is_empty() {
                        eprintln!("{warning}");
                    }
                    app.manage(state::HintsWarning(warning));
                    // A send failure would mean the worker died between its
                    // spawn above and this line, which cannot happen — it is
                    // awaiting its first input.
                    let _ = hints_tx.send(tokenizer.map(Arc::new));

                    match state::load_state(&root) {
                        // ... the existing three arms, unchanged ...
                    }
                }
                // `TA_HINTS_DICT` set and unreadable. The same fatal arm
                // today's `load_state` hints error already lands in, so no
                // behaviour moves.
                Err(e) => {
                    app.manage(state::HintsWarning(String::new()));
                    app.manage(state::StartupFailure(e.to_string()));
                    app.manage(state::NeedsDictionary(false));
                }
            }
```

`settings.snapshot()` rather than `loaded.mecab_hints`: `loaded` has already moved into `SettingsState` by this point in `setup`, and reading it back off the shared state avoids a second ordering constraint nobody would notice breaking.

- [ ] **Step 17: Run the whole crate's tests and clippy**

```bash
cargo test -p clipyomi
cargo clippy -p clipyomi --all-targets 2>&1 | grep -c "^warning"
```

Expected: PASS, with the new `state::` and `parse::` tests present. The warning count must equal Task 0's baseline — this is the phase's first touch of `src-tauri`, and the four Windows `upper_case_acronyms` warnings are inherited, not this phase's to fix or to grow.

- [ ] **Step 18: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/settings.rs src-tauri/src/state.rs \
        src-tauri/src/parse.rs src-tauri/src/commands.rs src-tauri/src/main.rs Cargo.lock
git commit -m "feat(clipyomi): carry hints on their own watch channel"
```

---

## Task 4: `src-tauri` commands — the toggle itself

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs` (`HintsInFlight` managed, four commands registered)

**Interfaces:**
- Consumes: `HintsSender`, `DictionaryPaths.hints_dir`, `state::HintsWarning`, `claim_download`, `release_download`, `InputSender`, `SettingsState::update` — all from Task 3 and earlier.
- Produces:
  - `pub const HINTS_STATUS_EVENT: &str = "hints-status"`
  - `pub struct HintsInFlight(pub Arc<AtomicBool>)`
  - `#[tauri::command] pub async fn set_mecab_hints(enabled: bool, ..) -> Result<(), String>`
  - `#[tauri::command] pub fn hints_warning(..) -> Option<String>`
  - `#[tauri::command] pub fn hints_dir(..) -> String`
  - `fn reparse(tx: &watch::Sender<String>)`

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/src/commands.rs`'s `mod tests`:

```rust
/// Nothing has been parsed yet, so there is nothing to re-parse. Sending an
/// empty string would wake the worker to parse nothing.
#[tokio::test]
async fn reparse_skips_an_empty_channel() {
    let (tx, mut rx) = tokio::sync::watch::channel(String::new());
    reparse(&tx);
    assert!(!rx.has_changed().expect("the sender is alive"));
}

/// Enabling hints changes the segmentation of text already on screen. Without
/// this the toggle looks inert until the next clipboard copy, which for a
/// segmentation-quality switch is the entire feedback the user gets.
///
/// `watch::send` marks the slot changed even for an equal value, which is the
/// mechanism this relies on.
#[tokio::test]
async fn reparse_resends_the_current_text() {
    let (tx, mut rx) = tokio::sync::watch::channel(String::new());
    push_input(&tx, "東京都".to_string()).expect("push");
    assert_eq!(
        crate::parse::next_input(&mut rx).await.as_deref(),
        Some("東京都"),
        "the first send is consumed so the re-send is what the assert below sees"
    );

    reparse(&tx);

    assert_eq!(
        crate::parse::next_input(&mut rx).await.as_deref(),
        Some("東京都")
    );
}

/// The same exclusion `download_dictionary` has, re-pointed at the hints slot:
/// two acquisitions against one hints directory would race on the published
/// slot, and the second toggle must be refused rather than queued.
#[test]
fn a_second_hints_acquisition_is_rejected_while_one_is_running() {
    let flag = std::sync::atomic::AtomicBool::new(false);
    claim_download(&flag).expect("the first claim succeeds");
    assert!(claim_download(&flag).is_err());
    release_download(&flag);
    claim_download(&flag).expect("retry after release");
}

/// With any dictionary accepted, a load failure also means a format mismatch —
/// this tree pins `vibrato` 0.5.2, which reads v0.5.0-format dictionaries. The
/// message has to say so rather than reading as a corrupt download.
#[test]
fn a_hints_load_failure_names_the_format_it_expects() {
    let message = describe_hints_error(jparser::hints::HintsError::Dictionary(
        "unsupported version".to_string(),
    ));
    assert!(message.contains("unsupported version"), "got {message}");
    assert!(message.contains("v0.5.0"), "got {message}");
}
```

- [ ] **Step 2: Run them to verify they fail**

```bash
cargo test -p clipyomi commands::
```

Expected: FAIL to compile — ``cannot find function `reparse` `` and ``cannot find function `describe_hints_error` ``.

- [ ] **Step 3: Implement the event, the slot, and the two helpers**

In `src-tauri/src/commands.rs`, after `DICTIONARY_STATUS_EVENT` and its neighbours:

```rust
/// Emitted with a `String` payload as the hints toggle moves between phases:
/// `"downloading"`, `"unpacking"`, `"ready"`, `"off"`, or an error message.
///
/// A const beside `DICTIONARY_STATUS_EVENT` rather than a literal at each emit
/// site, for the reason that one already is.
pub const HINTS_STATUS_EVENT: &str = "hints-status";

/// Guards against two concurrent `set_mecab_hints` acquisitions, exactly as
/// `DownloadInFlight` does for the index download. An `Arc` rather than a bare
/// `AtomicBool` because the command must clone it out of `State` before its
/// first `.await` — `tauri::State` is not `Send`.
pub struct HintsInFlight(pub Arc<AtomicBool>);

/// Emit a hints phase label, logging rather than propagating a failed emit.
fn emit_hints_status(app: &AppHandle, status: &str) {
    if let Err(e) = app.emit(HINTS_STATUS_EVENT, status.to_string()) {
        // Nothing to fall back on: if the event cannot reach the webview there
        // is no other channel to report progress through.
        eprintln!("emitting a hints-status event failed: {e}");
    }
}

/// A `VibratoTokenizer::load` failure, with the format hint the manual route
/// makes necessary.
///
/// Before this phase a load failure meant a corrupt download or a bad
/// `TA_HINTS_DICT`. With any dictionary accepted it also means a format
/// mismatch, and "the vibrato dictionary could not be loaded" alone reads as a
/// corrupt archive.
fn describe_hints_error(err: jparser::hints::HintsError) -> String {
    format!(
        "{err}. A hand-placed archive must hold a Vibrato v0.5.0-format \
         dictionary — that is the format this app downloads."
    )
}

/// Re-parse whatever is on screen.
///
/// Enabling hints changes the segmentation of text already displayed, and
/// without this the toggle appears inert until the next clipboard copy — which
/// for a segmentation-quality switch is the entire feedback the user gets.
///
/// An empty channel value means nothing has been parsed yet, so there is
/// nothing to re-parse. `watch::send` marks the slot changed even for an equal
/// value, so the worker wakes and parses the same text again, and
/// `run_worker`'s loop needs no change — `next_input`'s three tests keep
/// pinning the same code.
///
/// Making the worker `select!` over both channels was the alternative. It puts
/// the rule where any future publisher inherits it, at the cost of rewriting
/// the loop those tests cover and making the worker retain the last input
/// itself.
fn reparse(tx: &watch::Sender<String>) {
    let text = tx.borrow().clone();
    if text.is_empty() {
        return;
    }
    // A dead worker is reported by every other path that touches this channel;
    // a settings toggle is not where the user should learn about it.
    let _ = tx.send(text);
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p clipyomi commands::
```

Expected: PASS, four new tests.

- [ ] **Step 5: Write `set_mecab_hints`**

In `src-tauri/src/commands.rs`, after `download_dictionary`:

```rust
/// Switch MeCab boundary hints on or off, acquiring the dictionary if needed.
///
/// Returns `Err` only when the work could not be *started*; every outcome after
/// that — including failure — arrives as a `hints-status` event. Same split as
/// `download_dictionary` and `set_input`, and for the same reason: one path to
/// the screen.
///
/// The setting is persisted only on success, so a failed acquisition leaves it
/// off and the next launch does not claim hints it does not have.
#[tauri::command]
pub async fn set_mecab_hints(
    enabled: bool,
    app: AppHandle,
    settings: State<'_, Arc<SettingsState>>,
    hints: State<'_, HintsSender>,
    input: State<'_, InputSender>,
    paths: State<'_, DictionaryPaths>,
    inflight: State<'_, HintsInFlight>,
) -> Result<(), String> {
    // Everything needed after the `.await` is cloned out first: `tauri::State`
    // is not `Send`, so holding one across an await point does not compile.
    let hints_tx = hints.0.clone();
    let input_tx = input.0.clone();
    let settings = Arc::clone(&*settings);
    let hints_dir = paths.hints_dir.clone();

    if !enabled {
        // Nothing fallible and nothing slow, so no slot to claim and no thread
        // to spawn: publishing `None` is the whole of switching hints off.
        let _ = hints_tx.send(None);
        if let Err(e) = settings.update(|s| s.mecab_hints = false) {
            // A read-only config dir degrades to "the setting does not survive
            // a restart", not "the toggle does not work" — the same trade
            // `SettingsState::update` already documents.
            eprintln!("persisting the hints setting failed: {e}");
        }
        reparse(&input_tx);
        emit_hints_status(&app, "off");
        return Ok(());
    }

    claim_download(&inflight.0)?;
    let flag = Arc::clone(&inflight.0);
    let handle = app.clone();

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        // `resolve` returns a path, not which branch produced it, and by the
        // time it returns the label is no longer useful — so the branch is
        // picked here, with one `read_dir`. Being wrong costs a mislabelled
        // phase, not a wrong outcome, so the window between this check and
        // `resolve`'s own call is not worth a second return channel.
        let phase = if vibrato_source::hand_placed(&hints_dir).is_some() {
            "unpacking"
        } else {
            "downloading"
        };
        emit_hints_status(&handle, phase);

        let path = vibrato_source::resolve(&hints_dir).map_err(|e| e.to_string())?;
        jparser::hints::VibratoTokenizer::load(&path).map_err(describe_hints_error)
    })
    .await;

    release_download(&flag);

    let message = match outcome {
        Ok(Ok(tokenizer)) => {
            let _ = hints_tx.send(Some(Arc::new(tokenizer)));
            if let Err(e) = settings.update(|s| s.mecab_hints = true) {
                eprintln!("persisting the hints setting failed: {e}");
            }
            reparse(&input_tx);
            emit_hints_status(&app, "ready");
            return Ok(());
        }
        Ok(Err(message)) => message,
        Err(e) => format!("the hints task failed to run: {e}"),
    };

    emit_hints_status(&app, &message);
    Ok(())
}

/// Why hints are off while the setting says they are on, or `null` when the two
/// agree. Same empty-string sentinel as `shortcut_warning`, and read by both
/// windows — the main window appends it to `#parse-error`, the settings window
/// seeds its status line from it.
#[tauri::command]
pub fn hints_warning(warning: State<'_, HintsWarning>) -> Option<String> {
    if warning.0.is_empty() {
        None
    } else {
        Some(warning.0.clone())
    }
}

/// Where to drop a hand-placed dictionary archive.
///
/// The settings note names this path literally, so the manual route is
/// discoverable without documentation, and the webview cannot resolve the app
/// config dir itself.
#[tauri::command]
pub fn hints_dir(paths: State<'_, DictionaryPaths>) -> String {
    paths.hints_dir.display().to_string()
}
```

Add `HintsWarning` to that file's `use crate::state::{...}` list.

- [ ] **Step 6: Register everything in `main.rs`**

Beside `DownloadInFlight`:

```rust
            app.manage(commands::HintsInFlight(Arc::new(AtomicBool::new(false))));
```

And in `invoke_handler![...]`, after `commands::needs_dictionary`:

```rust
            commands::set_mecab_hints,
            commands::hints_warning,
            commands::hints_dir,
```

- [ ] **Step 7: Run the crate's tests and clippy**

```bash
cargo test -p clipyomi
cargo clippy -p clipyomi --all-targets 2>&1 | grep -c "^warning"
```

Expected: PASS; warning count still equal to Task 0's baseline.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs
git commit -m "feat(clipyomi): set_mecab_hints acquires and publishes the tokenizer"
```

---

## Task 5: The frontend — the row, its lifecycle, and the warning

**Files:**
- Modify: `src/types.ts`
- Modify: `src/settings.ts`
- Modify: `src/main.ts`
- Test: `src/settings.test.ts`, `src/main.test.ts`

**Interfaces:**
- Consumes: the commands `set_mecab_hints`, `hints_warning`, `hints_dir`, `get_settings`, and the `hints-status` event, all from Task 4.
- Produces: `Settings.mecab_hints?: boolean` in `src/types.ts`; `export async function showHintsWarning(): Promise<void>` in `src/main.ts`.

- [ ] **Step 1: Write the failing settings-window tests**

Append to `src/settings.test.ts`, inside the existing `describe('Settings Window', ...)`:

```ts
  /** Flush the promise chains `initSettingsApp` starts on import. */
  async function settled(): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  test('the hints checkbox reflects the persisted setting', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: true,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLInputElement>('#setting-mecab-hints')?.checked,
    ).toBe(true);
  });

  test('toggling hints on disables the checkbox and invokes set_mecab_hints', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: false,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));

    expect(invoke).toHaveBeenCalledWith('set_mecab_hints', { enabled: true });
    // The backend's event decides what the checkbox ends up showing; until one
    // arrives, a second click must not be able to race the acquisition.
    expect(checkbox.disabled).toBe(true);
  });

  test('a phase event shows its label and leaves the checkbox disabled', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'downloading');

    const status = container.querySelector<HTMLElement>('#setting-mecab-hints-status');
    expect(status?.textContent ?? '').not.toBe('');
    expect(status?.hidden).toBe(false);
    expect(checkbox.disabled).toBe(true);
  });

  test('a ready event checks the box, re-enables it, and clears the status', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'unpacking');
    emit('hints-status', 'ready');

    expect(checkbox.checked).toBe(true);
    expect(checkbox.disabled).toBe(false);
    expect(container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.hidden).toBe(
      true,
    );
  });

  test('an off event unchecks the box and re-enables it', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = false;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'off');

    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(false);
  });

  test('a failure message unchecks the box and shows the message verbatim', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'the dictionary server returned HTTP 404');

    // The setting was not persisted, so the checkbox must not read as on.
    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(false);
    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.textContent,
    ).toContain('HTTP 404');
  });

  test('a startup hints warning seeds the status line', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') {
        return Promise.resolve('MeCab boundary hints are switched on but not loaded');
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.textContent,
    ).toContain('not loaded');
  });

  test('the note names the hints directory so the manual route is discoverable', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_dir') return Promise.resolve('C:\\Users\\me\\AppData\\clipyomi\\hints');
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-note')?.textContent,
    ).toContain('clipyomi\\hints');
  });

  test('saving another setting never sends mecab_hints', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: true,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
    hidePos.checked = true;
    hidePos.dispatchEvent(new Event('change'));

    const save = invoke.mock.calls.find(([cmd]) => cmd === 'save_settings');
    expect(save).toBeDefined();
    // The backend owns this key outright. A snapshot carrying it could clobber
    // an in-flight toggle, and `merge_saved` is a whitelist precisely so it
    // cannot — this asserts the frontend half of that agreement.
    expect(Object.keys(save![1].settings)).not.toContain('mecab_hints');
  });
```

- [ ] **Step 2: Run them to verify they fail**

```bash
npx vitest run src/settings.test.ts
```

Expected: FAIL — `#setting-mecab-hints` is `null`, so the non-null assertions throw.

- [ ] **Step 3: Add the type**

In `src/types.ts`, in `Settings`, below `clipboard_monitoring`:

```ts
  /**
   * MeCab boundary hints. Backend-owned: `save_settings` never carries it, so
   * the settings window reads it and never writes it.
   */
  mecab_hints?: boolean;
```

- [ ] **Step 4: Add the Segmentation group to the markup**

In `src/settings.ts`, in the template literal, directly **above** the `Window &amp; Behavior` section:

```html
      <section class="settings-group">
        <h2 class="settings-group-title">Segmentation</h2>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-mecab-hints" />
          <span class="label-text">Boundary hints (MeCab)</span>
        </label>
        <p class="settings-note" id="setting-mecab-hints-note">
          Improves word splitting. Enabling downloads an 8 MB IPADIC dictionary.
        </p>
        <p class="settings-note" id="setting-mecab-hints-status" hidden></p>
      </section>
```

- [ ] **Step 5: Add its lifecycle**

In `src/settings.ts`, with the other `querySelector` handles:

```ts
  const mecabHints = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
  const mecabNote = container.querySelector<HTMLElement>('#setting-mecab-hints-note')!;
  const mecabStatus = container.querySelector<HTMLElement>('#setting-mecab-hints-status')!;
```

Then, below `updateFuriganaButtons`:

```ts
  /**
   * The backend's own phase labels. Anything not here is an error message to
   * show verbatim — the same lookup-not-enum shape `main.ts` uses for
   * `dictionary-status`, and for the same reason: the failure arm carries text
   * the user needs to read.
   */
  const HINTS_PHASES: Record<string, string> = {
    downloading: 'Downloading the dictionary…',
    unpacking: 'Unpacking the archive…',
  };

  function showHintsStatus(message: string) {
    mecabStatus.textContent = message;
    mecabStatus.hidden = message === '';
  }

  /**
   * The checkbox is never the source of truth: this is. A toggle disables the
   * control and invokes; what it ends up showing is whatever the backend says
   * happened.
   */
  function applyHintsStatus(status: string) {
    const phase = HINTS_PHASES[status];
    if (phase !== undefined) {
      // Left exactly as the click set it: the work is still running, and
      // flipping it here would fight the user's own gesture mid-fetch.
      showHintsStatus(phase);
      return;
    }
    mecabHints.disabled = false;
    if (status === 'ready') {
      mecabHints.checked = true;
      showHintsStatus('');
      return;
    }
    if (status === 'off') {
      mecabHints.checked = false;
      showHintsStatus('');
      return;
    }
    // A failure. The backend persisted nothing, so the checkbox must not read
    // as on — and the message is the only explanation the user gets.
    mecabHints.checked = false;
    showHintsStatus(status);
  }

  mecabHints.addEventListener('change', () => {
    mecabHints.disabled = true;
    showHintsStatus('');
    void invoke('set_mecab_hints', { enabled: mecabHints.checked }).catch((e) => {
      // Only reached when the work could not be *started* — no `hints-status`
      // event will arrive, so this is the one path that has to re-enable the
      // control itself.
      mecabHints.disabled = false;
      mecabHints.checked = false;
      showHintsStatus(String(e));
    });
  });
```

In `applySettings`, beside the other checkbox restores:

```ts
    if (s.mecab_hints !== undefined) mecabHints.checked = s.mecab_hints;
```

`currentSnapshot()` is deliberately not touched: this key never travels back.

And with the other module-level `invoke`/`listen` calls at the bottom of `initSettingsApp`:

```ts
  // The path is named literally so the manual route needs no documentation to
  // find; the webview cannot resolve the app config dir itself.
  void invoke<string>('hints_dir')
    .then((dir) => {
      if (typeof dir !== 'string' || dir === '') return;
      mecabNote.textContent =
        `Improves word splitting. Enabling downloads an 8 MB IPADIC dictionary. ` +
        `To use your own instead, drop a Vibrato .tar.xz in ${dir} and toggle this ` +
        `off and on — the newest archive wins.`;
    })
    .catch(() => {});

  // Seeded from the same command the main window reads, so a user who opens
  // settings to fix the warning sees the explanation next to the control.
  void invoke<string | null>('hints_warning')
    .then((message) => {
      if (message !== null && message !== undefined) showHintsStatus(String(message));
    })
    .catch(() => {});

  void listen<string>('hints-status', (e) => {
    applyHintsStatus(e.payload);
  });
```

- [ ] **Step 6: Run the tests to verify they pass**

```bash
npx vitest run src/settings.test.ts
npx tsc --noEmit
```

Expected: PASS, and a clean type-check.

- [ ] **Step 7: Write the failing main-window test, and unblock the two that would break**

In `src/main.test.ts`, the two tests whose mock falls through to a rejection (`'no other invoke call is expected in this test'` and `'unused'`) now see one more module-level call. Add to **both**:

```ts
      if (cmd === 'hints_warning') return Promise.resolve(null);
```

Then append to the `describe('the settings warning', ...)` block:

```ts
  test('hints silently off is reported in the window', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') {
        return Promise.resolve('MeCab boundary hints are switched on but not loaded');
      }
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showHintsWarning();

    expect(document.querySelector('#parse-error')?.textContent).toContain('not loaded');
  });

  test('a hints warning and a hotkey warning both survive', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') return Promise.resolve('hints are switched on but not loaded');
      if (cmd === 'shortcut_warning') return Promise.resolve('hotkeys in use: Alt+BracketLeft');
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showHintsWarning();
    await main.showShortcutWarning();

    const text = document.querySelector('#parse-error')?.textContent ?? '';
    expect(text).toContain('not loaded');
    expect(text).toContain('Alt+BracketLeft');
  });
```

- [ ] **Step 8: Run them to verify they fail**

```bash
npx vitest run src/main.test.ts
```

Expected: FAIL — `main.showHintsWarning is not a function`.

- [ ] **Step 9: Implement `showHintsWarning`**

In `src/main.ts`, directly below `showShortcutWarning`:

```ts
// Hints that are silently off are invisible otherwise: the setting reads on and
// the parse has none, which looks like the segmentation simply not improving.
// Exported for the same reason `showStartupError` is.
export async function showHintsWarning(): Promise<void> {
  const message = await invoke<string | null>('hints_warning');
  if (message === null) return;
  // `append`, not `replaceChildren`, for the reason `showSettingsWarning`
  // documents: a corrupt settings.json and a taken hotkey can already both be
  // in this slot, and none of the three may wipe the others.
  parseError.append(errorBlock(message));
}
```

And at the bottom of the module, beside the other two:

```ts
void showHintsWarning();
```

- [ ] **Step 10: Run the whole frontend suite**

```bash
npx vitest run
npx tsc --noEmit
```

Expected: PASS, at least 11 tests more than Task 0's baseline (9 settings + 2 main).

- [ ] **Step 11: Commit**

```bash
git add src/types.ts src/settings.ts src/settings.test.ts src/main.ts src/main.test.ts
git commit -m "feat(settings): add the MeCab boundary hints toggle and its warning"
```

---

## Task 6: Manual verification, the gate, and the docs

**Files:**
- Modify: `docs/superpowers/specs/2026-08-12-jparser-port-design.md`
- Create: `docs/superpowers/2026-08-27-jparser-phase5b-handoff.md`

**Interfaces:**
- Consumes: everything Tasks 1–5 built.
- Produces: the recorded manual results, the master-spec edits, and the handoff.

- [ ] **Step 1: Run the full gate**

```bash
cargo test --workspace
cargo test -p jparser --features hints-source
cargo check -p jparser --no-default-features --all-targets
cargo clippy --workspace --all-targets --exclude clipyomi -- -D warnings
cargo clippy -p clipyomi --all-targets 2>&1 | grep -c "^warning"
npx vitest run
npx playwright test
cargo tree --workspace | grep -E "openssl v|openssl-sys|native-tls|\bring v|aws-lc" || echo "TLS gate clean"
cargo tree --workspace --target x86_64-unknown-linux-gnu | grep -E "openssl v|openssl-sys|native-tls|\bring v|aws-lc" || echo "TLS gate clean (linux)"
```

Every count at or above Task 0's baseline; the `clipyomi` warning count exactly equal to it; both TLS greps clean. Record the numbers as they come out — the handoff quotes them.

- [ ] **Step 2: Manual check 1 — a fresh install downloads and takes effect live**

Move any existing `<app_config_dir>/hints` aside, launch the app (`npm run tauri dev`), copy `東京都に住んでいます` to the clipboard, and note the segmentation. Then open Settings and tick **Boundary hints (MeCab)**.

Expect: the status line reads `Downloading the dictionary…`, then clears with the box checked, and the sentence on screen **re-segments without a restart**. Record the wall time and the resulting segmentation.

- [ ] **Step 3: Manual check 2 — the second toggle is offline and instant**

Untick the box, then tick it again.

Expect: `ready` effectively immediately, `unpacking` never shown, and no network activity — `published()` short-circuited inside `resolve`. Record it.

- [ ] **Step 4: Manual check 3 — a hand-placed archive replaces the dictionary**

With the app running, copy the 5A asset (or any Vibrato `.tar.xz`) into `<app_config_dir>/hints/`, then untick and re-tick the box.

Expect: the status line reads `Unpacking the archive…`, `<hints>/current/` afterwards holds the archive's dictionary, and `ls -a <hints>` shows **no** `.partial` or `.backup` leftovers. Record the directory listing.

- [ ] **Step 5: Manual check 4 — a missing dictionary is a warning, not a fatal**

Quit the app. Delete `<app_config_dir>/hints/current/` while `settings.json` still has `"mecab_hints": true`. Relaunch.

Expect: the app starts normally, the warning appears in the main window's `#parse-error` **and** in the settings window's status line, and the checkbox still reads on. Record both.

- [ ] **Step 6: Manual check 5 — an unloadable dictionary names the format**

Quit. Write junk over `<hints>/current/system.dic`. Relaunch, open Settings, untick and re-tick.

Expect: the failure message names the v0.5.0 format, the checkbox falls back to unchecked, and `settings.json` still holds `"mecab_hints": false` afterwards. Record the message verbatim.

- [ ] **Step 7: Manual check 6 — the env override is still fatal**

Quit. Launch with `TA_HINTS_DICT` pointing at a path that does not exist.

Expect: a startup failure in `#output`, exactly as 2C shipped it. Note 5A's caveat when writing this up — the desktop app surfaces startup failures only into `StartupFailure` for the webview, so the evidence is the message in the window, not a line on stderr.

- [ ] **Step 8: Update the master spec**

In `docs/superpowers/specs/2026-08-12-jparser-port-design.md`:

- §11's Phase 5 table, the **5B** row: rewrite to past tense in the voice of the 5A row above it — the persisted `mecab_hints` setting and its settings-window control, `set_mecab_hints` acquiring on demand and publishing to the live worker over a second `watch`, `AppState.hints` deleted, an absent dictionary at startup meaning hints off plus a warning rather than a fatal, the hand-placed-archive route accepting any Vibrato dictionary, and `TA_HINTS_DICT` keeping its fatal. Name §12 rows 19–20.
- §12: append rows 19 and 20, copied **verbatim** from the design spec's §13 table.

- [ ] **Step 9: Write the handoff**

Create `docs/superpowers/2026-08-27-jparser-phase5b-handoff.md`, following `2026-08-27-jparser-phase5a-handoff.md`'s section shape:

1. **What shipped** — one entry per task's commit, with its test count.
2. **The measured facts** — Steps 2–7's recorded numbers and messages.
3. **What 6A/6B inherits** — the `hints-status` contract, `HintsWarning`, and the fact that the settings window is still absent from the Playwright suite (6B's scope, not this phase's).
4. **What is deliberately absent** — the design spec's §12 list: non-IPADIC detection, startup installation of a hand-placed archive, a progress percentage, staging sweeps, layout slots capturing `mecab_hints`, and update checking.
5. **Findings beyond the design spec** — anything implementation forced that §1–§13 do not describe.
6. **The verification gate** — Step 1's output, including the `clipyomi` clippy count and the note that the four Windows `upper_case_acronyms` errors are inherited from 4D and 5A and were neither fixed nor added to.
7. **Deviations folded into the master spec** — rows 19 and 20.
8. **What this phase did not clear** — anything from the 4D/5A handoffs still outstanding.

- [ ] **Step 10: Commit**

```bash
git add docs/superpowers/specs/2026-08-12-jparser-port-design.md \
        docs/superpowers/2026-08-27-jparser-phase5b-handoff.md
git commit -m "docs(phase5b): record the close, the measured facts, and deviations 19-20"
```

---

## Notes for the executor

- **The one ordering trap.** Task 3 removes `AppState.hints` and Task 3 Step 14 changes `run_worker`'s signature. Between Step 11 and Step 16 the crate does not compile — that is expected, and Step 17 is the first place it must. Do not "fix" the intermediate state by leaving the field in place.
- **`DICT_DIR` changed value, not name.** Any 5A-era `<hints>/ipadic-mecab-2_7_0/` on disk becomes invisible to this code. That is deliberate (design §3.1) and needs no migration: the toggle re-acquires into `current/`, and the old directory is inert.
- **Do not add `mecab_hints` to `merge_saved`.** If a test seems to want it there, the test is wrong: the settings window reads this key and never writes it.
- **The archive branch must never open a socket.** Every `resolve` test that exercises it asserts the connection counter is `0`. If a change makes that assertion awkward, the change is wrong, not the assertion.
- **`.github/workflows/ci.yml` is not edited by this phase.** Both gates already cover the new shape: the purity grep asserts `jparser --no-default-features` pulls in neither source crate, and the TLS grep runs over `cargo tree --workspace`, where `src-tauri` already appeared — it now reaches `rustls` + `rustls-graviola` through `vibrato-source`, which is the sanctioned provider that gate's comment already names. The `hints` job keeps compiling `jparser --features hints-source`.
- **`jparser`'s `hints-source` feature is not touched.** It stays what 5A made it — the CLI driver's gate. The shell depends on `vibrato-source` directly, the way it already does on `jmdict-source`.
