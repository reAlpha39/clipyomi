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
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('platform'));
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

    emit('settings-changed', { font_gloss: 20 });

    expect(container.querySelector<HTMLInputElement>('#setting-font-gloss')?.value).toBe('20');
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
});
