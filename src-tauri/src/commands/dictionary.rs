// SPDX-License-Identifier: MIT OR Apache-2.0

//! The dictionary commands: whether startup found an index, the download
//! that builds one and publishes it to the parse worker, and the kanji
//! lookup the popover asks for on hover. Moved out of `commands.rs` in
//! Phase 3c with no change to the code; the download slot and `reparse`
//! live here and are shared with `hints`, which is why they are
//! `pub(super)`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};
use tokio::sync::watch;

use crate::state::{AppState, NeedsDictionary};

/// Emitted with a `String` payload as the download moves between phases:
/// `"downloading"`, `"building"`, `"ready"`, or an error message.
pub const DICTIONARY_STATUS_EVENT: &str = "dictionary-status";

/// The sending half of the index channel, managed so `download_dictionary` can
/// publish a freshly built index to the already-running worker.
pub struct IndexSender(pub watch::Sender<Option<Arc<AppState>>>);

/// Where the dictionaries live. Resolved once in `main`'s `setup`, so no command
/// needs the app config dir, and so `source/` stays a sibling of `dict/` rather
/// than living inside a published generation, which is immutable.
pub struct DictionaryPaths {
    pub root: PathBuf,
    pub source_dir: PathBuf,
    /// The MeCab hints directory: `<app_config_dir>/hints`, holding the single
    /// published slot and any archive the user drops in beside it.
    pub hints_dir: PathBuf,
}

/// Guards against two concurrent `ensure_dictionary` runs. An `Arc` rather than
/// a bare `AtomicBool` because the command must clone it out of `State` before
/// its first `.await` — `tauri::State` is not `Send`.
pub struct DownloadInFlight(pub Arc<AtomicBool>);

/// Take the download slot, or report that it is already taken.
///
/// Separated from the command so the exclusion is testable without a Tauri app
/// handle or a real download.
pub(super) fn claim_download(flag: &AtomicBool) -> Result<(), String> {
    if flag.swap(true, Ordering::SeqCst) {
        Err("a dictionary download is already running".to_string())
    } else {
        Ok(())
    }
}

/// Release the slot. Called on every exit path, success or failure — a failed
/// download that never released would disable Retry for the rest of the session.
pub(super) fn release_download(flag: &AtomicBool) {
    flag.store(false, Ordering::SeqCst);
}

/// Re-parse whatever is on screen.
///
/// Enabling hints changes the segmentation of text already displayed, and
/// without this the toggle appears inert until the next clipboard copy — which
/// for a segmentation-quality switch is the entire feedback the user gets.
///
/// An empty channel value means nothing has been parsed yet, so there is
/// nothing to re-parse. `watch::send` marks the slot changed even for an equal
/// value, so the worker wakes and parses the same text again, and
/// `run_worker`'s loop needs no change — `next_input`'s three tests keep
/// pinning the same code.
///
/// Making the worker `select!` over both channels was the alternative. It puts
/// the rule where any future publisher inherits it, at the cost of rewriting
/// the loop those tests cover and making the worker retain the last input
/// itself.
pub(super) fn reparse(tx: &watch::Sender<String>) {
    let text = tx.borrow().clone();
    if text.is_empty() {
        return;
    }
    // A dead worker is reported by every other path that touches this channel;
    // a settings toggle is not where the user should learn about it.
    let _ = tx.send(text);
}

/// Whether startup found no dictionary, so the webview should show the download
/// screen rather than the panes.
#[tauri::command]
pub fn needs_dictionary(needs: State<'_, NeedsDictionary>) -> bool {
    needs.0
}

/// The KANJIDIC2 records for the kanji in `chars`, in input order, with
/// misses and non-kanji omitted. Split from the command for the reason
/// `merge_saved` is: a `#[tauri::command]` taking `State<'_, _>` needs a mock
/// app to call, and the lookup rule is worth pinning on its own.
pub(crate) fn lookup_kanji(
    index: &jparser::index::load::Index,
    chars: &str,
) -> Vec<jparser::index::KanjiData> {
    chars
        .chars()
        .filter_map(|ch| index.kanji(ch).cloned())
        .collect()
}

