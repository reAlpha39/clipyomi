// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Obtaining the JMdict source archive.
//!
//! Phase 2A's `ensure_dictionary` takes a lazy `FnOnce() -> io::Result<impl
//! BufRead>` so that the ~10 MB archive is never obtained on the steady-state
//! path. This crate is that closure's body: it opens a hand-placed file if one
//! exists, and otherwise downloads, verifies, and publishes one.
//!
//! Deliberately independent of `jparser`. Taking a source directory rather than
//! a generation root keeps it ignorant of 2A's layout — a `resolve(root)` that
//! looked inside the generation root would depend on that layout without
//! declaring it.

use std::io::BufRead;
use std::path::{Path, PathBuf};

pub mod fetch;

/// Directory name callers conventionally use for the source archive. Not
/// enforced — every function here takes the directory it is given.
pub const SOURCE_DIR: &str = "source";

/// The archive's name, as EDRDG publishes it. The extension is a location, not
/// a claim: [`open_local`] decides compression from the bytes.
pub const SOURCE_FILE: &str = "JMdict_e.gz";

/// Suffix marking a download that is in progress or not yet verified.
/// `fetch_from` appends the writing process's PID after it, so the staging
/// name is actually `<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>` rather than this
/// suffix trailing the name directly — that is what lets two concurrent
/// invocations stage into different files instead of one truncating the
/// other's write. Either shape is never resolved, so a killed or still-
/// running download cannot be mistaken for a hand-placed file.
///
/// **These accumulate.** A download that fails or is killed leaves its staging
/// file behind under a name no later run reuses, so an interrupted transfer
/// costs up to `MAX_ARCHIVE_BYTES` of disk permanently. Nothing sweeps them,
/// deliberately: a process cannot tell another's live staging file from a dead
/// one's leftovers, so deleting by pattern would reintroduce exactly the race
/// the PID exists to prevent. They are inert — never resolved, never verified
/// — so the cost is disk, not correctness. Users who care can delete
/// `JMdict_e.gz.partial.*` while no download is running.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Download attempts before [`fetch::fetch`] gives up.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Delay before the second attempt; doubles for each attempt after it.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);

/// The two-byte gzip magic, `1f 8b`.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Prefixes an ungzipped source file may start with: the real archive's XML
/// prolog, its doctype, and its root element for a hand-trimmed copy.
///
/// Matched against JMdict specifically rather than against `<`: the parser reads
/// markup it does not recognise as zero entries, so a saved error page would
/// otherwise build an empty index and report success.
const XML_PREFIXES: [&[u8]; 3] = [b"<?xml", b"<!DOCTYPE JMdict", b"<JMdict"];

/// A leading UTF-8 BOM, skipped before the prefix check: editors on Windows add
/// one, and the XML behind it is still the archive the user meant to place.
const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// Whether `head` — a file's first bytes — begins JMdict XML.
fn begins_jmdict_xml(head: &[u8]) -> bool {
    let head = head.strip_prefix(&UTF8_BOM).unwrap_or(head);
    let head = head.trim_ascii_start();
    XML_PREFIXES.iter().any(|prefix| head.starts_with(prefix))
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloading the dictionary failed: {0}")]
    Transport(String),
    #[error(
        "the dictionary server returned HTTP {status} for {url}; place a {file} \
         in {source_dir} manually to bypass the download",
        file = SOURCE_FILE,
    )]
    Http {
        status: u16,
        /// The URL that was requested. A 4xx is most often EDRDG renaming or
        /// moving the archive — this project has already been bitten by a
        /// wrong filename once — and a report with no URL leaves nothing to
        /// check by hand.
        url: String,
        /// Named for the same reason [`SourceError::TooManyAttempts`] carries
        /// it: the escape-hatch clause needs to say where to place the file.
        source_dir: PathBuf,
    },
    /// The downloaded bytes did not decode as a valid gzip archive. Distinct
    /// from [`SourceError::Io`]: opening or writing a local file failed for
    /// [`SourceError::Io`], but here the file opened and wrote fine — its
    /// *content* was bad. The retry policy treats the two oppositely: a
    /// corrupt body is worth retrying (the URL was not at fault), a local
    /// I/O failure is not (retrying will not free disk or fix permissions).
    #[error("the downloaded archive is corrupt: {0}")]
    Corrupt(String),
    /// The file at the resolved name is neither gzip nor JMdict XML. Only a
    /// hand-placed file reaches this: a download is gzip-verified before it is
    /// renamed into place. Separate from [`SourceError::Corrupt`], which says
    /// "the server sent us something broken, retrying may help" — here nothing
    /// will change until the file itself does, so the message says so.
    #[error(
        "{path} is neither a gzip archive nor JMdict XML — it starts with \"{head}\". \
         Delete it, or replace it with a real {file}",
        path = path.display(),
        file = SOURCE_FILE,
    )]
    NotAnArchive {
        path: PathBuf,
        /// The first bytes, escaped and truncated. A user who saved the wrong
        /// file usually recognises it from its opening — an unquoted length or
        /// byte count would not tell them which file they grabbed.
        head: String,
    },
    #[error(
        "could not obtain the dictionary after {attempts} attempts ({last}); \
         place a {file} in {source_dir} manually to bypass the download",
        file = SOURCE_FILE,
    )]
    TooManyAttempts {
        attempts: usize,
        source_dir: PathBuf,
        /// The final attempt's failure, rendered. Attempts can be exhausted by
        /// a transport error, a 5xx, or a corrupt archive, and those are three
        /// different things for a user to act on.
        last: String,
    },
}

