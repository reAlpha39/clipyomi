# Three corpora, one defect class — handoff

**Date:** 2026-09-18
**Branch:** `feat/corpus-disagree`, tip `63c7dc6`, thirteen commits off
`feat/sokuon-tail` (`5ffaba7`)
**Predecessor:** [`2026-09-18-corpus-disagreement-handoff.md`](2026-09-18-corpus-disagreement-handoff.md)

The previous session built the disagreement probe and ran it on one corpus.
Every class it found was therefore a class *one writer* produced, and nothing
said which of them were Japanese and which were that writer's habits. This
session ran it on two more, 67,433 further sentences, and adjudicated all
three side by side.

## 1. What shipped

| commit | what it is |
| --- | --- |
| `7bc7707` | `corpus_disagree.rs` prints the merge direction the ranking structurally cannot show — §3 |
| `7f4b146` | `table.rs`'s `ていく`/`でいく` rows, the directional auxiliary — §7 |
| `def0730` | `LEN_GAIN`'s bounds gate re-measured and refused again, with a corpus-scale number — §8 |
| `63c7dc6` | the second attachment site `ていく` needed, and a `BOUND_FORMS` extension refused beside it — §7 |

Gates at the tip: 658 tests passed, 0 failed, 1 ignored; `cargo fmt` and
`cargo clippy --workspace --all-targets --features mecab` clean; `npx tsc
--noEmit` clean; `npx vitest run` 375 of 375; the 26-row spurious-merge gate
26 of 26; `parse_sentences.txt`'s snapshot byte-identical.

## 2. The corpora, and how to rebuild the whole run

Both new corpora are in `vn_data/`, which is git-ignored. Table
`artificialtrans`, column **`source`** — never `origin`, whose every character
is doubled. `machineTrans` is a translation and is never a segmentation
oracle. Dump distinct non-empty rows with newlines stripped; that recipe
reproduces the previous session's corpus at exactly **8,334**, which is how it
was validated.

| corpus | file | distinct sentences |
| --- | --- | --- |
| renai | `D:\Code\oss\恋愛、はじめまして_…sqlite` (outside the repo) | 8,334 |
| kf2 | `vn_data/KF2ifFHD_…sqlite` | 31,734 |
| riruka | `vn_data/riruka_…sqlite` | 35,699 |

Copyrighted game script: local probe input only, nothing derived from it goes
in the repository — the same judgment the frequency-layer refusal made.

The v1 side is produced out of tree, because criterion 1 is that
`crates/jparser` contains nothing derived from ta-old. The detached worktree
at `D:\Code\oss\clipyomi-v1` (`1d5b0cb^`, GPL-2.0-only) survived the previous
session, with its untracked `examples/v1_dump.rs`. Check `git worktree list`
before re-adding it. Its index format is 3 and needs `build-index --engine v1`;
the current index format is 6. Neither engine has an XML in the tree —
decompress `%APPDATA%\dev.jparser.clipyomi\source\JMdict_e.gz` to a scratch
directory.

**v1 is triage, not an oracle.** Its own scores are criterion 2's floors
(strict 0.6810 / 0.6963, entry top-1 0.9159 / 0.9155) and every one is below
v2's, so a disagreement is a place worth adjudicating, never evidence that v1
is right.

## 3. The merge direction, which the ranking cannot show

The probe's module doc already said that three of its four signals detect
over-*splitting* only: `sht` needs us to cut a covered region, `rdg` compares
the parts' readings so a merge makes them trivially equal, and `prt` needs a
particle to survive as its own segment. Only `alt` is symmetric, and it scores
1. So a change that merges more will always look better in the ranking, and
nothing printed contradicted it.

`7bc7707` prints the counter-reading over every region, not only the printed
head. It comes from the covers already stored per region, so no new state.

| corpus | state | we merge, v1 splits | we split, v1 merges | same count, cuts move |
| --- | --- | --- | --- | --- |
| renai | on | 1,440 / 2,769 | 3,531 / 9,956 | 1,073 / 1,731 |
| renai | off | 1,141 / 2,144 | 5,406 / 12,036 | 1,421 / 2,092 |
| kf2 | on | 2,417 / 11,911 | 6,106 / 22,011 | 1,670 / 4,065 |
| kf2 | off | 1,793 / 4,417 | 9,992 / 34,475 | 2,081 / 4,219 |
| riruka | on | 3,539 / 25,438 | 7,265 / 39,007 | 2,787 / 5,650 |
| riruka | off | 3,072 / 8,811 | 11,450 / 51,122 | 3,575 / 6,805 |

