# The last six corpora, a control for the ratio, and three new classes — handoff

**Date:** 2026-09-21
**Branch:** `feat/corpus-disagree`, tip `43fa8dc`
**Predecessor:** [`2026-09-21-four-corpora-stray-kana-handoff.md`](2026-09-21-four-corpora-stray-kana-handoff.md)

The predecessor ran four corpora and left six in `vn-text` deliberately unrun.
This session ran all six — 56,065 sentences, none of them seen before — and
`vn-text` is now exhausted. Three things came out of it that the predecessor
could not have had: a **control** that says the merge-direction ratio moved
because the engine moved rather than because the writers changed, a **held-out
replication** of `640074c` against a pre-fix binary, and **three defect classes
that no previous session named**.

§1 to §7 are the measurement, written before anything was changed: no code, no
gate, and every artifact throwaway — the v1 index, the dumps and the pre-fix
worktree are gone with the session scratchpad. §8 is the one change the
measurement justified, shipped the same day — `4bc74b4`, §4a's class, which
turned out to be a class assignment rather than the gloss it looked like.

## 1. The corpora, and how to rebuild the run

Same recipe as the predecessor: table `artificialtrans`, column **`source`**,
distinct non-empty rows with whitespace stripped and internal newlines removed.
Re-dumping `kf3FHD` with it returns exactly **21,224**, the predecessor's
number, which is how the recipe was validated for a third time before anything
else ran.

| corpus | file | distinct sentences |
| --- | --- | --- |
| kf3if | `kf3if_sFHD_1760706903…sqlite` | 16,183 |
| pxc2 | `PxC2_1777642779…sqlite` | 10,499 |
| ikenaidx | `ikenaiDX_1765684285…sqlite` | 9,559 |
| enkou | `enkou_1754784961…sqlite` | 8,054 |
| onikiss | `おにキスＨ_1758981216…sqlite` | 6,517 |
| hitoduma2 | `hitoduma2_1783752238…sqlite` | 5,253 |

56,065 in total. **Every file in `D:\Code\oss\vn-text` has now been run at least
once** — these six here, `Start` / `amakano` / `kf3FHD` / `triple` in the
predecessor, `riruka` and `恋愛、はじめまして` two sessions back. `kf2` is still
`vn_data/KF2ifFHD_…sqlite` and is the only corpus outside that directory.

Copyrighted game script: local probe input only, and nothing derived from it is
in this repository.

The v1 side is produced out of tree as before, from the detached worktree at
`D:\Code\oss\clipyomi-v1` (`361a77f`, with its untracked `examples/v1_dump.rs`).
No v1 index survives on disk, so it was rebuilt from a scratch copy of
`%APPDATA%\dev.jparser.clipyomi\source\JMdict_e.gz` with
`build-index --engine v1`: **486,553 keys, 619,717 records, 121,048 exact stems,
398 v5-fallback, 5 empty** — identical to the predecessor's rebuild, which is
the second thing validated before the run.

`corpus_disagree` was run at `--top 6000` for the reason the predecessor's §7
gives. The shape pass is `span_dump` against `gen-3` (format 6) with the same
hints dictionary.

## 2. The merge direction, and the control that explains the shift

| corpus | state | we merge, v1 splits | we split, v1 merges | ratio |
| --- | --- | --- | --- | --- |
| pxc2 | on | 1,474 | 2,109 | 1.43× |
| enkou | on | 1,360 | 3,016 | 2.22× |
| hitoduma2 | on | 1,063 | 1,828 | 1.72× |
| ikenaidx | on | 1,857 | 3,079 | 1.66× |
| kf3if | on | 1,664 | 2,825 | 1.70× |
| onikiss | on | 842 | 1,794 | 2.13× |
| **all six** | **on** | **8,260** | **14,651** | **1.77×** |
| pxc2 | off | 1,297 | 3,541 | 2.73× |
| enkou | off | 1,179 | 4,557 | 3.87× |
| hitoduma2 | off | 771 | 3,126 | 4.05× |
| ikenaidx | off | 1,372 | 5,498 | 4.01× |
| kf3if | off | 1,304 | 4,590 | 3.52× |
| onikiss | off | 711 | 2,891 | 4.07× |
| **all six** | **off** | **6,634** | **24,203** | **3.65×** |

