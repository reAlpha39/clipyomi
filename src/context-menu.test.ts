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

    expect(shown()).toEqual({
      items: [
        { kind: 'radio', id: 'ctx-0', label: 'Header', checked: false },
        { kind: 'radio', id: 'ctx-1', label: 'Content', checked: true },
        { kind: 'separator' },
        { kind: 'action', id: 'ctx-3', label: 'Quit' },
      ],
      x: 40,
      y: 50,
    });
  });

  test('a pick runs the matching entry once', () => {
    openContextMenu(entries, 0, 0);

    pick('ctx-3');
    pick('ctx-3');

    expect(selected).toEqual(['quit']);
  });

  test('a pick for an id the open menu never offered runs nothing', () => {
    openContextMenu(entries, 0, 0);

    pick('ctx-9');

    expect(selected).toEqual([]);
  });

  // Opening again hands the ids to the new rows: a late pick from a menu the
  // OS already replaced must not fire the old row that shared its id.
  test('opening again retargets the ids to the new rows', () => {
    openContextMenu(entries, 0, 0);
    openContextMenu([{ kind: 'action', label: 'Only', onSelect: () => selected.push('only') }], 0, 0);

    pick('ctx-0');

    expect(selected).toEqual(['only']);
  });

  test('closing discards the pending rows', () => {
    openContextMenu(entries, 0, 0);
    closeContextMenu();

    pick('ctx-3');

    expect(selected).toEqual([]);
  });
});
