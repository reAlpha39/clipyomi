// SPDX-License-Identifier: MIT OR Apache-2.0

import { emit, listen } from '@tauri-apps/api/event';
import { clampFont } from './font-sizes';
import { clampFamily, quoteFamily } from './text-fonts';
import { renderTooltip } from './render/tooltip';
import type { Entry, GlossFilters, KanjiInfo } from './types';
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

type PopoverPayload =
  | Entry[]
  | {
      entries: Entry[];
      filters?: GlossFilters;
      gloss?: number;
      gloss_font?: string;
      kanji?: KanjiInfo[];
    };

/**
 * Write or remove one custom property on this window's root. Removed, not
 * left: this window is persistent, so a value one payload set has to go when
 * the next carries none, or Default in the settings window would take effect
 * only on the next launch.
 */
function setRootToken(token: string, value: string): void {
  if (value === '') document.documentElement.style.removeProperty(token);
  else document.documentElement.style.setProperty(token, value);
}

void listen<PopoverPayload>('popover-content', (e) => {
  const payload = e.payload;
  const entries = Array.isArray(payload) ? payload : payload.entries;
  const filters = Array.isArray(payload) ? undefined : payload.filters;
  // Absent on a bare-array payload and on one from a main window that found
  // no kanji; either renders exactly as before this field existed.
  const kanji = Array.isArray(payload) ? [] : (payload.kanji ?? []);

  // This webview has its own `:root`, so the main window's token write never
  // reached it. Applied before the render below, whose measurement is what
  // sizes the window — a size arriving afterwards would size it to the old
  // text. An absent size leaves `tooltip.css` owning the default.
  const gloss = Array.isArray(payload) ? null : clampFont('gloss', payload.gloss);
  setRootToken('--text-gloss', gloss === null ? '' : `${gloss}px`);
  const family = Array.isArray(payload) ? null : clampFamily(payload.gloss_font);
  setRootToken('--font-gloss', family === null ? '' : quoteFamily(family));

  const content = renderTooltip(entries, filters, kanji);
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
