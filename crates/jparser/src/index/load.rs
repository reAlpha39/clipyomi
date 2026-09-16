// SPDX-License-Identifier: MIT OR Apache-2.0

//! Open an index and walk it.

use std::fs::File;
use std::path::Path;

use fst::raw::Fst;
use memmap2::Mmap;

use crate::index::{
    EntryData, IndexError, IndexHeader, KanjiData, StoredRecord, ENTRIES_FILE, ENTRIES_INDEX_FILE,
    FST_FILE, HEADER_FILE, INDEX_FORMAT_VERSION, KANJI_FILE, LEN_PREFIX_BYTES, RECORDS_FILE,
};

/// One dictionary key that is a prefix of the queried text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixHit {
    /// Length of the matched key **in characters** of the source text.
    pub key_chars: usize,
    pub records: Vec<StoredRecord>,
}

pub struct Index {
    fst: Fst<Mmap>,
    records: Mmap,
    entries: Mmap,
    entry_offsets: Vec<(u32, u64)>,
    /// Every KANJIDIC2 record, sorted by codepoint. Resident, not mapped:
    /// 13k small records earn neither an mmap nor an FST, and a whole-file
    /// read keeps `open`'s failure modes the ones it has.
    kanji: Vec<KanjiData>,
    header: IndexHeader,
}

// `fst::raw::Fst` does not implement `Debug`, so this is written by hand.
// Needed so `Result<Index, IndexError>::unwrap_err()` compiles in tests.
impl std::fmt::Debug for Index {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Index")
            .field("header", &self.header)
            .finish_non_exhaustive()
    }
}

fn map(path: &Path) -> Result<Mmap, IndexError> {
    let file = File::open(path)?;
    // SAFETY: memmap2 requires that the mapped file is not mutated or
    // truncated while the mapping is alive, or the mapping is undefined
    // behavior (a SIGBUS on truncation, or this type handing out a `&[u8]`
    // that is presented as immutable but is actually being rewritten
    // underneath the reader). This crate does not enforce that on its own:
    // `build::build_from_reader` writes every index file in place via
    // `File::create`/`std::fs::write` (there is no build-to-temp-and-rename),
    // so nothing stops a rebuild from racing an `Index` that already has this
    // directory mapped. Soundness rests entirely on the caller obligation
    // documented on `Index::open`: no process may write to an index
    // directory while an `Index` for it is open.
    //
    // When a caller finally needs that obligation lifted, the fix is a fresh
    // directory per build — `<root>/.build-<nonce>` renamed to `<root>/gen-<N>`,
    // with readers taking the highest `gen-N` — and NOT a swap onto a live
    // path. Renaming over `dir`, flipping a symlink, and updating a `CURRENT`
    // pointer all look atomic and are all insufficient, because `Index::open`
    // is not atomic: it reads six files in sequence, so a reader that starts
    // mid-swap can splice one generation's `entries.idx` onto another's
    // `entries.bin` and then return well-formed wrong answers with no error.
    // A `gen-N` directory's contents never change after creation, so an open
    // that straddles a publish either succeeds wholly or fails with ENOENT.
    // Note also that `fs::rename` cannot replace a non-empty directory at all
    // (ENOTEMPTY), so the swap is not even expressible as a single operation.
    Ok(unsafe { Mmap::map(&file)? })
}

impl Index {
    /// Opens the index `build::build_from_reader` wrote to `dir`.
    ///
    /// # Caller obligation
    ///
    /// No process may write to `dir` for as long as the returned `Index`
    /// stays alive. The index files are memory-mapped, and the builder
    /// writes in place rather than building to a temp directory and
    /// renaming, so rebuilding into a directory with an open `Index` is
    /// undefined behavior, not just a race that yields stale reads.
    ///
    /// A rebuild interrupted partway is not detected either. The header is
    /// written last and only its version and conjugation fingerprint are
    /// validated here, and both still match after a torn rebuild, so the
    /// index opens cleanly and can then return wrong data with no error.
    /// Build into a fresh directory and point readers at it only once the
    /// build has completed; see the note on `map` above for why swapping a
    /// live path is not a substitute.
    ///
    /// Checks the index was built against the current conjugation table
    /// (`engine::build::fingerprint`); `open_with` takes the fingerprint
    /// explicitly, for tests.
    pub fn open(dir: &Path) -> Result<Self, IndexError> {
        Self::open_with(dir, crate::engine::build::fingerprint())
    }