/// Open `path`, decompressing it when its bytes say it is gzipped.
///
/// The file name is not consulted. EDRDG publishes `JMdict_e.gz`, but a user who
/// decompressed it by hand and kept the name is a case worth supporting, and
/// exactly one resolved name — rather than one per encoding — avoids needing a
/// precedence rule between two copies that disagree.
///
/// Anything that is neither is rejected rather than passed through as text: the
/// XML parser downstream reads what it cannot recognise as zero entries, so a
/// wrong file used to publish an empty index and report success. This is the
/// only boundary where those bytes are still identifiable as a wrong *file*
/// rather than an empty dictionary.
pub fn open_local(path: &Path) -> Result<Box<dyn BufRead>, SourceError> {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    // `fill_buf` peeks without consuming, so the sniffed bytes remain available
    // to whichever reader is constructed below. The borrow ends with this block,
    // before `reader` moves into a decoder.
    let (gzipped, xml, head) = {
        let head = reader.fill_buf()?;
        (
            head.starts_with(&GZIP_MAGIC),
            begins_jmdict_xml(head),
            // Enough to recognise a saved web page or a text file by, short
            // enough to sit inside a one-line message.
            String::from_utf8_lossy(&head[..head.len().min(24)])
                .escape_debug()
                .to_string(),
        )
    };

    if gzipped {
        return Ok(Box::new(std::io::BufReader::new(
            flate2::read::GzDecoder::new(reader),
        )));
    }
    if xml {
        return Ok(Box::new(reader));
    }
    Err(SourceError::NotAnArchive {
        path: path.to_path_buf(),
        head,
    })
}

