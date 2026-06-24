<!--
  KeyboardShortcuts — table of all shortcut actions with current binding and
  a "record" button that captures the next key combo pressed.
-->
<script lang="ts">
  import {
    SHORTCUT_DEFINITIONS,
    useShortcuts,
    isMac,
    type ShortcutAction,
    type ShortcutGroup,
  } from '$lib/stores/shortcuts.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import Select from '$lib/components/ui/Select.svelte';

  const shortcuts = useShortcuts();
  const settingsStore = useSettings();

  const PRESET_OPTIONS = [
    { value: 'vscode', label: 'VS Code' },
    { value: 'jetbrains', label: 'JetBrains' },
    { value: 'vim', label: 'Vim' },
  ];

  async function handlePresetChange(value: string) {
    const p = value as 'vscode' | 'jetbrains' | 'vim';
    shortcuts.setPreset(p);
    await settingsStore.set('shortcutPreset', p);
  }

  let recording = $state<ShortcutAction | null>(null);
  let lastRecordedCombo = '';
  let lastRecordedComboTime = 0;
  const DOUBLE_TAP_MS = 400;

  const conflictedActions = $derived.by(() => {
    const counts = new Map<string, ShortcutAction[]>();
    for (const def of SHORTCUT_DEFINITIONS) {
      const binding = shortcuts.getShortcut(def.action);
      if (!binding) continue;
      const list = counts.get(binding) ?? [];
      list.push(def.action);
      counts.set(binding, list);
    }
    const conflicted = new Set<ShortcutAction>();
    for (const actions of counts.values()) {
      if (actions.length > 1) actions.forEach(a => conflicted.add(a));
    }
    return conflicted;
  });

  function startRecording(action: ShortcutAction) {
    recording = action;
    lastRecordedCombo = '';
    lastRecordedComboTime = 0;
    shortcuts.setRecording(true);
  }

  function stopRecording() {
    recording = null;
    lastRecordedCombo = '';
    lastRecordedComboTime = 0;
    shortcuts.setRecording(false);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    const parts: string[] = [];
    if (isMac ? e.metaKey : e.ctrlKey) parts.push('Mod');
    if (isMac ? e.ctrlKey : e.metaKey) parts.push('Ctrl');
    if (e.altKey) parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');

    const key = e.key;
    const isModifierOnly = ['Meta', 'Control', 'Alt', 'Shift'].includes(key);

    if (isModifierOnly) {
      const now = Date.now();
      const combo = parts.join('+');
      if (combo && combo === lastRecordedCombo && now - lastRecordedComboTime < DOUBLE_TAP_MS) {
        shortcuts.setOverride(recording, `${combo}+${combo}`);
        lastRecordedCombo = '';
        lastRecordedComboTime = 0;
        stopRecording();
      } else {
        lastRecordedCombo = combo;
        lastRecordedComboTime = now;
      }
    } else {
      parts.push(key.length === 1 ? key.toUpperCase() : key);
      shortcuts.setOverride(recording, parts.join('+'));
      lastRecordedCombo = '';
      lastRecordedComboTime = 0;
      stopRecording();
    }
  }

  async function resetOverride(action: ShortcutAction) {
    await shortcuts.clearOverride(action);
  }

  const KEY_LABELS: Record<string, string> = {
    Mod: isMac ? 'Command' : 'Control',
    Shift: 'Shift',
    Alt: 'Option',
    Ctrl: 'Control',
    Enter: 'Enter',
    Escape: 'Escape',
    Backspace: 'Backspace',
    Delete: 'Delete',
    Tab: 'Tab',
    ArrowUp: 'Up',
    ArrowDown: 'Down',
    ArrowLeft: 'Left',
    ArrowRight: 'Right',
    PageUp: 'Page Up',
    PageDown: 'Page Down',
    Right: 'Right',
    Left: 'Left',
    Up: 'Up',
    Down: 'Down',
  };

  function bindingTooltip(binding: string): string {
    return parseBinding(binding).map(p => KEY_LABELS[p] ?? p).join('+');
  }

  const KEY_SYMBOLS: Record<string, string> = {
    Mod: isMac ? '⌘' : 'Ctrl',
    Shift: '⇧',
    Alt: '⌥',
    Ctrl: '⌃',
    Enter: '↵',
    Escape: 'Escape',
    Backspace: '⌫',
    Delete: '⌦',
    Tab: '⇥',
    ArrowUp: '↑',
    ArrowDown: '↓',
    ArrowLeft: '←',
    ArrowRight: '→',
    PageUp: '⇞',
    PageDown: '⇟',
    Right: '→',
    Left: '←',
    Up: '↑',
    Down: '↓',
  };

  function formatKey(key: string): string {
    return KEY_SYMBOLS[key] ?? key;
  }

  function parseBinding(binding: string): string[] {
    return binding.includes('+') ? binding.split('+') : [binding];
  }

  const GROUP_ORDER: ShortcutGroup[] = ['Query Editor', 'Navigation', 'Table View', 'General'];

  const groupedDefinitions = $derived.by(() => {
    return GROUP_ORDER.map(group => ({
      group,
      defs: SHORTCUT_DEFINITIONS.filter(d => d.group === group),
    }));
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="preset-row">
  <label class="preset-label" for="preset-select">Preset</label>
  <Select
    id="preset-select"
    value={shortcuts.preset}
    options={PRESET_OPTIONS}
    onchange={handlePresetChange}
    size="sm"
  />
</div>

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
      {#each groupedDefinitions as { group, defs }}
        <tr class="group-header-row">
          <td colspan="3" class="group-header">{group}</td>
        </tr>
        {#each defs as def (def.action)}
          {@const current = shortcuts.getShortcut(def.action)}
          {@const isOverridden = shortcuts.overrides.has(def.action)}
          {@const isRecording = recording === def.action}
          {@const isConflict = conflictedActions.has(def.action)}
          <tr class="shortcut-row" class:is-recording={isRecording}>
            <td class="col-action">
              <span class="action-label">{def.label}</span>
            </td>
            <td class="col-binding">
              {#if isRecording}
                <span class="recording-hint">Press a key combo…</span>
              {:else if current}
                <span class="kbd-wrap">
                  <kbd class="kbd" class:overridden={isOverridden} class:conflict={isConflict} title={bindingTooltip(current)}>{parseBinding(current).map(formatKey).join(' ')}</kbd>
                  {#if isConflict}
                    <svg class="conflict-icon" width="13" height="13" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                      <path d="M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Z" fill="currentColor"/>
                      <path d="M7.25 4.75a.75.75 0 0 1 1.5 0v3.5a.75.75 0 0 1-1.5 0v-3.5ZM8 11.5a.875.875 0 1 0 0-1.75.875.875 0 0 0 0 1.75Z" fill="currentColor"/>
                    </svg>
                  {/if}
                </span>
              {:else}
                <span class="unbound">None</span>
              {/if}
            </td>
            <td class="col-controls">
              <div class="controls-inner">
                {#if isRecording}
                  <button
                    class="cancel-btn"
                    onclick={stopRecording}
                    aria-label="Cancel recording"
                  >
                    Cancel
                  </button>
                {:else}
                  <button
                    class="record-btn"
                    onclick={() => startRecording(def.action)}
                    aria-label="Record shortcut for {def.label}"
                    title="Click to record"
                  >
                    Record
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
                {/if}
              </div>
            </td>
          </tr>
        {/each}
      {/each}
    </tbody>
  </table>
</div>

<style>
  .preset-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-bottom: var(--spacing-3);
  }

  .preset-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

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

  .group-header-row {
    background: var(--color-bg-secondary);
  }

  .group-header {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid var(--color-border);
    border-top: 1px solid var(--color-border);
  }

  .group-header-row:first-child .group-header {
    border-top: none;
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
  }

  .controls-inner {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    padding: 1px var(--spacing-1);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: 1.4;
  }


  .kbd.overridden {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .kbd.conflict {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .kbd-wrap {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
  }

  .conflict-icon {
    color: var(--color-danger);
    flex-shrink: 0;
  }

  .recording-hint {
    font-size: var(--font-size-xs);
    color: var(--color-accent);
    font-style: italic;
  }

  .unbound {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
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

  .cancel-btn {
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-accent);
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast);
  }

  .cancel-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border);
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
