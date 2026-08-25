import { describe, expect, test } from 'vitest';
import { MENU_MARGIN, clampMenu } from './context-menu';

const SIZE = { width: 140, height: 120 };
const VIEWPORT = { width: 400, height: 300 };

describe('clampMenu', () => {
  test('opens at the cursor when the menu fits below and right of it', () => {
    expect(clampMenu(20, 30, SIZE, VIEWPORT)).toEqual({ left: 20, top: 30 });
  });

  // Flipped, not slid: a menu that slid left would sit under the cursor and
  // swallow the click that opened it.
  test('flips left of the cursor when it would overflow the right edge', () => {
    expect(clampMenu(380, 30, SIZE, VIEWPORT)).toEqual({ left: 380 - SIZE.width, top: 30 });
  });

  test('flips above the cursor when it would overflow the bottom edge', () => {
    expect(clampMenu(20, 290, SIZE, VIEWPORT)).toEqual({ left: 20, top: 290 - SIZE.height });
  });

  test('flips on both axes at once', () => {
    expect(clampMenu(380, 290, SIZE, VIEWPORT)).toEqual({
      left: 380 - SIZE.width,
      top: 290 - SIZE.height,
    });
  });

  // The flip can itself go off the near edge in a small window, so the margin
  // clamp is applied last and wins.
  test('pins to the margin when the flip overshoots the near edge', () => {
    expect(clampMenu(20, 20, SIZE, { width: 150, height: 130 })).toEqual({
      left: MENU_MARGIN,
      top: MENU_MARGIN,
    });
  });

  // The case a window at the configured 160x80 minimum hits.
  test('pins to the margin when the menu is larger than the viewport', () => {
    expect(clampMenu(10, 10, { width: 400, height: 500 }, { width: 160, height: 80 })).toEqual({
      left: MENU_MARGIN,
      top: MENU_MARGIN,
    });
  });
});
