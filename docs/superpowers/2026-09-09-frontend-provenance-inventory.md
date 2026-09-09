# The frontend's provenance — an inventory for the copyright holder

**Date:** 2026-09-09
**Raised by:** [Phase 1b-ii handoff §1](2026-09-02-engine-v2-phase1b-ii-handoff.md),
which surfaced the question and deliberately left it to the copyright holder
**Status:** inventory complete, no decision taken, no file edited

## 1. What this document is, and what it is not

Phase 1b-ii recorded that seven comments under `src/` cite Translation
Aggregator's GPL-2.0-only C++ by file and line, that the clean-room rebuild
covered the parsing engine only, and that the frontend tooltip renderer was
never behind that fence and never reassessed. It stopped there, correctly:
the question belongs to the author.

This document is the missing input to that decision. For each of the seven
sites it records what the comment says verbatim, what behaviour the code
around it actually implements, and **what category of thing was taken** —
because that is the axis the answer turns on, and it is a question of fact
that can be settled by reading the code.

**It is not a legal opinion and does not pretend to be one.** Whether any
category below is protected expression is the author's call, with counsel if
the author wants one. Nothing here has been edited, removed, or reworded.

## 2. The seven sites

`README.md:5` names Translation Aggregator as the project's lineage and
`README.md:103` dedicates the work to its authors. Both are deliberate
attribution and neither is in question; they are the eighth and ninth
mentions if one is counting everything.

### 2.1 `src/render/tooltip-colour.test.ts:11` — `MyToolTip.cpp:154-160`

> `// MyToolTip.cpp:154-160 — a line flagged with \x01 is drawn entirely in the`
> `// conjugation colour, marker stripped.`

**What the code does.** `tooltip-text.ts` writes `CONJ_MARKER = '\u0001'` at
the head of a conjugation line; `tooltip-colour.ts` looks for it and emits
the whole line as one `conj` run with the marker stripped.

**Category: a protocol between two of ClipYomi's own modules.** Both ends of
this contract are in this repository. The test asserts that ClipYomi's writer
and ClipYomi's reader agree. The only borrowed element is the *choice* of
`\x01` as the sentinel, which is §2.6's question, not this one.

**Independently derivable:** yes, trivially — the behaviour under test is
this codebase's own internal agreement.

### 2.2 `src/render/tooltip-colour.test.ts:41` — `MyToolTip.cpp:214-217`

> `// MyToolTip.cpp:214-217 — everything from ( to its match is one paren run.`

**What the code does.** A parenthesised span renders in one colour, so
`(v1,vi)` is a single run rather than being split by its contents.

