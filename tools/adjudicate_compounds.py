#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Turn `common_inversion_probe --emit` output into compound_gold.jsonl.

Kept for reproducibility, like tools/extract_jmdict_subset.py: the committed
fixture is the artifact, and this is what rebuilds it.

The probe enumerates every window where the engine faced a compound decision
on the gold corpus: the merges it made and the compounds it declined. This
file holds the adjudication of those windows and applies it.

Adjudication is per distinct `(surface, headword)` type, not per window, so a
compound occurring in five sentences is judged once. Four classes, defined in
`crates/jparser/src/bin/jparser-cli/compounds.rs`:

  lexical      a dictionary unit a reader looks up whole -- compound noun,
               idiomatic expression, numeral + counter, a conjugated single
               verb, or a fixed grammatical construction whose force is not
               its parts stacked. The parse should place a segment here.
  transparent  function-word stacking whose parts are each what the reader
               needs to see. It should not.
  false-match  the headword is a different word from the text; merging would
               show a wrong gloss. It should not.
  ambiguous    context-dependent at type level; excluded from the metric and
               counted beside it rather than guessed.

**This is a model's adjudication.** build_gold.py's note applies: the author
may revise any verdict, and because each record carries its class, reversing a
whole class is a filter over the output rather than a re-read of the corpus.

Two things are resolved per sentence rather than per type. Overlapping
`lexical` windows cannot both be segments, so the longest wins and the others
carry `expect: split` with a note naming the winner; DEMOTE below overrides
that rule where longest-wins is wrong.

Usage:
  python3 tools/adjudicate_compounds.py decisions.jsonl out.jsonl
