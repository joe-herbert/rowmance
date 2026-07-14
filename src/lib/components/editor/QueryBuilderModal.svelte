<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';

  type Operation =
    // DML
    | 'select' | 'insert' | 'replace' | 'update' | 'delete'
    // DDL — table
    | 'create_table' | 'alter_table' | 'drop_table' | 'truncate'
    // DDL — index / view / database
    | 'create_index' | 'drop_index' | 'create_view' | 'drop_view'
    | 'create_database' | 'drop_database'
    // DCL
    | 'create_user' | 'drop_user' | 'alter_user' | 'grant' | 'revoke'
    // TCL
    | 'begin' | 'commit' | 'rollback' | 'savepoint';

  type AlterSubOp = 'add_column' | 'drop_column' | 'rename_column' | 'rename_table';
  type Aggregate = 'none' | 'COUNT' | 'COUNT_DISTINCT' | 'SUM' | 'AVG' | 'MIN' | 'MAX';
  type JoinType = 'INNER JOIN' | 'LEFT JOIN' | 'RIGHT JOIN' | 'FULL OUTER JOIN' | 'CROSS JOIN';
  type WhereOp = '=' | '!=' | '<' | '>' | '<=' | '>=' | 'LIKE' | 'NOT LIKE' | 'IN' | 'NOT IN' | 'IS NULL' | 'IS NOT NULL' | 'BETWEEN';
  type SortDir = 'ASC' | 'DESC';
  type Connector = 'AND' | 'OR';
  type GrantLevel = 'global' | 'database' | 'table';
  type IsolationLevel = '' | 'READ UNCOMMITTED' | 'READ COMMITTED' | 'REPEATABLE READ' | 'SERIALIZABLE';

  export interface SchemaTable { database: string; name: string; }
  export interface SchemaColumn { name: string; dataType: string; }

  interface WhereRow { id: string; column: string; operator: WhereOp; value: string; connector: Connector; valueIsExpression?: boolean; }
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

  const databases = $derived(
    [...new Set(tables.map(t => t.database))].sort((a, b) => {
      if (a === defaultDatabase) return -1;
      if (b === defaultDatabase) return 1;
      return a.localeCompare(b);
    })
  );
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
  const allTableOptions = $derived(
    [...tables].sort((a, b) => a.name.localeCompare(b.name)).map(t => ({
      value: `${t.database}.${t.name}`,
      label: multiDb ? `${t.database}.${t.name}` : t.name,
    }))
  );

  let joinCols = $state(new Map<string, SchemaColumn[]>());

  // ── SELECT state ─────────────────────────────────────────────────────────
  let selectDistinct = $state(false);
  let selectCols = $state<SelectCol[]>([]);
  let selectWhere = $state<WhereRow[]>([]);
  let selectJoins = $state<JoinRow[]>([]);
  let selectGroupBy = $state<string[]>([]);
  let selectOrderBy = $state<{ id: string; column: string; dir: SortDir }[]>([]);
  let selectLimit = $state<number | ''>('');
  let selectOffset = $state<number | ''>('');
  let selectHaving = $state('');

  // ── INSERT / REPLACE state ────────────────────────────────────────────────
  let insertCols = $state<InsertCol[]>([]);
  let replaceCols = $state<InsertCol[]>([]);

  // ── UPDATE state ─────────────────────────────────────────────────────────
  let updateSet = $state<UpdateSetRow[]>([]);
  let updateWhere = $state<WhereRow[]>([]);

  // ── DELETE state ─────────────────────────────────────────────────────────
  let deleteWhere = $state<WhereRow[]>([]);

  // ── ALTER TABLE state ─────────────────────────────────────────────────────
  let alterSubOp = $state<AlterSubOp>('add_column');
  let alterAddCol = $state<CreateCol>({ id: uid(), name: 'new_column', type: 'VARCHAR', length: '255', nullable: true, primaryKey: false, autoIncrement: false, unique: false, defaultValue: '' });
  let alterDropColName = $state('');
  let alterRenameFrom = $state('');
  let alterRenameTo = $state('');
  let alterRenameTableTo = $state('');

  // ── CREATE INDEX state ────────────────────────────────────────────────────
  let createIndexName = $state('');
  let createIndexUnique = $state(false);
  let createIndexCols = $state<string[]>([]);

  // ── DROP INDEX state ──────────────────────────────────────────────────────
  let dropIndexName = $state('');

  // ── CREATE TABLE state ────────────────────────────────────────────────────
  let createTableName = $state('new_table');
  let createCols = $state<CreateCol[]>([
    { id: uid(), name: 'id', type: 'INT', length: '', nullable: false, primaryKey: true, autoIncrement: true, unique: false, defaultValue: '' },
    { id: uid(), name: 'created_at', type: 'DATETIME', length: '', nullable: true, primaryKey: false, autoIncrement: false, unique: false, defaultValue: 'NULL' },
  ]);

  // ── CREATE VIEW state ────────────────────────────────────────────────────
  let createViewName = $state('new_view');
  let createViewOrReplace = $state(false);
  let createViewBody = $state('SELECT\n  *\nFROM table_name');

  // ── DROP VIEW state ───────────────────────────────────────────────────────
  let dropViewName = $state('');
  let dropViewIfExists = $state(false);

  // ── CREATE DATABASE state ─────────────────────────────────────────────────
  let createDbName = $state('');
  let createDbIfNotExists = $state(false);
  let createDbCharset = $state('');
  let createDbCollation = $state('');

  // ── DROP DATABASE state ───────────────────────────────────────────────────
  let dropDbName = $state('');
  let dropDbIfExists = $state(false);

  // ── CREATE USER state ─────────────────────────────────────────────────────
  let createUserName = $state('');
  let createUserHost = $state('%');
  let createUserPassword = $state('');
  let createUserIfNotExists = $state(false);

  // ── DROP USER state ───────────────────────────────────────────────────────
  let dropUserName = $state('');
  let dropUserHost = $state('%');
  let dropUserIfExists = $state(false);

  // ── ALTER USER state ──────────────────────────────────────────────────────
  let alterUserName = $state('');
  let alterUserHost = $state('%');
  let alterUserPassword = $state('');

  // ── GRANT state ───────────────────────────────────────────────────────────
  let grantPrivileges = $state<string[]>(['SELECT']);
  let grantLevel = $state<GrantLevel>('table');
  let grantDatabase = $state('');
  let grantTable = $state('');
  let grantUser = $state('');
  let grantUserHost = $state('%');
  let grantWithGrantOption = $state(false);

  // ── REVOKE state ──────────────────────────────────────────────────────────
  let revokePrivileges = $state<string[]>(['SELECT']);
  let revokeLevel = $state<GrantLevel>('table');
  let revokeDatabase = $state('');
  let revokeTable = $state('');
  let revokeUser = $state('');
  let revokeUserHost = $state('%');

  // ── TCL state ─────────────────────────────────────────────────────────────
  let beginIsolation = $state<IsolationLevel>('');
  let rollbackToSavepoint = $state('');
  let savepointName = $state('');
  let savepointAction = $state<'create' | 'release'>('create');

  // ── Load columns when table changes ──────────────────────────────────────
  $effect(() => {
    const db = selectedDatabase;
    const tbl = selectedTable;
    if (!db || !tbl) { columns = []; return; }
    columnsLoading = true;
    loadColumns(db, tbl).then(cols => {
      columns = cols;
      columnsLoading = false;
      selectCols = cols.map(c => ({ name: c.name, aggregate: 'none', alias: '', checked: true }));
      insertCols = cols.map(c => ({ name: c.name, dataType: c.dataType, value: '', checked: true, useNull: false }));
      replaceCols = cols.map(c => ({ name: c.name, dataType: c.dataType, value: '', checked: true, useNull: false }));
      if (cols.length > 0) updateSet = [{ id: uid(), column: cols[0].name, value: '', useNull: false }];
      selectWhere = []; updateWhere = []; deleteWhere = [];
      selectJoins = []; joinCols = new Map();
      selectGroupBy = []; selectOrderBy = [];
      selectLimit = ''; selectOffset = ''; selectHaving = '';
      alterDropColName = ''; alterRenameFrom = '';
      alterAddCol = { id: uid(), name: 'new_column', type: 'VARCHAR', length: '255', nullable: true, primaryKey: false, autoIncrement: false, unique: false, defaultValue: '' };
      createIndexCols = [];
    }).catch(() => { columns = []; columnsLoading = false; });
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

  // Operations that require a table to be selected
  const TABLE_OPS = new Set<Operation>(['select', 'insert', 'replace', 'update', 'delete', 'truncate', 'alter_table', 'drop_table', 'create_index', 'drop_index']);
  const showTableSelector = $derived(TABLE_OPS.has(operation));

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
      } else if (r.valueIsExpression) {
        expr = `${r.column} ${r.operator} ${r.value.trim()}`;
      } else {
        expr = `${r.column} ${r.operator} ${quoteVal(r.value, r.operator)}`;
      }
      return i === 0 ? expr : `${r.connector} ${expr}`;
    });
    return 'WHERE ' + parts.join('\n  ');
  }

  function buildSelect(): string {
    const checked = selectCols.filter(c => c.checked);
    const colList = checked.length === 0
      ? '*'
      : checked.map(c => {
          let expr = c.name;
          if (c.aggregate !== 'none') expr = c.aggregate === 'COUNT_DISTINCT' ? `COUNT(DISTINCT ${expr})` : `${c.aggregate}(${expr})`;
          return c.alias ? `${expr} AS ${c.alias}` : expr;
        }).join(',\n       ');

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
    if (selectLimit !== '' && selectLimit !== null) sql += `\nLIMIT ${selectLimit}`;
    if (selectOffset !== '' && selectOffset !== null) sql += `\nOFFSET ${selectOffset}`;
    return sql + ';';
  }

  function buildInsertRows(keyword: string, cols: InsertCol[]): string {
    const selected = cols.filter(c => c.checked);
    if (selected.length === 0) return `${keyword} ${qualifiedTable}\n-- select columns above`;
    const names = selected.map(c => c.name).join(', ');
    const vals = selected.map(c => c.useNull ? 'NULL' : quoteVal(c.value)).join(', ');
    return `${keyword} ${qualifiedTable} (${names})\nVALUES (${vals});`;
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
    sql += w ? `\n${w}` : `\n-- WARNING: no WHERE clause`;
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

  function buildAlterTable(): string {
    switch (alterSubOp) {
      case 'add_column': {
        const c = alterAddCol;
        const typeStr = c.length.trim() ? `${c.type}(${c.length.trim()})` : c.type;
        let def = `${c.name || 'new_column'} ${typeStr}`;
        if (c.autoIncrement) def += ' AUTO_INCREMENT';
        if (!c.nullable) def += ' NOT NULL';
        if (c.unique && !c.primaryKey) def += ' UNIQUE';
        if (c.defaultValue.trim()) def += ` DEFAULT ${c.defaultValue.trim()}`;
        return `ALTER TABLE ${qualifiedTable}\n  ADD COLUMN ${def};`;
      }
      case 'drop_column':
        return alterDropColName
          ? `ALTER TABLE ${qualifiedTable}\n  DROP COLUMN ${alterDropColName};`
          : `ALTER TABLE ${qualifiedTable}\n  DROP COLUMN -- select a column above`;
      case 'rename_column':
        return alterRenameFrom && alterRenameTo
          ? `ALTER TABLE ${qualifiedTable}\n  RENAME COLUMN ${alterRenameFrom} TO ${alterRenameTo};`
          : `ALTER TABLE ${qualifiedTable}\n  RENAME COLUMN old_name TO new_name;`;
      case 'rename_table':
        return alterRenameTableTo.trim()
          ? `ALTER TABLE ${qualifiedTable}\n  RENAME TO ${alterRenameTableTo.trim()};`
          : `ALTER TABLE ${qualifiedTable}\n  RENAME TO new_table_name;`;
    }
  }

  function buildCreateIndex(): string {
    const keyword = createIndexUnique ? 'UNIQUE INDEX' : 'INDEX';
    const name = createIndexName.trim() || 'idx_name';
    const cols = createIndexCols.length > 0 ? createIndexCols.join(', ') : 'column_name';
    return `CREATE ${keyword} ${name}\n  ON ${qualifiedTable} (${cols});`;
  }

  function buildDropIndex(): string {
    return `DROP INDEX ${dropIndexName.trim() || 'index_name'} ON ${qualifiedTable};`;
  }

  function buildCreateView(): string {
    const orReplace = createViewOrReplace ? 'OR REPLACE ' : '';
    const name = createViewName.trim() || 'new_view';
    const body = createViewBody.trim() || 'SELECT * FROM table_name';
    return `CREATE ${orReplace}VIEW ${name} AS\n${body};`;
  }

  function buildDropView(): string {
    const ifExists = dropViewIfExists ? 'IF EXISTS ' : '';
    return `DROP VIEW ${ifExists}${dropViewName.trim() || 'view_name'};`;
  }

  function buildCreateDatabase(): string {
    const ifne = createDbIfNotExists ? 'IF NOT EXISTS ' : '';
    const name = createDbName.trim() || 'database_name';
    let sql = `CREATE DATABASE ${ifne}${name}`;
    if (createDbCharset.trim()) sql += `\n  CHARACTER SET ${createDbCharset.trim()}`;
    if (createDbCollation.trim()) sql += `\n  COLLATE ${createDbCollation.trim()}`;
    return sql + ';';
  }

  function buildDropDatabase(): string {
    const ifExists = dropDbIfExists ? 'IF EXISTS ' : '';
    return `DROP DATABASE ${ifExists}${dropDbName.trim() || 'database_name'};`;
  }

  function buildCreateUser(): string {
    const ifne = createUserIfNotExists ? 'IF NOT EXISTS ' : '';
    const user = `'${createUserName.trim() || 'username'}'@'${createUserHost.trim() || '%'}'`;
    let sql = `CREATE USER ${ifne}${user}`;
    if (createUserPassword.trim()) sql += `\n  IDENTIFIED BY '${createUserPassword.trim()}'`;
    return sql + ';';
  }

  function buildDropUser(): string {
    const ifExists = dropUserIfExists ? 'IF EXISTS ' : '';
    const user = `'${dropUserName.trim() || 'username'}'@'${dropUserHost.trim() || '%'}'`;
    return `DROP USER ${ifExists}${user};`;
  }

  function buildAlterUser(): string {
    const user = `'${alterUserName.trim() || 'username'}'@'${alterUserHost.trim() || '%'}'`;
    let sql = `ALTER USER ${user}`;
    if (alterUserPassword.trim()) sql += `\n  IDENTIFIED BY '${alterUserPassword.trim()}'`;
    return sql + ';';
  }

  function grantObjectStr(level: GrantLevel, db: string, tbl: string): string {
    if (level === 'global') return '*.*';
    if (level === 'database') return `${db.trim() || '*'}.*`;
    return `${db.trim() || '*'}.${tbl.trim() || '*'}`;
  }

  function buildGrant(): string {
    const privs = grantPrivileges.length > 0 ? grantPrivileges.join(', ') : 'SELECT';
    const obj = grantObjectStr(grantLevel, grantDatabase, grantTable);
    const user = `'${grantUser.trim() || 'username'}'@'${grantUserHost.trim() || '%'}'`;
    let sql = `GRANT ${privs}\n  ON ${obj}\n  TO ${user}`;
    if (grantWithGrantOption) sql += '\n  WITH GRANT OPTION';
    return sql + ';';
  }

  function buildRevoke(): string {
    const privs = revokePrivileges.length > 0 ? revokePrivileges.join(', ') : 'SELECT';
    const obj = grantObjectStr(revokeLevel, revokeDatabase, revokeTable);
    const user = `'${revokeUser.trim() || 'username'}'@'${revokeUserHost.trim() || '%'}'`;
    return `REVOKE ${privs}\n  ON ${obj}\n  FROM ${user};`;
  }

  function buildBegin(): string {
    if (beginIsolation) return `SET TRANSACTION ISOLATION LEVEL ${beginIsolation};\nSTART TRANSACTION;`;
    return 'START TRANSACTION;';
  }

  function buildRollback(): string {
    return rollbackToSavepoint.trim()
      ? `ROLLBACK TO SAVEPOINT ${rollbackToSavepoint.trim()};`
      : 'ROLLBACK;';
  }

  function buildSavepoint(): string {
    const name = savepointName.trim() || 'sp1';
    return savepointAction === 'release'
      ? `RELEASE SAVEPOINT ${name};`
      : `SAVEPOINT ${name};`;
  }

  const generatedSql = $derived.by((): string => {
    // Operations that don't need a table
    switch (operation) {
      case 'create_table':    return buildCreateTable();
      case 'create_view':     return buildCreateView();
      case 'drop_view':       return buildDropView();
      case 'create_database': return buildCreateDatabase();
      case 'drop_database':   return buildDropDatabase();
      case 'create_user':     return buildCreateUser();
      case 'drop_user':       return buildDropUser();
      case 'alter_user':      return buildAlterUser();
      case 'grant':           return buildGrant();
      case 'revoke':          return buildRevoke();
      case 'begin':           return buildBegin();
      case 'commit':          return 'COMMIT;';
      case 'rollback':        return buildRollback();
      case 'savepoint':       return buildSavepoint();
    }
    // Operations that need a table
    if (!selectedTable) return '-- Select a table above';
    switch (operation) {
      case 'select':      return buildSelect();
      case 'insert':      return buildInsertRows('INSERT INTO', insertCols);
      case 'replace':     return buildInsertRows('REPLACE INTO', replaceCols);
      case 'update':      return buildUpdate();
      case 'delete':      return buildDelete();
      case 'truncate':    return `TRUNCATE TABLE ${qualifiedTable};`;
      case 'alter_table': return buildAlterTable();
      case 'drop_table':  return `DROP TABLE ${qualifiedTable};`;
      case 'create_index':return buildCreateIndex();
      case 'drop_index':  return buildDropIndex();
    }
  });

  // ── UI helpers ────────────────────────────────────────────────────────────
  const OPERATION_GROUPS: { label: string; ops: { id: Operation; label: string }[] }[] = [
    {
      label: 'DML',
      ops: [
        { id: 'select',  label: 'SELECT' },
        { id: 'insert',  label: 'INSERT' },
        { id: 'replace', label: 'REPLACE' },
        { id: 'update',  label: 'UPDATE' },
        { id: 'delete',  label: 'DELETE' },
      ],
    },
    {
      label: 'DDL',
      ops: [
        { id: 'create_table',    label: 'CREATE TABLE' },
        { id: 'alter_table',     label: 'ALTER TABLE' },
        { id: 'drop_table',      label: 'DROP TABLE' },
        { id: 'truncate',        label: 'TRUNCATE' },
        { id: 'create_index',    label: 'CREATE INDEX' },
        { id: 'drop_index',      label: 'DROP INDEX' },
        { id: 'create_view',     label: 'CREATE VIEW' },
        { id: 'drop_view',       label: 'DROP VIEW' },
        { id: 'create_database', label: 'CREATE DATABASE' },
        { id: 'drop_database',   label: 'DROP DATABASE' },
      ],
    },
    {
      label: 'DCL',
      ops: [
        { id: 'create_user', label: 'CREATE USER' },
        { id: 'drop_user',   label: 'DROP USER' },
        { id: 'alter_user',  label: 'ALTER USER' },
        { id: 'grant',       label: 'GRANT' },
        { id: 'revoke',      label: 'REVOKE' },
      ],
    },
    {
      label: 'TCL',
      ops: [
        { id: 'begin',     label: 'BEGIN' },
        { id: 'commit',    label: 'COMMIT' },
        { id: 'rollback',  label: 'ROLLBACK' },
        { id: 'savepoint', label: 'SAVEPOINT' },
      ],
    },
  ];

  const ALTER_SUB_OPS: { id: AlterSubOp; label: string }[] = [
    { id: 'add_column',    label: 'Add column' },
    { id: 'drop_column',   label: 'Drop column' },
    { id: 'rename_column', label: 'Rename column' },
    { id: 'rename_table',  label: 'Rename table' },
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
  const SQL_PRIVILEGES = ['ALL PRIVILEGES', 'SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'DROP', 'ALTER', 'INDEX', 'REFERENCES', 'EXECUTE', 'TRIGGER', 'CREATE VIEW', 'SHOW VIEW', 'LOCK TABLES', 'CREATE ROUTINE', 'ALTER ROUTINE', 'SHOW DATABASES', 'PROCESS', 'SUPER', 'FILE', 'REPLICATION SLAVE', 'REPLICATION CLIENT'];
  const ISOLATION_LEVELS: { value: IsolationLevel; label: string }[] = [
    { value: '', label: '(default)' },
    { value: 'READ UNCOMMITTED', label: 'READ UNCOMMITTED' },
    { value: 'READ COMMITTED', label: 'READ COMMITTED' },
    { value: 'REPEATABLE READ', label: 'REPEATABLE READ' },
    { value: 'SERIALIZABLE', label: 'SERIALIZABLE' },
  ];

  function newWhereRow(cols: string[]): WhereRow {
    return { id: uid(), column: cols[0] ?? '', operator: '=', value: '', connector: 'AND' };
  }

  function valueInputHidden(op: WhereOp) {
    return op === 'IS NULL' || op === 'IS NOT NULL';
  }

  function togglePrivilege(list: string[], priv: string, checked: boolean): string[] {
    if (priv === 'ALL PRIVILEGES') return checked ? ['ALL PRIVILEGES'] : [];
    const without = list.filter(p => p !== priv && p !== 'ALL PRIVILEGES');
    return checked ? [...without, priv] : without;
  }
</script>

<Modal label="Query Builder" onbackdropclick={onclose}>
  <div class="qb-card" role="dialog" tabindex="-1" onkeydown={e => e.key === 'Escape' && onclose()}>

    <!-- Header -->
    <div class="qb-header">
      <span class="qb-title">Query Builder</span>
      <button class="qb-close" onclick={onclose} aria-label="Close">✕</button>
    </div>

    <!-- Operation picker (grouped) -->
    <div class="qb-ops-groups">
      {#each OPERATION_GROUPS as group}
        <div class="qb-ops-group">
          <span class="qb-ops-group-label">{group.label}</span>
          <div class="qb-ops-group-btns">
            {#each group.ops as op}
              <button
                class="qb-op-btn"
                class:qb-op-btn--active={operation === op.id}
                onclick={() => { operation = op.id; }}
              >{op.label}</button>
            {/each}
          </div>
        </div>
      {/each}
    </div>

    <div class="qb-body">

      <!-- Table selector (table-based operations only) -->
      {#if showTableSelector}
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
              <Select value={col.aggregate} options={AGGREGATES.map(a => ({ value: a.value, label: a.label }))} size="xs" disabled={!col.checked}
                onchange={v => { selectCols[i] = { ...selectCols[i], aggregate: v as Aggregate }; }} />
              <input class="qb-input qb-input--xs" type="text" placeholder="alias" value={col.alias} disabled={!col.checked}
                oninput={e => { selectCols[i] = { ...selectCols[i], alias: (e.currentTarget as HTMLInputElement).value }; }} />
            {/each}
          </div>
        </div>

        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">JOINs</span>
            <button class="qb-add-btn" onclick={() => { selectJoins = [...selectJoins, { id: uid(), type: 'INNER JOIN', table: '', onLeft: '', onRight: '' }]; }}>+ Add join</button>
          </div>
          {#each selectJoins as join, i}
            <div class="qb-row qb-join-row">
              <Select value={join.type} options={JOIN_TYPES.map(t => ({ value: t, label: t }))} size="sm"
                onchange={v => { selectJoins[i] = { ...selectJoins[i], type: v as JoinType }; }} />
              <Select value={join.table || undefined} options={allTableOptions} size="sm" searchable placeholder="table…"
                onchange={v => handleJoinTableSelect(join.id, v, i)} />
              <span class="qb-label-on">ON</span>
              <Select value={join.onLeft || undefined} options={colOptions} size="sm" searchable placeholder="left col" disabled={colOptions.length === 0}
                onchange={v => { selectJoins[i] = { ...selectJoins[i], onLeft: v }; }} />
              <span class="qb-eq">=</span>
              <Select value={join.onRight || undefined} options={joinColOptions(join.id)} size="sm" searchable placeholder="right col" disabled={!join.table || joinColOptions(join.id).length === 0}
                onchange={v => { selectJoins[i] = { ...selectJoins[i], onRight: v }; }} />
              <button class="qb-remove-btn" onclick={() => { selectJoins = selectJoins.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

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
                  placeholder={row.valueIsExpression ? 'NOW()' : row.operator === 'BETWEEN' ? 'a, b' : row.operator === 'IN' || row.operator === 'NOT IN' ? 'a, b, c' : 'value'}
                  value={row.value}
                  oninput={e => { selectWhere[i] = { ...selectWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
                <button class="qb-expr-toggle" class:qb-expr-toggle--active={row.valueIsExpression}
                  type="button"
                  title={row.valueIsExpression ? 'Expression mode: value inserted as-is' : 'String mode: value wrapped in quotes'}
                  onclick={() => { selectWhere[i] = { ...selectWhere[i], valueIsExpression: !row.valueIsExpression }; }}
                >{row.valueIsExpression ? 'f()' : "'…'"}</button>
              {:else}
                <span class="qb-input-spacer"></span>
              {/if}
              <button class="qb-remove-btn" onclick={() => { selectWhere = selectWhere.filter((_, j) => j !== i); }}>✕</button>
            </div>
          {/each}
        </div>

        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">GROUP BY</span>
          </div>
          <div class="qb-checkboxes">
            {#each columns as col}
              {@const checked = selectGroupBy.includes(col.name)}
              <label class="qb-inline-check">
                <Checkbox {checked} size="sm" onchange={c => {
                  selectGroupBy = c ? [...selectGroupBy, col.name] : selectGroupBy.filter(n => n !== col.name);
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
              <input class="qb-input" type="text" placeholder="value" value={col.value} disabled={!col.checked || col.useNull}
                oninput={e => { insertCols[i] = { ...insertCols[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              <Checkbox bind:checked={insertCols[i].useNull} size="sm" disabled={!col.checked} />
            {/each}
          </div>
        </div>

      <!-- ═══ REPLACE ═══════════════════════════════════════════════════════════ -->
      {:else if operation === 'replace' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Values</span>
            <button class="qb-link" onclick={() => { replaceCols = replaceCols.map(c => ({ ...c, checked: true })); }}>All</button>
            <button class="qb-link" onclick={() => { replaceCols = replaceCols.map(c => ({ ...c, checked: false })); }}>None</button>
          </div>
          <div class="qb-col-grid qb-col-grid--insert">
            <div class="qb-col-hdr"></div>
            <div class="qb-col-hdr">Column</div>
            <div class="qb-col-hdr">Type</div>
            <div class="qb-col-hdr">Value</div>
            <div class="qb-col-hdr">NULL</div>
            {#each replaceCols as col, i}
              <Checkbox bind:checked={replaceCols[i].checked} size="sm" />
              <span class="qb-col-name" class:qb-col-name--dim={!col.checked}>{col.name}</span>
              <span class="qb-col-type">{col.dataType}</span>
              <input class="qb-input" type="text" placeholder="value" value={col.value} disabled={!col.checked || col.useNull}
                oninput={e => { replaceCols[i] = { ...replaceCols[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
              <Checkbox bind:checked={replaceCols[i].useNull} size="sm" disabled={!col.checked} />
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
              <input class="qb-input qb-input--flex" type="text" placeholder="value" value={row.value} disabled={row.useNull}
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
                  placeholder={row.valueIsExpression ? 'NOW()' : row.operator === 'BETWEEN' ? 'a, b' : 'value'}
                  value={row.value}
                  oninput={e => { updateWhere[i] = { ...updateWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
                <button class="qb-expr-toggle" class:qb-expr-toggle--active={row.valueIsExpression}
                  type="button"
                  title={row.valueIsExpression ? 'Expression mode: value inserted as-is' : 'String mode: value wrapped in quotes'}
                  onclick={() => { updateWhere[i] = { ...updateWhere[i], valueIsExpression: !row.valueIsExpression }; }}
                >{row.valueIsExpression ? 'f()' : "'…'"}</button>
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
                  placeholder={row.valueIsExpression ? 'NOW()' : row.operator === 'BETWEEN' ? 'a, b' : 'value'}
                  value={row.value}
                  oninput={e => { deleteWhere[i] = { ...deleteWhere[i], value: (e.currentTarget as HTMLInputElement).value }; }} />
                <button class="qb-expr-toggle" class:qb-expr-toggle--active={row.valueIsExpression}
                  type="button"
                  title={row.valueIsExpression ? 'Expression mode: value inserted as-is' : 'String mode: value wrapped in quotes'}
                  onclick={() => { deleteWhere[i] = { ...deleteWhere[i], valueIsExpression: !row.valueIsExpression }; }}
                >{row.valueIsExpression ? 'f()' : "'…'"}</button>
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

      <!-- ═══ CREATE TABLE ════════════════════════════════════════════════════ -->
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

      <!-- ═══ ALTER TABLE ══════════════════════════════════════════════════════ -->
      {:else if operation === 'alter_table' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-label">Operation</div>
          <div class="qb-sub-ops">
            {#each ALTER_SUB_OPS as sub}
              <button class="qb-op-btn" class:qb-op-btn--active={alterSubOp === sub.id} onclick={() => { alterSubOp = sub.id; }}>{sub.label}</button>
            {/each}
          </div>
        </div>

        {#if alterSubOp === 'add_column'}
          <div class="qb-section">
            <div class="qb-section-label">New column</div>
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
              <input class="qb-input" type="text" placeholder="col_name" value={alterAddCol.name}
                oninput={e => { alterAddCol = { ...alterAddCol, name: (e.currentTarget as HTMLInputElement).value }; }} />
              <Select value={alterAddCol.type} options={SQL_TYPES.map(t => ({ value: t, label: t }))} size="xs"
                onchange={v => { alterAddCol = { ...alterAddCol, type: v }; }} />
              <input class="qb-input qb-input--xs" type="text" placeholder="255" value={alterAddCol.length}
                oninput={e => { alterAddCol = { ...alterAddCol, length: (e.currentTarget as HTMLInputElement).value }; }} />
              <Checkbox bind:checked={alterAddCol.nullable} size="sm" />
              <Checkbox bind:checked={alterAddCol.primaryKey} size="sm" />
              <Checkbox bind:checked={alterAddCol.autoIncrement} size="sm" />
              <Checkbox bind:checked={alterAddCol.unique} size="sm" />
              <input class="qb-input" type="text" placeholder="NULL" value={alterAddCol.defaultValue}
                oninput={e => { alterAddCol = { ...alterAddCol, defaultValue: (e.currentTarget as HTMLInputElement).value }; }} />
              <span></span>
            </div>
          </div>
        {:else if alterSubOp === 'drop_column'}
          <div class="qb-section">
            <div class="qb-section-label">Column to drop</div>
            <Select value={alterDropColName || undefined} options={colOptions} size="md" searchable placeholder="Choose a column…"
              onchange={v => { alterDropColName = v; }} />
            {#if alterDropColName}
              <div class="qb-warning" style="margin-top: var(--spacing-2)">
                Column <strong>{alterDropColName}</strong> and all its data will be permanently removed.
              </div>
            {/if}
          </div>
        {:else if alterSubOp === 'rename_column'}
          <div class="qb-section">
            <div class="qb-section-label">Rename column</div>
            <div class="qb-row">
              <Select value={alterRenameFrom || undefined} options={colOptions} size="sm" searchable placeholder="From column…"
                onchange={v => { alterRenameFrom = v; }} />
              <span class="qb-label-on">TO</span>
              <input class="qb-input qb-input--flex" type="text" placeholder="new_name" value={alterRenameTo}
                oninput={e => { alterRenameTo = (e.currentTarget as HTMLInputElement).value; }} />
            </div>
          </div>
        {:else if alterSubOp === 'rename_table'}
          <div class="qb-section">
            <div class="qb-section-label">New table name</div>
            <input class="qb-input qb-input--full" type="text" placeholder="new_table_name" value={alterRenameTableTo}
              oninput={e => { alterRenameTableTo = (e.currentTarget as HTMLInputElement).value; }} />
          </div>
        {/if}

      <!-- ═══ DROP TABLE ════════════════════════════════════════════════════════ -->
      {:else if operation === 'drop_table' && selectedTable && !columnsLoading}
        <div class="qb-warning qb-warning--lg">
          This will permanently drop <strong>{qualifiedTable}</strong> and all its data. This action cannot be undone.
        </div>

      <!-- ═══ TRUNCATE ══════════════════════════════════════════════════════════ -->
      {:else if operation === 'truncate' && selectedTable && !columnsLoading}
        <div class="qb-warning qb-warning--lg">
          This will delete all rows from <strong>{qualifiedTable}</strong>. This action cannot be undone.
        </div>

      <!-- ═══ CREATE INDEX ════════════════════════════════════════════════════ -->
      {:else if operation === 'create_index' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-label">Index name</div>
          <input class="qb-input qb-input--full" type="text" placeholder="idx_table_column" value={createIndexName}
            oninput={e => { createIndexName = (e.currentTarget as HTMLInputElement).value; }} />
        </div>
        <div class="qb-section">
          <label class="qb-inline-check">
            <Checkbox bind:checked={createIndexUnique} size="sm" />
            <span>UNIQUE</span>
          </label>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Columns</div>
          <div class="qb-checkboxes">
            {#each columns as col}
              {@const checked = createIndexCols.includes(col.name)}
              <label class="qb-inline-check">
                <Checkbox {checked} size="sm" onchange={c => {
                  createIndexCols = c ? [...createIndexCols, col.name] : createIndexCols.filter(n => n !== col.name);
                }} />
                <span>{col.name}</span>
              </label>
            {/each}
          </div>
        </div>

      <!-- ═══ DROP INDEX ═══════════════════════════════════════════════════════ -->
      {:else if operation === 'drop_index' && selectedTable && !columnsLoading}
        <div class="qb-section">
          <div class="qb-section-label">Index name</div>
          <input class="qb-input qb-input--full" type="text" placeholder="index_name" value={dropIndexName}
            oninput={e => { dropIndexName = (e.currentTarget as HTMLInputElement).value; }} />
        </div>

      <!-- ═══ CREATE VIEW ═══════════════════════════════════════════════════════ -->
      {:else if operation === 'create_view'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">View name</span>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="view_name" bind:value={createViewName} />
            <label class="qb-inline-check">
              <Checkbox bind:checked={createViewOrReplace} size="sm" />
              <span>OR REPLACE</span>
            </label>
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">SELECT body</div>
          <textarea class="qb-textarea" bind:value={createViewBody} rows="5" placeholder="SELECT * FROM table_name"></textarea>
        </div>

      <!-- ═══ DROP VIEW ════════════════════════════════════════════════════════ -->
      {:else if operation === 'drop_view'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">View name</span>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="view_name" value={dropViewName}
              oninput={e => { dropViewName = (e.currentTarget as HTMLInputElement).value; }} />
            <label class="qb-inline-check">
              <Checkbox bind:checked={dropViewIfExists} size="sm" />
              <span>IF EXISTS</span>
            </label>
          </div>
        </div>

      <!-- ═══ CREATE DATABASE ══════════════════════════════════════════════════ -->
      {:else if operation === 'create_database'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Database name</span>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="database_name" value={createDbName}
              oninput={e => { createDbName = (e.currentTarget as HTMLInputElement).value; }} />
            <label class="qb-inline-check">
              <Checkbox bind:checked={createDbIfNotExists} size="sm" />
              <span>IF NOT EXISTS</span>
            </label>
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Options (optional)</div>
          <div class="qb-row">
            <label class="qb-field">
              <span class="qb-field-label">Charset</span>
              <input class="qb-input" type="text" placeholder="utf8mb4" value={createDbCharset}
                oninput={e => { createDbCharset = (e.currentTarget as HTMLInputElement).value; }} />
            </label>
            <label class="qb-field">
              <span class="qb-field-label">Collation</span>
              <input class="qb-input" type="text" placeholder="utf8mb4_unicode_ci" value={createDbCollation}
                oninput={e => { createDbCollation = (e.currentTarget as HTMLInputElement).value; }} />
            </label>
          </div>
        </div>

      <!-- ═══ DROP DATABASE ════════════════════════════════════════════════════ -->
      {:else if operation === 'drop_database'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">Database name</span>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="database_name" value={dropDbName}
              oninput={e => { dropDbName = (e.currentTarget as HTMLInputElement).value; }} />
            <label class="qb-inline-check">
              <Checkbox bind:checked={dropDbIfExists} size="sm" />
              <span>IF EXISTS</span>
            </label>
          </div>
          {#if dropDbName}
            <div class="qb-warning" style="margin-top: var(--spacing-2)">
              Database <strong>{dropDbName}</strong> and all its tables will be permanently deleted.
            </div>
          {/if}
        </div>

      <!-- ═══ CREATE USER ═══════════════════════════════════════════════════════ -->
      {:else if operation === 'create_user'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">User</span>
            <label class="qb-inline-check">
              <Checkbox bind:checked={createUserIfNotExists} size="sm" />
              <span>IF NOT EXISTS</span>
            </label>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="username" value={createUserName}
              oninput={e => { createUserName = (e.currentTarget as HTMLInputElement).value; }} />
            <span class="qb-label-on">@</span>
            <input class="qb-input" type="text" placeholder="%" value={createUserHost}
              oninput={e => { createUserHost = (e.currentTarget as HTMLInputElement).value; }} />
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Password (optional)</div>
          <input class="qb-input qb-input--full" type="password" placeholder="password" value={createUserPassword}
            oninput={e => { createUserPassword = (e.currentTarget as HTMLInputElement).value; }} />
        </div>

      <!-- ═══ DROP USER ════════════════════════════════════════════════════════ -->
      {:else if operation === 'drop_user'}
        <div class="qb-section">
          <div class="qb-section-header">
            <span class="qb-section-label">User</span>
            <label class="qb-inline-check">
              <Checkbox bind:checked={dropUserIfExists} size="sm" />
              <span>IF EXISTS</span>
            </label>
          </div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="username" value={dropUserName}
              oninput={e => { dropUserName = (e.currentTarget as HTMLInputElement).value; }} />
            <span class="qb-label-on">@</span>
            <input class="qb-input" type="text" placeholder="%" value={dropUserHost}
              oninput={e => { dropUserHost = (e.currentTarget as HTMLInputElement).value; }} />
          </div>
        </div>

      <!-- ═══ ALTER USER ════════════════════════════════════════════════════════ -->
      {:else if operation === 'alter_user'}
        <div class="qb-section">
          <div class="qb-section-label">User</div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="username" value={alterUserName}
              oninput={e => { alterUserName = (e.currentTarget as HTMLInputElement).value; }} />
            <span class="qb-label-on">@</span>
            <input class="qb-input" type="text" placeholder="%" value={alterUserHost}
              oninput={e => { alterUserHost = (e.currentTarget as HTMLInputElement).value; }} />
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">New password</div>
          <input class="qb-input qb-input--full" type="password" placeholder="new_password" value={alterUserPassword}
            oninput={e => { alterUserPassword = (e.currentTarget as HTMLInputElement).value; }} />
        </div>

      <!-- ═══ GRANT ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'grant'}
        <div class="qb-section">
          <div class="qb-section-label">Privileges</div>
          <div class="qb-checkboxes">
            {#each SQL_PRIVILEGES as priv}
              {@const checked = grantPrivileges.includes(priv)}
              <label class="qb-inline-check">
                <Checkbox {checked} size="sm" onchange={c => { grantPrivileges = togglePrivilege(grantPrivileges, priv, c); }} />
                <span>{priv}</span>
              </label>
            {/each}
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Grant level</div>
          <div class="qb-sub-ops">
            {#each [['global','Global (*.*)', 'global'], ['database','Database (db.*)', 'database'], ['table','Table (db.table)', 'table']] as [id, label]}
              <button class="qb-op-btn" class:qb-op-btn--active={grantLevel === id}
                onclick={() => { grantLevel = id as GrantLevel; }}>{label}</button>
            {/each}
          </div>
          {#if grantLevel !== 'global'}
            <div class="qb-row" style="margin-top: var(--spacing-2)">
              <label class="qb-field">
                <span class="qb-field-label">Database</span>
                <input class="qb-input" type="text" placeholder="db_name" value={grantDatabase}
                  oninput={e => { grantDatabase = (e.currentTarget as HTMLInputElement).value; }} />
              </label>
              {#if grantLevel === 'table'}
                <label class="qb-field">
                  <span class="qb-field-label">Table</span>
                  <input class="qb-input" type="text" placeholder="table_name" value={grantTable}
                    oninput={e => { grantTable = (e.currentTarget as HTMLInputElement).value; }} />
                </label>
              {/if}
            </div>
          {/if}
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Grant to</div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="username" value={grantUser}
              oninput={e => { grantUser = (e.currentTarget as HTMLInputElement).value; }} />
            <span class="qb-label-on">@</span>
            <input class="qb-input" type="text" placeholder="%" value={grantUserHost}
              oninput={e => { grantUserHost = (e.currentTarget as HTMLInputElement).value; }} />
            <label class="qb-inline-check">
              <Checkbox bind:checked={grantWithGrantOption} size="sm" />
              <span>WITH GRANT OPTION</span>
            </label>
          </div>
        </div>

      <!-- ═══ REVOKE ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'revoke'}
        <div class="qb-section">
          <div class="qb-section-label">Privileges</div>
          <div class="qb-checkboxes">
            {#each SQL_PRIVILEGES as priv}
              {@const checked = revokePrivileges.includes(priv)}
              <label class="qb-inline-check">
                <Checkbox {checked} size="sm" onchange={c => { revokePrivileges = togglePrivilege(revokePrivileges, priv, c); }} />
                <span>{priv}</span>
              </label>
            {/each}
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Grant level</div>
          <div class="qb-sub-ops">
            {#each [['global','Global (*.*)', 'global'], ['database','Database (db.*)', 'database'], ['table','Table (db.table)', 'table']] as [id, label]}
              <button class="qb-op-btn" class:qb-op-btn--active={revokeLevel === id}
                onclick={() => { revokeLevel = id as GrantLevel; }}>{label}</button>
            {/each}
          </div>
          {#if revokeLevel !== 'global'}
            <div class="qb-row" style="margin-top: var(--spacing-2)">
              <label class="qb-field">
                <span class="qb-field-label">Database</span>
                <input class="qb-input" type="text" placeholder="db_name" value={revokeDatabase}
                  oninput={e => { revokeDatabase = (e.currentTarget as HTMLInputElement).value; }} />
              </label>
              {#if revokeLevel === 'table'}
                <label class="qb-field">
                  <span class="qb-field-label">Table</span>
                  <input class="qb-input" type="text" placeholder="table_name" value={revokeTable}
                    oninput={e => { revokeTable = (e.currentTarget as HTMLInputElement).value; }} />
                </label>
              {/if}
            </div>
          {/if}
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Revoke from</div>
          <div class="qb-row">
            <input class="qb-input qb-input--flex" type="text" placeholder="username" value={revokeUser}
              oninput={e => { revokeUser = (e.currentTarget as HTMLInputElement).value; }} />
            <span class="qb-label-on">@</span>
            <input class="qb-input" type="text" placeholder="%" value={revokeUserHost}
              oninput={e => { revokeUserHost = (e.currentTarget as HTMLInputElement).value; }} />
          </div>
        </div>

      <!-- ═══ BEGIN ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'begin'}
        <div class="qb-section">
          <div class="qb-section-label">Isolation level (optional)</div>
          <Select value={beginIsolation} options={ISOLATION_LEVELS} size="md"
            onchange={v => { beginIsolation = v as IsolationLevel; }} />
        </div>

      <!-- ═══ COMMIT ════════════════════════════════════════════════════════════ -->
      {:else if operation === 'commit'}
        <div class="qb-info">Commits the current transaction.</div>

      <!-- ═══ ROLLBACK ══════════════════════════════════════════════════════════ -->
      {:else if operation === 'rollback'}
        <div class="qb-section">
          <div class="qb-section-label">Rollback to savepoint (optional)</div>
          <input class="qb-input qb-input--full" type="text" placeholder="savepoint_name — leave empty for full rollback" value={rollbackToSavepoint}
            oninput={e => { rollbackToSavepoint = (e.currentTarget as HTMLInputElement).value; }} />
        </div>

      <!-- ═══ SAVEPOINT ═════════════════════════════════════════════════════════ -->
      {:else if operation === 'savepoint'}
        <div class="qb-section">
          <div class="qb-section-label">Action</div>
          <div class="qb-sub-ops">
            <button class="qb-op-btn" class:qb-op-btn--active={savepointAction === 'create'} onclick={() => { savepointAction = 'create'; }}>SAVEPOINT</button>
            <button class="qb-op-btn" class:qb-op-btn--active={savepointAction === 'release'} onclick={() => { savepointAction = 'release'; }}>RELEASE SAVEPOINT</button>
          </div>
        </div>
        <div class="qb-section">
          <div class="qb-section-label">Savepoint name</div>
          <input class="qb-input qb-input--full" type="text" placeholder="sp1" value={savepointName}
            oninput={e => { savepointName = (e.currentTarget as HTMLInputElement).value; }} />
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
        disabled={showTableSelector && !selectedTable}
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
    width: min(900px, 96vw);
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

  /* ── Operation groups ───────────────────────────────────────────────────── */

  .qb-ops-groups {
    display: flex;
    gap: 0;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    overflow-x: auto;
  }

  .qb-ops-group {
    display: flex;
    flex-direction: column;
    padding: var(--spacing-2) var(--spacing-3);
    border-right: 1px solid var(--color-border);
    min-width: 0;
  }

  .qb-ops-group:last-child { border-right: none; }

  .qb-ops-group-label {
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.1em;
    margin-bottom: 4px;
    padding-left: 2px;
  }

  .qb-ops-group-btns {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
  }

  .qb-op-btn {
    padding: 3px 8px;
    border-radius: var(--radius-md);
    border: 1px solid transparent;
    background: transparent;
    color: var(--color-text-secondary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    cursor: pointer;
    transition: all var(--transition-fast);
    letter-spacing: 0.03em;
    white-space: nowrap;
  }

  .qb-op-btn:hover { background: var(--color-bg-hover); color: var(--color-text-primary); }

  .qb-op-btn--active {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  /* ── Sub-ops (inline within body) ──────────────────────────────────────── */

  .qb-sub-ops {
    display: flex;
    flex-wrap: wrap;
    gap: 2px;
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

  .qb-section-header .qb-section-label { margin-bottom: 0; }

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

  .qb-input--xs   { height: 22px; font-size: var(--font-size-xs); padding: 0 5px; width: 60px; }
  .qb-input--flex { flex: 1; min-width: 80px; }
  .qb-input--full { width: 100%; }

  .qb-textarea {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    outline: none;
    resize: vertical;
    box-sizing: border-box;
    line-height: 1.5;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .qb-textarea:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

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

  /* ── Warning / info ─────────────────────────────────────────────────────── */

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

  .qb-info {
    padding: var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    text-align: center;
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

  .qb-expr-toggle {
    flex-shrink: 0;
    height: 28px;
    padding: 0 6px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    font-size: 10px;
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast);
  }

  .qb-expr-toggle:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text-secondary);
  }

  .qb-expr-toggle--active {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
    color: var(--color-accent);
  }

  .qb-expr-toggle--active:hover {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
  }
</style>
