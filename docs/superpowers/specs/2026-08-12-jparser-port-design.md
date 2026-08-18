# JParser Port — Design

**Date:** 2026-08-12
**Last amended:** 2026-08-18
**Status:** In implementation — Phases 1 and 2 shipped, Phase 3 in progress. See
§11 for the per-phase breakdown; sub-phase design specs amend this document
rather than replace it.
**Reference implementation:** `ta-old/` (Translation Aggregator, GPL v2)
**Ships as:** ClipYomi (binary and app name, renamed during Phase 2)

## 1. Goal

Rebuild ta-old's **JParser** module — and only that module — as a cross-platform
Rust + Tauri desktop app that parses Japanese text from the clipboard and shows a
word-by-word breakdown with readings, conjugations, and English definitions.

### In scope

- Faithful port of the JParser parsing pipeline: conjugation table, dictionary
  index, prefix matcher with verb-conjugation recursion, dynamic-programming
  segmenter.
- MeCab boundary hints (optional, as in ta-old).
- Clipboard auto-monitoring. (Manual text entry was removed in Phase 2H — see
  `docs/superpowers/specs/2026-08-15-jparser-phase2h-design.md`. The clipboard is
  the only user-facing input path.)
- Segmented-sentence UI with furigana modes, and definitions on hover or focus.
  (The bottom definition-list pane was removed in Phase 2J — see
  `docs/superpowers/specs/2026-08-15-jparser-phase2j-design.md`. A popover window
  is the only definition surface.)
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
├── crates/jparser/               # pure Rust. No Tauri, no UI, no globals.
│   ├── src/
│   │   ├── lib.rs                # parse() entry point, public types
│   │   ├── kana.rs               # normalization, char classification
│   │   ├── romaji.rs             # romajiTable + ToRomaji port
│   │   ├── conjugation.rs        # table load + Next Type resolution
│   │   ├── jmdict.rs             # streaming XML -> headword records
│   │   ├── record.rs             # headword record + flags
│   │   ├── stem.rs               # verb stem generation
│   │   ├── rank.rs               # entry ordering
│   │   ├── index/
│   │   │   ├── build.rs          # records -> FST + payload blob
│   │   │   ├── load.rs           # mmap + prefix query
│   │   │   └── generations.rs    # immutable numbered publish (2A)
│   │   ├── matcher.rs            # matches at a position
│   │   ├── matcher/verb.rs       # verb-conjugation recursion
│   │   ├── segment.rs            # DP segmenter
│   │   ├── hints.rs              # Vibrato -> BoundaryHints (feature `mecab`)
│   │   └── bin/jparser-cli.rs    # index build, dump, parse
│   ├── assets/conjugations.json
│   └── tests/
├── crates/jmdict-source/         # JMdict acquisition: HTTP + gunzip (2B)
├── src-tauri/                    # thin shell
│   └── src/{main,state,clipboard,parse,commands,settings,popover,
│            mouse_tracker}.rs
└── src/                          # web UI (Vite + TypeScript)
    ├── main.ts, popover.ts, settings.ts
    ├── render/                   # sentence, furigana, tooltip text + colour
    └── styles/
```

The tree names the files that exist today; the sub-phase that introduced a
non-obvious one is noted in parentheses.

**Hard rule:** `crates/jparser` has no Tauri dependency and no I/O beyond
reading its index and asset files. It must be testable, benchmarkable, and
fuzzable without a window. This is the boundary that makes the port verifiable.

**Data flow** is one-directional:

```
clipboard poll → watch channel → parse worker → ParseResult
                                              → `parse-result` event → webview
