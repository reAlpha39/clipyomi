// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Settings } from './types';

/**
 * The three families the user can pick. `gloss` has no token in this window:
 * it rides the popover payload, that webview having its own `:root`.
 */
export type FamilyKind = 'sentence' | 'furigana' | 'gloss';

/** The settings key each kind is persisted under. */
export const FAMILY_KEYS: Record<FamilyKind, keyof Settings> = {
  sentence: 'font_family_sentence',
  furigana: 'font_family_furigana',
  gloss: 'font_family_gloss',
};

/** Longer than any family name an OS ships; shorter than a pasted paragraph. */
const MAX_FAMILY_LENGTH = 100;

/**
 * Characters that could end the quoted string or the declaration the name is
 * written into, plus control characters. Refused, not escaped: the list of
 * fonts the user picks from never contains them, so a name that does came
 * from a hand-edited file.
 */
const UNSAFE_IN_FAMILY = /["\\;{}\p{Cc}]/u;

/** VALUE as a trimmed family name, or `null` if it fails the guard. */
export function clampFamily(value: unknown): string | null {
  if (typeof value !== 'string') return null;
  const name = value.trim();
  if (name === '' || name.length > MAX_FAMILY_LENGTH || UNSAFE_IN_FAMILY.test(name)) return null;
  return name;
}

/** NAME as the stylesheet wants it: quoted, so spaces and digits are literal. */
export function quoteFamily(name: string): string {
  return `"${name}"`;
}

/**
 * The custom properties SETTINGS asks for, as `[token, value]` pairs.
 *
 * An absent or refused family produces an empty value: the caller removes the
 * token and `typography.css` owns the stack again, exactly as `fontVars` does
 * for the sizes. `--font-cjk` is the token `.sentence` already reads, so no
 * new token is needed for the main text.
 */
export function familyVars(settings: Partial<Settings>): Array<[string, string]> {
  const sentence = clampFamily(settings.font_family_sentence);
  const furigana = clampFamily(settings.font_family_furigana);
  return [
    ['--font-cjk', sentence === null ? '' : quoteFamily(sentence)],
    ['--font-furigana', furigana === null ? '' : quoteFamily(furigana)],
  ];
}
