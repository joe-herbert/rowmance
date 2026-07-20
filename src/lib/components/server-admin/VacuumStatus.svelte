<script lang="ts">
  import { onMount } from 'svelte';
  import type { VacuumInfo } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props {
    connectionId: string;
  }
  const { connectionId }: Props = $props();

  let vacuumData = $state<VacuumInfo[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      vacuumData = await api.getVacuumStatus(connectionId);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<div class="vacuum-status">
  <div class="toolbar">
    <button class="icon-btn" onclick={load} disabled={loading} title="Refresh">
      <RefreshIcon />
    </button>
    {#if loading}<Spinner size={12} />{/if}
  </div>
  {#if error}
    <div class="error">{error}</div>
  {:else if vacuumData.length === 0 && !loading}
    <div class="empty">No vacuum data available.</div>
  {:else}
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>Table</th>
            <th>Dead Tuples</th>
            <th>Live Tuples</th>
            <th>Last Vacuum</th>
            <th>Last Auto-Vacuum</th>
          </tr>
        </thead>
        <tbody>
          {#each vacuumData as row (row.table)}
            <tr>
              <td class="mono">{row.table}</td>
              <td class:high-dead={row.deadTuples > 10000}>{row.deadTuples.toLocaleString()}</td>
              <td>{row.liveTuples.toLocaleString()}</td>
              <td>{row.lastVacuum ?? '—'}</td>
              <td>{row.lastAutoVacuum ?? '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .vacuum-status {
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
  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .error,
  .empty {
    padding: 24px;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    text-align: center;
  }
  .error {
    color: var(--color-danger, #e53e3e);
  }
  .table-wrap {
    flex: 1;
    overflow: auto;
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }
  .data-table th,
  .data-table td {
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
  .data-table tbody tr:hover {
    background: var(--color-bg-hover);
  }
  .mono {
    font-family: var(--font-family-mono, monospace);
  }
  .high-dead {
    color: var(--color-danger, #e53e3e);
    font-weight: var(--font-weight-medium);
  }
</style>
