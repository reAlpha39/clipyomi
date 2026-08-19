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

// The backend pushes results as a `parse-result` event rather than returning
// them from `invoke`, so firing the fixture back in is on the test, standing in
// for the backend's async worker. Phase 2H removed the manual input the specs
// used to click; it never reached Rust here anyway, since Playwright runs
// against the stub.
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

      await emitFixtureResult(page);

      await expect(page.locator('.chip').first()).toBeVisible();
      await expect(page.locator('.sentence')).toBeVisible();
      await expect(page.locator('.def-row')).toHaveCount(0);

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

test('an unmatched run is not in the tab order', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  expect(await page.locator('.unmatched').evaluate((el) => (el as HTMLElement).tabIndex)).toBe(-1);
});

// The titlebar band. With `decorations: false` the band leaves the grid, so
// these tests are the ones that prove the promise the design rests on: the
// sentence sits at the window's top edge and does NOT move when the reveal
// happens. happy-dom resolves no layout, so this can only be checked here.
// The OS traffic lights fading in alongside the gear is a real `peek_titlebar`
// round-trip that Playwright cannot see — that stays a manual macOS check.
const HIDDEN_TITLEBAR_STUB = `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'get_settings') return Promise.resolve({ decorations: false });
    return realInvoke(cmd, args);
  };
`;

test('a hidden title bar leaves the gear invisible until the band is hovered', async ({
  page,
}) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${HIDDEN_TITLEBAR_STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const gear = page.locator('#settings-toggle');
  const band = page.locator('header.controls');
  // Asserted on resolved opacity, not visibility: the button stays in the DOM
  // and in the tab order, which is the point of revealing it on focus too.
  await expect(gear).toHaveCSS('opacity', '0');
  // The idle band is the 6px sliver, thin enough to leave the chips beneath it
  // clickable.
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(6);

  await band.hover();

  await expect(gear).toHaveCSS('opacity', '1');
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(28);
});

test('revealing the band offsets the content by exactly the band height when the frame can grow', async ({
  page,
}) => {
  const stub = `
    ${HIDDEN_TITLEBAR_STUB}
    const innerInvoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
      if (cmd === 'is_macos') return Promise.resolve(true);
      if (cmd === 'peek_grows_frame') return Promise.resolve(true);
      return innerInvoke(cmd, args);
    };
  `;
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${stub}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const sentence = page.locator('.sentence');
  const before = await sentence.evaluate((el) => el.getBoundingClientRect().top);
  await page.locator('header.controls').hover();
  await expect(page.locator('#settings-toggle')).toHaveCSS('opacity', '1');
  const after = await sentence.evaluate((el) => el.getBoundingClientRect().top);

  // Inside the window the content moves DOWN by the band's height; the window
  // simultaneously grows upward by the same amount, so on screen the sentence
  // does not move at all. Only the first half is checkable here — a browser tab
  // has no frame to grow — so this pins the offset that the growth cancels.
  // The two halves matching is a manual macOS check.
  expect(after - before).toBe(28);
});

test('revealing the band leaves the content stationary when the frame cannot grow', async ({
  page,
}) => {
  const stub = `
    ${HIDDEN_TITLEBAR_STUB}
    const innerInvoke = window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
      if (cmd === 'peek_grows_frame') return Promise.resolve(false);
      return innerInvoke(cmd, args);
    };
  `;
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${stub}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const sentence = page.locator('.sentence');
  const before = await sentence.evaluate((el) => el.getBoundingClientRect().top);
  await page.locator('header.controls').hover();
  await expect(page.locator('#settings-toggle')).toHaveCSS('opacity', '1');
  const after = await sentence.evaluate((el) => el.getBoundingClientRect().top);

  expect(after - before).toBe(0);
});

