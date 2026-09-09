// SPDX-License-Identifier: MIT OR Apache-2.0

/** Distance between the word and the tooltip, in px: the original tooltip kept two. */
const GAP = 2;
/** Closest the tooltip may sit to a work-area edge, in px. */
export const MARGIN = 8;

/** A rectangle in screen coordinates. */
export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Point {
  x: number;
  y: number;
}

/**
 * Chooses where to put the tooltip window relative to the word being
 * hovered, given the monitor's usable work area.
 *
 * Below the word is tried first, then above, and only if neither side has
 * room does the tooltip get pinned to the bottom of the work area and
 * nudged sideways instead. Below wins the tie because a tooltip that spills
 * off the bottom of the screen still overlaps mostly empty desktop, while
 * one that spills upward covers the line of text the reader is looking at
 * — the same choice the original tooltip made.
 *
 * The horizontal clamp is applied as an upper bound first and a lower
 * bound second, so the lower bound always wins when the two disagree. On
 * a work area narrower than the tooltip, clamping to "at most the right
 * margin" first would leave a value below the left margin; running the
 * lower-bound clamp afterwards corrects that, pulling the tooltip back
 * onto screen instead of letting it hang off the right edge.
 */
export function placePopover(
  chip: Rect,
  size: { width: number; height: number },
  work: Rect,
): { left: number; top: number } {
  let left = chip.left;
  let top: number;

  const below = chip.bottom + GAP;
  const above = chip.top - GAP - size.height;

  if (below + size.height <= work.bottom) {
    top = below;
  } else if (above >= work.top) {
    top = above;
  } else {
    top = work.bottom - size.height;

    if (chip.right + GAP + size.width <= work.right) {
      left = chip.right + GAP;
    } else if (chip.left - GAP - size.width >= work.left) {
      left = chip.left - GAP - size.width;
    }
  }

  left = Math.min(left, work.right - size.width - MARGIN);
  left = Math.max(left, work.left + MARGIN);

  top = Math.max(top, work.top + MARGIN);

  return { left, top };
}

/** Whether POINT lies within RECT, edges included. Both must be in the same unit. */
export function contains(rect: Rect, point: Point): boolean {
  return (
    point.x >= rect.left && point.x <= rect.right && point.y >= rect.top && point.y <= rect.bottom
  );
}

/** Centre of RECT, in whatever unit RECT is expressed in. */
export function centreOf(rect: Rect): Point {
  return { x: (rect.left + rect.right) / 2, y: (rect.top + rect.bottom) / 2 };
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/**
 * Whether a cursor that has left the word should keep the tooltip open.
 *
 * The original tooltip's rule: compare the cursor's distance to the tooltip's
 * centre against its distance from the previous sample. Closer means
 * the user is heading for the tooltip. Chosen over a grace period because it
 * holds no timer for every dismissal path to remember to clear, and because it
 * distinguishes moving *toward* the tooltip from merely moving slowly.
 */
export function shouldKeep(prev: Point, next: Point, centre: Point): boolean {
  return distance(next, centre) < distance(prev, centre);
}
