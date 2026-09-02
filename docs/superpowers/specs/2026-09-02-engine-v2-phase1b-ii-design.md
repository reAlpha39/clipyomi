# Engine v2 Phase 1b-ii — The switchover and the relicence

**Date:** 2026-09-02
**Master plan:** [`2026-08-27-engine-v2-master-plan.md`](2026-08-27-engine-v2-master-plan.md) §1, §3.2, §3.5, §4.3, §7 Phase 1 and Phase 1b, §9.2, §9.3
**Phase 1a design:** [`2026-09-01-engine-v2-phase1a-design.md`](2026-09-01-engine-v2-phase1a-design.md) §4.1, §8
**Phase 1a handoff:** [`../2026-09-01-engine-v2-phase1a-handoff.md`](../2026-09-01-engine-v2-phase1a-handoff.md) §9
**Phase 1b-i handoff:** [`../2026-09-02-engine-v2-phase1b-i-handoff.md`](../2026-09-02-engine-v2-phase1b-i-handoff.md) §4, §5
**Branch:** `feat/engine-v2`, from `0f3ab67`
**Status:** Design, approved in session, not implemented

---

## 1. Scope

Phase 1b-ii is the second and last half of Phase 1b. It makes the v2 engine
the only engine, deletes the nine hard-taint files master plan §3.2 names,
and relicenses the tree from `GPL-2.0-only` to `MIT OR Apache-2.0` — master
plan §1's goal, discharged.

**In.** Four commits, in this order and no other:

1. **Relocate.** Move the types v2 shares with v1 out of the deletion set,
   make the index layer engine-agnostic, and bump the index format. v1 is
   still the default and still compiles.
2. **Flip.** Make v2 the default everywhere — `src-tauri`, `jparser-cli`,
   every test — route the stale-index startup error to the download screen,
   and land the one metric-neutral table fix Phase 1b-i's handoff names.
3. **Delete.** Remove the nine files, `matcher/verb.rs`,
   `assets/conjugations.json`, the `xtask` crate, and the ported half of
   `hints.rs`. The compiler proves nothing references them.
4. **Relicense.** Replace every GPL header with the SPDX header, remove every
   `ta-old` mention and every `ported from Translation Aggregator` string
   tree-wide, swap the licence files, and run `cargo fmt` once.

**Out.** Any change to `cost.rs`, `lattice.rs`, `lex.rs`, `assemble::order`
or `table_data.rs` beyond §9's one `ATTACHES_TO` row reorder. k-best (Phase
2). Extra linguistic layers (Phase 3). Any UI change beyond what §3's startup
fix requires. Closing Phase 1a's open metric cells — see §10.

**Why four commits, when Phase 1a design §4.1 said one.** §4.1's reason for
"one commit, nothing shipped in between" was to avoid a shipped window in
which the engine emits `conjugation: None`. Phase 1b-i closed that window: v2
now labels every conjugated entry. What §4.1 still protects is the invariant
that the tree is never labelled `MIT OR Apache-2.0` while a line of ported
code remains. Ordering the licence flip (commit 4) strictly after the last
deletion (commit 3) preserves that invariant, and every intermediate commit
compiles and passes the standing gates. A single commit across roughly ninety
files would be unreviewable and would bisect a mistake to nothing; four
reviewable pieces are strictly safer. Recorded in §11.

---

## 2. Relocations and the index format (commit 1)

The dependency map taken before this design shows exactly four things the
v2 engine or the shared index layer take from the nine files. Each gets one
decision.

**`WordFlags` → `engine/flags.rs`, redefined fresh.** `record.rs` is
hard-taint and is not opened. The type is redefined from what is observable:
a `u16` newtype with eight consts — `PRIMARY`, `PRONOUNCE`, `COMMON_LINE`,
`COMMON`, `PARTICLE`, `COUNTER`, `TOP`, `IS_NAME` — serialising to the same
lowercase wire names `src/types.ts`'s `FlagName` already declares. The eight
names are the public contract (Phase 1a design §8); the bit values are not.
Every existing index is invalidated by this phase regardless (fingerprint
and format both change), so the bit assignment is free and is documented as
arbitrary. `engine/flags.rs` is the natural home: it is where v2 derives
flags from a `RawEntry`, and it is one of the five engine modules that
already import the type. The other four (`assemble`, `cost`, `lattice`,
`lex`) repoint to `crate::engine::flags::WordFlags`. `lib.rs` re-exports it
at the crate root as `jparser::WordFlags` — `jparser::record::WordFlags`
cannot survive, since `record` is deleted — and the three external importers
(`bin/jparser-cli/main.rs`, `tests/index_roundtrip.rs`, `tests/engine_v2.rs`)
update their `use` lines.

