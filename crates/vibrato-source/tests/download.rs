// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fetch half, against a local listener. Nothing here reaches the internet:
//! the real asset is fetched once, by hand, and recorded in the handoff.

mod support;

use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use support::*;
use vibrato_source::{
    fetch, published, resolve, resolve_from, SourceError, ASSET_BYTES, ASSET_SHA256, DICT_DIR,
    SYSTEM_DIC,
};

/// A one-shot server that answers every request with `status` and `body`,
/// counting the connections it accepted.
fn serve(
    status: u16,
    body: Vec<u8>,
    requests: Arc<AtomicUsize>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            requests.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let head = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });
    (format!("http://{addr}/asset.tar.xz"), handle)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The phase's central proof: bytes off a socket become a dictionary vibrato
/// loads, published under the resolved name.
#[test]
fn a_served_archive_is_downloaded_unpacked_and_published() {
    let dir = scratch("fetch-happy");
    let archive = real_shaped_archive();
    let hash = sha256_hex(&archive);
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, archive, requests.clone());

    let path = fetch::fetch_with_retry(&url, &hash, &dir, Duration::from_millis(1)).expect("fetch");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file)).expect("vibrato must read it");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "one attempt, no retries"
    );
}

/// Nothing at the resolved name may be a staging leftover.
#[test]
fn a_failed_fetch_leaves_nothing_published() {
    let dir = scratch("fetch-failed");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(
        &url,
        &sha256_hex(b"whatever"),
        &dir,
        Duration::from_millis(1),
    )
    .expect_err("must fail");

    assert!(
        matches!(err, SourceError::TooManyAttempts { .. }),
        "got {err:?}"
    );
    assert_eq!(published(&dir), None, "a failure must publish nothing");
}

#[test]
fn a_body_with_the_wrong_hash_is_rejected() {
    let dir = scratch("fetch-badhash");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let err = fetch::fetch_with_retry(
        &url,
        "0000000000000000000000000000000000000000000000000000000000000000",
        &dir,
        Duration::from_millis(1),
    )
    .expect_err("must fail");

    // A wrong hash is retried — a truncated or proxy-mangled body is exactly
    // what it detects, and that is worth a second attempt — so the terminal
    // error is `TooManyAttempts` carrying the hash failure as its `last`.
    match err {
        SourceError::TooManyAttempts { last, attempts, .. } => {
            assert_eq!(attempts, 3);
            assert!(last.contains("SHA-256"), "got {last}");
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(
        requests.load(Ordering::SeqCst),
        3,
        "a hash mismatch is retried"
    );
}

#[test]
fn a_404_fails_immediately_without_retrying() {
    let dir = scratch("fetch-404");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(404, b"gone".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(&url, &sha256_hex(b"x"), &dir, Duration::from_millis(1))
        .expect_err("must fail");

    match err {
        SourceError::Http {
            status,
            url: reported,
            ..
        } => {
            assert_eq!(status, 404);
            assert_eq!(reported, url, "the message must name what was requested");
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "a 4xx will not change on a retry"
    );
}

#[test]
fn a_5xx_exhausts_the_attempts() {
    let dir = scratch("fetch-500");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let err = fetch::fetch_with_retry(&url, &sha256_hex(b"x"), &dir, Duration::from_millis(1))
        .expect_err("must fail");

    assert!(
        matches!(err, SourceError::TooManyAttempts { attempts: 3, .. }),
        "got {err:?}"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

/// `resolve` must answer from disk when the dictionary is already published —
/// asserted by the server never being connected to, not by timing.
///
/// Driven through `resolve_from` rather than `resolve`, because `resolve`'s
/// fall-through is the live GitHub release: with no injected URL, a regression
/// in `published` would not fail this test, it would put a 7.7 MB download
/// inside the suite. The counter is what makes the doc sentence above true.
#[test]
fn resolve_returns_a_published_dictionary_without_downloading() {
    let dir = scratch("resolve-published");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    // An archive older than the slot. `resolve` looks for one on every call, so
    // this proves that lookup neither reaches the network nor clobbers a newer
    // slot on its way to the published short-circuit below.
    write(&dir, "stale.tar.xz", &real_shaped_archive());
    tick();
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let path = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect("resolve");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        0,
        "a published dictionary must be answered from disk"
    );

    // Only after the assertion above proves `published` still short-circuits is
    // it safe to run the production entry point, which has no local listener to
    // fall through to.
    assert_eq!(resolve(&dir).expect("resolve"), path);
    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"already here",
        "a stale archive must not have replaced the slot"
    );
}

/// The archive branch's central proof: an archive newer than the slot is
/// unpacked and published, and no socket is opened doing it.
#[test]
fn a_newer_hand_placed_archive_wins_without_touching_the_network() {
    let dir = scratch("resolve-archive-newer");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"the old one").expect("write");
    tick();
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let path = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect("resolve");

    assert_eq!(path, dir.join(DICT_DIR).join(SYSTEM_DIC));
    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file))
        .expect("the archive's dictionary must have replaced the old file");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        0,
        "the archive branch must not reach the network"
    );
}

/// The other direction, and the reason the rule is an mtime comparison rather
/// than "an archive present wins": the archive a user installed last month must
/// not re-unpack on every toggle.
#[test]
fn an_older_hand_placed_archive_loses_to_the_published_slot() {
    let dir = scratch("resolve-archive-older");
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());
    tick();
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, real_shaped_archive(), requests.clone());

    let path = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect("resolve");

    assert_eq!(
        std::fs::read(&path).expect("read"),
        b"already here",
        "an older archive must not replace the published slot"
    );
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}

