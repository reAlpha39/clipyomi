# JParser Phase 2E — Clipboard Monitoring and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the app autonomous — it watches the clipboard, parses what it finds, and remembers whether you wanted it to.

**Architecture:** A 200 ms clipboard poll and the manual text box both feed one `tokio::sync::watch` channel. A single worker awaits that channel, parses on `spawn_blocking`, and emits `parse-result` to the webview. Latest-wins falls out of `watch` retaining only its newest value. Two toggles — always-on-top and clipboard pause — persist to `settings.json`.

**Tech Stack:** Rust 2021 / `src-tauri` MSRV 1.88, `tauri 2`, `tauri-plugin-clipboard-manager 2.3.2`, `tokio` (sync/macros/rt/time), Vite 8, TypeScript 7, Vitest 4, Playwright 1.62. No frontend framework.

**Reference:** `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` (authoritative), with `docs/superpowers/specs/2026-08-12-jparser-port-design.md` §6/§7/§8/§9 for the shell's module split, UI direction, settings, and error policy. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **License GPL v2.** Every new Rust source file gets the standard header comment, copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **`src-tauri` MSRV is 1.88**, pinned separately from the workspace's 1.85. The MSRV gate is `cargo +1.85 check -p jparser -p jmdict-source -p xtask` — **never `--workspace`**, which fails inside Tauri's transitive tree by design.
- **`crates/jparser` gains nothing this phase.** No Tauri dependency, no new I/O, no edits at all. The parser does not know a clipboard exists.
- **`mecab` stays off by default in `jparser`.** The purity gate `cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"` must keep returning **0**.
- **Errors are explicit:** no `unwrap()`, `expect()`, or `unreachable!()` in library or binary code outside `#[cfg(test)]`. The `.expect` closing `main` is the one documented exception. Never swallow an error without a comment naming the reason.
- **Names are frozen.** Commands: `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`. Events: `parse-result`, `parse-error`. Settings file `settings.json`; keys `always_on_top` (default `false`) and `clipboard_monitoring` (default `true`).
- **No profile may set `panic = "abort"`.** `catch_unwind` catches nothing under an aborting profile and the panic containment silently disappears.
- **File size** 200–400 lines typical, **800 hard maximum including tests**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never `cargo fmt`, and never on a crate-root file** — `src-tauri/src/main.rs` and `crates/jparser/src/lib.rs` cascade into every `mod`-reachable file. After formatting run `git diff --stat` and `git checkout --` anything unintended.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` **and** `cargo clippy -p jparser --features mecab --all-targets -- -D warnings` clean at the end of every task.
- **`crates/jparser/src/segment.rs` is at 778/800 lines and must not be edited.**
- **No frontend framework** — vanilla TypeScript and DOM APIs only.
- **Every colour defined on bare `:root` first**; media queries and `[data-theme]` may only redefine. Only `transform` and `opacity` animated; the `prefers-reduced-motion` block must keep working.
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`.**

**Invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format; a published `gen-N` is immutable; directory knowledge lives only in `generations.rs` and `ensure_dictionary`; the eight serialized `WordFlags` names are public API; `src-tauri`'s empty-string `StartupFailure` sentinel means "startup succeeded", so every `StartupError` variant must keep rendering non-empty.

---

## Resolved facts — do not re-derive these

Measured 2026-08-14 against the tree at commit `772857b`.

| Fact | Value |
|---|---|
| `tauri::async_runtime` re-exports | `mpsc::{channel, Receiver, Sender}`, `Mutex`, `RwLock` — **not `watch`** |
| Therefore | add `tokio` to `src-tauri` directly; its module doc says to do exactly this |
| `tokio` already in tree | 1.53.1 via `tauri` — a direct dependency adds no transitive weight |
| Clipboard crate | `tauri-plugin-clipboard-manager` **2.3.2**, MIT OR Apache-2.0, taken under **MIT** |
| Panic strategy | No `[profile]` overrides in either manifest → `panic = "unwind"` is in force, so `catch_unwind` works |
| `catch_unwind` ergonomics | Requires `UnwindSafe`; `&Index`/`&ConjugationTable` will not satisfy it. Wrap in `AssertUnwindSafe` — sound here because managed state is read-only after startup and no `&mut` crosses the boundary |
| `serde_json::Value` | is `PartialEq` but **not `Eq`** (it holds `f64`), so `Settings` derives `PartialEq` only |
| Baseline tests | **317 passed / 1 ignored**; **16** Vitest; **10** Playwright local and CI-simulated |
| Current sizes | `state.rs` 194, `commands.rs` 138, `main.rs` 57, `main.ts` 83, `main.test.ts` 69, `global.css` 136 |
| 2D contract to migrate | `parse_text(text) -> Result<ParseResult, String>`, consumed by `src/main.ts` and `src/main.test.ts` |
| Existing state | `AppState { index, table, hints }` managed as `Arc<AppState>`; `StartupFailure(String)` managed unconditionally, empty string = success |

**Always-on-top is deliberately not tested automatically** (spec §7.1, port design §10). It is one call into Tauri's window API; what could break is the window manager's response, which no assertion here can observe. Verify by hand and say so in your report — do not invent a test to fill the gap.

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/settings.rs` | *(new)* `Settings`, load/save, defaults, unknown-key preservation |
| `src-tauri/src/parse.rs` | *(new)* `catch_parse`, `next_input`, the worker loop |
| `src-tauri/src/clipboard.rs` | *(new)* `should_parse`, `is_japanese`, the 200 ms poll |
| `src-tauri/src/state.rs` | *(modified)* `SettingsState` |
| `src-tauri/src/commands.rs` | *(modified)* `set_input`, both toggles, `get_settings`; `parse_text` removed |
| `src-tauri/src/main.rs` | *(modified)* load settings, spawn poll and worker, register plugin |
| `src-tauri/Cargo.toml` | *(modified)* `tokio`, `tauri-plugin-clipboard-manager` |
| `src/main.ts` | *(modified)* render from events; header controls |
| `src/main.test.ts` | *(modified)* event-path and control tests |
| `src/styles/global.css` | *(modified)* four-row grid, `.controls` |
| `e2e/stub.ts`, `e2e/panes.spec.ts` | *(modified)* stub `listen`; header assertions |

---

## Task 1: Settings

**Files:**
- Create: `src-tauri/src/settings.rs`
- Modify: `src-tauri/src/main.rs` (add `mod settings;` only)

**Interfaces:**
- Consumes: nothing.
- Produces: `Settings { always_on_top: bool, clipboard_monitoring: bool, extra: serde_json::Map<String, Value> }`, `Settings::default()`, `load(&Path) -> (Settings, Option<String>)`, `save(&Path, &Settings) -> Result<(), SettingsError>`, `settings_path(&Path) -> PathBuf`. Tasks 4 and 6 consume all of these.

Fully testable with no Tauri runtime. Start here.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/settings.rs` with the GPL header copied verbatim from `crates/jparser/src/index/mod.rs:1-6`, then the module doc and **only** this test module:

