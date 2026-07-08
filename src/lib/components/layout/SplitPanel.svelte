<!--
  SplitPanel — root of the split layout.
  Renders the recursive SplitNodeRenderer for layout/dividers, then overlays
  each SplitLeaf as a position:absolute portal so leaves never unmount when
  the tree structure changes (splits added/removed).
-->
<script lang="ts">
  import SplitNodeRenderer from './SplitNodeRenderer.svelte';
  import SplitLeaf from './SplitLeaf.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useLeafSlots } from '$lib/stores/splitLeafSlots.svelte';

  const panelStore = usePanels();
  const leafSlots = useLeafSlots();

  let containerEl = $state<HTMLElement | null>(null);

  // Map from splitId → pixel rect relative to containerEl
  let leafPositions = $state(
    new Map<string, { left: number; top: number; width: number; height: number }>(),
  );

  function syncPositions() {
    if (!containerEl) return;
    const containerRect = containerEl.getBoundingClientRect();
    // Start from existing positions so splits whose slots are mid-transition
    // (destroyed but not yet re-mounted) stay visible at their last known position.
    const next = new Map(leafPositions);
    for (const [splitId, slotEl] of leafSlots.slots) {
      const rect = slotEl.getBoundingClientRect();
      next.set(splitId, {
        left: rect.left - containerRect.left,
        top: rect.top - containerRect.top,
        width: rect.width,
        height: rect.height,
      });
    }
    // Remove positions only for splits that are gone from the layout entirely.
    const currentIds = new Set(panelStore.getAllLeafIds());
    for (const key of next.keys()) {
      if (!currentIds.has(key)) next.delete(key);
    }
    leafPositions = next;
  }

  // ResizeObserver on each slot element — fires on window resize and during
  // divider drags (each child's flex size changes even though the container
  // total size stays the same).
  let ro: ResizeObserver | null = null;

  $effect(() => {
    ro = new ResizeObserver(() => {
      syncPositions();
    });

    return () => {
      ro?.disconnect();
      ro = null;
    };
  });

  // Re-observe whenever the slot map changes (slots added/removed).
  $effect(() => {
    if (!ro) return;
    ro.disconnect();
    for (const slotEl of leafSlots.slots.values()) {
      ro.observe(slotEl);
    }
    // Sync positions immediately after slot map changes (new leaf mounted).
    requestAnimationFrame(() => syncPositions());
  });

  // Sync positions when the layout tree changes structurally.
  $effect(() => {
    // Track layout reactively.
    panelStore.layout;
    requestAnimationFrame(() => syncPositions());
  });
</script>

<div class="split-panel" bind:this={containerEl}>
  <SplitNodeRenderer node={panelStore.layout} />

  {#each panelStore.getAllLeafIds() as splitId (splitId)}
    {@const pos = leafPositions.get(splitId)}
    <div
      class="leaf-portal"
      style={pos
        ? `left:${pos.left}px;top:${pos.top}px;width:${pos.width}px;height:${pos.height}px`
        : 'display:none'}
    >
      <SplitLeaf {splitId} />
    </div>
  {/each}
</div>

<style>
  .split-panel {
    width: 100%;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  .leaf-portal {
    position: absolute;
    overflow: hidden;
  }
</style>
