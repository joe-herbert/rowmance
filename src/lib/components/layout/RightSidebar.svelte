<!--
  RightSidebar — toggleable right panel with icon tabs.
  Only one panel is active at a time; clicking the active tab collapses the sidebar.
  Panels: Query History, Saved Queries.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useCellSelection } from '$lib/stores/cellSelection.svelte';
  import * as historyApi from '$lib/tauri/history';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import * as schemaApi from '$lib/tauri/schema';
  import type {
    QueryHistoryEntry,
    FileQuery,
    FileQueryFolder,
    ColumnInfo,
    IndexInfo,
    ForeignKeyInfo,
  } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import RelationsPanel from '$lib/components/relations/RelationsPanel.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import { savedQueriesInvalidator } from '$lib/stores/savedQueriesInvalidator.svelte';
  import { portal } from '$lib/actions/portal';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';

  type ActivePanel = 'history' | 'saved' | 'column' | 'table-info' | 'relations' | null;

  interface Props {
    initialPanel?: ActivePanel;
    onPanelChange?: (_panel: ActivePanel) => void;
  }

  const { initialPanel = 'history', onPanelChange }: Props = $props();

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const cellSelectionStore = useCellSelection();
  const toast = useToast();

  let activePanel = $state<ActivePanel>(untrack(() => initialPanel));

  function selectPanel(panel: ActivePanel) {
    activePanel = panel;
    onPanelChange?.(panel);
    if (panel === 'history') loadHistory();
    if (panel === 'saved') loadSavedQueries();
    if (panel === 'column') columnInspectorKey = null;
  }

  // ── Relations panel ───────────────────────────────────────────────────────

  // ── Column Inspector ─────────────────────────────────────────────────────────

  let columnInfoLoading = $state(false);
  let columnInfoData = $state<ColumnInfo | null>(null);
  let columnIndexes = $state<IndexInfo[]>([]);
  let columnForeignKeys = $state<ForeignKeyInfo[]>([]);
  let columnInspectorKey = $state<string | null>(null);

  $effect(() => {
    const sel = cellSelectionStore.current;
    if (activePanel !== 'column') return;
    const key = sel ? `${sel.connectionId}:${sel.database}:${sel.table}:${sel.columnName}` : null;
    if (key === columnInspectorKey) return;
    columnInspectorKey = key;

    if (!sel) {
      columnInfoData = null;
      columnIndexes = [];
      columnForeignKeys = [];
      return;
    }

    columnInfoLoading = true;
    Promise.all([
      schemaApi.listColumns(sel.connectionId, sel.database, sel.table),
      schemaApi.listIndexes(sel.connectionId, sel.database, sel.table),
      schemaApi.listForeignKeys(sel.connectionId, sel.database, sel.table),
    ])
      .then(([cols, idxs, fks]) => {
        columnInfoData = cols.find((c) => c.name === sel.columnName) ?? null;
        columnIndexes = idxs.filter((idx) => idx.columns.includes(sel.columnName));
        columnForeignKeys = fks.filter((fk) => fk.columns.includes(sel.columnName));
        columnInfoLoading = false;
      })
      .catch((err) => {
        toast.addToast(errorMessage(err), 'error', 0);
        columnInfoLoading = false;
      });
  });

  // ── Query History ─────────────────────────────────────────────────────────────

  let historyEntries = $state<QueryHistoryEntry[]>([]);
  let historyLoading = $state(false);
  let selectedHistoryConnectionId = $state<string>('');

  const activeProfiles = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );

  $effect(() => {
    // When active connections change, default to the first one.
    const ids = activeProfiles.map((p) => p.id);
    if (ids.length > 0 && !ids.includes(selectedHistoryConnectionId)) {
      selectedHistoryConnectionId = ids[0];
    }
  });

  async function loadHistory() {
    if (!selectedHistoryConnectionId) return;
    historyLoading = true;
    try {
      historyEntries = await historyApi.listHistory(selectedHistoryConnectionId, 100, 0);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      historyLoading = false;
    }
  }

  async function handleClearHistory() {
    if (!selectedHistoryConnectionId) return;
    await historyApi.clearHistory(selectedHistoryConnectionId);
    historyEntries = [];
  }

  function openHistoryEntry(entry: QueryHistoryEntry) {
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: entry.connectionId,
      initialSql: entry.sql,
    });
  }

  function formatDuration(us: number | null): string {
    if (us === null) return '';
    if (us >= 1_000_000) return `${(us / 1_000_000).toFixed(1)}s`;
    if (us >= 1_000) return `${(us / 1_000).toFixed(1)}ms`;
    return `${us}µs`;
  }

  function timeAgo(isoString: string): string {
    const diff = Date.now() - new Date(isoString).getTime();
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins}m ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  }

  function truncateSql(sql: string, max = 80): string {
    const cleaned = sql.replace(/\s+/g, ' ').trim();
    return cleaned.length > max ? cleaned.slice(0, max) + '…' : cleaned;
  }

  // Reload history when the connection selector changes.
  $effect(() => {
    if (activePanel === 'history' && selectedHistoryConnectionId) {
      loadHistory();
    }
  });

  // ── Saved Queries ─────────────────────────────────────────────────────────────

  let savedFolders = $state<FileQueryFolder[]>([]);
  let savedQueries = $state<FileQuery[]>([]);
  let savedLoading = $state(false);
  let savedLoaded = $state(false);
  let expandedFolders = $state<Set<string>>(new Set());

  // Inline new-folder input
  let newFolderName = $state('');
  let showNewFolder = $state(false);

  // Inline new-query input
  let newQueryName = $state('');
  let newQueryFolderId = $state<string | null>(null);
  let showNewQuery = $state(false);

  // Context menu
  interface SavedCtxMenu {
    x: number;
    y: number;
    kind: 'query' | 'folder';
    id: string;
    name: string;
  }
  let savedCtxMenu = $state<SavedCtxMenu | null>(null);

  // Connection assignment dialog
  let assigningQuery = $state<FileQuery | null>(null);
  let assignConnectionId = $state('');

  async function loadSavedQueries() {
    savedLoading = true;
    try {
      const result = await savedQueriesApi.fileListSavedQueries();
      savedFolders = result.folders;
      savedQueries = result.queries;
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      savedLoading = false;
      savedLoaded = true;
    }
  }

  function toggleFolder(folderId: string) {
    const next = new Set(expandedFolders);
    if (next.has(folderId)) next.delete(folderId);
    else next.add(folderId);
    expandedFolders = next;
  }

  function openFileQuery(query: FileQuery) {
    const existing = panelStore.openItems.find(
      (item) => item.content.kind === 'query_editor' && item.content.savedQueryId === query.id,
    );
    if (existing) {
      panelStore.showItem(existing);
      return;
    }
    const connId = query.connectionId ?? connectionStore.profiles[0]?.id ?? '';
    panelStore.openInFocused({
      kind: 'query_editor',
      connectionId: connId,
      database: query.database ?? undefined,
      initialSql: query.sql,
      savedQueryId: query.id,
      savedQueryName: query.name,
    });
  }

  function handleQueryClick(query: FileQuery) {
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    if (query.connectionStatus === 'unresolved') {
      assigningQuery = query;
      assignConnectionId = connectionStore.profiles[0]?.id ?? '';
      return;
    }
    openFileQuery(query);
  }

  async function confirmAssignConnection() {
    if (!assigningQuery || !assignConnectionId) return;
    const query = assigningQuery;
    try {
      if (query.fileConnectionId) {
        await savedQueriesApi.fileAssignConnection(query.fileConnectionId, assignConnectionId);
      }
      await loadSavedQueries();
      // Open with the newly assigned connection.
      const updated = savedQueries.find((q) => q.id === query.id);
      openFileQuery(
        updated ?? { ...query, connectionId: assignConnectionId, connectionStatus: 'resolved' },
      );
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      assigningQuery = null;
    }
  }

  function openQueryAnyway() {
    if (!assigningQuery) return;
    const query = assigningQuery;
    assigningQuery = null;
    openFileQuery({ ...query, connectionId: connectionStore.profiles[0]?.id ?? '' });
  }

  async function createFolder() {
    if (!newFolderName.trim()) return;
    try {
      await savedQueriesApi.fileCreateFolder(newFolderName.trim());
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
      return;
    }
    newFolderName = '';
    showNewFolder = false;
    await loadSavedQueries();
  }

  async function createQuery() {
    if (!newQueryName.trim()) return;
    const connectionId =
      connectionStore.profiles.find((p) => connectionStore.isActive(p.id))?.id ?? null;
    try {
      await savedQueriesApi.fileCreateSavedQuery({
        name: newQueryName.trim(),
        sql: '',
        folderId: newQueryFolderId,
        connectionId,
      });
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
      return;
    }
    newQueryName = '';
    showNewQuery = false;
    newQueryFolderId = null;
    await loadSavedQueries();
  }

  function showSavedCtxMenu(e: MouseEvent, kind: 'query' | 'folder', id: string, name: string) {
    e.preventDefault();
    savedCtxMenu = { x: e.clientX, y: e.clientY, kind, id, name };
  }

  let confirmDeleteQueryId = $state<string | null>(null);

  async function confirmDeleteQuery() {
    if (!confirmDeleteQueryId) return;
    const deletingId = confirmDeleteQueryId;
    const openTab = panelStore.openItems.find(
      (item) => item.content.kind === 'query_editor' && item.content.savedQueryId === deletingId,
    );
    if (openTab?.content.kind === 'query_editor' && openTab.content.editorId) {
      panelStore.updateQueryEditorMeta(openTab.content.editorId, { savedQueryId: undefined });
    }
    await savedQueriesApi.fileDeleteSavedQuery(deletingId);
    confirmDeleteQueryId = null;
    await loadSavedQueries();
  }

  let confirmDeleteFolderId = $state<string | null>(null);
  let confirmDeleteFolderName = $state('');

  async function confirmDeleteFolder() {
    if (!confirmDeleteFolderId) return;
    try {
      await savedQueriesApi.fileDeleteFolder(confirmDeleteFolderId);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    }
    confirmDeleteFolderId = null;
    await loadSavedQueries();
  }

  let renamingQueryId = $state<string | null>(null);
  let renameQueryValue = $state('');
  let renameQueryInputEl = $state<HTMLInputElement | undefined>(undefined);

  function startRenameQuery(id: string, currentName: string) {
    savedCtxMenu = null;
    renamingQueryId = id;
    renameQueryValue = currentName;
    requestAnimationFrame(() => {
      renameQueryInputEl?.focus();
      renameQueryInputEl?.select();
    });
  }

  async function commitRenameQuery(query: FileQuery) {
    if (!renameQueryValue.trim()) {
      renamingQueryId = null;
      return;
    }
    const name = renameQueryValue.trim();
    renamingQueryId = null;
    try {
      const updated = await savedQueriesApi.fileUpdateSavedQuery(query.id, {
        name,
        sql: query.sql,
        connectionId: query.fileConnectionId,
        folderId: query.folderId,
        database: query.database,
      });
      const open = panelStore.openItems.find(
        (item) => item.content.kind === 'query_editor' && item.content.savedQueryId === query.id,
      );
      if (open?.content.kind === 'query_editor' && open.content.editorId) {
        panelStore.updateQueryEditorMeta(open.content.editorId, {
          savedQueryName: name,
          savedQueryId: updated.id,
        });
      }
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    }
    await loadSavedQueries();
  }

  async function handleSavedCtxDuplicate() {
    if (!savedCtxMenu || savedCtxMenu.kind !== 'query') return;
    const original = savedQueries.find((q) => q.id === savedCtxMenu?.id);
    if (!original) return;
    savedCtxMenu = null;
    try {
      await savedQueriesApi.fileCreateSavedQuery({
        name: `${original.name} (copy)`,
        sql: original.sql,
        folderId: original.folderId,
        connectionId: original.fileConnectionId,
      });
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    }
    await loadSavedQueries();
  }

  function handleSavedCtxDelete() {
    if (!savedCtxMenu) return;
    if (savedCtxMenu.kind === 'query') {
      confirmDeleteQueryId = savedCtxMenu.id;
      savedCtxMenu = null;
    } else {
      confirmDeleteFolderId = savedCtxMenu.id;
      confirmDeleteFolderName = savedCtxMenu.name;
      savedCtxMenu = null;
    }
  }

  async function handleSavedCtxRenameFolder() {
    if (!savedCtxMenu || savedCtxMenu.kind !== 'folder') return;
    const { id, name } = savedCtxMenu;
    savedCtxMenu = null;
    const newName = prompt('Rename folder', name);
    if (!newName || newName === name) return;
    try {
      await savedQueriesApi.fileRenameFolder(id, newName.trim());
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    }
    await loadSavedQueries();
  }

  function closeSavedCtxMenu() {
    savedCtxMenu = null;
  }

  // Load saved queries when the panel first opens.
  $effect(() => {
    if (activePanel === 'saved' && !savedLoaded && !savedLoading) {
      loadSavedQueries();
    }
  });

  // Reload whenever a query is saved from the editor.
  $effect(() => {
    savedQueriesInvalidator.version;
    if (activePanel === 'saved') {
      untrack(() => loadSavedQueries());
    }
  });

  // Group queries by folderId.
  const queriesByFolder = $derived.by<Map<string | null, FileQuery[]>>(() => {
    const map = new Map<string | null, FileQuery[]>();
    for (const q of savedQueries) {
      const key = q.folderId;
      const arr = map.get(key) ?? [];
      arr.push(q);
      map.set(key, arr);
    }
    return map;
  });

  type TopLevelItem =
    | { kind: 'query'; item: FileQuery }
    | { kind: 'folder'; item: FileQueryFolder };

  const foldersByParent = $derived.by<Map<string | null, FileQueryFolder[]>>(() => {
    const map = new Map<string | null, FileQueryFolder[]>();
    for (const f of savedFolders) {
      const key = f.parentId;
      const arr = map.get(key) ?? [];
      arr.push(f);
      map.set(key, arr);
    }
    return map;
  });

  function childItems(parentId: string | null): TopLevelItem[] {
    const queries: TopLevelItem[] = (queriesByFolder.get(parentId) ?? []).map((q) => ({
      kind: 'query',
      item: q,
    }));
    const folders: TopLevelItem[] = (foldersByParent.get(parentId) ?? []).map((f) => ({
      kind: 'folder',
      item: f,
    }));
    return [...queries, ...folders].sort((a, b) =>
      a.item.position !== b.item.position
        ? a.item.position - b.item.position
        : a.item.name.localeCompare(b.item.name, undefined, { sensitivity: 'base' }),
    );
  }

  function baseName(id: string): string {
    return id.split('/').pop() ?? id;
  }

  function isDescendantOf(folderId: string, ancestorId: string): boolean {
    const folder = savedFolders.find((f) => f.id === folderId);
    if (!folder || folder.parentId === null) return false;
    if (folder.parentId === ancestorId) return true;
    return isDescendantOf(folder.parentId, ancestorId);
  }

  // ── Drag & Drop ───────────────────────────────────────────────────────────────

  interface DragState {
    kind: 'query' | 'folder';
    id: string;
  }

  type DropZone =
    | { type: 'into-folder'; folderId: string }
    | { type: 'before-query'; queryId: string; folderId: string | null }
    | { type: 'after-query'; queryId: string; folderId: string | null }
    | { type: 'before-folder'; folderId: string }
    | { type: 'after-folder'; folderId: string };

  let dragging = $state<DragState | null>(null);
  let isDragging = $state(false);
  let dropZone = $state<DropZone | null>(null);
  let dragStartY = 0;
  let suppressNextClick = false;

  $effect(() => {
    document.body.classList.toggle('is-dragging', isDragging);
    return () => document.body.classList.remove('is-dragging');
  });

  $effect(() => {
    if (!dragging) return;
    function onMove(e: PointerEvent) {
      if (!isDragging && Math.abs(e.clientY - dragStartY) > 4) isDragging = true;
      if (!isDragging) return;
      dropZone = detectDropZone(e.clientX, e.clientY);
    }
    function onUp() {
      const drag = dragging;
      const zone = dropZone;
      dragging = null;
      isDragging = false;
      dropZone = null;
      if (drag && zone) {
        suppressNextClick = true;
        void commitDrop(drag, zone);
      }
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function startDrag(e: PointerEvent, kind: 'query' | 'folder', id: string) {
    if (e.button !== 0) return;
    dragStartY = e.clientY;
    dragging = { kind, id };
  }

  function detectDropZone(x: number, y: number): DropZone | null {
    if (!dragging) return null;
    const el = document.elementFromPoint(x, y);
    if (!el) return null;
    const node = (el as HTMLElement).closest<HTMLElement>('[data-drop-type]');
    if (!node) return null;
    const dropType = node.dataset.dropType!;
    const rect = node.getBoundingClientRect();
    const relY = (y - rect.top) / rect.height;

    if (dragging.kind === 'query') {
      if (dropType === 'folder') {
        const folderId = node.dataset.folderId!;
        // Edge zones (top/bottom 25%) = before/after; middle = into folder.
        if (relY < 0.25) return { type: 'before-folder', folderId };
        if (relY > 0.75) return { type: 'after-folder', folderId };
        return { type: 'into-folder', folderId };
      }
      if (dropType === 'query') {
        const queryId = node.dataset.queryId!;
        if (queryId === dragging.id) return null;
        const folderId = node.dataset.queryFolderId || null;
        return relY < 0.5
          ? { type: 'before-query', queryId, folderId }
          : { type: 'after-query', queryId, folderId };
      }
    }

    if (dragging.kind === 'folder') {
      if (dropType === 'folder') {
        const folderId = node.dataset.folderId!;
        if (folderId === dragging.id) return null;
        if (isDescendantOf(folderId, dragging.id)) return null;
        if (relY < 0.25) return { type: 'before-folder', folderId };
        if (relY > 0.75) return { type: 'after-folder', folderId };
        return { type: 'into-folder', folderId };
      }
      if (dropType === 'query') {
        const queryId = node.dataset.queryId!;
        const queryFolderId = node.dataset.queryFolderId || null;
        return relY < 0.5
          ? { type: 'before-query', queryId, folderId: queryFolderId }
          : { type: 'after-query', queryId, folderId: queryFolderId };
      }
    }
    return null;
  }

  async function commitDrop(drag: DragState, zone: DropZone) {
    if (drag.kind === 'query') await commitQueryDrop(drag.id, zone);
    else await commitFolderDrop(drag.id, zone);
    await loadSavedQueries();
  }

  async function updateOpenPanelId(oldId: string, newId: string) {
    if (newId === oldId) return;
    const open = panelStore.openItems.find(
      (item) => item.content.kind === 'query_editor' && item.content.savedQueryId === oldId,
    );
    if (open?.content.kind === 'query_editor' && open.content.editorId) {
      panelStore.updateQueryEditorMeta(open.content.editorId, { savedQueryId: newId });
    }
  }

  async function commitQueryDrop(queryId: string, zone: DropZone) {
    const query = savedQueries.find((q) => q.id === queryId);
    if (!query) return;

    if (zone.type === 'into-folder') {
      try {
        const updated = await savedQueriesApi.fileUpdateSavedQuery(queryId, {
          name: query.name,
          sql: query.sql,
          connectionId: query.fileConnectionId,
          folderId: zone.folderId,
          database: query.database,
        });
        await updateOpenPanelId(queryId, updated.id);
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
      }
    } else if (zone.type === 'before-folder' || zone.type === 'after-folder') {
      const refFolder = savedFolders.find((f) => f.id === zone.folderId);
      if (!refFolder) return;
      const targetFolderId = refFolder.parentId;
      let effectiveQueryId = queryId;
      if (query.folderId !== targetFolderId) {
        try {
          const updated = await savedQueriesApi.fileUpdateSavedQuery(queryId, {
            name: query.name,
            sql: query.sql,
            connectionId: query.fileConnectionId,
            folderId: targetFolderId,
            database: query.database,
          });
          effectiveQueryId = updated.id;
          await updateOpenPanelId(queryId, updated.id);
        } catch (err) {
          toast.addToast(errorMessage(err), 'error', 0);
          return;
        }
        await loadSavedQueries();
      }
      const siblings = childItems(targetFolderId);
      const draggedIdx = siblings.findIndex(
        (i) => i.kind === 'query' && i.item.id === effectiveQueryId,
      );
      if (draggedIdx === -1) return;
      const [dragged] = siblings.splice(draggedIdx, 1);
      const refIdx = siblings.findIndex((i) => i.kind === 'folder' && i.item.id === zone.folderId);
      if (refIdx === -1) return;
      siblings.splice(zone.type === 'before-folder' ? refIdx : refIdx + 1, 0, dragged);
      try {
        await savedQueriesApi.fileUpdateOrder(
          targetFolderId,
          siblings.map((i) => baseName(i.item.id)),
        );
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
      }
    } else if (zone.type === 'before-query' || zone.type === 'after-query') {
      const targetFolderId = zone.folderId;
      let effectiveQueryId = queryId;

      if (query.folderId !== targetFolderId) {
        try {
          const updated = await savedQueriesApi.fileUpdateSavedQuery(queryId, {
            name: query.name,
            sql: query.sql,
            connectionId: query.fileConnectionId,
            folderId: targetFolderId,
            database: query.database,
          });
          effectiveQueryId = updated.id;
          await updateOpenPanelId(queryId, updated.id);
        } catch (err) {
          toast.addToast(errorMessage(err), 'error', 0);
          return;
        }
        await loadSavedQueries();
      }

      const siblings = childItems(targetFolderId);
      const draggedIdx = siblings.findIndex(
        (i) => i.kind === 'query' && i.item.id === effectiveQueryId,
      );
      if (draggedIdx === -1) return;
      const [dragged] = siblings.splice(draggedIdx, 1);
      const refIdx = siblings.findIndex((i) => i.kind === 'query' && i.item.id === zone.queryId);
      if (refIdx === -1) return;
      siblings.splice(zone.type === 'before-query' ? refIdx : refIdx + 1, 0, dragged);
      try {
        await savedQueriesApi.fileUpdateOrder(
          targetFolderId,
          siblings.map((i) => baseName(i.item.id)),
        );
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
      }
    }
  }

  async function commitFolderDrop(folderId: string, zone: DropZone) {
    if (zone.type === 'into-folder') {
      if (zone.folderId === folderId || isDescendantOf(zone.folderId, folderId)) return;
      try {
        await savedQueriesApi.fileMoveFolder(folderId, zone.folderId);
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
      }
      return;
    }

    if (
      zone.type !== 'before-folder' &&
      zone.type !== 'after-folder' &&
      zone.type !== 'before-query' &&
      zone.type !== 'after-query'
    )
      return;

    // Determine the parent context of the reference item.
    let parentFolderId: string | null;
    if (zone.type === 'before-folder' || zone.type === 'after-folder') {
      const refFolder = savedFolders.find((f) => f.id === zone.folderId);
      if (!refFolder) return;
      parentFolderId = refFolder.parentId;
    } else {
      const refQuery = savedQueries.find((q) => q.id === zone.queryId);
      if (!refQuery) return;
      parentFolderId = refQuery.folderId;
    }

    // If the folder needs to move to a different parent, do that first.
    const folder = savedFolders.find((f) => f.id === folderId);
    if (!folder) return;
    let effectiveFolderId = folderId;
    if (folder.parentId !== parentFolderId) {
      try {
        await savedQueriesApi.fileMoveFolder(folderId, parentFolderId);
      } catch (err) {
        toast.addToast(errorMessage(err), 'error', 0);
        return;
      }
      await loadSavedQueries();
      const folderBaseName = baseName(folderId);
      effectiveFolderId = parentFolderId ? `${parentFolderId}/${folderBaseName}` : folderBaseName;
    }

    const siblings = childItems(parentFolderId);
    const draggedIdx = siblings.findIndex(
      (i) => i.kind === 'folder' && i.item.id === effectiveFolderId,
    );
    if (draggedIdx === -1) return;
    const [dragged] = siblings.splice(draggedIdx, 1);
    const refIdx =
      zone.type === 'before-folder' || zone.type === 'after-folder'
        ? siblings.findIndex((i) => i.kind === 'folder' && i.item.id === zone.folderId)
        : siblings.findIndex((i) => i.kind === 'query' && i.item.id === zone.queryId);
    if (refIdx === -1) return;
    siblings.splice(
      zone.type === 'before-folder' || zone.type === 'before-query' ? refIdx : refIdx + 1,
      0,
      dragged,
    );
    try {
      await savedQueriesApi.fileUpdateOrder(
        parentFolderId,
        siblings.map((i) => baseName(i.item.id)),
      );
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    }
  }

  function handleTabStripKeydown(e: KeyboardEvent) {
    const tabs = Array.from(
      (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>('[role="tab"]'),
    );
    const idx = tabs.indexOf(document.activeElement as HTMLButtonElement);
    if (idx === -1) return;
    let next = -1;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = (idx + 1) % tabs.length;
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') next = (idx - 1 + tabs.length) % tabs.length;
    if (next !== -1) {
      e.preventDefault();
      tabs[next].focus();
      tabs[next].click();
    }
  }

  function handleFolderToggle(folderId: string) {
    if (suppressNextClick) {
      suppressNextClick = false;
      return;
    }
    toggleFolder(folderId);
  }

  $effect(() => {
    function onFocusRightSidebar() {
      const activeTab = document.querySelector<HTMLButtonElement>(
        '.right-sidebar [role="tab"][aria-selected="true"], .right-sidebar [role="tab"].active',
      );
      (activeTab ?? document.querySelector<HTMLButtonElement>('.right-sidebar [role="tab"]'))?.focus();
    }
    document.addEventListener('focus-right-sidebar', onFocusRightSidebar);
    return () => document.removeEventListener('focus-right-sidebar', onFocusRightSidebar);
  });
</script>

<div class="right-sidebar">
  <!-- Icon tab strip -->
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div class="tab-strip" role="tablist" aria-label="Right sidebar panels" onkeydown={handleTabStripKeydown}>
    <div class="spacer"></div>
    <button
      class="tab-btn"
      class:active={activePanel === 'history'}
      role="tab"
      aria-selected={activePanel === 'history'}
      aria-controls="panel-history"
      title="Query History"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('history');
      }}
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        ><circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" /></svg
      >
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'saved'}
      role="tab"
      aria-selected={activePanel === 'saved'}
      aria-controls="panel-saved"
      title="Saved Queries"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('saved');
      }}
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"><path d="M19 21l-7-5-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z" /></svg
      >
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'column'}
      role="tab"
      aria-selected={activePanel === 'column'}
      aria-controls="panel-column"
      title="Column Inspector"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('column');
      }}
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        ><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /><line
          x1="8"
          y1="11"
          x2="14"
          y2="11"
        /><line x1="11" y1="8" x2="11" y2="14" /></svg
      >
    </button>

    <button
      class="tab-btn"
      class:active={activePanel === 'relations'}
      role="tab"
      aria-selected={activePanel === 'relations'}
      aria-controls="panel-relations"
      title="Relations"
      onclick={(e) => {
        e.stopPropagation();
        selectPanel('relations');
      }}
    >
      <svg
        width="15"
        height="15"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
        ><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path
          d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"
        /></svg
      >
    </button>
  </div>

  <!-- Panel content -->
  <div class="panel-content">
    {#if activePanel === 'history'}
      <div id="panel-history" role="tabpanel" aria-label="Query History">
        <div class="panel-toolbar">
          {#if activeProfiles.length > 1}
            <Select
              bind:value={selectedHistoryConnectionId}
              options={activeProfiles.map((p) => ({ value: p.id, label: p.name }))}
              aria-label="Select connection"
              size="xs"
            />
          {:else if activeProfiles.length === 1}
            <span class="connection-label">{activeProfiles[0].name}</span>
          {:else}
            <span class="muted">No active connections</span>
          {/if}

          <div class="toolbar-gap"></div>

          {#if historyEntries.length > 0}
            <button
              class="action-btn danger-btn"
              onclick={handleClearHistory}
              title="Clear history"
            >
              Clear
            </button>
          {/if}
        </div>

        {#if historyLoading}
          <div class="loading-row">Loading…</div>
        {:else if historyEntries.length === 0}
          <div class="empty-row">No history yet.</div>
        {:else}
          <ul class="history-list" role="list">
            {#each historyEntries as entry (entry.id)}
              <li class="history-item" role="listitem">
                <button
                  class="history-btn"
                  onclick={() => openHistoryEntry(entry)}
                  title="Open in editor"
                >
                  <span class="history-sql">{truncateSql(entry.sql)}</span>
                  <span class="history-meta">
                    <span
                      class="status-badge"
                      class:success={entry.status === 'success'}
                      class:error={entry.status === 'error'}
                    >
                      {entry.status}
                    </span>
                    {#if entry.durationUs !== null}
                      <span class="meta-item">{formatDuration(entry.durationUs)}</span>
                    {/if}
                    <span class="meta-item time-ago">{timeAgo(entry.executedAt)}</span>
                  </span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else if activePanel === 'saved'}
      <div id="panel-saved" role="tabpanel" aria-label="Saved Queries">
        <div class="panel-toolbar">
          <span class="panel-title">Saved Queries</span>
          <div class="toolbar-gap"></div>
          <button
            class="icon-btn"
            onclick={() => {
              showNewFolder = !showNewFolder;
            }}
            title="New folder"
            aria-label="New folder"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.9"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <path
                d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
              />
              <line x1="12" y1="11" x2="12" y2="17" />
              <line x1="9" y1="14" x2="15" y2="14" />
            </svg>
          </button>
          <button
            class="icon-btn"
            onclick={() => {
              showNewQuery = !showNewQuery;
            }}
            title="New query"
            aria-label="New query"
          >
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              stroke-width="1.9"
              stroke-linecap="round"
              stroke-linejoin="round"
              aria-hidden="true"
            >
              <line x1="12" y1="5" x2="12" y2="19" />
              <line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
        </div>

        {#if showNewFolder}
          <div class="inline-input-row">
            <input
              class="inline-input"
              type="text"
              placeholder="Folder name…"
              bind:value={newFolderName}
              onkeydown={(e) => {
                if (e.key === 'Enter') createFolder();
                if (e.key === 'Escape') {
                  showNewFolder = false;
                }
              }}
              aria-label="New folder name"
            />
            <button class="action-btn" onclick={createFolder}>Add</button>
          </div>
        {/if}

        {#if showNewQuery}
          <div class="inline-input-row">
            <input
              class="inline-input"
              type="text"
              placeholder="Query name…"
              bind:value={newQueryName}
              onkeydown={(e) => {
                if (e.key === 'Enter') createQuery();
                if (e.key === 'Escape') {
                  showNewQuery = false;
                }
              }}
              aria-label="New query name"
            />
            <button class="action-btn" onclick={createQuery}>Add</button>
          </div>
        {/if}

        {#if savedLoading}
          <div class="loading-row">Loading…</div>
        {:else if savedFolders.length === 0 && savedQueries.length === 0}
          <div class="empty-row">No saved queries yet.</div>
        {:else}
          {#snippet queryRow(query: FileQuery, folderId: string | null)}
            <li
              class="query-node"
              class:drop-before={dropZone?.type === 'before-query' && dropZone.queryId === query.id}
              class:drop-after={dropZone?.type === 'after-query' && dropZone.queryId === query.id}
              class:is-dragging={isDragging && dragging?.id === query.id}
              data-drop-type="query"
              data-query-id={query.id}
              data-query-folder-id={folderId ?? ''}
              role="treeitem"
              aria-selected={false}
            >
              {#if renamingQueryId === query.id}
                <input
                  bind:this={renameQueryInputEl}
                  bind:value={renameQueryValue}
                  class="query-rename-input"
                  type="text"
                  maxlength="120"
                  autocomplete="off"
                  spellcheck={false}
                  onclick={(e) => e.stopPropagation()}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      commitRenameQuery(query);
                    }
                    if (e.key === 'Escape') {
                      renamingQueryId = null;
                    }
                  }}
                  onblur={() => commitRenameQuery(query)}
                />
              {:else}
                <button
                  class="query-btn"
                  class:unresolved={query.connectionStatus === 'unresolved'}
                  onclick={() => handleQueryClick(query)}
                  onpointerdown={(e) => startDrag(e, 'query', query.id)}
                  oncontextmenu={(e) => showSavedCtxMenu(e, 'query', query.id, query.name)}
                  title={query.connectionStatus === 'unresolved'
                    ? `${query.name} — connection not found, click to assign`
                    : `Open ${query.name}`}
                >
                  <span class="query-icon" aria-hidden="true"
                    ><svg
                      width="13"
                      height="13"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="1.7"
                      stroke-linecap="round"
                      stroke-linejoin="round"
                      ><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"
                      ></path><polyline points="14 2 14 8 20 8"></polyline><polyline
                        points="9.5 13 8 15 9.5 17"
                      ></polyline><polyline points="14.5 13 16 15 14.5 17"></polyline></svg
                    ></span
                  >
                  <span class="query-name">{query.name}</span>
                  {#if query.connectionStatus === 'unresolved'}
                    <span
                      class="conn-warn"
                      aria-label="Connection not assigned"
                      title="Connection not assigned — click to assign">!</span
                    >
                  {:else if query.connectionStatus === 'resolved' && query.connectionId}
                    {@const conn = connectionStore.getById(query.connectionId)}
                    {#if conn}
                      <span
                        class="query-conn-dot"
                        style="background: {conn.color ?? 'var(--color-accent)'}"
                        title={conn.name}
                        aria-hidden="true"
                      ></span>
                    {/if}
                  {/if}
                  <span class="drag-handle" aria-hidden="true"
                    ><svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor"
                      ><circle cx="2" cy="2" r="1.2" /><circle cx="6" cy="2" r="1.2" /><circle
                        cx="2"
                        cy="6"
                        r="1.2"
                      /><circle cx="6" cy="6" r="1.2" /><circle cx="2" cy="10" r="1.2" /><circle
                        cx="6"
                        cy="10"
                        r="1.2"
                      /></svg
                    ></span
                  >
                </button>
              {/if}
            </li>
          {/snippet}

          {#snippet folderTree(items: TopLevelItem[], parentFolderId: string | null)}
            {#each items as tlItem (tlItem.kind === 'query' ? tlItem.item.id : 'f:' + tlItem.item.id)}
              {#if tlItem.kind === 'query'}
                {@render queryRow(tlItem.item, parentFolderId)}
              {:else}
                {@const folder = tlItem.item}
                {@const folderDirectQueries = queriesByFolder.get(folder.id) ?? []}
                {@const isOpen = expandedFolders.has(folder.id)}
                {@const folderChildItems = childItems(folder.id)}
                <li
                  class="folder-node"
                  class:drop-into={dropZone?.type === 'into-folder' &&
                    dropZone.folderId === folder.id}
                  class:drop-before={dropZone?.type === 'before-folder' &&
                    dropZone.folderId === folder.id}
                  class:drop-after={dropZone?.type === 'after-folder' &&
                    dropZone.folderId === folder.id}
                  class:is-dragging={isDragging &&
                    dragging?.kind === 'folder' &&
                    dragging.id === folder.id}
                  data-drop-type="folder"
                  data-folder-id={folder.id}
                  role="treeitem"
                  aria-expanded={isOpen}
                  aria-selected={false}
                >
                  <button
                    class="folder-btn"
                    onclick={() => handleFolderToggle(folder.id)}
                    onpointerdown={(e) => startDrag(e, 'folder', folder.id)}
                    oncontextmenu={(e) => showSavedCtxMenu(e, 'folder', folder.id, folder.name)}
                    aria-label="{isOpen ? 'Collapse' : 'Expand'} folder {folder.name}"
                  >
                    <span class="folder-icon" aria-hidden="true"
                      ><svg
                        width="13"
                        height="13"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.8"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        ><path
                          d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"
                        /></svg
                      ></span
                    >
                    <span class="folder-name">{folder.name}</span>
                    <span class="count-badge">{folderDirectQueries.length}</span>
                    <span class="chevron" class:open={isOpen} aria-hidden="true"
                      ><svg
                        width="10"
                        height="10"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.2"
                        stroke-linecap="round"
                        stroke-linejoin="round"><polyline points="9 18 15 12 9 6" /></svg
                      ></span
                    >
                    <span class="drag-handle" aria-hidden="true"
                      ><svg width="8" height="12" viewBox="0 0 8 12" fill="currentColor"
                        ><circle cx="2" cy="2" r="1.2" /><circle cx="6" cy="2" r="1.2" /><circle
                          cx="2"
                          cy="6"
                          r="1.2"
                        /><circle cx="6" cy="6" r="1.2" /><circle cx="2" cy="10" r="1.2" /><circle
                          cx="6"
                          cy="10"
                          r="1.2"
                        /></svg
                      ></span
                    >
                  </button>

                  {#if isOpen && folderChildItems.length > 0}
                    <ul class="folder-children" role="group">
                      {@render folderTree(folderChildItems, folder.id)}
                    </ul>
                  {/if}
                </li>
              {/if}
            {/each}
          {/snippet}

          <ul class="saved-list" role="tree" aria-label="Saved queries">
            {@render folderTree(childItems(null), null)}
          </ul>
        {/if}
      </div>
    {:else if activePanel === 'column'}
      <div id="panel-column" role="tabpanel" aria-label="Column Inspector">
        <div class="panel-toolbar">
          <span class="panel-title">Column Inspector</span>
        </div>
        {#if !cellSelectionStore.current}
          <div class="placeholder-panel">
            <p>Select a cell in a table to inspect its column.</p>
          </div>
        {:else if columnInfoLoading}
          <div class="loading-row">Loading…</div>
        {:else if !columnInfoData}
          <div class="placeholder-panel"><p>Column not found.</p></div>
        {:else}
          {@const sel = cellSelectionStore.current}
          <div class="context-bar">
            <span class="ctx-table">{sel!.table}</span><span class="ctx-dot">.</span><span
              class="ctx-col">{columnInfoData.name}</span
            >
          </div>
          <div class="info-section">
            <div class="info-section-title">Properties</div>
            <dl class="info-dl">
              <div class="info-row">
                <dt>Type</dt>
                <dd class="mono">{columnInfoData.dataType}</dd>
              </div>
              <div class="info-row">
                <dt>Nullable</dt>
                <dd>{columnInfoData.nullable ? 'Yes' : 'No'}</dd>
              </div>
              {#if columnInfoData.defaultValue !== null}
                <div class="info-row">
                  <dt>Default</dt>
                  <dd class="mono">{columnInfoData.defaultValue}</dd>
                </div>
              {/if}
              {#if columnInfoData.isPrimaryKey}
                <div class="info-row">
                  <dt>Primary Key</dt>
                  <dd class="badge-pk">PK</dd>
                </div>
              {/if}
              {#if columnInfoData.isAutoIncrement}
                <div class="info-row">
                  <dt>Auto Increment</dt>
                  <dd>Yes</dd>
                </div>
              {/if}
              {#if columnInfoData.isForeignKey}
                <div class="info-row">
                  <dt>Foreign Key</dt>
                  <dd>Yes</dd>
                </div>
              {/if}
              {#if columnInfoData.comment}
                <div class="info-row">
                  <dt>Comment</dt>
                  <dd>{columnInfoData.comment}</dd>
                </div>
              {/if}
            </dl>
          </div>
          {#if columnIndexes.length > 0}
            <div class="info-section">
              <div class="info-section-title">Indexes</div>
              {#each columnIndexes as idx (idx.name)}
                <div class="tag-row">
                  <span class="tag-name mono">{idx.name}</span>
                  {#if idx.unique}<span class="tag-badge">UNIQUE</span>{/if}
                  <span class="tag-type">{idx.indexType}</span>
                </div>
              {/each}
            </div>
          {/if}
          {#if columnForeignKeys.length > 0}
            <div class="info-section">
              <div class="info-section-title">Foreign Keys</div>
              {#each columnForeignKeys as fk (fk.constraintName)}
                <div class="fk-card">
                  <div class="fk-name mono">{fk.constraintName}</div>
                  <div class="fk-ref">
                    → <span class="mono"
                      >{fk.referencedTable}.{fk.referencedColumns.join(', ')}</span
                    >
                  </div>
                  <div class="fk-actions">ON DELETE {fk.onDelete} · ON UPDATE {fk.onUpdate}</div>
                </div>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {:else if activePanel === 'relations'}
      <div id="panel-relations" role="tabpanel" aria-label="Relations" class="relations-tabpanel">
        <div class="panel-toolbar">
          <span class="panel-title">Relations</span>
        </div>
        <RelationsPanel />
      </div>
    {/if}
  </div>
</div>

<!-- Saved queries context menu -->
<ContextMenu
  x={savedCtxMenu?.x ?? 0}
  y={savedCtxMenu?.y ?? 0}
  open={savedCtxMenu !== null}
  onclose={closeSavedCtxMenu}
  minWidth={140}
  zIndex={1000}
>
  {#if savedCtxMenu?.kind === 'query'}
    {@const ctxQuery = savedQueries.find((q) => q.id === savedCtxMenu?.id)}
    <CtxItem
      onclick={() => {
        if (ctxQuery) handleQueryClick(ctxQuery);
        closeSavedCtxMenu();
      }}>Open</CtxItem
    >
    <CtxItem onclick={handleSavedCtxDuplicate}>Duplicate</CtxItem>
    <CtxItem onclick={() => startRenameQuery(savedCtxMenu!.id, savedCtxMenu!.name)}>Rename</CtxItem>
    <CtxSep />
  {:else if savedCtxMenu?.kind === 'folder'}
    <CtxItem onclick={handleSavedCtxRenameFolder}>Rename</CtxItem>
    <CtxSep />
  {/if}
  <CtxItem danger onclick={handleSavedCtxDelete}>Delete</CtxItem>
</ContextMenu>

{#if confirmDeleteQueryId !== null}
  <ConfirmDialog
    title="Delete query"
    message="Delete this saved query? This will delete the .sql file. This cannot be undone."
    confirmText="Delete"
    cancelText="Cancel"
    danger={true}
    onconfirm={confirmDeleteQuery}
    oncancel={() => {
      confirmDeleteQueryId = null;
    }}
  />
{/if}

{#if confirmDeleteFolderId !== null}
  <ConfirmDialog
    title="Delete folder"
    message={`Delete the folder "${confirmDeleteFolderName}"? Any queries inside will become unfiled. This cannot be undone.`}
    confirmText="Delete"
    cancelText="Cancel"
    danger={true}
    onconfirm={confirmDeleteFolder}
    oncancel={() => {
      confirmDeleteFolderId = null;
    }}
  />
{/if}

{#if assigningQuery !== null}
  <div class="assign-overlay" use:portal>
    <div class="assign-dialog" role="dialog" aria-modal="true" aria-label="Assign connection">
      <h3 class="assign-title">Assign connection</h3>
      <p class="assign-desc">
        This query was saved with a connection that doesn't exist on this machine.
      </p>
      {#if assigningQuery.fileFingerprint}
        <p class="assign-fingerprint">
          <span class="assign-fp-label">Original connection:</span>
          <code class="assign-fp-value">{assigningQuery.fileFingerprint}</code>
        </p>
      {/if}
      <label class="assign-label" for="assign-select">Map to local connection:</label>
      <select id="assign-select" class="assign-select" bind:value={assignConnectionId}>
        {#each connectionStore.profiles as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
      <p class="assign-note">
        This mapping is saved locally and applied to all files sharing the same original connection.
      </p>
      <div class="assign-actions">
        <button class="assign-btn secondary" onclick={openQueryAnyway}>Open anyway</button>
        <button class="assign-btn secondary" onclick={() => (assigningQuery = null)}>Cancel</button>
        <button
          class="assign-btn primary"
          disabled={!assignConnectionId}
          onclick={confirmAssignConnection}
        >
          Assign &amp; open
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .right-sidebar {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .tab-strip {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 2px;
    padding: var(--spacing-1) var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .tab-btn {
    width: 30px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-md);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .tab-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
  }

  .spacer {
    flex: 1;
  }

  .panel-content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .panel-content > div {
    flex: 1;
    display: flex;
    flex-direction: column;
  }

  .panel-toolbar {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 6px var(--spacing-2);
    min-height: 34px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .panel-title {
    font-size: 10.5px;
    font-weight: 700;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .toolbar-gap {
    flex: 1;
  }

  .connection-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .action-btn {
    font-size: var(--font-size-xs);
    padding: 2px var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast);
    white-space: nowrap;
  }

  .action-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .danger-btn {
    color: var(--color-danger);
    border-color: var(--color-danger);
  }

  .danger-btn:hover {
    background: var(--color-danger-subtle);
  }

  .muted {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .loading-row,
  .empty-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  /* ── History panel ─────────────────────────────────────────────────────── */

  .history-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .history-item {
    border-bottom: 1px solid var(--color-border);
  }

  .history-btn {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--spacing-2) var(--spacing-3);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .history-btn:hover {
    background: var(--color-bg-hover);
  }

  .history-sql {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .history-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .status-badge {
    font-size: 10px;
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .status-badge.success {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .status-badge.error {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .meta-item {
    font-size: 10px;
    color: var(--color-text-muted);
  }

  .time-ago {
    margin-left: auto;
  }

  /* ── Saved Queries panel ───────────────────────────────────────────────── */

  .inline-input-row {
    display: flex;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
  }

  .inline-input {
    flex: 1;
    padding: 3px var(--spacing-2);
    font-size: var(--font-size-xs);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
  }

  .inline-input:focus {
    border-color: var(--color-accent);
  }

  .saved-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) var(--spacing-1);
  }

  .folder-node {
    display: flex;
    flex-direction: column;
  }

  .folder-btn,
  .query-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    width: 100%;
    padding: 6px var(--spacing-2);
    font-size: 13px;
    color: var(--color-text-secondary);
    text-align: left;
    cursor: pointer;
    border-radius: var(--radius-md);
    transition: background var(--transition-fast);
    -webkit-user-select: none;
    user-select: none;
  }

  .folder-btn:hover,
  .query-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .folder-icon,
  .query-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .folder-name,
  .query-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .folder-name {
    font-weight: var(--font-weight-medium);
  }

  .count-badge {
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    color: var(--color-text-disabled);
    background: var(--color-bg-tertiary);
    border-radius: 10px;
    padding: 1px 6px;
    flex-shrink: 0;
  }

  .chevron {
    color: var(--color-text-muted);
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .chevron svg {
    transition: transform var(--transition-fast);
  }

  .chevron.open svg {
    transform: rotate(90deg);
  }

  .folder-children {
    padding-left: var(--spacing-6);
  }

  .query-node {
    display: flex;
    align-items: center;
  }

  .query-rename-input {
    flex: 1;
    min-width: 0;
    height: 22px;
    margin: 2px var(--spacing-2) 2px var(--spacing-3);
    padding: 0 var(--spacing-1);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
  }

  /* ── Drag & Drop ─────────────────────────────────────────────────────────── */

  .drag-handle {
    opacity: 0;
    color: var(--color-text-muted);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    width: 14px;
    cursor: grab;
    transition: opacity var(--transition-fast);
  }

  .query-btn:hover .drag-handle,
  .folder-btn:hover .drag-handle {
    opacity: 0.5;
  }

  :global(body.is-dragging),
  :global(body.is-dragging *) {
    cursor: grabbing !important;
  }

  .query-node,
  .folder-node {
    position: relative;
  }

  .query-node.drop-before::before,
  .folder-node.drop-before::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--color-accent);
    pointer-events: none;
    z-index: 1;
  }

  .query-node.drop-after::after,
  .folder-node.drop-after::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--color-accent);
    pointer-events: none;
    z-index: 1;
  }

  .folder-node.drop-into > .folder-btn {
    background: var(--color-accent-subtle) !important;
    outline: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
  }

  .is-dragging {
    opacity: 0.35;
  }

  /* ── Connection status ───────────────────────────────────────────────────── */

  .query-btn.unresolved {
    color: var(--color-warning, #f59e0b);
  }

  .query-conn-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .conn-warn {
    flex-shrink: 0;
    width: 14px;
    height: 14px;
    display: grid;
    place-items: center;
    background: var(--color-warning, #f59e0b);
    color: #000;
    border-radius: 50%;
    font-size: 9px;
    font-weight: 800;
    line-height: 1;
  }

  /* ── Connection assignment dialog ────────────────────────────────────────── */

  .assign-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 600;
  }

  .assign-dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    padding: var(--spacing-5);
    width: min(420px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .assign-title {
    font-size: var(--font-size-md);
    font-weight: 600;
    color: var(--color-text-primary);
    margin: 0;
  }

  .assign-desc {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    margin: 0;
  }

  .assign-fingerprint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .assign-fp-label {
    font-weight: 600;
  }

  .assign-fp-value {
    font-family: var(--font-family-mono, monospace);
    background: var(--color-bg-secondary);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    word-break: break-all;
  }

  .assign-label {
    font-size: var(--font-size-sm);
    font-weight: 500;
    color: var(--color-text-primary);
  }

  .assign-select {
    width: 100%;
    padding: var(--spacing-2) var(--spacing-2);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
  }

  .assign-note {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin: 0;
    font-style: italic;
  }

  .assign-actions {
    display: flex;
    gap: var(--spacing-2);
    justify-content: flex-end;
    flex-wrap: wrap;
  }

  .assign-btn {
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-family: var(--font-family-ui);
    transition: background var(--transition-fast);
  }

  .assign-btn.secondary {
    background: transparent;
    border: 1px solid var(--color-border-strong);
    color: var(--color-text-secondary);
  }

  .assign-btn.secondary:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .assign-btn.primary {
    background: var(--color-accent);
    border: 1px solid transparent;
    color: #fff;
    font-weight: 600;
  }

  .assign-btn.primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .assign-btn.primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .placeholder-panel {
    padding: var(--spacing-3) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    line-height: var(--line-height-normal);
  }

  /* ── Column Inspector / Table Info shared ────────────────────────────────── */

  .context-bar {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 1px;
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .ctx-table {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
  }

  .ctx-dot {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .ctx-col {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .info-section {
    border-bottom: 1px solid var(--color-border);
    padding: var(--spacing-2) 0;
  }

  .info-section-title {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: 0 var(--spacing-2) var(--spacing-1);
  }

  .info-dl {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 0 var(--spacing-2);
  }

  .info-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
  }

  .info-row dt {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    min-width: 90px;
    flex-shrink: 0;
  }

  .info-row dd {
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  .badge-pk {
    font-size: 10px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-weight: var(--font-weight-semibold);
  }

  .tag-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: 2px var(--spacing-2);
    font-size: var(--font-size-xs);
  }

  .tag-name {
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 120px;
  }

  .tag-badge {
    font-size: 10px;
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-radius: var(--radius-sm);
    padding: 1px 4px;
    font-weight: var(--font-weight-semibold);
    flex-shrink: 0;
  }

  .tag-type {
    font-size: 10px;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .fk-card {
    margin: 0 var(--spacing-2) var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
  }

  .fk-name {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .fk-ref {
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    margin-top: 1px;
  }

  .fk-actions {
    font-size: 10px;
    color: var(--color-text-muted);
    margin-top: 1px;
  }

  .relations-tabpanel {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
</style>
