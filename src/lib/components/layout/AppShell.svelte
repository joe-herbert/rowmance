<!--
  AppShell — the root three-column layout of the application.
  Left sidebar | Main split-panel area | Right sidebar (toggleable).
  Handles horizontal resize of both sidebars via pointer-drag.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import Sidebar from './Sidebar.svelte';
  import SplitPanel from './SplitPanel.svelte';
  import RightSidebar from './RightSidebar.svelte';
  import StatusBar from './StatusBar.svelte';
  import Toast from '$lib/components/ui/Toast.svelte';
  import OnboardingTip from '$lib/components/ui/OnboardingTip.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import { usePanels, dirtyKeyForContent, sameContent } from '$lib/stores/panels.svelte';
  import { clearTablePendingState } from '$lib/components/table/TableBrowser.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import { useShortcuts } from '$lib/stores/shortcuts.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import CommandPalette from '$lib/components/palette/CommandPalette.svelte';
  import GlobalSearch from '$lib/components/palette/GlobalSearch.svelte';
  import RecordingModal from '$lib/components/ui/RecordingModal.svelte';
  import RevertModal from '$lib/components/ui/RevertModal.svelte';
  import { useRecording } from '$lib/stores/recording.svelte';
  import { useRevert } from '$lib/stores/revert.svelte';
  import * as updaterApi from '$lib/tauri/updater';
  import * as txApi from '$lib/tauri/transactions';
  import { errorMessage } from '$lib/utils/errors';
  import { openNewWindow, syncTrafficLightPosition } from '$lib/tauri/window';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import { listen } from '@tauri-apps/api/event';
  import { getVersion } from '@tauri-apps/api/app';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openFileDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs';

  // ── Settings ──────────────────────────────────────────────────────────────

  const settingsStore = useSettings();
  const settings = $derived(settingsStore.settings);

  // ── Sidebar widths (persisted as CSS variables) ───────────────────────────

  let leftWidth = $state(240);
  let leftVisible = $state(untrack(() => settings.leftSidebarVisible));
  let rightWidth = $state(280);
  type RightPanel = 'history' | 'saved' | 'column' | 'table-info' | 'relations';

  let rightVisible = $state(untrack(() => settings.rightSidebarVisible));
  let activeRightPanel = $state<RightPanel>(
    untrack(() => (settings.rightSidebarPanel as RightPanel) || 'history'),
  );

  const SIDEBAR_INSET = 0;

  // ── Stores ────────────────────────────────────────────────────────────────

  const panelStore = usePanels();
  const shortcutsStore = useShortcuts();
  const connectionsStore = useConnections();
  const toast = useToast();
  const recordingStore = useRecording();
  const revertStore = useRevert();

  // ── Active connection + view mode (derived from focused panel) ────────────

  const focusedContent = $derived(panelStore.focusedPanel.content);

  const activeConnection = $derived.by(() => {
    const c = focusedContent;
    if ('connectionId' in c) return connectionsStore.getById(c.connectionId) ?? null;
    return null;
  });

  const isConnected = $derived(
    activeConnection ? connectionsStore.isActive(activeConnection.id) : false,
  );

  $effect(() => {
    void settings.theme; // re-run after theme switches, which clear documentElement inline styles
    const color = activeConnection?.color;
    if (color) {
      document.documentElement.style.setProperty('--color-connection', color);
    } else {
      document.documentElement.style.removeProperty('--color-connection');
    }
  });

  $effect(() => {
    const isTableBrowser = focusedContent.kind === 'table_browser';
    invoke('menu_set_import_csv_enabled', { enabled: isTableBrowser }).catch(() => {});
  });

  type ViewMode = 'data' | 'structure' | 'sql';

  const activeView = $derived.by((): ViewMode | null => {
    switch (focusedContent.kind) {
      case 'table_browser':
        return 'data';
      case 'table_structure':
        return 'structure';
      case 'ddl_viewer':
        return 'sql';
      default:
        return null;
    }
  });

  function switchView(mode: ViewMode) {
    const c = focusedContent;
    if (!('connectionId' in c)) return;
    const { connectionId } = c;

    let database: string | null = null;
    let table: string | null = null;
    if (c.kind === 'table_browser') {
      database = c.database;
      table = c.table;
    } else if (c.kind === 'table_structure') {
      database = c.database;
      table = c.table;
    } else if (c.kind === 'ddl_viewer') {
      database = c.database;
      table = c.objectName;
    }
    if (!database || !table) return;

    if (mode === 'data') {
      panelStore.replaceInFocused({ kind: 'table_browser', connectionId, database, table });
    } else if (mode === 'structure') {
      panelStore.replaceInFocused({ kind: 'table_structure', connectionId, database, table });
    } else {
      panelStore.replaceInFocused({
        kind: 'ddl_viewer',
        connectionId,
        database,
        objectName: table,
        objectType: 'table',
      });
    }
  }

  const _hasTableContext = $derived(
    focusedContent.kind === 'table_browser' ||
      focusedContent.kind === 'table_structure' ||
      focusedContent.kind === 'ddl_viewer',
  );

  function openSettings() {
    panelStore.openInFocused({ kind: 'settings' });
  }

  async function openSqliteFile(filePath: string) {
    const filename = filePath.split('/').pop() ?? filePath;
    const name = filename.replace(/\.(sqlite3?|db)$/i, '');
    try {
      const profile = await connectionsStore.create({
        name,
        dbType: 'sqlite',
        host: filePath,
        port: 0,
        database: '',
        username: '',
      });
      await connectionsStore.connect(profile.id);
    } catch (err) {
      toast.addToast(`Failed to open ${filename}: ${errorMessage(err)}`, 'error');
    }
  }

  async function openSqlFile(filePath: string) {
    let content: string;
    try {
      content = await readTextFile(filePath);
    } catch (err) {
      toast.addToast(`Failed to read SQL file: ${errorMessage(err)}`, 'error');
      return;
    }
    const focused = panelStore.focusedPanel.content;
    const connectionId =
      'connectionId' in focused
        ? focused.connectionId
        : ([...connectionsStore.activeIds][0] ?? null);
    if (connectionId) {
      panelStore.openInFocused({ kind: 'query_editor', connectionId, initialSql: content });
    } else {
      toast.addToast('Connect to a database to open SQL files.', 'info');
    }
  }

  async function handleFileOpen(filePath: string) {
    const ext = filePath.split('.').pop()?.toLowerCase();
    if (ext === 'sqlite' || ext === 'db' || ext === 'sqlite3') {
      await openSqliteFile(filePath);
    } else if (ext === 'sql') {
      await openSqlFile(filePath);
    }
  }

  interface UpdateInfo {
    version: string;
    notes: string | null;
  }
  let pendingUpdate = $state<UpdateInfo | null>(null);
  let updateDismissed = $state(false);
  let installing = $state(false);

  onMount(() => {
    shortcutsStore.load(settings.shortcutPreset);

    const stored = localStorage.getItem('rowmance:pending-release-notes');
    if (stored) {
      localStorage.removeItem('rowmance:pending-release-notes');
      try {
        const { version } = JSON.parse(stored) as { version: string };
        if (version) {
          fetch(`https://api.github.com/repos/joe-herbert/rowmance/releases/tags/v${version}`, {
            headers: { Accept: 'application/vnd.github+json' },
          })
            .then((r) => (r.ok ? r.json() : null))
            .then((data: { body?: string } | null) => {
              const notes = data?.body?.trim() || '';
              panelStore.openInFocused({ kind: 'release_notes', version, notes });
            })
            .catch(() => {
              panelStore.openInFocused({ kind: 'release_notes', version, notes: '' });
            });
        }
      } catch {
        /* ignore malformed entry */
      }
    }

    function runUpdateCheck() {
      updaterApi
        .updaterCheck()
        .then((result) => {
          if (result.available && result.version) {
            pendingUpdate = { version: result.version, notes: result.notes };
          }
        })
        .catch(() => {});
    }

    let updateIntervalId: ReturnType<typeof setInterval> | undefined;
    if (settings.autoUpdateCheck) {
      runUpdateCheck();
      updateIntervalId = setInterval(runUpdateCheck, 60 * 60 * 1000);
    }

    let unlistenFn: (() => void) | undefined;
    Promise.all([
      listen('menu:open-settings', openSettings),
      listen('menu:new-query', () => {
        const focused = panelStore.focusedPanel.content;
        const connectionId = 'connectionId' in focused ? focused.connectionId : null;
        if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
      }),
      listen('menu:new-window', () => openNewWindow()),
      listen('menu:toggle-left-sidebar', () => toggleLeftSidebar()),
      listen('menu:toggle-right-sidebar', () => toggleRightSidebar()),
      listen('menu:toggle-system-items', () =>
        settingsStore.set('showSystemItems', !settingsStore.settings.showSystemItems),
      ),
      listen('menu:command-palette', () => openPalette()),
      listen('menu:check-updates', async () => {
        try {
          const result = await updaterApi.updaterCheck();
          if (result.available && result.version) {
            pendingUpdate = { version: result.version, notes: result.notes };
            updateDismissed = false;
          } else {
            toast.addToast('Rowmance is up to date.', 'success');
          }
        } catch {
          toast.addToast('Update check failed. Please try again later.', 'error');
        }
      }),
      listen('menu:open-file', async () => {
        const filePath = await openFileDialog({
          multiple: false,
          filters: [
            { name: 'Database & SQL Files', extensions: ['sqlite', 'db', 'sqlite3', 'sql'] },
          ],
        });
        if (filePath && typeof filePath === 'string') {
          await handleFileOpen(filePath);
        }
      }),
      listen('file:opened', async (event) => {
        const paths = event.payload as string[];
        for (const p of paths) {
          await handleFileOpen(p);
        }
      }),
      listen('menu:import-csv', () => document.dispatchEvent(new CustomEvent('menu-import-csv'))),
      listen('menu:import-sql', () => document.dispatchEvent(new CustomEvent('menu-import-sql'))),
      listen('menu:speed-analysis', () => panelStore.openInFocused({ kind: 'speed_analysis' })),
      listen('menu:whats-new', async () => {
        const version = await getVersion();
        fetch(`https://api.github.com/repos/joe-herbert/rowmance/releases/tags/v${version}`, {
          headers: { Accept: 'application/vnd.github+json' },
        })
          .then((r) => (r.ok ? r.json() : null))
          .then((data: { body?: string } | null) => {
            const notes = data?.body?.trim() || '';
            panelStore.openInFocused({ kind: 'release_notes', version, notes });
          })
          .catch(() => {
            panelStore.openInFocused({ kind: 'release_notes', version, notes: '' });
          });
      }),
      listen('menu:split-right', () =>
        panelStore.splitFocused('right', settings.maxHorizontalSplits, settings.maxVerticalSplits),
      ),
      listen('menu:split-down', () =>
        panelStore.splitFocused('down', settings.maxHorizontalSplits, settings.maxVerticalSplits),
      ),
      listen('menu:split-close', () => panelStore.closeSplit(panelStore.focusedSplitId)),
    ]).then((unlisteners) => {
      unlistenFn = () => unlisteners.forEach((u) => u());
    });

    window.addEventListener('resize', syncTrafficLightPosition);

    return () => {
      unlistenFn?.();
      window.removeEventListener('resize', syncTrafficLightPosition);
      clearInterval(updateIntervalId);
    };
  });

  async function installUpdate() {
    installing = true;
    try {
      if (pendingUpdate) {
        localStorage.setItem(
          'rowmance:pending-release-notes',
          JSON.stringify({ version: pendingUpdate.version }),
        );
      }
      await updaterApi.updaterInstall();
    } catch {
      localStorage.removeItem('rowmance:pending-release-notes');
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

  function onLeftResizeKeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 20 : 5;
    if (e.key === 'ArrowRight') {
      e.preventDefault();
      leftWidth = Math.min(500, leftWidth + step);
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      leftWidth = Math.max(160, leftWidth - step);
    }
  }

  function onRightResizeKeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 20 : 5;
    if (e.key === 'ArrowLeft') {
      e.preventDefault();
      rightWidth = Math.min(480, rightWidth + step);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      rightWidth = Math.max(200, rightWidth - step);
    }
  }

  function toggleLeftSidebar() {
    leftVisible = !leftVisible;
    settingsStore.set('leftSidebarVisible', leftVisible);
  }

  function toggleRightSidebar() {
    const wasHidden = !rightVisible;
    rightVisible = !rightVisible;
    settingsStore.set('rightSidebarVisible', rightVisible);
    if (wasHidden) {
      requestAnimationFrame(() => document.dispatchEvent(new CustomEvent('focus-right-sidebar')));
    }
  }

  function handleShortcutAction(e: Event) {
    const action = (e as CustomEvent<{ action: string }>).detail.action;
    if (action === 'TOGGLE_LEFT_SIDEBAR') toggleLeftSidebar();
    if (action === 'TOGGLE_RIGHT_SIDEBAR') toggleRightSidebar();
    if (action === 'NEW_WINDOW') openNewWindow();
    if (action === 'OPEN_SETTINGS') openSettings();
    if (action === 'COMMAND_PALETTE') openPalette();
    if (action === 'TOGGLE_SYSTEM_ITEMS')
      settingsStore.set('showSystemItems', !settingsStore.settings.showSystemItems);
    if (action === 'PANEL_CLOSE') {
      const focused = panelStore.focusedPanel.content;
      const dKey = dirtyKeyForContent(focused);
      if (dKey && panelStore.isItemDirty(dKey)) {
        confirmCloseFocused = true;
        confirmCloseFocusedKey = dKey;
      } else {
        panelStore.closeFocusedItem();
      }
    }
    if (action === 'PANEL_NEXT' || action === 'SPLIT_FOCUS_NEXT') panelStore.focusNext();
    if (action === 'PANEL_PREV' || action === 'SPLIT_FOCUS_PREV') panelStore.focusPrev();
    if (action === 'SPLIT_RIGHT') {
      panelStore.splitFocused('right', settings.maxHorizontalSplits, settings.maxVerticalSplits);
    }
    if (action === 'SPLIT_DOWN') {
      panelStore.splitFocused('down', settings.maxHorizontalSplits, settings.maxVerticalSplits);
    }
    if (action === 'SPLIT_CLOSE') {
      panelStore.closeSplit(panelStore.focusedSplitId);
    }
    if (action === 'CLOSE_OTHER_TABS') {
      const focusedContent = panelStore.focusedPanel.content;
      const splitItems = panelStore.getSplitItems(panelStore.focusedSplitId);
      const focusedItem = splitItems.find((i) => sameContent(i.content, focusedContent));
      if (focusedItem) panelStore.closeOtherItems(focusedItem.id);
    }
    if (action === 'NEW_QUERY_EDITOR') {
      const focused = panelStore.focusedPanel.content;
      const connectionId = 'connectionId' in focused ? focused.connectionId : null;
      if (connectionId) panelStore.openInFocused({ kind: 'query_editor', connectionId });
    }
    if (action === 'GLOBAL_SEARCH') openGlobalSearch();
    if (action.startsWith('TAB_')) {
      const n = parseInt(action.slice(4), 10);
      const items = panelStore.getSplitItems(panelStore.focusedSplitId);
      const target = n === 9 ? items[items.length - 1] : items[n - 1];
      if (target) panelStore.showItem(target);
    }
    if (action === 'FOCUS_SCHEMA_TREE') {
      if (!leftVisible) {
        leftVisible = true;
        settingsStore.set('leftSidebarVisible', true);
        requestAnimationFrame(() => document.dispatchEvent(new CustomEvent('focus-schema-tree')));
      } else {
        document.dispatchEvent(new CustomEvent('focus-schema-tree'));
      }
    }
    if (action === 'TOGGLE_READ_ONLY' && activeConnection)
      connectionsStore.toggleReadOnly(activeConnection.id).catch(() => {});
  }

  let paletteOpen = $state(false);
  let globalSearchOpen = $state(false);
  let confirmCloseFocused = $state(false);
  let confirmCloseFocusedKey = $state<string | null>(null);

  // ── Connection chip popup ─────────────────────────────────────────────────

  let connChipOpen = $state(false);
  let connChipEl = $state<HTMLElement | null>(null);
  let connPopupEl = $state<HTMLElement | null>(null);
  let popupX = $state(0);
  let popupY = $state(0);
  let elapsedDisplay = $state('');
  let disconnecting = $state(false);

  function toggleConnChip() {
    if (!connChipOpen && connChipEl) {
      const rect = connChipEl.getBoundingClientRect();
      popupX = rect.left;
      popupY = rect.bottom + 6;
    }
    connChipOpen = !connChipOpen;
  }

  function formatElapsed(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000);
    const h = Math.floor(totalSeconds / 3600);
    const m = Math.floor((totalSeconds % 3600) / 60);
    const s = totalSeconds % 60;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
  }

  $effect(() => {
    if (!activeConnection) connChipOpen = false;
  });

  $effect(() => {
    if (!connChipOpen || !activeConnection || !isConnected) return;
    const id = activeConnection.id;

    function update() {
      const start = connectionsStore.getConnectedAt(id);
      elapsedDisplay = start ? formatElapsed(Date.now() - start.getTime()) : '';
    }

    update();
    const timer = setInterval(update, 1000);
    return () => clearInterval(timer);
  });

  $effect(() => {
    if (!connChipOpen) return;

    function handleClickOutside(e: MouseEvent) {
      const target = e.target as Node;
      if (
        connChipEl &&
        !connChipEl.contains(target) &&
        connPopupEl &&
        !connPopupEl.contains(target)
      ) {
        connChipOpen = false;
      }
    }

    setTimeout(() => document.addEventListener('click', handleClickOutside), 0);
    return () => document.removeEventListener('click', handleClickOutside);
  });

  async function handleDisconnect() {
    if (!activeConnection) return;
    disconnecting = true;
    const id = activeConnection.id;
    try {
      await connectionsStore.disconnect(id);
      panelStore.closeItemsForConnection(id, { skipDirty: true });
    } finally {
      disconnecting = false;
      connChipOpen = false;
    }
  }

  let connecting = $state(false);

  async function handleConnect() {
    if (!activeConnection) return;
    connecting = true;
    try {
      await connectionsStore.connect(activeConnection.id);
    } finally {
      connecting = false;
      connChipOpen = false;
    }
  }

  // ── Transaction controls (in connection popup) ────────────────────────────

  let txBusy = $state(false);
  let txQueriesExpanded = $state(false);

  async function handleBeginTransaction() {
    if (!activeConnection) return;
    const id = activeConnection.id;
    txBusy = true;
    try {
      const c = focusedContent;
      let database: string | undefined;
      if ('database' in c && typeof c.database === 'string') {
        database = c.database;
      } else if (c.kind === 'query_editor' && c.editorId) {
        database = queryEditorCache.get(c.editorId)?.selectedDatabase || undefined;
      }
      await txApi.beginTransaction(id, database);
      connectionsStore.setTransactionActive(id, true, database);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    } finally {
      txBusy = false;
    }
  }

  async function handleCommitTransaction() {
    if (!activeConnection) return;
    const id = activeConnection.id;
    txBusy = true;
    try {
      await txApi.commitTransaction(id);
      connectionsStore.setTransactionActive(id, false);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    } finally {
      txBusy = false;
    }
  }

  async function handleRollbackTransaction() {
    if (!activeConnection) return;
    const id = activeConnection.id;
    txBusy = true;
    try {
      await txApi.rollbackTransaction(id);
      connectionsStore.setTransactionActive(id, false);
      txQueriesExpanded = false;
      document.dispatchEvent(new CustomEvent('tx-rollback', { detail: { connectionId: id } }));
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    } finally {
      txBusy = false;
    }
  }

  async function handleCommitTransactionWithReset() {
    await handleCommitTransaction();
    txQueriesExpanded = false;
  }

  function openTxQueryInEditor(sql: string) {
    if (!activeConnection) return;
    panelStore.openInFocused({ kind: 'query_editor', connectionId: activeConnection.id, initialSql: sql });
    connChipOpen = false;
  }

  function openAllTxQueriesInEditor() {
    if (!activeConnection) return;
    const queries = connectionsStore.getTxQueries(activeConnection.id);
    if (!queries.length) return;
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: activeConnection.id,
      initialSql: queries.join('\n\n'),
    });
    connChipOpen = false;
  }

  async function saveTxQueriesAsFile() {
    if (!activeConnection) return;
    const queries = connectionsStore.getTxQueries(activeConnection.id);
    if (!queries.length) return;
    const filePath = await saveDialog({
      defaultPath: 'transaction.sql',
      filters: [{ name: 'SQL', extensions: ['sql'] }],
    });
    if (!filePath) return;
    try {
      await writeTextFile(filePath, queries.join('\n\n'));
      toast.addToast('Saved transaction queries', 'success', 2000);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    }
  }

  function openPalette() {
    paletteOpen = true;
  }
  function closePalette() {
    paletteOpen = false;
  }
  function openGlobalSearch() {
    globalSearchOpen = true;
  }
  function closeGlobalSearch() {
    globalSearchOpen = false;
  }

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
  style="--sidebar-width: {leftVisible
    ? leftWidth + SIDEBAR_INSET
    : 0}px; --right-sidebar-width: {rightWidth}px;"
