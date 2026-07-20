<script lang="ts">
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import {
    generateSqlSelectAll,
    generateSqlSelectFirst,
    generateSqlInsert,
    generateSqlUpdate,
    generateSqlDelete,
  } from '$lib/utils/tab-sql-generation';
  import type { OpenItem } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';

  interface Props {
    item: OpenItem;
    splitId: string;
    x: number;
    y: number;
    open: boolean;
    minWidth?: number;
    onclose: () => void;
    onrename: (_item: OpenItem) => void;
  }

  const { item, splitId, x, y, open, minWidth, onclose, onrename }: Props = $props();

  const panelStore = usePanels();
  const connectionStore = useConnections();
  const settingsStore = useSettings();

  const splitItems = $derived(panelStore.getSplitItems(splitId));
  const itemIndex = $derived(splitItems.findIndex((i) => i.id === item.id));
  const hasBefore = $derived(itemIndex > 0);
  const hasAfter = $derived(itemIndex >= 0 && itemIndex < splitItems.length - 1);
  const isTabBar = $derived(settingsStore.settings.openItemsLocation === 'top');
  const beforeLabel = $derived(isTabBar ? 'Close tabs to the left' : 'Close tabs above');
  const afterLabel = $derived(isTabBar ? 'Close tabs to the right' : 'Close tabs below');
</script>

<ContextMenu {x} {y} {open} {onclose} {minWidth}>
  {#if item.content.kind === 'query_editor' && item.content.savedQueryId}
    <CtxItem
      onclick={() => {
        onclose();
        onrename(item);
      }}>Rename</CtxItem
    >
    <CtxSep />
  {/if}
  {#if item.content.kind === 'table_browser'}
    {@const tb = item.content}
    <CtxItem
      onclick={() => {
        onclose();
        generateSqlSelectAll(tb.connectionId, tb.database, tb.table, connectionStore, panelStore);
      }}>Select All Rows</CtxItem
    >
    <CtxItem
      onclick={() => {
        onclose();
        generateSqlSelectFirst(tb.connectionId, tb.database, tb.table, connectionStore, panelStore);
      }}>Select First N Rows</CtxItem
    >
    <CtxItem
      onclick={async () => {
        onclose();
        await generateSqlInsert(
          tb.connectionId,
          tb.database,
          tb.table,
          connectionStore,
          panelStore,
        );
      }}>Insert Row</CtxItem
    >
    <CtxItem
      onclick={async () => {
        onclose();
        await generateSqlUpdate(
          tb.connectionId,
          tb.database,
          tb.table,
          connectionStore,
          panelStore,
        );
      }}>Update Rows</CtxItem
    >
    <CtxItem
      onclick={async () => {
        onclose();
        await generateSqlDelete(
          tb.connectionId,
          tb.database,
          tb.table,
          connectionStore,
          panelStore,
        );
      }}>Delete Rows</CtxItem
    >
    <CtxSep />
  {/if}
  {#if hasBefore}
    <CtxItem
      onclick={() => {
        const id = item.id;
        onclose();
        panelStore.closeItemsBefore(id);
      }}>{beforeLabel}</CtxItem
    >
  {/if}
  {#if hasAfter}
    <CtxItem
      onclick={() => {
        const id = item.id;
        onclose();
        panelStore.closeItemsAfter(id);
      }}>{afterLabel}</CtxItem
    >
  {/if}
  {#if splitItems.length > 1}
    <CtxItem
      onclick={() => {
        const id = item.id;
        onclose();
        panelStore.closeOtherItems(id);
      }}>Close other tabs</CtxItem
    >
  {/if}
  {#if 'connectionId' in item.content}
    <CtxItem
      onclick={() => {
        const connId = (item.content as { connectionId: string }).connectionId;
        onclose();
        panelStore.closeItemsForConnection(connId);
      }}>Close all tabs for this connection</CtxItem
    >
  {/if}
  {#if panelStore.splitCount > 1}
    {#each panelStore.getAllLeafIds().filter((id) => id !== splitId) as otherSplitId}
      <CtxItem
        onclick={() => {
          const id = item.id;
          onclose();
          panelStore.moveItemToSplit(id, otherSplitId);
        }}>Move to {panelStore.getSplitLabel(otherSplitId)}</CtxItem
      >
    {/each}
    {#each panelStore.getAllLeafIds().filter((id) => id !== splitId) as otherSplitId}
      <CtxItem
        onclick={() => {
          const content = item.content;
          onclose();
          panelStore.copyItemToSplit(content, otherSplitId);
        }}>Open copy in {panelStore.getSplitLabel(otherSplitId)}</CtxItem
      >
    {/each}
    <CtxItem
      onclick={() => {
        onclose();
        panelStore.closeSplit(splitId);
      }}>Close split</CtxItem
    >
  {/if}
</ContextMenu>
