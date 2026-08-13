// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The download path, driven against a real socket.
//!
//! No mock framework and no network: a `TcpListener` on port 0 answers queued
//! requests with hardcoded HTTP/1.1 responses.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jmdict_source::{SourceError, PARTIAL_SUFFIX, SOURCE_FILE};

fn scratch(name: &str) -> PathBuf {
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

fn http(status: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

/// Serve `responses` in order, one per connection, then stop. Returns the URL
/// and a handle yielding the number of requests actually received.
fn serve(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || {
        let mut served = 0usize;
        for response in responses {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    // Drain the request head so the client sees a clean reply
                    // rather than a reset.
                    let mut buf = [0u8; 2048];
                    let _ = stream.read(&mut buf);
                    let _ = stream.write_all(&response);
                    let _ = stream.flush();
                    served += 1;
                }
                Err(_) => break,
            }
        }
        served
    });
    (format!("http://{addr}/{SOURCE_FILE}"), handle)
}

/// Accept one connection and drop it without replying.
fn serve_reset() -> (String, std::thread::JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let handle = std::thread::spawn(move || match listener.accept() {
        Ok((stream, _)) => {
            drop(stream);
            1
        }
        Err(_) => 0,
    });
    (format!("http://{addr}/{SOURCE_FILE}"), handle)
}

/// True if `dir` contains any leftover staging file.
///
/// Before Fix 1, the staging name was fixed (`{SOURCE_FILE}{PARTIAL_SUFFIX}`)
/// and a test could check that one exact path. It now embeds the writing
/// process's PID, so there is no single fixed name left to check — this
/// scans for any entry whose name contains `PARTIAL_SUFFIX` instead.
fn any_staging_file_exists(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().contains(PARTIAL_SUFFIX))
        })
        // An absent directory has no staging file in it, by construction.
        .unwrap_or(false)
}

const XML: &[u8] = b"<?xml version=\"1.0\"?><JMdict></JMdict>";

#[test]
fn a_good_download_lands_at_the_resolved_name() {
    let dir = scratch("dl-ok");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let path = jmdict_source::fetch::fetch_from(&url, &dir).expect("fetch");

    assert_eq!(path, dir.join(SOURCE_FILE));
    assert!(path.exists(), "archive missing");
    assert!(
        !any_staging_file_exists(&dir),
        "no staging file remains after success"
    );
    assert_eq!(server.join().expect("join"), 1);
}

/// Also the home for Fix 3's message check: a 4xx is the most likely
/// permanent real-world failure (EDRDG renaming or moving the archive), so
/// the rendered message must name the URL it tried and the escape hatch
/// (the source directory and [`SOURCE_FILE`]), not just the status code.
#[test]
fn a_404_fails_and_leaves_nothing_behind() {
    let dir = scratch("dl-404");
    let (url, server) = serve(vec![http("404 Not Found", b"nope")]);

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(
        matches!(err, SourceError::Http { status: 404, .. }),
        "got {err:?}"
    );
    let rendered = err.to_string();
    assert!(
        rendered.contains(&url),
        "the message must name the URL it tried: {rendered}"
    );
    assert!(
        rendered.contains(SOURCE_FILE),
        "the message must name the escape-hatch filename: {rendered}"
    );
    assert!(
        rendered.contains(&dir.display().to_string()),
        "the message must name the source directory: {rendered}"
    );
    assert!(!dir.join(SOURCE_FILE).exists(), "a 404 body was published");
    assert!(
        !any_staging_file_exists(&dir),
        "no staging file remains after failure"
    );
    assert_eq!(server.join().expect("join"), 1);
}

