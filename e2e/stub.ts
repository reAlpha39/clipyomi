// Serves the real render path against a fixture, with `invoke` stubbed.
//
// `tauri-driver` supports Windows and Linux only — macOS has no WKWebView driver
// — so visual tests run the frontend in plain Chromium instead. This deliberately
// does not exercise the Rust↔webview seam; the src-tauri tests cover that side.
//
// `window.__TAURI_INTERNALS__.invoke(cmd, args, options)` is confirmed as the
// real hook `@tauri-apps/api` 2.11.1 dispatches through — read directly from
// `node_modules/@tauri-apps/api/core.js`, not guessed.
export const STUB = `
  window.__TAURI_INTERNALS__ = {
    invoke: async (cmd) => (cmd === 'startup_error' ? null : window.__FIXTURE__),
  };
`;
