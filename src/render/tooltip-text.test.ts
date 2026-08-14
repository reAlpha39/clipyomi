import { describe, expect, test } from 'vitest';
import type { Entry } from '../types';
import { assembleTooltipText, CONJ_MARKER } from './tooltip-text';

function entry(overrides: Partial<Entry> = {}): Entry {
  return {
    headword: '消える',
    reading: 'きえる',
    conjugation: null,
    pos: ['v1', 'vi'],
    senses: [
      { pos: ['v1', 'vi'], glosses: ['to disappear', 'to vanish'], xrefs: [], misc: [], info: [] },
    ],
    flags: ['primary'],
    ...overrides,
  };
}

describe('assembleTooltipText', () => {
  // The headword line is ta-old's `headword【reading】` from
  // DictionaryUtil.cpp:57-77; senses are `(pos) (N) gloss/gloss`.
  test('renders a headword line and a numbered sense line', () => {
    expect(assembleTooltipText([entry()])).toBe(
      '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
    );
  });

  // Glosses join with "/" — ta-old's separator — not the pane's "; ".
  test('joins glosses with a slash', () => {
    const e = entry({
      senses: [{ pos: ['n'], glosses: ['a', 'b', 'c'], xrefs: [], misc: [], info: [] }],
    });
    expect(assembleTooltipText([e])).toContain('(n) (1) a/b/c');
  });

  // Numbering is per entry and starts at 1, matching the reference screenshot.
  test('numbers senses from one, per entry', () => {
    const e = entry({
      senses: [
        { pos: ['v1'], glosses: ['first'], xrefs: [], misc: [], info: [] },
        { pos: ['v1'], glosses: ['second'], xrefs: [], misc: [], info: [] },
      ],
    });
    expect(assembleTooltipText([e])).toContain('(v1) (2) second');
  });

  // Every match stacked is the point of this phase: the pane was previously
  // the only place alternates appeared at all.
  test('stacks every entry', () => {
    const text = assembleTooltipText([entry(), entry({ headword: '来る', reading: 'くる' })]);
    expect(text).toContain('消える【きえる】');
    expect(text).toContain('来る【くる】');
  });

  // A kana-only word has no separate reading to bracket.
  test('omits the bracket when there is no reading', () => {
    expect(assembleTooltipText([entry({ headword: 'ある', reading: null })])).toContain('ある\n');
  });

  // `(P)` is ta-old's common-word marker, appended to the last sense only.
  test('appends (P) for a common entry', () => {
    expect(assembleTooltipText([entry({ flags: ['primary', 'common'] })])).toMatch(/\/\(P\)$/);
  });

  // The conjugation gets its own line, marked so the colouriser can find it —
  // DictionaryUtil.cpp:46 sets `temp[0] = 1` for exactly this purpose.
  test('puts a conjugation on its own marked line', () => {
    const text = assembleTooltipText([entry({ conjugation: 'Negative Formal Past' })]);
    expect(text.split('\n')[0]).toBe(`${CONJ_MARKER}Negative Formal Past`);
  });

  test('renders nothing for no entries', () => {
    expect(assembleTooltipText([])).toBe('');
  });
});
