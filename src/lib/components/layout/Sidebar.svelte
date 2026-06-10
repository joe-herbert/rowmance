<!--
  Sidebar — the left sidebar containing two vertically resizable halves:
  - Top: list of currently open editors and table browsers (open panels).
  - Bottom: the connection/schema tree.
  The divider between the two halves is draggable.
-->
<script lang="ts">
  import SidebarTopHalf from './SidebarTopHalf.svelte';
  import SidebarBottomHalf from './SidebarBottomHalf.svelte';

  // Top half height as a percentage of the sidebar total height.
  let topHeightPct = $state(35);

  let dragging = $state(false);
  let containerEl = $state<HTMLElement | null>(null);

  function onDividerPointerDown(event: PointerEvent) {
    dragging = true;
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onDividerPointerMove(event: PointerEvent) {
    if (!dragging || !containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    const relative = event.clientY - rect.top;
    const pct = (relative / rect.height) * 100;
    topHeightPct = Math.max(15, Math.min(75, pct));
  }

  function onDividerPointerUp() {
    dragging = false;
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="sidebar"
  bind:this={containerEl}
  onpointermove={onDividerPointerMove}
  onpointerup={onDividerPointerUp}
>
  <div class="top-half" style="height: {topHeightPct}%;">
    <SidebarTopHalf />
  </div>

  <div
    class="divider"
    class:dragging
    role="separator"
    aria-orientation="horizontal"
    aria-label="Resize sidebar sections"
    onpointerdown={onDividerPointerDown}
  ></div>

  <div class="bottom-half" style="height: calc({100 - topHeightPct}% - 4px);">
    <SidebarBottomHalf />
  </div>
</div>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    position: relative;
  }

  .top-half,
  .bottom-half {
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .divider {
    flex-shrink: 0;
    height: 4px;
    background: var(--color-border);
    cursor: row-resize;
    transition: background var(--transition-fast);
  }

  .divider:hover,
  .divider.dragging {
    background: var(--color-accent);
    opacity: 0.5;
  }
</style>
