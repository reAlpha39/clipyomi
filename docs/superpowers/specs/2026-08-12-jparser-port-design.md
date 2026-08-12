# JParser Port — Design

**Date:** 2026-08-12
**Status:** Approved design, ready for implementation planning
**Reference implementation:** `ta-old/` (Translation Aggregator, GPL v2)

## 1. Goal

Rebuild ta-old's **JParser** module — and only that module — as a cross-platform
Rust + Tauri desktop app that parses Japanese text from the clipboard and shows a
word-by-word breakdown with readings, conjugations, and English definitions.

### In scope

- Faithful port of the JParser parsing pipeline: conjugation table, dictionary
  index, prefix matcher with verb-conjugation recursion, dynamic-programming
  segmenter.
- MeCab boundary hints (optional, as in ta-old).
- Clipboard auto-monitoring plus manual text entry.
- Segmented-sentence + definition-list UI with furigana modes.
- Light/dark/system theming, always-on-top, per-pixel background transparency,
  three window-chrome states.
- Parse history, gloss filters, adjustable font sizes, saved layout hotkeys.
- Windows and macOS builds.

### Out of scope

ta-old's other 20+ translation windows (Google, DeepL, Bing, Yandex, ATLAS, LEC,
SysTran, WWWJDIC…), the AGTH-style text hooking engine, DLL injection, menu
translation, Japanese-locale app launching, pre-translation substitution
profiles, and the standalone MeCab display pane.

### Non-goals

- Not a translator. JParser is a *reading aid*: it segments and defines, it does
  not produce sentence translations.
- No JMnedict/name dictionary in v1. The scoring code retains its `IS_NAME`
  penalty path so names can be added later without touching the segmenter.

## 2. What "the jparser module" is in ta-old

| File | Role |
|---|---|
| `exe/util/Dictionary.cpp` (1468 lines) | **Everything.** Conjugation table loader, dictionary compiler, matcher, segmenter, sorting, entry lookup, POS parsing, conjugation-label rendering. |
| `exe/util/Dictionary.h` | `Match`, `JapString`, `ConjInfo`, `EntryData`, `POSData`, flag constants. |
| `dictionaries/Conjugations.txt` | UTF-16LE JSON. 32 verb/adjective types, 29 tenses, 223 chained conjugations. |
| `exe/util/Mecab.cpp` | `libmecab.dll` dynamic loader + `MecabParseString`. |
| `Shared/StringUtil.cpp` | `wcsijcmp`/`wcsnijcmp` (kana-insensitive compare), `ToRomaji`/`ChunkToRomaji` + `romajiTable`. |
| `exe/TranslationWindows/LocalWindows/JParseWindow.cpp` | Win32 glue: threading, config dialog. Not portable. |
| `exe/TranslationWindows/LocalWindows/FuriganaWindow.cpp` | GDI rendering, `characterType` furigana modes. Not portable; behaviour is. |

Everything else in `ta-old/` is other translators, hooking, or Win32
infrastructure.

## 3. Architecture

```
translation-aggregator/
├── ta-old/                       # reference only, never modified
├── docs/superpowers/specs/
└── ta/
    ├── crates/jparser/           # pure Rust. No Tauri, no UI, no globals.
    │   ├── src/
    │   │   ├── lib.rs            # parse() entry point, public types
    │   │   ├── kana.rs           # normalization, char classification
    │   │   ├── romaji.rs         # romajiTable + ToRomaji port
    │   │   ├── conjugation.rs    # table load + Next Type resolution
    │   │   ├── jmdict.rs         # streaming XML -> headword records
    │   │   ├── index/
    │   │   │   ├── build.rs      # records -> FST + payload blob
    │   │   │   └── load.rs       # mmap + prefix query
    │   │   ├── matcher.rs        # matches at a position + verb recursion
    │   │   ├── segment.rs        # DP segmenter
    │   │   └── morph.rs          # Vibrato -> BoundaryHints
    │   ├── assets/conjugations.json
    │   └── tests/
    ├── src-tauri/                # thin shell
    │   └── src/{clipboard,parse,commands,settings,window}.rs
    └── src/                      # web UI (Vite + TypeScript)
```