```

Manual input was the second source into that channel until Phase 2H deleted it;
the clipboard is now the only one.

ta-old's mid-parse `PostMessage(WMA_JPARSER_STATE)` progress pings were to become
a plain event emit (`parse:progress`). **Not built** — a parse finishes fast
enough that nothing has needed to observe it mid-flight.

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
| `main.rs` | `setup`: settings load, geometry restore, window creation, task spawn |
| `state.rs` | shared app state, index generations, settings state |
| `clipboard.rs` | 200 ms poll loop |
| `parse.rs` | single worker, latest-wins |
| `commands.rs` | `parse_text`, toggles, `ensure_dictionary`, settings-window lifecycle |
| `settings.rs` | persisted JSON in the app config dir |
| `popover.rs` | the tooltip window: creation, placement, hide |
| `mouse_tracker.rs` | native cursor-tracking thread for unfocused hover (2L) |

There is no `window.rs`. Chrome state, always-on-top and geometry live in
`commands.rs` next to the other toggles, because each one is a persisted setting
first and a window call second. Layout slots (§7.9) are unbuilt.

**Three windows**: `main`, built by Tauri from the config array before `setup`
runs, and `popover` and `settings`, built in `setup`.

**No window is mapped before its page can paint itself.** A webview mapped
earlier shows its own default white, and the theme is not available that early:
it lives in stylesheets the pages import (§7.3), which under the dev server
arrive as module requests *after* the document itself finishes loading. That is
where the second-long white frame came from. A release build should be far
better — `vite build` extracts the CSS into a `<link>` in the document head —
but it has not been measured, so the reveal is gated the same way on both.

Two mechanisms, because the windows are created differently:

| Window | Reveal |
|---|---|
| `popover`, `settings` | Built hidden in `setup` and shown on demand. Nothing is left to load at reveal time, and it also avoids the hundreds of milliseconds a webview costs to build. Closing the settings window hides it rather than destroying it, so it is never rebuilt. |
| `main` | Configured `"visible": false` and reveals itself from `main.ts`, since Tauri builds it from the config array before `setup` runs. Hung off the settings-restore promise so the restored control states are in the first painted frame, and off `finally` so a failed read still leaves a visible window. |

Two reveal signals were tried and are wrong, recorded so they are not tried
again: `requestAnimationFrame` in the page never fires, because a window that is
not visible is not being composited — script evaluation is part of page load and
does run, which is what the main window's reveal relies on. And Tauri's
`on_page_load` `Finished` event fires at document load, before the stylesheets
have arrived.

Revealing late has a cost worth naming: if a page fails to load, its window
stays hidden rather than appearing blank. No watchdog reveal exists — one can
only fire early enough to show the unstyled frame this avoids, or too late to
help.

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

As shipped (3B onward) every persisted toggle — always-on-top, clipboard
monitoring, title bar, furigana mode, gloss filters — lives in the dedicated
settings window instead. The main header keeps only the drag region and the
gear that opens that window; the duplicate header controls were removed once
the settings window owned them, so a setting has exactly one control.

**The header is the titlebar band.** 28px, matching what macOS draws in the
same strip (the window keeps `Titled + FullSizeContentView` in both states, so
the webview owns it either way). The OS puts its buttons at the left; the gear
sits at the right. `#app[data-decorations]`, mirrored from the setting by
`main.ts`, picks between two shapes:

| `decorations` | Band | Gear | Divider |
|---|---|---|---|
| `true` | reserved 28px grid row | always visible | visible |
| `false` | out of the grid, absolute overlay, 6px idle → 28px while `#app.peeked`, opaque `--color-bg` fill | fades in with the band | only while revealed |

Hidden means the sentence starts at the window's top edge, and the revealed bar
is **added on top of the window** rather than laid over the content: the frame
grows upward by the band height (bottom edge fixed) while `#app` takes an equal
`padding-top`, so what the user was reading does not move on screen. The idle
band is a 6px sliver rather than a transparent 28px one so it cannot swallow
hovers and clicks aimed at chips at the top of the sentence, and it stays a drag
region so the top edge still drags the frameless window.

**The reveal trigger is the whole window**, not the strip: `pointerenter` /
`pointerleave` on `#app`, so the cursor anywhere inside shows the bar and only
leaving hides it. The gear's `focus`/`blur` drive the same path — CSS
`:hover`/`:focus-within` cannot be used at all here, because only the backend can
grow the frame and a CSS-only reveal would light the strip up while the window
was still the old height, i.e. over the sentence. `main.ts` owns both halves and
`BAND_HEIGHT` (28) is the number it sends; `--band-h` is the CSS twin.

The window is created **`titleBarStyle: "Overlay"`** (macOS), which is what
makes any of this possible: the webview owns the titlebar strip in *both*
states, so the gear can sit inside the title bar and the strip's ownership never
changes at runtime. The first cut of this feature had no such flag — the webview
only got `FullSizeContentView` when `decorations` was false — which produced two
defects: the gear rendered *below* the OS title bar whenever the title bar was
shown, and hovering blinked. The blink was a genuine feedback loop:
`setTitlebarAppearsTransparent(false)` handed the strip to the OS mid-hover, the
webview stopped receiving pointer events there, `pointerleave` fired, the
frontend undid the peek, the strip returned, `pointerenter` fired — measured at
five full cycles from one hover.

