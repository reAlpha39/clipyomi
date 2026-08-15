# JParser Phase 2H — The Clipboard Is the Only Input (Design)

2E promised this phase and named it: the manual text box goes, and the clipboard
becomes the app's single input path. This is a removal phase. It adds no
behaviour, and its value is that one input path means one source of truth for
"the current parse".

**Reference:** `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` §6.3
specified this phase in advance and is the authority for the decision itself.
This document settles what §6.3 left open and corrects one cost it overestimated.

**Predecessor code:** the shipped 2I tree at `91e6963`.

**Sequencing.** 2E §6.3 fixed 2H as the phase after 2G; 2I overtook it because 2G's
tooltip did not work at the app's own minimum window size. 2H now follows 2I. The
`ta-old/` tree is **read-only — never modify it**.

## 1. Scope

**In scope:**

- Delete `#text`, `#parse`, the `.input-row` wrapper, and their CSS
- Delete `run()` and its two listeners
- Keep the `set_input` command, and document why it now has no caller (§2.3)
- Remove the `disabled` toggling from `showStartupError` and `renderDictionary`
- Delete the ceremonial drive lines in `e2e/panes.spec.ts`
- Regenerate all ten screenshot baselines
- Amend port design §1, 2E §6, and 2E §6.3

**Not in scope:**

- **An empty-state hint.** Considered and rejected: with no text box, a first-time
  user sees two toggles and an empty pane with nothing naming the clipboard as the
  input. A one-line hint in `#output` would fix that for a few lines of markup.
  It is deliberately not in this phase — discoverability is its own decision and
  this phase stays a pure removal. Recorded so it stays a choice rather than an
  oversight.
- **The clipboard poll and the `InputSender` channel.** The poll becomes the sole
  producer, but the channel keeps its current shape. Collapsing a one-producer
  watch channel is a refactor this phase does not need.
- **The definitions pane, the tooltip, pane density, gloss filters, furigana.**
  Unchanged fences: the pane is Phase 3, the tooltip shipped in 2I.

## 2. What is removed

### 2.1 The frontend

`src/main.ts` loses, in `app.innerHTML`, the entire `.input-row` div; then the
`input` and `parseButton` element handles, the `run()` function, its `click`
listener on the parse button, and its `keydown` listener for Enter.

Four `disabled` assignments go with them: two in `showStartupError`, and the pair
in `renderDictionary` that disable on the idle/failed branch and re-enable on
`ready`. Nothing replaces them — after this phase there is no control whose
availability depends on whether an index exists.

`#parse-error` stays exactly as it is. It is the slot the `parse-error` event
already renders into, and §3 below makes it carry more weight, not less.

### 2.2 The styles

`.input-row` and `.input-row input` come out of `src/styles/global.css`. No other
rule in that file references either.

### 2.3 The command stays

`commands::set_input` has exactly one production caller — the `invoke` inside
`run()` — so removing `run()` leaves it with none. **It stays anyway**, and Rust
is otherwise untouched by this phase: no change to `generate_handler!`, no deleted
tests, and `parse.rs`'s comment about "two senders — the clipboard poll's and the
`set_input` command's" stays accurate as written. Rust holds at 357 tests.

The reason is testing. With the clipboard as the only user-facing input,
exercising the app by hand means putting text on the system clipboard for every
attempt. `set_input` is a programmatic way in — from the DevTools console, or from
a harness — that costs nothing to keep and is genuinely awkward to reconstruct
later.

This does not weaken the phase's rationale. §6.3's "one source of truth for the
current parse" is about the **user-facing** input path, and after this phase that
is the clipboard alone. A command reachable only from a developer console or a
test is not a second way for a user to enter text.

**One addition is required, not optional.** The command must carry a doc comment
saying it deliberately has no caller and why — that it is a test and debug entry
point, kept on purpose. Without it the command is indistinguishable from dead
code, and the next reader doing a cleanup sweep deletes it. That reading is not
hypothetical: this spec's first draft proposed deleting it, on exactly that
evidence.

## 3. The fatal state

`showStartupError` currently does two things: it writes the backend's message into
`#output`, and it disables `#text` and `#parse`. The disabling is what actually
stops a user parsing with no index. With both controls gone, something has to
decide what happens when the clipboard delivers text into a broken parser.

**Monitoring keeps running, and each failed parse surfaces its own error.** The
`parse-error` event already routes into `#parse-error`; a copy made with no usable
index produces an error there, the same as any other parse failure. The startup
message stays in `#output` explaining why.

Two alternatives were rejected. Forcing monitoring off and locking the toggle
would replace two disabled controls with one, but it invents a new locked state
whose only purpose is to prevent an error the app already knows how to report.
Leaving the error and doing nothing at all would let failures pass silently.
Per-parse errors need no new state and no new control, and they tell the truth
every time rather than once at startup.