Regions first, sentence-occurrences second. Same sign in all six: we over-split
2.5–3.5× more regions than we over-merge, so the ranking is reading the
dominant direction. **But the blind side is not small** — 1,440 to 3,539
regions per corpus — and an audit of the `--top 2000` files found that every
one of the 1,461 over-merge rows in them carries `alt` or `alt=v1` and **not
one carries `sht` or `rdg`**, so they are ordered by `n` alone and sit below
every two-signal row by construction. Between 1,069 and 2,966 over-merge
regions per corpus never entered the top 2,000 at all.

## 4. What three corpora agree on

Heads were joined by surface and normalised per 10,000 sentences. **300
surfaces appear in all three hints-on heads, 232 hints-off.**

| class | rate per 10k over 75,767 sentences | states | verdict |
| --- | --- | --- | --- |
| a covered unit split anyway — kanji compound *and* kana word | ~1,640 | **off only** | defect |
| clause-final particle string re-lemmatised as an unrelated godan verb | 169 | both | defect |
| `ていく`/`でいく` a separate segment | 157 | both | defect — **fixed, §7** |
| 〜てくる / 〜ておく / 〜てみる a separate segment | unmeasured | both | defect — **open, §7** |
| quotative `と言った` matched as `といった` "such … as" | 56 | both | defect |
| `ておく` contractions shatter | 22 | both | defect |
| `X`+particle tiling where v1 merges | ~1,100 / 1,967 / ~5,100 occ | both | **correct** — criterion 3 working |
| v1's merge would be the false match | 306 / 129 / 1,342 occ | both | **correct** |

The second row is the `からね` class and it is larger and wider than the
previous handoff recorded. Its shape is uniform: **a clause-final particle
string is consumed by the deinflector and re-lemmatised as an unrelated godan
verb.** Members measured, with the word actually displayed:

`からな` 625 → `かる` "to spur on", `からね` 162 → `かる`, `からねえ` 36 → `かる`,
`ないだろ` 297 → `なく` "to weep" (and the `う` after it becomes the Rabbit of
the zodiac), `のも` 210 → `のむ` "to drink", `もんな` 32 → `もむ` "to rub",
`のかな` 76 → `のく` "to step aside", `とかな` 29 → `とく` "to untie", `たまん` 26
→ `たまる`, `たいなら` 13 → `たく` "to cook rice", `いんや` 38 → `いる`, `ぐぐ` 14
→ `ぐぐる` "to google". Roughly 1,280 raw occurrences. `grep -c '刈らな\|駆らな'`
returns 0 on all three corpora, so the class is 100% false-match.

## 5. Contamination is a hints-off artefact, and it is large

| corpus | hints-off head | share | hints-on head |
| --- | --- | --- | --- |
| riruka | `七星` 3,156 | **33.5%** | **0%** |
| kf2 | `ソブリ` 648, `モルグレー` 314, `ヌが` 225, `コメン`+`ダーレ` 296, `ティントン` 128, `オは` 126, `スが` 74 | **30.4%** (34.7% with onomatopoeia) | **0%** |
| renai | `いおくん` 109, `夕陽` 33 | — | small |

`七星` is a character name and the corpus contains her self-introduction;
1,899 of the 3,156 are the bare speaker tag `七星「`. JMdict's only `七星` is
`[primary]` "the Big Dipper", a word this corpus never uses. **The hint
dictionary already merges every one of these**, which is why the hints-on
heads are clean — so contamination is a property of the hints-off probe, not
of the corpora. Any gate built off a hints-off list must subtract them.

`おまえさん` (822, riruka) was checked and is **not** contamination: an
ordinary second-person pronoun with a JMdict headword, used to many
addressees.

## 6. Why the kana heads print `-`, and what that hides

Every kana row at the head of every hints-on dump prints `-` in the cover
column — 68 distinct rows, 7,514 occurrences. The mechanism is exact and worth
carrying: `のか`, `には`, `のに`, `みたいな`, `それは` all carry `[primary]`,
while `ように`, `ために`, `くせに`, `ところで`, `たびに` carry `[reading]` only.
The discriminator is **kanji-headedness** — `みたいな` has no kanji form so the
kana *is* primary, while `ように` is a kana spelling of 様に and so is
`PRONOUNCE`-only. The cover column and the `is_grammatical_stack` exclusion
both require a `PRIMARY`, exact, uninflected cover, so both are silent exactly
where criterion-3 refusals are most frequent. `ために` and `くせに` even carry
the `particle` flag and are still invisible to both.

