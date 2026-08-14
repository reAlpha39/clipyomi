# JParser Phase 2G — Hover-to-Preview Popover (Design)

2F made the app usable without a terminal. 2G restores the **glance**: checking one
unfamiliar word without spending a click and then finding its row in the pane.

**Reference:** `docs/superpowers/specs/2026-08-14-jparser-phase2e-design.md` §6.2
schedules this phase and fixes its scope; port design
`docs/superpowers/specs/2026-08-12-jparser-port-design.md` §7.2 is the decision it
amends. `ta-old/` is read-only, and its behaviour here is
`FuriganaWindow.cpp:716-730` (`TME_HOVER`, `dwHoverTime = 350`) with `MyToolTip.cpp`.

**Predecessor code:** the shipped 2F tree at `bc5e38d`.

## 1. Scope

**In scope:**

- A hover popover showing one entry for the word under the cursor, after a dwell
- The same popover on keyboard focus, without a dwell
- Dismissal on every path that would leave it stale or unwanted
- Viewport-aware placement that never clips off-screen

**Not in scope:**

- **Pane density, font sizes, gloss filters, furigana modes.** Phase 3. §6.2 is
  explicit: the moment 2G pulls these in it has become Phase 3 under another name.
  The popover inherits the existing `--text-*` tokens and adds no typography
  decisions of its own.
- **Replacing the definitions pane.** This is additive. The pane remains the
  complete route to a definition, which §6.2 requires and §3.4 here relies on.
- **The collapsed alternates** (`<N> more`). They are interactive, and §3.3 makes
  the popover non-interactive on purpose.
- **Removing the manual text input.** That is Phase 2H (§6.3 of the 2E design).

### 1.1 Why hover comes back

The port replaced ta-old's hover tooltip with a permanent pane deliberately: every
definition visible at once, surviving the mouse moving away, with chips as real
`<button>`s carrying Enter/Space and a focus ring. That trade is right for reading a
whole sentence and is not being undone.

What it lost is the glance. First real use after 2E surfaced it immediately: to check
one word you click, the pane scrolls, and you find the marked row — three steps for a
question that ta-old answered by dwelling. 2G adds the glance back as a second
surface rather than reverting the first.

## 2. Architecture

`src/render/popover.ts` owns the surface. It exports:

```ts
export function showEntryPopover(chip: HTMLElement, entry: Entry): void
export function hideEntryPopover(): void
export function placePopover(
  chip: DOMRect, popover: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number }
```

One reused `<div class="entry-popover">` lives in `#app`, created on first use. It is
filled by calling `renderEntry(segment.entries[0])` — the ranked primary, the same one
the pane row shows above its collapsed alternates — exported from
`src/render/definitions.ts`,
otherwise unchanged — so the popover and the pane row render from one code path and
cannot drift. That reuse is also what keeps 2G out of Phase 3's typography: there are
no new text decisions to make.

`src/main.ts` keeps the last `ParseResult` in a module-local and resolves a chip to
its segment by `data-start`, the same key the existing click-to-mark path uses.
Unmatched runs are `<span>`s with no entries and therefore no popover, for free.

**The popover lives outside `.panes`.** `.panes` is `overflow-y: auto`
(`src/styles/global.css:58`), so a popover inside it would be clipped at the pane
edge and would scroll away from the word it describes.

**`aria-hidden="true"`.** The entry is already in the definitions pane, and
announcing it twice is worse than not announcing it. The chip keeps its own
accessible name; nothing about the popover changes what a screen reader hears.

**No Rust changes.** Every phase since 2A has touched `src-tauri`; this one does not.
The whole gate for 2G is the frontend one plus the unchanged Rust suite.

## 3. Interaction

### 3.1 Triggers

| Input | Behaviour |
|---|---|
| Mouse over a chip | Opens after `DWELL_MS` = 350, ta-old's `dwHoverTime` |
| Chip receives focus | Opens immediately, no dwell |

The dwell exists to stop the popover firing while the cursor sweeps across a
sentence. Focus has no equivalent problem — it moves only on a deliberate keypress —
so a dwell there would be a delay with nothing to prevent.

The mouse dwell is **re-armed per chip, with no sticky swap**: moving from one chip
to another hides the open popover and starts a fresh dwell. One rule rather than two,
and it is the rule the dwell was introduced to enforce.

Keyboard support is not decoration here. The chips were deliberately built as real
`<button>`s with Enter/Space activation and a focus ring, and `e2e/panes.spec.ts`
already holds the app to keyboard parity. A mouse-only glance would be the first
feature to break that pattern.

### 3.2 Dismissal

| Trigger | Why |
|---|---|
| Cursor leaves the chip | The glance is over |
| Chip loses focus | Symmetric with the focus trigger |
| `Escape` | Standard dismissal. **Focus does not move** — the chip keeps it |
| `.panes` scrolls | The stored position is now wrong |
| Window resizes | Same |
| A new `parse-result` arrives | See below |

