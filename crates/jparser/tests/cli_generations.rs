// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `jparser-cli` generation subcommands, end to end through a real process.

use std::path::{Path, PathBuf};
use std::process::Command;

const XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// `&Path`, never `&PathBuf` — the latter trips `clippy::ptr_arg`, which is a
/// hard error under this crate's `--all-targets -D warnings` gate.
fn cli(args: &[&str], cwd: &Path) -> String {
    let exe = env!("CARGO_BIN_EXE_jparser-cli");
    let out = Command::new(exe)
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run jparser-cli");
    assert!(
        out.status.success(),
        "cli failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

#[test]
fn ensure_dictionary_builds_once_and_then_reuses() {
    let dir = scratch("cli-gen");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    let root = dir.join("dict");

    let first = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);
    assert!(first.contains("generation: gen-1"), "got: {first}");
    assert!(first.contains("entries:    1"), "got: {first}");

    let second = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);
    assert!(second.contains("generation: gen-1"), "rebuilt: {second}");

    let listed = cli(&["gen-list", "dict"], &dir);
    assert_eq!(listed.lines().count(), 1, "got: {listed}");
    assert!(listed.starts_with("gen-1 ok entries=1"), "got: {listed}");

    assert!(root.join("gen-1").exists());
    assert!(!root.join("gen-2").exists());
}

#[test]
fn gen_sweep_reports_what_it_removed() {
    let dir = scratch("cli-sweep");
    let root = dir.join("dict");
    for name in ["gen-1", "gen-2", "gen-3", ".build-1-1"] {
        std::fs::create_dir_all(root.join(name)).expect("mkdir");
    }

    let out = cli(&["gen-sweep", "dict", "--keep", "1"], &dir);
    assert_eq!(out.trim(), "removed: 3");
    assert!(root.join("gen-3").exists());
    assert!(!root.join("gen-1").exists());
    assert!(!root.join(".build-1-1").exists());
}

/// `--keep 0` would make `sweep` delete the generation it just published, so it
/// is rejected at the boundary rather than clamped.
#[test]
fn gen_sweep_rejects_a_zero_keep() {
    let dir = scratch("cli-keep-zero");
    std::fs::create_dir_all(dir.join("dict").join("gen-1")).expect("mkdir");

    let out = Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["gen-sweep", "dict", "--keep", "0"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "--keep 0 must be rejected");
    assert!(
        dir.join("dict").join("gen-1").exists(),
        "a rejected sweep must not have deleted anything"
    );
}
