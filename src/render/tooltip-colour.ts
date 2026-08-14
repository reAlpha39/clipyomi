import { CONJ_MARKER } from './tooltip-text';

export type RunKind = 'conj' | 'paren' | 'kana' | 'kanji' | 'text';

export interface Run {
  text: string;
  kind: RunKind;
}

/** Hiragana, katakana, and the prolonged sound mark. */
const KANA = /[぀-ヿ]/;
/** 【 and 】, which ta-old treats as Japanese and breaks before. */
const OPEN_LENTICULAR = '【';
const BRACKET = /[【】]/;
/**
 * CJK ideographs, including extension A and the compatibility block, plus
 * the kanji iteration mark 々 — ta-old special-cases it explicitly
 * (`StringUtil.cpp:531-535`) rather than including it in a range.
 */
const CJK = /[㐀-䶿一-鿿豈-﫿々]/;

function isJapanese(ch: string): boolean {
  return KANA.test(ch) || BRACKET.test(ch) || CJK.test(ch);
}

/** Index just past the `)` matching the `(` at START, or the line's end. */
function closeParen(line: string, start: number): number {
  let depth = 0;
  for (let i = start; i < line.length; i += 1) {
    if (line[i] === '(') depth += 1;
    else if (line[i] === ')') {
      depth -= 1;
      if (depth === 0) return i + 1;
    }
  }
  // Unclosed: ta-old's FindCloseBrace returns null and the run simply extends.
  // Truncating instead would drop the rest of a gloss over a typo in JMdict.
  return line.length;
}

/** Index just past a Japanese run starting at START. */
function endOfJapanese(line: string, start: number): number {
  let i = start;
  while (i < line.length && isJapanese(line[i])) {
    // Break BEFORE a 【 that is not the run's first character
    // (MyToolTip.cpp:216). Without this, 消える【きえる】 is one run and the
    // reading inherits the kanji colour instead of the kana one.
    if (i > start && line[i] === OPEN_LENTICULAR) break;
    i += 1;
  }
  return i;
}

/**
 * One line of assembled tooltip text, split into coloured runs.
 *
 * Ported from `MyDrawText` (`MyToolTip.cpp:125-268`), which colours by what
 * characters *are* rather than by what they mean: it has no notion of a
 * headword or a part of speech. That is why `(v1,vi)`, `(1)` and `(P)` all
 * render alike — they are parentheses — and it is why this is a port of the
 * rule rather than of its output.
 */
export function colourLine(line: string): Run[] {
  if (line.startsWith(CONJ_MARKER)) {
    const text = line.slice(CONJ_MARKER.length);
    return text === '' ? [] : [{ text, kind: 'conj' }];
  }

  const runs: Run[] = [];
  let i = 0;

  while (i < line.length) {
    if (line[i] === '(') {
      // Checked first, so kanji inside parentheses stays parenthesis-coloured.
      const end = closeParen(line, i);
      runs.push({ text: line.slice(i, end), kind: 'paren' });
      i = end;
      continue;
    }

    if (isJapanese(line[i])) {
      const end = endOfJapanese(line, i);
      const text = line.slice(i, end);
      runs.push({ text, kind: CJK.test(text) ? 'kanji' : 'kana' });
      i = end;
      continue;
    }

    let end = i;
    while (end < line.length && line[end] !== '(' && !isJapanese(line[end])) end += 1;
    runs.push({ text: line.slice(i, end), kind: 'text' });
    i = end;
  }

  return runs;
}
