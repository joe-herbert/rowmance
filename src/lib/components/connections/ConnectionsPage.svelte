<!--
  ConnectionsPage — a full-panel view for managing all connection profiles.
  Shows cards organized by group with status indicators and quick actions.
  Right-clicking (or the ⋯ button) opens a context menu matching the sidebar tree.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as schemaApi from '$lib/tauri/schema';
  import { errorMessage } from '$lib/utils/errors';
  import ConnectionForm from './ConnectionForm.svelte';
  import ExportConnectionsDialog from './ExportConnectionsDialog.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import CtxSubmenuItem from '$lib/components/ui/CtxSubmenuItem.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import type { ConnectionProfile } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();
  const toast = useToast();
  const settingsStore = useSettings();

  let filterQuery = $state('');

  let showAddForm = $state(false);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);
  let newConnectionGroupId = $state<string | null>(null);

  let showExportDialog = $state(false);
  let exportPreselectIds = $state<string[] | null>(null);

  // ── Create group modal ────────────────────────────────────────────────────

  let showCreateGroupModal = $state(false);
  let newGroupName = $state('');
  let newGroupError = $state('');
  let newGroupLoading = $state(false);

  async function commitCreateGroup() {
    const name = newGroupName.trim();
    if (!name) return;
    newGroupLoading = true;
    newGroupError = '';
    try {
      await connectionsApi.createConnectionGroup(name);
      await connectionStore.load();
      showCreateGroupModal = false;
    } catch (e) {
      newGroupError = errorMessage(e);
    } finally {
      newGroupLoading = false;
    }
  }

  interface ConfirmState {
    title: string;
    message: string;
    confirmText?: string;
    onconfirm: () => void;
  }
  let confirmState = $state<ConfirmState | null>(null);

  // ── Per-card context menu ─────────────────────────────────────────────────

  interface CardCtx {
    profile: ConnectionProfile;
    x: number;
    y: number;
  }
  let cardCtx = $state<CardCtx | null>(null);

  function openCardCtx(e: MouseEvent, profile: ConnectionProfile) {
    e.preventDefault();
    e.stopPropagation();
    cardCtx = { profile, x: e.clientX, y: e.clientY };
  }

  // ── Create database modal ─────────────────────────────────────────────────

  interface CreateDbModal {
    connectionId: string;
    dbType: string;
  }
  let createDbModal = $state<CreateDbModal | null>(null);
  let createDbName = $state('');
  let createDbError = $state('');
  let createDbLoading = $state(false);

  function qi(name: string, dbType: string): string {
    if (dbType === 'mysql' || dbType === 'mariadb') return '`' + name.replace(/`/g, '``') + '`';
    return '"' + name.replace(/"/g, '""') + '"';
  }

  async function executeCreateDatabase() {
    if (!createDbModal) return;
    const name = createDbName.trim();
    if (!name) {
      createDbError = 'Name is required';
      return;
    }
    const { connectionId, dbType } = createDbModal;
    const sql =
      dbType === 'postgres'
        ? `CREATE SCHEMA ${qi(name, dbType)}`
        : `CREATE DATABASE ${qi(name, dbType)}`;
    createDbLoading = true;
    createDbError = '';
    try {
      await schemaApi.executeDdl(connectionId, sql);
      createDbModal = null;
      toast.addToast(`${dbType === 'postgres' ? 'Schema' : 'Database'} created`, 'success', 2000);
    } catch (err) {
      createDbError = errorMessage(err);
    } finally {
      createDbLoading = false;
    }
  }

  // ── Derived state ─────────────────────────────────────────────────────────

  const filteredProfiles = $derived(
    filterQuery
      ? connectionStore.profiles.filter(
          (p) =>
            p.name.toLowerCase().includes(filterQuery.toLowerCase()) ||
            p.host.toLowerCase().includes(filterQuery.toLowerCase()) ||
            p.database.toLowerCase().includes(filterQuery.toLowerCase()) ||
            p.dbType.toLowerCase().includes(filterQuery.toLowerCase()),
        )
      : connectionStore.profiles,
  );

  const grouped = $derived.by(() => {
    const groups = connectionStore.groups;
    const ungrouped = filteredProfiles.filter((p) => p.groupId === null);
    const byGroup = new Map<string, ConnectionProfile[]>();
    for (const g of groups) byGroup.set(g.id, []);
    for (const p of filteredProfiles) {
      if (p.groupId !== null && byGroup.has(p.groupId)) byGroup.get(p.groupId)!.push(p);
    }
    return { groups, ungrouped, byGroup };
  });

  const visibleGroups = $derived(
    grouped.groups.filter((g) => (grouped.byGroup.get(g.id) ?? []).length > 0),
  );

  const totalVisible = $derived(filteredProfiles.length);

  // ── Connection state helpers ──────────────────────────────────────────────

  function isConnected(id: string) {
    return connectionStore.activeIds.has(id);
  }

  function isConnecting(id: string) {
    return connectionStore.connectingIds.has(id);
  }

  function hasError(id: string) {
    return connectionStore.errorIds.has(id);
  }

  // ── Action handlers ───────────────────────────────────────────────────────

  async function handleConnect(profile: ConnectionProfile) {
    try {
      await connectionStore.connect(profile.id);
    } catch (err) {
      toast.addToast(`${profile.name}: ${errorMessage(err)}`, 'error', 0);
    }
  }

  async function handleDisconnect(profile: ConnectionProfile) {
    try {
      await connectionStore.disconnect(profile.id);
      panelStore.closeItemsForConnection(profile.id, { skipDirty: true });
    } catch (err) {
      toast.addToast(`${profile.name}: ${errorMessage(err)}`, 'error');
    }
  }

  function handleEdit(profile: ConnectionProfile) {
    editingProfile = profile;
    showAddForm = true;
  }

  function handleDelete(profile: ConnectionProfile) {
    confirmState = {
      title: 'Delete Connection',
      message: `Delete "${profile.name}"? This cannot be undone.`,
      confirmText: 'Delete',
      onconfirm: async () => {
        confirmState = null;
        if (connectionStore.isActive(profile.id)) await connectionStore.disconnect(profile.id);
        try {
          await connectionsApi.deleteConnection(profile.id);
          panelStore.closeItemsForConnection(profile.id);
          await connectionStore.load();
        } catch {
          /* ignore */
        }
      },
    };
  }

  async function handleDuplicate(profile: ConnectionProfile) {
    try {
      await connectionsApi.duplicateConnection(profile.id);
      await connectionStore.load();
      toast.addToast(`Duplicated "${profile.name}"`, 'success', 2000);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    }
  }

  async function handleNewQueryEditor(profile: ConnectionProfile) {
    if (!isConnected(profile.id)) await connectionStore.connect(profile.id);
    panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id });
  }

  async function handleManageUsers(profile: ConnectionProfile) {
    if (!isConnected(profile.id)) {
      try {
        await connectionStore.connect(profile.id);
      } catch (err) {
        toast.addToast(`${profile.name}: ${errorMessage(err)}`, 'error', 0);
        return;
      }
    }
    panelStore.openInFocused({ kind: 'user_manager', connectionId: profile.id });
  }

  function handleNewDatabase(profile: ConnectionProfile) {
    createDbName = '';
    createDbError = '';
    createDbModal = { connectionId: profile.id, dbType: profile.dbType };
  }

  async function handleToggleReadOnly(profile: ConnectionProfile) {
    try {
      await connectionStore.toggleReadOnly(profile.id);
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    }
  }

  async function handleCopyName(profile: ConnectionProfile) {
    try {
      await navigator.clipboard.writeText(profile.name);
      toast.addToast('Name copied', 'success', 1500);
    } catch (err) {
      toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error', 0);
    }
  }

  async function handleCopyDbUrl(profile: ConnectionProfile) {
    try {
      await connectionsApi.copyConnectionDbUrlToClipboard(profile.id);
      toast.addToast('Database URL copied', 'success', 1500);
    } catch (err) {
      toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error', 0);
    }
  }

  function handleCloseAllTabs(profile: ConnectionProfile) {
    panelStore.closeItemsForConnection(profile.id);
  }

  async function handleMoveToGroup(profile: ConnectionProfile, groupId: string | null) {
    try {
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
    } catch (err) {
      toast.addToast(errorMessage(err), 'error');
    }
  }

  async function handleImport() {
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
        2500,
      );
    }
  }

  // ── Display helpers ───────────────────────────────────────────────────────

  function statusLabel(profile: ConnectionProfile): string {
    if (isConnecting(profile.id)) return 'Connecting…';
    if (hasError(profile.id)) return 'Error';
    if (isConnected(profile.id)) return 'Connected';
    return 'Disconnected';
  }

  function hostDisplay(profile: ConnectionProfile): string {
    if (profile.dbType === 'sqlite') return profile.host;
    const db = profile.database ? `/${profile.database}` : '';
    return `${profile.host}:${profile.port}${db}`;
  }
