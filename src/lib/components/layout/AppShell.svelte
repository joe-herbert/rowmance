<!--
  AppShell — the root three-column layout of the application.
  Left sidebar | Main split-panel area | Right sidebar (toggleable).
  Handles horizontal resize of both sidebars via pointer-drag.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from './Sidebar.svelte';
  import SplitPanel from './SplitPanel.svelte';
  import RightSidebar from './RightSidebar.svelte';
  import Toast from '$lib/components/ui/Toast.svelte';
  import OnboardingTip from '$lib/components/ui/OnboardingTip.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useShortcuts } from '$lib/stores/shortcuts.svelte';
  import * as updaterApi from '$lib/tauri/updater';
  import { openNewWindow } from '$lib/tauri/window';
  import { listen } from '@tauri-apps/api/event';

  // ── Sidebar widths (persisted as CSS variables) ───────────────────────────

  let leftWidth = $state(240);
  let leftVisible = $state(true);
  let rightWidth = $state(280);
  let rightVisible = $state(true);

  const SIDEBAR_INSET = 0;

  // ── Update notification ───────────────────────────────────────────────────

  const settingsStore = useSettings();
  const settings = $derived(settingsStore.settings);
  const sidebarFloating = $derived(settings.sidebarFloating);
  const panelStore = usePanels();
  const shortcutsStore = useShortcuts();

  function openSettings() {
    const existingIdx = panelStore.panels.findIndex(p => p.content.kind === 'settings');
    if (existingIdx !== -1) {
      panelStore.focus(existingIdx);
    } else {
      panelStore.openInFocused({ kind: 'settings' });
    }
  }

  interface UpdateInfo { version: string; notes: string | null }
  let pendingUpdate = $state<UpdateInfo | null>(null);
  let updateDismissed = $state(false);
  let installing = $state(false);

  onMount(async () => {
    shortcutsStore.load(settings.shortcutPreset);

    if (settings.autoUpdateCheck) {
      try {
        const result = await updaterApi.updaterCheck();
        if (result.available && result.version) {
          pendingUpdate = { version: result.version, notes: result.notes };
        }
      } catch {
        // Update check failures are silently swallowed to avoid disrupting startup.
      }
    }

    const unlisten = await listen('menu:open-settings', openSettings);
    return unlisten;
  });

  async function installUpdate() {
    installing = true;
    try {
      await updaterApi.updaterInstall();
    } catch {
      installing = false;
    }
  }

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

  function toggleLeftSidebar() {
    leftVisible = !leftVisible;
  }

  function toggleRightSidebar() {
    rightVisible = !rightVisible;
  }

  function handleShortcutAction(e: Event) {
    const action = (e as CustomEvent<{ action: string }>).detail.action;
    if (action === 'TOGGLE_LEFT_SIDEBAR') toggleLeftSidebar();
    if (action === 'TOGGLE_RIGHT_SIDEBAR') toggleRightSidebar();
    if (action === 'NEW_WINDOW') openNewWindow();
    if (action === 'OPEN_SETTINGS') openSettings();
    if (action === 'NEW_QUERY_EDITOR') {
      const focused = panelStore.focusedPanel.content;
      const connectionId = 'connectionId' in focused ? focused.connectionId : null;
      if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
    }
  }

  // On macOS with titleBarStyle:"overlay" the webview fills behind the traffic
  // lights, so we render a draggable strip to push content clear of them.
  const isMacOS = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);
</script>

<svelte:document on:shortcut-action={handleShortcutAction} />

<a class="skip-link" href="#main-content">Skip to main content</a>

<div
  class="app-shell-wrapper"
  style="--sidebar-width: {leftVisible ? leftWidth + SIDEBAR_INSET : 0}px; --right-sidebar-width: {rightWidth}px;"
