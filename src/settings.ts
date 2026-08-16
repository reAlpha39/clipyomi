import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import type { FuriganaMode, Settings } from './types';
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
  const furiganaButtons = container.querySelectorAll<HTMLButtonElement>(
    '.segmented-control button[data-mode]',
  );

  let currentMode: FuriganaMode = 'none';

  function updateFuriganaButtons(mode: FuriganaMode) {
    currentMode = mode;
    furiganaButtons.forEach((btn) => {
      const match = btn.getAttribute('data-mode') === mode;
      btn.setAttribute('aria-checked', String(match));
    });
  }

  function currentSnapshot(): Settings {
    return {
      always_on_top: alwaysOnTop.checked,
      clipboard_monitoring: monitoring.checked,
      decorations: decorations.checked,
      furigana_mode: currentMode,
      hide_pos: hidePos.checked,
      hide_xrefs: hideXrefs.checked,
      hide_usage: hideUsage.checked,
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
    if (s.decorations !== undefined) decorations.checked = s.decorations;
    if (s.furigana_mode !== undefined) updateFuriganaButtons(s.furigana_mode);
  }

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
    void invoke('set_decorations', { enabled: decorations.checked }).catch(() => {});
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

  void invoke<Settings>('get_settings')
    .then((s) => {
      applySettings(s);
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
