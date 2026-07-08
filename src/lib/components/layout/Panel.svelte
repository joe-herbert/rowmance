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
  import UserManager from '$lib/components/users/UserManager.svelte';
  import SpeedAnalysis from '$lib/components/speed/SpeedAnalysis.svelte';
  import ReleaseNotes from '$lib/components/release/ReleaseNotes.svelte';

  interface Props {
    panel: PanelState;
    isFocused: boolean;
    splitId: string;
    itemId: string;
  }

  const { panel, isFocused, splitId, itemId }: Props = $props();
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

<div
  class="panel"
  class:focused={isFocused}
  role="region"
  aria-label={panelStore.getSplitLabel(splitId)}
>
  {#if panel.content.kind === 'query_editor'}
    {#key panel.content.editorId}
      <QueryEditor
        connectionId={panel.content.connectionId}
        database={panel.content.database}
        initialSql={panel.content.initialSql}
        editorId={panel.content.editorId}
        savedQueryId={panel.content.savedQueryId}
        savedQueryName={panel.content.savedQueryName}
      />
    {/key}
  {:else if panel.content.kind === 'table_browser'}
    {#key `${panel.content.connectionId}:${panel.content.database}:${panel.content.table}`}
      <TableBrowser
        connectionId={panel.content.connectionId}
        database={panel.content.database}
        table={panel.content.table}
        initialFilter={panel.content.initialFilter}
        {isFocused}
        {itemId}
        {splitId}
      />
    {/key}
  {:else if panel.content.kind === 'table_structure'}
    <TableStructure
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      table={panel.content.table}
      {itemId}
      {splitId}
    />
  {:else if panel.content.kind === 'ddl_viewer'}
    <DdlViewer
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      objectName={panel.content.objectName}
      objectType={panel.content.objectType}
      {itemId}
      {splitId}
    />
  {:else if panel.content.kind === 'erd'}
    <ErdCanvas connectionId={panel.content.connectionId} database={panel.content.database} />
  {:else if panel.content.kind === 'explain'}
    <ExplainCanvas rawJson={panel.content.sql} dialect={panel.content.dialect} />
  {:else if panel.content.kind === 'settings'}
    <Settings />
  {:else if panel.content.kind === 'user_manager'}
    <UserManager connectionId={panel.content.connectionId} />
  {:else if panel.content.kind === 'speed_analysis'}
    <SpeedAnalysis />
  {:else if panel.content.kind === 'release_notes'}
    <ReleaseNotes version={panel.content.version} notes={panel.content.notes} />
  {:else}
    <!-- Empty panel placeholder -->
    <div class="empty-panel">
      {#if !hasConnections}
        <div class="empty-panel-hint">
          <div class="empty-panel-icon" aria-hidden="true">
            <svg
              width="48"
              height="48"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.2"
              stroke-linecap="round"
              stroke-linejoin="round"
              ><ellipse cx="12" cy="5" rx="9" ry="3" /><path
                d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"
              /><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" /></svg
            >
          </div>
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
