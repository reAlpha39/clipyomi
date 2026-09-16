# Engine v2 Phase 3a — the KANJIDIC layer

**Date:** 2026-09-09
**Parent:** master plan §7, Phase 3, row "Kanji readings | KANJIDIC (EDRDG) |
CC BY-SA, same family as JMdict". That row is the whole of what the plan says
about this layer; this document is its scope.
**Status:** design approved in session; implementation plan to follow.

## 1. What it does, and what it deliberately does not

When the reader hovers a word, the popover today shows the word: every JMdict
match as `headword【reading】` followed by its senses. It shows nothing about
the characters the word is written with. This layer adds one block at the foot
of the popover, one line per distinct kanji in the hovered surface:

```text
東  トウ・ひがし  east, eastern
京  キョウ・ケイ・みやこ  capital, 10**16
都  ト・ツ・みやこ  metropolis, capital, all, everything
```

Readings are KANJIDIC2's `ja_on` and `ja_kun`; meanings are its English ones.
The purpose chosen was **readings and meanings together**: the meanings are
what let a reader decode an unfamiliar compound from its parts, which is the
one thing the word-level popover cannot do, and they come from the same record
as the readings at no extra cost.

Not in this layer, by decision: nanori (name readings); stroke count, grade,
JLPT level and newspaper frequency rank, all of which KANJIDIC2 carries and
which the parser reads and discards so a later layer can keep them with one
line; kanji lines for unmatched runs (a segmentation-adjacent use the plan's
own definition of a Phase 3 layer excludes); and any change to the popover's
layout beyond appending one block.

### 1.1 Decisions taken in session

| decision | choice | why |
| --- | --- | --- |
| Purpose | readings + meanings | decode-from-parts value; same record |
| Provisioning | folded into the dictionary build | one asset pipeline, one failure mode the app already handles; version bump forces the rebuild that surfaces it |
| Data path to the popover | lazy, on hover, via one Tauri command | the parse payload does not grow; it is already ~3x since k = 3, and the 10,000-character paste would otherwise pay for kanji nobody hovers |
| Placement | once per segment, at the foot | `renderTooltip` already takes one segment's entries; per-entry would repeat 東京都's three characters for every sense |
| Which characters | those in the **surface**, not the headword | a `PRONOUNCE` match (はいた → 履く) shows the reader kana; a 履 block would explain a character they are not looking at |
| Readings shown | on + kun; nanori dropped | nanori is noise for reading running text |
| Meanings shown | English only, capped at 4 at build time | KANJIDIC2 gives up to ~10 for common characters; four is the decode-from-parts need |
| Toggle | `hide_kanji`, default shown | joins the three existing gloss filters |
| Source crate | a second module in `jmdict-source`, not a new crate | same publisher, same fetch shape; renaming the crate is churn for nothing |

## 2. Licence, established in writing

Master plan §7: "No layer ships without its licence established in writing
first."

EDRDG's licence statement at `https://www.edrdg.org/edrdg/licence.html`
lists "KANJIDIC2 — File of Information about the Kanji in JIS X 0208, JIS X
0212 and JIS X 0213 in XML format" among the covered files, states that
copyright is held by James William Breen and the Electronic Dictionary
Research and Development Group, and under "Licence Conditions": "The
dictionary files are made available under a Creative Commons
Attribution-ShareAlike Licence (V4.0)." That is the same licence and the same
publisher as JMdict, which the index already ships under; §3.3 of the master
plan records that CC BY-SA constrains the index and not the code.

The file is `https://www.edrdg.org/kanjidic/kanjidic2.xml.gz`, 1,488,572
bytes on 2026-09-09 and last modified that same day. **It is regenerated
continuously, like JMdict, so it cannot be SHA-pinned** — the `vibrato-source`
comment that IPADIC is pinned by hash "where JMdict (rebuilt daily) can only
be checked structurally" applies here verbatim. The structural check is
§4's.

### 2.1 Attribution

CC BY-SA's condition is attribution, and today JMdict's attribution inside
the running app is one sentence in the first-run download message
(`src/main.ts`). Adding a second EDRDG file under the same licence is the
moment to do this properly. The settings window gains a footer line:

> Dictionary data: JMdict and KANJIDIC2 © Electronic Dictionary Research and
> Development Group, CC BY-SA 4.0

linking `https://www.edrdg.org/edrdg/licence.html`. The first-run message is
updated to name both files and the combined size. This is the smallest
persistent attribution surface the app has; it is in scope because the
licence gate is not met without it.

## 3. Index format: the sixth file

`crates/jparser/src/index/build.rs::write_index` is the format's one writer
and emits five files: `keys.fst`, `records.bin`, `entries.bin`,
`entries.idx`, `header.bin`. It gains a sixth, `kanji.bin`: a single bincode
blob of `Vec<KanjiData>` sorted by codepoint.

