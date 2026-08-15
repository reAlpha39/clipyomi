/** Distance between the word and the tooltip, in px. ta-old's `rAvoid.top-2`. */
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
 * Where to put the tooltip window, as a pure function of three rectangles.
 *
 * Separated from the DOM because happy-dom returns zeros from
 * `getBoundingClientRect()` — geometry asserted through a DOM in Vitest would
 * pass regardless of what this computed. That reasoning survives 2G unchanged;
 * what changed is that the third argument is the monitor's work area, so the
 * tooltip clamps above the dock rather than to the app's own window.
 */
export function placePopover(
  chip: Rect,
  size: { width: number; height: number },
  work: Rect,
): { left: number; top: number } {
  let top: number;
  let left = chip.left;

  const below = chip.bottom + GAP;
  const above = chip.top - GAP - size.height;

  if (below + size.height <= work.bottom) {
    top = below;
  } else if (above >= work.top) {
    top = above;
  } else {
    // Fits on neither side: pin to the bottom of the work area and step
    // sideways, right first (MyToolTip.cpp:518-523). This is the case a small
    // window hits most — a tall entry on a word low on the screen.
    top = work.bottom - size.height;
    if (chip.right + GAP + size.width <= work.right) left = chip.right + GAP;
    else if (chip.left - GAP - size.width >= work.left) left = chip.left - GAP - size.width;
  }

  // Applied last, and the left clamp wins: for a tooltip wider than the work
  // area the right-edge limit falls below the left margin, and `Math.max` pins
  // it to the margin instead of letting that push it off-screen.
  left = Math.max(work.left + MARGIN, Math.min(left, work.right - size.width - MARGIN));
  // The vertical pin can undershoot on a work area shorter than the tooltip;
  // the same argument applies.
  top = Math.max(work.top + MARGIN, top);

  return { left, top };
}

function distance(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/**
 * Whether a cursor that has left the word should keep the tooltip open.
 *
 * ta-old's rule (`MyToolTip.cpp:354`): compare the cursor's distance to the
 * tooltip's centre against its distance from the previous sample. Closer means
 * the user is heading for the tooltip. Chosen over a grace period because it
 * holds no timer for every dismissal path to remember to clear, and because it
 * distinguishes moving *toward* the tooltip from merely moving slowly.
 */
export function shouldKeep(prev: Point, next: Point, centre: Point): boolean {
  return distance(next, centre) < distance(prev, centre);
}
