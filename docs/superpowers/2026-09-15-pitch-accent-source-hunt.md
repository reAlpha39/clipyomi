# The pitch-accent source hunt — one candidate survives, and it is not the one that looked easiest

**Date:** 2026-09-15
**Branch:** `feat/engine-v2`
**Answers:** the second of the two spikes in the
[Phase 3c handoff](2026-09-12-engine-v2-phase3c-handoff.md) §5, and the
`Pitch accent` row of the master plan's Phase 3 table.
**Produces:** a recommendation and a licence finding. No code, no dependency,
no data downloaded.

## 1. What was asked

The Phase 3c handoff carried the hunt forward as two candidates "whose
licences need reading first — UniDic's accent fields under the BSD leg of its
triple licence, and Wadoku under CC BY-SA 3.0". The Phase 3 table's own row
has said since the master plan was written that this layer is the **highest
risk** of the four, because "NHK's dictionary is not free, and community
accent datasets have murky provenance — precisely the trap this plan exists to
escape."

The gate is the plan's, unchanged: **no layer ships without its licence
established in writing first**, and §3.4 rules out adopting anything whose
provenance traces somewhere it cannot follow.

## 2. Wadoku — the premise was wrong, and it fails

**Wadoku's data is not under CC BY-SA 3.0, or any Creative Commons licence.**
The belief is easy to acquire — third-party descriptions of Wadoku say
"licensed under CC BY-SA by Ulrich Apel and parts by WaDoku e.V.", and several
redistributions repeat it — but the rights holder's own pages say something
else, and they are the ones that count.

Two licences are in force, and the download page points at both:

- **`WaDoku-Datei-LIZENZ` (Ulrich Apel, 2001)**, which covers the original
  dictionary file. Section **g**: the data may not be embedded in software
  packages that are sold commercially without prior authorisation. Section
  **e**: they may not be served from WWW servers that charge for access
  without the copyright holder's prior permission. Permission is described as
  normally granted, **contingent on a share of revenue**.
- **`Wadoku.de-Daten Lizenz Version 1.0` (10 August 2011)**, which covers
  contributions made through the site — copyright resting with Ulrich Apel and
  the co-author community. Free redistribution for personal use, research,
  unmodified copies carrying the licence, non-commercial distribution at media
  cost, and integration into free software; **explicit written approval from
  both Ulrich Apel and Wadoku e.V.** is required for inclusion in commercial
  software, fee- or advertising-funded web distribution, printed or CD-ROM
  publication, and shareware.

A third document, *(Vorschlag) Neue Wadoku-Daten Lizenz*, states on its face
"Diese Lizenz ist Work in Progress und noch **NICHT gültig**" — work in
progress and not yet valid. So there is no pending CC relicensing to wait for
either.

**This fails the gate on the first clause of both licences.** A
permission-on-request term is not a licence this project can rely on: the
target is `MIT OR Apache-2.0`, the app is distributed as a binary that anyone
may sell or fork, and a field-of-use restriction on the data would propagate
to every such downstream. "Normally granted contingent on profit-sharing" is a
negotiation, not a written permission, and §3.4's whole point is that the
project does not adopt what it cannot state in writing.

**The finding worth keeping is not the refusal but the shape of the error.**
The CC BY-SA 3.0 attribution came from redistributors rather than from the
rights holder, and it was carried forward into this project's own plan as if
it were established. That is *exactly* the trap the Phase 3 table warns about,
found this time in the plan's own text rather than in a dataset. The rule it
argues for: **read the rights holder's page, not the packager's**, for every
data source, including the ones already believed settled.

## 3. UniDic — it passes, and the plan already knew

**UniDic is a genuine triple licence, and the BSD leg is selectable.** NINJAL's
commercial-use page states it directly:

> Ver.2.x以降の現代書き言葉UniDic（旧名：unidic-mecab）および、Ver.2.2.x以降の
> 現代話し言葉UniDicは、完全なフリーソフトウェアになりました。
> **GPL v2.0／LGPL v2.1／修正BSD のトリプルライセンス**

Any one of the three, or a combination, may be chosen, and commercial use is
free under them. Older UniDic releases carrying other licences are excluded
and require prior consultation — so the version matters, and a phase would
name it. Attribution is expected when results are published.

This is not new information to the plan. **§3.3's source table already reads
"UniDic (NINJAL) | GPL-2 *or* LGPL-2.1 *or* BSD New | Take under BSD"** — the
spike's job here was to confirm that the accent fields travel with the same
licence as the rest of the dictionary, and they do: they are columns of the
same lexicon, not a separately licensed layer.

**The accent columns exist and are documented.** UniDic's lexicon carries
`aType`, `aConType` and `aModType` alongside the fields §9.6 already read:

| field | what it holds |
| --- | --- |
| `aType` | accent type — the mora number carrying the accent in 標準語, comma-separated when a word has more than one accepted pattern, more common first |
| `aConType` | how the accent shifts when the word takes a counter |
| `aModType` | accent-related; its use is not clearly documented |

