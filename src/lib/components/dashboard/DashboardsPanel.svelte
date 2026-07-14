<!--
  DashboardsPanel — right sidebar panel listing all dashboards.
  Create, rename, delete, pin/unpin, and open dashboards.
-->
<script lang="ts">
  import { useDashboards } from '$lib/stores/dashboards.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { portal } from '$lib/actions/portal';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import IconPicker from './IconPicker.svelte';

  const dashboardsStore = useDashboards();
  const panelStore = usePanels();

  // ── Default icon ──────────────────────────────────────────────────────────

  const DEFAULT_ICON = `<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/></svg>`;

  // ── New dashboard form ────────────────────────────────────────────────────

  let showNewForm = $state(false);
  let newName = $state('');
  let newIcon = $state(DEFAULT_ICON);

  async function createDashboard() {
    if (!newName.trim()) return;
    const dashboard = await dashboardsStore.create({ name: newName.trim(), icon: newIcon });
    newName = '';
    newIcon = DEFAULT_ICON;
    showNewForm = false;
    panelStore.openInFocused({ kind: 'dashboard', dashboardId: dashboard.id });
  }

  // ── Context menu ──────────────────────────────────────────────────────────

  interface CtxMenu {
    x: number;
    y: number;
    id: string;
    name: string;
  }

  let ctxMenu = $state<CtxMenu | null>(null);

  function showCtxMenu(e: MouseEvent, id: string, name: string) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY, id, name };
  }

  // ── Rename ────────────────────────────────────────────────────────────────

  let renamingId = $state<string | null>(null);
  let renameValue = $state('');
  let renameInput = $state<HTMLInputElement | undefined>(undefined);

  function startRename(id: string, name: string) {
    ctxMenu = null;
    renamingId = id;
    renameValue = name;
    requestAnimationFrame(() => {
      renameInput?.focus();
      renameInput?.select();
    });
  }

  function commitRename() {
    if (renamingId && renameValue.trim()) {
      dashboardsStore.update(renamingId, { name: renameValue.trim() });
    }
    renamingId = null;
  }

  // ── Delete ────────────────────────────────────────────────────────────────

  let confirmDeleteId = $state<string | null>(null);
  let confirmDeleteName = $state('');

  function startDelete(id: string, name: string) {
    ctxMenu = null;
    confirmDeleteId = id;
    confirmDeleteName = name;
  }

  function confirmDelete() {
    if (!confirmDeleteId) return;
    void dashboardsStore.delete(confirmDeleteId);
    confirmDeleteId = null;
  }

  // ── Open ──────────────────────────────────────────────────────────────────

  function openDashboard(id: string) {
    panelStore.openInFocused({ kind: 'dashboard', dashboardId: id });
  }
</script>

