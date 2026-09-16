# Claims audit — what the repository asserts that is not true

**Date:** 2026-09-16
**Branch:** `develop` at `cb7b994`
**Scope:** every document and comment that speaks in the **present tense**.

## 1. Why this was run, and the rule it produced

Three times in one session a recorded claim turned out false because nobody
had re-checked it against its source:

- `.github/workflows/ci.yml` asserted "the repository has no remote", four
  times over, and used it to justify a real behaviour. There is a remote.
- Master plan §2's criterion 2 had been **failing for eight phases**. Entry
  top-1 sat below the v1 baseline in both hint states and every phase compared
  itself to the phase before instead of to the criterion.
- The Phase 4 writeup, one day old, carried sixteen false claims including one
  that inverted its own table.

The common mechanism is worth stating once:

> **A recorded number is a measurement with a date on it. A present-tense
> sentence is a claim about now, and it decays silently.** A phase that moves
> a cell must rewrite every present-tense statement of that cell in the same
> commit, or the next reader inherits a confident falsehood.

The corollary that made this audit tractable: **a dated handoff recording
"measured 1768 on 2026-09-12" is history and is fine.** Only documents that
say *is*, *now*, *today*, *the current figure*, or that describe how the code
works, can rot. Handoffs were therefore excluded; specs, code comments, the
README and CI were not.

## 2. The finding that cost something

Master plan §5.2 carries a block headed **"THE CURRENT BASELINE, and the one
place to read it from"**, which tells the reader every other table is history
and this one is authoritative.

**It was stale on five of its ten cells** — two phases behind on hints-on,
one behind on hints-off entry top-1.

That is the direct mechanism behind criterion 2 going unmet for eight phases.
The one block a reader is told to trust was not being updated, so nothing ever
compared itself to the v1 baseline. It now carries current figures and an
explicit rule that a phase moving any cell must rewrite the table in the same
commit.

## 3. Fixed in this pass

### Master plan and ULLE review (`98b80d5`)

| # | Claim | Truth |
| --- | --- | --- |
| 1 | "Status: Design draft, not approved, not implemented" | Five phases shipped and merged |
| 2 | §5.2's "current baseline" | Stale on 5 of 10 cells |
| 3 | §1.1 "ClipYomi is GPL-2.0-only and cannot be relicensed"; dual licensing "closed" | Root manifest is `MIT OR Apache-2.0`; both licence files present; `git grep GPL-2.0-only -- ':!docs'` is empty |
| 4 | §1.1 two `src-tauri/Cargo.toml` comments cite "Apache-2.0 alone is incompatible…" | That string is in no tracked file |
| 5 | §3.1 "this repository's own headers already assert" GPL-2.0-only | Every header asserts the permissive pair |
| 6 | §4.2 describes `segment.rs` in the present tense | Deleted at Phase 1b-ii |
| 7 | §4.3 "the DP cannot recover"; 63 spans suppressed | Phase 4's escape; 63 is a 2026-09-02 number, never re-measured |
| 8 | §9.6 `hints.rs` "now resolves the column … `TA_HINTS_READING_FIELD`" | No such code, no such variable outside two docs |
| 9 | §9.2 "ten line-range citations survive" | Seven, and resolved 2026-09-09 |
| 10 | §3.2 clean tier "25 references" | Zero |
| 11 | §9 open question 3 left unstruck | Answered at Phase 1b-ii |
| 12 | §7 "both strings grep to zero tree-wide" | True only outside `docs/`; this file contains one |
| 13 | §5.4 "67k captured lines" | 66k, per two other sites |
| 14 | §6 and §7 omit Phase 4 entirely | Root cause of 2 and 7 |
| 15 | §4.1 `Worker`'s public surface enumerated | Short by four methods; conclusion unaffected |
| 16 | ULLE note "§10 records four decisions" | Three name it |

### Engine, CLI and tests (this commit)

- **A real defect, not prose:** `src-tauri` declared `rust-version = "1.88"`
  while depending *directly* on `vibrato-source`, whose floor is 1.89. Cargo
  treats that as a hard error, so the declared floor was unbuildable. CI never
  caught it — the `msrv` job checks `jparser` and `jmdict-source` only. Raised
  to 1.89.
