# JParser Phase 2C — Handoff

Phase 2C gives `jparser`'s `BoundaryHints` trait its first real implementation.
`segment.rs` has weighted the trait into its DP since Phase 1B and nothing ever
produced one; this phase adds `VibratoTokenizer`, which tokenizes with
[Vibrato](https://github.com/daac-tools/vibrato) and derives per-position
boundary flags from an IPADIC-shaped dictionary, plus a `--hints` flag on
`jparser-cli parse` to reach it.

Executed from `docs/superpowers/plans/2026-08-14-jparser-phase2c.md` against
`docs/superpowers/specs/2026-08-14-jparser-phase2c-design.md`. **Both have since
been amended** at the sites listed below; read the amended plan and spec, not
the original text.

**Commits:** `c082dfc..3f761f8` on `master`, 8 commits, +716/−57 across 13
files. No branch, no worktree — the phase landed as a linear sequence of
commits directly on `master`.

**Verification at close:** `cargo test --workspace` → **292 passed / 1
ignored** (unchanged from the pre-phase count — 2C added no test outside the
`mecab` feature). `cargo test -p jparser --features mecab` → **271 passed**.
`cargo clippy --workspace --all-targets -- -D warnings` clean, plus (new)
`cargo clippy -p jparser --features mecab --all-targets -- -D warnings` clean.
`cargo llvm-cov -p jparser --features mecab` → **95.90%** crate lines,
**98.50%** on `hints.rs` alone (floor 80). Purity grep
`jmdict-source|ureq|flate2|vibrato` against `cargo tree -p jparser
--no-default-features` → **0**. `hints.rs` is **423 lines**, under the 800 cap.

---

## The public surface the next phase consumes

`crates/jparser/src/hints.rs`, the whole module gated `#[cfg(feature =
"mecab")]`. The feature is **not** in `default` — a caller opts in with
`--features mecab`, and `cargo check -p jparser --no-default-features
--all-targets` plus the purity grep above are what keep it that way.

```rust
pub struct VibratoTokenizer { /* tokenizer: vibrato::Tokenizer */ }

impl VibratoTokenizer {
    /// Load an uncompressed compiled Vibrato dictionary from `path`.
    pub fn load(path: &Path) -> Result<Self, HintsError>;
    /// Tokenize `text` and derive its boundary flags. A fresh worker per call.
    pub fn hints(&self, text: &str) -> BoundaryFlags;
}

pub struct BoundaryFlags { /* bad_start: Vec<bool>, bad_end: Vec<bool> */ }
impl BoundaryHints for BoundaryFlags {
    fn bad_start(&self, pos: usize) -> bool;
    fn bad_end(&self, pos: usize) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum HintsError {
    #[error("reading the vibrato dictionary at {path} failed: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("the vibrato dictionary could not be loaded: {0}")]
    Dictionary(String),
}
```

`READING_FIELD: usize` (`hints.rs:22`) and `fn flags_from_worker` (`hints.rs:69`)
are private — nothing outside the module needs the IPADIC field index or the
raw worker-to-flags derivation. Everything a caller touches is above.

**Worked example**, `jparser-cli.rs`'s `parse` arm (`crates/jparser/src/bin/jparser-cli.rs:275-297`):

```rust
Command::Parse { index, text, #[cfg(feature = "mecab")] hints } => {
    let table = ConjugationTable::load_embedded()?;
    let index = Index::open(&index)?;

    #[cfg(feature = "mecab")]
    let tokenizer = match &hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
        None => None,
    };
    #[cfg(feature = "mecab")]
    let flags = tokenizer.as_ref().map(|t| t.hints(&text));
    #[cfg(feature = "mecab")]
    let hints: Option<&dyn jparser::BoundaryHints> =
        flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
    #[cfg(not(feature = "mecab"))]
    let hints: Option<&dyn jparser::BoundaryHints> = None;

    let result = jparser::parse(&index, &table, &text, &ParseOptions::default(), hints)?;
    // ...
}
```

A load failure propagates through `?` and exits non-zero — a user who passed
`--hints` asked for hints, and silently parsing without them would return a
plausible result that is not what was requested. Absent, `parse` runs exactly
as before 2C; 1B/2A's committed CLI tests keep passing untouched.

The `mecab` feature adds one dependency: `vibrato = { version = "0.5",
default-features = false, optional = true }` in `crates/jparser/Cargo.toml`.
`default-features = false` drops vibrato's `train` feature and the `rucrf`
training stack — nothing here trains a model.

---

## Decisions that departed from the plan and the spec

Four, all ruled on by the human during execution. The plan file still
contains the superseded text at each site — see the amendments in
`docs/superpowers/plans/2026-08-14-jparser-phase2c.md`.

1. **Task 1 forbade any `.rs` file in its diff, and one appeared anyway.**
   Raising `rust-version` to 1.85 made clippy's MSRV-aware `unnecessary_map_or`
   fire — `Option::is_none_or` (stabilized 1.82) became available under the new
   floor. Fixed in the same commit (`7c17652`) at `generations.rs:116` and
   `rank.rs:53`. A follow-up commit (`8502a6b`) also unpinned `ureq` in
   `crates/jmdict-source/Cargo.toml`, whose `~3.2` pin comment cited the
   now-retired 1.75 floor.
2. **`HintsError::Io`'s frozen shape could not deliver what its own doc comment
   promised.** The plan froze `Io(#[from] std::io::Error)` under a comment
   claiming the error "names the file it could not load," with a test
   asserting only `!err.to_string().is_empty()`. That shape cannot name the
   file — `File::open`'s `io::Error` carries no path. Shipped as `Io { path:
   PathBuf, source: std::io::Error }`, with the test (`the_io_error_renders_usefully`,
   `hints.rs:308`) asserting the message contains the filename. Worth
   recording: `thiserror` renders a bare `{path}` shorthand for a `PathBuf`
   field directly, via its private `AsDisplay` trait — an implementer tried
   `path.display()` instead under a doc comment claiming `PathBuf` "does not
   implement `Display`," and that attempt was reverted in `70a697a` as an
   unnecessary deviation. `IndexError::GenerationExists` (`index/mod.rs:125-128`)
   and `SourceError::Http`/`TooManyAttempts` (`jmdict-source/src/lib.rs:66-70`)
   already relied on the same shorthand.
3. **Nothing compiled `hints.rs` at all before this phase's last commit.**
   `mecab` is off by default, so `cargo clippy --workspace --all-targets` never
   parsed the module, and CI had the identical hole — every "clippy clean" and
   "tests pass" checkpoint in Tasks 2-4 was clean by omission, not by verification.
   Closed with a dedicated `mecab` CI job (`cargo test -p jparser --features
   mecab` + `cargo clippy -p jparser --features mecab --all-targets -- -D
   warnings`) plus `--features mecab` added to the `msrv` and `coverage` jobs
   (`.github/workflows/ci.yml`).
4. **The plan's Task 5 Step 5 test invoked `["build-index", "idx",
   "mini.xml"]`**, but `Command::BuildIndex` takes `xml` then `out`
   (`jparser-cli.rs:51-58`) — the arguments were reversed and would have tried
   to build an index from a file named `idx`. The step's own instruction said
   to confirm the invocation against the file's other tests before using it;
   the shipped test (`cli_generations.rs:310`) uses the correct
   `["build-index", "mini.xml", "idx"]`.

---

## What the tests prove, and what they don't

The plan's Self-Review flagged its own weakest point: spec §10's sixth
required assertion — a sentence whose *segmentation* changes when hints are
supplied — was the phase's only end-to-end proof, and the plan doubted a
two-entry test dictionary could force it.

That resolved better than the plan expected.
`hints_change_which_segmentation_jparser_parse_returns` (`hints.rs:357-422`)
builds a real four-entry JMdict fixture, calls `jparser::parse` twice against
it, and shows the chosen cover flip: without hints, `東`+`京都` wins at cost
**18** (`東` alone costs `MATCH_BASE 10 + SINGLE_CHAR_PENALTY 1 = 11`; `京都`
adds `MATCH_BASE 10 - COMMON_BONUS 3 = 7` on top, `11 + 7 = 18`), beating
`東京`+`都` at cost **21** (`MATCH_BASE 10`, then `+ MATCH_BASE 10 +
SINGLE_CHAR_PENALTY 1`). With hints supplied, `bad_end(0)`/`bad_start(1)` tax
both matches straddling that interior by `MECAB_BAD_END`/`MECAB_BAD_START`
(10 each), so `東`+`京都` rises to **38** while `東京`+`都` is untouched at
**21** — the winner flips. Reviewers independently re-derived both totals
against `segment.rs`'s scoring constants and confirmed them.

**Be honest about the limit this proves.** The fixture engineers its own
baseline: it withholds a `ke_pri` marker from `東京` while giving one to
`京都`, so the plain dictionary-frequency tie-break lands on the *wrong*
split (`東`+`京都`) on its own, and the hint is what corrects it. Real JMdict
gives `東京` its own priority marker, so this proves the derivation reaches
`parse`'s output and can flip a cover — not that hints improve segmentation
against the real dictionary. The port design (`docs/superpowers/specs/2026-08-12-jparser-port-design.md:649-650`)
puts MeCab last precisely because it "is a ±10 tiebreaker on a 100/500
baseline, so it cannot be validated until the DP it nudges is known-good."
2C jumped that queue to give the trait an implementation, and this fixture's
engineered tie is where that shows: it is validated in isolation, not against
a DP whose baseline is trusted.

Two mutation checks ran on the derivation itself (plan Task 3 Step 6), each
proving a guard is load-bearing rather than a check that cannot fail:

| Mutation | Result |
|---|---|
| Deleted the reading guard (`if !matches!(reading, ...) { continue; }`) | `a_token_without_a_reading_marks_nothing` failed, as predicted |
| Changed `range.end.saturating_sub(1)` to `range.end` | `a_multi_char_token_marks_only_its_interior` and `a_single_char_token_marks_nothing` both failed, as predicted |

Both were restored before commit.

---

## Known issues carried forward

All in `hints.rs` unless noted:

- **`test_dictionary()` is `pub(crate)` inside a private `mod tests`
  (`hints.rs:163`)** — a no-op, since a private module's contents are not
  reachable from outside it regardless of the inner item's own visibility.
  Harmless while every test that needs it lives in this same file.
- **`empty_input_yields_empty_flags` (`hints.rs:234`) cannot distinguish an
  empty `Vec<bool>` from one that is all-`false`.** It is a no-panic smoke
  test on empty input; the name claims more than the assertions check.
- **`out_of_range_positions_are_false_not_a_panic` (`hints.rs:243`) probes
  `999`, not `usize::MAX`.** Totality holds by construction — `bad_start`/
  `bad_end` read through `Vec::get`, which is total for any `usize` — so a
  larger probe would add no coverage, only a bigger number.
- **The end-to-end test's own doc comment (`hints.rs:339-355`) calls its
  fixture "a realistic failure mode."** See the honesty note above: it is a
  realistic *shape* of failure (frequency alone cannot disambiguate the two
  splits), but the specific tie it breaks is manufactured by withholding a
  marker real JMdict supplies.
- **That same test couples `index::build`/`load`, `stem`, `conjugation`, and
  disk I/O (a `scratch` temp dir) inside `hints.rs`'s unit-test module.**
  Moving it to `tests/` would need `flags_for` and `test_dictionary` to become
  non-test API, which is a worse trade than the coupling.
- **`jparser-cli.rs:285-292` shadows `hints`.** The `Parse` variant's field
  `hints: Option<PathBuf>` (bound at line 279, last read at line 285) is
  rebound eight lines later at line 292 as `hints: Option<&dyn
  jparser::BoundaryHints>`. Legal and intentional — the CLI flag and the
  trait object share a name because they share a purpose — but it reads as a
  type change on the same identifier to anyone skimming the diff.

---

## What Phase 5 still owes

The port design's Phase 5 bullet (`docs/superpowers/specs/2026-08-12-jparser-port-design.md:643-644`)
is "Vibrato integration, on-demand dictionary download, boundary hints,
toggle." 2C delivered integration and hints. Still missing:

- **On-demand download and archive extraction.** The distributed IPADIC
  artifact is a `.tar.xz` containing a zstd-compressed `system.dic`; 2C
  deliberately does not decompress it (spec §5), which is what keeps
  `vibrato` this phase's only new dependency. A user extracts by hand today —
  `jparser-cli parse --help` documents the download URL and the `tar`/`zstd`
  commands (`jparser-cli.rs:128-136`, added in the phase's last commit,
  `3f761f8`).
- **A persisted enable/disable toggle.** Belongs with settings and
  persistence, which do not exist yet (port design §8).

---

## Invariants the next phase must not break

Carried forward from the 2B handoff, verified still current there
(`docs/superpowers/2026-08-13-jparser-phase2b-handoff.md`, "Invariants the next
phase must not break"): `INDEX_FORMAT_VERSION` stays 3; `EntryData`'s field
order is wire format; a published `gen-N` is immutable; directory knowledge
lives only in `generations.rs` and `ensure_dictionary`; the staging filename
stays process-unique; a `.partial` file is never resolved. 2C touched none of
these — `hints.rs` knows nothing about generations or the source archive, only
the one dictionary path it is handed. Also still standing: never `cargo fmt`
(`conjugation.rs` stays deliberately not rustfmt-clean); `segment.rs` is at
778/800 lines and must not be edited casually; no dependency may link
`native-tls`/OpenSSL (GPL v2).

2C adds two of its own:

- **`crates/jparser`'s library stays dependency-pure, and `mecab` stays off
  `default`.** `cargo check -p jparser --no-default-features --all-targets`
  must keep succeeding, exactly as it did before `vibrato` existed in the
  tree.
- **The purity grep must keep returning 0**, now against `jmdict-source|ureq|
  flate2|vibrato` rather than the three-name list 2B left it at
  (`.github/workflows/ci.yml`, `purity` job). Any future optional dependency
  added to the library side needs its name added to this same grep, in the
  same commit.
