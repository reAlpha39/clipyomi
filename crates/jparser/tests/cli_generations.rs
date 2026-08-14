// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! `jparser-cli` generation subcommands, end to end through a real process.

use std::path::{Path, PathBuf};
use std::process::Command;

use jmdict_source::SOURCE_FILE;

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

/// `--keep 0` would make `sweep` delete the generation it just published.
/// The library boundary (`ensure_dictionary`, `mod.rs`) clamps this to 1
/// instead of rejecting it, because `sweep` itself may legitimately be
/// called with `keep = 0` and Phase 2B calls `ensure_dictionary` directly
/// without going through this CLI. The CLI still rejects it here rather than
/// clamping: an interactive operator is better served by a clear usage error
/// than by a silent clamp they might never notice.
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

/// The same guard on the other subcommand. The two `#[arg]` blocks are
/// separate copies, so each needs its own test or one can silently lose the
/// validation.
#[test]
fn ensure_dictionary_rejects_a_zero_keep() {
    let dir = scratch("cli-keep-zero-ensure");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["ensure-dictionary", "dict", "mini.xml", "--keep", "0"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "--keep 0 must be rejected");
    assert!(
        !dir.join("dict").exists(),
        "a rejected run must not have built anything"
    );
}

/// An absent root is a usage error for `gen-list`, not an empty listing —
/// `latest` and `sweep` may treat it as "no dictionary yet", but this command
/// exists to tell a human what is actually on disk.
#[test]
fn gen_list_reports_an_absent_root() {
    let dir = scratch("cli-gen-list-absent");

    let out = Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["gen-list", "no-such-root"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "an absent root must not exit 0");
    assert!(
        out.stdout.is_empty(),
        "nothing should be listed: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `gen-list`'s `--help` says "newest first". Lexicographic order would print
/// `gen-9` before `gen-10`; this pins the numeric order the help text
/// promises, past the point where the two orders diverge.
#[test]
fn gen_list_orders_numerically_past_nine_generations() {
    let dir = scratch("cli-gen-list-order");
    let root = dir.join("dict");
    for n in 1..=10 {
        std::fs::create_dir_all(root.join(format!("gen-{n}"))).expect("mkdir");
    }

    let out = cli(&["gen-list", "dict"], &dir);
    let names: Vec<&str> = out
        .lines()
        .map(|line| line.split_whitespace().next().expect("name"))
        .collect();
    let expected: Vec<String> = (1..=10).rev().map(|n| format!("gen-{n}")).collect();
    assert_eq!(names, expected, "got: {out}");
}

/// `gen-remove` is the only thing that can act on an unopenable *newest*
/// generation, since `sweep` never touches it. It must remove exactly the
/// named directory and nothing else.
#[test]
fn gen_remove_deletes_exactly_the_named_generation() {
    let dir = scratch("cli-gen-remove");
    let root = dir.join("dict");
    for name in ["gen-1", "gen-2"] {
        std::fs::create_dir_all(root.join(name)).expect("mkdir");
    }

    let out = cli(&["gen-remove", "dict", "2"], &dir);
    assert!(out.contains("removed"), "got: {out}");
    assert!(!root.join("gen-2").exists());
    assert!(
        root.join("gen-1").exists(),
        "only the named generation must be removed"
    );
}

/// Removing an absent generation is not a usage error — the operator may not
/// know whether it was already cleaned up, and `gen-remove` must not fail
/// merely because there was nothing to do.
#[test]
fn gen_remove_of_an_absent_generation_is_not_an_error() {
    let dir = scratch("cli-gen-remove-absent");
    let root = dir.join("dict");
    std::fs::create_dir_all(root.join("gen-1")).expect("mkdir");

    let out = cli(&["gen-remove", "dict", "9"], &dir);
    assert!(out.contains("absent"), "got: {out}");
    assert!(root.join("gen-1").exists());
}

/// The xml positional must keep working — 2A's other tests pass it that way,
/// and breaking it would make this phase edit tests about generations.
#[test]
fn ensure_dictionary_still_accepts_a_positional_xml() {
    let dir = scratch("cli-positional");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = cli(&["ensure-dictionary", "dict", "mini.xml"], &dir);

    assert!(out.contains("generation: gen-1"), "got: {out}");
}

/// The ArgGroup is what makes "exactly one source" true rather than intended.
/// Asserts on stderr content, not just a non-zero exit: a bare exit-code
/// check would also pass against the pre-wiring binary, which rejected
/// `--source-dir` for the wrong reason (`unexpected argument`, clap not
/// yet knowing the flag). The `ArgGroup` conflict message is what proves
/// the mechanism is actually the group, not a missing flag.
#[test]
fn ensure_dictionary_rejects_both_sources_at_once() {
    let dir = scratch("cli-bothsources");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args([
            "ensure-dictionary",
            "dict",
            "mini.xml",
            "--source-dir",
            "src",
        ])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "both sources must be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("cannot be used with"),
        "expected an ArgGroup conflict, got: {stderr}"
    );
    assert!(
        stderr.contains("source-dir"),
        "the conflict must name --source-dir, got: {stderr}"
    );
}

/// Same reasoning as the test above: the pre-wiring binary already rejected
/// this (missing required `xml` positional), so only the message proves the
/// `ArgGroup`, not just the old positional, is what is enforcing it now.
#[test]
fn ensure_dictionary_rejects_no_source_at_all() {
    let dir = scratch("cli-nosource");

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["ensure-dictionary", "dict"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(!out.status.success(), "a missing source must be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("required arguments were not provided"),
        "got: {stderr}"
    );
    assert!(
        stderr.contains("source-dir"),
        "the required-source message must name --source-dir, got: {stderr}"
    );
}

/// The headline capability of this phase: `ensure_dictionary` actually built
/// through `jmdict_source::resolve`, not merely that clap accepted the flag.
/// Both rejection tests above stop at the `ArgGroup` before `main`'s `match`
/// ever runs, and `jmdict-source`'s own `seam.rs` calls `resolve` and
/// `ensure_dictionary` directly as library functions, bypassing clap
/// entirely — so nothing else in the suite drives this arm end to end.
///
/// The fixture is placed uncompressed: `jparser` has no `flate2`
/// dev-dependency, and `open_local`'s byte-sniff already passes a non-gzip
/// file straight through (covered by `jmdict-source`'s own tests and by
/// `seam.rs`'s gzip case), so a plain-XML archive still exercises the full
/// `--source-dir` route without adding a dependency.
#[test]
fn ensure_dictionary_builds_through_the_source_dir_route() {
    let dir = scratch("cli-source-dir");
    let source_dir = dir.join("src");
    std::fs::create_dir_all(&source_dir).expect("mkdir");
    std::fs::write(source_dir.join(SOURCE_FILE), XML).expect("write archive");

    let out = cli(&["ensure-dictionary", "dict", "--source-dir", "src"], &dir);

    assert!(out.contains("generation: gen-1"), "got: {out}");
}

/// The flag must be rejected when the dictionary is missing, rather than
/// silently parsing without hints. Gated on `mecab`: `--hints` does not exist
/// in the default build (see the CLI's `Parse` variant), so this only runs
/// under `cargo test -p jparser --features mecab`.
#[cfg(feature = "mecab")]
#[test]
fn parse_rejects_an_absent_hints_dictionary() {
    let dir = scratch("cli-hints-absent");
    std::fs::write(dir.join("mini.xml"), XML).expect("write xml");
    cli(&["build-index", "mini.xml", "idx"], &dir);

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_jparser-cli"))
        .args(["parse", "idx", "東京", "--hints", "nope.dic"])
        .current_dir(&dir)
        .output()
        .expect("run jparser-cli");

    assert!(
        !out.status.success(),
        "a missing dictionary must be rejected"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("nope.dic"), "wrong failure: {stderr}");
}
