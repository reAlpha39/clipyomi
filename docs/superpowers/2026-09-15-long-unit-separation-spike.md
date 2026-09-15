# The long-unit separation spike — §12.5 answered, and closed

**Date:** 2026-09-15
**Branch:** `feat/engine-v2`
**Probe:** `corpus_staging/long_unit_separation.py` (untracked — see §6)
**Answers:** [`2026-09-09-frequency-signal-candidates.md`](2026-09-09-frequency-signal-candidates.md) §12.5,
carried forward as the first of the two spikes in the
[Phase 3c handoff](2026-09-12-engine-v2-phase3c-handoff.md) §5.
**Produces:** a recommendation. No engine, index, wire or settings change and
no new dependency — `corpus_staging/` is ignored, so nothing entered the tree
but this document and two amendments to the documents it answers.

## 1. The question, and why it was still open

§12 ruled a frequency layer out on both buildable corpora: every count-derived
ranking scored AUC ≤ 0.51 held out, and two signals needing no corpus at all
beat all of them. §12.5 then recorded the one route the measurements left
open, and was careful to call it a route rather than a recommendation:

> The counts do say what *would* separate the two groups: a frequency table
> over a **long-unit** token inventory — one in which `副大統領` is a token and
> `には` is not. Then `tok_exact` is positive for the compounds and zero for
> the particle stacks … The missing piece is the **unit**, and a unit is a
> property of a segmenter's inventory, not of text.

That claim was never tested, and it is the last thing standing between the
compound-preservation thread and being closed. If it holds, compound
preservation has a Phase 4 — adopt a second segmenter whose long-unit
inventory is itself the signal, with its own size and licence gates. If it
does not, the thread ends where §12 left it.

**This spike tests the bound rather than the claim**, which is cheaper and
decides the same question. A count over an inventory can only separate two
classes if the inventory separates them. So: hand the adjudicated type list to
a real long-unit inventory and ask whether its own units tell `lexical` from
`transparent` + `false-match`. If they do not, no table built over those units
can, and no corpus needs tokenizing to find that out.

## 2. What was measured

The long-unit inventory is **SudachiDict Mode C**, reached through SudachiPy
exactly as master plan §9.7's `corpus_staging/measure_sudachi.py` reaches it —
out of tree, in a scratch venv, nothing distributed and no sudachi code
entering the repository. Mode C is Sudachi's *named-entity* granularity, the
longest of its three, and it is the concrete article §12.5 is describing:
`副大統領` is one Mode C unit, and §9.7 already measured 41 gold spans
disappearing inside Mode C units precisely because its vocabulary is not
JMdict's.

The fixture is `crates/jparser/tests/fixtures/compound_gold.jsonl` unchanged —
194 windows over 139 adjudicated types, 87 `lexical` / 22 `transparent` /
23 `false-match` / 7 `ambiguous`, which is §13.1's table to the digit.

Four readings per type, each scored the way §11.3 scores every other
candidate — AUC over the classes, and the best threshold's gain and cost in
windows against the blanket discount that needs no signal at all:

| reading | what it asks |
| --- | --- |
| `modec_exact` | Mode C emits the gold span as exactly one token. This is §12.5's `tok_exact` with the unit swapped, and the one the claim is about |
| `modec_uncut` | Mode C puts no boundary strictly inside the gold span. Weaker: it also holds when the span vanishes inside a *longer* unit |
| `modec_lemma` | a Mode C token begins where the gold span does and its dictionary or normalized form is the gold headword. `exact` asks after the inflected surface, which no inventory holds; this asks whether the inventory holds the **headword** — `となった` is a dictionary entry nowhere, `となる` is |
| `modec_alone` | the surface tokenized on its own is one Mode C token. Context-free, so it reads the inventory rather than the analysis, and it is the shape a build-time flag over a long-unit dictionary would have |

**Ground check first.** `measure_sudachi.py` was re-run before the probe and
reproduced master plan §9.7's table to every printed digit — jparser 0.8614 /
0.8206 / 0.8405, sudachi C 0.9796 / 0.8254 / 0.8960, 1,972 of 2,013 boundaries
hit — on **SudachiDict 20260723**, a newer release than Phase 0 read. The
inventory has not moved under the measurement, so the figures below are
comparable to §9.7's.

## 3. The result

```
per class, share of windows Mode C emits the gold span as one token:
  lexical      exact   9/112   uncut   9/112   lemma  11/112
  transparent  exact   0/ 42   uncut   0/ 42   lemma   1/ 42
  false-match  exact   0/ 32   uncut   0/ 32   lemma   0/ 32
  ambiguous    exact   0/  8   uncut   0/  8   lemma   0/  8

separation (positives = lexical, negatives = transparent + false-match):
  scalar            AUC   types +gain/-cost   windows +gain/-cost  net
  modec_exact     0.546    87/45               112/74             +38   at >= 0.0
  modec_uncut     0.546    87/45               112/74             +38   at >= 0.0
  modec_lemma     0.546    87/45               112/74             +38   at >= 0.0
  modec_alone     0.518    87/45               112/74             +38   at >= 0.0

  blanket             -    87/45               112/74             +38
```