**Hard rule:** `crates/jparser` has no Tauri dependency and no I/O beyond
reading its index and asset files. It must be testable, benchmarkable, and
fuzzable without a window. This is the boundary that makes the port verifiable.

**Data flow** is one-directional:

```
clipboard poll ─┐
manual input  ─┴→ parse worker → ParseResult → JSON event → webview
```

ta-old's mid-parse `PostMessage(WMA_JPARSER_STATE)` progress pings become a
plain event emit (`parse:progress`).

## 4. Data assets

### 4.1 Conjugation table

`ta-old/dictionaries/Conjugations.txt` is UTF-16LE JSON. Convert once to UTF-8
and commit as `crates/jparser/assets/conjugations.json`. It is data, and it is
the single most valuable artifact in ta-old — hand-tuned and not reconstructible.

Verified contents: 32 types, `Part of Speech` ∈ {`Verb`, `Adj`}, 223
conjugations carrying a `Next Type`.

Two kinds of type live in this list, and the distinction matters:

- **Entry types** — reachable from a JMdict POS tag. Names are literally EDICT
  POS codes: `adj-na`, `adj-i`, `v1`, `v5u`, `v5k`, `v5k-s`, `v5g`, `v5s`,
  `v5t`, `v5n`, `v5b`, `v5m`, `v5r`, `v5r-i`, `v5aru`, `v5u-s`, `v5uru`, `vk`,
  `vz`, `vs`, `vs-s`, `vs-i`.
- **Chain-only types** — never matched against a dictionary POS; exist solely as
  `Next Type` targets: `copula`, `adj-ta`, `v-i-stem`, `v-a-stem`, `v-ta-stem`,
  `v-u-stem`.

**Four names are duplicated** (`v5r-i`, `v5uru`, `vk`, `vs` each appear twice).
This is intentional: one POS maps to multiple type entries with different
conjugation sets. Lookup must return **all** matching types, not the first.

**Tense IDs are not a closed set.** 20 static names are seeded in fixed order;
the file contributes 9 more, appended in encounter order. Only these four
positions are semantically special-cased in the algorithms and must keep their
discriminants:

```rust
Remove  = 0,   NonPast = 1,   Stem = 2,   Potential = 3,
```

The remaining 25 are dynamic IDs resolved by name at load time.

### 4.2 `Next Type` resolution (port literally)

After loading, for each conjugation with a `next_type`: find that target type's
`remove_tense`/form-0 conjugation whose suffix is a **suffix of** this
conjugation's own suffix, then trim it off and store `next_verb_type_id`. This
is what allows conjugations to stack (て + いる + ない).

Four nested loops in `LoadConjugationTable`, not guessable from its output.
Port as-is, then test. Any unresolvable `Next Type` is a **hard startup failure**
naming the offending type — ta-old returned `false`, and that strictness is
correct because a silently unresolved chain degrades parsing invisibly.

### 4.3 JMdict

Downloaded on first run from EDRDG, gzipped XML, cached in the app data dir.
Chosen over ta-old's EDICT2 because it carries structured data where ta-old was
doing string surgery on English glosses:

| ta-old | Now |
|---|---|
| `(P)` substring in the gloss | `<ke_pri>` / `<re_pri>` = news1/ichi1/spec1/gai1 |
| `strncmp` of `"prt"`/`"conj"` vs gloss prefix | `<pos>` tag |
| `(ctr)`/`(suf)` present and `(arch)` absent | `<pos>` + `<misc>` |
| verb type matched by name against gloss text | `<pos>` tag → conjugation type name |

This is the one deliberate deviation from ta-old's data source, and it makes the
port both simpler and more accurate.

### 4.4 Vibrato dictionary

Downloaded **on demand** when the MeCab toggle is first enabled — not at first
run. Leaving MeCab off means never paying for it.

## 5. Parser crate

### 5.1 Public surface

