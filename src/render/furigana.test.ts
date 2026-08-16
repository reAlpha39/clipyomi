import { describe, expect, test } from 'vitest';
import { furiganaFor, toKatakana, toRomaji } from './furigana';
import type { Segment } from '../types';

describe('toKatakana', () => {
  test('converts basic hiragana to katakana', () => {
    expect(toKatakana('とうきょう')).toBe('トウキョウ');
    expect(toKatakana('あいうえお')).toBe('アイウエオ');
    expect(toKatakana('きゃきゅきょ')).toBe('キャキュキョ');
  });

  test('passes non-hiragana characters untouched', () => {
    expect(toKatakana('東京123!')).toBe('東京123!');
    expect(toKatakana('テスト')).toBe('テスト');
  });
});

describe('toRomaji', () => {
  test('converts basic kana to romaji', () => {
    expect(toRomaji('とうきょう')).toBe('toukyou');
    expect(toRomaji('にほん')).toBe('nihon');
    expect(toRomaji('すし')).toBe('sushi');
  });

  test('handles digraphs', () => {
    expect(toRomaji('きょうと')).toBe('kyouto');
    expect(toRomaji('しゃしん')).toBe('shashin');
    expect(toRomaji('おちゃ')).toBe('ocha');
    expect(toRomaji('ちょっと')).toBe('chotto');
  });

  test('handles sokuon double consonants', () => {
    expect(toRomaji('いった')).toBe('itta');
    expect(toRomaji('やっぱり')).toBe('yappari');
    expect(toRomaji('がっこう')).toBe('gakkou');
  });

  test('handles syllabic n with apostrophe before vowels and ya-row', () => {
    expect(toRomaji('かんい')).toBe("kan'i");
    expect(toRomaji('しんよう')).toBe("shin'you");
    expect(toRomaji('かんじ')).toBe('kanji');
    expect(toRomaji('かんか')).toBe('kanka');
    expect(toRomaji('しんらい')).toBe('shinrai');
  });

  test('applies particle override for wa and e', () => {
    expect(toRomaji('は', true)).toBe('wa');
    expect(toRomaji('へ', true)).toBe('e');
    expect(toRomaji('は', false)).toBe('ha');
    expect(toRomaji('へ', false)).toBe('he');
  });
});

describe('furiganaFor', () => {
  const kanjiSegment: Segment = {
    start: 0,
    len: 2,
    surface: '東京',
    reading: 'とうきょう',
    matched: true,
    entries: [],
  };

  const kanaSegment: Segment = {
    start: 2,
    len: 2,
    surface: 'これ',
    reading: 'これ',
    matched: true,
    entries: [],
  };

  const particleSegment: Segment = {
    start: 4,
    len: 1,
    surface: 'は',
    reading: 'は',
    matched: true,
    entries: [{ headword: 'は', reading: 'は', conjugation: null, pos: ['prt'], senses: [], flags: ['particle'] }],
  };

  const kanjiWithoutReading: Segment = {
    start: 5,
    len: 1,
    surface: '犬',
    reading: null,
    matched: false,
    entries: [],
  };

  test('mode none returns null', () => {
    expect(furiganaFor(kanjiSegment, 'none')).toBeNull();
    expect(furiganaFor(kanaSegment, 'none')).toBeNull();
  });

  test('mode hiragana returns reading only for kanji-bearing words', () => {
    expect(furiganaFor(kanjiSegment, 'hiragana')).toBe('とうきょう');
    expect(furiganaFor(kanaSegment, 'hiragana')).toBeNull();
    expect(furiganaFor(particleSegment, 'hiragana')).toBeNull();
    expect(furiganaFor(kanjiWithoutReading, 'hiragana')).toBeNull();
  });

  test('mode katakana returns katakana reading only for kanji-bearing words', () => {
    expect(furiganaFor(kanjiSegment, 'katakana')).toBe('トウキョウ');
    expect(furiganaFor(kanaSegment, 'katakana')).toBeNull();
    expect(furiganaFor(particleSegment, 'katakana')).toBeNull();
    expect(furiganaFor(kanjiWithoutReading, 'katakana')).toBeNull();
  });

  test('mode romaji returns romanization for all segments', () => {
    expect(furiganaFor(kanjiSegment, 'romaji')).toBe('toukyou');
    expect(furiganaFor(kanaSegment, 'romaji')).toBe('kore');
    expect(furiganaFor(particleSegment, 'romaji')).toBe('wa');
  });
});
