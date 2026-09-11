// SPDX-License-Identifier: MIT OR Apache-2.0

import { isIdeograph } from './render/tooltip-colour';
import type { KanjiInfo } from './types';

/** The distinct kanji in `surface`, in first-seen order. */
export function distinctKanji(surface: string): string[] {
  const seen = new Set<string>();
  for (const ch of surface) {
    if (isIdeograph(ch)) seen.add(ch);
  }
  return [...seen];
}

/** Whether `value` is a `KanjiInfo` as `kanji_info` serialises one. */
function isKanjiInfo(value: unknown): value is KanjiInfo {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.literal === 'string' &&
    Array.isArray(v.on) &&
    Array.isArray(v.kun) &&
    Array.isArray(v.meanings)
  );
}

/**
 * A page-lifetime kanji lookup over `ask` — `invoke('kanji_info', { chars })`
 * in the app, stubbed in tests. Every character is asked once: a hit is
 * cached as its record and a miss as `null`, so a second hover on any
 * character anywhere in the session costs no round trip (design §5.1).
 *
 * A round trip that fails caches nothing and yields no records: the popover
 * opens without its block rather than not opening, and the next hover asks
 * again. Validated at the boundary because the e2e stub answers `null` for
 * this command and nothing else guarantees the shape.
 */
export function createKanjiLookup(
  ask: (chars: string) => Promise<unknown>,
): (surface: string) => Promise<KanjiInfo[]> {
  const cache = new Map<string, KanjiInfo | null>();
  return async (surface) => {
    const wanted = distinctKanji(surface);
    const misses = wanted.filter((ch) => !cache.has(ch));
    if (misses.length > 0) {
      try {
        const answer = await ask(misses.join(''));
        const records = Array.isArray(answer) ? answer.filter(isKanjiInfo) : [];
        for (const ch of misses) cache.set(ch, null);
        for (const record of records) cache.set(record.literal, record);
      } catch {
        // Deliberately nothing cached: see the doc comment above.
      }
    }
    return wanted.flatMap((ch) => {
      const hit = cache.get(ch);
      return hit ? [hit] : [];
    });
  };
}
