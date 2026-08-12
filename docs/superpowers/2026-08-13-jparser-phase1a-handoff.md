# JParser Phase 1A — Handoff

Phase 1A is complete: parser foundations plus a memory-mapped FST dictionary
index, verifiable through `jparser-cli`. 113 tests, 93.85% line coverage,
`cargo clippy -- -D warnings` clean.

This document records what Phase 1B needs and what Phase 1A deliberately left
undone. It exists because these decisions are not recoverable from the diff.

**Plan:** `docs/superpowers/plans/2026-08-12-jparser-phase1a-index.md`.
Note the plan is a historical artifact and is now contradicted by the code in
two places — it specifies `IS_NAME = 0x0040` and `INDEX_FORMAT_VERSION = 1`,
both of which changed for reasons recorded below. Trust the code.

---

## Decisions that departed from the plan

**`v5` fallback tests were re-paired (plan defect).** The plan's tests asserted
`v5_fallback_stems > 0` using `言う` tagged `v5u` — a *correctly* annotated verb,
for which the counter is structurally pinned at zero. The nine length-3 `v5*`
types have mutually exclusive single-kana remove suffixes (`v5b` ぶ, `v5g` ぐ,
`v5k` く, `v5m` む, `v5n` ぬ, `v5r` る, `v5s` す, `v5t` つ, `v5u` う), so a
surface ending in う can only ever match `v5u`. The fallback only fires on
*mis*-annotated verbs — which the plan's own module prose states correctly. The
tests now use `言う` tagged `v5s`. The implementation was not changed.

**`WordFlags::IS_NAME` moved from `0x0040` to `0x0080`.** ta-old's `0x0040` is
`JAP_WORD_TOP`, so the plan's claim that flag values "mirror `JAP_WORD_*`
exactly" was false for that value. `TOP = 0x0040` is now reserved for the
Phase 1B scorer port. The other six flags do match ta-old exactly, which is what
the differential run depends on.

**`INDEX_FORMAT_VERSION` is 2**, because `IndexHeader` gained
`conjugation_fingerprint`.

**quick-xml has no `Event::GeneralRef` in 0.36.** The plan guessed it did. DTD
entity references surface inside `Event::Text` as literal `&name;` bytes;
`jmdict::decode_text` handles them. The `SPIKE RESULT` comment in `jmdict.rs`
records the measured event sequence.

---

## Known issues carried into Phase 1B

**`prefixes_of` returns the empty-key hit on every call.** Correct — empty stems
are legitimate and `する`/`来る` produce nothing else — but the scorer must
special-case `key_chars == 0`, and each call bincode-deserializes the entire
empty-key record vector. On a real corpus that vector holds one record per verb
whose whole surface is its own remove-suffix. If it is large, load it once
rather than per call; Phase 1B's parse loop calls this per character position.

**The index build is not atomic.** `build_from_reader` writes all five files
directly into the target directory with `File::create`, which truncates in
place, while `load.rs` mmaps them. Rebuilding into a directory that has a live
`Index` open against it is undefined behaviour. Soundness currently rests on an
unenforced caller obligation, documented on `Index::open`. This becomes live the
moment a long-lived process holds an `Index` — i.e. Phase 1B and the Tauri app.
The fix is ~10 lines: build into a sibling directory and `fs::rename` the
*directory* (renaming five files individually is not atomic). That still does
not stop a third process, so the caller obligation survives regardless.

**`PRIORITY_MARKERS` includes `spec2` on documentation, not evidence.** The
EDICT `(P)` rule is documented as firing on `ichi1`, `news1`, `spec1`, `spec2`,
`gai1`. This could not be verified from anything in the repo — there is no
`JMdict_e.xml`, and ta-old read EDICT2 where `(P)` was already baked in.
**Confirm against the JMdict DTD's `ke_pri`/`re_pri` documentation before
interpreting the differential run**, or a few thousand entries' `COMMON` flag
will differ for a reason that has nothing to do with the port.

**The `TOP` doc comment overstates the flag's status.** It cites
`Dictionary.cpp:1010` and `:1239` without noting that those read sites — and the
write sites at `:306`/`:388` — are all inside `#ifdef SETSUMI_CHANGES`, which
nothing in ta-old ever defines. The `(T)` tagging feature is dead in the shipped
build. The bit reservation is still right; the comment will mislead whoever
ports `FindBestMatches`.

**`StemStats` can misattribute exact vs fallback stems.** For a headword
carrying both a correct and a same-length mis-annotated `v5*` tag, whichever POS
tag comes first in source order wins, and dedup drops the second before its
counter fires. The authoritative measurement of whether the fallback earns its
keep is the `--no-v5-fallback` A/B, not these counters — on the test fixture,
11 records with it versus 8 without.

**The CLI is the only place all seven modules meet, and it has 0% coverage.**
A single process round-trip test buys more than another unit test anywhere else.

**~13 public items have no non-test caller yet**: `to_katakana`, the `kana`
classifiers other than `unify`/`unify_str`, `tense_name`, `MAX_CONJ_DEPTH`,
`Form::is_formal`/`is_negative`, `WordFlags::remove`, `IS_NAME`,
`JmdictError::BadEntry`. They are tested, not exercised. Expect the first real
consumer to want a different signature from at least one — `to_katakana`'s
`Option` return is the likely candidate.

---

## Invariants Phase 1B must not break

- **`kana::unify` is character-wise and therefore prefix-stable.** The entire
  FST design depends on folding a prefix equalling the prefix of a folded
  string. `kana.rs` tests this over every char boundary.
- **`kana::strip_suffix_unified` is shared** by conjugation-chain resolution and
  stem generation. They must agree or stems stop lining up with chains. If you
  teach kana folding about long-vowel marks, small kana, or halfwidth katakana
  (which `unify` deliberately does *not* fold, faithfully to ta-old), it changes
  both.
- **A stem carries exactly one verb type.** `build.rs` maps it to storage via
  `verb_types.first()`, lossless only because of this.
- **Duplicate type names are deliberate.** `vk`, `vs`, `v5r-i`, `v5uru` each
  appear twice — one twin carries kanji-form suffixes, the other kana-form —
  which is how readings are reconstructed for irregular verbs. `types_named`
  returns all matches; the stem candidate loop scans by name, not id.
- **Fixed tense discriminants** `Remove = 0`, `NonPast = 1`, `Stem = 2`,
  `Potential = 3` are special-cased and must not be reordered.
- **`verb_type` ids are indices into the conjugation table.** An index is bound
  to its asset by `conjugation_fingerprint`; a changed `conjugations.json` makes
  `Index::open` refuse rather than silently resolve the wrong verb.
- **ta-old stored `verbType` as `vt + 1`** with `0` meaning "not a verb". This
  port uses a 0-based id in an `Option`. The differential run compares against
  the old encoding.

---

## Verification

```
cargo test -p jparser                              # 113 tests
cargo clippy -p jparser --all-targets -- -D warnings
cargo llvm-cov -p jparser --summary-only           # 93.85%

cargo run -q -p jparser --bin jparser-cli -- \
  build-index crates/jparser/tests/fixtures/jmdict_mini.xml /tmp/idx
cargo run -q -p jparser --bin jparser-cli -- lookup /tmp/idx "言うから"
```

The fixture yields keys=11, records=11, entries=3, skipped=0, exact_stems=2,
v5_fallback_stems=3, empty_stems=0. With `--no-v5-fallback`: 8 keys, 8 records,
0 fallback stems.
