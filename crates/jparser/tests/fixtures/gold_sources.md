# Gold corpus provenance

Companion to `gold.jsonl`. One row per `source` id. Nothing parses this file; a
reviewer reads it, and it carries the attribution tiers B and D legally require.

Written for [Engine v2 Phase 0](../../../../docs/superpowers/specs/2026-08-28-engine-v2-phase0-design.md) §4.4.

## Tiers

| Tier | Source | Licence |
| --- | --- | --- |
| A | Aozora Bunko, 新字新仮名 editions only, max 8 sentences per work | Public domain |
| B | ja.Wikinews / ja.Wikipedia | CC BY 2.5 / CC BY-SA 4.0 |
| C1 | Hand-written pastiche | MIT OR Apache-2.0, with the crate |
| D | Noise derived from A–C1 | Follows its source line |

Tier C2 (real `vn_data` lines) is never committed and has no rows here.

**That exclusion has a measurement consequence, and it is recorded rather than
implied.** The 150 committed sentences are 69 C1, 41 A, 32 B and 8 D — so
**visual-novel dialogue is 0 of 150**, while README's first paragraph names
visual novels as what this program is for. Every gate in master plan §5.2 is
therefore measured on pastiche, pre-war literary prose and Wikinews. The
licensing decision above is the right one and is not in question; how to weigh
the numbers that follow from it is in
[master plan §5.2](../../../../docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md),
"what register these numbers speak for", together with the unused
`gold_local.jsonl` mechanism that would close the gap.

## How this corpus was adjudicated, and what that means for its numbers

**Read this before quoting any score measured against `gold.jsonl`.**

150 sentences, 2,163 spans. Every one was sampled because jparser and Vibrato
disagreed about where a boundary belongs (design §6.1), so the corpus is
deliberately hard and its absolute accuracy is not a quality verdict on the
engine. The number exists to be comparable across phases, not to be quoted on
its own.

**The adjudication is a model's, reviewed by the author on one of its two
axes.** At the author's instruction the spans and entries were adjudicated by a
model, then reviewed. What that review covered, and what it did not:

| Axis | Reviewed | Evidence |
| --- | --- | --- |
| Entry decisions (`ent_seq`) | Yes | 14 corrections applied; `audit_entries.py` re-checks all 262 non-null entry decisions (per-surface rulings plus per-occurrence overrides, together resolving 1,554 spans) against full JMdict entries and a POS-consistency table — 0 flags, 0 entries absent from the subset |
| Span decisions (the cuts) | **No**, beyond a spot-read | No mechanical check exists |

The unreviewed axis is the one the headline metric measures. In 54 of the 150
sentences the model cut a boundary neither segmenter proposed, or removed one
both agreed on — those are the decisions made without either source of
evidence, and they are the ones to read first if a Phase 1 result ever looks
wrong. `corpus_staging/REVIEW.md` section 3 lists them.

**A known systematic bias, mitigated in the metric rather than the corpus.**
The sampler shows Vibrato's cuts, so the adjudication settled at short-unit
granularity throughout. That penalises the JMdict compound preservation master
plan §5.2 names as the reason the DP exists: gold splits `音がした` into
`音 / が / した` where jparser correctly keeps the entry `音がする [2826774]`.
Re-adjudicating at headword granularity would make the corpus agree with the
engine by construction, so `score` reports a second, compound-tolerant accuracy
instead and prints both. On this corpus that is 16.5 points of what strict
scoring calls error.

**The tolerant number over-forgives, measurably.** Roughly 28 of its 167
merges are spurious — jparser absorbing a particle into the following word and
landing on an unrelated kana-unified headword (`は` + `いた` matched as
履く [1399760]). Both endpoints are still gold boundaries, so the rule credits
them. True span quality is nearer 0.835 than the printed 0.8465. No mechanical
discriminator separates a spurious merge from a real compound, and none should
be added; **read a rising merge count as a warning, not a win.**