```rust
/// One KANJIDIC2 character, as the popover needs it.
pub struct KanjiData {
    pub literal: char,
    /// `ja_on` readings, katakana as KANJIDIC2 gives them.
    pub on: Vec<String>,
    /// `ja_kun` readings. KANJIDIC2's `.` (okurigana boundary) and `-`
    /// (affix) marks are kept: `ひがし`, `た.べる`, `-がた`.
    pub kun: Vec<String>,
    /// `<meaning>` elements with no `m_lang` attribute -- English --
    /// capped at four at build time.
    pub meanings: Vec<String>,
}
```

About 13,000 records; well under 2 MB serialised. `IndexHeader` gains
`kanji: u32`. **`INDEX_FORMAT_VERSION` goes 4 → 5.** That bump is what makes
an existing install rebuild: `ensure_dictionary` already treats
`VersionMismatch` as "rebuild", so a user upgrading gets the block on first
launch with no new code path.

`Index::open` reads `kanji.bin` fully into a `Vec<KanjiData>` and exposes

```rust
pub fn kanji(&self, ch: char) -> Option<&KanjiData>
```

as a binary search by codepoint. No mmap and no FST: 13k small records earn
neither, and a whole-file read keeps `open`'s failure modes the ones it has.

### 3.1 The parser

`crates/jparser/src/kanjidic.rs`, beside `jmdict.rs` and in its style: a
streaming `quick-xml` reader over `<character>` elements, collecting
`<literal>`, every `<reading r_type="ja_on">` and `r_type="ja_kun">` in
`<rmgroup>`, and every `<meaning>` without `m_lang`. `<nanori>`, every other
`r_type`, and every other element are read and discarded. Entities: KANJIDIC2
declares none of JMdict's POS entities, so the entity spike `jmdict.rs`
records does not recur here; the file does carry a DTD, which the reader
skips as it skips JMdict's.

### 3.2 Builders

`engine::build::build_from_reader(jmdict, dir)` **keeps its signature** and
becomes a wrapper over a new

```rust
pub fn build_from_readers(jmdict: impl BufRead, kanjidic: impl BufRead, dir: &Path)
```

passing an empty reader. An empty reader yields a valid `kanji.bin` with zero
records, so every existing fixture, test, example and probe builds unchanged
and `Index::kanji` simply returns `None` on those indexes. Only two callers
use the two-reader form: the app's startup, and `jparser-cli build-index`,
which gains an optional `--kanjidic <path>`.

`index::ensure_dictionary(root, keep, source)` takes its one `FnOnce() ->
BufRead` today; it takes a pair. The Tauri startup closure supplies both
resolvers from §4.

Nothing under `crates/jparser/src/engine/` other than `build.rs` changes.
`lex`, `lattice`, `cost`, `assemble`, `table` are untouched, which is what
lets §6 demand the baseline cells byte-identical.

## 4. Fetch: one descriptor, two sources

`crates/jmdict-source` keeps its name and gains `src/kanjidic.rs`. Its
`fetch::fetch_from(url, dir)`, the gzip-magic sniff, the prolog check and
the size cap generalise over

```rust
pub struct Source {
    pub url: &'static str,
    pub file_name: &'static str,
    /// Byte prefixes an ungzipped file may start with. Matched against the
    /// specific file rather than `<`, for the reason `lib.rs` gives: a
    /// saved error page must not build an empty index and report success.
    pub prologs: &'static [&'static [u8]],
    pub max_bytes: u64,
}
```

`SOURCE_FILE`, `PROLOGS` and the existing cap become the JMdict descriptor;
a second describes `kanjidic2.xml.gz` with `<?xml`, `<!DOCTYPE kanjidic2`
and `<kanjidic2>` as prologs and a cap sized the way JMdict's is — the
current 1.49 MB with headroom for growth. The PID-staged `.partial` naming,
the retry and backoff, the concurrent-download rule and `open_local`'s
compression-by-bytes decision are shared, not copied. `resolve(dir)` gains a
sibling `resolve_kanjidic(dir)`; a hand-placed `kanjidic2.xml.gz` or
`kanjidic2.xml` in the source directory is honoured exactly as a hand-placed
JMdict is.

A KANJIDIC2 fetch failure fails the dictionary build the way a JMdict failure
does, and reaches the user through the same first-run screen. That coupling
was chosen knowingly in §1.1 over a silent-degrade path nobody would test.

## 5. Runtime: one command, one payload field, one filter

### 5.1 Main webview

The popover opens after a dwell (`src/main.ts`), by `emit('popover-content',
{ entries, filters, gloss })` to the second webview. Before that emit, main
now:

1. collects the distinct kanji in the hovered segment's `surface`, in order,
   using `isIdeograph` from `src/render/tooltip-colour.ts` — exported, not
   reimplemented;
2. looks each up in a page-lifetime `Map<string, KanjiInfo | null>`;
3. for the misses, awaits one `invoke('kanji_info', { chars })` and fills the
   map — `null` for a character the index has no record for, so a miss is
   asked once;
4. emits the payload with `kanji: KanjiInfo[]` beside the existing fields, in
   surface order, misses omitted.

The one added cost is a single IPC round trip on the dwell path, a binary
search over 13k records. A second hover on any character anywhere in the
session costs nothing.

