# JParser Phase 2D — Tauri Shell and Parse Panes (Design)

The first user-facing surface of the port. Phase 1 built a parser with 292 tests
and no window; 2A–2C built the index pipeline, the fetch pipeline, and boundary
hints. Nothing so far can be looked at. 2D puts parsed Japanese on screen.

**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` is
authoritative for architecture (§3), the app shell's module split (§6), and the
UI direction (§7). This spec narrows those to what Phase 2D builds and records
the decisions that document does not fix.

**Predecessor:** `docs/superpowers/2026-08-14-jparser-phase2c-handoff.md` for the
`hints` surface, and the invariants 2D must not break.

## 1. Scope

The port design's Phase 2 bullet — "Tauri shell, first-run download, clipboard
monitor, sentence + definition panes, light/dark, always-on-top" — is five
subsystems. Each of 2A, 2B, and 2C was one crate feature. 2D takes the slice that
gets pixels on screen; 2E takes the slice that makes the app autonomous.

**In scope:**

- The Tauri shell: one window, native decorations, managed state
- A manual text input — the only parse trigger this phase
- `parse_text` over IPC
- The sentence pane and the definition pane
- Light/dark theming per port design §7.3
- `Serialize` on `jparser`'s result types, which is what makes the IPC contract
  possible at all
- MeCab hints when `TA_HINTS_DICT` names a dictionary

**Deferred to 2E:** clipboard monitor, first-run download, settings persistence,
always-on-top.

**Deferred to Phase 3/4:** furigana modes, gloss filters, the settings popover,
the three chrome states, per-pixel transparency, layout hotkeys, parse history.

**Window geometry.** Default 720×480, minimum 480×320. Resizable; geometry is not
persisted — that is settings work, and settings are 2E. The minimum is what the
definition pane needs before its rows start wrapping incoherently, and it is the
lower bound the visual tests in §7 exercise.

**Native window decorations this phase.** Port design §7.4's default chrome state
is `decorations: false` with our own 32px header acting as titlebar. Adopting that
now would mean building a custom titlebar, drag regions, and invisible resize
edges — Phase 4 work, pulled forward to serve nothing 2D needs. A native frame
also sidesteps §7.4's note that a native titlebar over a transparent body is
broken on macOS, which is a Phase 4 constraint we do not have to reason about yet.

## 2. Why `jparser` gains `Serialize`

`ParseResult`, `Segment`, and `Entry` derive nothing today. The `index` module's
types do (`index/mod.rs:46,62,78,87`) because the on-disk index format needs them,
which means `serde` is already a non-optional dependency (`crates/jparser/Cargo.toml:26`)
and `Sense` — a re-export of `index::SenseData` — is already serializable.

So the change is four derives and one hand-written impl. It adds no dependency.

**It does not violate the §3 hard rule.** That rule says `crates/jparser` has no
Tauri dependency and no I/O beyond its index and asset files. `serde` is neither.

**Not behind a feature gate.** Phase 2C gated `mecab` because the gate removed a
real dependency (`vibrato`) from the default build, and a purity grep proves it.
A `serde` feature would remove nothing — `serde` is already unconditional — so the
gate would be a knob with no effect on the dependency tree. Consistency with 2C's
*reasoning* means not gating here, even though it means diverging from its *shape*.

### 2.1 `WordFlags` on the wire

`WordFlags` is `pub struct WordFlags(pub u16)` (`record.rs:30`) — a bitfield whose
constants live in Rust. The UI needs flag *meaning*: port design §7.2 colors
sentence chips "by content class (kanji-bearing / kana-only / particle) from
semantic tokens."

A derived `Serialize` would emit `9`. The frontend would then re-declare
`PRIMARY = 0x0001`, `PARTICLE = 0x0010` and so on in TypeScript — constants
duplicated across a language boundary with no compiler relating them. Renaming or
renumbering a flag in Rust would silently change what the UI paints.

**`WordFlags` gets a hand-written `Serialize` emitting an array of names:**

```json
{ "headword": "言う", "conjugation": "Negative Formal Past",
  "flags": ["primary", "common"] }
```

The frontend reads `flags.includes("particle")`. `jparser` stays the single source
of truth for what the bits mean.

### 2.2 The names are now public API

The moment the webview reads `"particle"`, that string is a compatibility surface.
Renaming `WordFlags::PARTICLE` would compile clean, pass every existing test, and
silently stop the sentence pane colouring particles — no error anywhere.

**A test pins the serialized names as literal strings.** A rename then fails a test
instead of shipping. This is the cheapest possible guard on the one thing in this
phase that can break invisibly.

## 3. Architecture

```
src-tauri/src/
  main.rs       setup: resolve dict root, open index, manage state
  state.rs      App { index, table, hints: Option<VibratoTokenizer> }
  commands.rs   parse_text
