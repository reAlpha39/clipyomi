# JParser Phase 2F — First-Run Dictionary Download Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A new user opens the app, presses one button, and has a working dictionary — no terminal.

**Architecture:** The downloader already exists in `crates/jmdict-source`. This phase wires it in. `AppState` stops being Tauri-managed and travels to the parse worker through a second `watch` channel, so the worker can be spawned unconditionally and start working the moment an index arrives — no restart. Startup gains a third outcome: `NoIndex` becomes an actionable download screen instead of a fatal error.

**Tech Stack:** Rust 2021 / `src-tauri` MSRV 1.88, `tauri 2`, `tokio` (sync/macros/rt/time), `jmdict-source` (new to the shell), Vite, TypeScript, Vitest, Playwright. No frontend framework.

**Spec:** `docs/superpowers/specs/2026-08-14-jparser-phase2f-design.md` (authoritative). Port design `docs/superpowers/specs/2026-08-12-jparser-port-design.md` §4.3/§4.4 for the data assets. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **License GPL v2.** Every new Rust source file gets the standard header comment, copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **`src-tauri` MSRV is 1.88.** The MSRV gate is `cargo +1.85 check -p jparser -p jmdict-source -p xtask` — **never `--workspace`**, which fails inside Tauri's transitive tree by design.
- **`crates/jparser` gains nothing this phase.** No edits at all. `crates/jparser/src/segment.rs` is at 778/800 lines and must not be touched.
- **`mecab` stays off by default in `jparser`.** The purity gate `cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"` must keep returning **0**.
- **Errors are explicit:** no `unwrap()`, `expect()`, or `unreachable!()` in library or binary code outside `#[cfg(test)]`. The `.expect` closing `main` is the one documented exception. Never swallow an error without a comment naming the reason.
- **Names are frozen.** New command: `download_dictionary`. New event: `dictionary-status`. Existing commands `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`, `startup_error`, `settings_warning`, `frontend_ready` and events `parse-result`, `parse-error` are unchanged.
- **No profile may set `panic = "abort"`.** `catch_parse`'s containment catches nothing under an aborting profile.
- **File size** 200–400 lines typical, **800 hard maximum including tests**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never `cargo fmt`.** `src-tauri/src/main.rs` is permitted (standing ruling from 2E); run `git diff --stat` after and `git checkout --` anything unintended. The ban remains absolute for `crates/jparser/src/lib.rs`.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` **and** `cargo clippy -p jparser --features mecab --all-targets -- -D warnings` clean at the end of every task.
- **No frontend framework** — vanilla TypeScript and DOM APIs only.
- **Every colour defined on bare `:root` first**; media queries and `[data-theme]` may only redefine. Only `transform` and `opacity` animated; the `prefers-reduced-motion` block must keep working.
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`.**

**Invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format; a published `gen-N` is immutable; directory knowledge lives only in `generations.rs` and `ensure_dictionary`; a `.partial` file is never resolved; the eight serialized `WordFlags` names are public API; `StartupFailure`'s empty string means "startup succeeded", so every `StartupError` variant must keep rendering non-empty.

---

## Resolved facts — do not re-derive these

Measured against the tree at commit `fbc7f4f`.

| Fact | Value |
|---|---|
| Downloader | `jmdict_source::resolve(&source_dir) -> std::io::Result<Box<dyn BufRead>>`; retries 3× with 2 s backoff internally |
| Source dir name | `jmdict_source::SOURCE_DIR` = `"source"` |
| Archive name | `jmdict_source::SOURCE_FILE` = `"JMdict_e.gz"` |
| Cached-file reuse | `resolve` checks `source_dir.join(SOURCE_FILE)` exists before fetching; `open_local` sniffs gzip magic, so a hand-decompressed file also works |
| Build entry point | `jparser::index::ensure_dictionary(&root, &table, &opts, keep, source_closure) -> Result<Index, IndexError>` |
| Build args used by the CLI | `ConjugationTable::load_embedded()`, `StemOptions::default()`, `jparser::index::generations::DEFAULT_KEEP_GENERATIONS` |
| Index build time | **under 15 s** on the reference machine — so no progress callback, and `crates/jparser` needs no API change |
| `AppState` consumers | `parse::run_worker` only. 2E deleted `parse_text`, which was the sole `State<'_, Arc<AppState>>` user |
| `State` across `.await` | Not `Send`. An async command must clone what it needs out of `State` **before** the first `.await` — the pattern 2E's deleted `parse_text` used |
| Capability | `dictionary-status` is an event; `core:event:allow-listen` is already granted. **No capability change needed** |
| Baseline tests | 343 Rust passed / 1 ignored; 27 Vitest; 11 Playwright local and CI-simulated |
| Current sizes | `main.rs` 123, `parse.rs` 180, `commands.rs` 138, `state.rs` 336, `clipboard.rs` 251, `main.ts` 205 |

