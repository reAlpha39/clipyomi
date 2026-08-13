// JParser — Japanese text parser ported from Translation Aggregator.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.

//! One-off asset conversion: ta-old's UTF-16LE Conjugations.txt to UTF-8 JSON.
//!
//! Run with: cargo run -p xtask -- convert-conjugations

use std::fs;
use std::path::Path;

const SOURCE: &str = "../ta-old/dictionaries/Conjugations.txt";
const DEST: &str = "crates/jparser/assets/conjugations.json";
const UTF16LE_BOM: [u8; 2] = [0xFF, 0xFE];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("convert-conjugations") {
        eprintln!("usage: cargo run -p xtask -- convert-conjugations");
        std::process::exit(2);
    }

    let bytes = fs::read(SOURCE)?;
    let body = bytes.strip_prefix(&UTF16LE_BOM[..]).unwrap_or(&bytes);
    if body.len() % 2 != 0 {
        return Err("source is not valid UTF-16LE: odd byte count".into());
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|p| u16::from_le_bytes([p[0], p[1]]))
        .collect();
    let text = String::from_utf16(&units)?;

    // Round-trip through serde_json so the committed asset is validated and
    // normalized rather than trusted verbatim.
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let array = value.as_array().ok_or("expected a top-level JSON array")?;

    let path = Path::new(DEST);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(&value)?)?;

    let conjugations: usize = array
        .iter()
        .filter_map(|t| t.get("Tenses")?.as_array().map(Vec::len))
        .sum();
    println!("wrote {DEST}: {} types, {conjugations} conjugations", array.len());
    Ok(())
}
