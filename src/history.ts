import type { ParseResult } from './types';

/**
 * How many parses the session keeps (§7.8).
 *
 * Full `ParseResult`s rather than the input text: navigation is instant, and a
 * dictionary rebuild mid-session cannot change what an old step renders as.
 */
export const HISTORY_CAP = 50;

export interface History {
  /**
   * Records a parse and makes it current, unless it repeats the newest one.
   *
   * The cursor always lands on the newest parse afterwards, and the parses
   * ahead of it survive — history is a timeline of everything copied this
   * session, not a browsing tree, so nothing the user copied is ever dropped
   * to make room for a newer branch.
   */
  push(result: ParseResult): void;
  /**
   * The parse now on screen, or `null` before the first one arrives.
   *
   * `push` skips a duplicate, so the caller cannot assume the parse it just
   * pushed became current — it has to ask.
   */
  current(): ParseResult | null;
  /** The previous parse, or `null` when already on the oldest. */
  back(): ParseResult | null;
  /** The next parse, or `null` when already on the newest. */
  forward(): ParseResult | null;
  canBack(): boolean;
  canForward(): boolean;
}

/** The dedup key: what the user copied, independent of how it segmented. */
function surfaceText(result: ParseResult): string {
  return result.segments.map((segment) => segment.surface).join('');
}

/**
 * A session's parse history. A factory rather than module state so each test —
 * and each webview — gets its own, with no reset to remember.
 */
export function createHistory(): History {
  const buffer: ParseResult[] = [];
  let cursor = -1;

  const canBack = (): boolean => cursor > 0;
  const canForward = (): boolean => cursor < buffer.length - 1;

  return {
    push(result: ParseResult): void {
      const newest = buffer[buffer.length - 1];
      // Same copied text, so this is not a new step in the chain — but it can
      // be a new *reading* of that text: 5B's hints toggle re-parses what is
      // already on screen, and the re-segmentation joins back to an identical
      // surface. Dropping it outright left the stale segmentation on screen
      // and made the toggle look inert, so the newest entry is replaced
      // instead. One entry per copied text either way.
      //
      // The cursor is left alone on purpose, exactly as skipping did: no step
      // was added, so a user reading an older parse is not yanked forward.
      if (newest !== undefined && surfaceText(newest) === surfaceText(result)) {
        buffer[buffer.length - 1] = result;
        return;
      }

      buffer.push(result);
      if (buffer.length > HISTORY_CAP) buffer.shift();
      // Set from the length, not incremented: a shift above moves every index
      // down by one, and this is what keeps the cursor off a stale slot.
      cursor = buffer.length - 1;
    },

    current(): ParseResult | null {
      return cursor < 0 ? null : buffer[cursor];
    },

    back(): ParseResult | null {
      if (!canBack()) return null;
      cursor -= 1;
      return buffer[cursor];
    },

    forward(): ParseResult | null {
      if (!canForward()) return null;
      cursor += 1;
      return buffer[cursor];
    },

    canBack,
    canForward,
  };
}
