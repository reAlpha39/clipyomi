# JParser Phase 4D — Handoff

**Date:** 2026-08-26
**Spec:** [`docs/superpowers/specs/2026-08-26-jparser-phase4d-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-26-jparser-phase4d-design.md)
**Commits:** `c98095c..b05a4d1` on `develop` (fifteen commits, unpushed)

This is the last sub-phase of Phase 4. Phase 4 (window behaviour) is
complete as of this close.

---

## 1. What Shipped

1. **The dependency** (`c98095c`). `tauri-plugin-global-shortcut`, pinned to
   the exact resolved `2.3.2`, matching the `tauri-plugin-clipboard-manager`
   precedent. MIT OR Apache-2.0, taken under **MIT** — Apache-2.0 alone is
   incompatible with this project's GPL-2.0-only. Task 0 verified the
   license and the four API facts §4.1 of the design spec assumed (the
   builder/handler shape, `ShortcutState`, shortcut-string parsing) before
   any code was written; a fifth fact — whether macOS needs an accessibility
   permission for this backend — could not be confirmed from source and is
   carried into §4 below.

2. **`LayoutSlot` and `Settings.layout_slots`** (`2f9b97c`). A `BTreeMap<String,
   LayoutSlot>` in `settings.rs`, persisted only when non-empty. The same
   commit extracts `merge_saved` out of `save_settings` so the
   `layout_slots`-must-stay-untouched contract (§3.3 design) has one place to
   live and one test pinning it.

3. **`layout.rs` and the pure geometry clamp** (`3d3a5c6`, `462779c`,
   `ffa336e`). `fit(slot, monitors, primary, scale_hint) -> PhysRect` keeps a
   restored window on a monitor that still exists: probes the slot's
   physical box, targets whichever monitor's work area overlaps it most (or
   falls back to primary), re-derives size at the target's own scale, then
   clamps into the work area at the configured 160×80 floor.

4. **`unpeek` and `commands::is_peeked`** (`c8fe56c`). Corrects a bind
   captured mid-peek (macOS only) so the stored box doesn't carry the extra
   band height and the shifted `y` the peek grew the frame by.

5. **`apply_slot`** (`8177a4a`). The assignment merge: a restored slot
   overwrites `SettingsState`'s geometry, chrome mode, always-on-top, alpha,
   font sizes and furigana mode outright — never merges — because a slot
   that recorded "fonts unset" has to clear them, which a merge can't
   express.

6. **`slot_key`, `slot_from`, `live_box`, `bind`** (`d1fc64c`). Bind reads
   geometry from the live window (`inner_size` + `outer_position` +
   `scale_factor`, corrected for an in-flight peek) and everything else from
   `SettingsState` — never from the frontend's live CSS token, per 4B
   handoff §3's warning about the clamped alpha.

7. **`apply_chrome`** (`ea5e014`), extracted from `set_chrome_mode` so
   restore and the settings-window chrome switch share one per-platform
   body rather than duplicating it.

8. **`slot_to_restore`, `monitors`, `restore`** (`1fffa94`). An absent slot
   is a silent no-op. Otherwise: resolve the mode through `supported()`,
   assign into `SettingsState`, apply geometry (fitted), always-on-top and
   chrome in Rust, then emit one bare `settings-changed` carrying the full
   `Settings` for the token-level half.

9. **The plugin, the twenty registrations, `ShortcutWarning`,
   `showShortcutWarning`** (`f352c03`). `Alt+Digit1..9` restores,
   `Shift+Alt+Digit1..9` binds, `Alt+BracketLeft`/`Alt+BracketRight` step
   history. The handler gates on `ShortcutState::Pressed` (the release event
   is otherwise a second, harmless call) and does not debounce auto-repeat
   (both bind and restore are idempotent, so a held key is harmless by
   construction, not by luck). A shortcut another application already owns
   fails to register; the failed names collect into a managed
   `ShortcutWarning(String)`, exposed by a `shortcut_warning` command and
   rendered the same way `showSettingsWarning` renders `SettingsWarning`.

10. **`history-step`** (`b537080`). `Alt+[`/`Alt+]` emit `-1`/`1` to the main
    window; the listener is the same one-liner the header chevrons already
    use, so `display(history.back())`/`display(history.forward())` fires
    with no new guard — `display(null)` is already a no-op at either end of
    the timeline.

11. **The settings-window font-box gap and the one-emit restore pin**
    (`84c909c`, `4a5a8c8`, `170aaec`). `settings.ts`'s `applySettings` now
    clears its font boxes on a full snapshot whose font keys are absent,
    rather than leaving a stale number showing after a restore-to-unset —
    the second surface of 4B ruling 1's bug class. A `main.test.ts` test
    pins that one restore emit moves the alpha, all three font sizes, the
    furigana mode and the chrome mode together, with a real rendered-furigana
    assertion (not just a token-clearing check — see §2 below).

12. **The `e2e/stub.ts` gap that failed the gate, closed** (`b05a4d1`). See
    §2's execution finding.

---

## 2. Design Decisions and Findings Taken During Execution

These are the rulings recorded in `.superpowers/sdd/2026-08-26-jparser-phase4d/progress.md`
that a future reader of this phase needs, carried into my own words.

1. **Slot keys are `String`, not `u8`.** `settings.rs`'s `load` turns *any*
   parse failure into the whole settings file resetting to defaults plus a
   warning — the behaviour the existing
   `an_unknown_mode_name_falls_back_to_defaults_with_a_warning` test pins
   deliberately. A `u8` key would mean one hand-typed slot name (e.g. a
   stray `"1a"` from manual `settings.json` editing) costs the user every
   other setting — geometry, alpha, chrome mode, fonts — not just the one
   slot. A `String` key costs nothing: the key is simply never looked up,
   and the `extra`-style pass-through keeps it in the file for the next
   load to ignore or use. Nine slots is nine times the surface for that
   typo, which is what tips the decision away from the stricter type.

2. **`save_settings` deliberately never names `layout_slots`.** Its absence
   from the key-by-key merge *is* the feature: the settings window's
   `currentSnapshot()` never builds `layout_slots`, so if `save_settings`
   assigned it, every save from the settings window (changing the theme,
   toggling monitoring) would wipe every bound slot back to an empty map.
   A test pins the omission directly, because nothing in the code itself
   says "leave this key alone" — only a test stops a future contributor
   from "completing" the pattern by adding the assignment.

3. **`#[allow(dead_code)]` was used only as temporary scaffolding.** As the
   phase built up function-by-function ahead of their eventual callers
   (`fit`/`overlap`, then `unpeek`/`is_peeked`, then `bind` itself), each
   suppression carried a one-sentence comment naming the later task that
   would retire it. By the phase's end every one had been removed — Task 7
   confirmed zero `#[allow(dead_code)]` survive in the phase's own code
   (the one pre-existing survivor, `mouse_tracker.rs:10`, predates this
   phase entirely).

4. **The plan's own code was corrected twice during execution, both against
   the compiler.** First: `register(shortcut)`, not
   `register(shortcut.clone())` as the plan's code block had it — `Shortcut`
   is `Copy`, so the clone would have tripped clippy's `clone_on_copy`,
   which the four-warning ceiling forbids. Second: `showSettingsWarning` was
   changed from `replaceChildren` to `append`, matching `showShortcutWarning`,
   so the two independent fire-and-forget warning renders can't wipe each
   other out regardless of which resolves first — a real hazard once a
   second warning source (`shortcut_warning`) existed alongside the first.

5. **Task 10's restore-emit test was rewritten to earn its name.** Review
   found the first version was named "moves the alpha, the sizes, the
   furigana and the chrome" but asserted no furigana effect and exercised
   only one of the three sizes — the design spec §8.2 explicitly asks for
   the fuller assertion the plan's test text had omitted. The fix added a
   real assertion driven by a rendered parse (traced through `display` and
   `render/furigana.ts`, not just a mode flag) and made the `--text-gloss`
   check load-bearing rather than vacuous.

6. **A false-positive review finding cost two fix rounds and is worth
   recording so it isn't repeated.** Two reviewers independently asserted
   that rustc cannot pair `error[E0425]` ("cannot find value/function") with
   the message text "cannot find type" in the same compile. It can — for a
   name that appears in generic-argument position, a struct-literal
   position, and a call position in the same broken module, rustc correctly
   emits `E0422` for one shape and `E0425` for the others, and one of those
   E0425s does read "cannot find type `MonitorRect` in this scope" when the
   missing name is used where a type is expected. Confirmed by a direct
   three-shape probe against rustc rather than by re-reading the same
   transcript a third time. The lesson for future reviewers on this port:
   an "impossible error pairing" claim about rustc's diagnostic wording is
   itself a claim that needs a probe, not an assumption.

7. **The gate caught a real regression in the test harness, not in the app,
   and it is the most instructive finding of this phase.** `e2e/stub.ts`
   never answered the `shortcut_warning` command Task 7 added, so a call to
   it fell through to the stub's default `return undefined`.
   `showShortcutWarning`'s guard is `if (message === null) return;`, and
   `undefined` passes that guard (only `null` matches), so `errorBlock(undefined)`
   ran; `.textContent = undefined` coerces to the empty string for a
   nullable DOMString, producing an empty `<pre class="startup-error">`
   element — and that class carries `padding: var(--space-pane)`, so the
   empty box still occupied space and pushed the download-screen content
   down roughly 55px in four Playwright screenshots (`panes.spec.ts`,
   the download screen's offer/in-flight states, light and dark). The
   `actual`/`diff` PNGs showed identical content, shifted — no new glyphs,
   which is what pointed at layout rather than content. **Production was
   never at risk**: the real `shortcut_warning` command returns
   `Option<String>`, so a live app gets `null`, not `undefined`, and the
   guard short-circuits correctly. Fixed in `b05a4d1` — one line in
   `e2e/stub.ts` answering `shortcut_warning` with `null`, mirroring the
   `settings_warning` line already above it — with no baseline
   regenerated. The lesson: `e2e/stub.ts`'s standing contract is that it
   answers *every* command the frontend can call, so any phase that adds a
   command must extend the stub in the same commit or a later one, and the
   failure mode when that's missed is silent — an invisible padded element
   whose only visible symptom is a pixel shift in a screenshot test
   completely unrelated to the feature that added the command.

---

## 3. The Verification Gate

Run and personally re-verified at commit `b05a4d1` (tip of `develop`):

- **Vitest:** 252 passed (up from 244 at phase start). `Test Files 13
  passed (13)`.
- **`npm run build`:** clean — `tsc` reports no errors, `vite build`
  completes (`✓ built in 77ms`).
- **Playwright:** **45 passed**, up from 41 at 4B's close. This is 45, not
  the design spec's expected 44 — the extra test predates this phase
  (`26ac516`, a 4B fix-round commit, added one test to `chrome.spec.ts`
  before Task 0 of 4D ever ran), so 44 was already a stale expectation
  going into this phase, and the difference is not something 4D caused.
  Re-ran twice; both times all 45 pass and `git status --short` shows no
  screenshot baseline touched — only the pre-existing, unrelated
  `.gitignore` modification that predates this phase's first commit.
- **`cargo test`:** 119 passed (up from 88 at phase start).
- **`cargo clippy --all-targets`:** exactly four warnings, all
  `upper_case_acronyms`, on `RECT`, `STYLESTRUCT`, `SUBCLASSPROC`,
  `WINDOWPOS` — the same four that predate this phase. 4D added no new
  clippy debt.

**Note on how this gate was reached.** The first pass through this gate, at
`170aaec`, showed the same four Playwright screenshot mismatches described
in §2 finding 7. That run was reported rather than worked around — no code
was changed on that pass, and no handoff was written claiming a passing
gate that hadn't passed. The stub fix landed as a separate commit
(`b05a4d1`) after diagnosis, and the gate above is the re-run after that
fix, verified again with my own commands rather than taken on trust.

---

## 4. What Is Unverified

**Every one of the seven manual checks in the design spec's §8.4 — NOT RUN.**
Individually, by name:

1. Bind content mode + 35% alpha + large fonts to slot 1, header-mode
   defaults to slot 2, alternate — confirming all six captured things move
   each way — **NOT RUN.**
2. A slot bound with the font boxes empty restores to empty — **NOT RUN.**
3. Bind on a second monitor, unplug it, restore — confirming the window
   lands on the remaining screen — **NOT RUN.**
4. Hold `Alt+3` — confirming the auto-repeats are harmless — **NOT RUN.**
5. Run with another application already owning `Alt+5` — confirming the
   startup warning names it — **NOT RUN.**
6. macOS/Linux only: restore a 40% slot while chrome is `native` —
   confirming the screen clamps to 100% and `settings.json` still reads 40
   — **NOT RUN.**
7. macOS only: bind during a peek — confirming the stored height carries no
   extra band — **NOT RUN.**

None of these seven is claimed as passed, inferred from the automated gate,
or simulated anywhere in this handoff. An unrun check that goes unmentioned
would read to whoever picks up the next phase as a check that passed; it
did not run at all. This phase was implemented and verified entirely on
Windows, as every prior phase of this port was.

**macOS is entirely unrun**, same as every prior phase — not just checks 6
and 7 above, but the plugin's behaviour on that platform generally.

**One API fact from Task 0 remains unconfirmed from source: whether macOS
needs an accessibility permission for the global-shortcut backend.** Task 0
could not find a statement either way in the crate's docs and reported it
as an inference only: the twenty shortcuts register through Carbon's
`RegisterEventHotKey`, which historically does not require that permission
(unlike the `CGEventTap` media-key path this crate also exposes but this
phase never calls). A later reviewer independently traced the same code and
confirmed the twenty registrations never reach `start_watching_media_keys`
or the `CGEventTapCreate` path — which corroborates the inference without
proving it. Whoever first runs this on macOS should treat "hotkeys are
silently dead until a permission is granted" as a live possibility until
observed otherwise.

---

## 5. What This Phase Did Not Clear

Carried forward from 4B handoff §8, still outstanding — 4D did not touch
any of these:

- **4A's own manual checklist** — never run in 4A, still not run.
- **4B's six manual checks and two Windows-only manual items** (4B handoff
  §5/§5.1) — five of the six remain NOT RUN (checks 1, 2, 3, 5, 6); check 4
  was only partially observed by hand once, and that partial observation is
  4B's own record, not re-verified here.
- **`docs/superpowers/2026-08-18-windows-titlebar-test-plan.md`** — carried
  forward unreported since at least 4A's handoff §6; still outstanding.
- **4A's open finding on `set_chrome_mode`'s persist ordering** (4A handoff
  §9, restated in 4B handoff §8): persistence runs before the window is
  actually updated to match, so a failure in the apply half leaves a
  persisted value the window never reached. 4D's `restore` adds a second
  caller to the same pattern (design spec §5.2) and does not fix it.
- **4B's peeked-band double-surface cosmetic gap** (4B handoff §7 item 4):
  the control strip renders slightly more solid than the surface it sits
  on. Left as-is by 4B's own ruling; 4D does not touch it.

Deferred minors recorded in this phase's own ledger, none acted on:

- Task 0: the report's fact-1 citation is off by one line (`lib.rs:23-25`
  vs. the actual `:22`) — report text only, no code impact.
