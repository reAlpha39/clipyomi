// SPDX-License-Identifier: MIT OR Apache-2.0

/**
 * Fitting the main window's height to the sentence it shows.
 *
 * The window hugs the text: as tall as the sentence plus its padding, from
 * one line up to a number of lines, past which the pane scrolls. Width and
 * position stay the user's; `up` holds the bottom edge (a window lined up
 * over a VN textbox), `down` holds the top.
 *
 * The user's own box — whatever they dragged the window to — is the base the
 * fit is an offset from, above or below it. It is what the geometry save
 * persists and what turning the fit off returns to.
 *
 * The arithmetic is pure and the controller takes its window, clock and frame
 * scheduler as parameters, so all of it runs under the test suite without a
 * webview — `main.ts` only measures the page and wires the real ones in.
 */

import type { GrowDirection } from './types';

/** The edge the window grows from, once growing at all. */
export type GrowSide = Exclude<GrowDirection, 'off'>;

/** A window box: outer position and inner size, in physical px. */
export interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/**
 * Bounds for the line cap. The settings file is hand-editable, so these are
 * the guard, not just the input's `min`/`max`.
 */
export const GROW_LINE_LIMITS = { min: 1, max: 10 } as const;

/** The most lines the window shows before the pane scrolls. */
export const DEFAULT_GROW_LINES = 3;

/**
 * The shortest the window may be fitted to, in logical px: `tauri.conf.json`'s
 * `minHeight`, mirrored by `layout::geometry::MIN_LOGICAL_HEIGHT`. The OS
 * refuses anything shorter, and a box it refused would leave the grower's
 * idea of the window, and an `up` fit's bottom edge, off by the difference.
 */
export const MIN_WINDOW_HEIGHT = 40;

/** How long one grow or shrink takes when animated. */
export const GROW_DURATION_MS = 150;

/**
 * How long the base stays pinned after a change lands. The webview lays out
 * at a new size a frame or three after the window takes it, and a sentence
 * measured in that gap has to be measured against the base, not against a
 * growth the page does not show yet.
 */
const SETTLE_MS = 100;

/** VALUE as a direction, or `off` for anything unrecognised. */
export function clampGrowDirection(value: unknown): GrowDirection {
  return value === 'up' || value === 'down' ? value : 'off';
}

/** VALUE pinned into range, or the default if it is not a finite number. */
export function clampGrowLines(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return DEFAULT_GROW_LINES;
  return Math.min(GROW_LINE_LIMITS.max, Math.max(GROW_LINE_LIMITS.min, Math.round(value)));
}

/**
 * The sentence's average line height: HEIGHT over the lines BOTTOMS steps
 * through, or 0 with nothing rendered.
 *
 * BOTTOMS are the sentence's elements' bottom edges in document order. Within
 * a line they differ by a chip's padding; a new line moves them down by a
 * whole line, so only a step past THRESHOLD counts. Measured rather than read
 * off `line-height`, because ruby makes a line taller than that says, and a
 * cap of two lines has to show two lines of what is actually on screen.
 */
export function linePitch(bottoms: readonly number[], height: number, threshold: number): number {
  if (bottoms.length === 0) return 0;
  let lines = 1;
  let lineBottom = bottoms[0];
  for (const bottom of bottoms) {
    if (bottom > lineBottom + threshold) {
      lines += 1;
      lineBottom = bottom;
    }
  }
  return height / lines;
}

/**
 * How much of TEXT's height the window shows: all of it, up to the text of
 * LINES lines of PITCH. Without a measured line, all of it.
 *
 * TEXT runs to the bottom of the last line's text, not its line box, so it
 * is shorter than its lines' boxes by TRAILING, the leading under a line's
 * text. The cap ends the same way, at the text of its last line: LINES whole
 * boxes would show that line's leading, and the next line's ruby, as a gap
 * no padding setting could close. No floor of one line, for the same reason.
 */
export function fitText(text: number, pitch: number, lines: number, trailing = 0): number {
  if (pitch <= 0) return Math.ceil(text);
  return Math.ceil(Math.min(text, lines * pitch - trailing));
}

/** BASE grown by EXTRA from SIDE. */
export function grownBox(base: Box, extra: number, side: GrowSide): Box {
  return {
    x: base.x,
    y: side === 'up' ? base.y - extra : base.y,
    width: base.width,
    height: base.height + extra,
  };
}

