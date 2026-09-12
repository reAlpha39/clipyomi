// SPDX-License-Identifier: MIT OR Apache-2.0

//! The installed font families, for the settings window's font selects.
//!
//! Enumerated once and cached: DirectWrite, CoreText and fontconfig each walk
//! every installed face to answer, which is hundreds of milliseconds nobody
//! wants paid on every open of the settings window.

use std::sync::OnceLock;

/// Family names as a select wants them: sorted case-insensitively, exact
/// duplicates folded (every face of a family reports the same family name),
/// blanks dropped.
pub fn family_names<I>(names: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut names: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .collect();
    names.sort_by_cached_key(|n| n.to_lowercase());
    names.dedup();
    names
}

fn enumerate() -> Vec<String> {
    match font_enumeration::Collection::new() {
        Ok(fonts) => family_names(fonts.take().into_iter().map(|f| f.family_name)),
        // An empty list leaves the selects showing only "Default"; the window
        // still works, the user just cannot pick a family from it.
        Err(_) => Vec::new(),
    }
}

/// The installed font families, sorted and deduplicated.
#[tauri::command]
pub fn system_fonts() -> Vec<String> {
    static FONTS: OnceLock<Vec<String>> = OnceLock::new();
    FONTS.get_or_init(enumerate).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    #[test]
    fn family_names_are_sorted_case_insensitively_and_deduplicated() {
        let got = family_names(owned(&[
            "Yu Gothic",
            "arial",
            "Yu Gothic",
            "Meiryo",
            "Arial",
        ]));
        assert_eq!(got, owned(&["arial", "Arial", "Meiryo", "Yu Gothic"]));
    }

    #[test]
    fn family_names_drop_blanks() {
        let got = family_names(owned(&["", "  ", " Meiryo "]));
        assert_eq!(got, owned(&["Meiryo"]));
    }
}