- `engine/mod.rs` — `parse`'s own doc still described the pre-Phase-4 hard
  constraint. The commit that fixed two comments about this missed the public
  entry point. Also "three stages" where there are four.
- `cost.rs` — the module doc said the particle term was "not implemented";
  `FUNCTION_STACK` *is* it. `cost`'s term list omitted `FUNCTION_STACK`. The
  `FUNCTION_STACK` measurement paragraph was undated and two phases stale on
  its hints-on half while the hints-off half beside it was exact. "Reproduced
  trial 2's exact corpus numbers" overstated the trial log. The §9.1 example
  list contained two examples that falsify the sentence they illustrate and
  one that is not in §9.1. The `丨FIFA` worked example cited the wrong section.
- `assemble.rs` — "`order`'s five clauses" is seven; the bar on what may enter
  the key was attributed to master plan §5.2, which sets no such bar.
- `tests/engine_v2.rs` — a comment claiming its own test "fails today" (it
  passes); "no frontend consumer exists" for `alternatives` (`src/main.ts:739`
  consumes it); "the two tests" that need real boundaries (eight); "eight
  entries" in the hint fixture (fifteen); and the −88/+8 vs 1768→1713
  arithmetic I wrote yesterday, which mixed spans with tokens.
- `bin/jparser-cli/main.rs` — "No UI, no Tauri, no network" on a binary with
  two network subcommands; `--compounds-detail` documented as listing every
  window when it lists only mismatches.
- `bin/jparser-cli/bench.rs` — `median` documented as the lower middle; it
  returns the upper, and the test asserting so was named for the wrong one.
- `bin/jparser-cli/diff.rs` — "duplicated from `hints.rs`' private constant";
  that constant went with the v1 engine and this is the only copy.
- `hints.rs` — "the one production implementor"; `engine::relax::Relaxed` is a
  second, added by Phase 4.
- `e2e/panes.spec.ts` — **the fifth "no git remote here" site**, which commit
  `5273ca1` claimed to have corrected and missed.

## 4. Outstanding, by severity

Not fixed here. Each was verified by an auditor against the tree; spot-checks
of a sample were correct, but **treat each as a lead to confirm, not a fact**,
because auditors were wrong at least once in this session.

### 4.1 Wrong about what the code does

- `src-tauri/src/layout.rs:526` — "Twenty bindings"; `BINDINGS` has **two**,
  and the test it documents is named `the_binding_table_is_two_distinct_keys`.
- `layout.rs:105, :170, :498` — documents `Shift+Alt+N` / `Alt+N` / `Alt+7`
  slot keys. No slot key is registered; the same file says so twice and a test
  enforces it.
- `state.rs:434` — "`load_state` is the only code that reads it"
  (`TA_HINTS_DICT`); the sole reader is `load_hints`.
- `state.rs:25` — "settings persistence is 2E"; there is now a persisted
  `Settings::mecab_hints` and a checkbox.
- `commands.rs:1168` — "all eight"; the block assigns ten keys.
- `commands.rs:1390` — "all three"; `currentSnapshot()` sends six.
- `commands.rs:942-952` — two doc comments merged onto one item: the text
  describes `peek_titlebar`, which now carries none.
- `src/main.ts:388-390, :405-406` — "the OS frame grows upward (macOS,
  Windows)"; `peek_grows_frame()` is macOS-only and two other authorities say
  so. `src/main.test.ts:1068` repeats it.
