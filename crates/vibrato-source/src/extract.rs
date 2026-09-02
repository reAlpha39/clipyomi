// SPDX-License-Identifier: MIT OR Apache-2.0

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

/// Cap on the tar the xz layer decodes into memory.
///
/// Derived from the measured asset: `xz -dc ipadic-mecab-2_7_0.tar.xz | wc -c`
/// is 8,058,880 bytes, so this is ~1.5x the real value — headroom for a future
/// dictionary, while an xz stream that claims to expand to gigabytes is refused
/// after 12 MiB rather than after the OOM killer.
///
/// `lzma-rs` 0.3 offers no bounded entry point: `xz_decompress` writes into
/// whatever `Write` it is handed until the stream ends. The bound therefore has
/// to be the writer itself — see [`LimitedWriter`].
const MAX_TAR_BYTES: u64 = 12 * 1024 * 1024;

/// Cap on the decompressed dictionary.
///
/// Derived from the measured asset: the real `system.dic` is 47,788,814 bytes,
/// so this is ~1.4x it. Same reasoning as [`MAX_TAR_BYTES`], with one
/// difference that matters: this layer streams to disk, so an unbounded frame
/// fills the user's disk rather than their memory.
const MAX_DICT_BYTES: u64 = 64 * 1024 * 1024;

/// A `Write` that refuses to pass on more than `limit` bytes, and remembers
/// whether the last failure was its own cap or the writer underneath it.
///
/// Both jobs exist for one reason: `std::io::copy` collapses three unrelated
/// failures — the decoder gave up, the output grew past its cap, the disk is
/// full — into a single `io::Error`, and a caller must act on them differently.
/// The first two are a bad archive, which [`crate::fetch::fetch_with_retry`]
/// retries; the third is local I/O, where retrying re-downloads 7.7 MB and
/// re-decompresses it twice more onto a still-full disk.
struct LimitedWriter<W> {
    inner: W,
    /// Bytes still allowed through. Counted down rather than up so the check
    /// below needs no addition that could overflow.
    remaining: u64,
    limit: u64,
    /// Set when [`Self::inner`] itself failed, which is the one case that is
    /// *not* the archive's fault.
    io_failed: bool,
}

impl<W: Write> LimitedWriter<W> {
    fn new(inner: W, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
            limit,
            io_failed: false,
        }
    }

    /// Which [`SourceError`] a failure out of this writer's chain actually is.
    /// `layer` names the format for the message, as the rest of this module
    /// does.
    fn classify(&self, e: std::io::Error, layer: &str) -> SourceError {
        if self.io_failed {
            SourceError::Io(e)
        } else {
            SourceError::Archive(format!("{layer}: {e}"))
        }
    }
}

impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Refusing outright rather than writing what fits: `io::copy` hands
        // over only the bytes the decoder actually produced, so a buffer larger
        // than what is left is proof the output has already passed the cap. A
        // stream of exactly `limit` bytes never trips this — its final buffer
        // is equal to `remaining`, not greater.
        if buf.len() as u64 > self.remaining {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("decompressed past the {} byte limit", self.limit),
            ));
        }
        match self.inner.write(buf) {
            Ok(n) => {
                self.remaining -= n as u64;
                Ok(n)
            }
            Err(e) => {
                self.io_failed = true;
                Err(e)
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.inner.flush() {
            Ok(()) => Ok(()),
            Err(e) => {
                self.io_failed = true;
                Err(e)
            }
        }
    }
}

/// Copy `src` into `dst` under `dst`'s cap, reporting each failure as what it
/// is. See [`LimitedWriter`] for why the three cannot share one variant.
fn copy_bounded<R: std::io::Read, W: Write>(
    src: &mut R,
    dst: &mut LimitedWriter<W>,
    layer: &str,
) -> Result<(), SourceError> {
    if let Err(e) = std::io::copy(src, dst) {
        return Err(dst.classify(e, layer));
    }
    // A `BufWriter` under the cap holds bytes back until here, so a full disk
    // is as likely to surface at the flush as during the copy. It can only come
    // from the writer underneath, so it is always local I/O.
    dst.flush().map_err(SourceError::Io)
}

/// Unpack the `.tar.xz` at `archive` into `staging`.
///
/// Only three members are written, matched by **basename**, to paths this
/// function builds. Nothing derived from an entry's own path reaches the
/// filesystem: the archive is pinned and hashed today, but the extractor's
/// safety must not depend on a constant somewhere else in the crate, or a
/// later phase that repoints the pin or adds a hand-placed archive inherits a
/// hole rather than a guard.
///
/// For the same reason, both decompressed layers are bounded — see
/// [`MAX_TAR_BYTES`] and [`MAX_DICT_BYTES`]. An overrun is
/// [`SourceError::Archive`]; a failure to *write* the output is
/// [`SourceError::Io`], because a full disk is not a corrupt archive and must
/// not be retried as one.
///
/// An unrecognised member is skipped rather than refused — a future release
/// that adds a README must not fail the fetch. A missing [`SYSTEM_DIC_ZST`] is
/// [`SourceError::MissingMember`].
///
/// When the zstd frame carries a content checksum, it is verified against the
/// decoded bytes before this returns — `ruzstd`'s own decoder does not do this
/// itself; see the check inline below. Today's pinned asset does **not** carry
/// one (measured; see the comment at that check), so on the real download the
/// integrity guarantee is [`crate::ASSET_SHA256`] over the whole `.tar.xz`.
pub fn unpack_into(archive: &Path, staging: &Path) -> Result<(), SourceError> {
    unpack_bounded(archive, staging, MAX_TAR_BYTES, MAX_DICT_BYTES)
}