<div class="dashboards-panel">
  <div class="panel-toolbar">
    <span class="panel-title">Dashboards</span>
    <div class="toolbar-gap"></div>
    <button
      class="icon-btn"
      onclick={() => (showNewForm = !showNewForm)}
      title="New dashboard"
      aria-label="New dashboard"
      type="button"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <line x1="12" y1="5" x2="12" y2="19"/>
        <line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
    </button>
  </div>

  {#if showNewForm}
    <div class="new-form">
      <input
        class="new-name-input"
        type="text"
        placeholder="Dashboard name…"
        bind:value={newName}
        onkeydown={(e) => {
          if (e.key === 'Enter') createDashboard();
          if (e.key === 'Escape') { showNewForm = false; }
        }}
        autofocus
      />
      <div class="icon-picker-wrap">
        <span class="icon-picker-label">Icon</span>
        <IconPicker value={newIcon} onchange={(s) => (newIcon = s)} />
      </div>
      <div class="new-form-actions">
        <button class="action-btn" onclick={() => (showNewForm = false)} type="button">Cancel</button>
        <button class="action-btn action-btn--primary" onclick={createDashboard} disabled={!newName.trim()} type="button">Create</button>
      </div>
    </div>
  {/if}

  {#if dashboardsStore.dashboards.length === 0 && !showNewForm}
    <div class="empty-row">No dashboards yet.</div>
  {:else}
    <ul class="dashboard-list" role="list">
      {#each dashboardsStore.dashboards as dashboard (dashboard.id)}
        {@const isPinned = dashboard.pinned}
        {@const canPin = !isPinned && dashboardsStore.canPin()}

        <li class="dashboard-item" role="listitem">
          {#if renamingId === dashboard.id}
            <div class="rename-row">
              <input
                bind:this={renameInput}
                class="rename-input"
                type="text"
                bind:value={renameValue}
                onblur={commitRename}
                onkeydown={(e) => {
                  if (e.key === 'Enter') commitRename();
                  if (e.key === 'Escape') { renamingId = null; }
                }}
              />
            </div>
          {:else}
            <div class="dashboard-row">
              <button
                class="dashboard-btn"
                onclick={() => openDashboard(dashboard.id)}
                oncontextmenu={(e) => showCtxMenu(e, dashboard.id, dashboard.name)}
                title="Open {dashboard.name}"
                type="button"
              >
                <span class="dash-icon" aria-hidden="true">
                  {@html dashboard.icon}
                </span>
                <span class="dash-name">{dashboard.name}</span>
              </button>
              <button
                class="pin-btn"
                class:pin-btn--active={isPinned}
                onclick={() => dashboardsStore.togglePin(dashboard.id)}
                title={isPinned ? 'Unpin from title bar' : canPin ? 'Pin to title bar' : 'Max 3 pinned'}
                disabled={!isPinned && !canPin}
                aria-label={isPinned ? 'Unpin' : 'Pin to title bar'}
                type="button"
              >
                <svg width="11" height="11" viewBox="0 0 24 24" fill={isPinned ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <line x1="12" y1="17" x2="12" y2="22"></line>
                  <path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z"></path>
                </svg>
              </button>
            </div>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</div>

<ContextMenu
  x={ctxMenu?.x ?? 0}
  y={ctxMenu?.y ?? 0}
  open={ctxMenu !== null}
  onclose={() => (ctxMenu = null)}
  minWidth={140}
  zIndex={1000}
>
  {#if ctxMenu}
    {@const d = dashboardsStore.getById(ctxMenu.id)}
    <CtxItem onclick={() => openDashboard(ctxMenu!.id)}>Open</CtxItem>
    <CtxItem onclick={() => startRename(ctxMenu!.id, ctxMenu!.name)}>Rename</CtxItem>
    <CtxItem onclick={() => {
      if (!d) return;
      dashboardsStore.togglePin(ctxMenu!.id);
      ctxMenu = null;
    }}>
      {d?.pinned ? 'Unpin from title bar' : 'Pin to title bar'}
    </CtxItem>
    <CtxSep />
    <CtxItem danger onclick={() => startDelete(ctxMenu!.id, ctxMenu!.name)}>Delete</CtxItem>
  {/if}
</ContextMenu>

{#if confirmDeleteId}
  <ConfirmDialog
    title="Delete dashboard"
    message={`Delete "${confirmDeleteName}"? This cannot be undone.`}
    confirmText="Delete"
    cancelText="Cancel"
    danger
    onconfirm={confirmDelete}
    oncancel={() => (confirmDeleteId = null)}
  />
{/if}

<style>
  .dashboards-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
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

  .toolbar-gap {
    flex: 1;
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
    border: none;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  /* ── New form ─────────────────────────────────────────────────────────────── */

  .new-form {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-2);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .new-name-input {
    width: 100%;
    padding: 5px var(--spacing-2);
    font-size: var(--font-size-sm);
    background: var(--color-bg-input, var(--color-bg-primary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
    box-sizing: border-box;
  }

  .icon-picker-wrap {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .icon-picker-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
  }

  .new-form-actions {
    display: flex;
    gap: var(--spacing-1);
    justify-content: flex-end;
  }

  .action-btn {
    font-size: var(--font-size-xs);
    padding: 3px var(--spacing-2);
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

  .action-btn--primary {
    background: var(--color-accent);
    color: #fff;
    border-color: transparent;
    font-weight: var(--font-weight-medium);
  }

  .action-btn--primary:hover:not(:disabled) {
    opacity: 0.9;
    color: #fff;
  }

  .action-btn--primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Dashboard list ──────────────────────────────────────────────────────── */

  .empty-row {
    padding: var(--spacing-3) var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .dashboard-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1);
  }

  .dashboard-item {
    display: flex;
    align-items: center;
  }

  .dashboard-row {
    display: flex;
    align-items: center;
    width: 100%;
    border-radius: var(--radius-md);
    transition: background var(--transition-fast);
  }

  .dashboard-row:hover {
    background: var(--color-bg-hover);
  }

  .dashboard-row:hover .pin-btn {
    opacity: 1;
  }

  .dashboard-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex: 1;
    min-width: 0;
    padding: 6px var(--spacing-2);
    font-size: 13px;
    color: var(--color-text-secondary);
    text-align: left;
    cursor: pointer;
    border-radius: var(--radius-md);
    background: none;
    border: none;
    transition: color var(--transition-fast);
  }

  .dashboard-btn:hover {
    color: var(--color-text-primary);
  }

  .dash-icon {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    color: var(--color-accent);
  }

  .dash-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pin-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    margin-right: 4px;
    border-radius: var(--radius-sm);
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    opacity: 0;
    flex-shrink: 0;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .pin-btn--active {
    opacity: 1 !important;
    color: var(--color-accent);
  }

  .pin-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .pin-btn:disabled {
    cursor: not-allowed;
    opacity: 0.3 !important;
  }

  .rename-row {
    width: 100%;
    padding: 3px var(--spacing-2);
  }

  .rename-input {
    width: 100%;
    padding: 4px var(--spacing-2);
    font-size: var(--font-size-sm);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
    box-sizing: border-box;
  }
</style>
