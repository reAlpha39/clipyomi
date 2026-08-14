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
  <div class="panes"><div id="output"></div></div>
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

const alwaysOnTop = app.querySelector<HTMLButtonElement>('#always-on-top');
const monitor = app.querySelector<HTMLButtonElement>('#monitor');

// Set the instant a user clicks either toggle, before its request even
// starts: `applySettings` below checks this and skips its writes once it's
// true, so a settings response that resolves after the user has already
// acted can never clobber what they just set.
let touched = false;

function bindToggle(button: HTMLButtonElement | null, command: string): void {
  if (button === null) return;
  button.addEventListener('click', () => {
    touched = true;
    const next = button.getAttribute('aria-pressed') !== 'true';
    // Flip first so the control feels immediate; a rejected command reverts it.
    button.setAttribute('aria-pressed', String(next));
    // Disabled for the duration of the request, re-enabled in `finally`
    // regardless of outcome: two clicks on the same button can otherwise
    // settle out of order (the backend writes settings to disk outside a
    // mutex), and a late revert from an earlier click could stomp a later
    // one's settled state. Disabling makes overlapping requests on this
    // button structurally impossible rather than merely unlikely.
    button.disabled = true;
    void invoke(command, { enabled: next })
      .catch((e) => {
        button.setAttribute('aria-pressed', String(!next));
        parseError.replaceChildren(errorBlock(String(e)));
      })
      .finally(() => {
        button.disabled = false;
      });
  });
}

bindToggle(alwaysOnTop, 'set_always_on_top');
bindToggle(monitor, 'set_clipboard_monitoring');

async function applySettings(): Promise<void> {
  const settings = await invoke<{ always_on_top: boolean; clipboard_monitoring: boolean }>(
    'get_settings',
  );
  if (touched) return;
  alwaysOnTop?.setAttribute('aria-pressed', String(settings.always_on_top));
  monitor?.setAttribute('aria-pressed', String(settings.clipboard_monitoring));
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

void listen<ParseResult>('parse-result', (e) => show(e.payload));
// A failure replaces only the message, never the result: the previous parse
// stays readable while the user works out what went wrong.
void listen<string>('parse-error', (e) => parseError.replaceChildren(errorBlock(e.payload)));

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
