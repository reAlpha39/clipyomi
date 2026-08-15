# JParser Phase 2K — Title Bar Toggle & Window Geometry Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a runtime toggle to show/hide window decorations (title bar) with native frameless dragging, and persist window size & position across launches in `settings.json`.

**Architecture:** Extend existing `Settings` in Rust and TypeScript with `decorations`, `window_width`, `window_height`, `window_x`, and `window_y`. Rust exposes `set_decorations` and `save_window_geometry` commands, and restores them at startup in `main.rs`. The frontend adds a `#decorations` toggle button to `.controls`, adds `data-tauri-drag-region` for native window dragging, and debounces window geometry saving on resize and move events.

**Tech Stack:** Tauri 2 (`@tauri-apps/api` 2.11.1), Rust (`serde`, `serde_json`, `tauri`), TypeScript (strict), Vitest + happy-dom, Playwright, plain CSS custom properties.

**Spec:** `docs/superpowers/specs/2026-08-15-jparser-phase2k-design.md` (authoritative, committed at `ff94d30`). `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **No new external crates or plugins.** Use existing `settings.json` and Tauri core APIs (`window.set_decorations`, `window.set_size`, `window.set_position`).
- **No frontend framework.** Vanilla TypeScript and DOM APIs only.
- **`npx tsc --noEmit` clean, `npm test` clean, `cargo test` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean at the end of every task.**
- **File size** 200–400 lines typical, **800 hard maximum** including tests.

---

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/settings.rs` | *(modified)* `Settings` schema with `decorations` & geometry fields |
| `src-tauri/src/commands.rs` | *(modified)* `set_decorations` and `save_window_geometry` commands |
| `src-tauri/src/main.rs` | *(modified)* register commands & restore saved decorations/geometry on startup |
| `src/types.ts` | *(modified)* `Settings` interface updated with new fields |
| `src/main.ts` | *(modified)* `#decorations` button, `data-tauri-drag-region`, debounced geometry save |
| `src/main.test.ts` | *(modified)* unit tests for `#decorations` toggle button |
| `e2e/stub.ts` | *(modified)* mock new commands |
| `e2e/panes.spec.ts` | *(modified)* e2e coverage for `#decorations` toggle |
| `e2e/panes.spec.ts-snapshots/*.png` | *(regenerated)* baseline screenshots including the third control button |

---

## Task 1: Rust Backend (Settings, Commands, and Startup Restoration)

**Files:**
- Modify: `src-tauri/src/settings.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Consumes: Tauri Window & AppState
- Produces: Commands `set_decorations(enabled: bool, ...)` and `save_window_geometry(width: u32, height: u32, x: i32, y: i32, ...)`.

- [ ] **Step 1: Write failing Rust unit tests in `src-tauri/src/settings.rs` and `src-tauri/src/commands.rs`**

In `src-tauri/src/settings.rs` tests:
```rust
#[test]
fn default_decorations_is_true() {
    let s = Settings::default();
    assert!(s.decorations);
    assert_eq!(s.window_width, None);
    assert_eq!(s.window_height, None);
    assert_eq!(s.window_x, None);
    assert_eq!(s.window_y, None);
}

#[test]
fn geometry_round_trips_through_json() {
    let json = r#"{
        "always_on_top": true,
        "clipboard_monitoring": false,
        "decorations": false,
        "window_width": 500,
        "window_height": 120,
        "window_x": 100,
        "window_y": 200
    }"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    assert!(!s.decorations);
    assert_eq!(s.window_width, Some(500));
    assert_eq!(s.window_height, Some(120));
    assert_eq!(s.window_x, Some(100));
    assert_eq!(s.window_y, Some(200));
}
```

In `src-tauri/src/commands.rs` tests:
```rust
#[test]
fn save_window_geometry_updates_settings_state() {
    let temp = tempfile::tempdir().unwrap();
    let state = Arc::new(SettingsState::load_from(&temp.path().join("settings.json")));
    save_window_geometry(600, 200, 50, 80, State::from(&state)).unwrap();
    let s = state.snapshot();
    assert_eq!(s.window_width, Some(600));
    assert_eq!(s.window_height, Some(200));
    assert_eq!(s.window_x, Some(50));
    assert_eq!(s.window_y, Some(80));
}
```

- [ ] **Step 2: Update `Settings` in `src-tauri/src/settings.rs`**

Extend `Settings`:
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
    #[serde(default = "default_true")]
    pub decorations: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            clipboard_monitoring: true,
            decorations: true,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            extra: serde_json::Map::new(),
        }
    }
}
```

- [ ] **Step 3: Implement commands in `src-tauri/src/commands.rs`**

