// Serves the real render path against a fixture, with `invoke` and the event
// system stubbed.
//
// `tauri-driver` supports Windows and Linux only — macOS has no WKWebView driver
// — so visual tests run the frontend in plain Chromium instead. This deliberately
// does not exercise the Rust↔webview seam; the src-tauri tests cover that side.
//
// `window.__TAURI_INTERNALS__.invoke(cmd, args, options)` is confirmed as the
// real hook `@tauri-apps/api` 2.11.1 dispatches through — read directly from
// `node_modules/@tauri-apps/api/core.js`, not guessed.
//
// Task 5's frontend gets its results from events, not from `invoke`'s return
// value, so `listen` needs a stub too. Read directly from
// `node_modules/@tauri-apps/api/event.js`: `listen(event, handler)` calls
// `invoke('plugin:event|listen', { event, target, handler: transformCallback(handler) })`,
// where `transformCallback` (from `core.js`) is just
// `window.__TAURI_INTERNALS__.transformCallback(callback, once)` — in a real
// webview this is injected by the Tauri runtime and stores the callback under
// an id that the Rust side later `eval()`s back into. There's no Rust side
// here, so the id indirection buys nothing: `transformCallback` is the
// identity function below, and `invoke('plugin:event|listen', …)` records the
// handler by event name directly. Since nothing plays the role of the backend
// calling back in, the registered handler is exposed as `window.__TA_EMIT__`
// for a test to invoke directly — the plan's named fallback for firing
// events, used here because a real IPC round trip has nothing on the other
// end to answer it.
export const STUB = `
  window.__TA_LISTENERS__ = {};
  window.__TA_EMIT__ = (event, payload) => {
    const handler = window.__TA_LISTENERS__[event];
    if (handler === undefined) throw new Error('nothing listening for ' + event);
    handler({ event, id: 0, payload });
  };
  window.__TAURI_INTERNALS__ = {
    transformCallback: (callback) => callback,
    invoke: async (cmd, args) => {
      if (cmd === 'startup_error') return null;
      // No corrupt settings.json in this stub's world — the header still
      // needs an answer, and null is the "nothing to report" case Task 6
      // itself defines for this command.
      if (cmd === 'settings_warning') return null;
      // Fixed, deliberately unpressed/pressed pair rather than mirroring
      // some other stubbed state: it gives the visual baselines a stable,
      // non-default-looking pair (one pressed, one not) without depending on
      // a toggle actually round-tripping through this stub.
      if (cmd === 'get_settings') return { always_on_top: false, clipboard_monitoring: true };
      if (cmd === 'set_always_on_top' || cmd === 'set_clipboard_monitoring') return undefined;
      if (cmd === 'plugin:event|listen') {
        window.__TA_LISTENERS__[args.event] = args.handler;
        return 0;
      }
      return undefined;
    },
  };
`;
