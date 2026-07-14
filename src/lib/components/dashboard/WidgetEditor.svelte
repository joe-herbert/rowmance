<!--
  WidgetEditor — modal for adding or editing a dashboard widget.
  Supports SQL mode and a basic visual builder mode.
-->
<script lang="ts">
  import type { DashboardWidget, WidgetDisplayType, SingleValueFormat } from '$lib/types';
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import Select from '$lib/components/ui/Select.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import QueryBuilderModal from '$lib/components/editor/QueryBuilderModal.svelte';
  import type { SchemaTable, SchemaColumn } from '$lib/components/editor/QueryBuilderModal.svelte';

  interface Props {
    widget?: DashboardWidget | null;
    onsave: (w: Omit<DashboardWidget, 'id' | 'x' | 'y'>) => void;
    oncancel: () => void;
  }

  const { widget, onsave, oncancel }: Props = $props();

  const connectionsStore = useConnections();

  // ── Form state ────────────────────────────────────────────────────────────

  let title = $state(widget?.title ?? '');
  let connectionId = $state(
    widget?.connectionId ??
      connectionsStore.profiles.find((p) => connectionsStore.isActive(p.id))?.id ??
      connectionsStore.profiles[0]?.id ??
      '',
  );
  let database = $state(widget?.database ?? '');
  let sql = $state(widget?.sql ?? '');
  let displayType = $state<WidgetDisplayType>(widget?.displayType ?? 'table');
  let singleValueFormat = $state<SingleValueFormat>(widget?.singleValueFormat ?? 'auto');
  let singleValueCurrency = $state(widget?.singleValueCurrency ?? 'GBP');
  let w = $state(widget?.w ?? 6);
  let h = $state(widget?.h ?? 3);

  // ── Query Builder state ───────────────────────────────────────────────────

  let showQueryBuilder = $state(false);
  let qbTables = $state<SchemaTable[]>([]);
  let qbLoading = $state(false);
  let qbError = $state('');
  const columnCache = new Map<string, SchemaColumn[]>();

  async function openBuilder() {
    if (!connectionId || !connectionsStore.isActive(connectionId)) return;
    qbLoading = true;
    qbError = '';
    try {
      const databases = await schemaApi.listDatabases(connectionId);
      const rows: SchemaTable[] = [];
      for (const db of databases) {
        const tables = await schemaApi.listTables(connectionId, db);
        for (const t of tables) rows.push({ database: db, name: t.name });
      }
      qbTables = rows;
      showQueryBuilder = true;
    } catch (e) {
      qbError = String(e);
    } finally {
      qbLoading = false;
    }
  }

  async function loadColumns(db: string, table: string): Promise<SchemaColumn[]> {
    const key = `${db}.${table}`;
    const cached = columnCache.get(key);
    if (cached) return cached;
    const cols = await schemaApi.listColumns(connectionId, db, table);
    const result = cols.map((c) => ({ name: c.name, dataType: c.dataType }));
    columnCache.set(key, result);
    return result;
  }

  function handleSave() {
    onsave({
      title: title || 'Widget',
      connectionId,
      database: database || null,
      sql,
      displayType,
      singleValueFormat: displayType === 'count' ? singleValueFormat : undefined,
      singleValueCurrency: displayType === 'count' && singleValueFormat === 'currency' ? singleValueCurrency : undefined,
      w,
      h,
    });
  }

  const canSave = $derived(connectionId && sql.trim().length > 0);

  const SINGLE_VALUE_FORMATS: { value: SingleValueFormat; label: string }[] = [
    { value: 'auto',           label: 'Auto' },
    { value: 'number',         label: 'Number' },
    { value: 'number_compact', label: 'Number (compact)' },
    { value: 'currency',       label: 'Currency' },
    { value: 'percent',        label: 'Percentage' },
    { value: 'date',           label: 'Date' },
    { value: 'datetime',       label: 'Date & Time' },
    { value: 'boolean',        label: 'Boolean' },
    { value: 'text',           label: 'Text' },
  ];

  const CURRENCIES = [
    { value: 'USD', label: 'USD — US Dollar' },
    { value: 'EUR', label: 'EUR — Euro' },
    { value: 'GBP', label: 'GBP — British Pound' },
    { value: 'JPY', label: 'JPY — Japanese Yen' },
    { value: 'CAD', label: 'CAD — Canadian Dollar' },
    { value: 'AUD', label: 'AUD — Australian Dollar' },
    { value: 'CHF', label: 'CHF — Swiss Franc' },
    { value: 'CNY', label: 'CNY — Chinese Yuan' },
    { value: 'INR', label: 'INR — Indian Rupee' },
    { value: 'BRL', label: 'BRL — Brazilian Real' },
    { value: 'MXN', label: 'MXN — Mexican Peso' },
    { value: 'SGD', label: 'SGD — Singapore Dollar' },
    { value: 'HKD', label: 'HKD — Hong Kong Dollar' },
    { value: 'NOK', label: 'NOK — Norwegian Krone' },
    { value: 'SEK', label: 'SEK — Swedish Krona' },
    { value: 'DKK', label: 'DKK — Danish Krone' },
    { value: 'NZD', label: 'NZD — New Zealand Dollar' },
    { value: 'ZAR', label: 'ZAR — South African Rand' },
    { value: 'KRW', label: 'KRW — South Korean Won' },
  ];

  const DISPLAY_TYPES: { value: WidgetDisplayType; label: string }[] = [
    { value: 'count', label: 'Single Value' },
    { value: 'table', label: 'Table' },
    { value: 'bar_chart', label: 'Bar Chart' },
    { value: 'line_chart', label: 'Line Chart' },
    { value: 'countdown', label: 'Countdown' },
  ];
