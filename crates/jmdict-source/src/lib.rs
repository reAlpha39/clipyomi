// SPDX-License-Identifier: MIT OR Apache-2.0

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

use std::io::{BufRead, Cursor, Read};
use std::path::{Path, PathBuf};

pub mod fetch;

/// Directory name callers conventionally use for the source archive. Not
/// enforced — every function here takes the directory it is given.
pub const SOURCE_DIR: &str = "source";

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
/// costs up to the source's `max_bytes` of disk permanently. Nothing sweeps them,
/// deliberately: a process cannot tell another's live staging file from a dead
/// one's leftovers, so deleting by pattern would reintroduce exactly the race
/// the PID exists to prevent. They are inert — never resolved, never verified
/// — so the cost is disk, not correctness. Users who care can delete
/// `JMdict_e.gz.partial.*` and `kanjidic2.xml.gz.partial.*` while no
/// download is running.
pub const PARTIAL_SUFFIX: &str = ".partial";

/// Download attempts before [`fetch::fetch`] gives up.
pub const DOWNLOAD_ATTEMPTS: usize = 3;

/// Delay before the second attempt; doubles for each attempt after it.
pub const RETRY_BACKOFF: std::time::Duration = std::time::Duration::from_secs(2);

/// The two-byte gzip magic, `1f 8b`.
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// A leading UTF-8 BOM, skipped before the prefix check: editors on Windows add
/// one, and the XML behind it is still the archive the user meant to place.
const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// How many bytes [`open_local`] reads ahead to decide what a file is. The
/// real archives reach `<!DOCTYPE` at byte 1,135 (JMdict) and 39
/// (KANJIDIC2), so the decision is made well inside this; [`begins_xml`]
/// says what happens when a file's preamble outruns it.
const PEEK_BYTES: usize = 8 * 1024;

/// One EDRDG source file: where it is published, what it is called, what its
/// root element is called, and how large a download may be.
///
/// Two exist, [`JMDICT`] and [`KANJIDIC`]. Everything in this crate that
/// downloads, verifies, stages or opens a file takes one of these, so the
/// second file shares the first's retry, staging, size cap and sniff
/// discipline rather than copying it (Phase 3a design §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    /// Plain HTTP by necessity: EDRDG serves no usable HTTPS (certificate
    /// subject-name mismatch) and this crate has no TLS backend. That is
    /// precisely why [`fetch::fetch_from`]'s verification is mandatory.
    pub url: &'static str,
    /// The archive's name, as EDRDG publishes it. The extension is a
    /// location, not a claim: [`open_local`] decides compression from bytes.
    pub file_name: &'static str,
    /// The XML root element's name — `JMdict` or `kanjidic2` — which is also
    /// the name a `<!DOCTYPE` carries. [`begins_xml`] looks for either behind
    /// an optional declaration and any comments, so the other source's file
    /// under this name, or a saved error page, is refused rather than parsed
    /// as an empty dictionary (Phase 3c design §4).
    pub root: &'static str,
    /// Upper bound on the response body a download will accept, enforced via
    /// `ureq`'s `BodyWithConfig::limit`. Nothing else bounds how much a
    /// misbehaving response could stream to disk before verification runs.
    pub max_bytes: u64,
}

/// JMdict, English glosses. 32 MiB is a little over 3x the measured
/// 10,545,887-byte archive.
pub const JMDICT: Source = Source {
    url: "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz",
    file_name: "JMdict_e.gz",
    root: "JMdict",
    max_bytes: 32 * 1024 * 1024,
};

/// KANJIDIC2. 8 MiB is a little over 5x the 1,488,571 bytes measured on
/// 2026-09-11; the same host and the same regenerate-daily policy as JMdict,
/// so it cannot be hash-pinned either.
pub const KANJIDIC: Source = Source {
    url: "http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz",
    file_name: "kanjidic2.xml.gz",
    root: "kanjidic2",
    max_bytes: 8 * 1024 * 1024,
};

/// JMdict's file name. Kept as a constant because every message, test and
/// caller that predates the second source names it this way.
pub const SOURCE_FILE: &str = JMDICT.file_name;

