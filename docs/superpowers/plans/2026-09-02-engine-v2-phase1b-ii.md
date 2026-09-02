# Engine v2 Phase 1b-ii Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the v2 engine the only engine, delete the nine ported files, and relicense the tree from `GPL-2.0-only` to `MIT OR Apache-2.0` — master plan §1's goal, discharged.

**Architecture:** Four stages in a fixed order. Stage 1 lands everything v1 tolerates (a startup fix, two measured table fixes, a fingerprint pin, two test dispositions). Stage 2 is a single indivisible commit that rewires the crate: shared types relocate out of the deletion set, the index format bumps, `parse` becomes v2, and `src-tauri`, the CLI and every test follow. Stage 3 `git rm`s the orphaned files. Stage 4 rewrites headers, scrubs provenance strings, swaps the licence files and formats once. The tree is never labelled `MIT OR Apache-2.0` while a ported line remains.

**Tech Stack:** Rust 2021 (`crates/jparser`, `src-tauri` as package `clipyomi`), `insta` for the v2 snapshot, `clap` for the CLI, PowerShell for the header sweep. No new dependencies.

**Spec:** [`docs/superpowers/specs/2026-09-02-engine-v2-phase1b-ii-design.md`](../specs/2026-09-02-engine-v2-phase1b-ii-design.md)

**Master plan:** [`docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`](../specs/2026-08-27-engine-v2-master-plan.md)
**Phase 1b-i handoff:** [`docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md`](../2026-09-02-engine-v2-phase1b-i-handoff.md)

## Global Constraints

These apply to every task. Read them before Task 1 and again before any commit.

**Clean-room (master plan §3.5; spec §2, §7).**

- **Do not open the bodies of these ten files:** `crates/jparser/src/{segment,matcher,stem,rank,record,kana,romaji,conjugation}.rs`, `crates/jparser/src/matcher/verb.rs`, `crates/jparser/assets/conjugations.json`. You may read their first eight lines to confirm a header. In stage 2 you remove their `mod` declarations from `lib.rs` without reading them; in stage 3 you `git rm` them. Nothing from them is copied anywhere — `WordFlags` is redefined from `src/types.ts`'s wire names, and 来る's reading rule from the observable alternation, not from `record.rs` or v1's `kuruHack`.
- New code carries the existing Phase 1a-style header until stage 4 replaces every header at once. Do not write SPDX lines before Task 7.

**The stage invariant (spec §1).**

- **Stage 2 (Task 5) is one commit.** Do not commit partway through it. If the working tree does not compile, keep going until it does. Stage 3 (Task 6) is one commit. Stages 1 and 4 may be several.
- The licence flips in Task 7 only, after Task 6 has deleted the last ported file.

**Gates**, run before every commit:

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo clippy -p jparser --all-targets -- -D warnings
cargo clippy -p jparser --all-targets --features mecab -- -D warnings
cargo check -p jparser --no-default-features
cargo test -p clipyomi
```

The sixth is new this phase: `src-tauri` is edited in Tasks 1 and 5, so its tests are a gate from Task 1 on. Baseline at `e08abfb`: `jparser` **433 passed / 0 ignored** default, **468 passed / 1 ignored** with `mecab` (the ignored one is `the_particle_absorbing_merges_are_gone`, Phase 1a's open item; it stays ignored). Measure and record `clipyomi`'s count at the start of Task 1.

**Metrics (spec §9, §10).** `jparser-cli score`, both hint states, over `tests/fixtures/gold.jsonl` against an index built from `tests/fixtures/gold_jmdict_subset.xml`, must reproduce Phase 1b-i's eight cells exactly, or the difference is a finding to report before committing, not a number to record:

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573/2163) | 0.7975 (1725/2163) | 0.9075 (1108/1221) | 75 |
| hints on | 0.8031 (1737/2163) | 0.8521 (1843/2163) | 0.9063 (1170/1291) | 53 |

Tasks 2 and 3 each change candidate generation deliberately and each re-measures. A cell moving **up** after Task 3 is expected (it can only add matches for surfaces unmatched today) and is recorded; a cell moving **down** anywhere is a finding.

`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic` exists (45.6 MB); the hints-on row is measurable and not optional.

**Scope.**

- No change to `cost.rs`, `lattice.rs`, `lex.rs` beyond `entry_classes`, `assemble::order`, or `table_data.rs`. The only `table.rs` changes are Task 2's row reorder and Task 3's second `vk` row plus `classes_for_pos`.
- No TypeScript change except stage 4's comment rewording. The label and the flag names are opaque to the frontend and unchanged.
- `cargo fmt` runs once, in Task 8, and nowhere before.

**Copyright holder** for `LICENSE-MIT`: `Copyright (c) 2026 the ClipYomi contributors`. This is a default the plan chose because no `authors` field exists anywhere; the user may override it before Task 7.

**Commit messages:** `<type>(engine-v2): <description>`, normal prose body.

---

## File Structure

| File | Responsibility after this phase | Task |
| --- | --- | --- |
| `src-tauri/src/state.rs` | `is_missing_dictionary` routes stale indexes to the download screen; `AppState.table: Table` | 1, 5 |
| `crates/jparser/src/engine/table.rs` | `ATTACHES_TO` reordered; `vk` → two classes; `classes_for_pos` | 2, 3 |
| `crates/jparser/src/engine/{flags,lex}.rs` | iterate `classes_for_pos` | 3 |
| `crates/jparser/src/engine/assemble.rs` | 来る reading rule | 3 |
| `crates/jparser/tests/engine_v2.rs` | 来る tests (from `parse_irregular.rs`); fingerprint-rejection test; `parse` rename | 3, 5 |
| `crates/jparser/src/engine/build.rs` | fingerprint pinned; no `StemStats`/`verb_type` | 4, 5 |
| `crates/jparser/src/engine/flags.rs` | **owns `WordFlags`** | 5 |
| `crates/jparser/src/lib.rs` | public surface only: re-exports, `ParseOptions`, `Segment`, `Entry`, `ParseError` | 5 |
| `crates/jparser/src/engine/mod.rs` | `parse` (was `parse_v2`) | 5 |
| `crates/jparser/src/index/{mod,build,load,generations}.rs` | format 4, no `verb_type`, engine-agnostic lifecycle | 5 |
| `crates/jparser/src/hints.rs` | `Boundaries` implementor only | 5 |
| `crates/jparser/src/bin/jparser-cli/{main,score,diff,label}.rs` | single engine | 5 |
| `src-tauri/src/{parse,commands,test_support}.rs` | v2 wiring | 5 |
| `crates/jparser/tests/{entry_id,cli_parse,cli_score,cli_label,index_roundtrip,generations_torn_build}.rs`, `examples/boundary_diag.rs` | repointed | 5 |
| `crates/jparser/tests/fixtures/jmdict_mini.xml` | 言う corrected to `v5u` | 5 |
| nine files + `matcher/verb.rs` + `assets/conjugations.json` + `xtask/` | **deleted** | 6 |
| 62 headers, ~40 `ta-old` sites, `LICENSE-*`, `Cargo.toml`, `package.json`, `README.md` | relicensed | 7 |
| master plan, handoff | closed | 8 |

---

## Stage 1 — Prep (v1 still the default)

### Task 1: A stale index is the download screen's to fix

**Files:**
- Modify: `src-tauri/src/state.rs:100-113` (`is_missing_dictionary` and its doc)
- Test: `src-tauri/src/state.rs` (`mod tests`)

**Interfaces:**
- Produces: `is_missing_dictionary(&StartupError) -> bool` returns `true` for `StartupError::Index(IndexError::ConjugationMismatch{..} | IndexError::VersionMismatch{..})`. Task 5 relies on this when it makes every existing user's index a `ConjugationMismatch`.

- [ ] **Step 1: Record `clipyomi`'s baseline test count**

Run: `cargo test -p clipyomi 2>&1 | Select-String "^test result:"`
Record the sum of `passed` and the `ignored` count; they go in this task's commit message and every later comparison.

- [ ] **Step 2: Write the failing tests**

In `src-tauri/src/state.rs`'s `mod tests`, after `a_corrupt_generation_is_an_index_error_not_no_index`:

```rust
    /// After an engine upgrade the newest generation carries the previous
    /// engine's fingerprint, or an older on-disk format. `ensure_dictionary`
    /// already rebuilds on exactly these two errors, so the download screen
    /// can repair them; treating them as fatal would strand every upgrading
    /// user on an error a click can fix.
    #[test]
    fn a_stale_index_is_the_download_screens_to_fix() {
        let stale_fingerprint = StartupError::Index(IndexError::ConjugationMismatch {
            found: 1,
            expected: 2,
        });
        let stale_format = StartupError::Index(IndexError::VersionMismatch {
            found: 3,
            expected: 4,
        });
        assert!(is_missing_dictionary(&stale_fingerprint));
        assert!(is_missing_dictionary(&stale_format));
    }

    /// Every other index failure stays fatal: a download cannot fix a failing
    /// disk or a corrupt payload, and offering one would loop the user through
    /// a wait that changes nothing.
    #[test]
    fn a_corrupt_or_unreadable_index_is_still_fatal() {
        let io = StartupError::Index(IndexError::Io(std::io::Error::other("boom")));
        assert!(!is_missing_dictionary(&io));
    }
