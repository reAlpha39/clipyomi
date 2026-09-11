// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';
import type { ChromeMode } from './types';

// `main.ts` is the wired app entry, not a pure render function like
// `sentence.ts` — exercising it means stubbing both IPC boundaries it uses:
// `@tauri-apps/api/event` (`listen`, for the parse-result/parse-error push
// events) and `@tauri-apps/api/core` (`invoke`, still used directly for
// `set_input` and `startup_error`).
const listeners = new Map<string, (e: { payload: unknown }) => void>();
/** Every `emit` the app made, as [event, payload] pairs. */
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
/** The startup reveal — `main.ts` shows the window itself, see its comment. */
const show = vi.fn(() => Promise.resolve());
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

/**
 * Flush `openFor`'s one await — the `kanji_info` round trip — and the
 * `.then` that emits. Four ticks is more than the chain needs; the exact
 * count is not the property under test.
 */
async function flushLookup(): Promise<void> {
  for (let i = 0; i < 4; i += 1) await Promise.resolve();
}

const startResizeDragging = vi.fn((_dir: string) => Promise.resolve());

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
    show,
    startResizeDragging,
  }),
  cursorPosition: () => Promise.resolve({ x: 0, y: 0 }),
  availableMonitors: () => Promise.resolve([]),
}));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

describe('the event-driven render path', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    // `startup_error` and `settings_warning` resolve to null (nothing
    // startup-fatal, nothing settings-cosmetic to report), and `get_settings`
    // resolves to a harmless default: the header calls all three
    // unconditionally on import, and `frontend_ready` is fired once the event
    // listeners resolve (falls through to the `null` default below and is
    // caught either way, see `main.ts`'s own comment on that call) — every
    // test that imports `./main` must tolerate all of these or the module's
    // own fire-and-forget calls contaminate it with unhandled rejections.
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  test('renders a parse-result event', () => {
    emit('parse-result', {
      segments: [
        {
          start: 0, len: 2, surface: '東京', reading: 'とうきょう', matched: true,
          entries: [{
            headword: '東京', reading: 'とうきょう', conjugation: null, pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          }],
        },
      ],
      alternatives: [],
    });
    expect(document.querySelector('.chip')?.textContent).toBe('東京');
    expect(document.querySelector('.sentence')).not.toBeNull();
    expect(document.querySelector('.definitions')).toBeNull();
    expect(document.querySelector('.def-row')).toBeNull();
  });

  // 5B's hints toggle re-parses the text already on screen
  // (`commands::reparse`), so a second result can arrive carrying the same
  // surface with a different segmentation. `history.push` dedupes on that
  // surface, and dropping the result outright left the stale chips rendered —
  // the toggle looked inert until the user copied something else. This pins
  // the whole show/push/display path rather than history's contract alone.
  test('a re-segmentation of the same text replaces what is on screen', () => {
    const seg = (start: number, surface: string) => ({
      start,
      len: surface.length,
      surface,
      reading: null,
      matched: true,
      entries: [
        {
          headword: surface,
          reading: null,
          conjugation: null,
          pos: ['n'],
          senses: [{ pos: ['n'], glosses: ['x'], xrefs: [], misc: [], info: [] }],
          flags: ['primary'],
        },
      ],
    });
    const chips = () =>
      [...document.querySelectorAll('.chip')].map((c) => c.textContent);

    emit('parse-result', { segments: [seg(0, '東京'), seg(2, '都')], alternatives: [] });
    expect(chips()).toEqual(['東京', '都']);

    emit('parse-result', { segments: [seg(0, '東京都')], alternatives: [] });

    expect(chips()).toEqual(['東京都']);
  });

  // Important 2 (final review): the clipboard poll's first tick waits on the
  // Rust-side `frontend_ready` signal before it reads anything (see
  // `clipboard::wait_for_frontend`), so text copied before launch is not
  // dropped on a webview that has not finished registering its listeners
  // yet. This proves the frontend half of that handshake actually fires,
  // and only after both `listen()` calls this test's mock already recorded
  // into `listeners` have resolved.
  test('signals frontend_ready once both parse-result and parse-error listeners are registered', async () => {
    await Promise.resolve();
    await Promise.resolve();
    expect(listeners.has('parse-result')).toBe(true);
    expect(listeners.has('parse-error')).toBe(true);
    expect(invoke.mock.calls.some(([cmd]) => cmd === 'frontend_ready')).toBe(true);
  });

  // Replaces the old invoke-reject-based "two consecutive failed parses leave
  // exactly one error block" test: errors no longer arrive by `run()`
  // catching a rejected `invoke('parse_text')`, they arrive as `parse-error`
  // events, so the same one-slot-never-stacks invariant has to be proven on
  // that path instead. This version also proves the previous *result* survives
  // a failure, which the old test (no prior success in play) couldn't check.
  test('a parse-error event leaves the previous result on screen', () => {
    emit('parse-result', {
      segments: [
        { start: 0, len: 1, surface: '本', reading: 'ほん', matched: true, entries: [] },
      ],
      alternatives: [],
    });
    emit('parse-error', 'the parser panicked on an input of 4096 characters');
    emit('parse-error', 'a second failure');

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
    expect(document.querySelector('.sentence')).not.toBeNull();
  });

  // Phase 2 design §6, landed by Phase 2b: readings 2..k ride on the same
  // event as `alternatives`, in the order Phase 2b's `demote` gives them.
  const readingSeg = (start: number, surface: string, matched = true) => ({
    start,
    len: surface.length,
    surface,
    reading: null,
    matched,
    entries: matched
      ? [
          {
            headword: surface,
            reading: null,
            conjugation: null,
            pos: ['n'],
            senses: [{ pos: ['n'], glosses: [surface], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          },
        ]
      : [],
  });
  /** Every rendered segment in order, chip or unmatched run. */
  const rendered = () => [...document.querySelectorAll('.sentence > *')].map((el) => el.textContent);
  const menuLabels = () => {
    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    return [...document.querySelectorAll('.context-menu button')].map((b) => b.textContent);
  };
  const press = (key: string, init: KeyboardEventInit = {}) =>
    document.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...init }));
  const panes = () => document.querySelector<HTMLElement>('.panes')!;
  const tokyo = () => ({
    segments: [readingSeg(0, '東京都')],
    alternatives: [
      [readingSeg(0, '東京'), readingSeg(2, '都')],
      [readingSeg(0, '東', false), readingSeg(1, '京都')],
    ],
  });

  test('alternates appear as reading rows and a row switches the sentence', () => {
    emit('parse-result', tokyo());
    expect(rendered()).toEqual(['東京都']);

    expect(menuLabels().slice(0, 3)).toEqual(['Reading 1 of 3', 'Reading 2 of 3', 'Reading 3 of 3']);
    [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
      .find((b) => b.textContent === 'Reading 3 of 3')!
      .click();

    expect(rendered()).toEqual(['東', '京都']);
    expect(document.querySelector('.unmatched')?.textContent).toBe('東');
    // Reopened, the menu marks the reading now on screen.
    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const checked = [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
      .filter((b) => b.getAttribute('aria-checked') === 'true')
      .map((b) => b.textContent);
    expect(checked).toContain('Reading 3 of 3');
  });

  test('a result with no alternates offers no reading rows', () => {
    emit('parse-result', { segments: [readingSeg(0, '東京都')], alternatives: [] });
    expect(menuLabels().some((label) => label?.startsWith('Reading'))).toBe(false);
  });

  test('] and [ cycle the readings and wrap at both ends', () => {
    emit('parse-result', tokyo());
    press(']');
    expect(rendered()).toEqual(['東京', '都']);
    press(']');
    press(']');
    expect(rendered()).toEqual(['東京都']);
    press('[');
    expect(rendered()).toEqual(['東', '京都']);
  });

  test('a modified bracket is not a reading step', () => {
    // Alt+] and Alt+[ are history navigation, handled on the Rust side; the
    // webview still sees the keydown and must not also cycle readings.
    emit('parse-result', tokyo());
    press(']', { altKey: true });
    press('[', { ctrlKey: true });
    expect(rendered()).toEqual(['東京都']);
  });

  test('a new result starts at reading 1 again', () => {
    emit('parse-result', tokyo());
    press(']');
    emit('parse-result', {
      segments: [readingSeg(0, '東京'), readingSeg(2, '都')],
      alternatives: [[readingSeg(0, '東京都')]],
    });
    expect(rendered()).toEqual(['東京', '都']);
  });

  test('the popover reads the displayed reading, not the top one', async () => {
    emit('parse-result', tokyo());
    press(']');
    document
      .querySelector<HTMLElement>('.chip[data-start="2"]')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    const content = emitted.find(([event]) => event === 'popover-content');
    expect(content).toBeDefined();
    const payload = content![1] as { entries: { headword: string }[] };
    expect(payload.entries[0].headword).toBe('都');
  });

  test('a furigana change re-renders the reading on screen, not reading 1', () => {
    emit('parse-result', tokyo());
    press(']');
    emit('settings-changed', { furigana_mode: 'hiragana' });
    expect(rendered()).toEqual(['東京', '都']);
  });

  test('cycling keeps the scroll position; a new result returns to the top', () => {
    // Phase 2b design §6: a reading switch is made while reading one word,
    // and must not lose the reader's place. A different result is a new
    // sentence and starts at the top, as every render did before.
    emit('parse-result', tokyo());
    panes().scrollTop = 120;
    press(']');
    expect(panes().scrollTop).toBe(120);
    emit('parse-result', { segments: [readingSeg(0, '東京')], alternatives: [] });
    expect(panes().scrollTop).toBe(0);
  });

  test('a furigana change keeps the scroll position too', () => {
    emit('parse-result', tokyo());
    panes().scrollTop = 120;
    emit('settings-changed', { furigana_mode: 'hiragana' });
    expect(panes().scrollTop).toBe(120);
  });

  test('cycling keeps the keyboard position, by character and not by chip', () => {
    // The other half of the scroll guard above, and the more direct defect of
    // the two: `]`/`[` is a keyboard gesture, so replacing `#output`'s
    // children under a focused chip threw away the position of the very
    // reader who pressed the key. Reading 2 splits 東京都 into 東京 + 都, so
    // focus must land on the chip covering character 2 — 都, the second chip —
    // rather than on chip index 0 where it started.
    emit('parse-result', tokyo());
    document.querySelector<HTMLButtonElement>('[data-start="0"]')!.focus();
    press(']');
    expect(rendered()).toEqual(['東京', '都']);
    expect((document.activeElement as HTMLElement).dataset.start).toBe('0');

    document.querySelector<HTMLButtonElement>('[data-start="2"]')!.focus();
    press('[');
    expect(rendered()).toEqual(['東京都']);
    expect((document.activeElement as HTMLElement).dataset.start).toBe('0');
  });

  test('cycling onto an unmatched run lands on the pane, never on the body', () => {
    // Reading 3 is 東 (unmatched) + 京都, and an unmatched run is a <span>
    // with no tabindex on purpose — so there is no chip to return to. The
    // point of `#output`'s own `tabindex="-1"` is that focus stays in the
    // sentence pane instead of falling all the way to <body>, which is where
    // it went for every switch before this.
    emit('parse-result', tokyo());
    document.querySelector<HTMLButtonElement>('[data-start="0"]')!.focus();
    press('[');
    expect(rendered()).toEqual(['東', '京都']);
    expect(document.activeElement).toBe(document.querySelector('#output'));
  });

  test('a render nobody was focused on does not steal focus', () => {
    // The gate `renderDictionary` already applies: a parse arriving while the
    // reader is somewhere else entirely must not pull them into the pane.
    emit('parse-result', tokyo());
    const outside = document.querySelector<HTMLButtonElement>('#window-close')!;
    outside.focus();
    press(']');
    expect(document.activeElement).toBe(outside);
  });

  test('cycling readings from the keyboard closes an open context menu', () => {
    // `openContextMenu` moves focus into the menu, and a bracket keypress
    // there bubbles to `document` — without a close, the menu would keep
    // checking the reading the step just left behind.
    emit('parse-result', tokyo());
    menuLabels();
    expect(document.querySelector('.context-menu')).not.toBeNull();
    press(']');
    expect(document.querySelector('.context-menu')).toBeNull();
  });
});

