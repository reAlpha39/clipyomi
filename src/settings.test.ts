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
          decorations: true,
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
    const decorations = container.querySelector<HTMLInputElement>('#setting-decorations');
    const kataBtn = container.querySelector<HTMLButtonElement>('button[data-mode="katakana"]');

    expect(hidePos?.checked).toBe(true);
    expect(hideXrefs?.checked).toBe(true);
    expect(hideUsage?.checked).toBe(false);
    expect(alwaysOnTop?.checked).toBe(true);
    expect(monitoring?.checked).toBe(false);
    expect(decorations?.checked).toBe(true);
    expect(kataBtn?.getAttribute('aria-checked')).toBe('true');
  });

  test('toggling gloss filter checkbox invokes save_settings and emits settings-changed', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          decorations: true,
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
          decorations: true,
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
          decorations: true,
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

    const decorations = container.querySelector<HTMLInputElement>('#setting-decorations')!;
    decorations.checked = false;
    decorations.dispatchEvent(new Event('change', { bubbles: true }));

    expect(invoke).toHaveBeenCalledWith('set_decorations', { enabled: false });
  });

  test('reacts to incoming settings-changed events from other windows', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve<Settings>({
          always_on_top: false,
          clipboard_monitoring: true,
          decorations: true,
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
});
