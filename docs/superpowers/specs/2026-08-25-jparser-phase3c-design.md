# JParser Phase 3C — Font Sizes & Density Design

**Date:** 2026-08-25
**Status:** Implemented. Written after the fact — see §8.
**Reference:** `docs/superpowers/specs/2026-08-12-jparser-port-design.md` (§7.7, §8, §11)

---

## 1. Goal

Bind the typography tokens to persisted settings so a user can rescale the
reading surface without touching a stylesheet: the sentence pane, the definition
text (main window and tooltip webview), and the furigana over the chips. This is
the last reading aid of Phase 3, and 4D depends on it — a layout slot captures
font sizes among the state it restores.

---

## 2. What the master spec asked for, and what shipped instead

§7.7 names `normalFontSize` and `furiganaFontSize` (ta-old's JParser config)
bound to `--text-cjk` and `--text-furigana`. Neither token exists in this port:

| §7.7 says | Reality in `src/styles/tokens.css` |
| --- | --- |
| `--text-cjk` | `--text-sentence: 24px` — the sentence pane's size |
| `--text-furigana` | Undefined. `global.css` reads it only as `var(--text-furigana, 0.58em)`, an em ratio of the sentence, with `var(--text-furigana-romaji, 0.52em)` beside it |
| — | `--text-gloss: 14px`, the definition and tooltip size, unmentioned by §7.7 |

Three decisions follow, all deliberate departures:

1. **Three controls, not two.** `--text-gloss` gets one too. The right pane is
   where the reading actually happens; a port that rescales the Japanese and
   leaves the English fixed solves half the problem.
2. **Furigana stays a ratio.** It is persisted as a percentage of the sentence
   and applied as an `em`, matching what `global.css` already does. An absolute
   px furigana would fall out of proportion every time the sentence size moved.
3. **`--text-tag` and `--text-reading` get no control.** They are chrome, not
   reading surface. Two more inputs for metadata labels is density nobody asked
   for.

---

## 3. Data contract

Three optional keys, mirroring the gloss filters of 3B in both languages:

| Key | Unit | Range | Token(s) |
| --- | --- | --- | --- |
| `font_sentence` | px | 12–48 | `--text-sentence` |
| `font_gloss` | px | 10–28 | `--text-gloss` |
| `furigana_scale` | % of sentence | 30–100 | `--text-furigana`, `--text-furigana-romaji` |

`Option<u32>` in Rust with `skip_serializing_if = "Option::is_none"`, `number |
undefined` in TypeScript. **Absent means "the stylesheet decides."** No default
is restated in Rust or TypeScript — `tokens.css` stays the single place a
default lives, which is also what makes clearing a box expressible.

`--text-furigana-romaji` is derived, not persisted: 0.9 × the furigana ratio,
from the two stylesheet defaults it replaces (`0.52em` against `0.58em`). The
maths runs as a whole number over a whole number, because `58 * 0.9 / 100` lands
on a float whose shortest representation is not the decimal that should reach
the stylesheet.

---

## 4. Architecture & data flow

```
src/
├── font-sizes.ts             # NEW. FONT_LIMITS, clampFont, fontVars — pure
├── font-sizes.test.ts        # NEW. Unit tests for clamping and unit conversion
├── types.ts                  # Settings gains the three keys
├── main.ts                   # applyFontSizes writes the tokens; sends gloss onward
├── popover.ts                # Applies the payload's gloss before it renders
├── popover.test.ts           # NEW. First test file for the tooltip webview entry
├── settings.ts               # "Text Size" group, three number inputs
└── styles/settings.css       # .settings-number-row
src-tauri/src/
├── settings.rs               # Three Option<u32> keys + round-trip tests
└── commands.rs               # save_settings assigns them
```

`font-sizes.ts` is a separate module rather than three functions inside
`main.ts` because `main.ts` is already past 1000 lines, and because `popover.ts`
needs the same clamp in a different webview.

### 4.1 Two webviews, two roots

The main window writes the tokens onto `document.documentElement`. That write
cannot reach the tooltip, which is a second webview with its own `:root`. So the
gloss size travels with the content:

```
main.ts   emit('popover-content', { entries, filters, gloss? })
popover.ts   apply --text-gloss  →  renderTooltip  →  measure  →  emit('popover-measured')
```

Order matters and is the reason the size rides the payload rather than arriving
through a `settings-changed` listener: the popover window sizes itself off what
that render measures, so a size applied afterwards would size the window to the
old text. `gloss` is omitted when nothing is persisted, and a bare `Entry[]`
payload still renders — the union in `popover.ts` keeps both older shapes.

### 4.2 Clearing a size

An emptied input is a real user action meaning "give me the stylesheet default
back," and it has to survive three hops:

1. `settings.ts` sends the key as `undefined`, so it leaves the snapshot.
2. `save_settings` **assigns** the three sizes instead of merging them the way
   it merges the toggles. A merge cannot express "unset," so the old value would
   return on the next launch. Safe because the settings window is the only
   caller and always sends a full snapshot of all three.
3. `fontVars` returns an empty value for an unset size, and `applyFontSizes`
   turns that into `removeProperty` — otherwise the last value stays stuck on
   the root until a restart.

### 4.3 Clamping

`clampFont` pins to the table in §3, rounds fractional values, and returns
`null` for anything that is not a finite number. It runs at three points:

- `settings.ts`, on change — `min`/`max` bound the steppers but not typing or a
  paste, and an unclamped value would be what gets persisted.
- `main.ts` and `popover.ts`, on apply — the settings file is hand-editable, so
  every value arriving from it is untrusted input.

Rust does not clamp. It stores what it is given; the frontend is the single
choke point every value passes through on its way to a stylesheet.

---

## 5. UI

A "Text Size" group in the settings window, between Furigana Display and Window
& Behavior:

| Label | Control | Unit shown |
| --- | --- | --- |
| Sentence | `<input type="number" id="setting-font-sentence" min="12" max="48" step="1">` | px |
| Definitions | `<input type="number" id="setting-font-gloss" min="10" max="28" step="1">` | px |
| Furigana | `<input type="number" id="setting-furigana-scale" min="30" max="100" step="1">` | % of sentence |

Number inputs rather than sliders or S/M/L presets: native, keyboard-reachable,
the exact value visible, and no styling needed to sit beside the existing
checkboxes. The `min`/`max` attributes come from `FONT_LIMITS`, so the bounds
are stated once.

Applying a size is a token write only. Nothing re-renders — the ruby, the chips
and the definition pane all reflow themselves off one style write, which is what
§7.7 meant by "two inputs rescale coherently with no layout special-casing."

---

## 6. Verification

| Layer | Coverage |
| --- | --- |
| `src/font-sizes.test.ts` | Unit conversion, the 0.9 romaji ratio, clamping, non-finite input, the empty-value clear contract |
| `src/main.test.ts` | Tokens written on load and on `settings-changed`; a dropped size clears its token; a hand-edited out-of-range value clamps; the gloss size rides the popover payload and stops when cleared |
| `src/popover.test.ts` | The payload's gloss reaches `--text-gloss` before the render; clamped; absent leaves the stylesheet default; a bare array payload still renders |
| `src/settings.test.ts` | Inputs initialize from `get_settings`, save and broadcast on change, pin a typed-past-the-limit value, send nothing for an emptied box, mirror an incoming event |
| `src-tauri/src/settings.rs` | Round-trip through the file; an unset size is absent from the JSON |
| `src-tauri/src/commands.rs` | `save_settings` persists all three; clears one the payload omits |

Result at implementation: 159 Vitest tests and 72 Rust tests green, `tsc
--noEmit` clean, `cargo clippy` clean but for four pre-existing Win32 acronym
warnings.

`e2e/popover.spec.ts:72` fails on Windows, and did so before this phase — the
same failure reproduces with all `src` and `src-tauri` changes stashed. Out of
scope here; it belongs with the Windows work of the titlebar test plan.

---

## 7. Deferred

- A master scale that moves all four tokens at once.
- Controls for `--text-tag` and `--text-reading`.
- A reset-to-default button. Clearing a box already does it; a button is worth
  adding only if clearing turns out not to be discoverable.
- Per-window overrides.

---

## 8. Note on process

This document was written after the implementation, not before it. The work was
classified as a bounded change — the settings pipeline built in 3A and 3B was
already there to extend — so it ran through an in-chat design and straight into
TDD, with no spec or plan file. Every phase from 1A to 3B has a spec, and §11 of
the master spec lists 3C alongside them, so the record is reconstructed here to
match. The design below is what was approved in chat; §4.2 is the one thing that
changed during implementation, and it changed because merging made a cleared box
unclearable.
