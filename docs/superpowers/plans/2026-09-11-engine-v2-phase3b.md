# Engine v2 Phase 3b — the KANJIDIC misc fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep the four `<misc>` numbers KANJIDIC2 gives each character — school grade, stroke count, newspaper frequency rank, former JLPT level — on the kanji record Phase 3a already stores, and render them as one parenthesised aside at the end of each kanji line in the popover.

**Architecture:** `KanjiData` in `jparser::index` gains four `Option` fields; because `kanji.bin`'s record layout changes, `INDEX_FORMAT_VERSION` goes 5 → 6 with `IndexHeader`'s 24-byte layout untouched, so an installed v5 index is refused with `VersionMismatch` and `ensure_dictionary` rebuilds it through the path that already exists. The streaming parser in `kanjidic.rs` fills the fields from `<grade>`, the first `<stroke_count>`, `<freq>` and `<jlpt>`, skipping and counting a character whose number does not parse, through the same path a bad `<literal>` takes today. `KanjiData` is already the wire type, so `kanji_info` needs no change; the webview's `KanjiInfo` gains four optional fields and `kanjiLine` a fourth column, a `(…)` span `colourLine` already paints as a `paren` run. No builder, fetch, command, settings or CSS change.

**Tech Stack:** Rust 2021 (`jparser`: `quick-xml` 0.36, `bincode` 1.3, serde), TypeScript + Vite frontend tested with Vitest. Tauri backend in `src-tauri` is untouched.

**Spec:** `docs/superpowers/specs/2026-09-11-engine-v2-phase3b-design.md` — read it first; this plan argues from it. The 2026-09-11 KANJIDIC2 file was inspected: 13,108 `<character>` elements; 2,999 carry `<grade>`, 13,654 `<stroke_count>` elements over 13,108 characters (525 characters carry two), 2,501 `<freq>`, 2,230 `<jlpt>`; maximum stroke count 34; maximum frequency rank 2,501.

## Global Constraints

- **Baseline cells byte-identical, both hint states** (spec §6 gate 1): hints on strict `0.8174 (1768)`, tolerant `0.8525 (1844)`, entry top-1 `0.9092`, compounds `0.6129 (114 / 186)` with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42; hints off `0.7342 (1588)`, `0.7924 (1714)`, `0.9093`, `0.5860 (109 / 186)`. **Nothing under `crates/jparser/src/engine/` is edited by this plan — not even `build.rs`** — so any movement is a defect in this plan's work.
- **Bench scaling cell read as a band, not a digit** (spec §6 gate 2): hints on `1.10x`, hints off `1.05x`; the hints-off cell has measured `0.98x`–`1.04x` across four runs of identical code, and a 4 ms swing on the short median moves it by about `0.07x`. Inside that spread is a pass; outside it is a re-run, not an explanation.
- **The `parse_sentences.txt` insta snapshot byte-identical**: `git status --short crates/jparser/tests/snapshots` prints nothing after `cargo test -p jparser --test engine_v2`.
- **Standing gates**, before every commit: `cargo test --workspace`, `cargo test -p jparser --features mecab`, `cargo test -p jparser --features hints-source`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`, `cargo clippy -p jparser --features hints-source --all-targets -- -D warnings`, `cargo check -p jparser --no-default-features --all-targets`, `cargo fmt --all --check`; `npm test` and `npx tsc --noEmit` before every commit that touches `src/`. Baselines from the Phase 3a handoff §2 at `9bf74ef`: jparser mecab 366, hints-source 367, workspace 598 passed and 1 ignored, `src-tauri` 152, npm 302, playwright 45. **Task 1 Step 1 re-takes these at the branch tip before anything changes**, since five review commits landed after `9bf74ef`. `TA_HINTS_DICT` or `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic` must exist for the two IPADIC-pinned `engine_v2` tests.
- **No new dependency**, no new Tauri command, no new settings field, no CSS change.
- **`INDEX_FORMAT_VERSION` goes 5 → 6 and `IndexHeader`'s layout does not change.** The 24-byte header is what lets a v6 loader read a v5 file's version before failing (Phase 3a handoff §3 item 1). `a_v4_header_is_refused_with_a_version_mismatch` keeps passing; Task 1 adds its v5 sibling.
- **`KanjiData` stays the wire type.** No Rust `KanjiInfo`. serde writes `None` as `null`; bincode 1.x cannot deserialise a struct whose fields were skipped on serialisation, so `#[serde(skip_serializing_if)]` is not used. TypeScript's four fields are optional (`grade?: number | null`, …) so every existing `KanjiInfo` literal in the test suites stays valid.
- **The first `<stroke_count>` wins**; later ones are dropped. **A `<misc>` number that does not parse into its field's type skips the character and counts it** in `skipped_count()`; it is never a build error and never silently `None`.
- **Grade codes render** `grade N` for 1–6, `secondary school` for 8, `name kanji` for 9 and 10, and `grade N` (the raw code) for anything else. **Strokes** render `N strokes`, `1 stroke`. **Frequency** renders `freq N`. **JLPT** renders `old JLPT N` — the former four-level scale, never mapped to N-levels.
- **Docs, code comments, commit messages and UI copy are written in normal prose**, not the session's terse style. Commits follow the repository's `type(scope): description` form with a prose body. No new source files are created, so no SPDX question arises; the one new file is the handoff.

### Measuring the gates

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase3b"
mkdir -p "$S"
./target/release/jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$S/gold_index"
export DIC="$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"
G=crates/jparser/tests/fixtures/gold.jsonl
C=crates/jparser/tests/fixtures/compound_gold.jsonl
./target/release/jparser-cli score "$S/gold_index" "$G" --compounds "$C"
./target/release/jparser-cli score "$S/gold_index" "$G" --compounds "$C" --hints "$DIC"
./target/release/jparser-cli bench "$S/gold_index" "$G"
./target/release/jparser-cli bench "$S/gold_index" "$G" --hints "$DIC"
```

The gold index is built without `--kanjidic` on purpose: the cells are a property of the parse, which never reads `kanji.bin`. Task 5 also builds it once *with* the fixture and diffs the hints-on `score` output against the run above.

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `crates/jparser/src/index/mod.rs` | `KanjiData` gains `grade`, `strokes`, `freq`, `jlpt`; `INDEX_FORMAT_VERSION` 5 → 6 and its doc comment | 1 |
| `crates/jparser/src/kanjidic.rs` | the yield site names the four fields (Task 1); `Field` gains four variants, `Pending` struct with `take` and `finish`, `leaf_field`; tests (Task 2) | 1, 2 |
| `crates/jparser/tests/engine_v2.rs` | the wire-pin literal names the four fields (Task 1); asserts them (Task 3) | 1, 3 |
| `crates/jparser/tests/index_roundtrip.rs` | `a_v5_header_is_refused_with_a_version_mismatch` (Task 1); `a_kanjidic_record_round_trips` asserts the fields (Task 2) | 1, 2 |
| `crates/jparser/tests/fixtures/kanjidic_mini.xml` | `<misc>` blocks for 都 and 食 | 2 |
| `crates/jparser/tests/fixtures/README.md` | the `kanjidic_mini.xml` entry names the `<misc>` cases | 2 |
| `src/types.ts` | `KanjiInfo` gains four optional fields | 3 |
| `src/kanji-lookup.ts` | `isKanjiInfo` validates them | 3 |
| `src/kanji-lookup.test.ts` | one test | 3 |
| `src/render/tooltip-text.ts` | `gradeLabel`, `kanjiAside`, `kanjiLine`'s fourth column | 4 |
| `src/render/tooltip-text.test.ts` | the aside's tests | 4 |
| `src/render/tooltip-colour.test.ts` | one test: the aside is one `paren` run | 4 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | Phase 3 table rows; `### Phase 3b` entry | 5 |
| `docs/superpowers/2026-09-11-engine-v2-phase3b-handoff.md` | created | 5 |