>
  <div class="titlebar-card" data-tauri-drag-region>
    {#if isMacOS}
      <div class="traffic-lights-spacer" aria-hidden="true"></div>
    {/if}

    {#if activeConnection}
      {@const txActiveInBar = connectionsStore.isTransactionActive(activeConnection.id)}
      <div class="conn-chip-wrapper" bind:this={connChipEl} data-tauri-drag-region="false">
        <button
          class="conn-chip"
          class:conn-chip--open={connChipOpen}
          style="border-left-color: {activeConnection.color ?? 'var(--color-accent)'}"
          onclick={toggleConnChip}
          aria-expanded={connChipOpen}
          aria-haspopup="true"
        >
          <span
            class="conn-chip-dot"
            style="background: {activeConnection.color ??
              'var(--color-accent)'}; box-shadow: 0 0 0 3px color-mix(in srgb, {activeConnection.color ??
              'var(--color-accent)'} 26%, transparent)"
          ></span>
          <span class="conn-chip-name">{activeConnection.name}</span>
          <span class="conn-chip-tag">{activeConnection.dbType}</span>
          {#if txActiveInBar}
            <span class="conn-chip-tx-badge" aria-label="Transaction active">TX</span>
          {/if}
          {#if recordingStore.isRecordingConnection(activeConnection.id)}
            <span class="conn-chip-rec-dot" class:conn-chip-rec-dot--paused={recordingStore.isPaused} aria-label="Recording active"></span>
          {/if}
          <svg
            class="conn-chip-chevron"
            width="10"
            height="10"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.2"
            stroke-linecap="round"
            aria-hidden="true"
          >
            <polyline points="6 9 12 15 18 9"></polyline>
          </svg>
        </button>
        <button
          class="read-only-toggle"
          class:read-only-toggle--on={activeConnection.readOnly}
          onclick={() => connectionsStore.toggleReadOnly(activeConnection!.id)}
          title={activeConnection.readOnly
            ? 'Read-only on — click to disable'
            : 'Read-only off — click to enable'}
          aria-label={activeConnection.readOnly
            ? 'Disable read-only mode'
            : 'Enable read-only mode'}
          aria-pressed={activeConnection.readOnly}
        >
          {#if activeConnection.readOnly}
            <!-- Locked padlock -->
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <rect x="5" y="11" width="14" height="9" rx="2"></rect>
              <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
            </svg>
          {:else}
            <!-- Unlocked padlock -->
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <rect x="5" y="11" width="14" height="9" rx="2"></rect>
              <path d="M8 11V8a4 4 0 0 1 7.5-3.9"></path>
            </svg>
          {/if}
        </button>
      </div>
    {:else}
      <span class="app-title">Rowmance</span>
    {/if}

    {#if activeView !== null}
      <div class="view-switcher" data-tauri-drag-region="false" role="group" aria-label="View mode">
        <button
          class="view-btn"
          class:view-btn--active={activeView === 'data'}
          onclick={() => switchView('data')}
          title="Data view">Data</button
        >
        <button
          class="view-btn"
          class:view-btn--active={activeView === 'structure'}
          onclick={() => switchView('structure')}
          title="Structure view">Structure</button
        >
        <button
          class="view-btn"
          class:view-btn--active={activeView === 'sql'}
          onclick={() => switchView('sql')}
          title="DDL">SQL</button
        >
      </div>
    {/if}

    <div class="titlebar-spacer" data-tauri-drag-region></div>
    <button
      class="titlebar-btn"
      onclick={openGlobalSearch}
      title="Search (⌘F)"
      aria-label="Open global search"
    >
      <svg
        width="14"
        height="14"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.7"
        stroke-linecap="round"
      >
        <circle cx="11" cy="11" r="7"></circle>
        <line x1="21" y1="21" x2="16.5" y2="16.5"></line>
      </svg>
      <span class="titlebar-btn-label">Search</span>
      <kbd>⌘F</kbd>
    </button>
    <button
      class="titlebar-btn titlebar-btn--icon"
      onclick={openSettings}
      title="Settings"
      aria-label="Open settings"
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.7"
        stroke-linecap="round"
        stroke-linejoin="round"
      >
        <circle cx="12" cy="12" r="3"></circle>
        <path
          d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
        ></path>
      </svg>
    </button>
  </div>

  {#if pendingUpdate && !updateDismissed}
    <div class="update-banner" role="alert" aria-live="polite">
      <span class="update-message">
        Rowmance {pendingUpdate.version} is available
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
          aria-label="Dismiss update notification">Later</button
        >
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
      <aside
        class="left-sidebar"
        class:floating={settings.sidebarFloating}
        style="width: {leftWidth}px;"
        transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}
      >
        <Sidebar />
      </aside>
    {/if}

    <!-- Resize handle: left sidebar ↔ main area -->
    {#if leftVisible && !settings.sidebarFloating}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="resize-handle resize-handle--horizontal left-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize left sidebar"
        tabindex="0"
        style="left: {leftWidth}px;"
        onpointerdown={(e) => onResizePointerDown('left', e)}
        onkeydown={onLeftResizeKeydown}
        class:dragging={dragging === 'left'}
      ></div>
    {/if}

    <!-- Main split-panel area -->
    <main id="main-content" class="main-area">
      <div class="split-panel-wrapper">
        <SplitPanel />
      </div>
    </main>

    <!-- Resize handle: main area ↔ right sidebar -->
    {#if rightVisible && !settings.sidebarFloating}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="resize-handle resize-handle--horizontal right-resize"
        role="separator"
        aria-orientation="vertical"
        aria-label="Resize right sidebar"
        tabindex="0"
        style="right: {rightWidth}px;"
        onpointerdown={(e) => onResizePointerDown('right', e)}
        onkeydown={onRightResizeKeydown}
        class:dragging={dragging === 'right'}
      ></div>
    {/if}

    <!-- Right sidebar (toggleable) -->
    {#if rightVisible}
      <aside
        class="right-sidebar"
        class:floating={settings.sidebarFloating}
        style="width: {rightWidth}px;"
        transition:slide={{ axis: 'x', duration: 200, easing: cubicOut }}
      >
        <RightSidebar
          initialPanel={activeRightPanel}
          onPanelChange={(p) => {
            activeRightPanel = (p ?? 'history') as RightPanel;
            settingsStore.set('rightSidebarPanel', activeRightPanel);
          }}
        />
      </aside>
    {/if}
  </div>

  <div class="statusbar-row">
    <button
      class="sidebar-toggle-btn"
      class:sidebar-toggle-btn--active={leftVisible}
      onclick={toggleLeftSidebar}
      aria-label="{leftVisible ? 'Hide' : 'Show'} left sidebar"
      title="{leftVisible ? 'Hide' : 'Show'} left sidebar"
    >
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" aria-hidden="true">
        <rect x="1" y="1" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.2" />
        <line x1="5" y1="1" x2="5" y2="14" stroke="currentColor" stroke-width="1.2" />
        <rect x="1.6" y="1.6" width="2.8" height="11.8" rx="1" fill="currentColor" opacity="0.5" />
      </svg>
    </button>
    <StatusBar />
    <button
      class="sidebar-toggle-btn"
      class:sidebar-toggle-btn--active={rightVisible}
      onclick={toggleRightSidebar}
      aria-label="{rightVisible ? 'Hide' : 'Show'} right sidebar"
      title="{rightVisible ? 'Hide' : 'Show'} right sidebar"
    >
      <svg width="15" height="15" viewBox="0 0 15 15" fill="none" aria-hidden="true">
        <rect x="1" y="1" width="13" height="13" rx="2" stroke="currentColor" stroke-width="1.2" />
        <line x1="10" y1="1" x2="10" y2="14" stroke="currentColor" stroke-width="1.2" />
        <rect x="10.6" y="1.6" width="2.8" height="11.8" rx="1" fill="currentColor" opacity="0.5" />
      </svg>
    </button>
  </div>
</div>

{#if connChipOpen && activeConnection}
  <div
    class="conn-popup"
    bind:this={connPopupEl}
    role="dialog"
    aria-label="Connection details"
    style="left: {popupX}px; top: {popupY}px;"
    transition:fade={{ duration: 100 }}
  >
    <div class="conn-popup-row">
      <span
        class="conn-popup-dot"
        style="background: {activeConnection.color ?? 'var(--color-accent)'}"
      ></span>
      <span class="conn-popup-name">{activeConnection.name}</span>
      <span class="conn-chip-tag">{activeConnection.dbType}</span>
    </div>
    <div class="conn-popup-time">
      <svg
        width="12"
        height="12"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="10"></circle>
        <polyline points="12 6 12 12 16 14"></polyline>
      </svg>
      {#if isConnected}
        Connected {elapsedDisplay}
      {:else}
        Disconnected
      {/if}
    </div>

    {#if isConnected}
      {@const txActive = connectionsStore.isTransactionActive(activeConnection.id)}
      <div class="tx-section" class:tx-section--active={txActive}>
        <div class="tx-section-row">
          <div class="tx-section-header">
            <span class="tx-indicator" class:tx-indicator--active={txActive}></span>
            <span class="tx-section-label">
              {txActive ? 'Transaction active' : 'Transaction'}
            </span>
          </div>
          <div class="tx-section-actions">
          {#if !txActive}
            <button class="tx-btn" onclick={handleBeginTransaction} disabled={txBusy}>
              Start
            </button>
          {:else}
            <button
              class="tx-btn tx-btn--commit"
              onclick={handleCommitTransactionWithReset}
              disabled={txBusy}
            >
              Commit
            </button>
            <button
              class="tx-btn tx-btn--rollback"
              onclick={handleRollbackTransaction}
              disabled={txBusy}
            >
              Rollback
            </button>
          {/if}
          </div>
        </div>
        {#if txActive}
          {@const txQueryList = connectionsStore.getTxQueries(activeConnection.id)}
          {#if txQueryList.length > 0}
            <div class="tx-queries">
              <button
                class="tx-queries-toggle"
                onclick={() => (txQueriesExpanded = !txQueriesExpanded)}
                aria-expanded={txQueriesExpanded}
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true" style="transform: rotate({txQueriesExpanded ? 180 : 0}deg); transition: transform 150ms">
                  <polyline points="6 9 12 15 18 9"></polyline>
                </svg>
                {txQueryList.length}
                {txQueryList.length === 1 ? 'query' : 'queries'} in transaction
              </button>
              {#if txQueriesExpanded}
                <div class="tx-queries-list" transition:slide={{ duration: 150 }}>
                  {#each txQueryList as sql, i (i)}
                    <div class="tx-query-item">
                      <div class="tx-query-scroll">
                        <span class="tx-query-sql">{sql}</span>
                      </div>
                      <button
                        class="tx-query-open"
                        onclick={() => openTxQueryInEditor(sql)}
                        title="Open in editor"
                        aria-label="Open query in editor"
                      >
                        <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                          <polyline points="16 18 22 12 16 6"></polyline>
                          <polyline points="8 6 2 12 8 18"></polyline>
                          <line x1="14" y1="5" x2="10" y2="19"></line>
                        </svg>
                      </button>
                    </div>
                  {/each}
                  <div class="tx-queries-actions">
                    <button class="tx-queries-action-btn" onclick={openAllTxQueriesInEditor}>Open all in editor</button>
                    <button class="tx-queries-action-btn" onclick={saveTxQueriesAsFile}>Save as SQL</button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
    {/if}

    {#if isConnected}
      {@const recActive = recordingStore.isRecordingConnection(activeConnection.id)}
      {@const recPaused = recActive && recordingStore.isPaused}
      <div class="tx-section" class:tx-section--recording={recActive && !recPaused} class:tx-section--rec-paused={recPaused}>
        <div class="tx-section-row">
          <div class="tx-section-header">
            <span class="rec-indicator" class:rec-indicator--active={recActive && !recPaused} class:rec-indicator--paused={recPaused}></span>
            <span class="tx-section-label">
              {#if recPaused}
                Recording paused
              {:else if recActive}
                Recording · {recordingStore.statements.length} {recordingStore.statements.length === 1 ? 'statement' : 'statements'}
              {:else}
                Recording
              {/if}
            </span>
          </div>
          <div class="tx-section-actions">
            {#if !recActive}
              <button class="tx-btn" onclick={(e) => { e.stopPropagation(); recordingStore.start(activeConnection.id); }}>
                Start
              </button>
            {:else if recPaused}
              <button class="tx-btn tx-btn--commit" onclick={(e) => { e.stopPropagation(); recordingStore.resume(); }}>
                Resume
              </button>
              <button class="tx-btn tx-btn--rollback" onclick={(e) => { e.stopPropagation(); recordingStore.stop(); }}>
                Stop
              </button>
            {:else}
              <button class="tx-btn" onclick={(e) => { e.stopPropagation(); recordingStore.pause(); }}>
                Pause
              </button>
              <button class="tx-btn tx-btn--rollback" onclick={(e) => { e.stopPropagation(); recordingStore.stop(); }}>
                Stop
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if isConnected}
      {@const rvActive = revertStore.isRevertingConnection(activeConnection.id)}
      {@const rvPaused = rvActive && revertStore.isPaused}
      <div class="tx-section" class:tx-section--revert={rvActive && !rvPaused} class:tx-section--revert-paused={rvPaused}>
        <div class="tx-section-row">
          <div class="tx-section-header">
            <span class="revert-indicator" class:revert-indicator--active={rvActive && !rvPaused} class:revert-indicator--paused={rvPaused}></span>
            <span class="tx-section-label">
              {#if rvPaused}
                Revert paused · {revertStore.entries.length} {revertStore.entries.length === 1 ? 'change' : 'changes'}
              {:else if rvActive}
                Revert · {revertStore.entries.length} {revertStore.entries.length === 1 ? 'change' : 'changes'}
              {:else}
                Revert Mode
              {/if}
            </span>
          </div>
          <div class="tx-section-actions">
            {#if !rvActive}
              <button class="tx-btn" onclick={(e) => { e.stopPropagation(); revertStore.start(activeConnection.id); }}>
                Start
              </button>
            {:else if rvPaused}
              <button class="tx-btn tx-btn--commit" onclick={(e) => { e.stopPropagation(); revertStore.resume(); }}>
                Resume
              </button>
              <button class="tx-btn tx-btn--rollback" onclick={(e) => { e.stopPropagation(); revertStore.stop(); }}>
                Stop
              </button>
            {:else}
              <button class="tx-btn" onclick={(e) => { e.stopPropagation(); revertStore.pause(); }}>
                Pause
              </button>
              <button class="tx-btn tx-btn--rollback" onclick={(e) => { e.stopPropagation(); revertStore.stop(); }}>
                Stop
              </button>
            {/if}
            {#if rvActive && revertStore.entries.length > 0}
              <button class="tx-btn" onclick={(e) => { e.stopPropagation(); revertStore.openModal(); }}>
                View
              </button>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if isConnected}
      <button class="conn-popup-disconnect" onclick={handleDisconnect} disabled={disconnecting}>
        {disconnecting ? 'Disconnecting…' : 'Disconnect'}
      </button>
    {:else}
      <button class="conn-popup-connect" onclick={handleConnect} disabled={connecting}>
        {connecting ? 'Connecting…' : 'Connect'}
      </button>
    {/if}
  </div>
{/if}

{#if confirmCloseFocused}
  <ConfirmDialog
    title="Close tab"
    message="This table has unsaved changes. Close anyway?"
    confirmText="Close"
    cancelText="Cancel"
    danger={true}
    onconfirm={() => {
      if (confirmCloseFocusedKey) {
        clearTablePendingState(confirmCloseFocusedKey);
        panelStore.setItemDirty(confirmCloseFocusedKey, false);
      }
      panelStore.closeFocusedItem();
      confirmCloseFocused = false;
      confirmCloseFocusedKey = null;
    }}
    oncancel={() => {
      confirmCloseFocused = false;
      confirmCloseFocusedKey = null;
    }}
  />
{/if}

<Toast />
<OnboardingTip />

{#if paletteOpen}
  <CommandPalette onclose={closePalette} />
{/if}

{#if globalSearchOpen}
  <GlobalSearch onclose={closeGlobalSearch} />
{/if}

{#if recordingStore.reviewOpen}
  <RecordingModal />
{/if}

{#if revertStore.modalOpen}
  <RevertModal />
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
    -webkit-user-select: none;
    user-select: none;
  }

  .titlebar-spacer {
    flex: 1;
  }

  /* ── Connection chip ───────────────────────────────────────────────────── */

  .conn-chip-wrapper {
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .conn-chip {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 8px 4px 8px;
    border-radius: var(--radius-lg);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-left-width: 3px;
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    -webkit-user-select: none;
    user-select: none;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .conn-chip:hover,
  .conn-chip--open {
    background: var(--color-bg-hover);
  }

  .conn-chip-chevron {
    color: var(--color-text-muted);
    flex-shrink: 0;
    transition: transform var(--transition-fast);
  }

  .conn-chip--open .conn-chip-chevron {
    transform: rotate(180deg);
  }

  .conn-chip-tx-badge {
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    letter-spacing: 0.05em;
    color: var(--color-warning, #f59e0b);
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 15%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 40%, transparent);
    border-radius: var(--radius-sm);
    padding: 0 4px;
    line-height: 16px;
    flex-shrink: 0;
  }

  .conn-chip-rec-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--color-danger, #ef4444);
    flex-shrink: 0;
    animation: rec-pulse 1.2s ease-in-out infinite;
  }

  .conn-chip-rec-dot--paused {
    animation: none;
    opacity: 0.5;
  }

  @keyframes rec-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  .tx-section {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
  }

  .tx-section-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .tx-section--active {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 8%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, var(--color-warning, #f59e0b) 35%, transparent);
  }

  .tx-section--recording {
    background: color-mix(in srgb, var(--color-danger, #ef4444) 8%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, var(--color-danger, #ef4444) 35%, transparent);
  }

  .tx-section--recording .tx-section-label {
    color: var(--color-danger, #ef4444);
    font-weight: var(--font-weight-medium);
  }

  .tx-section--rec-paused {
    background: color-mix(in srgb, var(--color-danger, #ef4444) 5%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, var(--color-danger, #ef4444) 20%, transparent);
  }

  .tx-section--rec-paused .tx-section-label {
    color: color-mix(in srgb, var(--color-danger, #ef4444) 70%, var(--color-text-muted));
    font-weight: var(--font-weight-medium);
  }

  .rec-indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-muted);
    opacity: 0.4;
  }

  .rec-indicator--active {
    background: var(--color-danger, #ef4444);
    opacity: 1;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-danger, #ef4444) 25%, transparent);
    animation: rec-indicator-pulse 1.2s ease-in-out infinite;
  }

  .rec-indicator--paused {
    background: var(--color-danger, #ef4444);
    opacity: 0.45;
  }

  @keyframes rec-indicator-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .tx-section--revert {
    background: color-mix(in srgb, #f59e0b 8%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, #f59e0b 35%, transparent);
  }

  .tx-section--revert .tx-section-label {
    color: #f59e0b;
    font-weight: var(--font-weight-medium);
  }

  .tx-section--revert-paused {
    background: color-mix(in srgb, #f59e0b 5%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, #f59e0b 20%, transparent);
  }

  .tx-section--revert-paused .tx-section-label {
    color: color-mix(in srgb, #f59e0b 70%, var(--color-text-muted));
    font-weight: var(--font-weight-medium);
  }

  .revert-indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-muted);
    opacity: 0.4;
  }

  .revert-indicator--active {
    background: #f59e0b;
    opacity: 1;
    box-shadow: 0 0 0 2px color-mix(in srgb, #f59e0b 25%, transparent);
    animation: revert-indicator-pulse 1.2s ease-in-out infinite;
  }

  .revert-indicator--paused {
    background: #f59e0b;
    opacity: 0.45;
  }

  @keyframes revert-indicator-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .tx-section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
  }

  .tx-indicator {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-muted);
    opacity: 0.4;
  }

  .tx-indicator--active {
    background: var(--color-warning, #f59e0b);
    opacity: 1;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-warning, #f59e0b) 25%, transparent);
  }

  .tx-section-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .tx-section--active .tx-section-label {
    color: var(--color-warning, #f59e0b);
    font-weight: var(--font-weight-medium);
  }

  .tx-section-actions {
    display: flex;
    gap: 5px;
  }

  .tx-btn {
    padding: 0 8px;
    height: 22px;
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
    white-space: nowrap;
  }

  .tx-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tx-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .tx-btn--commit {
    border-color: var(--color-success);
    color: var(--color-success);
  }

  .tx-btn--commit:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-success) 10%, transparent);
  }

  .tx-btn--rollback {
    border-color: var(--color-danger);
    color: var(--color-danger);
  }

  .tx-btn--rollback:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-danger) 10%, transparent);
  }

  .tx-queries {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 1px solid color-mix(in srgb, var(--color-border) 60%, transparent);
    overflow: hidden;
  }

  .tx-queries-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    background: none;
    border: none;
    padding: 4px 0;
    font-size: 11px;
    color: var(--color-text-secondary);
    cursor: pointer;
    text-align: left;
  }

  .tx-queries-toggle:hover {
    color: var(--color-text);
  }

  .tx-queries-list {
    margin-top: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 200px;
    overflow-y: auto;
  }

  .tx-query-item {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
  }

  .tx-query-scroll {
    flex: 1;
    overflow-x: auto;
    padding: 4px 6px;
    min-width: 0;
  }

  .tx-query-sql {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--color-text-secondary);
    white-space: pre;
  }

  .tx-query-open {
    flex-shrink: 0;
    background: none;
    border: none;
    padding: 4px 6px;
    color: var(--color-text-muted);
    cursor: pointer;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    border-left: 1px solid var(--color-border);
  }

  .tx-query-open:hover {
    color: var(--color-text);
    background: var(--color-bg-hover);
  }

  .tx-queries-actions {
    display: flex;
    gap: 6px;
    margin-top: 6px;
    padding-top: 6px;
    border-top: 1px solid var(--color-border);
  }

  .tx-queries-action-btn {
    flex: 1;
    background: none;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 4px 6px;
    font-size: 11px;
    color: var(--color-text-secondary);
    cursor: pointer;
    text-align: center;
  }

  .tx-queries-action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text);
  }

  .conn-popup {
    position: fixed;
    min-width: 220px;
    max-width: 320px;
    width: max-content;
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 200;
  }

  .conn-popup-row {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .conn-popup-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .conn-popup-name {
    font-weight: var(--font-weight-semibold);
    font-size: 12.5px;
    color: var(--color-text-primary);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conn-popup-time {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .conn-popup-disconnect {
    padding: 6px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
    align-self: stretch;
    text-align: center;
  }

  .conn-popup-disconnect:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-danger, #e53e3e) 10%, transparent);
    border-color: var(--color-danger, #e53e3e);
    color: var(--color-danger, #e53e3e);
  }

  .conn-popup-disconnect:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .conn-popup-connect {
    padding: 6px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
    align-self: stretch;
    text-align: center;
  }

  .conn-popup-connect:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 10%, transparent);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .conn-popup-connect:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .conn-chip-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .conn-chip-name {
    font-weight: var(--font-weight-semibold);
    font-size: 12.5px;
    color: var(--color-text-primary);
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conn-chip-tag {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: var(--font-weight-medium);
  }

  .read-only-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .read-only-toggle:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border);
    color: var(--color-text-secondary);
  }

  .read-only-toggle--on {
    color: var(--color-accent);
  }

  .read-only-toggle--on:hover {
    color: var(--color-accent);
  }

  /* ── View switcher ─────────────────────────────────────────────────────── */

  .view-switcher {
    display: flex;
    padding: 2px;
    border-radius: var(--radius-lg);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .view-btn {
    padding: 4px 12px;
    border: none;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    white-space: nowrap;
  }

  .view-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .view-btn--active {
    background: var(--color-bg-primary);
    color: var(--color-accent);
    font-weight: var(--font-weight-semibold);
  }

  .view-btn--active:hover {
    background: var(--color-bg-primary);
  }

  .view-btn:disabled {
    opacity: 0.35;
    cursor: default;
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
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
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

  .left-sidebar.floating {
    position: absolute;
    left: var(--panel-spacing);
    top: var(--panel-spacing);
    bottom: var(--panel-spacing);
    z-index: 15;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.18);
  }

  /* ── Main content area ─────────────────────────────────────────────────── */

  .main-area {
    flex: 1;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .split-panel-wrapper {
    flex: 1;
    min-height: 0;
    overflow: hidden;
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

  .right-sidebar.floating {
    position: absolute;
    right: var(--panel-spacing);
    top: var(--panel-spacing);
    bottom: var(--panel-spacing);
    z-index: 15;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.18);
  }

  /* ── Resize handles ─────────────────────────────────────────────────────── */

  .left-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    /* Minimum 10px so the handle is always grabbable even when --panel-spacing: 0 */
    width: max(10px, var(--panel-spacing));
    transform: translateX(-50%);
    z-index: 20;
    cursor: col-resize;
    background: transparent;
  }

  .right-resize {
    position: absolute;
    top: 0;
    bottom: 0;
    width: max(10px, var(--panel-spacing));
    transform: translateX(50%);
    z-index: 20;
    cursor: col-resize;
    background: transparent;
  }

  .left-resize::after,
  .right-resize::after {
    content: '';
    position: absolute;
    top: var(--panel-radius);
    bottom: var(--panel-radius);
    left: 50%;
    width: 2px;
    transform: translateX(-50%);
    background: var(--color-accent);
    border-radius: 1px;
    opacity: 0;
    transition: opacity var(--transition-fast);
  }

  .left-resize:hover::after,
  .left-resize.dragging::after,
  .right-resize:hover::after,
  .right-resize.dragging::after {
    opacity: 0.5;
  }

  /* ── Status bar row (status bar + sidebar toggles) ─────────────────────── */

  .statusbar-row {
    display: flex;
    align-items: stretch;
    gap: var(--panel-spacing);
    flex-shrink: 0;
  }

  .sidebar-toggle-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: var(--statusbar-height);
    background: var(--color-bg-primary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border);
    border-radius: var(--panel-radius);
    color: var(--color-text-muted);
    cursor: pointer;
    opacity: var(--panel-opacity);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .sidebar-toggle-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .sidebar-toggle-btn--active {
    color: var(--color-accent);
  }
</style>
