# Katakana Run Integrity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the segmenter cutting a katakana run into pieces that are not words — `ユリナス` (a character's name) currently reads `ユリ` ("lily") + `ナス` ("eggplant") 1,721 times in six corpora.

**Architecture:** One new geometric property on `Candidate` (`inside_katakana_run`: the span is all katakana and a longer katakana run encloses it) set in `engine/lex.rs`, and one flat charge in `engine/cost.rs` applied when that property holds and the span is shorter than the existing `KATAKANA_MIN_PIECE`. A charge rather than a constraint, because the alternative to splitting is leaving the run unmatched at `UNMATCHED` (30) **per character**, so a long run out-argues the charge and keeps its cut while a short one does not. The magnitude is swept against a zero-gold-damage gate, not chosen.

**Tech Stack:** Rust (workspace crate `jparser`, feature `mecab` for the hint-dictionary paths), `cargo test`, `jparser-cli score`, `jparser-cli bench`, Python 3 for the corpus measurement scripts (this repo has no Python dependency; the scripts are throwaway probes run from a scratch directory).

**Spec:** [`docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md`](../specs/2026-09-21-katakana-run-integrity-design.md)

## Global Constraints

- **Nothing outside `crates/jparser/src/engine/` changes**, except the test file `crates/jparser/tests/engine_v2.rs` and the documentation this plan names. No index format change, no wire-shape change, no setting, no stylesheet, no frontend file.
- **Zero gold damage in either hint state.** No cell may fall below the baseline at `f750bd1`: hints-on strict 1694, tolerant 1948, entry top-1 0.9527, compounds 152; hints-off strict 1588, tolerant 1785, entry top-1 0.9504, compounds 118.
- **`crates/jparser/tests/fixtures/parse_sentences.txt` stays byte-identical.** Its only katakana runs of three or more are `コーヒー` and `パソコン`, both already matched whole in both states.
- **`spurious_merges.txt` 26 of 26 and every row of `auxiliary_merges.txt` keep passing.**
- **On this shell, prefix any command whose output contains Japanese with `PYTHONIOENCODING=utf-8`** when it is Python; Rust binaries need no prefix. Use forward slashes in paths; the Bash tool is Git Bash.
- **Katakana means `kana::is_katakana`** — `is_kana(c) && !is_hiragana(c)`, which is U+30A1..=U+30FA, the prolonged sound mark U+30FC, and halfwidth katakana U+FF66..=U+FF9D. One definition, used by the run scan and by the existing `all_katakana` field.
- **The corpora are local probe input** at `D:\Code\oss\vn-text` and nothing derived from them may be committed. Measurement scripts go in a scratch directory, not the repo.

---

### Task 1: The geometry — `inside_katakana_run`

Adds the property and nothing that reads it, so this task cannot change a single parse. That is the point: it lands and is reviewable on its own, and Task 2's charge is then a one-line consumer.

**Files:**
- Modify: `crates/jparser/src/engine/kana.rs` (add `is_katakana`, after `is_hiragana` at line 88)
- Modify: `crates/jparser/src/engine/lex.rs` (new `Candidate` field; `katakana_runs` helper; set the field where the candidate is built around line 205; reuse the helper at the `all_katakana` site)
- Test: `crates/jparser/src/engine/lex.rs` (its own `#[cfg(test)] mod tests`) and `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Consumes: `kana::is_kana`, `kana::is_hiragana` (existing).
- Produces: `kana::is_katakana(c: char) -> bool`; `Candidate.inside_katakana_run: bool`; private `fn katakana_runs(chars: &[char]) -> Vec<Option<(usize, usize)>>` in `lex.rs`, one entry per character holding the maximal katakana run `(start, end)` containing it, `None` for a non-katakana character.

- [ ] **Step 1: Write the failing unit test for the run table**

Add to the existing `#[cfg(test)] mod tests` at the bottom of `crates/jparser/src/engine/lex.rs`:

```rust
    #[test]
    fn a_katakana_run_is_the_maximal_span_around_each_of_its_characters() {
        let chars: Vec<char> = "をユリナスが".chars().collect();
        let runs = super::katakana_runs(&chars);
        assert_eq!(runs[0], None, "を is not katakana");
        assert_eq!(runs[1], Some((1, 5)), "ユ opens the run");
        assert_eq!(runs[4], Some((1, 5)), "ス closes it");
        assert_eq!(runs[5], None, "が is not katakana");
    }

    #[test]
    fn the_prolonged_sound_mark_stays_inside_the_run() {
        let chars: Vec<char> = "コーヒー".chars().collect();
        let runs = super::katakana_runs(&chars);
        assert_eq!(runs[1], Some((0, 4)), "ー does not break the run");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p jparser --features mecab --lib katakana_run`
