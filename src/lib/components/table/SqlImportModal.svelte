<!--
  SqlImportModal — imports a SQL file or clipboard text, showing progress as statements execute.
-->
<script lang="ts">
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import * as importApi from '$lib/tauri/import';
  import { onDestroy, onMount } from 'svelte';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    connectionId: string;
    source?: 'file' | 'clipboard';
    onclose: () => void;
    onimported?: (_statementCount: number) => void;
  }

  const { connectionId, source = 'file', onclose, onimported }: Props = $props();

  interface ProgressEvent {
    current: number;
    total: number | null;
    statement: string;
    error: string | null;
  }

  type Phase = 'pick' | 'running' | 'done';

  let phase = $state<Phase>('pick');
  let filePath = $state('');
  let sqlText = $state('');
  let stopOnError = $state(true);
  let progress = $state<ProgressEvent | null>(null);
  let errors = $state<string[]>([]);
  let executedCount = $state(0);
  let error = $state<string | null>(null);

  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    if (source === 'clipboard') {
      try {
        const text = await navigator.clipboard.readText();
        if (text.trim()) sqlText = text;
      } catch {
        // clipboard read failed — user will paste manually
      }
    }
  });

  onDestroy(() => {
    unlisten?.();
  });

  async function pickFile() {
    const selected = await openFileDialog({
      multiple: false,
      filters: [{ name: 'SQL Files', extensions: ['sql'] }],
    });
    if (typeof selected === 'string' && selected) {
      filePath = selected;
    }
  }

  async function startImport() {
    if (source === 'file' && !filePath) return;
    if (source === 'clipboard' && !sqlText.trim()) return;
    phase = 'running';
    errors = [];
    error = null;
    progress = null;
    unlisten = await listen<ProgressEvent>('import-sql-progress', (event) => {
      progress = event.payload;
      if (event.payload.error) {
        errors = [...errors, `Statement ${event.payload.current}: ${event.payload.error}`];
      }
    });

    try {
      if (source === 'clipboard') {
        executedCount = await importApi.importSqlText(connectionId, sqlText, stopOnError);
      } else {
        executedCount = await importApi.importSqlFile(connectionId, filePath, stopOnError);
      }
      phase = 'done';
      onimported?.(executedCount);
    } catch (err) {
      error = errorMessage(err);
      phase = 'pick';
    } finally {
      unlisten?.();
      unlisten = null;
    }
  }

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget && phase !== 'running') onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && phase !== 'running') onclose();
  }

  const progressPct = $derived(
    progress && progress.total ? Math.round((progress.current / progress.total) * 100) : null,
  );
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" role="dialog" aria-modal="true" aria-label="Import SQL File" tabindex="-1" onclick={handleBackdrop}>
  <div class="modal">
    <header class="modal-header">
      <h2 class="modal-title">Import SQL File</h2>
      {#if phase !== 'running'}
        <button class="close-btn" onclick={onclose} aria-label="Close">✕</button>
      {/if}
    </header>

    <div class="modal-body">
      {#if phase === 'pick'}
        <div class="pick-step">
          {#if source === 'clipboard'}
            <textarea
              class="paste-area"
              placeholder="Paste SQL here…"
              bind:value={sqlText}
              aria-label="SQL statements to execute"
            ></textarea>
          {:else}
            <div class="file-picker-row">
              <input
                class="file-input"
                type="text"
                value={filePath}
                readonly
                placeholder="No file selected…"
                aria-label="SQL file path"
              />
              <button class="btn btn--ghost btn--sm" onclick={pickFile}>Browse</button>
            </div>
          {/if}

          <div class="option-row">
            <label class="option-label" for="stop-on-error">
              Stop on first error
            </label>
            <input id="stop-on-error" type="checkbox" bind:checked={stopOnError} />
          </div>

          {#if error}
            <div class="error-msg">{error}</div>
          {/if}
        </div>

      {:else if phase === 'running'}
        <div class="running-step">
          {#if progress}
            <div class="progress-header">
              <span class="progress-count">
                Statement {progress.current}{progress.total ? ` of ${progress.total}` : ''}
              </span>
              {#if progressPct !== null}
                <span class="progress-pct">{progressPct}%</span>
              {/if}
            </div>

            {#if progressPct !== null}
              <div class="progress-bar-track" role="progressbar" aria-valuenow={progressPct} aria-valuemin={0} aria-valuemax={100}>
                <div class="progress-bar-fill" style="width: {progressPct}%"></div>
              </div>
            {/if}

            <pre class="statement-preview">{progress.statement}</pre>
          {:else}
            <div class="spinner-row">
              <div class="spinner" aria-label="Running…"></div>
              <span>Starting…</span>
            </div>
          {/if}

          {#if errors.length > 0}
            <div class="errors-list">
              <h3 class="errors-title">Errors ({errors.length})</h3>
              {#each errors as err}
                <div class="error-item">{err}</div>
              {/each}
            </div>
          {/if}
        </div>

      {:else if phase === 'done'}
        <div class="done-step">
          <span class="success-icon" aria-hidden="true">✓</span>
          <p>Executed <strong>{executedCount}</strong> statement{executedCount !== 1 ? 's' : ''}.</p>
          {#if errors.length > 0}
            <p class="error-summary">{errors.length} error{errors.length !== 1 ? 's' : ''} encountered.</p>
            <div class="errors-list">
              {#each errors as err}
                <div class="error-item">{err}</div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <footer class="modal-footer">
      {#if phase === 'pick'}
        <button class="btn btn--ghost" onclick={onclose}>Cancel</button>
        <button
          class="btn btn--primary"
          onclick={startImport}
          disabled={source === 'clipboard' ? !sqlText.trim() : !filePath}
        >
          Import
        </button>
      {:else if phase === 'done'}
        <button class="btn btn--primary" onclick={onclose}>Close</button>
      {/if}
    </footer>
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
    width: 560px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - var(--spacing-8));
    display: flex;
    flex-direction: column;
    overflow: hidden;
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
    padding: var(--spacing-5);
  }

  .modal-footer {
    padding: var(--spacing-4) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .pick-step {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .paste-area {
    width: 100%;
    min-height: 200px;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    resize: vertical;
    outline: none;
    box-sizing: border-box;
  }

  .paste-area:focus { border-color: var(--color-accent); }

  .file-picker-row {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .file-input {
    flex: 1;
    height: 28px;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono);
  }

  .option-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: var(--font-size-sm);
  }

  .option-label {
    color: var(--color-text-primary);
  }

  .running-step {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .progress-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

  .progress-count { font-weight: var(--font-weight-medium); }
  .progress-pct { color: var(--color-text-muted); }

  .progress-bar-track {
    height: 4px;
    background: var(--color-border);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-bar-fill {
    height: 100%;
    background: var(--color-accent);
    border-radius: 2px;
    transition: width 0.2s ease;
  }

  .statement-preview {
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 100px;
    overflow-y: auto;
    margin: 0;
  }

  .spinner-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--color-border);
    border-top-color: var(--color-accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  .errors-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
    max-height: 200px;
    overflow-y: auto;
  }

  .errors-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    margin-bottom: var(--spacing-1);
  }

  .error-item {
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .done-step {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-4);
    padding: var(--spacing-6) 0;
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  .success-icon {
    font-size: 40px;
    color: var(--color-success);
  }

  .error-summary {
    color: var(--color-danger);
    font-size: var(--font-size-sm);
    margin: 0;
  }

  .error-msg {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
  }

  .btn {
    height: 32px;
    padding: 0 var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn--sm {
    height: 28px;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-xs);
    flex-shrink: 0;
  }

  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover { background: var(--color-accent-hover); }

  .btn--ghost {
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .btn--ghost:not(:disabled):hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
