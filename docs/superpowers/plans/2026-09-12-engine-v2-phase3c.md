# Engine v2 Phase 3c — the KANJIDIC layer close-out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three items the Phase 3b handoff left open around the kanji block: kun readings lose KANJIDIC2's affix hyphen at render time and keep the okurigana dot; the source sniff becomes root-aware on both the plain and the gzip path; and the dictionary commands leave `src-tauri/src/commands.rs` for two modules of their own.

**Architecture:** Three independent changes with no shared code. (1) `kanjiLine` in `src/render/tooltip-text.ts` trims `-` from both ends of each kun reading before the join; the record on disk and on the wire is untouched. (2) `crates/jmdict-source/src/lib.rs` replaces the three-prefix `prologs` rule with one root name per `Source`; `open_local` peeks a fixed 8 KiB from the file — and, when the file is gzip, from the decoder — then requires `<!DOCTYPE {root}` or `<{root}` behind any declaration and comments, accepting only when the peek was cut short by its own size rather than by the end of the file. (3) The dictionary cluster of `commands.rs` — its six state structs, the index download, the kanji lookup, the hints acquisition and their tests — moves verbatim into `src-tauri/src/commands/dictionary.rs` and `src-tauri/src/commands/hints.rs`, and `main.rs` names the new paths. Nothing under `crates/jparser/src/engine/` or `crates/jparser/src/index/` is edited, `INDEX_FORMAT_VERSION` stays at 6, and no install rebuilds.

**Tech Stack:** Rust 2021 (`jmdict-source`: `flate2` 1, `thiserror`; `src-tauri`: Tauri 2, `tokio` watch channels), TypeScript + Vite frontend tested with Vitest 4. MSRV 1.85 for the library crates.

**Spec:** `docs/superpowers/specs/2026-09-12-engine-v2-phase3c-design.md` — read it first; this plan argues from it. Facts the plan leans on, measured in session: the 2026-09-11 KANJIDIC2 file has 16,036 kun readings, 392 with a hyphen, every one at an edge (266 leading, 126 trailing, none interior), 8,349 with a dot; the real archives put `<!DOCTYPE` at byte 1,135 (JMdict) and 39 (KANJIDIC2) and their root elements at 23 KB and 13 KB; `Source.prologs` is read nowhere outside `lib.rs`; `commands.rs` is 2,719 lines at `b287236`, the dictionary cluster is lines 1281–1694 and its tests are lines 1721–1817, 2218–2261 and 2262–2718.

## Global Constraints

- **Baseline cells byte-identical, both hint states** (spec §6 gate 1): hints on strict `0.8174 (1768)`, tolerant `0.8525 (1844)`, entry top-1 `0.9092`, compounds `0.6129 (114 / 186)` with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42; hints off `0.7342 (1588)`, `0.7924 (1714)`, `0.9093`, `0.5860 (109 / 186)`. **Nothing under `crates/jparser/src/engine/` or `crates/jparser/src/index/` is edited by this plan**, so any movement is a defect in this plan's work.
- **Bench scaling cell read as a band, both states** (spec §6 gate 2): hints off recorded `1.05x`, measured `0.98x`–`1.07x` across seven runs of identical code; hints on recorded `1.10x`, measured `1.07x`–`1.12x` across four. Inside is a pass; outside is a re-run, not an explanation.
- **The `parse_sentences.txt` insta snapshot byte-identical**: `git status --short crates/jparser/tests/snapshots` prints nothing after `cargo test -p jparser --test engine_v2`.
- **Standing gates**, before every commit: `cargo test --workspace`, `cargo test -p jparser --features mecab`, `cargo test -p jparser --features hints-source`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo clippy -p jparser --features mecab --all-targets -- -D warnings`, `cargo clippy -p jparser --features hints-source --all-targets -- -D warnings`, `cargo check -p jparser --no-default-features --all-targets`, `cargo fmt --all --check`; `npm test` and `npx tsc --noEmit` before every commit that touches `src/`. Baselines from the Phase 3b handoff §2 at `e37a408`: jparser mecab 373, hints-source 374, workspace 605 passed and 1 ignored, `src-tauri` 152, npm 312, playwright 45. **Task 1 Step 1 re-takes these at the branch tip before anything changes**, since commits landed after `e37a408`. `TA_HINTS_DICT` or `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic` must exist for the two IPADIC-pinned `engine_v2` tests.
- **No new dependency, no new Tauri command, no new settings field, no CSS change, no index format change.** `INDEX_FORMAT_VERSION` stays 6.
- **Kun readings**: `-` (ASCII hyphen-minus, U+002D) is trimmed from both ends of each kun reading in the popover only; `.` stays; on readings are not touched; `KanjiData`, the wire and the parser keep KANJIDIC2's notation and `keeps_okurigana_and_affix_marks_verbatim` keeps passing.
- **The sniff**: `Source.prologs` is replaced by `root: &'static str` (`"JMdict"`, `"kanjidic2"`). Accept means: after an optional BOM, whitespace, one `<?xml … ?>` and any comments, `<!DOCTYPE {root}` or `<{root}` followed by whitespace, `>`, `[` or `/`. A head cut short by the 8 KiB peek accepts; a head cut short by the end of the file is decided on what it holds. The gzip branch is sniffed on its decompressed head. The refusal is `SourceError::NotAnArchive`, unchanged in shape.
- **The split is a move, not an edit**: every function, struct, constant, doc comment and test moves verbatim; the only edits are `use` blocks, three `pub(super)` visibilities, module docs, `main.rs` paths and one prose reference in `parse.rs`. `tauri::generate_handler!` needs the real module path, so no `pub use` re-export.
- **Work proceeds directly on `feat/engine-v2`, no worktree.** Every commit stages only the files its task names — never `git add -A`. The working tree carries the user's uncommitted edits to `README.md` and `src-tauri/Cargo.toml`, which are never staged.
- **Docs, code comments, commit messages and UI copy are written in normal prose**, not the session's terse style. Commits follow the repository's `type(scope): description` form with a prose body. The two new Rust files carry `// SPDX-License-Identifier: MIT OR Apache-2.0` as their first line, like every `src-tauri` file.

### Measuring the gates

```bash
cargo build --release -p jparser --features mecab --bin jparser-cli
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase3c"
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

---

## File map

| File | Change | Task |
| --- | --- | --- |
| `src/render/tooltip-text.ts` | `kunReading` helper; `kanjiLine` maps kun readings through it; doc comment | 1 |
| `src/render/tooltip-text.test.ts` | `head`/`line` hoisted to the kanji-block scope; a `kun-reading marks` describe with three tests | 1 |
| `src/render/tooltip-colour.test.ts` | one test on a kun reading's runs | 1 |
| `crates/jmdict-source/src/lib.rs` | `Source.root`; `PEEK_BYTES`; `begins_xml` rewritten; `find`, `peek`, `sniffed`; `open_local` rewritten; `NotAnArchive` message and docs; tests | 2 |
| `src-tauri/src/commands/dictionary.rs` | **new** — the index download, kanji lookup, shared slot and `reparse`, their tests | 3 |
| `src-tauri/src/commands/hints.rs` | **new** — the hints acquisition, warning, directory, their tests | 3 |
| `src-tauri/src/commands.rs` | the cluster and its tests removed; `pub mod` lines; module doc; pruned imports | 3 |
| `src-tauri/src/main.rs` | six handler paths and five `manage` paths | 3 |
| `src-tauri/src/parse.rs` | one doc-comment path | 3 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | a `### Phase 3c` entry after the `### Phase 3b` entry | 4 |
| `docs/superpowers/2026-09-12-engine-v2-phase3c-handoff.md` | **new** | 4 |

