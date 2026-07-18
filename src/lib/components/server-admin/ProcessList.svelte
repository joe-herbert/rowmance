<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { ProcessInfo } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props {
    connectionId: string;
    canKill: boolean;
    canCancel: boolean;
  }
  const { connectionId, canKill, canCancel }: Props = $props();

  const toast = useToast();

  let processes = $state<ProcessInfo[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let lastUpdated = $state<Date | null>(null);
  let paused = $state(false);
  let refreshInterval = $state('10');
  let timer: ReturnType<typeof setInterval> | null = null;

  // Confirm dialog state
  let confirmKill = $state<{ process: ProcessInfo; mode: 'kill' | 'cancel' } | null>(null);

  const intervalOptions = [
    { value: '5', label: '5s' },
    { value: '10', label: '10s' },
    { value: '30', label: '30s' },
    { value: '60', label: '60s' },
  ];

  async function load() {
    if (loading) return;
    loading = true;
    error = null;
    try {
      processes = await api.listProcesses(connectionId);
      lastUpdated = new Date();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  function startTimer() {
    stopTimer();
    if (!paused) {
      const ms = parseInt(refreshInterval) * 1000;
      timer = setInterval(load, ms);
    }
  }

  function stopTimer() {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }

  function togglePause() {
    paused = !paused;
    if (paused) stopTimer();
    else {
      load();
      startTimer();
    }
  }

  $effect(() => {
    // restart timer when interval changes
    refreshInterval;
    if (!paused) startTimer();
  });

  onMount(() => {
    load();
    startTimer();
  });

  onDestroy(stopTimer);

  async function doKill(process: ProcessInfo, mode: 'kill' | 'cancel') {
    try {
      if (mode === 'kill') {
        await api.killSession(connectionId, process.id);
        toast.addToast(`Session ${process.id} killed`, 'success', 3000);
      } else {
        await api.cancelSession(connectionId, process.id);
        toast.addToast(`Query ${process.id} cancelled`, 'success', 3000);
      }
      await load();
    } catch (e) {
      toast.addToast(errorMessage(e), 'error', 0);
    }
  }

  function formatTime(seconds: number | null): string {
    if (seconds === null) return '—';
    if (seconds < 60) return `${seconds}s`;
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  }
</script>

<div class="process-list">
  <div class="toolbar">
    <button class="icon-btn" onclick={load} title="Refresh now" disabled={loading}>
      <RefreshIcon />
    </button>
    <button class="text-btn" onclick={togglePause}>
      {paused ? 'Resume' : 'Pause'}
    </button>
    <label class="interval-label">
      Refresh:
      <Select
        bind:value={refreshInterval}
        options={intervalOptions}
        size="xs"
        onchange={(v) => (refreshInterval = v)}
      />
    </label>
    <span class="spacer"></span>
    {#if lastUpdated}
      <span class="last-updated">Updated {lastUpdated.toLocaleTimeString()}</span>
    {/if}
    {#if loading}
      <Spinner size={12} />
    {/if}
  </div>

  {#if error}
    <div class="error-msg">{error}</div>
  {:else if processes.length === 0 && !loading}
    <div class="empty-msg">No active processes.</div>
  {:else}
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>ID</th>
            <th>User</th>
            <th>Host</th>
            <th>Database</th>
            <th>Command</th>
            <th>Time</th>
            <th>State</th>
            <th class="query-col">Query</th>
            {#if canKill || canCancel}
              <th class="actions-col">Actions</th>
            {/if}
          </tr>
        </thead>
        <tbody>
          {#each processes as proc (proc.id)}
            <tr>
              <td class="mono">{proc.id}</td>
              <td>{proc.user ?? '—'}</td>
              <td>{proc.host ?? '—'}</td>
              <td>{proc.database ?? '—'}</td>
              <td>{proc.command ?? '—'}</td>
              <td class="mono">{formatTime(proc.timeSeconds)}</td>
              <td>{proc.state ?? '—'}</td>
              <td class="query-cell" title={proc.info ?? ''}>{proc.info ?? '—'}</td>
              {#if canKill || canCancel}
                <td class="actions-cell">
                  {#if canCancel && proc.canCancel}
                    <button
                      class="action-btn cancel-btn"
                      onclick={() => (confirmKill = { process: proc, mode: 'cancel' })}
                      title="Cancel query"
                    >
                      Cancel
                    </button>
                  {/if}
                  {#if canKill && proc.canKill}
                    <button
                      class="action-btn kill-btn"
                      onclick={() => (confirmKill = { process: proc, mode: 'kill' })}
                      title="Kill session"
                    >
                      Kill
                    </button>
                  {/if}
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if confirmKill}
  <ConfirmDialog
    title={confirmKill.mode === 'kill' ? 'Kill Session' : 'Cancel Query'}
    message={confirmKill.mode === 'kill'
      ? `Kill session ${confirmKill.process.id} (user: ${confirmKill.process.user ?? 'unknown'})? This terminates the connection immediately.`
      : `Cancel the query running in session ${confirmKill.process.id}? The connection itself stays open.`}
    confirmText={confirmKill.mode === 'kill' ? 'Kill Session' : 'Cancel Query'}
    cancelText="Back"
    danger={true}
    onconfirm={() => {
      const c = confirmKill!;
      confirmKill = null;
      doKill(c.process, c.mode);
    }}
    oncancel={() => (confirmKill = null)}
  />
{/if}

<style>
  .process-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .spacer { flex: 1; }

  .last-updated {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .icon-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }
  .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .text-btn {
    padding: 3px 10px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    cursor: pointer;
    border: 1px solid var(--color-border);
    transition: background var(--transition-fast);
  }
  .text-btn:hover { background: var(--color-bg-hover); }

  .interval-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .error-msg, .empty-msg {
    padding: 24px;
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .error-msg { color: var(--color-danger, #e53e3e); }

  .table-wrap {
    flex: 1;
    overflow: auto;
  }

  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .data-table th, .data-table td {
    padding: 6px 10px;
    text-align: left;
    border-bottom: 1px solid var(--color-border);
    white-space: nowrap;
  }

  .data-table th {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
    background: var(--color-bg-secondary);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .data-table tbody tr:hover { background: var(--color-bg-hover); }

  .mono { font-family: var(--font-family-mono, monospace); }

  .query-col { width: 40%; }
  .query-cell {
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono, monospace);
  }

  .actions-col { width: 100px; }
  .actions-cell { display: flex; gap: 4px; }

  .action-btn {
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    font-size: 11px;
    cursor: pointer;
    border: 1px solid;
    transition: background var(--transition-fast);
  }

  .kill-btn {
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 12%, transparent);
    border-color: color-mix(in srgb, var(--color-danger, #e53e3e) 30%, transparent);
    color: var(--color-danger, #e53e3e);
  }
  .kill-btn:hover {
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 20%, transparent);
  }

  .cancel-btn {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 12%, transparent);
    border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 30%, transparent);
    color: var(--color-warning, #f59e0b);
  }
  .cancel-btn:hover {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 20%, transparent);
  }
</style>
