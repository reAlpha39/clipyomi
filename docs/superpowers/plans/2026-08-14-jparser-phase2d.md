# JParser Phase 2D — Tauri Shell and Parse Panes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Put parsed Japanese on screen — the port's first user-facing surface, after four phases of headless parser and index work.

**Architecture:** A new `src-tauri` workspace member holds the shell: it opens the newest index generation once at startup into Tauri managed state, and exposes one command, `parse_text`, which runs `jparser::parse` on `spawn_blocking` and returns a JSON `ParseResult`. A Vite + vanilla-TypeScript webview renders that result as a sentence pane of word chips and a definition pane of entry rows. No clipboard, no worker, no events — 2E adds those.

**Tech Stack:** Rust 2021 / MSRV 1.85, `tauri 2`, Vite 8, TypeScript 7, Vitest 4 (DOM tests), Playwright 1.62 (visual). No frontend framework.

**Reference:** `docs/superpowers/specs/2026-08-14-jparser-phase2d-design.md` (authoritative), with `docs/superpowers/specs/2026-08-12-jparser-port-design.md` §3/§6/§7 for architecture and UI direction, and `docs/superpowers/2026-08-14-jparser-phase2c-handoff.md` for the `hints` surface. The C++ original in `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **License GPL v2.** Every new Rust source file gets the standard header comment, copied verbatim from `crates/jparser/src/index/mod.rs:1-6`.
- **MSRV 1.85.** The gate is `cargo +1.85 check --workspace`. `tauri 2.11.5` declares `rust-version = "1.77.2"`, comfortably below.
- **`crates/jparser` keeps no Tauri dependency and no I/O beyond its index and asset files** (port design §3 hard rule). Task 1 adds only `serde` derives; `serde` is already non-optional there.
- **`mecab` stays off by default in `jparser`.** The purity gate `cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"` must keep returning **0**. `src-tauri` enables `mecab`; `jparser` itself does not.
- **Errors are explicit:** no `unwrap()`, `expect()`, or `unreachable!()` in library or binary code outside `#[cfg(test)]`. Never swallow an error without a comment naming the reason.
- **Naming, frozen:** `parse_text`, `AppState`, `TA_HINTS_DICT`, and the eight serialized flag names `primary`, `pronounce`, `common_line`, `common`, `particle`, `counter`, `top`, `is_name`.
- **File size** 200–400 lines typical, **800 hard maximum including tests**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. **Never `cargo fmt`, never `cargo fmt -p jparser`** — it reformats `conjugation.rs`, `kana.rs`, and `romaji.rs`, which this phase must leave untouched. `conjugation.rs` is deliberately not rustfmt-clean; "fixing" it is a defect. After formatting run `git diff --stat` and confirm only intended files moved.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` **and** `cargo clippy -p jparser --features mecab --all-targets -- -D warnings` clean at the end of every task.
- **`crates/jparser/src/segment.rs` is at 778/800 lines and must not be edited.**
- **Window geometry:** default 720×480, minimum 480×320, not persisted.

**Invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format; a published `gen-N` is immutable; directory knowledge lives only in `generations.rs` and `ensure_dictionary`; the staging filename stays process-unique; a `.partial` file is never resolved. Adding `Serialize` to result types is not an index-format change — `EntryData`'s on-disk encoding is untouched and `ParseResult` has never been persisted.

---

## Resolved facts — do not re-derive these

Measured 2026-08-14 against the tree at commit `4c9e0ae`. Spec §8 records the API shapes.

| Fact | Value |
|---|---|
| `tauri` crate latest | **2.11.5**, `rust-version = "1.77.2"` |
| `@tauri-apps/api` | **2.11.1** |
| `@tauri-apps/cli` | **2.11.4** |
| `vite` | **8.2.1** |
| `typescript` | **7.0.2** |
| `vitest` | **4.1.10** |
| `@playwright/test` | **1.62.1** |
| Local toolchain | cargo 1.97.1, node v26.0.0, npm 11.12.1 |
| `cargo-tauri` CLI | **not installed** — use the npm `@tauri-apps/cli` via `npx`, do not `cargo install` |

**Send + Sync verified by compile probe** — all three managed-state types pass `fn assert_send_sync<T: Send + Sync + 'static>()`:

```
jparser::index::load::Index
jparser::conjugation::ConjugationTable
jparser::hints::VibratoTokenizer
```

This was the main risk in putting them in `tauri::State`; it is resolved. Do not re-probe.

**API signatures, verified in-tree:**

```rust
jparser::index::generations::latest(root: &Path) -> Result<Option<PathBuf>, IndexError>
jparser::index::load::Index::open(dir: &Path) -> Result<Self, IndexError>
jparser::conjugation::ConjugationTable::load_embedded() -> Result<Self, ConjugationError>
jparser::hints::VibratoTokenizer::load(path: &Path) -> Result<Self, HintsError>
jparser::hints::VibratoTokenizer::hints(&self, text: &str) -> BoundaryFlags
jparser::parse(&Index, &ConjugationTable, &str, &ParseOptions,
               Option<&dyn BoundaryHints>) -> Result<ParseResult, ParseError>
```

**`WordFlags` is `pub struct WordFlags(pub u16)`** (`crates/jparser/src/record.rs:30`) with exactly eight constants, in bit order:

| Constant | Bit | Serialized name |
|---|---|---|
| `PRIMARY` | `0x0001` | `primary` |
| `PRONOUNCE` | `0x0002` | `pronounce` |
| `COMMON_LINE` | `0x0004` | `common_line` |
| `COMMON` | `0x0008` | `common` |
| `PARTICLE` | `0x0010` | `particle` |
| `COUNTER` | `0x0020` | `counter` |
| `TOP` | `0x0040` | `top` |
| `IS_NAME` | `0x0080` | `is_name` |

**Baseline:** `cargo test --workspace` = 292 passed / 0 failed / 1 ignored. `cargo test -p jparser --features mecab` = 271 passed.

