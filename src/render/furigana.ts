import type { FuriganaMode, Segment } from '../types';

const HIRAGANA_START = 0x3041;
const HIRAGANA_END = 0x3096;
const HIRAGANA_TO_KATAKANA = 0x60;

/**
 * Transliterates Hiragana characters to Katakana via +0x60 offset.
 */
export function toKatakana(text: string): string {
  let result = '';
  for (const ch of text) {
    const code = ch.charCodeAt(0);
    if (code >= HIRAGANA_START && code <= HIRAGANA_END) {
      result += String.fromCharCode(code + HIRAGANA_TO_KATAKANA);
    } else {
      result += ch;
    }
  }
  return result;
}

const ROMAJI_TABLE: [string, string][] = [
  ['キャ', 'kya'], ['キュ', 'kyu'], ['キョ', 'kyo'],
  ['シャ', 'sha'], ['シュ', 'shu'], ['ショ', 'sho'],
  ['チャ', 'cha'], ['チュ', 'chu'], ['チョ', 'cho'],
  ['ニャ', 'nya'], ['ニュ', 'nyu'], ['ニョ', 'nyo'],
  ['ヒャ', 'hya'], ['ヒュ', 'hyu'], ['ヒョ', 'hyo'],
  ['ミャ', 'mya'], ['ミュ', 'myu'], ['ミョ', 'myo'],
  ['リャ', 'rya'], ['リュ', 'ryu'], ['リョ', 'ryo'],
  ['ヰャ', 'wya'], ['ヰュ', 'wyu'], ['ヰョ', 'wyo'],
  ['ギャ', 'gya'], ['ギュ', 'gyu'], ['ギョ', 'gyo'],
  ['ヂャ', 'ja'],  ['ヂュ', 'ju'],  ['ヂョ', 'jo'],
  ['ジャ', 'ja'],  ['ジュ', 'ju'],  ['ジョ', 'jo'],
  ['ビャ', 'bya'], ['ビュ', 'byu'], ['ビョ', 'byo'],
  ['ピャ', 'pya'], ['ピュ', 'pyu'], ['ピョ', 'pyo'],
  ['イィ', 'yi'],  ['ユィ', 'yi'],  ['イェ', 'ye'], ['ユェ', 'ye'],
  ['ヷ', 'va'], ['ヴァ', 'va'], ['ヸ', 'vi'], ['ヴィ', 'vi'],
  ['ヴ', 'vu'], ['ヹ', 've'], ['ヴェ', 've'], ['ヺ', 'vo'], ['ヴォ', 'vo'],
  ['ヴャ', 'vya'], ['ヴュ', 'vyu'], ['ヴョ', 'vyo'],
  ['シェ', 'she'], ['ジェ', 'je'], ['チェ', 'che'],
  ['スィ', 'si'], ['スャ', 'sya'], ['スュ', 'syu'], ['スョ', 'syo'],
  ['ズィ', 'zi'], ['ズャ', 'zya'], ['ズュ', 'zyu'], ['ズョ', 'zyo'],
  ['ティ', 'ti'], ['トゥ', 'tu'], ['テャ', 'tya'], ['テュ', 'tyu'], ['テョ', 'tyo'],
  ['ディ', 'di'], ['ドゥ', 'du'], ['デャ', 'dya'], ['デュ', 'dyu'], ['デョ', 'dyo'],
  ['ツァ', 'tsa'], ['ツィ', 'tsi'], ['ツェ', 'tse'], ['ツォ', 'tso'],
  ['ファ', 'fa'], ['フィ', 'fi'], ['フェ', 'fe'], ['フォ', 'fo'],
  ['フャ', 'fya'], ['フュ', 'fyu'], ['フョ', 'fyo'],
  ['クァ', 'kwa'], ['クィ', 'kwi'], ['クェ', 'kwe'], ['クォ', 'kwo'],
  ['グァ', 'gwa'], ['グィ', 'gwi'], ['グェ', 'gwe'], ['グォ', 'gwo'],
  ['ア', 'a'], ['イ', 'i'], ['ウ', 'u'], ['エ', 'e'], ['オ', 'o'],
  ['カ', 'ka'], ['キ', 'ki'], ['ク', 'ku'], ['ケ', 'ke'], ['コ', 'ko'],
  ['サ', 'sa'], ['シ', 'shi'], ['ス', 'su'], ['セ', 'se'], ['ソ', 'so'],
  ['タ', 'ta'], ['チ', 'chi'], ['ツ', 'tsu'], ['テ', 'te'], ['ト', 'to'],
  ['ナ', 'na'], ['ニ', 'ni'], ['ヌ', 'nu'], ['ネ', 'ne'], ['ノ', 'no'],
  ['ハ', 'ha'], ['ヒ', 'hi'], ['フ', 'fu'], ['ヘ', 'he'], ['ホ', 'ho'],
  ['マ', 'ma'], ['ミ', 'mi'], ['ム', 'mu'], ['メ', 'me'], ['モ', 'mo'],
  ['ヤ', 'ya'], ['ユ', 'yu'], ['ヨ', 'yo'],
  ['ラ', 'ra'], ['リ', 'ri'], ['ル', 'ru'], ['レ', 're'], ['ロ', 'ro'],
  ['ワ', 'wa'], ['ヲ', 'wo'], ['ン', 'n'],
  ['ガ', 'ga'], ['ギ', 'gi'], ['グ', 'gu'], ['ゲ', 'ge'], ['ゴ', 'go'],
  ['ザ', 'za'], ['ジ', 'ji'], ['ズ', 'zu'], ['ゼ', 'ze'], ['ゾ', 'zo'],
  ['ダ', 'da'], ['ヂ', 'ji'], ['ヅ', 'zu'], ['デ', 'de'], ['ド', 'do'],
  ['バ', 'ba'], ['ビ', 'bi'], ['ブ', 'bu'], ['ベ', 'be'], ['ボ', 'bo'],
  ['パ', 'pa'], ['ピ', 'pi'], ['プ', 'pu'], ['ペ', 'pe'], ['ポ', 'po'],
  ['ァ', 'a'], ['ィ', 'i'], ['ゥ', 'u'], ['ェ', 'e'], ['ォ', 'o'],
  ['ャ', 'ya'], ['ュ', 'yu'], ['ョ', 'yo'], ['ッ', 'tsu'],
  ['ヮ', 'wa'], ['ヰ', 'wi'], ['ヱ', 'we'],
  ['ー', '-'], ['・', ' '],
];