/// Whether `head` — a file's first bytes — begins `source`'s XML.
///
/// Root-aware (Phase 3c design §4): after an optional BOM, whitespace, one
/// `<?xml … ?>` declaration and any number of comments, the next markup
/// must be `<!DOCTYPE {root}` or `<{root}`, followed by whitespace, `>`, `[`
/// or `/`. Before this the check accepted `<?xml` on its own, so a JMdict
/// file saved under the KANJIDIC2 name with its declaration intact built a
/// kanji-less index and reported success.
///
/// `more_follows` says whether `head` was cut short by [`PEEK_BYTES`] rather
/// than by the end of the file. When it was, a preamble that outruns the
/// peek — or a token the peek cuts in half — is an accept: the check exists
/// to catch the wrong file, not to refuse a right one for the length of its
/// leading comment, and accepting there is what the old rule did. A file the
/// peek read to its end is decided on what it holds.
fn begins_xml(source: &Source, head: &[u8], more_follows: bool) -> bool {
    let mut rest = head
        .strip_prefix(&UTF8_BOM)
        .unwrap_or(head)
        .trim_ascii_start();
    if let Some(after) = rest.strip_prefix(b"<?xml") {
        match find(after, b"?>") {
            Some(end) => rest = after[end + 2..].trim_ascii_start(),
            None => return more_follows,
        }
    }
    while let Some(after) = rest.strip_prefix(b"<!--") {
        match find(after, b"-->") {
            Some(end) => rest = after[end + 3..].trim_ascii_start(),
            None => return more_follows,
        }
    }
    let doctype = format!("<!DOCTYPE {}", source.root);
    let element = format!("<{}", source.root);
    // Bound to `accepted` rather than returned directly: as a tail expression,
    // `[doctype.as_bytes(), element.as_bytes()].into_iter()...`'s temporary
    // `std::array::IntoIter` would outlive `doctype` and `element` under NLL's
    // drop-order rules, which the borrow checker rejects even though nothing
    // borrowed survives past `.any(..)`.
    let accepted = [doctype.as_bytes(), element.as_bytes()]
        .into_iter()
        .any(|token| match rest.strip_prefix(token) {
            Some(after) => match after.first() {
                Some(b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'[' | b'/') => true,
                Some(_) => false,
                None => more_follows,
            },
            // A token the peek cut in half is undecidable; a short file that
            // simply stops, or holds something else, is not.
            None => more_follows && token.starts_with(rest),
        });
    accepted
}

/// Byte offset of the first `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("source io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("downloading the dictionary failed: {0}")]
    Transport(String),
    #[error(
        "the dictionary server returned HTTP {status} for {url}; place a {file} \
         in {source_dir} manually to bypass the download"
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
        /// Which of the two files this was, so the escape hatch names it.
        file: &'static str,
    },
    /// The downloaded bytes did not decode as a valid gzip archive. Distinct
    /// from [`SourceError::Io`]: opening or writing a local file failed for
    /// [`SourceError::Io`], but here the file opened and wrote fine — its
    /// *content* was bad. The retry policy treats the two oppositely: a
    /// corrupt body is worth retrying (the URL was not at fault), a local
    /// I/O failure is not (retrying will not free disk or fix permissions).
    #[error("the downloaded archive is corrupt: {0}")]
    Corrupt(String),
    /// The file at the resolved name does not hold that source's XML — JMdict
    /// for [`JMDICT`], KANJIDIC2 for [`KANJIDIC`] — either plain or inside a
    /// gzip archive, per the `file` this variant carries. Usually a
    /// hand-placed file; a download reaches it only if the server sent a
    /// valid gzip of the wrong content, since [`fetch`] verifies the gzip
    /// framing before renaming a download into place. Separate from
    /// [`SourceError::Corrupt`], which says "the server sent us something
    /// broken, retrying may help" — here nothing will change until the file
    /// itself does, so the message says so.
    #[error(
        "{path} does not hold {file}'s XML, gzipped or plain — it starts with \"{head}\". \
         Delete it, or replace it with a real {file}",
        path = path.display(),
    )]
    NotAnArchive {
        path: PathBuf,
        /// The first bytes — decompressed, when the file was gzip — escaped
        /// and truncated. A user who saved the wrong file usually recognises
        /// it from its opening; an unquoted length or byte count would not
        /// tell them which file they grabbed.
        head: String,
        file: &'static str,
    },
    #[error(
        "could not obtain the dictionary after {attempts} attempts ({last}); \
         place a {file} in {source_dir} manually to bypass the download"
    )]
    TooManyAttempts {
        attempts: usize,
        source_dir: PathBuf,
        /// The final attempt's failure, rendered. Attempts can be exhausted by
        /// a transport error, a 5xx, or a corrupt archive, and those are three
        /// different things for a user to act on.
        last: String,
        file: &'static str,
    },
}

