# JParser Phase 5B — Design

**Date:** 2026-08-27
**Master spec:** [`2026-08-12-jparser-port-design.md`](2026-08-12-jparser-port-design.md) §11 Phase 5
**Predecessor:** [`2026-08-27-jparser-phase5a-design.md`](2026-08-27-jparser-phase5a-design.md),
[`../2026-08-27-jparser-phase5a-handoff.md`](../2026-08-27-jparser-phase5a-handoff.md)
**Status:** Design approved, not implemented

---

## 1. Goal

Make MeCab boundary hints reachable without a terminal. 5A shipped acquisition
as a library; 2C shipped the tokenizer behind the `mecab` feature and the
`TA_HINTS_DICT` env override. Nothing connects them: a user who wants better
word splitting must today set an environment variable pointing at a file they
unpacked by hand.

5B is the plumbing between those two halves — a persisted toggle, acquisition
driven by that toggle, and the resulting tokenizer reaching the running parse
worker without a restart.

## 2. Scope

**In:**

- A persisted `mecab_hints` setting and a settings-window control for it.
- `set_mecab_hints`, which acquires the dictionary on demand and publishes it
  to the live worker.
- A second `watch` channel carrying the tokenizer, replacing `AppState.hints`.
- Startup resolution: an absent dictionary means hints off, not a fatal.
- A hand-placed-archive route, accepting **any** Vibrato dictionary rather than
  only the pinned one (§3).

**Out:**

- Detecting a non-IPADIC dictionary (§12).
- Acquisition at startup. The toggle is the only acquisition gesture (§5).
- A percentage progress bar (§6.1).
- Layout slots capturing `mecab_hints`. §7.9's slot is the shape of the window;
  hints are a parsing setting with a network fetch behind them.

---

## 3. Acquisition

### 3.1 One version-blind slot

5A published into `<hints>/ipadic-mecab-2_7_0/`, a directory named for the
pinned release, on the reasoning that a later release would land beside it.
Nothing ever reads a second one, and this phase accepts dictionaries the pin
does not describe, so a name derived from the pin would be a lie about the
contents.

`DICT_DIR` therefore becomes `current`, and there is exactly one slot:

```
<app_config_dir>/hints/
  current/
    COPYING
    NOTICE
    system.dic
  <anything>.tar.xz            # optional, hand-placed by the user
  .current.partial.<pid>-<n>/  # staging, never resolved
```

`<app_config_dir>/hints` comes from a new `vibrato_source::HINTS_DIR = "hints"`,
mirroring `jmdict_source::SOURCE_DIR`: the crate names its own conventional
subdirectory without deriving an app layout it does not own.

`published(hints_dir)` is unchanged in shape — one `is_file` on
`<hints>/current/system.dic`.

### 3.2 Newest wins

`resolve(hints_dir)` becomes one rule, in this order:

1. **Hand-placed archive.** `hand_placed(hints_dir)` returns the file directly
   in `<hints>/` with the newest mtime among those whose name ends `.tar.xz`,
   skipping anything starting with `.`. Staging archives are named
   `.current.tar.xz.partial.<pid>-<n>` and are excluded twice over — by the
   leading dot and by not ending in the suffix. A `read_dir` that fails, on a
   directory that does not exist yet, is `None` rather than an error: first run
   is not a failure.
2. If that archive's mtime is newer than `<hints>/current/system.dic`'s — or the
   slot does not exist — unpack it and publish onto the slot. **No hash check.**
3. Otherwise, if the slot exists, use it.
4. Otherwise fetch `ASSET_URL`, verify `ASSET_BYTES` and `ASSET_SHA256`, and
   publish onto the slot.

Two archives present is not an error. The newest wins, which is the same rule
step 2 applies against the slot, so there is one sentence to explain and no new
error variant. An unreadable mtime on either side is treated as "not newer":
keeping a dictionary that works beats replacing it on a failed `stat`.

This is also how a user **replaces** a dictionary: drop the new archive in,
toggle off and on. No directory to delete first.

### 3.3 Why the hand-placed archive is not hash-verified

`ASSET_SHA256` exists to prove that *a network response* is the artifact this
design chose — it defends against a repointed URL, a truncating proxy, and a
tampered mirror. A file the user placed in their own config directory is already
their choice; hashing it could only refuse the thing they explicitly asked for.
`jmdict-source` has the same asymmetry for the same reason, and its hand-placed
file has no hash at all.

The download branch keeps both the size and hash checks unchanged.

### 3.4 The extractor needs no change

`extract::unpack_into`'s doc comment already anticipated this phase:

