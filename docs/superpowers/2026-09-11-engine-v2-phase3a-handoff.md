# Engine v2 Phase 3a — handoff

**Date:** 2026-09-11
**Design:** [`specs/2026-09-09-engine-v2-phase3a-design.md`](specs/2026-09-09-engine-v2-phase3a-design.md)
**Plan:** [`plans/2026-09-11-engine-v2-phase3a.md`](plans/2026-09-11-engine-v2-phase3a.md)
**Branch:** `feat/engine-v2`

## 1. What landed

Nine commits stand between the plan (`29131dc`) and this handoff's own commit,
which cannot name itself. Eight carry the layer; one (`e23dc03`) is a rustfmt
chore on `examples/derivation_probe.rs`, whose drift predates this plan and was
cleared early so the `cargo fmt --all --check` gate was real from Task 2 on.

`7cd025d` adds `crates/jparser/src/kanjidic.rs`, a streaming KANJIDIC2 reader
built on the same `quick-xml` pattern as `jmdict.rs`. It yields `KanjiData
{ literal, on, kun, meanings }` — `ja_on` and `ja_kun` readings kept,
everything else dropped, English meanings only (`m_lang`-tagged ones
discarded) and capped at `MAX_MEANINGS = 4`, `<nanori>` ignored. A
four-character fixture, `tests/fixtures/kanjidic_mini.xml`, comes with it.

`a59b871` makes `kanji.bin` the sixth index file: a bincode `Vec<KanjiData>`
sorted by codepoint that `Index::open` reads whole and `Index::kanji(ch)`
binary-searches. `INDEX_FORMAT_VERSION` goes 4 → 5. `build_from_reader` keeps
its signature as a wrapper over the new `build_from_readers`, so every
existing fixture, test, snapshot and probe builds unchanged.

`b640983` generalises `jmdict-source` over a `Source` descriptor — `JMDICT`
and `KANJIDIC` — so `open_local`, `resolve_from`, `fetch_from` and
`fetch_with_retry` all take the source as a first argument and the prolog
sniff, the size cap and the three error variants carry the file they are
talking about.

`b48042e` threads the source pair through every build: `build_from_readers`,
`build_new(root, jmdict, kanjidic)`, `ensure_dictionary`'s lazy closure
returning both archives, and the CLI's `--kanjidic` on `build-index` and
`ensure-dictionary`, with `fetch-source` now fetching both.

`3f290e9` adds `lookup_kanji` and the `kanji_info` Tauri command, and pins
`KanjiData`'s field names in the `engine_v2` wire test.

`b574a8b` renders the block: `assembleTooltipText` and `renderTooltip` take a
`kanji` array and append one flat line per character at the foot of the
popover, after a blank line, so `colourLine` colours it as it colours
everything else.

`3ccf096` has the main webview collect the distinct kanji of the hovered
surface, ask `kanji_info` for the ones a page-lifetime cache has not seen, and
send the answer beside the entries in the same `popover-content` payload. The
new `src/kanji-lookup.ts` holds the extraction and the cache. `kanji` is
omitted when empty, so a kana-only hover emits the payload it emitted before.
The first-run message names both files.

`9bf74ef` adds `hide_kanji` as the fourth gloss filter — settings checkbox,
TypeScript key, Rust field, `merge_saved` — and the settings window's
attribution footer, the app's first persistent attribution surface.

Forty files changed in total, 1,739 insertions against 226 deletions.

## 2. The gates

Gold index rebuilt from `crates/jparser/tests/fixtures/gold_jmdict_subset.xml`
(5,402 entries, 9,930 keys, 13,898 records); Vibrato dictionary at
`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`. Release build of
`jparser-cli` with `--features mecab`.

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

Every cell identical. The engine was not touched: nothing under
`crates/jparser/src/engine/` other than `build.rs` appears in the range's
diff, and `build.rs`'s change is the `build_from_reader` → `build_from_readers`
split.

**The cells do not depend on `kanji.bin`'s presence either.** The gold index
was built a second time *with* the fixture KANJIDIC2 —

```
entries:             5402
skipped entries:     0
keys:                9930
records:             13898
kanji:               4
skipped kanji:       0
```

— and the hints-on `score` re-run against it. The two outputs were diffed
rather than compared by eye: `diff` reports no difference. The parse never
reads `kanji.bin`, and the measurement now says so directly.

### Gate 2 — the bench scaling cell

```
--- hints off ---
     chars   median ms      min      max
      1005          56       55       59
     10040         584      579      696
scaling:          1.04x   (linear 1.00x, quadratic 9.99x)

--- hints on ---
     chars   median ms      min      max
      1005          46       45       49
     10040         510      501      571
scaling:          1.10x   (linear 1.00x, quadratic 9.99x)
```

