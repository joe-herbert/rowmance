<!--
  TableStructure — panel showing columns, indexes, and foreign keys for a table,
  with inline editing of structure (add/modify/drop columns, indexes, and foreign keys).
-->
<script lang="ts">
  import * as schemaApi from '$lib/tauri/schema';
  import type {
    ColumnInfo,
    IndexInfo,
    ForeignKeyInfo,
    ColumnRef,
    VirtualRelation,
    PolymorphicVirtualRelation,
  } from '$lib/types';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useVirtualRelations } from '$lib/stores/virtualRelations.svelte';
  import { errorMessage } from '$lib/utils/errors';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import VirtualRelationModal from '$lib/components/relations/VirtualRelationModal.svelte';
  import PolymorphicVirtualRelationModal from '$lib/components/relations/PolymorphicVirtualRelationModal.svelte';
  import Loader from '$lib/components/ui/Loader.svelte';
  import SqlHighlight from '$lib/components/ui/SqlHighlight.svelte';
  import { useTabDrag } from '$lib/stores/tabDragState.svelte';
  import DragHandleIcon from '$lib/components/icons/DragHandleIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';
  import PencilSmIcon from '$lib/components/icons/PencilSmIcon.svelte';
  import TrashSmIcon from '$lib/components/icons/TrashSmIcon.svelte';
  import LinkSmIcon from '$lib/components/icons/LinkSmIcon.svelte';

  interface Props {
    connectionId: string;
    database: string;
    table: string;
    itemId?: string;
    splitId?: string;
  }

  const { connectionId, database, table, itemId = '', splitId = '' }: Props = $props();

  const connections = useConnections();
  const vrStore = useVirtualRelations();
  const tabDrag = useTabDrag();

  let labelDragActive = $state(false);
  let labelDragStartX = 0;
  let labelDragStartY = 0;

  $effect(() => {
    if (!labelDragActive) return;
    function onMove(e: PointerEvent) {
      if (
        !tabDrag.isDragging &&
        (Math.abs(e.clientX - labelDragStartX) > 4 || Math.abs(e.clientY - labelDragStartY) > 4)
      ) {
        if (itemId && splitId) {
          tabDrag.start(itemId, splitId);
        } else {
          tabDrag.startContent({ kind: 'table_structure', connectionId, database, table });
        }
      }
    }
    function onUp() {
      labelDragActive = false;
      if (tabDrag.isDragging) tabDrag.end();
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onLabelPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    labelDragStartX = e.clientX;
    labelDragStartY = e.clientY;
    labelDragActive = true;
    (e.target as HTMLElement).releasePointerCapture(e.pointerId);
  }
  const profile = $derived(connections.getById(connectionId));
  const dbType = $derived(profile?.dbType ?? 'mysql');
  const isReadOnly = $derived(profile?.readOnly ?? false);
  const isMysql = $derived(dbType === 'mysql' || dbType === 'mariadb');
  const isPostgres = $derived(dbType === 'postgres');
  const isSqlite = $derived(dbType === 'sqlite');
  const isSqlServer = $derived(dbType === 'sqlserver');

  // ── Data state ─────────────────────────────────────────────────────────────

  let isLoading = $state(true);
  let loadError = $state<string | null>(null);
  let columns = $state<ColumnInfo[]>([]);
  let indexes = $state<IndexInfo[]>([]);
  let foreignKeys = $state<ForeignKeyInfo[]>([]);

  const uniqueColumns = $derived(
    new Set(
      indexes.filter((idx) => idx.unique && idx.columns.length === 1).map((idx) => idx.columns[0]),
    ),
  );

  function loadData() {
    isLoading = true;
    loadError = null;
    Promise.all([
      schemaApi.listColumns(connectionId, database, table),
      schemaApi.listIndexes(connectionId, database, table),
      schemaApi.listForeignKeys(connectionId, database, table),
    ])
      .then(([cols, idxs, fks]) => {
        columns = cols;
        indexes = idxs;
        foreignKeys = fks;
        isLoading = false;
      })
      .catch((err) => {
        loadError = errorMessage(err);
        isLoading = false;
      });
  }

  $effect(() => {
    editMode = false;
    loadData();
  });

  // ── Edit state ─────────────────────────────────────────────────────────────

  let editMode = $state(false);

  $effect(() => {
    if (isReadOnly) editMode = false;
  });
  let isSaving = $state(false);
  let saveError = $state<string | null>(null);

  // Name, Type, Key, Null, Uniq, Default, [Actions], [Drag handle]
  const colTableColCount = $derived(6 + (editMode ? 1 : 0) + (editMode && isMysql ? 1 : 0));

  interface ColForm {
    mode: 'add' | 'edit';
    original: ColumnInfo | null;
    name: string;
    dataType: string;
    nullable: boolean;
    defaultValue: string;
    autoIncrement: boolean;
    comment: string;
  }
  let columnForm = $state<ColForm | null>(null);

  interface IdxForm {
    name: string;
    selectedColumns: string[];
    unique: boolean;
    isPrimary: boolean;
  }
  let indexForm = $state<IdxForm | null>(null);

  interface FkForm {
    constraintName: string;
    selectedColumns: string[];
    referencedTable: string;
    referencedColumns: string;
    onDelete: string;
    onUpdate: string;
  }
  let fkForm = $state<FkForm | null>(null);
  let fkRefTableOptions = $state<{ value: string; label: string }[]>([]);
  let fkRefColumnOptions = $state<{ value: string; label: string }[]>([]);

  async function loadFkRefTables() {
    try {
      const tables = await schemaApi.listTables(connectionId, database);
      fkRefTableOptions = tables.map((t) => ({ value: t.name, label: t.name }));
    } catch {
      fkRefTableOptions = [];
    }
  }

  async function loadFkRefColumns(tableName: string) {
    if (!tableName) {
      fkRefColumnOptions = [];
      return;
    }
    try {
      const cols = await schemaApi.listColumns(connectionId, database, tableName);
      fkRefColumnOptions = cols.map((c) => ({ value: c.name, label: c.name }));
    } catch {
      fkRefColumnOptions = [];
    }
  }

  interface ConfirmDrop {
    label: string;
    sqls: string[];
  }
  let confirmDrop = $state<ConfirmDrop | null>(null);

  // ── SQL helpers ────────────────────────────────────────────────────────────

  function qi(name: string): string {
    if (isMysql) return '`' + name.replace(/`/g, '``') + '`';
    if (isSqlServer) return '[' + name.replace(/\]/g, ']]') + ']';
    return '"' + name.replace(/"/g, '""') + '"';
  }

  function tRef(): string {
    if (isSqlite) return qi(table);
    return qi(database) + '.' + qi(table);
  }

  function escStr(s: string): string {
    return "'" + s.replace(/'/g, "''") + "'";
  }

  async function execSqls(sqls: string[]) {
    for (const sql of sqls) {
      await schemaApi.executeDdl(connectionId, sql);
    }
  }

  // ── Column SQL ─────────────────────────────────────────────────────────────

  function colDef(form: ColForm): string {
    let s = `${qi(form.name)} ${form.dataType}`;
    if (!form.nullable) s += ' NOT NULL';
    if (isMysql && form.autoIncrement) s += ' AUTO_INCREMENT';
    if (form.defaultValue.trim()) s += ` DEFAULT ${form.defaultValue.trim()}`;
    if (isMysql && form.comment.trim()) s += ` COMMENT ${escStr(form.comment.trim())}`;
    return s;
  }

  function buildAddColSqls(form: ColForm): string[] {
    return [`ALTER TABLE ${tRef()} ADD COLUMN ${colDef(form)}`];
  }

  function buildEditColSqls(orig: ColumnInfo, form: ColForm): string[] {
    if (isMysql) {
      return [`ALTER TABLE ${tRef()} CHANGE COLUMN ${qi(orig.name)} ${colDef(form)}`];
    }
    if (isPostgres) {
      const stmts: string[] = [];
      const t = tRef();
      const oq = qi(orig.name);
      if (form.dataType !== orig.dataType) {
        stmts.push(`ALTER TABLE ${t} ALTER COLUMN ${oq} TYPE ${form.dataType}`);
      }
      if (form.nullable !== orig.nullable) {
        stmts.push(
          `ALTER TABLE ${t} ALTER COLUMN ${oq} ${form.nullable ? 'DROP NOT NULL' : 'SET NOT NULL'}`,
        );
      }
      const origDef = orig.defaultValue ?? '';
      const newDef = form.defaultValue.trim();
      if (newDef !== origDef) {
        stmts.push(
          newDef
            ? `ALTER TABLE ${t} ALTER COLUMN ${oq} SET DEFAULT ${newDef}`
            : `ALTER TABLE ${t} ALTER COLUMN ${oq} DROP DEFAULT`,
        );
      }
      if (form.name !== orig.name) {
        stmts.push(`ALTER TABLE ${t} RENAME COLUMN ${oq} TO ${qi(form.name)}`);
      }
      return stmts;
    }
    if (isSqlite && form.name !== orig.name) {
      return [`ALTER TABLE ${tRef()} RENAME COLUMN ${qi(orig.name)} TO ${qi(form.name)}`];
    }
    return [];
  }

  function buildDropColSql(name: string): string {
    return `ALTER TABLE ${tRef()} DROP COLUMN ${qi(name)}`;
  }

  // ── Index SQL ──────────────────────────────────────────────────────────────

  function buildAddIdxSql(form: IdxForm): string {
    const cols = form.selectedColumns.map(qi).join(', ');
    if (form.isPrimary) {
      return `ALTER TABLE ${tRef()} ADD PRIMARY KEY (${cols})`;
    }
    const name = form.name.trim() || `idx_${table}_${form.selectedColumns.join('_')}`;
    return `CREATE ${form.unique ? 'UNIQUE ' : ''}INDEX ${qi(name)} ON ${tRef()} (${cols})`;
  }

  function buildDropIdxSql(name: string): string {
    if (name === 'PRIMARY' && isMysql) return `ALTER TABLE ${tRef()} DROP PRIMARY KEY`;
    if (isMysql) return `DROP INDEX ${qi(name)} ON ${tRef()}`;
    if (isPostgres) return `DROP INDEX ${qi(database)}.${qi(name)}`;
    if (isSqlServer) return `DROP INDEX ${qi(name)} ON ${tRef()}`;
    return `DROP INDEX ${qi(name)}`;
  }

  // ── FK SQL ─────────────────────────────────────────────────────────────────

  function buildAddFkSql(form: FkForm): string {
    const local = form.selectedColumns.map(qi).join(', ');
    const refCols = form.referencedColumns
      .split(',')
      .map((s) => qi(s.trim()))
      .join(', ');
    const name = form.constraintName.trim() || `fk_${table}_${form.selectedColumns.join('_')}`;
    return `ALTER TABLE ${tRef()} ADD CONSTRAINT ${qi(name)} FOREIGN KEY (${local}) REFERENCES ${qi(form.referencedTable.trim())} (${refCols}) ON DELETE ${form.onDelete} ON UPDATE ${form.onUpdate}`;
  }

  function buildDropFkSql(name: string): string {
    if (isMysql) return `ALTER TABLE ${tRef()} DROP FOREIGN KEY ${qi(name)}`;
    return `ALTER TABLE ${tRef()} DROP CONSTRAINT ${qi(name)}`;
  }

  // ── Handlers ───────────────────────────────────────────────────────────────

  function openAddCol() {
    columnForm = {
      mode: 'add',
      original: null,
      name: '',
      dataType: isMysql ? 'VARCHAR(255)' : 'TEXT',
      nullable: true,
      defaultValue: '',
      autoIncrement: false,
      comment: '',
    };
    saveError = null;
  }

  function openEditCol(col: ColumnInfo) {
    columnForm = {
      mode: 'edit',
      original: col,
      name: col.name,
      dataType: col.dataType,
      nullable: col.nullable,
      defaultValue: col.defaultValue ?? '',
      autoIncrement: col.isAutoIncrement,
      comment: col.comment ?? '',
    };
    saveError = null;
  }

  async function submitCol() {
    if (!columnForm) return;
    if (!columnForm.name.trim()) {
      saveError = 'Column name is required';
      return;
    }
    if (!columnForm.dataType.trim()) {
      saveError = 'Data type is required';
      return;
    }
    isSaving = true;
    saveError = null;
    try {
      const sqls =
        columnForm.mode === 'add'
          ? buildAddColSqls(columnForm)
          : buildEditColSqls(columnForm.original!, columnForm);
      if (sqls.length > 0) await execSqls(sqls);
      columnForm = null;
      loadData();
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      isSaving = false;
    }
  }

  function openAddIdx() {
    indexForm = { name: '', selectedColumns: [], unique: false, isPrimary: false };
    saveError = null;
  }

  async function submitIdx() {
    if (!indexForm) return;
    if (!indexForm.selectedColumns.length) {
      saveError = 'Select at least one column';
      return;
    }
    isSaving = true;
    saveError = null;
    try {
      await execSqls([buildAddIdxSql(indexForm)]);
      indexForm = null;
      loadData();
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      isSaving = false;
    }
  }

  function openAddFk() {
    fkForm = {
      constraintName: '',
      selectedColumns: [],
      referencedTable: '',
      referencedColumns: '',
      onDelete: 'NO ACTION',
      onUpdate: 'NO ACTION',
    };
    fkRefTableOptions = [];
    fkRefColumnOptions = [];
    saveError = null;
    loadFkRefTables();
  }

  async function submitFk() {
    if (!fkForm) return;
    if (!fkForm.selectedColumns.length) {
      saveError = 'Select at least one column';
      return;
    }
    if (!fkForm.referencedTable.trim()) {
      saveError = 'Referenced table is required';
      return;
    }
    if (!fkForm.referencedColumns.trim()) {
      saveError = 'Referenced columns are required';
      return;
    }
    isSaving = true;
    saveError = null;
    try {
      await execSqls([buildAddFkSql(fkForm)]);
      fkForm = null;
      loadData();
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      isSaving = false;
    }
  }

  function requestDrop(label: string, sqls: string[]) {
    confirmDrop = { label, sqls };
    saveError = null;
  }

  async function execConfirm() {
    if (!confirmDrop) return;
    isSaving = true;
    saveError = null;
    try {
      await execSqls(confirmDrop.sqls);
      confirmDrop = null;
      loadData();
    } catch (err) {
      saveError = errorMessage(err);
      confirmDrop = null;
    } finally {
      isSaving = false;
    }
  }

  function toggleColSel(col: string, arr: string[]): string[] {
    return arr.includes(col) ? arr.filter((c) => c !== col) : [...arr, col];
  }

  // ── Column reorder (MySQL/MariaDB only) ────────────────────────────────────

  let colDragFromIdx = $state<number | null>(null);
  let colIsDragging = $state(false);
  let colDragDropTarget = $state<{ index: number; position: 'before' | 'after' } | null>(null);
  let colDragPointerStartY = 0;

  const colInsertAt = $derived.by(() => {
    if (!colDragDropTarget || !colIsDragging) return null;
    return colDragDropTarget.position === 'after'
      ? colDragDropTarget.index + 1
      : colDragDropTarget.index;
  });

  $effect(() => {
    if (colDragFromIdx === null) return;
    function onMove(e: PointerEvent) {
      if (!colIsDragging && Math.abs(e.clientY - colDragPointerStartY) > 4) {
        colIsDragging = true;
      }
      if (!colIsDragging) return;
      const el = document.elementFromPoint(e.clientX, e.clientY);
      const row = el?.closest<HTMLElement>('[data-col-drag-idx]');
      const targetStr = row?.dataset.colDragIdx;
      if (targetStr === undefined) {
        colDragDropTarget = null;
        return;
      }
      const targetIndex = parseInt(targetStr, 10);
      if (targetIndex === colDragFromIdx) {
        colDragDropTarget = null;
        return;
      }
      const rect = row!.getBoundingClientRect();
      const position = e.clientY < rect.top + rect.height / 2 ? 'before' : 'after';
      colDragDropTarget = { index: targetIndex, position };
    }
    function onUp() {
      if (colIsDragging && colDragDropTarget !== null && colDragFromIdx !== null) {
        const from = colDragFromIdx;
        const { index: toIndex, position } = colDragDropTarget;
        let toIdx = position === 'after' ? toIndex + 1 : toIndex;
        if (from < toIndex) toIdx -= 1;
        if (toIdx !== from) {
          const newCols = [...columns];
          const [moved] = newCols.splice(from, 1);
          newCols.splice(toIdx, 0, moved);
          applyColReorder(newCols, moved);
        }
      }
      colDragFromIdx = null;
      colIsDragging = false;
      colDragDropTarget = null;
    }
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
    return () => {
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
  });

  function onColDragHandlePointerDown(e: PointerEvent, index: number) {
    if (e.button !== 0) return;
    colDragPointerStartY = e.clientY;
    colDragFromIdx = index;
    e.preventDefault();
  }

  function colDefFromInfo(col: ColumnInfo): string {
    let s = `${qi(col.name)} ${col.dataType}`;
    if (!col.nullable) s += ' NOT NULL';
    if (isMysql && col.isAutoIncrement) s += ' AUTO_INCREMENT';
    if (col.defaultValue !== null && col.defaultValue !== '') s += ` DEFAULT ${col.defaultValue}`;
    if (isMysql && col.comment) s += ` COMMENT ${escStr(col.comment)}`;
    return s;
  }

  async function applyColReorder(newCols: ColumnInfo[], movedCol: ColumnInfo) {
    const movedIdx = newCols.indexOf(movedCol);
    const sql =
      movedIdx === 0
        ? `ALTER TABLE ${tRef()} MODIFY COLUMN ${colDefFromInfo(movedCol)} FIRST`
        : `ALTER TABLE ${tRef()} MODIFY COLUMN ${colDefFromInfo(movedCol)} AFTER ${qi(newCols[movedIdx - 1].name)}`;
    columns = newCols;
    isSaving = true;
    saveError = null;
    try {
      await execSqls([sql]);
      loadData();
    } catch (err) {
      saveError = errorMessage(err);
      loadData();
    } finally {
      isSaving = false;
    }
  }

  // ── Virtual Relations ──────────────────────────────────────────────────────

  interface VrModal {
    from: ColumnRef;
    editId?: string;
    initialTo?: ColumnRef;
    initialLabel?: string;
  }
  let vrModal = $state<VrModal | null>(null);

  const tableVirtualRelations = $derived(
    vrStore.relations.filter(
      (vr) =>
        (vr.from.connectionId === connectionId &&
          vr.from.database === database &&
          vr.from.table === table) ||
        (vr.to.connectionId === connectionId &&
          vr.to.database === database &&
          vr.to.table === table),
    ),
  );

  let vrColPickModal = $state<{ selectedColumn: string } | null>(null);

  function openAddVrFromHeader() {
    if (!columns.length) return;
    vrColPickModal = { selectedColumn: columns[0].name };
  }

  function submitVrColPick() {
    if (!vrColPickModal) return;
    const col = vrColPickModal.selectedColumn;
    vrColPickModal = null;
    openAddVr(col);
  }

  function openAddVr(colName: string) {
    vrModal = { from: { connectionId, database, table, column: colName } };
  }

  function openEditVr(vr: VirtualRelation) {
    const isFrom =
      vr.from.connectionId === connectionId &&
      vr.from.database === database &&
      vr.from.table === table;
    vrModal = isFrom
      ? { from: vr.from, editId: vr.id, initialTo: vr.to, initialLabel: vr.label }
      : { from: vr.to, editId: vr.id, initialTo: vr.from, initialLabel: vr.label };
  }

  function connName(connId: string): string {
    return connections.getById(connId)?.name ?? connId;
  }

  // ── Polymorphic Virtual Relations ─────────────────────────────────────────

  const tablePolymorphicRelations = $derived(
    vrStore.polymorphicRelations.filter(
      (pvr) =>
        pvr.connectionId === connectionId &&
        pvr.database === database &&
        pvr.table === table,
    ),
  );

  let pvrEditModal = $state<PolymorphicVirtualRelation | null>(null);
  let pvrCreateModal = $state(false);

  const refActions = [
    { value: 'NO ACTION', label: 'NO ACTION' },
    { value: 'RESTRICT', label: 'RESTRICT' },
    { value: 'CASCADE', label: 'CASCADE' },
    { value: 'SET NULL', label: 'SET NULL' },
    { value: 'SET DEFAULT', label: 'SET DEFAULT' },
  ];
</script>

<div class="structure-viewer">
  <!-- ── Toolbar ─────────────────────────────────────────────────────────── -->
  <div class="toolbar">
    <span
      class="object-label"
      title="Drag to open in another split"
      onpointerdown={onLabelPointerDown}
    >
      <span class="object-type">table</span>
      <span class="object-type-sep">/</span>
      <span class="object-path">{database}.{table}</span>
    </span>
    {#if !isLoading && !loadError}
      {#if !isReadOnly}
        <button
          class="edit-toggle"
          class:edit-toggle--active={editMode}
          onclick={() => {
            editMode = !editMode;
            saveError = null;
          }}
        >
          {editMode ? 'Done' : 'Edit'}
        </button>
      {/if}
    {/if}
  </div>

  <!-- ── Content ─────────────────────────────────────────────────────────── -->
  <div class="content">
    {#if isLoading}
      <div class="state-overlay">
        <Loader />
      </div>
    {:else if loadError}
      <div class="state-overlay state-overlay--error">
        <span class="error-text">{loadError}</span>
      </div>
    {:else}
      <div class="sections">
        <!-- Columns ──────────────────────────────────────────────────────── -->
        <section class="section">
          <div class="section-header section-header--flex">
            <span>Columns ({columns.length})</span>
            {#if editMode}
              <button class="add-btn" onclick={openAddCol}>+ Add Column</button>
            {/if}
          </div>
          <table
            class="col-table"
            class:col-table--editing={editMode}
            class:col-table--dragging={colIsDragging}
          >
            <thead>
              <tr>
                {#if editMode && isMysql}<th class="th-drag"></th>{/if}
                <th>Name</th>
                <th>Type</th>
                <th class="th-narrow">Key</th>
                <th class="th-narrow" title="Nullable">Null</th>
                <th class="th-narrow" title="Unique">Uniq</th>
                <th>Default</th>
                {#if editMode}<th class="th-actions">Actions</th>{/if}
              </tr>
            </thead>
            <tbody>
              {#each columns as col, i (col.name)}
                {#if colInsertAt === i}
                  <tr class="col-drop-line" aria-hidden="true"
                    ><td colspan={colTableColCount}></td></tr
                  >
                {/if}
                <tr
                  class:pk-row={col.isPrimaryKey}
                  class:col-row-dragging={colDragFromIdx === i && colIsDragging}
                  data-col-drag-idx={i}
                >
                  {#if editMode && isMysql}
                    <td class="col-drag-cell">
                      <span
                        class="col-drag-handle"
                        aria-hidden="true"
                        onpointerdown={(e) => onColDragHandlePointerDown(e, i)}
                      >
                        <DragHandleIcon />
                      </span>
                    </td>
                  {/if}
                  <td class="col-name mono">{col.name}</td>
                  <td class="col-type mono">{col.dataType}</td>
                  <td class="col-keys">
                    {#if col.isPrimaryKey}<span class="badge badge--pk" title="Primary Key">PK</span
                      >{/if}
                    {#if col.isAutoIncrement}<span class="badge badge--ai" title="Auto Increment"
                        >AI</span
                      >{/if}
                    {#if col.isForeignKey}<span class="badge badge--fk" title="Foreign Key">FK</span
                      >{/if}
                  </td>
                  <td class="col-null center-cell"
                    >{#if col.nullable}<CheckIcon width={12} height={12} strokeWidth={2.5} />{/if}</td
                  >
                  <td class="col-unique center-cell">
                    {#if uniqueColumns.has(col.name) || col.isPrimaryKey}
                      <CheckIcon width={12} height={12} strokeWidth={2.5} />
                    {/if}
                  </td>
                  <td class="col-default mono">{col.defaultValue ?? ''}</td>
                  {#if editMode}
                    <td class="col-actions">
                      <div class="row-actions">
                        <button
                          class="act-btn"
                          title="Edit column"
                          onclick={() => openEditCol(col)}
                        >
                          <PencilSmIcon />
                        </button>
                        <button
                          class="act-btn act-btn--danger"
                          title="Drop column"
                          onclick={() =>
                            requestDrop(`Drop column "${col.name}"?`, [buildDropColSql(col.name)])}
                        >
                          <TrashSmIcon />
                        </button>
                        <button
                          class="act-btn act-btn--connect"
                          title="Add virtual relation"
                          onclick={() => openAddVr(col.name)}
                        >
                          <LinkSmIcon />
                        </button>
                      </div>
                    </td>
                  {/if}
                </tr>
                {#if col.comment}
                  <tr class="comment-row">
                    <td colspan={colTableColCount} class="col-comment">{col.comment}</td>
                  </tr>
                {/if}
              {/each}
              {#if colInsertAt === columns.length}
                <tr class="col-drop-line" aria-hidden="true"
                  ><td colspan={colTableColCount}></td></tr
                >
              {/if}
            </tbody>
          </table>
        </section>

        <!-- Indexes ──────────────────────────────────────────────────────── -->
        {#if indexes.length > 0 || editMode}
          <section class="section">
            <div class="section-header section-header--flex">
              <span>Indexes ({indexes.length})</span>
              {#if editMode}
                <button class="add-btn" onclick={openAddIdx}>+ Add Index</button>
              {/if}
            </div>
            <div class="index-list">
              {#each indexes as idx (idx.name)}
                <div class="index-row">
                  <div class="index-main">
                    <span class="index-name mono">{idx.name}</span>
                    <span class="index-cols mono">{idx.columns.join(', ')}</span>
                  </div>
                  <div class="index-meta">
                    {#if idx.unique}<span class="badge badge--unique">UNIQUE</span>{/if}
                    <span class="index-type">{idx.indexType}</span>
                  </div>
                  {#if editMode}
                    <div class="row-actions row-actions--shown">
                      <button
                        class="act-btn act-btn--danger"
                        title="Drop index"
                        onclick={() =>
                          requestDrop(`Drop index "${idx.name}"?`, [buildDropIdxSql(idx.name)])}
                      >
                        <TrashSmIcon />
                      </button>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <!-- Foreign Keys ─────────────────────────────────────────────────── -->
        {#if foreignKeys.length > 0 || editMode}
          <section class="section">
            <div class="section-header section-header--flex">
              <span>Foreign Keys ({foreignKeys.length})</span>
              {#if editMode && !isSqlite}
                <button class="add-btn" onclick={openAddFk}>+ Add Foreign Key</button>
              {/if}
            </div>
            <div class="fk-list">
              {#each foreignKeys as fk (fk.constraintName)}
                <div class="fk-card" class:fk-card--edit={editMode}>
                  {#if editMode}
                    <button
                      class="act-btn act-btn--danger fk-drop-btn"
                      title="Drop foreign key"
                      onclick={() =>
                        requestDrop(`Drop foreign key "${fk.constraintName}"?`, [
                          buildDropFkSql(fk.constraintName),
                        ])}
                    >
                      <TrashSmIcon />
                    </button>
                  {/if}
                  <div class="fk-name mono">{fk.constraintName}</div>
                  <div class="fk-relation">
                    <span class="mono fk-cols">{fk.columns.join(', ')}</span>
                    <span class="fk-arrow">→</span>
                    <span class="mono fk-ref"
                      >{fk.referencedTable}.{fk.referencedColumns.join(', ')}</span
                    >
                  </div>
                  <div class="fk-actions">
                    <span class="fk-action-label">ON DELETE</span>
                    {fk.onDelete}
                    <span class="fk-sep">·</span>
                    <span class="fk-action-label">ON UPDATE</span>
                    {fk.onUpdate}
                  </div>
                </div>
              {/each}
              {#if foreignKeys.length === 0 && editMode && !isSqlite}
                <div class="empty-hint">No foreign keys defined.</div>
              {/if}
              {#if editMode && isSqlite}
                <div class="sqlite-note">
                  SQLite does not support adding foreign key constraints to existing tables.
                </div>
              {/if}
            </div>
          </section>
        {/if}

        <!-- Virtual relations ──────────────────────────────────────────── -->
        {#if tableVirtualRelations.length > 0 || editMode}
          <section class="section">
            <div class="section-header section-header--flex">
              <span>Virtual relations ({tableVirtualRelations.length})</span>
              {#if editMode}
                <button class="add-btn" onclick={openAddVrFromHeader}
                  >+ Add Virtual relation</button
                >
              {/if}
            </div>
            <div class="fk-list">
              {#each tableVirtualRelations as vr (vr.id)}
                {@const isFrom =
                  vr.from.connectionId === connectionId &&
                  vr.from.database === database &&
                  vr.from.table === table}
                {@const localCol = isFrom ? vr.from.column : vr.to.column}
                {@const otherRef = isFrom ? vr.to : vr.from}
                <div class="fk-card vr-card">
                  <div class="vr-actions">
                    <button class="act-btn" title="Edit connection" onclick={() => openEditVr(vr)}>
                      <PencilSmIcon />
                    </button>
                    <button
                      class="act-btn act-btn--danger"
                      title="Remove connection"
                      onclick={() => {
                        vrStore.remove(vr.id);
                      }}
                    >
                      <TrashSmIcon />
                    </button>
                  </div>
                  {#if vr.label}
                    <div class="fk-name">{vr.label}</div>
                  {/if}
                  <div class="fk-relation">
                    {#if isFrom}
                      <span class="mono fk-cols">{localCol}</span>
                      <span class="fk-arrow">→</span>
                      <span class="mono fk-ref vr-other-ref"
                        >{#if otherRef.connectionId !== connectionId}<span class="vr-conn-hint"
                            >{connName(otherRef.connectionId)}/</span
                          >{/if}{otherRef.database}.{otherRef.table}.{otherRef.column}</span
                      >
                    {:else}
                      <span class="mono fk-ref vr-other-ref"
                        >{#if otherRef.connectionId !== connectionId}<span class="vr-conn-hint"
                            >{connName(otherRef.connectionId)}/</span
                          >{/if}{otherRef.database}.{otherRef.table}.{otherRef.column}</span
                      >
                      <span class="fk-arrow">→</span>
                      <span class="mono fk-cols">{localCol}</span>
                    {/if}
                  </div>
                  <div class="fk-actions">
                    <span class="badge badge--vr">virtual</span>
                  </div>
                </div>
              {/each}
              {#if tableVirtualRelations.length === 0 && editMode}
                <div class="empty-hint">No virtual relations defined.</div>
              {/if}
            </div>
          </section>
        {/if}

        <!-- Polymorphic Virtual Relations ───────────────────────────────── -->
        {#if tablePolymorphicRelations.length > 0 || editMode}
          <section class="section">
            <div class="section-header section-header--flex">
              <span>Polymorphic Virtual Relations ({tablePolymorphicRelations.length})</span>
              {#if editMode}
                <button class="add-btn" onclick={() => { pvrCreateModal = true; }}>+ Add Polymorphic Relation</button>
              {/if}
            </div>
            <div class="fk-list">
              {#each tablePolymorphicRelations as pvr (pvr.id)}
                <div class="fk-card vr-card">
                  <div class="vr-actions">
                    <button class="act-btn" title="Edit relation" onclick={() => { pvrEditModal = pvr; }}>
                      <PencilSmIcon />
                    </button>
                    <button class="act-btn act-btn--danger" title="Remove relation" onclick={() => vrStore.removePolymorphic(pvr.id)}>
                      <TrashSmIcon />
                    </button>
                  </div>
                  {#if pvr.label}
                    <div class="fk-name">{pvr.label}</div>
                  {/if}
                  <div class="pvr-cols-row">
                    <span class="pvr-col-label">type</span>
                    <span class="mono pvr-col-name">{pvr.typeColumn}</span>
                    <span class="pvr-col-sep">·</span>
                    <span class="pvr-col-label">value</span>
                    <span class="mono pvr-col-name">{pvr.valueColumn}</span>
                  </div>
                  <div class="pvr-mappings">
                    {#each pvr.mappings as mapping (mapping.id)}
                      <div class="pvr-mapping-row">
                        <span class="pvr-type-badge">{mapping.typeValue}</span>
                        <span class="fk-arrow">→</span>
                        <span class="mono pvr-target">
                          {#if mapping.to.connectionId !== connectionId}<span class="vr-conn-hint">{connName(mapping.to.connectionId)}/</span>{/if}{mapping.to.database}.{mapping.to.table}.{mapping.to.column}
                        </span>
                      </div>
                    {/each}
                  </div>
                  <div class="fk-actions">
                    <span class="badge badge--vr">polymorphic</span>
                  </div>
                </div>
              {/each}
              {#if tablePolymorphicRelations.length === 0 && editMode}
                <div class="empty-hint">No polymorphic relations defined.</div>
              {/if}
            </div>
          </section>
        {/if}
      </div>
    {/if}
  </div>
</div>

<!-- ── Column type suggestions ────────────────────────────────────────────── -->
<datalist id="col-dtype-opts">
  {#if isMysql}
    <option value="INT"></option>
    <option value="BIGINT"></option>
    <option value="SMALLINT"></option>
    <option value="TINYINT"></option>
    <option value="TINYINT(1)"></option>
    <option value="VARCHAR(255)"></option>
    <option value="VARCHAR(100)"></option>
    <option value="CHAR(36)"></option>
    <option value="TEXT"></option>
    <option value="MEDIUMTEXT"></option>
    <option value="LONGTEXT"></option>
    <option value="DECIMAL(10,2)"></option>
    <option value="FLOAT"></option>
    <option value="DOUBLE"></option>
    <option value="BOOLEAN"></option>
    <option value="DATE"></option>
    <option value="DATETIME"></option>
    <option value="TIMESTAMP"></option>
    <option value="JSON"></option>
  {:else if isPostgres}
    <option value="integer"></option>
    <option value="bigint"></option>
    <option value="smallint"></option>
    <option value="serial"></option>
    <option value="bigserial"></option>
    <option value="text"></option>
    <option value="varchar(255)"></option>
    <option value="char(36)"></option>
    <option value="boolean"></option>
    <option value="numeric(10,2)"></option>
    <option value="float"></option>
    <option value="double precision"></option>
    <option value="date"></option>
    <option value="timestamp"></option>
    <option value="timestamptz"></option>
    <option value="uuid"></option>
    <option value="jsonb"></option>
    <option value="json"></option>
    <option value="bytea"></option>
  {:else}
    <option value="INTEGER"></option>
    <option value="TEXT"></option>
    <option value="REAL"></option>
    <option value="BLOB"></option>
    <option value="NUMERIC"></option>
  {/if}
</datalist>

<!-- ── Virtual relation Column Picker ───────────────────────────────────── -->
{#if vrColPickModal}
  {@const form = vrColPickModal}
  <Modal label="Add Virtual relation" onbackdropclick={() => (vrColPickModal = null)}>
    <div class="modal-card modal-card--sm">
      <div class="modal-title">Add Virtual relation</div>
      <div class="modal-body">
        <div class="form-row">
          <div class="form-label">Local Column</div>
          <Select
            options={columns.map((c) => ({ value: c.name, label: c.name }))}
            value={form.selectedColumn}
            onchange={(v) => {
              vrColPickModal!.selectedColumn = v;
            }}
            size="md"
            searchable
          />
        </div>
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (vrColPickModal = null)}>Cancel</button>
        <button class="btn btn--primary" onclick={submitVrColPick}>Next</button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Virtual Relation Modal ─────────────────────────────────────────────── -->
{#if vrModal}
  <VirtualRelationModal
    from={vrModal.from}
    editId={vrModal.editId}
    initialTo={vrModal.initialTo}
    initialLabel={vrModal.initialLabel}
    onClose={() => (vrModal = null)}
  />
{/if}

<!-- ── Polymorphic Virtual Relation Modals ────────────────────────────────── -->
{#if pvrCreateModal}
  <PolymorphicVirtualRelationModal
    {connectionId}
    {database}
    {table}
    onClose={() => (pvrCreateModal = false)}
  />
{/if}

{#if pvrEditModal}
  <PolymorphicVirtualRelationModal
    connectionId={pvrEditModal.connectionId}
    database={pvrEditModal.database}
    table={pvrEditModal.table}
    editRelation={pvrEditModal}
    onClose={() => (pvrEditModal = null)}
  />
{/if}

<!-- ── Column Modal ──────────────────────────────────────────────────────── -->
{#if columnForm}
  {@const form = columnForm}
  <Modal
    label={form.mode === 'add' ? 'Add Column' : 'Edit Column'}
    onbackdropclick={() => !isSaving && (columnForm = null)}
  >
    <div class="modal-card">
      <div class="modal-title">
        {form.mode === 'add' ? 'Add Column' : `Edit Column: ${form.original?.name}`}
      </div>
      <div class="modal-body">
        <div class="form-row">
          <label class="form-label" for="col-name">Name</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="col-name"
            class="form-input"
            value={form.name}
            autofocus
            oninput={(e) => {
              columnForm!.name = (e.target as HTMLInputElement).value;
            }}
            placeholder="column_name"
          />
        </div>
        <div class="form-row">
          <label class="form-label" for="col-type">
            Type
            {#if isSqlite && form.mode === 'edit'}
              <span class="form-hint">(SQLite: rename only)</span>
            {/if}
          </label>
          <input
            id="col-type"
            class="form-input"
            list="col-dtype-opts"
            value={form.dataType}
            oninput={(e) => {
              columnForm!.dataType = (e.target as HTMLInputElement).value;
            }}
            placeholder={isMysql ? 'VARCHAR(255)' : 'TEXT'}
            disabled={isSqlite && form.mode === 'edit'}
          />
        </div>
        {#if !(isSqlite && form.mode === 'edit')}
          <div class="form-check-row">
            <Checkbox
              id="col-nullable"
              checked={form.nullable}
              onchange={(c) => {
                columnForm!.nullable = c;
              }}
            />
            <label for="col-nullable" class="form-check-label">Allow NULL</label>
          </div>
          <div class="form-row">
            <label class="form-label" for="col-default">Default Value</label>
            <input
              id="col-default"
              class="form-input"
              value={form.defaultValue}
              oninput={(e) => {
                columnForm!.defaultValue = (e.target as HTMLInputElement).value;
              }}
              placeholder="e.g. 0, 'active', NULL, CURRENT_TIMESTAMP"
            />
          </div>
          {#if isMysql}
            <div class="form-check-row">
              <Checkbox
                id="col-ai"
                checked={form.autoIncrement}
                onchange={(c) => {
                  columnForm!.autoIncrement = c;
                }}
              />
              <label for="col-ai" class="form-check-label">Auto Increment</label>
            </div>
            <div class="form-row">
              <label class="form-label" for="col-comment">Comment</label>
              <input
                id="col-comment"
                class="form-input"
                value={form.comment}
                oninput={(e) => {
                  columnForm!.comment = (e.target as HTMLInputElement).value;
                }}
                placeholder="Optional column comment"
              />
            </div>
          {/if}
        {/if}
        {#if saveError}
          <div class="modal-error">{saveError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (columnForm = null)} disabled={isSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitCol} disabled={isSaving}>
          {isSaving ? 'Saving…' : form.mode === 'add' ? 'Add Column' : 'Save Changes'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Index Modal ───────────────────────────────────────────────────────── -->
{#if indexForm}
  {@const form = indexForm}
  <Modal label="Add Index" onbackdropclick={() => !isSaving && (indexForm = null)}>
    <div class="modal-card">
      <div class="modal-title">Add Index</div>
      <div class="modal-body">
        <div class="form-row">
          <label class="form-label" for="idx-name">
            Index Name <span class="form-hint">(auto-generated if blank)</span>
          </label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="idx-name"
            class="form-input"
            value={form.name}
            autofocus
            oninput={(e) => {
              indexForm!.name = (e.target as HTMLInputElement).value;
            }}
            placeholder={`idx_${table}_column`}
          />
        </div>
        <div class="form-row">
          <div class="form-label">Columns</div>
          <div class="col-selector">
            {#each columns as col}
              {@const cbId = `idx-col-${col.name}`}
              <div class="col-selector-item">
                <Checkbox
                  id={cbId}
                  size="sm"
                  checked={form.selectedColumns.includes(col.name)}
                  onchange={() => {
                    indexForm!.selectedColumns = toggleColSel(col.name, indexForm!.selectedColumns);
                  }}
                />
                <label class="col-selector-label" for={cbId}>
                  <span class="mono">{col.name}</span>
                  <span class="col-type-hint">{col.dataType}</span>
                </label>
              </div>
            {/each}
          </div>
        </div>
        {#if !form.isPrimary}
          <div class="form-check-row">
            <Checkbox
              id="idx-unique"
              checked={form.unique}
              onchange={(c) => {
                indexForm!.unique = c;
              }}
            />
            <label for="idx-unique" class="form-check-label">Unique</label>
          </div>
        {/if}
        {#if !isSqlite}
          <div class="form-check-row">
            <Checkbox
              id="idx-pk"
              checked={form.isPrimary}
              onchange={(c) => {
                indexForm!.isPrimary = c;
                if (indexForm!.isPrimary) indexForm!.unique = true;
              }}
            />
            <label for="idx-pk" class="form-check-label">Primary Key</label>
          </div>
        {/if}
        {#if saveError}
          <div class="modal-error">{saveError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (indexForm = null)} disabled={isSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitIdx} disabled={isSaving}>
          {isSaving ? 'Saving…' : 'Add Index'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Foreign Key Modal ─────────────────────────────────────────────────── -->
{#if fkForm}
  {@const form = fkForm}
  <Modal label="Add Foreign Key" onbackdropclick={() => !isSaving && (fkForm = null)}>
    <div class="modal-card">
      <div class="modal-title">Add Foreign Key</div>
      <div class="modal-body">
        <div class="form-row">
          <label class="form-label" for="fk-name">
            Constraint Name <span class="form-hint">(auto-generated if blank)</span>
          </label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="fk-name"
            class="form-input"
            value={form.constraintName}
            autofocus
            oninput={(e) => {
              fkForm!.constraintName = (e.target as HTMLInputElement).value;
            }}
            placeholder={`fk_${table}_column`}
          />
        </div>
        <div class="form-row">
          <div class="form-label">Local Columns</div>
          <div class="col-selector">
            {#each columns as col}
              {@const cbId = `fk-col-${col.name}`}
              <div class="col-selector-item">
                <Checkbox
                  id={cbId}
                  size="sm"
                  checked={form.selectedColumns.includes(col.name)}
                  onchange={() => {
                    fkForm!.selectedColumns = toggleColSel(col.name, fkForm!.selectedColumns);
                  }}
                />
                <label class="col-selector-label" for={cbId}>
                  <span class="mono">{col.name}</span>
                  <span class="col-type-hint">{col.dataType}</span>
                </label>
              </div>
            {/each}
          </div>
        </div>
        <div class="form-row">
          <label class="form-label" for="fk-ref-table">Referenced Table</label>
          <Select
            id="fk-ref-table"
            options={fkRefTableOptions}
            value={form.referencedTable}
            onchange={(v) => {
              fkForm!.referencedTable = v;
              fkForm!.referencedColumns = '';
              loadFkRefColumns(v);
            }}
            size="md"
            searchable
            placeholder="other_table"
          />
        </div>
        <div class="form-row">
          <label class="form-label" for="fk-ref-cols">Referenced Column</label>
          <Select
            id="fk-ref-cols"
            options={fkRefColumnOptions}
            value={form.referencedColumns}
            onchange={(v) => {
              fkForm!.referencedColumns = v;
            }}
            size="md"
            searchable
            placeholder="id"
            disabled={!form.referencedTable}
          />
        </div>
        <div class="form-row-pair">
          <div class="form-row">
            <div class="form-label">ON DELETE</div>
            <Select
              options={refActions}
              value={form.onDelete}
              onchange={(v) => {
                fkForm!.onDelete = v;
              }}
              size="md"
            />
          </div>
          <div class="form-row">
            <div class="form-label">ON UPDATE</div>
            <Select
              options={refActions}
              value={form.onUpdate}
              onchange={(v) => {
                fkForm!.onUpdate = v;
              }}
              size="md"
            />
          </div>
        </div>
        {#if saveError}
          <div class="modal-error">{saveError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (fkForm = null)} disabled={isSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitFk} disabled={isSaving}>
          {isSaving ? 'Saving…' : 'Add Foreign Key'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Drop Confirm Modal ─────────────────────────────────────────────────── -->
{#if confirmDrop}
  {@const drop = confirmDrop}
  <Modal label="Confirm drop" onbackdropclick={() => !isSaving && (confirmDrop = null)}>
    <div class="modal-card modal-card--sm">
      <div class="modal-title">{drop.label}</div>
      <div class="modal-body">
        <p class="confirm-text">This action cannot be undone.</p>
        <div class="preview-sql mono"><SqlHighlight sql={drop.sqls[0]} /></div>
        {#if saveError}
          <div class="modal-error">{saveError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (confirmDrop = null)} disabled={isSaving}>Cancel</button>
        <button class="btn btn--danger" onclick={execConfirm} disabled={isSaving}>
          {isSaving ? 'Dropping…' : 'Drop'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .structure-viewer {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  /* ── Toolbar ───────────────────────────────────────────────────────────── */

  .toolbar {
    flex-shrink: 0;
    height: var(--toolbar-height);
    min-height: var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
  }

  .object-label {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    min-width: 0;
    cursor: grab;
    -webkit-user-select: none;
    user-select: none;
  }

  .object-type {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-disabled);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    flex-shrink: 0;
  }

  .object-type-sep {
    color: var(--color-border-strong);
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .object-path {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .edit-toggle {
    margin-left: auto;
    flex-shrink: 0;
    height: 22px;
    padding: 0 10px;
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
    cursor: pointer;
    letter-spacing: 0.02em;
    transition: all var(--transition-fast);
  }

  .edit-toggle:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .edit-toggle--active {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .edit-toggle--active:hover {
    background: var(--color-accent-hover, var(--color-accent));
    border-color: var(--color-accent-hover, var(--color-accent));
    opacity: 0.9;
  }

  /* ── Scroll content ────────────────────────────────────────────────────── */

  .content {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .sections {
    display: flex;
    flex-direction: column;
    padding-bottom: var(--spacing-4);
    min-width: fit-content;
  }

  .section {
    border-bottom: 1px solid var(--color-border);
  }

  .section:last-child {
    border-bottom: none;
  }

  .section-header {
    height: 29px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 var(--spacing-3);
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    border-bottom: 1px solid var(--color-border);
  }

  .section-header--flex {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  /* ── Add buttons in section headers ────────────────────────────────────── */

  .add-btn {
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    padding: 2px 8px;
    height: 18px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    letter-spacing: 0;
    text-transform: none;
    transition: all var(--transition-fast);
    white-space: nowrap;
    display: flex;
    align-items: center;
  }

  .add-btn:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  /* ── Columns table ─────────────────────────────────────────────────────── */

  .col-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .col-table thead tr {
    background: var(--color-table-header-bg);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
  }

  .col-table th {
    padding: 5px var(--spacing-3);
    text-align: left;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border-strong);
    white-space: nowrap;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .th-narrow {
    width: 1px;
    text-align: center;
    white-space: nowrap;
  }

  .th-actions {
    width: 52px;
  }

  .th-drag {
    width: 20px;
    padding: 0 !important;
  }

  .col-table--editing .th-actions,
  .col-table--editing .col-actions {
    width: 74px;
  }

  .col-table--dragging {
    user-select: none;
  }

  .col-table--dragging td {
    pointer-events: none;
  }

  .col-table--dragging .col-drag-cell {
    pointer-events: auto;
  }

  .col-row-dragging td {
    opacity: 0.4;
  }

  .col-drop-line td {
    height: 2px;
    padding: 0 !important;
    background: var(--color-accent);
    border: none !important;
  }

  .col-drag-cell {
    padding: 0 var(--spacing-1) !important;
    width: 20px;
  }

  .col-drag-handle {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0 2px;
    color: var(--color-text-muted);
    cursor: grab;
    opacity: 0.6;
    transition: opacity var(--transition-fast);
    touch-action: none;
  }

  .col-table--dragging .col-drag-handle {
    cursor: grabbing;
  }

  .col-table tbody tr:hover .col-drag-handle {
    opacity: 1;
  }

  .col-table td {
    padding: 7px var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    vertical-align: middle;
    transition: background var(--transition-fast);
  }

  .col-table tbody tr:last-child td {
    border-bottom: none;
  }

  .col-table tbody tr:hover td {
    background: var(--color-table-row-hover);
  }

  .pk-row td {
    background: rgba(124, 92, 255, 0.03);
  }

  .col-table tbody tr.pk-row:hover td {
    background: var(--color-table-row-hover);
  }

  .pk-row .col-name {
    color: var(--color-accent);
  }

  .center-cell {
    text-align: center;
  }

  .col-name {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    white-space: nowrap;
  }

  .col-type {
    color: var(--color-text-secondary);
    white-space: nowrap;
  }

  .col-keys {
    white-space: nowrap;
    text-align: center;
  }

  .col-null {
    text-align: center;
    color: var(--color-success);
  }

  .col-unique {
    text-align: center;
    color: var(--color-success);
  }

  .col-default {
    color: var(--color-text-muted);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-actions {
    padding: 0 var(--spacing-2) !important;
    width: 52px;
  }

  .comment-row td {
    padding: 0 var(--spacing-3) 6px;
    font-size: 11px;
    color: var(--color-text-muted);
    font-style: italic;
    border-bottom: 1px solid var(--color-border);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  /* ── Row action buttons ────────────────────────────────────────────────── */

  .row-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .act-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    flex-shrink: 0;
    transition: all var(--transition-fast);
  }

  .act-btn:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border);
    color: var(--color-text-primary);
  }

  .act-btn--danger:hover {
    background: var(--color-danger-subtle, rgba(239, 68, 68, 0.1));
    border-color: rgba(239, 68, 68, 0.3);
    color: var(--color-danger, #ef4444);
  }

  /* ── Badges ────────────────────────────────────────────────────────────── */

  .badge {
    display: inline-block;
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.04em;
    margin-right: 2px;
    border: 1px solid transparent;
  }

  .badge--pk {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: rgba(124, 92, 255, 0.22);
  }

  .badge--ai {
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    color: var(--color-text-muted);
    border-color: var(--color-border);
  }

  .badge--fk {
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-color: rgba(22, 163, 74, 0.22);
  }

  .badge--unique {
    background: var(--color-success-subtle);
    color: var(--color-success);
    border-color: rgba(22, 163, 74, 0.22);
    font-size: 9px;
  }

  /* ── Indexes ───────────────────────────────────────────────────────────── */

  .index-list {
    padding: 0;
  }

  .index-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 7px var(--spacing-3);
    font-size: var(--font-size-xs);
    border-bottom: 1px solid var(--color-border);
    transition: background var(--transition-fast);
  }

  .index-row:last-child {
    border-bottom: none;
  }

  .index-row:hover {
    background: var(--color-table-row-hover);
  }

  .index-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .index-name {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .index-cols {
    color: var(--color-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
  }

  .index-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    flex-shrink: 0;
  }

  .index-type {
    font-size: 10px;
    color: var(--color-text-muted);
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--color-border);
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }

  /* ── Foreign Keys ──────────────────────────────────────────────────────── */

  .fk-list {
    padding: var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .fk-card {
    padding: var(--spacing-2) var(--spacing-3);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    -webkit-backdrop-filter: var(--glass-blur);
    backdrop-filter: var(--glass-blur);
    box-shadow: var(--shadow-sm);
    font-size: var(--font-size-xs);
    transition:
      box-shadow var(--transition-fast),
      border-color var(--transition-fast);
    position: relative;
  }

  .fk-card:hover {
    border-color: var(--color-border-strong);
    box-shadow: var(--shadow-md);
  }

  .fk-card--edit {
    padding-right: var(--spacing-8);
  }

  .fk-drop-btn {
    position: absolute;
    top: var(--spacing-2);
    right: var(--spacing-2);
  }

  .fk-name {
    color: var(--color-text-muted);
    font-weight: var(--font-weight-medium);
    margin-bottom: 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    letter-spacing: 0.02em;
    font-family: var(--font-family-mono);
  }

  .fk-relation {
    display: flex;
    align-items: center;
    gap: var(--spacing-1) var(--spacing-2);
    margin-bottom: 6px;
    flex-wrap: wrap;
    font-family: var(--font-family-mono);
  }

  .fk-cols {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .fk-arrow {
    color: var(--color-accent);
    flex-shrink: 0;
    opacity: 0.6;
  }

  .fk-ref {
    color: var(--color-accent);
  }

  .fk-actions {
    font-size: 10px;
    color: var(--color-text-muted);
    padding-top: 5px;
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: 3px;
    flex-wrap: wrap;
  }

  .fk-action-label {
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-size: 9px;
    color: var(--color-text-muted);
  }

  .fk-sep {
    color: var(--color-border-strong);
    margin: 0 5px;
  }

  .act-btn--connect:hover {
    background: var(--color-accent-subtle);
    border-color: rgba(124, 92, 255, 0.22);
  }

  .badge--vr {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: rgba(124, 92, 255, 0.22);
    font-size: 9px;
  }

  .vr-card {
    padding-right: var(--spacing-8);
  }

  .vr-actions {
    position: absolute;
    top: var(--spacing-2);
    right: var(--spacing-2);
    display: flex;
    gap: 2px;
  }

  .vr-other-ref {
    color: var(--color-accent);
  }

  .vr-conn-hint {
    color: var(--color-text-muted);
    font-size: 10px;
  }

  /* ── Polymorphic relation card ─────────────────────────────────────────── */

  .pvr-cols-row {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--font-size-xs);
    margin-bottom: 6px;
    flex-wrap: wrap;
  }

  .pvr-col-label {
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--color-text-muted);
  }

  .pvr-col-name {
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .pvr-col-sep {
    color: var(--color-border-strong);
    margin: 0 2px;
  }

  .pvr-mappings {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-bottom: 6px;
    padding: var(--spacing-1) var(--spacing-2);
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
  }

  .pvr-mapping-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--font-size-xs);
    min-width: 0;
  }

  .pvr-type-badge {
    flex-shrink: 0;
    display: inline-block;
    padding: 1px 5px;
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-radius: var(--radius-sm);
    font-family: var(--font-family-mono);
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    white-space: nowrap;
  }

  .pvr-target {
    color: var(--color-accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    font-size: var(--font-size-xs);
  }

  .sqlite-note {
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-style: italic;
    padding: var(--spacing-1) 0;
  }

  .empty-hint {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-xs);
    color: var(--color-text-disabled);
    font-style: italic;
  }

  /* ── State overlays ────────────────────────────────────────────────────── */

  .state-overlay {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-8) var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    height: 200px;
  }

  .state-overlay--error {
    color: var(--color-danger);
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  /* ── Modal card ────────────────────────────────────────────────────────── */

  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 440px;
    max-width: 92vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  .modal-card--sm {
    width: 360px;
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
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
    max-height: 65vh;
    overflow-y: auto;
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

  /* ── Form elements ─────────────────────────────────────────────────────── */

  .form-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-row-pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-3);
  }

  .form-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .form-hint {
    font-weight: var(--font-weight-regular, 400);
    color: var(--color-text-disabled);
    font-size: 10px;
  }

  .form-input {
    height: 30px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
  }

  .form-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .form-input:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .form-check-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .form-check-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
  }

  /* ── Column multi-selector ─────────────────────────────────────────────── */

  .col-selector {
    display: flex;
    flex-direction: column;
    max-height: 140px;
    overflow-y: auto;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: 3px;
    background: var(--color-bg-secondary);
  }

  .col-selector-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 4px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    transition: background var(--transition-fast);
  }

  .col-selector-item:hover {
    background: var(--color-bg-hover);
  }

  .col-selector-label {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex: 1;
    cursor: pointer;
    min-width: 0;
  }

  .col-type-hint {
    margin-left: auto;
    font-size: 10px;
    color: var(--color-text-disabled);
    font-family: var(--font-family-mono);
  }

  /* ── Modal buttons ─────────────────────────────────────────────────────── */

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

  .btn:hover:not(:disabled) {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover:not(:disabled) {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }

  .btn--danger {
    background: var(--color-danger, #ef4444);
    border-color: var(--color-danger, #ef4444);
    color: white;
  }

  .btn--danger:hover:not(:disabled) {
    opacity: 0.88;
  }

  /* ── Modal content helpers ─────────────────────────────────────────────── */

  .modal-error {
    padding: 8px 10px;
    background: var(--color-danger-subtle, rgba(239, 68, 68, 0.08));
    border: 1px solid rgba(239, 68, 68, 0.35);
    border-radius: var(--radius-md);
    color: var(--color-danger, #ef4444);
    font-size: var(--font-size-xs);
    line-height: var(--line-height-normal);
  }

  .confirm-text {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
  }

  .preview-sql {
    margin-top: var(--spacing-2);
    padding: 8px 10px;
    background: var(--color-bg-tertiary, var(--color-bg-hover));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: 11px;
    color: var(--color-text-secondary);
    word-break: break-all;
    line-height: 1.5;
  }
</style>
