#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Written independently for the Engine v2 measurement harness. Nothing here
# derives from Translation Aggregator.
"""Extract a sampling pool from LunaTranslator capture caches.

Usage:
    python3 tools/extract_vn_pool.py vn_data vn_data/pool.tsv

Reads every *.sqlite in the input directory. The schema is
`artificialtrans(id, source, machineTrans, origin)`; only `source` is read —
`origin` stores each line doubled and is unusable.

Output is `source_id<TAB>text<TAB>speaker` TSV, which is what
`jparser-cli diff` reads. The speaker name is separated out rather than
dropped: a captured line often carries one, and it belongs in the noise tier
rather than in every dialogue line's parse.

The captured text is copyrighted. The pool is NEVER committed --- `vn_data/`
is in .gitignore --- and its only committed product is which register classes
the disagreement sampler surfaced. Register and style are not copyrightable;
transcribed lines are.
"""

import re
import sqlite3
import sys
from pathlib import Path

# A speaker prefix: a name, then an opening quote. Captures split on the quote
# so the quote itself stays with the text, where the parser will see it.
SPEAKER_RE = re.compile(r"^([^「『（\s]{1,12})(?=[「『])")

# Below this a line carries no segmentation signal; above it, a capture has
# almost always concatenated several lines and the sentence is not a sentence.
MIN_CHARS = 6
MAX_CHARS = 120


def lines(db_path):
    """Every non-empty `source` value in one cache, in row order."""
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        for (text,) in conn.execute(
            "select source from artificialtrans where source is not null"
        ):
            yield text
    finally:
        conn.close()


def clean(text):
    """One captured line to `(text, speaker)`, or None to drop it.

    CRLF and the ideographic space U+3000 are normalised out of the *line
    structure* only: a capture that spans two script lines is not one sentence,
    and an interior U+3000 is an indent artefact rather than a word boundary.
    Both survive elsewhere as deliberate noise-tier artefacts (design §3.1),
    which is why the noise tier is hand-derived rather than sampled from here.
    """
    text = text.replace("\r", "").replace("\n", "").strip()
    text = text.strip("　")
    if not text:
        return None
    speaker = ""
    m = SPEAKER_RE.match(text)
    if m:
        speaker = m.group(1)
        text = text[len(speaker):]
    if not (MIN_CHARS <= len(text) <= MAX_CHARS):
        return None
    if "\t" in text:
        return None
    return text, speaker


def main(argv):
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2
    in_dir, out_path = Path(argv[1]), Path(argv[2])

    dbs = sorted(in_dir.glob("*.sqlite"))
    if not dbs:
        print(f"no *.sqlite in {in_dir}", file=sys.stderr)
        return 1

    seen = {}
    read = 0
    for db in dbs:
        stem = db.name.split("_")[0]
        for i, raw in enumerate(lines(db)):
            read += 1
            cleaned = clean(raw)
            if cleaned is None:
                continue
            text, speaker = cleaned
            # First occurrence wins, so the source id is stable across runs.
            seen.setdefault(text, (f"c2:{stem}-{i}", speaker))

    with out_path.open("w", encoding="utf-8", newline="\n") as out:
        out.write("# Sampling pool from vn_data. NEVER COMMIT THIS FILE.\n")
        out.write("# source<TAB>text<TAB>speaker\n")
        for text, (source, speaker) in seen.items():
            out.write(f"{source}\t{text}\t{speaker}\n")

    print(f"caches:  {len(dbs)}")
    print(f"read:    {read}")
    print(f"kept:    {len(seen)}")
    print(f"dropped: {read - len(seen)}")
    return 0


def demo():
    """Self-check for `clean`, the only branching logic here."""
    assert clean("テスト太郎「これはためしの文です」") == (
        "「これはためしの文です」",
        "テスト太郎",
    )
    assert clean("「ここにもためしの文があります」") == ("「ここにもためしの文があります」", "")
    assert clean("短い") is None, "under MIN_CHARS"
    assert clean("あ" * 200) is None, "over MAX_CHARS"
    assert clean("  　 ") is None, "whitespace only"
    assert clean("ここに\tタブが入った文です") is None, "a tab would break the TSV"
    assert clean("前半です。\r\n後半です。") == ("前半です。後半です。", "")
    print("demo: ok")


if __name__ == "__main__":
    if sys.argv[1:2] == ["--demo"]:
        demo()
    else:
        sys.exit(main(sys.argv))
