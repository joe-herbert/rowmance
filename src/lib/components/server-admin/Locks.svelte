<script lang="ts">
  import { onMount } from 'svelte';
  import type { LockInfo } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props {
    connectionId: string;
    canKill: boolean;
  }
  const { connectionId, canKill }: Props = $props();

  const toast = useToast();

  let locks = $state<LockInfo[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let lastUpdated = $state<Date | null>(null);
  let confirmKillBlocker = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      locks = await api.listLocks(connectionId);
      lastUpdated = new Date();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function killBlocker(sessionId: string) {
    try {
      await api.killSession(connectionId, sessionId);
      toast.addToast(`Session ${sessionId} killed`, 'success', 3000);
      await load();
    } catch (e) {
      toast.addToast(errorMessage(e), 'error', 0);
    }
  }
</script>

<div class="locks">
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
    <div class="state-msg error">{error}</div>
  {:else if locks.length === 0 && !loading}
    <div class="state-msg">No blocking locks detected.</div>
  {:else}
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>Type</th>
            <th>Mode</th>
            <th>Object</th>
            <th>Blocker</th>
            <th>Waiting</th>
            <th>Duration</th>
            {#if canKill}<th></th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each locks as lock (lock.lockId)}
            <tr>
              <td>{lock.lockType}</td>
              <td><span class="mono">{lock.lockMode}</span></td>
              <td>{lock.objectName ?? '—'}</td>
              <td class="mono">{lock.blockerSessionId ?? '—'}</td>
              <td class="mono">{lock.waitingSessionId ?? '—'}</td>
              <td>{lock.durationMs !== null ? `${lock.durationMs}ms` : '—'}</td>
              {#if canKill}
                <td>
                  {#if lock.blockerSessionId}
                    <button
                      class="kill-btn"
                      onclick={() => (confirmKillBlocker = lock.blockerSessionId)}
                    >
                      Kill Blocker
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

{#if confirmKillBlocker}
  <ConfirmDialog
    title="Kill Blocking Session"
    message="Kill session {confirmKillBlocker}? This terminates the connection that is holding the lock."
    confirmText="Kill Session"
    cancelText="Cancel"
    danger={true}
    onconfirm={() => {
      const id = confirmKillBlocker!;
      confirmKillBlocker = null;
      killBlocker(id);
    }}
    oncancel={() => (confirmKillBlocker = null)}
  />
{/if}

<style>
  .locks {
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
  .spacer {
    flex: 1;
  }
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }
  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .state-msg {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    padding: 24px;
  }
  .state-msg.error {
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
  .kill-btn {
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    font-size: 11px;
    cursor: pointer;
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-danger, #e53e3e) 30%, transparent);
    color: var(--color-danger, #e53e3e);
    transition: background var(--transition-fast);
  }
  .kill-btn:hover {
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 20%, transparent);
  }
</style>
