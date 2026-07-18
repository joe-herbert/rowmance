<script lang="ts">
  import { onMount } from 'svelte';
  import type { ScheduledJob } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props { connectionId: string; }
  const { connectionId }: Props = $props();

  let jobs = $state<ScheduledJob[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      jobs = await api.listScheduledJobs(connectionId);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<div class="scheduled-jobs">
  <div class="toolbar">
    <button class="icon-btn" onclick={load} title="Refresh" disabled={loading}>
      <RefreshIcon />
    </button>
    {#if loading}<Spinner size={12} />{/if}
  </div>

  {#if error}
    <div class="state-msg error">{error}</div>
  {:else if jobs.length === 0 && !loading}
    <div class="state-msg">No scheduled jobs found.</div>
  {:else}
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Schedule</th>
            <th>Enabled</th>
            <th>Last Run</th>
            <th>Next Run</th>
          </tr>
        </thead>
        <tbody>
          {#each jobs as job (job.id)}
            <tr>
              <td class="name-cell">{job.name}</td>
              <td class="mono">{job.schedule}</td>
              <td>
                <span class="enabled-badge" class:enabled={job.enabled}>
                  {job.enabled ? 'Enabled' : 'Disabled'}
                </span>
              </td>
              <td>{job.lastRun ?? '—'}</td>
              <td>{job.nextRun ?? '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
  .scheduled-jobs { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
  .toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
  .icon-btn { display: grid; place-items: center; width: 26px; height: 26px; border-radius: var(--radius-md); background: transparent; color: var(--color-text-muted); cursor: pointer; transition: background var(--transition-fast), color var(--transition-fast); }
  .icon-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }
  .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .state-msg { flex: 1; display: flex; align-items: center; justify-content: center; color: var(--color-text-muted); font-size: var(--font-size-sm); padding: 24px; }
  .state-msg.error { color: var(--color-danger, #e53e3e); }
  .table-wrap { flex: 1; overflow: auto; }
  .data-table { width: 100%; border-collapse: collapse; font-size: var(--font-size-xs); }
  .data-table th, .data-table td { padding: 6px 10px; text-align: left; border-bottom: 1px solid var(--color-border); white-space: nowrap; }
  .data-table th { font-size: 11px; font-weight: 600; letter-spacing: 0.04em; color: var(--color-text-muted); background: var(--color-bg-secondary); position: sticky; top: 0; z-index: 1; }
  .data-table tbody tr:hover { background: var(--color-bg-hover); }
  .mono { font-family: var(--font-family-mono, monospace); }
  .name-cell { font-weight: var(--font-weight-medium); color: var(--color-text-primary); }
  .enabled-badge { padding: 1px 7px; border-radius: 99px; font-size: 10px; font-weight: 600; }
  .enabled-badge.enabled { background: color-mix(in srgb, var(--color-success, #22c55e) 12%, transparent); color: var(--color-success, #22c55e); }
  .enabled-badge:not(.enabled) { background: var(--color-bg-secondary); color: var(--color-text-disabled); }
</style>
