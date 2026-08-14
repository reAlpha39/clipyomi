# JParser Phase 2E — Clipboard Monitoring and Settings (Design)

2D put parsed Japanese on screen, but only when someone typed into a text box.
2E makes the app autonomous: it watches the clipboard, parses what it finds, and
remembers whether you wanted it to.

**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` is
authoritative for the app shell's module split (§6), the clipboard and
latest-wins rules (§6), settings (§8), error policy (§9), and UI direction (§7).
This spec narrows those to what 2E builds and records the decisions that
document does not fix.

**Predecessor:** `docs/superpowers/specs/2026-08-14-jparser-phase2d-design.md`
and the shipped 2D code at `ae51029`. 2E changes one of 2D's IPC contracts; §4
says which and why.

## 1. Scope

The port design's Phase 2 bullet is five subsystems. 2D took the slice that gets
pixels on screen. 2E takes the slice that makes the app run by itself.

**In scope:**

- A 200 ms clipboard poll with the port design's four skip conditions
- A single parse worker with latest-wins semantics, fed by both the clipboard and
  the text box
- Parse results pushed to the webview as events rather than returned from a
  command
- An always-on-top toggle
- A clipboard pause toggle
- A settings file persisting exactly those two toggles

**Deferred to 2F:** the first-run dictionary download. Until it lands, a user
builds an index with `jparser-cli ensure-dictionary`, and 2D's no-index startup
message already names that remedy.

**Deferred to Phase 3:** the settings popover, theme control, furigana modes,
font sizes, gloss filters — and **definition-pane density**, which real
dictionary data exposed as a genuine problem (§6.1) but which is a typography
question, not a concurrency one.

**Deferred to Phase 4:** parse history, the three chrome states, per-pixel
transparency, layout hotkeys, persisted window geometry.

### 1.1 Monitoring without history

Port design §7.8 says session history "is what makes monitoring safe to leave
running", then schedules history in Phase 4 — two phases after the monitor.

2E ships the monitor anyway, with the **pause toggle as the mitigation**: pause
to study the current parse, unpause to resume. This accepts a real rough edge —
a copy landing before you reach the toggle destroys the previous result — in
exchange for not pulling ring-buffer state and back/forward navigation into a
phase that otherwise has no history UI at all. Phase 4 builds history whole
rather than inheriting half of it.

Monitoring defaults **on**. It is the app's headline behaviour, and a user who
does not want it has a visible toggle on first launch.

## 2. Architecture

| File | Responsibility |
|---|---|
| `src-tauri/src/clipboard.rs` | *(new)* 200 ms poll; the four skip conditions as a pure predicate |
| `src-tauri/src/parse.rs` | *(new)* single worker, latest-wins, result emission |
| `src-tauri/src/settings.rs` | *(new)* JSON in the app config dir; load, save, defaults |
| `src-tauri/src/commands.rs` | *(modified)* `set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings` |

**Names are frozen**, because both sides of an IPC boundary hard-code them and a
rename compiles clean on each side while breaking the app. Commands:
`set_input`, `set_always_on_top`, `set_clipboard_monitoring`, `get_settings`.
Events: `parse-result` and `parse-error`. Settings file: `settings.json` in the
app config dir — a sibling of 2D's `dict/`, not inside it, since a generation
directory is immutable once published. Keys: `always_on_top` (default `false`)
and `clipboard_monitoring` (default `true`).
| `src-tauri/src/main.rs` | *(modified)* spawn the poll and the worker in `setup` |
| `src/main.ts` | *(modified)* render from the event stream; header controls |

**No `window.rs` this phase.** Port design §6 lists one, but in 2E it would hold
a single `set_always_on_top` call. Chrome modes, geometry, and layout slots are
Phase 4; the file earns its existence then. Always-on-top lives in `commands.rs`
until it has company.

`crates/jparser` gains nothing. The parser does not know a clipboard exists.

## 3. The four skip conditions

The poll's decision is extracted as a pure function:

```rust
fn should_parse(text: &str, last_seen: Option<&str>, last_written: Option<&str>) -> bool
```

It returns false when the text is unchanged since the last tick, contains no kana
and no CJK ideograph, matches what this app last wrote to the clipboard, or
exceeds 10 000 characters.

The third condition is why `last_written` exists: copying an entry out of our own
definition pane must not trigger a re-parse of our own output. Nothing in 2E
writes to the clipboard yet, so `last_written` is always `None` this phase — it
is in the signature because the predicate is the thing being tested, and adding
the parameter later would invalidate those tests.

The 10 000-character cap is a soundness guard, not a performance one: the matcher
does offset arithmetic over the whole input, and an unbounded paste is the
easiest way to find out what that costs.

Extracting this predicate is what makes the poll testable. The system clipboard
is global mutable state shared with the developer's actual desktop; §7.1 explains
why no test touches it.

## 4. Data flow

```
clipboard poll (200 ms) ─┐
                         ├─→ watch::Sender<String> ─→ worker ─→ spawn_blocking(jparser::parse) ─→ emit("parse-result")
