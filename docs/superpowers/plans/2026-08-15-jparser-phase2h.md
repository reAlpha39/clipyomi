# JParser Phase 2H — The Clipboard Is the Only Input Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the manual text box so the clipboard is the app's only user-facing input path.

**Architecture:** A removal. The frontend loses four elements, one function and two listeners; the stylesheet loses one rule pair and one grid row; the Rust command that fed the box **stays**, documented as a test and debug entry point with no UI caller. No behaviour is added.

**Tech Stack:** Tauri 2 (`@tauri-apps/api` 2.11.1), Vite, TypeScript (strict), Vitest + happy-dom, Playwright, plain CSS custom properties. No frontend framework. **Rust is untouched apart from one doc comment.**

**Spec:** `docs/superpowers/specs/2026-08-15-jparser-phase2h-design.md` (authoritative, committed at `c28996b`). It settles what `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` §6.3 left open. `ta-old/` is **read-only — never modify it**.

## Global Constraints

- **No frontend framework.** Vanilla TypeScript and DOM APIs only.
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`.** (`app.innerHTML` for the static shell is pre-existing and stays.)
- **Every colour is defined on bare `:root` first** in `src/styles/tokens.css`; the `@media (prefers-color-scheme: dark)` and `:root[data-theme='dark']` blocks may only redefine. This phase adds no colours.
- **Only `transform` and `opacity` are animated.** Any `@media (prefers-reduced-motion: reduce)` override must come **after** the base rule it overrides.
- **`npx tsc --noEmit` clean, `npm run build` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean at the end of every task.** `strict` is on: no `any`, no non-null assertion except where a file already uses one for a required shell element.
- **Rust stays at 357 passing.** This phase deletes no Rust code and adds no Rust tests.
- **The ten screenshot baselines all change, exactly once, in Task 2**, and are inspected rather than accepted. No baseline may change in Task 1 or Task 3.
- **Names are frozen.** Nothing is renamed. `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`, `startup_error`, `settings_warning`, `frontend_ready`, `download_dictionary`, `needs_dictionary` and the events `parse-result`, `parse-error`, `dictionary-status`, `popover-content`, `popover-measured` are all unchanged.
- **File size** 200–400 lines typical, **800 hard maximum** including tests.
- **Scope fence.** The definitions pane, the tooltip window, the clipboard poll and the `InputSender` channel are untouched. No empty-state hint — spec §1 rejects it deliberately.

**Invariants this phase must not break:** the definitions pane remains a complete route to a definition; unmatched runs stay non-interactive `<span>`s never in the tab order; chips stay real `<button>`s with Enter/Space activation; `.panes` keeps `overflow-y: auto`; the popup page never imports `main.ts`.

---

## Resolved facts — do not re-derive these

Measured against the tree at `c28996b`.

| Fact | Value |
|---|---|
| `src/main.ts` | 735 lines |
| `.input-row` markup | `src/main.ts:25-28`, inside the `app.innerHTML` template |
| Element handles | `src/main.ts:34` (`input`), `:35` (`parseButton`) |
| `disabled` assignments | `src/main.ts:61-62` (`showStartupError`), `:112-113` (`renderDictionary` ready branch), `:183-184` (`renderDictionary` idle/failed branch) |
| `run()` and its listeners | `src/main.ts:719-730` |
| Grid | `src/styles/global.css:12-16`, `grid-template-rows: auto auto auto 1fr` |
| `.input-row` CSS | `src/styles/global.css:41-46` and `:48-` (`.input-row input`) |
| `src/main.test.ts` | 663 lines; 1 describe title, 4 test titles and 5 assertions name the controls |
| Vitest sites | describe `:170`; tests `:179`, `:207`, `:514`, `:593`; assertions `:200-201`, `:223-224`, `:534-535`, `:596` |
| `e2e/panes.spec.ts` | 276 lines; `#text` at `:36, :60, :71, :89, :224, :263`; `#parse` at `:37, :61, :72, :90, :225, :264` — **12 lines total** |
| `e2e/popover.spec.ts` | never touches the input; not modified by this phase |
| Baselines | 10 files in `e2e/panes.spec.ts-snapshots/`, all `*-darwin.png` |
| `set_input` | `src-tauri/src/commands.rs:41-49`, doc comment at `:41-45` |
| Baseline counts | Vitest 82, Playwright 20, Rust 357 |

---

## File Structure

