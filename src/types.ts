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
  | 'is_name'
  | 'uk';

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

/**
 * One KANJIDIC2 record, as `kanji_info` returns it. Mirrors
 * `jparser::index::KanjiData`; the Rust test
 * `kanji_data_serializes_to_the_documented_wire_shape` pins these names on the
 * other side.
 */
export interface KanjiInfo {
  literal: string;
  /** On readings, katakana. */
  on: string[];
  /** Kun readings, with KANJIDIC2's `.` and `-` marks kept: `た.べる`, `-がた`. */
  kun: string[];
  /** English meanings, at most four. */
  meanings: string[];
  // KANJIDIC2's `<misc>` numbers (Phase 3b). The backend always sends each
  // as a number or `null`; they are optional here so a payload from an older
  // main webview, and the test suites' shorter literals, stay valid.
  /** School grade code: 1–6 the year taught, 8 remaining jōyō (secondary school), 9–10 name kanji. */
  grade?: number | null;
  /** The accepted stroke count. */
  strokes?: number | null;
  /** Newspaper frequency rank, 1–2,501. */
  freq?: number | null;
  /** Level in the former four-level JLPT, 1 (advanced) to 4 (elementary). */
  jlpt?: number | null;
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
  /** Phase 3a: the KANJIDIC block at the foot of the popover. Default shown. */
  hide_kanji?: boolean;
}

/** Which of the three window chrome shapes the main window wears (spec §7.4). */
export type ChromeMode = 'native' | 'header' | 'content';

/** Whether the main window grows to fit a long sentence, and from which edge. */
export type GrowDirection = 'off' | 'up' | 'down';

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
  /** Phase 3a: the KANJIDIC block at the foot of the popover. Default shown. */
  hide_kanji?: boolean;
  /** Sentence pane size in px, bound to `--text-sentence`. */
  font_sentence?: number;
  /** Definition and tooltip size in px, bound to `--text-gloss`. */
  font_gloss?: number;
  /** Furigana size as a percentage of the sentence, bound to `--text-furigana`. */
  furigana_scale?: number;
  /** Space between the furigana and its text in px, bound to `--furigana-gap`. */
  furigana_gap?: number;
  /** Window background opacity as a percentage, bound to `--surface-alpha`. */
  surface_alpha?: number;
  // Text appearance. Each absent key leaves `tokens.css` owning the default.
  /** `#rrggbb`, bound to `--color-kanji`. */
  colour_kanji?: string;
  colour_kana?: string;
  colour_particle?: string;
  colour_counter?: string;
  colour_furigana?: string;
  /** Punctuation and runs the parser matched nothing for, bound to `--color-unmatched`. */
  colour_unmatched?: string;
  /** The main window's background, bound to `--color-bg`. */
  colour_bg?: string;
  /** A family name, bound to `--font-cjk`. */
  font_family_sentence?: string;
  /** Bound to `--font-furigana`; unset, the ruby inherits the sentence font. */
  font_family_furigana?: string;
  /** Rides the popover payload as `gloss_font`, bound to `--font-gloss` there. */
  font_family_gloss?: string;
  /** A stroke around the sentence text, for reading it over a see-through window. */
  text_outline?: boolean;
  /** Stroke width in px, bound to `--text-stroke-width`. */
  text_outline_width?: number;
  /** A drop shadow under the sentence text. */
  text_shadow?: boolean;
  /** Shadow offset and blur in px, bound to `--text-shadow-size`. */
  text_shadow_size?: number;
  /** Grow the main window to fit an overflowing sentence, and from which edge. */
  grow_direction?: GrowDirection;
  /** The most lines that growth may add. */
  grow_lines?: number;
  /** Whether growing and shrinking animate. Absent reads as on. */
  grow_animate?: boolean;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}

