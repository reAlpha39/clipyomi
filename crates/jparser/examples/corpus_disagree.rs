// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rank the places two engines disagree over a corpus, so defects are found
//! by counting rather than by screenshotting one sentence at a time.
//!
//! Every defect fixed on `feat/sokuon-tail` came from a screenshot pair, one
//! sentence at a time, which finds whatever the reviewer happened to paste.
//! This asks a corpus instead: where does today's engine cut differently from
//! the ta-old port, how often, and do the engine's *own* signals agree that
//! the difference is a defect.
//!
//! ## The second opinion is triage, not an oracle
//!
//! v1's own scores are below v2's on every cell (master plan §2, criterion
//! 2), so a disagreement is **not** evidence that v1 is right. It is a place
//! worth adjudicating against JMdict. The ranking therefore keys on how many
//! independent signals fire, not on v1's opinion, and a region no signal
//! touches sorts to the bottom.
//!
//! The v1 side is read from a TSV file produced out-of-tree, because that
//! engine is GPL-2.0-only and this crate is not. Nothing here links it, and
//! the file it reads is `text<TAB>start:len:matched,...`.
//!
//! ## The four signals
//!
//! | signal | what it means |
//! | --- | --- |
//! | `alt` | reading 1 differs from reading 2..k here — "the right cover ranked second", how `物思いにふける` and `なのに` were found |
//! | `rdg` | one entry covers the region and its reading is **not** the sum of the parts' readings — `唐揚げ` is `からあげ`, never `とう` + `あげ` |
//! | `sht` | one entry covers the region and this engine splits it anyway — `examples/shatter_sweep.rs`'s question, asked per occurrence |
//! | `prt` | a `は`/`へ` segment inside the region takes the PARTICLE flag, so `src/render/furigana.ts` romanises it `wa`/`e` — `へへ` read as `e e` |
//!
//! `alt=v1` is the strongest form of the first: an alternative reading agrees
//! with v1 exactly, so the cover is already in the lattice and only the cost
//! model ranked it second.
//!
//! **Three of the four detect over-*splitting* only, and that is a blind
//! spot, not a design.** `sht` requires that we cut a covered region, `rdg`
//! compares the *parts'* readings so a merge makes it trivially equal, and
//! `prt` needs a lone `は`/`へ` to still be its own segment — so when the
//! engine swallows one into a bogus word (`今の俺はいない` → `はい` "yes")
//! every one of them is silent. Only `alt` is symmetric, and it scores 1. A
//! region where this engine merged and v1 split can therefore never reach the
//! head of the ranking, and a change that increases merging will always look
//! good here. Read the ranking against a count of merge-direction regions,
//! not on its own.
//!
//! ```text
//! cargo run --release -p jparser --features mecab --example corpus_disagree -- \
//!   <v2-index> <corpus.txt> <v1-dump.tsv> [--hints <system.dic>] [--top N]
//! ```
//!
//! Read-only: it opens an index, parses text and prints. The corpus is a
//! local probe input and no part of it belongs in the repository.

#[cfg(feature = "mecab")]
use std::collections::BTreeSet;

#[cfg(feature = "mecab")]
const USAGE: &str =
    "usage: corpus_disagree <index-dir> <corpus.txt> <v1-dump.tsv> [--hints <dic>] [--top N]";

/// Kana that carry no word on their own: the small vowels, the sokuon, the
/// small `や`-row and the long mark, in both scripts.
#[cfg(feature = "mecab")]
const SMALL_KANA: &str = "ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶー";

#[cfg(feature = "mecab")]
/// Readings asked of the engine. 3 is what the `alt` signal was found with
/// (handoff §3a: `なのに` ranked second), and the cost of k is measured in
/// `jparser-cli bench`'s scaling cell.
const READINGS: usize = 3;

#[cfg(feature = "mecab")]
/// One disagreement region, accumulated over the corpus.
#[derive(Default)]
struct Region {
    /// Distinct sentences the region occurs in. Counted, not summed: one
    /// sentence repeating a surface is one piece of evidence, not three.
    sentences: BTreeSet<String>,
    v1_cover: String,
    v2_cover: String,
    example: String,
    /// The headword of the single entry covering the whole region, if one
    /// does, with its reading.
    covered_by: Option<(String, String)>,
    alt: bool,
    alt_is_v1: bool,
    reading_differs: bool,
    shattered: bool,
    particle_romaji: bool,
    /// The covering entry is what `lex::is_grammatical_stack` calls a stack —
    /// `には`, `それは`, `ではない`. JMdict carries every one of them as a
    /// headword, and success criterion 3 plus the 26-row `spurious_merges.txt`
    /// gate exist precisely to refuse those merges (handoff §3c). They are
    /// not defects, they are the standard, so they are reported apart from
    /// the ranking rather than at the top of it.
    stack: bool,
}

