import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { renderSentence } from './render/sentence';
import { renderDefinitions } from './render/definitions';
import type { ParseResult } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <header class="controls">
    <button id="always-on-top" type="button" aria-pressed="false">Always on top</button>
    <button id="monitor" type="button" aria-pressed="true">Monitoring</button>
  </header>
  <div class="input-row">
    <input id="text" type="text" aria-label="Japanese text to parse" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
  <div id="parse-error"></div>
  <div class="panes"><div id="dictionary"></div><div id="output"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const input = app.querySelector<HTMLInputElement>('#text')!;
const parseButton = app.querySelector<HTMLButtonElement>('#parse')!;
// A fixed slot rather than a node prepended into `output`: `output` is what a
// successful parse replaces wholesale (and what Task 5 replaces with two
// panes), so an error node living there either gets silently wiped on the
// next success or, on repeated failures, piles up because nothing ever
// removes the previous one. This slot always holds at most one error.
const parseError = app.querySelector<HTMLElement>('#parse-error')!;

function errorBlock(message: string): HTMLElement {
  const el = document.createElement('pre');
  el.className = 'startup-error';
  el.textContent = message;
  return el;
}

// Exported so a test can await it directly rather than racing the
// fire-and-forget call at the bottom of this module.
export async function showStartupError(): Promise<void> {
  const message = await invoke<string | null>('startup_error');
  if (message === null) return;
  // No index (the expected first run — 2E adds the download) and an
  // unopenable index both mean `parse_text` cannot succeed. Disabling the
  // controls here is what stops that failure from ever reaching the user as
  // a raw Tauri "state not managed" string the moment they try to parse.
  output.replaceChildren(errorBlock(message));
  input.disabled = true;
  parseButton.disabled = true;
}

// A corrupt settings.json is cosmetic, not fatal: the app already fell back
// to defaults by the time this resolves, so — unlike `showStartupError`
// above — this must never disable a control and must never touch `output`.
// Exported for the same reason `showStartupError` is.
export async function showSettingsWarning(): Promise<void> {
  const message = await invoke<string | null>('settings_warning');
  if (message === null) return;
  parseError.replaceChildren(errorBlock(message));
}

const dictionary = app.querySelector<HTMLElement>('#dictionary')!;

// The backend's own phase labels. Anything not in this map is an error message
// to show verbatim, which is why this is a lookup rather than an enum — the
// failure arm carries text the user needs to read.
const PHASE_LABELS: Record<string, string> = {
  downloading: 'Downloading dictionary…',
  building: 'Building index…',
};

function renderDictionary(status: string | null): void {
  if (status === 'ready') {
    dictionary.replaceChildren();
    input.disabled = false;
    parseButton.disabled = false;
    return;
  }

  const el = document.createElement('div');
  el.className = 'dictionary';
  const phase = status === null ? undefined : PHASE_LABELS[status];

  if (phase !== undefined) {
    const label = document.createElement('p');
    label.textContent = phase;
    const spinner = document.createElement('div');
    spinner.className = 'spinner';
    // Decorative: the label above already carries the information, and under
    // `prefers-reduced-motion` this stops moving entirely.
    spinner.setAttribute('aria-hidden', 'true');
    el.replaceChildren(label, spinner);
    dictionary.replaceChildren(el);
    return;
  }

  // Idle or failed. `null` is the first-run offer; anything else is a message
  // from the backend, which already names the archive and directory to drop in.
  const message = document.createElement('p');
  message.textContent =
    status === null
      ? 'No dictionary yet. JMdict is a one-time download of roughly ten megabytes from EDRDG.'
      : status;

  const button = document.createElement('button');
  button.id = 'download';
  button.type = 'button';
  button.textContent = status === null ? 'Download dictionary' : 'Retry';

  // Closure-local, not `button.disabled`: disabling a focused element blurs it
  // and drops it from the tab order, which nothing here restores. Same guard
  // the header toggles use.
  let pending = false;
  button.addEventListener('click', () => {
    if (pending) return;
    pending = true;
    renderDictionary('downloading');
    void invoke('download_dictionary')
      .catch((e) => renderDictionary(String(e)))
      .finally(() => {
        pending = false;
      });
  });

  el.replaceChildren(message, button);
  dictionary.replaceChildren(el);
}

// Exported for the same reason `showStartupError` is: a test can await it
// directly rather than racing the fire-and-forget call at the bottom.
export async function showDictionaryScreen(): Promise<void> {
  if (!(await invoke<boolean>('needs_dictionary'))) return;
  // Parsing cannot succeed until an index exists. Unlike `showStartupError`'s
  // disabling, this is reversible — `ready` turns both back on.
  input.disabled = true;
  parseButton.disabled = true;
  renderDictionary(null);
}

interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
}

const alwaysOnTop = app.querySelector<HTMLButtonElement>('#always-on-top');
const monitor = app.querySelector<HTMLButtonElement>('#monitor');

// Populated with a button the instant it's clicked, before its request even
// starts: `applySettings` below skips writing to a button that's in this
// set, so a settings response that resolves after the user has already
// acted can never clobber what they just set. Per-button (not a single
// shared flag) so clicking one control doesn't also freeze its sibling out
// of ever receiving its real persisted value.
const touchedButtons = new Set<HTMLButtonElement>();