Expected: FAIL to compile — `cannot find function 'katakana_runs' in module 'super'`.

- [ ] **Step 3: Add the katakana predicate**

In `crates/jparser/src/engine/kana.rs`, immediately after `is_hiragana`:

```rust
/// Katakana (U+30A1..=U+30FA), the prolonged sound mark (U+30FC) and
/// halfwidth katakana (U+FF66..=U+FF9D): the katakana leg of [`is_kana`] on
/// its own, and the definition `lex`'s run scan and `all_katakana` share.
pub fn is_katakana(c: char) -> bool {
    is_kana(c) && !is_hiragana(c)
}
```

- [ ] **Step 4: Add the run table to `lex.rs`**

Add this function to `crates/jparser/src/engine/lex.rs`, above `fn candidates`:

```rust
/// The maximal katakana run containing each character, or `None` where the
/// character is not katakana.
///
/// Computed once per sentence so the per-candidate test in [`candidates`] is
/// O(1) rather than O(len): `jparser-cli bench`'s scaling cell is what a
/// per-candidate scan would show up in, and Phase 2c exists because an
/// earlier version of this module was superlinear in input length.
fn katakana_runs(chars: &[char]) -> Vec<Option<(usize, usize)>> {
    let mut runs = vec![None; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if !kana::is_katakana(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && kana::is_katakana(chars[i]) {
            i += 1;
        }
        for slot in &mut runs[start..i] {
            *slot = Some((start, i));
        }
    }
    runs
}
```

- [ ] **Step 5: Run the unit tests to verify they pass**

Run: `cargo test -p jparser --features mecab --lib katakana_run`
Expected: PASS, 2 tests.

- [ ] **Step 6: Write the failing integration test for the flag**

Add to `crates/jparser/tests/engine_v2.rs`, directly after the `kuru_*` block that ends with `kuru_in_the_old_kanji_still_deinflects`:

```rust
// A katakana run cut into pieces that are not words: ユリナス, a character's
// name, reads ユリ ("lily") + ナス ("eggplant") 1,721 times over six corpora.
// See docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md.
//
// A private fixture for the reason `KURU_FIXTURE` is one: the extractor that
// regenerates `jmdict_subset.xml` would drop hand-added entries.
const KATAKANA_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE JMdict [
<!ENTITY n "noun (common) (futsuumeishi)">
]>
<JMdict>
<entry>
<ent_seq>1000010</ent_seq>
<k_ele><keb>ユリ</keb></k_ele>
<r_ele><reb>ゆり</reb></r_ele>
<sense><pos>&n;</pos><gloss>lily</gloss></sense>
</entry>
<entry>
<ent_seq>1000020</ent_seq>
<k_ele><keb>ナス</keb></k_ele>
<r_ele><reb>なす</reb></r_ele>
<sense><pos>&n;</pos><gloss>eggplant</gloss></sense>
</entry>
<entry>
<ent_seq>1000030</ent_seq>
<r_ele><reb>ソロ</reb></r_ele>
<sense><pos>&n;</pos><gloss>solo</gloss></sense>
</entry>
<entry>
<ent_seq>1000040</ent_seq>
<r_ele><reb>ホームラン</reb></r_ele>
<sense><pos>&n;</pos><gloss>home run</gloss></sense>
</entry>
<entry>
<ent_seq>1000050</ent_seq>
<r_ele><reb>ラスト</reb></r_ele>
<sense><pos>&n;</pos><gloss>last</gloss></sense>
</entry>
<entry>
<ent_seq>1000060</ent_seq>
<r_ele><reb>ラン</reb></r_ele>
<sense><pos>&n;</pos><gloss>run</gloss></sense>
</entry>
<entry>
<ent_seq>1000070</ent_seq>
<r_ele><reb>ランニング</reb></r_ele>
<sense><pos>&n;</pos><gloss>running</gloss></sense>
</entry>
<entry>
<ent_seq>1000080</ent_seq>
<r_ele><reb>ウェア</reb></r_ele>
<sense><pos>&n;</pos><gloss>wear</gloss></sense>
</entry>
<entry>
<ent_seq>1000090</ent_seq>
<k_ele><keb>キツい</keb></k_ele>
<r_ele><reb>きつい</reb></r_ele>
<sense><pos>&n;</pos><gloss>tight</gloss></sense>
</entry>
</JMdict>
"#;

fn build_katakana(dir: &Path) {
    jparser::engine::build::build_from_reader(BufReader::new(KATAKANA_FIXTURE.as_bytes()), dir)
        .expect("v2 build");
}

/// The geometry the charge keys on, asserted on the flag rather than on a
/// parse: a two-character piece of a four-character run is a fragment, the
/// whole run is not, and a span that leaves the run is not either.
#[test]
fn a_piece_of_a_longer_katakana_run_is_marked_as_one() {
    let dir = scratch("katakana-fragment-flag");
    build_katakana(&dir);
    let c = cands(&dir, "ユリナス");
    let piece = c[0]
        .iter()
        .find(|x| x.len == 2 && x.headword == "ユリ")
        .expect("ユリ is a candidate at 0");
    assert!(piece.inside_katakana_run, "ユリ is a piece of ユリナス");

    let tail = c[2]
        .iter()
        .find(|x| x.len == 2 && x.headword == "ナス")
        .expect("ナス is a candidate at 2");
    assert!(tail.inside_katakana_run, "ナス is a piece of ユリナス");
}

/// The converse, and the case a charge must never reach: a span that starts
/// inside a katakana run and ends outside it is not a piece of that run.
#[test]
fn a_span_that_leaves_the_katakana_run_is_not_a_piece_of_it() {
    let dir = scratch("katakana-fragment-crossing");
    build_katakana(&dir);
    let c = cands(&dir, "キツい");
    let crossing = c[0]
        .iter()
        .find(|x| x.len == 3)
        .expect("キツい is a candidate at 0");
    assert!(
        !crossing.inside_katakana_run,
        "キツい ends in hiragana, so it is not inside a katakana run"
    );
}
```

