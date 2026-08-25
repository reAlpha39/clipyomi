# JParser Phase 4A — Chrome Modes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `decorations` boolean with a three-way `chrome_mode`, add the content-only window state, and give it a right-click menu as the reach it removes.

**Architecture:** Content-only is header mode with the hover reveal switched off — the reserved row, the region clip, the `HTCAPTION` drag strip and the resize edges all already exist for header mode, so the new state needs no Win32 code. Windows has no native-frame state at all (`WS_CAPTION` is stripped for the window's life), so it offers two modes and resolves `native` to `header` through one pure function, which lets four dead Windows-only code paths go with it.

**Tech Stack:** Rust + Tauri 2 (`src-tauri`), vanilla TypeScript + Vite (`src`), Vitest, Playwright, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-08-25-jparser-phase4a-design.md`

## Global Constraints

- **The hidden-title-bar mode and its hover reveal are out of scope and must come out of this phase behaving exactly as they do now** (spec §2, §4.5). `e2e/panes.spec.ts` must pass with **zero screenshot baselines regenerated**. A baseline that needs refreshing is a regression, not a rebase — stop and find out why.
- Untouched by every task: `peek_titlebar`, `set_strip_shown`, `apply_round_region`, `WM_NCHITTEST`, `WM_NCCALCSIZE`, `apply_decorations_macos`, `set_titlebar_chrome`, `PEEK_HIDE_MS`, `BAND_HEIGHT`, `--band-h`, `peek_grows_frame`, and the geometry debounce that skips a resize while peeking.
- `BAND_HEIGHT_LOGICAL` (Rust) = `BAND_HEIGHT` (TS) = `--band-h` (CSS) = **28**. Never restate the number; reference the constant.
- Mode names on the wire are exactly `"native"`, `"header"`, `"content"` — lowercase, in `settings.json`, in `data-chrome`, and in the `set_chrome_mode` argument.
- Every source file starts with the existing GPL v2 header comment. New Rust and TS files copy it verbatim from a sibling.
- Commit messages: Conventional Commits (`feat`, `fix`, `docs`, `test`, `refactor`, `chore`), subject ≤ 50 chars, body in normal prose explaining *why*.
- The chrome commands resolve the main window **by label** via `chrome_target(&app)`, never from the injected `tauri::Window` — the settings window invokes the same commands.
- Run `npm test` (Vitest), `npm run build` (tsc + vite), `cargo test`, and `cargo clippy --all-targets` from `src-tauri` before every commit. `cargo clippy` has four pre-existing Win32 acronym warnings; those are the only ones allowed.
- `.gitignore` has an uncommitted local change. Never `git add .` or `git add -A` — stage explicit paths only.

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `src-tauri/src/commands.rs` | modify — spike removal, `set_chrome_mode`, the Windows deletions | 1, 3, 4 |
| `src-tauri/src/main.rs` | modify — startup restore, handler list, spike call | 1, 3, 4 |
| `src-tauri/src/settings.rs` | modify — `ChromeMode`, `supported`, `native_supported`, `normalize`, the `Settings` fields | 2, 3 |
| `xtask/src/probe.rs` | delete | 1 |
| `xtask/src/main.rs` | modify — drop the probe arms | 1 |
| `tools/probe-strip.mjs`, `tools/probe-appearance.mjs`, `tools/probe-shot.ps1` | delete | 1 |
| `src/types.ts` | modify — `ChromeMode`, `Settings.chrome_mode` replaces `decorations` | 3 |
| `src/main.ts` | modify — `applyChromeMode`, the peek gate, the menu wiring | 3, 7 |
| `src/settings.ts` | modify — the mode wiring, then the segmented control | 3, 5 |
| `src/styles/global.css` | modify — selector swap, the content rule, the menu rules | 3, 4, 7 |
| `src/context-menu.ts` | **create** — `clampMenu` (pure) and the menu DOM | 6, 7 |
| `src/context-menu.test.ts` | **create** | 6, 7 |
| `src/main.test.ts`, `src/settings.test.ts` | modify — mode assertions | 3, 4, 5, 7 |
| `e2e/stub.ts`, `e2e/panes.spec.ts` | modify — settings key rename only | 3 |
| `e2e/chrome.spec.ts` | **create** — the three modes and the menu | 8 |
| `docs/superpowers/specs/2026-08-12-jparser-port-design.md` | modify — phase table, deviations 10–12 | 9 |
| `docs/superpowers/2026-08-25-jparser-phase4a-handoff.md` | **create** | 9 |

---

## Task 1: Revert the throwaway Windows flash spike

Spec §7. This goes first so the 4A diff is readable, and because the spike's injected JS is keyed off `data-decorations`, which Task 3 renames. Both routes it measured were refuted; the numbers now live in the spec.

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `xtask/src/main.rs`
- Delete: `xtask/src/probe.rs`
- Delete: `tools/probe-strip.mjs`, `tools/probe-appearance.mjs`, `tools/probe-shot.ps1`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing. Pure deletion — later tasks only benefit from the smaller file.

**Keep, do not delete:** `update_clip_region` (a live command, called from `main.ts` on `tauri://scale-change`), `STRIP_SHOWN` and its doc comment including the 39 %-vs-1 % measurement, `WM_ERASEBKGND`'s background fill, `PEEKED`, and `tools/extract_jmdict_subset.py`.

- [ ] **Step 1: Delete the probe functions and statics in `commands.rs`**

Delete each of these whole items, including their doc comments:

- in `mod win32_frameless`: `static PROBE_FLUSH`, `pub fn probe_clip`, `pub fn probe_wait_compose`, `pub fn probe_window_rect`
- at module level: `fn probe_qpc`, `static PROBE_PAINTS`, `static PROBE_ACTIVE`, `fn probe_log_toggle`, `fn probe_log_toggle_at`, `fn probe_note_paint`, `pub fn probe_autotoggle`, and the `#[tauri::command] pub fn probe_region_clip`

- [ ] **Step 2: Remove the probe hooks from the live code paths**

In `mod win32_frameless`'s `extern "system"` block, delete the two lines:

```rust
        /// THROWAWAY PROBE (spike, revert me).
        fn DwmFlush() -> i32;
```

In the constants near the top of the module, delete:

```rust
    /// THROWAWAY PROBE (spike, revert me).
    const WM_PAINT: u32 = 0x000F;
```

In `frameless_subclass_proc`, delete this whole arm:

```rust
            // THROWAWAY PROBE (spike, revert me): noted, then handed straight
            // on — the parent rarely paints, so seeing one at all near a reveal
            // is itself the signal.
            WM_PAINT => {
                super::probe_note_paint(2);
            }
```

In the same function's `WM_ERASEBKGND` arm, delete only these four lines, leaving the `GetClientRect` / `FillRect` body intact:

```rust
                // THROWAWAY PROBE (spike, revert me): whether a reveal's flash
                // coincides with a paint cycle decides which fix is the right
                // one, so the tick goes in a buffer here and the probe thread
                // writes it out. A push is all this arm can afford.
                super::probe_note_paint(1);
```

In `peek_titlebar`, the Windows block becomes exactly:

```rust
    #[cfg(target_os = "windows")]
    {
        let _ = height;
        win32_frameless::set_strip_shown(&window, visible);
    }
```

- [ ] **Step 3: Remove the probe from `main.rs`**

Delete this block from `setup`:

```rust
            // THROWAWAY PROBE (spike, revert me).
            #[cfg(target_os = "windows")]
            commands::probe_autotoggle(app.handle().clone());
```

And delete this line from `invoke_handler`'s list:

```rust
            commands::probe_region_clip,
```

- [ ] **Step 4: Remove the probe from `xtask`**

Delete `xtask/src/probe.rs`. In `xtask/src/main.rs` delete `#[cfg(windows)] mod probe;`, the five `probe-*` argument arms in `main`, the second `eprintln!` usage line mentioning `probe-sample`, and the two `THROWAWAY PROBE` comment lines in the module doc and in `main`. The module doc's remaining line reads:

```rust
//! One-off asset conversion: ta-old's UTF-16LE Conjugations.txt to UTF-8 JSON.
//!
//! Run with: cargo run -p xtask -- convert-conjugations
```

- [ ] **Step 5: Delete the probe harness scripts**

```bash
git rm tools/probe-strip.mjs tools/probe-appearance.mjs tools/probe-shot.ps1
```

- [ ] **Step 6: Verify nothing references the probe and everything still builds**

```bash
git grep -n "probe_\|PROBE_\|THROWAWAY" -- src-tauri xtask tools src e2e
```

Expected: no output.

```bash
cd src-tauri && cargo test && cargo clippy --all-targets && cd ..
npm test
npm run build
```

Expected: `cargo test` green, `cargo clippy` clean but for the four pre-existing Win32 acronym warnings, Vitest green, build clean.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs xtask/src/main.rs xtask/src/probe.rs tools/probe-strip.mjs tools/probe-appearance.mjs tools/probe-shot.ps1
git commit -m "refactor(windows): revert the refuted flash spike"
```

Body:

```
Both routes it measured were refuted: DwmFlush on the main thread made
the flash eleven times more likely and dragged the sampler from 144Hz to
80Hz, and moving the same wait off-thread was three times worse than
nothing. The numbers are recorded in the phase 4A design spec, so the
code carrying them can go.

Its injected JS was also keyed off `data-decorations`, which 4A renames,
so it breaks in this phase whichever way. Going first keeps the 4A diff
readable.

STRIP_SHOWN and its 39%-vs-1% measurement stay: that one justifies the
shipped design rather than a rejected one.
```

---

## Task 2: `ChromeMode`, `supported`, `native_supported`

Spec §4.2, §4.3. Pure addition — nothing calls it yet, so this task cannot change behaviour and is reviewable on its own.

**Files:**
- Modify: `src-tauri/src/settings.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum ChromeMode { Native, Header, Content }` — `Copy`, `Eq`, `Default` (default `Native`), serde `rename_all = "lowercase"`.
  - `pub fn supported(mode: ChromeMode, native_supported: bool) -> ChromeMode`
  - `pub fn native_supported() -> bool`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `src-tauri/src/settings.rs`:

```rust
    /// The wire names are the strings that appear in settings.json, in
    /// `data-chrome`, and in the `set_chrome_mode` argument. Pinned as strings
    /// because three surfaces spell them independently.
    #[test]
    fn chrome_modes_serialize_as_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&ChromeMode::Native).unwrap(),
            "\"native\""
        );
        assert_eq!(
            serde_json::to_string(&ChromeMode::Header).unwrap(),
            "\"header\""
        );
        assert_eq!(
            serde_json::to_string(&ChromeMode::Content).unwrap(),
            "\"content\""
        );
        assert_eq!(
            serde_json::from_str::<ChromeMode>("\"content\"").unwrap(),
            ChromeMode::Content
        );
    }

    #[test]
    fn the_default_mode_is_native() {
        assert_eq!(ChromeMode::default(), ChromeMode::Native);
    }

    /// Windows has no native-frame state at all, so a setting naming it has to
    /// resolve to the nearest shape that exists rather than draw nothing.
    /// Parameterised instead of `cfg`-gated so both arms run on both platforms.
    #[test]
    fn native_resolves_to_header_where_it_is_unsupported() {
        assert_eq!(supported(ChromeMode::Native, false), ChromeMode::Header);
        assert_eq!(supported(ChromeMode::Native, true), ChromeMode::Native);
    }

    #[test]
    fn the_other_two_modes_are_supported_everywhere() {
        for supported_here in [true, false] {
            assert_eq!(
                supported(ChromeMode::Header, supported_here),
                ChromeMode::Header
            );
            assert_eq!(
                supported(ChromeMode::Content, supported_here),
                ChromeMode::Content
            );
        }
    }

    #[test]
    fn only_windows_lacks_the_native_frame() {
        assert_eq!(native_supported(), !cfg!(target_os = "windows"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test --lib settings 2>&1 | head -30; cd ..
```

Expected: compile errors — `cannot find type ChromeMode`, `cannot find function supported`, `cannot find function native_supported`.

- [ ] **Step 3: Write the implementation**

Insert into `src-tauri/src/settings.rs` immediately after the `SETTINGS_FILE` constant and the `default_true` helper:

```rust
/// Which of §7.4's three window chrome shapes the main window wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChromeMode {
    /// The OS draws the title bar and its buttons. macOS and Linux only.
    #[default]
    Native,
    /// Frameless, our 28px band, revealed by a hover.
    Header,
    /// Frameless, our band never revealed. Maximum real estate over a game.
    Content,
}

/// Whether this platform can present `ChromeMode::Native`.
///
/// Windows cannot: `apply_dwm_attributes` and the `WM_STYLECHANGING` arm in
/// `commands.rs` strip `WS_CAPTION` for the window's whole life (`d07711f`,
/// `e45d55f`), which makes the in-app band the title bar there. Restoring the
/// OS caption re-opens the DWM shadow that `e45d55f` spent a commit removing,
/// so it is its own sub-phase.
pub fn native_supported() -> bool {
    !cfg!(target_os = "windows")
}

/// A mode the running platform can actually draw.
///
/// Takes the platform answer as a parameter rather than reading `cfg!` inside,
/// so both arms are testable wherever the suite runs — the same reason
/// `peek_grows_frame` is a function and not a `cfg!` at each use site.
pub fn supported(mode: ChromeMode, native_supported: bool) -> ChromeMode {
    match mode {
        ChromeMode::Native if !native_supported => ChromeMode::Header,
        other => other,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cd src-tauri && cargo test --lib settings && cargo clippy --all-targets && cd ..
```

Expected: all settings tests PASS; clippy clean but for the four pre-existing Win32 acronym warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "feat(settings): add the ChromeMode type"
```

Body:

```
The three window chrome shapes of the master spec §7.4, plus the one
pure function that resolves a mode the running platform cannot draw.

Windows has no native-frame state -- WS_CAPTION is stripped for the
window's whole life -- so `supported` pins it to `header` there.
Parameterised rather than cfg-gated inside, so both arms are testable
on either platform.

Nothing calls any of this yet.
```

---

## Task 3: The rename, end to end, behaviour unchanged

Spec §5, §6.1, §6.2, §4.5. The setting, the command, the attribute and the CSS all change vocabulary while the user-visible behaviour stays exactly what it is. **This is the invariance-proof task**: `e2e/panes.spec.ts` must pass with zero baselines regenerated.

The control in the settings window stays a **checkbox** here, mapping on/off to `native`/`header`. Task 5 replaces it. Three lines of throwaway wiring, deliberately: it keeps every commit in this phase a working app.

Compile-atomic across Rust and TS — the settings key cannot be renamed halfway — so this task is larger than its neighbours by nature.

**Files:**
- Modify: `src-tauri/src/settings.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`
- Modify: `src/types.ts`, `src/main.ts`, `src/settings.ts`, `src/styles/global.css`
- Modify: `src/main.test.ts`, `src/settings.test.ts`, `e2e/stub.ts`, `e2e/panes.spec.ts`

**Interfaces:**
- Consumes: `ChromeMode`, `supported`, `native_supported` from Task 2.
- Produces:
  - Rust: `Settings.chrome_mode: Option<ChromeMode>`, `Settings.decorations: Option<bool>` (legacy, never written), `settings::normalize(Settings) -> Settings`, `#[tauri::command] set_chrome_mode(mode: ChromeMode, app, settings)`.
  - TS: `export type ChromeMode = 'native' | 'header' | 'content'`, `Settings.chrome_mode?: ChromeMode`, and in `main.ts` the module-scope `chromeMode: ChromeMode` plus `applyChromeMode(mode: ChromeMode): void`.
  - DOM: `#app[data-chrome]` carrying the mode name. `#app[data-decorations]` ceases to exist.

- [ ] **Step 1: Write the failing Rust migration tests**

Append to `mod tests` in `src-tauri/src/settings.rs`:

```rust
    /// The pre-4A boolean folds into the new key, and the mode is pinned to
    /// what the platform can draw. `native_supported()` decides the second
    /// column of the spec's §5 table, which is why every expectation here is
    /// written against it rather than against a hardcoded mode.
    #[test]
    fn a_legacy_decorations_true_becomes_native_where_native_exists() {
        let s: Settings = serde_json::from_str(r#"{"decorations":true}"#).unwrap();
        let s = normalize(s);
        assert_eq!(
            s.chrome_mode,
            Some(supported(ChromeMode::Native, native_supported()))
        );
        assert_eq!(s.decorations, None, "the legacy key must be cleared");
    }

    #[test]
    fn a_legacy_decorations_false_becomes_header() {
        let s: Settings = serde_json::from_str(r#"{"decorations":false}"#).unwrap();
        assert_eq!(normalize(s).chrome_mode, Some(ChromeMode::Header));
    }

    #[test]
    fn a_file_with_neither_key_takes_the_default_mode() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            normalize(s).chrome_mode,
            Some(supported(ChromeMode::default(), native_supported()))
        );
    }

    /// Both keys present is what a downgrade-then-upgrade produces. The new
    /// one wins, or the user's last choice would be overwritten by a value
    /// this version stopped maintaining.
    #[test]
    fn the_new_key_wins_over_the_legacy_one() {
        let s: Settings =
            serde_json::from_str(r#"{"decorations":true,"chrome_mode":"content"}"#).unwrap();
        assert_eq!(normalize(s).chrome_mode, Some(ChromeMode::Content));
    }

    /// The migration is self-completing: the legacy key stops being written
    /// the first time anything saves, so it cannot come back and re-decide the
    /// mode on a later launch.
    #[test]
    fn the_legacy_key_is_gone_from_the_file_after_a_load_and_save() {
        let path = scratch("legacy-drop").join("settings.json");
        std::fs::write(&path, br#"{"decorations":false,"always_on_top":true}"#).expect("write");

        let (loaded, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("decorations"), "got {raw}");
        assert!(raw.contains("\"chrome_mode\": \"header\""), "got {raw}");
        assert!(raw.contains("\"always_on_top\": true"), "unrelated key lost");
    }

    #[test]
    fn chrome_mode_round_trips_through_the_file() {
        let path = scratch("chrome-round-trip").join("settings.json");
        let written = Settings {
            chrome_mode: Some(ChromeMode::Content),
            ..Default::default()
        };
        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert_eq!(read_back.chrome_mode, Some(ChromeMode::Content));
    }

    /// An unknown mode name is a parse failure, which `load` already turns into
    /// defaults plus a visible warning. Pinned so that fallback is deliberate
    /// rather than something a future serde attribute silently changes.
    #[test]
    fn an_unknown_mode_name_falls_back_to_defaults_with_a_warning() {
        let path = scratch("bad-mode").join("settings.json");
        std::fs::write(&path, br#"{"chrome_mode":"borderless"}"#).expect("write");

        let (s, warning) = load(&path);
        assert_eq!(
            s.chrome_mode,
            Some(supported(ChromeMode::default(), native_supported()))
        );
        assert!(warning.expect("a reason").contains("could not be read"));
    }

    /// `load` must normalize on every path, including the ones that never see
    /// a parsed file — otherwise a missing or corrupt file leaves `chrome_mode`
    /// unset and every reader downstream needs its own fallback.
    #[test]
    fn a_missing_file_still_arrives_with_a_mode_set() {
        let path = scratch("missing-mode").join("settings.json");
        let (s, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert!(s.chrome_mode.is_some());
    }
```

Then update the two existing tests that name the old field. In `default_decorations_is_true`, rename it and replace its first two lines:

```rust
    #[test]
    fn defaults_leave_every_optional_key_unset() {
        let s = Settings::default();
        assert_eq!(s.chrome_mode, None, "only `normalize` fills this in");
        assert_eq!(s.decorations, None);
        assert_eq!(s.furigana_mode, None);
```

(the remaining `assert_eq!` lines of that test stay as they are)

In `geometry_round_trips_through_json` replace `assert!(!s.decorations);` with:

```rust
        assert_eq!(s.decorations, Some(false), "read verbatim; `normalize` folds it");
```

In `settings_roundtrips_furigana_mode` and `settings_roundtrips_gloss_filters`, replace the `decorations: false,` field in each struct literal with:

```rust
            chrome_mode: Some(ChromeMode::Header),
            decorations: None,
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test --lib settings 2>&1 | head -30; cd ..
```

Expected: compile errors — `cannot find function normalize`, and `struct Settings has no field named chrome_mode`.

- [ ] **Step 3: Change the `Settings` fields and add `normalize`**

In `src-tauri/src/settings.rs`, replace the `decorations` field:

```rust
    #[serde(default = "default_true")]
    pub decorations: bool,
```

with:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome_mode: Option<ChromeMode>,
    /// Pre-4A, replaced by `chrome_mode`. Read once by `normalize` to seed the
    /// new key, then cleared — which is what makes the migration
    /// self-completing: it stops being written the first time anything saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decorations: Option<bool>,
```

In `impl Default for Settings`, replace `decorations: true,` with:

```rust
            chrome_mode: None,
            decorations: None,
```

Delete the now-unused `default_true` helper **only if** `clippy` reports it unused — `clipboard_monitoring` also uses it, so it stays.

Add `normalize` immediately above `load`:

```rust
/// Fold the pre-4A `decorations` boolean into `chrome_mode`, clear it, and pin
/// the result to a mode this platform can draw.
///
/// Done here, once, rather than behind an accessor: the same call
/// `initial_height`'s comment makes for geometry — one place converts and
/// nothing downstream has to remember to. After this runs, `chrome_mode` is
/// always `Some`, and `decorations` is always `None`.
fn normalize(mut settings: Settings) -> Settings {
    let from_legacy = match settings.decorations {
        Some(false) => ChromeMode::Header,
        // `Some(true)` and `None` alike: the pre-4A default was a title bar.
        _ => ChromeMode::default(),
    };
    let mode = settings.chrome_mode.unwrap_or(from_legacy);
    settings.chrome_mode = Some(supported(mode, native_supported()));
    settings.decorations = None;
    settings
}
```

Make `load` normalize on all three of its paths:

```rust
pub fn load(path: &Path) -> (Settings, Option<String>) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        // Any read failure is treated as absent. Distinguishing "not found" from
        // "unreadable" would change nothing: both mean we start from defaults.
        Err(_) => return (normalize(Settings::default()), None),
    };

    match serde_json::from_str(&raw) {
        Ok(settings) => (normalize(settings), None),
        Err(e) => (
            normalize(Settings::default()),
            Some(format!(
                "{} could not be read, using defaults: {e}",
                path.display()
            )),
        ),
    }
}
```

Update the module doc's first paragraph, which still says "two keys today":

```rust
//! Persisted user settings.
//!
//! Unknown keys survive a rewrite so a file written by a later version is not
//! silently truncated by this one, and `normalize` folds the pre-4A
//! `decorations` boolean into `chrome_mode` on the way in.
```

- [ ] **Step 4: Run the Rust settings tests to verify they pass**

```bash
cd src-tauri && cargo test --lib settings 2>&1 | tail -20; cd ..
```

Expected: every settings test PASS. `commands.rs` and `main.rs` still fail to compile — that is the next step.

- [ ] **Step 5: Rename the command and fix the Rust call sites**

In `src-tauri/src/commands.rs`, add `ChromeMode` to the settings import (find the existing `use crate::settings::{...}` line and add it), then replace the whole `set_decorations` function with:

```rust
#[tauri::command]
pub fn set_chrome_mode(
    mode: ChromeMode,
    app: tauri::AppHandle,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    // Resolved before it is applied or persisted, so an out-of-date webview
    // cannot write a mode this platform will not honour.
    let mode = crate::settings::supported(mode, crate::settings::native_supported());
    let window = chrome_target(&app)?;
    #[cfg(target_os = "macos")]
    apply_decorations_macos(&window, mode == ChromeMode::Native)?;
    #[cfg(target_os = "windows")]
    {
        // No `grow_top` here any more. It existed to shed and regain the
        // reserved row across the native boundary, and `supported` above means
        // Windows never crosses it — `header` and `content` reserve the row
        // identically, so switching between them must move nothing. Task 4
        // deletes the function itself.
        win32_frameless::init_window(&window);
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        let _ = window.set_decorations(mode == ChromeMode::Native);
    }
    settings
        .update(|s| s.chrome_mode = Some(mode))
        .map_err(|e| e.to_string())
}
```

In `src-tauri/src/main.rs`, replace the macOS startup arm:

```rust
                #[cfg(target_os = "macos")]
                if !loaded.decorations {
                    let _ = commands::apply_decorations_macos(&main_window.as_ref().window(), false);
                }
```

with:

```rust
                #[cfg(target_os = "macos")]
                if loaded.chrome_mode != Some(settings::ChromeMode::Native) {
                    let _ = commands::apply_decorations_macos(&main_window.as_ref().window(), false);
                }
```

and the `initial_height` call:

```rust
                    let height =
                        commands::initial_height(loaded.window_height, loaded.decorations);
```

with:

```rust
                    let height = commands::initial_height(
                        loaded.window_height,
                        loaded.chrome_mode == Some(settings::ChromeMode::Native),
                    );
```

and in `invoke_handler`, `commands::set_decorations,` becomes `commands::set_chrome_mode,`.

- [ ] **Step 6: Verify Rust builds and every test passes**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets && cd ..
```

Expected: green, clippy clean but for the four pre-existing Win32 acronym warnings.

- [ ] **Step 7: Write the failing frontend tests**

In `src/main.test.ts`, replace the three existing `data-decorations` assertions. Find each `getAttribute('data-decorations')` and change the attribute name and expected value: `'false'` becomes `'header'`, `'true'` becomes `'native'`. Then add these tests inside the same `describe`:

```ts
  test('the chrome mode from get_settings lands on #app', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    await vi.waitFor(() =>
      expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('header'),
    );
  });

  test('a settings-changed event re-applies the chrome mode', async () => {
    await import('./main');
    await vi.waitFor(() => expect(listeners.has('settings-changed')).toBe(true));
    emit('settings-changed', { chrome_mode: 'header' });
    expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('header');
  });

  // The band boots decorated and the attribute is set synchronously, before
  // `get_settings` resolves. The CSS keys the frameless rules off
  // `:not([data-chrome='native'])`, so an absent attribute would match them and
  // flash the frameless shape for a frame.
  test('#app carries a chrome mode before any settings arrive', async () => {
    await import('./main');
    expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('native');
  });
```

In `src/settings.test.ts`, find every place the fixture object or an assertion uses `decorations` and change it to `chrome_mode`, with `true` becoming `'native'` and `false` becoming `'header'`. Add:

```ts
  test('the title bar checkbox reflects and writes the chrome mode', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' } as Settings);
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);
    const box = container.querySelector<HTMLInputElement>('#setting-decorations')!;
    await vi.waitFor(() => expect(box.checked).toBe(false));

    box.checked = true;
    box.dispatchEvent(new Event('change'));
    expect(invoke).toHaveBeenCalledWith('set_chrome_mode', { mode: 'native' });
    expect(emitted.at(-1)?.[1]).toMatchObject({ chrome_mode: 'native' });
  });
```

- [ ] **Step 8: Run the frontend tests to verify they fail**

```bash
npm test 2>&1 | tail -30
```

Expected: the new tests FAIL — `data-chrome` is null, and `set_chrome_mode` was never invoked.

- [ ] **Step 9: Rename the types**

In `src/types.ts`, add above the `Settings` interface:

```ts
/** Which of the three window chrome shapes the main window wears (spec §7.4). */
export type ChromeMode = 'native' | 'header' | 'content';
```

and in `Settings`, replace `decorations: boolean;` with:

```ts
  chrome_mode?: ChromeMode;
```

- [ ] **Step 10: Rewrite the chrome state in `main.ts`**

Add `ChromeMode` to the type import from `./types`. Replace the `titlebarShown` declaration and its doc comment:

```ts
/**
 * Which chrome shape the window is wearing. The control for it lives in the
 * settings window and in the right-click menu; the main window still needs the
 * value, because it decides both shapes of the band — a reserved 28px row in
 * `native`, a floating overlay over the sentence in the other two — and the CSS
 * keys every one of those rules off `#app[data-chrome]`.
 */
let chromeMode: ChromeMode = 'native';
```

In `showPeek`, replace the guard line:

```ts
  if (titlebarShown || peeking) return;
```

with:

```ts
  // Only `header` reveals. `native` has nothing to reveal — the chrome is
  // already there — and `content` deliberately never shows it (spec §3).
  if (chromeMode !== 'header' || peeking) return;
```

Replace `applyDecorations` and its call:

```ts
function applyChromeMode(mode: ChromeMode): void {
  chromeMode = mode;
  app.setAttribute('data-chrome', mode);
  // A peek still notionally in flight when the user leaves `header` is now
  // wrong either way: `native` has shown the chrome for real, and `content`
  // wants none. The flag has to drop here or the `pointerleave` that follows
  // would undo chrome the user just asked for.
  if (mode !== 'header') {
    window.clearTimeout(peekHide);
    peekHide = undefined;
    peeking = false;
    app.classList.remove('peeked', 'peek-offset');
  }
}

applyChromeMode('native');
```

In `applySettings`, replace:

```ts
  if (settings.decorations !== undefined) {
    applyDecorations(settings.decorations);
  }
```

with:

```ts
  if (settings.chrome_mode !== undefined) {
    applyChromeMode(settings.chrome_mode);
  }
```

and in the `settings-changed` listener, replace:

```ts
  if (settings.decorations !== undefined) applyDecorations(settings.decorations);
```

with:

```ts
  if (settings.chrome_mode !== undefined) applyChromeMode(settings.chrome_mode);
```

- [ ] **Step 11: Rewire `settings.ts` — three lines, checkbox unchanged**

The markup and the `#setting-decorations` element stay exactly as they are; only what they read and write changes. Task 5 replaces the control itself.

Add `ChromeMode` to the type import. Add below `let currentMode: FuriganaMode = 'none';`:

```ts
  // Task 5 replaces the checkbox with a three-way control. Until then this
  // maps the two states it can express, so the phase has no commit where the
  // title-bar toggle does nothing.
  let currentChrome: ChromeMode = 'native';
```

In `currentSnapshot`, replace `decorations: decorations.checked,` with:

```ts
      chrome_mode: currentChrome,
```

In `applySettings`, replace `if (s.decorations !== undefined) decorations.checked = s.decorations;` with:

```ts
    if (s.chrome_mode !== undefined) {
      currentChrome = s.chrome_mode;
      decorations.checked = s.chrome_mode === 'native';
    }
```

Replace the `decorations` change handler:

```ts
  decorations.addEventListener('change', () => {
    currentChrome = decorations.checked ? 'native' : 'header';
    void invoke('set_chrome_mode', { mode: currentChrome }).catch(() => {});
    saveAndBroadcast();
  });
```

- [ ] **Step 12: Swap the CSS selectors**

In `src/styles/global.css`, replace every occurrence of `[data-decorations='false']` with `:not([data-chrome='native'])`. There are **twelve**, spread across eight rules (four of the rules are comma-separated selector lists, which is where the extra occurrences come from).

`:not()` carries the specificity of its argument, so each selector keeps the exact weight it had — which is what lets the committed screenshot baselines stay valid. And `header` and `content` both match it, which is the whole reason for the negative form: the two frameless modes share every band rule (spec §6.2).

Verify with a diff-count before and after:

```bash
git grep -c "data-decorations" -- src/styles/global.css
```

Expected before the edit: `src/styles/global.css:12`. After it: no output.

Then update the comment above the last pair of rules, which still names the old attribute:

```css
/* Frameless modes: idle hides the controls; a peek reveals them. `header` and
   `content` share this rule — `content` simply never gets `.peeked`, and Task 4
   takes its controls out of the tab order on top of this. */
```

- [ ] **Step 13: Rename the settings key in the e2e stubs**

In `e2e/stub.ts`, change the `get_settings` answer and the command name in the pass-through list:

```js
      if (cmd === 'get_settings') return { always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' };
```

```js
        cmd === 'set_chrome_mode' ||
```

In `e2e/panes.spec.ts`, in both `HIDDEN_TITLEBAR_STUB` and `WINDOWS_STUB`, replace `{ decorations: false }` with `{ chrome_mode: 'header' }`. Update the two comments that name the old attribute:

- the block comment above `HIDDEN_TITLEBAR_STUB`: "With `decorations: false`" becomes "In `header` mode".
- the inline comment inside the hidden-title-bar test: "the same `#app[data-decorations='false']` rule" becomes "the same `#app:not([data-chrome='native'])` rule".

- [ ] **Step 14: Run everything, and hold the invariance gate**

```bash
npm test
npm run build
npx playwright test e2e/panes.spec.ts
git status --porcelain e2e/panes.spec.ts-snapshots
git grep -n "data-decorations\|set_decorations\|applyDecorations\|titlebarShown" -- src src-tauri e2e
```

Expected: Vitest green, build clean, `panes.spec.ts` green, **`git status` on the snapshots directory prints nothing**, and the final grep prints nothing.

If a baseline changed, the CSS swap altered a computed style. Stop, diff the selector specificity, and fix it rather than accepting the new image.

- [ ] **Step 15: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/commands.rs src-tauri/src/main.rs src/types.ts src/main.ts src/settings.ts src/styles/global.css src/main.test.ts src/settings.test.ts e2e/stub.ts e2e/panes.spec.ts
git commit -m "refactor(chrome): replace decorations with chrome_mode"
```

Body:

```
Same two shapes, new vocabulary. `settings.json` speaks `chrome_mode`,
the command is `set_chrome_mode`, and `#app` carries `data-chrome`
instead of `data-decorations`. `normalize` folds the old boolean in on
load and clears it, so the migration completes on the first save.

The CSS swap is `[data-decorations='false']` to
`:not([data-chrome='native'])`. `:not()` takes the specificity of its
argument, so every rule keeps the weight it had -- and the negative form
is what lets `header` and `content` share the band rules in the next
commit. Held to zero regenerated screenshot baselines, which is the
proof that the hover-reveal mode is untouched.

One behaviour does change, and it is the point: Windows can no longer
enter the decorated state, which shrank the window 28px and then clipped
the band it had just reserved, leaving no reachable gear. `supported`
pins it to `header` there.

The settings checkbox still reads and writes only two of the three
modes. The three-way control replaces it two commits from here.
```

---

## Task 4: Content-only mode, and reaping the dead Windows code

Spec §3, §4.4, §4.5. The third mode becomes real, and the four Windows code paths that only the native boundary used are deleted. Reachable at this point only by hand-editing `settings.json`; Task 5 gives it a control.

**Files:**
- Modify: `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`
- Modify: `src/styles/global.css`, `src/main.test.ts`

**Interfaces:**
- Consumes: `ChromeMode`, `set_chrome_mode`, `applyChromeMode` from Tasks 2–3.
- Produces: `pub fn initial_height(saved: Option<u32>) -> u32` — one parameter, not two. `grow_top` and `top_grown` no longer exist.

- [ ] **Step 1: Write the failing tests**

In `src-tauri/src/commands.rs`'s `mod tests`, replace `initial_height_adds_the_reserved_row_only_on_a_first_undecorated_launch` entirely with:

```rust
    /// The reserved row is Windows-only and unconditional there now: `native`
    /// does not exist on that platform, and `header` and `content` both reserve
    /// the row (spec §4.4). Saved values are the real box already, so they pass
    /// through untouched — that is what keeps `save_window_geometry` a fixed
    /// point.
    #[test]
    fn initial_height_reserves_the_band_row_only_on_a_first_windows_launch() {
        assert_eq!(initial_height(Some(508)), 508);
        if cfg!(target_os = "windows") {
            assert_eq!(
                initial_height(None),
                DEFAULT_WINDOW_HEIGHT + BAND_HEIGHT_LOGICAL
            );
        } else {
            assert_eq!(initial_height(None), DEFAULT_WINDOW_HEIGHT);
        }
    }
```

Add to `src/main.test.ts`. **These three are regression guards, not red tests** — Task 3's `chromeMode !== 'header'` gate already satisfies them, and they exist so a later change to `showPeek` cannot quietly let content mode reveal. Say so in the commit rather than pretending they failed first. The `visibility` half of §3.2 has no red test available here at all: happy-dom resolves no styles, which is why Task 8 test 2 exists.

```ts
  test('content mode does not peek', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'content' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('content'));

    // The reveal is header mode's alone. happy-dom resolves no layout, so the
    // proof here is the absent backend call and the absent class, not a height.
    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(false);
    expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', expect.anything());
  });

  test('header mode still peeks', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('header'));

    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(true);
    expect(invoke).toHaveBeenCalledWith('peek_titlebar', { visible: true, height: 28 });
  });

  test('switching to content mid-peek drops the peek', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('header'));
    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(true);

    emit('settings-changed', { chrome_mode: 'content' });
    expect(appEl.classList.contains('peeked')).toBe(false);
    expect(appEl.classList.contains('peek-offset')).toBe(false);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cd src-tauri && cargo test --lib initial_height 2>&1 | head -20; cd ..