**Adding `src-tauri` to the workspace changes what CI compiles.** Tauri needs system libraries on Linux (`libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`, `libxdo-dev`, `libssl-dev`). Every CI job that compiles the workspace must install them. Task 2 Step 8 handles this; it is not optional and it is the most likely way this phase turns CI red.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/jparser/src/lib.rs` | *(modified)* `Serialize` on `ParseResult`, `Segment`, `Entry` |
| `crates/jparser/src/record.rs` | *(modified)* hand-written `Serialize` for `WordFlags` + name-pinning tests |
| `Cargo.toml` | *(modified)* add `src-tauri` to `members` |
| `.github/workflows/ci.yml` | *(modified)* Linux system deps; a `frontend` job |
| `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html` | *(new)* frontend toolchain |
| `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs` | *(new)* shell manifest and config |
| `src-tauri/src/main.rs` | *(new)* entry point, state construction, startup wiring |
| `src-tauri/src/state.rs` | *(new)* `AppState`, `StartupError`, dictionary-root resolution |
| `src-tauri/src/commands.rs` | *(new)* `parse_text`, `startup_error` |
| `src/main.ts` | *(new)* input handler → `invoke` → render |
| `src/render/sentence.ts` | *(new)* `ParseResult` → chip DOM |
| `src/render/definitions.ts` | *(new)* `ParseResult` → entry rows |
| `src/types.ts` | *(new)* TypeScript mirror of the wire format |
| `src/styles/{tokens,typography,global}.css` | *(new)* design tokens and layout |
| `src/render/*.test.ts` | *(new)* Vitest DOM tests |
| `src/fixtures/tokyo.json` | *(new)* `ParseResult` fixture shared by Vitest and Playwright |
| `playwright.config.ts`, `e2e/panes.spec.ts`, `e2e/stub.ts` | *(new)* visual regression |

---

## Task 1: `Serialize` for the parse result types

**Files:**
- Modify: `crates/jparser/src/lib.rs`, `crates/jparser/src/record.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `ParseResult`, `Segment`, `Entry` serialize to JSON; `WordFlags` serializes as an array of the eight names above. Tasks 3–5 depend on this exact wire shape.

This task touches only `crates/jparser` and is fully testable there. No Tauri yet.

- [ ] **Step 1: Write the failing test for the flag names**

Add to `crates/jparser/src/record.rs`'s `#[cfg(test)] mod tests` (create the module at the end of the file if absent):

```rust
    #[test]
    fn flags_serialize_as_names_in_bit_order() {
        let all = WordFlags(0x00FF);
        let json = serde_json::to_string(&all).expect("serialize");
        assert_eq!(
            json,
            r#"["primary","pronounce","common_line","common","particle","counter","top","is_name"]"#
        );
    }

    /// These strings are public API the moment the webview reads them: a rename
    /// in Rust would compile clean and silently stop the sentence pane colouring
    /// particles. This test is the only thing that catches that.
    #[test]
    fn each_flag_name_is_pinned() {
        for (flag, name) in [
            (WordFlags::PRIMARY, "primary"),
            (WordFlags::PRONOUNCE, "pronounce"),
            (WordFlags::COMMON_LINE, "common_line"),
            (WordFlags::COMMON, "common"),
            (WordFlags::PARTICLE, "particle"),
            (WordFlags::COUNTER, "counter"),
            (WordFlags::TOP, "top"),
            (WordFlags::IS_NAME, "is_name"),
        ] {
            let json = serde_json::to_string(&flag).expect("serialize");
            assert_eq!(json, format!(r#"["{name}"]"#), "flag {name} renamed?");
        }
    }

    #[test]
    fn empty_flags_serialize_as_an_empty_array() {
        assert_eq!(serde_json::to_string(&WordFlags(0)).expect("ser"), "[]");
    }

    /// Bits with no constant must not invent a name or panic.
    #[test]
    fn unknown_bits_are_ignored() {
        assert_eq!(serde_json::to_string(&WordFlags(0x8000)).expect("ser"), "[]");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p jparser --lib record::tests`

Expected: FAIL to compile — `the trait bound 'WordFlags: Serialize' is not satisfied`. That is the intended RED.

- [ ] **Step 3: Implement `Serialize` for `WordFlags`**

In `crates/jparser/src/record.rs`, add with the other imports:

```rust
use serde::ser::SerializeSeq;
```

and immediately after the `impl WordFlags` block:

```rust
/// The eight flags paired with their wire names, in bit order.
///
/// The webview reads these strings to pick a chip's content class, so they are
/// public API — see the pinning tests below. A `u16` on the wire would force the
/// frontend to re-declare every bit constant in TypeScript, where nothing relates
/// them to this file.
const FLAG_NAMES: [(WordFlags, &str); 8] = [
    (WordFlags::PRIMARY, "primary"),
    (WordFlags::PRONOUNCE, "pronounce"),
    (WordFlags::COMMON_LINE, "common_line"),
    (WordFlags::COMMON, "common"),
    (WordFlags::PARTICLE, "particle"),
    (WordFlags::COUNTER, "counter"),
    (WordFlags::TOP, "top"),
    (WordFlags::IS_NAME, "is_name"),
];

impl serde::Serialize for WordFlags {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Bits without a constant are skipped rather than reported: they cannot
        // occur today, and a wire format is the wrong place to raise an error
        // about a bit the UI has no name for.
        let mut seq = s.serialize_seq(None)?;
        for (flag, name) in FLAG_NAMES {
            if self.contains(flag) {
                seq.serialize_element(name)?;
            }
        }
        seq.end()
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p jparser --lib record::tests`
Expected: PASS, 4 new tests.

- [ ] **Step 5: Derive `Serialize` on the three result types**

In `crates/jparser/src/lib.rs`, `ParseResult`, `Segment`, and `Entry` each currently carry `#[derive(Debug, Clone, PartialEq, Eq)]`. Make each:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
```

Do **not** add `Deserialize` — nothing reads a `ParseResult` back into Rust, and an unused derive is surface with no consumer.

- [ ] **Step 6: Write the wire-shape test**

Add a `#[cfg(test)] mod tests` at the end of `crates/jparser/src/lib.rs` (or extend it if present):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::WordFlags;

    /// Pins the exact JSON the webview parses. Field names here are the
    /// TypeScript property names in `src/types.ts`; changing either side alone
    /// breaks rendering with no compiler error anywhere.
    #[test]
    fn parse_result_serializes_to_the_documented_wire_shape() {
        let result = ParseResult {
            segments: vec![Segment {
                start: 0,
                len: 2,
                surface: "東京".to_string(),
                reading: Some("とうきょう".to_string()),
                matched: true,
                entries: vec![Entry {
                    headword: "東京".to_string(),
                    reading: Some("とうきょう".to_string()),
                    conjugation: None,
                    pos: vec!["n".to_string()],
                    senses: vec![],
                    flags: WordFlags::PRIMARY,
                }],
            }],
        };

        let json = serde_json::to_value(&result).expect("serialize");
        let seg = &json["segments"][0];
        assert_eq!(seg["start"], 0);
        assert_eq!(seg["len"], 2);
        assert_eq!(seg["surface"], "東京");
        assert_eq!(seg["reading"], "とうきょう");
        assert_eq!(seg["matched"], true);

        let entry = &seg["entries"][0];
        assert_eq!(entry["headword"], "東京");
        assert_eq!(entry["conjugation"], serde_json::Value::Null);
        assert_eq!(entry["pos"][0], "n");
        assert_eq!(entry["flags"][0], "primary");
    }

    /// An unmatched run must still be a well-formed segment: the sentence pane
    /// renders these muted and unchipped, so it needs them present, not omitted.
    #[test]
    fn an_unmatched_segment_serializes_with_null_reading_and_no_entries() {
        let result = ParseResult {
            segments: vec![Segment {
                start: 0,
                len: 1,
                surface: "〜".to_string(),
                reading: None,
                matched: false,
                entries: vec![],
            }],
        };

        let json = serde_json::to_value(&result).expect("serialize");
        assert_eq!(json["segments"][0]["reading"], serde_json::Value::Null);
        assert_eq!(json["segments"][0]["matched"], false);
        assert!(json["segments"][0]["entries"]
            .as_array()
            .expect("array")
            .is_empty());
    }
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p jparser --lib`
Expected: PASS. The lib count was 220 without features; this adds 6 → **226**.

Run: `cargo test --workspace` → **298 passed / 0 failed / 1 ignored** (292 + 6).

- [ ] **Step 8: Prove the name pinning is load-bearing**

Temporarily change `"particle"` to `"particles"` in `FLAG_NAMES` and re-run `cargo test -p jparser --lib record::tests`. Both `flags_serialize_as_names_in_bit_order` and `each_flag_name_is_pinned` must fail. Restore, re-run to green, and record both outputs in your report.

A pin that cannot be caught failing is not a pin.

- [ ] **Step 9: Format, gate, and commit**

```bash
rustfmt --edition 2021 crates/jparser/src/lib.rs crates/jparser/src/record.rs
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p jparser --features mecab --all-targets -- -D warnings
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
git diff --stat
git add crates/jparser/src/lib.rs crates/jparser/src/record.rs
git commit -m "feat: serialize parse results, with flags as names"
```

The purity grep must print **0**. `git diff --stat` must show only those two files — `conjugation.rs`, `kana.rs`, `romaji.rs`, and `segment.rs` must not appear.

---

## Task 2: The Tauri shell, its state, and CI

**Files:**
- Create: `package.json`, `vite.config.ts`, `tsconfig.json`, `index.html`, `src/main.ts`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/build.rs`, `src-tauri/src/main.rs`, `src-tauri/src/state.rs`
- Modify: `Cargo.toml`, `.gitignore`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Task 1's serializable types; `generations::latest`, `Index::open`, `ConjugationTable::load_embedded`, `VibratoTokenizer::load`.
- Produces: `AppState { index: Index, table: ConjugationTable, hints: Option<VibratoTokenizer> }`, `StartupError`, `StartupFailure(String)`, `resolve_dict_root(&Path) -> PathBuf`, and `load_state(&Path) -> Result<AppState, StartupError>`. Task 3 consumes all of these.

The deliverable: the app launches, and it knows whether it has an index.

- [ ] **Step 1: Add the frontend toolchain**

Create `package.json`:

```json
{
  "name": "translation-aggregator",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "tauri": "tauri",
    "test": "vitest run",
    "test:e2e": "playwright test"
  },
  "dependencies": {
    "@tauri-apps/api": "2.11.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "2.11.4",
    "typescript": "7.0.2",
    "vite": "8.2.1"
  }
}
```

Create `vite.config.ts`:

```ts
import { defineConfig } from 'vite';

// Port 1420 is fixed because tauri.conf.json's devUrl names it; strictPort makes
// a busy port fail loudly instead of silently serving where Tauri is not looking.
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: 'esnext' },
});
```

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noEmit": true,
    "isolatedModules": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"]
  },
  "include": ["src", "e2e", "vite.config.ts", "playwright.config.ts"]
}
```

Create `index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Translation Aggregator</title>
  </head>
  <body>
    <main id="app"></main>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Create a placeholder `src/main.ts` so the build has an entry point — Task 4 replaces it entirely:

```ts
document.querySelector('#app')!.textContent = 'Translation Aggregator';
```

Run `npm install`. Then append to `.gitignore`:

```
node_modules/
dist/
```

- [ ] **Step 2: Create the Tauri crate**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "translation-aggregator"
version = "0.1.0"
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
# `mecab` is enabled here and nowhere else: the shell is the only consumer that
# wants a tokenizer, and `jparser` itself must stay pure.
jparser = { path = "../crates/jparser", features = ["mecab"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Translation Aggregator",
  "version": "0.1.0",
  "identifier": "dev.jparser.translation-aggregator",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Translation Aggregator",
        "width": 720,
        "height": 480,
        "minWidth": 480,
        "minHeight": 320,
        "resizable": true
      }
    ],
    "security": { "csp": null }
  },
  "bundle": { "active": false }
}
```

**`bundle.active` is `false` deliberately.** Bundling requires an icon set this phase has no use for; installers are Phase 6 ("Windows + macOS builds"). `tauri dev` and `cargo test` do not need it.

Add `"src-tauri"` to the root `Cargo.toml`'s `members` array, keeping the existing three entries.

- [ ] **Step 3: Write the failing state tests**

Create `src-tauri/src/state.rs` with the GPL header copied verbatim from `crates/jparser/src/index/mod.rs:1-6`, then the module doc and **only** this test module:

```rust
//! Startup state: the index, the conjugation table, and optional hints.
//!
//! Everything here is built once at launch and then read-only, which is what
//! lets `AppState` live in Tauri's managed state behind a shared reference.

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-state-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// The expected first-run condition, not an error: 2E adds the download.
    #[test]
    fn an_empty_root_reports_no_index_rather_than_failing() {
        let dir = scratch("no-index");
        let err = load_state(&dir.join("dict")).err().expect("must fail");
        assert!(matches!(err, StartupError::NoIndex { .. }), "got {err:?}");
    }

    /// The message has to name the command that fixes it: this is the state
    /// every new user hits first, and "no index" without a remedy reads as a bug.
    #[test]
    fn the_no_index_error_names_the_build_command() {
        let dir = scratch("no-index-msg");
        let err = load_state(&dir.join("dict")).err().expect("must fail");
        let msg = err.to_string();
        assert!(msg.contains("build-index"), "got {msg}");
    }

    /// A directory that looks like a generation but is not one must surface as a
    /// distinct failure — collapsing it into NoIndex would tell the user to build
    /// an index they already have.
    #[test]
    fn a_corrupt_generation_is_an_index_error_not_no_index() {
        let dir = scratch("corrupt");
        let root = dir.join("dict");
        std::fs::create_dir_all(root.join("gen-1")).expect("mkdir");
        std::fs::write(root.join("gen-1").join("header.bin"), b"not a header").expect("write");

        let err = load_state(&root).err().expect("must fail");
        assert!(matches!(err, StartupError::Index(_)), "got {err:?}");
    }
}
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator`

Expected: FAIL to compile — `cannot find function 'load_state'`, `cannot find type 'StartupError'`. That is the intended RED.

**Confirm the header filename** before relying on Step 3's `header.bin`: grep `crates/jparser/src/index/mod.rs` for `HEADER_FILE`. If the real constant differs, use the real name and **report the difference**.

- [ ] **Step 5: Implement the state**

Insert into `src-tauri/src/state.rs`, above the test module:

```rust
use std::path::{Path, PathBuf};

use jparser::conjugation::{ConjugationError, ConjugationTable};
use jparser::hints::{HintsError, VibratoTokenizer};
use jparser::index::generations;
use jparser::index::load::Index;
use jparser::index::IndexError;

/// Environment variable naming an uncompressed compiled Vibrato dictionary.
///
/// An env var rather than a setting because settings persistence is 2E. When set
/// and unreadable this is fatal: a user who asked for hints and silently did not
/// get them receives a plausible result that is not what they asked for.
pub const HINTS_ENV: &str = "TA_HINTS_DICT";

/// Everything built once at launch and then only read.
pub struct AppState {
    pub index: Index,
    pub table: ConjugationTable,
    pub hints: Option<VibratoTokenizer>,
}

/// A startup failure, kept as a rendered string so the webview can display it.
/// Managed instead of `AppState` when `load_state` fails.
pub struct StartupFailure(pub String);

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error(
        "no dictionary index in {root}. Build one with:\n    \
         jparser-cli build-index <JMdict_e.xml> {root}"
    )]
    NoIndex { root: PathBuf },
    #[error("opening the dictionary index failed: {0}")]
    Index(#[from] IndexError),
    #[error("loading the conjugation table failed: {0}")]
    Conjugation(#[from] ConjugationError),
    #[error("{HINTS_ENV} is set but the dictionary could not be loaded: {0}")]
    Hints(#[from] HintsError),
}

/// The directory holding published index generations.
pub fn resolve_dict_root(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join("dict")
}

/// Open the newest published generation and build the app's read-only state.
pub fn load_state(root: &Path) -> Result<AppState, StartupError> {
    let generation = generations::latest(root)?.ok_or_else(|| StartupError::NoIndex {
        root: root.to_path_buf(),
    })?;

    let index = Index::open(&generation)?;
    let table = ConjugationTable::load_embedded()?;

    let hints = match std::env::var_os(HINTS_ENV) {
        Some(path) => Some(VibratoTokenizer::load(Path::new(&path))?),
        None => None,
    };

    Ok(AppState {
        index,
        table,
        hints,
    })
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator`
Expected: PASS, 3 tests.

If `a_corrupt_generation_is_an_index_error_not_no_index` fails with `NoIndex`, then `generations::latest` filters unreadable generations rather than returning them. Read `crates/jparser/src/index/generations.rs:123` to see which it does, adjust the **test** to match the real behavior, and **report the difference** — do not change `load_state` to force the test.

- [ ] **Step 7: Wire up `main.rs`**

Create `src-tauri/src/main.rs` with the GPL header, then:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Tauri shell. Opens the index once at startup and exposes `parse_text`.

mod state;

use std::sync::Arc;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let root = state::resolve_dict_root(&config_dir);
            match state::load_state(&root) {
                // `Arc` because `parse_text` moves a handle into `spawn_blocking`,
                // and `tauri::State` itself is not `Send`.
                Ok(s) => {
                    app.manage(Arc::new(s));
                }
                Err(e) => {
                    // Startup failures are surfaced to the webview rather than
                    // aborting: an app that will not launch cannot tell the user
                    // to run `build-index`.
                    app.manage(state::StartupFailure(e.to_string()));
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the Tauri runtime failed to start");
}
```

**The `.expect` in `main` is the one permitted exception** to the no-`expect` rule: if the Tauri runtime cannot start there is no window in which to report anything, and the alternative is a silent exit. It is the documented Tauri pattern.

- [ ] **Step 8: Add Tauri's system dependencies to CI**

In `.github/workflows/ci.yml`, every job that compiles the workspace now builds `src-tauri` and needs Linux system libraries. **Read the file and identify which jobs actually compile** — at time of writing those are `test`, `msrv`, `coverage`, and the `mecab` job added in Phase 2C. The `purity` job runs `cargo tree`, which resolves without compiling, so it should not need them; verify rather than assume.

Add to each compiling job, immediately after `actions/checkout` and before the toolchain step:

```yaml
      # src-tauri joined the workspace in Phase 2D, so every job that compiles
      # the workspace now links against webkit2gtk.
      - name: Install Tauri system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev \
            librsvg2-dev patchelf libxdo-dev libssl-dev
```

Record in your report exactly which jobs you changed and why you left any compiling job alone.

- [ ] **Step 9: Verify the whole gate**

```bash
cargo build -p translation-aggregator
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
cargo +1.85 check --workspace --quiet
npm run build
```

Expected: all succeed; the purity grep prints **0**; the workspace total is **301 passed / 1 ignored** (298 + 3).

If `cargo +1.85 check --workspace` fails inside a Tauri dependency, **stop and report** — that is an MSRV conflict this plan did not anticipate, not something to work around.

- [ ] **Step 10: Format and commit**

```bash
rustfmt --edition 2021 src-tauri/src/main.rs src-tauri/src/state.rs
git diff --stat
git add package.json package-lock.json vite.config.ts tsconfig.json index.html \
        src/main.ts src-tauri Cargo.toml Cargo.lock .gitignore \
        .github/workflows/ci.yml
git commit -m "feat: add the Tauri shell and its startup state"
```

---

## Task 3: The `parse_text` command

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: `AppState`, `StartupFailure` (Task 2); `jparser::parse`.
- Produces: `invoke("parse_text", { text })` resolving to a JSON `ParseResult` or rejecting with a string, and `invoke("startup_error")` resolving to `string | null`. Task 4's frontend calls exactly these two.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/commands.rs` with the GPL header, the module doc, and **only** this test module:

```rust
//! The webview's two entry points: parse a string, and ask why startup failed.

#[cfg(test)]
mod tests {
    use super::*;

    /// `parse_text` is a thin async wrapper, so the part worth testing is the
    /// piece that is not Tauri: turning parser output into the command's Result.
    /// A Tauri command needs a live app handle; this seam does not.
    #[test]
    fn an_empty_input_parses_to_no_segments() {
        let table = jparser::conjugation::ConjugationTable::load_embedded().expect("table");
        let out = run_parse(None, &table, "", None).expect("empty input parses");
        assert!(out.segments.is_empty());
    }

    #[test]
    fn a_startup_failure_is_reported_verbatim() {
        let msg = "no dictionary index in /nowhere";
        assert_eq!(startup_message(Some(msg)), Some(msg.to_string()));
        assert_eq!(startup_message(None), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p translation-aggregator commands`
Expected: FAIL to compile — `cannot find function 'run_parse'`, `cannot find function 'startup_message'`, and `file not found for module 'commands'` until Step 5 declares it.

- [ ] **Step 3: Implement the command**

Insert into `src-tauri/src/commands.rs`, above the test module:

```rust
use std::sync::Arc;

use jparser::conjugation::ConjugationTable;
use jparser::index::load::Index;
use jparser::hints::VibratoTokenizer;
use jparser::{BoundaryHints, ParseOptions, ParseResult};
use tauri::State;

use crate::state::{AppState, StartupFailure};

/// Parse `text`, applying hints when a tokenizer was loaded.
///
/// `index: None` yields an empty result rather than an error: it is the shape a
/// caller with no dictionary gets, and the webview already shows the startup
/// message in that case.
fn run_parse(
    index: Option<&Index>,
    table: &ConjugationTable,
    text: &str,
    hints: Option<&VibratoTokenizer>,
) -> Result<ParseResult, String> {
    let Some(index) = index else {
        return Ok(ParseResult { segments: vec![] });
    };
    let flags = hints.map(|t| t.hints(text));
    jparser::parse(
        index,
        table,
        text,
        &ParseOptions::default(),
        flags.as_ref().map(|f| f as &dyn BoundaryHints),
    )
    .map_err(|e| e.to_string())
}

/// The startup error to show, if startup failed.
fn startup_message(failure: Option<&str>) -> Option<String> {
    failure.map(str::to_string)
}

/// Parse TEXT against the loaded index.
///
/// The parse runs on a blocking thread: `jparser::parse` is synchronous CPU work
/// over the whole input, and running it on the async runtime would stall the
/// webview. `tauri::State` is not `Send`, so the `Arc` is cloned out first and
/// the clone is what crosses the boundary.
#[tauri::command]
pub async fn parse_text(
    text: String,
    state: State<'_, Arc<AppState>>,
) -> Result<ParseResult, String> {
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        run_parse(Some(&state.index), &state.table, &text, state.hints.as_ref())
    })
    .await
    .map_err(|e| format!("the parse task failed to run: {e}"))?
}

/// The startup error, or `null` when startup succeeded.
#[tauri::command]
pub fn startup_error(failure: Option<State<'_, StartupFailure>>) -> Option<String> {
    startup_message(failure.as_ref().map(|f| f.0.as_str()))
}
```

**Two things to verify rather than assume**, reporting either difference:

1. Whether `Option<State<'_, T>>` is a valid Tauri 2 command parameter for "this state may not be managed". If it is not, the alternative is managing a `StartupFailure` unconditionally in Task 2 Step 7 — empty string meaning success — and reading it directly. Check `node_modules`-free: read the `tauri` docs.rs page for `State`, or try it and read the compiler error.
2. Whether `spawn_blocking`'s `JoinHandle` error type formats with `{e}`. If not, adjust the `map_err`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p translation-aggregator`
Expected: PASS, 5 tests (3 from Task 2, 2 new).

- [ ] **Step 5: Register the commands**

In `src-tauri/src/main.rs`, add `mod commands;` beside `mod state;`, and add to the builder chain before `.run(...)`:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::parse_text,
            commands::startup_error
        ])
```

- [ ] **Step 6: Verify the gate**

```bash
cargo build -p translation-aggregator
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: all pass; workspace total **303 passed / 1 ignored**.

- [ ] **Step 7: Format and commit**

```bash
rustfmt --edition 2021 src-tauri/src/commands.rs src-tauri/src/main.rs
git diff --stat
git add src-tauri/src
git commit -m "feat: add the parse_text command"
```

---

## Task 4: The sentence pane

**Files:**
- Create: `src/types.ts`, `src/fixtures/tokyo.json`, `src/render/sentence.ts`, `src/render/sentence.test.ts`, `src/styles/tokens.css`, `src/styles/typography.css`, `src/styles/global.css`
- Modify: `src/main.ts`, `vite.config.ts`, `package.json`

**Interfaces:**
- Consumes: the wire format from Task 1; the `parse_text` and `startup_error` commands from Task 3.
- Produces: `renderSentence(result: ParseResult): HTMLElement`, and the `ParseResult` / `Segment` / `Entry` / `Sense` / `FlagName` TypeScript types. Task 5 consumes both.

- [ ] **Step 1: Add Vitest and the wire types**

```bash
npm install -D vitest@4.1.10 happy-dom@20.0.0
```

Change `vite.config.ts`'s import to `import { defineConfig } from 'vitest/config';` and add to the exported object:

```ts
  test: { environment: 'happy-dom', include: ['src/**/*.test.ts'] },
