import { beforeEach, describe, expect, test, vi } from 'vitest';

// `main.ts` is the wired app entry, not a pure render function like
// `sentence.ts` — exercising it means stubbing both IPC boundaries it uses:
// `@tauri-apps/api/event` (`listen`, for the parse-result/parse-error push
// events) and `@tauri-apps/api/core` (`invoke`, still used directly for
// `set_input` and `startup_error`).
const listeners = new Map<string, (e: { payload: unknown }) => void>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
}));

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

describe('the event-driven render path', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    // `startup_error` and `settings_warning` resolve to null (nothing
    // startup-fatal, nothing settings-cosmetic to report), and `get_settings`
    // resolves to a harmless default: the header now calls all three
    // unconditionally on import, so every test that imports `./main` must
    // answer them or the module's own fire-and-forget calls contaminate
    // these tests with unhandled rejections. Nothing else calls `invoke` in
    // these tests.
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
    });
    expect(document.querySelector('.chip')?.textContent).toBe('東京');
    expect(document.querySelector('.def-row')).not.toBeNull();
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
    });
    emit('parse-error', 'the parser panicked on an input of 4096 characters');
    emit('parse-error', 'a second failure');

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
    expect(document.querySelector('.sentence')).not.toBeNull();
  });
});

describe('main: a startup failure disables the parse controls', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    vi.resetModules();
  });

  test('startup_error resolving to a message disables #text and #parse', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'startup_error') return Promise.resolve('no dictionary index in /nowhere');
      // The header's own settings calls fire unconditionally on import, so
      // they need real (harmless) answers; anything else is the thing this
      // test is guarding against.
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'settings_warning') return Promise.resolve(null);
      return Promise.reject('parse_text must not be reachable once controls are disabled');
    });

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(true);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(true);
    expect(document.querySelector('.startup-error')?.textContent).toContain(
      'no dictionary index',
    );
  });

  test('startup_error resolving to null leaves the controls enabled', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'startup_error') return Promise.resolve(null);
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'settings_warning') return Promise.resolve(null);
      return Promise.reject('unused');
    });

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('the header controls', () => {
  beforeEach(async () => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    // Deliberately the INVERSE of the markup's hardcoded aria-pressed
    // defaults ('false' on #always-on-top, 'true' on #monitor): if these
    // matched the markup, "reflects loaded settings" would still pass with
    // `applySettings` deleted entirely, since the DOM would already show the
    // right values before it ever ran.
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: true, clipboard_monitoring: false });
      }
      if (cmd === 'set_always_on_top' || cmd === 'set_clipboard_monitoring') {
        return Promise.resolve(undefined);
      }
      // startup_error / settings_warning: nothing to report in these tests.
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    // let the get_settings promise resolve
    await Promise.resolve();
    await Promise.resolve();
  });

  test('reflects loaded settings in aria-pressed', () => {
    expect(document.querySelector('#monitor')?.getAttribute('aria-pressed')).toBe('false');
    expect(document.querySelector('#always-on-top')?.getAttribute('aria-pressed')).toBe('true');
  });

  test('toggling monitoring flips aria-pressed', async () => {
    const button = document.querySelector<HTMLButtonElement>('#monitor');
    if (button === null) throw new Error('#monitor missing');
    button.click();
    await Promise.resolve();
    expect(button.getAttribute('aria-pressed')).toBe('true');
  });

  // Regression 2: the first version of this fix disabled the button for the
  // duration of its request, which blurs a focused element and drops it
  // from the tab order — a keyboard user activating a toggle would lose
  // their place. Activation must never move focus off the button.
  test('activating a focused toggle keeps focus on it', () => {
    const button = document.querySelector<HTMLButtonElement>('#monitor');
    if (button === null) throw new Error('#monitor missing');
    button.focus();
    expect(document.activeElement).toBe(button);
    button.click();
    expect(document.activeElement).toBe(button);
  });
});