/// The popover's kanji block: one record per character of `chars`, which the
/// webview sends as the distinct kanji of the hovered surface. Asked lazily
/// on hover so the parse payload does not grow (Phase 3a design §5). Empty
/// until a dictionary has been published to the worker.
#[tauri::command]
pub fn kanji_info(chars: String, index: State<'_, IndexSender>) -> Vec<jparser::index::KanjiData> {
    // Cloned out of the borrow before the lookup: `watch::Sender::borrow`
    // holds the channel's read lock, and a `download_dictionary` publish
    // would otherwise wait on this command.
    let state = index.0.borrow().clone();
    match state {
        Some(state) => lookup_kanji(&state.index, &chars),
        None => Vec::new(),
    }
}

/// Emit a phase label, logging rather than propagating a failed emit.
fn emit_status(app: &AppHandle, status: &str) {
    if let Err(e) = app.emit(DICTIONARY_STATUS_EVENT, status.to_string()) {
        // Nothing to fall back on: if the event cannot reach the webview there
        // is no other channel to report progress through.
        eprintln!("emitting a dictionary-status event failed: {e}");
    }
}

/// The failure text for `err`, preferring a more specific inner error when
/// one is recoverable.
///
/// `ensure_dictionary`'s source closure hands `resolve`'s failure to it as a
/// plain `std::io::Error` — `jmdict_source::resolve_from` boxes the original
/// `SourceError` via `std::io::Error::other`, since `jmdict-source` cannot
/// depend on `jparser`'s `IndexError` to return one directly — and
/// `ensure_dictionary` then wraps *that* a second time as `IndexError::Io`.
/// Shown as-is, that reads as "index io failed: could not obtain the
/// dictionary after 3 attempts (...)": the outer wrapper's prefix describes a
/// build-time I/O failure, which is not what happened. Downcasting recovers
/// the original `SourceError` and shows its own, more specific message
/// instead. `jmdict_source`'s own `the_typed_error_survives_the_io_wrapper`
/// test exists to prove this downcast is reliable; a genuine index-level I/O
/// failure (no `SourceError` inside) falls through to `IndexError`'s own
/// message unchanged.
fn describe_index_error(err: &jparser::index::IndexError) -> String {
    if let jparser::index::IndexError::Io(io_err) = err {
        if let Some(source_err) = io_err
            .get_ref()
            .and_then(|e| e.downcast_ref::<jmdict_source::SourceError>())
        {
            return source_err.to_string();
        }
    }
    err.to_string()
}

/// Append the manual-fallback instructions to `reason`, unless it already
/// names `source_dir_display`.
///
/// `SourceError::Http` and `SourceError::TooManyAttempts` already end with
/// "place a <file> in `<source_dir>` manually to bypass the download"
/// (`crates/jmdict-source/src/lib.rs`), naming whichever of the two files
/// failed — appending again would say it twice on screen. Every other
/// failure (a corrupt archive, a dead worker) says nothing about the
/// directory at all, so this is the only place those learn about the
/// escape hatch, and it names both files because it cannot tell which one
/// the reader is missing.
fn with_fallback(reason: String, source_dir_display: &str) -> String {
    if reason.contains(source_dir_display) {
        format!("{reason}. Retry.")
    } else {
        format!(
            "{reason}. Retry, or place {} and {} in {source_dir_display} and retry.",
            jmdict_source::JMDICT.file_name,
            jmdict_source::KANJIDIC.file_name,
        )
    }
}

