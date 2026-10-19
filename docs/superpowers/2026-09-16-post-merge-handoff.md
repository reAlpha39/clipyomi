# After the merge — handoff

**Date:** 2026-09-16
**Branch:** `develop`, tip `e80cf2c`, **27 commits ahead of `origin/develop`**
**Predecessor:** [`2026-09-16-engine-v2-phase4-handoff.md`](2026-09-16-engine-v2-phase4-handoff.md)

Engine v2 merged into `develop` as `0ef1ef9` with all four master-plan criteria
met. Everything below happened *after* that, driven by screenshots of the app
compared against Translation Aggregator.

## 1. Where the numbers stand

Gold index from `gold_jmdict_subset.xml`, 150 sentences, 2,163 spans.

| | hints off | hints on |
| --- | --- | --- |
| span accuracy | 0.7393 (1599) | **0.8428 (1823)** |
| entry top-1 | 0.9374 | **0.9347** |
| span tolerant | 0.8003 (1731) | 0.8826 (1909) |
| compounds | 0.6129 (114 / 186) | 0.6344 (118 / 186) |
| by class, hints on | — | lexical 46 / 112, false-match 30 / 32, transparent 42 / 42 |

v1 floors for criterion 2 were 0.6810 / 0.6963 span accuracy and 0.9159 /
0.9155 entry top-1. All four clear with margin. Spurious merges 26 of 26,
snapshot unmoved, the eighteen reported sentences 486 of 630 against
hints-off's 487, bench 1.00x, **656 workspace tests, 0 failed, 1 ignored**
(a doctest).

Master plan §5.2's current-baseline block and §2's criterion 2 table are
current as of this tip. **Rewrite them in the same commit as any change that
moves a cell** — that rule exists because the block was two phases stale and
that is why criterion 2 went unmet for eight phases unnoticed.

## 2. What shipped, and the one idea behind most of it

Nearly every defect below is the same arithmetic. `COMMON` is −2 a character
against `COVER`'s +1, so **a COMMON span costs −1 a character and a
non-COMMON one +1** — a tiling of COMMON fragments beats a longer non-COMMON
dictionary word, at every magnitude. `COMMON`'s doc comment claims
length-proportionality closed §4.3's shattering class; it closed it *between
COMMON candidates only*.

The visible form: on
`「ある日起きたら、英国海軍第一卿になって…」` the engine ranked the cover
keeping all four of `ある日`, `起きたら`, `第一` and `なって` whole
**eighteenth**, behind every cover refusing at least one.

| commit | what it fixed |
| --- | --- |
| `7e51dde` | `PRONOUNCE_CROSS` no longer charges for a boundary inside an inflection — `なった`, `あった`, 19 instances in the corpus. The chain's range as a whole, not each step's: `なった` is two steps and IPADIC cuts the junction between them |
| `7e51dde` | `assemble::order` breaks entry ties on JMdict's sense count, placed last where `entry_id` decided arbitrarily (炒る "to roast" beat 行く "to go" on a smaller number) |
| `f36c7e1` | a hiragana-free token opens if it tiles into pieces of ≥3 chars — `ロイヤル`+`ネイビー`+`ガール` yes, `ニュルンベルク` no |
| `1661396` | `dedup_shallowest` keeps the **exact** record when two kana headwords collapse to one key; a hiragana input was being charged `INEXACT` for a headword spelled identically |
| `6701cd4` | 81-entry Vibrato user lexicon for words JMdict has and IPADIC cuts |
| `2d121c0` | `ある日`, `その日` added to it — a deliberate exception to the `exp` filter |
| `311ccdd` | **`MATCH = 2`**, reopening what §4.3 closed. Also fixes `ミスった` |
| `9edb454` / `15942d8` | `DIGIT_COUNTER = -3`, gated on the merged reading differing from its parts — `1週間` → `いっしゅうかん` |
| `36baa3e` | `DIGIT_NONCOUNTER = 3` — `N人目` → `N人` + `目`, because `人目` is `ひとめ`, "public gaze" |
| `ebfa26a` | frontend romaji handles `ー`; it was copying the mark out verbatim while the engine romanized it correctly |

## 3. Measured and refused — do not re-litigate without new evidence

- **A larger `MATCH`.** 6 is what `ある日` needs unaided. Refused: the compound
  headline rises on `lexical` while **`transparent` collapses 42 → 30** and
  `false-match` 30 → 27. Read the class lines, never the headline.
- **`PARTICLE_CROSS`.** Built, works, needed only at `MATCH ≥ 6`, a wash at 2.
- **A bulk user lexicon.** 12,643 shattered words; seven in ten split into
  pieces of 3+ characters, where splitting is *better*. The 956 damaged
  hiragana ones occur **zero times** in 3.1M characters of real text — IPADIC's
  gaps are rare words by construction. The text-ranked 164 cost 2 gold spans
  through global Viterbi perturbation. 81 is the largest size measured free.
- **Promoting prefix entries** so `お` reads "honorific" rather than "tail":
  −15 entry top-1, and −7 even narrowed to pure-`pref` entries.

## 4. Known-wrong, unfixed

- **`8本` reads `はち` + `もと`.** JMdict has no `8本` entry; `本` alone reads
  `もと`. Needs counter phonology as data.
- **`お邪魔` splits.** `[primary]` against `邪魔`'s COMMON — §2's arithmetic.
  Needs `MATCH ≥ 6` or a lexicon entry, and `お`-prefixed words are a large
  open class.
- **`お` ranks as "tail (of an animal)".** §3's last bullet.
- **`忍` reads `しのび`, not the name `しのぶ`.** JMdict_e carries no name
  readings; that is JMnedict, which this project does not ship.
- **`いちゃいちゃ` in isolation.** Fixed via the lexicon, not the cost model;
  the class beyond those 81 words is open.

## 5. Also outstanding

- **27 commits unpushed.** `origin/develop` is still at `0ef1ef9`.
- **The claims audit's §4**, roughly seventy findings across `src-tauri`,
  `src/`, the example probes and the three source crates
  ([`2026-09-16-claims-audit.md`](2026-09-16-claims-audit.md)). Leads to
  confirm, not facts — an auditor was wrong at least once.
- **The hints-on bench band** (1.07x–1.12x) is stale; readings now cluster
  1.00x–1.07x. Re-derive rather than carry forward.
- **Six darwin screenshot baselines**, owed since Phase 3a/3b. Needs a Mac.
- **CI has never been read back.** `gh` is not installed here; `ci.yml` is on
  `origin/main`, so it has had every chance to run.

## 6. How to work on this

`examples/` carries the instruments, and they are the reason the refusals
above are evidence rather than opinion:

- `span_dump.rs` — segments, boundaries, relaxed positions and entries as
  JSONL. **`score`'s `strict` credits an exactly-placed segment whether or not
  it matched**, so the totals hide direction; always decompose.
- `order_sweep.rs` — `order` runs after the DP and cannot move a boundary, so
  every ordering variant scores offline from one parse.
- `shatter_sweep.rs` — words JMdict has and the tokenizer cuts.

And the habit that found most of this: **read the compound metric's three
class lines, not its ratio.** Two changes this session looked good on the
headline and were over-merging.