describe('overlapping toggle requests', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    vi.resetModules();
  });

  // Finding 2: without some guard against a second click landing before the
  // first settles, two in-flight requests could arrive out of order.
  // Guarded with a closure-local `pending` flag, not `button.disabled` — see
  // the focus-preservation test above for why not.
  test('a button suppresses overlapping clicks while its request is in flight, without disabling the element', async () => {
    let resolveSet: (() => void) | undefined;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'set_clipboard_monitoring') {
        return new Promise<void>((resolve) => {
          resolveSet = resolve;
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const button = document.querySelector<HTMLButtonElement>('#monitor');
    if (button === null) throw new Error('#monitor missing');

    button.click();
    // Never disabled — disabling a focused button would blur it (Regression 2).
    expect(button.disabled).toBe(false);

    // A second click while the first request is still in flight must not
    // start a second, overlapping request.
    button.click();
    expect(
      invoke.mock.calls.filter(([cmd]) => cmd === 'set_clipboard_monitoring'),
    ).toHaveLength(1);

    resolveSet?.();
    await Promise.resolve();
    await Promise.resolve();

    // Settled: a further click now goes through as a new request.
    button.click();
    expect(
      invoke.mock.calls.filter(([cmd]) => cmd === 'set_clipboard_monitoring'),
    ).toHaveLength(2);
  });

  // Minor: the re-enable path was only ever exercised on success. A rejected
  // command must both revert `aria-pressed` and clear `pending`, or the
  // control would be stuck refusing every click after its first failure.
  test('a rejected toggle reverts aria-pressed and accepts the next click', async () => {
    let rejectFirst: ((reason: unknown) => void) | undefined;
    let calls = 0;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'set_clipboard_monitoring') {
        calls += 1;
        if (calls === 1) {
          return new Promise((_resolve, reject) => {
            rejectFirst = reject;
          });
        }
        return Promise.resolve(undefined);
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const button = document.querySelector<HTMLButtonElement>('#monitor');
    if (button === null) throw new Error('#monitor missing');

    button.click(); // markup/loaded default 'true' -> flips to 'false'
    expect(button.getAttribute('aria-pressed')).toBe('false');

    rejectFirst?.('backend refused');
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    expect(button.getAttribute('aria-pressed')).toBe('true'); // reverted
    expect(document.querySelector('#parse-error')?.textContent).toContain('backend refused');

    // `pending` must have cleared even on rejection, or this click would be
    // silently swallowed exactly like an overlapping one.
    button.click();
    expect(button.getAttribute('aria-pressed')).toBe('false');
  });

  // Finding 3 (a click before settings load must survive the late response)
  // AND Regression 1 (that must not come at the cost of freezing the
  // sibling control the user never touched).
  test('a click before settings load survives the late response, and the untouched sibling still updates', async () => {
    let resolveGetSettings: ((value: unknown) => void) | undefined;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return new Promise((resolve) => {
          resolveGetSettings = resolve;
        });
      }
      if (cmd === 'set_always_on_top') return Promise.resolve(undefined);
      return Promise.resolve(null);
    });

    await import('./main');

    const alwaysOnTopButton = document.querySelector<HTMLButtonElement>('#always-on-top');
    const monitorButton = document.querySelector<HTMLButtonElement>('#monitor');
    if (alwaysOnTopButton === null) throw new Error('#always-on-top missing');
    if (monitorButton === null) throw new Error('#monitor missing');

    // Markup default is 'false'; the click flips it before get_settings has
    // resolved at all. #monitor is never clicked.
    alwaysOnTopButton.click();
    expect(alwaysOnTopButton.getAttribute('aria-pressed')).toBe('true');

    // Both values differ from their own control's markup default (false and
    // true respectively). If `touched` were a single flag shared by both
    // buttons — the actual regression found in review — #monitor would stay
    // frozen at its 'true' markup default instead of picking up 'false'
    // here, which is exactly what the second assertion below would catch.
    resolveGetSettings?.({ always_on_top: true, clipboard_monitoring: false });
    await Promise.resolve();
    await Promise.resolve();

    // The clicked control keeps the user's value...
    expect(alwaysOnTopButton.getAttribute('aria-pressed')).toBe('true');
    // ...and the untouched sibling still receives its real loaded value.
    expect(monitorButton.getAttribute('aria-pressed')).toBe('false');
  });
});

// Carried forward from Task 4: `settings_warning` exists but, until now,
// nothing rendered it. Deliberately not colocated with the fatal
// `startup_error` tests above — the whole point is that this path must
// behave differently (cosmetic, non-disabling) from that one.
describe('the settings warning', () => {
  beforeEach(() => {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    vi.resetModules();
  });

  test('a non-null settings_warning renders into #parse-error and leaves #text/#parse enabled', async () => {
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
    // The point of this assertion: a settings warning is cosmetic. It must
    // never disable the controls the way a fatal `startup_error` correctly
    // does, and it must never touch `output` (nothing was parsed).
    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(false);
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
});
