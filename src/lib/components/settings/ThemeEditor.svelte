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
  import { useToast } from '$lib/stores/toast.svelte';
  import { VARIABLE_GROUPS } from './theme-variables';

  interface Props {
    themeName: string;
    onrename?: (_newName: string) => Promise<void>;
    ondelete?: () => void;
    onexport?: () => void;
  }

  const { themeName, onrename, ondelete, onexport }: Props = $props();
  const toast = useToast();

  let themeData = $state<ThemeData | null>(null);
  let loadError = $state<string | null>(null);
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
    } catch (err) {
      editingName = themeName;
      toast.addToast(errorMessage(err), 'error', 0);
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
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
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
      <div class="editor-meta-left">
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
        <span class="theme-extends">based on: {themeData.extends}</span>
      </div>
      <div class="editor-meta-actions">
        {#if onexport}
          <button class="meta-btn" onclick={onexport} aria-label="Export theme">Export</button>
        {/if}
        {#if ondelete}
          <button class="meta-btn meta-btn--danger" onclick={ondelete} aria-label="Delete theme">Delete</button>
        {/if}
      </div>
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
    justify-content: space-between;
    gap: var(--spacing-3);
    padding-bottom: var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
  }

  .editor-meta-left {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    flex: 1;
    min-width: 0;
  }

  .editor-meta-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    flex-shrink: 0;
  }

  .meta-btn {
    height: 24px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .meta-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .meta-btn--danger {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .meta-btn--danger:hover {
    background: var(--color-danger-subtle);
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
    flex: 1;
    max-width: 500px;
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
    flex: 1;
    min-width: 0;
    max-width: 500px;
    transition: border-color var(--transition-fast);
  }

  .text-input--sm {
    flex: 1;
    min-width: 0;
    width: auto;
  }

  .text-input:focus {
    border-color: var(--color-accent);
  }
</style>
