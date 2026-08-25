import { describe, expect, test } from 'vitest';
import { HISTORY_CAP, createHistory } from './history';
import type { ParseResult } from './types';

/** A parse whose only distinguishing feature is its surface text. */
function parse(...surfaces: string[]): ParseResult {
  let start = 0;
  return {
    segments: surfaces.map((surface) => {
      const segment = {
        start,
        len: surface.length,
        surface,
        reading: null,
        matched: true,
        entries: [],
      };
      start += surface.length;
      return segment;
    }),
  };
}

/** The surface text of a parse, or `null` for no parse at all. */
function text(result: ParseResult | null): string | null {
  return result === null ? null : result.segments.map((s) => s.surface).join('');
}

describe('createHistory', () => {
  test('offers no navigation when empty', () => {
    const history = createHistory();

    expect(history.canBack()).toBe(false);
    expect(history.canForward()).toBe(false);
    expect(history.back()).toBeNull();
    expect(history.forward()).toBeNull();
  });

  test('offers no navigation with a single parse', () => {
    const history = createHistory();
    history.push(parse('猫'));

    expect(history.canBack()).toBe(false);
    expect(history.canForward()).toBe(false);
  });

  test('has no current parse before the first one arrives', () => {
    expect(createHistory().current()).toBeNull();
  });

  test('makes a pushed parse current', () => {
    const history = createHistory();
    history.push(parse('猫'));

    expect(text(history.current())).toBe('猫');
  });

  // What `show` relies on to avoid re-rendering the newest parse over the older
  // one the user is reading.
  test('leaves the current parse alone when a duplicate is skipped', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));
    history.back();

    history.push(parse('犬'));

    expect(text(history.current())).toBe('猫');
  });

  test('follows the cursor as it moves', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));

    expect(text(history.current())).toBe('犬');
    history.back();
    expect(text(history.current())).toBe('猫');
    history.forward();
    expect(text(history.current())).toBe('犬');
  });

  test('walks back and forward between two parses', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));

    expect(history.canBack()).toBe(true);
    expect(history.canForward()).toBe(false);

    expect(text(history.back())).toBe('猫');
    expect(history.canBack()).toBe(false);
    expect(history.canForward()).toBe(true);

    expect(text(history.forward())).toBe('犬');
    expect(history.canForward()).toBe(false);
  });

  test('stays put at the oldest parse', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));
    history.back();

    expect(history.back()).toBeNull();
    // Still on the oldest, not pushed off the end of the buffer.
    expect(text(history.forward())).toBe('犬');
  });

  test('stays put at the newest parse', () => {
    const history = createHistory();
    history.push(parse('猫'));

    expect(history.forward()).toBeNull();
    expect(history.canBack()).toBe(false);
  });

  // §7.8. The clipboard worker already skips text it has just seen, so a
  // consecutive duplicate reaching here means the same parse arrived twice by
  // another route — storing it would put two identical steps in the chain.
  test('does not store a parse identical to the newest', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('猫'));

    expect(history.canBack()).toBe(false);
  });

  test('stores a repeat that is not consecutive', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));
    history.push(parse('猫'));

    expect(text(history.back())).toBe('犬');
    expect(text(history.back())).toBe('猫');
  });

  // The dedup key is the surface text, so a parse is a duplicate of the newest
  // however its segments happen to be shaped.
  test('compares whole surface text, not segment boundaries', () => {
    const history = createHistory();
    history.push(parse('猫', 'が'));
    history.push(parse('猫が'));

    expect(history.canBack()).toBe(false);
  });

  test('skips a duplicate of the newest without moving the cursor', () => {
    const history = createHistory();
    history.push(parse('猫'));
    history.push(parse('犬'));
    history.back();

    history.push(parse('犬'));

    // Nothing new arrived, so the view should not have been yanked forward.
    expect(history.canForward()).toBe(true);
    expect(text(history.forward())).toBe('犬');
  });

  // Nav model A: a new parse always becomes the current one, and the parses
  // ahead of the cursor survive. Nothing the user copied disappears.
  test('jumps to the newest parse and keeps the ones ahead of the cursor', () => {
    const history = createHistory();
    history.push(parse('A'));
    history.push(parse('B'));
    history.push(parse('C'));
    history.back();
    history.back();

    history.push(parse('D'));

    expect(history.canForward()).toBe(false);
    expect(text(history.back())).toBe('C');
    expect(text(history.back())).toBe('B');
    expect(text(history.back())).toBe('A');
    expect(history.canBack()).toBe(false);
  });

  test(`holds ${HISTORY_CAP} parses and evicts the oldest`, () => {
    const history = createHistory();
    for (let i = 0; i < HISTORY_CAP + 1; i += 1) history.push(parse(`s${i}`));

    let steps = 0;
    while (history.canBack()) {
      history.back();
      steps += 1;
    }

    expect(steps).toBe(HISTORY_CAP - 1);
    // s0 fell off the front; s1 is now the oldest reachable parse.
    expect(text(history.forward())).toBe('s2');
    history.back();
    expect(history.canBack()).toBe(false);
  });

  // Eviction shifts every index down by one. Because a push always lands the
  // cursor on the newest parse, that shift can never leave it stale.
  test('keeps the cursor on the newest parse across an eviction', () => {
    const history = createHistory();
    for (let i = 0; i < HISTORY_CAP + 5; i += 1) history.push(parse(`s${i}`));

    expect(history.canForward()).toBe(false);
    expect(text(history.back())).toBe(`s${HISTORY_CAP + 3}`);
  });
});
