# JParser Phase 2I — The Tooltip as a Real Window (Design)

2G brought back the glance. First real use showed it only half works: the popover
is a `<div>` inside the webview, so the main window's edge clips it, and at the
480×320 size the app is built for there is nearly nothing left to show. 2I makes
the tooltip an actual OS window, the way ta-old always had it, and gives it
ta-old's content.

**Reference:** ta-old's `exe/Dialogs/MyToolTip.cpp` and
`exe/util/DictionaryUtil.cpp` are the behaviour being ported. `ta-old/` is
**read-only — never modify it**. The predecessor spec is
`docs/superpowers/specs/2026-08-14-jparser-phase2g-design.md`; this document
supersedes its §2 (architecture), §3.3 (non-interactive), §3.4 (height cap), and
§4 (placement), and leaves the rest standing.

**Predecessor code:** the shipped 2G tree at `986e93a`.

**Sequencing note.** The 2E design §6.3 named Phase 2H as the removal of the
manual text input and fixed it as the next phase. 2I ships **before** 2H.
Recorded here so the reordering is a decision with a date rather than drift: 2G
shipped a feature that does not work at the app's own minimum window size, and
fixing that outranks removing an input box. §6.3 is otherwise unchanged — 2H
still follows.

## 1. Scope

**In scope:**

- The tooltip becomes a second OS window, able to extend past the main window
  onto the desktop, clamped to the monitor work area
- ta-old's content: every match for the word, stacked, not just the ranked primary
- ta-old's lexical colouring, ported as a rule rather than as a palette
- Scrolling when the content is taller than the work area allows
- ta-old's direction-of-travel rule for keeping the tooltip alive

**Not in scope:**

- **Multi-spelling headwords.** ta-old renders `旅立つ; 旅だつ【たびだつ】`; the
  index stores no `<keb>` list, so this needs a format bump and a forced
  dictionary rebuild for every existing user. Deferred to its own phase — §9.
- **The definitions pane.** It keeps `renderEntry` and its current layout. This
  phase gives the tooltip its own renderer rather than changing both surfaces.
- **Pane density, font sizes, gloss filters, furigana.** Still Phase 3, still the
  fence 2G's §1 drew.
- **Removing the manual text input.** Still Phase 2H.
- **A configurable palette.** ta-old's colours come from `config.toolTipFont`,
  `config.toolTipKanji`, and friends — user-editable. 2I ships fixed tokens; a
  settings surface for them is not in this phase and may never be.

### 1.1 Why the DOM popover cannot be made to work

A webview cannot paint outside the OS window that hosts it. This is not a Tauri
limitation to route around; it is what a window is. 2G's popover additionally
caps itself with `max-width: min(320px, 100vw - 16px)` and
`max-height: calc(100vh - 16px)`, so at 480×320 a six-sense entry has roughly
three lines of room.

ta-old never had this problem because its tooltip was never a child of the
furigana window: `MyToolTip.cpp:825` creates it as `WS_POPUP | WS_BORDER` with
`WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST`, a top-level window of its
own, positioned against the monitor rather than the parent. The observed
screenshot has the tooltip roughly five times the height of the window that
spawned it.

Two alternatives were considered and rejected. Enlarging the main window with a
transparent margin breaks resizing and hit-testing, and still bounds the tooltip
by the enlarged window. Shrinking the content to fit is what 2G already does, and
is the complaint.

## 2. Architecture

### 2.1 The window

Rust creates one `WebviewWindow` labelled `popover` at startup, hidden, and keeps
it for the process lifetime. Creating a webview per hover costs hundreds of
milliseconds and would be visible after a 350 ms dwell.

Its flags are the Tauri equivalents of ta-old's:

| ta-old | here |
|---|---|
| `WS_POPUP`, no caption | `decorations(false)` |
| `WS_EX_TOPMOST` | `always_on_top(true)` |
| `WS_EX_NOACTIVATE` | `focusable(false)` |
| `WS_EX_TOOLWINDOW` | `skip_taskbar(true)` |
| `WS_BORDER` | a 1px CSS border on the body |

