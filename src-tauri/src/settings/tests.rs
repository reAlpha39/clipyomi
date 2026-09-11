// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for `settings`. Split out when the parent crossed the 800-line
//! ceiling; `use super::*` keeps every private item reachable exactly as it
//! was when this was an inline `mod tests`.

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

/// Owned by the backend, not the settings window: `merge_saved` is a whitelist
/// that does not carry this key, so the only writer is `set_mecab_hints`. Off
/// by default — a first launch must not download 8 MB nobody asked for.
#[test]
fn mecab_hints_defaults_off_and_survives_a_round_trip() {
    let dir = scratch("mecab-hints-roundtrip");
    let path = dir.join(SETTINGS_FILE);
    assert!(!Settings::default().mecab_hints);

    let settings = Settings {
        mecab_hints: true,
        ..Settings::default()
    };
    save(&path, &settings).expect("save");

    let (reloaded, warning) = load(&path);
    assert!(reloaded.mecab_hints);
    assert!(warning.is_none(), "got {warning:?}");
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
    assert_eq!(s.hide_kanji, None);
    assert_eq!(s.window_width, None);
    assert_eq!(s.window_height, None);
    assert_eq!(s.window_x, None);
    assert_eq!(s.window_y, None);
    assert_eq!(s.surface_alpha, None);
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
        mecab_hints: false,
        chrome_mode: Some(ChromeMode::Header),
        decorations: None,
        furigana_mode: Some("hiragana".to_string()),
        hide_pos: None,
        hide_xrefs: None,
        hide_usage: None,
        hide_kanji: None,
        font_sentence: None,
        font_gloss: None,
        furigana_scale: None,
        surface_alpha: None,
        window_width: Some(400),
        window_height: Some(300),
        window_x: None,
        window_y: None,
        layout_slots: BTreeMap::new(),
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
        mecab_hints: false,
        chrome_mode: Some(ChromeMode::Header),
        decorations: None,
        furigana_mode: Some("hiragana".to_string()),
        hide_pos: Some(true),
        hide_xrefs: Some(false),
        hide_usage: Some(true),
        hide_kanji: Some(false),
        font_sentence: None,
        font_gloss: None,
        furigana_scale: None,
        surface_alpha: None,
        window_width: Some(400),
        window_height: Some(300),
        window_x: None,
        window_y: None,
        layout_slots: BTreeMap::new(),
        extra: serde_json::Map::new(),
    };
    let json = serde_json::to_string(&settings).expect("serialization succeeds");
    assert!(json.contains("\"hide_pos\":true"));
    assert!(json.contains("\"hide_xrefs\":false"));
    assert!(json.contains("\"hide_usage\":true"));
    assert!(json.contains("\"hide_kanji\":false"));
    let deserialized: Settings = serde_json::from_str(&json).expect("deserialization succeeds");
    assert_eq!(deserialized.hide_pos, Some(true));
    assert_eq!(deserialized.hide_xrefs, Some(false));
    assert_eq!(deserialized.hide_usage, Some(true));
    assert_eq!(deserialized.hide_kanji, Some(false));
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

/// `Option` for the same reason the font sizes are: "never set" has to
/// stay distinguishable from a value that happens to equal the stylesheet
/// default, which is what keeps `tokens.css` the one place a default lives.
#[test]
fn surface_alpha_round_trips_through_the_file() {
    let path = scratch("surface-alpha").join("settings.json");
    let written = Settings {
        surface_alpha: Some(62),
        ..Default::default()
    };

    save(&path, &written).expect("save");
    let (read_back, warning) = load(&path);
    assert_eq!(read_back.surface_alpha, Some(62));
    assert!(warning.is_none(), "got {warning:?}");
}

#[test]
fn an_unset_surface_alpha_is_left_out_of_the_file() {
    let path = scratch("surface-alpha-unset").join("settings.json");
    save(&path, &Settings::default()).expect("save");

    let raw = std::fs::read_to_string(&path).expect("read back");
    assert!(!raw.contains("surface_alpha"), "got {raw}");
}

/// Zero is a real, storable setting — not a missing value. Per-pixel alpha
/// leaves the text opaque, so a 0 % background is still a usable window.
#[test]
fn a_zero_surface_alpha_survives_the_file() {
    let path = scratch("surface-alpha-zero").join("settings.json");
    let written = Settings {
        surface_alpha: Some(0),
        ..Default::default()
    };

    save(&path, &written).expect("save");
    assert_eq!(load(&path).0.surface_alpha, Some(0));
}

