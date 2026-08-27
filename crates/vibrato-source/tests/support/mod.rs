// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Fixtures, built in memory.
//!
//! Nothing here is committed as a file. The dictionary is the same two-entry
//! IPADIC-shaped one `jparser`'s `hints.rs` builds in its own tests, pushed
//! back out through the exact stack the production code reads: vibrato →
//! zstd → tar → xz. A fixture that is a real archive of a real dictionary is
//! the only kind that can prove the unpacker end to end.
//!
//! This module is compiled once per integration-test binary that declares
//! `mod support;`, and no single binary calls every helper here — so an
//! unused one is not dead code, just a fixture another binary needs.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};

/// A compiled two-entry dictionary, serialized as vibrato writes it.
pub fn dictionary_bytes() -> Vec<u8> {
    const LEX: &str = "東京,0,0,5000,名詞,固有名詞,地域,一般,*,*,東京,トウキョウ,トーキョー\n\
                       都,0,0,5000,名詞,接尾,地域,*,*,*,都,ト,ト\n";
    const MATRIX: &str = "1 1\n0 0 0\n";
    const CHAR: &str = "DEFAULT 0 1 0\nKANJI 0 0 2\n0x4E00..0x9FFF KANJI\n";
    const UNK: &str = "DEFAULT,0,0,5000,記号,*,*,*,*,*,*\n\
                       KANJI,0,0,5000,名詞,一般,*,*,*,*,*\n";

    let dict = vibrato::SystemDictionaryBuilder::from_readers(
        LEX.as_bytes(),
        MATRIX.as_bytes(),
        CHAR.as_bytes(),
        UNK.as_bytes(),
    )
    .expect("the fixture dictionary must build");

    let mut out = Vec::new();
    dict.write(&mut out)
        .expect("the fixture dictionary must serialize");
    out
}

/// zstd-compress `plain` the way the release asset's `system.dic.zst` is.
pub fn zstd(plain: &[u8]) -> Vec<u8> {
    ruzstd::encoding::compress_to_vec(plain, ruzstd::encoding::CompressionLevel::Fastest)
}

/// A `.tar.xz` holding `members`, each `(path_in_archive, bytes)`.
///
/// Paths are written verbatim, including ones that try to escape — that is what
/// makes the traversal test meaningful. `Header::set_path`/`Builder::append_data`
/// refuse to *write* a `..` component even though the crate reads one back
/// without complaint, so a traversing name is poked straight into the header's
/// raw `name` bytes instead, bypassing that write-side guard the way a
/// hand-crafted malicious archive would.
pub fn tar_xz(members: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, bytes) in members {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        let name_bytes = name.as_bytes();
        let gnu = header.as_gnu_mut().expect("gnu header");
        gnu.name[..name_bytes.len()].copy_from_slice(name_bytes);
        header.set_cksum();
        builder.append(&header, bytes.as_slice()).expect("append");
    }
    let tar_bytes = builder.into_inner().expect("finish tar");

    let mut xz = Vec::new();
    lzma_rs::xz_compress(&mut tar_bytes.as_slice(), &mut xz).expect("xz compress");
    xz
}

/// The archive the real release publishes: three members under one directory.
pub fn real_shaped_archive() -> Vec<u8> {
    tar_xz(&[
        (
            "ipadic-mecab-2_7_0/COPYING",
            b"IPADIC licence text".to_vec(),
        ),
        ("ipadic-mecab-2_7_0/NOTICE", b"IPADIC notice text".to_vec()),
        (
            "ipadic-mecab-2_7_0/system.dic.zst",
            zstd(&dictionary_bytes()),
        ),
    ])
}

/// A clean scratch directory, removed and recreated per test.
pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vibrato-source-it-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Write `bytes` to `dir/name` and return the path.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    let mut file = std::fs::File::create(&path).expect("create");
    file.write_all(bytes).expect("write");
    path
}

/// Sleep long enough that the next file written compares strictly newer.
///
/// The archive-versus-slot rule is an mtime comparison, and two files written
/// in the same tick can compare equal. Crude, but the alternative is a
/// `filetime` dependency for a test-only concern.
pub fn tick() {
    std::thread::sleep(std::time::Duration::from_millis(50));
}
