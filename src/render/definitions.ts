import type { Entry, ParseResult } from '../types';

export function renderEntry(entry: Entry): HTMLElement {
  const el = document.createElement('div');
  el.className = 'entry';

  const head = document.createElement('div');
  head.className = 'entry-head';

  const headword = document.createElement('span');
  headword.className = 'headword';
  headword.textContent = entry.headword;
  head.append(headword);

  if (entry.conjugation !== null) {
    const tag = document.createElement('span');
    tag.className = 'conjugation';
    tag.textContent = entry.conjugation;
    head.append(tag);
  }
  el.append(head);

  if (entry.reading !== null) {
    const reading = document.createElement('div');
    reading.className = 'reading';
    reading.textContent = entry.reading;
    el.append(reading);
  }

  const senses = document.createElement('ol');
  senses.className = 'senses';
  for (const sense of entry.senses) {
    const li = document.createElement('li');
    li.textContent = sense.glosses.join('; ');
    senses.append(li);
  }
  el.append(senses);

  return el;
}

export function renderDefinitions(result: ParseResult): HTMLElement {
  const root = document.createElement('div');
  root.className = 'definitions';

  for (const segment of result.segments) {
    // Unmatched runs have no entries, so they get no row — the sentence pane
    // already shows them as gaps.
    if (!segment.matched || segment.entries.length === 0) continue;

    const row = document.createElement('section');
    row.className = 'def-row';
    row.dataset.start = String(segment.start);

    const [primary, ...alternates] = segment.entries;
    row.append(renderEntry(primary));

    if (alternates.length > 0) {
      // Collapsed past the first: the payoff from the segmenter's backtrack
      // pass, without letting alternates bury the ranked primary.
      const details = document.createElement('details');
      const summary = document.createElement('summary');
      summary.textContent = `${alternates.length} more`;
      details.append(summary);
      for (const alternate of alternates) details.append(renderEntry(alternate));
      row.append(details);
    }

    root.append(row);
  }

  return root;
}