src/
  main.ts                input handler → invoke → render
  render/sentence.ts     ParseResult → chip DOM
  render/definitions.ts  ParseResult → rows
  styles/{tokens,typography,global}.css
```

Port design §6 lists five shell modules: `clipboard.rs`, `parse.rs`,
`commands.rs`, `settings.rs`, `window.rs`. **Of those five, 2D creates only
`commands.rs`** — `main.rs` and `state.rs` above are shell plumbing §6 does not
enumerate, not deferred modules arriving early. `clipboard.rs` and `settings.rs`
belong to 2E; `window.rs` to Phase 4; `parse.rs` is discussed in §4. Empty modules
are scaffolding for later, and later can scaffold for itself.

### 3.1 Startup

Resolve `<app_config_dir>/dict/`, call `index::generations::latest(root)`
(`generations.rs:123`, returning `Result<Option<PathBuf>, IndexError>`), and on
`Some(path)` call `Index::open`. `Index::open` memory-maps, so this is cheap and
happens once, off the render path. `ConjugationTable::load_embedded()` alongside it.

`latest` is the right seam because it answers "is there an index?" without needing
a source fetcher. 2E replaces this call with `ensure_dictionary`
(`index/mod.rs:146`), which takes a `FnOnce() -> io::Result<impl BufRead>` — the
signature 2B shaped specifically to accept `jmdict_source::resolve` with no mapping.

## 4. Data flow

```
text input → invoke("parse_text") → spawn_blocking(jparser::parse) → JSON → render()
```

One direction, no events, no worker.

**Why not the worker.** Port design §6 specifies `parse.rs` as "single worker,
latest-wins", justified by ta-old's note that local algorithms "stop and rerun
rather than queue, which matters when game text advances faster than parsing."
That is a clipboard concern. With manual input as the only producer, there is never
more than one parse in flight, so latest-wins is unobservable — and therefore
untestable. Building it now means shipping unverified logic to satisfy a diagram.

2E adds the worker and the `parse:result` event when the clipboard makes
supersession real. The migration cost is one frontend file: `await invoke(...)`
becomes a `listen(...)` subscription.

**`spawn_blocking` is not optional.** `jparser::parse` is synchronous CPU work over
the whole input; running it on the async runtime's thread would stall the webview.

### 4.1 Hints

When `TA_HINTS_DICT` is set, `VibratoTokenizer::load` runs once at startup and the
tokenizer lives in managed state. Each parse derives fresh `BoundaryFlags` via
`hints(text)` and passes them through. When unset, `parse` receives `None`.

This is the first time the derivation meets real Japanese. Phase 2C proved it
changes the DP's output on a constructed fixture, and its handoff is explicit that
this is *not* evidence of real-world accuracy — the port design (`:649`) says MeCab
lands late precisely because it "cannot be validated until the DP it nudges is
known-good." 2D is where that validation becomes possible.

An env var rather than a setting because settings persistence is 2E. A bad path is
**fatal at startup**, matching the CLI rule from 2C: a user who asked for hints and
silently did not get them receives a plausible result that is not what they asked for.

## 5. Error handling

Three states the UI must keep distinct. Collapsing them is how a working first run
reads as breakage.

| State | Cause | Presentation |
|---|---|---|
| **No index** | `generations::latest` → `Ok(None)` | Empty state naming the exact `jparser-cli build-index` invocation. Not an error — this is the expected condition until 2E. |
| **Index unopenable** | `IndexError` from `latest` or `Index::open` | Message surfaced; app stays up rather than failing to launch. |
| **Parse failed** | `ParseError` through the command's `Err` | Shown inline; the previous result stays on screen rather than blanking. |

`ParseError` today has one variant, `Index(IndexError)` (`lib.rs:113`). The
frontend must not match on variants — it renders the message.

## 6. UI

Port design §7.1's direction is unchanged: **Swiss / technical**, because the
content *is* typography. System CJK stack (Hiragino Sans / Yu Gothic UI), system UI
for glosses, system mono for tags. Zero webfonts — no download, no FOUT, and
correct per-platform glyphs a webfont would actively get wrong. Motion on
`transform` and `opacity` only, behind `prefers-reduced-motion`.

**Sentence pane.** Segmented text as inline word chips, coloured by content class
derived from the §2.1 flag names. Unmatched runs render muted and unchipped, so
coverage gaps stay visible rather than disguised — this matters more in 2D than
later, because seeing where the parser fails is the point of having a window.
Chip click scrolls to and marks its definition row.

No furigana this phase. §7.2's `<ruby>` kana modes and the romaji-second-line rule
are Phase 3.

**Definition pane.** One row per span, in sentence order:

```
言われた      [24px CJK]        Negative Formal Past   [mono tag]
いわれた      [11px muted]
              1. to be said; to be called
              2. …