// Residual 2 (re-review, final wave): the original fix wrapped the whole
// `Promise.all([...listen calls]).then(() => invoke('frontend_ready'))`
// chain in one `.catch(() => {})`, so a `listen()` rejection was swallowed
// exactly the same way a `frontend_ready` rejection was — but the comment
// only ever described the `frontend_ready` consequence. This proves the
// narrower catch: a `frontend_ready`-only failure degrades quietly (no
// unhandled rejection, listeners keep working), which is the one case this
// catch is meant to cover.
describe('the frontend_ready call', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    vi.resetModules();
  });

  test('a frontend_ready rejection is swallowed without disrupting the registered listeners', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'frontend_ready') return Promise.reject('frontend_ready unavailable');
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    // The rejection above did not become an unhandled rejection (vitest
    // would fail this test file if it had) and did not stop the listeners
    // from being registered or from rendering a result.
    emit('parse-result', {
      segments: [
        { start: 0, len: 1, surface: '本', reading: 'ほん', matched: true, entries: [] },
      ],
      alternatives: [],
    });
    expect(document.querySelector('.sentence')).not.toBeNull();
  });
});

describe('the startup reveal', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    show.mockReset();
    show.mockImplementation(() => Promise.resolve());
    vi.resetModules();
  });

  test('reveals the window once the restored settings have been applied', async () => {
    let settingsResolved = false;
    // Sampled inside `show` rather than after the await below: by then both
    // have happened either way, which is what let an unordered reveal pass.
    let resolvedWhenShown: boolean | null = null;
    show.mockImplementation(() => {
      resolvedWhenShown = settingsResolved;
      return Promise.resolve();
    });
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true }).then((v) => {
          settingsResolved = true;
          return v;
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    // The window is configured hidden, so nothing else reveals it.
    await vi.waitFor(() => {
      expect(show).toHaveBeenCalled();
    });
    // Ordering is the point: revealing first would paint the default toggle
    // states and then flip them into place.
    expect(resolvedWhenShown).toBe(true);
  });

});

