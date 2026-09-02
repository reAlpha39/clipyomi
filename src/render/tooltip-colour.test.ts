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

  // The kanji iteration mark 々 is special-cased into the kanji test, since no
  // Unicode range covers it. Without it, 時々 splits into a kanji run and a
  // stray text run instead of staying one kanji run.
  test('kanji iteration mark 々 stays in the kanji run', () => {
    expect(colourLine('時々')).toEqual([{ text: '時々', kind: 'kanji' }]);
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

  // Assumption: katakana counts as kana just like hiragana does — the ten
  // cases above only ever exercise hiragana for the non-kanji case.
  test('an all-katakana run is kana, not text', () => {
    expect(colourLine('ケーキ')).toEqual([{ text: 'ケーキ', kind: 'kana' }]);
  });

  // Assumption: the break before 【 is a property of the run boundary, not a
  // side effect of one side being a kanji run — an all-kana headword still
  // splits into two runs at its bracketed reading.
  test('breaks before 【 even when neither side contains kanji', () => {
    expect(colourLine('たつ【たつ】')).toEqual([
      { text: 'たつ', kind: 'kana' },
      { text: '【たつ】', kind: 'kana' },
    ]);
  });

  // Assumption: the lenticular brackets are themselves part of a Japanese
  // run (never `text`), including when the run opens with one rather than
  // being reached mid-run after a headword.
  test('a bracketed reading with no preceding headword is one kana run', () => {
    expect(colourLine('【たとえば】')).toEqual([{ text: '【たとえば】', kind: 'kana' }]);
  });

  // Assumption: "the standard Unicode ideograph ranges" means more than the
  // BMP block the other cases happen to use — a supplementary-plane
  // ideograph (here U+20000, in CJK Unified Ideographs Extension B) must
  // still count as kanji, and must survive being a UTF-16 surrogate pair.
  test('a supplementary-plane ideograph counts as kanji', () => {
    expect(colourLine('\u{20000}')).toEqual([{ text: '\u{20000}', kind: 'kanji' }]);
  });
});
