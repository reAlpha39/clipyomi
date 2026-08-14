import type { Entry } from '../types';
import { renderEntry } from './definitions';

/** Distance between the chip and the popover, in px. */
const GAP = 6;
/** Closest the popover may sit to a viewport edge, in px. */
const MARGIN = 8;

/**
 * Where to put the popover, as a pure function of three rectangles.
 *
 * Separated from the DOM because happy-dom returns zeros from
 * `getBoundingClientRect()`: geometry asserted through the DOM in a unit test
 * would pass regardless of what this computed.
 */
export function placePopover(
  chip: DOMRect,
  popover: { width: number; height: number },
  viewport: { width: number },
): { left: number; top: number } {
  // Above the sentence is the input row, which nobody is reading; below it are
  // the definition rows, which they might be. So above is preferred, and below
  // is the fallback when the preferred position would clip the window top.
  const above = chip.top - popover.height - GAP;
  const top = above >= MARGIN ? above : chip.bottom + GAP;

  // The left clamp is applied last on purpose: for a popover wider than the
  // viewport the right-edge limit goes negative, and `Math.max` pins it to the
  // margin instead of letting that negative value push it off-screen.
  const rightmost = viewport.width - popover.width - MARGIN;
  const left = Math.max(MARGIN, Math.min(chip.left, rightmost));

  return { left, top };
}

/**
 * The one popover element, created on first use.
 *
 * Module-local rather than recreated per hover: one node means one place for
 * the open state to live, and nothing to leak when a sentence is replaced.
 */
let popover: HTMLElement | null = null;

function element(): HTMLElement {
  if (popover === null) {
    popover = document.createElement('div');
    popover.className = 'entry-popover';
    // The same entry is already in the definitions pane, so announcing it here
    // too would read it twice. The chip keeps its own accessible name; this
    // surface is a visual convenience only.
    popover.setAttribute('aria-hidden', 'true');
    // `#app`, not `.panes`: `.panes` is `overflow-y: auto`, so a popover inside
    // it would be clipped at the pane edge and would scroll away from the word
    // it describes. Created lazily, which is also why `main.ts` assigning
    // `app.innerHTML` at import time cannot wipe it.
    document.querySelector<HTMLElement>('#app')!.append(popover);
  }
  return popover;
}

/** Show ENTRY beside CHIP. Safe to call when one is already open. */
export function showEntryPopover(chip: HTMLElement, entry: Entry): void {
  const el = element();
  el.replaceChildren(renderEntry(entry));
  // Reset before measuring: a previous open leaves its inline `left` applied,
  // and for a `position: fixed` box with `right: auto; width: auto`, used
  // width is shrink-to-fit bounded by `containing-block width - left`.
  // Measuring against a stale `left` would starve `getBoundingClientRect()`
  // of the viewport width it actually has, undersizing this entry's box (and,
  // through wrapped text, its measured height too) for anything that would
  // need more room than was left over from the previous position.
  el.style.left = '0px';
  // Measured before it is placed, because the height decides which side of the
  // chip it goes on. The base style is `visibility: hidden`, which still lays
  // out — so this measures the real box at the reset position, and no frame is
  // ever painted in between: the style write and this layout happen inside
  // one task.
  const { left, top } = placePopover(chip.getBoundingClientRect(), el.getBoundingClientRect(), {
    width: window.innerWidth,
  });
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
  el.classList.add('open');
}

/** Hide it. A no-op when nothing has been shown yet. */
export function hideEntryPopover(): void {
  popover?.classList.remove('open');
}
