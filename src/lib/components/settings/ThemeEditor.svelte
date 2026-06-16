<!--
  ThemeEditor — live CSS variable editor for a user theme.
  Changes apply instantly to the document root and are saved with a 500ms debounce.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import * as themesApi from '$lib/tauri/themes';
  import { syncTrafficLightPosition } from '$lib/tauri/window';
  import type { ThemeData } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    themeName: string;
    onrename?: (newName: string) => Promise<void>;
  }

  const { themeName, onrename }: Props = $props();

  // Variable groups drawn from the design token structure in variables.css.
  const VARIABLE_GROUPS: { label: string; vars: string[] }[] = [
    {
      label: 'Background',
      vars: [
        '--app-background',
        '--glass-blur',
      ],
    },
    {
      label: 'Panels',
      vars: [
        '--panel-spacing',
        '--panel-radius',
        '--panel-opacity',
      ],
    },
    {
      label: 'Colours',
      vars: [
        '--color-bg-primary', '--color-bg-secondary', '--color-bg-tertiary',
        '--color-bg-overlay', '--color-bg-hover', '--color-bg-active',
        '--color-border', '--color-border-strong',
        '--color-text-primary', '--color-text-secondary', '--color-text-muted',
        '--color-text-disabled', '--color-text-on-accent',
        '--color-accent', '--color-accent-hover', '--color-accent-subtle',
        '--color-danger', '--color-danger-hover', '--color-danger-subtle',
        '--color-warning', '--color-warning-subtle',
        '--color-success', '--color-success-subtle',
        '--color-null',
      ],
    },
    {
      label: 'Editor',
      vars: [
        '--color-editor-bg', '--color-editor-text',
        '--color-editor-gutter-bg', '--color-editor-gutter-text',
        '--color-editor-selection', '--color-editor-active-line',
        '--color-editor-cursor', '--color-editor-keyword',
        '--color-editor-string', '--color-editor-number',
        '--color-editor-comment', '--color-editor-operator',
        '--color-editor-function', '--color-editor-type',
        '--color-editor-bracket-match',
      ],
    },
    {
      label: 'Connections',
      vars: [
        '--color-connection-connected',
        '--color-connection-connecting',
        '--color-connection-error',
      ],
    },
    {
      label: 'Scrollbar',
      vars: [
        '--color-scrollbar-thumb',
        '--color-scrollbar-thumb-hover',
        '--color-scrollbar-track',
      ],
    },
    {
      label: 'Table',
      vars: [
        '--color-table-row-alt',
        '--color-table-row-hover',
        '--color-table-row-selected',
        '--color-table-header-bg',
      ],
    },
    {
      label: 'Typography',
      vars: [
        '--font-family-ui', '--font-family-mono',
        '--font-size-xs', '--font-size-sm', '--font-size-md',
        '--font-size-lg', '--font-size-xl',
        '--font-weight-normal', '--font-weight-medium', '--font-weight-semibold',
        '--line-height-tight', '--line-height-normal',
      ],
    },
    {
      label: 'Spacing',
      vars: [
        '--spacing-1', '--spacing-2', '--spacing-3', '--spacing-4',
        '--spacing-5', '--spacing-6', '--spacing-8',
      ],
    },
    {
      label: 'Radius',
      vars: [
        '--radius-sm', '--radius-md', '--radius-lg', '--radius-xl',
      ],
    },
    {
      label: 'Shadows',
      vars: [
        '--shadow-sm', '--shadow-md', '--shadow-lg', '--shadow-overlay',
      ],
    },
    {
      label: 'Transitions',
      vars: [
        '--transition-fast', '--transition-md', '--transition-slow',
      ],
    },
  ];

  let themeData = $state<ThemeData | null>(null);
  let loadError = $state<string | null>(null);
  let saveError = $state<string | null>(null);
  let renameError = $state<string | null>(null);
  let editingName = $state('');

  $effect(() => {
    editingName = themeName;
  });

  // Local overrides: variable name → value (accumulated edits not yet reflected in themeData)
  let localValues = $state<Record<string, string>>({});

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      themeData = await themesApi.themesRead(themeName);
      // Apply all stored variables to the document root.
      for (const [k, v] of Object.entries(themeData.variables)) {
        document.documentElement.style.setProperty(k, v);
      }
      localValues = { ...themeData.variables };
    } catch (err) {
      loadError = errorMessage(err);
    }
  });

  function getValue(varName: string): string {
    if (localValues[varName] !== undefined) return localValues[varName];
    return getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  }

  function isColorVar(varName: string): boolean {
    return varName.startsWith('--color-');
  }

  function handleChange(varName: string, newValue: string) {
    localValues = { ...localValues, [varName]: newValue };
    document.documentElement.style.setProperty(varName, newValue);
    if (varName === '--panel-spacing') syncTrafficLightPosition();
    scheduleSave();
  }

  async function commitRename() {
    const trimmed = editingName.trim();
    if (!trimmed || trimmed === themeName || !onrename) return;
    try {
      await onrename(trimmed);
      renameError = null;
    } catch (err) {
      editingName = themeName;
      renameError = errorMessage(err);
    }
  }

  function scheduleSave() {
    if (debounceTimer !== null) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(async () => {
      if (!themeData) return;
      const updated: ThemeData = {
        ...themeData,
        variables: { ...themeData.variables, ...localValues },
      };
      try {
        await themesApi.themesWrite(themeName, updated);
        themeData = updated;
        saveError = null;
      } catch (err) {
        saveError = errorMessage(err);
      }
    }, 500);
  }
