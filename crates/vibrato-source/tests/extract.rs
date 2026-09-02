// SPDX-License-Identifier: MIT OR Apache-2.0

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
        SourceError::MissingMember { name, archive: named } => {
            assert_eq!(name, "system.dic.zst");
            assert_eq!(named, archive, "the message must say which archive");
        }
        other => panic!("got {other:?}"),
    }
}

/// The trust boundary. The archive is pinned and hashed, but the extractor
/// must not depend on that: nothing derived from an entry's own path may reach
/// the filesystem, or a later phase that relaxes the pin inherits a hole.
///
/// The traversing entry is `../COPYING` specifically. A name the whitelist does
/// not know (`../escaped.txt`) is skipped as an unrecognised member and proves
/// nothing this file does not already prove elsewhere — it is the *whitelisted*
/// basename behind a `..` that makes basename extraction load-bearing.
#[test]
fn a_traversing_entry_writes_nothing_outside_the_staging_directory() {
    let dir = scratch("unpack-traversal");
    let archive = write(
        &dir,
        "asset.tar.xz",
        &tar_xz(&[
            ("../COPYING", b"escaped".to_vec()),
            (
                "ipadic-mecab-2_7_0/system.dic.zst",
                zstd(&dictionary_bytes()),
            ),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(
        !dir.join(COPYING).exists(),
        "a whitelisted basename behind `..` escaped staging"
    );
    assert!(
        staging.join(COPYING).is_file(),
        "it must land inside staging instead, not be dropped"
    );
    assert!(
        staging.join(SYSTEM_DIC).is_file(),
        "the legitimate member must still land"
    );
}

/// A dictionary that decompresses past its cap is a bad archive, not a
/// successful unpack of 64 MiB the caller never asked for. The cap is injected
/// rather than fixture-sized: building a >64 MiB fixture to prove a `>` costs
/// far more than it proves.
#[test]
fn a_dictionary_over_its_cap_is_an_archive_error() {
    let dir = scratch("unpack-dictcap");
    let archive = write(&dir, "asset.tar.xz", &real_shaped_archive());

    let err = extract::unpack_into_for_test(&archive, &dir.join("staging"), 12 << 20, 16)
        .expect_err("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
}

/// The xz layer decodes into memory, so an unbounded one is an allocation a
/// hostile archive chooses. `lzma-rs` 0.3 has no bounded entry point, so this
/// exercises the limiting writer that stands in for one.
#[test]
fn a_tar_over_its_cap_is_an_archive_error() {
    let dir = scratch("unpack-tarcap");
    let archive = write(&dir, "asset.tar.xz", &real_shaped_archive());

    let err = extract::unpack_into_for_test(&archive, &dir.join("staging"), 16, 64 << 20)
        .expect_err("must fail");
    assert!(matches!(err, SourceError::Archive(_)), "got {err:?}");
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
            (
                "ipadic-mecab-2_7_0/system.dic.zst",
                zstd(&dictionary_bytes()),
            ),
        ]),
    );
    let staging = dir.join("staging");

    extract::unpack_into(&archive, &staging).expect("unpack");

    assert!(
        !staging.join("README.md").exists(),
        "only the three known names"
    );
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

/// The shape the `tar xf` instructions this phase deleted leave behind: the
/// resolved directory name, non-empty, holding the *compressed* member and no
/// `system.dic`. Before this variant, every run failed identically with
/// `hints io failed: Directory not empty (os error 39)` — which names neither
/// path, offers no way out, and discarded the 46 MB that had just been
/// unpacked.
#[test]
fn publishing_over_a_blocking_directory_names_both_paths_and_a_way_out() {
    let dir = scratch("publish-blocked");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join("system.dic.zst"), b"still compressed").expect("write");
    std::fs::write(target.join(COPYING), b"licence").expect("write");

    let staging = dir.join("staging");
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the real one").expect("write");

    let err = publish(&staging, &target).expect_err("must fail");
    match &err {
        SourceError::Publish {
            staging: from,
            target: to,
            ..
        } => {
            assert_eq!(from, &staging);
            assert_eq!(to, &target);
        }
        other => panic!("got {other:?}"),
    }

    let msg = err.to_string();
    assert!(msg.contains(&staging.display().to_string()), "got {msg}");
    assert!(msg.contains(&target.display().to_string()), "got {msg}");
    assert!(
        msg.contains("Remove or rename"),
        "the message must say what to do, got {msg}"
    );
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
    assert!(
        !staging.exists(),
        "the losing staging directory must be removed"
    );
}

/// Two archives is not an error: the newest wins, which is the same rule
/// `resolve` applies against the published slot. One sentence to explain, and
/// no new error variant.
#[test]
fn the_newest_hand_placed_archive_wins() {
    let dir = scratch("handplaced-newest");
    write(&dir, "old.tar.xz", &real_shaped_archive());
    tick();
    write(&dir, "new.tar.xz", &real_shaped_archive());

    assert_eq!(
        vibrato_source::hand_placed(&dir),
        Some(dir.join("new.tar.xz"))
    );
}

/// A staging archive is a download in progress or a dead one's leftover, and
/// neither is a file the user placed. It is excluded twice over — by the
/// leading dot and by not ending in the suffix — and this pins both halves.
#[test]
fn dotfiles_and_staging_archives_are_not_hand_placed() {
    let dir = scratch("handplaced-staging");
    let (staging_archive, _) = vibrato_source::staging_names(&dir);
    write(&dir, ".hidden.tar.xz", b"not a candidate");
    std::fs::write(&staging_archive, b"mid download").expect("write");
    write(&dir, "notes.txt", b"not an archive");

    assert_eq!(vibrato_source::hand_placed(&dir), None);
}

/// First run: the hints directory does not exist yet. A `read_dir` failure
/// there is "no archive", not an error to report.
#[test]
fn an_absent_hints_directory_has_no_hand_placed_archive() {
    let dir = scratch("handplaced-absent");
    assert_eq!(vibrato_source::hand_placed(&dir.join("never-created")), None);
}

/// The whole point of the second entry point: `publish` treats an occupied
/// target as a benign race and throws the new dictionary away, which is right
/// for two processes fetching the same pinned artifact and wrong for a
/// deliberate replacement.
#[test]
fn publish_replacing_overwrites_an_existing_dictionary() {
    let dir = scratch("replace-happy");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the old one").expect("write");

    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the new one").expect("write");

    vibrato_source::publish_replacing(&staging, &target).expect("replace");

    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("read"),
        b"the new one"
    );
    assert!(!staging.exists(), "the staging directory must not survive");
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .expect("read_dir")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".partial"))
        .collect();
    assert!(leftovers.is_empty(), "backup left behind: {leftovers:?}");
}