```rust
//! Persisted user settings.
//!
//! Two keys today, both toggles the header owns. Unknown keys survive a rewrite
//! so a file written by a later version is not silently truncated by this one.

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-settings-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Monitoring on, always-on-top off: the app's headline behaviour works on
    /// first launch, and the window does not seize the foreground uninvited.
    #[test]
    fn defaults_are_monitoring_on_and_always_on_top_off() {
        let s = Settings::default();
        assert!(s.clipboard_monitoring);
        assert!(!s.always_on_top);
    }

    /// First run is not an error state.
    #[test]
    fn a_missing_file_loads_defaults_without_a_warning() {
        let path = scratch("missing").join("settings.json");
        let (s, warning) = load(&path);
        assert_eq!(s, Settings::default());
        assert!(warning.is_none(), "got {warning:?}");
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let path = scratch("round-trip").join("settings.json");
        let mut written = Settings::default();
        written.always_on_top = true;
        written.clipboard_monitoring = false;

        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        assert_eq!(read_back, written);
        assert!(warning.is_none(), "got {warning:?}");
    }

    /// A corrupt file must not stop the app launching — but the user has to be
    /// told, or their settings appear to silently reset themselves.
    #[test]
    fn a_corrupt_file_loads_defaults_and_reports_why() {
        let path = scratch("corrupt").join("settings.json");
        std::fs::write(&path, b"{ not json").expect("write");

        let (s, warning) = load(&path);
        assert_eq!(s, Settings::default());
        let warning = warning.expect("a corrupt file must report a reason");
        assert!(!warning.is_empty());
    }

    /// Phases 3 and 4 add many keys. A downgrade that drops them is data loss
    /// that would surface long after the downgrade.
    #[test]
    fn unknown_keys_survive_a_rewrite() {
        let path = scratch("unknown").join("settings.json");
        std::fs::write(
            &path,
            br#"{"always_on_top":true,"clipboard_monitoring":true,"furigana_mode":"all","font_size":18}"#,
        )
        .expect("write");

        let (loaded, _) = load(&path);
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(json["furigana_mode"], "all", "unknown string key dropped");
        assert_eq!(json["font_size"], 18, "unknown number key dropped");
        assert_eq!(json["always_on_top"], true, "known key lost");
    }

    #[test]
    fn the_settings_path_is_a_sibling_of_the_dict_directory() {
        let root = std::path::Path::new("/tmp/cfg");
        assert_eq!(settings_path(root), root.join("settings.json"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator settings`

Expected: FAIL to compile — `cannot find type 'Settings'`, `cannot find function 'load'`, and `file not found for module 'settings'` until you add `mod settings;` to `main.rs`. Add that declaration now; leave everything else failing. That is the intended RED.

- [ ] **Step 3: Implement the settings module**

Insert into `src-tauri/src/settings.rs`, above the test module:

```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Filename inside the app config dir. A sibling of `dict/`, never inside it —
/// a published generation directory is immutable.
pub const SETTINGS_FILE: &str = "settings.json";

fn default_true() -> bool {
    true
}

/// `Eq` is not derived: `extra` holds `serde_json::Value`, which is `PartialEq`
/// but not `Eq` because it can hold an `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
    /// Keys this version does not know about, carried through on rewrite so a
    /// file written by a later version is not truncated by this one.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            clipboard_monitoring: true,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("writing {path} failed: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("encoding settings failed: {0}")]
    Encode(#[from] serde_json::Error),
}

/// The settings file inside an app config directory.
pub fn settings_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(SETTINGS_FILE)
}

/// Load settings, falling back to defaults.
///
/// Returns the settings and, when something was wrong with the file, a reason to
/// show the user. A missing file is not wrong — it is first run — so it reports
/// no reason. Never fails: settings are not important enough to block launch.
pub fn load(path: &Path) -> (Settings, Option<String>) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        // Any read failure is treated as absent. Distinguishing "not found" from
        // "unreadable" would change nothing: both mean we start from defaults.
        Err(_) => return (Settings::default(), None),
    };

    match serde_json::from_str(&raw) {
        Ok(settings) => (settings, None),
        Err(e) => (
            Settings::default(),
            Some(format!(
                "{} could not be read, using defaults: {e}",
                path.display()
            )),
        ),
    }
}