- [ ] **Step 7: Run the integration tests to verify they fail**

Run: `cargo test -p jparser --features mecab --test engine_v2 katakana`
Expected: FAIL to compile — `no field 'inside_katakana_run' on type 'Candidate'`.

- [ ] **Step 8: Add the field and set it**

In `crates/jparser/src/engine/lex.rs`, add to `pub struct Candidate` immediately after the `all_katakana` field and its doc comment:

```rust
    /// Every character of the span is katakana **and** a longer katakana run
    /// encloses it — the span is a proper piece of that run, on at least one
    /// side.
    ///
    /// Geometry only: how short a piece has to be before it is charged is
    /// `cost::KATAKANA_MIN_PIECE`'s business, so the threshold lives there
    /// with the constant it shares. `ユリナス` tiles into two such pieces and
    /// reads "lily eggplant"; `ソロ|ホームラン` tiles into one and is right.
    pub inside_katakana_run: bool,
```

Inside `fn candidates`, before the `for start in 0..len` loop, add:

```rust
    let runs = katakana_runs(chars);
```

In the `Candidate { .. }` literal inside the `for record in &records` loop, add the field beside `all_katakana`:

```rust
                            inside_katakana_run: match runs[start] {
                                // `end <= run_end` puts the whole span in the
                                // same run: runs are maximal and contiguous,
                                // so a span that starts in one and ends
                                // within its end never leaves it.
                                Some((run_start, run_end)) if end <= run_end => {
                                    run_start < start || run_end > end
                                }
                                _ => false,
                            },
```

Then replace the `all_katakana` expression with the shared predicate, which is the same test spelled once:

```rust
                            all_katakana: chars[start..end].iter().all(|&c| kana::is_katakana(c)),
```

- [ ] **Step 9: Fix every other construction site the new field breaks**

Run: `cargo test --workspace --features mecab 2>&1 | grep -E "missing field|error\[" | head -20`
Expected: a list of test helpers that build a `Candidate` literal — at least `cost.rs`'s `fn cand` and `engine_v2.rs`'s literal in `a_noncounter_after_a_number_only_loses_to_a_merge_that_exists`. Add `inside_katakana_run: false,` to each, beside the existing `all_katakana: false,`.

- [ ] **Step 10: Run the full suite to verify nothing moved**

Run: `cargo test --workspace --features mecab 2>&1 | grep -E "^test result" | awk -F'[ .;]+' '{p+=$4; f+=$6} END {print "passed", p, "failed", f}'`
Expected: `passed 670 failed 0` — the 666 at `f750bd1` plus this task's four. No existing test may change, because nothing reads the field yet.

- [ ] **Step 11: Commit**

```bash
cargo fmt --all
git add crates/jparser/src/engine/kana.rs crates/jparser/src/engine/lex.rs crates/jparser/src/engine/cost.rs crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine): mark a candidate that is a piece of a longer katakana run

Geometry only, read by nothing yet. A span every character of which is
katakana and which a longer katakana run encloses is a piece of that run;
the run table is built once per sentence so the per-candidate test is O(1).
kana::is_katakana is the one definition, shared with all_katakana."
```