A worked precedent for the BSD leg exists in the wild: `polm/unidic-py` ships
UniDic 3.1.0 (770 MB on disk) and **redistributes it under the BSD licence**,
naming the triple and its choice.

## 4. What this does *not* settle

Three things, and the first is the one that decides whether a phase is worth
starting.

**Coverage is unmeasured.** How many JMdict `(headword, reading)` pairs a
UniDic-derived accent table would actually reach is unknown, and it is the
number that decides the layer — a table that covers the common vocabulary a
learner hovers is worth a phase, one that covers the lemma list but misses the
inflected surfaces the popover shows is not. The join is UniDic's
`lemma` / `lForm` / `pron` against JMdict's kanji and kana forms, and whether
it is clean is an empirical question about two independently built word lists.
**No phase should start before that figure exists**, on the same principle as
§5.2's gates: every other Phase 3 layer was decided by a number.

**Which release carries the columns is unconfirmed here.** UniDic 3.1.x
documents all three accent fields. Whether `unidic-mecab-2.1.2` — the release
§9.6 measured at 199 MB and declined as a hints source — carries them too was
not established, and the answer changes the download size a build would pay.

**`aModType`'s meaning is not documented anywhere reachable.** If the layer
ships, it ships `aType` and possibly `aConType`; a field whose use cannot be
stated is not a field this project renders.

## 5. The shape a phase would take, if the coverage number justifies one

Recorded so the recommendation is actionable, not as a design:

- **Phase 3a's shape, not §9.6's.** The wrong move is swapping the hints
  dictionary to UniDic to read accent off the tokenizer: §9.6 declined that on
  quality (one span worse on the gold corpus, slightly worse entry top-1) and
  on size (199 MB against IPADIC's 46), and none of that changes because a new
  column became interesting. The right move is what Phase 3a did for
  KANJIDIC — derive a static table at build time, carry it as an index file,
  reach it lazily on hover.
- **The 576 MB is a developer cost, not a user download.** UniDic
  3.1.0's modern-written distribution is 576 MB without `model.def` and
  `matrix.def` (2.1 GB with). That is paid once, in the repository, to produce
  a compact `(reading, accent)` table; the artefact the app ships is that
  table. JMdict and KANJIDIC2 are fetched at runtime because EDRDG publishes
  archives sized for it — UniDic is not, and should not be.
- **Attribution goes where Phase 3a's went.** The settings footer already
  carries the app's first persistent attribution, for EDRDG under CC BY-SA
  4.0. A BSD-3-Clause notice for UniDic sits beside it; a permissive data
  licence next to a share-alike one is exactly the case that footer was built
  to hold.
- **No new dependency.** Vibrato is already in-tree under `MIT OR Apache-2.0`
  and the project already reads comma-separated feature columns by index —
  `READING_FIELD` at `crates/jparser/src/bin/jparser-cli/diff.rs:31`, and §9.6
  read UniDic's own reading at column 9 through the same mechanism. Whatever
  reads `aType` is the code that already exists, pointed at another column.

## 6. Recommendation

1. **Drop Wadoku from the candidate list, and correct the record.** It is not
   CC BY-SA 3.0; both licences in force restrict commercial use behind
   permission, and the proposed replacement is explicitly not valid. The
   master plan's Phase 3 table and the Phase 3c handoff both name it as a
   candidate on the CC BY-SA premise, and both are amended by this document.
2. **Keep UniDic, taken under the modified-BSD leg**, as the only source that
   has passed a written-licence reading. It is the plan's own §3.3 choice and
   nothing about the accent columns changes it.
3. **Do not open a phase yet. Measure coverage first** — JMdict
   `(headword, reading)` pairs reached by a UniDic accent table, and the
   cleanliness of the join. That is a spike of its own, cheaper than a phase
   and the same shape as this one, and it is what the Phase 3 table's gate
   asks for.
4. **Do not touch `kanjium` or any community accent dataset.** It declares
   CC BY-SA 4.0 over 124,137 accent entries and **states no source for them**.
   It is the most widely used accent dataset in the hobbyist tooling around
   this app's problem, which is precisely why naming it here matters: a
   declared licence over undeclared provenance is what §3.4 exists to refuse,
   and the refusal does not depend on any belief about where the data came
   from.
5. **NHK's accent dictionary remains out**, unchanged and for the original
   reason: it is not free, and no reading of it was needed to say so.

## 7. Open items

- **Nothing was downloaded**, so every size in §5 is quoted from the
  publisher's own page rather than measured. The coverage spike will produce
  measured figures and should re-quote them.
- **The Japanese licence text is authoritative and was read in Japanese**;
  the English in §3 is a summary, not a translation, and a phase that ships
  the layer should quote NINJAL's own wording in its NOTICE rather than this
  document's.
- **`unidic-lite` was not evaluated.** It is the small-package alternative to
  the full UniDic and its field coverage — specifically whether it carries the
  accent columns — is undocumented in its own README. If it does, the
  developer-side download cost in §5 falls a long way, so the coverage spike
  should check it before assuming 576 MB.