describe('main: a startup failure reports itself', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    vi.resetModules();
  });

  test('startup_error resolving to a message renders it into #output', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'startup_error') return Promise.resolve('no dictionary index in /nowhere');
      // The header's own settings calls fire unconditionally on import, so
      // they need real (harmless) answers; anything else is the thing this
      // test is guarding against.
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'settings_warning') return Promise.resolve(null);
      // Same module-level fire-and-forget hazard as `settings_warning` above,
      // now that `showShortcutWarning` is one of the calls import triggers.
      if (cmd === 'shortcut_warning') return Promise.resolve(null);
      // showDictionaryScreen's own invoke call, awaited before this test's
      // assertions run — without an explicit answer here it falls through to
      // the reject below and turns `void showDictionaryScreen()` into an
      // unhandled rejection that fails the whole file.
      if (cmd === 'needs_dictionary') return Promise.resolve(false);
      if (cmd === 'hints_warning') return Promise.resolve(null);
      return Promise.reject('no other invoke call is expected in this test');
    });

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect(document.querySelector('.startup-error')?.textContent).toContain(
      'no dictionary index',
    );
  });

  test('startup_error resolving to null renders nothing', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'startup_error') return Promise.resolve(null);
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'settings_warning') return Promise.resolve(null);
      if (cmd === 'shortcut_warning') return Promise.resolve(null);
      // Same reason as the sibling test above: showDictionaryScreen must not
      // fall through to a rejection here.
      if (cmd === 'needs_dictionary') return Promise.resolve(false);
      if (cmd === 'hints_warning') return Promise.resolve(null);
      return Promise.reject('unused');
    });

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect(document.querySelector('.startup-error')).toBeNull();
  });
});

describe('the settings warning', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    vi.resetModules();
  });

  test('a non-null settings_warning renders into #parse-error and leaves #output alone', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'settings_warning') {
        return Promise.resolve('settings.json was corrupt; defaults were used');
      }
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });

    const { showSettingsWarning } = await import('./main');
    await showSettingsWarning();

    expect(document.querySelector('#parse-error')?.textContent).toContain(
      'settings.json was corrupt',
    );
    // The point of this assertion: a settings warning is cosmetic. Unlike a
    // fatal `startup_error` it must never touch `output` — nothing was parsed.
    expect(document.querySelector('#output')?.children).toHaveLength(0);
  });

  test('a null settings_warning renders nothing', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });

    const { showSettingsWarning } = await import('./main');
    await showSettingsWarning();

    expect(document.querySelector('#parse-error')?.children).toHaveLength(0);
  });

  test('a hotkey another app owns is reported in the window', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'shortcut_warning') {
        return Promise.resolve('These hotkeys are already in use: Alt+BracketLeft');
      }
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showShortcutWarning();

    expect(document.querySelector('#parse-error')?.textContent).toContain('Alt+BracketLeft');
  });

  test('a settings warning and a hotkey warning both survive', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'settings_warning') return Promise.resolve('settings.json could not be read');
      if (cmd === 'shortcut_warning') return Promise.resolve('hotkeys in use: Alt+BracketLeft');
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showSettingsWarning();
    await main.showShortcutWarning();

    const text = document.querySelector('#parse-error')?.textContent ?? '';
    expect(text).toContain('could not be read');
    expect(text).toContain('Alt+BracketLeft');
  });

  test('hints silently off is reported in the window', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') {
        return Promise.resolve('MeCab boundary hints are switched on but not loaded');
      }
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showHintsWarning();

    expect(document.querySelector('#parse-error')?.textContent).toContain('not loaded');
  });

  test('a hints warning and a hotkey warning both survive', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'hints_warning') return Promise.resolve('hints are switched on but not loaded');
      if (cmd === 'shortcut_warning') return Promise.resolve('hotkeys in use: Alt+BracketLeft');
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    const main = await import('./main');

    await main.showHintsWarning();
    await main.showShortcutWarning();

    const text = document.querySelector('#parse-error')?.textContent ?? '';
    expect(text).toContain('not loaded');
    expect(text).toContain('Alt+BracketLeft');
  });
});

describe('the first-run download screen', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'needs_dictionary') return Promise.resolve(true);
      if (cmd === 'download_dictionary') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    await Promise.resolve();
    await Promise.resolve();
  });

  test('offers a download when no dictionary is present', () => {
    expect(document.querySelector('#download')).not.toBeNull();
  });

  // Final review, Finding 3: without this, a screen-reader user who presses
  // Download hears nothing for the 15+ seconds the build takes, and nothing
  // again if it fails. `role="status"` is a native ARIA live region
  // (implicit `aria-live="polite"`), set once in the markup rather than
  // toggled per render, so this only needs to prove it is there at all.
  test('#dictionary is a live region so status changes are announced', () => {
    expect(document.querySelector('#dictionary')?.getAttribute('role')).toBe('status');
  });

  test('a status event replaces the button with the phase', () => {
    emit('dictionary-status', 'building');
    expect(document.querySelector('#dictionary')?.textContent).toContain('Building');
    expect(document.querySelector('#download')).toBeNull();
  });

  test('ready clears the screen', () => {
    emit('dictionary-status', 'ready');
    expect(document.querySelector('#dictionary')?.childElementCount).toBe(0);
  });

  // A failure must leave a working Retry, or the user relaunches for a problem
  // that reconnecting to wifi would have fixed.
  test('a failure shows the reason and a retry', () => {
    emit('dictionary-status', 'could not reach the server');
    expect(document.querySelector('#dictionary')?.textContent).toContain('could not reach');
    expect(document.querySelector('#download')?.textContent).toBe('Retry');
  });

  // Fix-round finding 2: every other test in this describe drives state
  // through emit(...), never a real click, so nothing proved the button's
  // own handler calls invoke('download_dictionary') at all, or that it
  // synchronously flips the screen to the downloading phase before that call
  // settles. Without this, a handler that called the wrong command, or one
  // that forgot the immediate `renderDictionary('downloading')`, would still
  // pass the whole file.
  test('clicking download calls download_dictionary and shows the downloading phase', async () => {
    const button = document.querySelector<HTMLButtonElement>('#download');
    if (button === null) throw new Error('#download missing');

    button.click();
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith('download_dictionary');
    expect(document.querySelector('#dictionary')?.textContent).toContain('Downloading');
    // The offer button is gone — replaced by the phase view, not left behind.
    expect(document.querySelector('#download')).toBeNull();
  });

  // Fix-round finding 2: proves the `.catch((e) => renderDictionary(String(e)))`
  // branch actually renders the rejection and leaves a Retry that works, not
  // just that `emit('dictionary-status', <failure string>)` can — nothing
  // upstream of that emit (a real invoke rejection) was ever exercised.
  test('a rejected download renders the failure and a working retry', async () => {
    let downloadCalls = 0;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'download_dictionary') {
        downloadCalls += 1;
        // First attempt fails; Retry's second attempt succeeds.
        return downloadCalls === 1
          ? Promise.reject('could not reach the server')
          : Promise.resolve(undefined);
      }
      return Promise.resolve(null);
    });

    const button = document.querySelector<HTMLButtonElement>('#download');
    if (button === null) throw new Error('#download missing');

    button.click();
    // Flushes the rejection and the .catch(...) render it triggers.
    await Promise.resolve();
    await Promise.resolve();

    expect(document.querySelector('#dictionary')?.textContent).toContain(
      'could not reach the server',
    );
    const retry = document.querySelector<HTMLButtonElement>('#download');
    expect(retry?.textContent).toBe('Retry');

    retry?.click();
    await Promise.resolve();

    expect(downloadCalls).toBe(2);
  });
});

// Phase 2H: the clipboard is the only user-facing input path. `#text` and
// `#parse` were the two elements a user could type into or click to parse, so
// their absence is what makes that claim true. Asserted on the rendered shell
// rather than on the source, because the shell is what a user gets.
describe('the input surface', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  test('renders no manual text input', () => {
    expect(document.querySelector('#text')).toBeNull();
    expect(document.querySelector('#parse')).toBeNull();
    expect(document.querySelector('.input-row')).toBeNull();
  });
});

describe('the header drag region', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  test('header carries data-tauri-drag-region attribute', () => {
    const header = document.querySelector<HTMLElement>('header.controls');
    expect(header?.hasAttribute('data-tauri-drag-region')).toBe(true);
  });
});

