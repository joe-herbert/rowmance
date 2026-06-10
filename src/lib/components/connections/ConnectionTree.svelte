<!--
  ConnectionTree — lists all connection profiles grouped by folder.
  Each connection shows a status dot and a context menu.
  Clicking a disconnected connection prompts to connect; clicking a connected one expands the schema.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import ConnectionForm from './ConnectionForm.svelte';
  import type { ConnectionProfile } from '$lib/types';

  const connectionStore = useConnections();
  const panelStore = usePanels();

  let showAddForm = $state(false);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);

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
</script>

<div class="connection-tree">
  <div class="tree-header no-select">
    <span class="header-label">Connections</span>
    <button
      class="add-btn"
      title="Add connection"
      aria-label="Add new connection"
      onclick={() => (showAddForm = true)}>+</button
    >
  </div>

  {#if connectionStore.profiles.length === 0}
    <div class="empty-state">
      <p>No connections yet.</p>
      <button class="link-btn" onclick={() => (showAddForm = true)}
        >Add your first connection</button
      >
    </div>
  {:else}
    <ul class="tree-list" role="tree" aria-label="Database connections">
      {#each connectionStore.profiles as profile (profile.id)}
        {@const status = statusClass(profile.id)}
        <li class="tree-item" role="treeitem" aria-selected={false}>
          <div class="connection-row" class:active={status === 'connected'}>
            <!-- Connection colour accent -->
            {#if profile.color}
              <span class="color-dot" style="background: {profile.color};" aria-hidden="true"
              ></span>
            {/if}

            <!-- Status dot -->
            <span
              class="status-dot status-dot--{status}"
              title={statusLabel(profile.id)}
              aria-label={statusLabel(profile.id)}
            ></span>

            <!-- Name -->
            <button
              class="connection-name"
              onclick={() =>
                status === 'connected' ? openQueryEditor(profile) : handleConnect(profile)}
              title={profile.host}
            >
              {profile.name}
            </button>

            <!-- DB type badge -->
            <span class="db-badge">{profile.dbType}</span>

            <!-- Edit button -->
            <button
              class="edit-btn"
              title="Edit connection"
              aria-label="Edit {profile.name}"
              onclick={(e) => { e.stopPropagation(); editingProfile = profile; }}
            >✎</button>
          </div>

          <!-- Error message inline -->
          {#if connectionStore.errorIds.has(profile.id)}
            <div class="error-row">
              {connectionStore.errorIds.get(profile.id)}
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if showAddForm}
  <ConnectionForm onclose={() => (showAddForm = false)} />
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
</style>
