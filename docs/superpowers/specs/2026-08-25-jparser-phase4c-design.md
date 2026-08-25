# JParser Phase 4C — Parse History Design

**Date:** 2026-08-25
**Status:** Approved, not yet implemented.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.8, §11)

---

## 1. Goal

Keep the session's parses reachable. With clipboard monitoring on, every copy
destroys the previous parse — a user who copies a second sentence to check a
word loses the first one with no way back. §7.8 puts it plainly: history is what
makes leaving monitoring running safe.

4C is first in Phase 4 because it depends on nothing (§11) and because it is the
sub-phase a user notices immediately.

---

## 2. Scope

Ring buffer of 50 **full `ParseResult`s**, consecutive duplicates deduped,
Back/Forward buttons in the header band. Session-only.

Storing whole results rather than the input text is §7.8's own decision, and it
buys two things: navigation is a render with no round trip to Rust, and a
dictionary rebuild mid-session cannot change what an old step renders as.

Out of scope, and why:

| Not doing | Reason |
| --- | --- |
| Disk persistence | §7.8 says session-only; ta-old's `exe/History/` is named there as a later addition |
| Keyboard shortcuts | 4D owns the global-shortcut plugin and `Alt+1..9`; a local `Alt+Left` now would collide with the binding surface 4D defines |
| A history list or dropdown | §7.8 asks for back/forward. A jump-to-any-parse surface is a different feature with its own layout question |
| Anything in Rust | The buffer holds a shape the frontend already has in hand, for a lifetime shorter than the webview's |

---

## 3. Where the buffer lives

**In the webview, not in Rust.** Three reasons, in order of weight:

1. Session-only means the buffer's lifetime is *shorter* than the webview's, so
   there is nothing for Rust to outlive.
2. `ParseResult` is already in the webview as a plain object. Keeping it in
   `AppState` would mean an `invoke` per navigation, serialising a full parse
   across the IPC boundary to render something the frontend just had.
3. Rust already owns the *other* dedup — `clipboard::should_parse` skips text it
   has just seen. Splitting one concern across two languages is how it stops
   being one concern.

The cost is honest and small: a webview reload during `npm run dev` drops the
history. In a shipped build there is no reload.

---

## 4. Navigation model

A parse arriving while the user is reading an older step **appends, keeps
everything ahead of the cursor, and moves the cursor to the newest**.

```
before:  [A][B][C][D]        cursor at B
new parse E arrives
after:   [A][B][C][D][E]     cursor at E      Back walks E→D→C→B→A
```

Rejected: browser-style truncation, which would drop C and D. Browsers truncate
because forward history is a path *not taken* — the user chose B over C. Here
nobody chose anything: C and D are things the user copied, and the arrival of E
is the clipboard's doing, not a navigation. Truncating would delete real content
on an event the user did not initiate, and with monitoring left running that
event is the normal case.

Also rejected: appending without moving the cursor. Nothing would ever steal the
view mid-read, but the newest parse would then never appear on its own, which is
the app's entire behaviour.

**Consequence worth stating:** because a push always lands the cursor on the
newest entry, eviction at the cap can never leave the cursor stale. Eviction
shifts every index down by one; a cursor set from `length - 1` after the shift is
correct by construction, with no rebasing arithmetic anywhere.

---

## 5. Dedup

Consecutive duplicates only, keyed on **the joined surface text** of the
segments — what the user copied, independent of how it segmented.

A duplicate is *not stored and does not move the cursor*. Nothing new arrived, so
a user reading an older step must not be yanked forward. This is the one place
where "skip the push" and "skip the whole event" differ, and getting it wrong is
a visible bug.

The surface text rather than a deep compare of the whole result: it is the
identity of the input, it is cheap, and two parses of the same text are
interchangeable to a reader even if a dictionary rebuild between them changed an
entry. §7.8's "immune to the dictionary changing" is about what an old step
*renders as*, which storing full results already guarantees.

Note that `clipboard::should_parse` means a consecutive duplicate rarely reaches
the frontend at all — it arrives via the `parse_text` debug command, or after the
worker's `last_seen` resets. The rule is cheap and the alternative is a second
identical step in the chain, so it stays.

---

## 6. Architecture & data flow

```
src/
├── history.ts          # NEW. createHistory() — pure, no DOM
├── history.test.ts     # NEW.
├── main.ts             # header buttons; show() splits into push + display
└── styles/global.css   # .history-controls
e2e/
└── panes.spec.ts       # band-reveal assertion; ten baselines regenerate
```

No Rust, no `types.ts` change, no new dependency.

### 6.1 The module

`createHistory()` returns a closure over a `ParseResult[]` and a cursor:

```
push(result)              record and make current, unless it repeats the newest
current()                 the parse on screen, or null before the first arrives
back() / forward()        the neighbour, or null at either end
canBack() / canForward()
```

The cap is an exported `HISTORY_CAP = 50`, so the tests assert against the
constant rather than restating the number.

A factory, not module-level state, so each test gets a fresh one with no reset to
remember — and so a second webview could hold its own if one ever needs to.

`current()` exists because `push` may skip: the caller cannot assume the parse it
just pushed became the current one, so it has to ask. Without it every call site
re-derives the skip rule, and one of them eventually gets it wrong.

