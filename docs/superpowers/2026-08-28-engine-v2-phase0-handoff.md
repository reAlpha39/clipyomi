# Engine v2 Phase 0 — Handoff (harness complete, corpus and baseline outstanding)

**Date:** 2026-08-28
**Design:** [`specs/2026-08-28-engine-v2-phase0-design.md`](specs/2026-08-28-engine-v2-phase0-design.md)
**Plan:** [`plans/2026-08-28-engine-v2-phase0.md`](plans/2026-08-28-engine-v2-phase0.md)
**Master plan:** [`specs/2026-08-27-engine-v2-master-plan.md`](specs/2026-08-27-engine-v2-master-plan.md) §5, §7 Phase 0, §9.1
**Status:** Plan tasks 1–7 implemented and reviewed. Task 8's pools are built and
the top 150 are model-adjudicated and now **author-reviewed** (§7 — 14 corrections
applied). Promotion to `crates/jparser/tests/fixtures/gold.jsonl` is still the
author's call, and until it happens Phase 0 is not closed: its done-when clause
requires a *committed* corpus. Master plan §9.1, §9.2 and §9.6 are all answered.

---

## 1. What exists now

Thirteen commits, `0e8b1cb`..`7ed79c4` (originally `4933d91`..`765da93`, rewritten by the §9 scrub). Tests: **320** under default
features, **353** under `--features mecab`, all passing. `cargo check -p jparser
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
against every tokenizer's 0.97-plus: jparser is throwing away 279 gold
boundaries the hint source already hands it. Changing hint sources is worth at
most 1.8 points of recall. Using the existing ones as §4.3 describes is worth
eleven. That is the Phase 1 target, and it is where the §7.3 recombiner and the
new cost model should be aimed.

Tooling: `corpus_staging/measure_sudachi.py` and `sudachi_examples.py`, run
under a venv at `%LOCALAPPDATA%\Temp\claude\D--Code-oss-clipyomi\phase0\sudachi-venv`.
Raw output in `corpus_staging/sudachi_result.txt`. SudachiPy 0.6.11 is the
Python binding over sudachi.rs, not a separate port, so no parity assumption is
carried. Nothing was distributed; no sudachi code is in the tree.

## 9. Loose ends

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