```

Create `src/types.ts`:

```ts
// Mirrors crates/jparser/src/lib.rs. The Rust test
// `parse_result_serializes_to_the_documented_wire_shape` pins these names on the
// other side; changing one side alone breaks rendering silently.

export type FlagName =
  | 'primary'
  | 'pronounce'
  | 'common_line'
  | 'common'
  | 'particle'
  | 'counter'
  | 'top'
  | 'is_name';

export interface Sense {
  pos: string[];
  gloss: string[];
  misc: string[];
  info: string[];
  dialect: string[];
}

export interface Entry {
  headword: string;
  reading: string | null;
  conjugation: string | null;
  pos: string[];
  senses: Sense[];
  flags: FlagName[];
}

export interface Segment {
  start: number;
  len: number;
  surface: string;
  reading: string | null;
  matched: boolean;
  entries: Entry[];
}

export interface ParseResult {
  segments: Segment[];
}
```

**Confirm `Sense`'s fields** before writing this: grep `crates/jparser/src/index/mod.rs` for `struct SenseData`. If the field names or count differ, use the real ones — and update the fixture in Step 2 to match — then **report the difference**.

- [ ] **Step 2: Create the fixture**

Create `src/fixtures/tokyo.json` — three segments so the panes have something to lay out, including one unmatched run:

```json
{
  "segments": [
    {
      "start": 0, "len": 2, "surface": "東京", "reading": "とうきょう",
      "matched": true,
      "entries": [{
        "headword": "東京", "reading": "とうきょう", "conjugation": null,
        "pos": ["n"],
        "senses": [{ "pos": ["n"], "gloss": ["Tokyo"], "misc": [], "info": [], "dialect": [] }],
        "flags": ["primary", "common"]
      }]
    },
    {
      "start": 2, "len": 1, "surface": "は", "reading": "は",
      "matched": true,
      "entries": [{
        "headword": "は", "reading": "は", "conjugation": null,
        "pos": ["prt"],
        "senses": [{ "pos": ["prt"], "gloss": ["topic marker"], "misc": [], "info": [], "dialect": [] }],
        "flags": ["primary", "particle"]
      }]
    },
    {
      "start": 3, "len": 2, "surface": "〜〜", "reading": null,
      "matched": false, "entries": []
    }
  ]
}
```

- [ ] **Step 3: Write the failing tests**

Create `src/render/sentence.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { renderSentence } from './sentence';
import fixture from '../fixtures/tokyo.json';
import type { ParseResult } from '../types';