`focusable(false)` and not `focused(false)`: `focused` governs creation only.
Showing the window is not passive — `WindowMessage::Show` maps to tao's
`set_visible(true)`, which on macOS is `makeKeyAndOrderFront:`, and tao's
`canBecomeKeyWindow` returns the `focusable` ivar, which defaults to true. With
`focused(false)` alone the tooltip takes key status on every hover and the main
window loses its focus ring — §7's named top risk. A non-key window still
receives `scrollWheel:` under the cursor, so §3.4's scrolling is unaffected.

It loads `popover.html`, a second Vite entry point, which renders nothing but the
tooltip. Vite needs `build.rollupOptions.input` to emit both pages.

### 2.2 The split: TypeScript computes, Rust applies

Placement arithmetic stays in TypeScript. `placePopover` is already a pure
function with unit tests, and 2G's §4 reasoning still holds — happy-dom returns
zeros from `getBoundingClientRect()`, so geometry asserted through a DOM is not
coverage. Its inputs change from a viewport width to a monitor work-area
rectangle; its shape does not.

Rust owns the window operations, exposing two commands:

```rust
#[tauri::command] fn place_popover(x: i32, y: i32, width: u32, height: u32)
#[tauri::command] fn hide_popover()
```

This split is chosen over doing the window work from JavaScript for a concrete
reason: `src-tauri/capabilities/default.json` currently grants exactly one
permission, `core:event:allow-listen`, to one window. Creating and manipulating
windows from the frontend would need `core:webview:allow-create-webview-window`
plus five or more `core:window:allow-*` grants and a second capability file, and
would permanently widen what the frontend may do to any window in the app. Two
commands are a smaller and more legible surface than that.

**This ends 2G's "no Rust changes" rule.** That was a scoping decision for one
phase, not an invariant.

### 2.3 Content delivery

The main window emits `popover-content` carrying the segment's entries; the popup
listens and renders. The popup needs its own capability entry, since capabilities
are per-window-label and `default.json` names only `main`.

Order of operations, matching `MyToolTip.cpp:455-531`:

1. Main window resolves the chip to its segment
2. Emits the entries to the popup
3. Popup renders and reports its content size back
4. Main window computes placement from that size, the chip's screen rect, and the
   work area
5. `place_popover` sizes, positions, and shows in one call

The window stays hidden until step 5, so no frame is painted at a stale position.
This is the same guarantee 2G got from `visibility: hidden`, moved up to the
window level.

### 2.4 Screen coordinates

The chip's `getBoundingClientRect()` is in the main webview's client space. Screen
space needs the window's `innerPosition()` and `scaleFactor()`, both on
`getCurrentWindow()`.

**`innerPosition()`, not `outerPosition()`.** This document said `outerPosition()`
until first real use proved it wrong. `outerPosition()` is the top-left of the
window *frame*; `innerPosition()` is the top-left of its *client area*, which is
the origin `getBoundingClientRect()` measures from. On a decorated window — and
the main window is decorated — the two differ by the title bar, so adding a
client-space rect to the frame's corner places every tooltip a title bar too
high: on top of the word it is anchored to, rather than below it. The two reads
also need different capabilities, so picking the wrong one is denied at runtime
rather than merely misplaced.