**Corrections are cheap.** `gold.jsonl` is append-only and last-wins (design
§4.2), so a corrected sentence is one appended record plus a re-run of `score`.
The corpus is frozen for comparability, not frozen permanently.

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

## Sources

One row per work or article the committed corpus draws on. A row here means
`gold.jsonl` cites that source; sources sampled into the pool but not
adjudicated into the corpus were dropped, so there are no orphaned claims.

### Tier A — Aozora Bunko (public domain, 新字新仮名 only)

| source id | author | title | Aozora card | 底本 | retrieved |
| --- | --- | --- | --- | --- | --- |
| `a:000206-*` | 有島武郎 | 小さき者へ | https://www.aozora.gr.jp/cards/000025/card206.html | 小さき者へ・生れ出づる悩み | 2026-08-28 |
| `a:000323-*` | 国木田独歩 | 牛肉と馬鈴薯 | https://www.aozora.gr.jp/cards/000038/card323.html | 牛肉と馬鈴薯・酒中日記 | 2026-08-28 |
| `a:000424-*` | 梶井基次郎 | 檸檬 | https://www.aozora.gr.jp/cards/000074/card424.html | 檸檬・ある心の風景　他二十編 | 2026-08-28 |
| `a:000585-*` | 佐々木味津三 | 右門捕物帖 | https://www.aozora.gr.jp/cards/000111/card585.html | 右門捕物帖（一） | 2026-08-28 |
| `a:000883-*` | 渡辺温 | 嘘 | https://www.aozora.gr.jp/cards/000020/card883.html | アンドロギュノスの裔 | 2026-08-28 |
| `a:001567-*` | 太宰治 | 走れメロス | https://www.aozora.gr.jp/cards/000035/card1567.html | 太宰治全集3 | 2026-08-28 |
| `a:002722-*` | 佐左木俊郎 | 芋 | https://www.aozora.gr.jp/cards/000134/card2722.html | 日本プロレタリア文学集・11　「文芸戦線」作家集（二） | 2026-08-28 |
| `a:003521-*` | 海野十三 | 雷 | https://www.aozora.gr.jp/cards/000160/card3521.html | 海野十三全集　第5巻　浮かぶ飛行島 | 2026-08-28 |
| `a:004601-*` | 宮沢賢治 | フランドン農学校の豚 | https://www.aozora.gr.jp/cards/000081/card4601.html | 新編　風の又三郎 | 2026-08-28 |
| `a:004842-*` | 堀辰雄 | 窓 | https://www.aozora.gr.jp/cards/001030/card4842.html | 堀辰雄集　新潮日本文学16 | 2026-08-28 |
| `a:043426-*` | 蘭郁二郎 | 穴 | https://www.aozora.gr.jp/cards/000325/card43426.html | 怪奇探偵小説名作選7　蘭郁二郎集　魔像 | 2026-08-28 |
| `a:045245-*` | 森鴎外 | 高瀬舟 | https://www.aozora.gr.jp/cards/000129/card45245.html | 山椒大夫・高瀬舟他四篇 | 2026-08-28 |
| `a:045339-*` | 国枝史郎 | 剣侠 | https://www.aozora.gr.jp/cards/000255/card45339.html | 国枝史郎伝奇全集　巻四 | 2026-08-28 |
| `a:045670-*` | 林不忘 | 魔像 | https://www.aozora.gr.jp/cards/000290/card45670.html | カラー版国民の文学　10　林不忘 | 2026-08-28 |
| `a:048085-*` | 小酒井不木 | 按摩 | https://www.aozora.gr.jp/cards/000262/card48085.html | 怪奇探偵小説名作選１　小酒井不木集 | 2026-08-28 |
| `a:049597-*` | 中里介山 | 山道 | https://www.aozora.gr.jp/cards/000283/card49597.html | 山の旅　大正・昭和篇 | 2026-08-28 |
| `a:052446-*` | 吉川英治 | 脚 | https://www.aozora.gr.jp/cards/001562/card52446.html | 柳生月影抄　名作短編集（二） | 2026-08-28 |
| `a:053148-*` | 小川未明 | 櫛 | https://www.aozora.gr.jp/cards/001475/card53148.html | 文豪怪談傑作選　小川未明集　幽霊船 | 2026-08-28 |
| `a:054479-*` | 大倉燁子 | 鷺娘 | https://www.aozora.gr.jp/cards/001669/card54479.html | 大倉燁子探偵小説選 | 2026-08-28 |
| `a:056104-*` | 野村胡堂 | 礫心中 | https://www.aozora.gr.jp/cards/001670/card56104.html | 野村胡堂伝奇幻想小説集成 | 2026-08-28 |
| `a:057516-*` | 江戸川乱歩 | 鬼 | https://www.aozora.gr.jp/cards/001779/card57516.html | 江戸川乱歩全集　第8巻　目羅博士の不思議な犯罪 | 2026-08-28 |
| `a:057582-*` | 山本周五郎 | 薊 | https://www.aozora.gr.jp/cards/001869/card57582.html | 山本周五郎全集第二十八巻　ちいさこべ・落葉の隣り | 2026-08-28 |
| `a:058135-*` | 久生十蘭 | 生霊 | https://www.aozora.gr.jp/cards/001224/card58135.html | 久生十蘭ジュラネスク　珠玉傑作集 | 2026-08-28 |
| `a:059739-*` | 山川方夫 | 菊 | https://www.aozora.gr.jp/cards/001801/card59739.html | 親しい友人たち　山川方夫ミステリ傑作選 | 2026-08-28 |