Hints on is 1.10x, the recorded value exactly. **Hints off read 1.04x against
a recorded 1.05x, and that difference is timer noise, not movement.** The
hints-off bench was run three further times and returned 0.98x, 0.98x and
1.03x; across the four runs the 1,005-character median moved between 56 and
60 ms, and a 4 ms swing on that median alone moves the ratio by about 0.07x.
Every observation is at or below the recorded 1.05x — that is, on the
sub-linear side, which is the direction this gate exists to permit rather than
the direction it watches for. The deterministic gate over the same code, the
`score` cells above, is identical to the digit.

### Gate 3 — the snapshot

`cargo test -p jparser --test engine_v2`: 38 passed, including
`v2_snapshots_every_sentence`. `git status --short crates/jparser/tests/snapshots`
prints nothing — no `*.snap.new`, so `parse_sentences.txt` is byte-identical.

### Gate 4 — the suites

| suite | at `29131dc` | at `9bf74ef` |
| --- | --- | --- |
| `cargo test -p jparser --features mecab` | 346 | 366 |
| `cargo test -p jparser --features hints-source` | — | 367 |
| `cargo test --workspace` | 569 passed, 1 ignored | 598 passed, 1 ignored |
| `src-tauri` (a workspace member, inside that total) | — | 152 |
| `npm test` | 282 | 302 |

Clean, with no output worth quoting: `cargo clippy --workspace --all-targets
-- -D warnings`, the same with `-p jparser --features mecab` and with
`--features hints-source`, `cargo check -p jparser --no-default-features
--all-targets`, `cargo fmt --all --check`, `npx tsc --noEmit`, and
`cargo +1.85 check -p jparser -p jmdict-source` — the MSRV floor still holds
for the two library crates.

Coverage, read per file rather than per crate, as design §6 gate 4 requires:

```
Filename                         Lines      Missed Lines     Cover
kanjidic.rs                        190                 9    95.26%
index\build.rs                      48                 1    97.92%
index\load.rs                      157                21    86.62%
engine\build.rs                     97                 1    98.97%

jmdict-source lib.rs               203                 0   100.00%
jmdict-source fetch.rs             208                 7    96.63%
```

All six are above the 80% floor, `index/load.rs` by the smallest margin.

**One methodological note, because it nearly produced a false failure.** The
first `cargo llvm-cov -p jmdict-source` run reported `fetch.rs` at 74.22%
lines and emitted `warning: 12 functions have mismatched data`. That warning
was the tell: the profile data was stale from earlier builds in this session.
After `cargo llvm-cov clean --workspace` the same command reports 96.63%, with
only lines 168–170, 173–175 and 198 uncovered. A 22-point error in the
direction of failure is worth knowing about — **treat a "mismatched data"
warning as invalidating the table, not as a footnote.** The `jparser` run was
re-taken after the clean and is unchanged, so its numbers were never affected.

### Gate 5 — Playwright

`npx playwright test`: **43 passed, 2 failed.** Both failures are
`toHaveScreenshot` comparisons, and both are in `panes.spec.ts`:

```
  2 failed
    e2e\panes.spec.ts:295:3 › the download screen's offer renders correctly in light
    e2e\panes.spec.ts:295:3 › the download screen's offer renders correctly in dark
```

```
974 pixels (ratio 0.01 of all image pixels) are different.
Snapshot: panes-download-offer-dark.png
Expected: e2e\panes.spec.ts-snapshots\panes-download-offer-dark-win32.png
```

**These are stale baselines, and this plan is what made them stale — they are
not an environment artifact.** The baselines compared against are the `-win32`
ones, captured on this platform, so the usual macOS/Windows pixel caveat does
not apply. The failing screen is the first-run download offer, and `3ccf096`
deliberately rewrote its one sentence:

```
- 'No dictionary yet. JMdict is a one-time download of roughly ten megabytes from EDRDG.'
+ 'No dictionary yet. JMdict and KANJIDIC2 are a one-time download of roughly twelve megabytes from EDRDG.'
```

974 pixels is one line of text. The test's one DOM assertion — that
`#download` is visible — passes in both themes; the failure is at line 304,
the `toHaveScreenshot` call after it. The neighbouring
`panes-download-downloading-*` screenshots pass, because that screen's copy
did not change.

The two `-win32` baselines therefore need refreshing, and the two `-darwin`
ones need refreshing on a macOS machine, which this session could not do. That
is left open in §5 rather than half-done here: regenerating only the Windows
pair would leave the platform pair inconsistent, and this task was scoped to
change no code.

## 3. Where the implementation departs from the design

Six places. Five contradict something the design says; the sixth is a
placement the design left open and the plan pinned.

