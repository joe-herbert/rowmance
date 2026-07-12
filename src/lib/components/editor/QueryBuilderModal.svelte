<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';

  type Operation = 'select' | 'insert' | 'update' | 'delete' | 'create_table' | 'truncate';
  type Aggregate = 'none' | 'COUNT' | 'COUNT_DISTINCT' | 'SUM' | 'AVG' | 'MIN' | 'MAX';
  type JoinType = 'INNER JOIN' | 'LEFT JOIN' | 'RIGHT JOIN' | 'FULL OUTER JOIN' | 'CROSS JOIN';
  type WhereOp = '=' | '!=' | '<' | '>' | '<=' | '>=' | 'LIKE' | 'NOT LIKE' | 'IN' | 'NOT IN' | 'IS NULL' | 'IS NOT NULL' | 'BETWEEN';
  type SortDir = 'ASC' | 'DESC';
  type Connector = 'AND' | 'OR';

  export interface SchemaTable { database: string; name: string; }
  export interface SchemaColumn { name: string; dataType: string; }

  interface WhereRow { id: string; column: string; operator: WhereOp; value: string; connector: Connector; }
  interface JoinRow { id: string; type: JoinType; table: string; onLeft: string; onRight: string; }
  interface SelectCol { name: string; aggregate: Aggregate; alias: string; checked: boolean; }
  interface InsertCol { name: string; dataType: string; value: string; checked: boolean; useNull: boolean; }
  interface UpdateSetRow { id: string; column: string; value: string; useNull: boolean; }
  interface CreateCol { id: string; name: string; type: string; length: string; nullable: boolean; primaryKey: boolean; autoIncrement: boolean; unique: boolean; defaultValue: string; }

  interface Props {
    tables: SchemaTable[];
    loadColumns: (database: string, table: string) => Promise<SchemaColumn[]>;
    defaultDatabase?: string;
    oninsert: (sql: string) => void;
    onclose: () => void;
  }

  let { tables, loadColumns, defaultDatabase = '', oninsert, onclose }: Props = $props();

  function uid() { return Math.random().toString(36).slice(2, 9); }

  // ── Core state ───────────────────────────────────────────────────────────
  let operation = $state<Operation>('select');
  let selectedDatabase = $state(untrack(() => defaultDatabase));
  let selectedTable = $state('');

  const databases = $derived([...new Set(tables.map(t => t.database))].sort());
  const multiDb = $derived(databases.length > 1);

  const tableOptions = $derived(
    multiDb
      ? databases.map(db => ({
          group: db,
          options: tables
            .filter(t => t.database === db)
            .sort((a, b) => a.name.localeCompare(b.name))
            .map(t => ({ value: `${db}.${t.name}`, label: t.name })),
        }))
      : tables
          .sort((a, b) => a.name.localeCompare(b.name))
          .map(t => ({ value: `${t.database}.${t.name}`, label: t.name }))
  );

  let columns = $state<SchemaColumn[]>([]);
  let columnsLoading = $state(false);

  const colNames = $derived(columns.map(c => c.name));
  const colOptions = $derived(columns.map(c => ({ value: c.name, label: c.name })));
  // Always use db.table as value for consistency when looking up columns
  const allTableOptions = $derived(
    tables.sort((a, b) => a.name.localeCompare(b.name)).map(t => ({
      value: `${t.database}.${t.name}`,
      label: multiDb ? `${t.database}.${t.name}` : t.name,
    }))
  );

  // Columns for each join's target table, keyed by join id
  let joinCols = $state(new Map<string, SchemaColumn[]>());

  // ── SELECT state ─────────────────────────────────────────────────────────
  let selectDistinct = $state(false);
  let selectCols = $state<SelectCol[]>([]);
  let selectWhere = $state<WhereRow[]>([]);
  let selectJoins = $state<JoinRow[]>([]);
  let selectGroupBy = $state<string[]>([]);
  let selectOrderBy = $state<{ id: string; column: string; dir: SortDir }[]>([]);
  let selectLimit = $state('');
  let selectOffset = $state('');
  let selectHaving = $state('');

  // ── INSERT state ─────────────────────────────────────────────────────────
  let insertCols = $state<InsertCol[]>([]);

  // ── UPDATE state ─────────────────────────────────────────────────────────
  let updateSet = $state<UpdateSetRow[]>([]);
  let updateWhere = $state<WhereRow[]>([]);

  // ── DELETE state ─────────────────────────────────────────────────────────
  let deleteWhere = $state<WhereRow[]>([]);

  // ── CREATE TABLE state ────────────────────────────────────────────────────
  let createTableName = $state('new_table');
  let createCols = $state<CreateCol[]>([
    { id: uid(), name: 'id', type: 'INT', length: '', nullable: false, primaryKey: true, autoIncrement: true, unique: false, defaultValue: '' },
    { id: uid(), name: 'created_at', type: 'DATETIME', length: '', nullable: true, primaryKey: false, autoIncrement: false, unique: false, defaultValue: 'NULL' },
  ]);

  // ── Load columns and reset state when table changes ──────────────────────
  $effect(() => {
    const db = selectedDatabase;
    const tbl = selectedTable;
    if (!db || !tbl) {
      columns = [];
      return;
    }
    columnsLoading = true;
    loadColumns(db, tbl).then(cols => {
      columns = cols;
      columnsLoading = false;
      selectCols = cols.map(c => ({ name: c.name, aggregate: 'none', alias: '', checked: true }));
      insertCols = cols.map(c => ({ name: c.name, dataType: c.dataType, value: '', checked: true, useNull: false }));
      if (cols.length > 0) {
        updateSet = [{ id: uid(), column: cols[0].name, value: '', useNull: false }];
      }
      selectWhere = [];
      updateWhere = [];
      deleteWhere = [];
      selectJoins = [];
      joinCols = new Map();
      selectGroupBy = [];
      selectOrderBy = [];
      selectLimit = '';
      selectOffset = '';
      selectHaving = '';
    }).catch(() => {
      columns = [];
      columnsLoading = false;
    });
  });

  function parseDbTable(val: string): { db: string; table: string } | null {
    const dot = val.indexOf('.');
    if (dot === -1) return null;
    return { db: val.slice(0, dot), table: val.slice(dot + 1) };
  }

  function handleJoinTableSelect(joinId: string, val: string, i: number) {
    selectJoins[i] = { ...selectJoins[i], table: val, onRight: '' };
    const parsed = parseDbTable(val);
    if (!parsed) return;
    loadColumns(parsed.db, parsed.table).then(cols => {
      joinCols = new Map(joinCols).set(joinId, cols);
    }).catch(() => {});
  }

  function joinColOptions(joinId: string): { value: string; label: string }[] {
    return (joinCols.get(joinId) ?? []).map(c => ({ value: c.name, label: c.name }));
  }

  // ── Table selection ───────────────────────────────────────────────────────
  function handleTableSelect(val: string) {
    const dot = val.indexOf('.');
    if (dot !== -1) {
      selectedDatabase = val.slice(0, dot);
      selectedTable = val.slice(dot + 1);
    } else {
      selectedTable = val;
      if (databases.length === 1) selectedDatabase = databases[0];
    }
  }

  const tableSelectValue = $derived(
    selectedTable && selectedDatabase ? `${selectedDatabase}.${selectedTable}` : undefined
  );

  // ── SQL generation ────────────────────────────────────────────────────────
  const qualifiedTable = $derived(
    selectedTable
      ? (multiDb && selectedDatabase ? `${selectedDatabase}.${selectedTable}` : selectedTable)
      : 'table_name'
  );

  function quoteVal(v: string, op: WhereOp = '='): string {
    const t = v.trim();
    if (!t || t.toUpperCase() === 'NULL') return 'NULL';
    if (t.startsWith('(') || /^-?\d+(\.\d+)?$/.test(t)) return t;
    if (op === 'IN' || op === 'NOT IN') {
      if (t.startsWith('(')) return t;
      return '(' + t.split(',').map(p => `'${p.trim().replace(/'/g, "''")}'`).join(', ') + ')';
    }
    return `'${t.replace(/'/g, "''")}'`;
  }

  function buildWhere(rows: WhereRow[]): string {
    if (rows.length === 0) return '';
    const parts = rows.map((r, i) => {
      let expr: string;
      if (r.operator === 'IS NULL' || r.operator === 'IS NOT NULL') {
        expr = `${r.column} ${r.operator}`;
      } else if (r.operator === 'BETWEEN') {
        const [a, b = ''] = r.value.split(',');
        expr = `${r.column} BETWEEN ${quoteVal(a.trim())} AND ${quoteVal(b.trim())}`;
      } else {
        expr = `${r.column} ${r.operator} ${quoteVal(r.value, r.operator)}`;
      }
      return i === 0 ? expr : `${r.connector} ${expr}`;
    });
    return 'WHERE ' + parts.join('\n  ');
  }

  function buildSelect(): string {
    const checked = selectCols.filter(c => c.checked);
    let colList: string;
    if (checked.length === 0) {
      colList = '*';
    } else {
      colList = checked.map(c => {
        let expr = c.name;
        if (c.aggregate !== 'none') {
          expr = c.aggregate === 'COUNT_DISTINCT' ? `COUNT(DISTINCT ${expr})` : `${c.aggregate}(${expr})`;
        }
        return c.alias ? `${expr} AS ${c.alias}` : expr;
      }).join(',\n       ');
    }

    let sql = `SELECT ${selectDistinct ? 'DISTINCT ' : ''}${colList}\nFROM ${qualifiedTable}`;

    for (const j of selectJoins) {
      if (j.table) {
        sql += `\n${j.type} ${j.table}`;
        if (j.onLeft && j.onRight) sql += ` ON ${j.onLeft} = ${j.onRight}`;
      }
    }

    const w = buildWhere(selectWhere);
    if (w) sql += `\n${w}`;

    if (selectGroupBy.length > 0) {
      sql += `\nGROUP BY ${selectGroupBy.join(', ')}`;
      if (selectHaving.trim()) sql += `\nHAVING ${selectHaving.trim()}`;
    }

    const validOrder = selectOrderBy.filter(o => o.column);
    if (validOrder.length > 0) sql += `\nORDER BY ${validOrder.map(o => `${o.column} ${o.dir}`).join(', ')}`;

    if (selectLimit.trim()) sql += `\nLIMIT ${selectLimit.trim()}`;
    if (selectOffset.trim()) sql += `\nOFFSET ${selectOffset.trim()}`;

    return sql + ';';
  }

  function buildInsert(): string {
    const cols = insertCols.filter(c => c.checked);
    if (cols.length === 0) return `INSERT INTO ${qualifiedTable}\n-- select columns above`;
    const names = cols.map(c => c.name).join(', ');
    const vals = cols.map(c => c.useNull ? 'NULL' : quoteVal(c.value)).join(', ');
    return `INSERT INTO ${qualifiedTable} (${names})\nVALUES (${vals});`;
  }

  function buildUpdate(): string {
    const sets = updateSet.filter(s => s.column);
    if (sets.length === 0) return `UPDATE ${qualifiedTable}\nSET -- add columns`;
    const setParts = sets.map(s => `${s.column} = ${s.useNull ? 'NULL' : quoteVal(s.value)}`);
    let sql = `UPDATE ${qualifiedTable}\nSET ${setParts.join(',\n    ')}`;
    const w = buildWhere(updateWhere);
    if (w) sql += `\n${w}`;
    return sql + ';';
  }

  function buildDelete(): string {
    let sql = `DELETE FROM ${qualifiedTable}`;
    const w = buildWhere(deleteWhere);
    if (w) sql += `\n${w}`;
    else sql += `\n-- WARNING: no WHERE clause`;
    return sql + ';';
  }

  function buildCreateTable(): string {
    const name = createTableName.trim() || 'new_table';
    const defs = createCols.map(c => {
      const typeStr = c.length.trim() ? `${c.type}(${c.length.trim()})` : c.type;
      let def = `  ${c.name} ${typeStr}`;
      if (c.autoIncrement) def += ' AUTO_INCREMENT';
      if (!c.nullable) def += ' NOT NULL';
      if (c.unique && !c.primaryKey) def += ' UNIQUE';
      if (c.defaultValue.trim()) def += ` DEFAULT ${c.defaultValue.trim()}`;
      return def;
    });
    const pks = createCols.filter(c => c.primaryKey).map(c => c.name);
    if (pks.length > 0) defs.push(`  PRIMARY KEY (${pks.join(', ')})`);
    return `CREATE TABLE ${name} (\n${defs.join(',\n')}\n);`;
  }

  const generatedSql = $derived.by((): string => {
    if (operation === 'create_table') return buildCreateTable();
    if (!selectedTable) return '-- Select a table above';
    switch (operation) {
      case 'select': return buildSelect();
      case 'insert': return buildInsert();
      case 'update': return buildUpdate();
      case 'delete': return buildDelete();
      case 'truncate': return `TRUNCATE TABLE ${qualifiedTable};`;
    }
  });

  // ── UI helpers ────────────────────────────────────────────────────────────
  const OPERATIONS: { id: Operation; label: string }[] = [
    { id: 'select', label: 'SELECT' },
    { id: 'insert', label: 'INSERT' },
    { id: 'update', label: 'UPDATE' },
    { id: 'delete', label: 'DELETE' },
    { id: 'create_table', label: 'CREATE TABLE' },
    { id: 'truncate', label: 'TRUNCATE' },
  ];

  const WHERE_OPS = ['=', '!=', '<', '>', '<=', '>=', 'LIKE', 'NOT LIKE', 'IN', 'NOT IN', 'IS NULL', 'IS NOT NULL', 'BETWEEN'] as const;
  const JOIN_TYPES = ['INNER JOIN', 'LEFT JOIN', 'RIGHT JOIN', 'FULL OUTER JOIN', 'CROSS JOIN'] as const;
  const AGGREGATES: { value: Aggregate; label: string }[] = [
    { value: 'none', label: '—' },
    { value: 'COUNT', label: 'COUNT()' },
    { value: 'COUNT_DISTINCT', label: 'COUNT(DISTINCT)' },
    { value: 'SUM', label: 'SUM()' },
    { value: 'AVG', label: 'AVG()' },
    { value: 'MIN', label: 'MIN()' },
    { value: 'MAX', label: 'MAX()' },
  ];
  const SQL_TYPES = ['INT', 'BIGINT', 'SMALLINT', 'TINYINT', 'VARCHAR', 'CHAR', 'TEXT', 'MEDIUMTEXT', 'LONGTEXT', 'DECIMAL', 'FLOAT', 'DOUBLE', 'DATE', 'DATETIME', 'TIMESTAMP', 'BOOLEAN', 'JSON', 'UUID', 'BLOB'];

  function newWhereRow(cols: string[]): WhereRow {
    return { id: uid(), column: cols[0] ?? '', operator: '=', value: '', connector: 'AND' };
  }

  function valueInputHidden(op: WhereOp) {
    return op === 'IS NULL' || op === 'IS NOT NULL';
  }
