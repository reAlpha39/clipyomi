// SPDX-License-Identifier: MIT OR Apache-2.0

//! Does IPADIC's per-word cost give Phase 3 the frequency signal it wants?
//!
//! Master plan §7's Phase 3 note says frequency rank "stopped being a
//! nice-to-have at Phase 1a": JMdict's own priority marker is inverted on
//! exactly the compounds the plan exists to preserve, so it rewards
//! splitting them, and no cost-model magnitude fixes a signal pointing the
//! wrong way. The note adds that a frequency layer's licensing is
//! unresolved.
//!
//! It may already be resolved. §3.3 established IPADIC as permissive
//! (notice retention only) and the app already ships it as the boundary-hint
//! dictionary; `vibrato`'s `Token::word_cost()` is public. A MeCab-style word
//! cost is `-log P(word)` scaled, so it is a frequency-derived prior, already
//! on disk under a licence the plan accepts.
//!
//! The question this probe answers is whether it is the *right* signal.
//! IPADIC is short-unit, and the gap is compound preservation, so the thing
//! to find out is whether the compounds in question are IPADIC entries at
//! all -- if they are not, their cost is unavailable precisely where it is
//! needed.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example ipadic_cost_probe -- \
//!   "$APPDATA/dev.jparser.clipyomi/hints/current/system.dic"
//! ```

#[cfg(not(feature = "mecab"))]
fn main() {
    eprintln!("build with --features mecab");
    std::process::exit(2);
}

#[cfg(feature = "mecab")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use vibrato::{Dictionary, Tokenizer};

    let args: Vec<String> = std::env::args().collect();
    let [_, dic] = args.as_slice() else {
        return Err("usage: ipadic_cost_probe <system.dic>".into());
    };
    let reader = std::fs::File::open(Path::new(dic))?;
    let tokenizer = Tokenizer::new(Dictionary::read(reader)?);

    // Compounds the master plan names, plus controls: a very common word, a
    // rare one, and a compound whose parts are both common.
    let probes = [
        // Compound against its own parts, for whether the cost prefers the
        // compound the way a segmentation prior must.
        "東京都",
        "東京",
        "都",
        "日本語",
        "日本",
        "語",
        "情報処理",
        "情報",
        "処理",
        "研究者",
        "研究",
        "者",
        "高等学校",
        "高等",
        "学校",
        // Common against rare, for whether the cost is a frequency rank.
        // If it were, every word on the first line would be far cheaper than
        // every word on the second.
        "食べる",
        "見る",
        "言う",
        "人",
        "時間",
        "邂逅",
        "逍遥",
        "膏薬",
        "顳顬",
        "軋轆",
    ];

    println!(
        "{:<10} {:>6}  {:<28} tokens",
        "surface", "cost", "feature[0..3]"
    );
    for text in probes {
        let mut worker = tokenizer.new_worker();
        worker.reset_sentence(text);
        worker.tokenize();
        let n = worker.num_tokens();
        let parts: Vec<String> = (0..n)
            .map(|i| {
                let t = worker.token(i);
                format!("{}({})", t.surface(), t.word_cost())
            })
            .collect();
        // A single token spanning the whole surface means IPADIC holds this
        // surface as one entry, so its cost is usable for it.
        let whole = if n == 1 { "ONE ENTRY" } else { "split" };
        let first = worker.token(0);
        let feat: String = first
            .feature()
            .split(',')
            .take(3)
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{:<10} {:>6}  {:<28} {:<9} {}",
            text,
            first.word_cost(),
            feat,
            whole,
            parts.join(" + ")
        );
    }
    Ok(())
}
