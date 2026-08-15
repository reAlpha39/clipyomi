import { emit, listen } from '@tauri-apps/api/event';
import { renderTooltip } from './render/tooltip';
import type { Entry } from './types';
import './styles/tooltip.css';

// The tooltip window's entry point. Deliberately does not import `main.ts`:
// this page runs in a second webview, and pulling in the app entry would start
// a second clipboard handshake and a second set of event listeners.
const tooltip = document.querySelector<HTMLElement>('#tooltip')!;

/**
 * Total vertical padding on `#tooltip`, in px.
 *
 * Kept in step by hand with `padding: 3px 4px` in `src/styles/tooltip.css` —
 * 3 top + 3 bottom. A stylesheet value cannot be imported into a module, and
 * reading it back with `getComputedStyle` would cost a layout to learn a
 * constant this file already knows.
 */
const VERTICAL_PADDING = 6;

void listen<Entry[]>('popover-content', (e) => {
  const content = renderTooltip(e.payload);
  tooltip.replaceChildren(content);
  tooltip.scrollTop = 0;
  // The main window cannot measure this content — it is in another webview —
  // so the size round-trips back.
  //
  // Measured off `content`, never off `#tooltip`: that element is `height:
  // 100%` of the window, and per CSSOM-View `scrollHeight`/`scrollWidth` are
  // floored at the padding box — so measuring it can never report a size
  // smaller than the window it is measured in. The height would latch at the
  // tallest entry ever hovered and the width would shrink by the border width
  // on every round trip. `content` is the `div.tt` child, `height: auto`, so
  // its rect is the real content extent.
  //
  // `scrollWidth` is still the width source: `.tt-line` sets `overflow-wrap:
  // anywhere`, so content always rewraps to fit and a content-extent width
  // would be meaningless. Adding back the chrome the client box excludes
  // (borders, and a scrollbar where they are classic rather than overlay)
  // makes the reported inner width exactly the one the window already has,
  // which is what stops the shrink.
  const chromeWidth = tooltip.offsetWidth - tooltip.clientWidth;
  const chromeHeight = tooltip.offsetHeight - tooltip.clientHeight;
  void emit('popover-measured', {
    width: tooltip.scrollWidth + chromeWidth,
    height: content.getBoundingClientRect().height + VERTICAL_PADDING + chromeHeight,
  }).catch(() => {
    // Fire-and-forget, matching `src/main.ts`'s policy for the other half of
    // this round trip: a failed emit leaves the tooltip unshown, which is no
    // worse than the user never having hovered — and better than an unhandled
    // rejection in a webview with no console anyone reads.
  });
});
