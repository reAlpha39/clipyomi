# Engine v2 Phase 2b — handoff

**Date:** 2026-09-09
**Design:** [`specs/2026-09-09-engine-v2-phase2b-design.md`](specs/2026-09-09-engine-v2-phase2b-design.md)
**Plan:** [`plans/2026-09-09-engine-v2-phase2b.md`](plans/2026-09-09-engine-v2-phase2b.md)
**Branch:** `feat/engine-v2`

## 1. What landed before the gate

One commit, re-measured at `--readings 1` against Phase 1b-i handoff §2's
eight cells and byte-identical to them: `lattice::demote`, the reorder
between `undominated` and the cut to k (design §3.2), over the raw pool the
shipped k = 3 already holds — cap 12, no DP widening. `k_best`'s signature,
`parse`, `score` and the wire shape are unchanged; `ParseResult::alternatives`'s
doc comment now says rank is not cost order alone.

## 2. The gate measurement, over the existing pool

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`;
Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

Hints off:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7272  (1573 / 2163)
entry top-1:    0.9075  (1108 / 1221)
  by decision:  e 0.7272 (1573 / 2163)
span tolerant:  0.7975  (1725 / 2163)
  coarsening:   152 gold spans inside 75 dictionary merges
oracle@3 strict:    0.7628  (1650 / 2163)
oracle@3 tolerant:  0.8206  (1775 / 2163)
oracle@3 sentences: 0.0067  (1 / 150)
```

Hints on:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8031  (1737 / 2163)
entry top-1:    0.9063  (1170 / 1291)
  by decision:  e 0.8031 (1737 / 2163)
span tolerant:  0.8521  (1843 / 2163)
  coarsening:   106 gold spans inside 53 dictionary merges
oracle@3 strict:    0.8632  (1867 / 2163)
oracle@3 tolerant:  0.8821  (1908 / 2163)
oracle@3 sentences: 0.1533  (23 / 150)
```

Phase 2 recorded, at the same `--readings 3` before this phase: hints on
oracle@3 strict 1846, tolerant 1883, sentences 25; hints off 1629, 1749, 1.

## 3. Verdict

Design §8 clause 1, which is Phase 2 design §8 clause 1 unchanged: oracle@3
tolerant hints on must be at least 1,898 of 2,163, 55 spans over top-1's
1,843. Measured over the pool k = 3 already holds: **1,908 of 2,163, 1,908 −
1,843 = 65 spans.** The gate **holds** at this pool.

Strict oracle@3 hints on: 1,867 of 2,163 (Phase 2: 1,846). Whole-sentence gold
cover in the top three: 23 of 150 (Phase 2: 25). Neither is thresholded
(design §8 clause 3); both are recorded here rather than averaged away. This
on-engine figure, 25 → 23, is a deeper regression than the off-engine spike's
25 → 24; the spike ran a wider reading pool, which is why.

## 4. What landed after the gate

`src-tauri/src/parse.rs` asks for three readings; `src/main.ts` renders the
chosen one, offers the others as `Reading n of 3` radio rows in the
right-click menu, steps through them with `]` and `[`, and keeps the
sentence pane's scroll position across a reading switch and a furigana
re-render (design §6). Commit `4cbfd50`.

## 5. Standing gates at the phase's final code commit

Run at `4cbfd50`: `cargo test -p jparser` (290 passed), the same with
`--features mecab` (316 passed, 1 ignored), both clippy invocations clean,
`cargo check -p jparser --no-default-features` clean, `cargo fmt -p jparser
-- --check` clean, `npm test` (278 passed), `npx tsc --noEmit` clean,
`cargo test --manifest-path src-tauri/Cargo.toml` (149 passed). Eight cells
byte-identical at `--readings 1`, per §1 above — not re-run here, since no
crate code has changed since and the comparison needs a release build and the
46 MB dictionary.

## 6. Open items

- **The product question is now live, not hypothetical.** `くるまで` against
  `くる` + `まで` is exactly the relation `demote` sinks (design §13). The
  order that ships is the one the gate can see; whether it is the one a
  reader wants is what the UI now makes observable for the first time. The
  whole-sentence cover count in §3 is the measured footprint so far.
- **k stays at 3 on the wire**, so the sunk merge/split variants do not reach
  the reader at all. Raising k for the app while the gate stays at 3 is the
  named follow-up once there is UI evidence to argue from (design §13).
- **The scroll guard covers furigana re-renders too**, because both paths
  re-render `lastResult`. If that turns out to change furigana behaviour
  someone relies on, the guard narrows to the reading-switch path
  (design §13).
- The 9 gold spans with no candidate and the 3 with the span but not the
  entry (Phase 2 handoff §6) are unchanged by this phase and remain lexicon
  or index findings.
- ~~**A reading switch lost keyboard focus.**~~ **FIXED 2026-09-09.** The
  scroll guard this phase added was written with only half the problem in
  view: `replaceChildren` on `#output` drops focus to `<body>` the instant
  the focused child leaves the document, so `]`/`[` — a *keyboard* gesture —
  threw away the keyboard position of the reader who pressed it, on every
  press. Scroll survived; focus did not. `renderDictionary` already carried
  the pattern (read whether focus was inside *before* mutating, restore
  after, gated so a background render steals nothing) and `display` now uses
  it. Focus returns to the chip covering the **character** the reader was on
  rather than the chip at the same ordinal, because another reading segments
  the sentence differently — reading 2 splits `東京都` into `東京` + `都`, and
  offset 2 is `都` where index 0 is not. `#output` gained `tabindex="-1"`, for
  the reason `#dictionary` has it: somewhere to land when the offset now
  falls inside an unmatched run, which is a `<span>` and deliberately not in
  the tab order. Four tests; two of them fail against the pre-fix code with
  `expected <body> to be <div id="output">`, which is the defect stated
  exactly.

  Worth recording as a process note, not just a fix: this defect was written
  down in a session note and **not** in the repository, so success criterion
  4 — "every deliberate shortcut is recorded, not discovered later" — was
  satisfied only by luck. A known defect belongs in this list on the day it
  is known.
