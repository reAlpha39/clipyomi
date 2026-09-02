# Engine v2 Phase 1b-i Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fill `Entry::conjugation` on the v2 path with a learner-standard English label rendered from `Entry::steps`, gated by a hand-written class × chain fixture that exists before the formatter does.

**Architecture:** One additive field, `ConjStep.aux`, records the auxiliary lemma `Table::search` already knows (一段 and 不変化型 are each shared by several auxiliaries, so class+form cannot name one). A new pure module `engine/label.rs` maps lemmas and the outermost form to words, deduplicates, applies three overrides, and joins. The gate is a tab-separated fixture read by an integration test that runs each surface through the real `Table` and checks the label; a coverage test in `table.rs` walks every reachable chain to depth 2 and proves the fixture omits nothing at depth 0–1.

**Tech Stack:** Rust 2021 (`crates/jparser`), `insta` for the existing snapshot. No new dependencies. No Python: the grid walk lives in a Rust test.

**Spec:** [`docs/superpowers/specs/2026-09-02-engine-v2-phase1b-i-design.md`](../specs/2026-09-02-engine-v2-phase1b-i-design.md)

**Master plan:** [`docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`](../specs/2026-08-27-engine-v2-master-plan.md)
**Phase 1a handoff:** [`docs/superpowers/2026-09-01-engine-v2-phase1a-handoff.md`](../2026-09-01-engine-v2-phase1a-handoff.md)

## Global Constraints

These apply to every task. Read them before Task 1 and again before any commit.

**Clean-room (master plan §3.5, spec §6).**

- **Do not open** `crates/jparser/assets/conjugations.json`, `crates/jparser/src/conjugation.rs`, or `crates/jparser/src/matcher.rs` at any point in this phase. The English column and the formatter come from the Wikipedia article named below and from `table_data.rs`'s IPADIC form names. If you believe you need something from those three files, stop and ask.
- **The grammar reference** is *Japanese verb conjugation*, English Wikipedia, `https://en.wikipedia.org/wiki/Japanese_verb_conjugation`. Terminology only. It glosses 仮定形 as "provisional, conditional, hypothetical", 命令形 as "imperative", the -ō/-yō form as "hortative, volitional, tentative, presumptive", 連用形 as "continuative, adverbial", れる/られる as "passive" and "potential", せる/させる as "causative", た as "perfective / past". The learner-standard choices among those synonyms are fixed in spec §3 and are not re-decided per task.
- New files carry Phase 1a's header verbatim (copy it from `crates/jparser/src/engine/chain.rs` lines 1–11) plus the provenance lines Task 2 and Task 3 specify. The SPDX flip is Phase 1b-ii's; do not touch licence text.

**Fixture authority (spec §4.1).** `tests/fixtures/conjugation_labels.txt` is written in Task 2 and is authoritative from then on. A row the formatter cannot pass is a finding: either the row is wrong (fix the row, say why in the commit message) or spec §3 is incomplete (add the rule, add a decision-log row in the spec). **Never edit a row's `expected` column to match what the code produces.**

**Scope.**

- **No switchover.** v1 stays the default. The nine hard-taint files, `assets/conjugations.json`, `xtask`'s converter, every `ta-old` citation and `ported from Translation Aggregator` header survive this phase untouched.
- **No change to `ATTACHES_TO`, `cost.rs`, `lattice.rs`, `lex.rs`, or candidate generation.** Table gaps the grid exposes (spec §7 lists four) are recorded in the handoff with the surfaces that expose them, not fixed.
- No TypeScript change. No k-best. No UI.

**Standing gates**, run before every commit:

```powershell
cargo test -p jparser
cargo test -p jparser --features mecab
cargo clippy -p jparser --all-targets -- -D warnings
cargo clippy -p jparser --all-targets --features mecab -- -D warnings
cargo check -p jparser --no-default-features
```

Baseline at `dd0c647`: **418** passed / 0 ignored default, **453** passed / 1 ignored with `--features mecab`, all clippy gates clean. (`the_particle_absorbing_merges_are_gone` is the one ignored mecab test; it stays ignored — Phase 1a's open item, not this phase's.)

**Commit messages:** `<type>(engine-v2): <description>`, body in normal prose, as the branch's existing commits do.

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/jparser/src/engine/mod.rs` | `ConjStep` gains `aux`; `pub mod label` | 1, 2 |
| `crates/jparser/src/engine/table.rs` | `TrieHit::Continuation` carries the lemma; `search` writes it into the step; coverage test; lemma-word completeness test | 1, 4 |
| `crates/jparser/src/engine/chain.rs` | Contract doc gains the `aux` invariant | 1 |
| `crates/jparser/src/engine/assemble.rs` | Three test literals gain `aux: None`; `build_entry` calls `label::fmt` | 1, 5 |
| `crates/jparser/src/engine/label.rs` | **New.** `AUX_WORD`, `FORM_WORD`, `OVERRIDE`; `pub fn fmt` | 2 (stub), 3 |
| `crates/jparser/tests/fixtures/conjugation_labels.txt` | **New.** The hand-written gate | 2 |
| `crates/jparser/tests/conjugation_labels.rs` | **New.** The gate test | 2, 3 |
| `crates/jparser/tests/engine_v2.rs` | One test inverted; snapshot regenerated | 5 |
| `crates/jparser/tests/snapshots/engine_v2__sentences.snap` | Regenerated | 5 |
| `crates/jparser/tests/fixtures/README.md` | Entry for the new fixture | 6 |
| `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` | Phase 1b split; decision log | 6 |
| `docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md` | **New.** Handoff | 6 |

---

### Task 1: `ConjStep.aux` — the auxiliary lemma on the step

**Files:**
- Modify: `crates/jparser/src/engine/mod.rs:50-60`
- Modify: `crates/jparser/src/engine/table.rs:280-297` (`TrieHit`), `:379-415` (`Table::new`), `:461-498` (`search`)
- Modify: `crates/jparser/src/engine/chain.rs:31-53` (contract doc)
- Modify: `crates/jparser/src/engine/assemble.rs:276-280`, `:313-317`, `:341-345` (test literals)
- Test: `crates/jparser/src/engine/table.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `pub struct ConjStep { pub class: ClassId, pub form: FormId, pub range: Range<usize>, pub aux: Option<&'static str> }`. Invariant: `aux` is `None` on `steps[0]` and `Some(lemma)` — the `ATTACHES_TO` lemma exactly as written there — on every later step. Every later task reads `aux`.

- [ ] **Step 1: Write the failing test**

Add to `table.rs`'s existing `mod tests`, after `a_long_auxiliary_chain_resolves_to_the_verb`:

```rust
    #[test]
    fn every_step_after_the_first_names_its_auxiliary() {
        // Phase 1b-i's formatter cannot tell せる from られる (both 一段) or
        // ん from て (both 不変化型) by class and form alone; the lemma has to
        // ride on the step. Master plan §7.3's chain, innermost first.
        let c = chars("食べさせられませんでした");
        let hits = table().chains_ending_at(&c, c.len(), c.len());
        let hit = hits
            .iter()
            .find(|h| key(&c, h) == "食べる")
            .expect("the chain must reach 食べる");
        let auxes: Vec<Option<&str>> = hit.steps.iter().map(|s| s.aux).collect();
        assert_eq!(
            auxes,
            vec![
                None,
                Some("させる"),
                Some("られる"),
                Some("ます"),
                Some("ん"),
                Some("です"),
                Some("た"),
            ]
        );
    }

    #[test]
    fn an_uninflected_or_single_step_chain_carries_no_auxiliary() {
        let c = chars("書いた");
        let hits = table().chains_ending_at(&c, 3, 3);
        let hit = hits.iter().find(|h| key(&c, h) == "書く").expect("書いた -> 書く");
        assert_eq!(hit.steps[0].aux, None, "the main word's own inflection has no lemma");
        assert_eq!(hit.steps[1].aux, Some("た"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jparser --lib engine::table::tests::every_step_after_the_first_names_its_auxiliary`
Expected: compile error, `no field `aux` on type `ConjStep``.

- [ ] **Step 3: Add the field**

In `crates/jparser/src/engine/mod.rs`, replace lines 50–60 with:

```rust
/// One step of a deinflection chain: which conjugation class was applied, in
/// which form, over which characters of the span.
///
/// Filled by Phase 1a (`chain.rs`); named in English by Phase 1b-i
/// (`label.rs`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConjStep {
    pub class: ClassId,
    pub form: FormId,
    pub range: std::ops::Range<usize>,
    /// The auxiliary this step is, as `table.rs`'s `ATTACHES_TO` spells its
    /// lemma — `Some("られる")`, `Some("ます")` — or `None` on the first step,
    /// which is the main word's own inflection and belongs to no auxiliary.
    /// Recorded here because the class alone cannot say: せる, させる, れる
    /// and られる are all 一段, and ん, う, よう, て and で are all 不変化型.
    pub aux: Option<&'static str>,
}
```

- [ ] **Step 4: Carry the lemma through the trie**

In `crates/jparser/src/engine/table.rs`, change `TrieHit::Continuation` (lines 288–296) to:

```rust
    /// One specific auxiliary's own conjugated surface ends here (built
    /// from an `ATTACHES_TO` row, not from `CLASSES` directly). The chain
    /// can continue only if the text immediately before it ends in
    /// `requires` — the 活用形 that auxiliary attaches to. `lemma` is the
    /// row's first column, carried onto the step so `label.rs` can name it.
    Continuation {
        class: ClassId,
        form: FormId,
        requires: &'static str,
        lemma: &'static str,
    },
```

In `Table::new`, the `insert(...)` inside the `ATTACHES_TO` loop (lines 405–414) becomes:

```rust
                insert(
                    &mut nodes,
                    &surface,
                    TrieHit::Continuation {
                        class: class_id as ClassId,
                        form: form_id as FormId,
                        requires,
                        lemma,
                    },
                );
```

In `search`, the `Terminal` arm's `ConjStep` literal (lines 467–471) gains `aux: None,`:

```rust
                                steps: vec![ConjStep {
                                    class,
                                    form,
                                    range: end - len..end,
                                    aux: None,
                                }],
```

and the `Continuation` arm (lines 476–498) destructures and writes the lemma:

```rust
                    TrieHit::Continuation {
                        class,
                        form,
                        requires,
                        lemma,
                    } => {
                        if len == 0 || !form_named(required, class, form) {
                            continue;
                        }
                        for mut sub in self.search(chars, end - len, budget - len, Some(requires)) {
                            sub.consumed += len;
                            sub.steps.push(ConjStep {
                                class,
                                form,
                                range: end - len..end,
                                aux: Some(lemma),
                            });
                            out.push(sub);
                        }
                    }
```

Keep the existing `// len == 0 is the loop-prevention guard` comment above the `if`.

- [ ] **Step 5: Fix the three test literals in `assemble.rs`**

Each of the `ConjStep { class, form: 0, range: .. }` literals at lines 276, 313 and 341 gains `aux: None,` as its last field. The third one (inside `order_candidate`) is:

```rust
            steps: vec![
                ConjStep {
                    class: 0,
                    form: 0,
                    range: 0..0,
                    aux: None,
                };
                step_count
            ],
```

- [ ] **Step 6: State the invariant in `chain.rs`'s contract**

In `crates/jparser/src/engine/chain.rs`, after the "**An uninflected match has no steps.**" bullet (line 50–53), add:

```rust
/// - **`aux` names the auxiliary, and only the auxiliary.** `steps[0].aux`
///   is `None` — it is the main word's own inflection. Every later step's
///   `aux` is `Some(lemma)`, the `ATTACHES_TO` row that produced it,
///   spelled exactly as that table spells it. `label.rs` keys on it.
```

- [ ] **Step 7: Run the tests**

Run: `cargo test -p jparser`
Expected: all pass, including the two new ones. Count: 420.

- [ ] **Step 8: Standing gates, then commit**

Run the five standing-gate commands. Then:

```powershell
git add crates/jparser/src/engine/mod.rs crates/jparser/src/engine/table.rs crates/jparser/src/engine/chain.rs crates/jparser/src/engine/assemble.rs
git commit -m "feat(engine-v2): record the auxiliary lemma on every ConjStep" -m "The formatter Phase 1b-i writes cannot name an auxiliary from class and form alone: せる, させる, れる and られる all conjugate as 一段, and ん, う, よう, て and で are all 不変化型. Table::search already knows which ATTACHES_TO row produced a Continuation hit; it now carries that row's lemma onto the step as ConjStep::aux, None on the first step. Not persisted (build.rs writes no step data), not read by the webview (types.ts declares conjugation only), not printed by the v2 snapshot."
```

---

### Task 2: The fixture, the formatter stub, and the gate test

**Files:**
- Create: `crates/jparser/tests/fixtures/conjugation_labels.txt`
- Create: `crates/jparser/src/engine/label.rs` (stub only)
- Create: `crates/jparser/tests/conjugation_labels.rs`
- Modify: `crates/jparser/src/engine/mod.rs:22-32` (`pub mod label;`)

**Interfaces:**
- Consumes: `ConjStep.aux` (Task 1); `jparser::engine::table::{Table, CLASSES, ChainHit}` (already `pub`).
- Produces: `pub fn jparser::engine::label::fmt(steps: &[ConjStep]) -> Option<String>` — stubbed to `None` here, implemented in Task 3. The fixture's three-column format, which Task 4's coverage test also parses.

The fixture is written **before** the formatter and is the authority. Its rows come from the mechanical grid (spec §4: every `(adjective?, form)` at depth 0, every `(aux lemma, aux form)` at depth 1, curated rows deeper); the English column was written reading each surface against the Wikipedia reference in the register spec §3 fixes. A few surfaces are not natural Japanese (食べよよう, 読んなら) — they are chains the table generates today, labelled by the same rules and marked `table finding`; spec §7 records the `ATTACHES_TO` cause.

- [ ] **Step 1: Write the fixture**

