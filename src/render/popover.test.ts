import { describe, expect, test } from 'vitest';
import { placePopover } from './popover';

/** A chip 40×28 sitting 100px down and 60px in from the viewport's left edge. */
function chip(overrides: Partial<DOMRect> = {}): DOMRect {
  const base = {
    x: 60,
    y: 100,
    width: 40,
    height: 28,
    top: 100,
    left: 60,
    right: 100,
    bottom: 128,
  };
  return { ...base, ...overrides } as DOMRect;
}

const POPOVER = { width: 200, height: 60 };
const VIEWPORT = { width: 480 };

describe('placePopover', () => {
  // Above by preference: above the sentence is the input row, which nobody is
  // reading, while below it are the definition rows, which they might be.
  test('sits above the chip when there is room, with the 6px gap', () => {
    const { top, left } = placePopover(chip(), POPOVER, VIEWPORT);
    expect(top).toBe(100 - 60 - 6);
    expect(left).toBe(60);
  });

  // A chip on the first line of a sentence has almost nothing above it. Placing
  // there anyway would push the popover off the top of the window, where the
  // user cannot read it and nothing scrolls it back.
  test('flips below the chip when there is no room above', () => {
    const { top } = placePopover(chip({ top: 20, bottom: 48 }), POPOVER, VIEWPORT);
    expect(top).toBe(48 + 6);
  });

  // The last chip of a wrapped line sits near the right edge; left-aligning the
  // popover to it would hang the popover's tail off-screen.
  test('clamps to the right edge rather than overflowing it', () => {
    const { left } = placePopover(chip({ left: 400, right: 440 }), POPOVER, VIEWPORT);
    expect(left).toBe(480 - 200 - 8);
  });

  // The left clamp is applied last, so a popover wider than the viewport is
  // pinned at the left margin rather than pushed off the left edge by the
  // right-edge clamp producing a negative value.
  test('pins to the left margin when the popover is wider than the viewport', () => {
    const { left } = placePopover(chip({ left: 4 }), { width: 600, height: 60 }, VIEWPORT);
    expect(left).toBe(8);
  });
});
