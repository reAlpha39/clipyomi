// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Downloading the archive, and proving it is one before publishing it.

use std::path::{Path, PathBuf};

use crate::{SourceError, PARTIAL_SUFFIX, SOURCE_FILE};

/// Prove `path` is a complete, well-formed gzip archive by decoding all of it
/// and discarding the output.
///
/// Runs on a **downloaded** archive before it is renamed into place, never on a
/// hand-placed one — pre-verifying a user's own file would double the read on
/// every rebuild to guard against a mistake they made deliberately, and that
/// failure already surfaces correctly through the parser.
///
/// gzip carries a CRC32 and an uncompressed length in its trailer, both checked
/// at end of stream, so this rejects a corrupt checksum, a corrupt length, a
/// truncation, and a body that is not gzip at all. The measured behavior is
/// tabulated in the plan's "Resolved facts".
///
pub(crate) fn verify_archive(path: &Path) -> Result<(), SourceError> {
    let file = std::fs::File::open(path)?;
    let mut decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    // `io::sink` discards the output; only the error matters.
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    Ok(())
}

/// Download `url` into `source_dir` and publish it as [`crate::SOURCE_FILE`].
///
/// One attempt. [`fetch`] adds the retry policy.
///
/// Stages into `<SOURCE_FILE><PARTIAL_SUFFIX>`, verifies it, and only then
/// renames — so a file at the resolved name is always a complete, valid
/// archive. Two failures make that necessary: a killed process leaves a
/// truncation, and a proxy can answer 200 with an HTML page of the right
/// length. Either one, published, is indistinguishable from a hand-placed file
/// and fails identically on every subsequent run.
///
/// The staging file is always in `source_dir`, so the rename never crosses a
/// filesystem — `fs::rename` returns `EXDEV` across devices and never falls
/// back to copying.
///
/// **Not part of the supported surface.** It is separate from [`fetch`] so tests
/// can point it at a local listener; production callers use [`fetch`].
pub fn fetch_from(url: &str, source_dir: &Path) -> Result<PathBuf, SourceError> {
    std::fs::create_dir_all(source_dir)?;
    let target = source_dir.join(SOURCE_FILE);
    let staging = source_dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}"));

    match download_and_verify(url, &staging) {
        Ok(()) => {
            std::fs::rename(&staging, &target)?;
            Ok(target)
        }
        Err(e) => {
            // Best-effort cleanup: the download already failed, and a leftover
            // `.partial` is never resolved, so a failure here changes nothing
            // a caller can act on.
            let _ = std::fs::remove_file(&staging);
            Err(e)
        }
    }
}

/// Write `url`'s body to `staging`, then prove it is a valid archive.
fn download_and_verify(url: &str, staging: &Path) -> Result<(), SourceError> {
    let mut response = match ureq::get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => return Err(SourceError::Http { status }),
        Err(e) => return Err(SourceError::Transport(e.to_string())),
    };
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(SourceError::Http { status });
    }

    {
        let mut body = response.body_mut().as_reader();
        let mut file = std::io::BufWriter::new(std::fs::File::create(staging)?);
        // A mid-transfer disconnect surfaces here, and it is a transport
        // failure rather than a corrupt archive — the retry policy in Task 4
        // treats the two the same way, but the message should not lie.
        std::io::copy(&mut body, &mut file).map_err(|e| SourceError::Transport(e.to_string()))?;
        std::io::Write::flush(&mut file)?;
    }

    verify_archive(staging)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).expect("gz write");
        e.finish().expect("gz finish")
    }

    /// Big enough that a half-truncation lands mid-deflate-stream.
    fn payload() -> Vec<u8> {
        b"<?xml version=\"1.0\"?><JMdict></JMdict>".repeat(50)
    }

    fn write(dir: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join("archive");
        std::fs::write(&path, bytes).expect("write");
        path
    }

    #[test]
    fn an_intact_archive_verifies() {
        let dir = scratch("verify-ok");
        let path = write(&dir, &gz(&payload()));
        assert!(verify_archive(&path).is_ok());
    }

    #[test]
    fn a_corrupt_checksum_fails() {
        let dir = scratch("verify-crc");
        let mut bytes = gz(&payload());
        let n = bytes.len();
        bytes[n - 8] ^= 0xff; // CRC32 field
        let path = write(&dir, &bytes);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_corrupt_length_fails() {
        let dir = scratch("verify-isize");
        let mut bytes = gz(&payload());
        let n = bytes.len();
        bytes[n - 4] ^= 0xff; // ISIZE field
        let path = write(&dir, &bytes);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_missing_trailer_fails() {
        let dir = scratch("verify-notrailer");
        let bytes = gz(&payload());
        let path = write(&dir, &bytes[..bytes.len() - 8]);
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn a_mid_stream_truncation_fails() {
        let dir = scratch("verify-midstream");
        let bytes = gz(&payload());
        let path = write(&dir, &bytes[..bytes.len() / 2]);
        assert!(verify_archive(&path).is_err());
    }

    /// The captive-portal case: a proxy answers 200 with an HTML login page.
    /// This is the scenario that decides the design against a Content-Length
    /// check, and it fails at the gzip header, so rejection is nearly free.
    #[test]
    fn an_html_body_fails() {
        let dir = scratch("verify-html");
        let path = write(&dir, b"<html><body>Login required</body></html>");
        assert!(verify_archive(&path).is_err());
    }

    #[test]
    fn an_empty_file_fails() {
        let dir = scratch("verify-empty");
        let path = write(&dir, b"");
        assert!(verify_archive(&path).is_err());
    }
}