```

- [ ] **Step 3: Run to verify the first fails**

Run: `cargo test -p clipyomi a_stale_index_is_the_download_screens_to_fix`
Expected: FAIL — `is_missing_dictionary` returns `false` for `Index(_)`.

- [ ] **Step 4: Change the predicate**

Replace `is_missing_dictionary` and its doc comment (`state.rs:100-113`) with:

```rust
/// Whether this startup error is one the download screen can fix: no index at
/// all, or an index a previous engine or on-disk format built.
///
/// The second case is deliberate. `ensure_dictionary` rebuilds on exactly
/// `ConjugationMismatch` and `VersionMismatch` — both expected after an
/// application upgrade — so routing them to the download screen repairs a
/// stale index through the path that already exists, with no new UI. Every
/// other `IndexError` is a real failure a download cannot fix.
///
/// Matched exhaustively rather than with a wildcard: a future `StartupError`
/// variant must force a decision here. Offering a download for an error a
/// download cannot fix would loop the user through a wait that changes nothing.
pub fn is_missing_dictionary(error: &StartupError) -> bool {
    match error {
        StartupError::NoIndex { .. } => true,
        StartupError::Index(
            IndexError::ConjugationMismatch { .. } | IndexError::VersionMismatch { .. },
        ) => true,
        StartupError::Index(_) | StartupError::Conjugation(_) | StartupError::Hints(_) => false,
    }
}
```

- [ ] **Step 5: Run both tests**

Run: `cargo test -p clipyomi is_the_download_screens_to_fix` and `cargo test -p clipyomi a_corrupt_or_unreadable_index_is_still_fatal`
Expected: both PASS. Also `cargo test -p clipyomi` in full — the existing `a_corrupt_generation_is_an_index_error_not_no_index` still passes (a garbage `header.bin` is a bincode `Encoding` error, not a mismatch).

- [ ] **Step 6: Gates, then commit**

Run all six gate commands. Then:

```powershell
git add src-tauri/src/state.rs
git commit -m "fix(app): route a stale dictionary index to the download screen" -m "is_missing_dictionary treated every IndexError as fatal. ConjugationMismatch and VersionMismatch are the two errors ensure_dictionary already rebuilds on -- both expected after an upgrade -- so the download screen can repair them through its existing path. Without this, Phase 1b-ii's fingerprint change would show every upgrading user a fatal error for a condition one click fixes. clipyomi baseline: <N> passed / <I> ignored before, +2 after."
```

---

### Task 2: The たい row moves below た, measured

**Files:**
- Modify: `crates/jparser/src/engine/table.rs:255-278` (`ATTACHES_TO` and its doc)
- Regenerate: `crates/jparser/tests/snapshots/engine_v2__sentences.snap`

**Interfaces:**
- Produces: for any surface whose tail `た` follows a 連用形, the `た:基本形` chain is generated before the `たい:ガル接続` chain, so `lex::dedup_shallowest`'s equal-depth tie keeps the past-tense reading. Task 3's 来た test depends on this.

- [ ] **Step 1: Move the row**

In `ATTACHES_TO`, the rows currently read (`table.rs:268-270`):

```rust
    ("たい", "特殊・タイ", "連用形"),
    ("た", "特殊・タ", "連用タ接続"),
    ("た", "特殊・タ", "連用形"),
```

Change them to:

```rust
    ("た", "特殊・タ", "連用タ接続"),
    ("た", "特殊・タ", "連用形"),
    // Below both た rows on purpose. 特殊・タイ removes たい, so its ガル接続
    // suffix た is a bare た over an empty stem -- the same surface as the
    // past auxiliary. Both are real Continuation hits at the same trie node,
    // both accept a preceding 連用形, and they tie on span, entry and depth,
    // so `lex::dedup_shallowest` keeps whichever `Table::new` inserted first.
    // Row order is that order. With たい first, every polite past (読みました)
    // labelled Want-to; with た first, Past. This reorder changes which of two
    // equal-depth chains survives and nothing else -- same span, same
    // entry_id -- so it cannot move a score cell (Phase 1b-ii design §9.2).
    // The principled fix, making ガル接続 non-terminal, deletes candidates and
    // is a measured Phase 1a follow-up.
    ("たい", "特殊・タイ", "連用形"),
```

Also append one sentence to `ATTACHES_TO`'s doc comment (before `const ATTACHES_TO`): "Row order is load-bearing where two rows produce the same surface at the same node — see the たい row."

- [ ] **Step 2: The gate and the coverage walk still hold**

Run: `cargo test -p jparser --test conjugation_labels`
Expected: 3 passed — every chain is still generated exactly once (the fixture pins each chain by its rendering, not by order).

Run: `cargo test -p jparser --lib engine::table::tests::the_label_fixture_covers_every_reachable_tuple_and_fmt_is_total`
Expected: PASS, still 21,760 chains (same rows, different order).

- [ ] **Step 3: See the snapshot diff, then accept it**

Run: `cargo test -p jparser --test engine_v2 v2_snapshots_every_sentence`
Expected: FAIL with insta's diff. Every changed line must be a `Want-to` → `Past` flip on a surface ending in た, with headword, reading and gloss unchanged. Phase 1b-i's handoff names three: 読みました and しました (`Polite Want-to` → `Polite Past`) and 書かせられた (`Causative Passive/Potential Want-to` → `…Past`). More of the same shape may appear among lower-ranked sibling entries; that is expected. If any changed line is *not* a Want-to→Past flip, stop and report.

Then:

```powershell
$env:INSTA_UPDATE = "always"
cargo test -p jparser --test engine_v2 v2_snapshots_every_sentence
Remove-Item Env:\INSTA_UPDATE
git diff --stat crates/jparser/tests/snapshots/engine_v2__sentences.snap
```

Count the flipped lines for the commit message.

- [ ] **Step 4: Measure**

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase1b-ii"
New-Item -ItemType Directory -Force $scratch | Out-Null
cargo run --release -p jparser --bin jparser-cli -- build-index --engine v2 `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\gold-v2"
$gold = "crates/jparser/tests/fixtures/gold.jsonl"
$app  = "$env:APPDATA\dev.jparser.clipyomi"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-v2" $gold --engine v2
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-v2" $gold --engine v2 --hints "$app\hints\current\system.dic"
```

Expected: all eight cells equal the Global Constraints table exactly. A difference is a finding: the reorder changes only which of two same-span, same-entry chains survives, so `score` — which reads spans and `entries.first().entry_id` only — cannot see it. Report rather than commit if any cell differs.

- [ ] **Step 5: Gates, then commit**

```powershell
git add crates/jparser/src/engine/table.rs crates/jparser/tests/snapshots/engine_v2__sentences.snap
git commit -m "fix(engine-v2): let the past auxiliary win the bare-た tie" -m "特殊・タイ's ガル接続 surface is a bare た over an empty stem, the same surface as the past auxiliary. Both chains tie in lex::dedup_shallowest and ATTACHES_TO's row order decided that Want-to survived, so every polite past labelled Want-to. Moving the たい row below the two た rows flips <K> snapshot labels from Want-to to Past and nothing else. score reproduces Phase 1b-i's eight cells exactly in both hint states, as the code guarantees: the survivor has the same span and entry_id."
```

---

### Task 3: Kanji 来る deinflects and reads correctly, measured

**Files:**
- Modify: `crates/jparser/src/engine/table.rs:54-66` (`JMDICT_POS_CLASS`), `:206-212` (`class_for_pos`), `:586` (test), the coverage test's chain-count literal
- Modify: `crates/jparser/src/engine/flags.rs:21,58-72`
- Modify: `crates/jparser/src/engine/lex.rs:25,225-240`
- Modify: `crates/jparser/src/engine/assemble.rs` (`reading_for` and its doc; tests at `:228,281,319`)
- Modify: `crates/jparser/tests/engine_v2.rs` (new tests)
- Delete: `crates/jparser/tests/parse_irregular.rs`

**Interfaces:**
- Produces: `pub fn engine::table::classes_for_pos(pos: &str) -> impl Iterator<Item = ClassId> + '_` replaces `class_for_pos`. `vk` yields `カ変・クル` then `カ変・来ル`. Task 5 touches none of this.

- [ ] **Step 1: Write the failing integration tests — the ported observables**

Append to `crates/jparser/tests/engine_v2.rs`:

```rust
// 来る in kanji: the one common verb whose kanji stem's reading changes with
// the form (来た = きた, 来ない = こない, 来る = くる). These are the observables
// `tests/parse_irregular.rs` pinned for v1, ported to v2. A private fixture,
// because `jmdict_subset.xml` is extractor-generated and would drop a
// hand-added entry on regeneration (`fixtures/README.md`).
const KURU_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY vk "Kuru verb - special class">
]>
<JMdict>
<entry>
<ent_seq>1000040</ent_seq>
<k_ele><keb>来る</keb></k_ele>
<r_ele><reb>くる</reb></r_ele>
<sense><pos>&vk;</pos><gloss>to come</gloss></sense>
</entry>
</JMdict>
"#;

fn parse_kuru(name: &str, text: &str) -> jparser::ParseResult {
    let dir = scratch(name);
    jparser::engine::build::build_from_reader(BufReader::new(KURU_FIXTURE.as_bytes()), &dir)
        .expect("v2 build");
    parse_v2_over(&dir, text)
}

#[test]
fn kuru_in_kanji_uninflected_keeps_the_dictionary_reading() {
    let r = parse_kuru("kuru-plain", "来る");
    assert_eq!(r.segments.len(), 1);
    let s = &r.segments[0];
    assert_eq!((s.start, s.len, s.surface.as_str()), (0, 2, "来る"));
    assert!(s.matched);
    assert_eq!(s.reading.as_deref(), Some("くる"));
    assert_eq!(s.entries[0].headword, "来る");
    assert_eq!(s.entries[0].conjugation, None);
}

#[test]
fn kuru_in_kanji_deinflects_and_reads_through_the_kana_class() {
    // Fails today at the first assertion: `vk` maps only to カ変・クル, whose
    // suffixes are whole kana words, so the kanji stem 来 matches nothing and
    // 来 + た are two unmatched spans.
    let r = parse_kuru("kuru-past", "来た");
    assert_eq!(r.segments.len(), 1, "one span, not 来 + た: {:?}", r.segments);
    let s = &r.segments[0];
    assert!(s.matched);
    // Not くた: 来's reading in 連用形 is き, which カ変・クル's same-named
    // form supplies.
    assert_eq!(s.reading.as_deref(), Some("きた"));
    let e = &s.entries[0];
    assert_eq!(e.headword, "来る");
    assert_eq!(e.reading.as_deref(), Some("きた"));
    assert_eq!(e.conjugation.as_deref(), Some("Past"));
    assert_eq!(e.pos, vec!["vk"]);
    assert_eq!(e.senses[0].glosses, vec!["to come"]);
}

#[test]
fn kuru_in_kanji_leaves_an_unmatched_tail_alone() {
    // 。 is in no dictionary here; an unmatched run has no reading and no
    // entries, and the caller can rebuild the input verbatim from the spans.
    let r = parse_kuru("kuru-unmatched", "来た。");
    assert_eq!(r.segments.len(), 2);
    assert!(r.segments[0].matched);
    let tail = &r.segments[1];
    // Char offset 2, not byte offset 6: 来た。 is 3 chars and 9 bytes.
    assert_eq!((tail.start, tail.len, tail.surface.as_str()), (2, 1, "。"));
    assert!(!tail.matched);
    assert_eq!(tail.reading, None);
    assert!(tail.entries.is_empty());
}

#[test]
fn empty_text_parses_into_no_segments() {
    let r = parse_kuru("kuru-empty", "");
    assert!(r.segments.is_empty());
}
```

- [ ] **Step 2: Run to verify the second fails as described**

Run: `cargo test -p jparser --test engine_v2 kuru_in_kanji`
Expected: `kuru_in_kanji_deinflects_and_reads_through_the_kana_class` FAILS at `segments.len() == 1` (got 2). The other three pass already.

