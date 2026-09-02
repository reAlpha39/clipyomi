// SPDX-License-Identifier: MIT OR Apache-2.0

//! Downloading the archive, and proving it is the pinned one before unpacking.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::Digest;

use crate::{
    extract, publish, staging_names, SourceError, ASSET_BYTES, ASSET_SHA256, ASSET_URL, DICT_DIR,
    DOWNLOAD_ATTEMPTS, SYSTEM_DIC,
};

/// Global timeout for one download attempt, via `ureq`'s `timeout_global` —
/// "end-to-end, from DNS lookup to finishing reading the response body". Every
/// field of `ureq`'s `Timeouts` defaults to `None` except `await_100`, so
/// without this a server that completes the handshake and then stalls hangs
/// the call forever, with no error and no retry.
///
/// 120 seconds bounds the **whole call**, not idle time: the measured asset is
/// 7,704,680 bytes = ~62 Mbit, so this sets an effective sustained-throughput
/// floor of ~0.51 Mbit/s. Raise it before assuming a slow link is at fault.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound on the response body, via `ureq`'s `BodyWithConfig::limit`.
/// 32 MiB is a little over 4x the measured asset — headroom for a future
/// dictionary, while bounding a runaway body to tens of megabytes.
///
/// This is a first line, not the verification: the SHA-256 check below is what
/// decides whether the bytes are the pinned archive.
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;

/// Bytes hashed per read. Large enough that a 7.7 MB file is ~118 reads.
const HASH_CHUNK: usize = 64 * 1024;

/// An agent carrying this crate's TLS decision.
///
/// `rustls` with an **injected** provider: its defaults are `ring` (C and
/// assembly) or `aws-lc-rs` (whose `-sys` crate builds a BoringSSL fork), and
/// neither may enter a build that must stay free of a C toolchain.
/// `rustls-graviola` is pure Rust, taken under ISC. Roots come from the OS
/// trust store rather than a vendored bundle.
///
/// Do not "simplify" this to ureq's default features: that silently takes
/// `ring` back, and `ci.yml`'s TLS gate is the only thing that would notice.
fn agent(timeout: Duration) -> ureq::Agent {
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .unversioned_rustls_crypto_provider(Arc::new(rustls_graviola::default_provider()))
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();

    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .tls_config(tls)
            .build(),
    )
}

/// SHA-256 of `path`, lowercase hex.
///
/// Read in chunks rather than through `io::copy` into the hasher: the digest
/// crates' `io::Write` impl is feature-gated, and a 46 MB read has no business
/// depending on which features happened to resolve.
fn sha256_hex(path: &Path) -> Result<String, SourceError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; HASH_CHUNK];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// Download `url` to `staging`, then prove it is the archive `sha256` names.
///
/// `timeout` and `max_bytes` are parameters rather than reads of the constants
/// above purely so tests can exercise the real timeout and limiting mechanisms
/// against a tiny stall and a tiny body.
fn download_and_verify(
    url: &str,
    staging: &Path,
    sha256: &str,
    timeout: Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    let mut response = match agent(timeout).get(url).call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(status)) => {
            return Err(SourceError::Http {
                status,
                url: url.to_string(),
                hints_dir: staging
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
            })
        }
        Err(e) => return Err(SourceError::Transport(e.to_string())),
    };

    {
        let mut body = response.body_mut().with_config().limit(max_bytes).reader();
        let mut file = std::io::BufWriter::new(std::fs::File::create(staging)?);
        // A mid-transfer disconnect, a stall past `timeout`, and a body over
        // `max_bytes` all surface here as an `io::Error`. All three are
        // transport failures rather than a bad archive, and the message should
        // not lie about which layer failed.
        std::io::copy(&mut body, &mut file).map_err(|e| SourceError::Transport(e.to_string()))?;
        std::io::Write::flush(&mut file)?;
    }

    // The size pin is half of the same pin as the hash, so it only says
    // anything when the hash being checked is the pinned one — every test
    // driving this function names a body of its own instead. Checked first
    // because it is a `metadata` call rather than a 7.7 MB read, and because
    // "expected 7704680 bytes, got 1274" is a report a human can act on
    // without diffing two hex strings.
    if sha256 == ASSET_SHA256 {
        let actual = std::fs::metadata(staging)?.len();
        if actual != ASSET_BYTES {
            return Err(SourceError::Size {
                expected: ASSET_BYTES,
                actual,
            });
        }
    }

    let actual = sha256_hex(staging)?;
    if actual != sha256 {
        return Err(SourceError::Hash {
            expected: sha256.to_string(),
            actual,
        });
    }
    Ok(())
}

