# JParser Phase 2F — First-Run Dictionary Download (Design)

2E made the app autonomous, but only for someone who had already built an index
from a terminal. 2F removes the terminal: a new user opens the app, presses one
button, and has a working dictionary.

**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` is
authoritative for the data assets (§4.3 JMdict, §4.4 Vibrato) and the phasing
(§11). `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` is the
predecessor; 2F changes the startup path it established and §2 says how.

**Predecessor code:** the shipped 2E tree at `144af28`.

## 1. Scope

**In scope:**

- A user-initiated first-run download of JMdict, wired to the existing
  `jmdict_source::resolve`
- A startup outcome that distinguishes "no dictionary yet" from "something is
  broken"
- A parse worker that survives starting without an index and begins working when
  one arrives — no restart
- A download screen with three states: needs-dictionary, working, failed
- Retry on failure, with a manual fallback that costs no new code

**Not in scope:**

- **The Vibrato dictionary.** Port design §4.4 fixes it as on-demand at the MeCab
  toggle, which is Phase 5. Leaving MeCab off must keep costing nothing.
- **An "update dictionary" affordance.** `ensure_dictionary` already rebuilds on
  `VersionMismatch` and `ConjugationMismatch`, so an upgrade repairs itself on the
  next launch. A manual update button is a Phase 3/4 settings concern.
- **A file picker.** §5 explains why the fallback needs none.
- **Sweeping orphaned `.partial` files.** §5 records the gap.

### 1.1 The downloader already exists

`crates/jmdict-source` ships the whole fetch path and was written for this:
`resolve(source_dir)` retries `DOWNLOAD_ATTEMPTS` (3) times with `RETRY_BACKOFF`
(2 s), stages through `<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>`, and hands back a
`BufRead`. `open_local` sniffs gzip magic rather than trusting the filename.
`resolve`'s own doc comment gives the intended call shape.

So 2F builds no networking. It wires an existing library into the app and designs
what the user sees while it runs. The work is in the startup path and the UI.

## 2. Architecture

**`AppState` stops being managed state.** 2E's Task 4 deleted `parse_text`, which
was the only `State<'_, Arc<AppState>>` consumer; `run_worker` is now its sole
reader. So the index reaches the worker through a second `watch` channel instead
of Tauri's managed state:

```rust
let (index_tx, index_rx) = tokio::sync::watch::channel(None::<Arc<AppState>>);
tauri::async_runtime::spawn(parse::run_worker(handle, index_rx, rx));  // ALWAYS
app.manage(commands::IndexSender(index_tx.clone()));
```

The worker is spawned unconditionally. 2E spawned it only on the success branch
and dropped `rx` otherwise — correct then, but it leaves no live channel for a
result to arrive on after a first-run download. Spawning always removes the
special case rather than adding one.

**Correction (final review):** this section originally said `run_poll`'s exit
path was unchanged. It is not — it is now unreachable. That path is
`clipboard::run_poll`'s `if tx.send(text).is_err() { return; }`, which used to
fire whenever `run_worker` had not been spawned (2E's fatal-startup branch
dropped `rx` immediately, so the very first tick's `send` failed and the poll
stopped). Now that the worker is spawned unconditionally and always holds
`rx`, that `send` can never fail, so on a genuinely fatal startup the poll
keeps ticking for the life of the app, feeding a worker that silently
discards everything via its `current_index` guard. Harmless — nothing reads
the discarded input — but it is a real behavior change this section did not
record.

`run_worker` gains exactly one guard: `borrow()` the index before parsing and skip
when it is `None`. `next_input`, `catch_parse`, the `spawn_blocking` call, and the
emit routing are untouched.

### 2.1 Startup gains a third outcome

| `load_state` result | 2E | 2F |
|---|---|---|
| Index opens | `StartupFailure("")` | unchanged, plus `index_tx.send(Some(state))` |
| `StartupError::NoIndex` | `StartupFailure(msg)`, controls disabled | **needs-dictionary** — drives the download screen |
| `Index` / `Conjugation` / `Hints` error | `StartupFailure(msg)` | unchanged, still fatal |

This split is the point of the phase. Today a first-run user gets the *fatal*
treatment — `#text` and `#parse` disabled, and a message naming a CLI command.
2F turns that exact condition into something actionable in the window.

`StartupFailure`'s empty-string sentinel keeps its meaning: startup succeeded.
The needs-dictionary state is carried separately, the way 2E's `SettingsWarning`
is carried separately from `StartupFailure` and for the same reason — one is
fatal and disables controls, the other is not.

