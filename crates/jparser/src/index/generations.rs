// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Immutable generation directories.
//!
//! An index is published as `<root>/gen-<N>/`, built first into
//! `<root>/.build-<pid>-<nanos>-<seq>/` and moved into place with a single
//! `fs::rename`. Readers resolve the highest `N`.
//!
//! The layout exists because `Index::open` is a five-file sequence against a
//! directory *name*: `header.bin`, `entries.idx`, then three mmaps. Any scheme
//! where readers resolve a **mutable** name — rename-over-the-directory, a
//! symlink flip, or a `CURRENT` pointer file — can splice one generation's
//! `entries.idx` onto another's `entries.bin` when a reader opens mid-swap,
//! which yields well-formed wrong answers rather than an error. Atomicity of
//! the pointer swap does not help, because the open is not atomic. A `gen-N`
//! directory's contents never change after creation, so a straddling open
//! either succeeds wholly or gets `ENOENT`.

use std::io::BufRead;
use std::path::{Path, PathBuf};

use crate::conjugation::ConjugationTable;
use crate::index::build::build_from_reader;
use crate::index::{BuildReport, IndexError};
use crate::stem::StemOptions;

/// Directory-name prefix for a published generation.
pub const GENERATION_PREFIX: &str = "gen-";

/// Directory-name prefix for an in-progress build. Dotted so it sorts and
/// displays as hidden, and so it can never collide with `GENERATION_PREFIX`.
pub const BUILD_PREFIX: &str = ".build-";

/// Generations retained by `sweep`. Two rather than one: `sweep` cannot delete
/// a mapped file on Windows, so a failed sweep must never be load-bearing.
/// See the design spec §7.
pub const DEFAULT_KEEP_GENERATIONS: usize = 2;

/// Publish attempts before `build_new` reports sustained contention. Each
/// attempt recomputes the target generation, so a builder that loses a race
/// simply takes the next number rather than failing.
const PUBLISH_ATTEMPTS: usize = 8;

/// Parse `gen-<N>` into `N`, rejecting everything else.
///
/// Deliberately strict. `gen-01` is rejected rather than read as 1: a
/// permissive parse would let a hand-created directory shadow a real
/// generation, which is precisely the ambiguity immutable names remove.
///
/// `pub` because it is already the single strict parser for the layout, and
/// `jparser-cli`'s `gen-list` needs it to sort generations numerically
/// rather than lexicographically.
pub fn generation_number(name: &str) -> Option<u64> {
    let digits = name.strip_prefix(GENERATION_PREFIX)?;
    if digits.is_empty() {
        return None;
    }
    // Rejects '+', '-', whitespace, and any non-ASCII digit `u64::from_str`
    // would otherwise accept or refuse on its own terms — checked explicitly
    // so the rule is visible rather than implied by the parser.
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Leading zeros: `gen-01` and `gen-1` would both claim generation 1.
    if digits.len() > 1 && digits.starts_with('0') {
        return None;
    }
    digits.parse::<u64>().ok()
}

/// Directory entries in `root` paired with their names, or `None` when `root`
/// does not exist. Shared by `latest_number` and `sweep` so the absent-root,
/// non-directory, and non-UTF-8 rules cannot drift apart.
fn scan_dirs(root: &Path) -> Result<Option<Vec<(String, PathBuf)>>, IndexError> {
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut found = Vec::new();
    for entry in read {
        let entry = entry?;
        // `file_type()` does not follow symlinks — intent, not omission. A
        // symlinked `gen-N` would be a mutable name, the exact hazard this
        // layout exists to remove, so resolving it would reintroduce it.
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        found.push((name.to_owned(), entry.path()));
    }
    Ok(Some(found))
}

/// Highest generation in `root`, as `(number, path)`.
///
/// An absent `root` yields `Ok(None)` — that is the first-run "no dictionary
/// yet" signal, not an error.
pub(crate) fn latest_number(root: &Path) -> Result<Option<(u64, PathBuf)>, IndexError> {
    let Some(found) = scan_dirs(root)? else {
        return Ok(None);
    };

    let mut best: Option<(u64, PathBuf)> = None;
    for (name, path) in found {
        let Some(number) = generation_number(&name) else {
            continue;
        };
        if best.as_ref().is_none_or(|(best, _)| number > *best) {
            best = Some((number, path));
        }
    }
    Ok(best)
}

