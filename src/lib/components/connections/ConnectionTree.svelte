<!--
  ConnectionTree — unified connection + schema tree.
  Shows all connections with expand-to-browse (databases → tables).
  Visual design matches Glass Workspace spec: chevron + color-dot-with-glow + name.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import ConnectionForm from './ConnectionForm.svelte';
  import DbIcon from '$lib/components/icons/DbIcon.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import { isSystemDatabase, isSystemTable } from '$lib/utils/system-items';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as schemaApi from '$lib/tauri/schema';
  import { errorMessage } from '$lib/utils/errors';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import { portal } from '$lib/actions/portal';
  import type { ConnectionProfile, ConnectionGroup, TableInfo } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const settingsStore = useSettings();

  // ── Add / edit forms ──────────────────────────────────────────────────────

  let showAddForm = $state(false);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);
  let newConnectionGroupId = $state<string | null | undefined>(undefined);

  // ── Schema state ──────────────────────────────────────────────────────────

  let schemaCache = $state<Map<string, Map<string, TableInfo[]>>>(new Map());
  let expandedConnections = $state<Set<string>>(new Set());
  let expandedDatabases = $state<Set<string>>(new Set());
  let loadingKeys = $state<Set<string>>(new Set());
  let loadErrors = $state<Map<string, string>>(new Map());

  // ── Group UI state ────────────────────────────────────────────────────────

  let expandedGroups = $state<Set<string>>(new Set());
  let ungroupedExpanded = $state(true);
  let renamingGroupId   = $state<string | null>(null);
  let renameValue       = $state('');
  let renameError       = $state('');
  let renameLoading     = $state(false);

  // ── Context menus ─────────────────────────────────────────────────────────

  interface TableCtxMenu  { x: number; y: number; connectionId: string; database: string; table: TableInfo }
  interface DbCtxMenu     { x: number; y: number; connectionId: string; database: string }
  interface GrpCtxMenu    { x: number; y: number; group: ConnectionGroup }
  interface ConnCtxMenu   { x: number; y: number; profile: ConnectionProfile }
  interface PanelCtxMenu  { x: number; y: number }

  let tableCtx  = $state<TableCtxMenu | null>(null);
  let dbCtx     = $state<DbCtxMenu | null>(null);
  let grpCtx    = $state<GrpCtxMenu | null>(null);
  let connCtx   = $state<ConnCtxMenu | null>(null);
  let panelCtx  = $state<PanelCtxMenu | null>(null);

  let moveToGroupSubmenuOpen = $state(false);
  let moveToGroupSubmenuTimer = $state<ReturnType<typeof setTimeout> | null>(null);

  interface ConfirmState { title: string; message: string; confirmText?: string; onconfirm: () => void }
  let confirmState = $state<ConfirmState | null>(null);
  let errorModal = $state<{ title: string; message: string } | null>(null);

  // ── Create modals ─────────────────────────────────────────────────────────

  interface CreateDbModal { connectionId: string; dbType: string }
  interface CreateTableColumn { name: string; type: string; nullable: boolean; primaryKey: boolean }
  interface CreateTableFk { localColumn: string; refTable: string; refColumn: string; onDelete: string; onUpdate: string }
  interface CreateTableModal { connectionId: string; database: string; dbType: string }

  let createDbModal   = $state<CreateDbModal | null>(null);
  let createDbName    = $state('');
  let createDbError   = $state('');
  let createDbLoading = $state(false);

  let createTableModal   = $state<CreateTableModal | null>(null);
  let createTableName    = $state('');
  let createTableColumns = $state<CreateTableColumn[]>([]);
  let createTableFks     = $state<CreateTableFk[]>([]);
  let createTableError   = $state('');
  let createTableLoading = $state(false);

  const refActions = [
    { value: 'NO ACTION', label: 'NO ACTION' },
    { value: 'RESTRICT',  label: 'RESTRICT'  },
    { value: 'CASCADE',   label: 'CASCADE'   },
    { value: 'SET NULL',  label: 'SET NULL'  },
  ];

  // ── Connection helpers ────────────────────────────────────────────────────

  function isConnected(id: string)   { return connectionStore.activeIds.has(id); }
  function isConnecting(id: string)  { return connectionStore.connectingIds.has(id); }
  function hasError(id: string)      { return connectionStore.errorIds.has(id); }
  function connError(id: string)     { return connectionStore.errorIds.get(id) ?? 'Connection error'; }
  function dotColor(profile: ConnectionProfile): string {
    return profile.color ?? '#7c5cff';
  }

  async function handleConnect(profile: ConnectionProfile) {
    if (isConnected(profile.id)) return;
    await connectionStore.connect(profile.id);
    // Auto-expand schema when connection succeeds
    if (isConnected(profile.id)) toggleExpand(profile.id);
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
          await connectionStore.load();
          onDone?.();
        } catch { /* ignore */ }
      },
    };
  }

  // ── Schema helpers ────────────────────────────────────────────────────────

  async function toggleExpand(connectionId: string) {
    if (expandedConnections.has(connectionId)) {
      expandedConnections = new Set([...expandedConnections].filter(id => id !== connectionId));
    } else {
      expandedConnections = new Set([...expandedConnections, connectionId]);
      await loadDatabases(connectionId);
    }
  }

  async function loadDatabases(connectionId: string) {
    if (schemaCache.has(connectionId)) return;
    loadingKeys = new Set([...loadingKeys, connectionId]);
    loadErrors = new Map([...loadErrors].filter(([k]) => k !== connectionId));
    try {
      const databases = await schemaApi.listDatabases(connectionId);
      const dbMap = new Map<string, TableInfo[]>();
      databases.forEach(db => dbMap.set(db, []));
      schemaCache = new Map([...schemaCache, [connectionId, dbMap]]);
    } catch (err) {
      loadErrors = new Map([...loadErrors, [connectionId, errorMessage(err)]]);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter(k => k !== connectionId));
    }
  }

  async function toggleDatabase(connectionId: string, database: string) {
    const key = `${connectionId}/${database}`;
    if (expandedDatabases.has(key)) {
      expandedDatabases = new Set([...expandedDatabases].filter(k => k !== key));
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
    loadErrors = new Map([...loadErrors].filter(([k]) => k !== key));
    try {
      const tables = await schemaApi.listTables(connectionId, database);
      const connMap = new Map(schemaCache.get(connectionId) ?? []);
      connMap.set(database, tables);
      schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
    } catch (err) {
      loadErrors = new Map([...loadErrors, [key, errorMessage(err)]]);
    } finally {
      loadingKeys = new Set([...loadingKeys].filter(k => k !== key));
    }
  }

  const focusedContent = $derived(panelStore.focusedPanel?.content);

  function isTableActive(connectionId: string, database: string, tableName: string): boolean {
    return focusedContent?.kind === 'table_browser' &&
      focusedContent.connectionId === connectionId &&
      focusedContent.database === database &&
      focusedContent.table === tableName;
  }

  function openTable(connectionId: string, database: string, table: string) {
    panelStore.openInFocused({ kind: 'table_browser', connectionId, database, table });
  }

  // ── Group helpers ─────────────────────────────────────────────────────────

  function toggleGroup(groupId: string) {
    if (expandedGroups.has(groupId)) {
      expandedGroups = new Set([...expandedGroups].filter(id => id !== groupId));
    } else {
      expandedGroups = new Set([...expandedGroups, groupId]);
    }
  }

  async function commitRename() {
    if (!renamingGroupId || !renameValue.trim()) { renamingGroupId = null; return; }
    renameLoading = true;
    renameError   = '';
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
        } catch { /* ignore */ }
      },
    };
  }

  // ── Group creation ────────────────────────────────────────────────────────

  let createGroupModal  = $state(false);
  let newGroupName      = $state('');
  let newGroupError     = $state('');
  let newGroupLoading   = $state(false);

  function startCreateGroup() {
    closeAllCtx();
    newGroupName  = '';
    newGroupError = '';
    createGroupModal = true;
  }

  async function commitCreateGroup() {
    const name = newGroupName.trim();
    if (!name) return;
    newGroupLoading = true;
    newGroupError   = '';
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
    tableCtx = null; dbCtx = null; grpCtx = null; connCtx = null; panelCtx = null;
    moveToGroupSubmenuOpen = false;
    if (moveToGroupSubmenuTimer) { clearTimeout(moveToGroupSubmenuTimer); moveToGroupSubmenuTimer = null; }
  }

  function showTableCtx(e: MouseEvent, connectionId: string, database: string, table: TableInfo) {
    e.preventDefault();
    closeAllCtx();
    tableCtx = { x: e.clientX, y: e.clientY, connectionId, database, table };
  }

  function showDbCtx(e: MouseEvent, connectionId: string, database: string) {
    e.preventDefault();
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
      poolMin: profile.poolMin,
      poolMax: profile.poolMax,
    });
  }

  async function ctxConnToggleReadOnly() {
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
      readOnly: !profile.readOnly,
      groupId: profile.groupId,
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
      poolMin: profile.poolMin,
      poolMax: profile.poolMax,
    });
  }

  async function ctxConnDisconnect() {
    if (!connCtx) return;
    const id = connCtx.profile.id;
    connCtx = null;
    await connectionStore.disconnect(id);
    expandedConnections = new Set([...expandedConnections].filter(i => i !== id));
  }

  function ctxManageUsers() {
    if (!connCtx) return;
    panelStore.openInFocused({ kind: 'user_manager', connectionId: connCtx.profile.id });
    connCtx = null;
  }

  function ctxOpenTable() {
    if (!tableCtx) return;
    openTable(tableCtx.connectionId, tableCtx.database, tableCtx.table.name);
    tableCtx = null;
  }

  function ctxViewDdl() {
    if (!tableCtx) return;
    panelStore.openInFocused({ kind: 'ddl_viewer', connectionId: tableCtx.connectionId, database: tableCtx.database, objectName: tableCtx.table.name, objectType: tableCtx.table.tableType });
    tableCtx = null;
  }

  function ctxCopyName() {
    if (!tableCtx) return;
    navigator.clipboard.writeText(tableCtx.table.name);
    tableCtx = null;
  }

  function ctxOpenErd() {
    if (!dbCtx) return;
    panelStore.openInFocused({ kind: 'erd', connectionId: dbCtx.connectionId, database: dbCtx.database });
    dbCtx = null;
  }

  function qi(name: string, dbType: string): string {
    if (dbType === 'mysql' || dbType === 'mariadb') return '`' + name.replace(/`/g, '``') + '`';
    return '"' + name.replace(/"/g, '""') + '"';
  }

  function ctxDropTable() {
    if (!tableCtx) return;
    const { connectionId, database, table } = tableCtx;
    const profile = connectionStore.getById(connectionId);
    tableCtx = null;
    if (!profile || profile.readOnly) return;
    const dbType = profile.dbType;
    const sql = dbType === 'sqlite'
      ? `DROP TABLE ${qi(table.name, dbType)}`
      : `DROP TABLE ${qi(database, dbType)}.${qi(table.name, dbType)}`;
    confirmState = {
      title: 'Drop Table',
      message: `Drop table "${table.name}"? This will permanently delete the table and all its data. This cannot be undone.`,
      confirmText: 'Drop Table',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
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
    const dbType = profile.dbType;
    let sql: string;
    if (dbType === 'mysql' || dbType === 'mariadb') {
      sql = `DROP DATABASE ${qi(database, dbType)}`;
    } else if (dbType === 'postgres') {
      sql = `DROP SCHEMA ${qi(database, dbType)} CASCADE`;
    } else {
      return;
    }
    confirmState = {
      title: dbType === 'postgres' ? 'Drop Schema' : 'Drop Database',
      message: `Drop ${dbType === 'postgres' ? 'schema' : 'database'} "${database}"? This will permanently delete all tables and data within it. This cannot be undone.`,
      confirmText: dbType === 'postgres' ? 'Drop Schema' : 'Drop Database',
      onconfirm: async () => {
        confirmState = null;
        try {
          await schemaApi.executeDdl(connectionId, sql);
          const connMap = new Map(schemaCache.get(connectionId) ?? []);
          connMap.delete(database);
          schemaCache = new Map([...schemaCache, [connectionId, connMap]]);
          expandedDatabases = new Set([...expandedDatabases].filter(k => k !== `${connectionId}/${database}`));
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
    createDbModal = { connectionId: profile.id, dbType: profile.dbType };
  }

  async function executeCreateDatabase() {
    if (!createDbModal) return;
    const name = createDbName.trim();
    if (!name) { createDbError = 'Name is required'; return; }
    const { connectionId, dbType } = createDbModal;
    const sql = dbType === 'postgres'
      ? `CREATE SCHEMA ${qi(name, dbType)}`
      : `CREATE DATABASE ${qi(name, dbType)}`;
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

  function defaultColumnType(dbType: string): string {
    if (dbType === 'postgres') return 'SERIAL';
    if (dbType === 'sqlite') return 'INTEGER';
    return 'INT';
  }

  function columnTypes(dbType: string): string[] {
    if (dbType === 'mysql' || dbType === 'mariadb') {
      return ['INT', 'BIGINT', 'SMALLINT', 'TINYINT', 'VARCHAR(255)', 'TEXT', 'LONGTEXT', 'DATETIME', 'DATE', 'FLOAT', 'DOUBLE', 'DECIMAL(10,2)', 'BOOLEAN', 'JSON'];
    } else if (dbType === 'postgres') {
      return ['INTEGER', 'BIGINT', 'SMALLINT', 'VARCHAR(255)', 'TEXT', 'TIMESTAMP', 'DATE', 'REAL', 'NUMERIC(10,2)', 'BOOLEAN', 'JSON', 'JSONB', 'UUID', 'SERIAL', 'BIGSERIAL'];
    }
    return ['INTEGER', 'TEXT', 'REAL', 'BLOB', 'NUMERIC'];
  }

  function ctxNewTable() {
    if (!dbCtx) return;
    const { connectionId, database } = dbCtx;
    const profile = connectionStore.getById(connectionId);
    dbCtx = null;
    if (!profile) return;
    createTableName = '';
    createTableError = '';
    createTableColumns = [{ name: 'id', type: defaultColumnType(profile.dbType), nullable: false, primaryKey: true }];
    createTableFks = [];
    createTableModal = { connectionId, database, dbType: profile.dbType };
  }

  async function executeCreateTable() {
    if (!createTableModal) return;
    const name = createTableName.trim();
    if (!name) { createTableError = 'Table name is required'; return; }
    const emptyCol = createTableColumns.find(c => !c.name.trim());
    if (emptyCol) { createTableError = 'All columns must have a name'; return; }
    const incompleteFk = createTableFks.find(fk => !fk.localColumn || !fk.refTable.trim() || !fk.refColumn.trim());
    if (incompleteFk) { createTableError = 'All foreign keys must have a local column, referenced table, and referenced column'; return; }
    const { connectionId, database, dbType } = createTableModal;
    const q = (n: string) => qi(n, dbType);
    const pkCols = createTableColumns.filter(c => c.primaryKey);
    const colDefs = createTableColumns.map(c => {
      let def = `  ${q(c.name.trim())} ${c.type}`;
      if (!c.nullable) def += ' NOT NULL';
      return def;
    });
    if (pkCols.length > 0) {
      colDefs.push(`  PRIMARY KEY (${pkCols.map(c => q(c.name.trim())).join(', ')})`);
    }
    for (const fk of createTableFks) {
      const fkName = `fk_${name}_${fk.localColumn}`;
      colDefs.push(`  CONSTRAINT ${q(fkName)} FOREIGN KEY (${q(fk.localColumn)}) REFERENCES ${q(fk.refTable.trim())} (${q(fk.refColumn.trim())}) ON DELETE ${fk.onDelete} ON UPDATE ${fk.onUpdate}`);
    }
    const tablePath = dbType === 'sqlite' ? q(name) : `${q(database)}.${q(name)}`;
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

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { closeAllCtx(); renamingGroupId = null; }
  }

  function handleWindowClick(e: MouseEvent) {
    if (!(e.target as Element | null)?.closest('.ctx-menu')) closeAllCtx();
  }

  // ── System database / table detection ────────────────────────────────────

  function checkSystemDatabase(name: string) {
    return isSystemDatabase(name, settingsStore.settings.systemDatabases);
  }
  function checkSystemTable(name: string) {
    return isSystemTable(name, settingsStore.settings.systemTablePatterns);
  }

  // ── Derived groupings ─────────────────────────────────────────────────────

  const grouped = $derived(() => {
    const groups = connectionStore.groups;
    const profiles = connectionStore.profiles;
    const ungrouped = profiles.filter(p => p.groupId === null);
    const byGroup = new Map<string, ConnectionProfile[]>();
    for (const g of groups) byGroup.set(g.id, []);
    for (const p of profiles) {
      if (p.groupId !== null && byGroup.has(p.groupId)) byGroup.get(p.groupId)!.push(p);
    }
    return { groups, ungrouped, byGroup };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="connection-tree">
  <!-- Section header -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="tree-header no-select" oncontextmenu={showPanelCtx}>
    <span class="header-label">CONNECTIONS</span>
    <span class="header-count">{connectionStore.profiles.length}</span>
  </div>

  <!-- Scrollable list -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="tree-scroll gscroll" oncontextmenu={(e) => { if ((e.target as Element).closest('.conn-item,.group-section,.ctx-menu')) return; showPanelCtx(e); }}>
    {#if connectionStore.profiles.length === 0 && connectionStore.groups.length === 0}
      <div class="empty-state">
        <p>No connections yet.</p>
      </div>
    {:else}

      <!-- Ungrouped profiles -->
      {#if connectionStore.groups.length > 0 && grouped().ungrouped.length > 0}
        <div class="group-section">
          <button class="group-row" onclick={() => (ungroupedExpanded = !ungroupedExpanded)}>
            <span class="chevron" class:open={ungroupedExpanded} aria-hidden="true">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
            </span>
            <span class="group-name">Ungrouped</span>
            <span class="group-count">{grouped().ungrouped.length}</span>
          </button>
          {#if ungroupedExpanded}
            {#each grouped().ungrouped as profile (profile.id)}
              {@render connectionRow(profile)}
            {/each}
          {/if}
        </div>
      {:else}
        {#each grouped().ungrouped as profile (profile.id)}
          {@render connectionRow(profile)}
        {/each}
      {/if}

      <!-- Named groups -->
      {#each grouped().groups as group (group.id)}
        {@const isExpanded = expandedGroups.has(group.id)}
        {@const groupProfiles = grouped().byGroup.get(group.id) ?? []}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="group-section" oncontextmenu={(e) => showGrpCtx(e, group)}>
          <button class="group-row" onclick={() => toggleGroup(group.id)}>
            <span class="chevron" class:open={isExpanded} aria-hidden="true">
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
            </span>
            <span class="group-name">{group.name}</span>
            <span class="group-count">{groupProfiles.length}</span>
          </button>
          {#if isExpanded}
            {#each groupProfiles as profile (profile.id)}
              {@render connectionRow(profile)}
            {/each}
          {/if}
        </div>
      {/each}

    {/if}

    <!-- Add connection inline row -->
    <button class="add-row" onclick={() => { newConnectionGroupId = undefined; showAddForm = true; }}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
        <line x1="12" y1="5" x2="12" y2="19"></line>
        <line x1="5" y1="12" x2="19" y2="12"></line>
      </svg>
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
    <div class="conn-row" class:connected class:errored oncontextmenu={(e) => showConnCtx(e, profile)} onclick={() => connected ? toggleExpand(profile.id) : handleConnect(profile)}>
      <div class="conn-row-left">
        <!-- Chevron: rotates when expanded -->
        <button
          class="conn-chevron"
          class:open={expanded}
          onclick={(e) => { e.stopPropagation(); connected ? toggleExpand(profile.id) : handleConnect(profile); }}
          aria-label="{expanded ? 'Collapse' : 'Expand'} {profile.name}"
          disabled={connecting}
        >
          {#if connecting}
            <span class="spin-ring" aria-hidden="true"></span>
          {:else}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="9 18 15 12 9 6"></polyline>
            </svg>
          {/if}
        </button>

        <!-- Color dot with glow -->
        <span
          class="color-dot"
          class:dim={!connected && !connecting}
          style="background:{color};{connected ? `box-shadow:0 0 0 3px color-mix(in srgb,${color} 18%,transparent)` : ''}"
          aria-hidden="true"
        ></span>

        <!-- Name -->
        <button
          class="conn-name"
          onclick={(e) => { e.stopPropagation(); connected ? panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id }) : handleConnect(profile); }}
          title={profile.host}
        >{profile.name}</button>

        <!-- Lock icon if read-only -->
        {#if profile.readOnly}
          <svg class="lock-icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-label="Read-only">
            <rect x="5" y="11" width="14" height="9" rx="2"></rect>
            <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
          </svg>
        {/if}
      </div>

      <!-- Hover actions -->
      <div class="conn-actions">
        <button
          class="action-btn"
          onclick={(e) => { e.stopPropagation(); editingProfile = profile; }}
          title="Edit connection"
          aria-label="Edit {profile.name}"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
          </svg>
        </button>
      </div>
    </div>

    <!-- Error banner -->
    {#if errored}
      <div class="error-row">
        <span class="error-msg">{connError(profile.id)}</span>
        <button class="retry-btn" onclick={() => handleConnect(profile)}>Retry</button>
      </div>
    {/if}

    <!-- Schema tree when expanded -->
    {#if expanded && connected}
      {@const databases = schemaCache.get(profile.id)}
      {@const isLoadingConn = loadingKeys.has(profile.id)}
      {@const connLoadError = loadErrors.get(profile.id)}

      <div class="schema-children">
        {#if isLoadingConn}
          <div class="loading-row">
            <span class="loading-dots" aria-label="Loading">Loading…</span>
          </div>
        {:else if connLoadError}
          <div class="load-error">{connLoadError}</div>
        {:else if databases}
          {#each [...databases.keys()].filter(db => settingsStore.settings.showSystemItems || !checkSystemDatabase(db)) as database}
            {@const dbKey = `${profile.id}/${database}`}
            {@const isDbExpanded = expandedDatabases.has(dbKey)}
            {@const isDbLoading = loadingKeys.has(dbKey)}
            {@const tables = databases.get(database) ?? []}
            {@const dbLoadError = loadErrors.get(dbKey)}

            {@const isDbSystem = checkSystemDatabase(database)}
            <div class="db-item" class:system-item={isDbSystem}>
              <button
                class="db-row"
                class:open={isDbExpanded}
                onclick={() => toggleDatabase(profile.id, database)}
                oncontextmenu={(e) => showDbCtx(e, profile.id, database)}
                aria-label="{isDbExpanded ? 'Collapse' : 'Expand'} {database}"
              >
                <span class="chevron" class:open={isDbExpanded} aria-hidden="true">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
                </span>
                <DbIcon system={isDbSystem} aria-hidden="true" />
                <span class="db-name">{database}</span>
                {#if isDbLoading}
                  <span class="loading-dots" aria-label="Loading">…</span>
                {/if}
              </button>

              {#if dbLoadError}
                <div class="load-error db-load-error">{dbLoadError}</div>
              {/if}

              {#if isDbExpanded && tables.length > 0}
                <div class="table-list">
                  {#each tables.filter(t => settingsStore.settings.showSystemItems || !(isDbSystem || checkSystemTable(t.name))) as table}
                    {@const isTableSystem = isDbSystem || checkSystemTable(table.name)}
                    <button
                      class="table-row"
                      class:system-item={isTableSystem}
                      class:active={isTableActive(profile.id, database, table.name)}
                      onclick={() => openTable(profile.id, database, table.name)}
                      oncontextmenu={(e) => showTableCtx(e, profile.id, database, table)}
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
    {/if}
  </div>
{/snippet}

<!-- Context menus -->
{#if panelCtx}
  <div class="ctx-menu" role="menu" style="top:{panelCtx.y}px;left:{panelCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={() => { panelCtx = null; newConnectionGroupId = undefined; showAddForm = true; }}>New Connection</button>
    <button class="ctx-item" role="menuitem" onclick={startCreateGroup}>New Group</button>
  </div>
{/if}

{#if tableCtx}
  {@const tableCtxProfile = connectionStore.getById(tableCtx.connectionId)}
  <div class="ctx-menu" role="menu" style="top:{tableCtx.y}px;left:{tableCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={ctxOpenTable}>Open Table</button>
    <button class="ctx-item" role="menuitem" onclick={ctxViewDdl}>View DDL</button>
    <button class="ctx-item" role="menuitem" onclick={ctxCopyName}>Copy Name</button>
    {#if !tableCtxProfile?.readOnly}
      <div class="ctx-sep" role="separator"></div>
      <button class="ctx-item ctx-item--danger" role="menuitem" onclick={ctxDropTable}>Drop Table</button>
    {/if}
  </div>
{/if}

{#if dbCtx}
  {@const dbCtxProfile = connectionStore.getById(dbCtx.connectionId)}
  <div class="ctx-menu" role="menu" style="top:{dbCtx.y}px;left:{dbCtx.x}px" use:portal>
    {#if !dbCtxProfile?.readOnly}
      <button class="ctx-item" role="menuitem" onclick={ctxNewTable}>New Table</button>
      <div class="ctx-sep" role="separator"></div>
    {/if}
    <button class="ctx-item" role="menuitem" onclick={ctxOpenErd}>Open ERD</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item" role="menuitem" onclick={() => { if (dbCtx) { navigator.clipboard.writeText(dbCtx.database); dbCtx = null; } }}>Copy Name</button>
    {#if !dbCtxProfile?.readOnly && dbCtxProfile?.dbType !== 'sqlite'}
      <div class="ctx-sep" role="separator"></div>
      <button class="ctx-item ctx-item--danger" role="menuitem" onclick={ctxDropDatabase}>
        {dbCtxProfile?.dbType === 'postgres' ? 'Drop Schema' : 'Drop Database'}
      </button>
    {/if}
  </div>
{/if}

{#if grpCtx}
  <div class="ctx-menu" role="menu" style="top:{grpCtx.y}px;left:{grpCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={() => { if (grpCtx) { newConnectionGroupId = grpCtx.group.id; showAddForm = true; grpCtx = null; } }}>New Connection in Group</button>
    <button class="ctx-item" role="menuitem" onclick={() => { if (grpCtx) { renamingGroupId = grpCtx.group.id; renameValue = grpCtx.group.name; renameError = ''; grpCtx = null; } }}>Rename Group</button>
    <button class="ctx-item ctx-item--danger" role="menuitem" onclick={() => grpCtx && deleteGroup(grpCtx.group)}>Delete Group</button>
  </div>
{/if}

{#if connCtx}
  {@const connConnected = isConnected(connCtx.profile.id)}
  <div class="ctx-menu" role="menu" style="top:{connCtx.y}px;left:{connCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { panelStore.openInFocused({ kind: 'query_editor', connectionId: connCtx.profile.id }); connCtx = null; } }}>New Query Editor</button>
    {#if connConnected && !connCtx.profile.readOnly && connCtx.profile.dbType !== 'sqlite'}
      <button class="ctx-item" role="menuitem" onclick={ctxNewDatabase}>New {connCtx.profile.dbType === 'postgres' ? 'Schema' : 'Database'}</button>
    {/if}
    <button class="ctx-item" role="menuitem" onclick={ctxManageUsers}>Manage Users</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { editingProfile = connCtx.profile; connCtx = null; } }}>Edit</button>
    <button class="ctx-item" role="menuitem" onclick={ctxConnToggleReadOnly}>{connCtx.profile.readOnly ? 'Disable Read Only' : 'Enable Read Only'}</button>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { navigator.clipboard.writeText(connCtx.profile.name); connCtx = null; } }}>Copy Name</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item" role="menuitem" onclick={() => { settingsStore.set('showSystemItems', !settingsStore.settings.showSystemItems); connCtx = null; }}>
      {settingsStore.settings.showSystemItems ? 'Hide System Items' : 'Show System Items'}
    </button>
    <div class="ctx-sep" role="separator"></div>
    {#if connConnected}
      <button class="ctx-item" role="menuitem" onclick={ctxConnDisconnect}>Disconnect</button>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { handleConnect(connCtx.profile); connCtx = null; } }}>Connect</button>
    {/if}
    <div class="ctx-sep" role="separator"></div>
    {#if connCtx.profile.groupId !== null}
      <button class="ctx-item" role="menuitem" onclick={() => ctxMoveToGroup(null)}>Remove from Group</button>
    {/if}
    {#if connectionStore.groups.length > 0}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="ctx-item ctx-item--submenu"
        role="menuitem"
        aria-haspopup="true"
        onmouseenter={() => {
          if (moveToGroupSubmenuTimer) { clearTimeout(moveToGroupSubmenuTimer); moveToGroupSubmenuTimer = null; }
          moveToGroupSubmenuOpen = true;
        }}
        onmouseleave={() => {
          moveToGroupSubmenuTimer = setTimeout(() => { moveToGroupSubmenuOpen = false; }, 150);
        }}
      >
        Move to Group
        <svg class="ctx-caret" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
        {#if moveToGroupSubmenuOpen}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="ctx-submenu"
            role="menu"
            onmouseenter={() => {
              if (moveToGroupSubmenuTimer) { clearTimeout(moveToGroupSubmenuTimer); moveToGroupSubmenuTimer = null; }
            }}
            onmouseleave={() => {
              moveToGroupSubmenuTimer = setTimeout(() => { moveToGroupSubmenuOpen = false; }, 150);
            }}
          >
            {#each connectionStore.groups.filter(g => g.id !== connCtx.profile.groupId) as g (g.id)}
              <button class="ctx-item" role="menuitem" onclick={() => ctxMoveToGroup(g.id)}>{g.name}</button>
            {/each}
          </div>
        {/if}
      </div>
      <div class="ctx-sep" role="separator"></div>
    {/if}
    <button class="ctx-item" role="menuitem" onclick={() => { connCtx = null; startCreateGroup(); }}>New Group</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item ctx-item--danger" role="menuitem" onclick={() => { if (connCtx) { deleteConnection(connCtx.profile); connCtx = null; } }}>Delete</button>
  </div>
{/if}

{#if createDbModal}
  {@const dbLabel = createDbModal.dbType === 'postgres' ? 'Schema' : 'Database'}
  <Modal label="New {dbLabel}" onbackdropclick={() => { createDbModal = null; }}>
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
          onkeydown={(e) => { if (e.key === 'Enter') executeCreateDatabase(); if (e.key === 'Escape') createDbModal = null; }}
          autofocus
        />
        {#if createDbError}
          <div class="field-error">{createDbError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => createDbModal = null}>Cancel</button>
        <button class="btn btn--primary" onclick={executeCreateDatabase} disabled={createDbLoading}>
          {createDbLoading ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if createGroupModal}
  <Modal label="New Group" onbackdropclick={() => { createGroupModal = false; }}>
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
          onkeydown={(e) => { if (e.key === 'Enter') commitCreateGroup(); if (e.key === 'Escape') createGroupModal = false; }}
          autofocus
        />
        {#if newGroupError}
          <div class="field-error">{newGroupError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => createGroupModal = false}>Cancel</button>
        <button class="btn btn--primary" onclick={commitCreateGroup} disabled={newGroupLoading}>
          {newGroupLoading ? 'Creating…' : 'Create'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if renamingGroupId}
  <Modal label="Rename Group" onbackdropclick={() => { renamingGroupId = null; }}>
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
          onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renamingGroupId = null; }}
          autofocus
        />
        {#if renameError}
          <div class="field-error">{renameError}</div>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn" onclick={() => renamingGroupId = null}>Cancel</button>
        <button class="btn btn--primary" onclick={commitRename} disabled={renameLoading}>
          {renameLoading ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

{#if createTableModal}
  {@const types = columnTypes(createTableModal.dbType)}
  <Modal label="New Table" onbackdropclick={() => { createTableModal = null; }}>
    <div class="create-modal-card create-modal-card--wide">
      <div class="create-modal-title">New Table in <span class="create-modal-db">{createTableModal.database}</span></div>
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
              onclick={() => { createTableColumns = [...createTableColumns, { name: '', type: types[0], nullable: true, primaryKey: false }]; }}
            >+ Add Column</button>
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
              <input class="field-input col-name-input" type="text" bind:value={col.name} placeholder="column_name" autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck={false} />
              <Select bind:value={col.type} options={types.map(t => ({ value: t, label: t }))} size="sm" mono={true} style="width:100%" />
              <Checkbox bind:checked={col.nullable} size="sm" aria-label="Nullable" />
              <Checkbox bind:checked={col.primaryKey} size="sm" aria-label="Primary key" />
              <button
                class="col-del-btn"
                onclick={() => { createTableColumns = createTableColumns.filter((_, idx) => idx !== i); }}
                disabled={createTableColumns.length <= 1}
                aria-label="Remove column"
              >
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
              </button>
            </div>
          {/each}
        </div>

        {#if createTableModal.dbType !== 'sqlite'}
          <div class="cols-section">
            <div class="cols-header">
              <span class="field-label">Foreign Keys</span>
              <button
                class="btn-add-col"
                onclick={() => { createTableFks = [...createTableFks, { localColumn: createTableColumns[0]?.name ?? '', refTable: '', refColumn: '', onDelete: 'NO ACTION', onUpdate: 'NO ACTION' }]; }}
              >+ Add FK</button>
            </div>
            {#each createTableFks as fk, i}
              <div class="fk-card">
                <div class="fk-row">
                  <Select
                    bind:value={fk.localColumn}
                    options={createTableColumns.filter(c => c.name.trim()).map(c => ({ value: c.name, label: c.name }))}
                    size="sm"
                    aria-label="Local column"
                    style="flex:1;min-width:0"
                  />
                  <span class="fk-arrow">→</span>
                  <input
                    class="field-input fk-input"
                    type="text"
                    bind:value={fk.refTable}
                    placeholder="ref_table"
                    autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck={false}
                  />
                  <input
                    class="field-input fk-input"
                    type="text"
                    bind:value={fk.refColumn}
                    placeholder="ref_col"
                    autocomplete="off" autocapitalize="off" autocorrect="off" spellcheck={false}
                  />
                  <button
                    class="col-del-btn"
                    onclick={() => { createTableFks = createTableFks.filter((_, idx) => idx !== i); }}
                    aria-label="Remove foreign key"
                  >
                    <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
                  </button>
                </div>
                <div class="fk-actions-row">
                  <span class="fk-action-label">ON DELETE</span>
                  <Select bind:value={fk.onDelete} options={refActions} size="sm" style="flex:1;min-width:0" />
                  <span class="fk-action-label">ON UPDATE</span>
                  <Select bind:value={fk.onUpdate} options={refActions} size="sm" style="flex:1;min-width:0" />
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
        <button class="btn" onclick={() => createTableModal = null}>Cancel</button>
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
    onclose={() => { showAddForm = false; newConnectionGroupId = undefined; }}
  />
{/if}

{#if editingProfile}
  <ConnectionForm
    profile={editingProfile}
    onclose={() => (editingProfile = undefined)}
    ondelete={() => { if (editingProfile) deleteConnection(editingProfile, () => { editingProfile = undefined; }); }}
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
    padding: 4px 8px;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
    text-transform: uppercase;
    text-align: left;
    border-radius: 6px;
    cursor: pointer;
    transition: background var(--transition-fast);
    user-select: none;
  }

  .group-row:hover { background: var(--color-bg-hover); }

  .group-name { flex: 1; }

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

  .rename-input {
    width: 100%;
    height: 24px;
    padding: 0 var(--spacing-2);
    margin: 2px 0;
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: 10.5px;
    font-family: var(--font-family-ui);
    outline: none;
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
    border-radius: 8px;
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
    border-radius: 4px;
    cursor: pointer;
    transition: transform var(--transition-fast), color var(--transition-fast);
    background: transparent;
  }

  .conn-chevron.open svg {
    transform: rotate(90deg);
  }

  .conn-chevron svg {
    transition: transform var(--transition-fast);
  }

  .conn-chevron:disabled { opacity: 0.5; cursor: default; }

  .color-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: opacity var(--transition-fast);
  }

  .color-dot.dim { opacity: 0.4; }

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

  .conn-name:hover { color: var(--color-accent); }

  .lock-icon {
    flex-shrink: 0;
    color: var(--color-text-muted);
    opacity: 0.6;
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

  .conn-row:hover .conn-actions { opacity: 1; }

  .action-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 5px;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
    background: transparent;
  }

  .action-btn:hover { background: var(--color-bg-active); color: var(--color-text-primary); }
  .action-btn--danger:hover { color: var(--color-danger); }

  /* ── Error / loading ── */

  .error-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 8px 4px 34px;
    font-size: 11px;
    color: var(--color-danger);
  }

  .error-msg {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .retry-btn {
    flex-shrink: 0;
    height: 16px;
    padding: 0 6px;
    background: transparent;
    border: 1px solid var(--color-danger);
    border-radius: var(--radius-sm);
    font-size: 10px;
    font-family: var(--font-family-ui);
    color: var(--color-danger);
    cursor: pointer;
    line-height: 1;
    transition: background var(--transition-fast);
  }

  .retry-btn:hover { background: var(--color-danger-subtle); }

  .loading-row {
    padding: 4px 8px 4px 34px;
  }

  .loading-dots {
    font-size: 11px;
    color: var(--color-text-muted);
    animation: pulse 1s infinite;
  }

  .load-error {
    padding: 3px 8px 3px 34px;
    font-size: 11px;
    color: var(--color-danger);
    word-break: break-word;
  }

  .db-load-error {
    padding-left: 50px;
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
    border-radius: 7px;
    cursor: pointer;
    user-select: none;
    transition: background var(--transition-fast);
    background: transparent;
  }

  .db-row:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  .db-icon { flex-shrink: 0; color: var(--color-text-muted); }
  .db-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

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
    border-radius: 8px;
    cursor: pointer;
    user-select: none;
    transition: background var(--transition-fast), color var(--transition-fast);
    background: transparent;
  }

  .table-row:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }
  .table-row.active { background: var(--color-accent-subtle); color: var(--color-accent); box-shadow: inset 2px 0 0 var(--color-accent); }
  .table-row.active:hover { background: var(--color-accent-subtle); }

  .table-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .row-count {
    font-size: 10px;
    color: var(--color-text-disabled);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
  }

  /* ── System items ── */

  .system-item { opacity: 0.6; }
  .system-item:hover { opacity: 0.9; }

  /* ── Chevron shared ── */

  .chevron {
    flex-shrink: 0;
    color: var(--color-text-muted);
    display: flex;
    align-items: center;
  }

  .chevron svg { transition: transform var(--transition-fast); }
  .chevron.open svg { transform: rotate(90deg); }

  /* ── Add connection row ── */

  .add-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px;
    margin-top: 4px;
    border-radius: 8px;
    font-size: 12.5px;
    font-weight: 500;
    color: var(--color-text-muted);
    cursor: pointer;
    text-align: left;
    transition: background var(--transition-fast), color var(--transition-fast);
    background: transparent;
  }

  .add-row:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  /* ── Empty state ── */

  .empty-state {
    padding: 8px 10px;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  /* ── Spinning indicator ── */

  .spin-ring {
    display: block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
    border-top-color: var(--color-accent);
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  /* ── Context menu ── */

  .ctx-menu {
    position: fixed;
    z-index: 500;
    min-width: 160px;
    padding: var(--spacing-1) 0;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
  }

  .ctx-item {
    display: block;
    width: 100%;
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
    background: transparent;
  }

  .ctx-item:hover { background: var(--color-bg-active); }
  .ctx-item--danger { color: var(--color-danger); }
  .ctx-item--active { color: var(--color-accent); }

  .ctx-item--submenu {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: default;
    user-select: none;
  }
  .ctx-item--submenu:hover { background: var(--color-bg-active); }

  .ctx-caret {
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

  .ctx-sep {
    height: 1px;
    margin: var(--spacing-1) 0;
    background: var(--color-border);
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
    from { opacity: 0; transform: scale(0.96) translateY(-6px); }
    to   { opacity: 1; transform: scale(1)    translateY(0); }
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

  .field-input:focus { border-color: var(--color-accent); }

  .field-error {
    font-size: 11.5px;
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

  .btn-add-col:hover { background: var(--color-accent-subtle); }

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

  .col-cell-flag { text-align: center; }

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
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .col-del-btn:hover:not(:disabled) { background: var(--color-danger-subtle); color: var(--color-danger); }
  .col-del-btn:disabled { opacity: 0.3; cursor: default; }

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

  .btn--primary:hover { opacity: 0.88; border-color: var(--color-accent); background: var(--color-accent); color: white; }
  .btn--primary:disabled { opacity: 0.5; cursor: default; }

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
</style>