> the archive is pinned and hashed today, but the extractor's safety must not
> depend on a constant somewhere else in the crate, or a later phase that
> repoints the pin or adds a hand-placed archive inherits a hole rather than a
> guard.

So the three guards that matter against an untrusted file already hold: the
basename whitelist (`COPYING`, `NOTICE`, `system.dic.zst`, with nothing derived
from an entry's own path ever reaching the filesystem), both output caps (12 MiB
tar, 64 MiB dictionary), and `SourceError::MissingMember` when the archive
carries no `system.dic.zst`. 5A's traversal fixture — a `..`-prefixed member
with a whitelisted basename — is exactly the malicious case this route makes
reachable, and it already passes.

### 3.5 Replacing a published slot

`publish` treats "the target already holds a `system.dic`" as a benign race and
discards the staging directory. That is right for two processes fetching the
same pinned artifact and wrong for a deliberate replacement, so the archive
branch uses a second entry point:

```rust
fn publish_replacing(staging: &Path, target: &Path) -> Result<(), SourceError>
```

Rename the existing target aside to a backup name derived from `staging` — which
already carries the PID and `STAGING_SEQ` counter, so the backup is unique to
this call for the same reason the staging directory is — rename staging onto the
target, then remove the backup. A failure at the second rename renames the
backup back, so the user is never left with no dictionary. All three paths stay
inside `hints_dir`, so no rename crosses a filesystem.

---

## 4. State and transport

### 4.1 The channel split

`AppState.hints` is deleted. `AppState` keeps `index` and `table` and remains
what `state.rs`'s module doc claims it is: built once at launch, then read-only.
That claim is load-bearing — `parse::catch_parse`'s `AssertUnwindSafe` soundness
argument rests on it — and interior mutability would have required rewriting
both.

A second channel is created in `setup` beside the index one:

```rust
pub struct HintsSender(pub watch::Sender<Option<Arc<VibratoTokenizer>>>);
```

`parse::run_worker` gains a third parameter and a `current_hints(&rx)` mirroring
`current_index`, with the same shape: the `watch` borrow is released before the
value is returned, so no guard is ever held across an `.await`. The parse
closure reads its flags off the channel value rather than off `state`.

Rebuilding the whole `AppState` and republishing over the index channel was the
alternative. It reuses 2F's path verbatim but reopens the index on every toggle
and gives a mid-rebuild failure a way to leave the worker holding no index at
all.

### 4.2 What this deletes

`commands::download_dictionary` carries a copy of `load_state`'s env-var hints
branch, because a first-run download builds a fresh `AppState` and had to
re-attach hints to it. With hints on their own channel it is dead: the branch
goes, and 2F's download stops knowing about MeCab.

---

## 5. Startup

Hints load in a new function, called independently of `load_state`:

```rust
pub fn load_hints(hints_dir: &Path, enabled: bool)
    -> Result<(Option<VibratoTokenizer>, String), StartupError>
```

Deliberately **not** inside `load_state`. That function returns `Err(NoIndex)`
on first run and never reaches its own tail, so hints folded into it would go
unloaded on exactly the run that downloads JMdict, and stay unloaded until a
restart.

| Input | Result |
| --- | --- |
| `TA_HINTS_DICT` set | `VibratoTokenizer::load(env)?` — `StartupError::Hints`, still fatal |
| env unset, setting off | `(None, "")` |
| env unset, setting on, `published()` loads | `(Some(t), "")` |
| env unset, setting on, `published()` is `None` | `(None, warning)` |
| env unset, setting on, published but unloadable | `(None, warning)` |

The env var keeps its fatal because it keeps its meaning: an explicit request,
made outside the app, with no UI to recover in. 2C's reasoning — "a user who
asked for hints and silently did not get them receives a plausible result that
is not what they asked for" — still describes the env var exactly. It does not
describe a persisted setting, which has a control the user can see and a warning
surface to explain itself, which is what master spec §11's 5B row means by
"absent dictionary means hints off, not a fatal".

**Startup never acquires.** It calls `published()`, never `resolve()` — no
network, and no unpack of a hand-placed archive either. An unpack is a second of
work with no gesture behind it and its own ways to fail. The consequence is
worth stating: an archive dropped in while the app is closed is not installed by
the next launch. The toggle installs it, and the settings note says so.

### 5.1 The warning

A new managed `HintsWarning(String)`, empty-means-nothing, on `ShortcutWarning`'s
stated precedent — one managed string per warning kind, one command each,
because the messages are independent and any of them can be present without the
others.

