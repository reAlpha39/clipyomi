import { expect, test } from '@playwright/test';
// Playwright loads this file through Node's native ESM loader (not Vite), which
// requires an explicit import attribute for JSON modules on Node 22+.
import fixture from '../src/fixtures/tokyo.json' with { type: 'json' };
import { STUB } from './stub';

const SIZES = [
  { name: 'compact', width: 480, height: 320 },
  { name: 'default', width: 720, height: 480 },
];
const THEMES = ['light', 'dark'] as const;

for (const size of SIZES) {
  for (const theme of THEMES) {
    test(`panes render at ${size.name} in ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width: size.width, height: size.height });
      await page.emulateMedia({ colorScheme: theme });
      await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
      await page.goto('/');

      await page.fill('#text', '東京は');
      await page.click('#parse');

      await expect(page.locator('.chip').first()).toBeVisible();
      await expect(page.locator('.def-row')).toHaveCount(2);

      // Screenshot baselines here were written and eyeballed on macOS (see the
      // task report). CI runs ubuntu-latest, where font rendering differs
      // enough that these same baselines would mismatch on every run — a
      // permanently-red visual job that teaches everyone to ignore CI. Rather
      // than commit baselines we cannot generate on Linux (no git remote here
      // to run a Linux CI job and capture its output), the pixel comparison
      // runs locally only; CI still gets the DOM assertions above on every run.
      if (!process.env.CI) {
        await expect(page).toHaveScreenshot(`panes-${size.name}-${theme}.png`);
      }
    });
  }
}

test('a chip click marks its definition row', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');

  await page.click('.chip[data-start="2"]');
  await expect(page.locator('.def-row[data-start="2"]')).toHaveClass(/marked/);
});

test('keyboard activation marks the same row a click does, on Enter and Space', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');

  await page.locator('.chip[data-start="2"]').focus();
  await page.keyboard.press('Enter');
  await expect(page.locator('.def-row[data-start="2"]')).toHaveClass(/marked/);

  await page.locator('.chip[data-start="0"]').focus();
  await page.keyboard.press('Space');
  await expect(page.locator('.def-row[data-start="0"]')).toHaveClass(/marked/);
  // Only one row marked at a time.
  await expect(page.locator('.def-row[data-start="2"]')).not.toHaveClass(/marked/);
});

test('an unmatched run is not in the tab order', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');

  expect(await page.locator('.unmatched').evaluate((el) => (el as HTMLElement).tabIndex)).toBe(-1);
});
