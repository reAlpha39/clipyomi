// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.

//! Phase 1b-i's gate: every row of `fixtures/conjugation_labels.txt` is run
//! through the real deinflection trie, its chain must be found exactly once,
//! and `label::fmt` must produce the row's expected English. Design §4.2.

use jparser::engine::label;
use jparser::engine::table::{ChainHit, Table, CLASSES};
use jparser::engine::ConjStep;

const FIXTURE: &str = include_str!("fixtures/conjugation_labels.txt");

/// One parsed fixture row.
struct Row {
    line_no: usize,
    surface: String,
    chain: String,
    expected: Option<String>,
}

/// The fixture's rows, `#` comments and blank lines skipped. Panics on a
/// malformed row so a stray space instead of a tab is loud, not silent.
fn rows() -> Vec<Row> {
    FIXTURE
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.starts_with('#') && !l.trim().is_empty())
        .map(|(i, l)| {
            let mut cols = l.split('\t');
            let surface = cols.next().expect("surface").to_string();
            let chain = cols
                .next()
                .unwrap_or_else(|| panic!("line {}: no chain column (tab-separated?)", i + 1))
                .to_string();
            let expected = cols
                .next()
                .unwrap_or_else(|| panic!("line {}: no expected column", i + 1));
            assert!(cols.next().is_none(), "line {}: more than three columns", i + 1);
            Row {
                line_no: i + 1,
                surface,
                chain,
                expected: (expected != "-").then(|| expected.to_string()),
            }
        })
        .collect()
}

/// A chain in the fixture's notation: `class:form` for the first step,
/// `lemma:form` for each auxiliary.
fn render(steps: &[ConjStep]) -> String {
    steps
        .iter()
        .map(|s| {
            let class = &CLASSES[s.class as usize];
            let form = class.forms[s.form as usize].name;
            match s.aux {
                None => format!("{}:{form}", class.name),
                Some(lemma) => format!("{lemma}:{form}"),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Every chain the table finds over the whole surface, with the budget set
/// to the surface's own length so nothing depends on `lex.rs`'s MAX_CHAIN.
fn chains_over(table: &Table, surface: &str) -> Vec<ChainHit> {
    let chars: Vec<char> = surface.chars().collect();
    table.chains_ending_at(&chars, chars.len(), chars.len())
}

#[test]
fn the_fixture_parses_and_is_not_empty() {
    let rows = rows();
    assert!(
        rows.len() >= 221,
        "the grid lost rows: {} (the fixture is the phase's authority; rows may be added, never quietly dropped)",
        rows.len()
    );
    for r in &rows {
        assert!(r.chain.contains(':'), "line {}: chain has no class:form step", r.line_no);
    }
}

#[test]
fn every_fixture_chain_is_generated_exactly_once() {
    let table = Table::new();
    let mut wrong = Vec::new();
    for r in rows() {
        let hits = chains_over(&table, &r.surface);
        let matching = hits.iter().filter(|h| render(&h.steps) == r.chain).count();
        if matching != 1 {
            let found: Vec<String> = hits.iter().map(|h| render(&h.steps)).collect();
            wrong.push(format!(
                "line {}: {} — chain `{}` found {matching} times; table generates {found:?}",
                r.line_no, r.surface, r.chain
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "fixture chains the table does not generate exactly once:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn every_fixture_row_labels_as_expected() {
    let table = Table::new();
    let mut wrong = Vec::new();
    for r in rows() {
        let hits = chains_over(&table, &r.surface);
        let Some(hit) = hits.iter().find(|h| render(&h.steps) == r.chain) else {
            // The test above reports this case with more detail.
            continue;
        };
        let got = label::fmt(&hit.steps);
        if got != r.expected {
            wrong.push(format!(
                "line {}: {} [{}] expected {:?}, got {got:?}",
                r.line_no, r.surface, r.chain, r.expected
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} rows mislabelled:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
