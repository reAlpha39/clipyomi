# Windows test plan — titlebar band and hover reveal

**Audience:** an agent with this repo checked out on Windows.
**Commit under test:** `d8ab92f` on `develop` (`feat(main): make the header a titlebar band that reveals on hover`).
**Why you:** the feature was built and verified on macOS only. This repo has never been run on Windows and has no CI, so every Windows claim below is "reads correct in the source", not "tested".

Your job is to find out what actually happens, not to fix it. Three of the checks below are *expected* to fail — they are known consequences of a macOS-first design, and one of them has a planned fix already sketched. Report; do not repair unless a follow-up asks you to.

## What the feature is

The main window's header is now the title bar: a 28px band whose only child is the settings gear, right-aligned. It has two states, keyed off `data-decorations` on `#app`, which `src/main.ts` mirrors from the persisted `decorations` setting:

- `data-decorations="true"` — band is a reserved 28px grid row; gear and the divider under it are always visible.
- `data-decorations="false"` — band leaves the grid and becomes an absolutely positioned 6px sliver at the top with the gear at `opacity: 0`. The cursor entering the window anywhere adds a `peeked` class to `#app`, which expands the band to 28px, fills it with `--color-bg`, and fades the gear in. Leaving the window removes it after `PEEK_HIDE_MS` (1000ms); re-entering within that window cancels the hide outright.

On macOS the reveal also calls the `peek_titlebar` command, which shows the OS title text and traffic lights *and grows the window frame upward by 28px* so the content does not move on screen while `#app` carries an equal `padding-top`.

Constants and selectors, if you want to assert on them:

| Thing | Where | Value |
| --- | --- | --- |
| `BAND_HEIGHT` | `src/main.ts` | `28` (sent to the backend with each peek) |
| `--band-h` | `src/styles/global.css` | `28px` (its CSS twin) |
| `PEEK_HIDE_MS` | `src/main.ts` | `1000` |
| band element | DOM | `header.controls` |
| state | DOM | `#app[data-decorations]`, `#app.peeked` |
| gear | DOM | `#settings-toggle` |

## Setup

```powershell
npm install
npm test                    # vitest, 119 tests, expected green on any platform
npm run test:e2e            # Playwright, 20 tests — see the screenshot note below
cd src-tauri; cargo test    # 58 tests, expected green
cd ..
npm run tauri dev
```

Before you start:

- **Playwright screenshots**: the committed baselines are `*-darwin.png`, generated on macOS. Playwright looks for a platform-suffixed name, so on Windows the snapshot assertions will report *missing* baselines rather than mismatches. Do not commit Windows baselines as part of this task — just report what it says.
- The specs run against a stub (`e2e/stub.ts`), not the Rust backend, so they cover the CSS/JS half only. Everything about real window chrome must be checked by hand under `npm run tauri dev`.
- Persisted settings live at `%APPDATA%\dev.jparser.clipyomi\settings.json` (Tauri's `app_config_dir` for identifier `dev.jparser.clipyomi`). Confirm that path before trusting it; if the app shows a settings warning at startup, that is where it looked. The key that matters is `"decorations": true | false`.
- A first-run dictionary download screen may appear inside the window. Irrelevant here — this is all window chrome. Do not download anything.

Toggle the title bar from the settings window: click the gear, then "Title bar & window borders" under *Window & Behavior*.

## Checks

### 1. Title bar shown — baseline layout

Ensure the checkbox is on. Observe where the gear sits relative to the native Windows caption.

**Expected to FAIL as designed.** `titleBarStyle: "Overlay"` in `src-tauri/tauri.conf.json` is macOS-only and ignored on Windows, so the webview does not own the caption strip. The gear will sit in the band *below* the Windows caption rather than inside it. Report what you see, and the caption plus band height in pixels if you can measure it.

### 2. Hiding the title bar

Turn the checkbox off.

**Expected to PASS.** `commands.rs` takes the non-macOS branch and calls `window.set_decorations(false)`, so the real caption should disappear and the window should become frameless. Confirm: no caption, no system buttons, and the app's own 6px strip at the top.

### 3. Hiding survives a restart

With the title bar off, quit and relaunch.

**Expected to PASS.** `src-tauri/src/main.rs` applies `set_decorations(false)` during `setup` when the loaded setting is off. Confirm the window returns frameless rather than flashing a caption first. If it flashes, say roughly for how long.

### 4. Hover reveal — the gear

Title bar off. Move the cursor outside the window, then into it — anywhere, not just the top edge.

**Expected to PASS.** The gear should fade in over roughly 120ms and the band should thicken to 28px. Then move the cursor fully outside: it should fade out about a second later. Re-entering before that second elapses should cancel the hide entirely.

### 5. Hover reveal — the content jump

Title bar off, cursor outside the window. Note the vertical position of the first line of content on screen, then move the cursor into the window.

**Expected to FAIL — this is the known defect.** `#app.peeked` applies `padding-top: 28px`. On macOS `peek_titlebar` grows the frame upward by the same 28px, so the two cancel and the content is stationary. On Windows `peek_titlebar` is a deliberate no-op (`src-tauri/src/commands.rs`, the `#[cfg(not(target_os = "macos"))]` branch), so nothing cancels the padding and the content should shift **down** 28px on every reveal and back up on every hide.

Measure it: report the actual pixel delta, and whether it is exactly 28 logical px at 100% scaling. Check 150% and 200% display scaling too — the padding is in CSS pixels so it should stay 28 CSS px, but confirm rather than assume.

Planned fix, for information only — do not implement unless asked: have `peek_titlebar` return a bool saying whether it really grew the frame, and have `main.ts` apply the offset only when it did.

### 6. Hover reveal — the caption

Title bar off, hover into the window.

**Expected to FAIL as designed.** The Windows caption should *not* come back, because `peek_titlebar` does nothing off macOS. That was deliberate: the Windows caption is non-client area outside the client rect, so toggling it on hover would resize the client area by its height on every hover in and out, and the debounced geometry save would then persist hover-induced sizes. Confirm the caption stays away, and that no window resizing happens on hover beyond the content shift from check 5.

### 7. Dragging

Title bar off. Try dragging the window by the 6px strip at the very top, then by the 28px band once revealed.

**Unverified.** The band carries `data-tauri-drag-region`. Report whether either drag works, and whether starting on the thin strip differs from starting on the revealed band.

### 8. Resizing while frameless

Title bar off. Try to resize from each edge and corner.

**Unverified, and expected to be limited.** The design doc (`docs/superpowers/specs/2026-08-12-jparser-port-design.md`, §11) lists invisible CSS resize edges calling `startResizeDragging` as *unbuilt*. Report which edges, if any, are draggable, and whether the window can be resized at all when frameless.

### 9. Geometry does not creep

Title bar off. Hover in and out ten times. Quit, relaunch, compare the window size to before.

**Expected to PASS.** `scheduleGeometrySave` in `src/main.ts` returns early while a peek is active, so hover-induced resizes are never persisted. Also open `%APPDATA%\dev.jparser.clipyomi\settings.json` and confirm `window_height` did not grow by a multiple of 28.

### 10. Keyboard parity

Title bar off. Click into the window, press Tab.

**Expected to PASS.** The gear is the first tab stop and its `focus` handler drives the same reveal as hover, so it should become visible and focused with a visible ring. Confirm it does not stay invisible — an invisible tab stop is the trap this guards against.

## If you need to see the backend talking

`peek_titlebar` has no logging. To count calls, add one line at the top of the command in `src-tauri/src/commands.rs`:

```rust
eprintln!("[peek] visible={visible} height={height}");
```

`npm run tauri dev` prints it to the terminal. This is exactly how the macOS blink bug was diagnosed: one hover produced five `true`/`false` pairs, proving a feedback loop rather than a one-off repaint. If a single hover on Windows produces a stream of pairs, that is a new bug — report it loudly. Remove the line before you report back.

## What to send back

1. Each numbered check: pass / fail / couldn't test, with what you actually observed.
2. For check 5, the measured pixel delta at 100%, 150%, and 200% scaling.
3. Anything that behaved differently from its "expected" note — including things that unexpectedly *worked*.
4. Windows version, display scaling, and whether you ran a dev build or a bundled one.
5. No commits: no fixes, no Windows screenshot baselines, and no leftover diagnostic `eprintln!`. Report only — fix decisions live with the macOS side.
