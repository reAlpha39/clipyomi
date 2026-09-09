// SPDX-License-Identifier: MIT OR Apache-2.0

//! Corpus counts for the 139 adjudicated compound types, so a candidate
//! frequency corpus can be judged before `cost.rs` is touched.
//!
//! `docs/superpowers/2026-09-09-frequency-signal-candidates.md` §11.3 priced
//! Phase 3's ceiling: a blanket COMMON discount takes the `lexical` class
//! from 41 to 88 of 112 (+47) and pays `transparent` 30 -> 10 and
//! `false-match` 30 -> 28 (-22), net +25. A frequency table is only worth
//! having if it earns the +47 without the -22, and that is a *ranking*
//! question over the adjudicated type list: does the table put the `lexical`
//! types above the `transparent` and `false-match` ones? §8's three corpus
//! candidates can therefore be separated by counting, with no engine change
//! and no cost model at all.
//!
//! Emits one JSONL record per distinct `(surface, headword)` type with every
//! count a ranking could plausibly key on, since one corpus pass is all of
//! them:
//!
//! | field | what it counts |
//! | --- | --- |
//! | `raw` | the surface as a substring, boundaries ignored |
//! | `aligned` | the surface where both its endpoints are token boundaries |
//! | `tok_exact` | the surface as exactly one token |
//! | `lemma_surface` / `lemma_headword` | tokens whose IPADIC base form is that string |
//! | `parts` / `part_tok` | the surface tokenized alone, and each part's own `tok_exact` |
//!
//! The tokenizer is the engine's own (`hints::VibratoTokenizer`'s dictionary),
//! so "as a word" here means what the shipped boundary hints already mean and
//! introduces no second segmenter's opinion.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example freq_counts -- \
//!   "$APPDATA/dev.jparser.clipyomi/hints/current/system.dic" \
//!   crates/jparser/tests/fixtures/compound_gold.jsonl \
//!   corpus_staging/freq/aozora.txt > counts_aozora.jsonl
//! ```

#[cfg(not(feature = "mecab"))]
fn main() {
    eprintln!("build with --features mecab");
    std::process::exit(2);
}

#[cfg(feature = "mecab")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use std::path::Path;
    use vibrato::{Dictionary, Tokenizer};

    let args: Vec<String> = std::env::args().collect();
    let [_, dic, gold, corpus] = args.as_slice() else {
        return Err("usage: freq_counts <system.dic> <compound_gold.jsonl> <corpus.txt>".into());
    };

    // Distinct (surface, headword) types, in file order. 194 windows collapse
    // to 139 types; the class is a property of the type, so the first
    // window's class is the type's.
    let mut types: Vec<(String, String, String)> = Vec::new();
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    for line in std::fs::read_to_string(Path::new(gold))?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(line)?;
        let s = v["surface"].as_str().unwrap_or_default().to_string();
        let h = v["headword"].as_str().unwrap_or_default().to_string();
        let c = v["class"].as_str().unwrap_or_default().to_string();
        if seen.insert((s.clone(), h.clone()), types.len()).is_none() {
            types.push((s, h, c));
        }
    }

    let reader = std::fs::File::open(Path::new(dic))?;
    let tokenizer = Tokenizer::new(Dictionary::read(reader)?);
    let mut worker = tokenizer.new_worker();

    let mut tok_count: HashMap<String, u64> = HashMap::new();
    let mut lemma_count: HashMap<String, u64> = HashMap::new();
    let mut raw = vec![0u64; types.len()];
    let mut aligned = vec![0u64; types.len()];
    let mut chars_seen: u64 = 0;
    let mut tokens_seen: u64 = 0;

    let text = std::fs::read_to_string(Path::new(corpus))?;
    // Sentence-ish chunks: `reset_sentence` is per sentence, and a boundary
    // set is only meaningful within one tokenization. Splitting on 。 as well
    // as newlines keeps a chunk to a length the lattice was designed for
    // without changing any count -- a target surface never spans a 。.
    for chunk in text
        .split(['\n', '。', '！', '？'])
        .map(str::trim)
        .filter(|c| !c.is_empty())
    {
        chars_seen += chunk.chars().count() as u64;

        worker.reset_sentence(chunk);
        worker.tokenize();

        // Char-offset boundary set, the same shape `hints::BoundarySet` holds.
        let char_len = chunk.chars().count();
        let mut bounds = vec![false; char_len + 1];
        bounds[0] = true;
        bounds[char_len] = true;
        for i in 0..worker.num_tokens() {
            let t = worker.token(i);
            let r = t.range_char();
            bounds[r.start] = true;
            bounds[r.end] = true;
            *tok_count.entry(t.surface().to_string()).or_default() += 1;
            // IPADIC feature CSV: 品詞,細分類1,2,3,活用型,活用形,原形,読み,発音.
            // Field 6 is the base form -- what a real frequency list is keyed
            // on, as against the inflected surface beside it.
            if let Some(base) = t.feature().split(',').nth(6) {
                if base != "*" {
                    *lemma_count.entry(base.to_string()).or_default() += 1;
                }
            }
            tokens_seen += 1;
        }

        // Byte offset -> char offset, so a byte-indexed match can ask the
        // char-indexed boundary set about its endpoints.
        let mut byte_to_char = vec![0u32; chunk.len() + 1];
        for (ci, (bi, _)) in chunk.char_indices().enumerate() {
            byte_to_char[bi] = ci as u32;
        }
        byte_to_char[chunk.len()] = char_len as u32;

        for (i, (surface, _, _)) in types.iter().enumerate() {
            let slen = surface.chars().count();
            for (bi, _) in chunk.match_indices(surface.as_str()) {
                raw[i] += 1;
                let cs = byte_to_char[bi] as usize;
                if bounds[cs] && bounds[cs + slen] {
                    aligned[i] += 1;
                }
            }
        }
    }

    eprintln!(
        "corpus: chars={chars_seen} tokens={tokens_seen} distinct_tokens={} distinct_lemmas={}",
        tok_count.len(),
        lemma_count.len()
    );

    for (i, (surface, headword, class)) in types.iter().enumerate() {
        // The surface tokenized on its own: the parts a split alternative is
        // made of, and the decomposition a build-time derivation would see.
        worker.reset_sentence(surface.as_str());
        worker.tokenize();
        let parts: Vec<String> = (0..worker.num_tokens())
            .map(|j| worker.token(j).surface().to_string())
            .collect();
        let part_tok: Vec<u64> = parts
            .iter()
            .map(|p| tok_count.get(p).copied().unwrap_or(0))
            .collect();

        let rec = serde_json::json!({
            "surface": surface,
            "headword": headword,
            "class": class,
            "raw": raw[i],
            "aligned": aligned[i],
            "tok_exact": tok_count.get(surface).copied().unwrap_or(0),
            "lemma_surface": lemma_count.get(surface).copied().unwrap_or(0),
            "lemma_headword": lemma_count.get(headword).copied().unwrap_or(0),
            "parts": parts,
            "part_tok": part_tok,
        });
        println!("{rec}");
    }

    Ok(())
}
