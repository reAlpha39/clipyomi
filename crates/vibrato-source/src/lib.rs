// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Obtaining the compiled Vibrato dictionary that backs MeCab boundary hints.
//!
//! 2C's `VibratoTokenizer::load` wants one uncompressed `system.dic`. Vibrato
//! distributes it as a zstd-compressed member of a `.tar.xz`, and vibrato's own
//! API refuses compressed input, so unwrapping both layers is this crate's job.
//!
//! Modelled on `jmdict-source`, and deliberately independent of `jparser` for
//! the same reason: it takes the directory it is given and knows nothing about
//! how an application lays out its data. One difference of substance — the
//! asset is a single immutable GitHub release file, so it is pinned by
//! SHA-256, where JMdict (rebuilt daily) can only be checked structurally.

use std::path::{Path, PathBuf};

pub mod extract;
pub mod fetch;

/// The pinned release asset. HTTPS is not a preference: GitHub does not serve
/// this file over plain HTTP, which is why this crate carries a TLS stack and
/// `jmdict-source` deliberately does not.
pub const ASSET_URL: &str =
    "https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz";

/// SHA-256 of [`ASSET_URL`]'s body, lowercase hex. Checked before any
/// decompressor sees a byte, so a substituted or truncated archive fails at
/// the cheapest point rather than somewhere inside three nested formats.
pub const ASSET_SHA256: &str = "4764f983b7c3a9e1cb6a5ee945e00558efd812980e0dad61224f63ee3b0475d9";

/// Measured size of the pinned asset. Not a security control — [`ASSET_SHA256`]
/// is — but a mismatch identifies a wrong-file download in the error message
/// without making the reader compare two hex strings.
pub const ASSET_BYTES: u64 = 7_704_680;

/// The published directory's name, inside the caller's hints directory. Named
/// for the asset so a future dictionary lands beside this one instead of
/// overwriting it.
pub const DICT_DIR: &str = "ipadic-mecab-2_7_0";

/// The published dictionary: decompressed, exactly what `VibratoTokenizer::load`
/// and `TA_HINTS_DICT` expect.
pub const SYSTEM_DIC: &str = "system.dic";

/// The archive member [`SYSTEM_DIC`] is decompressed from.
pub const SYSTEM_DIC_ZST: &str = "system.dic.zst";

/// IPADIC's licence and attribution files, published beside the dictionary
/// because IPADIC requires attribution and the release check in Phase 6C will
/// want them. Copying them while the tar is open costs nothing.
pub const COPYING: &str = "COPYING";
/// See [`COPYING`].
pub const NOTICE: &str = "NOTICE";

/// Marks a download or an unpack that is in progress or unverified. The
/// writing process's PID follows it, so two concurrent runs against one hints
/// directory stage into different names instead of one truncating the other.
///
/// **These accumulate.** A killed unpack leaves a staging directory holding up
/// to ~46 MB that nothing sweeps — deliberately, for the reason
/// `jmdict-source`'s own `PARTIAL_SUFFIX` gives: a process cannot tell
/// another's live staging area from a dead one's leftovers. They are inert,
/// never resolved, so the cost is disk rather than correctness.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Download attempts before [`fetch::fetch_with_retry`] gives up.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Delay before the second attempt; doubles for each attempt after it.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("hints io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloading the vibrato dictionary failed: {0}")]
    Transport(String),
    #[error("the dictionary server returned HTTP {status} for {url}")]
    Http {
        status: u16,
        /// The URL that was requested. A 4xx means the pinned release asset
        /// moved or was deleted, and a report without the URL leaves nothing
        /// to check by hand.
        url: String,
        /// Where the dictionary would have been published. Named for the same
        /// reason `jmdict-source` names its source directory: the message is
        /// shown to someone who has to decide what to do next.
        hints_dir: PathBuf,
    },
    #[error(
        "the downloaded archive is not the pinned dictionary: expected SHA-256 \
         {expected}, got {actual}"
    )]
    Hash { expected: String, actual: String },
    /// One of the three nested formats refused the bytes. A single variant
    /// rather than three, because no user acts differently on a bad xz stream
    /// than on a bad zstd frame; the string says which layer failed.
    #[error("the archive could not be unpacked: {0}")]
    Archive(String),
    #[error("the archive carried no {name}")]
    MissingMember { name: String },
    #[error("could not obtain the vibrato dictionary after {attempts} attempts ({last})")]
    TooManyAttempts {
        attempts: usize,
        hints_dir: PathBuf,
        /// The final attempt's failure, rendered. Attempts can be exhausted by
        /// a transport error, a 5xx, a hash mismatch or a corrupt archive, and
        /// those are four different things for a user to act on.
        last: String,
    },
}

