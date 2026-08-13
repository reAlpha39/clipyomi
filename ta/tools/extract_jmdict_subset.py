#!/usr/bin/env python3
# JParser — Japanese text parser ported from Translation Aggregator.
# Copyright (C) 2026
#
# This program is free software; you can redistribute it and/or modify it
# under the terms of the GNU General Public License version 2 as published
# by the Free Software Foundation.
"""Extract the JMdict subset the parse-snapshot sentences can reach.

Usage:
    python3 tools/extract_jmdict_subset.py \\
        /tmp/jmdict/JMdict_e.xml \\
        crates/jparser/tests/fixtures/parse_sentences.txt \\
        crates/jparser/assets/conjugations.json \\
        crates/jparser/tests/fixtures/jmdict_subset.xml

Entries are copied verbatim by text slicing, never re-serialized: JMdict's
parts of speech are entity references and a DTD-aware parser would expand
them into prose, which is precisely the data jparser reads as POS codes.
"""

import json
import re
import sys
from pathlib import Path

ENTRY_RE = re.compile(r"<entry>.*?</entry>", re.DOTALL)
SURFACE_RE = re.compile(r"<(?:keb|reb)>(.*?)</(?:keb|reb)>", re.DOTALL)
# Exactly the POS codes record::headwords can map to a conjugation type: every
# table name is v*, adj-*, or copula, and copula is not a JMdict code.
CONJUGABLE_POS_RE = re.compile(r"<pos>&(?:v|adj-)")

# Mirrors kana::unify (crates/jparser/src/kana.rs). Character-wise, so folding
# a prefix equals the prefix of a folded string — which is what lets this work
# on substrings at all.
HIRAGANA_START, HIRAGANA_END = 0x3041, 0x3096      # end exclusive
HIRAGANA_TO_KATAKANA = 0x60
FULLWIDTH_START, FULLWIDTH_END = 0xFF01, 0xFF20    # end exclusive
FULLWIDTH_TO_ASCII = 0xFEE0

# The tense a type strips to make a stem: its own "Remove" entry if it declares
# one, otherwise "Non-past" (conjugation.rs / contract §1.2).
REMOVE_TENSE = "Remove"
DEFAULT_REMOVE_TENSE = "Non-past"

NOTICE = """<!-- Curated subset of JMdict_e.xml, derived for jparser's parse snapshots.

     Source:  Electronic Dictionary Research and Development Group (EDRDG),
              http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz
     Licence: Creative Commons Attribution-ShareAlike 4.0 International
              (CC BY-SA 4.0), https://www.edrdg.org/edrdg/licence.html
     Notice:  This file contains material from JMdict, Copyright (C) EDRDG.

     Only entries reachable from tests/fixtures/parse_sentences.txt are kept.
     Every kept entry is byte-identical to its JMdict original; nothing is
     rewritten. Regenerate with tools/extract_jmdict_subset.py.
     See tests/fixtures/README.md for the retrieval date. -->
"""


def unify(ch):
    x = ord(ch)
    if HIRAGANA_START <= x < HIRAGANA_END:
        x += HIRAGANA_TO_KATAKANA
    elif FULLWIDTH_START <= x < FULLWIDTH_END:
        x -= FULLWIDTH_TO_ASCII
    c = chr(x)
    return c.upper() if c.isascii() else c


def unify_str(s):
    return "".join(unify(c) for c in s)


def remove_suffixes(asset_path):
    """Every remove-tense/form-0 suffix in the conjugation asset, unified."""
    out = set()
    for ty in json.loads(Path(asset_path).read_text(encoding="utf-8")):
        tenses = ty["Tenses"]
        names = {t["Tense"] for t in tenses}
        remove = REMOVE_TENSE if REMOVE_TENSE in names else DEFAULT_REMOVE_TENSE
        for t in tenses:
            if (
                t["Tense"] == remove
                and not t.get("Formal", False)
                and not t.get("Negative", False)
            ):
                out.add(unify_str(t["Suffix"]))
    return {s for s in out if s}


def wanted_keys(sentences):
    """Every unified substring of the corpus, plus the empty key.

    The empty key is always wanted: する and 来る strip to nothing, so their
    stem matches at every position and they must never be filtered out.
    """
    keys = {""}
    for sentence in sentences:
        u = unify_str(sentence)
        for i in range(len(u)):
            for j in range(i + 1, len(u) + 1):
                keys.add(u[i:j])
    return keys


def is_needed(block, wanted, suffixes):
    surfaces = SURFACE_RE.findall(block)
    for surface in surfaces:
        if unify_str(surface) in wanted:
            return True
    # The stem rule applies only where record::headwords would attach a verb
    # type. Gating on it drops no reachable stem and keeps the subset small.
    if not CONJUGABLE_POS_RE.search(block):
        return False
    for surface in surfaces:
        u = unify_str(surface)
        for suffix in suffixes:
            if u.endswith(suffix) and u[: len(u) - len(suffix)] in wanted:
                return True
    return False


def main(argv):
    if len(argv) != 5:
        print(__doc__, file=sys.stderr)
        return 2
    jmdict_path, sentences_path, asset_path, out_path = argv[1:]

    sentences = [
        line.strip()
        for line in Path(sentences_path).read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    if not sentences:
        print("no sentences: check the corpus path", file=sys.stderr)
        return 1

    wanted = wanted_keys(sentences)
    suffixes = remove_suffixes(asset_path)
    text = Path(jmdict_path).read_text(encoding="utf-8")
    first = text.index("<entry>")

    prolog = text[:first].replace("<JMdict>", NOTICE + "<JMdict>", 1)
    kept = [
        m.group(0)
        for m in ENTRY_RE.finditer(text, first)
        if is_needed(m.group(0), wanted, suffixes)
    ]

    out = Path(out_path)
    out.write_text(prolog + "\n".join(kept) + "\n</JMdict>\n", encoding="utf-8")
    print(f"sentences:     {len(sentences)}")
    print(f"wanted keys:   {len(wanted)}")
    print(f"stem suffixes: {len(suffixes)}")
    print(f"kept entries:  {len(kept)}")
    print(f"output bytes:  {out.stat().st_size}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
