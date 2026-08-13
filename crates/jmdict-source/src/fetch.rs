// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Downloading the archive, and proving it is one before publishing it.

use std::path::Path;

use crate::SourceError;

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
/// `allow(dead_code)`: Task 3 wires the only non-test caller into `fetch_from`;
/// until that lands, nothing outside this module's tests calls it.
#[allow(dead_code)]
pub(crate) fn verify_archive(path: &Path) -> Result<(), SourceError> {
    let file = std::fs::File::open(path)?;
    let mut decoder = flate2::read::GzDecoder::new(std::io::BufReader::new(file));
    // `io::sink` discards the output; only the error matters.
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    Ok(())
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
