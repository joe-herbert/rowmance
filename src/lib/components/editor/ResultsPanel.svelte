<script lang="ts">
  import type { QueryResult } from '$lib/types';
  import DataTable from '$lib/components/table/DataTable.svelte';

  interface Props {
    result: QueryResult | null;
  }

  let { result }: Props = $props();

  let rowLabel = $derived(
    result
      ? `${result.rows.length.toLocaleString()} row${result.rows.length !== 1 ? 's' : ''}`
      : '',
  );

  let durationLabel = $derived(result ? `${result.durationMs}ms` : '');

  let affectedLabel = $derived(
    result?.affectedRows != null ? `${result.affectedRows} affected` : '',
  );

  // Whether we have a meaningful data payload (successful query with columns).
  let hasData = $derived(result !== null && result.error === null && result.columns.length > 0);
</script>

<div class="results-panel">
  {#if result === null}
    <div class="placeholder">
      <span class="placeholder-text">Run a query to see results</span>
    </div>
  {:else if result.error !== null}
    <div class="error-box" role="alert">
      <span class="error-label">Error</span>
      <span class="error-message">{result.error}</span>
    </div>
  {:else if hasData}
    <div class="table-wrapper">
      <DataTable columns={result.columns} rows={result.rows} />
    </div>
    <div class="status-bar">
      <span class="status-item">{rowLabel}</span>
      {#if durationLabel}
        <span class="status-separator">·</span>
        <span class="status-item">{durationLabel}</span>
      {/if}
      {#if affectedLabel}
        <span class="status-separator">·</span>
        <span class="status-item">{affectedLabel}</span>
      {/if}
    </div>
  {:else}
    <!-- Query ran successfully but returned no columns (e.g. DDL or empty result). -->
    <div class="empty-result">
      <span class="empty-text">Query executed successfully — no rows returned.</span>
    </div>
    <div class="status-bar">
      {#if durationLabel}
        <span class="status-item">{durationLabel}</span>
      {/if}
      {#if affectedLabel}
        <span class="status-separator">·</span>
        <span class="status-item">{affectedLabel}</span>
      {/if}
    </div>
  {/if}
</div>

<style>
  .results-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .placeholder-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .error-box {
    margin: var(--spacing-4);
    padding: var(--spacing-3) var(--spacing-4);
    background: var(--color-danger-subtle);
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .error-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .error-message {
    font-size: var(--font-size-sm);
    color: var(--color-danger);
    font-family: var(--font-family-mono);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .table-wrapper {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .empty-result {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .status-bar {
    flex: 0 0 var(--statusbar-height);
    min-height: var(--statusbar-height);
    max-height: var(--statusbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-top: 1px solid var(--color-border);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .status-separator {
    color: var(--color-border-strong);
  }

  .status-item {
    font-variant-numeric: tabular-nums;
  }
</style>
