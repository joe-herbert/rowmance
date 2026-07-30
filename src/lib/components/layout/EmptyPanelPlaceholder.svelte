<!--
  EmptyPanelPlaceholder — shown in a panel when no tab is open.
  Surfaces connected connections (with a quick "new query" action), a table
  search backed by the same schema cache as GlobalSearch, recently viewed
  dashboards, saved queries, and a shortcut to start a new AI chat.
-->
<script lang="ts">
  import Fuse from 'fuse.js';
  import { onMount } from 'svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { useGlobalSearchCache } from '$lib/stores/globalSearchCache.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import type { FileQuery, ConnectionProfile } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import DbIcon from '$lib/components/icons/DbIcon.svelte';
  import LinkIcon from '$lib/components/icons/LinkIcon.svelte';
  import PlusIcon from '$lib/components/icons/PlusIcon.svelte';
  import SearchIcon from '$lib/components/icons/SearchIcon.svelte';
  import TableIcon from '$lib/components/icons/TableIcon.svelte';
  import SavedQueryIcon from '$lib/components/icons/SavedQueryIcon.svelte';
  import ChatIcon from '$lib/components/icons/ChatIcon.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import ShareIcon from '$lib/components/icons/ShareIcon.svelte';

  const connections = useConnections();
  const panels = usePanels();
  const dashboardsStore = useDashboards();
  const settingsStore = useSettings();
  const searchCache = useGlobalSearchCache();
  const toast = useToast();

  const hasConnections = $derived(connections.profiles.length > 0);
  const connectedProfiles = $derived(
    connections.profiles.filter((p) => connections.activeIds.has(p.id)),
  );
  const aiEnabled = $derived(settingsStore.settings.aiProvider !== 'none');

  let savedQueries = $state<FileQuery[]>([]);
  let searchInputEl = $state<HTMLInputElement>();
  let hasFocusedSearch = false;

  $effect(() => {
    if (searchInputEl && !hasFocusedSearch) {
      hasFocusedSearch = true;
      searchInputEl.focus();
    }
  });

  onMount(() => {
    savedQueriesApi
      .fileListSavedQueries()
      .then((result) => {
        savedQueries = [...result.queries]
          .sort((a, b) => b.updatedAt.localeCompare(a.updatedAt))
          .slice(0, 5);
      })
      .catch(() => {});
  });

  $effect(() => {
    if (connectedProfiles.length > 0) {
      searchCache.populate(connectedProfiles);
    }
  });

  const recentDashboards = $derived(
    [...dashboardsStore.dashboards]
      .filter((d) => d.lastViewedAt)
      .sort((a, b) => (b.lastViewedAt ?? '').localeCompare(a.lastViewedAt ?? ''))
      .slice(0, 5),
  );

  // ── Table search ──────────────────────────────────────────────────────────

  let query = $state('');
  let selectedIndex = $state(0);

  const connectedIds = $derived(new Set(connectedProfiles.map((p) => p.id)));

  const fuseTables = $derived(
    new Fuse(
      searchCache.tableEntries.filter((e) => connectedIds.has(e.connectionId)),
      { keys: ['name', 'database', 'connectionName'], threshold: 0.4, includeScore: true },
    ),
  );

  const tableResults = $derived.by(() => {
    const q = query.trim();
    if (!q) return [];
    return fuseTables.search(q).slice(0, 8).map((r) => r.item);
  });

  $effect(() => {
    query;
    selectedIndex = 0;
  });

  function openTable(item: (typeof tableResults)[number]) {
    panels.openInFocused({
      kind: 'table_browser',
      connectionId: item.connectionId,
      database: item.database,
      table: item.name,
      instanceDb: item.instanceDb,
    });
    query = '';
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (tableResults.length === 0) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % tableResults.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + tableResults.length) % tableResults.length;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = tableResults[selectedIndex];
      if (item) openTable(item);
    }
  }

  // ── Actions ───────────────────────────────────────────────────────────────

  function openConnectionSwitcher() {
    document.dispatchEvent(
      new CustomEvent('shortcut-action', {
        detail: { action: 'CONNECT_TO_CONNECTION' },
        bubbles: true,
      }),
    );
  }

  function openConnectionsPage() {
    panels.openInFocused({ kind: 'connections' });
  }

  function newQuery(connectionId: string) {
    panels.openInFocused({ kind: 'query_editor', connectionId });
  }

  let connCtx = $state<{ x: number; y: number; profile: ConnectionProfile } | null>(null);

  function showConnCtx(e: MouseEvent, profile: ConnectionProfile) {
    e.preventDefault();
    connCtx = { x: e.clientX, y: e.clientY, profile };
  }

  function closeConnCtx() {
    connCtx = null;
  }

  async function ctxDisconnect() {
    if (!connCtx) return;
    const id = connCtx.profile.id;
    connCtx = null;
    await connections.disconnect(id);
    panels.closeItemsForConnection(id, { skipDirty: true });
  }

  async function ctxCopyName() {
    if (!connCtx) return;
    const name = connCtx.profile.name;
    connCtx = null;
    try {
      await navigator.clipboard.writeText(name);
      toast.addToast('Name copied', 'success');
    } catch (err) {
      toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error', 0);
    }
  }

  function openDashboard(dashboardId: string) {
    panels.openInFocused({ kind: 'dashboard', dashboardId });
  }

  function connectionColor(connectionId: string | null): string {
    const profile = connectionId
      ? connections.profiles.find((p) => p.id === connectionId)
      : undefined;
    return profile?.color ?? 'var(--color-accent)';
  }

  function openSavedQuery(q: FileQuery) {
    const existing = panels.openItems.find(
      (item) => item.content.kind === 'query_editor' && item.content.savedQueryId === q.id,
    );
    if (existing) {
      panels.showItem(existing);
      return;
    }
    const connectionId = q.connectionId ?? connections.profiles[0]?.id ?? '';
    panels.openInFocused({
      kind: 'query_editor',
      connectionId,
      database: q.database ?? undefined,
      initialSql: q.sql,
      initialDescription: q.description ?? undefined,
      initialAnnotations: q.annotations ?? undefined,
      savedQueryId: q.id,
      savedQueryName: q.name,
    });
  }

  function startNewAiChat() {
    const draftId = `draft-${crypto.randomUUID()}`;
    panels.openCopyInFocused({ kind: 'ai_chat', conversationId: draftId });
  }
