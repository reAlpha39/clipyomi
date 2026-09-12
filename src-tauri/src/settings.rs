// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persisted user settings.
//!
//! Unknown keys survive a rewrite so a file written by a later version is not
//! silently truncated by this one, and `normalize` folds the pre-4A
//! `decorations` boolean into `chrome_mode` on the way in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Filename inside the app config dir. A sibling of `dict/`, never inside it —
/// a published generation directory is immutable.
pub const SETTINGS_FILE: &str = "settings.json";

fn default_true() -> bool {
    true
}

/// Which of §7.4's three window chrome shapes the main window wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChromeMode {
    /// The OS draws the title bar and its buttons. macOS and Linux only.
    #[default]
    Native,
    /// Frameless, our 28px band, revealed by a hover.
    Header,
    /// Frameless, our band never revealed. Maximum real estate over a game.
    Content,
}

/// Whether this platform can present `ChromeMode::Native`.
///
/// Windows cannot: `apply_dwm_attributes` and the `WM_STYLECHANGING` arm in
/// `commands.rs` strip `WS_CAPTION` for the window's whole life (`d07711f`,
/// `e45d55f`), which makes the in-app band the title bar there. Restoring the
/// OS caption re-opens the DWM shadow that `e45d55f` spent a commit removing,
/// so it is its own sub-phase.
pub fn native_supported() -> bool {
    !cfg!(target_os = "windows")
}

/// A mode the running platform can actually draw.
///
/// Takes the platform answer as a parameter rather than reading `cfg!` inside,
/// so both arms are testable wherever the suite runs — the same reason
/// `peek_grows_frame` is a function and not a `cfg!` at each use site.
pub fn supported(mode: ChromeMode, native_supported: bool) -> ChromeMode {
    match mode {
        ChromeMode::Native if !native_supported => ChromeMode::Header,
        other => other,
    }
}

