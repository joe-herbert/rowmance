<!--
  SidebarTopHalf — shows the list of currently open panels (query editors and
  table browsers). Clicking an item focuses that panel; clicking × closes it.
  This replaces top tab bars — all open work is visible in the sidebar tree.
-->
<script lang="ts">
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { openNewWindow } from '$lib/tauri/window';
  import type { PanelKind } from '$lib/types';

  const panelStore = usePanels();
  const connectionStore = useConnections();

  /** Human-readable label for a panel based on its content type. */
  function panelLabel(content: PanelKind): string {
    switch (content.kind) {
      case 'query_editor': {
        const conn = connectionStore.getById(content.connectionId);
        return conn ? `Query — ${conn.name}` : 'Query Editor';
      }
      case 'table_browser':
        return content.table;
      case 'empty':
        return 'Empty panel';
    }
  }

  /** Icon character for each panel type. */
  function panelIcon(content: PanelKind): string {
    switch (content.kind) {
      case 'query_editor':
        return '⌨';
      case 'table_browser':
        return '⊞';
      case 'empty':
        return '○';
    }
  }
</script>

<div class="section">
  <div class="section-header no-select">
    <span>Open</span>
    <button
      class="new-window-btn"
      onclick={openNewWindow}
      title="New Window (⌘⇧N)"
      aria-label="Open new window"
    >⊕</button>
  </div>

  {#if panelStore.panels.length === 0 || (panelStore.panels.length === 1 && panelStore.panels[0].content.kind === 'empty')}
    <div class="empty-hint">No open editors</div>
  {:else}
    <ul class="panel-list" role="listbox" aria-label="Open panels">
      {#each panelStore.panels as panel, index (panel.id)}
        {@const isFocused = panelStore.focusedIndex === index}
        <li
          class="panel-item"
          class:focused={isFocused}
          role="option"
          aria-selected={isFocused}
          onclick={() => panelStore.focus(index)}
          onkeydown={(e) => e.key === 'Enter' && panelStore.focus(index)}
          tabindex="0"
        >
          <span class="panel-icon" aria-hidden="true">{panelIcon(panel.content)}</span>
          <span class="panel-label">{panelLabel(panel.content)}</span>
          <button
            class="close-btn"
            aria-label="Close panel"
            onclick={(e) => {
              e.stopPropagation();
              panelStore.closePanel(index);
            }}>×</button
          >
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-1) var(--spacing-1) var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .new-window-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-md);
    color: var(--color-text-muted);
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
  }

  .section:hover .new-window-btn {
    opacity: 1;
  }

  .new-window-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .empty-hint {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .panel-list {
    overflow-y: auto;
    flex: 1;
    padding: var(--spacing-1) 0;
  }

  .panel-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    border-radius: var(--radius-sm);
    margin: 1px var(--spacing-1);
    user-select: none;
    transition: background var(--transition-fast);
  }

  .panel-item:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .panel-item.focused {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .panel-icon {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    width: 14px;
    text-align: center;
  }

  .panel-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .close-btn {
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-md);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast);
  }

  .panel-item:hover .close-btn,
  .panel-item.focused .close-btn {
    opacity: 1;
  }

  .close-btn:hover {
    background: var(--color-bg-active);
    color: var(--color-text-primary);
  }
</style>