### 6.2 The split in `main.ts`

`show()` today does three jobs at once. It becomes two functions:

```
display(result | null)   closePopover, lastResult = result, renderSentence, sync
                         the button disabled state. null is the no-op at either
                         end, so click handlers pass back()/forward() straight
                         through.

show(result)             history.push(result); clear parseError;
                         display(history.current())
```

`display` is the **only** writer of `lastResult`, which is what keeps the hover
lookup honest: `segmentAt` resolves chips against the sentence actually on
screen, so a chip in a parse navigated back to opens its own tooltip and not the
newest parse's. That property is the reason for the split — a nav path that
re-rendered without updating `lastResult` would look right and behave wrong.

`display` renders with the furigana mode current *now*, not the mode in force
when the parse arrived, so an old step looks like the rest of the session. This
falls out of reusing `currentFuriganaMode` and is worth a test rather than a
comment.

`parseError` is cleared in `show` only, not in `display`. Navigating therefore
leaves an error banner standing. That is deliberate and already the established
behaviour: `main.ts` keeps a failure message above the last good parse on
purpose, so the previous parse stays readable while the user works out what went
wrong. Nav clearing it would be new behaviour and more code.

---

## 7. UI

Two icon buttons in a `<nav aria-label="Parse history">` at the **left** end of
the header band, ahead of the gear.

Left, not beside the gear: `#settings-toggle` carries `margin-left: auto`, so
anything before it sits at the left edge — space that is otherwise empty, and
where a browser puts Back and Forward. It also keeps them clear of the
gear/minimise/close cluster, where a mis-click costs the window.

- 24 px squares, chevron SVGs matching the gear's stroke weight, `border-radius:
  4px`, `--color-muted` with `--color-hover` on hover.
- `position: relative; z-index: 60` — the band is a `data-tauri-drag-region`, and
  this is the same stacking the gear and window buttons already use to stay
  clickable above it.
- **Disabled, not hidden, at the ends.** Both carry `disabled` in the markup, so
  the first painted frame is already correct with no boot-time sync to race;
  `syncHistoryButtons` maintains the pair from `canBack()`/`canForward()` on
  every `display`. Two greyed chevrons on a fresh launch
  teach that history exists before there is any; a pair that materialises after
  the second copy is a feature nobody finds. `opacity: 0.45` rather than a dimmer
  colour token, so the blend is against whatever background the theme supplies —
  `--color-rule` reads clearly on the dark background and nearly vanishes on the
  light one.
- Both IDs join the two `[data-decorations='false']` rules, so the pair hides and
  peeks with the rest of the chrome in frameless mode.

### 7.1 The focus trap this creates

The pair is *ahead of* the gear, so once there is history to walk it becomes the
**first tab stop in the band** — and in frameless idle mode that stop is at
`opacity: 0`. An invisible tab stop is a WCAG 2.4.7 failure.

`main.ts` already solves this for the gear with JS rather than CSS
(`settingsToggle.addEventListener('focus', showPeek)`), because only the backend
can grow the frame and a `:focus-within` rule cannot ask it to. Both buttons get
the same `focus`/`blur` pair.

Worth writing down because it is easy to miss in testing: `disabled` buttons take
no focus, so an empty history never reveals the trap. It appears only after the
second parse.

---

## 8. Verification

| Layer | Coverage |
| --- | --- |
| `src/history.test.ts` | Empty state; a single entry offers no navigation; the two-entry walk; both ends hold; `current()` before, after, and across a skip; consecutive dedup; a non-consecutive repeat *is* stored; the key is segmentation-independent; a skip leaves the cursor; nav model A keeps the entries ahead; eviction at `HISTORY_CAP`; the cursor survives eviction |
| `src/main.test.ts` | Buttons start disabled; one parse leaves them disabled; a second enables Back; Back and Forward render and re-sync; a hover after navigating finds the *displayed* parse's entries; a new parse takes over and keeps what was ahead; a duplicate leaves the view put; an old step honours the current furigana mode; focusing either button peeks the band and blurring hides it |
| `e2e/panes.spec.ts` | `nav.history-controls` hides and reveals with the gear off the shared CSS rule; ten full-page baselines regenerate for the new header content |

`npx tsc --noEmit`, `npm test` and the Playwright suite green before the phase is
done. No Rust changes, so `cargo test` and `cargo clippy` are untouched by 4C and
are run only to confirm that.

**Known flake, not caused by 4C:** the Playwright suite fails exactly one test per
full run, and *which* test moves between runs — a first-navigation race against a
cold Vite dev server, where the fixture is emitted before `main.ts` has
registered its listener. A baseline run with 4C's changes stashed failed
`popover.spec.ts:72`; the run with them applied failed `panes.spec.ts:31`; both
runs passed 21. Every one of these passes in isolation. Out of scope here — it
belongs with 6B, which owns the suite.

---

## 9. Deferred

- Disk persistence across launches (§7.8 names it as a later addition).
- Keyboard shortcuts, once 4D has defined the binding surface.
- A jump-to-any-parse list, if back/forward turns out to be too slow to reach an
  older sentence.
- A history-cleared control. Nothing sensitive is stored that a restart does not
  already drop.
