# Test fixtures

## `jmdict_mini.xml`

Hand-written, three entries, no external source. Used by
`tests/index_roundtrip.rs`, `tests/cli_parse.rs`, and `record.rs`'s own tests.

## `jmdict_matcher.xml`

Hand-written, eight entries, no external source. Used by `src/matcher.rs`'s own
test module. Covers a hiragana/katakana homophone pair, a particle, two
homographs, a nested-prefix pair, a `v1` verb, and a `vs-i` verb whose stem is
the empty string.

## `jmdict_subset.xml`

A curated subset of **JMdict**, containing only the entries the sentences in
`parse_sentences.txt` can reach. Used by `tests/parse_snapshots.rs`.

- **Source:** Electronic Dictionary Research and Development Group (EDRDG),
  <http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz>
- **Retrieved:** 2026-08-13 (JMdict is rebuilt daily, so the committed
  subset — not the download — is the reproducible artifact)
- **Licence:** Creative Commons Attribution-ShareAlike 4.0 International
  (CC BY-SA 4.0), <https://www.edrdg.org/edrdg/licence.html>
- **Notice:** This file contains material from JMdict, Copyright (C) EDRDG.

This is third-party **data**, not source: it carries the EDRDG notice in its own
XML header instead of the crate's GPL v2 header, and it is not relicensed. The
crate is `GPL-2.0-only` and CC BY-SA 4.0 is one-way compatible with GPL v3, not
v2; the subset is kept as a separately-licensed data asset for that reason.

Regenerate with:

```bash
python3 tools/extract_jmdict_subset.py \
  /path/to/JMdict_e.xml \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  crates/jparser/assets/conjugations.json \
  crates/jparser/tests/fixtures/jmdict_subset.xml
```

One entry is hand-added rather than extractor-derived: `食べる` (ent_seq
1358280, `v1`, gloss "to eat"), appended just before `</JMdict>`. It backs
`tests/engine_v2.rs`'s `chain.rs` tests, which need an ichidan verb whose
chain includes a zero-consumption 連用形 terminal, and it has no sentence in
`parse_sentences.txt` behind it. Regenerating this fixture from a real
`JMdict_e.xml` will drop it — re-add it by hand afterward, or add a sentence
using 食べる to `parse_sentences.txt` first.

## `parse_sentences.txt`

The snapshot corpus. Adding a sentence requires re-running the extractor against
a real `JMdict_e.xml` — the committed subset will not contain the new
vocabulary, and the snapshot will show unmatched spans instead of failing
loudly.

## `conjugation_labels.txt`

Hand-written, no external data source. Phase 1b-i's class × chain gate:
one row per deinflection chain the v2 table generates, with the English
label `engine/label.rs` must produce for it. Used by
`tests/conjugation_labels.rs` (every row through the real trie, exactly one
matching chain, label equal) and by `engine/table.rs`'s coverage test (every
depth-0 and depth-1 tuple reachable from a `JMDICT_POS_CLASS`-mapped class
has a row).

The English column was written from *Japanese verb conjugation*, English
Wikipedia, terminology only, **before** `label.rs` existed, and is
authoritative over it: a row the formatter cannot pass is a finding about
the formatter or the design, never a row to edit toward the code. Rows
marked `table finding` are surfaces the table generates that are not
natural Japanese; their cause is an `ATTACHES_TO` row and is recorded in
`docs/superpowers/2026-09-02-engine-v2-phase1b-i-handoff.md` §4.

Class and form names are `engine/table_data.rs`'s IPADIC names; regenerating
that table can rename or add forms, and the coverage test will then name
every tuple that lost its row.