The OS chrome follows the reveal through `peek_titlebar(visible)`, which calls
the narrow `set_titlebar_chrome` helper: title-text visibility and the four
standard window buttons, and **nothing else** — no `setStyleMask:`, no
transparency flip (ownership), and no `set_theme` repaint (that stays in
`apply_decorations_macos`, where a deliberate toggle hides it). No settings write
either, because a hover must never rewrite the persisted flag. `main.ts` fires it
on those transitions only, and the backend keeps its own `PEEKED` flag so a
duplicate show cannot grow the frame twice and a webview reload mid-peek cannot
desync the frontend's copy from the real frame. The growth arrives in the webview
as an ordinary `tauri://resize`, which the geometry debounce skips while peeking
— otherwise the stored height would gain a band on every hover. The hide is delayed by
`PEEK_HIDE_MS` (1s, after the cursor leaves the window) and cancelled by a
re-entry: the revealed buttons are real
NSViews over the band's left edge, so reaching for one reads as a
`pointerleave`, and an instant hide would oscillate again in that small region.
A `.peeked` class holds the band open across that gap, since CSS `:hover` cannot
see a pointer resting on OS chrome. It is a no-op off macOS: the Windows and GTK
caption is non-client area outside the client rect, so toggling it resizes the
content on every hover and the debounced geometry save would persist those
sizes. Off macOS the gear reveal is the whole feature; drawing our own caption
buttons there is the §11 "Header + resize edges" work, still unbuilt.

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

The palette therefore arrives with the stylesheets and not before, which is why
no window is shown until its page has loaded them — see §6.

### 7.4 Window chrome

Three states, mapping onto ta-old's three (`MakeWindow`, `TranslationAggregator.cpp:542`):

| State | ta-old equivalent | Implementation |
|---|---|---|
| **Header + resize edges** (default) | `WS_POPUP \| WS_THICKFRAME` | `decorations: false`; our header is the titlebar; invisible CSS edges call `startResizeDragging` |
| **Content only** | Setsumi's `borderlessWindow` (`WS_POPUP`) | Also hide our header; toggles move to a right-click menu. Maximum real estate over a game. |
| **Native frame** | `WS_OVERLAPPEDWINDOW` | Real OS titlebar and buttons. **Forces background opacity to 100%** — a native titlebar over a transparent body is broken on macOS. |

ta-old toggled by destroying and recreating the window; Tauri does it live.

The chrome commands resolve the main window **by label**, never from the
`tauri::Window` Tauri injects into a command — that is whichever webview
invoked it, and the settings window invokes the same commands, so an injected
caller undecorated the settings window instead.

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
contrast checks: the header's single settings button plus the chips need real
focus states; the settings window's own controls are covered by
`settings.test.ts`.

### Not tested

Always-on-top, transparency, and chrome switching. Manual per-platform
checklist — automating window-manager behaviour is more fragile than the bugs it
would catch.

### Coverage

**80 % on `crates/jparser`**, held as a real line. Not meaningful in `src-tauri`,
which is glue and window plumbing; a repo-wide number would average the two into
something that means nothing.

## 11. Phasing

The six themes below still describe the work, but the unit that actually ships is
the **lettered sub-phase**. Phase 1 split in two, Phase 2 ran to twelve letters
and absorbed both MeCab hints and most of the window behaviour, and Phase 3
started before Phase 2's tail was finished.

Every sub-phase has a design spec in `docs/superpowers/specs/` and a plan in
`docs/superpowers/plans/`, named `<date>-jparser-phase<N><L>[-design].md`; some
also have a handoff in `docs/superpowers/`.

### Phase 1 — Parser core (shipped)

| Sub-phase | Delivered |
|---|---|
| **1A** (08-12) | Character handling, romaji, conjugation table + `Next Type`, JMdict streaming, verb stem generation, mmap FST index. CLI dumps every record matching a query. |
| **1B** (08-13) | Matcher with verb-conjugation recursion, min-cost DP segmenter with ta-old's scoring, conjugation labels, reading reconstruction, public `parse()`. |

### Phase 2 — Minimum viable app (shipped)

Scope grew well past "minimum": MeCab hints arrived here instead of Phase 5, and
so did the chrome and geometry work originally filed under Phase 4.

| Sub-phase | Delivered |
|---|---|
| **2A** (08-13) | Dictionary lifecycle: immutable numbered index generations, headless `ensure_dictionary`, CLI driver. An interrupted rebuild cannot serve well-formed wrong data. |
| **2B** (08-13) | JMdict acquisition in `crates/jmdict-source` — hand-placed file, else download + verify + publish. No HTTP client or decompressor in `crates/jparser`. |
| **2C** (08-14) | MeCab boundary hints: `VibratoTokenizer` behind the optional `mecab` feature, IPADIC field 7 marking token interiors, `jparser-cli parse --hints`. |
| **2D** (08-14) | Tauri shell: `src-tauri` workspace member, `parse_text` on `spawn_blocking`, Vite + vanilla-TS webview, sentence and definition panes. First user-facing surface. |
| **2E** (08-14) | Autonomy: 200 ms clipboard poll into a `watch` channel, one worker, `parse-result` events, always-on-top and clipboard-pause persisted to `settings.json`. |
| **2F** (08-14) | First-run download screen. `AppState` travels to the worker by a second `watch`, so an arriving index starts serving without a restart. |
| **2G** (08-14) | Hover-to-preview popover inside the webview: reused `renderEntry`, `pointer-events: none`, placement as a pure function. |
| **2H** (08-15) | Manual text box deleted — the clipboard becomes the only user-facing input. The Rust command stays as a test and debug entry point. |
| **2I** (08-15) | The popover becomes a real OS window, able to extend past the main window onto the desktop, with ta-old's `MyDrawText` colouring and a cursor-poll keep rule. |
| **2J** (08-15) | Bottom definitions pane removed; the popover window is the only definition surface, and the segmented sentence is the sole output child. |
| **2K** (08-15) | Title-bar (decorations) toggle with native frameless dragging; window size and position persisted across launches. |
| **2L** (08-16) | Hover popovers while the window is unfocused, driven by a native cursor-tracking thread, without stealing focus from the foreground app. |

