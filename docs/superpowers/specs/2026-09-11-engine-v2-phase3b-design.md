# Engine v2 Phase 3b — the KANJIDIC misc fields

**Date:** 2026-09-11
**Parent:** Phase 3a's design, §1 ("stroke count, grade, JLPT level and
newspaper frequency rank, all of which KANJIDIC2 carries and which the parser
reads and discards so a later layer can keep them with one line") and §7; the
Phase 3a handoff §5 ("Four KANJIDIC2 fields are discarded"); master plan §7,
Phase 3, row "JLPT level | community lists | Unofficial, provenance varies".
This is that later layer, and this document is its scope.
**Status:** design written; awaiting review; implementation plan to follow.

## 1. What it does, and what it deliberately does not

Phase 3a's kanji block shows, for each distinct kanji in the hovered surface,
its readings and up to four meanings:

```text
東  トウ・ひがし  east
京  キョウ・ケイ・キン・みやこ  capital, 10**16, metropolis, ten quadrillion
都  ト・ツ・みやこ  metropolis, capital, all, everything
```

KANJIDIC2's `<misc>` element carries four more facts about each character
that the Phase 3a parser reads and throws away: the school grade it is taught
in, its stroke count, its newspaper frequency rank, and its level in the
former Japanese Language Proficiency Test. This phase keeps them and appends
them to the same line as one parenthesised aside:

```text
東  トウ・ひがし  east  (grade 2, 8 strokes, freq 37, old JLPT 4)
京  キョウ・ケイ・キン・みやこ  capital, 10**16, metropolis, ten quadrillion  (grade 2, 8 strokes, freq 74, old JLPT 3)
都  ト・ツ・みやこ  metropolis, capital, all, everything  (grade 3, 11 strokes, freq 123, old JLPT 3)
```

The asides carry the 2026-09-11 file's real values for the three characters. The
aside answers the question the readings and meanings cannot: *how much should
I expect to know this character?* Grade and the former JLPT level say where it
sits on the two learning curricula the file records; frequency says how often
running text uses it; stroke count is the number a reader needs to find it in
a paper dictionary or write it. All four are in the record Phase 3a already
parses, so they cost one parse, one format bump and one column.

Not in this layer, by decision: any field outside `<misc>` (dictionary
indices, query codes, radicals, variants); the miscount stroke totals
KANJIDIC2 lists after the accepted one; a mapping from the former four-level
JLPT scale to the current N1–N5 scale (see §1.2); a separate toggle for the
aside (it lives and dies with `hide_kanji`); a CLI surface for reading a
kanji record; and any change to the popover's layout beyond the one column.

### 1.1 Decisions taken in session

| decision | choice | why |
| --- | --- | --- |
| Which fields | all four `<misc>` numerics: grade, stroke count, frequency, JLPT | Phase 3a named these four as the deferred set; one line each to keep |
| Where they go | a fourth column on the existing kanji line, parenthesised | one block, one line per kanji, as Phase 3a promised; a second line per kanji would double the block's height for four short tokens |
| Why parentheses | `colourLine` already paints a `(…)` span as a `paren` run | the aside is coloured like a POS tag with no renderer or CSS change; a bare trailing column would be indistinguishable from a fifth meaning |
| Order inside the aside | grade, strokes, frequency, JLPT | KANJIDIC2's own `<misc>` element order |
| Absent fields | omitted; an aside with nothing in it is omitted | 10,109 of 13,108 characters have no grade, 10,607 no frequency, 10,878 no JLPT; a placeholder would be the common case |
| Grade codes 8, 9, 10 | rendered `secondary school`, `name kanji`, `name kanji` | the DTD defines 8 as the remaining jōyō kanji learned in junior high school and 9–10 as jinmeiyō; `grade 8` is a codebook value, not a school year |
| Stroke count | the first `<stroke_count>` only | the DTD: "the first is considered the accepted count, while subsequent ones are common miscounts"; 525 characters carry more than one |
| Frequency | `freq N`, the raw rank | the DTD's number is a rank 1–2,501 over a newspaper survey; there is no scale to normalise it to |
| JLPT | the raw former level, labelled `old JLPT N` | honest about what the number is; see §1.2 |
| Storage | four `Option` fields flat on `KanjiData` | the struct is already the disk and wire type; a nested struct would be a name for nothing |
| Format | `INDEX_FORMAT_VERSION` 5 → 6, `IndexHeader` untouched | the record layout changes, so a v5 `kanji.bin` must be refused; the 24-byte header is what the upgrade path depends on (3a handoff §3 item 1) |
| Malformed numbers | the character is skipped and counted, as a bad `<literal>` is today | reuses the one skip path the build already surfaces; a hard failure would take every user's first-run build down for one typo upstream |
| Wire | serde's `null` for `None`; TypeScript fields optional | bincode 1.x cannot skip fields, so the JSON carries them; optional on the TypeScript side keeps every existing fixture and an older payload valid |
| Toggle | none new | four tokens do not earn a fifth checkbox; `hide_kanji` hides the line they are on |

