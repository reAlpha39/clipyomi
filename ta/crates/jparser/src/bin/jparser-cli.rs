// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! Phase 1A verification harness. No UI, no Tauri, no network.

use std::io::BufReader;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use jparser::conjugation::ConjugationTable;
use jparser::index::build::build_from_reader;
use jparser::index::load::Index;
use jparser::record::WordFlags;
use jparser::stem::StemOptions;

/// Flags rendered by `lookup`, paired with their display labels.
const FLAG_LABELS: &[(WordFlags, &str)] = &[
    (WordFlags::PRIMARY, "primary"),
    (WordFlags::PRONOUNCE, "reading"),
    (WordFlags::COMMON, "common"),
    (WordFlags::COMMON_LINE, "common-line"),
    (WordFlags::PARTICLE, "particle"),
    (WordFlags::COUNTER, "counter"),
];

#[derive(Parser)]
#[command(name = "jparser-cli", about = "JParser Phase 1A harness")]
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
    /// Print every dictionary record that is a prefix of TEXT.
    Lookup {
        /// Index directory.
        index: PathBuf,
        /// Text to walk.
        text: String,
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