/// Open `path`, decompressing it when its bytes say it is gzipped.
///
/// The file name is not consulted. EDRDG publishes `JMdict_e.gz`, but a user who
/// decompressed it by hand and kept the name is a case worth supporting, and
/// exactly one resolved name — rather than one per encoding — avoids needing a
/// precedence rule between two copies that disagree.
///
/// What comes out of the file — or out of the gzip decoder — is then checked
/// against the source's root name by [`begins_xml`], and anything else is
/// rejected rather than passed through as text: the XML parser downstream
/// reads what it cannot recognise as zero entries, so a wrong file used to
/// publish an empty index and report success. This is the only boundary where
/// those bytes are still identifiable as a wrong *file* rather than an empty
/// dictionary. Both branches peek the same fixed number of bytes and hand the
/// reader back with those bytes still in front of it.
pub fn open_local(source: &Source, path: &Path) -> Result<Box<dyn BufRead>, SourceError> {
    let (head, file) = peek(std::fs::File::open(path)?)?;
    if head.starts_with(&GZIP_MAGIC) {
        // The decoder reads the peeked bytes first, then the rest of the
        // file; its output is sniffed exactly as a plain file's is, so a
        // gzipped wrong file is refused here rather than parsed as an empty
        // dictionary. A stream that will not decode fails this read as `Io`.
        let decoder = flate2::read::GzDecoder::new(Cursor::new(head).chain(file));
        let (head, decoder) = peek(decoder)?;
        return sniffed(source, path, head, decoder);
    }
    sniffed(source, path, head, file)
}

