// SPDX-License-Identifier: MIT OR Apache-2.0

// Mirrors crates/jparser/src/lib.rs. The Rust test
// `parse_result_serializes_to_the_documented_wire_shape` pins these names on the
// other side; changing one side alone breaks rendering silently.

export type FlagName =
  | 'primary'
  | 'pronounce'
  | 'common_line'
  | 'common'
  | 'particle'
  | 'counter'
  | 'top'
  | 'is_name';

export interface Sense {
  pos: string[];
  glosses: string[];
  xrefs: string[];
  misc: string[];
  info: string[];
}

export interface Entry {
  headword: string;
  reading: string | null;
  conjugation: string | null;
  pos: string[];
  senses: Sense[];
  flags: FlagName[];
}

export interface Segment {
  start: number;
  len: number;
  surface: string;
  reading: string | null;
  matched: boolean;
  entries: Entry[];
}

export interface ParseResult {
  segments: Segment[];
  /**
   * Readings 2..k, each a full cover like `segments`. The order is the rank:
   * readings that differ from an earlier one only by a merge or split come
   * after every reading that does not (Phase 2b design §4).
   */
  alternatives: Segment[][];
}

export type FuriganaMode = 'none' | 'hiragana' | 'katakana' | 'romaji';

export interface GlossFilters {
  hide_pos?: boolean;
  hide_xrefs?: boolean;
  hide_usage?: boolean;
}

/** Which of the three window chrome shapes the main window wears (spec §7.4). */
export type ChromeMode = 'native' | 'header' | 'content';

export interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
  /**
   * MeCab boundary hints. Backend-owned: `save_settings` never carries it, so
   * the settings window reads it and never writes it.
   */
  mecab_hints?: boolean;
  chrome_mode?: ChromeMode;
  furigana_mode?: FuriganaMode;
  hide_pos?: boolean;
  hide_xrefs?: boolean;
  hide_usage?: boolean;
  /** Sentence pane size in px, bound to `--text-sentence`. */
  font_sentence?: number;
  /** Definition and tooltip size in px, bound to `--text-gloss`. */
  font_gloss?: number;
  /** Furigana size as a percentage of the sentence, bound to `--text-furigana`. */
  furigana_scale?: number;
  /** Window background opacity as a percentage, bound to `--surface-alpha`. */
  surface_alpha?: number;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}

