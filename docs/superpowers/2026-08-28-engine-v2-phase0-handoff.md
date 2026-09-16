# Engine v2 Phase 0 — Handoff (complete)

**Date:** 2026-08-28
**Design:** [`specs/2026-08-28-engine-v2-phase0-design.md`](specs/2026-08-28-engine-v2-phase0-design.md)
**Plan:** [`plans/2026-08-28-engine-v2-phase0.md`](plans/2026-08-28-engine-v2-phase0.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §5, §7 Phase 0, §9.1
**Status:** **Closed.** All nine plan tasks are done. The 150-sentence corpus is
committed at `crates/jparser/tests/fixtures/gold.jsonl` with its subset index
beside it, the baseline is recorded against it (§11), and design §14's deviations
are folded into the master plan. Master plan §9.1, §9.2, §9.6 and §9.7 are all
answered. Two items stay open and neither blocks Phase 1 — see §11.4.

---

## 1. What exists now

Thirteen commits, `0e8b1cb`..`7ed79c4` (originally `4933d91`..`765da93`, rewritten by the §10 scrub). Tests: **324** under default
features, **357** under `--features mecab`, all passing. `cargo check -p jparser
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

## 2. What was outstanding, and how it was settled

*Kept as written, because the reasoning still governs how the committed corpus
must be read.*

**Plan tasks 8 (build the corpus) and 9 (record the baseline) were not executed
by the model on its own initiative.**

Task 8 step 5 is adjudication: `label` exists to record *your* ruling on correct
Japanese segmentation. A model filling it in produces a model's opinion labelled
"gold", and since every later phase is measured against that file, the corruption
would be permanent and invisible — worse than the ranked-list tautology §7.2 was
written to prevent. Tier selection in steps 1–4 is the same judgement in weaker form.

**Settled at the author's instruction: adjudicate with the model, review it, and
commit it labelled.** The alternative was not a better corpus — it was no
baseline, and Phase 1's done-when clause cannot be evaluated without one. A full
author pass over the 246 sampled sentences is ~4,000 span decisions (§5, finding
1). What makes the promotion safe is that the risk is stated where the number is
read rather than hidden: `gold_sources.md` now carries a section naming exactly
which axis was reviewed and which was not. §11 records what was committed and
what remains unchecked.

## 3. Two defects found late, and what they change about how you label

Both were found by the whole-branch review, both corrupt the corpus rather than the
code, and both are fixed (`6676cc2`). They matter to you because they change what to
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

*Both were run — see §11 for what was committed. This section is kept because
the commands, the asset paths and the four findings at the end are what anyone
extending the corpus past 150 sentences needs.*

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

150 records, 2,163 spans. **These numbers were superseded by the review in §7**,
which found and corrected 14 wrong entry decisions; the corrected table is there.
As first recorded: 1,549 non-null, 614 null.

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

## 6. §9.6 answered: stay on IPADIC

Measured at the Phase 0/1 boundary against the 150-sentence corpus, with
unidic-mecab-2.1.2 fetched out-of-tree.

| hints | span accuracy | entry top-1 |
| --- | --- | --- |
| none | 0.6810 (1473 / 2163) | 0.9166 |
| IPADIC, field 7 | **0.6963 (1506 / 2163)** | **0.9169** |
| UniDic, field 9 (its true reading column) | 0.6958 (1505 / 2163) | 0.9151 |
| UniDic, field 7 (the naive setting) | 0.6958 (1505 / 2163) | 0.9151 |

UniDic is one span worse and slightly worse on entry top-1, for 199 MB unpacked
against IPADIC's 46 MB. The "better readings" argument does not appear in either
metric this project scores on. Revisit only if furigana quality becomes a
measured goal.

**The silent-degradation hazard is real, and the dangerous direction is the
opposite of the one §9.6 anticipated.** Measured layouts: IPADIC's surface
reading is field 7 and its unknown words get a *short* 7-field string, so the
guard fires on an absent column for 3.7% of tokens. UniDic's field 7 is the
**lemma**; its surface pronunciation is field 9.

- UniDic read at 7 drops the guard from 13.8% of tokens to 0.5% — degraded and
  silent, but the score is identical either way, so immaterial here.
- **IPADIC read at 9 disables every hint.** The guard fires on 100% of tokens
  (IPADIC strings are only 9 fields wide), and the run scores *exactly* the
  no-hints baseline of 1473 spans with no error anywhere. That silently costs the
  whole +1.5-point hint benefit, and it is the direction to fear.

`hints.rs` now resolves the column once at load time, overridable via
`TA_HINTS_READING_FIELD`, and carries a test pinning that a column the
dictionary lacks derives no flags at all rather than partial ones. That test is
the guard against this failure recurring as "hints just did not help".

A side observation worth having: the unknown-word guard is barely load-bearing.
It skips 3.7% of IPADIC tokens, and switching UniDic's guard rate between 0.5%
and 13.8% does not move either metric by a single span.

## 7. Review of the model-adjudicated corpus — 14 corrections

The author asked for a reviewable audit before deciding whether to promote
`gold_model.jsonl`. `corpus_staging/review_report.py` produces
`corpus_staging/REVIEW.md`, which reorders the same 2,163 decisions so the ones
the most spans depend on come first. Reviewing that report found **14 wrong
entry decisions**, all now corrected in the scratch tables, `gold_model.jsonl`
rebuilt, and every gate re-run.

14 of 2,163 spans is 0.6%. The number matters less than the two causes, because
both are mechanical, both were invisible to the existing self-audit, and both
would have recurred in a second pass.

### 7.1 Cause one — null reasons written against a truncated candidate list

Six of the fourteen are spans labelled null with a stated reason of the form
"the index offers only X, Y, Z". In each case the right entry was in the
candidate list, deeper than the reason's author had looked, or in a later sense
of an entry the reason had already dismissed on its first gloss.

This is the same failure `audit_entries.py` was written to catch — a decision
made from a first-sense view — but that audit only checks **non-null**
decisions, so the null side had no guard at all. It does now: the report prints
the full candidate list for every deliberate null.

| span | was | is | why |
| --- | --- | --- | --- |
| `せい` @ `a:043426-2` | null | 1610040 所為 | "consequence; result; blame" — candidate #10 of 20 |
| `たち` @ `a:000206-6` | null | 1416220 達 | pluralizing suffix — candidate #7 of 17, and 「お前たち」 is exactly it |
| `おり` @ `b:wn25517-3` | null | 1577985 居る/おる | senses 2–3 are the `aux-v`; 「としており」 is that. Candidate #6 of 11 |
| `いき` @ `a:045670-4` | null | 1578850 行く | 「いきかかる」 is 行きかかる. Candidate #8 of 16 |
| `な` @ `a:000424-6` | null | 2029110 | sense **6** is "right?; isn't it?"; the reason dismissed the entry on sense 1, "don't" |
| `な` @ `a:057582-7` | null | 2029110 | same |

Three `な` spans stay null and are still right: 「おしゃれな」「立派な」「微細な」
are the attributive, which 2029110 does not cover.

### 7.2 Cause two — a per-surface decision on a surface whose answer varies

The remaining eight are surfaces resolved once in `surface_entries.tsv` and
inherited by every occurrence, where the correct entry actually differs between
occurrences. `overrides.tsv` exists for exactly this and held four rows; it
needed twelve.

The reason this was missed is worth stating precisely, because the original
write-up presents it as a verified property. It says no surface's **candidate
list** varied between occurrences, and that is true. But a stable candidate list
says nothing about a stable correct **answer** — the list is a function of the
surface string, and the answer is a function of the context. The check that was
run could not have detected this class, and reads as though it had.

| span | was | is | context |
| --- | --- | --- | --- |
| `いった` @ `a:053148-4` | 1578850 行く | 1587040 言う | 「と一人の女房がいった」 — said, not went |
| `から` @ `a:004601-8` | 1002980 (prt) | 1245280 空 | 「胃ももうからになった」 — empty |
| `なんだ` @ `a:002722-8` | 2140410 | 2119750 何だ | 「なんだと？」 — the interjection |
| `よう` @ `a:052446-7` | 1605840 様 | 2158950 良う | 「ようございます」 — adverbial よい |
| `し` @ `b:wn44498-3` | 2086640 (prt) | 1157170 する | 「サービスを開始し、」 — 連用形 |
| `そう` @ `c1:elo-09` ×2 | 1006610 | 2137720 然う | 「そうかな」「そう思わない」 — the adverb; 1006610 is the conjectural suffix, a different item |
| `そう` @ `a:000585-5` | 1006610 | null | 「出るそうですぜ」 — hearsay. The index carries the conjectural and the adverbial and neither is this, so the corpus's own rule applies |

`し` and `から` are the ones to remember: both are high-frequency particles with
a homograph that is an ordinary content word, so the per-surface shortcut is
most attractive and most wrong exactly where it costs most.

### 7.3 What the corrections do to the numbers

Span accuracy is untouched — these are entry decisions and the segmentation did
not move. Entry top-1 shifts slightly, and **downward**, which is the honest
direction: the corpus had been giving the parser a free pass on spans it scored
as null where a real entry existed.

|  | span accuracy | entry top-1 (was) |
| --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | **0.9159** (1046 / 1142) — was 0.9166 |
| hints on | 0.6963 (1506 / 2163) | **0.9155** (1072 / 1171) — was 0.9169 |
| delta | **+0.0153** (+33 spans) | **−0.0004** — was +0.0003 |

The headline is unchanged: **hints buy +1.5 points of span accuracy and nothing
measurable on entry top-1.** The entry delta crossing zero is two spans of
movement on a 1,171 denominator and should not be read as a sign change.

Non-null spans went 1,549 → 1,554, null 614 → 609: six nulls became entries and
one entry became null.

### 7.4 Gates re-run after the corrections

- `build_gold.py` — every non-null choice is among its own span's candidate list.
- `audit_entries.py` — 262 decisions, 0 POS-consistency flags, 0 entries absent
  from the subset.
- **Design §11.4 equivalence** — `score --json` over the full 67 MB `gen-1`
  index and over the regenerated 2.66 MB subset returns identical metrics in
  both hints states. Every field matches; only `index_generation` differs, which
  is the label that makes a swapped pair detectable.
- **Design §3.4 subset size** — 2,662,806 bytes, unchanged, so 250 sentences
  still project to ~4.4 MB against the 8 MB cap.

### 7.5 What is still unreviewed, and what it would cost

The report's section 3 lists **54 of the 150 sentences** where the model cut a
boundary neither segmenter proposed, or removed one both agreed on. Those are
the model overruling its only two sources of evidence, so they carry the most
risk per decision — but unlike sections 1 and 2 there is no mechanical check
available, because the whole point is that no tokenizer has an opinion. Reading
them is ~54 sentences of genuine adjudication.

Spot-reading the top of that list, the invented cuts are defensible: splitting
`?` from `」`, splitting a mimetic from its following `と`, splitting `ゼップ`
from `・ブラッタ`. One is a real judgement call worth the author's eye —
`タイムリー・ヒット` split at `b:wn10805-2`, which is the short-unit analysis
where §5.2 states a preference for JMdict compound preservation.

The 106 null spans of four or more characters are section 2.3 of the report and
are also unchecked; they are mostly the tier C1 mimetics, where null is correct
by construction.

## 8. §9.7 answered: stay on Vibrato, and the reason is not about sudachi

Measured out-of-tree against the corrected 150-sentence corpus. Full write-up,
including the caveats, is master plan §9.7; the short version:

| source | recall | precision | F1 | gold span exact |
| --- | ---: | ---: | ---: | ---: |
| jparser (hints off) | 0.8614 | 0.8206 | 0.8405 | 0.5902 |
| **Vibrato / IPADIC** | 0.9707 | **0.8426** | **0.9021** | 0.6708 |
| sudachi Mode A | **0.9886** | 0.8073 | 0.8888 | 0.6478 |
| sudachi Mode B | 0.9856 | 0.8232 | 0.8971 | **0.6727** |
| sudachi Mode C | 0.9796 | 0.8254 | 0.8960 | 0.6603 |

Three things to carry into Phase 1.

**Mode C is the wrong mode, for the predicted reason.** §9.7 warned that Mode C
emits SudachiDict long units rather than JMdict headwords. Measured: 41 gold
spans vanish inside a longer Mode C unit — `武田`/`信玄` inside 武田信玄,
`東京都`/`新宿` inside 東京都新宿区, `JR`/`東日本`, `17`/`290` inside 17万290.
Mode C's exact-match rate lands below Mode B's *and* below Vibrato's.

**The engines fail on disjoint classes.** Mode C gets 55 gold spans exactly that
Vibrato misses, and misses 43 that Vibrato gets. The 55 are almost all
unregistered reduplicated mimetics, which sudachi's OOV handling groups into one
token where Vibrato splits them — and §4.1 above measured that class as ~65% of
real conflicts in `vn_data`. So this is not a clean loss for sudachi; it is two
engines with opposite weaknesses.

**What actually decides it is jparser's own row.** Boundary recall 0.8614
against every tokenizer's 0.97-plus. Changing hint sources is worth at most 1.8
points of recall; the distance between jparser and its existing hint source is
eleven. Whatever else Phase 1 does, it is not going to be about the tokenizer.

**Do not read that eleven points as headroom.** §9 decomposes the gap and finds
most of it is not a defect at all — it is §5.2's compound preservation being
penalised by a corpus adjudicated at short-unit granularity. Read §9 before
sizing any Phase 1 work from this table.

Tooling: `corpus_staging/measure_sudachi.py` and `sudachi_examples.py`, run
under a venv at `%LOCALAPPDATA%\Temp\claude\D--Code-oss-clipyomi\phase0\sudachi-venv`.
Raw output in `corpus_staging/sudachi_result.txt`. SudachiPy 0.6.11 is the
Python binding over sudachi.rs, not a separate port, so no parity assumption is
carried. Nothing was distributed; no sudachi code is in the tree.

## 9. The boundary gap is mostly not a gap — and the metric has a problem

§8 ended by recommending Phase 1 aim at jparser's 0.8614 boundary recall
against the hint source's 0.9707, on the reading that 266 gold boundaries were
reaching the engine and being thrown away. **That reading was wrong, and the
diagnostic written to size the work is what disproved it.**

`crates/jparser/examples/boundary_diag.rs` takes every gold boundary Vibrato has
and jparser lacks, finds the jparser segment that swallowed it, and asks two
questions: did that segment match a dictionary entry, and are the two halves
themselves matchable units. It is an example rather than a subcommand because
Phase 0 writes no code under `crates/jparser/src/`; the probe is `parse` on the
substring, so the real matcher and its conjugation recursion decide, not a raw
FST walk. Output kept at `corpus_staging/boundary_diag.txt`.

```
interior gold boundaries: 2013
jparser has:              1734  (0.8614)
vibrato has:              1954  (0.9707)
vibrato has, jparser not: 266        from 230 distinct covering spans

WHICH STAGE LOST IT              WHAT IT LOST THE BOUNDARY TO
 178  66.9%  cost model           104  39.1%  JMdict multi-word expression (pos=exp)
  52  19.5%  lexical               92  34.6%  an ordinary longer headword
  36  13.5%  fallback              21   7.9%  fallback: a mixed unmatched run
                                   18   6.8%  numeral + counter as a date headword
                                   16   6.0%  noun + する as the vs-noun headword
                                   15   5.6%  fallback: an all-punctuation run
```

### 9.1 What the merges actually are

Every one of them is a real JMdict entry that jparser found correctly:

| jparser kept | as | gold wanted |
| --- | --- | --- |
| `音がした` | 音がする [2826774] | 音 / が / した |
| `ことになっている` | ことになっている [2215350] | こと / に / なっている |
| `首を振った` | 首を振る [2253980] | 首 / を / 振った |
| `これまでに` | これまでに [2008170] | これまで / に |
| `天と地` | 天と地 [1896300] | 天 / と / 地 |
| `2月`, `18日` | ２月 [1461560], １８日 [2737790] | 2 / 月, 18 / 日 |

**This is §5.2's compound preservation working, and the corpus is penalising
it.** The corpus was adjudicated at the tokenizer's short-unit granularity —
which is the natural thing to do when the sampler shows you Vibrato's cuts — and
that standard is now baked into the metric every later phase is measured by.

Roughly 45% of the 266 (the `exp` and vs-noun merges) plus 7% (numeral +
counter) are jparser doing what the design asks. Only the 36-boundary fallback
bucket is unambiguously a defect, and it is cheap: runs of punctuation lumped
into a single unmatched segment (`」、`, `、「喜`, `丨FIFA`). Splitting unmatched
runs at script and punctuation transitions recovers most of it.

### 9.2 The problem this creates for Phase 1

The master plan's decision log already rejected boundary F1 because it
"punishes JMdict compound preservation (§5.2)". **Span accuracy against a
short-unit corpus has the same defect**, and it was not noticed because nothing
had yet decomposed a disagreement.

Phase 1's done-when clause is "both Phase 0 metrics at or above baseline". As
the corpus stands, a Phase 1 engine is rewarded for splitting `音がする` into
`音 / が / した` — for removing the compound preservation §5.2 names as the
reason the DP exists at all. Optimising this metric directly optimises against
the product.

### 9.3 Three ways out, and a recommendation

1. **Re-adjudicate at JMdict-headword granularity** wherever a JMdict entry
   covers the span. Expensive, and it edges toward the §7.2 tautology — the
   corpus would agree with the engine by construction.
2. **Add a compound-tolerant second metric** to `score`: a jparser span that
   merges several gold spans counts as correct when the merged surface is
   itself an entry the index carries. Cheap — `score.rs` already resolves entry
   ids, and it is harness code, which Phase 0 is allowed to touch. Report both
   numbers side by side and let the pair say what a single number cannot.
3. **Accept it**, read span accuracy as an explicitly short-unit measure, and
   never optimise it directly.

**Recommend 2.** It keeps the existing baseline comparable across phases, needs
no re-labelling, and it is the only option that makes the §5.2 behaviour
visible rather than merely un-punished. The two numbers together also localise
regressions: a Phase 1 change that moves strict accuracy without moving the
tolerant one has changed granularity, not correctness.

### 9.4 Built — and it costs 16.5 points

Option 2 is implemented in `score.rs`. A gold span strict accuracy scored as a
miss counts as a hit when a single **matched** segment swallows it whole and
that segment's own endpoints are both gold boundaries. Three conditions, each
load-bearing and each with a test: an unmatched fallback run is not forgiven
(it is the §9.1 punctuation-lumping defect, not an analysis), a merge that ends
inside a gold span is not forgiven (it cut one), and the strict number is still
printed beside it.

Both output forms are additive and silent when there is no coarsening — which
is itself the reading "strict and tolerant agree" — so an exact-parse corpus
serializes byte-identically to before the field existed.

```
span accuracy:  0.6810  (1473 / 2163)
entry top-1:    0.9159  (1046 / 1142)
  by decision:  e 0.6810 (1473 / 2163)
span tolerant:  0.8465  (1831 / 2163)
  coarsening:   358 gold spans inside 167 dictionary merges
```

| | span accuracy | compound-tolerant | coarsening |
| --- | --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | **0.8465** (1831 / 2163) | 358 spans, 167 merges |
| hints on | 0.6963 (1506 / 2163) | **0.8678** (1877 / 2163) | 371 spans, 173 merges |
| delta | +0.0153 | **+0.0213** | |

**16.5 points of what looked like error was benign coarsening.** The engine's
real span behaviour on a deliberately hard, disagreement-sampled corpus is
0.85, not 0.68.

**The hint benefit was being understated.** +2.13 points tolerant against +1.53
strict. §8.3's comparison — the one Phase 0 exists to produce — comes out more
strongly in favour of hints than the strict metric said, so this correction
strengthens the existing conclusion rather than revising it.

**Design §11.4 still holds, and this was the real risk.** The tolerant metric
depends on a merged surface having matched an entry, so it was not obvious the
subset index would agree with the full 67 MB one — an entry needed only for a
merge might never have been named by gold and so never kept. It agrees exactly:
`coarse_forgiven` 358 and `coarse_merges` 167 on both indexes, hints-off and
hints-on, every other field identical too. The offline gate survives the new
metric.

Tests: 324 default, 357 under `--features mecab`, both clippy gates clean.
`cli_score.rs`'s by-decision fixture turned out to be the smallest possible
demonstration of the whole finding — its gold splits `高い` into `高` / `い`
while the index carries `高い` — so it now pins both numbers.

### 9.5 Correction — a quarter of the "compound preservation" is spurious

§9.1 said the merges are all real JMdict entries jparser found correctly. That
was written from about 40 of the 230 covering spans and **it is wrong for
roughly 28 of them.** Reading all 92 of the `ordinary longer headword` bucket
turned up a coherent defect class the sample had missed.

jparser absorbs a preceding particle or fragment into the following word and
finds a kana-unified dictionary match that is semantically absurd:

| kept | matched as | actually |
| --- | --- | --- |
| `はいた` | はく [1399760] 履く, to wear shoes | は + いた |
| `ももう` | もむ [1567610] 揉む, to rub | も + もう |
| `はこ` | はこ [1585650] 箱, box | は + この |
| `がい` | がい [1204330] 甲斐, worth | が + いった |
| `になって` | になう [1599900] 荷なう, to shoulder | に + なる |
| `してい` | してい [1308000] 指定, designation | して + いた |
| `どうし` | どうし [1451380] 同士, fellow | どう + しました |
| `ことし` | ことし [1579130] 今年, this year | こと + してる |
| `クで` / `タは` / `ヌの` | くで / たは / ぬの | katakana unified into unrelated kana headwords |
| `的で` | 的 [1437260] | `目的` split as 目 + 的で |

**Vibrato has the boundary in every one of them** — that is the criterion that
put them in this set at all. So this is §4.3's "hints are advisory where they
should be decisive", with a measured example list attached. It is the strongest
Phase 1 evidence Phase 0 has produced.

Revised decomposition of the 266, hand-classified over the full list rather
than sampled:

| | count | share | verdict |
| --- | ---: | ---: | --- |
| `exp` and vs-noun MWE merges | ~120 | 45% | legitimate — §5.2 working |
| ordinary compound nouns (`副大統領`, `定期点検`, `ノーベル化学賞`) | ~64 | 24% | legitimate |
| numeral + counter (`2月`, `五日`) | 18 | 7% | legitimate |
| **spurious particle-absorbing merges** | **~28** | **10.5%** | **defect** |
| **fallback punctuation lumping** | **36** | **13.5%** | **defect** |

Real engine defect is therefore about **64 boundaries, not 36** — and the
particle-absorbing half is the interesting one, because it is a wrong answer
rather than a coarse one.

### 9.6 What this does to the tolerant metric

**It over-forgives, and now that is measured rather than suspected.** A
spurious merge is a matched segment whose endpoints are gold boundaries, so
§9.4's rule credits it. Roughly 28 of the 167 merges are of this kind, so the
0.8465 is optimistic by about 1.3 points; true span quality is nearer 0.835.

**No heuristic is going to fix this, and one should not be added.** The
distinguishing fact is whether the merged entry is semantically right, and the
corpus carries no label for a merged span — that is what makes it merged.
Every mechanical discriminator tried separates the wrong way: `音がする` and
`はいた` are both matched entries spanning gold boundaries, both swallow a
particle, and both differ from their headword by conjugation.

So the honest treatment is the one already in place — report `coarsening: N
spans in M merges` beside both accuracies, and read a rising M as a warning
rather than a win. A Phase 1 change that lifts tolerant accuracy by inflating
the merge count has probably found more spurious matches, not more compounds.
Anyone quoting the tolerant number should quote this paragraph with it.

## 10. Loose ends

- **Deferred minors, all judged safe to stand:** two `#[allow(clippy::err_expect)]` in
  `corpus.rs` (the fix is the one-word `.expect_err`); `use std::io::Write;` inside a
  loop body in `diff.rs`; `scratch()` not removing its temp dir (the repo-wide pattern,
  and it `remove_dir_all`s first).
- **`diff.rs`'s zero-token and clamp guards are unit-tested at the helper level only**,
  not through `tokenize`, because that needs the 46 MB dictionary. Call sites were
  verified by inspection.
- **Captured text in git history — scrubbed, and the documented range was wrong.**
  Two verbatim `vn_data` dialogue lines were committed in error and removed from the
  tree in `ea635e4` (was `4f9cd06`). This entry used to say they remained in
  `b2e9534`..`cca9a22`; enumerating every occurrence before the rewrite showed that
  was understated in two ways. The taint started one commit earlier, at the design
  doc `7f2d4e0`, and it reached `crates/jparser/src/bin/jparser-cli/diff.rs` as well
  as `tools/extract_vn_pool.py` and the two docs — five distinct lines across four
  files, because the character name `奈緒矢` also appeared bare, as a TSV fixture
  value, and in prose. The tree-level scrub had removed the dialogue but the name
  survived in history for several more commits.

  `git filter-repo --replace-text` has now rewritten all 401 commits. The two
  dialogue lines and the character name are gone from every reachable object;
  `git log --all -S` finds nothing. The replacements are the same invented strings
  the tree already uses (`テスト太郎`, `これはためしの文です`), so history is now
  byte-consistent with `HEAD` rather than merely clean.

  What this cost, and what it did not. `HEAD`'s tree hash is unchanged
  (`f96d2784`), so no working content moved. `develop` (`a4df417`) and `main`
  (`eea94eb`) kept their hashes and their upstream tracking, because the earliest
  rewritten commit is not an ancestor of either — only this branch's 21 commits got
  new ids. The `origin` remote is removed by `filter-repo` as a safety measure and
  has been re-added. Every commit hash cited elsewhere in these docs that fell in
  the rewrite range has been updated in the same commit that records this.

  A pre-rewrite bundle of all refs is at
  `%LOCALAPPDATA%\Temp\claude\D--Code-oss-clipyomi\phase0\pre-filter-repo-backup.bundle`.
  It contains the strings, by definition — delete it once you are satisfied.

  Nothing else in the tree is traceable to a copyrighted work.

## 11. Phase close — what was committed, and what was not

### 11.1 The corpus

`corpus_staging/gold_model.jsonl` was promoted verbatim to
`crates/jparser/tests/fixtures/gold.jsonl`: **150 records, 2,163 spans, 1,554
non-null entry decisions, 609 null**, every record's `decision` field reading
`e`. `build_gold.py` was re-run from the scratch tables and reproduces the
promoted file byte-for-byte, so the committed corpus is regenerable from its
inputs rather than being a hand-edited artefact.

`gold_jmdict_subset.xml` was regenerated against it — 5,402 entries, 633 of them
named directly by gold, **2,662,806 bytes**, unchanged from §7.4's figure. Design
§3.4's cap is 8 MB, so a 250-sentence corpus still projects to ~4.4 MB and no
capping is needed.

`gold_sources.md` gained two things: attribution rows for the 56 works and
articles the committed 150 actually cite — filtered from
`corpus_staging/manifest_rows.md`, which covers all 246 sampled sentences, so no
row claims a source the corpus does not use — and a section, **"How this corpus
was adjudicated"**, stating in plain words that the entry decisions were
author-reviewed and mechanically audited while the span decisions were not. The
four tier C1 classes are covered by that file's existing prose rather than by
rows, which is why 60 source prefixes resolve to 56 rows.

### 11.2 The baseline

Measured against the committed corpus, not the staging copy:

| | span accuracy | compound-tolerant | coarsening | entry top-1 |
| --- | --- | --- | --- | --- |
| hints off | 0.6810 (1473 / 2163) | 0.8465 (1831 / 2163) | 358 spans, 167 merges | 0.9159 (1046 / 1142) |
| hints on | 0.6963 (1506 / 2163) | 0.8678 (1877 / 2163) | 371 spans, 173 merges | 0.9155 (1072 / 1171) |
| delta | **+0.0153** | **+0.0213** | +13 spans, +6 merges | −0.0004 |

Identical to §7.3 and §9.4, which is the expected result and the point of
re-running it: the promotion moved no numbers.

Artefact hashes are in design §8.3. In short: `gold.jsonl` sha256 `6332128b…`,
subset sha256 `d445c45a…`, hint dictionary sha256 `f3ddd9ca…`
(ipadic-mecab-2.7.0-20070801).

### 11.3 Gates, all re-run on the committed artefacts

| Gate | Result |
| --- | --- |
| Design §11.4 equivalence | **Pass.** `score --json` over the full `gen-1` index and the committed subset returns identical values for every field in both hint states; only `index_generation` differs, which is the label that makes a swapped pair detectable |
| Design §3.4 subset size | **Pass.** 2,662,806 bytes against an 8 MB cap |
| `build_gold.py` | **Pass.** Every non-null choice is among its own span's candidate list; output byte-identical to the promoted file |
| `audit_entries.py` | **Pass.** 262 decisions, 0 POS-consistency flags, 0 entries absent from the subset |
| `cargo test -p jparser` | 324 passing |
| `cargo test -p jparser --features mecab` | 357 passing |
| `cargo clippy -p jparser --all-targets -- -D warnings` | Clean, with and without `--features mecab` |
| `cargo check -p jparser --no-default-features` | Clean |

### 11.4 What is not committed, and what stays unchecked

Neither blocks Phase 1; both should be visible to it.

- **Tier C2's local baseline was not built.** `gold_local.jsonl` is local-only by
  design §3.2 and outside the done-when clause, which requires a *committed*
  corpus. Until it exists there is no check that the committed corpus has not
  drifted from the real capture distribution — which is the one thing the
  committed tiers cannot verify about themselves.
- **96 of the 246 sampled sentences remain unadjudicated** in
  `corpus_staging/disagreements.jsonl`. They are the low-conflict-density tail,
  so they are also the cheapest per sentence and the least informative.
- **The 54 span decisions of §7.5 are still unreviewed.** These are the
  sentences where the model cut a boundary neither segmenter proposed, or
  removed one both agreed on — the decisions made with no evidence from either
  source, and the ones with no mechanical check available. If a Phase 1 result
  ever looks wrong, read these first: `corpus_staging/REVIEW.md` section 3 lists
  them, and at ~54 sentences against ~4,000 span decisions for a full pass they
  are by a wide margin the best risk-per-minute review available.

### 11.5 Documents brought up to date

- **Design §8.3** — the baseline table, filled with all four measurements plus
  the artefact hashes. §14 marked folded in. A broken cross-reference to a
  non-existent §8.4 corrected to §11.4.
- **Master plan §5.3** — the whole-sentence denominator and `ent_seq: null` as a
  positive label; §9.1's answer summarised; the two sampling hazards (Aozora's
  edition flag, and "cover class X" not being "write sentences containing X").
- **Master plan §5.4** — the refinement predicate, `label` reading a line rather
  than a keystroke, and candidates ordered by `ent_seq` rather than by rank.
- **Master plan §7 Phase 0** — marked complete, the "no code under
  `crates/jparser/src/`" rule amended to "no change to the parser library" with
  its two named exceptions, the baseline recorded, and the three constraints on
  how it may be read.
- **Master plan §9.1** — answered: five tiers, four committed, with the measured
  adjudication cost.
- **Master plan §10** — six decision-log rows, the first of which records the
  decision to commit a model-adjudicated corpus labelled for what it is.