// The Windows shape, checked on this machine because the branch is pure CSS and
// the only platform input is `is_macos`. What is NOT checked here: the Win32
// region that clips the band away while idle, which is what makes the always-on
// 28px band invisible on Windows. Without that clipping this layout would show a
// permanent empty strip — see the Windows test plan.
const WINDOWS_STUB = `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'is_macos' || cmd === 'peek_grows_frame') return Promise.resolve(false);
    if (cmd === 'platform') return Promise.resolve('windows');
    if (cmd === 'get_settings') return Promise.resolve({ decorations: false });
    return realInvoke(cmd, args);
  };
`;

test('the Windows shape keeps a full-height band and a permanent offset', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${WINDOWS_STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const band = page.locator('header.controls');
  const shell = page.locator('#app');
  expect(await band.evaluate((el) => el.getBoundingClientRect().height)).toBe(28);
  await expect(shell).toHaveCSS('padding-top', '28px');

  // The frame does not grow on Windows, so the reveal must not add a second
  // offset on top of the permanent one — that was the content-jump bug.
  const before = await page.locator('.sentence').evaluate((el) => el.getBoundingClientRect().top);
  await band.hover();
  await expect(page.locator('#settings-toggle')).toHaveCSS('opacity', '1');
  const after = await page.locator('.sentence').evaluate((el) => el.getBoundingClientRect().top);
  expect(after).toBe(before);
});

test('the gear reveals itself when it takes keyboard focus', async ({ page }) => {
  await page.addInitScript(HIDDEN_TITLEBAR_STUB);
  await page.goto('/');

  const gear = page.locator('#settings-toggle');
  await expect(gear).toHaveCSS('opacity', '0');

  // First stop from a fresh load: an invisible tab stop would be a trap, so
  // `:focus-within` reveals the band the same way `:hover` does.
  await page.keyboard.press('Tab');

  await expect(gear).toBeFocused();
  await expect(gear).toHaveCSS('opacity', '1');
});

// STUB's default `needs_dictionary` answer is `false` so every other spec in
// this file keeps exercising the parse path; the tests below need the
// opposite, so the override lives here rather than reshaping STUB itself.
// Wrapping the real invoke (rather than replacing __TAURI_INTERNALS__
// wholesale) keeps the `plugin:event|listen` handling STUB already provides.
// Shared by every download-screen test in this file, not just the first one.
const NEEDS_DICTIONARY_STUB = `
  ${STUB}
  const realInvoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = (cmd, args) => {
    if (cmd === 'needs_dictionary') return true;
    return realInvoke(cmd, args);
  };
`;

test('the first-run download screen appears and clears on ready', async ({ page }) => {
  await page.addInitScript(NEEDS_DICTIONARY_STUB);
  await page.goto('/');

  await expect(page.locator('#download')).toBeVisible();

  await page.evaluate(() => window.__TA_EMIT__('dictionary-status', 'ready'));
  await expect(page.locator('#dictionary')).toBeEmpty();
});

// Final review, Finding 3: the click handler used to call
// `renderDictionary('downloading')` synchronously, which `replaceChildren`s
// the very button the user just activated out of the DOM — real browsers
// move focus to `<body>` the instant a focused element is removed that way.
// `renderDictionary` now restores focus into whatever the new content
// offers (the live region itself when there is no button yet) whenever focus
// was inside `#dictionary` to begin with. This drives the whole journey —
// activation, the button-less in-flight phase, then a real failure — because
// each of those three renders is a separate DOM replacement that could
// independently drop focus; a single assertion at the end would not tell
// which step (if any) regressed.
test('activating Download keeps keyboard focus through a failure and into Retry', async ({
  page,
}) => {
  await page.addInitScript(NEEDS_DICTIONARY_STUB);
  await page.goto('/');

  const download = page.locator('#download');
  await download.focus();
  await page.keyboard.press('Enter');

  // The button is gone the instant the click handler re-renders to the
  // downloading phase, which has no button at all — focus must land on the
  // live region itself, never on <body>.
  await expect(page.locator('#dictionary')).toBeFocused();

  // The real failure surface is a `dictionary-status` event carrying an
  // error message, not a rejected `invoke` — `download_dictionary`'s own doc
  // comment is explicit that it fails this way on purpose (design §2.2).
  await page.evaluate(() => window.__TA_EMIT__('dictionary-status', 'a network error'));
  await expect(page.locator('#download')).toBeFocused();
});