**§12.5's prediction holds on its negative half and fails on its positive
one.** Mode C never emits a `transparent` or a `false-match` span as a token —
0 of 42 and 0 of 32, and 0 of 32 even under the lemma reading, which is
cleaner than any corpus-derived scalar §12 tested. That half of the sentence
is exactly right, and it is why the AUC is above 0.5 at all.

But it emits only **9 of 112 `lexical` windows** as a token. The positive half
of the prediction — "`tok_exact` is positive for the compounds" — is false for
92 % of them. And a scalar that is zero on almost every positive is not a
ranking: every reading's best threshold is `>= 0.0`, which admits everything,
which **is the blanket**. Net +38 for the signal and net +38 for no signal at
all. The long-unit inventory buys nothing over merging everything, which is
the same verdict §12 reached for the two corpora and for the same structural
reason one level up.

### 3.1 Why, and it is not a Sudachi failure

The nine are these, and they have one shape between them:

```
タイムリーヒット (×2)   何時も   黒服   月平均
副大統領   名誉教授   ラストラン   定期点検
```

Noun-noun compounds, every one. What the other 103 `lexical` windows hold is
`音がする`, `音を立てる`, `風を切る`, `首を振る`, `ことになっている`,
`これまでに`, `となった`, `である`, `によると` — the class
`compounds.rs`'s own doc comment calls "a fixed grammatical construction whose
meaning is not its parts stacked". **No segmenter's inventory holds a
construction, at any unit length**, because a construction is not a word; it
is a pattern with slots, and the slots are filled by inflection and by
arbitrary nouns. Mode C is doing its job correctly and the job is the wrong
one.

So §12.5's diagnosis was half right in a way that matters. It said the missing
piece was the unit rather than the corpus, and it was right that no corpus
supplies it. What this measures is that the unit does not supply it either,
because the property the compound metric rewards is **constructionhood**, and
wordhood does not measure constructionhood at any granularity. That is a
stronger and more final statement than §12's, and it closes the thread rather
than moving it.

### 3.2 The nine, priced against the engine as it stands

The nine windows are not worthless, and the spike prices them rather than
waving at them. Measured at the Phase 3c gate baseline — gold index rebuilt
from `gold_jmdict_subset.xml` (5,402 entries, 9,930 keys, 13,898 records),
hints on, `score --compounds --compounds-detail` reproducing **0.8174 /
0.8525 / 0.9092, compounds 0.6129, `lexical` 42 / 112** to the digit:

- **Seven of the nine are windows the parse splits today**, so they are the
  achievable move: `黒服`, `月平均`, `副大統領`, `名誉教授`, `ラストラン`,
  `定期点検`, and one of the two `タイムリーヒット` windows. The other two —
  `何時も` and the second `タイムリーヒット` — the engine already keeps whole,
  so a long-unit signal would re-decide them the same way and buy nothing.
- **Zero cost on the negative classes**, because Mode C is 0 / 42 and 0 / 32
  there. Whatever else is true, this signal would not ship a wrong merge on
  this corpus.
- **And the short-unit gold splits all seven into exactly two spans each.**
  `副` + `大統領`, `名誉` + `教授`, `定期` + `点検`, and so on — verified per
  window against `gold.jsonl`, and verified again against the parse: the
  engine hits **all fourteen** of those inner gold spans today.

So the trade is **+7 compound windows (114 → 121 of 186, 0.6129 → 0.6505) for
−14 strict spans (1768 → 1754, 0.8174 → 0.8109, −0.65 points)**. Tolerant is
unchanged by construction — the fourteen lost spans become coarsening inside
seven new dictionary merges, which is what that metric credits, so it stays
1,844 with the merge count going 38 → 45. (The strict and compound figures are
measured; the tolerant reading is derived from the metric's definition, since
no such signal was implemented.)

That trade has the same sign as the one §10.3 priced at about −6.9 points
strict and the plan declined. It is much smaller, and it is still the wrong
side of the comparison that matters: **`FUNCTION_STACK`, the derivation that
did ship, moved strict +31 *and* compounds +13 with no class regressing.**
A candidate that has to buy compound windows with strict spans is not in that
league, and §5.2's whole reason for reporting the two metrics separately is
that a change moving them in opposite directions has changed granularity
rather than improved.

### 3.3 The price of admission, for completeness

Even had the separation been there, the adoption cost was going to be read
before anything shipped, so it is recorded:

- **`SudachiDict core`'s `system.dic` is 217,466,039 bytes** — 207 MiB,
  against the 47.8 MiB Vibrato/IPADIC dictionary the app publishes today.
  4.5× the download, for a signal whose ceiling is seven windows.
- **Apache-2.0 with no MIT leg**, for both `sudachipy` and `SudachiDict-core`
  as their installed metadata states, which is the same footing master plan
  §9.7 recorded for `sudachi.rs` itself. That is compatible with this
  project's `MIT OR Apache-2.0` target — §9.2's reason for choosing that
  target was precisely to keep the door open — so licence was never the
  blocker here. Size and measurement are.
- **§9.7 had already declined sudachi.rs on quality**, in both directions:
  higher boundary recall in every mode, lower precision in every mode, worse
  F1, and Mode C worse than Mode B because its long units swallow JMdict
  headwords. Nothing in this spike disturbs that, and this spike is not a
  reopening of it: it borrows Sudachi's *inventory* as a measuring instrument,
  not as a candidate hints source.

## 4. Recommendation

**Close §12.5. Compound preservation gets no Phase 4 on this route, and the
frequency-and-wordhood thread is finished.**

Concretely:

1. **Do not adopt a second segmenter for a long-unit signal.** The inventory
   does not separate the adjudicated classes (AUC 0.546 at best, best
   threshold identical to the blanket on every reading), its achievable move
   is seven windows, and those seven cost fourteen strict spans and 207 MiB.
2. **Do not build a long-unit frequency table.** It was the next step only if
   this one passed; a count over an inventory that does not separate cannot
   separate. Both corpora stay where §12 left them, built and scored and
   ruled out, and `corpus_staging/freq/` needs nothing further.
3. **`FUNCTION_STACK` stands as the last derivation in this thread**, and as
   the shape any future candidate has to beat: both span metrics up, no class
   down, no new data and no new dependency.
4. **What is *not* closed** is compound preservation as an engine question.
   `lexical` is 42 of 112 and the 103 construction windows are still the gap;
   what this spike rules out is one particular class of answer — a signal
   bought in from a word inventory or a word count. Anything that reaches
   those windows has to model the construction, which is a lattice-and-cost
   question inside the engine, not a data-acquisition question outside it.
   That is a different premise from Phase 3's and is not scoped here.

The second spike, the pitch-accent source hunt (UniDic's accent fields under
the BSD leg of its triple licence, and Wadoku under CC BY-SA 3.0), is
untouched by any of this and remains open.

## 5. Reproducing

```
# out-of-tree venv; nothing is added to the project's dependencies
python -m venv <scratch>/sudachi-venv
<scratch>/sudachi-venv/Scripts/python -m pip install sudachipy sudachidict_core

# ground check: reproduces master plan §9.7's table
<scratch>/sudachi-venv/Scripts/python corpus_staging/measure_sudachi.py

# the spike; writes corpus_staging/long_unit_types.txt beside its summary
<scratch>/sudachi-venv/Scripts/python corpus_staging/long_unit_separation.py

# the baseline the pricing in §3.2 is read against
jparser-cli build-index crates/jparser/tests/fixtures/gold_jmdict_subset.xml <scratch>/gold_index
jparser-cli score <scratch>/gold_index crates/jparser/tests/fixtures/gold.jsonl \
  --compounds crates/jparser/tests/fixtures/compound_gold.jsonl --compounds-detail \
  --hints %APPDATA%\dev.jparser.clipyomi\hints\current\system.dic
```

`corpus_staging/long_unit_types.txt` carries the per-type detail — class,
windows, and each of the four readings — so the verdict above can be read off
the 139 cases rather than off the summary.

## 6. Open items

- **The probe and its detail file are not in the repository.**
  `corpus_staging/` is ignored in its entirety (`.gitignore:45`), as it was
  for `separation.py`, `measure_sudachi.py` and every other probe this thread
  produced, and this spike did not make an exception for itself. That is why
  §3.1 quotes all nine cases and §3 quotes the probe's output verbatim: this
  document is the artefact, and a reader checking the verdict should not need
  the file or a venv to do it.
- **The probe reads one inventory.** UniDic's own long-unit layer (`luw`) and
  JUMAN's are not tested, and neither is a hand-built long-unit list. The
  reason for stopping is §3.1 rather than §3: the 103 missing windows are
  constructions, and no *word* inventory holds them, so a second inventory
  would be a different measurement of the same structural fact. If that
  reading is ever disputed, the probe takes a new tokenizer in one function.
- **`何時も` is `lexical` and also a Mode C token**, and the engine already
  merges it — but the short-unit gold splits it `何時` + `も`, so it is one of
  the places where the two adjudications genuinely disagree and the engine
  follows the compound one. Nothing here changes that; it is noted because it
  is why the achievable move is seven rather than nine.