Not touched, by design: everything under `crates/jparser/src/engine/`, `crates/jparser/src/index/build.rs`, `crates/jparser/src/index/load.rs`, `crates/jmdict-source/`, `src-tauri/`, `src/main.ts`, `src/popover.ts`, `src/settings.ts`, `src/styles/`, `e2e/`.

---

### Task 1: Format 6, and the four fields on the record

**Files:**
- Modify: `crates/jparser/src/index/mod.rs` — the `INDEX_FORMAT_VERSION` doc comment and value (lines 21–28); `KanjiData` with its doc comment and derive (lines 89–106)
- Modify: `crates/jparser/src/kanjidic.rs` — the `KanjiData { … }` yield at line 180
- Modify: `crates/jparser/tests/engine_v2.rs` — the `KanjiData { … }` literal at line 972
- Test: `crates/jparser/tests/index_roundtrip.rs` — new test after `a_v4_header_is_refused_with_a_version_mismatch` (line 410–430)

**Interfaces:**
- Consumes: `KanjiData` as Phase 3a left it: `literal: char, on: Vec<String>, kun: Vec<String>, meanings: Vec<String>`; `Index::open`, `Index::header()`, `IndexError::VersionMismatch { found, expected }`.
- Produces: `pub struct KanjiData { literal, on, kun, meanings, grade: Option<u8>, strokes: Option<u8>, freq: Option<u16>, jlpt: Option<u8> }` in `jparser::index`; `INDEX_FORMAT_VERSION == 6`. Task 2 fills the fields; Task 3 pins them on the wire; Task 4 renders them.

- [ ] **Step 1: Take the suite baselines at the branch tip**

```bash
git log --oneline -1
cargo test --workspace 2>&1 | grep -E "^test result" 
cargo test -p jparser --features mecab 2>&1 | grep -E "^test result"
cargo test -p jparser --features hints-source 2>&1 | grep -E "^test result"
npm test 2>&1 | grep -E "Tests "
```

Record the commit hash and the four totals in a scratch note — `cargo test` prints one `test result` line per test binary, so sum them as the Phase 3a handoff did; Task 5's handoff quotes them as the "before" column. Expected: in the neighbourhood of the Global Constraints figures (598 / 366 / 367 / 302), all passing.

- [ ] **Step 2: Write the failing test — a v5 header is refused**

Add to `crates/jparser/tests/index_roundtrip.rs`, immediately after `a_v4_header_is_refused_with_a_version_mismatch`:

```rust
/// Phase 3b changed `KanjiData`'s layout, so a v5 `kanji.bin` must never be
/// decoded by this loader; the header's version is compared before that
/// file is read. The header itself is still the 24-byte v4/v5 layout, which
/// is what lets a v6 loader read a v5 file's version at all.
#[test]
fn a_v5_header_is_refused_with_a_version_mismatch() {
    let dir = tmpdir("kanji-v5-header");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let fingerprint = index.header().conjugation_fingerprint;
    drop(index);

    // The v5 layout, byte for byte: version, keys, records, entries, fingerprint.
    let v5 = bincode::serialize(&(5u32, 3u32, 3u32, 3u32, fingerprint)).unwrap();
    assert_eq!(v5.len(), 24);
    std::fs::write(dir.join("header.bin"), v5).unwrap();

    match Index::open(&dir).unwrap_err() {
        IndexError::VersionMismatch { found, expected } => {
            assert_eq!(found, 5);
            assert_eq!(expected, INDEX_FORMAT_VERSION);
        }
        other => panic!("a v5 header must be a version mismatch, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p jparser --test index_roundtrip a_v5_header_is_refused_with_a_version_mismatch`
Expected: FAIL — `Index::open` succeeds on a version-5 header while the constant is still 5, so `unwrap_err()` panics with `called \`Result::unwrap_err()\` on an \`Ok\` value`.

- [ ] **Step 4: Bump the version and add the fields**

In `crates/jparser/src/index/mod.rs`, replace the constant's doc comment and value (lines 21–28) with:

```rust
/// Bumped whenever the on-disk layout changes. A mismatch forces a rebuild; the
/// loader must never try to read an index it does not recognize.
///
/// 4 (Phase 1b-ii): `StoredRecord` lost `verb_type`.
/// 5 (Phase 3a): a sixth file, `kanji.bin`. `IndexHeader`'s own layout is
/// unchanged, and must stay so unless `open_with` learns to read the version
/// before decoding the rest — see `a_v4_header_is_refused_with_a_version_mismatch`.
/// 6 (Phase 3b): `KanjiData` gained `grade`, `strokes`, `freq` and `jlpt`, so
/// `kanji.bin`'s record layout changed. The header is still untouched — see
/// `a_v5_header_is_refused_with_a_version_mismatch`.
pub const INDEX_FORMAT_VERSION: u32 = 6;
```

Replace `KanjiData`, its doc comment and its derive (lines 89–106) with:

```rust
/// One KANJIDIC2 character, as the popover needs it.
///
/// Also the wire shape `kanji_info` returns to the webview: `src/types.ts`'s
/// `KanjiInfo` names these eight fields, and
/// `kanji_data_serializes_to_the_documented_wire_shape` in
/// `tests/engine_v2.rs` pins them, so neither side can drift alone. The four
/// `Option`s cross the wire as a number or `null`; bincode 1.x cannot skip a
/// field on serialisation and read it back, so they are never omitted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KanjiData {
    pub literal: char,
    /// `ja_on` readings, katakana as KANJIDIC2 gives them.
    pub on: Vec<String>,
    /// `ja_kun` readings. KANJIDIC2's `.` (okurigana boundary) and `-`
    /// (affix) marks are kept: `ひがし`, `た.べる`, `-がた`.
    pub kun: Vec<String>,
    /// `<meaning>` elements with no `m_lang` attribute -- English -- the
    /// first [`crate::kanjidic::MAX_MEANINGS`] of them.
    pub meanings: Vec<String>,
    /// `<grade>`: 1–6 the school year a kyōiku kanji is taught in, 8 the
    /// remaining jōyō kanji (secondary school), 9 and 10 jinmeiyō (name)
    /// kanji. `None` for the 10,109 characters the file does not grade.
    pub grade: Option<u8>,
    /// The first `<stroke_count>` -- the accepted count. Later ones are
    /// common miscounts and are dropped. The file's maximum is 34.
    pub strokes: Option<u8>,
    /// `<freq>`: newspaper frequency rank, 1–2,501. `None` when unranked.
    pub freq: Option<u16>,
    /// `<jlpt>`: level in the former four-level JLPT, 1 (advanced) to 4
    /// (elementary). `None` when the character was on no level's list.
    pub jlpt: Option<u8>,
}
```

- [ ] **Step 5: Make the two construction sites compile**

In `crates/jparser/src/kanjidic.rs`, the yield at line 180 becomes (the four fields are filled in Task 2; here they only exist):

