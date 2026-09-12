# Engine v2 Phase 3b — handoff

**Date:** 2026-09-12
**Design:** [`specs/2026-09-11-engine-v2-phase3b-design.md`](specs/2026-09-11-engine-v2-phase3b-design.md)
**Plan:** [`plans/2026-09-11-engine-v2-phase3b.md`](plans/2026-09-11-engine-v2-phase3b.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Five commits stand between the plan (`deb2994`) and this phase's last
implementation commit (`e37a408`). Four carry the work, one per task; the
fifth, `1257a2d`, is a controller bookkeeping commit that corrected a test
count in the plan's own prose after Task 2 reported the miscount rather than
quietly adjusting to it.

`0ddb7c7` gives `KanjiData` the four `Option` fields — `grade: Option<u8>`,
`strokes: Option<u8>`, `freq: Option<u16>`, `jlpt: Option<u8>` — which changes
`kanji.bin`'s record layout, so `INDEX_FORMAT_VERSION` goes 5 → 6. The header
keeps its 24-byte layout, which is what lets a v6 loader read a v5 file's
version before refusing it with `VersionMismatch`, the error `ensure_dictionary`
rebuilds on; `a_v5_header_is_refused_with_a_version_mismatch` hand-encodes that
header and pins the refusal, beside the v4 test Phase 3a left. The parser left
the fields `None` until the next commit.

`9de5bf3` fills them. The streaming reader in `kanjidic.rs` takes `<grade>`,
the first `<stroke_count>` (later ones are the DTD's recorded common
miscounts), `<freq>` and `<jlpt>`. A number that does not parse into its
field's type marks the character bad and it is skipped and counted at
`</character>`, through the path a two-character `<literal>` already took, so
the build report surfaces it and nothing is silently `None`. The `End` arm's
two jobs moved into `Pending::take` and `Pending::finish`, which left `next()`
shorter than it was — see §3. The fixture `kanjidic_mini.xml` gained 都's real
block, with its two stroke counts and a variant to ignore, and a trimmed block
for 食; `tests/fixtures/README.md` records what the fixture now carries.

`24d3cbf` takes the fields across the wire. The `engine_v2` wire pin asserts
all four by name and asserts that an absent one is a present key holding
`null` rather than a missing key. On the webview side `KanjiInfo` gains them
as `number | null` optionals and the boundary validator accepts each as
absent, `null` or numeric and rejects anything else.

`e37a408` renders them. Each kanji line ends in one parenthesised aside in
KANJIDIC2's own field order — `(grade 2, 8 strokes, freq 37, old JLPT 4)`.
Absent fields are left out and an empty aside is dropped with the column.
Grade codes 8, 9 and 10 are named as the DTD defines them, a single stroke is
singular, and the JLPT number is labelled as the former four-level scale
rather than mapped to N-levels. Because the aside is a `(…)` span,
`colourLine` paints it as a paren run and neither the colouriser nor the
stylesheet changed.

Thirteen files in all: on the Rust side `index/mod.rs`, `kanjidic.rs`, the
`engine_v2` and `index_roundtrip` tests, the KANJIDIC fixture and its README;
on the webview side `types.ts`, `kanji-lookup.ts` and its test,
`render/tooltip-text.ts` and its test, and `render/tooltip-colour.test.ts`;
plus the one-line plan correction. **396 insertions against 56 deletions**
(`git diff --stat deb2994..e37a408`). Nothing under `crates/jparser/src/engine/`
appears in the range at all, and neither do `index/build.rs`, `index/load.rs`,
`src-tauri/` or any stylesheet.

## 2. The gates

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
(5,402 entries, 9,930 keys, 13,898 records — Phase 3a's figures to the digit);
Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.
Release build of `jparser-cli` with `--features mecab`.

### Gate 1 — the baseline cells, both hint states

Measured, hints off:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.7342  (1588 / 2163)
entry top-1:    0.9093  (1123 / 1235)
  by decision:  e 0.7342 (1588 / 2163)
span tolerant:  0.7924  (1714 / 2163)
  coarsening:   126 gold spans inside 62 dictionary merges
compounds:      0.5860  (109 / 186)
  kept whole:   44 / 106     split apart: 65 / 80
  by class:     lexical 46 / 112  false-match 24 / 32  transparent 39 / 42   (8 ambiguous, excluded)
```

Measured, hints on:

```
sentences:      150
gold spans:     2163
superseded:     0
span accuracy:  0.8174  (1768 / 2163)
entry top-1:    0.9092  (1201 / 1321)
  by decision:  e 0.8174 (1768 / 2163)
span tolerant:  0.8525  (1844 / 2163)
  coarsening:   76 gold spans inside 38 dictionary merges
compounds:      0.6129  (114 / 186)
  kept whole:   36 / 106     split apart: 78 / 80
  by class:     lexical 42 / 112  false-match 30 / 32  transparent 42 / 42   (8 ambiguous, excluded)
```

Against the required cells, cell by cell:

| cell | required | measured |
| --- | --- | --- |
| strict, hints on | 0.8174 (1768) | 0.8174 (1768) |
| tolerant, hints on | 0.8525 (1844) | 0.8525 (1844) |
| entry top-1, hints on | 0.9092 | 0.9092 |
| compounds, hints on | 0.6129 (114 / 186) | 0.6129 (114 / 186) |
| `lexical`, hints on | 42 / 112 | 42 / 112 |
| `false-match`, hints on | 30 / 32 | 30 / 32 |
| `transparent`, hints on | 42 / 42 | 42 / 42 |
| strict, hints off | 0.7342 (1588) | 0.7342 (1588) |
| tolerant, hints off | 0.7924 (1714) | 0.7924 (1714) |
| entry top-1, hints off | 0.9093 | 0.9093 |
| compounds, hints off | 0.5860 (109 / 186) | 0.5860 (109 / 186) |

Every cell identical. That is the expected result rather than a lucky one:
this phase edited nothing the parse reads.

**And the cells still do not depend on `kanji.bin`'s presence, now that the
record inside it is a different shape.** The gold index was built a second
time *with* the fixture KANJIDIC2 —

```
entries:             5402
skipped entries:     0
keys:                9930
records:             13898
kanji:               4
skipped kanji:       0
```

— and the hints-on `score` re-run against it. The two outputs were diffed
rather than compared by eye:

```
$ diff "$S/score_plain.txt" "$S/score_kanji.txt" && echo identical
identical
```

### Gate 2 — the bench scaling cell, read as a band

```
--- hints off ---
index:            gold_index
hints:            off
readings:         3
runs:             5

     chars   median ms      min      max
      1005          58       56       59
     10040         583      581      695

scaling:          1.00x   (linear 1.00x, quadratic 9.99x)

--- hints on ---
index:            gold_index
hints:            on
readings:         3
runs:             5

     chars   median ms      min      max
      1005          47       45       56
     10040         506      497      551

scaling:          1.07x   (linear 1.00x, quadratic 9.99x)
```

The recorded values are 1.05x hints off and 1.10x hints on, and this plan's
Global Constraints read both as a band rather than a digit: the hints-off cell
measured 0.98x–1.04x across four runs of identical code in Phase 3a, and a
4 ms swing on the 1,005-character median moves the ratio by about 0.07x. The
run above sits inside that spread on both cells — 1.00x against 1.05x, and
1.07x against 1.10x, each within a single-millisecond wobble of the short
median.

Two further samples were taken of each state, not to find a better number but
to show the spread the band describes: **hints off 1.04x and 1.07x; hints on
1.12x and 1.08x.** The recorded values sit inside both sets, and the hints-on
cell straddles 1.10x directly. The deterministic gate over the same code —
Gate 1's `score` cells — is identical to the digit.

### Gate 3 — the snapshot

`cargo test -p jparser --test engine_v2`: 38 passed, including
`v2_snapshots_every_sentence`. `git status --short crates/jparser/tests/snapshots`
prints nothing — no `*.snap.new`, so `parse_sentences.txt` is byte-identical.

### Gate 4 — the suites

| suite | before (`deb2994`) | after (`e37a408`) |
| --- | --- | --- |
| `cargo test -p jparser --features mecab` | 367 | 373 |
| `cargo test -p jparser --features hints-source` | 368 | 374 |
| `cargo test --workspace` | 601 passed, 1 ignored | 605 passed, 1 ignored |
| `src-tauri` (a workspace member, inside that total) | 152 | 152 |
| `npm test` | 302 | 312 |
| `npx playwright test` | 45 | 45 |

The first four "before" figures are Task 1 Step 1's re-take at `deb2994`,
which is why they differ from the Phase 3a handoff's: five review commits
landed after `9bf74ef`. Task 1 re-took only the three Rust suites (workspace,
`mecab`, `hints-source`) at that commit and touched no TypeScript, so **the
`npm test` and Playwright "before" figures are the Phase 3a handoff's at
`9bf74ef`**, not a measurement taken for this phase; the npm total at
`deb2994` itself was never measured. Task 3 added one Vitest test
(`src/kanji-lookup.test.ts`); Task 4 added seven (six in
`src/render/tooltip-text.test.ts`, one in `src/render/tooltip-colour.test.ts`)
— eight in the range, reaching the recorded after-total of 312 without an
own-phase base to count them from.

The `mecab` and workspace "after" figures both check out against a listing
of the test binaries: `mecab` lists 373 tests, and workspace lists 605
passed plus 1 ignored as 606 — matching each recorded total. None of the six
new Rust tests (five in `kanjidic.rs`, one in `index_roundtrip.rs`) is
feature-gated, and the workspace build already pulls in `mecab` through
`src-tauri`'s `features = ["mecab"]`, so both suites should have gained six
from those six tests alone. `mecab` did (367 to 373); the workspace total did
not (601 to 605, a gain of four) — the recorded `deb2994` "before" of 601 is
unreconciled by two, and the discrepancy is recorded here rather than
explained.

Clean, with no output worth quoting: `cargo clippy --workspace --all-targets
-- -D warnings`, the same with `-p jparser --features mecab` and with
`--features hints-source`, `cargo check -p jparser --no-default-features
--all-targets`, `cargo fmt --all --check`, `npx tsc --noEmit`, and
`cargo +1.85 check -p jparser -p jmdict-source` — the MSRV floor still holds
for the two library crates.

Coverage, read per file rather than per crate, after `cargo llvm-cov clean
--workspace` and with no `mismatched data` warning in the run:

```
Filename                         Lines      Missed Lines     Cover
kanjidic.rs                        246                 9    96.34%
index\build.rs                      48                 1    97.92%
index\load.rs                      157                21    86.62%
engine\build.rs                     97                 1    98.97%
```

All four are above the 80% floor, `index/load.rs` by the smallest margin and
at exactly the figure Phase 3a recorded — it was not touched. `kanjidic.rs`
grew from 190 executable lines to 246 by this table's count and its coverage
rose from 95.26% to 96.34%, so the new parsing is better covered than the file
it joined. The clean was run first
rather than as a remedy, because Phase 3a's §2 records a stale profile
producing a 22-point error in the direction of failure.

### Gate 5 — Playwright

`npx playwright test`: **45 passed, 0 failed.** No baseline moved. This phase
changed no first-run copy and no screen this suite pictures — the two
screenshot failures Phase 3a had to diagnose were caused by a sentence that
phase rewrote, and nothing here rewrote one.

## 3. Where the implementation departs from the design

**Nothing in the implementation contradicts the design.** No field, rule,
placement or exclusion in §1–§6 was traded away. What follows is the phase's
rulings — five decisions taken during execution that the design or plan did
not settle, each with its reason.

1. **Work proceeded directly on `feat/engine-v2`, with no worktree.** Phases
   0 through 3a all executed on this branch in this checkout, the user asked
   that the branch stay open, and a worktree would have hidden the user's own
   uncommitted edits from the checkout they watch. The cost if it were the
   wrong call is a rebase nobody needs.
2. **Every implementer staged only the files its task named**, never
   `git add -A`. The working tree carries the user's uncommitted edits to
   `README.md` and `src-tauri/Cargo.toml`, which are theirs and are not part
   of any commit here. The cost is one amend if a task ever misses a file.
3. **Task 1's `KanjiData` doc comment says `KanjiInfo` names eight fields and
   that the wire test pins them, which was true only after Task 3.** Accepted
   as written: the comment describes the phase's end state, Task 3 landed two
   commits later on the same unmerged branch, and softening it then
   re-hardening it is churn. The cost is one intermediate commit carrying a
   forward-looking comment; Task 3's review confirmed the statement holds now.
4. **A plan defect was reported rather than absorbed.** The plan's Task 2
   Step 5 said "fourteen / nine existing" where the counts were fifteen and
   ten. The implementer reported it instead of adjusting the work to match,
   and the controller corrected the plan's prose in `1257a2d` — bookkeeping,
   not a code fix.
5. **Task 3's commit subject carries no scope**, `feat: the four kanji misc
   fields cross the wire`. The repository's git-workflow rule gives the form
   as `<type>: <description>` with the scope optional, and that commit spans
   both the Rust wire pin and the webview, so no single scope fits. The cost
   is one scope-less subject in the log.

Two claims the plan made about its own effects were checked rather than
assumed, since both could have gone the other way:

- **`next()` really did come in shorter.** `kanjidic.rs`'s
  `Iterator::next` was about 107 lines before this phase — Phase 3a's handoff
  §5 carried its length as a deferred minor — and is **75 lines** now, counted
  from `fn next` to its closing brace. Moving the `End` arm's two jobs into
  `Pending::take` and `Pending::finish` took more out than the four new fields
  put in, so a file that grew from 319 lines to 436 has a shorter hot
  function than it started with.
- **No TypeScript literal needed the new fields.** Because the four are
  declared optional, every existing `KanjiInfo` literal in the suites stayed
  valid: across `kanji-lookup.test.ts`, `render/tooltip-text.test.ts` and
  `render/tooltip-colour.test.ts` the whole phase's diff contains **no deleted
  line at all**. Every front-end test change is an addition.

## 4. The upgrade path, proved headlessly

The design's claim is that an existing install refuses its v5 index and
rebuilds once, from archives it already holds. That was verified headlessly on
a scratch copy of the real install; **`%APPDATA%\dev.jparser.clipyomi` was
never written to**, and its source directory's two archives still carry their
original timestamps.

First, the real KANJIDIC2 — 13,108 characters, not the four-character fixture
— decompressed into scratch beside the real JMdict and built:

```
$ time jparser-cli build-index .../JMdict_e.xml .../full_index --kanjidic .../kanjidic2.xml
entries:             218449
skipped entries:     0
keys:                451430
records:             498294
kanji:               13108
skipped kanji:       0

real	0m1.748s
```

**`kanji: 13108`, `skipped kanji: 0`** — every character in the file parsed,
and not one was skipped for a `<misc>` number that would not parse into its
field's type. The skip path exists and is tested on a fixture; the real file
never takes it.

```
.../full_index:
  entries.bin   36604387
  entries.idx    2621396
  header.bin          24
  kanji.bin      1390617
  keys.fst       5287278
  records.bin   19837366
```

`kanji.bin` is 1,390,617 bytes against Phase 3a's 1,314,846 — **75,771 bytes
for four numbers across 13,108 characters, under six bytes each**, and still
well inside the 2 MB the format budgeted. `header.bin` is still 24 bytes,
which is the design's whole upgrade mechanism seen on disk.

Then the refusal. The installed generation is
`%APPDATA%\dev.jparser.clipyomi\dict\gen-2`, whose `header.bin` begins with
the bytes of `5u32` — it was rebuilt to format 5 by Phase 3a on 2026-09-11.
Copied into scratch and opened by the v6 binary:

```
$ jparser-cli lookup .../upgrade/dict/gen-2 東
Error: VersionMismatch { found: 5, expected: 6 }
```

Found 5, expected 6, exactly as the design requires and as
`a_v5_header_is_refused_with_a_version_mismatch` pins in the suite.

Then the rebuild, pointed at the real source directory read-only:

```
$ time jparser-cli ensure-dictionary .../upgrade/dict --source-dir %APPDATA%/dev.jparser.clipyomi/source
generation: gen-3
entries:    218449
kanji:      13108

real	0m1.853s
```

**Under two seconds and no download**: both `JMdict_e.gz` and
`kanjidic2.xml.gz` were already in the source directory and were reused —
`resolve_from` fetches only when the target file does not exist, and the two
archives' timestamps (16 August and 11 September) are unchanged after the run.
`gen-2` was left in place beside the new `gen-3`, as generation handling
intends, and `gen-3`'s `kanji.bin` is 1,390,617 bytes, the same file the
direct build produced. The same `lookup .../gen-3 東` that was refused against
`gen-2` returns its records. That is the whole cost a user pays on first
launch after this phase, and it is smaller than Phase 3a's 3.9 seconds because
this one downloads nothing.

**The GUI check was the user's, not the agent's, as it was for Phase 3a, and
the user performed it on 2026-09-12** after the final review's fix wave
(`ba5820f`): hovering 東京都 in the running app shows the three asides —
`(grade 2, 8 strokes, freq 37, old JLPT 4)`, `(grade 2, 8 strokes, freq 74,
old JLPT 3)` and `(grade 3, 11 strokes, freq 123, old JLPT 3)` — and Settings'
"Hide kanji readings & meanings" hides them. The agent's own proof of the same
path stayed headless, above.

## 5. Open items

Carried from design §7:

- **The former JLPT scale is rendered as the former scale.** A reader who
  wants N-levels gets the DTD's own hedge — old level 2 is now divided between
  N2 and N3 — and nothing firmer, because nothing firmer exists. If an
  official N-level kanji list is ever published, mapping becomes a data
  question again rather than a declined one.
- **Frequency is a newspaper rank**, and the DTD says its discrimination among
  rarer characters is not strong. `freq 2401` and `freq 2501` mean about the
  same thing; the line neither pretends otherwise nor buckets the number into
  words.
- **No separate toggle for the aside.** If readers who want readings and
  meanings without the numbers turn out to exist, a fifth gloss filter is the
  shape Phase 3a's `hide_kanji` already has.
- **Existing installs pay one rebuild, the second in two phases.** §4 measures
  it at 1.9 seconds with no download. A third format bump in short order would
  be worth batching with whatever else is pending.
- **The README's feature list never gained the kanji block** in Phase 3a and
  does not gain the aside here. It is the user's file to shape and was
  carrying uncommitted edits throughout this phase.
- **Kun readings still keep KANJIDIC2's marks** (`ひと.つ`, `ひと-`), carried
  from Phase 3a §7 unchanged. The aside makes the line longer, which is a
  reason to revisit that, not a reason to have done it here.
- **`src-tauri/src/commands.rs` is 2,719 lines** and this phase did not touch
  it, so the kanji command's move to its own module stays where the
  whole-branch review left it.

Still open from the Phase 3a handoff §5:

- **The `-darwin` pair of the first-run download-offer baselines is owed a
  refresh on a Mac.** `panes-download-offer-light-darwin.png` and
  `panes-download-offer-dark-darwin.png` still picture the old one-file
  sentence. Their `-win32` counterparts were regenerated in Phase 3a and the
  suite is green here, so this surfaces the first time the suite runs on
  macOS, as two screenshot failures to regenerate rather than investigate.
- **A JMdict file saved under the KANJIDIC name, with its `<?xml` declaration
  intact, passes the prolog sniff** and would build a kanji-less index while
  reporting success. Unchanged by this phase, which touched neither
  `jmdict-source` nor the sniff.
- **The bench scaling cell wanted a band rather than a digit, and now has
  one.** This plan's Global Constraints read both cells as bands, and §2 is
  the first gate run to record them that way. The item is closed as far as the
  gate's definition goes; what remains is that the cell is still a ratio of two
  wall-clock medians and will keep moving by more than its own last digit.

Surfaced by this phase:

- **The band is needed on the hints-on cell too, not only hints-off.** Three
  runs here gave 1.07x, 1.12x and 1.08x around a recorded 1.10x. Phase 3a
  characterised the noise on the hints-off cell alone and matched 1.10x exactly
  once; that match was luck, not stability.

Deferred minors, each raised and accepted during review:

- A malformed *first* `<stroke_count>` lets the second one into `strokes`.
  Harmless, because the malformed first also marks the character bad and the
  whole record is discarded, but it wants a half-line comment saying so.
- A malformed *later* `<stroke_count>` is never parsed at all, so unlike
  grade, freq and jlpt it cannot mark the character bad. That asymmetry is what
  the brief asked for; it too wants a comment.
- `Event::Empty` falls through the match, so `<grade/>` reads as `None` while
  `<grade></grade>` skips the character. The shape predates this phase and the
  real file writes neither form.
- The fixture's 都 carries `freq 123` and `jlpt 3` that no test asserts.
- No test puts a `<misc>` number before `<literal>`; the parser is
  order-independent by construction rather than by assertion.
- The new doc prose in `kanjidic.rs` uses ASCII `--` where the module's own
  doc comment uses `—`. Written that way because the brief gave the text
  verbatim.
- The wire validator's test rejects only a string in `grade`; a boolean,
  object or array in the other three is untested, which `isOptionalNumber`
  makes trivially safe.
- No test covers the column combination of readings present, meanings absent
  and aside present.
- A negative or fractional misc number arriving from the wire renders raw
  (`freq -1`). The render layer is not the validation boundary, and the
  parser's own types make it unreachable from a real index.