/// Open the JMdict source in `source_dir`, downloading it first if absent.
///
/// Built to be the body of Phase 2A's lazy `source` closure:
///
/// ```ignore
/// let index = jparser::index::ensure_dictionary(
///     &generation_root, &table, &opts, keep,
///     || jmdict_source::resolve(&source_dir),
/// )?;
/// ```
///
/// 2A guarantees the closure runs only when a rebuild is actually needed, so a
/// steady-state start never touches the network or the ~10 MB archive.
///
/// A `<SOURCE_FILE><PARTIAL_SUFFIX>.<pid>` file is never resolved: it is either
/// a download in progress or one that failed verification, and treating it as a
/// hand-placed archive is how a truncation reaches the parser.
pub fn resolve(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>> {
    resolve_from(fetch::JMDICT_URL, source_dir, RETRY_BACKOFF)
}

/// [`resolve`] with the URL and backoff injected, so tests can point at a
/// local listener without touching the real network or sleeping at the
/// production backoff — the same reason [`fetch::fetch_with_retry`] exists
/// for [`fetch::fetch`].
///
/// **Not part of the supported surface.** Production callers use [`resolve`].
pub fn resolve_from(
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> std::io::Result<Box<dyn BufRead>> {
    let target = source_dir.join(SOURCE_FILE);
    let path = if target.exists() {
        target
    } else {
        fetch::fetch_with_retry(url, source_dir, backoff).map_err(std::io::Error::other)?
    };
    open_local(&path).map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("jmdict-source-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// gzip `data`, so tests never ship a binary fixture.
    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).expect("gz write");
        e.finish().expect("gz finish")
    }

    const XML: &[u8] = b"<?xml version=\"1.0\"?><JMdict></JMdict>";

    fn read_all(mut r: Box<dyn BufRead>) -> Vec<u8> {
        let mut out = Vec::new();
        r.read_to_end(&mut out).expect("read");
        out
    }

    #[test]
    fn a_gzipped_file_is_decompressed() {
        let dir = scratch("open-gz");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(XML)).expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), XML);
    }

    /// The extension says `.gz`, but the bytes decide. A user who decompressed
    /// the archive by hand and kept the name must still work.
    #[test]
    fn a_plain_xml_file_is_passed_through() {
        let dir = scratch("open-plain");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, XML).expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), XML);
    }

    /// A hand-trimmed file that starts at the root element, with no prolog.
    #[test]
    fn a_plain_file_starting_at_the_root_element_is_passed_through() {
        let dir = scratch("open-root");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<JMdict></JMdict>").expect("write");
        assert_eq!(
            read_all(open_local(&path).expect("open")),
            b"<JMdict></JMdict>"
        );
    }

    /// Editors on Windows prepend a BOM. That is not the file's fault, and the
    /// XML behind it is still the archive the user meant to place.
    #[test]
    fn a_bom_and_leading_whitespace_do_not_reject_real_xml() {
        let dir = scratch("open-bom");
        let path = dir.join(SOURCE_FILE);
        let mut bytes = vec![0xEF, 0xBB, 0xBF, b'\n', b' '];
        bytes.extend_from_slice(XML);
        std::fs::write(&path, &bytes).expect("write");
        assert_eq!(read_all(open_local(&path).expect("open")), bytes);
    }

    /// The failure this rejection exists for: the XML parser reads anything it
    /// does not recognise as zero entries, so a file that is neither gzip nor
    /// JMdict used to build an empty index and report success. Phase 2F put
    /// "place a JMdict_e.gz here" in front of every user who hits a download
    /// failure, so a wrong file is now something users will actually do.
    #[test]
    fn a_file_that_is_neither_gzip_nor_jmdict_xml_is_rejected() {
        let dir = scratch("open-junk");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"this is not a dictionary").expect("write");

        let err = open_local(&path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        // The message has to name the file and what to do, because it is shown
        // verbatim on the download screen's failure state.
        let msg = err.to_string();
        assert!(msg.contains(SOURCE_FILE), "got {msg}");
    }

    /// The realistic wrong file: a captive portal or 404 page saved under the
    /// archive's name. It IS well-formed markup, so a bare "starts with `<`"
    /// check would wave it through.
    #[test]
    fn an_html_page_saved_under_the_archive_name_is_rejected() {
        let dir = scratch("open-html");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<!DOCTYPE html><html><body>404</body></html>").expect("write");

        let err = open_local(&path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// Shorter than the two magic bytes. Must not panic on the slice — and is
    /// not a dictionary either.
    #[test]
    fn a_one_byte_file_is_rejected() {
        let dir = scratch("open-tiny");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<").expect("write");

        let err = open_local(&path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// A zero-length file is the shape a killed `cp` or a full disk leaves.
    #[test]
    fn an_empty_file_is_rejected() {
        let dir = scratch("open-empty");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"").expect("write");

        let err = open_local(&path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn an_absent_file_is_an_io_error() {
        let dir = scratch("open-absent");
        // `expect_err` would require `Box<dyn BufRead>: Debug` to render the
        // Ok side of a hypothetical panic message; it does not implement
        // Debug, so go through `Option` instead, which needs no such bound.
        let err = open_local(&dir.join(SOURCE_FILE)).err().expect("must fail");
        assert!(matches!(err, SourceError::Io(_)), "got {err:?}");
    }

    #[test]
    fn the_source_file_name_is_the_edrdg_name() {
        // EDRDG publishes `JMdict_e.gz`. An earlier draft of the design said
        // `JMdict_e.xml.gz`, which does not exist.
        assert_eq!(SOURCE_FILE, "JMdict_e.gz");
    }

    #[test]
    fn a_present_archive_is_opened_without_downloading() {
        let dir = scratch("resolve-local");
        std::fs::write(dir.join(SOURCE_FILE), gz(XML)).expect("write");
        assert_eq!(read_all(resolve(&dir).expect("resolve")), XML);
    }

    #[test]
    fn a_plain_archive_at_the_resolved_name_is_opened_too() {
        let dir = scratch("resolve-plain");
        std::fs::write(dir.join(SOURCE_FILE), XML).expect("write");
        assert_eq!(read_all(resolve(&dir).expect("resolve")), XML);
    }

    // `a_partial_alone_does_not_satisfy_resolve` and
    // `the_typed_error_survives_the_io_wrapper` live in `tests/download.rs`:
    // proving `resolve`'s fall-through to a failing download needs a real (if
    // local) listener, which belongs with the rest of the socket-driven tests,
    // not here. Moved by Ruling G after the brief's originals were found to
    // fall through to the real production URL.
}
