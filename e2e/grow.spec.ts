import { expect, test, type Page } from '@playwright/test';
import { STUB } from './stub';

declare global {
  interface Window {
    __TA_EMIT__: (event: string, payload: unknown) => void;
    __BOX__: { x: number; y: number; width: number; height: number };
  }
}

// Grow-to-fit needs a real layout engine: the measurement reads the page's
// geometry, which happy-dom does not compute. The stub below plays the
// window — `set_window_box` records the box, and `follow` applies it to the
// viewport the way the OS would resize the webview.
//
// It answers as Windows, where the band leaves the grid flow and `.panes`
// lands in an `auto` row: a short sentence's pane is only as tall as the
// sentence, which is the layout that once kept a grown window from shrinking.
const WINDOW = (height: number, lines: number) => `
  window.__BOX__ = { x: 100, y: 500, width: 600, height: ${height} };
  const stubbed = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    if (cmd === 'platform') return 'windows';
    if (cmd === 'is_macos' || cmd === 'peek_grows_frame') return false;
    if (cmd === 'get_settings') {
      return { always_on_top: false, clipboard_monitoring: true, chrome_mode: 'header',
               grow_direction: 'up', grow_lines: ${lines}, grow_animate: false };
    }
    if (cmd === 'plugin:window|inner_size') return { width: window.__BOX__.width, height: window.__BOX__.height };
    if (cmd === 'plugin:window|outer_position') return { x: window.__BOX__.x, y: window.__BOX__.y };
    if (cmd === 'plugin:window|scale_factor') return 1;
    if (cmd === 'plugin:window|is_minimized' || cmd === 'plugin:window|is_maximized') return false;
    if (cmd === 'set_window_box') { window.__BOX__ = args; return null; }
    return stubbed(cmd, args);
  };
`;

const parse = (text: string) => ({
  segments: [{ start: 0, len: text.length, surface: text, reading: null, matched: false, entries: [] }],
  alternatives: [],
});

async function show(page: Page, text: string): Promise<void> {
  await page.evaluate((p) => window.__TA_EMIT__('parse-result', p), parse(text));
}

/** Keep the viewport on the box last set, until it holds still. */
async function follow(page: Page): Promise<{ y: number; height: number }> {
  let still = 0;
  let last = '';
  while (still < 10) {
    const box = await page.evaluate(() => window.__BOX__);
    if (page.viewportSize()!.height !== box.height) {
      await page.setViewportSize({ width: box.width, height: box.height });
    }
    const key = JSON.stringify(box);
    still = key === last ? still + 1 : 0;
    last = key;
    await page.waitForTimeout(20);
  }
  return JSON.parse(last);
}

test('a long sentence grows the window upward and a short one shrinks it back', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 110 });
  await page.addInitScript(`${STUB}; ${WINDOW(110, 2)}`);
  await page.goto('/');

  await show(page, 'なかなかのエセネイティブ感が出ててグッドだ。'.repeat(6));
  const grown = await follow(page);
  expect(grown.height).toBeGreaterThan(110);
  // Up holds the bottom edge.
  expect(grown.y + grown.height).toBe(610);

  await show(page, 'これで気にせず動けるよ');
  expect(await follow(page)).toEqual({ x: 100, y: 500, width: 600, height: 110 });
});

// A window sized to one line but a few px short of its bottom padding, and one
// extra line allowed: a two-line sentence must fit with no scroll. The cap
// once counted px, so those few px came out of it and the pane scrolled by
// the padding.
test('one extra line shows a two-line sentence in full from a base cut short', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 95 });
  await page.addInitScript(`${STUB}; ${WINDOW(95, 1)}`);
  await page.goto('/');

  await show(page, 'なかなかのエセネイティブ感が出ててグッドだ。'.repeat(2));
  await follow(page);
  const overflow = await page.evaluate(() => {
    const panes = document.querySelector<HTMLElement>('.panes')!;
    return panes.scrollHeight - panes.clientHeight;
  });
  expect(overflow).toBeLessThanOrEqual(0);
});