---

### Task 2: The charge

**Files:**
- Modify: `crates/jparser/src/engine/cost.rs` (new constant beside `KATAKANA_MIN_PIECE` at line 569; one `if` in `cost()` around line 654, beside `digit_counter`)
- Test: `crates/jparser/src/engine/cost.rs` (its own `#[cfg(test)] mod tests`), `crates/jparser/tests/engine_v2.rs`

**Interfaces:**
- Consumes: `Candidate.inside_katakana_run` (Task 1), `KATAKANA_MIN_PIECE` (existing, 3).
- Produces: `const KATAKANA_FRAGMENT: i32 = 60;` — an opening value inside the window the spec derives, **swept to its final value in Task 3**.

- [ ] **Step 1: Write the failing unit test for the term**

Add to `#[cfg(test)] mod tests` in `crates/jparser/src/engine/cost.rs`. The existing helper is `fn cand(start, len, flags, exact)`; build on it:

```rust
    #[test]
    fn a_short_piece_of_a_katakana_run_is_charged_and_a_long_one_is_not() {
        let plain = cand(0, 2, WordFlags::PRIMARY, true);
        let mut fragment = cand(0, 2, WordFlags::PRIMARY, true);
        fragment.inside_katakana_run = true;
        assert_eq!(
            cost(&fragment, None) - cost(&plain, None),
            KATAKANA_FRAGMENT,
            "a two-character piece pays the charge"
        );

        let long_plain = cand(0, 3, WordFlags::PRIMARY, true);
        let mut long_piece = cand(0, 3, WordFlags::PRIMARY, true);
        long_piece.inside_katakana_run = true;
        assert_eq!(
            cost(&long_piece, None),
            cost(&long_plain, None),
            "three characters is KATAKANA_MIN_PIECE, so it is a word-sized piece"
        );
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p jparser --features mecab --lib a_short_piece_of_a_katakana_run`
Expected: FAIL to compile — `cannot find value 'KATAKANA_FRAGMENT' in this scope`.

- [ ] **Step 3: Add the constant**

In `crates/jparser/src/engine/cost.rs`, immediately after `KATAKANA_MIN_PIECE`:

```rust
/// What a span pays for being a piece shorter than [`KATAKANA_MIN_PIECE`] of
/// a longer katakana run (`lex::Candidate::inside_katakana_run`).
///
/// **The defect.** A run is cut wherever its pieces happen to be dictionary
/// words, whether or not the run is one: `ユリナス`, a character's name,
/// reads `ユリ` ("lily") + `ナス` ("eggplant") 1,721 times over six corpora
/// and `アジュラ` reads `ア` + `ジュ` + `ラ` 663 times hints-off. 2,320 runs
/// of three or more characters are split hints-on and 8,050 hints-off, and
/// the shortest piece is under three in 87% and 94% of them.
///
/// **Why a charge and not a rule.** Gold cuts five of its 58 katakana runs
/// and two of those leave a short piece — `ソロ` in ソロホームラン, `ラン` in
/// ラストラン — both of which the engine gets right today. Forbidding a short
/// piece deletes those tilings and costs four gold spans. A charge does not,
/// because the alternative to a split is leaving the run unmatched at
/// [`UNMATCHED`] **per character**: the whole-run option costs 120 for a
/// four-character name and 210 for ソロホームラン, so the same flat charge
/// flips the first and not the second.
///
/// **Magnitude.** Swept against a zero-gold-damage gate; see the plan's Task
/// 3 and the design's §4. The value shipped is the lowest that recovers the
/// hints-off katakana spans without moving a gold cell.
const KATAKANA_FRAGMENT: i32 = 60;
```

- [ ] **Step 4: Apply it in `cost()`**

In `pub fn cost`, immediately after the `digit_counter` / `after_digit_noncounter` block and before `if let Some(bounds) = bounds {`:

```rust
    if cand.inside_katakana_run && cand.len < KATAKANA_MIN_PIECE {
        total += KATAKANA_FRAGMENT;
    }
```

- [ ] **Step 5: Run the unit test to verify it passes**

Run: `cargo test -p jparser --features mecab --lib a_short_piece_of_a_katakana_run`
Expected: PASS.

- [ ] **Step 6: Write the failing integration tests — the three parses that define the change**

Append to `crates/jparser/tests/engine_v2.rs`, after Task 1's two tests:

```rust
/// The defect. No entry covers ユリナス, so the run comes out as one
/// unmatched span — which is the honest output for a name the dictionary does
/// not have, and still romanises, because `src/render/furigana.ts` falls back
/// to the surface when a segment has no reading.
#[test]
fn a_katakana_name_is_one_run_not_two_words() {
    let dir = scratch("katakana-name");
    build_katakana(&dir);
    let r = parse_v2_over(&dir, "ユリナス");
    assert_eq!(r.segments.len(), 1, "{:?}", r.segments);
    assert_eq!(r.segments[0].surface, "ユリナス");
    assert!(
        !r.segments[0].matched,
        "no entry covers it; unglossed is the honest output"
    );
}

/// The long-run case the charge must not reach, and the reason this is a
/// charge rather than a rule: gold cuts ソロホームラン at a two-character
/// piece and the engine agrees today.
#[test]
fn a_long_katakana_compound_keeps_its_cut_at_a_short_piece() {
    let dir = scratch("katakana-solo-homerun");
    build_katakana(&dir);
    let r = parse_v2_over(&dir, "ソロホームラン");
    assert_eq!(r.segments.len(), 2, "{:?}", r.segments);
    assert_eq!(r.segments[0].surface, "ソロ");
    assert_eq!(r.segments[1].surface, "ホームラン");
}

/// The window's other end, also a gold cut: ラスト + ラン, five characters.
#[test]
fn a_five_character_compound_keeps_its_cut_at_a_short_piece() {
    let dir = scratch("katakana-last-run");
    build_katakana(&dir);
    let r = parse_v2_over(&dir, "ラストラン");
    assert_eq!(r.segments.len(), 2, "{:?}", r.segments);
    assert_eq!(r.segments[0].surface, "ラスト");
    assert_eq!(r.segments[1].surface, "ラン");
}

/// A compound whose pieces are both word-sized is never charged at all, so it
/// is untouched by any magnitude.
#[test]
fn a_compound_of_word_sized_pieces_is_not_charged() {
    let dir = scratch("katakana-running-wear");
    build_katakana(&dir);
    let c = cands(&dir, "ランニングウェア");
    for x in c[0].iter().chain(c[5].iter()) {
        assert!(
            !(x.inside_katakana_run && x.len < 3),
            "{:?} is word-sized and must not be a charged fragment",
            x.headword
        );
    }
    let r = parse_v2_over(&dir, "ランニングウェア");
    assert_eq!(r.segments.len(), 2, "{:?}", r.segments);
}
```

- [ ] **Step 7: Run them to verify the first fails and the rest pass**

Run: `cargo test -p jparser --features mecab --test engine_v2 katakana`
Expected before Step 4 is in place: `a_katakana_name_is_one_run_not_two_words` FAILS with two segments. With Step 4 in place it PASSES and the other three also pass. If `a_long_katakana_compound_keeps_its_cut_at_a_short_piece` or `a_five_character_compound_keeps_its_cut_at_a_short_piece` fails, the magnitude is already too high — record the value and drop it to 50 before continuing, and say so in Task 3's table.

- [ ] **Step 8: Run the full suite and the two fixture gates**

Run: `cargo test --workspace --features mecab 2>&1 | grep -E "^test result|FAILED|panicked" | head -20`
Expected: no `FAILED`. In particular `spurious_merges` and `auxiliary_merges` pass and the `parse_sentences` snapshot is unchanged — `コーヒー` and `パソコン` are whole runs, never pieces, so nothing there can be charged.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all
git add crates/jparser/src/engine/cost.rs crates/jparser/tests/engine_v2.rs
git commit -m "feat(engine): charge a short piece of a katakana run

ユリナス read ユリ (lily) + ナス (eggplant), a character's name as two
vegetables, 1,721 times over six corpora. A piece shorter than
KATAKANA_MIN_PIECE inside a longer katakana run now pays a flat charge, so a
run whose only tilings are short pieces loses to leaving the run whole.

A charge, not a rule: gold cuts ソロホームラン and ラストラン at a short piece
and both are right, and an unmatched run costs UNMATCHED per character, so a
long run still out-argues the charge. Magnitude is provisional at 60 and is
swept next."
```

---

### Task 3: Sweep the magnitude

The value from Task 2 is a guess inside a window derived from approximate costs. This task measures it. Nothing here is TDD — it is a measurement whose output is a number and a table, and it ends by pinning the number.

**Files:**
- Modify: `crates/jparser/src/engine/cost.rs` (the `KATAKANA_FRAGMENT` value and the `Magnitude` paragraph of its doc)
- Modify: `docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md` (append a "Measured" section, the way `2026-09-15-engine-v2-phase4-design.md` §8 and §9 do)

**Interfaces:**
- Consumes: `KATAKANA_FRAGMENT` (Task 2), the tests of Tasks 1 and 2.
- Produces: the shipped magnitude, and a table of what each swept value did.

- [ ] **Step 1: Build the measurement inputs**

The corpora are outside the repository and the scripts are throwaway. Create a scratch directory and write the two scripts below into it.

```bash
SCR="$(mktemp -d)/katakana" && mkdir -p "$SCR" && echo "$SCR"
```

Dump the six corpora (table `artificialtrans`, column `source`, distinct, trimmed, internal newlines removed — this recipe reproduces `kf3FHD` at exactly 21,224 and the six below at 56,065 in total):

```python
# $SCR/dump.py
import sqlite3, glob, os, sys
NAMES = {"PxC2": "pxc2", "enkou": "enkou", "hitoduma2": "hitoduma2",
         "ikenaiDX": "ikenaidx", "kf3if_sFHD": "kf3if", "おにキスＨ": "onikiss"}
