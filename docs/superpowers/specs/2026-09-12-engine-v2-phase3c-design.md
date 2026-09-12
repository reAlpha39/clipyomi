# Engine v2 Phase 3c — the KANJIDIC layer close-out

**Date:** 2026-09-12
**Parent:** the Phase 3b handoff §5 (open items), which carries three items
from the Phase 3a design §7 and the whole-branch review that this phase
closes: the kun-reading marks, the prolog sniff, and the size of
`src-tauri/src/commands.rs`. Master plan §7's Phase 3 table is not changed
by this phase; no new layer ships and no licence question arises.
**Status:** design written; awaiting review; implementation plan to follow.

## 1. What it does, and what it deliberately does not

Phases 3a and 3b built the kanji block and left three loose ends behind
them, each raised and accepted rather than fixed at the time. This phase
ties them off and adds nothing else:

1. **Kun readings drop KANJIDIC2's affix hyphen and keep its okurigana
   dot.** `-ぐ.い` renders `ぐ.い`, `ひと-` renders `ひと`, `た.べる` is
   unchanged. Render-only; the record on disk and on the wire stays verbatim.
2. **The source sniff becomes root-aware, on both the plain and the gzip
   path.** A JMdict file saved under the KANJIDIC2 name with its `<?xml`
   declaration intact is refused instead of building a kanji-less index that
   reports success, and a gzipped file is now sniffed at all.
3. **The dictionary commands leave `commands.rs`.** Two new modules,
   `commands/dictionary.rs` and `commands/hints.rs`, take the index download,
   the kanji lookup, the Vibrato hints acquisition and their six state
   structs, with their tests. No behaviour changes.

