<script lang="ts">
  import { onMount } from 'svelte';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';

  interface Props { connectionId: string; }
  const { connectionId }: Props = $props();

  let statusText = $state('');
  let loading = $state(false);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      statusText = await api.getInnodbStatus(connectionId);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);
</script>

<div class="innodb-status">
  <div class="toolbar">
    <button class="icon-btn" onclick={load} disabled={loading} title="Refresh">
      <RefreshIcon />
    </button>
    {#if loading}<Spinner size={12} />{/if}
  </div>
  {#if error}
    <div class="error">{error}</div>
  {:else}
    <pre class="status-text">{statusText}</pre>
  {/if}
</div>

<style>
  .innodb-status { display: flex; flex-direction: column; height: 100%; overflow: hidden; }
  .toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--color-border); flex-shrink: 0; }
  .icon-btn { display: grid; place-items: center; width: 26px; height: 26px; border-radius: var(--radius-md); background: transparent; color: var(--color-text-muted); cursor: pointer; }
  .icon-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }
  .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .error { padding: 16px; color: var(--color-danger, #e53e3e); font-size: var(--font-size-sm); }
  .status-text { flex: 1; overflow: auto; padding: 12px 16px; font-family: var(--font-family-mono, monospace); font-size: 12px; line-height: 1.5; color: var(--color-text-secondary); white-space: pre; margin: 0; }
</style>
