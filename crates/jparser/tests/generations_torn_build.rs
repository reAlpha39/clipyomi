// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! The generation layout's reason to exist, as executable evidence.
//!
//! Phase 1A established that a rebuild interrupted *inside a live index
//! directory* can leave `Index::open` succeeding against a spliced set of
//! files and returning well-formed wrong answers. These tests pin both halves:
//! that the hazard is real when a build writes directly into a served
//! directory, and that publishing through `gen-N` removes it.

use std::path::{Path, PathBuf};

use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::generations::{build_new, latest, BUILD_PREFIX};
use jparser::index::load::Index;
use jparser::index::{ENTRIES_FILE, ENTRIES_INDEX_FILE, FST_FILE, HEADER_FILE, RECORDS_FILE};
use jparser::stem::StemOptions;

/// One dictionary.
const XML_A: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

/// A different dictionary: different ids, different surfaces, different sizes.
const XML_B: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>2000010</ent_seq><k_ele><keb>山</keb></k_ele>",
    "<r_ele><reb>やま</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>mountain</gloss></sense></entry>",
    "<entry><ent_seq>2000020</ent_seq><k_ele><keb>川</keb></k_ele>",
    "<r_ele><reb>かわ</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>river</gloss></sense></entry>",
    "<entry><ent_seq>2000030</ent_seq><k_ele><keb>海</keb></k_ele>",
    "<r_ele><reb>うみ</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>sea</gloss></sense></entry>",
    "</JMdict>",
);

const FILES: [&str; 5] = [
    FST_FILE,
    RECORDS_FILE,
    ENTRIES_FILE,
    ENTRIES_INDEX_FILE,
    HEADER_FILE,
];

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("jparser-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn table_and_opts() -> (ConjugationTable, StemOptions) {
    (
        ConjugationTable::load_embedded().expect("table"),
        StemOptions::default(),
    )
}

/// Build `xml` directly into `dir` — the pre-generations behaviour.
fn build_directly(dir: &Path, xml: &str) {
    let (table, opts) = table_and_opts();
    build_from_reader(xml.as_bytes(), &table, &opts, dir).expect("build");
}

/// Delete a file, or truncate it to half its length.
fn damage(dir: &Path, file: &str, absent: bool) {
    let path = dir.join(file);
    if absent {
        std::fs::remove_file(&path).expect("remove");
    } else {
        let len = std::fs::metadata(&path).expect("metadata").len();
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open for truncate");
        f.set_len(len / 2).expect("truncate");
    }
}

/// The hazard, reproduced. A rebuild interrupted inside a directory that is
/// already being served leaves `Index::open` reading a mix of two builds.
///
/// This test asserts the BAD outcome deliberately: it is the baseline the
/// generation layout is measured against. If a future change makes
/// `Index::open` validate its payload, this test starts failing for a good
/// reason — delete it and say so in the commit. Do not "fix" it.
#[test]
fn a_rebuild_into_a_live_directory_can_serve_data_from_neither_build() {
    let dir = scratch("torn-hazard");
    build_directly(&dir, XML_A);

    // Interrupt a rebuild after two of the five files have been replaced.
    let staging = scratch("torn-hazard-staging");
    build_directly(&staging, XML_B);
    for file in [FST_FILE, RECORDS_FILE] {
        std::fs::copy(staging.join(file), dir.join(file)).expect("copy");
    }

    // `header.bin` is still A's, so version and fingerprint both validate and
    // `open` succeeds. What it returns is a splice of two dictionaries.
    let index = Index::open(&dir).expect("open succeeds — that is the hazard");
    let a_present = index.entry(1000010).expect("entry").is_some();
    let b_present = index.entry(2000010).expect("entry").is_some();
    assert!(
        !(a_present && b_present),
        "a spliced index cannot coherently hold both dictionaries"
    );

    // `prefixes_of` is the actually-spliced read path: it walks B's `keys.fst`,
    // decodes B's `records.bin`, then resolves the resulting `entry_id`
    // against A's `entries.idx`. The `entry()` checks above cannot see the
    // splice at all, because `entry()` reads only A's two payload files.
    let hits = index.prefixes_of("山").expect("prefixes_of");
    assert!(!hits.is_empty(), "B's keys.fst was not actually consulted");
    for hit in &hits {
        for record in &hit.records {
            assert!(
                index.entry(record.entry_id).expect("entry").is_none(),
                "a key from build B resolved into build A's payload"
            );
        }
    }
}