**`StemStats` and `BuildReport.stems` — deleted.** v2 generates no stems;
`engine/build.rs` fills the field with `StemStats::default()` and the field's
only reader is three `println!`s in `jparser-cli build-index`. Both go.

**`StoredRecord.verb_type` and `VerbTypeId` — deleted; `INDEX_FORMAT_VERSION`
3 → 4.** v2 always writes `None`. Removing the field changes the on-disk
record shape, which the version bump makes explicit; since the fingerprint
change already invalidates every index, the bump costs nothing extra and
leaves no dead field behind.

**`Index::open(dir)` becomes `open_with(dir, engine::build::fingerprint())`.**
Today it hardcodes `conjugation::embedded_asset_fingerprint()`. `open_with`
stays public for tests that build under a deliberately wrong fingerprint.

**`Index::prefixes_of` — deleted.** Its default fold is v1's `kana::unify`;
v2 calls `prefixes_of_with(key, engine::kana::fold)` at every site and never
the default. Removing it removes the index layer's last import from `kana.rs`.

**`engine/build.rs`'s `fingerprint_differs_from_the_v1_conjugation_asset`**
becomes a stability test: it pins the current `fingerprint()` value as a
literal, so a regeneration of `table_data.rs` is a visible, deliberate change
rather than a silent one — the same shape as Phase 1b-i's exact 21,760 chain
count.

Everything else the map found is v1-only and goes in commit 3.

---

## 3. Index lifecycle and startup (commits 1–2)

**`ensure_dictionary(root, keep, fetch)`.** The `table: &ConjugationTable` and
`opts: &StemOptions` parameters go; `generations::build_new` calls
`engine::build::build_from_reader`. v1's `index::build::build_from_reader` is
deleted; the `write_index` helper Phase 1a extracted from it, which both
builders already share, stays where it is.

**The startup hazard, and its fix.** `src-tauri/src/state.rs::load_state`
opens the newest generation with `Index::open`. After this phase that check
uses v2's fingerprint, so every user upgrading with a v1-built index gets
`IndexError::ConjugationMismatch`. Today `is_missing_dictionary` maps
`StartupError::Index(_)` to `false`, and the webview treats a non-recoverable
startup error as fatal — the user would see an error screen for a condition
the download screen already knows how to repair. The fix is one arm:

```rust
StartupError::Index(IndexError::ConjugationMismatch { .. } | IndexError::VersionMismatch { .. }) => true,
```

Those two are exactly the errors `ensure_dictionary` already rebuilds on, so
the existing download path repairs a stale index with no new UI and no new
command. Every other `IndexError` stays fatal. `is_missing_dictionary`'s own
doc comment demanded that a new case force a decision here; this is it.

**`AppState.table`** becomes `engine::table::Table`, constructed by
`Table::new()`. It is infallible, so `StartupError::Conjugation` and the
`ConjugationError` it wrapped go with `ConjugationTable`.

---

## 4. Public API, application, CLI (commit 2)

**`jparser::parse` is v2.** `engine::parse_v2` is renamed `parse` and
re-exported at the crate root; its signature is v2's:

```rust
pub fn parse(
    index: &Index,
    table: &engine::table::Table,
    text: &str,
    opts: &ParseOptions,
    bounds: Option<&dyn Boundaries>,
) -> Result<ParseResult, ParseError>
```

`engine::Boundaries` is re-exported at the crate root where `BoundaryHints`
was. `ParseOptions` is already field-less and shared by both engines; it
stays as it is. `Segment`, `Entry`, `ParseResult` and `ParseError` are already
shared and unchanged; `Entry::steps`'s doc drops its "empty on the v1 path"
clause.

**`src-tauri`, five files.** `state.rs`: the table type, the startup arm
above, the deleted error variant. `parse.rs`: the worker calls
`tokenizer.boundaries(&text)` instead of `.hints(&text)` and passes the
result as `Option<&dyn Boundaries>` — the hard-constraint path Phase 1a
measured as hints-on. `commands.rs`: the `ensure_dictionary` call loses two
arguments and the `ConjugationTable`/`StemOptions` imports. `test_support.rs`:
the same. `main.rs`: unchanged — `VibratoTokenizer` and its `watch` channel
carry over as they are.

