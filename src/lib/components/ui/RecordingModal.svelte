<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';
  import { useRecording } from '$lib/stores/recording.svelte';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { savedQueriesInvalidator } from '$lib/stores/savedQueriesInvalidator.svelte';

  const recording = useRecording();

  let saveNameInput = $state('');
  let isSaving = $state(false);
  let copied = $state(false);
  let showSaveForm = $state(false);
  let saveNameInputEl = $state<HTMLInputElement | undefined>(undefined);

  const combinedSql = $derived(recording.statements.map((s) => s.sql.trim()).join(';\n\n'));
  const connectionId = $derived(recording.statements[0]?.connectionId ?? '');
  const database = $derived(recording.statements[0]?.database ?? null);

  function copyAll() {
    navigator.clipboard.writeText(combinedSql);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  function openSaveForm() {
    showSaveForm = true;
    setTimeout(() => saveNameInputEl?.focus(), 0);
  }

  async function confirmSave() {
    if (!saveNameInput.trim() || isSaving) return;
    isSaving = true;
    try {
      await savedQueriesApi.fileCreateSavedQuery({
        name: saveNameInput.trim(),
        sql: combinedSql,
        connectionId,
        database,
      });
      savedQueriesInvalidator.invalidate();
      recording.closeReview();
    } finally {
      isSaving = false;
    }
  }

  function discard() {
    recording.closeReview();
  }
</script>

<Modal label="Recording" onbackdropclick={discard}>
  <div class="modal-card">
    <div class="modal-header">
      <span class="modal-title">Recording</span>
      <span class="stmt-count">{recording.statements.length} statement{recording.statements.length !== 1 ? 's' : ''}</span>
    </div>

    <div class="statements-list">
      {#each recording.statements as stmt, i (i)}
        <div class="statement-item">
          <span class="stmt-number">{i + 1}</span>
          <pre class="stmt-sql">{stmt.sql.trim()}</pre>
        </div>
      {/each}
    </div>

    <div class="modal-footer">
      {#if showSaveForm}
        <form
          class="save-form"
          onsubmit={(e) => {
            e.preventDefault();
            confirmSave();
          }}
        >
          <input
            bind:this={saveNameInputEl}
            bind:value={saveNameInput}
            class="save-input"
            type="text"
            placeholder="Query name"
            maxlength="120"
            autocomplete="off"
            spellcheck={false}
            onkeydown={(e) => {
              if (e.key === 'Escape') showSaveForm = false;
            }}
          />
          <button type="submit" class="btn btn--primary" disabled={!saveNameInput.trim() || isSaving}>
            Save
          </button>
          <button type="button" class="btn" onclick={() => (showSaveForm = false)}>Cancel</button>
        </form>
      {:else}
        <button class="btn" onclick={discard}>Discard</button>
        <div class="footer-right">
          <button class="btn" onclick={copyAll}>
            {copied ? 'Copied!' : 'Copy All'}
          </button>
          <button class="btn btn--primary" onclick={openSaveForm}>Save Query…</button>
        </div>
      {/if}
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 600px;
    max-width: 92vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .stmt-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .statements-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-2) 0;
  }

  .statement-item {
    display: flex;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
  }

  .statement-item:last-child {
    border-bottom: none;
  }

  .stmt-number {
    flex-shrink: 0;
    width: 20px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    padding-top: 1px;
    text-align: right;
  }

  .stmt-sql {
    margin: 0;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .footer-right {
    display: flex;
    gap: var(--spacing-2);
  }

  .save-form {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
    width: 100%;
  }

  .save-input {
    flex: 1;
    height: 28px;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    outline: none;
  }

  .save-input:focus {
    border-color: var(--color-accent);
  }

  .btn {
    height: 28px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    transition: all var(--transition-fast);
    white-space: nowrap;
    font-family: var(--font-family-ui);
  }

  .btn:hover:not(:disabled) {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover:not(:disabled) {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }
</style>
