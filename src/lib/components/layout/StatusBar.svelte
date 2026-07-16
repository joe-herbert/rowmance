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
  const connType = $derived(focusedConnection?.dialectInfo.displayName ?? null);
  const host = $derived(
    focusedConnection ? `${focusedConnection.host}:${focusedConnection.port}` : null,
  );

  const timingText = $derived(
    statusBar.lastQueryMs !== null
      ? statusBar.lastQueryMs < 1000
        ? `${statusBar.lastQueryMs}ms`
        : `${(statusBar.lastQueryMs / 1000).toFixed(1)}s`
      : null,
  );

  const rowCountText = $derived.by(() => {
    if (statusBar.rowCount === null) return null;
    const filtered = statusBar.rowCount.toLocaleString();
    if (statusBar.totalRowCount !== null && statusBar.totalRowCount !== statusBar.rowCount) {
      return `${filtered} of ${statusBar.totalRowCount.toLocaleString()} rows`;
    }
    return `${filtered} rows`;
  });

  const dirtyText = $derived.by(() => {
    const cells = statusBar.pendingCellCount;
    const rows = statusBar.pendingRowCount;
    if (cells === 0) return null;
    const rowSuffix = cells !== 1 || rows !== 1 ? ` in ${rows} row${rows !== 1 ? 's' : ''}` : '';
    return `${cells} unsaved change${cells !== 1 ? 's' : ''}${rowSuffix}`;
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
      <span class="item hide-xs">{connType}</span>
    {/if}
    {#if host}
      <span class="item faint hide-sm">{host}</span>
    {/if}
  </div>

  <div class="spacer"></div>

  <!-- Right: stats -->
  <div class="right">
    {#if dirtyText}
      <span class="item dirty">{dirtyText}</span>
    {/if}

    {#if rowCountText}
      <span class="item hide-xs">{rowCountText}</span>
    {/if}

    <span class="item faint hide-sm">UTF-8</span>

    {#if timingText}
      <span class="item accent">{timingText}</span>
    {/if}
  </div>
</div>

<style>
  .status-bar {
    container-type: inline-size;
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
    overflow: hidden;
    white-space: nowrap;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    overflow: hidden;
    flex-shrink: 1;
  }

  .right {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }

  .spacer {
    flex: 1;
    min-width: 0;
  }

  .conn-status {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    overflow: hidden;
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
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item {
    color: var(--color-text-muted);
    flex-shrink: 0;
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

  @container (max-width: 500px) {
    .hide-sm {
      display: none;
    }
  }

  @container (max-width: 320px) {
    .hide-xs {
      display: none;
    }
  }
</style>
