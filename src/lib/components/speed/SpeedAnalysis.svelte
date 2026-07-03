<!--
  SpeedAnalysis — dev-only panel showing timing breakdowns for slow queries (>= 500ms).
  Opened via Developer → Speed Analysis in the menu bar.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '$lib/tauri/speed_analysis';
  import type { SpeedAnalysisEntry } from '$lib/tauri/speed_analysis';

  let entries = $state<SpeedAnalysisEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await api.speedAnalysisList();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function clearAll() {
    await api.speedAnalysisClear();
    entries = [];
  }

  onMount(load);

  const avg = $derived(
    entries.length > 0
      ? Math.round(entries.reduce((s, e) => s + e.totalUs, 0) / entries.length)
      : 0,
  );
  const max = $derived(entries.length > 0 ? Math.max(...entries.map((e) => e.totalUs)) : 0);

  function formatUs(us: number): string {
    if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(2)}s`;
    if (us >= 1_000) return `${(us / 1_000).toFixed(1)}ms`;
    return `${us}µs`;
  }

  function formatSql(sql: string): string {
    const trimmed = sql.trim().replace(/\s+/g, ' ');
    return trimmed.length > 120 ? trimmed.slice(0, 120) + '…' : trimmed;
  }

  function formatTime(iso: string): string {
    try {
      return new Date(iso + 'Z').toLocaleTimeString();
    } catch {
      return iso;
    }
  }

  function segmentWidth(ms: number, total: number): string {
    if (total === 0) return '0%';
    return `${Math.max(0.5, (ms / total) * 100).toFixed(1)}%`;
  }
</script>

<div class="speed-panel">
  <div class="header">
    <div class="title-row">
      <h2 class="title">Speed Analysis</h2>
      <span class="badge">dev only</span>
    </div>
    <div class="actions">
      <button class="btn-ghost" onclick={load} disabled={loading}>Refresh</button>
      <button class="btn-danger" onclick={clearAll} disabled={entries.length === 0}>Clear</button>
    </div>
  </div>

  {#if loading}
    <div class="state-message">Loading…</div>
  {:else if error}
    <div class="state-message error">{error}</div>
  {:else if entries.length === 0}
    <div class="empty">
      <p class="empty-title">No queries recorded</p>
    </div>
  {:else}
    <div class="summary">
      <div class="stat">
        <span class="stat-value">{entries.length}</span>
        <span class="stat-label">recorded</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatUs(avg)}</span>
        <span class="stat-label">avg total</span>
      </div>
      <div class="stat">
        <span class="stat-value">{formatUs(max)}</span>
        <span class="stat-label">max total</span>
      </div>
      <div class="legend">
        <span class="legend-dot pool"></span>pool acquire
        <span class="legend-dot switch"></span>db switch
        <span class="legend-dot exec"></span>execution
        <span class="legend-dot proc"></span>processing
      </div>
    </div>

    <div class="entries">
      {#each entries as entry (entry.id)}
        <div class="entry">
          <div class="entry-header">
            <span class="entry-time">{formatTime(entry.executedAt)}</span>
            <span class="entry-total">{formatUs(entry.totalUs)}</span>
          </div>
          <div class="entry-sql">{formatSql(entry.sql)}</div>
          <div class="bar-container">
            {#if entry.poolAcquireUs > 0}
              <div
                class="bar-seg pool"
                style="width: {segmentWidth(entry.poolAcquireUs, entry.totalUs)}"
                title="Pool acquire: {entry.poolAcquireUs}µs"
              ></div>
            {/if}
            {#if entry.dbSwitchUs > 0}
              <div
                class="bar-seg switch"
                style="width: {segmentWidth(entry.dbSwitchUs, entry.totalUs)}"
                title="DB switch: {entry.dbSwitchUs}µs"
              ></div>
            {/if}
            {#if entry.executionUs > 0}
              <div
                class="bar-seg exec"
                style="width: {segmentWidth(entry.executionUs, entry.totalUs)}"
                title="Execution: {entry.executionUs}µs"
              ></div>
            {/if}
            {#if entry.resultProcessingUs > 0}
              <div
                class="bar-seg proc"
                style="width: {segmentWidth(entry.resultProcessingUs, entry.totalUs)}"
                title="Result processing: {entry.resultProcessingUs}µs"
              ></div>
            {/if}
          </div>
          <div class="entry-breakdown">
            {#if entry.poolAcquireUs > 0}
              <span class="phase pool">{formatUs(entry.poolAcquireUs)} pool</span>
            {/if}
            {#if entry.dbSwitchUs > 0}
              <span class="phase switch">{formatUs(entry.dbSwitchUs)} switch</span>
            {/if}
            <span class="phase exec">{formatUs(entry.executionUs)} exec</span>
            {#if entry.resultProcessingUs > 0}
              <span class="phase proc">{formatUs(entry.resultProcessingUs)} proc</span>
            {/if}
            {#if entry.rowCount !== null}
              <span class="row-count">{entry.rowCount.toLocaleString()} rows</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .speed-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--color-bg-primary);
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin: 0;
  }

  .badge {
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    color: var(--color-warning);
    background: var(--color-warning-subtle);
    border: 1px solid var(--color-warning);
    border-radius: 3px;
    padding: 1px 5px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    opacity: 0.8;
  }

  .actions {
    display: flex;
    gap: var(--spacing-2);
  }

  .btn-ghost {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .btn-ghost:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .btn-ghost:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .btn-danger {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
    background: transparent;
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-sm);
    cursor: pointer;
    opacity: 0.7;
  }

  .btn-danger:hover:not(:disabled) {
    background: var(--color-danger-subtle);
    opacity: 1;
  }

  .btn-danger:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .summary {
    display: flex;
    align-items: center;
    gap: var(--spacing-4);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stat-value {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    font-variant-numeric: tabular-nums;
  }

  .stat-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: auto;
    flex-wrap: wrap;
  }

  .legend-dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .entries {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-2) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .entry {
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .entry-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .entry-time {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-variant-numeric: tabular-nums;
  }

  .entry-total {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-danger);
    font-variant-numeric: tabular-nums;
  }

  .entry-sql {
    font-family: var(--font-family-mono, monospace);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    white-space: pre-wrap;
    word-break: break-all;
    line-height: 1.4;
  }

  .bar-container {
    display: flex;
    height: 8px;
    border-radius: 4px;
    overflow: hidden;
    background: var(--color-border);
    gap: 1px;
  }

  .bar-seg {
    height: 100%;
    transition: opacity 0.1s;
    min-width: 2px;
  }

  .bar-seg:hover {
    opacity: 0.75;
  }

  .entry-breakdown {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-2);
    align-items: center;
  }

  .phase {
    font-size: var(--font-size-xs);
    padding: 1px 6px;
    border-radius: 3px;
    font-variant-numeric: tabular-nums;
  }

  .row-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: auto;
  }

  /* Segment colours */
  .pool { background-color: #f59e0b; }
  .switch { background-color: #3b82f6; }
  .exec { background-color: #ef4444; }
  .proc { background-color: #10b981; }

  .legend-dot.pool { background-color: #f59e0b; }
  .legend-dot.switch { background-color: #3b82f6; }
  .legend-dot.exec { background-color: #ef4444; }
  .legend-dot.proc { background-color: #10b981; }

  .phase.pool { color: #b45309; background: rgba(245, 158, 11, 0.15); }
  .phase.switch { color: #1d4ed8; background: rgba(59, 130, 246, 0.15); }
  .phase.exec { color: #b91c1c; background: rgba(239, 68, 68, 0.15); }
  .phase.proc { color: #065f46; background: rgba(16, 185, 129, 0.15); }

  .state-message {
    padding: var(--spacing-6) var(--spacing-4);
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .state-message.error {
    color: var(--color-danger);
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-2);
    padding: var(--spacing-8);
  }

  .empty-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    margin: 0;
  }

  .empty-sub {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
    text-align: center;
  }
</style>
