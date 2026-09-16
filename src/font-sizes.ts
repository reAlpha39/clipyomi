// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Settings } from './types';

/**
 * The sizes the user can move. Furigana is a percentage, the rest px: `gap`
 * is the space between the furigana and the text it annotates, `outline` the
 * width of the stroke around the sentence glyphs, and `shadow` the one knob
 * the drop shadow's offset and blur are both derived from.
 */
export type FontKind = 'sentence' | 'gloss' | 'furigana' | 'gap' | 'outline' | 'shadow';

/**
 * Bounds for each control.
 *
 * These are the guard, not just the `min`/`max` on the number inputs: the
 * settings file is hand-editable, so every value arriving from it is untrusted
 * and passes through `clampFont` before it reaches a stylesheet.
 */
export const FONT_LIMITS: Record<FontKind, { min: number; max: number }> = {
  sentence: { min: 12, max: 48 },
  gloss: { min: 10, max: 28 },
  furigana: { min: 30, max: 100 },
  gap: { min: 0, max: 24 },
  // A stroke is centred on the glyph edge, so only half of this width shows
  // outside it; past 8px the inner half closes up the counters of dense kanji.
  outline: { min: 1, max: 8 },
  shadow: { min: 0, max: 12 },
};

/**
 * Ratio of the romaji reading to kana furigana as a fraction of a thousand,
 * from the two stylesheet defaults it replaces: `0.52em` against `0.58em` in
 * `global.css`, near enough 0.9. Scaling furigana keeps romaji in the same
 * proportion rather than letting the two cross over.
 *
 * A whole number over a whole number because `58 * 0.9 / 100` lands on a float
 * whose shortest representation is not the decimal it should be, and that
 * string is what reaches the stylesheet.
 */
const ROMAJI_PER_MILLE = 9;
const PER_MILLE = 1000;

/** Percent to the em ratio the stylesheet wants. */
const PERCENT = 100;

/** The token each kind is bound to. */
const TOKENS: Record<FontKind, string> = {
  sentence: '--text-sentence',
  gloss: '--text-gloss',
  furigana: '--text-furigana',
  gap: '--furigana-gap',
  outline: '--text-stroke-width',
  shadow: '--text-shadow-size',
};

/** VALUE pinned into KIND's range, or `null` if it is not a finite number. */
export function clampFont(kind: FontKind, value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  const { min, max } = FONT_LIMITS[kind];
  return Math.min(max, Math.max(min, Math.round(value)));
}

/**
 * The custom properties SETTINGS asks for, as `[token, value]` pairs.
 *
 * A size the settings file does not carry produces an empty value rather than
 * no pair at all: clearing a size box sends a snapshot with the key gone, and
 * the token then has to be removed so `tokens.css` owns that size again. This
 * module never restates a default of its own.
 */
export function fontVars(settings: Partial<Settings>): Array<[string, string]> {
  const sentence = clampFont('sentence', settings.font_sentence);
  const gloss = clampFont('gloss', settings.font_gloss);
  const furigana = clampFont('furigana', settings.furigana_scale);
  const gap = clampFont('gap', settings.furigana_gap);
  const outline = clampFont('outline', settings.text_outline_width);
  const shadow = clampFont('shadow', settings.text_shadow_size);

  return [
    ['--text-sentence', sentence === null ? '' : `${sentence}px`],
    ['--text-gloss', gloss === null ? '' : `${gloss}px`],
    ['--text-furigana', furigana === null ? '' : `${furigana / PERCENT}em`],
    [
      '--text-furigana-romaji',
      furigana === null ? '' : `${(furigana * ROMAJI_PER_MILLE) / PER_MILLE}em`,
    ],
    ['--furigana-gap', gap === null ? '' : `${gap}px`],
    ['--text-stroke-width', outline === null ? '' : `${outline}px`],
    ['--text-shadow-size', shadow === null ? '' : `${shadow}px`],
  ];
}

/**
 * KIND's default as the stylesheet states it, or `null` if it states none.
 *
 * The settings window shows these in its boxes on a first run, when nothing is
 * persisted yet. Read back rather than restated so `tokens.css` stays the one
 * place a default lives — a copy in TypeScript would drift the first time
 * somebody retunes the stylesheet.
 */
export function stylesheetDefault(kind: FontKind): number | null {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(TOKENS[kind]);
  const value = Number.parseFloat(raw);
  if (!Number.isFinite(value)) return null;
  // The furigana token is an em ratio; its control is a percentage.
  return clampFont(kind, kind === 'furigana' ? value * PERCENT : value);
}