</script>

<Modal label="Query Builder" onbackdropclick={onclose}>
  <div class="qb-card" role="dialog" tabindex="-1" onkeydown={e => e.key === 'Escape' && onclose()}>

    <!-- Header -->
    <div class="qb-header">
      <span class="qb-title">Query Builder</span>
      <button class="qb-close" onclick={onclose} aria-label="Close">✕</button>
    </div>

    <!-- Operation picker -->
    <div class="qb-ops">
      {#each OPERATIONS as op}
        <button
          class="qb-op-btn"
          class:qb-op-btn--active={operation === op.id}
          onclick={() => { operation = op.id; }}
        >{op.label}</button>
      {/each}
    </div>

    <div class="qb-body">

      <!-- Table selector (all ops except create_table) -->
      {#if operation !== 'create_table'}
        <div class="qb-section">
          <div class="qb-section-label">Table</div>
          <Select
            value={tableSelectValue}
            options={tableOptions}
            size="md"
            searchable
            placeholder="Choose a table…"
            onchange={handleTableSelect}
          />
        </div>
      {/if}

      {#if columnsLoading}
        <div class="qb-loading">Loading columns…</div>
      {/if}

      <!-- ═══ SELECT ═══════════════════════════════════════════════════════════ -->
      {#if operation === 'select' && selectedTable && !columnsLoading}

        <!-- Columns -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Columns</span>
            <label class="qb-inline-check">
              <Checkbox bind:checked={selectDistinct} size="sm" />
              <span>DISTINCT</span>
            </label>
            <button class="qb-link" onclick={() => { selectCols = selectCols.map(c => ({ ...c, checked: true })); }}>All</button>
            <button class="qb-link" onclick={() => { selectCols = selectCols.map(c => ({ ...c, checked: false })); }}>None</button>
          </div>
          <div class="qb-col-grid qb-col-grid--select">
            <div class="qb-col-hdr"></div>
            <div class="qb-col-hdr">Column</div>
            <div class="qb-col-hdr">Aggregate</div>
            <div class="qb-col-hdr">Alias</div>
            {#each selectCols as col, i}
              <Checkbox bind:checked={selectCols[i].checked} size="sm" />
              <span class="qb-col-name" class:qb-col-name--dim={!col.checked}>{col.name}</span>
              <Select
                value={col.aggregate}
                options={AGGREGATES.map(a => ({ value: a.value, label: a.label }))}
                size="xs"
                disabled={!col.checked}
                onchange={v => { selectCols[i] = { ...selectCols[i], aggregate: v as Aggregate }; }}
              />
              <input
                class="qb-input qb-input--xs"
                type="text"
                placeholder="alias"
                value={col.alias}
                disabled={!col.checked}
                oninput={e => { selectCols[i] = { ...selectCols[i], alias: (e.currentTarget as HTMLInputElement).value }; }}
              />
            {/each}
          </div>
        </div>

        <!-- JOINs -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">JOINs</span>
            <button class="qb-add-btn" onclick={() => { selectJoins = [...selectJoins, { id: uid(), type: 'INNER JOIN', table: '', onLeft: '', onRight: '' }]; }}>+ Add join</button>
          </div>
          {#each selectJoins as join, i}
            <div class="qb-row qb-join-row">
              <Select
                value={join.type}
                options={JOIN_TYPES.map(t => ({ value: t, label: t }))}
                size="sm"
                onchange={v => { selectJoins[i] = { ...selectJoins[i], type: v as JoinType }; }}
              />
              <Select
                value={join.table || undefined}
                options={allTableOptions}
                size="sm"
                searchable
                placeholder="table…"
                onchange={v => handleJoinTableSelect(join.id, v, i)}
              />
              <span class="qb-label-on">ON</span>
              <Select
                value={join.onLeft || undefined}
                options={colOptions}
                size="sm"
                searchable
                placeholder="left col"
                disabled={colOptions.length === 0}
                onchange={v => { selectJoins[i] = { ...selectJoins[i], onLeft: v }; }}
              />
              <span class="qb-eq">=</span>
              <Select
                value={join.onRight || undefined}
                options={joinColOptions(join.id)}
                size="sm"
                searchable
                placeholder="right col"
                disabled={!join.table || joinColOptions(join.id).length === 0}
                onchange={v => { selectJoins[i] = { ...selectJoins[i], onRight: v }; }}
              />
              <button class="qb-remove-btn" onclick={() => { selectJoins = selectJoins.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

        <!-- WHERE -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">WHERE</span>
            <button class="qb-add-btn" onclick={() => { selectWhere = [...selectWhere, newWhereRow(colNames)]; }}>+ Add condition</button>
          </div>
          {#each selectWhere as row, i}
            <div class="qb-row">
              {#if i > 0}
                <Select value={row.connector} options={[{ value: 'AND', label: 'AND' }, { value: 'OR', label: 'OR' }]} size="xs"
                  onchange={v => { selectWhere[i] = { ...selectWhere[i], connector: v as Connector }; }} />
              {:else}
                <span class="qb-where-placeholder"></span>
              {/if}
              <Select value={row.column} options={colOptions} size="sm" searchable
                onchange={v => { selectWhere[i] = { ...selectWhere[i], column: v }; }} />
              <Select value={row.operator} options={WHERE_OPS.map(o => ({ value: o, label: o }))} size="sm"
                onchange={v => { selectWhere[i] = { ...selectWhere[i], operator: v as WhereOp }; }} />
              {#if !valueInputHidden(row.operator)}
                <input class="qb-input qb-input--flex" type="text"
                  placeholder={row.operator === 'BETWEEN' ? 'a, b' : row.operator === 'IN' || row.operator === 'NOT IN' ? 'a, b, c' : 'value'}
                  value={row.value}
                  oninput={e => { selectWhere[i] = { ...selectWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              {:else}
                <span class="qb-input-spacer"></span>
              {/if}
              <button class="qb-remove-btn" onclick={() => { selectWhere = selectWhere.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

        <!-- GROUP BY + HAVING -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">GROUP BY</span>
          </div>
          <div class="qb-checkboxes">
            {#each columns as col}
              {@const checked = selectGroupBy.includes(col.name)}
              <label class="qb-inline-check">
                <Checkbox {checked} size="sm" onchange={c => {
                  selectGroupBy = c
                    ? [...selectGroupBy, col.name]
                    : selectGroupBy.filter(n => n !== col.name);
                }} />
                <span>{col.name}</span>
              </label>
            {/each}
          </div>
          {#if selectGroupBy.length > 0}
            <div class="qb-row qb-having-row">
              <span class="qb-section-label">HAVING</span>
              <input class="qb-input qb-input--flex" type="text" bind:value={selectHaving} placeholder="e.g. COUNT(*) > 1" />
            </div>
          {/if}
        </div>

        <!-- ORDER BY -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">ORDER BY</span>
            <button class="qb-add-btn" onclick={() => { selectOrderBy = [...selectOrderBy, { id: uid(), column: colNames[0] ?? '', dir: 'ASC' }]; }}>+ Add</button>
          </div>
          {#each selectOrderBy as ord, i}
            <div class="qb-row">
              <Select value={ord.column} options={colOptions} size="sm" searchable
                onchange={v => { selectOrderBy[i] = { ...selectOrderBy[i], column: v }; }} />
              <Select value={ord.dir} options={[{ value: 'ASC', label: 'ASC' }, { value: 'DESC', label: 'DESC' }]} size="sm"
                onchange={v => { selectOrderBy[i] = { ...selectOrderBy[i], dir: v as SortDir }; }} />
              <button class="qb-remove-btn" onclick={() => { selectOrderBy = selectOrderBy.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

        <!-- LIMIT / OFFSET -->
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">LIMIT / OFFSET</span>
          </div>
          <div class="qb-row">
            <label class="qb-field">
              <span class="qb-field-label">LIMIT</span>
              <input class="qb-input" type="number" min="0" bind:value={selectLimit} placeholder="e.g. 100" />
            </label>
            <label class="qb-field">
              <span class="qb-field-label">OFFSET</span>
              <input class="qb-input" type="number" min="0" bind:value={selectOffset} placeholder="e.g. 0" />
            </label>
          </div>
        </div>

      <!-- ═══ INSERT ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'insert' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Values</span>
            <button class="qb-link" onclick={() => { insertCols = insertCols.map(c => ({ ...c, checked: true })); }}>All</button>
            <button class="qb-link" onclick={() => { insertCols = insertCols.map(c => ({ ...c, checked: false })); }}>None</button>
          </div>
          <div class="qb-col-grid qb-col-grid--insert">
            <div class="qb-col-hdr"></div>
            <div class="qb-col-hdr">Column</div>
            <div class="qb-col-hdr">Type</div>
            <div class="qb-col-hdr">Value</div>
            <div class="qb-col-hdr">NULL</div>
            {#each insertCols as col, i}
              <Checkbox bind:checked={insertCols[i].checked} size="sm" />
              <span class="qb-col-name" class:qb-col-name--dim={!col.checked}>{col.name}</span>
              <span class="qb-col-type">{col.dataType}</span>
              <input
                class="qb-input"
                type="text"
                placeholder="value"
                value={col.value}
                disabled={!col.checked || col.useNull}
                oninput={e => { insertCols[i] = { ...insertCols[i], value: (e.currentTarget as HTMLInputElement).value }; }}
              />
              <Checkbox bind:checked={insertCols[i].useNull} size="sm" disabled={!col.checked} />
            {/each}
          </div>
        </div>

      <!-- ═══ UPDATE ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'update' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">SET</span>
            <button class="qb-add-btn" onclick={() => { updateSet = [...updateSet, { id: uid(), column: colNames[0] ?? '', value: '', useNull: false }]; }}>+ Add column</button>
          </div>
          {#each updateSet as row, i}
            <div class="qb-row">
              <Select value={row.column} options={colOptions} size="sm" searchable
                onchange={v => { updateSet[i] = { ...updateSet[i], column: v }; }} />
              <span class="qb-eq">=</span>
              <input class="qb-input qb-input--flex" type="text" placeholder="value"
                value={row.value} disabled={row.useNull}
                oninput={e => { updateSet[i] = { ...updateSet[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              <label class="qb-inline-check">
                <Checkbox bind:checked={updateSet[i].useNull} size="sm" />
                <span>NULL</span>
              </label>
              <button class="qb-remove-btn" onclick={() => { updateSet = updateSet.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">WHERE</span>
            <button class="qb-add-btn" onclick={() => { updateWhere = [...updateWhere, newWhereRow(colNames)]; }}>+ Add condition</button>
          </div>
          {#each updateWhere as row, i}
            <div class="qb-row">
              {#if i > 0}
                <Select value={row.connector} options={[{ value: 'AND', label: 'AND' }, { value: 'OR', label: 'OR' }]} size="xs"
                  onchange={v => { updateWhere[i] = { ...updateWhere[i], connector: v as Connector }; }} />
              {:else}
                <span class="qb-where-placeholder"></span>
              {/if}
              <Select value={row.column} options={colOptions} size="sm" searchable
                onchange={v => { updateWhere[i] = { ...updateWhere[i], column: v }; }} />
              <Select value={row.operator} options={WHERE_OPS.map(o => ({ value: o, label: o }))} size="sm"
                onchange={v => { updateWhere[i] = { ...updateWhere[i], operator: v as WhereOp }; }} />
              {#if !valueInputHidden(row.operator)}
                <input class="qb-input qb-input--flex" type="text"
                  placeholder={row.operator === 'BETWEEN' ? 'a, b' : 'value'}
                  value={row.value}
                  oninput={e => { updateWhere[i] = { ...updateWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              {:else}
                <span class="qb-input-spacer"></span>
              {/if}
              <button class="qb-remove-btn" onclick={() => { updateWhere = updateWhere.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
          {#if updateWhere.length === 0}
            <div class="qb-warning">No WHERE clause — all rows will be updated.</div>
          {/if}
        </div>

      <!-- ═══ DELETE ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'delete' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">WHERE</span>
            <button class="qb-add-btn" onclick={() => { deleteWhere = [...deleteWhere, newWhereRow(colNames)]; }}>+ Add condition</button>
          </div>
          {#each deleteWhere as row, i}
            <div class="qb-row">
              {#if i > 0}
                <Select value={row.connector} options={[{ value: 'AND', label: 'AND' }, { value: 'OR', label: 'OR' }]} size="xs"
                  onchange={v => { deleteWhere[i] = { ...deleteWhere[i], connector: v as Connector }; }} />
              {:else}
                <span class="qb-where-placeholder"></span>
              {/if}
              <Select value={row.column} options={colOptions} size="sm" searchable
                onchange={v => { deleteWhere[i] = { ...deleteWhere[i], column: v }; }} />
              <Select value={row.operator} options={WHERE_OPS.map(o => ({ value: o, label: o }))} size="sm"
                onchange={v => { deleteWhere[i] = { ...deleteWhere[i], operator: v as WhereOp }; }} />
              {#if !valueInputHidden(row.operator)}
                <input class="qb-input qb-input--flex" type="text"
                  placeholder={row.operator === 'BETWEEN' ? 'a, b' : 'value'}
                  value={row.value}
                  oninput={e => { deleteWhere[i] = { ...deleteWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              {:else}
                <span class="qb-input-spacer"></span>
              {/if}
              <button class="qb-remove-btn" onclick={() => { deleteWhere = deleteWhere.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
          {#if deleteWhere.length === 0}
            <div class="qb-warning">No WHERE clause — all rows will be deleted.</div>
          {/if}
        </div>

      <!-- ═══ CREATE TABLE ═══════════════════════════════════════════════════════ -->
      {:else if operation === 'create_table'}
        <div class="qb-section">
          <div class="qb-section-label">Table name</div>
          <input class="qb-input qb-input--full" type="text" bind:value={createTableName} placeholder="table_name" />
        </div>
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Columns</span>
            <button class="qb-add-btn" onclick={() => { createCols = [...createCols, { id: uid(), name: '', type: 'VARCHAR', length: '255', nullable: true, primaryKey: false, autoIncrement: false, unique: false, defaultValue: '' }]; }}>+ Add column</button>
          </div>
          <div class="qb-create-grid">
            <div class="qb-col-hdr">Name</div>
            <div class="qb-col-hdr">Type</div>
            <div class="qb-col-hdr">Length</div>
            <div class="qb-col-hdr" title="Nullable">NULL</div>
            <div class="qb-col-hdr" title="Primary key">PK</div>
            <div class="qb-col-hdr" title="Auto increment">AI</div>
            <div class="qb-col-hdr" title="Unique">UQ</div>
            <div class="qb-col-hdr">Default</div>
            <div class="qb-col-hdr"></div>
            {#each createCols as col, i}
              <input class="qb-input" type="text" placeholder="col_name" value={col.name}
                oninput={e => { createCols[i] = { ...createCols[i], name: (e.currentTarget as HTMLInputElement).value }; }} />
              <Select value={col.type} options={SQL_TYPES.map(t => ({ value: t, label: t }))} size="xs"
                onchange={v => { createCols[i] = { ...createCols[i], type: v }; }} />
              <input class="qb-input qb-input--xs" type="text" placeholder="255" value={col.length}
                oninput={e => { createCols[i] = { ...createCols[i], length: (e.currentTarget as HTMLInputElement).value }; }} />
              <Checkbox bind:checked={createCols[i].nullable} size="sm" />
              <Checkbox bind:checked={createCols[i].primaryKey} size="sm" />
              <Checkbox bind:checked={createCols[i].autoIncrement} size="sm" />
              <Checkbox bind:checked={createCols[i].unique} size="sm" />
              <input class="qb-input" type="text" placeholder="NULL" value={col.defaultValue}
                oninput={e => { createCols[i] = { ...createCols[i], defaultValue: (e.currentTarget as HTMLInputElement).value }; }} />
              <button class="qb-remove-btn" onclick={() => { createCols = createCols.filter((_, j) => j !== i); }}>✕</button>
            {/each}
          </div>
        </div>

      <!-- ═══ TRUNCATE ═══════════════════════════════════════════════════════════ -->
      {:else if operation === 'truncate' && selectedTable && !columnsLoading}
        <div class="qb-warning qb-warning--lg">
          This will delete all rows from <strong>{qualifiedTable}</strong>. This action cannot be undone.
        </div>
      {/if}

    </div>

    <!-- SQL preview -->
    <div class="qb-preview">
      <div class="qb-preview-label">SQL Preview</div>
      <div class="qb-preview-code">
        <SqlHighlight sql={generatedSql} />
      </div>
    </div>

    <!-- Footer -->
    <div class="qb-footer">
      <button class="qb-btn" onclick={onclose}>Cancel</button>
      <button
        class="qb-btn qb-btn--primary"
        disabled={operation !== 'create_table' && !selectedTable}
        onclick={() => oninsert(generatedSql)}
      >Insert at line</button>
    </div>

  </div>
</Modal>

<style>
  .qb-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-overlay);
    width: min(860px, 94vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-in 140ms ease both;
    font-family: var(--font-family-ui);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  @keyframes modal-in {
    from { opacity: 0; transform: scale(0.97) translateY(-8px); }
    to   { opacity: 1; transform: scale(1) translateY(0); }
  }

  /* ── Header ─────────────────────────────────────────────────────────────── */

  .qb-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .qb-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    flex: 1;
  }

  .qb-close {
    background: none;
    border: none;
    color: var(--color-text-muted);
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .qb-close:hover { color: var(--color-text-primary); background: var(--color-bg-hover); }

  /* ── Operation tabs ─────────────────────────────────────────────────────── */

  .qb-ops {
    display: flex;
    gap: var(--spacing-1);
    padding: var(--spacing-2) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .qb-op-btn {
    padding: 4px 10px;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: all var(--transition-fast);
    letter-spacing: 0.04em;
  }

  .qb-op-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  .qb-op-btn--active {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  /* ── Body ───────────────────────────────────────────────────────────────── */

  .qb-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-3) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  /* ── Section ────────────────────────────────────────────────────────────── */

  .qb-section {
    padding: var(--spacing-2) 0;
    border-bottom: 1px solid var(--color-border);
  }

  .qb-section:last-child { border-bottom: none; }

  .qb-section-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-bottom: var(--spacing-2);
  }

  .qb-section-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    margin-bottom: var(--spacing-2);
    display: block;
  }

  .qb-section-header .qb-section-label {
    margin-bottom: 0;
  }

  /* ── Inputs ─────────────────────────────────────────────────────────────── */

  .qb-input {
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-sm);
    outline: none;
    min-width: 0;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
    box-sizing: border-box;
  }

  .qb-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .qb-input:disabled { opacity: 0.4; cursor: not-allowed; }

  .qb-input--xs { height: 22px; font-size: var(--font-size-xs); padding: 0 5px; width: 60px; }
  .qb-input--flex { flex: 1; min-width: 80px; }
  .qb-input--full { width: 100%; }

  /* ── Rows ───────────────────────────────────────────────────────────────── */

  .qb-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-bottom: var(--spacing-1);
  }

  .qb-join-row { flex-wrap: wrap; }
  .qb-having-row { margin-top: var(--spacing-2); }

  .qb-where-placeholder { width: 42px; flex-shrink: 0; }
  .qb-input-spacer { flex: 1; min-width: 80px; }
  .qb-label-on, .qb-eq {
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    flex-shrink: 0;
  }

  /* ── Column grids ───────────────────────────────────────────────────────── */

  .qb-col-grid {
    display: grid;
    gap: 4px var(--spacing-2);
    align-items: center;
  }

  .qb-col-grid--select { grid-template-columns: 16px 1fr 110px 90px; }
  .qb-col-grid--insert { grid-template-columns: 16px 1fr 80px 1fr 16px; }

  .qb-create-grid {
    display: grid;
    grid-template-columns: 1fr 90px 50px 20px 20px 20px 20px 80px 22px;
    gap: 4px var(--spacing-2);
    align-items: center;
  }

  .qb-col-hdr {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-semibold);
    padding-bottom: 2px;
  }

  .qb-col-name {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .qb-col-name--dim { color: var(--color-text-muted); }
  .qb-col-type { font-family: var(--font-family-mono); font-size: var(--font-size-xs); color: var(--color-text-muted); }

  /* ── Checkboxes strip ───────────────────────────────────────────────────── */

  .qb-checkboxes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-2) var(--spacing-4);
  }

  .qb-inline-check {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    cursor: pointer;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

  /* ── Buttons ────────────────────────────────────────────────────────────── */

  .qb-add-btn {
    background: none;
    border: none;
    color: var(--color-accent);
    font-size: var(--font-size-xs);
    cursor: pointer;
    padding: 0;
    font-family: var(--font-family-ui);
    transition: opacity var(--transition-fast);
  }

  .qb-add-btn:hover { opacity: 0.75; }

  .qb-link {
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    cursor: pointer;
    padding: 0;
    font-family: var(--font-family-ui);
    transition: color var(--transition-fast);
  }

  .qb-link:hover { color: var(--color-text-primary); }

  .qb-remove-btn {
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 12px;
    cursor: pointer;
    padding: 2px 4px;
    border-radius: var(--radius-sm);
    line-height: 1;
    flex-shrink: 0;
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .qb-remove-btn:hover { color: var(--color-danger); background: var(--color-danger-subtle); }

  /* ── Field label helper ─────────────────────────────────────────────────── */

  .qb-field {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .qb-field-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Warning ────────────────────────────────────────────────────────────── */

  .qb-warning {
    background: var(--color-warning-subtle);
    border: 1px solid var(--color-warning);
    border-radius: var(--radius-md);
    color: var(--color-warning);
    font-size: var(--font-size-xs);
    padding: 4px 10px;
    margin-top: var(--spacing-1);
  }

  .qb-warning--lg {
    padding: var(--spacing-3);
    font-size: var(--font-size-sm);
    margin: var(--spacing-2) 0;
  }

  .qb-loading {
    padding: var(--spacing-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    text-align: center;
  }

  /* ── SQL Preview ────────────────────────────────────────────────────────── */

  .qb-preview {
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
    background: var(--color-editor-bg);
  }

  .qb-preview-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    padding: var(--spacing-2) var(--spacing-4) 0;
  }

  .qb-preview-code {
    padding: var(--spacing-2) var(--spacing-4) var(--spacing-3);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    white-space: pre;
    overflow-x: auto;
    max-height: 160px;
    overflow-y: auto;
    line-height: 1.6;
  }

  /* ── Footer ─────────────────────────────────────────────────────────────── */

  .qb-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .qb-btn {
    height: 28px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    font-family: var(--font-family-ui);
    transition: all var(--transition-fast);
    white-space: nowrap;
  }

  .qb-btn:hover { border-color: var(--color-border-strong); color: var(--color-text-primary); background: var(--color-bg-hover); }

  .qb-btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .qb-btn--primary:hover:not(:disabled) { opacity: 0.88; }
  .qb-btn--primary:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
