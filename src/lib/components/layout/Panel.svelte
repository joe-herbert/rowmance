<!--
  Panel — a single cell in the split-panel grid.
  Routes its PanelState.content to the appropriate component.
  Clicking anywhere in an unfocused panel focuses it.
-->
<script lang="ts">
  import type { PanelState } from '$lib/types';
  import { usePanels } from '$lib/stores/panels.svelte';
  import QueryEditor from '$lib/components/editor/QueryEditor.svelte';
  import TableBrowser from '$lib/components/table/TableBrowser.svelte';

  interface Props {
    index: number;
    panel: PanelState;
    isFocused: boolean;
  }

  const { index, panel, isFocused }: Props = $props();
  const panelStore = usePanels();
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
    <QueryEditor connectionId={panel.content.connectionId} />
  {:else if panel.content.kind === 'table_browser'}
    <TableBrowser
      connectionId={panel.content.connectionId}
      database={panel.content.database}
      table={panel.content.table}
    />
  {:else}
    <!-- Empty panel placeholder -->
    <div class="empty-panel">
      <div class="empty-panel-hint">
        <p>Open a table from the sidebar, or start a new query.</p>
      </div>
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
    /* Subtle overlay on unfocused panels to indicate they are clickable. */
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: rgba(0, 0, 0, 0.015);
  }

  .empty-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-primary);
  }

  .empty-panel-hint {
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    max-width: 220px;
    line-height: var(--line-height-normal);
  }
</style>
