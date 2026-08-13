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
    // `io::sink` discards the output; only the error matters. This is a
    // decode failure, not a local I/O failure, even though flate2 reports it
    // as `std::io::Error` — the file opened and is readable, but its content
    // is not a valid gzip stream. Mapped to `Corrupt` rather than left to
    // `#[from]`'s `Io`, so the retry policy in `fetch_with_retry` can tell a
    // bad body (retry) apart from a local write failure (do not retry).
    std::io::copy(&mut decoder, &mut std::io::sink())
        .map_err(|e| SourceError::Corrupt(e.to_string()))?;
    Ok(())
}

/// Global timeout for one download attempt, via `ureq`'s own
/// `timeout_global` — "end-to-end, from DNS lookup to finishing reading the
/// response body. Thus it covers all other timeouts" (`ureq`'s doc for it).
/// Every field of `ureq` 3.2.1's `Timeouts` defaults to `None` except
/// `await_100`, so without this a server that completes the handshake and
/// then stalls mid-body hangs the call forever — and with it, `fetch` and
/// therefore `ensure_dictionary`'s source closure, with no error and no
/// retry.
///
/// 120 seconds, and note this bounds the **whole call**, not idle time:
/// `timeout_global` starts counting at the request, so it caps total download
/// duration rather than only detecting a stall. The measured archive is
/// 10,545,887 bytes (spec's resolved facts) = ~84 Mbit, so this sets an
/// effective sustained-throughput floor of ~0.70 Mbit/s, below which every
/// attempt times out and the user gets `TooManyAttempts`. A 1 Mbit/s link
/// needs ~84 s of the 120 s budget — real headroom, but far less than it
/// looks. Raise this before assuming a slow connection is at fault.
///
/// A stall surfaces as `ureq::Error::Timeout`, which the existing `Err(e) =>
/// Transport(...)` fallthrough below already retries — this composes with
/// [`fetch_with_retry`] rather than needing its own handling.
const DOWNLOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Upper bound on the response body a download will accept, enforced via
/// `ureq`'s `BodyWithConfig::limit`. The transport is plain HTTP with no
/// authenticity guarantee (spec §10 — EDRDG's certificate fails
/// subject-name validation), so nothing else bounds how much a misbehaving
/// or malicious response could stream to disk before `verify_archive` ever
/// runs.
///
/// 32 MiB: a little over 3x the measured 10,545,887-byte archive. Generous
/// headroom for the dictionary to grow across future EDRDG releases, while
/// still bounding a runaway body to tens of megabytes rather than an
/// unbounded amount.
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;

/// Download `url` into `source_dir` and publish it as [`crate::SOURCE_FILE`].
///
/// One attempt. [`fetch`] adds the retry policy.
///
/// Stages into `<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>`, verifies it, and only
/// then renames — so a file at the resolved name is always a complete, valid
/// archive. Two failures make that necessary: a killed process leaves a
/// truncation, and a proxy can answer 200 with an HTML page of the right
/// length. Either one, published, is indistinguishable from a hand-placed file
/// and fails identically on every subsequent run.
///
/// The PID is what makes this safe when two processes race against the same
/// `source_dir` — `fetch-source` against `ensure-dictionary --source-dir`, or
/// two of either. Each stages into its own file, so neither can truncate the
/// other's write mid-flight, and a successful `fs::rename` can never publish
/// a second process's still-partial bytes under the first process's name.
/// The remaining worst case is last-verified-writer-wins, not corruption.
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
    // The PID makes the staging name unique per process. Without it, two
    // concurrent invocations against the same `source_dir` share one inode:
    // `File::create` truncates whichever one runs second, and a `rename`
    // from either side can publish the other's still-partial bytes. Do not
    // simplify this back to a fixed name.
    let staging = source_dir.join(format!(
        "{SOURCE_FILE}{PARTIAL_SUFFIX}.{}",
        std::process::id()
    ));

    match download_and_verify(
        url,
        source_dir,
        &staging,
        DOWNLOAD_TIMEOUT,
        MAX_ARCHIVE_BYTES,
    ) {
        Ok(()) => {
            std::fs::rename(&staging, &target)?;
            Ok(target)
        }
        Err(e) => {
            // Best-effort cleanup: the download already failed, and a leftover
            // `.partial.<pid>` is never resolved, so a failure here changes
            // nothing a caller can act on.
            let _ = std::fs::remove_file(&staging);
            Err(e)
        }
    }
}

/// Write `url`'s body to `staging`, then prove it is a valid archive.
///
/// `timeout` and `max_bytes` are parameters (rather than reading
/// [`DOWNLOAD_TIMEOUT`] and [`MAX_ARCHIVE_BYTES`] directly) purely so unit
/// tests can exercise the real timeout and limiting mechanisms against a
/// tiny stall and a tiny body instead of the production values.
fn download_and_verify(
    url: &str,
    source_dir: &Path,
    staging: &Path,
    timeout: std::time::Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    let mut response = match ureq::get(url)
        .config()
        .timeout_global(Some(timeout))
        .build()
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(SourceError::Http {
                status,
                url: url.to_string(),
                source_dir: source_dir.to_path_buf(),
            })
        }
        Err(e) => return Err(SourceError::Transport(e.to_string())),
    };
    {
        let mut body = response.body_mut().with_config().limit(max_bytes).reader();
        let mut file = std::io::BufWriter::new(std::fs::File::create(staging)?);
        // A mid-transfer disconnect, a stall past `DOWNLOAD_TIMEOUT`, and a
        // body over `max_bytes` all surface here as an `io::Error` (the last
        // one via `ureq::Error::BodyExceedsLimit`, wrapped by `Error::into_io`
        // since `LimitReader` has no other way to signal it through `Read`).
        // All three are transport failures rather than a corrupt archive —
        // the retry policy in `fetch_with_retry` treats them the same way,
        // but the message should not lie about which layer failed.
        std::io::copy(&mut body, &mut file).map_err(|e| SourceError::Transport(e.to_string()))?;
        std::io::Write::flush(&mut file)?;
    }

    verify_archive(staging)
}

