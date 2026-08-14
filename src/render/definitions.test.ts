import { describe, expect, test } from 'vitest';
import { renderDefinitions } from './definitions';
import fixture from '../fixtures/tokyo.json';
import type { ParseResult } from '../types';

const result = fixture as ParseResult;

describe('renderDefinitions', () => {
  test('renders one row per matched segment, skipping unmatched runs', () => {
    // The fixture has two matched segments and one unmatched run.
    expect(renderDefinitions(result).querySelectorAll('.def-row')).toHaveLength(2);
  });

  test('shows headword, reading, and glosses', () => {
    const first = renderDefinitions(result).querySelector('.def-row')!;
    expect(first.querySelector('.headword')?.textContent).toBe('東京');
    expect(first.querySelector('.reading')?.textContent).toBe('とうきょう');
    expect(first.textContent).toContain('Tokyo');
  });

  test('addresses each row by its segment start, matching the sentence chips', () => {
    const el = renderDefinitions(result);
    expect(el.querySelector('.def-row[data-start="0"]')).not.toBeNull();
    expect(el.querySelector('.def-row[data-start="2"]')).not.toBeNull();
  });

  test('omits the conjugation tag when there is no conjugation', () => {
    expect(renderDefinitions(result).querySelector('.conjugation')).toBeNull();
  });

  test('renders a conjugation tag when present', () => {
    const conjugated: ParseResult = {
      segments: [{
        start: 0, len: 4, surface: '言われた', reading: 'いわれた', matched: true,
        entries: [{
          headword: '言う', reading: 'いう', conjugation: 'Negative Formal Past',
          pos: ['v5u'],
          senses: [{ pos: ['v5u'], glosses: ['to say'], xrefs: [], misc: [], info: [] }],
          flags: ['primary'],
        }],
      }],
    };
    expect(renderDefinitions(conjugated).querySelector('.conjugation')?.textContent)
      .toBe('Negative Formal Past');
  });

  test('collapses alternative entries past the first', () => {
    const alternates: ParseResult = {
      segments: [{
        start: 0, len: 1, surface: '生', reading: 'せい', matched: true,
        entries: [
          { headword: '生', reading: 'せい', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['life'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'] },
          { headword: '生', reading: 'なま', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['raw'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'] },
        ],
      }],
    };
    const details = renderDefinitions(alternates).querySelector('details');
    expect(details).not.toBeNull();
    expect(details?.hasAttribute('open')).toBe(false);
    expect(details?.textContent).toContain('raw');
  });

  test('renders an empty result without throwing', () => {
    expect(renderDefinitions({ segments: [] }).querySelectorAll('.def-row')).toHaveLength(0);
  });
});
