// SPDX-License-Identifier: MIT OR Apache-2.0

import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import {
  availableMonitors,
  cursorPosition,
  getCurrentWindow,
} from '@tauri-apps/api/window';
import { renderSentence } from './render/sentence';
import {
  MARGIN,
  centreOf,
  contains,
  placePopover,
  shouldKeep,
  type Point,
  type Rect,
} from './render/popover';
import { clampFont, fontVars } from './font-sizes';
import { clampAlpha, surfaceAlphaVar } from './surface-alpha';
import { colourVars, resolveColor } from './text-colours';
import { clampFamily, familyVars } from './text-fonts';
import { createHistory } from './history';
import { openContextMenu, type MenuEntry } from './context-menu';
import { createKanjiLookup } from './kanji-lookup';
import {
  clampGrowDirection,
  clampGrowLines,
  createGrower,
  growTarget,
  linePitch,
} from './window-grow';
import type {
  ChromeMode,
  Entry,
  FuriganaMode,
  GlossFilters,
  GrowDirection,
  KanjiInfo,
  ParseResult,
  Segment,
  Settings,
} from './types';
import './styles/global.css';

/**
 * The resize edges `startResizeDragging` accepts.
 *
 * Read off that method rather than imported: `@tauri-apps/api` 2.11.1 declares
 * `ResizeDirection` in `window.d.ts` but never exports it, so importing the name
 * fails `tsc` (TS2459) and broke `npm run build` — invisibly, because `vite dev`
 * does not typecheck. Deriving it keeps one source of truth and follows upstream
 * if the union changes.
 */
type ResizeDirection = Parameters<
  ReturnType<typeof getCurrentWindow>['startResizeDragging']
>[0];

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <div class="resize-handle resize-top" data-direction="North"></div>
  <div class="resize-handle resize-bottom" data-direction="South"></div>
  <div class="resize-handle resize-left" data-direction="West"></div>
  <div class="resize-handle resize-right" data-direction="East"></div>
  <div class="resize-handle resize-top-left" data-direction="NorthWest"></div>
  <div class="resize-handle resize-top-right" data-direction="NorthEast"></div>
  <div class="resize-handle resize-bottom-left" data-direction="SouthWest"></div>
  <div class="resize-handle resize-bottom-right" data-direction="SouthEast"></div>
  <header class="controls" data-tauri-drag-region>
    <nav class="history-controls" aria-label="Parse history">
      <button id="history-back" type="button" title="Back" aria-label="Back" disabled>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
      </button>
      <button id="history-forward" type="button" title="Forward" aria-label="Forward" disabled>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <polyline points="9 18 15 12 9 6"/>
        </svg>
      </button>
    </nav>
    <button id="settings-toggle" type="button" title="Settings" aria-label="Settings">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
        <circle cx="12" cy="12" r="3"/>
      </svg>
    </button>
    <div id="window-controls" class="window-controls">
      <button id="window-minimize" type="button" title="Minimize" aria-label="Minimize" tabindex="-1">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0,5 L10,5" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
      <button id="window-close" type="button" title="Close" aria-label="Close" tabindex="-1">
        <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0.5,0.5 L9.5,9.5 M9.5,0.5 L0.5,9.5" stroke="currentColor" stroke-width="1" />
        </svg>
      </button>
    </div>
  </header>
  <div id="parse-error"></div>
  <div class="panes"><div id="dictionary" role="status" tabindex="-1"></div><div id="output" tabindex="-1"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const panes = app.querySelector<HTMLElement>('.panes')!;
// A fixed slot rather than a node prepended into `output`: `output` is what a
// successful parse replaces wholesale (and what Task 5 replaces with two
// panes), so an error node living there either gets silently wiped on the
// next success or, on repeated failures, piles up because nothing ever
// removes the previous one. This slot always holds at most one error.
const parseError = app.querySelector<HTMLElement>('#parse-error')!;

function errorBlock(message: string): HTMLElement {
  const el = document.createElement('pre');
  el.className = 'startup-error';
  el.textContent = message;
  return el;
}

// Exported so a test can await it directly rather than racing the
// fire-and-forget call at the bottom of this module.
export async function showStartupError(): Promise<void> {
  const message = await invoke<string | null>('startup_error');
  if (message === null) return;
  // No index (the expected first run — 2E adds the download) and an
  // unopenable index both mean the worker has no index to parse with.
  // Clipboard monitoring keeps running regardless, but a copy made in this
  // state produces nothing — the worker never reaches `jparser::parse`, so
  // there is no `parse-result` and no `parse-error` either. This message
  // stays in `#output` as the standing explanation for why nothing has
  // appeared there yet.
  output.replaceChildren(errorBlock(message));
}

// A corrupt settings.json is cosmetic, not fatal: the app already fell back
// to defaults by the time this resolves, so — unlike `showStartupError`
// above — this must never touch `output`.
// Exported for the same reason `showStartupError` is.
export async function showSettingsWarning(): Promise<void> {
  const message = await invoke<string | null>('settings_warning');
  if (message === null) return;
  // `append`, not `replaceChildren`: this and `showShortcutWarning` are both
  // fire-and-forget at the bottom of this module, so whichever resolves
  // second must not wipe the other's warning out of `#parse-error`.
  parseError.append(errorBlock(message));
}

// A hotkey another application owns is the one hotkey failure that would
// otherwise be invisible: the keys would simply do nothing.
// Exported for the same reason `showStartupError` is.
export async function showShortcutWarning(): Promise<void> {
  const message = await invoke<string | null>('shortcut_warning');
  if (message === null) return;
  // `append`, not `replaceChildren`: a corrupt settings.json puts its own,
  // unrelated warning in this slot, and both are worth showing. The next
  // successful parse clears the slot, exactly as it already does for that one.
  parseError.append(errorBlock(message));
}

// Hints that are silently off are invisible otherwise: the setting reads on and
// the parse has none, which looks like the segmentation simply not improving.
// Exported for the same reason `showStartupError` is.
export async function showHintsWarning(): Promise<void> {
  const message = await invoke<string | null>('hints_warning');
  if (message === null) return;
  // `append`, not `replaceChildren`, for the reason `showSettingsWarning`
  // documents: a corrupt settings.json and a taken hotkey can already both be
  // in this slot, and none of the three may wipe the others.
  parseError.append(errorBlock(message));
}

const dictionary = app.querySelector<HTMLElement>('#dictionary')!;

// `role="status"` on the markup element (a native ARIA live region — no JS
// wiring needed) is what makes every `renderDictionary` text update below
// audible to a screen reader: without it, a user who presses Download hears
// nothing for the 15+ seconds the build takes, and nothing again if it fails.

// The backend's own phase labels. Anything not in this map is an error message
// to show verbatim, which is why this is a lookup rather than an enum — the
// failure arm carries text the user needs to read.
const PHASE_LABELS: Record<string, string> = {
  downloading: 'Downloading dictionary…',
  building: 'Building index…',
};

/**
 * Whether the `ready` now in flight belongs to a download this screen started
 * on a machine that has never had hints, and should therefore acquire them.
 *
 * **Why a fresh install needs this at all.** `mecab_hints` defaults to `false`
 * and that is not a preference — it records whether the 7.7 MB Vibrato
 * dictionary has been fetched (see `settings.rs`'s own comment on the field).
 * `load_hints` never downloads; `set_mecab_hints` is the only code path that
 * does. So without this, every fresh install runs the weaker engine forever
 * unless the user finds the toggle: on the gold corpus, hints off reads span
 * accuracy 1593 against 1698, tolerant 1786 against 1948 and 118 compound
 * windows against 152.
 *
 * **Why a flag and not `.then()` on the invoke.** `download_dictionary`
 * resolves `Ok` as soon as the work is *started* — every outcome including
 * failure arrives as a `dictionary-status` event, which is the command's
 * documented contract. Chaining off the promise would fetch hints after a
 * failed index download. The `ready` event is the only signal that the
 * dictionary actually landed, and `emit_status(.., "ready")` has exactly one
 * call site, in that success branch. (`hints-status` carries a `"ready"` of its
 * own; it is a different channel and this listener never sees it.)
 *
 * **Three things have to be true, and a click is only one of them.**
 *
 * 1. *This screen started the download.* The listener is global and outlives
 *    the screen, so an unrelated `ready` must not arm anything.
 * 2. *No hints dictionary exists yet* — [`hintsNeverAcquired`]. The download
 *    screen is **not** the first-run screen it looks like:
 *    `state::is_missing_dictionary` deliberately routes `ConjugationMismatch`
 *    and `VersionMismatch` to it as well, which are the expected states after
 *    an application upgrade. Without this condition, an established user who
 *    had switched hints off would have them switched silently back on by the
 *    rebuild they had no choice but to run.
 * 3. *The user has not engaged the toggle themselves since clicking.* An index
 *    download and build take a while, and the settings window is reachable
 *    throughout. `emit_hints_status` is called only from `set_mecab_hints`, so
 *    **any** payload on that channel — `downloading`, `unpacking`, `ready`,
 *    `off`, or an error — means they are answering the question and this screen
 *    must not answer it for them. It disarms on all five rather than reading
 *    the payload, because a started acquisition and a failed one both mean the
 *    same thing here.
 */