It exists because §5's rule creates a visible mismatch: the setting stays on
while hints are off, so the checkbox reads on and the parse has no hints. The
warning names that state and the gesture that fixes it — toggling off and on.

`setup` calls `load_hints` first. Its `Err` short-circuits to `StartupFailure` +
`NeedsDictionary(false)`, which is the same fatal arm today's `load_state` hints
error already lands in, so no behaviour moves. `Ok` manages the warning, seeds
the hints channel, and falls through to the existing three-arm `load_state`
match untouched.

---

## 6. The command

```rust
#[tauri::command]
pub async fn set_mecab_hints(enabled: bool, ...) -> Result<(), String>
```

Persistence goes through `SettingsState::update`, exactly as
`set_clipboard_monitoring` does.

**Off:** send `None`, persist `false`, re-parse, emit `off`. No fallible work,
so it is not spawned.

**On:** claim a `HintsInFlight` slot — reusing `claim_download` and
`release_download` verbatim, which are already separated from their command for
exactly this kind of reuse — then `spawn_blocking`:

1. emit `downloading` or `unpacking` on `HINTS_STATUS_EVENT`
2. `vibrato_source::resolve(&hints_dir)?`
3. `VibratoTokenizer::load(&path)?`

`resolve` returns a path, not which branch produced it, and by the time it
returns the label is no longer useful. So the command picks the label itself
before calling it, with a `hand_placed(&hints_dir).is_some()` check — which is
why that function is public rather than private to `resolve`. It is one
`read_dir`, and being wrong costs a mislabelled phase, not a wrong outcome: the
window between the check and `resolve`'s own call is not worth a second return
channel.

`HINTS_STATUS_EVENT` is a `parse.rs`-style const beside
`DICTIONARY_STATUS_EVENT`, not a bare literal at each emit site.

On success: publish to the hints channel, persist `true`, re-parse, emit
`ready`. On failure: emit the message and **persist nothing**, so the setting
stays off and the next launch does not claim hints it does not have.

`Err` is returned only when the work could not be *started* — the same split
`download_dictionary` and `set_input` already use, so there is one path to the
screen.

### 6.1 Two phase labels, no percentage

`downloading` for the network branch, `unpacking` for the archive branch. A
hand-placed archive skips the download entirely, and labelling a one-second
local unpack as a download points the user at the wrong thing when it stalls.

No percentage. 5A §10 deferred this decision to 5B; 5A also measured the whole
first fetch at **2.643 s** on Windows against the real asset, download included.
A progress bar for 2.6 s of work needs a callback seam threaded through `ureq`'s
reader and both decompressors, and buys nothing at that duration.

### 6.2 Re-parsing what is on screen

Enabling hints changes the segmentation of text already displayed. Without a
re-parse the toggle appears inert until the next clipboard copy, which for a
segmentation-quality switch is the entire feedback the user gets.

```rust
fn reparse(tx: &watch::Sender<String>)
```

Reads the input channel's current value, skips it if empty (nothing is on screen
yet), and sends it back. `watch::send` marks the slot changed even for an equal
value, so the worker wakes and re-parses. `run_worker`'s loop is untouched —
`next_input`'s three tests keep pinning the same code.

Making the worker `select!` over both channels was the alternative. It puts the
rule where any future publisher inherits it, at the cost of rewriting the loop
those tests cover and making the worker retain the last input itself.

### 6.3 Load failures are now a user state

`VibratoTokenizer::load` failing used to mean a corrupt download or a bad
`TA_HINTS_DICT`. With any dictionary accepted it also means a format mismatch —
this tree pins `vibrato` 0.5.2, which reads v0.5.0-format dictionaries. The
message says so rather than reading as a corrupt archive.

---

## 7. Surfaces

### 7.1 The settings row

A new `Segmentation` group in the settings window, above `Window & Behavior`:

```
[ ] Boundary hints (MeCab)
    Improves word splitting. First enable downloads an 8 MB IPADIC
    dictionary, or drop a .tar.xz in <hints dir> and it is used instead.
    <status line>
```

The checkbox is never the source of truth. `change` disables it and invokes; the
`hints-status` event decides what it ends up showing:

| Event | Checkbox | Status line |
| --- | --- | --- |
| `downloading` / `unpacking` | disabled, unchanged | phase text |
| `ready` | checked, enabled | cleared |
| `off` | unchecked, enabled | cleared |
| anything else | unchecked, enabled | the message |