Create `crates/jparser/tests/fixtures/conjugation_labels.txt`. Columns are separated by a single **tab**; the file must be UTF-8 with LF line endings (check `.gitattributes`/editor settings — the repo's other fixtures are LF).

```
# Phase 1b-i's class x chain gate: the English label expected for each
# deinflection chain the v2 table generates.
# Design: docs/superpowers/specs/2026-09-02-engine-v2-phase1b-i-design.md §3-§4.
#
# Columns, tab-separated: surface, chain, expected.
#   surface  - a complete conjugated word. Table::chains_ending_at does not
#              consult the dictionary, so the headword only serves the reader.
#   chain    - space-separated steps, innermost first: class:form for the
#              first step, lemma:form for each auxiliary after it. Class and
#              form names are table_data.rs's IPADIC names; lemmas are
#              table.rs's ATTACHES_TO spellings.
#   expected - the label, or "-" for None.
#
# The English column was written by hand from "Japanese verb conjugation",
# English Wikipedia (https://en.wikipedia.org/wiki/Japanese_verb_conjugation),
# read 2026-09, terminology only, in the learner-standard register the design
# fixes: Causative, Passive/Potential, Negative, Volitional, Polite, Want-to,
# Past, Te-form, Copula, Conditional, Imperative, Adverbial. It was written
# before crates/jparser/src/engine/label.rs existed, and is authoritative over
# it. assets/conjugations.json, src/conjugation.rs and src/matcher.rs were not
# opened while this file was written.
#
# Rows marked "table finding" are surfaces that are not natural Japanese but
# are chains the table generates today. They are labelled by the same rules;
# the cause is an ATTACHES_TO row (span shape, Phase 1a's territory) and is
# recorded in the 1b-i handoff, not fixed here.
#
# A surface can appear twice with different chains (書け is both 仮定形 and
# 命令ｅ; 読んで reaches で through two rows). Each chain is its own row.

# --- Part A: depth 0. One row per (adjective?, form) reachable from a class
# --- JMDICT_POS_CLASS maps. Only Conditional, Imperative and Adverbial name a
# --- form on its own; every other form is the plain form or a dangling stem.
書け	五段・カ行イ音便:仮定形	Conditional
書け	五段・カ行イ音便:命令ｅ	Imperative
書きゃ	五段・カ行イ音便:仮定縮約１	Conditional
書く	五段・カ行イ音便:基本形	-
書か	五段・カ行イ音便:未然形	-
書こ	五段・カ行イ音便:未然ウ接続	-
書き	五段・カ行イ音便:連用形	-
書い	五段・カ行イ音便:連用タ接続	-
行っ	五段・カ行促音便:連用タ接続	-
取ん	五段・ラ行:未然特殊	-
食べれ	一段:仮定形	Conditional
食べりゃ	一段:仮定縮約１	Conditional
食べろ	一段:命令ｒｏ	Imperative
食べよ	一段:命令ｙｏ	Imperative
食べよ	一段:未然ウ接続	-
食べる	一段:基本形	-
食べ	一段:未然形	-
食べ	一段:連用形	-
食べん	一段:体言接続特殊	-
すれ	サ変・スル:仮定形	Conditional
しろ	サ変・スル:命令ｒｏ	Imperative
せよ	サ変・スル:命令ｙｏ	Imperative
せい	サ変・スル:命令ｉ	Imperative
する	サ変・スル:基本形	-
す	サ変・スル:文語基本形	-
す	サ変・スル:体言接続特殊２	-
し	サ変・スル:未然形	-
し	サ変・スル:連用形	-
しよ	サ変・スル:未然ウ接続	-
せ	サ変・スル:未然ヌ接続	-
さ	サ変・スル:未然レル接続	-
愛せ	サ変・−スル:未然レル接続	-
くれ	カ変・クル:仮定形	Conditional
こい	カ変・クル:命令ｉ	Imperative
こよ	カ変・クル:命令ｙｏ	Imperative
くる	カ変・クル:基本形	-
こ	カ変・クル:未然形	-
き	カ変・クル:連用形	-
くん	カ変・クル:体言接続特殊	-
く	カ変・クル:体言接続特殊２	-
あれ	ラ変:仮定形	Conditional
あれ	ラ変:命令ｅ	Imperative
あり	ラ変:基本形	-
ある	ラ変:体言接続	-
高けれ	形容詞・アウオ段:仮定形	Conditional
高けりゃ	形容詞・アウオ段:仮定縮約１	Conditional
高きゃ	形容詞・アウオ段:仮定縮約２	Conditional
高かれ	形容詞・アウオ段:命令ｅ	Imperative
高く	形容詞・アウオ段:連用テ接続	Adverbial
高い	形容詞・アウオ段:基本形	-
高し	形容詞・アウオ段:文語基本形	-
高き	形容詞・アウオ段:体言接続	-
高	形容詞・アウオ段:ガル接続	-
高かろ	形容詞・アウオ段:未然ウ接続	-
高から	形容詞・アウオ段:未然ヌ接続	-
高う	形容詞・アウオ段:連用ゴザイ接続	-
高かっ	形容詞・アウオ段:連用タ接続	-
いい	形容詞・イイ:基本形	-
いいっ	形容詞・イイ:基本形-促音便	-

# --- Part B: depth 1. One row per (auxiliary lemma, auxiliary form) with a
# --- non-empty surface. せる/れる on a godan 未然形, させる/られる on an ichidan
# --- one, as they are used; た/て on 書い, だ/で on 読ん.
書かせれ	五段・カ行イ音便:未然形 せる:仮定形	Causative Conditional
書かせりゃ	五段・カ行イ音便:未然形 せる:仮定縮約１	Causative Conditional
書かせん	五段・カ行イ音便:未然形 せる:体言接続特殊	Causative
書かせろ	五段・カ行イ音便:未然形 せる:命令ｒｏ	Causative Imperative
書かせよ	五段・カ行イ音便:未然形 せる:命令ｙｏ	Causative Imperative
書かせよ	五段・カ行イ音便:未然形 せる:未然ウ接続	Causative
書かせる	五段・カ行イ音便:未然形 せる:基本形	Causative
書かせ	五段・カ行イ音便:未然形 せる:未然形	Causative
書かせ	五段・カ行イ音便:未然形 せる:連用形	Causative
食べさせれ	一段:未然形 させる:仮定形	Causative Conditional
食べさせりゃ	一段:未然形 させる:仮定縮約１	Causative Conditional
食べさせん	一段:未然形 させる:体言接続特殊	Causative
食べさせろ	一段:未然形 させる:命令ｒｏ	Causative Imperative
食べさせよ	一段:未然形 させる:命令ｙｏ	Causative Imperative
食べさせよ	一段:未然形 させる:未然ウ接続	Causative
食べさせる	一段:未然形 させる:基本形	Causative
食べさせ	一段:未然形 させる:未然形	Causative
食べさせ	一段:未然形 させる:連用形	Causative
書かれれ	五段・カ行イ音便:未然形 れる:仮定形	Passive/Potential Conditional
書かれりゃ	五段・カ行イ音便:未然形 れる:仮定縮約１	Passive/Potential Conditional
書かれん	五段・カ行イ音便:未然形 れる:体言接続特殊	Passive/Potential
書かれろ	五段・カ行イ音便:未然形 れる:命令ｒｏ	Passive/Potential Imperative
書かれよ	五段・カ行イ音便:未然形 れる:命令ｙｏ	Passive/Potential Imperative
書かれよ	五段・カ行イ音便:未然形 れる:未然ウ接続	Passive/Potential
書かれる	五段・カ行イ音便:未然形 れる:基本形	Passive/Potential
書かれ	五段・カ行イ音便:未然形 れる:未然形	Passive/Potential
書かれ	五段・カ行イ音便:未然形 れる:連用形	Passive/Potential
食べられれ	一段:未然形 られる:仮定形	Passive/Potential Conditional
食べられりゃ	一段:未然形 られる:仮定縮約１	Passive/Potential Conditional
食べられん	一段:未然形 られる:体言接続特殊	Passive/Potential
食べられろ	一段:未然形 られる:命令ｒｏ	Passive/Potential Imperative
食べられよ	一段:未然形 られる:命令ｙｏ	Passive/Potential Imperative
食べられよ	一段:未然形 られる:未然ウ接続	Passive/Potential
食べられる	一段:未然形 られる:基本形	Passive/Potential
食べられ	一段:未然形 られる:未然形	Passive/Potential
食べられ	一段:未然形 られる:連用形	Passive/Potential
食べな	一段:未然形 ない:ガル接続	Negative
食べなけれ	一段:未然形 ない:仮定形	Negative Conditional
食べなけりゃ	一段:未然形 ない:仮定縮約１	Negative Conditional
食べなきゃ	一段:未然形 ない:仮定縮約２	Negative Conditional
食べなき	一段:未然形 ない:体言接続	Negative
食べなかれ	一段:未然形 ない:命令ｅ	Negative Imperative
食べない	一段:未然形 ない:基本形	Negative
食べなし	一段:未然形 ない:文語基本形	Negative
食べなかろ	一段:未然形 ない:未然ウ接続	Negative
食べなから	一段:未然形 ない:未然ヌ接続	Negative
食べのう	一段:未然形 ない:連用ゴザイ接続	Negative
食べなかっ	一段:未然形 ない:連用タ接続	Negative
食べなく	一段:未然形 ない:連用テ接続	Negative Adverbial
食べない	一段:未然形 ない:連用デ接続	Negative
食べねえ	一段:未然形 ない:音便基本形	Negative
書かね	五段・カ行イ音便:未然形 ぬ:仮定形	Negative Conditional
書かざる	五段・カ行イ音便:未然形 ぬ:体言接続	Negative
書かぬ	五段・カ行イ音便:未然形 ぬ:基本形	Negative
書かず	五段・カ行イ音便:未然形 ぬ:文語基本形	Negative
書かず	五段・カ行イ音便:未然形 ぬ:連用ニ接続	Negative
書かざり	五段・カ行イ音便:未然形 ぬ:連用形	Negative
書かん	五段・カ行イ音便:未然形 ん:基本形	Negative
書こう	五段・カ行イ音便:未然ウ接続 う:基本形	Volitional
# table finding: よう requires 未然ウ接続, so the natural 食べよう is only
# reachable through う (食べよ+う); this is the one chain よう produces.
食べよよう	一段:未然ウ接続 よう:基本形	Volitional
食べますれ	一段:連用形 ます:仮定形	Polite Conditional
食べませ	一段:連用形 ます:命令ｅ	Polite Imperative
食べまし	一段:連用形 ます:命令ｉ	Polite Imperative
食べます	一段:連用形 ます:基本形	Polite
食べましょ	一段:連用形 ます:未然ウ接続	Polite
食べませ	一段:連用形 ます:未然形	Polite
食べまし	一段:連用形 ます:連用形	Polite
食べた	一段:連用形 たい:ガル接続	Want-to
食べたけれ	一段:連用形 たい:仮定形	Want-to Conditional
食べたけりゃ	一段:連用形 たい:仮定縮約１	Want-to Conditional
食べたきゃ	一段:連用形 たい:仮定縮約２	Want-to Conditional
食べたき	一段:連用形 たい:体言接続	Want-to
食べたい	一段:連用形 たい:基本形	Want-to
食べたし	一段:連用形 たい:文語基本形	Want-to
食べたかろ	一段:連用形 たい:未然ウ接続	Want-to
食べたから	一段:連用形 たい:未然ヌ接続	Want-to
食べとう	一段:連用形 たい:連用ゴザイ接続	Want-to
食べたかっ	一段:連用形 たい:連用タ接続	Want-to
食べたく	一段:連用形 たい:連用テ接続	Want-to Adverbial
食べてえ	一段:連用形 たい:音便基本形	Want-to
書いたら	五段・カ行イ音便:連用タ接続 た:仮定形	Conditional
書いた	五段・カ行イ音便:連用タ接続 た:基本形	Past
書いたろ	五段・カ行イ音便:連用タ接続 た:未然形	Past
# table finding: ATTACHES_TO points the voiced past だ at the copula class
# 特殊・ダ, whose forms are なら/な/なれ/だろ/だっ/で, so 読んだら is unreachable
# and the four rows below it are spurious. Labelled by rule regardless.
読んなら	五段・マ行:連用タ接続 だ:仮定形	Conditional
読んな	五段・マ行:連用タ接続 だ:体言接続	Past
読んなれ	五段・マ行:連用タ接続 だ:命令ｅ	Past Imperative
読んだ	五段・マ行:連用タ接続 だ:基本形	Past
読んだろ	五段・マ行:連用タ接続 だ:未然形	Past
読んだっ	五段・マ行:連用タ接続 だ:連用タ接続	Past
読んで	五段・マ行:連用タ接続 だ:連用形	Te-form
書いて	五段・カ行イ音便:連用タ接続 て:基本形	Te-form
読んで	五段・マ行:連用タ接続 で:基本形	Te-form
食べるです	一段:基本形 です:基本形	Polite
食べるでしょ	一段:基本形 です:未然形	Polite
食べるでし	一段:基本形 です:連用形	Polite
食べるじゃ	一段:基本形 じゃ:基本形	Copula
食べるじゃろ	一段:基本形 じゃ:未然形	Copula
食べるじゃっ	一段:基本形 じゃ:連用形	Copula
食べるや	一段:基本形 や:基本形	Copula
食べるやろ	一段:基本形 や:未然形	Copula
食べるやっ	一段:基本形 や:連用形	Copula

# --- Part C: curated, depth 2-6. Every override and the Polite dedup, the
# --- corpus's deepest chain, and one row per remaining mapped class. An
# --- auxiliary's form here is the one the next auxiliary's ATTACHES_TO row
# --- requires (られ before ます is 連用形; before ない it is 未然形).
食べさせられませんでした	一段:未然形 させる:未然形 られる:連用形 ます:未然形 ん:基本形 です:連用形 た:基本形	Causative Passive/Potential Polite Negative Past
食べさせられませんでしたら	一段:未然形 させる:未然形 られる:連用形 ます:未然形 ん:基本形 です:連用形 た:仮定形	Causative Passive/Potential Polite Negative Conditional
食べさせられる	一段:未然形 させる:未然形 られる:基本形	Causative Passive/Potential
食べさせられたい	一段:未然形 させる:未然形 られる:連用形 たい:基本形	Causative Passive/Potential Want-to
食べさせよう	一段:未然形 させる:未然ウ接続 う:基本形	Causative Volitional
食べさせません	一段:未然形 させる:連用形 ます:未然形 ん:基本形	Causative Polite Negative
食べられません	一段:未然形 られる:連用形 ます:未然形 ん:基本形	Passive/Potential Polite Negative
食べられませんでした	一段:未然形 られる:連用形 ます:未然形 ん:基本形 です:連用形 た:基本形	Passive/Potential Polite Negative Past
食べられなかった	一段:未然形 られる:未然形 ない:連用タ接続 た:基本形	Passive/Potential Negative Past
書かせられた	五段・カ行イ音便:未然形 せる:未然形 られる:連用形 た:基本形	Causative Passive/Potential Past
書かせなかった	五段・カ行イ音便:未然形 せる:未然形 ない:連用タ接続 た:基本形	Causative Negative Past
書かせん	五段・カ行イ音便:未然形 せる:未然形 ん:基本形	Causative Negative
書かれました	五段・カ行イ音便:未然形 れる:連用形 ます:連用形 た:基本形	Passive/Potential Polite Past
書きません	五段・カ行イ音便:連用形 ます:未然形 ん:基本形	Polite Negative
食べました	一段:連用形 ます:連用形 た:基本形	Polite Past
食べましたら	一段:連用形 ます:連用形 た:仮定形	Polite Conditional
食べませんでした	一段:連用形 ます:未然形 ん:基本形 です:連用形 た:基本形	Polite Negative Past
食べませんでしたら	一段:連用形 ます:未然形 ん:基本形 です:連用形 た:仮定形	Polite Negative Conditional
食べましょう	一段:連用形 ます:未然ウ接続 う:基本形	Polite Volitional
食べなかった	一段:未然形 ない:連用タ接続 た:基本形	Negative Past
食べなかったら	一段:未然形 ない:連用タ接続 た:仮定形	Negative Conditional
食べたかった	一段:連用形 たい:連用タ接続 た:基本形	Want-to Past
食べたかったら	一段:連用形 たい:連用タ接続 た:仮定形	Want-to Conditional
食べたら	一段:連用形 た:仮定形	Conditional
食べた	一段:連用形 た:基本形	Past
食べて	一段:連用形 て:基本形	Te-form
# nonstandard surface, but the chain です:基本形 -> た generates it
食べるでした	一段:基本形 です:連用形 た:基本形	Polite Past
しよう	サ変・スル:未然ウ接続 う:基本形	Volitional
こよう	カ変・クル:未然ウ接続 う:基本形	Volitional
しなかった	サ変・スル:未然形 ない:連用タ接続 た:基本形	Negative Past
こなかった	カ変・クル:未然形 ない:連用タ接続 た:基本形	Negative Past
きました	カ変・クル:連用形 ます:連用形 た:基本形	Polite Past
されました	サ変・スル:未然レル接続 れる:連用形 ます:連用形 た:基本形	Passive/Potential Polite Past
させます	サ変・スル:未然レル接続 せる:連用形 ます:基本形	Causative Polite
愛されない	サ変・スル:未然レル接続 れる:未然形 ない:基本形	Passive/Potential Negative
愛させられた	サ変・スル:未然レル接続 せる:未然形 られる:連用形 た:基本形	Causative Passive/Potential Past
高かった	形容詞・アウオ段:連用タ接続 た:基本形	Past
高かったら	形容詞・アウオ段:連用タ接続 た:仮定形	Conditional
よかった	形容詞・アウオ段:連用タ接続 た:基本形	Past
いらっしゃいませ	五段・ラ行特殊:連用形 ます:命令ｅ	Polite Imperative
いらっしゃった	五段・ラ行特殊:連用タ接続 た:基本形	Past
ありません	五段・ラ行アル:連用形 ます:未然形 ん:基本形	Polite Negative
あります	五段・ラ行アル:連用形 ます:基本形	Polite
話した	五段・サ行:連用形 た:基本形	Past
話しましょう	五段・サ行:連用形 ます:未然ウ接続 う:基本形	Polite Volitional
泳いだ	五段・ガ行:連用タ接続 だ:基本形	Past
泳ぎたい	五段・ガ行:連用形 たい:基本形	Want-to
死んで	五段・ナ行:連用タ接続 で:基本形	Te-form
死なせた	五段・ナ行:未然形 せる:連用形 た:基本形	Causative Past
遊ばれた	五段・バ行:未然形 れる:連用形 た:基本形	Passive/Potential Past
待って	五段・タ行:連用タ接続 て:基本形	Te-form
待ちません	五段・タ行:連用形 ます:未然形 ん:基本形	Polite Negative
買った	五段・ワ行促音便:連用タ接続 た:基本形	Past
買わなかった	五段・ワ行促音便:未然形 ない:連用タ接続 た:基本形	Negative Past
問うた	五段・ワ行ウ音便:連用タ接続 た:基本形	Past
取られました	五段・ラ行:未然形 れる:連用形 ます:連用形 た:基本形	Passive/Potential Polite Past
行った	五段・カ行促音便:連用タ接続 た:基本形	Past
読みたかった	五段・マ行:連用形 たい:連用タ接続 た:基本形	Want-to Past
```

Row count: Part A 59, Part B 101, Part C 58 — 218 rows. Every `(surface, chain)` pair is unique; the same surface may recur with a different chain.

- [ ] **Step 2: Write the stub**

Create `crates/jparser/src/engine/label.rs`:

```rust
// ClipYomi — Engine v2, Phase 1b-i.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b-ii, when the crate stops being GPL-2.0-only.
//
// English vocabulary from "Japanese verb conjugation", English Wikipedia,
// https://en.wikipedia.org/wiki/Japanese_verb_conjugation, read 2026-09;
// terminology only. assets/conjugations.json, src/conjugation.rs and
// src/matcher.rs's label renderer were not opened while this file or
// tests/fixtures/conjugation_labels.txt was written.

//! The English conjugation label -- Phase 1b-i's half of master plan §7.3's
//! recombiner. Reads [`ConjStep`]s, writes `Entry::conjugation`.
//!
//! Stub: Task 3 of the 1b-i plan writes the formatter against
//! `tests/fixtures/conjugation_labels.txt`, which exists first.

use crate::engine::ConjStep;

/// The learner-facing label for a deinflection chain, innermost step first,
/// or `None` when no step contributes a word.
pub fn fmt(steps: &[ConjStep]) -> Option<String> {
    let _ = steps;
    None
}
```

In `crates/jparser/src/engine/mod.rs`, add `pub mod label;` between `pub mod kana;` and `pub mod lattice;` (alphabetical).

- [ ] **Step 3: Write the gate test**

Create `crates/jparser/tests/conjugation_labels.rs`:

```rust
// ClipYomi — Engine v2, Phase 1b-i.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.

//! Phase 1b-i's gate: every row of `fixtures/conjugation_labels.txt` is run
//! through the real deinflection trie, its chain must be found exactly once,
//! and `label::fmt` must produce the row's expected English. Design §4.2.

use jparser::engine::label;
use jparser::engine::table::{ChainHit, Table, CLASSES};
use jparser::engine::ConjStep;

const FIXTURE: &str = include_str!("fixtures/conjugation_labels.txt");

/// One parsed fixture row.
struct Row {
    line_no: usize,
    surface: String,
    chain: String,
    expected: Option<String>,
}

/// The fixture's rows, `#` comments and blank lines skipped. Panics on a
/// malformed row so a stray space instead of a tab is loud, not silent.
fn rows() -> Vec<Row> {
    FIXTURE
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.starts_with('#') && !l.trim().is_empty())
        .map(|(i, l)| {
            let mut cols = l.split('\t');
            let surface = cols.next().expect("surface").to_string();
            let chain = cols
                .next()
                .unwrap_or_else(|| panic!("line {}: no chain column (tab-separated?)", i + 1))
                .to_string();
            let expected = cols
                .next()
                .unwrap_or_else(|| panic!("line {}: no expected column", i + 1));
            assert!(cols.next().is_none(), "line {}: more than three columns", i + 1);
            Row {
                line_no: i + 1,
                surface,
                chain,
                expected: (expected != "-").then(|| expected.to_string()),
            }
        })
        .collect()
}