/// The published archive. `pub(crate)` rather than private only so `lib.rs`'s
/// `resolve` can hand it to [`crate::resolve_from`]; not `pub`, since exposing
/// it crate-externally would invite a caller to fetch it directly and skip
/// the staging and verification in [`fetch_from`], which are the only things
/// standing between a proxy's error page and the resolved name. EDRDG serves
/// no usable HTTPS — the certificate fails subject-name validation — so this
/// is plain HTTP by necessity, which is precisely why verification is
/// mandatory rather than defensive.
pub(crate) const JMDICT_URL: &str = "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz";

/// Download the archive into `source_dir`, retrying transient failures.
pub fn fetch(source_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(JMDICT_URL, source_dir, crate::RETRY_BACKOFF)
}

/// [`fetch`] with the URL and backoff injected, so tests can point at a local
/// listener without sleeping.
pub fn fetch_with_retry(
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> Result<PathBuf, SourceError> {
    let mut delay = backoff;
    let mut last = String::new();

    for attempt in 1..=crate::DOWNLOAD_ATTEMPTS {
        match fetch_from(url, source_dir) {
            Ok(path) => return Ok(path),
            // A 4xx will not change on a retry, and neither will a local write
            // failure. Surface both immediately rather than sleeping first.
            Err(
                e @ SourceError::Http {
                    status: 400..=499, ..
                },
            ) => return Err(e),
            Err(e @ SourceError::Io(_)) => return Err(e),
            Err(e) => {
                // `Http`'s own message carries the URL and the hand-placement
                // escape hatch, because an immediate 4xx never reaches
                // `TooManyAttempts`. Here it does reach it, and
                // `TooManyAttempts` appends that same clause itself — so keep
                // only the status, or the user reads the instruction twice in
                // one sentence.
                last = match &e {
                    SourceError::Http { status, .. } => format!("HTTP {status}"),
                    other => other.to_string(),
                };
                if attempt < crate::DOWNLOAD_ATTEMPTS {
                    std::thread::sleep(delay);
                    delay = delay.saturating_mul(2);
                }
            }
        }
    }

    Err(SourceError::TooManyAttempts {
        attempts: crate::DOWNLOAD_ATTEMPTS,
        source_dir: source_dir.to_path_buf(),
        last,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

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

    /// Exercises the real `ureq` limiting mechanism — not a mock — against a
    /// tiny body and a tiny limit, rather than transferring `MAX_ARCHIVE_BYTES`
    /// worth of data just to prove the same code path. `download_and_verify`
    /// takes the limit as a parameter for exactly this reason.
    #[test]
    fn a_body_over_the_limit_is_rejected_not_silently_truncated() {
        let dir = scratch("download-bodylimit");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let body = b"this response body is longer than the tiny limit below";
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes());
                let _ = stream.write_all(body);
                let _ = stream.flush();
            }
        });

        let url = format!("http://{addr}/archive");
        let staging = dir.join("staging");
        // A limit far below `body`'s length: enough to prove rejection without
        // needing to move `MAX_ARCHIVE_BYTES` of data through a test socket.
        let err = download_and_verify(&url, &dir, &staging, DOWNLOAD_TIMEOUT, 8)
            .expect_err("a body over the limit must not verify as Ok");
        // `Transport`, specifically. `ureq`'s `LimitReader` errors on the read
        // that would exceed the cap, *before* touching the inner reader, so the
        // failure arrives through `io::copy` and never reaches
        // `verify_archive`. Asserting only `is_err()` would not distinguish
        // that from the failure this test exists to catch: a silently truncated
        // body that `verify_archive` then rejects as `Corrupt` — which looks
        // identical from outside while meaning the limit did nothing.
        assert!(matches!(err, SourceError::Transport(_)), "got {err:?}");
        handle.join().expect("server thread");
    }

    /// Exercises the real `ureq` timeout mechanism against a tiny injected
    /// value and a short stall, rather than trusting the doc comment alone
    /// (or, worse, waiting out `DOWNLOAD_TIMEOUT`'s real 120s). Before Fix 2,
    /// `ureq`'s defaults leave every timeout unset except `await_100`, so a
    /// server that accepts the connection and then never answers hung this
    /// call forever; this proves it now returns a real error instead.
    #[test]
    fn a_stalled_server_is_rejected_by_the_timeout_not_left_hanging() {
        let dir = scratch("download-timeout");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                // Stall well past the tiny timeout given to `download_and_verify`
                // below before sending anything back.
                std::thread::sleep(std::time::Duration::from_millis(300));
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            }
        });

        let url = format!("http://{addr}/archive");
        let staging = dir.join("staging");
        let err = download_and_verify(
            &url,
            &dir,
            &staging,
            std::time::Duration::from_millis(50),
            MAX_ARCHIVE_BYTES,
        )
        .expect_err("a stalled server must not hang forever");
        assert!(!matches!(err, SourceError::Http { .. }), "got {err:?}");
        handle.join().expect("server thread");
    }
}
