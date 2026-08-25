# JParser Phase 4C — Handoff

**Date:** 2026-08-25
**Spec:** [`docs/superpowers/specs/2026-08-25-jparser-phase4c-design.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/specs/2026-08-25-jparser-phase4c-design.md)
**Plan:** [`docs/superpowers/plans/2026-08-25-jparser-phase4c.md`](file:///D:/Code/oss/clipyomi/docs/superpowers/plans/2026-08-25-jparser-phase4c.md)
**Commits:** `482c5fc..eb08d15` on `develop` (eight commits, unpushed)

---

## 1. What Was Completed in Phase 4C

1. **Pure history module ([`src/history.ts`](file:///D:/Code/oss/clipyomi/src/history.ts))**:
   - `HISTORY_CAP = 50` — the only statement of the cap; the tests assert against
     the constant rather than restating the number.
   - `createHistory()` returns a closure over a `ParseResult[]` and a cursor —
     `push`, `current`, `back`, `forward`, `canBack`, `canForward`. A factory
     rather than module state, so each test gets a fresh one with no reset to
     remember.
   - `push` skips a parse whose joined surface text repeats the newest entry,
     and on a real push sets `cursor = buffer.length - 1` **from the length**
     after the `shift()`. That is what makes eviction free of rebasing
     arithmetic — see design §4.
   - `current()` exists because `push` may skip: a caller cannot assume the
     parse it just pushed became current, so it has to ask.

2. **The split in [`src/main.ts`](file:///D:/Code/oss/clipyomi/src/main.ts)**:
   - `display(result | null)` renders whatever parse it is handed, resets
     `panes.scrollTop`, and re-syncs the buttons. It is the **only** writer of
     `lastResult`, which is what keeps `segmentAt` resolving a hovered chip
     against the sentence actually on screen.
   - `show(result)` records first, clears `parseError`, then displays
     `history.current()` — never the parse it just pushed.
   - The `settings-changed` furigana re-render now calls `display(lastResult)`
     instead of duplicating its body. Before the fix wave it was a second,
     divergent render path; anything added to `display` would have silently
     skipped it.
   - `parseError` is cleared in `show` only, never in `display`, so navigating
     leaves an error banner standing above the parse — deliberate, and now
     pinned by tests.

3. **The header nav ([`src/main.ts`](file:///D:/Code/oss/clipyomi/src/main.ts), [`src/styles/global.css`](file:///D:/Code/oss/clipyomi/src/styles/global.css))**:
   - Two 24 px chevron buttons in a `<nav aria-label="Parse history">` at the
     band's left edge, ahead of the gear — `#settings-toggle` carries
     `margin-left: auto`, so anything before it sits left.
   - `disabled` in the markup, so the first painted frame is correct with no
     boot-time sync to race. `opacity: 0.45` when disabled, an opacity rather
     than a dimmer token so the blend works in both themes.
   - `.history-controls` joins the two `[data-decorations='false']` rules as a
     single class selector, so the pair hides and peeks with the rest of the
     chrome off one shared rule.
   - `showPeek`/`hidePeek` wired to `focus`/`blur` on both buttons. The nav is
     DOM-ahead of the gear and the window buttons carry `tabindex="-1"`, so
     `#history-back` is genuinely the document's first tab stop once enabled —
     invisible in frameless idle mode without this. See design §7.1.

4. **Verification gate**:
   - 194/194 Vitest across 11 files (`history.test.ts` is new).
   - 0 TypeScript errors. `cargo test` and `cargo clippy --all-targets` clean
     but for the four pre-existing Win32 acronym warnings — 4C changed no Rust.
   - `npx playwright test e2e/panes.spec.ts`: 19/19. Full suite 22/22 on a warm
     dev server; see design §8 for the cold-server flake, which reproduced
     during final verification and is not 4C's.
   - Ten `*-win32.png` baselines regenerated. The `*-darwin.png` set is now
     stale against them and can only be regenerated on macOS — the established
     pattern in this repo.

**Not done:** a manual pass under `npm run tauri dev`. The buttons have never
been clicked by hand, and three things in particular are only ever going to be
judged by eye: whether `opacity: 0.45` actually reads as *disabled* rather than
as *broken* on a real display, whether the chevrons sit comfortably beside the
macOS traffic lights (the band reserves 78 px there), and whether the scroll
reset feels right or jarring when stepping back through long parses.

---

## 2. Relationship to Master Spec §7.8

§7.8 asked for a ring buffer of 50 full `ParseResult`s, consecutive duplicates
deduped, back/forward in the header, session-only. All four shipped as written.

§7.8 did not specify what happens when a parse arrives while the user is reading
an older step, so design §4 settles it: **append, keep everything ahead of the
cursor, move the cursor to the newest.** Browser-style truncation was rejected
because forward history in a browser is a path the user chose not to take, while
here C and D are things the user actually copied and the arrival of E is the
clipboard's doing, not a navigation. That is an elaboration of §7.8, not a
deviation from it.

---

## 3. Process note, for anyone auditing this phase

4C ran through subagent-driven development: five tasks, a fresh implementer and
an independent reviewer per task, then a whole-branch review on the most capable
model and one scoped fix wave. Every task review came back clean; the final
review returned no Critical or Important findings.

One provenance caveat the git history cannot show. An earlier in-session build of
this phase existed in a stash, and that stash was popped during Task 1 — so
commit `748874b` (`src/history.ts` and its tests) almost certainly contains that
earlier build's files rather than an independently written module. Its RED/GREEN
evidence was re-derived honestly by removing the file and re-running, and its
review read the diff without knowing any of this and passed it. The remaining
tasks ran against a tree with that work quarantined back out. Recorded because
"this was written test-first from the brief" is a weaker claim for `748874b`
than for the other four commits, and nothing in the log says so.

Two findings were parked rather than fixed, both in design §9: the focus-drop on
disable, and the reduced-motion gap whose plan-specified fix provably does not
work (`.history-controls` at 0,1,0 loses to `#app[data-decorations='false']
.history-controls` at 2,1,0). Both match `#settings-toggle`'s existing behaviour
exactly, so they are house style rather than new debt.

---

## 4. Next Up

**4A — the other two chrome states (§7.4).** Content-only mode with the toggles
moving to a right-click menu, plus native frame; a three-way control persisted
alongside `decorations`. Depends on 2K, which shipped. Then **4B — per-pixel
background alpha (§7.5)**, which 4A must define the clamp for, since a native
frame forces alpha to 100 %.

**4D — layout hotkeys (§7.9)** stays last. One thing 4C settles for it: **a
layout slot does not need to capture history.** History is session-only and
holds no persisted key, so 4D's snapshot list is unchanged by this phase —
geometry, chrome mode, background alpha, always-on-top, furigana mode, and 3C's
three font sizes. If disk persistence for history is ever built (design §9), it
belongs in its own file, not in a layout slot.

Also worth knowing before touching the header again: the band is now carrying
four interactive things (history pair, gear, minimise, close) plus the macOS
traffic-light reservation. 4A moves the toggles into a context menu, which is
the pressure release, but anything else added to the band should probably go
there instead.

Separately open, unrelated to the phase track: the Windows titlebar test plan of
2026-08-18, still unreported; the cold-server Playwright flake of design §8,
which belongs to 6B; and the stale `*-darwin.png` baselines.
