// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! JParser Phase 1 harness. No UI, no Tauri, no network.

use std::io::BufReader;
use std::path::PathBuf;

use clap::{ArgGroup, Parser, Subcommand};
use jmdict_source::SOURCE_DIR;
use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::ensure_dictionary;
use jparser::index::generations::{
    generation_number, latest, sweep, DEFAULT_KEEP_GENERATIONS, GENERATION_PREFIX,
};
use jparser::index::load::Index;
use jparser::record::WordFlags;
use jparser::stem::StemOptions;
use jparser::ParseOptions;

/// Flags rendered by `lookup`, paired with their display labels.
const FLAG_LABELS: &[(WordFlags, &str)] = &[
    (WordFlags::PRIMARY, "primary"),
    (WordFlags::PRONOUNCE, "reading"),
    (WordFlags::COMMON, "common"),
    (WordFlags::COMMON_LINE, "common-line"),
    (WordFlags::PARTICLE, "particle"),
    (WordFlags::COUNTER, "counter"),
];

/// Rendered wherever a `reading` or `conjugation` is `None`, and by
/// `ensure-dictionary` when there is no generation to name. Named because it
/// is part of the frozen output format, not incidental formatting.
const NONE_LABEL: &str = "-";

#[derive(Parser)]
#[command(name = "jparser-cli", about = "JParser Phase 1 harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build an index from a JMdict XML file.
    BuildIndex {
        /// Path to JMdict_e.xml (uncompressed).
        xml: PathBuf,
        /// Directory to write the index into.
        out: PathBuf,
        /// Disable the v5 mis-annotation fallback, to measure its effect.
        #[arg(long)]
        no_v5_fallback: bool,
    },
    /// Open the newest usable index in ROOT, building from a source if needed.
    #[command(group(
        ArgGroup::new("source").required(true).args(["xml", "source_dir"])
    ))]
    EnsureDictionary {
        /// Generation root directory.
        root: PathBuf,
        /// Path to an uncompressed JMdict XML file, read only if a build is
        /// needed. Kept positional so existing invocations still work.
        xml: Option<PathBuf>,
        /// Directory holding the source archive, downloading it if absent.
        #[arg(long)]
        source_dir: Option<PathBuf>,
        /// Generations to retain after a rebuild. Must be at least 1.
        #[arg(
            long,
            default_value_t = DEFAULT_KEEP_GENERATIONS,
            value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
        )]
        keep: usize,
    },
    /// Download the JMdict archive into DIR without building an index.
    FetchSource {
        /// Directory to download into. Conventionally named `source`.
        dir: PathBuf,
    },
    /// List the generations in ROOT, newest first.
    GenList {
        /// Generation root directory.
        root: PathBuf,
    },
    /// Remove build orphans and all but the newest generations.
    GenSweep {
        /// Generation root directory.
        root: PathBuf,
        /// Generations to retain. Must be at least 1.
        #[arg(
            long,
            default_value_t = DEFAULT_KEEP_GENERATIONS,
            value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
        )]
        keep: usize,
    },
    /// Remove exactly `gen-GENERATION` from ROOT, whether or not it exists.
    ///
    /// The repair path for an unopenable newest generation: `sweep` never
    /// removes the newest, `ensure-dictionary` returns the error rather than
    /// rebuilding over it, and `--keep 0` is rejected. This removes it
    /// directly so a later `ensure-dictionary` call can build a fresh one.
    GenRemove {
        /// Generation root directory.
        root: PathBuf,
        /// Generation number to remove.
        generation: u64,
    },
    /// Print every dictionary record that is a prefix of TEXT.
    Lookup {
        /// Index directory.
        index: PathBuf,
        /// Text to walk.
        text: String,
    },
    /// Segment TEXT against an index and print the result.
    Parse {
        /// Index directory.
        index: PathBuf,
        /// Text to parse.
        text: String,
        /// Path to an uncompressed compiled Vibrato dictionary. When given,
        /// tokenization supplies boundary hints to the segmenter.
        ///
        /// The distributed archive is compressed twice, and this flag wants
        /// neither layer. Download IPADIC from
        /// https://github.com/daac-tools/vibrato/releases/download/v0.5.0/ipadic-mecab-2_7_0.tar.xz
        /// then run: tar xf ipadic-mecab-2_7_0.tar.xz && zstd -d
        /// ipadic-mecab-2_7_0/system.dic.zst -- and pass the resulting
        /// ipadic-mecab-2_7_0/system.dic here.
        #[cfg(feature = "mecab")]
        #[arg(long)]
        hints: Option<PathBuf>,
    },
    /// Convert kana to romaji.
    Romaji {
        text: String,
        /// Apply the particle-only corrections (は to wa, へ to e).
        #[arg(long)]
        particle: bool,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match Cli::parse().command {
        Command::BuildIndex {
            xml,
            out,
            no_v5_fallback,
        } => {
            let table = ConjugationTable::load_embedded()?;
            let opts = StemOptions {
                v5_misannotation_fallback: !no_v5_fallback,
            };
            let file = BufReader::new(std::fs::File::open(&xml)?);
            let report = build_from_reader(file, &table, &opts, &out)?;
            println!("entries:             {}", report.entries);
            println!("skipped entries:     {}", report.skipped_entries);
            println!("keys:                {}", report.keys);
            println!("records:             {}", report.records);
            println!("stems (exact):       {}", report.stems.exact_stems);
            println!("stems (v5 fallback): {}", report.stems.v5_fallback_stems);
            println!("stems (empty):       {}", report.stems.empty_stems);
            if report.skipped_entries > 0 {
                eprintln!(
                    "warning: {} malformed entries were skipped",
                    report.skipped_entries
                );
            }
        }
        Command::EnsureDictionary {
            root,
            xml,
            source_dir,
            keep,
        } => {
            let table = ConjugationTable::load_embedded()?;
            let opts = StemOptions::default();
            // The ArgGroup guarantees exactly one is set, so both real branches
            // are reachable and the third exists only to satisfy the compiler.
            let index = match (&xml, &source_dir) {
                (Some(xml), _) => ensure_dictionary(&root, &table, &opts, keep, || {
                    std::fs::File::open(xml).map(BufReader::new)
                })?,
                (None, Some(dir)) => {
                    ensure_dictionary(&root, &table, &opts, keep, || jmdict_source::resolve(dir))?
                }
                (None, None) => return Err("no source given".into()),
            };
            let current = latest(&root)?;
            let name = current
                .as_deref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| NONE_LABEL.to_string());
            println!("generation: {name}");
            println!("entries:    {}", index.entry_count());
        }
        Command::FetchSource { dir } => {
            let path = jmdict_source::fetch::fetch(&dir)?;
            println!("archive:    {}", path.display());
            println!("convention: keep it in a directory named {SOURCE_DIR}");
        }
        Command::GenList { root } => {
            let mut entries: Vec<(String, PathBuf)> = Vec::new();
            for entry in std::fs::read_dir(&root)? {
                let path = entry?.path();
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                if !name.starts_with(GENERATION_PREFIX) {
                    continue;
                }
                entries.push((name, path));
            }
            // Numeric, not lexicographic: `--help` promises "newest first",
            // and `gen-10` must sort ahead of `gen-9`. A name that is not a
            // valid generation number sorts last — `latest` ignores it too.
            entries.sort_by_key(|(name, _)| std::cmp::Reverse(generation_number(name)));
            for (name, path) in entries {
                match Index::open(&path) {
                    Ok(index) => println!("{name} ok entries={}", index.entry_count()),
                    Err(e) => println!("{name} unusable {e}"),
                }
            }
        }
        Command::GenSweep { root, keep } => {
            println!("removed: {}", sweep(&root, keep)?);
        }
        Command::GenRemove { root, generation } => {
            let target = root.join(format!("{GENERATION_PREFIX}{generation}"));
            match std::fs::remove_dir_all(&target) {
                Ok(()) => println!("removed: {}", target.display()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("absent: {}", target.display());
                }
                Err(e) => return Err(e.into()),
            }
        }
        Command::Lookup { index, text } => {
            let table = ConjugationTable::load_embedded()?;
            let index = Index::open(&index)?;
            for hit in index.prefixes_of(&text)? {
                let matched: String = text.chars().take(hit.key_chars).collect();
                println!("[{matched}] ({} chars)", hit.key_chars);
                for r in &hit.records {
                    let verb = match r.verb_type {
                        Some(vt) => table.types()[vt].name.as_str(),
                        None => "-",
                    };
                    let flags = WordFlags(r.flags);
                    let labels: Vec<&str> = FLAG_LABELS
                        .iter()
                        .filter(|(f, _)| flags.contains(*f))
                        .map(|(_, name)| *name)
                        .collect();
                    let glosses = index
                        .entry(r.entry_id)?
                        .and_then(|e| e.senses.first().map(|s| s.glosses.join("; ")))
                        .unwrap_or_default();
                    println!(
                        "    {:8} type={verb:8} [{}] {glosses}",
                        r.surface,
                        labels.join(",")
                    );
                }
            }
        }
        Command::Parse {
            index,
            text,
            #[cfg(feature = "mecab")]
            hints,
        } => {
            let table = ConjugationTable::load_embedded()?;
            let index = Index::open(&index)?;

            #[cfg(feature = "mecab")]
            let tokenizer = match &hints {
                Some(path) => Some(jparser::hints::VibratoTokenizer::load(path)?),
                None => None,
            };
            #[cfg(feature = "mecab")]
            let flags = tokenizer.as_ref().map(|t| t.hints(&text));
            #[cfg(feature = "mecab")]
            let hints: Option<&dyn jparser::BoundaryHints> =
                flags.as_ref().map(|f| f as &dyn jparser::BoundaryHints);
            #[cfg(not(feature = "mecab"))]
            let hints: Option<&dyn jparser::BoundaryHints> = None;

            let result = jparser::parse(&index, &table, &text, &ParseOptions::default(), hints)?;
            for seg in &result.segments {
                if !seg.matched {
                    println!(
                        "start={} len={} {} unmatched",
                        seg.start, seg.len, seg.surface
                    );
                    continue;
                }
                println!(
                    "start={} len={} {} matched reading={}",
                    seg.start,
                    seg.len,
                    seg.surface,
                    seg.reading.as_deref().unwrap_or(NONE_LABEL)
                );
                for entry in &seg.entries {
                    let glosses = entry
                        .senses
                        .first()
                        .map(|s| s.glosses.join("; "))
                        .unwrap_or_default();
                    println!(
                        "    {} ({}) [{}] {glosses}",
                        entry.headword,
                        entry.conjugation.as_deref().unwrap_or(NONE_LABEL),
                        entry.reading.as_deref().unwrap_or(NONE_LABEL),
                    );
                }
            }
        }
        Command::Romaji { text, particle } => {
            let out = jparser::romaji::to_romaji(&text);
            let out = if particle {
                jparser::romaji::apply_particle_fixup(&out)
            } else {
                out
            };
            println!("{out}");
        }
    }
    Ok(())
}