let acquireHintsOnReady = false;

/**
 * Whether this machine has no published Vibrato dictionary, so hints have never
 * been acquired and the question has never been put to the user.
 *
 * Resolved once, when the download screen is about to be shown, rather than in
 * the click handler — the handler is synchronous and a `ready` can in principle
 * beat an `invoke` it fired, which on a fast local rebuild is not hypothetical.
 * `false` until answered, so the unanswered state arms nothing.
 */
let hintsNeverAcquired = false;

function renderDictionary(status: string | null): void {
  // Read before any DOM mutation below: `replaceChildren` on a parent whose
  // descendant currently has focus moves focus straight to `<body>` the
  // instant that descendant is removed (confirmed against both real Chromium
  // and happy-dom while writing this fix) — which is exactly what used to
  // happen the moment a click on Download re-rendered the button out from
  // under itself. Gating the restore below on this, rather than always
  // refocusing something, is also what stops an unrelated background `ready`
  // from stealing focus from wherever the user actually is when they never
  // touched this screen.
  const hadFocus = dictionary.contains(document.activeElement);

  // `#dictionary` carries `tabindex="-1"` in the markup precisely so this has
  // somewhere to land even in states with no interactive child at all — the
  // downloading/building spinner, and the empty node `ready` leaves behind.
  function restoreFocus(): void {
    if (!hadFocus) return;
    (dictionary.querySelector('button') ?? dictionary).focus();
  }

  if (status === 'ready') {
    dictionary.replaceChildren();
    restoreFocus();
    return;
  }

  const el = document.createElement('div');
  el.className = 'dictionary';
  const phase = status === null ? undefined : PHASE_LABELS[status];

  if (phase !== undefined) {
    const label = document.createElement('p');
    label.textContent = phase;
    const spinner = document.createElement('div');
    spinner.className = 'spinner';
    // Decorative: the label above already carries the information, and under
    // `prefers-reduced-motion` this stops moving entirely.
    spinner.setAttribute('aria-hidden', 'true');
    el.replaceChildren(label, spinner);
    dictionary.replaceChildren(el);
    restoreFocus();
    return;
  }

  // Idle or failed. `null` is the first-run offer; anything else is a message
  // from the backend, which already names the archive and directory to drop in.
  const message = document.createElement('p');
  message.textContent =
    status === null
      ? // Names both downloads on purpose. This is the one screen that tells
        // the user what is about to be fetched, and since the first run also
        // acquires the hint dictionary the old "roughly twelve megabytes from
        // EDRDG" named one of the two hosts and understated the total by
        // `vibrato_source::ASSET_BYTES`, 7.7 MB — about twenty megabytes, not
        // twelve, which is 64% more than it said. That matters on a metered
        // connection and behind a proxy that allows EDRDG but not GitHub.
        'No dictionary yet. JMdict and KANJIDIC2 are a one-time download of roughly twelve megabytes from EDRDG, plus an eight-megabyte IPADIC dictionary from GitHub that improves word splitting.'
      : status;

  const button = document.createElement('button');
  button.id = 'download';
  button.type = 'button';
  button.textContent = status === null ? 'Download dictionary' : 'Retry';

  // Closure-local, not `button.disabled`: disabling a focused element blurs
  // it and drops it from the tab order, which nothing here restores (design
  // §3). This one is belt-and-braces behind an immediate re-render:
  // `renderDictionary('downloading')` replaces this button synchronously,
  // before `invoke` even starts, so the node a second click would need to
  // land on is already gone by the time anyone could click it again. That
  // same synchronous removal used to blur focus just as thoroughly as
  // `disabled` would have — `restoreFocus`, above, is what fixes that half
  // of the trade rather than swapping one way of losing focus for another.
  let pending = false;
  button.addEventListener('click', () => {
    if (pending) return;
    pending = true;
    // Armed here rather than at the `ready` that consumes it — see
    // `acquireHintsOnReady` for all three conditions. A Retry after a failed
    // attempt re-arms from `hintsNeverAcquired`, which is why the `hints-status`
    // listener has to clear *that* too: re-arming is free only while the user
    // has not answered in the meantime.
    acquireHintsOnReady = hintsNeverAcquired;
    renderDictionary('downloading');
    void invoke('download_dictionary')
      .catch((e) => renderDictionary(String(e)))
      .finally(() => {
        pending = false;
      });
  });

  el.replaceChildren(message, button);
  dictionary.replaceChildren(el);
  restoreFocus();
}

// Exported for the same reason `showStartupError` is: a test can await it
// directly rather than racing the fire-and-forget call at the bottom.
export async function showDictionaryScreen(): Promise<void> {
  if (!(await invoke<boolean>('needs_dictionary'))) return;
  // Asked before the screen exists, so the click handler can stay synchronous
  // and still know the answer — see `hintsNeverAcquired`. A failure here leaves
  // it false, which only means this download does not also acquire hints; the
  // settings toggle still does, so the degradation is the pre-change behaviour.
  hintsNeverAcquired = await invoke<boolean>('hints_available')
    .then((available) => !available)
    .catch(() => false);
  // Parsing cannot succeed until an index exists; `renderDictionary` shows the
  // download/build screen in place of `#dictionary`'s idle state until a
  // `ready` event clears it.
  renderDictionary(null);
}
// Owned by the settings window now; the main window only reads it to decide
// how `renderSentence` draws ruby.
let currentFuriganaMode: FuriganaMode = 'none';

const currentFilters: GlossFilters = {
  hide_pos: false,
  hide_xrefs: false,
  hide_usage: false,
  hide_kanji: false,
};

/** The popover's kanji block, asked once per character for the page's life. */
const lookupKanji = createKanjiLookup((chars) => invoke('kanji_info', { chars }));

// The grow-to-fit settings, owned by the settings window like the furigana
// mode. Off until `get_settings` says otherwise.
let growDirection: GrowDirection = 'off';
let growLines = clampGrowLines(undefined);
let growAnimate = true;

/** Grows and shrinks this window around the user's own box (`window-grow.ts`). */
const grower = createGrower({
  readBox: async () => {
    const win = getCurrentWindow();
    const [pos, size, minimized, maximized] = await Promise.all([
      win.outerPosition(),
      win.innerSize(),
      win.isMinimized(),
      win.isMaximized(),
    ]);
    // A minimized window reads as 0×0 at (-32000, -32000) on Windows, and a
    // maximized one is the OS's size, not the user's. Neither is a base to
    // grow from; the rejection leaves the window alone until the next sentence.
    if (minimized || maximized || size.width === 0 || size.height === 0) {
      throw new Error('no box of the user’s own to grow from');
    }
    return { x: pos.x, y: pos.y, width: size.width, height: size.height };
  },
  setBox: (box) => {
    void invoke('set_window_box', { ...box }).catch(() => {});
  },
  frame: (cb) => {
    window.requestAnimationFrame(cb);
  },
  now: () => performance.now(),
});

function applyGrowSettings(settings: Partial<Settings>): void {
  if (settings.grow_direction !== undefined) {
    growDirection = clampGrowDirection(settings.grow_direction);
  }
  if (settings.grow_lines !== undefined) growLines = clampGrowLines(settings.grow_lines);
  if (settings.grow_animate !== undefined) growAnimate = settings.grow_animate;
}

const settingsToggle = app.querySelector<HTMLButtonElement>('#settings-toggle');
const windowMinimize = app.querySelector<HTMLButtonElement>('#window-minimize');
const windowClose = app.querySelector<HTMLButtonElement>('#window-close');
const headerControls = app.querySelector<HTMLElement>('header.controls');
const historyBackButton = app.querySelector<HTMLButtonElement>('#history-back');
const historyForwardButton = app.querySelector<HTMLButtonElement>('#history-forward');

/** This session's parse history (§5, §6.1) — one buffer for this webview. */
const history = createHistory();

settingsToggle?.addEventListener('click', () => {
  void invoke('open_settings_window').catch(() => {});
});

windowMinimize?.addEventListener('click', () => {
  void invoke('minimize_window').catch(() => {});
});

windowClose?.addEventListener('click', () => {
  void invoke('close_window').catch(() => {});
});

app.querySelectorAll<HTMLElement>('.resize-handle').forEach((handle) => {
  handle.addEventListener('mousedown', (e) => {
    if (e.button === 0) {
      e.preventDefault();
      const dir = handle.dataset.direction as ResizeDirection;
      if (dir) {
        void getCurrentWindow().startResizeDragging(dir).catch(() => {});
      }
    }
  });
});

headerControls?.addEventListener('dblclick', (e) => {
  if (!(e.target as HTMLElement)?.closest('button')) {
    void invoke('toggle_maximize_window').catch(() => {});
  }
});