| File | Responsibility |
|---|---|
| `src/main.ts` | *(modified)* loses the input markup, two handles, `run()`, two listeners, four `disabled` assignments |
| `src/main.test.ts` | *(modified)* titles and assertions rewritten around what survives; one new test pins the absence |
| `src/styles/global.css` | *(modified)* `.input-row` rules deleted, grid drops a row |
| `e2e/panes.spec.ts` | *(modified)* 12 ceremonial drive lines deleted |
| `e2e/panes.spec.ts-snapshots/*.png` | *(regenerated)* all ten |
| `src-tauri/src/commands.rs` | *(modified)* doc comment only — why `set_input` has no caller |
| `docs/superpowers/specs/2026-08-12-jparser-port-design.md` | *(modified)* §1 scope line |
| `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` | *(modified)* §6 closing paragraph, §6.3 annotation |

---

## Task 1: Remove the input from the frontend

**Files:**
- Modify: `src/main.ts`, `src/main.test.ts`, `src/styles/global.css`

**Interfaces:**
- Consumes: nothing.
- Produces: a DOM with no `#text` and no `#parse`. Task 2's e2e edits depend on that absence.

- [ ] **Step 1: Write the failing test**

In `src/main.test.ts`, add this new describe at the **end** of the file. It mirrors the setup the other describes use, so it is self-contained:

```ts
// Phase 2H: the clipboard is the only user-facing input path. `#text` and
// `#parse` were the two elements a user could type into or click to parse, so
// their absence is what makes that claim true. Asserted on the rendered shell
// rather than on the source, because the shell is what a user gets.
describe('the input surface', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  test('renders no manual text input', () => {
    expect(document.querySelector('#text')).toBeNull();
    expect(document.querySelector('#parse')).toBeNull();
    expect(document.querySelector('.input-row')).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npx vitest run src/main.test.ts -t "renders no manual text input"`

Expected: FAIL — all three elements currently exist, so the first `expect` reports a received element rather than `null`. That is the intended RED.

- [ ] **Step 3: Rewrite the tests whose titles or assertions name the controls**

Five edits in `src/main.test.ts`. **Two of them replace an assertion rather than deleting it** — deleting alone would leave a test asserting nothing, which the review rubric treats as a defect.

3a. The describe at `:170`:

```ts
describe('main: a startup failure disables the parse controls', () => {
```

becomes:

```ts
describe('main: a startup failure reports itself', () => {
```

3b. The test at `:179`:

```ts
  test('startup_error resolving to a message disables #text and #parse', async () => {
```

becomes:

```ts
  test('startup_error resolving to a message renders it into #output', async () => {
```

and its two assertions at `:200-201` are **deleted outright** — the `.startup-error` assertion immediately below them already carries the test:

```ts
    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(true);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(true);
```

3c. The test at `:207`:

```ts
  test('startup_error resolving to null leaves the controls enabled', async () => {
```

becomes:

```ts
  test('startup_error resolving to null renders nothing', async () => {
```

and its two assertions at `:223-224` are **replaced** — this test has no other assertion, so deleting them would empty it:

```ts
    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(false);
```

becomes:

```ts
    expect(document.querySelector('.startup-error')).toBeNull();
```

3d. The test at `:514`:

```ts
  test('a non-null settings_warning renders into #parse-error and leaves #text/#parse enabled', async () => {
```

becomes:

```ts
  test('a non-null settings_warning renders into #parse-error and leaves #output alone', async () => {
```

Its comment and two assertions at `:530-535` become:

```ts
    // The point of this assertion: a settings warning is cosmetic. Unlike a
    // fatal `startup_error` it must never touch `output` — nothing was parsed.
    expect(document.querySelector('#output')?.children).toHaveLength(0);
```

(The `#output` assertion already existed at `:536`; the two `disabled` lines above it go.)

3e. The test at `:593`:

```ts
  test('ready clears the screen and re-enables the controls', () => {
```

becomes:

```ts
  test('ready clears the screen', () => {
```

and its assertion at `:596` is **deleted** — the `childElementCount` assertion above it carries the test:

```ts
    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
```

- [ ] **Step 4: Remove the input from the markup and the module**

In `src/main.ts`, delete these four lines from the `app.innerHTML` template (`:25-28`):

```html
  <div class="input-row">
    <input id="text" type="text" aria-label="Japanese text to parse" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
```

Delete both element handles (`:34-35`):

```ts
const input = app.querySelector<HTMLInputElement>('#text')!;
const parseButton = app.querySelector<HTMLButtonElement>('#parse')!;
```

Delete all three `disabled` pairs. In `showStartupError` (`:61-62`) and in `renderDictionary`'s idle/failed branch (`:183-184`):

```ts
  input.disabled = true;
  parseButton.disabled = true;
```

and in `renderDictionary`'s `ready` branch (`:112-113`):

```ts
    input.disabled = false;
    parseButton.disabled = false;
```

**Leave `restoreFocus` and every other line of `renderDictionary` alone.** It exists because `replaceChildren` moves focus to `<body>`, and it sits immediately beside the lines you are deleting (spec §9).

Delete `run()` and both its listeners (`:719-730`):

```ts
async function run(): Promise<void> {
  try {
    await invoke('set_input', { text: input.value });
  } catch (e) {
    parseError.replaceChildren(errorBlock(String(e)));
  }
}

parseButton.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});
```

`parseError` and `errorBlock` both stay — `showSettingsWarning` and the `parse-error` listener still use them.

- [ ] **Step 5: Remove the styles and the grid row**

In `src/styles/global.css`, delete the `.input-row` rule and the `.input-row input` rule (from `:41` through the end of the second rule). Then change the grid at `:14`:

```css
  grid-template-rows: auto auto auto 1fr;
```

to:

```css
  grid-template-rows: auto auto 1fr;
```

The remaining three rows are the header, `#parse-error`, and the `1fr` pane region. Grep the file for `input-row` afterwards and confirm zero matches.

- [ ] **Step 6: Run the tests to verify they pass**

```bash
npx vitest run
npx tsc --noEmit
npm run build
```

Expected: Vitest **83** (82 plus the new absence test); `tsc` silent; the build emits both `index.html` and `popover.html`. Report the actual count — do not assume it. If it is not 83, say so and explain which test changed count rather than adjusting a test to fit.

Do **not** run Playwright in this task. Its baselines are now stale by design and Task 2 owns them.

- [ ] **Step 7: Commit**

```bash
git add src/main.ts src/main.test.ts src/styles/global.css
git commit -m "feat: remove the manual text input"
```

---

## Task 2: The e2e drive lines and the baselines

**Files:**
- Modify: `e2e/panes.spec.ts`
- Regenerate: all ten files in `e2e/panes.spec.ts-snapshots/`

**Interfaces:**
- Consumes: the DOM without `#text`/`#parse` from Task 1.
- Produces: nothing later tasks import.

- [ ] **Step 1: Delete the ceremonial drive lines**

`e2e/panes.spec.ts` drives the app with a `fill`/`click` pair in six places. **Both lines of each pair go — twelve lines total**, at `:36-37`, `:60-61`, `:71-72`, `:89-90`, `:224-225`, `:263-264` (line numbers before any deletion; work bottom-up or re-find each pair):

```ts
  await page.fill('#text', '東京は');
  await page.click('#parse');
```

Leave every `emitFixtureResult(page)` call exactly where it is — that is what actually renders, and it is why these twelve lines were ceremony rather than coverage (spec §5).

Update the file's header comment, which currently reads:

```ts
// The backend now pushes results as a `parse-result` event rather than
// returning them from `invoke`; `#parse` still triggers `set_input` for real
// (see stub.ts), but firing the fixture back in is on the test, standing in
// for the backend's async worker.
```

to:

```ts
// The backend pushes results as a `parse-result` event rather than returning
// them from `invoke`, so firing the fixture back in is on the test, standing in
// for the backend's async worker. Phase 2H removed the manual input the specs
// used to click; it never reached Rust here anyway, since Playwright runs
// against the stub.
```

Report the file's line count before and after, and confirm the spec count is unchanged.

- [ ] **Step 2: Confirm the baselines are the only thing left failing**

Run: `CI=1 npx playwright test`

Expected: **20 passed.** `CI=1` skips every `toHaveScreenshot`, so this proves the twelve deletions broke no behavioural assertion before any image is touched. If anything fails here, stop — it is a real regression, not a baseline drift.

- [ ] **Step 3: Regenerate the baselines**

```bash
npx playwright test --update-snapshots
git status --porcelain e2e/panes.spec.ts-snapshots/
```

Expected: exactly ten modified files, no additions and no deletions. **If any file is added or removed, stop and report** — the set is fixed at ten and a new name means a spec title changed.

- [ ] **Step 4: Inspect every regenerated baseline**

**Required, and the most important step in this phase.** Open all ten images and look at them. Phase 2G committed a baseline that ratified a serif gloss nobody had asked for, and this task regenerates every baseline at once — the largest opportunity in the project's history to make a defect official.

For each of `panes-{compact,default}-{light,dark}.png`, `panes-activated-{light,dark}.png` and the four `panes-download-*.png`, confirm:

1. The input row is **gone**, and nothing occupies the space it left.
2. The header (`Always on top`, `Monitoring`) sits directly above the panes region.
3. Sentence chips, definition rows and their colours are unchanged from the previous baseline — this phase touched no colour and no font.
4. Nothing is clipped at the compact 480×320 size.

Report what you saw per image, in one line each. "They look fine" is not a report.

- [ ] **Step 5: Run both Playwright modes**

```bash
npx playwright test
CI=1 npx playwright test
```

Expected: **20 passed** both ways. Report both totals.

- [ ] **Step 6: Commit**

```bash
git add e2e
git commit -m "test: drive the panes specs without the removed input"
```

---

## Task 3: Document the surviving command, amend the specs, and run the gate

**Files:**
- Modify: `src-tauri/src/commands.rs`, `docs/superpowers/specs/2026-08-12-jparser-port-design.md`, `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md`

**Interfaces:**
- Consumes: everything Tasks 1 and 2 produced.
- Produces: nothing.

- [ ] **Step 1: Document why `set_input` has no caller**

This is the step that stops a future cleanup sweep deleting it. In `src-tauri/src/commands.rs`, the doc comment at `:41-45` currently reads:

```rust
/// Queue TEXT for parsing. The result arrives as a `parse-result` event.
///
/// Fire-and-forget rather than request/response: the clipboard produces parses
/// nobody asked for, so the webview renders from the event stream either way and
/// a returned value here would be a second, redundant path.
```

Append to it, keeping the existing text:

```rust
///
/// **This command deliberately has no caller.** Phase 2H removed the manual
/// text box, making the clipboard the only user-facing input path, and with it
/// the one `invoke` that reached this. It is kept as a test and debug entry
/// point: with clipboard-only input, exercising a parse by hand otherwise means
/// putting text on the system clipboard for every attempt, and this can be
/// driven from the DevTools console instead. Not dead code — do not remove it
/// without replacing that affordance.
```

Change nothing else in the file. The command body, its two tests, and its entry in `generate_handler!` all stay exactly as they are.

- [ ] **Step 2: Amend port design §1**

In `docs/superpowers/specs/2026-08-12-jparser-port-design.md`, the in-scope list contains:

```markdown
- Clipboard auto-monitoring plus manual text entry.
```

Replace it with:

```markdown
- Clipboard auto-monitoring. (Manual text entry was removed in Phase 2H — see
  `docs/superpowers/specs/2026-08-15-jparser-phase2h-design.md`. The clipboard is
  the only user-facing input path.)
```

- [ ] **Step 3: Amend 2E §6**

In `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md`, §6 ends with:

```markdown
The manual text input stays. Port design §1 is explicit that the app offers
"clipboard auto-monitoring plus manual text entry" — the clipboard does not
replace the box.
```

Replace that paragraph with:

```markdown
The manual text input stays **for this phase only**. §6.3 below schedules its
removal, and Phase 2H carried it out — the clipboard did replace the box. See
`docs/superpowers/specs/2026-08-15-jparser-phase2h-design.md`.
```

- [ ] **Step 4: Annotate 2E §6.3 with the cost correction**

Still in the 2E design, §6.3's known-cost list contains:

```markdown
- All Playwright specs currently drive the app via `page.fill('#text')` +
  `page.click('#parse')`; every one must be rewritten to emit a `parse-result`
  event instead.
```

Append immediately below that bullet:

```markdown
  **Corrected when 2H landed:** they already emitted. Playwright runs against
  the Vite dev server with `__TAURI_INTERNALS__` stubbed, so the `set_input`
  that `#parse` triggered never reached Rust, and `emitFixtureResult` was
  already what produced every render. The `fill`/`click` pairs were ceremony:
  twelve lines deleted, no spec rewritten, no spec count changed.
```

- [ ] **Step 5: Run the full gate**

```bash
npx vitest run
npx tsc --noEmit
npm run build
ls dist/
npx playwright test
CI=1 npx playwright test
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
git status --porcelain
```

Expected: Vitest 83; Playwright 20 both ways; Rust **357, unchanged**; clippy clean; `tsc` silent; `dist/` contains both `index.html` and `popover.html`; the working tree clean apart from any pre-existing untracked images in the repo root, which are **not yours to add**. Report every count.

- [ ] **Step 6: Verify by hand what no test covers**

**Required.** `npm run tauri dev`, then:

1. Confirm the window opens with **no text box** — header, then the panes region.
2. Press Tab from a fresh window. The first focusable element should now be **Always on top**, where it used to be the text box. Confirm the focus ring is visible on it (spec §9 names this as the one manual check).
3. Copy a Japanese sentence to the clipboard. Confirm it parses and renders, proving the clipboard path is unaffected by the removal.
4. Open the DevTools console and run
   `await window.__TAURI_INTERNALS__.invoke('set_input', { text: '東京は' })`.
   Confirm it parses. This is the affordance Step 1 documented, and this is the only check that it actually works.

Report exactly what you observed. If something cannot be verified in your environment, say so plainly rather than claiming it — this project has shipped defects behind green suites in 2E, 2F, 2G and 2I, and every one was found by an honest manual pass or not at all.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands.rs docs/superpowers/specs
git commit -m "docs: record that set_input survives as a debug entry point"
```

---

## Self-Review

**1. Spec coverage.**

| Spec requirement | Task |
|---|---|
| §1 delete `#text`, `#parse`, `.input-row` and their CSS | 1 Steps 4–5 |
| §1 delete `run()` and its two listeners | 1 Step 4 |
| §1 keep `set_input`, document why | 3 Step 1 |
| §1 remove `disabled` toggling from both functions | 1 Step 4 |
| §1 delete the ceremonial drive lines | 2 Step 1 |
| §1 regenerate all ten baselines | 2 Steps 3–4 |
| §1 amend port design §1, 2E §6, 2E §6.3 | 3 Steps 2–4 |
| §1 no empty-state hint | Not implemented, by design |
| §2.1 four `disabled` assignments, `#parse-error` untouched | 1 Step 4 |
| §2.2 `.input-row` CSS removed | 1 Step 5 |
| §2.3 Rust otherwise untouched, 357 holds | 3 Steps 1 and 5 |
| §3 monitoring keeps running; errors via `#parse-error` | No code needed — the `parse-error` listener already does this, and Task 1 Step 4 removes the only thing that suppressed it |
| §4 grid drops to `auto auto 1fr` | 1 Step 5 |
| §5 Vitest titles and assertions rewritten | 1 Step 3 |
| §5 Playwright unchanged in count | 2 Steps 1 and 5 |
| §5 baselines inspected, not accepted | 2 Step 4 |
| §8 inherited constraints | Global Constraints |
| §9 tab-order check | 3 Step 6 item 2 |
| §9 `restoreFocus` undisturbed | 1 Step 4, called out explicitly |

**2. Placeholder scan.** No `TBD`, no `TODO`, no "similar to Task N". Every edit quotes the exact text being replaced and the exact text replacing it. Three steps direct the implementer to report rather than assume: Task 1 Step 6 (the Vitest count), Task 2 Step 4 (per-image inspection), and Task 3 Step 6 (the manual pass).

**3. Type consistency across task boundaries.** This phase defines no new types, functions or signatures. The only cross-task dependency is Task 2 relying on the DOM Task 1 produces — `#text` and `#parse` absent — which Task 1 Step 1 pins with an explicit test.

**4. Residual risks a human should look at.**

- **Ten baselines change at once.** Task 2 Step 4 is the only thing standing between a layout defect and a committed baseline that makes it official. Do not let it be skipped or answered with "they look fine".
- **The Vitest count is expected to rise to 83, not hold at 82.** The new absence test is an addition; the five assertion edits net to zero tests. An implementer who "fixes" a count mismatch by deleting the new test has removed the only guard on the phase's whole premise.
- **`set_input` is now uncalled by design.** Task 3 Step 1's comment is the only thing marking it as intentional. If a future dead-code sweep runs `knip`, `ts-prune` or an equivalent Rust pass, the command will surface as unused and the comment is the reason not to act on it.
- **Step 6 item 4 is the only test of the debug affordance.** Nothing automated covers `set_input` end to end after this phase, because Playwright stubs the IPC boundary and the Rust tests exercise the channel push rather than the command over IPC. If the console invoke does not work, the justification for keeping the command is weaker than the spec assumes.
