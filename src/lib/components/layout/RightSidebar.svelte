<!--
  RightSidebar — toggleable right panel with icon tabs.
  Only one panel is active at a time; clicking the active tab collapses the sidebar.
  Available panels: Column Inspector, Query History, Saved Queries, Table Info.
-->
<script lang="ts">
  type ActivePanel = 'history' | 'saved' | null;

  interface Props {
    onClose: () => void;
  }

  const { onClose }: Props = $props();

  let activePanel = $state<ActivePanel>('history');

  function selectPanel(panel: ActivePanel) {
    // Clicking the already-active tab closes the sidebar.
    if (activePanel === panel) {
      onClose();
    } else {
      activePanel = panel;
    }
  }
</script>

<div class="right-sidebar">
  <!-- Icon tab strip -->
  <div class="tab-strip" role="tablist" aria-label="Right sidebar panels">
    <button
      class="tab-btn"
      class:active={activePanel === 'history'}
      role="tab"
      aria-selected={activePanel === 'history'}
      aria-controls="panel-history"
      title="Query History"
      onclick={() => selectPanel('history')}
    >
      🕐
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'saved'}
      role="tab"
      aria-selected={activePanel === 'saved'}
      aria-controls="panel-saved"
      title="Saved Queries"
      onclick={() => selectPanel('saved')}
    >
      🔖
    </button>

    <div class="spacer"></div>

    <button
      class="tab-btn close-btn"
      title="Close sidebar"
      onclick={onClose}
      aria-label="Close sidebar"
    >
      ›
    </button>
  </div>

  <!-- Panel content -->
  <div class="panel-content">
    {#if activePanel === 'history'}
      <div id="panel-history" role="tabpanel" aria-label="Query History">
        <div class="placeholder">Query history will appear here.</div>
      </div>
    {:else if activePanel === 'saved'}
      <div id="panel-saved" role="tabpanel" aria-label="Saved Queries">
        <div class="placeholder">Saved queries will appear here.</div>
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
    flex-direction: column;
    align-items: center;
    width: 100%;
    padding: var(--spacing-1) 0;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-direction: row;
    gap: 2px;
    padding: var(--spacing-1) var(--spacing-2);
  }

  .tab-btn {
    width: 32px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
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

  .close-btn {
    font-size: var(--font-size-lg);
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-2);
  }

  .placeholder {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
    padding: var(--spacing-2);
  }
</style>
