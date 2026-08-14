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
