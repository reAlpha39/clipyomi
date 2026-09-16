# Engine v2 Phase 2c — candidate generation walks the index once per position

**Date:** 2026-09-09
**Branch:** `feat/engine-v2`
**Finding this closes:** [`2026-09-09-candidate-generation-blowup.md`](../2026-09-09-candidate-generation-blowup.md)
**Prior:** [master plan](2026-08-27-engine-v2-master-plan.md) §7 · [Phase 2b design](2026-09-09-engine-v2-phase2b-design.md)

## 1. Scope

`lex::candidates` is superlinear in input length: it visits every one of the
`O(n²)` `(start, end)` span pairs, and for each builds a stem `String` of up
to `n` characters and hands it to the FST. A 10,000-character paste — the
app's own clipboard ceiling — did not finish in 600 seconds at k = 1. The
blowup document has the measurements and the derivation.

**In.** One FST walk per start position instead of one per `(start, end)`
pair. The walk terminates where the index has no matching transition, so the
dictionary's own content bounds the span. `Index` gains a resumable walk;
`prefixes_of_with` is reimplemented on it and keeps its signature and its
behaviour. `candidates` is restructured around it. The candidate set is
unchanged, span for span, record for record, and in the same order.

**Out.** The cost model, `is_allowed`, the lattice, `demote`, the index
format, the wire shape, `ParseOptions`, and every public signature in
`jparser` except the one new `Index` method. No new dependency. No change to
what the engine *decides* — only to how long it takes to decide it.

**Explicitly not a bound.** The blowup document's §5 listed four ways to
source a constant span bound. This design takes none of them: a constant is
correct for today's JMdict by four characters of margin and silently drops a
candidate if a future release adds a 53-character reading, and every option
that sources the constant honestly (an FST scan at open, an index header
field) is a mechanism this restructure deletes rather than needs. The bound
survives only as §9's fallback if the gate fails.

## 2. Why walking once is the fix, not merely a faster loop

`Index::prefixes_of_with` (`index/load.rs:151`) is already a forward FST
walk: it folds one character at a time, follows a byte transition per
character, and **returns early the moment `find_input` fails**. It returns
every dictionary key that is a prefix of the string it was given, tagged with
`key_chars`.

`candidates` calls it once per `(start, end, chain hit)` with a freshly built
key, then discards every prefix hit except the one whose length matches the
key exactly (`lex.rs:124`). So for a single `start`, the identical walk over
`chars[start..]` is recomputed for every `end` — and every prefix hit that
walk finds along the way, which is exactly the information the outer loop is
enumerating span-by-span to rediscover, is thrown away.

That is the whole defect. The missing span bound is a symptom of it: with the
walk hoisted out, no bound is needed, because the walk's own dead end is the
bound and it is tighter than any global constant — typically a few characters
rather than 49.

## 3. The change

### 3.1 `Index`: a resumable walk

```rust
/// A position in the key FST: the node reached by some folded prefix, and
/// the output accumulated to it.
pub struct Walk<'a> { /* &'a Index, node, output */ }

impl Index {
    /// A walk at the FST root, before any character.
    pub fn walk(&self) -> Walk<'_>;
}

impl<'a> Walk<'a> {
    /// Advance over `ch`, folded. `false` if the index has no such
    /// transition, in which case the walk is spent and no longer position
    /// of any key -- the caller stops extending.
    pub fn push(&mut self, ch: char, fold: fn(char) -> char) -> bool;
    /// The records stored at exactly this position, or `None` when no key
    /// ends here.
    pub fn records(&self) -> Result<Option<Vec<StoredRecord>>, IndexError>;
    /// The records at this position extended by `suffix`, without moving
    /// this walk. `None` when the extension leaves the index or no key ends
    /// there.
    pub fn records_after(&self, suffix: &str, fold: fn(char) -> char)
        -> Result<Option<Vec<StoredRecord>>, IndexError>;
}
```

`fold` stays a parameter rather than becoming `kana::fold` internally,
because that is what `prefixes_of_with` does today and this design does not
relitigate it.

`prefixes_of_with` is rewritten as a loop over `walk.push` collecting
`walk.records()` at each depth. Its signature, its return type, its
`key_chars` semantics and its early return are unchanged — it has callers in
`jparser-cli`, `index_roundtrip.rs` and `generations_torn_build.rs`, and none
of them are touched.

### 3.2 `candidates`: chains reindexed by where the stem ends

