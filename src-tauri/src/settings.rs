// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Persisted user settings.
//!
//! Unknown keys survive a rewrite so a file written by a later version is not
//! silently truncated by this one, and `normalize` folds the pre-4A
//! `decorations` boolean into `chrome_mode` on the way in.

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
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
    /// Sentence pane size in px. `None` leaves `tokens.css` owning the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_sentence: Option<u32>,
    /// Definition and tooltip size in px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_gloss: Option<u32>,
    /// Furigana size as a percentage of the sentence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub furigana_scale: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_x: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_y: Option<i32>,
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
            chrome_mode: None,
            decorations: None,
            furigana_mode: None,
            hide_pos: None,
            hide_xrefs: None,
            hide_usage: None,
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            window_width: None,
            window_height: None,
            window_x: None,
            window_y: None,
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
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ta-settings-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir
    }

    /// Monitoring on, always-on-top off: the app's headline behaviour works on
    /// first launch, and the window does not seize the foreground uninvited.
    #[test]
    fn defaults_are_monitoring_on_and_always_on_top_off() {
        let s = Settings::default();
        assert!(s.clipboard_monitoring);
        assert!(!s.always_on_top);
    }

    /// The three sizes phase 3C added, through the file and back. They are
    /// `Option` so that "never set" stays distinguishable from a value equal to
    /// the stylesheet default, which is what keeps `tokens.css` the one place a
    /// default lives.
    #[test]
    fn font_sizes_round_trip_through_the_file() {
        let path = scratch("font-sizes").join("settings.json");
        let written = Settings {
            font_sentence: Some(30),
            font_gloss: Some(18),
            furigana_scale: Some(70),
            ..Default::default()
        };

        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        // `load` normalizes, so what comes back is the normalized form of what
        // went in — the round trip is over `normalize`, not over the raw value.
        assert_eq!(read_back, normalize(written));
        assert!(warning.is_none(), "got {warning:?}");
    }

    #[test]
    fn an_unset_font_size_is_left_out_of_the_file() {
        let path = scratch("font-sizes-unset").join("settings.json");
        save(&path, &Settings::default()).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("font_sentence"), "got {raw}");
    }

    /// First run is not an error state.
    #[test]
    fn a_missing_file_loads_defaults_without_a_warning() {
        let path = scratch("missing").join("settings.json");
        let (s, warning) = load(&path);
        assert_eq!(s, normalize(Settings::default()));
        assert!(warning.is_none(), "got {warning:?}");
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let path = scratch("round-trip").join("settings.json");
        let written = Settings {
            always_on_top: true,
            clipboard_monitoring: false,
            ..Default::default()
        };

        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        // `load` normalizes, so what comes back is the normalized form of what
        // went in — the round trip is over `normalize`, not over the raw value.
        assert_eq!(read_back, normalize(written));
        assert!(warning.is_none(), "got {warning:?}");
    }

    /// A corrupt file must not stop the app launching — but the user has to be
    /// told, or their settings appear to silently reset themselves.
    #[test]
    fn a_corrupt_file_loads_defaults_and_reports_why() {
        let path = scratch("corrupt").join("settings.json");
        std::fs::write(&path, b"{ not json").expect("write");

        let (s, warning) = load(&path);
        assert_eq!(s, normalize(Settings::default()));
        let warning = warning.expect("a corrupt file must report a reason");
        assert!(!warning.is_empty());
    }

    /// Phases 3 and 4 add many keys. A downgrade that drops them is data loss
    /// that would surface long after the downgrade.
    #[test]
    fn unknown_keys_survive_a_rewrite() {
        let path = scratch("unknown").join("settings.json");
        std::fs::write(
            &path,
            br#"{"always_on_top":true,"clipboard_monitoring":true,"unknown_key":"all","font_size":18}"#,
        )
        .expect("write");

        let (loaded, _) = load(&path);
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(json["unknown_key"], "all", "unknown string key dropped");
        assert_eq!(json["font_size"], 18, "unknown number key dropped");
        assert_eq!(json["always_on_top"], true, "known key lost");
    }

    #[test]
    fn the_settings_path_is_a_sibling_of_the_dict_directory() {
        let root = std::path::Path::new("/tmp/cfg");
        assert_eq!(settings_path(root), root.join("settings.json"));
    }

    #[test]
    fn defaults_leave_every_optional_key_unset() {
        let s = Settings::default();
        assert_eq!(s.chrome_mode, None, "only `normalize` fills this in");
        assert_eq!(s.decorations, None);
        assert_eq!(s.furigana_mode, None);
        assert_eq!(s.hide_pos, None);
        assert_eq!(s.hide_xrefs, None);
        assert_eq!(s.hide_usage, None);
        assert_eq!(s.window_width, None);
        assert_eq!(s.window_height, None);
        assert_eq!(s.window_x, None);
        assert_eq!(s.window_y, None);
    }

    #[test]
    fn geometry_round_trips_through_json() {
        let json = r#"{
            "always_on_top": true,
            "clipboard_monitoring": false,
            "decorations": false,
            "window_width": 500,
            "window_height": 120,
            "window_x": 100,
            "window_y": 200
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(
            s.decorations,
            Some(false),
            "read verbatim; `normalize` folds it"
        );
        assert_eq!(s.window_width, Some(500));
        assert_eq!(s.window_height, Some(120));
        assert_eq!(s.window_x, Some(100));
        assert_eq!(s.window_y, Some(200));
    }

    #[test]
    fn settings_roundtrips_furigana_mode() {
        let settings = Settings {
            always_on_top: true,
            clipboard_monitoring: true,
            chrome_mode: Some(ChromeMode::Header),
            decorations: None,
            furigana_mode: Some("hiragana".to_string()),
            hide_pos: None,
            hide_xrefs: None,
            hide_usage: None,
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            window_width: Some(400),
            window_height: Some(300),
            window_x: None,
            window_y: None,
            extra: serde_json::Map::new(),
        };
        let json = serde_json::to_string(&settings).expect("serialization succeeds");
        assert!(json.contains("\"furigana_mode\":\"hiragana\""));
        let deserialized: Settings = serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(deserialized.furigana_mode, Some("hiragana".to_string()));
    }

    #[test]
    fn settings_roundtrips_gloss_filters() {
        let settings = Settings {
            always_on_top: true,
            clipboard_monitoring: true,
            chrome_mode: Some(ChromeMode::Header),
            decorations: None,
            furigana_mode: Some("hiragana".to_string()),
            hide_pos: Some(true),
            hide_xrefs: Some(false),
            hide_usage: Some(true),
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            window_width: Some(400),
            window_height: Some(300),
            window_x: None,
            window_y: None,
            extra: serde_json::Map::new(),
        };
        let json = serde_json::to_string(&settings).expect("serialization succeeds");
        assert!(json.contains("\"hide_pos\":true"));
        assert!(json.contains("\"hide_xrefs\":false"));
        assert!(json.contains("\"hide_usage\":true"));
        let deserialized: Settings = serde_json::from_str(&json).expect("deserialization succeeds");
        assert_eq!(deserialized.hide_pos, Some(true));
        assert_eq!(deserialized.hide_xrefs, Some(false));
        assert_eq!(deserialized.hide_usage, Some(true));
    }

    /// The wire names are the strings that appear in settings.json, in
    /// `data-chrome`, and in the `set_chrome_mode` argument. Pinned as strings
    /// because three surfaces spell them independently.
    #[test]
    fn chrome_modes_serialize_as_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&ChromeMode::Native).unwrap(),
            "\"native\""
        );
        assert_eq!(
            serde_json::to_string(&ChromeMode::Header).unwrap(),
            "\"header\""
        );
        assert_eq!(
            serde_json::to_string(&ChromeMode::Content).unwrap(),
            "\"content\""
        );
        assert_eq!(
            serde_json::from_str::<ChromeMode>("\"content\"").unwrap(),
            ChromeMode::Content
        );
    }

    #[test]
    fn the_default_mode_is_native() {
        assert_eq!(ChromeMode::default(), ChromeMode::Native);
    }

    /// Windows has no native-frame state at all, so a setting naming it has to
    /// resolve to the nearest shape that exists rather than draw nothing.
    /// Parameterised instead of `cfg`-gated so both arms run on both platforms.
    #[test]
    fn native_resolves_to_header_where_it_is_unsupported() {
        assert_eq!(supported(ChromeMode::Native, false), ChromeMode::Header);
        assert_eq!(supported(ChromeMode::Native, true), ChromeMode::Native);
    }

    #[test]
    fn the_other_two_modes_are_supported_everywhere() {
        for supported_here in [true, false] {
            assert_eq!(
                supported(ChromeMode::Header, supported_here),
                ChromeMode::Header
            );
            assert_eq!(
                supported(ChromeMode::Content, supported_here),
                ChromeMode::Content
            );
        }
    }

    #[test]
    fn only_windows_lacks_the_native_frame() {
        assert_eq!(native_supported(), !cfg!(target_os = "windows"));
    }

    /// The pre-4A boolean folds into the new key, and the mode is pinned to
    /// what the platform can draw. `native_supported()` decides the second
    /// column of the spec's §5 table, which is why every expectation here is
    /// written against it rather than against a hardcoded mode.
    #[test]
    fn a_legacy_decorations_true_becomes_native_where_native_exists() {
        let s: Settings = serde_json::from_str(r#"{"decorations":true}"#).unwrap();
        let s = normalize(s);
        assert_eq!(
            s.chrome_mode,
            Some(supported(ChromeMode::Native, native_supported()))
        );
        assert_eq!(s.decorations, None, "the legacy key must be cleared");
    }

    #[test]
    fn a_legacy_decorations_false_becomes_header() {
        let s: Settings = serde_json::from_str(r#"{"decorations":false}"#).unwrap();
        assert_eq!(normalize(s).chrome_mode, Some(ChromeMode::Header));
    }

    #[test]
    fn a_file_with_neither_key_takes_the_default_mode() {
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(
            normalize(s).chrome_mode,
            Some(supported(ChromeMode::default(), native_supported()))
        );
    }

    /// Both keys present is what a downgrade-then-upgrade produces. The new
    /// one wins, or the user's last choice would be overwritten by a value
    /// this version stopped maintaining.
    #[test]
    fn the_new_key_wins_over_the_legacy_one() {
        let s: Settings =
            serde_json::from_str(r#"{"decorations":true,"chrome_mode":"content"}"#).unwrap();
        assert_eq!(normalize(s).chrome_mode, Some(ChromeMode::Content));
    }

    /// The migration is self-completing: the legacy key stops being written
    /// the first time anything saves, so it cannot come back and re-decide the
    /// mode on a later launch.
    #[test]
    fn the_legacy_key_is_gone_from_the_file_after_a_load_and_save() {
        let path = scratch("legacy-drop").join("settings.json");
        std::fs::write(&path, br#"{"decorations":false,"always_on_top":true}"#).expect("write");

        let (loaded, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read back");
        assert!(!raw.contains("decorations"), "got {raw}");
        assert!(raw.contains("\"chrome_mode\": \"header\""), "got {raw}");
        assert!(
            raw.contains("\"always_on_top\": true"),
            "unrelated key lost"
        );
    }

    #[test]
    fn chrome_mode_round_trips_through_the_file() {
        let path = scratch("chrome-round-trip").join("settings.json");
        let written = Settings {
            chrome_mode: Some(ChromeMode::Content),
            ..Default::default()
        };
        save(&path, &written).expect("save");
        let (read_back, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert_eq!(read_back.chrome_mode, Some(ChromeMode::Content));
    }

    /// An unknown mode name is a parse failure, which `load` already turns into
    /// defaults plus a visible warning. Pinned so that fallback is deliberate
    /// rather than something a future serde attribute silently changes.
    #[test]
    fn an_unknown_mode_name_falls_back_to_defaults_with_a_warning() {
        let path = scratch("bad-mode").join("settings.json");
        std::fs::write(&path, br#"{"chrome_mode":"borderless"}"#).expect("write");

        let (s, warning) = load(&path);
        assert_eq!(
            s.chrome_mode,
            Some(supported(ChromeMode::default(), native_supported()))
        );
        assert!(warning.expect("a reason").contains("could not be read"));
    }

    /// `load` must normalize on every path, including the ones that never see
    /// a parsed file — otherwise a missing or corrupt file leaves `chrome_mode`
    /// unset and every reader downstream needs its own fallback.
    #[test]
    fn a_missing_file_still_arrives_with_a_mode_set() {
        let path = scratch("missing-mode").join("settings.json");
        let (s, warning) = load(&path);
        assert!(warning.is_none(), "got {warning:?}");
        assert!(s.chrome_mode.is_some());
    }
}