/// Path of the highest generation in `root`, or `None` if there is none.
pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError> {
    Ok(latest_number(root)?.map(|(_, path)| path))
}

/// Move a completed build into `<root>/gen-<generation>`.
///
/// The rename is the publish: it is the single operation that makes a
/// generation visible, and it is atomic. On failure the build directory is
/// left in place so the caller can retry without rebuilding.
///
/// POSIX `rename` silently replaces an **empty** target directory rather than
/// failing, so a hand-made empty `gen-N` would be consumed without error.
/// Harmless here: no reader can ever hold a valid `Index` on an empty
/// directory, and the race test below already relies on this — it writes an
/// `occupied` file into `gen-1` before driving the collision, specifically so
/// the target is non-empty.
fn publish(build_dir: &Path, root: &Path, generation: u64) -> Result<PathBuf, IndexError> {
    let target = root.join(format!("{GENERATION_PREFIX}{generation}"));
    match std::fs::rename(build_dir, &target) {
        Ok(()) => Ok(target),
        Err(e) => {
            // The raw errno differs per platform (ENOTEMPTY is 66 on Darwin,
            // 39 on Linux, and Windows reports something else entirely).
            // Probing the target is portable and says the same thing: if it
            // exists, somebody else published first.
            if target.exists() {
                Err(IndexError::GenerationExists {
                    generation,
                    build_dir: build_dir.to_path_buf(),
                })
            } else {
                Err(e.into())
            }
        }
    }
}

/// Next generation number to try publishing as. Recomputed fresh on every
/// call so a builder that lost a race simply advances past whoever won,
/// rather than retrying the same doomed target.
fn next_generation(root: &Path) -> Result<u64, IndexError> {
    Ok(latest_number(root)?.map_or(1, |(n, _)| n + 1))
}

/// Distinguishes concurrent builders *within* one process. The pid separates
/// processes and the clock separates sequential builds, but two threads share
/// a pid and can read the same coarse clock value, so neither is sufficient
/// alone.
static BUILD_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Build an index from `xml` and publish it as the next generation.
///
/// Builds into `<root>/.build-<pid>-<nanos>-<seq>/` first, so `root` never
/// contains a partially-written generation. `root` and the build directory
/// are therefore always on one filesystem — `fs::rename` returns `EXDEV`
/// across devices and never falls back to copying.
///
/// Publishing is retried up to `PUBLISH_ATTEMPTS` times: two builders
/// starting from the same `root` both compute the same target, so the loser
/// of a single attempt would otherwise fail even though a valid generation
/// now exists. Each retry recomputes the target rather than reusing the one
/// that just lost.
pub fn build_new(
    root: &Path,
    xml: impl BufRead,
    table: &ConjugationTable,
    opts: &StemOptions,
) -> Result<(PathBuf, BuildReport), IndexError> {
    std::fs::create_dir_all(root)?;

    // A clock before the Unix epoch is not a reason to refuse to build a
    // dictionary; the pid half still separates concurrent processes.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    // Two threads in this process share `pid` and can read the same coarse
    // `nanos` value, which would otherwise collide on `create_dir` below.
    let sequence = BUILD_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let build_dir = root.join(format!("{BUILD_PREFIX}{pid}-{nanos}-{sequence}"));

    // `create_dir`, never `create_dir_all`: the nonce must not already exist,
    // and a collision is a signal worth surfacing rather than absorbing.
    std::fs::create_dir(&build_dir)?;

    let report = build_from_reader(xml, table, opts, &build_dir)?;

    let mut published = publish(&build_dir, root, next_generation(root)?);
    for _ in 1..PUBLISH_ATTEMPTS {
        if !matches!(published, Err(IndexError::GenerationExists { .. })) {
            break;
        }
        published = publish(&build_dir, root, next_generation(root)?);
    }
    Ok((published?, report))
}