### 1.2 What this settles in the master plan

Master plan §7's Phase 3 table has a row "JLPT level | community lists |
Unofficial, provenance varies". KANJIDIC2's `<jlpt>` is the one JLPT signal in
the project's tree with a licence already established in writing, and this
phase ships it. What it does and does not settle should be stated exactly.

**What it settles.** The kanji half of the row. The DTD describes the field
as "the (former) Japanese Language Proficiency test level for this kanji",
1 (most advanced) to 4 (most elementary), and 2,230 of 13,108 characters
carry it. The old test published its kanji lists officially, so the
provenance is clean; the licence is KANJIDIC2's, §2.

**What it does not settle, and the decision not to pretend otherwise.** The
DTD continues: "the JLPT test levels changed in 2010, with a new 5-level
system (N1 to N5) being introduced. No official kanji lists are available for
the new levels. The new levels are regarded as being similar to the old
levels except that the old level 2 is now divided between N2 and N3." So a
modern reader seeing `JLPT 4` would read N4 and be wrong by a level (old 4 is
roughly N5), and rendering `N5` would present the publisher's "regarded as
similar" as a fact the app vouches for. The line therefore says what the
number is — `old JLPT 4` — and nothing it is not.

**The word-level half of the row is declined, not left pending.** No official
N1–N5 vocabulary list exists; every candidate is a community reconstruction,
which is what the row's "unofficial, provenance varies" already records. A
source that cannot state its own provenance cannot meet §7's "no layer ships
without its licence established in writing first", and that is not going to
change with time. At this phase's completion the row is rewritten to:

> JLPT level | KANJIDIC2 `<jlpt>`, kanji only, former four-level scale |
> **Kanji half shipped, Phase 3b (2026-09-11)**, labelled as the former
> scale. Word-level JLPT declined: no official N1–N5 list exists, so no
> source can meet the written-provenance gate.

and the "Kanji readings" row gains "Phase 3b added grade, stroke count,
frequency rank and former JLPT level to the same line."

## 2. Licence, unchanged and stated

Master plan §7: "No layer ships without its licence established in writing
first." This phase adds no file, no publisher and no licence: the four fields
are elements of the same `kanjidic2.xml.gz` that Phase 3a's design §2
established under EDRDG's CC BY-SA 4.0 statement, and the app's attribution
footer already names KANJIDIC2. Nothing in §2 or §2.1 of that design moves.

Two of the fields deserve a sentence about where their content comes from,
because "same file" is not the same as "same kind of fact". The grade is
MEXT's jōyō and jinmeiyō classification as EDRDG has encoded it; the
frequency rank is, in the DTD's words, "based on a survey in newspapers"
compiled for KANJIDIC itself. Neither introduces a third party's data into
the tree, and both are what EDRDG publishes under its own licence. The
stroke count and the former JLPT level are likewise EDRDG's own encoding of
public standards.

## 3. Index format: the same sixth file, one version later

`kanji.bin` stays a single bincode `Vec<KanjiData>` sorted by codepoint.
`KanjiData` gains four fields:

```rust
pub struct KanjiData {
    pub literal: char,
    pub on: Vec<String>,
    pub kun: Vec<String>,
    pub meanings: Vec<String>,
    /// `<grade>`: 1–6 the school year a kyōiku kanji is taught in, 8 the
    /// remaining jōyō kanji (secondary school), 9 and 10 jinmeiyō (name)
    /// kanji. `None` for the 10,109 characters the file does not grade.
    pub grade: Option<u8>,
    /// The first `<stroke_count>` — the accepted count. Later ones are
    /// common miscounts and are dropped. The file's maximum is 34.
    pub strokes: Option<u8>,
    /// `<freq>`: newspaper frequency rank, 1–2,501. `None` when unranked.
    pub freq: Option<u16>,
    /// `<jlpt>`: level in the former four-level JLPT, 1 (advanced) to 4
    /// (elementary). `None` when the character was on no level's list.
    pub jlpt: Option<u8>,
}
```

bincode encodes each `Option` as a one-byte tag plus the value, so the four
add at most nine bytes per record: about 118 KB over 13,108 characters, and
`kanji.bin` grows from 1.25 MB to roughly 1.4 MB, inside the 2 MB the Phase
3a design budgeted.

**`INDEX_FORMAT_VERSION` goes 5 → 6.** The layout of `Vec<KanjiData>`
changes, so a v5 `kanji.bin` must not be decoded by a v6 reader; `open_with`
compares the header's version before it reads `kanji.bin`, so the refusal is
`VersionMismatch` and `ensure_dictionary` rebuilds, exactly as a v4 index is
refused today. **`IndexHeader`'s layout does not change** — the 3a
handoff §3 item 1 and the comment on `INDEX_FORMAT_VERSION` explain why the
24-byte header is load-bearing, and `a_v4_header_is_refused_with_a_version_mismatch`
keeps pinning it. The version constant's doc comment gains a line for 6.

`Index::kanji(ch)` and `Index::kanji_count()` are unchanged; the new fields
ride on the record they already return.

### 3.1 The parser

`crates/jparser/src/kanjidic.rs` gains four `Field` variants — `Grade`,
`Strokes`, `Freq`, `Jlpt` — selected on `<grade>`, `<stroke_count>`,
`<freq>` and `<jlpt>` inside a `<character>`. `<misc>`'s other children
(`<variant>`, `<rad_name>`) keep falling through to `Field::None`. Three
rules:

- **The first `<stroke_count>` wins.** A second one in the same character is
  read and discarded, whatever its value.
- **A number that does not parse skips the character.** `<freq>abc</freq>`
  marks the character bad; at `</character>` a bad character is counted in
  `skipped` and not yielded, the same path a two-character `<literal>` takes
  today, so `BuildReport.skipped_kanji` and the CLI's "skipped kanji" line
  surface it with no new plumbing. A number that parses but does not fit its
  type (a grade of 300) is the same case.
- **Absence is `None`.** Most characters have no grade, frequency or JLPT
  level, and the DTD makes all three optional; `<stroke_count>` is required
  by the DTD, but the reader does not enforce that — the fixture's `京` has
  no `<misc>` at all and must still parse.

The handoff §5 records `Iterator::next` at about 107 lines. Adding four
fields to it in place would take it past 130, so the `End` arm's two jobs
are split out as it grows: a `take_field(&mut self, field, text)` that
stores one leaf's value into the pending record, and a `finish_character`
that does the literal check, the bad-character check and the yield. The
resulting `next` is shorter than today's, not longer. This is the targeted
improvement to the file the phase is working in, and nothing else in the
parser's structure moves.

### 3.2 Builders and fetch

Nothing changes. `build_from_reader`, `build_from_readers`, `build_new`,
`ensure_dictionary`, `write_index`'s signature, `jmdict-source`'s `Source`
descriptors, `resolve_kanjidic` and the CLI's `--kanjidic` are all as Phase
3a left them: the parser yields a richer record and `write_index` serialises
whatever it is given. The existing install pays one rebuild on first launch
after upgrade, and because the source directory already holds
`kanjidic2.xml.gz` beside `JMdict_e.gz`, that rebuild downloads nothing
unless the user has cleared it — the 3a handoff §4 measured the whole
rebuild at 3.9 seconds *with* the download.

Nothing under `crates/jparser/src/engine/` changes at all — not even
`build.rs`, which Phase 3a had to touch. That is what lets §6 demand the
baseline cells byte-identical with no argument.

## 4. Runtime: no new command, four wire fields, one column

### 4.1 Backend

