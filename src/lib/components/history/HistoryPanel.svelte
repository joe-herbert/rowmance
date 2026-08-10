<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import * as historyApi from '$lib/tauri/history';
  import type { QueryHistoryEntry } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const toast = useToast();

  let historyEntries = $state<QueryHistoryEntry[]>([]);
  let historyLoading = $state(false);
  let selectedHistoryConnectionId = $state<string>('');
  let historySourceFilter = $state<'all' | 'user' | 'system'>('all');

  const filteredHistoryEntries = $derived(
    historySourceFilter === 'all'
      ? historyEntries
      : historyEntries.filter((entry) => entry.source === historySourceFilter),
  );

  const activeProfiles = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );

  $effect(() => {
    // When active connections change, default to the first one.
    const ids = activeProfiles.map((p) => p.id);
    if (ids.length > 0 && !ids.includes(selectedHistoryConnectionId)) {
      selectedHistoryConnectionId = ids[0];
    }
  });

  async function loadHistory() {
    if (!selectedHistoryConnectionId) return;
    historyLoading = true;
    try {
      historyEntries = await historyApi.listHistory(selectedHistoryConnectionId, 100, 0);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      historyLoading = false;
    }
  }

  async function handleClearHistory() {
    if (!selectedHistoryConnectionId) return;
    await historyApi.clearHistory(selectedHistoryConnectionId);
    historyEntries = [];
  }

  function openHistoryEntry(entry: QueryHistoryEntry) {
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: entry.connectionId,
      initialSql: entry.sql,
    });
  }

  function formatDuration(us: number | null): string {
    if (us === null) return '';
    if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(1)}s`;
    if (us >= 1_000) return `${(us / 1_000).toFixed(1)}ms`;
    return `${us}µs`;
  }

  function timeAgo(isoString: string): string {
    const diff = Date.now() - new Date(isoString).getTime();
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  }

  function truncateSql(sql: string, max = 80): string {
    const cleaned = sql.replace(/\s+/g, ' ').trim();
    return cleaned.length > max ? cleaned.slice(0, max) + '…' : cleaned;
  }

  // Reload history when the connection selector changes.
  $effect(() => {
    if (selectedHistoryConnectionId) {
      loadHistory();
    }
  });
</script>

<div id="panel-history" role="tabpanel" aria-label="Query History">
  <div class="panel-toolbar">
    {#if activeProfiles.length > 1}
      <Select
        bind:value={selectedHistoryConnectionId}
        options={activeProfiles.map((p) => ({ value: p.id, label: p.name }))}
        aria-label="Select connection"
        size="xs"
        searchable
      />
    {:else if activeProfiles.length === 1}
      <span class="connection-label">{activeProfiles[0].name}</span>
    {:else}
      <span class="muted">No active connections</span>
    {/if}

    <div class="toolbar-gap"></div>

    <SegmentedControl
      options={[
        { value: 'all', label: 'All' },
        { value: 'user', label: 'User' },
        { value: 'system', label: 'System' },
      ]}
      value={historySourceFilter}
      onchange={(v) => (historySourceFilter = v as 'all' | 'user' | 'system')}
    />

    {#if historyEntries.length > 0}
      <button class="action-btn danger-btn" onclick={handleClearHistory} title="Clear history">
        Clear
      </button>
    {/if}
  </div>

  {#if historyLoading}
    <div class="loading-row">Loading…</div>
  {:else if filteredHistoryEntries.length === 0}
    <div class="empty-row">
      {historyEntries.length === 0 ? 'No history yet.' : 'No matching history entries.'}
    </div>
  {:else}
    <ul class="history-list" role="list">
      {#each filteredHistoryEntries as entry (entry.id)}
        <li class="history-item" role="listitem">
          <button class="history-btn" onclick={() => openHistoryEntry(entry)} title="Open in editor">
            <span class="history-sql">{truncateSql(entry.sql)}</span>
            <span class="history-meta">
              <span
                class="status-badge"
                class:success={entry.status === 'success'}
                class:error={entry.status === 'error'}
              >
                {entry.status}
              </span>
              <span class="meta-item source-badge">{entry.source}</span>
              {#if entry.durationUs !== null}
                <span class="meta-item">{formatDuration(entry.durationUs)}</span>
              {/if}
              <span class="meta-item time-ago">{timeAgo(entry.executedAt)}</span>
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .toolbar-gap {
    flex: 1;
  }

  .connection-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .action-btn {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
    white-space: nowrap;
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .danger-btn {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .danger-btn:hover {
    background: var(--color-danger-subtle);
  }

  .muted {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .loading-row,
  .empty-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .history-item {
    border-bottom: 1px solid var(--color-border);
  }

  .history-btn {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--spacing-2) var(--spacing-3);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .history-btn:hover {
    background: var(--color-bg-hover);
  }

  .history-sql {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .history-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .status-badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .status-badge.success {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .status-badge.error {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .meta-item {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .time-ago {
    margin-left: auto;
  }
</style>
