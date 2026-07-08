<!--
  SplitNodeRenderer — recursive component that renders a SplitNode tree.
  Leaf nodes render a SplitLeaf; container nodes render a flex layout
  with draggable resize dividers between children.
-->
<script lang="ts">
  import SplitNodeRenderer from './SplitNodeRenderer.svelte';
  import type { SplitNode } from '$lib/types';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useLeafSlots } from '$lib/stores/splitLeafSlots.svelte';

  const leafSlots = useLeafSlots();

  function registerLeafSlot(el: HTMLElement, splitId: string) {
    leafSlots.register(splitId, el);
    return {
      update(newSplitId: string) {
        leafSlots.unregisterIfOwner(splitId, el);
        leafSlots.register(newSplitId, el);
        splitId = newSplitId;
      },
      destroy() {
        leafSlots.unregisterIfOwner(splitId, el);
      },
    };
  }

  interface Props {
    node: SplitNode;
  }

  const { node }: Props = $props();

  const panelStore = usePanels();

  // ── Local resize state (only used for container nodes) ────────────────────

  let localSizes = $state<number[]>([]);
  let resizing = $state<number | null>(null); // index of the divider being dragged (1-based: divider i is between children i-1 and i)
  let resizeStartPos = $state(0);
  let resizeStartSizes = $state<number[]>([]);
  let containerEl = $state<HTMLElement | null>(null);

  // Keep localSizes in sync with node.children when the count changes
  $effect(() => {
    if (node.type !== 'container') return;
    const childCount = node.children.length;
    if (localSizes.length !== childCount) {
      localSizes = node.children.map((c) => c.size);
    }
  });

  // Also sync localSizes from the store when not resizing (handles external updates)
  $effect(() => {
    if (node.type !== 'container' || resizing !== null) return;
    const storeSizes = node.children.map((c) => c.size);
    const same =
      storeSizes.length === localSizes.length &&
      storeSizes.every((s, i) => Math.abs(s - (localSizes[i] ?? 0)) < 0.01);
    if (!same) {
      localSizes = storeSizes;
    }
  });

  function onDividerPointerDown(e: PointerEvent, dividerIndex: number) {
    if (node.type !== 'container') return;
    e.preventDefault();
    resizing = dividerIndex;
    resizeStartPos = node.direction === 'horizontal' ? e.clientX : e.clientY;
    resizeStartSizes = [...localSizes];
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (resizing === null || !containerEl || node.type !== 'container') return;

    const rect = containerEl.getBoundingClientRect();
    // 11 matches --panel-spacing in variables.css
    const dividerPx = 11;
    const containerDim =
      node.direction === 'horizontal'
        ? rect.width - (node.children.length - 1) * dividerPx
        : rect.height - (node.children.length - 1) * dividerPx;

    const currentPos = node.direction === 'horizontal' ? e.clientX : e.clientY;
    const delta = currentPos - resizeStartPos;
    const deltaPct = (delta / containerDim) * 100;

    const i = resizing - 1; // index of child before divider
    const j = resizing; // index of child after divider

    let newA = resizeStartSizes[i] + deltaPct;
    let newB = resizeStartSizes[j] - deltaPct;

    // Clamp to 10% minimum
    if (newA < 10) {
      newB += newA - 10;
      newA = 10;
    }
    if (newB < 10) {
      newA += newB - 10;
      newB = 10;
    }
    if (newA < 10) newA = 10;
    if (newB < 10) newB = 10;

    localSizes = localSizes.map((s, idx) => (idx === i ? newA : idx === j ? newB : s));
  }

  function onPointerUp() {
    if (resizing === null) return;
    if (node.type === 'container') {
      panelStore.updateContainerSizes(node.containerId, localSizes);
    }
    resizing = null;
  }

  function onDividerKeydown(e: KeyboardEvent, dividerIndex: number) {
    if (node.type !== 'container') return;
    const step = e.shiftKey ? 5 : 1;
    const i = dividerIndex - 1;
    const j = dividerIndex;
    const isH = node.direction === 'horizontal';

    const shrinkFirst = isH ? e.key === 'ArrowLeft' : e.key === 'ArrowUp';
    const growFirst = isH ? e.key === 'ArrowRight' : e.key === 'ArrowDown';

    if (shrinkFirst || growFirst) {
      e.preventDefault();
      const delta = shrinkFirst ? -step : step;
      let newA = localSizes[i] + delta;
      let newB = localSizes[j] - delta;
      newA = Math.max(10, Math.min(90, newA));
      newB = Math.max(10, Math.min(90, newB));
      localSizes = localSizes.map((s, idx) => (idx === i ? newA : idx === j ? newB : s));
      panelStore.updateContainerSizes(node.containerId, localSizes);
    }
  }
</script>

{#if node.type === 'leaf'}
  <div class="leaf-slot" data-leaf-slot-id={node.splitId} use:registerLeafSlot={node.splitId}></div>
{:else}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="split-container"
    class:horizontal={node.direction === 'horizontal'}
    class:vertical={node.direction === 'vertical'}
    bind:this={containerEl}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
  >
    {#each node.children as child, i}
      {#if i > 0}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="divider"
          class:divider--col={node.direction === 'horizontal'}
          class:divider--row={node.direction === 'vertical'}
          class:dragging={resizing === i}
          onpointerdown={(e) => onDividerPointerDown(e, i)}
          onkeydown={(e) => onDividerKeydown(e, i)}
          role="separator"
          tabindex="0"
          aria-orientation={node.direction === 'horizontal' ? 'vertical' : 'horizontal'}
          aria-label={node.direction === 'horizontal'
            ? 'Resize panels left or right'
            : 'Resize panels up or down'}
        ></div>
      {/if}
      <div
        class="split-child"
        style="flex: {localSizes[i] ?? child.size} 1 0%; min-width: 0; min-height: 0;"
      >
        <SplitNodeRenderer node={child.node} />
      </div>
    {/each}
  </div>
{/if}

<style>
  .leaf-slot {
    width: 100%;
    height: 100%;
  }

  .split-container {
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  .split-container.horizontal {
    display: flex;
    flex-direction: row;
  }

  .split-container.vertical {
    display: flex;
    flex-direction: column;
  }

  .split-child {
    overflow: hidden;
    height: 100%;
  }

  .split-container.vertical .split-child {
    height: auto;
    width: 100%;
  }

  .divider {
    flex: 0 0 var(--panel-spacing);
    background: transparent;
    z-index: 5;
    position: relative;
    outline: none;
  }

  .divider--col {
    cursor: col-resize;
    width: var(--panel-spacing);
    height: 100%;
  }

  .divider--row {
    cursor: row-resize;
    height: var(--panel-spacing);
    width: 100%;
  }

  .divider::after {
    content: '';
    position: absolute;
    background: var(--color-accent);
    border-radius: 1px;
    opacity: 0;
    transition: opacity var(--transition-fast);
  }

  .divider--col::after {
    top: var(--panel-radius);
    bottom: var(--panel-radius);
    left: 50%;
    width: 2px;
    transform: translateX(-50%);
  }

  .divider--row::after {
    left: var(--panel-radius);
    right: var(--panel-radius);
    top: 50%;
    height: 2px;
    transform: translateY(-50%);
  }

  .divider:hover::after,
  .divider.dragging::after {
    opacity: 0.5;
  }
</style>
