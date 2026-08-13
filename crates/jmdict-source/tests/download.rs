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

fn partial(dir: &Path) -> PathBuf {
    dir.join(format!("{SOURCE_FILE}{PARTIAL_SUFFIX}"))
}

const XML: &[u8] = b"<?xml version=\"1.0\"?><JMdict></JMdict>";

#[test]
fn a_good_download_lands_at_the_resolved_name() {
    let dir = scratch("dl-ok");
    let (url, server) = serve(vec![http("200 OK", &gz(XML))]);

    let path = jmdict_source::fetch::fetch_from(&url, &dir).expect("fetch");

    assert_eq!(path, dir.join(SOURCE_FILE));
    assert!(path.exists(), "archive missing");
    assert!(!partial(&dir).exists(), "the .partial survived a success");
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_404_fails_and_leaves_nothing_behind() {
    let dir = scratch("dl-404");
    let (url, server) = serve(vec![http("404 Not Found", b"nope")]);

    let err = jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(
        matches!(err, SourceError::Http { status: 404 }),
        "got {err:?}"
    );
    assert!(!dir.join(SOURCE_FILE).exists(), "a 404 body was published");
    assert!(!partial(&dir).exists(), "the .partial survived a failure");
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
    assert!(!partial(&dir).exists(), "the .partial survived a failure");
    assert_eq!(server.join().expect("join"), 1);
}

#[test]
fn a_truncated_archive_never_reaches_the_resolved_name() {
    let dir = scratch("dl-trunc");
    let full = gz(&XML.repeat(50));
    let (url, server) = serve(vec![http("200 OK", &full[..full.len() / 2])]);

    jmdict_source::fetch::fetch_from(&url, &dir).expect_err("must fail");

    assert!(!dir.join(SOURCE_FILE).exists());
    assert!(!partial(&dir).exists());
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
        matches!(err, SourceError::Http { status: 404 }),
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
