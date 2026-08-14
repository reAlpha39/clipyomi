import { invoke } from '@tauri-apps/api/core';
import { renderSentence } from './render/sentence';
import { renderDefinitions } from './render/definitions';
import type { ParseResult } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <div class="input-row">
    <input id="text" type="text" placeholder="Paste Japanese text" aria-label="Japanese text to parse" />
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

export async function run(): Promise<void> {
  try {
    const result = await invoke<ParseResult>('parse_text', { text: input.value });
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

    output.replaceChildren(sentence, definitions);
    parseError.replaceChildren();
  } catch (e) {
    // A parse failure keeps the previous result on screen rather than blanking
    // it; only the message is shown, replacing any previous message rather
    // than stacking alongside it.
    parseError.replaceChildren(errorBlock(String(e)));
  }
}

parseButton.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