Not in this phase, by decision: the README feature list (the user's file, and
carrying uncommitted edits throughout; it also describes "pitch/romaji
support" that no code in the tree provides, which is the user's to correct);
the two research questions the handoff names — the long-unit token inventory
of the frequency doc §12.5 and a pitch-accent source hunt — which are spikes
whose output is a recommendation, not code, and run separately after this
phase; the `-darwin` screenshot baselines, still owed a Mac; the
`win32_frameless` module, 730 lines of Windows chrome that stays in
`commands.rs` because it has nothing to do with this branch; and any change
to the engine, the index format, the wire, the settings, the CSS or the CLI.

Because nothing under `crates/jparser/src/engine/` or `crates/jparser/src/index/`
is edited, **`INDEX_FORMAT_VERSION` stays at 6 and existing installs pay no
rebuild this time** — the Phase 3b handoff §5 asked that a third format bump
in short order be batched, and this phase has none to batch.

### 1.1 Decisions taken in session

| decision | choice | why |
| --- | --- | --- |
| Which of the handoff's six candidates | the three code items | one coherent theme, gates identical by construction; README is blocked by the never-stage rule and the two spikes are research, not a build phase |
| Kun marks | strip `-`, keep `.` | the dot shows where the okurigana starts, which is how the reader writes the word; the hyphen marks affix-only use, a lexicographer's fact that reads as a stray hyphen beside `・` separators |
| Where the marks are handled | the popover renderer only | the 3a design §7 called it a popover-text question, not a data one; `kanji.bin`, the wire and the parser's own test stay verbatim |
| How `-` is removed | trimmed from both ends of each kun reading | in the 2026-09-11 file every one of the 392 hyphenated kun readings carries it at an edge (266 leading, 126 trailing, none interior), so edge-trimming is exact and a blanket replace would only be equivalent by accident |
| Sniff rule | DOCTYPE name first, root element as fallback | the real archives put `<!DOCTYPE` at byte 1,135 (JMdict) and 39 (KANJIDIC2), inside the reader's 8 KiB peek; their root elements sit at 23 KB and 13 KB, out of reach; a DTD's DOCTYPE name is its root name by definition |
| When the peek cannot decide | accept when the peek ended the head, reject when the file did | today's behaviour where it matters: a legitimate file with a very long leading comment must not start failing because a check that used to pass it grew stricter. A file the peek read to its end is the other case — nothing more is coming, and a file that simply stops holds no dictionary |
| Gzip path | sniffed too, on the decompressed head | today it is not sniffed at all, so a renamed archive passes unexamined; the download path opens the archive through the same function |
| `Source.prologs` | replaced by `root: &'static str` | three byte prefixes were the old rule's shape; the new rule needs one name; the field is read nowhere outside `lib.rs` |
| Module split | two modules, not one | one file would land near 990 lines with its tests, over the 800-line rule on the day it is created; two land near 420 and 570 |
| Where `reparse` and the download claim live | `dictionary.rs`, imported by `hints.rs` | both halves call them; the index side is the older owner |
| Handler registration | `main.rs` names the submodule paths | `tauri::generate_handler!` expands to the command's generated `__cmd__` macro beside it, so re-exporting the function alone does not compile; six handler lines and five `manage` lines change |
| `win32_frameless` | stays | outside the branch's theme; moving it would be for the file-size rule alone |

## 2. Licence, unchanged

Master plan §7: "No layer ships without its licence established in writing
first." This phase adds no data, no file from a publisher and no dependency.
The kun-reading change reads the same KANJIDIC2 field Phase 3a established
under EDRDG's CC BY-SA 4.0 statement and drops one character of its notation
at render time. Nothing in the attribution footer moves.

## 3. Kun readings: the hyphen goes, the dot stays

`kanjiLine` in `src/render/tooltip-text.ts` joins on readings and kun
readings with the katakana middle dot. It gains one step: each kun reading
is trimmed of ASCII hyphen-minus at both ends before the join. On readings
are not touched (the file writes no marks on them). The result, on the
fixture's 食:

```text
before  食  ショク・ジキ・く.う・く.らう・た.べる・は.む・-ぐ.い  eat, food  (grade 2, 9 strokes)
after   食  ショク・ジキ・く.う・く.らう・た.べる・は.む・ぐ.い  eat, food  (grade 2, 9 strokes)
```

What the dot does in the colouriser does not change: `tooltip-colour.ts`
treats `.` as a non-Japanese character, so `た.べる` is already painted as a
kana run, a text run and a kana run, and stays that way. A hyphen was
painted the same way, which is part of why it looked like a stray mark.

The record is not changed anywhere upstream. `KanjiData.kun` on disk, the
`kanji_info` payload and the parser all keep KANJIDIC2's notation, and the
parser test `keeps_okurigana_and_affix_marks_verbatim` keeps passing as
written. The one existing render test that names a kun reading,
`ひと.つ`, is unchanged by the rule. A `KanjiInfo` literal in the test
suites with a hyphenated kun reading does not exist today, so nothing
existing moves.

## 4. The sniff: root-aware, both paths

`crates/jmdict-source/src/lib.rs`'s `begins_xml(source, head)` decides
whether a file's first bytes are the source's XML. Today it accepts any of
three prefixes per source, and `<?xml` is one of them for both — so a JMdict
file with its declaration intact passes as a KANJIDIC2, and vice versa. And
`open_local`'s gzip branch returns the decoder without sniffing at all.

### 4.1 The rule

`Source` loses `prologs` and gains `root: &'static str` — `"JMdict"` and
`"kanjidic2"`. `begins_xml` becomes:

1. Strip a UTF-8 BOM and leading ASCII whitespace, as today.
2. Skip one `<?xml … ?>` declaration if present.
3. Skip any number of `<!-- … -->` comments, with any whitespace between
   and after them.
4. The next bytes must be `<!DOCTYPE {root}` or `<{root}`, followed by a
   delimiter: whitespace, `>`, `[` or `/`. That is the accept.
5. If the head runs out inside step 2, 3 or 4 before a decision is reached,
   **what ran out decides**. When the *peek* ended the head — the read
   filled all `PEEK_BYTES` and the file continues past them — **accept**:
   the check exists to catch the wrong file, not to fail a right one whose
   leading comment is longer than the reader's buffer, and accepting there
   is exactly as permissive as the old rule. When the *end of the file*
   ended the head, **reject**: everything the file holds was read and none
   of it is this source's root, so a file that simply stops holds no
   dictionary. A token the peek cut in half counts as the peek ending the
   head — a half-written `<!DOCTYPE JMdict`, and equally the `<!-` of a
   comment opener the loop in step 3 could not strip.

Anything else is the reject, with the same `SourceError::NotAnArchive
{ path, head, file }` as today and the same 24-byte escaped head in the
message.

The delimiter in step 4 is what keeps `<!DOCTYPE JMdict [` (the real
file's internal DTD subset opens with a bracket) accepted and
`<!DOCTYPE JMdictX` rejected. The fetch test's synthetic body,
`<?xml version="1.0"?><JMdict></JMdict>`, passes through steps 2 and 4.

### 4.2 The gzip half

`open_local`'s gzip branch wraps `GzDecoder` in a `BufReader`, peeks its
first buffer with `fill_buf`, applies the same `begins_xml`, and returns
that `BufReader` — the peeked bytes stay in it, exactly as the plain branch
already relies on. A wrong archive is refused with `NotAnArchive` carrying
the *decompressed* head, so the message quotes what the file contains rather
than gzip magic. A gzip stream that fails to decode at the first read
surfaces as `SourceError::Io` here instead of inside the parser, which is
earlier and no less clear.

The download path opens the archive it just verified through this same
function (`resolve_from` calls `open_local`), so a served body that gunzips
to something other than the expected XML is now caught at the same boundary
as a hand-placed file.

### 4.3 Who this touches

Only `jmdict-source`'s callers: the app's `download_dictionary` (through
`resolve` and `resolve_kanjidic`) and `jparser-cli ensure-dictionary`.
`jparser-cli build-index` opens its XML paths directly, every test fixture in
the tree is read directly by a parser, and no `src-tauri` test writes a
source archive — the "hand-placed archive" tests there concern the Vibrato
dictionary, not these files. So no fixture, snapshot or gate outside
`jmdict-source`'s own tests is reached by the change.

### 4.4 Tests

The three-prolog tests (`every_kanjidic_prolog_is_accepted` and the JMdict
counterparts) are rewritten to the new rule rather than kept alongside it:
`<?xml`-led with root, `<!DOCTYPE`-led, bare root, and `<?xml`-led with a
comment before `<!DOCTYPE` all accepted, for both sources. Added: a
`<?xml`-led JMdict under the KANJIDIC2 name is rejected, plain and gzipped
(the case the handoff records as open); a gzipped HTML page under either
name is rejected; a `<?xml`-led file whose comment outruns the peek is
accepted, where the same shape ended by the end of the file, and a bare
declaration, are rejected (§4.1 step 5); `<!DOCTYPE JMdictX` is rejected.
The existing
`a_trimmed_jmdict_file_under_the_kanjidic_name_is_rejected` test's doc
comment, which explains why the `<?xml` case was not distinguishable, is
rewritten to say that it now is.

## 5. `commands.rs`: the dictionary cluster moves out

`src-tauri/src/commands.rs` is 2,719 lines. Between `shortcut_warning` and
the test module sits the dictionary cluster — about 400 lines of commands,
helpers and state structs — and the test module carries about 590 lines
that belong to it. This phase moves that cluster into two modules under
`src-tauri/src/commands/`, following the `settings.rs` + `settings/tests.rs`
and `layout.rs` + `layout/geometry.rs` precedent. Nothing else in
`commands.rs` moves: the window chrome, the settings commands, the startup
warnings and `win32_frameless` stay where they are.

### 5.1 What goes where

| module | code | tests |
| --- | --- | --- |
| `commands/dictionary.rs` | `IndexSender`, `DictionaryPaths`, `DownloadInFlight`; `needs_dictionary`, `lookup_kanji`, `kanji_info`, `download_dictionary`; `emit_status`, `describe_index_error`, `with_fallback`, `claim_download`, `release_download`, `reparse` | the download-claim pair, the four `describe_index_error` / `with_fallback` tests, the two `reparse` tests, the three kanji tests |
| `commands/hints.rs` | `HintsInFlight`, `HintsSender`; `set_mecab_hints`, `hints_warning`, `hints_dir`; `emit_hints_status`, `describe_hints_error` | the hints-claim test, the load-failure and warning tests, the `HintsHarness` and its helpers, the three async acquisition tests |

`claim_download` and `release_download` guard both the index download and
the hints acquisition, and `reparse` is called by both; they live in
`dictionary.rs` as `pub(super)` and `hints.rs` imports them. Each new file
carries the `// SPDX-License-Identifier: MIT OR Apache-2.0` header the
`src-tauri` files use and a module doc comment; `commands.rs`'s own module
doc drops the sentences that described the download.

### 5.2 What changes outside the moved code

`main.rs` names the new paths: six entries in `generate_handler!`
(`download_dictionary`, `needs_dictionary`, `kanji_info`, `set_mecab_hints`,
`hints_warning`, `hints_dir`) and the five `app.manage(...)` calls for
`IndexSender`, `HintsSender`, `DictionaryPaths`, `DownloadInFlight` and
`HintsInFlight`. Nothing
else references the moved items by path: `layout.rs` reaches only the
chrome helpers, `parse.rs` and `clipboard.rs` mention commands only in
prose. The TypeScript side invokes commands by string name and does not
change.

### 5.3 Proof that nothing moved but text

Three checks, all run rather than assumed:

- The `src-tauri` test binary's listing (`cargo test -p clipyomi -- --list`)
  is captured before and after; the sorted set of test names is identical
  except for the module path prefix, and the count is the same.
- `git diff --stat` for the split commit shows `commands.rs` losing what
  the two new files gain, within the lines the module docs and `use` blocks
  account for.
- Every gate in §6 passes at the same counts.

The split lands as its own commit after the two behaviour changes, so a
reader of the log sees a pure move and not a move with edits inside it.

## 6. Testing and gates

Tests added or rewritten:

- **Popover** — `kanjiLine` with `-ぐ.い`, `ひと-`, and a reading carrying
  both marks; on readings untouched; `た.べる` unchanged; `colourLine` on a
  kun reading that had a hyphen yields no stray text run where it was.
- **Sniff** — §4.4's list, in `jmdict-source`'s own test module.
- **Split** — none new; the moved tests pass in their new modules and the
  listing diff of §5.3 is the assertion.

Gates, in the order that matters, every one run rather than assumed:

1. **Baseline cells byte-identical, both hint states**: strict 0.8174 (1768),
   tolerant 0.8525 (1844), entry top-1 0.9092, compounds 0.6129 (114 / 186)
   with `lexical` 42 / 112, `false-match` 30 / 32, `transparent` 42 / 42
   hints on; 0.7342 (1588) / 0.7924 (1714) / 0.9093 / 0.5860 (109 / 186)
   hints off. Nothing under `engine/` or `index/` is edited, so any movement
   is a defect.
2. `jparser-cli bench` scaling cell inside the band the Phase 3b handoff §2
   and §5 established for **both** hint states: hints off recorded 1.05x,
   measured 0.98x–1.07x across the last seven runs of identical code; hints
   on recorded 1.10x, measured 1.07x–1.12x across four. Inside is a pass;
   outside is a re-run, not an explanation.
3. The `parse_sentences.txt` snapshot byte-identical: no `*.snap.new`.
4. jparser with `mecab` and with `hints-source`, workspace, `src-tauri`,
   npm, `tsc --noEmit`, Playwright; `cargo clippy` under `-D warnings` for
   the workspace and both feature sets; `cargo check --no-default-features`;
   `cargo fmt --all --check`; `cargo +1.85 check -p jparser -p jmdict-source`;
   `cargo llvm-cov` over 80 with **`jmdict-source/src/lib.rs` read
   per-file**, after `cargo llvm-cov clean`. Counts are re-taken at the
   branch tip before anything changes, because commits landed after the
   Phase 3b handoff's figures (mecab 373, hints-source 374, workspace 605
   passed and 1 ignored, `src-tauri` 152, npm 312, Playwright 45).
5. Playwright: no first-run copy and no pictured screen changes, so the
   download-offer baselines are untouched and the `-darwin` pair stays
   exactly as owed.
6. **The sniff, seen once on the real files**: `jparser-cli ensure-dictionary`
   against a scratch copy of the real source directory still opens both
   archives and builds (`kanji: 13108`); the real `JMdict_e.gz` copied to a
   scratch directory as `kanjidic2.xml.gz` is refused with `NotAnArchive`
   naming the KANJIDIC2 file. The GUI check — hover 食 and see `ぐ.い`
   without its hyphen — is the user's, as it was for 3a and 3b.

Work proceeds directly on `feat/engine-v2`, one commit per task, each
staging only the files its task names. The working tree carries the user's
uncommitted edits to `README.md` and `src-tauri/Cargo.toml`; they are never
staged.

## 7. Open items, stated rather than hidden

Carried unchanged from the Phase 3b handoff §5: the former JLPT scale
rendered as the former scale; frequency as a raw newspaper rank; no separate
toggle for the aside; the README feature list; the `-darwin` baselines; the
bench cell as a ratio of two medians.

Left open by this phase's own decisions:

- **`commands.rs` is still about 1,700 lines after the split**, 730 of them
  `win32_frameless`. That module is a single `cfg(windows)` block with one
  test and would move by cut and paste; it stays because it is Windows
  chrome, not engine work.
- **The sniff accepts a file whose leading comment outruns the peek.** By
  design (§4.1 step 5) — the peek, not the file, ended the head, so nothing
  was decided and the accept stands; a comment that ends at the end of the
  file is the other half of that step and is rejected. No real EDRDG file is
  shaped the first way; a stricter reading would need to read past the
  buffer, which is more machinery than the case earns.
- **The two spikes the handoff names are next, and are not phases.** The
  long-unit separation test can run today with no new tooling —
  `corpus_staging/sudachi_result.txt` already holds Mode C output for the
  150 gold sentences and `compound_gold.jsonl` the 139 adjudicated types —
  and its answer decides whether compound preservation has a Phase 4. The
  pitch-accent hunt has two candidates with licences in writing to check
  first: UniDic's accent fields under the BSD leg of its triple licence, and
  Wadoku under CC BY-SA 3.0. Both produce a recommendation, not code.
