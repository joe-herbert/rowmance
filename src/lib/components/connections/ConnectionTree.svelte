<!--
  ConnectionTree — unified connection + schema tree.
  Shows all connections with expand-to-browse (databases → tables).
  Visual design matches Glass Workspace spec: chevron + color-dot-with-glow + name.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import ConnectionForm from './ConnectionForm.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as schemaApi from '$lib/tauri/schema';
  import { ask } from '@tauri-apps/plugin-dialog';
  import { errorMessage } from '$lib/utils/errors';
  import { portal } from '$lib/actions/portal';
  import type { ConnectionProfile, ConnectionGroup, TableInfo } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();

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
  let renamingGroupId = $state<string | null>(null);
  let renameValue = $state('');

  // ── Context menus ─────────────────────────────────────────────────────────

  interface TableCtxMenu { x: number; y: number; connectionId: string; database: string; table: TableInfo }
  interface DbCtxMenu    { x: number; y: number; connectionId: string; database: string }
  interface GrpCtxMenu   { x: number; y: number; group: ConnectionGroup }
  interface ConnCtxMenu  { x: number; y: number; profile: ConnectionProfile }

  let tableCtx = $state<TableCtxMenu | null>(null);
  let dbCtx    = $state<DbCtxMenu | null>(null);
  let grpCtx   = $state<GrpCtxMenu | null>(null);
  let connCtx  = $state<ConnCtxMenu | null>(null);

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

  async function deleteConnection(profile: ConnectionProfile) {
    if (!await ask(`Delete "${profile.name}"? This cannot be undone.`, { title: 'Delete Connection', kind: 'warning' })) return;
    if (connectionStore.isActive(profile.id)) await connectionStore.disconnect(profile.id);
    try {
      await connectionsApi.deleteConnection(profile.id);
      await connectionStore.load();
    } catch { /* ignore */ }
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
    try {
      await connectionsApi.updateConnectionGroup(renamingGroupId, { name: renameValue.trim() });
      await connectionStore.load();
    } catch { /* ignore */ } finally { renamingGroupId = null; }
  }

  async function deleteGroup(group: ConnectionGroup) {
    grpCtx = null;
    if (!await ask(`Delete group "${group.name}" and all its connections? This cannot be undone.`, { title: 'Delete Group', kind: 'warning' })) return;
    try {
      await connectionsApi.deleteConnectionGroup(group.id);
      await connectionStore.load();
    } catch { /* ignore */ }
  }

  // ── Context menu helpers ──────────────────────────────────────────────────

  function closeAllCtx() { tableCtx = null; dbCtx = null; grpCtx = null; connCtx = null; }

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
    closeAllCtx();
    connCtx = { x: e.clientX, y: e.clientY, profile };
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

  function handleWindowKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') { closeAllCtx(); if (renamingGroupId) renamingGroupId = null; }
  }

  function handleWindowClick(e: MouseEvent) {
    if (!(e.target as Element | null)?.closest('.ctx-menu')) closeAllCtx();
  }

  // ── System database / table detection ────────────────────────────────────

  const SYSTEM_DATABASES = new Set([
    // MySQL / MariaDB
    'information_schema', 'mysql', 'performance_schema', 'sys',
    // PostgreSQL
    'postgres', 'template0', 'template1',
    // SQL Server
    'master', 'model', 'msdb', 'tempdb',
  ]);
  const SYSTEM_TABLE_PATTERNS = [
    // Generic
    /^migrations$/i,
    // Drizzle
    /^__drizzle_migrations$/i,
    // Prisma
    /^_prisma_migrations$/i,
    // Rails / ActiveRecord
    /^schema_migrations$/i, /^ar_internal_metadata$/i,
    // Django
    /^django_migrations$/i,
    // Laravel
    /^laravel_migrations$/i,
    // Flyway
    /^flyway_schema_history$/i,
    // Liquibase
    /^databasechangelog(lock)?$/i,
    // Knex
    /^knex_migrations(_(lock))?$/i,
    // Sequelize
    /^sequelize_meta$/i,
    // TypeORM
    /^typeorm_metadata$/i,
    // Alembic
    /^alembic_version$/i,
    // Goose
    /^goose_db_version$/i,
  ];

  function isSystemDatabase(name: string) { return SYSTEM_DATABASES.has(name.toLowerCase()); }
  function isSystemTable(name: string) { return SYSTEM_TABLE_PATTERNS.some(p => p.test(name)); }

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