Regions, not occurrences. Hints-on reads **below** the predecessor's band of
2.2–2.5×, and the obvious reading — a different set of writers — is the wrong
one.

**The control.** `kf3FHD` was re-run on the current engine, same corpus, same
state, same v1 dump recipe:

| kf3FHD, hints-on | we merge, v1 splits | we split, v1 merges | ratio |
| --- | --- | --- | --- |
| predecessor §2, before `640074c` | 1,837 | 4,194 | 2.28× |
| this session, after it | **2,178** | **4,148** | **1.90×** |

+341 merge regions and −46 split regions on text that did not change. The whole
shift is `640074c` turning `〜てくる` / `〜ておく` / `〜てみる` splits into merges,
and 1.90× sits inside this session's own 1.43–2.22 band. **Anyone comparing a
ratio across sessions has to re-run one old corpus first**, or they are reading
two changes as one.

Over eleven corpora and roughly 207,000 sentences the sign has still never
changed.

67 surfaces appear in all six hints-on heads and 46 in all six hints-off. Those
totals are not comparable with the predecessor's 556 and 370, which were taken
over four corpora.

## 3. `640074c` replicated, held out, against a pre-fix binary

The predecessor's §9 measured its own change on the corpora it was written
against. This session built the **pre-fix engine** — a throwaway worktree at
`eff06b6`, `span_dump` only, removed afterwards — and ran both binaries over
the six unseen corpora:

| class | windows | cut before | cut now, on | cut now, off |
| --- | --- | --- | --- | --- |
| てくる | 1,437 | 1,287 (89.6%) | **15 (1.0%)** | 39 (2.7%) |
| ておく | 92 | 87 (94.6%) | **6 (6.5%)** | 3 (3.3%) |
| てみる | 157 | 157 (100%) | **4 (2.5%)** | 0 (0%) |
| ていく | 1,410 | 12 (0.9%) | 12 (0.9%) | 33 (2.3%) |

`ていく` is the control column and is **identical, 12 against 12**, on text
neither build was tuned against.

The collision guards hold too, which is the part that could only be checked on
new text: **`てくださ` 355 occurrences and 0 merged** (320 of them `てください`),
**`てみせ` 57 and 0 merged**. The merges that should happen do: `ておい` 93 of
93, `てみな` 18 of 23 — `してみない`, `やってみない`, the negative of the
auxiliary rather than the window's noise.

## 4. Three classes no previous session named

### 4a. `〜てくれる` read as `〜てくる`

**Fixed in `4bc74b4`, and the cause was not what this section guessed — see
§8.** What follows is the measurement as it was taken.

`舐め取ってくれる` is emitted as one span glossed **`取ってくる`**, "to fetch".
The word is `取って` + `くれる`, the benefactive — somebody did it *for* the
speaker. The gloss does not merely coarsen the span, it inverts the meaning.

30 wrong hints-on (5.4 per 10k) and 22 hints-off (3.9 per 10k) against ~1,768
literal `てくれ` occurrences, so ~1.7% of the family. Hints-on, by surface:
`取ってくれる`→`取ってくる` 6, `戻ってくれる`→`戻ってくる` 4, `出てくれた`→`出てくる`
3, then `持ってくれて`, `行ってくれる`, `戻ってくれた`, `持ってくれる`,
`持ってくれている`, `出てくれる`, `持ってくれたら`, `行ってくれなかった`,
`帰ってくれる`.

**It is not `640074c`'s.** The pre-fix binary's hints-on list is byte-identical
to the current one — same twelve surfaces, same counts — which is the
predecessor §9's "byte-identical" claim confirmed on text it never saw. The
deinflector reaches `くる` from `くれる` as a passive/potential, and that path
predates the `ATTACHES_TO` rows. **The half-sentence after that is wrong**: the
passive/potential is the *label*, not the cause. The cause is which class the
entry was conjugating in at all, and §8 has it.