---

### Task 1: Kun readings drop the affix hyphen

**Files:**
- Modify: `src/render/tooltip-text.ts` — the `kanjiLine` function and its doc comment (lines 92–104)
- Test: `src/render/tooltip-text.test.ts` — inside `describe('the kanji block', …)` (lines 187–299)
- Test: `src/render/tooltip-colour.test.ts` — after the Phase 3b test (line 122)

**Interfaces:**
- Consumes: `KanjiInfo { literal: string; on: string[]; kun: string[]; meanings: string[]; grade?, strokes?, freq?, jlpt? }` from `src/types.ts`; `assembleTooltipText(entries, filters?, kanji?)`; `colourLine(line): Run[]` where `Run = { text: string; kind: 'conj' | 'paren' | 'kana' | 'kanji' | 'text' }`.
- Produces: nothing another task consumes. `kunReading(reading: string): string` stays module-private.

- [ ] **Step 1: Take the suite baselines at the branch tip**

```bash
git log --oneline -1
cargo test --workspace 2>&1 | grep -E "^test result"
cargo test -p jparser --features mecab 2>&1 | grep -E "^test result"
cargo test -p jparser --features hints-source 2>&1 | grep -E "^test result"
cargo test -p clipyomi 2>&1 | grep -E "^test result"
npm test 2>&1 | grep -E "Tests "
npx playwright test 2>&1 | grep -E "passed|failed"
```

Record the commit hash and the six totals in a scratch note — `cargo test` prints one `test result` line per test binary, so sum them as the Phase 3b handoff did. Task 4's handoff quotes them as the "before" column. Expected: in the neighbourhood of the Global Constraints figures (605+1 / 373 / 374 / 152 / 312 / 45), all passing. This is the first phase to take the npm and Playwright figures at its own tip rather than inherit them; say so in the note.

- [ ] **Step 2: Write the failing tests**

In `src/render/tooltip-text.test.ts`, inside `describe('the kanji block', () => {`, move the `head` and `line` helpers up to the block's scope so both inner describes share them. The block currently opens:

```ts
  describe('the kanji block', () => {
    const tokyo: KanjiInfo[] = [
      { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east', 'eastern'] },
      { literal: '京', on: ['キョウ', 'ケイ'], kun: ['みやこ'], meanings: ['capital', '10**16'] },
    ];
```

Make it:

```ts
  describe('the kanji block', () => {
    const tokyo: KanjiInfo[] = [
      { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east', 'eastern'] },
      { literal: '京', on: ['キョウ', 'ケイ'], kun: ['みやこ'], meanings: ['capital', '10**16'] },
    ];
    const head = '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n';
    /** One kanji's rendered line, with the entry block ahead of it cut off. */
    const line = (k: KanjiInfo): string =>
      assembleTooltipText([entry()], undefined, [k]).slice(head.length);
```

Then in `describe('the misc aside', () => {` delete its own two definitions so it opens:

```ts
    describe('the misc aside', () => {
      const east = tokyo[0];
```

Now add, immediately before `// Phase 3b: KANJIDIC2's <misc> numbers ride the same line …`:

```ts
    // Phase 3c: KANJIDIC2 marks a reading used only as a suffix or a prefix
    // with a hyphen (`-ぐ.い`, `ひと-`) and the okurigana boundary with a dot
    // (`た.べる`). The hyphen is trimmed at render time; the dot stays, since
    // it shows where the kanji stops and the reader's own kana begin. The
    // record on the wire keeps both marks — this is the popover's choice.
    describe('kun-reading marks', () => {
      test('trims the affix hyphen from either end of a kun reading', () => {
        expect(
          line({ literal: '食', on: ['ショク'], kun: ['た.べる', '-ぐ.い'], meanings: ['eat'] }),
        ).toBe('食  ショク・た.べる・ぐ.い  eat');
        expect(line({ literal: '一', on: [], kun: ['ひと-', 'ひと.つ'], meanings: ['one'] })).toBe(
          '一  ひと・ひと.つ  one',
        );
      });

      test('keeps the okurigana dot', () => {
        expect(line({ literal: '食', on: [], kun: ['た.べる'], meanings: [] })).toBe('食  た.べる');
      });

      test('leaves on readings alone', () => {
        expect(line({ literal: '東', on: ['トウ'], kun: [], meanings: ['east'] })).toBe(
          '東  トウ  east',
        );
      });
    });
```

In `src/render/tooltip-colour.test.ts`, after the closing `});` of `a kanji line with its misc aside ends in one paren run` (line 122) and before the file's final `});`, add:

```ts
  // Phase 3c: the affix hyphen is gone from kun readings before the line
  // reaches this module, so a reading that carried one is a plain kana run
  // again. The okurigana dot is not Japanese to this classifier and still
  // splits the run, as it always has; the middle dot rides in the kana run.
  test('a kun reading keeps its okurigana dot as a text run', () => {
    expect(colourLine('食  ショク・た.べる・ぐ.い  eat')).toEqual([
      { text: '食', kind: 'kanji' },
      { text: '  ', kind: 'text' },
      { text: 'ショク・た', kind: 'kana' },
      { text: '.', kind: 'text' },
      { text: 'べる・ぐ', kind: 'kana' },
      { text: '.', kind: 'text' },
      { text: 'い', kind: 'kana' },
      { text: '  eat', kind: 'text' },
    ]);
  });
```

- [ ] **Step 3: Run the tests to verify the new ones fail**

```bash
npx vitest run src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts
```

Expected: the colour test **passes** (it pins behaviour the colouriser already has — that is deliberate, it is the regression guard for the line shape) and the three `kun-reading marks` tests **fail** with received strings still carrying `-ぐ.い` and `ひと-`. Every other test in both files passes, including the moved-helper `misc aside` tests.

- [ ] **Step 4: Implement `kunReading`**

In `src/render/tooltip-text.ts`, replace the `kanjiLine` function and its doc comment (lines 92–104) with:

```ts
/**
 * A kun reading as the popover shows it. KANJIDIC2 writes a hyphen on a
 * reading used only as a suffix (`-がた`) or only as a prefix (`ひと-`) and a
 * dot at the okurigana boundary (`た.べる`). The hyphen is trimmed: it is a
 * lexicographer's fact that reads as a stray mark beside the middle-dot
 * separators. The dot stays: it shows where the kanji stops and the reader's
 * own kana begin, which is how the word is written. In the 2026-09-11 file
 * every hyphen sits at an edge (266 leading, 126 trailing, none interior), so
 * trimming both ends is the whole rule. The record itself keeps the notation;
 * this is a rendering choice (Phase 3c design §3).
 */
function kunReading(reading: string): string {
  return reading.replace(/^-+|-+$/g, '');
}

/**
 * One kanji's line: the literal, its on then kun readings joined by the
 * katakana middle dot, its English meanings, and the misc aside. Columns are
 * two spaces apart; an empty column is dropped rather than left as a gap.
 * Kun readings pass through `kunReading` first.
 */
function kanjiLine(kanji: KanjiInfo): string {
  const readings = [...kanji.on, ...kanji.kun.map(kunReading)].join('・');
  return [kanji.literal, readings, kanji.meanings.join(', '), kanjiAside(kanji)]
    .filter((column) => column !== '')
    .join('  ');
}
```