/// A slot fixture with every optional key set, so a round trip that drops
/// one is visible.
fn slot_fixture() -> LayoutSlot {
    LayoutSlot {
        window_width: 720,
        window_height: 200,
        window_x: 1200,
        window_y: 40,
        chrome_mode: ChromeMode::Content,
        always_on_top: true,
        surface_alpha: Some(35),
        font_sentence: Some(30),
        font_gloss: Some(18),
        furigana_scale: Some(70),
        furigana_mode: Some("hiragana".to_string()),
        extra: serde_json::Map::new(),
    }
}

#[test]
fn a_layout_slot_round_trips_through_the_file() {
    let path = scratch("layout-slot").join("settings.json");
    let mut written = Settings::default();
    written.layout_slots.insert("3".to_string(), slot_fixture());

    save(&path, &written).expect("save");
    let (read_back, warning) = load(&path);
    assert!(warning.is_none(), "got {warning:?}");
    assert_eq!(read_back.layout_slots.get("3"), Some(&slot_fixture()));
}

/// The same reason the font sizes are `Option`: a slot bound while the
/// boxes were empty has to restore them empty, so "unset" must survive the
/// file as an absent key rather than as a number.
#[test]
fn a_slots_unset_keys_are_left_out_of_the_file() {
    let path = scratch("layout-slot-unset").join("settings.json");
    let mut written = Settings::default();
    written.layout_slots.insert(
        "1".to_string(),
        LayoutSlot {
            surface_alpha: None,
            font_sentence: None,
            font_gloss: None,
            furigana_scale: None,
            furigana_mode: None,
            ..slot_fixture()
        },
    );

    save(&path, &written).expect("save");
    let raw = std::fs::read_to_string(&path).expect("read back");
    assert!(!raw.contains("surface_alpha"), "got {raw}");
    assert!(!raw.contains("font_sentence"), "got {raw}");
    assert_eq!(load(&path).0.layout_slots["1"].surface_alpha, None);
}

#[test]
fn no_slots_means_no_key_in_the_file() {
    let path = scratch("layout-slot-empty").join("settings.json");
    save(&path, &Settings::default()).expect("save");

    let raw = std::fs::read_to_string(&path).expect("read back");
    assert!(!raw.contains("layout_slots"), "got {raw}");
}

/// The reason the key is a `String` and not a `u8`. `load` turns any parse
/// failure into the whole file reset to defaults — so with a stricter key
/// type, one hand-typed slot name would also cost the user their fonts,
/// geometry, alpha and chrome mode. It costs them nothing instead.
#[test]
fn a_garbage_slot_key_is_ignored_without_resetting_the_file() {
    let path = scratch("layout-slot-garbage").join("settings.json");
    std::fs::write(
        &path,
        br#"{"always_on_top":true,"clipboard_monitoring":true,
             "layout_slots":{"1a":{"window_width":720,"window_height":200,
             "window_x":0,"window_y":0,"chrome_mode":"content",
             "always_on_top":false}}}"#,
    )
    .expect("write");

    let (loaded, warning) = load(&path);
    assert!(
        warning.is_none(),
        "the file parsed, so nothing to warn about"
    );
    assert!(loaded.always_on_top, "an unrelated key was reset");
    assert!(
        loaded.layout_slots.contains_key("1a"),
        "the key was dropped"
    );
    assert!(!loaded.layout_slots.contains_key("1"));
}

/// A slot key added by a later version must survive a rewrite by this one,
/// exactly as `unknown_keys_survive_a_rewrite` requires of the top level.
#[test]
fn an_unknown_key_inside_a_slot_survives_a_rewrite() {
    let path = scratch("layout-slot-extra").join("settings.json");
    std::fs::write(
        &path,
        br#"{"always_on_top":false,"clipboard_monitoring":true,
             "layout_slots":{"2":{"window_width":720,"window_height":200,
             "window_x":0,"window_y":0,"chrome_mode":"header",
             "always_on_top":false,"theme":"dark"}}}"#,
    )
    .expect("write");

    let (loaded, _) = load(&path);
    save(&path, &loaded).expect("save");

    let raw = std::fs::read_to_string(&path).expect("read back");
    assert!(raw.contains("\"theme\""), "got {raw}");
}
