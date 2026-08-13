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

## `parse_sentences.txt`

The snapshot corpus. Adding a sentence requires re-running the extractor against
a real `JMdict_e.xml` — the committed subset will not contain the new
vocabulary, and the snapshot will show unmatched spans instead of failing
loudly.
