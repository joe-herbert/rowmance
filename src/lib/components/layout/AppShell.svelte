<!--
  AppShell — the root three-column layout of the application.
  Left sidebar | Main split-panel area | Right sidebar (toggleable).
  Handles horizontal resize of both sidebars via pointer-drag.
-->
<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import Sidebar from './Sidebar.svelte';
  import SplitPanel from './SplitPanel.svelte';
  import RightSidebar from './RightSidebar.svelte';
  import Toast from '$lib/components/ui/Toast.svelte';
  import OnboardingTip from '$lib/components/ui/OnboardingTip.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useShortcuts } from '$lib/stores/shortcuts.svelte';
  import CommandPalette from '$lib/components/palette/CommandPalette.svelte';
  import * as updaterApi from '$lib/tauri/updater';
  import { openNewWindow, syncTrafficLightPosition } from '$lib/tauri/window';
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
  const panelStore = usePanels();
  const shortcutsStore = useShortcuts();

  function openSettings() {
    panelStore.openInFocused({ kind: 'settings' });
  }

  interface UpdateInfo { version: string; notes: string | null }
  let pendingUpdate = $state<UpdateInfo | null>(null);
  let updateDismissed = $state(false);
  let installing = $state(false);

  $effect(() => {
    const _theme = settings.theme;
    tick().then(syncTrafficLightPosition);
  });

  onMount(() => {
    shortcutsStore.load(settings.shortcutPreset);

    if (settings.autoUpdateCheck) {
      updaterApi.updaterCheck().then((result) => {
        if (result.available && result.version) {
          pendingUpdate = { version: result.version, notes: result.notes };
        }
      }).catch(() => {
        // Update check failures are silently swallowed to avoid disrupting startup.
      });
    }

    let unlistenFn: (() => void) | undefined;
    listen('menu:open-settings', openSettings).then((unlisten) => {
      unlistenFn = unlisten;
    });

    return () => {
      unlistenFn?.();
    };
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
    if (action === 'COMMAND_PALETTE') openPalette();
    if (action === 'NEW_QUERY_EDITOR') {
      const focused = panelStore.focusedPanel.content;
      const connectionId = 'connectionId' in focused ? focused.connectionId : null;
      if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
    }
  }

  let paletteOpen = $state(false);

  function openPalette() { paletteOpen = true; }
  function closePalette() { paletteOpen = false; }

  // On macOS with titleBarStyle:"overlay" the webview fills behind the native traffic
  // lights, so we reserve space to push content clear of them.
  const isMacOS = typeof navigator !== 'undefined' && /Mac/.test(navigator.platform);

  $effect(() => {
    document.addEventListener('shortcut-action', handleShortcutAction);
    return () => document.removeEventListener('shortcut-action', handleShortcutAction);
  });
</script>

<a class="skip-link" href="#main-content">Skip to main content</a>

<div
  class="app-shell-wrapper"
  style="--sidebar-width: {leftVisible ? leftWidth + SIDEBAR_INSET : 0}px; --right-sidebar-width: {rightWidth}px;"
>
  <div class="titlebar-card" data-tauri-drag-region>
    {#if isMacOS}
      <div class="traffic-lights-spacer" aria-hidden="true"></div>
    {/if}
    <span class="app-title">Rowmance</span>
    <div class="titlebar-spacer"></div>
    <button class="titlebar-btn" onclick={openPalette} title="Command palette (⌘K)" aria-label="Open command palette">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
        <circle cx="11" cy="11" r="7"></circle>
        <line x1="21" y1="21" x2="16.5" y2="16.5"></line>
      </svg>
      <span class="titlebar-btn-label">Search</span>
      <kbd>⌘K</kbd>
    </button>
    <button class="titlebar-btn titlebar-btn--icon" onclick={openSettings} title="Settings" aria-label="Open settings">
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="3"></circle>
        <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
      </svg>
    </button>
  </div>

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
  class="app-body"
  role="application"
  onpointermove={onResizePointerMove}
  onpointerup={onResizePointerUp}
>
  <!-- Left sidebar (toggleable) -->
  {#if leftVisible}
    <aside class="left-sidebar" style="width: {leftWidth}px;" transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}>
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
      transition:fade={{ duration: 150 }}
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
      style="left: {leftWidth}px;"
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
      style="right: {rightWidth}px;"
      onpointerdown={(e) => onResizePointerDown('right', e)}
      class:dragging={dragging === 'right'}
    ></div>
  {/if}

  <!-- Right sidebar (toggleable) -->
  {#if rightVisible}
    <aside class="right-sidebar" style="width: {rightWidth}px;" transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}>
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
      transition:fade={{ duration: 150 }}
    >
      ‹
    </button>
  {/if}
</div>
</div>

<Toast />
<OnboardingTip />

{#if paletteOpen}
  <CommandPalette onclose={closePalette} />
{/if}

<style>
  .app-shell-wrapper {
    display: flex;
    flex-direction: column;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    padding: var(--panel-spacing);
    gap: var(--panel-spacing);
    background: transparent;
  }

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

  /* ── Title bar card ────────────────────────────────────────────────────── */

  .titlebar-card {
    flex-shrink: 0;
    height: 46px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 16px;
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--panel-radius);
    opacity: var(--panel-opacity);
    position: relative;
    z-index: 45;
  }

  /* Space reserved for native macOS traffic light buttons (3×12px + 2×8px gap + margins) */
  .traffic-lights-spacer {
    width: 68px;
    flex-shrink: 0;
  }

  .app-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    letter-spacing: 0.02em;
    user-select: none;
  }

  .titlebar-spacer {
    flex: 1;
  }

  .titlebar-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 10px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
    flex-shrink: 0;
  }

  .titlebar-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .titlebar-btn--icon {
    width: 30px;
    height: 30px;
    padding: 0;
    justify-content: center;
  }

  .titlebar-btn-label {
    color: var(--color-text-muted);
  }

  kbd {
    font-size: 10px;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-family: var(--font-family-ui);
  }

  /* ── Update banner ─────────────────────────────────────────────────────── */

  .update-banner {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-2) var(--spacing-4);
    background: var(--color-warning-subtle);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-xl);
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

  .update-btn:hover { background: rgba(0, 0, 0, 0.05); }
  .update-btn--primary { background: var(--color-warning); color: #fff; }
  .update-btn--primary:hover { opacity: 0.9; }
  .update-btn:disabled { opacity: 0.6; cursor: not-allowed; }

  /* ── App body (flex row with sidebar + main + sidebar) ─────────────────── */

  .app-body {
    display: flex;
    flex: 1;
    min-height: 0;
    gap: var(--panel-spacing);
    position: relative;
  }

  /* ── Left sidebar card ─────────────────────────────────────────────────── */

  .left-sidebar {
    flex-shrink: 0;
    overflow: hidden;
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--panel-radius);
    opacity: var(--panel-opacity);
  }

  /* ── Main content area ─────────────────────────────────────────────────── */

  .main-area {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--panel-radius);
    opacity: var(--panel-opacity);
  }

  /* ── Right sidebar card ────────────────────────────────────────────────── */

  .right-sidebar {
    flex-shrink: 0;
    overflow: hidden;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border-radius: var(--panel-radius);
    opacity: var(--panel-opacity);
  }

  /* ── Resize handles ─────────────────────────────────────────────────────── */

  .left-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    width: var(--panel-spacing);
    z-index: 20;
    cursor: col-resize;
    background: transparent;
  }

  .right-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    width: var(--panel-spacing);
    z-index: 20;
    cursor: col-resize;
    background: transparent;
  }

  .left-resize:hover,
  .left-resize.dragging,
  .right-resize:hover,
  .right-resize.dragging {
    background: color-mix(in srgb, var(--color-accent) 30%, transparent);
    border-radius: var(--radius-sm);
  }

  /* ── Sidebar toggle buttons (when sidebar is hidden) ───────────────────── */

  .left-sidebar-toggle {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 28px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: 0 var(--radius-lg) var(--radius-lg) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-lg);
    cursor: pointer;
    z-index: 10;
    transition: background var(--transition-fast), color var(--transition-fast);
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
    width: 28px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg) 0 0 var(--radius-lg);
    color: var(--color-text-muted);
    font-size: var(--font-size-lg);
    cursor: pointer;
    z-index: 10;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .right-sidebar-toggle:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
