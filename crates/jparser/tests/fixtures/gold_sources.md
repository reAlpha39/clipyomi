# Gold corpus provenance

Companion to `gold.jsonl`. One row per `source` id. Nothing parses this file; a
reviewer reads it, and it carries the attribution tiers B and D legally require.

Written for [Engine v2 Phase 0](../../../../docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md) §4.4.

## Tiers

| Tier | Source | Licence |
| --- | --- | --- |
| A | Aozora Bunko, 新字新仮名 editions only, max 8 sentences per work | Public domain |
| B | ja.Wikinews / ja.Wikipedia | CC BY 2.5 / CC BY-SA 4.0 |
| C1 | Hand-written pastiche | GPL-2.0-only, with the crate |
| D | Noise derived from A–C1 | Follows its source line |

Tier C2 (real `vn_data` lines) is never committed and has no rows here.

## Tier A stripping rules

Aozora ruby (`《…》`), reading marks (`｜`), and editorial annotations
(`［＃…］`) are removed before a line enters the pool. Note the collision: `｜`
is both an Aozora ruby delimiter and a tier D vertical-text artefact. Stripping
happens at pool-construction time, before any tier D derivation, so a `｜` in a
committed line is a deliberate artefact and never residue.

Pre-war orthography is rejected outright. Both segmenters flail on 旧字体 and
旧仮名遣い, so an unfiltered Aozora sample returns disproportionately many of
them — a distribution ClipYomi never sees, which would then be what Phase 1 is
tuned against.

## Tier D derivation table

**Real capture artefacts**, observed in `vn_data`: ASCII `......` where `……`
belongs; halfwidth `?` and `!`; U+3000 mid-sentence; speaker-name prefixes;
`――` em-dash runs; halfwidth digits in Japanese context (`1つ`).

**Synthetic OCR confusables**, which text hooking cannot produce and only real
OCR does: 力/カ, 口/ロ, 一/ー/丨, ニ/二, 八/ハ, 卜/ト, dropped dakuten
(`ば`→`は`), full/half-width mixing, spurious intra-word space.

**Vertical-text artefacts**: `ー` rendered as `｜` or `丨`, `〜` as `～`.

## Tier C1 coverage intent

Read out of a `jparser-cli diff` run over the whole `vn_data` pool
(67,469 captured lines, 66,114 kept after cleaning), so the pastiche covers
the register classes that actually generate disagreement rather than the
ones a plan guessed at. The run emitted 23,628 disagreement records (about
36% of the pool); the classes below are read from its top 100 by conflict
count. Two of the design brief's five guessed classes — contracted verb
forms and sentence-final particle stacks — turned out not to drive real
conflicts: both are common in the pool, but jparser handles them as benign
coarsening (it merges whole Vibrato tokens, e.g. a te-form plus its
auxiliary, or a numeral plus its counter) rather than cutting inside one, so
the sampler correctly excludes them. The four classes actually observed:

- **Reduplicated onomatopoeia / mimetic repetition** — a short hiragana mora
  unit (2-4 kana) repeats many times with no kanji anchor (invented
  illustration: `ぱたぱたぱたぱた`, footsteps; `ごろごろごろごろ`, rolling
  thunder). jparser re-cuts the repeat every 1-2 characters; Vibrato folds
  the whole run into one token, confidently — the cleanest instances of this
  class carry 0-2 unknown-reading tokens. By far the largest driver: the
  primary cause in roughly 65 of the top 100 disagreements, and a
  contributing factor in several more that mix a repeat into a longer line.
  The strongest evidence in the sample, because Vibrato is not guessing.
- **Elongated interjection / scream vowel runs** — a chōonpu (`ー`) run three
  or more marks long, or an identical vowel kana repeated (invented
  illustration: `きゃーーーー`; `あぁぁぁぁ`), usually mixed with single-mora
  interjection fragments. jparser cuts the elongation mark-by-mark and each
  fragment separately; Vibrato groups the whole run into one token. Primary
  driver in about 7 of the top 100 on its own, but present in roughly 40 once
  lines that combine it with onomatopoeia are counted. The weakest evidence
  of the four classes: unknown-token counts here are the highest in the
  sample (never below 6 per record) — both segmenters have little lexical
  purchase on these strings, not only jparser.
- **Unregistered katakana proper nouns** — a multi-mora katakana name (a
  character or place name) absent from JMdict/IPADIC. jparser fragments it
  into 1-3 kana chunks; Vibrato's katakana-run handling keeps the whole name
  as one token. Primary driver in about 19 of the top 100, a secondary
  contributor in several more narrative lines that name more than one
  character. Mid-confidence: unknown-token counts sit between the other two
  classes (Vibrato is reasonably sure these are single tokens even without
  recognising them).
- **Everything else** (~7 of the top 100) — one record is a Vibrato failure
  rather than a jparser one (an unregistered name plus a stray chōonpu
  apparently confused Vibrato's own segmentation into swallowing several real
  words as one token); a couple are one-off phonetic respellings used for a
  single character's speech affectation. Neither generalises into a tier C1
  category on its own.
