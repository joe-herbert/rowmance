<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';

  interface Props {
    statements: string[];
    onrun: () => void;
    oncancel: () => void;
    ondiscard: () => void;
    onopeneditor?: () => void;
  }

  const { statements, onrun, oncancel, ondiscard, onopeneditor }: Props = $props();

  let copied = $state(false);

  const allSql = $derived(statements.join('\n\n'));

  async function copyAll(): Promise<void> {
    await navigator.clipboard.writeText(allSql);
    copied = true;
    setTimeout(() => {
      copied = false;
    }, 1500);
  }
</script>

<Modal label="SQL Preview" zindex={400} onbackdropclick={oncancel}>
  <div class="modal-card">
    <div class="modal-header">
      <span class="modal-title">SQL Preview</span>
      <button class="copy-btn" onclick={copyAll} title="Copy all SQL">
        {#if copied}
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg
          >
          Copied
        {:else}
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="13"
            height="13"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            ><rect x="9" y="9" width="13" height="13" rx="2" ry="2" /><path
              d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"
            /></svg
          >
          Copy
        {/if}
      </button>
    </div>

    <div class="sql-body">
      <pre class="sql-pre">{allSql}</pre>
    </div>

    <div class="modal-footer">
      <button class="btn btn--discard" onclick={ondiscard}>Cancel and Discard</button>
      <div class="footer-right">
        {#if onopeneditor}
          <button class="btn" onclick={onopeneditor}>Open in Editor</button>
        {/if}
        <button class="btn" onclick={oncancel}>Cancel</button>
        <button class="btn btn--primary" onclick={onrun}>Run</button>
      </div>
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 620px;
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
    justify-content: space-between;
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .copy-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 3px var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .copy-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
  }

  .sql-body {
    flex: 1;
    overflow: auto;
    padding: var(--spacing-3) var(--spacing-4);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .sql-pre {
    margin: 0;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    line-height: 1.6;
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: break-all;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-3) var(--spacing-4);
    flex-shrink: 0;
  }

  .footer-right {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
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

  .btn:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }

  .btn--discard {
    background: var(--color-danger-subtle);
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .btn--discard:hover {
    background: var(--color-danger);
    border-color: var(--color-danger);
    color: white;
  }
</style>
