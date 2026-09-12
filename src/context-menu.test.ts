// SPDX-License-Identifier: MIT OR Apache-2.0

import { beforeEach, describe, expect, test, vi } from 'vitest';
import { closeContextMenu, openContextMenu, type MenuEntry } from './context-menu';

// The menu is drawn by the OS now. What this module owns is the wire shape it
// sends and the dispatch of the id that comes back, so both IPC boundaries
// are stubbed the way `main.test.ts` stubs them.
const { listeners, invoke } = vi.hoisted(() => ({
  listeners: new Map<string, (e: { payload: unknown }) => void>(),
  invoke: vi.fn((..._args: unknown[]) => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return Promise.resolve(() => listeners.delete(event));
  },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

function pick(id: string): void {
  const handler = listeners.get('context-menu-pick');
  if (handler === undefined) throw new Error('nothing listening for context-menu-pick');
  handler({ payload: id });
}

/** The payload of the last `show_context_menu` call. */
function shown(): { items: unknown[]; x: number; y: number } {
  const call = [...invoke.mock.calls]
    .reverse()
    .find(([cmd]: unknown[]) => cmd === 'show_context_menu');
  if (call === undefined) throw new Error('show_context_menu was not invoked');
  return call[1] as { items: unknown[]; x: number; y: number };
}

describe('the right-click menu', () => {
  const selected: string[] = [];
  const entries: MenuEntry[] = [
    { kind: 'radio', label: 'Header', checked: false, onSelect: () => selected.push('header') },
    { kind: 'radio', label: 'Content', checked: true, onSelect: () => selected.push('content') },
    { kind: 'separator' },
    { kind: 'action', label: 'Quit', onSelect: () => selected.push('quit') },
  ];

  beforeEach(() => {
    selected.length = 0;
    invoke.mockClear();
    closeContextMenu();
  });

  // Ids are this module's to choose and the backend's to echo; the labels and
  // check states are what the OS draws.
  test('asks the backend to pop the rows at the cursor', () => {
    openContextMenu(entries, 40, 50);

    const { items, x, y } = shown();
    expect({ x, y }).toEqual({ x: 40, y: 50 });
    expect(items.map((item) => ({ ...(item as object), id: undefined }))).toEqual([
      { kind: 'radio', label: 'Header', checked: false, id: undefined },
      { kind: 'radio', label: 'Content', checked: true, id: undefined },
      { kind: 'separator', id: undefined },
      { kind: 'action', label: 'Quit', id: undefined },
    ]);
    const ids = items.map((item) => (item as { id?: string }).id);
    expect(ids[2]).toBeUndefined();
    expect(new Set([ids[0], ids[1], ids[3]]).size).toBe(3);
  });

  test('a pick runs the matching entry once', () => {
    openContextMenu(entries, 0, 0);
    const id = (shown().items[3] as { id: string }).id;

    pick(id);
    pick(id);

    expect(selected).toEqual(['quit']);
  });

  test('a pick for an id the open menu never offered runs nothing', () => {
    openContextMenu(entries, 0, 0);

    pick('ctx-never');

    expect(selected).toEqual([]);
  });

  // A pick travels OS -> backend -> webview while the user may already have
  // right-clicked again. Ids carry the open they belong to, so a pick from a
  // menu the OS already replaced runs nothing rather than the new menu's row
  // at the same position.
  test('a pick from a menu already replaced runs nothing', () => {
    openContextMenu(entries, 0, 0);
    const stale = (shown().items[3] as { id: string }).id;
    openContextMenu([{ kind: 'action', label: 'Only', onSelect: () => selected.push('only') }], 0, 0);
    const fresh = (shown().items[0] as { id: string }).id;
    expect(fresh).not.toBe(stale);

    pick(stale);
    expect(selected).toEqual([]);

    pick(fresh);
    expect(selected).toEqual(['only']);
  });

  test('a menu the backend failed to show forgets its rows', async () => {
    invoke.mockRejectedValueOnce(new Error('no window'));
    openContextMenu(entries, 0, 0);
    const id = (shown().items[3] as { id: string }).id;
    await Promise.resolve();
    await Promise.resolve();

    pick(id);

    expect(selected).toEqual([]);
  });

  test('closing discards the pending rows', () => {
    openContextMenu(entries, 0, 0);
    closeContextMenu();

    pick('ctx-3');

    expect(selected).toEqual([]);
  });
});
