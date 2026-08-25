/** Closest the menu may sit to a viewport edge, in px. */
export const MENU_MARGIN = 4;

/**
 * Where to put the right-click menu, as a pure function of the cursor, the
 * menu's size and the viewport.
 *
 * Separated from the DOM for the reason `placePopover` is (see
 * `render/popover.ts`): happy-dom returns zeros from
 * `getBoundingClientRect()` and from `offsetWidth`, so geometry asserted
 * through a DOM in Vitest would pass regardless of what this computed.
 *
 * Flips rather than slides on an overflow: a menu that slid back along the
 * axis would sit under the cursor and swallow the click that opened it. The
 * margin clamp is applied last and wins, because in a small window the flip
 * can itself overshoot the near edge — and this window's configured minimum is
 * 160x80.
 */
export function clampMenu(
  x: number,
  y: number,
  size: { width: number; height: number },
  viewport: { width: number; height: number },
): { left: number; top: number } {
  let left = x;
  let top = y;
  if (left + size.width + MENU_MARGIN > viewport.width) left = x - size.width;
  if (top + size.height + MENU_MARGIN > viewport.height) top = y - size.height;
  left = Math.max(MENU_MARGIN, Math.min(left, viewport.width - size.width - MENU_MARGIN));
  top = Math.max(MENU_MARGIN, Math.min(top, viewport.height - size.height - MENU_MARGIN));
  return { left, top };
}
