<!--
  ForeignKeyPicker — searchable list of rows from the referenced table of a
  foreign key column. Resolves the FK target itself, then searches across all
  of the referenced table's columns (not just the target column) so users can
  find a row by name/description/etc rather than needing to know its id.
  Each result previews a few columns to help disambiguate; selecting one
  returns the target column's value.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { executeQuery } from '$lib/tauri/query';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { qi, tableRef, castToText, defaultDialectInfo } from '$lib/utils/dialect';
  import Spinner from '$lib/components/ui/Spinner.svelte';

  type CellValue = string | number | boolean | null;

  interface Props {
    connectionId: string;
    database: string | null;
    table: string;
    column: string;
    onSelect: (_value: CellValue) => void;
  }

  let { connectionId, database, table, column, onSelect }: Props = $props();

  const connections = useConnections();

  // Column types not worth casting to text for search/preview (large binary blobs).
  const UNSEARCHABLE_TYPES = ['blob', 'binary', 'image', 'bytea'];

  function isSearchable(dataType: string): boolean {
    const lower = dataType.toLowerCase();
    return !UNSEARCHABLE_TYPES.some((t) => lower.includes(t));
  }

  interface Target {
    table: string;
    column: string;
    /** All searchable columns on the referenced table, target first. */
    searchColumns: string[];
    /** A handful of columns (including target) to display per result row. */
    previewColumns: string[];
  }

  let searchTerm = $state('');
  let results = $state<CellValue[][]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let focusedIndex = $state(-1);
  let target = $state<Target | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);

  let searchToken = 0;

  async function resolveTarget(): Promise<Target | null> {
    try {
      const fks = await schemaApi.listForeignKeys(connectionId, database ?? '', table);
      const fk = fks.find((f) => f.columns.includes(column));
      if (!fk) return null;
      const idx = fk.columns.indexOf(column);
      const refCol = fk.referencedColumns[idx] ?? fk.referencedColumns[0];
      if (!refCol) return null;

      const allColumns = await schemaApi.listColumns(
        connectionId,
        database ?? '',
        fk.referencedTable,
      );
      const searchColumns = [
        refCol,
        ...allColumns
          .filter((c) => c.name !== refCol && isSearchable(c.dataType))
          .map((c) => c.name),
      ];
      const previewColumns = [
        refCol,
        ...allColumns
          .filter((c) => c.name !== refCol && isSearchable(c.dataType))
          .slice(0, 5)
          .map((c) => c.name),
      ];

      return { table: fk.referencedTable, column: refCol, searchColumns, previewColumns };
    } catch {
      return null;
    }
  }

  async function runSearch(term: string): Promise<void> {
    if (!target) return;
    const resolvedTarget = target;
    const token = ++searchToken;
    loading = true;
    error = null;

    const d = connections.getById(connectionId)?.dialectInfo ?? defaultDialectInfo;
    const qTable = tableRef(database ?? '', resolvedTarget.table, d);
    const qPreviewCols = resolvedTarget.previewColumns.map((c) => qi(c, d));
    const qTargetCol = qi(resolvedTarget.column, d);

    function escapeLike(word: string): string {
      return word
        .replace(/\\/g, '\\\\')
        .replace(/%/g, '\\%')
        .replace(/_/g, '\\_')
        .replace(/'/g, "''");
    }

    // Each word must match *some* column, but different words may match
    // different columns — e.g. "Joe Herbert" matches firstname=Joe AND
    // surname=Herbert, not just a single column containing the whole phrase.
    const words = term.trim().split(/\s+/).filter(Boolean);
    const op = d.usesIlike ? 'ILIKE' : 'LIKE';
    const where = words.length
      ? `WHERE ${words
          .map(
            (word) =>
              `(${resolvedTarget.searchColumns
                .map((c) => `${castToText(qi(c, d), d)} ${op} '%${escapeLike(word)}%'`)
                .join(' OR ')})`,
          )
          .join(' AND ')}`
      : '';
    const sql = d.selectTop
      ? `SELECT TOP 50 ${qPreviewCols.join(', ')} FROM ${qTable} ${where} ORDER BY ${qTargetCol}`.trim()
      : `SELECT ${qPreviewCols.join(', ')} FROM ${qTable} ${where} ORDER BY ${qTargetCol} LIMIT 50`.trim();

    try {
      const result = await executeQuery(connectionId, sql, 1, 50, database ?? null);
      if (token !== searchToken) return;
      if (result.error) {
        error = result.error;
        results = [];
      } else {
        results = result.rows;
      }
    } catch (err) {
      if (token !== searchToken) return;
      error = err instanceof Error ? err.message : String(err);
      results = [];
    }
    if (token === searchToken) {
      loading = false;
      focusedIndex = results.length > 0 ? 0 : -1;
    }
  }

  onMount(async () => {
    inputEl?.focus();
    target = await resolveTarget();
    if (!target) {
      loading = false;
      error = 'No foreign key found for this column';
      return;
    }
    await runSearch('');
  });

  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  function handleInput(): void {
    clearTimeout(debounceHandle);
    debounceHandle = setTimeout(() => runSearch(searchTerm), 200);
  }

  function selectRow(row: CellValue[]): void {
    onSelect(row[0]);
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      e.stopPropagation();
      if (results.length > 0) focusedIndex = (focusedIndex + 1) % results.length;
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      if (results.length > 0) focusedIndex = (focusedIndex - 1 + results.length) % results.length;
    } else if (e.key === 'Enter') {
      if (focusedIndex >= 0 && focusedIndex < results.length) {
        e.preventDefault();
        e.stopPropagation();
        selectRow(results[focusedIndex]);
      }
    }
  }