>
  {#if isMacOS}
    <div class="titlebar" data-tauri-drag-region aria-hidden="true"></div>
  {/if}

  {#if pendingUpdate && !updateDismissed}
    <div class="update-banner" role="alert" aria-live="polite">
      <span class="update-message">
        Update {pendingUpdate.version} available
        {#if pendingUpdate.notes} — {pendingUpdate.notes.slice(0, 80)}{/if}
      </span>
      <div class="update-actions">
        <button
          class="update-btn update-btn--primary"
          onclick={installUpdate}
          disabled={installing}
        >
          {installing ? 'Installing…' : 'Install Now'}
        </button>
        <button
          class="update-btn"
          onclick={() => (updateDismissed = true)}
          aria-label="Dismiss update notification"
        >Later</button>
      </div>
    </div>
  {/if}

<div
  class="app-shell"
  role="application"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
>
  <!-- Left sidebar (toggleable) -->
  {#if leftVisible}
    <aside class="left-sidebar" class:floating={sidebarFloating} style="width: {leftWidth}px;">
      <Sidebar onClose={toggleLeftSidebar} />
    </aside>
  {/if}

  <!-- Floating toggle button when left sidebar is hidden -->
  {#if !leftVisible}
    <button
      class="left-sidebar-toggle"
      onclick={toggleLeftSidebar}
      aria-label="Show left sidebar"
      title="Show left sidebar"
    >
      ›
    </button>
  {/if}

  <!-- Resize handle: left sidebar ↔ main area -->
  {#if leftVisible}
    <div
      class="resize-handle resize-handle--horizontal left-resize"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize left sidebar"
      style="left: {leftWidth + SIDEBAR_INSET}px;"
      onpointerdown={(e) => onResizePointerDown('left', e)}
      class:dragging={dragging === 'left'}
    ></div>
  {/if}

  <!-- Main split-panel area -->
  <main id="main-content" class="main-area">
    <SplitPanel />
  </main>

  <!-- Resize handle: main area ↔ right sidebar -->
  {#if rightVisible}
    <div
      class="resize-handle resize-handle--horizontal right-resize"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize right sidebar"
      style="right: {rightWidth + SIDEBAR_INSET}px;"
      onpointerdown={(e) => onResizePointerDown('right', e)}
      class:dragging={dragging === 'right'}
    ></div>
  {/if}

  <!-- Right sidebar (toggleable) -->
  {#if rightVisible}
    <aside class="right-sidebar" class:floating={sidebarFloating} style="width: {rightWidth}px;">
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
</div>

<Toast />
<OnboardingTip />

<style>
  .skip-link {
    position: absolute;
    left: -9999px;
    top: 0;
    z-index: 9999;
    padding: var(--spacing-2) var(--spacing-4);
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    text-decoration: none;
    border-radius: 0 0 var(--radius-md) var(--radius-md);
  }

  .skip-link:focus {
    left: var(--spacing-4);
  }

  .titlebar {
    height: 28px;
    flex-shrink: 0;
    /* Left portion matches the sidebar, right matches the main area — hard stop
       at the sidebar width using the same CSS variable used for layout. */
    background: linear-gradient(
      to right,
      var(--color-bg-secondary) var(--sidebar-width),
      var(--color-bg-primary) var(--sidebar-width)
    );
    /* Keep the border consistent with the sidebar/main-area boundary. */
    border-bottom: 1px solid var(--color-border);
    position: relative;
    z-index: 50;
  }

  .app-shell-wrapper {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .app-shell {
    display: flex;
    flex: 1;
    min-height: 0;
    overflow: hidden;
    position: relative;
    background: var(--color-bg-primary);
  }

  .update-banner {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-4);
    background: var(--color-warning-subtle);
    border-bottom: 1px solid var(--color-warning);
    flex-shrink: 0;
  }

  .update-message {
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--color-warning);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .update-btn {
    height: 26px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-warning);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .update-btn:hover {
    background: rgba(0, 0, 0, 0.05);
  }

  .update-btn--primary {
    background: var(--color-warning);
    color: #fff;
  }

  .update-btn--primary:hover {
    opacity: 0.9;
  }

  .update-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .left-sidebar {
    flex-shrink: 0;
    overflow: hidden;
    background: var(--color-bg-secondary);
    margin: 0;
    border-right: 1px solid var(--color-border);
  }

  .left-sidebar.floating {
    position: absolute;
    left: 8px;
    top: 8px;
    bottom: 8px;
    margin: 0;
    z-index: 15;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
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
    overflow: hidden;
    border-left: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    margin: 0;
  }

  .right-sidebar.floating {
    position: absolute;
    right: 8px;
    top: 8px;
    bottom: 8px;
    margin: 0;
    z-index: 15;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
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

  .left-sidebar-toggle {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 20px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-left: none;
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    color: var(--color-text-secondary);
    font-size: var(--font-size-lg);
    cursor: pointer;
    z-index: 10;
    transition: background var(--transition-fast);
  }

  .left-sidebar-toggle:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
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
