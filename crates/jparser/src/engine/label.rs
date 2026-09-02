// ClipYomi — Engine v2, Phase 1b-i.
// Copyright (C) 2026
//
// This program is free software; you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 2 as published
// by the Free Software Foundation.
//
// Written for ClipYomi without reference to Translation Aggregator's JParser.
// See docs/superpowers/specs/2026-08-27-engine-v2-master-plan.md §3.5 for the
// clean-room discipline this file was written under. The SPDX header of §9.2
// is written in Phase 1b-ii, when the crate stops being GPL-2.0-only.
//
// English vocabulary from "Japanese verb conjugation", English Wikipedia,
// https://en.wikipedia.org/wiki/Japanese_verb_conjugation, read 2026-09;
// terminology only. assets/conjugations.json, src/conjugation.rs and
// src/matcher.rs's label renderer were not opened while this file or
// tests/fixtures/conjugation_labels.txt was written.

//! The English conjugation label -- Phase 1b-i's half of master plan §7.3's
//! recombiner. Reads [`ConjStep`]s, writes `Entry::conjugation`.
//!
//! Stub: Task 3 of the 1b-i plan writes the formatter against
//! `tests/fixtures/conjugation_labels.txt`, which exists first.

use crate::engine::ConjStep;

/// The learner-facing label for a deinflection chain, innermost step first,
/// or `None` when no step contributes a word.
pub fn fmt(steps: &[ConjStep]) -> Option<String> {
    let _ = steps;
    None
}
