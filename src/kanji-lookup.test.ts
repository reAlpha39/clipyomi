// SPDX-License-Identifier: MIT OR Apache-2.0

import { describe, expect, test, vi } from 'vitest';
import { createKanjiLookup, distinctKanji } from './kanji-lookup';
import type { KanjiInfo } from './types';

const 東: KanjiInfo = { literal: '東', on: ['トウ'], kun: ['ひがし'], meanings: ['east'] };
const 京: KanjiInfo = { literal: '京', on: ['キョウ'], kun: ['みやこ'], meanings: ['capital'] };
const 都: KanjiInfo = { literal: '都', on: ['ト'], kun: ['みやこ'], meanings: ['metropolis'] };

describe('distinctKanji', () => {
  test('keeps first-seen order and drops repeats', () => {
    expect(distinctKanji('東京都')).toEqual(['東', '京', '都']);
    expect(distinctKanji('日日')).toEqual(['日']);
  });

  // The surface, not the headword (design §1.1): a PRONOUNCE match shows the
  // reader kana, and kana have no block.
  test('is empty for a kana-only surface', () => {
    expect(distinctKanji('はいた')).toEqual([]);
    expect(distinctKanji('')).toEqual([]);
  });
});

describe('createKanjiLookup', () => {
  test('returns records in surface order, whatever order the answer came in', async () => {
    const ask = vi.fn(() => Promise.resolve([都, 東, 京]));
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東京都')).toEqual([東, 京, 都]);
    expect(ask).toHaveBeenCalledWith('東京都');
  });

  test('asks once for two hovers of the same character', async () => {
    const ask = vi.fn(() => Promise.resolve([東]));
    const lookup = createKanjiLookup(ask);
    await lookup('東');
    expect(await lookup('東')).toEqual([東]);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  test('asks only for the characters it has not seen', async () => {
    const ask = vi.fn((chars: string) =>
      Promise.resolve([東, 京, 都].filter((k) => chars.includes(k.literal))),
    );
    const lookup = createKanjiLookup(ask);
    await lookup('東京');
    expect(await lookup('京都')).toEqual([京, 都]);
    expect(ask.mock.calls.map((c) => c[0])).toEqual(['東京', '都']);
  });

  test('remembers a miss so it is asked once', async () => {
    const ask = vi.fn(() => Promise.resolve([]));
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東')).toEqual([]);
    expect(await lookup('東')).toEqual([]);
    expect(ask).toHaveBeenCalledTimes(1);
  });

  test('does not ask at all for a kana-only surface', async () => {
    const ask = vi.fn(() => Promise.resolve([]));
    expect(await createKanjiLookup(ask)('はいた')).toEqual([]);
    expect(ask).not.toHaveBeenCalled();
  });

  // The e2e stub answers null for a command it does not name, and a
  // hand-edited backend could answer anything; the boundary validates.
  test('treats a non-array or malformed answer as no records', async () => {
    const lookup = createKanjiLookup(() => Promise.resolve([{ literal: 1 }, null, 東]));
    expect(await lookup('東京')).toEqual([東]);
    expect(await createKanjiLookup(() => Promise.resolve(null))('東')).toEqual([]);
  });

  // Phase 3b: the four misc fields cross the wire as a number or null, and a
  // payload from an older main webview may lack them altogether. A string in
  // one is a malformed record, like a numeric literal.
  test('accepts the misc fields absent, null or numeric, and rejects a string', async () => {
    const full: KanjiInfo = { ...東, grade: 2, strokes: 8, freq: 37, jlpt: 4 };
    const nulls: KanjiInfo = { ...京, grade: null, strokes: null, freq: null, jlpt: null };
    const bad = { ...都, grade: '3' };
    const lookup = createKanjiLookup(() => Promise.resolve([full, nulls, bad]));
    expect(await lookup('東京都')).toEqual([full, nulls]);
  });

  test('a failed round trip yields no records and is asked again next time', async () => {
    const ask = vi
      .fn<(chars: string) => Promise<unknown>>()
      .mockRejectedValueOnce(new Error('ipc down'))
      .mockResolvedValueOnce([東]);
    const lookup = createKanjiLookup(ask);
    expect(await lookup('東')).toEqual([]);
    expect(await lookup('東')).toEqual([東]);
    expect(ask).toHaveBeenCalledTimes(2);
  });
});
