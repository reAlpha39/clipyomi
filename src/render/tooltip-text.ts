// SPDX-License-Identifier: MIT OR Apache-2.0

import type { Entry, GlossFilters, KanjiInfo } from '../types';

/**
 * Prefix marking a line as the conjugation label.
 *
 * The original tooltip used the literal control character `\x01` for this, and
 * the colouriser looks for it the same way. Kept as a control character rather
 * than a friendlier sentinel precisely because it cannot occur in dictionary
 * text.
 */
export const CONJ_MARKER = '\u0001';

/** One entry's block: an optional conjugation line, a headword line, then senses. */
function entryLines(entry: Entry, filters?: GlossFilters): string[] {
  const lines: string[] = [];

  if (entry.conjugation !== null) lines.push(`${CONJ_MARKER}${entry.conjugation}`);

  // `headword【reading】`, with the bracket omitted for a kana-only word where
  // the surface already is the reading.
  lines.push(entry.reading === null ? entry.headword : `${entry.headword}【${entry.reading}】`);

  const common = entry.flags.includes('common');
  entry.senses.forEach((sense, i) => {
    // Glosses join with "/", the tooltip's separator; the pane uses "; ". The
    // two surfaces render differently on purpose from this phase onward.
    const parts: string[] = [...sense.glosses];

    if (!filters?.hide_usage) {
      for (const m of sense.misc) {
        parts.push(`(${m})`);
      }
      for (const inf of sense.info) {
        parts.push(`(${inf})`);
      }
    }

    if (!filters?.hide_xrefs) {
      for (const xr of sense.xrefs) {
        parts.push(`(see ${xr})`);
      }
    }

    if (common && i === entry.senses.length - 1) {
      parts.push('(P)');
    }

    const posPrefix =
      !filters?.hide_pos && sense.pos.length > 0 ? `(${sense.pos.join(',')}) ` : '';
    const glosses = parts.join('/');
    lines.push(`${posPrefix}(${i + 1}) ${glosses}`);
  });

  return lines;
}

/** A misc field that is present: the backend sends a number or `null`; an older payload sends nothing. */
function has(value: number | null | undefined): value is number {
  return typeof value === 'number';
}

/**
 * `grade N` for a school year. KANJIDIC2's DTD defines 8 as the remaining
 * jōyō kanji learned in junior high school and 9–10 as jinmeiyō (name)
 * kanji, so those codes are named rather than numbered; any other code is
 * shown raw rather than guessed at.
 */
function gradeLabel(grade: number): string {
  if (grade === 8) return 'secondary school';
  if (grade === 9 || grade === 10) return 'name kanji';
  return `grade ${grade}`;
}

/**
 * The parenthesised aside carrying KANJIDIC2's `<misc>` numbers, in the
 * file's own order; absent fields are left out and an aside with nothing in
 * it is `''`, so `kanjiLine` drops the column. Parentheses because
 * `colourLine` already paints a `(…)` span as a `paren` run. The JLPT level
 * is the former four-level scale and is labelled as such (design §1.2).
 */
function kanjiAside(kanji: KanjiInfo): string {
  const parts: string[] = [];
  if (has(kanji.grade)) parts.push(gradeLabel(kanji.grade));
  if (has(kanji.strokes)) parts.push(`${kanji.strokes} ${kanji.strokes === 1 ? 'stroke' : 'strokes'}`);
  if (has(kanji.freq)) parts.push(`freq ${kanji.freq}`);
  if (has(kanji.jlpt)) parts.push(`old JLPT ${kanji.jlpt}`);
  return parts.length > 0 ? `(${parts.join(', ')})` : '';
}

/**
 * A kun reading as the popover shows it. KANJIDIC2 writes a hyphen on a
 * reading used only as a suffix (`-がた`) or only as a prefix (`ひと-`) and a
 * dot at the okurigana boundary (`た.べる`). The hyphen is trimmed: it is a
 * lexicographer's fact that reads as a stray mark beside the middle-dot
 * separators. The dot stays: it shows where the kanji stops and the reader's
 * own kana begin, which is how the word is written. In the 2026-09-11 file
 * every hyphen sits at an edge (266 leading, 126 trailing, none interior), so
 * trimming both ends is the whole rule. The record itself keeps the notation;
 * this is a rendering choice (Phase 3c design §3).
 */
function kunReading(reading: string): string {
  return reading.replace(/^-+|-+$/g, '');
}

/**
 * One kanji's line: the literal, its on then kun readings joined by the
 * katakana middle dot, its English meanings, and the misc aside. Columns are
 * two spaces apart; an empty column is dropped rather than left as a gap.
 * Kun readings pass through `kunReading` first.
 */
function kanjiLine(kanji: KanjiInfo): string {
  const readings = [...kanji.on, ...kanji.kun.map(kunReading)].join('・');
  return [kanji.literal, readings, kanji.meanings.join(', '), kanjiAside(kanji)]
    .filter((column) => column !== '')
    .join('  ');
}

/**
 * Every match for one word, as the tooltip's flat text block, followed —
 * when there are any and the reader has not hidden them — by one blank line
 * and one line per kanji in the hovered surface.
 *
 * Flat text rather than structured markup because the colouring that follows
 * is lexical, not semantic (see `tooltip-colour.ts`): it reads characters, so
 * what it needs is characters.
 */
export function assembleTooltipText(
  entries: Entry[],
  filters?: GlossFilters,
  kanji: KanjiInfo[] = [],
): string {
  const lines = entries.flatMap((e) => entryLines(e, filters));
  if (!filters?.hide_kanji && kanji.length > 0) lines.push('', ...kanji.map(kanjiLine));
  return lines.join('\n');
}