- [ ] **Step 5: Run the tests to verify they pass**

```bash
npx vitest run src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts
npm test 2>&1 | grep -E "Tests "
npx tsc --noEmit
```

Expected: both files green; the npm total is Step 1's figure plus four; `tsc` prints nothing.

- [ ] **Step 6: Commit**

```bash
git add src/render/tooltip-text.ts src/render/tooltip-text.test.ts src/render/tooltip-colour.test.ts
git commit -m "feat(popover): kun readings drop KANJIDIC2's affix hyphen

The kanji line trims the hyphen KANJIDIC2 puts on a suffix-only or
prefix-only kun reading and keeps the okurigana dot, so 食's last reading
renders as ぐ.い rather than -ぐ.い. Render-time only: the record on disk
and on the wire keeps the notation, and the parser's own test that pins it
is untouched. In the 2026-09-11 file every one of the 392 hyphenated kun
readings carries the mark at an edge, so trimming both ends is exact."
```

---

### Task 2: The sniff reads the root name, gzipped or plain

**Files:**
- Modify: `crates/jmdict-source/src/lib.rs` — the imports (line 15); the `Source` doc and struct (lines 55–82); the two `*_PROLOGS` constants and the two `Source` constants (lines 84–103); `begins_xml` (lines 109–114); the `NotAnArchive` variant (lines 139–160); `open_local` and its doc (lines 172–225); the test module (lines 278 onward)

**Interfaces:**
- Consumes: `Source { url, file_name, prologs, max_bytes }`, `open_local(source, path) -> Result<Box<dyn BufRead>, SourceError>`, `SourceError::NotAnArchive { path, head, file }`, `GZIP_MAGIC`, `UTF8_BOM`, the test helpers `scratch`, `gz`, `read_all`, `XML`, `KANJIDIC_XML`.
- Produces: `Source { url, file_name, root: &'static str, max_bytes }` — `prologs` is gone, and nothing outside `lib.rs` read it (verified by grep at `b287236`: the only hits are `lib.rs` and old `.superpowers/sdd` briefs). `open_local`'s signature and `SourceError`'s variants are unchanged; `NotAnArchive`'s message text changes (see Step 4). Task 4 reads the message on the real file.

- [ ] **Step 1: Write the failing tests**

In the test module of `crates/jmdict-source/src/lib.rs`, replace `every_kanjidic_prolog_is_accepted` (the test and its doc comment, lines 470–486) with the two tests below, and rewrite the doc comment on `a_trimmed_jmdict_file_under_the_kanjidic_name_is_rejected` (lines 488–495). Then append the rest after `an_html_page_under_the_kanjidic_name_is_rejected`, before `a_present_kanjidic_archive_is_opened_without_downloading`.

```rust
    /// Each opening a real or hand-trimmed KANJIDIC2 can have passes the
    /// root-aware check: declaration then root, DOCTYPE then root, bare
    /// root, and — the shape of the published file — declaration, comments,
    /// DOCTYPE.
    #[test]
    fn every_legitimate_kanjidic_opening_is_accepted() {
        for (i, head) in [
            &b"<?xml version=\"1.0\"?><kanjidic2></kanjidic2>"[..],
            b"<!DOCTYPE kanjidic2 [\n]><kanjidic2></kanjidic2>",
            b"<kanjidic2></kanjidic2>",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- KANJIDIC2 -->\n\
              <!-- 2026-09-11 -->\n<!DOCTYPE kanjidic2 [\n]>\n<kanjidic2></kanjidic2>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-kanjidic-opening-{i}"));
            let path = dir.join(KANJIDIC.file_name);
            std::fs::write(&path, head).expect("write");
            assert_eq!(read_all(open_local(&KANJIDIC, &path).expect("open")), *head);
        }
    }

    /// The JMdict side of the same table, including the published file's own
    /// shape: a declaration, a run of revision comments, then `<!DOCTYPE
    /// JMdict [` opening an internal subset. The declaration-then-root and
    /// bare-root shapes are already covered by the two pass-through tests
    /// above.
    #[test]
    fn every_legitimate_jmdict_opening_is_accepted() {
        for (i, head) in [
            &b"<!DOCTYPE JMdict [\n<!ENTITY n \"noun\">\n]>\n<JMdict></JMdict>"[..],
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Rev 1.09\n\tAdded the \
              g_type attribute\n-->\n<!-- Rev 1.08 -->\n<!DOCTYPE JMdict [\n]>\n<JMdict></JMdict>",
            b"<JMdict/>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-jmdict-opening-{i}"));
            let path = dir.join(SOURCE_FILE);
            std::fs::write(&path, head).expect("write");
            assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), *head);
        }
    }
```

The rewritten doc comment on `a_trimmed_jmdict_file_under_the_kanjidic_name_is_rejected` (the test body does not change):

```rust
    /// The check is per source: a hand-trimmed JMdict copy — one that starts
    /// at its root element — saved under the KANJIDIC2 name would parse as a
    /// KANJIDIC2 with zero characters and build an index with no kanji
    /// block, silently. It is refused at the only boundary where it is still
    /// recognisable as the wrong file. A declaration-led copy was once
    /// indistinguishable here, because `<?xml` was an accept for either
    /// source; since Phase 3c the name behind the declaration decides — see
    /// the next test.
