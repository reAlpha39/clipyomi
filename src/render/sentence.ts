import type { ParseResult, Segment } from '../types';

/**
 * Content class for a segment's chip, from the flags the Rust side named.
 * Reading `flags` rather than inspecting the surface keeps one definition of
 * "particle" in the codebase, on the Rust side.
 */
function contentClass(segment: Segment): string {
  const flags = segment.entries[0]?.flags ?? [];
  if (flags.includes('particle')) return 'particle';
  if (flags.includes('counter')) return 'counter';
  return /[一-鿿]/.test(segment.surface) ? 'kanji' : 'kana';
}

export function renderSentence(result: ParseResult): HTMLElement {
  const root = document.createElement('div');
  root.className = 'sentence';

  for (const segment of result.segments) {
    const el = document.createElement('span');
    el.dataset.start = String(segment.start);
    el.textContent = segment.surface;

    // Unmatched runs stay unchipped so coverage gaps are visible rather than
    // disguised — seeing where the parser fails is the point of the window.
    el.className = segment.matched ? `chip ${contentClass(segment)}` : 'unmatched';

    root.append(el);
  }

  return root;
}
