# Engine v2 Phase 3a — the KANJIDIC layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one block at the foot of the hover popover, one line per distinct kanji in the hovered surface, giving that character's on and kun readings and up to four English meanings from KANJIDIC2, with the data provisioned through the existing dictionary build and reached lazily on hover.

**Architecture:** KANJIDIC2 is fetched beside JMdict by `jmdict-source`, whose fetch and prolog checks generalise over a `Source` descriptor. `jparser` gains a streaming `kanjidic.rs` parser and a sixth index file, `kanji.bin`, a bincode `Vec<KanjiData>` sorted by codepoint that `Index::open` reads whole and `Index::kanji(ch)` binary-searches. `build_from_reader` keeps its signature as a wrapper over `build_from_readers` with an empty KANJIDIC2 reader, so every existing fixture and test builds unchanged; `ensure_dictionary`'s lazy source closure returns the pair. The main webview asks one new Tauri command, `kanji_info`, for the distinct kanji in the hovered surface through a page-lifetime cache and puts the answer beside the entries in the existing `popover-content` payload; the popover appends the block as flat text, so `colourLine` colours it as it colours everything else. A fourth gloss filter, `hide_kanji`, and a settings-window attribution footer complete the licence gate.

**Tech Stack:** Rust 2021 (`jparser`: `quick-xml` 0.36, `bincode` 1.3, `fst`, serde; `jmdict-source`: `ureq` 3, `flate2`), Tauri 2 backend in `src-tauri` (tokio `watch`), TypeScript + Vite frontend tested with Vitest (happy-dom) and Playwright.

**Spec:** `docs/superpowers/specs/2026-09-09-engine-v2-phase3a-design.md` — read it first; this plan argues from it. The KANJIDIC2 file itself was inspected on 2026-09-11: 13,108 `<character>` elements, no `<!ENTITY` declarations, `<literal>` then `<reading_meaning>` holding one or more `<rmgroup>` (each with `<reading r_type="...">` and `<meaning>` / `<meaning m_lang="...">`) and then `<nanori>` elements outside the group.

## Global Constraints

- **Baseline cells byte-identical, both hint states** (spec §6 gate 1; master plan §5.2's current-baseline block): hints on strict `0.8174 (1768)`, tolerant `0.8525 (1844)`, entry top-1 `0.9092`, compounds `0.6129 (114 / 186)` with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42; hints off `0.7342 (1588)`, `0.7924 (1714)`, `0.9093`, `0.5860 (109 / 186)`. `jparser-cli bench` scaling `1.10x` hints on, `1.05x` off. The `parse_sentences.txt` insta snapshot byte-identical. Nothing under `crates/jparser/src/engine/` other than `build.rs` is edited by this plan, so any movement is a defect in this plan's work.
- **Standing gates**, before every commit: `cargo test --workspace`, `cargo test -p jparser --features mecab`, `cargo test -p jparser --features hints-source`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`, `cargo clippy -p jparser --features hints-source --all-targets -- -D warnings`, `cargo check -p jparser --no-default-features --all-targets`, `cargo fmt --all --check`; `npm test` and `npx tsc --noEmit` before every commit that touches `src/`. Baselines at `969223e`, from the `3850108` commit message: jparser mecab 346 passed, workspace 569 passed and 1 ignored, npm 282. `cargo +1.85 check -p jparser -p jmdict-source` and `cargo llvm-cov` (per-file, see Task 9) before the final commit. Every one of these must be run, not assumed: `TA_HINTS_DICT` or `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic` must exist for the two IPADIC-pinned `engine_v2` tests.
- **No new dependency.** `quick-xml`, `bincode`, `serde`, `ureq`, `flate2`, tokio `watch` are all already in the tree. No Tauri plugin is added (see the attribution decision below).
- **`INDEX_FORMAT_VERSION` goes 4 → 5, and `IndexHeader`'s layout does not change.** This is a deliberate deviation from spec §3 ("`IndexHeader` gains `kanji: u32`"): bincode 1.x encodes the header as fixed-width fields (4 × u32 + u64 = 24 bytes), so a header with a sixth field would fail to *decode* a v4 file with an `Encoding` error before `open_with` ever compares versions, and `ensure_dictionary` would return that error instead of rebuilding — the exact upgrade path §3 relies on. The record count is `Index::kanji_count()`, read off the vector. Task 2 pins this with a test that hand-encodes a v4 header and asserts `VersionMismatch`.
- **`KanjiData` is the wire type.** No separate `KanjiInfo` Rust struct (spec §5.2 says "mirrors"): `KanjiData` already derives `Serialize` for bincode and serialises to exactly the JSON the webview wants. The TypeScript interface is named `KanjiInfo` as the spec says, and the wire pin test in `engine_v2.rs` pins `KanjiData`'s field names.
- **The meanings cap lives in the parser** (`kanjidic::MAX_MEANINGS = 4`), one place, where spec §6 lists its test.
- **`ensure_dictionary` takes one closure returning `(jmdict, kanjidic)`**, not two closures. Spec §3.2 says "it takes a pair"; one closure keeps the app's "downloading" / "building" status split where it is.
- **The popover payload's `kanji` field is omitted when the array is empty**, so a hover on a kana-only word, or on any index built without KANJIDIC2, emits exactly the payload it emits today. Every existing `popover-content` assertion stays valid except for the new `hide_kanji: false` key in `filters`.
- **The attribution footer names the licence URL as text, not a hyperlink.** Spec §2.1 says "linking". No opener plugin is installed, and a plain `<a href>` in a Tauri 2 webview navigates the settings webview itself away from the page. Adding `tauri-plugin-opener` for one link is a dependency this plan does not take; the footer's text is selectable so the URL can be copied.
- **Every existing `open_local`, `fetch_from`, `fetch_with_retry` and `resolve_from` call takes the `Source` as a new first argument.** The test-injected URL stays a separate parameter, as today, so tests keep pointing at a local listener.
- **Two tests that hand-place `JMdict_e.gz` must also hand-place `kanjidic2.xml.gz`, or they reach the real network:** `crates/jmdict-source/tests/seam.rs` and `cli_generations.rs`'s `ensure_dictionary_builds_through_the_source_dir_route`. Task 4 does both.
- **New Rust files carry the SPDX header only**, matching `jmdict.rs` (the clean-room note is for `engine/`). New TypeScript files carry the SPDX header, matching `src/render/tooltip-text.ts`.
- **Docs, code comments, commit messages and UI copy are written in normal prose**, not the session's terse style. Commits follow the repository's `type(scope): description` form with a prose body.

### Measuring the gates

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase3a"
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

The gold index is built without `--kanjidic` on purpose: the cells are a property of the parse, which never reads `kanji.bin`. Task 9 also builds it once *with* the fixture KANJIDIC2 and re-runs the hints-on `score` to show the cells do not depend on the file's presence either.

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `crates/jparser/src/kanjidic.rs` | created: streaming KANJIDIC2 reader, `MAX_MEANINGS`, `KanjidicError` | 1 |
| `crates/jparser/src/index/mod.rs` | `KanjiData`; `IndexError::Kanjidic`; `KANJI_FILE`; version 5; `BuildReport.kanji`, `.skipped_kanji`; `ensure_dictionary` pair closure; test call sites | 1, 2, 4 |
| `crates/jparser/src/lib.rs` | `pub mod kanjidic;` | 1 |
| `crates/jparser/tests/fixtures/kanjidic_mini.xml` | created: four characters | 1 |
| `crates/jparser/tests/fixtures/README.md` | entry for the new fixture | 1 |
| `crates/jparser/src/index/build.rs` | `write_index` takes and writes `kanji` | 2 |
| `crates/jparser/src/index/load.rs` | `Index.kanji`, `Index::kanji`, `Index::kanji_count`; reads `kanji.bin` | 2 |
| `crates/jparser/src/engine/build.rs` | `build_from_readers`; `build_from_reader` becomes its wrapper | 2 |
| `crates/jparser/tests/index_roundtrip.rs` | five tests | 2 |
| `crates/jmdict-source/src/lib.rs` | `Source`, `JMDICT`, `KANJIDIC`, `open_local(source, ..)`, `resolve_kanjidic`, `file` on three error variants | 3 |
| `crates/jmdict-source/src/fetch.rs` | descriptor-driven `fetch_from`, `fetch_with_retry`, `fetch_kanjidic` | 3 |
| `crates/jmdict-source/tests/download.rs` | call sites; one destructure | 3 |
| `src-tauri/src/commands.rs` | error literal (3); download closure, `with_fallback` (4); `lookup_kanji`, `kanji_info` (5); `hide_kanji` merge and test (8) | 3, 4, 5, 8 |
| `crates/jparser/src/index/generations.rs` | `build_new(root, jmdict, kanjidic)` | 4 |
| `crates/jparser/tests/generations_torn_build.rs` | call sites | 4 |
| `crates/jmdict-source/tests/seam.rs` | places KANJIDIC2 too; pair closure | 4 |
| `src-tauri/src/test_support.rs` | `FIXTURE_KANJIDIC`; pair closure | 4 |
| `crates/jparser/src/bin/jparser-cli/main.rs` | `--kanjidic` on `build-index` and `ensure-dictionary`; `fetch-source` fetches both | 4 |
| `crates/jparser/tests/cli_generations.rs` | source-dir test places KANJIDIC2; `build-index --kanjidic` test | 4 |
| `src-tauri/src/main.rs` | registers `kanji_info` | 5 |
| `crates/jparser/tests/engine_v2.rs` | wire pin for `KanjiData` | 5 |
| `src/types.ts` | `KanjiInfo`; `hide_kanji` on `GlossFilters` and `Settings` | 5, 6 |
| `src/render/tooltip-text.ts`, `src/render/tooltip.ts`, `src/popover.ts` | the block | 6 |
| `src/render/tooltip-text.test.ts`, `src/popover.test.ts` | block tests | 6 |
| `src/render/tooltip-colour.ts` | `isIdeograph` exported | 7 |
| `src/kanji-lookup.ts`, `src/kanji-lookup.test.ts` | created: distinct-kanji extraction and the cached lookup | 7 |
| `src/main.ts` | `openFor` asks `kanji_info`; `hide_kanji` filter; first-run copy | 7 |
| `src/main-tooltip.test.ts`, `src/main.test.ts` | flushes and `hide_kanji: false`; two new tests | 7 |
| `src/settings.ts`, `src/settings.test.ts`, `src/styles/settings.css` | fourth checkbox; attribution footer | 8 |
| `src-tauri/src/settings.rs`, `src-tauri/src/settings/tests.rs` | `hide_kanji` | 8 |
| `README.md` | names KANJIDIC2 | 8 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | §7 Phase 3a entry; Phase 3 table row | 9 |
| `docs/superpowers/2026-09-11-engine-v2-phase3a-handoff.md` | created | 9 |

---

### Task 1: The KANJIDIC2 parser

**Files:**
- Create: `crates/jparser/src/kanjidic.rs`
- Create: `crates/jparser/tests/fixtures/kanjidic_mini.xml`
- Modify: `crates/jparser/src/index/mod.rs` — `KanjiData` after `EntryData` (line 82); `IndexError::Kanjidic` after `Jmdict` (line 102); the two exhaustive `match`es at lines 154–159 and 202–209
- Modify: `crates/jparser/src/lib.rs` — `pub mod kanjidic;` after `pub mod jmdict;` (line 18)
- Modify: `crates/jparser/tests/fixtures/README.md` — new section

**Interfaces:**
- Consumes: `quick_xml::Reader`, `BytesStart::try_get_attribute`, `BytesText::unescape` (0.36).
- Produces: `pub struct KanjiData { literal: char, on: Vec<String>, kun: Vec<String>, meanings: Vec<String> }` in `jparser::index` (serde `Serialize`/`Deserialize`, `Debug, Clone, PartialEq, Eq`); `pub fn kanjidic::parse_characters<R: BufRead>(R) -> KanjidicReader<R>` iterating `Result<KanjiData, KanjidicError>` with `skipped_count()`; `pub const kanjidic::MAX_MEANINGS: usize = 4`; `IndexError::Kanjidic(KanjidicError)`. Task 2 calls `parse_characters` and stores `KanjiData`.

- [ ] **Step 1: Write the fixture**

Create `crates/jparser/tests/fixtures/kanjidic_mini.xml`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!-- This file contains material from KANJIDIC2, Copyright (C) EDRDG, used
     under CC BY-SA 4.0 (https://www.edrdg.org/edrdg/licence.html). Four
     characters, structure as published; see README.md in this directory. -->
<!DOCTYPE kanjidic2 [
<!ELEMENT kanjidic2 (header,character*)>
]>
<kanjidic2>
<header>
<file_version>4</file_version>
<database_version>2026-254</database_version>
<date_of_creation>2026-09-11</date_of_creation>
</header>
<character>
<literal>東</literal>
<codepoint>
<cp_value cp_type="ucs">6771</cp_value>
</codepoint>
<misc>
<grade>2</grade>
<stroke_count>8</stroke_count>
<freq>37</freq>
<jlpt>4</jlpt>
</misc>
<reading_meaning>
<rmgroup>
<reading r_type="pinyin">dong1</reading>
<reading r_type="korean_h">동</reading>
<reading r_type="ja_on">トウ</reading>
<reading r_type="ja_kun">ひがし</reading>
<meaning>east</meaning>
<meaning m_lang="fr">Est</meaning>
<meaning m_lang="es">Este</meaning>
</rmgroup>
<nanori>あずま</nanori>
<nanori>ひが</nanori>
</reading_meaning>
</character>
<character>
<literal>京</literal>
<codepoint>
<cp_value cp_type="ucs">4eac</cp_value>
</codepoint>
<reading_meaning>
<rmgroup>
<reading r_type="ja_on">キョウ</reading>
<reading r_type="ja_on">ケイ</reading>
<reading r_type="ja_on">キン</reading>
<reading r_type="ja_kun">みやこ</reading>
<meaning>capital</meaning>
<meaning>10**16</meaning>
<meaning>metropolis</meaning>
<meaning>ten quadrillion</meaning>
<meaning>a fifth meaning, past the cap</meaning>
</rmgroup>
</reading_meaning>
</character>
<character>
<literal>都</literal>
<codepoint>
<cp_value cp_type="ucs">90fd</cp_value>
</codepoint>
<reading_meaning>
<rmgroup>
<reading r_type="ja_on">ト</reading>
<reading r_type="ja_on">ツ</reading>
<reading r_type="ja_kun">みやこ</reading>
<meaning>metropolis</meaning>
<meaning>capital</meaning>
<meaning>all</meaning>
<meaning>everything</meaning>
</rmgroup>
</reading_meaning>
</character>
<character>
<literal>食</literal>
<codepoint>
<cp_value cp_type="ucs">98df</cp_value>
</codepoint>
<reading_meaning>
<rmgroup>
<reading r_type="ja_on">ショク</reading>
<reading r_type="ja_on">ジキ</reading>
<reading r_type="ja_kun">く.う</reading>
<reading r_type="ja_kun">く.らう</reading>
<reading r_type="ja_kun">た.べる</reading>
<reading r_type="ja_kun">は.む</reading>
<reading r_type="ja_kun">-ぐ.い</reading>
<meaning>eat</meaning>
<meaning>food</meaning>
</rmgroup>
</reading_meaning>
</character>
</kanjidic2>
```

Append to `crates/jparser/tests/fixtures/README.md`:

```markdown
## `kanjidic_mini.xml`

Four characters (東, 京, 都, 食) in **KANJIDIC2**'s published structure,
trimmed to the elements the reader keeps or must skip: a foreign-language
`<meaning m_lang>`, `<nanori>`, non-Japanese `r_type`s, a character with five
English meanings (the cap is four), and kun readings carrying the `.` and `-`
marks. Used by `src/kanjidic.rs`'s own tests, `tests/index_roundtrip.rs`, and
`src-tauri`'s command tests.

- **Source:** Electronic Dictionary Research and Development Group (EDRDG),
  <http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz>
- **Retrieved:** 2026-09-11 (KANJIDIC2 is regenerated continuously, like
  JMdict, so the committed excerpt is the reproducible artifact)
- **Licence:** Creative Commons Attribution-ShareAlike 4.0 International
  (CC BY-SA 4.0), <https://www.edrdg.org/edrdg/licence.html>
- **Notice:** This file contains material from KANJIDIC2, Copyright (C) EDRDG.

Third-party **data**, not source, on the same footing as `jmdict_subset.xml`
above: it carries the EDRDG notice in its own header and is not relicensed.
```

- [ ] **Step 2: Add `KanjiData` and the error variant**

In `crates/jparser/src/index/mod.rs`, after the `EntryData` struct (line 82), insert:

```rust
/// One KANJIDIC2 character, as the popover needs it.
///
/// Also the wire shape `kanji_info` returns to the webview: `src/types.ts`'s
/// `KanjiInfo` names these four fields, and
/// `kanji_data_serializes_to_the_documented_wire_shape` in
/// `tests/engine_v2.rs` pins them, so neither side can drift alone.
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
}
```

In `IndexError`, after the `Jmdict` variant:

```rust
    #[error("reading KANJIDIC2 failed: {0}")]
    Kanjidic(#[from] crate::kanjidic::KanjidicError),
```

In `ensure_dictionary`'s match (line 154), add the arm between `Jmdict` and `GenerationExists`:

```rust
            | Err(e @ IndexError::Kanjidic(_))
```

and the same line in `open_published_or_newest`'s match (line 205), between `Jmdict` and `VersionMismatch`.

In `crates/jparser/src/lib.rs`, after `pub mod jmdict;`:

```rust
pub mod kanjidic;
```

- [ ] **Step 3: Write the parser with its failing tests**

Create `crates/jparser/src/kanjidic.rs`:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Streaming KANJIDIC2 XML reader.
//!
//! One `<character>` per kanji. Kept: `<literal>`, every `<reading>` whose
//! `r_type` is `ja_on` or `ja_kun`, and every `<meaning>` with no `m_lang`
//! attribute (English), capped at [`MAX_MEANINGS`]. Read and discarded:
//! `<nanori>`, every other `r_type` (pinyin, korean, vietnam), every other
//! language, and `<misc>`'s grade, stroke count, frequency and JLPT fields —
//! one line each to keep, the day a layer wants them (Phase 3a design §1).
//!
//! KANJIDIC2 declares no entities in its DTD (checked against the 2026-09-11
//! file: zero `<!ENTITY` lines), so the entity spike `jmdict.rs` records does
//! not recur here and `unescape()` is enough. The file does carry a DTD,
//! which `quick-xml` surfaces as a `DocType` event this reader ignores.

use std::io::BufRead;

use quick_xml::errors::IllFormedError;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Error, Reader};

