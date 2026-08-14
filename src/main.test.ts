import { beforeEach, describe, expect, test, vi } from 'vitest';

// `main.ts` is the wired app entry, not a pure render function like
// `sentence.ts` — exercising it means mocking the IPC boundary
// (`@tauri-apps/api/core`) rather than just calling a function with data.
// `sentence.test.ts` covers `renderSentence` only; this is the module that
// actually owns the bug (`run`'s catch branch), so its test lives next to it.
const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

describe('main: the parse-error slot', () => {
  beforeEach(() => {
    vi.resetModules();
    invoke.mockReset();
    // `startup_error` resolves to null (no startup failure) so the module's
    // own fire-and-forget `showStartupError()` call is a no-op; `parse_text`
    // always rejects so every `run()` call below hits the catch branch.
    invoke.mockImplementation((cmd: string) =>
      cmd === 'startup_error' ? Promise.resolve(null) : Promise.reject('boom'),
    );
    document.body.innerHTML = '<main id="app"></main>';
  });

  test('two consecutive failed parses leave exactly one error block', async () => {
    const { run } = await import('./main');

    await run();
    await run();

    expect(document.querySelectorAll('.startup-error')).toHaveLength(1);
  });
});
