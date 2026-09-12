// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test } from 'vitest';
import { clampColour, colourVars, resolveColor, stylesheetDefault } from './text-colours';

describe('clampColour', () => {
  // `<input type="color">` always emits lowercase `#rrggbb`; a hand-edited
  // file may not, and the two must compare equal so a swatch is not marked
  // custom for a value that only differs in case.
  test('passes a hex colour through lowercased', () => {
    expect(clampColour('#AABBCC')).toBe('#aabbcc');
  });

  // The settings file is hand-editable, and the value lands in a CSS custom
  // property: anything that is not exactly `#rrggbb` is refused.
  test('rejects every other syntax', () => {
    expect(clampColour('#abc')).toBeNull();
    expect(clampColour('red')).toBeNull();
    expect(clampColour('#aabbcc; color: red')).toBeNull();
    expect(clampColour('')).toBeNull();
    expect(clampColour(0xaabbcc)).toBeNull();
    expect(clampColour(undefined)).toBeNull();
  });
});

describe('colourVars', () => {
  // An empty value is the instruction to remove the token: Reset, or a
  // snapshot with the key gone, hands the colour back to `tokens.css`.
  test('clears every token when no colour is persisted', () => {
    expect(colourVars({})).toEqual([
      ['--color-kanji', ''],
      ['--color-kana', ''],
      ['--color-particle', ''],
      ['--color-counter', ''],
      ['--color-furigana', ''],
      ['--color-unmatched', ''],
      ['--color-bg', ''],
    ]);
  });

  test('maps each persisted colour onto its token', () => {
    expect(colourVars({ colour_kana: '#112233', colour_furigana: '#445566' })).toEqual([
      ['--color-kanji', ''],
      ['--color-kana', '#112233'],
      ['--color-particle', ''],
      ['--color-counter', ''],
      ['--color-furigana', '#445566'],
      ['--color-unmatched', ''],
      ['--color-bg', ''],
    ]);
  });

  // The window background is a colour kind like the six text ones, so the
  // same clamp, the same token removal on Reset. `--color-bg-surface` is
  // mixed from it, so the opacity setting keeps working over a picked colour.
  test('maps the window background onto its token', () => {
    expect(colourVars({ colour_bg: '#102030' })).toContainEqual(['--color-bg', '#102030']);
  });

  test('treats a value that is not a hex colour as unset', () => {
    expect(colourVars({ colour_kanji: 'garbage' } as never)).toContainEqual(['--color-kanji', '']);
  });
});

describe('resolveColor', () => {
  // The test DOM has no canvas backend, so this exercises the regex fallback
  // `main.ts` has always carried for that case.
  test('reads an rgb() triple', () => {
    expect(resolveColor('rgb(17, 34, 51)')).toEqual([17, 34, 51]);
  });

  test('gives up on a colour it cannot parse without a canvas', () => {
    expect(resolveColor('')).toBeNull();
  });
});

describe('stylesheetDefault', () => {
  // The defaults belong to `tokens.css`, authored in `oklch()`, and a colour
  // input only accepts hex — so the token is resolved rather than restated.
  test('reads a token back as a hex colour', () => {
    document.documentElement.style.setProperty('--color-kanji', 'rgb(17, 34, 51)');

    expect(stylesheetDefault('kanji')).toBe('#112233');
  });

  test('returns null when the stylesheet does not define the token', () => {
    document.documentElement.style.removeProperty('--color-furigana');

    expect(stylesheetDefault('furigana')).toBeNull();
  });
});