out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for f in sorted(glob.glob(r"D:\Code\oss\vn-text\*.sqlite")):
    base = os.path.basename(f).split("_1")[0]
    if base not in NAMES:
        continue
    c = sqlite3.connect(f"file:{f}?mode=ro", uri=True)
    rows = c.execute("select distinct trim(source) from artificialtrans "
                     "where source is not null and trim(source)<>''").fetchall()
    c.close()
    seen, lines = set(), []
    for (s,) in rows:
        s = s.replace("\r", "").replace("\n", "")
        if s and s not in seen:
            seen.add(s); lines.append(s)
    name = NAMES[base]
    with open(os.path.join(out, name + ".tsv"), "w", encoding="utf-8") as fh:
        for i, s in enumerate(lines, 1):
            fh.write(f"{name}-{i:06d}\t{s}\n")
    print(f"{name:>10} {len(lines):>6}")
```

Run: `PYTHONIOENCODING=utf-8 python "$SCR/dump.py" "$SCR/pool"`
Expected: `pxc2 10499`, `enkou 8054`, `hitoduma2 5253`, `ikenaidx 9559`, `kf3if 16183`, `onikiss 6517`. **If any count differs, stop** — the recipe has drifted and every number below would be incomparable.

The measurement script, which reports both halves of the spec's §2.2 table:

```python
# $SCR/measure.py
import json, glob, os, sys
from collections import Counter
KATA = lambda c: ("\u30a1" <= c <= "\u30fa") or c == "\u30fc"
dist = Counter()
for p in sorted(glob.glob(os.path.join(sys.argv[1], "*.jsonl"))):
    for line in open(p, encoding="utf-8"):
        o = json.loads(line); ch = list(o["text"])
        starts = sorted({s[0] for s in o["segs"]} | {len(ch)})
        i = 0
        while i < len(ch):
            if not KATA(ch[i]):
                i += 1; continue
            j = i
            while j < len(ch) and KATA(ch[j]):
                j += 1
            if j - i >= 3:
                cuts = [p2 for p2 in starts if i < p2 < j]
                if cuts:
                    edges = [i] + cuts + [j]
                    pieces = [edges[k+1] - edges[k] for k in range(len(edges) - 1)]
                    dist["short" if min(pieces) < 3 else "wordsized"] += 1
                else:
                    dist["whole"] += 1
            i = j
print(f"runs>=3 whole {dist['whole']}, split-with-short-piece {dist['short']}, "
      f"split-into-word-sized {dist['wordsized']}")
```

And the gold check, which counts the katakana gold spans the engine misses:

```python
# $SCR/gold_katakana.py
import json, sys
KATA = lambda c: ("\u30a1" <= c <= "\u30fa") or c == "\u30fc"
gold = {}
for l in open(sys.argv[1], encoding="utf-8"):
    r = json.loads(l); gold[r["source"]] = r
hit = miss = 0; missed = []
for line in open(sys.argv[2], encoding="utf-8"):
    o = json.loads(line); g = gold[o["source"]]; ch = list(g["text"])
    segs = {(s[0], s[1]) for s in o["segs"]}
    for sp in g["spans"]:
        s, n = sp["start"], sp["len"]
        t = "".join(ch[s:s+n])
        if n >= 3 and all(KATA(c) for c in t):
            if (s, n) in segs:
                hit += 1
            else:
                miss += 1; missed.append(t)
print(f"katakana gold spans >=3: {hit+miss}, matched {hit}, missed {miss}")
print("missed:", "  ".join(missed))
```

Build the gold pool and the gold index once:

```bash
cd /d/Code/oss/clipyomi
python - <<'EOF'
import json
rows = [json.loads(l) for l in open("crates/jparser/tests/fixtures/gold.jsonl", encoding="utf-8")]
import os
scr = os.environ["SCR"]
with open(os.path.join(scr, "gold_pool.tsv"), "w", encoding="utf-8") as f:
    for r in rows:
        f.write(f"{r['source']}\t{r['text']}\n")
