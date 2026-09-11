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

/**
 * One kanji's line: the literal, its on then kun readings joined by the
 * katakana middle dot, its English meanings. Columns are two spaces apart;
 * an empty column is dropped rather than left as a gap. KANJIDIC2's `.` and
 * `-` marks in kun readings are rendered raw, deliberately (design §7).
 */
function kanjiLine(kanji: KanjiInfo): string {
  const readings = [...kanji.on, ...kanji.kun].join('・');
  return [kanji.literal, readings, kanji.meanings.join(', ')]
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