text box (set_input) ────┘
```

A `tokio::sync::watch` channel retains only its newest value, which is exactly
the port design's latest-wins rule: text arriving mid-parse supersedes the
in-flight input rather than queueing behind it. ta-old's note is that local
algorithms "stop and rerun"; a watch channel is that behaviour with no
bookkeeping.

The worker awaits `changed()`, clones the current value, and runs the parse on
`spawn_blocking` — the same reasoning as 2D's `parse_text`, since `jparser::parse`
is synchronous CPU work over the whole input.

### 4.1 One path, and what it costs

Both the clipboard and the text box feed the same channel, and the webview
renders purely from `parse-result` events. There is one source of truth for
"the current parse" and latest-wins applies uniformly.

**This changes a 2D contract.** `parse_text(text) -> Result<ParseResult, String>`
becomes `set_input(text)`, fire-and-forget. The alternative — keeping the command
for manual input and adding events only for the clipboard — means two ways for a
result to reach the screen, two error models, and a race when a clipboard result
lands mid-manual-parse. One path is worth the migration.

The migration is not free and the plan must budget for it: `src/main.ts`'s
`run()` stops awaiting a result, `src/main.test.ts`'s parse-failure test moves to
the event path, and `commands.rs`'s `run_parse` tests change shape. That work is
Phase 2E's, not a surprise for whoever writes the tasks.

## 5. Error handling

| Boundary | Policy |
|---|---|
| **Clipboard read** | Log-and-skip the tick. Deliberately not surfaced — another app briefly holding the clipboard is not information the user can act on. |
| **Parse error** | Emit `parse-error` with the message; the previous result stays on screen. |
| **Parse panic** | `catch_unwind` at the worker boundary. Report with the input length, keep the previous result, and keep the worker alive. The matcher does offset arithmetic; a panic must not take the app down with it. |
| **Settings missing** | Defaults, silently. First run is not an error. |
| **Settings corrupt** | Defaults, with the reason surfaced. Never a failed startup — same principle as 2D's no-index state. |
| **Settings write fails** | Surface it; keep running with the in-memory value. A read-only config dir must not make the toggles stop working. |

**Unknown keys are preserved across a rewrite.** A settings file written by a
later version, opened by this one, must not lose the keys this version does not
understand. Phases 3 and 4 add many; a downgrade that silently discards them is a
data-loss bug that would surface long after the downgrade.

## 6. UI

A header row above the existing input row. `#app`'s grid goes from three rows to
four — `auto auto auto 1fr` — and the fourth stays the scrollable pane region.

Two controls, both native `<button>` with `aria-pressed` reflecting state:
always-on-top and clipboard pause. 2D established that native buttons are the
right primitive here — they carry Enter/Space activation and tab order without an
ARIA widget — and its focus-ring and reduced-motion rules apply unchanged.

The port design's full header (`[— ひ カ R]` · always-on-top · clipboard pause ·
⚙) is Phase 3/4; 2E ships the two controls it has behaviour for and leaves the
row's remaining space empty rather than stubbing dead affordances.

The manual text input stays. Port design §1 is explicit that the app offers
"clipboard auto-monitoring plus manual text entry" — the clipboard does not
replace the box.

### 6.1 Pane density is a known Phase 3 input

Running 2D against the real 218 431-entry index showed the definition pane fits
roughly two and a half entries at the default 720×480: a six-segment sentence is
mostly scrolling. The 24 px headword and its vertical rhythm read as generous in
a screenshot and sparse in use.