    /// [`Index::open`] with the caller supplying the conjugation fingerprint it
    /// expects the index to have been built against.
    ///
    /// The guard is what makes reading a stale index impossible rather than
    /// silently wrong.
    pub fn open_with(dir: &Path, expected_fingerprint: u64) -> Result<Self, IndexError> {
        let header: IndexHeader = bincode::deserialize(&std::fs::read(dir.join(HEADER_FILE))?)?;
        if header.version != INDEX_FORMAT_VERSION {
            return Err(IndexError::VersionMismatch {
                found: header.version,
                expected: INDEX_FORMAT_VERSION,
            });
        }
        if header.conjugation_fingerprint != expected_fingerprint {
            return Err(IndexError::ConjugationMismatch {
                found: header.conjugation_fingerprint,
                expected: expected_fingerprint,
            });
        }
        let entry_offsets: Vec<(u32, u64)> =
            bincode::deserialize(&std::fs::read(dir.join(ENTRIES_INDEX_FILE))?)?;
        let kanji: Vec<KanjiData> = bincode::deserialize(&std::fs::read(dir.join(KANJI_FILE))?)?;
        Ok(Self {
            fst: Fst::new(map(&dir.join(FST_FILE))?)?,
            records: map(&dir.join(RECORDS_FILE))?,
            entries: map(&dir.join(ENTRIES_FILE))?,
            entry_offsets,
            kanji,
            header,
        })
    }

    pub fn header(&self) -> &IndexHeader {
        &self.header
    }

    /// Every dictionary key that is a prefix of `text`, shortest first, with
    /// the key normalization supplied by the caller.
    ///
    /// One forward walk, stopping where the index has no transition: a
    /// caller that wants the walk itself, to test several keys sharing a
    /// prefix without repeating it, wants [`Index::walk`] instead. This is
    /// that walk with every key it passes collected. Every node that is
    /// final marks a complete headword, and `""` is a prefix of every
    /// string, so the root node's finality is checked before consuming any
    /// character.
    ///
    /// That root check is unreachable by construction with today's builder,
    /// and is kept as a property of the walk rather than as a live guarantee:
    /// [`crate::engine::build`] stores headwords only, and no JMdict entry has
    /// an empty headword, so no index this crate can write has an empty key.
    /// A `key_chars == 0` hit was reachable under the v1 builder, which also
    /// indexed generated stems — one verb type stripped its whole surface and
    /// so keyed the empty string. Phase 1b-ii deleted that builder along with
    /// the branch's only test; the branch itself stays, because removing it
    /// would change the behaviour of a surviving public API on an index some
    /// later writer might legitimately produce.
    ///
    /// `fold` is a parameter rather than a constant because an index is built
    /// with the fold its builder used, and the walk has to agree with it.
    pub fn prefixes_of_with(
        &self,
        text: &str,
        fold: fn(char) -> char,
    ) -> Result<Vec<PrefixHit>, IndexError> {
        let mut walk = self.walk();
        let mut hits = Vec::new();
        if let Some(records) = walk.records()? {
            hits.push(PrefixHit {
                key_chars: 0,
                records,
            });
        }
        for (consumed, ch) in text.chars().enumerate() {
            if !walk.push(ch, fold) {
                return Ok(hits);
            }
            if let Some(records) = walk.records()? {
                hits.push(PrefixHit {
                    key_chars: consumed + 1,
                    records,
                });
            }
        }
        Ok(hits)
    }

    /// A walk at the FST root, before any character.
    pub fn walk(&self) -> Walk<'_> {
        Walk {
            index: self,
            node: self.fst.root(),
            output: 0,
            alive: true,
        }
    }

    fn records_at(&self, offset: u64) -> Result<Vec<StoredRecord>, IndexError> {
        Ok(bincode::deserialize(slice_at(&self.records, offset)?)?)
    }

    pub fn entry(&self, id: u32) -> Result<Option<EntryData>, IndexError> {
        let Ok(i) = self.entry_offsets.binary_search_by_key(&id, |(k, _)| *k) else {
            return Ok(None);
        };
        let (_, offset) = self.entry_offsets[i];
        Ok(Some(bincode::deserialize(slice_at(
            &self.entries,
            offset,
        )?)?))
    }

    /// Number of entries in the payload. Cheap: the offset table is already
    /// resident after `open`, so this touches no mmap page.
    pub fn entry_count(&self) -> usize {
        self.entry_offsets.len()
    }

    /// The KANJIDIC2 record for `ch`, or `None`: every non-kanji character,
    /// every character KANJIDIC2 does not carry, and every character of an
    /// index built through `build_from_reader`, which supplies no KANJIDIC2.
    pub fn kanji(&self, ch: char) -> Option<&KanjiData> {
        self.kanji
            .binary_search_by_key(&ch, |k| k.literal)
            .ok()
            .map(|i| &self.kanji[i])
    }

    /// Number of KANJIDIC2 records. Cheap: resident after `open`.
    pub fn kanji_count(&self) -> usize {
        self.kanji.len()
    }
}