/// The captive-portal case, end to end: HTTP 200, correct Content-Length,
/// HTML body. Verification must stop it before the rename.
#[test]
fn an_html_200_never_reaches_the_resolved_name() {
    let dir = scratch("dl-html");
    let body = b"<html><body>Login required</body></html>";
    let (url, server) = serve(vec![http("200 OK", body)]);

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(!matches!(err, SourceError::Http { .. }), "got {err:?}");
    assert!(
        !dir.join(SOURCE_FILE).exists(),
        "an HTML login page was published as the dictionary"
    );
    assert!(
        !any_staging_file_exists(&dir),
        "no staging file remains after failure"
    );
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_truncated_archive_never_reaches_the_resolved_name() {
    let dir = scratch("dl-trunc");
    let full = gz(&XML.repeat(50));
    let (url, server) = serve(vec![http("200 OK", &full[..full.len() / 2])]);

    jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(!dir.join(SOURCE_FILE).exists());
    assert!(
        !any_staging_file_exists(&dir),
        "no staging file remains after failure"
    );
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_dropped_connection_is_a_transport_error() {
    let dir = scratch("dl-reset");
    let (url, server) = serve_reset();

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(matches!(err, SourceError::Transport(_)), "got {err:?}");
    assert!(!dir.join(SOURCE_FILE).exists());
    let _ = server.join();
}

#[test]
fn the_source_directory_is_created_if_absent() {
    let dir = scratch("dl-mkdir");
    let nested = dir.join("does").join("not").join("exist");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let path = jmdict_source::fetch::fetch_from(&url, &nested).expect("fetch");

    assert!(path.exists());
    assert_eq!(server.join().expect("join"), 1);
}

/// Fix 1's mechanism, demonstrated rather than raced. A genuine two-process
/// race is timing-dependent and not exercised here — building a harness for
/// it would be disproportionate to what needs proving. What this test does
/// prove, deterministically: (1) two different PIDs compute two different
/// staging paths for the same `source_dir`, which is the property the fix
/// depends on; and (2) a real download from *this* process leaves a staging
/// file bearing a *different* PID completely untouched, because `fetch_from`
/// only ever creates or removes the one path it computed for itself.
///
/// What this does NOT prove: that a real concurrent race between two live
/// processes resolves to "last verified writer wins" rather than some other
/// interleaving. That would require two processes actually racing on the
/// same clock, which is exactly the kind of flaky, hard-to-reproduce test
/// this fix wave was asked to avoid building.
#[test]
fn a_stray_staging_file_from_another_pid_is_never_touched() {
    let dir = scratch("dl-pid-unique");
    let real_pid = std::process::id();
    let other_pid = real_pid.wrapping_add(1).max(1); // some PID that is not ours
    let mine = dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}.{real_pid}"));
    let foreign = dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}.{other_pid}"));
    assert_ne!(
        mine, foreign,
        "two processes' staging paths must not collide"
    );
    std::fs::write(&foreign, b"leftover from a different process").expect("write");

    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);
    let path = jmdict_source::fetch::fetch_from(&url, &dir).expect("fetch");

    assert_eq!(path, dir.join(SOURCE_FILE));
    assert!(
        foreign.exists(),
        "a staging file left by a different PID must survive our own fetch"
    );
    assert!(
        any_staging_file_exists(&dir),
        "the foreign leftover must still be detected as a staging file"
    );
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_transient_failure_is_retried_and_can_succeed() {
    let dir = scratch("retry-recover");
    let (url, server) = serve(vec![
        http("503 Service Unavailable", b"busy"),
        http("200 OK", &gz(XML)),
    ]);

    let path = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO).expect("fetch");

    assert!(path.exists());
    assert_eq!(server.join().expect("join"), 2, "the 503 was not retried");
}

#[test]
fn a_404_is_not_retried() {
    let dir = scratch("retry-404");
    // Three responses queued, but only one may be consumed.
    let (url, server) = serve(vec![
        http("404 Not Found", b"nope"),
        http("200 OK", &gz(XML)),
        http("200 OK", &gz(XML)),
    ]);

    let err =
        jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO).expect_err("must fail");

    assert!(
        matches!(err, SourceError::Http { status: 404, .. }),
        "got {err:?}"
    );
    assert!(!dir.join(SOURCE_FILE).exists());
    // The server thread is still blocked in accept(); dropping the handle
    // avoids a timing-dependent join. What matters is that the client stopped
    // after one request, which the error variant already proves.
    drop(server);
}

