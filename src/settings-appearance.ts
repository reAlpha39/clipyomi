// SPDX-License-Identifier: MIT OR Apache-2.0

import { invoke } from '@tauri-apps/api/core';
import { FONT_LIMITS } from './font-sizes';
import {
  COLOUR_KEYS,
  COLOUR_KINDS,
  TEXT_COLOUR_KINDS,
  clampColour,
  stylesheetDefault,
  type ColourKind,
} from './text-colours';
import { FAMILY_KEYS, clampFamily, type FamilyKind } from './text-fonts';
import type { FuriganaMode, Settings } from './types';

/**
 * The appearance controls of the settings window: three font selects, five
 * colour swatches with a Reset each, and the outline checkbox. Split out of
 * `settings.ts` so that file stays under the ceiling; the markup and the
 * behaviour live together here so a row cannot drift from its handler.
 */

const FAMILY_KINDS: readonly FamilyKind[] = ['sentence', 'furigana', 'gloss'];

const FAMILY_LABELS: Record<FamilyKind, string> = {
  sentence: 'Sentence',
  furigana: 'Furigana',
  gloss: 'Definitions',
};

const COLOUR_LABELS: Record<ColourKind, string> = {
  kanji: 'Kanji words',
  kana: 'Kana words',
  particle: 'Particles',
  counter: 'Counters',
  furigana: 'Furigana',
  unmatched: 'Unmatched text',
  bg: 'Background',
};

function familyRow(kind: FamilyKind): string {
  return `
        <label class="settings-select-row">
          <span class="label-text">${FAMILY_LABELS[kind]}</span>
          <select id="setting-font-family-${kind}"><option value="">Default</option></select>
        </label>`;
}

function colourRow(kind: ColourKind): string {
  return `
        <div class="settings-colour-row">
          <label class="label-text" for="setting-colour-${kind}">${COLOUR_LABELS[kind]}</label>
          <input type="color" id="setting-colour-${kind}" />
          <button type="button" class="settings-reset" id="setting-colour-${kind}-reset"
                  aria-label="Reset ${COLOUR_LABELS[kind].toLowerCase()} colour">Reset</button>
        </div>`;
}

/** The Fonts and Text Colours sections, for the page template. */
export const APPEARANCE_SECTIONS = `
      <section class="settings-group">
        <h2 class="settings-group-title">Fonts</h2>
        ${FAMILY_KINDS.map(familyRow).join('')}
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Text Colours</h2>
        ${TEXT_COLOUR_KINDS.map(colourRow).join('')}
      </section>`;

/**
 * The window background swatch, for the top of the Background section.
 *
 * Rendered here rather than in `settings.ts` so one function builds every
 * colour row; wired by `initAppearanceControls` with the six text colours,
 * which find it by id wherever the page puts it.
 */
export const BG_COLOUR_ROW = colourRow('bg');

/**
 * The outline and drop-shadow rows, for the Background section beside the
 * opacity slider. Each carries a size box alongside its checkbox.
 *
 * A `div` wrapping a `label`, not a `label` wrapping everything: a number
 * input inside the label would sit inside the checkbox's own hit area, and
 * clicking to type would toggle the setting. The boxes are wired in
 * `settings.ts` with the other sizes, whose flow they ride.
 */
export const OUTLINE_ROW = `
        <div class="settings-checkbox-row">
          <label class="checkbox-label">
            <input type="checkbox" id="setting-text-outline" />
            <span class="label-text">Outline text (helps at low opacity)</span>
          </label>
          <input type="number" id="setting-text-outline-width"
                 aria-label="Outline width in pixels"
                 min="${FONT_LIMITS.outline.min}" max="${FONT_LIMITS.outline.max}" step="1" />
          <span class="unit">px</span>
        </div>
        <div class="settings-checkbox-row">
          <label class="checkbox-label">
            <input type="checkbox" id="setting-text-shadow" />
            <span class="label-text">Drop shadow</span>
          </label>
          <input type="number" id="setting-text-shadow-size"
                 aria-label="Drop shadow size in pixels"
                 min="${FONT_LIMITS.shadow.min}" max="${FONT_LIMITS.shadow.max}" step="1" />
          <span class="unit">px</span>
        </div>`;

