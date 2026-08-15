import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

// Split out of `main.test.ts` (Task 6 review, round 2) once the tooltip
// coverage pushed that file over the 800-line cap: everything that hovers,
// focuses, or measures a chip lives here instead. `vi.mock` is hoisted per
// module, so this file needs its own copy of the same IPC-boundary mocks
// `main.test.ts` declares — see that file's own comment for why both exist.
const listeners = new Map<string, (e: { payload: unknown }) => void>();
/** Every `emit` the app made, as [event, payload] pairs. */
const emitted: [string, unknown][] = [];
/** The `target` option each `listen()` call registered with, keyed by event. */
const listenTargets = new Map<string, unknown>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (
    event: string,
    handler: (e: { payload: unknown }) => void,
    options?: { target?: unknown },
  ) => {
    listeners.set(event, handler);
    listenTargets.set(event, options?.target);
    return Promise.resolve(() => listeners.delete(event));
  },
  emit: (event: string, payload: unknown) => {
    emitted.push([event, payload]);
    return Promise.resolve();
  },
}));

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke }));

/** Just the shape `placeFor` actually reads off a Tauri `Monitor`. */
interface MonitorStub {
  workArea: { position: { x: number; y: number }; size: { width: number; height: number } };
}

// Controllable stubs: this file is the only one that ever drives `placeFor`
// or the keep poll, so every one of these gets exercised (unlike the trivial
// stub `main.test.ts` keeps for its own, tooltip-free describes).
const outerPosition = vi.fn(() => Promise.resolve({ x: 0, y: 0 }));
const scaleFactor = vi.fn(() => Promise.resolve(1));
const cursorPositionMock = vi.fn(() => Promise.resolve({ x: 0, y: 0 }));
const monitorFromPointMock = vi.fn(
  (_x: number, _y: number): Promise<MonitorStub | null> => Promise.resolve(null),
);

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'main', outerPosition, scaleFactor }),
  cursorPosition: () => cursorPositionMock(),
  monitorFromPoint: (x: number, y: number) => monitorFromPointMock(x, y),
}));

function emit(event: string, payload: unknown): void {
  const handler = listeners.get(event);
  if (handler === undefined) throw new Error(`nothing listening for ${event}`);
  handler({ payload });
}

