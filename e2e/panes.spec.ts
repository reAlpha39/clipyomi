import { expect, test, type Page } from '@playwright/test';
// Playwright loads this file through Node's native ESM loader (not Vite), which
// requires an explicit import attribute for JSON modules on Node 22+.
import fixture from '../src/fixtures/tokyo.json' with { type: 'json' };
import { STUB } from './stub';

declare global {
  interface Window {
    __FIXTURE__: unknown;
    __TA_EMIT__: (event: string, payload: unknown) => void;
  }
}

// The backend now pushes results as a `parse-result` event rather than
// returning them from `invoke`; `#parse` still triggers `set_input` for real
// (see stub.ts), but firing the fixture back in is on the test, standing in
// for the backend's async worker.
async function emitFixtureResult(page: Page): Promise<void> {
  await page.evaluate(() => window.__TA_EMIT__('parse-result', window.__FIXTURE__));
}

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
      await emitFixtureResult(page);

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
  await emitFixtureResult(page);

  await page.click('.chip[data-start="2"]');
  await expect(page.locator('.def-row[data-start="2"]')).toHaveClass(/marked/);
});

test('keyboard activation marks the same row a click does, on Enter and Space', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');
  await emitFixtureResult(page);

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
  await emitFixtureResult(page);

  expect(await page.locator('.unmatched').evaluate((el) => (el as HTMLElement).tabIndex)).toBe(-1);
});

// The header toggles don't need a parse result to exist, so no fixture here
// — just the STUB, since `#monitor` still calls the real (stubbed)
// `set_clipboard_monitoring` command on activation. Real Chromium implements
// the spec's disabled-blurs-focus behaviour that happy-dom does not (see the
// task report), so this is the one place that can actually prove activating
// a toggle never drops keyboard focus — the closure-local `pending` guard in
// `bindToggle` is what makes that true without ever touching `.disabled`.
test('activating a toggle keeps keyboard focus', async ({ page }) => {
  await page.addInitScript(STUB);
  await page.goto('/');

  const monitor = page.locator('#monitor');
  await monitor.focus();
  await page.keyboard.press('Enter');
  await expect(monitor).toBeFocused();
});

// Runs everywhere, including CI (no `!process.env.CI` guard) — a screenshot
// diff is skipped there, so this is what actually protects the focus ring
// and the marked-row border from a silent CSS regression on that runner.
// Reads resolved styles via getComputedStyle rather than asserting on the
// class name alone, which would still pass if the underlying CSS rule were
// deleted.
test('a focused chip resolves a real outline, and a marked row is border-distinguishable', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.fill('#text', '東京は');
  await page.click('#parse');
  await emitFixtureResult(page);

  // :focus-visible needs real keyboard traversal: a bare `.focus()` call
  // does not flip Chromium's focus modality after the preceding mouse click
  // on #parse (confirmed by hand while writing this fix — see the report).
  await page.keyboard.press('Tab'); // #parse -> 東京
  await page.keyboard.press('Tab'); // 東京 -> は
  const chip = page.locator('.chip[data-start="2"]');
  const outlineStyle = await chip.evaluate((el) => getComputedStyle(el).outlineStyle);
  const outlineWidth = await chip.evaluate((el) => parseFloat(getComputedStyle(el).outlineWidth));
  expect(outlineStyle).not.toBe('none');
  expect(outlineWidth).toBeGreaterThan(0);

  await page.keyboard.press('Enter');
  const markedColor = await page
    .locator('.def-row[data-start="2"]')
    .evaluate((el) => getComputedStyle(el).borderLeftColor);
  const unmarkedColor = await page
    .locator('.def-row[data-start="0"]')
    .evaluate((el) => getComputedStyle(el).borderLeftColor);
  expect(markedColor).not.toBe(unmarkedColor);
});

// A committed baseline of the activated state, so the two cues above have a
// visual proof that persists in the repo rather than living only in a
// throwaway check. Compact size only, both themes: the cues are theme-
// dependent (colour tokens) but not size-dependent (no size-specific CSS on
// `outline`/`border-left`), so a second size would add baseline surface with
// no extra coverage. One screenshot per theme captures both cues at once —
// activating a chip with Enter leaves focus on it, so the ring and the
// marked row's border are both visible in the same frame.
for (const theme of THEMES) {
  test(`activated chip and marked row render correctly in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
    await page.goto('/');
    await page.fill('#text', '東京は');
    await page.click('#parse');
    await emitFixtureResult(page);

    await page.keyboard.press('Tab');
    await page.keyboard.press('Tab');
    await page.keyboard.press('Enter');
    await expect(page.locator('.def-row[data-start="2"]')).toHaveClass(/marked/);

    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-activated-${theme}.png`);
    }
  });
}