describe('the titlebar band', () => {
  function load(mode: ChromeMode, peekGrowsFrame = false, platformName?: string) {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: mode });
      if (cmd === 'is_macos') return Promise.resolve(peekGrowsFrame);
      if (cmd === 'peek_grows_frame') return Promise.resolve(peekGrowsFrame);
      if (cmd === 'platform') return Promise.resolve(platformName ?? (peekGrowsFrame ? 'macos' : 'windows'));
      return Promise.resolve(null);
    });
    vi.resetModules();
    return import('./main');
  }

  // The reveal trigger is the whole window: the cursor anywhere inside it
  // shows the title bar, and only leaving hides it.
  function shell(): HTMLElement {
    const el = document.querySelector<HTMLElement>('#app');
    if (el === null) throw new Error('#app missing');
    return el;
  }

  test('mirrors platform returned from backend onto #app', async () => {
    await load('native', false, 'linux');
    await Promise.resolve();
    await Promise.resolve();
    expect(document.querySelector('#app')?.getAttribute('data-platform')).toBe('linux');
  });

  // The attribute is the whole state machine: CSS keys the reserved row, the
  // overlay, and the reveal off it, and the pointer handlers below read it to
  // decide whether the native chrome is theirs to peek at.
  test('mirrors the loaded chrome mode onto #app', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    expect(document.querySelector('#app')?.getAttribute('data-chrome')).toBe('header');
  });

  // The band boots decorated and the attribute is set synchronously, before
  // `get_settings` resolves. The CSS keys the frameless rules off
  // `:not([data-chrome='native'])`, so an absent attribute would match them and
  // flash the frameless shape for a frame.
  test('#app carries a chrome mode before any settings arrive', async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    // Nothing the backend is asked ever settles, so the boot call at module
    // scope is the only thing that can have written the attribute.
    invoke.mockImplementation(() => new Promise(() => {}));
    vi.resetModules();
    await import('./main');
    expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('native');
  });

  test('a settings-changed event moves the attribute with it', async () => {
    await load('native');
    await Promise.resolve();
    await Promise.resolve();
    expect(document.querySelector('#app')?.getAttribute('data-chrome')).toBe('native');

    emit('settings-changed', { chrome_mode: 'header' });

    expect(document.querySelector('#app')?.getAttribute('data-chrome')).toBe('header');
  });

  test('hovering the band peeks the native chrome while the title bar is hidden', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointerenter'));
    expect(invoke).toHaveBeenCalledWith('peek_titlebar', { visible: true, height: 28 });

    vi.useFakeTimers();
    try {
      shell().dispatchEvent(new Event('pointerleave'));
      vi.advanceTimersByTime(1000);
    } finally {
      vi.useRealTimers();
    }
    expect(invoke).toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });
  });

  test('a peek never rewrites the persisted setting', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    vi.useFakeTimers();
    try {
      shell().dispatchEvent(new Event('pointerenter'));
      shell().dispatchEvent(new Event('pointerleave'));
      vi.advanceTimersByTime(1000);
    } finally {
      vi.useRealTimers();
    }

    expect(invoke).not.toHaveBeenCalledWith('save_settings', expect.anything());
    expect(invoke).not.toHaveBeenCalledWith('set_chrome_mode', expect.anything());
  });

  // With the title bar shown there is nothing to reveal — the chrome is
  // already there, and peeking would hide it on the way out.
  test('hovering does nothing while the title bar is shown', async () => {
    await load('native');
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointerenter'));
    shell().dispatchEvent(new Event('pointerleave'));

    expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', expect.anything());
  });

  // `pointerenter` fires once per crossing in a real browser, but a stray
  // repeat (or a settings change mid-hover) must not queue a second objc
  // round-trip: only transitions talk to the backend.
  test('repeated enters do not re-invoke the peek', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointerenter'));
    shell().dispatchEvent(new Event('pointerenter'));

    const peeks = invoke.mock.calls.filter((c: unknown[]) => c[0] === 'peek_titlebar');
    expect(peeks).toHaveLength(1);
  });

  // The blink this replaced: peeking used to hand the strip to the OS, whose
  // traffic lights then swallowed the pointer, firing `pointerleave` and
  // undoing the peek at pointer rate. Ownership no longer moves, but the
  // lights still sit over the band's left edge, so the hide is delayed and
  // cancelled by a re-entry — one crossing over a button can no longer
  // collapse the reveal.
  test('a re-entry within the grace period cancels the hide entirely', async () => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      shell().dispatchEvent(new Event('pointerleave'));
      vi.advanceTimersByTime(400);
      shell().dispatchEvent(new Event('pointerenter'));
      vi.advanceTimersByTime(3000);

      expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });
      const peeks = invoke.mock.calls.filter((c: unknown[]) => c[0] === 'peek_titlebar');
      expect(peeks).toHaveLength(1);
    } finally {
      vi.useRealTimers();
    }
  });

  test('leaving for good hides the chrome once the grace period elapses', async () => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      shell().dispatchEvent(new Event('pointerleave'));
      expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });

      vi.advanceTimersByTime(1000);

      expect(invoke).toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });
    } finally {
      vi.useRealTimers();
    }
  });

  // CSS reveals the band on `:hover`, but the pointer sitting on a traffic
  // light is NOT hovering the band as far as the webview is concerned. The
  // class is what keeps the strip open across that gap.
  test('#app carries a peeked class for as long as the chrome is up', async () => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      expect(shell().classList.contains('peeked')).toBe(true);

      shell().dispatchEvent(new Event('pointerleave'));
      expect(shell().classList.contains('peeked')).toBe(true);

      vi.advanceTimersByTime(1000);
      expect(shell().classList.contains('peeked')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  // The colour the Windows frame growth erases newly exposed pixels with. A
  // wrong one is visible as a coloured strip for the frame or two before the
  // webview paints, which is why the renderer reports its own.
  test('startup reports the background colour the page renders', async () => {
    document.body.style.backgroundColor = 'rgb(18, 18, 18)';
    try {
      await load('header', true);
      await Promise.resolve();
      await Promise.resolve();

      // `opaque` is the *effective* alpha, not the stored one: this fixture
      // persists none, so the stylesheet default of 1 applies and the Windows
      // erase fill stays enabled.
      expect(invoke).toHaveBeenCalledWith('set_window_background', {
        r: 18,
        g: 18,
        b: 18,
        opaque: true,
      });
    } finally {
      document.body.style.backgroundColor = '';
    }
  });

  test('peeking adds the peek-offset class when the frame can grow', async () => {
    vi.useFakeTimers();
    try {
      await load('header', true);
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      expect(shell().classList.contains('peeked')).toBe(true);
      expect(shell().classList.contains('peek-offset')).toBe(true);

      shell().dispatchEvent(new Event('pointerleave'));
      vi.advanceTimersByTime(1000);
      expect(shell().classList.contains('peeked')).toBe(false);
      expect(shell().classList.contains('peek-offset')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  test('peeking adds only peeked when the frame cannot grow', async () => {
    vi.useFakeTimers();
    try {
      // Linux: the only platform left whose frame does not grow on peek.
      await load('header', false, 'linux');
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      expect(shell().classList.contains('peeked')).toBe(true);
      expect(shell().classList.contains('peek-offset')).toBe(false);

      shell().dispatchEvent(new Event('pointerleave'));
      vi.advanceTimersByTime(1000);
      expect(shell().classList.contains('peeked')).toBe(false);
      expect(shell().classList.contains('peek-offset')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  test('pointermove inside the window also reveals the band', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointermove'));
    expect(shell().classList.contains('peeked')).toBe(true);
  });

  test('unfocused-mouse-move reveals the band and unfocused-mouse-leave hides it', async () => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();

      emit('unfocused-mouse-move', { x: 50, y: 10, screen_x: 100, screen_y: 100 });
      expect(shell().classList.contains('peeked')).toBe(true);

      emit('unfocused-mouse-leave', {});
      vi.advanceTimersByTime(1000);
      expect(shell().classList.contains('peeked')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  // The peek grows the window by 28px. The geometry debounce watches
  // `tauri://resize`, so without this the persisted height would creep a band
  // taller on every hover and the window would grow across launches.
  test('a resize seen while peeked is not persisted', async () => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();

      shell().dispatchEvent(new Event('pointerenter'));
      emit('tauri://resize', {});
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();

      expect(invoke).not.toHaveBeenCalledWith('save_window_geometry', expect.anything());
    } finally {
      vi.useRealTimers();
    }
  });

  // Leaving the band hides the chrome again; if the user then turns the title
  // bar back on, the shown state must not stay stuck behind a stale peek.
  test('turning the title bar back on while hovered leaves the chrome shown', async () => {
    await load('header');
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointerenter'));
    emit('settings-changed', { chrome_mode: 'native' });
    shell().dispatchEvent(new Event('pointerleave'));

    expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });
  });

  test('content mode does not peek', async () => {
    await load('content', false, 'macos');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('content'));

    // The reveal is header mode's alone. happy-dom resolves no layout, so the
    // proof here is the absent backend call and the absent class, not a height.
    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(false);
    expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', expect.anything());
  });

  test('header mode still peeks', async () => {
    await load('header', false, 'macos');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('header'));

    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(true);
    expect(invoke).toHaveBeenCalledWith('peek_titlebar', { visible: true, height: 28 });
  });

  test('switching to content mid-peek drops the peek', async () => {
    await load('header', false, 'macos');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('header'));
    appEl.dispatchEvent(new Event('pointerenter'));
    expect(appEl.classList.contains('peeked')).toBe(true);

    emit('settings-changed', { chrome_mode: 'content' });
    expect(appEl.classList.contains('peeked')).toBe(false);
    expect(appEl.classList.contains('peek-offset')).toBe(false);
  });

  // The history pair sits ahead of the gear, so it is the first tab stop in
  // the band. In frameless idle mode that stop starts at opacity 0 — an
  // invisible tab stop is a WCAG 2.4.7 trap unless focus peeks the chrome the
  // same way the gear already does.
  test.each([
    ['#history-back', () => document.querySelector<HTMLButtonElement>('#history-back')!],
    ['#history-forward', () => document.querySelector<HTMLButtonElement>('#history-forward')!],
  ])('focusing %s peeks the chrome, and blur hides it after the grace period', async (_name, getButton) => {
    vi.useFakeTimers();
    try {
      await load('header');
      await Promise.resolve();
      await Promise.resolve();
      // Disabled buttons take no focus, so a fresh launch cannot reveal the
      // trap — it only shows once there is history to walk.
      getButton().disabled = false;

      getButton().dispatchEvent(new Event('focus'));
      expect(shell().classList.contains('peeked')).toBe(true);

      getButton().dispatchEvent(new Event('blur'));
      expect(shell().classList.contains('peeked')).toBe(true);

      vi.advanceTimersByTime(1000);
      expect(shell().classList.contains('peeked')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  test('a right-click opens the chrome menu and suppresses the default one', async () => {
    await load('content', false, 'macos');
    await vi.waitFor(() =>
      expect(document.querySelector('#app')!.getAttribute('data-chrome')).toBe('content'),
    );

    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(true);

    const labels = [...document.querySelectorAll('.context-menu button')].map(
      (b) => b.textContent,
    );
    expect(labels).toEqual([
      'Native frame',
      'Header + resize edges',
      'Content only',
      'Settings…',
      'Quit',
    ]);
    expect(
      document
        .querySelector('.context-menu button[aria-checked="true"]')!
        .textContent,
    ).toBe('Content only');
  });

  test('the menu omits the native mode on Windows', async () => {
    await load('header', false, 'windows');
    await vi.waitFor(() =>
      expect(document.querySelector('#app')!.getAttribute('data-platform')).toBe('windows'),
    );

    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const labels = [...document.querySelectorAll('.context-menu button')].map(
      (b) => b.textContent,
    );
    expect(labels).not.toContain('Native frame');
    expect(labels).toContain('Header + resize edges');
  });

  test('picking a mode from the menu applies, persists and broadcasts it', async () => {
    emitted.length = 0;
    await load('native', false, 'macos');
    const appEl = document.querySelector('#app')!;
    await vi.waitFor(() => expect(appEl.getAttribute('data-chrome')).toBe('native'));

    document.body.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true }));
    const item = [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')].find(
      (b) => b.textContent === 'Content only',
    )!;
    item.click();

    // Applied locally as well as broadcast: the emit round-trip is async, and
    // the mode should change on the frame the user clicked.
    expect(appEl.getAttribute('data-chrome')).toBe('content');
    expect(invoke).toHaveBeenCalledWith('set_chrome_mode', { mode: 'content' });
    expect(emitted.at(-1)).toEqual(['settings-changed', { chrome_mode: 'content' }]);
  });

  test('Settings… and Quit route to their existing commands', async () => {
    await load('content', false, 'macos');
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith('get_settings'));

    const open = (label: string) => {
      document.body.dispatchEvent(
        new MouseEvent('contextmenu', { bubbles: true, cancelable: true }),
      );
      [...document.querySelectorAll<HTMLButtonElement>('.context-menu button')]
        .find((b) => b.textContent === label)!
        .click();
    };

    open('Settings…');
    expect(invoke).toHaveBeenCalledWith('open_settings_window');
    open('Quit');
    expect(invoke).toHaveBeenCalledWith('close_window');
  });

  /** Like `load`, but with a `surface_alpha` in the settings snapshot. */
  function loadWithAlpha(mode: ChromeMode, alpha?: number) {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings')
        return Promise.resolve({ chrome_mode: mode, surface_alpha: alpha });
      if (cmd === 'platform') return Promise.resolve('macos');
      if (cmd === 'is_macos') return Promise.resolve(true);
      if (cmd === 'peek_grows_frame') return Promise.resolve(true);
      return Promise.resolve(null);
    });
    vi.resetModules();
    return import('./main');
  }

  function alphaToken(): string {
    return document.documentElement.style.getPropertyValue('--surface-alpha');
  }

  test('a persisted alpha reaches the root element as a ratio', async () => {
    await loadWithAlpha('header', 62);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('0.62');
  });

  // Zero is a real setting, and `''` would mean "unset" — the two must not
  // collapse into each other on the way to the stylesheet.
  test('a zero alpha is written, not treated as unset', async () => {
    await loadWithAlpha('content', 0);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('0');
  });

  test('an absent alpha leaves the property off so the stylesheet owns it', async () => {
    await loadWithAlpha('header', undefined);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('');
  });

  // The clamp, half one: an opaque OS caption over a see-through body is an
  // artifact, so native renders fully opaque whatever is stored.
  test('native chrome clamps the applied alpha to full opacity', async () => {
    await loadWithAlpha('native', 20);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('1');
  });

  // The clamp, half two, and the reason it is applied rather than persisted:
  // the user's value has to come back when they leave native.
  test('leaving native restores the stored alpha', async () => {
    await loadWithAlpha('native', 20);
    await Promise.resolve();
    await Promise.resolve();
    expect(alphaToken()).toBe('1');

    emit('settings-changed', { chrome_mode: 'header' });
    await Promise.resolve();
    expect(alphaToken()).toBe('0.2');
  });

  test('an alpha arriving by event is applied without a reload', async () => {
    await loadWithAlpha('header', 100);
    await Promise.resolve();
    await Promise.resolve();

    emit('settings-changed', { surface_alpha: 35 });
    await Promise.resolve();
    expect(alphaToken()).toBe('0.35');
  });

  test('a partial payload does not wipe the font-size tokens', async () => {
    await loadWithAlpha('header', 100);
    await Promise.resolve();
    await Promise.resolve();

    emit('settings-changed', {
      always_on_top: false,
      clipboard_monitoring: true,
      font_sentence: 30,
      font_gloss: 18,
      furigana_scale: 70,
    });
    await Promise.resolve();
    expect(document.documentElement.style.getPropertyValue('--text-sentence')).toBe('30px');

    // The opacity slider and the right-click menu both emit one key alone.
    emit('settings-changed', { surface_alpha: 40 });
    await Promise.resolve();
    expect(document.documentElement.style.getPropertyValue('--text-sentence')).toBe('30px');

    emit('settings-changed', { chrome_mode: 'content' });
    await Promise.resolve();
    expect(document.documentElement.style.getPropertyValue('--text-sentence')).toBe('30px');
  });

  // The other half of the guard: a full snapshot with a size missing means the
  // user cleared that box, and the token must go so `tokens.css` owns it.
  test('a full snapshot with a cleared size still clears the token', async () => {
    await loadWithAlpha('header', 100);
    await Promise.resolve();
    await Promise.resolve();

    emit('settings-changed', {
      always_on_top: false,
      clipboard_monitoring: true,
      font_sentence: 30,
    });
    await Promise.resolve();
    expect(document.documentElement.style.getPropertyValue('--text-sentence')).toBe('30px');

    emit('settings-changed', { always_on_top: false, clipboard_monitoring: true });
    await Promise.resolve();
    expect(document.documentElement.style.getPropertyValue('--text-sentence')).toBe('');
  });

  // `getComputedStyle` resolves no rule in this DOM without a real
  // stylesheet (the CSS import is a no-op under `vitest`'s default `css:
  // false`), so — exactly like the "startup reports the background colour"
  // fixture above — the body needs an explicit inline colour before
  // `reportBackground` has anything to parse and report.
  test('a see-through surface tells the backend to stop filling', async () => {
    document.body.style.backgroundColor = 'rgb(18, 18, 18)';
    try {
      await loadWithAlpha('header', 40);
      await Promise.resolve();
      await Promise.resolve();

      const calls = invoke.mock.calls.filter(([cmd]) => cmd === 'set_window_background');
      expect(calls.length).toBeGreaterThan(0);
      expect(calls[calls.length - 1][1]).toMatchObject({ opaque: false });
    } finally {
      document.body.style.backgroundColor = '';
    }
  });

  // Native renders fully opaque whatever is stored, and that is exactly the
  // mode whose erase fill stops the top-edge flash. Reading the stored value
  // here would switch the fill off in the one mode that needs it.
  test('native keeps the fill enabled despite a low stored alpha', async () => {
    document.body.style.backgroundColor = 'rgb(18, 18, 18)';
    try {
      await loadWithAlpha('native', 20);
      await Promise.resolve();
      await Promise.resolve();

      const calls = invoke.mock.calls.filter(([cmd]) => cmd === 'set_window_background');
      expect(calls[calls.length - 1][1]).toMatchObject({ opaque: true });
    } finally {
      document.body.style.backgroundColor = '';
    }
  });
});

