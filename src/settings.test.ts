// SPDX-License-Identifier: MIT OR Apache-2.0

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
  // The `input` path targets the main window directly (see settings.ts) --
  // captured into the same `emitted` array since these tests only assert on
  // the event name and payload shape, not on which window received it.
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

describe('Settings Window', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('main');
    document.body.appendChild(container);
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
  });

  afterEach(() => {
    container.remove();
  });

  test('initializes controls from get_settings snapshot', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: true,
          clipboard_monitoring: false,
          chrome_mode: 'native',
          furigana_mode: 'katakana',
          hide_pos: true,
          hide_xrefs: true,
          hide_usage: false,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos');
    const hideXrefs = container.querySelector<HTMLInputElement>('#setting-hide-xrefs');
    const hideUsage = container.querySelector<HTMLInputElement>('#setting-hide-usage');
    const alwaysOnTop = container.querySelector<HTMLInputElement>('#setting-always-on-top');
    const monitoring = container.querySelector<HTMLInputElement>('#setting-clipboard-monitoring');
    const kataBtn = container.querySelector<HTMLButtonElement>('button[data-mode="katakana"]');

    expect(hidePos?.checked).toBe(true);
    expect(hideXrefs?.checked).toBe(true);
    expect(hideUsage?.checked).toBe(false);
    expect(alwaysOnTop?.checked).toBe(true);
    expect(monitoring?.checked).toBe(false);
    expect(kataBtn?.getAttribute('aria-checked')).toBe('true');
  });

  test('toggling gloss filter checkbox invokes save_settings and emits settings-changed', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          furigana_mode: 'none',
          hide_pos: false,
          hide_xrefs: false,
          hide_usage: false,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
    hidePos.checked = true;
    hidePos.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({
        hide_pos: true,
        hide_xrefs: false,
        hide_usage: false,
      }),
    });

    expect(emitted).toContainEqual([
      'settings-changed',
      expect.objectContaining({
        hide_pos: true,
      }),
    ]);
  });

  test('clicking furigana mode button updates active mode and emits settings-changed', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          furigana_mode: 'none',
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const hiraBtn = container.querySelector<HTMLButtonElement>('button[data-mode="hiragana"]')!;
    hiraBtn.click();

    expect(hiraBtn.getAttribute('aria-checked')).toBe('true');
    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({
        furigana_mode: 'hiragana',
      }),
    });

    expect(emitted).toContainEqual([
      'settings-changed',
      expect.objectContaining({
        furigana_mode: 'hiragana',
      }),
    ]);
  });

  test('toggling window preference checkboxes invokes setter commands and broadcasts changes', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const alwaysOnTop = container.querySelector<HTMLInputElement>('#setting-always-on-top')!;
    alwaysOnTop.checked = true;
    alwaysOnTop.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('set_always_on_top', { enabled: true });
    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({
        always_on_top: true,
      }),
    });

    const monitoring = container.querySelector<HTMLInputElement>('#setting-clipboard-monitoring')!;
    monitoring.checked = false;
    monitoring.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('set_clipboard_monitoring', { enabled: false });
  });

  test('the chrome control reflects the persisted mode', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'content' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);

    const checked = () =>
      container.querySelector<HTMLButtonElement>('[data-chrome-mode][aria-checked="true"]')
        ?.dataset.chromeMode;
    await vi.waitFor(() => expect(checked()).toBe('content'));
  });

  test('picking a mode invokes set_chrome_mode and broadcasts it', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'native' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_settings'));

    container.querySelector<HTMLButtonElement>('[data-chrome-mode="header"]')!.click();

    expect(invoke).toHaveBeenCalledWith('set_chrome_mode', { mode: 'header' });
    expect(emitted.at(-1)?.[1]).toMatchObject({ chrome_mode: 'header' });
    expect(
      container
        .querySelector<HTMLButtonElement>('[data-chrome-mode="header"]')!
        .getAttribute('aria-checked'),
    ).toBe('true');
  });

  // Windows has no native-frame state (spec §4), so offering it would name a
  // shape the platform cannot draw — and the backend would resolve it back to
  // `header` anyway, leaving the control showing a mode the window is not in.
  test('the native option is absent on Windows', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'header' } as Settings);
      if (cmd === 'platform') return Promise.resolve('windows');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);

    await vi.waitFor(() =>
      expect(container.querySelector('[data-chrome-mode="native"]')).toBeNull(),
    );
    expect(container.querySelector('[data-chrome-mode="header"]')).not.toBeNull();
    expect(container.querySelector('[data-chrome-mode="content"]')).not.toBeNull();
  });

  test('the native option survives on macOS', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: 'native' } as Settings);
      if (cmd === 'platform') return Promise.resolve('macos');
      return Promise.resolve(undefined);
    });
    initSettingsApp(container);

    // `invoke('platform')` fires synchronously inside initSettingsApp, so
    // waiting for that call (as the Windows test's DOM-polling waitFor does
    // implicitly) proves nothing here — the button is present by default, so
    // the assertion below would pass even if the `.then()` that would remove
    // it never ran. Flush the microtask queue so that callback has actually
    // run before asserting the button survived it.
    await Promise.resolve();
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith('platform');
    expect(container.querySelector('[data-chrome-mode="native"]')).not.toBeNull();
  });

  test('reacts to incoming settings-changed events from other windows', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          furigana_mode: 'none',
          hide_pos: false,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
    const romajiBtn = container.querySelector<HTMLButtonElement>('button[data-mode="romaji"]')!;

    expect(hidePos.checked).toBe(false);
    expect(romajiBtn.getAttribute('aria-checked')).toBe('false');

    emit('settings-changed', {
      hide_pos: true,
      furigana_mode: 'romaji',
    });

    expect(hidePos.checked).toBe(true);
    expect(romajiBtn.getAttribute('aria-checked')).toBe('true');
  });

  test('initializes the size inputs from the get_settings snapshot', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          font_sentence: 30,
          font_gloss: 18,
          furigana_scale: 70,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')?.value).toBe('30');
    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')?.value).toBe('18');
    expect(container.querySelector<HTMLInputElement>('#setting-furigana-scale')?.value).toBe('70');
  });

  test('changing a size input saves it and broadcasts the change', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          font_sentence: 24,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const sentence = container.querySelector<HTMLInputElement>('#setting-font-sentence')!;
    sentence.value = '32';
    sentence.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({ font_sentence: 32 }),
    });
    expect(emitted).toContainEqual([
      'settings-changed',
      expect.objectContaining({ font_sentence: 32 }),
    ]);
  });

  // `min`/`max` on a number input stop the steppers, not typing or a paste.
  test('pins a size typed past the limit back into range', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const gloss = container.querySelector<HTMLInputElement>('#setting-font-gloss')!;
    gloss.value = '400';
    gloss.dispatchEvent(new Event('change', { bubbles: true }));

    expect(gloss.value).toBe('28');
    expect(invoke).toHaveBeenCalledWith('save_settings', {
      settings: expect.objectContaining({ font_gloss: 28 }),
    });
  });

  // An emptied box means "use the stylesheet default", not "size zero".
  test('sends no size for an emptied input', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          font_sentence: 24,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    const sentence = container.querySelector<HTMLInputElement>('#setting-font-sentence')!;
    sentence.value = '';
    sentence.dispatchEvent(new Event('change', { bubbles: true }));

    const [, payload] = emitted.find(([event]) => event === 'settings-changed')!;
    expect((payload as Settings).font_sentence).toBeUndefined();
  });

  test('mirrors a size arriving on a settings-changed event', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          font_gloss: 14,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    // A real settings-changed snapshot always carries these two non-optional
    // `Settings` keys, marking it a full snapshot rather than a partial
    // payload like `{ surface_alpha }` — the same convention main.test.ts's
    // parallel font-size tests already follow.
    emit('settings-changed', { always_on_top: false, clipboard_monitoring: true, font_gloss: 20 });

    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')?.value).toBe('20');
  });

  test('a full snapshot with no font sizes empties the boxes', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          font_sentence: 30,
          font_gloss: 18,
          furigana_scale: 70,
        });
      }
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    // What `layout::restore` emits for a slot that recorded the sizes unset:
    // a full snapshot whose three font keys are simply absent.
    emit('settings-changed', { always_on_top: false, clipboard_monitoring: true });

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')!.value).toBe('');
    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')!.value).toBe('');
    expect(container.querySelector<HTMLInputElement>('#setting-furigana-scale')!.value).toBe('');
  });

  test('a partial payload leaves the font boxes alone', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          font_sentence: 30,
        });
      }
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    // A slider drag emits this sixty times a second. Clearing the boxes on it
    // would reproduce 4B ruling 1's bug on this surface.
    emit('settings-changed', { surface_alpha: 40 });

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')!.value).toBe('30');
  });

  // A first run has nothing persisted. Empty boxes give the user nothing to
  // adjust from, so the stylesheet's own defaults are shown instead — read
  // from the tokens, not restated here.
  test('shows the stylesheet defaults when no size is persisted', async () => {
    document.documentElement.style.setProperty('--text-sentence', '24px');
    document.documentElement.style.setProperty('--text-gloss', '14px');
    document.documentElement.style.setProperty('--text-furigana', '0.58em');
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')?.value).toBe('24');
    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')?.value).toBe('14');
    expect(container.querySelector<HTMLInputElement>('#setting-furigana-scale')?.value).toBe('58');
  });

  test('a persisted size wins over the stylesheet default', async () => {
    document.documentElement.style.setProperty('--text-sentence', '24px');
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          font_sentence: 36,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();

    expect(container.querySelector<HTMLInputElement>('#setting-font-sentence')?.value).toBe('36');
  });

  /** Boot the settings window with a given snapshot and settle its promises. */
  async function boot(settings: Partial<Settings>) {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      return Promise.resolve(null);
    });
    initSettingsApp(container);
    await Promise.resolve();
    await Promise.resolve();
  }

  function slider(): HTMLInputElement {
    const el = container.querySelector<HTMLInputElement>('#setting-surface-alpha');
    if (el === null) throw new Error('#setting-surface-alpha missing');
    return el;
  }

  test('the slider shows the persisted opacity', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 62 });
    expect(slider().value).toBe('62');
    expect(
      container.querySelector('#setting-surface-alpha-value')?.textContent,
    ).toBe('62%');
  });

  // A range input always has a value, so unlike an empty font box it cannot
  // signal "unset" by being blank -- it is seeded from the stylesheet instead.
  test('with nothing persisted the slider shows the stylesheet default', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true });
    expect(slider().value).toBe('100');
  });

  // A drag fires `input` per frame. Broadcasting the whole snapshot at that
  // rate would re-run the main window's gloss merge and furigana comparison
  // sixty times a second for a value neither of them contains.
  test('dragging broadcasts a partial payload without persisting', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 100 });
    emitted.length = 0;
    invoke.mockClear();

    slider().value = '35';
    slider().dispatchEvent(new Event('input'));

    expect(emitted).toEqual([['settings-changed', { surface_alpha: 35 }]]);
    expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
  });

  test('releasing the slider persists the full snapshot', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, surface_alpha: 100 });
    invoke.mockClear();

    slider().value = '35';
    slider().dispatchEvent(new Event('change'));

    const save = invoke.mock.calls.find(([cmd]) => cmd === 'save_settings');
    expect(save).toBeDefined();
    expect((save?.[1] as { settings: Settings }).settings.surface_alpha).toBe(35);
  });

  // The native frame renders fully opaque whatever is stored, so an enabled
  // slider there would move nothing.
  test('the slider is disabled in native chrome and enabled on leaving it', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' });
    expect(slider().disabled).toBe(true);

    container.querySelector<HTMLButtonElement>('[data-chrome-mode="content"]')?.click();
    expect(slider().disabled).toBe(false);
  });

  // A disabled control leaves the tab order, so a `title` on it can only be
  // reached with a mouse. The reason has to be readable without focusing it.
  test('native chrome states in visible text why the slider is inert', async () => {
    await boot({ always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' });

    const note = container.querySelector<HTMLElement>('#setting-surface-alpha-note')!;
    expect(note.hidden).toBe(false);
    expect(note.textContent?.trim()).not.toBe('');

    container.querySelector<HTMLButtonElement>('[data-chrome-mode="content"]')?.click();
    expect(note.hidden).toBe(true);
  });

  /** Flush the promise chains `initSettingsApp` starts on import. */
  async function settled(): Promise<void> {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }

  test('the hints checkbox reflects the persisted setting', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: true,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLInputElement>('#setting-mecab-hints')?.checked,
    ).toBe(true);
  });

  test('toggling hints on disables the checkbox and invokes set_mecab_hints', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: false,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));

    expect(invoke).toHaveBeenCalledWith('set_mecab_hints', { enabled: true });
    // The backend's event decides what the checkbox ends up showing; until one
    // arrives, a second click must not be able to race the acquisition.
    expect(checkbox.disabled).toBe(true);
  });

  test('a phase event shows its label and leaves the checkbox disabled', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'downloading');

    const status = container.querySelector<HTMLElement>('#setting-mecab-hints-status');
    expect(status?.textContent ?? '').not.toBe('');
    expect(status?.hidden).toBe(false);
    expect(checkbox.disabled).toBe(true);
  });

  test('a ready event checks the box, re-enables it, and clears the status', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'unpacking');
    emit('hints-status', 'ready');

    expect(checkbox.checked).toBe(true);
    expect(checkbox.disabled).toBe(false);
    expect(container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.hidden).toBe(
      true,
    );
  });

  test('an off event unchecks the box and re-enables it', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = false;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'off');

    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(false);
  });

  test('a failure message unchecks the box and shows the message verbatim', async () => {
    invoke.mockImplementation(() => Promise.resolve(null));
    initSettingsApp(container);
    await settled();

    const checkbox = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
    checkbox.checked = true;
    checkbox.dispatchEvent(new Event('change'));
    emit('hints-status', 'the dictionary server returned HTTP 404');

    // The setting was not persisted, so the checkbox must not read as on.
    expect(checkbox.checked).toBe(false);
    expect(checkbox.disabled).toBe(false);
    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.textContent,
    ).toContain('HTTP 404');
  });

  test('a startup hints warning seeds the status line', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') {
        return Promise.resolve('MeCab boundary hints are switched on but not loaded');
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-status')?.textContent,
    ).toContain('not loaded');
  });

  test('the note names the hints directory so the manual route is discoverable', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_dir') return Promise.resolve('C:\\Users\\me\\AppData\\clipyomi\\hints');
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    expect(
      container.querySelector<HTMLElement>('#setting-mecab-hints-note')?.textContent,
    ).toContain('clipyomi\\hints');
  });

  test('saving another setting never sends mecab_hints', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          mecab_hints: true,
        });
      }
      return Promise.resolve(null);
    });

    initSettingsApp(container);
    await settled();

    const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
    hidePos.checked = true;
    hidePos.dispatchEvent(new Event('change'));

    const save = invoke.mock.calls.find(([cmd]) => cmd === 'save_settings');
    expect(save).toBeDefined();
    // The backend owns this key outright. A snapshot carrying it could clobber
    // an in-flight toggle, and `merge_saved` is a whitelist precisely so it
    // cannot — this asserts the frontend half of that agreement.
    expect(Object.keys(save![1].settings)).not.toContain('mecab_hints');
  });
});