Two shapes in the same window are **correct** and a fix must not touch them:
the JMdict headword `出してくれる` (9 occurrences, "to pay for; to foot the
bill"), and the 仮定形 `〜てくれば` — `連れてくれば`, `戻ってくれば`,
`行ってくれば` (4).

### 4b. `達し|た`

2,022 kanji+`した` windows; **147 cut at the `し|た` junction hints-on (7.3%)
and 211 hints-off (10.4%)**. One lexeme is 66 of them in both states: `達した`,
emitted as `達し` + `た`.

Both words are in JMdict — `達する` has an entry, and so does the noun `達し`,
"official notice". The engine prefers the noun plus the past auxiliary over
`達する`'s past form, so this is the cost model rather than coverage: the same
family as `ミスった` losing to `ミス` + `った`. The rest of the class is a long
tail (`出し|た` 14, `隠し|た` 9) and most kanji+`した` is already whole — `出した`
283, `射精した` 104, `勃起した` 77.

### 4c. `イ|って`

Katakana stem + `って`/`った`: 343 windows, **319 cut hints-on (93.0%)**, 305
hints-off. Most of that is correct — `ビクビク|って`, `キス|って`, `グリグリ|って`
are the quotative and belong apart. The defect is one stem: **`イ` cut 109 times
hints-on and 95 hints-off**, against 14 and 28 kept whole.

`イく` is not a JMdict headword (0 `keb` in the XML), so `イ` resolves through
the kana class to `い` and matches the one-character entries. But the coverage
is reachable — `イっちゃう` parses as one five-character span while
`イってしまう` splits `イ`|`って`|`しまう` — so this too is the ranking rather
than the lexicon. Register-specific to eroge, and the largest single-surface
cut in this set after the stray kana.

## 5. Everything the predecessor named, re-measured

| class | four corpora | six corpora |
| --- | --- | --- |
| stray small kana, sentences carrying ≥1 | 16.5% on | **16.0% on**, 15.6% off |
| stray small kana, segments per 10k | 3,547.7 on | **3,258.7 on**, 3,073.2 off |
| false gloss on a stray | `ぇ`→`ェ` 1,869, `ー`→`ー` 785 | **`ぇ`→`ェ` 925, `ー`→`ー` 293** |
| particle → godan, wrong whole merges | 474 on (62.5/10k) | **296 on (52.8/10k)** |
| same, hints-off | 1,026 (135/10k) | **679 (121.1/10k)** |
| `のも` / `のかな` | hints-off only | **0 / 1 on, 238 / 179 off** |
| atomisation `気持ちよく` / `お尻` / `気がする` | whole on, 0 off | **385 / 361 / 79 on, 0 / 0 / 0 off** |
| `やだ` emitted whole | 0 of 84 | **0 of 191**, both states |

Every class reproduces, including the hint-state split inside the particle
family: `からね` 123, `からな` 97, `ないだろ` 67 and `もんな` 9 survive the hint
dictionary, while `のも` and `のかな` are hints-off only. `やだ` is now 191
occurrences and has never once been emitted whole.

**One number moved, and it matters for the fix.** The stray-kana attachment
split reads **88% attached to a preceding kana run and 12% following punctuation
or opening a line**, against the predecessor's 74/26. The 12% are the ones with
no correct merge, so the cheapest candidate — never segment inside a run of kana
that carries no word — reaches more of this class than the four-corpus figure
suggested, and still leaves the strays alone by construction.

Visible hints-off only, and not a separate class: **`千歳` 1,211 regions in
pxc2**, split `千`|`歳` with the entry `千歳 (ちとせ)` sitting right there. Proper
names are the hints-off atomisation class, measured on a corpus whose heroine
carries the name.

## 6. Method notes worth keeping

- **Never glob a dump directory while a dump is still being written.** An
  interim pre/post comparison in this session read 16 wrong against 7 and
  reversed completely on the finished files: the `*.on.jsonl` glob had picked up
  a half-written fourth corpus on one side only. The finished answer is
  byte-identical lists. Compare complete files, or compare corpus by corpus.
- **A ratio moves when the engine moves.** §2's control is cheap — one corpus,
  one state, about three minutes including the v1 dump — and without it this
  session would have reported a register difference that does not exist.
- **Build the old binary rather than trusting the old numbers.** §3's before
  column is the pre-fix engine on *this* text, not the predecessor's table
  carried over. A worktree plus `cargo build --release -p jparser --features
  mecab --example span_dump` is a few minutes and it is what makes the
  replication held out.
- **A window count needs adjudicating before it is a defect count.** 343
  katakana windows are 93% cut and that number is nearly meaningless — one stem
  is the defect and the rest is the quotative. Same for `てくれ`: 43 merged spans
  hints-on are 30 defects, 9 a JMdict headword and 4 a 仮定形.
- On this shell, Python needs `PYTHONIOENCODING=utf-8` or every Japanese column
  prints mojibake.

## 7. Not done

- ~~**§4a's `〜てくれる`**~~ — **fixed the same day**, and it was a deeper
  defect than §4a could see. See §8.
- **The other two new rows.** `達した` is the cheaper — one lexeme,
  deterministic, 66 occurrences — though gold contains no `達` at all, so
  neither of them has a cell that can move.
- **The pre-fix binary was run hints-on only**, so §4a's "not `640074c`'s" claim
  is a hints-on claim. The hints-off column was never compared against it.
- **The stray-kana class still has no gate**, which the predecessor's §8 already
  said and this session did nothing about. It remains the largest thing measured
  and no cell in `score` moves when it moves.
- **`vn-text` is exhausted.** Another corpus means another source, and eleven
  writers have not changed a sign yet — the next unknown is a different
  register, not a twelfth eroge script.
- Everything the predecessor's §8 left open that this session only re-measured:
  the particle → godan family, the hints-off atomisation class, `やだ`, the
  heavier auxiliary metric, `BOUND_FORMS` for an auxiliary's own 連用タ接続, and
  the six darwin screenshot baselines.

## 8. Postscript: §4a fixed, and what it was covering

`4bc74b4`, the same day. §4a named a wrong gloss; the cause under it was a
class assignment, and closing it exposed a missing one.

**The class.** `カ変・来ル` is 来る written with its kanji — stem 来, and every
suffix in the class is what follows it (empty for 未然形 and 連用形, る for
基本形). It removes る, so **every kana `vk` headword satisfied it by accident**
and deinflected over a stem that is not one: 取ってく + れる, 連れてく + させる.
`連れてくさせる` was matched and glossed, with the reading `こさせる`; the word
is 連れてこさせる. Nothing in text produces most of these, which is why only the
one that collides with a real word — `取ってくれる` against the benefactive
くれる — ever reached a corpus count.

The headword settles it, and the dictionary partitions with nothing left over:
over JMdict's 38 `vk` entries, **65 headwords end 来る or 來る and 82 end くる
or クる**. One early return in `lex::allows_chain`, before the entry's own POS
is consulted.

**What the hole was covering.** くれる carries POS **`v1-s`**, which was in
neither `JMDICT_POS_CLASS` nor `POS_WITHOUT_A_CLASS` — an omission rather than
a documented gap. So the benefactive matched uninflected only, and
`くれた`/`くれて`/`くれない` were reached *through the hole*, with the gloss くる.
Remove the hole alone and they fragment: `言ってくれた` becomes
`言って`|`くれ`|`た`, 1,159 sentences changed and **+1,264 spans net**. With
`v1-s` mapped to IPADIC's own `一段・クレル`, the same 1,159 sentences change
**span for span** — 251 out, 255 in — and only the gloss moves, くる to くれる.
Four JMdict entries carry `v1-s`, every one ending in れる.

**The reading had the same premise.** `reading_for`'s 来ル branch rebuilt the
reading from the kana class's suffix *alone*, so a compound lost everything
before the 来: `連れて来い` read `こい`, not `つれてこい`, and the reading is
what the furigana renders. It now prefixes the dictionary reading minus its
trailing くる; bare 来る strips to empty and is unchanged by construction.

| class | before | after |
| --- | --- | --- |
| `〜てくれ` wrong glosses, hints-on | 30 | **1** |
| correct merges in the same shape, on | 13 | **36** |
| `〜てくれ` wrong glosses, hints-off | 22 | **10** |
| `連れて来い` | こい | **つれてこい** |
| `連れてくさせる` | one matched span | **not matched** |

**Every other measured class is unchanged** — stray kana 18,270 segments and
16.0% of sentences, the four auxiliary cut rates (15 / 6 / 4 / 12 hints-on),
the particle → godan family (97 / 67 / 123 / 0 / 1), `やだ` 0 of 191, the
hints-off atomisation class. The change is confined to the くる family.

Gold is byte-identical in both states — 1694 / 1948 / 0.9527 / 152 hints-on and
1588 / 1785 / 0.9504 / 118 off. That is a damage check and not a win: **gold
carries no 来る, 来た, て来 or くれる at all**, which is why this class needed
a unit gate rather than a score cell. Five tests in `engine_v2.rs`, over a
private fixture carrying 来る/來る, 出てくる/出て来る, 取ってくる and 呉れる:
the two bogus stems must not be candidates, the two カ変・クル forms in the
same text (`取ってくれ`, `取ってく`) must survive, `出て来た` must read
でてきた, `来た` must still read きた, and `取ってくれた` must gloss くれる.
Suite 666 passed, 0 failed, 1 ignored (a pre-existing doc-test); fmt and
clippy clean; the table fingerprint moved deliberately, 140,947 → 145,482, for
the 22nd mapped class.

**Two things it does not fix, both stated rather than hidden.**

- **The 仮定形 residue.** What is left of `〜てくれ` is bare `やってくれ` (7) and
  `持ってくれ` (3) hints-off, matched as 持ってくる's own 仮定形 — a real form
  of a real entry (`持ってくれば`), so no headword rule separates it. The
  auxiliary chain already blocks this through `AUX_FORM_BLOCK`; the *entry*
  cannot be blocked the same way without losing 〜てくれば.
- **One wash, 115 occurrences, hints-off katakana.** `キツク` was `キツ` +
  `ク` (キツい plus a bogus クる) and is now `キ` + `ツク` (a bogus キる plus
  つく). Both are wrong. The right answer is one span, `きつく`, and it loses
  on the katakana penalty in both builds — `きつく締め付け` parses correctly and
  `キツク締め付け` does not, before this change and after it.

Against that, hints-off gains outside the くれる family that §4a never
predicted, because the hole was eating adjective 連用形 wherever one met an
auxiliary: `気持ちよ`|`くないです` glossed "to come" is now
`気持ちよく`|`ないです`, `全くない` reads `くない` instead of くる, and
`気持ちよ`|`くさせたい` is now `気持ちよく`|`させたい`. 1,868 sentences change
hints-off in total.

## 9. Postscript: the katakana run charge, measured and refused

`76edf85` and `4007a45`, the same day. The katakana-run-integrity phase
(`docs/superpowers/specs/2026-09-21-katakana-run-integrity-design.md`) added
one field and one cost term and changed nothing else:
`lex::Candidate.inside_katakana_run` is pure geometry, set at construction
and read by nothing until the cost term exists; `cost::KATAKANA_FRAGMENT`
(`crates/jparser/src/engine/cost.rs:603`) is a flat charge paid by a
candidate shorter than `KATAKANA_MIN_PIECE` (3) that sits inside a longer
run of katakana. No new constraint, no index format change, nothing outside
`crates/jparser/src/engine/`.

Task 3 swept six candidate magnitudes — 40, 50, 60, 80, 100, 130 — against
the design's zero-gold-damage gate (§5) and cleared none of them; design §8
carries the full table and the per-value reasoning and is not duplicated
here. The constant was left at 60, "the least damaged of the six," recorded
in `cost.rs`'s own doc comment as an unvalidated placeholder rather than a
shipped magnitude. This section re-runs every gate against that decision on
the tip that carries it (`585f8a9`), across all six corpora rather than the
two Task 3 used for its corpus rows.

### 9.1 Gates

`cargo fmt --all -- --check` is clean. `cargo clippy --workspace
--all-targets --features mecab` reports zero warnings and zero errors.
`cargo test --workspace --features mecab` is 675 passed, 0 failed, 1
ignored (the pre-existing `jmdict_source` doc-test). `git diff --stat`
against `crates/jparser/tests/fixtures/parse_sentences.txt` is empty — its
only katakana runs of three or more, コーヒー and パソコン, are unaffected in
both hint states.

### 9.2 The gold pair

Both `score` commands, at this tip, against §5's baseline (1694 / 1948 /
0.9527 (1188/1247) / 152 hints-on; 1588 / 1785 / 0.9504 (1169/1230) / 118
hints-off):

