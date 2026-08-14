import { describe, expect, test } from 'vitest';
import { placePopover, shouldKeep, type Rect } from './popover';

/** A chip 40×28 at (60,100), in screen coordinates. */
function chip(overrides: Partial<Rect> = {}): Rect {
  return { left: 60, top: 100, right: 100, bottom: 128, ...overrides };
}

const SIZE = { width: 200, height: 60 };
/** A 1000×700 work area at the origin — no dock, no menu bar. */
const WORK: Rect = { left: 0, top: 0, right: 1000, bottom: 700 };

describe('placePopover', () => {
  // Below is preferred, reversing 2G. That reasoning was about what the
  // popover covered inside the window; a tooltip that leaves the window
  // mostly covers desktop, and below-first is ta-old's behaviour.
  test('sits below the chip when there is room, with the 2px gap', () => {
    expect(placePopover(chip(), SIZE, WORK)).toEqual({ left: 60, top: 130 });
  });

  // A word near the bottom of the screen has no room below, and the tooltip
  // would otherwise be pushed under the dock.
  test('flips above when below would overflow the work area', () => {
    expect(placePopover(chip({ top: 650, bottom: 678 }), SIZE, WORK).top).toBe(650 - 60 - 2);
  });

  // Neither side fits: pin to the work-area bottom and go right of the word.
  // This is MyToolTip.cpp:518-523, the case 2G had no answer for.
  test('pins to the bottom and moves right when neither side fits', () => {
    const tall = { width: 200, height: 690 };
    expect(placePopover(chip({ top: 300, bottom: 328 }), tall, WORK)).toEqual({
      left: 102,
      top: 10,
    });
  });

  // No room on the right either, so it goes to the word's left.
  test('moves left when there is no room on the right', () => {
    const tall = { width: 200, height: 690 };
    const right = chip({ left: 850, right: 890, top: 300, bottom: 328 });
    expect(placePopover(right, tall, WORK).left).toBe(850 - 200 - 2);
  });

  // The horizontal clamp is last, so a tooltip anchored near the right edge is
  // pulled back inside rather than hanging off it.
  test('clamps the right edge into the work area', () => {
    expect(placePopover(chip({ left: 950, right: 990 }), SIZE, WORK).left).toBe(1000 - 200 - 8);
  });

  // The left clamp wins over the right one, so a tooltip wider than the work
  // area pins at the left margin instead of going negative.
  test('pins to the left margin when wider than the work area', () => {
    expect(placePopover(chip(), { width: 1200, height: 60 }, WORK).left).toBe(8);
  });

  // A work area that does not start at the origin — a Mac with a menu bar, or
  // a second monitor to the right of the first.
  test('respects a work area with a non-zero origin', () => {
    const work: Rect = { left: 1000, top: 25, right: 2000, bottom: 700 };
    expect(placePopover(chip({ left: 1060, right: 1100 }), SIZE, work)).toEqual({
      left: 1060,
      top: 130,
    });
  });
});

describe('shouldKeep', () => {
  const centre = { x: 500, y: 500 };

  // ta-old's rule (MyToolTip.cpp:354): moving toward the tooltip keeps it, so
  // the cursor can cross the gap between the word and the tooltip to scroll it.
  test('keeps it when the cursor moves toward the tooltip', () => {
    expect(shouldKeep({ x: 0, y: 0 }, { x: 100, y: 100 }, centre)).toBe(true);
  });

  test('dismisses it when the cursor moves away', () => {
    expect(shouldKeep({ x: 100, y: 100 }, { x: 0, y: 0 }, centre)).toBe(false);
  });

  // Equal distance is not "toward": a cursor circling at a fixed radius is not
  // heading for the tooltip, and treating that as intent would make the
  // tooltip impossible to dismiss by moving sideways.
  test('dismisses it when the distance does not change', () => {
    expect(shouldKeep({ x: 500, y: 400 }, { x: 400, y: 500 }, centre)).toBe(false);
  });
});
