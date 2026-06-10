<!--
  AppShell — the root three-column layout of the application.
  Left sidebar | Main split-panel area | Right sidebar (toggleable).
  Handles horizontal resize of both sidebars via pointer-drag.
-->
<script lang="ts">
  import Sidebar from './Sidebar.svelte';
  import SplitPanel from './SplitPanel.svelte';
  import RightSidebar from './RightSidebar.svelte';

  // ── Sidebar widths (persisted as CSS variables) ───────────────────────────

  let leftWidth = $state(240);
  let rightWidth = $state(280);
  let rightVisible = $state(true);

  // ── Resize drag state ─────────────────────────────────────────────────────

  type ResizeTarget = 'left' | 'right';
  let dragging = $state<ResizeTarget | null>(null);
  let dragStartX = $state(0);
  let dragStartWidth = $state(0);

  function onResizePointerDown(target: ResizeTarget, event: PointerEvent) {
    dragging = target;
    dragStartX = event.clientX;
    dragStartWidth = target === 'left' ? leftWidth : rightWidth;
    (event.target as HTMLElement).setPointerCapture(event.pointerId);
  }

  function onResizePointerMove(event: PointerEvent) {
    if (!dragging) return;
    const delta = event.clientX - dragStartX;

    if (dragging === 'left') {
      leftWidth = Math.max(160, Math.min(500, dragStartWidth + delta));
    } else {
      // Right sidebar grows when dragging leftward (negative delta).
      rightWidth = Math.max(200, Math.min(480, dragStartWidth - delta));
    }
  }

  function onResizePointerUp() {
    dragging = null;
  }

  function toggleRightSidebar() {
    rightVisible = !rightVisible;
  }
</script>

<div
  class="app-shell"
  role="application"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
  style="--sidebar-width: {leftWidth}px; --right-sidebar-width: {rightWidth}px;"
>
  <!-- Left sidebar -->
  <aside class="left-sidebar" style="width: {leftWidth}px;">
    <Sidebar />
  </aside>

  <!-- Resize handle: left sidebar ↔ main area -->
  <div
    class="resize-handle resize-handle--horizontal left-resize"
    role="separator"
    aria-orientation="vertical"
    aria-label="Resize left sidebar"
    style="left: {leftWidth}px;"
    onpointerdown={(e) => onResizePointerDown('left', e)}
    class:dragging={dragging === 'left'}
  ></div>

  <!-- Main split-panel area -->
  <main class="main-area">
    <SplitPanel />
  </main>

  <!-- Resize handle: main area ↔ right sidebar -->
  {#if rightVisible}
    <div
      class="resize-handle resize-handle--horizontal right-resize"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize right sidebar"
      style="right: {rightWidth}px;"
      onpointerdown={(e) => onResizePointerDown('right', e)}
      class:dragging={dragging === 'right'}
    ></div>
  {/if}

  <!-- Right sidebar (toggleable) -->
  {#if rightVisible}
    <aside class="right-sidebar" style="width: {rightWidth}px;">
      <RightSidebar onClose={toggleRightSidebar} />
    </aside>
  {/if}

  <!-- Floating toggle button when right sidebar is hidden -->
  {#if !rightVisible}
    <button
      class="right-sidebar-toggle"
      onclick={toggleRightSidebar}
      aria-label="Show right sidebar"
      title="Show right sidebar"
    >
      ‹
    </button>
  {/if}
</div>

<style>
  .app-shell {
    display: flex;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    position: relative;
    background: var(--color-bg-primary);
  }

  .left-sidebar {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    position: relative;
  }

  .main-area {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .right-sidebar {
    flex-shrink: 0;
    height: 100%;
    overflow: hidden;
    border-left: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .left-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 4px;
    transform: translateX(-2px);
    z-index: 20;
  }

  .right-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 4px;
    transform: translateX(2px);
    z-index: 20;
  }

  .right-sidebar-toggle {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-right: none;
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
    color: var(--color-text-secondary);
    font-size: var(--font-size-lg);
    cursor: pointer;
    z-index: 10;
    transition: background var(--transition-fast);
  }

  .right-sidebar-toggle:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