1. **`IndexHeader`'s layout does not change**, against spec §3's "`IndexHeader`
   gains `kanji: u32`". Only `INDEX_FORMAT_VERSION` moves, 4 → 5. bincode 1.x
   encodes the header as fixed-width fields — four `u32` and a `u64`, 24 bytes
   — so a header with a sixth field would fail to *decode* a v4 file with an
   `Encoding` error before `open_with` ever compared versions, and
   `ensure_dictionary` would surface that error instead of rebuilding. That is
   precisely the upgrade path §3 relies on, so the field was dropped and the
   record count is read off the vector by `Index::kanji_count()`. Pinned by
   `a_v4_header_is_refused_with_a_version_mismatch`, which hand-encodes the
   24-byte v4 layout and asserts `VersionMismatch`. The live proof in §4
   confirms the header is still 24 bytes on disk.
2. **`KanjiData` is the wire type**, against spec §5.2's "mirrors" — there is
   no separate Rust `KanjiInfo`. `KanjiData` already derives `Serialize` for
   bincode and serialises to exactly the JSON the webview wants, so a second
   struct would be a copy to keep in step for no gain. The TypeScript interface
   still carries the spec's name, `KanjiInfo`, and the `engine_v2` wire test
   pins `KanjiData`'s field names against it.
3. **`ensure_dictionary` takes one closure returning `(jmdict, kanjidic)`**,
   not two. Spec §3.2 asks for a pair; one closure keeps the app's
   "downloading" / "building" status split exactly where it already was.
4. **The popover payload omits `kanji` when the array is empty.** A hover on a
   kana-only word, or on any index built without KANJIDIC2, emits byte for byte
   the payload it emitted before this phase. Every existing `popover-content`
   assertion stayed valid but for the new `hide_kanji: false` key in `filters`.
5. **The attribution footer names the licence URL as text, not a hyperlink**,
   against spec §2.1's "linking". No opener plugin is installed, and a plain
   `<a href>` in a Tauri 2 webview navigates the settings webview itself away
   from the page. Adding `tauri-plugin-opener` for one link was a dependency
   this plan declined to take; the footer text is selectable, so the URL can be
   copied.
6. **The meanings cap lives in the parser**, `kanjidic::MAX_MEANINGS = 4`, in
   one place. The design names the cap and its test but not its home; putting
   it at the parser keeps the four-meaning rule where the data is first read
   rather than at each consumer.

Two further departures were ruled during execution rather than planned:

- **The KANJIDIC prolog-sniff test changed shape (Task 3).** The plan's
  `a_jmdict_file_under_the_kanjidic_name_is_rejected` fed
  `<?xml ...?><JMdict>` to the KANJIDIC check, but `<?xml` is a listed prolog
  for *both* sources, so the sniff accepts it — a defect in the plan, not the
  implementation. The prolog lists and `begins_xml` were kept as the spec
  wrote them (the sniff was never root-aware, and making it root-aware would
  mean skipping JMdict's multi-kilobyte header comment inside a single
  `fill_buf`), and the test became the case the lists *do* distinguish: a
  hand-trimmed JMdict copy starting at `<JMdict>` under the KANJIDIC name,
  renamed `a_trimmed_jmdict_file_under_the_kanjidic_name_is_rejected`. The
  cost is recorded in §5.
- **The real GUI launch was replaced by a headless proof of the same path.**
  See §4.

## 4. The upgrade path, proved headlessly

**The design's §3 claim is that an existing install rebuilds once through the
path that already exists.** Plan Step 3 asked for that to be seen in a running
app — `npm run tauri dev`, hover 東京都, toggle the fourth checkbox. That needs
a human at a GUI and was not performed. **The hover, the three lines at the
foot of the popover, and the settings checkbox hiding them on the next hover
remain unverified by eye and are left for the user.** What was verified is the
upgrade path itself, headlessly, on a scratch copy of the real install —
`%APPDATA%\dev.jparser.clipyomi` was never written to.

The copied `dict/gen-1` is a pre-Phase-3a index built 2026-08-16: five files,
a 24-byte `header.bin`, no `kanji.bin`. Opened directly by a format-5 binary
it is refused, exactly as the design requires:

```
$ jparser-cli lookup .../upgrade/dict/gen-1 東京都
Error: VersionMismatch { found: 3, expected: 5 }
```

**Found 3, not the 4 the plan expected** — the installed index on this machine
predates format 4 as well. The refusal is the same either way, since `open`
compares against 5 and rebuilds on any mismatch; the format-4 case specifically
is covered by `a_v4_header_is_refused_with_a_version_mismatch` rather than by
this run.