/// Remove `path`, treating an already-absent directory as success. Another
/// sweeper reaching it first produces the same end state this function exists
/// to produce.
fn remove_if_present(path: &Path) -> Result<bool, IndexError> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Remove `.build-*` orphans and all but the `keep` highest generations.
/// Returns the number of directories removed.
///
/// # Precondition
///
/// **Call only before any `Index` has been opened from `root`** — at
/// application startup, never during a session. Phase 1A verified on Darwin
/// that an established mmap survives `remove_dir_all` on its parent, but
/// Windows does not generally permit deleting a mapped file.
/// [`DEFAULT_KEEP_GENERATIONS`] exists so that a sweep which fails anyway is
/// never load-bearing.
///
/// Directories that are neither a valid generation nor a build orphan are left
/// alone: this function did not create them.
///
/// When `build_from_reader` itself fails — malformed XML, a full disk — `build_new`
/// leaves its `.build-<pid>-<nanos>-<seq>` directory behind with no cleanup.
/// `sweep` is what reclaims those.
pub fn sweep(root: &Path, keep: usize) -> Result<usize, IndexError> {
    let Some(found) = scan_dirs(root)? else {
        return Ok(0);
    };

    let mut generations: Vec<(u64, PathBuf)> = Vec::new();
    let mut orphans: Vec<PathBuf> = Vec::new();
    for (name, path) in found {
        if let Some(number) = generation_number(&name) {
            generations.push((number, path));
        } else if name.starts_with(BUILD_PREFIX) {
            orphans.push(path);
        }
    }

    // Highest first, so the tail past `keep` is exactly what to drop.
    generations.sort_by_key(|b| std::cmp::Reverse(b.0));

    let mut removed = 0usize;
    for path in orphans
        .iter()
        .chain(generations.iter().skip(keep).map(|(_, path)| path))
    {
        if remove_if_present(path)? {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::conjugation::ConjugationTable;
    use crate::stem::StemOptions;

    /// The smallest JMdict that produces a non-empty index.
    const MINI_XML: &str = concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        "<JMdict>",
        "<entry><ent_seq>1000001</ent_seq>",
        "<k_ele><keb>本</keb></k_ele>",
        "<r_ele><reb>ほん</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
        "<entry><ent_seq>1000002</ent_seq>",
        "<k_ele><keb>山</keb></k_ele>",
        "<r_ele><reb>やま</reb></r_ele>",
        "<sense><pos>&n;</pos><gloss>mountain</gloss></sense></entry>",
        "</JMdict>",
    );

    fn build_into(root: &Path) -> PathBuf {
        let table = ConjugationTable::load_embedded().expect("table");
        let opts = StemOptions::default();
        let (path, _) = build_new(root, MINI_XML.as_bytes(), &table, &opts).expect("build_new");
        path
    }

    /// Per the crate's no-`tempfile` rule; mirrors `tests/index_roundtrip.rs:20`.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn mkdir(root: &Path, name: &str) {
        std::fs::create_dir_all(root.join(name)).expect("mkdir");
    }

    #[test]
    fn an_absent_root_is_not_an_error() {
        let root = std::env::temp_dir().join("jparser-test-gen-absent");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(latest(&root).expect("latest"), None);
    }

    #[test]
    fn an_empty_root_has_no_generation() {
        let root = scratch("gen-empty");
        assert_eq!(latest(&root).expect("latest"), None);
    }

    #[test]
    fn the_highest_generation_wins() {
        let root = scratch("gen-highest");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-2");
        mkdir(&root, "gen-7");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-7")));
    }

    /// Lexicographic ordering would pick gen-9. The comparison is numeric.
    #[test]
    fn generations_order_numerically_not_lexicographically() {
        let root = scratch("gen-numeric");
        mkdir(&root, "gen-9");
        mkdir(&root, "gen-10");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-10")));
    }

    #[test]
    fn malformed_generation_names_are_ignored() {
        let root = scratch("gen-malformed");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-");
        mkdir(&root, "gen-abc");
        mkdir(&root, "gen-1x");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    /// A hand-made `gen-01` must not shadow the real `gen-1`, and must not be
    /// mistaken for a *higher* generation than `gen-1` either.
    #[test]
    fn a_zero_padded_generation_is_ignored() {
        let root = scratch("gen-padded");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-01");
        mkdir(&root, "gen-007");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    #[test]
    fn build_directories_are_never_returned() {
        let root = scratch("gen-buildskip");
        mkdir(&root, ".build-123-456");
        assert_eq!(latest(&root).expect("latest"), None);
    }

    /// A *file* named like a generation is not a generation.
    #[test]
    fn a_file_named_like_a_generation_is_ignored() {
        let root = scratch("gen-file");
        std::fs::write(root.join("gen-9"), b"not a directory").expect("write");
        mkdir(&root, "gen-1");
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-1")));
    }

    #[test]
    fn latest_number_reports_the_number_beside_the_path() {
        let root = scratch("gen-number");
        mkdir(&root, "gen-4");
        let (n, path) = latest_number(&root).expect("latest_number").expect("some");
        assert_eq!(n, 4);
        assert_eq!(path, root.join("gen-4"));
    }

    #[test]
    fn the_first_build_publishes_generation_one() {
        let root = scratch("gen-first");
        assert_eq!(build_into(&root), root.join("gen-1"));
    }

    #[test]
    fn successive_builds_increment_the_generation() {
        let root = scratch("gen-increment");
        assert_eq!(build_into(&root), root.join("gen-1"));
        assert_eq!(build_into(&root), root.join("gen-2"));
        assert_eq!(build_into(&root), root.join("gen-3"));
    }

    #[test]
    fn a_successful_build_leaves_no_temp_directory() {
        let root = scratch("gen-notemp");
        build_into(&root);
        let leftovers: Vec<_> = std::fs::read_dir(&root)
            .expect("read_dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(BUILD_PREFIX))
            .collect();
        assert!(leftovers.is_empty(), "temp directory survived a success");
    }

    #[test]
    fn a_published_generation_opens() {
        let root = scratch("gen-opens");
        let path = build_into(&root);
        let index = crate::index::load::Index::open(&path).expect("open");
        let entry = index.entry(1000001).expect("entry").expect("present");
        assert_eq!(entry.id, 1000001);
    }

    /// Two builders running at once must not collide on the nonce. Both
    /// threads create their build directory before either renames, so a
    /// constant temp name would make one `create_dir` fail with
    /// `AlreadyExists`.
    ///
    /// With the bounded retry in place, both must also *publish* — as gen-1
    /// and gen-2 — rather than one of them failing outright. That is the
    /// whole point of the retry: a double-launched application must not fail
    /// to start just because it lost a race with itself.
    ///
    /// This is also `build_new`'s retry loop's *only* coverage — see
    /// `retrying_publish_after_a_lost_race_lands_on_the_next_number` below,
    /// which cannot reach the loop at all. That coverage is inherently
    /// probabilistic: this test detects the loop being removed only when the
    /// two threads actually contend for the same generation number, which
    /// depends on how the scheduler interleaves them, not on anything this
    /// test controls.
    #[test]
    fn two_concurrent_builds_use_distinct_temp_names() {
        let root = scratch("gen-nonce");
        let table = ConjugationTable::load_embedded().expect("table");
        let opts = StemOptions::default();

        let results = std::thread::scope(|s| {
            let handles: Vec<_> = (0..2)
                .map(|_| s.spawn(|| build_new(&root, MINI_XML.as_bytes(), &table, &opts)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("join"))
                .collect::<Vec<_>>()
        });

        for result in &results {
            assert!(
                result.is_ok(),
                "the retry must absorb a lost race: {result:?}"
            );
        }
        assert_eq!(latest(&root).expect("latest"), Some(root.join("gen-2")));
    }

    /// Confirms that publishing again after a lost race lands on the next
    /// generation number — i.e. that `publish` and `next_generation` compose
    /// correctly for a retry. **Not** a test of `build_new`'s retry loop
    /// itself: it drives `publish`/`next_generation` directly rather than
    /// calling `build_new`, and would still pass even if that loop were
    /// deleted entirely. Nothing can deterministically force two live
    /// builders to collide on their first attempt — that is inherently a
    /// race — so the loop's own coverage is `two_concurrent_builds_use_distinct_temp_names`
    /// above, and it is only probabilistic there.
    #[test]
    fn retrying_publish_after_a_lost_race_lands_on_the_next_number() {
        let root = scratch("gen-retry");
        mkdir(&root, "gen-1");
        std::fs::write(root.join("gen-1").join("occupied"), b"x").expect("write");

        let build_dir = root.join(format!("{BUILD_PREFIX}test-retry"));
        std::fs::create_dir(&build_dir).expect("create build dir");

        // First attempt loses the race: gen-1 already exists.
        let lost = publish(&build_dir, &root, 1);
        assert!(matches!(lost, Err(IndexError::GenerationExists { .. })));
        assert!(
            build_dir.exists(),
            "the loser's build must survive for a retry"
        );

        // The retry recomputes the target and succeeds against the next number.
        let retried = next_generation(&root)
            .and_then(|next| publish(&build_dir, &root, next))
            .expect("the retry must succeed");
        assert_eq!(retried, root.join("gen-2"));
        assert!(
            !build_dir.exists(),
            "a successful publish consumes the build dir"
        );
    }

    /// The lost-race branch, driven directly because `build_new` alone can
    /// never collide — it always targets `latest + 1`.
    #[test]
    fn publishing_onto_an_existing_generation_reports_the_race() {
        let root = scratch("gen-race");
        mkdir(&root, "gen-1");
        std::fs::write(root.join("gen-1").join("occupied"), b"x").expect("write");

        let build_dir = root.join(format!("{BUILD_PREFIX}test-1"));
        std::fs::create_dir(&build_dir).expect("create build dir");

        let err = publish(&build_dir, &root, 1).expect_err("must not overwrite");
        match err {
            IndexError::GenerationExists {
                generation,
                build_dir: kept,
            } => {
                assert_eq!(generation, 1);
                assert_eq!(kept, build_dir);
            }
            other => panic!("expected GenerationExists, got {other:?}"),
        }
        assert!(
            build_dir.exists(),
            "the loser's build must survive for a retry"
        );
        assert!(
            root.join("gen-1").join("occupied").exists(),
            "winner untouched"
        );
    }

    #[test]
    fn sweep_keeps_the_highest_generations() {
        let root = scratch("gen-sweep-keep");
        for n in 1..=5 {
            mkdir(&root, &format!("gen-{n}"));
        }
        assert_eq!(sweep(&root, 2).expect("sweep"), 3);
        assert!(!root.join("gen-3").exists());
        assert!(root.join("gen-4").exists());
        assert!(root.join("gen-5").exists());
    }

    #[test]
    fn sweep_removes_orphaned_builds() {
        let root = scratch("gen-sweep-orphan");
        mkdir(&root, "gen-1");
        mkdir(&root, ".build-1-2");
        mkdir(&root, ".build-3-4");
        assert_eq!(sweep(&root, 2).expect("sweep"), 2);
        assert!(root.join("gen-1").exists());
        assert!(!root.join(".build-1-2").exists());
    }

    #[test]
    fn sweep_keeps_everything_when_keep_exceeds_the_count() {
        let root = scratch("gen-sweep-few");
        mkdir(&root, "gen-1");
        assert_eq!(sweep(&root, 2).expect("sweep"), 0);
        assert!(root.join("gen-1").exists());
    }

    /// Malformed names are not generations, so `sweep` must not count them
    /// toward `keep` — nor delete them, since it did not create them.
    #[test]
    fn sweep_ignores_malformed_names() {
        let root = scratch("gen-sweep-malformed");
        mkdir(&root, "gen-1");
        mkdir(&root, "gen-01");
        mkdir(&root, "unrelated");
        assert_eq!(sweep(&root, 1).expect("sweep"), 0);
        assert!(root.join("gen-01").exists());
        assert!(root.join("unrelated").exists());
    }

    /// The arm `sweep`'s racing-sweeper tolerance depends on, covered
    /// directly and deterministically rather than through two racing
    /// `sweep` calls: that race was measured to hit the `NotFound` arm only
    /// ~1 run in 20 (3/60), which made the fix it exists to cover
    /// effectively untested. There is nothing probabilistic about "call this
    /// on a directory that is already gone," so drive it as that.
    #[test]
    fn remove_if_present_removes_an_existing_directory() {
        let root = scratch("gen-remove-if-present-existing");
        mkdir(&root, "victim");
        let path = root.join("victim");
        assert!(remove_if_present(&path).expect("remove_if_present"));
        assert!(!path.exists());
    }

    /// Two sweepers racing on one root — two `ensure_dictionary` calls, or one
    /// plus `gen-sweep` — must not treat "the other one already removed this
    /// directory" as an error; that is the exact outcome this arm exists to
    /// produce, not a failure of it.
    #[test]
    fn remove_if_present_tolerates_an_absent_directory() {
        let root = scratch("gen-remove-if-present-absent");
        let path = root.join("never-existed");
        assert!(!remove_if_present(&path).expect("remove_if_present"));
    }

    #[test]
    fn sweep_on_an_absent_root_is_not_an_error() {
        let root = std::env::temp_dir().join("jparser-test-gen-sweep-absent");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(sweep(&root, 2).expect("sweep"), 0);
    }

    #[test]
    fn the_default_retention_is_two() {
        assert_eq!(DEFAULT_KEEP_GENERATIONS, 2);
    }
}