### Tier B — ja.Wikinews (CC BY 2.5)

Attribution required. Revision ids are recorded because the article
can be edited after retrieval; the sentence is from that revision.

| source id | title | revision | url | retrieved |
| --- | --- | --- | --- | --- |
| `b:wn10681-*` | 2007年ノーベル平和賞、米元副大統領アル・ゴア氏とIPCCへ | 197775 | https://ja.wikinews.org/wiki/2007%E5%B9%B4%E3%83%8E%E3%83%BC%E3%83%99%E3%83%AB%E5%B9%B3%E5%92%8C%E8%B3%9E%E3%80%81%E7%B1%B3%E5%85%83%E5%89%AF%E5%A4%A7%E7%B5%B1%E9%A0%98%E3%82%A2%E3%83%AB%E3%83%BB%E3%82%B4%E3%82%A2%E6%B0%8F%E3%81%A8IPCC%E3%81%B8 | 2026-08-28 |
| `b:wn10805-*` | 2007年セ・リーグクライマックス 中日がリーグ戦の雪辱 日本シリーズは2年連続対日本ハム戦 | 194688 | https://ja.wikinews.org/wiki/2007%E5%B9%B4%E3%82%BB%E3%83%BB%E3%83%AA%E3%83%BC%E3%82%B0%E3%82%AF%E3%83%A9%E3%82%A4%E3%83%9E%E3%83%83%E3%82%AF%E3%82%B9_%E4%B8%AD%E6%97%A5%E3%81%8C%E3%83%AA%E3%83%BC%E3%82%B0%E6%88%A6%E3%81%AE%E9%9B%AA%E8%BE%B1_%E6%97%A5%E6%9C%AC%E3%82%B7%E3%83%AA%E3%83%BC%E3%82%BA%E3%81%AF2%E5%B9%B4%E9%80%A3%E7%B6%9A%E5%AF%BE%E6%97%A5%E6%9C%AC%E3%83%8F%E3%83%A0%E6%88%A6 | 2026-08-28 |
| `b:wn11044-*` | 2007サッカーJ2リーグ、札幌と東京VがJ1昇格 | 194872 | https://ja.wikinews.org/wiki/2007%E3%82%B5%E3%83%83%E3%82%AB%E3%83%BCJ2%E3%83%AA%E3%83%BC%E3%82%B0%E3%80%81%E6%9C%AD%E5%B9%8C%E3%81%A8%E6%9D%B1%E4%BA%ACV%E3%81%8CJ1%E6%98%87%E6%A0%BC | 2026-08-28 |
| `b:wn11412-*` | 2007年の年末ジャンボ宝くじ、抽選行われる | 194960 | https://ja.wikinews.org/wiki/2007%E5%B9%B4%E3%81%AE%E5%B9%B4%E6%9C%AB%E3%82%B8%E3%83%A3%E3%83%B3%E3%83%9C%E5%AE%9D%E3%81%8F%E3%81%98%E3%80%81%E6%8A%BD%E9%81%B8%E8%A1%8C%E3%82%8F%E3%82%8C%E3%82%8B | 2026-08-28 |
| `b:wn12024-*` | JRおおさか東線部分開業 | 195200 | https://ja.wikinews.org/wiki/JR%E3%81%8A%E3%81%8A%E3%81%95%E3%81%8B%E6%9D%B1%E7%B7%9A%E9%83%A8%E5%88%86%E9%96%8B%E6%A5%AD | 2026-08-28 |
| `b:wn13213-*` | Jリーグ・浦和対G大阪戦でサポーターの暴動騒ぎ | 195166 | https://ja.wikinews.org/wiki/J%E3%83%AA%E3%83%BC%E3%82%B0%E3%83%BB%E6%B5%A6%E5%92%8C%E5%AF%BEG%E5%A4%A7%E9%98%AA%E6%88%A6%E3%81%A7%E3%82%B5%E3%83%9D%E3%83%BC%E3%82%BF%E3%83%BC%E3%81%AE%E6%9A%B4%E5%8B%95%E9%A8%92%E3%81%8E | 2026-08-28 |
| `b:wn13606-*` | 3連単馬券、全レースでの発売へと拡大 - 中央競馬など | 195600 | https://ja.wikinews.org/wiki/3%E9%80%A3%E5%8D%98%E9%A6%AC%E5%88%B8%E3%80%81%E5%85%A8%E3%83%AC%E3%83%BC%E3%82%B9%E3%81%A7%E3%81%AE%E7%99%BA%E5%A3%B2%E3%81%B8%E3%81%A8%E6%8B%A1%E5%A4%A7_-_%E4%B8%AD%E5%A4%AE%E7%AB%B6%E9%A6%AC%E3%81%AA%E3%81%A9 | 2026-08-28 |
| `b:wn16800-*` | サッカー・Jリーグ、史上初の「再開試合」は川崎Fが逃げ切る | 195952 | https://ja.wikinews.org/wiki/%E3%82%B5%E3%83%83%E3%82%AB%E3%83%BC%E3%83%BBJ%E3%83%AA%E3%83%BC%E3%82%B0%E3%80%81%E5%8F%B2%E4%B8%8A%E5%88%9D%E3%81%AE%E3%80%8C%E5%86%8D%E9%96%8B%E8%A9%A6%E5%90%88%E3%80%8D%E3%81%AF%E5%B7%9D%E5%B4%8EF%E3%81%8C%E9%80%83%E3%81%92%E5%88%87%E3%82%8B | 2026-08-28 |
| `b:wn16962-*` | 2009プロ野球 巨人が7年ぶりの日本一奪回 | 195972 | https://ja.wikinews.org/wiki/2009%E3%83%97%E3%83%AD%E9%87%8E%E7%90%83_%E5%B7%A8%E4%BA%BA%E3%81%8C7%E5%B9%B4%E3%81%B6%E3%82%8A%E3%81%AE%E6%97%A5%E6%9C%AC%E4%B8%80%E5%A5%AA%E5%9B%9E | 2026-08-28 |
| `b:wn18704-*` | JR西日本の電車で緊急無線用ヒューズの盗難相次ぐ | 196538 | https://ja.wikinews.org/wiki/JR%E8%A5%BF%E6%97%A5%E6%9C%AC%E3%81%AE%E9%9B%BB%E8%BB%8A%E3%81%A7%E7%B7%8A%E6%80%A5%E7%84%A1%E7%B7%9A%E7%94%A8%E3%83%92%E3%83%A5%E3%83%BC%E3%82%BA%E3%81%AE%E7%9B%97%E9%9B%A3%E7%9B%B8%E6%AC%A1%E3%81%90 | 2026-08-28 |
| `b:wn19194-*` | 2010年ノーベル化学賞 鈴木章氏・根岸英一氏ら3人が受賞 日本人のノーベル賞受賞者18人に | 196672 | https://ja.wikinews.org/wiki/2010%E5%B9%B4%E3%83%8E%E3%83%BC%E3%83%99%E3%83%AB%E5%8C%96%E5%AD%A6%E8%B3%9E_%E9%88%B4%E6%9C%A8%E7%AB%A0%E6%B0%8F%E3%83%BB%E6%A0%B9%E5%B2%B8%E8%8B%B1%E4%B8%80%E6%B0%8F%E3%82%893%E4%BA%BA%E3%81%8C%E5%8F%97%E8%B3%9E_%E6%97%A5%E6%9C%AC%E4%BA%BA%E3%81%AE%E3%83%8E%E3%83%BC%E3%83%99%E3%83%AB%E8%B3%9E%E5%8F%97%E8%B3%9E%E8%80%8518%E4%BA%BA%E3%81%AB | 2026-08-28 |
| `b:wn21710-*` | 300系新幹線電車がラストラン | 197037 | https://ja.wikinews.org/wiki/300%E7%B3%BB%E6%96%B0%E5%B9%B9%E7%B7%9A%E9%9B%BB%E8%BB%8A%E3%81%8C%E3%83%A9%E3%82%B9%E3%83%88%E3%83%A9%E3%83%B3 | 2026-08-28 |
| `b:wn2172-*` | 2006年度の連続テレビ小説ヒロイン決まる | 174937 | https://ja.wikinews.org/wiki/2006%E5%B9%B4%E5%BA%A6%E3%81%AE%E9%80%A3%E7%B6%9A%E3%83%86%E3%83%AC%E3%83%93%E5%B0%8F%E8%AA%AC%E3%83%92%E3%83%AD%E3%82%A4%E3%83%B3%E6%B1%BA%E3%81%BE%E3%82%8B | 2026-08-28 |
| `b:wn24579-*` | 2013年の今年の漢字は「輪」 東京五輪決定や災害被災地支援などを反映 | 197495 | https://ja.wikinews.org/wiki/2013%E5%B9%B4%E3%81%AE%E4%BB%8A%E5%B9%B4%E3%81%AE%E6%BC%A2%E5%AD%97%E3%81%AF%E3%80%8C%E8%BC%AA%E3%80%8D_%E6%9D%B1%E4%BA%AC%E4%BA%94%E8%BC%AA%E6%B1%BA%E5%AE%9A%E3%82%84%E7%81%BD%E5%AE%B3%E8%A2%AB%E7%81%BD%E5%9C%B0%E6%94%AF%E6%8F%B4%E3%81%AA%E3%81%A9%E3%82%92%E5%8F%8D%E6%98%A0 | 2026-08-28 |
| `b:wn25517-*` | JR品川駅・田町駅周辺の再開発構想案発表 | 199322 | https://ja.wikinews.org/wiki/JR%E5%93%81%E5%B7%9D%E9%A7%85%E3%83%BB%E7%94%B0%E7%94%BA%E9%A7%85%E5%91%A8%E8%BE%BA%E3%81%AE%E5%86%8D%E9%96%8B%E7%99%BA%E6%A7%8B%E6%83%B3%E6%A1%88%E7%99%BA%E8%A1%A8 | 2026-08-28 |
| `b:wn27619-*` | JR山手線の線路脇のケーブルが燃える 山手線他で一時運転見合わせ | 198346 | https://ja.wikinews.org/wiki/JR%E5%B1%B1%E6%89%8B%E7%B7%9A%E3%81%AE%E7%B7%9A%E8%B7%AF%E8%84%87%E3%81%AE%E3%82%B1%E3%83%BC%E3%83%96%E3%83%AB%E3%81%8C%E7%87%83%E3%81%88%E3%82%8B_%E5%B1%B1%E6%89%8B%E7%B7%9A%E4%BB%96%E3%81%A7%E4%B8%80%E6%99%82%E9%81%8B%E8%BB%A2%E8%A6%8B%E5%90%88%E3%82%8F%E3%81%9B | 2026-08-28 |
| `b:wn28412-*` | FIFAの8時間におよぶ聴聞会でプラティニ氏は不正を否定 | 173789 | https://ja.wikinews.org/wiki/FIFA%E3%81%AE8%E6%99%82%E9%96%93%E3%81%AB%E3%81%8A%E3%82%88%E3%81%B6%E8%81%B4%E8%81%9E%E4%BC%9A%E3%81%A7%E3%83%97%E3%83%A9%E3%83%86%E3%82%A3%E3%83%8B%E6%B0%8F%E3%81%AF%E4%B8%8D%E6%AD%A3%E3%82%92%E5%90%A6%E5%AE%9A | 2026-08-28 |
| `b:wn28781-*` | 【熊本地震】オープンストリートマップが救援地図作り「クライシスマッピング」を開始 | 198464 | https://ja.wikinews.org/wiki/%E3%80%90%E7%86%8A%E6%9C%AC%E5%9C%B0%E9%9C%87%E3%80%91%E3%82%AA%E3%83%BC%E3%83%97%E3%83%B3%E3%82%B9%E3%83%88%E3%83%AA%E3%83%BC%E3%83%88%E3%83%9E%E3%83%83%E3%83%97%E3%81%8C%E6%95%91%E6%8F%B4%E5%9C%B0%E5%9B%B3%E4%BD%9C%E3%82%8A%E3%80%8C%E3%82%AF%E3%83%A9%E3%82%A4%E3%82%B7%E3%82%B9%E3%83%9E%E3%83%83%E3%83%94%E3%83%B3%E3%82%B0%E3%80%8D%E3%82%92%E9%96%8B%E5%A7%8B | 2026-08-28 |
| `b:wn3065-*` | 2007年度の大河ドラマは「風林火山」 | 176315 | https://ja.wikinews.org/wiki/2007%E5%B9%B4%E5%BA%A6%E3%81%AE%E5%A4%A7%E6%B2%B3%E3%83%89%E3%83%A9%E3%83%9E%E3%81%AF%E3%80%8C%E9%A2%A8%E6%9E%97%E7%81%AB%E5%B1%B1%E3%80%8D | 2026-08-28 |
| `b:wn4245-*` | 2005年10月は先月に続き地球規模で高温、10月では観測史上最も高い気温に | 194844 | https://ja.wikinews.org/wiki/2005%E5%B9%B410%E6%9C%88%E3%81%AF%E5%85%88%E6%9C%88%E3%81%AB%E7%B6%9A%E3%81%8D%E5%9C%B0%E7%90%83%E8%A6%8F%E6%A8%A1%E3%81%A7%E9%AB%98%E6%B8%A9%E3%80%8110%E6%9C%88%E3%81%A7%E3%81%AF%E8%A6%B3%E6%B8%AC%E5%8F%B2%E4%B8%8A%E6%9C%80%E3%82%82%E9%AB%98%E3%81%84%E6%B0%97%E6%B8%A9%E3%81%AB | 2026-08-28 |
| `b:wn44498-*` | みちびき5号機搭載H3ロケット 打ち上げ失敗 | 200235 | https://ja.wikinews.org/wiki/%E3%81%BF%E3%81%A1%E3%81%B3%E3%81%8D5%E5%8F%B7%E6%A9%9F%E6%90%AD%E8%BC%89H3%E3%83%AD%E3%82%B1%E3%83%83%E3%83%88_%E6%89%93%E3%81%A1%E4%B8%8A%E3%81%92%E5%A4%B1%E6%95%97 | 2026-08-28 |
| `b:wn4998-*` | 2006年お年玉付き年賀はがきの当選番号決定 | 198848 | https://ja.wikinews.org/wiki/2006%E5%B9%B4%E3%81%8A%E5%B9%B4%E7%8E%89%E4%BB%98%E3%81%8D%E5%B9%B4%E8%B3%80%E3%81%AF%E3%81%8C%E3%81%8D%E3%81%AE%E5%BD%93%E9%81%B8%E7%95%AA%E5%8F%B7%E6%B1%BA%E5%AE%9A | 2026-08-28 |
| `b:wn6519-*` | 2006FIFAワールドカップグループF、日本は1-4でブラジルに逆転負け | 198810 | https://ja.wikinews.org/wiki/2006FIFA%E3%83%AF%E3%83%BC%E3%83%AB%E3%83%89%E3%82%AB%E3%83%83%E3%83%97%E3%82%B0%E3%83%AB%E3%83%BC%E3%83%97F%E3%80%81%E6%97%A5%E6%9C%AC%E3%81%AF1-4%E3%81%A7%E3%83%96%E3%83%A9%E3%82%B8%E3%83%AB%E3%81%AB%E9%80%86%E8%BB%A2%E8%B2%A0%E3%81%91 | 2026-08-28 |
| `b:wn9103-*` | K-1、秋山選手に無期限出場停止を課す | 194376 | https://ja.wikinews.org/wiki/K-1%E3%80%81%E7%A7%8B%E5%B1%B1%E9%81%B8%E6%89%8B%E3%81%AB%E7%84%A1%E6%9C%9F%E9%99%90%E5%87%BA%E5%A0%B4%E5%81%9C%E6%AD%A2%E3%82%92%E8%AA%B2%E3%81%99 | 2026-08-28 |