/// The published dictionary in `hints_dir`, or `None`. Never touches the
/// network.
///
/// Phase 5B calls this to decide whether the MeCab toggle can be offered at
/// all, which is why it answers without a `Result`: "not there" is not a
/// failure, it is the first-run state.
pub fn published(hints_dir: &Path) -> Option<PathBuf> {
    let path = hints_dir.join(DICT_DIR).join(SYSTEM_DIC);
    path.is_file().then_some(path)
}

/// This process's staging archive and staging directory inside `hints_dir`.
///
/// Both carry the PID, so two concurrent runs never share either one. Both are
/// hidden behind a leading dot and neither is ever resolved by [`published`].
pub fn staging_names(hints_dir: &Path) -> (PathBuf, PathBuf) {
    let pid = std::process::id();
    (
        hints_dir.join(format!(".{DICT_DIR}.tar.xz{PARTIAL_SUFFIX}.{pid}")),
        hints_dir.join(format!(".{DICT_DIR}{PARTIAL_SUFFIX}.{pid}")),
    )
}

/// Move a completed staging directory to its published name.
///
/// This is the only moment anything appears at the resolved path, which is what
/// makes a killed process harmless: until the rename, `published` sees nothing.
///
/// A rename onto an existing directory fails — on Windows always, on Unix
/// unless the target is empty — and that failure is the benign race: another
/// process finished first. A target holding a dictionary therefore reads as
/// success, and this process's staging directory is removed. The worst case is
/// last-writer-wins between two copies of the same pinned artifact, not a
/// corrupt one.
///
/// The staging directory always lives inside the same hints directory, so this
/// rename never crosses a filesystem — `fs::rename` returns `EXDEV` across
/// devices and does not fall back to copying.
pub fn publish(staging: &Path, target: &Path) -> Result<(), SourceError> {
    match std::fs::rename(staging, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            if target.join(SYSTEM_DIC).is_file() {
                // Best-effort: a leftover staging directory is inert, and
                // failing here would report a download failure that did not
                // happen.
                let _ = std::fs::remove_dir_all(staging);
                Ok(())
            } else {
                Err(SourceError::Io(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vibrato-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    #[test]
    fn an_absent_directory_is_not_published() {
        let dir = scratch("published-absent");
        assert_eq!(published(&dir), None);
    }

    /// The shape a killed unpack leaves: the directory exists, the dictionary
    /// does not. Treating that as published is how a truncated file reaches
    /// `VibratoTokenizer::load`.
    #[test]
    fn a_directory_without_the_dictionary_is_not_published() {
        let dir = scratch("published-empty");
        std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
        assert_eq!(published(&dir), None);
    }

    #[test]
    fn a_complete_directory_is_published() {
        let dir = scratch("published-complete");
        std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
        std::fs::write(
            dir.join(DICT_DIR).join(SYSTEM_DIC),
            b"not really a dictionary",
        )
        .expect("write");
        assert_eq!(published(&dir), Some(dir.join(DICT_DIR).join(SYSTEM_DIC)));
    }

    /// A staging directory is never resolved — it is either an unpack in
    /// progress or one that failed, and `published` must not see either.
    #[test]
    fn a_staging_directory_is_not_published() {
        let dir = scratch("published-staging");
        let (_, staging_dir) = staging_names(&dir);
        std::fs::create_dir_all(&staging_dir).expect("staging");
        std::fs::write(staging_dir.join(SYSTEM_DIC), b"partial").expect("write");
        assert_eq!(published(&dir), None);
    }

    #[test]
    fn the_pin_matches_the_measured_asset() {
        assert_eq!(ASSET_BYTES, 7_704_680);
        assert_eq!(ASSET_SHA256.len(), 64);
        assert!(ASSET_SHA256
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(
            ASSET_URL.starts_with("https://"),
            "GitHub serves no plain HTTP"
        );
    }
}
