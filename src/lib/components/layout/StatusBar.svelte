<!--
  StatusBar — fixed footer bar showing connection context, query timing,
  and save/discard actions when there are pending table edits.
-->
<script lang="ts">
  import { useStatusBar } from '$lib/stores/statusBar.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';

  const statusBar = useStatusBar();
  const connectionStore = useConnections();
  const panelStore = usePanels();

  const focusedConnection = $derived.by(() => {
    const content = panelStore.focusedPanel?.content;
    if (!content || !('connectionId' in content)) return null;
    return connectionStore.getById(content.connectionId) ?? null;
  });

  const connColor = $derived(focusedConnection?.color ?? 'var(--color-accent)');
  const connName = $derived(focusedConnection?.name ?? null);
  const connType = $derived(
    focusedConnection
      ? focusedConnection.dbType === 'postgres'
        ? 'PostgreSQL'
        : focusedConnection.dbType === 'mysql'
        ? 'MySQL'
        : 'MariaDB'
      : null
  );
  const host = $derived(
    focusedConnection ? `${focusedConnection.host}:${focusedConnection.port}` : null
  );

  const timingText = $derived(
    statusBar.lastQueryMs !== null
      ? statusBar.lastQueryMs < 1000
        ? `${statusBar.lastQueryMs}ms`
        : `${(statusBar.lastQueryMs / 1000).toFixed(1)}s`
      : null
  );

  const rowCountText = $derived(
    statusBar.rowCount !== null ? `${statusBar.rowCount.toLocaleString()} rows` : null
  );

  const dirtyText = $derived.by(() => {
    const cells = statusBar.pendingCellCount;
    const rows = statusBar.pendingRowCount;
    if (cells === 0) return null;
    return `${cells} unsaved change${cells !== 1 ? 's' : ''} in ${rows} row${rows !== 1 ? 's' : ''}`;
  });
</script>

<div class="status-bar" role="status" aria-label="Status bar">
  <!-- Left: connection info -->
  <div class="left">
    {#if connName}
      <span class="conn-status">
        <span class="conn-dot" style="background: {connColor};" aria-hidden="true"></span>
        <span class="conn-name">{connName}</span>
      </span>
    {/if}
    {#if connType}
      <span class="item">{connType}</span>
    {/if}
    {#if host}
      <span class="item faint">{host}</span>
    {/if}
  </div>

  <div class="spacer"></div>

  <!-- Right: stats -->
  {#if dirtyText}
    <span class="item dirty">{dirtyText}</span>
  {/if}

  {#if rowCountText}
    <span class="item">{rowCountText}</span>
  {/if}

  <span class="item faint">UTF-8</span>

  {#if timingText}
    <span class="item accent">{timingText}</span>
  {/if}
</div>

<style>
  .status-bar {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: var(--statusbar-height);
    padding: 0 14px;
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--panel-radius);
    font-size: 11px;
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    opacity: var(--panel-opacity);
  }

  .left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .spacer {
    flex: 1;
  }

  .conn-status {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .conn-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .conn-name {
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .item {
    color: var(--color-text-muted);
  }

  .faint {
    color: var(--color-text-faint, var(--color-text-muted));
    opacity: 0.7;
  }

  .dirty {
    color: var(--color-accent);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-xs);
  }

  .accent {
    color: var(--color-accent);
  }

</style>
