// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test } from 'vitest';
import { clampFamily, familyVars } from './text-fonts';

describe('clampFamily', () => {
  test('passes a family name through trimmed', () => {
    expect(clampFamily(' Yu Gothic ')).toBe('Yu Gothic');
    expect(clampFamily('Source Han Sans HW')).toBe('Source Han Sans HW');
    expect(clampFamily('M+ 1p-Regular_1.5')).toBe('M+ 1p-Regular_1.5');
  });

  // The name lands quoted inside a CSS custom property. The settings file is
  // hand-editable, so anything that could close the quote or the declaration
  // is refused rather than escaped.
  test('rejects names that could escape a quoted CSS string', () => {
    expect(clampFamily('Bad"Name')).toBeNull();
    expect(clampFamily('Bad\\Name')).toBeNull();
    expect(clampFamily('x; color: red')).toBeNull();
    expect(clampFamily('x{y}')).toBeNull();
    expect(clampFamily('a\tb')).toBeNull();
  });

  test('rejects blanks, over-long names and non-strings', () => {
    expect(clampFamily('')).toBeNull();
    expect(clampFamily('   ')).toBeNull();
    expect(clampFamily('a'.repeat(101))).toBeNull();
    expect(clampFamily(42)).toBeNull();
    expect(clampFamily(undefined)).toBeNull();
  });
});

describe('familyVars', () => {
  // Only the two tokens this window owns: the gloss family rides the popover
  // payload instead, that webview having its own `:root`.
  test('clears both tokens when no family is persisted', () => {
    expect(familyVars({})).toEqual([
      ['--font-cjk', ''],
      ['--font-furigana', ''],
    ]);
  });

  test('maps each persisted family onto its token, quoted', () => {
    expect(
      familyVars({ font_family_sentence: 'Yu Gothic', font_family_furigana: 'Meiryo' }),
    ).toEqual([
      ['--font-cjk', '"Yu Gothic"'],
      ['--font-furigana', '"Meiryo"'],
    ]);
  });

  test('treats a name that fails the guard as unset', () => {
    expect(familyVars({ font_family_sentence: 'x; y' } as never)).toContainEqual([
      '--font-cjk',
      '',
    ]);
  });
});