```rust
pub fn parse(text: &str, opts: &ParseOptions, hints: Option<&dyn BoundaryHints>)
    -> Result<ParseResult, ParseError>;

pub struct ParseResult { pub segments: Vec<Segment> }

pub struct Segment {
    pub start: usize,          // char offset into input, not byte
    pub len: usize,            // in chars
    pub surface: String,
    pub reading: Option<String>,   // display reading = entries[0].reading;
                                   // what furigana renders. None when unmatched
                                   // (no MeCab fallback — see §5.7)
    pub matched: bool,         // false for unmatched runs
    pub entries: Vec<Entry>,   // primary first, then alternatives
}

pub struct Entry {
    pub headword: String,
    pub reading: Option<String>,
    pub conjugation: Option<String>,   // "Negative Formal Past"
    pub pos: Vec<String>,
    pub senses: Vec<Sense>,
    pub flags: WordFlags,
}
```

All public types are owned and immutable. The DP is inherently a mutable table
fill; that mutation stays local to `segment()`, which returns an owned `Vec`.

### 5.2 Index build

Stream JMdict with `quick-xml`. Per `<entry>`, emit one **headword record** per
`<keb>` and per `<reb>` — mirroring ta-old's `JapString` list — carrying:

```rust
struct HeadwordRecord {
    surface: String,
    flags: WordFlags,     // PRIMARY | PRONOUNCE | COMMON | COMMON_LINE
                          //   | PARTICLE | COUNTER | IS_NAME
    verb_type: Option<VerbTypeId>,
    entry_id: u32,
}
```

Flag derivation:

- `PRIMARY` — first headword of the entry.
- `PRONOUNCE` — a `<reb>` on an entry that also has kanji forms.
- `COMMON` — a priority marker on **this** headword's `<ke_pri>`/`<re_pri>`.
- `COMMON_LINE` — a priority marker on **any** form of the entry. ta-old carried
  both (`JAP_WORD_COMMON` vs `JAP_WORD_COMMON_LINE`) because EDICT2's `(P)` could
  be per-line or per-form; JMdict makes both cleanly derivable, and both are read
  by the segmenter and the sort.
- `PARTICLE` — any sense has POS `prt` or `conj`.
- `COUNTER` — any sense has POS `ctr` or `suf`, and no sense is `arch`.
- `IS_NAME` — reserved for JMnedict; unused in v1.

#### Verb stem generation (the part that is easy to miss)

The index contains **headwords *and* generated verb stems as separate entries**.
`CreateDict` (`Dictionary.cpp:413-470`) does this, and the matcher depends on it:
`FindVerbMatches` begins matching suffixes from a stem, not from a full headword.

For each verb/adjective headword, for each type from its POS:

1. Iterate **all 32 types**. Accept a candidate type if either:
   - its name equals the POS type name, **or**
   - **the v5 fix:** both names start with `"v5"` *and* have equal length.
     ta-old's comment: *"Fix a couple dozen incorrectly annotated verbs. Doesn't
     get them all, but gets a lot."* A verb mis-tagged `v5r` therefore also gets
     stems for `v5k`, `v5m`, `v5t`, … This deliberate over-generation absorbs
     EDICT mis-annotation and must be preserved.
2. Within the accepted type, find its `remove_tense`/form-0 conjugation whose
   suffix matches the headword's tail (kana-insensitively).
3. Emit a new record: headword with that suffix stripped, tagged with that
   `verb_type`. **Empty stems are legal** — `Dictionary.h` notes *"len 0 is for
   verbs which have 0 characters after removing the suffix."*
4. Deduplicate on `(stem, verb_type)` within the entry.

> **Documented deviation.** ta-old's step-4 dedupe is dead code: the loop at
> `Dictionary.cpp:450` has `if (...) continue;` as its entire body, so it always
> runs to completion, so the guard `if (js < numJStrings) break;` never fires.
> We implement the intent. Duplicate stems only add match-list noise, and both
> `FindMatches` and `SortMatches` dedupe downstream, so this is safe.