/** The base a LIVE box grown by EXTRA from SIDE came from. */
export function baseBox(live: Box, extra: number, side: GrowSide): Box {
  return grownBox(live, -extra, side);
}

/** Fast out, slow in: the window arrives gently. */
function easeOut(t: number): number {
  return 1 - (1 - t) ** 3;
}

export interface GrowDeps {
  /** The window's live box. */
  readBox(): Promise<Box>;
  /** Move and resize the window in one step. */
  setBox(box: Box): void;
  /** Run CB on the next frame, `requestAnimationFrame`-style. */
  frame(cb: (now: number) => void): void;
  now(): number;
}

export interface Grower {
  /** Head for TARGET px off the base from SIDE; negative is shorter than it. */
  want(target: number, side: GrowSide, animate: boolean): void;
  /** The offset off the base last applied, in px, signed. */
  applied(): number;
  /** The side that growth is on. */
  side(): GrowSide;
  /** The base, pinned while a change is in flight and briefly after; `null` when idle. */
  base(): Box | null;
  /** The base LIVE came from, given the growth applied now. */
  baseOf(live: Box): Box;
}

interface Request {
  target: number;
  side: GrowSide;
  animate: boolean;
}

export function createGrower(deps: GrowDeps): Grower {
  let applied = 0;
  let side: GrowSide = 'down';
  // Read from the live window at the start of each change and dropped a
  // moment after its end, so a drag between changes is picked up by the next.
  let base: Box | null = null;
  let reading = false;
  let request: Request | null = null;
  let anim: { from: number; to: number; start: number } | null = null;
  let looping = false;
  let releaseAt = 0;

  function apply(extra: number): void {
    applied = extra;
    if (base !== null) deps.setBox(grownBox(base, extra, side));
  }

  function loop(): void {
    if (looping) return;
    looping = true;
    deps.frame(tick);
  }

  function tick(now: number): void {
    looping = false;
    if (anim === null) {
      // Landed: hold the base until the page has caught up, then let go.
      if (base === null || request !== null) return;
      if (now >= releaseAt) base = null;
      else loop();
      return;
    }
    // Clamped below too: a frame stamped before `start` would run the easing
    // backwards and dip the window under the base.
    const t = Math.min(1, Math.max(0, (now - anim.start) / GROW_DURATION_MS));
    apply(Math.round(anim.from + (anim.to - anim.from) * easeOut(t)));
    if (t < 1) {
      loop();
      return;
    }
    anim = null;
    settle();
  }

  function steer(): void {
    if (request === null || base === null) return;
    if (applied === 0) side = request.side;
    // A side switch while grown goes back to the base the old way first;
    // `settle` then sets off the new way from there.
    const to = request.side !== side ? 0 : request.target;
    if (!request.animate) {
      anim = null;
      apply(to);
      settle();
      return;
    }
    if (anim !== null && anim.to === to) return;
    if (anim === null && to === applied) {
      settle();
      return;
    }
    // From wherever the window is now, so a new sentence mid-animation turns
    // it around rather than making it jump.
    anim = { from: applied, to, start: deps.now() };
    loop();
  }

  function settle(): void {
    if (request !== null && (request.side !== side || request.target !== applied)) {
      steer();
      return;
    }
    request = null;
    releaseAt = deps.now() + SETTLE_MS;
    loop();
  }

  return {
    want(target, nextSide, animate) {
      // Signed: a short sentence fits the window below the base.
      const wanted = Math.round(target);
      // Every sentence that fits asks for zero; while idle at the size asked
      // for, that costs nothing — not even a window read.
      if (base === null && !reading && wanted === applied && (applied === 0 || nextSide === side)) {
        return;
      }
      request = { target: wanted, side: nextSide, animate };
      if (base !== null) {
        steer();
        return;
      }
      if (reading) return;
      reading = true;
      deps.readBox().then(
        (live) => {
          reading = false;
          base = baseBox(live, applied, side);
          steer();
        },
        () => {
          // Nothing moved. The next sentence asks again.
          reading = false;
          request = null;
        },
      );
    },
    applied: () => applied,
    side: () => side,
    base: () => base,
    baseOf: (live) => baseBox(live, applied, side),
  };
}