**`jparser-cli`.** `--engine` and `EngineChoice` are deleted, and with them
the `main.rs:55-57` comment Phase 1b-i's handoff flagged as stating an
obsolete reason. `--no-v5-fallback` is deleted (it configured v1's stem
generator). `build-index` loses its three stem-statistics lines. `open_for`
collapses to `Index::open`. `diff`, `label` and `score` call `parse`; `score`
loses its v1 arm and the `--engine` plumbing Phase 1a Task 16a added. The
`Romaji` subcommand calls `engine::romaji::to_romaji` and **loses its
`--particle` flag**: that flag applied v1's `apply_particle_fixup` (は → wa,
へ → e on the romaji string), a function with no v2 counterpart, and
`jparser::romaji` has no caller but this subcommand — the application renders
its romaji furigana mode in TypeScript (`src/render/furigana.ts`) and never
touches the crate's converter. Porting a two-rule fixup fresh for a
developer utility nobody ships is not worth a file; if it is wanted later it
is a ten-line addition to `engine/romaji.rs`, written from the behaviour.

**`examples/boundary_diag.rs`** repoints to `parse`. It is the Phase 0
diagnostic that generated `spurious_merges.txt` and may be needed again.

---

## 5. Hints (commits 2–3)

`hints.rs` is soft-tier (master plan §3.2) and is the only production
implementor of `engine::Boundaries`. It stays. What goes with v1:

- the `BoundaryHints` trait (defined in `segment.rs`, re-exported from
  `lib.rs`);
- `BoundaryFlags` and `impl BoundaryHints for BoundaryFlags`;
- `VibratoTokenizer::hints()`, which produced them;
- `flags_from_worker`, the one function §3.2 flags as a documented port of
  `Dictionary.cpp:1115-1126`.

Master plan §4.3 said Phase 1 would *dissolve* the `flags_from_worker`
question rather than argue it: tokenizer output becomes a DP constraint via
`boundaries()`, so the ported function has no successor. Deleting it is the
dissolution. `VibratoTokenizer::load`, `boundaries()`, `BoundarySet`,
`HintsError` and `write_test_dictionary` all stay.

---

## 6. Test disposition (commit 2)

Six test files, one example and two in-file test modules depend on v1. One
decision each; the rule is that a test of engine-neutral behaviour is
repointed, a test of v1-specific behaviour is ported only where v2 must
satisfy the same observable, and a test whose subject no longer exists is
deleted.

| File | What it tests | Disposition |
| --- | --- | --- |
| `tests/parse_snapshots.rs` + `snapshots/parse_snapshots__sentences.snap` | v1's 30-sentence snapshot | **Delete.** `tests/engine_v2.rs` owns the same 30 sentences against v2 already. |
| `tests/parse_irregular.rs` | v1's する / 来る special handling | **Port, then delete.** Each assertion is read for the observable it pins (した → する, きた → 来る, …); those v2 must also satisfy move into `engine_v2.rs`. An assertion v2 legitimately fails is a finding for the handoff, not a test to keep. |
| `tests/entry_id.rs` | `Entry::entry_id` is the JMdict `ent_seq` | **Repoint** to `parse`. Engine-neutral. |
| `tests/cli_parse.rs`, `cli_score.rs`, `cli_label.rs` | Drive the binary | **Repoint**: fixtures built with `engine::build`. `cli_parse.rs`'s test that `--engine v2` rejects a v1-built index is deleted — there is no second engine to reject. |
| `tests/index_roundtrip.rs`, `generations_torn_build.rs` | The index layer | **Repoint** to the v2 builder; `verb_type` assertions go with the field. |
| `tests/engine_v2.rs` | v2 | Loses the v1 fixture it builds for `the_two_engines_cannot_read_each_others_indexes`; that test becomes "an index built under a different fingerprint is rejected", using `open_with` and an arbitrary wrong value. |
| `examples/boundary_diag.rs` | Phase 0 diagnostic | **Repoint** to `parse`. |
| `hints.rs`'s and `matcher.rs`'s `mod tests` | Build v1 fixtures | `matcher.rs`'s go with the file; `hints.rs`'s repoint to `engine::build` and lose their `BoundaryFlags` cases. |

`Cargo.toml`'s `[[test]]` table loses no entries it needs; `parse_snapshots`
and `parse_irregular` had none.

---

## 7. Deletions (commit 3)

- `crates/jparser/src/{segment,matcher,stem,rank,record,kana,romaji,conjugation}.rs`
  and `matcher/verb.rs` — the nine hard-taint files of master plan §3.2.
