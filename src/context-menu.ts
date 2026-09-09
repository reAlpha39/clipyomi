// SPDX-License-Identifier: MIT OR Apache-2.0

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

/**
 * One row of the menu. Built by the caller, so this module owns presentation
 * and keyboard behaviour and knows nothing about chrome modes or commands.
 */
export type MenuEntry =
  | { kind: 'radio'; label: string; checked: boolean; onSelect: () => void }
  | { kind: 'action'; label: string; onSelect: () => void }
  | { kind: 'separator' };

/**
 * The open menu, or `null`. Looked up from the DOM rather than kept in a
 * module-level variable: in production this module is loaded once, so the
 * two are equivalent, but Vitest's `vi.resetModules()` + re-import pattern
 * (see `main.test.ts`) loads several independent copies of this module over
 * a test file's life, each with its own private state. A DOM query is the
 * one thing all those copies actually share, so it is the only way a later
 * copy's `openContextMenu` reliably replaces an earlier copy's menu instead
 * of leaving it stacked underneath.
 */
function current(): HTMLElement | null {
  return document.querySelector<HTMLElement>('.context-menu');
}

/**
 * What had focus just before the menu opened, restored when it closes so a
 * keyboard user pressing Escape (or selecting an entry) does not land on
 * `<body>`. Module-level for the same reason `current()` is not: within one
 * loaded copy of this module, open and close always pair up.
 */
let previouslyFocused: HTMLElement | null = null;

export function closeContextMenu(): void {
  current()?.remove();
  const toRestore = previouslyFocused;
  previouslyFocused = null;
  // Guards the element having been removed from the document (or never
  // having been focusable in the first place — a no-op `focus()` call).
  if (toRestore !== null && document.contains(toRestore)) toRestore.focus();
}

function rowFor(entry: MenuEntry): HTMLElement {
  if (entry.kind === 'separator') {
    // Not a wrapper around a separate separator element — this `<li>` IS the
    // separator, so it keeps its own ARIA role rather than `role="none"`.
    const separator = document.createElement('li');
    separator.className = 'context-menu-separator';
    separator.setAttribute('role', 'separator');
    return separator;
  }
  // `role="none"` strips the implicit `listitem` role so the button below is
  // an owned child of the `<ul role="menu">` as far as accessibility APIs are
  // concerned, rather than a `listitem` sitting between menu and menuitem.
  const row = document.createElement('li');
  row.setAttribute('role', 'none');
  const button = document.createElement('button');
  button.type = 'button';
  // The radio marker is a CSS `::before`, so the label stays the button's only
  // text and `textContent` reads exactly what the caller passed.
  button.textContent = entry.label;
  if (entry.kind === 'radio') {
    button.setAttribute('role', 'menuitemradio');
    button.setAttribute('aria-checked', String(entry.checked));
  } else {
    button.setAttribute('role', 'menuitem');
  }
  button.addEventListener('click', () => {
    // Closed first: an entry that opens another window would otherwise leave
    // the menu standing behind it.
    closeContextMenu();
    entry.onSelect();
  });
  row.append(button);
  return row;
}

/**
 * Open the menu at the cursor, replacing any menu already open.
 *
 * Inserted hidden and positioned before it is shown: `offsetWidth` needs the
 * node in the document, and `clampMenu` needs the measurement. In happy-dom
 * those measurements are zero, which places the menu at the cursor — harmless,
 * and the reason `clampMenu` is unit-tested separately.
 */
export function openContextMenu(entries: MenuEntry[], x: number, y: number): void {
  closeContextMenu();
  // Recorded after the close above, so replacing an already-open menu keeps
  // the original trigger rather than the outgoing menu's own focused item.
  previouslyFocused = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const menu = document.createElement('ul');
  menu.className = 'context-menu';
  menu.setAttribute('role', 'menu');
  for (const entry of entries) menu.append(rowFor(entry));

  menu.addEventListener('keydown', (event) => {
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button')];
    if (buttons.length === 0) return;
    // Enter and Space need no arm: these are real <button>s.
    if (event.key === 'Escape') {
      event.preventDefault();
      closeContextMenu();
      return;
    }
    const step = event.key === 'ArrowDown' ? 1 : event.key === 'ArrowUp' ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    // `at` is -1 when focus is elsewhere, which lands on the first item going
    // down and the last going up — both the right answer.
    buttons[(at + step + buttons.length) % buttons.length]?.focus();
  });

  menu.style.visibility = 'hidden';
  document.body.append(menu);
  const { left, top } = clampMenu(
    x,
    y,
    { width: menu.offsetWidth, height: menu.offsetHeight },
    { width: window.innerWidth, height: window.innerHeight },
  );
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
  menu.style.visibility = '';
  menu.querySelector<HTMLButtonElement>('button')?.focus();
}

// Registered once, at module scope, rather than per open: the `pointerdown`
// that opens the menu on a right-click fires BEFORE the `contextmenu` event, so
// at that moment no menu is open yet and this is a no-op — a per-open
// listener would have to work around its own opening click.
document.addEventListener('pointerdown', (event) => {
  const menu = current();
  if (menu !== null && !menu.contains(event.target as Node)) closeContextMenu();
});
// The window losing focus means the click went to another window or to OS
// chrome, neither of which produces a `pointerdown` here.
window.addEventListener('blur', closeContextMenu);