#### FST

- **Keys:** every record's surface, normalized to hiragana, inserted in
  lexicographic order (`fst::Map` requires sorted input).
- **Values:** `u64` offset into an mmap'd payload blob. Multiple records collide
  on one key — readings are shared across entries — so a key maps to a *list*.
- **Payload:** flat, mmap-readable region of record lists plus entry data
  (senses, glosses, POS, xrefs).

This is how kana-insensitivity is removed without losing behaviour. ta-old
threaded a custom comparator (`wcsijcmp`) through binary search in three places.
Here the *key* is normalized so the walk is trivial, the *surface* is preserved
in the payload, and `inexact` is set by comparing surface against the source
slice after the walk. ta-old's +10 inexact penalty still fires; the comparator
does not exist.

- **Header** carries a format version. Mismatch ⇒ rebuild from source, never
  attempt a read. (ta-old's `DICT_VERSION 0x000C`.)
- **Cache key** is a hash of the source file, replacing ta-old's
  size+mtime+ctime `FileSig`.

**Known limitation, accepted deliberately.** Half-width katakana normalization
can change character counts, which would desynchronize match offsets. ta-old did
not handle this either. Mark with a `ponytail:` comment naming the upgrade path:
a full-width pre-pass over the input plus an offset map.

### 5.3 Matcher

`matches_at(text, i) -> Vec<Match>` walks the FST from position `i`; every
terminal node passed is a candidate.

- **Non-verb record:** one match, `len == src_len == surface length`,
  `inexact = (surface != source slice)`.
- **Verb/adjective record** (`verb_type` set): recurse conjugation suffixes per
  `FindVerbMatches` —
  - depth ≤ `MAX_CONJ_DEPTH` (5),
  - skip `Remove` tense conjugations,
  - chain through `next_verb_type_id`,
  - informal `Stem` conjugations at depth > 0 do **not** consume depth and are
    not added to the list,
  - drop `Potential Potential` duplicates (explicitly guarded in ta-old).

Each match carries the full `(type, tense, conj, form)` chain per depth level,
which is what renders the conjugation label.

`form` is a 2-bit field: `1 = formal`, `2 = negative`.

### 5.4 Segmenter

Dynamic program over character positions. **Low score wins.** ta-old's constants
are kept verbatim as named constants — they are tuned against real Japanese and
are not to be re-derived by feel.

```
SKIP_CHAR                 100
SKIP_KANJI_EXTRA         +400      (CJK ideograph 0x4E00..=0x9FBF)
MATCH_BASE                 10
PARTICLE_BONUS             -2
SINGLE_CHAR_PENALTY        +1      (non-particle, len == 1)
MID_NUMBER_BREAK         +100
COMMON_BONUS               -3
COUNTER_AFTER_NUMBER       -2      (else clear the COUNTER flag entirely,
                                    so it cannot win the later sort)
INEXACT_PENALTY           +10
NAME_DICT_BAD       +500 * len     (outside a katakana run)
NAME_DICT_OK               +5
MECAB_BAD_START           +10
MECAB_BAD_END             +10
```

`IsDigit` counts ASCII digits, full-width digits, and 一二三四五六七八九十百千万.

`COMMON_BONUS` applies when **either** `COMMON` or `COMMON_LINE` is set (ta-old
tested the pair together). The two `NAME_DICT_*` constants are implemented but
**dormant in v1** — nothing sets `IS_NAME` until JMnedict lands, so no
implementer should go looking for the name dictionary that feeds them.

**Backtrack pass — do not omit.** After choosing the cheapest path, walk it and
collect *every* match aligning to the chosen `(start, len)` spans, not only the
winners. This is what populates the definition list with alternative readings
instead of a single guess, and it is what makes the collapsed-alternatives UI
possible.

**Then `SortMatches`:** dedupe (including ta-old's inexact-value reconciliation
across identical matches), then rank by inexact, then name flag, then the flag
mask `COUNTER | PARTICLE | COMMON | COMMON_LINE | PRIMARY`, then dict index,
then conjugation form.

### 5.5 Conjugation label rendering

Port `GetConjString`: for each depth level with a type, prefix `"Negative "` if
`form & 2`, `"Formal "` if `form & 1`, then append the tense name — **skipping**
`Stem` tenses, and skipping `NonPast` at depth > 0 (unless depth > 1 or depth 0
was not `Stem`). Trim the trailing space.

### 5.6 Kana and romaji

Port from `Shared/StringUtil.cpp` and `FuriganaWindow::GetFurigana`:

- **Hiragana mode:** reading as-is.
- **Katakana mode:** hiragana → katakana via `+0x60`, but **bail and return the
  raw reading** if any char is `>= 0x3097` (katakana-only characters have no
  hiragana counterpart).
- **Romaji mode:** `ToRomaji` over `romajiTable` — digraphs first (キャ→`kya`),
  then singles, with っ doubling the following consonant. Port the table
  verbatim; it is data.
  - **Particle fixup, required:** if the word is a particle, a trailing `ha`
    becomes `wa` *unless* preceded by `c` (so `cha` survives), and a bare `he`
    becomes `e`. That is は→wa and へ→e.
- **None:** no annotation.

### 5.7 MeCab boundary hints

```rust
pub trait BoundaryHints {
    fn bad_start(&self, pos: usize) -> bool;
    fn bad_end(&self, pos: usize) -> bool;
}
```

`pos` is a **char** offset, matching `Segment.start` — not a byte offset. Mixing
the two here would silently corrupt hint alignment on every multi-byte character,
which is all of them.

`segment()` takes `Option<&dyn BoundaryHints>`, which keeps the toggle honest and
lets the DP be unit-tested without a large dictionary fixture.

Vibrato implementation, porting `Dictionary.cpp:1085-1138` exactly:

1. **Re-locate** each token's surface in the input by scanning forward, allowing
   skipped characters. If the token is not fully found, or the located span does
   not kana-insensitively equal it, **rewind and ignore the token**
   (*"I don't trust mecab all that much."*).
2. **Require the reading field** (feature field 7) to exist and not be `*`. A `*`
   means MeCab does not actually know the word, so it gets no vote.
3. **Mark interior boundaries only.** For a token at `[s, s+n)`: `bad_end` on
   `s ..= s+n-2`, `bad_start` on `s+1 ..= s+n-1`. The token's first character may
   still start a match and its last may still end one.

MeCab tokens are **never used as words**. Every output word comes from the
dictionary; MeCab only votes on where boundaries should not fall, at ±10 against
a 100/500 baseline — a tiebreaker a good dictionary hit overrides freely.

Vibrato is chosen over `libmecab` bindings because it is pure Rust: no native
dependency to install per platform, dictionary loaded once instead of
`mecab_new2`/`mecab_destroy` per parse, no global lock, and native UTF-8 instead
of ta-old's UTF-16 → Shift-JIS/EUC-JP round trip.

## 6. App shell (`src-tauri`)

| Module | Responsibility |
|---|---|
| `clipboard.rs` | 200 ms poll loop |
| `parse.rs` | single worker, latest-wins |
| `commands.rs` | `parse_text`, toggles, `ensure_dictionary` |
| `settings.rs` | persisted JSON in the app config dir |
| `window.rs` | chrome state, always-on-top, geometry, layout slots |

**Clipboard poll** skips work on four conditions: text unchanged, no Japanese
characters present (kana or CJK ideograph — ta-old's rule), text this app placed
on the clipboard (tracked by retaining the last value we wrote, so copying out of
our own definition pane cannot trigger a re-parse), or length over 10 000
characters. The length cap is a soundness guard, not a performance one.

**Latest-wins parsing.** ta-old's note is that local algorithms "stop and rerun"
rather than queue, which matters when game text advances faster than parsing. One
worker fed by a watch channel; a new input supersedes the in-flight parse rather
than stacking behind it.

## 7. UI

### 7.1 Direction

**Swiss / technical**, chosen because the content *is* typography — Japanese at
24 px, readings at 11 px, glosses at 14 px. Hierarchy comes from scale contrast
and a hairline rule system, not cards, shadows, or a decorative accent.

**Fonts:** system CJK stack (Hiragino Sans / Yu Gothic UI), system UI for
glosses, system mono for metadata tags. Zero webfonts — no download, no FOUT,
and correct per-platform glyphs that a webfont would actively get wrong.

**Motion:** `transform` and `opacity` only. Chip hover, plus a short row stagger
on new parse so a content change is visible. Behind `prefers-reduced-motion`.

### 7.2 Layout

**Header, 32 px** — drag region + status text · history ◀ ▶ · furigana mode
`[— ひ カ R]` · always-on-top · clipboard pause · ⚙

**Sentence pane** — segmented text as inline word chips, colored by content
class (kanji-bearing / kana-only / particle) from semantic tokens. Unmatched runs
render muted and unchipped, so coverage gaps are visible rather than disguised.
Hover lifts the surface; click scrolls to and marks the definition row. Kana
furigana modes use `<ruby>`; **romaji renders as a second line under the chip**,
because romaji is far wider than the kanji above it (ta-old special-cased this
too, `FuriganaWindow.cpp:368`).

**Definition pane** — one row per span, in sentence order:

```
言われた      [24px CJK]        Negative Formal Past   [mono tag]
いわれた      [11px muted]
              1. to be said; to be called
              2. …
```

Alternative entries for the same span nest under the row, collapsed past the
first — the payoff from the segmenter's backtrack pass.

**Settings popover** (not a modal; does not block the parse behind it): theme
(system/light/dark), MeCab toggle, gloss filters, font sizes, background opacity,
chrome mode, dictionary update.

Placement rule: **frequency decides.** Mid-session controls live in the header;
set-once controls live in the popover.

### 7.3 Theming

Tokens in `styles/tokens.css`. Three states — system / light / dark:

```css
:root { /* complete light palette */ }
:root:not([data-theme="light"]) {
  @media (prefers-color-scheme: dark) { /* dark overrides only */ }
}
:root[data-theme="dark"] { /* manual override wins in both directions */ }
```

The webview follows the OS on both platforms, so `prefers-color-scheme` handles
the system case with no Rust involvement; the shell stores only the manual
override. No color may have its only definition inside a media query.

### 7.4 Window chrome

Three states, mapping onto ta-old's three (`MakeWindow`, `TranslationAggregator.cpp:542`):

| State | ta-old equivalent | Implementation |
|---|---|---|
| **Header + resize edges** (default) | `WS_POPUP \| WS_THICKFRAME` | `decorations: false`; our header is the titlebar; invisible CSS edges call `startResizeDragging` |
| **Content only** | Setsumi's `borderlessWindow` (`WS_POPUP`) | Also hide our header; toggles move to a right-click menu. Maximum real estate over a game. |
| **Native frame** | `WS_OVERLAPPEDWINDOW` | Real OS titlebar and buttons. **Forces background opacity to 100%** — a native titlebar over a transparent body is broken on macOS. |

ta-old toggled by destroying and recreating the window; Tauri does it live.

Two ta-old chrome flags are **N/A, not forgotten**: `showToolbars` (per-pane
translate buttons — we have one pane) and `lockWindows` (locks child-pane
splitters — we have no child panes).

### 7.5 Transparency

**Per-pixel background only.** `"transparent": true` + `decorations: false`, with
opacity driven by a `--surface-alpha` token. Text, readings, and glosses stay
100 % opaque.

Requires Tauri's `macos-private-api` on macOS — acceptable for a
self-distributed OSS build; it does rule out Mac App Store distribution.

Deliberately **not** ta-old's behaviour. ta-old used uniform whole-window alpha
(`SetLayeredWindowAttributes(hWnd, 0, alpha, LWA_ALPHA)`, clamped 20–255), which
dimmed the Japanese text the user was trying to read. Per-pixel gives the
see-through effect while keeping glyphs crisp, and needs no per-platform shim.

### 7.6 Gloss filters

Three of ta-old's six JParser display options are real against JMdict:

| ta-old option | New behaviour |
|---|---|
| Hide cross refs | drop `<xref>` + `<ant>` |
| Hide usage | drop `<s_inf>` + `<misc>` |
| Hide POS | drop `<pos>` from display |
| No kana brackets | **N/A** — stripped `[KANA]` from EDICT2's `KANJI [KANA] /gloss/` line format. Our reading is a separate field; there are no brackets. |
| Definition lines | **Subsumed** by the two-pane layout |
| Japanese own line | **Subsumed** by the two-pane layout |

Shipping no-op toggles would be worse than explaining their absence.

### 7.7 Font sizes

`normalFontSize` and `furiganaFontSize` (ta-old's JParser config) bound to
`--text-cjk` and `--text-furigana`. Every size in the design derives from tokens,
so two inputs rescale coherently with no layout special-casing.

### 7.8 Parse history

Ring buffer of 50, storing **full parse results** rather than input text —
instant navigation, and immune to the dictionary changing mid-session.
Consecutive duplicates deduped. Back/forward in the header.

This matters more than it looks: with clipboard monitoring on, every new copy
destroys the previous parse. History is what makes monitoring safe to leave
running.

Session-only. ta-old persisted history to disk (`exe/History/`); disk
persistence is a later addition.

### 7.9 Layout hotkeys

Tauri global-shortcut plugin. `Alt/Option+1..9` restores a slot,
`Shift+Alt+1..9` binds the current state — matching ta-old's `alt-#` /
`shift-alt-#`.

A slot captures what ta-old's did plus the new controls: geometry, chrome mode,
background opacity, always-on-top, font sizes, furigana mode.

## 8. Settings and persistence

Persisted as JSON in the app config dir:

- theme (system/light/dark), always-on-top, chrome mode, background opacity
- clipboard monitoring on/off, MeCab on/off
- furigana mode, font sizes, gloss filters
- window geometry, pane split
- layout slots 1–9

## 9. Error handling

| Boundary | Policy |
|---|---|
| **Dictionary download** | Resume on retry; verify gzip integrity; build to a temp path and atomically rename. A failed update never leaves the app dictionary-less — the previous index stays live. |
| **JMdict parse** | Skip malformed *entries* with a counted warning surfaced in the status line. **Hard-fail on an unmapped verb POS** — silently dropping conjugations degrades parsing invisibly, the worst failure mode for this app. |
| **Conjugation table** | Committed asset, so failure is a packaging bug. Keep ta-old's strictness: fail loudly at startup naming the type whose `Next Type` could not resolve. |
| **Index version mismatch** | Rebuild from source; never attempt a read. |
| **Vibrato dictionary** | Missing or corrupt ⇒ MeCab toggle shows unavailable *with the reason*; parsing continues unhinted. Never blocks a parse. |
| **Clipboard read** | Can fail transiently when another app holds it. Log-and-skip the tick — the one case deliberately not surfaced, since a poll failure is not information. |
| **Parse panic** | The matcher does offset arithmetic. Catch at the worker boundary, report with input length, keep the previous result on screen. A panic must not kill the app. |

## 10. Testing

### Unit — `crates/jparser`, where the signal is

- `kana.rs` — normalization round-trips, the `+0x60` mapping, and its `>= 0x3097` bail.
- `romaji.rs` — table cases, っ doubling, and the particle fixup **including** the `cha` exception. Pure functions with exact expected outputs.
- `conjugation.rs` — **highest-risk part of the port.** Assert that a stacked form (e.g. 言われなかった) resolves to the correct *chain*, not merely that something matched.
- `index/build.rs` — verb stem generation: that the v5 fix over-generates as intended, and that empty stems are produced and retained.
- `matcher.rs` — golden tests against a hand-written ~20-entry dictionary, not JMdict. Fast and immune to dictionary version drift.
- `segment.rs` — DP with `None` hints and with a stub `BoundaryHints`, asserting **the cost**, not just the winning segmentation, so a scoring regression is caught even when the winner does not change.
- **`insta` snapshots over ~30 real sentences** — the highest-value test here; it answers "did my refactor change the parse?"

### Differential against ta-old

ta-old is a working binary. Push ~200 sentences through both and diff the
segmentations. **Not a CI gate** — JMdict and EDICT2 will legitimately disagree —
but it is the only way to learn whether the port is *faithful* rather than merely
self-consistent. Run once, deliberately, before shipping.

### Integration

Index build over a truncated JMdict fixture → query → expected entries. The
download path is tested against a `file://` URL, never the network.

### UI — Playwright

320 / 768 / 1024 for overflow, both themes, reduced-motion, against a **fixed
parse fixture** so screenshots are deterministic. Keyboard navigation and
contrast checks: five header controls plus a segmented control need real focus
states.

### Not tested

Always-on-top, transparency, and chrome switching. Manual per-platform
checklist — automating window-manager behaviour is more fragile than the bugs it
would catch.

### Coverage

**80 % on `crates/jparser`**, held as a real line. Not meaningful in `src-tauri`,
which is glue and window plumbing; a repo-wide number would average the two into
something that means nothing.

## 11. Phasing

Scope roughly doubled during design, so the implementation plan should sequence
rather than treat this as one pile.

**Phase 1 — Parser core.** Conjugation table + `Next Type`, kana/romaji, JMdict
streaming, index build with stem generation, FST, matcher, DP segmenter. Unit
tests and snapshots. No UI. Verifiable via a CLI harness.

**Phase 2 — Minimum viable app.** Tauri shell, first-run download, clipboard
monitor, sentence + definition panes, light/dark, always-on-top. Usable end to
end.

**Phase 3 — Reading aids.** Furigana modes (all four), font sizes, gloss
filters, settings popover.

**Phase 4 — Window behaviour.** Three chrome states, per-pixel transparency,
layout hotkeys, parse history.

**Phase 5 — MeCab.** Vibrato integration, on-demand dictionary download,
boundary hints, toggle.

**Phase 6 — Verification.** Differential run against ta-old, Playwright suite,
per-platform manual checklist, Windows + macOS builds.

MeCab lands late deliberately: it is a ±10 tiebreaker on a 100/500 baseline, so
it cannot be validated until the DP it nudges is known-good.

## 12. Deliberate deviations from ta-old

| # | Deviation | Why |
|---|---|---|
| 1 | JMdict XML instead of EDICT2 | Structured `<pos>`/priority instead of string surgery on English glosses |
| 2 | FST instead of binary search + `wcsijcmp` | Fewer lines *and* faster; deletes three copies of a custom comparator |
| 3 | Vibrato instead of `libmecab` | Pure Rust; no per-platform native dependency, no per-parse init, no lock, no encoding round trip |
| 4 | Per-pixel background alpha instead of uniform window alpha | ta-old dimmed the text being read; also needs no platform shim |
| 5 | Stem dedupe actually implemented | ta-old's is dead code (`Dictionary.cpp:450`); duplicates are pure noise |
| 6 | No-kana-brackets filter dropped | N/A against JMdict's structured reading field |
| 7 | Definition-lines / Japanese-own-line dropped | Subsumed by the two-pane layout |
| 8 | Settings popover instead of modal dialog | Does not block the parse behind it |
| 9 | History session-only | ta-old persisted to disk; deferred, not refused |

## 13. Deferred

- JMnedict name dictionary (scoring path retained).
- Disk-persisted history.
- Half-width katakana normalization with an offset map.
- Pre-translation substitution profiles.
- Standalone MeCab comparison pane.
- Linux build (nothing in the code should prevent it).

## 14. Licensing

ta-old is **GPL v2**. This port reuses its algorithms and its
`Conjugations.txt` data asset, so the new app is a derivative work and ships
**GPL v2**. JMdict is CC-BY-SA 4.0 and requires EDRDG attribution in-app.
