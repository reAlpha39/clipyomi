# Engine v2 Phase 3c — handoff

**Date:** 2026-09-12
**Design:** [`specs/2026-09-12-engine-v2-phase3c-design.md`](specs/2026-09-12-engine-v2-phase3c-design.md)
**Plan:** [`plans/2026-09-12-engine-v2-phase3c.md`](plans/2026-09-12-engine-v2-phase3c.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Three commits stand between the plan (`009b427`) and this phase's last
implementation commit (`1e91ebb`), one per task. The phase closes the three
loose ends Phases 3a and 3b left around the kanji block and does nothing
else; the three changes share no code, which is why they could be written in
any order.

`b0e1e1a` takes the affix hyphen out of kun readings at render time.
KANJIDIC2 marks a reading used only as a suffix or a prefix with an ASCII
hyphen — `-ぐ.い`, `ひと-` — and marks the okurigana boundary with a dot,
`た.べる`. A new module-private `kunReading` in `src/render/tooltip-text.ts`
trims the hyphen from both ends and `kanjiLine` maps every kun reading
through it before the join; the dot is left alone, because it shows where the
kanji stops and the reader's own kana begin. Nothing else sees the change:
`KanjiData`, `kanji.bin` and the wire keep KANJIDIC2's notation, and the on
readings, which never carry the mark, are not touched. Three files,
**64 insertions against 6 deletions** — the implementation itself is two
lines and a doc comment, the rest is tests.

`aa535f1` makes the source sniff read the root element's name. `Source`
loses `prologs: &'static [&'static [u8]]`, the three-prefix rule that
accepted any file opening with an XML declaration, and gains
`root: &'static str` — `"JMdict"` and `"kanjidic2"`. `open_local` peeks a
fixed `PEEK_BYTES` (8 KiB) from the file and, when the first bytes are gzip
magic, builds a `GzDecoder` over `Cursor::new(head).chain(file)` and peeks
*that* instead, so both paths sniff real XML rather than framing. The
rewritten `begins_xml` steps over an optional BOM, whitespace, one
`<?xml … ?>` declaration and any number of comments, then requires
`<!DOCTYPE {root}` or `<{root}` followed by whitespace, `>`, `[` or `/`.
A head cut short by the peek's own size is undecided and accepts; a head cut
short by the end of the file is decided on what it holds. The refusal is
`SourceError::NotAnArchive`, unchanged in shape, with a message that now
names the file the archive should have held and quotes the decompressed
head. One file, **340 insertions against 64 deletions**, and the crate's lib
tests went 27 → 37: one test replaced by two, and nine added.

`1e91ebb` moves the dictionary cluster out of `src-tauri/src/commands.rs`.
`commands/dictionary.rs` (453 lines) now holds `DICTIONARY_STATUS_EVENT`,
the state structs the download needs, `download_dictionary`,
`needs_dictionary`, `lookup_kanji`, `kanji_info`, `describe_index_error`,
`with_fallback`, the shared download slot (`claim_download`,
`release_download`) and `reparse`, plus their eleven tests.
`commands/hints.rs` (607 lines) holds `HINTS_STATUS_EVENT`,
`set_mecab_hints`, `hints_warning`, `hints_dir`, `describe_hints_error` and
the hints test harness. `main.rs` names the new paths in six
`generate_handler!` entries and five `manage` calls, and one doc comment in
`parse.rs` follows. `commands.rs` itself is down to 1,704 lines. Five files,
**1,087 insertions against 1,038 deletions**, nearly all of it the same lines
arriving at a new address.

Across the phase's code range, `git diff --stat 009b427..1e91ebb` reads
**nine files, 1,491 insertions against 1,108 deletions**. Measured from the
design commit instead — `git diff --stat b287236..HEAD` — it reads ten files
and 2,556 insertions, the difference being the plan document's own 1,065
lines; the code figure is the one above.

**`INDEX_FORMAT_VERSION` is still 6** (`crates/jparser/src/index/mod.rs:31`)
and **no install rebuilds.** Nothing under `crates/jparser/src/engine/` or
`crates/jparser/src/index/` appears in the range at all, and neither does any
stylesheet, settings field, Tauri command name or wire shape. The record on
disk is the same record Phase 3b wrote.

## 2. The gates

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
into scratch — 5,402 entries, 9,930 keys, 13,898 records, Phase 3a's figures
to the digit and Phase 3b's unchanged:

```
entries:             5402
skipped entries:     0
keys:                9930
records:             13898
kanji:               0
skipped kanji:       0
```

Vibrato dictionary at `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.
Release build of `jparser-cli` with `--features mecab`. Every figure below was
printed in the gate session at `1e91ebb`; none is copied forward from an
implementer's report except the "before" column of the suite table, which is
labelled where it appears.

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

Every cell identical, as it should be: this phase edited nothing the parse
reads. The hints-off block's secondary lines — `kept whole: 44 / 106`,
`split apart: 65 / 80`, the 126 gold spans inside 62 merges — are identical
to Phase 3b's too, and so are their hints-on counterparts, which is a
stronger check than the eleven required cells alone.

### Gate 2 — the bench scaling cell, read as a band

```
--- hints off ---
index:            gold_index
hints:            off
readings:         3
runs:             5

     chars   median ms      min      max
      1005          58       57       62
     10040         596      592      684

scaling:          1.02x   (linear 1.00x, quadratic 9.99x)

--- hints on ---
index:            gold_index
hints:            on
readings:         3
runs:             5

     chars   median ms      min      max
      1005          46       46       48
     10040         510      501      518

scaling:          1.10x   (linear 1.00x, quadratic 9.99x)
```

The recorded values are 1.05x hints off and 1.10x hints on, and this plan's
Global Constraints read both as bands rather than digits: hints off
0.98x–1.07x, hints on 1.07x–1.12x, each measured across several runs of
identical code in earlier phases. **Hints off measured 1.02x and hints on
1.10x, both inside their band on the first run**, so no re-run was owed and
none was taken. The hints-on cell landing on 1.10x exactly is luck of the
kind Phase 3b's §5 warned about, not evidence of stability; the deterministic
gate over the same code is Gate 1, and that one is identical to the digit.

### Gate 3 — the snapshot

`cargo test -p jparser --test engine_v2`: **38 passed**, including
`v2_snapshots_every_sentence`. `git status --short crates/jparser/tests/snapshots`
prints nothing — no `*.snap.new`, so `parse_sentences.txt` is byte-identical.

### Gate 4 — the suites

| suite | before (`009b427`) | after (`1e91ebb`) |
| --- | --- | --- |
| `cargo test -p jparser --features mecab` | 373 | 373 |
| `cargo test -p jparser --features hints-source` | 374 | 374 |
| `cargo test --workspace` | 605 passed, 1 ignored | 615 passed, 1 ignored |
| `src-tauri` (a workspace member, inside that total) | 152 | 152 |
| `npm test` | 312 | 316 |
| `npx playwright test` | 45 | 45 |

The "before" column is Task 1 Step 1's re-take at `009b427`, the first
measurement this phase owns; the "after" column was measured for this
document at `1e91ebb` rather than carried over from any task report. Every
delta is accounted for:

- **`npm test` +4.** Task 1 added three tests to
  `src/render/tooltip-text.test.ts` and one to
  `src/render/tooltip-colour.test.ts`.
- **workspace +10.** Task 2's net gain in `jmdict-source`'s lib tests, 27 to
  37 — one prolog test replaced by two opening tests, and nine new ones. The
  workspace total is the sum over all 22 test binaries, and that lib is one
  of them.
- **`mecab` and `hints-source` ±0.** Both are `-p jparser`, and no test in
  that crate changed. That those two figures held while the workspace moved
  is the arithmetic working, not a coincidence.
- **`src-tauri` ±0.** Task 3 moved tests between files and added none. The
  152 is the same 152 by name, not only by count — see the listing check
  below.
- **Playwright ±0.** No first-run copy and no pictured screen changed, so no
  baseline moved; `git status --short e2e` prints nothing after the run.

The test listing across the split was re-checked here rather than taken on
trust. Task 3 captured `cargo test -p clipyomi -- --list` before its move at
`aa535f1`, with each line reduced to the bare test name so that a changed
module prefix could not hide a changed set. Diffed against the same listing
taken at the branch tip in this session:

```
$ diff "$S/tests_before.txt" "$S/tests_tip.txt" && echo identical
identical
```

152 names before, 152 after, the same 152 — no test was lost, renamed or
quietly duplicated by the move.

Clean, with no output worth quoting: `cargo clippy --workspace --all-targets
-- -D warnings`, the same with `-p jparser --features mecab` and with
`--features hints-source`, `cargo check -p jparser --no-default-features
--all-targets`, `cargo fmt --all --check`, `npx tsc --noEmit`, and
`cargo +1.85 check -p jparser -p jmdict-source` — the MSRV floor still holds
for the two library crates, which matters here because Task 2 wrote new code
in one of them.

Coverage, read per file rather than per crate, after
`cargo llvm-cov clean --workspace` and with no `mismatched data` warning in
either run. `cargo llvm-cov -p jmdict-source`:

```
Filename                      Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover    Branches   Missed Branches     Cover
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
fetch.rs                          388                23    94.07%          23                 2    91.30%         208                 7    96.63%           0                 0         -
lib.rs                            818                27    96.70%          41                 0   100.00%         359                 4    98.89%           0                 0         -
-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
TOTAL                            1206                50    95.85%          64                 2    96.88%         567                11    98.06%           0                 0         -
```

`jmdict-source/src/lib.rs` — the file Task 2 rewrote — is at **98.89% lines,
four missed out of 359**, and every one of its 41 functions is executed. The
floor is 80%, so the new sniff is not merely above it: the whole file is
close to exhaustively exercised, which is what a ten-test gain on a module
this size buys.

And `cargo llvm-cov -p jparser --features mecab`, the four rows the plan asks
after (the table's other 23 rows are omitted here, not from the run):

```
Filename                         Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover    Branches   Missed Branches     Cover
kanjidic.rs                          384                 9    97.66%          29                 1    96.55%         247                 9    96.36%           0                 0         -
index\build.rs                       114                15    86.84%           1                 0   100.00%          48                 1    97.92%           0                 0         -
index\load.rs                        245                45    81.63%          23                 7    69.57%         157                21    86.62%           0                 0         -
engine\build.rs                      152                 5    96.71%           9                 0   100.00%          97                 1    98.97%           0                 0         -
```

`index/build.rs` at 97.92%, `index/load.rs` at 86.62% and `engine/build.rs`
at 98.97% are the Phase 3b handoff's figures to the digit, which is the
expected reading for files nothing has touched since.

**`kanjidic.rs` reads 96.36% where Phase 3b recorded 96.34%, and the
difference is real rather than a stale profile.** Phase 3b measured its table
at `e37a408`; `d31b4ee`, a commit from that phase's own final review wave,
landed afterwards and rewrote `the_first_stroke_count_is_the_accepted_one` to
bind `find('都')` to a local and assert the four `<misc>` fields as one tuple
instead of two loose assertions. That is one more executable line in the file
— 247 counted here against 246 there, with the same nine missed — and
238/247 is 96.36% where 237/246 was 96.34%. No Phase 3c commit touches
`kanjidic.rs`: `git log e37a408..HEAD -- crates/jparser/src/kanjidic.rs`
names `d31b4ee` and nothing else.

### Gate 5 — Playwright

`npx playwright test`: **45 passed, 0 failed.** No baseline moved, and none
should have: this phase changed no first-run copy and no screen the suite
pictures. The kun-hyphen change is the only front-end edit in the range, and
it renders inside the popover's kanji block, which no screenshot test
captures.

## 3. Where the implementation departs from the design

**Nothing in the implementation contradicts the design.** No field, rule,
placement or exclusion in §1–§6 was traded away, and the three tasks landed
the shape the design specified. What follows is the phase's two pre-flight
rulings, and the three places execution diverged from the plan's own prose,
each with its reason.

1. **Work proceeded directly on `feat/engine-v2`, with no worktree.** Phases
   0 through 3b all executed on this branch in this checkout, the user asked
   for it in the brief and the design §6 records it, and a worktree would
   have hidden the user's own uncommitted edits from the checkout they watch.
   The cost if it were the wrong call is a rebase nobody needs.
2. **Every implementer staged only the files its task named**, never
   `git add -A`. The working tree carries the user's uncommitted edits to
   `README.md` and `src-tauri/Cargo.toml`, which are theirs and are not part
   of any commit here. The cost is one amend if a task ever misses a file;
   none did.
3. **Only one of Task 1's three new tooltip-text tests went red before the
   implementation**, where the plan's Step 3 said three. The failing one was
   `trims the affix hyphen from either end of a kun reading`, which is the
   test the change exists for; the other two pin behaviour that already held
   — the okurigana dot was never at risk, and on readings were never in the
   mapped array. They were kept as written rather than trimmed, because a
   test that pins what must *not* change earns its line count precisely when
   the change beside it is a string transformation. The colouriser test in
   the same task passed before the implementation for the same reason and by
   the same intent.
4. **Task 2 bound `begins_xml`'s tail expression to a local.** The plan gave
   the function's body verbatim, ending in
   `[doctype.as_bytes(), element.as_bytes()].into_iter().any(…)` as the tail
   expression; that does not compile. `std::array::IntoIter` has drop glue,
   so under NLL's drop-order rules the temporary iterator outlives `doctype`
   and `element`, and rustc rejects it with two `E0597`s even though nothing
   borrowed is read after `.any(…)` returns its `bool`. The implementer bound
   the expression to `let accepted = …;` and returned `accepted` on the next
   line — rustc's own suggested fix — with a comment saying why. The reviewer
   confirmed the error is genuine rather than a mistranscription. No semantic
   change; the plan's code is otherwise byte-for-byte what shipped.
5. **Task 3 pruned two imports beyond the plan's expected list.** The plan
   named `Emitter`, `std::path::{Path, PathBuf}`,
   `jparser::hints::VibratoTokenizer` and three `crate::state` items as the
   imports the move would strand, and named `AppHandle` among those expected
   to stay. The compiler found otherwise on both counts it could: the bare
   `AppHandle` in `commands.rs`'s top-level `tauri` import went, because
   every remaining use in that file spells the type out as
   `tauri::AppHandle<R>` in a generic signature and the one bare-typed
   parameter belonged to `download_dictionary`, which moved; and `Listener`
   went from the test module's import, because the hints tests were the only
   callers of `app.listen`. The plan also said the compiler arbitrates the
   final list, and it did. Both are import removals: no visibility widened,
   no body touched.

Two claims this plan made about its own effects were checked rather than
assumed, since both could have gone the other way:

- **The split really was a move.** Three independent readings agree. The test
  listing is identical across it, name for name (§2). The controller's move
  audit — a multiset comparison of every line removed from `commands.rs`
  against every line added to the two new files, kept in `move-audit.txt` —
  leaves only the enumerated edits unmatched on either side. And
  `git diff --color-moved=dimmed-zebra aa535f1..1e91ebb -- src-tauri/src`,
  read here with the moved-block colours separated from the plain ones,
  shows 1,972 line changes coloured as moved and **not one plain line inside
  a moved block**. Every plain line is on the enumerated list: the rewritten
  module doc and the two new ones, the reshaped `use` blocks, the two
  `pub mod` lines, the three `pub(super)` signatures, the one
  `push_input` → `crate::commands::push_input` call, the `hints_scratch`
  doc-and-body rewrite onto `crate::test_support::scratch`, the two SPDX
  lines, the eleven `main.rs` paths and the one in `parse.rs`. The only plain
  line that is neither a rename nor a deletion is rustfmt's: the
  `DownloadInFlight` and `HintsInFlight` `manage` calls wrapped onto three
  lines each, because `commands::dictionary::` and `commands::hints::` pushed
  them past the width limit. That is the formatter following the rename, not
  an edit to the code.
- **No import the plan expected to keep went missing beyond the two above,
  and none it expected to prune stayed.** `commands.rs`'s imports now read
  `AtomicBool, Ordering`, `Arc`, `tauri::{Manager, State, WebviewUrl,
  WebviewWindowBuilder}`, `tokio::sync::{watch, Notify}`,
  `crate::settings::{ChromeMode, Settings}` and
  `crate::state::{SettingsState, SettingsWarning, ShortcutWarning,
  StartupFailure}` — the plan's keep list minus `AppHandle`, with every name
  on its prune list gone and `VibratoTokenizer` absent from the file
  entirely.

## 4. The sniff on the real files, headlessly

The design's claim is that the root-aware sniff passes both real EDRDG
archives through the gzip branch and refuses a JMdict archive saved under the
KANJIDIC2 name. That was verified headlessly on copies;
**`%APPDATA%\dev.jparser.clipyomi` was read from and never written to**, and
its two archives still carry their original timestamps (16 August and
11 September) after the runs.

First the real pair — `JMdict_e.gz` at 10,548,458 bytes and
`kanjidic2.xml.gz` at 1,488,571, copied into scratch and built from there:

```
$ time jparser-cli ensure-dictionary .../real/dict --source-dir .../real/source
generation: gen-1
entries:    218449
kanji:      13108

real	0m1.882s
```

**218,449 entries and 13,108 kanji in 1.9 seconds, with no download.** Both
files are gzip, so both took the branch that decompresses 8 KiB and sniffs
what comes out, and both were accepted on their real content: JMdict's
`<!DOCTYPE JMdict` sits at byte 1,135 behind a declaration and the licence
comment, KANJIDIC2's at byte 39. The old three-prefix rule would have
accepted them too — the point of this run is that the stricter rule does not
reject what it must accept, and that the cost of decompressing a head is
invisible against the two seconds the build takes anyway.

Then the wrong file: the same `JMdict_e.gz` copied twice, once under its own
name and once as `kanjidic2.xml.gz`, which is exactly the case the Phase 3b
handoff §5 recorded as open.

```
$ jparser-cli ensure-dictionary .../wrong/dict --source-dir .../wrong/source
Error: Io(Custom { kind: Other, error: NotAnArchive { path: "...\\wrong\\source\\kanjidic2.xml.gz", head: "<?xml version=\\\"1.0\\\" enco", file: "kanjidic2.xml.gz" } })
exit: 1

$ ls .../wrong/dict
(empty)
```

Non-zero exit, and **no generation directory was created** — the refusal
happens inside the source closure, before any index is built, so there is no
half-written `gen-1` to clean up. Nothing was downloaded either: both names
were present in the source directory, so `resolve_from` never reached the
network, and both files are still 10,548,458 bytes afterwards.

The message is the **Debug** form because `jparser-cli`'s `main` returns
`Result<(), Box<dyn Error>>` and the runtime prints `{:?}`. The Display text
the app shows on its download screen is the one a user would read —
`… does not hold kanjidic2.xml.gz's XML, gzipped or plain — it starts with
"<?xml …"` — and it is pinned by Task 2's unit tests rather than by this run:
`a_gzipped_jmdict_archive_under_the_kanjidic_name_is_rejected` builds exactly
this case in scratch and asserts on `err.to_string()` that the message quotes
the decompressed head and names `kanjidic2.xml.gz`. Two things are visible in
the Debug output regardless: the `head` field holds XML text rather than gzip
bytes, which is the decompressed peek doing its job, and `file` names
`kanjidic2.xml.gz` rather than the JMdict the bytes actually are, which is
the message telling the user which file to replace rather than which file
they have.

**The GUI check is the user's, not the agent's**, as it was for Phases 3a and
3b: `npm run tauri dev`, hover 食, and see `ぐ.い` at the end of its kun
readings with no hyphen. Nothing above claims it was performed. The agent's
own proof of the render path is Task 1's Vitest assertions on
`assembleTooltipText`, which pin the whole line —
`食  ショク・た.べる・ぐ.い  eat` — rather than the trimming function alone.

## 5. Open items

Carried from design §7, which carried them from the Phase 3b handoff §5:

- **The former JLPT scale is rendered as the former scale.** A reader who
  wants N-levels gets the DTD's own hedge and nothing firmer, because nothing
  firmer exists. If an official N-level kanji list is ever published, mapping
  becomes a data question again rather than a declined one.
- **Frequency is a newspaper rank**, and the DTD says its discrimination
  among rarer characters is not strong. The line neither pretends otherwise
  nor buckets the number into words.
- **No separate toggle for the aside.** If readers who want readings and
  meanings without the numbers turn out to exist, a fifth gloss filter is the
  shape Phase 3a's `hide_kanji` already has.
- **The README's feature list never gained the kanji block**, and still
  advertises "pitch/romaji support" where romaji is the half that shipped and
  pitch accent is a spike that has not started. It is the user's file to
  shape and was carrying uncommitted edits throughout this phase, which is
  the second reason no task touched it.
- **The `-darwin` pair of the first-run download-offer baselines is owed a
  refresh on a Mac.** `panes-download-offer-light-darwin.png` and
  `panes-download-offer-dark-darwin.png` still picture the old one-file
  sentence. Their `-win32` counterparts were regenerated in Phase 3a and the
  suite is green here, so this surfaces the first time the suite runs on
  macOS, as two screenshot failures to regenerate rather than investigate.
- **The bench cell is still a ratio of two wall-clock medians.** Reading it
  as a band is the accommodation, not a fix; it will keep moving by more than
  its own last digit, and Gate 2 above is the second phase to record it that
  way.

Left open by this phase's own decisions:

- **`commands.rs` is still 1,704 lines after the split, 698 of them
  `win32_frameless`** — counted here from its `pub mod` line to the matching
  closing brace. That module is a single `cfg(windows)` block with one
  test and would move by cut and paste; it stays because it is Windows
  chrome rather than engine work, and because moving it would have doubled
  this phase's third task for no reader's benefit. What is left is a
  plausible shape for the file: the webview's small entry points, plus one
  large platform block.
- **The sniff accepts a file whose leading comment outruns the peek.** By
  design (§4.1 step 5), and pinned by
  `a_preamble_that_outruns_the_peek_is_accepted`: a head cut short by
  `PEEK_BYTES` is undecided and accepts, because deciding it would mean
  reading past the buffer. No real EDRDG file is shaped that way — JMdict's
  licence comment ends at byte 1,135 — and a stricter reading is more
  machinery than the case earns.
- **The two spikes are next, and are not phases.** The long-unit separation
  test can run today with no new tooling: `corpus_staging/sudachi_result.txt`
  already holds Mode C output for the 150 gold sentences and
  `compound_gold.jsonl` the 139 adjudicated types, and its answer decides
  whether compound preservation has a Phase 4 at all. The pitch-accent hunt
  has two candidates whose licences need reading first — UniDic's accent
  fields under the BSD leg of its triple licence, and Wadoku under
  CC BY-SA 3.0. Both produce a recommendation, not code.

Surfaced by this phase:

- **`kanjidic.rs`'s coverage figure moved without this phase touching the
  file.** §2 traces it to `d31b4ee`, a Phase 3b review commit that landed
  after that phase's table was measured. The lesson is narrow but worth
  writing down: a coverage figure quoted in a handoff is pinned to the commit
  it was measured at rather than to the phase, so a later phase should expect
  to explain small movements in files it never opened whenever review commits
  landed in between.
- **The hints-on bench cell matched its recorded 1.10x exactly, for the
  second time in three phases.** Phase 3b called its own exact match luck.
  This one is too — the same session's hints-off cell moved from 1.00x to
  1.02x — and the band remains the right way to read both.

Deferred minors, each raised and accepted during review:

- Task 1 has no test for a reading hyphenated at both ends, or made only of
  hyphens. The regex handles both; no such reading exists in the real file,
  which is why the case was declined rather than written.
- Task 2 has no test for a gzip stream that fails to decode at the peek. The
  spec says it surfaces as `SourceError::Io`; a truncated `gz(XML)` asserting
  `Io` would pin it.
- `begins_xml`'s `accepted` binding could go if the array were iterated with
  `.iter()` rather than `.into_iter()`, since `slice::Iter` has no drop glue.
  That variant was not compiled during review, so the working form stays,
  with its comment.
- `strip_prefix(b"<?xml")` also steps over an `<?xml-stylesheet …?>`
  processing instruction as if it were the declaration. Harmless: the root
  check still gates what follows it.
- A file exactly `PEEK_BYTES` long reports `more_follows = true` with nothing
  following. That is the specified ceiling of the `head.len() == PEEK_BYTES`
  rule rather than a defect in its implementation.
- `hints.rs`'s `HINTS_STATUS_EVENT` doc still says "A const beside
  `DICTIONARY_STATUS_EVENT`", which now lives in `dictionary.rs`. The wording
  is stale by one module; the rationale it gives is intact.