**The monitor is found by containment, not by `monitorFromPoint`.** This document
originally named `currentMonitor()`, and the implementation reached for
`monitorFromPoint()` to get the screen under the *word* rather than under the
app. That call's units are not the same on every platform: on macOS tao hands
the point straight to `CGRectContainsPoint(CGDisplayBounds(…))`, which is
logical points, while on Windows it goes to `MonitorFromPoint`, which is
physical device pixels — and `Monitor.position`/`size` are physical on both.
Fed physical coordinates on a 2× display it missed every screen for any word
past roughly half the display's width and returned `null`, and placement
returned silently, so the right-hand side of every sentence had no tooltip at
all. `availableMonitors()` plus a containment test against each monitor's
physical bounds has one answer on every platform. The work area comes from `currentMonitor()`, whose `Monitor`
type carries `workArea` in `@tauri-apps/api` 2.11.1 — the direct equivalent of
`GetMonitorInfo`'s `rcWork`, and the reason the tooltip must not clamp to the
full monitor: it would sit under the dock or the taskbar.

All three are physical pixels and must be divided by the scale factor before
mixing with CSS pixels. Getting this wrong is invisible on a 1× display and
doubles every offset on a Retina one, which is where this will be developed.

## 3. Interaction

### 3.1 Triggers

Unchanged from 2G §3.1: mouse dwell of `DWELL_MS` = 350 (ta-old's
`dwHoverTime`), re-armed per chip; focus opens immediately with no dwell.

### 3.2 The keep rule

2G dismissed the popover the moment the cursor left the chip, which was
affordable because the popover could not be interacted with. It can now, so
leaving the chip must not always dismiss.

ta-old's answer is a direction-of-travel test, not a timer
(`MyToolTip.cpp:334-354`): on each mouse move, compare the cursor's distance to
the tooltip's centre against its distance from the previous position. Closer means
the user is heading for the tooltip — keep it. Otherwise dismiss. Edge-specific
variants short-circuit the common cases (moving down toward a tooltip below,
moving up toward one above).

**Only the distance comparison is ported** (`MyToolTip.cpp:352-354`). ta-old's
four edge-specific variants at `:339-350` — the ones that make a move straight
down toward a tooltip below forgiving of horizontal drift — are a deliberate
simplification, not an oversight. The consequence is that this rule feels
twitchier than ta-old's on exactly those approaches: a diagonal drift while
heading for a tooltip directly below the word can measure as moving away and
dismiss. Recorded as a decision so a later phase can port them knowingly.

Chosen over a grace period because it holds no timer that every dismissal path
must remember to clear, and because it distinguishes the two cases a timer
conflates: moving *toward* the tooltip and merely moving *slowly*.

### 3.3 Dismissal

| Trigger | Why |
|---|---|
| Cursor leaves the chip **and** the keep rule fails | The glance is over |
| Cursor leaves the tooltip itself | Same |
| Chip loses focus | Symmetric with the focus trigger |
| `Escape` | Standard. **Focus does not move** — the chip keeps it |
| A new `parse-result` | The anchoring chip is about to leave the document |
| Main window scrolls, resizes, **or moves** | The stored screen position is now wrong |

Window **move** is new and is a direct consequence of §2.1. A DOM popover
travelled with its parent for free; a separate window does not, and would be left
stranded on the desktop.

### 3.4 Scrolling replaces the height cap

2G §3.4 accepted truncation: `pointer-events: none` made the tooltip
unscrollable, so a tall entry showed its head and nothing else. That is reversed.
The window is capped at the work-area height and scrolls within it, as ta-old does
(`MyToolTip.cpp:492-497`, `hScroll` moving by whole line heights).

The pane remains a complete route to a definition, but it is no longer the *only*
complete one — which retires the single largest cost 2G knowingly accepted.

## 4. Placement

`GAP` drops from 6px to **2px**, matching ta-old's `rAvoid.top-2` and
`rAvoid.right+2`. Clamping is against the work area, not the window.

**Anchored to the chip's rectangle, not to the cursor.** ta-old positions from the
hover point and merely *avoids* the word's rect (`rAvoid`), which it can do
because it has no keyboard trigger. §3.1 keeps 2G's focus path, and a focused chip
has no cursor to position from — so the chip rect is the anchor in both cases.
This is the one deliberate divergence from `MyToolTip.cpp` in this section, and it
also keeps `placePopover`'s existing tests meaningful.

