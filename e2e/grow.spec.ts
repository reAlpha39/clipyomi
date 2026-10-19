import { expect, test, type Page } from '@playwright/test';
import { STUB } from './stub';

declare global {
  interface Window {
    __TA_EMIT__: (event: string, payload: unknown) => void;
    __BOX__: { x: number; y: number; width: number; height: number };
  }
}

// Fit-to-text needs a real layout engine: the measurement reads the page's
// geometry, which happy-dom does not compute. The stub below plays the
// window — `set_window_box` records the box, and `follow` applies it to the
// viewport the way the OS would resize the webview.
//
// It answers as Windows, where the band leaves the grid flow and `.panes`
// lands in an `auto` row: a short sentence's pane is only as tall as the
// sentence, which is the layout that once kept a grown window from shrinking.
const BASE = { x: 100, y: 500, width: 600, height: 110 };
const WINDOW = (settings: object) => `
  window.__BOX__ = ${JSON.stringify(BASE)};
  const stubbed = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
    if (cmd === 'platform') return 'windows';
    if (cmd === 'is_macos' || cmd === 'peek_grows_frame') return false;
    if (cmd === 'get_settings') {
      return { always_on_top: false, clipboard_monitoring: true, chrome_mode: 'header',
               grow_direction: 'up', grow_lines: 2, grow_animate: false, ...${JSON.stringify(settings)} };
    }
    if (cmd === 'plugin:window|inner_size') return { width: window.__BOX__.width, height: window.__BOX__.height };
    if (cmd === 'plugin:window|outer_position') return { x: window.__BOX__.x, y: window.__BOX__.y };
    if (cmd === 'plugin:window|scale_factor') return 1;
    if (cmd === 'plugin:window|is_minimized' || cmd === 'plugin:window|is_maximized') return false;
    if (cmd === 'set_window_box') { window.__BOX__ = args; return null; }
    return stubbed(cmd, args);
  };
`;

const LONG = 'なかなかのエセネイティブ感が出ててグッドだ。'.repeat(6);
const TWO_LINES = 'なかなかのエセネイティブ感が出ててグッドだ。'.repeat(2);
const SHORT = 'これで気にせず動けるよ';

const parse = (text: string) => ({
  segments: [{ start: 0, len: text.length, surface: text, reading: null, matched: false, entries: [] }],
  alternatives: [],
});

async function open(page: Page, settings: object = {}): Promise<void> {
  await page.setViewportSize({ width: BASE.width, height: BASE.height });
  await page.addInitScript(`${STUB}; ${WINDOW(settings)}`);
  await page.goto('/');
}

async function show(page: Page, text: string): Promise<void> {
  await page.evaluate((p) => window.__TA_EMIT__('parse-result', p), parse(text));
}

/** Keep the viewport on the box last set, until it holds still. */
async function follow(page: Page): Promise<typeof BASE> {
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

/** The page's vertical geometry: the gap under the text, its lines, any scroll. */
function measure(page: Page) {
  return page.evaluate(() => {
    const panes = document.querySelector<HTMLElement>('.panes')!;
    const sentence = document.querySelector<HTMLElement>('.sentence')!;
    const padBottom = Number.parseFloat(getComputedStyle(panes).paddingBottom);
    const line = Number.parseFloat(getComputedStyle(sentence).lineHeight);
    return {
      gapUnderText: window.innerHeight - padBottom - sentence.getBoundingClientRect().bottom,
      linesShown: (panes.clientHeight - 2 * padBottom) / line,
      overflow: panes.scrollHeight - panes.clientHeight,
    };
  });
}

test('a long sentence fits the window to the line cap, holding the bottom edge', async ({ page }) => {
  await open(page);
  await show(page, LONG);
  const box = await follow(page);
  expect(box.y + box.height).toBe(BASE.y + BASE.height);
  const m = await measure(page);
  expect(m.linesShown).toBeCloseTo(2, 1);
  expect(m.overflow).toBeGreaterThan(0);
});

// Hugging: a one-line sentence fits a one-line window, below the size the
// user set, and turning the fit off hands that size back exactly.
test('a short sentence hugs the window below the user size, and off restores it', async ({ page }) => {
  await open(page);
  await show(page, LONG);
  await follow(page);
  await show(page, SHORT);
  const box = await follow(page);
  expect(box.height).toBeLessThan(BASE.height);
  expect(box.y + box.height).toBe(BASE.y + BASE.height);
  const m = await measure(page);
  expect(m.gapUnderText).toBeGreaterThanOrEqual(0);
  expect(m.gapUnderText).toBeLessThan(1.5);

  await page.evaluate(() => window.__TA_EMIT__('settings-changed', { grow_direction: 'off' }));
  expect(await follow(page)).toEqual(BASE);
});

// What the padding box promises: at 0 nothing sits between the text and the
// window's bottom edge, and a sentence within the cap never scrolls.
test('no padding leaves no gap under the text and no scroll', async ({ page }) => {
  await open(page, { pane_padding: 0 });
  await show(page, TWO_LINES);
  await follow(page);
  const m = await measure(page);
  expect(m.overflow).toBeLessThanOrEqual(0);
  expect(m.gapUnderText).toBeGreaterThanOrEqual(0);
  expect(m.gapUnderText).toBeLessThan(1.5);
});