function bindToggle(
  button: HTMLButtonElement | null,
  command: string,
  settingsKey: keyof Settings,
): void {
  if (button === null) return;
  // Closure-local, not `button.disabled`: disabling a focused element blurs
  // it and drops it from the tab order, which a `finally` re-enable does not
  // undo. This guard gives the same exclusion — a click while one is
  // already in flight is a no-op — without touching the DOM or focus at all.
  let pending = false;
  button.addEventListener('click', () => {
    if (pending) return;
    pending = true;
    touchedButtons.add(button);
    const next = button.getAttribute('aria-pressed') !== 'true';
    // Flip first so the control feels immediate; a rejected command corrects it below.
    button.setAttribute('aria-pressed', String(next));
    void invoke(command, { enabled: next })
      .catch(async (e) => {
        // Rendered first and unconditionally: the user must see *some*
        // message, even if the resync below also fails. Doing this before
        // the `await` means it never depends on that second call succeeding.
        parseError.replaceChildren(errorBlock(String(e)));
        // A rejected setter doesn't say which of two things happened:
        // `state.rs`'s `SettingsState::update` applies a change in memory
        // *before* it tries to persist it, so a write failure (e.g. a
        // read-only config dir) still leaves the new value in effect
        // (design §5) — reverting to the naive inverse would then show the
        // opposite of what the backend actually did. Re-reading
        // `get_settings` shows whichever outcome really happened instead of
        // guessing from the shape of the error.
        try {
          const settings = await invoke<Settings>('get_settings');
          button.setAttribute('aria-pressed', String(settings[settingsKey]));
        } catch {
          // Best-effort only: if even this fails, the button keeps its
          // optimistic (possibly wrong) value, but the error above is
          // already visible — degrading silently here is better than an
          // unhandled rejection with no message shown at all.
        }
      })
      .finally(() => {
        pending = false;
      });
  });
}

bindToggle(alwaysOnTop, 'set_always_on_top', 'always_on_top');
bindToggle(monitor, 'set_clipboard_monitoring', 'clipboard_monitoring');

async function applySettings(): Promise<void> {
  const settings = await invoke<Settings>('get_settings');
  if (alwaysOnTop !== null && !touchedButtons.has(alwaysOnTop)) {
    alwaysOnTop.setAttribute('aria-pressed', String(settings.always_on_top));
  }
  if (monitor !== null && !touchedButtons.has(monitor)) {
    monitor.setAttribute('aria-pressed', String(settings.clipboard_monitoring));
  }
}

function show(result: ParseResult): void {
  const sentence = renderSentence(result);
  const definitions = renderDefinitions(result);

  // Delegated to the pane, not per-chip: chips are re-created on every
  // parse, but the sentence container itself is fresh each time too, so one
  // listener per parse is exactly right — nothing to leak, nothing to
  // rebind mid-life.
  sentence.addEventListener('click', (e) => {
    const chip = (e.target as HTMLElement).closest<HTMLElement>('[data-start]');
    if (chip === null) return;
    const row = definitions.querySelector(`.def-row[data-start="${chip.dataset.start}"]`);
    row?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    definitions.querySelectorAll('.marked').forEach((n) => n.classList.remove('marked'));
    row?.classList.add('marked');
  });

  parseError.replaceChildren();
  output.replaceChildren(sentence, definitions);
}

// `listen()`'s returned promise resolves only once its IPC round trip has
// registered the handler on the Rust side. The clipboard poll's first tick
// waits on `frontend_ready` before it reads anything (see
// `clipboard::wait_for_frontend`), so firing it only after BOTH listeners
// are confirmed registered closes the race where clipboard text present at
// launch gets parsed and emitted before anything here is listening for it.
//
// A `listen()` failure above is deliberately NOT caught by anything here: it
// means parse events can never reach this webview at all (not merely that
// the poll's first tick might drop), which is not a condition to swallow —
// it surfaces as an unhandled rejection, exactly as it would have before
// this handshake existed. Only the `frontend_ready` call below gets its own,
// narrower catch, since that one really is limited to "the poll's first
// tick after launch might drop".
void Promise.all([
  listen<ParseResult>('parse-result', (e) => show(e.payload)),
  // A failure replaces only the message, never the result: the previous
  // parse stays readable while the user works out what went wrong.
  listen<string>('parse-error', (e) => parseError.replaceChildren(errorBlock(e.payload))),
  listen<string>('dictionary-status', (e) => renderDictionary(e.payload)),
]).then(() => {
  invoke('frontend_ready').catch(() => {
    // Not actionable by the user if this fails — same policy as a skipped
    // clipboard read (design §5). The only consequence is that the poll's
    // first tick after launch can drop; nothing else in the app depends on
    // this call succeeding.
  });
});

async function run(): Promise<void> {
  try {
    await invoke('set_input', { text: input.value });
  } catch (e) {
    parseError.replaceChildren(errorBlock(String(e)));
  }
}

parseButton.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
void applySettings();
void showSettingsWarning();
void showDictionaryScreen();