Order, from `MyToolTip.cpp:514-527`:

1. **Preferred:** below the chip (`rAvoid.bottom + GAP`). This reverses 2G §4,
   which preferred above on the grounds that the input row sits above the
   sentence and the definition rows below it. That argument was about what the
   popover would cover *inside the window*; a tooltip that leaves the window
   mostly covers desktop, and below-first is both ta-old's behaviour and the
   convention every other tooltip follows.
2. **If it overflows the work area's bottom:** flip above the word
   (`rAvoid.top - GAP - height`)
3. **If it fits neither:** pin to the work-area bottom and move sideways — to the
   right of the word if there is room (`rAvoid.right + GAP`), otherwise to its
   left (`rAvoid.left - GAP - width`)
4. **Horizontal, last:** clamp the right edge into the work area

Step 3 is what 2G lacked, and it is the case a small window hits most: a tooltip
too tall for either side of a word near the bottom of the screen.

`placePopover` keeps its signature shape, taking a work-area rectangle in place of
a viewport width. It stays pure and stays unit-tested.

## 5. Rendering

### 5.1 The text

Assembled per match, following `DictionaryUtil.cpp:24-105`:

```
<conjugation, when present, on its own line>
消える【きえる】
  (v1,vi) (1) to disappear/to vanish/to go out of sight/to become lost
  (v1,vi) (2) to go out (of a fire, light, etc.)/to die/(P)
```

Glosses join with `/`, not `; `. Senses are numbered inline as `(N)`, not by an
`<ol>`. The `(P)` marker comes from the `common` flag. Readings sit in `【】`.
Every match is rendered, in rank order — the ranked primary is simply first.

With §9 deferred, the headword line carries the matched surface only.

### 5.2 Colour is lexical, not semantic

The important discovery, and the reason this is a port rather than a redesign:
`MyDrawText` (`MyToolTip.cpp:125-268`) never knows what a headword or a
part-of-speech is. It walks word runs and colours each by what its characters
*are*:

| Run | Token |
|---|---|
| A line prefixed with `\x01` — the conjugation line | `--tt-conj` |
| Any run from `(` to its matching `)` | `--tt-paren` |
| A Japanese run that is entirely kana | `--tt-kana` |
| A Japanese run containing anything else | `--tt-kanji` |
| Everything else | `--tt-text` |

Parenthesis colouring is tested **first**, so `(e.g. 寿司)` is entirely
parenthesis-coloured and its kanji does not win.

This is why `(v1,vi)`, `(1)`, `(e.g. of hope)` and `(P)` all render identically:
nothing distinguishes them. It is also why a gloss that happens to contain kanji
renders in the kanji colour — an outcome semantic markup would get wrong, and a
concrete argument for porting the rule rather than approximating its output.

The renderer is therefore a **lexical colouriser**: assembled string in, coloured
runs out. Pure, and unit-testable without a DOM.

### 5.3 Tokens

Five new colour tokens — `--tt-kanji`, `--tt-kana`, `--tt-paren`, `--tt-conj`,
`--tt-text` — plus a tooltip background and border. Each is defined on bare
`:root` first and redefined in both the `@media (prefers-color-scheme: dark)` and
`:root[data-theme='dark']` blocks, which is the rule this project has held since
2D.

The light background approximates `COLOR_INFOBK`'s pale yellow. Dark values are
picked fresh rather than reused: a crimson legible on pale yellow is not legible
on near-black.

### 5.4 Layout details worth porting

Continuation lines indent by 10px (`xOffset = 10`), so a wrapped sense hangs under
its own start rather than resetting to the margin. Word breaks fall after `-`,
`/`, `,`, `;`, `)`, at script boundaries, and before `(` and `【`. These are what
make the wrapped output in the reference screenshot read as columns rather than
as a paragraph.

## 6. Testing

**Vitest, pure functions — where the real coverage is:**

