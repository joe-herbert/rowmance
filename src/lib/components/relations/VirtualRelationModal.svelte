<!--
  VirtualRelationModal — form to create a virtual column connection.
  The "from" side is pre-filled from the current cell selection.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import type { ColumnRef, TableInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';

  interface Props {
    from: ColumnRef;
    onClose: () => void;
    onCreated?: () => void;
    editId?: string;
    initialTo?: ColumnRef;
    initialLabel?: string;
  }

  const { from, onClose, onCreated, editId, initialTo, initialLabel }: Props = $props();

  const connectionStore = useConnections();
  const vrStore = useVirtualRelations();

  let toConnectionId = $state(untrack(() => initialTo?.connectionId ?? ''));
  let toDatabase = $state(untrack(() => initialTo?.database ?? ''));
  let toTable = $state(untrack(() => initialTo?.table ?? ''));
  let toColumn = $state(untrack(() => initialTo?.column ?? ''));
  let label = $state(untrack(() => initialLabel ?? ''));

  let databases = $state<string[]>([]);
  let tables = $state<TableInfo[]>([]);
  let columns = $state<string[]>([]);

  let dbLoading = $state(false);
  let tableLoading = $state(false);
  let colLoading = $state(false);
  let error = $state<string | null>(null);

  const activeConnections = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id))
  );

  const connectionOptions = $derived([
    { value: '', label: 'Select connection…' },
    ...activeConnections.map((c) => ({ value: c.id, label: c.name })),
  ]);

  const databaseOptions = $derived([
    { value: '', label: dbLoading ? 'Loading…' : 'Select database…' },
    ...databases.map((db) => ({ value: db, label: db })),
  ]);

  const tableOptions = $derived([
    { value: '', label: tableLoading ? 'Loading…' : 'Select table…' },
    ...tables.map((t) => ({ value: t.name, label: t.name })),
  ]);

  const columnOptions = $derived([
    { value: '', label: colLoading ? 'Loading…' : 'Select column…' },
    ...columns.map((c) => ({ value: c, label: c })),
  ]);

  // Pre-load cascading data when editing an existing relation
  $effect(() => {
    if (!initialTo) return;
    schemaApi.listDatabases(initialTo.connectionId).then((dbs) => {
      databases = dbs;
      return schemaApi.listTables(initialTo.connectionId, initialTo.database);
    }).then((tbls) => {
      tables = tbls;
      return schemaApi.listColumns(initialTo.connectionId, initialTo.database, initialTo.table);
    }).then((cols) => {
      columns = cols.map((c) => c.name);
    }).catch((err) => {
      error = errorMessage(err);
    });
  });

  async function onConnectionChange(val: string) {
    toConnectionId = val;
    toDatabase = '';
    toTable = '';
    toColumn = '';
    databases = [];
    tables = [];
    columns = [];
    if (!val) return;
    dbLoading = true;
    try {
      databases = await schemaApi.listDatabases(val);
      if (databases.length === 1) {
        toDatabase = databases[0];
        await onDatabaseChange(toDatabase);
      }
    } catch (err) {
      error = errorMessage(err);
    } finally {
      dbLoading = false;
    }
  }

  async function onDatabaseChange(val: string) {
    toDatabase = val;
    toTable = '';
    toColumn = '';
    tables = [];
    columns = [];
    if (!toConnectionId || !val) return;
    tableLoading = true;
    try {
      tables = await schemaApi.listTables(toConnectionId, val);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      tableLoading = false;
    }
  }

  async function onTableChange(val: string) {
    toTable = val;
    toColumn = '';
    columns = [];
    if (!toConnectionId || !toDatabase || !val) return;
    colLoading = true;
    try {
      const cols = await schemaApi.listColumns(toConnectionId, toDatabase, val);
      columns = cols.map((c) => c.name);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      colLoading = false;
    }
  }

  function handleSave() {
    if (!toConnectionId || !toDatabase || !toTable || !toColumn) return;
    const toRef: ColumnRef = { connectionId: toConnectionId, database: toDatabase, table: toTable, column: toColumn };
    if (editId) vrStore.remove(editId);
    vrStore.add({ from, to: toRef, label: label.trim() || undefined });
    onCreated?.();
    onClose();
  }

  const canSave = $derived(!!toConnectionId && !!toDatabase && !!toTable && !!toColumn);
  const modalTitle = $derived(editId ? 'Edit Connection' : 'Connect Column');

  const fromConnName = $derived(connectionStore.profiles.find((p) => p.id === from.connectionId)?.name ?? from.connectionId);
