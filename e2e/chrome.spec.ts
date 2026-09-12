import { expect, test } from '@playwright/test';
import { STUB } from './stub';

const THEMES = ['light', 'dark'] as const;
const MODES = ['native', 'header', 'content'] as const;

/**
 * A per-mode stub, built the way `panes.spec.ts` builds `HIDDEN_TITLEBAR_STUB`
 * and `WINDOWS_STUB`: wrap `STUB`'s `invoke` and override only what this file
 * needs to pin — the chrome mode and the platform the CSS keys off.
 */
function stubFor(
  mode: (typeof MODES)[number],
  platform: 'macos' | 'windows',
  alpha?: number,
): string {
  return `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'platform') return Promise.resolve('${platform}');
    if (cmd === 'is_macos') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'peek_grows_frame') return Promise.resolve(${platform === 'macos'});
    if (cmd === 'get_settings') return Promise.resolve({ chrome_mode: '${mode}', surface_alpha: ${alpha ?? 'undefined'} });
    return realInvoke(cmd, args);
  };
`;
}

test('content mode paints nothing above the sentence', async ({ page }) => {
  await page.setViewportSize({ width: 768, height: 480 });
  await page.addInitScript(stubFor('content', 'macos'));
  await page.goto('/');

  const app = page.locator('#app');
  const band = page.locator('header.controls');
  await expect(app).toHaveAttribute('data-chrome', 'content');
  // Same 6px sliver header mode idles at (both modes share the
  // `:not([data-chrome='native'])` rule) -- content mode just never grows it.
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(6);
  await expect(page.locator('#settings-toggle')).toHaveCSS('visibility', 'hidden');
  await expect(page.locator('.history-controls')).toHaveCSS('visibility', 'hidden');
  await expect(page.locator('.window-controls')).toHaveCSS('visibility', 'hidden');

  // `showPeek()` early-returns for every mode but `header` (main.ts), so a
  // pointer inside the window must not reveal anything here.
  await page.mouse.move(384, 200);
  await expect(app).not.toHaveClass(/peeked/);
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(6);
});

// The WCAG 2.4.7 check `visibility: hidden` (rather than `opacity: 0`) exists
// to satisfy: a control that is invisible AND can never be revealed must not
// be a tab stop, or a keyboard user tabs into a trap. happy-dom resolves no
// styles, so this can only be written here.
test('content mode leaves no tab stop in the band', async ({ page }) => {
  await page.addInitScript(stubFor('content', 'macos'));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'content');

  await page.keyboard.press('Tab');

  const landedInBand = await page.evaluate(
    () => document.activeElement?.closest('header.controls') !== null,
  );
  expect(landedInBand).toBe(false);
});

// Deliberately a near-duplicate of panes.spec.ts's "a hidden title bar leaves
// the gear invisible until the band is hovered": this file has to fail if
// content mode's rules ever leak onto header mode. The rule at risk is
// global.css's `#app[data-chrome='content'] .history-controls, ... {
// visibility: hidden }` -- if that selector is ever broadened to the
// `:not([data-chrome='native'])` form its neighbours use (the plausible
// mistake), header mode's controls go invisible too. So this test asserts
// `visibility: visible` on top of the opacity panes.spec.ts already checks:
// panes.spec.ts's test would sail straight past a broadened visibility rule
// since it only reads opacity, so it alone would not catch that leak.
//
// No `#output` position assertion here. On macOS the OS window frame grows
// upward by the band height on reveal (`peek_grows_frame` true, per
// `stubFor`), which Playwright cannot see; `peek-offset` compensates inside
// the tab, so `#output` legitimately moves down by 28px in what Playwright
// measures while staying still on the physical screen. That fork already has
// two owners in panes.spec.ts -- one pinning `=== 28` with `peek_grows_frame`
// true, one pinning `=== 0` with it false -- and re-asserting either number
// here would be a second maintenance point with no new coverage.
test('header mode still reveals on hover', async ({ page }) => {
  await page.addInitScript(stubFor('header', 'macos'));
  await page.goto('/');

  const band = page.locator('header.controls');
  const gear = page.locator('#settings-toggle');
  const history = page.locator('.history-controls');
  const windowControls = page.locator('.window-controls');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'header');
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(6);

  await band.hover();

  await expect(gear).toHaveCSS('opacity', '1');
  await expect(gear).toHaveCSS('visibility', 'visible');
  await expect(history).toHaveCSS('opacity', '1');
  await expect(history).toHaveCSS('visibility', 'visible');
  await expect(windowControls).toHaveCSS('opacity', '1');
  await expect(windowControls).toHaveCSS('visibility', 'visible');
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(28);
});

