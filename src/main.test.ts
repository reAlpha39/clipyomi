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
    // `startup_error` resolves to null (no startup failure) so the module's
    // own fire-and-forget `showStartupError()` call is a no-op and leaves the
    // controls enabled; nothing else calls `invoke` in these tests.
    invoke.mockImplementation((cmd: string) =>
      cmd === 'startup_error' ? Promise.resolve(null) : Promise.resolve(undefined),
    );
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
    invoke.mockImplementation((cmd: string) =>
      cmd === 'startup_error'
        ? Promise.resolve('no dictionary index in /nowhere')
        : Promise.reject('parse_text must not be reachable once controls are disabled'),
    );

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(true);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(true);
    expect(document.querySelector('.startup-error')?.textContent).toContain(
      'no dictionary index',
    );
  });

  test('startup_error resolving to null leaves the controls enabled', async () => {
    invoke.mockImplementation((cmd: string) =>
      cmd === 'startup_error' ? Promise.resolve(null) : Promise.reject('unused'),
    );

    const { showStartupError } = await import('./main');
    await showStartupError();

    expect((document.querySelector('#text') as HTMLInputElement).disabled).toBe(false);
    expect((document.querySelector('#parse') as HTMLButtonElement).disabled).toBe(false);
  });
});
