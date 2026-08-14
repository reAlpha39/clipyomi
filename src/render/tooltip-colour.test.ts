import { describe, expect, test } from 'vitest';
import { CONJ_MARKER } from './tooltip-text';
import { colourLine } from './tooltip-colour';

/** The kinds a line produces, in order — the shape most assertions care about. */
function kinds(line: string): string[] {
  return colourLine(line).map((run) => run.kind);
}

describe('colourLine', () => {
  // MyToolTip.cpp:154-160 — a line flagged with \x01 is drawn entirely in the
  // conjugation colour, marker stripped.
  test('a marked line is one conjugation run with the marker removed', () => {
    expect(colourLine(`${CONJ_MARKER}Negative Formal Past`)).toEqual([
      { text: 'Negative Formal Past', kind: 'conj' },
    ]);
  });

  // A run containing kanji takes the kanji colour; an all-kana run does not.
  // This is why 消える reads red and 【きえる】 green in the reference shot.
  test('splits a headword line into a kanji run and a kana run', () => {
    expect(colourLine('消える【きえる】')).toEqual([
      { text: '消える', kind: 'kanji' },
      { text: '【きえる】', kind: 'kana' },
    ]);
  });

  // The break before 【 is what separates them: without it the whole string is
  // one Japanese run and the reading inherits the kanji colour.
  test('breaks before 【 even mid-run', () => {
    expect(kinds('旅だつ【たびだつ】')).toEqual(['kanji', 'kana']);
  });

  // MyToolTip.cpp:214-217 — everything from ( to its match is one paren run.
  test('a parenthesised span is one run', () => {
    expect(colourLine('(v1,vi) x')).toEqual([
      { text: '(v1,vi)', kind: 'paren' },
      { text: ' x', kind: 'text' },
    ]);
  });

  // Parenthesis colouring is tested BEFORE the Japanese check
  // (MyToolTip.cpp:224), so kanji inside parentheses does not win.
  test('parenthesis beats kanji', () => {
    expect(colourLine('(e.g. 寿司)')).toEqual([{ text: '(e.g. 寿司)', kind: 'paren' }]);
  });

  // The converse: kanji loose in gloss text takes the kanji colour — an
  // outcome semantic markup would get wrong.
  test('kanji loose in gloss text still takes the kanji colour', () => {
    expect(kinds('a type of 寿司 dish')).toEqual(['text', 'kanji', 'text']);
  });

  test('nested parentheses close at the outermost match', () => {
    expect(colourLine('(a (b) c)')).toEqual([{ text: '(a (b) c)', kind: 'paren' }]);
  });

  // An unclosed parenthesis runs to the end of the line rather than throwing
  // or silently dropping the rest of a gloss over a typo in JMdict.
  test('an unclosed parenthesis runs to end of line', () => {
    expect(colourLine('x (abc')).toEqual([
      { text: 'x ', kind: 'text' },
      { text: '(abc', kind: 'paren' },
    ]);
  });

  test('an empty line produces no runs', () => {
    expect(colourLine('')).toEqual([]);
  });
});