`kanji_info` and `lookup_kanji` in `src-tauri/src/commands.rs` are
unchanged: they return `KanjiData`, which now serialises with `grade`,
`strokes`, `freq` and `jlpt` as numbers or `null`. bincode 1.x cannot
deserialise a struct whose fields were skipped on serialisation, so
`#[serde(skip_serializing_if)]` is not available and the wire carries the
`null`s. `kanji_data_serializes_to_the_documented_wire_shape` in
`tests/engine_v2.rs` extends to pin the four names and one `null`.

### 4.2 Main webview

`src/kanji-lookup.ts`'s cache and `main.ts`'s hover path do not change: the
same command, the same distinct-kanji extraction, the same payload field. Two
small things do:

- `src/types.ts`'s `KanjiInfo` gains `grade?: number | null`, `strokes?:
  number | null`, `freq?: number | null`, `jlpt?: number | null`. Optional
  rather than required so that the existing `KanjiInfo` literals in the
  test suites and a payload from an older main webview during a hot reload
  stay valid, as the 3a design §5.3 wanted for the block itself.
- `isKanjiInfo`, the boundary validator, accepts each of the four as absent,
  `null`, or a number, and rejects anything else — the e2e stub answers
  `null` for the whole command and nothing else guarantees the shape.

### 4.3 Popover

`kanjiLine` in `src/render/tooltip-text.ts` gains a fourth column, built by a
`kanjiAside(kanji)` helper that returns `''` when no field is present, so the
existing `.filter((column) => column !== '')` drops it with the others:

```text
東  トウ・ひがし  east  (grade 2, 8 strokes, freq 37, old JLPT 4)
一  イチ・イツ・ひと-・ひと.つ  one, one radical (no.1)  (grade 1, 1 stroke, freq 2, old JLPT 4)
々  repetition mark
```

The tokens, in order, each present only when its field is:

| field | rendered |
| --- | --- |
| grade 1–6 | `grade N` |
| grade 8 | `secondary school` |
| grade 9, 10 | `name kanji` |
| any other grade | `grade N`, the raw code, rather than a guess |
| strokes | `N strokes`; `1 stroke` |
| freq | `freq N` |
| jlpt | `old JLPT N` |

Because the aside is a `(…)` span, `colourLine` paints it as a `paren` run,
the colour POS tags already use, and nothing in `tooltip-colour.ts` or
`tooltip.css` changes. A line that wraps — 食 has seven readings — wraps
inside `pre-wrap` as today, and a wrapped aside stays one run. `popover.ts`
and `renderTooltip` pass the records through as they do now.

### 4.4 Settings

Nothing. The aside is part of the kanji line and `hide_kanji` hides the
line. A separate toggle is an open item (§7), not a default.

## 5. Fixture

`crates/jparser/tests/fixtures/kanjidic_mini.xml` gains `<misc>` blocks so
the four cases the parser must handle are each one character:

| character | `<misc>` | exercises |
| --- | --- | --- |
| 東 | grade 2, 8 strokes, freq 37, jlpt 4 — already present | all four fields |
| 京 | none — stays absent | every field `None`; DTD's required `<stroke_count>` not enforced |
| 都 | grade 3, strokes 11 then 10, freq 123, jlpt 3 — the real block, which has two `<stroke_count>`s | first stroke count wins; `<variant>` ignored |
| 食 | grade 2, 9 strokes only — trimmed from the real block | partial: freq and jlpt `None` |

The malformed-number case is an inline string in `kanjidic.rs`'s tests, as
the bad-literal case is today. The fixture's README entry names the added
elements; the notice and licence lines do not change.

## 6. Testing and gates

**Compatibility is the cheap part**: no builder or fetch signature moves, so
no existing test, fixture, snapshot or probe changes beyond the header
version and the fixture's own `<misc>` blocks.

Tests added or extended:

- **Parser** — the four fixture cases of §5 through `kanjidic.rs`; the first
  stroke count wins; `<freq>abc</freq>` and `<grade>300</grade>` each skip
  the character and count it; the existing tests keep passing unchanged.
- **Format** — `a_kanjidic_record_round_trips` asserts 東's four fields and
  京's four `None`s; a v5 header is refused by v6 `open` with
  `VersionMismatch`, beside the v4 test and in its shape.
- **Wire** — the field-name pin extended to `grade`, `strokes`, `freq`,
  `jlpt`, with one `None` asserted as JSON `null`.
- **Popover** — `assembleTooltipText` with all four fields; with none (no
  aside, no doubled gap); with a partial set; `1 stroke`; grades 8, 9 and 10
  by label; the aside under `hide_kanji` (gone with the line); and
  `colourLine` on a full kanji line yielding the aside as one `paren` run.
- **Lookup** — `isKanjiInfo` accepts a record with the four fields absent,
  `null` and numeric, and rejects a string in one.
- **Command** — `kanji_info`'s existing tests pass unchanged; the `src-tauri`
  fixture's 本 gains no `<misc>`, so its record serialises with four
  `null`s, which the existing assertions already tolerate.

Gates, in the order that matters, every one of them run rather than assumed:

1. **Baseline cells byte-identical, both hint states**: strict 0.8174 (1768),
   tolerant 0.8525 (1844), entry top-1 0.9092, compounds 0.6129 (114 / 186)
   with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42
   hints on; 0.7342 (1588) / 0.7924 (1714) / 0.9093 / 0.5860 (109 / 186)
   hints off. Nothing under `engine/` is edited, so any movement is a defect.
2. `jparser-cli bench` scaling cell unchanged, **read as the band the 3a
   handoff §2 and §5 established rather than a digit**: hints on 1.10x, hints
   off 1.05x, where the hints-off cell has measured 0.98x–1.04x across four
   runs of identical code and a 4 ms swing on the short median moves it by
   about 0.07x. A measurement inside that spread is a pass; outside it is a
   finding to be re-run, not explained away.
3. The `parse_sentences.txt` snapshot byte-identical: no `*.snap.new`.
4. jparser with `mecab` and with `hints-source`, workspace, src-tauri, npm,
   `tsc --noEmit`, playwright; `cargo clippy` under `-D warnings` for the
   workspace and both feature sets; `cargo check --no-default-features`;
   `cargo fmt --all --check`; `cargo +1.85 check -p jparser -p jmdict-source`;
   `cargo llvm-cov` over 80 with **`kanjidic.rs`, `index/load.rs` and
   `index/build.rs` read per-file**, after `cargo llvm-cov clean` — the 3a
   handoff §2 records a stale-profile run that reported 74% for a 97% file
   and the "mismatched data" warning that gave it away.
5. Playwright: the first-run copy does not change, so the download-offer
   baselines are untouched and the `-darwin` pair stays exactly as owed.
6. **The upgrade path, seen once on the real file**: `jparser-cli
   build-index` with `--kanjidic` over the real `kanjidic2.xml` reports
   `kanji: 13108` and `skipped kanji: 0`, and a v5 index opened by the v6
   binary is refused with `VersionMismatch { found: 5, expected: 6 }` — the
   3a handoff §4's headless proof, repeated one version on. The GUI check —
   hover 東京都 and see the three asides — is the user's, as it was for 3a.

## 7. Open items, stated rather than hidden

- **The former JLPT scale is rendered as the former scale.** A reader who
  wants N-levels gets the DTD's hedge ("regarded as being similar … old
  level 2 is now divided between N2 and N3") and nothing firmer, because
  nothing firmer exists. If an official N-level kanji list is ever
  published, mapping becomes a data question again.
- **Frequency is a newspaper rank.** The DTD says so and says the
  discrimination among rarer characters "is not strong". `freq 2401` and
  `freq 2501` mean about the same thing; the line does not pretend
  otherwise, and does not bucket the number into words either.
- **No separate toggle for the aside.** If readers who want readings and
  meanings without the numbers turn out to exist, a fifth gloss filter is
  the shape Phase 3a's `hide_kanji` already has.
- **Existing installs pay one rebuild**, the second in two phases. The
  source archives are reused, so it is a few seconds and no download; but a
  third format bump in short order would be worth batching with whatever
  else is pending.
- **The README's feature list never gained the kanji block** in Phase 3a and
  does not gain the aside here. It is the user's file to shape and is
  carrying uncommitted edits at the time of writing.
- **Kun readings still keep KANJIDIC2's marks** (`ひと.つ`, `ひと-`),
  carried from 3a §7 unchanged; the aside makes the line longer, which is a
  reason to revisit that, not a reason to do it here.
- **`src-tauri/src/commands.rs` is 2,719 lines** and this phase does not
  touch it, so the kanji command's move to its own module stays where the
  whole-branch review left it.