**No test may touch the network or run a real index build** (spec §6). `jmdict-source` already tests `fetch_with_retry` against a local listener; duplicating that here buys nothing.

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/parse.rs` | *(modified)* worker takes the index through a channel and skips while it is absent |
| `src-tauri/src/state.rs` | *(modified)* `NeedsDictionary`, `is_missing_dictionary` |
| `src-tauri/src/commands.rs` | *(modified)* `download_dictionary`, `needs_dictionary`, the in-flight guard, `IndexSender`, `DictionaryPaths` |
| `src-tauri/src/main.rs` | *(modified)* index channel, unconditional worker spawn, third startup outcome, handler list |
| `src-tauri/Cargo.toml` | *(modified)* `jmdict-source` |
| `src/main.ts` | *(modified)* the three-state download screen |
| `src/main.test.ts` | *(modified)* state tests |
| `src/styles/global.css` | *(modified)* `.dictionary`, `.spinner` |
| `e2e/stub.ts`, `e2e/panes.spec.ts` | *(modified)* stub the new commands, assert the screen |

---

## Task 1: The worker survives having no index

**Files:**
- Modify: `src-tauri/src/parse.rs`, `src-tauri/src/main.rs` (call-site shim only)

**Interfaces:**
- Consumes: `AppState` from `state.rs`.
- Produces: `run_worker(AppHandle, watch::Receiver<Option<Arc<AppState>>>, watch::Receiver<String>)` and `current_index(&watch::Receiver<Option<Arc<AppState>>>) -> Option<Arc<AppState>>`. Task 4 spawns `run_worker`.

This task alone will not compile `main.rs`, which still calls the old three-argument `run_worker` with an `Arc<AppState>`. **Update that call site minimally so the tree builds** — construct a throwaway `watch::channel(Some(shared))` and pass its receiver — and say in your report that you did so as a temporary shim Task 4 replaces.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/parse.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    /// Before the first-run download finishes there is no index, and input can
    /// still arrive — the clipboard poll runs regardless. The worker must skip
    /// rather than panic or emit an error the user cannot act on.
    #[test]
    fn no_index_yet_yields_nothing_to_parse() {
        let (_tx, rx) = watch::channel(None::<Arc<AppState>>);
        assert!(current_index(&rx).is_none());
    }

    /// The download publishes into this channel; the worker must see it without
    /// a restart. This is the mechanism 2F's "no restart" promise rests on.
    #[test]
    fn a_published_index_becomes_visible_without_a_restart() {
        use jparser::conjugation::ConjugationTable;
        use jparser::index::load::Index;

        let root = crate::test_support::scratch("worker-late-index");
        let generation = crate::test_support::build_index_generation(&root);
        let state = AppState {
            index: Index::open(&generation).expect("open"),
            table: ConjugationTable::load_embedded().expect("table"),
            hints: None,
        };

        let (tx, rx) = watch::channel(None);
        assert!(current_index(&rx).is_none(), "starts empty");
        tx.send(Some(Arc::new(state))).expect("send");
        assert!(current_index(&rx).is_some(), "publishing must be visible");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator parse`

Expected: FAIL to compile — `cannot find function 'current_index' in this scope`. That is the intended RED.

- [ ] **Step 3: Implement**

In `src-tauri/src/parse.rs`, add above `run_worker`:

```rust
/// The index, or `None` while the app is still waiting for a first-run
/// download to finish.
///
/// Separated from `run_worker` so the "no index yet" branch is testable
/// without a live Tauri app handle. The `watch` borrow is released before the
/// value is returned, so no guard is ever held across an `.await`.
pub fn current_index(index: &watch::Receiver<Option<Arc<AppState>>>) -> Option<Arc<AppState>> {
    index.borrow().clone()
}
```

Then replace `run_worker`'s signature and the first two lines of its loop body:

```rust
/// Parse each new input and emit the outcome to the webview.
///
/// The index arrives through `index` rather than being passed by value, because
/// the worker is spawned before startup knows whether there is one: on a first
/// run it starts empty and begins parsing when `commands::download_dictionary`
/// publishes. Input arriving before then is dropped — the download screen is on
/// top at that point, so there is nothing a message could usefully tell the user.
pub async fn run_worker(
    app: AppHandle,
    index: watch::Receiver<Option<Arc<AppState>>>,
    mut rx: watch::Receiver<String>,
) {
    while let Some(text) = next_input(&mut rx).await {
        let Some(state) = current_index(&index) else {
            continue;
        };
        let len = text.chars().count();
```