describe('window geometry save on resize/move', () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true, chrome_mode: 'native' });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  test('debounces save_window_geometry on move and resize events', async () => {
    emit('tauri://resize', {});
    emit('tauri://move', {});
    expect(invoke).not.toHaveBeenCalledWith('save_window_geometry', expect.anything());

    vi.advanceTimersByTime(300);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith('save_window_geometry', {
      width: 800,
      height: 600,
      x: 100,
      y: 100,
    });
  });

  test('scale-change event invokes update_clip_region and schedules geometry save', async () => {
    emit('tauri://scale-change', {});
    expect(invoke).toHaveBeenCalledWith('update_clip_region');

    vi.advanceTimersByTime(300);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(invoke).toHaveBeenCalledWith('save_window_geometry', {
      width: 800,
      height: 600,
      x: 100,
      y: 100,
    });
  });
});

describe('furigana mode reaching the render', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    vi.resetModules();
  });

  // The picker itself lives in the settings window (settings.test.ts covers
  // it); what the main window still owns is redrawing the sentence when the
  // mode changes under it, and honouring a mode restored at startup.
  test('re-renders the sentence when a settings-changed event switches mode', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ furigana_mode: 'none' });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    emit('parse-result', {
      segments: [
        {
          start: 0,
          len: 2,
          surface: '東京',
          reading: 'とうきょう',
          matched: true,
          entries: [
            {
              headword: '東京',
              reading: 'とうきょう',
              conjugation: null,
              pos: ['n'],
              senses: [],
              flags: ['primary'],
            },
          ],
        },
      ],
      alternatives: [],
    });

    expect(document.querySelector('.chip ruby')).toBeNull();

    emit('settings-changed', { furigana_mode: 'hiragana' });
    const rubyH = document.querySelector('.chip ruby');
    expect(rubyH?.querySelector('rt')?.textContent).toBe('とうきょう');

    emit('settings-changed', { furigana_mode: 'katakana' });
    expect(document.querySelector('.chip ruby rt')?.textContent).toBe('トウキョウ');

    emit('settings-changed', { furigana_mode: 'romaji' });
    expect(document.querySelector('.chip ruby rt.romaji')?.textContent).toBe('toukyou');

    emit('settings-changed', { furigana_mode: 'none' });
    expect(document.querySelector('.chip ruby')).toBeNull();
    expect(document.querySelector('.chip')?.textContent).toBe('東京');
  });

  test('a mode restored by get_settings is used by the first render', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ furigana_mode: 'katakana' });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    emit('parse-result', {
      segments: [
        {
          start: 0,
          len: 2,
          surface: '東京',
          reading: 'とうきょう',
          matched: true,
          entries: [],
        },
      ],
      alternatives: [],
    });

    expect(document.querySelector('.chip ruby rt')?.textContent).toBe('トウキョウ');
  });
});

