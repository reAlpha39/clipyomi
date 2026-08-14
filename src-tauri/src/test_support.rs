// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Test-only fixtures shared by `commands`' and `state`'s test modules.
//!
//! Building a real index — rather than relying only on the `index: None`
//! shortcut — turns out to be cheap: `jparser::index::ensure_dictionary` and
//! `jparser::stem::StemOptions` are both public, so a one-entry fixture is
//! exactly as easy to build here as it is inside `jparser`'s own tests.
//! Only compiled for `cargo test`; nothing here is reachable from `main`.

use std::path::{Path, PathBuf};

use jparser::conjugation::ConjugationTable;
use jparser::index::{ensure_dictionary, generations};
use jparser::stem::StemOptions;

/// A single-entry JMdict fixture — the smallest input `ensure_dictionary`
/// accepts, mirroring `crates/jparser/src/index/mod.rs`'s own test fixture.
const FIXTURE_XML: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    "<JMdict>",
    "<entry><ent_seq>1000010</ent_seq><k_ele><keb>本</keb></k_ele>",
    "<r_ele><reb>ほん</reb></r_ele>",
    "<sense><pos>&n;</pos><gloss>book</gloss></sense></entry>",
    "</JMdict>",
);

/// A scratch directory unique to `name`, freshly emptied.
pub(crate) fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ta-src-tauri-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// Build a real one-entry index generation under `root` and return the
/// generation's own directory (`root/gen-1`).
pub(crate) fn build_index_generation(root: &Path) -> PathBuf {
    let table = ConjugationTable::load_embedded().expect("embedded conjugation table");
    let opts = StemOptions::default();
    drop(
        ensure_dictionary(root, &table, &opts, 1, || Ok(FIXTURE_XML.as_bytes()))
            .expect("build the fixture index"),
    );
    generations::latest(root)
        .expect("read back the generation just built")
        .expect("a generation exists immediately after building one")
}
