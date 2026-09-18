#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Written independently for the Engine v2 measurement harness. Nothing here
# derives from Translation Aggregator.
"""Cut disjoint sampling pools out of LunaTranslator capture caches.

Usage:
    python3 tools/cut_corpus_pools.py OUT_DIR DB [DB ...]

Sibling of `extract_vn_pool.py`, which does a *different* job: that one filters
by length, splits the speaker name into its own column, and emits the TSV
`jparser-cli diff` reads. This one keeps every distinct line unfiltered, which
is the recipe the corpus-disagreement probe and the `span_dump` A/B runs use —
it is validated by reproducing the renai corpus at exactly 8,334 rows.

**Why four pools and not one.** Every corpus-level number in the
three-corpus adjudication handoff was measured on `head -6000`, and because the
dump is distinct rows in game-script order that is the *opening chapters* of
three visual novels. A `head` pool cannot tell "this is Japanese" from "this is
how this game opens". So each corpus yields `head-N`, `tail-N` and a seeded
`random-N`, and the three are compared against each other: agreement across all
three is what makes a rate a rate. `head-6000` is cut as well and used only as a
reproduction control against the older run.

**Why N defaults to 4,000.** The pools must be disjoint, so the binding
constraint is `2N <= the smallest corpus`. renai has 8,334 distinct rows, which
6,000 violates — a head-6000 and a tail-6000 would share 3,666 sentences.

Table `artificialtrans`, column `source`. Never `origin`, whose every character
is doubled, and never `machineTrans`, which is a translation and is never a
segmentation oracle. Distinct rows in rowid order, first occurrence winning, so
the pools stay in game-script order.

The captured text is copyrighted. Pools are NEVER committed — `vn_data/` is in
.gitignore and the output directory belongs outside the repository. The only
committed product is which register classes a measurement surfaced. Register
and style are not copyrightable; transcribed lines are.
"""

import argparse
import random
import sqlite3
import sys
from pathlib import Path

# Changing this re-cuts every `random` pool, so a run that quotes a number must
# quote the seed beside it. Recorded in the handoff rather than left implicit.
DEFAULT_SEED = 20260918


def distinct_lines(db_path):
    """Every distinct non-empty `source` value, newlines stripped, in row order.

    Deduplication keeps the first occurrence so the result stays in game-script
    order — `SELECT DISTINCT` does not promise an order, and the head/tail split
    is meaningless without one.
    """
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        seen, out = set(), []
        for (text,) in conn.execute(
            "select source from artificialtrans order by rowid"
        ):
            if not text:
                continue
            text = text.replace("\r", "").replace("\n", "").strip()
            if not text or text in seen:
                continue
            seen.add(text)
            out.append(text)
        return out
    finally:
        conn.close()


def pools(rows, n, seed, control):
    """The three disjoint slices, plus the reproduction control when asked.

    `random.sample` rather than a shuffle: it is the same draw for a given seed
    regardless of how many rows follow the last one taken, so adding a corpus to
    a run does not move an existing corpus's pool.
    """
    cut = {"head": rows[:n], "tail": rows[-n:], "random": random.Random(seed).sample(rows, n)}
    out = {f"{name}-{n}": pool for name, pool in cut.items()}
    if control and len(rows) >= control:
        out[f"head-{control}"] = rows[:control]
    return out


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("out_dir", type=Path)
    ap.add_argument("dbs", type=Path, nargs="+")
    ap.add_argument("-n", type=int, default=4000, help="pool size (default 4000)")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument(
        "--control",
        type=int,
        default=6000,
        help="also cut head-CONTROL where the corpus is large enough; 0 to skip",
    )
    args = ap.parse_args(argv)

    args.out_dir.mkdir(parents=True, exist_ok=True)
    for db in args.dbs:
        rows = distinct_lines(db)
        if len(rows) < 2 * args.n:
            print(
                f"{db.name}: {len(rows)} distinct rows, fewer than 2 x {args.n} — "
                f"head and tail would overlap",
                file=sys.stderr,
            )
            return 1
        name = db.name.split("_")[0]
        cut = pools(rows, args.n, args.seed, args.control)
        for pool_name, lines in cut.items():
            # Built by concatenation, not `with_suffix`: a pool name contains a
            # dot (`head-4000` follows one), so `with_suffix` would treat
            # `.head-4000` as the extension and write every pool to one file.
            stem = f"{name}.{pool_name}"
            (args.out_dir / f"{stem}.txt").write_text(
                "\n".join(lines) + "\n", encoding="utf-8"
            )
            # `span_dump` wants `source<TAB>text`; the id only has to be unique
            # so that two dumps of one pool join row for row.
            (args.out_dir / f"{stem}.tsv").write_text(
                "".join(f"{stem}-{i}\t{l}\n" for i, l in enumerate(lines, 1)),
                encoding="utf-8",
            )
        avg = sum(len(r) for r in rows) / len(rows)
        print(f"{name:10s} distinct={len(rows):6d} avg={avg:5.1f} pools={sorted(cut)}")
    print(f"seed={args.seed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