Everything from `let outcome = tauri::async_runtime::spawn_blocking(move || {` onward is unchanged. The old `let state = Arc::clone(&state);` line is deleted — `current_index` already returns an owned `Arc`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator parse`
Expected: PASS, 7 tests.

- [ ] **Step 5: Prove the seam is load-bearing**

Delete `current_index` and inline `index.borrow().clone()` at its call site. Re-run `cargo test -p translation-aggregator parse` and confirm both new tests fail to compile. Restore, re-run green, and record both outputs.

A guard whose absence no test notices is not a guard.

- [ ] **Step 6: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/parse.rs
cargo clippy --workspace --all-targets -- -D warnings
git diff --stat
git add src-tauri/src/parse.rs src-tauri/src/main.rs
git commit -m "feat: let the parse worker start before an index exists"
```

---

## Task 2: Startup tells "no dictionary" apart from "broken"

**Files:**
- Modify: `src-tauri/src/state.rs`

**Interfaces:**
- Consumes: `StartupError` from `state.rs`.
- Produces: `is_missing_dictionary(&StartupError) -> bool` and `NeedsDictionary(pub bool)`. Task 4 manages `NeedsDictionary`; Task 3 exposes it as a command.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/state.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    /// The first-run condition is fixable from inside the window, so it must
    /// not take the fatal path that disables the parse controls.
    #[test]
    fn a_missing_index_is_the_first_run_condition_not_a_failure() {
        let err = StartupError::NoIndex {
            root: PathBuf::from("/nowhere"),
        };
        assert!(is_missing_dictionary(&err));
    }

    /// Everything else stays fatal. Listed exhaustively rather than with a
    /// wildcard so a new `StartupError` variant forces a decision here instead
    /// of silently defaulting to "offer a download that cannot help".
    #[test]
    fn every_other_startup_error_stays_fatal() {
        let fatal: Vec<StartupError> = vec![
            StartupError::Index(IndexError::Io(std::io::Error::other("boom"))),
            StartupError::Conjugation(ConjugationError::BadPartOfSpeech {
                name: "x".to_string(),
                pos: "y".to_string(),
            }),
            StartupError::Hints(HintsError::Dictionary("boom".to_string())),
        ];
        for err in fatal {
            assert!(
                !is_missing_dictionary(&err),
                "{err:?} must not offer a download"
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator state`

Expected: FAIL to compile — `cannot find function 'is_missing_dictionary'`. That is the intended RED.

- [ ] **Step 3: Implement**

Insert into `src-tauri/src/state.rs`, next to `StartupFailure`:

```rust
/// Whether startup found no index at all.
///
/// Managed unconditionally alongside `StartupFailure`, `false` meaning "there
/// is an index, or something worse is wrong", because a command parameter
/// cannot express "this state may not be managed" — see `StartupFailure`'s own
/// doc comment for the `Option<State<'_, T>>` reasoning.
pub struct NeedsDictionary(pub bool);

/// Whether this startup error is the first-run condition the download screen
/// can fix, rather than a genuine failure.
///
/// Matched exhaustively rather than with a wildcard: a future `StartupError`
/// variant must force a decision here. Offering a download for an error a
/// download cannot fix would loop the user through a wait that changes nothing.
pub fn is_missing_dictionary(error: &StartupError) -> bool {
    match error {
        StartupError::NoIndex { .. } => true,
        StartupError::Index(_) | StartupError::Conjugation(_) | StartupError::Hints(_) => false,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator state`
Expected: PASS. Report the count.

- [ ] **Step 5: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/state.rs
cargo clippy --workspace --all-targets -- -D warnings
git diff --stat
git add src-tauri/src/state.rs
git commit -m "feat: distinguish a missing dictionary from a fatal startup error"
```

`git diff --stat` must show only that one file.

---

## Task 3: The download command

**Files:**
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `NeedsDictionary` (Task 2), `AppState` (state.rs).
- Produces: `IndexSender(pub watch::Sender<Option<Arc<AppState>>>)`, `DictionaryPaths { root, source_dir }`, `DownloadInFlight(pub Arc<AtomicBool>)`, `DICTIONARY_STATUS_EVENT` = `"dictionary-status"`, and the commands `download_dictionary`, `needs_dictionary`. Task 4 manages all three state types and registers both commands.

- [ ] **Step 1: Add the dependency**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
# The first network code in the shell. `crates/jparser` must stay pure — the
# purity gate guards it — but the shell is where a download belongs, and
# `jmdict-source` already owns the retry, staging, and gzip handling.
jmdict-source = { path = "../crates/jmdict-source" }
```

- [ ] **Step 2: Write the failing tests**

Add to `src-tauri/src/commands.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    /// Two overlapping `ensure_dictionary` runs would build two generations
    /// against one source directory. The frontend's own guard protects one
    /// button in one webview, not the command, so the command needs its own.
    #[test]
    fn a_second_download_is_rejected_while_one_is_running() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("the first claim succeeds");
        assert!(
            claim_download(&flag).is_err(),
            "a concurrent claim must be refused"
        );
    }

    /// A finished download — successful or not — must leave the door open, or
    /// one failure would disable Retry for the rest of the session.
    #[test]
    fn releasing_the_claim_allows_a_retry() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("first");
        release_download(&flag);
        claim_download(&flag).expect("retry after release");
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator commands`

Expected: FAIL to compile — `cannot find function 'claim_download'`. That is the intended RED.

- [ ] **Step 4: Implement**

Add to the imports at the top of `src-tauri/src/commands.rs`:

```rust
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

use crate::state::{AppState, NeedsDictionary};
```

Then insert above the test module:

```rust
/// Emitted with a `String` payload as the download moves between phases:
/// `"downloading"`, `"building"`, `"ready"`, or an error message.
pub const DICTIONARY_STATUS_EVENT: &str = "dictionary-status";

/// The sending half of the index channel, managed so `download_dictionary` can
/// publish a freshly built index to the already-running worker.
pub struct IndexSender(pub watch::Sender<Option<Arc<AppState>>>);

/// Where the dictionary lives. Resolved once in `main`'s `setup`, so the
/// command does not need the app config dir, and so `source/` stays a sibling
/// of `dict/` rather than living inside a published generation, which is
/// immutable.
pub struct DictionaryPaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
}

/// Guards against two concurrent `ensure_dictionary` runs. An `Arc` rather than
/// a bare `AtomicBool` because the command must clone it out of `State` before
/// its first `.await` — `tauri::State` is not `Send`.
pub struct DownloadInFlight(pub Arc<AtomicBool>);

/// Take the download slot, or report that it is already taken.
///
/// Separated from the command so the exclusion is testable without a Tauri app
/// handle or a real download.
fn claim_download(flag: &AtomicBool) -> Result<(), String> {
    if flag.swap(true, Ordering::SeqCst) {
        Err("a dictionary download is already running".to_string())
    } else {
        Ok(())
    }
}

/// Release the slot. Called on every exit path, success or failure — a failed
/// download that never released would disable Retry for the rest of the session.
fn release_download(flag: &AtomicBool) {
    flag.store(false, Ordering::SeqCst);
}

/// Whether startup found no dictionary, so the webview should show the download
/// screen rather than the panes.
#[tauri::command]
pub fn needs_dictionary(needs: State<'_, NeedsDictionary>) -> bool {
    needs.0
}

/// Emit a phase label, logging rather than propagating a failed emit.
fn emit_status(app: &AppHandle, status: &str) {
    if let Err(e) = app.emit(DICTIONARY_STATUS_EVENT, status.to_string()) {
        // Nothing to fall back on: if the event cannot reach the webview there
        // is no other channel to report progress through.
        eprintln!("emitting a dictionary-status event failed: {e}");
    }
}

/// Download JMdict if absent, build the index, and publish it to the worker.
///
/// Returns `Err` only when the work could not be *started*; every outcome after
/// that — including failure — arrives as a `dictionary-status` event. Same
/// split as `set_input`, and for the same reason: one path to the screen.
#[tauri::command]
pub async fn download_dictionary(
    app: AppHandle,
    paths: State<'_, DictionaryPaths>,
    index: State<'_, IndexSender>,
    inflight: State<'_, DownloadInFlight>,
) -> Result<(), String> {
    claim_download(&inflight.0)?;

    // Everything needed after the `.await` is cloned out first: `tauri::State`
    // is not `Send`, so holding one across an await point does not compile.
    let flag = Arc::clone(&inflight.0);
    let root = paths.root.clone();
    let source_dir = paths.source_dir.clone();
    let tx = index.0.clone();
    let handle = app.clone();
    let failure_hint = format!(
        " Retry, or place {} in {} and retry.",
        jmdict_source::SOURCE_FILE,
        source_dir.display()
    );

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        emit_status(&handle, "downloading");
        let table = jparser::conjugation::ConjugationTable::load_embedded()
            .map_err(|e| format!("loading the conjugation table failed: {e}"))?;
        let opts = jparser::stem::StemOptions::default();

        // `resolve` reuses an already-downloaded archive before it reaches the
        // network, so a retry after a *build* failure is fast and offline.
        // `ensure_dictionary` runs the closure only when a rebuild is needed.
        let index = jparser::index::ensure_dictionary(
            &root,
            &table,
            &opts,
            jparser::index::generations::DEFAULT_KEEP_GENERATIONS,
            || {
                emit_status(&handle, "building");
                jmdict_source::resolve(&source_dir)
            },
        )
        .map_err(|e| e.to_string())?;

        let hints = match std::env::var_os(crate::state::HINTS_ENV) {
            Some(path) => Some(
                jparser::hints::VibratoTokenizer::load(std::path::Path::new(&path))
                    .map_err(|e| format!("loading the hints dictionary failed: {e}"))?,
            ),
            None => None,
        };

        Ok::<AppState, String>(AppState {
            index,
            table,
            hints,
        })
    })
    .await;

    release_download(&flag);

    let message = match outcome {
        Ok(Ok(state)) => {
            if tx.send(Some(Arc::new(state))).is_err() {
                // The worker is gone, which no retry fixes. Reported like any
                // other failure rather than swallowed.
                "the parse worker is no longer running".to_string()
            } else {
                emit_status(&app, "ready");
                return Ok(());
            }
        }
        // The failure hint names the archive and directory, which is the whole
        // manual fallback: `resolve` serves a hand-placed file without touching
        // the network, so a user behind a proxy can drop it in and press Retry.
        Ok(Err(message)) => format!("{message}.{failure_hint}"),
        Err(e) => format!("the download task failed to run: {e}.{failure_hint}"),
    };

    emit_status(&app, &message);
    Ok(())
}
```

**Verify rather than assume**, reporting either difference: that `jparser::stem::StemOptions` and `jparser::index::generations::DEFAULT_KEEP_GENERATIONS` are the correct paths from this crate (the CLI imports them that way), and that moving `table` into the returned `AppState` after `ensure_dictionary` borrowed it compiles — if the borrow checker objects, load the table once into a local, pass `&table` to `ensure_dictionary`, and move it afterwards, or clone it, and say which you did.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator commands`
Expected: PASS. Report the count.

- [ ] **Step 6: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/commands.rs
cargo clippy --workspace --all-targets -- -D warnings
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
git diff --stat
git add src-tauri/src/commands.rs src-tauri/Cargo.toml Cargo.lock
git commit -m "feat: add the dictionary download command"
```

The purity grep must print **0** — `jmdict-source` joined the shell, not the parser.

---

## Task 4: Wire the index channel into startup

**Files:**
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: everything Tasks 1–3 produced.
- Produces: a `setup` that always spawns the worker and manages the three new state values.

- [ ] **Step 1: Replace the worker spawn and the startup match**

In `src-tauri/src/main.rs`, add to the imports:

```rust
use std::sync::atomic::AtomicBool;
```

Replace Task 1's temporary shim and the whole `match state::load_state(&root)` block with:

```rust
            // The worker is spawned unconditionally, before startup knows
            // whether an index exists. 2E spawned it only on the success branch
            // and dropped `rx` otherwise, which left no live channel for a
            // first-run download's result to arrive on. Starting it always
            // deletes that special case rather than adding one.
            let (index_tx, index_rx) = tokio::sync::watch::channel(None::<Arc<state::AppState>>);
            app.manage(commands::IndexSender(index_tx.clone()));

            let source_dir = config_dir.join(jmdict_source::SOURCE_DIR);
            app.manage(commands::DictionaryPaths {
                root: root.clone(),
                source_dir,
            });
            app.manage(commands::DownloadInFlight(Arc::new(AtomicBool::new(false))));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(parse::run_worker(handle, index_rx, rx));

            // `StartupFailure` and `NeedsDictionary` are both managed on every
            // branch: `Option<State<'_, T>>` is not a valid Tauri 2 command
            // parameter, so a command cannot express "this state may not be
            // managed". Empty string / `false` are the "nothing wrong" values.
            match state::load_state(&root) {
                Ok(s) => {
                    app.manage(state::StartupFailure(String::new()));
                    app.manage(state::NeedsDictionary(false));
                    // A send failure would mean the worker died between its
                    // spawn above and this line, which cannot happen — it is
                    // awaiting its first input. Ignored with that reason rather
                    // than turned into a startup abort.
                    let _ = index_tx.send(Some(Arc::new(s)));
                }
                // The expected first run. Deliberately not a `StartupFailure`:
                // that one disables the parse controls, and this state is
                // fixable from inside the window.
                Err(e) if state::is_missing_dictionary(&e) => {
                    app.manage(state::StartupFailure(String::new()));
                    app.manage(state::NeedsDictionary(true));
                }
                Err(e) => {
                    app.manage(state::StartupFailure(e.to_string()));
                    app.manage(state::NeedsDictionary(false));
                }
            }
```

`app.manage(Arc::clone(&shared))` for `AppState` is **deleted** — nothing consumes it now that `run_worker` receives the index through the channel.

- [ ] **Step 2: Register the new commands**

Add `commands::download_dictionary` and `commands::needs_dictionary` to `invoke_handler`'s `generate_handler!` list, after `commands::frontend_ready`.

- [ ] **Step 3: Verify the gate**

```bash
cargo build -p translation-aggregator
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo +1.85 check -p jparser -p jmdict-source -p xtask
```

Expected: PASS. Report the test count.

**Confirm and report:** that `grep -rn "allow(dead_code)" src-tauri/src/` returns nothing. Every item Tasks 1–3 add is consumed by a later task in this phase, so none should be needed. If clippy demands one, name the item and why rather than leaving it in silently.

- [ ] **Step 4: Format and commit**

```bash
rustfmt --edition 2021 src-tauri/src/main.rs
git diff --stat
git add src-tauri/src
git commit -m "feat: spawn the parse worker before the index exists"
```

`git diff --stat` must not list any file under `crates/`.

---

## Task 5: The download screen

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`, `src/styles/global.css`, `e2e/stub.ts`, `e2e/panes.spec.ts`

**Interfaces:**
- Consumes: `needs_dictionary`, `download_dictionary`, and the `dictionary-status` event (Tasks 3, 4).
- Produces: nothing later in this phase consumes.

- [ ] **Step 1: Write the failing tests**

Extend the `invoke` mock at the top of `src/main.test.ts` so `needs_dictionary` resolves to `true` and `download_dictionary` resolves to `undefined`. Every existing describe block must keep passing — if any of them now render the download screen, give those blocks a mock where `needs_dictionary` is `false`.

Then add:

```ts
describe('the first-run download screen', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    vi.resetModules();
    await import('./main');
    await Promise.resolve();
    await Promise.resolve();
  });

  test('offers a download when no dictionary is present', () => {
    expect(document.querySelector('#download')).not.toBeNull();
  });

  test('a status event replaces the button with the phase', () => {
    emit('dictionary-status', 'building');
    expect(document.querySelector('#dictionary')?.textContent).toContain('Building');
    expect(document.querySelector('#download')).toBeNull();
  });

  test('ready clears the screen and re-enables the controls', () => {
    emit('dictionary-status', 'ready');
    expect(document.querySelector('#dictionary')?.childElementCount).toBe(0);
    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
  });

  // A failure must leave a working Retry, or the user relaunches for a problem
  // that reconnecting to wifi would have fixed.
  test('a failure shows the reason and a retry', () => {
    emit('dictionary-status', 'could not reach the server');
    expect(document.querySelector('#dictionary')?.textContent).toContain('could not reach');
    expect(document.querySelector('#download')?.textContent).toBe('Retry');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts`
Expected: FAIL — `#download` is null; there is no screen yet.

- [ ] **Step 3: Implement the screen**

In `src/main.ts`, add `<div id="dictionary"></div>` as the first child of `.panes`, before `<div id="output"></div>`, then add:

```ts
const dictionary = app.querySelector<HTMLElement>('#dictionary')!;

// The backend's own phase labels. Anything not in this map is an error message
// to show verbatim, which is why this is a lookup rather than an enum — the
// failure arm carries text the user needs to read.
const PHASE_LABELS: Record<string, string> = {
  downloading: 'Downloading dictionary…',
  building: 'Building index…',
};

function renderDictionary(status: string | null): void {
  if (status === 'ready') {
    dictionary.replaceChildren();
    input.disabled = false;
    parseButton.disabled = false;
    return;
  }

  const el = document.createElement('div');
  el.className = 'dictionary';
  const phase = status === null ? undefined : PHASE_LABELS[status];

  if (phase !== undefined) {
    const label = document.createElement('p');
    label.textContent = phase;
    const spinner = document.createElement('div');
    spinner.className = 'spinner';
    // Decorative: the label above already carries the information, and under
    // `prefers-reduced-motion` this stops moving entirely.
    spinner.setAttribute('aria-hidden', 'true');
    el.replaceChildren(label, spinner);
    dictionary.replaceChildren(el);
    return;
  }

  // Idle or failed. `null` is the first-run offer; anything else is a message
  // from the backend, which already names the archive and directory to drop in.
  const message = document.createElement('p');
  message.textContent =
    status === null
      ? 'No dictionary yet. JMdict is a one-time download of roughly ten megabytes from EDRDG.'
      : status;

  const button = document.createElement('button');
  button.id = 'download';
  button.type = 'button';
  button.textContent = status === null ? 'Download dictionary' : 'Retry';

  // Closure-local, not `button.disabled`: disabling a focused element blurs it
  // and drops it from the tab order, which nothing here restores. Same guard
  // the header toggles use.
  let pending = false;
  button.addEventListener('click', () => {
    if (pending) return;
    pending = true;
    renderDictionary('downloading');
    void invoke('download_dictionary')
      .catch((e) => renderDictionary(String(e)))
      .finally(() => {
        pending = false;
      });
  });

  el.replaceChildren(message, button);
  dictionary.replaceChildren(el);
}

// Exported for the same reason `showStartupError` is: a test can await it
// directly rather than racing the fire-and-forget call at the bottom.
export async function showDictionaryScreen(): Promise<void> {
  if (!(await invoke<boolean>('needs_dictionary'))) return;
  // Parsing cannot succeed until an index exists. Unlike `showStartupError`'s
  // disabling, this is reversible — `ready` turns both back on.
  input.disabled = true;
  parseButton.disabled = true;
  renderDictionary(null);
}
```

Register the listener alongside the existing two, inside the same `Promise.all`:

```ts
  listen<string>('dictionary-status', (e) => renderDictionary(e.payload)),
```

And call it at the bottom, beside the other fire-and-forget calls:

```ts
void showDictionaryScreen();
```

- [ ] **Step 4: Add the styles**

In `src/styles/global.css`:

```css
.dictionary {
  padding: var(--space-pane);
  font-family: var(--font-ui);
  font-size: var(--text-tag);
  color: var(--color-muted);
}

.dictionary button {
  font: inherit;
  color: var(--color-text);
  background: none;
  border: 1px solid var(--color-text);
  border-radius: 3px;
  padding: 4px 10px;
  margin-top: 8px;
  cursor: pointer;
}

.spinner {
  width: 12px;
  height: 12px;
  margin-top: 8px;
  border: 2px solid var(--color-rule);
  border-top-color: var(--color-text);
  border-radius: 50%;
  animation: spin 700ms linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
```

Add to the existing `@media (prefers-reduced-motion: reduce)` block:

```css
  .spinner { animation: none; }
```

Only `transform` is animated, and reduced motion stops it — the phase label still carries the information, so nothing is lost.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run`
Expected: PASS. Report the count.

- [ ] **Step 6: Update the Playwright stub and specs**

In `e2e/stub.ts`, make `invoke` answer the new commands: `needs_dictionary` returns `false` — every existing spec exercises the parse path and must keep doing so — and `download_dictionary` returns `undefined`.

Add one spec to `e2e/panes.spec.ts` that overrides `needs_dictionary` to `true`, asserts `#download` is visible, then fires `dictionary-status` with `ready` through `__TA_EMIT__` and asserts `#dictionary` is empty. Follow the file's existing conventions rather than inventing new ones.

- [ ] **Step 7: Confirm the baselines did NOT change**

With `needs_dictionary` stubbed `false`, `#dictionary` renders empty, so the six committed baselines must be **unchanged**. Run `npx playwright test` and confirm no screenshot mismatch.

**If any baseline changes, stop and report** — it means an empty `#dictionary` div is affecting layout, which it must not. Do not regenerate to make a mismatch go away.

If you add a screenshot for the download screen itself, open the written PNG with the Read tool and describe what you saw. Do not accept a baseline you have not looked at.

- [ ] **Step 8: Run the full gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
npx playwright test
CI=1 npx playwright test
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p jparser --features mecab --all-targets -- -D warnings
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
cargo +1.85 check -p jparser -p jmdict-source -p xtask
cargo llvm-cov -p jparser --features mecab --summary-only --fail-under-lines 80
```

The purity grep must print **0**. Report `src-tauri`'s coverage number (`cargo llvm-cov -p translation-aggregator --summary-only`); there is **no gate on it** and you must not add tests solely to move it.

- [ ] **Step 9: Verify by hand what no test can**

**Required, not optional** (spec §6). On a clean profile:

1. Delete the app config directory entirely.
2. `npm run tauri dev`.
3. Confirm the download screen appears rather than a fatal error.
4. Press **Download dictionary**. Confirm the phase label changes and the screen clears when it finishes.
5. Copy Japanese text. Confirm it parses **without restarting the app** — this is the phase's headline claim.
6. Disconnect the network, delete the config dir again, relaunch, press Download. Confirm the failure names a reason and a path, and that **Retry** works once the network is back.

**Report exactly what you observed.** If something cannot be verified in your environment, say so plainly rather than claiming it. 2E's Critical defect — the entire event architecture dead behind 340 green Rust tests — was found only because this step was attempted honestly.

- [ ] **Step 10: Commit**

```bash
git diff --stat
git add src e2e
git commit -m "feat: add the first-run dictionary download screen"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 user-initiated download | 3 (command), 5 (button) |
| §1 startup distinguishes no-dictionary from broken | 2, 4 |
| §1 worker survives no index, no restart | 1, 4 |
| §1 three screen states | 5 |
| §1 retry with a manual fallback | 3 (cached-file reuse + failure hint), 5 (Retry) |
| §1.1 downloader reused, not rebuilt | 3 |
| §2 `AppState` leaves managed state | 1, 4 |
| §2 worker spawned unconditionally | 4 |
| §2.1 third startup outcome | 2, 4 |
| §2.2 `download_dictionary` on `spawn_blocking`, `dictionary-status` event | 3 |
| §2.2 `Result<(), String>` means "could not start" only | 3 |
| §2.2 concurrent call rejected | 3 (Steps 2, 4) |
| §2.2 source dir is a sibling of `dict/` | 4 |
| §2.3 `jmdict-source` added to the shell | 3 |
| §3 three states; `pending` guard, not `disabled` | 5 |
| §3 spinner is `transform`-only and reduced-motion safe | 5 (Step 4) |
| §4 data flow; poll runs throughout | 1 (skip guard), 4 |
| §5 failure names the path | 3 (`failure_hint`) |
| §5 no new capability | Global Constraints, Resolved facts |
| §6 seam tests, not I/O | 1, 2, 3 |
| §6 manual first run required | 5 Step 9 |
| §7 invariants | Global Constraints |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Two steps direct the implementer to *verify and report* rather than guess — Task 3 Step 4 (the `StemOptions` / `DEFAULT_KEEP_GENERATIONS` paths, and whether `table` can move into `AppState` after being borrowed) and Task 4 Step 3 (`allow(dead_code)`) — each naming the exact uncertainty.

**3. Type consistency across task boundaries.** Checked:

- `watch::Receiver<Option<Arc<AppState>>>` is what `run_worker` takes and `current_index` borrows (Task 1), and the receiver half of `index_tx` (Task 4) — match.
- `IndexSender(watch::Sender<Option<Arc<AppState>>>)` is defined in Task 3, managed in Task 4, and its `.0` is cloned inside `download_dictionary` (Task 3) — match.
- `is_missing_dictionary(&StartupError) -> bool` (Task 2) is called in Task 4's match guard — match.
- `NeedsDictionary(pub bool)` (Task 2) is managed in Task 4 and read by `needs_dictionary` (Task 3), which Task 5 invokes expecting `boolean` — match.
- `DICTIONARY_STATUS_EVENT` is `"dictionary-status"` (Task 3) and is the string Task 5 listens for — match.
- The phase labels `"downloading"` / `"building"` / `"ready"` are emitted in Task 3 and keyed in Task 5's `PHASE_LABELS` and its `'ready'` branch — match.
- `DictionaryPaths { root, source_dir }` is constructed in Task 4 from `state::resolve_dict_root(&config_dir)` and `config_dir.join(jmdict_source::SOURCE_DIR)`, and read in Task 3 — match.

**4. Residual risks a human should look at.**

- **Task 1 leaves the tree needing a shim.** Its `main.rs` call-site change is temporary and Task 4 replaces it. Reviewers should expect it and not score it as scope creep.
- **`table` is moved into `AppState` after `ensure_dictionary` borrowed it** in Task 3's closure. This should compile — the borrow ends when `ensure_dictionary` returns — but it is the one line most likely to need a small rearrangement, which is why Step 4 asks for it to be verified and reported rather than assumed.
- **The empty `#dictionary` div must not shift layout.** Task 5 Step 7 makes this a check rather than an assumption, because six committed baselines depend on it, and the instruction is explicitly to stop rather than regenerate.
- **`run_worker` now drops input silently while the index is absent.** Deliberate — the download screen is on top — but it means a `set_input` during that window returns `Ok` and produces nothing.
- **The download is not cancellable.** `spawn_blocking` cannot be interrupted, so a user who starts a download on a slow connection waits it out or quits the app. Consistent with how the parse worker already treats `spawn_blocking`, and out of scope here, but it is the most likely first complaint.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-14-jparser-phase2f.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
