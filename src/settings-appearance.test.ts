// SPDX-License-Identifier: MIT OR Apache-2.0

// The appearance controls of the settings window: the five colour swatches,
// the three font selects and the outline checkbox. A second file rather than
// more of `settings.test.ts`, which is at the 800-line ceiling; the IPC
// scaffold is the same.

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import { initSettingsApp } from './settings';
import type { Settings } from './types';

const { listeners, emitted, invoke } = vi.hoisted(() => ({
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
  emitted: [] as [string, unknown][],
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
  emit: (event: string, payload: unknown) => {
    emitted.push([event, payload]);
    return Promise.resolve();
  },
  emitTo: (_target: string, event: string, payload: unknown) => {
    emitted.push([event, payload]);
    return Promise.resolve();
  },
}));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

const FULL: Settings = { always_on_top: false, clipboard_monitoring: true };

describe('the appearance controls', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('main');
    document.body.appendChild(container);
    document.documentElement.removeAttribute('style');
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
  });

  afterEach(() => {
    container.remove();
  });

  /** Boot with a snapshot and an installed-font list, then settle the promises. */
  /** What `system_fonts` answers: two Japanese-capable families and one Latin-only. */
  const FONTS = [
    { name: 'Meiryo', japanese: true },
    { name: 'Segoe UI', japanese: false },
    { name: 'Yu Gothic', japanese: true },
  ];

  async function boot(settings: Partial<Settings>, fonts: unknown = FONTS) {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      if (cmd === 'system_fonts') return Promise.resolve(fonts);
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  function el<T extends HTMLElement>(selector: string): T {
    const found = container.querySelector<T>(selector);
    if (found === null) throw new Error(`${selector} missing`);
    return found;
  }

  function saved(): Settings {
    const save = invoke.mock.calls.find(([cmd]) => cmd === 'save_settings');
    if (save === undefined) throw new Error('save_settings was not invoked');
    return (save[1] as { settings: Settings }).settings;
  }

  describe('colours', () => {
    test('shows the persisted colours in the swatches', async () => {
      await boot({ ...FULL, colour_kanji: '#112233', colour_furigana: '#445566' });

      expect(el<HTMLInputElement>('#setting-colour-kanji').value).toBe('#112233');
      expect(el<HTMLInputElement>('#setting-colour-furigana').value).toBe('#445566');
    });

    // A colour input always holds a value, so like the opacity slider it
    // cannot signal "unset" by being blank: it shows the theme's own colour.
    test('seeds an unset swatch from the stylesheet default', async () => {
      document.documentElement.style.setProperty('--color-kana', 'rgb(17, 34, 51)');
      await boot(FULL);

      expect(el<HTMLInputElement>('#setting-colour-kana').value).toBe('#112233');
    });

    test('an unset colour is not sent with the snapshot', async () => {
      document.documentElement.style.setProperty('--color-kana', 'rgb(17, 34, 51)');
      await boot(FULL);

      el<HTMLInputElement>('#setting-hide-pos').dispatchEvent(new Event('change'));

      expect(Object.keys(saved())).not.toContain('colour_kana');
    });

    test('picking a colour persists it and broadcasts the snapshot', async () => {
      await boot(FULL);
      const swatch = el<HTMLInputElement>('#setting-colour-particle');

      swatch.value = '#abcdef';
      swatch.dispatchEvent(new Event('change'));

      expect(saved().colour_particle).toBe('#abcdef');
      const broadcast = emitted.find(([, payload]) => (payload as Settings).always_on_top !== undefined);
      expect((broadcast?.[1] as Settings).colour_particle).toBe('#abcdef');
    });

    // The native picker fires `input` continuously while the user drags
    // inside it; the main window previews each one without a disk write.
    test('dragging inside the picker previews without persisting', async () => {
      await boot(FULL);
      emitted.length = 0;
      const swatch = el<HTMLInputElement>('#setting-colour-counter');

      swatch.value = '#abcdef';
      swatch.dispatchEvent(new Event('input'));

      expect(emitted).toEqual([['settings-changed', { colour_counter: '#abcdef' }]]);
      expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
    });

    // The grey of punctuation and unparsed runs is a sixth colour of its own,
    // separate from the muted token the window chrome shares.
    test('the unmatched-text swatch persists like the others', async () => {
      await boot(FULL);
      const swatch = el<HTMLInputElement>('#setting-colour-unmatched');

      swatch.value = '#778899';
      swatch.dispatchEvent(new Event('change'));

      expect(saved().colour_unmatched).toBe('#778899');
    });

    test('Reset drops the colour and shows the stylesheet default again', async () => {
      document.documentElement.style.setProperty('--color-kanji', 'rgb(1, 2, 3)');
      await boot({ ...FULL, colour_kanji: '#112233' });

      el<HTMLButtonElement>('#setting-colour-kanji-reset').click();

      expect(Object.keys(saved())).not.toContain('colour_kanji');
      expect(el<HTMLInputElement>('#setting-colour-kanji').value).toBe('#010203');
    });

    // A restored layout slot, or another window's save, arrives as a full
    // snapshot; a key gone from it means unset. A partial payload — an
    // opacity drag — carries no colours and must not unset anything.
    test('a full snapshot owns the swatches and a partial payload leaves them alone', async () => {
      await boot({ ...FULL, colour_kanji: '#112233' });

      emit('settings-changed', { surface_alpha: 40 });
      el<HTMLInputElement>('#setting-hide-pos').dispatchEvent(new Event('change'));
      expect(saved().colour_kanji).toBe('#112233');

      invoke.mockClear();
      emit('settings-changed', { ...FULL, colour_kana: '#445566' });
      el<HTMLInputElement>('#setting-hide-pos').dispatchEvent(new Event('change'));
      expect(Object.keys(saved())).not.toContain('colour_kanji');
      expect(saved().colour_kana).toBe('#445566');
    });
  });

  describe('fonts', () => {
    function options(selector: string): string[] {
      return [...el<HTMLSelectElement>(selector).options].map((o) => o.textContent ?? '');
    }

    // A Latin-only family applied to Japanese text falls back glyph by glyph
    // to the system CJK font, so the pick looks like nothing happened. The
    // two Japanese selects offer only families that can render the text.
    test('the sentence and furigana selects list only Japanese-capable families', async () => {
      await boot(FULL);

      expect(options('#setting-font-family-sentence')).toEqual(['Default', 'Meiryo', 'Yu Gothic']);
      expect(options('#setting-font-family-furigana')).toEqual(['Default', 'Meiryo', 'Yu Gothic']);
      expect(el<HTMLSelectElement>('#setting-font-family-sentence').value).toBe('');
    });

    // Romaji is Latin text: a Japanese font's Latin glyphs are rarely what
    // anyone wants there, and every installed family can render it.
    test('in romaji mode the furigana select lists every family', async () => {
      await boot({ ...FULL, furigana_mode: 'romaji' });

      expect(options('#setting-font-family-furigana')).toEqual([
        'Default',
        'Meiryo',
        'Segoe UI',
        'Yu Gothic',
      ]);
      expect(options('#setting-font-family-sentence')).toEqual(['Default', 'Meiryo', 'Yu Gothic']);
    });

    test('switching the furigana mode re-filters the select and keeps the pick', async () => {
      await boot({ ...FULL, furigana_mode: 'hiragana', font_family_furigana: 'Meiryo' });
      const select = el<HTMLSelectElement>('#setting-font-family-furigana');
      expect(options('#setting-font-family-furigana')).not.toContain('Segoe UI');

      el<HTMLButtonElement>('[data-mode="romaji"]').click();
      expect(options('#setting-font-family-furigana')).toContain('Segoe UI');
      expect(select.value).toBe('Meiryo');

      el<HTMLButtonElement>('[data-mode="katakana"]').click();
      expect(options('#setting-font-family-furigana')).not.toContain('Segoe UI');
      expect(select.value).toBe('Meiryo');
    });

    // Definitions are English, so every installed family is a candidate.
    test('the definitions select lists every family', async () => {
      await boot(FULL);

      expect(options('#setting-font-family-gloss')).toEqual([
        'Default',
        'Meiryo',
        'Segoe UI',
        'Yu Gothic',
      ]);
    });

    test('ignores entries that are not family records', async () => {
      await boot(FULL, ['Meiryo', { name: 'Yu Gothic' }, { name: 'Segoe UI', japanese: false }]);

      expect(options('#setting-font-family-gloss')).toEqual(['Default', 'Segoe UI']);
    });

    test('shows the persisted family', async () => {
      await boot({ ...FULL, font_family_gloss: 'Meiryo' });

      expect(el<HTMLSelectElement>('#setting-font-family-gloss').value).toBe('Meiryo');
    });

    // A family picked on another machine, or since uninstalled, is still what
    // the file says and what the main window applies; the select shows it
    // rather than silently reading as Default.
    test('shows a persisted family the list does not carry', async () => {
      await boot({ ...FULL, font_family_furigana: 'Gone Font' });

      expect(el<HTMLSelectElement>('#setting-font-family-furigana').value).toBe('Gone Font');
      expect(options('#setting-font-family-furigana')).toContain('Gone Font');
    });

    test('survives a font list that never arrives', async () => {
      await boot({ ...FULL, font_family_sentence: 'Meiryo' }, null);

      expect(options('#setting-font-family-sentence')).toEqual(['Default', 'Meiryo']);
    });

    test('picking a family persists it and broadcasts the snapshot', async () => {
      await boot(FULL);
      const select = el<HTMLSelectElement>('#setting-font-family-sentence');

      select.value = 'Yu Gothic';
      select.dispatchEvent(new Event('change'));

      expect(saved().font_family_sentence).toBe('Yu Gothic');
    });

    test('Default sends no family', async () => {
      await boot({ ...FULL, font_family_sentence: 'Meiryo' });
      const select = el<HTMLSelectElement>('#setting-font-family-sentence');

      select.value = '';
      select.dispatchEvent(new Event('change'));

      expect(Object.keys(saved())).not.toContain('font_family_sentence');
    });
  });

  describe('outline', () => {
    test('the checkbox reflects the persisted setting', async () => {
      await boot({ ...FULL, text_outline: true });

      expect(el<HTMLInputElement>('#setting-text-outline').checked).toBe(true);
    });

    test('toggling it persists and broadcasts', async () => {
      await boot(FULL);
      const box = el<HTMLInputElement>('#setting-text-outline');

      box.checked = true;
      box.dispatchEvent(new Event('change'));

      expect(saved().text_outline).toBe(true);
      expect(emitted.some(([, p]) => (p as Settings).text_outline === true)).toBe(true);
    });
  });

  // The gap box lives in the Text Size section and rides the font-size flow
  // in `settings.ts`; pinned here because this file is where the appearance
  // controls are exercised.
  describe('furigana gap', () => {
    test('shows the persisted gap and saves a change', async () => {
      await boot({ ...FULL, furigana_gap: 6 });
      const box = el<HTMLInputElement>('#setting-furigana-gap');
      expect(box.value).toBe('6');

      box.value = '10';
      box.dispatchEvent(new Event('change'));

      expect(saved().furigana_gap).toBe(10);
    });

    test('an emptied box sends no gap, handing the default back', async () => {
      await boot({ ...FULL, furigana_gap: 6 });
      const box = el<HTMLInputElement>('#setting-furigana-gap');

      box.value = '';
      box.dispatchEvent(new Event('change'));

      // `undefined`, as the other size boxes send an emptied value; the IPC
      // layer drops the key and `merge_saved` assigns `None`.
      expect(saved().furigana_gap).toBeUndefined();
    });
  });

  // The two appearance sizes ride the font-size flow, wired in `settings.ts`
  // beside the other boxes; pinned here with the toggles they belong to.
  describe('outline width and shadow size', () => {
    test('shows the persisted sizes and saves a change', async () => {
      await boot({ ...FULL, text_outline_width: 3, text_shadow_size: 5 });
      const stroke = el<HTMLInputElement>('#setting-text-outline-width');
      const shadow = el<HTMLInputElement>('#setting-text-shadow-size');
      expect(stroke.value).toBe('3');
      expect(shadow.value).toBe('5');

      stroke.value = '6';
      stroke.dispatchEvent(new Event('change'));

      expect(saved().text_outline_width).toBe(6);
      expect(saved().text_shadow_size).toBe(5);
    });

    test('an emptied box sends no size, handing the default back', async () => {
      await boot({ ...FULL, text_outline_width: 3 });
      const stroke = el<HTMLInputElement>('#setting-text-outline-width');

      stroke.value = '';
      stroke.dispatchEvent(new Event('change'));

      expect(saved().text_outline_width).toBeUndefined();
    });
  });

  // The window background sits with the opacity slider rather than among the
  // text colours, being the thing they are read against, but it persists and
  // previews by the same path as any of them.
  describe('window background colour', () => {
    test('shows the persisted background and saves a pick', async () => {
      await boot({ ...FULL, colour_bg: '#102030' });
      const swatch = el<HTMLInputElement>('#setting-colour-bg');
      expect(swatch.value).toBe('#102030');

      swatch.value = '#abcdef';
      swatch.dispatchEvent(new Event('change'));

      expect(saved().colour_bg).toBe('#abcdef');
    });

    test('dragging inside the picker previews without persisting', async () => {
      await boot(FULL);
      emitted.length = 0;
      const swatch = el<HTMLInputElement>('#setting-colour-bg');

      swatch.value = '#abcdef';
      swatch.dispatchEvent(new Event('input'));

      expect(emitted).toEqual([['settings-changed', { colour_bg: '#abcdef' }]]);
      expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
    });

    test('Reset drops the background and shows the stylesheet default again', async () => {
      document.documentElement.style.setProperty('--color-bg', 'rgb(1, 2, 3)');
      await boot({ ...FULL, colour_bg: '#102030' });

      el<HTMLButtonElement>('#setting-colour-bg-reset').click();

      expect(Object.keys(saved())).not.toContain('colour_bg');
      expect(el<HTMLInputElement>('#setting-colour-bg').value).toBe('#010203');
    });

    // The Text Colours section is the six that colour the sentence; the
    // background belongs with the opacity slider it is read through.
    test('is not listed among the text colours', async () => {
      await boot(FULL);

      const section = [...container.querySelectorAll('.settings-group')].find((g) =>
        g.querySelector('.settings-group-title')?.textContent?.includes('Text Colours'),
      );
      expect(section?.querySelector('#setting-colour-bg')).toBeNull();
    });
  });

  describe('drop shadow', () => {
    test('the checkbox reflects the persisted setting', async () => {
      await boot({ ...FULL, text_shadow: true });

      expect(el<HTMLInputElement>('#setting-text-shadow').checked).toBe(true);
    });

    test('toggling it persists and broadcasts', async () => {
      await boot(FULL);
      const box = el<HTMLInputElement>('#setting-text-shadow');

      box.checked = true;
      box.dispatchEvent(new Event('change'));

      expect(saved().text_shadow).toBe(true);
      expect(emitted.some(([, p]) => (p as Settings).text_shadow === true)).toBe(true);
    });
  });
});
