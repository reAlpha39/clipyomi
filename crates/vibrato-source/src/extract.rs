// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Unwrapping the archive: xz, then tar, then zstd.
//!
//! `vibrato::Dictionary::read` takes a plain compiled dictionary, and vibrato's
//! README is explicit that its API will not decompress one ("you must
//! decompress them outside of the API"). Both layers are therefore unwrapped
//! here, and the dictionary is written out decompressed so nothing downstream —
//! `VibratoTokenizer::load`, `TA_HINTS_DICT`, `parse --hints` — has to learn a
//! second thing a dictionary file can be.

use std::io::Write;
use std::path::Path;

use crate::{SourceError, COPYING, NOTICE, SYSTEM_DIC, SYSTEM_DIC_ZST};

/// Unpack the `.tar.xz` at `archive` into `staging`.
///
/// Only three members are written, matched by **basename**, to paths this
/// function builds. Nothing derived from an entry's own path reaches the
/// filesystem: the archive is pinned and hashed today, but the extractor's
/// safety must not depend on a constant somewhere else in the crate, or a
/// later phase that repoints the pin or adds a hand-placed archive inherits a
/// hole rather than a guard.
///
/// An unrecognised member is skipped rather than refused — a future release
/// that adds a README must not fail the fetch. A missing [`SYSTEM_DIC_ZST`] is
/// [`SourceError::MissingMember`].
///
/// When the zstd frame carries a content checksum, it is verified against the
/// decoded bytes before this returns — `ruzstd`'s own decoder does not do this
/// itself; see the check inline below.
pub fn unpack_into(archive: &Path, staging: &Path) -> Result<(), SourceError> {
    // The xz layer is decoded to memory rather than streamed: `xz_decompress`
    // wants a `BufRead` and a `Write`, the decompressed tar is ~8 MB, and the
    // 46 MB dictionary inside it is still streamed to disk below.
    let mut tar_bytes = Vec::new();
    {
        let mut input = std::io::BufReader::new(std::fs::File::open(archive)?);
        lzma_rs::xz_decompress(&mut input, &mut tar_bytes)
            .map_err(|e| SourceError::Archive(format!("xz: {e}")))?;
    }

    std::fs::create_dir_all(staging)?;

    let mut found = false;
    let mut tar = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = tar
        .entries()
        .map_err(|e| SourceError::Archive(format!("tar: {e}")))?;

    for entry in entries {
        let mut entry = entry.map_err(|e| SourceError::Archive(format!("tar: {e}")))?;
        // The borrow of `entry` for its path ends with this block, so the entry
        // can be read from below.
        let name = {
            let path = entry
                .path()
                .map_err(|e| SourceError::Archive(format!("tar: {e}")))?;
            match path.file_name().and_then(|n| n.to_str()) {
                Some(name) => name.to_owned(),
                None => continue,
            }
        };

        match name.as_str() {
            SYSTEM_DIC_ZST => {
                let mut out =
                    std::io::BufWriter::new(std::fs::File::create(staging.join(SYSTEM_DIC))?);
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut entry)
                    .map_err(|e| SourceError::Archive(format!("zstd: {e}")))?;
                std::io::copy(&mut decoder, &mut out)
                    .map_err(|e| SourceError::Archive(format!("zstd: {e}")))?;
                out.flush()?;
                // `StreamingDecoder`'s `Read` impl decodes block-by-block and
                // does not itself compare the frame's trailing content
                // checksum against what it decoded — a corrupt byte that
                // does not break block *structure* (e.g. one flipped in an
                // uncompressed block, which is what the fixture dictionary,
                // tiny enough to skip real compression, produces) would
                // otherwise decode "successfully" into wrong bytes. Both
                // values are only populated once every byte has been read,
                // which the `copy` above just did.
                if let (Some(expected), Some(actual)) = (
                    decoder.decoder.get_checksum_from_data(),
                    decoder.decoder.get_calculated_checksum(),
                ) {
                    if expected != actual {
                        return Err(SourceError::Archive(format!(
                            "zstd: content checksum mismatch: expected {expected:#010x}, got {actual:#010x}"
                        )));
                    }
                }
                found = true;
            }
            COPYING | NOTICE => {
                let mut out = std::io::BufWriter::new(std::fs::File::create(staging.join(&name))?);
                std::io::copy(&mut entry, &mut out)?;
                out.flush()?;
            }
            _ => continue,
        }
    }

    if !found {
        return Err(SourceError::MissingMember {
            name: SYSTEM_DIC_ZST.to_string(),
        });
    }
    Ok(())
}