`commands::merge_saved` is a whitelist, so `save_settings` cannot touch
`mecab_hints`: the settings window never sends it, `currentSnapshot()` never
carries it, and the backend owns it outright. Worth stating because the
alternative — a snapshot written by an unrelated font-box change clobbering an
in-flight toggle — is silent when it happens.

The note names the hints directory literally, so the manual route is
discoverable without documentation. That path comes from a new `hints_dir()`
command; the webview cannot resolve the app config dir itself.

### 7.2 The warning, in both windows

One `hints_warning()` command reading the managed `HintsWarning`, called from two
places:

- `main.ts` gains `showHintsWarning()`, a three-line copy of
  `showShortcutWarning` — `append`, not `replaceChildren`, for the reason that
  function already documents, since a corrupt `settings.json` and a taken hotkey
  can both be in `#parse-error` at the same time. The justification is the same
  too: hints silently off is invisible otherwise.
- The settings window seeds its status line from the same command on load, so a
  user who opens settings to fix it sees the explanation next to the control.

One source, two readers, no duplicated logic.

---

## 8. Dependencies and gating

`src-tauri/Cargo.toml` gains `vibrato-source` **directly**, the way it already
carries `jmdict-source`. `jparser`'s `hints-source` feature stays what 5A made it
— the CLI driver's gate — and is not touched.

`ci.yml`'s two gates are unaffected and need no edit: the purity grep asserts
`jparser --no-default-features` pulls in neither source crate, and the TLS grep
asserts no `openssl v`/`openssl-sys`/`native-tls`/`ring v`/`aws-lc` anywhere in
`cargo tree --workspace`. `src-tauri` already appeared in that workspace tree; it
now reaches `rustls` + `rustls-graviola` through `vibrato-source`, which is the
sanctioned provider the gate's comment already names.

The `hints` job 5A's fix wave added keeps compiling `jparser --features
hints-source`. Nothing new is needed there.

---

## 9. Task order

0. **Baseline.** Capture `cargo test --workspace`, `npx vitest run`, `npx
   playwright test`, and the clippy warning count before touching anything, the
   way 5A's Task 0 did.
1. **`vibrato-source`.** `HINTS_DIR`; `DICT_DIR` to `current` with its doc
   rewritten; `hand_placed`; `publish_replacing`; the four-step `resolve_from`;
   and `SourceError::MissingMember` gaining the archive's path, which only
   matters once more than one archive can be present.
   Tests: a newer archive beats the slot, an older one loses, dotfiles and
   `.partial.` names are ignored, an unhashed archive publishes, a replacement
   overwrites, `MissingMember` still fires and names its archive. 5A's
   zero-connection `resolve_from` test gains an archive-branch assertion.
2. **`jparser`.** `#[doc(hidden)] pub fn write_test_dictionary(path)` behind
   `mecab`, writing the two-entry IPADIC dictionary `hints.rs`'s own tests
   already build in memory via `vibrato::SystemDictionaryBuilder::from_readers`.
   This is what lets `src-tauri` exercise the real `VibratoTokenizer::load`
   without a 46 MB file or a network call.
3. **`src-tauri` state.** `Settings::mecab_hints`; `HintsSender`;
   `HintsWarning`; `load_hints`; `AppState.hints` removed; `run_worker`'s third
   parameter and `current_hints`; `download_dictionary`'s hints branch deleted;
   `main.rs` wiring.
4. **`src-tauri` commands.** `set_mecab_hints`, `reparse`, `hints_warning`,
   `hints_dir`, and `HintsInFlight`.
5. **Frontend.** The settings row and its lifecycle; `showHintsWarning`; vitest
   for both.
6. **Manual verification and docs.** §10.4, then the master-spec edits of §13.

---

## 10. Verification

### 10.1 Unit — `vibrato-source`

Every case in §9 Task 1 drives real files in a scratch directory and never the
network. Archive fixtures are built in-process the way 5A's already are.

### 10.2 Unit — `src-tauri`

- `load_hints`' five branches. Task 2's helper covers the two that need a real
  dictionary; a junk file written to the slot path makes `published()` say yes
  and the load fail, reaching the warning branch.
- `reparse` skips an empty channel value and re-sends a non-empty one, with the
  receiver observing `changed()`.
- `current_hints` on an empty channel, and a published tokenizer becoming
  visible without a restart — the hints counterpart of
  `a_published_index_becomes_visible_without_a_restart`, writable for real
  because of Task 2.
- `claim_download`'s exclusion, re-pointed at `HintsInFlight`.
- `every_startup_error_variant_renders_a_non_empty_string` and
  `every_other_startup_error_stays_fatal` still enumerate `StartupError`
  exhaustively; it neither gains nor loses a variant this phase.

