# Engine v2 Phase 0 — Handoff (harness complete, corpus and baseline outstanding)

**Date:** 2026-08-28
**Design:** [`specs/2026-08-28-engine-v2-phase0-design.md`](specs/2026-08-28-engine-v2-phase0-design.md)
**Plan:** [`plans/2026-08-28-engine-v2-phase0.md`](plans/2026-08-28-engine-v2-phase0.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §5, §7 Phase 0, §9.1
**Status:** Plan tasks 1–7 implemented and reviewed. Tasks 8–9 outstanding, deliberately.

---

## 1. What exists now

Thirteen commits, `4933d91`..`765da93` on `develop`. Tests: **320** under default
features, **351** under `--features mecab`, all passing. `cargo check -p jparser
--no-default-features` clean; `cargo clippy -p jparser --all-targets -- -D warnings`
clean with and without `--features mecab`.

| Delivered | Where |
| --- | --- |
| `jparser-cli score <index> <gold.jsonl> [--hints <dic>] [--json]` | `src/bin/jparser-cli/score.rs` |
| `jparser-cli diff <index> <pool.tsv> --hints <dic> [--all] [--limit N]` | `src/bin/jparser-cli/diff.rs` |
| `jparser-cli label <index> <disagreements.jsonl> <gold.jsonl>` | `src/bin/jparser-cli/label.rs` |
| Gold + diff JSONL formats, append-only, last-wins | `src/bin/jparser-cli/corpus.rs` |
| `Entry::entry_id` (the JMdict `ent_seq`) | `src/lib.rs` — the one additive parser-library change |
| `vn_data` pool extractor | `tools/extract_vn_pool.py` |
| JSONL + BOM + gold-`ent_seq` support in the subset extractor | `tools/extract_jmdict_subset.py` |
| Provenance manifest with the measured register classes | `crates/jparser/tests/fixtures/gold_sources.md` |

## 2. What is not done, and why

**Plan tasks 8 (build the corpus) and 9 (record the baseline) were not executed.**

Task 8 step 5 is adjudication: `label` exists to record *your* ruling on correct
Japanese segmentation. A model filling it in produces a model's opinion labelled
"gold", and since every later phase is measured against that file, the corruption
would be permanent and invisible — worse than the ranked-list tautology §7.2 was
written to prevent. Tier selection in steps 1–4 is the same judgement in weaker form.

So the harness is finished and tested; the corpus is yours to author. §5 below has the
commands.

## 3. Two defects found late, and what they change about how you label

Both were found by the whole-branch review, both corrupt the corpus rather than the
code, and both are fixed (`8890a67`). They matter to you because they change what to
watch while adjudicating.

### 3.1 `label`'s `j` key blesses the hints-off parse

`diff` parses with no hints, so a `DiffRecord`'s `jparser` cut list *is* the hints-off
segmentation. Pressing `j` copies it verbatim into gold, so that sentence scores 1.0
under `score` hints-off by construction — while `score --hints` can re-segment it and
score lower. Left alone, this biases §8.3's hints-on-minus-hints-off delta *against*
the hint mechanism, which is the one comparison Phase 0 exists to produce.

Running `diff` hints-on is **not** a fix — it would make `j` bless the hints-on parse
and inflate the other side of the same comparison. The bias is anchoring, not logic:
the sampler selects sentences precisely because jparser disagrees with Vibrato, so `j`
is legitimate whenever jparser is actually right.

What was done instead: every gold record now carries `decision` (`"j"`, `"v"` or
`"e"`), and `score` prints span accuracy split by it:

```
span accuracy:  0.8734  (2167 / 2481)
entry top-1:    0.7914  (1502 / 1898)
  by decision:  j 1.0000 (980 / 980)  v 0.7418 (612 / 825)  e 0.8506 (575 / 676)
```

**Read that line while you label.** A `j` bucket at or near 1.0000 with a large
denominator means the fast path was rubber-stamped, not that the parser is good. If
`j` dominates, the headline hints-off number is inflated by roughly its share.

### 3.2 A gold `ent_seq` absent from the index is now a hard error

Spec §12 required this and nothing implemented it; the test suite had actually pinned
the wrong behaviour. Silently scoring an absent id as a miss turns a stale-fixture bug
into apparent engine error — the failure mode is 60 stale spans dragging a baseline
from 0.79 to 0.76, a later phase regenerating the subset, the number jumping three
points, and a clean-room rewrite being credited for a fixture fix.

`score` now refuses to run and names the record and the id. Consequences for you:
`label`'s `#<ent_seq>` escape hatch validates against the index and re-prompts on a
typo, and **you must re-run `extract_jmdict_subset.py` after every append to
`gold.jsonl`** — the extractor now also keeps every `ent_seq` your gold file names, so
an entry you chose by id cannot fall out of the subset.

## 4. Findings that change the corpus design

### 4.1 Tier C1 is aimed at the wrong register

Design §3.1 guessed tier C1 should cover "casual and contracted speech". Measured over
the whole 66,114-line `vn_data` pool, that guess is wrong: contracted verb forms and
sentence-final particle stacks are common but jparser handles them as **benign
coarsening** — it merges whole Vibrato tokens rather than cutting inside one — so they
never surface as conflicts at all.

What actually breaks, from the top 100 disagreements by conflict count:

| Class | Share | Evidence strength |
| --- | --- | --- |
| Reduplicated onomatopoeia / mimetic repetition | ~65 / 100 | Strongest — Vibrato confident, 0–2 unknown-reading tokens |
| Unregistered katakana proper nouns | ~19 / 100 | Mid — Vibrato groups them without recognising them |
| Elongated interjection / scream vowel runs | ~7 primary, ~40 contributing | **Weakest** — unknown tokens never below 6, so *both* segmenters are guessing |

So tier C1 should be dominated by mimetics, not contracted speech. Full write-up with
counts and invented illustrations is in `crates/jparser/tests/fixtures/gold_sources.md`.

On the elongation class: since neither segmenter has lexical purchase there, a gold
label is your call with no tokenizer support either way. Consider labelling fewer of
them, or marking them for a second pass.

### 4.2 The sampling predicate's value, measured

Design §6.1 argued from first principles that the one-directional predicate
(`Bj \ Bv ≠ ∅`) is what keeps labelling affordable. Over the 66,114-line pool:

- A naive symmetric "the boundary sets differ" predicate flags **63,593** sentences — **96.2%** of the pool.
- The one-directional predicate flags **23,628** — **35.7%**.
- It therefore excludes **39,965** benign-coarsening records.

A manual check read a random sample of 200 of those 39,965 excluded records: all were
jparser merging whole Vibrato tokens (verb+auxiliary chains, numeral+counter, compound
nouns and names, idioms). **Zero real conflicts were hidden by the predicate.**

Note for re-verifying later: `--all --limit N` used to be useless for this, because
records rank by conflict count descending and `--all`'s extra records sort last. `--all`
now sorts ascending, so the excluded records come first and `--all --limit 200` does
what it says.

## 5. Running tasks 8 and 9

Assets already on this machine; no downloads needed:

- Full JMdict index: `%APPDATA%\dev.jparser.clipyomi\dict\gen-1`
- Vibrato dictionary: `%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`
- Gzipped JMdict source: `%APPDATA%\dev.jparser.clipyomi\source\JMdict_e.gz` (expand it outside the repo)
- `vn_data/pool.tsv` — 66,114 cleaned capture lines, gitignored

### Task 8 — build the corpus

Targets from design §3.4: 250 committed sentences (tier A 80 Aozora 新字新仮名 only,
max 8 per work; tier B 50 ja.Wikinews/Wikipedia; tier C1 80 pastiche — **mimetics-led,
per §4.1 above**; tier D 40 hand-derived noise), plus ~100 local tier C2 lines from
`vn_data` in the gitignored `gold_local.jsonl`.

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"

# sample a pool (tiers A/B/C1 pools you author; C2 uses vn_data/pool.tsv)
cargo run --release -p jparser --features mecab --bin jparser-cli -- `
  diff "$app\dict\gen-1" <pool.tsv> --hints "$app\hints\current\system.dic" > disagreements.jsonl

# adjudicate — appends, resumable, re-labelling supersedes
cargo run --release -p jparser --bin jparser-cli -- `
  label "$app\dict\gen-1" disagreements.jsonl crates/jparser/tests/fixtures/gold.jsonl

# regenerate the subset after EVERY append (see §3.2)
python tools/extract_jmdict_subset.py <expanded JMdict_e.xml> `
  crates/jparser/tests/fixtures/gold.jsonl `
  crates/jparser/assets/conjugations.json `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml
```

Keys: `j` jparser spans, `v` Vibrato spans, `e` retype the interior cuts, `s` skip,
`q` quit; then per span a numbered pick, `n` for no entry, `#<ent_seq>` by id. Skip
freely — an unlabelled sentence costs a denominator entry, a guessed one costs the
baseline's meaning.

**Check the subset size at 100 labelled sentences.** If 2.5× the printed `output bytes`
exceeds 8 MB, cap the corpus and record the cap (design §3.4).

### Task 9 — record the baseline

```powershell
$gold = "crates/jparser/tests/fixtures/gold.jsonl"
cargo run -p jparser --bin jparser-cli -- build-index `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml <scratch>\gold-index

# the four numbers, plus the same pair over gold_local.jsonl against the full index
cargo run --release -p jparser --features mecab --bin jparser-cli -- score <scratch>\gold-index $gold
cargo run --release -p jparser --features mecab --bin jparser-cli -- score <scratch>\gold-index $gold --hints "$app\hints\current\system.dic"
```

Then **the equivalence check of design §11.4**, which is the one gate that cannot be
replaced by reasoning: `score --json` the committed corpus against the full index and
against the subset index, hints-on and hints-off, and confirm all four pairs are
identical. If they differ, §8.2's argument is wrong and the offline gate is not met —
that difference is the finding; do not adjust anything. `--json` now records the hints
flag and the index generation, so a swapped pair is detectable.

Finally: fill design §8.3's table, and fold the §14 deviations into the master plan
(§5.3, §5.4, §7, §9, §10) — including the two new rows this run earned, the measured
predicate value in §4.2 above and the tier C1 correction in §4.1.

## 6. Loose ends

- **Deferred minors, all judged safe to stand:** two `#[allow(clippy::err_expect)]` in
  `corpus.rs` (the fix is the one-word `.expect_err`); `use std::io::Write;` inside a
  loop body in `diff.rs`; `scratch()` not removing its temp dir (the repo-wide pattern,
  and it `remove_dir_all`s first).
- **`diff.rs`'s zero-token and clamp guards are unit-tested at the helper level only**,
  not through `tokenize`, because that needs the 46 MB dictionary. Call sites were
  verified by inspection.
- **Captured text in git history.** Two verbatim `vn_data` dialogue lines were committed
  in error and scrubbed in `4f9cd06`; no tracked file carries them now. They remain in
  commits `b2e9534`..`cca9a22`. Since relicensing and publishing is Engine v2's whole
  purpose, decide deliberately whether to `git filter-repo` those strings before any
  publish. Nothing else in the tree is traceable to a copyrighted work.
