// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which words does JMdict know that the hint tokenizer cuts apart?
//!
//! `is_allowed` forbids a span from starting or ending inside a hint token, so
//! wherever IPADIC holds a word whole the engine keeps it whole — that is why
//! `美少女` survives hints-on and shatters hints-off. The converse is this
//! sweep's subject: a word JMdict has and IPADIC splits gets no protection,
//! and the cost model then prefers a tiling of COMMON fragments (see
//! `docs/superpowers/2026-09-16-per-match-cost-spike.md` for why no magnitude
//! fixes that).
//!
//! Read-only. It reports candidates for a Vibrato user lexicon; it writes no
//! dictionary and changes no behaviour.
//!
//! Entries JMdict tags `exp` are excluded: a phrase *should* come apart into
//! its component words, so a split there is not damage.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example shatter_sweep -- \
//!   <index> <system.dic> <JMdict_e.xml> [min-len]
//!
//! The XML is read uncompressed on purpose: `jparser` must not gain a
//! decompressor, which is what CI's `purity` job pins.
//! ```

use std::collections::BTreeSet;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use jparser::engine::table::Table;
use jparser::index::load::Index;
use jparser::{parse, ParseOptions};

fn main() -> Result<(), Box<dyn Error>> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let min_len: usize = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);

    let index = Index::open(Path::new(&a[0]))?;
    let table = Table::new();
    let tok = jparser::hints::VibratoTokenizer::load(Path::new(&a[1]))?;
    let opts = ParseOptions::default();

    // Kana-only headwords: an entry with no kanji form, whose reading is the
    // word itself. Those are the ones the cost model cannot defend, because a
    // kana word's pieces are overwhelmingly COMMON particles and stems.
    let mut words: BTreeSet<String> = BTreeSet::new();
    let mut pos_of: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for entry in jparser::jmdict::parse_entries(BufReader::new(File::open(&a[2])?)) {
        let entry = entry?;
        // Kanji forms are considered too. The first run of this sweep took
        // kana-only headwords, which silently excluded every word like
        // `ある日` -- the same defect with the same arithmetic (`ある日` costs
        // +3 as a non-COMMON headword, `ある` + `日` costs -3 as COMMON
        // pieces) and a far commoner shape.
        // JMdict tags a phrase `exp`. Splitting one into its component words
        // is what a reader wants -- ああでもないこうでもない is six words and
        // reads better as six -- so an expression is not damage and is not a
        // user-lexicon candidate. Only single lexical items are.
        if entry
            .senses
            .iter()
            .any(|sn| sn.pos.iter().any(|p| p == "exp"))
        {
            continue;
        }
        // Kanji forms always; readings **only for kana-only entries**. A
        // reading of a kanji entry is a pronunciation, not a way the word is
        // written -- taking `してい` from 指定 or `どうし` from 同士 as a
        // surface produces exactly handoff §9.5's absurdities, and an earlier
        // pass of this sweep did precisely that.
        let surfaces = entry.kanji.iter().map(|k| &k.text).chain(
            entry
                .readings
                .iter()
                .filter(|_| entry.kanji.is_empty())
                .map(|r| &r.text),
        );
        for w in surfaces {
            if w.chars().count() >= min_len {
                words.insert(w.clone());
                pos_of
                    .entry(w.clone())
                    .or_default()
                    .extend(entry.senses.iter().flat_map(|sn| sn.pos.iter().cloned()));
            }
        }
    }
    eprintln!(
        "kana-only headwords of {min_len}+ characters: {}",
        words.len()
    );

    // Connection ids are lifted from real IPADIC entries rather than invented:
    // Vibrato verifies them against the connection matrix, and a wrong pair
    // would distort its own Viterbi rather than fail loudly. `ゆっくり` is a
    // 副詞, `りんご` a 名詞; tokenizing each gives the ids IPADIC itself uses.
    let ids_for = |probe: &str| -> (u16, u16) {
        let t = tok.ids_of(probe);
        t.unwrap_or((1285, 1285))
    };
    let (adv_l, adv_r) = ids_for("ゆっくり");
    let (noun_l, noun_r) = ids_for("りんご");
    eprintln!("ids: adverb {adv_l}/{adv_r}, noun {noun_l}/{noun_r}");

    let mut split_by_tokenizer = 0usize;
    let mut shattered: Vec<(String, usize, String, usize, bool, bool)> = Vec::new();
    for w in &words {
        let chars: Vec<char> = w.chars().collect();
        let entry_pos = pos_of.get(w).cloned().unwrap_or_default();
        let b = tok.boundaries(w);
        let cuts = (1..chars.len())
            .filter(|&p| jparser::engine::Boundaries::is_boundary(&b, p))
            .count();
        if cuts == 0 {
            continue; // the tokenizer already holds it whole
        }
        split_by_tokenizer += 1;
        let bounds: Option<&dyn jparser::engine::Boundaries> = Some(&b);
        let parsed = parse(&index, &table, w, &opts, bounds)?;
        let whole = parsed.segments.len() == 1 && parsed.segments[0].matched;
        if !whole {
            let got: Vec<&str> = parsed.segments.iter().map(|s| s.surface.as_str()).collect();
            // The shortest piece separates a useful decomposition (three
            // lookupable loanwords) from real damage (a stem and two
            // fragments).
            let min_piece = parsed.segments.iter().map(|s| s.len).min().unwrap_or(0);
            let all_hira = w.chars().all(|c| ('\u{3041}'..='\u{3096}').contains(&c));
            let is_adv = entry_pos.iter().any(|p| p.starts_with("adv"));
            shattered.push((w.clone(), cuts, got.join("|"), min_piece, all_hira, is_adv));
        }
    }

    println!("kana-only headwords considered:        {}", words.len());
    println!("  the tokenizer cuts inside:           {split_by_tokenizer}");
    println!("  and the engine then shatters:        {}", shattered.len());
    for min in 1..=2 {
        let n = shattered.iter().filter(|r| r.3 == min).count();
        let h = shattered.iter().filter(|r| r.3 == min && r.4).count();
        println!("    shortest piece {min}: {n:5}   of which all-hiragana: {h}");
    }
    let big = shattered.iter().filter(|r| r.3 >= 3).count();
    println!("    shortest piece 3+: {big:5}   (a decomposition into real words)");

    let mut bad: Vec<_> = shattered.iter().filter(|r| r.4 && r.3 <= 2).collect();
    bad.sort_by(|x, y| {
        y.0.chars()
            .count()
            .cmp(&x.0.chars().count())
            .then(x.0.cmp(&y.0))
    });
    println!(
        "\nDAMAGED -- single lexical items, all-hiragana, shattered to a 1-2 char piece: {}",
        bad.len()
    );
    for (w, _, got, mp, _, _) in bad.iter().take(45) {
        println!("  {w}  (min {mp}) -> {got}");
    }
    if let Some(out) = a
        .iter()
        .position(|x| x == "--emit")
        .map(|i| a[i + 1].clone())
    {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(File::create(&out)?);
        // `--all` emits every shattered word, not just the damaged tail, so
        // the list can be ranked by how often it actually occurs.
        let emit_all = a.iter().any(|x| x == "--all");
        let rows: Vec<_> = if emit_all {
            shattered.iter().collect()
        } else {
            bad.clone()
        };
        for (w, _, _, mp, _, is_adv) in rows.iter() {
            let _ = mp;
            let n = w.chars().count() as f64;
            // MeCab's own user-dictionary heuristic, floored the way mkuserdic
            // floors it. Low enough that the long word wins Vibrato's lattice,
            // not so low that it swallows its neighbours.
            let cost = (-400.0 * n.powf(1.5)).max(-36000.0) as i32;
            let (l, r) = if *is_adv {
                (adv_l, adv_r)
            } else {
                (noun_l, noun_r)
            };
            let pos = if *is_adv {
                "副詞,一般"
            } else {
                "名詞,一般"
            };
            writeln!(f, "{w},{l},{r},{cost},{pos},*,*,*,*,{w},{w}")?;
        }
        // Companion list so a caller can filter by how badly the word broke:
        // one that already comes apart into pieces of three or more is served
        // better by `relax::MIN_PIECE` than by a hint token.
        let mut m = std::io::BufWriter::new(File::create(format!("{out}.meta"))?);
        for (w, _, _, mp, _, _) in rows.iter() {
            writeln!(m, "{w}\t{mp}")?;
        }
        eprintln!("wrote {} entries to {out}", rows.len());
    }
    Ok(())
}
