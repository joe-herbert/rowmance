<!--
  Panel — a single cell in the split-panel grid.
  Routes its PanelState.content to the appropriate component.
  Clicking anywhere in an unfocused panel focuses it.
-->
<script lang="ts">
  import type { PanelState } from '$lib/types';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useStatusBar } from '$lib/stores/statusBar.svelte';
  import QueryEditor from '$lib/components/editor/QueryEditor.svelte';
  import TableBrowser from '$lib/components/table/TableBrowser.svelte';
  import DdlViewer from '$lib/components/schema/DdlViewer.svelte';
  import TableStructure from '$lib/components/schema/TableStructure.svelte';
  import ErdCanvas from '$lib/components/erd/ErdCanvas.svelte';
  import ExplainCanvas from '$lib/components/explain/ExplainCanvas.svelte';
  import Settings from '$lib/components/settings/Settings.svelte';
  import UserManager from '$lib/components/users/UserManager.svelte';
  import ServerAdmin from '$lib/components/server-admin/ServerAdmin.svelte';
  import SpeedAnalysis from '$lib/components/speed/SpeedAnalysis.svelte';
  import ReleaseNotes from '$lib/components/release/ReleaseNotes.svelte';
  import Releases from '$lib/components/release/Releases.svelte';
  import OracleClientHelp from '$lib/components/connections/OracleClientHelp.svelte';
  import ConnectionsPage from '$lib/components/connections/ConnectionsPage.svelte';
  import DashboardPanel from '$lib/components/dashboard/DashboardPanel.svelte';
  import AiChatTabPanel from '$lib/components/ai/AiChatTabPanel.svelte';
  import EmptyPanelPlaceholder from '$lib/components/layout/EmptyPanelPlaceholder.svelte';

  interface Props {
    panel: PanelState;
    isFocused: boolean;
    splitId: string;
    itemId: string;
  }

  const { panel, isFocused, splitId, itemId }: Props = $props();
  const panelStore = usePanels();
  const statusBar = useStatusBar();

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
        initialDescription={panel.content.initialDescription}
        initialAnnotations={panel.content.initialAnnotations}
        editorId={panel.content.editorId}
        savedQueryId={panel.content.savedQueryId}
        savedQueryName={panel.content.savedQueryName}
      />
    {/key}
  {:else if panel.content.kind === 'table_browser'}
    {#key `${panel.content.connectionId}:${panel.content.instanceDb ?? ''}:${panel.content.database}:${panel.content.table}`}
      <TableBrowser
        connectionId={panel.content.connectionId}
        database={panel.content.database}
        table={panel.content.table}
        instanceDb={panel.content.instanceDb}
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
      instanceDb={panel.content.instanceDb}
      {itemId}
      {splitId}
    />
  {:else if panel.content.kind === 'ddl_viewer'}
    <DdlViewer
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      objectName={panel.content.objectName}
      objectType={panel.content.objectType}
      instanceDb={panel.content.instanceDb}
      {itemId}
      {splitId}
    />
  {:else if panel.content.kind === 'erd'}
    <ErdCanvas
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      instanceDb={panel.content.instanceDb}
    />
  {:else if panel.content.kind === 'explain'}
    <ExplainCanvas rawJson={panel.content.sql} dialect={panel.content.dialect} />
  {:else if panel.content.kind === 'settings'}
    <Settings />
  {:else if panel.content.kind === 'user_manager'}
    <UserManager connectionId={panel.content.connectionId} />
  {:else if panel.content.kind === 'server_admin'}
    <ServerAdmin connectionId={panel.content.connectionId} />
  {:else if panel.content.kind === 'speed_analysis'}
    <SpeedAnalysis />
  {:else if panel.content.kind === 'release_notes'}
    <ReleaseNotes version={panel.content.version} notes={panel.content.notes} />
  {:else if panel.content.kind === 'releases'}
    <Releases />
  {:else if panel.content.kind === 'oracle_client_help'}
    <OracleClientHelp />
  {:else if panel.content.kind === 'connections'}
    <ConnectionsPage />
  {:else if panel.content.kind === 'dashboard'}
    {#key panel.content.dashboardId}
      <DashboardPanel dashboardId={panel.content.dashboardId} />
    {/key}
  {:else if panel.content.kind === 'ai_chat'}
    {#key panel.content.conversationId}
      <AiChatTabPanel conversationId={panel.content.conversationId} />
    {/key}
  {:else}
    <EmptyPanelPlaceholder />
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

</style>