```

Alternative entries for the same span nest under the row, collapsed past the first
— the payoff from the segmenter's backtrack pass, and the first visible evidence
that `Segment::entries` is ranked rather than arbitrary.

**Theming** is §7.3 verbatim. Tokens in `styles/tokens.css`; three states
(system / light / dark); manual override wins in both directions. No colour may
have its only definition inside a media query. The webview follows the OS, so the
system case needs no Rust at all — 2D stores no theme override, because storing it
is settings persistence and that is 2E.

## 7. Testing

The repository's standing web-testing rules target responsive websites — four
breakpoints, Lighthouse, three browsers. A Tauri app is one window in one platform
webview. The rules are adapted to the surface, and the drops are recorded here
rather than left as silent omissions.

**Rust — `src-tauri`.** Unit and integration tests for the `parse_text` command,
the flag-name mapping of §2.2, and each of §5's three error paths. The 80% coverage
target applies here.

**TypeScript — `render()`.** The render layer is a pure function from `ParseResult`
to DOM. Tested directly against fixture results, asserting structure and classes.

**Visual — Playwright against `vite dev` with a stubbed `invoke`.** Two window
sizes (compact 480×320, default 720×480) across both themes.

### 7.1 Why not WebDriver against the built app

`tauri-driver` supports Windows and Linux only: macOS has no WKWebView driver tool.
A macOS path does exist — `@wdio/tauri-service` runs an embedded WebDriver server
inside the app — but taking it means adopting WebdriverIO as a second test
framework beside Playwright, plus a Tauri plugin, and rebuilding the app for every
visual run.

Stubbing `invoke` needs none of that, runs in ordinary Chromium on any platform,
and the fixtures double as the corpus for the DOM tests above.

**The gap this leaves, stated plainly:** nothing in 2D exercises the real
Rust↔webview seam end to end. The `src-tauri` tests cover it from the Rust side and
the DOM tests cover the shape the frontend expects, but no test proves a real
`ParseResult` survives a real `invoke`. If 2E's clipboard work makes an end-to-end
harness worth its cost, that is the phase to add it.

**Dropped as inapplicable:** Lighthouse (no navigation, no network, no SEO), the
320px breakpoint (below any sane window minimum), and cross-browser (the targets
are WKWebView and WebView2, neither of which Playwright drives).

## 8. Resolved facts

Measured against the tree at commit `641c789`.

| Fact | Value |
|---|---|
| `jparser` serde | `serde = { version = "1", features = ["derive"] }`, non-optional |
| Already serializable | `index::{EntryData, SenseData, …}`; `Sense` is a re-export of `SenseData` |
| Not serializable | `ParseResult`, `Segment`, `Entry`, `WordFlags` |
| `WordFlags` repr | `pub struct WordFlags(pub u16)`, 8 constants, `record.rs:30-52` |
| Newest-generation lookup | `index::generations::latest(&Path) -> Result<Option<PathBuf>, IndexError>` |
| Build-from-source path | `index::ensure_dictionary(root, table, opts, keep, source)` — 2E's seam |
| Hints surface | `VibratoTokenizer::load(&Path)`, `.hints(&str) -> BoundaryFlags` |
| `parse` signature | `parse(&Index, &ConjugationTable, &str, &ParseOptions, Option<&dyn BoundaryHints>)` |
| Workspace MSRV | 1.85 |
| Baseline tests | 292 passed / 1 ignored; 271 with `--features mecab` |

## 9. Invariants this phase must not break

Carried from the 2B and 2C handoffs. 2D touches none of them, which is worth
stating because it adds the first consumer outside the CLI:

- `INDEX_FORMAT_VERSION` stays 3
- `EntryData`'s field order is wire format
- A published `gen-N` is immutable
- Directory knowledge lives only in `generations.rs` and `ensure_dictionary`
- The staging filename stays process-unique; a `.partial` file is never resolved
- `crates/jparser` keeps no Tauri dependency and no I/O beyond index and assets
- `mecab` stays off by default and the purity grep keeps returning 0

Adding `Serialize` to result types is not an index-format change: `EntryData`'s
on-disk encoding is untouched, and `ParseResult` has never been persisted.

## 10. Constraints inherited

- **GPL v2 header** on every new source file, verbatim from `crates/jparser/src/index/mod.rs:1-6`
- **No `unwrap()` / `expect()` / `unreachable!()`** in library or binary code outside `#[cfg(test)]`
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **`crates/jparser/src/segment.rs` must not be edited** — 778 of the 800 cap
- **Formatting is per-file `rustfmt --edition 2021 <file>`, never `cargo fmt`** —
  `conjugation.rs` is deliberately not rustfmt-clean and "fixing" it is a defect
- **Clippy clean** at `cargo clippy --workspace --all-targets -- -D warnings`, and
  at `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`
- **`ta-old/` is read-only**