- Task 1: the tripped clippy lint that forced a `contains_key` rewrite is
  named `unnecessary_get_then_check` in the implementer's report without
  independent confirmation of that exact lint name — clippy is clean
  either way.
- Task 3: `cargo fmt -- --check` shows 33 pre-existing diffs on this tree,
  confirmed present before this phase started (`git stash` comparison
  against the base commit). Not this phase's to fix; the gate never named
  `cargo fmt`. Flagged again here for whoever next touches formatting.
- Task 5: `live_box` takes the default-runtime `tauri::Window` rather than
  being generic over `R: Runtime`, consistent with `bind`'s own
  non-generic `&AppHandle`. Forecloses a non-default runtime this desktop
  app never uses.
- Task 6: `supported()` is computed twice on the restore path (once for
  `apply_chrome`, once inside `apply_slot`). Both calls are pure and cheap.
- Task 7: `register_all` folds a genuine key conflict and an OS-layer
  registration error into the same "taken" bucket in `ShortcutWarning`.
  Matches the design spec's stated shape; a finer-grained diagnostic would
  widen that match arm.
- Task 8: `HISTORY_STEP_EVENT` sits above its own listener in `main.ts`
  rather than in the file's existing central constants block
  (`PEEK_HIDE_MS`, `DWELL_MS`, `KEEP_POLL_MS`).