/// Download JMdict if absent, build the index, and publish it to the worker.
///
/// Returns `Err` only when the work could not be *started*; every outcome after
/// that — including failure — arrives as a `dictionary-status` event. Same
/// split as `set_input`, and for the same reason: one path to the screen.
#[tauri::command]
pub async fn download_dictionary(
    app: AppHandle,
    paths: State<'_, DictionaryPaths>,
    index: State<'_, IndexSender>,
    inflight: State<'_, DownloadInFlight>,
) -> Result<(), String> {
    claim_download(&inflight.0)?;

    // Everything needed after the `.await` is cloned out first: `tauri::State`
    // is not `Send`, so holding one across an await point does not compile.
    let flag = Arc::clone(&inflight.0);
    let root = paths.root.clone();
    let source_dir = paths.source_dir.clone();
    // Read before `source_dir` moves into the closure below, so the fallback
    // message can still be built from it once the closure has finished.
    let source_dir_display = source_dir.display().to_string();
    let tx = index.0.clone();
    let handle = app.clone();

    let outcome = tauri::async_runtime::spawn_blocking(move || {
        emit_status(&handle, "downloading");

        // `resolve` reuses an already-downloaded archive before it reaches the
        // network, so a retry after a *build* failure is fast and offline.
        // `ensure_dictionary` runs the closure only when a rebuild is needed.
        let index = jparser::index::ensure_dictionary(
            &root,
            jparser::index::generations::DEFAULT_KEEP_GENERATIONS,
            || {
                let jmdict = jmdict_source::resolve(&source_dir)?;
                let kanjidic = jmdict_source::resolve_kanjidic(&source_dir)?;
                // Emitted only after both resolves return, not before: the
                // network fetches (or reuse of already-downloaded archives)
                // happen inside `resolve` itself, so emitting "building" any
                // earlier would mislabel a download as index-building.
                // "downloading" covers both files; "building" covers the
                // `build_from_readers` work that follows.
                emit_status(&handle, "building");
                Ok((jmdict, kanjidic))
            },
        )
        .map_err(|e| describe_index_error(&e))?;

        Ok::<AppState, String>(AppState {
            index,
            table: jparser::engine::table::Table::new(),
        })
    })
    .await;

    release_download(&flag);

    let message = match outcome {
        Ok(Ok(state)) => {
            if tx.send(Some(Arc::new(state))).is_err() {
                // The worker is gone, which no retry fixes. Reported like any
                // other failure rather than swallowed.
                "the parse worker is no longer running".to_string()
            } else {
                emit_status(&app, "ready");
                return Ok(());
            }
        }
        // `with_fallback` names the archive and directory only when `message`
        // does not already: `resolve` serves a hand-placed file without
        // touching the network, so a user behind a proxy can drop it in and
        // press Retry either way.
        Ok(Err(message)) => with_fallback(message, &source_dir_display),
        Err(e) => with_fallback(
            format!("the download task failed to run: {e}"),
            &source_dir_display,
        ),
    };

    emit_status(&app, &message);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::Manager;

    /// Two overlapping `ensure_dictionary` runs would build two generations
    /// against one source directory. The frontend's own guard protects one
    /// button in one webview, not the command, so the command needs its own.
    #[test]
    fn a_second_download_is_rejected_while_one_is_running() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("the first claim succeeds");
        assert!(
            claim_download(&flag).is_err(),
            "a concurrent claim must be refused"
        );
    }

    /// A finished download — successful or not — must leave the door open, or
    /// one failure would disable Retry for the rest of the session.
    #[test]
    fn releasing_the_claim_allows_a_retry() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        claim_download(&flag).expect("first");
        release_download(&flag);
        claim_download(&flag).expect("retry after release");
    }

    /// Final review, Finding 1: `resolve_from` boxes a `SourceError` inside a
    /// plain `io::Error` (`std::io::Error::other`), which `ensure_dictionary`
    /// wraps again as `IndexError::Io`. The downcast must recover the
    /// original message rather than showing the generic "index io failed: "
    /// wrapper ahead of it.
    #[test]
    fn a_wrapped_source_error_replaces_the_generic_io_prefix() {
        let source_err = jmdict_source::SourceError::TooManyAttempts {
            attempts: 3,
            source_dir: PathBuf::from("/tmp/example/source"),
            last: "downloading the dictionary failed: connection refused".to_string(),
            file: jmdict_source::SOURCE_FILE,
        };
        let expected = source_err.to_string();
        let wrapped = jparser::index::IndexError::Io(std::io::Error::other(source_err));

        let reason = describe_index_error(&wrapped);
        assert_eq!(reason, expected, "must show SourceError's own message");
        assert!(
            !reason.starts_with("index io failed"),
            "the generic wrapper prefix must not leak: {reason}"
        );
    }

    /// An `IndexError::Io` with no `SourceError` inside it is a genuine
    /// index-level I/O failure (e.g. a disk full while publishing a
    /// generation), which `IndexError`'s own "index io failed: " prefix
    /// describes correctly — the downcast must not touch it.
    #[test]
    fn a_genuine_index_io_error_keeps_its_own_message() {
        let wrapped = jparser::index::IndexError::Io(std::io::Error::other("disk full"));
        let reason = describe_index_error(&wrapped);
        assert_eq!(reason, wrapped.to_string());
        assert!(reason.starts_with("index io failed"), "got {reason}");
    }

    /// `SourceError::Http` and `SourceError::TooManyAttempts` already name the
    /// source directory as part of their own manual-fallback advice —
    /// appending a second copy would say it twice, which is the bug the whole
    /// review finding is about.
    #[test]
    fn the_fallback_is_not_repeated_when_the_reason_already_names_the_directory() {
        let dir = "/tmp/example/source";
        let reason = format!(
            "could not obtain the dictionary after 3 attempts (...); place a \
             JMdict_e.gz in {dir} manually to bypass the download"
        );

        let full = with_fallback(reason, dir);

        assert_eq!(
            full.matches(dir).count(),
            1,
            "the directory must appear exactly once: {full}"
        );
        assert!(full.contains("Retry"), "got {full}");
    }

    /// A corrupt-archive build failure (`IndexError::Jmdict`) never mentions
    /// the source directory on its own, so the fallback must still be added —
    /// this is the escape hatch's only appearance for that failure.
    #[test]
    fn the_fallback_is_added_when_the_reason_does_not_name_the_directory() {
        let dir = "/tmp/example/source";
        let full = with_fallback("reading JMdict failed: unexpected eof".to_string(), dir);

        assert!(full.contains(dir), "got {full}");
        assert!(full.contains("Retry"), "got {full}");
        assert!(full.contains(jmdict_source::SOURCE_FILE), "got {full}");
        assert!(
            full.contains(jmdict_source::KANJIDIC.file_name),
            "got {full}"
        );
    }

    /// Nothing has been parsed yet, so there is nothing to re-parse. Sending an
    /// empty string would wake the worker to parse nothing.
    #[tokio::test]
    async fn reparse_skips_an_empty_channel() {
        let (tx, rx) = tokio::sync::watch::channel(String::new());
        reparse(&tx);
        assert!(!rx.has_changed().expect("the sender is alive"));
    }

    /// Enabling hints changes the segmentation of text already on screen. Without
    /// this the toggle looks inert until the next clipboard copy, which for a
    /// segmentation-quality switch is the entire feedback the user gets.
    ///
    /// `watch::send` marks the slot changed even for an equal value, which is the
    /// mechanism this relies on.
    #[tokio::test]
    async fn reparse_resends_the_current_text() {
        let (tx, mut rx) = tokio::sync::watch::channel(String::new());
        crate::commands::push_input(&tx, "東京都".to_string()).expect("push");
        assert_eq!(
            crate::parse::next_input(&mut rx).await.as_deref(),
            Some("東京都"),
            "the first send is consumed so the re-send is what the assert below sees"
        );

        reparse(&tx);

        assert_eq!(
            crate::parse::next_input(&mut rx).await.as_deref(),
            Some("東京都")
        );
    }

    /// Hits come back in input order; kana and characters the index has no
    /// record for are simply absent. The fixture index carries 本 only.
    #[test]
    fn lookup_kanji_returns_hits_in_order_and_omits_misses() {
        let root = crate::test_support::scratch("kanji-lookup");
        let generation = crate::test_support::build_index_generation(&root);
        let index = jparser::index::load::Index::open(&generation).expect("open");

        let hits = lookup_kanji(&index, "あ本東");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].literal, '本');
        assert_eq!(hits[0].on, vec!["ホン"]);
        assert_eq!(hits[0].kun, vec!["もと"]);
        // The parser's cap, visible end to end.
        assert_eq!(hits[0].meanings, vec!["book", "present", "main", "origin"]);

        assert!(lookup_kanji(&index, "").is_empty());
        assert!(lookup_kanji(&index, "はいた").is_empty());
    }

    /// Before the first download the channel holds `None`; the command must
    /// answer empty rather than fail, because the popover can open on a
    /// stale parse while the download screen is up.
    #[test]
    fn kanji_info_is_empty_before_a_dictionary_is_loaded() {
        let app = tauri::test::mock_app();
        let (tx, _rx) = tokio::sync::watch::channel(None::<Arc<AppState>>);
        app.manage(IndexSender(tx));
        let out = kanji_info("本".to_string(), app.state::<IndexSender>());
        assert!(out.is_empty());
    }

    #[test]
    fn kanji_info_answers_from_the_published_index() {
        let root = crate::test_support::scratch("kanji-info");
        let generation = crate::test_support::build_index_generation(&root);
        let index = jparser::index::load::Index::open(&generation).expect("open");
        let state = AppState {
            index,
            table: jparser::engine::table::Table::new(),
        };
        let app = tauri::test::mock_app();
        let (tx, _rx) = tokio::sync::watch::channel(Some(Arc::new(state)));
        app.manage(IndexSender(tx));

        let out = kanji_info("本".to_string(), app.state::<IndexSender>());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].literal, '本');
    }
}
