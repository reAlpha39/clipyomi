import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { availableMonitors, cursorPosition, getCurrentWindow } from '@tauri-apps/api/window';
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
import type { FuriganaMode, GlossFilters, ParseResult, Segment, Settings } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <header class="controls" data-tauri-drag-region>
    <button id="settings-toggle" type="button" title="Settings">⚙</button>
    <div id="window-controls" class="window-controls">
      <button id="window-minimize" type="button" title="Minimize" tabindex="-1">─</button>
      <button id="window-maximize" type="button" title="Maximize" tabindex="-1">□</button>
      <button id="window-close" type="button" title="Close" tabindex="-1">✕</button>
    </div>
  </header>
  <div id="parse-error"></div>
  <div class="panes"><div id="dictionary" role="status" tabindex="-1"></div><div id="output"></div></div>
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
  parseError.replaceChildren(errorBlock(message));
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
      ? 'No dictionary yet. JMdict is a one-time download of roughly ten megabytes from EDRDG.'
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
};

const settingsToggle = app.querySelector<HTMLButtonElement>('#settings-toggle');
const windowMinimize = app.querySelector<HTMLButtonElement>('#window-minimize');
const windowMaximize = app.querySelector<HTMLButtonElement>('#window-maximize');
const windowClose = app.querySelector<HTMLButtonElement>('#window-close');
const headerControls = app.querySelector<HTMLElement>('header.controls');

settingsToggle?.addEventListener('click', () => {
  void invoke('open_settings_window').catch(() => {});
});

windowMinimize?.addEventListener('click', () => {
  void invoke('minimize_window').catch(() => {});
});

windowMaximize?.addEventListener('click', () => {
  void invoke('toggle_maximize_window').catch(() => {});
});

windowClose?.addEventListener('click', () => {
  void invoke('close_window').catch(() => {});
});

headerControls?.addEventListener('dblclick', (e) => {
  if (e.target === headerControls) {
    void invoke('toggle_maximize_window').catch(() => {});
  }
});

/**
 * Whether the OS is drawing its title bar. The control for it lives in the
 * settings window; the main window still needs the flag, because it decides
 * both shapes of the band — a reserved 28px row when the title bar is shown,
 * a floating overlay over the sentence when it is hidden — and the CSS keys
 * every one of those rules off `#app[data-decorations]`.
 */
let titlebarShown = true;

/**
 * True while a hover is showing chrome that `decorations: false` normally
 * hides. Tracked rather than derived from the event, so only *crossings* reach
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
 * absorb the revealed band (macOS). Queried once at startup so `showPeek`
 * stays synchronous.
 */
let peekGrowsFrame = false;

/** Pending un-peek, or `undefined` when none is armed. */
let peekHide: number | undefined;