const result = fixture as ParseResult;

describe('renderSentence', () => {
  test('renders one element per segment, in order', () => {
    const spans = renderSentence(result).querySelectorAll('[data-start]');
    expect(spans).toHaveLength(3);
    expect(spans[0].textContent).toBe('東京');
    expect(spans[1].textContent).toBe('は');
    expect(spans[2].textContent).toBe('〜〜');
  });

  test('chips a matched segment and leaves an unmatched run unchipped', () => {
    const spans = renderSentence(result).querySelectorAll('[data-start]');
    expect(spans[0].classList.contains('chip')).toBe(true);
    expect(spans[2].classList.contains('chip')).toBe(false);
    expect(spans[2].classList.contains('unmatched')).toBe(true);
  });

  test('classes a particle by its flag, not by its surface', () => {
    const el = renderSentence(result);
    expect(el.querySelector('[data-start="2"]')?.classList.contains('particle')).toBe(true);
    expect(el.querySelector('[data-start="0"]')?.classList.contains('particle')).toBe(false);
  });

  test('carries the start offset so a chip can address its definition row', () => {
    const el = renderSentence(result);
    expect(el.querySelector('[data-start="0"]')).not.toBeNull();
    expect(el.querySelector('[data-start="3"]')).not.toBeNull();
  });

  test('renders an empty result without throwing', () => {
    expect(renderSentence({ segments: [] }).querySelectorAll('[data-start]')).toHaveLength(0);
  });
});
```

- [ ] **Step 4: Run the tests to verify they fail**

Run: `npx vitest run src/render/sentence.test.ts`
Expected: FAIL — `Failed to resolve import "./sentence"`.

- [ ] **Step 5: Implement the renderer**

Create `src/render/sentence.ts`:

```ts
import type { ParseResult, Segment } from '../types';