### 2.2 The command and the event

`download_dictionary` runs on `spawn_blocking` and mirrors the CLI's shape:

```rust
ensure_dictionary(&root, &table, &opts, keep, || jmdict_source::resolve(&source_dir))
```

with `StemOptions::default()` and `DEFAULT_KEEP_GENERATIONS`, then publishes the
resulting state through `index_tx`. The source directory is
`app_config_dir.join(jmdict_source::SOURCE_DIR)` — a sibling of `dict/`, never
inside it, because a published generation is immutable.

Progress is a `dictionary-status` event carrying a phase label. The index build
measures **under 15 seconds** on the reference machine, so there is nothing to
report but which phase is running; determinate progress would require a callback
parameter on `build_from_reader` and `ensure_dictionary`, and `crates/jparser`
gains nothing this phase.

**Names are frozen.** Command: `download_dictionary`. Event:
`dictionary-status`. Both sides of an IPC boundary hard-code them and a rename
compiles clean on each side while breaking the app.

`download_dictionary` returns `Result<(), String>` and is otherwise
fire-and-forget: the `Err` reports only that the work could not be *started*, and
every outcome after that — including failure — arrives as a `dictionary-status`
event. This is the same split 2E chose for `set_input`, and for the same reason:
one path to the screen, not two.

**A second concurrent call must be rejected, not queued.** Two overlapping
`ensure_dictionary` runs would build two generations against one source
directory. The frontend's `pending` guard is not sufficient — it protects one
button in one webview, not the command. The command holds its own in-flight flag
and returns `Err` immediately if one is already running.

### 2.3 New dependency

`src-tauri` gains `jmdict-source`, and through it `ureq` and `flate2` — the first
network code in the shell. The purity gate guards `crates/jparser`, not the shell,
so this is permitted; it is stated here so it is a decision in the spec rather
than a surprise in a diff.

**Addendum (final review):** this is also the first time that connection is
opened by an end user's click rather than a developer running a CLI. Its
properties, already recorded for the CLI path in the Phase 2B spec, are worth
repeating here for that reason: the fetch is plain HTTP
(`http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz` — `https://` fails certificate
validation with a subject-name mismatch), integrity is checked only via
gzip's own CRC32 trailer, and there is no authenticity guarantee at all, so an
on-path substitution is indistinguishable from a real response until the
archive is decoded.

## 3. States

| State | Content |
|---|---|
| Needs dictionary | That it fetches JMdict from EDRDG, that it is a one-time download of roughly ten megabytes, and a **Download dictionary** button |
| Working | Phase label (`Downloading…` → `Building index…`) and a spinner |
| Failed | The reason, the source directory path, a **Retry** button |

**Correction (final review):** this originally said the screen occupies
`output`, the node `showStartupError` already uses, because it is the app's
entire content until an index exists. The shipped code instead gives it its
own `<div id="dictionary">`, a fixed sibling of `output` inside `.panes` — a
deliberate controller ruling, not a drift from the plan. `output` is what a
successful parse replaces wholesale (the same hazard `#parse-error`'s own
comment documents for a different node), so a screen living there would be
silently wiped by the very first parse it enables, or would pile up on repeat
failures because nothing removes the previous node. A fixed slot has neither
problem, and `output` is empty for the whole time this screen is showing
regardless, so the intent — this screen is the app's entire visible content
until an index exists — is unaffected. On success it simply clears; no
restart, no modal, nothing to dismiss.

**The spinner inherits 2D's motion rules:** `transform` and `opacity` only, so it
is a rotation, and the `prefers-reduced-motion` block must replace it with a
static phase label rather than leaving a frozen partial rotation on screen.

**The button uses 2E's `pending` guard, not `disabled`.** 2E established that
disabling a focused element blurs it and drops it from the tab order with no
restoration; a keyboard user activating Download would lose their place for the
duration. A closure-local `pending` boolean gives the same double-submit
protection with no DOM mutation.

## 4. Data flow

```
[no index at startup]  →  needs-dictionary screen
        │ user presses Download
        ▼
download_dictionary (spawn_blocking)
        │  emit dictionary-status "downloading"
        │  jmdict_source::resolve  →  cached file, or fetch with 3 retries
        │  emit dictionary-status "building"
        │  ensure_dictionary  →  Index
        ▼
index_tx.send(Some(Arc<AppState>))   →  worker starts parsing
        │  emit dictionary-status "ready"
        ▼
screen clears; clipboard monitoring already running takes effect immediately
```