Today `chains_by_end[end - 1]` holds the chains *ending* at `end`, each with
`consumed`; the loop derives `stem_end = end - hit.consumed` per pair. A walk
from `start` arrives at stem ends in ascending order, so the same data is
needed the other way round:

```rust
// chains_by_stem_end[m] holds, for every chain whose stem ends at m, the
// span end it implies and its index within `chains_by_end[end - 1]`.
// Built once, by inverting `chains_by_end`; `end = m + consumed`.
let chains_by_stem_end: Vec<Vec<(usize, usize, &ChainHit)>>;
```

Then, per `start`:

1. `let mut walk = index.walk();`
2. For `m` from `start` upward, while the walk is alive:
   - for each `(end, chain_index, hit)` in `chains_by_stem_end[m]`:
     - the key is `chars[start..m]` plus `CLASSES[class].remove` for
       `hit.stem_class`, or `chars[start..m]` alone for `None`;
     - the records are `walk.records()` for `None`, `walk.records_after(remove, fold)`
       otherwise — no `String` is built to ask the question;
     - for each record, `allows_chain` gates it exactly as today, and a
       `Candidate` is collected with `len: end - start`;
   - then `walk.push(chars[m], fold)`; stop if it returns `false` or `m` reaches
     `chars.len()`.
3. Emit the collected candidates for this `start` in the order §3.3 requires,
   then `dedup_shallowest` as today.

**One guard the span loop gave for free.** Today's outer loop is
`for end in (start + 1..=len).rev()`, so a span end equal to `start` is
never considered. Indexed by stem end, the identity chain hit
(`consumed: 0`) at `stem_end == start` implies `end == start`, which would
emit a zero-length candidate. The restructure therefore skips any
`end <= start` explicitly. It is the only invariant the loop shape was
carrying implicitly, and it is worth a test of its own.

**The key string is built only for records that hit**, because `exact:
record.surface == key` needs it. That is the second allocation the current
loop pays per pair and this one pays per hit.

### 3.3 Order is preserved exactly, and it is load-bearing

Today's outer loop runs `for end in (start + 1..=len).rev()`, so a `start`'s
candidates are pushed longest span first, and within one `end` in the order
`chains_by_end[end - 1]` holds. `dedup_shallowest`'s doc comment states that
it never disturbs the relative order between different keys and that the
longest-span-first guarantee depends on it; `assemble::order` and the DP's
tie-breaks read the result.

A walk visits stem ends ascending, and span length is
`(m - start) + consumed`, so walk order is neither longest-first nor
end-ordered. Therefore candidates are **collected** per `start` as
`(end, chain_index, Candidate)` and emitted sorted by `end` descending,
`chain_index` ascending, with a stable sort so record order within one chain
hit survives. That reproduces today's push order term for term — which is
why §5's gate can demand byte-identical output rather than merely equivalent
output.

## 4. Complexity

Today: `O(n²)` pairs, each `O(n)` to build a stem plus a walk that dies after
a few characters — the stem build dominates and the measured exponent is
about `n^2.6`.

After: one walk per start, each advancing until the index has no transition —
`O(n · d)` where `d` is the mean live walk depth, a property of the
dictionary and the text and not of the input's length. Per stem end, work is
proportional to the chains ending there, which `MAX_CHAIN` already bounds at
12. Linear in `n` with a dictionary-shaped constant.

## 5. Done when

1. **The candidate set is byte-identical.** The eight Phase 1b-i cells at
   `--readings 1`, both hint states, and the Phase 2b handoff §2's recorded
   `oracle@3` figures at `--readings 3`, both hint states — hints on, strict
   1,867, tolerant 1,908, sentences 23. `alternatives`'s order depends on the
   candidate set, so the oracle figures are part of this gate and not
   context beside it.
2. **A 10,000-character parse completes in under 30 seconds** on the full
   218,449-entry index, hints off, release build — against "did not finish in
   600" today. 30 rather than the 15 the experimental bound reached, because
   this design is not obliged to beat that number, only to remove the
   blowup; the handoff records what it actually reaches.
3. **The measured curve is linear to within a factor of two** across 500,
   1,000, 2,000, 5,000 and 10,000 characters — the table goes in the handoff
   beside today's, which is the finding's own evidence.