/**
 * Which chrome shape the window is wearing. The control for it lives in the
 * settings window and in the right-click menu; the main window still needs the
 * value, because it decides both shapes of the band — a reserved 28px row in
 * `native`, a floating overlay over the sentence in the other two — and the CSS
 * keys every one of those rules off `#app[data-chrome]`.
 */
let chromeMode: ChromeMode = 'native';

/**
 * The persisted background opacity as a percentage, or `null` while none is
 * persisted and `tokens.css` owns it. Stored raw: the `native` clamp is
 * applied on the way to the stylesheet, never written back here, so leaving
 * native restores exactly what the user chose.
 */
let currentAlpha: number | null = null;

/**
 * The platform, as the backend reports it. Module-scope because the right-click
 * menu needs it too, and it decides whether the native-frame item exists.
 * Defaults to `macos` for the window between import and the first `platform`
 * answer — a right-click inside those few milliseconds is the only thing that
 * could see the wrong answer.
 */
let currentPlatform = 'macos';

/**
 * True while a hover is showing chrome that `header` mode normally hides.
 * Tracked rather than derived from the event, so only *crossings* reach
 * the backend: each peek is an objc round-trip on the main thread, and a
 * duplicate `pointerenter` (or one arriving while the reveal is already up)
 * should cost nothing.
 */
let peeking = false;

/**
 * Grace period before a peek is undone, once the cursor has left the window.
 *
 * Long enough (a second) to be forgiving of a cursor that clips the edge on its
 * way somewhere else, and of the reach for a traffic light: those buttons are
 * real NSViews over the band, so touching one reads as a `pointerleave` here.
 * A short delay made that collapse the reveal mid-reach, and the re-entry
 * re-showed it — the same oscillation the transparency flip used to cause,
 * localised to the buttons.
 */
const PEEK_HIDE_MS = 1000;

/**
 * Height of the titlebar band, in logical pixels, and the amount the window
 * grows *upward* while peeking so the revealed bar is added ON TOP of the
 * window rather than covering the first line of the sentence. Sent to the
 * backend with each peek: layout owns this number, and `--band-h` in
 * `global.css` is the same value for the padding that keeps the content
 * stationary while the frame is taller.
 */
const BAND_HEIGHT = 28;

/**
 * Whether `peek_titlebar` grows the window frame upward on this platform to
 * absorb the revealed band (macOS, Windows). Queried once at startup so
 * `showPeek` stays synchronous.
 */
let peekGrowsFrame = false;

/** Pending un-peek, or `undefined` when none is armed. */
let peekHide: number | undefined;

function showPeek(): void {
  // A re-entry inside the grace period cancels the pending hide outright, so a
  // trip across a traffic light costs no backend calls at all.
  window.clearTimeout(peekHide);
  peekHide = undefined;
  // Only `header` reveals. `native` has nothing to reveal — the chrome is
  // already there — and `content` deliberately never shows it (spec §3).
  if (chromeMode !== 'header' || peeking) return;
  peeking = true;
  // `peeked` expands the band and fades the gear in on every platform.
  // `peek-offset` adds `padding-top` only when the OS frame grows upward to
  // absorb it (macOS, Windows), keeping the content stationary on screen.
  app.classList.add('peeked');
  if (peekGrowsFrame) {
    app.classList.add('peek-offset');
  }
  void invoke('peek_titlebar', { visible: true, height: BAND_HEIGHT }).catch(() => {});
}

function hidePeek(): void {
  if (!peeking || peekHide !== undefined) return;
  peekHide = window.setTimeout(() => {
    peekHide = undefined;
    peeking = false;
    app.classList.remove('peeked', 'peek-offset');
    void invoke('peek_titlebar', { visible: false, height: BAND_HEIGHT }).catch(() => {});
  }, PEEK_HIDE_MS);
}

function applyChromeMode(mode: ChromeMode): void {
  chromeMode = mode;
  app.setAttribute('data-chrome', mode);
  // A peek still notionally in flight when the user leaves `header` is now
  // wrong either way: `native` has shown the chrome for real, and `content`
  // wants none. The flag has to drop here or the `pointerleave` that follows
  // would undo chrome the user just asked for.
  if (mode !== 'header') {
    window.clearTimeout(peekHide);
    peekHide = undefined;
    app.classList.remove('peeked', 'peek-offset');
    // And `content` has to tell the backend. The reveal is its state, not the
    // class's — the region on Windows, the frame height on macOS — and this is
    // the last chance to undo it: the `pointerleave` that follows reads the
    // flag dropped just below and returns early. Left standing, the reserved
    // row stays inside the Windows region while `content` hides every control
    // that paints it, which is a dead 28px strip above the sentence until the
    // next peek.
    //
    // `native` is deliberately excluded, not overlooked: there the chrome the
    // peek was borrowing is now shown for real, and un-peeking would take it
    // straight back off. Gated on `peeking` for the reason a repeated
    // `pointerenter` is: only transitions talk to the backend, and boot applies
    // a mode before any pointer has been near the band.
    if (peeking && mode === 'content') {
      void invoke('peek_titlebar', { visible: false, height: BAND_HEIGHT }).catch(() => {});
    }
    peeking = false;
  }
  // The clamp reads the mode, so it has to be re-evaluated whenever the mode
  // moves — `native` renders opaque whatever `currentAlpha` holds.
  applySurfaceAlpha();
}

applyChromeMode('native');

/** The label shown for each mode, in the order the menu lists them. */
const CHROME_LABELS: ReadonlyArray<[ChromeMode, string]> = [
  ['native', 'Native frame'],
  ['header', 'Header + resize edges'],
  ['content', 'Content only'],
];

/**
 * Apply a mode the user picked from the menu.
 *
 * Applied locally as well as sent: the `emit` round-trip is asynchronous and
 * comes back to this same listener, so without the local call the window would
 * change a frame or two after the click. `applyChromeMode` is idempotent, so
 * handling the echo costs nothing.
 */
function selectChromeMode(mode: ChromeMode): void {
  applyChromeMode(mode);
  void invoke('set_chrome_mode', { mode }).catch(() => {});
  void emit('settings-changed', { chrome_mode: mode }).catch(() => {});
}

function chromeMenuEntries(): MenuEntry[] {
  const entries: MenuEntry[] = CHROME_LABELS
    // Windows has no native-frame state (spec §4), so offering it would name a
    // shape the platform cannot draw.
    .filter(([mode]) => mode !== 'native' || currentPlatform !== 'windows')
    .map(([mode, label]) => ({
      kind: 'radio' as const,
      label,
      checked: chromeMode === mode,
      onSelect: () => selectChromeMode(mode),
    }));
  entries.push({ kind: 'separator' });
  entries.push({
    kind: 'action',
    label: 'Settings…',
    onSelect: () => {
      void invoke('open_settings_window').catch(() => {});
    },
  });
  entries.push({
    kind: 'action',
    label: 'Quit',
    onSelect: () => {
      void invoke('close_window').catch(() => {});
    },
  });
  return entries;
}

/**
 * Bound to `document`, not `#app`.
 *
 * A keyboard-invoked context menu (Shift+F10, or the Menu key) targets the
 * focused element, which in content mode is `<body>` — an ancestor of `#app`,
 * so the event would never reach a listener bound there. Content mode has no
 * tab stops at all, so this is the only binding that keeps the menu reachable
 * without a pointer. `preventDefault` also suppresses the WebView2 default
 * menu. The coordinates are viewport-relative CSS px, which is what the
 * backend's `popup_at` takes relative to this window.
 */
document.addEventListener('contextmenu', (event) => {
  event.preventDefault();
  openContextMenu([...readingMenuEntries(), ...chromeMenuEntries()], event.clientX, event.clientY);
});

// The cursor anywhere inside the window reveals the bar; only leaving the
// window hides it. `#app` fills the viewport, so its own enter/leave is the
// window's — and unlike the 6px band it cannot be missed by a fast pointer.
app.addEventListener('pointerenter', showPeek);
app.addEventListener('pointermove', showPeek);
app.addEventListener('pointerleave', hidePeek);

// Keyboard parity: the gear is a tab stop even while invisible, and only the
// backend can grow the frame, so focus has to drive the same path as hover
// rather than leaning on a CSS `:focus-within` rule. The history pair sits
// ahead of the gear in tab order, so it needs the same pair of listeners —
// otherwise it is the first, invisible tab stop once there is history to walk
// (spec §7.1, a WCAG 2.4.7 trap).
settingsToggle?.addEventListener('focus', showPeek);
settingsToggle?.addEventListener('blur', hidePeek);
historyBackButton?.addEventListener('focus', showPeek);
historyBackButton?.addEventListener('blur', hidePeek);
historyForwardButton?.addEventListener('focus', showPeek);
historyForwardButton?.addEventListener('blur', hidePeek);

