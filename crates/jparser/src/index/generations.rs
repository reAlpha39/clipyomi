// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Immutable generation directories.
//!
//! An index is published as `<root>/gen-<N>/`, built first into
//! `<root>/.build-<pid>-<nanos>/` and moved into place with a single
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

use std::path::{Path, PathBuf};

use crate::index::IndexError;

/// Directory-name prefix for a published generation.
pub const GENERATION_PREFIX: &str = "gen-";

/// Directory-name prefix for an in-progress build. Dotted so it sorts and
/// displays as hidden, and so it can never collide with `GENERATION_PREFIX`.
pub const BUILD_PREFIX: &str = ".build-";

/// Generations retained by `sweep`. Two rather than one: `sweep` cannot delete
/// a mapped file on Windows, so a failed sweep must never be load-bearing.
/// See the design spec §7.
pub const DEFAULT_KEEP_GENERATIONS: usize = 2;

/// Parse `gen-<N>` into `N`, rejecting everything else.
///
/// Deliberately strict. `gen-01` is rejected rather than read as 1: a
/// permissive parse would let a hand-created directory shadow a real
/// generation, which is precisely the ambiguity immutable names remove.
fn generation_number(name: &str) -> Option<u64> {
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

/// Highest generation in `root`, as `(number, path)`.
///
/// An absent `root` yields `Ok(None)` — that is the first-run "no dictionary
/// yet" signal, not an error.
pub(crate) fn latest_number(root: &Path) -> Result<Option<(u64, PathBuf)>, IndexError> {
    let read = match std::fs::read_dir(root) {
        Ok(read) => read,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    let mut best: Option<(u64, PathBuf)> = None;
    for entry in read {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(number) = generation_number(name) else {
            continue;
        };
        // MSRV 1.75: `Option::is_none_or` is 1.82. Do not "simplify" this.
        if best.as_ref().map_or(true, |(best, _)| number > *best) {
            best = Some((number, entry.path()));
        }
    }
    Ok(best)
}

/// Path of the highest generation in `root`, or `None` if there is none.
pub fn latest(root: &Path) -> Result<Option<PathBuf>, IndexError> {
    Ok(latest_number(root)?.map(|(_, path)| path))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
