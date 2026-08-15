import { expect, test } from '@playwright/test';

declare global {
  interface Window {
    __TA_EMIT__: (event: string, payload: unknown) => void;
    /** The last `popover-measured` payload the page emitted, recorded by the stub. */
    __TA_MEASURED__: { width: number; height: number } | undefined;
  }
}

/**
 * The tooltip page, loaded directly.
 *
 * Playwright drives webviews, not the OS window manager, so it can prove what
 * the page renders and that it scrolls — but NOT that the window escapes the
 * main window's bounds, never steals focus, or clamps above the dock. Those
 * are Step 6's manual checks, and nothing here stands in for them.
 */
const ENTRIES = [
  {
    headword: '消える',
    reading: 'きえる',
    conjugation: null,
    pos: ['v1', 'vi'],
    senses: [
      { pos: ['v1', 'vi'], glosses: ['to disappear', 'to vanish'], xrefs: [], misc: [], info: [] },
      {
        pos: ['v1', 'vi'],
        glosses: ['to go out (of a fire, light, etc.)', 'to die'],
        xrefs: [],
        misc: [],
        info: [],
      },
    ],
    flags: ['primary', 'common'],
  },
];

/** Twenty senses: taller than any viewport these specs use. */
const MANY = [
  {
    ...ENTRIES[0],
    senses: Array.from({ length: 20 }, (_, i) => ({
      pos: ['v1'],
      glosses: [`sense number ${i + 1} with enough text to wrap onto a second line`],
      xrefs: [],
      misc: [],
      info: [],
    })),
  },
];

/** Stub the event bridge before the page's module runs. */
const STUB = `
  window.__TA_CB__ = () => {};
  window.__TA_EMIT__ = (event, payload) => window.__TA_CB__({ event, payload });
  window.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { window.__TA_CB__ = cb; return 1; },
    invoke: (cmd, args) => {
      // \`emit()\` from @tauri-apps/api dispatches through here as
      // \`plugin:event|emit\` (read from node_modules/@tauri-apps/api/event.js).
      // Recording it is the only way to observe what the page measured — there
      // is no Rust side in this suite to receive it.
      if (cmd === 'plugin:event|emit' && args.event === 'popover-measured') {
        window.__TA_MEASURED__ = args.payload;
      }
      return Promise.resolve(1);
    },
  };
`;

test('renders every sense, coloured by ta-old rules', async ({ page }) => {
  await page.addInitScript(STUB);
  await page.goto('/popover.html');
  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), ENTRIES);

  const tooltip = page.locator('#tooltip');
  await expect(tooltip).toContainText('消える【きえる】');
  await expect(tooltip).toContainText('(1) to disappear/to vanish');
  await expect(tooltip).toContainText('(2)');
  // `(P)` comes from the `common` flag, on the last sense only.
  await expect(tooltip).toContainText('(P)');

  // The colour classes are the contract between the colouriser and the CSS.
  // Asserting classes rather than computed colours keeps this from breaking
  // every time a token is retuned.
  await expect(page.locator('.tt-kanji').first()).toHaveText('消える');
  await expect(page.locator('.tt-kana').first()).toHaveText('【きえる】');
  await expect(page.locator('.tt-paren').first()).toHaveText('(v1,vi)');
});

test('scrolls rather than truncating when the content is tall', async ({ page }) => {
  await page.setViewportSize({ width: 320, height: 120 });
  await page.addInitScript(STUB);
  await page.goto('/popover.html');

  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), MANY);

  // 2G truncated here and offered no way to read the rest; spec §3.4 reverses
  // that. `scrollHeight > clientHeight` only proves the content overflows the
  // box — it is true under `overflow: hidden` too, so it can't tell truncation
  // and scrolling apart on its own. The `overflow-y` check below is the one
  // that actually distinguishes them, and is what fails if it regresses.
  const tooltip = page.locator('#tooltip');
  const overflows = await tooltip.evaluate((el) => el.scrollHeight > el.clientHeight);
  expect(overflows).toBe(true);

  const overflowY = await tooltip.evaluate((el) => getComputedStyle(el).overflowY);
  expect(overflowY).toBe('auto');
});

/**
 * The only automated observation of `src/popover.ts`'s measurement.
 *
 * It used to measure `#tooltip`, which is `height: 100%` of the window and
 * therefore floors `scrollHeight`/`scrollWidth` at its own padding box
 * (CSSOM-View). A tall entry latched the height for every later hover — a
 * one-sense word became a near-full-screen empty box — and the width shrank by
 * the border width on every round trip, for the life of the process. Measuring
 * the content child fixes both, and nothing else in either suite reads the
 * emitted payload at all.
 */
async function measured(page: import('@playwright/test').Page) {
  const value = await page.evaluate(() => window.__TA_MEASURED__);
  if (value === undefined) throw new Error('no popover-measured was emitted');
  return value;
}

test('measures its own content, so a short entry does not inherit a tall window', async ({
  page,
}) => {
  await page.setViewportSize({ width: 320, height: 600 });
  await page.addInitScript(STUB);
  await page.goto('/popover.html');

  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), MANY);
  const tall = await measured(page);
  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), ENTRIES);
  const short = await measured(page);

  // The tall entry really did overflow the 600px viewport, so the short one
  // below is measured against a window that had every reason to latch.
  expect(tall.height).toBeGreaterThan(short.height);
  // Two senses of one entry: a handful of lines, nowhere near the viewport.
  expect(short.height).toBeLessThan(200);
  // Identical, not merely close: the width is the window's own inner width
  // plus the chrome the client box excludes, which is what stops the
  // per-round-trip shrink.
  expect(short.width).toBe(tall.width);
});
