// SPDX-License-Identifier: MIT OR Apache-2.0

import { afterEach, beforeEach, describe, expect, test } from 'vitest';
import { MENU_MARGIN, clampMenu, closeContextMenu, openContextMenu, type MenuEntry } from './context-menu';

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

describe('openContextMenu', () => {
  const selected: string[] = [];

  function entries(): MenuEntry[] {
    return [
      { kind: 'radio', label: 'Native frame', checked: true, onSelect: () => selected.push('native') },
      { kind: 'radio', label: 'Content only', checked: false, onSelect: () => selected.push('content') },
      { kind: 'separator' },
      { kind: 'action', label: 'Quit', onSelect: () => selected.push('quit') },
    ];
  }

  beforeEach(() => {
    selected.length = 0;
    document.body.innerHTML = '';
  });

  afterEach(closeContextMenu);

  const menu = () => document.querySelector<HTMLElement>('.context-menu');

  test('renders one button per entry, with the separator between them', () => {
    openContextMenu(entries(), 10, 10);
    const labels = [...menu()!.querySelectorAll('button')].map((b) => b.textContent);
    expect(labels).toEqual(['Native frame', 'Content only', 'Quit']);
    expect(menu()!.querySelectorAll('[role="separator"]')).toHaveLength(1);
  });

  test('marks the checked radio and leaves the actions unmarked', () => {
    openContextMenu(entries(), 10, 10);
    const buttons = [...menu()!.querySelectorAll('button')];
    expect(buttons[0].getAttribute('aria-checked')).toBe('true');
    expect(buttons[1].getAttribute('aria-checked')).toBe('false');
    expect(buttons[2].getAttribute('role')).toBe('menuitem');
    expect(buttons[2].hasAttribute('aria-checked')).toBe(false);
  });

  test('a click runs the entry and closes the menu', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.querySelectorAll<HTMLButtonElement>('button')[1].click();
    expect(selected).toEqual(['content']);
    expect(menu()).toBeNull();
  });

  test('opening again replaces the open menu rather than stacking one', () => {
    openContextMenu(entries(), 10, 10);
    openContextMenu(entries(), 20, 20);
    expect(document.querySelectorAll('.context-menu')).toHaveLength(1);
  });

  test('Escape closes it', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(menu()).toBeNull();
  });

  test('the arrows wrap around the items', () => {
    openContextMenu(entries(), 10, 10);
    const buttons = [...menu()!.querySelectorAll<HTMLButtonElement>('button')];
    expect(document.activeElement).toBe(buttons[0]);

    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true }));
    expect(document.activeElement).toBe(buttons[2]);
    menu()!.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
    expect(document.activeElement).toBe(buttons[0]);
  });

  test('a pointerdown outside closes it, one inside does not', () => {
    openContextMenu(entries(), 10, 10);
    menu()!.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(menu()).not.toBeNull();

    document.body.dispatchEvent(new Event('pointerdown', { bubbles: true }));
    expect(menu()).toBeNull();
  });

  test('losing window focus closes it', () => {
    openContextMenu(entries(), 10, 10);
    window.dispatchEvent(new Event('blur'));
    expect(menu()).toBeNull();
  });

  // A keyboard user pressing Escape must not land on <body>: the element
  // that had focus before the menu opened gets it back. Guarded so a trigger
  // removed from the document meanwhile (or never focusable) cannot throw.
  test('closing restores focus to whatever opened the menu', () => {
    const trigger = document.createElement('button');
    trigger.textContent = 'open menu';
    document.body.append(trigger);
    trigger.focus();
    expect(document.activeElement).toBe(trigger);

    openContextMenu(entries(), 10, 10);
    expect(document.activeElement).not.toBe(trigger);

    closeContextMenu();
    expect(document.activeElement).toBe(trigger);

    // A second trigger, removed from the document before the menu closes:
    // must not throw, and must not leave a detached element active.
    const goneTrigger = document.createElement('button');
    document.body.append(goneTrigger);
    goneTrigger.focus();
    openContextMenu(entries(), 10, 10);
    goneTrigger.remove();
    expect(() => closeContextMenu()).not.toThrow();
    expect(document.contains(document.activeElement)).toBe(true);
  });
});