</script>

<Modal label={modalTitle} onbackdropclick={onClose}>
  <div class="modal-box">
    <div class="modal-header">
      <h2 class="modal-title">{modalTitle}</h2>
      <button class="close-btn" onclick={onClose} aria-label="Close">×</button>
    </div>

    <div class="modal-body">
      {#if error}
        <div class="error-msg">{error}</div>
      {/if}

      <div class="section">
        <div class="section-label">From (source)</div>
        <div class="ref-chip">
          <span class="chip-conn">{fromConnName}</span>
          <span class="chip-sep">/</span>
          <span class="chip-db">{from.database}</span>
          <span class="chip-sep">.</span>
          <span class="chip-table">{from.table}</span>
          <span class="chip-sep">.</span>
          <span class="chip-col">{from.column}</span>
        </div>
      </div>

      <div class="arrow-row">↓ connects to</div>

      <div class="section">
        <div class="section-label">To (target)</div>

        <div class="field">
          <label class="field-label" for="vr-conn">Connection</label>
          <Select
            id="vr-conn"
            bind:value={toConnectionId}
            options={connectionOptions}
            onchange={onConnectionChange}
            size="md"
          />
        </div>

        <div class="field">
          <label class="field-label" for="vr-db">Database</label>
          <Select
            id="vr-db"
            bind:value={toDatabase}
            options={databaseOptions}
            onchange={onDatabaseChange}
            disabled={!toConnectionId || dbLoading}
            size="md"
          />
        </div>

        <div class="field">
          <label class="field-label" for="vr-table">Table</label>
          <Select
            id="vr-table"
            bind:value={toTable}
            options={tableOptions}
            onchange={onTableChange}
            disabled={!toDatabase || tableLoading}
            size="md"
          />
        </div>

        <div class="field">
          <label class="field-label" for="vr-col">Column</label>
          <Select
            id="vr-col"
            bind:value={toColumn}
            options={columnOptions}
            onchange={(v) => { toColumn = v; }}
            disabled={!toTable || colLoading}
            size="md"
          />
        </div>

        <div class="field">
          <label class="field-label" for="vr-label">Label (optional)</label>
          <input
            id="vr-label"
            class="field-input"
            type="text"
            placeholder="e.g. user → profile"
            bind:value={label}
          />
        </div>
      </div>
    </div>

    <div class="modal-footer">
      <button class="btn btn--ghost" onclick={onClose}>Cancel</button>
      <button class="btn btn--primary" onclick={handleSave} disabled={!canSave}>Connect</button>
    </div>
  </div>
</Modal>

<style>
  .modal-box {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 420px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - 80px);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .modal-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    flex: 1;
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .close-btn {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    line-height: 1;
    cursor: pointer;
    background: none;
    border: none;
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .close-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .modal-body {
    padding: var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
    overflow-y: auto;
  }

  .error-msg {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-danger);
    background: var(--color-danger-subtle);
    border-radius: var(--radius-md);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .section-label {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--color-text-muted);
  }

  .ref-chip {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 1px;
    padding: var(--spacing-2);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
  }

  .chip-conn { color: var(--color-text-muted); }
  .chip-sep { color: var(--color-text-muted); }
  .chip-db { color: var(--color-text-secondary); }
  .chip-table { color: var(--color-text-primary); font-weight: var(--font-weight-medium); }
  .chip-col { color: var(--color-accent); font-weight: var(--font-weight-medium); }

  .arrow-row {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    text-align: center;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .field-label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .field-input {
    height: 32px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-md);
    font-family: var(--font-family-ui);
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px rgba(79, 70, 229, 0.15);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--spacing-2);
    padding: var(--spacing-4) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .btn {
    height: 32px;
    padding: 0 var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast), border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover { background: var(--color-accent-hover); }

  .btn--ghost {
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .btn--ghost:not(:disabled):hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