// Final review, Finding 2: the download screen — the first screen a new user
// ever sees — had no visual baseline at all, which is why a dead
// reduced-motion CSS rule (Ruling 11) had to be caught by a human reading the
// cascade instead of a screenshot diff. Compact size only, both themes, same
// reasoning as the activated-chip baselines below: `.dictionary`/`.spinner`/
// `.dictionary button` carry no size-specific CSS, so a second viewport would
// add baseline surface with no extra coverage. Two states are captured
// because they are visually distinct and each has its own failure mode: the
// initial offer (button, `.dictionary` copy) and the in-flight phase
// (spinner, `@keyframes spin`) — Playwright's `toHaveScreenshot` default of
// `animations: 'disabled'` completes the infinite spin at the end of its
// first iteration (a full 360°, indistinguishable from the rest state) before
// capturing, so this is not flaky despite the animation being unbounded.
for (const theme of THEMES) {
  test(`the download screen's offer renders correctly in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(NEEDS_DICTIONARY_STUB);
    await page.goto('/');

    await expect(page.locator('#download')).toBeVisible();

    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-download-offer-${theme}.png`);
    }
  });

  test(`the download screen's in-flight phase renders correctly in ${theme}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(NEEDS_DICTIONARY_STUB);
    await page.goto('/');

    await page.evaluate(() => window.__TA_EMIT__('dictionary-status', 'downloading'));
    await expect(page.locator('.spinner')).toBeVisible();

    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-download-downloading-${theme}.png`);
    }
  });
}

// Runs everywhere, including CI (no `!process.env.CI` guard) — a screenshot
// diff is skipped there, so this is what actually protects the focus ring
// from a silent CSS regression on that runner.
// Reads resolved styles via getComputedStyle rather than asserting on the
// class name alone, which would still pass if the underlying CSS rule were
// deleted.
test('a focused chip resolves a real outline', async ({ page }) => {
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  // :focus-visible needs real keyboard traversal, and this test asserts a
  // ring the keyboard produces. There is no `#parse` to click into focus any
  // more, so every stop from a fresh load is walked by hand.
  await page.keyboard.press('Tab'); // -> #settings-toggle
  await page.keyboard.press('Tab'); // -> 東京
  await page.keyboard.press('Tab'); // 東京 -> は
  const chip = page.locator('.chip[data-start="2"]');
  const outlineStyle = await chip.evaluate((el) => getComputedStyle(el).outlineStyle);
  const outlineWidth = await chip.evaluate((el) => parseFloat(getComputedStyle(el).outlineWidth));
  expect(outlineStyle).not.toBe('none');
  expect(outlineWidth).toBeGreaterThan(0);
});

// A committed baseline of the activated state, so the cue above has a
// visual proof that persists in the repo rather than living only in a
// throwaway check. Compact size only, both themes: the cues are theme-
// dependent (colour tokens) but not size-dependent (no size-specific CSS on
// `outline`), so a second size would add baseline surface with
// no extra coverage. One screenshot per theme captures the cue —
// activating a chip with Enter leaves focus on it, so the ring is visible in
// the same frame.
for (const theme of THEMES) {
  test(`activated chip renders correctly in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
    await page.goto('/');
    await emitFixtureResult(page);

    await page.keyboard.press('Tab'); // -> #settings-toggle
    await page.keyboard.press('Tab'); // -> 東京
    await page.keyboard.press('Tab'); // 東京 -> は
    await page.keyboard.press('Enter');
    await expect(page.locator('.chip[data-start="2"]')).toBeFocused();

    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-activated-${theme}.png`);
    }
  });
}