export interface AppearanceHooks {
  /** Persist and broadcast the full snapshot. */
  save(): void;
  /** Send a partial payload to the main window alone, for a live preview. */
  preview(payload: Partial<Settings>): void;
}

export interface AppearanceControls {
  /**
   * The furigana mode decides what the furigana select offers: every family
   * for romaji, Japanese-capable ones otherwise. Called by `settings.ts`
   * wherever it moves the mode buttons, so a click and an incoming snapshot
   * both refill the select.
   */
  setFuriganaMode(mode: FuriganaMode): void;
  /** The keys this module owns, only those that are set. */
  snapshot(): Partial<Settings>;
  /**
   * Mirror an incoming payload. A full snapshot owns every control, and a
   * key gone from it means unset; a partial payload touches only the keys
   * it carries — the same discrimination `settings.ts` makes for the sizes.
   */
  apply(settings: Partial<Settings>, isFullSnapshot: boolean): void;
}

export function initAppearanceControls(
  container: HTMLElement,
  hooks: AppearanceHooks,
): AppearanceControls {
  const swatches = Object.fromEntries(
    COLOUR_KINDS.map((kind) => [
      kind,
      container.querySelector<HTMLInputElement>(`#setting-colour-${kind}`)!,
    ]),
  ) as Record<ColourKind, HTMLInputElement>;
  const selects = Object.fromEntries(
    FAMILY_KINDS.map((kind) => [
      kind,
      container.querySelector<HTMLSelectElement>(`#setting-font-family-${kind}`)!,
    ]),
  ) as Record<FamilyKind, HTMLSelectElement>;
  const outline = container.querySelector<HTMLInputElement>('#setting-text-outline')!;
  const shadow = container.querySelector<HTMLInputElement>('#setting-text-shadow')!;

  // The colours the user has actually picked. A colour input always holds a
  // value, so the input itself cannot say "unset": while a kind is absent
  // here its swatch shows the theme default and the snapshot omits it.
  const customColours = new Map<ColourKind, string>();
  const families = new Map<FamilyKind, string>();

  function seedSwatch(kind: ColourKind) {
    const fallback = stylesheetDefault(kind);
    if (fallback !== null) swatches[kind].value = fallback;
  }

  for (const kind of COLOUR_KINDS) {
    const swatch = swatches[kind];
    const key = COLOUR_KEYS[kind];
    seedSwatch(kind);

    // `input` fires continuously while the user drags inside the native
    // picker: a partial payload to the main window, no disk write, exactly
    // as the opacity slider does. `change` fires once the picker closes.
    swatch.addEventListener('input', () => {
      const hex = clampColour(swatch.value);
      if (hex === null) return;
      customColours.set(kind, hex);
      hooks.preview({ [key]: hex });
    });
    swatch.addEventListener('change', () => {
      const hex = clampColour(swatch.value);
      if (hex === null) return;
      customColours.set(kind, hex);
      hooks.save();
    });
    container
      .querySelector<HTMLButtonElement>(`#setting-colour-${kind}-reset`)!
      .addEventListener('click', () => {
        customColours.delete(kind);
        seedSwatch(kind);
        hooks.save();
      });
  }

  // `document.createElement`, not `new Option(...)`: the test DOM has no
  // `Option` constructor, and a `ReferenceError` inside the `system_fonts`
  // `.then` would vanish into its `.catch`.
  function option(label: string, value: string): HTMLOptionElement {
    const el = document.createElement('option');
    el.value = value;
    el.textContent = label;
    return el;
  }

  /**
   * Show KIND's current family, appending an option for it when the list
   * does not carry it: a family picked on another machine, or since
   * uninstalled, is still what the file says and what the main window
   * applies, so the select must not silently read as Default.
   */
  function showFamily(kind: FamilyKind) {
    const select = selects[kind];
    const name = families.get(kind) ?? '';
    if (name !== '' && ![...select.options].some((o) => o.value === name)) {
      select.append(option(name, name));
    }
    select.value = name;
  }

  /** What `system_fonts` answered, kept so a mode change can refill a select. */
  let installed: Array<{ name: string; japanese: boolean }> = [];
  let furiganaMode: FuriganaMode = 'none';

  /**
   * Whether KIND's select needs Japanese glyphs. A Latin-only family applied
   * to Japanese text falls back glyph by glyph to the system CJK font, and
   * the pick looks like nothing happened — so the sentence, and the furigana
   * while it is kana, list only families that cover the script. Romaji is
   * Latin text and definitions are English: every family stands there.
   */
  function needsJapanese(kind: FamilyKind): boolean {
    if (kind === 'sentence') return true;
    return kind === 'furigana' && furiganaMode !== 'romaji';
  }

  function fillSelect(kind: FamilyKind) {
    const offered = needsJapanese(kind) ? installed.filter((f) => f.japanese) : installed;
    selects[kind].replaceChildren(
      option('Default', ''),
      ...offered.map((f) => option(f.name, f.name)),
    );
    showFamily(kind);
  }

  function populateFamilies(list: unknown) {
    if (!Array.isArray(list)) return;
    installed = list.flatMap((entry: unknown) => {
      if (typeof entry !== 'object' || entry === null) return [];
      const { name, japanese } = entry as { name?: unknown; japanese?: unknown };
      const clean = clampFamily(name);
      return clean === null || typeof japanese !== 'boolean' ? [] : [{ name: clean, japanese }];
    });
    for (const kind of FAMILY_KINDS) fillSelect(kind);
  }

  for (const kind of FAMILY_KINDS) {
    selects[kind].addEventListener('change', () => {
      const name = clampFamily(selects[kind].value);
      if (name === null) families.delete(kind);
      else families.set(kind, name);
      hooks.save();
    });
  }

  // Enumerated once by the backend and cached there; an empty or failed
  // answer leaves each select showing Default and whatever is persisted.
  void invoke<unknown>('system_fonts').then(populateFamilies).catch(() => {});

  outline.addEventListener('change', hooks.save);
  shadow.addEventListener('change', hooks.save);

  return {
    setFuriganaMode(mode) {
      if (mode === furiganaMode) return;
      furiganaMode = mode;
      fillSelect('furigana');
    },

    snapshot() {
      const out: Partial<Settings> = {
        text_outline: outline.checked,
        text_shadow: shadow.checked,
      };
      for (const kind of COLOUR_KINDS) {
        const hex = customColours.get(kind);
        if (hex !== undefined) out[COLOUR_KEYS[kind]] = hex as never;
      }
      for (const kind of FAMILY_KINDS) {
        const name = families.get(kind);
        if (name !== undefined) out[FAMILY_KEYS[kind]] = name as never;
      }
      return out;
    },

    apply(settings, isFullSnapshot) {
      for (const kind of COLOUR_KINDS) {
        const hex = clampColour(settings[COLOUR_KEYS[kind]]);
        if (hex !== null) {
          customColours.set(kind, hex);
          swatches[kind].value = hex;
        } else if (isFullSnapshot) {
          customColours.delete(kind);
          seedSwatch(kind);
        }
      }
      for (const kind of FAMILY_KINDS) {
        const name = clampFamily(settings[FAMILY_KEYS[kind]]);
        if (name !== null) families.set(kind, name);
        else if (isFullSnapshot) families.delete(kind);
        if (name !== null || isFullSnapshot) showFamily(kind);
      }
      if (settings.text_outline !== undefined) outline.checked = settings.text_outline;
      if (settings.text_shadow !== undefined) shadow.checked = settings.text_shadow;
    },
  };
}