"""

import collections
import io
import json
import sys

V = {
    # --- expressions: meaning not recoverable from the parts -------------
    ("音がした", "音がする"): "lexical",
    ("音がする", "音がする"): "lexical",
    ("音を立てた", "音を立てる"): "lexical",
    ("風を切る", "風を切る"): "lexical",
    ("首を振った", "首を振る"): "lexical",
    ("峠を越える", "峠を越える"): "lexical",
    ("気を失って", "気を失う"): "lexical",
    ("手をやった", "手をやる"): "lexical",
    ("服を着て", "服を着る"): "lexical",
    ("調子を取った", "調子を取る"): "lexical",
    ("注意を促して", "注意を促す"): "lexical",
    ("運転を見合わせた", "運転を見合わせる"): "lexical",
    ("恐れ乍ら", "恐れ乍ら"): "lexical",
    ("このうえ", "このうえ"): "lexical",
    # --- compound verbs -------------------------------------------------
    ("出ていった", "出ていく"): "lexical",
    ("生きて行く", "生きて行く"): "lexical",
    # --- conjugated single verbs (not compounds at all) -----------------
    ("行かなきゃ", "行く"): "lexical",
    ("見ねえ", "見る"): "lexical",
    # --- compound nouns -------------------------------------------------
    ("副大統領", "副大統領"): "lexical",
    ("大統領選挙", "大統領選挙"): "lexical",
    ("名誉教授", "名誉教授"): "lexical",
    ("日本郵政公社", "日本郵政公社"): "lexical",
    ("郵政公社", "郵政公社"): "lexical",
    ("定期点検", "定期点検"): "lexical",
    ("月平均", "月平均"): "lexical",
    ("総売り上げ", "総売り上げ"): "lexical",
    ("鉄道ファン", "鉄道ファン"): "lexical",
    ("ラストラン", "ラストラン"): "lexical",
    ("タイムリーヒット", "タイムリーヒット"): "lexical",
    ("ノーベル化学賞", "ノーベル化学賞"): "lexical",
    ("凱旋将軍", "凱旋将軍"): "lexical",
    ("二重三重", "二重三重"): "lexical",
    ("黒服", "黒服"): "lexical",
    ("一歩先", "一歩先"): "lexical",
    ("発売所", "発売所"): "lexical",
    ("おでん屋", "おでん屋"): "lexical",
    ("伯父さん", "伯父さん"): "lexical",
    ("お前たち", "お前たち"): "lexical",
    ("年ぶり", "年ぶり"): "lexical",
    ("山ほど", "山ほど"): "lexical",
    ("誰か", "誰か"): "lexical",
    ("誰でも", "誰でも"): "lexical",
    ("何も", "何も"): "lexical",
    ("何時も", "何時も"): "lexical",
    ("先に", "先に"): "lexical",
    ("自分で", "自分で"): "lexical",
    ("静かに", "静かに"): "lexical",
    ("この頃", "この頃"): "lexical",
    ("その頃", "その頃"): "lexical",
    # --- numeral + counter ----------------------------------------------
    ("1月", "１月"): "lexical",
    ("2月", "２月"): "lexical",
    ("4月", "４月"): "lexical",
    ("9月", "９月"): "lexical",
    ("10月", "１０月"): "lexical",
    ("9日", "９日"): "lexical",
    ("16日", "１６日"): "lexical",
    ("18日", "１８日"): "lexical",
    ("31日", "３１日"): "lexical",
    ("五日", "五日"): "lexical",
    ("1点", "１点"): "lexical",
    ("2回", "２回"): "lexical",
    ("8つ", "８つ"): "lexical",
    # --- fixed grammatical constructions: own force, not に+は stacking --
    ("である", "である"): "lexical",
    ("であった", "である"): "lexical",
    ("として", "として"): "lexical",
    ("とする", "とする"): "lexical",
    ("となり", "となる"): "lexical",
    ("となった", "となる"): "lexical",
    ("による", "による"): "lexical",
    ("によると", "によると"): "lexical",
    ("に比べ", "に比べ"): "lexical",
    ("のうち", "のうち"): "lexical",
    ("ことになって", "ことになる"): "lexical",
    ("ことになっている", "ことになっている"): "lexical",
    ("じゃない", "じゃない"): "lexical",
    ("でもない", "でもない"): "lexical",
    ("なきゃいけない", "なきゃいけない"): "lexical",
    ("といっても", "といっても"): "lexical",
    ("ように", "ように"): "lexical",
    ("のように", "のように"): "lexical",
    ("ようだ", "ようだ"): "lexical",
    ("そうです", "そうです"): "lexical",
    ("そうか", "そうか"): "lexical",
    ("どうか", "どうか"): "lexical",
    ("かなあ", "かなあ"): "lexical",
    ("せいか", "せいか"): "lexical",
    ("なんだと", "なんだと"): "lexical",
    ("これにより", "これにより"): "lexical",
    ("これまでに", "これまでに"): "lexical",
    # --- transparent function-word stacking -----------------------------
    ("には", "には"): "transparent",
    ("にも", "にも"): "transparent",
    ("での", "での"): "transparent",
    ("との", "との"): "transparent",
    ("のか", "のか"): "transparent",
    ("かよ", "かよ"): "transparent",
    ("がいい", "がいい"): "transparent",
    ("いいから", "いいから"): "transparent",
    ("ときの", "ときの"): "transparent",
    ("ことに", "ことに"): "transparent",
    ("をして", "をして"): "transparent",
    ("んじゃ", "んじゃ"): "transparent",
    ("こうで", "こう"): "transparent",
    ("どうに", "どうに"): "transparent",
    ("らが", "らが"): "transparent",
    ("彼の", "彼の"): "transparent",
    ("それは", "それは"): "transparent",
    ("それを", "それを"): "transparent",
    ("そこに", "そこに"): "transparent",
    ("そこから", "そこから"): "transparent",
    ("側から", "側から"): "transparent",
    ("今も", "今も"): "transparent",
    # --- false matches: the headword is a different word ----------------
    ("がした", "がす"): "false-match",
    ("がする", "ガスる"): "false-match",
    ("いたが", "いたがる"): "false-match",
    ("いたから", "いたい"): "false-match",
    ("あると", "アルト"): "false-match",
    ("とが", "トガ"): "false-match",
    ("うえは", "ウエハ"): "false-match",
    ("ないん", "ナイン"): "false-match",
    ("のバス", "のばす"): "false-match",
    ("はいた", "はいた"): "false-match",
    ("はこう", "はく"): "false-match",
    ("はそう", "はそう"): "false-match",
    ("はどう", "はどう"): "false-match",
    ("もうどう", "もうどう"): "false-match",
    ("もうから", "もうかる"): "false-match",
    ("になって", "になう"): "false-match",
    ("になった", "になう"): "false-match",
    ("名の", "名のる"): "false-match",
    ("煙が", "煙がる"): "false-match",
    ("といき", "といき"): "false-match",
    ("つの", "つの"): "false-match",
    ("タッと", "たっとい"): "false-match",
    ("月9", "月９"): "false-match",
    # --- genuinely context-dependent at type level ----------------------
    ("では", "では"): "ambiguous",
    ("でも", "でも"): "ambiguous",
    ("にして", "にして"): "ambiguous",
    ("時に", "時に"): "ambiguous",
    ("天と地", "天と地"): "ambiguous",
}

# どうか and かなあ overlap on か in 「それはどうかなあ」 and both are real
# units: どう+か+なあ and どうか+なあ are both defensible readings of that
# line, so neither is adjudicated. Reclassified from lexical after the
# nesting pass surfaced the collision.
V[("どうか", "どうか")] = "ambiguous"
V[("かなあ", "かなあ")] = "ambiguous"

# Overlaps the longest-wins rule resolves the wrong way. Keyed by
# (source, surface): the window that must lose its merge expectation even
# though it is the longer of the pair.
#
# 「立派な黒服を着て」-- 服を着る is a real expression, but here it straddles
# the noun 黒服, and 黒服 | を | 着て is the reading. The longer window is not
# the unit.
DEMOTE = {("a:000883-2", "服を着て")}

def build(rows):
    """One record per window, with overlaps resolved per sentence."""
    missing = sorted({(r["surface"], r["headword"]) for r in rows} - set(V))
    if missing:
        raise SystemExit(
            "no verdict for %d type(s): %s"
            % (len(missing), ", ".join(f"{s}/{h}" for s, h in missing))
        )

    by_sentence = collections.defaultdict(list)
    for r in rows:
        by_sentence[r["text"]].append(r)

    out = []
    for text, windows in by_sentence.items():
        lexical = [
            w
            for w in windows
            if V[(w["surface"], w["headword"])] == "lexical"
            and (w["source"], w["surface"]) not in DEMOTE
        ]
        # Longest first; a window overlapping an already-kept one cannot also
        # be a segment.
        winners = []
        for w in sorted(lexical, key=lambda w: (-w["len"], w["start"])):
            a, b = w["start"], w["start"] + w["len"]
            if any(
                not (b <= x["start"] or x["start"] + x["len"] <= a) for x in winners
            ):
                continue
            winners.append(w)
        kept = {(w["start"], w["len"]) for w in winners}

        for w in sorted(windows, key=lambda w: (w["start"], w["len"])):
            cls = V[(w["surface"], w["headword"])]
            rec = {
                "text": text,
                "source": w["source"],
                "start": w["start"],
                "len": w["len"],
                "surface": w["surface"],
                "headword": w["headword"],
                "entry_id": w["entry_id"],
                "class": cls,
            }
            if cls == "ambiguous":
                rec["expect"] = "skip"
            elif cls == "lexical" and (w["start"], w["len"]) in kept:
                rec["expect"] = "merge"
            elif cls == "lexical":
                rec["expect"] = "split"
                over = [
                    x
                    for x in winners
                    if not (
                        w["start"] + w["len"] <= x["start"]
                        or x["start"] + x["len"] <= w["start"]
                    )
                ]
                rec["note"] = (
                    "a lexical unit, but %s is the segment here" % over[0]["surface"]
                )
            else:
                rec["expect"] = "split"
            out.append(rec)
    return out


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__.strip().splitlines()[-1])
    rows = [
        json.loads(line)
        for line in io.open(sys.argv[1], encoding="utf-8")
        if line.strip()
    ]
    out = build(rows)
    with io.open(sys.argv[2], "w", encoding="utf-8", newline="\n") as f:
        for rec in out:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    counts = collections.Counter(r["expect"] for r in out)
    print(
        "%d windows: %d merge, %d split, %d skip"
        % (len(out), counts["merge"], counts["split"], counts["skip"])
    )


if __name__ == "__main__":
    main()