/**
 * Content class for a segment's chip, from the flags the Rust side named.
 * Reading `flags` rather than inspecting the surface keeps one definition of
 * "particle" in the codebase, on the Rust side.
 */
function contentClass(segment: Segment): string {
  const flags = segment.entries[0]?.flags ?? [];
  if (flags.includes('particle')) return 'particle';
  if (flags.includes('counter')) return 'counter';
  return /[一-鿿]/.test(segment.surface) ? 'kanji' : 'kana';
}

export function renderSentence(result: ParseResult): HTMLElement {
  const root = document.createElement('div');
  root.className = 'sentence';

  for (const segment of result.segments) {
    const el = document.createElement('span');
    el.dataset.start = String(segment.start);
    el.textContent = segment.surface;

    // Unmatched runs stay unchipped so coverage gaps are visible rather than
    // disguised — seeing where the parser fails is the point of the window.
    el.className = segment.matched ? `chip ${contentClass(segment)}` : 'unmatched';

    root.append(el);
  }

  return root;
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/render/sentence.test.ts`
Expected: PASS, 5 tests.

- [ ] **Step 7: Add the stylesheets**

Create `src/styles/tokens.css`:

```css
:root {
  --color-bg: oklch(99% 0 0);
  --color-surface: oklch(97% 0 0);
  --color-text: oklch(20% 0 0);
  --color-muted: oklch(55% 0 0);
  --color-rule: oklch(90% 0 0);
  --color-kanji: oklch(45% 0.12 250);
  --color-kana: oklch(45% 0.08 160);
  --color-particle: oklch(55% 0.10 30);
  --color-counter: oklch(50% 0.10 300);

  --text-sentence: 24px;
  --text-reading: 11px;
  --text-gloss: 14px;
  --text-tag: 11px;

  --space-pane: 16px;
  --duration-fast: 120ms;
  --ease-out: cubic-bezier(0.16, 1, 0.3, 1);
}

@media (prefers-color-scheme: dark) {
  :root:not([data-theme='light']) {
    --color-bg: oklch(18% 0 0);
    --color-surface: oklch(22% 0 0);
    --color-text: oklch(93% 0 0);
    --color-muted: oklch(65% 0 0);
    --color-rule: oklch(32% 0 0);
    --color-kanji: oklch(78% 0.12 250);
    --color-kana: oklch(78% 0.08 160);
    --color-particle: oklch(80% 0.10 30);
    --color-counter: oklch(78% 0.10 300);
  }
}

:root[data-theme='dark'] {
  --color-bg: oklch(18% 0 0);
  --color-surface: oklch(22% 0 0);
  --color-text: oklch(93% 0 0);
  --color-muted: oklch(65% 0 0);
  --color-rule: oklch(32% 0 0);
  --color-kanji: oklch(78% 0.12 250);
  --color-kana: oklch(78% 0.08 160);
  --color-particle: oklch(80% 0.10 30);
  --color-counter: oklch(78% 0.10 300);
}
```

Every colour is defined on bare `:root` first; the media query and the explicit override only redefine. No colour has its only definition inside a media query — port design §7.3.

Create `src/styles/typography.css`:

```css
:root {
  --font-cjk: 'Hiragino Sans', 'Yu Gothic UI', 'Noto Sans JP', sans-serif;
  --font-ui: system-ui, -apple-system, 'Segoe UI', sans-serif;
  --font-mono: ui-monospace, 'SF Mono', Menlo, monospace;
}

.sentence {
  font-family: var(--font-cjk);
  font-size: var(--text-sentence);
  line-height: 1.7;
}

.definitions {
  font-family: var(--font-ui);
  font-size: var(--text-gloss);
}
```

Create `src/styles/global.css`:

```css
@import './tokens.css';
@import './typography.css';

* { box-sizing: border-box; }

body {
  margin: 0;
  background: var(--color-bg);
  color: var(--color-text);
}

#app {
  display: grid;
  grid-template-rows: auto 1fr;
  height: 100vh;
}