The clipboard poll runs throughout — it is gated on the settings flag, not on the
index — so text copied during the download is simply skipped by the worker's
`None` guard. The first copy after `ready` parses normally.

## 5. Error handling

| Boundary | Policy |
|---|---|
| **Download fails after 3 attempts** | Failed state: the reason, the source directory path, a Retry button |
| **Build fails** (corrupt or truncated archive) | Same surface. `resolve` finds the cached archive on retry, so a retry after a build failure is fast and offline |
| **Disk full / permission denied** | Same surface; the io error's message is the reason |
| **Fatal startup errors** | Unchanged from 2E — still `StartupFailure`, still disables the controls |

The failure message names the path, which is the whole manual fallback:

```
Could not download the dictionary: <reason>.
Retry, or place JMdict_e.gz in <source dir> and retry.
```

`resolve` checks for the target file before fetching and `open_local` accepts the
archive compressed or already decompressed, so a user who drops the file in and
presses Retry is served from disk with no network call. **This needs no file
picker**, and therefore no `tauri-plugin-dialog` and no new capability
permission — which matters, because 2E's Critical defect was a missing capability
that failed silently behind a fully green test suite. A printed path buys the same
outcome at none of that risk.

**No new capability is required.** `dictionary-status` is an event, and
`core:event:allow-listen` is already granted in `src-tauri/capabilities/default.json`.
Anyone adding a *plugin* command here must grant it explicitly; that file exists
and this is why.

**Known gap, recorded rather than fixed:** a crash mid-download orphans a
`<SOURCE_FILE>.partial.<pid>` file. It is never resolved as a dictionary — an
existing invariant — so it is harmless but accumulates. Sweeping belongs with
whatever does housekeeping later.

## 6. Testing

Following §7.1 of the 2E spec: test the seams, not the I/O.

- **The startup split** — `NoIndex` yields needs-dictionary; every other
  `StartupError` stays fatal. Pure, unit-testable in `state.rs`, and it must be
  exhaustive over the variants so a new one forces a decision.
- **The worker's `None` guard** — that input arriving before an index exists is
  skipped rather than panicking or emitting an error. Tests at the same seam
  `next_input` does.
- **Vitest** covers the three screen states with `listen` mocked, including that a
  failed download leaves a Retry that works.
- **Playwright** stubs `dictionary-status` exactly as it already stubs
  `parse-result`, and the committed baselines gain the download screen.

**Deliberately untested**, on the same reasoning that excludes the clipboard and
always-on-top: the real network fetch — `jmdict-source` already tests
`fetch_with_retry` against a local listener — and the real index build, which is
slow and needs a ~10 MB asset.

**A manual first run on a clean profile is required, not optional.** Delete the
config directory, launch, press Download, and confirm the dictionary arrives and
parsing begins without a restart. 2E's Critical defect — the entire event
architecture dead behind 340 green Rust tests — was found only because manual
verification was attempted honestly. This phase's equivalent blind spot is the
same seam.

## 7. Invariants this phase must not break

- `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field order is wire format
- A published `gen-N` is immutable; directory knowledge lives only in
  `generations.rs` and `ensure_dictionary`
- The staging filename stays process-unique; a `.partial` file is never resolved
- `crates/jparser` keeps no Tauri dependency and gains nothing this phase
- `mecab` stays off by default and the purity grep keeps returning 0
- The eight serialized `WordFlags` names are public API
- `StartupFailure`'s empty string means "startup succeeded"; every
  `StartupError` variant must keep rendering non-empty
- No profile may set `panic = "abort"` — `catch_parse`'s containment depends on
  unwinding

## 8. Constraints inherited

- **GPL v2 header** on every new source file, verbatim from
  `crates/jparser/src/index/mod.rs:1-6`
- **No `unwrap()` / `expect()` / `unreachable!()`** outside `#[cfg(test)]`; the
  `.expect` closing `main` is the one documented exception. Never swallow an error
  without a comment naming the reason
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **`crates/jparser/src/segment.rs` must not be edited** — 778 of the 800 cap
- **Per-file `rustfmt --edition 2021`, never `cargo fmt`**
- **Clippy clean** at `cargo clippy --workspace --all-targets -- -D warnings` and
  at `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`
- **MSRV gate** is `cargo +1.85 check -p jparser -p jmdict-source -p xtask` —
  never `--workspace`
- **No frontend framework**; every colour on bare `:root` first; only `transform`
  and `opacity` animated; dictionary content via `textContent`, never `innerHTML`
- **`ta-old/` is read-only**
