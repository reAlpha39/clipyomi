// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { Entry } from './types';

// The tooltip webview's entry, exercised the same way `main.test.ts` exercises
// the app entry: both IPC boundaries stubbed, then the module imported for its
// side effect of registering the listener.
const listeners = new Map<string, (e: { payload: unknown }) => void>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
  emit: () => Promise.resolve(),
}));

const entries: Entry[] = [
  {
    headword: '東京',
    reading: 'とうきょう',
    conjugation: null,
    pos: ['n'],
    senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
    flags: ['primary'],
  },
];

function send(payload: unknown): void {
  const handler = listeners.get('popover-content');
  if (handler === undefined) throw new Error('nothing listening for popover-content');
  handler({ payload });
}

function token(name: string): string {
  return document.documentElement.style.getPropertyValue(name);
}

describe('the popover window', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<div id="tooltip"></div>';
    document.documentElement.removeAttribute('style');
    listeners.clear();
    vi.resetModules();
    await import('./popover');
  });

  test('renders the entries it is sent', () => {
    send({ entries });

    expect(document.querySelector('#tooltip')?.textContent).toContain('東京');
  });

  // The size arrives with the content rather than through a settings listener
  // so that it is applied before the render this handler then measures.
  test('applies the gloss size the payload carries', () => {
    send({ entries, gloss: 18 });

    expect(token('--text-gloss')).toBe('18px');
  });

  test('clamps a gloss size outside the supported range', () => {
    send({ entries, gloss: 400 });

    expect(token('--text-gloss')).toBe('28px');
  });

  test('leaves the stylesheet default alone when the payload carries no size', () => {
    send({ entries, filters: { hide_pos: true } });

    expect(token('--text-gloss')).toBe('');
  });

  test('still renders a bare array payload from an older main window', () => {
    send(entries);

    expect(document.querySelector('#tooltip')?.textContent).toContain('東京');
  });

  test('renders the kanji block it is sent, after a blank line', () => {
    send({
      entries,
      kanji: [{ literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] }],
    });

    const lines = [...document.querySelectorAll('.tt-line')].map((el) => el.textContent);
    expect(lines).toEqual(['東京【とうきょう】', '(n) (1) Tokyo', '', '東  トウ・ひがし  east']);
    // The blank line keeps its height, as the gap between two entries does.
    expect(document.querySelectorAll('.tt-line br')).toHaveLength(1);
    // The literal is coloured as kanji, like a headword's characters.
    const literal = [...document.querySelectorAll('.tt-kanji')].map((el) => el.textContent);
    expect(literal).toContain('東');
  });

  test('renders no block under hide_kanji', () => {
    send({
      entries,
      filters: { hide_kanji: true },
      kanji: [{ literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] }],
    });

    expect(document.querySelectorAll('.tt-line')).toHaveLength(2);
  });
});