```rust
                                return Some(Ok(KanjiData {
                                    literal: ch,
                                    on: std::mem::take(&mut on),
                                    kun: std::mem::take(&mut kun),
                                    meanings: std::mem::take(&mut meanings),
                                    grade: None,
                                    strokes: None,
                                    freq: None,
                                    jlpt: None,
                                }));
```

In `crates/jparser/tests/engine_v2.rs`, the literal at line 972 becomes:

```rust
    let data = jparser::index::KanjiData {
        literal: '東',
        on: vec!["トウ".to_string()],
        kun: vec!["ひがし".to_string()],
        meanings: vec!["east".to_string()],
        grade: None,
        strokes: None,
        freq: None,
        jlpt: None,
    };
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p jparser --test index_roundtrip`
Expected: PASS, including both `a_v4_header_is_refused_with_a_version_mismatch` (a v4 header is still refused, and the `IndexHeader` layout is still 24 bytes) and the new v5 test. Then `cargo test -p jparser` for the crate: the `ensure_dictionary` tests in `index/mod.rs` that corrupt a header to `INDEX_FORMAT_VERSION - 1` now write 5 and still rebuild. Then the standing gates in Global Constraints, all of them; `cargo test --workspace` must pass, which shows `src-tauri`'s command tests build with the eight-field record.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/src/index/mod.rs crates/jparser/src/kanjidic.rs crates/jparser/tests/engine_v2.rs crates/jparser/tests/index_roundtrip.rs
git commit -m "feat(jparser): index format 6, KanjiData carries the four misc fields

KanjiData gains grade, strokes, freq and jlpt as Options, which changes
kanji.bin's record layout, so INDEX_FORMAT_VERSION goes 5 to 6. The header
keeps its 24-byte layout so a v6 loader still reads a v5 file's version
and refuses it with VersionMismatch, the error ensure_dictionary rebuilds
on; a test hand-encodes a v5 header and pins that. The parser leaves the
fields None until the next commit."
```

---

### Task 2: The parser fills them

**Files:**
- Modify: `crates/jparser/tests/fixtures/kanjidic_mini.xml` — `<misc>` blocks for 都 and 食
- Modify: `crates/jparser/tests/fixtures/README.md` — the `kanjidic_mini.xml` paragraph (lines 140–147)
- Modify: `crates/jparser/src/kanjidic.rs` — module doc (lines 3–15), `Field` (lines 58–66), the `Iterator` impl (lines 88–199), tests
- Test: `crates/jparser/src/kanjidic.rs` tests module; `crates/jparser/tests/index_roundtrip.rs::a_kanjidic_record_round_trips` (lines 356–371)

**Interfaces:**
- Consumes: `KanjiData` with the eight fields from Task 1; `MAX_MEANINGS`; `KanjidicReader::skipped_count()`.
- Produces: the same `parse_characters` / `KanjidicReader` API, now yielding records whose four `Option`s are filled from `<misc>`. Private helpers `Pending::take(Field, String)`, `Pending::finish() -> Option<KanjiData>`, `leaf_field(&BytesStart) -> Field`. Nothing outside the file sees them.

- [ ] **Step 1: Extend the fixture**

In `crates/jparser/tests/fixtures/kanjidic_mini.xml`, give 都 its real 2026-09-11 `<misc>` block — two stroke counts, the accepted one first, and a `<variant>` the reader must ignore — by inserting after 都's `</codepoint>`:

```xml
<misc>
<grade>3</grade>
<stroke_count>11</stroke_count>
<stroke_count>10</stroke_count>
<variant var_type="jis213">1-92-74</variant>
<freq>123</freq>
<jlpt>3</jlpt>
</misc>
```

Give 食 a block trimmed to two fields, inserted after 食's `</codepoint>`:

```xml
<misc>
<grade>2</grade>
<stroke_count>9</stroke_count>
</misc>
```

東 already carries `grade 2, stroke_count 8, freq 37, jlpt 4`; 京 keeps no `<misc>` at all. In `crates/jparser/tests/fixtures/README.md`, the paragraph under `## kanjidic_mini.xml` becomes:

```markdown
Four characters (東, 京, 都, 食) in **KANJIDIC2**'s published structure,
trimmed to the elements the reader keeps or must skip: a foreign-language
`<meaning m_lang>`, `<nanori>`, non-Japanese `r_type`s, a character with five
English meanings (the cap is four), and kun readings carrying the `.` and `-`
marks. The `<misc>` blocks cover the four cases Phase 3b's parser handles:
東 carries all four numbers; 京 has no `<misc>` at all; 都 has its real block,
with two `<stroke_count>`s (the accepted 11, then the miscount 10) and a
`<variant>` to ignore; 食 is trimmed to grade and stroke count only. Used by
`src/kanjidic.rs`'s own tests, `tests/index_roundtrip.rs`, and `src-tauri`'s
command tests.
```

- [ ] **Step 2: Write the failing parser tests**

Add to the `tests` module of `crates/jparser/src/kanjidic.rs`, after `keeps_english_meanings_only`:

```rust
    // Phase 3b: the four `<misc>` numbers. 東's block is the file's real one.
    #[test]
    fn reads_the_four_misc_fields() {
        let k = find('東');
        assert_eq!(
            (k.grade, k.strokes, k.freq, k.jlpt),
            (Some(2), Some(8), Some(37), Some(4))
        );
    }

    #[test]
    fn a_character_without_misc_has_none_in_every_field() {
        let k = find('京');
        assert_eq!((k.grade, k.strokes, k.freq, k.jlpt), (None, None, None, None));
    }

    /// KANJIDIC2's DTD: the first count is the accepted one, later ones are
    /// common miscounts. 都's real block lists 11 then 10.
    #[test]
    fn the_first_stroke_count_is_the_accepted_one() {
        assert_eq!(find('都').strokes, Some(11));
        assert_eq!(find('都').grade, Some(3));
    }

    #[test]
    fn a_partial_misc_block_leaves_the_rest_none() {
        let k = find('食');
        assert_eq!((k.grade, k.strokes, k.freq, k.jlpt), (Some(2), Some(9), None, None));
    }

    /// A number that does not parse into its field's type takes the same
    /// path a two-character `<literal>` does: the character is skipped and
    /// counted, never a build error and never a silent `None` (design §3.1).
    #[test]
    fn a_malformed_misc_number_skips_the_character_and_counts_it() {
        let xml = "<kanjidic2>\
            <character><literal>一</literal><misc><freq>abc</freq></misc></character>\
            <character><literal>二</literal><misc><grade>300</grade></misc></character>\
            <character><literal>本</literal><misc><grade>1</grade></misc></character>\
            </kanjidic2>";
        let mut reader = parse_characters(std::io::Cursor::new(xml));
        let ok: Vec<_> = reader.by_ref().filter_map(Result::ok).collect();
        assert_eq!(ok.len(), 1, "the good character must still be returned");
        assert_eq!(ok[0].literal, '本');
        assert_eq!(ok[0].grade, Some(1));
        assert_eq!(reader.skipped_count(), 2);
    }
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p jparser --lib kanjidic::tests`
Expected: the five new tests FAIL — `reads_the_four_misc_fields` on `(None, None, None, None)` against `Some`s, `the_first_stroke_count_is_the_accepted_one` and `a_partial_misc_block_leaves_the_rest_none` likewise, `a_malformed_misc_number_skips_the_character_and_counts_it` with `ok.len()` 3 and `skipped_count()` 0. `a_character_without_misc_has_none_in_every_field` passes already, which is fine: it pins that 京 stays `None` once the others fill.