.input-row {
  display: flex;
  gap: 8px;
  padding: var(--space-pane);
  border-bottom: 1px solid var(--color-rule);
}

.input-row input {
  flex: 1;
  font-family: var(--font-cjk);
  font-size: 16px;
  background: var(--color-surface);
  color: var(--color-text);
  border: 1px solid var(--color-rule);
  padding: 6px 8px;
}

.panes { overflow-y: auto; padding: var(--space-pane); }
.sentence { margin-bottom: var(--space-pane); }

.chip {
  display: inline-block;
  padding: 2px 4px;
  border-radius: 3px;
  cursor: pointer;
  transition: transform var(--duration-fast) var(--ease-out);
}

.chip:hover { transform: translateY(-1px); }
.chip.kanji { color: var(--color-kanji); }
.chip.kana { color: var(--color-kana); }
.chip.particle { color: var(--color-particle); }
.chip.counter { color: var(--color-counter); }
.unmatched { color: var(--color-muted); }

.startup-error {
  padding: var(--space-pane);
  font-family: var(--font-mono);
  font-size: var(--text-tag);
  white-space: pre-wrap;
  color: var(--color-muted);
}

@media (prefers-reduced-motion: reduce) {
  .chip { transition: none; }
  .chip:hover { transform: none; }
}
```

- [ ] **Step 8: Wire the app entry**

Replace `src/main.ts` entirely:

```ts
import { invoke } from '@tauri-apps/api/core';
import { renderSentence } from './render/sentence';
import type { ParseResult } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <div class="input-row">
    <input id="text" type="text" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
  <div class="panes"><div id="output"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const input = app.querySelector<HTMLInputElement>('#text')!;

function errorBlock(message: string): HTMLElement {
  const el = document.createElement('pre');
  el.className = 'startup-error';
  el.textContent = message;
  return el;
}

async function showStartupError(): Promise<void> {
  const message = await invoke<string | null>('startup_error');
  if (message !== null) output.replaceChildren(errorBlock(message));
}

async function run(): Promise<void> {
  try {
    const result = await invoke<ParseResult>('parse_text', { text: input.value });
    output.replaceChildren(renderSentence(result));
  } catch (e) {
    // A parse failure keeps the previous result on screen rather than blanking
    // it; only the message is added.
    output.prepend(errorBlock(String(e)));
  }
}

app.querySelector('#parse')!.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
```

- [ ] **Step 9: Verify the gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 5 tests pass, no type errors, the Vite build succeeds.

- [ ] **Step 10: Commit**

```bash
git add src vite.config.ts package.json package-lock.json
git commit -m "feat: render the sentence pane"
```

---

## Task 5: The definition pane and visual regression

**Files:**
- Create: `src/render/definitions.ts`, `src/render/definitions.test.ts`, `playwright.config.ts`, `e2e/stub.ts`, `e2e/panes.spec.ts`
- Modify: `src/main.ts`, `src/styles/global.css`, `package.json`, `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: the TypeScript types and `renderSentence` (Task 4).
- Produces: `renderDefinitions(result: ParseResult): HTMLElement`. Nothing later in this phase consumes it.

- [ ] **Step 1: Write the failing tests**