/// Read up to [`PEEK_BYTES`] from `reader` and hand both back. Reads until
/// the buffer is full or the reader is done, rather than trusting one
/// `read` call: a gzip decoder returns what one block yields, which can be
/// less than asked for with more still to come, and the decision in
/// [`begins_xml`] depends on knowing which of the two stopped the read.
fn peek<R: Read>(mut reader: R) -> std::io::Result<(Vec<u8>, R)> {
    let mut head = vec![0u8; PEEK_BYTES];
    let mut filled = 0;
    while filled < PEEK_BYTES {
        match reader.read(&mut head[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    head.truncate(filled);
    Ok((head, reader))
}

/// Accept `head` as `source`'s XML and return a reader over it followed by
/// `rest`, or refuse with the head quoted.
fn sniffed<R: Read + 'static>(
    source: &Source,
    path: &Path,
    head: Vec<u8>,
    rest: R,
) -> Result<Box<dyn BufRead>, SourceError> {
    if begins_xml(source, &head, head.len() == PEEK_BYTES) {
        return Ok(Box::new(std::io::BufReader::new(
            Cursor::new(head).chain(rest),
        )));
    }
    Err(SourceError::NotAnArchive {
        path: path.to_path_buf(),
        // Enough to recognise a saved web page or a text file by, short
        // enough to sit inside a one-line message.
        head: String::from_utf8_lossy(&head[..head.len().min(24)])
            .escape_debug()
            .to_string(),
        file: source.file_name,
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
    resolve_from(&JMDICT, JMDICT.url, source_dir, RETRY_BACKOFF)
}

/// [`resolve`] for KANJIDIC2: the second half of `ensure_dictionary`'s
/// source pair. A hand-placed `kanjidic2.xml.gz` or `kanjidic2.xml`-content
/// file at that name is honoured exactly as a hand-placed JMdict is.
pub fn resolve_kanjidic(source_dir: &Path) -> std::io::Result<Box<dyn BufRead>> {
    resolve_from(&KANJIDIC, KANJIDIC.url, source_dir, RETRY_BACKOFF)
}

/// [`resolve`] with the source, URL and backoff injected, so tests can point
/// at a local listener without touching the real network or sleeping at the
/// production backoff — the same reason [`fetch::fetch_with_retry`] exists
/// for [`fetch::fetch`].
///
/// **Not part of the supported surface.** Production callers use [`resolve`]
/// and [`resolve_kanjidic`].
pub fn resolve_from(
    source: &Source,
    url: &str,
    source_dir: &Path,
    backoff: std::time::Duration,
) -> std::io::Result<Box<dyn BufRead>> {
    let target = source_dir.join(source.file_name);
    let path = if target.exists() {
        target
    } else {
        fetch::fetch_with_retry(source, url, source_dir, backoff).map_err(std::io::Error::other)?
    };
    open_local(source, &path).map_err(std::io::Error::other)
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
        assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), XML);
    }

    /// The extension says `.gz`, but the bytes decide. A user who decompressed
    /// the archive by hand and kept the name must still work.
    #[test]
    fn a_plain_xml_file_is_passed_through() {
        let dir = scratch("open-plain");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, XML).expect("write");
        assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), XML);
    }

    /// A hand-trimmed file that starts at the root element, with no prolog.
    #[test]
    fn a_plain_file_starting_at_the_root_element_is_passed_through() {
        let dir = scratch("open-root");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<JMdict></JMdict>").expect("write");
        assert_eq!(
            read_all(open_local(&JMDICT, &path).expect("open")),
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
        assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), bytes);
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

        let err = open_local(&JMDICT, &path).err().expect("must fail");
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

        let err = open_local(&JMDICT, &path).err().expect("must fail");
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

        let err = open_local(&JMDICT, &path).err().expect("must fail");
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

        let err = open_local(&JMDICT, &path).err().expect("must fail");
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
        let err = open_local(&JMDICT, &dir.join(SOURCE_FILE))
            .err()
            .expect("must fail");
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

    const KANJIDIC_XML: &[u8] = b"<?xml version=\"1.0\"?><kanjidic2></kanjidic2>";

    #[test]
    fn the_kanjidic_descriptor_is_the_edrdg_one() {
        assert_eq!(KANJIDIC.file_name, "kanjidic2.xml.gz");
        // Plain HTTP on the same host as JMdict, for the same reason: this
        // crate has no TLS backend. Verified reachable 2026-09-11 (HTTP 200,
        // Content-Length 1488571).
        assert_eq!(
            KANJIDIC.url,
            "http://ftp.edrdg.org/pub/Nihongo/kanjidic2.xml.gz"
        );
        assert_eq!(JMDICT.url, "http://ftp.edrdg.org/pub/Nihongo/JMdict_e.gz");
        assert_eq!(SOURCE_FILE, JMDICT.file_name);
    }

    /// Each opening a real or hand-trimmed KANJIDIC2 can have passes the
    /// root-aware check: declaration then root, DOCTYPE then root, bare
    /// root, and — the shape of the published file — declaration, comments,
    /// DOCTYPE.
    #[test]
    fn every_legitimate_kanjidic_opening_is_accepted() {
        for (i, head) in [
            &b"<?xml version=\"1.0\"?><kanjidic2></kanjidic2>"[..],
            b"<!DOCTYPE kanjidic2 [\n]><kanjidic2></kanjidic2>",
            b"<kanjidic2></kanjidic2>",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- KANJIDIC2 -->\n\
              <!-- 2026-09-11 -->\n<!DOCTYPE kanjidic2 [\n]>\n<kanjidic2></kanjidic2>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-kanjidic-opening-{i}"));
            let path = dir.join(KANJIDIC.file_name);
            std::fs::write(&path, head).expect("write");
            assert_eq!(read_all(open_local(&KANJIDIC, &path).expect("open")), *head);
        }
    }

    /// The JMdict side of the same table, including the published file's own
    /// shape: a declaration, a run of revision comments, then `<!DOCTYPE
    /// JMdict [` opening an internal subset. The declaration-then-root and
    /// bare-root shapes are already covered by the two pass-through tests
    /// above.
    #[test]
    fn every_legitimate_jmdict_opening_is_accepted() {
        for (i, head) in [
            &b"<!DOCTYPE JMdict [\n<!ENTITY n \"noun\">\n]>\n<JMdict></JMdict>"[..],
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- Rev 1.09\n\tAdded the \
              g_type attribute\n-->\n<!-- Rev 1.08 -->\n<!DOCTYPE JMdict [\n]>\n<JMdict></JMdict>",
            b"<JMdict/>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-jmdict-opening-{i}"));
            let path = dir.join(SOURCE_FILE);
            std::fs::write(&path, head).expect("write");
            assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), *head);
        }
    }

    /// The check is per source: a hand-trimmed JMdict copy — one that starts
    /// at its root element — saved under the KANJIDIC2 name would parse as a
    /// KANJIDIC2 with zero characters and build an index with no kanji
    /// block, silently. It is refused at the only boundary where it is still
    /// recognisable as the wrong file. A declaration-led copy was once
    /// indistinguishable here, because `<?xml` was an accept for either
    /// source; since Phase 3c the name behind the declaration decides — see
    /// the next test.
    #[test]
    fn a_trimmed_jmdict_file_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-wrong-file");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, b"<JMdict></JMdict>").expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
        assert!(!msg.contains("JMdict_e.gz"), "named the wrong file: {msg}");
    }

    #[test]
    fn an_html_page_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-html");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, b"<!DOCTYPE html><html><body>404</body></html>").expect("write");
        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// The case the Phase 3b handoff §5 recorded as open: a JMdict file with
    /// its declaration intact, saved under the KANJIDIC2 name.
    #[test]
    fn a_declared_jmdict_file_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-declared-jmdict");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, XML).expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
        assert!(!msg.contains("JMdict_e.gz"), "named the wrong file: {msg}");
    }

    /// The gzip path is sniffed too. A real `JMdict_e.gz` renamed to
    /// `kanjidic2.xml.gz` used to pass unexamined — the old branch returned
    /// the decoder without looking at what it decoded — and the message
    /// quotes the decompressed head, which is what the user would recognise.
    #[test]
    fn a_gzipped_jmdict_archive_under_the_kanjidic_name_is_rejected() {
        let dir = scratch("open-kanjidic-gzipped-jmdict");
        let path = dir.join(KANJIDIC.file_name);
        std::fs::write(&path, gz(XML)).expect("write");

        let err = open_local(&KANJIDIC, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("<?xml"),
            "must quote the decompressed head: {msg}"
        );
        assert!(msg.contains("kanjidic2.xml.gz"), "got {msg}");
    }

    /// And the converse, so neither source is the lenient one.
    #[test]
    fn a_gzipped_kanjidic_archive_under_the_jmdict_name_is_rejected() {
        let dir = scratch("open-jmdict-gzipped-kanjidic");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(KANJIDIC_XML)).expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// A served error page that happens to arrive gzipped.
    #[test]
    fn a_gzipped_html_page_is_rejected() {
        let dir = scratch("open-gzipped-html");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, gz(b"<!DOCTYPE html><html><body>404</body></html>")).expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// The root's name must be the whole name: `JMdictionary` is not
    /// `JMdict`, and neither is a DOCTYPE for one. The delimiter after the
    /// name is what separates these from `<JMdict>` and `<!DOCTYPE JMdict [`.
    #[test]
    fn a_root_that_merely_starts_with_the_name_is_rejected() {
        for (i, head) in [
            &b"<JMdictionary></JMdictionary>"[..],
            b"<!DOCTYPE JMdictX><JMdictX/>",
        ]
        .iter()
        .enumerate()
        {
            let dir = scratch(&format!("open-jmdict-near-miss-{i}"));
            let path = dir.join(SOURCE_FILE);
            std::fs::write(&path, head).expect("write");
            let err = open_local(&JMDICT, &path).err().expect("must fail");
            assert!(
                matches!(err, SourceError::NotAnArchive { .. }),
                "got {err:?}"
            );
        }
    }

    /// A preamble longer than the peek is undecidable, and undecidable is an
    /// accept: the file opens and reads back whole. No EDRDG file is shaped
    /// like this; the point is that a stricter check must not start refusing
    /// a file the old one opened.
    #[test]
    fn a_preamble_that_outruns_the_peek_is_accepted() {
        let dir = scratch("open-long-preamble");
        let path = dir.join(SOURCE_FILE);
        let mut bytes = b"<?xml version=\"1.0\"?>\n<!-- ".to_vec();
        bytes.resize(bytes.len() + PEEK_BYTES, b'x');
        bytes.extend_from_slice(b" -->\n<JMdict></JMdict>");
        std::fs::write(&path, &bytes).expect("write");
        assert_eq!(read_all(open_local(&JMDICT, &path).expect("open")), bytes);
    }

    /// The same shape cut short by the end of the file is not undecidable:
    /// everything the file holds was read, and none of it is a root element.
    #[test]
    fn a_short_file_that_ends_inside_its_preamble_is_rejected() {
        let dir = scratch("open-short-preamble");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<?xml version=\"1.0\"?>\n<!-- never closed").expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// A file that is a declaration and nothing else is not a dictionary
    /// either, and the old rule would have opened it.
    #[test]
    fn a_bare_declaration_is_rejected() {
        let dir = scratch("open-bare-declaration");
        let path = dir.join(SOURCE_FILE);
        std::fs::write(&path, b"<?xml version=\"1.0\"?>\n").expect("write");
        let err = open_local(&JMDICT, &path).err().expect("must fail");
        assert!(
            matches!(err, SourceError::NotAnArchive { .. }),
            "got {err:?}"
        );
    }

    /// The peek must not lose a byte: a file longer than the peek reads back
    /// whole through the chained reader, plain and gzipped, with the DOCTYPE
    /// where the real archives put it — past a kilobyte of comments.
    #[test]
    fn a_file_longer_than_the_peek_reads_back_whole() {
        let mut bytes = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- ".to_vec();
        bytes.resize(bytes.len() + 1_200, b'r');
        bytes.extend_from_slice(b" -->\n<!DOCTYPE JMdict [\n]>\n<JMdict>");
        bytes.resize(bytes.len() + 3 * PEEK_BYTES, b'.');
        bytes.extend_from_slice(b"</JMdict>\n");

        let dir = scratch("open-longer-than-peek");
        let plain = dir.join(SOURCE_FILE);
        std::fs::write(&plain, &bytes).expect("write plain");
        assert_eq!(
            read_all(open_local(&JMDICT, &plain).expect("open plain")),
            bytes
        );

        let zipped = dir.join("JMdict_e.zipped.gz");
        std::fs::write(&zipped, gz(&bytes)).expect("write gz");
        assert_eq!(
            read_all(open_local(&JMDICT, &zipped).expect("open gz")),
            bytes
        );
    }

    #[test]
    fn a_present_kanjidic_archive_is_opened_without_downloading() {
        let dir = scratch("resolve-kanjidic-local");
        std::fs::write(dir.join(KANJIDIC.file_name), gz(KANJIDIC_XML)).expect("write");
        assert_eq!(
            read_all(resolve_kanjidic(&dir).expect("resolve")),
            KANJIDIC_XML
        );
    }

    /// The two sources resolve independently: a KANJIDIC2 archive does not
    /// satisfy `resolve`, and vice versa. Without the file at its own name
    /// `resolve_from` would go to the injected URL, so an unroutable one
    /// makes the miss observable without a listener.
    #[test]
    fn each_source_resolves_only_its_own_file() {
        let dir = scratch("resolve-independent");
        std::fs::write(dir.join(KANJIDIC.file_name), gz(KANJIDIC_XML)).expect("write");
        // A port a listener just released, not the fixed `127.0.0.1:1`: that
        // relied on a privileged port refusing, which is a guess about the
        // machine running the test, not a guarantee of it.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        let err = resolve_from(
            &JMDICT,
            &format!("http://{addr}/JMdict_e.gz"),
            &dir,
            std::time::Duration::ZERO,
        )
        .err()
        .expect("JMdict is absent and must not be satisfied by KANJIDIC2");
        assert!(err.to_string().contains("JMdict_e.gz"), "got {err}");
    }
}