describe('settings window and gloss filters', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    vi.resetModules();
  });

  test('clicking settings toggle button invokes open_settings_window', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const toggle = document.querySelector<HTMLButtonElement>('#settings-toggle');
    expect(toggle).not.toBeNull();

    toggle?.click();
    expect(invoke).toHaveBeenCalledWith('open_settings_window');
  });

  test('settings-changed event updates the filters the main window sends onward', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          hide_pos: false,
          hide_xrefs: false,
          hide_usage: false,
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    // Simulate settings-changed event from Settings Window
    emit('settings-changed', {
      furigana_mode: 'none',
      hide_pos: true,
      hide_xrefs: true,
      hide_usage: false,
    });

    const entries = [
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

    // The filters are not observable directly; the popover payload is what
    // carries them, so that is what proves the event was applied.
    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    expect(emitted).toContainEqual([
      'popover-content',
      { entries, filters: { hide_pos: true, hide_xrefs: true, hide_usage: false, hide_kanji: false } },
    ]);
  });

  test('openFor sends current active filters in popover-content event', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
          hide_pos: true,
          hide_xrefs: false,
          hide_usage: true,
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const sampleEntries = [
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
        {
          start: 0,
          len: 2,
          surface: '東京',
          reading: 'とうきょう',
          matched: true,
          entries: sampleEntries,
        },
      ],
      alternatives: [],
    });

    const chip = document.querySelector<HTMLElement>('.chip');
    chip?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    expect(emitted).toContainEqual([
      'popover-content',
      {
        entries: sampleEntries,
        filters: {
          hide_pos: true,
          hide_xrefs: false,
          hide_usage: true,
          hide_kanji: false,
        },
      },
    ]);
  });

  test('clicking window controls invokes corresponding commands', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const min = document.querySelector<HTMLButtonElement>('#window-minimize');
    const close = document.querySelector<HTMLButtonElement>('#window-close');
    const header = document.querySelector<HTMLElement>('header.controls');

    min?.click();
    expect(invoke).toHaveBeenCalledWith('minimize_window');

    close?.click();
    expect(invoke).toHaveBeenCalledWith('close_window');

    header?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(invoke).toHaveBeenCalledWith('toggle_maximize_window');
    expect(min).not.toBeNull();
    expect(close).not.toBeNull();
  });

  test('pressing mouse on resize handles triggers startResizeDragging with direction', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          chrome_mode: 'native',
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    startResizeDragging.mockClear();
    const southHandle = document.querySelector<HTMLElement>('.resize-bottom');
    southHandle?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    expect(startResizeDragging).toHaveBeenCalledWith('South');

    startResizeDragging.mockClear();
    const eastHandle = document.querySelector<HTMLElement>('.resize-right');
    eastHandle?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    expect(startResizeDragging).toHaveBeenCalledWith('East');

    startResizeDragging.mockClear();
    const seHandle = document.querySelector<HTMLElement>('.resize-bottom-right');
    seHandle?.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 }));
    expect(startResizeDragging).toHaveBeenCalledWith('SouthEast');
  });
});