/**
 * Tells the backend the background colour this page renders.
 *
 * Windows erases the pixels a peek newly exposes before the webview has painted
 * them, and it can only do that in a colour it was given. Read from the live
 * computed style rather than passed as a constant: `--color-bg` is an `oklch()`
 * token with a `prefers-color-scheme` branch and a `data-theme` override, and
 * the browser is the only thing that resolves all three to the sRGB triple the
 * strip has to match.
 *
 * Below 100 % alpha, the canvas's internal premultiply/unpremultiply round
 * trip can shift the reported RGB by one at the low end (`[17, 17, 17]` for a
 * page rendering `18, 18, 18`). Inert: that value is only sent onward when
 * `opaque` is true below, and every path that reaches `opaque` also
 * re-reports once the effective alpha is back at 1, so the off-by-one is
 * never the value the erase fill actually uses.
 */
function reportBackground(): void {
  const rendered = getComputedStyle(document.body).backgroundColor;
  const rgb = resolveColor(rendered);
  if (rgb === null) return;
  // The *effective* alpha, not the stored one: `native` renders fully opaque
  // whatever `surface_alpha` holds, and that is precisely the mode whose erase
  // fill stops the top-edge flash. An empty value means unset, which the
  // stylesheet resolves to 1.
  const [, effective] = surfaceAlphaVar(currentAlpha, chromeMode);
  const opaque = effective === '' || effective === '1';
  void invoke('set_window_background', {
    r: rgb[0],
    g: rgb[1],
    b: rgb[2],
    opaque,
  }).catch(() => {});
}

// The token flips with the OS theme, and the erase colour has to flip with it.
window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', reportBackground);

/**
 * Push the font-size tokens SETTINGS carries onto the root element.
 *
 * Custom properties only — no re-render. Every size in the design derives from
 * these four tokens, so the ruby, the chips and the definition pane all reflow
 * themselves off one style write.
 */
function applyFontSizes(settings: Partial<Settings>): void {
  for (const [token, value] of fontVars(settings)) {
    if (value === '') document.documentElement.style.removeProperty(token);
    else document.documentElement.style.setProperty(token, value);
  }
  currentGloss = clampFont('gloss', settings.font_gloss);
}

/** Write or remove one custom property on the root element. */
function setRootToken(token: string, value: string): void {
  if (value === '') document.documentElement.style.removeProperty(token);
  else document.documentElement.style.setProperty(token, value);
}

/**
 * Push the appearance tokens SETTINGS carries onto the root element: the six
 * text colours, the window background, the two font families, and the
 * outline and drop-shadow classes.
 *
 * FULL says whether an absent key means "cleared" — a full snapshot, where
 * Reset or Default emptied it — or "not mentioned", as in the colour picker's
 * per-frame `{ colour_kana }` preview, which must not wipe the other four.
 * The gloss family has no token here: it rides the popover payload.
 */
function applyAppearance(settings: Partial<Settings>, full: boolean): void {
  for (const [token, value] of [...colourVars(settings), ...familyVars(settings)]) {
    if (full || value !== '') setRootToken(token, value);
  }
  if (full || settings.font_family_gloss !== undefined) {
    currentGlossFont = clampFamily(settings.font_family_gloss);
  }
  if (settings.text_outline !== undefined) {
    document.documentElement.classList.toggle('text-outline', settings.text_outline);
  }
  if (settings.text_shadow !== undefined) {
    document.documentElement.classList.toggle('text-shadow', settings.text_shadow);
  }
  // The erase fill Windows paints over newly exposed pixels is whatever this
  // page last reported, so a picked background has to re-report or a peek
  // shows a strip of the old colour. A drag previews one payload per frame
  // and each is a colour change like any other.
  if (full || settings.colour_bg !== undefined) reportBackground();
}

/**
 * Push the background-opacity token onto the root element, clamped by the
 * chrome mode.
 *
 * Separate from `applyFontSizes` because it has a second input — the mode —
 * which is also why `applyChromeMode` calls it: a mode change is itself a
 * reason for the clamp to flip, and that path carries no alpha of its own.
 */
function applySurfaceAlpha(): void {
  const [token, value] = surfaceAlphaVar(currentAlpha, chromeMode);
  if (value === '') document.documentElement.style.removeProperty(token);
  else document.documentElement.style.setProperty(token, value);
  // `reportBackground` runs at startup and on an OS theme change today. The
  // erase gate rides on the effective alpha too, so it has to follow both of
  // this function's inputs. Sending it unconditionally is cheaper than
  // tracking whether the value crossed the opaque boundary.
  reportBackground();
}

/**
 * The gloss size to send with popover content, or `null` while none is
 * persisted and the tooltip's own stylesheet owns the default.
 */
let currentGloss: number | null = null;

/** The gloss family to send with popover content, or `null` while unset. */
let currentGlossFont: string | null = null;

async function applySettings(): Promise<void> {
  currentPlatform = await invoke<string>('platform').catch(async () => {
    const isMac = await invoke<boolean>('is_macos').catch(() => false);
    return isMac ? 'macos' : 'windows';
  });
  peekGrowsFrame = await invoke<boolean>('peek_grows_frame').catch(() => false);
  app.setAttribute('data-platform', currentPlatform);
  reportBackground();
  const settings = await invoke<Settings>('get_settings');
  currentAlpha = clampAlpha(settings.surface_alpha);
  if (settings.chrome_mode !== undefined) {
    applyChromeMode(settings.chrome_mode);
  }
  if (settings.furigana_mode !== undefined) {
    currentFuriganaMode = settings.furigana_mode;
  }
  if (settings.hide_pos !== undefined) {
    currentFilters.hide_pos = settings.hide_pos;
  }
  if (settings.hide_xrefs !== undefined) {
    currentFilters.hide_xrefs = settings.hide_xrefs;
  }
  if (settings.hide_usage !== undefined) {
    currentFilters.hide_usage = settings.hide_usage;
  }
  if (settings.hide_kanji !== undefined) {
    currentFilters.hide_kanji = settings.hide_kanji;
  }
  applyFontSizes(settings);
  applyAppearance(settings, true);
  applyGrowSettings(settings);
  // Unconditional: a settings file with no `chrome_mode` never reaches
  // `applyChromeMode`, and the token still has to be written.
  applySurfaceAlpha();
}

// `Partial<Settings>`, not `Settings`: a bare `{ chrome_mode }` from the
// right-click menu and a `{ surface_alpha }` from a slider drag are both real
// payloads on this event, and the `isFullSnapshot` check below is only
// meaningful if the type admits that the keys can be missing. `settings.ts`'s
// `applySettings` takes the same view of the same event.
void listen<Partial<Settings>>('settings-changed', (e) => {
  const settings = e.payload;
  if (settings.hide_pos !== undefined) currentFilters.hide_pos = settings.hide_pos;
  if (settings.hide_xrefs !== undefined) currentFilters.hide_xrefs = settings.hide_xrefs;
  if (settings.hide_usage !== undefined) currentFilters.hide_usage = settings.hide_usage;
  if (settings.hide_kanji !== undefined) currentFilters.hide_kanji = settings.hide_kanji;
  // `applyFontSizes` reads an absent size as "clear it", which is right for a
  // full snapshot — the user emptied that box — and wrong for a partial
  // payload. `{ chrome_mode }` from the right-click menu and
  // `{ surface_alpha }` from a slider drag carry no sizes at all, and applying
  // them unguarded wipes the sizes the user set. A full snapshot is the one
  // carrying the two keys `Settings` does not mark optional.
  const isFullSnapshot =
    settings.always_on_top !== undefined && settings.clipboard_monitoring !== undefined;
  if (isFullSnapshot) applyFontSizes(settings);
  applyAppearance(settings, isFullSnapshot);
  if (settings.surface_alpha !== undefined) {
    currentAlpha = clampAlpha(settings.surface_alpha);
    applySurfaceAlpha();
  }
  if (settings.chrome_mode !== undefined) applyChromeMode(settings.chrome_mode);
  if (settings.furigana_mode !== undefined && settings.furigana_mode !== currentFuriganaMode) {
    currentFuriganaMode = settings.furigana_mode;
    // `display` is the single writer of `lastResult` (§6.2) and already does
    // exactly what this used to duplicate by hand: `closePopover()`, then
    // `renderSentence` into `#output`. Routing through it here means this
    // path gets every future change to `display` — the scroll guard above
    // included — for free, instead of silently missing it. `display(null)`
    // is already a no-op, so no `lastResult !== null` guard is needed here.
    display(lastResult);
  }
  applyGrowSettings(settings);
  // A size, a font or the grow settings themselves can each change what fits.
  // Unconditional, because asking for the growth already applied is free.
  refit();
});

/** Milliseconds the cursor must rest on a chip before its popover opens. */
const DWELL_MS = 350;

/**
 * The most recent parse, kept so a hover can find the entry for the chip under
 * the cursor. The chips carry only `data-start`; the entries live here.
 */
let lastResult: ParseResult | null = null;

