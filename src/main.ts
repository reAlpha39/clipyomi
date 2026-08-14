import { invoke } from '@tauri-apps/api/core';
import { renderSentence } from './render/sentence';
import type { ParseResult } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <div class="input-row">
    <input id="text" type="text" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
  <div id="parse-error"></div>
  <div class="panes"><div id="output"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const input = app.querySelector<HTMLInputElement>('#text')!;
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

async function showStartupError(): Promise<void> {
  const message = await invoke<string | null>('startup_error');
  if (message !== null) output.replaceChildren(errorBlock(message));
}

export async function run(): Promise<void> {
  try {
    const result = await invoke<ParseResult>('parse_text', { text: input.value });
    output.replaceChildren(renderSentence(result));
    parseError.replaceChildren();
  } catch (e) {
    // A parse failure keeps the previous result on screen rather than blanking
    // it; only the message is shown, replacing any previous message rather
    // than stacking alongside it.
    parseError.replaceChildren(errorBlock(String(e)));
  }
}

app.querySelector('#parse')!.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