describe('font sizes reaching the root element', () => {
  function load(settings: Record<string, unknown>) {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      return Promise.resolve(null);
    });
    vi.resetModules();
    return import('./main');
  }

  function token(name: string): string {
    return document.documentElement.style.getPropertyValue(name);
  }

  beforeEach(() => {
    // The root element outlives module resets, so a size set by one test would
    // otherwise be read back as a pass by the next.
    document.documentElement.removeAttribute('style');
  });

  test('applies the persisted sizes on load', async () => {
    await load({ font_sentence: 30, font_gloss: 18, furigana_scale: 70 });
    await Promise.resolve();
    await Promise.resolve();

    expect(token('--text-sentence')).toBe('30px');
    expect(token('--text-gloss')).toBe('18px');
    expect(token('--text-furigana')).toBe('0.7em');
    expect(token('--text-furigana-romaji')).toBe('0.63em');
  });

  // Absent means "whatever the stylesheet says", so the app must not write a
  // second copy of the default over the top of `tokens.css`.
  test('leaves the tokens alone when no size is persisted', async () => {
    await load({ chrome_mode: 'native' });
    await Promise.resolve();
    await Promise.resolve();

    expect(token('--text-sentence')).toBe('');
    expect(token('--text-gloss')).toBe('');
  });

  test('a settings-changed event restyles the root', async () => {
    await load({ font_sentence: 24 });
    await Promise.resolve();
    await Promise.resolve();
    expect(token('--text-sentence')).toBe('24px');

    // A real settings-changed snapshot always carries these two non-optional
    // `Settings` keys — that pair is what marks it a full snapshot rather
    // than a partial payload like `{ surface_alpha }` or `{ chrome_mode }`.
    emit('settings-changed', {
      always_on_top: false,
      clipboard_monitoring: true,
      font_sentence: 36,
      furigana_scale: 50,
    });

    expect(token('--text-sentence')).toBe('36px');
    expect(token('--text-furigana')).toBe('0.5em');
  });

  // Clearing a size box in the settings window sends a snapshot with the key
  // absent. That has to put the token back to the stylesheet default, not
  // leave the last value stuck on the root until the next restart.
  test('a settings-changed event that drops a size clears the token', async () => {
    await load({ font_sentence: 36, furigana_scale: 70 });
    await Promise.resolve();
    await Promise.resolve();
    expect(token('--text-sentence')).toBe('36px');

    emit('settings-changed', {
      always_on_top: false,
      clipboard_monitoring: true,
      furigana_scale: 70,
    });

    expect(token('--text-sentence')).toBe('');
    expect(token('--text-furigana')).toBe('0.7em');
  });

  test('clamps a size a hand-edited settings file put out of range', async () => {
    await load({ font_sentence: 900 });
    await Promise.resolve();
    await Promise.resolve();

    expect(token('--text-sentence')).toBe('48px');
  });

  test('one restore snapshot moves the alpha, the sizes, the furigana and the chrome', () => {
    const root = document.documentElement;

    // A stale gloss token and a rendered chip both have to be overwritten by
    // the restore, not merely absent-by-coincidence — otherwise the
    // assertions below would pass whether or not the listener does anything.
    root.style.setProperty('--text-gloss', '99px');
    emit('parse-result', {
      segments: [
        {
          start: 0,
          len: 2,
          surface: '東京',
          reading: 'とうきょう',
          matched: true,
          entries: [],
        },
      ],
      alternatives: [],
    });
    expect(document.querySelector('.chip ruby')).toBeNull();

    // Exactly what `layout::restore` emits: a full `Settings`, with the keys a
    // slot recorded as unset simply absent.
    emit('settings-changed', {
      always_on_top: true,
      clipboard_monitoring: true,
      chrome_mode: 'content' as ChromeMode,
      surface_alpha: 35,
      font_sentence: 30,
      furigana_scale: 70,
      furigana_mode: 'hiragana',
    });

    expect(root.style.getPropertyValue('--surface-alpha')).toBe('0.35');
    // Both sizes the payload carries — the sentence box and the furigana
    // scale — not just the one this test used to single out.
    expect(root.style.getPropertyValue('--text-sentence')).toBe('30px');
    expect(root.style.getPropertyValue('--text-furigana')).toBe('0.7em');
    expect(document.querySelector('#app')?.getAttribute('data-chrome')).toBe('content');
    // `--text-gloss` was set above, so this proves the absent key clears a
    // real value rather than reading back a property that was never set —
    // the half of the contract that depends on serde omitting the key.
    expect(root.style.getPropertyValue('--text-gloss')).toBe('');
    // The mode reaches the screen through `display`'s re-render, so this is
    // the only assertion that would actually catch `furigana_mode` silently
    // failing to propagate on a restore snapshot.
    expect(document.querySelector('.chip ruby rt')?.textContent).toBe('とうきょう');
  });
});

