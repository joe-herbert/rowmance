<!--
  ExportConnectionsDialog — select which connections to export and whether to
  include sensitive credentials. The file save dialog is opened here.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import Modal from '$lib/components/Modal.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import type { ConnectionProfile } from '$lib/types';
  import * as connectionsApi from '$lib/tauri/connections';
  import { errorMessage } from '$lib/utils/errors';
  import WarningIcon from '$lib/components/icons/WarningIcon.svelte';

  interface Props {
    profiles: ConnectionProfile[];
    /** When set, only these connections are pre-selected. Null = select all. */
    preselectIds?: string[] | null;
    onclose: () => void;
    onsuccess: (_count: number) => void;
    onerror: (_msg: string) => void;
  }

  let { profiles, preselectIds = null, onclose, onsuccess, onerror }: Props = $props();

  const initialIds = untrack(() =>
    preselectIds ? new Set(preselectIds) : new Set(profiles.map((p) => p.id)),
  );
  let selectedIds = $state(initialIds);
  let includeSensitive = $state(false);
  let exporting = $state(false);

  function toggleAll() {
    if (selectedIds.size === profiles.length) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(profiles.map((p) => p.id));
    }
  }

  function toggleOne(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) {
      next.delete(id);
    } else {
      next.add(id);
    }
    selectedIds = next;
  }

  async function handleExport() {
    if (selectedIds.size === 0) return;

    const filePath = await saveDialog({
      defaultPath: 'connections.json',
      filters: [{ name: 'Connection file', extensions: ['json'] }],
    });
    if (!filePath) return;

    exporting = true;
    try {
      await connectionsApi.exportConnections([...selectedIds], filePath, includeSensitive);
      onsuccess(selectedIds.size);
      onclose();
    } catch (err) {
      onerror(errorMessage(err));
    } finally {
      exporting = false;
    }
  }
</script>

<Modal label="Export Connections" onbackdropclick={exporting ? undefined : onclose}>
  <div class="export-dialog">
    <p class="description">
      Select which connections to export. The file can be imported on another machine.
    </p>

    <!-- Connection list -->
    <div class="conn-list">
      <div class="list-header">
        <button class="toggle-all" onclick={toggleAll}>
          {selectedIds.size === profiles.length ? 'Deselect all' : 'Select all'}
        </button>
        <span class="count">{selectedIds.size} / {profiles.length} selected</span>
      </div>
      <div class="list-items">
        {#each profiles as profile (profile.id)}
          <label class="conn-row">
            <Checkbox
              checked={selectedIds.has(profile.id)}
              onchange={() => toggleOne(profile.id)}
            />
            <span
              class="color-dot"
              style="background:{profile.color ?? 'var(--color-accent)'}"
              aria-hidden="true"
            ></span>
            <span class="conn-name">{profile.name}</span>
            <span class="conn-meta">{profile.host}</span>
          </label>
        {/each}
      </div>
    </div>

    <!-- Sensitive data toggle -->
    <div class="sensitive-section">
      <label class="sensitive-row">
        <Checkbox
          checked={includeSensitive}
          onchange={() => (includeSensitive = !includeSensitive)}
        />
        <span>Include passwords</span>
      </label>
      {#if includeSensitive}
        <div class="warning">
          <WarningIcon width={14} height={14} />
          Passwords will be stored as plaintext in the exported file. Keep the file secure and delete
          it when no longer needed.
        </div>
      {/if}
    </div>

    <!-- Actions -->
    <div class="actions">
      <button class="btn-secondary" onclick={onclose} disabled={exporting}>Cancel</button>
      <button
        class="btn-primary"
        onclick={handleExport}
        disabled={selectedIds.size === 0 || exporting}
      >
        {exporting
          ? 'Exporting…'
          : `Export ${selectedIds.size > 0 ? selectedIds.size : ''} Connection${selectedIds.size !== 1 ? 's' : ''}…`}
      </button>
    </div>
  </div>
</Modal>

<style>
  .export-dialog {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 400px;
    max-width: 520px;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    padding: 20px;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .description {
    margin: 0;
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .conn-list {
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .list-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    background: var(--color-surface-raised);
    border-bottom: 1px solid var(--color-border);
    font-size: 12px;
  }

  .toggle-all {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    color: var(--color-accent);
    font-size: 12px;
  }

  .toggle-all:hover {
    text-decoration: underline;
  }

  .count {
    color: var(--color-text-muted);
  }

  .list-items {
    max-height: 220px;
    overflow-y: auto;
  }

  .conn-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    cursor: pointer;
    font-size: 13px;
    border-bottom: 1px solid var(--color-border);
  }

  .conn-row:last-child {
    border-bottom: none;
  }

  .conn-row:hover {
    background: var(--color-surface-hover);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .conn-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .conn-meta {
    color: var(--color-text-muted);
    font-size: 11px;
    flex-shrink: 0;
  }

  .sensitive-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sensitive-row {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    font-size: 13px;
  }

  .warning {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 8px 10px;
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 40%, transparent);
    border-radius: var(--radius-md);
    font-size: 12px;
    color: var(--color-text);
    line-height: 1.5;
  }

  .warning :global(svg) {
    flex-shrink: 0;
    margin-top: 1px;
    color: var(--color-warning, #f59e0b);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .btn-primary,
  .btn-secondary {
    padding: 6px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .btn-primary {
    background: var(--color-accent);
    color: var(--color-accent-fg, #fff);
  }

  .btn-primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-secondary {
    background: var(--color-surface-raised);
    border-color: var(--color-border);
    color: var(--color-text);
  }

  .btn-secondary:hover:not(:disabled) {
    background: var(--color-surface-hover);
  }
</style>
