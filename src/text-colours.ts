// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Settings } from './types';

/** The five text colours the user can move. */
export type ColourKind = 'kanji' | 'kana' | 'particle' | 'counter' | 'furigana' | 'unmatched';

export const COLOUR_KINDS: readonly ColourKind[] = [
  'kanji',
  'kana',
  'particle',
  'counter',
  'furigana',
  'unmatched',
];

/** The token each kind is bound to. */
const TOKENS: Record<ColourKind, string> = {
  kanji: '--color-kanji',
  kana: '--color-kana',
  particle: '--color-particle',
  counter: '--color-counter',
  furigana: '--color-furigana',
  unmatched: '--color-unmatched',
};

/** The settings key each kind is persisted under. */
export const COLOUR_KEYS: Record<ColourKind, keyof Settings> = {
  kanji: 'colour_kanji',
  kana: 'colour_kana',
  particle: 'colour_particle',
  counter: 'colour_counter',
  furigana: 'colour_furigana',
  unmatched: 'colour_unmatched',
};

/**
 * Exactly what `<input type="color">` emits, and nothing else.
 *
 * This is the guard, not a convenience: the settings file is hand-editable
 * and the value lands in a CSS custom property, so every value arriving from
 * it is untrusted and passes through here before it reaches a stylesheet.
 */
const HEX_COLOUR = /^#[0-9a-f]{6}$/i;

/** VALUE as a lowercase `#rrggbb`, or `null` if it is anything else. */
export function clampColour(value: unknown): string | null {
  if (typeof value !== 'string' || !HEX_COLOUR.test(value)) return null;
  return value.toLowerCase();
}

/**
 * The custom properties SETTINGS asks for, as `[token, value]` pairs.
 *
 * A colour the settings file does not carry produces an empty value rather
 * than no pair at all: Reset sends a snapshot with the key gone, and the token
 * then has to be removed so `tokens.css` owns that colour again. This module
 * never restates a default of its own.
 */
export function colourVars(settings: Partial<Settings>): Array<[string, string]> {
  return COLOUR_KINDS.map((kind) => [TOKENS[kind], clampColour(settings[COLOUR_KEYS[kind]]) ?? '']);
}

/**
 * A CSS colour as an sRGB triple, whatever syntax it arrives in.
 *
 * Painted onto a 1x1 canvas and read back rather than parsed: Chromium
 * serializes a computed colour in the space it was authored in, so the
 * `oklch()` tokens come back as `oklch(0.18 0 0)` and string parsing silently
 * gave up on every one of them — the erase kept a colour guessed from the
 * system theme, and a peek showed a strip of the wrong theme.
 *
 * The regex is the fallback for environments with no canvas backend, which
 * includes the test DOM.
 *
 * That fallback can no longer match a real computed background now that the
 * alpha token is in play: with `--color-bg-surface` derived through
 * `color-mix()`, Chromium serializes the computed value as
 * `color(srgb r g b / a)`, not `rgba(...)` — the pattern this regex expects.
 * If `getContext('2d')` ever returned null (headless environments without a
 * canvas backend, mainly), the fallback would silently fail to parse and
 * `reportBackground` would give up (`rgb === null`) rather than merely
 * reporting a stale colour, letting the CSS token and `BACKGROUND_OPAQUE`
 * diverge with no error. Worth knowing; not worth code, since every
 * environment this app actually runs in has a canvas backend.
 */
export function resolveColor(css: string): [number, number, number] | null {
  const ctx = document.createElement('canvas').getContext('2d');
  if (ctx !== null) {
    ctx.fillStyle = css;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return [r, g, b];
  }
  const rgb = /^rgba?\((\d+),\s*(\d+),\s*(\d+)/.exec(css);
  return rgb === null ? null : [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])];
}

const HEX_RADIX = 16;

function toHex([r, g, b]: [number, number, number]): string {
  return '#' + [r, g, b].map((c) => c.toString(HEX_RADIX).padStart(2, '0')).join('');
}

/**
 * KIND's default as the stylesheet states it, as a hex colour, or `null` if
 * it states none.
 *
 * The settings window shows these in its swatches while nothing is persisted.
 * Read back rather than restated so `tokens.css` stays the one place a default
 * lives — and resolved, because the tokens are authored in `oklch()` and a
 * colour input only accepts `#rrggbb`.
 */
export function stylesheetDefault(kind: ColourKind): string | null {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(TOKENS[kind]).trim();
  if (raw === '') return null;
  const rgb = resolveColor(raw);
  return rgb === null ? null : toHex(rgb);
}