4. **A long-input canary lands in the test suite.** `engine_v2.rs` parses a
   synthetic input of a few thousand characters and asserts the cover is
   contiguous. It asserts no timing — a wall-clock assertion is flaky — but
   if the blowup returns, the suite stops finishing, which is a loud failure
   and is the signal that was missing. §7 of the blowup document is explicit
   that one long-input timing anywhere in the gates would have caught this.
5. **Standing gates:** `cargo test -p jparser`, the same with `--features
   mecab`, `cargo clippy -p jparser --all-targets -- -D warnings` in both
   feature states, `cargo check -p jparser --no-default-features`, `cargo fmt
   -p jparser -- --check`, `npm test`, `npx tsc --noEmit`, `cargo test
   --manifest-path src-tauri/Cargo.toml`.

## 6. Tests

- **`index_roundtrip`**: every existing `prefixes_of_with` assertion passes
  unchanged through the reimplementation — that is the regression net for
  §3.1. Plus: a walk that dies mid-key reports `false` and yields no records;
  `records_after` on a live walk finds a key the walk itself would not reach;
  `records_after` does not move the walk it was called on.
- **`lex`**: a conjugated form still resolves to its dictionary headword
  through the new path (`書いた` → `書く`); a span whose stem leaves the index
  early contributes no candidate; the longest-span-first order holds for a
  start with candidates at several ends, asserted on the emitted order rather
  than inferred from `dedup_shallowest`'s output.
- **`engine_v2`**: the canary of §5 clause 4; every existing integration
  assertion unchanged.

## 7. Budget

`index/load.rs` +70 on 234 (the `Walk` type and its three methods,
`prefixes_of_with` shrinking as it delegates). `lex.rs` net about +30 on 234
— the reindex and the collect-and-emit are new, the `(start, end)` loop and
its per-pair `String` go. Tests about +120. No new file, no new dependency,
one new public method on `Index`.

## 8. Master plan edits this design owes

- **§7, a new Phase 2c entry** between Phase 2 and Phase 3: what it is, that
  it changes no decision the engine makes, and its measured before and after.
- **§5.2**, a sentence beside the metric definitions: the harness measures
  quality at corpus sentence lengths and measured nothing at clipboard
  lengths, which is why a defect this large survived four phases.

## 9. Fallback

If clause 1 fails — the restructure changes the candidate set somewhere this
design has not foreseen — the fallback is the blowup document's §5 option 2:
leave `candidates` as it is and bound its `end` loop at the longest key
length, scanned once from the FST at `Index::open`. That is a much smaller
change, it was measured at a 10× improvement with the eight cells
byte-identical, and it keeps the correctness question to one number instead
of a restructured loop. It is second choice because it leaves the redundant
per-pair walking in place and needs a mechanism this design does not.

## 10. Decision log

| Decision | Rationale |
| --- | --- |
| Walk once per start, no span bound | The walk's dead end bounds the span tighter than any constant and needs no mechanism to source it. The bound was the symptom's fix; the redundant walk is the defect (§2) |
| `Index` gains a walk; `prefixes_of_with` is rewritten on it | It is already this walk with the results collected. Reimplementing it keeps one FST traversal in the codebase instead of two, and its existing tests become the regression net (§3.1) |
| Reindex chains by stem end | A walk arrives at stem ends ascending; the current index is by span end because the current loop enumerates span ends. Inverting it once per parse is cheaper than either loop shape's alternative (§3.2) |
| Collect and emit in end-descending order | `dedup_shallowest` documents a dependence on longest-span-first, and `assemble::order` and the DP's tie-breaks read the result. Preserving it exactly is what lets the gate demand byte-identical output (§3.3) |
| Skip `end <= start` explicitly | The `(start + 1..=len)` loop bound carried it implicitly; indexed by stem end, the identity chain at `stem_end == start` would emit a zero-length candidate (§3.2) |
| The key `String` is built only for records that hit | `exact` needs it; asking the index whether anything is there does not. This is the allocation the current loop pays `O(n²)` times (§3.2) |
| The oracle@3 figures join the gate | `alternatives`'s order is a function of the candidate set, so a change here can move them while leaving top-1 alone. Phase 2b's numbers are the reference (§5 clause 1) |
| A canary, not a timing assertion | A wall-clock threshold in a test suite is flaky on shared hardware. An input long enough that a return of the blowup stops the suite finishing is a loud, non-flaky signal (§5 clause 4) |
| The bound is the fallback, not the plan | Correct today by four characters of margin, silent when wrong, and every honest way to source it is a mechanism the restructure deletes (§1, §9) |
