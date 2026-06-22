<!--
  CellEditor — inline cell editor that mounts over a table cell.
  Positioned absolutely relative to the DataTable container.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { useSettings } from '$lib/stores/settings.svelte';

  type CellValue = string | number | boolean | null;

  interface Props {
    value: CellValue;
    originalValue: CellValue;
    dataType: string;
    top: number;
    left: number;
    width: number;
    height: number;
    containerHeight: number;
    onConfirm: (_newValue: CellValue) => void;
    onCancel: () => void;
  }

  let { value, originalValue, dataType, top, left, width, height, containerHeight, onConfirm, onCancel }: Props = $props();

  const { settings } = useSettings();

  // ── Pure helpers (also exported for tests) ──────────────────────────────────

  export function isBooleanType(dt: string): boolean {
    return dt.toLowerCase().includes('bool');
  }

  export function isDateType(dt: string): boolean {
    return dt.toLowerCase().includes('date');
  }

  export function isDateTimeType(dt: string): boolean {
    const lower = dt.toLowerCase();
    return lower.includes('date') && lower.includes('time');
  }

  export function getInputType(dt: string): 'boolean' | 'datetime-local' | 'date' | 'text' {
    if (isBooleanType(dt)) return 'boolean';
    if (isDateTimeType(dt)) return 'datetime-local';
    if (isDateType(dt)) return 'date';
    return 'text';
  }

  // ── Local state ─────────────────────────────────────────────────────────────

  const inputType = $derived(getInputType(dataType));

  // For boolean: cycle null → true → false → null
  let boolState = $state<boolean | null>(
    untrack(() => typeof value === 'boolean' ? value : null),
  );

  // For text/date: string representation
  let textValue = $state<string>(
    untrack(() => value === null ? '' : String(value)),
  );

  let inputEl = $state<HTMLInputElement | null>(null);
  let cellEditorEl = $state<HTMLDivElement | null>(null);
  let actionsEl = $state<HTMLDivElement | null>(null);

  // Height of the actions bar — used to decide above/below placement
  const ACTIONS_HEIGHT = 28;
  const ACTIONS_GAP = 3;

  const actionsTop = $derived(
    top + height + ACTIONS_GAP + ACTIONS_HEIGHT > containerHeight
      ? top - ACTIONS_GAP - ACTIONS_HEIGHT
      : top + height + ACTIONS_GAP,
  );

  const actionsCenter = $derived(left + width / 2);

  onMount(() => {
    if (inputEl) {
      inputEl.focus();
      inputEl.select();
    }

    function handlePointerDown(e: PointerEvent): void {
      const target = e.target as Node;
      if (
        cellEditorEl && cellEditorEl.contains(target) ||
        actionsEl && actionsEl.contains(target)
      ) return;
      if (settings.clickOutsideEdit === 'confirm') {
        confirmEdit();
      } else {
        onCancel();
      }
    }

    document.addEventListener('pointerdown', handlePointerDown);
    return () => document.removeEventListener('pointerdown', handlePointerDown);
  });

  function cycleBool(): void {
    if (boolState === null) boolState = true;
    else if (boolState === true) boolState = false;
    else boolState = null;
  }

  function confirmEdit(): void {
    if (inputType === 'boolean') {
      onConfirm(boolState);
    } else if (textValue === '') {
      // Empty text — keep as empty string, not null (user can click Set NULL for that)
      onConfirm('');
    } else if (inputType === 'text') {
      // Try to coerce numbers back if the original DB value was a number
      const asNum = Number(textValue);
      if (typeof originalValue === 'number' && !isNaN(asNum) && textValue.trim() !== '') {
        onConfirm(asNum);
      } else {
        onConfirm(textValue);
      }
    } else {
      onConfirm(textValue);
    }
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') {
      e.preventDefault();
      confirmEdit();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }

  function boolLabel(v: boolean | null): string {
    if (v === null) return 'NULL';
    return v ? '✓' : '✗';
  }
</script>

<div
  bind:this={cellEditorEl}
  class="cell-editor"
  style="top: {top}px; left: {left}px; width: {width}px; height: {height}px;"
  role="dialog"
  aria-label="Edit cell"
  tabindex="-1"
  onkeydown={handleKeydown}
>
  {#if inputType === 'boolean'}
    <button
      class="bool-toggle"
      class:bool-true={boolState === true}
      class:bool-false={boolState === false}
      class:bool-null={boolState === null}
      onclick={cycleBool}
      title="Click to cycle: true → false → NULL"
      aria-label="Toggle boolean value"
    >
      {boolLabel(boolState)}
    </button>
  {:else if inputType === 'datetime-local'}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="datetime-local"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit datetime value"
    />
  {:else if inputType === 'date'}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="date"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit date value"
    />
  {:else}
    <input
      bind:this={inputEl}
      class="editor-input"
      type="text"
      bind:value={textValue}
      autocomplete="off"
      autocorrect="off"
      autocapitalize="off"
      spellcheck="false"
      aria-label="Edit cell value"
    />
  {/if}
</div>

<div
  bind:this={actionsEl}
  class="editor-actions"
  style="top: {actionsTop}px; left: {actionsCenter}px; transform: translateX(-50%);"
  role="toolbar"
  tabindex="0"
  aria-label="Edit actions"
  onkeydown={handleKeydown}
>
  {#if inputType !== 'boolean'}
    <button class="action-btn confirm-btn" onclick={confirmEdit} title="Confirm (Enter)">✓</button>
  {:else}
    <button class="action-btn confirm-btn" onclick={() => onConfirm(boolState)} title="Confirm">✓</button>
  {/if}
  <button
    class="action-btn null-btn"
    onclick={() => onConfirm(null)}
    title="Set to NULL"
    aria-label="Set to NULL"
  >
    NULL
  </button>
  <button class="action-btn cancel-btn" onclick={onCancel} title="Cancel (Escape)">✕</button>
</div>

<style>
  .cell-editor {
    position: absolute;
    z-index: 100;
    display: flex;
    align-items: stretch;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    box-sizing: border-box;
    overflow: hidden;
  }

  .editor-input {
    flex: 1;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-primary);
    border: none;
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    min-width: 0;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
  }

  .bool-toggle {
    flex: 1;
    width: 100%;
    padding: 0 var(--spacing-3);
    background: var(--color-bg-tertiary);
    border: none;
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-mono);
    cursor: pointer;
    transition: background var(--transition-fast);
    height: 100%;
  }

  .bool-toggle.bool-true {
    color: var(--color-success);
  }

  .bool-toggle.bool-false {
    color: var(--color-danger);
  }

  .bool-toggle.bool-null {
    color: var(--color-null);
    font-style: italic;
  }

  .bool-toggle:hover {
    background: var(--color-bg-hover);
  }

  .editor-actions {
    position: absolute;
    z-index: 100;
    display: flex;
    align-items: stretch;
    height: 28px;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow-md);
    overflow: hidden;
  }

  .action-btn {
    padding: 0 var(--spacing-2);
    background: transparent;
    border: none;
    border-left: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition: background var(--transition-fast);
    color: var(--color-text-secondary);
    font-family: var(--font-family-ui);
    height: 100%;
    white-space: nowrap;
  }

  .action-btn:first-child {
    border-left: none;
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
  }

  .confirm-btn:hover {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .null-btn {
    font-size: 10px;
    font-family: var(--font-family-mono);
    color: var(--color-null);
  }

  .null-btn:hover {
    background: var(--color-accent-subtle);
  }

  .cancel-btn:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }
</style>
