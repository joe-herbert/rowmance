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

  type Section = 'general' | 'editor' | 'keyboard' | 'connections' | 'appearance';

  let activeSection = $state<Section>('general');
  const settingsStore = useSettings();
  const settings = $derived(settingsStore.settings);

  let userThemes = $state<ThemeMeta[]>([]);
  let selectedCustomTheme = $state<string | null>(null);
  let themeError = $state<string | null>(null);

  onMount(async () => {
    try {
      userThemes = await themesApi.themesList();
      if (userThemes.length > 0) selectedCustomTheme = userThemes[0].name;
    } catch (err) {
      themeError = errorMessage(err);
    }
  });

  async function createNewTheme() {
    const base = settings.theme === 'light' ? 'light' : 'dark';
    const newName = `custom-${Date.now()}`;
    try {
      const meta = await themesApi.themesDuplicate(base, newName);
      userThemes = [...userThemes, meta];
      selectedCustomTheme = meta.name;
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function deleteTheme(name: string) {
    try {
      await themesApi.themesDelete(name);
      userThemes = userThemes.filter((t) => t.name !== name);
      if (selectedCustomTheme === name) {
        selectedCustomTheme = userThemes[0]?.name ?? null;
      }
    } catch (err) {
      themeError = errorMessage(err);
    }
  }

  async function update<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    await settingsStore.set(key, value);
  }
</script>

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
            <span class="label-text">Theme</span>
            <span class="label-hint">Appearance of the application</span>
          </div>
          <select
            class="setting-select"
            value={settings.theme}
            onchange={(e) => update('theme', (e.currentTarget as HTMLSelectElement).value as AppSettings['theme'])}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>

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

      <div class="setting-group">
        <div class="setting-row">
          <div class="setting-label">
            <span class="label-text">Theme</span>
            <span class="label-hint">Base colour scheme</span>
          </div>
          <select
            class="setting-select"
            value={settings.theme}
            onchange={(e) => update('theme', (e.currentTarget as HTMLSelectElement).value as AppSettings['theme'])}
          >
            <option value="system">System</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
        </div>
      </div>

      <div class="appearance-section-title">Custom Themes</div>

      {#if themeError}
        <p class="section-description" style="color: var(--color-danger);">{themeError}</p>
      {/if}

      <div class="theme-actions">
        <button class="action-btn" onclick={createNewTheme}>+ New Theme</button>
        {#if userThemes.length > 0}
          <select
            class="setting-select"
            value={selectedCustomTheme}
            onchange={(e) => (selectedCustomTheme = (e.currentTarget as HTMLSelectElement).value)}
            aria-label="Select custom theme to edit"
          >
            {#each userThemes as theme (theme.name)}
              <option value={theme.name}>{theme.name}</option>
            {/each}
          </select>
          {#if selectedCustomTheme}
            <button
              class="action-btn action-btn--danger"
              onclick={() => selectedCustomTheme && deleteTheme(selectedCustomTheme)}
            >Delete</button>
          {/if}
        {/if}
      </div>

      {#if selectedCustomTheme}
        <div class="theme-editor-wrap">
          <ThemeEditor themeName={selectedCustomTheme} />
        </div>
      {:else if userThemes.length === 0}
        <p class="section-description">No custom themes yet. Click "+ New Theme" to create one.</p>
      {/if}
    {/if}
  </div>
</div>

<style>
  .settings-page {
    display: flex;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .settings-nav {
    width: 160px;
    flex-shrink: 0;
    border-right: 1px solid var(--color-border);
    padding: var(--spacing-3) 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--color-bg-secondary);
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
    background: var(--color-bg-primary);
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

  .appearance-section-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin-top: var(--spacing-4);
    margin-bottom: var(--spacing-2);
  }

  .theme-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-bottom: var(--spacing-3);
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

  .theme-editor-wrap {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-height: 500px;
    overflow-y: auto;
  }
</style>