/// The failure the backup exists for: the second rename does not happen, and
/// the user must not be left with no dictionary at all. A staging directory
/// that is not there is the portable way to make that rename fail.
#[test]
fn a_failed_replacement_puts_the_old_dictionary_back() {
    let dir = scratch("replace-restore");
    let target = dir.join(DICT_DIR);
    std::fs::create_dir_all(&target).expect("target");
    std::fs::write(target.join(SYSTEM_DIC), b"the old one").expect("write");

    let (_, staging) = vibrato_source::staging_names(&dir);

    let err = vibrato_source::publish_replacing(&staging, &target).expect_err("must fail");
    assert!(matches!(err, SourceError::Publish { .. }), "got {err:?}");
    assert_eq!(
        std::fs::read(target.join(SYSTEM_DIC)).expect("the old dictionary must survive"),
        b"the old one"
    );
}

/// No dictionary to replace is not a special case for the caller to handle.
#[test]
fn publish_replacing_onto_nothing_is_a_plain_publish() {
    let dir = scratch("replace-empty");
    let (_, staging) = vibrato_source::staging_names(&dir);
    std::fs::create_dir_all(&staging).expect("staging");
    std::fs::write(staging.join(SYSTEM_DIC), b"the only one").expect("write");

    vibrato_source::publish_replacing(&staging, &dir.join(DICT_DIR)).expect("publish");

    assert!(vibrato_source::published(&dir).is_some());
}

/// The fixture-archive helper's own round trip: what `pack_archive` writes
/// must be exactly what `unpack_into` reads back. `src-tauri`'s
/// hand-placed-archive test drives the real `resolve` against an archive this
/// helper builds, and if the two disagreed that test would be proving
/// something false.
#[test]
fn pack_archive_round_trips_through_unpack_into() {
    let dir = scratch("pack-roundtrip");
    let dictionary = dictionary_bytes();
    let archive = dir.join("packed.tar.xz");

    vibrato_source::pack_archive(&dictionary, &archive).expect("pack");

    let staging = dir.join("staging");
    extract::unpack_into(&archive, &staging).expect("unpack");

    assert_eq!(
        std::fs::read(staging.join(SYSTEM_DIC)).expect("read"),
        dictionary,
        "the unpacked dictionary must equal what was packed"
    );
}