/// [`download_and_verify`] with the constants injected.
///
/// **Not part of the supported surface.** It exists so the timeout and body-cap
/// tests can drive the real `ureq` mechanisms against tiny values instead of
/// moving 32 MiB through a test socket.
#[doc(hidden)]
pub fn download_and_verify_for_test(
    url: &str,
    staging: &Path,
    sha256: &str,
    timeout: Duration,
    max_bytes: u64,
) -> Result<(), SourceError> {
    download_and_verify(url, staging, sha256, timeout, max_bytes)
}

/// One attempt: download, verify, unpack, publish. [`fetch_with_retry`] adds
/// the retry policy.
///
/// Both staging names come from [`staging_names`], which makes them unique per
/// call as well as per process — two concurrent runs against one hints
/// directory never share a path, and neither do two calls inside one process.
/// Neither name is ever resolved by [`crate::published`], so a killed run
/// cannot be mistaken for a finished one.
pub fn fetch_from(url: &str, sha256: &str, hints_dir: &Path) -> Result<PathBuf, SourceError> {
    std::fs::create_dir_all(hints_dir)?;
    let (staging_archive, staging_dir) = staging_names(hints_dir);

    let result = download_and_verify(
        url,
        &staging_archive,
        sha256,
        DOWNLOAD_TIMEOUT,
        MAX_ARCHIVE_BYTES,
    )
    .and_then(|()| extract::unpack_into(&staging_archive, &staging_dir))
    .and_then(|()| publish(&staging_dir, &hints_dir.join(DICT_DIR)));

    // Best-effort either way: the archive has served its purpose on success,
    // and on failure a leftover is inert — never resolved — so a cleanup
    // failure changes nothing a caller can act on.
    let _ = std::fs::remove_file(&staging_archive);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }

    result.map(|()| hints_dir.join(DICT_DIR).join(SYSTEM_DIC))
}

/// Download the dictionary into `hints_dir`, retrying transient failures.
pub fn fetch(hints_dir: &Path) -> Result<PathBuf, SourceError> {
    fetch_with_retry(ASSET_URL, ASSET_SHA256, hints_dir, crate::RETRY_BACKOFF)
}

/// [`fetch`] with the URL, hash and backoff injected, so tests can point at a
/// local listener without sleeping at the production backoff.
pub fn fetch_with_retry(
    url: &str,
    sha256: &str,
    hints_dir: &Path,
    backoff: Duration,
) -> Result<PathBuf, SourceError> {
    let mut delay = backoff;
    let mut last = String::new();

    for attempt in 1..=DOWNLOAD_ATTEMPTS {
        match fetch_from(url, sha256, hints_dir) {
            Ok(path) => return Ok(path),
            // A 4xx means the pinned asset moved or was deleted, and a local
            // I/O failure will not free disk or fix permissions. Surface both
            // immediately rather than sleeping first.
            Err(
                e @ SourceError::Http {
                    status: 400..=499, ..
                },
            ) => return Err(e),
            // A publish failure is a directory in the way, not a bad download:
            // the archive already arrived and unpacked. Retrying re-downloads
            // 7.7 MB into the same obstruction.
            Err(e @ (SourceError::Io(_) | SourceError::Publish { .. })) => return Err(e),
            Err(e) => {
                last = e.to_string();
                if attempt < DOWNLOAD_ATTEMPTS {
                    std::thread::sleep(delay);
                    delay = delay.saturating_mul(2);
                }
            }
        }
    }

    Err(SourceError::TooManyAttempts {
        attempts: DOWNLOAD_ATTEMPTS,
        hints_dir: hints_dir.to_path_buf(),
        last,
    })
}
