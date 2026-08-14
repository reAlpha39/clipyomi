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

/**
 * A sentence long enough to wrap, so some chip ends a line hard against the
 * right edge — the case the horizontal clamp exists for. The committed
 * fixture's three-chip sentence never gets near it.
 */
async function emitWideResult(page: Page): Promise<void> {
  await page.evaluate(() => {
    const segments = Array.from({ length: 24 }, (_, i) => ({
      start: i,
      len: 1,
      surface: '本',
      reading: 'ほん',
      matched: true,
      entries: [
        {
          headword: '本',
          reading: 'ほん',
          conjugation: null,
          pos: ['n'],
          senses: [{ pos: ['n'], glosses: ['book'], xrefs: [], misc: [], info: [] }],
          flags: ['primary'],
        },
      ],
    }));
    window.__TA_EMIT__('parse-result', { segments });
  });
}

/** Index of the chip whose left edge is furthest right — the one nearest the edge. */
async function rightmostChip(page: Page): Promise<number> {
  const chips = page.locator('.chip');
  const count = await chips.count();
  let index = 0;
  let furthest = -1;
  for (let i = 0; i < count; i += 1) {
    const box = await chips.nth(i).boundingBox();
    if (box !== null && box.x > furthest) {
      furthest = box.x;
      index = i;
    }
  }
  return index;
}

test('dwelling on a chip opens its definition above it, inside the viewport', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const chip = page.locator('.chip').first();
  await chip.hover();
  // No explicit wait for the 350 ms dwell: the visibility assertion auto-waits,
  // which is the deterministic form of the same check.
  const popover = page.locator('.entry-popover');
  await expect(popover).toBeVisible();
  await expect(popover).toContainText('Tokyo');

  const chipBox = await chip.boundingBox();
  const popBox = await popover.boundingBox();
  if (chipBox === null || popBox === null) throw new Error('no box');
  expect(popBox.y + popBox.height).toBeLessThanOrEqual(chipBox.y);
  expect(popBox.x).toBeGreaterThanOrEqual(8);
  expect(popBox.y).toBeGreaterThanOrEqual(0);
});

test('a chip at the right edge gets a clamped popover, not an overflowing one', async ({
  page,
}) => {
  await page.setViewportSize({ width: 480, height: 320 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitWideResult(page);

  const popover = page.locator('.entry-popover');

  // Hover a left-side chip first, so the popover's inline `left` is already
  // set to a small value before the rightmost chip is ever measured. This is
  // what exercises the stale-position bug: measuring the second open while
  // the first's `left` is still applied would starve `getBoundingClientRect()`
  // of the viewport width it needs, letting an undersized measurement pass
  // the clamp untouched. Hovering only the rightmost chip, as this spec used
  // to, can never see that — it is always the first open.
  await page.locator('.chip').first().hover();
  await expect(popover).toBeVisible();

  const index = await rightmostChip(page);
  await page.locator('.chip').nth(index).hover();
  await expect(popover).toBeVisible();

  const popBox = await popover.boundingBox();
  if (popBox === null) throw new Error('no box');
  // 8px margin, with a pixel of slack for fractional layout.
  expect(popBox.x + popBox.width).toBeLessThanOrEqual(480 - 8 + 1);
});

test('focusing a chip opens the popover, and Escape closes it without losing focus', async ({
  page,
}) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  await page.locator('.chip').first().focus();
  const popover = page.locator('.entry-popover');
  await expect(popover).toBeVisible();

  await page.keyboard.press('Escape');
  await expect(popover).not.toBeVisible();
  // The chip keeps focus, so the user does not have to Tab back in.
  await expect(page.locator('.chip').first()).toBeFocused();
});

// Task 2 could not prove this: its fixture has no unmatched run, and inventing
// one there would have tested the mock rather than the guard.
test('an unmatched run opens no popover', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await page.evaluate(() => {
    window.__TA_EMIT__('parse-result', {
      segments: [{ start: 0, len: 3, surface: 'xyz', reading: null, matched: false, entries: [] }],
    });
  });

  await page.locator('.unmatched').hover();
  // Past the 350 ms dwell: without this the assertion is true the instant it
  // runs, and would pass even with the `.chip` guard deleted (controller
  // ruling, task-3-brief.md — the brief's original immediate assertion is a
  // vacuous pass regardless of the guard).
  await page.waitForTimeout(400);
  await expect(page.locator('.entry-popover')).not.toBeVisible();
});

for (const theme of THEMES) {
  test(`the popover renders correctly in ${theme}`, async ({ page }) => {
    await page.setViewportSize({ width: 480, height: 320 });
    await page.emulateMedia({ colorScheme: theme });
    await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
    await page.goto('/');
    await emitFixtureResult(page);

    await page.locator('.chip').first().hover();
    const popover = page.locator('.entry-popover');
    await expect(popover).toBeVisible();

    // Load-bearing per design §3.3, not polish: this is the entire anti-flicker
    // argument (the popover can never intercept the hover that opened it), so
    // it needs its own assertion rather than riding along on the screenshot.
    expect(await popover.evaluate((el) => getComputedStyle(el).pointerEvents)).toBe('none');

    // Local only, exactly as every other baseline in this file: CI is
    // ubuntu-latest and these were written on macOS.
    if (!process.env.CI) {
      await expect(page).toHaveScreenshot(`panes-popover-${theme}.png`);
    }
  });
}

// Design §5: the reduced-motion override lives in a `@media` block placed
// after `.entry-popover`'s own transition rule, exactly as Phase 2F's dead
// override did not — that one sat before its base rule and lost the cascade
// silently, caught only by a human reading the CSS, not a test. This proves
// the live behaviour that ordering is protected by: no baseline, since it
// asserts a computed style rather than pixels.
test('respects prefers-reduced-motion: the popover opens with no transition', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 480 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.addInitScript(`window.__FIXTURE__ = ${JSON.stringify(fixture)}; ${STUB}`);
  await page.goto('/');
  await emitFixtureResult(page);

  const popover = page.locator('.entry-popover');
  await page.locator('.chip').first().hover();
  await expect(popover).toBeVisible();

  expect(await popover.evaluate((el) => getComputedStyle(el).transitionProperty)).toBe('none');
});