</script>

<div class="empty-panel">
  {#if !hasConnections}
    <div class="empty-panel-hint">
      <div class="empty-panel-icon" aria-hidden="true">
        <DbIcon size={48} />
      </div>
      <p class="empty-panel-title">No database connections</p>
      <p class="empty-panel-subtitle">Add a connection in the left sidebar to get started</p>
    </div>
  {:else}
    <div class="start-page">
      <div class="start-section">
        <div class="start-section-header">
          <span class="start-section-title">Connections</span>
          <div class="start-section-actions">
            <button class="start-link-btn" type="button" onclick={openConnectionSwitcher}>
              <LinkIcon width={11} height={11} strokeWidth={2} />
              Connect
            </button>
            <button
              class="start-icon-btn"
              type="button"
              title="Manage all connections"
              aria-label="Manage all connections"
              onclick={openConnectionsPage}
            >
              <ShareIcon width={12} height={12} strokeWidth={1.8} />
            </button>
          </div>
        </div>
        {#if connectedProfiles.length > 0}
          <ul class="start-chip-list start-chip-list--stacked">
            {#each connectedProfiles as profile (profile.id)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <li
                class="start-chip start-chip--connection"
                style="--chip-color: {profile.color ?? 'var(--color-accent)'}"
                role="button"
                tabindex="0"
                title="New query on {profile.name}"
                onclick={() => newQuery(profile.id)}
                oncontextmenu={(e) => showConnCtx(e, profile)}
              >
                <span class="start-connection-dot" aria-hidden="true"></span>
                <span class="start-chip-label">{profile.name}</span>
                <span class="start-connection-type">{profile.dbType}</span>
                <PlusIcon width={11} height={11} strokeWidth={2} />
              </li>
            {/each}
          </ul>
        {:else}
          <p class="start-empty-note">No connections open right now.</p>
        {/if}
      </div>

      {#if connCtx}
        <ContextMenu x={connCtx.x} y={connCtx.y} open={true} onclose={closeConnCtx}>
          <CtxItem onclick={ctxDisconnect}>Disconnect</CtxItem>
          <CtxItem onclick={ctxCopyName}>Copy name</CtxItem>
        </ContextMenu>
      {/if}

      {#if connectedProfiles.length > 0}
        <div class="start-section">
          <div class="start-search">
            <span class="start-search-icon" aria-hidden="true">
              <SearchIcon width={14} height={14} strokeWidth={1.8} />
            </span>
            <input
              type="text"
              class="start-search-input"
              placeholder="Search tables in connected databases…"
              bind:value={query}
              bind:this={searchInputEl}
              onkeydown={handleSearchKeydown}
              autocomplete="off"
              spellcheck="false"
              aria-label="Search tables"
            />
          </div>
          {#if tableResults.length > 0}
            <ul class="start-results" role="listbox">
              {#each tableResults as item, index (`${item.connectionId}:${item.instanceDb ?? ''}:${item.database}:${item.name}`)}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <li
                  class="start-result"
                  class:selected={index === selectedIndex}
                  role="option"
                  aria-selected={index === selectedIndex}
                  onclick={() => openTable(item)}
                  onmouseenter={() => (selectedIndex = index)}
                >
                  <span class="start-result-icon" aria-hidden="true">
                    <TableIcon size={12} />
                  </span>
                  <span class="start-result-label">{item.name}</span>
                  <span class="start-result-subtitle">
                    {item.instanceDb
                      ? `${item.connectionName} › ${item.instanceDb} › ${item.database}`
                      : `${item.connectionName} › ${item.database}`}
                  </span>
                </li>
              {/each}
            </ul>
          {:else if query.trim()}
            <p class="start-empty-note">No matching tables</p>
          {/if}
        </div>
      {/if}

      {#if recentDashboards.length > 0}
        <div class="start-section">
          <div class="start-section-header">
            <span class="start-section-title">Recent dashboards</span>
          </div>
          <ul class="start-chip-list">
            {#each recentDashboards as dashboard (dashboard.id)}
              <li>
                <button
                  class="start-chip"
                  type="button"
                  style="--chip-color: {dashboard.color ?? 'var(--color-accent)'}"
                  onclick={() => openDashboard(dashboard.id)}
                  title={dashboard.name}
                >
                  <span class="start-chip-icon" aria-hidden="true">{@html dashboard.icon}</span>
                  <span class="start-chip-label">{dashboard.name}</span>
                </button>
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if savedQueries.length > 0}
        <div class="start-section">
          <div class="start-section-header">
            <span class="start-section-title">Saved queries</span>
          </div>
          <ul class="start-chip-list">
            {#each savedQueries as savedQuery (savedQuery.id)}
              <li>
                <button
                  class="start-chip"
                  type="button"
                  style="--chip-color: {connectionColor(savedQuery.connectionId)}"
                  onclick={() => openSavedQuery(savedQuery)}
                  title={savedQuery.name}
                >
                  <span class="start-chip-icon" aria-hidden="true">
                    <SavedQueryIcon width={12} height={12} />
                  </span>
                  <span class="start-chip-label">{savedQuery.name}</span>
                </button>
              </li>
            {/each}
          </ul>
        </div>
      {/if}

      {#if aiEnabled}
        <div class="start-section">
          <button class="start-ai-btn" type="button" onclick={startNewAiChat}>
            <ChatIcon width={13} height={13} />
            New AI chat
          </button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .empty-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    overflow: auto;
    padding: var(--spacing-6) 0;
  }

  .empty-panel-hint {
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    max-width: 260px;
    line-height: var(--line-height-normal);
  }

  .empty-panel-icon {
    font-size: 40px;
    margin-bottom: var(--spacing-3);
    line-height: 1;
    opacity: 0.5;
  }

  .empty-panel-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    margin: 0 0 var(--spacing-2);
  }

  .empty-panel-subtitle {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
    line-height: var(--line-height-normal);
  }

  .start-page {
    width: 100%;
    max-width: 420px;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-5);
    padding: 0 var(--spacing-4);
  }

  .start-section {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .start-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-2);
  }

  .start-section-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .start-section-title {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .start-search {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    transition: border-color var(--transition-fast);
  }

  .start-search:focus-within {
    border-color: var(--color-accent);
  }

  .start-search-icon {
    color: var(--color-text-muted);
    display: flex;
    align-items: center;
    flex-shrink: 0;
  }

  .start-search-input {
    flex: 1;
    border: none;
    background: transparent;
    outline: none;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    color: var(--color-text-primary);
  }

  .start-search-input::placeholder {
    color: var(--color-text-disabled);
  }

  .start-results {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    max-height: 220px;
    overflow-y: auto;
  }

  .start-result {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-3);
    cursor: pointer;
    text-align: left;
  }

  .start-result.selected,
  .start-result:hover {
    background: var(--color-bg-hover);
  }

  .start-result-icon {
    display: flex;
    align-items: center;
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  .start-result-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .start-result-subtitle {
    margin-left: auto;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
    flex-shrink: 0;
  }

  .start-empty-note {
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-style: italic;
    margin: 0;
  }

  .start-link-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: var(--spacing-1) var(--spacing-2);
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
  }

  .start-link-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .start-connection-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--chip-color);
    flex-shrink: 0;
  }

  .start-connection-type {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    margin-left: auto;
    flex-shrink: 0;
  }

  .start-icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    color: var(--color-text-muted);
    border-radius: var(--radius-lg);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .start-icon-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .start-chip-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-2);
  }

  .start-chip {
    --chip-color: var(--color-accent);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 200px;
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background: var(--color-bg-primary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    text-align: left;
    transition:
      background-color var(--transition-slow),
      border-color var(--transition-slow),
      color var(--transition-slow);
  }

  .start-chip:hover {
    background: color-mix(in srgb, var(--chip-color) 5%, transparent);
    border-color: var(--chip-color);
    color: var(--color-text-primary);
  }

  .start-chip-list--stacked {
    flex-wrap: nowrap;
    flex-direction: column;
  }

  .start-chip-list--stacked > li {
    width: 100%;
  }

  .start-chip--connection {
    width: 100%;
    max-width: none;
  }

  .start-chip-icon {
    display: flex;
    align-items: center;
    width: 13px;
    height: 13px;
    flex-shrink: 0;
    color: var(--chip-color);
  }

  .start-chip-icon :global(svg) {
    width: 100%;
    height: 100%;
  }

  .start-chip-label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .start-ai-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 5px 10px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .start-ai-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .start-ai-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