Create `src/render/definitions.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { renderDefinitions } from './definitions';
import fixture from '../fixtures/tokyo.json';
import type { ParseResult } from '../types';

const result = fixture as ParseResult;

describe('renderDefinitions', () => {
  test('renders one row per matched segment, skipping unmatched runs', () => {
    // The fixture has two matched segments and one unmatched run.
    expect(renderDefinitions(result).querySelectorAll('.def-row')).toHaveLength(2);
  });

  test('shows headword, reading, and glosses', () => {
    const first = renderDefinitions(result).querySelector('.def-row')!;
    expect(first.querySelector('.headword')?.textContent).toBe('東京');
    expect(first.querySelector('.reading')?.textContent).toBe('とうきょう');
    expect(first.textContent).toContain('Tokyo');
  });

  test('addresses each row by its segment start, matching the sentence chips', () => {
    const el = renderDefinitions(result);
    expect(el.querySelector('.def-row[data-start="0"]')).not.toBeNull();
    expect(el.querySelector('.def-row[data-start="2"]')).not.toBeNull();
  });

  test('omits the conjugation tag when there is no conjugation', () => {
    expect(renderDefinitions(result).querySelector('.conjugation')).toBeNull();
  });

  test('renders a conjugation tag when present', () => {
    const conjugated: ParseResult = {
      segments: [{
        start: 0, len: 4, surface: '言われた', reading: 'いわれた', matched: true,
        entries: [{
          headword: '言う', reading: 'いう', conjugation: 'Negative Formal Past',
          pos: ['v5u'],
          senses: [{ pos: ['v5u'], gloss: ['to say'], misc: [], info: [], dialect: [] }],
          flags: ['primary'],
        }],
      }],
    };
    expect(renderDefinitions(conjugated).querySelector('.conjugation')?.textContent)
      .toBe('Negative Formal Past');
  });

  test('collapses alternative entries past the first', () => {
    const alternates: ParseResult = {
      segments: [{
        start: 0, len: 1, surface: '生', reading: 'せい', matched: true,
        entries: [
          { headword: '生', reading: 'せい', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], gloss: ['life'], misc: [], info: [], dialect: [] }],
            flags: ['primary'] },
          { headword: '生', reading: 'なま', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], gloss: ['raw'], misc: [], info: [], dialect: [] }],
            flags: ['primary'] },
        ],
      }],
    };
    const details = renderDefinitions(alternates).querySelector('details');
    expect(details).not.toBeNull();
    expect(details?.hasAttribute('open')).toBe(false);
    expect(details?.textContent).toContain('raw');
  });

  test('renders an empty result without throwing', () => {
    expect(renderDefinitions({ segments: [] }).querySelectorAll('.def-row')).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/render/definitions.test.ts`
Expected: FAIL — `Failed to resolve import "./definitions"`.

- [ ] **Step 3: Implement the renderer**

Create `src/render/definitions.ts`:

```ts
import type { Entry, ParseResult } from '../types';

function renderEntry(entry: Entry): HTMLElement {
  const el = document.createElement('div');
  el.className = 'entry';

  const head = document.createElement('div');
  head.className = 'entry-head';

  const headword = document.createElement('span');
  headword.className = 'headword';
  headword.textContent = entry.headword;
  head.append(headword);

  if (entry.conjugation !== null) {
    const tag = document.createElement('span');
    tag.className = 'conjugation';
    tag.textContent = entry.conjugation;
    head.append(tag);
  }
  el.append(head);

  if (entry.reading !== null) {
    const reading = document.createElement('div');
    reading.className = 'reading';
    reading.textContent = entry.reading;
    el.append(reading);
  }

  const senses = document.createElement('ol');
  senses.className = 'senses';
  for (const sense of entry.senses) {
    const li = document.createElement('li');
    li.textContent = sense.gloss.join('; ');
    senses.append(li);
  }
  el.append(senses);

  return el;
}

export function renderDefinitions(result: ParseResult): HTMLElement {
  const root = document.createElement('div');
  root.className = 'definitions';

  for (const segment of result.segments) {
    // Unmatched runs have no entries, so they get no row — the sentence pane
    // already shows them as gaps.
    if (!segment.matched || segment.entries.length === 0) continue;

    const row = document.createElement('section');
    row.className = 'def-row';
    row.dataset.start = String(segment.start);

    const [primary, ...alternates] = segment.entries;
    row.append(renderEntry(primary));

    if (alternates.length > 0) {
      // Collapsed past the first: the payoff from the segmenter's backtrack
      // pass, without letting alternates bury the ranked primary.
      const details = document.createElement('details');
      const summary = document.createElement('summary');
      summary.textContent = `${alternates.length} more`;
      details.append(summary);
      for (const alternate of alternates) details.append(renderEntry(alternate));
      row.append(details);
    }

    root.append(row);
  }

  return root;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run`
Expected: PASS, 12 tests (5 sentence + 7 definitions).

- [ ] **Step 5: Wire the pane and chip linking**

In `src/main.ts`, add the import:

```ts
import { renderDefinitions } from './render/definitions';
```

and replace the `try` block's body in `run`:

```ts
    const result = await invoke<ParseResult>('parse_text', { text: input.value });
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

    output.replaceChildren(sentence, definitions);
```

Append to `src/styles/global.css`:

```css
.def-row {
  padding: 8px 0;
  border-top: 1px solid var(--color-rule);
}

.def-row.marked { background: var(--color-surface); }

.entry-head { display: flex; align-items: baseline; gap: 8px; }

.headword {
  font-family: var(--font-cjk);
  font-size: var(--text-sentence);
}

.conjugation {
  font-family: var(--font-mono);
  font-size: var(--text-tag);
  color: var(--color-muted);
}

.reading {
  font-family: var(--font-cjk);
  font-size: var(--text-reading);
  color: var(--color-muted);
}

.senses { margin: 4px 0 0; padding-left: 20px; }

details summary {
  font-size: var(--text-tag);
  color: var(--color-muted);
  cursor: pointer;
}
```

- [ ] **Step 6: Add the visual regression harness**

```bash
npm install -D @playwright/test@1.62.1
npx playwright install chromium
```

Create `e2e/stub.ts`:

```ts
// Serves the real render path against a fixture, with `invoke` stubbed.
//
// `tauri-driver` supports Windows and Linux only — macOS has no WKWebView driver
// — so visual tests run the frontend in plain Chromium instead. This deliberately
// does not exercise the Rust↔webview seam; the src-tauri tests cover that side.
export const STUB = `
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd) => (cmd === 'startup_error' ? null : window.__FIXTURE__),
  };
`;
```

Create `playwright.config.ts`:

```ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
  },
  use: { baseURL: 'http://localhost:1420' },
});
```

Create `e2e/panes.spec.ts`:

```ts
import { expect, test } from '@playwright/test';
import fixture from '../src/fixtures/tokyo.json';
import { STUB } from './stub';

const SIZES = [
  { name: 'compact', width: 480, height: 320 },
  { name: 'default', width: 720, height: 480 },
];
const THEMES = ['light', 'dark'] as const;

for (const size of SIZES) {
  for (const theme of THEMES) {
    test(`panes render at ${size.name} in ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width: size.width, height: size.height });
      await page.emulateMedia({ colorScheme: theme });
      await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
      await page.goto('/');

      await page.fill('#text', '東京は');
      await page.click('#parse');

      await expect(page.locator('.chip').first()).toBeVisible();
      await expect(page.locator('.def-row')).toHaveCount(2);
      await expect(page).toHaveScreenshot(`panes-${size.name}-${theme}.png`);
    });
  }
}