```rust
#[tauri::command]
pub fn set_decorations(
    enabled: bool,
    window: tauri::Window,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    window
        .set_decorations(enabled)
        .map_err(|e| e.to_string())?;
    settings
        .update(|s| s.decorations = enabled)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_window_geometry(
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    settings: State<'_, Arc<SettingsState>>,
) -> Result<(), String> {
    settings
        .update(|s| {
            s.window_width = Some(width);
            s.window_height = Some(height);
            s.window_x = Some(x);
            s.window_y = Some(y);
        })
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Register commands and restore settings at startup in `src-tauri/src/main.rs`**

1. Add `commands::set_decorations, commands::save_window_geometry` to `tauri::generate_handler!`.
2. In `setup()`:
```rust
if let Some(main_window) = app.get_webview_window("main") {
    if !settings.decorations {
        let _ = main_window.set_decorations(false);
    }
    if let (Some(w), Some(h)) = (settings.window_width, settings.window_height) {
        let _ = main_window.set_size(tauri::LogicalSize::new(w, h));
    }
    if let (Some(x), Some(y)) = (settings.window_x, settings.window_y) {
        let _ = main_window.set_position(tauri::LogicalPosition::new(x, y));
    }
}
```

- [ ] **Step 5: Verify Rust tests and Clippy**

Run:
```bash
cargo test && cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/
git commit -m "feat: add decorations toggle and window geometry persistence in Rust"
```

---

## Task 2: Frontend Controls, Drag Region & Geometry Debounce

**Files:**
- Modify: `src/types.ts`, `src/main.ts`, `src/main.test.ts`

**Interfaces:**
- Consumes: `set_decorations`, `save_window_geometry`, `get_settings`
- Produces: Interactive `#decorations` button and native window dragging.

- [ ] **Step 1: Write failing unit tests in `src/main.test.ts`**

In `src/main.test.ts`, add a describe block:
```ts
describe('the decorations toggle', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          decorations: true,
        });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  test('renders in the header with initial state from get_settings', () => {
    const button = document.querySelector<HTMLButtonElement>('#decorations');
    expect(button).not.toBeNull();
    expect(button?.getAttribute('aria-pressed')).toBe('true');
  });

  test('clicking toggles aria-pressed and invokes set_decorations with negated state', async () => {
    const button = document.querySelector<HTMLButtonElement>('#decorations')!;
    button.click();
    await Promise.resolve();
    expect(invoke).toHaveBeenCalledWith('set_decorations', { enabled: false });
    expect(button.getAttribute('aria-pressed')).toBe('false');
  });
});
```

- [ ] **Step 2: Update `src/types.ts`**

```ts
export interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
  decorations: boolean;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}
```

- [ ] **Step 3: Update `src/main.ts`**

1. In `app.innerHTML`:
   Add `data-tauri-drag-region` to `<header class="controls" data-tauri-drag-region>`
   Add `<button id="decorations" type="button" aria-pressed="true">Title bar</button>` inside `.controls`.
2. Grab button handle:
   `const decorationsButton = document.querySelector<HTMLButtonElement>('#decorations')!;`
3. Bind toggle:
   `bindToggle(decorationsButton, 'set_decorations', (s) => s.decorations, (d) => ({ enabled: d }));`
4. In `tauri://resize` and `tauri://move` listeners:
   Add debounced save of window size and position:
   ```ts
   let geometrySaveTimer: number | null = null;
   function scheduleGeometrySave(): void {
     if (geometrySaveTimer !== null) window.clearTimeout(geometrySaveTimer);
     geometrySaveTimer = window.setTimeout(async () => {
       try {
         const win = getCurrentWindow();
         const size = await win.innerSize();
         const pos = await win.outerPosition();
         const factor = await win.scaleFactor();
         const logicalSize = size.toLogical(factor);
         const logicalPos = pos.toLogical(factor);
         void invoke('save_window_geometry', {
           width: Math.round(logicalSize.width),
           height: Math.round(logicalSize.height),
           x: Math.round(logicalPos.x),
           y: Math.round(logicalPos.y),
         });
       } catch {}
     }, 300);
   }
   ```
   Call `scheduleGeometrySave()` inside the `move` and `resize` listeners.

- [ ] **Step 4: Run tests and typecheck**

Run:
```bash
npm test && npx tsc --noEmit
```

- [ ] **Step 5: Commit**

```bash
git add src/
git commit -m "feat: add title bar toggle and window geometry debounced save in frontend"
```

---

## Task 3: End-to-End Tests, Stubbing, and Snapshot Baselines

**Files:**
- Modify: `e2e/stub.ts`, `e2e/panes.spec.ts`
- Regenerate: `e2e/panes.spec.ts-snapshots/*.png`

**Interfaces:**
- Consumes: Updated controls layout with 3 buttons
- Produces: Clean Playwright run and updated screenshot baselines.

- [ ] **Step 1: Update `e2e/stub.ts`**

Ensure `STUB` mock in `e2e/stub.ts` handles `set_decorations` and `save_window_geometry` without throwing errors, and returns `decorations: true` in `get_settings`.

- [ ] **Step 2: Update `e2e/panes.spec.ts`**

Add e2e test for `#decorations` button keeping keyboard focus on activation.

- [ ] **Step 3: Regenerate Playwright snapshots**

Run:
```bash
npx playwright test -u
```
Verify all 18+ tests pass and snapshots match the 3-button header.

- [ ] **Step 4: Run full verification gate**

Run:
```bash
npm test && npx tsc --noEmit && cargo test && cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add e2e/
git commit -m "test: update e2e specs, stubs, and snapshots for title bar toggle"
```