## 4. Layout

The header, `#parse-error` and `.panes` all move up by the input row's height.
`#app`'s `grid-template-rows` goes from `auto auto auto 1fr` to `auto auto 1fr`,
reversing the change 2E §6 made when it added the header above the input row. The
pane region stays the `1fr` row.

That is the phase's only visual change, and it is why all ten screenshot baselines
regenerate.

## 5. Testing

**Vitest:** `src/main.test.ts` asserts on `#text`/`#parse` in five places across
four tests, and two of those tests are *named* for the controls —
`startup_error resolving to a message disables #text and #parse` and
`a non-null settings_warning renders into #parse-error and leaves #text/#parse
enabled`. Deleting the assertions alone would leave two tests whose titles
describe behaviour that no longer exists.

Both are rewritten around what survives, which is the part that always mattered:
the startup error still renders its message into `#output`, and the settings
warning still renders into `#parse-error`. The three remaining "control is
enabled" assertions are deleted outright, since there is no control to be enabled.
No test loses its purpose and no test disappears, so the Vitest count is expected
to hold — the implementation confirms it rather than assuming it. The clipboard
path's own coverage is untouched throughout.

**Playwright — and here 2E §6.3 overestimated the cost.** §6.3 warned that "all
Playwright specs currently drive the app via `page.fill('#text')` +
`page.click('#parse')`; every one must be rewritten to emit a `parse-result` event
instead." They already do. Playwright runs against the Vite dev server with
`__TAURI_INTERNALS__` stubbed, so the `set_input` that `#parse` triggers never
reaches Rust, and `emitFixtureResult` is what actually produces every render. The
`fill`/`click` pair is ceremony. Six lines are deleted across `panes.spec.ts`;
`popover.spec.ts` never touched the input. No spec is rewritten and no spec count
changes.

**Baselines:** all ten regenerate, and they are **looked at**, not accepted
blindly. 2G shipped a serif gloss that a committed baseline had ratified, and this
phase regenerates every baseline at once — the single largest opportunity in the
project's history to ratify a defect.

**Rust:** unchanged at 357. This phase touches no Rust beyond the doc comment
§2.3 requires, which is why the `set_input` tests stay green rather than
disappearing with their command.

## 6. Documents amended

- **Port design §1** lists "Clipboard auto-monitoring plus manual text entry" in
  scope. The second half is struck, with a pointer here.
- **2E §6** ends "The manual text input stays. Port design §1 is explicit that the
  app offers 'clipboard auto-monitoring plus manual text entry' — the clipboard
  does not replace the box." That paragraph is replaced by a pointer to this
  phase.
- **2E §6.3** is annotated with the Playwright correction in §5 above, so the next
  reader does not budget for a rewrite that is not needed.

§6.3 said these amendments happen "when 2h lands, not before". They land with it.

## 7. What is lost

Text that cannot be copied — in an image, handwritten, or heard — now has to be
typed into another application and copied back. 2E §6.3 weighed this and accepted
it; restated here so it survives as a decision rather than being rediscovered as a
regression.

One objection considered and rejected in §6.3, restated because it is the first
one anybody raises: pausing the monitor with no text box does not leave the app
without input. 2E §1.1 defines pause as "pause to study the current parse, unpause
to resume" — freezing the current result is the entire purpose, so there is
nothing to type during it.

## 8. Constraints inherited

- **No frontend framework**; vanilla TypeScript and DOM APIs only
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`**
- **Every colour on bare `:root` first**; the two dark blocks may only redefine
- **Only `transform` and `opacity` animated**, with any `prefers-reduced-motion`
  override placed after the base rule it overrides
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **`npx tsc --noEmit` clean**, `npm run build` clean, `cargo clippy --workspace
  --all-targets -- -D warnings` clean
- Chips stay real `<button>`s with Enter/Space activation; unmatched runs stay
  non-interactive `<span>`s never in the tab order; `.panes` keeps
  `overflow-y: auto`; the popup page never imports `main.ts`
- **`ta-old/` is read-only**

## 9. Risks

- **Baseline regeneration is the phase's one irreversible-feeling step.** Ten
  images change at once, so a defect introduced anywhere in the layout gets
  committed as the new truth unless each is inspected. This is the phase's largest
  risk and it is entirely mitigable by looking.
- **The first tab stop changes.** The text box was the first focusable element;
  after this it is the always-on-top button. No test asserts tab order from the
  top of the document, so this is a manual check.
- **Removing `disabled` handling touches the download flow.** `renderDictionary`
  is the most stateful function in the frontend and its focus-restoration logic
  sits beside the lines being deleted. The deletion must not disturb
  `restoreFocus`, which exists because `replaceChildren` moves focus to `<body>`.