- [ ] **Step 3: `vk` maps to both classes; `classes_for_pos` replaces `class_for_pos`**

In `crates/jparser/src/engine/table.rs`, after the existing `("vk", "カ変・クル"),` row (`:66`), add:

```rust
    // 来る written with its kanji. The stem is 来 and this class's suffixes are
    // what follows it (empty for 未然形 and 連用形, る for 基本形), where
    // カ変・クル's suffixes are the whole kana word over an empty stem. A
    // headword can only match through the class whose remove-suffix it ends
    // in, so both rows are needed and the headword decides; `classes_for_pos`
    // returns every row for a code. Without this row every kanji form -- 来た,
    // 来ない, 来ます -- is unmatched (Phase 1b-ii design §9.1).
    ("vk", "カ変・来ル"),
```

Replace `class_for_pos` (`:206-212`) with:

```rust
/// Every IPADIC conjugation class a JMdict POS code maps to, in table order.
///
/// Usually zero or one. `vk` is the one code with two, because 来る's kanji
/// and kana spellings conjugate over different stems (`カ変・来ル`, `カ変・クル`)
/// and a headword can only match through the class whose remove-suffix it
/// ends in — so the entry's own headword picks between them at match time.
pub fn classes_for_pos(pos: &str) -> impl Iterator<Item = ClassId> + '_ {
    JMDICT_POS_CLASS
        .iter()
        .filter(move |(p, _)| *p == pos)
        .filter_map(|(_, name)| CLASSES.iter().position(|c| c.name == *name))
        .map(|i| i as ClassId)
}
```

Update the doc comment above `JMDICT_POS_CLASS` (`:47-53`): its sentence "the code must map to a class the generated table actually carries" stands; add "A code may appear in more than one row."

In `table.rs`'s test `every_jmdict_verb_pos_in_the_fixture_maps_to_a_real_class` (`:586`), change `class_for_pos(pos).is_some()` to `classes_for_pos(pos).next().is_some()`.

- [ ] **Step 4: The two production callers iterate**

`crates/jparser/src/engine/flags.rs`: change the import at `:21` to `use super::table::classes_for_pos;` and `classes` (`:58-72`) to:

```rust
/// `classes_for_pos` over every sense's POS codes, deduplicated. An entry
/// tagged `v5r` in one sense and `v1` in another legitimately yields two
/// classes, and `vk` alone yields two, which is why this is a `Vec` and not
/// an `Option`.
fn classes(raw: &RawEntry) -> Vec<ClassId> {
    let mut out = Vec::new();
    for pos in all_pos(raw) {
        for class in classes_for_pos(pos) {
            if !out.contains(&class) {
                out.push(class);
            }
        }
    }
    out
}
```

`crates/jparser/src/engine/lex.rs`: change the import at `:25` to `use crate::engine::table::{classes_for_pos, ChainHit, Table, CLASSES};` and `entry_classes` (`:228-239`) to:

```rust
fn entry_classes(entry: &EntryData) -> Vec<ClassId> {
    let mut classes = Vec::new();
    for sense in &entry.senses {
        for pos in &sense.pos {
            for class in classes_for_pos(pos) {
                if !classes.contains(&class) {
                    classes.push(class);
                }
            }
        }
    }
    classes
}
```

`crates/jparser/src/engine/assemble.rs` tests: `:228` import becomes `use crate::engine::table::classes_for_pos;`; `:281` becomes `let class = classes_for_pos("v5k").next().expect("v5k maps to a class");`; `:319` becomes `let class = classes_for_pos("v1").next().expect("v1 maps to a class");`.

- [ ] **Step 5: Run — the match now happens, the reading is wrong**

Run: `cargo test -p jparser --test engine_v2 kuru_in_kanji_deinflects`
Expected: FAIL at `reading == Some("きた")` with `Some("くた")`. This is the failure the reading rule exists for: `reading_for` strips `カ変・来ル`'s remove `る` from くる and appends た.

- [ ] **Step 6: The reading rule**

In `crates/jparser/src/engine/assemble.rs`, above `reading_for`, add:

```rust
/// The kanji spelling of 来る. Its stem's reading alternates with the form
/// (来た = きた, 来ない = こない, 来る = くる), so stripping the class's
/// remove-suffix from the dictionary reading -- right for every other class
/// -- would produce くた.
const KURU_KANJI_CLASS: &str = "カ変・来ル";
/// The kana spelling's class. It conjugates the same ten form names over an
/// empty stem, so each form's suffix *is* the kanji stem's reading in that
/// form.
const KURU_KANA_CLASS: &str = "カ変・クル";

/// `カ変・クル`'s suffix for the form named `form`: the reading of 来's stem
/// in that form. `None` only if the two classes' form sets have drifted
/// apart, which `kuru_classes_share_every_form_name` guards against.
fn kuru_stem_reading(form: &str) -> Option<&'static str> {
    CLASSES
        .iter()
        .find(|c| c.name == KURU_KANA_CLASS)?
        .forms
        .iter()
        .find(|f| f.name == form)
        .map(|f| f.suffix)
}
```

Replace the last paragraph of `reading_for`'s doc comment (the one beginning "When the dictionary reading does not end in that suffix at all") with:

```rust
///   Kanji 来る is the exception: under `カ変・来ル` the strip *succeeds* (くる
///   ends in る) and would give くた, so that class takes its stem reading
///   from `カ変・クル`'s same-named form instead -- 連用形 き, 未然形 こ,
///   仮定形 くれ -- followed by the surface after the step's own range. Written
///   from the observable alternation; nothing here derives from v1.
```

Replace `reading_for`'s body from `let class = ...` to the end with:

```rust
    let class = &CLASSES[first_step.class as usize];
    let end = cand.start + cand.len;
    if class.name == KURU_KANJI_CLASS {
        let form = class.forms[first_step.form as usize].name;
        let stem = kuru_stem_reading(form)?;
        // The step's own range is the kanji form's suffix (empty for 連用形,
        // れ for 仮定形), already accounted for by the kana form's suffix; the
        // auxiliaries after it are kana as written.
        let tail: String = chars[first_step.range.end..end].iter().collect();
        return Some(format!("{stem}{tail}"));
    }
    let stem = dict_reading.strip_suffix(class.remove)?;
    let tail: String = chars[first_step.range.start..end].iter().collect();
    Some(format!("{stem}{tail}"))
```

Add to `assemble.rs`'s `mod tests`, after `a_reading_that_does_not_end_in_the_classs_suffix_is_never_invented`:

```rust
    fn kuru_step(form: &str, range: std::ops::Range<usize>) -> ConjStep {
        let class = CLASSES
            .iter()
            .position(|c| c.name == KURU_KANJI_CLASS)
            .expect("カ変・来ル") as u16;
        let form = CLASSES[class as usize]
            .forms
            .iter()
            .position(|f| f.name == form)
            .expect("form") as u16;
        ConjStep {
            class,
            form,
            range,
            aux: None,
        }
    }

    #[test]
    fn kuru_in_kanji_reads_its_stem_from_the_kana_class() {
        let e = entry(&["くる"]);
        // 来た: 来 is 連用形 (empty suffix, range 1..1); た follows.
        let chars: Vec<char> = "来た".chars().collect();
        let c = candidate(0, 2, WordFlags::PRIMARY, vec![kuru_step("連用形", 1..1)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("きた".to_string()));
        // 来ない: 未然形, also empty.
        let chars: Vec<char> = "来ない".chars().collect();
        let c = candidate(0, 3, WordFlags::PRIMARY, vec![kuru_step("未然形", 1..1)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("こない".to_string()));
        // 来れ: 仮定形's suffix れ is the step's own range, so nothing follows.
        let chars: Vec<char> = "来れ".chars().collect();
        let c = candidate(0, 2, WordFlags::PRIMARY, vec![kuru_step("仮定形", 1..2)]);
        assert_eq!(reading_for(&e, &c, &chars), Some("くれ".to_string()));
    }

    #[test]
    fn kuru_classes_share_every_form_name() {
        // The reading rule looks each カ変・来ル form up by name in カ変・クル.
        // A regeneration that renamed a form in one class only would make the
        // lookup fail and the reading silently vanish; this catches it.
        let kanji = CLASSES
            .iter()
            .find(|c| c.name == KURU_KANJI_CLASS)
            .expect("kanji class");
        for form in kanji.forms {
            assert!(
                kuru_stem_reading(form.name).is_some(),
                "{} has no カ変・クル twin",
                form.name
            );
        }
    }
```

- [ ] **Step 7: Run — GREEN**

Run: `cargo test -p jparser --test engine_v2 kuru` and `cargo test -p jparser --lib engine::assemble::tests::kuru`
Expected: all PASS.

- [ ] **Step 8: Re-pin the coverage walk's chain count**

Run: `cargo test -p jparser --lib engine::table::tests::the_label_fixture_covers_every_reachable_tuple_and_fmt_is_total`
Expected: FAIL on the exact-count assertion — one more mapped class (`カ変・来ル`, ten forms) reaches more chains. The message reports the new count. **Also check `missing` is empty**: `カ変・来ル`'s ten form names (仮定形, 仮定縮約１, 体言接続特殊, 体言接続特殊２, 命令ｉ, 命令ｙｏ, 基本形, 未然ウ接続, 未然形, 連用形) are all already verb tuples in the fixture's Part A, so no row should be missing. If one is, add a row on stem 来 with the same English as the existing row for that form.

Update the literal and its message to the new count, re-deriving the three-term sum in the message the way the existing one does (depth 0 + depth 1 + depth 2 over the now-21 mapped classes). Run again: PASS.

- [ ] **Step 9: Delete the v1 test file**

```powershell
git rm crates/jparser/tests/parse_irregular.rs
```

Its four observables now live in `engine_v2.rs`. (It had no `[[test]]` entry in `Cargo.toml`.)

- [ ] **Step 10: Measure**