describe('the hover tooltip', () => {
  const SEGMENTS = {
    segments: [
      {
        start: 0,
        len: 2,
        surface: '東京',
        reading: 'とうきょう',
        matched: true,
        entries: [
          {
            headword: '東京',
            reading: 'とうきょう',
            conjugation: null,
            pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          },
        ],
      },
    ],
  };

  // Three separate chips (distinct `start`s): a single-chip fixture cannot
  // distinguish "re-armed per chip" from "just works because there's only
  // one chip to hover" — the sweep problem the dwell exists to solve.
  const SWEEP_SEGMENTS = {
    segments: [0, 2, 4].map((start) => ({
      start,
      len: 2,
      surface: '東京',
      reading: 'とうきょう',
      matched: true,
      entries: [
        {
          headword: '東京',
          reading: 'とうきょう',
          conjugation: null,
          pos: ['n'],
          senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
          flags: ['primary'],
        },
      ],
    })),
  };

  function chip(): HTMLButtonElement {
    const el = document.querySelector<HTMLButtonElement>('.chip');
    if (el === null) throw new Error('.chip missing');
    return el;
  }

  /** Names of the commands invoked so far, in order. */
  function calls(): string[] {
    return invoke.mock.calls.map((c) => c[0] as string);
  }

  /** Names of the events emitted so far, in order. */
  function events(): string[] {
    return emitted.map((e) => e[0]);
  }

  beforeEach(async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    vi.resetModules();
    await import('./main');
    emit('parse-result', SEGMENTS);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // The dwell is what stops the tooltip firing for every chip the cursor
  // sweeps across on its way somewhere else.
  test('a completed dwell sends the content', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    expect(events()).not.toContain('popover-content');
    vi.advanceTimersByTime(350);
    expect(emitted).toContainEqual(['popover-content', SEGMENTS.segments[0].entries]);
  });

  // Restores the coverage deleted with 2G's DOM-popover specs in Task 7: an
  // unmatched run is a plain `<span>` with no `chip` class, so `chipFrom`'s
  // `.closest('.chip')` returns null for it and no dwell is ever armed. An
  // empty tooltip for a span with no entries is the one wrong outcome
  // available here.
  test('an unmatched run opens no popover', () => {
    emit('parse-result', {
      segments: [
        { start: 0, len: 2, surface: '𠮟る', reading: null, matched: false, entries: [] },
      ],
    });
    const span = document.querySelector<HTMLElement>('.unmatched');
    if (span === null) throw new Error('.unmatched missing');
    span.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    vi.advanceTimersByTime(350);
    expect(events()).not.toContain('popover-content');
  });

  test('a cursor that leaves before the dwell completes sends nothing', () => {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    vi.advanceTimersByTime(200);
    chip().dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
    vi.advanceTimersByTime(500);
    expect(events()).not.toContain('popover-content');
  });

  // Focus moves only on a deliberate keypress, so it has no sweeping problem
  // for a dwell to solve.
  test('focus sends the content immediately, with no timer', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    expect(events()).toContain('popover-content');
  });

  // Escape must not move focus: the user is mid-sentence and would otherwise
  // have to Tab back in from the start.
  test('Escape hides it and leaves focus on the chip', () => {
    const target = chip();
    target.focus();
    target.dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    expect(calls()).toContain('hide_popover');
    expect(document.activeElement).toBe(target);
  });

  // `show()` replaces `#output` wholesale, so a tooltip left open would be
  // anchored to a chip that is no longer in the document.
  test('a new parse-result hides it', () => {
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('parse-result', SEGMENTS);
    expect(calls()).toContain('hide_popover');
  });

  // New this phase: a separate window does not travel with its parent, so a
  // moved main window would strand it on the desktop. Also pins the fix for
  // Task 6 review's Critical 1: a bare `listen()` targets `{ kind: 'Any' }`,
  // which Tauri's window-event dispatch never matches — the registration
  // itself must carry this window's own label, or the handler never fires.
  test('moving the main window hides it', () => {
    expect(listenTargets.get('tauri://move')).toEqual({ kind: 'Window', label: 'main' });
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('tauri://move', {});
    expect(calls()).toContain('hide_popover');
  });

  test('resizing the main window hides it', () => {
    expect(listenTargets.get('tauri://resize')).toEqual({ kind: 'Window', label: 'main' });
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    invoke.mockClear();
    emit('tauri://resize', {});
    expect(calls()).toContain('hide_popover');
  });

  // Design §3.1: "re-armed per chip, with no sticky swap" — the rule the
  // dwell was introduced to enforce, and the one the single-chip "leaves
  // before the dwell completes" test above cannot exercise, since there is
  // nothing to re-arm against with only one chip in play. Each `mouseover`
  // below is well inside the previous chip's 350ms dwell, so a sweep that
  // sent content for every chip touched — instead of re-arming and
  // cancelling — would leave `popover-content` among the events at the end.
  test('a sweep across several chips sends nothing, including from the last one it touched', () => {
    emit('parse-result', SWEEP_SEGMENTS);
    const chips = Array.from(document.querySelectorAll<HTMLButtonElement>('.chip'));
    expect(chips).toHaveLength(3);

    for (const c of chips) {
      c.dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
      vi.advanceTimersByTime(20);
    }
    // The cursor kept moving past the last chip too, not stopping on it.
    chips[chips.length - 1].dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));

    // Past DWELL_MS for every chip touched, including the last.
    vi.advanceTimersByTime(350);
    expect(events()).not.toContain('popover-content');
  });
});