npm test 2>&1 | tail -20
```

Expected: Rust **fails to compile** — `initial_height` takes two arguments, and the test calls it with one. That is this task's red test.

Expected: the three new Vitest tests **pass immediately**, for the reason stated in Step 1. If any of them fails, Task 3's peek gate is wrong — fix that before continuing, because it means header mode's reveal is reachable from a mode that must never show it.

- [ ] **Step 3: Collapse `initial_height`**

In `src-tauri/src/commands.rs`, replace the function and its doc comment:

```rust
/// The height to give the window when nothing has been saved yet.
///
/// The reserved row is part of the window's real box and is persisted that way —
/// converting between a "visible" and a "real" box on every save and restore was
/// tried first and is a drift hazard: `save_window_geometry` is a fixed point
/// today (saving a saved value changes nothing), and subtracting a band on each
/// save breaks that the moment anything re-saves a stored value. So the only
/// place the band is added is here, once, when there is no saved height to
/// honour.
///
/// No mode parameter: the row is reserved on Windows and nowhere else, and on
/// Windows every reachable mode reserves it (spec §4.4).
pub fn initial_height(saved: Option<u32>) -> u32 {
    match saved {
        Some(h) => h,
        None if cfg!(target_os = "windows") => DEFAULT_WINDOW_HEIGHT + BAND_HEIGHT_LOGICAL,
        None => DEFAULT_WINDOW_HEIGHT,
    }
}
```

In `src-tauri/src/main.rs`, the call becomes:

```rust
                    let height = commands::initial_height(loaded.window_height);
