import { describe, expect, test } from 'vitest';
import { clampFont, fontVars, stylesheetDefault } from './font-sizes';

describe('fontVars', () => {
  // An empty value is the instruction to remove the token, which is what
  // clearing a size box has to do: the stylesheet default takes over again.
  test('clears every token when no size is persisted', () => {
    expect(fontVars({})).toEqual([
      ['--text-sentence', ''],
      ['--text-gloss', ''],
      ['--text-furigana', ''],
      ['--text-furigana-romaji', ''],
    ]);
  });

  test('maps the sentence size onto its token in px', () => {
    expect(fontVars({ font_sentence: 30 })).toContainEqual(['--text-sentence', '30px']);
  });

  test('maps the gloss size onto its token in px', () => {
    expect(fontVars({ font_gloss: 18 })).toContainEqual(['--text-gloss', '18px']);
  });

  // Furigana is a ratio of the sentence, not a size of its own, so the
  // sentence size can move without dragging furigana out of proportion.
  test('maps the furigana percentage onto an em ratio', () => {
    expect(fontVars({ furigana_scale: 70 })).toContainEqual(['--text-furigana', '0.7em']);
  });

  // The romaji reading is set slightly smaller than kana furigana — 0.52em
  // against 0.58em in the stylesheet. That 0.9 relationship survives scaling.
  test('scales the romaji reading to nine tenths of the furigana ratio', () => {
    expect(fontVars({ furigana_scale: 58 })).toContainEqual(['--text-furigana-romaji', '0.522em']);
  });

  test('emits every persisted size at once', () => {
    expect(fontVars({ font_sentence: 24, font_gloss: 14, furigana_scale: 50 })).toEqual([
      ['--text-sentence', '24px'],
      ['--text-gloss', '14px'],
      ['--text-furigana', '0.5em'],
      ['--text-furigana-romaji', '0.45em'],
    ]);
  });

  // The settings file is hand-editable, so the values arriving here are
  // untrusted input, not just whatever the number inputs allowed.
  test('clamps sizes read from a hand-edited settings file', () => {
    expect(fontVars({ font_sentence: 999, font_gloss: 2, furigana_scale: 5 })).toEqual([
      ['--text-sentence', '48px'],
      ['--text-gloss', '10px'],
      ['--text-furigana', '0.3em'],
      ['--text-furigana-romaji', '0.27em'],
    ]);
  });

  test('treats values that are not finite numbers as unset', () => {
    expect(fontVars({ font_sentence: Number.NaN } as never)).toContainEqual(['--text-sentence', '']);
    expect(fontVars({ font_gloss: '18' } as never)).toContainEqual(['--text-gloss', '']);
    expect(fontVars({ furigana_scale: null } as never)).toContainEqual(['--text-furigana', '']);
  });
});

describe('clampFont', () => {
  test('passes an in-range value through', () => {
    expect(clampFont('sentence', 24)).toBe(24);
  });

  test('pins a value to the nearer bound', () => {
    expect(clampFont('gloss', 1)).toBe(10);
    expect(clampFont('gloss', 99)).toBe(28);
  });

  test('rounds a fractional value', () => {
    expect(clampFont('gloss', 18.6)).toBe(19);
  });

  test('returns null for anything that is not a finite number', () => {
    expect(clampFont('furigana', Number.NaN)).toBeNull();
    expect(clampFont('furigana', undefined)).toBeNull();
  });
});

describe('stylesheetDefault', () => {
  // The defaults belong to `tokens.css`. The settings window reads them back
  // rather than restating them, so there is still one place a default lives.
  test('reads a px token as a number', () => {
    document.documentElement.style.setProperty('--text-sentence', '24px');

    expect(stylesheetDefault('sentence')).toBe(24);
  });

  test('reads the furigana em ratio back as a percentage', () => {
    document.documentElement.style.setProperty('--text-furigana', '0.58em');

    expect(stylesheetDefault('furigana')).toBe(58);
  });

  test('returns null when the stylesheet does not define the token', () => {
    document.documentElement.style.removeProperty('--text-gloss');

    expect(stylesheetDefault('gloss')).toBeNull();
  });
});
