import { emit, listen } from '@tauri-apps/api/event';
import { renderTooltip } from './render/tooltip';
import type { Entry } from './types';
import './styles/tooltip.css';

// The tooltip window's entry point. Deliberately does not import `main.ts`:
// this page runs in a second webview, and pulling in the app entry would start
// a second clipboard handshake and a second set of event listeners.
const tooltip = document.querySelector<HTMLElement>('#tooltip')!;

void listen<Entry[]>('popover-content', (e) => {
  tooltip.replaceChildren(renderTooltip(e.payload));
  tooltip.scrollTop = 0;
  // The main window cannot measure this content — it is in another webview —
  // so the size round-trips back. `scrollWidth`/`scrollHeight` rather than
  // `getBoundingClientRect`: the window is still at its previous size, so the
  // laid-out box is the old one and only the scroll extent reflects the new
  // content.
  void emit('popover-measured', {
    width: tooltip.scrollWidth,
    height: tooltip.scrollHeight,
  });
});