---

## 6. Deferred (design spec §9)

- **A transient "Bound N" indicator.** Bind is silent by design — restore
  is its own verification (press `Alt+N`, the window snaps), and the one
  genuinely invisible failure (a shortcut another app owns) is already
  surfaced at startup via `ShortcutWarning`. New transient UI, its CSS and
  its tests were judged not worth that gap.
- **User-editable key bindings.** The twenty shortcuts are fixed. A
  rebinding surface needs persistence, conflict detection and a settings
  UI; nothing currently asks for it.
- **A slot list or clear affordance in the settings window.** Slots are
  visible and editable directly in `settings.json`; overwriting a slot by
  rebinding is the only supported "clear."
- **Disk-persisted history inside a slot.** Permanently out, per 4C
  handoff §4 — history stays session-only with no persisted key.
- **The mixed-DPI second pass in `fit`.** Named as §6's stated ceiling: the
  current single re-derivation at the target monitor's scale is the extent
  of the DPI handling shipped.
- **`set_chrome_mode`'s persist-before-apply ordering.** 4A handoff §9's
  open finding, which `restore` now shares as a second caller (§5 above).

---

## 7. Master Spec Changes Made at This Close

- **§11's Phase 4 heading and intro**: now reads "4A, 4B, 4C and 4D
  shipped" — Phase 4 is complete.
- **§11's 4D row**: rewritten to past tense, describing what shipped (the
  plugin, the twenty shortcuts, the slot shape, the fit-to-monitor clamp,
  the stored-not-clamped alpha), and noting the two deviations recorded
  below.
- **§12**: gained rows 13 (the two global history hotkeys, added beyond
  §7.9's original scope) and 14 (restore clamps geometry into a visible
  monitor rather than restoring it verbatim), exactly as design spec §10
  states them.

No other master-spec text was touched.
