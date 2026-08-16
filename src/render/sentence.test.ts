import { describe, expect, test } from 'vitest';
import { renderSentence } from './sentence';
import fixture from '../fixtures/tokyo.json';
import type { ParseResult } from '../types';

const result = fixture as ParseResult;

describe('renderSentence', () => {
  test('renders one element per segment, in order', () => {
    const spans = renderSentence(result).querySelectorAll('[data-start]');
    expect(spans).toHaveLength(3);
    expect(spans[0].textContent).toBe('東京');
    expect(spans[1].textContent).toBe('は');
    expect(spans[2].textContent).toBe('〜〜');
  });

  test('chips a matched segment and leaves an unmatched run unchipped', () => {
    const spans = renderSentence(result).querySelectorAll('[data-start]');
    expect(spans[0].classList.contains('chip')).toBe(true);
    expect(spans[2].classList.contains('chip')).toBe(false);
    expect(spans[2].classList.contains('unmatched')).toBe(true);
  });

  test('classes a particle by its flag, not by its surface', () => {
    const el = renderSentence(result);
    expect(el.querySelector('[data-start="2"]')?.classList.contains('particle')).toBe(true);
    expect(el.querySelector('[data-start="0"]')?.classList.contains('particle')).toBe(false);
  });

  test('carries the start offset so a chip can address its definition row', () => {
    const el = renderSentence(result);
    expect(el.querySelector('[data-start="0"]')).not.toBeNull();
    expect(el.querySelector('[data-start="3"]')).not.toBeNull();
  });

  test('renders an empty result without throwing', () => {
    expect(renderSentence({ segments: [] }).querySelectorAll('[data-start]')).toHaveLength(0);
  });

  test('only a matched chip is a real control; an unmatched run is not focusable', () => {
    const el = renderSentence(result);
    // A native <button> is focusable and fires click on Enter/Space with no
    // extra wiring; a plain <span> with no tabindex is not in the tab order.
    expect(el.querySelector('[data-start="0"]')?.tagName).toBe('BUTTON');
    expect(el.querySelector('[data-start="3"]')?.tagName).toBe('SPAN');
    expect(el.querySelector('[data-start="3"]')?.hasAttribute('tabindex')).toBe(false);
  });
});

describe('renderSentence with furigana modes', () => {
  const furiganaResult: ParseResult = {
    segments: [
      {
        start: 0,
        len: 2,
        surface: '東京',
        reading: 'とうきょう',
        matched: true,
        entries: [{ headword: '東京', reading: 'とうきょう', conjugation: null, pos: ['n'], senses: [], flags: ['primary'] }],
      },
      {
        start: 2,
        len: 1,
        surface: 'に',
        reading: 'に',
        matched: true,
        entries: [{ headword: 'に', reading: 'に', conjugation: null, pos: ['prt'], senses: [], flags: ['particle'] }],
      },
      {
        start: 3,
        len: 1,
        surface: '。',
        reading: null,
        matched: false,
        entries: [],
      },
    ],
  };

  test('mode none renders clean text without ruby', () => {
    const el = renderSentence(furiganaResult, 'none');
    const chips = el.querySelectorAll('.chip');
    expect(chips[0].textContent).toBe('東京');
    expect(chips[0].querySelector('ruby')).toBeNull();
    expect(chips[1].textContent).toBe('に');
  });

  test('mode hiragana renders ruby for kanji word only', () => {
    const el = renderSentence(furiganaResult, 'hiragana');
    const chips = el.querySelectorAll('.chip');
    const ruby0 = chips[0].querySelector('ruby');
    expect(ruby0).not.toBeNull();
    expect(ruby0?.querySelector('rt')?.textContent).toBe('とうきょう');
    expect(chips[1].querySelector('ruby')).toBeNull();
  });

  test('mode katakana renders katakana ruby for kanji word', () => {
    const el = renderSentence(furiganaResult, 'katakana');
    const chips = el.querySelectorAll('.chip');
    const ruby0 = chips[0].querySelector('ruby');
    expect(ruby0).not.toBeNull();
    expect(ruby0?.querySelector('rt')?.textContent).toBe('トウキョウ');
    expect(chips[1].querySelector('ruby')).toBeNull();
  });

  test('mode romaji renders romaji ruby for all matched segments', () => {
    const el = renderSentence(furiganaResult, 'romaji');
    const chips = el.querySelectorAll('.chip');
    const rt0 = chips[0].querySelector('rt.romaji');
    expect(rt0).not.toBeNull();
    expect(rt0?.textContent).toBe('toukyou');

    const rt1 = chips[1].querySelector('rt.romaji');
    expect(rt1).not.toBeNull();
    expect(rt1?.textContent).toBe('ni');
  });
});
