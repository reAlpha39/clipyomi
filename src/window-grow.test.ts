// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test, vi } from 'vitest';
import {
  DEFAULT_GROW_LINES,
  GROW_LINE_LIMITS,
  baseBox,
  clampGrowDirection,
  clampGrowLines,
  createGrower,
  fitText,
  grownBox,
  linePitch,
  type Box,
} from './window-grow';

describe('clampGrowDirection', () => {
  test('passes the three directions through', () => {
    expect(clampGrowDirection('off')).toBe('off');
    expect(clampGrowDirection('up')).toBe('up');
    expect(clampGrowDirection('down')).toBe('down');
  });

  // A hand-edited file, or none at all: the window keeps the size it was given.
  test('reads anything else as off', () => {
    expect(clampGrowDirection(undefined)).toBe('off');
    expect(clampGrowDirection('sideways')).toBe('off');
    expect(clampGrowDirection(1)).toBe('off');
  });
});

describe('clampGrowLines', () => {
  test('pins a value into range, rounded', () => {
    expect(clampGrowLines(3)).toBe(3);
    expect(clampGrowLines(2.6)).toBe(3);
    expect(clampGrowLines(0)).toBe(GROW_LINE_LIMITS.min);
    expect(clampGrowLines(99)).toBe(GROW_LINE_LIMITS.max);
  });

  test('falls back to the default for a non-number', () => {
    expect(clampGrowLines(undefined)).toBe(DEFAULT_GROW_LINES);
    expect(clampGrowLines(Number.NaN)).toBe(DEFAULT_GROW_LINES);
    expect(clampGrowLines('3')).toBe(DEFAULT_GROW_LINES);
  });
});

describe('linePitch', () => {
  test('is the whole height for one line', () => {
    expect(linePitch([40, 40, 41], 40, 12)).toBe(40);
  });

  // A chip carries 2px of padding an unmatched run does not, so bottoms on
  // one line differ by a few px; only a jump past the threshold is a new line.
  test('averages the height over the lines the bottoms step through', () => {
    expect(linePitch([40, 42, 40, 92, 94, 144], 150, 12)).toBe(50);
  });

  test('is zero with nothing rendered', () => {
    expect(linePitch([], 0, 12)).toBe(0);
  });
});

describe('fitText', () => {
  // The window hugs the text: as tall as it is, whatever the user's own size.
  test('is the text height while it is within the cap', () => {
    expect(fitText(82, 41, 3)).toBe(82);
    expect(fitText(81.6, 40.8, 3)).toBe(82);
  });

  test('stops at the cap and leaves the rest to the scrollbar', () => {
    expect(fitText(300, 41, 3)).toBe(123);
  });

  // Never shorter than one line, so a window between sentences, or showing
  // a one-character one, keeps a line's room.
  test('keeps at least one line', () => {
    expect(fitText(10, 41, 3)).toBe(41);
  });

  test('takes the text as it is without a measured line', () => {
    expect(fitText(57.5, 0, 3)).toBe(58);
  });
});

describe('grownBox and baseBox', () => {
  const base: Box = { x: 10, y: 500, width: 600, height: 200 };

  // Up holds the bottom edge where it is: over a VN textbox, that edge is
  // the one the user lined up.
  test('up moves the top and holds the bottom edge', () => {
    const grown = grownBox(base, 80, 'up');
    expect(grown).toEqual({ x: 10, y: 420, width: 600, height: 280 });
    expect(grown.y + grown.height).toBe(base.y + base.height);
  });

  test('down holds the top edge', () => {
    expect(grownBox(base, 80, 'down')).toEqual({ x: 10, y: 500, width: 600, height: 280 });
  });

  test('baseBox inverts grownBox either way', () => {
    expect(baseBox(grownBox(base, 80, 'up'), 80, 'up')).toEqual(base);
    expect(baseBox(grownBox(base, 80, 'down'), 80, 'down')).toEqual(base);
  });
});

/** A grower driven by a fake window, a fake clock and a manual frame queue. */
function harness(start: Box) {
  let live = start;
  let clock = 0;
  let frames: Array<(now: number) => void> = [];
  const boxes: Box[] = [];
  const readBox = vi.fn(() => Promise.resolve(live));
  const grower = createGrower({
    readBox,
    setBox: (box) => {
      boxes.push(box);
      live = box;
    },
    frame: (cb) => frames.push(cb),
    now: () => clock,
  });
  /** Run N frames 16 ms apart, or until the animation stops asking. */
  const step = (n = Number.POSITIVE_INFINITY) => {
    for (let i = 0; i < n && frames.length > 0; i += 1) {
      clock += 16;
      const due = frames;
      frames = [];
      for (const cb of due) cb(clock);
    }
  };
  return {
    grower,
    boxes,
    readBox,
    step,
    live: () => live,
    /** Move or resize the window as a user drag would. */
    drag: (box: Box) => {
      live = box;
    },
    /** Let the live-box read resolve. */
    settle: async () => {
      for (let i = 0; i < 3; i += 1) await Promise.resolve();
    },
  };
}

