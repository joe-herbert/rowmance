<!--
  KeyboardShortcuts — table of all shortcut actions with current binding and
  a "record" button that captures the next key combo pressed.
-->
<script lang="ts">
  import {
    SHORTCUT_DEFINITIONS,
    useShortcuts,
    type ShortcutAction,
  } from '$lib/stores/shortcuts.svelte';

  const shortcuts = useShortcuts();

  let recording = $state<ShortcutAction | null>(null);

  function startRecording(action: ShortcutAction) {
    recording = action;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.key === 'Escape') {
      recording = null;
      return;
    }

    const parts: string[] = [];
    if (e.metaKey || e.ctrlKey) parts.push('Mod');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');

    const key = e.key;
    if (!['Meta', 'Control', 'Alt', 'Shift'].includes(key)) {
      parts.push(key.length === 1 ? key.toUpperCase() : key);
      const combo = parts.join('+');
      shortcuts.setOverride(recording, combo);
      recording = null;
    }
  }

  async function resetOverride(action: ShortcutAction) {
    await shortcuts.clearOverride(action);
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="shortcuts-table-wrap">
  <table class="shortcuts-table">
    <thead>
      <tr>
        <th class="col-action">Action</th>
        <th class="col-binding">Binding</th>
        <th class="col-controls"></th>
      </tr>
    </thead>
    <tbody>
      {#each SHORTCUT_DEFINITIONS as def (def.action)}
        {@const current = shortcuts.getShortcut(def.action)}
        {@const isOverridden = shortcuts.overrides.has(def.action)}
        {@const isRecording = recording === def.action}
        <tr class="shortcut-row" class:is-recording={isRecording}>
          <td class="col-action">
            <span class="action-label">{def.label}</span>
          </td>
          <td class="col-binding">
            {#if isRecording}
              <span class="recording-hint">Press a key combo… (Esc to cancel)</span>
            {:else}
              <kbd class="kbd" class:overridden={isOverridden}>{current}</kbd>
            {/if}
          </td>
          <td class="col-controls">
            <button
              class="record-btn"
              class:active={isRecording}
              onclick={() => startRecording(def.action)}
              aria-label="Record shortcut for {def.label}"
              title={isRecording ? 'Press a key combo…' : 'Click to record'}
            >
              {isRecording ? '…' : 'Record'}
            </button>
            {#if isOverridden}
              <button
                class="reset-btn"
                onclick={() => resetOverride(def.action)}
                aria-label="Reset {def.label} to preset default"
                title="Reset to preset default"
              >
                Reset
              </button>
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .shortcuts-table-wrap {
    overflow-x: auto;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .shortcuts-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-sm);
  }

  .shortcuts-table th {
    text-align: left;
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .shortcut-row td {
    padding: var(--spacing-2) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    vertical-align: middle;
  }

  .shortcut-row:last-child td {
    border-bottom: none;
  }

  .shortcut-row.is-recording {
    background: var(--color-accent-subtle);
  }

  .action-label {
    color: var(--color-text-primary);
  }

  .col-binding {
    width: 200px;
  }

  .col-controls {
    width: 140px;
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .kbd {
    display: inline-block;
    padding: 1px var(--spacing-2);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
  }

  .kbd.overridden {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .recording-hint {
    font-size: var(--font-size-xs);
    color: var(--color-accent);
    font-style: italic;
  }

  .record-btn,
  .reset-btn {
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast);
  }

  .record-btn:hover,
  .reset-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .record-btn.active {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .reset-btn {
    color: var(--color-text-muted);
  }

  .reset-btn:hover {
    color: var(--color-danger);
    border-color: var(--color-danger);
    background: var(--color-danger-subtle);
  }
</style>
