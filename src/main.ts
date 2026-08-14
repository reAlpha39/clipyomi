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
  <div class="panes"><div id="output"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const input = app.querySelector<HTMLInputElement>('#text')!;

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

async function run(): Promise<void> {
  try {
    const result = await invoke<ParseResult>('parse_text', { text: input.value });
    output.replaceChildren(renderSentence(result));
  } catch (e) {
    // A parse failure keeps the previous result on screen rather than blanking
    // it; only the message is added.
    output.prepend(errorBlock(String(e)));
  }
}

app.querySelector('#parse')!.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
