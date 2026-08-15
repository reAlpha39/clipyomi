// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Persisted user settings.
//!
//! Two keys today, both toggles the header owns. Unknown keys survive a rewrite
//! so a file written by a later version is not silently truncated by this one.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Filename inside the app config dir. A sibling of `dict/`, never inside it —
/// a published generation directory is immutable.
pub const SETTINGS_FILE: &str = "settings.json";

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_true")]
    pub clipboard_monitoring: bool,
    #[serde(default = "default_true")]
    pub decorations: bool,
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
            decorations: true,
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
        Err(_) => return (Settings::default(), None),
    };

    match serde_json::from_str(&raw) {
        Ok(settings) => (settings, None),
        Err(e) => (
            Settings::default(),
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

    /// First run is not an error state.
    #[test]
    fn a_missing_file_loads_defaults_without_a_warning() {
        let path = scratch("missing").join("settings.json");
        let (s, warning) = load(&path);
        assert_eq!(s, Settings::default());
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
        assert_eq!(read_back, written);
        assert!(warning.is_none(), "got {warning:?}");
    }

    /// A corrupt file must not stop the app launching — but the user has to be
    /// told, or their settings appear to silently reset themselves.
    #[test]
    fn a_corrupt_file_loads_defaults_and_reports_why() {
        let path = scratch("corrupt").join("settings.json");
        std::fs::write(&path, b"{ not json").expect("write");

        let (s, warning) = load(&path);
        assert_eq!(s, Settings::default());
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
            br#"{"always_on_top":true,"clipboard_monitoring":true,"furigana_mode":"all","font_size":18}"#,
        )
        .expect("write");

        let (loaded, _) = load(&path);
        save(&path, &loaded).expect("save");

        let raw = std::fs::read_to_string(&path).expect("read");
        let json: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        assert_eq!(json["furigana_mode"], "all", "unknown string key dropped");
        assert_eq!(json["font_size"], 18, "unknown number key dropped");
        assert_eq!(json["always_on_top"], true, "known key lost");
    }

    #[test]
    fn the_settings_path_is_a_sibling_of_the_dict_directory() {
        let root = std::path::Path::new("/tmp/cfg");
        assert_eq!(settings_path(root), root.join("settings.json"));
    }

    #[test]
    fn default_decorations_is_true() {
        let s = Settings::default();
        assert!(s.decorations);
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
        assert!(!s.decorations);
        assert_eq!(s.window_width, Some(500));
        assert_eq!(s.window_height, Some(120));
        assert_eq!(s.window_x, Some(100));
        assert_eq!(s.window_y, Some(200));
    }
}

