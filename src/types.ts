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
}

export interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
  decorations: boolean;
  window_width?: number;
  window_height?: number;
  window_x?: number;
  window_y?: number;
}