</script>

<Modal label="Widget Editor" onbackdropclick={oncancel}>
  <div class="dialog">
    <div class="dialog-header">
      <h3 class="dialog-title">{widget ? 'Edit Widget' : 'Add Widget'}</h3>
      <button class="close-btn" onclick={oncancel} aria-label="Close" type="button">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>

    <div class="dialog-body">
      <!-- Title -->
      <div class="field">
        <label class="field-label" for="widget-title">Title</label>
        <input
          id="widget-title"
          class="field-input"
          type="text"
          placeholder="Widget title…"
          bind:value={title}
        />
      </div>

      <!-- Connection -->
      <div class="field">
        <label class="field-label" for="widget-conn">Connection</label>
        <Select
          id="widget-conn"
          bind:value={connectionId}
          options={connectionsStore.profiles.map((p) => ({ value: p.id, label: p.name }))}
          aria-label="Connection"
          size="sm"
        />
      </div>

      <!-- Database -->
      <div class="field">
        <label class="field-label" for="widget-db">Database</label>
        <input
          id="widget-db"
          class="field-input"
          type="text"
          placeholder="database name (optional)"
          bind:value={database}
        />
      </div>

      <!-- SQL -->
      <div class="field">
        <div class="sql-label-row">
          <label class="field-label" for="widget-sql">SQL</label>
          <button
            class="builder-open-btn"
            type="button"
            disabled={qbLoading || !connectionsStore.isActive(connectionId)}
            onclick={openBuilder}
            title={!connectionsStore.isActive(connectionId) ? 'Connect to a database first' : ''}
          >{qbLoading ? 'Loading…' : 'Query Builder'}</button>
        </div>
        {#if qbError}<p class="builder-error">{qbError}</p>{/if}
        <textarea
          id="widget-sql"
          class="sql-input"
          placeholder="SELECT COUNT(*) FROM ..."
          rows="5"
          spellcheck={false}
          bind:value={sql}
        ></textarea>
      </div>

      <!-- Display type -->
      <div class="field">
        <label class="field-label">Display as</label>
        <div class="display-types">
          {#each DISPLAY_TYPES as dt}
            <button
              class="display-btn"
              class:active={displayType === dt.value}
              type="button"
              onclick={() => (displayType = dt.value)}
            >{dt.label}</button>
          {/each}
        </div>
      </div>

      {#if displayType === 'count'}
        <div class="field">
          <label class="field-label" for="sv-format">Value format</label>
          <Select
            id="sv-format"
            bind:value={singleValueFormat}
            options={SINGLE_VALUE_FORMATS}
            size="sm"
          />
        </div>
        {#if singleValueFormat === 'currency'}
          <div class="field">
            <label class="field-label" for="sv-currency">Currency</label>
            <Select
              id="sv-currency"
              bind:value={singleValueCurrency}
              options={CURRENCIES}
              size="sm"
              searchable
            />
          </div>
        {/if}
      {/if}

      <!-- Grid size -->
      <div class="field-row">
        <div class="field">
          <label class="field-label" for="widget-w">Width (1–12 cols)</label>
          <input
            id="widget-w"
            class="field-input field-input--sm"
            type="number"
            min="1"
            max="12"
            bind:value={w}
          />
        </div>
        <div class="field">
          <label class="field-label" for="widget-h">Height (1–16 rows)</label>
          <input
            id="widget-h"
            class="field-input field-input--sm"
            type="number"
            min="1"
            max="16"
            bind:value={h}
          />
        </div>
      </div>

    </div>

    <div class="dialog-footer">
      <button class="btn btn-secondary" onclick={oncancel} type="button">Cancel</button>
      <button class="btn btn-primary" onclick={handleSave} disabled={!canSave} type="button">
        {widget ? 'Save' : 'Add Widget'}
      </button>
    </div>
  </div>
</Modal>

{#if showQueryBuilder}
  <QueryBuilderModal
    tables={qbTables}
    loadColumns={loadColumns}
    defaultDatabase={database}
    oninsert={(generatedSql) => {
      sql = generatedSql;
      showQueryBuilder = false;
    }}
    onclose={() => {
      showQueryBuilder = false;
    }}
  />
{/if}

<style>
  .dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg);
    width: min(540px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .dialog-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    margin: 0;
  }

  .close-btn {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-md);
    color: var(--color-text-muted);
    cursor: pointer;
    background: none;
    border: none;
  }

  .close-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .dialog-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-4) var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .dialog-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-5);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-3);
  }

  .field-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .field-input {
    padding: 6px var(--spacing-2);
    font-size: var(--font-size-sm);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    outline: none;
    font-family: var(--font-family-ui);
  }

  .field-input:focus {
    border-color: var(--color-accent);
  }

  .field-input--sm {
    width: 100px;
  }


  .sql-input {
    padding: var(--spacing-2);
    font-size: 12.5px;
    font-family: var(--font-family-mono);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    resize: vertical;
    outline: none;
    line-height: 1.6;
  }

  .sql-input:focus {
    border-color: var(--color-accent);
  }

  .display-types {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .display-btn {
    padding: 4px 12px;
    font-size: var(--font-size-xs);
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
  }

  .display-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .btn {
    padding: var(--spacing-2) var(--spacing-4);
    font-size: var(--font-size-sm);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-family: var(--font-family-ui);
    font-weight: var(--font-weight-medium);
    transition: background var(--transition-fast);
    border: none;
  }

  .btn-secondary {
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    color: var(--color-text-secondary);
  }

  .btn-secondary:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .btn-primary {
    background: var(--color-accent);
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    opacity: 0.9;
  }

  .btn-primary:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .sql-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .builder-open-btn {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    padding: 2px 8px;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast);
  }

  .builder-open-btn:hover:not(:disabled) {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .builder-open-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .builder-error {
    font-size: var(--font-size-xs);
    color: var(--color-danger);
    margin: 0;
  }
</style>
