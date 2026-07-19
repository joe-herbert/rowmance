<!--
  PolymorphicVirtualRelationModal — form to create a polymorphic virtual relation.
  A polymorphic relation uses two columns: a type column (discriminator) and a
  value column (FK value), with mappings from type values to target tables.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import * as queryApi from '$lib/tauri/query';
  import ComboInput from '$lib/components/ui/ComboInput.svelte';
  import type { ColumnRef, PolymorphicVirtualRelation, TableInfo } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { qi, tableRef, defaultDialectInfo } from '$lib/utils/dialect';
  import PencilIcon from '$lib/components/icons/PencilIcon.svelte';
  import TrashIcon from '$lib/components/icons/TrashIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';

  interface MappingRow {
    typeValue: string;
    toConnectionId: string;
    toDatabase: string;
    toTable: string;
    toColumn: string;
    databases: string[];
    tables: TableInfo[];
    columns: string[];
    dbLoading: boolean;
    tableLoading: boolean;
    colLoading: boolean;
    collapsed: boolean;
  }

  function isComplete(m: MappingRow): boolean {
    return !!(m.typeValue && m.toConnectionId && m.toDatabase && m.toTable && m.toColumn);
  }

  interface Props {
    connectionId: string;
    database: string;
    table: string;
    /** Column that was right-clicked — pre-selected as value column. */
    initialColumn?: string;
    onClose: () => void;
    onCreated?: () => void;
    editRelation?: PolymorphicVirtualRelation;
  }

  const { connectionId, database, table, initialColumn, onClose, onCreated, editRelation }: Props =
    $props();

  const connectionStore = useConnections();
  const vrStore = useVirtualRelations();

  let tableColumns = $state<string[]>([]);
  let tableColumnsLoading = $state(false);

  let typeColumn = $state(untrack(() => editRelation?.typeColumn ?? ''));
  let valueColumn = $state(untrack(() => editRelation?.valueColumn ?? initialColumn ?? ''));
  let label = $state(untrack(() => editRelation?.label ?? ''));

  /** Distinct values from the type column, used as combobox suggestions. */
  let typeColumnValues = $state<string[]>([]);

  async function loadTypeColumnValues(col: string) {
    if (!col) {
      typeColumnValues = [];
      return;
    }
    try {
      const d = connectionStore.getById(connectionId)?.dialectInfo ?? defaultDialectInfo;
      const quotedCol = qi(col, d);
      const tblRef = tableRef(database, table, d);
      const result = await queryApi.executeSelection(
        connectionId,
        `SELECT DISTINCT ${quotedCol} FROM ${tblRef} WHERE ${quotedCol} IS NOT NULL ORDER BY ${quotedCol} LIMIT 100`,
        database,
      );
      if (!result.error) {
        typeColumnValues = result.rows.map((r) => String(r[0])).filter(Boolean);
      }
    } catch {
      // Non-fatal — user can still type freely
    }
  }

  $effect(() => {
    loadTypeColumnValues(typeColumn);
  });

  let mappings = $state<MappingRow[]>(
    untrack(
      () =>
        editRelation?.mappings.map((m) => ({
          typeValue: m.typeValue,
          toConnectionId: m.to.connectionId,
          toDatabase: m.to.database,
          toTable: m.to.table,
          toColumn: m.to.column,
          databases: [],
          tables: [],
          columns: [],
          dbLoading: false,
          tableLoading: false,
          colLoading: false,
          collapsed: true,
        })) ?? [],
    ),
  );

  let error = $state<string | null>(null);

  const activeConnections = $derived(
    connectionStore.profiles.filter((p) => connectionStore.isActive(p.id)),
  );

  const connectionOptions = $derived([
    { value: '', label: 'Select connection…' },
    ...activeConnections.map((c) => ({ value: c.id, label: c.name })),
  ]);

  const columnOptions = $derived([
    { value: '', label: tableColumnsLoading ? 'Loading…' : 'Select column…' },
    ...tableColumns.map((c) => ({ value: c, label: c })),
  ]);

  function connName(id: string): string {
    return connectionStore.profiles.find((p) => p.id === id)?.name ?? id;
  }

  // Load source table columns on mount
  $effect(() => {
    tableColumnsLoading = true;
    schemaApi
      .listColumns(connectionId, database, table)
      .then((cols) => {
        tableColumns = cols.map((c) => c.name);
      })
      .catch((err) => {
        error = errorMessage(err);
      })
      .finally(() => {
        tableColumnsLoading = false;
      });
  });

  // Pre-load cascading data for existing mappings when editing
  $effect(() => {
    if (!editRelation) return;
    editRelation.mappings.forEach((m, idx) => {
      schemaApi
        .listDatabases(m.to.connectionId)
        .then((dbs) => {
          mappings[idx].databases = dbs;
          return schemaApi.listTables(m.to.connectionId, m.to.database);
        })
        .then((tbls) => {
          mappings[idx].tables = tbls;
          return schemaApi.listColumns(m.to.connectionId, m.to.database, m.to.table);
        })
        .then((cols) => {
          mappings[idx].columns = cols.map((c) => c.name);
        })
        .catch((err) => {
          error = errorMessage(err);
        });
    });
  });

  async function addMapping() {
    const idx = mappings.length;
    mappings = [
      ...mappings,
      {
        typeValue: '',
        toConnectionId: connectionId,
        toDatabase: database,
        toTable: '',
        toColumn: '',
        databases: [],
        tables: [],
        columns: [],
        dbLoading: true,
        tableLoading: false,
        colLoading: false,
        collapsed: false,
      },
    ];
    try {
      const [dbs, tbls] = await Promise.all([
        schemaApi.listDatabases(connectionId),
        schemaApi.listTables(connectionId, database),
      ]);
      mappings[idx].databases = dbs;
      mappings[idx].tables = tbls;
    } catch (err) {
      error = errorMessage(err);
    } finally {
      mappings[idx].dbLoading = false;
    }
  }

  const mappedTypeValues = $derived(new Set(mappings.map((m) => m.typeValue).filter(Boolean)));

  const unmappedTypeValues = $derived(typeColumnValues.filter((v) => !mappedTypeValues.has(v)));

  async function addAllMappings() {
    // Snapshot before mutating — unmappedTypeValues is $derived and will
    // recalculate to zero once the rows are appended with their type values set.
    const values = [...unmappedTypeValues];
    if (values.length === 0) return;
    const startIdx = mappings.length;

    mappings = [
      ...mappings,
      ...values.map((v) => ({
        typeValue: v,
        toConnectionId: connectionId,
        toDatabase: database,
        toTable: '',
        toColumn: '',
        databases: [] as string[],
        tables: [] as TableInfo[],
        columns: [] as string[],
        dbLoading: true,
        tableLoading: false,
        colLoading: false,
        collapsed: false,
      })),
    ];

    try {
      const [dbs, tbls] = await Promise.all([
        schemaApi.listDatabases(connectionId),
        schemaApi.listTables(connectionId, database),
      ]);
      for (let i = 0; i < values.length; i++) {
        mappings[startIdx + i].databases = dbs;
        mappings[startIdx + i].tables = tbls;
      }
    } catch (err) {
      error = errorMessage(err);
    } finally {
      for (let i = 0; i < values.length; i++) {
        mappings[startIdx + i].dbLoading = false;
      }
    }
  }

  function removeMapping(idx: number) {
    mappings = mappings.filter((_, i) => i !== idx);
  }

  function expandMapping(idx: number) {
    mappings[idx].collapsed = false;
  }

  async function onMappingConnectionChange(idx: number, val: string) {
    mappings[idx].toConnectionId = val;
    mappings[idx].toDatabase = '';
    mappings[idx].toTable = '';
    mappings[idx].toColumn = '';
    mappings[idx].databases = [];
    mappings[idx].tables = [];
    mappings[idx].columns = [];
    if (!val) return;
    mappings[idx].dbLoading = true;
    try {
      const dbs = await schemaApi.listDatabases(val);
      mappings[idx].databases = dbs;
      if (dbs.length === 1) {
        mappings[idx].toDatabase = dbs[0];
        await onMappingDatabaseChange(idx, dbs[0]);
      }
    } catch (err) {
      error = errorMessage(err);
    } finally {
      mappings[idx].dbLoading = false;
    }
  }

  async function onMappingDatabaseChange(idx: number, val: string) {
    mappings[idx].toDatabase = val;
    mappings[idx].toTable = '';
    mappings[idx].toColumn = '';
    mappings[idx].tables = [];
    mappings[idx].columns = [];
    if (!mappings[idx].toConnectionId || !val) return;
    mappings[idx].tableLoading = true;
    try {
      mappings[idx].tables = await schemaApi.listTables(mappings[idx].toConnectionId, val);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      mappings[idx].tableLoading = false;
    }
  }

  async function onMappingTableChange(idx: number, val: string) {
    mappings[idx].toTable = val;
    mappings[idx].toColumn = '';
    mappings[idx].columns = [];
    if (!mappings[idx].toConnectionId || !mappings[idx].toDatabase || !val) return;
    mappings[idx].colLoading = true;
    try {
      const cols = await schemaApi.listColumns(
        mappings[idx].toConnectionId,
        mappings[idx].toDatabase,
        val,
      );
      mappings[idx].columns = cols.map((c) => c.name);
      const pk = cols.find((c) => c.isPrimaryKey);
      const autoCol = pk ?? cols[0];
      if (autoCol) onMappingColumnChange(idx, autoCol.name);
    } catch (err) {
      error = errorMessage(err);
    } finally {
      mappings[idx].colLoading = false;
    }
  }

  function onMappingColumnChange(idx: number, val: string) {
    mappings[idx].toColumn = val;
  }

  async function handleSave() {
    if (!typeColumn || !valueColumn || mappings.length === 0) return;
    const validMappings = mappings.filter(isComplete);
    if (validMappings.length === 0) return;

    const input = {
      label: label.trim() || undefined,
      connectionId,
      database,
      table,
      typeColumn,
      valueColumn,
      mappings: validMappings.map((m) => ({
        typeValue: m.typeValue,
        to: {
          connectionId: m.toConnectionId,
          database: m.toDatabase,
          table: m.toTable,
          column: m.toColumn,
        } satisfies ColumnRef,
      })),
    };

    try {
      if (editRelation) {
        await vrStore.updatePolymorphic(editRelation.id, input);
      } else {
        await vrStore.addPolymorphic(input);
      }
      onCreated?.();
      onClose();
    } catch (err) {
      error = errorMessage(err);
    }
  }

  /** Type values already committed by other mappings. */
  function otherTypeValues(idx: number): Set<string> {
    const s = new Set<string>();
    mappings.forEach((m, i) => {
      if (i !== idx && m.typeValue) s.add(m.typeValue);
    });
    return s;
  }

  /** Suggestions for a mapping: column values minus those used by other mappings. */
  function suggestionsFor(idx: number): string[] {
    const others = otherTypeValues(idx);
    return typeColumnValues.filter((v) => !others.has(v));
  }

  function isDuplicate(idx: number): boolean {
    const v = mappings[idx].typeValue;
    return !!v && mappings.some((m, i) => i !== idx && m.typeValue === v);
  }

  const hasDuplicates = $derived(mappings.some((_, i) => isDuplicate(i)));

  const duplicateRelationExists = $derived(
    !!typeColumn &&
      !!valueColumn &&
      typeColumn !== valueColumn &&
      vrStore.polymorphicRelations.some(
        (pvr) =>
          pvr.id !== editRelation?.id &&
          pvr.connectionId === connectionId &&
          pvr.database === database &&
          pvr.table === table &&
          pvr.typeColumn === typeColumn &&
          pvr.valueColumn === valueColumn,
      ),
  );

  const canSave = $derived(
    !!typeColumn &&
      !!valueColumn &&
      typeColumn !== valueColumn &&
      !hasDuplicates &&
      !duplicateRelationExists &&
      mappings.some(isComplete),
  );

  const modalTitle = $derived(editRelation ? 'Edit Polymorphic Relation' : 'Connect Polymorphic');
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
        <div class="section-label">Source table</div>
        <div class="ref-chip">
          <span class="chip-db">{database}</span>
          <span class="chip-sep">.</span>
          <span class="chip-table">{table}</span>
        </div>
      </div>

      <div class="section">
        <div class="section-label">Columns</div>
        <div class="two-col">
          <div class="field">
            <label class="field-label" for="pvr-type-col">Type column</label>
            <Select
              id="pvr-type-col"
              bind:value={typeColumn}
              options={columnOptions}
              onchange={(v) => {
                typeColumn = v;
              }}
              size="md"
              searchable
            />
          </div>
          <div class="field">
            <label class="field-label" for="pvr-value-col">Value column</label>
            <Select
              id="pvr-value-col"
              bind:value={valueColumn}
              options={columnOptions}
              onchange={(v) => {
                valueColumn = v;
              }}
              size="md"
              searchable
            />
          </div>
        </div>
        {#if typeColumn && valueColumn && typeColumn === valueColumn}
          <div class="validation-msg">Type column and value column must be different.</div>
        {:else if duplicateRelationExists}
          <div class="validation-msg">
            A polymorphic relation with this type and value column already exists on this table.
          </div>
        {/if}
      </div>

      <div class="section">
        <div class="section-label">Type mappings</div>
        {#if mappings.length === 0}
          <div class="mappings-hint">
            Map each value of the type column to a target connection, database, table, and column.
          </div>
        {/if}

        <div class="mappings-list">
          {#each mappings as mapping, idx}
            {#if mapping.collapsed && isComplete(mapping)}
              <!-- Compact card -->
              <div class="mapping-compact">
                <div class="compact-top">
                  <span class="compact-type-value">{mapping.typeValue}</span>
                </div>
                <div class="compact-bottom">
                  <span class="compact-arrow">→</span>
                  <div class="compact-target">
                    <span class="compact-path">
                      {#if mapping.toConnectionId !== connectionId}<span class="compact-conn-prefix"
                          >{connName(mapping.toConnectionId)} /
                        </span>{/if}{mapping.toDatabase}.{mapping.toTable}.{mapping.toColumn}
                    </span>
                  </div>
                </div>
                <div class="compact-actions">
                  <button class="compact-btn" onclick={() => expandMapping(idx)}>
                    <PencilIcon width={11} height={11} />
                    Edit
                  </button>
                  <button
                    class="compact-btn compact-btn--danger"
                    onclick={() => removeMapping(idx)}
                  >
                    <TrashIcon width={11} height={11} />
                    Remove
                  </button>
                </div>
              </div>
            {:else}
              <!-- Expanded form -->
              <div class="mapping-expanded">
                <div class="mapping-header">
                  <span class="mapping-index">Mapping {idx + 1}</span>
                  <div class="mapping-header-actions">
                    <button
                      class="icon-btn icon-btn--danger"
                      onclick={() => removeMapping(idx)}
                      aria-label="Remove mapping"
                      title="Remove"
                    >
                      <CloseIcon width={12} height={12} />
                    </button>
                  </div>
                </div>

                <div class="field">
                  <label class="field-label" for="pvr-type-val-{idx}">When type =</label>
                  <ComboInput
                    id="pvr-type-val-{idx}"
                    bind:value={mapping.typeValue}
                    suggestions={suggestionsFor(idx)}
                    placeholder={typeColumnValues.length > 0
                      ? 'Choose or type a value…'
                      : 'e.g. Post, Article…'}
                    onchange={(v) => {
                      mapping.typeValue = v;
                    }}
                  />
                  {#if isDuplicate(idx)}
                    <span class="field-error"
                      >This type value is already used in another mapping.</span
                    >
                  {/if}
                </div>

                <div class="arrow-row">↓ navigate to</div>

                <div class="field">
                  <label class="field-label" for="pvr-conn-{idx}">Connection</label>
                  <Select
                    id="pvr-conn-{idx}"
                    bind:value={mapping.toConnectionId}
                    options={connectionOptions}
                    onchange={(v) => onMappingConnectionChange(idx, v)}
                    size="md"
                    searchable
                  />
                </div>

                <div class="field">
                  <label class="field-label" for="pvr-db-{idx}">Database</label>
                  <Select
                    id="pvr-db-{idx}"
                    bind:value={mapping.toDatabase}
                    options={[
                      { value: '', label: mapping.dbLoading ? 'Loading…' : 'Select database…' },
                      ...mapping.databases.map((db) => ({ value: db, label: db })),
                    ]}
                    onchange={(v) => onMappingDatabaseChange(idx, v)}
                    disabled={!mapping.toConnectionId || mapping.dbLoading}
                    size="md"
                    searchable
                  />
                </div>

                <div class="field">
                  <label class="field-label" for="pvr-table-{idx}">Table</label>
                  <Select
                    id="pvr-table-{idx}"
                    bind:value={mapping.toTable}
                    options={[
                      { value: '', label: mapping.tableLoading ? 'Loading…' : 'Select table…' },
                      ...mapping.tables.map((t) => ({ value: t.name, label: t.name })),
                    ]}
                    onchange={(v) => onMappingTableChange(idx, v)}
                    disabled={!mapping.toDatabase || mapping.tableLoading}
                    size="md"
                    searchable
                  />
                </div>

                <div class="field">
                  <label class="field-label" for="pvr-col-{idx}">Match column</label>
                  <Select
                    id="pvr-col-{idx}"
                    bind:value={mapping.toColumn}
                    options={[
                      { value: '', label: mapping.colLoading ? 'Loading…' : 'Select column…' },
                      ...mapping.columns.map((c) => ({ value: c, label: c })),
                    ]}
                    onchange={(v) => onMappingColumnChange(idx, v)}
                    disabled={!mapping.toTable || mapping.colLoading}
                    size="md"
                    searchable
                  />
                </div>

                {#if isComplete(mapping)}
                  <button
                    class="confirm-btn"
                    onclick={() => {
                      mapping.collapsed = true;
                    }}
                  >
                    Confirm
                  </button>
                {/if}
              </div>
            {/if}
          {/each}

          <button class="add-mapping-btn" onclick={addMapping}>+ Add mapping</button>
          {#if typeColumn && unmappedTypeValues.length > 0}
            <button class="add-mapping-btn" onclick={addAllMappings}>
              + Add all ({unmappedTypeValues.length})
            </button>
          {/if}
        </div>
      </div>

      <div class="section">
        <div class="field">
          <label class="field-label" for="pvr-label">Label (optional)</label>
          <input
            id="pvr-label"
            class="field-input"
            type="text"
            placeholder="e.g. commentable polymorphic"
            bind:value={label}
          />
        </div>
      </div>
    </div>

    <div class="modal-footer">
      <button class="btn btn--ghost" onclick={onClose}>Cancel</button>
      <button class="btn btn--primary" onclick={handleSave} disabled={!canSave}>
        {editRelation ? 'Save' : 'Connect'}
      </button>
    </div>
  </div>
</Modal>

<style>
  .modal-box {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 900px;
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
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
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

  .chip-sep {
    color: var(--color-text-muted);
  }
  .chip-db {
    color: var(--color-text-secondary);
  }
  .chip-table {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-3);
  }

  .validation-msg {
    font-size: var(--font-size-sm);
    color: var(--color-danger);
  }

  .mappings-hint {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  /* ── Mappings flex container ────────────────────────────────────────────────── */

  .mappings-list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-2);
    align-items: flex-start;
  }

  /* ── Compact card ───────────────────────────────────────────────────────────── */

  .mapping-compact {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    flex: 1 1 200px;
    min-width: 180px;
    overflow: hidden;
  }

  .compact-top {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .compact-type-value {
    display: inline-block;
    padding: 1px 6px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .compact-bottom {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-1);
    min-width: 0;
  }

  .compact-arrow {
    flex-shrink: 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .compact-target {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .compact-path {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compact-conn-prefix {
    color: var(--color-text-muted);
  }

  .compact-actions {
    display: flex;
    gap: var(--spacing-1);
    padding-top: var(--spacing-1);
  }

  .compact-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: 1;
    justify-content: center;
    height: 26px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition:
      color var(--transition-fast),
      background var(--transition-fast),
      border-color var(--transition-fast);
    white-space: nowrap;
  }

  .compact-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
    border-color: var(--color-border-strong);
  }

  .compact-btn--danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
    border-color: var(--color-danger);
  }

  /* ── Icon buttons (edit / collapse / remove) ─────────────────────────────── */

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
    flex-shrink: 0;
  }

  .icon-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .icon-btn--danger:hover {
    color: var(--color-danger);
    background: var(--color-danger-subtle);
  }

  /* ── Expanded form card ─────────────────────────────────────────────────────── */

  .mapping-expanded {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    padding: var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    flex: 1 1 260px;
    min-width: 260px;
  }

  .mapping-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .mapping-index {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
  }

  .mapping-header-actions {
    display: flex;
    gap: var(--spacing-1);
  }

  .arrow-row {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    text-align: center;
  }

  /* ── Confirm button ─────────────────────────────────────────────────────────── */

  .confirm-btn {
    align-self: flex-end;
    height: 28px;
    padding: 0 var(--spacing-3);
    border: 1px solid var(--color-accent);
    border-radius: var(--radius-md);
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .confirm-btn:hover {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }

  /* ── Add mapping button ─────────────────────────────────────────────────────── */

  .add-mapping-btn {
    flex: 0 0 auto;
    align-self: stretch;
    display: flex;
    align-items: center;
    background: none;
    border: 1px dashed var(--color-border);
    border-radius: var(--radius-md);
    color: var(--color-text-secondary);
    cursor: pointer;
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    padding: var(--spacing-2) var(--spacing-3);
    white-space: nowrap;
    transition:
      color var(--transition-fast),
      border-color var(--transition-fast),
      background var(--transition-fast);
  }

  .add-mapping-btn:hover {
    color: var(--color-text-primary);
    border-color: var(--color-accent);
    background: var(--color-bg-hover);
  }

  /* ── Form fields ────────────────────────────────────────────────────────────── */

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

  .field-error {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
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
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .field-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  /* ── Footer ─────────────────────────────────────────────────────────────────── */

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
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover {
    background: var(--color-accent-hover);
  }

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
