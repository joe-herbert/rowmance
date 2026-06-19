<!--
  CellEditorModal — full-screen modal cell editor mounted via portal.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { portal } from '$lib/actions/portal';
  import { useSettings } from '$lib/stores/settings.svelte';

  function getInputType(dt: string): 'boolean' | 'datetime-local' | 'date' | 'text' {
    const lower = dt.toLowerCase();
    if (lower.includes('bool')) return 'boolean';
    if (lower.includes('date') && lower.includes('time')) return 'datetime-local';
    if (lower.includes('date')) return 'date';
    return 'text';
  }

  type CellValue = string | number | boolean | null;

  interface Props {
    value: CellValue;
    originalValue: CellValue;
    colName: string;
    dataType: string;
    onConfirm: (_newValue: CellValue) => void;
    onCancel: () => void;
  }

  let { value, originalValue, colName, dataType, onConfirm, onCancel }: Props = $props();

  const { settings } = useSettings();

  const inputType = $derived(getInputType(dataType));

  let boolState = $state<boolean | null>(
    untrack(() => (typeof value === 'boolean' ? value : null)),
  );

  let textValue = $state<string>(untrack(() => (value === null ? '' : String(value))));

  let inputEl = $state<HTMLInputElement | HTMLTextAreaElement | null>(null);
  let modalEl = $state<HTMLDivElement | null>(null);

  onMount(() => {
    if (inputEl) {
      inputEl.focus();
      if ('select' in inputEl) inputEl.select();
    }
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
      onConfirm('');
    } else if (inputType === 'text') {
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
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      confirmEdit();
    }
  }

  function handleBackdropPointerDown(e: PointerEvent): void {
    if (e.target !== e.currentTarget) return;
    if (settings.clickOutsideEdit === 'confirm') {
      confirmEdit();
    } else {
      onCancel();
    }
  }

  function boolLabel(v: boolean | null): string {
    if (v === null) return 'NULL';
    return v ? '✓ true' : '✗ false';
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="modal-backdrop"
  onpointerdown={handleBackdropPointerDown}
  onkeydown={handleKeydown}
  use:portal
>
  <div
    bind:this={modalEl}
    class="modal-dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Edit {colName}"
  >
    <header class="modal-header">
      <span class="modal-title">{colName}</span>
      <span class="modal-type">{dataType}</span>
    </header>

    <div class="modal-body">
      {#if inputType === 'boolean'}
        <button
          class="bool-toggle"
          class:bool-true={boolState === true}
          class:bool-false={boolState === false}
          class:bool-null={boolState === null}
          onclick={cycleBool}
          title="Click to cycle: true → false → NULL"
        >
          {boolLabel(boolState)}
        </button>
      {:else if inputType === 'datetime-local'}
        <input
          bind:this={inputEl}
          class="modal-input"
          type="datetime-local"
          bind:value={textValue}
          autocomplete="off"
          spellcheck="false"
          aria-label="Edit datetime value"
        />
      {:else if inputType === 'date'}
        <input
          bind:this={inputEl}
          class="modal-input"
          type="date"
          bind:value={textValue}
          autocomplete="off"
          spellcheck="false"
          aria-label="Edit date value"
        />
      {:else}
        <textarea
          bind:this={inputEl}
          class="modal-textarea"
          bind:value={textValue}
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          aria-label="Edit cell value"
        ></textarea>
      {/if}
    </div>

    <footer class="modal-footer">
      <span class="modal-hint">
        {inputType === 'boolean' ? 'Click to cycle value' : 'Ctrl+Enter to confirm · Escape to cancel'}
      </span>
      <div class="modal-actions">
        <button class="modal-btn btn-null" onclick={() => onConfirm(null)}>Set NULL</button>
        <button class="modal-btn btn-cancel" onclick={onCancel}>Cancel</button>
        <button class="modal-btn btn-confirm" onclick={confirmEdit}>Confirm</button>
      </div>
    </footer>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 500;
    background: rgba(0, 0, 0, 0.55);
    -webkit-backdrop-filter: blur(2px);
    backdrop-filter: blur(2px);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .modal-dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: min(600px, calc(100vw - 48px));
    max-height: calc(100vh - 96px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .modal-type {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
  }

  .modal-body {
    flex: 1;
    padding: var(--spacing-3) var(--spacing-4);
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .modal-textarea {
    flex: 1;
    width: 100%;
    min-height: 160px;
    resize: vertical;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    line-height: 1.5;
    box-sizing: border-box;
    transition: border-color var(--transition-fast);
  }

  .modal-textarea:focus {
    border-color: var(--color-accent);
  }

  .modal-input {
    width: 100%;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    box-sizing: border-box;
    transition: border-color var(--transition-fast);
  }

  .modal-input:focus {
    border-color: var(--color-accent);
  }

  .bool-toggle {
    align-self: flex-start;
    padding: var(--spacing-2) var(--spacing-4);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-mono);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .bool-toggle.bool-true {
    color: var(--color-success);
    border-color: var(--color-success);
  }

  .bool-toggle.bool-false {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .bool-toggle.bool-null {
    color: var(--color-null);
    font-style: italic;
  }

  .bool-toggle:hover {
    background: var(--color-bg-hover);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-3);
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .modal-actions {
    display: flex;
    gap: var(--spacing-2);
  }

  .modal-btn {
    padding: var(--spacing-1) var(--spacing-3);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    border: 1px solid var(--color-border);
    transition: background var(--transition-fast), color var(--transition-fast);
    background: transparent;
    color: var(--color-text-primary);
  }

  .modal-btn:hover {
    background: var(--color-bg-hover);
  }

  .btn-null {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-null);
  }

  .btn-null:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-cancel:hover {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .btn-confirm {
    background: var(--color-accent);
    color: #fff;
    border-color: var(--color-accent);
  }

  .btn-confirm:hover {
    background: color-mix(in srgb, var(--color-accent) 85%, black);
  }
</style>
