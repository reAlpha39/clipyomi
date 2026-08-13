// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Memory-mapped dictionary index.
//!
//! Replaces ta-old's hand-rolled sorted-array-plus-binary-search index
//! (`LoadDict`/`FindMatches`, `ta-old/exe/util/Dictionary.cpp`). Keys are
//! surfaces normalized with `kana::unify`, so the kana-insensitive comparator
//! ta-old threaded through three separate binary searches does not exist here.
//! The original surface is kept in the payload so inexact hiragana/katakana
//! matches can still be detected and penalized in Phase 1B.

pub mod build;
pub mod generations;
pub mod load;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::conjugation::ConjugationTable;
use crate::conjugation::VerbTypeId;
use crate::index::load::Index;
use crate::stem::StemOptions;
use crate::stem::StemStats;

/// Bumped whenever the on-disk layout changes. A mismatch forces a rebuild; the
/// loader must never try to read an index it does not recognize.
pub const INDEX_FORMAT_VERSION: u32 = 3;

pub const HEADER_FILE: &str = "header.bin";
pub const FST_FILE: &str = "keys.fst";
pub const RECORDS_FILE: &str = "records.bin";
pub const ENTRIES_FILE: &str = "entries.bin";
pub const ENTRIES_INDEX_FILE: &str = "entries.idx";

/// Byte width of the little-endian length prefix `build` writes ahead of
/// each bincode blob in `records.bin`/`entries.bin`, and `load` reads back.
/// Shared so the two sides cannot drift apart independently.
pub const LEN_PREFIX_BYTES: usize = std::mem::size_of::<u32>();

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexHeader {
    pub version: u32,
    pub keys: u32,
    pub records: u32,
    pub entries: u32,
    /// FNV-1a fingerprint of the conjugation asset this index's `verb_type`
    /// ids were assigned from (`conjugation::embedded_asset_fingerprint`).
    /// `Index::open` rejects a mismatch: reordering or adding a type in the
    /// asset changes every id downstream of the change, so an index built
    /// against a different asset would otherwise silently resolve
    /// `verb_type` to the wrong verb.
    pub conjugation_fingerprint: u64,
}