```

- [ ] **Step 4: Delete the dead Windows code**

In `mod win32_frameless`:

Replace `region_top_inset` with:

```rust
    /// How much of the window's top the region clips away.
    ///
    /// Zero while the band is peeked, `strip_px` otherwise. There is no
    /// decorated case to check any more: `native` does not exist on Windows
    /// (spec §4), so the row is always reserved and always clipped at rest.
    /// This used to read `WS_CAPTION` off the live window to answer that
    /// question, which could only ever answer "undecorated" — the
    /// `WM_STYLECHANGING` arm below strips the bit for the window's life.
    fn region_top_inset(dpi: i32) -> i32 {
        if STRIP_SHOWN.load(Ordering::Relaxed) {
            0
        } else {
            strip_px(dpi)
        }
    }
```

Update its two callers to drop the `hwnd` argument: in `visual_top`, `window_top + region_top_inset(dpi)`; in `apply_round_region`, `let top = region_top_inset(dpi_val);`. `visual_top` keeps its `hwnd` parameter only if something else uses it — if `cargo clippy` reports it unused, drop the parameter there too and fix its single call site in `WM_NCHITTEST`.

Delete `pub fn grow_top` and `fn top_grown` entirely, along with these three tests in the module's `mod tests`: `a_top_grow_leaves_the_bottom_edge_where_it_is`, `repeated_grow_and_shrink_cycles_do_not_drift`, and any remaining test naming `top_grown`. Keep `the_valid_rect_is_what_the_old_and_new_client_share` — `valid_rect` is still live in `WM_NCCALCSIZE`.

If `GetWindowRect`, `GetWindowLongPtrW`, `WS_CAPTION` or `GWL_STYLE` become unused, `cargo clippy` will say so. `GetWindowRect` is still used by `set_strip_shown` and `apply_dwm_attributes`; `WS_CAPTION` and `GWL_STYLE` are still used by `frameless_style`, `apply_dwm_attributes` and the `WM_STYLECHANGING` arm. Delete only what clippy actually flags.

- [ ] **Step 5: Add the content-mode CSS**

Append to `src/styles/global.css`, immediately after the frameless-mode opacity rules:

```css
/* Content-only mode (spec §3.2). The band stays — it is the drag region, and
   on Windows its row is reserved either way — but nothing inside it is
   reachable, and it never peeks.

   `visibility`, not `opacity`: an `opacity: 0` button is still a tab stop, and
   this band never reveals, so tabbing would move focus onto controls that can
   never become visible. That is the trap 4C had to fix for the history nav,
   with the difference that there the fix was to reveal and here there is
   nothing to reveal. `visibility: hidden` takes all three out of the tab order
   and out of hit-testing in one property. */
