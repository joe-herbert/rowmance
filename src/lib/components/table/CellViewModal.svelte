<!--
  CellViewModal — full-screen modal cell viewer mounted via portal.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';

  type CellValue = string | number | boolean | null;

  interface Props {
    value: CellValue;
    colName: string;
    dataType: string;
    onClose: () => void;
  }

  let { value, colName, dataType, onClose }: Props = $props();

  const isJsonType = $derived(
    dataType.toLowerCase() === 'json' || dataType.toLowerCase() === 'jsonb',
  );

  let textValue = $state<string>(untrack(() => (value === null ? '' : String(value))));
  let copied = $state(false);

  function contentLooksLikeJson(v: string): boolean {
    const t = v.trim();
    if (!t || (t[0] !== '{' && t[0] !== '[')) return false;
    try {
      const parsed = JSON.parse(t);
      return typeof parsed === 'object' && parsed !== null;
    } catch {
      return false;
    }
  }

  const showFormatJson = $derived(
    value !== null && (isJsonType || contentLooksLikeJson(textValue)),
  );

  const hasInvalidJson = $derived(
    isJsonType &&
      value !== null &&
      textValue.trim() !== '' &&
      (() => {
        try {
          JSON.parse(textValue);
          return false;
        } catch {
          return true;
        }
      })(),
  );

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    }
  }

  function formatJson(): void {
    try {
      textValue = JSON.stringify(JSON.parse(textValue), null, 2);
    } catch {
      // not valid JSON, do nothing
    }
  }

  async function copyValue(): Promise<void> {
    try {
      await navigator.clipboard.writeText(textValue);
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 1500);
    } catch {
      // clipboard unavailable
    }
  }
</script>

<Modal zindex={500} label="View {colName}" onbackdropclick={onClose}>
  <div class="modal-dialog" role="presentation" onkeydown={handleKeydown}>
    <header class="modal-header">
      <span class="modal-title">{colName}</span>
      <span class="modal-type">{dataType}</span>
    </header>

    <div class="modal-body">
      {#if value === null}
        <div class="null-display">NULL</div>
      {:else}
        <pre class="modal-pre" class:invalid-json={hasInvalidJson}>{textValue}</pre>
      {/if}
    </div>

    <footer class="modal-footer">
      <div class="modal-footer-left">
        <span class="modal-hint">Escape to close</span>
        {#if hasInvalidJson}
          <span class="modal-invalid-json">Invalid JSON</span>
        {/if}
      </div>
      <div class="modal-actions">
        {#if showFormatJson}
          <button class="modal-btn btn-format-json" onclick={formatJson}>Format JSON</button>
        {/if}
        {#if value !== null}
          <button class="modal-btn btn-copy" onclick={copyValue}>
            {copied ? 'Copied!' : 'Copy'}
          </button>
        {/if}
        <button class="modal-btn btn-close" onclick={onClose}>Close</button>
      </div>
    </footer>
  </div>
</Modal>

<style>
  .modal-dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: min(600px, calc(100vw - 48px));
    max-height: 90vh;
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
    padding: var(--spacing-3) var(--spacing-4);
    overflow-y: auto;
    min-height: 0;
  }

  .null-display {
    min-height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-null);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
  }

  .modal-pre {
    margin: 0;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-all;
    -webkit-user-select: text;
    user-select: text;
  }

  .modal-pre.invalid-json {
    border-color: var(--color-danger);
    background: var(--color-danger-subtle);
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

  .modal-footer-left {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .modal-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .modal-invalid-json {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-danger);
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
    color: var(--color-text-primary);
  }

  .modal-btn:hover {
    background: var(--color-bg-hover);
  }

  .btn-format-json {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
  }

  .btn-format-json:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-copy {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
  }

  .btn-close:hover {
    background: var(--color-bg-hover);
  }
</style>
