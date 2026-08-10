<!--
  RightSidebar — toggleable right panel with icon tabs.
  Only one panel is active at a time; clicking the active tab collapses the sidebar.
  Panels: Query History, Saved Queries, Column Inspector, Relations, JSON Viewer, Dashboards, AI Chats.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import ClockIcon from '$lib/components/icons/ClockIcon.svelte';
  import BookmarkIcon from '$lib/components/icons/BookmarkIcon.svelte';
  import SearchPlusIcon from '$lib/components/icons/SearchPlusIcon.svelte';
  import LinkIcon from '$lib/components/icons/LinkIcon.svelte';
  import JsonIcon from '$lib/components/icons/JsonIcon.svelte';
  import GridIcon from '$lib/components/icons/GridIcon.svelte';
  import ChatIcon from '$lib/components/icons/ChatIcon.svelte';
  import HistoryPanel from '$lib/components/history/HistoryPanel.svelte';
  import SavedQueriesPanel from '$lib/components/saved-queries/SavedQueriesPanel.svelte';
  import ColumnInspectorPanel from '$lib/components/schema/ColumnInspectorPanel.svelte';
  import RelationsPanel from '$lib/components/relations/RelationsPanel.svelte';
  import DashboardsPanel from '$lib/components/dashboard/DashboardsPanel.svelte';
  import JsonViewerPanel from '$lib/components/json/JsonViewerPanel.svelte';
  import AiChatPanel from '$lib/components/ai/AiChatPanel.svelte';

  type ActivePanel =
    | 'history'
    | 'saved'
    | 'column'
    | 'table-info'
    | 'relations'
    | 'json'
    | 'dashboards'
    | 'ai-chat'
    | null;

  interface Props {
    initialPanel?: ActivePanel;
    onPanelChange?: (_panel: ActivePanel) => void;
  }

  const { initialPanel = 'history', onPanelChange }: Props = $props();

  let activePanel = $state<ActivePanel>(untrack(() => initialPanel));

  function selectPanel(panel: ActivePanel) {
    activePanel = panel;
    onPanelChange?.(panel);
  }

  function handleTabStripKeydown(e: KeyboardEvent) {
    const tabs = Array.from(
      (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="tab"]'),
    );
    const idx = tabs.indexOf(document.activeElement as HTMLButtonElement);
    if (idx === -1) return;
    let next = -1;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = (idx + 1) % tabs.length;
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft')
      next = (idx - 1 + tabs.length) % tabs.length;
    if (next !== -1) {
      e.preventDefault();
      tabs[next].focus();
      tabs[next].click();
    }
  }

  $effect(() => {
    function onFocusRightSidebar() {
      const activeTab = document.querySelector<HTMLButtonElement>(
        '.right-sidebar [role="tab"][aria-selected="true"], .right-sidebar [role="tab"].active',
      );
      (
        activeTab ?? document.querySelector<HTMLButtonElement>('.right-sidebar [role="tab"]')
      )?.focus();
    }
    document.addEventListener('focus-right-sidebar', onFocusRightSidebar);
    return () => document.removeEventListener('focus-right-sidebar', onFocusRightSidebar);
  });
</script>

<div class="right-sidebar">
  <!-- Icon tab strip -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="tab-strip"
    role="tablist"
    aria-label="Right sidebar panels"
    onkeydown={handleTabStripKeydown}
  >
    <div class="spacer"></div>
    <button
      class="tab-btn"
      class:active={activePanel === 'history'}
      role="tab"
      aria-selected={activePanel === 'history'}
      aria-controls="panel-history"
      title="Query History"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('history');
      }}
    >
      <ClockIcon width={15} height={15} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'saved'}
      role="tab"
      aria-selected={activePanel === 'saved'}
      aria-controls="panel-saved"
      title="Saved Queries"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('saved');
      }}
    >
      <BookmarkIcon width={15} height={15} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'column'}
      role="tab"
      aria-selected={activePanel === 'column'}
      aria-controls="panel-column"
      title="Column Inspector"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('column');
      }}
    >
      <SearchPlusIcon width={15} height={15} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'relations'}
      role="tab"
      aria-selected={activePanel === 'relations'}
      aria-controls="panel-relations"
      title="Relations"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('relations');
      }}
    >
      <LinkIcon width={15} height={15} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'json'}
      role="tab"
      aria-selected={activePanel === 'json'}
      aria-controls="panel-json"
      title="JSON Viewer"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('json');
      }}
    >
      <JsonIcon width={15} height={15} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'dashboards'}
      role="tab"
      aria-selected={activePanel === 'dashboards'}
      aria-controls="panel-dashboards"
      title="Dashboards"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('dashboards');
      }}
    >
      <GridIcon width={13} height={13} />
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'ai-chat'}
      role="tab"
      aria-selected={activePanel === 'ai-chat'}
      aria-controls="panel-ai-chat"
      title="AI Chats"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('ai-chat');
      }}
    >
      <ChatIcon width={15} height={15} />
    </button>
  </div>

  <!-- Panel content -->
  <div class="panel-content">
    {#if activePanel === 'history'}
      <HistoryPanel />
    {:else if activePanel === 'saved'}
      <SavedQueriesPanel />
    {:else if activePanel === 'column'}
      <ColumnInspectorPanel />
    {:else if activePanel === 'relations'}
      <div id="panel-relations" role="tabpanel" aria-label="Relations" class="relations-tabpanel">
        <div class="panel-toolbar">
          <span class="panel-title">Relations</span>
        </div>
        <RelationsPanel />
      </div>
    {:else if activePanel === 'json'}
      <div id="panel-json" role="tabpanel" aria-label="JSON Viewer" class="relations-tabpanel">
        <div class="panel-toolbar">
          <span class="panel-title">JSON Viewer</span>
        </div>
        <JsonViewerPanel />
      </div>
    {:else if activePanel === 'dashboards'}
      <div id="panel-dashboards" role="tabpanel" aria-label="Dashboards" class="relations-tabpanel">
        <DashboardsPanel />
      </div>
    {:else if activePanel === 'ai-chat'}
      <div id="panel-ai-chat" role="tabpanel" aria-label="AI Chats" class="relations-tabpanel">
        <AiChatPanel />
      </div>
    {/if}
  </div>
</div>

<style>
  .right-sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .tab-strip {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 2px;
    padding: var(--spacing-1) var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .tab-btn {
    width: 30px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-md);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .tab-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
  }

  .spacer {
    flex: 1;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .panel-content > :global(div) {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .panel-title {
    font-size: 10.5px;
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .relations-tabpanel {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
</style>