/**
 * Which of `lastResult`'s readings is on screen: 0 is `segments`, n ≥ 1 is
 * `alternatives[n - 1]` (Phase 2 design §6). `display` resets it whenever a
 * different result arrives; a re-render of the same result — a furigana
 * change, a reading switch — keeps it.
 */
let readingIndex = 0;

/** Every reading of `result`, top-1 first. */
function readings(result: ParseResult): Segment[][] {
  return [result.segments, ...result.alternatives];
}

/** The reading currently rendered, or `[]` before the first parse. */
function displayedSegments(): Segment[] {
  if (lastResult === null) return [];
  return readings(lastResult)[readingIndex] ?? lastResult.segments;
}

/** Show reading `index` of the current result, wrapping at either end. */
function showReading(index: number): void {
  if (lastResult === null) return;
  const count = readings(lastResult).length;
  if (count < 2) return;
  readingIndex = ((index % count) + count) % count;
  display(lastResult);
}

/**
 * `Reading n of k` radio rows for the right-click menu, then a separator —
 * or nothing at all when the parse has one reading, so the menu is exactly
 * what it was before Phase 2.
 */
function readingMenuEntries(): MenuEntry[] {
  if (lastResult === null) return [];
  const count = readings(lastResult).length;
  if (count < 2) return [];
  const entries: MenuEntry[] = [];
  for (let i = 0; i < count; i += 1) {
    entries.push({
      kind: 'radio',
      label: `Reading ${i + 1} of ${count}`,
      checked: i === readingIndex,
      onSelect: () => showReading(i),
    });
  }
  entries.push({ kind: 'separator' });
  return entries;
}

/** Pending dwell timer, or `undefined` when none is armed. */
let dwell: number | undefined;

function segmentAt(start: string | undefined): Segment | undefined {
  if (start === undefined || lastResult === null) return undefined;
  return displayedSegments().find((segment) => String(segment.start) === start);
}

/**
 * The chip an event happened on, or `null`.
 *
 * `.unmatched` runs are `<span>`s without the `chip` class, so this returns
 * `null` for them and they get no popover — an empty box for a span with no
 * entries is the one wrong outcome available here.
 */
function chipFrom(target: EventTarget | null): HTMLElement | null {
  if (!(target instanceof HTMLElement)) return null;
  return target.closest<HTMLElement>('.chip');
}

function clearDwell(): void {
  if (dwell === undefined) return;
  clearTimeout(dwell);
  dwell = undefined;
}

/** How often the keep rule samples the cursor while the tooltip is open, in ms. */
const KEEP_POLL_MS = 60;

/**
 * Screen position of the webview viewport's top-left corner, in CSS px, or
 * `null` before any mouse has moved over the app.
 *
 * This exists because `innerPosition()` does not deliver it. It is documented
 * as the client area's corner, but on macOS it returns the window *frame*'s
 * corner — measured in a running app it came back byte-identical to
 * `outerPosition()`, so the title bar was missing from it and every tooltip
 * was placed a title bar too high: "below the word" landed on top of the word,
 * and "above the word" floated the same distance clear of it.
 *
 * A mouse event carries both viewport (`clientX/Y`) and screen (`screenX/Y`)
 * coordinates in the same CSS px, so their difference is the origin — measured
 * rather than assumed. `window.screenX/Y` is not an alternative: WKWebView
 * reports it as the screen's own origin, not the viewport's.
 */
let viewportOrigin: Point | null = null;

// Passive and document-wide so any movement over the app calibrates this, not
// only movement that reaches a chip: the keyboard path has no mouse event of
// its own and would otherwise never have an origin to use.
document.addEventListener(
  'mousemove',
  (e) => {
    viewportOrigin = { x: e.screenX - e.clientX, y: e.screenY - e.clientY };
  },
  { passive: true },
);

/**
 * The tooltip's rectangle as last placed, in **physical** screen px — the same
 * unit `cursorPosition()` reports, and the only unit `shouldKeep`'s distance
 * comparison can be safely measured against. Every other coordinate in this
 * file is CSS px; this one field deliberately is not — mixing the two here
 * makes a cursor moving toward the tooltip measure as moving away on any
 * display above 1x. `null` when closed.
 *
 * The whole rect rather than just its centre because spec §3.3 has two rules,
 * not one: a cursor *inside* the tooltip is reading it and must never be
 * judged by direction of travel (reaching for the scrollbar at the right edge
 * is almost always a move away from the centre), and a cursor that leaves the
 * tooltip dismisses it. Neither is expressible from a centre point alone.
 */
let tooltipRect: Rect | null = null;
/** Previous cursor sample, so the keep rule has something to compare against. */
let lastCursor: Point | null = null;
/** The keep-rule poll, or `undefined` when the tooltip is closed. */
let keepPoll: number | undefined;
/**
 * Whether the cursor has left the chip the open tooltip belongs to.
 *
 * Spec §3.3's first row is a conjunction — "cursor leaves the chip **and** the
 * keep rule fails" — so the keep rule must not judge anything while the cursor
 * is still on the word. The original tooltip held the same shape: its keep
 * flag was consumed by the word window on mouse-leave, so its mouse hook never
 * hid a tooltip whose word was still under the cursor. Set by the chip's
 * `mouseout`, which also means a tooltip opened by FOCUS never arms the poll at
 * all — there is no cursor gesture to judge, and unrelated mouse movement
 * elsewhere on screen must not dismiss a tooltip the keyboard opened.
 */
let keepArmed = false;
/** The chip awaiting a measurement, so the reply knows what to anchor to. */
let pendingChip: HTMLElement | null = null;
/**
 * Bumped every time `closePopover` runs. `placeFor` captures this at the
 * start of its round trip and re-checks it after every await: a dismissal
 * that lands mid-flight (Escape, a scroll, a fresh parse, the next chip's own
 * dwell) must stop that flight from showing a tooltip nothing can dismiss
 * anymore, rather than racing it to the screen.
 */
let openId = 0;

function closePopover(): void {
  clearDwell();
  openId += 1;
  // Nothing shown or committed to showing means nothing for Rust to hide —
  // the common case, and the one a momentum scroll fires this into many
  // times a second.
  const wasActive = keepPoll !== undefined || pendingChip !== null;
  if (keepPoll !== undefined) {
    clearInterval(keepPoll);
    keepPoll = undefined;
  }
  keepArmed = false;
  tooltipRect = null;
  lastCursor = null;
  pendingChip = null;
  if (wasActive) {
    // Fire-and-forget: a hide that fails to reach Rust is cosmetic (the
    // tooltip stays visible one extra beat), not worth an unhandled
    // rejection over.
    void invoke('hide_popover').catch(() => {});
  }
}

/**
 * Watch the cursor while the tooltip is open.
 *
 * Polled rather than event-driven because the cursor spends the decisive
 * moments over NEITHER webview — in the gap between the word and the tooltip —
 * where no `mousemove` reaches either page. `cursorPosition()` reads the
 * global position, which is the only thing that works there.
 *
 * Called from two places — the chip's `mouseout` and the end of `placeFor` —
 * because either can be last: the cursor can leave the word before the
 * measurement round trip returns, or long after. Both guards below make the
 * earlier of the two a no-op.
 */
function startKeepPoll(): void {
  // Not armed means the cursor is still on the word (or the tooltip was opened
  // by focus and there is no cursor gesture to judge); no rect means nothing
  // has been placed yet for this open.
  if (!keepArmed || tooltipRect === null || keepPoll !== undefined) return;
  keepPoll = window.setInterval(() => {
    if (tooltipRect === null) return;
    void cursorPosition()
      .then((position) => {
        // A stale tick from a tooltip that closed while this read was in
        // flight must not resurrect `lastCursor` out from under whatever
        // opened next — checked first, before touching it.
        const rect = tooltipRect;
        if (rect === null) return;
        const next = { x: position.x, y: position.y };
        const previous = lastCursor;
        const wasInside = previous !== null && contains(rect, previous);
        lastCursor = next;
        // Inside the tooltip: the user is reading or scrolling it, and no
        // movement in there is a dismissal. Reaching for the scrollbar at the
        // right edge is a move away from the centre, so judging this by
        // direction of travel would close the tooltip mid-scroll.
        if (contains(rect, next)) return;
        // Spec §3.3, second row: "cursor leaves the tooltip itself".
        if (wasInside) {
          closePopover();
          return;
        }
        if (previous === null) return;
        // A resting cursor is not movement away, so it keeps the tooltip.
        if (previous.x === next.x && previous.y === next.y) return;
        if (!shouldKeep(previous, next, centreOf(rect))) closePopover();
      })
      .catch(() => {
        // A dropped read costs this tick its comparison; the next one, 60ms
        // later, tries again. Not worth an unhandled rejection every 60ms.
      });
  }, KEEP_POLL_MS);
}

/** What `popover-content` carries. `kanji` is present only when there is a block to show. */
interface PopoverPayload {
  entries: Entry[];
  filters: GlossFilters;
  gloss?: number;
  /** The definitions font family, a bare name; `popover.ts` quotes it. */
  gloss_font?: string;
  kanji?: KanjiInfo[];
}

