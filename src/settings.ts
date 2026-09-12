// SPDX-License-Identifier: MIT OR Apache-2.0

import { invoke } from '@tauri-apps/api/core';
import { emit, emitTo, listen } from '@tauri-apps/api/event';
import { FONT_LIMITS, clampFont, stylesheetDefault, type FontKind } from './font-sizes';
import { ALPHA_LIMITS, clampAlpha } from './surface-alpha';
import { APPEARANCE_SECTIONS, OUTLINE_ROW, initAppearanceControls } from './settings-appearance';
import type { ChromeMode, FuriganaMode, Settings } from './types';
import './styles/global.css';
import './styles/settings.css';

/**
 * The hints note's base sentence — what downloads, and how big. Shared by the
 * static template below and the dynamic note `hints_dir` fills in once it
 * answers, so the two do not drift apart.
 */
const HINTS_NOTE_BASE = 'Improves word splitting. Enabling downloads an 8 MB IPADIC dictionary.';

export function initSettingsApp(container: HTMLElement) {
  container.innerHTML = `
    <div class="settings-page">
      <header class="settings-header">
        <h1>ClipYomi Settings</h1>
      </header>

      <section class="settings-group">
        <h2 class="settings-group-title">Gloss &amp; Definitions</h2>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-hide-pos" />
          <span class="label-text">Hide parts of speech (POS)</span>
        </label>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-hide-xrefs" />
          <span class="label-text">Hide cross-references</span>
        </label>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-hide-usage" />
          <span class="label-text">Hide usage notes &amp; misc</span>
        </label>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-hide-kanji" />
          <span class="label-text">Hide kanji readings &amp; meanings</span>
        </label>
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Furigana Display</h2>
        <div class="segmented-control" role="radiogroup" aria-label="Furigana mode">
          <button type="button" role="radio" data-mode="none" aria-checked="true" title="No furigana">—</button>
          <button type="button" role="radio" data-mode="hiragana" aria-checked="false" title="Hiragana furigana">あ</button>
          <button type="button" role="radio" data-mode="katakana" aria-checked="false" title="Katakana furigana">カ</button>
          <button type="button" role="radio" data-mode="romaji" aria-checked="false" title="Romaji phonetic">R</button>
        </div>
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Text Size</h2>
        <label class="settings-number-row">
          <span class="label-text">Sentence</span>
          <input type="number" id="setting-font-sentence"
                 min="${FONT_LIMITS.sentence.min}" max="${FONT_LIMITS.sentence.max}" step="1" />
          <span class="unit">px</span>
        </label>
        <label class="settings-number-row">
          <span class="label-text">Definitions</span>
          <input type="number" id="setting-font-gloss"
                 min="${FONT_LIMITS.gloss.min}" max="${FONT_LIMITS.gloss.max}" step="1" />
          <span class="unit">px</span>
        </label>
        <label class="settings-number-row">
          <span class="label-text">Furigana</span>
          <input type="number" id="setting-furigana-scale"
                 min="${FONT_LIMITS.furigana.min}" max="${FONT_LIMITS.furigana.max}" step="1" />
          <span class="unit">% of sentence</span>
        </label>
      </section>
      ${APPEARANCE_SECTIONS}

      <section class="settings-group">
        <h2 class="settings-group-title">Segmentation</h2>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-mecab-hints" />
          <span class="label-text">Boundary hints (MeCab)</span>
        </label>
        <p class="settings-note" id="setting-mecab-hints-note">
          ${HINTS_NOTE_BASE}
        </p>
        <p class="settings-note" id="setting-mecab-hints-status" hidden></p>
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Window &amp; Behavior</h2>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-always-on-top" />
          <span class="label-text">Always on top</span>
        </label>
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-clipboard-monitoring" />
          <span class="label-text">Clipboard monitoring</span>
        </label>
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Window Chrome</h2>
        <div class="segmented-control" role="radiogroup" aria-label="Window chrome">
          <button type="button" role="radio" data-chrome-mode="native" aria-checked="true" title="Native frame: the OS draws the title bar">Native</button>
          <button type="button" role="radio" data-chrome-mode="header" aria-checked="false" title="Header: our band, revealed on hover">Header</button>
          <button type="button" role="radio" data-chrome-mode="content" aria-checked="false" title="Content only: nothing above the sentence">Content</button>
        </div>
      </section>

      <section class="settings-group">
        <h2 class="settings-group-title">Background</h2>
        <label class="settings-range-row">
          <span class="label-text">Opacity</span>
          <input type="range" id="setting-surface-alpha"
                 min="${ALPHA_LIMITS.min}" max="${ALPHA_LIMITS.max}" step="1" />
          <output class="unit" id="setting-surface-alpha-value">100%</output>
        </label>
        <p class="settings-note" id="setting-surface-alpha-note" hidden>
          The native window frame is always fully opaque.
        </p>
        ${OUTLINE_ROW}
      </section>

      <footer class="settings-footer">
        <p class="settings-note">
          Dictionary data: JMdict and KANJIDIC2 &copy; Electronic Dictionary Research and Development Group,
          CC BY-SA 4.0. Licence: https://www.edrdg.org/edrdg/licence.html
        </p>
      </footer>
    </div>
  `;

  const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
  const hideXrefs = container.querySelector<HTMLInputElement>('#setting-hide-xrefs')!;
  const hideUsage = container.querySelector<HTMLInputElement>('#setting-hide-usage')!;
  const hideKanji = container.querySelector<HTMLInputElement>('#setting-hide-kanji')!;
  const alwaysOnTop = container.querySelector<HTMLInputElement>('#setting-always-on-top')!;
  const monitoring = container.querySelector<HTMLInputElement>('#setting-clipboard-monitoring')!;
  const fontSentence = container.querySelector<HTMLInputElement>('#setting-font-sentence')!;
  const fontGloss = container.querySelector<HTMLInputElement>('#setting-font-gloss')!;
  const furiganaScale = container.querySelector<HTMLInputElement>('#setting-furigana-scale')!;
  const furiganaButtons = container.querySelectorAll<HTMLButtonElement>(
    '.segmented-control button[data-mode]',
  );
  const surfaceAlpha = container.querySelector<HTMLInputElement>('#setting-surface-alpha')!;
  const surfaceAlphaValue = container.querySelector<HTMLOutputElement>(
    '#setting-surface-alpha-value',
  )!;
  const surfaceAlphaNote = container.querySelector<HTMLElement>(
    '#setting-surface-alpha-note',
  )!;
  const mecabHints = container.querySelector<HTMLInputElement>('#setting-mecab-hints')!;
  const mecabNote = container.querySelector<HTMLElement>('#setting-mecab-hints-note')!;
  const mecabStatus = container.querySelector<HTMLElement>('#setting-mecab-hints-status')!;

  let currentMode: FuriganaMode = 'none';

  // Wired before the handlers below are declared, which is fine: both hooks
  // are function declarations in this scope, hoisted, and only ever called
  // from an event. `preview` targets `main` alone for the reason the
  // opacity slider's `input` handler gives.
  const appearance = initAppearanceControls(container, {
    save: () => saveAndBroadcast(),
    preview: (payload) => {
      void emitTo('main', 'settings-changed', payload).catch(() => {});
    },
  });

  const chromeButtons = container.querySelectorAll<HTMLButtonElement>('[data-chrome-mode]');

  // No default mode: until `get_settings` answers, this window does not know
  // which chrome the main window is wearing, and inventing `native` here is how
  // a snapshot sent before that answer could name a mode the platform cannot
  // draw. `currentSnapshot` omits the key instead, which every consumer already
  // treats as "unchanged" — the same way an empty font-size box does.
  let currentChrome: ChromeMode | undefined;

  function updateChromeButtons(mode: ChromeMode) {
    currentChrome = mode;
    chromeButtons.forEach((btn) => {
      btn.setAttribute('aria-checked', String(btn.dataset.chromeMode === mode));
    });
    // The native frame renders fully opaque whatever is stored, so the slider
    // would move nothing there. It is disabled and the reason is stated in
    // visible text rather than a `title`: a disabled control leaves the tab
    // order, so a tooltip on it is reachable by mouse alone, and the wrapping
    // label already owns the accessible name so the title is never announced.
    const opaqueByChrome = mode === 'native';
    surfaceAlpha.disabled = opaqueByChrome;
    surfaceAlphaNote.hidden = !opaqueByChrome;
  }

  function updateFuriganaButtons(mode: FuriganaMode) {
    currentMode = mode;
    furiganaButtons.forEach((btn) => {
      const match = btn.getAttribute('data-mode') === mode;
      btn.setAttribute('aria-checked', String(match));
    });
    appearance.setFuriganaMode(mode);
  }

  /**
   * The backend's own phase labels. Anything not here is an error message to
   * show verbatim — the same lookup-not-enum shape `main.ts` uses for
   * `dictionary-status`, and for the same reason: the failure arm carries text
   * the user needs to read.
   */
  const HINTS_PHASES: Record<string, string> = {
    downloading: 'Downloading the dictionary…',
    unpacking: 'Unpacking the archive…',
  };

  function showHintsStatus(message: string) {
    mecabStatus.textContent = message;
    mecabStatus.hidden = message === '';
  }

  /**
   * The checkbox is never the source of truth: this is. A toggle disables the
   * control and invokes; what it ends up showing is whatever the backend says
   * happened.
   */
  function applyHintsStatus(status: string) {
    const phase = HINTS_PHASES[status];
    if (phase !== undefined) {
      // Left exactly as the click set it: the work is still running, and
      // flipping it here would fight the user's own gesture mid-fetch.
      showHintsStatus(phase);
      return;
    }
    mecabHints.disabled = false;
    if (status === 'ready') {
      mecabHints.checked = true;
      showHintsStatus('');
      return;
    }
    if (status === 'off') {
      mecabHints.checked = false;
      showHintsStatus('');
      return;
    }
    // A failure. The backend persisted nothing, so the checkbox must not read
    // as on — and the message is the only explanation the user gets.
    mecabHints.checked = false;
    showHintsStatus(status);
  }

  mecabHints.addEventListener('change', () => {
    mecabHints.disabled = true;
    showHintsStatus('');
    void invoke('set_mecab_hints', { enabled: mecabHints.checked }).catch((e) => {
      // Only reached when the work could not be *started* — no `hints-status`
      // event will arrive, so this is the one path that has to re-enable the
      // control itself.
      mecabHints.disabled = false;
      mecabHints.checked = false;
      showHintsStatus(String(e));
    });
  });

  function showAlpha(value: number) {
    surfaceAlphaValue.textContent = `${value}%`;
  }

  /**
   * Seed the slider from the stylesheet's own opacity.
   *
   * The font boxes detect "nothing persisted" by being empty and fill in the
   * default afterwards. A range input always carries a value — the midpoint,
   * by default — so it has to be seeded up front and overwritten if
   * `get_settings` turns out to carry one.
   */
  function seedAlphaFromStylesheet() {
    const raw = getComputedStyle(document.documentElement).getPropertyValue('--surface-alpha');
    const ratio = Number.parseFloat(raw);
    const seeded = clampAlpha(Number.isFinite(ratio) ? ratio * 100 : ALPHA_LIMITS.max);
    const value = seeded ?? ALPHA_LIMITS.max;
    surfaceAlpha.value = String(value);
    showAlpha(value);
  }

  seedAlphaFromStylesheet();

  /**
   * The size in INPUT, pinned into range, or `undefined` for an empty box —
   * which means "leave the stylesheet default alone", not "size zero".
   *
   * Clamping here as well as at the apply site is not belt and braces: `min`
   * and `max` bound the steppers, but a typed or pasted value walks straight
   * past them and would otherwise be what gets persisted.
   */
  function sizeOf(input: HTMLInputElement, kind: FontKind): number | undefined {
    if (input.value.trim() === '') return undefined;
    const size = clampFont(kind, input.valueAsNumber);
    if (size === null) return undefined;
    input.value = String(size);
    return size;
  }

  function currentSnapshot(): Settings {
    return {
      always_on_top: alwaysOnTop.checked,
      clipboard_monitoring: monitoring.checked,
      chrome_mode: currentChrome,
      furigana_mode: currentMode,
      hide_pos: hidePos.checked,
      hide_xrefs: hideXrefs.checked,
      hide_usage: hideUsage.checked,
      hide_kanji: hideKanji.checked,
      font_sentence: sizeOf(fontSentence, 'sentence'),
      font_gloss: sizeOf(fontGloss, 'gloss'),
      furigana_scale: sizeOf(furiganaScale, 'furigana'),
      surface_alpha: clampAlpha(Number(surfaceAlpha.value)) ?? ALPHA_LIMITS.max,
      ...appearance.snapshot(),
    };
  }

  function broadcast(payload: Partial<Settings>) {
    void emit('settings-changed', payload).catch(() => {});
  }

  function saveAndBroadcast() {
    const snapshot = currentSnapshot();
    void invoke('save_settings', { settings: snapshot }).catch(() => {});
    broadcast(snapshot);
  }

  function applySettings(s: Partial<Settings>) {
    if (s.hide_pos !== undefined) hidePos.checked = s.hide_pos;
    if (s.hide_xrefs !== undefined) hideXrefs.checked = s.hide_xrefs;
    if (s.hide_usage !== undefined) hideUsage.checked = s.hide_usage;
    if (s.hide_kanji !== undefined) hideKanji.checked = s.hide_kanji;
    if (s.always_on_top !== undefined) alwaysOnTop.checked = s.always_on_top;
    if (s.clipboard_monitoring !== undefined) monitoring.checked = s.clipboard_monitoring;
    if (s.mecab_hints !== undefined) mecabHints.checked = s.mecab_hints;
    if (s.chrome_mode !== undefined) updateChromeButtons(s.chrome_mode);
    if (s.furigana_mode !== undefined) updateFuriganaButtons(s.furigana_mode);
    // Full snapshots own the boxes outright, including emptying them: an
    // absent size in a full payload means the user cleared that box, or a
    // restored layout slot recorded it as unset. A partial payload —
    // `{ surface_alpha }` from a slider drag, `{ chrome_mode }` from the main
    // window's menu — carries no sizes at all, and clearing on those would
    // reproduce 4B ruling 1's bug on this surface. Discriminated by the two
    // keys `Settings` does not mark optional, exactly as `main.ts` does.
    const isFullSnapshot =
      s.always_on_top !== undefined && s.clipboard_monitoring !== undefined;
    if (isFullSnapshot) {
      fontSentence.value = s.font_sentence === undefined ? '' : String(s.font_sentence);
      fontGloss.value = s.font_gloss === undefined ? '' : String(s.font_gloss);
      furiganaScale.value = s.furigana_scale === undefined ? '' : String(s.furigana_scale);
    }
    appearance.apply(s, isFullSnapshot);
    if (s.surface_alpha !== undefined) {
      const value = clampAlpha(s.surface_alpha) ?? ALPHA_LIMITS.max;
      surfaceAlpha.value = String(value);
      showAlpha(value);
    }
  }

  fontSentence.addEventListener('change', saveAndBroadcast);
  fontGloss.addEventListener('change', saveAndBroadcast);
  furiganaScale.addEventListener('change', saveAndBroadcast);

  hidePos.addEventListener('change', saveAndBroadcast);
  hideXrefs.addEventListener('change', saveAndBroadcast);
  hideUsage.addEventListener('change', saveAndBroadcast);
  hideKanji.addEventListener('change', saveAndBroadcast);

  alwaysOnTop.addEventListener('change', () => {
    void invoke('set_always_on_top', { enabled: alwaysOnTop.checked }).catch(() => {});
    saveAndBroadcast();
  });

  monitoring.addEventListener('change', () => {
    void invoke('set_clipboard_monitoring', { enabled: monitoring.checked }).catch(() => {});
    saveAndBroadcast();
  });

  chromeButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = btn.dataset.chromeMode as ChromeMode;
      if (!mode) return;
      updateChromeButtons(mode);
      void invoke('set_chrome_mode', { mode }).catch(() => {});
      saveAndBroadcast();
    });
  });

  /**
   * Windows has no native-frame state (spec §4), so the button naming it goes.
   *
   * Removed rather than disabled: the backend resolves `native` to `header`
   * there, so a disabled-but-present button would sit next to a control showing
   * a mode the window is not in. `querySelectorAll` above is a static NodeList,
   * so the removed node simply stops being found by the loops.
   */
  void invoke<string>('platform')
    .then((p) => {
      if (p === 'windows') container.querySelector('[data-chrome-mode="native"]')?.remove();
    })
    .catch(() => {});

  furiganaButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = btn.getAttribute('data-mode') as FuriganaMode;
      if (mode) {
        updateFuriganaButtons(mode);
        saveAndBroadcast();
      }
    });
  });

  surfaceAlpha.addEventListener('input', () => {
    const value = clampAlpha(Number(surfaceAlpha.value)) ?? ALPHA_LIMITS.max;
    showAlpha(value);
    // A partial payload, not `currentSnapshot()`: this fires once per frame
    // during a drag, and the main window's `settings-changed` listener would
    // otherwise re-run the gloss merge and the furigana comparison — which
    // re-renders the whole sentence — for a value neither of them contains.
    // `main.ts` already emits a bare `{ chrome_mode }` the same way.
    //
    // Targeted at `main` specifically, unlike `broadcast`/`saveAndBroadcast`
    // below: `emit` reaches every window, including this settings window's
    // own `settings-changed` listener, which would then write the dragged
    // value back onto the slider the user is physically moving — one IPC
    // round trip stale each time. The main window is the only intended
    // consumer of this partial payload, so it is addressed directly instead.
    void emitTo('main', 'settings-changed', { surface_alpha: value }).catch(() => {});
  });

  surfaceAlpha.addEventListener('change', saveAndBroadcast);

  /**
   * Show the stylesheet's own size in any box the settings file left empty.
   *
   * First run only, not on every incoming event: a box the user cleared has to
   * stay cleared for as long as they are looking at it. The value shown here is
   * not persisted until the user changes something, at which point it is what
   * they saw and agreed to.
   */
  function showDefaultsInEmptyBoxes() {
    const boxes: Array<[HTMLInputElement, FontKind]> = [
      [fontSentence, 'sentence'],
      [fontGloss, 'gloss'],
      [furiganaScale, 'furigana'],
    ];
    for (const [input, kind] of boxes) {
      if (input.value !== '') continue;
      const fallback = stylesheetDefault(kind);
      if (fallback !== null) input.value = String(fallback);
    }
  }

  void invoke<Settings>('get_settings')
    .then((s) => {
      applySettings(s);
      showDefaultsInEmptyBoxes();
    })
    .catch(() => {});

  void listen<Settings>('settings-changed', (e) => {
    applySettings(e.payload);
  });

  // The path is named literally so the manual route needs no documentation to
  // find; the webview cannot resolve the app config dir itself.
  void invoke<string>('hints_dir')
    .then((dir) => {
      if (typeof dir !== 'string' || dir === '') return;
      mecabNote.textContent =
        `${HINTS_NOTE_BASE} To use your own instead, drop an IPADIC-format Vibrato ` +
        `.tar.xz in ${dir} and toggle this off and on — the newest archive wins.`;
    })
    .catch(() => {});

  // Seeded from the same command the main window reads, so a user who opens
  // settings to fix the warning sees the explanation next to the control.
  void invoke<string | null>('hints_warning')
    .then((message) => {
      if (message !== null && message !== undefined) showHintsStatus(String(message));
    })
    .catch(() => {});

  void listen<string>('hints-status', (e) => {
    applyHintsStatus(e.payload);
  });
}

const appEl = document.querySelector<HTMLElement>('#settings-app');
if (appEl !== null) {
  initSettingsApp(appEl);
}