describe('the gloss size reaching the popover window', () => {
  const sampleEntries = [
    {
      headword: '東京',
      reading: 'とうきょう',
      conjugation: null,
      pos: ['n'],
      senses: [],
      flags: ['primary'],
    },
  ];

  async function hoverChip(settings: Record<string, unknown>) {
    document.body.innerHTML = '<main id="app"></main>';
    document.documentElement.removeAttribute('style');
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve(settings);
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    emit('parse-result', {
      segments: [
        {
          start: 0,
          len: 2,
          surface: '東京',
          reading: 'とうきょう',
          matched: true,
          entries: sampleEntries,
        },
      ],
      alternatives: [],
    });
    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();
  }

  // The popover is a second webview with its own `:root`, so the size has to
  // travel with the content — and it has to arrive before the render, because
  // the window sizes itself off what that render measures.
  test('carries the persisted gloss size in the payload', async () => {
    await hoverChip({ font_gloss: 18 });

    expect(emitted).toContainEqual([
      'popover-content',
      expect.objectContaining({ gloss: 18 }),
    ]);
  });

  test('clamps the gloss size before sending it', async () => {
    await hoverChip({ font_gloss: 400 });

    expect(emitted).toContainEqual([
      'popover-content',
      expect.objectContaining({ gloss: 28 }),
    ]);
  });

  // No size persisted means the popover's own stylesheet decides, so the key
  // must be absent rather than carrying a duplicated default.
  test('stops sending the size once it is cleared', async () => {
    await hoverChip({ font_gloss: 18 });
    expect(emitted).toContainEqual(['popover-content', expect.objectContaining({ gloss: 18 })]);

    emitted.length = 0;
    // A real settings-changed snapshot always carries these two non-optional
    // `Settings` keys — without them the payload reads as partial and the
    // font-size guard (see `main.ts`) would leave the cleared size untouched.
    emit('settings-changed', { always_on_top: false, clipboard_monitoring: true, hide_pos: false });
    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    expect(emitted).toContainEqual([
      'popover-content',
      {
        entries: sampleEntries,
        filters: { hide_pos: false, hide_xrefs: false, hide_usage: false, hide_kanji: false },
      },
    ]);
  });

  test('omits the size when none is persisted', async () => {
    await hoverChip({ chrome_mode: 'native' });

    expect(emitted).toContainEqual([
      'popover-content',
      {
        entries: sampleEntries,
        filters: { hide_pos: false, hide_xrefs: false, hide_usage: false, hide_kanji: false },
      },
    ]);
  });
});

describe('the header history buttons', () => {
  /** A parse whose surface (and, optionally, reading/entries) distinguish it. */
  function parse(
    surface: string,
    opts: { reading?: string | null; entries?: unknown[] } = {},
  ) {
    return {
      segments: [
        {
          start: 0,
          len: surface.length,
          surface,
          reading: opts.reading ?? null,
          matched: true,
          entries: opts.entries ?? [],
        },
      ],
      alternatives: [],
    };
  }

  function historyBack(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#history-back')!;
  }

  function historyForward(): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>('#history-forward')!;
  }

  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({});
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    await Promise.resolve();
    await Promise.resolve();
  });

  test('both buttons start disabled', () => {
    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(true);
  });

  test('a single parse leaves both buttons disabled', () => {
    emit('parse-result', parse('猫'));

    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(true);
  });

  test('a second parse enables Back only', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));

    expect(historyBack().disabled).toBe(false);
    expect(historyForward().disabled).toBe(true);
  });

  test('Back renders the previous parse and flips both disabled states', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));

    historyBack().click();

    expect(document.querySelector('.chip')?.textContent).toBe('猫');
    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(false);
  });

  test('Forward returns to the newer parse', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    historyBack().click();

    historyForward().click();

    expect(document.querySelector('.chip')?.textContent).toBe('犬');
    expect(historyBack().disabled).toBe(false);
    expect(historyForward().disabled).toBe(true);
  });

  // segmentAt resolves a hover against `lastResult`, which `display` — not
  // `show` — writes. If a nav step failed to update it, this would find the
  // newest parse's entries instead of the one actually on screen.
  test('a hover after navigating emits popover-content for the displayed parse entries', async () => {
    const catEntries = [
      { headword: '猫', reading: 'ねこ', conjugation: null, pos: ['n'], senses: [], flags: [] },
    ];
    const dogEntries = [
      { headword: '犬', reading: 'いぬ', conjugation: null, pos: ['n'], senses: [], flags: [] },
    ];
    emit('parse-result', parse('猫', { entries: catEntries }));
    emit('parse-result', parse('犬', { entries: dogEntries }));
    historyBack().click();

    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    await flushLookup();

    expect(emitted).toContainEqual([
      'popover-content',
      expect.objectContaining({ entries: catEntries }),
    ]);
  });

  test('a new parse takes over the view and keeps the entries ahead of the cursor', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    historyBack().click(); // viewing 猫, with 犬 ahead of the cursor

    emit('parse-result', parse('鳥'));

    expect(document.querySelector('.chip')?.textContent).toBe('鳥');
    expect(historyBack().disabled).toBe(false);
    expect(historyForward().disabled).toBe(true);

    historyBack().click();
    expect(document.querySelector('.chip')?.textContent).toBe('犬');
    historyBack().click();
    expect(document.querySelector('.chip')?.textContent).toBe('猫');
  });

  test('a duplicate of the newest leaves the view where it is', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    historyBack().click(); // viewing 猫, 犬 is the newest

    emit('parse-result', parse('犬')); // repeats the newest — skipped, per design §5

    expect(document.querySelector('.chip')?.textContent).toBe('猫');
    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(false);
  });

  test('a parse navigated to renders with the furigana mode current now', () => {
    emit('parse-result', parse('東京', { reading: 'とうきょう' }));
    emit('parse-result', parse('犬'));
    emit('settings-changed', { furigana_mode: 'hiragana' });

    historyBack().click();

    expect(document.querySelector('.chip ruby rt')?.textContent).toBe('とうきょう');
  });

  // Design §6.2: `parseError` is cleared in `show` only, never in `display`,
  // so navigating leaves a standing error banner alone — this app already
  // keeps a failure message above the last good parse on purpose, and nav
  // must not be an exception to that.
  test('navigating with Back leaves a standing parse-error banner on screen', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    emit('parse-error', 'the parser panicked on an input of 4096 characters');

    historyBack().click();

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
    expect(document.querySelector('.chip')?.textContent).toBe('猫');
  });

  test('a new parse-result clears a standing parse-error banner', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    emit('parse-error', 'the parser panicked on an input of 4096 characters');
    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);

    emit('parse-result', parse('鳥'));

    expect(document.querySelectorAll('.startup-error')).toHaveLength(0);
  });

  test('a parse-error event leaves the history buffer untouched', () => {
    emit('parse-result', parse('猫'));
    emit('parse-result', parse('犬'));
    historyBack().click(); // viewing 猫, with 犬 ahead of the cursor

    emit('parse-error', 'the parser panicked on an input of 4096 characters');

    // Neither button's disabled state moved...
    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(false);
    // ...and Forward still reaches the same 犬 that was ahead of the cursor
    // before the error arrived — the buffer was never touched by it.
    historyForward().click();
    expect(document.querySelector('.chip')?.textContent).toBe('犬');
  });

  test('a history-step event walks back and forward', () => {
    emit('parse-result', parse('一'));
    emit('parse-result', parse('二'));

    emit('history-step', -1);
    expect(document.querySelector('#output')?.textContent).toContain('一');

    emit('history-step', 1);
    expect(document.querySelector('#output')?.textContent).toContain('二');
  });

  test('a history-step at either end changes nothing', () => {
    emit('parse-result', parse('一'));

    emit('history-step', -1);
    emit('history-step', 1);

    expect(document.querySelector('#output')?.textContent).toContain('一');
  });

  // The chevrons and the keys share one cursor, so the buttons have to follow
  // a keyboard step or they would advertise the wrong state.
  test('a history-step updates the chevrons', () => {
    emit('parse-result', parse('一'));
    emit('parse-result', parse('二'));

    emit('history-step', -1);

    expect(historyBack().disabled).toBe(true);
    expect(historyForward().disabled).toBe(false);
  });
});