/**
 * Converts Hiragana or Katakana text to Romaji.
 */
export function toRomaji(text: string, isParticle = false): string {
  if (isParticle) {
    if (text === 'は' || text === 'ハ') return 'wa';
    if (text === 'へ' || text === 'ヘ') return 'e';
  }

  const kata = toKatakana(text);
  let res = '';
  let i = 0;

  while (i < kata.length) {
    const ch = kata[i];

    // Sokuon (っ / ッ)
    if (ch === 'ッ') {
      if (i + 1 < kata.length) {
        const nextSub = kata.slice(i + 1);
        let nextRomaji = '';
        for (const [k, r] of ROMAJI_TABLE) {
          if (nextSub.startsWith(k)) {
            nextRomaji = r;
            break;
          }
        }
        if (nextRomaji.length > 0) {
          res += nextRomaji[0] === 'c' ? 't' : nextRomaji[0];
          i++;
          continue;
        }
      }
      res += 'tsu';
      i++;
      continue;
    }

    // Check digraphs (2 chars) then single kana
    let matched = false;
    for (const [k, r] of ROMAJI_TABLE) {
      if (kata.startsWith(k, i)) {
        if (k === 'ン') {
          res += 'n';
          if (i + 1 < kata.length) {
            const nextCh = kata[i + 1];
            // If next is a vowel or ya/yu/yo, append apostrophe
            const nextCode = nextCh.charCodeAt(0);
            if (
              (nextCode >= 0x30A1 && nextCode <= 0x30AA) || // ァ..オ
              (nextCode >= 0x30E3 && nextCode <= 0x30E8)    // ャ..ヨ
            ) {
              res += "'";
            }
          }
        } else {
          res += r;
        }
        i += k.length;
        matched = true;
        break;
      }
    }

    if (!matched) {
      res += ch;
      i++;
    }
  }

  return res;
}

/**
 * Determines the furigana annotation string for a segment given the active mode.
 */
export function furiganaFor(segment: Segment, mode: FuriganaMode): string | null {
  if (mode === 'none') return null;

  const hasKanji = /[一-鿿]/.test(segment.surface);
  const reading = segment.reading ?? segment.surface;
  const isParticle = segment.entries[0]?.flags?.includes('particle') ?? false;

  if (mode === 'hiragana') {
    return hasKanji ? segment.reading : null;
  }
  if (mode === 'katakana') {
    return hasKanji && segment.reading ? toKatakana(segment.reading) : null;
  }
  if (mode === 'romaji') {
    return toRomaji(reading, isParticle);
  }

  return null;
}
