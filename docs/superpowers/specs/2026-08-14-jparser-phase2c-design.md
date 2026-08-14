# JParser Phase 2C — MeCab Boundary Hints — Design

## 1. Scope

Phase 2C gives `jparser` a real producer for the `BoundaryHints` trait, backed by
[Vibrato](https://github.com/daac-tools/vibrato) — a Rust reimplementation of
MeCab's Viterbi tokenizer. Port design §5.7.

The consumer side already exists and is tested: `segment.rs` declares the trait,
weights it into the DP cost with `MECAB_BAD_START`/`MECAB_BAD_END` (both 10), and
covers it with an `AlwaysBad` test double. `parse` already takes
`hints: Option<&dyn BoundaryHints>`. **Nothing implements the trait.** This phase
supplies that, and nothing else.

### In scope

- Load a compiled Vibrato dictionary from a path.
- Tokenize text and derive boundary flags, porting ta-old's derivation.
- A `--hints <dict>` flag on `jparser-cli parse`.
- Raise the workspace MSRV from 1.75 to 1.85 (§7).

### Out of scope, deliberately

- **Downloading the dictionary.** Port design §4.4 says it is fetched on demand
  when the MeCab toggle is first enabled. There is no toggle — the app shell
  (port design §6) is unbuilt — so the trigger has nothing to hang on. Building
  acquisition now costs TLS, xz, and tar (§9 below) for an asset whose trigger is
  deferred anyway. The app shell needs an HTTP stack regardless; acquisition
  lands there, once.
- **Decompressing the distributed archive.** The user supplies an uncompressed
  compiled dictionary. See §5 below.
- **A persisted enable/disable toggle**, which belongs with settings and
  persistence (port design §8).

> Section references are to **this document** unless marked "port design". The two
> numbering schemes overlap, and §6/§8 mean different things in each.

---

## 2. Why the implementation lives inside `jparser`

A separate `vibrato-hints` crate was the first choice and **does not compile.**
`BoundaryHints` is `jparser`'s trait, so any implementor must depend on
`jparser`; but `jparser-cli` lives inside `crates/jparser`, so `jparser` would
have to depend back on that crate. Cargo rejects package-level dependency cycles,
and features cannot break one — resolution is per package.

`jmdict-source` avoids this only because its edge runs one way: `jparser →
jmdict-source`, never back. Its `jparser` dev-dependency in `seam.rs` is legal
precisely because dev-dependency cycles are exempt; a normal dependency is not.

So the implementation lives in `crates/jparser/src/hints.rs`, behind an optional
`mecab` feature:

```toml
[features]
default = ["cli"]
cli = ["dep:jmdict-source"]
mecab = ["dep:vibrato"]

[dependencies]
vibrato = { version = "0.5", default-features = false, optional = true }
```

`default-features = false` drops vibrato's default `train` feature and the
`rucrf` training stack with it — nothing here trains a model.

The alternative that would also work is moving `jparser-cli` into its own crate,
freeing a separate hints crate to depend on `jparser`. That is a refactor 2C
should not pay for, and it is available later if the CLI grows.

### The purity gate generalizes

Phase 2B made "the parser library gains no HTTP client and no decompressor" a
compile-checked property. The same gate covers this phase unchanged in shape,
with one more name:

```bash
cargo check -p jparser --no-default-features --all-targets
cargo tree -p jparser --no-default-features | grep -cE "jmdict-source|ureq|flate2|vibrato"   # 0
```

`mecab` is **not** in `default`. A caller wanting hints opts in.

---

## 3. The derivation

Ported from ta-old `exe/util/Dictionary.cpp:1115-1126`. For each token, using
`token.range_char()` — char units, matching `BoundaryHints`'s indexing with no
conversion:

```text
skip the token unless feature field 7 exists and is neither "*" nor empty
for i in 0 .. (len - 1):
    bad_end  [start + i]     = true
    bad_start[start + i + 1] = true
```

Two `Vec<bool>` sized to the input's char count. Out-of-range queries return
`false`.

**Interior positions only.** A word should not *end* before the token's last
char, nor *start* after its first. Token boundaries themselves stay free, which
is the whole point: the hint says "do not split inside this token," not "split
here." A single-char token marks nothing — the loop is empty at `len == 1`.

**Field 7 is IPADIC's reading.** ta-old's comment is "If katakana is '*' or does
not exist, not real word, so don't penalize" — an unknown-word token carries no
reading, and penalizing splits inside a guess would be worse than staying silent.
This is why the phase targets IPADIC: the guard ports literally. UniDic's feature
layout differs, and re-deriving the field index is a silent-failure risk — the
wrong index suppresses flagging everywhere, or nowhere, and both look plausible.

### One deliberate departure

ta-old carries a second guard: after walking the source to match the token's
surface, it re-checks with a fuzzy comparison (`wcsnijcmp`) and skips the token
when it fails, commented *"I don't trust mecab all that much."*

**Not ported.** That guard exists because ta-old drove MeCab through a text pipe
and had to re-find each token in the source string by scanning. Vibrato returns
char ranges into the exact string it was handed, so alignment holds by
construction. Porting the check would mean writing a defense against a state that
cannot occur, and the dead branch would be untestable — the same category of
finding that got an unreachable guard deleted in Phase 2B.

Recorded here so it reads as a decision rather than an oversight.

---

## 4. Module surface

`crates/jparser/src/hints.rs`, gated `#[cfg(feature = "mecab")]`:

```rust
/// A loaded Vibrato dictionary. Loading is expensive; loading is separate from
/// use so a caller pays once and reuses across parses.
pub struct VibratoTokenizer { /* dict + tokenizer */ }

impl VibratoTokenizer {
    pub fn load(dict: &Path) -> Result<Self, HintsError>;
    /// Tokenize `text` and derive its boundary flags.
    pub fn hints(&self, text: &str) -> BoundaryFlags;
}

/// Flags for one text. Cheap, owns two bit vectors, implements the trait.
pub struct BoundaryFlags { /* bad_start, bad_end */ }
impl BoundaryHints for BoundaryFlags { .. }

#[derive(Debug, thiserror::Error)]
pub enum HintsError {
    #[error("reading the vibrato dictionary failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("the vibrato dictionary could not be loaded: {0}")]
    Dictionary(String),
}
```

> **Amended after implementation (2026-08-14).** `HintsError::Io`'s shape
> above cannot deliver what §10's testing table implicitly wants — a `File::
> open` `io::Error` carries no path, so a tuple variant can never name the
> file it could not load. Shipped as `Io { path: PathBuf, source: std::io::
> Error }`, rendered by `thiserror`'s bare `{path}` shorthand for `PathBuf`
> fields (its private `AsDisplay` trait), the same pattern already used by
> `IndexError::GenerationExists` and `SourceError::Http` elsewhere in this
> codebase.

The split matters: a `BoundaryHints` implementor is queried by position for **one
text**, so per-text state cannot live on the tokenizer without making it
single-use or stateful. `hints(text) -> BoundaryFlags` keeps the expensive object
immutable and shared.

`Dictionary(String)` renders vibrato's error rather than wrapping its type, so
`vibrato` stays out of `jparser`'s public API — the same reason
`SourceError::Transport` holds a `String` instead of a `ureq` error. A
feature-gated public type that leaked a dependency's error would make that
dependency part of the API for anyone matching on it.

Vibrato's worker is created per call and not held across calls; workers are
mutable scratch space, and sharing one would make `hints` require `&mut self` for
no benefit.

---

## 5. Dictionary format, and what the user must supply

`--hints <path>` takes an **uncompressed compiled** Vibrato dictionary.

The distributed artifact is a `.tar.xz` containing a zstd-compressed
`system.dic`. Accepting it directly would add xz, tar, and zstd to `jparser` for
a path that exists only until the app shell (port design §6) lands acquisition. Instead the CLI documents the
commands that produce the file, and 2C's entire new dependency surface is
`vibrato`.

This is the phase's main ergonomic cost, and it is accepted: 2C's deliverable is
the derivation and its proof, not another downloader. Phase 2B already
established the acquisition shape, and the app shell will reuse it against an HTTPS host
with an archive format that needs its own decompressors.

> **Amended after implementation (2026-08-14).** "The CLI documents the
> commands" was aspirational when this section was written, not yet true:
> `--hints`'s `--help` text carried no download or extraction instructions
> until the phase's final commit (`3f761f8`, review finding I2). Before that,
> a user pointed at `--hints` with no dictionary in hand had nowhere in the
> binary itself to learn what to fetch or how to unpack it.

---

## 6. CLI

```text
jparser-cli parse <text> [--hints <dict>]
```

Absent, `parse` is called with `None` exactly as today — no behavior change, and
1B/2A's committed CLI tests keep passing untouched. Present, the dictionary is
loaded once, `hints(text)` produces the flags, and they are passed as
`Some(&flags)`.

The flag requires the `mecab` feature. `jparser-cli` already carries
`required-features = ["cli"]`; the flag is additionally gated so a build without
`mecab` does not advertise an option it cannot honor.

---

## 7. The MSRV change: 1.75 → 1.85

**This phase raises the workspace floor.** Measured (§9): vibrato 0.5.0 is the
newest release that builds on 1.75, and only via `bincode 2.0.0-rc.2` — a release
candidate — while 0.5.1 and 0.5.2 require `bincode 2.0.1`, whose floor is 1.85.

Holding 1.75 would mean pinning `vibrato = "=0.5.0"` and carrying an RC
dependency transitively. That would be the **second** pin in as many days: Phase
2B's follow-up already pinned `clap = "=4.5.51"` for the same reason. Each pin is
individually defensible; the pattern is the signal.

1.85 is chosen as the **lowest floor the dependency tree actually supports**, not
as "latest" — the same discipline as before, applied to a number that is now
true. Nothing consumes `jparser` from crates.io; it is a desktop application
port, so the floor buys no external compatibility and costs two pins.

Consequences, all of which this phase must carry out:

- `rust-version = "1.85"` in the root `[workspace.package]`.
- **Revert `clap` to `"4"`.** Its pin exists only to hold 1.75.
- CI's `msrv` job matrix moves to `"1.85"`.
- The Phase 2B spec's §9 and §10 amendments and the 2B handoff both assert 1.75
  and compile-verification against it. Both need a short amendment noting 2C
  raised the floor, in the same house style — a spec that silently disagrees with
  the tree is what §10 was amended about in the first place.
- `resolver = "2"` stays. It is unrelated, and Cargo 1.85 accepting `"3"` does not
  make the change worth its blast radius here.

The MSRV gate remains `cargo +<floor> check --workspace`, which is what makes the
floor real rather than declared.

---

## 8. Error handling and failure behavior

| Failure | Result |
|---|---|
| `--hints` path missing or unreadable | `HintsError::Io`, CLI exits non-zero |
| Path is not a valid compiled dictionary | `HintsError::Dictionary`, CLI exits non-zero |
| Dictionary loads, text tokenizes to nothing | Empty flags; `parse` behaves exactly as with `None` |
| Token whose feature field 7 is `*` | Contributes no flags (§3) |

Loading failure is **fatal to the invocation, not silently degraded.** A user who
passed `--hints` asked for hints; falling back to unhinted parsing would produce a
plausible-looking result that quietly is not what was requested. This mirrors 2B's
stance that a failure the user can act on must say so.

Tokenization after a successful load is infallible in this design: vibrato's
worker does not fail on arbitrary text, and unknown input becomes unknown-word
tokens, which §3's field-7 guard already discards.

---

## 9. Resolved facts

Measured 2026-08-14 against the live registry, the live host, and a compile probe.
Recorded so the plan does not re-derive them.

**1. The `vibrato` crate.**

| Fact | Value |
|---|---|
| Latest | 0.5.2 |
| License | MIT OR Apache-2.0 (GPL-v2 compatible) |
| Declared `rust-version` | 1.65 — **misleading**, see below |
| Default features | `["train"]` → pulls `rucrf`; use `default-features = false` |

**2. The real floor is set transitively, not by vibrato's own `rust-version`.**

| vibrato | bincode | `cargo +1.75 check` |
|---|---|---|
| 0.5.0 | 2.0.0-rc.2 | **passes** |
| 0.5.1 | 2.0.1 | fails — requires rustc 1.85 |
| 0.5.2 | 2.0.1 | fails — requires rustc 1.85 |

Note `jparser` already depends on `bincode 1.3`; vibrato brings the 2.x major, so
the tree carries both. Cargo permits this.

**3. The API, verified to compile** (not read from docs):

```rust
SystemDictionaryBuilder::from_readers(lexicon, matrix, char_def, unk_def) -> Result<Dictionary>
Tokenizer::new(dict); tokenizer.new_worker();
worker.reset_sentence(text); worker.tokenize(); worker.num_tokens();
worker.token(i).feature() -> &str
worker.token(i).range_char() -> Range<usize>   // char units
worker.token(i).range_byte() -> Range<usize>
```

`SystemDictionaryBuilder` is available **without** the `train` feature, which is
what makes §10's test strategy possible.

**4. The IPADIC artifact**, recorded for the app shell's benefit rather than this phase's:

| Fact | Value |
|---|---|
| URL | `https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz` |
| Size | 7,704,680 bytes |
| Transport | **HTTPS only** — unlike EDRDG, TLS works |
| Alternatives | `.tar.zst` and `.tar.gz` both 404 |

**5. The seam this phase fills**, in `crates/jparser/src/segment.rs`:

```rust
pub trait BoundaryHints {
    fn bad_start(&self, pos: usize) -> bool;
    fn bad_end(&self, pos: usize) -> bool;
}
const MECAB_BAD_START: i32 = 10;   // ta-old Dictionary.cpp:1181
const MECAB_BAD_END: i32 = 10;     // ta-old Dictionary.cpp:1183
```

---

## 10. Testing

**No binary fixture and no network.** Tests build a tiny system dictionary in
memory from CSV string literals via `SystemDictionaryBuilder::from_readers`, which
§9 verified is reachable without the `train` feature. This mirrors Phase 2B, which
gzips its XML inside the test rather than committing an archive.

Required assertions:

| Behavior | Why it needs a test |
|---|---|
| Interior positions of a multi-char token are flagged; its first `bad_start` and last `bad_end` are **not** | This is the entire derivation. Flagging the boundaries would invert the meaning and forbid the split the tokenizer actually proposed |
| A single-char token flags nothing | The `len - 1` loop is empty; an off-by-one here silently penalizes every one-char token |
| A token whose feature field 7 is `*` contributes no flags | ta-old's unknown-word guard; without it, splits inside a guessed word get penalized |
| Empty input yields empty flags and no panic | `Vec<bool>` sizing off a char count |
| Out-of-range positions return `false` | `segment.rs` queries `m.start + m.len - 1`; a panic here would be a crash in the DP |
| **A sentence whose segmentation changes when hints are supplied** | The phase's only end-to-end proof. Everything above tests the derivation in isolation; this is the one assertion that shows hints reach the DP and alter its chosen cover |
| `parse(.., None)` is byte-identical to today on the same input | The feature must be additive; 1B's snapshots are the guard |

The sixth row is the load-bearing one, and it is the phase's risk: it requires
finding an input where the DP's chosen cover actually differs. **If no such case
can be constructed, the phase has demonstrated nothing** — the derivation would be
correct in isolation and inert in practice. The implementation plan must treat
failure to produce that case as a stop-and-report, not as a test to weaken.

---

## 11. Constraints inherited

From the Phase 2B handoff and the crate's standing rules:

- **GPL v2.** Every new source file carries the standard header, verbatim from
  `crates/jparser/src/index/mod.rs:1-6`. No dependency may link
  `native-tls`/OpenSSL. `vibrato` is MIT OR Apache-2.0 (§9).
- **`crates/jparser`'s library stays pure.** `vibrato` is optional and off by
  default; the §2 gate proves it.
- **Errors are explicit.** No `unwrap()`/`expect()`/`unreachable!()` in library
  code outside `#[cfg(test)]`. Never swallow an error without a comment naming the
  reason.
- **No magic numbers**, no bare literals for names that have constants. Feature
  field index 7 gets a named constant with ta-old's line cited.
- **File size** 200-400 lines typical, 800 hard maximum including tests.
  `hints.rs` is new; `segment.rs` is at 778/800 and **must not be edited**.
- **Formatting:** `rustfmt --edition 2021 <individual files>`. Never `cargo fmt` —
  `conjugation.rs` is deliberately not rustfmt-clean and "fixing" it is a defect.
  CI has no formatting job for this reason.
- **Clippy:** `cargo clippy --workspace --all-targets -- -D warnings` clean.
- **Coverage:** 80% lines minimum, enforced in CI.
- **CI must stay green**, including the `msrv` job at its new 1.85 value and the
  purity job with `vibrato` added to its grep.

**2A/2B invariants this phase must not break:** `INDEX_FORMAT_VERSION` stays 3;
`EntryData`'s field order is wire format; a published `gen-N` is immutable;
directory knowledge lives only in `generations.rs` and `ensure_dictionary`; the
staging filename stays process-unique; a `.partial` file is never resolved. This
phase touches none of them — it produces boundary flags and knows nothing about
dictionaries on disk beyond the one path it is handed.
