// SPDX-License-Identifier: MIT OR Apache-2.0

import { CONJ_MARKER } from './tooltip-text';

/**
 * Lexical colouring for one line of assembled tooltip text.
 *
 * `colourLine` walks a line and splits it into runs the renderer can paint
 * differently: the conjugation label, parenthesised asides, Japanese text
 * (kanji vs. kana), and everything else. It is lexical, not semantic — it
 * never looks at where the text came from, only at the characters
 * themselves, which is what lets `assembleTooltipText` stay a flat string.
 */

export type RunKind = 'conj' | 'paren' | 'kana' | 'kanji' | 'text';

export interface Run {
  text: string;
  kind: RunKind;
}

/**
 * The kanji iteration (repetition) mark, e.g. the second character of 時々.
 * It stands in for "repeat the previous character" rather than being an
 * ideograph itself, so it falls outside every Unicode ideograph range and
 * has to be special-cased into the kanji test below.
 */
const ITERATION_MARK = '々';

/** Lenticular brackets, used to wrap a reading after a headword: 【きえる】. */
const LENTICULAR_OPEN = '【';
const LENTICULAR_CLOSE = '】';

/**
 * A CJK ideograph ("kanji"), across the ranges Unicode actually assigns
 * them in, plus the iteration mark (see above). Deliberately broader than
 * the handful of ranges any single test happens to exercise.
 */
export function isIdeograph(ch: string): boolean {
  if (ch === ITERATION_MARK) return true;
  const cp = ch.codePointAt(0);
  if (cp === undefined) return false;
  return (
    (cp >= 0x4e00 && cp <= 0x9fff) || // CJK Unified Ideographs
    (cp >= 0x3400 && cp <= 0x4dbf) || // CJK Unified Ideographs Extension A
    (cp >= 0xf900 && cp <= 0xfaff) || // CJK Compatibility Ideographs
    (cp >= 0x20000 && cp <= 0x2fa1f) // Supplementary Ideographic Plane (Ext. B and on)
  );
}

/** Hiragana or katakana. */
function isKana(ch: string): boolean {
  const cp = ch.codePointAt(0);
  if (cp === undefined) return false;
  return (cp >= 0x3040 && cp <= 0x309f) || (cp >= 0x30a0 && cp <= 0x30ff);
}

/**
 * Everything a "Japanese" run may be built from: kana, kanji, and the
 * lenticular brackets that wrap a reading. The brackets carry no meaning of
 * their own here — they just need to ride along with the kana run they
 * bracket instead of falling out as stray `text`.
 */
function isJapaneseChar(ch: string): boolean {
  return isKana(ch) || isIdeograph(ch) || ch === LENTICULAR_OPEN || ch === LENTICULAR_CLOSE;
}

/** Where a run of Japanese characters starting at `chars[start]` ends. */
function japaneseRunEnd(chars: string[], start: number): number {
  let end = start + 1;
  while (end < chars.length) {
    const ch = chars[end];
    // Break *before* an opening bracket that isn't the run's first
    // character: a headword immediately followed by its bracketed reading
    // (消える【きえる】) is two runs, not one, so the reading can be coloured
    // on its own kana/kanji merits instead of inheriting the headword's.
    if (ch === LENTICULAR_OPEN) break;
    if (!isJapaneseChar(ch)) break;
    end++;
  }
  return end;
}

/** 'kanji' if any character in the run is an ideograph, else 'kana'. */
function japaneseRunKind(chars: string[], start: number, end: number): RunKind {
  for (let i = start; i < end; i++) {
    if (isIdeograph(chars[i])) return 'kanji';
  }
  return 'kana';
}

/** Where the parenthesised span starting at `chars[start]` (a `(`) closes. */
function parenRunEnd(chars: string[], start: number): number {
  let depth = 0;
  for (let i = start; i < chars.length; i++) {
    if (chars[i] === '(') depth++;
    else if (chars[i] === ')') {
      depth--;
      if (depth === 0) return i + 1;
    }
  }
  // Unclosed: run to end of line rather than throwing or dropping the rest
  // of the gloss over a stray '(' left by a typo in the source data.
  return chars.length;
}

/**
 * Split one line of tooltip text into colour runs.
 *
 * A line flagged with the conjugation marker is entirely one `conj` run.
 * Otherwise the line is scanned left to right; at each position a `(`
 * starts a parenthesised run (checked first, so kanji inside parentheses
 * stays `paren`), a Japanese character starts a kana/kanji run, and
 * anything else accumulates into `text` until one of those takes over.
 */
export function colourLine(line: string): Run[] {
  if (line.startsWith(CONJ_MARKER)) {
    const text = line.slice(CONJ_MARKER.length);
    return text.length > 0 ? [{ text, kind: 'conj' }] : [];
  }

  // Split into Unicode code points, not UTF-16 code units, so a
  // supplementary-plane ideograph (a surrogate pair) is never sliced in half.
  const chars = Array.from(line);
  const runs: Run[] = [];
  let textStart = 0;
  let i = 0;

  const flushText = (end: number): void => {
    if (end > textStart) runs.push({ text: chars.slice(textStart, end).join(''), kind: 'text' });
  };

  while (i < chars.length) {
    const ch = chars[i];

    if (ch === '(') {
      flushText(i);
      const end = parenRunEnd(chars, i);
      runs.push({ text: chars.slice(i, end).join(''), kind: 'paren' });
      i = end;
      textStart = end;
      continue;
    }

    if (isJapaneseChar(ch)) {
      flushText(i);
      const end = japaneseRunEnd(chars, i);
      runs.push({ text: chars.slice(i, end).join(''), kind: japaneseRunKind(chars, i, end) });
      i = end;
      textStart = end;
      continue;
    }

    i++;
  }

  flushText(chars.length);
  return runs;
}