/// One layout slot (§7.9): the whole shape of the window, restorable with one
/// keystroke.
///
/// Geometry and `chrome_mode` are not `Option` — a bind always has a real
/// window box and a mode that has already been through `supported()`. The rest
/// are, for the reason 3C and 4B both give: "unset, `tokens.css` owns the
/// default" is a reachable state, and a slot bound while the font boxes were
/// empty has to restore them empty rather than silently pinning 24 px.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutSlot {
    pub window_width: u32,
    pub window_height: u32,
    pub window_x: i32,
    pub window_y: i32,
    pub chrome_mode: ChromeMode,
    pub always_on_top: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_alpha: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_sentence: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_gloss: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_mode: Option<String>,
    /// Keys a later version added, carried through on rewrite for the same
    /// reason `Settings::extra` exists.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
    /// Whether MeCab boundary hints are switched on (§7.1).
    ///
    /// Owned by the backend outright: `commands::merge_saved` is a whitelist
    /// and does not carry this key, the settings window never sends it, and
    /// `set_mecab_hints` is the only writer. The alternative — a snapshot
    /// written by an unrelated font-box change clobbering an in-flight toggle —
    /// is silent when it happens.
    #[serde(default)]
    pub mecab_hints: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chrome_mode: Option<ChromeMode>,
    /// Pre-4A, replaced by `chrome_mode`. Read once by `normalize` to seed the
    /// new key, then cleared — which is what makes the migration
    /// self-completing: it stops being written the first time anything saves.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decorations: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_pos: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_xrefs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_usage: Option<bool>,
    /// Phase 3a: hides the KANJIDIC block at the foot of the popover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_kanji: Option<bool>,
    /// Sentence pane size in px. `None` leaves `tokens.css` owning the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_sentence: Option<u32>,
    /// Definition and tooltip size in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_gloss: Option<u32>,
    /// Furigana size as a percentage of the sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_scale: Option<u32>,
    /// Space between the furigana and its text, in px. A size like the three
    /// above, assigned in `merge_saved` so an emptied box clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_gap: Option<u32>,
    /// Window background opacity as a percentage (§7.5). `None` leaves
    /// `tokens.css` owning the default. Not clamped here: out-of-range values
    /// from a hand-edited file are pinned at apply time by `clampAlpha`,
    /// exactly as the font sizes are.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_alpha: Option<u32>,
    // Text appearance. Each `None` leaves `tokens.css` owning the default,
    // exactly as the font sizes do. The colours are `#rrggbb` strings and the
    // families bare names; neither is validated here — a hand-edited file is
    // pinned at apply time by `clampColour` and `clampFamily`, as the sizes
    // are by `clampFont`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_kanji: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_kana: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_particle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_counter: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_furigana: Option<String>,
    /// Punctuation and runs the parser matched nothing for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_unmatched: Option<String>,
    /// The main window's background. A colour kind like the six above, sitting
    /// with the opacity slider in the settings window because it is what they
    /// are read against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub colour_bg: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family_sentence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family_furigana: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family_gloss: Option<String>,
    /// A stroke around the sentence text, for reading it over a see-through
    /// window. A toggle like `hide_*`, merged the same way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_outline: Option<bool>,
    /// The stroke's width in px. A size like the font sizes, not a toggle:
    /// assigned in `merge_saved`, so an emptied box clears it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_outline_width: Option<u32>,
    /// A drop shadow under the sentence text. A toggle like `text_outline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_shadow: Option<bool>,
    /// The shadow's size in px, which its offset and blur are both derived
    /// from. A size like `text_outline_width`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_shadow_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
    /// The nine layout slots (§7.9), keyed by digit, holding only the ones
    /// actually bound.
    ///
    /// A `String` key rather than a `u8`: `load` turns any parse failure into
    /// the whole file reset to defaults, so a stricter key type would make one
    /// hand-typed slot name cost the user every other setting too. An unknown
    /// key here is simply never looked up.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout_slots: BTreeMap<String, LayoutSlot>,
    /// Keys this version does not know about, carried through on rewrite so a
    /// file written by a later version is not truncated by this one.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            always_on_top: false,
            clipboard_monitoring: true,
            mecab_hints: false,
            chrome_mode: None,
            decorations: None,
            furigana_mode: None,
            hide_pos: None,
            hide_xrefs: None,
            hide_usage: None,
            hide_kanji: None,
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            furigana_gap: None,
            surface_alpha: None,
            colour_kanji: None,
            colour_kana: None,
            colour_particle: None,
            colour_counter: None,
            colour_furigana: None,
            colour_unmatched: None,
            colour_bg: None,
            font_family_sentence: None,
            font_family_furigana: None,
            font_family_gloss: None,
            text_outline: None,
            text_outline_width: None,
            text_shadow: None,
            text_shadow_size: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
            layout_slots: BTreeMap::new(),
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("writing {path} failed: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("encoding settings failed: {0}")]
    Encode(#[from] serde_json::Error),
}

/// The settings file inside an app config directory.
pub fn settings_path(app_config_dir: &Path) -> PathBuf {
    app_config_dir.join(SETTINGS_FILE)
}

/// Fold the pre-4A `decorations` boolean into `chrome_mode`, clear it, and pin
/// the result to a mode this platform can draw.
///
/// Done here, once, rather than behind an accessor: the same call
/// `initial_height`'s comment makes for geometry — one place converts and
/// nothing downstream has to remember to. After this runs, `chrome_mode` is
/// always `Some`, and `decorations` is always `None`.
fn normalize(mut settings: Settings) -> Settings {
    let from_legacy = match settings.decorations {
        Some(false) => ChromeMode::Header,
        // `Some(true)` and `None` alike: the pre-4A default was a title bar.
        _ => ChromeMode::default(),
    };
    let mode = settings.chrome_mode.unwrap_or(from_legacy);
    settings.chrome_mode = Some(supported(mode, native_supported()));
    settings.decorations = None;
    settings
}

/// Load settings, falling back to defaults.
///
/// Returns the settings and, when something was wrong with the file, a reason to
/// show the user. A missing file is not wrong — it is first run — so it reports
/// no reason. Never fails: settings are not important enough to block launch.
pub fn load(path: &Path) -> (Settings, Option<String>) {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        // Any read failure is treated as absent. Distinguishing "not found" from
        // "unreadable" would change nothing: both mean we start from defaults.
        Err(_) => return (normalize(Settings::default()), None),
    };

    match serde_json::from_str(&raw) {
        Ok(settings) => (normalize(settings), None),
        Err(e) => (
            normalize(Settings::default()),
            Some(format!(
                "{} could not be read, using defaults: {e}",
                path.display()
            )),
        ),
    }
}

/// Write settings, creating the parent directory if needed.
pub fn save(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| SettingsError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let encoded = serde_json::to_string_pretty(settings)?;
    std::fs::write(path, encoded).map_err(|source| SettingsError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests;