#app[data-chrome='content'] .history-controls,
#app[data-chrome='content'] #settings-toggle,
#app[data-chrome='content'] .window-controls {
  visibility: hidden;
}
```

- [ ] **Step 6: Run everything, and hold the invariance gate again**

```bash
cd src-tauri && cargo test && cargo clippy --all-targets && cd ..
npm test
npm run build
npx playwright test e2e/panes.spec.ts
git status --porcelain e2e/panes.spec.ts-snapshots
```

Expected: all green, clippy clean but for the four pre-existing Win32 acronym warnings, and **no snapshot file modified**.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs src/styles/global.css src/main.test.ts
git commit -m "feat(chrome): add content-only mode"
```

Body:

```
Content-only is header mode with the reveal switched off. The reserved
row, the region clip, the HTCAPTION drag strip and the resize edges all
already exist for header mode, so the new state needs no Win32 code and
no geometry difference from its neighbour -- switching between the two
moves nothing on screen, and drag and resize keep working with nothing
painted above the sentence.

Its controls hide with `visibility`, not `opacity`. An opacity-zero
button is still a tab stop, and this band never reveals, so the 4C
history-nav trap would come back in a form no hover could fix.

With no native boundary left on Windows, four things lose their last
caller and go with it: region_top_inset's WS_CAPTION read, which could
only ever answer "undecorated" because WM_STYLECHANGING strips the bit
for the window's life; grow_top and top_grown, whose only caller was the
decorated toggle; and initial_height's mode parameter, since every
reachable Windows mode reserves the row.
```