<div class="connection-tree">
  <!-- Section header -->
  <div class="tree-header no-select">
    <span class="header-label">CONNECTIONS</span>
    <span class="header-count">{connectionStore.profiles.length}</span>
  </div>

  <!-- Scrollable list -->
  <div class="tree-scroll gscroll">
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
          {#if renamingGroupId === group.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="rename-input"
              type="text"
              bind:value={renameValue}
              onblur={commitRename}
              onkeydown={(e) => { if (e.key === 'Enter') commitRename(); if (e.key === 'Escape') renamingGroupId = null; }}
              aria-label="Rename group"
              autofocus
            />
          {:else}
            <button class="group-row" onclick={() => toggleGroup(group.id)}>
              <span class="chevron" class:open={isExpanded} aria-hidden="true">
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"></polyline></svg>
              </span>
              <span class="group-name">{group.name}</span>
              <span class="group-count">{groupProfiles.length}</span>
            </button>
          {/if}
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
    <div class="conn-row" class:connected class:errored oncontextmenu={(e) => showConnCtx(e, profile)}>
      <!-- Chevron: rotates when expanded -->
      <button
        class="conn-chevron"
        class:open={expanded}
        onclick={() => connected ? toggleExpand(profile.id) : handleConnect(profile)}
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
        onclick={() => connected ? panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id }) : handleConnect(profile)}
        title={profile.host}
      >{profile.name}</button>

      <!-- Lock icon if read-only -->
      {#if profile.readOnly}
        <svg class="lock-icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-label="Read-only">
          <rect x="5" y="11" width="14" height="9" rx="2"></rect>
          <path d="M8 11V8a4 4 0 0 1 8 0v3"></path>
        </svg>
      {/if}

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
        <button
          class="action-btn action-btn--danger"
          onclick={(e) => { e.stopPropagation(); deleteConnection(profile); }}
          title="Delete connection"
          aria-label="Delete {profile.name}"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"></path>
            <path d="M10 11v6"></path><path d="M14 11v6"></path>
            <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"></path>
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
          {#each [...databases.keys()] as database}
            {@const dbKey = `${profile.id}/${database}`}
            {@const isDbExpanded = expandedDatabases.has(dbKey)}
            {@const isDbLoading = loadingKeys.has(dbKey)}
            {@const tables = databases.get(database) ?? []}
            {@const dbLoadError = loadErrors.get(dbKey)}

            <div class="db-item" class:system-item={isSystemDatabase(database)}>
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
                <svg class="db-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
                  <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
                  <path d="M3 5v6c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
                  <path d="M3 11v6c0 1.66 4 3 9 3s9-1.34 9-3v-6"></path>
                </svg>
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
                  {#each tables as table}
                    <button
                      class="table-row"
                      class:system-item={isSystemTable(table.name)}
                      class:active={isTableActive(profile.id, database, table.name)}
                      onclick={() => openTable(profile.id, database, table.name)}
                      oncontextmenu={(e) => showTableCtx(e, profile.id, database, table)}
                      title={table.name}
                      aria-label="Open {table.name}"
                    >
                      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" class="table-icon">
                        <rect x="3" y="4" width="18" height="16" rx="2"></rect>
                        <line x1="3" y1="9.5" x2="21" y2="9.5"></line>
                        <line x1="9" y1="9.5" x2="9" y2="20"></line>
                      </svg>
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
{#if tableCtx}
  <div class="ctx-menu" role="menu" style="top:{tableCtx.y}px;left:{tableCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={ctxOpenTable}>Open Table</button>
    <button class="ctx-item" role="menuitem" onclick={ctxViewDdl}>View DDL</button>
    <button class="ctx-item" role="menuitem" onclick={ctxCopyName}>Copy Name</button>
  </div>
{/if}

{#if dbCtx}
  <div class="ctx-menu" role="menu" style="top:{dbCtx.y}px;left:{dbCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={ctxOpenErd}>Open ERD</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item" role="menuitem" onclick={() => { if (dbCtx) { navigator.clipboard.writeText(dbCtx.database); dbCtx = null; } }}>Copy Name</button>
  </div>
{/if}

{#if grpCtx}
  <div class="ctx-menu" role="menu" style="top:{grpCtx.y}px;left:{grpCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={() => { if (grpCtx) { newConnectionGroupId = grpCtx.group.id; showAddForm = true; grpCtx = null; } }}>New Connection in Group</button>
    <button class="ctx-item" role="menuitem" onclick={() => { if (grpCtx) { renamingGroupId = grpCtx.group.id; renameValue = grpCtx.group.name; grpCtx = null; } }}>Rename Group</button>
    <button class="ctx-item ctx-item--danger" role="menuitem" onclick={() => grpCtx && deleteGroup(grpCtx.group)}>Delete Group</button>
  </div>
{/if}

{#if connCtx}
  {@const connConnected = isConnected(connCtx.profile.id)}
  <div class="ctx-menu" role="menu" style="top:{connCtx.y}px;left:{connCtx.x}px" use:portal>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { panelStore.openInFocused({ kind: 'query_editor', connectionId: connCtx.profile.id }); connCtx = null; } }}>New Query Editor</button>
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { editingProfile = connCtx.profile; connCtx = null; } }}>Edit</button>
    <button class="ctx-item" role="menuitem" onclick={ctxConnToggleReadOnly}>{connCtx.profile.readOnly ? 'Disable Read Only' : 'Enable Read Only'}</button>
    <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { navigator.clipboard.writeText(connCtx.profile.name); connCtx = null; } }}>Copy Name</button>
    <div class="ctx-sep" role="separator"></div>
    {#if connConnected}
      <button class="ctx-item" role="menuitem" onclick={ctxConnDisconnect}>Disconnect</button>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={() => { if (connCtx) { handleConnect(connCtx.profile); connCtx = null; } }}>Connect</button>
    {/if}
    <div class="ctx-sep" role="separator"></div>
    <button class="ctx-item ctx-item--danger" role="menuitem" onclick={() => { if (connCtx) { deleteConnection(connCtx.profile); connCtx = null; } }}>Delete</button>
  </div>
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
  />
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
    cursor: default;
    transition: background var(--transition-fast);
    position: relative;
  }

  .conn-row:hover {
    background: var(--color-bg-hover);
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

  .conn-chevron:hover { color: var(--color-text-primary); background: var(--color-bg-active); }
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
    flex: 1;
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

  .table-icon { flex-shrink: 0; }
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

  .ctx-sep {
    height: 1px;
    margin: var(--spacing-1) 0;
    background: var(--color-border);
  }
</style>