</script>

<div class="connections-page">
  <!-- Page header -->
  <div class="page-header">
    <div class="page-title-row">
      <h1 class="page-title">Connections</h1>
      <span class="page-count">{connectionStore.profiles.length}</span>
    </div>

    <div class="header-actions">
      <div class="search-wrap">
        <svg
          class="search-icon"
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          aria-hidden="true"
        >
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
        <input
          class="search-input"
          type="text"
          placeholder="Filter connections…"
          bind:value={filterQuery}
          autocomplete="off"
          spellcheck={false}
          onkeydown={(e) => {
            if (e.key === 'Escape') filterQuery = '';
          }}
        />
        {#if filterQuery}
          <button class="search-clear" onclick={() => (filterQuery = '')} aria-label="Clear filter">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" aria-hidden="true">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        {/if}
      </div>

      <button
        class="action-btn"
        onclick={() => { newGroupName = ''; newGroupError = ''; showCreateGroupModal = true; }}
        title="New folder"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
          <line x1="12" y1="11" x2="12" y2="17"></line>
          <line x1="9" y1="14" x2="15" y2="14"></line>
        </svg>
        New Folder
      </button>
      <button class="action-btn" onclick={handleImport} title="Import connections from JSON">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="17 8 12 3 7 8"></polyline>
          <line x1="12" y1="3" x2="12" y2="15"></line>
        </svg>
        Import
      </button>
      <button
        class="action-btn"
        onclick={() => { exportPreselectIds = null; showExportDialog = true; }}
        title="Export connections to JSON"
        disabled={connectionStore.profiles.length === 0}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
          <polyline points="7 10 12 15 17 10"></polyline>
          <line x1="12" y1="15" x2="12" y2="3"></line>
        </svg>
        Export
      </button>
      <button
        class="action-btn action-btn--primary"
        onclick={() => { editingProfile = undefined; newConnectionGroupId = null; showAddForm = true; }}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
          <line x1="12" y1="5" x2="12" y2="19"></line>
          <line x1="5" y1="12" x2="19" y2="12"></line>
        </svg>
        Add Connection
      </button>
    </div>
  </div>

  <!-- Connections grid -->
  <div class="page-body">
    {#if connectionStore.profiles.length === 0}
      <div class="empty-state">
        <div class="empty-icon" aria-hidden="true">
          <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round">
            <ellipse cx="12" cy="5" rx="9" ry="3"></ellipse>
            <path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path>
            <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path>
          </svg>
        </div>
        <p class="empty-title">No connections yet</p>
        <p class="empty-subtitle">Add your first connection to get started</p>
        <button
          class="action-btn action-btn--primary"
          onclick={() => { editingProfile = undefined; newConnectionGroupId = null; showAddForm = true; }}
        >Add Connection</button>
      </div>
    {:else if filterQuery && totalVisible === 0}
      <div class="empty-state">
        <p class="empty-title">No matches for "{filterQuery}"</p>
        <button class="action-btn" onclick={() => (filterQuery = '')}>Clear filter</button>
      </div>
    {:else}
      <!-- Ungrouped connections -->
      {#if grouped.ungrouped.length > 0}
        {#if grouped.groups.length > 0}
          <div class="group-label">Ungrouped</div>
        {/if}
        <div class="cards-grid">
          {#each grouped.ungrouped as profile (profile.id)}
            {@render Card(profile)}
          {/each}
        </div>
      {/if}

      <!-- Groups -->
      {#each visibleGroups as group (group.id)}
        {@const groupProfiles = grouped.byGroup.get(group.id) ?? []}
        {#if groupProfiles.length > 0}
          <div class="group-label">{group.name}</div>
          <div class="cards-grid">
            {#each groupProfiles as profile (profile.id)}
              {@render Card(profile)}
            {/each}
          </div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

<!-- ── Per-card context menu ─────────────────────────────────────────────── -->

{#if cardCtx}
  {@const p = cardCtx.profile}
  {@const connected = isConnected(p.id)}
  {@const hasGroups = connectionStore.groups.length > 0}
  <ContextMenu x={cardCtx.x} y={cardCtx.y} open={true} onclose={() => (cardCtx = null)}>
    {#if connected && !p.readOnly && p.dbType !== 'sqlite'}
      <CtxItem onclick={() => { const prof = p; cardCtx = null; handleNewDatabase(prof); }}>
        New {p.dbType === 'postgres' ? 'Schema' : 'Database'}
      </CtxItem>
    {/if}
    <CtxItem onclick={() => { const prof = p; cardCtx = null; handleManageUsers(prof); }}>
      Manage Users
    </CtxItem>
    <CtxSep />
    <CtxItem onclick={() => { const prof = p; cardCtx = null; handleDuplicate(prof); }}>
      Duplicate
    </CtxItem>
    <CtxItem onclick={() => { const prof = p; cardCtx = null; handleCopyName(prof); }}>
      Copy Name
    </CtxItem>
    <CtxSep />
    {#if connected}
      <CtxItem onclick={() => { const prof = p; cardCtx = null; handleDisconnect(prof).then(() => handleConnect(prof)); }}>
        Refresh
      </CtxItem>
      <CtxItem onclick={() => { const prof = p; cardCtx = null; handleDisconnect(prof); }}>
        Disconnect
      </CtxItem>
    {/if}
    <CtxItem onclick={() => { const prof = p; cardCtx = null; handleCloseAllTabs(prof); }}>
      Close All Tabs
    </CtxItem>
    {#if hasGroups}
      <CtxSep />
      {@const otherGroups = connectionStore.groups.filter((g) => g.id !== p.groupId)}
      {#if otherGroups.length > 0}
        <CtxSubmenuItem label="Move to">
          {#each otherGroups as g (g.id)}
            <CtxItem onclick={() => { const prof = p; cardCtx = null; handleMoveToGroup(prof, g.id); }}>
              {g.name}
            </CtxItem>
          {/each}
        </CtxSubmenuItem>
      {/if}
      {#if p.groupId !== null}
        <CtxItem onclick={() => { const prof = p; cardCtx = null; handleMoveToGroup(prof, null); }}>
          Remove from Group
        </CtxItem>
      {/if}
    {/if}
  </ContextMenu>
{/if}

<!-- Add/Edit connection form -->
{#if showAddForm}
  <ConnectionForm
    profile={editingProfile}
    groupId={newConnectionGroupId}
    onclose={async () => {
      showAddForm = false;
      editingProfile = undefined;
      await connectionStore.load();
    }}
    ondelete={editingProfile
      ? () => {
          const p = editingProfile!;
          showAddForm = false;
          editingProfile = undefined;
          handleDelete(p);
        }
      : undefined}
  />
{/if}

<!-- Export dialog -->
{#if showExportDialog}
  <ExportConnectionsDialog
    profiles={connectionStore.profiles}
    preselectIds={exportPreselectIds}
    onclose={() => (showExportDialog = false)}
    onsuccess={(count) => {
      showExportDialog = false;
      toast.addToast(`Exported ${count} connection${count !== 1 ? 's' : ''}`, 'success', 2500);
    }}
    onerror={(msg) => toast.addToast(msg, 'error', 0)}
  />
{/if}

<!-- New folder modal -->
{#if showCreateGroupModal}
  <Modal label="New Folder" onbackdropclick={newGroupLoading ? undefined : () => (showCreateGroupModal = false)}>
    <div class="create-modal-card">
      <div class="create-modal-title">New Folder</div>
      <div class="create-modal-body">
        <label class="field-label" for="new-group-name">Folder Name</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input
          id="new-group-name"
          class="field-input"
          type="text"
          bind:value={newGroupName}
          placeholder="Folder name"
          autofocus
          disabled={newGroupLoading}
          onkeydown={(e) => {
            if (e.key === 'Enter') commitCreateGroup();
            if (e.key === 'Escape') showCreateGroupModal = false;
          }}
        />
        {#if newGroupError}
          <p class="field-error">{newGroupError}</p>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn-secondary" onclick={() => (showCreateGroupModal = false)} disabled={newGroupLoading}>
          Cancel
        </button>
        <button class="btn-primary" onclick={commitCreateGroup} disabled={newGroupLoading || !newGroupName.trim()}>
          {newGroupLoading ? 'Creating…' : 'Create Folder'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- New database/schema modal -->
{#if createDbModal}
  {@const dbLabel = createDbModal.dbType === 'postgres' ? 'Schema' : 'Database'}
  <Modal label="New {dbLabel}" onbackdropclick={createDbLoading ? undefined : () => (createDbModal = null)}>
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
          placeholder="{dbLabel} name"
          autofocus
          disabled={createDbLoading}
          onkeydown={(e) => {
            if (e.key === 'Enter') executeCreateDatabase();
            if (e.key === 'Escape') createDbModal = null;
          }}
        />
        {#if createDbError}
          <p class="field-error">{createDbError}</p>
        {/if}
      </div>
      <div class="create-modal-footer">
        <button class="btn-secondary" onclick={() => (createDbModal = null)} disabled={createDbLoading}>
          Cancel
        </button>
        <button class="btn-primary" onclick={executeCreateDatabase} disabled={createDbLoading}>
          {createDbLoading ? 'Creating…' : `Create ${dbLabel}`}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- Confirm dialog -->
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

<!-- ── Card snippet ────────────────────────────────────────────────────────── -->

{#snippet Card(profile: ConnectionProfile)}
  {@const connected = isConnected(profile.id)}
  {@const connecting = isConnecting(profile.id)}
  {@const error = hasError(profile.id)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="card"
    class:card--connected={connected}
    class:card--error={error}
    style="--conn-color: {profile.color ?? 'var(--color-accent)'}"
    oncontextmenu={(e) => openCardCtx(e, profile)}
  >
    <!-- Identity -->
    <div class="card-header">
      <div class="card-identity">
        <span
          class="card-dot"
          class:card-dot--connected={connected}
          class:card-dot--connecting={connecting}
          class:card-dot--error={error}
          style={connected || connecting || error ? '' : `background: ${profile.color ?? 'var(--color-accent)'}`}
          aria-hidden="true"
        ></span>
        <span class="card-name" title={profile.name}>{profile.name}</span>
        <span class="card-type">{profile.dbType}</span>
      </div>
      <div class="card-badges">
        {#if profile.readOnly}
          <span class="badge badge--readonly" title="Read-only">RO</span>
        {/if}
        {#if profile.sshEnabled}
          <span class="badge badge--ssh" title="SSH tunnel">SSH</span>
        {/if}
        {#if profile.sslEnabled}
          <span class="badge badge--ssl" title="SSL/TLS">SSL</span>
        {/if}
      </div>
    </div>

    <!-- Connection info -->
    <div class="card-detail">
      <span class="card-host" title={hostDisplay(profile)}>{hostDisplay(profile)}</span>
      {#if profile.dbType !== 'sqlite' && profile.username}
        <span class="card-user">{profile.username}</span>
      {/if}
    </div>

    <!-- Status -->
    <div class="card-status">
      <span
        class="status-dot"
        class:status-dot--connected={connected}
        class:status-dot--connecting={connecting}
        class:status-dot--error={error}
        aria-hidden="true"
      ></span>
      <span class="status-text">{statusLabel(profile)}</span>
    </div>

    <!-- Actions -->
    <div class="card-actions">
      {#if connected}
        <button class="card-btn card-btn--disconnect" onclick={() => handleDisconnect(profile)}>
          Disconnect
        </button>
      {:else}
        <button class="card-btn card-btn--connect" onclick={() => handleConnect(profile)} disabled={connecting}>
          {connecting ? 'Connecting…' : 'Connect'}
        </button>
      {/if}
      <div class="card-secondary-actions">
        <button
          class="card-icon-btn"
          onclick={() => handleNewQueryEditor(profile)}
          title="New query editor"
          aria-label="New query editor"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <polyline points="4 17 10 11 4 5"></polyline>
            <line x1="12" y1="19" x2="20" y2="19"></line>
          </svg>
        </button>
        <button
          class="card-icon-btn"
          onclick={() => handleEdit(profile)}
          title="Edit"
          aria-label="Edit connection"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
            <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
          </svg>
        </button>
        <button
          class="card-icon-btn"
          onclick={() => handleToggleReadOnly(profile)}
          title={profile.readOnly ? 'Disable read only' : 'Enable read only'}
          aria-label={profile.readOnly ? 'Disable read only' : 'Enable read only'}
        >
          {#if profile.readOnly}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
              <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
            </svg>
          {:else}
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
              <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
            </svg>
          {/if}
        </button>
        <button
          class="card-icon-btn"
          onclick={() => handleCopyDbUrl(profile)}
          title="Copy as database URL"
          aria-label="Copy as database URL"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
          </svg>
        </button>
        <button
          class="card-icon-btn"
          onclick={() => { exportPreselectIds = [profile.id]; showExportDialog = true; }}
          title="Export"
          aria-label="Export connection"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
        </button>
        <button
          class="card-icon-btn card-icon-btn--danger"
          onclick={() => handleDelete(profile)}
          title="Delete"
          aria-label="Delete connection"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <polyline points="3 6 5 6 21 6"></polyline>
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"></path>
            <path d="M10 11v6"></path>
            <path d="M14 11v6"></path>
            <path d="M9 6V4a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2"></path>
          </svg>
        </button>
        <button
          class="card-icon-btn card-more-btn"
          onclick={(e) => openCardCtx(e, profile)}
          title="More actions"
          aria-label="More actions"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <circle cx="5" cy="12" r="2"></circle>
            <circle cx="12" cy="12" r="2"></circle>
            <circle cx="19" cy="12" r="2"></circle>
          </svg>
        </button>
      </div>
    </div>
  </div>
{/snippet}

<style>
  .connections-page {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  /* ── Header ──────────────────────────────────────────────────────────────── */

  .page-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .page-title-row {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .page-title {
    font-size: 16px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin: 0;
  }

  .page-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-weight: normal;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex: 1;
    flex-wrap: wrap;
  }

  /* ── Search ──────────────────────────────────────────────────────────────── */

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 180px;
    max-width: 280px;
    flex: 1;
  }

  .search-icon {
    position: absolute;
    left: 9px;
    color: var(--color-text-muted);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    height: 30px;
    padding: 0 28px 0 30px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .search-input::placeholder {
    color: var(--color-text-disabled);
  }

  .search-input:focus {
    border-color: var(--color-accent);
  }

  .search-clear {
    position: absolute;
    right: 7px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .search-clear:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Action buttons ──────────────────────────────────────────────────────── */

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    flex-shrink: 0;
  }

  .action-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent, #fff);
    border-color: transparent;
    font-weight: var(--font-weight-medium);
  }

  .action-btn--primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-accent) 85%, black);
    color: var(--color-text-on-accent, #fff);
  }

  /* ── Body / grid ─────────────────────────────────────────────────────────── */

  .page-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .group-label {
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--color-text-muted);
    padding: 0 2px;
    margin-top: var(--spacing-2);
  }

  .group-label:first-child {
    margin-top: 0;
  }

  .cards-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--spacing-3);
  }

  /* ── Empty state ─────────────────────────────────────────────────────────── */

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: var(--spacing-3);
    padding: 48px var(--spacing-5);
    color: var(--color-text-muted);
    flex: 1;
  }

  .empty-icon {
    opacity: 0.35;
    margin-bottom: var(--spacing-2);
  }

  .empty-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    margin: 0;
  }

  .empty-subtitle {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
  }

  /* ── Cards ───────────────────────────────────────────────────────────────── */

  .card {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding: var(--spacing-3);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-left: 3px solid var(--conn-color, var(--color-accent));
    border-radius: var(--radius-lg);
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
  }

  .card:hover {
    border-color: color-mix(in srgb, var(--conn-color, var(--color-accent)) 50%, var(--color-border));
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
  }

  .card--connected {
    background: color-mix(in srgb, var(--conn-color, var(--color-accent)) 5%, var(--color-bg-secondary));
  }

  .card--error {
    border-left-color: var(--color-danger, #dc2626);
  }

  /* ── Card header ─────────────────────────────────────────────────────────── */

  .card-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .card-identity {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
  }

  .card-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--conn-color, var(--color-accent));
    transition: box-shadow var(--transition-fast);
  }

  .card-dot--connected {
    background: var(--color-success, #16a34a);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--color-success, #16a34a) 25%, transparent);
  }

  .card-dot--connecting {
    background: var(--color-warning, #f59e0b);
    animation: dot-pulse 1s ease-in-out infinite;
  }

  .card-dot--error {
    background: var(--color-danger, #dc2626);
  }

  @keyframes dot-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .card-name {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .card-type {
    font-size: 9.5px;
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .card-badges {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .badge {
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.03em;
  }

  .badge--readonly {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
    border: 1px solid color-mix(in srgb, var(--color-accent) 30%, transparent);
  }

  .badge--ssh {
    background: color-mix(in srgb, var(--color-success, #16a34a) 12%, transparent);
    color: var(--color-success, #16a34a);
    border: 1px solid color-mix(in srgb, var(--color-success, #16a34a) 30%, transparent);
  }

  .badge--ssl {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 12%, transparent);
    color: var(--color-warning, #f59e0b);
    border: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 30%, transparent);
  }


  /* ── Card detail ─────────────────────────────────────────────────────────── */

  .card-detail {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .card-host {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-user {
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-family: var(--font-family-mono);
  }

  /* ── Card status ─────────────────────────────────────────────────────────── */

  .card-status {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-top: var(--spacing-1);
    border-top: 1px solid var(--color-border);
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-text-disabled);
  }

  .status-dot--connected {
    background: var(--color-success, #16a34a);
  }

  .status-dot--connecting {
    background: var(--color-warning, #f59e0b);
    animation: dot-pulse 1s ease-in-out infinite;
  }

  .status-dot--error {
    background: var(--color-danger, #dc2626);
  }

  .status-text {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  /* ── Card actions ────────────────────────────────────────────────────────── */

  .card-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-top: var(--spacing-1);
  }

  .card-btn {
    height: 26px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-primary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .card-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .card-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .card-btn--connect {
    border-color: var(--conn-color, var(--color-accent));
    color: var(--conn-color, var(--color-accent));
  }

  .card-btn--connect:hover:not(:disabled) {
    background: color-mix(in srgb, var(--conn-color, var(--color-accent)) 10%, transparent);
  }

  .card-btn--open {
    border-color: var(--color-success, #16a34a);
    color: var(--color-success, #16a34a);
  }

  .card-btn--open:hover:not(:disabled) {
    background: color-mix(in srgb, var(--color-success, #16a34a) 10%, transparent);
  }

  .card-btn--disconnect {
    border-color: var(--color-danger, #dc2626);
    color: var(--color-danger, #dc2626);
  }

  .card-btn--disconnect:hover {
    background: color-mix(in srgb, var(--color-danger, #dc2626) 10%, transparent);
  }

  .card-secondary-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
  }

  .card-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-md);
    border: none;
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .card-icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .card-icon-btn--danger:hover {
    background: color-mix(in srgb, var(--color-danger, #dc2626) 10%, transparent);
    color: var(--color-danger, #dc2626);
  }

  .card-more-btn {
    color: var(--color-text-muted);
  }

  .card-more-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── Create database modal ───────────────────────────────────────────────── */

  .create-modal-card {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
    padding: var(--spacing-5);
    min-width: 320px;
  }

  .create-modal-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .create-modal-body {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .field-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .field-input {
    height: 32px;
    padding: 0 var(--spacing-3);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    outline: none;
    transition: border-color var(--transition-fast);
  }

  .field-input:focus {
    border-color: var(--color-accent);
  }

  .field-error {
    font-size: var(--font-size-xs);
    color: var(--color-danger, #dc2626);
    margin: 0;
  }

  .create-modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
  }

  .btn-secondary {
    height: 32px;
    padding: 0 var(--spacing-4);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-bg-hover);
  }

  .btn-primary {
    height: 32px;
    padding: 0 var(--spacing-4);
    border: none;
    border-radius: var(--radius-md);
    background: var(--color-accent);
    color: var(--color-text-on-accent, #fff);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: opacity var(--transition-fast);
  }

  .btn-primary:hover:not(:disabled) {
    opacity: 0.88;
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
