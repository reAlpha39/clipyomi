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

Measurement only. No code changed, no gate was re-run, and every artifact —
the v1 index, the dumps, the pre-fix worktree — is gone with the session
scratchpad.

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
predates the `ATTACHES_TO` rows.

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

- **The three new rows.** `達した` is the cheapest — one lexeme, deterministic,
  66 occurrences — and the only one of the three with a plausible gold-corpus
  cell, since `達する` is ordinary prose rather than eroge register.
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