`ensure-dictionary` then rebuilt it, with the source directory holding only
`JMdict_e.gz`:

```
$ time jparser-cli ensure-dictionary .../upgrade/dict --source-dir .../upgrade/source
generation: gen-2
entries:    218449
kanji:      13108

real	0m3.852s
```

218,449 entries is the full dictionary; 13,108 is the character count of the
2026-09-11 KANJIDIC2. **3.9 seconds, download included** — the whole upgrade a
user pays for on first launch after this phase.

The artefacts agree with the claim:

```
.../upgrade/dict/gen-2:            .../upgrade/source:
  entries.bin   36604387             JMdict_e.gz      10548458   (reused)
  entries.idx    2621396             kanjidic2.xml.gz  1488571   (downloaded)
  header.bin          24
  kanji.bin      1314846
  keys.fst       5287278
  records.bin   19837366
```

Six files where there were five. `kanji.bin` is 1.25 MB, well under the 2 MB
the format budgeted. `header.bin` is still 24 bytes, which is deviation 1 of
§3 seen on disk. `kanjidic2.xml.gz` (1.42 MB) was fetched from
`http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz` into the source directory
beside `JMdict_e.gz`, which was reused rather than re-downloaded. `gen-1` was
left in place, as generation handling intends.

That the block's data is really in the index is what `kanji: 13108` says, and
`jparser-cli lookup .../gen-2 東京都` returns records for 東, 東京 and so on
from the same generation.

## 5. Open items

Carried from design §7:

- **Kun readings keep KANJIDIC2's marks** (`た.べる`, `-がた`), rendered raw.
  A deliberate first cut: the marks inform a reader who knows them and are
  harmless to one who does not. Stripping or styling them is a popover-text
  question, not a data one.
- **Four KANJIDIC2 fields are discarded** — grade, stroke count, JLPT level,
  frequency — each about one line to keep. The `jlpt` field would partly cover
  the master plan's separate "JLPT level" row, whose community sources that
  plan calls unofficial; that remains a separate decision.
- **Attribution sits in the settings footer** because it is the only persistent
  surface the app has. If an About surface ever exists, it moves there.
- **Existing installs pay one rebuild**, now with a second download. The
  design asked that the first-run screen name what it is fetching; `3ccf096`
  did that, and §4 measured the rebuild at 3.9 seconds. The remaining cost is
  the two stale screenshot baselines that copy change left behind, below.

Surfaced by this phase:

- **Two Playwright screenshot baselines are stale** —
  `panes-download-offer-light` and `panes-download-offer-dark` — because the
  first-run sentence changed. The `-win32` pair can be regenerated here; the
  `-darwin` pair needs a macOS machine, so neither was touched. Until both are
  refreshed `npx playwright test` reports 2 failed locally. Full detail in §2.
- **A JMdict file saved under the KANJIDIC name, with its XML declaration
  intact, passes the prolog sniff** and would build a kanji-less index while
  reporting success. This is the cost of the Task 3 ruling in §3, and it is the
  same class of miss the JMdict-side check already tolerates for any
  `<?xml`-led file. A root-aware sniff would close it and would have to skip
  JMdict's multi-kilobyte header comment inside one `fill_buf` to do so.
- **The hints-off bench scaling cell is noise-dominated at this precision.**
  Four runs in one session gave 1.04x, 0.98x, 0.98x and 1.03x against a
  recorded 1.05x. The cell is a ratio of two wall-clock medians and a few
  milliseconds on the short one moves it by more than the gate's own
  resolution. If it is to stay a gate it wants a band, not a digit.
- **The GUI has not been seen.** §4's proof is of the upgrade path, not of the
  rendered block, the colouring, or the checkbox.

Deferred minors, each raised and accepted during review:

- `kanjidic.rs`'s `Iterator::next` is about 107 lines.
- `reading_field` and `meaning_field` fold a `try_get_attribute` error into
  `Field::None` rather than surfacing it.
- No test covers `write_index`'s duplicate-literal dedup path.
- `fetch_from`'s doc comment names only `fetch` as its production caller;
  `fetch_kanjidic` is one too.
- `fetch_kanjidic` has no direct test, as `fetch` never had one either.
- `download_and_verify`'s doc dropped the doc-link for the size cap.
- `lookup_kanji`'s order test has one real hit; two hits with a miss between
  would need a second fixture character.
- `src-tauri/src/commands.rs` is 2,714 lines. Pre-existing, and this phase
  added to it.
- No test covers a kanji with readings but empty meanings, and the readings
  run carries no `.tt-kana` assertion.
- No `main.ts`-level test asserts that `hide_kanji` skips the `kanji_info`
  round trip.
- `hide_kanji` carries a doc comment its three sibling settings fields lack.
