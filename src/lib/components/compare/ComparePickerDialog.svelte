<!--
  ComparePickerDialog — asks the user to pick the "other side" of a schema
  compare: a connection, a database, and (in table mode) a table. Modeled
  directly on ConnectionPickerDialog's cascading Select pattern.

  Note: SQL Server's instanceDb (instance-level database -> schema hierarchy)
  is not threaded through here in v1 — left unset. Follow-up if needed.
-->
<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as schemaApi from '$lib/tauri/schema';

  interface Props {
    mode: 'table' | 'database';
    title?: string;
    onconfirm: (result: {
      connectionId: string;
      database: string;
      instanceDb?: string;
      table?: string;
    }) => void;
    oncancel: () => void;
  }

  const { mode, title, onconfirm, oncancel }: Props = $props();

  const connections = useConnections();

  let connectionId = $state('');
  let database = $state('');
  let table = $state('');
  let databases = $state<string[]>([]);
  let tables = $state<string[]>([]);
  let dbLoading = $state(false);
  let tableLoading = $state(false);

  const resolvedTitle = $derived(title ?? 'Compare with...');

  const connectionOptions = $derived([
    { value: '', label: 'Select connection…' },
    ...connections.profiles
      .filter((p) => connections.isActive(p.id))
      .map((p) => ({ value: p.id, label: p.name })),
  ]);

  const databaseOptions = $derived([
    { value: '', label: dbLoading ? 'Loading…' : 'Select database…' },
    ...databases.map((db) => ({ value: db, label: db })),
  ]);

  const tableOptions = $derived([
    { value: '', label: tableLoading ? 'Loading…' : 'Select table…' },
    ...tables.map((t) => ({ value: t, label: t })),
  ]);

  async function onConnectionChange(id: string) {
    connectionId = id;
    database = '';
    table = '';
    databases = [];
    tables = [];
    if (!id) return;
    dbLoading = true;
    try {
      databases = await schemaApi.listDatabases(id);
      if (databases.length === 1) {
        database = databases[0];
        await onDatabaseChange(database);
      }
    } catch {
      databases = [];
    } finally {
      dbLoading = false;
    }
  }

  async function onDatabaseChange(db: string) {
    database = db;
    table = '';
    tables = [];
    if (mode !== 'table' || !db || !connectionId) return;
    tableLoading = true;
    try {
      const infos = await schemaApi.listTables(connectionId, db);
      tables = infos.map((t) => t.name);
    } catch {
      tables = [];
    } finally {
      tableLoading = false;
    }
  }

  const canConfirm = $derived(
    !!connectionId && !!database && (mode === 'database' || !!table),
  );

  function confirm() {
    if (!canConfirm) return;
    onconfirm({
      connectionId,
      database,
      table: mode === 'table' ? table : undefined,
    });
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Modal label={resolvedTitle} onbackdropclick={oncancel}>
  <div class="modal-card">
    <div class="modal-title">{resolvedTitle}</div>
    <div class="modal-body">
      <p class="modal-message">
        {mode === 'table'
          ? 'Select the connection, database, and table to compare against.'
          : 'Select the connection and database to compare against.'}
      </p>
      <div class="picker-row">
        <Select
          aria-label="Connection"
          bind:value={connectionId}
          options={connectionOptions}
          onchange={onConnectionChange}
          searchable
        />
        <Select
          aria-label="Database"
          bind:value={database}
          options={databaseOptions}
          onchange={onDatabaseChange}
          disabled={!connectionId || dbLoading}
          searchable
        />
        {#if mode === 'table'}
          <Select
            aria-label="Table"
            bind:value={table}
            options={tableOptions}
            disabled={!database || tableLoading}
            searchable
          />
        {/if}
      </div>
    </div>
    <div class="modal-footer">
      <button class="btn" onclick={oncancel}>Cancel</button>
      <button class="btn btn--primary" onclick={confirm} disabled={!canConfirm}>Compare</button>
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 380px;
    max-width: 92vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-title {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-body {
    padding: var(--spacing-4);
  }

  .modal-message {
    margin: 0 0 var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: var(--line-height-normal);
  }

  .picker-row {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

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

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }
</style>
