import type { Entry } from '../types';
import { assembleTooltipText } from './tooltip-text';
import { colourLine } from './tooltip-colour';

/**
 * The tooltip's body: every match for one word, coloured ta-old's way.
 *
 * Deliberately NOT `renderEntry`. 2G shared one renderer between the pane and
 * the popover so they could not drift; from this phase they are meant to
 * differ, so each owns its own. The pane is untouched.
 */
export function renderTooltip(entries: Entry[]): HTMLElement {
  const root = document.createElement('div');
  root.className = 'tt';

  for (const line of assembleTooltipText(entries).split('\n')) {
    const el = document.createElement('div');
    el.className = 'tt-line';
    for (const run of colourLine(line)) {
      const span = document.createElement('span');
      span.className = `tt-${run.kind}`;
      // `textContent`, never `innerHTML`: this is the one place in the phase
      // where a pre-assembled string meets the DOM, and it is exactly the
      // shape that invites the wrong API.
      span.textContent = run.text;
      el.append(span);
    }
    // An empty line still needs height, or stacked entries run together.
    if (!el.hasChildNodes()) el.append(document.createElement('br'));
    root.append(el);
  }

  return root;
}