use crate::index::KanjiData;

/// English meanings kept per character. KANJIDIC2 gives up to about ten for
/// a common character; four is the decode-from-parts need (design §1.1).
pub const MAX_MEANINGS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum KanjidicError {
    #[error("malformed KANJIDIC2 XML: {0}")]
    Xml(#[from] quick_xml::Error),
}

pub fn parse_characters<R: BufRead>(reader: R) -> KanjidicReader<R> {
    KanjidicReader {
        reader: Reader::from_reader(reader),
        buf: Vec::new(),
        skipped: 0,
        done: false,
    }
}

pub struct KanjidicReader<R: BufRead> {
    reader: Reader<R>,
    buf: Vec<u8>,
    skipped: usize,
    done: bool,
}

impl<R: BufRead> KanjidicReader<R> {
    /// Characters skipped because their `<literal>` was not exactly one
    /// character. Never silently discarded — the caller surfaces this count.
    pub fn skipped_count(&self) -> usize {
        self.skipped
    }
}

/// Which leaf's text is being accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    Literal,
    On,
    Kun,
    Meaning,
}

/// `r_type="ja_on"` or `"ja_kun"` selects a field; every other reading —
/// pinyin, korean, vietnam — is skipped.
fn reading_field(e: &BytesStart) -> Field {
    match e.try_get_attribute("r_type") {
        Ok(Some(attr)) => match attr.value.as_ref() {
            b"ja_on" => Field::On,
            b"ja_kun" => Field::Kun,
            _ => Field::None,
        },
        _ => Field::None,
    }
}

/// A `<meaning>` with no `m_lang` attribute is English.
fn meaning_field(e: &BytesStart) -> Field {
    match e.try_get_attribute("m_lang") {
        Ok(None) => Field::Meaning,
        _ => Field::None,
    }
}

impl<R: BufRead> Iterator for KanjidicReader<R> {
    type Item = Result<KanjiData, KanjidicError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let mut in_character = false;
        let mut literal = String::new();
        let mut on: Vec<String> = Vec::new();
        let mut kun: Vec<String> = Vec::new();
        let mut meanings: Vec<String> = Vec::new();
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
                    field = match e.local_name().as_ref() {
                        b"literal" => Field::Literal,
                        b"reading" => reading_field(&e),
                        b"meaning" => meaning_field(&e),
                        _ => Field::None,
                    };
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
                    let value = text.trim().to_string();
                    match field {
                        Field::Literal => literal = value,
                        Field::On => on.push(value),
                        Field::Kun => kun.push(value),
                        Field::Meaning => {
                            if meanings.len() < MAX_MEANINGS {
                                meanings.push(value);
                            }
                        }
                        Field::None => {}
                    }
                    field = Field::None;
                    text.clear();

