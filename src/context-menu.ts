// SPDX-License-Identifier: MIT OR Apache-2.0

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * The right-click menu, drawn by the OS.
 *
 * It used to be HTML in this webview, which capped it at the window's own
 * size: a short window cut the menu off, and no clamp can fix that from the
 * inside. The rows and what they do still live here; the backend's
 * `show_context_menu` builds the native menu from them and pops it, and the
 * picked row's id comes back on `context-menu-pick`. Dismissal, keyboard
 * navigation and positioning are the OS's now.
 */

/**
 * One row of the menu. Built by the caller, so this module owns the wire
 * shape and the dispatch and knows nothing about chrome modes or commands.
 */
export type MenuEntry =
  | { kind: 'radio'; label: string; checked: boolean; onSelect: () => void }
  | { kind: 'action'; label: string; onSelect: () => void }
  | { kind: 'separator' };

/** What the backend's `MenuSpec` deserializes. */
type MenuSpec =
  | { kind: 'radio'; id: string; label: string; checked: boolean }
  | { kind: 'action'; id: string; label: string }
  | { kind: 'separator' };

const PICK_EVENT = 'context-menu-pick';

/**
 * The rows of the menu currently up, by id, or `null` when none is. Ids are
 * positional and reused across opens, so this is replaced wholesale on each
 * open: a late pick from a menu the OS already replaced must not fire the old
 * row that shared its id.
 */
let pending: Map<string, () => void> | null = null;

export function openContextMenu(entries: MenuEntry[], x: number, y: number): void {
  const handlers = new Map<string, () => void>();
  const items: MenuSpec[] = entries.map((entry, index) => {
    if (entry.kind === 'separator') return { kind: 'separator' };
    const id = `ctx-${index}`;
    handlers.set(id, entry.onSelect);
    return entry.kind === 'radio'
      ? { kind: 'radio', id, label: entry.label, checked: entry.checked }
      : { kind: 'action', id, label: entry.label };
  });
  pending = handlers;
  // Fire-and-forget: a menu that failed to pop leaves the user exactly where
  // a dismissed one would, and the next right-click tries again.
  void invoke('show_context_menu', { items, x, y }).catch(() => {});
}

/**
 * Forget the rows of the open menu. The OS menu itself cannot be closed from
 * here; a pick arriving afterwards runs nothing.
 */
export function closeContextMenu(): void {
  pending = null;
}

void listen<string>(PICK_EVENT, (e) => {
  const handler = pending?.get(e.payload);
  pending = null;
  handler?.();
});
