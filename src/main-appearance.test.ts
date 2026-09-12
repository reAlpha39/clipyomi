// SPDX-License-Identifier: MIT OR Apache-2.0

// The appearance settings reaching the main window: colour and font-family
// tokens on the root element, the outline class, and the gloss family riding
// the popover payload. A second file rather than more of `main.test.ts`,
// which is far past the ceiling; the IPC scaffold is the same.

import { beforeEach, describe, expect, test, vi } from 'vitest';
import type { Entry } from './types';

const listeners = new Map<string, (e: { payload: unknown }) => void>();
const emitted: [string, unknown][] = [];

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
  emit: (event: string, payload: unknown) => {
    emitted.push([event, payload]);
    return Promise.resolve();
  },
}));

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    label: 'main',
    innerPosition: () => Promise.resolve({ x: 0, y: 0 }),
    outerPosition: () =>
      Promise.resolve({
        x: 100,
        y: 100,
        toLogical: (factor: number) => ({ x: 100 / factor, y: 100 / factor }),
      }),
    innerSize: () =>
      Promise.resolve({
        width: 800,
        height: 600,
        toLogical: (factor: number) => ({ width: 800 / factor, height: 600 / factor }),
      }),
    scaleFactor: () => Promise.resolve(1),
    show: () => Promise.resolve(),
    startResizeDragging: () => Promise.resolve(),
  }),
  cursorPosition: () => Promise.resolve({ x: 0, y: 0 }),
  availableMonitors: () => Promise.resolve([]),
}));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

const FULL = { always_on_top: false, clipboard_monitoring: true };

function token(name: string): string {
  return document.documentElement.style.getPropertyValue(name);
}

async function load(settings: Record<string, unknown>) {
  document.body.innerHTML = '<main id="app"></main>';
  listeners.clear();
  emitted.length = 0;
  invoke.mockReset();
  invoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_settings') return Promise.resolve(settings);
    if (cmd === 'kanji_info') return Promise.resolve([]);
    return Promise.resolve(null);
  });
  vi.resetModules();
  await import('./main');
  await Promise.resolve();
  await Promise.resolve();
}

describe('appearance settings reaching the main window', () => {
  beforeEach(() => {
    // The root element outlives module resets, so a token or class set by one
    // test would otherwise be read back as a pass by the next.
    document.documentElement.removeAttribute('style');
    document.documentElement.className = '';
  });

  test('applies the persisted colours on load', async () => {
    await load({ colour_kanji: '#112233', colour_furigana: '#445566' });

    expect(token('--color-kanji')).toBe('#112233');
    expect(token('--color-furigana')).toBe('#445566');
    expect(token('--color-kana')).toBe('');
  });

  test('applies the persisted families on load, quoted', async () => {
    await load({ font_family_sentence: 'Yu Gothic', font_family_furigana: 'Meiryo' });

    expect(token('--font-cjk')).toBe('"Yu Gothic"');
    expect(token('--font-furigana')).toBe('"Meiryo"');
  });

  test('sets the outline class from the persisted toggle', async () => {
    await load({ text_outline: true });

    expect(document.documentElement.classList.contains('text-outline')).toBe(true);
  });

  test('a full snapshot with a colour gone clears its token', async () => {
    await load({ colour_kanji: '#112233' });

    emit('settings-changed', { ...FULL, colour_kana: '#445566' });

    expect(token('--color-kanji')).toBe('');
    expect(token('--color-kana')).toBe('#445566');
  });

  // The picker's drag sends one key alone, at frame rate; it must land as a
  // preview and must not read the other colours as cleared.
  test('a partial colour payload previews without clearing the rest', async () => {
    await load({ colour_kanji: '#112233' });

    emit('settings-changed', { colour_kana: '#abcdef' });

    expect(token('--color-kana')).toBe('#abcdef');
    expect(token('--color-kanji')).toBe('#112233');
  });

  test('an opacity drag leaves the colours and families alone', async () => {
    await load({ colour_kanji: '#112233', font_family_sentence: 'Meiryo' });

    emit('settings-changed', { surface_alpha: 40 });

    expect(token('--color-kanji')).toBe('#112233');
    expect(token('--font-cjk')).toBe('"Meiryo"');
  });

  test('sets the drop-shadow class from the persisted toggle and drops it on a snapshot', async () => {
    await load({ text_shadow: true });
    expect(document.documentElement.classList.contains('text-shadow')).toBe(true);

    emit('settings-changed', { ...FULL, text_shadow: false });
    expect(document.documentElement.classList.contains('text-shadow')).toBe(false);
  });

  test('a snapshot turning the outline off removes the class', async () => {
    await load({ text_outline: true });

    emit('settings-changed', { ...FULL, text_outline: false });

    expect(document.documentElement.classList.contains('text-outline')).toBe(false);
  });

  // The popover is a second webview with its own `:root`, so the gloss family
  // travels with the content, as the gloss size does.
  test('the gloss family rides the popover payload', async () => {
    await load({ font_family_gloss: 'Segoe UI' });
    const entries: Entry[] = [
      {
        headword: '東京',
        reading: 'とうきょう',
        conjugation: null,
        pos: ['n'],
        senses: [],
        flags: ['primary'],
      },
    ];
    emit('parse-result', {
      segments: [
        { start: 0, len: 2, surface: '東京', reading: 'とうきょう', matched: true, entries },
      ],
      alternatives: [],
    });

    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    for (let i = 0; i < 4; i += 1) await Promise.resolve();

    const content = emitted.find(([event]) => event === 'popover-content');
    expect((content?.[1] as { gloss_font?: string }).gloss_font).toBe('Segoe UI');
  });
});
