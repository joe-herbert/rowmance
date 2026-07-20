<!--
  SpeedAnalysis — dev-only panel showing timing breakdowns for slow queries (>= 500ms).
  Opened via Developer → Speed Analysis in the menu bar.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '$lib/tauri/speed_analysis';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import type { SpeedAnalysisEntry } from '$lib/tauri/speed_analysis';
  import { useConnections } from '$lib/stores/connections.svelte';
  import Select from '$lib/components/ui/Select.svelte';

  type StepKey = 'total' | 'pool' | 'switch' | 'exec' | 'proc' | 'rows';
  type SortField = 'date' | 'total' | 'pool' | 'switch' | 'exec' | 'proc' | 'rows';
  type SortDir = 'asc' | 'desc';

  interface TimingFilter {
    id: string;
    step: StepKey;
    min: string;
    max: string;
  }

  let entries = $state<SpeedAnalysisEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let timingFilters = $state<TimingFilter[]>([]);
  type SshFilter = 'all' | 'ssh' | 'no-ssh';
  let filterSsh = $state<SshFilter>('all');
  let filterConnectionId = $state('');

  let sortField = $state<SortField>('date');
  let sortDir = $state<SortDir>('desc');

  const connections = useConnections();

  function stepUs(entry: SpeedAnalysisEntry, step: StepKey): number {
    if (step === 'total') return entry.totalUs;
    if (step === 'pool') return entry.poolAcquireUs;
    if (step === 'switch') return entry.dbSwitchUs;
    if (step === 'exec') return entry.executionUs;
    return entry.resultProcessingUs;
  }

  function addFilter() {
    timingFilters = [
      ...timingFilters,
      { id: Math.random().toString(36).slice(2), step: 'total', min: '', max: '' },
    ];
  }

  function removeFilter(id: string) {
    timingFilters = timingFilters.filter((f) => f.id !== id);
  }

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

  function clearFilters() {
    timingFilters = [];
    filterSsh = 'all';
    filterConnectionId = '';
  }

  onMount(load);

  const entryConnectionIds = $derived([...new Set(entries.map((e) => e.connectionId))]);

  const hasActiveFilter = $derived(
    timingFilters.length > 0 || filterSsh !== 'all' || filterConnectionId !== '',
  );

  function sortValue(entry: SpeedAnalysisEntry, field: SortField): number | string {
    switch (field) {
      case 'date':
        return entry.executedAt;
      case 'total':
        return entry.totalUs;
      case 'pool':
        return entry.poolAcquireUs;
      case 'switch':
        return entry.dbSwitchUs;
      case 'exec':
        return entry.executionUs;
      case 'proc':
        return entry.resultProcessingUs;
      case 'rows':
        return entry.rowCount ?? -Infinity;
    }
  }

  const filteredEntries = $derived.by(() => {
    const filtered = entries.filter((entry) => {
      for (const f of timingFilters) {
        if (f.step === 'rows') {
          const rows = entry.rowCount;
          if (f.min !== '') {
            if (rows === null || rows < parseFloat(f.min)) return false;
          }
          if (f.max !== '') {
            if (rows === null || rows > parseFloat(f.max)) return false;
          }
        } else {
          const us = stepUs(entry, f.step);
          if (f.min !== '') {
            if (us < parseFloat(f.min) * 1000) return false;
          }
          if (f.max !== '') {
            if (us > parseFloat(f.max) * 1000) return false;
          }
        }
      }

      if (filterSsh !== 'all') {
        const sshEnabled =
          connections.profiles.find((p) => p.id === entry.connectionId)?.sshEnabled ?? false;
        if (filterSsh === 'ssh' && !sshEnabled) return false;
        if (filterSsh === 'no-ssh' && sshEnabled) return false;
      }

      if (filterConnectionId !== '' && entry.connectionId !== filterConnectionId) return false;

      return true;
    });

    const dir = sortDir === 'desc' ? -1 : 1;
    return filtered.slice().sort((a, b) => {
      const av = sortValue(a, sortField);
      const bv = sortValue(b, sortField);
      if (av < bv) return -dir;
      if (av > bv) return dir;
      return 0;
    });
  });

  const avg = $derived(
    filteredEntries.length > 0
      ? Math.round(filteredEntries.reduce((s, e) => s + e.totalUs, 0) / filteredEntries.length)
      : 0,
  );
  const max = $derived(
    filteredEntries.length > 0 ? Math.max(...filteredEntries.map((e) => e.totalUs)) : 0,
  );

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
      return new Date(iso + 'Z').toLocaleString(undefined, {
        month: 'short',
        day: 'numeric',
        hour: 'numeric',
        minute: '2-digit',
        second: '2-digit',
      });
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

  {#if !loading && !error && entries.length > 0}
    <div class="filters">
      {#if timingFilters.length > 0}
        <div class="filter-rows">
          {#each timingFilters as f (f.id)}
            <div class="filter-row">
              <Select
                size="xs"
                bind:value={f.step}
                options={[
                  { value: 'total', label: 'Total' },
                  { value: 'pool', label: 'Pool' },
                  { value: 'switch', label: 'Switch' },
                  { value: 'exec', label: 'Exec' },
                  { value: 'proc', label: 'Proc' },
                  { value: 'rows', label: 'Rows' },
                ]}
              />
              <span class="filter-op">≥</span>
              <input
                class="filter-input"
                type="number"
                min="0"
                placeholder={f.step === 'rows' ? 'rows' : 'ms'}
                bind:value={f.min}
              />
              <span class="filter-op">≤</span>
              <input
                class="filter-input"
                type="number"
                min="0"
                placeholder={f.step === 'rows' ? 'rows' : 'ms'}
                bind:value={f.max}
              />
              <button
                class="filter-remove"
                onclick={() => removeFilter(f.id)}
                aria-label="Remove filter">×</button
              >
            </div>
          {/each}
        </div>
      {/if}
      <div class="filter-toolbar">
        <button class="btn-ghost" onclick={addFilter}>+ Add filter</button>
        <Select
          size="xs"
          bind:value={filterSsh}
          options={[
            { value: 'all', label: 'All SSH states' },
            { value: 'ssh', label: 'SSH only' },
            { value: 'no-ssh', label: 'Not SSH' },
          ]}
        />
        {#if entryConnectionIds.length > 1}
          <Select
            size="xs"
            bind:value={filterConnectionId}
            options={[
              { value: '', label: 'All connections' },
              ...entryConnectionIds.map((cid) => ({
                value: cid,
                label: connections.profiles.find((p) => p.id === cid)?.name ?? cid,
              })),
            ]}
          />
        {/if}
        <span class="toolbar-divider"></span>
        <span class="toolbar-label">Sort</span>
        <Select
          size="xs"
          bind:value={sortField}
          options={[
            { value: 'date', label: 'Date' },
            { value: 'total', label: 'Total time' },
            { value: 'pool', label: 'Pool time' },
            { value: 'switch', label: 'Switch time' },
            { value: 'exec', label: 'Exec time' },
            { value: 'proc', label: 'Proc time' },
            { value: 'rows', label: 'Rows' },
          ]}
        />
        <button
          class="sort-dir-btn"
          onclick={() => (sortDir = sortDir === 'desc' ? 'asc' : 'desc')}
          title={sortDir === 'desc' ? 'Descending' : 'Ascending'}
          >{sortDir === 'desc' ? '↓' : '↑'}</button
        >
        {#if hasActiveFilter}
          <button class="btn-ghost filter-clear" onclick={clearFilters}>Clear filters</button>
        {/if}
      </div>
    </div>
  {/if}

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
        <span class="stat-value"
          >{filteredEntries.length}{#if hasActiveFilter}<span class="stat-total"
              >/{entries.length}</span
            >{/if}</span
        >
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
      {#each filteredEntries as entry (entry.id)}
        <div class="entry">
          <div class="entry-header">
            <span class="entry-time">{formatTime(entry.executedAt)}</span>
            <span class="entry-total">{formatUs(entry.totalUs)}</span>
          </div>
          <div class="entry-sql"><SqlHighlight sql={formatSql(entry.sql)} /></div>
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
    border-radius: var(--radius-xs);
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

  .filters {
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .filter-rows {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: var(--spacing-2) var(--spacing-4);
    padding-bottom: var(--spacing-1);
    border-bottom: 1px solid var(--color-border);
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .filter-op {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .filter-input {
    width: 68px;
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-xs);
    font-variant-numeric: tabular-nums;
  }

  .filter-input:focus {
    outline: none;
    border-color: var(--color-accent);
  }

  .filter-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    font-size: 14px;
    line-height: 1;
    color: var(--color-text-muted);
    background: transparent;
    border: none;
    border-radius: var(--radius-xs);
    cursor: pointer;
    flex-shrink: 0;
  }

  .filter-remove:hover {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  .filter-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
  }

  .toolbar-divider {
    width: 1px;
    height: 14px;
    background: var(--color-border);
    flex-shrink: 0;
    margin: 0 var(--spacing-1);
  }

  .toolbar-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    white-space: nowrap;
  }

  .sort-dir-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-xs);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      color var(--transition-fast),
      border-color var(--transition-fast),
      background var(--transition-fast);
  }

  .sort-dir-btn:hover {
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
    background: var(--color-bg-hover);
  }

  .filter-clear {
    margin-left: auto;
  }

  .stat-total {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-normal);
    color: var(--color-text-muted);
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
    border-radius: var(--radius-xs);
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
    border-radius: var(--radius-sm);
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
    border-radius: var(--radius-xs);
    font-variant-numeric: tabular-nums;
  }

  .row-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: auto;
  }

  /* Segment colours */
  .pool {
    background-color: var(--color-warning);
  }
  .switch {
    background-color: var(--color-accent);
  }
  .exec {
    background-color: var(--color-danger);
  }
  .proc {
    background-color: var(--color-success);
  }

  .legend-dot.pool {
    background-color: var(--color-warning);
  }
  .legend-dot.switch {
    background-color: var(--color-accent);
  }
  .legend-dot.exec {
    background-color: var(--color-danger);
  }
  .legend-dot.proc {
    background-color: var(--color-success);
  }

  .phase.pool {
    color: var(--color-warning);
    background: var(--color-warning-subtle);
  }
  .phase.switch {
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }
  .phase.exec {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
  }
  .phase.proc {
    color: var(--color-success);
    background: var(--color-success-subtle);
  }

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
</style>