- `crates/jparser/assets/conjugations.json` — the copied data.
- The `xtask/` crate and its `[workspace] members` entry. Its one subcommand
  converted `../ta-old/dictionaries/Conjugations.txt` into that asset;
  nothing else lives there.
- `.gitignore`'s `ta-old/` line. The upstream tree is a sibling directory,
  never inside the repository, and after this phase must not be.
- In `lib.rs`: `parse`'s v1 body and every private helper reachable only
  from it (`entry_data`, `assemble_entry`, `dictionary_form`,
  `reconstruct_reading`, `strip_remove_suffix`, `kuru_hack`, `tails_match`,
  `KURU_HACK_MAX_CHARS`), the `mod` declarations, and the `BoundaryHints`
  re-export.
- In `hints.rs`: §5's four items.

Commit 3 must leave zero references. The compiler is the check, and
`cargo check -p jparser --no-default-features` is the one that catches a
dependency hidden behind the `mecab` gate.

---

## 8. Provenance and licence (commit 4)

Master plan §7's Phase 1 done-when is a tree-wide grep, not a directory list
— it fell behind a directory list twice. This commit is measured by that
grep.

**Headers.** 74 files carry the GPL-2 boilerplate. Each gets master plan
§9.2's header. On the engine files and any file written under the
clean-room discipline:

```rust
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's
// JParser. See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md
// §3.5 for the clean-room discipline this file was written under.
```

Elsewhere, the SPDX line alone. The Phase 1a and 1b-i engine headers' "the
SPDX header of §9.2 is written in Phase 1b" note is replaced by the header it
promised. TypeScript, CSS and Python files take the same SPDX line in their
own comment syntax.

**Strings.** The 47 `ported from Translation Aggregator` headers and the 78
`ta-old` mentions go to zero. Most vanish with commit 3's deletions. The rest
are behavioural — `src/**.ts` and `src-tauri/` describing "ta-old's rule" for
a tooltip or a popover, `jmdict.rs` describing what it replaced — and are
reworded to describe the behaviour without naming the source, because the
done-when counts the literal string. `tests/fixtures/spurious_merges.txt`
loses both its `ta-old` mention and its dangling citation of Phase 1a's
gitignored `task-15-report.md`; the substance of that report is in Phase 1a's
committed handoff, which the comment cites instead.

**Licence files.** Root `Cargo.toml`'s `[workspace.package] license` becomes
`"MIT OR Apache-2.0"`; every member inherits it. The root `LICENSE` (GPL-2
text) is replaced by `LICENSE-MIT` and `LICENSE-APACHE`. `package.json`, which
has no licence field, gains `"license": "MIT OR Apache-2.0"`.

**Unchanged, and separately noticed.** The JMdict subset fixtures stay CC
BY-SA 4.0 data assets with their own header (`fixtures/README.md` already
says why they are not relicensed with the crate). `IPADIC_NOTICE` stays
beside `table_data.rs`. The Wikinews corpus text stays CC BY 2.5 under its
own notice. None of these is source, and none is affected.

**Dependencies.** A one-time read of `cargo tree`'s licence set confirms no
GPL-family crate is in the graph — master plan §3.4's constraint — and the
handoff records the result. Not automated; there is no CI to hang it on yet.

**`cargo fmt`** runs once, over the whole `jparser` crate, as this commit's
last step. Phase 1b-i counted 39 sites, 32 of them in files this commit
rewrites or deletes anyway. Landing them here costs nothing and ends the
drift.

---

## 9. The one bundled table fix (commit 2)

Phase 1b-i's handoff §4 finding 5: 特殊・タ and 特殊・タイ both emit a bare `た`
surface, the two chains tie in `lex::dedup_shallowest`, and `ATTACHES_TO`'s
row order — `("たい", …)` before the two `("た", …)` rows — decides that the
Want-to reading survives. The final review of that phase called this a
blocker for the moment v2 becomes the default: *"Polite Want-to"* on ました
is wrong on the most common polite form in the language.

**The fix taken here is the cheap one.** The `("たい", "特殊・タイ", "連用形")`
row moves below the two `た` rows. It changes which of two equal-depth chains
survives dedup and nothing else: the survivor has the same span, `len` and
`entry_id`, which is the whole of what `score.rs::accumulate` reads.

**It is measured anyway.** The neutrality argument is derived from the code;
1b-i's handoff asked that it be confirmed the way §8.4 was. Expected: the
snapshot's 読みました, しました and 書かせられた flip to `Past`; the coverage
walk's 21,760 is unchanged (same rows, different order); `score` reproduces
1b-i's eight cells exactly. A difference in any cell is a finding, not a
number to record.

