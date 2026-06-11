<!--
  ConnectionTree — lists all connection profiles grouped by folder.
  Each connection shows a status dot and a context menu.
  Clicking a disconnected connection prompts to connect; clicking a connected one expands the schema.
  Groups are collapsible and have a right-click context menu (New Connection, Rename, Delete).
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import ConnectionForm from './ConnectionForm.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import { ask } from '@tauri-apps/plugin-dialog';
  import type { ConnectionProfile, ConnectionGroup } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();

  let showAddForm = $state(false);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);
  let newConnectionGroupId = $state<string | null | undefined>(undefined); // undefined = not set

  // Group UI state
  let expandedGroups = $state<Set<string>>(new Set());
  let ungroupedExpanded = $state(true);

  // Group context menu
  interface GroupContextMenu {
    x: number;
    y: number;
    group: ConnectionGroup;
  }
  let groupContextMenu = $state<GroupContextMenu | null>(null);

  // Rename state
  let renamingGroupId = $state<string | null>(null);
  let renameValue = $state('');

  // ── Helpers ────────────────────────────────────────────────────────────────────

  async function handleConnect(profile: ConnectionProfile) {
    if (connectionStore.isActive(profile.id)) return;
    await connectionStore.connect(profile.id);
  }

  function openQueryEditor(profile: ConnectionProfile) {
    panelStore.openInFocused({ kind: 'query_editor', connectionId: profile.id });
  }

  function statusClass(profileId: string): string {
    if (connectionStore.connectingIds.has(profileId)) return 'connecting';
    if (connectionStore.errorIds.has(profileId)) return 'error';
    if (connectionStore.activeIds.has(profileId)) return 'connected';
    return 'idle';
  }

  function statusLabel(profileId: string): string {
    const status = statusClass(profileId);
    const labels: Record<string, string> = {
      connected: 'Connected',
      connecting: 'Connecting…',
      error: connectionStore.errorIds.get(profileId) ?? 'Connection error',
      idle: 'Not connected',
    };
    return labels[status];
  }

  // ── Group helpers ──────────────────────────────────────────────────────────────

  function toggleGroup(groupId: string) {
    if (expandedGroups.has(groupId)) {
      expandedGroups = new Set([...expandedGroups].filter((id) => id !== groupId));
    } else {
      expandedGroups = new Set([...expandedGroups, groupId]);
    }
  }

  function showGroupContextMenu(event: MouseEvent, group: ConnectionGroup) {
    event.preventDefault();
    groupContextMenu = { x: event.clientX, y: event.clientY, group };
  }

  function closeGroupContextMenu() {
    groupContextMenu = null;
  }

  function ctxNewConnectionInGroup() {
    if (!groupContextMenu) return;
    newConnectionGroupId = groupContextMenu.group.id;
    showAddForm = true;
    closeGroupContextMenu();
  }

  function ctxRenameGroup() {
    if (!groupContextMenu) return;
    renamingGroupId = groupContextMenu.group.id;
    renameValue = groupContextMenu.group.name;
    closeGroupContextMenu();
  }

  async function commitRename() {
    if (!renamingGroupId || !renameValue.trim()) {
      renamingGroupId = null;
      return;
    }
    try {
      await connectionsApi.updateConnectionGroup(renamingGroupId, { name: renameValue.trim() });
      await connectionStore.load();
    } catch {
      // Silently ignore — the name reverts on next load.
    } finally {
      renamingGroupId = null;
    }
  }

  async function ctxDeleteGroup() {
    if (!groupContextMenu) return;
    const { name, id } = groupContextMenu.group;
    closeGroupContextMenu();
    if (!await ask(`Delete group "${name}" and all its connections? This cannot be undone.`, { title: 'Delete Group', kind: 'warning' })) return;
    try {
      await connectionsApi.deleteConnectionGroup(id);
      await connectionStore.load();
    } catch {
      // Ignore.
    }
  }

  async function deleteConnection(profile: ConnectionProfile) {
    if (!await ask(`Delete "${profile.name}"? This cannot be undone.`, { title: 'Delete Connection', kind: 'warning' })) return;
    if (connectionStore.isActive(profile.id)) {
      await connectionStore.disconnect(profile.id);
    }
    try {
      await connectionsApi.deleteConnection(profile.id);
      await connectionStore.load();
    } catch {
      // Ignore.
    }
  }

  function handleWindowKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      if (groupContextMenu) closeGroupContextMenu();
      if (renamingGroupId) renamingGroupId = null;
    }
  }

  function handleWindowClick(event: MouseEvent) {
    if (!groupContextMenu) return;
    const target = event.target as Element | null;
    if (!target?.closest('.group-context-menu')) {
      closeGroupContextMenu();
    }
  }

  // ── Derived groupings ──────────────────────────────────────────────────────────

  const grouped = $derived(() => {
    const groups = connectionStore.groups;
    const profiles = connectionStore.profiles;

    const ungrouped = profiles.filter((p) => p.groupId === null);
    const byGroup = new Map<string, ConnectionProfile[]>();
    for (const g of groups) byGroup.set(g.id, []);
    for (const p of profiles) {
      if (p.groupId !== null && byGroup.has(p.groupId)) {
        byGroup.get(p.groupId)!.push(p);
      }
    }
    return { groups, ungrouped, byGroup };
  });