/// A broken hand-placed archive is the common case now that the extractor's
/// caps refuse anything but an IPADIC-sized dictionary: this proves it costs
/// nothing when it happens. Bytes that are not xz fail `unpack_into` before
/// `publish_replacing` ever runs, so the slot's original contents survive,
/// the failed staging directory is swept rather than left behind holding
/// disk, and the network is never touched.
#[test]
fn a_broken_hand_placed_archive_leaves_the_published_slot_untouched() {
    let dir = scratch("resolve-archive-broken");
    std::fs::create_dir_all(dir.join(DICT_DIR)).expect("dict dir");
    std::fs::write(dir.join(DICT_DIR).join(SYSTEM_DIC), b"already here").expect("write");
    tick();
    write(&dir, "ipadic.tar.xz", b"not actually xz");

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let err = resolve_from(
        &url,
        &sha256_hex(b"never used"),
        &dir,
        Duration::from_millis(1),
    )
    .expect_err("an unxz-able archive must not resolve");

    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
    assert_eq!(
        std::fs::read(dir.join(DICT_DIR).join(SYSTEM_DIC)).expect("read"),
        b"already here",
        "a broken archive must not have replaced the published slot"
    );
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".partial") || name.contains(".backup"))
        .collect();
    assert!(leftovers.is_empty(), "got leftovers: {leftovers:?}");
    assert_eq!(
        requests.load(Ordering::SeqCst),
        0,
        "a broken archive must not fall through to the network"
    );
}

/// A file the user placed in their own config directory is already their
/// choice; hashing it could only refuse what they explicitly asked for. The
/// hash passed here matches nothing, and the archive is published anyway.
#[test]
fn a_hand_placed_archive_is_published_without_a_hash_check() {
    let dir = scratch("resolve-archive-nohash");
    write(&dir, "ipadic.tar.xz", &real_shaped_archive());

    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(500, b"boom".to_vec(), requests.clone());

    let path = resolve_from(
        &url,
        "0000000000000000000000000000000000000000000000000000000000000000",
        &dir,
        Duration::from_millis(1),
    )
    .expect("an unhashed hand-placed archive must publish");

    let file = std::fs::File::open(&path).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file)).expect("vibrato must read it");
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}

/// `ASSET_BYTES` is the cheap half of the pin: a truncating proxy or a
/// repointed release is named by a length, before 7.7 MB is hashed to say the
/// same thing in hex. Getting `Size` rather than `Hash` here is what proves the
/// order.
#[test]
fn a_body_of_the_wrong_length_is_rejected_by_the_size_pin() {
    let dir = scratch("fetch-badsize");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, b"far too short".to_vec(), requests.clone());

    let err = fetch::download_and_verify_for_test(
        &url,
        &dir.join("staging.tar.xz"),
        ASSET_SHA256,
        Duration::from_secs(30),
        32 * 1024 * 1024,
    )
    .expect_err("must fail");

    match err {
        SourceError::Size { expected, actual } => {
            assert_eq!(expected, ASSET_BYTES);
            assert_eq!(actual, 13);
        }
        other => panic!("got {other:?}"),
    }
}

/// A staging directory from a killed run is not a published dictionary, and a
/// fresh fetch must publish over it rather than resolve it.
#[test]
fn a_staging_directory_does_not_satisfy_resolve() {
    let dir = scratch("resolve-staging");
    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"partial").expect("write");

    assert_eq!(published(&dir), None);

    let archive = real_shaped_archive();
    let hash = sha256_hex(&archive);
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, archive, requests.clone());

    fetch::fetch_with_retry(&url, &hash, &dir, Duration::from_millis(1)).expect("fetch");
    assert!(published(&dir).is_some());
}

/// Exercises the real `ureq` limiting mechanism against a tiny limit rather
/// than moving 32 MiB through a test socket. Asserting only `is_err()` would
/// not distinguish a rejected body from a silently truncated one.
#[test]
fn a_body_over_the_limit_is_rejected_not_truncated() {
    let dir = scratch("fetch-limit");
    let requests = Arc::new(AtomicUsize::new(0));
    let (url, _server) = serve(200, vec![b'x'; 4096], requests.clone());

    let err = fetch::download_and_verify_for_test(
        &url,
        &dir.join("staging.tar.xz"),
        &sha256_hex(b"x"),
        Duration::from_secs(30),
        8,
    )
    .expect_err("must fail");

    assert!(matches!(err, SourceError::Transport(_)), "got {err:?}");
}

/// Before this, ureq's defaults leave every timeout unset except `await_100`,
/// so a server that accepts and never answers hangs the call forever.
#[test]
fn a_stalled_server_is_rejected_by_the_timeout() {
    let dir = scratch("fetch-stall");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            std::thread::sleep(Duration::from_millis(300));
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
        }
    });

    let err = fetch::download_and_verify_for_test(
        &format!("http://{addr}/asset.tar.xz"),
        &dir.join("staging.tar.xz"),
        &sha256_hex(b"x"),
        Duration::from_millis(50),
        32 * 1024 * 1024,
    )
    .expect_err("a stalled server must not hang forever");

    assert!(!matches!(err, SourceError::Http { .. }), "got {err:?}");
    handle.join().expect("server thread");
}
