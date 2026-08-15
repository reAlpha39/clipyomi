# JParser Phase 2K — Title Bar Toggle & Window Geometry Persistence (Design)

**Date:** 2026-08-15  
**Status:** Approved design, ready for implementation planning  
**Reference implementation:** `ta-old/` (Translation Aggregator, GPL v2)  
**Predecessor code:** the shipped Phase 2J tree at `dc112ce`  

## 1. Goal & Rationale

With the definitions pane removed in Phase 2J and the window minimum dimensions lowered to 160×80, the main window can be used as a slim, compact reading strip.

However:
1. Standard OS title bars (decorations) take 28–32 px of vertical space, preventing true minimal sizing and subtitle-style placement.
2. Window size and position reset on every launch back to default 720×480, forcing the user to manually reposition and resize the window every time the app opens.

Phase 2K addresses both concerns by:
- Adding a runtime toggle for window decorations (title bar) with native frameless dragging via `data-tauri-drag-region`.
- Persisting window dimensions (`width`, `height`) and coordinates (`x`, `y`) directly in `settings.json`, restored automatically at startup.

### In scope

- Add `decorations: bool`, `window_width: Option<u32>`, `window_height: Option<u32>`, `window_x: Option<i32>`, `window_y: Option<i32>` to `Settings` in `src-tauri/src/settings.rs`
- Add `set_decorations` and `save_window_geometry` commands in `src-tauri/src/commands.rs`
- Restore saved decorations, size, and position during app startup in `src-tauri/src/main.rs`
- Add `#decorations` toggle button to `.controls` header in `src/main.ts`
- Add `data-tauri-drag-region` to `.controls` header for native dragging
- Add debounced window geometry persistence on `tauri://resize` and `tauri://move` in `src/main.ts`
- Update unit tests (`src/main.test.ts`, Rust test suite) and end-to-end tests (`e2e/panes.spec.ts`)

### Out of scope / Non-goals

- Third-party window plugins (e.g. `tauri-plugin-window-state`): `settings.rs` already manages persistent user state; adding a crate is unnecessary overhead.
- Custom window close/minimize title-bar controls: OS keyboard shortcuts (Cmd+W / Alt+F4) or toggling the title bar back cover window management cleanly.
- Furigana rendering modes and font scale controls (reserved for Phase 3).

---

## 2. Backend & Rust Plumbing

### 2.1 Settings Schema (`src-tauri/src/settings.rs`)
`Settings` struct is extended:
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
```
Default remains `decorations: true`, geometry keys default to `None`.

### 2.2 Tauri Commands (`src-tauri/src/commands.rs`)
Two commands are added:
1. `set_decorations(enabled: bool, window: tauri::Window, settings: State<'_, Arc<SettingsState>>) -> Result<(), String>`:
   Calls `window.set_decorations(enabled)` and persists `settings.decorations = enabled`.
2. `save_window_geometry(width: u32, height: u32, x: i32, y: i32, settings: State<'_, Arc<SettingsState>>) -> Result<(), String>`:
   Updates `settings.window_width = Some(width)`, `settings.window_height = Some(height)`, `settings.window_x = Some(x)`, `settings.window_y = Some(y)` and writes to disk.

### 2.3 Startup Window Restoration (`src-tauri/src/main.rs`)
In `main.rs` setup:
- If `!settings.decorations`: `main_window.set_decorations(false)`.
- If `window_width` and `window_height` are present in settings: `main_window.set_size(tauri::LogicalSize::new(w, h))`.
- If `window_x` and `window_y` are present in settings: `main_window.set_position(tauri::LogicalPosition::new(x, y))`.

---

## 3. Frontend & UI

### 3.1 Types (`src/types.ts`)
Update `Settings` interface:
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

### 3.2 HTML Shell & Controls (`src/main.ts`)
- Header `.controls` receives `data-tauri-drag-region`:
  `<header class="controls" data-tauri-drag-region>`
- Add `#decorations` button:
  `<button id="decorations" type="button" aria-pressed="true">Title bar</button>`
- Wire `#decorations` button with `bindToggle(decorationsButton, 'set_decorations', (s) => s.decorations, (d) => ({ enabled: d }))`.

### 3.3 Geometry Persistence Debounce (`src/main.ts`)
When `tauri://move` or `tauri://resize` fires on the main window:
- In addition to calling `invalidateGeometry()` (which closes any open tooltip popover), schedule a 300ms debounced task:
  Reads `getCurrentWindow().innerSize()` and `getCurrentWindow().outerPosition()`, then invokes `save_window_geometry`.

---

## 4. Testing & Verification

1. **Rust Tests (`src-tauri`)**:
   - `settings::tests`: verify `decorations` defaults to true, geometry defaults to `None`, and all fields serialize/deserialize correctly.
   - `commands::tests`: verify `set_decorations` and `save_window_geometry` correctly update `SettingsState`.
2. **TypeScript / Vitest (`src/main.test.ts`)**:
   - `#decorations` toggle button renders with `aria-pressed` initialized from `get_settings`.
   - Clicking `#decorations` button invokes `set_decorations` with toggled boolean.
3. **End-to-End (`e2e/panes.spec.ts`)**:
   - Stub handles `set_decorations` and `save_window_geometry`.
   - Update screenshot baselines to include the third header button.
4. **Platform Verification**:
   - Verify frameless dragging via `data-tauri-drag-region` on macOS and Windows.
   - Verify window border resizing remains functional when undecorated.
