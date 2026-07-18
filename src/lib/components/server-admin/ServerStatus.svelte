<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import type { ServerStatus } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props { connectionId: string; }
  const { connectionId }: Props = $props();

  let status = $state<ServerStatus | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let lastUpdated = $state<Date | null>(null);
  let timer: ReturnType<typeof setInterval> | null = null;

  async function load() {
    loading = true;
    error = null;
    try {
      status = await api.getStatus(connectionId);
      lastUpdated = new Date();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(() => {
    load();
    timer = setInterval(load, 30_000);
  });
  onDestroy(() => { if (timer) clearInterval(timer); });

  function formatUptime(secs: number): string {
    if (secs === 0) return '—';
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    if (d > 0) return `${d}d ${h}h ${m}m`;
    if (h > 0) return `${h}h ${m}m ${s}s`;
    return `${m}m ${s}s`;
  }
</script>

<div class="server-status">
  <div class="toolbar">
    <button class="icon-btn" onclick={load} title="Refresh" disabled={loading}>
      <RefreshIcon />
    </button>
    <span class="spacer"></span>
    {#if lastUpdated}
      <span class="last-updated">Updated {lastUpdated.toLocaleTimeString()}</span>
    {/if}
    {#if loading}<Spinner size={12} />{/if}
  </div>

  {#if error}
    <div class="error-msg">{error}</div>
  {:else if !status}
    <div class="loading-msg"><Spinner size={20} /></div>
  {:else}
    <div class="metrics-grid">
      <div class="metric-card">
        <div class="metric-label">Version</div>
        <div class="metric-value version-value">{status.version}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Uptime</div>
        <div class="metric-value">{formatUptime(status.uptimeSeconds)}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Connections</div>
        <div class="metric-value">
          {status.connectionsCurrent}
          {#if status.connectionsMax}
            <span class="metric-sub">/ {status.connectionsMax}</span>
          {/if}
        </div>
      </div>
      {#if status.queriesPerSecond !== null}
        <div class="metric-card">
          <div class="metric-label">Queries/sec</div>
          <div class="metric-value">{status.queriesPerSecond.toFixed(1)}</div>
        </div>
      {/if}
      {#if status.cacheHitRatio !== null}
        <div class="metric-card">
          <div class="metric-label">Cache Hit</div>
          <div class="metric-value">{status.cacheHitRatio.toFixed(1)}%</div>
        </div>
      {/if}
      {#each Object.entries(status.extra) as [k, v]}
        <div class="metric-card">
          <div class="metric-label">{k}</div>
          <div class="metric-value">{v}</div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .server-status { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
  .toolbar {
    display: flex; align-items: center; gap: 8px; padding: 8px 12px;
    border-bottom: 1px solid var(--color-border); flex-shrink: 0;
  }
  .spacer { flex: 1; }
  .last-updated { font-size: var(--font-size-xs); color: var(--color-text-muted); }
  .icon-btn {
    display: grid; place-items: center; width: 26px; height: 26px;
    border-radius: var(--radius-md); background: transparent; color: var(--color-text-muted);
    cursor: pointer; transition: background var(--transition-fast), color var(--transition-fast);
  }
  .icon-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }
  .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .error-msg, .loading-msg {
    flex: 1; display: flex; align-items: center; justify-content: center;
    color: var(--color-text-muted); font-size: var(--font-size-sm);
  }
  .error-msg { color: var(--color-danger, #e53e3e); }

  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: 12px;
    padding: 16px;
    overflow-y: auto;
  }

  .metric-card {
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    padding: 14px 16px;
  }

  .metric-label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--color-text-muted);
    text-transform: uppercase;
    margin-bottom: 6px;
  }

  .metric-value {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    word-break: break-all;
  }

  .version-value {
    font-size: var(--font-size-sm);
    font-family: var(--font-family-mono, monospace);
  }

  .metric-sub {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-weight: normal;
  }
</style>