describe('createGrower', () => {
  const base: Box = { x: 10, y: 500, width: 600, height: 200 };

  // Hugging a short sentence takes the window below the user's own size; up
  // still holds the bottom edge, and the base is still the box to return to.
  test('shrinks below the base for a negative target, holding the bottom edge going up', async () => {
    const h = harness(base);
    h.grower.want(-80, 'up', false);
    await h.settle();
    expect(h.live()).toEqual({ x: 10, y: 580, width: 600, height: 120 });
    expect(h.grower.baseOf(h.live())).toEqual(base);
    h.step();
    h.grower.want(0, 'up', true);
    await h.settle();
    h.step();
    expect(h.live()).toEqual(base);
  });

  test('snaps to the target in one call with animation off', async () => {
    const h = harness(base);
    h.grower.want(80, 'down', false);
    await h.settle();
    expect(h.boxes).toEqual([{ x: 10, y: 500, width: 600, height: 280 }]);
    expect(h.grower.applied()).toBe(80);
  });

  test('animates to exactly the target, holding the bottom edge on every frame going up', async () => {
    const h = harness(base);
    h.grower.want(80, 'up', true);
    await h.settle();
    h.step();
    expect(h.boxes.length).toBeGreaterThan(2);
    for (const box of h.boxes) expect(box.y + box.height).toBe(700);
    expect(h.live()).toEqual({ x: 10, y: 420, width: 600, height: 280 });
  });

  test('shrinks back to exactly the box the user set', async () => {
    const h = harness(base);
    h.grower.want(80, 'up', true);
    await h.settle();
    h.step();
    h.grower.want(0, 'up', true);
    await h.settle();
    h.step();
    expect(h.live()).toEqual(base);
    expect(h.grower.applied()).toBe(0);
  });

  // A sentence arriving mid-animation changes course from wherever the
  // window is, rather than jumping back to the start or to the old target.
  test('a new target mid-animation continues from the current height', async () => {
    const h = harness(base);
    h.grower.want(120, 'down', true);
    await h.settle();
    h.step(3);
    const midway = h.live().height;
    expect(midway).toBeGreaterThan(200);
    expect(midway).toBeLessThan(320);
    h.grower.want(0, 'down', true);
    h.step(1);
    expect(Math.abs(h.live().height - midway)).toBeLessThan(40);
    h.step();
    expect(h.live()).toEqual(base);
  });

  test('a direction switch while grown returns the old way before growing the new', async () => {
    const h = harness(base);
    h.grower.want(80, 'up', true);
    await h.settle();
    h.step();
    h.grower.want(80, 'down', true);
    await h.settle();
    h.step();
    const back = h.boxes.findIndex((box) => box.y === base.y && box.height === base.height);
    expect(back).toBeGreaterThan(0);
    for (const box of h.boxes.slice(0, back)) expect(box.y + box.height).toBe(700);
    for (const box of h.boxes.slice(back)) expect(box.y).toBe(500);
    expect(h.live()).toEqual({ x: 10, y: 500, width: 600, height: 280 });
  });

  // Every sentence that fits asks for zero. That must cost nothing, not a
  // window read per clipboard change.
  test('asking for the size it already has touches nothing', async () => {
    const h = harness(base);
    h.grower.want(0, 'up', true);
    await h.settle();
    expect(h.readBox).not.toHaveBeenCalled();
    expect(h.boxes).toEqual([]);
  });

  test('pins the base only while a change is in flight', async () => {
    const h = harness(base);
    expect(h.grower.base()).toBeNull();
    h.grower.want(80, 'up', true);
    await h.settle();
    h.step(2);
    expect(h.grower.base()).toEqual(base);
    h.step();
    expect(h.grower.base()).toBeNull();
  });

  // The webview lays out at a new size a frame or more after the window takes
  // it. A sentence measured in that gap needs the base, so the base outlives
  // the change by a moment, snap or not.
  test('holds the base a moment after a snap, then lets go', async () => {
    const h = harness(base);
    h.grower.want(80, 'down', false);
    await h.settle();
    expect(h.grower.base()).toEqual(base);
    h.step();
    expect(h.grower.base()).toBeNull();
    expect(h.boxes).toHaveLength(1);
  });

  // A frame stamped a hair before the animation's start must not run the
  // easing backwards and dip the window below the user's own size.
  test('never goes below the base on an early first frame', async () => {
    let frames: Array<(now: number) => void> = [];
    const boxes: Box[] = [];
    const grower = createGrower({
      readBox: () => Promise.resolve(base),
      setBox: (box) => boxes.push(box),
      frame: (cb) => frames.push(cb),
      now: () => 100,
    });
    grower.want(80, 'down', true);
    for (let i = 0; i < 3; i += 1) await Promise.resolve();
    const due = frames;
    frames = [];
    for (const cb of due) cb(97);
    expect(boxes[0].height).toBeGreaterThanOrEqual(base.height);
  });

  // The window the user dragged elsewhere while grown: the next change starts
  // from where it is now, not where it was.
  test('reads the live box again for each change, so a drag between them sticks', async () => {
    const h = harness(base);
    h.grower.want(80, 'up', false);
    await h.settle();
    h.step();
    h.drag({ x: 300, y: 100, width: 500, height: 280 });
    h.grower.want(0, 'up', false);
    await h.settle();
    expect(h.live()).toEqual({ x: 300, y: 180, width: 500, height: 200 });
  });

  test('baseOf takes the applied growth back off a live box', async () => {
    const h = harness(base);
    h.grower.want(80, 'up', false);
    await h.settle();
    expect(h.grower.baseOf(h.live())).toEqual(base);
  });

  // A failed window read must not wedge the grower: the next sentence tries again.
  test('recovers from a failed live-box read', async () => {
    const h = harness(base);
    h.readBox.mockImplementationOnce(() => Promise.reject(new Error('gone')));
    h.grower.want(80, 'down', false);
    await h.settle();
    expect(h.boxes).toEqual([]);
    h.grower.want(80, 'down', false);
    await h.settle();
    expect(h.live().height).toBe(280);
  });
});