---

## Task 5: The three-way control in the settings window

Spec §6.3. Replaces Task 3's checkbox, and is the point at which content mode becomes reachable without editing a file.

**Files:**
- Modify: `src/settings.ts`, `src/settings.test.ts`

**Interfaces:**
- Consumes: `ChromeMode` and `set_chrome_mode` from Task 3.
- Produces: `[data-chrome-mode]` buttons inside `.segmented-control[aria-label="Window chrome"]`. `#setting-decorations` no longer exists.

- [ ] **Step 1: Write the failing tests**

Replace the `the title bar checkbox reflects and writes the chrome mode` test from Task 3 with:

```ts
  test('the chrome control reflects the persisted mode', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'content' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);

    const checked = () =>
      container.querySelector<HTMLButtonElement>('[data-chrome-mode][aria-checked="true"]')
        ?.dataset.chromeMode;
    await vi.waitFor(() => expect(checked()).toBe('content'));
  });

  test('picking a mode invokes set_chrome_mode and broadcasts it', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'native' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_settings'));

    container.querySelector<HTMLButtonElement>('[data-chrome-mode="header"]')!.click();

    expect(invoke).toHaveBeenCalledWith('set_chrome_mode', { mode: 'header' });
    expect(emitted.at(-1)?.[1]).toMatchObject({ chrome_mode: 'header' });
    expect(
      container
        .querySelector<HTMLButtonElement>('[data-chrome-mode="header"]')!
        .getAttribute('aria-checked'),
    ).toBe('true');
  });

  // Windows has no native-frame state (spec §4), so offering it would name a
  // shape the platform cannot draw — and the backend would resolve it back to
  // `header` anyway, leaving the control showing a mode the window is not in.
  test('the native option is absent on Windows', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' } as Settings);
      if (cmd === 'platform') return Promise.resolve('windows');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);

    await vi.waitFor(() =>
      expect(container.querySelector('[data-chrome-mode="native"]')).toBeNull(),
    );
    expect(container.querySelector('[data-chrome-mode="header"]')).not.toBeNull();
    expect(container.querySelector('[data-chrome-mode="content"]')).not.toBeNull();
  });

  test('the native option survives on macOS', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'native' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('platform'));
    expect(container.querySelector('[data-chrome-mode="native"]')).not.toBeNull();
  });
```

Also delete any remaining `#setting-decorations` reference in this file.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test src/settings.test.ts 2>&1 | tail -20
```

Expected: FAIL — `[data-chrome-mode]` matches nothing.

- [ ] **Step 3: Replace the checkbox with a segmented control**

In `src/settings.ts`, delete this row from the "Window & Behavior" section:

```html
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-decorations" />
          <span class="label-text">Title bar &amp; window borders</span>
        </label>
```

and add a new section immediately after that `</section>`:

```html
      <section class="settings-group">
        <h2 class="settings-group-title">Window Chrome</h2>
        <div class="segmented-control" role="radiogroup" aria-label="Window chrome">
          <button type="button" role="radio" data-chrome-mode="native" aria-checked="true" title="Native frame: the OS draws the title bar">Native</button>
          <button type="button" role="radio" data-chrome-mode="header" aria-checked="false" title="Header: our band, revealed on hover">Header</button>
          <button type="button" role="radio" data-chrome-mode="content" aria-checked="false" title="Content only: nothing above the sentence">Content</button>
        </div>
      </section>
```

Its own group rather than a row inside "Window & Behavior", so it reuses
`.settings-group .segmented-control` and the furigana picker's markup verbatim —
no new stylesheet rules.

Delete the `decorations` element lookup and its change handler. Replace the
`currentChrome` block from Task 3 with:

```ts
  const chromeButtons = container.querySelectorAll<HTMLButtonElement>('[data-chrome-mode]');

  let currentChrome: ChromeMode = 'native';

  function updateChromeButtons(mode: ChromeMode) {
    currentChrome = mode;
    chromeButtons.forEach((btn) => {
      btn.setAttribute('aria-checked', String(btn.dataset.chromeMode === mode));
    });
  }

  chromeButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = btn.dataset.chromeMode as ChromeMode;
      if (!mode) return;
      updateChromeButtons(mode);
      void invoke('set_chrome_mode', { mode }).catch(() => {});
      saveAndBroadcast();
    });
  });

  /**
   * Windows has no native-frame state (spec §4), so the button naming it goes.
   *
   * Removed rather than disabled: the backend resolves `native` to `header`
   * there, so a disabled-but-present button would sit next to a control showing
   * a mode the window is not in. `querySelectorAll` above is a static NodeList,
   * so the removed node simply stops being found by the loops.
   */
  void invoke<string>('platform')
    .then((p) => {
      if (p === 'windows') container.querySelector('[data-chrome-mode="native"]')?.remove();
    })
    .catch(() => {});
```

In `applySettings`, the chrome arm becomes:

```ts
    if (s.chrome_mode !== undefined) updateChromeButtons(s.chrome_mode);
```

`currentSnapshot` already reads `chrome_mode: currentChrome` from Task 3 — leave it.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npm test
npm run build
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/settings.ts src/settings.test.ts
git commit -m "feat(settings): make window chrome a three-way control"
```

Body:

```
The checkbox could only ever name two of the three shapes. Its own
group, reusing the furigana picker's segmented control markup and CSS
verbatim, so this adds no stylesheet rules.

The native button is removed on Windows rather than disabled. The
backend resolves that mode to header there, so a present-but-dead
button would sit beside a control claiming a mode the window is not in.

Content mode is reachable from the UI from here.
```

---

## Task 6: `clampMenu`, the pure placement function

Spec §6.4. Separated from the DOM for the reason `placePopover` is: happy-dom returns zeros from `getBoundingClientRect()`, so geometry asserted through a DOM in Vitest would pass regardless of what the code computed.

**Files:**
- Create: `src/context-menu.ts`
- Create: `src/context-menu.test.ts`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `export const MENU_MARGIN = 4`
  - `export function clampMenu(x: number, y: number, size: { width: number; height: number }, viewport: { width: number; height: number }): { left: number; top: number }`

- [ ] **Step 1: Write the failing tests**

Create `src/context-menu.test.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { MENU_MARGIN, clampMenu } from './context-menu';

const SIZE = { width: 140, height: 120 };
const VIEWPORT = { width: 400, height: 300 };

describe('clampMenu', () => {
  test('opens at the cursor when the menu fits below and right of it', () => {
    expect(clampMenu(20, 30, SIZE, VIEWPORT)).toEqual({ left: 20, top: 30 });
  });

  // Flipped, not slid: a menu that slid left would sit under the cursor and
  // swallow the click that opened it.
  test('flips left of the cursor when it would overflow the right edge', () => {
    expect(clampMenu(380, 30, SIZE, VIEWPORT)).toEqual({ left: 380 - SIZE.width, top: 30 });
  });

  test('flips above the cursor when it would overflow the bottom edge', () => {
    expect(clampMenu(20, 290, SIZE, VIEWPORT)).toEqual({ left: 20, top: 290 - SIZE.height });
  });

  test('flips on both axes at once', () => {
    expect(clampMenu(380, 290, SIZE, VIEWPORT)).toEqual({
      left: 380 - SIZE.width,
      top: 290 - SIZE.height,
    });
  });

  // The flip can itself go off the near edge in a small window, so the margin
  // clamp is applied last and wins.
  test('pins to the margin when the flip overshoots the near edge', () => {
    expect(clampMenu(20, 20, SIZE, { width: 150, height: 130 })).toEqual({
      left: MENU_MARGIN,
      top: MENU_MARGIN,
    });
  });

  // The case a window at the configured 160x80 minimum hits.
  test('pins to the margin when the menu is larger than the viewport', () => {
    expect(clampMenu(10, 10, { width: 400, height: 500 }, { width: 160, height: 80 })).toEqual({
      left: MENU_MARGIN,
      top: MENU_MARGIN,
    });
  });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test src/context-menu.test.ts 2>&1 | tail -15
```

