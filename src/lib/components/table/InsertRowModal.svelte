<script lang="ts">
  import { insertRow } from '$lib/tauri/query';
  import { errorMessage } from '$lib/utils/errors';
  import type { ColumnMeta } from '$lib/types';

  interface Props {
    connectionId: string;
    database: string;
    table: string;
    columns: ColumnMeta[];
    onclose: () => void;
    oninserted: () => void;
  }

  const { connectionId, database, table, columns, onclose, oninserted }: Props = $props();

  let fieldValues = $state<Record<string, string>>(
    Object.fromEntries(columns.map((c) => [c.name, ''])),
  );
  let isSubmitting = $state(false);
  let submitError = $state<string | null>(null);

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }

  async function handleSubmit(e: SubmitEvent) {
    e.preventDefault();
    isSubmitting = true;
    submitError = null;

    const values: Record<string, unknown> = {};
    for (const col of columns) {
      const raw = fieldValues[col.name];
      if (raw === '') {
        if (col.nullable) {
          values[col.name] = null;
        }
        // skip non-nullable empty fields — DB will use its default or error
      } else {
        values[col.name] = raw;
      }
    }

    // Don't send if all skipped (shouldn't happen with at least one required column)
    if (Object.keys(values).length === 0) {
      submitError = 'Please fill in at least one column.';
      isSubmitting = false;
      return;
    }

    try {
      await insertRow(connectionId, database, table, values);
      oninserted();
    } catch (err) {
      submitError = errorMessage(err);
    } finally {
      isSubmitting = false;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="backdrop"
  role="dialog"
  aria-modal="true"
  aria-label="Insert new row"
  tabindex="-1"
  onclick={handleBackdrop}
  onkeydown={handleKeydown}
>
  <div class="modal" onclick={(e) => e.stopPropagation()} role="presentation" onkeydown={undefined}>
    <header class="modal-header">
      <h2 class="modal-title">Insert row</h2>
      <button class="close-btn" onclick={onclose} aria-label="Close">✕</button>
    </header>

    <form class="modal-body" onsubmit={handleSubmit}>
      <div class="fields">
        {#each columns as col, i}
          <div class="field-row">
            <label class="field-label" for="insert-col-{col.name}">
              <span class="col-name">{col.name}</span>
              <span class="col-type">{col.dataType}</span>
              {#if !col.nullable}
                <span class="required-dot" title="Not nullable">*</span>
              {/if}
            </label>
            <input
              id="insert-col-{col.name}"
              class="field-input"
              type="text"
              placeholder={col.nullable ? 'NULL' : ''}
              bind:value={fieldValues[col.name]}
              autofocus={i === 0}
              autocomplete="off"
              spellcheck="false"
            />
          </div>
        {/each}
      </div>

      {#if submitError !== null}
        <div class="error-bar" role="alert">
          <span class="error-msg">{submitError}</span>
          <button type="button" class="error-close" onclick={() => (submitError = null)} aria-label="Dismiss">✕</button>
        </div>
      {/if}

      <footer class="modal-footer">
        <button type="button" class="btn btn-secondary" onclick={onclose} disabled={isSubmitting}>
          Cancel
        </button>
        <button type="submit" class="btn btn-primary" disabled={isSubmitting}>
          {isSubmitting ? 'Inserting…' : 'Insert row'}
        </button>
      </footer>
    </form>
  </div>
</div>

<style>
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
    width: 480px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - var(--spacing-8));
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
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
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-4) var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .field-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: center;
    gap: var(--spacing-3);
  }

  .field-label {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    overflow: hidden;
  }

  .col-name {
    font-family: var(--font-family-mono);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-type {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .required-dot {
    color: var(--color-error, #e53e3e);
    font-size: var(--font-size-xs);
    flex-shrink: 0;
  }

  .field-input {
    width: 100%;
    padding: var(--spacing-1) var(--spacing-2);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
    background: var(--color-bg-input, var(--color-bg-elevated));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
    transition: border-color var(--transition-fast);
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: var(--color-accent);
  }

  .field-input::placeholder {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .error-bar {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    background: var(--color-error-bg, rgba(229, 62, 62, 0.1));
    border: 1px solid var(--color-error, #e53e3e);
    border-radius: var(--radius-sm);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-error, #e53e3e);
  }

  .error-msg {
    flex: 1;
    user-select: text;
  }

  .error-close {
    color: var(--color-error, #e53e3e);
    font-size: var(--font-size-xs);
    flex-shrink: 0;
  }

  .modal-footer {
    padding: var(--spacing-3) 0 0;
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .btn {
    padding: var(--spacing-1) var(--spacing-4);
    font-size: var(--font-size-sm);
    border-radius: var(--radius-sm);
    font-weight: var(--font-weight-medium);
    transition: background var(--transition-fast), color var(--transition-fast);
    cursor: pointer;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: transparent;
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .btn-primary {
    background: var(--color-accent);
    border: 1px solid transparent;
    color: var(--color-accent-fg, #fff);
  }

  .btn-primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }
</style>
