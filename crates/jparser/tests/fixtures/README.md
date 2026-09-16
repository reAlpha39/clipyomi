# Test fixtures

## `jmdict_mini.xml`

Hand-written, three entries, no external source. Used by
`tests/index_roundtrip.rs`, `tests/cli_parse.rs`, `tests/cli_label.rs`,
`tests/cli_score.rs`, `tests/entry_id.rs`, and `src/jmdict.rs`'s own tests.

## `jmdict_matcher.xml`

Hand-written, eight entries, no external source. Covers a hiragana/katakana
homophone pair, a particle, two homographs, a nested-prefix pair, a `v1` verb,
and a `vs-i` verb whose stem is the empty string.

**Currently unused.** Its only consumer was `src/matcher.rs`'s own test
module, deleted with the v1 engine at Phase 1b-ii. It is kept rather than
deleted because the eight shapes it covers are still the ones a lexical-lookup
test wants and nothing else in the tree covers them together; whether to adopt
it or drop it is a decision for whoever next touches the lookup layer.

## `jmdict_subset.xml`

A curated subset of **JMdict**, containing only the entries the sentences in
`parse_sentences.txt` can reach. Used by `tests/engine_v2.rs`.

- **Source:** Electronic Dictionary Research and Development Group (EDRDG),
  <http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz>
- **Retrieved:** 2026-08-13 (JMdict is rebuilt daily, so the committed
  subset — not the download — is the reproducible artifact)
- **Licence:** Creative Commons Attribution-ShareAlike 4.0 International
  (CC BY-SA 4.0), <https://www.edrdg.org/edrdg/licence.html>
- **Notice:** This file contains material from JMdict, Copyright (C) EDRDG.

This is third-party **data**, not source: it carries the EDRDG notice in its own
XML header instead of the crate's SPDX header, and it is not relicensed. The
crate is `MIT OR Apache-2.0`, and CC BY-SA 4.0 is a share-alike licence that
offers no option to take either of those; the subset is kept as a
separately-licensed data asset for that reason.

Regenerate with:

```bash
python3 tools/extract_jmdict_subset.py \
  /path/to/JMdict_e.xml \
  crates/jparser/tests/fixtures/parse_sentences.txt \
  <conjugation source> \
  crates/jparser/tests/fixtures/jmdict_subset.xml
```

**The extractor does not run as written today.** Its third argument was
`crates/jparser/assets/conjugations.json`, deleted at Phase 1b-ii, and it
needs a replacement built from `crates/jparser/src/engine/table.rs` — see the
note in `tools/extract_jmdict_subset.py`'s docstring. Until that port lands,
recover the old asset from git history if a regeneration is genuinely needed.
The committed subsets are the reproducible artifacts, so no regeneration is
pending.

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

## `compound_gold.jsonl`

The adjudicated compound corpus behind `score --compounds` and
`src/bin/jparser-cli/compounds.rs`. One record per window where the engine
faced a compound decision on `gold.jsonl` — 194 of them, being the 53 merges
it made plus the 141 compounds it declined, enumerated by
`examples/common_inversion_probe.rs`. Each carries a `class` (the type-level
judgment) and an `expect` (what that sentence permits after overlapping
windows are resolved).

It exists because neither span metric can register compound preservation as
a gain: the gold corpus is adjudicated at short-unit granularity, so keeping
`音がする` whole costs strict accuracy and earns tolerant nothing. See
`docs/superpowers/2026-09-09-frequency-signal-candidates.md` §10–§11 for the
measurement that forced it and `compounds.rs` for the four classes.

It carries JMdict content in its `headword` and `entry_id` columns — 139
dictionary forms and the `ent_seq` each came from — so the note on
`jmdict_subset.xml` above applies: **this file contains material from JMdict,
Copyright (C) EDRDG, CC BY-SA 4.0**, <https://www.edrdg.org/edrdg/licence.html>.
The `text` and `surface` columns come from `gold.jsonl`, whose own provenance
is in `gold_sources.md`. JSONL carries no comment syntax, which is why the
notice lives here rather than in the file.

**This is a model's adjudication, not the author's.** `corpus_staging/build_gold.py`
records that distinction for the gold corpus and it applies here too: every
verdict is revisable, and because the class is stored per record, reversing a
whole class's judgment is a filter over this file rather than a re-read of the
corpus. The judgment most likely to be disputed is the `transparent` class —
`には`, `にも`, `での`, `との` are JMdict headwords adjudicated *split* here,
on the ground that merging them hides the particle a reader needs; the engine
merges `には` and `にも` today, so that call alone accounts for 12 of the
metric's misses.

Regenerate with:

```bash
cargo run --release -p jparser --features mecab --example common_inversion_probe --   <gold-index> crates/jparser/tests/fixtures/gold.jsonl <system.dic> /tmp/decisions.jsonl
python3 tools/adjudicate_compounds.py /tmp/decisions.jsonl   crates/jparser/tests/fixtures/compound_gold.jsonl
```

The adjudication script hard-fails on any `(surface, headword)` type it has no
verdict for, so a corpus change that introduces a new compound cannot silently
drop out of the metric.

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

## `kanjidic_mini.xml`

Four characters (東, 京, 都, 食) in **KANJIDIC2**'s published structure,
trimmed to the elements the reader keeps or must skip: a foreign-language
`<meaning m_lang>`, `<nanori>`, non-Japanese `r_type`s, a character with five
English meanings (the cap is four), and kun readings carrying the `.` and `-`
marks. The `<misc>` blocks cover the four cases Phase 3b's parser handles:
東 carries all four numbers; 京 has no `<misc>` at all; 都 has its real block,
with two `<stroke_count>`s (the accepted 11, then the miscount 10) and a
`<variant>` to ignore; 食 is trimmed to grade and stroke count only. Used by
`src/kanjidic.rs`'s own tests, `tests/index_roundtrip.rs`, and `src-tauri`'s
command tests.

- **Source:** Electronic Dictionary Research and Development Group (EDRDG),
  <http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz>
- **Retrieved:** 2026-09-11 (KANJIDIC2 is regenerated continuously, like
  JMdict, so the committed excerpt is the reproducible artifact)
- **Licence:** Creative Commons Attribution-ShareAlike 4.0 International
  (CC BY-SA 4.0), <https://www.edrdg.org/edrdg/licence.html>
- **Notice:** This file contains material from KANJIDIC2, Copyright (C) EDRDG.

Third-party **data**, not source, on the same footing as `jmdict_subset.xml`
above: it carries the EDRDG notice in its own header and is not relicensed.