Expected: FAIL — cannot resolve `./context-menu`.

- [ ] **Step 3: Write the implementation**

Create `src/context-menu.ts` with the GPL header copied verbatim from a sibling `.ts` file if one carries it, then:

```ts
/** Closest the menu may sit to a viewport edge, in px. */
export const MENU_MARGIN = 4;

/**
 * Where to put the right-click menu, as a pure function of the cursor, the
 * menu's size and the viewport.
 *
 * Separated from the DOM for the reason `placePopover` is (see
 * `render/popover.ts`): happy-dom returns zeros from
 * `getBoundingClientRect()` and from `offsetWidth`, so geometry asserted
 * through a DOM in Vitest would pass regardless of what this computed.
 *
 * Flips rather than slides on an overflow: a menu that slid back along the
 * axis would sit under the cursor and swallow the click that opened it. The
 * margin clamp is applied last and wins, because in a small window the flip
 * can itself overshoot the near edge — and this window's configured minimum is
 * 160x80.
 */
export function clampMenu(
  x: number,
  y: number,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number } {
  let left = x;
  let top = y;
  if (left + size.width + MENU_MARGIN > viewport.width) left = x - size.width;
  if (top + size.height + MENU_MARGIN > viewport.height) top = y - size.height;
  left = Math.max(MENU_MARGIN, Math.min(left, viewport.width - size.width - MENU_MARGIN));
  top = Math.max(MENU_MARGIN, Math.min(top, viewport.height - size.height - MENU_MARGIN));
  return { left, top };
}
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
npm test src/context-menu.test.ts
npm run build
```

Expected: 6 tests PASS, build clean.

- [ ] **Step 5: Commit**

```bash
git add src/context-menu.ts src/context-menu.test.ts
git commit -m "feat(menu): add the clampMenu placement function"
```

Body:

```
Pure, for the reason placePopover is pure: happy-dom returns zeros from
offsetWidth, so placement asserted through a DOM would pass whatever the
code computed.

Flips rather than slides on overflow -- a slid menu would sit under the
cursor and swallow the click that opened it -- and the margin clamp is
applied last, because the flip can overshoot the near edge in a window
at the configured 160x80 minimum.
```

---

## Task 7: The right-click menu

Spec §6.4. Content mode removes the gear, the history chevrons and the window buttons, so this is the reach that replaces them. Available in every mode: one code path is less code than a conditional, and a two-click mode switch is useful everywhere.

**Files:**
- Modify: `src/context-menu.ts`, `src/context-menu.test.ts`
- Modify: `src/main.ts`, `src/main.test.ts`
- Modify: `src/styles/global.css`

**Interfaces:**
- Consumes: `clampMenu` (Task 6), `applyChromeMode` and `chromeMode` (Task 3), the existing `open_settings_window` and `close_window` commands.
- Produces:
  - `export type MenuEntry = { kind: 'radio'; label: string; checked: boolean; onSelect: () => void } | { kind: 'action'; label: string; onSelect: () => void } | { kind: 'separator' }`
  - `export function openContextMenu(entries: MenuEntry[], x: number, y: number): void`
  - `export function closeContextMenu(): void`

- [ ] **Step 1: Write the failing tests**

In `src/context-menu.test.ts`, extend the two existing import lines at the top of the file rather than adding new ones at the bottom:

```ts
import { afterEach, beforeEach, describe, expect, test } from 'vitest';
import { MENU_MARGIN, clampMenu, closeContextMenu, openContextMenu, type MenuEntry } from './context-menu';
```

then append this second `describe` block:

```ts
describe('openContextMenu', () => {
  const selected: string[] = [];

  function entries(): MenuEntry[] {
    return [
      { kind: 'radio', label: 'Native frame', checked: true, onSelect: () => selected.push('native') },
      { kind: 'radio', label: 'Content only', checked: false, onSelect: () => selected.push('content') },
      { kind: 'separator' },
      { kind: 'action', label: 'Quit', onSelect: () => selected.push('quit') },
    ];
  }

  beforeEach(() => {
    selected.length = 0;
    document.body.innerHTML = '';
  });

  afterEach(closeContextMenu);

  const menu = () => document.querySelector<HTMLElement>('.context-menu');

  test('renders one button per entry, with the separator between them', () => {
    openContextMenu(entries(), 10, 10);
    const labels = [...menu()!.querySelectorAll('button')].map((b) => b.textContent);
    expect(labels).toEqual(['Native frame', 'Content only', 'Quit']);
    expect(menu()!.querySelectorAll('[role="separator"]')).toHaveLength(1);
  });

  test('marks the checked radio and leaves the actions unmarked', () => {
    openContextMenu(entries(), 10, 10);
    const buttons = [...menu()!.querySelectorAll('button')];
    expect(buttons[0].getAttribute('aria-checked')).toBe('true');
    expect(buttons[1].getAttribute('aria-checked')).toBe('false');
    expect(buttons[2].getAttribute('role')).toBe('menuitem');
    expect(buttons[2].hasAttribute('aria-checked')).toBe(false);
  });

  test('a click runs the entry and closes the menu', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.querySelectorAll<HTMLButtonElement>('button')[1].click();
    expect(selected).toEqual(['content']);
    expect(menu()).toBeNull();
  });

  test('opening again replaces the open menu rather than stacking one', () => {
    openContextMenu(entries(), 10, 10);
    openContextMenu(entries(), 20, 20);
    expect(document.querySelectorAll('.context-menu')).toHaveLength(1);
  });

  test('Escape closes it', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(menu()).toBeNull();
  });

  test('the arrows wrap around the items', () => {
    openContextMenu(entries(), 10, 10);
    const buttons = [...menu()!.querySelectorAll<HTMLButtonElement>('button')];
    expect(document.activeElement).toBe(buttons[0]);

    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
    expect(document.activeElement).toBe(buttons[2]);
    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement).toBe(buttons[0]);
  });

  test('a pointerdown outside closes it, one inside does not', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(menu()).not.toBeNull();

    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(menu()).toBeNull();
  });

  test('losing window focus closes it', () => {
    openContextMenu(entries(), 10, 10);
    window.dispatchEvent(new Event('blur'));
    expect(menu()).toBeNull();
  });
});
```

Append to `src/main.test.ts`:

```ts
  test('a right-click opens the chrome menu and suppresses the default one', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'content' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    await vi.waitFor(() =>
      expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('content'),
    );

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);

    const labels = [...document.querySelectorAll('.context-menu button')].map(
      (b) => b.textContent,
    );
    expect(labels).toEqual([
      'Native frame',
      'Header + resize edges',
      'Content only',
      'Settings…',
      'Quit',
    ]);
    expect(
      document
        .querySelector('.context-menu button[aria-checked="true"]')!
        .textContent,
    ).toBe('Content only');
  });

  test('the menu omits the native mode on Windows', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' });
      if (cmd === 'platform') return Promise.resolve('windows');
      return Promise.resolve(null);
    });
    await import('./main');
    await vi.waitFor(() =>
      expect(document.querySelector('#app')!.getAttribute('data-platform')).toBe('windows'),
    );

    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const labels = [...document.querySelectorAll('.context-menu button')].map(
      (b) => b.textContent,
    );
    expect(labels).not.toContain('Native frame');
    expect(labels).toContain('Header + resize edges');
  });

  test('picking a mode from the menu applies, persists and broadcasts it', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'native' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('native'));

    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const item = [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')].find(
      (b) => b.textContent === 'Content only',
    )!;
    item.click();

    // Applied locally as well as broadcast: the emit round-trip is async, and
    // the mode should change on the frame the user clicked.
    expect(appEl.getAttribute('data-chrome')).toBe('content');
    expect(invoke).toHaveBeenCalledWith('set_chrome_mode', { mode: 'content' });
    expect(emitted.at(-1)).toEqual(['settings-changed', { chrome_mode: 'content' }]);
  });

  test('Settings… and Quit route to their existing commands', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'content' });
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(null);
    });
    await import('./main');
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_settings'));

    const open = (label: string) => {
      document.body.dispatchEvent(
        new MouseEvent('contextmenu', { bubbles: true, cancelable: true }),
      );
      [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
        .find((b) => b.textContent === label)!
        .click();
    };

    open('Settings…');
    expect(invoke).toHaveBeenCalledWith('open_settings_window');
    open('Quit');
    expect(invoke).toHaveBeenCalledWith('close_window');
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm test 2>&1 | tail -30
```

Expected: FAIL — `openContextMenu` is not exported, and `.context-menu` matches nothing.

One thing about the `main.test.ts` cases, so an implementer does not chase it: `vi.resetModules()` plus a re-import per test registers a fresh `document`-level `contextmenu` listener each time, and the old ones survive on the same `document`. They fire in registration order and every `openContextMenu` closes the menu already open, so the last listener — the current module's — is the one whose menu remains. The stale modules' `pointerdown` and `blur` listeners see their own `current === null` and no-op. The `beforeEach` reset of `document.body.innerHTML` clears any leftover node. Correct, but do not add a fourth accumulating listener without re-checking this.

- [ ] **Step 3: Write the menu DOM**

Append to `src/context-menu.ts`:

```ts
/**
 * One row of the menu. Built by the caller, so this module owns presentation
 * and keyboard behaviour and knows nothing about chrome modes or commands.
 */
export type MenuEntry =
  | { kind: 'radio'; label: string; checked: boolean; onSelect: () => void }
  | { kind: 'action'; label: string; onSelect: () => void }
  | { kind: 'separator' };

/** The open menu, or `null`. One window, so one menu. */
let current: HTMLElement | null = null;

export function closeContextMenu(): void {
  current?.remove();
  current = null;
}

function rowFor(entry: MenuEntry): HTMLElement {
  if (entry.kind === 'separator') {
    const separator = document.createElement('li');
    separator.className = 'context-menu-separator';
    separator.setAttribute('role', 'separator');
    return separator;
  }
  const row = document.createElement('li');
  const button = document.createElement('button');
  button.type = 'button';
  // The radio marker is a CSS `::before`, so the label stays the button's only
  // text and `textContent` reads exactly what the caller passed.
  button.textContent = entry.label;
  if (entry.kind === 'radio') {
    button.setAttribute('role', 'menuitemradio');
    button.setAttribute('aria-checked', String(entry.checked));
  } else {
    button.setAttribute('role', 'menuitem');
  }
  button.addEventListener('click', () => {
    // Closed first: an entry that opens another window would otherwise leave
    // the menu standing behind it.
    closeContextMenu();
    entry.onSelect();
  });
  row.append(button);
  return row;
}

/**
 * Open the menu at the cursor, replacing any menu already open.
 *
 * Inserted hidden and positioned before it is shown: `offsetWidth` needs the
 * node in the document, and `clampMenu` needs the measurement. In happy-dom
 * those measurements are zero, which places the menu at the cursor — harmless,
 * and the reason `clampMenu` is unit-tested separately.
 */
export function openContextMenu(entries: MenuEntry[], x: number, y: number): void {
  closeContextMenu();

  const menu = document.createElement('ul');
  menu.className = 'context-menu';
  menu.setAttribute('role', 'menu');
  for (const entry of entries) menu.append(rowFor(entry));

  menu.addEventListener('keydown', (event) => {
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')];
    if (buttons.length === 0) return;
    // Enter and Space need no arm: these are real <button>s.
    if (event.key === 'Escape') {
      event.preventDefault();
      closeContextMenu();
      return;
    }
    const step = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    // `at` is -1 when focus is elsewhere, which lands on the first item going
    // down and the last going up — both the right answer.
    buttons[(at + step + buttons.length) % buttons.length]?.focus();
  });

  menu.style.visibility = 'hidden';
  document.body.append(menu);
  const { left, top } = clampMenu(
    x,
    y,
    { width: menu.offsetWidth, height: menu.offsetHeight },
    { width: window.innerWidth, height: window.innerHeight },
  );
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  menu.style.visibility = '';
  current = menu;
  menu.querySelector<HTMLButtonElement>('button')?.focus();
}

// Registered once, at module scope, rather than per open: the `pointerdown`
// that opens the menu on a right-click fires BEFORE the `contextmenu` event, so
// at that moment `current` is still null and this is a no-op — a per-open
// listener would have to work around its own opening click.
document.addEventListener('pointerdown', (event) => {
  if (current !== null && !current.contains(event.target as Node)) closeContextMenu();
});
// The window losing focus means the click went to another window or to OS
// chrome, neither of which produces a `pointerdown` here.
window.addEventListener('blur', closeContextMenu);
```

- [ ] **Step 4: Wire it into `main.ts`**

Add the import:

```ts
import { openContextMenu, type MenuEntry } from './context-menu';
```

Hoist the platform to module scope. Replace the `const currentPlatform = ...` line inside `applySettings` with an assignment to a new module-scope binding declared next to `chromeMode`:

```ts
/**
 * The platform, as the backend reports it. Module-scope because the right-click
 * menu needs it too, and it decides whether the native-frame item exists.
 * Defaults to `macos` for the window between import and the first `platform`
 * answer — a right-click inside those few milliseconds is the only thing that
 * could see the wrong answer.
 */
let currentPlatform = 'macos';
```

so `applySettings` reads:

```ts
  currentPlatform = await invoke<string>('platform').catch(async () => {
    const isMac = await invoke<boolean>('is_macos').catch(() => false);
    return isMac ? 'macos' : 'windows';
  });
```

Then add, next to `applyChromeMode`:

```ts
/** The label shown for each mode, in the order the menu lists them. */
const CHROME_LABELS: ReadonlyArray<[ChromeMode, string]> = [
  ['native', 'Native frame'],
  ['header', 'Header + resize edges'],
  ['content', 'Content only'],
];

/**
 * Apply a mode the user picked from the menu.
 *
 * Applied locally as well as sent: the `emit` round-trip is asynchronous and
 * comes back to this same listener, so without the local call the window would
 * change a frame or two after the click. `applyChromeMode` is idempotent, so
 * handling the echo costs nothing.
 */
function selectChromeMode(mode: ChromeMode): void {
  applyChromeMode(mode);
  void invoke('set_chrome_mode', { mode }).catch(() => {});
  void emit('settings-changed', { chrome_mode: mode }).catch(() => {});
}

function chromeMenuEntries(): MenuEntry[] {
  const entries: MenuEntry[] = CHROME_LABELS
    // Windows has no native-frame state (spec §4), so offering it would name a
    // shape the platform cannot draw.
    .filter(([mode]) => mode !== 'native' || currentPlatform !== 'windows')
    .map(([mode, label]) => ({
      kind: 'radio' as const,
      label,
      checked: chromeMode === mode,
      onSelect: () => selectChromeMode(mode),
    }));
  entries.push({ kind: 'separator' });
  entries.push({
    kind: 'action',
    label: 'Settings…',
    onSelect: () => {
      void invoke('open_settings_window').catch(() => {});
    },
  });
  entries.push({
    kind: 'action',
    label: 'Quit',
    onSelect: () => {
      void invoke('close_window').catch(() => {});
    },
  });
  return entries;
}

/**
 * Bound to `document`, not `#app`.
 *
 * A keyboard-invoked context menu (Shift+F10, or the Menu key) targets the
 * focused element, which in content mode is `<body>` — an ancestor of `#app`,
 * so the event would never reach a listener bound there. Content mode has no
 * tab stops at all, so this is the only binding that keeps the menu reachable
 * without a pointer. `preventDefault` also suppresses the WebView2 default
 * menu.
 */
document.addEventListener('contextmenu', (event) => {
  event.preventDefault();
  openContextMenu(chromeMenuEntries(), event.clientX, event.clientY);
});
```

- [ ] **Step 5: Style it**

Append to `src/styles/global.css`:

```css
/* --- Right-click menu (spec §6.4) ---
   Content-only mode removes the gear, the history nav and the window buttons,
   so this is the only reach into the app there. `left` and `top` are set by
   `clampMenu` at open time, which is why nothing here positions it. */
.context-menu {
  position: fixed;
  z-index: 100;
  margin: 0;
  padding: 4px;
  min-width: 150px;
  list-style: none;
  background: var(--color-bg);
  border: 1px solid var(--color-rule);
  border-radius: 6px;
  box-shadow: 0 4px 16px rgb(0 0 0 / 0.28);
}

.context-menu button {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 4px 8px;
  font-family: var(--font-ui);
  font-size: var(--text-tag);
  text-align: left;
  color: var(--color-text);
  background: none;
  border: 0;
  border-radius: 4px;
  cursor: pointer;
}

.context-menu button:hover,
.context-menu button:focus-visible {
  background: var(--color-hover);
  outline: none;
}

/* The radio marker is generated, so the label stays the button's only text and
   `textContent` in the tests reads exactly the label. */
.context-menu button[role='menuitemradio']::before {
  content: '○';
  width: 1em;
}

.context-menu button[aria-checked='true']::before {
  content: '●';
}

/* An indent matching the marker's width, so the action labels line up with the
   radio labels rather than with their markers. */
.context-menu button[role='menuitem'] {
  padding-left: calc(8px + 1em + 6px);
}

.context-menu-separator {
  height: 1px;
  margin: 4px 2px;
  background: var(--color-rule);
}
```

- [ ] **Step 6: Run everything, and hold the invariance gate**

```bash
npm test
npm run build
npx playwright test e2e/panes.spec.ts
git status --porcelain e2e/panes.spec.ts-snapshots
```

Expected: green, build clean, `panes.spec.ts` green with **no snapshot modified**. The menu is appended to `document.body` and `position: fixed`, so it adds nothing to the existing baselines while closed.

- [ ] **Step 7: Commit**

```bash
git add src/context-menu.ts src/context-menu.test.ts src/main.ts src/main.test.ts src/styles/global.css
git commit -m "feat(menu): add the chrome right-click menu"
```

Body:

```
Content mode removes the gear, the history chevrons and the window
buttons, so this is the reach that replaces them: the three modes as a
radio group, Settings and Quit. Offered in every mode -- one code path
is less code than a conditional, and a two-click mode switch is useful
everywhere -- and the native item is absent on Windows.

Bound to `document`, not `#app`. A keyboard-invoked menu (Shift+F10, the
Menu key) targets the focused element, which in content mode is <body>,
an ancestor of #app -- a listener there would never see it, and content
mode has no tab stops to focus instead.