### 5.2 Backend

```rust
#[tauri::command]
pub fn kanji_info(chars: String, state: State<'_, ...>) -> Vec<KanjiInfo>
```

reads `AppState.index` and returns `Index::kanji` hits in input order,
omitting misses and non-kanji input. `KanjiInfo` mirrors `KanjiData` on the
wire — `literal`, `on`, `kun`, `meanings` — and is declared in
`src/types.ts` beside `Entry`. The Rust test that pins `Entry`'s wire field
names (`types.ts`'s header names it) pins `KanjiInfo`'s too, so the two sides
cannot drift silently.

### 5.3 Popover

`renderTooltip(entries, filters, kanji)` appends, after the entries' lines
and one blank line, one line per kanji as flat text —

```text
東  トウ・ひがし  east, eastern
```

— so `colourLine`, which is lexical and reads characters, colours the kana
runs and the ideograph as it colours everything else. `popover.ts` passes the
payload's `kanji` through; it stays a renderer. Nothing renders when the
array is empty or `filters.hide_kanji` is set, and a payload without the
field (an older main webview, during a hot reload) renders exactly as today.

### 5.4 Settings

`GlossFilters` and `Settings` gain `hide_kanji?: boolean` in `src/types.ts`
and both `settings.rs` structs, `None`-defaulted like the other three. A
fourth checkbox joins `hide_pos`, `hide_xrefs`, `hide_usage` in
`src/settings.ts`, wired the same way. Default shown.

## 6. Testing and gates

**Compatibility is the cheap part and the point of §3.2**: because
`build_from_reader` is unchanged, no existing test, fixture, snapshot or probe
is touched by the format change beyond the header version.

Tests added:

- **Parser** — a KANJIDIC2 fixture of three characters through
  `kanjidic.rs`: one with `m_lang` meanings (dropped), one with `<nanori>`
  (dropped), one with a kun reading carrying `.` and `-` (kept verbatim); a
  fourth with more than four meanings, capped.
- **Format** — write and reopen with records; `Index::kanji` hit and miss; a
  v4 header refused by v5 `open` with `VersionMismatch`, in the shape of the
  existing fingerprint-mismatch test.
- **Builder** — `build_from_reader` yields a zero-record `kanji.bin` that
  opens and misses; `build_from_readers` yields records.
- **Fetch** — the KANJIDIC descriptor's prolog check accepts the three real
  prologs and refuses an HTML error page, mirroring the JMdict tests.
- **Command** — `kanji_info` with hits, misses, kana input, and an empty
  string.
- **Popover** — `renderTooltip` with the block, without it, and under
  `hide_kanji`; the blank-line separator; the flat-text line shape.
- **Main** — the hover path invokes once for two hovers of the same
  character and omits `kanji` when the surface has none (`main.test.ts`
  already mocks `invoke`).
- **Wire** — the field-name pin extended to `KanjiInfo`.
- **Settings** — the fourth checkbox round-trips like the other three.

Gates, in the order that matters:

1. **Baseline cells byte-identical, both hint states**: strict 0.8174 (1768),
   tolerant 0.8525 (1844), entry top-1 0.9092, compounds 0.6129 (114 / 186)
   with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42
   hints on; 0.7342 / 0.7924 / 0.9093 / 0.5860 hints off. This layer touches
   nothing under `engine/` but `build.rs`, so any movement is a defect.
2. `jparser-cli bench` scaling cell unchanged (1.10x hints on, 1.05x off).
3. The `parse_sentences.txt` snapshot byte-identical.
4. jparser with `mecab` and with `hints-source`, workspace, src-tauri, npm,
   `tsc --noEmit`, playwright; `cargo clippy` under `-D warnings` for the
   workspace and both feature sets; `cargo fmt --all --check`;
   `cargo +1.85 check`; `cargo llvm-cov` over 80 with **the new files read
   per-file** — a crate-level gate hid a 41% file this week.
5. CI: the `mecab` jobs are unaffected; `coverage` picks up the new code. The
   first-run download in `ci.yml`'s `hints` step is IPADIC, not this, so
   nothing there changes.

## 7. Open items, stated rather than hidden

- **Existing installs pay one rebuild** on first launch after upgrade, and
  that rebuild now downloads a second file. The first-run screen already
  covers a download in progress; it should name what it is fetching.
- **Kun readings keep KANJIDIC2's marks** (`た.べる`, `-がた`). Rendering
  them raw is a deliberate first cut: the marks are informative to a reader
  who knows them and harmless to one who does not. Stripping or styling them
  is a popover-text question for later, not a data question.
- **The four discarded fields** (grade, stroke count, JLPT, frequency) are
  one line each to keep. KANJIDIC2's `jlpt` field would partly cover the
  plan's separate "JLPT level" row, whose community sources the plan calls
  unofficial; that is a separate decision and this document does not take
  it.
- **Attribution placement** is the settings footer because it is the only
  persistent surface the app has. If an About surface ever exists, it moves
  there.