**The principled fix is named, not taken.** Making `たい:ガル接続`
non-terminal — no `ATTACHES_TO` row requires that form — would delete
candidates and can move the DP. That is a Phase 1a follow-up, measured. The
same goes for the godan 仮定形/命令ｅ tie (書け as Conditional or Imperative),
which the row reorder does not touch.

---

## 10. Done when

1. `git grep -c "ta-old"` and `git grep -c "ported from Translation
   Aggregator"` both return nothing, tree-wide.
2. `git grep -l "GNU General Public License"` returns nothing outside
   `LICENSE-APACHE`'s own text.
3. `cargo test -p jparser` with and without `--features mecab`; `cargo clippy
   -p jparser --all-targets -- -D warnings` in both feature states; `cargo
   check -p jparser --no-default-features`; and `cargo test -p clipyomi` (the
   `src-tauri` crate) — all green, at every one of the four commits.
4. The application, started against a v1-built index, reaches the download
   screen rather than a fatal error, rebuilds, and parses.
5. `jparser-cli score`, both hint states, reproduces Phase 1b-i's eight cells
   exactly after §9's reorder.
6. `jparser-cli parse 読みました` labels 読む `Polite Past`.

**Phase 1's own done-when, amended.** Its relicence clause — zero hits of both
strings tree-wide — is met by this phase. Its metric clause — both Phase 0
metrics at or above baseline in both hint states — is **not met and is
recorded as superseded**, per the decision taken in this design's session:
master plan §7's Phase 3 note already concedes that the tolerant gap is a
structural ceiling in JMdict's priority tagging that only a frequency layer
can cross, so no change within Phase 1's tools can satisfy the clause as
written. The numbers stand beside the amendment: in the configuration the
application ships (hints on), v2 is +10.7 points strict, −1.6 tolerant, −0.9
entry top-1 against v1. Strict measures the particle-absorbing defect class
this project exists to fix; tolerant measures compound preservation, which
Phase 3 owns.

---

## 11. Master plan edits this design owes

- §7 Phase 1b-ii marked complete, with the four-commit sequencing and the
  hazard §3 found.
- §7 Phase 1's done-when amended as §10 states, with the numbers.
- §3.2's taint map marked discharged: the hard tier deleted, the soft tier
  scrubbed (§9.3's assumption held), the clean tier's header strings gone.
- §4.3: `flags_from_worker` recorded as dissolved, not argued.
- §9.2: the header written; the tree relicensed.
- §10 decision log, the rows below.

---

## 12. Decision log

| Decision | Rationale |
| --- | --- |
| Four commits, licence flipped last, instead of §4.1's one | §4.1's reason (a shipped `conjugation: None` window) was closed by 1b-i. The invariant that survives — never labelled MIT while ported code exists — holds when commit 4 follows commit 3. Ninety files in one diff bisect to nothing (§1) |
| `WordFlags` redefined fresh in `engine/flags.rs`, bit values arbitrary | The eight wire names are the contract; every index is invalidated this phase anyway, so the bits are free. `record.rs` is not opened (§2) |
| `verb_type` dropped, format 3 → 4 | v2 never writes it; the rebuild is happening regardless; a dead field is worse than a bump (§2) |
| `is_missing_dictionary` routes `ConjugationMismatch`/`VersionMismatch` to the download screen | Those are the exact errors `ensure_dictionary` already repairs; without this every upgrading user sees a fatal error for a fixable condition (§3) |
| `flags_from_worker` deleted, not rewritten | Master plan §4.3 said Phase 1 dissolves the question: constraints replace advisory flags, so the port has no successor (§5) |
| `parse_irregular.rs` ported by observable, not kept | The tests pin what a parse of した must produce, not how v1 produced it; v2 must satisfy the same or the gap is a finding (§6) |
| `xtask` crate deleted whole | Its single job was the ta-old converter; an empty crate is worse than none (§7) |
| Behavioural `ta-old` mentions reworded, not kept | The done-when counts the literal string; describing the behaviour without the name loses nothing (§8) |
| The たい row reorder taken; the non-terminal fix named and left | The reorder is provably score-neutral and fixes the most visible wrong label; the principled fix moves the DP and belongs to a measured 1a follow-up (§9) |
| Phase 1's metric clause recorded as superseded, not left open | The master plan itself assigns the remaining gap to Phase 3's frequency layer; a done-when no Phase 1 tool can satisfy should say so rather than stay silently open (§10) |