The outside-click listener is registered once at module scope rather
than per open, because the pointerdown that opens the menu fires before
the contextmenu event and would otherwise have to be worked around.
```

---

## Task 8: Playwright coverage for the three modes

Spec §8. Real layout, which is the only place the band's geometry and the `visibility` of the content-mode controls can be checked — happy-dom resolves neither.

Screenshot budget, stated rather than silently capped: the spec asks for 320 / 768 / 1024 in both themes. Pixels are captured at **768 in both themes for each offered mode** (6 baselines); 320 and 1024 are covered by assertions on the band's measured height and the controls' resolved `visibility`, which is the information a third and fourth width actually carries here.

**Files:**
- Create: `e2e/chrome.spec.ts`

**Interfaces:**
- Consumes: `STUB` from `e2e/stub.ts`, and the `fixture` pattern from `e2e/panes.spec.ts`.
- Produces: nothing other tasks use.

- [ ] **Step 1: Read the two patterns this file follows**

```bash
sed -n '1,60p' e2e/panes.spec.ts
sed -n '160,200p' e2e/panes.spec.ts
```

Copy the fixture construction, the `page.addInitScript` shape, the theme-switching helper and the `toHaveScreenshot` call style from there. Do not invent a second harness.

- [ ] **Step 2: Write the spec**

Create `e2e/chrome.spec.ts`. Build a per-mode stub the way `panes.spec.ts` builds `HIDDEN_TITLEBAR_STUB`, overriding `get_settings` and `platform`:

```ts
function stubFor(mode: 'native' | 'header' | 'content', platform: 'macos' | 'windows'): string {
  return `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'platform') return Promise.resolve('${platform}');
    if (cmd === 'is_macos') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'peek_grows_frame') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: '${mode}' });
    return realInvoke(cmd, args);
  };
`;
}
```

Then the tests, each one asserting rather than only photographing:

1. **`content mode paints nothing above the sentence`** — macOS, 768 wide. `#app` has `data-chrome="content"`; `header.controls` measures 6px tall; `#settings-toggle`, `.history-controls` and `.window-controls` each resolve to `visibility: hidden`; a `page.mouse.move` into the window leaves `#app` without the `peeked` class and the band still 6px.
2. **`content mode leaves no tab stop in the band`** — macOS. Press `Tab` from the body and assert `document.activeElement` is not inside `header.controls`. This is the WCAG check §3.2 exists for, and it cannot be written in happy-dom.
3. **`header mode still reveals on hover`** — macOS. The band is 6px, a `page.mouse.move` inside takes it to 28px, and `#output`'s bounding box `y` is unchanged between the two measurements. Deliberately a near-duplicate of `panes.spec.ts`'s own test: this file must fail if content mode's rules leak into header mode.
4. **`native mode reserves the row`** — macOS. The band measures 28px with no hover, and the gear resolves to `opacity: 1`.
5. **`Windows offers no native mode`** — the Windows stub with `chrome_mode: 'native'`; assert `#app` still ends up at `data-chrome="native"` from the stub (the stub is not the real backend) **and** that the right-click menu omits `Native frame`. Comment that the resolution itself is a Rust-side concern, covered by `cargo test`.
6. **`the menu opens inside the viewport from the bottom-right corner`** — 320 wide, content mode. Right-click at `(width - 2, height - 2)`; assert the menu's bounding box lies entirely within the viewport on all four edges.
7. **`the menu closes on Escape and on an outside click`** — content mode.
8. **Screenshots** — for each of the three modes, at 768, in light and dark, `await expect(page).toHaveScreenshot()` following `panes.spec.ts`'s naming.
9. **Band geometry at 320 and 1024** — for each mode, assert the band's measured height and the controls' resolved `visibility` at both widths. No screenshots.

- [ ] **Step 3: Run it and generate the new baselines**

```bash
npx playwright test e2e/chrome.spec.ts --update-snapshots
npx playwright test e2e/chrome.spec.ts
```

Expected: the first run writes six baselines, the second passes against them.

- [ ] **Step 4: Confirm the existing suite is still untouched**

```bash
npx playwright test
git status --porcelain e2e/panes.spec.ts-snapshots
```

Expected: whole suite green, and **`git status` on the `panes.spec.ts` snapshots prints nothing** — only `chrome.spec.ts-snapshots` is new.

- [ ] **Step 5: Commit**

```bash
git add e2e/chrome.spec.ts e2e/chrome.spec.ts-snapshots
git commit -m "test(chrome): cover the three modes in Playwright"
```

Body:

```
Real layout, which is the only place the band's measured height and the
resolved visibility of the content-mode controls can be checked --
happy-dom resolves neither, so the WCAG tab-stop check in particular
cannot live in Vitest.

The header-mode reveal test here is a near-duplicate of panes.spec.ts's
own on purpose: this file has to fail if content mode's rules leak into
the mode that is out of scope for this phase.

Pixels at 768 in both themes for each mode; 320 and 1024 are asserted on
band height and resolved visibility instead, which is the information a
third and fourth screenshot of a 28px band would carry.
```

---

## Task 9: Close the phase in the docs

Follows 4C's pattern: the master spec is updated and a handoff written once the code is in.

**Files:**
- Modify: `docs/superpowers/specs/2026-08-12-jparser-port-design.md`
- Create: `docs/superpowers/2026-08-25-jparser-phase4a-handoff.md`

**Interfaces:**
- Consumes: everything above.
- Produces: nothing.

- [ ] **Step 1: Update the Phase 4 table in the master spec**

In §11, change the heading `### Phase 4 — Window behaviour (4C shipped; 4A/4B/4D remaining)` to `### Phase 4 — Window behaviour (4A and 4C shipped; 4B/4D remaining)`, and rewrite the **4A** row's Scope cell:

```
Shipped. Three-way `chrome_mode` replacing the `decorations` boolean, plus the content-only state (§7.4): the band never reveals, its controls leave the tab order, and drag and resize survive. The toggles did not move to a right-click menu — they had already moved to the settings window in 3B — so the menu carries the mode switch, Settings and Quit instead. **Native frame is macOS/Linux only**: Windows is permanently frameless (`d07711f`, `e45d55f`) and resolves `native` to `header`, which also removed a broken decorated state there.
```

Update the paragraph above the table, which still says three sub-phases remain, to name two (4B and 4D) and to record that 4B's clamp now has a defined state to clamp on macOS only.

- [ ] **Step 2: Add deviations 10–12**

Append three rows to §12's table, copying the wording from the 4A design spec's §10:

| 10 | Native frame is macOS/Linux only | The Windows window is permanently frameless (`d07711f`, `e45d55f`); restoring `WS_CAPTION` re-opens the DWM shadow that commit removed |
| 11 | Default chrome mode is `native`, not §7.4's `header` | The shipped `Settings::default()` has always been the other one, and a first-run window with no title bar cannot explain itself |
| 12 | Content-only keeps drag and resize | ta-old's `WS_POPUP` had neither; a mode that cannot be left without editing `settings.json` is a trap, and keeping them costs nothing |

- [ ] **Step 3: Add the deferred item**

In §13, add:

```
- A real OS caption on Windows (the `native` chrome mode there).
```

- [ ] **Step 4: Write the handoff**

Create `docs/superpowers/2026-08-25-jparser-phase4a-handoff.md` following the structure of `docs/superpowers/2026-08-25-jparser-phase4c-handoff.md`: what was completed, the design decisions worth carrying forward, the verification gate with real numbers from the runs, and what 4B inherits. Cover at minimum:

- the `chrome_mode` migration and that it is self-completing;
- why content mode needed no Win32 code;
- the Windows native-frame finding, in full, since it is the thing a later sub-phase has to pick up;
- what Task 1 deleted and where its measurements went;
- the invariance gate and the fact that no `panes.spec.ts` baseline changed;
- for 4B: `native` is the state that must clamp `--surface-alpha` to 100 %, and it exists on macOS and Linux only, so the Windows arm of that clamp has nothing to do.

- [ ] **Step 5: Verify the whole gate one last time and record the numbers**

```bash
npm test
npm run build
cd src-tauri && cargo test && cargo clippy --all-targets && cd ..
npx playwright test
```

Put the actual counts in the handoff — test totals per suite, and the confirmation that clippy reported only the four pre-existing Win32 acronym warnings.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/specs/2026-08-12-jparser-port-design.md docs/superpowers/2026-08-25-jparser-phase4a-handoff.md
git commit -m "docs(phase4a): close the phase and record the deviations"
```

---

## Manual verification (both platforms, after Task 8)

§10 declines to automate chrome switching, so these are by hand under `npm run tauri dev`. Record the results in the Task 9 handoff.

- [ ] Every offered transition, each way, on each platform: macOS `native ↔ header ↔ content ↔ native`; Windows `header ↔ content`. After each, confirm the window's box has not drifted — same position, same size.
- [ ] In content mode: drag the window by the top strip; resize from each edge and corner; open the menu and switch back out.
- [ ] In content mode with the window focused and nothing clicked, press Shift+F10 (Windows) or the Menu key and confirm the menu opens.
- [ ] Quit from the menu closes the app.
- [ ] Switch modes from the settings window and confirm the main window follows without a restart; switch from the menu and confirm the settings window's control follows.
- [ ] Delete `settings.json`, launch, and confirm the first-run mode is `native` on macOS and `header` on Windows.
- [ ] Hand-write `{"decorations": false}` as the whole file, launch, confirm `header`, change any setting, and confirm `decorations` is gone from the file and `chrome_mode` is present.
- [ ] Header mode's hover reveal, on both platforms, exactly as `docs/superpowers/2026-08-18-windows-titlebar-test-plan.md` describes it. Nothing about it may have changed.

---

## Self-Review

**Spec coverage.** §2 scope → Tasks 1–8. §3 the three states → Tasks 3, 4. §3.1 content-is-header → Task 4. §3.2 `visibility` → Task 4 Step 5, verified in Task 8 test 2. §3.3 drag and resize → manual checks, since no automated harness can drag an OS window. §4.1 the finding → Task 4's deletions and Task 9's handoff. §4.2 resolution → Task 2. §4.3 `supported` → Task 2. §4.4 the four deletions → Task 4 Steps 3–4. §4.5 invariance → the Global Constraints and the gate in Tasks 3, 4, 7, 8. §5 persistence → Task 3 Steps 1–3. §5.1 default → Task 2's `#[default] Native`, checked in the manual list. §6.1 backend → Task 3 Step 5. §6.2 frontend → Task 3 Steps 10, 12. §6.3 settings window → Task 5. §6.4 menu → Tasks 6, 7. §7 the spike → Task 1. §8 verification → distributed, plus Task 8. §9 deferred → Task 9 Step 3. §10 deviations → Task 9 Step 2.

**Placeholders.** None. Every code step carries the code. Task 8 Step 2 describes nine tests rather than transcribing them, because each must copy `panes.spec.ts`'s live harness — Step 1 makes reading that harness a required step, and each test's assertions are named exactly.

**Type consistency.** `ChromeMode` is the Rust enum and the TS union with the same three lowercase wire names. `supported(mode, native_supported)` and `native_supported()` are named identically in Task 2's implementation, Task 3's tests and Task 3's command. `normalize` is private to `settings.rs` and used only by `load`. `initial_height` takes two arguments in Task 3 and one from Task 4 onward — the change is explicit in both tasks, and `main.rs` is edited in each. `applyChromeMode` and `chromeMode` are the only names used across Tasks 3, 4 and 7. `clampMenu`, `MENU_MARGIN`, `MenuEntry`, `openContextMenu` and `closeContextMenu` are spelled the same in Tasks 6 and 7. `data-chrome-mode` (settings buttons) is deliberately distinct from `data-chrome` (the `#app` state) — different things, and the tests select on both.
