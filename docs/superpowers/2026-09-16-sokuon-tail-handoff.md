# The sentence-final っ — handoff

**Date:** 2026-09-16
**Branch:** `feat/sokuon-tail`, off `develop` tip `0eadc7f`
**Predecessor:** [`2026-09-16-post-merge-handoff.md`](2026-09-16-post-merge-handoff.md)

One screenshot pair, ClipYomi against Translation Aggregator, on
`「ええっ、いいの!? 忍くん優しいっ!」`. Three differences, two of them ours.

## 1. What shipped

| commit | what it fixed |
| --- | --- |
| `0a0c104` | a sokuon with nothing to double no longer romanizes as `tsu` — `ええっ` was `eetsu`, and `っあ`/`っん` doubled the vowel and the `n` |
| `3dc632d` | `連用タ接続` is not a word on its own — `優しいっ` was `優し` + `いっ`, a bare 五段 onbin stem beating `優しい` plus an emphatic `っ` |

The romaji half is the same class of defect as `d29360d`: the frontend
keeps its own kana table and it had drifted from `crates/jparser`'s
romanizer. Every new expectation in `furigana.test.ts` was read off
`jparser-cli romaji` first. The one place the two still disagree is `いっち`
— frontend `itchi`, engine `icchi` — and the frontend is the correct one
there (traditional Hepburn `tch`), so the engine is what wants fixing.

The engine half is `BOUND_FORMS`, a one-name list in `table.rs`. Every
`CLASSES` form was inserted as a `Terminal` trie hit, so `search` would end
a chain at `連用タ接続`, a form that only exists before `た/て/だ/で`. A
`Terminal` that is bound is now emitted only when `required` is `Some` —
which is exactly when an auxiliary asked for it, so `行った` is untouched.

`ちゃう/じゃう/ちまう/じまう` had no `ATTACHES_TO` rows, and they are the only
other consumers of `連用タ接続`, so the refusal on its own left `買っちゃった`
with no chain through `買っ` at all. Eight rows (`連用タ接続` and `連用形`,
the pair `た/て` each need) restore it and take `食べちゃう` to one span.
They are free on the corpus: every cell below is identical with and
without them.

## 2. The numbers

Gold index rebuilt from `gold_jmdict_subset.xml`, 150 sentences, 2,163
spans. Release build, `--features mecab`, Vibrato dictionary at
`%APPDATA%\dev.jparser.clipyomi\hints\current\system.dic`.

| | strict | tolerant | entry top-1 | compounds |
| --- | --- | --- | --- | --- |
| off, before | 0.7393 (1599) | 0.8003 (1731) | 0.9374 (1168 / 1246) | 0.6129 (114) |
| off, after | **0.7448 (1611)** | **0.8086 (1749)** | 0.9347 (1174 / 1256) | 0.6075 (113) |
| on, before | 0.8428 (1823) | 0.8826 (1909) | 0.9347 (1289 / 1379) | 0.6344 (118) |
| on, after | **0.8442 (1826)** | **0.8840 (1912)** | 0.9341 (1289 / 1380) | 0.6344 (118) |

Hints-on compound class lines do not move: `lexical` 46 / 112,
`false-match` 30 / 32, `transparent` 42 / 42. Read them, per the previous
handoff's §6, and not the ratio.

**Entry top-1's ratio falls in both states while its numerator holds or
rises.** The denominator is correctly-spanned tokens, and the change adds
spans: hints off, +6 correct out of +10 newly scored. Both v1 floors
(0.9159 / 0.9155) clear with margin, so criterion 2 still holds.

**The one real cost is one hints-off compound**, and `--compounds-detail`
names all three moved windows: `なきゃいけない` now merges as adjudicated
(`lexical` +1), `になって` and `いたから` now merge where the adjudication
splits them (`false-match` −2).

Bench, 10,040 chars, k = 3: 607 → 596 ms hints off, 539 → 543 ms hints on;
scaling 1.01x and 1.00x. Spurious merges 26 of 26. 656 workspace tests, 0
failed, 1 ignored (a doctest). `clippy --all-targets --features mecab` and
`fmt` clean. `vitest` 375 passed, `tsc --noEmit` clean.

The snapshot moved twice and both are corrections:
`待っ|ていた` ("severe") became `待って|いた` (`待つ` Te-form, `いる` Past), and
`ない` stopped offering `泣く`.

## 3. What this cost elsewhere

- **Three fixture rows deleted** from `conjugation_labels.txt`: `書い`,
  `行っ`, `高かっ`, the three bare `連用タ接続` stems, each labelled `-`. The
  table no longer generates them standalone, and the fixture's own rule is
  that rows may be added but never *quietly* dropped — the file carries a
  comment naming them and why.
- **28 rows added** for the `ちゃう` family, and one new label word,
  `Completive`. It is the only word in `label.rs` the fixture's Wikipedia
  terminology list does not carry; the fixture's header now says where it
  came from and why not "Regrettably".
- **The reachable-chain constant went 22,783 → 50,419.** Four lemmas
  attaching at two forms each, whose own seven forms are then attachment
  sites, is where the depth-2 term more than doubles. Test-walk only; bench
  says the runtime cost is nothing.
- **`the_same_headword_at_two_different_lengths_is_not_deduplicated_away`
  moved off `書いた` to `食べた`.** `書い` is `連用タ接続` and is no longer a
  candidate, so the pair the test protects would have been a single
  candidate and the assertion vacuous. `食べ` is `連用形` — a form that does
  end words — so it is still two.

## 4. Not done

- **Gate 6, the eighteen reported sentences, was not run.** The sentence
  set is not in the repository: `docs/` quotes two of the eighteen and
  records 486 / 630, but nothing holds the list. Whoever has it should
  re-run the cell, or commit the list so the gate stops depending on one
  session's context.
- **`忍` still reads `しのび`, not `しのぶ`.** Third difference in the
  screenshot pair, and not a defect this branch can fix: JMdict_e has no
  name readings, that is JMnedict, which this project does not ship. The
  previous handoff's §4 already lists it.
- **The other bound forms.** `未然形`, `ガル接続` (the `たい` row's comment has
  wanted this since Phase 1a) and the rest of the `接続` family are the same
  one-line change. Each deletes candidates of its own and wants its own
  score run — one at a time, cheapest defect first.
- **The engine's `いっち` → `icchi`.** See §1.