/// Write settings, creating the parent directory if needed.
pub fn save(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| SettingsError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let encoded = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, encoded).map_err(|source| SettingsError::Write {
        path: path.to_path_buf(),
        source,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator settings`
Expected: PASS, 6 tests.

- [ ] **Step 5: Prove the unknown-key preservation is load-bearing**

Temporarily delete the `#[serde(flatten)] pub extra: …` field and its initializer, and re-run `cargo test -p translation-aggregator settings`. `unknown_keys_survive_a_rewrite` must fail. Restore, re-run to green, and record both outputs in your report.

A preservation guarantee that cannot be caught failing is not a guarantee.

- [ ] **Step 6: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/settings.rs
cargo clippy --workspace --all-targets -- -D warnings
git diff --stat
git add src-tauri/src/settings.rs src-tauri/src/main.rs
git commit -m "feat: add persisted settings with unknown-key preservation"
```

`git diff --stat` must show only those two files.

---

## Task 2: The parse worker

**Files:**
- Create: `src-tauri/src/parse.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/main.rs` (add `mod parse;` only)

**Interfaces:**
- Consumes: `AppState` from `state.rs`.
- Produces: `catch_parse(usize, F) -> Result<ParseResult, String>`, `next_input(&mut watch::Receiver<String>) -> Option<String>`, `run_worker(AppHandle, Arc<AppState>, watch::Receiver<String>)`, and the constants `PARSE_RESULT_EVENT` = `"parse-result"` / `PARSE_ERROR_EVENT` = `"parse-error"`. Tasks 4 and 5 consume these.

- [ ] **Step 1: Add the tokio dependency**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
# `tauri::async_runtime` re-exports mpsc, Mutex, and RwLock but not `watch`, and
# its own module doc says to use tokio directly when what you need is missing.
# tokio 1.53 is already in the tree via tauri, so this adds no transitive weight.
# `macros` and `rt` are for `#[tokio::test]`; `time` is for the poll's sleep.
tokio = { version = "1", features = ["sync", "macros", "rt", "time"] }
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/parse.rs` with the GPL header, then the module doc and **only** this test module:

```rust
//! The parse worker: one task, latest-wins, panics contained.

#[cfg(test)]
mod tests {
    use super::*;

    /// The port design's rule is "stop and rerun", not "queue": when text
    /// arrives faster than parsing, the worker must skip to the newest rather
    /// than working through a backlog the user can no longer see.
    #[tokio::test]
    async fn the_worker_sees_only_the_newest_input() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        tx.send("one".to_string()).expect("send");
        tx.send("two".to_string()).expect("send");
        tx.send("three".to_string()).expect("send");

        assert_eq!(next_input(&mut rx).await.as_deref(), Some("three"));
    }

    /// A dropped sender ends the worker rather than spinning.
    #[tokio::test]
    async fn a_closed_channel_ends_the_worker() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        drop(tx);
        assert_eq!(next_input(&mut rx).await, None);
    }

    #[test]
    fn catch_parse_returns_the_value_when_nothing_panics() {
        let out = catch_parse(3, || Ok(ParseResult { segments: vec![] })).expect("ok");
        assert!(out.segments.is_empty());
    }

    /// The matcher does offset arithmetic over the whole input. A panic there
    /// must not take the app down — the previous result stays on screen and the
    /// worker lives to parse the next copy.
    #[test]
    fn catch_parse_contains_a_panic_and_names_the_input_length() {
        let err = catch_parse(4096, || panic!("offset out of range")).expect_err("must be Err");
        assert!(err.contains("4096"), "got {err}");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Add `mod parse;` to `src-tauri/src/main.rs`, then run: `cargo test -p translation-aggregator parse`

Expected: FAIL to compile — `cannot find function 'next_input'`, `cannot find function 'catch_parse'`. That is the intended RED.

- [ ] **Step 4: Implement the worker**

Insert into `src-tauri/src/parse.rs`, above the test module:

```rust
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use jparser::{BoundaryHints, ParseOptions, ParseResult};
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

use crate::state::AppState;

/// Emitted with a `ParseResult` payload whenever a parse succeeds.
pub const PARSE_RESULT_EVENT: &str = "parse-result";
/// Emitted with a `String` payload when a parse fails or panics.
pub const PARSE_ERROR_EVENT: &str = "parse-error";

/// Run a parse, converting a panic into an error rather than an abort.
///
/// `AssertUnwindSafe` is sound here: the managed state is read-only after
/// startup and no `&mut` crosses this boundary, so there is no invariant a
/// half-completed parse could leave broken.
pub fn catch_parse<F>(input_len: usize, f: F) -> Result<ParseResult, String>
where
    F: FnOnce() -> Result<ParseResult, String>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => Err(format!(
            "the parser panicked on an input of {input_len} characters; \
             the previous result is still shown"
        )),
    }
}

/// Wait for new input and return the newest value, skipping anything superseded.
///
/// `borrow_and_update` is what makes this latest-wins: a `watch` channel keeps
/// only its most recent value, so inputs that arrived during a parse are already
/// collapsed by the time we look.
pub async fn next_input(rx: &mut watch::Receiver<String>) -> Option<String> {
    rx.changed().await.ok()?;
    let text = rx.borrow_and_update().clone();
    Some(text)
}

/// Parse each new input and emit the outcome to the webview.
pub async fn run_worker(app: AppHandle, state: Arc<AppState>, mut rx: watch::Receiver<String>) {
    while let Some(text) = next_input(&mut rx).await {
        let state = Arc::clone(&state);
        let len = text.chars().count();

        let outcome = tauri::async_runtime::spawn_blocking(move || {
            catch_parse(len, || {
                let flags = state.hints.as_ref().map(|t| t.hints(&text));
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

        let emitted = match outcome {
            Ok(Ok(result)) => app.emit(PARSE_RESULT_EVENT, result),
            Ok(Err(message)) => app.emit(PARSE_ERROR_EVENT, message),
            Err(e) => app.emit(
                PARSE_ERROR_EVENT,
                format!("the parse task failed to run: {e}"),
            ),
        };

        if let Err(e) = emitted {
            // Nothing to fall back on: if the event cannot reach the webview
            // there is no other channel to report it through. Log and continue
            // so one failed emit does not end monitoring.
            eprintln!("emitting a parse event failed: {e}");
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator parse`
Expected: PASS, 4 tests.

**Verify rather than assume**, reporting either difference: that `tauri::Emitter` is the trait providing `AppHandle::emit` in Tauri 2.11 (in some versions it is inherent, in others the trait must be in scope), and that `spawn_blocking`'s `JoinHandle` error formats with `{e}`.

- [ ] **Step 6: Prove the panic containment is load-bearing**

Temporarily replace `catch_unwind(AssertUnwindSafe(f))` with a direct `Ok(f())` call and re-run `cargo test -p translation-aggregator parse`. `catch_parse_contains_a_panic_and_names_the_input_length` must fail by panicking rather than returning `Err`. Restore, re-run to green, and record both outputs.

- [ ] **Step 7: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/parse.rs
cargo clippy --workspace --all-targets -- -D warnings
git diff --stat
git add src-tauri/src/parse.rs src-tauri/src/main.rs src-tauri/Cargo.toml Cargo.lock
git commit -m "feat: add the latest-wins parse worker with panic containment"
```

---

## Task 3: The clipboard poll

**Files:**
- Create: `src-tauri/src/clipboard.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/src/main.rs` (add `mod clipboard;` only)

**Interfaces:**
- Consumes: `SettingsState` (Task 4 defines it — see Step 4's note), the `watch::Sender<String>` half (Task 2).
- Produces: `should_parse(&str, Option<&str>, Option<&str>) -> bool`, `MAX_INPUT_CHARS`, `run_poll(AppHandle, watch::Sender<String>, Arc<SettingsState>)`. Task 4 spawns `run_poll`.

- [ ] **Step 1: Add the clipboard plugin**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
# MIT OR Apache-2.0, taken under MIT: Apache-2.0 alone is incompatible with this
# project's GPL-2.0-only.
tauri-plugin-clipboard-manager = "2.3.2"
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/clipboard.rs` with the GPL header, then the module doc and **only** this test module:

```rust
//! The clipboard poll.
//!
//! The decision of whether a copy is worth parsing lives in `should_parse`,
//! which is pure. The loop around it is a thin shell — the system clipboard is
//! global state shared with the developer's own desktop, so no test touches it.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn japanese_text_is_worth_parsing() {
        assert!(should_parse("東京は", None, None));
        assert!(should_parse("とうきょう", None, None));
        assert!(should_parse("トウキョウ", None, None));
    }

    /// Every poll tick re-reads the clipboard; without this the parser would
    /// re-run five times a second on text that has not changed.
    #[test]
    fn unchanged_text_is_skipped() {
        assert!(!should_parse("東京", Some("東京"), None));
    }

    /// ta-old's rule. English, code, and URLs pass through the clipboard
    /// constantly and none of them are worth a parse.
    #[test]
    fn text_with_no_japanese_is_skipped() {
        assert!(!should_parse("hello world", None, None));
        assert!(!should_parse("https://example.com/path?q=1", None, None));
        assert!(!should_parse("", None, None));
    }

    /// Copying an entry out of our own definition pane must not trigger a
    /// re-parse of our own output.
    #[test]
    fn text_this_app_wrote_is_skipped() {
        assert!(!should_parse("東京", None, Some("東京")));
    }

    /// A soundness guard, not a performance one: the matcher does offset
    /// arithmetic over the whole input.
    #[test]
    fn text_over_the_cap_is_skipped() {
        let long: String = "あ".repeat(MAX_INPUT_CHARS + 1);
        assert!(!should_parse(&long, None, None));
    }

    #[test]
    fn text_exactly_at_the_cap_is_parsed() {
        let exact: String = "あ".repeat(MAX_INPUT_CHARS);
        assert!(should_parse(&exact, None, None));
    }

    /// The cap counts characters, not bytes: Japanese is three bytes a
    /// character in UTF-8, so a byte cap would reject a third of the intended
    /// length.
    #[test]
    fn the_cap_counts_characters_not_bytes() {
        let text: String = "あ".repeat(4000);
        assert!(
            text.len() > MAX_INPUT_CHARS,
            "precondition: byte length exceeds the cap"
        );
        assert!(should_parse(&text, None, None));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Add `mod clipboard;` to `src-tauri/src/main.rs`, then run: `cargo test -p translation-aggregator clipboard`

Expected: FAIL to compile — `cannot find function 'should_parse'`, `cannot find value 'MAX_INPUT_CHARS'`. That is the intended RED.

- [ ] **Step 4: Implement the predicate and the poll**

Insert into `src-tauri/src/clipboard.rs`, above the test module:

```rust
use std::sync::Arc;
use std::time::Duration;

use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::watch;

use crate::state::SettingsState;

/// Poll interval. Port design §6.
const POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Longest input worth parsing, in characters.
///
/// A soundness guard rather than a performance one: the matcher does offset
/// arithmetic over the whole input, and an unbounded paste is the cheapest way
/// to find out what that costs.
pub const MAX_INPUT_CHARS: usize = 10_000;

/// Kana or a CJK ideograph — ta-old's test for "is this worth parsing".
fn is_japanese(c: char) -> bool {
    matches!(c,
        '\u{3040}'..='\u{309F}'   // hiragana
        | '\u{30A0}'..='\u{30FF}' // katakana
        | '\u{4E00}'..='\u{9FFF}' // CJK unified ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK unified ideographs extension A
    )
}

/// Whether a clipboard value should be parsed.
///
/// `last_written` is always `None` in this phase — nothing writes to the
/// clipboard yet. It is in the signature because this predicate is the thing
/// under test, and adding a parameter later would invalidate those tests.
pub fn should_parse(text: &str, last_seen: Option<&str>, last_written: Option<&str>) -> bool {
    if last_seen == Some(text) || last_written == Some(text) {
        return false;
    }
    let mut chars = 0usize;
    let mut has_japanese = false;
    for c in text.chars() {
        chars += 1;
        if chars > MAX_INPUT_CHARS {
            return false;
        }
        has_japanese |= is_japanese(c);
    }
    has_japanese
}

/// Poll the clipboard and push anything worth parsing into the input channel.
///
/// Runs for the life of the app. Pausing is a settings flag rather than a
/// stopped task: restarting a task on every toggle is more moving parts than
/// checking a bool five times a second.
pub async fn run_poll(app: AppHandle, tx: watch::Sender<String>, settings: Arc<SettingsState>) {
    let mut last_seen: Option<String> = None;

    loop {
        tokio::time::sleep(POLL_INTERVAL).await;

        if !settings.monitoring_enabled() {
            continue;
        }

        // A read can fail transiently when another app holds the clipboard.
        // Skipping the tick is the whole policy: a poll failure is not
        // information the user can act on (port design §9).
        let Ok(text) = app.clipboard().read_text() else {
            continue;
        };

        if !should_parse(&text, last_seen.as_deref(), None) {
            continue;
        }

        last_seen = Some(text.clone());
        if tx.send(text).is_err() {
            // The worker is gone; nothing left to poll for.
            return;
        }
    }
}
```

**`SettingsState` does not exist until Task 4.** Write `run_poll` now anyway — the compile error is expected and Task 4 resolves it. If you prefer a green build at the end of this task, move `SettingsState` into `state.rs` now using Task 4 Step 3's code verbatim, and say in your report that you pulled it forward.

**Verify rather than assume**, reporting either difference: that `ClipboardExt` is the trait giving `app.clipboard()` in `tauri-plugin-clipboard-manager` 2.3.2, and that `read_text()` returns `Result<String, _>` rather than `Result<Option<String>, _>`. Read the crate's docs.rs page or the vendored source under `~/.cargo/registry/src`; adjust the `let Ok(text)` binding if it is `Option`-shaped.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator clipboard`
Expected: PASS, 7 tests.

- [ ] **Step 6: Format, gate, and commit**

```bash
rustfmt --edition 2021 src-tauri/src/clipboard.rs
cargo clippy --workspace --all-targets -- -D warnings
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
git diff --stat
git add src-tauri/src Cargo.lock src-tauri/Cargo.toml
git commit -m "feat: add the clipboard poll and its skip predicate"
```

The purity grep must print **0**.

---

## Task 4: Wiring — commands, shared state, startup

**Files:**
- Modify: `src-tauri/src/state.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: everything Tasks 1–3 produced.
- Produces: `SettingsState::new(PathBuf, Settings)` with `monitoring_enabled() -> bool`, `snapshot() -> Settings`, and `update<F: FnOnce(&mut Settings)>(F) -> Result<(), SettingsError>`; `InputSender(pub watch::Sender<String>)`; the four commands `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`. Tasks 5 and 6 call all four from the webview.

**This task removes `parse_text`.** That is the 2D contract migration the spec's §4.1 budgets for. The frontend still calls `parse_text` until Task 5, so `npm run build` keeps working but the app's parse button does not — that is expected between these two tasks and is why they are adjacent.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/state.rs`'s existing `#[cfg(test)] mod tests` (it already has a `scratch` helper — reuse it):

```rust
    #[test]
    fn settings_state_reports_the_monitoring_flag() {
        let dir = scratch("monitoring");
        let state = SettingsState::new(dir.join("settings.json"), Settings::default());
        assert!(state.monitoring_enabled(), "default is monitoring on");

        state.update(|s| s.clipboard_monitoring = false).expect("update");
        assert!(!state.monitoring_enabled());
    }

    /// The toggle has to outlive the process, or it is not a setting.
    #[test]
    fn updating_settings_writes_them_to_disk() {
        let dir = scratch("persist");
        let path = dir.join("settings.json");
        let state = SettingsState::new(path.clone(), Settings::default());

        state.update(|s| s.always_on_top = true).expect("update");

        let (reloaded, warning) = crate::settings::load(&path);
        assert!(reloaded.always_on_top, "not persisted");
        assert!(warning.is_none(), "got {warning:?}");
    }

    /// A read-only config dir must not make the toggles stop working in the
    /// running session — the in-memory value still changes.
    #[test]
    fn a_failed_write_still_updates_the_in_memory_value() {
        // A path whose parent is a file, not a directory: create_dir_all fails.
        let dir = scratch("readonly");
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"not a directory").expect("write");
        let state = SettingsState::new(blocker.join("settings.json"), Settings::default());

        let result = state.update(|s| s.always_on_top = true);
        assert!(result.is_err(), "the write should have failed");
        assert!(
            state.snapshot().always_on_top,
            "in-memory value must still change"
        );
    }
```

Add to `src-tauri/src/commands.rs`'s test module, replacing the existing `a_missing_index_parses_to_no_segments` test, which covers `run_parse` — a function this task deletes:

```rust
    /// `set_input` is a thin push into the watch channel; the part worth testing
    /// is that the newest value is what a reader sees.
    #[tokio::test]
    async fn set_input_publishes_the_text_to_the_channel() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        push_input(&tx, "東京".to_string()).expect("push");
        assert_eq!(
            crate::parse::next_input(&mut rx).await.as_deref(),
            Some("東京")
        );
    }

    /// A dead worker must surface as an error rather than a silent no-op: the
    /// user pressed Parse and nothing would ever arrive.
    #[tokio::test]
    async fn set_input_reports_a_dead_worker() {
        let (tx, rx) = tokio::sync::watch::channel(String::new());
        drop(rx);
        assert!(push_input(&tx, "東京".to_string()).is_err());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator`

Expected: FAIL to compile — `cannot find type 'SettingsState'`, `cannot find function 'push_input'`. That is the intended RED. (If you pulled `SettingsState` forward in Task 3, only `push_input` is missing.)

- [ ] **Step 3: Add `SettingsState` to `state.rs`**

Insert into `src-tauri/src/state.rs`, above the test module:

```rust
use std::sync::Mutex;

use crate::settings::{save, Settings, SettingsError};

/// Settings plus the path they persist to, shared between the commands that
/// change them and the poll that reads them.
pub struct SettingsState {
    path: PathBuf,
    settings: Mutex<Settings>,
}

impl SettingsState {
    pub fn new(path: PathBuf, settings: Settings) -> Self {
        Self {
            path,
            settings: Mutex::new(settings),
        }
    }

    /// A lock poisoned by a panic in another thread still holds usable settings:
    /// every write replaces the whole value, so there is no torn state to
    /// protect against. Recovering beats refusing to read a toggle.
    fn locked(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.settings.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> Settings {
        self.locked().clone()
    }

    pub fn monitoring_enabled(&self) -> bool {
        self.locked().clipboard_monitoring
    }

    /// Apply a change and persist it.
    ///
    /// The in-memory value is updated even when the write fails, so a read-only
    /// config dir degrades to "settings do not survive restart" rather than
    /// "the toggles do not work".
    pub fn update<F>(&self, change: F) -> Result<(), SettingsError>
    where
        F: FnOnce(&mut Settings),
    {
        let snapshot = {
            let mut settings = self.locked();
            change(&mut settings);
            settings.clone()
        };
        save(&self.path, &snapshot)
    }
}
```

- [ ] **Step 4: Rewrite `commands.rs`**

Read the shipped `startup_error` first and preserve its behaviour exactly. Then replace everything in `src-tauri/src/commands.rs` above the test module with:

```rust
use tauri::State;
use tokio::sync::watch;

use crate::settings::Settings;
use crate::state::{SettingsState, StartupFailure};

/// The sending half of the input channel, managed so commands can reach it.
pub struct InputSender(pub watch::Sender<String>);

/// Publish text for the worker to parse.
///
/// Separated from the command so it can be tested without a Tauri app handle.
fn push_input(tx: &watch::Sender<String>, text: String) -> Result<(), String> {
    tx.send(text)
        .map_err(|_| "the parse worker is no longer running".to_string())
}

/// Queue TEXT for parsing. The result arrives as a `parse-result` event.
///
/// Fire-and-forget rather than request/response: the clipboard produces parses
/// nobody asked for, so the webview renders from the event stream either way and
/// a returned value here would be a second, redundant path.
#[tauri::command]
pub fn set_input(text: String, input: State<'_, InputSender>) -> Result<(), String> {
    push_input(&input.0, text)
}

#[tauri::command]
pub fn set_clipboard_monitoring(
    enabled: bool,
    settings: State<'_, SettingsState>,
) -> Result<(), String> {
    settings
        .update(|s| s.clipboard_monitoring = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_always_on_top(
    enabled: bool,
    window: tauri::Window,
    settings: State<'_, SettingsState>,
) -> Result<(), String> {
    window.set_always_on_top(enabled).map_err(|e| e.to_string())?;
    settings
        .update(|s| s.always_on_top = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_settings(settings: State<'_, SettingsState>) -> Settings {
    settings.snapshot()
}

/// The startup error, or `null` when startup succeeded.
#[tauri::command]
pub fn startup_error(failure: State<'_, StartupFailure>) -> Option<String> {
    let message = failure.0.as_str();
    if message.is_empty() {
        None
    } else {
        Some(message.to_string())
    }
}
```

Delete `run_parse` and `parse_text`; `parse.rs` owns parsing now.

- [ ] **Step 5: Wire `main.rs`**

In `src-tauri/src/main.rs`'s `setup`, after the existing config-dir resolution and before the `load_state` match, add:

```rust
            // Settings first: the poll needs the monitoring flag before it starts.
            let settings_path = settings::settings_path(&config_dir);
            let (loaded, settings_warning) = settings::load(&settings_path);
            if let Some(warning) = settings_warning {
                eprintln!("{warning}");
            }
            if loaded.always_on_top {
                if let Some(window) = app.get_webview_window("main") {
                    // Not fatal: the window exists, it just is not pinned, and
                    // the toggle can retry.
                    let _ = window.set_always_on_top(true);
                }
            }
            let settings = std::sync::Arc::new(state::SettingsState::new(settings_path, loaded));

            let (tx, rx) = tokio::sync::watch::channel(String::new());
            app.manage(commands::InputSender(tx.clone()));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(clipboard::run_poll(
                handle,
                tx,
                std::sync::Arc::clone(&settings),
            ));
```

Then, on the success branch where `Arc<AppState>` is managed, spawn the worker:

```rust
                Ok(s) => {
                    let shared = std::sync::Arc::new(s);
                    app.manage(std::sync::Arc::clone(&shared));
                    app.manage(state::StartupFailure(String::new()));
                    let handle = app.handle().clone();
                    tauri::async_runtime::spawn(parse::run_worker(handle, shared, rx));
                }
```

Register the plugin and replace the handler list:

```rust
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            commands::set_input,
            commands::set_clipboard_monitoring,
            commands::set_always_on_top,
            commands::get_settings,
            commands::startup_error
        ])
```

**One thing to resolve and report:** the poll holds `Arc<SettingsState>` while the commands take `State<'_, SettingsState>`, so `app.manage` needs a value the commands can extract. Managing the bare `SettingsState` and giving the poll its own `Arc` means two objects and two sources of truth — wrong. Either manage `Arc<SettingsState>` and make the commands take `State<'_, Arc<SettingsState>>`, or have the poll take `State`-free access some other way. **Pick one, make both sides agree, and say which you chose and why.**

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator`
Expected: PASS. Report the count.

- [ ] **Step 7: Verify the gate**

```bash
cargo build -p translation-aggregator
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo +1.85 check -p jparser -p jmdict-source -p xtask
```

- [ ] **Step 8: Format and commit**

```bash
rustfmt --edition 2021 src-tauri/src/state.rs src-tauri/src/commands.rs src-tauri/src/main.rs
git diff --stat
git add src-tauri/src Cargo.lock
git commit -m "feat: wire the clipboard, worker, and settings commands"
```

`git diff --stat` must not list any file under `crates/jparser/`.

---

## Task 5: The event-driven frontend

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`, `e2e/stub.ts`, `e2e/panes.spec.ts`

**Interfaces:**
- Consumes: `set_input`, `startup_error`, and the `parse-result` / `parse-error` events (Tasks 2, 4).
- Produces: a `main.ts` that renders purely from events. Task 6 adds controls to it.

This task changes how results arrive and changes nothing about how they look. Keep the rendering identical so a regression is unambiguous.

- [ ] **Step 1: Resolve how Playwright stubs `listen`**

`e2e/stub.ts` currently overrides `window.__TAURI_INTERNALS__.invoke`. Events use a different path: `listen` registers a callback and the backend invokes it by id.

**Read `node_modules/@tauri-apps/api/event.js` and `core.js`** and determine what `listen` actually calls and how the callback is addressed — in Tauri 2.11 it goes through `invoke('plugin:event|listen', …)` with a callback created by `transformCallback`, which registers a function on `window`. Extend the stub so a test can fire an event, and **report exactly what you found**. The version is pinned, so the answer is in the tree — do not guess.

If it proves impractical to emulate faithfully, the fallback is for the stub to capture the registered handler and expose it as `window.__TA_EMIT__(event, payload)`; say so and use it.

- [ ] **Step 2: Write the failing tests**

Rewrite `src/main.test.ts` for the event path:

```ts
import { describe, expect, test, vi, beforeEach } from 'vitest';

const listeners = new Map<string, (e: { payload: unknown }) => void>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (cmd: string) => (cmd === 'startup_error' ? null : undefined)),
}));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

describe('the event-driven render path', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    vi.resetModules();
    await import('./main');
  });

  test('renders a parse-result event', () => {
    emit('parse-result', {
      segments: [
        {
          start: 0, len: 2, surface: '東京', reading: 'とうきょう', matched: true,
          entries: [{
            headword: '東京', reading: 'とうきょう', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          }],
        },
      ],
    });
    expect(document.querySelector('.chip')?.textContent).toBe('東京');
    expect(document.querySelector('.def-row')).not.toBeNull();
  });

  test('a parse-error event leaves the previous result on screen', () => {
    emit('parse-result', {
      segments: [
        { start: 0, len: 1, surface: '本', reading: 'ほん', matched: true, entries: [] },
      ],
    });
    emit('parse-error', 'the parser panicked on an input of 4096 characters');
    emit('parse-error', 'a second failure');

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
    expect(document.querySelector('.sentence')).not.toBeNull();
  });
});
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts`
Expected: FAIL — `main.ts` still awaits `invoke('parse_text')` and never registers a listener, so `emit` throws `nothing listening for parse-result`.

- [ ] **Step 4: Rewrite `run` and add the listeners**

In `src/main.ts`, add the import and replace `run`'s body with a `show` helper plus listener registration. Preserve the existing `parseError` / `output` / `input` lookups, `errorBlock`, `showStartupError`, and the click and keydown handlers exactly as they are:

```ts
import { listen } from '@tauri-apps/api/event';

function show(result: ParseResult): void {
  const sentence = renderSentence(result);
  const definitions = renderDefinitions(result);

  sentence.addEventListener('click', (e) => {
    const chip = (e.target as HTMLElement).closest<HTMLElement>('[data-start]');
    if (chip === null) return;
    const row = definitions.querySelector(`.def-row[data-start="${chip.dataset.start}"]`);
    row?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    definitions.querySelectorAll('.marked').forEach((n) => n.classList.remove('marked'));
    row?.classList.add('marked');
  });

  parseError.replaceChildren();
  output.replaceChildren(sentence, definitions);
}

void listen<ParseResult>('parse-result', (e) => show(e.payload));
// A failure replaces only the message, never the result: the previous parse
// stays readable while the user works out what went wrong.
void listen<string>('parse-error', (e) => parseError.replaceChildren(errorBlock(e.payload)));

async function run(): Promise<void> {
  try {
    await invoke('set_input', { text: input.value });
  } catch (e) {
    parseError.replaceChildren(errorBlock(String(e)));
  }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run`
Expected: PASS. Report the count.

- [ ] **Step 6: Update the Playwright stub and specs**

Extend `e2e/stub.ts` per Step 1's finding so the fixture arrives as a `parse-result` event rather than an `invoke` return, and update `e2e/panes.spec.ts` to fire it. The assertions — chip visibility, `.def-row` count, chip-click marking, the computed-style focus and border checks — stay exactly as they are; only how the result arrives changes.

- [ ] **Step 7: Verify the gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
npx playwright test
CI=1 npx playwright test
```

- [ ] **Step 8: Commit**

```bash
git add src e2e
git commit -m "feat: render from the parse event stream"
```

---

## Task 6: The header controls

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`, `src/styles/global.css`, `e2e/panes.spec.ts`

**Interfaces:**
- Consumes: `get_settings`, `set_always_on_top`, `set_clipboard_monitoring` (Task 4).
- Produces: nothing later in this phase consumes.

- [ ] **Step 1: Write the failing tests**

Extend the `invoke` mock at the top of `src/main.test.ts` so `get_settings` resolves to `{ always_on_top: false, clipboard_monitoring: true }` and both setters resolve to `undefined`. Then add:

```ts
describe('the header controls', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    vi.resetModules();
    await import('./main');
    // let the get_settings promise resolve
    await Promise.resolve();
    await Promise.resolve();
  });

  test('reflects loaded settings in aria-pressed', () => {
    expect(document.querySelector('#monitor')?.getAttribute('aria-pressed')).toBe('true');
    expect(document.querySelector('#always-on-top')?.getAttribute('aria-pressed')).toBe('false');
  });

  test('toggling monitoring flips aria-pressed', async () => {
    const button = document.querySelector<HTMLButtonElement>('#monitor');
    if (button === null) throw new Error('#monitor missing');
    button.click();
    await Promise.resolve();
    expect(button.getAttribute('aria-pressed')).toBe('false');
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/main.test.ts`
Expected: FAIL — `#monitor missing`; there is no header yet.

- [ ] **Step 3: Add the header**

In `src/main.ts`, make the header the **first** child of `#app`, before `.input-row`:

```ts
app.innerHTML = `
  <header class="controls">
    <button id="always-on-top" type="button" aria-pressed="false">Always on top</button>
    <button id="monitor" type="button" aria-pressed="true">Monitoring</button>
  </header>
  <div class="input-row">
    <input id="text" type="text" aria-label="Japanese text to parse" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
  <div id="parse-error"></div>
  <div class="panes"><div id="output"></div></div>
`;

const alwaysOnTop = app.querySelector<HTMLButtonElement>('#always-on-top');
const monitor = app.querySelector<HTMLButtonElement>('#monitor');

function bindToggle(button: HTMLButtonElement | null, command: string): void {
  if (button === null) return;
  button.addEventListener('click', () => {
    const next = button.getAttribute('aria-pressed') !== 'true';
    // Flip first so the control feels immediate; a rejected command reverts it.
    button.setAttribute('aria-pressed', String(next));
    void invoke(command, { enabled: next }).catch((e) => {
      button.setAttribute('aria-pressed', String(!next));
      parseError.replaceChildren(errorBlock(String(e)));
    });
  });
}

bindToggle(alwaysOnTop, 'set_always_on_top');
bindToggle(monitor, 'set_clipboard_monitoring');

async function applySettings(): Promise<void> {
  const settings = await invoke<{ always_on_top: boolean; clipboard_monitoring: boolean }>(
    'get_settings',
  );
  alwaysOnTop?.setAttribute('aria-pressed', String(settings.always_on_top));
  monitor?.setAttribute('aria-pressed', String(settings.clipboard_monitoring));
}

void applySettings();
```

- [ ] **Step 4: Add the styles**

In `src/styles/global.css`, change `#app`'s grid to four rows and add the header:

```css
#app {
  display: grid;
  grid-template-rows: auto auto auto 1fr;
  height: 100vh;
}

.controls {
  display: flex;
  gap: 8px;
  padding: 8px var(--space-pane);
  border-bottom: 1px solid var(--color-rule);
}

.controls button {
  font-family: var(--font-ui);
  font-size: var(--text-tag);
  color: var(--color-muted);
  background: none;
  border: 1px solid var(--color-rule);
  border-radius: 3px;
  padding: 3px 8px;
  cursor: pointer;
}

.controls button[aria-pressed='true'] {
  color: var(--color-text);
  border-color: var(--color-text);
}
```

The pressed state carries both a colour change and a border change, so it is not colour-only.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npx vitest run`
Expected: PASS. Report the count.

- [ ] **Step 6: Regenerate the baselines and inspect every one**

The header changes every screenshot. Regenerate, then **open each written PNG** under `e2e/panes.spec.ts-snapshots/` and confirm: the header reads as a control strip rather than a browser toolbar, the pressed and unpressed states are distinguishable in both themes, the four-row grid still gives `.panes` the flexible track, and nothing clips at 480×320.

**Do not accept a baseline you have not looked at.** Describe what you saw per image in your report.

- [ ] **Step 7: Run the full gate**

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

- [ ] **Step 8: Verify by hand what no test can**

Build an index if one is absent (`jparser-cli ensure-dictionary --source-dir <dir> <root>`), run `npm run tauri dev`, and confirm:

1. Copying Japanese text from another app parses it without touching the window.
2. Toggling **Monitoring** off stops that; toggling it back on resumes.
3. Toggling **Always on top** keeps the window in front of another app, and off releases it.
4. Both settings survive a restart.
5. Copying non-Japanese text does nothing.

Always-on-top has no automated test by design (spec §7.1). **Report exactly what you observed.** If something cannot be verified in your environment, say so plainly rather than claiming it.

- [ ] **Step 9: Commit**

```bash
git diff --stat
git add src e2e
git commit -m "feat: add the always-on-top and monitoring controls"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 200 ms poll with four skip conditions | 3 |
| §1 single worker, latest-wins | 2 |
| §1 results pushed as events | 2 (emit), 5 (receive) |
| §1 always-on-top toggle | 4 (command), 6 (control) |
| §1 clipboard pause toggle | 4 (command), 6 (control) |
| §1 settings file for exactly those two keys | 1 |
| §1.1 monitoring defaults on, pause as mitigation | 1 (default), 6 (control) |
| §2 module split; no `window.rs` | File Structure |
| §3 `should_parse` as a pure predicate | 3 |
| §3 `last_written` in the signature, unused this phase | 3 |
| §4 watch channel, `spawn_blocking` | 2 |
| §4.1 `parse_text` → `set_input` migration | 4 (Rust), 5 (frontend) |
| §5 clipboard read log-and-skip | 3 |
| §5 parse error keeps previous result | 2 (emit), 5 (render) |
| §5 panic containment | 2 |
| §5 settings missing / corrupt / write-fails | 1, 4 |
| §5 unknown-key preservation | 1 (Steps 1, 5) |
| §6 header row, four-row grid, `aria-pressed` | 6 |
| §6 manual input stays | 5, 6 |
| §7.1 no test touches the clipboard | 3 (predicate only) |
| §7.1 always-on-top verified by hand | 6 Step 8 |
| §7.2 Vitest with `listen` mocked; Playwright stubs events | 5, 6 |
| §7.3 coverage reported, `src-tauri` ungated | 6 Step 7 |
| §8 resolved facts consumed, not re-derived | "Resolved facts" |
| §9 invariants, including no `panic = "abort"` | Global Constraints |
| §10 constraints inherited | Global Constraints |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Five steps direct the implementer to *verify and report* rather than guess — Task 2 Step 5 (`Emitter` trait and `JoinHandle` formatting), Task 3 Step 4 (`ClipboardExt` and `read_text`'s return shape), Task 4 Step 5 (how `SettingsState` is managed), Task 5 Step 1 (how `listen` is stubbed), and Task 6 Step 8 (manual verification). Each names the exact uncertainty and where the answer lives.

**3. Type consistency across task boundaries.** Checked:

- `Settings { always_on_top, clipboard_monitoring, extra }` (Task 1) is what `SettingsState` wraps (Task 4) and what `get_settings` returns to the TypeScript literal in Task 6 — match.
- `settings_path(&Path) -> PathBuf` (Task 1) is called in `main.rs`'s setup (Task 4) — match.
- `watch::Sender<String>` is created in Task 4's `main.rs`, wrapped as `InputSender` for commands (Task 4), and its `Receiver` half is passed to `run_worker` (Task 2) — match.
- `next_input(&mut watch::Receiver<String>)` (Task 2) is called by `run_worker` (Task 2) and by Task 4's `set_input` test — match.
- `PARSE_RESULT_EVENT` / `PARSE_ERROR_EVENT` are `"parse-result"` / `"parse-error"` (Task 2) and are the strings Task 5 listens for — match.
- `should_parse(&str, Option<&str>, Option<&str>)` (Task 3) is called with those exact types by `run_poll` and its tests — match.
- `Sense` in Task 5's fixture uses `glosses`/`xrefs`, matching `SenseData` and the shipped `src/types.ts` — match. It does **not** use the `gloss`/`dialect` names that were wrong in the 2D plan.

**4. Residual risks a human should look at.**

- **Task 5 Step 1 is the phase's biggest unknown.** Stubbing `listen` for Playwright is genuinely harder than stubbing `invoke` was, and the fallback (`window.__TA_EMIT__`) is less faithful. If it turns out impractical, dropping the event assertions from Playwright and relying on Vitest is an acceptable retreat — say so rather than shipping a stub that silently tests nothing.
- **Tasks 4 and 5 are a broken window between them.** Task 4 deletes `parse_text` while the frontend still calls it. They must land together or the app is unusable at that commit; the review can still gate them separately.
- **Task 3 does not compile on its own** unless the implementer pulls `SettingsState` forward, which Step 4 permits and asks them to report. Either choice is fine; leaving it ambiguous would not be.
- **`run_poll` never stops.** Pausing is a flag check, not a cancelled task. That is deliberate, but it means the poll survives every toggle and its only exit is the worker's channel closing.
- **Always-on-top is untested by design.** Task 6 Step 8 is the only thing standing behind it, and it depends on the implementer actually doing it and reporting honestly.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-14-jparser-phase2e.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