**Adjudicating those 68 rows split the review.** Read against the project's own
rule — `transparent` is "function-word stacking whose parts are each what the
reader needs to see" — `よう` + `に` composes and the `に` is doing work, so
the head is mostly criterion 3 working; the two corpus adjudicators both
reached that reading independently. The probe auditor read 16 of the rows as
`lexical` misses instead. **The rows are the same either way and what is
underneath them is not in dispute**: the split displays a **false gloss on a
part** — `な` as the prohibitive "don't", `くせ` as `くる (Causative)`, `たび`
as 足袋 "split-toe socks", `とき` as `とく` "to untie", `ついで` as 次いで,
`おれ` as `おる (Conditional)`, `しれない` as `する`, `みた` as `たい`.

That is **entry ranking, criterion 2, not segmentation**, and by volume it is
larger than anything in §4. It is a phase of its own and this session did not
open it.

## 7. `ていく` / `でいく`

`歩いて` | `いく`, `引きずって` | `いった`, in **both** hint states. Every other
auxiliary this engine knows already merged — 〜た, 〜ない, 〜ます, 〜たい,
〜ちゃう, and 〜てる / 〜ている as of the two commits before this session — so
the split was a missing row rather than a policy. 1,189 lines of 75,767 carry
one, at 9.6, 15.5 and 17.3 per **thousand** in the three corpora
**separately**, which is what makes it a rate rather than one writer's habit.

**It is the most frequent 〜て auxiliary still split, not the only one.**
〜てくる, 〜ておく and 〜てみる have no rows either and still read as two
segments — `歩いて` | `くる`, `買って` | `おく`, `歩いて` | `みる` — and
`持ってくる` merges only because JMdict carries it as a headword. Anyone adding
〜てくる needs a *second* label word: it is the same directional concept in the
opposite direction, and `label::fmt`'s "each word appears at most once" rule
would silently drop one of them from a chain carrying both.

`五段・カ行促音便` is 行く's own class and gives the paradigm from one row per
voicing: ていく, ていっ(た), ていか(ない), ていき(ます), ていけ(ば), ていこ(う).
`でいく` mirrors `でいる` and takes 連用タ接続 only, following an onbin stem.

**No `CHAIN_BLOCK` or `AUX_FORM_BLOCK` rows accompany them**, and that is a
statement about the class rather than a measurement. Both tables exist because
一段's 未然形 and 連用形 have *empty* suffixes, so `てる` carries a bare `て` on
which a one-kana auxiliary stacks and collides with a sentence-final particle
(`してね`, `待ってな`, `待ってよ`). 五段・カ行促音便 has no empty form — its 未然形
is `ていか` — so what an auxiliary can reach is `ていかね` and `ていかな`, which
are `持っていかねば` and `持っていかなきゃ`.

**"Directional" is the third label word taken from outside the fixture's
Wikipedia terminology list.** Unlike Completive and Progressive it could not
take the standard aspect term with a JMdict gloss agreeing, because JMdict has
no `ていく` headword at all; it takes the learner-standard name for the
〜ていく / 〜てくる pair instead. The fixture records why, as it does for the
other two.

**It took two commits, and the second is the lesson.** `7f4b146` gave `ていく`
a single attachment site, 連用タ接続, which reaches 五段 onbin stems and leaves
**every 一段, サ変・スル and 特殊・デス verb behind** — `歩いていった` merged while
`食べていく`, `落ちていった`, `溶けていく` and `見ていく` all still split. The rule
is in `ATTACHES_TO`'s own doc and predates this work: IPADIC names 連用タ接続
only where a class distinguishes it from plain 連用形, so **either row alone
leaves half the classes unreachable**. `でいく` correctly keeps one row, because
で follows an onbin stem and nothing else. If you add another 〜て auxiliary,
give it both sites unless it is a で-form.

Cells at `63c7dc6`, hints on: strict 1714 → **1698**, tolerant **1948**
unmoved, entry top-1 0.9534 → 0.9528, compounds 152 with all three class lines
identical, coarsening 234-in-108 → 250-in-116. Hints off: strict 1610 →
**1593**, tolerant 1791 → 1786, entry top-1 0.9513 → 0.9506, compounds 118
with all three class lines identical, coarsening 181-in-88 → 193-in-94.

**The strict fall is decomposed, not assumed.** Hints on, 8 gold spans lost
and 0 gained, and **all 8 have both endpoints on gold boundaries** — four
intended merges and nothing else, zero damage. Hints off, 11 lost and 0
gained: 8 the same coarsening and **3 real damage, all in one sentence** —
`出ていったようだ` reads `出ていっ` | `たよう` | `だ`, where `たよう` is 態様
"condition; manner" and the auxiliary's own onbin stem has been taken for a
word.