### 10.3 Frontend

Vitest: the checkbox lifecycle across all four `hints-status` outcomes, the
disabled-during-fetch state, and the warning seeding the status line on load.
`showHintsWarning` appending rather than replacing, beside the two warnings
already asserted there.

No new Playwright: the settings window is not in that suite today, and adding it
is 6B's scope, not this phase's.

### 10.4 Manual, once

1. Fresh config dir, toggle on: observe `downloading`, then a parse whose
   segmentation changes under the toggle without a restart.
2. Toggle off, then on again: no network, `unpacking` never shown, `ready`
   immediately — `published()` short-circuited.
3. Drop a second `.tar.xz` in `<hints>/`, toggle off and on: `unpacking`, and
   `<hints>/current/` holds the new dictionary with no `.partial` leftovers.
4. Delete `<hints>/current/` with the setting on, restart: hints off, the
   warning visible in both windows, the checkbox still reading on.
5. Write junk to `<hints>/current/system.dic`, toggle on: the load failure names
   the Vibrato version as a possible cause and the setting stays off.
6. `TA_HINTS_DICT` set to a nonexistent path: still a fatal startup failure.

### 10.5 Gate

`cargo test --workspace` at or above the baseline; `cargo test -p jparser
--features hints-source`; `cargo check -p jparser --no-default-features
--all-targets`; `cargo clippy --workspace --all-targets --exclude clipyomi -- -D
warnings` clean; the clippy warning count for `clipyomi` unchanged from
baseline; `npx vitest run`; `npx playwright test`; both `cargo tree` TLS greps
clean on the Linux target and the Windows host.

The `-D warnings` run including `clipyomi` still fails on Windows for the four
pre-existing `upper_case_acronyms` warnings the 4D and 5A handoffs both record.
This is the first phase since then to touch `src-tauri`; it does not fix them,
and it must not add to them.

---

## 11. Errors

| Condition | Surfaces as | Retried |
| --- | --- | --- |
| No archive, network fails | `SourceError::TooManyAttempts` in the status line | by the user |
| Downloaded bytes wrong length or hash | `SourceError::Size` / `Hash` | inside `fetch_with_retry` |
| Archive unreadable, wrong format, or over a cap | `SourceError::Archive` | no |
| Archive carries no `system.dic.zst` | `SourceError::MissingMember`, naming the archive | no |
| Publish blocked | `SourceError::Publish`, naming both paths | no |
| Disk full | `SourceError::Io` | no |
| Dictionary loads but Vibrato refuses it | `HintsError::Dictionary`, wrapped with the version hint | no |
| Setting on, nothing published, at startup | `HintsWarning` in both windows | n/a |
| `TA_HINTS_DICT` set and unreadable | `StartupError::Hints`, fatal | n/a |

---

## 12. Deferred

- **Detecting a non-IPADIC dictionary.** `hints.rs:22` hardcodes
  `READING_FIELD = 7` as IPADIC's reading column. A UniDic or NEologd dictionary
  dropped into the slot loads without error and puts something else in field 7,
  so hints degrade silently rather than failing. Detection means probing a known
  token's feature layout at load — a guess with a false-positive cost that would
  refuse a working dictionary. The settings note naming IPADIC is the honest fix
  for now.
- **Installing a hand-placed archive at startup.** §5.
- **A progress percentage.** §6.1.
- **Sweeping staging directories.** Inherited from 5A unchanged, for the reason
  `jmdict-source` already documents: a process cannot distinguish another's live
  staging directory from a dead one's leftovers.
- **Layout slots capturing `mecab_hints`.** §2.
- **Dictionary updates as a first-class action.** Dropping a newer archive in is
  the whole mechanism. Nothing checks for a new release.

---

## 13. Deviations to record in the master spec at phase close

| # | Deviation | Why |
| - | --------- | --- |
| 19 | One version-blind slot, `<hints>/current/`, amending row 17's version-named directory | Row 17's directory was named for the pinned release on the reasoning that a later one would land beside it. Nothing ever reads a second slot, and 5B accepts dictionaries the pin does not describe, so a name derived from the pin would misdescribe its own contents. §3.1 |
| 20 | A hand-placed archive is unpacked without a hash check, while the download keeps `ASSET_SHA256` and `ASSET_BYTES` | The pin proves a *network response* is the artifact this design chose — against a repointed URL, a truncating proxy, a tampered mirror. A file the user placed in their own config directory is already their choice, and hashing it could only refuse what they asked for. `jmdict-source`'s hand-placed file has no hash either. §3.3 |