**Category: observable rendered output.** Anyone who has seen the upstream
tooltip can state this from a screenshot; the comment's own neighbour two
tests up appeals to exactly that ("this is why 消える reads red and 【きえる】
green in the reference shot").

**Independently derivable:** yes, from observation of rendered output.

### 2.3 `src/render/tooltip-colour.test.ts:50` — `MyToolTip.cpp:224`

> `// Parenthesis colouring is tested BEFORE the Japanese check`
> `// (MyToolTip.cpp:224), so kanji inside parentheses does not win.`

**What the code does.** `(e.g. 寿司)` renders entirely paren-coloured; the
kanji inside does not take the kanji colour.

**Category: this one is mixed.** The *outcome* is observable output, like
§2.2. The *explanation* — that upstream tests one condition before the other,
at a named line — describes upstream's internal control flow, which
observation of output does not give you. It is the only site of the seven
whose wording asserts something about how upstream is written rather than
what it does.

**Independently derivable:** the outcome yes; the stated reason no. Restating
it as the outcome would need no citation.

### 2.4 `src/render/popover.ts:45` — `MyToolTip.cpp:518-523` — **production code**

> `// Fits on neither side: pin to the bottom of the work area and step`
> `// sideways, right first (MyToolTip.cpp:518-523). This is the case a small`
> `// window hits most — a tall entry on a word low on the screen.`

**What the code does.** When a tooltip fits neither below nor above the
hovered word, `placePopover` pins it to the bottom of the work area and moves
it beside the word, trying right before left.

**Category: a layout fallback policy.** Observable by hovering a low word
with a tall entry in the upstream program. The citation of source lines
suggests it was read rather than observed, which is the fact worth noting.

**Independently derivable:** yes, from observation — though whoever wrote it
did not derive it that way.

### 2.5 `src/render/popover.test.ts:28` — `MyToolTip.cpp:518-523`

> `// Neither side fits: pin to the work-area bottom and go right of the word.`
> `// This is MyToolTip.cpp:518-523, the case 2G had no answer for.`

Same citation and same policy as §2.4, restated as the test's expectation.
"The case 2G had no answer for" is an internal reference to an earlier
ClipYomi phase, not to upstream.

**Category and derivability:** as §2.4.

### 2.6 `src/render/tooltip-text.test.ts:66` — `DictionaryUtil.cpp:46`

> `// The conjugation gets its own line, marked so the colouriser can find it —`
> `// DictionaryUtil.cpp:46 sets \`temp[0] = 1\` for exactly this purpose.`

**What the code does.** `entryLines` prefixes a conjugation line with
`CONJ_MARKER`, i.e. byte value 1.

**Category: a single arbitrary constant, quoted as an upstream statement.**
This is the most literal of the seven — it reproduces the substance of one
assignment. What it encodes, though, is only "use byte 1 as an in-band
marker", and `tooltip-text.ts:5-11` already justifies that choice on its own
terms: a control character "cannot occur in dictionary text."

**Independently derivable:** the requirement yes, and this repo's own doc
comment derives it. The specific value is one of several dozen equally
serviceable control bytes.

### 2.7 `src/styles/tooltip.css:50` — `MyDrawText`'s `xOffset = 10`

> `/* Wrapped continuations hang by 10px, matching MyDrawText's \`xOffset = 10\`,`
> `   so a long sense reads as a column rather than as a paragraph. */`

**What the code does.** A 10px hanging indent on wrapped tooltip lines.

**Category: a single numeric constant.** No algorithm, no structure.

**Independently derivable:** the *intent* (hanging indent so senses read as a
column) is observable; the exact 10 is a match to upstream's value.

## 3. The summary that matters

Across all seven: **nothing reproduces upstream code, structure, or
algorithmic expression.** What the citations record is observable rendered
behaviour (§2.2, §2.3-outcome, §2.4, §2.5), one internal protocol of this
codebase's own (§2.1), and two arbitrary constants (§2.6, §2.7). One site
(§2.3) additionally states something about upstream's internal ordering.

The `colourLine` implementation those tests specify was already rewritten
clean-room at `c827ef2` — the implementer worked only from
`tooltip-colour.test.ts`, was forbidden from reading the previous
implementation or any upstream source, and confirmed that constraint held.

**And that is the sharpest structural point in this document.** That clean
room used as its specification a test file whose expected values cite GPL-2.0
source line ranges. If those expectations are observable behaviour, the chain
is clean and the citations are honest attribution. If any of them is
protected expression, then the clean room laundered it rather than avoiding
it. The chain is only as clean as the weakest of the seven, which is why the
categories above are the deliverable and not a summary verdict.

## 4. What the relicence currently rests on

- **No file under `src/` carries a licence header.** 33 TypeScript and CSS
  files, zero SPDX identifiers. The frontend is declared `MIT OR Apache-2.0`
  by `package.json`'s `license` field together with `README.md` and the
  `LICENSE-MIT`/`LICENSE-APACHE` pair — not by any per-file assertion.
- **The engine, by contrast, asserts it per file.** Every file under
  `crates/jparser/src/engine/` opens with "Written for ClipYomi without
  reference to Translation Aggregator's JParser", and the measurement harness
  files carry "Nothing here derives from Translation Aggregator." 47 of 48
  `.rs` files under `crates/` carry SPDX.
- **The citations are contained.** Nothing under `crates/`, `src-tauri/src/`,
  `e2e/` or `tools/` cites upstream code; every mention there is a negative
  assertion or the harness's own attribution.
- **Three files outside `src/` lack SPDX** and are worth closing while anyone
  is in the area: `src-tauri/src/mouse_tracker.rs`,
  `src-tauri/src/popover.rs`, `crates/jparser/examples/boundary_diag.rs`.

## 5. Two problems, needing different answers

**(a) Substance.** Is anything actually derived? §3 is the factual input; the
verdict is the author's.

**(b) Auditability.** Even if (a) is entirely clean, the artifact *reads*
derived: a `MIT OR Apache-2.0` declaration sitting above files that cite
GPL-2.0-only source by line, with no per-file assertion anywhere under `src/`
to say otherwise. A downstream distributor or a packager reviewing provenance
sees the citations, not the reasoning. (b) is worth fixing whatever the
answer to (a) is.

## 6. Options

Ordered by what they assume about (a).

1. **Close (b) only, assuming nothing about (a): add SPDX headers to all 33
   `src/` files** and to the three stragglers in §4, and give the frontend
   files the same per-file provenance line the engine carries — worded to
   match what is actually true of each, which for the tooltip renderer is
   *not* "written without reference to". Leaves every citation in place.
   Cheap, honest, and removes the "declared by `package.json` alone"
   weakness.
2. **If (a) is judged clean: restate the citations as behaviour.** Each
   comment says what the code does and why, and attributes the idea to
   Translation Aggregator without a file-and-line pointer that reads as a
   derivation trail. This *removes information* — that the behaviour was
   checked against upstream — so it is only honest if (a) really is clean.
   If it is not, this is laundering, and doing it is worse than leaving the
   citations alone.
3. **If any site is judged not clean: clean-room that site**, the `c827ef2`
   method, but with the specification written from observed behaviour rather
   than from upstream line numbers — which is the flaw in the chain §3
   describes. §2.4 and §2.5 (`placePopover`'s fallback) are the only
   production-code candidates; the rest are tests and constants.
4. **Ask counsel**, if the author wants the question settled rather than
   judged. §2 is the document to hand over; it is organised by category for
   that reason.

## 7. Recommendation

Do option 1 regardless — it is unconditional, it is a day's mechanical work
at most, and it converts the frontend's licence status from an inference into
an assertion. Then decide (a) with §2 in hand, and let that answer pick
between 2 and 3.

What not to do is merge the branch with (b) open. The relicence is the whole
purpose of Engine v2 (master plan §1); shipping a `MIT OR Apache-2.0`
artifact whose frontend has no per-file licence assertion and seven
unexplained GPL-2.0 citations gives away, at the last step, the thing four
phases were built to secure.

## 8. Status — option 1, partially applied 2026-09-09

Twenty-nine files now carry `SPDX-License-Identifier: MIT OR Apache-2.0`:
every file under `src/` except the seven below, plus the three stragglers §4
named (`src-tauri/src/mouse_tracker.rs`, `src-tauri/src/popover.rs`,
`crates/jparser/examples/boundary_diag.rs`). `npx tsc --noEmit` clean,
`npm test` 279 passed, `cargo test --manifest-path src-tauri/Cargo.toml` 149
passed, clippy and `cargo fmt` clean.

**Seven files were deliberately left unstamped**, and this is a departure
from §6 option 1 as written. Option 1 proposed stamping all 33 `src/` files
on the reasoning that the header assumes nothing about §5(a). That is wrong
for these seven: stamping a file `MIT OR Apache-2.0` *is* an assertion about
its licence, which is precisely the question §5(a) leaves open. So they wait
for the answer rather than pre-empting it.

| File | Why held |
| --- | --- |
| `src/render/tooltip-colour.test.ts` | carries §2.1, §2.2, §2.3 |
| `src/render/tooltip-colour.ts` | the implementation those citations specify; rewritten clean-room at `c827ef2` *from* that test file — the chain §3 describes |
| `src/render/tooltip-text.test.ts` | carries §2.6 |
| `src/render/tooltip-text.ts` | writes the `\u0001` marker §2.6 cites |
| `src/render/popover.ts` | carries §2.4 — production code |
| `src/render/popover.test.ts` | carries §2.5 |
| `src/styles/tooltip.css` | carries §2.7 |

`src/render/tooltip.ts` was stamped: it imports both modules but composes
them, and its own content is this project's.

So the frontend's licence is now a per-file assertion everywhere its
provenance is not in question, and the seven files where it *is* in question
are the only ones still resting on `package.json` alone — which is a more
honest position than either stamping them or leaving all 33 unmarked.