</script>

<svelte:window onkeydown={handleWindowKeydown} onclick={handleWindowClick} />

<div class="connection-tree">
  <div class="tree-header no-select">
    <span class="header-label">Connections</span>
    <button
      class="add-btn"
      title="Add connection"
      aria-label="Add new connection"
      onclick={() => { newConnectionGroupId = undefined; showAddForm = true; }}>+</button
    >
  </div>

  {#if connectionStore.profiles.length === 0 && connectionStore.groups.length === 0}
    <div class="empty-state">
      <p>No connections yet.</p>
      <button class="link-btn" onclick={() => { newConnectionGroupId = undefined; showAddForm = true; }}>
        Add your first connection
      </button>
    </div>
  {:else}
    <ul class="tree-list" role="tree" aria-label="Database connections">

      <!-- Ungrouped profiles -->
      {#if grouped().ungrouped.length > 0 || connectionStore.groups.length > 0}
        {#if connectionStore.groups.length > 0}
          <!-- Only show "Ungrouped" header when there are groups -->
          <li class="group-item" role="treeitem" aria-expanded={ungroupedExpanded}>
            <button
              class="group-header"
              onclick={() => (ungroupedExpanded = !ungroupedExpanded)}
              aria-label="{ungroupedExpanded ? 'Collapse' : 'Expand'} Ungrouped"
            >
              <span class="group-chevron" class:open={ungroupedExpanded} aria-hidden="true">›</span>
              <span class="group-name">Ungrouped</span>
            </button>
            {#if ungroupedExpanded}
              <ul role="group" class="group-children">
                {#each grouped().ungrouped as profile (profile.id)}
                  {@const status = statusClass(profile.id)}
                  <li class="tree-item" role="treeitem" aria-selected={false}>
                    <div class="connection-row" class:active={status === 'connected'}>
                      {#if profile.color}
                        <span class="color-dot" style="background: {profile.color};" aria-hidden="true"></span>
                      {/if}
                      <span class="status-dot status-dot--{status}" title={statusLabel(profile.id)} aria-label={statusLabel(profile.id)}></span>
                      <button
                        class="connection-name"
                        onclick={() => status === 'connected' ? openQueryEditor(profile) : handleConnect(profile)}
                        title={profile.host}
                      >{profile.name}</button>
                      <span class="db-badge">{profile.dbType}</span>
                      <button
                        class="edit-btn"
                        title="Edit connection"
                        aria-label="Edit {profile.name}"
                        onclick={(e) => { e.stopPropagation(); editingProfile = profile; }}
                      >✎</button>
                      <button
                        class="delete-btn"
                        title="Delete connection"
                        aria-label="Delete {profile.name}"
                        onclick={(e) => { e.stopPropagation(); deleteConnection(profile); }}
                      >✕</button>
                    </div>
                    {#if connectionStore.errorIds.has(profile.id)}
                      <div class="error-row">{connectionStore.errorIds.get(profile.id)}</div>
                    {/if}
                  </li>
                {/each}
              </ul>
            {/if}
          </li>
        {:else}
          <!-- No groups: render profiles flat -->
          {#each grouped().ungrouped as profile (profile.id)}
            {@const status = statusClass(profile.id)}
            <li class="tree-item" role="treeitem" aria-selected={false}>
              <div class="connection-row" class:active={status === 'connected'}>
                {#if profile.color}
                  <span class="color-dot" style="background: {profile.color};" aria-hidden="true"></span>
                {/if}
                <span class="status-dot status-dot--{status}" title={statusLabel(profile.id)} aria-label={statusLabel(profile.id)}></span>
                <button
                  class="connection-name"
                  onclick={() => status === 'connected' ? openQueryEditor(profile) : handleConnect(profile)}
                  title={profile.host}
                >{profile.name}</button>
                <span class="db-badge">{profile.dbType}</span>
                <button
                  class="edit-btn"
                  title="Edit connection"
                  aria-label="Edit {profile.name}"
                  onclick={(e) => { e.stopPropagation(); editingProfile = profile; }}
                >✎</button>
                <button
                  class="delete-btn"
                  title="Delete connection"
                  aria-label="Delete {profile.name}"
                  onclick={(e) => { e.stopPropagation(); deleteConnection(profile); }}
                >✕</button>
              </div>
              {#if connectionStore.errorIds.has(profile.id)}
                <div class="error-row">{connectionStore.errorIds.get(profile.id)}</div>
              {/if}
            </li>
          {/each}
        {/if}
      {/if}

      <!-- Named groups -->
      {#each grouped().groups as group (group.id)}
        {@const isExpanded = expandedGroups.has(group.id)}
        {@const groupProfiles = grouped().byGroup.get(group.id) ?? []}
        <li class="group-item" role="treeitem" aria-expanded={isExpanded}>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="group-header-wrapper"
            oncontextmenu={(e) => showGroupContextMenu(e, group)}
          >
            {#if renamingGroupId === group.id}
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
              <button
                class="group-header"
                onclick={() => toggleGroup(group.id)}
                aria-label="{isExpanded ? 'Collapse' : 'Expand'} {group.name}"
              >
                <span class="group-chevron" class:open={isExpanded} aria-hidden="true">›</span>
                <span class="group-name">{group.name}</span>
                <span class="group-count">{groupProfiles.length}</span>
              </button>
            {/if}
          </div>

          {#if isExpanded}
            <ul role="group" class="group-children">
              {#each groupProfiles as profile (profile.id)}
                {@const status = statusClass(profile.id)}
                <li class="tree-item" role="treeitem" aria-selected={false}>
                  <div class="connection-row" class:active={status === 'connected'}>
                    {#if profile.color}
                      <span class="color-dot" style="background: {profile.color};" aria-hidden="true"></span>
                    {/if}
                    <span class="status-dot status-dot--{status}" title={statusLabel(profile.id)} aria-label={statusLabel(profile.id)}></span>
                    <button
                      class="connection-name"
                      onclick={() => status === 'connected' ? openQueryEditor(profile) : handleConnect(profile)}
                      title={profile.host}
                    >{profile.name}</button>
                    <span class="db-badge">{profile.dbType}</span>
                    <button
                      class="edit-btn"
                      title="Edit connection"
                      aria-label="Edit {profile.name}"
                      onclick={(e) => { e.stopPropagation(); editingProfile = profile; }}
                    >✎</button>
                    <button
                      class="delete-btn"
                      title="Delete connection"
                      aria-label="Delete {profile.name}"
                      onclick={(e) => { e.stopPropagation(); deleteConnection(profile); }}
                    >✕</button>
                  </div>
                  {#if connectionStore.errorIds.has(profile.id)}
                    <div class="error-row">{connectionStore.errorIds.get(profile.id)}</div>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}

    </ul>
  {/if}
</div>

<!-- Group context menu -->
{#if groupContextMenu}
  <div
    class="group-context-menu"
    role="menu"
    aria-label="Group options"
    style="top: {groupContextMenu.y}px; left: {groupContextMenu.x}px;"
  >
    <button class="ctx-item" role="menuitem" onclick={ctxNewConnectionInGroup}>
      New Connection in Group
    </button>
    <button class="ctx-item" role="menuitem" onclick={ctxRenameGroup}>Rename Group</button>
    <button class="ctx-item ctx-item--danger" role="menuitem" onclick={ctxDeleteGroup}>
      Delete Group
    </button>
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
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
  }

  .tree-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .header-label {
    flex: 1;
  }

  .add-btn {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-lg);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .add-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .empty-state {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .link-btn {
    color: var(--color-accent);
    font-size: var(--font-size-sm);
    text-decoration: underline;
    margin-top: var(--spacing-1);
    display: block;
  }

  .tree-list {
    padding: var(--spacing-1) 0;
  }

  /* ── Groups ── */

  .group-item {
    display: flex;
    flex-direction: column;
  }

  .group-header-wrapper {
    display: flex;
  }

  .group-header {
    flex: 1;
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-3);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
    user-select: none;
  }

  .group-header:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-secondary);
  }

  .group-chevron {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    transition: transform var(--transition-fast);
    display: inline-block;
    width: 10px;
  }

  .group-chevron.open {
    transform: rotate(90deg);
  }

  .group-name {
    flex: 1;
  }

  .group-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    padding: 0 4px;
    font-weight: var(--font-weight-normal);
    text-transform: none;
    letter-spacing: normal;
  }

  .group-children {
    padding: 0;
  }

  .rename-input {
    flex: 1;
    height: 24px;
    padding: 0 var(--spacing-2);
    margin: 2px var(--spacing-3);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    outline: none;
  }

  /* ── Connection rows ── */

  .tree-item {
    display: flex;
    flex-direction: column;
  }

  .connection-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-3);
    transition: background var(--transition-fast);
  }

  .connection-row:hover {
    background: var(--color-bg-hover);
  }

  .connection-row.active {
    background: var(--color-accent-subtle);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-dot--connected {
    background: var(--color-connection-connected);
  }

  .status-dot--connecting {
    background: var(--color-connection-connecting);
    animation: pulse 1s infinite;
  }

  .status-dot--error {
    background: var(--color-connection-error);
  }

  .status-dot--idle {
    background: var(--color-border-strong);
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

  .connection-name {
    flex: 1;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: var(--font-weight-medium);
  }

  .connection-name:hover {
    color: var(--color-accent);
  }

  .db-badge {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    padding: 1px var(--spacing-1);
    text-transform: lowercase;
    flex-shrink: 0;
  }

  .error-row {
    padding: 2px var(--spacing-3) var(--spacing-1) 36px;
    font-size: var(--font-size-xs);
    color: var(--color-danger);
  }

  .edit-btn {
    opacity: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
    transition: opacity var(--transition-fast), color var(--transition-fast),
      background var(--transition-fast);
  }

  .connection-row:hover .edit-btn {
    opacity: 1;
  }

  .edit-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-active);
  }

  .delete-btn {
    opacity: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
    transition: opacity var(--transition-fast), color var(--transition-fast),
      background var(--transition-fast);
  }

  .connection-row:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    color: var(--color-danger);
    background: var(--color-bg-active);
  }

  /* ── Group context menu ── */

  .group-context-menu {
    position: fixed;
    z-index: 500;
    min-width: 180px;
    padding: var(--spacing-1) 0;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border);
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
  }

  .ctx-item:hover {
    background: var(--color-bg-active);
  }

  .ctx-item--danger {
    color: var(--color-danger);
  }
</style>
