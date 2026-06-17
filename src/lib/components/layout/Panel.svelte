<!--
  Panel — a single cell in the split-panel grid.
  Routes its PanelState.content to the appropriate component.
  Clicking anywhere in an unfocused panel focuses it.
-->
<script lang="ts">
  import type { PanelState } from '$lib/types';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useStatusBar } from '$lib/stores/statusBar.svelte';
  import QueryEditor from '$lib/components/editor/QueryEditor.svelte';
  import TableBrowser from '$lib/components/table/TableBrowser.svelte';
  import DdlViewer from '$lib/components/schema/DdlViewer.svelte';
  import TableStructure from '$lib/components/schema/TableStructure.svelte';
  import ErdCanvas from '$lib/components/erd/ErdCanvas.svelte';
  import ExplainCanvas from '$lib/components/explain/ExplainCanvas.svelte';
  import Settings from '$lib/components/settings/Settings.svelte';

  interface Props {
    index: number;
    panel: PanelState;
    isFocused: boolean;
  }

  const { index, panel, isFocused }: Props = $props();
  const panelStore = usePanels();
  const connectionStore = useConnections();
  const statusBar = useStatusBar();

  const hasConnections = $derived(connectionStore.profiles.length > 0);

  // When a non-table panel gains focus, clear any stale status bar state.
  $effect(() => {
    if (isFocused && panel.content.kind !== 'table_browser') {
      statusBar.clear();
    }
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="panel"
  class:focused={isFocused}
  role="region"
  aria-label="Panel {index + 1}"
  onclick={() => !isFocused && panelStore.focus(index)}
>
  {#if panel.content.kind === 'query_editor'}
    {#key panel.content}
      <QueryEditor connectionId={panel.content.connectionId} initialSql={panel.content.initialSql} />
    {/key}
  {:else if panel.content.kind === 'table_browser'}
    {#key panel.content}
      <TableBrowser
        connectionId={panel.content.connectionId}
        database={panel.content.database}
        table={panel.content.table}
        initialFilter={panel.content.initialFilter}
        {isFocused}
      />
    {/key}
  {:else if panel.content.kind === 'table_structure'}
    <TableStructure
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      table={panel.content.table}
    />
  {:else if panel.content.kind === 'ddl_viewer'}
    <DdlViewer
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      objectName={panel.content.objectName}
      objectType={panel.content.objectType}
    />
  {:else if panel.content.kind === 'erd'}
    <ErdCanvas
      connectionId={panel.content.connectionId}
      database={panel.content.database}
    />
  {:else if panel.content.kind === 'explain'}
    <ExplainCanvas
      rawJson={panel.content.sql}
      dialect={panel.content.dialect}
    />
  {:else if panel.content.kind === 'settings'}
    <Settings />
  {:else}
    <!-- Empty panel placeholder -->
    <div class="empty-panel">
      {#if !hasConnections}
        <div class="empty-panel-hint">
          <div class="empty-panel-icon" aria-hidden="true">🗄</div>
          <p class="empty-panel-title">No database connections</p>
          <p class="empty-panel-subtitle">Add a connection in the left sidebar to get started</p>
        </div>
      {:else}
        <div class="empty-panel-hint">
          <p>Open a table from the sidebar, or start a new query.</p>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    height: 100%;
    min-width: 0;
    position: relative;
  }

  .panel.focused {
    outline: none;
  }

  .panel:not(.focused)::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: rgba(0, 0, 0, 0.02);
  }

  .empty-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
  }

  .empty-panel-hint {
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    max-width: 260px;
    line-height: var(--line-height-normal);
  }

  .empty-panel-icon {
    font-size: 40px;
    margin-bottom: var(--spacing-3);
    line-height: 1;
    opacity: 0.5;
  }

  .empty-panel-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    margin: 0 0 var(--spacing-2);
  }

  .empty-panel-subtitle {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
    line-height: var(--line-height-normal);
  }
</style>