**That one has a named cause and a priced fix, and the fix is refused.**
`BOUND_FORMS` exists so that a chain *ending* at 連用タ接続 is not a word —
`優しいっ` must not read `優し` + `いっ` — but it is consulted only for a
headword's own inflection, never for an auxiliary's. Extending it to
auxiliaries was built and works: `出ていったようだ` comes back whole, hints-off
strict 1593 → 1594, tolerant 1786 → **1789**, compounds 118 → **119** with
`lexical` 56 → 57, hints on identical to the digit. It is refused because it
deletes **nine committed rows** from `conjugation_labels.txt` — `食べなかっ`,
`食べたかっ`, `読んだっ`, `買っちゃっ`, `読んじゃっ`, `買っちまっ`, `読んじまっ`
and this change's own two — and that fixture declares itself **authoritative
over the code**: "a row this code cannot pass is a finding about the table, not
a row to edit". Seven of the nine predate this work. It is therefore a fixture
revision wanting its own adjudication, and `BOUND_FORMS`'s own comment already
set that precedent — each bound form "deletes candidates of its own and wants
its own score run".

Chain count 73,731 → **90,359**. Latency unmoved on an interleaved A/B, three
rounds each way: at 10,040 characters, hints on 567 / 543 / 544 ms before
against 542 / 545 / 537 after, hints off 618 / 618 / 619 against 620 / 599 /
606.

## 8. `LEN_GAIN` hints-off: measured twice, refused twice

**This is the largest class in §4 and the only shape that can reach it**, so
it was priced properly rather than left on the old note.

The arithmetic first, because it rules out every cheaper fix. Hints off, a
non-COMMON entry of length `n` tiling into `k` COMMON content pieces loses by
`2n + 3 − 3k`; at `k = 2` that is `2n − 3`, which **grows with `n`**. No value
of `MATCH` or `CONTENT_MATCH` closes a gap that grows. Only the square can,
and `LEN_GAIN` is gated on `bounds`, which means hints-on only. Since
`mecab_hints` is `#[serde(default)]` on a bool, **the broken state is the one a
fresh install runs in.**

The refusal recorded in the comment predated `CONTENT_MATCH`,
`KATAKANA_MIN_PIECE`, the `uk` clause and the 〜てる / 〜ている / 〜ていく rows,
so it described an engine that no longer runs. Re-measured on today's:

| hints off | strict | tolerant | compounds | lexical | false-match | transparent | criterion 3 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `7f4b146` (the variants' baseline) | 1604 | 1789 | 118 | 56 / 112 | 23 / 32 | 39 / 42 | 26 / 26 |
| ungated | 1525 | **1811** | **158** | **94** | 25 | 39 | **26 / 26** |
| gain only on uninflected spans | 1576 | **1816** | 151 | 87 | 25 | 39 | 26 / 26 |

Hints on is identical to the digit in both variants, by construction. **No
split class regresses in either**, which is the check the old note's headline
hid, and 99 of the ungated variant's 132 lost gold spans are coarsening
against 33 of real damage. On the gold corpus, both variants say ship it.

**The corpus says it is a wash, and a wash is not worth the risk.** Span dumps
over 18,000 corpus sentences in both builds, classifying each new span by
whether **both its endpoints were boundaries in the other build's own parse**.
Read the proxy for what it is: it measures agreement with the *previous
engine*, not with truth, so a genuine boundary correction files under "cuts
across one" too. It says the conservative and the boundary-moving changes are
equally numerous; it does not by itself say harm equals good.

| variant | sentences changed | new spans merging adjacent base spans | new spans cutting across one |
| --- | --- | --- | --- |
| ungated | 9,370 / 18,000 | 9,849 | **9,737** |
| uninflected only | 8,155 / 18,000 | 7,248 | **9,537** |

The gold corpus's 3:1 ratio is an artefact of 150 curated sentences. And what
the invented spans destroy is morphology, which is worse than an unmerged
compound: `話しかける` → `話しかけ` + `る`, `やってくる` → `やってく` + `る`,
`言いつつ` → `言`("word") + `いつつ`, `見つめながら` → `見つめな` + `がら`,
`もしかして` → `も` + `し` + `かして`, `かもしれない` splits again, `おかえりなさいです`
→ `おかえり` | `な` | `さいで` | `す`.

Against real gains on the same sample — `あの頃`, `おかげ`, `居座る`, `人魚族`,
`滑り込ませて`, `言葉を失った`, `小金`, `州長官`, `金貸し`, `しゃぶりついた`,
`おまえさん`, `ちょいと`, `待ち合わせ`, `後にする`, `目を逸らす`, `三つ指` — the
two sides are of the same order — which is what reading the changed sentences,
rather than the counts, actually establishes. That is the refusal, and the
finding worth carrying past it is that **the gold corpus's 3:1
coarsening-to-damage ratio does not survive 18,000 real sentences.**

**The narrower variant does not help.** It reads better than ungating on every
gold cell and worse on the corpus, and the mechanism is specific: an inflected
span gets no gain while a shorter uninflected one does, so `来ていない` shatters
into `来` + `ていない` (庭内, "grounds"). **Two predicates tried, both refused;
no narrower one has been found** — which is not a proof that none exists, and
the doc comment says so in those words. The shape nothing expresses today is a
term that sees the **residue** of a cut rather than the length of a span, the
way `relax::MIN_PIECE` and `KATAKANA_MIN_PIECE` already do. `cost` is per
candidate and cannot see the tiling, so that is a `lattice` change.

Capped variants were swept and are dominated: `min(l, 3)` reads strict 1550 /
`lexical` 82, `min(l, 2)` reads 1571 / 71, both below the two above on the
compound line and no better on strict.

## 9. Method notes worth keeping

- **`--top 2000` truncates the entire zero-signal tail.** The summary reports
  2,875 to 5,642 zero-signal regions per corpus and not one of them is in the
  file. An agent asked to scan the tail will correctly report that the tail
  does not exist. Raise `--top` or emit the tail separately.
- **The probe is a *disagreement* probe, so the agreed-wrong set is outside its
  universe.** `言っとく` (言 "word" + っ + `とく` 解く), `置いといて`, `と言った`
  → `といった` are wrong identically in both engines and emit no region at all.
  "Not in the probe" means "v1 agrees with us", not "correct".
- **Covers are a sample of size one.** Seven of the 68 kana head rows print a
  cover that is a homophone of an unrelated noun: `さんが` 参賀 "congratulatory
  visit to the Imperial Palace", `さんと` 三都, `せいか` a pile of eight,
  `そうに` 僧尼, `なのか` 七日, `ぼくが` 墨画, `だか` ダカ. Reading the cover
  column as evidence of a missed unit mis-adjudicates all seven.
- **Homographs hide behind one row.** `ところで` is 71 conjunction against 335
  compositional 所+で; `ことに` is 0 against 713; `それに` is genuinely mixed at
  roughly 60/40; `それで` is three words in one row, of which `それでも` is 78.
  `ときに` is 157 of 158 compositional against a cover glossed "by the way".

## 10. Not done

- **The clause-final particle → godan verb class (§4 row 2, ~1,280
  occurrences).** The largest open segmentation defect, both hint states, all
  three corpora. §9.3 of the previous handoff priced the `(lemma, form)` route
  and refused it — 167 sentences changed against 34, two adjudicated fixture
  rows deleted — and shipped `CHAIN_BLOCK`'s pair key for a different subset.
  The wider family here was not priced.
- **The hints-off atomisation class (§4 row 1, ~1,640 per 10k).** Now known to
  be unreachable by any constant and refused for the square. What remains
  untried is a term that can see the *residue* of a cut rather than the length
  of a span — `relax::MIN_PIECE` and `KATAKANA_MIN_PIECE` already encode "a
  piece shorter than three is not evidence", and nothing expresses that for
  the merge side. `cost()` is per candidate and cannot see the tiling, so this
  is a `lattice` change, not a cost constant.
- **Criterion 2 on the split parts (§6).** Bigger by volume than anything in
  §4 and a different kind of defect. `な` as the prohibitive alone touches
  every adnominal in every corpus.
- **The quotative `と言った` class (428 occurrences with a `[primary,particle]`
  cover) and the `ておく` contractions (~170).** Both invisible to the probe by
  construction; both want the second, engine-internal pass §9 describes.
- **`BOUND_FORMS` for an auxiliary's own 連用タ接続**, §7. Built, measured,
  refused on the fixture. It is a strict improvement on every cell it moves;
  what it costs is nine adjudicated fixture rows, seven of which predate this
  work. Whoever takes it should decide those nine first and the code second.
- Everything the previous handoff's §11 left open that this session did not
  touch: `からね`/`からな` (now measured wider, above), `いた` outside the
  auxiliary, the six darwin screenshot baselines, gate 6's eighteen sentences
  still not in the repository, `忍` still reading `しのび`, the `[noun]達`
  class, and the romanizer's `icchi` where the frontend's `itchi` is correct.
