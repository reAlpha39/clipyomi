// SPDX-License-Identifier: MIT OR Apache-2.0

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
      ['--furigana-gap', ''],
      ['--text-stroke-width', ''],
      ['--text-shadow-size', ''],
    ]);
  });

  // The gap between the furigana and the text it annotates: a size of its
  // own, in px, not a ratio, so a large sentence and a small one can share a
  // hairline.
  test('maps the furigana gap onto its token in px', () => {
    expect(fontVars({ furigana_gap: 6 })).toContainEqual(['--furigana-gap', '6px']);
  });

  // How thick the outline stroke is drawn. Half of it falls outside the
  // glyph, so the default 2px reads as a 1px edge.
  test('maps the outline width onto its token in px', () => {
    expect(fontVars({ text_outline_width: 3 })).toContainEqual(['--text-stroke-width', '3px']);
  });

  // One knob for the drop shadow: the rule derives offset and blur from it.
  test('maps the shadow size onto its token in px', () => {
    expect(fontVars({ text_shadow_size: 5 })).toContainEqual(['--text-shadow-size', '5px']);
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
    expect(
      fontVars({
        font_sentence: 24,
        font_gloss: 14,
        furigana_scale: 50,
        furigana_gap: 4,
        text_outline_width: 3,
        text_shadow_size: 5,
      }),
    ).toEqual([
      ['--text-sentence', '24px'],
      ['--text-gloss', '14px'],
      ['--text-furigana', '0.5em'],
      ['--text-furigana-romaji', '0.45em'],
      ['--furigana-gap', '4px'],
      ['--text-stroke-width', '3px'],
      ['--text-shadow-size', '5px'],
    ]);
  });

  // The settings file is hand-editable, so the values arriving here are
  // untrusted input, not just whatever the number inputs allowed.
  test('clamps sizes read from a hand-edited settings file', () => {
    expect(
      fontVars({
        font_sentence: 999,
        font_gloss: 2,
        furigana_scale: 5,
        furigana_gap: 99,
        text_outline_width: 0,
        text_shadow_size: 99,
      }),
    ).toEqual([
      ['--text-sentence', '48px'],
      ['--text-gloss', '10px'],
      ['--text-furigana', '0.3em'],
      ['--text-furigana-romaji', '0.27em'],
      ['--furigana-gap', '24px'],
      ['--text-stroke-width', '1px'],
      ['--text-shadow-size', '12px'],
    ]);
  });

  test('treats values that are not finite numbers as unset', () => {
    expect(fontVars({ font_sentence: Number.NaN } as never)).toContainEqual(['--text-sentence', '']);
    expect(fontVars({ font_gloss: '18' } as never)).toContainEqual(['--text-gloss', '']);
    expect(fontVars({ furigana_scale: null } as never)).toContainEqual(['--text-furigana', '']);
  });
});

describe('clampFont', () => {
  // Zero is inside the shadow's range and means "no shadow", which is not the
  // same as an empty box: the box hands the stylesheet default back instead.
  test('keeps a zero shadow size rather than reading it as unset', () => {
    expect(clampFont('shadow', 0)).toBe(0);
    expect(fontVars({ text_shadow_size: 0 })).toContainEqual(['--text-shadow-size', '0px']);
  });

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