</script>

<div class="theme-editor">
  {#if loadError}
    <div class="editor-error">{loadError}</div>
  {:else if !themeData}
    <div class="editor-loading">Loading…</div>
  {:else}
    <div class="editor-meta">
      {#if onrename}
        <input
          class="theme-name-input"
          type="text"
          bind:value={editingName}
          onblur={commitRename}
          onkeydown={(e) => { if (e.key === 'Enter') (e.currentTarget as HTMLInputElement).blur(); if (e.key === 'Escape') { editingName = themeName; (e.currentTarget as HTMLInputElement).blur(); } }}
          aria-label="Theme name"
        />
      {:else}
        <span class="theme-name">{themeData.name}</span>
      {/if}
      <span class="theme-extends">extends: {themeData.extends}</span>
      {#if renameError}
        <span class="save-error">{renameError}</span>
      {:else if saveError}
        <span class="save-error">{saveError}</span>
      {/if}
    </div>

    {#each VARIABLE_GROUPS as group}
      <div class="var-group">
        <div class="group-label">{group.label}</div>
        {#each group.vars as varName}
          {@const value = getValue(varName)}
          <div class="var-row">
            <span class="var-name" title={varName}>{varName}</span>
            {#if isColorVar(varName)}
              <div class="color-input-wrap">
                <span class="color-swatch" style="background: {value};"></span>
                <input
                  type="color"
                  class="color-input"
                  value={value.startsWith('#') ? value : '#000000'}
                  oninput={(e) => handleChange(varName, (e.currentTarget as HTMLInputElement).value)}
                  aria-label="Pick colour for {varName}"
                />
                <input
                  type="text"
                  class="text-input text-input--sm"
                  value={value}
                  onchange={(e) => handleChange(varName, (e.currentTarget as HTMLInputElement).value)}
                  aria-label="Colour value for {varName}"
                />
              </div>
            {:else}
              <input
                type="text"
                class="text-input"
                value={value}
                onchange={(e) => handleChange(varName, (e.currentTarget as HTMLInputElement).value)}
                aria-label="Value for {varName}"
              />
            {/if}
          </div>
        {/each}
      </div>
    {/each}
  {/if}
</div>

<style>
  .theme-editor {
    padding: var(--spacing-3);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .editor-error {
    color: var(--color-danger);
    font-size: var(--font-size-sm);
  }

  .editor-loading {
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-style: italic;
  }

  .editor-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding-bottom: var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
  }

  .theme-name {
    font-weight: var(--font-weight-semibold);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  .theme-name-input {
    font-weight: var(--font-weight-semibold);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    padding: 1px var(--spacing-1);
    outline: none;
    font-family: var(--font-family-ui);
    min-width: 80px;
    max-width: 200px;
    transition: border-color var(--transition-fast), background var(--transition-fast);
  }

  .theme-name-input:hover {
    border-color: var(--color-border);
    background: var(--color-bg-secondary);
  }

  .theme-name-input:focus {
    border-color: var(--color-accent);
    background: var(--color-bg-secondary);
  }

  .theme-extends {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .save-error {
    margin-left: auto;
    font-size: var(--font-size-xs);
    color: var(--color-danger);
  }

  .var-group {
    display: flex;
    flex-direction: column;
    gap: 1px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .group-label {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .var-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border-bottom: 1px solid var(--color-border);
    min-height: 36px;
  }

  .var-row:last-child {
    border-bottom: none;
  }

  .var-name {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 180px;
  }

  .color-input-wrap {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .color-swatch {
    width: 18px;
    height: 18px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .color-input {
    width: 32px;
    height: 24px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    padding: 0 2px;
    background: transparent;
  }

  .text-input {
    height: 26px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    outline: none;
    width: 160px;
    transition: border-color var(--transition-fast);
  }

  .text-input--sm {
    width: 90px;
  }

  .text-input:focus {
    border-color: var(--color-accent);
  }
</style>
