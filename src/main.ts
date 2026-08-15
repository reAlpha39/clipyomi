import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { cursorPosition, getCurrentWindow, monitorFromPoint } from '@tauri-apps/api/window';
import { renderSentence } from './render/sentence';
import { renderDefinitions } from './render/definitions';
import { MARGIN, placePopover, shouldKeep, type Point, type Rect } from './render/popover';
import type { ParseResult, Segment } from './types';
import './styles/global.css';

const app = document.querySelector<HTMLElement>('#app')!;

app.innerHTML = `
  <header class="controls">
    <button id="always-on-top" type="button" aria-pressed="false">Always on top</button>
    <button id="monitor" type="button" aria-pressed="true">Monitoring</button>
  </header>
  <div class="input-row">
    <input id="text" type="text" aria-label="Japanese text to parse" placeholder="Paste Japanese text" />
    <button id="parse">Parse</button>
  </div>
  <div id="parse-error"></div>
  <div class="panes"><div id="dictionary" role="status" tabindex="-1"></div><div id="output"></div></div>
`;

const output = app.querySelector<HTMLElement>('#output')!;
const input = app.querySelector<HTMLInputElement>('#text')!;
const parseButton = app.querySelector<HTMLButtonElement>('#parse')!;
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
  // unopenable index both mean `parse_text` cannot succeed. Disabling the
  // controls here is what stops that failure from ever reaching the user as
  // a raw Tauri "state not managed" string the moment they try to parse.
  output.replaceChildren(errorBlock(message));
  input.disabled = true;
  parseButton.disabled = true;
}

// A corrupt settings.json is cosmetic, not fatal: the app already fell back
// to defaults by the time this resolves, so — unlike `showStartupError`
// above — this must never disable a control and must never touch `output`.
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
    input.disabled = false;
    parseButton.disabled = false;
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
  // §3). Unlike the header toggles' `pending` (`bindToggle`, below), where
  // the same button really does stay in the DOM across the whole in-flight
  // request, this one is belt-and-braces behind an immediate re-render:
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
  // Parsing cannot succeed until an index exists. Unlike `showStartupError`'s
  // disabling, this is reversible — `ready` turns both back on.
  input.disabled = true;
  parseButton.disabled = true;
  renderDictionary(null);
}

interface Settings {
  always_on_top: boolean;
  clipboard_monitoring: boolean;
}

const alwaysOnTop = app.querySelector<HTMLButtonElement>('#always-on-top');
const monitor = app.querySelector<HTMLButtonElement>('#monitor');

// Populated with a button the instant it's clicked, before its request even
// starts: `applySettings` below skips writing to a button that's in this
// set, so a settings response that resolves after the user has already
// acted can never clobber what they just set. Per-button (not a single
// shared flag) so clicking one control doesn't also freeze its sibling out
// of ever receiving its real persisted value.
const touchedButtons = new Set<HTMLButtonElement>();

function bindToggle(
  button: HTMLButtonElement | null,
  command: string,
  settingsKey: keyof Settings,
): void {
  if (button === null) return;
  // Closure-local, not `button.disabled`: disabling a focused element blurs
  // it and drops it from the tab order, which a `finally` re-enable does not
  // undo. This guard gives the same exclusion — a click while one is
  // already in flight is a no-op — without touching the DOM or focus at all.
  let pending = false;
  button.addEventListener('click', () => {
    if (pending) return;
    pending = true;
    touchedButtons.add(button);
    const next = button.getAttribute('aria-pressed') !== 'true';
    // Flip first so the control feels immediate; a rejected command corrects it below.
    button.setAttribute('aria-pressed', String(next));
    void invoke(command, { enabled: next })
      .catch(async (e) => {
        // Rendered first and unconditionally: the user must see *some*
        // message, even if the resync below also fails. Doing this before
        // the `await` means it never depends on that second call succeeding.
        parseError.replaceChildren(errorBlock(String(e)));
        // A rejected setter doesn't say which of two things happened:
        // `state.rs`'s `SettingsState::update` applies a change in memory
        // *before* it tries to persist it, so a write failure (e.g. a
        // read-only config dir) still leaves the new value in effect
        // (design §5) — reverting to the naive inverse would then show the
        // opposite of what the backend actually did. Re-reading
        // `get_settings` shows whichever outcome really happened instead of
        // guessing from the shape of the error.
        try {
          const settings = await invoke<Settings>('get_settings');
          button.setAttribute('aria-pressed', String(settings[settingsKey]));
        } catch {
          // Best-effort only: if even this fails, the button keeps its
          // optimistic (possibly wrong) value, but the error above is
          // already visible — degrading silently here is better than an
          // unhandled rejection with no message shown at all.
        }
      })
      .finally(() => {
        pending = false;
      });
  });
}

bindToggle(alwaysOnTop, 'set_always_on_top', 'always_on_top');
bindToggle(monitor, 'set_clipboard_monitoring', 'clipboard_monitoring');

async function applySettings(): Promise<void> {
  const settings = await invoke<Settings>('get_settings');
  if (alwaysOnTop !== null && !touchedButtons.has(alwaysOnTop)) {
    alwaysOnTop.setAttribute('aria-pressed', String(settings.always_on_top));
  }
  if (monitor !== null && !touchedButtons.has(monitor)) {
    monitor.setAttribute('aria-pressed', String(settings.clipboard_monitoring));
  }
}

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
 * Centre of the tooltip as last placed, in **physical** screen px — the same
 * unit `cursorPosition()` reports, and the only unit `shouldKeep`'s distance
 * comparison can be safely measured against. Every other coordinate in this
 * file is CSS px; this one field deliberately is not — mixing the two here
 * makes a cursor moving toward the tooltip measure as moving away on any
 * display above 1x. `null` when closed.
 */