#[test]
fn exhausting_the_attempts_reports_the_last_failure_and_the_escape_hatch() {
    let dir = scratch("retry-exhaust");
    let responses: Vec<Vec<u8>> = (0..jmdict_source::DOWNLOAD_ATTEMPTS)
        .map(|_| http("500 Internal Server Error", b"x"))
        .collect();
    let (url, server) = serve(responses);

    let err =
        jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO).expect_err("must fail");

    let rendered = err.to_string();
    match err {
        SourceError::TooManyAttempts {
            attempts,
            source_dir,
            last,
        } => {
            assert_eq!(attempts, jmdict_source::DOWNLOAD_ATTEMPTS);
            assert_eq!(source_dir, dir);
            assert!(!last.is_empty(), "the last failure was not recorded");
        }
        other => panic!("expected TooManyAttempts, got {other:?}"),
    }
    assert!(rendered.contains(SOURCE_FILE), "got: {rendered}");
    assert!(
        rendered.contains(&dir.display().to_string()),
        "the message must name the directory: {rendered}"
    );
    assert_eq!(
        server.join().expect("join"),
        jmdict_source::DOWNLOAD_ATTEMPTS,
        "wrong number of attempts"
    );
}

/// A corrupt body is retried, because the bytes were bad and the URL was not.
#[test]
fn a_corrupt_archive_is_retried() {
    let dir = scratch("retry-corrupt");
    let (url, server) = serve(vec![
        http("200 OK", b"<html>not gzip</html>"),
        http("200 OK", &gz(XML)),
    ]);

    let path = jmdict_source::fetch::fetch_with_retry(&url, &dir, Duration::ZERO).expect("fetch");

    assert!(path.exists());
    assert_eq!(
        server.join().expect("join"),
        2,
        "the corrupt body was not retried"
    );
}

#[test]
fn the_default_attempt_count_is_three() {
    assert_eq!(jmdict_source::DOWNLOAD_ATTEMPTS, 3);
}

/// A killed download leaves a `.partial`. It must not satisfy `resolve`, or a
/// truncation gets fed to the parser as though hand-placed. The listener
/// answers 404, so the fall-through to the download ran (and failed on its
/// own account) rather than the `.partial` being mistaken for the archive.
#[test]
fn a_partial_alone_does_not_satisfy_resolve() {
    let dir = scratch("resolve-partial");
    // Any name containing `PARTIAL_SUFFIX` proves the point; the exact
    // trailing PID that a real `fetch_from` would append is not the property
    // under test here — `resolve` only ever looks for an exact `SOURCE_FILE`
    // match, so no staging-shaped name should satisfy it.
    std::fs::write(dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}")), gz(XML)).expect("write");
    let (url, server) = serve(vec![http("404 Not Found", b"nope")]);

    // `expect_err` needs `Box<dyn BufRead>: Debug`, which it is not — go
    // through `Option` instead, as `lib.rs`'s own tests do for the same
    // reason.
    let err = jmdict_source::resolve_from(&url, &dir, Duration::ZERO)
        .err()
        .expect("a .partial must not resolve");
    assert!(
        err.get_ref().is_some(),
        "the SourceError was lost inside the io::Error"
    );
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn the_typed_error_survives_the_io_wrapper() {
    let dir = scratch("resolve-typed");
    let (url, server) = serve(vec![http("404 Not Found", b"nope")]);

    let err = jmdict_source::resolve_from(&url, &dir, Duration::ZERO)
        .err()
        .expect("no local file and a failing download");
    let inner = err.get_ref().and_then(|e| e.downcast_ref::<SourceError>());
    assert!(inner.is_some(), "expected a SourceError inside: {err:?}");
    assert_eq!(server.join().expect("join"), 1);
}

/// The coverage the injection seam was added for: nothing else in the suite
/// proves `resolve`'s fetch branch succeeds end to end.
#[test]
fn a_missing_archive_is_downloaded_and_then_opened() {
    let dir = scratch("resolve-fetch");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let mut reader = jmdict_source::resolve_from(&url, &dir, Duration::ZERO).expect("resolve");
    let mut got = Vec::new();
    reader.read_to_end(&mut got).expect("read");

    assert_eq!(got, XML);
    assert!(
        dir.join(SOURCE_FILE).exists(),
        "the download was not published"
    );
    assert_eq!(server.join().expect("join"), 1);
}