- `src/main.ts:1106-1111` and `main-tooltip.test.ts:796-800` — a stated
  premise (`pendingChip` null during `placeFor`'s flight) that a later commit
  falsified.
- `src/settings.ts:420-421` — the static-NodeList reasoning is backwards.
- `src/render/tooltip.ts:10-12` and `tooltip-text.ts:27-28` — describe a
  definitions pane deleted in `ac89eb2`.
- `src/settings-appearance.ts:17-18` — "five colour swatches … the outline
  checkbox"; seven swatches, two checkboxes.
- `src/main.test.ts:998-1000` — "CSS reveals the band on `:hover`"; no such
  rule ever existed.
- `crates/jparser/src/kanjidic.rs:141-143` — the stated reason is false in
  exactly the case the preceding clause describes; the conclusion survives.
- `crates/jparser/src/jmdict.rs:26-27` — `dial`/`field` are not handled at all.
- `jmdict.rs:43-51` — defers a confirmation "once a real `JMdict_e.xml` is
  available"; two in-tree fixtures carry the DTD documentation that confirms it.
- `crates/jmdict-source/src/lib.rs:5-6` and its ```` ```ignore ```` example —
  an `ensure_dictionary` signature that no longer exists. The `ignore` fence is
  why the compiler never caught the drift.
- `jmdict-source/src/lib.rs:32-36` — "a download that fails … leaves its
  staging file behind"; `fetch.rs` removes it and three tests assert so.
- `crates/vibrato-source/src/lib.rs:364` — "a single `is_file`"; `resolve`
  always does a full `read_dir` plus per-entry `metadata`.
- `vibrato-source/src/lib.rs:185-186` — "Phase 5B calls this to decide whether
  the MeCab toggle can be offered"; the toggle never calls it.
- `index/generations.rs:9-10` and `generations_torn_build.rs:99` — "five index
  files"; six since Phase 3a, and the same test file's own array is `[&str; 6]`.
- `index/load.rs:60-63` — proposes as future work a scheme `generations.rs`
  already implements.
- `index/mod.rs:319, :385` — "~60 MB download"; the same file says ~12 MB.

### 4.2 Stale numbers stated in the present tense

- `tests/fixtures/gold_sources.md:55-60` — the file that opens "Read this
  before quoting any score" carries three pre-Phase-1 figures: "16.5 points",
  "167 merges", "0.8465".
- `tests/fixtures/README.md:106-107, :117-118` — "139 dictionary forms" (135
  distinct; 139 is the pair count) and "12 of the metric's misses" (that class
  scores 42/42 hints-on today).
- `engine_v2.rs:373-374, :835-836` — hints-on strict quoted as 0.8174 and
  0.8031; it is 0.8285.
- `engine_v2.rs:369-371` — "hints match 194 of 299 characters", present tense,
  pre-escape; the sibling test labels its equivalent figures "before the
  escape".
- `examples/derivation_probe.rs`, `freq_counts.rs`, `ipadic_cost_probe.rs`,
  `common_inversion_probe.rs`, `reading_identity.rs`, `oracle_ceiling.rs` —
  every probe quotes pre-`FUNCTION_STACK` cells, and two compute `is_allowed`
  against the *raw* boundary set while calling `parse`, which now relaxes it
  internally. `oracle_ceiling.rs:294` has a `usize` subtraction the escape can
  now invert.
- `examples/order_sweep.rs` — I wrote it before implementing the change, so it
  labels variant 0 "what ships" when variant 9 ships, and says `order` has no
  context when it takes one. Its two "rejected near miss" figures are also not
  reproducible from the binary as written.
- `src-tauri` and `e2e` — the **1% vs 2.7% flash rate**. Four comments written
  by the same commit put a real-hover-path *before* (39%) next to a
  pinned-geometry-lab *after* (~1%) as if one experiment, while
  `panes.spec.ts` carries the 2.7% the commit body calls the residual. One is
  wrong in context wherever it appears.

### 4.3 Dangling or wrong references

- `e2e/panes.spec.ts:169` — `tools/probe-strip.mjs` was deleted in `9330ff8`.
- `e2e/panes.spec.ts:283` — "Ruling 11" resolves to an unrelated note.
- `e2e/stub.ts:80-82, :70-71` — "the one test" (six cases) and "their
  baselines are the darwin ones" (half are win32).
- `vite.config.ts:21` — "Two pages"; three inputs are declared below it.
- `src/main-tooltip.test.ts` — `outerPosition`/`monitorFromPoint`/
  `tooltipCentre` name things `placeFor` does not use or that were renamed.
- `src/main.ts:121` — "2E adds the download"; it was 2F.
- `src/main.test.ts:8-10, :144-145` — `set_input` is invoked nowhere;
  `commands::reparse` moved to `commands::dictionary`.
- **Seven rotted line-number citations** across `main.rs`, `layout.rs` and
  `layout/geometry.rs`, each pointing at an unrelated line.
- `crates/jparser/src/lib.rs:5` — "`parse` is the whole public surface"; five
  more modules are exported.
- `label.rs:17-19` — "every branch covered"; five branches are not.
- `bench.rs:49-50` — `LABEL_WIDTH` "matching `score`'s own reports"; 18 vs 16.
- `fixtures/README.md` — two consumer lists omit real consumers;
  `kanjidic_mini.xml` is credited to `src-tauri` tests that use their own
  fixture.
- `jmdict-source/src/lib.rs:541` cites a "Ruling G" that exists nowhere;
  `index/mod.rs:499` cites an unresolvable "R1".

### 4.4 The `ponytail:` markers

Two of three are accurate (`lattice.rs:51`, `fonts.rs:47`). The third,
`commands.rs:536`, is **stale in three ways**: its trigger no longer exists on
Windows, one of its proposed "structural fixes" is the design the module now
implements, and the `--color-bg` mitigation it cites unconditionally is gated
behind `BACKGROUND_OPAQUE`.

### 4.5 README

- "JMDict **multilingual**" — the downloader is pinned to `JMdict_e.gz`,
  English only.
- "`jmdict-source/ # … and indexing pipeline`" — it does no indexing; that is
  `crates/jparser/src/index/`.
- "**Node.js:** v18+" — nothing installs on 18; vite needs ≥20.19, CI pins 26,
  and `package.json` has no `engines` field.
- "*Causative-Passive*" — the engine emits `"Causative Passive/Potential"`.
- The architecture tree omits `docs/`, `e2e/` and `tools/`.
- "`cargo test --workspace`" is unqualified and fails on the macOS the README
  claims to support, because eight tests need `TA_HINTS_DICT` or `%APPDATA%`.
- "Automatically downloads" contradicts line 71, which says the user clicks a
  button.
- "a rewrite **of** the classic JParser engine" vs "clean-room implementation"
  twelve lines later — worth tightening, since it is the licence-facing line.

## 5. What is trustworthy

Recorded so the audit is not read as "nothing here can be believed".

- **Master plan §5.2's four-cell bench table survives 219 commits.** Measured
  today: k=1 hints-on reproduces to the millisecond (24 ms / 263 ms / 1.09x);
  all four scaling cells are inside the stated ±0.1x.
- Every Cargo feature claim in the tree checks out, and every `rust-version`
  rationale except `src-tauri`'s own floor.
- `score.rs`'s metric prose matches its code throughout — strict, tolerant,
  coarsening, the compound metric's exclusions, and entry top-1's
  conditioning. This matters, because a phase was misread this week by
  trusting prose over code, and here the prose is right.
- Every fixture count in `gold.jsonl`, `compound_gold.jsonl`,
  `spurious_merges.txt`, `conjugation_labels.txt`, `parse_sentences.txt` and
  the three XML subsets is exact, as is every `ent_seq` cited anywhere.
- Every Tauri command, event name, setting key, CSS selector and custom
  property named in any comment exists. Every cited commit hash resolves with
  a matching subject. `tauri.conf.json` agrees with everything asserted about
  it.
- `engine/relax.rs`'s figures — the freshest file in the tree — check against
  Phase 4 design §9 exactly, including the 189 = 47 + 142 decomposition.
- No `#[ignore]` exists anywhere in `crates/`; the workspace's single ignored
  item is a doctest.

## 6. One more finding, from the slice only a build could check

**The hints-on bench band is stale.** Phases 3b, 3c and 4 all state the gate as
`hints off 0.98x–1.07x, hints on 1.07x–1.12x`. Four readings of the hints-on
k=3 cell today: **1.07x, 1.02x, 1.06x, 1.06x** — at or below the floor every
time. The direction is safe, so no gate should have failed, but the band is
written as the expected range and no longer describes the engine. Phase 4's
narrowing relaxes 47 tokens where the unconditional escape relaxed 189, so the
lattice carries fewer allowed spans than when the band was set. **Re-derive it
rather than carrying it forward**, or a future phase passes a floor it has
silently outgrown.