EOF
cargo build --release -p jparser --features mecab --bins --examples
./target/release/jparser-cli.exe build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$SCR/gold_index"
```

- [ ] **Step 2: Record the baseline, with the charge at 0**

Set `KATAKANA_FRAGMENT` to `0` temporarily, rebuild, and run every measurement below. This is the row every other row is read against, and it must reproduce the numbers in the spec: gold 1694/1948/0.9527/152 and 1588/1785/0.9504/118; katakana gold misses 7 hints-on and 26 hints-off; corpus split-with-short-piece 2,017 hints-on and 7,573 hints-off.

The four commands, with `H` the hints dictionary and `IDX` the installed index:

```bash
H="$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"
IDX="$APPDATA/dev.jparser.clipyomi/dict/gen-3"

# 1. gold, both states
./target/release/jparser-cli.exe score "$SCR/gold_index" crates/jparser/tests/fixtures/gold.jsonl \
  --compounds crates/jparser/tests/fixtures/compound_gold.jsonl --hints "$H" \
  | grep -E "span accuracy|entry top-1|span tolerant|compounds:"
./target/release/jparser-cli.exe score "$SCR/gold_index" crates/jparser/tests/fixtures/gold.jsonl \
  --compounds crates/jparser/tests/fixtures/compound_gold.jsonl \
  | grep -E "span accuracy|entry top-1|span tolerant|compounds:"

# 2. the katakana gold spans
./target/release/examples/span_dump.exe "$SCR/gold_index" "$SCR/gold_pool.tsv" --hints "$H" > "$SCR/gold.on.jsonl"
./target/release/examples/span_dump.exe "$SCR/gold_index" "$SCR/gold_pool.tsv" > "$SCR/gold.off.jsonl"
PYTHONIOENCODING=utf-8 python "$SCR/gold_katakana.py" crates/jparser/tests/fixtures/gold.jsonl "$SCR/gold.on.jsonl"
PYTHONIOENCODING=utf-8 python "$SCR/gold_katakana.py" crates/jparser/tests/fixtures/gold.jsonl "$SCR/gold.off.jsonl"

# 3. two corpora, both states (pxc2 and kf3if: the smallest and the largest)
mkdir -p "$SCR/on" "$SCR/off"
for c in pxc2 kf3if; do
  ./target/release/examples/span_dump.exe "$IDX" "$SCR/pool/$c.tsv" --hints "$H" > "$SCR/on/$c.jsonl"
  ./target/release/examples/span_dump.exe "$IDX" "$SCR/pool/$c.tsv" > "$SCR/off/$c.jsonl"
done
PYTHONIOENCODING=utf-8 python "$SCR/measure.py" "$SCR/on"
PYTHONIOENCODING=utf-8 python "$SCR/measure.py" "$SCR/off"
```

- [ ] **Step 3: Sweep**

For each value in `40, 50, 60, 80, 100, 130`, in that order: edit the constant, `cargo build --release -p jparser --features mecab --bins --examples`, then run, in this order, stopping at the first failure and recording why:

1. `cargo test -p jparser --features mecab --test engine_v2 katakana` — the four parses. `ソロホームラン` and `ラストラン` failing means the value is too high.
2. The two `score` commands. **Any cell below baseline rejects the value.**
3. `gold_katakana.py` on both dumps — how many of the 26 come back.
4. `measure.py` on both corpus directories — the short-piece row should collapse and the word-sized row must not move.

Record one row per value: value, tests pass/fail, the eight gold cells, katakana misses on/off, and the three corpus counts per state.

- [ ] **Step 4: Pin the value**

Choose the **lowest** value with zero gold damage and the most katakana misses recovered. Set `KATAKANA_FRAGMENT` to it, and replace the `Magnitude` paragraph of its doc comment with the measured sentence — the value, what it recovered, and the value above and below it with what each did.

If no value clears the gate, **stop and write the refusal** into the spec instead: the constant goes back to the value with the least damage, is not shipped, and the design gets a "Measured, and refused" section naming the cell that fell. That is a complete outcome for this task, not a failure of it.

- [ ] **Step 5: Append the measurement to the spec**

Add a `## 8. Measured 2026-09-21: the sweep` section to `docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md` carrying the table from Step 3, the chosen value, and one sentence on why the neighbours were not chosen.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
cargo test --workspace --features mecab 2>&1 | grep -E "^test result" | tail -3
git add crates/jparser/src/engine/cost.rs docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md
git commit -m "perf(engine): pin KATAKANA_FRAGMENT at the swept value