let tooltipCentre: Point | null = null;
/** Previous cursor sample, so the keep rule has something to compare against. */
let lastCursor: Point | null = null;
/** The keep-rule poll, or `undefined` when the tooltip is closed. */
let keepPoll: number | undefined;
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
  tooltipCentre = null;
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
 */
function startKeepPoll(): void {
  if (keepPoll !== undefined) return;
  keepPoll = window.setInterval(() => {
    if (tooltipCentre === null) return;
    void cursorPosition()
      .then((position) => {
        // A stale tick from a tooltip that closed while this read was in
        // flight must not resurrect `lastCursor` out from under whatever
        // opened next — checked first, before touching it.
        if (tooltipCentre === null) return;
        const next = { x: position.x, y: position.y };
        const previous = lastCursor;
        lastCursor = next;
        if (previous === null) return;
        // A resting cursor is not movement away, so it keeps the tooltip.
        if (previous.x === next.x && previous.y === next.y) return;
        if (!shouldKeep(previous, next, tooltipCentre)) closePopover();
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
  pendingChip = chip;
  // Fire-and-forget: a failed emit just leaves the tooltip unshown, no worse
  // than the user never having hovered at all.
  void emit('popover-content', entries).catch(() => {});
}

/**
 * Convert the chip's client rect to screen coordinates and place the window.
 *
 * `outerPosition` and the monitor are physical pixels; everything the DOM
 * reports is CSS pixels. Dividing by the scale factor before mixing them is
 * invisible on a 1x display and doubles every offset on a Retina one.
 */
async function placeFor(chip: HTMLElement, size: { width: number; height: number }): Promise<void> {
  // Captured once, re-checked after every await below: a dismissal that
  // lands mid-flight must stop this round trip from showing a tooltip
  // nothing can dismiss anymore, rather than racing it to the screen.
  const generation = openId;
  const current = getCurrentWindow();
  const [origin, scale] = await Promise.all([current.outerPosition(), current.scaleFactor()]);
  if (openId !== generation || !chip.isConnected) return;
  const box = chip.getBoundingClientRect();
  const left = origin.x / scale + box.left;
  const top = origin.y / scale + box.top;
  const rect: Rect = { left, top, right: left + box.width, bottom: top + box.height };

  // The monitor under the WORD, not the app's own — a window straddling two
  // screens must clamp against the one the user is looking at.
  const monitor = await monitorFromPoint(left * scale, top * scale);
  if (openId !== generation) return;
  if (monitor === null) return;
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
  // Physical px, matching `cursorPosition()` — see `tooltipCentre`'s own
  // declaration for why this one field isn't CSS px like everything above it.
  tooltipCentre = {
    x: (placed.left + size.width / 2) * scale,
    y: (placed.top + height / 2) * scale,
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
  // Re-armed per chip with no sticky swap: moving between chips hides the open
  // tooltip and starts a fresh dwell.
  closePopover();
  dwell = window.setTimeout(() => openFor(chip), DWELL_MS);
});

output.addEventListener('mouseout', (e) => {
  if (chipFrom(e.target) === null) return;
  // Only the pending dwell is cancelled here. An OPEN tooltip is left to the
  // keep rule — leaving the word *toward* the tooltip must not dismiss it,
  // which is the whole point of spec §3.2.
  clearDwell();
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

// The tooltip is placed from a rectangle all of these invalidate. `move` is
// new this phase: a DOM popover travelled with its parent for free, a separate
// window does not, and would be stranded on the desktop.
panes.addEventListener('scroll', closePopover);
// A bare `listen(event, handler)` defaults to `{ target: { kind: 'Any' } }`,
// which Tauri's window-event dispatch never matches against a `Window`-kind
// emit — left unscoped, both listeners below register but are never invoked,
// and the tooltip strands on the desktop the first time this window moves.
const windowTarget = { kind: 'Window', label: getCurrentWindow().label } as const;
void listen('tauri://move', closePopover, { target: windowTarget }).catch(() => {
  // Unlike the parse/dictionary listeners below, losing this one doesn't
  // break parsing — the tooltip just stops following window moves, a step
  // back to the old DOM-popover behaviour rather than a functional failure.
});
void listen('tauri://resize', closePopover, { target: windowTarget }).catch(() => {
  // Same reasoning as the `move` listener above.
});

function show(result: ParseResult): void {
  // Before anything is replaced: a popover left open would be anchored to a
  // chip from the previous sentence that is about to leave the document.
  closePopover();
  lastResult = result;

  const sentence = renderSentence(result);
  const definitions = renderDefinitions(result);

  // Delegated to the pane, not per-chip: chips are re-created on every
  // parse, but the sentence container itself is fresh each time too, so one
  // listener per parse is exactly right — nothing to leak, nothing to
  // rebind mid-life.
  sentence.addEventListener('click', (e) => {
    const chip = (e.target as HTMLElement).closest<HTMLElement>('[data-start]');
    if (chip === null) return;
    const row = definitions.querySelector(`.def-row[data-start="${chip.dataset.start}"]`);
    row?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    definitions.querySelectorAll('.marked').forEach((n) => n.classList.remove('marked'));
    row?.classList.add('marked');
  });

  parseError.replaceChildren();
  output.replaceChildren(sentence, definitions);
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
    const chip = pendingChip;
    pendingChip = null;
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

async function run(): Promise<void> {
  try {
    await invoke('set_input', { text: input.value });
  } catch (e) {
    parseError.replaceChildren(errorBlock(String(e)));
  }
}

parseButton.addEventListener('click', () => void run());
input.addEventListener('keydown', (e) => {
  if (e.key === 'Enter') void run();
});

void showStartupError();
void applySettings();
void showSettingsWarning();
void showDictionaryScreen();
