// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The archive half: unpacking and publishing, with no socket in sight.

mod support;

use support::*;
use vibrato_source::{extract, publish, SourceError, COPYING, DICT_DIR, NOTICE, SYSTEM_DIC};

/// The central proof of this task: what comes out of the unpacker is a file
/// vibrato itself will load.
#[test]
fn a_real_shaped_archive_yields_a_loadable_dictionary() {
    let dir = scratch("unpack-happy");
    let archive = write(&dir, "asset.tar.xz", &real_shaped_archive());
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    let dic = staging.join(SYSTEM_DIC);
    assert!(dic.is_file(), "the dictionary must be written decompressed");
    let file = std::fs::File::open(&dic).expect("open");
    vibrato::Dictionary::read(std::io::BufReader::new(file))
        .expect("vibrato must read what the unpacker wrote");

    assert!(staging.join(COPYING).is_file(), "attribution must survive");
    assert!(staging.join(NOTICE).is_file(), "attribution must survive");
}

#[test]
fn bytes_that_are_not_xz_are_an_archive_error_not_an_io_error() {
    let dir = scratch("unpack-notxz");
    let archive = write(&dir, "asset.tar.xz", b"<html><body>404</body></html>");

    let err = extract::unpack_into(&archive, &dir.join("staging")).expect_err("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
}

#[test]
fn an_archive_without_the_dictionary_names_the_missing_member() {
    let dir = scratch("unpack-nomember");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[("ipadic-mecab-2_7_0/COPYING", b"licence".to_vec())]),
    );

    let err = extract::unpack_into(&archive, &dir.join("staging")).expect_err("must fail");
    match err {
        SourceError::MissingMember { name } => assert_eq!(name, "system.dic.zst"),
        other => panic!("got {other:?}"),
    }
}

/// The trust boundary. The archive is pinned and hashed, but the extractor
/// must not depend on that: nothing derived from an entry's own path may reach
/// the filesystem, or a later phase that relaxes the pin inherits a hole.
#[test]
fn a_traversing_entry_writes_nothing_outside_the_staging_directory() {
    let dir = scratch("unpack-traversal");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[
            ("../escaped.txt", b"nope".to_vec()),
            ("ipadic-mecab-2_7_0/system.dic.zst", zstd(&dictionary_bytes())),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(!dir.join("escaped.txt").exists(), "an entry escaped staging");
    assert!(staging.join(SYSTEM_DIC).is_file(), "the dictionary must still land");
}

/// A future release adding a README must not fail the fetch.
#[test]
fn an_unexpected_member_is_skipped_not_fatal() {
    let dir = scratch("unpack-extra");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[
            ("ipadic-mecab-2_7_0/README.md", b"hello".to_vec()),
            ("ipadic-mecab-2_7_0/system.dic.zst", zstd(&dictionary_bytes())),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(!staging.join("README.md").exists(), "only the three known names");
    assert!(staging.join(SYSTEM_DIC).is_file());
}

/// A member whose zstd frame is damaged. The dictionary is the one member
/// whose content is decompressed, so this is the only place a corrupt frame
/// can surface.
#[test]
fn a_corrupt_zstd_member_is_an_archive_error() {
    let dir = scratch("unpack-badzstd");
    let mut packed = zstd(&dictionary_bytes());
    let n = packed.len();
    packed[n / 2] ^= 0xff;
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[("ipadic-mecab-2_7_0/system.dic.zst", packed)]),
    );

    let err = extract::unpack_into(&archive, &dir.join("staging")).expect_err("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
}

#[test]
fn publishing_renames_the_staging_directory_into_place() {
    let dir = scratch("publish-happy");
    let staging = dir.join("staging");
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"dictionary").expect("write");

    publish(&staging, &dir.join(DICT_DIR)).expect("publish");

    assert!(dir.join(DICT_DIR).join(SYSTEM_DIC).is_file());
    assert!(!staging.exists(), "the staging directory must not survive");
}

/// The benign race: another process published first. The rename fails on
/// Windows against an existing directory, and that must read as success, not
/// as a failed download.
#[test]
fn publishing_over_an_existing_dictionary_succeeds_and_clears_staging() {
    let dir = scratch("publish-race");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the winner").expect("write");

    let staging = dir.join("staging");
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the loser").expect("write");

    publish(&staging, &target).expect("a published rival is not a failure");

    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("read"),
        b"the winner",
        "the already-published copy must win"
    );
    assert!(!staging.exists(), "the losing staging directory must be removed");
}
