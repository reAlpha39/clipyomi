// SPDX-License-Identifier: MIT OR Apache-2.0

//! `fetch-hints` reaches the crate that does the work.
//!
//! The download itself is covered in `vibrato-source`'s own suite against a
//! local listener; what is unproven here is the wiring — that the subcommand
//! exists under its feature, resolves the directory it is given, and reports a
//! failure rather than panicking. So this drives the binary against an
//! already-published dictionary, which needs no network at all.

use std::path::Path;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-cli-hints-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn fetch_hints_reports_an_already_published_dictionary_without_downloading() {
    let dir = scratch("published");
    let published = dir.join(vibrato_source::DICT_DIR);
    std::fs::create_dir_all(&published).expect("dict dir");
    std::fs::write(published.join(vibrato_source::SYSTEM_DIC), b"already here").expect("write");

    let out = cli()
        .args(["fetch-hints", dir.to_str().expect("utf-8 path")])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("system.dic"),
        "the published path must be printed, got {stdout}"
    );
    assert!(
        Path::new(stdout.trim()).is_file(),
        "what is printed must be a real path, got {stdout}"
    );
}
