<!--
  ConnectionSwitcher — opens via the "Connect to Connection" shortcut/command.
  Fuzzy-searches saved connections and connects to the selected one.
-->
<script lang="ts">
  import Fuse from 'fuse.js';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useTags } from '$lib/stores/tags.svelte';
  import { useToast } from '$lib/stores/toast.svelte';
  import { focusTrap } from '$lib/utils/focus-trap';
  import { errorMessage } from '$lib/utils/errors';
  import * as connectionsApi from '$lib/tauri/connections';
  import type { ConnectionProfile } from '$lib/types';
  import SearchIcon from '$lib/components/icons/SearchIcon.svelte';
  import SpinnerIcon from '$lib/components/icons/SpinnerIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import LockIcon from '$lib/components/icons/LockIcon.svelte';
  import ShieldIcon from '$lib/components/icons/ShieldIcon.svelte';
  import GearIcon from '$lib/components/icons/GearIcon.svelte';
  import ShareIcon from '$lib/components/icons/ShareIcon.svelte';
  import ContextMenu from '$lib/components/ui/ContextMenu.svelte';
  import CtxItem from '$lib/components/ui/CtxItem.svelte';
  import CtxSep from '$lib/components/ui/CtxSep.svelte';
  import CtxSubmenuItem from '$lib/components/ui/CtxSubmenuItem.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import ConnectionForm from '$lib/components/connections/ConnectionForm.svelte';
  import ExportConnectionsDialog from '$lib/components/connections/ExportConnectionsDialog.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';

  interface Props {
    onclose: () => void;
  }

  const { onclose }: Props = $props();

  const connections = useConnections();
  const panels = usePanels();
  const tagsStore = useTags();
  const toast = useToast();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);
  let connectingId = $state<string | null>(null);
  let disconnectingId = $state<string | null>(null);

  interface ConnectionItem {
    id: string;
    name: string;
    dbType: string;
    color: string | null;
    connected: boolean;
    readOnly: boolean;
    safeMode: boolean;
    groupName: string | null;
    tags: { id: string; name: string; color: string | null }[];
  }

  const items = $derived.by<ConnectionItem[]>(() =>
    connections.profiles.map((p) => ({
      id: p.id,
      name: p.name,
      dbType: p.dbType,
      color: p.color,
      connected: connections.activeIds.has(p.id),
      readOnly: p.readOnly,
      safeMode: p.safeMode,
      groupName: connections.groups.find((g) => g.id === p.groupId)?.name ?? null,
      tags: p.tags,
    })),
  );

  // ── Display settings (persisted) ──────────────────────────────────────────

  interface DisplaySettings {
    showTags: boolean;
    showFolder: boolean;
    showDbType: boolean;
    showReadOnly: boolean;
    showSafeMode: boolean;
  }

  const DISPLAY_SETTINGS_KEY = 'connectionSwitcher.displaySettings';

  function loadDisplaySettings(): DisplaySettings {
    const defaults: DisplaySettings = {
      showTags: true,
      showFolder: true,
      showDbType: true,
      showReadOnly: true,
      showSafeMode: true,
    };
    try {
      const saved = JSON.parse(localStorage.getItem(DISPLAY_SETTINGS_KEY) ?? '{}');
      return { ...defaults, ...saved };
    } catch {
      return defaults;
    }
  }

  let displaySettings = $state<DisplaySettings>(loadDisplaySettings());
  let displaySettingsOpen = $state(false);
  let settingsBtnEl = $state<HTMLElement | undefined>(undefined);
  let settingsMenuEl = $state<HTMLElement | undefined>(undefined);

  $effect(() => {
    localStorage.setItem(DISPLAY_SETTINGS_KEY, JSON.stringify(displaySettings));
  });

  $effect(() => {
    if (!displaySettingsOpen) return;
    function handleClickOutside(e: MouseEvent) {
      const target = e.target as Node;
      if (
        settingsMenuEl &&
        !settingsMenuEl.contains(target) &&
        settingsBtnEl &&
        !settingsBtnEl.contains(target)
      ) {
        displaySettingsOpen = false;
      }
    }
    setTimeout(() => document.addEventListener('click', handleClickOutside), 0);
    return () => document.removeEventListener('click', handleClickOutside);
  });

  const fuse = $derived(
    new Fuse(items, { keys: ['name', 'dbType'], threshold: 0.4, includeScore: true }),
  );

  const filtered = $derived.by<ConnectionItem[]>(() => {
    const q = query.trim();
    if (!q) return items;
    return fuse.search(q).map((r) => r.item);
  });

  async function disconnectItem(item: ConnectionItem) {
    if (!item.connected) return;
    disconnectingId = item.id;
    try {
      await connections.disconnect(item.id);
      toast.addToast(`Disconnected from "${item.name}"`, 'success', 2000);
    } catch (err) {
      toast.addToast(`Failed to disconnect from "${item.name}": ${errorMessage(err)}`, 'error');
    } finally {
      disconnectingId = null;
    }
  }

  async function executeItem(
    item: ConnectionItem,
    opts: { disconnect?: boolean; keepOpen?: boolean } = {},
  ) {
    if (opts.disconnect) {
      await disconnectItem(item);
      return;
    }
    if (item.connected) {
      if (!opts.keepOpen) onclose();
      return;
    }
    connectingId = item.id;
    try {
      await connections.connect(item.id);
      toast.addToast(`Connected to "${item.name}"`, 'success', 2000);
      if (!opts.keepOpen) onclose();
    } catch (err) {
      toast.addToast(`Failed to connect to "${item.name}": ${errorMessage(err)}`, 'error');
    } finally {
      connectingId = null;
    }
  }

  // ── Right-click context menu ───────────────────────────────────────────────

  interface ConnCtxMenu {
    x: number;
    y: number;
    profile: ConnectionProfile;
  }
  let connCtx = $state<ConnCtxMenu | null>(null);
  let editingProfile = $state<ConnectionProfile | undefined>(undefined);
  let exportProfileId = $state<string | null>(null);
  let showExportDialog = $state(false);

  interface ConfirmState {
    title: string;
    message: string;
    confirmText?: string;
    onconfirm: () => void;
  }
  let confirmState = $state<ConfirmState | null>(null);

  function showConnCtx(e: MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    const profile = connections.getById(id);
    if (!profile) return;
    connCtx = { x: e.clientX, y: e.clientY, profile };
  }

  async function ctxConnect(profile: ConnectionProfile) {
    connCtx = null;
    connectingId = profile.id;
    try {
      await connections.connect(profile.id);
      toast.addToast(`Connected to "${profile.name}"`, 'success', 2000);
    } catch (err) {
      toast.addToast(`Failed to connect to "${profile.name}": ${errorMessage(err)}`, 'error');
    } finally {
      connectingId = null;
    }
  }

  async function ctxDisconnect(profile: ConnectionProfile) {
    connCtx = null;
    const item = items.find((i) => i.id === profile.id);
    if (item) await disconnectItem({ ...item, connected: true });
  }

  function ctxDelete(profile: ConnectionProfile, onDone?: () => void) {
    connCtx = null;
    if (profile.unsaved) {
      confirmState = {
        title: 'Disconnect Connection',
        message: `Disconnect "${profile.name}"? It was never saved, so it will not appear again.`,
        confirmText: 'Disconnect',
        onconfirm: async () => {
          confirmState = null;
          if (connections.isActive(profile.id)) await connections.disconnect(profile.id);
          panels.closeItemsForConnection(profile.id);
          onDone?.();
        },
      };
      return;
    }
    confirmState = {
      title: 'Delete Connection',
      message: `Delete "${profile.name}"? This cannot be undone.`,
      onconfirm: async () => {
        confirmState = null;
        try {
          await connections.delete(profile.id);
          panels.closeItemsForConnection(profile.id);
          toast.addToast(`Deleted "${profile.name}"`, 'success', 2000);
          onDone?.();
        } catch (err) {
          toast.addToast(`Failed to delete "${profile.name}": ${errorMessage(err)}`, 'error');
        }
      },
    };
  }

  function ctxExport(profile: ConnectionProfile) {
    connCtx = null;
    exportProfileId = profile.id;
    showExportDialog = true;
  }

  async function ctxCopyName(profile: ConnectionProfile) {
    connCtx = null;
    try {
      await navigator.clipboard.writeText(profile.name);
      toast.addToast('Name copied', 'success');
    } catch (err) {
      toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error');
    }
  }

  async function ctxCopyDbUrl(profile: ConnectionProfile) {
    connCtx = null;
    try {
      await connectionsApi.copyConnectionDbUrlToClipboard(profile.id);
      toast.addToast('Database URL copied', 'success');
    } catch (err) {
      toast.addToast(`Copy failed: ${errorMessage(err)}`, 'error');
    }
  }

  async function ctxToggleReadOnly(profile: ConnectionProfile) {
    connCtx = null;
    await connections.toggleReadOnly(profile.id);
  }

  async function ctxToggleSafeMode(profile: ConnectionProfile) {
    connCtx = null;
    await connections.toggleSafeMode(profile.id);
  }

  async function ctxToggleTag(profile: ConnectionProfile, tagId: string) {
    connCtx = null;
    const currentIds = profile.tags.map((t) => t.id);
    const newIds = currentIds.includes(tagId)
      ? currentIds.filter((id) => id !== tagId)
      : [...currentIds, tagId];
    await connections.setTags(profile.id, newIds);
  }

  function ctxEdit(profile: ConnectionProfile) {
    connCtx = null;
    editingProfile = profile;
  }

  async function ctxNewQueryEditor(profile: ConnectionProfile) {
    connCtx = null;
    if (!connections.isActive(profile.id)) await connections.connect(profile.id);
    panels.openInFocused({ kind: 'query_editor', connectionId: profile.id });
    onclose();
  }

  // ── Bulk connect / disconnect ─────────────────────────────────────────────

  let bulkBusy = $state(false);

  async function disconnectAll() {
    const activeIds = [...connections.activeIds];
    if (activeIds.length === 0 || bulkBusy) return;
    bulkBusy = true;
    try {
      await Promise.all(activeIds.map((id) => connections.disconnect(id)));
      for (const id of activeIds) panels.closeItemsForConnection(id, { skipDirty: true });
      toast.addToast(
        `Disconnected ${activeIds.length} connection${activeIds.length !== 1 ? 's' : ''}`,
        'success',
        2000,
      );
    } finally {
      bulkBusy = false;
    }
  }

  async function connectAll() {
    const toConnect = connections.profiles.filter((p) => !connections.activeIds.has(p.id));
    if (toConnect.length === 0 || bulkBusy) return;
    bulkBusy = true;
    try {
      const results = await Promise.allSettled(toConnect.map((p) => connections.connect(p.id)));
      const failed = results.filter((r) => r.status === 'rejected').length;
      const succeeded = toConnect.length - failed;
      if (succeeded > 0) {
        toast.addToast(
          `Connected ${succeeded} connection${succeeded !== 1 ? 's' : ''}`,
          'success',
          2000,
        );
      }
      if (failed > 0) {
        toast.addToast(`Failed to connect ${failed} connection${failed !== 1 ? 's' : ''}`, 'error');
      }
    } finally {
      bulkBusy = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (editingProfile || confirmState || showExportDialog || connCtx) return;
    if (e.key === 'Escape' && displaySettingsOpen) {
      displaySettingsOpen = false;
      return;
    }
    if (e.key === 'Escape') {
      onclose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % Math.max(filtered.length, 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex =
        (selectedIndex - 1 + Math.max(filtered.length, 1)) % Math.max(filtered.length, 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = filtered[selectedIndex];
      if (item) executeItem(item, { disconnect: e.altKey, keepOpen: e.shiftKey });
    }
  }

  $effect(() => {
    query;
    selectedIndex = 0;
  });

  $effect(() => {
    inputEl?.focus();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="backdrop"
  role="dialog"
  aria-modal="true"
  aria-label="Connect to Connection"
  tabindex="-1"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div class="palette" use:focusTrap>
    <div class="search-row">
      <span class="search-icon" aria-hidden="true">
        <SearchIcon width={15} height={15} strokeWidth={1.8} />
      </span>
      <input
        class="search-input"
        type="text"
        placeholder="Connect to a connection…"
        bind:value={query}
        bind:this={inputEl}
        autocomplete="off"
        spellcheck="false"
        aria-label="Search connections"
        aria-autocomplete="list"
        aria-controls="connection-switcher-list"
      />
      <button
        class="search-action-btn"
        onclick={connectAll}
        disabled={bulkBusy}
        type="button"
        title="Connect all"
      >
        Connect All
      </button>
      <button
        class="search-action-btn"
        onclick={disconnectAll}
        disabled={bulkBusy}
        type="button"
        title="Disconnect all"
      >
        Disconnect All
      </button>
      <button
        class="search-action-btn search-action-btn--icon"
        onclick={() => {
          panels.openInFocused({ kind: 'connections' });
          onclose();
        }}
        type="button"
        title="Manage all connections"
        aria-label="Manage connections"
      >
        <ShareIcon width={13} height={13} />
      </button>
      <button
        class="search-action-btn search-action-btn--icon"
        class:active={displaySettingsOpen}
        bind:this={settingsBtnEl}
        onclick={() => (displaySettingsOpen = !displaySettingsOpen)}
        type="button"
        title="Display settings"
        aria-label="Display settings"
        aria-expanded={displaySettingsOpen}
        aria-haspopup="true"
      >
        <GearIcon width={13} height={13} />
      </button>
      {#if displaySettingsOpen}
        <div class="display-settings-menu" bind:this={settingsMenuEl} role="menu">
          <label class="display-settings-item">
            <Checkbox size="sm" bind:checked={displaySettings.showTags} />
            Tags
          </label>
          <label class="display-settings-item">
            <Checkbox size="sm" bind:checked={displaySettings.showFolder} />
            Folder
          </label>
          <label class="display-settings-item">
            <Checkbox size="sm" bind:checked={displaySettings.showDbType} />
            Database type
          </label>
          <label class="display-settings-item">
            <Checkbox size="sm" bind:checked={displaySettings.showReadOnly} />
            Read-only icon
          </label>
          <label class="display-settings-item">
            <Checkbox size="sm" bind:checked={displaySettings.showSafeMode} />
            Safe Mode icon
          </label>
        </div>
      {/if}
    </div>

    {#snippet folderAndTags(item: ConnectionItem)}
      {#if displaySettings.showFolder && item.groupName}
        <span class="item-folder" title={item.groupName}>{item.groupName}</span>
      {/if}
      {#if displaySettings.showTags && item.tags.length > 0}
        <span class="item-tags">
          {#each item.tags as tag (tag.id)}
            <span
              class="item-tag-chip"
              style="border-color:{tag.color ?? 'var(--color-border)'};color:{tag.color ??
                'var(--color-text-secondary)'}"
              title={tag.name}
            >
              {tag.name}
            </span>
          {/each}
        </span>
      {/if}
    {/snippet}

    <ul id="connection-switcher-list" class="results-list" role="listbox">
      {#each filtered as item, index (item.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          class="result-item"
          class:selected={index === selectedIndex}
          role="option"
          aria-selected={index === selectedIndex}
          onclick={(e) => executeItem(item, { disconnect: e.altKey, keepOpen: e.shiftKey })}
          onmouseenter={() => (selectedIndex = index)}
          oncontextmenu={(e) => showConnCtx(e, item.id)}
        >
          {#if connectingId === item.id || disconnectingId === item.id}
            <span class="item-status" aria-label={connectingId === item.id ? 'Connecting…' : 'Disconnecting…'}>
              <SpinnerIcon width={12} height={12} strokeWidth={2} class="spin" />
            </span>
          {:else if item.connected}
            <button
              class="item-status item-status--connected"
              style={`color: ${item.color ?? 'var(--color-success)'}`}
              onclick={(e) => {
                e.stopPropagation();
                disconnectItem(item);
              }}
              disabled={disconnectingId === item.id}
              title="Disconnect"
              aria-label="Disconnect from {item.name}"
              type="button"
            >
              <span class="status-icon status-icon--check" aria-hidden="true">
                <CheckIcon width={12} height={12} strokeWidth={2.5} />
              </span>
              <span class="status-icon status-icon--close" aria-hidden="true">
                <CloseIcon width={12} height={12} strokeWidth={2.5} />
              </span>
            </button>
          {:else}
            <span class="item-status" aria-label="Disconnected">
              <span
                class="result-dot result-dot--dim"
                style={item.color ? `background: ${item.color}` : undefined}
                aria-hidden="true"
              ></span>
            </span>
          {/if}
          <span class="item-main">
            <span class="item-label">{item.name}</span>
            {#if displaySettings.showReadOnly && item.readOnly}
              <span class="item-flag" title="Read-only" aria-label="Read-only">
                <LockIcon width={11} height={11} />
              </span>
            {/if}
            {#if displaySettings.showSafeMode && item.safeMode}
              <span class="item-flag" title="Safe Mode" aria-label="Safe Mode">
                <ShieldIcon width={11} height={11} />
              </span>
            {/if}
            {#if displaySettings.showDbType}
              {@render folderAndTags(item)}
            {/if}
          </span>
          {#if displaySettings.showDbType}
            <span class="item-subtitle">{item.dbType}</span>
          {:else}
            <span class="item-trailing">
              {@render folderAndTags(item)}
            </span>
          {/if}
        </li>
      {:else}
        <li class="empty-item" role="presentation">
          {query.trim() ? 'No matching connections' : 'No connections saved'}
        </li>
      {/each}
    </ul>
  </div>
</div>

{#if connCtx}
  {@const profile = connCtx.profile}
  {@const connConnected = connections.isActive(profile.id)}
  <ContextMenu x={connCtx.x} y={connCtx.y} open={true} onclose={() => (connCtx = null)}>
    {#if connConnected}
      <CtxItem onclick={() => ctxDisconnect(profile)}>Disconnect</CtxItem>
    {:else}
      <CtxItem onclick={() => ctxConnect(profile)}>Connect</CtxItem>
    {/if}
    <CtxItem onclick={() => ctxNewQueryEditor(profile)}>New Query Editor</CtxItem>
    {#if !profile.unsaved}
      <CtxSep />
      <CtxItem onclick={() => ctxEdit(profile)}>Edit</CtxItem>
      <CtxItem onclick={() => ctxToggleReadOnly(profile)}
        >{profile.readOnly ? 'Disable Read Only' : 'Enable Read Only'}</CtxItem
      >
      <CtxItem onclick={() => ctxToggleSafeMode(profile)}
        >{profile.safeMode ? 'Disable Safe Mode' : 'Enable Safe Mode'}</CtxItem
      >
    {/if}
    <CtxSep />
    <CtxItem onclick={() => ctxCopyName(profile)}>Copy Name</CtxItem>
    {#if !profile.unsaved}
      <CtxItem onclick={() => ctxCopyDbUrl(profile)}>Copy as Database URL</CtxItem>
      <CtxItem onclick={() => ctxExport(profile)}>Export Connection…</CtxItem>
    {/if}
    {#if tagsStore.tags.length > 0 && !profile.unsaved}
      <CtxSep />
      <CtxSubmenuItem label="Tags">
        {#each tagsStore.tags as tag (tag.id)}
          {@const applied = profile.tags.some((t) => t.id === tag.id)}
          <CtxItem onclick={() => ctxToggleTag(profile, tag.id)}>
            {applied ? '✓ ' : ''}{tag.name}
          </CtxItem>
        {/each}
      </CtxSubmenuItem>
    {/if}
    <CtxSep />
    <CtxItem danger onclick={() => ctxDelete(profile)}>Delete</CtxItem>
  </ContextMenu>
{/if}

{#if editingProfile}
  <ConnectionForm
    profile={editingProfile}
    zindex={600}
    onclose={() => (editingProfile = undefined)}
    ondelete={() => {
      if (editingProfile)
        ctxDelete(editingProfile, () => {
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
    zindex={600}
    onconfirm={confirmState.onconfirm}
    oncancel={() => (confirmState = null)}
  />
{/if}

{#if showExportDialog}
  <ExportConnectionsDialog
    profiles={connections.profiles}
    preselectIds={exportProfileId ? [exportProfileId] : null}
    zindex={600}
    onclose={() => {
      showExportDialog = false;
      exportProfileId = null;
    }}
    onsuccess={(count) => {
      showExportDialog = false;
      exportProfileId = null;
      toast.addToast(`Exported ${count} connection${count !== 1 ? 's' : ''}`, 'success');
    }}
    onerror={(msg) => {
      showExportDialog = false;
      exportProfileId = null;
      toast.addToast(`Export failed: ${msg}`, 'error', 0);
    }}
  />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    -webkit-backdrop-filter: blur(4px);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 15vh var(--spacing-4) 5vh;
    z-index: 500;
  }

  .palette {
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-overlay);
    width: 560px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: min(480px, 80vh);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .search-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-3) var(--spacing-4) var(--spacing-3) 26px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .search-action-btn {
    flex-shrink: 0;
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    padding: 2px var(--spacing-2);
    cursor: pointer;
    line-height: 1.6;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .search-action-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
    border-color: var(--color-border-strong);
  }

  .search-action-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .search-action-btn--icon {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 3px 5px;
  }

  .search-action-btn--icon.active {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
    border-color: color-mix(in srgb, var(--color-accent) 35%, transparent);
  }

  .display-settings-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: var(--spacing-2);
    z-index: 10;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
    padding: var(--spacing-2);
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    min-width: 170px;
  }

  .display-settings-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-2);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .display-settings-item:hover {
    background: var(--color-bg-hover);
  }

  .search-icon {
    color: var(--color-text-muted);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: var(--font-size-md);
    color: var(--color-text-primary);
    outline: none;
    font-family: var(--font-family-ui);
    padding: 0 4px;
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  :global(.spin) {
    animation: spin 0.8s linear infinite;
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .result-item.selected,
  .result-item:hover {
    background: var(--color-bg-hover);
  }

  .result-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--color-accent);
    margin: 2px;
  }

  .result-dot--dim {
    opacity: 0.35;
  }

  .item-main {
    display: flex;
    align-items: center;
    gap: 5px;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .item-label {
    flex-shrink: 1;
    min-width: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .item-flag {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    color: var(--color-text-muted);
  }

  .item-folder {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    padding: 1px 6px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  .item-tags {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 3px;
    overflow: hidden;
  }

  .item-tag-chip {
    flex-shrink: 0;
    font-size: var(--font-size-xs);
    padding: 1px 6px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 90px;
  }

  .item-status {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  button.item-status--connected {
    position: relative;
    width: 12px;
    height: 12px;
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
  }

  button.item-status--connected:hover {
    color: var(--color-danger);
  }

  button.item-status--connected:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .status-icon {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .status-icon--close {
    display: none;
  }

  button.item-status--connected:hover .status-icon--check {
    display: none;
  }

  button.item-status--connected:hover .status-icon--close {
    display: flex;
  }

  .item-subtitle {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
    flex-shrink: 0;
  }

  .item-trailing {
    display: flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    overflow: hidden;
    max-width: 240px;
  }

  .empty-item {
    padding: var(--spacing-4);
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
  }
</style>