#[cfg(feature = "mecab")]
impl Region {
    /// `alt` and `sht` only, and the count is a tiebreak rather than the
    /// ranking — both measured over the corpus.
    ///
    /// `rdg` fired on 82 rows hints-on and **every one of them also fired
    /// `sht`**: it is a strict subset, so counting it adds no row and merely
    /// double-weights a quarter of the `sht` class. That is how `誰が`(たが)
    /// reached rank 1 on a surface whose 28 corpus sentences all read
    /// `だれが`. `prt` fired **0 times** hints-on across 4,838 rows and 8
    /// times hints-off, all 8 already `sht`. Both are kept as annotations,
    /// because they say something about the *kind* of defect, and neither
    /// ranks.
    fn signals(&self) -> usize {
        usize::from(self.alt) + usize::from(self.shattered)
    }

    fn tags(&self) -> String {
        let mut t = Vec::new();
        if self.alt {
            t.push(if self.alt_is_v1 { "alt=v1" } else { "alt" });
        }
        if self.reading_differs {
            t.push("rdg");
        }
        if self.shattered {
            t.push("sht");
        }
        if self.particle_romaji {
            t.push("prt");
        }
        if t.is_empty() {
            "-".into()
        } else {
            t.join("+")
        }
    }
}

#[cfg(feature = "mecab")]
/// Cut positions strictly inside `(lo, hi)`, from a span list.
///
/// Sorted, because the two sides are compared with `==` and only one of them
/// is this crate's. Nothing in the dump's format contract says the v1 spans
/// arrive in ascending order, and an unsorted dump would otherwise mint
/// regions whose cut *sets* are identical.
fn cuts_in(spans: &[(usize, usize)], lo: usize, hi: usize) -> Vec<usize> {
    let mut out: Vec<usize> = spans
        .iter()
        .map(|(s, l)| s + l)
        .filter(|&p| p > lo && p < hi)
        .collect();
    out.sort_unstable();
    out
}

/// Whether `spans` covers `0..n` exactly once, in order.
#[cfg(feature = "mecab")]
fn tiles(spans: &[(usize, usize)], n: usize) -> bool {
    spans
        .iter()
        .try_fold(0, |at, &(s, l)| (s == at).then_some(s + l))
        == Some(n)
}