/// A chain in the fixture's notation: `class:form` for the first step,
/// `lemma:form` for each auxiliary.
fn render(steps: &[ConjStep]) -> String {
    steps
        .iter()
        .map(|s| {
            let class = &CLASSES[s.class as usize];
            let form = class.forms[s.form as usize].name;
            match s.aux {
                None => format!("{}:{form}", class.name),
                Some(lemma) => format!("{lemma}:{form}"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every chain the table finds over the whole surface, with the budget set
/// to the surface's own length so nothing depends on `lex.rs`'s MAX_CHAIN.
fn chains_over(table: &Table, surface: &str) -> Vec<ChainHit> {
    let chars: Vec<char> = surface.chars().collect();
    table.chains_ending_at(&chars, chars.len(), chars.len())
}

#[test]
fn the_fixture_parses_and_is_not_empty() {
    let rows = rows();
    assert!(rows.len() > 200, "expected the full grid, got {} rows", rows.len());
    for r in &rows {
        assert!(r.chain.contains(':'), "line {}: chain has no class:form step", r.line_no);
    }
}

#[test]
fn every_fixture_chain_is_generated_exactly_once() {
    let table = Table::new();
    let mut wrong = Vec::new();
    for r in rows() {
        let hits = chains_over(&table, &r.surface);
        let matching = hits.iter().filter(|h| render(&h.steps) == r.chain).count();
        if matching != 1 {
            let found: Vec<String> = hits.iter().map(|h| render(&h.steps)).collect();
            wrong.push(format!(
                "line {}: {} — chain `{}` found {matching} times; table generates {found:?}",
                r.line_no, r.surface, r.chain
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "fixture chains the table does not generate exactly once:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn every_fixture_row_labels_as_expected() {
    let table = Table::new();
    let mut wrong = Vec::new();
    for r in rows() {
        let hits = chains_over(&table, &r.surface);
        let Some(hit) = hits.iter().find(|h| render(&h.steps) == r.chain) else {
            // The test above reports this case with more detail.
            continue;
        };
        let got = label::fmt(&hit.steps);
        if got != r.expected {
            wrong.push(format!(
                "line {}: {} [{}] expected {:?}, got {got:?}",
                r.line_no, r.surface, r.chain, r.expected
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} rows mislabelled:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
```

- [ ] **Step 4: Run the gate — chains first**

Run: `cargo test -p jparser --test conjugation_labels every_fixture_chain_is_generated_exactly_once`
Expected: **PASS**. Every row's chain is one the table generates. If any row fails here, the fixture row is wrong (a typo in a form name, or a chain the table does not produce) — fix the **chain** column against the message's `table generates [...]` list, never the `expected` column, and say so in the commit message. If a natural surface has *no* matching chain at all, that is a table finding: delete the row, and add the surface to the handoff's findings list (Task 6).

- [ ] **Step 5: Run the gate — labels, expected RED**

Run: `cargo test -p jparser --test conjugation_labels every_fixture_row_labels_as_expected`
Expected: **FAIL**, with every row whose `expected` is not `-` listed (about 150 rows; the stub returns `None`). Record the exact count from the message — the Task 3 commit message quotes it.

- [ ] **Step 6: Mark the label test ignored for the commit**

The standing gate must stay green at every commit, and the fixture must land before the formatter. Add, directly above `fn every_fixture_row_labels_as_expected`:

```rust
#[ignore = "Task 3 of the 1b-i plan writes label.rs against this fixture; un-ignore there"]
```

Run: `cargo test -p jparser --test conjugation_labels`
Expected: 2 passed, 1 ignored.

- [ ] **Step 7: Standing gates, then commit**

Run the five standing-gate commands (`cargo test -p jparser` now reports 422 passed, 1 ignored). Then:

```powershell
git add crates/jparser/tests/fixtures/conjugation_labels.txt crates/jparser/tests/conjugation_labels.rs crates/jparser/src/engine/label.rs crates/jparser/src/engine/mod.rs
git commit -m "test(engine-v2): pin the conjugation-label fixture before the formatter exists" -m "Design §4.1: the class x chain grid's English column is written by hand, from Wikipedia's Japanese verb conjugation article in the learner-standard register, and committed before label.rs has a body, so it cannot agree with the code by construction. 218 rows: every (adjective?, form) tuple at depth 0, every (auxiliary lemma, auxiliary form) tuple at depth 1, and a curated set to depth 6 covering each override and the Polite dedup. The gate test runs each surface through the real trie and finds every chain exactly once; the label half is ignored until Task 3 un-ignores it against a real fmt. label.rs is a stub returning None."
```

---

### Task 3: `label.rs` — the formatter

**Files:**
- Modify: `crates/jparser/src/engine/label.rs` (replace the stub body)
- Modify: `crates/jparser/tests/conjugation_labels.rs` (remove the `#[ignore]`)

**Interfaces:**
- Consumes: `ConjStep { class, form, aux }` (Task 1); `jparser::engine::table::CLASSES`.
- Produces: `pub fn fmt(steps: &[ConjStep]) -> Option<String>`; `pub(crate) fn aux_word(lemma: &str) -> Option<&'static str>` (Task 4's completeness test reads it).

- [ ] **Step 1: Un-ignore the gate and confirm RED**

Delete the `#[ignore = ...]` line added in Task 2 Step 6.

Run: `cargo test -p jparser --test conjugation_labels every_fixture_row_labels_as_expected`
Expected: FAIL, same row count as Task 2 Step 5.

- [ ] **Step 2: Write the formatter**

Replace everything in `crates/jparser/src/engine/label.rs` below the header comment block (keep lines 1–18, the licence and provenance header) with:

```rust
//! The English conjugation label -- Phase 1b-i's half of master plan §7.3's
//! recombiner. Reads [`ConjStep`]s, writes `Entry::conjugation`.
//!
//! Three tables and four rules (design §3). Every auxiliary lemma
//! contributes one word, in step order; the outermost step's form
//! contributes one more; each word appears at most once; and three
//! `(lemma, outermost form)` pairs collapse into a single word. Zero words is
//! `None`. The tables are whatever passes
//! `tests/fixtures/conjugation_labels.txt`, which was written first and is
//! authoritative -- a row this code cannot pass is a finding about the
//! table, not a row to edit.

use crate::engine::table::CLASSES;
use crate::engine::ConjStep;

/// One word per auxiliary lemma, spelled as `table.rs`'s `ATTACHES_TO`
/// spells it (design §3.1). れる/られる are ambiguous between passive,
/// potential and honorific and the parser cannot tell, so the label says so.
/// だ here is the voiced た of 読んだ, not the copula. じゃ/や are
/// `ATTACHES_TO`'s placeholder rows.
const AUX_WORD: &[(&str, &str)] = &[
    ("せる", "Causative"),
    ("させる", "Causative"),
    ("れる", "Passive/Potential"),
    ("られる", "Passive/Potential"),
    ("ない", "Negative"),
    ("ぬ", "Negative"),
    ("ん", "Negative"),
    ("う", "Volitional"),
    ("よう", "Volitional"),
    ("ます", "Polite"),
    ("です", "Polite"),
    ("たい", "Want-to"),
    ("た", "Past"),
    ("だ", "Past"),
    ("て", "Te-form"),
    ("で", "Te-form"),
    ("じゃ", "Copula"),
    ("や", "Copula"),
];

/// One word from the outermost step's 活用形 (design §3.2). A form not
/// listed here -- 基本形, every 未然/連用 attachment stem, 体言接続, the
/// 文語 and 音便 variants -- adds nothing: it is either the plain form or a
/// dangling stem whose meaning arrives with the next auxiliary.
const FORM_WORD: &[(&str, &str)] = &[
    ("仮定形", "Conditional"),
    ("仮定縮約１", "Conditional"),
    ("仮定縮約２", "Conditional"),
    ("命令ｅ", "Imperative"),
    ("命令ｉ", "Imperative"),
    ("命令ｒｏ", "Imperative"),
    ("命令ｙｏ", "Imperative"),
    ("連用テ接続", "Adverbial"),
];

/// `(lemma, outermost form)` pairs whose two words collapse into one
/// (design §3.3 rule 2). たら is a conditional, not a "Past Conditional";
/// 読んで through the copula class's 連用形 is the te-form, not "Past".
const OVERRIDE: &[(&str, &str, &str)] = &[
    ("た", "仮定形", "Conditional"),
    ("だ", "仮定形", "Conditional"),
    ("だ", "連用形", "Te-form"),
];

/// The word for an auxiliary lemma, if `AUX_WORD` knows it.
pub(crate) fn aux_word(lemma: &str) -> Option<&'static str> {
    AUX_WORD.iter().find(|(l, _)| *l == lemma).map(|(_, w)| *w)
}

fn form_word(form: &str) -> Option<&'static str> {
    FORM_WORD.iter().find(|(f, _)| *f == form).map(|(_, w)| *w)
}

fn override_word(lemma: &str, form: &str) -> Option<&'static str> {
    OVERRIDE
        .iter()
        .find(|(l, f, _)| *l == lemma && *f == form)
        .map(|(_, _, w)| *w)
}

fn form_name(step: &ConjStep) -> &'static str {
    CLASSES[step.class as usize].forms[step.form as usize].name
}

/// Rule 1's dedup: a word already present is not repeated.
fn push(words: &mut Vec<&'static str>, word: Option<&'static str>) {
    if let Some(w) = word {
        if !words.contains(&w) {
            words.push(w);
        }
    }
}

/// The learner-facing label for a deinflection chain, innermost step first,
/// or `None` when no step contributes a word (an uninflected match, a plain
/// form, or a bare stem).
pub fn fmt(steps: &[ConjStep]) -> Option<String> {
    let (last, inner) = steps.split_last()?;
    let mut words: Vec<&'static str> = Vec::new();
    for step in inner {
        push(&mut words, step.aux.and_then(aux_word));
    }
    let form = form_name(last);
    match last.aux {
        Some(lemma) => match override_word(lemma, form) {
            Some(w) => push(&mut words, Some(w)),
            None => {
                push(&mut words, aux_word(lemma));
                push(&mut words, form_word(form));
            }
        },
        None => push(&mut words, form_word(form)),
    }
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class_id(name: &str) -> u16 {
        CLASSES
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("no class {name}")) as u16
    }

    fn form_id(class: &str, form: &str) -> u16 {
        CLASSES[class_id(class) as usize]
            .forms
            .iter()
            .position(|f| f.name == form)
            .unwrap_or_else(|| panic!("{class} has no form {form}")) as u16
    }

    /// A step by names. Ranges are irrelevant to `fmt` and left empty.
    fn step(class: &str, form: &str, aux: Option<&'static str>) -> ConjStep {
        ConjStep {
            class: class_id(class),
            form: form_id(class, form),
            range: 0..0,
            aux,
        }
    }

    #[test]
    fn no_steps_is_no_label() {
        assert_eq!(fmt(&[]), None);
    }

    #[test]
    fn a_plain_form_or_a_bare_stem_is_no_label() {
        assert_eq!(fmt(&[step("一段", "基本形", None)]), None);
        assert_eq!(fmt(&[step("五段・カ行イ音便", "連用タ接続", None)]), None);
        assert_eq!(fmt(&[step("一段", "未然形", None)]), None);
    }

    #[test]
    fn a_form_alone_can_carry_a_word() {
        assert_eq!(
            fmt(&[step("五段・カ行イ音便", "仮定形", None)]).as_deref(),
            Some("Conditional")
        );
        assert_eq!(
            fmt(&[step("形容詞・アウオ段", "連用テ接続", None)]).as_deref(),
            Some("Adverbial")
        );
    }

    #[test]
    fn words_come_in_step_order_then_the_outermost_form() {
        let steps = [
            step("一段", "未然形", None),
            step("一段", "未然形", Some("させる")),
            step("一段", "仮定形", Some("られる")),
        ];
        assert_eq!(
            fmt(&steps).as_deref(),
            Some("Causative Passive/Potential Conditional")
        );
    }

    #[test]
    fn a_repeated_word_appears_once() {
        // ませんでした: ます and です both say Polite.
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "未然形", Some("ます")),
            step("不変化型", "基本形", Some("ん")),
            step("特殊・デス", "連用形", Some("です")),
            step("特殊・タ", "基本形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Polite Negative Past"));
    }

    #[test]
    fn an_override_replaces_both_words_but_keeps_the_inner_ones() {
        // たら alone is Conditional, not Past Conditional...
        let steps = [
            step("一段", "連用形", None),
            step("特殊・タ", "仮定形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Conditional"));
        // ...and ましたら keeps Polite in front of it.
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "連用形", Some("ます")),
            step("特殊・タ", "仮定形", Some("た")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Polite Conditional"));
    }

    #[test]
    fn an_unknown_lemma_contributes_nothing_rather_than_panicking() {
        let steps = [
            step("一段", "連用形", None),
            step("特殊・マス", "仮定形", Some("not-a-lemma")),
        ];
        assert_eq!(fmt(&steps).as_deref(), Some("Conditional"));
    }
}
```

- [ ] **Step 3: Run the gate — GREEN**

Run: `cargo test -p jparser --test conjugation_labels`
Expected: 3 passed, 0 ignored.

If `every_fixture_row_labels_as_expected` still lists rows: stop and read each. Per the Global Constraints, either the row is wrong or spec §3 is incomplete. Do not edit `expected` to match `got`. If the fix is a new `OVERRIDE` row or `FORM_WORD` row, add it here **and** add a row to spec §10's decision log naming the fixture line that forced it.

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p jparser --lib engine::label`
Expected: 7 passed.

- [ ] **Step 5: Standing gates, then commit**

Run the five standing-gate commands. Then:

```powershell
git add crates/jparser/src/engine/label.rs crates/jparser/tests/conjugation_labels.rs
git commit -m "feat(engine-v2): render the conjugation label from the chain's steps" -m "label::fmt maps each auxiliary lemma to one learner-standard word in step order, adds one word for the outermost form, deduplicates (ませんでした says Polite once), and collapses three (lemma, form) pairs -- たら and the copula-class 読んで -- into a single word. Un-ignores the gate: all <N> previously-failing fixture rows now pass, no fixture row was edited to match the code."
```

Replace `<N>` with the count recorded in Task 2 Step 5.

---

### Task 4: The coverage test — the fixture omits nothing the table reaches

**Files:**
- Modify: `crates/jparser/src/engine/table.rs` (`mod tests`)

**Interfaces:**
- Consumes: `ATTACHES_TO`, `CLASSES`, `JMDICT_POS_CLASS` (private/pub in `table.rs`); `label::{fmt, aux_word}` (Task 3); the fixture's format (Task 2).

Two tests. The first walks every chain reachable from a mapped class to two auxiliaries deep — the same walk that sized the gate (spec §4: 170 + 1,860 + 19,730 chains) — and asserts every depth-0 `(is_adjective, form)` tuple and every depth-1 `(lemma, form)` tuple has a fixture row, and that `fmt` is total and never repeats a word. It asserts **no English** beyond what the fixture already pins (spec §4.3). The second asserts every `ATTACHES_TO` lemma has an `AUX_WORD` row.

- [ ] **Step 1: Write the tests**

Add to `table.rs`'s `mod tests`, after `every_tokushu_class_has_an_attaches_to_row`:

```rust
    #[test]
    fn every_attaches_to_lemma_has_a_label_word() {
        // A lemma label.rs does not know would silently contribute no word.
        for (lemma, _, _) in ATTACHES_TO {
            assert!(
                crate::engine::label::aux_word(lemma).is_some(),
                "ATTACHES_TO lemma {lemma} has no AUX_WORD row in label.rs"
            );
        }
    }

    /// The fixture's depth-0 and depth-1 tuples, parsed from its `chain`
    /// column: `(is_adjective, form)` for one-step rows, `(lemma, form)` for
    /// two-step rows. Deeper rows are curated and not enumerated here.
    fn fixture_tuples() -> (
        std::collections::BTreeSet<(bool, String)>,
        std::collections::BTreeSet<(String, String)>,
    ) {
        const FIXTURE: &str = include_str!("../../tests/fixtures/conjugation_labels.txt");
        let mut depth0 = std::collections::BTreeSet::new();
        let mut depth1 = std::collections::BTreeSet::new();
        for line in FIXTURE
            .lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        {
            let chain = line.split('\t').nth(1).expect("chain column");
            let steps: Vec<&str> = chain.split(' ').collect();
            let (class_name, form0) = steps[0].split_once(':').expect("class:form");
            let is_adjective = class(class_name).is_adjective;
            match steps.len() {
                1 => {
                    depth0.insert((is_adjective, form0.to_string()));
                }
                2 => {
                    let (lemma, form1) = steps[1].split_once(':').expect("lemma:form");
                    depth1.insert((lemma.to_string(), form1.to_string()));
                }
                _ => {}
            }
        }
        (depth0, depth1)
    }

    /// The auxiliary's conjugated surfaces, the way `Table::new` builds them:
    /// its lemma minus its class's `remove`, plus each form's suffix, empty
    /// surfaces skipped.
    fn aux_forms(lemma: &str, class_name: &str) -> Vec<(FormId, String)> {
        let c = class(class_name);
        let stem = lemma.strip_suffix(c.remove).unwrap_or(lemma);
        c.forms
            .iter()
            .enumerate()
            .map(|(id, f)| (id as FormId, format!("{stem}{}", f.suffix)))
            .filter(|(_, surface)| !surface.is_empty())
            .collect()
    }

    fn class_id_of(name: &str) -> ClassId {
        CLASSES.iter().position(|c| c.name == name).expect("class") as ClassId
    }

    /// Asserts `fmt` is total on `steps`: returns `None` or a non-empty label
    /// with no repeated word. Asserts nothing about which words.
    fn check_fmt(steps: &[ConjStep]) {
        if let Some(label) = crate::engine::label::fmt(steps) {
            assert!(!label.is_empty(), "empty Some for {steps:?}");
            let words: Vec<&str> = label.split(' ').collect();
            let unique: std::collections::BTreeSet<&str> = words.iter().copied().collect();
            assert_eq!(words.len(), unique.len(), "repeated word in {label:?} for {steps:?}");
        }
    }

    #[test]
    fn the_label_fixture_covers_every_reachable_tuple_and_fmt_is_total() {
        // Design §4.3. Walk every chain a JMDICT_POS_CLASS-mapped class can
        // start, two auxiliaries deep: (class, form) --requires--> (aux,
        // form) --requires--> (aux, form). Depth 0 and 1 must each have a
        // fixture row; every chain to depth 2 must not break fmt. No English
        // is asserted here -- doing so compositionally would test the
        // formatter against its own rule (Phase 1a design §2.2's tautology).
        let (depth0, depth1) = fixture_tuples();
        let mapped: std::collections::BTreeSet<&str> =
            JMDICT_POS_CLASS.iter().map(|(_, c)| *c).collect();
        let step = |class: ClassId, form: FormId, aux: Option<&'static str>| ConjStep {
            class,
            form,
            range: 0..0,
            aux,
        };

        let mut missing = Vec::new();
        let mut chains = 0usize;
        for class_name in mapped {
            let c0 = class_id_of(class_name);
            let c = class(class_name);
            for (f0, form0) in c.forms.iter().enumerate() {
                let s0 = step(c0, f0 as FormId, None);
                chains += 1;
                check_fmt(&[s0.clone()]);
                if !depth0.contains(&(c.is_adjective, form0.name.to_string())) {
                    missing.push(format!("depth 0: {class_name}:{}", form0.name));
                }
                for &(lemma1, ac1, req1) in ATTACHES_TO.iter().filter(|(_, _, r)| *r == form0.name) {
                    let c1 = class_id_of(ac1);
                    for (f1, _) in aux_forms(lemma1, ac1) {
                        let form1 = CLASSES[c1 as usize].forms[f1 as usize].name;
                        let s1 = step(c1, f1, Some(lemma1));
                        chains += 1;
                        check_fmt(&[s0.clone(), s1.clone()]);
                        if !depth1.contains(&(lemma1.to_string(), form1.to_string())) {
                            missing.push(format!("depth 1: {lemma1}:{form1}"));
                        }
                        for &(lemma2, ac2, _) in ATTACHES_TO.iter().filter(|(_, _, r)| *r == form1) {
                            let c2 = class_id_of(ac2);
                            for (f2, _) in aux_forms(lemma2, ac2) {
                                chains += 1;
                                check_fmt(&[s0.clone(), s1.clone(), step(c2, f2, Some(lemma2))]);
                            }
                        }
                    }
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "reachable tuples with no fixture row:\n{}",
            missing.join("\n")
        );
        assert!(
            chains > 20_000,
            "the walk reached only {chains} chains; the table's shape changed -- re-size the gate"
        );
    }
```

`class(name)` is the existing test helper at the top of `mod tests`. `ConjStep` is already imported at the top of `table.rs`; `ClassId`/`FormId` are re-exported there too.

- [ ] **Step 2: Run**

Run: `cargo test -p jparser --lib engine::table::tests::the_label_fixture_covers_every_reachable_tuple_and_fmt_is_total`
Expected: PASS. If it lists `missing` tuples, the fixture is incomplete: **add** a row for each (surface on any stem that reaches the form; English by spec §3), re-run Task 2 Step 4's chain test to confirm the new row's chain is generated, and re-run this test. Adding rows is allowed; changing an existing row's `expected` is not.

Run: `cargo test -p jparser --lib engine::table::tests::every_attaches_to_lemma_has_a_label_word`
Expected: PASS.

- [ ] **Step 3: Standing gates, then commit**

Run the five standing-gate commands. Then:

```powershell
git add crates/jparser/src/engine/table.rs crates/jparser/tests/fixtures/conjugation_labels.txt
git commit -m "test(engine-v2): prove the label fixture covers every chain the table reaches" -m "Walks every chain a JMDICT_POS_CLASS-mapped class can start to two auxiliaries deep -- the walk that sized the gate -- and asserts every depth-0 (adjective?, form) and depth-1 (lemma, form) tuple has a fixture row, and that label::fmt is total and never repeats a word on any of the ~21,700 chains. Asserts no English beyond what the fixture pins. Also asserts every ATTACHES_TO lemma has an AUX_WORD row."
```

If Step 2 added fixture rows, say how many and which tuples in the commit body.

---

### Task 5: Wire the label into `Entry`, regenerate the snapshot, confirm the metrics did not move

**Files:**
- Modify: `crates/jparser/src/engine/assemble.rs:13-24` (module doc), `:137-158` (`build_entry`)
- Modify: `crates/jparser/tests/engine_v2.rs:403-412`
- Regenerate: `crates/jparser/tests/snapshots/engine_v2__sentences.snap`

**Interfaces:**
- Consumes: `label::fmt` (Task 3), `chain::steps_for` (existing).
- Produces: `Entry::conjugation` is `Some(label)` on the v2 path wherever `fmt` returns one. Nothing else observable changes.

- [ ] **Step 1: Invert the Phase 1a test**

In `crates/jparser/tests/engine_v2.rs`, replace `phase_1a_emits_no_conjugation_label_but_does_emit_the_steps` (lines 403–412) with:

```rust
#[test]
fn the_v2_path_labels_a_conjugated_entry_from_its_steps() {
    let dir = scratch("assemble-steps");
    build_v2(&dir);
    let r = parse_v2_over(&dir, "書いた");
    let e = &r.segments[0].entries[0];
    assert_eq!(e.headword, "書く");
    assert!(!e.steps.is_empty(), "Phase 1a hands the formatter the steps");
    assert_eq!(e.conjugation.as_deref(), Some("Past"), "Phase 1b-i names them");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p jparser --test engine_v2 the_v2_path_labels_a_conjugated_entry_from_its_steps`
Expected: FAIL, `left: None, right: Some("Past")`.

- [ ] **Step 3: Wire `fmt` into `build_entry`**

In `crates/jparser/src/engine/assemble.rs`, add `use crate::engine::label;` after `use crate::engine::chain;`. Replace `build_entry`'s doc comment and body (lines 135–158) with:

```rust
/// One [`Entry`] for `cand`: its dictionary data resolved through `index`,
/// its reading reconstructed per this module's contract, and its
/// conjugation label rendered from the chain by `label::fmt`.
fn build_entry(index: &Index, chars: &[char], cand: &Candidate) -> Result<Entry, IndexError> {
    // A candidate's `entry_id` always names an entry in the same index it
    // was matched from -- `lex::candidates` reads both from `index` in the
    // same call. A well-formed index never disagrees with itself.
    let entry_data = index.entry(cand.entry_id)?.expect(
        "invariant: lex::candidates only ever emits a Candidate whose entry_id it just \
         read out of this same `index`, so `index.entry(cand.entry_id)` resolving to None \
         here means the index disagreed with itself between the two calls",
    );

    let reading = reading_for(&entry_data, cand, chars);
    let pos = union_pos(&entry_data.senses);
    let steps = chain::steps_for(cand);

    Ok(Entry {
        headword: cand.headword.clone(),
        reading,
        conjugation: label::fmt(&steps),
        pos,
        senses: entry_data.senses,
        flags: cand.flags,
        entry_id: cand.entry_id,
        steps,
    })
}
```

Replace the module doc's second paragraph (lines 16–24, "Per spec §8, [`Entry::conjugation`] stays `None`...") with:

```rust
//! `Entry::conjugation` is `label::fmt` over the chain `chain::steps_for`
//! hands back (Phase 1b-i). What this module owns beyond that is reading
//! reconstruction: `EntryData::readings` holds a reading only for a kanji
//! entry, and only in its dictionary form, so a conjugated match has to
//! strip that dictionary form's own suffix and rebuild it from the matched
//! surface's tail. A reading that does not strip cleanly is never guessed
//! at -- `None` is what goes out, because a wrong reading renders as
//! furigana the reader has no reason to doubt.
```

- [ ] **Step 4: Run the inverted test**

Run: `cargo test -p jparser --test engine_v2 the_v2_path_labels_a_conjugated_entry_from_its_steps`
Expected: PASS.

- [ ] **Step 5: See the snapshot diff, then accept it**

Run: `cargo test -p jparser --test engine_v2 v2_snapshots_every_sentence`
Expected: FAIL with insta's diff. Read it. Every changed line must be of the shape `    <headword> (-) [...]` → `    <headword> (<Label>) [...]`. **No** `start=`/`len=`/`reading=` line may change, no entry may reorder, no gloss may change. If anything else moved, stop: the wiring changed something it must not, and that is a bug to find before accepting.

Then accept:

```powershell
$env:INSTA_UPDATE = "always"
cargo test -p jparser --test engine_v2 v2_snapshots_every_sentence
Remove-Item Env:\INSTA_UPDATE
git diff --stat crates/jparser/tests/snapshots/engine_v2__sentences.snap
```

Read `git diff crates/jparser/tests/snapshots/engine_v2__sentences.snap` once more in full, and count: how many `(-)` slots became labels, and which labels appear. Record both for the commit message and the handoff. Look specifically for labels that read wrong on their sentence (a `Copula` or `Past Imperative` on running text is a table finding for the handoff, not a formatter change).

- [ ] **Step 6: Confirm the label is visible from the CLI**

```powershell
$scratch = "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase1b-i"
New-Item -ItemType Directory -Force $scratch
cargo run --release -p jparser --bin jparser-cli -- build-index --engine v2 `
  crates/jparser/tests/fixtures/jmdict_subset.xml "$scratch\subset-index-v2"
cargo run --release -p jparser --bin jparser-cli -- parse "$scratch\subset-index-v2" --engine v2 "食べさせられませんでした"
```

Expected: the JSON for the 食べる entry carries `"conjugation": "Causative Passive/Potential Polite Negative Past"` and its `steps` each carry `"aux"`. (If `parse`'s argument order differs, `cargo run ... -- parse --help` shows it; the CLI is unchanged by this phase.)

- [ ] **Step 7: Confirm both Phase 0 metrics are byte-identical (spec §8.4)**

```powershell
cargo run --release -p jparser --bin jparser-cli -- build-index --engine v2 `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml "$scratch\gold-index-v2"
$gold = "crates/jparser/tests/fixtures/gold.jsonl"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index-v2" $gold --engine v2
$app = "$env:APPDATA\dev.jparser.clipyomi"
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  score "$scratch\gold-index-v2" $gold --engine v2 --hints "$app\hints\current\system.dic"
```

Expected, from the Phase 1a handoff §2 — every number must match exactly:

| | strict | tolerant | entry top-1 | merges |
| --- | --- | --- | --- | --- |
| hints off | 0.7272 (1573/2163) | 0.7975 (1725/2163) | 0.9075 (1108/1221) | 75 |
| hints on | 0.8031 (1737/2163) | 0.8521 (1843/2163) | 0.9063 (1170/1291) | 53 |

A label cannot move a span or an `entry_id`; a difference here means the wiring did something else and is a bug to find, not a number to record. If `system.dic` is absent on this machine, the hints-on row cannot be run: say so in the commit message and the handoff rather than skipping silently.

- [ ] **Step 8: Standing gates, then commit**

Run the five standing-gate commands. Then:

```powershell
git add crates/jparser/src/engine/assemble.rs crates/jparser/tests/engine_v2.rs crates/jparser/tests/snapshots/engine_v2__sentences.snap
git commit -m "feat(engine-v2): fill Entry::conjugation on the v2 path" -m "assemble::build_entry renders label::fmt over the chain. The v2 snapshot regenerates: <K> entry lines gain a label across the 30 sentences, and no span, reading, gloss or entry order changed. jparser-cli score --engine v2 reproduces the Phase 1a handoff §2 table exactly in both hint states, as it must -- the label is invisible to both metrics."
```

Replace `<K>` with the count from Step 5, and amend the hints-on sentence if it could not run.

---

### Task 6: Documents — master plan split, fixture README, handoff

**Files:**
- Modify: `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md` (§7 Phase 1b, §10 decision log)
- Modify: `crates/jparser/tests/fixtures/README.md`
- Create: `docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md`

No code changes. Normal prose throughout.

- [ ] **Step 1: Split Phase 1b in the master plan**

In `docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md`, directly under the heading `#### Phase 1b — Formatter, class × chain gate, and the switchover` (line 826), insert:

```markdown
**Split into Phase 1b-i and Phase 1b-ii (design
[`2026-09-02-engine-v2-phase1b-i-design.md`](2026-09-02-engine-v2-phase1b-i-design.md)
§1, added 2026-09-02).** 1b-i is the formatter and its gate, landed behind
the existing `--engine v2` flag; 1b-ii is the switchover, with the In/Out/
Done-when below carried verbatim. The split exists because Phase 1a's metric
gate is open on four of eight cells and the switchover decision wants the
formatter working and those numbers in hand, and because §7.3 names the
formatter the highest-risk item — de-risked first, it leaves the switchover
a smaller commit. Phase 1's own done-when is unchanged and lands at the end
of 1b-ii.

**Phase 1b-i — complete (handoff
[`../2026-09-02-engine-v2-phase1b-i-handoff.md`](../2026-09-02-engine-v2-phase1b-i-handoff.md)).**
`Entry::conjugation` is filled on the v2 path by `engine/label.rs` from
`Entry::steps`, in the learner-standard register (Causative,
Passive/Potential, Negative, Volitional, Polite, Want-to, Past, Te-form,
Conditional, Imperative, Adverbial), with the English Wikipedia article
*Japanese verb conjugation* as the named reference. The gate is
`tests/fixtures/conjugation_labels.txt` — 218 hand-written rows: every
`(adjective?, form)` at depth 0, every `(auxiliary, form)` at depth 1, a
curated set to depth 6 — written before the formatter and run through the
real trie, plus a mechanical coverage walk to depth 2 that asserts the
fixture omits nothing and asserts no English. Both Phase 0 metrics are
byte-identical to Phase 1a's, as they must be. The grid exposed four
`ATTACHES_TO` gaps (design §7, handoff §4) that are span-shape questions and
are left for a measured 1a follow-up or 1b-ii.
```

Then replace the paragraph beginning `**Phase 1b has not started.**` (lines 859–864) with:

```markdown
**Phase 1b-ii has not started.** Design §9's budget for the switchover
stands. Whoever plans it should read Phase 1a's measured outcome above and
the 1b-i handoff's §4 table findings before deciding whether v2 is ready to
become the default — the metric gap that remains is concentrated and
understood, and four of the still-shattered or mislabelled surfaces now
have a named `ATTACHES_TO` cause.
```

In §10's decision-log table, append these rows:

```markdown
| Phase 1b split into 1b-i (formatter + gate) and 1b-ii (switchover) | 1a's metric gate is open on four cells; the switchover decision wants the formatter working and the numbers in hand, not 480 ungated lines in the relicence commit (1b-i design §1) |
| Label register is learner-standard; られる/れる is "Passive/Potential" | The product is a learner tool and §7.3's own example is in this register. Ichidan られる is ambiguous and the parser cannot resolve it, so the label says so (1b-i design §3) |
| `ConjStep.aux` records the auxiliary lemma | 一段 is shared by せる/させる/れる/られる and 不変化型 by ん/う/よう/て/で; class and form cannot name the auxiliary (1b-i design §2) |
| The label fixture is hand-written to depth 1 in full, curated deeper, with a mechanical completeness test that asserts no English | Depth 2 is 1,028 distinct tuples — past what a reviewer reads rather than skims; asserting compositional English at depth 2 would test the formatter against its own rule (1b-i design §4) |
```

- [ ] **Step 2: Document the fixture**

Append to `crates/jparser/tests/fixtures/README.md`:

```markdown
## `conjugation_labels.txt`

Hand-written, no external data source. Phase 1b-i's class × chain gate:
one row per deinflection chain the v2 table generates, with the English
label `engine/label.rs` must produce for it. Used by
`tests/conjugation_labels.rs` (every row through the real trie, exactly one
matching chain, label equal) and by `engine/table.rs`'s coverage test (every
depth-0 and depth-1 tuple reachable from a `JMDICT_POS_CLASS`-mapped class
has a row).

The English column was written from *Japanese verb conjugation*, English
Wikipedia, terminology only, **before** `label.rs` existed, and is
authoritative over it: a row the formatter cannot pass is a finding about
the formatter or the design, never a row to edit toward the code. Rows
marked `table finding` are surfaces the table generates that are not
natural Japanese; their cause is an `ATTACHES_TO` row and is recorded in
`docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md` §4.

Class and form names are `engine/table_data.rs`'s IPADIC names; regenerating
that table can rename or add forms, and the coverage test will then name
every tuple that lost its row.
```

- [ ] **Step 3: Write the handoff**

Create `docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md`. Sections and what each must contain — fill every number from this session's actual output, not from this plan:

```markdown
# Engine v2 Phase 1b-i — Handoff

**Date:** 2026-09-02
**Design:** [`specs/2026-09-02-engine-v2-phase1b-i-design.md`](specs/2026-09-02-engine-v2-phase1b-i-design.md)
**Plan:** [`plans/2026-09-02-engine-v2-phase1b-i.md`](plans/2026-09-02-engine-v2-phase1b-i.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §7 Phase 1b, §7.3
**Status:** Closed. Design §8's four done-when conditions are met (§2 below).

## 1. What exists now
   - `engine/label.rs`, `ConjStep.aux`, the fixture and its two tests, the
     regenerated snapshot. Test counts under both feature states, clippy,
     `--no-default-features`. Line counts per file against design §5's table.

## 2. Design §8's done-when, one condition at a time
   - §8.1 gate + coverage: row count, chain count walked.
   - §8.2 CLI label for 食べさせられませんでした; snapshot: how many `(-)`
     became labels, which labels appear, any that read wrong on running text.
   - §8.3 standing gates, the five commands and their outputs.
   - §8.4 the score table, both hint states, beside handoff-1a §2's — or the
     statement that hints-on could not run and why.

## 3. What the fixture decided that design §3 did not say in advance
   - Any `OVERRIDE`/`FORM_WORD` row added in Task 3 Step 3, with the fixture
     line that forced it; any row whose chain column was corrected in Task 2
     Step 4 and why; any rows added in Task 4 Step 2.
   - Design §3.2's correction: 連用デ接続's surface is ない itself (the で is
     absent), so it adds no word.

## 4. Table findings — span shape, not fixed here
   For each, the surfaces that expose it and the `ATTACHES_TO` row(s) involved:
   1. て and ない have no row requiring 連用テ接続, so 高くて, 食べなくて,
      高くない, 食べたくない never decompose as one chain.
   2. う attaches only to 未然ウ接続, so だろう, たろう, でしょう never
      decompose; よう's only chains are spurious (食べよよう).
   3. The voiced past だ row names the copula class 特殊・ダ, so 読んだら is
      unreachable and 読んな/読んなら/読んなれ/読んだっ are generated.
   4. せる/させる and れる/られる are not restricted by the inner class, so
      食べせる and 書かられる are generated.
   Plus anything the snapshot review (Task 5 Step 5) surfaced.

## 5. What Phase 1b-ii needs
   - The switchover list, verbatim from master plan §7 Phase 1b and Phase 1a
     handoff §9 (the `WordFlags`/`StemStats`/fingerprint-test relocation).
   - Whether to take the §4 table fixes first, measured against the Phase 0
     metrics, before making v2 the default.
```

Write it as prose with the real numbers; the outline above is the required content, not the final text.

- [ ] **Step 4: Sweep for stale statements**

Run: `rg -n "Phase 1b writes|1b writes the formatter|stays .None. throughout|leaves this .None.|Phase 1a leaves" crates/jparser/src crates/jparser/tests docs/superpowers/specs/2026-09-01-engine-v2-phase1a-design.md`

Every hit in `crates/jparser/src` or `crates/jparser/tests` must now be true or be reworded (`lib.rs:130`'s `Entry::steps` doc and `mod.rs`'s module doc are the likely ones). Hits in the Phase 1a design are historical and stay.

- [ ] **Step 5: Standing gates, then commit**

Run the five standing-gate commands (docs only, but the sweep in Step 4 may have touched a doc comment). Then:

```powershell
git add docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md crates/jparser/tests/fixtures/README.md docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md
git add -u crates/jparser/src
git commit -m "docs(engine-v2): close Phase 1b-i and split 1b-ii out in the master plan" -m "Records 1b-i's outcome against design §8's done-when, the fixture's authority over label.rs, the four ATTACHES_TO findings the grid exposed (span shape, left for a measured follow-up), and what 1b-ii inherits."
```

---

## Self-review against the spec

- **§1 scope** — Tasks 1–5 touch only `engine/{mod,table,chain,assemble,label}.rs`, the fixture, its test, and the snapshot. No v1 file, TS, `ATTACHES_TO`, or cost model. ✓
- **§2 `ConjStep.aux`** — Task 1. Terminal `None`, Continuation `Some(lemma)`, three test literals, chain.rs bullet. ✓
- **§3 vocabulary and rules** — Task 3's three tables and `fmt`; rule 1 (order + dedup) via `push`, rule 2 via `OVERRIDE`, rule 3 via `then`, rule 4 by the fixture's two-row surfaces. ✓
- **§4.1 fixture** — Task 2 Step 1, Parts A/B/C, header with provenance. ✓
- **§4.2 gate test** — Task 2 Step 3; exactly-one and label-equal as two tests so a chain failure is reported separately. ✓
- **§4.3 coverage test** — Task 4; asserts no English. ✓
- **§4.4 snapshot and inverted test** — Task 5 Steps 1, 5. ✓
- **§5 files** — File Structure table. ✓
- **§6 provenance** — `label.rs` header (Task 2 Step 2), fixture header (Task 2 Step 1). ✓
- **§7 findings** — Task 6 Step 3 §4, four items. ✓
- **§8 done-when** — Task 5 Steps 6–7 (§8.2, §8.4), Task 3/4 (§8.1), standing gates every task (§8.3). ✓
- **§9 master-plan edits** — Task 6 Step 1. ✓

Type consistency: `fmt(&[ConjStep]) -> Option<String>`, `aux_word(&str) -> Option<&'static str>`, `ConjStep.aux: Option<&'static str>`, `render(&[ConjStep]) -> String` used identically across Tasks 2–5.