/// One headword or stem as stored in the payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredRecord {
    /// Original, unnormalized surface. Compared against the source text to set
    /// the inexact-match flag in Phase 1B.
    pub surface: String,
    pub flags: u16,
    /// `Some` for generated verb stems, `None` for plain headwords. ta-old
    /// stored `verbType` as `vt + 1`, with `0` meaning "not a verb"
    /// (`ConjInfo::verbType`, `Dictionary.h`); this port uses a 0-based id
    /// inside an `Option` instead, so the Phase 1B differential run must
    /// account for that offset (add 1, map `None` to `0`) when comparing
    /// against the old encoding.
    pub verb_type: Option<VerbTypeId>,
    pub entry_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SenseData {
    pub pos: Vec<String>,
    pub glosses: Vec<String>,
    pub xrefs: Vec<String>,
    pub misc: Vec<String>,
    pub info: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryData {
    pub id: u32,
    /// The entry's `<reb>` forms in document order, stored **only when the
    /// entry also has kanji forms** — exactly the set ta-old flags
    /// `JAP_WORD_PRONOUNCE`. Empty for kana-only entries, where the surface
    /// already is the reading and ta-old renders no furigana.
    pub readings: Vec<String>,
    pub senses: Vec<SenseData>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BuildReport {
    pub keys: usize,
    pub records: usize,
    pub entries: usize,
    /// Malformed JMdict entries skipped. Surfaced, never silently dropped.
    pub skipped_entries: usize,
    pub stems: StemStats,
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("index io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("index fst failed: {0}")]
    Fst(#[from] fst::Error),
    #[error("index encoding failed: {0}")]
    Encoding(#[from] bincode::Error),
    #[error("reading JMdict failed: {0}")]
    Jmdict(#[from] crate::jmdict::JmdictError),
    #[error("index format version mismatch: found {found}, expected {expected}")]
    VersionMismatch { found: u32, expected: u32 },
    #[error(
        "index conjugation asset mismatch: found fingerprint {found:#x}, \
         expected {expected:#x}"
    )]
    ConjugationMismatch { found: u64, expected: u64 },
    #[error(
        "index generation {generation} already exists after repeated publish \
         attempts; contention did not clear (partial build kept at {build_dir})"
    )]
    GenerationExists {
        generation: u64,
        build_dir: std::path::PathBuf,
    },
}

/// Open the newest usable index in `root`, building one first if necessary.
///
/// `source` is a *lazy* producer of JMdict XML: it is invoked only when a
/// build is actually required, because the real source is a ~60 MB download
/// and the steady-state path must not pay for it.
///
/// A version or conjugation-fingerprint mismatch triggers a rebuild — both are
/// expected after an application upgrade or a changed `conjugations.json`. Any
/// **other** open failure is returned rather than rebuilt: a generation that
/// was published as complete but does not read back is a bug or a failing
/// disk, and silently rebuilding would hide it.
pub fn ensure_dictionary<R, F>(
    root: &Path,
    table: &ConjugationTable,
    opts: &StemOptions,
    keep: usize,
    source: F,
) -> Result<Index, IndexError>
where
    F: FnOnce() -> std::io::Result<R>,
    R: std::io::BufRead,
{
    // `keep = 0` would let `sweep` delete the generation this very call just
    // published, so the trailing `Index::open` below would fail with a bare
    // `Io(NotFound)`. Clamped rather than rejected: `sweep` itself may still
    // accept 0 (deleting everything is a coherent thing for a primitive to
    // do), but this is the policy boundary that must not self-destruct. The
    // CLI keeps its own guard too, since it can give a better usage message
    // than a silent clamp.
    let keep = keep.max(1);

    if let Some(current) = generations::latest(root)? {
        match Index::open(&current) {
            Ok(index) => return Ok(index),
            // Expected after an upgrade or an asset change: rebuild.
            Err(IndexError::VersionMismatch { .. })
            | Err(IndexError::ConjugationMismatch { .. }) => {}
            // Listed exhaustively rather than with a catch-all so that a new
            // IndexError variant forces a decision here instead of silently
            // landing in one arm or the other.
            Err(e @ IndexError::Io(_))
            | Err(e @ IndexError::Fst(_))
            | Err(e @ IndexError::Encoding(_))
            | Err(e @ IndexError::Jmdict(_))
            | Err(e @ IndexError::GenerationExists { .. }) => return Err(e),
        }
    }

    let xml = source()?;
    let (published, _report) = generations::build_new(root, xml, table, opts)?;
    // Sweep after the publish, never before: sweeping first could delete the
    // generation the caller would have fallen back to had the build failed.
    //
    // Retention is best-effort by contract: `sweep` cannot delete a mapped
    // file on Windows, and `DEFAULT_KEEP_GENERATIONS` exists precisely so a
    // failed sweep leaves a usable index behind. Failing the whole call here
    // would discard a generation that was just published successfully, after
    // the ~60 MB source was already consumed.
    match generations::sweep(root, keep) {
        Ok(_removed) => {}
        Err(_e) => {}
    }
    open_published_or_newest(root, &published)
}

/// Open `published` — the generation this call just built — falling back to
/// whatever is newest in `root` if `published` is already gone.
///
/// `build_new`'s retry loop lets multiple concurrent builders all succeed
/// from the same `root`, so by the time execution reaches here a *different*
/// call may already have published a later generation and swept `published`
/// away — at `keep = 1` with two concurrent builders, or the default
/// `keep = 2` with three. That is a successful supersession, not a failure:
/// the caller asked for the newest usable index, not specifically for the
/// generation this call happened to build. Only when nothing survives at
/// all — `latest` also returns `None` — is the original `NotFound` a real
/// error.
fn open_published_or_newest(root: &Path, published: &Path) -> Result<Index, IndexError> {
    match Index::open(published) {
        Ok(index) => Ok(index),
        Err(IndexError::Io(io_err)) if io_err.kind() == std::io::ErrorKind::NotFound => {
            match generations::latest(root)? {
                Some(fallback) => Index::open(&fallback),
                None => Err(IndexError::Io(io_err)),
            }
        }
        // Listed exhaustively, matching the discipline above: a new
        // IndexError variant must force a decision here too.
        Err(e @ IndexError::Io(_))
        | Err(e @ IndexError::Fst(_))
        | Err(e @ IndexError::Encoding(_))
        | Err(e @ IndexError::Jmdict(_))
        | Err(e @ IndexError::VersionMismatch { .. })
        | Err(e @ IndexError::ConjugationMismatch { .. })
        | Err(e @ IndexError::GenerationExists { .. }) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::PathBuf;

    use super::*;
    use crate::conjugation::ConjugationTable;
    use crate::stem::StemOptions;

    const XML: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "<JMdict>",
        "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
        "<r_ele><reb>ほん</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
        "</JMdict>",
    );

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn table_and_opts() -> (ConjugationTable, StemOptions) {
        (
            ConjugationTable::load_embedded().expect("table"),
            StemOptions::default(),
        )
    }

    /// Rewrite `gen-N`'s header so the next open rejects it.
    fn corrupt_header(dir: &std::path::Path, f: impl FnOnce(&mut IndexHeader)) {
        let path = dir.join(HEADER_FILE);
        let mut header: IndexHeader =
            bincode::deserialize(&std::fs::read(&path).expect("read")).expect("decode");
        f(&mut header);
        std::fs::write(&path, bincode::serialize(&header).expect("encode")).expect("write");
    }

    #[test]
    fn an_empty_root_builds_the_first_generation() {
        let root = scratch("ensure-first");
        let (table, opts) = table_and_opts();
        let calls = Cell::new(0usize);

        let index = ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("ensure_dictionary");

        assert_eq!(calls.get(), 1);
        assert!(root.join("gen-1").exists());
        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
    }

    /// The steady-state path. A ~60 MB download must not happen just because
    /// the application started.
    #[test]
    fn a_valid_generation_is_reused_without_consulting_the_source() {
        let root = scratch("ensure-reuse");
        let (table, opts) = table_and_opts();

        let first = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            first.set(first.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("first");
        assert_eq!(first.get(), 1);

        let second = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            second.set(second.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("second");
        assert_eq!(second.get(), 0, "the source was consulted on a valid index");
        assert!(
            !root.join("gen-2").exists(),
            "a redundant generation was built"
        );
    }

    #[test]
    fn a_version_mismatch_rebuilds() {
        let root = scratch("ensure-version");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        corrupt_header(&root.join("gen-1"), |h| {
            h.version = INDEX_FORMAT_VERSION - 1;
        });

        let calls = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("rebuild");
        assert_eq!(calls.get(), 1, "a stale version must rebuild");
        assert!(root.join("gen-2").exists());
    }

    #[test]
    fn a_fingerprint_mismatch_rebuilds() {
        let root = scratch("ensure-fingerprint");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        corrupt_header(&root.join("gen-1"), |h| {
            h.conjugation_fingerprint ^= 1;
        });

        let calls = Cell::new(0usize);
        ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect("rebuild");
        assert_eq!(calls.get(), 1);
        assert!(root.join("gen-2").exists());
    }

    /// A published-then-corrupt generation is a bug or a failing disk.
    /// Rebuilding would re-download ~60 MB and hide it.
    #[test]
    fn a_corrupt_generation_errors_rather_than_rebuilding() {
        let root = scratch("ensure-corrupt");
        let (table, opts) = table_and_opts();
        ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("first");

        // Truncate a file `open` reads eagerly, leaving the header intact so
        // version and fingerprint both still validate.
        std::fs::write(root.join("gen-1").join(ENTRIES_INDEX_FILE), b"\x00\x00").expect("truncate");

        let calls = Cell::new(0usize);
        let err = ensure_dictionary(&root, &table, &opts, 2, || {
            calls.set(calls.get() + 1);
            Ok(XML.as_bytes())
        })
        .expect_err("corruption must surface");

        assert!(
            matches!(err, IndexError::Encoding(_) | IndexError::Io(_)),
            "expected a decode or io error, got {err:?}"
        );
        assert_eq!(calls.get(), 0, "a corrupt index must not trigger a rebuild");
        assert!(!root.join("gen-2").exists());
    }

    #[test]
    fn a_rebuild_sweeps_to_the_retention_limit() {
        let root = scratch("ensure-sweep");
        let (table, opts) = table_and_opts();

        // Four rounds, invalidating the head each time so every round rebuilds.
        for _ in 0..4 {
            ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("ensure");
            let head = generations::latest(&root).expect("latest").expect("head");
            corrupt_header(&head, |h| h.version = INDEX_FORMAT_VERSION - 1);
        }

        let remaining = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(generations::GENERATION_PREFIX)
            })
            .count();
        assert_eq!(
            remaining, 2,
            "retention was not applied: {remaining} generations"
        );
    }

    /// `keep` must actually reach `sweep`. Every other test in this module
    /// passes `keep = 2`, which equals `DEFAULT_KEEP_GENERATIONS` — so
    /// replacing `sweep(root, keep)` with `sweep(root, DEFAULT_KEEP_GENERATIONS)`
    /// inside `ensure_dictionary` would leave every one of them green. This is
    /// the test that fails if that substitution is made.
    #[test]
    fn ensure_dictionary_honors_a_keep_of_one() {
        let root = scratch("ensure-keep-one");
        let (table, opts) = table_and_opts();

        // Three rounds at keep=2, so all three generations would survive
        // under the default — the point below is the fourth round's keep=1.
        for _ in 0..3 {
            ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("ensure");
            let head = generations::latest(&root).expect("latest").expect("head");
            corrupt_header(&head, |h| h.version = INDEX_FORMAT_VERSION - 1);
        }

        let index = ensure_dictionary(&root, &table, &opts, 1, || Ok(XML.as_bytes()))
            .expect("ensure with keep=1");

        let remaining = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(generations::GENERATION_PREFIX)
            })
            .count();
        assert_eq!(remaining, 1, "keep=1 must leave exactly one generation");
        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
    }

    /// The library boundary must not accept `keep = 0`: it would let `sweep`
    /// delete the very generation this call just published, leaving nothing
    /// for the trailing `Index::open` to open. Phase 2B calls
    /// `ensure_dictionary` directly, so the CLI's own guard cannot be relied
    /// on to catch this.
    #[test]
    fn a_zero_keep_is_clamped_rather_than_self_destructive() {
        let root = scratch("ensure-keep-zero");
        let (table, opts) = table_and_opts();

        let index = ensure_dictionary(&root, &table, &opts, 0, || Ok(XML.as_bytes()))
            .expect("ensure_dictionary must not self-destruct on keep=0");

        assert!(
            root.join("gen-1").exists(),
            "the published generation must survive a keep=0 call"
        );
        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
    }

    /// Covers the interaction `open_published_or_newest` exists to survive:
    /// `build_new`'s retry loop lets concurrent builders all succeed, so the
    /// generation `ensure_dictionary` "published" can be swept away by a
    /// different, later-publishing call before this call's own `Index::open`
    /// ever runs. Reachable at `keep = 1` with two concurrent builders, or
    /// the default `keep = 2` with three — see R1.
    ///
    /// Driven directly against the helper rather than through a real race:
    /// nothing can deterministically force two live `ensure_dictionary`
    /// calls to interleave between "this call's publish returns" and "this
    /// call's own open runs," so instead publish two generations for real,
    /// delete the one this call would have "published," and confirm the
    /// fallback opens the survivor.
    #[test]
    fn open_published_or_newest_falls_back_when_its_own_generation_is_swept() {
        let root = scratch("ensure-superseded");
        let (table, opts) = table_and_opts();

        let (gen1, _) =
            generations::build_new(&root, XML.as_bytes(), &table, &opts).expect("gen-1");
        let (gen2, _) =
            generations::build_new(&root, XML.as_bytes(), &table, &opts).expect("gen-2");

        // Simulate a different call's sweep removing gen1 — the generation
        // this call "published" — before this call's own open ran.
        std::fs::remove_dir_all(&gen1).expect("simulate a concurrent sweep");

        let index = open_published_or_newest(&root, &gen1)
            .expect("a superseding generation must open, not error");
        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
        assert_eq!(gen2, root.join("gen-2"));
    }

    /// The other half of the fallback: when nothing survives at all, the
    /// original `NotFound` must still surface rather than being swallowed.
    #[test]
    fn open_published_or_newest_errors_when_nothing_survives() {
        let root = scratch("ensure-superseded-none");
        let err = open_published_or_newest(&root, &root.join("gen-1"))
            .expect_err("an absent root has nothing to fall back to");
        assert!(matches!(err, IndexError::Io(_)), "expected Io, got {err:?}");
    }

    /// A sweep that fails outright — not merely a benign race with another
    /// sweeper, but a real removal failure — must still leave
    /// `ensure_dictionary` returning the freshly published index. Injecting a
    /// portable `remove_dir_all` failure needs an OS-specific trick (`chmod`
    /// here); there is no equivalently simple one on Windows, so this test is
    /// `#[cfg(unix)]` rather than run everywhere.
    #[cfg(unix)]
    #[test]
    fn a_sweep_failure_does_not_fail_ensure_dictionary() {
        use std::os::unix::fs::PermissionsExt;

        /// Restores write permission on drop, including on a test panic, so a
        /// failed assertion here cannot leave a scratch directory that a
        /// later run of this same test cannot clean up.
        struct RestorePerms(PathBuf);
        impl Drop for RestorePerms {
            fn drop(&mut self) {
                let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
            }
        }

        let root = scratch("ensure-sweep-fails");
        let (table, opts) = table_and_opts();

        // Three rounds, invalidating the head each time. keep=2's normal
        // operation already sweeps gen-1 away during this loop, leaving
        // gen-2 and gen-3 (corrupt) once it finishes.
        for _ in 0..3 {
            ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes())).expect("ensure");
            let head = generations::latest(&root).expect("latest").expect("head");
            corrupt_header(&head, |h| h.version = INDEX_FORMAT_VERSION - 1);
        }
        assert!(
            !root.join("gen-1").exists(),
            "setup: gen-1 should already be swept"
        );
        assert!(root.join("gen-2").exists(), "setup: gen-2 should remain");

        // gen-2 is what the next sweep(keep=2) will target once gen-4
        // publishes. Strip write permission so `remove_dir_all` cannot
        // unlink its contents — a real failure, not a benign race.
        let victim = root.join("gen-2");
        std::fs::set_permissions(&victim, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        let _restore = RestorePerms(victim);

        let index = ensure_dictionary(&root, &table, &opts, 2, || Ok(XML.as_bytes()))
            .expect("a failed sweep must not fail ensure_dictionary");

        assert_eq!(
            index.entry(1000010).expect("entry").expect("present").id,
            1000010
        );
        assert!(
            root.join("gen-4").exists(),
            "the newly published generation must survive"
        );
        assert!(
            root.join("gen-2").exists(),
            "the un-removable generation must survive the failed sweep"
        );
    }
}
