import type { Entry } from '../types';

/**
 * Prefix marking a line as the conjugation label.
 *
 * ta-old uses the literal control character `\x01` for this
 * (`DictionaryUtil.cpp:46`), and the colouriser looks for it the same way
 * `MyDrawText` does. Kept as a control character rather than a friendlier
 * sentinel precisely because it cannot occur in dictionary text.
 */
export const CONJ_MARKER = '\u0001';

/** One entry's block: an optional conjugation line, a headword line, then senses. */
function entryLines(entry: Entry): string[] {
  const lines: string[] = [];

  if (entry.conjugation !== null) lines.push(`${CONJ_MARKER}${entry.conjugation}`);

  // `headword【reading】`, with the bracket omitted for a kana-only word where
  // the surface already is the reading.
  lines.push(entry.reading === null ? entry.headword : `${entry.headword}【${entry.reading}】`);

  const common = entry.flags.includes('common');
  entry.senses.forEach((sense, i) => {
    // Glosses join with "/" — ta-old's separator. The pane uses "; "; the two
    // surfaces render differently on purpose from this phase onward.
    const glosses = sense.glosses.join('/');
    const tail = common && i === entry.senses.length - 1 ? '/(P)' : '';
    lines.push(`(${sense.pos.join(',')}) (${i + 1}) ${glosses}${tail}`);
  });

  return lines;
}

/**
 * Every match for one word, as ta-old's flat text block.
 *
 * Flat text rather than structured markup because the colouring that follows
 * is lexical, not semantic (see `tooltip-colour.ts`): it reads characters, so
 * what it needs is characters.
 */
export function assembleTooltipText(entries: Entry[]): string {
  return entries.flatMap(entryLines).join('\n');
}
