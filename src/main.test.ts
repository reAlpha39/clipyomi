import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

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

// `main.ts` now reads the current window's label at import time (to scope
// its move/resize listeners), so every test that imports it needs this
// mocked too, even though nothing in this file ever reaches `placeFor` or
// the keep poll — that coverage, and the fuller controllable stubs it needs,
// lives in `main-tooltip.test.ts`.
// All three exports `main.ts` imports, even though no describe in this file
// reaches the last two: a factory that omits an export fails any future test
// that does touch it with an opaque "No export is defined on the mock".
// `src/main-tooltip.test.ts` owns the describes that actually drive these.
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
    });
    expect(document.querySelector('.chip')?.textContent).toBe('東京');
    expect(document.querySelector('.sentence')).not.toBeNull();
    expect(document.querySelector('.definitions')).toBeNull();
    expect(document.querySelector('.def-row')).toBeNull();
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
    });
    emit('parse-error', 'the parser panicked on an input of 4096 characters');
    emit('parse-error', 'a second failure');

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
    expect(document.querySelector('.sentence')).not.toBeNull();
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
      // showDictionaryScreen's own invoke call, awaited before this test's
      // assertions run — without an explicit answer here it falls through to
      // the reject below and turns `void showDictionaryScreen()` into an
      // unhandled rejection that fails the whole file.
      if (cmd === 'needs_dictionary') return Promise.resolve(false);
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
      // Same reason as the sibling test above: showDictionaryScreen must not
      // fall through to a rejection here.
      if (cmd === 'needs_dictionary') return Promise.resolve(false);
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
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true, decorations: true });
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
  function load(decorations: boolean, peekGrowsFrame = false) {
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') return Promise.resolve({ decorations });
      if (cmd === 'is_macos') return Promise.resolve(peekGrowsFrame);
      if (cmd === 'peek_grows_frame') return Promise.resolve(peekGrowsFrame);
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

  // The attribute is the whole state machine: CSS keys the reserved row, the
  // overlay, and the reveal off it, and the pointer handlers below read it to
  // decide whether the native chrome is theirs to peek at.
  test('mirrors the loaded decorations flag onto #app', async () => {
    await load(false);
    await Promise.resolve();
    await Promise.resolve();

    expect(document.querySelector('#app')?.getAttribute('data-decorations')).toBe('false');
  });

  test('a settings-changed event moves the attribute with it', async () => {
    await load(true);
    await Promise.resolve();
    await Promise.resolve();
    expect(document.querySelector('#app')?.getAttribute('data-decorations')).toBe('true');

    emit('settings-changed', { decorations: false });

    expect(document.querySelector('#app')?.getAttribute('data-decorations')).toBe('false');
  });

  test('hovering the band peeks the native chrome while the title bar is hidden', async () => {
    await load(false);
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
    await load(false);
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
    expect(invoke).not.toHaveBeenCalledWith('set_decorations', expect.anything());
  });

  // With the title bar shown there is nothing to reveal — the chrome is
  // already there, and peeking would hide it on the way out.
  test('hovering does nothing while the title bar is shown', async () => {
    await load(true);
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
    await load(false);
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
      await load(false);
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
      await load(false);
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
      await load(false);
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

  test('peeking adds the peek-offset class when the frame can grow', async () => {
    vi.useFakeTimers();
    try {
      await load(false, true);
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
      await load(false, false);
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
    await load(false);
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointermove'));
    expect(shell().classList.contains('peeked')).toBe(true);
  });

  test('unfocused-mouse-move reveals the band and unfocused-mouse-leave hides it', async () => {
    vi.useFakeTimers();
    try {
      await load(false);
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
      await load(false);
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
    await load(false);
    await Promise.resolve();
    await Promise.resolve();

    shell().dispatchEvent(new Event('pointerenter'));
    emit('settings-changed', { decorations: true });
    shell().dispatchEvent(new Event('pointerleave'));

    expect(invoke).not.toHaveBeenCalledWith('peek_titlebar', { visible: false, height: 28 });
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
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true, decorations: true });
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
          decorations: true,
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
    });

    // The filters are not observable directly; the popover payload is what
    // carries them, so that is what proves the event was applied.
    document
      .querySelector<HTMLElement>('.chip')
      ?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));

    expect(emitted).toContainEqual([
      'popover-content',
      { entries, filters: { hide_pos: true, hide_xrefs: true, hide_usage: false } },
    ]);
  });

  test('openFor sends current active filters in popover-content event', async () => {
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({
          always_on_top: false,
          clipboard_monitoring: true,
          decorations: true,
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
    });

    const chip = document.querySelector<HTMLElement>('.chip');
    chip?.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));

    expect(emitted).toContainEqual([
      'popover-content',
      {
        entries: sampleEntries,
        filters: {
          hide_pos: true,
          hide_xrefs: false,
          hide_usage: true,
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
          decorations: true,
        });
      }
      return Promise.resolve(null);
    });

    await import('./main');
    await Promise.resolve();
    await Promise.resolve();

    const min = document.querySelector<HTMLButtonElement>('#window-minimize');
    const max = document.querySelector<HTMLButtonElement>('#window-maximize');
    const close = document.querySelector<HTMLButtonElement>('#window-close');
    const header = document.querySelector<HTMLElement>('header.controls');

    min?.click();
    expect(invoke).toHaveBeenCalledWith('minimize_window');

    max?.click();
    expect(invoke).toHaveBeenCalledWith('toggle_maximize_window');

    close?.click();
    expect(invoke).toHaveBeenCalledWith('close_window');

    header?.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    expect(invoke).toHaveBeenCalledWith('toggle_maximize_window');
  });
});