                    if e.local_name().as_ref() == b"character" {
                        in_character = false;
                        let mut chars = literal.chars();
                        match (chars.next(), chars.next()) {
                            (Some(ch), None) => {
                                return Some(Ok(KanjiData {
                                    literal: ch,
                                    on: std::mem::take(&mut on),
                                    kun: std::mem::take(&mut kun),
                                    meanings: std::mem::take(&mut meanings),
                                }));
                            }
                            _ => {
                                self.skipped += 1;
                                literal.clear();
                                on.clear();
                                kun.clear();
                                meanings.clear();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn characters() -> Vec<KanjiData> {
        let xml = include_str!("../tests/fixtures/kanjidic_mini.xml");
        parse_characters(std::io::Cursor::new(xml))
            .collect::<Result<Vec<_>, _>>()
            .expect("fixture must parse")
    }

    fn find(literal: char) -> KanjiData {
        characters()
            .into_iter()
            .find(|k| k.literal == literal)
            .unwrap_or_else(|| panic!("{literal} is in the fixture"))
    }

    #[test]
    fn reads_every_character_in_document_order() {
        let literals: Vec<char> = characters().iter().map(|k| k.literal).collect();
        assert_eq!(literals, vec!['東', '京', '都', '食']);
    }

    #[test]
    fn reads_on_and_kun_readings_in_order() {
        let k = find('食');
        assert_eq!(k.on, vec!["ショク", "ジキ"]);
        assert_eq!(k.kun, vec!["く.う", "く.らう", "た.べる", "は.む", "-ぐ.い"]);
    }

    /// The `.` okurigana boundary and the `-` affix mark are KANJIDIC2's own
    /// notation and are kept verbatim (design §7): stripping them is a
    /// popover-text question, not a data one.
    #[test]
    fn keeps_okurigana_and_affix_marks_verbatim() {
        let k = find('食');
        assert!(k.kun.contains(&"た.べる".to_string()));
        assert!(k.kun.contains(&"-ぐ.い".to_string()));
    }

    #[test]
    fn drops_nanori_and_non_japanese_readings() {
        let k = find('東');
        assert_eq!(k.on, vec!["トウ"]);
        assert_eq!(k.kun, vec!["ひがし"]);
        let all: Vec<&String> = k.on.iter().chain(&k.kun).collect();
        assert!(!all.iter().any(|r| *r == "あずま" || *r == "dong1" || *r == "동"));
    }

    #[test]
    fn keeps_english_meanings_only() {
        assert_eq!(find('東').meanings, vec!["east"]);
    }

    #[test]
    fn caps_meanings_at_four() {
        let k = find('京');
        assert_eq!(k.meanings.len(), MAX_MEANINGS);
        assert_eq!(
            k.meanings,
            vec!["capital", "10**16", "metropolis", "ten quadrillion"]
        );
        // Exactly four is not truncated.
        assert_eq!(find('都').meanings.len(), 4);
    }

    #[test]
    fn skips_a_character_with_a_bad_literal_and_counts_it() {
        let xml = "<kanjidic2>\
            <character><literal>ab</literal></character>\
            <character><literal></literal></character>\
            <character><literal>本</literal><reading_meaning><rmgroup>\
            <reading r_type=\"ja_on\">ホン</reading></rmgroup></reading_meaning></character>\
            </kanjidic2>";
        let mut reader = parse_characters(std::io::Cursor::new(xml));
        let all: Vec<_> = reader.by_ref().collect();
        let ok: Vec<_> = all.into_iter().filter_map(Result::ok).collect();
        assert_eq!(ok.len(), 1, "the good character must still be returned");
        assert_eq!(ok[0].literal, '本');
        assert_eq!(ok[0].on, vec!["ホン"]);
        assert_eq!(reader.skipped_count(), 2);
    }

    #[test]
    fn decodes_a_standard_escape_in_a_meaning() {
        let xml = "<kanjidic2><character><literal>本</literal>\
            <reading_meaning><rmgroup><meaning>book &amp; origin</meaning></rmgroup>\
            </reading_meaning></character></kanjidic2>";
        let k = parse_characters(std::io::Cursor::new(xml))
            .next()
            .expect("one character")
            .expect("parses");
        assert_eq!(k.meanings, vec!["book & origin"]);
    }

    #[test]
    fn reports_malformed_xml_as_an_error() {
        let bad = "<kanjidic2><character><literal>本</literal>";
        let result: Result<Vec<_>, _> = parse_characters(std::io::Cursor::new(bad)).collect();
        assert!(result.is_err());
    }

    /// `build_from_reader` supplies `std::io::empty()`; it must read as a
    /// KANJIDIC2 with no characters, not as an error.
    #[test]
    fn an_empty_reader_yields_nothing() {
        let mut reader = parse_characters(std::io::empty());
        assert!(reader.next().is_none());
        assert_eq!(reader.skipped_count(), 0);
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p jparser kanjidic`
Expected: 10 passed. If `try_get_attribute` does not resolve, the `quick-xml` in `Cargo.lock` is not 0.36 — stop and report; do not bump.

- [ ] **Step 5: Standing gates, then commit**

Run: `cargo test -p jparser && cargo clippy -p jparser --all-targets -- -D warnings && cargo check -p jparser --no-default-features --all-targets && cargo fmt --all --check`
Expected: all green; the exhaustive matches in `mod.rs` compile because both arms were added.

```bash
git add crates/jparser/src/kanjidic.rs crates/jparser/src/index/mod.rs crates/jparser/src/lib.rs crates/jparser/tests/fixtures/kanjidic_mini.xml crates/jparser/tests/fixtures/README.md
git commit -m "feat(jparser): streaming KANJIDIC2 reader, the Phase 3a layer's parser

One character per <character>: the literal, the ja_on and ja_kun readings in
document order with KANJIDIC2's okurigana and affix marks kept verbatim, and
the first four English meanings. Nanori, every other reading type, every other
language, and the grade, stroke, frequency and JLPT fields are read and
discarded. KANJIDIC2 declares no entities, so unescape() is enough.

KanjiData lives in index::mod because it is also the on-disk record and the
wire shape; the next commit writes it into a sixth index file."
```

---

### Task 2: The sixth index file, and `build_from_readers`

**Files:**
- Modify: `crates/jparser/src/index/mod.rs` — `INDEX_FORMAT_VERSION` (line 25), file constants (line 27–31), `BuildReport` (line 85–92)
- Modify: `crates/jparser/src/index/build.rs` — `write_index` signature and body
- Modify: `crates/jparser/src/index/load.rs` — `Index` struct, `open_with`, new methods; the "five files" comment at line 61
- Modify: `crates/jparser/src/engine/build.rs` — `build_from_reader` becomes a wrapper; new `build_from_readers`
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` — `build-index` prints the two new counts (lines 268–281)
- Test: `crates/jparser/tests/index_roundtrip.rs`

**Interfaces:**
- Consumes: `kanjidic::parse_characters`, `KanjiData` (Task 1).
- Produces: `pub const index::KANJI_FILE: &str = "kanji.bin"`; `INDEX_FORMAT_VERSION = 5`; `BuildReport { keys, records, entries, kanji, skipped_entries, skipped_kanji }`; `pub fn engine::build::build_from_readers<J: BufRead, K: BufRead>(xml: J, kanjidic: K, out_dir: &Path) -> Result<BuildReport, IndexError>`; `build_from_reader<R: BufRead>(xml: R, out_dir: &Path)` unchanged; `Index::kanji(&self, ch: char) -> Option<&KanjiData>`; `Index::kanji_count(&self) -> usize`. Tasks 4 and 5 call these.

- [ ] **Step 1: Write the failing tests**

In `crates/jparser/tests/index_roundtrip.rs`, change the imports and helpers at the top:

```rust
use jparser::engine::build::{build_from_reader, build_from_readers};
use jparser::engine::kana::fold;
use jparser::index::load::Index;
use jparser::index::{BuildReport, IndexError, INDEX_FORMAT_VERSION};
use jparser::WordFlags;

const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");
const KANJIDIC_FIXTURE: &str = include_str!("fixtures/kanjidic_mini.xml");
```

and add beside `build`:

```rust
/// The two-reader build: JMdict plus the four-character KANJIDIC2 fixture.
fn build_with_kanji(dir: &Path) -> BuildReport {
    build_from_readers(
        std::io::Cursor::new(FIXTURE),
        std::io::Cursor::new(KANJIDIC_FIXTURE),
        dir,
    )
    .expect("build must succeed")
}
```

In `writes_all_expected_files`, add `"kanji.bin",` after `"entries.idx",`.

Append at the end of the file:

```rust
/// Phase 3a: the sixth file. Records are sorted by codepoint on write and
/// binary-searched on read; the fixture's document order is 東 京 都 食,
/// which is not codepoint order, so a writer that forgot to sort would miss.
#[test]
fn a_kanjidic_record_round_trips() {
    let dir = tmpdir("kanji-hit");
    let report = build_with_kanji(&dir);
    assert_eq!(report.kanji, 4);
    assert_eq!(report.skipped_kanji, 0);

    let index = Index::open(&dir).unwrap();
    assert_eq!(index.kanji_count(), 4);
    for literal in ['東', '京', '都', '食'] {
        assert!(index.kanji(literal).is_some(), "{literal} must be found");
    }
    let east = index.kanji('東').expect("東");
    assert_eq!(east.on, vec!["トウ"]);
    assert_eq!(east.kun, vec!["ひがし"]);
    assert_eq!(east.meanings, vec!["east"]);
}

#[test]
fn a_character_without_a_record_misses() {
    let dir = tmpdir("kanji-miss");
    build_with_kanji(&dir);
    let index = Index::open(&dir).unwrap();
    assert!(index.kanji('あ').is_none(), "kana have no record");
    // A JMdict headword character that the KANJIDIC2 fixture does not carry.
    assert!(index.kanji('本').is_none());
}

/// `build_from_reader` keeps its signature and supplies no KANJIDIC2, so
/// every pre-existing fixture and probe builds a valid index whose kanji
/// lookups all miss — never an error.
#[test]
fn an_index_built_without_kanjidic_opens_and_misses() {
    let dir = tmpdir("kanji-none");
    let report = build(&dir);
    assert_eq!(report.kanji, 0);
    assert!(dir.join("kanji.bin").exists(), "the file is written even when empty");

    let index = Index::open(&dir).unwrap();
    assert_eq!(index.kanji_count(), 0);
    assert!(index.kanji('東').is_none());
}

/// The upgrade path. An installed v4 index has a 24-byte header — four `u32`
/// counts and the `u64` fingerprint — and `ensure_dictionary` rebuilds on
/// exactly `VersionMismatch`. That only works if a v5 loader can still
/// *decode* a v4 header far enough to read its version, which is why
/// `IndexHeader` did not gain a field for this format (plan §Global
/// Constraints): a sixth field would turn this into an `Encoding` error
/// before the version is ever compared, and the app would report a failure
/// a download cannot fix instead of rebuilding.
#[test]
fn a_v4_header_is_refused_with_a_version_mismatch() {
    let dir = tmpdir("kanji-v4-header");
    build(&dir);
    let index = Index::open(&dir).unwrap();
    let fingerprint = index.header().conjugation_fingerprint;
    drop(index);

    // The v4 layout, byte for byte: version, keys, records, entries, fingerprint.
    let v4 = bincode::serialize(&(4u32, 3u32, 3u32, 3u32, fingerprint)).unwrap();
    assert_eq!(v4.len(), 24);
    std::fs::write(dir.join("header.bin"), v4).unwrap();

    match Index::open(&dir).unwrap_err() {
        IndexError::VersionMismatch { found, expected } => {
            assert_eq!(found, 4);
            assert_eq!(expected, INDEX_FORMAT_VERSION);
        }
        other => panic!("a v4 header must be a version mismatch, got {other:?}"),
    }
}

/// A build whose KANJIDIC2 is unreadable fails the build, the way a bad
/// JMdict does — knowingly, over a silent-degrade path (design §4).
#[test]
fn a_malformed_kanjidic_fails_the_build() {
    let dir = tmpdir("kanji-malformed");
    let err = build_from_readers(
        std::io::Cursor::new(FIXTURE),
        std::io::Cursor::new("<kanjidic2><character><literal>本"),
        &dir,
    )
    .unwrap_err();
    assert!(matches!(err, IndexError::Kanjidic(_)), "got {err:?}");
}
```

`bincode` is not a dev-dependency of the test crate — it is a regular dependency of `jparser`, and integration tests can use the package's own dependencies only if they are declared; check `cargo test -p jparser --test index_roundtrip` compiles. If it does not, add `bincode = "1.3"` under `[dev-dependencies]` in `crates/jparser/Cargo.toml` (the same version already in `[dependencies]`, so nothing new enters the tree).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p jparser --test index_roundtrip`
Expected: compile errors — `build_from_readers`, `kanji`, `kanji_count`, `report.kanji` do not exist.

- [ ] **Step 3: The format constants and the report**

In `crates/jparser/src/index/mod.rs`:

```rust
/// Bumped whenever the on-disk layout changes. A mismatch forces a rebuild; the
/// loader must never try to read an index it does not recognize.
///
/// 4 (Phase 1b-ii): `StoredRecord` lost `verb_type`.
/// 5 (Phase 3a): a sixth file, `kanji.bin`. `IndexHeader`'s own layout is
/// unchanged, and must stay so unless `open_with` learns to read the version
/// before decoding the rest — see `a_v4_header_is_refused_with_a_version_mismatch`.
pub const INDEX_FORMAT_VERSION: u32 = 5;

pub const HEADER_FILE: &str = "header.bin";
pub const FST_FILE: &str = "keys.fst";
pub const RECORDS_FILE: &str = "records.bin";
pub const ENTRIES_FILE: &str = "entries.bin";
pub const ENTRIES_INDEX_FILE: &str = "entries.idx";
/// A bincode `Vec<KanjiData>` sorted by codepoint, read whole on open.
pub const KANJI_FILE: &str = "kanji.bin";
```

`BuildReport`:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildReport {
    pub keys: usize,
    pub records: usize,
    pub entries: usize,
    /// KANJIDIC2 characters stored. Zero for a `build_from_reader` build.
    pub kanji: usize,
    /// Malformed JMdict entries skipped. Surfaced, never silently dropped.
    pub skipped_entries: usize,
    /// KANJIDIC2 characters skipped for a literal that was not one character.
    pub skipped_kanji: usize,
}
```

- [ ] **Step 4: The writer**

In `crates/jparser/src/index/build.rs`, the import list gains `KanjiData` and `KANJI_FILE`, the doc comment says "six index files", and the signature and tail become:

```rust
pub(crate) fn write_index(
    by_key: BTreeMap<String, Vec<StoredRecord>>,
    entries: Vec<EntryData>,
    record_count: usize,
    mut kanji: Vec<KanjiData>,
    fingerprint: u64,
    out_dir: &Path,
) -> Result<(), IndexError> {
```

and, after the `ENTRIES_INDEX_FILE` write and before the header:

```rust
    // Sorted here, in the format's one writer, because the loader binary
    // searches by codepoint; a duplicate literal would make that search
    // ambiguous, so the later one is dropped.
    kanji.sort_by_key(|k| k.literal);
    kanji.dedup_by_key(|k| k.literal);
    std::fs::write(out_dir.join(KANJI_FILE), bincode::serialize(&kanji)?)?;
```

- [ ] **Step 5: The loader**

In `crates/jparser/src/index/load.rs`, the import list gains `KanjiData` and `KANJI_FILE`; `Index` gains a field after `entry_offsets`:

```rust
    /// Every KANJIDIC2 record, sorted by codepoint. Resident, not mapped:
    /// 13k small records earn neither an mmap nor an FST, and a whole-file
    /// read keeps `open`'s failure modes the ones it has.
    kanji: Vec<KanjiData>,
```

In `open_with`, after `entry_offsets` is read:

```rust
        let kanji: Vec<KanjiData> = bincode::deserialize(&std::fs::read(dir.join(KANJI_FILE))?)?;
```

and `kanji,` in the `Ok(Self { .. })` literal. The comment at line 61 becomes "it reads six files in sequence" and the `open` doc's "The header is written last" paragraph is unchanged. After `entry_count`, add:

```rust
    /// The KANJIDIC2 record for `ch`, or `None`: every non-kanji character,
    /// every character KANJIDIC2 does not carry, and every character of an
    /// index built through `build_from_reader`, which supplies no KANJIDIC2.
    pub fn kanji(&self, ch: char) -> Option<&KanjiData> {
        self.kanji
            .binary_search_by_key(&ch, |k| k.literal)
            .ok()
            .map(|i| &self.kanji[i])
    }

    /// Number of KANJIDIC2 records. Cheap: resident after `open`.
    pub fn kanji_count(&self) -> usize {
        self.kanji.len()
    }
```

- [ ] **Step 6: The builder**

In `crates/jparser/src/engine/build.rs`, the imports gain `use crate::index::KanjiData;` and `use crate::kanjidic::parse_characters;`. Replace the `build_from_reader` doc and signature with:

```rust
/// Builds an index from JMdict `xml` alone: [`build_from_readers`] with an
/// empty KANJIDIC2 reader, so `Index::kanji` answers `None` for every
/// character. Kept with this signature so every fixture, test, example and
/// probe that predates the kanji layer builds unchanged (Phase 3a design §3.2).
pub fn build_from_reader<R: BufRead>(xml: R, out_dir: &Path) -> Result<BuildReport, IndexError> {
    build_from_readers(xml, std::io::empty(), out_dir)
}

/// Builds an index from JMdict `xml` and KANJIDIC2 `kanjidic` into `out_dir`.
pub fn build_from_readers<J: BufRead, K: BufRead>(
    xml: J,
    kanjidic: K,
    out_dir: &Path,
) -> Result<BuildReport, IndexError> {
```

The body is the old body, with two additions. After `let skipped_entries = reader.skipped_count();`:

```rust
    // Read after JMdict, not interleaved: the two files are independent and
    // a failure in either fails the build the same way.
    let mut kanji_reader = parse_characters(kanjidic);
    let kanji: Vec<KanjiData> = kanji_reader.by_ref().collect::<Result<_, _>>()?;
    let skipped_kanji = kanji_reader.skipped_count();
    let kanji_count = kanji.len();
```

and the tail:

```rust
    write_index(by_key, entries, record_count, kanji, fingerprint(), out_dir)?;

    Ok(BuildReport {
        keys,
        records: record_count,
        entries: entry_count,
        kanji: kanji_count,
        skipped_entries,
        skipped_kanji,
    })
```

Update the module doc's first sentence to "Builds the index from JMdict XML and KANJIDIC2 XML".

- [ ] **Step 7: The CLI's report lines**

In `crates/jparser/src/bin/jparser-cli/main.rs`, `Command::BuildIndex`'s arm prints two more lines, aligned to the existing 21-column labels, after `records:`:

```rust
            println!("kanji:               {}", report.kanji);
            println!("skipped kanji:       {}", report.skipped_kanji);
```

and a second warning:

```rust
            if report.skipped_kanji > 0 {
                eprintln!(
                    "warning: {} malformed KANJIDIC2 characters were skipped",
                    report.skipped_kanji
                );
            }
```

(`--kanjidic` itself arrives in Task 4; for now `build-index` still calls `build_from_reader`, so these print 0.)

- [ ] **Step 8: Run the tests**

Run: `cargo test -p jparser --test index_roundtrip`
Expected: every test passes, including the five new ones and the two pre-existing header tests (`rejects_a_header_with_the_wrong_format_version`, `rejects_a_header_with_the_wrong_conjugation_fingerprint` — the latter relies on the fingerprint being the header's last field, which the unchanged layout preserves).

Run: `cargo test -p jparser --features mecab`
Expected: green. The `engine_v2` insta snapshot is untouched: `git status` shows no `*.snap.new`. If `cli_parse`, `cli_score` or `cli_label` pin `build-index`'s stdout verbatim, they will now fail on the two added lines; update those expectations and nothing else.

- [ ] **Step 9: Standing gates, then commit**

Run the standing gates from Global Constraints (workspace test and clippy included: `src-tauri` compiles against `Index` and must still build).

```bash
git add crates/jparser
git commit -m "feat(jparser): kanji.bin, the sixth index file, and build_from_readers

INDEX_FORMAT_VERSION goes 4 to 5 so an existing install rebuilds on first
launch, through the VersionMismatch arm ensure_dictionary already has. The
header's layout is deliberately unchanged: bincode decodes it as fixed-width
fields, so a header with a sixth field would fail to decode a v4 file before
the version was ever compared, and the rebuild path would report a failure
instead. A test hand-encodes a v4 header and pins VersionMismatch.

build_from_reader keeps its signature as a wrapper over build_from_readers
with an empty KANJIDIC2 reader, so nothing that builds a fixture index changes;
Index::kanji answers None on those. Records are sorted by codepoint in the
format's one writer and binary-searched on read."
```

---

### Task 3: `jmdict-source` learns a second source

**Files:**
- Modify: `crates/jmdict-source/src/lib.rs`
- Modify: `crates/jmdict-source/src/fetch.rs`
- Modify: `crates/jmdict-source/tests/download.rs` — call sites and one destructure
- Modify: `src-tauri/src/commands.rs:1714` — one struct literal gains a field

**Interfaces:**
- Produces: `pub struct Source { url, file_name, prologs, max_bytes }`; `pub const JMDICT: Source`; `pub const KANJIDIC: Source`; `pub const SOURCE_FILE: &str` (unchanged value, now `JMDICT.file_name`); `pub fn open_local(source: &Source, path: &Path)`; `pub fn resolve(dir)` (unchanged); `pub fn resolve_kanjidic(dir: &Path) -> io::Result<Box<dyn BufRead>>`; `pub fn resolve_from(source: &Source, url: &str, dir: &Path, backoff: Duration)`; `fetch::fetch(dir)` (unchanged); `fetch::fetch_kanjidic(dir: &Path) -> Result<PathBuf, SourceError>`; `fetch::fetch_from(source, url, dir)`; `fetch::fetch_with_retry(source, url, dir, backoff)`; `SourceError::Http`, `NotAnArchive`, `TooManyAttempts` each gain `file: &'static str`. Task 4 calls `resolve_kanjidic` and `fetch_kanjidic`.

- [ ] **Step 1: Write the failing tests**

In `crates/jmdict-source/src/lib.rs`'s test module, append:

```rust
    const KANJIDIC_XML: &[u8] = b"<?xml version=\"1.0\"?><kanjidic2></kanjidic2>";

    #[test]
    fn the_kanjidic_descriptor_is_the_edrdg_one() {
        assert_eq!(KANJIDIC.file_name, "kanjidic2.xml.gz");
        // Plain HTTP on the same host as JMdict, for the same reason: this
        // crate has no TLS backend. Verified reachable 2026-09-11 (HTTP 200,
        // Content-Length 1488571).
        assert_eq!(
            KANJIDIC.url,
            "http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz"
        );
        assert_eq!(JMDICT.url, "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz");
        assert_eq!(SOURCE_FILE, JMDICT.file_name);
    }

    /// Each of KANJIDIC2's three legitimate openings passes the prolog check.
    #[test]
    fn every_kanjidic_prolog_is_accepted() {
        for (i, head) in [
            &b"<?xml version=\"1.0\"?><kanjidic2></kanjidic2>"[..],
            b"<!DOCTYPE kanjidic2 [\n]><kanjidic2></kanjidic2>",
            b"<kanjidic2></kanjidic2>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-kanjidic-prolog-{i}"));
            let path = dir.join(KANJIDIC.file_name);
            std::fs::write(&path, head).expect("write");
            assert_eq!(
                read_all(open_local(&KANJIDIC, &path).expect("open")),
                *head
            );
        }
    }

    /// The check is per source: a JMdict file saved under the KANJIDIC2 name
    /// would parse as a KANJIDIC2 with zero characters and build an index
    /// with no kanji block, silently. It is refused at the only boundary
    /// where it is still recognisable as the wrong file.
    #[test]
    fn a_jmdict_file_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-wrong-file");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, XML).expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(matches!(err, SourceError::NotAnArchive { .. }), "got {err:?}");
        let msg = err.to_string();
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
        assert!(!msg.contains("JMdict_e.gz"), "named the wrong file: {msg}");
    }

    #[test]
    fn an_html_page_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-html");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, b"<!DOCTYPE html><html><body>404</body></html>").expect("write");
        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(matches!(err, SourceError::NotAnArchive { .. }), "got {err:?}");
    }

    #[test]
    fn a_present_kanjidic_archive_is_opened_without_downloading() {
        let dir = scratch("resolve-kanjidic-local");
        std::fs::write(dir.join(KANJIDIC.file_name), gz(KANJIDIC_XML)).expect("write");
        assert_eq!(read_all(resolve_kanjidic(&dir).expect("resolve")), KANJIDIC_XML);
    }

    /// The two sources resolve independently: a KANJIDIC2 archive does not
    /// satisfy `resolve`, and vice versa. Without the file at its own name
    /// `resolve_from` would go to the injected URL, so an unroutable one
    /// makes the miss observable without a listener.
    #[test]
    fn each_source_resolves_only_its_own_file() {
        let dir = scratch("resolve-independent");
        std::fs::write(dir.join(KANJIDIC.file_name), gz(KANJIDIC_XML)).expect("write");
        let err = resolve_from(
            &JMDICT,
            "http://127.0.0.1:1/JMdict_e.gz",
            &dir,
            std::time::Duration::ZERO,
        )
        .err()
        .expect("JMdict is absent and must not be satisfied by KANJIDIC2");
        assert!(err.to_string().contains("JMdict_e.gz"), "got {err}");
    }
```

Note `resolve_from(&JMDICT, "http://127.0.0.1:1/...", ..)` dials a closed port three times with zero backoff; that is a fast connection-refused, not a hang.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p jmdict-source --lib`
Expected: compile errors — `Source`, `KANJIDIC`, `resolve_kanjidic` do not exist.

- [ ] **Step 3: The descriptor**

In `crates/jmdict-source/src/lib.rs`, replace the `SOURCE_FILE` and `XML_PREFIXES` definitions (keep `SOURCE_DIR`, `PARTIAL_SUFFIX`, `DOWNLOAD_ATTEMPTS`, `RETRY_BACKOFF`, `GZIP_MAGIC`, `UTF8_BOM`) with:

```rust
/// One EDRDG source file: where it is published, what it is called, how an
/// ungzipped copy of it begins, and how large a download may be.
///
/// Two exist, [`JMDICT`] and [`KANJIDIC`]. Everything in this crate that
/// downloads, verifies, stages or opens a file takes one of these, so the
/// second file shares the first's retry, staging, size cap and prolog
/// discipline rather than copying it (Phase 3a design §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// Plain HTTP by necessity: EDRDG serves no usable HTTPS (certificate
    /// subject-name mismatch) and this crate has no TLS backend. That is
    /// precisely why [`fetch::fetch_from`]'s verification is mandatory.
    pub url: &'static str,
    /// The archive's name, as EDRDG publishes it. The extension is a
    /// location, not a claim: [`open_local`] decides compression from bytes.
    pub file_name: &'static str,
    /// Byte prefixes an ungzipped file may start with. Matched against the
    /// specific file rather than `<`: the parser reads markup it does not
    /// recognise as zero entries, so a saved error page — or the other
    /// source's file under this name — would otherwise build an empty
    /// index and report success.
    pub prologs: &'static [&'static [u8]],
    /// Upper bound on the response body a download will accept, enforced via
    /// `ureq`'s `BodyWithConfig::limit`. Nothing else bounds how much a
    /// misbehaving response could stream to disk before verification runs.
    pub max_bytes: u64,
}

const JMDICT_PROLOGS: &[&[u8]] = &[b"<?xml", b"<!DOCTYPE JMdict", b"<JMdict"];
const KANJIDIC_PROLOGS: &[&[u8]] = &[b"<?xml", b"<!DOCTYPE kanjidic2", b"<kanjidic2"];

/// JMdict, English glosses. 32 MiB is a little over 3x the measured
/// 10,545,887-byte archive.
pub const JMDICT: Source = Source {
    url: "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz",
    file_name: "JMdict_e.gz",
    prologs: JMDICT_PROLOGS,
    max_bytes: 32 * 1024 * 1024,
};

/// KANJIDIC2. 8 MiB is a little over 5x the 1,488,571 bytes measured on
/// 2026-09-11; the same host and the same regenerate-daily policy as JMdict,
/// so it cannot be hash-pinned either.
pub const KANJIDIC: Source = Source {
    url: "http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz",
    file_name: "kanjidic2.xml.gz",
    prologs: KANJIDIC_PROLOGS,
    max_bytes: 8 * 1024 * 1024,
};

/// JMdict's file name. Kept as a constant because every message, test and
/// caller that predates the second source names it this way.
pub const SOURCE_FILE: &str = JMDICT.file_name;
```

Replace `begins_jmdict_xml` with:

```rust
/// Whether `head` — a file's first bytes — begins `source`'s XML.
fn begins_xml(source: &Source, head: &[u8]) -> bool {
    let head = head.strip_prefix(&UTF8_BOM).unwrap_or(head);
    let head = head.trim_ascii_start();
    source.prologs.iter().any(|prefix| head.starts_with(prefix))
}
```

In `SourceError`, give three variants a `file` field and use it in their messages:

```rust
    #[error(
        "the dictionary server returned HTTP {status} for {url}; place a {file} \
         in {source_dir} manually to bypass the download"
    )]
    Http {
        status: u16,
        url: String,
        source_dir: PathBuf,
        /// Which of the two files this was, so the escape hatch names it.
        file: &'static str,
    },
```

```rust
    #[error(
        "{path} is neither a gzip archive nor {file}'s XML — it starts with \"{head}\". \
         Delete it, or replace it with a real {file}",
        path = path.display(),
    )]
    NotAnArchive {
        path: PathBuf,
        head: String,
        file: &'static str,
    },
```

```rust
    #[error(
        "could not obtain the dictionary after {attempts} attempts ({last}); \
         place a {file} in {source_dir} manually to bypass the download"
    )]
    TooManyAttempts {
        attempts: usize,
        source_dir: PathBuf,
        last: String,
        file: &'static str,
    },