/// Copy a completed build into an orphaned `.build-*` directory, damaged at
/// `state`, and leave it in `root` the way a killed process would. Returns the
/// orphan's path so the caller never has to reconstruct the name itself.
fn strand_interrupted_build(root: &Path, state: usize) -> PathBuf {
    let staging = scratch(&format!("torn-staging-{state}"));
    build_directly(&staging, XML_B);

    let orphan = root.join(format!("{BUILD_PREFIX}9999-{state}"));
    std::fs::create_dir_all(&orphan).expect("orphan dir");
    for file in FILES {
        std::fs::copy(staging.join(file), orphan.join(file)).expect("copy");
    }

    // States 0..=9 damage one file: state/2 selects it, state%2 selects
    // absent-vs-truncated. State 10 leaves the build whole but unpublished,
    // which is the "died just before the rename" case.
    if state < 10 {
        damage(&orphan, FILES[state / 2], state % 2 == 0);
    }
    orphan
}

/// No assertion below depends on *which* of the eleven states ran — and that
/// invariance is the property under test, not a gap in it. An interrupted
/// build is excluded by its `.build-` directory *name* before any byte of its
/// contents is ever read (`generations::latest_number` filters on
/// `generation_number(name)` alone), so it does not matter whether the orphan
/// is missing a file, truncated, or — state 10 — a complete, undamaged build
/// of a different dictionary: the outcome is identical either way. State 10
/// is the sharpest witness to that: it shows the guarantee is not "corrupt
/// builds get rejected" but "an unpublished build is never served, corrupt or
/// not," which is asserted directly below via state 10's own orphan.
#[test]
fn no_interrupted_build_is_ever_served() {
    let (table, opts) = table_and_opts();

    for state in 0..11 {
        let root = scratch(&format!("torn-state-{state}"));

        // A good generation is already published and being served.
        let (good, _) = build_new(&root, XML_A.as_bytes(), &table, &opts).expect("build_new");
        assert_eq!(good, root.join("gen-1"));

        let orphan = strand_interrupted_build(&root, state);

        // 1. The interrupted build is never what a reader resolves.
        let resolved = latest(&root).expect("latest").expect("a generation");
        assert_eq!(resolved, good, "state {state}: latest resolved the orphan");

        // 2. No `.build-*` path is ever returned.
        let name = resolved
            .file_name()
            .expect("name")
            .to_string_lossy()
            .into_owned();
        assert!(
            !name.starts_with(BUILD_PREFIX),
            "state {state}: latest returned a build directory"
        );

        // 3. The resolved generation opens and is internally coherent. This is
        //    the assertion that would have caught the original hazard.
        let index = Index::open(&resolved).expect("open");
        let entry = index.entry(1000010).expect("entry").expect("present");
        assert_eq!(entry.id, 1000010, "state {state}: cross-generation splice");
        assert!(
            index.entry(2000010).expect("entry").is_none(),
            "state {state}: data from the interrupted build leaked in"
        );

        // State 10's orphan is a WHOLE, undamaged build of XML_B — the
        // "died just before the rename" case. Assert that directly, so the
        // matrix carries at least one state-sensitive claim: what keeps an
        // interrupted build from being served is its *name*, not any defect
        // in its contents. Checked here, before assertion 4's rebuild moves
        // `latest` on to `gen-2`, because the claim is that `good` (`gen-1`)
        // is still what resolves even while state 10's intact orphan sits
        // right there in `root`.
        if state == 10 {
            let whole = Index::open(&orphan).expect("state 10's orphan is an intact index");
            assert_eq!(
                whole.entry(2000010).expect("entry").expect("present").id,
                2000010,
                "state 10's orphan should be a complete build of XML_B"
            );
            // ...and it is still not what any reader resolves.
            assert_eq!(latest(&root).expect("latest"), Some(good.clone()));
        }

        // 4. A later build still succeeds despite the orphan.
        let (next, _) =
            build_new(&root, XML_A.as_bytes(), &table, &opts).expect("build after orphan");
        assert_eq!(
            next,
            root.join("gen-2"),
            "state {state}: the orphan blocked a rebuild"
        );
    }
}