// Closes a coverage gap: no other describe in this file ever reaches
// `placeFor` — the whole measure -> place -> show half of the tooltip
// architecture was unexercised, which is exactly why a physical/CSS pixel
// mixup could sit in it undetected (see `tooltipCentre`'s own comment in
// `main.ts`). Kept small on purpose — `popover.test.ts` already owns
// `placePopover`'s arithmetic; these tests only pin the wiring around it.
describe('the tooltip round trip', () => {
  const SEGMENTS = {
    segments: [
      {
        start: 0,
        len: 2,
        surface: '東京',
        reading: 'とうきょう',
        matched: true,
        entries: [
          {
            headword: '東京',
            reading: 'とうきょう',
            conjugation: null,
            pos: ['n'],
            senses: [{ pos: ['n'], glosses: ['Tokyo'], xrefs: [], misc: [], info: [] }],
            flags: ['primary'],
          },
        ],
      },
    ],
  };

  function chip(): HTMLButtonElement {
    const el = document.querySelector<HTMLButtonElement>('.chip');
    if (el === null) throw new Error('.chip missing');
    return el;
  }

  /**
   * Flush the awaits inside `placeFor`: the `outerPosition`/`scaleFactor`
   * `Promise.all`, the `monitorFromPoint` read, and the `place_popover`
   * invoke, plus the `.then` chaining between them.
   */
  async function flushPlacement(): Promise<void> {
    for (let i = 0; i < 6; i += 1) await Promise.resolve();
  }

  /**
   * Open and place the tooltip by HOVER, leaving the cursor on the chip.
   *
   * The mouse path, not the focus one: spec §3.3's keep rule only starts
   * judging once the cursor has left the chip, so a focus-opened tooltip
   * never arms the poll at all and could not exercise it.
   */
  async function openByHover(): Promise<void> {
    chip().dispatchEvent(new MouseEvent('mouseover', { bubbles: true }));
    vi.advanceTimersByTime(350);
    emit('popover-measured', { width: 200, height: 60 });
    await flushPlacement();
  }

  /** The cursor has left the word — this is what arms the keep rule. */
  function leaveChip(): void {
    chip().dispatchEvent(new MouseEvent('mouseout', { bubbles: true }));
  }

  /** Feed the keep poll one cursor sample, in physical px, and let its tick settle. */
  async function sample(x: number, y: number): Promise<void> {
    cursorPositionMock.mockResolvedValueOnce({ x, y });
    vi.advanceTimersByTime(60);
    await Promise.resolve();
    await Promise.resolve();
  }

  beforeEach(async () => {
    // Fake timers purely so `startKeepPoll`'s real `setInterval` (armed once
    // the cursor leaves the chip, below) can't keep firing with real timers in
    // the background after this test ends.
    vi.useFakeTimers();
    document.body.innerHTML = '<main id="app"></main>';
    listeners.clear();
    emitted.length = 0;
    // These two are the only window stubs any test overrides; resetting them
    // restores the implementation `vi.fn(impl)` was given AND drops any
    // `…Once` value a test queued but never consumed, which would otherwise
    // leak into whichever test ran next.
    cursorPositionMock.mockReset();
    scaleFactor.mockReset();
    invoke.mockReset();
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      return Promise.resolve(null);
    });
    // A 1000x800 CSS-px work area at the origin (2000x1600 physical, at the
    // scale of 2 the tests below pin).
    monitorFromPointMock.mockResolvedValue({
      workArea: { position: { x: 0, y: 0 }, size: { width: 2000, height: 1600 } },
    });
    vi.resetModules();
    await import('./main');
    emit('parse-result', SEGMENTS);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // Scale is pinned at 2, not 1: at 1x, a `placeFor` that forgot to convert
  // physical <-> CSS px anywhere produces the same numbers as one that
  // didn't, so the test would pass either way. This is the load-bearing
  // choice that actually exercises the conversion.
  test('measuring a chip places the popover with rounded, scale-corrected coordinates', async () => {
    scaleFactor.mockResolvedValueOnce(2);
    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    emit('popover-measured', { width: 200, height: 60 });

    // Flushes the `Promise.all` for outerPosition/scaleFactor, the
    // `monitorFromPoint` await, and the `place_popover` invoke in turn.
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    // happy-dom's `getBoundingClientRect()` is all zeros, so the chip sits at
    // the window origin: below-the-chip placement (top=2) clamps to the
    // 8px margin on both axes, landing at (8,8).
    expect(invoke).toHaveBeenCalledWith('place_popover', { x: 8, y: 8, width: 200, height: 60 });
  });

  // Task 6 review round 2, Important 3: this test's own assertion above is
  // scale-invariant (nothing in it ever reads `tooltipCentre`), so it would
  // still pass against a CSS-px centre. The keep poll is the one thing that
  // actually consumes `tooltipCentre` — this test drives it with two
  // physical-px cursor samples chosen so a CSS-px centre and the correct
  // physical-px one give opposite keep/dismiss verdicts, which is the only
  // way to make the unit choice itself observable.
  //
  // At scale 2 the tooltip lands at (8,8)/200x60 CSS px (see the sibling
  // test), so its rect is (16,16)-(416,136) physical with centre (216,76),
  // against (8,8)-(208,68) with centre (108,38) had it been left in CSS px.
  // The samples below run along y=300 — outside BOTH rects, so neither
  // version short-circuits on the inside-the-tooltip rule — and move LEFT,
  // from x=162 to x=100. That crosses between the two candidate centres:
  // distance to the physical centre grows (~230 -> ~252, a dismiss) while
  // distance to the CSS one shrinks (~268 -> ~262, a keep). The two units
  // disagree on the same movement, which is the only way to make the unit
  // choice itself observable.
  //
  // Opened by hover rather than by focus since the whole-branch review's C1:
  // the keep rule is gated on the cursor having left the chip, so a
  // focus-opened tooltip never arms the poll. The comparison being asserted
  // is unchanged.
  test('the keep poll compares against the physical-px centre, not a CSS-px one', async () => {
    scaleFactor.mockResolvedValueOnce(2);
    await openByHover();
    leaveChip();
    invoke.mockClear();

    // First sample: no prior cursor position to compare against, so this
    // only sets the baseline — no verdict yet.
    await sample(162, 300);
    // Second sample: the real comparison. Physical-px code dismisses; a
    // CSS-px regression would keep it instead.
    await sample(100, 300);

    expect(invoke.mock.calls.map((c) => c[0])).toContain('hide_popover');
  });

  // Whole-branch review C1. At the default scale of 1 the tooltip's rect is
  // (8,8)-(208,68) with centre (108,38). Reaching for the scrollbar at its
  // right edge — (110,40) to (200,66), both inside, distance to the centre
  // growing from ~3 to ~96 — is movement away by every measure the keep rule
  // has, and dismissing there closes the tooltip mid-scroll. A cursor inside
  // the tooltip is reading it, and is never judged.
  test('a cursor inside the tooltip keeps it, even moving away from its centre', async () => {
    await openByHover();
    leaveChip();
    invoke.mockClear();

    await sample(110, 40);
    await sample(200, 66);

    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('hide_popover');
  });

  // Spec §3.3's second row, which the shipped code could not express at all
  // from a centre point. The move below LOWERS the distance to the centre
  // (~99 -> ~78), so the direction-of-travel rule alone would keep it — only
  // "was inside, now outside" dismisses here.
  test('a cursor that was inside the tooltip and leaves it dismisses it', async () => {
    await openByHover();
    leaveChip();
    invoke.mockClear();

    await sample(12, 60); // inside, near the left edge
    await sample(60, 100); // outside: below the rect's bottom of 68

    expect(invoke.mock.calls.map((c) => c[0])).toContain('hide_popover');
  });

  // Spec §3.3's first row is a conjunction: "cursor leaves the chip AND the
  // keep rule fails". Armed from `place_popover` instead, a two-pixel drift
  // of a hand resting on the word dismissed the tooltip 60ms after it opened
  // — and it could not be reopened without leaving and re-entering the chip,
  // since `mouseover` does not re-fire inside the same element. The samples
  // below move away from the centre, so they would dismiss if the poll were
  // judging at all.
  test('movement while the cursor is still on the chip dismisses nothing', async () => {
    await openByHover();
    invoke.mockClear();

    await sample(108, 300);
    await sample(108, 400);

    expect(invoke.mock.calls.map((c) => c[0])).not.toContain('hide_popover');
  });

  // Task 6 review round 2, Important 1: during the round trip `pendingChip`
  // is already `null` (cleared before `placeFor` is even called) and
  // `keepPoll` isn't armed until after `place_popover` resolves, so
  // `closePopover`'s `wasActive` gate (finding 7) sees nothing to hide and
  // skips the IPC call for the whole flight. If `place_popover` is still in
  // flight when a dismissal lands, `placeFor` itself must send the
  // `hide_popover` nothing else will, once it notices its own generation
  // went stale.
  test('a dismissal while place_popover is in flight still hides the tooltip once it resolves', async () => {
    let resolvePlace: (() => void) | undefined;
    invoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_settings') {
        return Promise.resolve({ always_on_top: false, clipboard_monitoring: true });
      }
      if (cmd === 'place_popover') {
        return new Promise<void>((resolve) => {
          resolvePlace = resolve;
        });
      }
      return Promise.resolve(null);
    });

    chip().dispatchEvent(new FocusEvent('focusin', { bubbles: true }));
    emit('popover-measured', { width: 200, height: 60 });
    // Flushes the outerPosition/scaleFactor Promise.all and the
    // monitorFromPoint await — enough for `place_popover` to have been
    // dispatched (and `resolvePlace` captured), but it deliberately never
    // resolves on its own.
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(resolvePlace).toBeDefined();

    invoke.mockClear();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

    resolvePlace?.();
    await Promise.resolve();
    await Promise.resolve();

    expect(invoke.mock.calls.map((c) => c[0])).toContain('hide_popover');
  });
});
