<!--
  Settings — application settings page with sections for General, Editor,
  Keyboard, and Connections (placeholder).
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import KeyboardShortcuts from '$lib/components/settings/KeyboardShortcuts.svelte';
  import ThemeEditor from '$lib/components/settings/ThemeEditor.svelte';
  import * as themesApi from '$lib/tauri/themes';
  import type { AppSettings, ThemeMeta } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { portal } from '$lib/utils/portal';
  import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { ALL_THEME_VARS } from './theme-variables';
  import type { ThemeData } from '$lib/types';

  type Section = 'general' | 'editor' | 'keyboard' | 'connections' | 'appearance';

  let activeSection = $state<Section>('general');
  const settingsStore = useSettings();
  const settings = $derived(settingsStore.settings);

  const BUILTIN_THEMES = ['system', 'light', 'dark'];

  let userThemes = $state<ThemeMeta[]>([]);
  let themeError = $state<string | null>(null);
  let creatingTheme = $state(false);
  let newThemeName = $state('');
  let newThemeBase = $state('light');
  let confirmingDelete = $state(false);

  const isCustomTheme = $derived(!BUILTIN_THEMES.includes(settings.theme));

  onMount(async () => {
    try {
      userThemes = await themesApi.themesList();
    } catch (err) {
      themeError = errorMessage(err);
    }
  });

  function startCreatingTheme() {
    newThemeName = '';
    const currentBase = BUILTIN_THEMES.includes(settings.theme)
      ? settings.theme
      : (userThemes.find((t) => t.name === settings.theme)?.extends ?? 'light');
    newThemeBase = currentBase === 'system' ? 'light' : currentBase;
    themeError = null;
    creatingTheme = true;
  }

  async function confirmCreateTheme() {
    const name = newThemeName.trim();
    if (!name) return;
    try {
      const variables = await resolveBaseVariables(newThemeBase);
      const data: ThemeData = { name, extends: newThemeBase, variables };
      await themesApi.themesWrite(name, data);
      const meta = { name, extends: newThemeBase };
      userThemes = [...userThemes, meta];
      await update('theme', name);
      creatingTheme = false;
      newThemeName = '';
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function resolveBaseVariables(base: string): Promise<Record<string, string>> {
    // Custom theme: variables are stored in the JSON file on disk.
    if (!BUILTIN_THEMES.includes(base)) {
      const data = await themesApi.themesRead(base);
      return { ...data.variables };
    }

    // Built-in theme: snapshot computed CSS by temporarily switching data-theme.
    const root = document.documentElement;
    const style = root.style;

    // Stash and clear inline custom properties so computed values reflect the
    // CSS theme file rather than any active custom theme.
    const stash: Record<string, string> = {};
    const inlineProps: string[] = [];
    for (let i = 0; i < style.length; i++) {
      if (style[i].startsWith('--')) inlineProps.push(style[i]);
    }
    inlineProps.forEach((p) => { stash[p] = style.getPropertyValue(p); style.removeProperty(p); });

    const prevTheme = root.getAttribute('data-theme') ?? 'system';
    root.setAttribute('data-theme', base);

    const computed = getComputedStyle(root);
    const variables: Record<string, string> = {};
    for (const v of ALL_THEME_VARS) {
      variables[v] = computed.getPropertyValue(v).trim();
    }

    root.setAttribute('data-theme', prevTheme);
    inlineProps.forEach((p) => style.setProperty(p, stash[p]));

    return variables;
  }

  function cancelCreateTheme() {
    creatingTheme = false;
    newThemeName = '';
  }

  async function renameTheme(newName: string) {
    const oldName = settings.theme;
    if (!oldName || newName === oldName) return;
    try {
      const meta = await themesApi.themesRename(oldName, newName);
      userThemes = userThemes.map((t) => t.name === oldName ? meta : t);
      await update('theme', meta.name);
      themeError = null;
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function deleteTheme() {
    const name = settings.theme;
    if (!name || BUILTIN_THEMES.includes(name)) return;
    try {
      await themesApi.themesDelete(name);
      userThemes = userThemes.filter((t) => t.name !== name);
      await update('theme', 'system');
      confirmingDelete = false;
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function exportTheme() {
    const filePath = await saveDialog({
      defaultPath: `${settings.theme}.json`,
      filters: [{ name: 'Theme', extensions: ['json'] }],
    });
    if (!filePath) return;
    try {
      await themesApi.themesExport(settings.theme, filePath);
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function importTheme() {
    const filePath = await openDialog({
      multiple: false,
      filters: [{ name: 'Theme', extensions: ['json'] }],
    });
    if (!filePath || typeof filePath !== 'string') return;
    try {
      const meta = await themesApi.themesImport(filePath);
      if (!userThemes.some((t) => t.name === meta.name)) {
        userThemes = [...userThemes, meta];
      } else {
        userThemes = userThemes.map((t) => t.name === meta.name ? meta : t);
      }
      await update('theme', meta.name);
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    await settingsStore.set(key, value);
  }
</script>

<svelte:window onkeydown={(e) => {
  if (e.key === 'Escape' && creatingTheme) cancelCreateTheme();
  if (e.key === 'Escape' && confirmingDelete) confirmingDelete = false;
}} />

<div class="settings-page">
  <!-- Sidebar nav -->
  <nav class="settings-nav" aria-label="Settings sections">
    {#each (['general', 'editor', 'keyboard', 'connections', 'appearance'] as const) as section}
      <button
        class="nav-item"
        class:active={activeSection === section}
        onclick={() => (activeSection = section)}
        aria-current={activeSection === section ? 'page' : undefined}
      >
        {section.charAt(0).toUpperCase() + section.slice(1)}
      </button>
    {/each}
  </nav>

  <!-- Content -->
  <div class="settings-content">
    {#if activeSection === 'general'}
      <h2 class="section-title">General</h2>

      <div class="setting-group">
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Page Size</span>
            <span class="label-hint">Rows per page in query results</span>
          </div>
          <input
            class="setting-input setting-input--sm"
            type="number"
            min="10"
            max="1000"
            value={settings.pageSize}
            onchange={(e) => update('pageSize', parseInt((e.currentTarget as HTMLInputElement).value, 10))}
          />
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Transaction Mode</span>
            <span class="label-hint">Show transaction toolbar in query editor</span>
          </div>
          <input
            type="checkbox"
            class="setting-checkbox"
            checked={settings.transactionMode}
            onchange={(e) => update('transactionMode', (e.currentTarget as HTMLInputElement).checked)}
          />
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">History Max Entries</span>
            <span class="label-hint">Maximum number of query history entries</span>
          </div>
          <input
            class="setting-input setting-input--sm"
            type="number"
            min="50"
            max="5000"
            value={settings.historyMaxEntries}
            onchange={(e) => update('historyMaxEntries', parseInt((e.currentTarget as HTMLInputElement).value, 10))}
          />
        </div>
      </div>

    {:else if activeSection === 'editor'}
      <h2 class="section-title">Editor</h2>

      <div class="setting-group">
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Tab Size</span>
            <span class="label-hint">Number of spaces per tab</span>
          </div>
          <input
            class="setting-input setting-input--sm"
            type="number"
            min="1"
            max="8"
            value={settings.editorTabSize}
            onchange={(e) => update('editorTabSize', parseInt((e.currentTarget as HTMLInputElement).value, 10))}
          />
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Line Wrap</span>
            <span class="label-hint">Wrap long lines in the editor</span>
          </div>
          <input
            type="checkbox"
            class="setting-checkbox"
            checked={settings.editorLineWrap}
            onchange={(e) => update('editorLineWrap', (e.currentTarget as HTMLInputElement).checked)}
          />
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Font Family</span>
            <span class="label-hint">Font used in the editor and results</span>
          </div>
          <input
            class="setting-input"
            type="text"
            value={settings.fontFamily}
            onchange={(e) => update('fontFamily', (e.currentTarget as HTMLInputElement).value)}
          />
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Font Size</span>
            <span class="label-hint">Base font size in pixels</span>
          </div>
          <input
            class="setting-input setting-input--sm"
            type="number"
            min="8"
            max="32"
            value={settings.fontSize}
            onchange={(e) => update('fontSize', parseInt((e.currentTarget as HTMLInputElement).value, 10))}
          />
        </div>
      </div>

    {:else if activeSection === 'keyboard'}
      <h2 class="section-title">Keyboard Shortcuts</h2>
      <p class="section-description">
        Click <strong>Record</strong> next to any action, then press the key combination you want.
        Press Escape to cancel recording.
      </p>
      <KeyboardShortcuts />

    {:else if activeSection === 'connections'}
      <h2 class="section-title">Connections</h2>
      <p class="section-description">
        Connection-level settings are managed per connection in the Connections panel.
      </p>

    {:else if activeSection === 'appearance'}
      <h2 class="section-title">Appearance</h2>

      {#if themeError}
        <p class="section-description" style="color: var(--color-danger);">{themeError}</p>
      {/if}

      <div class="setting-group">
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Theme</span>
            <span class="label-hint">Colour scheme</span>
          </div>
          <div class="theme-selector-row">
            <select
              class="setting-select"
              value={settings.theme}
              onchange={(e) => { confirmingDelete = false; update('theme', (e.currentTarget as HTMLSelectElement).value); }}
              aria-label="Select theme"
            >
              <optgroup label="Built-in">
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </optgroup>
              {#if userThemes.length > 0}
                <optgroup label="Custom">
                  {#each userThemes as t (t.name)}
                    <option value={t.name}>{t.name}</option>
                  {/each}
                </optgroup>
              {/if}
            </select>
            <button class="action-btn" onclick={startCreatingTheme}>+ New</button>
            <button class="action-btn" onclick={importTheme}>Import</button>
          </div>
        </div>

        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Floating Sidebars</span>
            <span class="label-hint">Sidebars overlay the main content instead of pushing it</span>
          </div>
          <input
            type="checkbox"
            class="setting-checkbox"
            checked={settings.sidebarFloating}
            onchange={(e) => update('sidebarFloating', (e.currentTarget as HTMLInputElement).checked)}
          />
        </div>
      </div>


      {#if isCustomTheme}
        {#key settings.theme}
          <div class="theme-editor-wrap" style="margin-top: var(--spacing-3);">
            <ThemeEditor themeName={settings.theme} onrename={renameTheme} ondelete={() => { confirmingDelete = true; }} onexport={exportTheme} />
          </div>
        {/key}
      {/if}
    {/if}
  </div>
</div>

{#if creatingTheme}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="backdrop" use:portal role="dialog" aria-modal="true" aria-label="New theme" tabindex="-1"
    onclick={(e) => { if (e.target === e.currentTarget) cancelCreateTheme(); }}>
    <div class="modal">
      <header class="modal-header">
        <h2 class="modal-title">New Theme</h2>
        <button class="close-btn" onclick={cancelCreateTheme} aria-label="Close">✕</button>
      </header>

      <div class="modal-body">
        <div class="modal-field">
          <label class="modal-label" for="new-theme-base">Base theme</label>
          <span class="modal-hint">Your new theme starts as a copy of this</span>
          <select id="new-theme-base" class="setting-select modal-select" bind:value={newThemeBase}>
            <optgroup label="Built-in">
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </optgroup>
            {#if userThemes.length > 0}
              <optgroup label="Custom">
                {#each userThemes as t (t.name)}
                  <option value={t.name}>{t.name}</option>
                {/each}
              </optgroup>
            {/if}
          </select>
        </div>

        <div class="modal-field">
          <label class="modal-label" for="new-theme-name">Theme name</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="new-theme-name"
            class="setting-input modal-input"
            type="text"
            placeholder="My Theme"
            bind:value={newThemeName}
            onkeydown={(e) => { if (e.key === 'Enter') confirmCreateTheme(); }}
            autofocus
          />
        </div>

        {#if themeError}
          <p class="modal-error">{themeError}</p>
        {/if}
      </div>

      <footer class="modal-footer">
        <button class="action-btn" onclick={cancelCreateTheme}>Cancel</button>
        <button class="action-btn action-btn--primary" onclick={confirmCreateTheme} disabled={!newThemeName.trim()}>Create</button>
      </footer>
    </div>
  </div>
{/if}

{#if confirmingDelete}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="backdrop" use:portal role="dialog" aria-modal="true" aria-label="Delete theme" tabindex="-1"
    onclick={(e) => { if (e.target === e.currentTarget) confirmingDelete = false; }}>
    <div class="modal">
      <header class="modal-header">
        <h2 class="modal-title">Delete Theme</h2>
        <button class="close-btn" onclick={() => (confirmingDelete = false)} aria-label="Close">✕</button>
      </header>
      <div class="modal-body">
        <p class="modal-confirm-text">Delete <strong>{settings.theme}</strong>? This cannot be undone.</p>
      </div>
      <footer class="modal-footer">
        <button class="action-btn" onclick={() => (confirmingDelete = false)}>Cancel</button>
        <button class="action-btn action-btn--danger" onclick={deleteTheme}>Delete</button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .settings-page {
    display: flex;
    height: 100%;
    overflow: hidden;
  }

  .settings-nav {
    width: 160px;
    flex-shrink: 0;
    border-right: 1px solid var(--color-border);
    padding: var(--spacing-3) 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    padding: var(--spacing-2) var(--spacing-3);
    text-align: left;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    border-radius: 0;
    transition: background var(--transition-fast), color var(--transition-fast);
    font-family: var(--font-family-ui);
  }

  .nav-item:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .nav-item.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-6);
  }

  .section-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin-bottom: var(--spacing-4);
  }

  .section-description {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin-bottom: var(--spacing-4);
    line-height: 1.5;
  }

  .setting-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-4);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
  }

  .setting-row:last-child {
    border-bottom: none;
  }

  .setting-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .label-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .label-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .setting-input {
    height: 28px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    transition: border-color var(--transition-fast);
    min-width: 200px;
  }

  .setting-input--sm {
    min-width: 80px;
    width: 80px;
  }

  .setting-input:focus {
    border-color: var(--color-accent);
  }

  .setting-select {
    height: 28px;
    padding: 0 var(--spacing-5) 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    cursor: pointer;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath d='M1 1l5 5 5-5' stroke='%23999' stroke-width='1.5' fill='none' stroke-linecap='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right var(--spacing-2) center;
  }

  .setting-checkbox {
    width: 16px;
    height: 16px;
    cursor: pointer;
    accent-color: var(--color-accent);
  }

  .theme-selector-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .action-btn {
    height: 28px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .action-btn--danger {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .action-btn--danger:hover {
    background: var(--color-danger-subtle);
  }

  .action-btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border-color: var(--color-accent);
  }

  .action-btn--primary:hover:not(:disabled) {
    background: var(--color-accent-hover);
    border-color: var(--color-accent-hover);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .theme-editor-wrap {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-height: 500px;
    overflow-y: auto;
  }

  .modal-confirm-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    line-height: 1.5;
  }

  /* ── New Theme Modal ─────────────────────────────────────────────────────── */

  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 300;
  }

  .modal {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 400px;
    max-width: calc(100vw - var(--spacing-8));
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-title {
    flex: 1;
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .close-btn {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .close-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .modal-body {
    padding: var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .modal-footer {
    padding: var(--spacing-4) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
  }

  .modal-field {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .modal-label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-primary);
  }

  .modal-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-bottom: var(--spacing-1);
  }

  .modal-select,
  .modal-input {
    width: 100%;
    box-sizing: border-box;
  }

  .modal-error {
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }
</style>