/** Ask the popup to render CHIP's entries, if the last parse still knows that span. */
function openFor(chip: HTMLElement): void {
  const segment = segmentAt(chip.dataset.start);
  const entries = segment?.entries;
  // No entries means a stale chip from a superseded parse, or an unmatched
  // run — neither is an error worth surfacing.
  if (segment === undefined || entries === undefined || entries.length === 0) return;
  // Bumped here, not only in `closePopover`: focus moving straight from chip
  // A to chip B (no `closePopover` in between — see the `focusin` listener
  // below) must still invalidate a `placeFor` already in flight for A, or
  // the two placements race and whichever resolves last wins the window.
  openId += 1;
  // A fresh open owns its own arm state. On the mouse path this is already
  // false (a `mouseout` cancels the dwell, so a completed dwell means the
  // cursor never left), and the focus path must not inherit a `true` left by
  // an earlier hover that never opened anything.
  keepArmed = false;
  // Captured before the one await below and re-checked after it, exactly as
  // `placeFor` does: a dismissal, or the next chip's own open, that lands
  // while the kanji lookup is in flight must not emit content for a popover
  // nothing will place.
  const generation = openId;
  // The kanji of the SURFACE, not the headword (design §1.1): a PRONOUNCE
  // match shows the reader kana, and a block for 履 would explain a
  // character they are not looking at. Under `hide_kanji` there is nothing
  // to ask for, so no round trip is paid.
  const kanji = currentFilters.hide_kanji
    ? Promise.resolve<KanjiInfo[]>([])
    : lookupKanji(segment.surface);
  void kanji
    .then((records) => {
      if (openId !== generation) return;
      // Set only now, once content is actually about to be emitted — not
      // before the lookup — so a chip left behind by a fast mouse move
      // during the round trip finds `pendingChip` null (or already the next
      // chip) when its own stale reply lands, and `closePopover`'s null
      // stands: the reply is dropped instead of placed at the wrong chip.
      pendingChip = chip;
      // Fire-and-forget: a failed emit just leaves the tooltip unshown, no
      // worse than the user never having hovered at all.
      // The size rides along with the content because the popover is a second
      // webview with its own `:root`, and it must be applied before that
      // render: the window sizes itself off what the render measures. The
      // kanji ride along for the same reason — one render, one measurement.
      const payload: PopoverPayload = { entries, filters: currentFilters };
      if (currentGloss !== null) payload.gloss = currentGloss;
      if (currentGlossFont !== null) payload.gloss_font = currentGlossFont;
      if (records.length > 0) payload.kanji = records;
      return emit('popover-content', payload);
    })
    .catch(() => {});
}

/**
 * Convert the chip's client rect to screen coordinates and place the window.
 *
 * `innerPosition` and the monitor are physical pixels; everything the DOM
 * reports is CSS pixels. Dividing by the scale factor before mixing them is
 * invisible on a 1x display and doubles every offset on a Retina one.
 */
async function placeFor(chip: HTMLElement, size: { width: number; height: number }): Promise<void> {
  // Captured once, re-checked after every await below: a dismissal that
  // lands mid-flight must stop this round trip from showing a tooltip
  // nothing can dismiss anymore, rather than racing it to the screen.
  const generation = openId;
  const current = getCurrentWindow();
  // `innerPosition`, NOT `outerPosition`: the rect below is measured from the
  // top-left of the webview, and on a decorated window that is a title bar
  // lower than the window frame's corner. Added to the frame's corner instead,
  // every tooltip lands a title bar too high — squarely on top of the word it
  // is anchored to, rather than below it.
  const [origin, scale] = await Promise.all([current.innerPosition(), current.scaleFactor()]);
  if (openId !== generation || !chip.isConnected) return;
  const box = chip.getBoundingClientRect();
  // The measured origin when there is one; `innerPosition` only as a fallback,
  // and a knowingly wrong one — see `viewportOrigin`. It is reached before the
  // mouse has ever entered the app, i.e. a session driven by Tab from the very
  // first keystroke, where being a title bar out beats not opening at all.
  const client = viewportOrigin ?? { x: origin.x / scale, y: origin.y / scale };
  const left = client.x + box.left;
  const top = client.y + box.top;
  const rect: Rect = { left, top, right: left + box.width, bottom: top + box.height };

  // The monitor under the WORD, not the app's own — a window straddling two
  // screens must clamp against the one the user is looking at.
  //
  // Found by containment rather than with `monitorFromPoint`, whose units are
  // not the same on every platform: on macOS tao hands the point straight to
  // `CGRectContainsPoint(CGDisplayBounds(..))`, which is logical points, while
  // on Windows it goes to `MonitorFromPoint`, which is physical device pixels.
  // Fed physical coordinates on a 2x display it therefore missed every screen
  // for any word past roughly half the display's width and returned null — and
  // this function returned silently, so the right-hand side of every sentence
  // had no tooltip at all. `position` and `size` are physical everywhere, so
  // testing the point against them has one answer on every platform.
  const monitors = await availableMonitors();
  if (openId !== generation) return;
  const point = { x: left * scale, y: top * scale };
  const monitor = monitors.find((m) =>
    contains(
      {
        left: m.position.x,
        top: m.position.y,
        right: m.position.x + m.size.width,
        bottom: m.position.y + m.size.height,
      },
      point,
    ),
  );
  // Genuinely off every screen — a window dragged mostly past an edge. Placing
  // against an arbitrary monitor would park the tooltip somewhere unrelated to
  // the word, so nothing is shown.
  if (monitor === undefined) return;
  const work: Rect = {
    left: monitor.workArea.position.x / scale,
    top: monitor.workArea.position.y / scale,
    right: (monitor.workArea.position.x + monitor.workArea.size.width) / scale,
    bottom: (monitor.workArea.position.y + monitor.workArea.size.height) / scale,
  };

  // Never taller than the work area allows: past that it scrolls, the way
  // the original tooltip did, rather than being placed off-screen.
  const height = Math.min(size.height, work.bottom - work.top - 2 * MARGIN);
  const placed = placePopover(rect, { width: size.width, height }, work);
  // Physical px, matching `cursorPosition()` — see `tooltipRect`'s own
  // declaration for why this one field isn't CSS px like everything above it.
  tooltipRect = {
    left: placed.left * scale,
    top: placed.top * scale,
    right: (placed.left + size.width) * scale,
    bottom: (placed.top + height) * scale,
  };
  lastCursor = null;
  await invoke('place_popover', {
    x: Math.round(placed.left),
    y: Math.round(placed.top),
    width: Math.round(size.width),
    height: Math.round(height),
  });
  if (openId !== generation) {
    // The window is already shown — that `invoke` above was already in
    // flight when this round trip was invalidated. Whatever invalidated it
    // ran while `pendingChip` was `null` and `keepPoll` was not yet armed, so
    // `closePopover`'s own `wasActive` gate (finding 7) saw nothing to hide
    // and skipped it. This is the one place that must undo a show it did not
    // arm, rather than leaving a tooltip nothing can ever dismiss again.
    void invoke('hide_popover').catch(() => {});
    return;
  }
  startKeepPoll();
}

// Delegated on `#output` rather than on `.sentence`: `show()` replaces the
// sentence element on every parse, so a listener bound to it would be dropped
// with it, while `#output` lives for the app's lifetime.
output.addEventListener('mouseover', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  const relatedChip = chipFrom(e.relatedTarget as EventTarget | null);
  if (relatedChip === chip) return;

  viewportOrigin = { x: e.screenX - e.clientX, y: e.screenY - e.clientY };

  // Re-armed per chip with no sticky swap: moving between chips hides the open
  // tooltip and starts a fresh dwell.
  closePopover();
  dwell = window.setTimeout(() => openFor(chip), DWELL_MS);
});

output.addEventListener('mouseout', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  const relatedChip = chipFrom(e.relatedTarget as EventTarget | null);
  if (relatedChip === chip) return;

  // Only the pending dwell is cancelled here. An OPEN tooltip is left to the
  // keep rule — leaving the word *toward* the tooltip must not dismiss it,
  // which is the whole point of spec §3.2.
  clearDwell();
  // And this is the other half of spec §3.3's first row: the keep rule only
  // starts judging once the cursor has actually left the chip. Before that,
  // every sample is a hand resting on the word, and any two-pixel drift away
  // from the tooltip's centre would dismiss it — with no way to reopen, since
  // `mouseover` does not re-fire inside the same element.
  keepArmed = true;
  startKeepPoll();
});

output.addEventListener('focusin', (e) => {
  const chip = chipFrom(e.target);
  if (chip === null) return;
  clearDwell();
  openFor(chip);
});

output.addEventListener('focusout', (e) => {
  if (chipFrom(e.target) === null) return;
  closePopover();
});