| | on strict | on tolerant | on entry top-1 | on compounds | off strict | off tolerant | off entry top-1 | off compounds |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | 1694 | 1948 | 0.9527 (1188/1247) | 152 | 1588 | 1785 | 0.9504 (1169/1230) | 118 |
| this tip (60) | 1692 ↓ | 1948 | 0.9526 ↓ (1186/1245) | 153 ↑ | 1590 ↑ | 1791 ↑ | 0.9493 ↓ (1160/1222) | 120 ↑ |

Three cells fall: on strict (1694 → 1692), on entry top-1 (0.9527 → 0.9526,
1188/1247 → 1186/1245), and off entry top-1 (0.9504 → 0.9493, 1169/1230 →
1160/1222) — the same three cells, and the same values, design §8.1 already
recorded for value 60. On tolerant holds exactly. Four cells rise: off
strict, off tolerant, on compounds and off compounds. §5's gate is "no cell
above may fall"; three do, so the gate fails here exactly as it failed for
every value Task 3 swept — this reproduces design §8.4's refusal rather than
reopening it.

### 9.3 Six corpora, before and after

| hint state | build | runs ≥3 whole | split-with-short-piece | split-into-word-sized |
| --- | --- | --- | --- | --- |
| on | baseline (charge-free, `76edf85`) | 23328 | 2017 | 303 |
| on | this tip (charge 60) | 25111 | 210 | 327 |
| off | baseline (charge-free, `76edf85`) | 17598 | 7573 | 477 |
| off | this tip (charge 60) | 22434 | 2646 | 568 |