#[cfg(feature = "mecab")]
fn surfaces_in(chars: &[char], spans: &[(usize, usize)], lo: usize, hi: usize) -> String {
    spans
        .iter()
        .filter(|(s, l)| *s >= lo && s + l <= hi)
        .map(|(s, l)| chars[*s..s + l].iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("|")
}

/// The probe needs the app's own tokenizer to reproduce the hint state the
/// user runs in, so it is `mecab`-only. `examples/freq_counts.rs` carries the
/// same pair for the same reason: `cargo check -p jparser
/// --no-default-features --all-targets` builds every example, and an
/// unconditional `jparser::hints` path fails it.
#[cfg(not(feature = "mecab"))]
fn main() {
    eprintln!("build with --features mecab");
    std::process::exit(2);
}

#[cfg(feature = "mecab")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::collections::HashMap;
    use std::fs;
    use std::path::Path;

    use jparser::engine::table::Table;
    use jparser::engine::{kana, lex};
    use jparser::index::load::Index;
    use jparser::{parse, Boundaries, ParseOptions, WordFlags};

    let args: Vec<String> = std::env::args().skip(1).collect();
    let [index_dir, corpus, v1_dump, ..] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let hints = match args.iter().position(|a| a == "--hints") {
        Some(i) => Some(args.get(i + 1).ok_or(USAGE)?.clone()),
        None => None,
    };
    let top: usize = args
        .iter()
        .position(|a| a == "--top")
        .map(|i| args.get(i + 1).expect(USAGE).parse().expect(USAGE))
        .unwrap_or(40);

    // `text -> spans`. The dump is keyed by the sentence itself rather than by
    // a line number so the two sides cannot silently drift out of alignment
    // when one of them skips a line.
    let dump = fs::read_to_string(Path::new(v1_dump))?;
    let mut v1: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for line in dump.lines() {
        let Some((text, spans)) = line.split_once('\t') else {
            continue;
        };
        let parsed = spans
            .split(',')
            .filter(|s| !s.is_empty())
            .filter_map(|s| {
                let mut it = s.split(':');
                Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
            })
            .collect();
        v1.insert(text, parsed);
    }
    eprintln!("v1 dump: {} sentences", v1.len());

    let index = Index::open(Path::new(index_dir))?;
    let table = Table::new();
    let tokenizer = match &hints {
        Some(path) => Some(jparser::hints::VibratoTokenizer::load(Path::new(path))?),
        None => None,
    };
    let opts = ParseOptions::with_readings(READINGS);

    let mut regions: HashMap<String, Region> = HashMap::new();
    let (mut sentences, mut disagreeing, mut missing) = (0usize, 0usize, 0usize);
    let mut untiled = 0usize;

    for text in fs::read_to_string(Path::new(corpus))?.lines() {
        if text.trim().is_empty() {
            continue;
        }
        let Some(v1_spans) = v1.get(text) else {
            missing += 1;
            continue;
        };
        sentences += 1;
        let chars: Vec<char> = text.chars().collect();
        let n = chars.len();
        let bound_set = tokenizer.as_ref().map(|t| t.boundaries(text));
        let bounds: Option<&dyn Boundaries> = bound_set.as_ref().map(|b| b as &dyn Boundaries);
        let parsed = parse(&index, &table, text, &opts, bounds)?;

        let v2_spans: Vec<(usize, usize)> =
            parsed.segments.iter().map(|s| (s.start, s.len)).collect();
        // Both starts and ends. A dump that omits its unmatched runs would
        // otherwise hide every gap's opening position, `b1 == b2` would be
        // unsatisfiable and every sentence would read as disagreeing. The
        // tiling check below is the other half of that guard: `surfaces_in`
        // and the reading comparisons all assume no span straddles a region
        // edge, which holds only while both lists cover `0..n` exactly.
        let b1: BTreeSet<usize> = std::iter::once(0)
            .chain(v1_spans.iter().flat_map(|&(s, l)| [s, s + l]))
            .collect();
        let b2: BTreeSet<usize> = std::iter::once(0)
            .chain(v2_spans.iter().flat_map(|&(s, l)| [s, s + l]))
            .collect();
        if !tiles(v1_spans, n) {
            untiled += 1;
            continue;
        }
        if b1 == b2 {
            continue;
        }
        disagreeing += 1;

        // Candidates are recomputed rather than returned by `parse`: this is a
        // probe, so it pays a second pass instead of widening the engine's
        // public result type (`span_dump.rs` makes the same trade).
        let by_start = lex::candidates(&index, &table, &chars)?;

        // Positions both engines agree are boundaries bound each region; a
        // region is a maximal stretch where the cut sets differ.
        let common: Vec<usize> = b1
            .intersection(&b2)
            .copied()
            .chain(std::iter::once(n))
            .collect();
        let mut common = common;
        common.sort_unstable();
        common.dedup();

        for w in common.windows(2) {
            let (lo, hi) = (w[0], w[1]);
            let c1 = cuts_in(v1_spans, lo, hi);
            let c2 = cuts_in(&v2_spans, lo, hi);
            if c1 == c2 {
                continue;
            }
            let surface: String = chars[lo..hi].iter().collect();
            // A region of pure punctuation is a disagreement about how to
            // group characters neither engine matched. Nothing to adjudicate.
            //
            // A small kana does not count as content. The first version of
            // this guard asked `is_kana || is_kanji` and let 146 zero-signal
            // rows of `ぅ……`, `ゅ、`, `っ、` through, because `ぅ` is kana:
            // v1 glues an unmatched small kana to the punctuation beside it
            // and the engine does not, which is not a segmentation opinion.
            if !surface
                .chars()
                .any(|c| kana::is_kanji(c) || (kana::is_kana(c) && !SMALL_KANA.contains(c)))
            {
                continue;
            }
            let r = regions.entry(surface.clone()).or_default();
            if r.sentences.is_empty() {
                r.example = text.to_string();
                r.v1_cover = surfaces_in(&chars, v1_spans, lo, hi);
                r.v2_cover = surfaces_in(&chars, &v2_spans, lo, hi);
            }
            r.sentences.insert(text.to_string());

            // `alt`: does any other reading cut this region differently, and
            // does one of them agree with v1 outright?
            for alt in &parsed.alternatives {
                let spans: Vec<(usize, usize)> = alt.iter().map(|s| (s.start, s.len)).collect();
                let ca = cuts_in(&spans, lo, hi);
                if ca != c2 {
                    r.alt = true;
                    if ca == c1 {
                        r.alt_is_v1 = true;
                    }
                }
            }

            // `sht`: one entry covers the whole region and the engine split it.
            // The same question `shatter_sweep.rs` asks of the dictionary,
            // asked here of an occurrence, so frequency can rank it.
            // Uninflected and exact only. A deinflecting cover is still "an
            // entry that spans it" in the lattice's sense and is almost
            // always noise here: `はもう` is covered by `食む` through a chain,
            // which says nothing about whether `は`|`もう` was the wrong cut.
            // A PRONOUNCE-only record is a *reading* of some kanji entry, not
            // a way the word is written: `はは` finds 義母 (ぎぼ), `誰が` finds
            // 誰が (たが), `みた` finds 蒙(むた). Taking those as covers is
            // `shatter_sweep.rs`'s own §9.5 absurdity, so PRIMARY is required.
            let cover = by_start
                .get(lo)
                .into_iter()
                .flatten()
                .find(|c| {
                    c.len == hi - lo
                        && c.exact
                        && c.steps.is_empty()
                        && c.flags.contains(WordFlags::PRIMARY)
                })
                .map(|c| (c.entry_id, c.headword.clone(), c.grammatical_stack));
            if let Some((id, headword, stack)) = cover {
                // A region whose last piece is a COMMON particle is the same
                // refusal as a grammatical stack, and `is_grammatical_stack`
                // does not catch it because it asks about *every* piece.
                // Measured: 33 such rows carry 287 occurrences and only two
                // are worth fixing. The class is every archaic-reading trap —
                // `誰が`(たが), `天の`(あめの), `外に`(ほかに), `本の`(ほんの),
                // `並に`(ならびに) — where the split is right, plus `みたいな`
                // at n = 128. Commonness of the *cover* is not the
                // discriminator and reaching for it is backwards: those are
                // `[primary,common-line]` while `呼び方`, `部屋着` and
                // `キャンプ場`, the real defects, are bare `[primary]`.
                let particle_tail = parsed
                    .segments
                    .iter()
                    .rfind(|s| s.start >= lo && s.start + s.len <= hi)
                    .and_then(|s| s.entries.first())
                    .is_some_and(|e| {
                        e.flags.contains(WordFlags::PARTICLE) && e.flags.contains(WordFlags::COMMON)
                    });
                r.stack |= stack || particle_tail;
                // Every reading, not the first. An entry stores its `<reb>`
                // forms in document order and a multi-reading entry's first is
                // arbitrary as far as this comparison goes, so `rdg` would
                // otherwise fire or not for a reason unrelated to the region.
                // Kana-only entries store none and the surface is the reading.
                let readings = index.entry(id)?.map(|e| e.readings).unwrap_or_default();
                let whole = if readings.is_empty() {
                    vec![surface.clone()]
                } else {
                    readings
                };
                r.covered_by = Some((headword, whole[0].clone()));
                if !c2.is_empty() {
                    r.shattered = true;
                }
                // `rdg`: the reading is not the sum of its parts, which is
                // what separates a lexical compound (`唐揚げ` is `からあげ`)
                // from a compositional one (`無断で` is `むだん` + `で`).
                let parts: String = parsed
                    .segments
                    .iter()
                    .filter(|s| s.start >= lo && s.start + s.len <= hi)
                    .map(|s| s.reading.clone().unwrap_or_else(|| s.surface.clone()))
                    .collect();
                if !parts.is_empty()
                    && !whole
                        .iter()
                        .any(|w| kana::fold_str(&parts) == kana::fold_str(w))
                {
                    r.reading_differs = true;
                }
                // `prt`: a lone は/へ inside a region a real word covers takes
                // the particle romanisation in the frontend.
                if parsed
                    .segments
                    .iter()
                    .filter(|s| s.start >= lo && s.start + s.len <= hi)
                    .any(|s| {
                        matches!(s.surface.as_str(), "は" | "へ" | "ハ" | "ヘ")
                            && s.entries
                                .first()
                                .is_some_and(|e| e.flags.contains(WordFlags::PARTICLE))
                    })
                {
                    r.particle_romaji = true;
                }
            }
        }
    }

    // **Frequency first, among the rows a signal reached.** Ranking by signal
    // count was measured against the corpus and it is close to ranking by
    // "does a dictionary entry happen to exist": `alt` fires on 53% of rows
    // and `sht` on 6.7%, so the count separates almost nothing while
    // frequency — the thing the whole probe exists to supply — was only a
    // tiebreak inside it. It buried `いた` (219 sentences, `待って`|`い`|`た`
    // where `い` glosses "the Boar") at rank 316 under `誰が` at 28. Whether
    // *any* signal fired still gates: a row no signal touches is probably v1
    // being wrong, and those sort last.
    let by_rank = |a: &(&String, &Region), b: &(&String, &Region)| {
        (b.1.signals() > 0)
            .cmp(&(a.1.signals() > 0))
            .then(b.1.sentences.len().cmp(&a.1.sentences.len()))
            .then(b.1.signals().cmp(&a.1.signals()))
            .then(a.0.cmp(b.0))
    };
    let (stacks, mut ranked): (Vec<_>, Vec<_>) = regions.iter().partition(|(_, r)| r.stack);
    ranked.sort_by(by_rank);
    let mut stacks = stacks;
    stacks.sort_by(by_rank);

    println!("sentences parsed:        {sentences}");
    println!("  not in the v1 dump:    {missing}");
    println!("  v1 spans do not tile:  {untiled}");
    println!("  where the two differ:  {disagreeing}");
    println!("distinct regions:        {}", regions.len());
    println!(
        "  grammatical stacks:    {} (refused by criterion 3, listed apart)",
        stacks.len()
    );
    for k in (1..=4).rev() {
        let n = ranked.iter().filter(|(_, r)| r.signals() == k).count();
        println!("  {k} signal(s):           {n}");
    }
    println!(
        "  none (probably v1):    {}",
        ranked.iter().filter(|(_, r)| r.signals() == 0).count()
    );

    // Three of the four signals detect over-*splitting* only (module doc), so
    // a change that merges more will always look better in the ranking above.
    // The counter-reading is this: how many regions did each engine cut into
    // fewer pieces. Both counts are needed, because the ranking cannot see one
    // of them.
    let pieces = |cover: &str| cover.split('|').count();
    let (mut merged, mut split, mut same) = (0usize, 0usize, 0usize);
    let (mut merged_n, mut split_n, mut same_n) = (0usize, 0usize, 0usize);
    for r in regions.values() {
        let (a, b) = (pieces(&r.v1_cover), pieces(&r.v2_cover));
        let n = r.sentences.len();
        match b.cmp(&a) {
            std::cmp::Ordering::Less => {
                merged += 1;
                merged_n += n;
            }
            std::cmp::Ordering::Greater => {
                split += 1;
                split_n += n;
            }
            std::cmp::Ordering::Equal => {
                same += 1;
                same_n += n;
            }
        }
    }
    println!("merge direction, all regions (the ranking above cannot see the first)");
    println!("  we merge, v1 splits:   {merged:>6} regions, {merged_n:>6} sentence-occurrences");
    println!("  we split, v1 merges:   {split:>6} regions, {split_n:>6} sentence-occurrences");
    println!("  same count, cuts move: {same:>6} regions, {same_n:>6} sentence-occurrences");

    println!(
        "\n{:>4}  {:<20} {:>4}  {:<12} {:<26} {:<26} one entry covering it",
        "#", "signals", "n", "region", "v1", "v2 (ours)"
    );
    for (i, (surface, r)) in ranked.iter().take(top).enumerate() {
        let cover = match &r.covered_by {
            Some((h, rd)) => format!("{h} ({rd})"),
            None => "-".into(),
        };
        println!(
            "{:>4}  {:<20} {:>4}  {:<12} {:<26} {:<26} {}",
            i + 1,
            r.tags(),
            r.sentences.len(),
            surface,
            r.v1_cover,
            r.v2_cover,
            cover
        );
    }

    println!(
        "\ngrammatical stacks, refused on purpose — top {}",
        stacks.len().min(10)
    );
    for (surface, r) in stacks.iter().take(10) {
        println!("  {:<12} {:>4}  {}", surface, r.sentences.len(), r.v2_cover);
    }

    println!("\nexamples");
    for (surface, r) in ranked.iter().take(top.min(20)) {
        println!("  {surface}  [{}]  {}", r.tags(), r.example);
    }
    Ok(())
}