/// [`unpack_into`] with the caps injected.
///
/// **Not part of the supported surface.** It exists so the overrun tests can
/// drive the real limiting mechanism against tiny values instead of building a
/// 64 MiB fixture — the same reason [`crate::fetch::download_and_verify_for_test`]
/// exists.
#[doc(hidden)]
pub fn unpack_into_for_test(
    archive: &Path,
    staging: &Path,
    max_tar: u64,
    max_dict: u64,
) -> Result<(), SourceError> {
    unpack_bounded(archive, staging, max_tar, max_dict)
}

/// [`unpack_bounded`]'s real work, wrapped by it so `SourceError::Archive`
/// gets the archive's path named exactly once, at this function's one caller,
/// rather than at each of the several places below that can produce one.
fn unpack_bounded(
    archive: &Path,
    staging: &Path,
    max_tar: u64,
    max_dict: u64,
) -> Result<(), SourceError> {
    unpack_bounded_inner(archive, staging, max_tar, max_dict).map_err(|e| match e {
        // With two `.tar.xz` files possible in the drop box, "the archive
        // could not be unpacked: xz: ..." does not say which one to remove.
        // Named once, here, rather than threaded through every construction
        // site inside `unpack_bounded_inner`.
        SourceError::Archive(msg) => {
            SourceError::Archive(format!("{msg} (in {})", archive.display()))
        }
        other => other,
    })
}

fn unpack_bounded_inner(
    archive: &Path,
    staging: &Path,
    max_tar: u64,
    max_dict: u64,
) -> Result<(), SourceError> {
    // The xz layer is decoded to memory rather than streamed: `xz_decompress`
    // wants a `BufRead` and a `Write`, the decompressed tar is ~8 MB, and the
    // 46 MB dictionary inside it is still streamed to disk below.
    let mut tar_bytes = Vec::new();
    {
        let mut input = std::io::BufReader::new(std::fs::File::open(archive)?);
        // The sink is memory, so nothing here can be a local write failure:
        // every error out of this call is the xz layer's, cap overrun included.
        let mut out = LimitedWriter::new(&mut tar_bytes, max_tar);
        lzma_rs::xz_decompress(&mut input, &mut out)
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
                let file =
                    std::io::BufWriter::new(std::fs::File::create(staging.join(SYSTEM_DIC))?);
                let mut out = LimitedWriter::new(file, max_dict);
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(&mut entry)
                    .map_err(|e| SourceError::Archive(format!("zstd: {e}")))?;
                copy_bounded(&mut decoder, &mut out, "zstd")?;
                // `StreamingDecoder`'s `Read` impl decodes block-by-block and
                // does not itself compare the frame's trailing content
                // checksum against what it decoded — a corrupt byte that
                // does not break block *structure* (e.g. one flipped in an
                // uncompressed block, which is what the fixture dictionary,
                // tiny enough to skip real compression, produces) would
                // otherwise decode "successfully" into wrong bytes. Both
                // values are only populated once every byte has been read,
                // which the copy above just did.
                //
                // Conditional because the flag is optional, and measured to be
                // absent from today's pin: the real `system.dic.zst` opens
                // `28 b5 2f fd 00`, and that fifth byte is the frame header
                // descriptor — bit 2, the Content_Checksum_flag, is clear. So
                // this guards the fixtures and any future asset built with
                // `--check`, not the current download, whose integrity rests on
                // [`crate::ASSET_SHA256`] instead. Do not restate it as
                // unconditional protection.
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
                let file = std::io::BufWriter::new(std::fs::File::create(staging.join(&name))?);
                // Already bounded by the tar these come out of, which the xz
                // cap above bounds in turn; the cap here is what gives
                // `copy_bounded` a classifier. A read failure is the tar's
                // fault — a truncated `COPYING` is a malformed archive, not a
                // local I/O problem — so it is `Archive`, as the dictionary
                // branch above already is.
                let mut out = LimitedWriter::new(file, max_tar);
                copy_bounded(&mut entry, &mut out, "tar")?;
            }
            _ => continue,
        }
    }

    if !found {
        return Err(SourceError::MissingMember {
            name: SYSTEM_DIC_ZST.to_string(),
            archive: archive.to_path_buf(),
        });
    }
    Ok(())
}