document.addEventListener('keydown', (e) => {
  // Focus is deliberately not moved: the user is mid-sentence, and Escape
  // dismissing a tooltip should not cost them their place in the tab order.
  if (e.key === 'Escape') closePopover();
  // `]` / `[` step through the current parse's readings (Phase 2 design §6).
  // Bare keys: this window has no text input, both exist on JIS and US
  // layouts, and neither scrolls `.panes`. The Alt-modified pair is history
  // (`history-step`, Rust-side) and the webview sees that keydown too, so a
  // modifier means this is not a reading step.
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  // A native menu, while open, takes the keyboard for itself, so a bracket
  // pressed then never reaches this listener; there is no open menu to close
  // under a changed reading any more.
  if (e.key === ']') showReading(readingIndex + 1);
  if (e.key === '[') showReading(readingIndex - 1);
});

let lastUnfocusedChip: HTMLElement | null = null;

void listen<{ x: number; y: number; screen_x: number; screen_y: number }>(
  'unfocused-mouse-move',
  (e) => {
    showPeek();
    if (document.hasFocus()) {
      lastUnfocusedChip = null;
      return;
    }
    const el = document.elementFromPoint(e.payload.x, e.payload.y);
    const chip = chipFrom(el);
    if (chip === null) {
      if (lastUnfocusedChip !== null) {
        lastUnfocusedChip = null;
        clearDwell();
        keepArmed = true;
        startKeepPoll();
      }
      return;
    }
    viewportOrigin = {
      x: e.payload.screen_x - e.payload.x,
      y: e.payload.screen_y - e.payload.y,
    };
    if (lastUnfocusedChip === chip) return;
    lastUnfocusedChip = chip;
    closePopover();
    dwell = window.setTimeout(() => openFor(chip), DWELL_MS);
  },
);

void listen('unfocused-mouse-leave', async () => {
  hidePeek();
  if (document.hasFocus()) return;
  lastUnfocusedChip = null;
  clearDwell();
  if (tooltipRect !== null) {
    try {
      const pos = await cursorPosition();
      if (!contains(tooltipRect, pos)) {
        closePopover();
      } else {
        keepArmed = true;
        startKeepPoll();
      }
    } catch {
      closePopover();
    }
  } else {
    closePopover();
  }
});

// The tooltip is placed from a rectangle all of these invalidate. `move` is
// new this phase: a DOM popover travelled with its parent for free, a separate
// window does not, and would be stranded on the desktop.
panes.addEventListener('scroll', closePopover);
// A bare `listen(event, handler)` defaults to `{ target: { kind: 'Any' } }`,
// which Tauri's window-event dispatch never matches against a `Window`-kind
// emit — left unscoped, both listeners below register but are never invoked,
// and the tooltip strands on the desktop the first time this window moves.
const windowTarget = { kind: 'Window', label: getCurrentWindow().label } as const;

let geometrySaveTimer: number | null = null;
function scheduleGeometrySave(): void {
  // The peek grows the frame by `BAND_HEIGHT`, which arrives here as an
  // ordinary `tauri://resize`. Persisting it would add a band to the stored
  // height on every hover. Nothing is lost by skipping: leaving the window
  // shrinks the frame back, and that resize — with `peeking` already false —
  // saves whatever size the user actually left it at.
  if (peeking) return;
  if (geometrySaveTimer !== null) window.clearTimeout(geometrySaveTimer);
  geometrySaveTimer = window.setTimeout(async () => {
    // Mid-grow the window is part way between two sizes, and the user's own
    // box has not moved; the resize that ends it saves.
    if (grower.base() !== null) return;
    try {
      const win = getCurrentWindow();
      const size = await win.innerSize();
      const pos = await win.outerPosition();
      const factor = await win.scaleFactor();
      // The user's own box, with any growth taken back off: a restart opens
      // at the size they set, not at whatever the last sentence needed.
      const own = grower.baseOf({ x: pos.x, y: pos.y, width: size.width, height: size.height });
      void invoke('save_window_geometry', {
        width: Math.round(own.width / factor),
        height: Math.round(own.height / factor),
        x: Math.round(own.x / factor),
        y: Math.round(own.y / factor),
      });
    } catch {}
  }, 300);
}

/**
 * The window moved or resized, so the open tooltip's anchor is stale — and so
 * is the measured viewport origin, since dragging a title bar produces no
 * `mousemove` inside the webview to re-measure it. Dropping it falls placement
 * back to `innerPosition` until the next mouse movement, which is wrong by a
 * title bar rather than by however far the window travelled.
 */
function invalidateGeometry(): void {
  viewportOrigin = null;
  closePopover();
  scheduleGeometrySave();
}

void listen('tauri://move', invalidateGeometry, { target: windowTarget }).catch(() => {
  // Unlike the parse/dictionary listeners below, losing this one doesn't
  // break parsing — the tooltip just stops following window moves, a step
  // back to the old DOM-popover behaviour rather than a functional failure.
});
void listen('tauri://resize', invalidateGeometry, { target: windowTarget }).catch(() => {
  // Same reasoning as the `move` listener above.
});
void listen('tauri://scale-change', () => {
  invalidateGeometry();
  void invoke('update_clip_region').catch(() => {});
}, { target: windowTarget }).catch(() => {
  // Same reasoning as the `move` listener above.
});

/** Keeps both buttons' `disabled` state in sync with the history's cursor. */
function syncHistoryButtons(): void {
  if (historyBackButton) historyBackButton.disabled = !history.canBack();
  if (historyForwardButton) historyForwardButton.disabled = !history.canForward();
}

/**
 * Renders `result`, or does nothing for `null` — the value `history.back()`
 * and `history.forward()` return at either end, so the click handlers below
 * pass them straight through with no guard of their own.
 *
 * The only writer of `lastResult`: `segmentAt` resolves a hover against
 * whatever this last rendered, so a chip in a parse navigated back to opens
 * its own tooltip rather than the newest parse's (§6.2).
 */
function display(result: ParseResult | null): void {
  if (result === null) return;
  // Before anything is replaced: a popover left open would be anchored to a
  // chip from the previous sentence that is about to leave the document.
  closePopover();
  // The same result re-rendered — a reading switch, a furigana change — keeps
  // the reading on screen and the reader's place in the pane: both happen
  // while reading one word, and losing the scroll position would make a
  // reading switch useless on exactly the long sentences it is for (Phase 2b
  // design §6). A different result is a new sentence: reading 1, top of the
  // pane.
  const same = result === lastResult;
  if (!same) readingIndex = 0;
  const scrollTop = same ? panes.scrollTop : 0;
  // Read before the mutation below, for exactly the reason `renderDictionary`
  // reads its own: `replaceChildren` on a parent holding focus drops focus to
  // `<body>` the instant the focused child leaves the document. The scroll
  // guard above was written without this half, and a reading switch is a
  // *keyboard* gesture (`]`/`[`) — so cycling readings threw away the
  // keyboard position of the reader who made the gesture, on every press.
  // Gated on focus having been inside, so a background parse arriving while
  // the user is elsewhere never steals it.
  const hadFocus = output.contains(document.activeElement);
  const focusedStart = hadFocus
    ? Number((document.activeElement as HTMLElement).dataset.start ?? Number.NaN)
    : Number.NaN;
  lastResult = result;

  const sentence = renderSentence(
    { segments: displayedSegments(), alternatives: [] },
    currentFuriganaMode,
  );
  output.replaceChildren(sentence);
  // `.panes` is the scroll container (up to 10,000 clipboard characters can
  // far exceed a viewport), and replacing `#output`'s children never touches
  // it on its own — so it is set explicitly: back to where it was for a
  // re-render, to the top for a new result rather than wherever a previous,
  // longer parse happened to leave it.
  panes.scrollTop = scrollTop;
  if (hadFocus) {
    // By character offset, not by chip ordinal: a different reading segments
    // the same sentence differently, so chip *i* is a different word while
    // character *i* is the same character the reader was looking at. Only for
    // a re-render — a new sentence is new text, and putting focus back on
    // "the word at offset 12" of something the reader has not read yet would
    // be a guess, so that case lands on the pane.
    const chip = same ? chipCovering(focusedStart) : null;
    // `#output` carries `tabindex="-1"` for the same reason `#dictionary`
    // does: somewhere to land when there is no focusable chip to return to —
    // a new sentence, or an offset that now falls inside an unmatched run,
    // which is a `<span>` and deliberately not in the tab order.
    (chip ?? output).focus();
  }
  syncHistoryButtons();
  refit();
}

/**
 * The sentence's average line height in CSS px, ruby included, or 0 before
 * anything is rendered.
 */
function sentenceLinePitch(): number {
  const sentence = output.querySelector<HTMLElement>('.sentence');
  if (sentence === null) return 0;
  // Per line fragment, not per element: an unmatched run is inline and can
  // wrap across several lines, which one bounding box would count as one.
  const bottoms = Array.from(sentence.children).flatMap((el) =>
    Array.from(el.getClientRects(), (rect) => rect.bottom),
  );
  // Half a character: well past a chip's padding, well short of a line.
  const threshold = Number.parseFloat(getComputedStyle(sentence).fontSize) / 2;
  return linePitch(bottoms, sentence.getBoundingClientRect().height, threshold);
}

