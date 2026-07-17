<!--
  ConnectionTree — unified connection + schema tree.
  Shows all connections with expand-to-browse (databases → tables).
  Visual design matches Glass Workspace spec: chevron + color-dot-with-glow + name.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useTabDrag } from '$lib/stores/tabDragState.svelte';
  import ConnectionForm from './ConnectionForm.svelte';
  import DbIcon from '$lib/components/icons/DbIcon.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import ShareIcon from '$lib/components/icons/ShareIcon.svelte';
  import SearchIcon from '$lib/components/icons/SearchIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import PlusIcon from '$lib/components/icons/PlusIcon.svelte';
  import LockIcon from '$lib/components/icons/LockIcon.svelte';
  import EditIcon from '$lib/components/icons/EditIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import { getAllSystemDatabases } from '$lib/stores/dialects.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as schemaApi from '$lib/tauri/schema';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import ExportConnectionsDialog from './ExportConnectionsDialog.svelte';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import type { ConnectionProfile, ConnectionGroup, TableInfo } from '$lib/types';
  import { qi, tableRef as dialectTableRef } from '$lib/utils/dialect';
  import { listen } from '@tauri-apps/api/event';
  import { useGlobalSearchCache } from '$lib/stores/globalSearchCache.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import SchemaIcon from '$lib/components/icons/SchemaIcon.svelte';

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const settingsStore = useSettings();
  const toast = useToast();
  const tabDrag = useTabDrag();
  const globalSearchCache = useGlobalSearchCache();

  let treeScrollEl = $state<HTMLDivElement | undefined>(undefined);

  $effect(() => {
    function handleFocusSchemaTree() {
      const firstBtn = treeScrollEl?.querySelector<HTMLElement>('button');
      (firstBtn ?? treeScrollEl)?.focus();
    }
    document.addEventListener('focus-schema-tree', handleFocusSchemaTree);
    return () => document.removeEventListener('focus-schema-tree', handleFocusSchemaTree);
  });

  // ── Add / edit forms ──────────────────────────────────────────────────────

  let showAddForm = $state(false);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);
  let newConnectionGroupId = $state<string | null | undefined>(undefined);

  // ── Schema state ──────────────────────────────────────────────────────────

  let schemaCache = $state<Map<string, Map<string, TableInfo[]>>>(new Map());
  // SQL Server only: connectionId → instanceDb → schemas[]
  let instanceSchemaCache = $state<Map<string, Map<string, string[]>>>(new Map());
  let expandedConnections = $state<Set<string>>(new Set());
  let expandedDatabases = $state<Set<string>>(new Set());
  // SQL Server only: `${connectionId}/${instanceDb}/${schema}`
  let expandedSchemas = $state<Set<string>>(new Set());
  let loadingKeys = $state<Set<string>>(new Set());

  $effect(() => {
    let unlisten: (() => void) | null = null;
    listen<{ connectionId: string; database: string; tableName: string; count: number }>(
      'table-count-updated',
      (event) => {
        const { connectionId, database, tableName, count } = event.payload;
        const connMap = schemaCache.get(connectionId);
        const tables = connMap?.get(database);
        if (!tables) return;
        const updated = tables.map((t) => (t.name === tableName ? { ...t, rowCount: count } : t));
        const newConnMap = new Map(connMap);
        newConnMap.set(database, updated);
        schemaCache = new Map([...schemaCache, [connectionId, newConnMap]]);
      },
    ).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  });

  // ── Group UI state ────────────────────────────────────────────────────────

  let expandedGroups = $state<Set<string>>(new Set());
  let ungroupedExpanded = $state(true);
  let renamingGroupId = $state<string | null>(null);
  let renameValue = $state('');
  let renameError = $state('');
  let renameLoading = $state(false);

  // ── Filter state ──────────────────────────────────────────────────────────

  let filterQuery = $state('');
  let filterInputEl = $state<HTMLInputElement | undefined>(undefined);

  // ── Context menus ─────────────────────────────────────────────────────────

  interface TableCtxMenu {
    x: number;
    y: number;
    connectionId: string;
    database: string;
    table: TableInfo;
  }
  interface DbCtxMenu {
    x: number;
    y: number;
    connectionId: string;
    database: string;
  }
  interface GrpCtxMenu {
    x: number;
    y: number;
    group: ConnectionGroup;
  }
  interface ConnCtxMenu {
    x: number;
    y: number;
    profile: ConnectionProfile;
  }
  interface PanelCtxMenu {
    x: number;
    y: number;
  }

  interface InstDbCtxMenu {
    x: number;
    y: number;
    connectionId: string;
    instanceDb: string;
  }
  interface SchemaCtxMenu {
    x: number;
    y: number;
    connectionId: string;
    instanceDb: string;
    schema: string;
  }

  interface StaticDbCtxMenu {
    x: number;
    y: number;
    connectionId: string;
    database: string;
  }

  let tableCtx = $state<TableCtxMenu | null>(null);
  let dbCtx = $state<DbCtxMenu | null>(null);
  let grpCtx = $state<GrpCtxMenu | null>(null);
  let connCtx = $state<ConnCtxMenu | null>(null);
  let panelCtx = $state<PanelCtxMenu | null>(null);
  let instDbCtx = $state<InstDbCtxMenu | null>(null);
  let schemaCtx = $state<SchemaCtxMenu | null>(null);
  let staticDbCtx = $state<StaticDbCtxMenu | null>(null);

  let moveToGroupSubmenuOpen = $state(false);
  let moveToGroupSubmenuTimer = $state<ReturnType<typeof setTimeout> | null>(null);

  interface ConfirmState {
    title: string;
    message: string;
    confirmText?: string;
    onconfirm: () => void;
  }
  let confirmState = $state<ConfirmState | null>(null);
  let errorModal = $state<{ title: string; message: string } | null>(null);

  // ── Create modals ─────────────────────────────────────────────────────────

  interface CreateDbModal {
    connectionId: string;
  }
  interface CreateTableColumn {
    name: string;
    type: string;
    nullable: boolean;
    primaryKey: boolean;
  }
  interface CreateTableFk {
    localColumn: string;
    refTable: string;
    refColumn: string;
    onDelete: string;
    onUpdate: string;
  }
  interface CreateTableModal {
    connectionId: string;
    database: string;
    dbType: string;
  }

  let createDbModal = $state<CreateDbModal | null>(null);
  let createDbName = $state('');
  let createDbError = $state('');
  let createDbLoading = $state(false);

  let createTableModal = $state<CreateTableModal | null>(null);
  let createTableName = $state('');
  let createTableColumns = $state<CreateTableColumn[]>([]);
  let createTableFks = $state<CreateTableFk[]>([]);
  let createTableError = $state('');
  let createTableLoading = $state(false);
  let fkRefColumns = $state<Map<number, string[]>>(new Map());

  interface CreateSchemaModal {
    connectionId: string;
    instanceDb: string;
  }
  let createSchemaModal = $state<CreateSchemaModal | null>(null);
  let createSchemaName = $state('');
  let createSchemaError = $state('');
  let createSchemaLoading = $state(false);

  async function loadFkRefColumns(fkIndex: number, refTable: string) {
    if (!createTableModal || !refTable) return;
    const { connectionId, database } = createTableModal;
    try {
      const cols = await schemaApi.listColumns(connectionId, database, refTable);
      fkRefColumns = new Map([...fkRefColumns, [fkIndex, cols.map((c) => c.name)]]);
    } catch {
      fkRefColumns = new Map([...fkRefColumns, [fkIndex, []]]);
    }
  }

  const refActions = [
    { value: 'NO ACTION', label: 'NO ACTION' },
    { value: 'RESTRICT', label: 'RESTRICT' },
    { value: 'CASCADE', label: 'CASCADE' },
    { value: 'SET NULL', label: 'SET NULL' },
  ];

  // ── Connection helpers ────────────────────────────────────────────────────

  function isConnected(id: string) {
    return connectionStore.activeIds.has(id);
  }
  function isConnecting(id: string) {
    return connectionStore.connectingIds.has(id);
  }
  function hasError(id: string) {
    return connectionStore.errorIds.has(id);
  }

  function dotColor(profile: ConnectionProfile): string {
    return profile.color ?? 'var(--color-accent)';
  }

  async function handleConnect(profile: ConnectionProfile) {
    if (isConnected(profile.id)) return;
    try {
      await connectionStore.connect(profile.id);
      if (isConnected(profile.id)) toggleExpand(profile.id);
    } catch (err) {
      toast.addToast(`${profile.name}: ${errorMessage(err)}`, 'error', 0);
    }
  }

  function deleteConnection(profile: ConnectionProfile, onDone?: () => void) {
    confirmState = {
      title: 'Delete Connection',
      message: `Delete "${profile.name}"? This cannot be undone.`,
      onconfirm: async () => {
        confirmState = null;
        if (connectionStore.isActive(profile.id)) await connectionStore.disconnect(profile.id);
        try {
          await connectionsApi.deleteConnection(profile.id);
          panelStore.closeItemsForConnection(profile.id);
          await connectionStore.load();
          onDone?.();
        } catch {
          /* ignore */
        }
      },
    };
  }

  // ── Schema helpers ────────────────────────────────────────────────────────

  async function toggleExpand(connectionId: string) {
    if (expandedConnections.has(connectionId)) {
      expandedConnections = new Set([...expandedConnections].filter((id) => id !== connectionId));
    } else {
      expandedConnections = new Set([...expandedConnections, connectionId]);
      await loadDatabases(connectionId);
    }
  }

  async function loadDatabases(connectionId: string) {
    if (schemaCache.has(connectionId)) return;
    loadingKeys = new Set([...loadingKeys, connectionId]);
    try {
      const databases = await schemaApi.listDatabases(connectionId);
      const dbMap = new Map<string, TableInfo[]>();
      databases.forEach((db) => dbMap.set(db, []));
      schemaCache = new Map([...schemaCache, [connectionId, dbMap]]);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== connectionId));
    }
  }

  async function toggleDatabase(connectionId: string, database: string) {
    const key = `${connectionId}/${database}`;
    if (expandedDatabases.has(key)) {
      expandedDatabases = new Set([...expandedDatabases].filter((k) => k !== key));
    } else {
      expandedDatabases = new Set([...expandedDatabases, key]);
      await loadTables(connectionId, database);
    }
  }

  async function loadTables(connectionId: string, database: string) {
    const key = `${connectionId}/${database}`;
    const existing = schemaCache.get(connectionId)?.get(database);
    if (existing && existing.length > 0) return;
    loadingKeys = new Set([...loadingKeys, key]);
    try {
      const tables = await schemaApi.listTables(connectionId, database);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(database, tables);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== key));
    }
  }

  // ── SQL Server 3-level tree ────────────────────────────────────────────────

  async function toggleInstanceDb(connectionId: string, instanceDb: string) {
    const key = `${connectionId}/${instanceDb}`;
    if (expandedDatabases.has(key)) {
      expandedDatabases = new Set([...expandedDatabases].filter((k) => k !== key));
    } else {
      expandedDatabases = new Set([...expandedDatabases, key]);
      await loadSchemas(connectionId, instanceDb);
    }
  }

  async function loadSchemas(connectionId: string, instanceDb: string) {
    const existing = instanceSchemaCache.get(connectionId)?.get(instanceDb);
    if (existing && existing.length > 0) return;
    const key = `${connectionId}/${instanceDb}`;
    loadingKeys = new Set([...loadingKeys, key]);
    try {
      const schemas = await schemaApi.listSchemas(connectionId, instanceDb);
      const instMap = new Map(instanceSchemaCache.get(connectionId) ?? []);
      instMap.set(instanceDb, schemas);
      instanceSchemaCache = new Map([...instanceSchemaCache, [connectionId, instMap]]);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== key));
    }
  }

  async function toggleSchema(connectionId: string, instanceDb: string, schema: string) {
    const key = `${connectionId}/${instanceDb}/${schema}`;
    if (expandedSchemas.has(key)) {
      expandedSchemas = new Set([...expandedSchemas].filter((k) => k !== key));
    } else {
      expandedSchemas = new Set([...expandedSchemas, key]);
      await loadTablesForSchema(connectionId, instanceDb, schema);
    }
  }

  async function loadTablesForSchema(connectionId: string, instanceDb: string, schema: string) {
    const cacheKey = `${instanceDb}/${schema}`;
    const existing = schemaCache.get(connectionId)?.get(cacheKey);
    if (existing && existing.length > 0) return;
    const key = `${connectionId}/${instanceDb}/${schema}`;
    loadingKeys = new Set([...loadingKeys, key]);
    try {
      const tables = await schemaApi.listTables(connectionId, schema, instanceDb);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(cacheKey, tables);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error', 0);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter((k) => k !== key));
    }
  }

  const focusedContent = $derived(panelStore.focusedPanel?.content);

  function isTableActive(connectionId: string, database: string, tableName: string): boolean {
    return (
      focusedContent?.kind === 'table_browser' &&
      focusedContent.connectionId === connectionId &&
      focusedContent.database === database &&
      focusedContent.table === tableName
    );
  }

  function openTable(connectionId: string, database: string, table: string, instanceDb?: string) {
    panelStore.openInFocused({ kind: 'table_browser', connectionId, database, table, instanceDb });
  }

  // ── Group helpers ─────────────────────────────────────────────────────────

  function toggleGroup(groupId: string) {
    if (expandedGroups.has(groupId)) {
      expandedGroups = new Set([...expandedGroups].filter((id) => id !== groupId));
    } else {
      expandedGroups = new Set([...expandedGroups, groupId]);
    }
  }

  async function commitRename() {
    if (!renamingGroupId || !renameValue.trim()) {
      renamingGroupId = null;
      return;
    }
    renameLoading = true;
    renameError = '';
    try {
      await connectionsApi.updateConnectionGroup(renamingGroupId, { name: renameValue.trim() });
      await connectionStore.load();
      renamingGroupId = null;
    } catch (e) {
      renameError = errorMessage(e);
    } finally {
      renameLoading = false;
    }
  }

  function deleteGroup(group: ConnectionGroup) {
    grpCtx = null;
    confirmState = {
      title: 'Delete Group',
      message: `Delete group "${group.name}" and all its connections? This cannot be undone.`,
      onconfirm: async () => {
        confirmState = null;
        try {
          await connectionsApi.deleteConnectionGroup(group.id);
          await connectionStore.load();
        } catch {
          /* ignore */
        }
      },
    };
  }

  // ── Group creation ────────────────────────────────────────────────────────

  let createGroupModal = $state(false);
  let newGroupName = $state('');
  let newGroupError = $state('');
  let newGroupLoading = $state(false);

  // ── Import / Export ───────────────────────────────────────────────────────

  let showExportDialog = $state(false);
  let exportSingleId = $state<string | null>(null);
  let exportPreselectIds = $state<string[] | null>(null);

  async function handleImportConnections() {
    closeAllCtx();
    const files = await openFileDialog({
      multiple: true,
      filters: [{ name: 'Connection file', extensions: ['json'] }],
    });
    if (!files) return;
    const paths = Array.isArray(files) ? files : [files];
    if (paths.length === 0) return;

    let totalImported = 0;
    const errors: string[] = [];
    for (const filePath of paths) {
      try {
        const result = await connectionsApi.importConnections(filePath);
        totalImported += result.imported;
      } catch (err) {
        errors.push(errorMessage(err));
      }
    }

    await connectionStore.load();

    if (errors.length > 0) {
      toast.addToast(`Import failed: ${errors[0]}`, 'error', 0);
    } else {
      toast.addToast(
        `Imported ${totalImported} connection${totalImported !== 1 ? 's' : ''}`,
        'success',
      );
    }
  }

  function handleExportConnections() {
    closeAllCtx();
    exportSingleId = null;
    exportPreselectIds = null;
    showExportDialog = true;
  }

  function startCreateGroup() {
    closeAllCtx();
    newGroupName = '';
    newGroupError = '';
    createGroupModal = true;
  }

  async function commitCreateGroup() {
    const name = newGroupName.trim();
    if (!name) return;
    newGroupLoading = true;
    newGroupError = '';
    try {
      await connectionsApi.createConnectionGroup(name);
      await connectionStore.load();
      createGroupModal = false;
    } catch (e) {
      newGroupError = errorMessage(e);
    } finally {
      newGroupLoading = false;
    }
  }

  // ── Context menu helpers ──────────────────────────────────────────────────

  function closeAllCtx() {
    tableCtx = null;
    dbCtx = null;
    grpCtx = null;
    connCtx = null;
    panelCtx = null;
    instDbCtx = null;
    schemaCtx = null;
    staticDbCtx = null;
    moveToGroupSubmenuOpen = false;
    if (moveToGroupSubmenuTimer) {
      clearTimeout(moveToGroupSubmenuTimer);
      moveToGroupSubmenuTimer = null;
    }
  }

  function showTableCtx(e: MouseEvent, connectionId: string, database: string, table: TableInfo) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    tableCtx = { x: e.clientX, y: e.clientY, connectionId, database, table };
  }

  function showDbCtx(e: MouseEvent, connectionId: string, database: string) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    dbCtx = { x: e.clientX, y: e.clientY, connectionId, database };
  }

  function showGrpCtx(e: MouseEvent, group: ConnectionGroup) {
    e.preventDefault();
    closeAllCtx();
    grpCtx = { x: e.clientX, y: e.clientY, group };
  }

  function showConnCtx(e: MouseEvent, profile: ConnectionProfile) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    connCtx = { x: e.clientX, y: e.clientY, profile };
  }

  function showPanelCtx(e: MouseEvent) {
    e.preventDefault();
    closeAllCtx();
    panelCtx = { x: e.clientX, y: e.clientY };
  }

  function showInstDbCtx(e: MouseEvent, connectionId: string, instanceDb: string) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    instDbCtx = { x: e.clientX, y: e.clientY, connectionId, instanceDb };
  }

  function showSchemaCtx(e: MouseEvent, connectionId: string, instanceDb: string, schema: string) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    schemaCtx = { x: e.clientX, y: e.clientY, connectionId, instanceDb, schema };
  }

  function showStaticDbCtx(e: MouseEvent, connectionId: string, database: string) {
    e.preventDefault();
    e.stopPropagation();
    closeAllCtx();
    staticDbCtx = { x: e.clientX, y: e.clientY, connectionId, database };
  }

  async function ctxRefreshConnection() {
    if (!connCtx) return;
    const { profile } = connCtx;
    connCtx = null;
    schemaCache = new Map([...schemaCache].filter(([k]) => k !== profile.id));
    expandedDatabases = new Set(
      [...expandedDatabases].filter((k) => !k.startsWith(`${profile.id}/`)),
    );
    await loadDatabases(profile.id);
  }

  async function ctxRefreshDatabase() {
    if (!dbCtx) return;
    const { connectionId, database } = dbCtx;
    dbCtx = null;
    const connMap = new Map(schemaCache.get(connectionId) ?? []);
    connMap.set(database, []);
    schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    if (expandedDatabases.has(`${connectionId}/${database}`)) {
      await loadTables(connectionId, database);
    }
  }

  async function ctxRefreshInstanceDb() {
    if (!instDbCtx) return;
    const { connectionId, instanceDb } = instDbCtx;
    instDbCtx = null;
    const instMap = new Map(instanceSchemaCache.get(connectionId) ?? []);
    instMap.delete(instanceDb);
    instanceSchemaCache = new Map([...instanceSchemaCache, [connectionId, instMap]]);
    const connMap = new Map(schemaCache.get(connectionId) ?? []);
    for (const key of [...connMap.keys()].filter((k) => k.startsWith(`${instanceDb}/`))) {
      connMap.delete(key);
    }
    schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    if (expandedDatabases.has(`${connectionId}/${instanceDb}`)) {
      await loadSchemas(connectionId, instanceDb);
    }
  }

  function ctxDropInstanceDatabase() {
    if (!instDbCtx) return;
    const { connectionId, instanceDb } = instDbCtx;
    const profile = connectionStore.getById(connectionId);
    instDbCtx = null;
    if (!profile || profile.readOnly) return;
    const d = profile.dialectInfo;
    const sql = `DROP DATABASE ${qi(instanceDb, d)}`;
    confirmState = {
      title: 'Drop Database',
      message: `Drop database "${instanceDb}"? This will permanently delete all schemas, tables, and data within it. This cannot be undone.`,
      confirmText: 'Drop Database',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
          const connMap = new Map(schemaCache.get(connectionId) ?? []);
          for (const key of [...connMap.keys()].filter(
            (k) => k.startsWith(`${instanceDb}/`) || k === instanceDb,
          )) {
            connMap.delete(key);
          }
          schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
          const instMap = new Map(instanceSchemaCache.get(connectionId) ?? []);
          instMap.delete(instanceDb);
          instanceSchemaCache = new Map([...instanceSchemaCache, [connectionId, instMap]]);
          expandedDatabases = new Set(
            [...expandedDatabases].filter((k) => k !== `${connectionId}/${instanceDb}`),
          );
          expandedSchemas = new Set(
            [...expandedSchemas].filter((k) => !k.startsWith(`${connectionId}/${instanceDb}/`)),
          );
        } catch (err) {
          errorModal = { title: 'Drop Failed', message: errorMessage(err) };
        }
      },
    };
  }

  function ctxNewSchemaInDb() {
    if (!instDbCtx) return;
    const { connectionId, instanceDb } = instDbCtx;
    instDbCtx = null;
    createSchemaName = '';
    createSchemaError = '';
    createSchemaModal = { connectionId, instanceDb };
  }

  async function executeCreateSchema() {
    if (!createSchemaModal) return;
    const name = createSchemaName.trim();
    if (!name) {
      createSchemaError = 'Name is required';
      return;
    }
    const { connectionId, instanceDb } = createSchemaModal;
    const profile = connectionStore.getById(connectionId);
    const d = profile?.dialectInfo;
    const dbEsc = instanceDb.replace(/]/g, ']]');
    const sql = `USE [${dbEsc}]; CREATE SCHEMA ${d ? qi(name, d) : `[${name}]`}`;
    createSchemaLoading = true;
    createSchemaError = '';
    try {
      await schemaApi.executeDdl(connectionId, sql);
      const instMap = new Map(instanceSchemaCache.get(connectionId) ?? []);
      instMap.delete(instanceDb);
      instanceSchemaCache = new Map([...instanceSchemaCache, [connectionId, instMap]]);
      await loadSchemas(connectionId, instanceDb);
      createSchemaModal = null;
    } catch (err) {
      createSchemaError = errorMessage(err);
    } finally {
      createSchemaLoading = false;
    }
  }

  async function ctxRefreshSchema() {
    if (!schemaCtx) return;
    const { connectionId, instanceDb, schema } = schemaCtx;
    schemaCtx = null;
    const cacheKey = `${instanceDb}/${schema}`;
    const connMap = new Map(schemaCache.get(connectionId) ?? []);
    connMap.set(cacheKey, []);
    schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    if (expandedSchemas.has(`${connectionId}/${instanceDb}/${schema}`)) {
      await loadTablesForSchema(connectionId, instanceDb, schema);
    }
  }

  function ctxNewTableInSchema() {
    if (!schemaCtx) return;
    const { connectionId, instanceDb, schema } = schemaCtx;
    const profile = connectionStore.getById(connectionId);
    schemaCtx = null;
    if (!profile) return;
    createTableName = '';
    createTableError = '';
    createTableColumns = [
      { name: 'id', type: profile.dialectInfo.defaultColumnType, nullable: false, primaryKey: true },
    ];
    createTableFks = [];
    fkRefColumns = new Map();
    createTableModal = { connectionId, database: schema, dbType: profile.dbType };
  }

  function ctxOpenErdForSchema() {
    if (!schemaCtx) return;
    panelStore.openInFocused({
      kind: 'erd',
      connectionId: schemaCtx.connectionId,
      database: schemaCtx.schema,
    });
    schemaCtx = null;
  }

  function ctxDropSchemaInDb() {
    if (!schemaCtx) return;
    const { connectionId, instanceDb, schema } = schemaCtx;
    const profile = connectionStore.getById(connectionId);
    schemaCtx = null;
    if (!profile || profile.readOnly) return;
    const d = profile.dialectInfo;
    const dbEsc = instanceDb.replace(/]/g, ']]');
    const sql = `USE [${dbEsc}]; DROP SCHEMA ${qi(schema, d)}`;
    confirmState = {
      title: 'Drop Schema',
      message: `Drop schema "${schema}" in "${instanceDb}"? This will permanently delete all tables and data within it. This cannot be undone.`,
      confirmText: 'Drop Schema',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
          const cacheKey = `${instanceDb}/${schema}`;
          const connMap = new Map(schemaCache.get(connectionId) ?? []);
          connMap.delete(cacheKey);
          schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
          const instMap = new Map(instanceSchemaCache.get(connectionId) ?? []);
          const schemas = instMap.get(instanceDb) ?? [];
          instMap.set(
            instanceDb,
            schemas.filter((s) => s !== schema),
          );
          instanceSchemaCache = new Map([...instanceSchemaCache, [connectionId, instMap]]);
          expandedSchemas = new Set(
            [...expandedSchemas].filter((k) => k !== `${connectionId}/${instanceDb}/${schema}`),
          );
        } catch (err) {
          errorModal = { title: 'Drop Failed', message: errorMessage(err) };
        }
      },
    };
  }

  function ctxNewQueryEditorForInstanceDb() {
    if (!instDbCtx) return;
    const { connectionId, instanceDb } = instDbCtx;
    instDbCtx = null;
    panelStore.openInFocused({ kind: 'query_editor', connectionId, database: instanceDb });
  }

  function ctxNewQueryEditorForSchema() {
    if (!schemaCtx) return;
    const { connectionId, schema } = schemaCtx;
    schemaCtx = null;
    panelStore.openInFocused({ kind: 'query_editor', connectionId, database: schema });
  }

  function ctxNewSchemaForStaticDb() {
    if (!staticDbCtx) return;
    const { connectionId } = staticDbCtx;
    staticDbCtx = null;
    createDbName = '';
    createDbError = '';
    createDbModal = { connectionId };
  }

  async function ctxRefreshStaticDb() {
    if (!staticDbCtx) return;
    const { connectionId } = staticDbCtx;
    staticDbCtx = null;
    schemaCache = new Map([...schemaCache].filter(([k]) => k !== connectionId));
    expandedDatabases = new Set(
      [...expandedDatabases].filter((k) => !k.startsWith(`${connectionId}/`)),
    );
    await loadDatabases(connectionId);
  }

  async function ctxMoveToGroup(groupId: string | null) {
    if (!connCtx) return;
    const { profile } = connCtx;
    connCtx = null;
    await connectionStore.update(profile.id, {
      name: profile.name,
      dbType: profile.dbType,
      host: profile.host,
      port: profile.port,
      database: profile.database,
      username: profile.username,
      color: profile.color,
      readOnly: profile.readOnly,
      groupId,
      sshEnabled: profile.sshEnabled,
      sshHost: profile.sshHost,
      sshPort: profile.sshPort,
      sshUser: profile.sshUser,
      sshAuthType: profile.sshAuthType,
      sshKeyPath: profile.sshKeyPath,
      sslEnabled: profile.sslEnabled,
      sslCaPath: profile.sslCaPath,
      sslCertPath: profile.sslCertPath,
      sslKeyPath: profile.sslKeyPath,
      poolMax: profile.poolMax,
    });
  }

  async function ctxConnToggleReadOnly() {
    if (!connCtx) return;
    const id = connCtx.profile.id;
    connCtx = null;
    await connectionStore.toggleReadOnly(id);
  }

  async function ctxDisconnectAll() {
    panelCtx = null;
    const activeIds = [...connectionStore.activeIds];
    await Promise.all(activeIds.map((id) => connectionStore.disconnect(id)));
    for (const id of activeIds) {
      panelStore.closeItemsForConnection(id, { skipDirty: true });
    }
    expandedConnections = new Set([...expandedConnections].filter((i) => !activeIds.includes(i)));
  }

  async function ctxConnDisconnect() {
    if (!connCtx) return;
    const id = connCtx.profile.id;
    connCtx = null;
    await connectionStore.disconnect(id);
    panelStore.closeItemsForConnection(id, { skipDirty: true });
    expandedConnections = new Set([...expandedConnections].filter((i) => i !== id));
  }

  async function ctxNewQueryEditor() {
    if (!connCtx) return;
    const profile = connCtx.profile;
    connCtx = null;
    if (!isConnected(profile.id)) await connectionStore.connect(profile.id);
    panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id });
  }

  async function ctxManageUsers() {
    if (!connCtx) return;
    const profile = connCtx.profile;
    connCtx = null;
    if (!isConnected(profile.id)) await connectionStore.connect(profile.id);
    panelStore.openInFocused({ kind: 'user_manager', connectionId: profile.id });
  }

  function ctxOpenTable() {
    if (!tableCtx) return;
    openTable(tableCtx.connectionId, tableCtx.database, tableCtx.table.name);
    tableCtx = null;
  }

  function ctxOpenTableCopy() {
    if (!tableCtx) return;
    panelStore.openCopyInFocused({
      kind: 'table_browser',
      connectionId: tableCtx.connectionId,
      database: tableCtx.database,
      table: tableCtx.table.name,
    });
    tableCtx = null;
  }

  function ctxViewDdl() {
    if (!tableCtx) return;
    panelStore.openInFocused({
      kind: 'ddl_viewer',
      connectionId: tableCtx.connectionId,
      database: tableCtx.database,
      objectName: tableCtx.table.name,
      objectType: tableCtx.table.tableType,
    });
    tableCtx = null;
  }

  function ctxCopyName() {
    if (!tableCtx) return;
    navigator.clipboard.writeText(tableCtx.table.name);
    tableCtx = null;
  }

  function ctxGenerateSqlSelectAll() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile) return;
    const ref = dialectTableRef(database, table.name, profile.dialectInfo);
    panelStore.openCopyInFocused({
      kind: 'query_editor',
      connectionId,
      database,
      initialSql: `SELECT * FROM ${ref}`,
    });
  }

  function ctxGenerateSqlSelectFirst() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile) return;
    const ref = dialectTableRef(database, table.name, profile.dialectInfo);
    panelStore.openCopyInFocused({
      kind: 'query_editor',
      connectionId,
      database,
      initialSql: profile.dialectInfo.selectTop ? `SELECT TOP  * FROM ${ref}` : `SELECT * FROM ${ref} LIMIT `,
    });
  }

  async function ctxGenerateSqlInsert() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile) return;
    const ref = dialectTableRef(database, table.name, profile.dialectInfo);
    let sql: string;
    try {
      const columns = await schemaApi.listColumns(connectionId, database, table.name);
      const insertCols = columns.filter((c) => !c.isAutoIncrement);
      const colList = insertCols.map((c) => qi(c.name, profile.dialectInfo)).join(', ');
      const valList = insertCols.map(() => '').join(', ');
      sql = `INSERT INTO ${ref} (${colList})\nVALUES (${valList})`;
    } catch {
      sql = `INSERT INTO ${ref} ()\nVALUES ()`;
    }
    panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
  }

  async function ctxGenerateSqlUpdate() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile) return;
    const ref = dialectTableRef(database, table.name, profile.dialectInfo);
    let sql: string;
    try {
      const columns = await schemaApi.listColumns(connectionId, database, table.name);
      const pkCols = columns.filter((c) => c.isPrimaryKey);
      const dataCols = columns.filter((c) => !c.isPrimaryKey);
      const setCols = dataCols.length > 0 ? dataCols : columns;
      const setClauses = setCols
        .map((c) => `    ${qi(c.name, profile.dialectInfo)} = `)
        .join(',\n');
      const whereClauses =
        pkCols.length > 0
          ? pkCols.map((c) => `${qi(c.name, profile.dialectInfo)} = `).join(' AND ')
          : '';
      sql = `UPDATE ${ref}\nSET\n${setClauses}\nWHERE ${whereClauses}`;
    } catch {
      sql = `UPDATE ${ref}\nSET\n    \nWHERE `;
    }
    panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
  }

  async function ctxGenerateSqlDelete() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile) return;
    const ref = dialectTableRef(database, table.name, profile.dialectInfo);
    let sql: string;
    try {
      const columns = await schemaApi.listColumns(connectionId, database, table.name);
      const pkCols = columns.filter((c) => c.isPrimaryKey);
      const whereClauses =
        pkCols.length > 0
          ? pkCols.map((c) => `${qi(c.name, profile.dialectInfo)} = `).join(' AND ')
          : '';
      sql = `DELETE FROM ${ref}\nWHERE ${whereClauses}`;
    } catch {
      sql = `DELETE FROM ${ref}\nWHERE `;
    }
    panelStore.openCopyInFocused({ kind: 'query_editor', connectionId, database, initialSql: sql });
  }

  function ctxOpenErd() {
    if (!dbCtx) return;
    panelStore.openInFocused({
      kind: 'erd',
      connectionId: dbCtx.connectionId,
      database: dbCtx.database,
    });
    dbCtx = null;
  }

  function ctxDropTable() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile || profile.readOnly) return;
    const d = profile.dialectInfo;
    const sql = d.usesSchema
      ? `DROP TABLE ${qi(database, d)}.${qi(table.name, d)}`
      : `DROP TABLE ${qi(table.name, d)}`;
    confirmState = {
      title: 'Drop Table',
      message: `Drop table "${table.name}"? This will permanently delete the table and all its data. This cannot be undone.`,
      confirmText: 'Drop Table',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
          panelStore.closeItemsForTable(connectionId, database, table.name);
          const connMap = new Map(schemaCache.get(connectionId) ?? []);
          connMap.set(database, []);
          schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
          await loadTables(connectionId, database);
        } catch (err) {
          errorModal = { title: 'Drop Table Failed', message: errorMessage(err) };
        }
      },
    };
  }

  function ctxDropDatabase() {
    if (!dbCtx) return;
    const { connectionId, database } = dbCtx;
    const profile = connectionStore.getById(connectionId);
    dbCtx = null;
    if (!profile || profile.readOnly) return;
    const d = profile.dialectInfo;
    let sql: string;
    const isSchema = d.usesSchema;
    if (d.usesDatabaseKeyword) {
      sql = `DROP DATABASE ${qi(database, d)}`;
    } else if (isSchema) {
      sql = `DROP SCHEMA ${qi(database, d)}${d.dropSchemaCascade ? ' CASCADE' : ''}`;
    } else {
      return;
    }
    confirmState = {
      title: isSchema ? 'Drop Schema' : 'Drop Database',
      message: `Drop ${isSchema ? 'schema' : 'database'} "${database}"? This will permanently delete all tables and data within it. This cannot be undone.`,
      confirmText: isSchema ? 'Drop Schema' : 'Drop Database',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
          panelStore.closeItemsForDatabase(connectionId, database);
          const connMap = new Map(schemaCache.get(connectionId) ?? []);
          connMap.delete(database);
          schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
          expandedDatabases = new Set(
            [...expandedDatabases].filter((k) => k !== `${connectionId}/${database}`),
          );
        } catch (err) {
          errorModal = { title: 'Drop Failed', message: errorMessage(err) };
        }
      },
    };
  }

  // ── Create database ───────────────────────────────────────────────────────

  function ctxNewDatabase() {
    if (!connCtx) return;
    const { profile } = connCtx;
    connCtx = null;
    createDbName = '';
    createDbError = '';
    createDbModal = { connectionId: profile.id };
  }

  async function executeCreateDatabase() {
    if (!createDbModal) return;
    const name = createDbName.trim();
    if (!name) {
      createDbError = 'Name is required';
      return;
    }
    const { connectionId } = createDbModal;
    const profile = connectionStore.getById(connectionId);
    const d = profile?.dialectInfo;
    const sql =
      d?.hasInstanceDatabases || d?.usesDatabaseKeyword
        ? `CREATE DATABASE ${qi(name, d)}`
        : `CREATE SCHEMA ${d ? qi(name, d) : `"${name}"`}`;
    createDbLoading = true;
    createDbError = '';
    try {
      await schemaApi.executeDdl(connectionId, sql);
      schemaCache = new Map([...schemaCache].filter(([k]) => k !== connectionId));
      await loadDatabases(connectionId);
      createDbModal = null;
    } catch (err) {
      createDbError = errorMessage(err);
    } finally {
      createDbLoading = false;
    }
  }

  // ── Create table ──────────────────────────────────────────────────────────


  function ctxNewTable() {
    if (!dbCtx) return;
    const { connectionId, database } = dbCtx;
    const profile = connectionStore.getById(connectionId);
    dbCtx = null;
    if (!profile) return;
    createTableName = '';
    createTableError = '';
    createTableColumns = [
      { name: 'id', type: profile.dialectInfo.defaultColumnType, nullable: false, primaryKey: true },
    ];
    createTableFks = [];
    fkRefColumns = new Map();
    createTableModal = { connectionId, database, dbType: profile.dbType };
  }

  async function executeCreateTable() {
    if (!createTableModal) return;
    const name = createTableName.trim();
    if (!name) {
      createTableError = 'Table name is required';
      return;
    }
    const emptyCol = createTableColumns.find((c) => !c.name.trim());
    if (emptyCol) {
      createTableError = 'All columns must have a name';
      return;
    }
    const incompleteFk = createTableFks.find(
      (fk) => !fk.localColumn || !fk.refTable.trim() || !fk.refColumn.trim(),
    );
    if (incompleteFk) {
      createTableError =
        'All foreign keys must have a local column, referenced table, and referenced column';
      return;
    }
    const { connectionId, database, dbType } = createTableModal;
    const prof = connectionStore.getById(connectionId);
    const d = prof?.dialectInfo;
    const q = (n: string) => (d ? qi(n, d) : `"${n}"`);
    const pkCols = createTableColumns.filter((c) => c.primaryKey);
    const colDefs = createTableColumns.map((c) => {
      let def = `  ${q(c.name.trim())} ${c.type}`;
      if (!c.nullable) def += ' NOT NULL';
      return def;
    });
    if (pkCols.length > 0) {
      colDefs.push(`  PRIMARY KEY (${pkCols.map((c) => q(c.name.trim())).join(', ')})`);
    }
    for (const fk of createTableFks) {
      const fkName = `fk_${name}_${fk.localColumn}`;
      colDefs.push(
        `  CONSTRAINT ${q(fkName)} FOREIGN KEY (${q(fk.localColumn)}) REFERENCES ${q(fk.refTable.trim())} (${q(fk.refColumn.trim())}) ON DELETE ${fk.onDelete} ON UPDATE ${fk.onUpdate}`,
      );
    }
    const tablePath = d?.usesSchema ? `${q(database)}.${q(name)}` : q(name);
    const sql = `CREATE TABLE ${tablePath} (\n${colDefs.join(',\n')}\n)`;
    createTableLoading = true;
    createTableError = '';
    try {
      await schemaApi.executeDdl(connectionId, sql);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(database, []);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
      if (expandedDatabases.has(`${connectionId}/${database}`)) {
        await loadTables(connectionId, database);
      }
      createTableModal = null;
    } catch (err) {
      createTableError = errorMessage(err);
    } finally {
      createTableLoading = false;
    }
  }

  // ── Table row drag-to-split ───────────────────────────────────────────────

  interface TableDragStart {
    x: number;
    y: number;
    connectionId: string;
    database: string;
    tableName: string;
  }

  let tableDragStart = $state<TableDragStart | null>(null);

  function onTablePointerDown(
    e: PointerEvent,
    connectionId: string,
    database: string,
    tableName: string,
  ) {
    tableDragStart = { x: e.clientX, y: e.clientY, connectionId, database, tableName };
  }

  function onTablePointerMove(e: PointerEvent) {
    if (!tableDragStart || tabDrag.isDragging) return;
    const dx = e.clientX - tableDragStart.x;
    const dy = e.clientY - tableDragStart.y;
    if (Math.hypot(dx, dy) > 5) {
      tabDrag.startContent({
        kind: 'table_browser',
        connectionId: tableDragStart.connectionId,
        database: tableDragStart.database,
        table: tableDragStart.tableName,
      });
    }
  }

  function onTablePointerUp() {
    tableDragStart = null;
  }

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      renamingGroupId = null;
      filterQuery = '';
    }
  }

  function handleWindowPointerUp() {
    tableDragStart = null;
    if (tabDrag.isDragging && tabDrag.dragContent !== null) {
      tabDrag.end();
    }
  }

  // ── System database / table detection ────────────────────────────────────

  function checkSystemDatabase(name: string) {
    return isSystemDatabase(name, [...settingsStore.settings.systemDatabases, ...getAllSystemDatabases()]);
  }
  function checkSystemTable(name: string) {
    return isSystemTable(name, settingsStore.settings.systemTablePatterns);
  }

  // ── Filter helpers ────────────────────────────────────────────────────────

  $effect(() => {
    if (!filterQuery) return;
    const connectedProfiles = connectionStore.profiles.filter((p) =>
      connectionStore.activeIds.has(p.id),
    );
    globalSearchCache.populate(connectedProfiles);
  });

  function profileMatchesFilter(profile: ConnectionProfile): boolean {
    if (!filterQuery) return true;
    const q = filterQuery.toLowerCase();
    if (profile.name.toLowerCase().includes(q)) return true;
    if (!isConnected(profile.id)) return false;
    return (
      globalSearchCache.databaseEntries.some(
        (e) => e.connectionId === profile.id && e.database.toLowerCase().includes(q),
      ) ||
      globalSearchCache.tableEntries.some(
        (e) => e.connectionId === profile.id && e.name.toLowerCase().includes(q),
      )
    );
  }

  function dbMatchesFilter(profileId: string, database: string): boolean {
    if (!filterQuery) return true;
    const q = filterQuery.toLowerCase();
    if (database.toLowerCase().includes(q)) return true;
    return globalSearchCache.tableEntries.some(
      (e) => e.connectionId === profileId && e.database === database && e.name.toLowerCase().includes(q),
    );
  }

  // Builds a merged Map<db, TableInfo[]> from the global search cache for filter mode,
  // preferring schemaCache entries (which have rowCount).
  function buildFilterMap(
    profileId: string,
    schemaDb: Map<string, TableInfo[]> | undefined,
  ): Map<string, TableInfo[]> | undefined {
    const result = new Map<string, TableInfo[]>();
    for (const e of globalSearchCache.databaseEntries) {
      if (e.connectionId === profileId) result.set(e.database, []);
    }
    for (const e of globalSearchCache.tableEntries) {
      if (e.connectionId === profileId) {
        result.get(e.database)?.push({ name: e.name, tableType: e.tableType, rowCount: null });
      }
    }
    if (schemaDb) {
      for (const [db, tables] of schemaDb) {
        if (tables.length > 0) result.set(db, tables);
        else if (!result.has(db)) result.set(db, []);
      }
    }
    return result.size > 0 ? result : schemaDb;
  }

  function handleTreeKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (
      (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) &&
      target !== filterInputEl
    )
      return;
    if (renamingGroupId || createGroupModal || createDbModal || createTableModal || confirmState)
      return;

    if (e.key === 'Escape' && filterQuery) {
      filterQuery = '';
      treeScrollEl?.focus();
      return;
    }

    if (e.key.length === 1 && !e.metaKey && !e.ctrlKey && target !== filterInputEl) {
      filterQuery += e.key;
      requestAnimationFrame(() => filterInputEl?.focus());
      return;
    }

    if (e.key === 'Backspace' && target !== filterInputEl && filterQuery) {
      e.preventDefault();
      filterQuery = filterQuery.slice(0, -1);
    }
  }

  // ── Derived groupings ─────────────────────────────────────────────────────

  const grouped = $derived(() => {
    const groups = connectionStore.groups;
    const profiles = connectionStore.profiles;
    const ungrouped = profiles.filter((p) => p.groupId === null);
    const byGroup = new Map<string, ConnectionProfile[]>();
    for (const g of groups) byGroup.set(g.id, []);
    for (const p of profiles) {
      if (p.groupId !== null && byGroup.has(p.groupId)) byGroup.get(p.groupId)!.push(p);
    }
    return { groups, ungrouped, byGroup };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} onpointerup={handleWindowPointerUp} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="connection-tree" onkeydown={handleTreeKeydown}>
  <!-- Section header -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="tree-header no-select" oncontextmenu={showPanelCtx}>
    <span class="header-label">CONNECTIONS</span>
    <span class="header-count">{connectionStore.profiles.length}</span>
    <div class="spacer"></div>
    <button
      class="header-icon-btn"
      onclick={() => panelStore.openInFocused({ kind: 'connections' })}
      title="Manage all connections"
      aria-label="Manage connections"
    >
      <ShareIcon width={13} height={13} />
    </button>
  </div>

  <!-- Filter bar -->
  {#if filterQuery}
    <div class="filter-bar">
      <SearchIcon width={12} height={12} class="filter-icon" />
      <input
        bind:this={filterInputEl}
        bind:value={filterQuery}
        class="filter-input"
        type="text"
        autocomplete="off"
        autocapitalize="off"
        autocorrect="off"
        spellcheck={false}
        oninput={() => {
          if (!filterQuery) treeScrollEl?.focus();
        }}
        onkeydown={(e) => {
          if (e.key === 'Escape') {
            filterQuery = '';
            treeScrollEl?.focus();
          }
        }}
      />
      <button
        class="filter-clear"
        onclick={() => {
          filterQuery = '';
          treeScrollEl?.focus();
        }}
        aria-label="Clear filter"
      >
        <CloseIcon width={12} height={12} strokeWidth={2.5} />
      </button>
    </div>
  {/if}

  <!-- Scrollable list -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="tree-scroll gscroll"
    bind:this={treeScrollEl}
    tabindex="-1"
    oncontextmenu={(e) => {
      if ((e.target as Element).closest('.conn-item,.group-section,.ctx-menu')) return;
      showPanelCtx(e);
    }}
  >
    {#if connectionStore.profiles.length === 0 && connectionStore.groups.length === 0}
      <div class="empty-state">
        <p>No connections yet.</p>
      </div>
    {:else}
      <!-- Ungrouped profiles -->
      {#if connectionStore.groups.length > 0 && grouped().ungrouped.length > 0}
        {@const filteredUngrouped = grouped().ungrouped.filter(profileMatchesFilter)}
        {#if filteredUngrouped.length > 0}
          <div class="group-section">
            <button class="group-row" onclick={() => (ungroupedExpanded = !ungroupedExpanded)}>
              <span class="chevron" class:open={ungroupedExpanded || !!filterQuery} aria-hidden="true">
                <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
              </span>
              <span class="group-name">Ungrouped</span>
              <span class="group-count">{filteredUngrouped.length}</span>
            </button>
            {#if ungroupedExpanded || filterQuery}
              {#each filteredUngrouped as profile (profile.id)}
                {@render connectionRow(profile)}
              {/each}
            {/if}
          </div>
        {/if}
      {:else}
        {#each grouped().ungrouped.filter(profileMatchesFilter) as profile (profile.id)}
          {@render connectionRow(profile)}
        {/each}
      {/if}

      <!-- Named groups -->
      {#each grouped().groups as group (group.id)}
        {@const isExpanded = expandedGroups.has(group.id)}
        {@const groupProfiles = (grouped().byGroup.get(group.id) ?? []).filter(profileMatchesFilter)}
        {#if !filterQuery || groupProfiles.length > 0}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="group-section" oncontextmenu={(e) => showGrpCtx(e, group)}>
            <button class="group-row" onclick={() => toggleGroup(group.id)}>
              <span class="chevron" class:open={isExpanded || !!filterQuery} aria-hidden="true">
                <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
              </span>
              <span class="group-name">{group.name}</span>
              <span class="group-count">{groupProfiles.length}</span>
            </button>
            {#if isExpanded || filterQuery}
              {#each groupProfiles as profile (profile.id)}
                {@render connectionRow(profile)}
              {/each}
            {/if}
          </div>
        {/if}
      {/each}

      {#if filterQuery && grouped().ungrouped.filter(profileMatchesFilter).length === 0 && grouped().groups.every((g) => (grouped().byGroup.get(g.id) ?? []).filter(profileMatchesFilter).length === 0)}
        <div class="empty-state">
          <p>No matches for "{filterQuery}".</p>
        </div>
      {/if}
    {/if}

    <!-- Add connection inline row -->
    <button
      class="add-row"
      onclick={() => {
        newConnectionGroupId = undefined;
        showAddForm = true;
      }}
    >
      <PlusIcon width={14} height={14} strokeWidth={1.8} />
      Add connection
    </button>
  </div>
</div>

<!-- ── Connection row snippet ─────────────────────────────────────────────── -->
{#snippet connectionRow(profile: ConnectionProfile)}
  {@const connected = isConnected(profile.id)}
  {@const connecting = isConnecting(profile.id)}
  {@const errored = hasError(profile.id)}
  {@const expanded = expandedConnections.has(profile.id)}
  {@const color = dotColor(profile)}

  <div class="conn-item">
    <!-- Main row -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="conn-row"
      class:connected
      class:errored
      oncontextmenu={(e) => showConnCtx(e, profile)}
      onclick={() => (connected ? toggleExpand(profile.id) : handleConnect(profile))}
      onkeydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          connected ? toggleExpand(profile.id) : handleConnect(profile);
        }
      }}
    >
      <div class="conn-row-left">
        <!-- Chevron: rotates when expanded -->
        <button
          class="conn-chevron"
          class:open={expanded}
          onclick={(e) => {
            e.stopPropagation();
            connected ? toggleExpand(profile.id) : handleConnect(profile);
          }}
          aria-label="{expanded ? 'Collapse' : 'Expand'} {profile.name}"
          disabled={connecting}
        >
          {#if connecting}
            <Spinner />
          {:else}
            <ChevronIcon direction="right" width={12} height={12} strokeWidth={2.2} />
          {/if}
        </button>

        <!-- Color dot with glow -->
        <span
          class="color-dot"
          class:dim={!connected && !connecting}
          style="background:{color};{connected
            ? `box-shadow:0 0 0 3px color-mix(in srgb,${color} 18%,transparent)`
            : ''}"
          aria-hidden="true"
        ></span>

        <!-- Name -->
        <button
          class="conn-name"
          onclick={(e) => {
            e.stopPropagation();
            connected
              ? panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id })
              : handleConnect(profile);
          }}
          title={profile.host}>{profile.name}</button
        >

        <!-- Lock icon if read-only — click to toggle -->
        {#if profile.readOnly}
          <button
            class="lock-icon-btn"
            onclick={(e) => {
              e.stopPropagation();
              connectionStore.toggleReadOnly(profile.id);
            }}
            title="Read-only — click to disable"
            aria-label="Disable read-only for {profile.name}"
            aria-pressed={true}
          >
            <LockIcon width={11} height={11} />
          </button>
        {/if}
      </div>

      <!-- Hover actions -->
      <div class="conn-actions">
        <button
          class="action-btn"
          onclick={(e) => {
            e.stopPropagation();
            editingProfile = profile;
          }}
          title="Edit connection"
          aria-label="Edit {profile.name}"
        >
          <EditIcon width={13} height={13} strokeWidth={1.8} />
        </button>
      </div>
    </div>

    <!-- Schema tree when expanded -->
    {#if (expanded || filterQuery) && connected}
      {@const schemaDb = schemaCache.get(profile.id)}
      {@const databases = filterQuery ? buildFilterMap(profile.id, schemaDb) : schemaDb}
      {@const isLoadingConn = loadingKeys.has(profile.id)}

      <div class="schema-children">
        {#if isLoadingConn}
          <div class="loading-row">
            <span class="loading-dots" aria-label="Loading">Loading…</span>
          </div>
        {:else if databases}
          {@const hasInstanceDbs = profile.dialectInfo?.hasInstanceDatabases ?? false}
          {#if hasInstanceDbs}
            <!-- SQL Server 3-level: Instance DB → Schema → Tables -->
            {#each [...databases.keys()].filter((db) => !db.includes('/') && (settingsStore.settings.showSystemItems || !checkSystemDatabase(db))) as instanceDb}
              {@const instKey = `${profile.id}/${instanceDb}`}
              {@const isInstExpanded = expandedDatabases.has(instKey)}
              {@const isInstLoading = loadingKeys.has(instKey)}
              {@const isInstSystem = checkSystemDatabase(instanceDb)}
              <div class="db-item" class:system-item={isInstSystem}>
                <button
                  class="db-row"
                  class:open={isInstExpanded}
                  onclick={() => toggleInstanceDb(profile.id, instanceDb)}
                  oncontextmenu={(e) => showInstDbCtx(e, profile.id, instanceDb)}
                  aria-label="{isInstExpanded ? 'Collapse' : 'Expand'} {instanceDb}"
                >
                  <span class="chevron" class:open={isInstExpanded} aria-hidden="true">
                    <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
                  </span>
                  <DbIcon system={isInstSystem} aria-hidden="true" />
                  <span class="db-name">{instanceDb}</span>
                  {#if isInstLoading}
                    <Spinner label="Loading" />
                  {/if}
                </button>

                {#if isInstExpanded}
                  {#each (instanceSchemaCache.get(profile.id)?.get(instanceDb) ?? []).filter((s) => settingsStore.settings.showSystemItems || !checkSystemDatabase(s)) as schema}
                    {@const schemaKey = `${profile.id}/${instanceDb}/${schema}`}
                    {@const isSchemaExpanded = expandedSchemas.has(schemaKey)}
                    {@const isSchemaLoading = loadingKeys.has(schemaKey)}
                    {@const schemaTables = schemaCache.get(profile.id)?.get(`${instanceDb}/${schema}`) ?? []}
                    <div class="db-item schema-item">
                      <button
                        class="db-row schema-row"
                        class:open={isSchemaExpanded}
                        onclick={() => toggleSchema(profile.id, instanceDb, schema)}
                        oncontextmenu={(e) => showSchemaCtx(e, profile.id, instanceDb, schema)}
                        aria-label="{isSchemaExpanded ? 'Collapse' : 'Expand'} {schema}"
                      >
                        <span class="chevron" class:open={isSchemaExpanded} aria-hidden="true">
                          <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
                        </span>
                        <SchemaIcon aria-hidden="true" />
                        <span class="db-name">{schema}</span>
                        {#if isSchemaLoading}
                          <Spinner label="Loading" />
                        {/if}
                      </button>

                      {#if isSchemaExpanded && schemaTables.length > 0}
                        <div class="table-list">
                          {#each schemaTables.filter((t) => settingsStore.settings.showSystemItems || !checkSystemTable(t.name)) as table}
                            {@const isTableSystem = checkSystemTable(table.name)}
                            <button
                              class="table-row"
                              class:system-item={isTableSystem}
                              class:active={isTableActive(profile.id, schema, table.name)}
                              onclick={(e) => {
                                if (tabDrag.isDragging) { e.preventDefault(); tabDrag.end(); return; }
                                openTable(profile.id, schema, table.name, instanceDb);
                              }}
                              oncontextmenu={(e) => showTableCtx(e, profile.id, schema, table)}
                              title={table.name}
                              aria-label="Open {table.name}"
                            >
                              <TableIcon system={isTableSystem} aria-hidden="true" />
                              <span class="table-name">{table.name}</span>
                              {#if table.rowCount !== null}
                                <span class="row-count">{table.rowCount.toLocaleString()}</span>
                              {/if}
                            </button>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                {/if}
              </div>
            {/each}
          {:else}
            {@const usesSchema = profile.dialectInfo?.usesSchema ?? false}
            {@const usesDatabaseKeyword = profile.dialectInfo?.usesDatabaseKeyword ?? false}
            {#if usesSchema && !usesDatabaseKeyword}
              <!-- Schema-based (Postgres): static database node wrapping schema list -->
              {@const dbNodeKey = `${profile.id}/__db__`}
              {@const isDbNodeExpanded = expandedDatabases.has(dbNodeKey)}
              <div class="db-item">
                <button
                  class="db-row"
                  class:open={isDbNodeExpanded || !!filterQuery}
                  onclick={() => {
                    expandedDatabases = isDbNodeExpanded
                      ? new Set([...expandedDatabases].filter((k) => k !== dbNodeKey))
                      : new Set([...expandedDatabases, dbNodeKey]);
                  }}
                  oncontextmenu={(e) => showStaticDbCtx(e, profile.id, profile.database)}
                  aria-label="{isDbNodeExpanded ? 'Collapse' : 'Expand'} {profile.database}"
                >
                  <span class="chevron" class:open={isDbNodeExpanded || !!filterQuery} aria-hidden="true">
                    <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
                  </span>
                  <DbIcon system={false} aria-hidden="true" />
                  <span class="db-name">{profile.database}</span>
                </button>

                {#if isDbNodeExpanded || filterQuery}
                  <div class="schema-item">
                    {#each [...databases.keys()].filter((db) => (settingsStore.settings.showSystemItems || !checkSystemDatabase(db)) && dbMatchesFilter(profile.id, db)) as database}
                      {@const dbKey = `${profile.id}/${database}`}
                      {@const isDbExpanded = expandedDatabases.has(dbKey)}
                      {@const isDbLoading = loadingKeys.has(dbKey)}
                      {@const tables = databases.get(database) ?? []}
                      <div class="db-item">
                        <button
                          class="db-row schema-row"
                          class:open={isDbExpanded || !!filterQuery}
                          onclick={() => toggleDatabase(profile.id, database)}
                          oncontextmenu={(e) => showDbCtx(e, profile.id, database)}
                          aria-label="{isDbExpanded ? 'Collapse' : 'Expand'} {database}"
                        >
                          <span class="chevron" class:open={isDbExpanded || !!filterQuery} aria-hidden="true">
                            <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
                          </span>
                          <SchemaIcon aria-hidden="true" />
                          <span class="db-name">{database}</span>
                          {#if isDbLoading}<Spinner label="Loading" />{/if}
                        </button>

                        {#if (isDbExpanded || filterQuery) && tables.length > 0}
                          <div class="table-list">
                            {#each tables.filter((t) => (settingsStore.settings.showSystemItems || !checkSystemTable(t.name)) && (!filterQuery || t.name.toLowerCase().includes(filterQuery.toLowerCase()))) as table}
                              {@const isTableSystem = checkSystemTable(table.name)}
                              <button
                                class="table-row"
                                class:system-item={isTableSystem}
                                class:active={isTableActive(profile.id, database, table.name)}
                                class:dragging={tabDrag.isDragging &&
                                  tabDrag.dragContent?.kind === 'table_browser' &&
                                  tabDrag.dragContent.connectionId === profile.id &&
                                  tabDrag.dragContent.database === database &&
                                  tabDrag.dragContent.table === table.name}
                                onclick={(e) => {
                                  if (tabDrag.isDragging) { e.preventDefault(); tabDrag.end(); return; }
                                  openTable(profile.id, database, table.name);
                                }}
                                oncontextmenu={(e) => showTableCtx(e, profile.id, database, table)}
                                onpointerdown={(e) => onTablePointerDown(e, profile.id, database, table.name)}
                                onpointermove={onTablePointerMove}
                                onpointerup={onTablePointerUp}
                                title={table.name}
                                aria-label="Open {table.name}"
                              >
                                <TableIcon system={isTableSystem} aria-hidden="true" />
                                <span class="table-name">{table.name}</span>
                                {#if table.rowCount !== null}
                                  <span class="row-count">{table.rowCount.toLocaleString()}</span>
                                {/if}
                              </button>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {:else}
              <!-- Regular 2-level: Database → Tables (MySQL, MariaDB, SQLite) -->
              {#each [...databases.keys()].filter((db) => (settingsStore.settings.showSystemItems || !checkSystemDatabase(db)) && dbMatchesFilter(profile.id, db)) as database}
                {@const dbKey = `${profile.id}/${database}`}
                {@const isDbExpanded = expandedDatabases.has(dbKey)}
                {@const isDbLoading = loadingKeys.has(dbKey)}
                {@const tables = databases.get(database) ?? []}

                {@const isDbSystem = checkSystemDatabase(database)}
                <div class="db-item" class:system-item={isDbSystem}>
                  <button
                    class="db-row"
                    class:open={isDbExpanded || !!filterQuery}
                    onclick={() => toggleDatabase(profile.id, database)}
                    oncontextmenu={(e) => showDbCtx(e, profile.id, database)}
                    aria-label="{isDbExpanded ? 'Collapse' : 'Expand'} {database}"
                  >
                    <span class="chevron" class:open={isDbExpanded || !!filterQuery} aria-hidden="true">
                      <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.2} />
                    </span>
                    <DbIcon system={isDbSystem} aria-hidden="true" />
                    <span class="db-name">{database}</span>
                    {#if isDbLoading}
                      <Spinner label="Loading" />
                    {/if}
                  </button>

                  {#if (isDbExpanded || filterQuery) && tables.length > 0}
                    <div class="table-list">
                      {#each tables.filter((t) => (settingsStore.settings.showSystemItems || !(isDbSystem || checkSystemTable(t.name))) && (!filterQuery || t.name.toLowerCase().includes(filterQuery.toLowerCase()))) as table}
                        {@const isTableSystem = isDbSystem || checkSystemTable(table.name)}
                        <button
                          class="table-row"
                          class:system-item={isTableSystem}
                          class:active={isTableActive(profile.id, database, table.name)}
                          class:dragging={tabDrag.isDragging &&
                            tabDrag.dragContent?.kind === 'table_browser' &&
                            tabDrag.dragContent.connectionId === profile.id &&
                            tabDrag.dragContent.database === database &&
                            tabDrag.dragContent.table === table.name}
                          onclick={(e) => {
                            if (tabDrag.isDragging) {
                              e.preventDefault();
                              tabDrag.end();
                              return;
                            }
                            openTable(profile.id, database, table.name);
                          }}
                          oncontextmenu={(e) => showTableCtx(e, profile.id, database, table)}
                          onpointerdown={(e) => onTablePointerDown(e, profile.id, database, table.name)}
                          onpointermove={onTablePointerMove}
                          onpointerup={onTablePointerUp}
                          title={table.name}
                          aria-label="Open {table.name}"
                        >
                          <TableIcon system={isTableSystem} aria-hidden="true" />
                          <span class="table-name">{table.name}</span>
                          {#if table.rowCount !== null}
                            <span class="row-count">{table.rowCount.toLocaleString()}</span>
                          {/if}
                        </button>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/each}
            {/if}
          {/if}
        {/if}
      </div>
    {/if}
  </div>
{/snippet}

<!-- Context menus -->
<ContextMenu
  x={panelCtx?.x ?? 0}
  y={panelCtx?.y ?? 0}
  open={panelCtx !== null}
  onclose={closeAllCtx}
>
  <CtxItem
    onclick={() => {
      panelCtx = null;
      newConnectionGroupId = undefined;
      showAddForm = true;
    }}>New Connection</CtxItem
  >
  <CtxItem onclick={startCreateGroup}>New Group</CtxItem>
  <CtxSep />
  <CtxItem onclick={handleImportConnections}>Import Connections…</CtxItem>
  {#if connectionStore.profiles.length > 0}
    <CtxItem onclick={handleExportConnections}>Export Connections…</CtxItem>
  {/if}
  {#if connectionStore.activeIds.size > 0}
    <CtxSep />
    <CtxItem onclick={ctxDisconnectAll}>Disconnect All</CtxItem>
  {/if}
</ContextMenu>

{#if tableCtx}
  {@const tableCtxProfile = connectionStore.getById(tableCtx.connectionId)}
  <ContextMenu x={tableCtx.x} y={tableCtx.y} open={true} onclose={closeAllCtx}>
    <CtxItem onclick={ctxOpenTable}>Open Table</CtxItem>
    <CtxItem onclick={ctxOpenTableCopy}>Open Copy</CtxItem>
    <CtxItem onclick={ctxViewDdl}>View DDL</CtxItem>
    <CtxItem onclick={ctxCopyName}>Copy Name</CtxItem>
    <CtxSep />
    <CtxItem onclick={ctxGenerateSqlSelectAll}>Select All Rows</CtxItem>
    <CtxItem onclick={ctxGenerateSqlSelectFirst}>Select First N Rows</CtxItem>
    <CtxItem onclick={ctxGenerateSqlInsert}>Insert Row</CtxItem>
    <CtxItem onclick={ctxGenerateSqlUpdate}>Update Rows</CtxItem>
    <CtxItem onclick={ctxGenerateSqlDelete}>Delete Rows</CtxItem>
    {#if !tableCtxProfile?.readOnly}
      <CtxSep />
      <CtxItem danger onclick={ctxDropTable}>Drop Table</CtxItem>
    {/if}
  </ContextMenu>
{/if}

{#if dbCtx}
  {@const dbCtxProfile = connectionStore.getById(dbCtx.connectionId)}
  <ContextMenu x={dbCtx.x} y={dbCtx.y} open={true} onclose={closeAllCtx}>
    {#if !dbCtxProfile?.readOnly}
      <CtxItem onclick={ctxNewTable}>New Table</CtxItem>
      <CtxSep />
    {/if}
    <CtxItem
      onclick={() => {
        if (dbCtx) {
          panelStore.openInFocused({
            kind: 'query_editor',
            connectionId: dbCtx.connectionId,
            database: dbCtx.database,
          });
          dbCtx = null;
        }
      }}>New Query Editor</CtxItem
    >
    <CtxSep />
    <CtxItem onclick={ctxRefreshDatabase}>Refresh</CtxItem>
    <CtxItem onclick={ctxOpenErd}>Open ERD</CtxItem>
    <CtxSep />
    <CtxItem
      onclick={() => {
        if (dbCtx) {
          navigator.clipboard.writeText(dbCtx.database);
          dbCtx = null;
        }
      }}>Copy Name</CtxItem
    >
    {#if !dbCtxProfile?.readOnly && (dbCtxProfile?.dialectInfo.usesSchema || dbCtxProfile?.dialectInfo.usesDatabaseKeyword)}
      <CtxSep />
      <CtxItem danger onclick={ctxDropDatabase}>
        Drop {dbCtxProfile?.dialectInfo.dbLabel}
      </CtxItem>
    {/if}
  </ContextMenu>
{/if}

{#if instDbCtx}
  {@const instDbProfile = connectionStore.getById(instDbCtx.connectionId)}
  <ContextMenu x={instDbCtx.x} y={instDbCtx.y} open={true} onclose={closeAllCtx}>
    {#if !instDbProfile?.readOnly}
      <CtxItem onclick={ctxNewSchemaInDb}>New Schema</CtxItem>
      <CtxSep />
    {/if}
    <CtxItem onclick={ctxNewQueryEditorForInstanceDb}>New Query Editor</CtxItem>
    <CtxSep />
    <CtxItem onclick={ctxRefreshInstanceDb}>Refresh</CtxItem>
    <CtxItem
      onclick={() => {
        if (instDbCtx) {
          navigator.clipboard.writeText(instDbCtx.instanceDb);
          instDbCtx = null;
        }
      }}>Copy Name</CtxItem
    >
    {#if !instDbProfile?.readOnly}
      <CtxSep />
      <CtxItem danger onclick={ctxDropInstanceDatabase}>Drop Database</CtxItem>
    {/if}
  </ContextMenu>
{/if}

{#if schemaCtx}
  {@const schemaProfile = connectionStore.getById(schemaCtx.connectionId)}
  <ContextMenu x={schemaCtx.x} y={schemaCtx.y} open={true} onclose={closeAllCtx}>
    {#if !schemaProfile?.readOnly}
      <CtxItem onclick={ctxNewTableInSchema}>New Table</CtxItem>
      <CtxSep />
    {/if}
    <CtxItem onclick={ctxNewQueryEditorForSchema}>New Query Editor</CtxItem>
    <CtxItem onclick={ctxOpenErdForSchema}>Open ERD</CtxItem>
    <CtxSep />
    <CtxItem onclick={ctxRefreshSchema}>Refresh</CtxItem>
    <CtxItem
      onclick={() => {
        if (schemaCtx) {
          navigator.clipboard.writeText(schemaCtx.schema);
          schemaCtx = null;
        }
      }}>Copy Name</CtxItem
    >
    {#if !schemaProfile?.readOnly}
      <CtxSep />
      <CtxItem danger onclick={ctxDropSchemaInDb}>Drop Schema</CtxItem>
    {/if}
  </ContextMenu>
{/if}

{#if staticDbCtx}
  {@const staticDbProfile = connectionStore.getById(staticDbCtx.connectionId)}
  <ContextMenu x={staticDbCtx.x} y={staticDbCtx.y} open={true} onclose={closeAllCtx}>
    {#if !staticDbProfile?.readOnly}
      <CtxItem onclick={ctxNewSchemaForStaticDb}>New Schema</CtxItem>
      <CtxSep />
    {/if}
    <CtxItem
      onclick={() => {
        if (staticDbCtx) {
          panelStore.openInFocused({ kind: 'query_editor', connectionId: staticDbCtx.connectionId });
          staticDbCtx = null;
        }
      }}>New Query Editor</CtxItem
    >
    <CtxSep />
    <CtxItem onclick={ctxRefreshStaticDb}>Refresh</CtxItem>
    <CtxItem
      onclick={() => {
        if (staticDbCtx) {
          navigator.clipboard.writeText(staticDbCtx.database);
          staticDbCtx = null;
        }
      }}>Copy Name</CtxItem
    >
  </ContextMenu>
{/if}

{#if grpCtx}
  <ContextMenu x={grpCtx.x} y={grpCtx.y} open={true} onclose={closeAllCtx}>
    <CtxItem
      onclick={() => {
        if (grpCtx) {
          newConnectionGroupId = grpCtx.group.id;
          showAddForm = true;
          grpCtx = null;
        }
      }}>New Connection in Group</CtxItem
    >
    <CtxItem
      onclick={() => {
        if (grpCtx) {
          renamingGroupId = grpCtx.group.id;
          renameValue = grpCtx.group.name;
          renameError = '';
          grpCtx = null;
        }
      }}>Rename Group</CtxItem
    >
    {#if (grouped().byGroup.get(grpCtx.group.id) ?? []).length > 0}
      <CtxSep />
      <CtxItem
        onclick={() => {
          if (grpCtx) {
            const ids = (grouped().byGroup.get(grpCtx.group.id) ?? []).map((p) => p.id);
            exportPreselectIds = ids;
            grpCtx = null;
            showExportDialog = true;
          }
        }}>Export Connections…</CtxItem
      >
    {/if}
    <CtxSep />
    <CtxItem danger onclick={() => grpCtx && deleteGroup(grpCtx.group)}>Delete Group</CtxItem>
  </ContextMenu>
{/if}

{#if connCtx}
  {@const connConnected = isConnected(connCtx.profile.id)}
  <ContextMenu x={connCtx.x} y={connCtx.y} open={true} onclose={closeAllCtx}>
    <CtxItem onclick={ctxNewQueryEditor}>New Query Editor</CtxItem>
    {#if connConnected && !connCtx.profile.readOnly}
      {#if connCtx.profile.dialectInfo.hasInstanceDatabases}
        <CtxItem onclick={ctxNewDatabase}>New Database</CtxItem>
      {:else if connCtx.profile.dialectInfo.usesDatabaseKeyword}
        <CtxItem onclick={ctxNewDatabase}>New {connCtx.profile.dialectInfo.dbLabel}</CtxItem>
      {/if}
    {/if}
    {#if connCtx.profile.dialectInfo.supportsUserManagement}
      <CtxItem onclick={ctxManageUsers}>Manage Users</CtxItem>
    {/if}
    <CtxSep />
    <CtxItem
      onclick={() => {
        if (connCtx) {
          editingProfile = connCtx.profile;
          connCtx = null;
        }
      }}>Edit</CtxItem
    >
    <CtxItem
      onclick={async () => {
        if (connCtx) {
          const id = connCtx.profile.id;
          connCtx = null;
          try {
            await connectionsApi.duplicateConnection(id);
            await connectionStore.load();
          } catch (err) {
            errorModal = { title: 'Duplicate Failed', message: errorMessage(err) };
          }
        }
      }}>Duplicate</CtxItem
    >
    <CtxItem onclick={ctxConnToggleReadOnly}
      >{connCtx.profile.readOnly ? 'Disable Read Only' : 'Enable Read Only'}</CtxItem
    >
    <CtxItem
      onclick={async () => {
        if (connCtx) {
          const name = connCtx.profile.name;
          connCtx = null;
          try {
            await navigator.clipboard.writeText(name);
            toast.addToast('Name copied', 'success');
          } catch (err) {
            toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error', 0);
          }
        }
      }}>Copy Name</CtxItem
    >
    <CtxItem
      onclick={async () => {
        if (connCtx) {
          const id = connCtx.profile.id;
          connCtx = null;
          try {
            await connectionsApi.copyConnectionDbUrlToClipboard(id);
            toast.addToast('Database URL copied', 'success');
          } catch (err) {
            toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error', 0);
          }
        }
      }}>Copy as Database URL</CtxItem
    >
    <CtxItem
      onclick={() => {
        if (connCtx) {
          exportSingleId = connCtx.profile.id;
          connCtx = null;
          showExportDialog = true;
        }
      }}>Export Connection…</CtxItem
    >
    <CtxSep />
    <CtxItem
      onclick={() => {
        settingsStore.set('showSystemItems', !settingsStore.settings.showSystemItems);
        connCtx = null;
      }}
    >
      {settingsStore.settings.showSystemItems ? 'Hide System Items' : 'Show System Items'}
    </CtxItem>
    <CtxSep />
    {#if connConnected}
      <CtxItem onclick={ctxRefreshConnection}>Refresh</CtxItem>
      <CtxItem onclick={ctxConnDisconnect}>Disconnect</CtxItem>
    {:else}
      <CtxItem
        onclick={() => {
          if (connCtx) {
            handleConnect(connCtx.profile);
            connCtx = null;
          }
        }}>Connect</CtxItem
      >
    {/if}
    <CtxItem
      onclick={() => {
        if (connCtx) {
          panelStore.closeItemsForConnection(connCtx.profile.id);
          connCtx = null;
        }
      }}>Close All Tabs</CtxItem
    >
    <CtxSep />
    {#if connectionStore.groups.length > 0}
      <div
        class="ctx-item--submenu"
        role="menuitem"
        tabindex="0"
        aria-haspopup="true"
        onmouseenter={() => {
          if (moveToGroupSubmenuTimer) {
            clearTimeout(moveToGroupSubmenuTimer);
            moveToGroupSubmenuTimer = null;
          }
          moveToGroupSubmenuOpen = true;
        }}
        onmouseleave={() => {
          moveToGroupSubmenuTimer = setTimeout(() => {
            moveToGroupSubmenuOpen = false;
          }, 150);
        }}
      >
        Move to Group
        <ChevronIcon direction="right" width={10} height={10} strokeWidth={2.5} class="ctx-caret" />
        {#if moveToGroupSubmenuOpen}
          <div
            class="ctx-submenu"
            role="menu"
            tabindex="0"
            onmouseenter={() => {
              if (moveToGroupSubmenuTimer) {
                clearTimeout(moveToGroupSubmenuTimer);
                moveToGroupSubmenuTimer = null;
              }
            }}
            onmouseleave={() => {
              moveToGroupSubmenuTimer = setTimeout(() => {
                moveToGroupSubmenuOpen = false;
              }, 150);
            }}
          >
            {#each connectionStore.groups.filter((g) => g.id !== connCtx?.profile.groupId) as g (g.id)}
              <CtxItem onclick={() => ctxMoveToGroup(g.id)}>{g.name}</CtxItem>
            {/each}
          </div>
        {/if}
      </div>
      {#if connCtx.profile.groupId !== null}
        <CtxItem onclick={() => ctxMoveToGroup(null)}>Remove from Group</CtxItem>
      {/if}
      <CtxSep />
    {/if}
    <CtxItem
      onclick={() => {
        connCtx = null;
        startCreateGroup();
      }}>New Group</CtxItem
    >
    <CtxSep />
    <CtxItem
      danger
      onclick={() => {
        if (connCtx) {
          deleteConnection(connCtx.profile);
          connCtx = null;
        }
      }}>Delete</CtxItem
    >
  </ContextMenu>
{/if}

{#if createDbModal}
  {@const dbLabel = connectionStore.getById(createDbModal.connectionId)?.dialectInfo.dbLabel ?? 'Database'}
  <Modal
    label="New {dbLabel}"
    onbackdropclick={() => {
      createDbModal = null;
    }}
  >
    <div class="create-modal-card">
      <div class="create-modal-title">New {dbLabel}</div>
      <div class="create-modal-body">
        <label class="field-label" for="create-db-name">{dbLabel} Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="create-db-name"
          class="field-input"
          type="text"
          bind:value={createDbName}
          placeholder="my_database"
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          onkeydown={(e) => {
            if (e.key === 'Enter') executeCreateDatabase();
            if (e.key === 'Escape') createDbModal = null;
          }}
          autofocus
        />
        {#if createDbError}
          <div class="field-error">{createDbError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => (createDbModal = null)}>Cancel</button>
        <button class="btn btn--primary" onclick={executeCreateDatabase} disabled={createDbLoading}>
          {createDbLoading ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if createSchemaModal}
  <Modal
    label="New Schema"
    onbackdropclick={() => {
      createSchemaModal = null;
    }}
  >
    <div class="create-modal-card">
      <div class="create-modal-title">
        New Schema in <span class="create-modal-db">{createSchemaModal.instanceDb}</span>
      </div>
      <div class="create-modal-body">
        <label class="field-label" for="create-schema-name">Schema Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="create-schema-name"
          class="field-input"
          type="text"
          bind:value={createSchemaName}
          placeholder="my_schema"
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          onkeydown={(e) => {
            if (e.key === 'Enter') executeCreateSchema();
            if (e.key === 'Escape') createSchemaModal = null;
          }}
          autofocus
        />
        {#if createSchemaError}
          <div class="field-error">{createSchemaError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => (createSchemaModal = null)}>Cancel</button>
        <button class="btn btn--primary" onclick={executeCreateSchema} disabled={createSchemaLoading}>
          {createSchemaLoading ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if createGroupModal}
  <Modal
    label="New Group"
    onbackdropclick={() => {
      createGroupModal = false;
    }}
  >
    <div class="create-modal-card">
      <div class="create-modal-title">New Group</div>
      <div class="create-modal-body">
        <label class="field-label" for="create-group-name">Group Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="create-group-name"
          class="field-input"
          type="text"
          bind:value={newGroupName}
          placeholder="Production"
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          onkeydown={(e) => {
            if (e.key === 'Enter') commitCreateGroup();
            if (e.key === 'Escape') createGroupModal = false;
          }}
          autofocus
        />
        {#if newGroupError}
          <div class="field-error">{newGroupError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => (createGroupModal = false)}>Cancel</button>
        <button class="btn btn--primary" onclick={commitCreateGroup} disabled={newGroupLoading}>
          {newGroupLoading ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if renamingGroupId}
  <Modal
    label="Rename Group"
    onbackdropclick={() => {
      renamingGroupId = null;
    }}
  >
    <div class="create-modal-card">
      <div class="create-modal-title">Rename Group</div>
      <div class="create-modal-body">
        <label class="field-label" for="rename-group-name">Group Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="rename-group-name"
          class="field-input"
          type="text"
          bind:value={renameValue}
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck={false}
          onkeydown={(e) => {
            if (e.key === 'Enter') commitRename();
            if (e.key === 'Escape') renamingGroupId = null;
          }}
          autofocus
        />
        {#if renameError}
          <div class="field-error">{renameError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => (renamingGroupId = null)}>Cancel</button>
        <button class="btn btn--primary" onclick={commitRename} disabled={renameLoading}>
          {renameLoading ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if createTableModal}
  {@const types = connectionStore.getById(createTableModal.connectionId)?.dialectInfo.commonColumnTypes ?? []}
  <Modal
    label="New Table"
    onbackdropclick={() => {
      createTableModal = null;
    }}
  >
    <div class="create-modal-card create-modal-card--wide">
      <div class="create-modal-title">
        New Table in <span class="create-modal-db">{createTableModal.database}</span>
      </div>
      <div class="create-modal-body">
        <div class="field-group">
          <label class="field-label" for="create-table-name">Table Name</label>
          <input
            id="create-table-name"
            class="field-input"
            type="text"
            bind:value={createTableName}
            placeholder="my_table"
            autocomplete="off"
            autocapitalize="off"
            autocorrect="off"
            spellcheck={false}
          />
        </div>

        <div class="cols-section">
          <div class="cols-header">
            <span class="field-label">Columns</span>
            <button
              class="btn-add-col"
              onclick={() => {
                createTableColumns = [
                  ...createTableColumns,
                  { name: '', type: types[0], nullable: true, primaryKey: false },
                ];
              }}>+ Add Column</button
            >
          </div>
          <div class="cols-head-row">
            <span class="col-cell-name">Name</span>
            <span class="col-cell-type">Type</span>
            <span class="col-cell-flag">Null</span>
            <span class="col-cell-flag">PK</span>
            <span class="col-cell-del"></span>
          </div>
          {#each createTableColumns as col, i}
            <div class="col-row">
              <input
                class="field-input col-name-input"
                type="text"
                bind:value={col.name}
                placeholder="column_name"
                autocomplete="off"
                autocapitalize="off"
                autocorrect="off"
                spellcheck={false}
              />
              <Select
                bind:value={col.type}
                options={types.map((t) => ({ value: t, label: t }))}
                size="sm"
                mono={true}
                style="width:100%"
                searchable
              />
              <Checkbox bind:checked={col.nullable} size="sm" aria-label="Nullable" />
              <Checkbox bind:checked={col.primaryKey} size="sm" aria-label="Primary key" />
              <button
                class="col-del-btn"
                onclick={() => {
                  createTableColumns = createTableColumns.filter((_, idx) => idx !== i);
                }}
                disabled={createTableColumns.length <= 1}
                aria-label="Remove column"
              >
                <CloseIcon width={11} height={11} strokeWidth={2.2} />
              </button>
            </div>
          {/each}
        </div>

        {#if connectionStore.getById(createTableModal.connectionId)?.dialectInfo.usesSchema}
          <div class="cols-section">
            <div class="cols-header">
              <span class="field-label">Foreign Keys</span>
              <button
                class="btn-add-col"
                onclick={() => {
                  createTableFks = [
                    ...createTableFks,
                    {
                      localColumn: createTableColumns[0]?.name ?? '',
                      refTable: '',
                      refColumn: '',
                      onDelete: 'NO ACTION',
                      onUpdate: 'NO ACTION',
                    },
                  ];
                }}>+ Add FK</button
              >
            </div>
            {#each createTableFks as fk, i}
              <div class="fk-card">
                <div class="fk-row">
                  <Select
                    bind:value={fk.localColumn}
                    options={createTableColumns
                      .filter((c) => c.name.trim())
                      .map((c) => ({ value: c.name, label: c.name }))}
                    size="sm"
                    aria-label="Local column"
                    style="flex:1;min-width:0"
                    searchable
                  />
                  <span class="fk-arrow">→</span>
                  <Select
                    bind:value={fk.refTable}
                    options={(schemaCache
                      .get(createTableModal!.connectionId)
                      ?.get(createTableModal!.database) ?? []).map((t) => ({
                      value: t.name,
                      label: t.name,
                    }))}
                    size="sm"
                    aria-label="Referenced table"
                    style="flex:1;min-width:0"
                    searchable
                    placeholder="ref_table"
                    onchange={() => {
                      fk.refColumn = '';
                      loadFkRefColumns(i, fk.refTable);
                    }}
                  />
                  <Select
                    bind:value={fk.refColumn}
                    options={(fkRefColumns.get(i) ?? []).map((c) => ({ value: c, label: c }))}
                    size="sm"
                    aria-label="Referenced column"
                    style="flex:1;min-width:0"
                    searchable
                    placeholder="ref_col"
                    disabled={!fk.refTable}
                  />
                  <button
                    class="col-del-btn"
                    onclick={() => {
                      createTableFks = createTableFks.filter((_, idx) => idx !== i);
                      const updated = new Map(fkRefColumns);
                      updated.delete(i);
                      fkRefColumns = updated;
                    }}
                    aria-label="Remove foreign key"
                  >
                    <CloseIcon width={11} height={11} strokeWidth={2.2} />
                  </button>
                </div>
                <div class="fk-actions-row">
                  <span class="fk-action-label">ON DELETE</span>
                  <Select
                    bind:value={fk.onDelete}
                    options={refActions}
                    size="sm"
                    style="flex:1;min-width:0"
                  />
                  <span class="fk-action-label">ON UPDATE</span>
                  <Select
                    bind:value={fk.onUpdate}
                    options={refActions}
                    size="sm"
                    style="flex:1;min-width:0"
                  />
                </div>
              </div>
            {/each}
          </div>
        {/if}

        {#if createTableError}
          <div class="field-error">{createTableError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => (createTableModal = null)}>Cancel</button>
        <button class="btn btn--primary" onclick={executeCreateTable} disabled={createTableLoading}>
          {createTableLoading ? 'Creating…' : 'Create Table'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if showAddForm}
  <ConnectionForm
    groupId={newConnectionGroupId ?? null}
    onclose={() => {
      showAddForm = false;
      newConnectionGroupId = undefined;
    }}
  />
{/if}

{#if editingProfile}
  <ConnectionForm
    profile={editingProfile}
    onclose={() => (editingProfile = undefined)}
    ondelete={() => {
      if (editingProfile)
        deleteConnection(editingProfile, () => {
          editingProfile = undefined;
        });
    }}
  />
{/if}

{#if confirmState}
  <ConfirmDialog
    title={confirmState.title}
    message={confirmState.message}
    confirmText={confirmState.confirmText ?? 'Delete'}
    danger={true}
    onconfirm={confirmState.onconfirm}
    oncancel={() => (confirmState = null)}
  />
{/if}

{#if showExportDialog}
  <ExportConnectionsDialog
    profiles={connectionStore.profiles}
    preselectIds={exportPreselectIds ?? (exportSingleId ? [exportSingleId] : null)}
    onclose={() => {
      showExportDialog = false;
      exportSingleId = null;
      exportPreselectIds = null;
    }}
    onsuccess={(count) => {
      showExportDialog = false;
      exportSingleId = null;
      exportPreselectIds = null;
      toast.addToast(`Exported ${count} connection${count !== 1 ? 's' : ''}`, 'success');
    }}
    onerror={(msg) => {
      showExportDialog = false;
      exportSingleId = null;
      exportPreselectIds = null;
      toast.addToast(`Export failed: ${msg}`, 'error', 0);
    }}
  />
{/if}

{#if errorModal}
  <Modal label={errorModal.title} onbackdropclick={() => (errorModal = null)}>
    <div class="create-modal-card">
      <div class="create-modal-title">{errorModal.title}</div>
      <div class="create-modal-body">
        <p class="error-modal-message">{errorModal.message}</p>
      </div>
      <div class="create-modal-footer">
        <button class="btn btn--primary" onclick={() => (errorModal = null)}>OK</button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .connection-tree {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  /* ── Header ── */

  .tree-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 11px 14px 6px;
  }

  .header-label {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  .header-count {
    font-size: 10.5px;
    color: var(--color-text-disabled);
  }

  .spacer {
    flex: 1;
  }

  .header-icon-btn {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    flex-shrink: 0;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .header-icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Scroll container ── */

  .tree-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 8px 8px;
    min-height: 0;
  }

  /* ── Group headers ── */

  .group-section {
    margin-bottom: 2px;
  }

  .group-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 8px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
    text-transform: uppercase;
    text-align: left;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background var(--transition-fast);
    -webkit-user-select: none;
    user-select: none;
  }

  .group-row:hover {
    background: var(--color-bg-hover);
  }

  .group-name {
    flex: 1;
  }

  .group-count {
    font-size: 10px;
    color: var(--color-text-disabled);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    padding: 0 4px;
    font-weight: 400;
    text-transform: none;
    letter-spacing: normal;
  }

  /* ── Connection rows ── */

  .conn-item {
    margin-bottom: 3px;
  }

  .conn-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: var(--radius-lg);
    cursor: pointer;
    transition: background var(--transition-fast);
    position: relative;
  }

  .conn-row:hover {
    background: var(--color-bg-hover);
  }

  .conn-row-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
  }

  .conn-chevron {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition:
      transform var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
  }

  .conn-chevron.open :global(svg) {
    transform: rotate(90deg);
  }

  .conn-chevron :global(svg) {
    transition: transform var(--transition-fast);
  }

  .conn-chevron:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .color-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: opacity var(--transition-fast);
  }

  .color-dot.dim {
    opacity: 0.4;
  }

  .conn-name {
    flex: 0 1 auto;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--color-text-primary);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    cursor: pointer;
  }

  .conn-name:hover {
    color: var(--color-accent);
  }

  .lock-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    padding: 2px;
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    opacity: 0.6;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition:
      opacity var(--transition-fast),
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .lock-icon-btn:hover {
    opacity: 1;
    color: var(--color-accent);
    background: var(--color-bg-hover);
  }

  /* Hover-only action buttons */
  .conn-actions {
    display: flex;
    gap: 2px;
    align-items: center;
    opacity: 0;
    flex-shrink: 0;
    transition: opacity var(--transition-fast);
    flex: 0;
  }

  .conn-row:hover .conn-actions {
    opacity: 1;
  }

  .action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
  }

  .action-btn:hover {
    background: var(--color-bg-active);
    color: var(--color-text-primary);
  }

  /* ── Error / loading ── */

  .loading-row {
    padding: 4px 8px 4px 34px;
  }

  .loading-dots {
    font-size: 11px;
    color: var(--color-text-muted);
    animation: pulse 1s infinite;
  }

  /* ── Schema children ── */

  .schema-children {
    padding-left: 8px;
  }

  /* ── Database rows ── */

  .db-item {
    margin-bottom: 1px;
  }

  .db-row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 5px 8px 5px 8px;
    font-size: 12px;
    color: var(--color-text-secondary);
    text-align: left;
    border-radius: var(--radius-md);
    cursor: pointer;
    -webkit-user-select: none;
    user-select: none;
    transition: background var(--transition-fast);
    background: transparent;
  }

  .db-row:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .db-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .schema-item {
    padding-left: 8px;
  }

  .schema-row {
    font-style: italic;
  }


  /* ── Table rows ── */

  .table-list {
    padding-left: 8px;
  }

  .table-row {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 6px 10px 6px 18px;
    font-size: 12.5px;
    color: var(--color-text-secondary);
    text-align: left;
    border-radius: var(--radius-lg);
    cursor: pointer;
    -webkit-user-select: none;
    user-select: none;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
  }

  .table-row:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
  .table-row.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    box-shadow: inset 2px 0 0 var(--color-accent);
  }
  .table-row.active:hover {
    background: var(--color-accent-subtle);
  }
  .table-row.dragging {
    opacity: 0.5;
    cursor: grabbing;
  }

  .table-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-count {
    font-size: 10px;
    color: var(--color-text-disabled);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
  }

  /* ── System items ── */

  .system-item {
    opacity: 0.6;
  }
  .system-item:hover {
    opacity: 0.9;
  }

  /* ── Chevron shared ── */

  .chevron {
    flex-shrink: 0;
    color: var(--color-text-muted);
    display: flex;
    align-items: center;
  }

  .chevron :global(svg) {
    transition: transform var(--transition-fast);
  }
  .chevron.open :global(svg) {
    transform: rotate(90deg);
  }

  /* ── Add connection row ── */

  .add-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px;
    margin-top: 4px;
    border-radius: var(--radius-lg);
    font-size: 12.5px;
    font-weight: 500;
    color: var(--color-text-muted);
    cursor: pointer;
    text-align: left;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
  }

  .add-row:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Empty state ── */

  .empty-state {
    padding: 8px 10px;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }


  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }

  /* ── Context menu ── */

  .ctx-item--submenu {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    cursor: default;
    -webkit-user-select: none;
    user-select: none;
    transition: background var(--transition-fast);
  }
  .ctx-item--submenu:hover {
    background: var(--color-bg-active);
  }

  :global(.ctx-caret) {
    flex-shrink: 0;
    opacity: 0.6;
  }

  .ctx-submenu {
    position: absolute;
    top: -4px;
    left: 100%;
    min-width: 140px;
    padding: var(--spacing-1) 0;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    z-index: 1;
  }

  /* ── Create modals ── */

  .create-modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 340px;
    max-width: 94vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  .create-modal-card--wide {
    width: 520px;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .create-modal-title {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .create-modal-db {
    font-weight: var(--font-weight-normal);
    color: var(--color-text-secondary);
  }

  .create-modal-body {
    padding: var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
    max-height: 60vh;
    overflow-y: auto;
  }

  .create-modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

  .error-modal-message {
    margin: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: var(--line-height-normal);
    word-break: break-word;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.03em;
  }

  .field-input {
    height: 30px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    transition: border-color var(--transition-fast);
    width: 100%;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: var(--color-accent);
  }

  .field-error {
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-danger-subtle);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }

  /* Columns table */

  .cols-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .cols-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .btn-add-col {
    font-size: 11.5px;
    font-weight: 500;
    color: var(--color-accent);
    background: transparent;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    font-family: var(--font-family-ui);
    transition: background var(--transition-fast);
  }

  .btn-add-col:hover {
    background: var(--color-accent-subtle);
  }

  .cols-head-row {
    display: grid;
    grid-template-columns: 1fr 1fr 40px 40px 24px;
    gap: 6px;
    padding: 0 2px;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.03em;
  }

  .col-cell-flag {
    text-align: center;
  }

  .col-row {
    display: grid;
    grid-template-columns: 1fr 1fr 40px 40px 24px;
    gap: 6px;
    align-items: center;
  }

  .col-name-input {
    min-width: 0;
  }

  .col-del-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .col-del-btn:hover:not(:disabled) {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }
  .col-del-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }

  /* Shared modal buttons */

  .btn {
    height: 28px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    transition: all var(--transition-fast);
    white-space: nowrap;
    font-family: var(--font-family-ui);
  }

  .btn:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }
  .btn--primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  /* ── FK cards ── */

  .fk-card {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 8px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
  }

  .fk-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .fk-arrow {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--color-text-muted);
  }

  .fk-input {
    flex: 1;
    min-width: 0;
  }

  .fk-actions-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .fk-action-label {
    flex-shrink: 0;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--color-text-muted);
    letter-spacing: 0.03em;
  }

  /* ── Filter bar ── */

  .filter-bar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px 4px 12px;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  :global(.filter-icon) {
    flex-shrink: 0;
    color: var(--color-text-muted);
  }

  .filter-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    font-size: 12px;
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
    padding: 0;
  }

  .filter-input::placeholder {
    color: var(--color-text-disabled);
  }

  .filter-clear {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .filter-clear:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