test('a chip click marks its definition row', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');

  await page.click('.chip[data-start="2"]');
  await expect(page.locator('.def-row[data-start="2"]')).toHaveClass(/marked/);
});
```

**If `window.__TAURI_INTERNALS__` is not the hook `@tauri-apps/api` 2.11.1 reads**, find the real one — read `node_modules/@tauri-apps/api/core.js` and look for what `invoke` dispatches through — use it, and **report the difference**. The version is pinned, so the answer is in the tree.

- [ ] **Step 7: Run the visual tests and inspect every baseline**

Run: `npx playwright test`

The first run writes baseline screenshots and reports them as failures — that is expected. **Open each written PNG** under `e2e/panes.spec.ts-snapshots/` and confirm the panes actually look right before accepting: Japanese rendering at 24px, readings muted at 11px, the particle chip a different colour from the noun, the unmatched run muted and unchipped, and dark mode genuinely dark rather than a light page with inverted text. Then re-run to confirm green.

**Do not accept a baseline you have not looked at.** A screenshot test whose baseline is wrong pins the bug.

- [ ] **Step 8: Run the full gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
npx playwright test
cargo test --workspace 2>&1 | grep -E "^test result"
cargo test -p jparser --features mecab 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p jparser --features mecab --all-targets -- -D warnings
cargo check -p jparser --no-default-features --all-targets --quiet
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"
cargo +1.85 check --workspace --quiet
cargo llvm-cov -p jparser --features mecab --summary-only --fail-under-lines 80
```

Expected: all pass; the purity grep prints **0**; the workspace total is **303 passed / 1 ignored**.

Also report `src-tauri`'s coverage number (`cargo llvm-cov -p translation-aggregator --summary-only`). There is **no coverage gate on `src-tauri`** — a Tauri crate's `main.rs` is largely unreachable from tests and a gate would push toward testing the untestable. Report the number; do not add tests solely to move it.

- [ ] **Step 9: Add the frontend CI job**

In `.github/workflows/ci.yml`, add:

```yaml
  frontend:
    name: frontend
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '26'
          cache: npm
      - run: npm ci
      - run: npx tsc --noEmit
      - run: npx vitest run
      - run: npx playwright install --with-deps chromium
      - run: npx playwright test
```

**Screenshot baselines are platform-dependent.** CI runs Linux; baselines written on macOS will not match, and font rendering differs. Decide one of: commit Linux baselines generated in CI, restrict `toHaveScreenshot` to CI-only, or drop the screenshot assertions from CI and keep the DOM assertions. **Record which you chose and why in your report** — a permanently-red visual job teaches everyone to ignore CI, which is worse than no visual job.

- [ ] **Step 10: Commit**

```bash
git diff --stat
git add src e2e playwright.config.ts package.json package-lock.json \
        .github/workflows/ci.yml
git commit -m "feat: render the definition pane and add visual regression"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 in-scope: Tauri shell, one window, managed state | 2 |
| §1 in-scope: manual text input | 4 |
| §1 in-scope: `parse_text` over IPC | 3 |
| §1 in-scope: sentence pane | 4 |
| §1 in-scope: definition pane | 5 |
| §1 in-scope: light/dark theming | 4 (tokens), 5 (visual proof) |
| §1 in-scope: `Serialize` on result types | 1 |
| §1 in-scope: hints via `TA_HINTS_DICT` | 2 (load), 3 (apply) |
| §1 window geometry 720×480 / min 480×320 | 2 (`tauri.conf.json`) |
| §1 native decorations, no custom titlebar | 2 (no `decorations` key) |
| §2 no feature gate on `serde` | 1 (unconditional derives) |
| §2.1 `WordFlags` as names | 1 |
| §2.2 names pinned by test | 1 (Steps 1, 8) |
| §3 module layout; only `commands.rs` of §6's five | 2, 3 |
| §3.1 `generations::latest` + `Index::open` at startup | 2 |
| §4 `spawn_blocking`, no worker, no events | 3 |
| §4.1 hints fatal on bad path | 2 (`StartupError::Hints`) |
| §5 three distinct error states | 2 (tests), 4 (`startup_error` display) |
| §6 Swiss direction, chips, unmatched visible, collapsed alternates | 4, 5 |
| §6 theming, no colour only in a media query | 4 (`tokens.css`) |
| §7 Rust tests, DOM tests, Playwright at two sizes, both themes | 2, 3, 4, 5 |
| §7.1 stubbed `invoke`, not WebDriver | 5 |
| §8 resolved facts consumed, not re-derived | "Resolved facts" |
| §9 invariants untouched | Global Constraints |
| §10 GPL header, no `unwrap`, formatting, clippy | 1–5 |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every code step carries runnable code; every test step a concrete expected value. Six steps direct the implementer to *verify and report* rather than guess — Task 2 Step 4 (the header filename), Task 2 Step 6 (`generations::latest`'s behavior on a corrupt generation), Task 2 Step 8 (which CI jobs compile), Task 3 Step 3 (`Option<State<'_, T>>` and the `JoinHandle` error), Task 4 Step 1 (`SenseData`'s fields), and Task 5 Step 6 (the Tauri invoke hook). Each names the exact uncertainty and how to resolve it. Task 5 Step 9 requires a decision and its rationale rather than prescribing one, because the right answer depends on what the baselines look like on the machine.

**3. Type consistency across task boundaries.** Checked:

- `WordFlags` serializes to an array of eight strings (Task 1); `FlagName` in `src/types.ts` (Task 4) lists the same eight — match.
- `ParseResult`/`Segment`/`Entry` field names in the Rust wire test (Task 1) match the TypeScript interfaces (Task 4) and the fixture (Task 4 Step 2) — match.
- `AppState { index, table, hints }` (Task 2) is read by `parse_text` (Task 3) through `Arc<AppState>`; Task 2 Step 7 manages `Arc::new(s)` and Task 3's signature is `State<'_, Arc<AppState>>` — match.
- `StartupFailure(String)` (Task 2) is read by `startup_error` (Task 3) and displayed by `showStartupError` (Task 4) — match.
- `renderSentence` and `renderDefinitions` both take `ParseResult` and return `HTMLElement` (Tasks 4, 5) — match.
- `data-start` is written by both renderers and read by the chip-click handler (Tasks 4, 5) — match.
- `run_parse(Option<&Index>, &ConjugationTable, &str, Option<&VibratoTokenizer>)` is called with those exact types by both its test and `parse_text` (Task 3) — match.

**4. Residual risks a human should look at.**

- **CI is the most likely thing to break.** Adding `src-tauri` to the workspace means several existing jobs now compile Tauri and need webkit2gtk. Task 2 Step 8 handles it but requires reading the real file rather than trusting this plan's job list.
- **Playwright baselines are platform-dependent.** Task 5 Step 9 names the decision and requires a recorded rationale, but does not make it — the right choice depends on inspecting the actual output.
- **Task 3's `Option<State<'_, T>>` may not be valid Tauri.** If it is not, the fallback (managing `StartupFailure` unconditionally) touches Task 2's `setup`, so this could bounce a completed task. It is called out in Task 3 Step 3 with the alternative spelled out.
- **The end-to-end gap is real and stated in spec §7.1.** Nothing in this phase proves a real `ParseResult` survives a real `invoke`. Both sides are tested; the seam is not.
- **`src-tauri` has no coverage floor**, deliberately. Task 5 Step 8 reports the number instead.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-14-jparser-phase2d.md`. Two execution options:

**1. Subagent-Driven (recommended)** — a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
