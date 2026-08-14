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
    // A matched chip is a real control (activates the same way for a mouse
    // click and a keyboard Enter/Space, focusable, out of the tab order when
    // disabled) — a native <button> gets all of that for free, with no
    // separate keydown handler to keep in sync with the click listener.
    // Unmatched runs stay a plain <span>: not a control, and — with no
    // tabindex or role added — never reachable by keyboard, so coverage gaps
    // are visible but never land in the tab order.
    const el = document.createElement(segment.matched ? 'button' : 'span');
    if (el instanceof HTMLButtonElement) el.type = 'button';
    el.dataset.start = String(segment.start);
    el.textContent = segment.surface;
    el.className = segment.matched ? `chip ${contentClass(segment)}` : 'unmatched';

    root.append(el);
  }

  return root;
}
