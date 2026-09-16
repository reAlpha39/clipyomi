// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written independently for the Engine v2 measurement harness. Nothing here
// derives from Translation Aggregator.

//! `jparser-cli bench` end to end against the hand-written mini fixture.
//!
//! Structure, never timings: the milliseconds are a property of whatever
//! machine runs this, so asserting them would make the test a flake detector
//! for the runner rather than a check on the command. What is asserted is
//! everything that is *not* machine-dependent — that both lengths were
//! measured, that the paste reached the app's own 10,000-character ceiling,
//! that the scaling cell carries its reference points, and that `--json`
//! carries the same numbers the human form does. `bench.rs`'s own unit tests
//! cover the arithmetic and the exact format.
//!
//! `--runs 1` and `--readings 1` keep it cheap. The mini fixture holds two
//! entries, so a 10,000-character paste is mostly unmatched runs — which
//! exercises the DP's fallback path at full length rather than the lexicon,
//! and is the fastest honest way to run the command end to end.

use std::path::{Path, PathBuf};
use std::process::Command;

use jparser::engine::build::build_from_reader;

const BIN: &str = env!("CARGO_BIN_EXE_jparser-cli");
const FIXTURE: &str = include_str!("fixtures/jmdict_mini.xml");

/// The paste's two lengths, mirroring `bench.rs`'s own `SHORT` and `LONG`.
/// Whole sentences only, so each lands at or just above its target.
const SHORT: usize = 1_000;
const LONG: usize = 10_000;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn index_dir(dir: &Path) -> PathBuf {
    let index = dir.join("index");
    std::fs::create_dir_all(&index).unwrap();
    build_from_reader(std::io::Cursor::new(FIXTURE), &index).expect("the mini fixture must build");
    index
}

/// A one-sentence gold corpus. `bench` reads only `text`, since its subject
/// is a paste built by cycling the corpus rather than the adjudication.
fn gold_file(dir: &Path) -> PathBuf {
    let gold = dir.join("gold.jsonl");
    std::fs::write(
        &gold,
        concat!(
            r#"{"text":"言ったは言った","source":"t:1","spans":"#,
            r#"[{"start":0,"len":3,"ent_seq":1000010},"#,
            r#"{"start":3,"len":1,"ent_seq":1000020},"#,
            r#"{"start":4,"len":3,"ent_seq":1000010}]}"#,
            "\n",
        ),
    )
    .unwrap();
    gold
}

fn bench(index: &Path, gold: &Path, extra: &[&str]) -> String {
    let out = Command::new(BIN)
        .arg("bench")
        .arg(index)
        .arg(gold)
        .args(["--runs", "1", "--readings", "1"])
        .args(extra)
        .output()
        .expect("jparser-cli must be runnable");
    assert!(
        out.status.success(),
        "exit {:?}, stderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("output must be UTF-8")
}

/// The columns after the two length columns are milliseconds, so this pulls
/// out only what the machine cannot change.
fn char_counts(out: &str) -> Vec<usize> {
    out.lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter_map(|first| first.parse::<usize>().ok())
        .collect()
}

#[test]
fn bench_measures_both_lengths_and_reaches_the_apps_own_ceiling() {
    let dir = scratch("cli-bench");
    let index = index_dir(&dir);
    let gold = gold_file(&dir);
    let out = bench(&index, &gold, &[]);

    let chars = char_counts(&out);
    assert_eq!(chars.len(), 2, "two points, one per length:\n{out}");
    assert!(
        chars[0] >= SHORT && chars[0] < SHORT + 7,
        "the short point overshoots its target by at most one sentence: {chars:?}"
    );
    assert!(
        chars[1] >= LONG && chars[1] < LONG + 7,
        "the long point must actually reach the 10,000-character ceiling: {chars:?}"
    );

    // Cheap to assert and the whole point of the command: a bare multiple is
    // unreadable without the two references beside it.
    assert!(
        out.contains("(linear 1.00x, quadratic 9.9"),
        "the scaling cell must carry both reference points:\n{out}"
    );
    // Hints were not passed, and the flag is recorded rather than inferred so
    // that two baselines are never silently compared across hint states.
    assert!(out.contains("hints:            off"), "{out}");
    assert!(out.contains("readings:         1"), "{out}");
    assert!(out.contains("runs:             1"), "{out}");
    assert!(out.contains("index:            index"), "{out}");
}

#[test]
fn the_json_form_carries_the_same_numbers_as_the_human_one() {
    let dir = scratch("cli-bench-json");
    let index = index_dir(&dir);
    let gold = gold_file(&dir);
    let out = bench(&index, &gold, &["--json"]);

    let v: serde_json::Value = serde_json::from_str(&out).expect("--json must emit one object");
    assert_eq!(v["hints"], false);
    assert_eq!(v["readings"], 1);
    assert_eq!(v["runs"], 1);
    assert_eq!(v["index_generation"], "index");
    let points = v["points"].as_array().expect("points");
    assert_eq!(points.len(), 2);
    assert!(points[0]["chars"].as_u64().unwrap() >= SHORT as u64);
    assert!(points[1]["chars"].as_u64().unwrap() >= LONG as u64);
    // Present even when it is 0: a 1,000-character parse can land under the
    // clock's resolution on a fast machine, and the field saying so is what
    // stops a missing number reading as "no regression".
    assert!(v["scaling_centi"].is_u64(), "scaling_centi must be present");
}