- [ ] **Step 4: Implement — `Field`, `Pending`, `leaf_field`, and a shorter `next`**

In `crates/jparser/src/kanjidic.rs`, replace the module doc's second paragraph (lines 5–10) so it reads:

```rust
//! One `<character>` per kanji. Kept: `<literal>`, every `<reading>` whose
//! `r_type` is `ja_on` or `ja_kun`, every `<meaning>` with no `m_lang`
//! attribute (English), capped at [`MAX_MEANINGS`], and four `<misc>`
//! numbers -- `<grade>`, the first `<stroke_count>`, `<freq>` and `<jlpt>`
//! (Phase 3b). Read and discarded: `<nanori>`, every other `r_type` (pinyin,
//! korean, vietnam), every other language, later `<stroke_count>`s (the DTD
//! calls them common miscounts), and `<misc>`'s `<variant>` and `<rad_name>`.
```

Replace `Field` (lines 58–66) with:

```rust
/// Which leaf's text is being accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Literal,
    On,
    Kun,
    Meaning,
    Grade,
    Strokes,
    Freq,
    Jlpt,
}

/// Which field a leaf element inside `<character>` feeds, or `None`.
fn leaf_field(e: &BytesStart) -> Field {
    match e.local_name().as_ref() {
        b"literal" => Field::Literal,
        b"reading" => reading_field(e),
        b"meaning" => meaning_field(e),
        b"grade" => Field::Grade,
        b"stroke_count" => Field::Strokes,
        b"freq" => Field::Freq,
        b"jlpt" => Field::Jlpt,
        _ => Field::None,
    }
}

/// The character being read, before its `</character>`.
#[derive(Default)]
struct Pending {
    literal: String,
    on: Vec<String>,
    kun: Vec<String>,
    meanings: Vec<String>,
    grade: Option<u8>,
    strokes: Option<u8>,
    freq: Option<u16>,
    jlpt: Option<u8>,
    /// Set when a `<misc>` number failed to parse. The character is then
    /// skipped and counted at `</character>`, as one with a bad `<literal>`
    /// is -- the build's one skip path, already surfaced (design §3.1).
    bad: bool,
}

impl Pending {
    /// Stores one closed leaf's trimmed text.
    fn take(&mut self, field: Field, value: String) {
        match field {
            Field::Literal => self.literal = value,
            Field::On => self.on.push(value),
            Field::Kun => self.kun.push(value),
            Field::Meaning => {
                if self.meanings.len() < MAX_MEANINGS {
                    self.meanings.push(value);
                }
            }
            Field::Grade => self.grade = self.number(&value),
            // The first count is the accepted one; later ones are common
            // miscounts (KANJIDIC2 DTD) and are dropped.
            Field::Strokes => {
                if self.strokes.is_none() {
                    self.strokes = self.number(&value);
                }
            }
            Field::Freq => self.freq = self.number(&value),
            Field::Jlpt => self.jlpt = self.number(&value),
            Field::None => {}
        }
    }

    /// `text` as a `T`, or `None` with the character marked bad. A value
    /// that parses but does not fit the type (a grade of 300) is the same
    /// case as one that does not parse at all.
    fn number<T: std::str::FromStr>(&mut self, text: &str) -> Option<T> {
        let parsed = text.parse().ok();
        if parsed.is_none() {
            self.bad = true;
        }
        parsed
    }

    /// The finished record, or `None` for a character to skip: a literal
    /// that is not exactly one character, or a malformed `<misc>` number.
    fn finish(self) -> Option<KanjiData> {
        if self.bad {
            return None;
        }
        let mut chars = self.literal.chars();
        match (chars.next(), chars.next()) {
            (Some(literal), None) => Some(KanjiData {
                literal,
                on: self.on,
                kun: self.kun,
                meanings: self.meanings,
                grade: self.grade,
                strokes: self.strokes,
                freq: self.freq,
                jlpt: self.jlpt,
            }),
            _ => None,
        }
    }
}
```

Replace the whole `impl<R: BufRead> Iterator for KanjidicReader<R>` block (lines 88–199) with:

```rust
impl<R: BufRead> Iterator for KanjidicReader<R> {
    type Item = Result<KanjiData, KanjidicError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut in_character = false;
        let mut pending = Pending::default();
        let mut field = Field::None;
        let mut text = String::new();

        loop {
            self.buf.clear();
            let event = match self.reader.read_event_into(&mut self.buf) {
                Ok(ev) => ev,
                Err(e) => {
                    self.done = true;
                    return Some(Err(KanjidicError::Xml(e)));
                }
            };

            match event {
                Event::Eof => {
                    self.done = true;
                    if in_character {
                        // The plain event reader does not check that every
                        // opened element was closed; a document truncated
                        // mid-character must still be an error, not a
                        // silently shorter result. Same rule as `jmdict.rs`.
                        return Some(Err(KanjidicError::Xml(Error::IllFormed(
                            IllFormedError::MissingEndTag("character".to_string()),
                        ))));
                    }
                    return None;
                }
                Event::Start(e) => {
                    if !in_character {
                        if e.local_name().as_ref() == b"character" {
                            in_character = true;
                        }
                        continue;
                    }
                    field = leaf_field(&e);
                    text.clear();
                }
                Event::Text(t) => {
                    if field != Field::None {
                        match t.unescape() {
                            Ok(s) => text.push_str(&s),
                            Err(e) => {
                                self.done = true;
                                return Some(Err(KanjidicError::Xml(e)));
                            }
                        }
                    }
                }
                Event::End(e) => {
                    if !in_character {
                        continue;
                    }
                    pending.take(field, text.trim().to_string());
                    field = Field::None;
                    text.clear();

                    if e.local_name().as_ref() == b"character" {
                        in_character = false;
                        match std::mem::take(&mut pending).finish() {
                            Some(record) => return Some(Ok(record)),
                            None => self.skipped += 1,
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
```

