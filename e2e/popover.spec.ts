import { expect, test } from '@playwright/test';

declare global {
  interface Window {
    __TA_EMIT__: (event: string, payload: unknown) => void;
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

/** Stub the event bridge before the page's module runs. */
const STUB = `
  window.__TA_CB__ = () => {};
  window.__TA_EMIT__ = (event, payload) => window.__TA_CB__({ event, payload });
  window.__TAURI_INTERNALS__ = {
    transformCallback: (cb) => { window.__TA_CB__ = cb; return 1; },
    invoke: () => Promise.resolve(1),
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

  const many = [
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
  await page.evaluate((entries) => window.__TA_EMIT__('popover-content', entries), many);

  // 2G truncated here and offered no way to read the rest; spec §3.4 reverses
  // that, and this is the assertion that fails if `overflow-y` regresses.
  const scrollable = await page
    .locator('#tooltip')
    .evaluate((el) => el.scrollHeight > el.clientHeight);
  expect(scrollable).toBe(true);
});