Six values measured against the zero-gold-damage gate; the table is in the
design's §8. The value shipped is the lowest that recovers the hints-off
katakana gold spans without moving a gold cell."
```

---

### Task 4: The gates, and the record

**Files:**
- Modify: `docs/superpowers/2026-09-21-six-corpora-handoff.md` (a postscript section, the shape §8 already has)
- No source changes. If this task needs one, it belongs in Task 2 or 3 and the plan was wrong.

**Interfaces:**
- Consumes: the shipped magnitude (Task 3).
- Produces: the recorded gate run, and the handoff paragraph a later session reads.

- [ ] **Step 1: Run every gate, in one pass**

```bash
cd /d/Code/oss/clipyomi
cargo fmt --all -- --check && echo "fmt clean"
cargo clippy --workspace --all-targets --features mecab 2>&1 | grep -cE "^(warning|error)"
cargo test --workspace --features mecab 2>&1 | grep -E "^test result" | awk -F'[ .;]+' '{p+=$4; f+=$6; i+=$8} END {print "passed", p, "failed", f, "ignored", i}'
git diff --stat crates/jparser/tests/fixtures/parse_sentences.txt
```

Expected: `fmt clean`; clippy `0`; `failed 0` with `ignored 1` (the pre-existing `jmdict_source` doc-test); no diff in the snapshot fixture.

- [ ] **Step 2: Re-run the gold pair and record it**

Both `score` commands from Task 3 Step 2. Expected: every cell at or above baseline — 1694 / 1948 / 0.9527 / 152 and 1588 / 1785 / 0.9504 / 118.

- [ ] **Step 3: Measure all six corpora, not two**

Repeat Task 3 Step 2's block 3 with all six names (`pxc2 enkou hitoduma2 ikenaidx kf3if onikiss`) and run `measure.py` over each state's directory. Expected direction: split-with-short-piece falls from 2,017 hints-on and 7,573 hints-off by at least an order of magnitude; split-into-word-sized stays at 303 and 477.

- [ ] **Step 4: Check the latency claim**

Run: `./target/release/jparser-cli.exe bench --help` and follow its interleaved A/B form against a copy of the pre-change binary. Record the **ratio**, not the milliseconds — absolute numbers drift about 10% across a session. Expected: no visible change; the run table is one O(n) pass per sentence.

- [ ] **Step 5: Write the postscript**

Append a section to `docs/superpowers/2026-09-21-six-corpora-handoff.md` in the shape of its §8: what shipped, the swept value and why, the before/after table for the katakana rows in both states, the gold pair (unchanged, and **say plainly that gold cannot show the hints-on win** — its own hints-on katakana misses are merges gold wants split), and what the change does not fix (`イ|って`, the `INEXACT` fold penalty, proper-noun recognition).

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/2026-09-21-six-corpora-handoff.md
git commit -m "docs(probe): the katakana run charge, measured on all six corpora"
```

---

## Self-Review

**Spec coverage.** §1's scope exclusions are carried into the Global Constraints and Task 4 Step 5. §2's rule is Tasks 1 and 2. §2.1's "charge not constraint" is Task 2's constant doc and the two compound tests. §2.2's signal is `measure.py`'s two rows. §3's placement is Task 1 Steps 4 and 8 and Task 2 Steps 3 and 4. §4's sweep protocol is Task 3 Step 3, in the spec's own order. §5's gates are Task 4 Steps 1–4. §6's five tests: geometry (Task 1 Step 6), run-crossing (Task 1 Step 6), ユリナス, ソロホームラン, ラストラン and the word-sized compound (Task 2 Step 6) — six, because the geometry case was worth its own. §7's open items are carried into Task 3 Step 4 (the empty-window refusal) and Task 4 Step 5.

**Placeholders.** None: every step names its command and its expected output, and every code step carries the code.

**Type consistency.** `inside_katakana_run` is spelled identically in Task 1 (field, two construction sites), Task 2 (unit test, `cost()`), and Task 2's word-sized test. `katakana_runs` returns `Vec<Option<(usize, usize)>>` in Task 1 Step 4 and is indexed that way in Step 8. `KATAKANA_FRAGMENT` is `i32`, matching `total`'s type in `cost()`. `kana::is_katakana` is used in `lex.rs` only after being added in Task 1 Step 3.

One correction made while reviewing: the spec's §6 lists five tests and this plan writes six — the geometry flag and the run-crossing case are separate tests because they assert on different candidates, and splitting them means a failure names which half broke.
