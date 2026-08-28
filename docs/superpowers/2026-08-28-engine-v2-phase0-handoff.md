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

**The pools are already built.** `corpus_staging/` (gitignored, on this machine)
holds a 612-line pool across all four tiers and a ranked `disagreements.jsonl`
with **246 sampled sentences** — essentially the design's 250-committed target.
`corpus_staging/README.md` is the guide; start there. Provenance rows for every
sentence are pre-written in `corpus_staging/manifest_rows.md`, to paste into
`gold_sources.md` once you know which sentences survived adjudication (a skipped
sentence must not leave an orphaned attribution row).

| tier | pool | sampled | design target | median spans |
| --- | --- | --- | --- | --- |
| A Aozora, 新字新仮名, 40 works/40 authors | 320 | 104 | 80 | 16 |
| B ja.Wikinews, CC BY 2.5, 48 articles | 111 | 53 | 50 | 23 |
| C1 hand-written pastiche | 123 | 73 | 80 | 9 |
| D derived noise | 58 | 16 | 40 | 22 |
| **all** | **612** | **246** | **250** | **14** |

So the only step left is adjudication:

```powershell
$app = "$env:APPDATA\dev.jparser.clipyomi"
cargo run --release -p jparser --bin jparser-cli -- `
  label "$app\dict\gen-1" corpus_staging\disagreements.jsonl `
  crates\jparser	estsixtures\gold.jsonl
```

Keys: `j` jparser spans, `v` Vibrato spans, `e` retype the interior cuts, `s`
skip, `q` quit; then per span a numbered pick, `n` for no entry, `#<ent_seq>` by
id. Skip freely — an unlabelled sentence costs a denominator entry, a guessed one
costs the baseline's meaning. Records are ranked by conflict density, so stopping
early costs the least.

**Regenerate the subset after every append** — `score` now hard-errors on a gold
`ent_seq` the subset does not carry (§3.2):

```powershell
python tools/extract_jmdict_subset.py `
  "$env:LOCALAPPDATA\Temp\claude\D--Code-oss-clipyomi\phase0\JMdict_e.xml" `
  crates/jparser/tests/fixtures/gold.jsonl `
  crates/jparser/assets/conjugations.json `
  crates/jparser/tests/fixtures/gold_jmdict_subset.xml
```

Also still to do for task 8: ~100 local tier C2 lines from `vn_data/pool.tsv`
into the gitignored `gold_local.jsonl`, and the subset-size check at 100 labelled
sentences (design §3.4 — if 2.5x the printed `output bytes` exceeds 8 MB, cap the
corpus and record the cap).

#### A model-adjudicated pass over the top 150 exists, for review

At the author's instruction the top 150 were adjudicated by the model. The
result is in `corpus_staging/gold_model.jsonl` and **deliberately not** in
`crates/jparser/tests/fixtures/gold.jsonl`: `label` exists to record the
author's ruling on correct Japanese segmentation, and a model's opinion must not
be mistakable for the baseline every later phase is measured against. Review it
before promoting; the numbers below are provisional on that review.

150 records, 2,163 spans, 1,549 non-null, 614 null.

|  | span accuracy | entry top-1 |
| --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | 0.9166 (1044 / 1139) |
| hints on | 0.6963 (1506 / 2163) | 0.9169 (1070 / 1167) |
| delta | **+0.0153** (+33 spans) | +0.0003 |

- **68% is not a quality verdict.** Every sentence here is one the two
  segmenters disagreed on, so the corpus is deliberately hard. The number exists
  for cross-phase comparability.
- **The hint mechanism measures positive**, +1.5 points of span accuracy — the
  §8.3 comparison Phase 0 exists to produce, and it favours hints.
- **`by decision` reads `e` for all 150**, because neither segmenter was right
  on any sampled sentence. So the §3.1 anchoring bias is structurally absent
  here, not merely measured. Watch that line if your own pass uses `j`.
- Entry decisions came from the ent_seq-sorted candidate lists, never from the
  ranker's top pick. 990 ambiguous spans reduced to 267 distinct surfaces, none
  of whose candidate lists varied by occurrence; nine surfaces are null because
  the index has no correct entry and the nearest miss would have been a lie.

**The entry decisions were self-audited.** `corpus_staging/audit_entries.py`
re-checks all 258 non-null choices against the full JMdict entries rather than
the first-sense gloss the candidate list displayed, plus a mechanical
POS-consistency test. **0 real errors**; the five flags the first run produced
were all the expectation table being wrong (JMdict tags 月 as `n-suf`, 零 and 円
as nouns, じゃん as `prt`, なさい as `aux-v`). It vindicated the riskiest picks —
`と` → 1008490, whose quotative sense is #4 and was never displayed. Re-run it
after any edit to `surface_entries.tsv`: 267 rows feed 990 spans, and `の` alone
reaches 111 of them.

**Two design gates were run for the first time and both pass.** §11.4's
equivalence check: `score --json` over the full 67 MB index and over the
subset returns byte-identical metrics, hints-on and hints-off. §3.4's
subset-size check: 150 sentences give a 2.66 MB subset, so 250 projects to
~4.4 MB, under the 8 MB cap — no capping needed.

#### Four findings from pre-staging, which change your expectations

1. **Adjudication costs ~60% more than the design estimated.** §3.4 assumed ~10
   spans per sentence; the measured median is 14 and the mean 16.3, so labelling
   all 246 is **~4,000 span decisions**, not 2,500. Not all are equal — a
   single-character particle is a fast `n` — but the count is real. Levers:
   stop at the top ~150 by conflict density (~2,400 decisions, still covers every
   tier), lower `MAX_CHARS` in the tier A/B fetchers, or shift the mix toward C1,
   which is by far the cheapest tier per sentence.
2. **Tier C1's first draft missed the class it was written for.** The vn_data
   measurement said reduplicated mimetics were ~65% of conflicts, so the first 40
   lines used ordinary mimetic adverbs (どきどき, ぶるぶる, ぐるぐる) — and only
   **12%** sampled, because those are JMdict headwords and both segmenters agree.
   The conflicting shape is a short mora unit repeated *many* times with little
   kanji anchor, ideally one JMdict lacks. Rewriting took mim to 35% and C1 to
   59%. General lesson: "cover class X" is not "write sentences containing X",
   and only the sampler tells you which you did.
3. **Aozora's 新字新仮名 flag describes the edition, not the content.** Two leaks
   beyond §3.3's warning, both now filtered: classical passages quoted inside a
   modern edition (宮沢賢治's 二十六夜 carries a pseudo-sutra in kanbun style),
   and kana iteration marks (`慌てゝ`, `こゝへ`). Also, a work usually has several
   catalogue rows and the 旧字旧仮名 edition often sorts first — resolving a title
   to "the first matching row" silently violates §3.3.
4. **Most capture artefacts do not change segmentation.** Tier D samples at only
   28%: halfwidth `?`, `！`→`!` and `，` mostly leave the parse identical. The OCR
   confusables and the spurious-space transform are what actually perturb it. Tier
   D is also capped at 3 lines per transform by design, which ceilings it at 58
   derived and 16 sampled — reaching the 40-sentence target means raising
   `MAX_PER_TRANSFORM`. One class is deliberately uncovered: 卜/ト is a real
   confusable but 卜 is vanishingly rare in prose, and a contrived sentence to
   exercise it would be worse than saying so.

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