A click **keeps** the popover open. The click also focuses the chip, so hiding would
contradict the focus rule and need a special case for no benefit; it goes away when
the pointer leaves and focus moves on.

The `parse-result` dismissal is the one with teeth: `show()` replaces `#output`
wholesale, so a popover left open would be anchored to a chip from the previous
sentence that is no longer in the document. `show()` hides the popover before it
swaps the panes.

No special case is needed for the first-run download screen: there are no chips while
it is up, so nothing can be hovered or focused.

### 3.3 The popover is not interactive

`pointer-events: none`, and this is load-bearing rather than polish. It means the
popover can never intercept the hover that opened it, so the flicker loop — popover
overlaps chip, `mouseout` fires, popover hides, `mouseover` fires, popover shows —
cannot occur at all, instead of being avoided by careful geometry.

It also settles the content question permanently: nothing inside the popover can be
clicked, expanded, scrolled, or selected. That is why the collapsed alternates stay
in the pane (§1) and why §3.4's height cap is a hard cap.

### 3.4 One accepted limit

The baseline window is 480×320. A polysemous word with many senses can render taller
than that. When it does, the popover shows its head and the remainder is only in the
pane — `pointer-events: none` makes it unscrollable, so this is a hard cap, not a
soft one.

Accepted rather than worked around: §6.2 requires the pane to remain the complete
route to a definition, and this is the case where that requirement earns its keep.

## 4. Placement

`position: fixed`, so `getBoundingClientRect()` values go in directly with no scroll
offsets to track — the other half of why the popover lives outside `.panes`.

Order of operations: fill the content, measure the popover, then place it.

- **Vertical:** preferred `top = chip.top - height - GAP` with `GAP` = 6; flips to
  `chip.bottom + GAP` when the preferred position would clip the viewport top.
- **Horizontal:** `left = chip.left`, clamped into
  `[MARGIN, viewport.width - width - MARGIN]` with `MARGIN` = 8.

Above is preferred deliberately: above the sentence is the input row, which nobody is
reading, while below it are the definition rows, which they might be.

`max-width: min(320px, 100vw - 16px)` so long gloss lists wrap instead of leaving the
clamp to do all the work, and `max-height: calc(100vh - 16px)`.

`placePopover` is a pure function of three rectangles because happy-dom returns zeros
from `getBoundingClientRect()`: geometry asserted through the DOM in Vitest would pass
regardless of what the code did. 2D established that a test which cannot fail is not
coverage.

## 5. Motion

`opacity` and a small `translateY` only, per 2D's rules, and the
`prefers-reduced-motion` override is placed **after** the base rule.

2F shipped that exact override dead — it sat before the base rule, equal specificity,
so the later rule won and reduced-motion users kept a spinning ring. No test in either
suite caught it; a human reading the cascade did.

## 6. Testing

**Vitest — `placePopover`, directly:** prefers above; flips below when the top would
clip; clamps at the left edge; clamps at the right edge.

**Vitest — the wiring, with fake timers:** a completed dwell opens it; a cursor that
leaves at 200 ms never opens it; focus opens it with no timer; `Escape` hides it and
leaves focus on the chip; a new `parse-result` hides it.

**Playwright — what only real geometry can prove:** dwell on a chip and assert the
popover's box is above it and inside the viewport; dwell on the **last** chip of a
long sentence and assert the right edge is clamped; Tab to a chip and assert the
popover appears; `Escape` and assert it is gone with focus intact.

**Baselines:** two new screenshots, light and dark at 480×320, following the file's
compact-only convention (as 2F's download-screen baselines did). They depend on
Playwright's `animations: 'disabled'`, exactly as 2F's spinner baselines already do.
The existing committed baselines must not change: the popover is absent until hovered.

**No Rust tests.** Nothing in `src-tauri` or `crates/` changes.

## 7. Risks

- **The delegated `mouseover` must ignore `.unmatched` spans.** They have no entries;
  a popover for one would be an empty box.
- **Focus churn while Tabbing** — one popover opens and closes per chip along a
  sentence. Accepted, not a defect: focus moves only on a keypress.
- **Scope creep into Phase 3.** The fence is §1: pane density, font sizes, gloss
  filters, furigana. Reusing `renderEntry` is what keeps the fence cheap to hold.
- **A stale chip reference** is the one way this can throw. §3.2's `parse-result`
  dismissal is the guard; the plan must test it.

## 8. Constraints inherited

- **No frontend framework**; vanilla TypeScript and DOM APIs only
- **Dictionary content reaches the DOM via `textContent`, never `innerHTML`** —
  `renderEntry` already obeys this and is reused rather than reimplemented
- **Every colour on bare `:root` first**; `@media` / `[data-theme]` may only redefine
- **Only `transform` and `opacity` animated**; `prefers-reduced-motion` respected,
  with the override after the base rule (§5)
- **Files 200–400 lines typical, 800 hard maximum** including tests
- **Prettier/ESLint via the project's own entrypoints**; `npx tsc --noEmit` clean
- **`ta-old/` is read-only**
