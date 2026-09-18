#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Written independently for the Engine v2 measurement harness. Nothing here
# derives from Translation Aggregator.
"""Compare two `span_dump` runs over one pool, by what happened to the words.

Usage:
    python3 tools/classify_spans.py BASE.jsonl VARIANT.jsonl [--examples N]

**Why this exists, and what it replaces.** The obvious comparison is to classify
each new span by whether both its endpoints were boundaries in the other build's
parse — call them `merge` and `cut`. That measures agreement with the *previous
engine*, not with truth, and it was wrong in both directions when the
`LEN_GAIN` question was priced on it:

  * A correct re-tiling files as damage. `中|出|しする` -> `中出し|する` ends
    inside `しする`, so it scored as an invented boundary. So did
    `絵|に|描いた|よ|うな` -> `絵に描いたよう|な`.
  * A bad merge of an already-mis-cut region files as coarsening.
    `なんでも|な|いよ` -> `なんでもな|いよ` has both ends on base boundaries.
  * One severing is counted twice, as the truncated word and again as the
    leftover: `抱え|ながら` -> `抱えな|がら` yields two "invented" spans and no
    merge.

Measured over 16,000 real sentences, that bucket held **seven** genuine
non-word merges out of 27,475 spans; the rest were recovered words (42.7%) and
double-counted severings (57.3%).

So this asks the two questions that do separate them, using each build's own
`matched` flag — whether a span is a dictionary entry or a bare fragment:

  severed     a span the BASE matched as a word that the variant cut through.
              Destroyed morphology: `抱え|ながら` -> `抱えな|がら`.
  gained      a span the VARIANT matches as a word, absent from the base, that
              covers at least one base boundary. A word recovered.
  entangled   a `gained` span that itself severs a base word down to <=2
              characters. Counted apart because such a span is a severing
              wearing a gain's clothes — `浮かべな` is a dictionary match and is
              the `ながら` defect.
  fragment    a span covering a base boundary that matches nothing at all.

Report `clean gains : severed` as the conservative ratio and say so; the figure
including `entangled` is the optimistic one.

Neither build is truth. `severed` and `gained` are claims about the dictionary,
not about Japanese, so a run that is going to decide whether to ship still owes
fifteen changed sentences read side by side — the counts alone are what
overstated the `LEN_GAIN` case the first time it was priced.
"""

import argparse
import json
from pathlib import Path

# A base span shorter than this carries too little to call its loss a severing:
# a one-character match is as often an artefact of the tiling as a word.
MIN_WORD = 2

# The residue that makes a cut suspicious rather than merely different. The same
# threshold `relax.MIN_PIECE` uses on the split side: 91.5% of the words the
# ungated `LEN_GAIN` severs lose exactly one character, 99.6% lose two or fewer.
MAX_RESIDUE = 2


def rows(path):
    with open(path, encoding="utf-8") as f:
        for line in f:
            o = json.loads(line)
            yield o["source"], o["text"], [(s[0], s[1], s[2]) for s in o["segs"]]