/**
 * Grow the window to show what of the sentence is hidden below the user's own
 * size, up to the line cap, or shrink it back when it all fits.
 *
 * Measured in CSS px and handed to the grower in physical px, the unit it
 * moves the window in. The page is laid out at the grown size, so the growth
 * already on screen is added back to find what the user's own size would
 * hide. While a change is in flight that growth is read off the viewport
 * rather than off what was last sent: the webview catches up with a resize a
 * frame late, and a sentence arriving mid-animation would otherwise be
 * measured against a size it has not reached.
 */
function refit(): void {
  // Hidden or minimized, there is no layout to measure; the next sentence
  // after it comes back measures again.
  if (document.hidden) return;
  const scale = window.devicePixelRatio || 1;
  const pinned = grower.base();
  const grownBy =
    pinned === null ? grower.applied() / scale : window.innerHeight - pinned.height / scale;
  const paneTop = panes.getBoundingClientRect().top;
  const contentBottom =
    output.getBoundingClientRect().bottom -
    paneTop +
    panes.scrollTop +
    Number.parseFloat(getComputedStyle(panes).paddingBottom || '0');
  // The room the window gives the pane, not `panes.clientHeight`: on Windows
  // the band leaves the grid flow, `.panes` lands in an `auto` row, and a short
  // sentence's pane is only as tall as the sentence — so the pane's own height
  // read the window as full and a grown window never shrank back.
  const room = window.innerHeight - paneTop;
  const hidden = contentBottom - (room - grownBy);
  const target = growDirection === 'off' ? 0 : growTarget(hidden, sentenceLinePitch(), growLines);
  const side = growDirection === 'off' ? grower.side() : growDirection;
  grower.want(target * scale, side, growAnimate);
}

/**
 * The chip whose segment contains character `offset`, or `null` when that is
 * an unmatched run or no chip covers it.
 *
 * Chips carry only their `start`, so the covering one is the last whose start
 * is at or before `offset` — which relies on `renderSentence` emitting them in
 * ascending order, as a segment cover is by construction. An unmatched run
 * returns `null` rather than itself: `focus()` on a `<span>` with no tabindex
 * is silently a no-op, and a caller needs to know to land somewhere else.
 */
function chipCovering(offset: number): HTMLElement | null {
  if (!Number.isFinite(offset)) return null;
  let covering: HTMLElement | null = null;
  for (const el of output.querySelectorAll<HTMLElement>('[data-start]')) {
    if (Number(el.dataset.start) > offset) break;
    covering = el;
  }
  return covering instanceof HTMLButtonElement ? covering : null;
}

function show(result: ParseResult): void {
  history.push(result);
  // Cleared here, not in `display`: a new parse always clears a standing
  // error, but navigating must not — the previous parse stays readable while
  // the user works out what went wrong (§6.2).
  parseError.replaceChildren();
  // `push` may have skipped `result` as a duplicate of the newest, so the
  // parse actually on screen has to be asked for rather than assumed.
  display(history.current());
}

historyBackButton?.addEventListener('click', () => display(history.back()));
historyForwardButton?.addEventListener('click', () => display(history.forward()));

/** Mirrors `layout::HISTORY_STEP_EVENT`. */
const HISTORY_STEP_EVENT = 'history-step';

// The global `Alt+[` / `Alt+]` pair (§7.9). Delegated to the same one-liner
// the chevrons use, so the disabled-state sync and the popover close in
// `display` come along for free — and `back()`/`forward()` return `null` at
// either end, which `display` already ignores.
void listen<number>(HISTORY_STEP_EVENT, (e) =>
  display(e.payload < 0 ? history.back() : history.forward()),
).catch(() => {
  // Same reasoning as the geometry listeners above: a listener that fails to
  // register leaves the keys inert, which is not worth taking the app down for.
});

// `listen()`'s returned promise resolves only once its IPC round trip has
// registered the handler on the Rust side. The clipboard poll's first tick
// waits on `frontend_ready` before it reads anything (see
// `clipboard::wait_for_frontend`), so firing it only after BOTH listeners
// are confirmed registered closes the race where clipboard text present at
// launch gets parsed and emitted before anything here is listening for it.
//
// A `listen()` failure above is deliberately NOT caught by anything here: it
// means parse events can never reach this webview at all (not merely that
// the poll's first tick might drop), which is not a condition to swallow —
// it surfaces as an unhandled rejection, exactly as it would have before
// this handshake existed. Only the `frontend_ready` call below gets its own,
// narrower catch, since that one really is limited to "the poll's first
// tick after launch might drop".
void Promise.all([
  listen<ParseResult>('parse-result', (e) => show(e.payload)),
  // A failure replaces only the message, never the result: the previous
  // parse stays readable while the user works out what went wrong.
  listen<string>('parse-error', (e) => parseError.replaceChildren(errorBlock(e.payload))),
  // Not rendered anywhere in this window — the settings window owns the hints
  // status line. Listened to for one reason: `emit_hints_status` is called only
  // from `set_mecab_hints`, so any event here means the user has engaged the
  // hints toggle themselves, and this screen must not re-answer a question they
  // are already answering. Condition 3 of `acquireHintsOnReady`.
  //
  // **Both flags, and the second one is the one that bites.** Clearing only
  // `acquireHintsOnReady` disarms the download in flight and leaves
  // `hintsNeverAcquired` stale, so a Retry after a failed index download re-arms
  // from it and overrides the answer the user just gave: turn hints on then off
  // mid-download, have the download fail, press Retry, and the `ready` switches
  // them back on. Once any hints event has been seen, the premise of condition 2
  // is gone for the rest of this window's life.
  listen<string>('hints-status', () => {
    acquireHintsOnReady = false;
    hintsNeverAcquired = false;
  }),
  listen<string>('dictionary-status', (e) => {
    renderDictionary(e.payload);
    if (e.payload !== 'ready' || !acquireHintsOnReady) return;
    acquireHintsOnReady = false;
    // Deliberately after `ready` has already cleared the screen, and
    // deliberately not awaited. The index is what makes the app usable and
    // hints only make it parse better, so holding the download screen up for
    // a second 7.7 MB fetch would delay the first thing the user came for.
    //
    // Failure is swallowed on purpose, and degrades to exactly today's
    // behaviour: `set_mecab_hints` persists the setting only on success, so a
    // fetch that fails leaves hints off, leaves `mecab_hints` false, and
    // leaves the toggle in the settings window for a second try. It reports
    // through `hints-status`, which this window listens to only to disarm the
    // flags above, and which the settings window renders. The alternative, an
    // error on the screen the user has just been told is ready, buys nothing
    // they can act on here.
    void invoke('set_mecab_hints', { enabled: true }).catch(() => {});
  }),
  listen<{ width: number; height: number }>('popover-measured', (e) => {
    // `pendingChip` is deliberately NOT cleared here — `closePopover` owns its
    // lifetime. Clearing it dropped the second of two interleaved
    // measurements: a fast Tab from A to B sets `pendingChip = B` before A's
    // measurement lands, so A's reply placed B at A's size and emptied the
    // slot, and B's own reply then arrived to nothing and was discarded,
    // leaving B wrong until the next hover. Leaving it set costs one extra
    // `place_popover` on that rare interleaving and re-places B correctly.
    const chip = pendingChip;
    // A chip removed by a parse that landed mid-round-trip has nothing to
    // anchor to; dropping the measurement is the correct outcome.
    if (chip === null || !chip.isConnected) return;
    // Fire-and-forget: a placement failure should leave the tooltip unshown,
    // not surface as an unhandled rejection over a single failed hover.
    void placeFor(chip, e.payload).catch(() => {});
  }),
]).then(() => {
  invoke('frontend_ready').catch(() => {
    // Not actionable by the user if this fails — same policy as a skipped
    // clipboard read (design §5). The only consequence is that the poll's
    // first tick after launch can drop; nothing else in the app depends on
    // this call succeeding.
  });
});

void showStartupError();
// The window is configured hidden (`tauri.conf.json`) and reveals itself here,
// for the reason `commands::create_settings_window` documents: a webview mapped
// before its page has loaded shows the webview's default white, and the theme
// lives in stylesheets that `main.ts` imports — so under the dev server they
// arrive as module requests after the document itself finishes loading.
//
// After `applySettings`, not before, so the restored toggle states are in the
// first painted frame rather than flipping into place afterwards. `finally`,
// because a settings read that fails must still leave a visible window.
//
// Not in a `requestAnimationFrame`: a window that is not visible is not being
// composited, so frame callbacks never run and it would stay hidden forever.
// Script evaluation is part of page load and does run, which is what this
// relies on.
void applySettings().finally(() => {
  void getCurrentWindow()
    .show()
    .catch(() => {});
});
void showSettingsWarning();
void showShortcutWarning();
void showHintsWarning();
void showDictionaryScreen();