Same commands as Task 2 Step 4, after rebuilding `$scratch\gold-v2` (the table changed, so the index's fingerprint changed and it must be rebuilt). Expected: every cell **equal or higher** than the Global Constraints table. The change can only turn unmatched 来 spans into matched ones. Record all eight cells for the commit message. Any cell **lower** is a finding — report, do not commit.

- [ ] **Step 11: Gates, then commit**

```powershell
git add crates/jparser/src/engine/table.rs crates/jparser/src/engine/flags.rs crates/jparser/src/engine/lex.rs crates/jparser/src/engine/assemble.rs crates/jparser/tests/engine_v2.rs
git commit -m "fix(engine-v2): deinflect kanji 来る and read its stem through the kana class" -m "vk mapped to カ変・クル only, whose suffixes are whole kana words, so the kanji headword's stem 来 matched nothing and 来た, 来ない, 来ます were all unmatched. JMDICT_POS_CLASS now maps vk to カ変・来ル as well and classes_for_pos returns every class for a code. That alone made 来た read くた -- the class's remove る strips cleanly off くる -- so reading_for takes a カ変・来ル stem's reading from カ変・クル's same-named form instead: 来た きた, 来ない こない, 来れ くれ. Written from the observable alternation. parse_irregular.rs's four v1 observables are ported to engine_v2.rs and the file deleted. Coverage walk re-pinned at <N> chains. score: <eight cells>, vs Phase 1b-i's <eight cells>."
```

---

### Task 4: Pin the fingerprint; delete the v1 snapshot

**Files:**
- Modify: `crates/jparser/src/engine/build.rs:159-177` (`mod tests`)
- Delete: `crates/jparser/tests/parse_snapshots.rs`, `crates/jparser/tests/snapshots/parse_snapshots__sentences.snap`

- [ ] **Step 1: Add the pin with a wrong value**

In `engine/build.rs`'s `mod tests`, after `fingerprint_is_stable_across_calls`:

```rust
    /// Pinned so a regeneration of `table_data.rs` is a visible, deliberate
    /// change: every class and form id an index stores is relative to that
    /// table, and `Index::open` refuses an index whose fingerprint differs.
    /// After an intended regeneration, run this test, take the `left` value
    /// from its failure, and replace the literal. Never loosen it.
    #[test]
    fn fingerprint_is_pinned() {
        assert_eq!(fingerprint(), 0);
    }
```

- [ ] **Step 2: Run, read the value, pin it**

Run: `cargo test -p jparser --lib engine::build::tests::fingerprint_is_pinned`
Expected: FAIL; the message's `left:` is the current fingerprint. Replace `0` with that value written as a `0x…` literal with `_` separators every four hex digits. Run again: PASS.

Leave `fingerprint_differs_from_the_v1_conjugation_asset` in place — it still compiles until Task 5 deletes `conjugation.rs`'s function, and Task 5 deletes the test.

- [ ] **Step 3: Delete the v1 snapshot test**

```powershell
git rm crates/jparser/tests/parse_snapshots.rs crates/jparser/tests/snapshots/parse_snapshots__sentences.snap
```

`tests/engine_v2.rs`'s `v2_snapshots_every_sentence` already owns the same 30 sentences from `parse_sentences.txt` against v2. Record how many tests the default count drops by.

- [ ] **Step 4: Gates, then commit**

```powershell
git add crates/jparser/src/engine/build.rs
git commit -m "test(engine-v2): pin the table fingerprint; drop v1's redundant snapshot" -m "The fingerprint is now a literal so a table regeneration fails a test rather than silently invalidating every index. parse_snapshots.rs snapshotted v1 over the same 30 sentences engine_v2.rs already snapshots against v2; with v1 leaving next commit it has no subject."
```

---

## Stage 2 — The switchover (ONE commit)

### Task 5: v2 becomes the only engine

This is the indivisible commit. It touches about forty files, all listed below with the exact edit. Work through the sub-sections in order — A through M — then run the gates. **Do not commit until every gate is green.** If you cannot make it compile, report BLOCKED with the exact errors rather than committing a partial state.

**Files:** every file named in sub-sections A–M.

**Interfaces:**
- Consumes: `classes_for_pos` (Task 3), `is_missing_dictionary`'s new arm (Task 1).
- Produces: `jparser::parse(index: &Index, table: &engine::table::Table, text: &str, opts: &ParseOptions, bounds: Option<&dyn Boundaries>) -> Result<ParseResult, ParseError>`; `jparser::WordFlags`; `jparser::Boundaries`; `index::ensure_dictionary(root: &Path, keep: usize, source: F) -> Result<Index, IndexError>`; `generations::build_new(root: &Path, xml: impl BufRead) -> Result<(PathBuf, BuildReport), IndexError>`; `Index::open(dir)` checks `engine::build::fingerprint()`; `INDEX_FORMAT_VERSION == 4`; `StoredRecord { surface, flags: u16, entry_id }`; `BuildReport { keys, records, entries, skipped_entries }`.

#### A. `WordFlags` moves to `engine/flags.rs`

- [ ] **A1.** In `crates/jparser/src/engine/flags.rs`, delete the line `use crate::record::WordFlags;` (`:24`) and, directly after the `ARCHAIC_MISC` const (`:31`), add:

```rust
/// Per-record flags the cost model and the webview read.
///
/// A `u16` newtype with eight named bits. The eight **names** are the
/// contract: they are the strings `Entry::flags` serialises and
/// `src/types.ts`'s `FlagName` declares, and the webview picks a chip's
/// content class by them. The bit **values** are arbitrary — an index stores
/// the raw `u16` (`StoredRecord::flags`), and Phase 1b-ii bumped the index
/// format at the same time it assigned these, so nothing built before them
/// can be read regardless. See `headwords` below for when each is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WordFlags(pub u16);

impl WordFlags {
    pub const PRIMARY: WordFlags = WordFlags(1 << 0);
    pub const PRONOUNCE: WordFlags = WordFlags(1 << 1);
    pub const COMMON_LINE: WordFlags = WordFlags(1 << 2);
    pub const COMMON: WordFlags = WordFlags(1 << 3);
    pub const PARTICLE: WordFlags = WordFlags(1 << 4);
    pub const COUNTER: WordFlags = WordFlags(1 << 5);
    /// Reserved; set by nothing (Phase 1a design §8).
    pub const TOP: WordFlags = WordFlags(1 << 6);
    /// Reserved for JMnedict; set by nothing (Phase 1a design §8).
    pub const IS_NAME: WordFlags = WordFlags(1 << 7);

    /// Every flag with its wire name, in bit order.
    const NAMES: [(WordFlags, &'static str); 8] = [
        (Self::PRIMARY, "primary"),
        (Self::PRONOUNCE, "pronounce"),
        (Self::COMMON_LINE, "common_line"),
        (Self::COMMON, "common"),
        (Self::PARTICLE, "particle"),
        (Self::COUNTER, "counter"),
        (Self::TOP, "top"),
        (Self::IS_NAME, "is_name"),
    ];

    pub fn contains(self, other: WordFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: WordFlags) {
        self.0 |= other.0;
    }

    /// The wire names of every set flag, in bit order.
    pub fn names(self) -> impl Iterator<Item = &'static str> {
        Self::NAMES
            .iter()
            .filter(move |(flag, _)| self.contains(*flag))
            .map(|(_, name)| *name)
    }
}

impl serde::Serialize for WordFlags {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.names())
    }
}
```

- [ ] **A2.** Add to `flags.rs`'s `mod tests`:

```rust
    #[test]
    fn flags_serialise_as_their_wire_names_in_bit_order() {
        let mut f = WordFlags::default();
        f.insert(WordFlags::COMMON);
        f.insert(WordFlags::PRIMARY);
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            r#"["primary","common"]"#,
            "the eight names are what src/types.ts's FlagName declares"
        );
        assert_eq!(serde_json::to_string(&WordFlags::default()).unwrap(), "[]");
    }

    #[test]
    fn contains_and_insert_are_bitwise() {
        let mut f = WordFlags::PRIMARY;
        assert!(f.contains(WordFlags::PRIMARY));
        assert!(!f.contains(WordFlags::COMMON));
        f.insert(WordFlags::COMMON);
        assert!(f.contains(WordFlags::COMMON));
        assert!(f.contains(WordFlags::PRIMARY));
    }

    #[test]
    fn the_eight_wire_names_are_exactly_types_ts_flagname() {
        let names: Vec<&str> = WordFlags(u16::MAX).names().collect();
        assert_eq!(
            names,
            ["primary", "pronounce", "common_line", "common", "particle", "counter", "top", "is_name"]
        );
    }
```

- [ ] **A3.** Repoint every other importer to the new home:
  - `engine/assemble.rs:32`, `engine/cost.rs:32`, `engine/lex.rs:29` → `use crate::engine::flags::WordFlags;`
  - `engine/lattice.rs:182` (test) → `use crate::engine::flags::WordFlags;`
  - `bin/jparser-cli/main.rs:29`, `bin/jparser-cli/score.rs:390` (test), `tests/engine_v2.rs:15`, `tests/index_roundtrip.rs:14` → `use jparser::WordFlags;` (the crate-root re-export sub-section B adds)

#### B. `lib.rs` becomes the public surface only

- [ ] **B1.** Replace `crates/jparser/src/lib.rs` lines 8–50 (module doc, `use`s, `mod`s, re-exports) with:

```rust
//! Japanese text parsing: dictionary matching, segmentation, entry assembly.
//!
//! `parse` is the whole public surface. It runs the Engine v2 core
//! (`engine`): candidate generation over a deinflection trie, a min-cost
//! segmentation with tokenizer boundaries as hard constraints, and entry
//! assembly — headword, reading and conjugation label — for each span.
//!
//! Every offset in this API is a **character** offset. The single conversion
//! point is `text.chars().collect()` at the top of `parse`; nothing below it
//! ever sees a byte index.

pub mod engine;
#[cfg(feature = "mecab")]
pub mod hints;
pub mod index;
pub mod jmdict;

pub use crate::engine::flags::WordFlags;
pub use crate::engine::parse;
pub use crate::engine::Boundaries;
/// One dictionary sense. A re-export rather than a parallel owned struct with
/// the same five fields.
pub use crate::index::SenseData as Sense;
```

Keep lines 1–7 (the licence header) as they are; Task 7 replaces them.

- [ ] **B2.** `ParseOptions`'s doc (`:52-58`): replace "boundary votes arrive through `parse`'s `hints` parameter" with "tokenizer boundaries arrive through `parse`'s `bounds` parameter" and delete the clause about "the v5 mis-annotation fallback is a build-time `StemOptions` flag".

- [ ] **B3.** `Entry`'s docs:
  - `conjugation` (`:101-108`): replace the whole comment with
    ```rust
    /// `engine::label::fmt`'s English label for the deinflection chain, e.g.
    /// `"Polite Negative Past"` (Phase 1b-i design §3). `None` when no step
    /// contributes a word: an uninflected match, a plain form, or a bare
    /// stem.
    ```
  - `steps` (`:126-132`): delete "and on the v1 path, which does not compute it" and "on the v2 path"; the remaining text reads "Empty for an uninflected match. `engine::label::fmt` turns this into `conjugation`'s English label; see docs/superpowers/specs/2026-09-02-engine-v2-phase1b-i-design.md §3."

- [ ] **B4.** Delete everything from `pub fn parse(` (`:156`) through the end of the file: the v1 `parse` body, `entry_data`, `assemble_entry`, `dictionary_form`, `reconstruct_reading`, `strip_remove_suffix`, `KURU_HACK_MAX_CHARS`, `kuru_hack`, `tails_match`, and the whole `#[cfg(test)] mod tests` (`:421-775`). Keep `ParseError` (`:140-148`) and the doc above it, rewording "the only fallible step in Phase 1B" to "the only fallible step".

#### C. `engine::parse_v2` becomes `parse`

- [ ] **C1.** In `crates/jparser/src/engine/mod.rs`: rename `pub fn parse_v2(` to `pub fn parse(`. Replace its doc's first three lines (`/// Parse text with the Engine v2 core.` through `/// parameter, which is a constraint source rather than an advisory hint.`) with:

```rust
/// Parse `text` against an already-open index — the crate's public entry
/// point, re-exported as `jparser::parse`.
///
/// `bounds` is a tokenizer's boundary set, applied as a hard constraint:
/// no span may end strictly inside a token. `None` lifts every constraint.
```

Replace the module doc (`:13-17`) with:

```rust
//! The segmentation engine. `crate::parse` is this module's `parse`.
//!
//! Written clean-room for ClipYomi (master plan §3.5); the Phase 1a design
//! records its cost model and the Phase 1b-i design its label formatter.
```

- [ ] **C2.** In `engine/mod.rs`'s `ConjStep` doc (`:50-54`), delete "Phase 1a leaves `Entry::conjugation` as `None`."

#### D. `index/mod.rs`

- [ ] **D1.** Delete the four imports at `:25-29` (`ConjugationTable`, `VerbTypeId`, `StemOptions`, `StemStats`). Keep `use crate::index::load::Index;`.

- [ ] **D2.** `INDEX_FORMAT_VERSION` (`:33`) → `4`, and extend its doc: "4 (Phase 1b-ii): `StoredRecord` lost `verb_type`."

- [ ] **D3.** `StoredRecord` (`:63-76`): delete the `verb_type` field and its doc comment; change `flags`'s line to

```rust
    /// The raw bits of an `engine::flags::WordFlags`.
    pub flags: u16,
```

and the `surface` doc to "Original, unnormalized surface. Compared against the source text to set the inexact-match flag."

- [ ] **D4.** `EntryData::readings` doc (`:88-93`): replace "exactly the set ta-old flags `JAP_WORD_PRONOUNCE`" with "exactly the readings a kanji entry needs furigana for" and "where the surface already is the reading and ta-old renders no furigana" with "where the surface already is the reading". (Stage 4 greps for the string; removing it here saves a second visit.)

- [ ] **D5.** `BuildReport` (`:97-105`): delete `pub stems: StemStats,`.

- [ ] **D6.** `ensure_dictionary` (`:140-198`): signature becomes

```rust
pub fn ensure_dictionary<R, F>(root: &Path, keep: usize, source: F) -> Result<Index, IndexError>
```

Its doc's "expected after an application upgrade or a changed `conjugations.json`" becomes "expected after an application upgrade or a regenerated conjugation table". The `build_new` call becomes `generations::build_new(root, xml)?`.

- [ ] **D7.** `index/mod.rs` `mod tests` (`:233-611`): delete `use crate::conjugation::ConjugationTable;`, `use crate::stem::StemOptions;` and the `table_and_opts` helper; delete every `let (table, opts) = table_and_opts();` line; change every `ensure_dictionary(&root, &table, &opts, K, || ...)` to `ensure_dictionary(&root, K, || ...)` and every `generations::build_new(&root, XML.as_bytes(), &table, &opts)` to `generations::build_new(&root, XML.as_bytes())`. The test at `:329` that sets `h.version = INDEX_FORMAT_VERSION - 1` still means "stale" and stays.

- [ ] **D8.** The module doc (`:8-14`) and `IndexHeader`'s doc (`:50-58`): delete the sentences naming `ta-old`, `LoadDict`/`FindMatches` and `Dictionary.cpp`; describe the layout in its own terms ("an FST from folded keys to record offsets, plus length-prefixed bincode blobs for records and entries").

#### E. `index/build.rs` keeps only the writer

- [ ] **E1.** Delete `pub fn build_from_reader` (`:26` through the closing brace before `write_index`'s doc, around `:118`) and `fn push` (`:181` to end). Delete the imports `ConjugationTable`, `parse_entries`, `unify_str`, `headwords`, `generate_stems, StemOptions, StemStats`, and `BufRead` if nothing else uses it. Keep `BTreeMap`, `File`, `BufWriter`, `Write`, `Path` and the `crate::index::{...}` import, trimming it to what `write_index` uses.

- [ ] **E2.** Replace the module doc with: `//! Writes the on-disk index. The builder is [`crate::engine::build`]; this is the format's one writer, so the two cannot drift.` Reword `write_index`'s doc: "Shared by both engines' builders" → "The format's one writer"; delete "because v2's builder must stamp its own table's fingerprint instead" → "the caller supplies the table fingerprint it built against".

#### F. `index/load.rs`

- [ ] **F1.** `Index::open` (`:95-97`) becomes `Self::open_with(dir, crate::engine::build::fingerprint())`. Its doc's last sentence about the two engines is replaced by: "Checks the index was built against the current conjugation table (`engine::build::fingerprint`); `open_with` takes the fingerprint explicitly, for tests."

- [ ] **F2.** Delete `pub fn prefixes_of` (`:171-183`) and its doc, and `use crate::kana::unify;` (`:20`). `prefixes_of_with` stays `pub`.

- [ ] **F3.** `open_with`'s doc (`:99-105`): delete "The two engines assign conjugation ids from different tables, so each must open only its own index … At the Phase 1b switchover this becomes the only form." Keep the sentence about the guard making a cross-read impossible, rephrased to "a stale index".

#### G. `index/generations.rs`

- [ ] **G1.** Delete imports at `:27,30`. `build_new` (`:185-190`) becomes `pub fn build_new(root: &Path, xml: impl BufRead) -> Result<(PathBuf, BuildReport), IndexError>`; its body's `build_from_reader(xml, table, opts, &build_dir)?` becomes `crate::engine::build::build_from_reader(xml, &build_dir)?` and the `use crate::index::build::build_from_reader;` import goes.

- [ ] **G2.** Tests (`:284-285,302-305,451-458`): delete the two imports; `build_into` becomes `let (path, _) = build_new(root, MINI_XML.as_bytes()).expect("build_new");` with the `table`/`opts` lines removed; `two_concurrent_builds_use_distinct_temp_names` likewise (`build_new(&root, MINI_XML.as_bytes())` inside the spawn).

#### H. `engine/build.rs`

- [ ] **H1.** Delete `use crate::stem::StemStats;` (`:34`), the `verb_type: None,` field (`:120`), and `stems: StemStats::default(),` with its comment (`:139-140`). Delete the test `fingerprint_differs_from_the_v1_conjugation_asset` (`:168-176`).

- [ ] **H2.** Module doc (`:13-22`): replace with

```rust
//! Builds the index from JMdict XML: one `StoredRecord` per
//! [`crate::engine::flags::Headword`], keyed by the folded surface, and no
//! generated stems — deinflection is a parse-time walk of
//! [`crate::engine::table::Table`]. [`fingerprint`] stamps the header so
//! `Index::open` refuses an index built against a different table.
```

`build_from_reader`'s doc (`:72-76`) becomes `/// Builds an index from `xml` into `out_dir`.`

#### I. `hints.rs` keeps the `Boundaries` half

- [ ] **I1.** Delete, in this order, letting `cargo check -p jparser --features mecab` name anything left: the `BoundaryFlags` struct and its `impl BoundaryHints` (`:50-69`); `flags_from_worker` and its doc (`:71-122`); `VibratoTokenizer::hints` (`:173-182`); the `reading_field` field of `VibratoTokenizer` and its initialisation in `load`, plus `READING_FIELD`, `READING_FIELD_ENV` and `reading_field_from_env` if nothing else uses them; and the `use crate::BoundaryHints` (or `crate::segment::BoundaryHints`) import.

- [ ] **I2.** Tests: delete `a_reading_field_the_dictionary_lacks_derives_no_flags_at_all`, `the_reading_field_override_falls_back_on_garbage` (if it exercised `reading_field_from_env`), `a_token_without_a_reading_marks_nothing`, the `flags_for` helper, and the whole 東京都 end-to-end test (`:545-611`) — it asserted v1's cost model. Keep every test of `boundaries()`, `BoundarySet` and `write_test_dictionary`.

- [ ] **I3.** Module doc (`:10-21`): delete the sentences about `BoundaryFlags` "staying on the v1 path", the `ta-old` reading-field guard, and `Dictionary.cpp`. State: "Vibrato's token boundaries as [`crate::engine::Boundaries`]: `VibratoTokenizer::boundaries` is the one production implementor."

#### J. The CLI

- [ ] **J1.** `bin/jparser-cli/main.rs` imports (`:19-31`): delete `ErrorKind`, `CommandFactory`, `ConjugationTable`, `index::build::build_from_reader`, `StemOptions`; change `jparser::record::WordFlags` to `jparser::WordFlags`. Delete the `EngineChoice` enum and its doc (`:54-62`) and `fn open_for` with its doc (`:237-255`).

- [ ] **J2.** `Command::BuildIndex` (`:66-80`): delete the `no_v5_fallback` and `engine` fields and their docs. Its arm (`:258-293`): delete the `ArgumentConflict` block, replace the `match engine` with `let report = jparser::engine::build::build_from_reader(file, &out)?;`, delete the three `stems` `println!`s.

- [ ] **J3.** `Command::EnsureDictionary` arm (`:294-320`): delete the `table`/`opts` lines; both `ensure_dictionary(&root, &table, &opts, keep, || …)` become `ensure_dictionary(&root, keep, || …)`.

- [ ] **J4.** `Command::Lookup` arm (`:375-407`): delete `let table = ...;`; `index.prefixes_of(&text)?` → `index.prefixes_of_with(&text, jparser::engine::kana::fold)?`; delete the `let verb = match r.verb_type {...};` block and the `type={verb:8}` segment of the `println!`, leaving `"    {:8} [{}] {glosses}"`.

- [ ] **J5.** `Command::Parse` (`:154-169`): delete the `engine` field. Its arm (`:409-476`): delete `let table = ConjugationTable::load_embedded()?;`, replace `open_for(engine, &index)?` with `Index::open(&index)?`, delete the v1 `flags`/`hints` block (the four `#[cfg]` lines producing `hints`), keep the `bound_set`/`bounds` block and its `#[cfg]`s, replace the `match engine` with

```rust
            let table = jparser::engine::table::Table::new();
            let result = jparser::parse(&index, &table, &text, &opts, bounds)?;
```

- [ ] **J6.** `Command::Score` (`:172-187`): delete the `engine` field; its arm passes no `engine`. `Command::Romaji` (`:229-234`, `:505-512`): delete the `particle` field and its doc; the arm becomes `println!("{}", jparser::engine::romaji::to_romaji(&text));`.

- [ ] **J7.** `bin/jparser-cli/score.rs`: delete `use crate::EngineChoice;` and `use jparser::conjugation::ConjugationTable;` (`:30-31`); `run`'s signature loses `engine: EngineChoice`; its body: `let table = jparser::engine::table::Table::new();` replaces both table lines, `let index = Index::open(index_dir)?;` replaces `crate::open_for(...)`, and the `match engine { … }` collapses to the v2 arm's body with `jparser::engine::parse_v2` → `jparser::parse`. Its doc comment's "Shared with `main.rs`'s `Parse` command (finding 3)…" paragraph goes. The test fixture at `:738-752` becomes `jparser::engine::build::build_from_reader(XML.as_bytes(), dir).expect("the fixture must build");` with the `table` line and the two extra arguments removed.

- [ ] **J8.** `bin/jparser-cli/diff.rs:27,192,207`: delete the import; `let table = jparser::engine::table::Table::new();`; `jparser::parse(&index, &table, &text, &ParseOptions::default(), None)?` (unchanged shape). `bin/jparser-cli/label.rs:30,79,86,132,270-285`: delete the import; `candidates`' parameter becomes `table: &jparser::engine::table::Table`; `run` builds `let table = jparser::engine::table::Table::new();`; the test fixture at `:277-285` becomes `jparser::engine::build::build_from_reader(XML.as_bytes(), &dir).expect("the fixture must build");`.

#### K. `src-tauri`

- [ ] **K1.** `src-tauri/src/state.rs`: imports (`:19`) — replace `ConjugationTable, ConjugationError` with `use jparser::engine::table::Table;`. `AppState.table: Table` (`:39`). Delete the `Conjugation(#[from] ConjugationError)` variant of `StartupError` (`:97-98`) and its arm in `is_missing_dictionary` (the `false` arm becomes `StartupError::Index(_) | StartupError::Hints(_) => false,`). `load_state` (`:127`): `let table = Table::new();`. In `every_startup_error_variant_renders_a_non_empty_string` (`:388-397`) delete the `StartupError::Conjugation(...)` entry.

- [ ] **K2.** `src-tauri/src/parse.rs`: import (`:14`) becomes `use jparser::{Boundaries, ParseOptions, ParseResult};`. The worker (`:111-118`) becomes

```rust
                let bounds = tokenizer.as_ref().map(|t| t.boundaries(&text));
                jparser::parse(
                    &state.index,
                    &state.table,
                    &text,
                    &ParseOptions::default(),
                    bounds.as_ref().map(|b| b as &dyn Boundaries),
                )
```

Tests: `:195-203` — delete `use jparser::conjugation::ConjugationTable;`, `table: jparser::engine::table::Table::new(),`; `:264` — `let table = jparser::engine::table::Table::new();`.

- [ ] **K3.** `src-tauri/src/commands.rs:1492-1518`: delete the `table` and `opts` lines; `ensure_dictionary(&root, jparser::index::generations::DEFAULT_KEEP_GENERATIONS, || {...})`; the closing `AppState { index, table }` becomes `AppState { index, table: jparser::engine::table::Table::new() }`. In `with_fallback`'s doc (`:1450-1451`) delete "a conjugation-table load failure,".

- [ ] **K4.** `src-tauri/src/test_support.rs`: imports (`:18-20`) become `use jparser::index::{ensure_dictionary, generations};`; `build_index_generation` (`:44-53`) drops the `table`/`opts` lines and calls `ensure_dictionary(root, 1, || Ok(FIXTURE_XML.as_bytes()))`. Module doc: delete the sentence naming `StemOptions`.

#### L. Tests and fixtures

- [ ] **L1.** `tests/fixtures/jmdict_mini.xml`: the fixture deliberately mis-tags 言う as `v5r` to exercise v1's mis-annotation fallback, which v2 does not have and should not. Change `<!ENTITY v5r "Godan verb with `ru' ending">` to `<!ENTITY v5u "Godan verb with `u' ending">` and `<pos>&v5r;</pos>` to `<pos>&v5u;</pos>`. In `tests/index_roundtrip.rs::retrieves_entry_data_by_id`, `vec!["v5r"]` becomes `vec!["v5u"]`.

- [ ] **L2.** The shared fixture-building pattern. In each of `tests/entry_id.rs`, `tests/cli_parse.rs`, `tests/cli_score.rs`, `tests/cli_label.rs`, `tests/index_roundtrip.rs`, `tests/generations_torn_build.rs`: delete the `use jparser::conjugation::ConjugationTable;` and `use jparser::stem::StemOptions;` lines; change `use jparser::index::build::build_from_reader;` to `use jparser::engine::build::build_from_reader;`; in the `index_dir`/`build`/`table_and_opts` helpers delete the `let table = …;` line and change `build_from_reader(std::io::Cursor::new(FIXTURE), &table, &StemOptions::default(), &dir)` to `build_from_reader(std::io::Cursor::new(FIXTURE), &dir)` (in `generations_torn_build.rs`, `build_from_reader(xml.as_bytes(), dir)` and `build_new(&root, XML_A.as_bytes())`, deleting `table_and_opts` entirely).

- [ ] **L3.** `tests/entry_id.rs`: `jparser::parse(&index, &table, "言った", …)` → `let table = jparser::engine::table::Table::new();` once per test in place of `ConjugationTable::load_embedded()`. Reword the first test's doc (`:39-41`): 言った matches 言う because the fixture tags it `v5u` and 言っ is that class's 連用タ接続; delete the "v5u mis-annotation fallback" clause.

- [ ] **L4.** `tests/cli_parse.rs`: delete `parse_engine_v2_rejects_a_v1_built_index` (`:98-121`) and its doc. Reword the two remaining tests' comments to v2 facts — 言った: "言う is `v5u`; 言っ is `五段・ワ行促音便`'s 連用タ接続 and た its past auxiliary; the reading strips う from いう and appends った"; はzz: "は is kana-only so `EntryData::readings` is empty and the reading is `-`; zz is two unmatched characters coalesced into one span". Run the file: the two `assert_eq!` strings are expected to hold unchanged (verified against v2 during planning: `言う (Past) [いった] to say; to utter` and the は/zz output are identical). If either differs, stop and report the actual output rather than editing the expectation blind.

- [ ] **L5.** `tests/cli_score.rs` and `tests/cli_label.rs`: only the L2 helper change. Run each; every assertion is expected to hold. If one does not, report the actual output.

- [ ] **L6.** `tests/index_roundtrip.rs`: import `use jparser::WordFlags;`; in `builds_and_reports_counts` delete `assert!(report.stems.exact_stems > 0);`; delete the tests `prefix_walk_returns_the_empty_stem_hit_for_an_irregular_verb` (`:122-175`, v1's generated stems) and `stems_carry_the_producing_verb_type_and_headwords_carry_none` (`:249-283`, the deleted field); every `index.prefixes_of(X)` becomes `index.prefixes_of_with(X, jparser::engine::kana::fold)`. `matches_katakana_text_against_a_hiragana_headword` is expected to still pass — `engine::kana::fold` unifies the scripts. Delete the `ta-old` comment at `:123` with its test.

- [ ] **L7.** `tests/engine_v2.rs`: `parse_v2_over`, `parse_v2_with_bounds` and every direct `jparser::engine::parse_v2(` call → `jparser::parse(`. `open_v2` (`:17-20`) becomes `jparser::index::load::Index::open(dir).expect("open v2 index")`. Replace `a_v2_index_opens_and_carries_no_stem_records` (`:37-49`) with

```rust
#[test]
fn a_v2_index_opens_under_the_engines_fingerprint() {
    let dir = scratch("build");
    build_v2(&dir);
    assert!(open_v2(&dir).entry_count() > 0);
}
```

and `the_two_engines_cannot_read_each_others_indexes` (`:51-87`) with

```rust
#[test]
fn an_index_built_under_another_fingerprint_is_rejected() {
    // The fingerprint guard, doing exactly what it exists for: a table
    // regeneration changes every class id an index stores, and `Index::open`
    // must refuse the stale index rather than misresolve classes silently.
    let dir = scratch("wrong-fingerprint");
    build_v2(&dir);
    let wrong = jparser::engine::build::fingerprint() ^ 1;
    let err = jparser::index::load::Index::open_with(&dir, wrong)
        .expect_err("a different fingerprint must not open this index");
    assert!(
        matches!(err, jparser::index::IndexError::ConjugationMismatch { .. }),
        "expected a fingerprint mismatch, got {err:?}"
    );
    open_v2(&dir);
}
```

Delete the now-unused `jparser::conjugation` / `jparser::stem` references (`:57-65`).

- [ ] **L8.** `examples/boundary_diag.rs:66,157`: `use jparser::engine::table::Table;` replaces the `ConjugationTable` import; `let table = Table::new();`.

#### M. Compile, gate, prove

- [ ] **M1.** `cargo check -p jparser --no-default-features`, then `cargo check -p jparser`, then `cargo check -p jparser --features mecab`, then `cargo check -p clipyomi`. Fix every error the compiler names — each will be a reference this checklist missed, and the fix is always the same shape (repoint to the v2 item or delete the v1-only use). Do not re-add any deleted item.

- [ ] **M2.** Run all six gates. Expected `jparser` counts: the Task 4 count, minus the deleted tests (`the_two_engines…` replaced 1:1, `a_v2_index_opens…` replaced 1:1, two `index_roundtrip` tests gone, `hints.rs`'s four gone, `lib.rs`'s whole test module gone, `cli_parse`'s rejection test gone), plus `flags.rs`'s three new. Record the exact numbers; a difference from your own arithmetic is a test you missed.

- [ ] **M3.** The CLI (release build):

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase1b-ii"
cargo run --release -p jparser --bin jparser-cli -- build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\gold"
cargo run --release -p jparser --bin jparser-cli -- parse "$scratch\gold" "読みました"
cargo run --release -p jparser --bin jparser-cli -- parse "$scratch\gold" "来た"
cargo run --release -p jparser --bin jparser-cli -- lookup "$scratch\gold" "東京"
```

Expected: no `--engine` flag exists; 読みました labels 読む `Polite Past`; 来た is one span, 来る, reading きた, `Past` (if the gold subset carries 来る; if not, say so and rely on Task 3's test); `lookup` prints records with no `type=` column.

- [ ] **M4.** Measure — same `score` commands as Task 2 Step 4 **without** `--engine v2`, against `$scratch\gold`. Expected: the eight cells equal Task 3's commit message exactly. The switchover changes no candidate and no cost; a difference is a bug.

- [ ] **M5.** The startup path, end to end. Build a v1-shaped stale generation the way the app will find one: any index whose header carries a different fingerprint. The simplest is the release CLI against a temp root: `ensure-dictionary "$scratch\app-root" crates/jparser/tests/fixtures/jmdict_mini.xml` publishes `gen-1`; then in a scratch Rust test or with a hex editor, flip one byte of `gen-1\header.bin`'s fingerprint field — **or** simpler and sufficient: run `cargo test -p clipyomi` and confirm `a_stale_index_is_the_download_screens_to_fix` passes (Task 1) together with `ensure_dictionary`'s own rebuild-on-mismatch tests in `index/mod.rs`. The two together are the path; record which you ran.

- [ ] **M6.** Commit — once, everything:

```powershell
git add -A crates/jparser/src crates/jparser/tests crates/jparser/examples src-tauri/src
git status --short   # must list only files this task named; the nine v1 files must show as untouched, still present
git commit -m "feat(engine-v2): make v2 the only engine" -m "jparser::parse is engine::parse; the v1 parse body, its helpers and its tests leave lib.rs and the nine ported files' mod declarations go with them, leaving those files on disk uncompiled for the next commit to delete. WordFlags is redefined in engine::flags from types.ts's eight wire names, bits arbitrary; StemStats, BuildReport.stems, StoredRecord.verb_type and VerbTypeId are gone and INDEX_FORMAT_VERSION is 4. Index::open checks engine::build::fingerprint; prefixes_of (v1's fold) is gone; ensure_dictionary and generations::build_new take no table or stem options. hints.rs keeps VibratoTokenizer::boundaries and loses BoundaryFlags, hints() and flags_from_worker -- the one function master plan 3.2 flagged as a documented port, dissolved as 4.3 promised. src-tauri passes boundaries() as a hard constraint and its AppState.table is engine::table::Table. The CLI loses --engine, EngineChoice, --no-v5-fallback and romaji --particle. jmdict_mini.xml's 言う is v5u, not the v5r that exercised v1's fallback. Test dispositions per design 6. score: <eight cells>, identical to the previous commit."
```

---

## Stage 3 — Delete

### Task 6: Remove the orphaned files

**Files:**
- Delete: `crates/jparser/src/{segment,matcher,stem,rank,record,kana,romaji,conjugation}.rs`, `crates/jparser/src/matcher/verb.rs`, `crates/jparser/assets/conjugations.json`, `xtask/` (whole directory)
- Modify: `Cargo.toml:2` (workspace members), `.gitignore:1`

- [ ] **Step 1: Prove they are orphans**

Run: `Get-ChildItem -Recurse crates/jparser/src, crates/jparser/tests, src-tauri/src -Include *.rs | Select-String "mod segment|mod matcher|mod stem|mod rank|mod record|mod kana;|mod romaji|mod conjugation|crate::segment|crate::matcher|crate::stem|crate::rank|crate::record|crate::kana::|crate::romaji|crate::conjugation|jparser::conjugation|jparser::stem|jparser::record|jparser::romaji|jparser::kana"`
Expected: no matches. (`crate::engine::kana` and `crate::engine::romaji` are different paths and do not match these patterns.) If anything matches, Task 5 missed it — fix it there first.

- [ ] **Step 2: Delete**

```powershell
git rm crates/jparser/src/segment.rs crates/jparser/src/matcher.rs crates/jparser/src/matcher/verb.rs crates/jparser/src/stem.rs crates/jparser/src/rank.rs crates/jparser/src/record.rs crates/jparser/src/kana.rs crates/jparser/src/romaji.rs crates/jparser/src/conjugation.rs
git rm crates/jparser/assets/conjugations.json
git rm -r xtask
```

In root `Cargo.toml`, `members` loses `"xtask"`. In `.gitignore`, delete line 1, `ta-old/`.

- [ ] **Step 3: Build everything**

Run: `cargo build --workspace` then all six gates. Expected: identical test counts to Task 5 — nothing compiled changed. `Cargo.lock` will drop `xtask`; include it in the commit.

- [ ] **Step 4: Commit**

```powershell
git add Cargo.toml Cargo.lock .gitignore
git commit -m "chore(engine-v2): delete the nine ported files, the converted asset and xtask" -m "Nothing referenced them after the previous commit -- the compiler is the proof, and every gate passes with identical counts. segment, matcher (and matcher/verb), stem, rank, record, kana, romaji and conjugation were master plan 3.2's hard-taint tier; assets/conjugations.json was the copied data; xtask's one subcommand converted it from the upstream tree, which .gitignore no longer needs to name. The tree is still labelled GPL-2.0-only for one more commit: the headers have not changed yet."
```

---

## Stage 4 — Relicense

### Task 7: Headers, strings, licence files

**Files:**
- Modify: 62 files carrying the GPL header (the 74 listed below minus the 12 deleted in Task 6), the `ta-old` sites listed below, `Cargo.toml`, `package.json`, `README.md`, `src-tauri/Cargo.toml`, `e2e/popover.spec.ts`, `crates/jparser/tests/fixtures/spurious_merges.txt`
- Create: `LICENSE-MIT`, `LICENSE-APACHE`
- Delete: `LICENSE`

- [ ] **Step 1: Replace the headers**

Three header shapes exist. Replace each with its SPDX form. The `//` forms are Rust; `tools/*.py` use `#`.

Shape (i) — the ported-crate header, on every file under `crates/jmdict-source`, `crates/vibrato-source`, `src-tauri`, `crates/jparser/src/{lib,hints,jmdict}.rs`, `crates/jparser/src/index/*.rs`, `crates/jparser/src/bin/jparser-cli/*.rs`, the remaining `crates/jparser/tests/{cli_generations,cli_hints,cli_parse,generations_torn_build,index_roundtrip}.rs`, and `tools/extract_jmdict_subset.py`:

```
// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
```
→
```
// SPDX-License-Identifier: MIT OR Apache-2.0
```

Shape (ii) — the Phase 1a / 1b-i header on `crates/jparser/src/engine/*.rs`, `tests/{engine_v2,conjugation_labels}.rs`, `tests/fixtures/conjugation_labels.txt` (`#` comments):

```
// ClipYomi — Engine v2, Phase 1a.          (or 1b-i)
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b, when the crate stops being GPL-2.0-only.
```
→
```
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.
```
(Keep any further provenance lines a file has after these, such as `label.rs`'s Wikipedia citation.)

Shape (iii) — the Phase 0 harness header on `tests/{entry_id,cli_score,cli_label}.rs` and `bin/jparser-cli/{corpus,diff,label,score}.rs`:

```
// ClipYomi — Engine v2 Phase 0 measurement harness.
// Copyright (C) 2026
//
// This program is free software; ... (three lines)
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.
```
→
```
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.
```

Do shape (i) with one PowerShell pass and verify, then (ii) and (iii) by hand (they vary):

```powershell
$files = Get-ChildItem -Recurse -Include *.rs,*.py crates, src-tauri, tools |
  Where-Object { (Get-Content $_.FullName -TotalCount 1) -match 'ported from Translation Aggregator' }
foreach ($f in $files) {
  $c = [IO.File]::ReadAllText($f.FullName)
  $marker = if ($f.Extension -eq '.py') { '#' } else { '//' }
  $pattern = "(?s)^(#!.*?\r?\n)?$([regex]::Escape($marker)) JParser — Japanese text parser ported from Translation Aggregator\.\r?\n$([regex]::Escape($marker)) Copyright \(C\) 2026\r?\n$([regex]::Escape($marker))\r?\n$([regex]::Escape($marker)) This program is free software.*?Free Software Foundation\.\r?\n"
  $new = [regex]::Replace($c, $pattern, "`$1$marker SPDX-License-Identifier: MIT OR Apache-2.0`n")
  if ($new -eq $c) { Write-Warning "no match: $($f.FullName)" }
  [IO.File]::WriteAllText($f.FullName, $new, (New-Object Text.UTF8Encoding $false))
}
```

Then: `Get-ChildItem -Recurse crates, src-tauri, tools, src, e2e -Include *.rs,*.ts,*.py,*.css,*.txt | Select-String "GNU General Public License"` must return nothing.

- [ ] **Step 2: Reword every surviving `ta-old` mention**

The done-when counts the literal string. These survive the deletions and must each be reworded to describe the behaviour without naming the source (behavioural description is permitted; the name is what the grep counts):

- `crates/jparser/src/hints.rs:19,21,71-86,422` — whatever Task 5's sub-section I left; the reading-field comments go with the deleted code.
- `crates/jparser/src/index/mod.rs:10,11,13,68,91,93` — Task 5 D4/D8 should have handled these; verify.
- `crates/jparser/src/jmdict.rs:10,11,12,46,49` — "Replaces ta-old's EDICT2 line parser (`CreateDict`, …)" → "Reads JMdict's structured XML: real `<pos>` tags instead of string surgery on English glosses." Lines 46/49 about the `(P)` marker: describe JMdict's priority tags on their own terms.
- `crates/jparser/tests/fixtures/spurious_merges.txt:35,39` — line 35's `task-15-report.md` citation (a gitignored file) becomes "see the Phase 1a handoff (`docs/superpowers/2026-09-01-engine-v2-phase1a-handoff.md`) §5 and §8 on why the third column is documentation rather than an assertion"; line 39's "or ta-old expression" → "or ported expression".
- `e2e/popover.spec.ts:72` — the test name `'renders every sense, coloured by ta-old rules'` → `'renders every sense, coloured by the tooltip rules'`.
- `src/main.ts:782,974`; `src/render/popover.ts:1,82`; `src/render/popover.test.ts:16,70`; `src/render/tooltip-colour.ts:12,17,36`; `src/render/tooltip-colour.test.ts:35`; `src/render/tooltip-text.ts:6,25,58`; `src/render/tooltip-text.test.ts:20,28,60`; `src/render/tooltip.ts:6`; `src/styles/tooltip.css:5,21`; `src/surface-alpha.ts:6`; `src/surface-alpha.test.ts:24` — each names the upstream as the source of a UI convention. Reword to "the original tooltip's rule" / "the upstream tooltip" or simply state the rule. Worked examples: `popover.ts:1` `/** Distance between the word and the tooltip, in px. ta-old's `rAvoid.top-2`. */` → `/** Distance between the word and the tooltip, in px: the original tooltip kept two. */`; `tooltip-text.ts:25` `// Glosses join with "/" — ta-old's separator. The pane uses "; "` → `// Glosses join with "/", the tooltip's separator; the pane uses "; "`; `clipboard.rs:33` `/// Kana or a CJK ideograph — ta-old's test for "is this worth parsing".` → `/// Kana or a CJK ideograph: the test for "is this worth parsing".`
- `src-tauri/src/clipboard.rs:33,208`; `src-tauri/src/popover.rs:17` — same treatment.

Verify: `git grep -n "ta-old"` returns **nothing**. (`autoresearch/` and `.superpowers/` are gitignored and do not count; `docs/` is intentionally historical and is excluded from this grep by the design — confirm with `git grep -n "ta-old" -- ':!docs'` returning nothing.)

Verify: `git grep -n "ported from Translation Aggregator"` returns nothing.

- [ ] **Step 3: Licence files and metadata**

Root `Cargo.toml:7`: `license = "MIT OR Apache-2.0"`.

`package.json`: after the `"name"` line add `"license": "MIT OR Apache-2.0",`.

`git rm LICENSE`. Create `LICENSE-MIT`:

```
MIT License

Copyright (c) 2026 the ClipYomi contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Create `LICENSE-APACHE` from the canonical text — do not retype it:

```powershell
Invoke-WebRequest -Uri https://www.apache.org/licenses/LICENSE-2.0.txt -OutFile LICENSE-APACHE
(Get-Content LICENSE-APACHE | Measure-Object -Line).Lines   # 202
Get-Content LICENSE-APACHE -TotalCount 3                      # "Apache License / Version 2.0, January 2004 / http://www.apache.org/licenses/"
```

`README.md:99-103`: rewrite the licence section — "ClipYomi is dual-licensed under **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`). The parsing engine is a clean-room implementation; see `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` §3.5. Dictionary data from the EDRDG's JMdict is CC BY-SA 4.0 under its own notice (`crates/jparser/tests/fixtures/README.md`); the IPADIC conjugation data carries its notice in `crates/jparser/assets/IPADIC_NOTICE`."

`src-tauri/Cargo.toml:23-24`: the comment "which this self-distributed GPL v2 build already accepts" → "which this self-distributed build already accepts".

- [ ] **Step 4: The dependency licence check**

Run: `cargo tree --workspace --format "{p} {l}" -e normal | Select-String -NotMatch "MIT|Apache|BSD|ISC|Zlib|Unlicense|MPL|CC0|Unicode" | Sort-Object -Unique`
Expected: nothing GPL-family. Record the command and result for the handoff. (This is a one-time read, not a new gate.)

- [ ] **Step 5: Gates, then commit**

All six gates (headers are comments; counts unchanged). Then:

```powershell
git add -A
git commit -m "chore: relicense the tree as MIT OR Apache-2.0" -m "Every GPL-2.0 header becomes the SPDX line master plan 9.2 specified, with the clean-room provenance lines kept on the engine files. Every 'ta-old' mention and every 'ported from Translation Aggregator' header is gone tree-wide -- git grep for either string returns nothing -- with behavioural comments in the frontend and shell reworded to state the rule without naming the source. LICENSE (GPL-2) is replaced by LICENSE-MIT and LICENSE-APACHE; Cargo.toml's workspace licence and package.json follow. JMdict fixtures (CC BY-SA 4.0), IPADIC_NOTICE and the Wikinews corpus keep their own notices unchanged. The dependency graph carries no GPL-family crate."
```

---

### Task 8: Format once; close the phase in the documents

**Files:**
- Modify: every file `cargo fmt` touches under `crates/jparser`
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` (§3.2, §4.3, §7 Phase 1, §7 Phase 1b, §9.2, §10)
- Create: `docs/superpowers/2026-09-02-engine-v2-phase1b-ii-handoff.md`

- [ ] **Step 1: Format**

Run: `cargo fmt -p jparser` then `cargo fmt -p jparser -- --check` (clean). Also run `cargo fmt -p clipyomi -- --check`; if it reports sites, run `cargo fmt -p clipyomi` too and say so in the commit. Run all six gates — formatting must change no behaviour.

- [ ] **Step 2: Master plan**

- §7 Phase 1b: mark Phase 1b-ii complete with one paragraph: the four stages, the startup hazard §3 found, the two table fixes and their measured cells.
- §7 Phase 1's done-when: replace the last two sentences ("Both Phase 0 metrics are at or above baseline…; くるまではこう parses…") with: "**Relicence clause met** (Phase 1b-ii): both strings grep to zero tree-wide. **Metric clause superseded**, not met: the tolerant gap is the structural JMdict-priority ceiling §7 Phase 3 already assigns to the frequency layer, and no Phase 1 tool reaches it. In the shipped configuration (hints on) v2 is +10.7 points strict, −1.6 tolerant, −0.9 entry top-1 against v1 — strict measures the particle-absorbing defect this project exists to fix; tolerant measures compound preservation, which Phase 3 owns. くるまではこう parses as くる / まで / は / こう."
- §3.2: add a line after the table: "**Discharged at Phase 1b-ii.** The hard tier is deleted; the soft tier's citations are scrubbed (§9.3's assumption held); the clean tier's header strings are gone."
- §4.3: after the `flags_from_worker` paragraph: "Dissolved at Phase 1b-ii: deleted with `BoundaryFlags`, no successor."
- §9.2: after the header block: "**Written at Phase 1b-ii.** The tree is `MIT OR Apache-2.0`."
- §10: append rows for the four-stage sequencing and its re-seating, the startup fix, the format bump, `flags_from_worker`'s deletion, `vk`'s second class with the reading rule, the たい reorder, the `romaji --particle` removal, and the metric-clause amendment — each one line, rationale pointing at the 1b-ii design section.

- [ ] **Step 3: The handoff**

Create `docs/superpowers/2026-09-02-engine-v2-phase1b-ii-handoff.md` in the same shape as the 1b-i handoff, with these sections and real numbers from this session:

1. **What exists now** — one engine; the public API (`parse`, `WordFlags`, `Boundaries`, `Table::new`); file counts and gate results at the final commit for both crates.
2. **Design §10's done-when, one at a time** — the two greps (zero), the GPL grep (only `LICENSE-APACHE`), the six gates, the startup path (which check you ran in Task 5 M5), the eight `score` cells after Task 3 beside 1b-i's, 読みました and 来た from the CLI.
3. **The two table fixes** — Task 2's flipped-label count and unchanged cells; Task 3's before/after cells and the 来る reading rule, with the principled fixes named and left (`たい:ガル接続` non-terminal; the godan 仮定形/命令ｅ tie).
4. **What the switchover dropped** — `romaji --particle`, `--engine`, `--no-v5-fallback`, `lookup`'s `type=` column, `prefixes_of`; `jmdict_mini.xml`'s corrected tag.
5. **What Phase 2 inherits** — the metric clause as amended; Phase 1a handoff §8's still-open items (`is_allowed` softening, `assemble::order`, the 18 shattered spans); the `ATTACHES_TO` findings from the 1b-i handoff §4 not taken here; the copyright-holder line as a default the maintainer may change.

- [ ] **Step 4: Sweep for stale statements**

Use the Grep tool over `crates/jparser/src`, `crates/jparser/tests`, `src-tauri/src` for: `Phase 1b`, `v1 path`, `the two engines`, `--engine`, `EngineChoice`, `ConjugationTable`, `conjugations.json`, `verb_type`. Every hit must be either historical-and-true or reworded. `docs/` hits stay.

- [ ] **Step 5: Gates, then commit**

```powershell
git add -A
git commit -m "docs(engine-v2): close Phase 1b-ii; format the crate once" -m "Master plan section 7 records Phase 1b-ii complete and Phase 1's metric clause as superseded by the Phase 3 frequency-layer finding, with the hints-on numbers beside it; 3.2's taint map is discharged and 4.3's flags_from_worker dissolved. The handoff carries the measured cells for both table fixes, the startup-path check, and what Phase 2 inherits. cargo fmt ran once over jparser (<N> sites), the first time the clean-room quarantine allowed it."
```

---

## Self-review against the spec

- **§1 four stages, licence last** — Tasks 1–4 (stage 1), Task 5 (stage 2, one commit, M6), Task 6 (stage 3), Tasks 7–8 (stage 4). Deletion (6) precedes the flip (7). ✓
- **§2 relocations** — `WordFlags` A1–A3 with the eight names from `types.ts`; `StemStats`/`stems` D5/H1/J2; `verb_type`/`VerbTypeId` D1–D3/H1/L6/J4; format 4 D2; `Index::open` F1; `prefixes_of` F2/J4/L6; fingerprint pin Task 4. ✓
- **§3 lifecycle and startup** — D6/G1/K3/K4; startup fix Task 1; `AppState.table` K1. ✓
- **§4 API, app, CLI** — B1/C1 (`parse`), `Boundaries` re-export B1, K2 (`boundaries()`), J1–J8 including `romaji --particle`, L8 for the example. ✓
- **§5 hints** — I1–I3. ✓
- **§6 tests** — `parse_snapshots` Task 4; `parse_irregular` Task 3 (ported, deleted); `entry_id` L3; CLI tests L4–L5; index tests L6/L2; `engine_v2` L7; `boundary_diag` L8; `hints.rs`/`matcher.rs` tests I2 and Task 6. ✓
- **§7 deletions** — Task 6, plus `lib.rs` B4 and `hints.rs` I1. ✓
- **§8 provenance and licence** — Task 7 Steps 1–4; `cargo fmt` Task 8 Step 1. ✓
- **§9.1 vk + reading rule** — Task 3. **§9.2 たい** — Task 2. Both measured. ✓
- **§10 done-when** — greps and licence grep Task 7 Step 2/1; gates every task; startup path M5; score M4; 読みました/来た M3. ✓
- **§11 master plan edits** — Task 8 Step 2. ✓

**Placeholder scan:** `<N>`, `<I>`, `<K>`, `<eight cells>` in commit messages are values the task itself produces and records — not plan placeholders. `LICENSE-APACHE` is fetched, not retyped. Task 4's `0` is replaced by a measured value in the same task.

**Type consistency:** `classes_for_pos(&str) -> impl Iterator<Item = ClassId> + '_` (Task 3) is what Task 5 leaves untouched; `WordFlags(pub u16)` with `contains`/`insert`/`names` (A1) is what `main.rs`'s `lookup` (`WordFlags(r.flags)`, `.contains`) and `lex.rs:141` (`WordFlags(record.flags)`) already use; `ensure_dictionary(root, keep, source)` (D6) matches every caller in D7/J3/K3/K4; `build_new(root, xml)` (G1) matches D6/D7/G2/L2; `parse(index, &Table, text, &ParseOptions, Option<&dyn Boundaries>)` (C1) matches J5/J7/J8/K2/L3/L7/L8.