/// A position in the key FST: the node some folded prefix of the input
/// reached, and the transducer output accumulated on the way to it.
///
/// Exists so one input position can be walked once rather than re-walked
/// for every span that starts there. `lex::candidates` used to build a key
/// per `(start, end)` pair and hand each to [`Index::prefixes_of_with`],
/// which walks from the root every time and discards every prefix it passes
/// but one; that made candidate generation superlinear in input length
/// (Phase 2c design §2).
#[derive(Clone, Copy)]
pub struct Walk<'a> {
    index: &'a Index,
    node: fst::raw::Node<'a>,
    output: u64,
    /// False once a `push` found no transition. A spent walk stays spent:
    /// no key has the prefix it stands on, so no longer prefix has it
    /// either, and a caller enumerating spans from one start stops.
    alive: bool,
}

impl<'a> Walk<'a> {
    /// Advance over `ch`, folded by `fold`. `false` when the index has no
    /// such transition, after which this walk is spent.
    pub fn push(&mut self, ch: char, fold: fn(char) -> char) -> bool {
        if !self.alive {
            return false;
        }
        let mut buf = [0u8; 4];
        for &byte in fold(ch).encode_utf8(&mut buf).as_bytes() {
            let Some(i) = self.node.find_input(byte) else {
                self.alive = false;
                return false;
            };
            let transition = self.node.transition(i);
            self.output += transition.out.value();
            self.node = self.index.fst.node(transition.addr);
        }
        true
    }

    /// The records of the key ending exactly where this walk stands, or
    /// `None` when no key ends here.
    ///
    /// The root counts: `""` is a prefix of every string, so a fresh walk
    /// standing on a final root reports a key. That is load-bearing rather
    /// than vestigial — see [`Index::prefixes_of_with`] for why the branch
    /// stays even though today's builder cannot produce such an index.
    pub fn records(&self) -> Result<Option<Vec<StoredRecord>>, IndexError> {
        if !self.alive || !self.node.is_final() {
            return Ok(None);
        }
        let offset = self.output + self.node.final_output().value();
        Ok(Some(self.index.records_at(offset)?))
    }

    /// The records of the key that ends `suffix` past where this walk
    /// stands, leaving this walk where it is. `None` when the extension
    /// leaves the index, or when no key ends there.
    ///
    /// This is what lets a deinflected key be tested without repeating the
    /// stem: the walk holds the stem, and `suffix` is the conjugation
    /// class's remove-suffix.
    pub fn records_after(
        &self,
        suffix: &str,
        fold: fn(char) -> char,
    ) -> Result<Option<Vec<StoredRecord>>, IndexError> {
        // `Walk` is `Copy` (every field is), so this is a cheap independent
        // position rather than a second traversal from the root.
        let mut probe = *self;
        for ch in suffix.chars() {
            if !probe.push(ch, fold) {
                return Ok(None);
            }
        }
        probe.records()
    }
}

/// Read a `u32`-length-prefixed blob at `offset`.
fn slice_at(blob: &[u8], offset: u64) -> Result<&[u8], IndexError> {
    let start = usize::try_from(offset).map_err(|_| corrupt("offset out of range"))?;
    let len_end = start
        .checked_add(LEN_PREFIX_BYTES)
        .ok_or_else(|| corrupt("offset overflows"))?;
    let prefix = blob
        .get(start..len_end)
        .ok_or_else(|| corrupt("length prefix past end of payload"))?;
    let len =
        u32::from_le_bytes(prefix.try_into().expect("checked LEN_PREFIX_BYTES bytes")) as usize;
    let end = len_end
        .checked_add(len)
        .ok_or_else(|| corrupt("length overflows"))?;
    blob.get(len_end..end)
        .ok_or_else(|| corrupt("blob past end of payload"))
}

fn corrupt(reason: &str) -> IndexError {
    IndexError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!("index payload corrupt: {reason}"),
    ))
}