Every row sums to 25,648 — the total count of katakana runs of three or more
characters across the six corpora (§1.1), unchanged by the charge in either
state, since the charge only moves runs between these three buckets.

Split-with-short-piece falls 9.6x hints-on (2017 → 210) and 2.9x hints-off
(7573 → 2646). The hints-on side clears §5's "at least an order of
magnitude" bar; hints-off — the state the fix is actually aimed at, and the
"draining population" per the four-corpora handoff §11 — does not, and its
ratio here (2.9x) matches design §8.2's own finding on its two-corpus subset
(4148 → 1651, 2.5x) closely enough that six corpora do not change the
conclusion.

Split-into-word-sized also grew: 303 → 327 hints-on, 477 → 568 hints-off.
This is the same reclassification design §8.2 found on its two-corpus
subset: blocking a short-piece split does not always fall back to leaving a
run whole; sometimes a different, all-≥3 tiling exists and the run lands in
a row the charge was never meant to touch, since nothing under three
characters is a candidate there. It is a second-order effect, not a gate
failure — §5 gates gold, not this row.

Fresh gold dumps at this tip (rebuilt for this task, not reused from Task 3)
give: katakana gold spans ≥3 missed, hints-on 8 of 54 (baseline 7 — the new
`ラスト` casualty, §9.4), hints-off 19 of 54 (baseline 26 — 7 recovered).
These match design §8.1/§8.2's already-recorded value-60 figures exactly.
(58 in design §1.1 and 54 here are different quantities, not a
discrepancy: §1.1 counts katakana *runs* of three or more characters found by
scanning the corpus text, while this 54 counts gold-annotated *spans* that
are all-katakana and three or more characters. The two differ wherever gold
cuts a run — a cut can leave a piece shorter than three characters, which no
longer counts as its own qualifying span. Re-running `gold_katakana.py`
should reproduce 54 against gold's annotations, not 58.)

### 9.4 The `ラストラン` cause

The one hints-on casualty is gold sentence `b:wn21710-1`, traced in full in
design §8.3. Gold annotates `ラスト` (start 32, len 3) and `ラン` (start 35,
len 2) as two spans; at this tip the engine returns `ラストラン` (start 32,
len 5) as one matched span in both hint states, because `ラストラン` is
itself a JMdict entry (ent_seq 2843587,
`crates/jparser/tests/fixtures/gold_jmdict_subset.xml:136897`) and any
charge above zero makes that cheap matched whole-run candidate beat the
split. Task 2's fixture test,
`a_five_character_katakana_compound_keeps_its_cut`, cannot see this:
`KATAKANA_FIXTURE` defines `ラスト` and `ラン` as entries but never defines
`ラストラン`, so inside the fixture the whole run really is an unmatched
150-point fallback and the test passes for a reason that does not hold
against the installed dictionary.

### 9.5 Why gold cannot show the hints-on win

There is no hints-on win for gold to show. Gold's own hints-on katakana
misses (7 of them at baseline, present before this charge existed) are not
the short-piece failure this charge targets — they are the opposite shape,
cases like `タイムリー`/`ヒット` where gold's annotation wants two words and
the engine already prefers the single merged JMdict entry, charge or no
charge. Since none of gold's existing hints-on katakana problems are runs
shattered into short pieces, no value of `KATAKANA_FRAGMENT` can turn one of
those misses into a hit; and on the one gold sentence where the charge does
cross a genuine word/dictionary-entry boundary (`ラストラン`, §9.4), the only
direction available is to cost a cell, never gain one. Hints-off gold moves
in both directions instead (§9.2: strict and tolerant rise, entry top-1
falls), but that is a mixed, noisier signal and not itself evidence for or
against the run-charge mechanism.

### 9.6 What this does not fix

Three things, all named out of scope in design §1: `イ|って` (109
occurrences hints-on) — the katakana run there is one character, so there is
no run to charge; the `INEXACT` fold penalty, which is still why
`キツク締め付け` parses worse than `きつく締め付け`; and proper-noun
recognition — `ユリナス` still comes out unglossed, the honest answer for a
word the dictionary does not have, not a gloss.

### 9.7 Latency

`jparser-cli bench` was run against `gen-3` on
`crates/jparser/tests/fixtures/gold.jsonl`, hints-on, four interleaved
rounds — baseline, candidate, baseline, candidate, ... — alternating
`$SCR/baseline/jparser-cli.exe` (built from `76edf85`, before the cost term
existed) with a release build of this tip, to cancel the roughly 10% drift a
single machine shows across a session. At the short paste (1,005 chars) the
candidate/baseline ratio across the four rounds was 0.944, 0.963, 1.000,
1.098 (median 0.981); at the long paste (10,040 chars) it was 0.991, 1.002,
1.011, 1.023 (median 1.007). Both medians sit inside session noise with no
trend against length, so the charge's once-per-sentence run scan is not
visible, as design §5 expected.

### 9.8 State of the branch

`KATAKANA_FRAGMENT` is live at 60 on this branch and costs the three gold
cells named in §9.2 (on strict, on entry top-1, off entry top-1) for a
six-corpus hints-off improvement that gold, by construction (§9.5), cannot
register as a win. The refusal recorded in design §8.4 stands unchanged.
Whether to ship this trade anyway or revert `KATAKANA_FRAGMENT` to 0 is an
open decision for the human partner, not settled by this task.