test('native mode reserves the row', async ({ page }) => {
  await page.addInitScript(stubFor('native', 'macos'));
  await page.goto('/');

  const band = page.locator('header.controls');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'native');
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(28);
  await expect(page.locator('#settings-toggle')).toHaveCSS('opacity', '1');
});

// The right-click menu is drawn by the OS now (see `context-menu.ts`), so
// Playwright cannot see it: what it offers, where it opens and how it closes
// are pinned in `main.test.ts` on the wire shape, and in `cargo test` on the
// menu the backend builds from it.

// Pixels at 768 in both themes for each mode. 320 and 1024 are covered below
// by assertions on the band's measured height and the controls' resolved
// visibility instead -- a third and fourth screenshot of a 28px band carries
// no information those assertions do not.
for (const mode of MODES) {
  for (const theme of THEMES) {
    test(`${mode} chrome renders correctly in ${theme}`, async ({ page }) => {
      await page.setViewportSize({ width: 768, height: 480 });
      await page.emulateMedia({ colorScheme: theme });
      await page.addInitScript(stubFor(mode, 'macos'));
      await page.goto('/');
      await expect(page.locator('#app')).toHaveAttribute('data-chrome', mode);

      if (!process.env.CI) {
        await expect(page).toHaveScreenshot(`chrome-${mode}-${theme}.png`);
      }
    });
  }
}

// No screenshots here: the band's height and the controls' resolved
// visibility are not width-dependent, so this is the information a third and
// fourth pixel baseline would carry, asserted directly instead.
for (const mode of MODES) {
  for (const width of [320, 1024]) {
    test(`band geometry holds at ${width}px in ${mode} mode`, async ({ page }) => {
      await page.setViewportSize({ width, height: 480 });
      await page.addInitScript(stubFor(mode, 'macos'));
      await page.goto('/');
      await expect(page.locator('#app')).toHaveAttribute('data-chrome', mode);

      const band = page.locator('header.controls');
      const expectedHeight = mode === 'native' ? 28 : 6;
      expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(
        expectedHeight,
      );

      const expectedVisibility = mode === 'content' ? 'hidden' : 'visible';
      await expect(page.locator('#settings-toggle')).toHaveCSS('visibility', expectedVisibility);
      await expect(page.locator('.history-controls')).toHaveCSS(
        'visibility',
        expectedVisibility,
      );
      await expect(page.locator('.window-controls')).toHaveCSS(
        'visibility',
        expectedVisibility,
      );
    });
  }
}

// The assertions below compare computed colours to each other rather than to
// a literal. `color-mix()` with an `oklch()` input serializes in whichever
// space the engine picks, so pinning a string would test Chromium's
// serializer, not the app.
test('the opacity setting reaches the window background', async ({ page }) => {
  await page.addInitScript(stubFor('header', 'macos', 100));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'header');

  const opaque = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);

  // The same path the slider drag uses: a partial `settings-changed` payload.
  await page.evaluate(() =>
    (window as unknown as { __TA_EMIT__: (e: string, p: unknown) => void }).__TA_EMIT__(
      'settings-changed',
      { surface_alpha: 0 },
    ),
  );

  const clear = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(clear).not.toBe(opaque);
});

// The clamp, from the outside: a stored 0 renders fully opaque under the
// native frame, and the stored value is not rewritten to say so.
test('native chrome clamps the opacity to fully opaque', async ({ page }) => {
  await page.addInitScript(stubFor('native', 'macos', 0));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'native');

  const token = await page.evaluate(() =>
    getComputedStyle(document.documentElement).getPropertyValue('--surface-alpha').trim(),
  );
  expect(token).toBe('1');
});

// The surface must be declared on `body` alone. Background propagation lifts
// it to the canvas by itself, so declaring it on `html` too paints it twice --
// a semi-transparent colour composites with itself and renders 1-(1-a)^2
// instead of the alpha the user asked for. Identical at 100%, which is why no
// screenshot baseline and no computed-style read of `body` can catch it.
test('the root element contributes no second background layer', async ({ page }) => {
  await page.addInitScript(stubFor('header', 'macos', 40));
  await page.goto('/');
  await expect(page.locator('#app')).toHaveAttribute('data-chrome', 'header');

  const rootBg = await page.evaluate(
    () => getComputedStyle(document.documentElement).backgroundColor,
  );
  expect(rootBg).toBe('rgba(0, 0, 0, 0)');
});