`next` is now about 70 lines, down from 107 (Phase 3a handoff §5's first deferred minor). `reading_field` and `meaning_field` are unchanged and still called from `leaf_field`.

- [ ] **Step 5: Run the parser tests to verify they pass**

Run: `cargo test -p jparser --lib kanjidic::tests`
Expected: PASS, all fourteen — the nine existing ones (`reads_every_character_in_document_order`, `reads_on_and_kun_readings_in_order`, `keeps_okurigana_and_affix_marks_verbatim`, `drops_nanori_and_non_japanese_readings`, `keeps_english_meanings_only`, `caps_meanings_at_four`, `skips_a_character_with_a_bad_literal_and_counts_it`, `decodes_a_standard_escape_in_a_meaning`, `reports_malformed_xml_as_an_error`, `an_empty_reader_yields_nothing`) and the five new ones.

- [ ] **Step 6: Extend the round-trip test**

In `crates/jparser/tests/index_roundtrip.rs`, `a_kanjidic_record_round_trips` (lines 356–371) gets, after its three `east` assertions:

```rust
    // Phase 3b: the four misc numbers survive the disk round trip, and a
    // character without a <misc> block reads back as four Nones.
    assert_eq!(
        (east.grade, east.strokes, east.freq, east.jlpt),
        (Some(2), Some(8), Some(37), Some(4))
    );
    let kyo = index.kanji('京').expect("京");
    assert_eq!((kyo.grade, kyo.strokes, kyo.freq, kyo.jlpt), (None, None, None, None));
    assert_eq!(index.kanji('都').expect("都").strokes, Some(11));
```

Run: `cargo test -p jparser --test index_roundtrip`
Expected: PASS.

- [ ] **Step 7: The standing gates, then commit**

Run every command in Global Constraints "Standing gates" for Rust (the `npm` pair is not needed; nothing under `src/` changed). `cargo test --workspace` shows `src-tauri`'s `kanji_info_answers_from_the_published_index` still passing against a `FIXTURE_KANJIDIC` with no `<misc>` — its 本 now serialises with four `null`s, which the test does not look at. Expected: all green, clippy clean, fmt clean.

```bash
git add crates/jparser/src/kanjidic.rs crates/jparser/tests/fixtures/kanjidic_mini.xml crates/jparser/tests/fixtures/README.md crates/jparser/tests/index_roundtrip.rs
git commit -m "feat(jparser): keep grade, stroke count, frequency and JLPT from KANJIDIC2

The reader fills KanjiData's four misc fields from <grade>, the first
<stroke_count> (later ones are the DTD's common miscounts), <freq> and
<jlpt>. A number that does not parse into its field's type marks the
character bad, and it is skipped and counted at </character> through the
path a two-character literal already takes, so the build report surfaces
it and nothing is silently None. The End arm's two jobs move into
Pending::take and Pending::finish, which leaves next() shorter than it
was. The fixture gains 都's real block, with its two stroke counts and a
variant to ignore, and a trimmed block for 食."
```

---

### Task 3: The wire, both sides

**Files:**
- Modify: `crates/jparser/tests/engine_v2.rs` — `kanji_data_serializes_to_the_documented_wire_shape` (lines 967–985)
- Modify: `src/types.ts` — `KanjiInfo` (lines 40–48)
- Modify: `src/kanji-lookup.ts` — `isKanjiInfo` (lines 16–25)
- Test: `src/kanji-lookup.test.ts` — one test in `describe('createKanjiLookup')`

**Interfaces:**
- Consumes: `KanjiData`'s eight fields (Task 1); serde's JSON for `Option`: a number or `null`.
- Produces: `KanjiInfo { literal, on, kun, meanings, grade?: number | null, strokes?: number | null, freq?: number | null, jlpt?: number | null }` in `src/types.ts`. Task 4 reads the four.

- [ ] **Step 1: Write the failing Rust pin**

In `crates/jparser/tests/engine_v2.rs`, replace the test (lines 967–985) with:

```rust
/// Pins the JSON `kanji_info` hands the webview. Field names here are the
/// TypeScript property names of `src/types.ts`'s `KanjiInfo`; `literal` is a
/// `char`, which serde_json writes as a one-character string. The four misc
/// numbers cross as a number or `null`, never as a missing key: bincode 1.x
/// cannot read back a struct whose fields were skipped on serialisation, so
/// `skip_serializing_if` is not an option (Phase 3b design §4.1).
#[test]
fn kanji_data_serializes_to_the_documented_wire_shape() {
    let data = jparser::index::KanjiData {
        literal: '東',
        on: vec!["トウ".to_string()],
        kun: vec!["ひがし".to_string()],
        meanings: vec!["east".to_string()],
        grade: Some(2),
        strokes: Some(8),
        freq: Some(37),
        jlpt: None,
    };
    let json = serde_json::to_value(&data).expect("serialize");
    assert_eq!(json["literal"], "東");
    assert_eq!(json["on"][0], "トウ");
    assert_eq!(json["kun"][0], "ひがし");
    assert_eq!(json["meanings"][0], "east");
    assert_eq!(json["grade"], 2);
    assert_eq!(json["strokes"], 8);
    assert_eq!(json["freq"], 37);
    let keys = json.as_object().expect("an object");
    assert!(keys.contains_key("jlpt"), "an absent field is a present key");
    assert!(json["jlpt"].is_null(), "an absent field is null, got {:?}", json["jlpt"]);
    assert_eq!(keys.len(), 8, "exactly the eight documented fields");
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p jparser --test engine_v2 kanji_data_serializes_to_the_documented_wire_shape`
Expected: PASS already — Task 1 made the record serialise this way; this step turns the fact into a pin. (If it fails, the record's derive or field names are wrong; fix Task 1's struct, not this test.)

- [ ] **Step 3: Write the failing TypeScript test**

Add to `src/kanji-lookup.test.ts`, inside `describe('createKanjiLookup')`, after `treats a non-array or malformed answer as no records`:

```ts
  // Phase 3b: the four misc fields cross the wire as a number or null, and a
  // payload from an older main webview may lack them altogether. A string in
  // one is a malformed record, like a numeric literal.
  test('accepts the misc fields absent, null or numeric, and rejects a string', async () => {
    const full: KanjiInfo = { ...東, grade: 2, strokes: 8, freq: 37, jlpt: 4 };
    const nulls: KanjiInfo = { ...京, grade: null, strokes: null, freq: null, jlpt: null };
    const bad = { ...都, grade: '3' };
    const lookup = createKanjiLookup(() => Promise.resolve([full, nulls, bad]));
    expect(await lookup('東京都')).toEqual([full, nulls]);
  });
```

- [ ] **Step 4: Run it to verify it fails**

Run: `npx vitest run src/kanji-lookup.test.ts`
Expected: `npx tsc --noEmit` would also fail here because `KanjiInfo` has no `grade`; the vitest run itself FAILS on the assertion — `bad` passes today's validator, so the result has three records.

- [ ] **Step 5: Extend the type and the validator**

In `src/types.ts`, `KanjiInfo` (lines 40–48) becomes:

```ts
export interface KanjiInfo {
  literal: string;
  /** On readings, katakana. */
  on: string[];
  /** Kun readings, with KANJIDIC2's `.` and `-` marks kept: `た.べる`, `-がた`. */
  kun: string[];
  /** English meanings, at most four. */
  meanings: string[];
  // KANJIDIC2's `<misc>` numbers (Phase 3b). The backend always sends each
  // as a number or `null`; they are optional here so a payload from an older
  // main webview, and the test suites' shorter literals, stay valid.
  /** School grade code: 1–6 the year taught, 8 remaining jōyō (secondary school), 9–10 name kanji. */
  grade?: number | null;
  /** The accepted stroke count. */
  strokes?: number | null;
  /** Newspaper frequency rank, 1–2,501. */
  freq?: number | null;
  /** Level in the former four-level JLPT, 1 (advanced) to 4 (elementary). */
  jlpt?: number | null;
}
```

In `src/kanji-lookup.ts`, replace `isKanjiInfo` (lines 16–25) with:

```ts
/** Absent, `null`, or a number: the three ways an `Option<u8>` reaches this side. */
function isOptionalNumber(value: unknown): boolean {
  return value === undefined || value === null || typeof value === 'number';
}

/** Whether `value` is a `KanjiInfo` as `kanji_info` serialises one. */
function isKanjiInfo(value: unknown): value is KanjiInfo {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.literal === 'string' &&
    Array.isArray(v.on) &&
    Array.isArray(v.kun) &&
    Array.isArray(v.meanings) &&
    isOptionalNumber(v.grade) &&
    isOptionalNumber(v.strokes) &&
    isOptionalNumber(v.freq) &&
    isOptionalNumber(v.jlpt)
  );
}
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npx vitest run src/kanji-lookup.test.ts && npx tsc --noEmit`
Expected: PASS, nine tests in the file; `tsc` clean. Then `npm test` for the whole suite: every existing `KanjiInfo` literal in `src/main-tooltip.test.ts`, `src/popover.test.ts` and `src/render/tooltip-text.test.ts` still type-checks because the four fields are optional. Then the Rust standing gates for `engine_v2.rs`: `cargo test -p jparser --test engine_v2`, `cargo clippy -p jparser --all-targets -- -D warnings`, `cargo fmt --all --check`.

- [ ] **Step 7: Commit**

```bash
git add crates/jparser/tests/engine_v2.rs src/types.ts src/kanji-lookup.ts src/kanji-lookup.test.ts
git commit -m "feat: the four kanji misc fields cross the wire

The engine_v2 wire pin asserts grade, strokes, freq and jlpt by name, and
that an absent one is a present key holding null rather than a missing
key. KanjiInfo gains the four as optional number-or-null fields, so older
payloads and the suites' shorter literals stay valid, and the boundary
validator accepts each as absent, null or numeric and rejects anything
else."
```

---

### Task 4: The popover renders the aside

**Files:**
- Modify: `src/render/tooltip-text.ts` — `kanjiLine` (lines 59–70)
- Test: `src/render/tooltip-text.test.ts` — new `describe` inside `describe('the kanji block')` (after line 219)
- Test: `src/render/tooltip-colour.test.ts` — one test at the end of `describe('colourLine')`

**Interfaces:**
- Consumes: `KanjiInfo`'s four optional fields (Task 3); `colourLine`'s existing `paren` run rule.
- Produces: `assembleTooltipText(entries, filters?, kanji = [])` unchanged in signature; each kanji line gains a trailing `(…)` column when any misc field is present. Private `gradeLabel(grade: number): string` and `kanjiAside(kanji: KanjiInfo): string`.

- [ ] **Step 1: Write the failing tests**

In `src/render/tooltip-text.test.ts`, inside `describe('the kanji block', …)` after the `omits an empty column rather than leaving a gap` test, add:

```ts
    // Phase 3b: KANJIDIC2's <misc> numbers ride the same line as one
    // parenthesised aside, in the file's order — grade, strokes, frequency,
    // JLPT — with absent fields left out. The JLPT number is the former
    // four-level scale and is labelled as such, never mapped to N-levels.
    describe('the misc aside', () => {
      const head = '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n';
      const line = (k: KanjiInfo): string =>
        assembleTooltipText([entry()], undefined, [k]).slice(head.length);
      const east = tokyo[0];

      test('appends all four fields in KANJIDIC2 order', () => {
        expect(line({ ...east, grade: 2, strokes: 8, freq: 37, jlpt: 4 })).toBe(
          '東  トウ・ひがし  east, eastern  (grade 2, 8 strokes, freq 37, old JLPT 4)',
        );
      });

      test('leaves out absent fields, and the whole aside when none is present', () => {
        expect(line({ ...east, grade: 2, strokes: 8 })).toBe(
          '東  トウ・ひがし  east, eastern  (grade 2, 8 strokes)',
        );
        expect(line({ ...east, freq: 37 })).toBe('東  トウ・ひがし  east, eastern  (freq 37)');
        expect(line({ ...east, grade: null, strokes: null, freq: null, jlpt: null })).toBe(
          '東  トウ・ひがし  east, eastern',
        );
        expect(line(east)).toBe('東  トウ・ひがし  east, eastern');
      });

      test('one stroke is singular', () => {
        expect(line({ literal: '一', on: ['イチ'], kun: ['ひと.つ'], meanings: ['one'], strokes: 1 })).toBe(
          '一  イチ・ひと.つ  one  (1 stroke)',
        );
      });

      // The DTD's codes: 1–6 are school years; 8 is the remaining jōyō kanji,
      // learned in secondary school; 9 and 10 are jinmeiyō (name) kanji. An
      // unknown code is shown raw rather than guessed at.
      test('names the grade codes that are not school years', () => {
        expect(line({ ...east, grade: 8 })).toBe('東  トウ・ひがし  east, eastern  (secondary school)');
        expect(line({ ...east, grade: 9 })).toBe('東  トウ・ひがし  east, eastern  (name kanji)');
        expect(line({ ...east, grade: 10 })).toBe('東  トウ・ひがし  east, eastern  (name kanji)');
        expect(line({ ...east, grade: 7 })).toBe('東  トウ・ひがし  east, eastern  (grade 7)');
      });

      test('a record with nothing but an aside still has no doubled gap', () => {
        expect(line({ literal: '々', on: [], kun: [], meanings: [], strokes: 3 })).toBe('々  (3 strokes)');
      });

      test('hide_kanji hides the aside with the line it is on', () => {
        expect(
          assembleTooltipText([entry()], { hide_kanji: true }, [{ ...east, grade: 2, strokes: 8 }]),
        ).toBe('消える【きえる】\n(v1,vi) (1) to disappear/to vanish');
      });
    });
```

In `src/render/tooltip-colour.test.ts`, at the end of `describe('colourLine')`:

```ts
  // Phase 3b: the misc aside on a kanji line is a `(…)` span, so it takes the
  // paren colour with no rule of its own; the katakana middle dot (U+30FB)
  // sits in the katakana block and rides in the kana run.
  test('a kanji line with its misc aside ends in one paren run', () => {
    const line = '東  トウ・ひがし  east  (grade 2, 8 strokes, freq 37, old JLPT 4)';
    expect(kinds(line)).toEqual(['kanji', 'text', 'kana', 'text', 'paren']);
    expect(colourLine(line).at(-1)).toEqual({
      text: '(grade 2, 8 strokes, freq 37, old JLPT 4)',
      kind: 'paren',
    });
  });
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npx vitest run src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts`
Expected: the colour test PASSES already (nothing in `colourLine` changes; this pins the property the design relies on). In `tooltip-text.test.ts`, `leaves out absent fields…`'s last two assertions and `hide_kanji hides the aside…` pass; the other assertions FAIL because `kanjiLine` emits no aside — e.g. received `'東  トウ・ひがし  east, eastern'` for the four-field case.

- [ ] **Step 3: Implement `gradeLabel`, `kanjiAside`, the fourth column**

In `src/render/tooltip-text.ts`, replace `kanjiLine` and its doc comment (lines 59–70) with:

```ts
/** A misc field that is present: the backend sends a number or `null`; an older payload sends nothing. */
function has(value: number | null | undefined): value is number {
  return typeof value === 'number';
}

/**
 * `grade N` for a school year. KANJIDIC2's DTD defines 8 as the remaining
 * jōyō kanji learned in junior high school and 9–10 as jinmeiyō (name)
 * kanji, so those codes are named rather than numbered; any other code is
 * shown raw rather than guessed at.
 */
function gradeLabel(grade: number): string {
  if (grade === 8) return 'secondary school';
  if (grade === 9 || grade === 10) return 'name kanji';
  return `grade ${grade}`;
}

/**
 * The parenthesised aside carrying KANJIDIC2's `<misc>` numbers, in the
 * file's own order; absent fields are left out and an aside with nothing in
 * it is `''`, so `kanjiLine` drops the column. Parentheses because
 * `colourLine` already paints a `(…)` span as a `paren` run. The JLPT level
 * is the former four-level scale and is labelled as such (design §1.2).
 */
function kanjiAside(kanji: KanjiInfo): string {
  const parts: string[] = [];
  if (has(kanji.grade)) parts.push(gradeLabel(kanji.grade));
  if (has(kanji.strokes)) parts.push(`${kanji.strokes} ${kanji.strokes === 1 ? 'stroke' : 'strokes'}`);
  if (has(kanji.freq)) parts.push(`freq ${kanji.freq}`);
  if (has(kanji.jlpt)) parts.push(`old JLPT ${kanji.jlpt}`);
  return parts.length > 0 ? `(${parts.join(', ')})` : '';
}

/**
 * One kanji's line: the literal, its on then kun readings joined by the
 * katakana middle dot, its English meanings, and the misc aside. Columns are
 * two spaces apart; an empty column is dropped rather than left as a gap.
 * KANJIDIC2's `.` and `-` marks in kun readings are rendered raw,
 * deliberately (design §7).
 */
function kanjiLine(kanji: KanjiInfo): string {
  const readings = [...kanji.on, ...kanji.kun].join('・');
  return [kanji.literal, readings, kanji.meanings.join(', '), kanjiAside(kanji)]
    .filter((column) => column !== '')
    .join('  ');
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts && npx tsc --noEmit`
Expected: PASS, every test in both files, `tsc` clean. Then `npm test` for the suite — `src/popover.test.ts` and `src/main-tooltip.test.ts` render `KanjiInfo` literals without misc fields and must produce exactly the lines they produced before.

- [ ] **Step 5: Commit**

```bash
git add src/render/tooltip-text.ts src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts
git commit -m "feat(popover): the kanji line gains its misc aside

Each kanji line ends in one parenthesised aside carrying KANJIDIC2's misc
numbers in the file's order: grade, stroke count, newspaper frequency
rank, former JLPT level. Absent fields are left out and an empty aside is
dropped with the column. Grade codes 8, 9 and 10 are named as the DTD
defines them, a single stroke is singular, and the JLPT number is
labelled as the former four-level scale rather than mapped to N-levels.
The aside is a (...) span, so colourLine paints it as a paren run and
nothing in the colouriser or the stylesheet changes."
```

---

### Task 5: Gates, the real file, and the record

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — the Phase 3 table's "JLPT level" row (line 1195) and "Kanji readings" row (line 1196); a `### Phase 3b` entry after the `### Phase 3a` entry (lines 1174–1185)
- Create: `docs/superpowers/2026-09-11-engine-v2-phase3b-handoff.md`

- [ ] **Step 1: The engine gates**

Run the block under "Measuring the gates" above. Compare against Global Constraints cell by cell: both `score` runs (every cell, and the three compound classes with hints on), both `bench` scaling cells read as the band. Then build the gold index once *with* the fixture and diff, rather than eyeball, the hints-on `score` output:

```bash
./target/release/jparser-cli score "$S/gold_index" "$G" --compounds "$C" --hints "$DIC" > "$S/score_plain.txt"
./target/release/jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$S/gold_index_k" --kanjidic crates/jparser/tests/fixtures/kanjidic_mini.xml
./target/release/jparser-cli score "$S/gold_index_k" "$G" --compounds "$C" --hints "$DIC" > "$S/score_kanji.txt"
diff "$S/score_plain.txt" "$S/score_kanji.txt" && echo identical
```

Expected: `identical`, and every cell equal to the required one. Any difference in any cell: stop, do not adjust the record, find the edit that caused it — this plan edits nothing under `engine/` or `index/build.rs` or `index/load.rs`, so a moved cell means a file was touched that should not have been.

- [ ] **Step 2: The snapshot and the suites**

```bash
cargo test -p jparser --test engine_v2
git status --short crates/jparser/tests/snapshots
```

Expected: pass; the second command prints nothing. Then every standing gate from Global Constraints, plus:

```bash
cargo +1.85 check -p jparser -p jmdict-source
cargo llvm-cov clean --workspace
cargo llvm-cov -p jparser --features mecab
npx playwright test
```

Read the per-file rows, not the crate total: `kanjidic.rs`, `index/build.rs`, `index/load.rs` must each be at or above 80% lines. If the run prints `warning: … functions have mismatched data`, the table is invalid — clean and re-run, as the Phase 3a handoff §2 records. Playwright: expected 45 passed; the first-run copy did not change, so no baseline moves. Record every total beside Task 1 Step 1's "before" figures.

- [ ] **Step 3: The real file, headlessly**

The CLI takes an uncompressed KANJIDIC2. Decompress the cached archive into scratch and build the full index with it:

```bash
gzip -dc "$APPDATA/dev.jparser.clipyomi/source/kanjidic2.xml.gz" > "$S/kanjidic2.xml"
gzip -dc "$APPDATA/dev.jparser.clipyomi/source/JMdict_e.gz" > "$S/JMdict_e.xml"
time ./target/release/jparser-cli build-index "$S/JMdict_e.xml" "$S/full_index" --kanjidic "$S/kanjidic2.xml"
ls -l "$S/full_index"
```

Expected: `kanji: 13108`, `skipped kanji: 0` — every character's four numbers parsed, none skipped — and `kanji.bin` about 1.4 MB against Phase 3a's 1,314,846 bytes; `header.bin` still 24 bytes. Note the build time.

Then the upgrade path: copy the installed generation and open it with the v6 binary — `%APPDATA%\dev.jparser.clipyomi` itself is never written to:

```bash
GEN=$(ls -d "$APPDATA"/dev.jparser.clipyomi/dict/gen-* | sort -V | tail -1)
mkdir -p "$S/upgrade/dict" && cp -r "$GEN" "$S/upgrade/dict/"
./target/release/jparser-cli lookup "$S/upgrade/dict/$(basename "$GEN")" 東
```

Expected: `Error: VersionMismatch { found: 5, expected: 6 }` (the install was rebuilt to format 5 on 2026-09-11; if it reads `found: 3` or `4`, the install was never upgraded through Phase 3a, and the refusal is the same). Then:

```bash
./target/release/jparser-cli ensure-dictionary "$S/upgrade/dict" --source-dir "$APPDATA/dev.jparser.clipyomi/source"
```

Expected: a new generation with `kanji: 13108`, no download (both archives are already in the source directory), a few seconds. Record what was seen. The GUI check — `npm run tauri dev`, hover 東京都, see three asides `(grade 2, 8 strokes, freq 37, old JLPT 4)`, `(grade 2, 8 strokes, freq 74, old JLPT 3)`, `(grade 3, 11 strokes, freq 123, old JLPT 3)`; open Settings and confirm "Hide kanji readings & meanings" hides them — needs a human at a GUI and is the user's, as it was for Phase 3a. The handoff says so rather than claiming it.

- [ ] **Step 4: The master plan**

In the Phase 3 table, the "JLPT level" row (line 1195) becomes:

```markdown
| JLPT level | KANJIDIC2 `<jlpt>`, kanji only, former four-level scale | **Kanji half shipped, Phase 3b (2026-09-11)**, labelled as the former scale. Word-level JLPT declined: no official N1–N5 list exists, so no source can meet the written-provenance gate |
```

The "Kanji readings" row (line 1196) becomes:

```markdown
| Kanji readings | KANJIDIC2 (EDRDG) | **Shipped, Phase 3a (2026-09-11).** CC BY-SA 4.0, same publisher and licence as JMdict; attribution in the settings footer. Phase 3b added grade, stroke count, frequency rank and former JLPT level to the same line |
```

After the `### Phase 3a — The KANJIDIC layer — **COMPLETE**` entry's last paragraph (line 1185), add:

```markdown
### Phase 3b — The KANJIDIC misc fields — **COMPLETE**

The four `<misc>` numbers Phase 3a's parser read and discarded — school
grade, stroke count, newspaper frequency rank, former JLPT level — kept on
the same record and appended to the same kanji line as one parenthesised
aside, `(grade 2, 8 strokes, freq 37, old JLPT 4)`, which `colourLine`
paints as a paren run with no renderer or stylesheet change. Index format
5 → 6 with the header untouched, so existing installs rebuild once from the
archives they already hold. The JLPT number is labelled as the former
four-level scale and never mapped to N-levels; the Phase 3 table's JLPT row
is settled for kanji and declined for words, with the reasons in the design.
Design: `specs/2026-09-11-engine-v2-phase3b-design.md`; record:
`2026-09-11-engine-v2-phase3b-handoff.md`. Nothing under `engine/` was
touched, and §5.2's baseline block is unchanged to the cell.
```

- [ ] **Step 5: The handoff**

Create `docs/superpowers/2026-09-11-engine-v2-phase3b-handoff.md` in the shape of `2026-09-11-engine-v2-phase3a-handoff.md`, prose throughout:

- **§1 What landed** — the four commits of Tasks 1–4 by hash and one paragraph each; the file map above collapsed to prose; the insertions/deletions count from `git diff --stat <Task 1's parent>..HEAD`.
- **§2 The gates** — the measured `score` blocks for both hint states pasted verbatim, the cell-by-cell table against the required values, the `diff … && echo identical` result, both `bench` blocks with the band read stated, the snapshot check, the suite totals before (Task 1 Step 1) and after in one table, the per-file coverage rows, playwright's total.
- **§3 Where the implementation departs from the design** — expected to be empty or near it; if a departure was ruled during execution, name it and its reason as the 3a handoff does. Two items to check honestly: whether `next()` really came in shorter than 107 lines (count them), and whether any TypeScript literal needed the new fields after all.
- **§4 The upgrade path, proved headlessly** — Step 3's outputs: the real-file build's `kanji` and `skipped kanji` lines, the file sizes, the `VersionMismatch { found: 5, expected: 6 }` refusal, the `ensure-dictionary` rebuild and its time. State plainly that the GUI check is the user's.
- **§5 Open items** — carried from design §7 verbatim in substance: the former JLPT scale rendered as the former scale; frequency is a newspaper rank with weak discrimination among rare characters; no separate toggle for the aside; existing installs pay a second rebuild in two phases; the README's feature list never gained the kanji block or the aside; kun marks still raw; `commands.rs` still 2,719 lines and untouched. Plus the Phase 3a handoff's items that remain: the `-darwin` download-offer baselines still owed on a Mac; the `<?xml`-led JMdict-under-KANJIDIC-name prolog gap; the hints-off bench cell wanting a band (now read as one by this plan's Global Constraints — say so). Plus anything this phase surfaced.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md docs/superpowers/2026-09-11-engine-v2-phase3b-handoff.md
git commit -m "docs(engine-v2): Phase 3b handoff, the KANJIDIC misc fields

Records the gates as measured: the baseline cells and the bench band
unchanged in both hint states with and without kanji.bin present, the
snapshot byte-identical, the suites and per-file coverage, the real file
building 13,108 characters with none skipped, and a v5 install refused
with VersionMismatch and rebuilt from the archives it already held. Marks
the phase complete in the master plan and settles the Phase 3 table's
JLPT row for kanji while declining it for words."
```

The branch stays `feat/engine-v2`; nothing here merges.

---

## Self-review

**Spec coverage.** §1 the aside on the same line, its example lines: Task 4. §1 exclusions — fields outside `<misc>` (Task 2's `leaf_field` maps only the four), miscount stroke totals (Task 2's `Field::Strokes` arm), no N-level mapping (Task 4's `old JLPT N`), no separate toggle (no task touches settings), no CLI surface (no task touches `jparser-cli`), no layout change (Task 4 adds a column, nothing else). §1.1 every row: which fields (Task 1 struct); where they go and why parentheses (Task 4 `kanjiAside`, colour test); order (Task 4 `parts` order); absent fields omitted (Task 4 tests); grade codes (Task 4 `gradeLabel`); first stroke count (Task 2); `freq N`; `old JLPT N`; storage flat `Option`s (Task 1); format 5 → 6 header untouched (Task 1 test); malformed numbers skip-and-count (Task 2 `Pending::number` / `finish`, test); wire `null` and optional TypeScript (Task 3 pin and type); no toggle. §1.2 the master plan rows and their exact text: Task 5 Step 4. §2 licence unchanged: nothing to implement; Task 5's handoff §5 does not reopen it. §3 struct and doc comments, size, version bump, `Index::kanji` unchanged: Task 1 (no task touches `load.rs`). §3.1 parser rules, the `take` / `finish` split: Task 2. §3.2 builders and fetch untouched: the file map's "not touched" list. §4.1 backend unchanged, wire pin extended with a `null`: Task 3. §4.2 `KanjiInfo` optional fields, validator: Task 3. §4.3 `kanjiAside`, token table, `paren` run, wrapping unchanged: Task 4. §4.4 no settings change. §5 fixture cases 東/京/都/食 and the inline malformed case, README entry: Task 2. §6 every listed test — parser (Task 2), format (Tasks 1, 2), wire (Task 3), popover (Task 4, including `hide_kanji` and the `colourLine` run), lookup (Task 3), command unchanged (Task 2 Step 7 notes the 本 fixture) — and gates 1–6 (Task 5; gate 5 is Step 2's playwright run, gate 6 is Step 3). §7 open items: Task 5's handoff §5.

**Placeholder scan.** No TBDs. Every code step carries its code; every run step its command and expected outcome. Task 5's handoff description names each section's content rather than leaving the executor to invent it; the one thing it cannot pre-write is the measured numbers, which is the point.

**Type consistency.** `KanjiData { literal, on, kun, meanings, grade: Option<u8>, strokes: Option<u8>, freq: Option<u16>, jlpt: Option<u8> }` in Tasks 1, 2 and 3 agrees field for field with `KanjiInfo { …, grade?: number | null, strokes?: number | null, freq?: number | null, jlpt?: number | null }` in Tasks 3 and 4. `Pending::take(&mut self, Field, String)`, `Pending::number<T: FromStr>(&mut self, &str) -> Option<T>`, `Pending::finish(self) -> Option<KanjiData>`, `leaf_field(&BytesStart) -> Field` appear only in Task 2 and agree with each other. `has(value: number | null | undefined): value is number`, `gradeLabel(grade: number): string`, `kanjiAside(kanji: KanjiInfo): string` appear only in Task 4. `assembleTooltipText(entries, filters?, kanji = [])` is unchanged and called that way in Task 4's tests. `INDEX_FORMAT_VERSION == 6` in Task 1 is what Task 5 Step 3's `expected: 6` reads back. The fixture literals 東 (2, 8, 37, 4), 京 (none), 都 (3, 11 then 10, 123, 3), 食 (2, 9) in Task 2 Step 1 match every assertion in Task 2 Steps 2 and 6 and the GUI expectation in Task 5 Step 3, and are the 2026-09-11 file's real values except 食's deliberate trim.