- the text assembler: one match, several matches, a conjugation line, a `(P)`
  entry, an entry with no reading
- the colouriser: each of the five run kinds; parenthesis beating kanji inside
  `(e.g. 寿司)`; a gloss containing kanji; nested and unclosed parentheses
- `placePopover` against a work-area rectangle: below-preferred, flip above,
  the sideways fallback both ways, and the horizontal clamp
- the keep rule: a cursor moving toward the tooltip keeps it, one moving away
  dismisses it, one moving parallel dismisses it

**Vitest, wiring:** the dwell, the focus path, `Escape`, and every dismissal
trigger in §3.3 including window move.

**Playwright:** what real geometry proves — that the tooltip window opens, shows
the right text, and scrolls. Playwright drives webviews, not the OS window
manager, so it cannot assert that the window escapes the main window's bounds.

**By hand, and this phase depends on it more than any before:** that the tooltip
really does extend past the main window; that it never steals focus; that it
clamps above the dock rather than under it; that it lands correctly on a Retina
display and on a second monitor. Every one of these is a window-manager
behaviour no assertion in this repo can observe. 2E and 2F each shipped a defect
that only a manual pass caught, and 2G shipped one — a serif gloss — that the
suites not only missed but committed a baseline ratifying.

**Screenshot baselines:** none for the tooltip. It is a separate OS window,
outside `page` capture. The ten existing baselines must not change; the two
`panes-popover-*.png` from 2G are **deleted**, since the surface they cover no
longer exists.

**Rust:** two new commands, thin enough that the tests are the frontend's. The
suite must stay at 357 passing plus whatever the commands add.

## 7. Risks

- **Focus stealing on macOS.** `focused(false)` at creation does not guarantee a
  window never takes focus later. If the popup activates, the main window loses
  its focus ring mid-sentence — the exact thing 2G's Escape rule was written to
  protect. Verify by hand; if it happens, the fallback is a non-activating panel,
  which is platform-specific work.
- **The keep rule needs the cursor position while the cursor is over neither
  window.** ta-old tracked `WM_MOUSEMOVE` globally. Two webviews each see only
  their own moves, and the gap between them is exactly where the rule matters.
  This is the highest-uncertainty item in the phase and should be proven before
  the rest is built.
- **Scale factor.** Mixing physical and logical pixels is invisible at 1× and
  doubles every offset at 2×. Development is on a Retina display, which surfaces
  it early — but a 1× external monitor is where it would hide.
- **Two webviews means two bundles.** The popup page must not import `main.ts`,
  or it will run the whole app a second time — including the clipboard handshake.
- **Scope creep into Phase 3.** The fence is unchanged: this phase touches the
  tooltip, never the pane.

## 8. Constraints inherited

- **No frontend framework**; vanilla TypeScript and DOM APIs only
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`** —
  the colouriser emits spans and sets their text, and this is the rule it is most
  tempting to break
- **Every colour on bare `:root` first**; `@media` / `[data-theme]` may only
  redefine
- **Only `transform` and `opacity` animated**; `prefers-reduced-motion` respected,
  with the override after the base rule
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **`npx tsc --noEmit` clean**, `npm run build` clean, clippy clean
- **`ta-old/` is read-only**

## 9. Deferred: multi-spelling headwords

ta-old renders `旅立つ; 旅だつ【たびだつ】` — every kanji spelling JMdict lists for
the entry. This app cannot: `EntryData` (`crates/jparser/src/index/mod.rs:87`)
stores `readings` and `senses` but no `<keb>` list, and `Entry.headword` is a
single `String` holding the matched surface rather than the entry's spelling set.

Adding it means a new field on `EntryData`, an index format version bump, and a
forced rebuild of every existing user's dictionary. 2F built the download and
rebuild UI, so the path exists — but it is a real cost to impose, and it buys one
line of the tooltip.

Recorded as a phase of its own so it stays a decision rather than an omission.