```

The new tests:

```rust
    /// The case the Phase 3b handoff §5 recorded as open: a JMdict file with
    /// its declaration intact, saved under the KANJIDIC2 name.
    #[test]
    fn a_declared_jmdict_file_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-declared-jmdict");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, XML).expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
        assert!(!msg.contains("JMdict_e.gz"), "named the wrong file: {msg}");
    }

    /// The gzip path is sniffed too. A real `JMdict_e.gz` renamed to
    /// `kanjidic2.xml.gz` used to pass unexamined — the old branch returned
    /// the decoder without looking at what it decoded — and the message
    /// quotes the decompressed head, which is what the user would recognise.
    #[test]
    fn a_gzipped_jmdict_archive_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-gzipped-jmdict");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, gz(XML)).expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("<?xml"), "must quote the decompressed head: {msg}");
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
    }

    /// And the converse, so neither source is the lenient one.
    #[test]
    fn a_gzipped_kanjidic_archive_under_the_jmdict_name_is_rejected() {
        let dir = scratch("open-jmdict-gzipped-kanjidic");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(KANJIDIC_XML)).expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// A served error page that happens to arrive gzipped.
    #[test]
    fn a_gzipped_html_page_is_rejected() {
        let dir = scratch("open-gzipped-html");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(b"<!DOCTYPE html><html><body>404</body></html>"))
            .expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// The root's name must be the whole name: `JMdictionary` is not
    /// `JMdict`, and neither is a DOCTYPE for one. The delimiter after the
    /// name is what separates these from `<JMdict>` and `<!DOCTYPE JMdict [`.
    #[test]
    fn a_root_that_merely_starts_with_the_name_is_rejected() {
        for (i, head) in [
            &b"<JMdictionary></JMdictionary>"[..],
            b"<!DOCTYPE JMdictX><JMdictX/>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-jmdict-near-miss-{i}"));
            let path = dir.join(SOURCE_FILE);
            std::fs::write(&path, head).expect("write");
            let err = open_local(&JMDICT, &path).err().expect("must fail");
            assert!(
                matches!(err, SourceError::NotAnArchive { .. }),
                "got {err:?}"
            );
        }
    }

    /// A preamble longer than the peek is undecidable, and undecidable is an
    /// accept: the file opens and reads back whole. No EDRDG file is shaped
    /// like this; the point is that a stricter check must not start refusing
    /// a file the old one opened.
    #[test]
    fn a_preamble_that_outruns_the_peek_is_accepted() {
        let dir = scratch("open-long-preamble");
        let path = dir.join(SOURCE_FILE);
        let mut bytes = b"<?xml version=\"1.0\"?>\n<!-- ".to_vec();
        bytes.resize(bytes.len() + PEEK_BYTES, b'x');
        bytes.extend_from_slice(b" -->\n<JMdict></JMdict>");
        std::fs::write(&path, &bytes).expect("write");
        assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), bytes);
    }

    /// The same shape cut short by the end of the file is not undecidable:
    /// everything the file holds was read, and none of it is a root element.
    #[test]
    fn a_short_file_that_ends_inside_its_preamble_is_rejected() {
        let dir = scratch("open-short-preamble");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<?xml version=\"1.0\"?>\n<!-- never closed").expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// A file that is a declaration and nothing else is not a dictionary
    /// either, and the old rule would have opened it.
    #[test]
    fn a_bare_declaration_is_rejected() {
        let dir = scratch("open-bare-declaration");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<?xml version=\"1.0\"?>\n").expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// The peek must not lose a byte: a file longer than the peek reads back
    /// whole through the chained reader, plain and gzipped, with the DOCTYPE
    /// where the real archives put it — past a kilobyte of comments.
    #[test]
    fn a_file_longer_than_the_peek_reads_back_whole() {
        let mut bytes = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- ".to_vec();
        bytes.resize(bytes.len() + 1_200, b'r');
        bytes.extend_from_slice(b" -->\n<!DOCTYPE JMdict [\n]>\n<JMdict>");
        bytes.resize(bytes.len() + 3 * PEEK_BYTES, b'.');
        bytes.extend_from_slice(b"</JMdict>\n");

        let dir = scratch("open-longer-than-peek");
        let plain = dir.join(SOURCE_FILE);
        std::fs::write(&plain, &bytes).expect("write plain");
        assert_eq!(
            read_all(open_local(&JMDICT, &plain).expect("open plain")),
            bytes
        );

        let zipped = dir.join("JMdict_e.zipped.gz");
        std::fs::write(&zipped, gz(&bytes)).expect("write gz");
        assert_eq!(
            read_all(open_local(&JMDICT, &zipped).expect("open gz")),
            bytes
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p jmdict-source --lib 2>&1 | grep -E "^test |PEEK_BYTES|error"
```

Expected: a compile error — `PEEK_BYTES` is not defined yet. That is the failing state for this step; the eleven new tests cannot run until Step 3 lands. (Do not stub the constant to get red tests: the old rule would then pass six of them by accident and fail five, which teaches nothing.)

- [ ] **Step 3: Rewrite the sniff**

In `crates/jmdict-source/src/lib.rs`:

**Imports** (line 15) become:

```rust
use std::io::{BufRead, Cursor, Read};
```

**The `Source` doc and struct** (lines 55–82): the doc's first paragraph ends "…and how large a download may be." — change "how an ungzipped copy of it begins" to "what its root element is called", and in the second paragraph change "size cap and prolog discipline" to "size cap and sniff discipline". Replace the `prologs` field and its doc with:

```rust
    /// The XML root element's name — `JMdict` or `kanjidic2` — which is also
    /// the name a `<!DOCTYPE` carries. [`begins_xml`] looks for either behind
    /// an optional declaration and any comments, so the other source's file
    /// under this name, or a saved error page, is refused rather than parsed
    /// as an empty dictionary (Phase 3c design §4).
    pub root: &'static str,
```

**The constants** (lines 84–103): delete `JMDICT_PROLOGS` and `KANJIDIC_PROLOGS`; in `JMDICT` replace `prologs: JMDICT_PROLOGS,` with `root: "JMdict",` and in `KANJIDIC` replace `prologs: KANJIDIC_PROLOGS,` with `root: "kanjidic2",`. Immediately after `UTF8_BOM` (line 53) add:

```rust
/// How many bytes [`open_local`] reads ahead to decide what a file is. The
/// real archives reach `<!DOCTYPE` at byte 1,135 (JMdict) and 39
/// (KANJIDIC2), so the decision is made well inside this; [`begins_xml`]
/// says what happens when a file's preamble outruns it.
const PEEK_BYTES: usize = 8 * 1024;
```

**`begins_xml`** (lines 109–114) becomes, with a helper beneath it:

```rust
/// Whether `head` — a file's first bytes — begins `source`'s XML.
///
/// Root-aware (Phase 3c design §4): after an optional BOM, whitespace, one
/// `<?xml … ?>` declaration and any number of comments, the next markup
/// must be `<!DOCTYPE {root}` or `<{root}`, followed by whitespace, `>`, `[`
/// or `/`. Before this the check accepted `<?xml` on its own, so a JMdict
/// file saved under the KANJIDIC2 name with its declaration intact built a
/// kanji-less index and reported success.
///
/// `more_follows` says whether `head` was cut short by [`PEEK_BYTES`] rather
/// than by the end of the file. When it was, a preamble that outruns the
/// peek — or a token the peek cuts in half — is an accept: the check exists
/// to catch the wrong file, not to refuse a right one for the length of its
/// leading comment, and accepting there is what the old rule did. A file the
/// peek read to its end is decided on what it holds.
fn begins_xml(source: &Source, head: &[u8], more_follows: bool) -> bool {
    let mut rest = head
        .strip_prefix(&UTF8_BOM)
        .unwrap_or(head)
        .trim_ascii_start();
    if let Some(after) = rest.strip_prefix(b"<?xml") {
        match find(after, b"?>") {
            Some(end) => rest = after[end + 2..].trim_ascii_start(),
            None => return more_follows,
        }
    }
    while let Some(after) = rest.strip_prefix(b"<!--") {
        match find(after, b"-->") {
            Some(end) => rest = after[end + 3..].trim_ascii_start(),
            None => return more_follows,
        }
    }
    let doctype = format!("<!DOCTYPE {}", source.root);
    let element = format!("<{}", source.root);
    [doctype.as_bytes(), element.as_bytes()]
        .into_iter()
        .any(|token| match rest.strip_prefix(token) {
            Some(after) => match after.first() {
                Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'[' | b'/') => true,
                Some(_) => false,
                None => more_follows,
            },
            // A token the peek cut in half is undecidable; a short file that
            // simply stops, or holds something else, is not.
            None => more_follows && token.starts_with(rest),
        })
}

/// Byte offset of the first `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
```

**`NotAnArchive`** (lines 139–160): replace the variant's doc comment, `#[error]` text and `head` field doc with:

```rust
    /// The file at the resolved name does not hold that source's XML — JMdict
    /// for [`JMDICT`], KANJIDIC2 for [`KANJIDIC`] — either plain or inside a
    /// gzip archive, per the `file` this variant carries. Usually a
    /// hand-placed file; a download reaches it only if the server sent a
    /// valid gzip of the wrong content, since [`fetch`] verifies the gzip
    /// framing before renaming a download into place. Separate from
    /// [`SourceError::Corrupt`], which says "the server sent us something
    /// broken, retrying may help" — here nothing will change until the file
    /// itself does, so the message says so.
    #[error(
        "{path} does not hold {file}'s XML, gzipped or plain — it starts with \"{head}\". \
         Delete it, or replace it with a real {file}",
        path = path.display(),
    )]
    NotAnArchive {
        path: PathBuf,
        /// The first bytes — decompressed, when the file was gzip — escaped
        /// and truncated. A user who saved the wrong file usually recognises
        /// it from its opening; an unquoted length or byte count would not
        /// tell them which file they grabbed.
        head: String,
        file: &'static str,
    },
```

**`open_local`** (lines 172–225): replace the function and its doc comment with the three functions below. The first two doc paragraphs of the old comment (about the extension being a location, and about a hand-decompressed copy) are kept verbatim; only the third paragraph and the body change.

```rust
/// Open `path`, decompressing it when its bytes say it is gzipped.
///
/// The file name is not consulted. EDRDG publishes `JMdict_e.gz`, but a user who
/// decompressed it by hand and kept the name is a case worth supporting, and
/// exactly one resolved name — rather than one per encoding — avoids needing a
/// precedence rule between two copies that disagree.
///
/// What comes out of the file — or out of the gzip decoder — is then checked
/// against the source's root name by [`begins_xml`], and anything else is
/// rejected rather than passed through as text: the XML parser downstream
/// reads what it cannot recognise as zero entries, so a wrong file used to
/// publish an empty index and report success. This is the only boundary where
/// those bytes are still identifiable as a wrong *file* rather than an empty
/// dictionary. Both branches peek the same fixed number of bytes and hand the
/// reader back with those bytes still in front of it.
pub fn open_local(source: &Source, path: &Path) -> Result<Box<dyn BufRead>, SourceError> {
    let (head, file) = peek(std::fs::File::open(path)?)?;
    if head.starts_with(&GZIP_MAGIC) {
        // The decoder reads the peeked bytes first, then the rest of the
        // file; its output is sniffed exactly as a plain file's is, so a
        // gzipped wrong file is refused here rather than parsed as an empty
        // dictionary. A stream that will not decode fails this read as `Io`.
        let decoder = flate2::read::GzDecoder::new(Cursor::new(head).chain(file));
        let (head, decoder) = peek(decoder)?;
        return sniffed(source, path, head, decoder);
    }
    sniffed(source, path, head, file)
}

/// Read up to [`PEEK_BYTES`] from `reader` and hand both back. Reads until
/// the buffer is full or the reader is done, rather than trusting one
/// `read` call: a gzip decoder returns what one block yields, which can be
/// less than asked for with more still to come, and the decision in
/// [`begins_xml`] depends on knowing which of the two stopped the read.
fn peek<R: Read>(mut reader: R) -> std::io::Result<(Vec<u8>, R)> {
    let mut head = vec![0u8; PEEK_BYTES];
    let mut filled = 0;
    while filled < PEEK_BYTES {
        match reader.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    head.truncate(filled);
    Ok((head, reader))
}

/// Accept `head` as `source`'s XML and return a reader over it followed by
/// `rest`, or refuse with the head quoted.
fn sniffed<R: Read + 'static>(
    source: &Source,
    path: &Path,
    head: Vec<u8>,
    rest: R,
) -> Result<Box<dyn BufRead>, SourceError> {
    if begins_xml(source, &head, head.len() == PEEK_BYTES) {
        return Ok(Box::new(std::io::BufReader::new(
            Cursor::new(head).chain(rest),
        )));
    }
    Err(SourceError::NotAnArchive {
        path: path.to_path_buf(),
        // Enough to recognise a saved web page or a text file by, short
        // enough to sit inside a one-line message.
        head: String::from_utf8_lossy(&head[..head.len().min(24)])
            .escape_debug()
            .to_string(),
        file: source.file_name,
    })
}
```

- [ ] **Step 4: Run the crate's tests and verify they pass**

```bash
cargo fmt -p jmdict-source
cargo test -p jmdict-source 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy -p jmdict-source --all-targets -- -D warnings
```

Expected: every `test result` line `ok`; the lib binary's count is the old count plus ten (one test replaced by two, nine added); clippy silent. If `a_file_longer_than_the_peek_reads_back_whole` fails on the gzip half with a short read-back, `peek`'s loop is not being used on the decoder — check that the decoder branch calls `peek(decoder)` and not `fill_buf`. If `a_short_file_that_ends_inside_its_preamble_is_rejected` fails, `more_follows` is being computed from something other than `head.len() == PEEK_BYTES`.

- [ ] **Step 5: The standing gates for a Rust change**

```bash
cargo test --workspace 2>&1 | grep -E "^test result"
cargo test -p jparser --features mecab 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p jparser --no-default-features --all-targets
cargo fmt --all --check
cargo +1.85 check -p jparser -p jmdict-source
```

Expected: workspace total is Task 1 Step 1's figure plus ten; the rest silent. `src-tauri` tests pass unchanged — nothing there asserts on the old message text (verified by grep: `neither a gzip` occurs only in `lib.rs`).

- [ ] **Step 6: Commit**

```bash
git add crates/jmdict-source/src/lib.rs
git commit -m "fix(jmdict-source): sniff the root name, gzipped or plain

open_local used to accept any file that began with <?xml for either
source, and returned a gzip decoder without looking at what it decoded,
so a JMdict file saved under the KANJIDIC2 name built a kanji-less index
and reported success. Each Source now names its root element; the sniff
skips a declaration and any comments and requires <!DOCTYPE root or
<root behind them, on the decompressed bytes when the file is gzip. A
head cut short by the 8 KiB peek still accepts, so no file the old rule
opened is refused for the length of its preamble; a head cut short by
the end of the file is decided on what it holds."
```

---

### Task 3: The dictionary and hints commands leave `commands.rs`

**Files:**
- Create: `src-tauri/src/commands/dictionary.rs`
- Create: `src-tauri/src/commands/hints.rs`
- Modify: `src-tauri/src/commands.rs` — module doc (lines 3–8), imports (lines 10–22), the cluster (lines 1281–1694), and the test module's moved tests (lines 1721–1817, 2218–2261, 2262–2718)
- Modify: `src-tauri/src/main.rs` — `app.manage(...)` at lines 163, 171, 175, 180, 181; `generate_handler!` entries at lines 265–270
- Modify: `src-tauri/src/parse.rs` — the doc comment at line 90

**Interfaces:**
- Consumes: everything named below exactly as it exists in `commands.rs` at `b287236`; `crate::test_support::scratch(name) -> PathBuf` and `crate::test_support::build_index_generation(&root)`; the private `fn push_input` in `commands.rs`, which a descendant module may call.
- Produces: `commands::dictionary::{IndexSender, DictionaryPaths, DownloadInFlight, needs_dictionary, kanji_info, download_dictionary}` and `commands::dictionary::{claim_download, release_download, reparse}` as `pub(super)`; `commands::hints::{HintsSender, HintsInFlight, set_mecab_hints, hints_warning, hints_dir}`. Every signature is unchanged.

- [ ] **Step 1: Capture the test listing before the move**

```bash
export S="$LOCALAPPDATA/Temp/claude/clipyomi-phase3c"
mkdir -p "$S"
cargo test -p clipyomi -- --list 2>/dev/null | grep ': test$' | sed 's/^.*:://; s/: test$//' | sort > "$S/tests_before.txt"
wc -l < "$S/tests_before.txt"
```

Expected: the `src-tauri` count from Task 1 Step 1 (152 at the Phase 3b handoff). This file is the assertion for Step 7; there is no new test in this task.

- [ ] **Step 2: Create `commands/dictionary.rs`**

Create `src-tauri/src/commands/dictionary.rs` with this head, then the moved items **in this order**, each cut from `commands.rs` verbatim with its doc comment:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dictionary commands: whether startup found an index, the download
//! that builds one and publishes it to the parse worker, and the kanji
//! lookup the popover asks for on hover. Moved out of `commands.rs` in
//! Phase 3c with no change to the code; the download slot and `reparse`
//! live here and are shared with `hints`, which is why they are
//! `pub(super)`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

use crate::state::{AppState, NeedsDictionary};
```

1. `pub const DICTIONARY_STATUS_EVENT` (lines 1281–1284)
2. `pub struct IndexSender` (lines 1359–1361)
3. `pub struct DictionaryPaths` (lines 1371–1380)
4. `pub struct DownloadInFlight` (lines 1382–1385)
5. `fn claim_download` (lines 1387–1397) — visibility becomes `pub(super) fn claim_download`
6. `fn release_download` (lines 1399–1403) — becomes `pub(super) fn release_download`
7. `fn reparse` (lines 1330–1357) — becomes `pub(super) fn reparse`
8. `pub fn needs_dictionary` with its `#[tauri::command]` (lines 1405–1410)
9. `pub(crate) fn lookup_kanji` (lines 1412–1424)
10. `pub fn kanji_info` with its attribute (lines 1426–1440)
11. `fn emit_status` (lines 1442–1449)
12. `fn describe_index_error` (lines 1451–1478)
13. `fn with_fallback` (lines 1480–1500)
14. `pub async fn download_dictionary` with its attribute (lines 1502–1587)

Then the test module. Its head:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;
```

followed by these tests moved verbatim from `commands.rs`'s test module, in this order: `a_second_download_is_rejected_while_one_is_running`, `releasing_the_claim_allows_a_retry`, `a_wrapped_source_error_replaces_the_generic_io_prefix`, `a_genuine_index_io_error_keeps_its_own_message`, `the_fallback_is_not_repeated_when_the_reason_already_names_the_directory`, `the_fallback_is_added_when_the_reason_does_not_name_the_directory` (lines 1721–1817, each with its doc comment); `reparse_skips_an_empty_channel`, `reparse_resends_the_current_text` (lines 2218–2249); `lookup_kanji_returns_hits_in_order_and_omits_misses`, `kanji_info_is_empty_before_a_dictionary_is_loaded`, `kanji_info_answers_from_the_published_index` (lines 2669–2718). One edit inside a moved test: in `reparse_resends_the_current_text`, `push_input(&tx, "東京都".to_string())` becomes `crate::commands::push_input(&tx, "東京都".to_string())` — the function stays private in `commands.rs`, and a descendant module reaches it by path.

- [ ] **Step 3: Create `commands/hints.rs`**

Create `src-tauri/src/commands/hints.rs` with this head, then the moved items in this order:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The MeCab boundary-hints commands: the toggle that acquires the Vibrato
//! dictionary and publishes a tokenizer to the parse worker, the warning
//! shown when hints are off against the setting, and the directory a
//! hand-placed archive goes in. Moved out of `commands.rs` in Phase 3c with
//! no change to the code. The download slot and `reparse` are `dictionary`'s.

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use jparser::hints::VibratoTokenizer;
use tauri::{Emitter, State};
use tokio::sync::watch;

use super::dictionary::{claim_download, release_download, reparse, DictionaryPaths};
use super::InputSender;
use crate::state::{HintsWarning, SettingsState};
```

1. `pub const HINTS_STATUS_EVENT` (lines 1286–1291)
2. `pub struct HintsInFlight` (lines 1293–1297)
3. `pub struct HintsSender` (lines 1363–1369)
4. `fn emit_hints_status` (lines 1299–1306)
5. `fn describe_hints_error` (lines 1308–1328)
6. `pub async fn set_mecab_hints` with its attribute (lines 1589–1672)
7. `pub fn hints_warning` with its attribute (lines 1674–1685)
8. `pub fn hints_dir` with its attribute (lines 1687–1694)

Then the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;
    use tauri::{Listener, Manager};
```

followed, verbatim and in order, by `a_second_hints_acquisition_is_rejected_while_one_is_running` (lines 2251–2261), `a_hints_load_failure_names_the_format_it_expects_and_the_way_out`, `hints_warning_reports_none_for_the_empty_sentinel`, `hints_warning_reports_a_non_empty_message`, the `// --- Phase 5B handoff §2.3 items 3 and 5, automated ---` comment block, `hints_scratch`, `struct HintsHarness`, `hints_harness`, `capture_hints_status`, `staging_leftovers`, `a_hand_placed_archive_replaces_the_published_dictionary`, `an_unloadable_dictionary_names_the_way_out_and_persists_nothing`, `the_second_toggle_is_offline_and_instant` (lines 2262–2667). One edit inside a moved helper: `hints_scratch` called `commands.rs`'s test-module `scratch`, which does not move and is not visible from here, so its body becomes:

```rust
    /// A scratch hints directory, removed and recreated per test, under
    /// `test_support`'s prefix now that these tests have a module of their own.
    fn hints_scratch(name: &str) -> std::path::PathBuf {
        crate::test_support::scratch(&format!("hints-{name}"))
    }
```

(replacing its doc comment as well as its body — the old comment explained a naming choice relative to the dictionary tests in the same module, which no longer applies).

- [ ] **Step 4: Edit `commands.rs`**

Delete everything moved in Steps 2 and 3 from `commands.rs` — lines 1281–1694 in full, and the moved tests and helpers from the test module — leaving the `#[cfg(test)] mod tests {` module with the tests that did not move (`set_input_*`, `initial_height_*`, `peek_does_not_grow_*`, `save_window_geometry_*`, `save_settings_*`, `window`, `chrome_target_*`, `set_chrome_mode_*`, `create_settings_window_*`, `open_settings_window_*`, `a_colorref_*`, `platform_commands_*`, `geometry_round_trip_*`, `window_controls_*`, `a_save_from_the_settings_window_*`) and the `scratch` helper they use.

Replace the module doc (lines 3–8) with:

```rust
//! The webview's entry points: push input, toggle settings, read settings, ask
//! why startup failed or why settings fell back to defaults, and signal that
//! the event listeners are registered. The dictionary download, the kanji
//! lookup and the hints toggle live in [`dictionary`] and [`hints`]. Parsing
//! itself runs in `parse::run_worker`; results arrive as `parse-result` /
//! `parse-error` events, not as a command return value.
```

After the `use crate::state::{…};` block add:

```rust
pub mod dictionary;
pub mod hints;
```

Then prune the imports. Expected to become unused and to be removed: `Emitter` from the `tauri` line; `use std::path::{Path, PathBuf};` entirely; `use jparser::hints::VibratoTokenizer;`; `AppState`, `HintsWarning` and `NeedsDictionary` from the `crate::state` line. Expected to stay: `AtomicBool` and `Ordering` (the `PEEKED` static and `is_peeked`), `Arc`, `Notify`, `watch`, `AppHandle`, `Manager`, `State`, `WebviewUrl`, `WebviewWindowBuilder`, `ChromeMode`, `Settings`, `SettingsState`, `SettingsWarning`, `ShortcutWarning`, `StartupFailure`. Let the compiler arbitrate: `cargo build -p clipyomi` and `cargo test -p clipyomi --no-run` report each unused import by name, and clippy under `-D warnings` will refuse any that remain.

- [ ] **Step 5: Edit `main.rs` and `parse.rs`**

In `src-tauri/src/main.rs`, the five `manage` calls become:

```rust
            app.manage(commands::dictionary::IndexSender(index_tx.clone()));
```
```rust
            app.manage(commands::hints::HintsSender(hints_tx.clone()));
```
```rust
            app.manage(commands::dictionary::DictionaryPaths {
                root: root.clone(),
                source_dir,
                hints_dir: hints_dir.clone(),
            });
            app.manage(commands::dictionary::DownloadInFlight(Arc::new(AtomicBool::new(false))));
            app.manage(commands::hints::HintsInFlight(Arc::new(AtomicBool::new(false))));
```

and the six handler entries become:

```rust
            commands::dictionary::download_dictionary,
            commands::dictionary::needs_dictionary,
            commands::hints::set_mecab_hints,
            commands::hints::hints_warning,
            commands::hints::hints_dir,
            commands::dictionary::kanji_info,
```

(`cargo fmt` may wrap the `DownloadInFlight` line; let it.) In `src-tauri/src/parse.rs` line 90, `commands::download_dictionary` becomes `commands::dictionary::download_dictionary`.

- [ ] **Step 6: Build, format, and run the crate's tests**

```bash
cargo fmt --all
cargo build -p clipyomi 2>&1 | grep -E "^(warning|error)" ; echo "build exit: ${PIPESTATUS[0]}"
cargo test -p clipyomi 2>&1 | grep -E "^test result|FAILED|panicked"
cargo clippy -p clipyomi --all-targets -- -D warnings
```

Expected: no warnings, `test result: ok` with the same total as Step 1, clippy silent. A `private function` or `not found in this scope` error names an item that was moved without its `pub(super)` or a `use` that was missed — the lists in Steps 2 and 3 are complete, so compare against them rather than widening visibility to `pub`.

- [ ] **Step 7: Prove it was a move**

```bash
cargo test -p clipyomi -- --list 2>/dev/null | grep ': test$' | sed 's/^.*:://; s/: test$//' | sort > "$S/tests_after.txt"
diff "$S/tests_before.txt" "$S/tests_after.txt" && echo identical
git diff --stat HEAD -- src-tauri/src
wc -l src-tauri/src/commands.rs src-tauri/src/commands/dictionary.rs src-tauri/src/commands/hints.rs
```

Expected: `identical`; the stat shows `commands.rs` losing roughly what the two new files gain, the difference being the two module docs, two `use` blocks and the `pub mod` lines; `commands.rs` near 1,700 lines, `dictionary.rs` near 420, `hints.rs` near 570. Record the three counts and the stat for the handoff.

- [ ] **Step 8: The standing gates**

```bash
cargo test --workspace 2>&1 | grep -E "^test result"
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Expected: workspace total unchanged from Task 2 Step 5; the rest silent.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/commands/dictionary.rs src-tauri/src/commands/hints.rs src-tauri/src/main.rs src-tauri/src/parse.rs
git commit -m "refactor(src-tauri): the dictionary and hints commands leave commands.rs

commands.rs was 2,719 lines. The index download, the kanji lookup, the
hints acquisition, their six state structs and their tests move verbatim
into commands/dictionary.rs and commands/hints.rs, following the
settings/ and layout/ precedent; the download slot and reparse stay with
the index side and hints imports them. main.rs names the new paths,
because generate_handler! needs the command's own module. The test
listing is identical before and after but for the module prefix."
```

---

### Task 4: Gates, the real files, and the record

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` — a `### Phase 3c` entry after the `### Phase 3b` entry's last paragraph (line 1200), before `### Phase 3 — Extra linguistic layers`
- Create: `docs/superpowers/2026-09-12-engine-v2-phase3c-handoff.md`

- [ ] **Step 1: The engine gates**

Run the block under "Measuring the gates" above. Compare against Global Constraints cell by cell: both `score` runs (every cell, and the three compound classes with hints on), both `bench` scaling cells read as the band. Expected: every cell equal to the required one. Any difference in any cell: stop, do not adjust the record, find the edit that caused it — this plan edits nothing under `engine/` or `index/`, so a moved cell means a file was touched that should not have been.

- [ ] **Step 2: The snapshot and the suites**

```bash
cargo test -p jparser --test engine_v2
git status --short crates/jparser/tests/snapshots
```

Expected: pass; the second command prints nothing. Then every standing gate from Global Constraints, plus:

```bash
cargo +1.85 check -p jparser -p jmdict-source
cargo llvm-cov clean --workspace
cargo llvm-cov -p jmdict-source
cargo llvm-cov -p jparser --features mecab
npx playwright test
```

Read the per-file rows, not the crate total: `jmdict-source/src/lib.rs` must be at or above 80% lines, and `kanjidic.rs`, `index/build.rs`, `index/load.rs` must still be where the Phase 3b handoff §2 left them (96.34%, 97.92%, 86.62%) — untouched files with a changed reading mean a stale profile. If the run prints `warning: … functions have mismatched data`, the table is invalid — clean and re-run. Playwright: expected 45 passed; no first-run copy and no pictured screen changed, so no baseline moves. Record every total beside Task 1 Step 1's "before" figures.

- [ ] **Step 3: The sniff on the real files, headlessly**

`%APPDATA%\dev.jparser.clipyomi` itself is never written to. Copy the real source directory and build from it:

```bash
mkdir -p "$S/real/source" "$S/real/dict"
cp "$APPDATA/dev.jparser.clipyomi/source/JMdict_e.gz" "$APPDATA/dev.jparser.clipyomi/source/kanjidic2.xml.gz" "$S/real/source/"
time ./target/release/jparser-cli ensure-dictionary "$S/real/dict" --source-dir "$S/real/source"
```

Expected: `generation: gen-1`, `entries: 218449`, `kanji: 13108`, no download, a few seconds — both real archives pass the root-aware sniff through the gzip branch. Then the wrong file:

```bash
mkdir -p "$S/wrong/source" "$S/wrong/dict"
cp "$APPDATA/dev.jparser.clipyomi/source/JMdict_e.gz" "$S/wrong/source/JMdict_e.gz"
cp "$APPDATA/dev.jparser.clipyomi/source/JMdict_e.gz" "$S/wrong/source/kanjidic2.xml.gz"
./target/release/jparser-cli ensure-dictionary "$S/wrong/dict" --source-dir "$S/wrong/source"; echo "exit: $?"
ls "$S/wrong/dict"
```

Expected: a non-zero exit whose `Error:` line is the **Debug** form of the error — `jparser-cli`'s `main` returns `Result<(), Box<dyn Error>>`, so the runtime prints `{:?}` — reading `Io(Custom { … NotAnArchive { path: "…/kanjidic2.xml.gz", head: "<?xml version=…", file: "kanjidic2.xml.gz" } … })`; and no generation directory under `$S/wrong/dict` — the refusal happens in the source closure, before anything is built, and nothing was downloaded because both names were present. The Display text the app shows on its download screen (`… does not hold kanjidic2.xml.gz's XML, gzipped or plain — it starts with "<?xml …"`) is pinned by Task 2's unit tests, not by this run; say so in the handoff rather than quoting a message the CLI did not print. Record both outputs verbatim. The GUI check — `npm run tauri dev`, hover 食, see `ぐ.い` at the end of its readings with no hyphen — needs a human at a GUI and is the user's, as it was for Phases 3a and 3b. The handoff says so rather than claiming it.

- [ ] **Step 4: The master plan**

After the `### Phase 3b — The KANJIDIC misc fields — **COMPLETE**` entry's last paragraph (line 1200) and before `### Phase 3 — Extra linguistic layers`, add:

```markdown
### Phase 3c — The KANJIDIC layer close-out — **COMPLETE**

The three loose ends Phases 3a and 3b left, closed together and nothing
else. Kun readings drop KANJIDIC2's affix hyphen at render time and keep
the okurigana dot (`-ぐ.い` renders `ぐ.い`, `た.べる` is unchanged); the
record and the wire keep the notation. The source sniff is root-aware on
both the plain and the gzip path: each `Source` names its root element, and
a JMdict file under the KANJIDIC2 name — declaration intact, or gzipped — is
refused instead of building a kanji-less index. The dictionary commands left
`src-tauri/src/commands.rs` for `commands/dictionary.rs` and
`commands/hints.rs` by a pure move. No engine, index format, wire, settings
or CSS change, so no install rebuilds and §5.2's baseline block is unchanged
to the cell. Design: `specs/2026-09-12-engine-v2-phase3c-design.md`; record:
`2026-09-12-engine-v2-phase3c-handoff.md`. The Phase 3 table is not changed
by this phase; the two research items it leaves — the long-unit token
inventory and a pitch-accent source hunt — are spikes, not phases.
```

- [ ] **Step 5: The handoff**

Create `docs/superpowers/2026-09-12-engine-v2-phase3c-handoff.md` in the shape of `2026-09-11-engine-v2-phase3b-handoff.md`, prose throughout:

- **§1 What landed** — the three commits of Tasks 1–3 by hash and one paragraph each; the file map above collapsed to prose; the insertions/deletions count from `git diff --stat b287236..HEAD`; the three line counts from Task 3 Step 7. State plainly that `INDEX_FORMAT_VERSION` is still 6 and no install rebuilds.
- **§2 The gates** — the measured `score` blocks for both hint states pasted verbatim, the cell-by-cell table against the required values, both `bench` blocks with the band read stated for both states, the snapshot check, the suite totals before (Task 1 Step 1) and after in one table with the delta per suite explained (npm +4, workspace +10, `src-tauri` ±0), the per-file coverage rows, Playwright's total, the test-listing diff of Task 3 Step 7.
- **§3 Where the implementation departs from the design** — expected to be empty or near it; if a departure was ruled during execution, name it and its reason as the 3b handoff does. Two things to check honestly: whether the split really was a move (the listing diff, and a `git diff --color-moved=dimmed-zebra` read of the split commit showing no changed line inside a moved block beyond the three `pub(super)` visibilities, the `crate::commands::push_input` path and `hints_scratch`), and whether any import the plan expected to prune stayed or any it expected to keep went.
- **§4 The sniff on the real files** — Step 3's outputs: the `ensure-dictionary` build over the copied real archives with its `kanji` line and time, and the refusal message verbatim with the absent generation directory. State plainly that the GUI check is the user's.
- **§5 Open items** — carried from design §7 in substance: the former JLPT scale, frequency as a rank, no aside toggle, the README (and its "pitch/romaji support" line), the `-darwin` baselines, the bench cell as a ratio; `commands.rs` still about 1,700 lines with `win32_frameless` in it; the sniff's accept when a preamble outruns the peek; the two spikes named as next with the facts the design gives for each. Plus anything this phase surfaced.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md docs/superpowers/2026-09-12-engine-v2-phase3c-handoff.md
git commit -m "docs(engine-v2): Phase 3c handoff, the KANJIDIC layer close-out

Records the gates as measured: the baseline cells and the bench band
unchanged in both hint states, the snapshot byte-identical, the suites and
per-file coverage, the test listing identical across the commands.rs split,
and the root-aware sniff building from the real archives while refusing a
JMdict archive under the KANJIDIC2 name. Marks the phase complete in the
master plan; the Phase 3 table does not change."
```

The branch stays `feat/engine-v2`; nothing here merges.

---

## Self-review

**Spec coverage.** §1 items 1–3: Tasks 1, 2, 3. §1's exclusions (README, spikes, darwin, `win32_frameless`, engine/format/wire/settings/CSS/CLI) are touched by no task; Task 4 §5 carries them. §1.1 every row: kun choice and edge-trim (Task 1 Step 4); render-only (Task 1 touches no Rust); DOCTYPE-first rule, undecidable-accepts, gzip sniffed, `root` field (Task 2 Step 3); two modules, shared items in `dictionary`, `main.rs` paths, `win32_frameless` stays (Task 3). §2 licence: nothing to do. §3: `kunReading`, the 食 example (Task 1 tests), the colouriser unchanged (Task 1 adds a test, edits no colour code), the Rust test untouched. §4.1 steps 1–5: `begins_xml`; the `[` delimiter and the fetch test body: covered by `every_legitimate_jmdict_opening_is_accepted` and the existing pass-through tests. §4.2: `peek` on the decoder, `Io` on a bad stream, the download path through `resolve_from` unchanged. §4.3: no fixture or `src-tauri` test touched by Task 2. §4.4 every listed test: present in Task 2 Step 1 (the "comment before DOCTYPE" case is inside both `every_legitimate_*` tests; `<!DOCTYPE JMdictX>` is in the near-miss test). §5.1 table: Task 3 Steps 2–3. §5.2: Step 5. §5.3 three checks: Steps 7, 8 and Task 4. §6 tests: Task 1 (four), Task 2 (ten net), Task 3 (none, by design); gates 1–6: Task 4 Steps 1–3. §7: Task 4 Step 5.

**Placeholder scan.** No "TBD", "similar to", or "add tests" remains; every code step carries its code; every command carries its expected output.

**Type consistency.** `kunReading(reading: string): string` is used only in `kanjiLine`. `begins_xml(source: &Source, head: &[u8], more_follows: bool) -> bool`, `find(haystack: &[u8], needle: &[u8]) -> Option<usize>`, `peek<R: Read>(reader: R) -> io::Result<(Vec<u8>, R)>`, `sniffed<R: Read + 'static>(source, path, head: Vec<u8>, rest: R) -> Result<Box<dyn BufRead>, SourceError>` are named identically in Step 3's code and Step 4's failure notes; `PEEK_BYTES` is `usize` where the tests use it in `Vec::resize` lengths and `3 * PEEK_BYTES`. Task 3's `pub(super)` trio is imported in `hints.rs` under the same three names, and `main.rs`'s paths match the module names `dictionary` and `hints` declared in `commands.rs`.
