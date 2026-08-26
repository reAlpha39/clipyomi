import type { ChromeMode } from './types';

/**
 * Bounds for the opacity control.
 *
 * The floor is 0, not ta-old's 20/255: that clamp existed because uniform
 * whole-window alpha at zero took the text with it, and per-pixel alpha does
 * not — at 0 % the sentence, its furigana and the chips are all still fully
 * opaque.
 *
 * These are the guard, not just the `min`/`max` on the input: the settings
 * file is hand-editable, so every value arriving from it is untrusted and
 * passes through `clampAlpha` before it reaches a stylesheet.
 */
export const ALPHA_LIMITS = { min: 0, max: 100 } as const;

/** The token the alpha is bound to. */
const TOKEN = '--surface-alpha';

/** Percent to the 0..1 ratio the stylesheet wants. */
const PERCENT = 100;

/** VALUE pinned into range, or `null` if it is not a finite number. */
export function clampAlpha(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null;
  return Math.min(ALPHA_LIMITS.max, Math.max(ALPHA_LIMITS.min, Math.round(value)));
}

/**
 * The custom property ALPHA asks for under CHROME, as a `[token, value]` pair.
 *
 * `native` forces full opacity whatever the stored value: the OS draws a solid
 * title bar in that mode, and a see-through body underneath it is a rendering
 * artifact. The stored value is never rewritten, so leaving native restores
 * exactly what the user chose.
 *
 * An unset alpha produces an empty value rather than `'1'`, matching
 * `fontVars`: the caller removes the property and `tokens.css` owns the
 * default again. This module never restates a default of its own.
 */
export function surfaceAlphaVar(
  alpha: number | null,
  chrome: ChromeMode | undefined,
): [string, string] {
  if (chrome === 'native') return [TOKEN, '1'];
  if (alpha === null) return [TOKEN, ''];
  return [TOKEN, String(alpha / PERCENT)];
}