### Tier C1 — hand-written pastiche (project licence)

Original composition; every character and place name is invented.
Class codes are in `corpus_staging/pool_c1.tsv` column 3 and match the
measured register classes above: `mim` reduplicated mimetics, `kat`
unregistered katakana proper nouns, `elo` elongation runs, `cas` casual
and contracted speech.


### Tier D — derived noise (licence follows each parent)

| source id | parent | group | transform |
| --- | --- | --- | --- |
| `d:a:000323-3` | `a:000323-3` | capture | halfwidth-question — ？ arrives as halfwidth ? |
| `d:a:001567-3` | `a:001567-3` | capture | ideographic-space — U+3000 lands mid-sentence |
| `d:a:003521-6` | `a:003521-6` | vertical | chouonpu-pipe — ー rendered as ｜ |
| `d:a:045245-6` | `a:045245-6` | vertical | vertical-brackets — 「」 rendered as the vertical presentation forms ﹁﹂ |
| `d:a:048085-2` | `a:048085-2` | capture | halfwidth-question — ？ arrives as halfwidth ? |
| `d:a:058135-5` | `a:058135-5` | vertical | vertical-ellipsis — …… rendered as ・・・ |
| `d:b:wn13606-1` | `b:wn13606-1` | vertical | chouonpu-pipe — ー rendered as ｜ |
| `d:b:wn28412-1` | `b:wn28412-1` | vertical | chouonpu-bar — ー rendered as 丨 |
