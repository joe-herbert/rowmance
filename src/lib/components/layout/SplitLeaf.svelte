<!--
  SplitLeaf — renders a single split panel: optional tab bar + panel content.
  Focus is managed here so Panel.svelte doesn't need an onclick handler.
  Per-split drop zones appear on the right and bottom edges during a tab/content drag.
-->
<script lang="ts">
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useTabDrag } from '$lib/stores/tabDragState.svelte';
  import TabBar from './TabBar.svelte';
  import Panel from './Panel.svelte';

  interface Props {
    splitId: string;
  }

  const { splitId }: Props = $props();

  const panelStore = usePanels();
  const settingsStore = useSettings();
  const tabDrag = useTabDrag();

  const isFocused = $derived(panelStore.focusedSplitId === splitId);

  let isDropHovered = $state(false);

  $effect(() => {
    if (!tabDrag.isDragging) isDropHovered = false;
  });

  const activeContent = $derived.by(() => {
    const items = panelStore.getSplitItems(splitId);
    const focusedId = panelStore.getSplitFocusedItemId(splitId);
    const item = items.find((i) => i.id === focusedId);
    return item?.content ?? { kind: 'empty' as const };
  });

  const focusedItemId = $derived(panelStore.getSplitFocusedItemId(splitId) ?? '');
  const hasItems = $derived(panelStore.getSplitItems(splitId).length > 0);

  const panel = $derived({ id: splitId, content: activeContent });

  const settings = $derived(settingsStore.settings);
  const canSplitHorizontal = $derived(
    panelStore.splitCount < settings.maxHorizontalSplits * settings.maxVerticalSplits &&
      panelStore.maxHorizontalColumns < settings.maxHorizontalSplits,
  );
  const canSplitVertical = $derived(
    panelStore.splitCount < settings.maxHorizontalSplits * settings.maxVerticalSplits &&
      panelStore.maxVerticalRows < settings.maxVerticalSplits,
  );

  let rightZoneHovered = $state(false);
  let bottomZoneHovered = $state(false);
  let leftZoneHovered = $state(false);
  let topZoneHovered = $state(false);

  function handleDrop(direction: 'right' | 'down' | 'left' | 'up') {
    const newSplitId = panelStore.splitAt(
      splitId,
      direction,
      settings.maxHorizontalSplits,
      settings.maxVerticalSplits,
    );
    if (!newSplitId) {
      tabDrag.end();
      return;
    }
    if (tabDrag.itemId) {
      panelStore.moveItemToSplit(tabDrag.itemId, newSplitId);
    } else if (tabDrag.dragContent) {
      panelStore.openInSplit(tabDrag.dragContent, newSplitId);
    }
    tabDrag.end();
    rightZoneHovered = false;
    bottomZoneHovered = false;
    leftZoneHovered = false;
    topZoneHovered = false;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="split-leaf"
  class:focused={isFocused}
  class:drop-target={isDropHovered}
  role="region"
  aria-label={panelStore.getSplitLabel(splitId)}
  data-split-leaf-id={splitId}
  onclick={() => !isFocused && panelStore.focusSplit(splitId)}
  onkeydown={(e) => {
    if (!isFocused && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      panelStore.focusSplit(splitId);
    }
  }}
  onpointerenter={() => {
    if (tabDrag.isDragging && (tabDrag.sourceSplitId !== splitId || tabDrag.dragContent !== null))
      isDropHovered = true;
  }}
  onpointerleave={() => {
    isDropHovered = false;
  }}
  onpointerup={() => {
    if (!tabDrag.isDragging) return;
    if (tabDrag.itemId && tabDrag.sourceSplitId !== splitId) {
      panelStore.moveItemToSplit(tabDrag.itemId, splitId);
      tabDrag.end();
    } else if (tabDrag.dragContent) {
      panelStore.openInSplit(tabDrag.dragContent, splitId);
      tabDrag.end();
    }
  }}
>
  {#if hasItems && settingsStore.settings.openItemsLocation === 'top'}
    <TabBar {splitId} />
  {/if}
  <Panel {panel} {isFocused} {splitId} itemId={focusedItemId} />

  {#if tabDrag.isDragging && canSplitHorizontal}
    <div
      class="drop-zone drop-zone--left"
      class:active={leftZoneHovered}
      onpointerenter={() => (leftZoneHovered = true)}
      onpointerleave={() => (leftZoneHovered = false)}
      onpointerup={() => handleDrop('left')}
      role="region"
      aria-label="Drop here to split left"
    >
      <span class="drop-zone-label drop-zone-label--vertical">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <line x1="19" y1="12" x2="5" y2="12"></line>
          <polyline points="11 18 5 12 11 6"></polyline>
        </svg>
        Split left
      </span>
    </div>

    <div
      class="drop-zone drop-zone--right"
      class:active={rightZoneHovered}
      onpointerenter={() => (rightZoneHovered = true)}
      onpointerleave={() => (rightZoneHovered = false)}
      onpointerup={() => handleDrop('right')}
      role="region"
      aria-label="Drop here to split right"
    >
      <span class="drop-zone-label drop-zone-label--vertical">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <line x1="5" y1="12" x2="19" y2="12"></line>
          <polyline points="13 6 19 12 13 18"></polyline>
        </svg>
        Split right
      </span>
    </div>
  {/if}

  {#if tabDrag.isDragging && canSplitVertical}
    <div
      class="drop-zone drop-zone--top"
      class:active={topZoneHovered}
      onpointerenter={() => (topZoneHovered = true)}
      onpointerleave={() => (topZoneHovered = false)}
      onpointerup={() => handleDrop('up')}
      role="region"
      aria-label="Drop here to split up"
    >
      <span class="drop-zone-label">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <line x1="12" y1="19" x2="12" y2="5"></line>
          <polyline points="18 11 12 5 6 11"></polyline>
        </svg>
        Split up
      </span>
    </div>

    <div
      class="drop-zone drop-zone--bottom"
      class:active={bottomZoneHovered}
      onpointerenter={() => (bottomZoneHovered = true)}
      onpointerleave={() => (bottomZoneHovered = false)}
      onpointerup={() => handleDrop('down')}
      role="region"
      aria-label="Drop here to split down"
    >
      <span class="drop-zone-label">
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <polyline points="6 13 12 19 18 13"></polyline>
        </svg>
        Split down
      </span>
    </div>
  {/if}
</div>

<style>
  .split-leaf {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    position: relative;
    border-radius: var(--panel-radius);
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    opacity: var(--panel-opacity);
  }

  .split-leaf:not(.focused)::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: rgba(0, 0, 0, 0.02);
    z-index: 1;
    border-radius: var(--panel-radius);
  }

  .split-leaf.drop-target {
    border-color: var(--color-accent);
    box-shadow: inset 0 0 0 1px var(--color-accent);
  }

  /* ── Per-split drop zones ─────────────────────────────────────────────────── */

  .drop-zone {
    position: absolute;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 20;
    background: color-mix(in srgb, var(--color-bg-primary) 60%, transparent);
    border: 2px dashed var(--color-border);
    transition:
      background var(--transition-fast),
      border-color var(--transition-fast);
    cursor: copy;
    pointer-events: all;
  }

  .drop-zone--left {
    top: 15%;
    left: 0;
    width: 52px;
    height: 70%;
    border-left: none;
    border-radius: 0 var(--panel-radius) var(--panel-radius) 0;
  }

  .drop-zone--top {
    left: 15%;
    top: 0;
    width: 70%;
    height: 52px;
    border-top: none;
    border-radius: 0 0 var(--panel-radius) var(--panel-radius);
  }

  .drop-zone--right {
    top: 15%;
    right: 0;
    width: 52px;
    height: 70%;
    border-right: none;
    border-radius: var(--panel-radius) 0 0 var(--panel-radius);
  }

  .drop-zone--bottom {
    left: 15%;
    bottom: 0;
    width: 70%;
    height: 52px;
    border-bottom: none;
    border-radius: var(--panel-radius) var(--panel-radius) 0 0;
  }

  .drop-zone.active {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    border-color: var(--color-accent);
  }

  .drop-zone-label {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    text-align: center;
    pointer-events: none;
    -webkit-user-select: none;
    user-select: none;
  }

  .drop-zone.active .drop-zone-label {
    color: var(--color-accent);
  }

  .drop-zone-label--vertical {
    writing-mode: vertical-rl;
    flex-direction: row;
    gap: 4px;
  }
</style>
