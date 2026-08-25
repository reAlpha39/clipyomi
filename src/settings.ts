import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { FONT_LIMITS, clampFont, stylesheetDefault, type FontKind } from './font-sizes';
import type { ChromeMode, FuriganaMode, Settings } from './types';
import './styles/global.css';
import './styles/settings.css';

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
        <label class="settings-checkbox-row">
          <input type="checkbox" id="setting-decorations" />
          <span class="label-text">Title bar &amp; window borders</span>
        </label>
      </section>
    </div>
  `;

  const hidePos = container.querySelector<HTMLInputElement>('#setting-hide-pos')!;
  const hideXrefs = container.querySelector<HTMLInputElement>('#setting-hide-xrefs')!;
  const hideUsage = container.querySelector<HTMLInputElement>('#setting-hide-usage')!;
  const alwaysOnTop = container.querySelector<HTMLInputElement>('#setting-always-on-top')!;
  const monitoring = container.querySelector<HTMLInputElement>('#setting-clipboard-monitoring')!;
  const decorations = container.querySelector<HTMLInputElement>('#setting-decorations')!;
  const fontSentence = container.querySelector<HTMLInputElement>('#setting-font-sentence')!;
  const fontGloss = container.querySelector<HTMLInputElement>('#setting-font-gloss')!;
  const furiganaScale = container.querySelector<HTMLInputElement>('#setting-furigana-scale')!;
  const furiganaButtons = container.querySelectorAll<HTMLButtonElement>(
    '.segmented-control button[data-mode]',
  );

  let currentMode: FuriganaMode = 'none';
  // Task 5 replaces the checkbox with a three-way control. Until then this
  // maps the two states it can express, so the phase has no commit where the
  // title-bar toggle does nothing.
  let currentChrome: ChromeMode = 'native';

  function updateFuriganaButtons(mode: FuriganaMode) {
    currentMode = mode;
    furiganaButtons.forEach((btn) => {
      const match = btn.getAttribute('data-mode') === mode;
      btn.setAttribute('aria-checked', String(match));
    });
  }

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
      font_sentence: sizeOf(fontSentence, 'sentence'),
      font_gloss: sizeOf(fontGloss, 'gloss'),
      furigana_scale: sizeOf(furiganaScale, 'furigana'),
    };
  }

  function saveAndBroadcast() {
    const snapshot = currentSnapshot();
    void invoke('save_settings', { settings: snapshot }).catch(() => {});
    void emit('settings-changed', snapshot).catch(() => {});
  }

  function applySettings(s: Partial<Settings>) {
    if (s.hide_pos !== undefined) hidePos.checked = s.hide_pos;
    if (s.hide_xrefs !== undefined) hideXrefs.checked = s.hide_xrefs;
    if (s.hide_usage !== undefined) hideUsage.checked = s.hide_usage;
    if (s.always_on_top !== undefined) alwaysOnTop.checked = s.always_on_top;
    if (s.clipboard_monitoring !== undefined) monitoring.checked = s.clipboard_monitoring;
    if (s.chrome_mode !== undefined) {
      currentChrome = s.chrome_mode;
      decorations.checked = s.chrome_mode === 'native';
    }
    if (s.furigana_mode !== undefined) updateFuriganaButtons(s.furigana_mode);
    if (s.font_sentence !== undefined) fontSentence.value = String(s.font_sentence);
    if (s.font_gloss !== undefined) fontGloss.value = String(s.font_gloss);
    if (s.furigana_scale !== undefined) furiganaScale.value = String(s.furigana_scale);
  }

  fontSentence.addEventListener('change', saveAndBroadcast);
  fontGloss.addEventListener('change', saveAndBroadcast);
  furiganaScale.addEventListener('change', saveAndBroadcast);

  hidePos.addEventListener('change', saveAndBroadcast);
  hideXrefs.addEventListener('change', saveAndBroadcast);
  hideUsage.addEventListener('change', saveAndBroadcast);

  alwaysOnTop.addEventListener('change', () => {
    void invoke('set_always_on_top', { enabled: alwaysOnTop.checked }).catch(() => {});
    saveAndBroadcast();
  });

  monitoring.addEventListener('change', () => {
    void invoke('set_clipboard_monitoring', { enabled: monitoring.checked }).catch(() => {});
    saveAndBroadcast();
  });

  decorations.addEventListener('change', () => {
    currentChrome = decorations.checked ? 'native' : 'header';
    void invoke('set_chrome_mode', { mode: currentChrome }).catch(() => {});
    saveAndBroadcast();
  });

  furiganaButtons.forEach((btn) => {
    btn.addEventListener('click', () => {
      const mode = btn.getAttribute('data-mode') as FuriganaMode;
      if (mode) {
        updateFuriganaButtons(mode);
        saveAndBroadcast();
      }
    });
  });

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
}

const appEl = document.querySelector<HTMLElement>('#settings-app');
if (appEl !== null) {
  initSettingsApp(appEl);
}