This is recorded here so Phase 3 inherits a measured observation rather than
rediscovering it, and deliberately not fixed in 2E — it belongs with font sizes
and gloss filters, decided together.

### 6.2 Hover-to-preview is a known Phase 3 input

ta-old showed a definition on **hover**, not on click: `FuriganaWindow.cpp:716-730`
tracks `WM_MOUSEMOVE`, arms `TrackMouseEvent` with `TME_HOVER` and
`dwHoverTime = 350`, and pops a tooltip (`MyToolTip.cpp`) once the cursor has
dwelt on a word for 350 ms.

The port replaced that deliberately — port design §7.2 assigns hover a purely
visual role ("hover lifts the surface; click scrolls to and marks the definition
row") and moves the content into a permanent pane listing every span in sentence
order. That trade is sound for reading a whole sentence: every definition is
visible at once, it survives the mouse moving away, and the chips are real
`<button>`s carrying Enter/Space activation and a focus ring, which a hover
tooltip cannot be.

What it loses is the **glance** — checking one unfamiliar word without spending a
click and then finding the matching row. First real use after 2E surfaced this
immediately.

Recorded here rather than fixed because it is the same complaint as §6.1 from the
other direction: the pane is where definitions live and there is not enough room
in it. Deciding hover separately from density, font sizes, and gloss filters would
mean deciding the same thing twice.

The likely shape is **additive, not a reversion** — keep the pane, add a hover
popover for the glance case, gated behind a dwell like ta-old's 350 ms so it does
not fire while the cursor sweeps across a sentence. A popover also inherits 2D's
constraints: `transform`/`opacity` only, `prefers-reduced-motion` respected, and
it must not become the only route to a definition, since hover has no keyboard
equivalent.

## 7. Testing

### 7.1 Why no test touches the clipboard

The system clipboard is global mutable state shared with whatever else is running
on the machine, including the developer's own copy buffer. A test that writes to
it races every other test in the binary and corrupts the desktop it runs on; a
test that reads it depends on what the developer copied last.

Coverage comes from the seams instead:

- **`should_parse`** — exhaustively, one case per skip condition plus the passing
  case, plus the boundary at 10 000 characters. This is where the poll's actual
  logic lives.
- **Latest-wins** — driven directly through the watch channel: send three inputs,
  assert the worker observes the newest and not the intermediate. No clipboard
  involved.
- **Panic containment** — the `catch_unwind` wrapper is its own function,
  `catch_parse(f: impl FnOnce() -> Result<ParseResult, String> + UnwindSafe)`,
  so the test passes it a closure that panics and asserts it returns an `Err`
  naming the input length rather than unwinding. Testing this through
  `jparser::parse` itself would mean engineering a parser input that panics,
  which is both hard and a moving target; the wrapper is the thing whose
  correctness is actually in question.
- **Settings** — round-trip, missing file, corrupt file, and unknown-key
  preservation, each against a scratch directory.

The 200 ms poll loop itself is a thin shell over `should_parse` and a clipboard
read, and stays untested. That is the deliberate trade: the logic is tested, the
timer is not.

**Always-on-top is not tested automatically either**, and that is deliberate
rather than an oversight — port design §10 puts it under "Not tested", alongside
transparency and chrome switching, on a manual per-platform checklist. Automating
window-manager behaviour is more fragile than the bugs it would catch. The
command is one call into Tauri's window API; what could break is the window
manager's response to it, which no assertion in this repo can observe. Verify by
hand: toggle it on, click another window, confirm ours stays in front.

Note also that `should_parse`'s "unchanged" condition can only be evaluated after
`read_text()`, so the poll pays a clipboard read every tick regardless. That cost
is accepted: five reads a second of a ≤10 000-character string is negligible, and
avoiding it would mean platform-specific change-counter code for no measurable
gain.

### 7.2 Frontend

Vitest covers the event-driven render path with `listen` mocked, the header
toggles' `aria-pressed` state, and that a `parse-error` event leaves the previous
result on screen. Playwright stubs `listen` and `emit` the same way it already
stubs `invoke`, and the committed baselines gain the header row.

### 7.3 Coverage

80 % stays a real line on `crates/jparser`, which 2E does not touch. `src-tauri`
remains ungated for the reasons 2D recorded — but the modules 2E adds are more
testable than 2D's were, and the number should move up on its own. Report it;
do not add tests to move it.

## 8. Resolved facts

Measured against the tree at commit `ae51029`.

| Fact | Value |
|---|---|
| `tauri::async_runtime` re-exports | `mpsc::{channel, Receiver, Sender}`, `Mutex`, `RwLock` — **not `watch`** |
| Therefore | `src-tauri` needs `tokio = { version = "1", features = ["sync"] }` directly; the module doc says as much |
| `tokio` already in tree | 1.53.1, via `tauri` — a direct dependency adds no transitive weight |
| Clipboard crate | `tauri-plugin-clipboard-manager` 2.3.2, MIT OR Apache-2.0 |
| License note | Taken under **MIT**. Apache-2.0 alone is incompatible with this project's `GPL-2.0-only`; the CI purity job already guards this boundary for TLS backends |
| Panic strategy | No `[profile]` overrides in either manifest, so `panic = "unwind"` is in force and `catch_unwind` works — see §9 |
| `catch_unwind` ergonomics | Requires `UnwindSafe`; `&Index` and `&ConjugationTable` will not satisfy it. Wrap in `AssertUnwindSafe`, which is sound here because the managed state is read-only after startup and no `&mut` crosses the boundary |
| `src-tauri` MSRV | 1.88, pinned separately from the workspace's 1.85 (see 2D) |
| MSRV gate | `cargo +1.85 check -p jparser -p jmdict-source -p xtask` — **not** `--workspace` |
| Baseline tests | 317 passed / 1 ignored; 16 Vitest; 10 Playwright local and CI-simulated |
| Current sizes | `state.rs` 194, `commands.rs` 138, `main.rs` 57, `main.ts` 83, `global.css` 136 |
| 2D contract to migrate | `parse_text(text) -> Result<ParseResult, String>` in `commands.rs`; consumed by `src/main.ts` and `src/main.test.ts` |

## 9. Invariants this phase must not break

- `INDEX_FORMAT_VERSION` stays 3
- `EntryData`'s field order is wire format
- A published `gen-N` is immutable
- Directory knowledge lives only in `generations.rs` and `ensure_dictionary`
- The staging filename stays process-unique; a `.partial` file is never resolved
- `crates/jparser` keeps no Tauri dependency and no I/O beyond index and assets
- `mecab` stays off by default and the purity grep keeps returning 0
- The eight serialized `WordFlags` names are public API — `primary`,
  `pronounce`, `common_line`, `common`, `particle`, `counter`, `top`, `is_name`
- `src-tauri`'s empty-string `StartupFailure` sentinel means "startup succeeded";
  every `StartupError` variant must keep rendering non-empty
- **No profile may set `panic = "abort"`.** §5's panic containment is
  `catch_unwind`, which catches nothing under an aborting profile — the app would
  die on a matcher panic instead of keeping the previous result on screen.
  Nothing fails loudly when this is added; the protection silently disappears.
  There are no `[profile]` overrides today, and Tauri's own release guidance
  recommends `panic = "abort"` for binary size, so this will come up.

## 10. Constraints inherited

- **GPL v2 header** on every new source file, verbatim from `crates/jparser/src/index/mod.rs:1-6`
- **No `unwrap()` / `expect()` / `unreachable!()`** in library or binary code
  outside `#[cfg(test)]`; the `.expect` closing `main` is the one documented
  exception
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **`crates/jparser/src/segment.rs` must not be edited** — 778 of the 800 cap
- **Formatting is per-file `rustfmt --edition 2021 <file>`, never `cargo fmt`** —
  and never on a crate-root file, which cascades into every `mod`-reachable file
  and reformats `conjugation.rs`, `kana.rs`, and `romaji.rs`. `conjugation.rs` is
  deliberately not rustfmt-clean; "fixing" it is a defect
- **Clippy clean** at `cargo clippy --workspace --all-targets -- -D warnings`, and
  at `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`
- **No frontend framework** — vanilla TypeScript and DOM APIs
- **Every colour defined on bare `:root` first**; media queries and `[data-theme]`
  may only redefine
- **Only `transform` and `opacity` animated**, and the reduced-motion block holds
- **Dictionary content reaches the DOM via `textContent`**, never `innerHTML`
- **`ta-old/` is read-only**