</script>

<div class="fk-picker">
  <input
    bind:this={inputEl}
    type="text"
    class="fk-picker-search"
    placeholder="Search {target?.table ?? 'values'}…"
    autocomplete="off"
    autocorrect="off"
    autocapitalize="off"
    spellcheck="false"
    bind:value={searchTerm}
    oninput={handleInput}
    onkeydown={handleKeydown}
  />
  <div class="fk-picker-results" role="listbox" aria-label="Foreign key values">
    {#if loading}
      <div class="fk-picker-status"><Spinner size={14} /></div>
    {:else if error}
      <div class="fk-picker-status fk-picker-error">{error}</div>
    {:else if results.length === 0}
      <div class="fk-picker-status">No matching rows</div>
    {:else}
      <table class="fk-picker-table">
        <thead>
          <tr>
            {#each target?.previewColumns ?? [] as colName, ci (colName)}
              <th class="fk-picker-th" class:fk-picker-th-target={ci === 0}>{colName}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each results as row, idx (idx)}
            <tr
              role="option"
              aria-selected={idx === focusedIndex}
              class="fk-picker-row"
              class:fk-picker-row--focused={idx === focusedIndex}
              onmouseenter={() => (focusedIndex = idx)}
              onmousedown={(e) => e.preventDefault()}
              onclick={() => selectRow(row)}
            >
              {#each row as cell, ci (ci)}
                <td class="fk-picker-td" class:fk-picker-td-target={ci === 0}>
                  {#if cell === null}
                    <span class="fk-picker-null">NULL</span>
                  {:else}
                    {String(cell)}
                  {/if}
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .fk-picker {
    display: flex;
    flex-direction: column;
    width: 420px;
    max-height: 320px;
  }

  .fk-picker-search {
    flex-shrink: 0;
    padding: var(--spacing-2);
    background: transparent;
    border: none;
    border-bottom: 1px solid var(--color-border);
    outline: none;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    box-sizing: border-box;
  }

  .fk-picker-results {
    overflow: auto;
    scrollbar-color: var(--color-scrollbar-thumb) transparent;
    scrollbar-width: thin;
  }

  .fk-picker-status {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }

  .fk-picker-error {
    color: var(--color-danger);
    text-align: center;
  }

  .fk-picker-table {
    border-collapse: separate;
    border-spacing: 0;
    font-size: var(--font-size-xs);
    white-space: nowrap;
    width: 100%;
  }

  .fk-picker-th {
    position: sticky;
    top: 0;
    padding: 5px 10px;
    font-size: 11px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    background: var(--color-table-header-bg);
    border-bottom: 1px solid var(--color-border-strong);
    text-align: left;
    white-space: nowrap;
  }

  .fk-picker-th-target {
    color: var(--color-accent);
  }

  .fk-picker-row {
    cursor: pointer;
  }

  .fk-picker-row:hover,
  .fk-picker-row--focused {
    background: var(--color-accent-subtle);
  }

  .fk-picker-td {
    padding: 5px 10px;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
  }

  .fk-picker-td-target {
    font-weight: var(--font-weight-medium);
  }

  .fk-picker-null {
    color: var(--color-null);
    font-style: italic;
  }
</style>
