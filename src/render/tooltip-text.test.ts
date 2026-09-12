// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test } from 'vitest';
import type { Entry, KanjiInfo } from '../types';
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
  // The headword line is `headword【reading】`; senses are
  // `(pos) (N) gloss/gloss`.
  test('renders a headword line and a numbered sense line', () => {
    expect(assembleTooltipText([entry()])).toBe(
      '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
    );
  });

  // Glosses join with "/", the tooltip's separator, not the pane's "; ".
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

  // `(P)` is the common-word marker, appended to the last sense only.
  test('appends (P) for a common entry', () => {
    expect(assembleTooltipText([entry({ flags: ['primary', 'common'] })])).toMatch(/\/\(P\)$/);
  });

  // The conjugation gets its own line, marked so the colouriser can find it.
  // See CONJ_MARKER for why byte 1 is the marker.
  test('puts a conjugation on its own marked line', () => {
    const text = assembleTooltipText([entry({ conjugation: 'Negative Formal Past' })]);
    expect(text.split('\n')[0]).toBe(`${CONJ_MARKER}Negative Formal Past`);
  });

  test('renders nothing for no entries', () => {
    expect(assembleTooltipText([])).toBe('');
  });

  describe('GlossFilters', () => {
    test('renders full sense metadata (pos, misc, info, xrefs, common) when unfiltered', () => {
      const e = entry({
        senses: [
          {
            pos: ['v5r'],
            glosses: ['to say', 'to utter'],
            misc: ['uk'],
            info: ['usually written in kana'],
            xrefs: ['言われる'],
          },
        ],
        flags: ['primary', 'common'],
      });
      expect(assembleTooltipText([e])).toBe(
        '消える【きえる】\n(v5r) (1) to say/to utter/(uk)/(usually written in kana)/(see 言われる)/(P)',
      );
    });

    test('hide_pos strips the pos tag from sense lines', () => {
      const e = entry({
        senses: [
          {
            pos: ['v5r'],
            glosses: ['to say'],
            misc: [],
            info: [],
            xrefs: [],
          },
        ],
      });
      expect(assembleTooltipText([e], { hide_pos: true })).toBe(
        '消える【きえる】\n(1) to say',
      );
    });

    test('hide_xrefs strips cross references', () => {
      const e = entry({
        senses: [
          {
            pos: ['v5r'],
            glosses: ['to say'],
            misc: [],
            info: [],
            xrefs: ['言われる'],
          },
        ],
      });
      expect(assembleTooltipText([e], { hide_xrefs: true })).toBe(
        '消える【きえる】\n(v5r) (1) to say',
      );
    });

    test('hide_usage strips misc and s_inf usage notes', () => {
      const e = entry({
        senses: [
          {
            pos: ['v5r'],
            glosses: ['to say'],
            misc: ['uk'],
            info: ['usually written in kana'],
            xrefs: [],
          },
        ],
      });
      expect(assembleTooltipText([e], { hide_usage: true })).toBe(
        '消える【きえる】\n(v5r) (1) to say',
      );
    });

    test('combined filters strip pos, xrefs, and usage simultaneously', () => {
      const e = entry({
        senses: [
          {
            pos: ['v5r'],
            glosses: ['to say', 'to utter'],
            misc: ['uk'],
            info: ['usually written in kana'],
            xrefs: ['言われる'],
          },
        ],
        flags: ['primary', 'common'],
      });
      expect(
        assembleTooltipText([e], { hide_pos: true, hide_xrefs: true, hide_usage: true }),
      ).toBe('消える【きえる】\n(1) to say/to utter/(P)');
    });

    test('omits pos prefix when sense pos is empty even if hide_pos is false', () => {
      const e = entry({
        senses: [
          {
            pos: [],
            glosses: ['particle'],
            misc: [],
            info: [],
            xrefs: [],
          },
        ],
      });
      expect(assembleTooltipText([e], { hide_pos: false })).toBe(
        '消える【きえる】\n(1) particle',
      );
    });
  });

  // Phase 3a: one blank line, then one flat line per kanji — literal, the on
  // and kun readings joined by the katakana middle dot, the meanings — so
  // `colourLine` colours the block as it colours everything else.
  describe('the kanji block', () => {
    const tokyo: KanjiInfo[] = [
      { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east', 'eastern'] },
      { literal: '京', on: ['キョウ', 'ケイ'], kun: ['みやこ'], meanings: ['capital', '10**16'] },
    ];

    test('appends one line per kanji after a blank line', () => {
      expect(assembleTooltipText([entry()], undefined, tokyo)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n' +
          '東  トウ・ひがし  east, eastern\n' +
          '京  キョウ・ケイ・みやこ  capital, 10**16',
      );
    });

    test('renders nothing extra when there are no kanji', () => {
      expect(assembleTooltipText([entry()], undefined, [])).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
      );
    });

    test('hide_kanji drops the block and the blank line with it', () => {
      expect(assembleTooltipText([entry()], { hide_kanji: true }, tokyo)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish',
      );
    });

    // A record with no readings or no meanings must not leave a doubled gap.
    test('omits an empty column rather than leaving a gap', () => {
      const bare: KanjiInfo[] = [{ literal: '々', on: [], kun: [], meanings: ['repetition mark'] }];
      expect(assembleTooltipText([entry()], undefined, bare)).toBe(
        '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n々  repetition mark',
      );
    });

    // Phase 3b: KANJIDIC2's <misc> numbers ride the same line as one
    // parenthesised aside, in the file's order — grade, strokes, frequency,
    // JLPT — with absent fields left out. The JLPT number is the former
    // four-level scale and is labelled as such, never mapped to N-levels.
    describe('the misc aside', () => {
      const head = '消える【きえる】\n(v1,vi) (1) to disappear/to vanish\n\n';
      const line = (k: KanjiInfo): string =>
        assembleTooltipText([entry()], undefined, [k]).slice(head.length);
      const east = tokyo[0];

      test('appends all four fields in KANJIDIC2 order', () => {
        expect(line({ ...east, grade: 2, strokes: 8, freq: 37, jlpt: 4 })).toBe(
          '東  トウ・ひがし  east, eastern  (grade 2, 8 strokes, freq 37, old JLPT 4)',
        );
      });

      test('leaves out absent fields, and the whole aside when none is present', () => {
        expect(line({ ...east, grade: 2, strokes: 8 })).toBe(
          '東  トウ・ひがし  east, eastern  (grade 2, 8 strokes)',
        );
        expect(line({ ...east, freq: 37 })).toBe('東  トウ・ひがし  east, eastern  (freq 37)');
        expect(line({ ...east, grade: null, strokes: null, freq: null, jlpt: null })).toBe(
          '東  トウ・ひがし  east, eastern',
        );
        expect(line(east)).toBe('東  トウ・ひがし  east, eastern');
      });

      test('one stroke is singular', () => {
        expect(line({ literal: '一', on: ['イチ'], kun: ['ひと.つ'], meanings: ['one'], strokes: 1 })).toBe(
          '一  イチ・ひと.つ  one  (1 stroke)',
        );
      });

      // The DTD's codes: 1–6 are school years; 8 is the remaining jōyō kanji,
      // learned in secondary school; 9 and 10 are jinmeiyō (name) kanji. An
      // unknown code is shown raw rather than guessed at.
      test('names the grade codes that are not school years', () => {
        expect(line({ ...east, grade: 8 })).toBe('東  トウ・ひがし  east, eastern  (secondary school)');
        expect(line({ ...east, grade: 9 })).toBe('東  トウ・ひがし  east, eastern  (name kanji)');
        expect(line({ ...east, grade: 10 })).toBe('東  トウ・ひがし  east, eastern  (name kanji)');
        expect(line({ ...east, grade: 7 })).toBe('東  トウ・ひがし  east, eastern  (grade 7)');
      });

      test('a record with nothing but an aside still has no doubled gap', () => {
        expect(line({ literal: '々', on: [], kun: [], meanings: [], strokes: 3 })).toBe('々  (3 strokes)');
      });

      test('hide_kanji hides the aside with the line it is on', () => {
        expect(
          assembleTooltipText([entry()], { hide_kanji: true }, [{ ...east, grade: 2, strokes: 8 }]),
        ).toBe('消える【きえる】\n(v1,vi) (1) to disappear/to vanish');
      });
    });
  });
});