function showPeek(): void {
  // A re-entry inside the grace period cancels the pending hide outright, so a
  // trip across a traffic light costs no backend calls at all.
  window.clearTimeout(peekHide);
  peekHide = undefined;
  if (titlebarShown || peeking) return;
  peeking = true;
  // `peeked` expands the band and fades the gear in on every platform.
  // `peek-offset` adds `padding-top` only when the OS frame grows upward to
  // absorb it (macOS), keeping the content stationary on screen.
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

function applyDecorations(shown: boolean): void {
  titlebarShown = shown;
  app.setAttribute('data-decorations', String(shown));
  // A peek still notionally in flight when the user turns the title bar back
  // ON is now redundant — `set_decorations` has shown the chrome for real. The
  // flag has to drop here or the `pointerleave` that follows would hide the
  // chrome the user just asked to keep.
  if (shown) {
    window.clearTimeout(peekHide);
    peekHide = undefined;
    peeking = false;
    app.classList.remove('peeked', 'peek-offset');
  }
}

applyDecorations(true);

// The cursor anywhere inside the window reveals the bar; only leaving the
// window hides it. `#app` fills the viewport, so its own enter/leave is the
// window's — and unlike the 6px band it cannot be missed by a fast pointer.
app.addEventListener('pointerenter', showPeek);
app.addEventListener('pointermove', showPeek);
app.addEventListener('pointerleave', hidePeek);

// Keyboard parity: the gear is a tab stop even while invisible, and only the
// backend can grow the frame, so focus has to drive the same path as hover
// rather than leaning on a CSS `:focus-within` rule.
settingsToggle?.addEventListener('focus', showPeek);
settingsToggle?.addEventListener('blur', hidePeek);

async function applySettings(): Promise<void> {
  const isMac = await invoke<boolean>('is_macos').catch(() => false);
  peekGrowsFrame = await invoke<boolean>('peek_grows_frame').catch(() => false);
  app.setAttribute('data-platform', isMac ? 'macos' : 'windows');
  const settings = await invoke<Settings>('get_settings');
  if (settings.decorations !== undefined) {
    applyDecorations(settings.decorations);
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
}

void listen<Settings>('settings-changed', (e) => {
  const settings = e.payload;
  if (settings.hide_pos !== undefined) currentFilters.hide_pos = settings.hide_pos;
  if (settings.hide_xrefs !== undefined) currentFilters.hide_xrefs = settings.hide_xrefs;
  if (settings.hide_usage !== undefined) currentFilters.hide_usage = settings.hide_usage;
  if (settings.decorations !== undefined) applyDecorations(settings.decorations);
  if (settings.furigana_mode !== undefined && settings.furigana_mode !== currentFuriganaMode) {
    currentFuriganaMode = settings.furigana_mode;
    if (lastResult !== null) {
      closePopover();
      output.replaceChildren(renderSentence(lastResult, currentFuriganaMode));
    }
  }
});

/** Milliseconds the cursor must rest on a chip before its popover opens. */
const DWELL_MS = 350;

/**
 * The most recent parse, kept so a hover can find the entry for the chip under
 * the cursor. The chips carry only `data-start`; the entries live here.
 */
let lastResult: ParseResult | null = null;

/** Pending dwell timer, or `undefined` when none is armed. */
let dwell: number | undefined;

function segmentAt(start: string | undefined): Segment | undefined {
  if (start === undefined || lastResult === null) return undefined;
  return lastResult.segments.find((segment) => String(segment.start) === start);
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
 * is still on the word. ta-old holds the same shape: `KeepToolTip` is only a
 * flag, and the word window consumes it on mouse-leave
 * (`MyToolTip.cpp:333-360`), so its mouse hook never hides a tooltip whose word
 * is still under the cursor. Set by the chip's `mouseout`, which also means a
 * tooltip opened by FOCUS never arms the poll at all — there is no cursor
 * gesture to judge, and unrelated mouse movement elsewhere on screen must not
 * dismiss a tooltip the keyboard opened.
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

/** Ask the popup to render CHIP's entries, if the last parse still knows that span. */
function openFor(chip: HTMLElement): void {
  const entries = segmentAt(chip.dataset.start)?.entries;
  // No entries means a stale chip from a superseded parse, or an unmatched
  // run — neither is an error worth surfacing.
  if (entries === undefined || entries.length === 0) return;
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
  pendingChip = chip;
  // Fire-and-forget: a failed emit just leaves the tooltip unshown, no worse
  // than the user never having hovered at all.
  void emit('popover-content', { entries, filters: currentFilters }).catch(() => {});
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

  // Never taller than the work area allows: past that it scrolls, as ta-old
  // does, rather than being placed off-screen.
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
    try {
      const win = getCurrentWindow();
      const size = await win.innerSize();
      const pos = await win.outerPosition();
      const factor = await win.scaleFactor();
      const logicalSize = size.toLogical(factor);
      const logicalPos = pos.toLogical(factor);
      void invoke('save_window_geometry', {
        width: Math.round(logicalSize.width),
        height: Math.round(logicalSize.height),
        x: Math.round(logicalPos.x),
        y: Math.round(logicalPos.y),
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

function show(result: ParseResult): void {
  // Before anything is replaced: a popover left open would be anchored to a
  // chip from the previous sentence that is about to leave the document.
  closePopover();
  lastResult = result;

  const sentence = renderSentence(result, currentFuriganaMode);

  parseError.replaceChildren();
  output.replaceChildren(sentence);
}

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
  listen<string>('dictionary-status', (e) => renderDictionary(e.payload)),
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
void showDictionaryScreen();