```

Delete the `file = SOURCE_FILE,` format arguments those three messages used. `open_local`:

```rust
pub fn open_local(source: &Source, path: &Path) -> Result<Box<dyn BufRead>, SourceError> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let (gzipped, xml, head) = {
        let head = reader.fill_buf()?;
        (
            head.starts_with(&GZIP_MAGIC),
            begins_xml(source, head),
            String::from_utf8_lossy(&head[..head.len().min(24)])
                .escape_debug()
                .to_string(),
        )
    };

    if gzipped {
        return Ok(Box::new(std::io::BufReader::new(
            flate2::read::GzDecoder::new(reader),
        )));
    }
    if xml {
        return Ok(Box::new(reader));
    }
    Err(SourceError::NotAnArchive {
        path: path.to_path_buf(),
        head,
        file: source.file_name,
    })
}
```

(keep the existing doc comment; its `JMdict_e.gz` example still holds.) The resolvers:

```rust
/// Open the JMdict source in `source_dir`, downloading it first if absent.
/// (existing doc body unchanged)
pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>> {
    resolve_from(&JMDICT, JMDICT.url, source_dir, RETRY_BACKOFF)
}

/// [`resolve`] for KANJIDIC2: the second half of `ensure_dictionary`'s
/// source pair. A hand-placed `kanjidic2.xml.gz` or `kanjidic2.xml`-content
/// file at that name is honoured exactly as a hand-placed JMdict is.
pub fn resolve_kanjidic(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>> {
    resolve_from(&KANJIDIC, KANJIDIC.url, source_dir, RETRY_BACKOFF)
}

/// [`resolve`] with the source, URL and backoff injected ... (existing doc)
pub fn resolve_from(
    source: &Source,
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> std::io::Result<Box<dyn BufRead>> {
    let target = source_dir.join(source.file_name);
    let path = if target.exists() {
        target
    } else {
        fetch::fetch_with_retry(source, url, source_dir, backoff).map_err(std::io::Error::other)?
    };
    open_local(source, &path).map_err(std::io::Error::other)
}
```

In the existing unit tests, every `open_local(&path)` becomes `open_local(&JMDICT, &path)` and every `open_local(&dir.join(SOURCE_FILE))` becomes `open_local(&JMDICT, &dir.join(SOURCE_FILE))`:

```bash
sed -i 's/open_local(&path)/open_local(\&JMDICT, \&path)/g; s/open_local(&dir.join(SOURCE_FILE))/open_local(\&JMDICT, \&dir.join(SOURCE_FILE))/g' crates/jmdict-source/src/lib.rs
```

- [ ] **Step 4: The fetcher**

In `crates/jmdict-source/src/fetch.rs`: the import becomes `use crate::{Source, SourceError, JMDICT, KANJIDIC, PARTIAL_SUFFIX};`. Delete `MAX_ARCHIVE_BYTES` and `JMDICT_URL` (their doc comments moved onto `Source` above; keep `DOWNLOAD_TIMEOUT` and its doc). Then:

```rust
pub fn fetch_from(source: &Source, url: &str, source_dir: &Path) -> Result<PathBuf, SourceError> {
    std::fs::create_dir_all(source_dir)?;
    let target = source_dir.join(source.file_name);
    // (existing PID comment)
    let staging = source_dir.join(format!(
        "{}{PARTIAL_SUFFIX}.{}",
        source.file_name,
        std::process::id()
    ));

    match download_and_verify(
        source,
        url,
        source_dir,
        &staging,
        DOWNLOAD_TIMEOUT,
        source.max_bytes,
    ) {
        // (unchanged)
    }
}

fn download_and_verify(
    source: &Source,
    url: &str,
    source_dir: &Path,
    staging: &Path,
    timeout: std::time::Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    // (unchanged, except the Http literal gains)
    //         file: source.file_name,
}

/// Download the JMdict archive into `source_dir`, retrying transient failures.
pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(&JMDICT, JMDICT.url, source_dir, crate::RETRY_BACKOFF)
}

/// [`fetch`] for KANJIDIC2.
pub fn fetch_kanjidic(source_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(&KANJIDIC, KANJIDIC.url, source_dir, crate::RETRY_BACKOFF)
}

pub fn fetch_with_retry(
    source: &Source,
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> Result<PathBuf, SourceError> {
    // (unchanged loop; `fetch_from(source, url, source_dir)`; the
    // TooManyAttempts literal gains `file: source.file_name,`)
}
```

`fetch_from`'s doc comment says "publish it as [`crate::SOURCE_FILE`]" — change to "publish it as `source.file_name`". In this file's two unit tests, `download_and_verify(&url, &dir, &staging, ...)` becomes `download_and_verify(&JMDICT, &url, &dir, &staging, ...)`.

- [ ] **Step 5: The integration tests' call sites**

```bash
sed -i 's/fetch::fetch_from(&url, /fetch::fetch_from(\&JMDICT, \&url, /g; s/fetch::fetch_with_retry(&url, /fetch::fetch_with_retry(\&JMDICT, \&url, /g; s/jmdict_source::resolve_from(&url, /jmdict_source::resolve_from(\&JMDICT, \&url, /g' crates/jmdict-source/tests/download.rs
```

Change the import at line 13 to `use jmdict_source::{SourceError, JMDICT, PARTIAL_SUFFIX, SOURCE_FILE};`. At line 308 the `TooManyAttempts` destructure lists three fields with no `..`; make it:

```rust
        SourceError::TooManyAttempts {
            attempts,
            source_dir,
            last,
            file,
        } => {
            assert_eq!(file, SOURCE_FILE);
```

In `src-tauri/src/commands.rs` line 1714, the test literal gains `file: jmdict_source::SOURCE_FILE,` after `last`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p jmdict-source`
Expected: green, including `seam.rs` (still the single-reader `ensure_dictionary`; Task 4 changes it) and the six new unit tests.

Run: `cargo test --workspace`
Expected: green — `commands.rs`'s literal compiles, `resolve` and `fetch` kept their signatures for `jparser-cli` and `src-tauri`.

- [ ] **Step 7: Standing gates, then commit**

```bash
git add crates/jmdict-source src-tauri/src/commands.rs
git commit -m "feat(jmdict-source): a Source descriptor, and KANJIDIC2 as the second one

The URL, file name, prolog list and size cap that were constants become fields
of a Source, and every function that downloads, verifies, stages or opens a
file takes one, so KANJIDIC2 shares JMdict's retry, PID-staged partial file,
gzip verification and prolog check instead of copying them. The prolog check
is per source: a JMdict file saved under the KANJIDIC2 name is refused, since
it would otherwise parse as zero characters and build a kanji-less index that
reports success.

The three error variants whose messages name the file to hand-place now carry
which file that is. resolve and fetch keep their signatures; resolve_kanjidic
and fetch_kanjidic are their KANJIDIC2 twins."
```

---

### Task 4: Both files reach every build

**Files:**
- Modify: `crates/jparser/src/index/generations.rs` — `build_new` (line 177) and its tests (lines 287, 438)
- Modify: `crates/jparser/src/index/mod.rs` — `ensure_dictionary` (line 130) and its tests (12 `Ok(XML.as_bytes())` sites, 2 `build_new` sites)
- Modify: `crates/jparser/tests/generations_torn_build.rs:164,214`
- Modify: `crates/jmdict-source/tests/seam.rs`
- Modify: `src-tauri/src/test_support.rs`
- Modify: `src-tauri/src/commands.rs` — `download_dictionary`'s closure (line 1497–1511), `with_fallback` (line 1454–1464) and its tests (line 1760–1775)
- Modify: `crates/jparser/src/bin/jparser-cli/main.rs` — `BuildIndex`, `EnsureDictionary`, `FetchSource`
- Test: `crates/jparser/tests/cli_generations.rs`

**Interfaces:**
- Consumes: `build_from_readers` (Task 2); `resolve_kanjidic`, `fetch_kanjidic`, `KANJIDIC` (Task 3).
- Produces: `pub fn generations::build_new(root: &Path, jmdict: impl BufRead, kanjidic: impl BufRead) -> Result<(PathBuf, BuildReport), IndexError>`; `pub fn ensure_dictionary<J, K, F>(root, keep, source: F) where F: FnOnce() -> io::Result<(J, K)>, J: BufRead, K: BufRead`; `pub(crate) const test_support::FIXTURE_KANJIDIC: &str` (Task 5's tests read the index it builds).

- [ ] **Step 1: Write the failing tests**

In `crates/jparser/src/index/mod.rs`'s test module, add after `const XML`:

```rust
    /// The empty KANJIDIC2 half of every source pair in this module. Kanji
    /// records are the roundtrip tests' subject, not this module's.
    const NO_KANJIDIC: &[u8] = b"";
```

and append this test:

```rust
    /// The source closure is consulted once and yields both files; a
    /// generation built through it carries the kanji records.
    #[test]
    fn ensure_dictionary_stores_the_kanjidic_half_of_the_pair() {
        let root = scratch("ensure-kanji");
        const KANJIDIC: &str = "<kanjidic2><character><literal>本</literal>\
            <reading_meaning><rmgroup><reading r_type=\"ja_on\">ホン</reading>\
            <meaning>book</meaning></rmgroup></reading_meaning></character></kanjidic2>";
        let index = ensure_dictionary(&root, 2, || Ok((XML.as_bytes(), KANJIDIC.as_bytes())))
            .expect("ensure_dictionary");
        assert_eq!(index.kanji_count(), 1);
        assert_eq!(index.kanji('本').expect("本").meanings, vec!["book"]);
    }
```

In `crates/jmdict-source/tests/seam.rs`, the import becomes `use jmdict_source::{KANJIDIC, SOURCE_FILE};`, and the test body writes a second archive and passes the pair:

```rust
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(XML.as_bytes()).expect("gz write");
    std::fs::write(source_dir.join(SOURCE_FILE), e.finish().expect("gz")).expect("write");
    // Plain XML at the gzip name: `open_local` decides by bytes, and this
    // proves the second source takes the same path as the first.
    std::fs::write(
        source_dir.join(KANJIDIC.file_name),
        "<kanjidic2><character><literal>本</literal><reading_meaning><rmgroup>\
         <reading r_type=\"ja_kun\">もと</reading></rmgroup></reading_meaning></character></kanjidic2>",
    )
    .expect("write kanjidic");

    let index = jparser::index::ensure_dictionary(&root, 2, || {
        Ok((
            jmdict_source::resolve(&source_dir)?,
            jmdict_source::resolve_kanjidic(&source_dir)?,
        ))
    })
    .expect("ensure_dictionary");

    assert_eq!(
        index.entry(1000010).expect("entry").expect("present").id,
        1000010
    );
    assert_eq!(index.kanji('本').expect("本").kun, vec!["もと"]);
    assert!(root.join("gen-1").exists());
    assert!(source_dir.join(SOURCE_FILE).exists(), "the source vanished");
    assert!(source_dir.join(KANJIDIC.file_name).exists(), "the kanjidic source vanished");
```

In `crates/jparser/tests/cli_generations.rs`, the import becomes `use jmdict_source::{KANJIDIC, SOURCE_FILE};`. In `ensure_dictionary_builds_through_the_source_dir_route`, after the JMdict write:

```rust
    // Without this the route would resolve KANJIDIC2 from the real network.
    std::fs::write(source_dir.join(KANJIDIC.file_name), "<kanjidic2></kanjidic2>")
        .expect("write kanjidic");
```

and append two tests:

```rust
const KANJIDIC_XML: &str = "<kanjidic2><character><literal>本</literal>\
    <reading_meaning><rmgroup><reading r_type=\"ja_on\">ホン</reading>\
    <meaning>book</meaning></rmgroup></reading_meaning></character></kanjidic2>";

#[test]
fn build_index_accepts_a_kanjidic_file() {
    let dir = scratch("cli-build-kanjidic");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    std::fs::write(dir.join("kanji.xml"), KANJIDIC_XML).expect("write kanjidic");

    let out = cli(&["build-index", "mini.xml", "idx", "--kanjidic", "kanji.xml"], &dir);

    assert!(out.contains("kanji:               1"), "got: {out}");
    assert!(out.contains("skipped kanji:       0"), "got: {out}");
}

#[test]
fn build_index_without_kanjidic_reports_zero_kanji() {
    let dir = scratch("cli-build-no-kanjidic");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = cli(&["build-index", "mini.xml", "idx"], &dir);

    assert!(out.contains("kanji:               0"), "got: {out}");
}

#[test]
fn ensure_dictionary_accepts_a_kanjidic_beside_the_positional_xml() {
    let dir = scratch("cli-ensure-kanjidic");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    std::fs::write(dir.join("kanji.xml"), KANJIDIC_XML).expect("write kanjidic");

    let out = cli(
        &["ensure-dictionary", "dict", "mini.xml", "--kanjidic", "kanji.xml"],
        &dir,
    );

    assert!(out.contains("generation: gen-1"), "got: {out}");
    assert!(out.contains("kanji:      1"), "got: {out}");
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p jparser --lib index::tests::ensure_dictionary_stores && cargo test -p jparser --test cli_generations`
Expected: a compile error on the closure's return type in the first; `--kanjidic` unknown-argument failures in the second.

- [ ] **Step 3: `build_new` and `ensure_dictionary`**

`crates/jparser/src/index/generations.rs`:

```rust
/// Build an index from `jmdict` and `kanjidic` and publish it as the next
/// generation.
/// (rest of the doc unchanged)
pub fn build_new(
    root: &Path,
    jmdict: impl BufRead,
    kanjidic: impl BufRead,
) -> Result<(PathBuf, BuildReport), IndexError> {
    // (unchanged until)
    let report = crate::engine::build::build_from_readers(jmdict, kanjidic, &build_dir)?;
```

In `sweep`'s doc, "When `build_from_reader` itself fails" becomes "When `build_from_readers` itself fails". In its test module add `const NO_KANJIDIC: &[u8] = b"";` beside `MINI_XML`, and both `build_new(root, MINI_XML.as_bytes())` / `build_new(&root, MINI_XML.as_bytes())` calls gain `, NO_KANJIDIC` as a third argument.

`crates/jparser/src/index/mod.rs`:

```rust
/// Open the newest usable index in `root`, building one first if necessary.
///
/// `source` is a *lazy* producer of the two EDRDG files, JMdict XML then
/// KANJIDIC2 XML: it is invoked only when a build is actually required,
/// because the real sources are a ~10 MB and a ~1.5 MB download and the
/// steady-state path must not pay for them. One closure rather than two so a
/// caller's "downloading" phase covers both fetches (Phase 3a design §3.2).
/// (rest of the doc unchanged)
pub fn ensure_dictionary<J, K, F>(root: &Path, keep: usize, source: F) -> Result<Index, IndexError>
where
    F: FnOnce() -> std::io::Result<(J, K)>,
    J: std::io::BufRead,
    K: std::io::BufRead,
{
    // (unchanged until)
    let (jmdict, kanjidic) = source()?;
    let (published, _report) = generations::build_new(root, jmdict, kanjidic)?;
```

Its tests:

```bash
sed -i 's/Ok(XML.as_bytes())/Ok((XML.as_bytes(), NO_KANJIDIC))/g; s/build_new(&root, XML.as_bytes())/build_new(\&root, XML.as_bytes(), NO_KANJIDIC)/g' crates/jparser/src/index/mod.rs
```

`crates/jparser/tests/generations_torn_build.rs`: add `const NO_KANJIDIC: &[u8] = b"";` beside its XML constants and give both `build_new(&root, XML_A.as_bytes())` calls the third argument.

- [ ] **Step 4: The two app-side closures**

`src-tauri/src/test_support.rs`, after `FIXTURE_XML`:

```rust
/// A one-character KANJIDIC2 fixture for the same headword, so a command test
/// can see the kanji block without a real download.
pub(crate) const FIXTURE_KANJIDIC: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<kanjidic2>",
    "<character><literal>本</literal><reading_meaning><rmgroup>",
    r#"<reading r_type="ja_on">ホン</reading><reading r_type="ja_kun">もと</reading>"#,
    "<meaning>book</meaning><meaning>present</meaning><meaning>main</meaning>",
    "<meaning>origin</meaning><meaning>true</meaning>",
    "</rmgroup></reading_meaning></character>",
    "</kanjidic2>",
);
```

and `build_index_generation`'s call becomes:

```rust
        ensure_dictionary(root, 1, || Ok((FIXTURE_XML.as_bytes(), FIXTURE_KANJIDIC.as_bytes())))
            .expect("build the fixture index"),
```

`src-tauri/src/commands.rs`, `download_dictionary`'s closure:

```rust
            || {
                let jmdict = jmdict_source::resolve(&source_dir)?;
                let kanjidic = jmdict_source::resolve_kanjidic(&source_dir)?;
                // Emitted only after both resolves return, not before: the
                // network fetches (or reuse of already-downloaded archives)
                // happen inside `resolve` itself, so emitting "building" any
                // earlier would mislabel a download as index-building.
                // "downloading" covers both files; "building" covers the
                // `build_from_readers` work that follows.
                emit_status(&handle, "building");
                Ok((jmdict, kanjidic))
            },
```

`with_fallback`:

```rust
/// Append the manual-fallback instructions to `reason`, unless it already
/// names `source_dir_display`.
///
/// `SourceError::Http` and `SourceError::TooManyAttempts` already end with
/// "place a <file> in `<source_dir>` manually to bypass the download"
/// (`crates/jmdict-source/src/lib.rs`), naming whichever of the two files
/// failed — appending again would say it twice on screen. Every other
/// failure (a corrupt archive, a dead worker) says nothing about the
/// directory at all, so this is the only place those learn about the
/// escape hatch, and it names both files because it cannot tell which one
/// the reader is missing.
fn with_fallback(reason: String, source_dir_display: &str) -> String {
    if reason.contains(source_dir_display) {
        format!("{reason}. Retry.")
    } else {
        format!(
            "{reason}. Retry, or place {} and {} in {source_dir_display} and retry.",
            jmdict_source::JMDICT.file_name,
            jmdict_source::KANJIDIC.file_name,
        )
    }
}
```

In `the_fallback_is_added_when_the_reason_does_not_name_the_directory` add `assert!(full.contains(jmdict_source::KANJIDIC.file_name), "got {full}");`.

- [ ] **Step 5: The CLI**

`crates/jparser/src/bin/jparser-cli/main.rs`. `BuildIndex` gains a flag:

```rust
    /// Build an index from a JMdict XML file, and optionally a KANJIDIC2 one.
    BuildIndex {
        /// Path to JMdict_e.xml (uncompressed).
        xml: PathBuf,
        /// Directory to write the index into.
        out: PathBuf,
        /// Path to kanjidic2.xml (uncompressed). Absent, the index carries no
        /// kanji records and the popover shows no kanji block; the parse is
        /// unaffected either way.
        #[arg(long)]
        kanjidic: Option<PathBuf>,
    },
```

`EnsureDictionary` gains the same flag after `source_dir`:

```rust
        /// KANJIDIC2 XML beside a positional XML. Ignored with `--source-dir`,
        /// which resolves both files from that directory.
        #[arg(long)]
        kanjidic: Option<PathBuf>,
```

A helper above `main`:

```rust
/// The KANJIDIC2 half of a build's source pair: the file at `path`, or an
/// empty reader when none was given. `Box<dyn BufRead>` so both arms are one
/// type.
fn kanjidic_reader(path: Option<&std::path::Path>) -> std::io::Result<Box<dyn std::io::BufRead>> {
    match path {
        Some(path) => Ok(Box::new(BufReader::new(std::fs::File::open(path)?))),
        None => Ok(Box::new(std::io::empty())),
    }
}
```

The arms:

```rust
        Command::BuildIndex { xml, out, kanjidic } => {
            let file = BufReader::new(std::fs::File::open(&xml)?);
            let kanjidic = kanjidic_reader(kanjidic.as_deref())?;
            let report = jparser::engine::build::build_from_readers(file, kanjidic, &out)?;
            // (the println! block from Task 2)
        }
        Command::EnsureDictionary {
            root,
            xml,
            source_dir,
            kanjidic,
            keep,
        } => {
            let index = match (&xml, &source_dir) {
                (Some(xml), _) => ensure_dictionary(&root, keep, || {
                    Ok((
                        BufReader::new(std::fs::File::open(xml)?),
                        kanjidic_reader(kanjidic.as_deref())?,
                    ))
                })?,
                (None, Some(dir)) => ensure_dictionary(&root, keep, || {
                    Ok((
                        jmdict_source::resolve(dir)?,
                        jmdict_source::resolve_kanjidic(dir)?,
                    ))
                })?,
                (None, None) => return Err("no source given".into()),
            };
            // (unchanged) ...
            println!("generation: {name}");
            println!("entries:    {}", index.entry_count());
            println!("kanji:      {}", index.kanji_count());
        }
        Command::FetchSource { dir } => {
            let jmdict = jmdict_source::fetch::fetch(&dir)?;
            let kanjidic = jmdict_source::fetch::fetch_kanjidic(&dir)?;
            println!("archive:    {}", jmdict.display());
            println!("kanjidic:   {}", kanjidic.display());
            println!("convention: keep them in a directory named {SOURCE_DIR}");
        }
```

`FetchSource`'s doc line becomes "Download the JMdict and KANJIDIC2 archives into DIR without building an index."

- [ ] **Step 6: Run the tests**

Run: `cargo test --workspace`
Expected: green. `cli_generations`'s source-dir test must complete in well under a second — if it takes seconds or fails with a transport error, the KANJIDIC2 file was not placed and it reached the network. Check `ensure_dictionary_still_accepts_a_positional_xml` still passes and that any test pinning `ensure-dictionary`'s stdout accepts the new `kanji:` line.

- [ ] **Step 7: Standing gates, then commit**

Also `cargo test -p jparser --features hints-source` and `cargo clippy -p jparser --features hints-source --all-targets -- -D warnings`: the `FetchSource` arm sits beside the `FetchHints` one.

```bash
git add crates src-tauri/src/commands.rs src-tauri/src/test_support.rs
git commit -m "feat: every dictionary build takes both EDRDG files

ensure_dictionary's lazy source closure now returns the pair (JMdict,
KANJIDIC2), and build_new threads it to build_from_readers. The app's
download closure resolves both from the source directory, so a first run and
an upgrade rebuild fetch kanjidic2.xml.gz beside JMdict_e.gz through the same
retry, staging and first-run screen; a KANJIDIC2 failure fails the build the
way a JMdict failure does, knowingly, over a silent-degrade path nobody would
test. The fallback message names both files.

jparser-cli build-index and ensure-dictionary gain --kanjidic; fetch-source
fetches both. The two tests that hand-place JMdict_e.gz now hand-place
kanjidic2.xml.gz too, since the source-dir route would otherwise have reached
the real network from the test suite."
```

---

### Task 5: The `kanji_info` command, and the wire pin

**Files:**
- Modify: `src-tauri/src/commands.rs` — new items after `needs_dictionary` (line 1406); tests appended to `mod tests`
- Modify: `src-tauri/src/main.rs` — `invoke_handler` list (line 268, after `commands::hints_dir`)
- Modify: `crates/jparser/tests/engine_v2.rs` — one test after `parse_result_serializes_to_the_documented_wire_shape`
- Modify: `src/types.ts` — `KanjiInfo` after `Entry`

**Interfaces:**
- Consumes: `Index::kanji` (Task 2); `IndexSender(watch::Sender<Option<Arc<AppState>>>)`; `test_support::build_index_generation` (Task 4 makes it carry 本).
- Produces: `pub(crate) fn lookup_kanji(index: &Index, chars: &str) -> Vec<KanjiData>`; `#[tauri::command] pub fn kanji_info(chars: String, index: State<'_, IndexSender>) -> Vec<KanjiData>` — JSON `[{ literal, on, kun, meanings }]`; TS `interface KanjiInfo { literal: string; on: string[]; kun: string[]; meanings: string[] }`. Task 7 invokes `kanji_info` with `{ chars }`.

- [ ] **Step 1: Write the failing tests**

`crates/jparser/tests/engine_v2.rs`, after the existing wire-shape test:

```rust
/// Pins the JSON `kanji_info` hands the webview. Field names here are the
/// TypeScript property names of `src/types.ts`'s `KanjiInfo`; `literal` is a
/// `char`, which serde_json writes as a one-character string.
#[test]
fn kanji_data_serializes_to_the_documented_wire_shape() {
    let data = jparser::index::KanjiData {
        literal: '東',
        on: vec!["トウ".to_string()],
        kun: vec!["ひがし".to_string()],
        meanings: vec!["east".to_string()],
    };
    let json = serde_json::to_value(&data).expect("serialize");
    assert_eq!(json["literal"], "東");
    assert_eq!(json["on"][0], "トウ");
    assert_eq!(json["kun"][0], "ひがし");
    assert_eq!(json["meanings"][0], "east");
}
```

`src-tauri/src/commands.rs` test module, appended:

```rust
    /// Hits come back in input order; kana and characters the index has no
    /// record for are simply absent. The fixture index carries 本 only.
    #[test]
    fn lookup_kanji_returns_hits_in_order_and_omits_misses() {
        let root = crate::test_support::scratch("kanji-lookup");
        let generation = crate::test_support::build_index_generation(&root);
        let index = jparser::index::load::Index::open(&generation).expect("open");

        let hits = lookup_kanji(&index, "あ本東");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].literal, '本');
        assert_eq!(hits[0].on, vec!["ホン"]);
        assert_eq!(hits[0].kun, vec!["もと"]);
        // The parser's cap, visible end to end.
        assert_eq!(hits[0].meanings, vec!["book", "present", "main", "origin"]);

        assert!(lookup_kanji(&index, "").is_empty());
        assert!(lookup_kanji(&index, "はいた").is_empty());
    }

    /// Before the first download the channel holds `None`; the command must
    /// answer empty rather than fail, because the popover can open on a
    /// stale parse while the download screen is up.
    #[test]
    fn kanji_info_is_empty_before_a_dictionary_is_loaded() {
        let app = tauri::test::mock_app();
        let (tx, _rx) = tokio::sync::watch::channel(None::<Arc<AppState>>);
        app.manage(IndexSender(tx));
        let out = kanji_info("本".to_string(), app.state::<IndexSender>());
        assert!(out.is_empty());
    }

    #[test]
    fn kanji_info_answers_from_the_published_index() {
        let root = crate::test_support::scratch("kanji-info");
        let generation = crate::test_support::build_index_generation(&root);
        let index = jparser::index::load::Index::open(&generation).expect("open");
        let state = AppState {
            index,
            table: jparser::engine::table::Table::new(),
        };
        let app = tauri::test::mock_app();
        let (tx, _rx) = tokio::sync::watch::channel(Some(Arc::new(state)));
        app.manage(IndexSender(tx));

        let out = kanji_info("本".to_string(), app.state::<IndexSender>());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].literal, '本');
    }
```

(`scratch` in `test_support` is `pub(crate)` already; if the commands test module has its own `scratch`, either works — use `crate::test_support::scratch` so the directory prefix is the shared one.)

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p jparser --test engine_v2 kanji_data && cargo test --manifest-path src-tauri/Cargo.toml kanji`
Expected: the first passes already (the type exists; keep it — it is the pin); the second fails to compile on `lookup_kanji` / `kanji_info`.

- [ ] **Step 3: The command**

In `src-tauri/src/commands.rs`, after `needs_dictionary`:

```rust
/// The KANJIDIC2 records for the kanji in `chars`, in input order, with
/// misses and non-kanji omitted. Split from the command for the reason
/// `merge_saved` is: a `#[tauri::command]` taking `State<'_, _>` needs a mock
/// app to call, and the lookup rule is worth pinning on its own.
pub(crate) fn lookup_kanji(
    index: &jparser::index::load::Index,
    chars: &str,
) -> Vec<jparser::index::KanjiData> {
    chars.chars().filter_map(|ch| index.kanji(ch).cloned()).collect()
}

/// The popover's kanji block: one record per character of `chars`, which the
/// webview sends as the distinct kanji of the hovered surface. Asked lazily
/// on hover so the parse payload does not grow (Phase 3a design §5). Empty
/// until a dictionary has been published to the worker.
#[tauri::command]
pub fn kanji_info(chars: String, index: State<'_, IndexSender>) -> Vec<jparser::index::KanjiData> {
    // Cloned out of the borrow before the lookup: `watch::Sender::borrow`
    // holds the channel's read lock, and a `download_dictionary` publish
    // would otherwise wait on this command.
    let state = index.0.borrow().clone();
    match state {
        Some(state) => lookup_kanji(&state.index, &chars),
        None => Vec::new(),
    }
}
```

In `src-tauri/src/main.rs`'s `generate_handler!` list, after `commands::hints_dir,`:

```rust
            commands::kanji_info,
```

`src/types.ts`, after `Entry`:

```ts
/**
 * One KANJIDIC2 record, as `kanji_info` returns it. Mirrors
 * `jparser::index::KanjiData`; the Rust test
 * `kanji_data_serializes_to_the_documented_wire_shape` pins these names on the
 * other side.
 */
export interface KanjiInfo {
  literal: string;
  /** On readings, katakana. */
  on: string[];
  /** Kun readings, with KANJIDIC2's `.` and `-` marks kept: `た.べる`, `-がた`. */
  kun: string[];
  /** English meanings, at most four. */
  meanings: string[];
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml && npx tsc --noEmit`
Expected: green; three new tests pass.

- [ ] **Step 5: Standing gates, then commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/main.rs crates/jparser/tests/engine_v2.rs src/types.ts
git commit -m "feat(app): kanji_info, the one command behind the popover's kanji block

A binary search per character over the published index, answered from the
watch channel's current value so no new state is managed and the command
answers empty before the first download. KanjiData is the wire type; the
engine_v2 wire-shape test pins its field names against src/types.ts's
KanjiInfo the way it pins Entry's."
```

---

### Task 6: The popover renders the block

**Files:**
- Modify: `src/types.ts` — `GlossFilters` and `Settings` gain `hide_kanji?: boolean`
- Modify: `src/render/tooltip-text.ts` — `assembleTooltipText(entries, filters?, kanji?)`
- Modify: `src/render/tooltip.ts` — `renderTooltip(entries, filters?, kanji?)`
- Modify: `src/popover.ts` — payload type and passthrough
- Test: `src/render/tooltip-text.test.ts`, `src/popover.test.ts`

**Interfaces:**
- Consumes: `KanjiInfo` (Task 5).
- Produces: `assembleTooltipText(entries: Entry[], filters?: GlossFilters, kanji: KanjiInfo[] = []): string`; `renderTooltip(entries: Entry[], filters?: GlossFilters, kanji: KanjiInfo[] = []): HTMLElement`; the `popover-content` payload accepts `kanji?: KanjiInfo[]`. Task 7 emits that field.

- [ ] **Step 1: Write the failing tests**

`src/render/tooltip-text.test.ts`, inside `describe('assembleTooltipText')`, append:

```ts
  // Phase 3a: one blank line, then one flat line per kanji — literal, the on
  // and kun readings joined by the katakana middle dot, the meanings — so
  // `colourLine` colours the block as it colours everything else.
  describe('the kanji block', () => {
    const tokyo: KanjiInfo[] = [
      { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east', 'eastern'] },
      { literal: '京', on: ['キョウ', 'ケイ'], kun: ['みやこ'], meanings: ['capital', '10**16'] },
    ];

    test('appends one line per kanji after a blank line', () => {
      expect(assembleTooltipText([entry()], undefined, tokyo)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n' +
          '東  トウ・ひがし  east, eastern\n' +
          '京  キョウ・ケイ・みやこ  capital, 10**16',
      );
    });

    test('renders nothing extra when there are no kanji', () => {
      expect(assembleTooltipText([entry()], undefined, [])).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
      );
    });

    test('hide_kanji drops the block and the blank line with it', () => {
      expect(assembleTooltipText([entry()], { hide_kanji: true }, tokyo)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
      );
    });

    // A record with no readings or no meanings must not leave a doubled gap.
    test('omits an empty column rather than leaving a gap', () => {
      const bare: KanjiInfo[] = [{ literal: '々', on: [], kun: [], meanings: ['repetition mark'] }];
      expect(assembleTooltipText([entry()], undefined, bare)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n々  repetition mark',
      );
    });
  });
```

and change the import to `import type { Entry, KanjiInfo } from '../types';`.

`src/popover.test.ts`, appended inside `describe('the popover window')`:

```ts
  test('renders the kanji block it is sent, after a blank line', () => {
    send({
      entries,
      kanji: [{ literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] }],
    });

    const lines = [...document.querySelectorAll('.tt-line')].map((el) => el.textContent);
    expect(lines).toEqual(['東京【とうきょう】', '(n) (1) Tokyo', '', '東  トウ・ひがし  east']);
    // The blank line keeps its height, as the gap between two entries does.
    expect(document.querySelectorAll('.tt-line br')).toHaveLength(1);
    // The literal is coloured as kanji, like a headword's characters.
    const literal = [...document.querySelectorAll('.tt-kanji')].map((el) => el.textContent);
    expect(literal).toContain('東');
  });

  test('renders no block under hide_kanji', () => {
    send({
      entries,
      filters: { hide_kanji: true },
      kanji: [{ literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] }],
    });

    expect(document.querySelectorAll('.tt-line')).toHaveLength(2);
  });
```

- [ ] **Step 2: Run to see them fail**

Run: `npx vitest run src/render/tooltip-text.test.ts src/popover.test.ts`
Expected: the new tests fail (`hide_kanji` is not a known key under `tsc`, and the text has no block).

- [ ] **Step 3: Types, text, render, passthrough**

`src/types.ts`, in `GlossFilters` and in `Settings` after `hide_usage`:

```ts
  /** Phase 3a: the KANJIDIC block at the foot of the popover. Default shown. */
  hide_kanji?: boolean;
```

`src/render/tooltip-text.ts`:

```ts
import type { Entry, GlossFilters, KanjiInfo } from '../types';
```

```ts
/**
 * One kanji's line: the literal, its on then kun readings joined by the
 * katakana middle dot, its English meanings. Columns are two spaces apart;
 * an empty column is dropped rather than left as a gap. KANJIDIC2's `.` and
 * `-` marks in kun readings are rendered raw, deliberately (design §7).
 */
function kanjiLine(kanji: KanjiInfo): string {
  const readings = [...kanji.on, ...kanji.kun].join('・');
  return [kanji.literal, readings, kanji.meanings.join(', ')]
    .filter((column) => column !== '')
    .join('  ');
}

/**
 * Every match for one word, as the tooltip's flat text block, followed —
 * when there are any and the reader has not hidden them — by one blank line
 * and one line per kanji in the hovered surface.
 *
 * Flat text rather than structured markup because the colouring that follows
 * is lexical, not semantic (see `tooltip-colour.ts`): it reads characters, so
 * what it needs is characters.
 */
export function assembleTooltipText(
  entries: Entry[],
  filters?: GlossFilters,
  kanji: KanjiInfo[] = [],
): string {
  const lines = entries.flatMap((e) => entryLines(e, filters));
  if (!filters?.hide_kanji && kanji.length > 0) lines.push('', ...kanji.map(kanjiLine));
  return lines.join('\n');
}
```

`src/render/tooltip.ts`:

```ts
import type { Entry, GlossFilters, KanjiInfo } from '../types';
```

```ts
export function renderTooltip(
  entries: Entry[],
  filters?: GlossFilters,
  kanji: KanjiInfo[] = [],
): HTMLElement {
  const root = document.createElement('div');
  root.className = 'tt';

  for (const line of assembleTooltipText(entries, filters, kanji).split('\n')) {
```

`src/popover.ts`:

```ts
import type { Entry, GlossFilters, KanjiInfo } from './types';
```

```ts
type PopoverPayload =
  | Entry[]
  | { entries: Entry[]; filters?: GlossFilters; gloss?: number; kanji?: KanjiInfo[] };
```

```ts
  const filters = Array.isArray(payload) ? undefined : payload.filters;
  // Absent on a bare-array payload and on one from a main window that found
  // no kanji; either renders exactly as before this field existed.
  const kanji = Array.isArray(payload) ? [] : (payload.kanji ?? []);
```

and `renderTooltip(entries, filters, kanji)`.

- [ ] **Step 4: Run the tests**

Run: `npx vitest run && npx tsc --noEmit`
Expected: green; `main.test.ts` and `main-tooltip.test.ts` are untouched by this task and still pass, since main does not send `kanji` yet.

- [ ] **Step 5: Commit**

```bash
git add src/types.ts src/render/tooltip-text.ts src/render/tooltip.ts src/popover.ts src/render/tooltip-text.test.ts src/popover.test.ts
git commit -m "feat(popover): the kanji block, one flat line per character at the foot

After the entries and one blank line: literal, on and kun readings joined by
the middle dot, English meanings. Flat text, so colourLine colours the literal
and the kana runs as it colours a headword line. Nothing renders when the
payload carries no kanji or hide_kanji is set, so a payload from an older main
window renders exactly as today."
```

---

### Task 7: The main window asks for the kanji

**Files:**
- Modify: `src/render/tooltip-colour.ts:39` — `export function isIdeograph`
- Create: `src/kanji-lookup.ts`, `src/kanji-lookup.test.ts`
- Modify: `src/main.ts` — imports (line 24), `currentFilters` (line 266), first-run copy (line 219), settings apply sites (lines 648–650, 666), `openFor` (line 930–956)
- Modify: `src/main-tooltip.test.ts` — two tests at lines 201 and 243; two new tests
- Modify: `src/main.test.ts` — the `focusin`-then-assert sites at lines 269–280, 1644–1654, 1700–1712, 2003–2013; the `hoverChip` helper; `filters` literals at 1652, 2011, 2022

**Interfaces:**
- Consumes: `kanji_info` (Task 5), `KanjiInfo`, the popover's `kanji` field (Task 6).
- Produces: `distinctKanji(surface: string): string[]`; `createKanjiLookup(ask: (chars: string) => Promise<unknown>): (surface: string) => Promise<KanjiInfo[]>`; `openFor` emits `popover-content` after one lookup, with `kanji` present only when non-empty; `currentFilters.hide_kanji`.

- [ ] **Step 1: Write the failing lookup tests**

Create `src/kanji-lookup.test.ts`:

```ts
// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test, vi } from 'vitest';
import { createKanjiLookup, distinctKanji } from './kanji-lookup';
import type { KanjiInfo } from './types';

const 東: KanjiInfo = { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] };
const 京: KanjiInfo = { literal: '京', on: ['キョウ'], kun: ['みやこ'], meanings: ['capital'] };
const 都: KanjiInfo = { literal: '都', on: ['ト'], kun: ['みやこ'], meanings: ['metropolis'] };

describe('distinctKanji', () => {
  test('keeps first-seen order and drops repeats', () => {
    expect(distinctKanji('東京都')).toEqual(['東', '京', '都']);
    expect(distinctKanji('日日')).toEqual(['日']);
  });

  // The surface, not the headword (design §1.1): a PRONOUNCE match shows the
  // reader kana, and kana have no block.
  test('is empty for a kana-only surface', () => {
    expect(distinctKanji('はいた')).toEqual([]);
    expect(distinctKanji('')).toEqual([]);
  });
});

describe('createKanjiLookup', () => {
  test('returns records in surface order, whatever order the answer came in', async () => {
    const ask = vi.fn(() => Promise.resolve([都, 東, 京]));
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東京都')).toEqual([東, 京, 都]);
    expect(ask).toHaveBeenCalledWith('東京都');
  });

  test('asks once for two hovers of the same character', async () => {
    const ask = vi.fn(() => Promise.resolve([東]));
    const lookup = createKanjiLookup(ask);
    await lookup('東');
    expect(await lookup('東')).toEqual([東]);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  test('asks only for the characters it has not seen', async () => {
    const ask = vi.fn((chars: string) =>
      Promise.resolve([東, 京, 都].filter((k) => chars.includes(k.literal))),
    );
    const lookup = createKanjiLookup(ask);
    await lookup('東京');
    expect(await lookup('京都')).toEqual([京, 都]);
    expect(ask.mock.calls.map((c) => c[0])).toEqual(['東京', '都']);
  });

  test('remembers a miss so it is asked once', async () => {
    const ask = vi.fn(() => Promise.resolve([]));
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東')).toEqual([]);
    expect(await lookup('東')).toEqual([]);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  test('does not ask at all for a kana-only surface', async () => {
    const ask = vi.fn(() => Promise.resolve([]));
    expect(await createKanjiLookup(ask)('はいた')).toEqual([]);
    expect(ask).not.toHaveBeenCalled();
  });

  // The e2e stub answers null for a command it does not name, and a
  // hand-edited backend could answer anything; the boundary validates.
  test('treats a non-array or malformed answer as no records', async () => {
    const lookup = createKanjiLookup(() => Promise.resolve([{ literal: 1 }, null, 東]));
    expect(await lookup('東京')).toEqual([東]);
    expect(await createKanjiLookup(() => Promise.resolve(null))('東')).toEqual([]);
  });

  test('a failed round trip yields no records and is asked again next time', async () => {
    const ask = vi
      .fn<(chars: string) => Promise<unknown>>()
      .mockRejectedValueOnce(new Error('ipc down'))
      .mockResolvedValueOnce([東]);
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東')).toEqual([]);
    expect(await lookup('東')).toEqual([東]);
    expect(ask).toHaveBeenCalledTimes(2);
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `npx vitest run src/kanji-lookup.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: The lookup module**

In `src/render/tooltip-colour.ts`, `function isIdeograph` becomes `export function isIdeograph` (nothing else changes; the doc comment already describes it).

Create `src/kanji-lookup.ts`:

```ts
// SPDX-License-Identifier: MIT OR Apache-2.0

import { isIdeograph } from './render/tooltip-colour';
import type { KanjiInfo } from './types';

/** The distinct kanji in `surface`, in first-seen order. */
export function distinctKanji(surface: string): string[] {
  const seen = new Set<string>();
  for (const ch of surface) {
    if (isIdeograph(ch)) seen.add(ch);
  }
  return [...seen];
}

/** Whether `value` is a `KanjiInfo` as `kanji_info` serialises one. */
function isKanjiInfo(value: unknown): value is KanjiInfo {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.literal === 'string' &&
    Array.isArray(v.on) &&
    Array.isArray(v.kun) &&
    Array.isArray(v.meanings)
  );
}

/**
 * A page-lifetime kanji lookup over `ask` — `invoke('kanji_info', { chars })`
 * in the app, stubbed in tests. Every character is asked once: a hit is
 * cached as its record and a miss as `null`, so a second hover on any
 * character anywhere in the session costs no round trip (design §5.1).
 *
 * A round trip that fails caches nothing and yields no records: the popover
 * opens without its block rather than not opening, and the next hover asks
 * again. Validated at the boundary because the e2e stub answers `null` for
 * this command and nothing else guarantees the shape.
 */
export function createKanjiLookup(
  ask: (chars: string) => Promise<unknown>,
): (surface: string) => Promise<KanjiInfo[]> {
  const cache = new Map<string, KanjiInfo | null>();
  return async (surface) => {
    const wanted = distinctKanji(surface);
    const misses = wanted.filter((ch) => !cache.has(ch));
    if (misses.length > 0) {
      try {
        const answer = await ask(misses.join(''));
        const records = Array.isArray(answer) ? answer.filter(isKanjiInfo) : [];
        for (const ch of misses) cache.set(ch, null);
        for (const record of records) cache.set(record.literal, record);
      } catch {
        // Deliberately nothing cached: see the doc comment above.
      }
    }
    return wanted.flatMap((ch) => {
      const hit = cache.get(ch);
      return hit ? [hit] : [];
    });
  };
}
```

Run: `npx vitest run src/kanji-lookup.test.ts` — Expected: 9 passed.

- [ ] **Step 4: Write the failing main-window tests**

`src/main-tooltip.test.ts`. Add a helper beside `chip()` in the first `describe`:

```ts
  /**
   * Flush `openFor`'s one await — the `kanji_info` round trip — and the
   * `.then` that emits. Four ticks is more than the chain needs; the exact
   * count is not the property under test.
   */
  async function flushLookup(): Promise<void> {
    for (let i = 0; i < 4; i += 1) await Promise.resolve();
  }
```

Rewrite the two tests:

```ts
  // The dwell is what stops the tooltip firing for every chip the cursor
  // sweeps across on its way somewhere else. The content follows one round
  // trip after the dwell — the kanji lookup — hence the async advance.
  test('a completed dwell sends the content', async () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    expect(events()).not.toContain('popover-content');
    await vi.advanceTimersByTimeAsync(350);
    expect(emitted).toContainEqual([
      'popover-content',
      {
        entries: SEGMENTS.segments[0].entries,
        filters: { hide_pos: false, hide_xrefs: false, hide_usage: false, hide_kanji: false },
      },
    ]);
  });
```

```ts
  // Focus moves only on a deliberate keypress, so it has no sweeping problem
  // for a dwell to solve: no timer, only the kanji round trip.
  test('focus sends the content immediately, with no timer', async () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();
    expect(events()).toContain('popover-content');
  });
```

Append two tests to the same `describe`:

```ts
  // Phase 3a: the block rides in the same payload as the entries, so the
  // popover renders once and measures once.
  test('the kanji records ride along when the index has them', async () => {
    const records = [
      { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] },
      { literal: '京', on: ['キョウ'], kun: ['みやこ'], meanings: ['capital'] },
    ];
    invoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'kanji_info') {
        expect(args).toEqual({ chars: '東京' });
        return Promise.resolve(records);
      }
      return Promise.resolve(null);
    });

    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    const content = emitted.find(([event]) => event === 'popover-content');
    expect(content?.[1]).toEqual(expect.objectContaining({ kanji: records }));
  });

  // The cache is the whole point of doing this in the main window: the second
  // hover on any character the session has seen costs no round trip.
  test('kanji_info is asked once for two hovers of the same word', async () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    expect(calls().filter((c) => c === 'kanji_info')).toHaveLength(1);
    expect(events().filter((e) => e === 'popover-content')).toHaveLength(2);
  });

  // A dismissal that lands while the lookup is in flight must win, the way
  // it wins over `placeFor`'s round trip: no content for a closed popover.
  test('a dismissal during the lookup suppresses the content', async () => {
    let answer: ((records: unknown) => void) | undefined;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'kanji_info') return new Promise((resolve) => (answer = resolve));
      return Promise.resolve(null);
    });

    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    answer?.([]);
    await flushLookup();

    expect(events()).not.toContain('popover-content');
  });
```

`src/main.test.ts`: add the same `flushLookup` helper at file scope (after `invoke` is declared), then:
- line 269–280 (`the popover reads the displayed reading, not the top one`): make the test `async`, add `await flushLookup();` after the `focusin` dispatch.
- line 1644–1654 (`settings-changed event updates the filters ...`): already `async`; add `await flushLookup();` after the dispatch; the expected `filters` literal gains `hide_kanji: false`.
- line 1700–1712 (`openFor sends current active filters ...`): add `await flushLookup();` after the dispatch; the literal gains `hide_kanji: false`.
- the `hoverChip` helper (search `async function hoverChip`): add `await flushLookup();` after its `focusin` dispatch.
- line 2003–2013 (`stops sending the size once it is cleared`): add `await flushLookup();` after the dispatch; literal gains `hide_kanji: false`.
- line 2022 (`omits the size when none is persisted`): literal gains `hide_kanji: false`.

Run: `npx vitest run src/main-tooltip.test.ts src/main.test.ts`
Expected: the rewritten and new tests fail (no `kanji_info` call, no `hide_kanji` key); every other test in both files still passes.

- [ ] **Step 5: `main.ts`**

Imports:

```ts
import { createKanjiLookup } from './kanji-lookup';
import type {
  ChromeMode,
  Entry,
  FuriganaMode,
  GlossFilters,
  KanjiInfo,
  ParseResult,
  Segment,
  Settings,
} from './types';
```

`currentFilters`:

```ts
const currentFilters: GlossFilters = {
  hide_pos: false,
  hide_xrefs: false,
  hide_usage: false,
  hide_kanji: false,
};
```

After it:

```ts
/** The popover's kanji block, asked once per character for the page's life. */
const lookupKanji = createKanjiLookup((chars) => invoke('kanji_info', { chars }));
```

The first-run copy at line 219:

```ts
      ? 'No dictionary yet. JMdict and KANJIDIC2 are a one-time download of roughly twelve megabytes from EDRDG.'
```

The two settings-apply sites: after each `hide_usage` line add the same shape for `hide_kanji` (`if (settings.hide_kanji !== undefined) { currentFilters.hide_kanji = settings.hide_kanji; }` in the block form at line 648, and the one-liner form at line 666).

`openFor`:

```ts
/** What `popover-content` carries. `kanji` is present only when there is a block to show. */
interface PopoverPayload {
  entries: Entry[];
  filters: GlossFilters;
  gloss?: number;
  kanji?: KanjiInfo[];
}

/** Ask the popup to render CHIP's entries, if the last parse still knows that span. */
function openFor(chip: HTMLElement): void {
  const segment = segmentAt(chip.dataset.start);
  const entries = segment?.entries;
  // No entries means a stale chip from a superseded parse, or an unmatched
  // run — neither is an error worth surfacing.
  if (segment === undefined || entries === undefined || entries.length === 0) return;
  // Bumped here, not only in `closePopover`: focus moving straight from chip
  // A to chip B (no `closePopover` in between — see the `focusin` listener
  // below) must still invalidate a `placeFor` already in flight for A, or
  // the two placements race and whichever resolves last wins the window.
  openId += 1;
  // A fresh open owns its own arm state. On the mouse path this is already
  // false (a `mouseout` cancels the dwell, so a completed dwell means the
  // cursor never left), and the focus path must not inherit a `true` left by
  // an earlier hover that never opened anything.
  keepArmed = false;
  pendingChip = chip;
  // Captured before the one await below and re-checked after it, exactly as
  // `placeFor` does: a dismissal, or the next chip's own open, that lands
  // while the kanji lookup is in flight must not emit content for a popover
  // nothing will place.
  const generation = openId;
  // The kanji of the SURFACE, not the headword (design §1.1): a PRONOUNCE
  // match shows the reader kana, and a block for 履 would explain a
  // character they are not looking at. Under `hide_kanji` there is nothing
  // to ask for, so no round trip is paid.
  const kanji = currentFilters.hide_kanji
    ? Promise.resolve<KanjiInfo[]>([])
    : lookupKanji(segment.surface);
  void kanji
    .then((records) => {
      if (openId !== generation) return;
      // Fire-and-forget: a failed emit just leaves the tooltip unshown, no
      // worse than the user never having hovered at all.
      // The size rides along with the content because the popover is a second
      // webview with its own `:root`, and it must be applied before that
      // render: the window sizes itself off what the render measures. The
      // kanji ride along for the same reason — one render, one measurement.
      const payload: PopoverPayload = { entries, filters: currentFilters };
      if (currentGloss !== null) payload.gloss = currentGloss;
      if (records.length > 0) payload.kanji = records;
      return emit('popover-content', payload);
    })
    .catch(() => {});
}
```

`segmentAt` already exists at line 755 and returns `Segment | undefined`, which carries `surface`.

- [ ] **Step 6: Run the tests**

Run: `npx vitest run && npx tsc --noEmit`
Expected: green across the suite. If a `main.test.ts` test that was not listed above now fails because it asserts `popover-content` synchronously after a `focusin`, add `await flushLookup();` there too and note it in the commit body; do not weaken the assertion.

- [ ] **Step 7: Commit**

```bash
git add src/render/tooltip-colour.ts src/kanji-lookup.ts src/kanji-lookup.test.ts src/main.ts src/main-tooltip.test.ts src/main.test.ts
git commit -m "feat(app): the main window asks kanji_info once per character and sends the block

openFor collects the distinct kanji of the hovered surface, asks kanji_info
for the ones a page-lifetime cache has not seen, and emits the answer beside
the entries in the same popover-content payload, so the popover renders and
measures once. The one added cost on the dwell path is a single IPC round
trip on first sight of a character; misses are cached too, so a character is
asked about once per session. The open's generation is captured before that
await and re-checked after it, the way placeFor already guards its own round
trip, so a dismissal in flight wins.

kanji is omitted when empty, which keeps every payload a kana-only word or an
index without KANJIDIC2 produces byte-identical to before. hide_kanji joins
the three gloss filters and short-circuits the lookup. The first-run message
names both files."
```

---

### Task 8: The setting, the footer, the README

**Files:**
- Modify: `src/settings.ts` — template (after line 38), element lookup (line 123), `currentSnapshot` (line 283), `applySettings` (line 304), listeners (line 337); footer after the last `</section>` (line 118)
- Modify: `src/styles/settings.css` — `.settings-footer`
- Modify: `src/settings.test.ts` — the restore test (line 55–87) and a new toggle test and a footer test
- Modify: `src-tauri/src/settings.rs` — field after `hide_usage` (line 119) and `Default` (line 169)
- Modify: `src-tauri/src/settings/tests.rs` — lines 150, 192, 221, 236–240
- Modify: `src-tauri/src/commands.rs` — `merge_saved` (line 1147) and `save_settings_updates_gloss_filters_in_settings_state` (line 1908–1918)
- Modify: `README.md:95,102`

**Interfaces:**
- Consumes: `Settings.hide_kanji` on both sides (Task 6 added the TS key).
- Produces: `#setting-hide-kanji` checkbox that saves and broadcasts `hide_kanji`; `Settings.hide_kanji: Option<bool>` persisted; `.settings-footer` attribution.

- [ ] **Step 1: Write the failing tests**

`src/settings.test.ts`: in the restore test (the `get_settings` mock at lines 55–65 and the assertions at 74–86) add `hide_kanji: true,` to the mock and

```ts
    const hideKanji = container.querySelector<HTMLInputElement>('#setting-hide-kanji');
    expect(hideKanji?.checked).toBe(true);
```

Append two tests to the same `describe`:

```ts
  test('toggling the kanji checkbox saves and broadcasts hide_kanji', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const hideKanji = container.querySelector<HTMLInputElement>('#setting-hide-kanji')!;
    hideKanji.checked = true;
    hideKanji.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({ hide_kanji: true }),
    });
    expect(emitted).toContainEqual([
      'settings-changed',
      expect.objectContaining({ hide_kanji: true }),
    ]);
  });

  // CC BY-SA's condition is attribution, and this window is the app's only
  // persistent surface for it (design §2.1). Text, not a link: no opener
  // plugin is installed, and an anchor would navigate this webview away.
  test('the footer attributes both EDRDG files and names the licence', () => {
    initSettingsApp(container);
    const footer = container.querySelector('.settings-footer')?.textContent ?? '';
    expect(footer).toContain('JMdict');
    expect(footer).toContain('KANJIDIC2');
    expect(footer).toContain('Electronic Dictionary Research and Development Group');
    expect(footer).toContain('CC BY-SA 4.0');
    expect(footer).toContain('https://www.edrdg.org/edrdg/licence.html');
    expect(container.querySelector('.settings-footer a')).toBeNull();
  });
```

`src-tauri/src/settings/tests.rs`: `defaults_leave_every_optional_key_unset` gains `assert_eq!(s.hide_kanji, None);`; both struct literals gain `hide_kanji: None,` and `hide_kanji: Some(false),` respectively (after `hide_usage`); `settings_roundtrips_gloss_filters` gains `assert!(json.contains("\"hide_kanji\":false"));` and `assert_eq!(deserialized.hide_kanji, Some(false));`.

`src-tauri/src/commands.rs`'s `save_settings_updates_gloss_filters_in_settings_state`: the literal gains `hide_kanji: Some(true),` and the assertions `assert_eq!(s.hide_kanji, Some(true));`.

- [ ] **Step 2: Run to see them fail**

Run: `npx vitest run src/settings.test.ts && cargo test --manifest-path src-tauri/Cargo.toml settings`
Expected: the TS tests fail (no element, no footer); Rust fails to compile (`hide_kanji` unknown).

- [ ] **Step 3: The settings window**

`src/settings.ts`, in the "Gloss &amp; Definitions" section after the `hide-usage` label:

```html
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-hide-kanji" />
          <span class="label-text">Hide kanji readings &amp; meanings</span>
        </label>
```

After the Background section's `</section>` and before `</div>`:

```html
      <footer class="settings-footer">
        <p class="settings-note">
          Dictionary data: JMdict and KANJIDIC2 &copy; Electronic Dictionary Research
          and Development Group, CC BY-SA 4.0. Licence:
          https://www.edrdg.org/edrdg/licence.html
        </p>
      </footer>
```

Element, snapshot, apply, listener — each one line beside its `hideUsage` twin:

```ts
  const hideKanji = container.querySelector<HTMLInputElement>('#setting-hide-kanji')!;
```

```ts
      hide_kanji: hideKanji.checked,
```

```ts
    if (s.hide_kanji !== undefined) hideKanji.checked = s.hide_kanji;
```

```ts
  hideKanji.addEventListener('change', saveAndBroadcast);
```

`src/styles/settings.css`, appended:

```css
/* The attribution the EDRDG licence asks for, on the one persistent surface
   the app has. Selectable, unlike the rest of the page, so the licence URL
   can be copied — it is text rather than a link because no opener plugin is
   installed and an anchor would navigate this webview away. */
.settings-footer {
  margin-top: auto;
  padding-top: 8px;
  border-top: 1px solid var(--color-rule);
}

.settings-footer .settings-note {
  user-select: text;
}
```

- [ ] **Step 4: The Rust side**

`src-tauri/src/settings.rs`, after `hide_usage` in the struct:

```rust
    /// Phase 3a: hides the KANJIDIC block at the foot of the popover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_kanji: Option<bool>,
```

and `hide_kanji: None,` in `Default`. `src-tauri/src/commands.rs` `merge_saved`, after the `hide_usage` arm:

```rust
    if incoming.hide_kanji.is_some() {
        current.hide_kanji = incoming.hide_kanji;
    }
```

`README.md` line 95:

```markdown
- **Dictionary:** [JMdict](https://www.edrdg.org/jmdict/j_jmdict.html) and [KANJIDIC2](https://www.edrdg.org/wiki/index.php/KANJIDIC_Project) (EDRDG)
```

line 102:

```markdown
- Dictionary data from the **EDRDG**'s JMdict and KANJIDIC2 is CC BY-SA 4.0 under its own notice (`crates/jparser/tests/fixtures/README.md`); the **IPADIC** conjugation data carries its notice in `crates/jparser/assets/IPADIC_NOTICE`.
```

- [ ] **Step 5: Run the tests**

Run: `npx vitest run && npx tsc --noEmit && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add src/settings.ts src/settings.test.ts src/styles/settings.css src-tauri/src/settings.rs src-tauri/src/settings/tests.rs src-tauri/src/commands.rs README.md
git commit -m "feat(settings): hide_kanji, and the attribution footer the licence gate needs

A fourth checkbox beside the three gloss filters, wired the same way on both
sides and None-defaulted so the block shows unless the reader turns it off.

The settings window gains a footer naming JMdict and KANJIDIC2, the EDRDG, and
CC BY-SA 4.0 with the licence URL: the smallest persistent attribution surface
the app has, and the first one beyond the first-run download message. The URL
is selectable text rather than a link, because no opener plugin is installed
and an anchor would navigate the settings webview itself."
```

---

### Task 9: Gates, and the record

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — §7: a new `### Phase 3a — The KANJIDIC layer — **COMPLETE**` entry after the Phase 2c one (line 1158–1170) and the Phase 3 table's "Kanji readings" row (line 1180)
- Create: `docs/superpowers/2026-09-11-engine-v2-phase3a-handoff.md`

- [ ] **Step 1: The engine gates**

Run the block under "Measuring the gates" above. Compare against Global Constraints line by line: both `score` runs (all cells, and the three compound classes, hints on), both `bench` scaling cells. Then build the gold index once *with* the fixture:

```bash
./target/release/jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$S/gold_index_k" --kanjidic crates/jparser/tests/fixtures/kanjidic_mini.xml
./target/release/jparser-cli score "$S/gold_index_k" "$G" --compounds "$C" --hints "$DIC"
```

Expected: identical to the hints-on row. Record all of it in the handoff. Any difference in any cell: stop, do not adjust the record, find the edit under `engine/` or `index/` that caused it.

- [ ] **Step 2: The snapshot and the suites**

```bash
cargo test -p jparser --test engine_v2
git status --short crates/jparser/tests/snapshots
```

Expected: pass; no `*.snap.new`. Then every standing gate from Global Constraints, plus:

```bash
cargo +1.85 check -p jparser -p jmdict-source
cargo llvm-cov -p jmdict-source
cargo llvm-cov -p jparser --features mecab
npx playwright test
```

Read the per-file rows, not the crate total: `kanjidic.rs`, `index/build.rs`, `index/load.rs`, `engine/build.rs`, `jmdict-source/src/lib.rs`, `jmdict-source/src/fetch.rs` must each be at or above 80% lines. A file below it gets a test in this task, not a note.

- [ ] **Step 3: A real launch**

Run `npm run tauri dev` once against the existing install. Expected sequence: the installed `dict/gen-N` is format 4, so startup routes to the download screen (`is_missing_dictionary` on `VersionMismatch`); the message names both files; "Download dictionary" reuses the `JMdict_e.gz` already in `%APPDATA%\dev.jparser.clipyomi\source`, downloads `kanjidic2.xml.gz` (~1.5 MB) beside it, builds, and the app comes up. Hover 東京都: three lines at the foot of the popover. Open Settings: the fourth checkbox hides them on the next hover; the footer is present. Note in the handoff what was seen, including the build time for the rebuild.

- [ ] **Step 4: The master plan and the handoff**

In the Phase 3 table, the "Kanji readings" row becomes:

```markdown
| Kanji readings | KANJIDIC2 (EDRDG) | **Shipped, Phase 3a (2026-09-11).** CC BY-SA 4.0, same publisher and licence as JMdict; attribution in the settings footer |
```

After the Phase 2c entry, add:

```markdown
### Phase 3a — The KANJIDIC layer — **COMPLETE**

One block at the foot of the popover, one line per distinct kanji in the
hovered surface: on and kun readings and up to four English meanings from
KANJIDIC2. Provisioned through the dictionary build (a sixth index file,
format 4 → 5, so existing installs rebuild once), fetched beside JMdict by
the same source crate, and reached lazily on hover through one command and a
page-lifetime cache, so the parse payload did not grow. `hide_kanji` joins
the gloss filters; the settings window gains the app's first persistent
attribution. Design: `specs/2026-09-09-engine-v2-phase3a-design.md`; record:
`2026-09-11-engine-v2-phase3a-handoff.md`. The engine was not touched, and
§5.2's baseline block is unchanged to the cell.
```

Create `docs/superpowers/2026-09-11-engine-v2-phase3a-handoff.md` in the shape of `2026-09-09-engine-v2-phase2c-handoff.md`: §1 what shipped (the file map above, collapsed to prose); §2 the gates, with the measured cells pasted verbatim beside the required ones and the test counts (`cargo test --workspace`, `-p jparser --features mecab`, `npm test`) before and after; §3 the deviations from the design and why (the five in Global Constraints); §4 what was seen on the real launch; §5 open items carried from design §7 (kun marks rendered raw; the four discarded KANJIDIC2 fields; attribution moves to an About surface if one exists; existing installs pay one rebuild) plus anything the work surfaced.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md docs/superpowers/2026-09-11-engine-v2-phase3a-handoff.md
git commit -m "docs(engine-v2): Phase 3a handoff, the KANJIDIC layer

Records the gates as measured: the baseline cells and bench scaling
unchanged in both hint states with and without kanji.bin present, the
snapshot byte-identical, the suites and coverage per file, and one real
launch through the upgrade rebuild. Marks the layer shipped in the master
plan's Phase 3 table and lists the five places the implementation departs
from the design, each with its reason."
```

---

## Self-review

**Spec coverage.** §1 block content and placement: Task 6. §1 exclusions (nanori, misc fields, unmatched runs): Task 1 discards; nothing renders for unmatched runs because `openFor` returns on empty entries. §1.1 every row: purpose (Tasks 1, 6); provisioning through the build (Tasks 2, 4); lazy via one command (Tasks 5, 7); once per segment at the foot (Task 6); surface not headword (Task 7 uses `segment.surface`); on + kun, nanori dropped (Task 1); English capped at four (Task 1); `hide_kanji` default shown (Tasks 7, 8); second module in `jmdict-source` (Task 3 — one crate, no rename). §2 licence in writing: the design records it; §2.1 attribution footer and first-run message: Tasks 8 and 7. §3 `kanji.bin`, version 5, `Index::kanji` binary search, whole-file read: Task 2 (header field: deliberate deviation, pinned by test). §3.1 parser: Task 1. §3.2 `build_from_reader` wrapper, `build_from_readers`, `ensure_dictionary` pair, CLI `--kanjidic`: Tasks 2, 4. §4 descriptor, prologs, cap, shared staging and retry, `resolve_kanjidic`, hand-placed file honoured, failure coupling: Tasks 3, 4. §5.1 main webview steps 1–4: Task 7. §5.2 command, wire pin: Task 5. §5.3 flat text, `colourLine`, empty and `hide_kanji` and older-payload cases: Task 6. §5.4 settings: Task 8. §6 every listed test: parser (Task 1), format (Task 2), builder (Task 2), fetch (Task 3), command (Task 5), popover (Task 6), main (Task 7), wire (Task 5), settings (Task 8); gates 1–5 (Task 9; CI file unchanged by design). §7 open items: carried into the handoff (Task 9).

**Placeholder scan.** No TBDs. Each step carries its code or its exact command. The two "existing doc unchanged" markers refer to text the executor is not to alter, not to text left unwritten.

**Type consistency.** `KanjiData { literal: char, on, kun, meanings }` in Tasks 1, 2, 5 and `KanjiInfo { literal: string, ... }` in Tasks 5–7 agree field for field. `build_from_readers(xml, kanjidic, out_dir)` in Tasks 2, 4. `build_new(root, jmdict, kanjidic)` in Task 4 only. `ensure_dictionary(root, keep, || Ok((j, k)))` in Tasks 4 and 5's fixture. `open_local(&Source, &Path)`, `resolve_from(&Source, url, dir, backoff)`, `fetch_from(&Source, url, dir)`, `fetch_with_retry(&Source, url, dir, backoff)` in Task 3 throughout. `lookup_kanji(&Index, &str) -> Vec<KanjiData>` and `kanji_info(String, State<IndexSender>) -> Vec<KanjiData>` in Task 5. `assembleTooltipText(entries, filters?, kanji = [])` and `renderTooltip(entries, filters?, kanji = [])` in Task 6; `createKanjiLookup(ask)(surface): Promise<KanjiInfo[]>` in Task 7. `BuildReport.kanji` / `.skipped_kanji` in Tasks 2 and 4's CLI output.