### Phase 3 — Reading aids (in progress)

| Sub-phase | Status |
|---|---|
| **3A** (08-16) | Shipped. Four furigana modes (`none`/`hiragana`/`katakana`/`romaji`) rendered as `<ruby>` over the chips, persisted. The picker moved to the settings window after 3B; the main window only reacts to `settings-changed` by redrawing. |
| **3B** (08-16) | Shipped. The three real gloss filters of §7.6 (`hide_pos`, `hide_xrefs`, `hide_usage`), persisted, applied in the popover. The settings surface shipped as a **dedicated window**, not the popover of deviation #8 — created hidden in `setup` like the popover, see §6. |
| **3C** | Remaining. Font sizes (§7.7): `--text-cjk` and `--text-furigana` bound to two persisted inputs. The last reading aid still unbuilt. |

### Phase 4 — Window behaviour (remaining, minus what 2K took)

2K already shipped the header + resize-edges state and geometry persistence.
Four sub-phases are left, and their order is a dependency chain, not a
preference: **4C first** because it depends on nothing and is the one users feel;
**4D last** because a slot captures the state 4A, 4B and 3C introduce.

| Sub-phase | Scope | Depends on |
|---|---|---|
| **4C** | Parse history (§7.8): ring buffer of 50 full `ParseResult`s, consecutive duplicates deduped, back/forward in the header. What makes clipboard monitoring safe to leave running. | — |
| **4A** | The other two chrome states (§7.4): content-only, with the toggles moving to a right-click menu, and native frame. Three-way control, persisted alongside `decorations`. | 2K |
| **4B** | Per-pixel background alpha (§7.5): `"transparent": true`, a `--surface-alpha` token and its control, `macos-private-api` on macOS. Native frame forces alpha to 100 %, so 4A defines the state 4B must clamp. | 4A |
| **4D** | Layout hotkeys (§7.9): global-shortcut plugin, `Alt+1..9` restores, `Shift+Alt+1..9` binds. A slot captures geometry, chrome mode, background alpha, always-on-top, font sizes, furigana mode. | 3C, 4A, 4B |

### Phase 5 — MeCab (remaining, minus what 2C took)

The hints themselves are done. What is left is the plumbing that makes them
reachable without a terminal — and it mirrors work already done for JMdict, so
both sub-phases have a shape to copy.

| Sub-phase | Scope | Precedent to copy |
|---|---|---|
| **5A** | On-demand download of the compiled Vibrato dictionary (§4.4), verified and published like an index generation, paid for only when MeCab is switched on. `TA_HINTS_DICT` survives as a test and debug override, no longer the only source. | 2B (acquisition), 2A (immutable publish) |
| **5B** | The toggle: a persisted setting, and hints reaching the running worker over a `watch` so enabling them does not need a restart. Absent dictionary means hints off, not a fatal — 2C's fatal was correct only while the env var was an explicit request. | 2F (`watch` into a live worker), 2E (persisted toggle) |

### Phase 6 — Verification (remaining)

The Playwright suite is not a Phase 6 deliverable: `e2e/` already carries
`panes.spec.ts` and `popover.spec.ts` and grows with each phase, so 6B completes
it rather than starting it.

| Sub-phase | Scope |
|---|---|
| **6A** | Differential run against ta-old (§10): ~200 sentences through both binaries, segmentations diffed, divergences triaged into port bugs vs legitimate JMdict/EDICT2 disagreement. Run once, deliberately, not a CI gate. |
| **6B** | Suite completion: the 80 % line on `crates/jparser` held as a real number, and Playwright coverage extended to what Phases 3–4 added — furigana modes, gloss filters, history navigation — at 320 / 768 / 1024, both themes, reduced-motion, against a fixed fixture. |
| **6C** | Release: Windows and macOS bundles, the per-platform manual checklist for the three things §10 declines to automate (always-on-top, transparency, chrome switching), and an attribution check — EDRDG in-app, GPL v2 (§14). |

6A can run as soon as 3C lands; it tests the parser, which stops changing after
Phase 1. 6C is the last thing that happens.

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