def compare(base_path, var_path, examples=0):
    counts = dict(sentences=0, changed=0, severed=0, gained=0, entangled=0, fragment=0)
    residue = {}
    shown = []
    for (sb, text, gb), (sv, _, gv) in zip(rows(base_path), rows(var_path)):
        if sb != sv:
            raise SystemExit(f"pools are not aligned: {sb!r} against {sv!r}")
        counts["sentences"] += 1
        if gb == gv:
            continue
        counts["changed"] += 1

        var_bounds = set()
        for s, l, _ in gv:
            var_bounds.add(s)
            var_bounds.add(s + l)
        words = [(s, l) for s, l, m in gb if m and l >= MIN_WORD]

        for s, l in words:
            inner = [b for b in var_bounds if s < b < s + l]
            if not inner:
                continue
            counts["severed"] += 1
            marks = [s, *inner, s + l]
            smallest = min(b - a for a, b in zip(marks, marks[1:]))
            residue[smallest] = residue.get(smallest, 0) + 1

        base_spans = {(s, l) for s, l, _ in gb}
        base_bounds = {s for s, _, _ in gb} | {s + l for s, l, _ in gb}
        for s, l, m in gv:
            if (s, l) in base_spans:
                continue
            if not any(s < b < s + l for b in base_bounds):
                # Spans no base boundary, so it is a leftover piece inside one
                # base span, not a merge of anything.
                continue
            if not m:
                counts["fragment"] += 1
            elif any(
                ws < edge < ws + wl and min(edge - ws, ws + wl - edge) <= MAX_RESIDUE
                for ws, wl in words
                for edge in (s, s + l)
            ):
                counts["entangled"] += 1
            else:
                counts["gained"] += 1
                if len(shown) < examples:
                    shown.append((text, text[s : s + l]))
    return counts, residue, shown


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("base", type=Path)
    ap.add_argument("variant", type=Path)
    ap.add_argument("--examples", type=int, default=0)
    args = ap.parse_args(argv)

    counts, residue, shown = compare(args.base, args.variant, args.examples)
    print(f"sentences {counts['sentences']}, changed {counts['changed']}")
    print(f"  clean gains {counts['gained']}")
    print(f"  entangled   {counts['entangled']}   (matched, but severs a word itself)")
    print(f"  severed     {counts['severed']}")
    print(f"  fragment    {counts['fragment']}   (merged across a boundary, matches nothing)")
    if counts["severed"]:
        print(f"  clean : severed = {counts['gained'] / counts['severed']:.2f}  (conservative)")
        optimistic = (counts["gained"] + counts["entangled"]) / counts["severed"]
        print(f"  all   : severed = {optimistic:.2f}  (optimistic)")
    if residue:
        total = sum(residue.values())
        print("  residue left of each severed word:")
        for k in sorted(residue):
            print(f"    {k} char: {residue[k]:6d}  {residue[k] / total * 100:5.1f}%")
    for text, span in shown:
        print(f"  gained {span!r} in {text[:60]}")
    return 0


def _selfcheck():
    """One runnable check: every class, hand-built, on two dumps.

    Each sentence exercises exactly one outcome:

    * `抱え|ながら` -> `抱えな|がら` severs `ながら` down to one character, and
      `抱えな` is a dictionary match, so it must land in `entangled` and not in
      `gained`. This is the case the old merge-vs-cut proxy counted as two
      invented spans and zero merges.
    * `子|宮|口` -> `子宮口` merges three one-character spans, none of which is
      long enough to be a word, so nothing is severed: a clean gain.
    * `甲|州` -> one unmatched span is a `fragment`, whatever its length.
    """
    import tempfile

    def dump(path, records):
        with open(path, "w", encoding="utf-8") as f:
            for src, text, segs in records:
                f.write(json.dumps({"source": src, "text": text, "segs": segs}) + "\n")

    with tempfile.TemporaryDirectory() as d:
        d = Path(d)
        base, var = d / "base.jsonl", d / "var.jsonl"
        dump(
            base,
            [
                ("a", "抱えながら", [[0, 2, True], [2, 3, True]]),
                ("b", "子宮口", [[0, 1, True], [1, 1, True], [2, 1, True]]),
                ("c", "甲州", [[0, 1, True], [1, 1, True]]),
            ],
        )
        dump(
            var,
            [
                ("a", "抱えながら", [[0, 3, True], [3, 2, True]]),
                ("b", "子宮口", [[0, 3, True]]),
                ("c", "甲州", [[0, 2, False]]),
            ],
        )
        counts, residue, _ = compare(base, var)

    assert counts["changed"] == 3, counts
    assert counts["severed"] == 1, counts
    assert residue == {1: 1}, residue
    assert counts["entangled"] == 1, counts
    assert counts["gained"] == 1, counts
    assert counts["fragment"] == 1, counts
    print("selfcheck ok")


if __name__ == "__main__":
    import sys

    if "--selfcheck" in sys.argv:
        _selfcheck()
    else:
        raise SystemExit(main())
