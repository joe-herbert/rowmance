<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import {
    EditorView,
    ViewPlugin,
    tooltips as cmTooltips,
    Decoration,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    drawSelection,
    dropCursor,
    rectangularSelection,
    crosshairCursor,
    gutter,
    GutterMarker,
    WidgetType,
    type DecorationSet,
    type ViewUpdate,
  } from '@codemirror/view';
  import {
    EditorState,
    StateEffect,
    StateField,
    RangeSetBuilder,
    type Extension,
  } from '@codemirror/state';
  import { highlightSelectionMatches } from '@codemirror/search';
  import { defaultKeymap, indentWithTab, history, historyKeymap } from '@codemirror/commands';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import {
    HighlightStyle,
    syntaxHighlighting,
    bracketMatching,
    foldGutter,
    syntaxTree,
  } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import type { SyntaxNode } from '@lezer/common';
  import {
    closeBrackets,
    closeBracketsKeymap,
    autocompletion,
    completionKeymap,
    type CompletionContext,
    type CompletionResult,
    type CompletionSource,
  } from '@codemirror/autocomplete';
  import { lintGutter, setDiagnostics } from '@codemirror/lint';
  import { format as sqlFormat } from 'sql-formatter';
  import { buildDiagnosticsFromErrors } from '$lib/utils/sqlErrorHighlight';
  import type { QueryResult, ForeignKeyInfo, VirtualRelation } from '$lib/types';
  import { executeMultiQuery, explainQuery } from '$lib/tauri/query';
  import { listen } from '@tauri-apps/api/event';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import ResultsPanel from '$lib/components/editor/ResultsPanel.svelte';
  import FkSearchPopup from '$lib/components/editor/FkSearchPopup.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { listVirtualRelations } from '$lib/tauri/virtual_relations';
  import {
    splitStatements,
    statementAtCursor,
    isMutatingStatement,
    stripLineComments,
  } from '$lib/utils/sql';
  import { errorMessage } from '$lib/utils/errors';
  import { getFkValueContext } from '$lib/utils/sqlFkContext';
  import Select from '$lib/components/ui/Select.svelte';
  import AiModal from '$lib/components/ai/AiModal.svelte';
  import QueryBuilderModal from '$lib/components/editor/QueryBuilderModal.svelte';
  import { portal } from '$lib/actions/portal';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { savedQueriesInvalidator } from '$lib/stores/savedQueriesInvalidator.svelte';
  import { sessionAcquire, sessionRelease } from '$lib/tauri/sessions';
  import type { FileQuery } from '$lib/types';
  import { useRecording } from '$lib/stores/recording.svelte';
  import { useRevert } from '$lib/stores/revert.svelte';
  import SaveIcon from '$lib/components/icons/SaveIcon.svelte';
  import FileDocIcon from '$lib/components/icons/FileDocIcon.svelte';
  import DotsIcon from '$lib/components/icons/DotsIcon.svelte';
  import WarningIcon from '$lib/components/icons/WarningIcon.svelte';
  import ChevronIcon from '$lib/components/icons/ChevronIcon.svelte';
  import { useChartData } from '$lib/stores/chartData.svelte';

  interface Props {
    connectionId: string;
    database?: string;
    initialSql?: string;
    initialDescription?: string;
    initialAnnotations?: string;
    editorId?: string;
    savedQueryId?: string;
    savedQueryName?: string;
    onExecute?: (_sql: string) => void;
  }

  let {
    connectionId,
    database: initialDatabase,
    initialSql = '',
    initialDescription = '',
    initialAnnotations = undefined,
    editorId,
    savedQueryId: initialSavedQueryId,
    savedQueryName: initialSavedQueryName,
    onExecute,
  }: Props = $props();

  const connections = useConnections();
  const settingsStore = useSettings();
  const panelStore = usePanels();
  const recording = useRecording();
  const revertStore = useRevert();
  const chartStore = useChartData();

  const cached = untrack(() => (editorId ? queryEditorCache.get(editorId) : undefined));

  const isFocusedPanel = $derived(
    panelStore.focusedPanel?.content?.kind === 'query_editor' &&
      panelStore.focusedPanel?.content?.editorId === editorId,
  );

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let resultsWrapperEl = $state<HTMLDivElement | undefined>(undefined);
  let sqlText = $state(untrack(() => cached?.sql ?? initialSql));
  let executedSql = $state('');
  let results = $state<QueryResult[]>(untrack(() => cached?.results ?? []));
  let executedStatements = $state<string[]>(untrack(() => cached?.executedStatements ?? []));
  let isRunning = $state(false);
  let databases = $state<string[]>([]);
  let selectedDatabase = $state<string>(
    untrack(
      () =>
        cached?.selectedDatabase ??
        initialDatabase ??
        connections.getById(connectionId)?.database ??
        '',
    ),
  );
  let activeResultTab = $state(untrack(() => cached?.activeResultTab ?? 0));

  // ── Editor session (pinned connection for variable persistence) ───────────

  $effect(() => {
    if (!editorId || !connections.isActive(connectionId)) return;
    sessionAcquire(connectionId, editorId).catch(() => {});
  });

  // ── SQL variable tracking (MySQL/MariaDB @varname) ────────────────────────

  let sqlVariableNames = $derived.by((): string[] => {
    if (!connections.getById(connectionId)?.dialectInfo.detectsSqlVariables) return [];
    const vars = new Set<string>();
    const re = /@([a-zA-Z_]\w*)/g;
    // Strip string literals and comments to avoid false positives
    const cleaned = sqlText
      .replace(/'(?:[^'\\]|\\.)*'/g, '')
      .replace(/"(?:[^"\\]|\\.)*"/g, '')
      .replace(/--[^\n]*/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '');
    let m;
    while ((m = re.exec(cleaned)) !== null) {
      vars.add(`@${m[1]}`);
    }
    return [...vars].sort();
  });

  let variableValues = $state<Record<string, string | null>>(
    untrack(() => cached?.variableValues ?? {}),
  );

  async function fetchVariableValues(): Promise<void> {
    const vars = sqlVariableNames;
    if (vars.length === 0) return;
    try {
      const res = await executeMultiQuery(
        connectionId,
        `SELECT ${vars.join(', ')}`,
        selectedDatabase || null,
        editorId,
      );
      if (res[0]?.error === null && res[0].rows.length > 0) {
        const newValues: Record<string, string | null> = {};
        res[0].columns.forEach((col, i) => {
          const val = res[0].rows[0][i];
          newValues[col.name] = val === null ? null : String(val);
        });
        variableValues = newValues;
      }
    } catch {
      // Silently ignore — variables panel just won't update
    }
  }

  $effect(() => {
    if (!editorId) return;
    queryEditorCache.save(editorId, {
      sql: sqlText,
      results,
      executedStatements,
      selectedDatabase,
      activeResultTab,
      variableValues,
      description: descriptionText,
      annotations: annotationsJson ?? undefined,
    });
  });

  $effect(() => {
    notesStructureVersion;
    const view = editorView;
    if (!view) return;
    view.dispatch({ effects: setNotesEffect.of([...inlineNotes]) });
  });

  $effect(() => {
    if (!editorId) return;
    const dirtyKey = `query:${editorId}`;
    if (savedSql !== null) {
      panelStore.setItemDirty(
        dirtyKey,
        sqlText !== savedSql ||
          descriptionText !== (savedDescription ?? '') ||
          annotationsJson !== savedAnnotations,
      );
    } else {
      panelStore.setItemDirty(dirtyKey, sqlText.trim() !== '' || descriptionText.trim() !== '');
    }
  });

  // ── Chart integration ────────────────────────────────────────────────────

  const chartSourceId = $derived(`query:${editorId}`);

  // Live-update chart when result rows change (only if this source is active).
  // untrack prevents reading chartSource inside updateRows() from creating a dependency.
  $effect(() => {
    const r = results[activeResultTab];
    const sid = chartSourceId;
    if (!r || !r.columns) return;
    untrack(() => chartStore.updateRows(sid, r.rows as (string | number | boolean | null)[][], []));
  });

  function openChartForCurrentResult() {
    const r = results[activeResultTab];
    if (!r || !r.columns || r.rows.length === 0) return;
    chartStore.openChart({
      columns: r.columns as import('$lib/types').ColumnMeta[],
      allRows: r.rows as (string | number | boolean | null)[][],
      selectedRows: [],
      source: `Query ${activeResultTab + 1} results`,
      sourceId: chartSourceId,
    });
  }

  let currentSavedQueryId = $state<string | undefined>(untrack(() => initialSavedQueryId));
  let currentSavedQueryName = $state<string | undefined>(untrack(() => initialSavedQueryName));
  let savedSql = $state<string | null>(untrack(() => (initialSavedQueryId ? initialSql : null)));
  let descriptionText = $state<string>(
    untrack(() => cached?.description ?? initialDescription ?? ''),
  );
  let savedDescription = $state<string | null>(
    untrack(() => (initialSavedQueryId ? (initialDescription ?? '') : null)),
  );
  let descriptionOpen = $state<boolean>(
    untrack(() => !!(cached?.description ?? initialDescription)),
  );
  let aiModal = $state<
    { mode: 'generate'; insertLine: number } | { mode: 'explain'; sql: string } | null
  >(null);
  let editorContextMenu = $state<{
    x: number;
    y: number;
    hasSelection: boolean;
    selectionSql: string;
    cursorSql: string;
  } | null>(null);

  // ── Inline Notes ──────────────────────────────────────────────────────────

  interface InlineNote {
    id: string;
    lineNumber: number;
    placement: 'above' | 'below';
    text: string;
  }

  function parseAnnotations(json: string | null | undefined): InlineNote[] {
    if (!json) return [];
    try {
      return JSON.parse(json);
    } catch {
      return [];
    }
  }

  let inlineNotes = $state<InlineNote[]>(
    untrack(() => parseAnnotations(cached?.annotations ?? initialAnnotations)),
  );
  let savedAnnotations = $state<string | null>(
    untrack(() => (initialSavedQueryId ? (initialAnnotations ?? null) : null)),
  );
  let noteMenu = $state<{ lineNumber: number; x: number; y: number } | null>(null);
  let queryBuilderLine = $state<number | null>(null);
  let notesStructureVersion = $state(0);

  const annotationsJson = $derived(inlineNotes.length > 0 ? JSON.stringify(inlineNotes) : null);

  const setNotesEffect = StateEffect.define<InlineNote[]>();

  const notesField = StateField.define<InlineNote[]>({
    create: () => [...inlineNotes],
    update(notes, tr) {
      for (const e of tr.effects) if (e.is(setNotesEffect)) return e.value;
      return notes;
    },
  });

  const noteCallbacksRef: {
    onUpdate: (_id: string, _text: string) => void;
    onRemove: (_id: string) => void;
  } = {
    onUpdate: () => {},
    onRemove: () => {},
  };

  class NoteWidget extends WidgetType {
    private note: InlineNote;
    constructor(note: InlineNote) {
      super();
      this.note = note;
    }

    eq(other: NoteWidget) {
      return (
        other.note.id === this.note.id &&
        other.note.lineNumber === this.note.lineNumber &&
        other.note.placement === this.note.placement
      );
    }

    toDOM(view: EditorView) {
      const wrap = document.createElement('div') as HTMLDivElement & { _ro?: ResizeObserver };
      wrap.className = 'cm-inline-note';

      const syncSize = () => {
        const gutterEl = view.dom.querySelector<HTMLElement>('.cm-gutters');
        const gutterWidth = gutterEl ? gutterEl.offsetWidth : 0;
        wrap.style.left = gutterWidth + 5 + 'px';
        wrap.style.width = view.scrollDOM.clientWidth - gutterWidth - 5 + 'px';
      };
      syncSize();
      wrap._ro = new ResizeObserver(syncSize);
      wrap._ro.observe(view.scrollDOM);

      const deleteBtn = document.createElement('button');
      deleteBtn.className = 'cm-inline-note-delete';
      deleteBtn.type = 'button';
      deleteBtn.textContent = '×';
      deleteBtn.setAttribute('aria-label', 'Delete note');
      deleteBtn.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        noteCallbacksRef.onRemove(this.note.id);
      });

      const textarea = document.createElement('textarea');
      textarea.className = 'cm-inline-note-textarea';
      textarea.value = this.note.text;
      textarea.placeholder = 'Write a note…';
      textarea.rows = 1;
      textarea.addEventListener('input', () => {
        noteCallbacksRef.onUpdate(this.note.id, textarea.value);
      });
      textarea.addEventListener('mousedown', (e) => e.stopPropagation());
      textarea.addEventListener('click', (e) => e.stopPropagation());
      textarea.addEventListener('keydown', (e) => e.stopPropagation());

      wrap.appendChild(deleteBtn);
      wrap.appendChild(textarea);
      return wrap;
    }

    destroy(dom: HTMLElement) {
      (dom as HTMLDivElement & { _ro?: ResizeObserver })._ro?.disconnect();
    }

    ignoreEvent() {
      return true;
    }
  }

  const gutterClickRef: { onClick: (_lineNumber: number, _x: number, _y: number) => void } = {
    onClick: () => {},
  };

  class PlusMarker extends GutterMarker {
    private lineNumber: number;
    constructor(lineNumber: number) {
      super();
      this.lineNumber = lineNumber;
    }
    eq(other: PlusMarker) {
      return other.lineNumber === this.lineNumber;
    }
    toDOM() {
      const btn = document.createElement('button');
      btn.className = 'cm-note-plus-btn';
      btn.type = 'button';
      btn.textContent = '+';
      btn.title = 'Add note';
      btn.addEventListener('mousedown', (e) => {
        e.preventDefault();
        e.stopPropagation();
        const rect = btn.getBoundingClientRect();
        gutterClickRef.onClick(this.lineNumber, rect.right + 6, rect.top);
      });
      return btn;
    }
  }

  function buildNoteDecorations(state: EditorState): DecorationSet {
    const notes = state.field(notesField);
    if (notes.length === 0) return Decoration.none;
    const docLines = state.doc.lines;
    const entries: Array<{ pos: number; side: number; note: InlineNote }> = [];
    for (const note of notes) {
      if (note.lineNumber < 1 || note.lineNumber > docLines) continue;
      const line = state.doc.line(note.lineNumber);
      if (note.placement === 'above') {
        entries.push({ pos: line.from, side: -1, note });
      } else {
        entries.push({ pos: line.to, side: 1, note });
      }
    }
    entries.sort((a, b) => (a.pos !== b.pos ? a.pos - b.pos : a.side - b.side));
    const builder = new RangeSetBuilder<Decoration>();
    for (const { pos, side, note } of entries) {
      builder.add(pos, pos, Decoration.widget({ widget: new NoteWidget(note), block: true, side }));
    }
    return builder.finish();
  }

  const notesDecoField = StateField.define<DecorationSet>({
    create(state) {
      return buildNoteDecorations(state);
    },
    update(deco, tr) {
      if (tr.docChanged || tr.effects.some((e) => e.is(setNotesEffect))) {
        return buildNoteDecorations(tr.state);
      }
      return deco.map(tr.changes);
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  function makeNotesGutter() {
    return gutter({
      class: 'cm-notes-gutter',
      renderEmptyElements: true,
      lineMarker(view, line) {
        const lineNum = view.state.doc.lineAt(line.from).number;
        return new PlusMarker(lineNum);
      },
      lineMarkerChange() {
        return false;
      },
    });
  }

  function addNote(lineNumber: number, placement: 'above' | 'below') {
    const id = crypto.randomUUID();
    inlineNotes = [...inlineNotes, { id, lineNumber, placement, text: '' }];
    notesStructureVersion++;
    noteMenu = null;
  }

  function openQueryBuilder(lineNumber: number) {
    queryBuilderLine = lineNumber;
    noteMenu = null;
  }

  function insertSqlAtLine(lineNumber: number, sql: string) {
    if (!editorView) return;
    const doc = editorView.state.doc;
    const line = doc.line(Math.min(lineNumber, doc.lines));
    const insertPos = line.from;
    const insertText = sql.endsWith('\n') ? sql : sql + '\n';
    editorView.dispatch({
      changes: { from: insertPos, to: insertPos, insert: insertText },
      selection: { anchor: insertPos + insertText.length },
    });
    editorView.focus();
    queryBuilderLine = null;
  }

  $effect(() => {
    if (initialSavedQueryName !== undefined) currentSavedQueryName = initialSavedQueryName;
  });

  $effect(() => {
    if (initialSavedQueryId === undefined && currentSavedQueryId !== undefined) {
      currentSavedQueryId = undefined;
      savedSql = null;
    }
  });
  let saveDialogOpen = $state(false);
  let saveNameInput = $state('');
  let isSaving = $state(false);
  let saveDialogTriggerEl = $state<HTMLButtonElement | undefined>(undefined);
  let saveDialogEl = $state<HTMLDivElement | undefined>(undefined);
  let saveDialogTop = $state(0);
  let saveDialogLeft = $state(0);
  let saveNameInputEl = $state<HTMLInputElement | undefined>(undefined);

  let editorHeightPct = $state(40);
  let isResizingEditor = $state(false);
  let varsOpen = $state(true);
  let editorPanelEl = $state<HTMLDivElement | undefined>(undefined);
  let resizerStartY = 0;
  let resizerStartPct = 0;

  function onResizerPointerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.preventDefault();
    isResizingEditor = true;
    resizerStartY = e.clientY;
    resizerStartPct = editorHeightPct;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onResizerPointerMove(e: PointerEvent): void {
    if (!isResizingEditor || !editorPanelEl) return;
    const rect = editorPanelEl.getBoundingClientRect();
    const deltaPct = ((e.clientY - resizerStartY) / rect.height) * 100;
    editorHeightPct = Math.min(80, Math.max(15, resizerStartPct + deltaPct));
  }

  function onResizerPointerUp(): void {
    isResizingEditor = false;
  }

  let toolbarEl = $state<HTMLDivElement | undefined>(undefined);
  let toolbarWidth = $state(9999);
  let actionsMenuOpen = $state(false);
  let actionsMenuTriggerEl = $state<HTMLButtonElement | undefined>(undefined);
  let actionsMenuEl = $state<HTMLDivElement | undefined>(undefined);
  let actionsMenuTop = $state(0);
  let actionsMenuLeft = $state(0);

  const compact = $derived(toolbarWidth < 700);

  $effect(() => {
    if (!toolbarEl) return;
    const ro = new ResizeObserver((entries) => {
      toolbarWidth = entries[0].contentRect.width;
    });
    ro.observe(toolbarEl);
    return () => ro.disconnect();
  });

  $effect(() => {
    if (!actionsMenuOpen) return;

    function positionMenu() {
      if (!actionsMenuTriggerEl || !actionsMenuEl) return;
      const rect = actionsMenuTriggerEl.getBoundingClientRect();
      actionsMenuTop = rect.bottom + 4;
      actionsMenuLeft = rect.right - actionsMenuEl.offsetWidth;
    }

    requestAnimationFrame(positionMenu);

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!actionsMenuTriggerEl?.contains(t) && !actionsMenuEl?.contains(t)) {
        actionsMenuOpen = false;
      }
    }

    document.addEventListener('mousedown', onMousedown, true);
    return () => document.removeEventListener('mousedown', onMousedown, true);
  });

  $effect(() => {
    if (!saveDialogOpen) return;

    function positionDialog() {
      if (!saveDialogTriggerEl || !saveDialogEl) return;
      const rect = saveDialogTriggerEl.getBoundingClientRect();
      saveDialogTop = rect.bottom + 4;
      saveDialogLeft = rect.right - saveDialogEl.offsetWidth;
    }

    requestAnimationFrame(() => {
      positionDialog();
      saveNameInputEl?.focus();
      saveNameInputEl?.select();
    });

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!saveDialogTriggerEl?.contains(t) && !saveDialogEl?.contains(t)) {
        saveDialogOpen = false;
      }
    }

    document.addEventListener('mousedown', onMousedown, true);
    return () => document.removeEventListener('mousedown', onMousedown, true);
  });

  async function saveQuery(): Promise<void> {
    if (!editorId) return;
    if (currentSavedQueryId) {
      isSaving = true;
      try {
        const updated: FileQuery = await savedQueriesApi.fileUpdateSavedQuery(currentSavedQueryId, {
          name: currentSavedQueryName ?? 'Query',
          sql: sqlText,
          description: descriptionText || null,
          annotations: annotationsJson,
          connectionId,
          database: selectedDatabase || null,
        });
        // ID may change if the file was renamed/moved.
        if (updated.id !== currentSavedQueryId) {
          currentSavedQueryId = updated.id;
          panelStore.updateQueryEditorMeta(editorId, { savedQueryId: updated.id });
        }
        savedSql = sqlText;
        savedDescription = descriptionText;
        savedAnnotations = annotationsJson;
        savedQueriesInvalidator.invalidate();
      } finally {
        isSaving = false;
      }
      return;
    }
    saveNameInput = currentSavedQueryName ?? '';
    saveDialogOpen = true;
  }

  function saveQueryAs(): void {
    saveNameInput = currentSavedQueryName ?? '';
    saveDialogOpen = true;
  }

  async function confirmSave(): Promise<void> {
    if (!editorId || !saveNameInput.trim()) return;
    isSaving = true;
    try {
      const saved: FileQuery = await savedQueriesApi.fileCreateSavedQuery({
        name: saveNameInput.trim(),
        sql: sqlText,
        description: descriptionText || null,
        annotations: annotationsJson,
        connectionId,
        database: selectedDatabase || null,
      });
      currentSavedQueryId = saved.id;
      currentSavedQueryName = saved.name;
      savedSql = sqlText;
      savedDescription = descriptionText;
      savedAnnotations = annotationsJson;
      saveDialogOpen = false;
      panelStore.updateQueryEditorMeta(editorId, {
        savedQueryId: saved.id,
        savedQueryName: saved.name,
      });
      savedQueriesInvalidator.invalidate();
    } finally {
      isSaving = false;
    }
  }

  let connectionName = $derived(connections.getById(connectionId)?.name ?? connectionId);

  const txDatabaseMismatch = $derived.by(() => {
    if (!connections.isTransactionActive(connectionId)) return null;
    const profile = connections.getById(connectionId);
    if (!profile || !profile.dialectInfo.warnsTxDatabaseMismatch) return null;
    const txDb = connections.getTxDatabase(connectionId);
    if (!txDb || !selectedDatabase || txDb === selectedDatabase) return null;
    return txDb;
  });

  let sqlDialect = $derived(() => {
    const profile = connections.getById(connectionId);
    return profile?.dialectInfo?.editorDialect ?? 'sql';
  });

  // ── Schema-aware autocomplete ─────────────────────────────────────────────

  interface SchemaTable {
    database: string;
    name: string;
  }

  interface SchemaColumn {
    name: string;
    dataType: string;
  }

  const schemaRef: {
    connectionId: string;
    tables: SchemaTable[];
    columns: Map<string, SchemaColumn[]>;
  } = { connectionId: '', tables: [], columns: new Map() };

  // ── Table-link highlight extensions (Ctrl/Cmd hover) ─────────────────────

  // Keywords that introduce a table name position in a SQL statement
  const TABLE_INTRODUCERS = new Set(['FROM', 'JOIN', 'UPDATE', 'INTO', 'TABLE', 'TRUNCATE']);
  // Keywords that change the clause context (used to track what governs each position)
  const CLAUSE_KEYWORDS = new Set([
    'SELECT',
    'FROM',
    'JOIN',
    'WHERE',
    'SET',
    'UPDATE',
    'INTO',
    'TABLE',
    'INSERT',
    'DELETE',
    'HAVING',
    'GROUP',
    'ORDER',
    'LIMIT',
    'ON',
    'WITH',
    'TRUNCATE',
    'CREATE',
    'DROP',
    'ALTER',
    'BY',
    'UNION',
    'INTERSECT',
    'EXCEPT',
  ]);

  function isTableNamePosition(view: EditorView, node: SyntaxNode): boolean {
    // Find the nearest Statement ancestor
    let stmt: SyntaxNode | null = node.parent;
    while (stmt && stmt.name !== 'Statement') stmt = stmt.parent;
    if (!stmt) return false;

    // Node must be a direct child of that statement (not nested in an expression)
    if (node.parent !== stmt) return false;

    // Scan siblings that precede this node to find the governing clause keyword
    let prevClauseKw: string | null = null;
    let child = stmt.firstChild;
    while (child && child.from < node.from) {
      if (child.name === 'Keyword') {
        const kw = view.state.sliceDoc(child.from, child.to).toUpperCase();
        if (CLAUSE_KEYWORDS.has(kw)) prevClauseKw = kw;
      }
      child = child.nextSibling;
    }

    return prevClauseKw !== null && TABLE_INTRODUCERS.has(prevClauseKw);
  }

  function buildTableLinkDecorations(view: EditorView): DecorationSet {
    const tableNames = new Set(schemaRef.tables.map((t) => t.name.toLowerCase()));
    if (tableNames.size === 0) return Decoration.none;
    const unq = (s: string) => s.replace(/^[`"'[]|[`"'\]]$/g, '');
    const builder = new RangeSetBuilder<Decoration>();
    const mark = Decoration.mark({ class: 'cm-table-link' });
    syntaxTree(view.state).iterate({
      enter(nodeRef) {
        const node = nodeRef.node;
        if (node.name === 'CompositeIdentifier') {
          if (isTableNamePosition(view, node)) {
            const parts = view.state.sliceDoc(node.from, node.to).split('.');
            if (tableNames.has(unq(parts[parts.length - 1]).toLowerCase())) {
              builder.add(node.from, node.to, mark);
            }
          }
          return false;
        }
        if (node.name === 'Identifier' || node.name === 'QuotedIdentifier') {
          if (
            isTableNamePosition(view, node) &&
            tableNames.has(unq(view.state.sliceDoc(node.from, node.to)).toLowerCase())
          ) {
            builder.add(node.from, node.to, mark);
          }
        }
      },
    });
    return builder.finish();
  }

  function makeTableHighlightExtensions(): Extension[] {
    const modifierEffect = StateEffect.define<boolean>();
    const modifierField = StateField.define<boolean>({
      create: () => false,
      update(value, tr) {
        for (const e of tr.effects) if (e.is(modifierEffect)) return e.value;
        return value;
      },
    });

    const plugin = ViewPlugin.fromClass(
      class {
        decorations: DecorationSet = Decoration.none;
        private cmView: EditorView;
        private boundKeyDown: (_e: KeyboardEvent) => void;
        private boundKeyUp: (_e: KeyboardEvent) => void;

        constructor(view: EditorView) {
          this.cmView = view;
          this.boundKeyDown = (e: KeyboardEvent) => {
            if ((e.metaKey || e.ctrlKey) && !this.cmView.state.field(modifierField)) {
              this.cmView.dispatch({ effects: modifierEffect.of(true) });
            }
          };
          this.boundKeyUp = (e: KeyboardEvent) => {
            if (!e.metaKey && !e.ctrlKey && this.cmView.state.field(modifierField)) {
              this.cmView.dispatch({ effects: modifierEffect.of(false) });
            }
          };
          window.addEventListener('keydown', this.boundKeyDown);
          window.addEventListener('keyup', this.boundKeyUp);
        }

        update(update: ViewUpdate) {
          const active = update.state.field(modifierField);
          if (!active) {
            this.decorations = Decoration.none;
            return;
          }
          if (
            update.docChanged ||
            update.viewportChanged ||
            update.transactions.some((tr) => tr.effects.some((e) => e.is(modifierEffect)))
          ) {
            this.decorations = buildTableLinkDecorations(update.view);
          }
        }

        destroy() {
          window.removeEventListener('keydown', this.boundKeyDown);
          window.removeEventListener('keyup', this.boundKeyUp);
        }
      },
      { decorations: (v) => v.decorations },
    );

    return [modifierField, plugin];
  }

  // ── FK / virtual-relation cache ───────────────────────────────────────────

  /** Cache: `${database}.${table}` → ForeignKeyInfo[] */
  const fkCache = new Map<string, ForeignKeyInfo[]>();
  /** Virtual relations loaded once per editor instance */
  let virtualRelations = $state<VirtualRelation[]>([]);

  $effect(() => {
    if (connections.isActive(connectionId)) {
      listVirtualRelations().then((vrs) => {
        virtualRelations = vrs;
      });
    }
  });

  // ── FK search popup state ─────────────────────────────────────────────────

  interface FkSearchState {
    referencedConnectionId: string;
    referencedDatabase: string;
    referencedTable: string;
    referencedColumn: string;
    /** Editor position where the selected value should be inserted */
    insertFrom: number;
    insertTo: number;
    anchorX: number;
    anchorY: number;
  }

  let fkSearchState = $state<FkSearchState | null>(null);

  function openFkSearch(state: FkSearchState): void {
    fkSearchState = state;
  }

  function closeFkSearch(): void {
    fkSearchState = null;
    editorView?.focus();
  }

  function applyFkSelection(value: string): void {
    if (!editorView || !fkSearchState) return;
    editorView.dispatch({
      changes: { from: fkSearchState.insertFrom, to: fkSearchState.insertTo, insert: value },
      selection: { anchor: fkSearchState.insertFrom + value.length },
    });
    closeFkSearch();
  }

  async function loadSchemaForCompletion(connId: string): Promise<void> {
    schemaRef.connectionId = connId;
    schemaRef.tables = [];
    schemaRef.columns = new Map();
    try {
      const dbs = await schemaApi.listDatabases(connId);
      databases = dbs;
      if (!selectedDatabase && dbs.length > 0) {
        selectedDatabase = dbs[0];
      }
      const tables: SchemaTable[] = [];
      for (const db of dbs) {
        const dbTables = await schemaApi.listTables(connId, db);
        for (const t of dbTables) {
          tables.push({ database: db, name: t.name });
        }
      }
      schemaRef.tables = tables;
    } catch {
      // Schema load failed — completions just won't show table/column names.
    }
  }

  $effect(() => {
    if (connections.isActive(connectionId)) {
      loadSchemaForCompletion(connectionId);
    }
  });

  // ── SQL completion context detection ────────────────────────────────────────

  type CompletionCtx =
    | { kind: 'table' }
    | { kind: 'column'; tables: string[] }
    | { kind: 'database' }
    | { kind: 'statement_start' }
    | { kind: 'none' };

  const TABLE_CTX_KEYWORDS = new Set([
    'FROM',
    'JOIN',
    'UPDATE',
    'TABLE',
    'INTO',
    'TRUNCATE',
    'RENAME',
  ]);
  const COLUMN_CTX_KEYWORDS = new Set([
    'SELECT',
    'WHERE',
    'HAVING',
    'SET',
    'ON',
    'AND',
    'OR',
    'NOT',
    'BY',
    'WHEN',
    'THEN',
    'ELSE',
    'RETURNING',
    'DISTINCT',
    'IF',
    'CASE',
  ]);
  const SUPPRESS_CTX_KEYWORDS = new Set(['LIMIT', 'OFFSET', 'AS', 'FETCH', 'ROWS', 'ONLY', 'TIES']);
  const DATABASE_CTX_KEYWORDS = new Set(['USE', 'DATABASE', 'SCHEMA']);

  const SQL_STATEMENT_KEYWORDS = [
    'SELECT',
    'INSERT INTO',
    'UPDATE',
    'DELETE FROM',
    'WITH',
    'CREATE TABLE',
    'CREATE VIEW',
    'DROP TABLE',
    'DROP VIEW',
    'ALTER TABLE',
    'EXPLAIN',
    'BEGIN',
    'COMMIT',
    'ROLLBACK',
    'TRUNCATE',
    'SHOW',
    'DESCRIBE',
    'USE',
  ];

  function detectCompletionCtx(sqlBeforeCursor: string): CompletionCtx {
    // Strip quoted identifiers, string literals, and comments so keywords
    // inside them don't confuse the scanner.
    const clean = sqlBeforeCursor
      .replace(/`[^`]*`/g, ' _ID_ ')
      .replace(/'(?:[^'\\]|\\.)*'/g, ' _STR_ ')
      .replace(/"(?:[^"\\]|\\.)*"/g, ' _STR_ ')
      .replace(/--[^\n]*/g, ' ')
      .replace(/\/\*[\s\S]*?\*\//g, ' ');

    if (!clean.trim()) return { kind: 'statement_start' };

    // Tokenise into words and a few punctuation chars we care about
    const tokens = [...clean.matchAll(/[A-Za-z_]\w*|[();,]/g)].map((m) => m[0]);

    if (tokens.length === 0) return { kind: 'statement_start' };

    // Walk backwards at depth 0 (depth increases inside parens)
    let depth = 0;
    for (let i = tokens.length - 1; i >= 0; i--) {
      const tok = tokens[i].toUpperCase();
      if (tok === ')') {
        depth++;
        continue;
      }
      if (tok === '(') {
        depth = Math.max(0, depth - 1);
        continue;
      }
      if (depth > 0) continue; // inside parenthesised expression — skip

      if (tok === ',') continue; // comma: keep scanning to find which clause we're in
      if (tok === ';') return { kind: 'statement_start' }; // hit previous statement

      if (TABLE_CTX_KEYWORDS.has(tok)) return { kind: 'table' };
      if (SUPPRESS_CTX_KEYWORDS.has(tok)) return { kind: 'none' };
      if (DATABASE_CTX_KEYWORDS.has(tok)) return { kind: 'database' };
      if (COLUMN_CTX_KEYWORDS.has(tok)) {
        return { kind: 'column', tables: extractReferencedTables(sqlBeforeCursor) };
      }
      // Non-keyword token (identifier, operator placeholder) — keep scanning
    }

    return { kind: 'statement_start' };
  }

  function extractReferencedTables(sql: string): string[] {
    const clean = sql
      .replace(/`([^`]*)`/g, '$1')
      .replace(/'[^']*'/g, '')
      .replace(/"[^"]*"/g, '')
      .replace(/--[^\n]*/g, '')
      .replace(/\/\*[\s\S]*?\*\//g, '');

    const tables = new Set<string>();
    const re = /\b(?:FROM|JOIN|UPDATE)\s+(?:\w+\.)?(\w+)/gi;
    let m;
    while ((m = re.exec(clean)) !== null) {
      // Skip subquery keyword "SELECT" which might follow "FROM ("
      if (m[1].toUpperCase() !== 'SELECT') tables.add(m[1].toLowerCase());
    }
    return [...tables];
  }

  /** Load and return column completions for the given table names. */
  async function columnCompletions(
    tableNames: string[],
  ): Promise<{ label: string; detail: string; type: string }[]> {
    const options: { label: string; detail: string; type: string }[] = [];
    const seen = new Set<string>(); // deduplicate column labels

    for (const tableName of tableNames) {
      const schemaTable =
        schemaRef.tables.find(
          (t) =>
            t.name.toLowerCase() === tableName.toLowerCase() &&
            (!selectedDatabase || t.database === selectedDatabase),
        ) ?? schemaRef.tables.find((t) => t.name.toLowerCase() === tableName.toLowerCase());

      if (!schemaTable) continue;

      const cacheKey = `${schemaTable.database}.${schemaTable.name}`;
      let cols = schemaRef.columns.get(cacheKey);
      if (!cols) {
        try {
          const colInfos = await schemaApi.listColumns(
            schemaRef.connectionId,
            schemaTable.database,
            schemaTable.name,
          );
          cols = colInfos.map((c) => ({ name: c.name, dataType: c.dataType }));
          schemaRef.columns.set(cacheKey, cols);
        } catch {
          cols = [];
        }
      }

      for (const col of cols) {
        if (!seen.has(col.name)) {
          seen.add(col.name);
          const detail =
            tableNames.length > 1 ? `${schemaTable.name} · ${col.dataType}` : col.dataType;
          options.push({ label: col.name, detail, type: 'property' });
        }
      }
    }

    return options;
  }

  function makeSchemaCompletionSource(): CompletionSource {
    return async (context: CompletionContext): Promise<CompletionResult | null> => {
      // ── FK value search (highest priority) ──────────────────────────────────
      const fkCtx = getFkValueContext(context.state, context.pos);
      if (fkCtx) {
        const fkCompletion = await buildFkCompletion(context, fkCtx.tableName, fkCtx.columnName);
        if (fkCompletion) return fkCompletion;
      }

      // ── table.column dot-completion ──────────────────────────────────────────
      const dotMatch = context.matchBefore(/[\w"`]+\.[\w"`]*/);
      if (dotMatch) {
        const rawTable = dotMatch.text.split('.')[0].replace(/["`]/g, '');
        const schemaTable = schemaRef.tables.find(
          (t) => t.name.toLowerCase() === rawTable.toLowerCase(),
        );
        if (schemaTable) {
          const cacheKey = `${schemaTable.database}.${schemaTable.name}`;
          let cols = schemaRef.columns.get(cacheKey);
          if (!cols) {
            try {
              const colInfos = await schemaApi.listColumns(
                schemaRef.connectionId,
                schemaTable.database,
                schemaTable.name,
              );
              cols = colInfos.map((c) => ({ name: c.name, dataType: c.dataType }));
              schemaRef.columns.set(cacheKey, cols);
            } catch {
              cols = [];
            }
          }
          return {
            from: dotMatch.from + rawTable.length + 1,
            options: cols.map((col) => ({
              label: col.name,
              detail: col.dataType,
              type: 'property',
            })),
            validFor: /^[\w"`]*$/,
          };
        }
      }

      // ── Context-aware word completion ────────────────────────────────────────
      const word = context.matchBefore(/\w*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;

      // Analyse the SQL before the current word to determine expected token kind
      const textBeforeWord = context.state.doc.sliceString(0, word.from);
      const ctx = detectCompletionCtx(textBeforeWord);

      if (ctx.kind === 'none') return null;

      if (ctx.kind === 'statement_start') {
        return {
          from: word.from,
          options: SQL_STATEMENT_KEYWORDS.map((k) => ({ label: k, type: 'keyword' })),
          validFor: /^\w*$/,
        };
      }

      if (ctx.kind === 'database') {
        return {
          from: word.from,
          options: databases.map((db) => ({ label: db, type: 'constant' })),
          validFor: /^\w*$/,
        };
      }

      if (ctx.kind === 'table') {
        if (schemaRef.tables.length === 0) return null;
        return {
          from: word.from,
          options: schemaRef.tables.map((t) => ({
            label: t.name,
            detail: t.database,
            type: 'class',
          })),
          validFor: /^\w*$/,
        };
      }

      // ctx.kind === 'column'
      if (ctx.tables.length > 0) {
        const cols = await columnCompletions(ctx.tables);
        if (cols.length > 0) {
          return { from: word.from, options: cols, validFor: /^\w*$/ };
        }
      }

      // No referenced tables found or columns not loaded yet — fall back to
      // showing all tables so the user at least sees something useful.
      if (schemaRef.tables.length === 0) return null;
      return {
        from: word.from,
        options: schemaRef.tables.map((t) => ({
          label: t.name,
          detail: t.database,
          type: 'class',
        })),
        validFor: /^\w*$/,
      };
    };
  }

  /**
   * Look up FK/virtual-relation info for `tableName.columnName` and return a
   * CompletionResult with a single "Search [refTable]…" item if a relation exists.
   */
  async function buildFkCompletion(
    context: CompletionContext,
    tableName: string,
    columnName: string,
  ): Promise<CompletionResult | null> {
    // Find which database has this table (use selectedDatabase first, then first match)
    const schemaTable =
      schemaRef.tables.find(
        (t) => t.name.toLowerCase() === tableName.toLowerCase() && t.database === selectedDatabase,
      ) ?? schemaRef.tables.find((t) => t.name.toLowerCase() === tableName.toLowerCase());

    const database = schemaTable?.database ?? selectedDatabase;
    if (!database) return null;

    // Load FK info (cached)
    const cacheKey = `${database}.${tableName}`;
    let fks = fkCache.get(cacheKey);
    if (!fks) {
      try {
        fks = await schemaApi.listForeignKeys(connectionId, database, tableName);
        fkCache.set(cacheKey, fks);
      } catch {
        fks = [];
        fkCache.set(cacheKey, fks);
      }
    }

    // Check real FKs
    const matchingFk = fks.find((fk) =>
      fk.columns.some((c) => c.toLowerCase() === columnName.toLowerCase()),
    );

    let referencedConnectionId: string = connectionId;
    let referencedDatabase: string = database;
    let referencedTable: string | null = null;
    let referencedColumn: string | null = null;

    if (matchingFk) {
      // Real FK: referenced table is always in the same connection/database
      const colIdx = matchingFk.columns.findIndex(
        (c) => c.toLowerCase() === columnName.toLowerCase(),
      );
      referencedTable = matchingFk.referencedTable;
      referencedColumn = matchingFk.referencedColumns[colIdx] ?? matchingFk.referencedColumns[0];
    } else {
      // Check virtual relations — these can point to a different database or connection
      const vr = virtualRelations.find(
        (r) =>
          r.from.connectionId === connectionId &&
          r.from.database === database &&
          r.from.table.toLowerCase() === tableName.toLowerCase() &&
          r.from.column.toLowerCase() === columnName.toLowerCase(),
      );
      if (vr) {
        referencedConnectionId = vr.to.connectionId;
        referencedDatabase = vr.to.database;
        referencedTable = vr.to.table;
        referencedColumn = vr.to.column;
      }
    }

    if (!referencedTable || !referencedColumn) return null;

    // Determine the range of text the completion should replace (current partial value word)
    const word = context.matchBefore(/[\w.'"-]*/);
    const from = word ? word.from : context.pos;

    // Capture in local vars for the closure
    const refConnId = referencedConnectionId;
    const refDb = referencedDatabase;
    const refTable = referencedTable;
    const refColumn = referencedColumn;

    return {
      from,
      filter: false,
      options: [
        {
          label: `Search ${refTable}…`,
          detail: `insert ${refColumn}`,
          type: 'keyword',
          boost: 99,
          apply: (view, _completion, applyFrom, applyTo) => {
            const coords = view.coordsAtPos(applyFrom);
            openFkSearch({
              referencedConnectionId: refConnId,
              referencedDatabase: refDb,
              referencedTable: refTable,
              referencedColumn: refColumn,
              insertFrom: applyFrom,
              insertTo: applyTo,
              anchorX: coords ? coords.left : window.innerWidth / 2,
              anchorY: coords ? coords.bottom : window.innerHeight / 2,
            });
          },
        },
      ],
      validFor: /^[\w.'"-]*$/,
    };
  }

  // ── Theme ─────────────────────────────────────────────────────────────────

  function resolveCSSVar(name: string, fallback = ''): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
  }

  function buildHighlightStyle(): HighlightStyle {
    return HighlightStyle.define([
      { tag: tags.keyword, color: resolveCSSVar('--color-editor-keyword') },
      { tag: tags.string, color: resolveCSSVar('--color-editor-string') },
      { tag: tags.number, color: resolveCSSVar('--color-editor-number') },
      { tag: tags.comment, color: resolveCSSVar('--color-editor-comment') },
      { tag: tags.operator, color: resolveCSSVar('--color-editor-operator') },
      {
        tag: [tags.function(tags.variableName), tags.function(tags.propertyName)],
        color: resolveCSSVar('--color-editor-function'),
      },
      { tag: [tags.typeName, tags.className], color: resolveCSSVar('--color-editor-type') },
    ]);
  }

  function buildTheme(): Extension {
    return EditorView.theme({
      '&': {
        backgroundColor: resolveCSSVar('--color-editor-bg'),
        color: resolveCSSVar('--color-editor-text'),
        height: '100%',
        fontFamily: 'var(--font-family-mono)',
        fontSize: 'var(--font-size-md)',
      },
      '.cm-content': {
        caretColor: resolveCSSVar('--color-editor-cursor'),
        padding: '0',
      },
      '.cm-cursor, .cm-dropCursor': {
        borderLeftColor: resolveCSSVar('--color-editor-cursor'),
      },
      '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground':
        {
          backgroundColor: 'var(--color-editor-selection)',
        },
      '.cm-gutters': {
        backgroundColor: resolveCSSVar('--color-editor-gutter-bg'),
        color: resolveCSSVar('--color-editor-gutter-text'),
        border: 'none',
        borderRight: `1px solid var(--color-border)`,
      },
      '.cm-lineNumbers .cm-gutterElement': {
        padding: '0 var(--spacing-2)',
        minWidth: '2.5ch',
      },
      '.cm-activeLine': {
        backgroundColor: resolveCSSVar('--color-editor-active-line'),
      },
      '.cm-activeLineGutter': {
        backgroundColor: resolveCSSVar('--color-editor-active-line'),
      },
      '.cm-matchingBracket': {
        backgroundColor: resolveCSSVar('--color-editor-bracket-match'),
        outline: 'none',
      },
      '.cm-scroller': {
        overflow: 'auto',
        fontFamily: 'var(--font-family-mono)',
      },
      // Error highlighting
      '.cm-lintRange-error': {
        backgroundImage: 'none',
        borderBottom: `2px solid ${resolveCSSVar('--color-danger')}`,
        paddingBottom: '1px',
      },
      '.cm-lintRange-warning': {
        backgroundImage: 'none',
        borderBottom: `2px solid ${resolveCSSVar('--color-warning')}`,
        paddingBottom: '1px',
      },
      '.cm-gutter-lint': {
        width: '16px',
      },
      '.cm-gutter-lint .cm-gutterElement': {
        padding: '0',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      },
      '.cm-lint-marker': {
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: '8px',
        height: '8px',
      },
      '.cm-lint-marker-error': {
        content: '""',
        display: 'block',
        width: '8px',
        height: '8px',
        borderRadius: '50%',
        backgroundColor: resolveCSSVar('--color-danger'),
      },
      '.cm-tooltip.cm-tooltip-lint': {
        background: 'var(--color-bg-overlay)',
        border: '1px solid var(--color-border-strong)',
        borderRadius: 'var(--radius-md)',
        boxShadow: 'var(--shadow-overlay)',
        backdropFilter: 'blur(20px) saturate(160%)',
        maxWidth: '420px',
      },
      '.cm-diagnosticText': {
        fontFamily: 'var(--font-family-mono)',
        fontSize: 'var(--font-size-xs)',
        color: 'var(--color-text-primary)',
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
        lineHeight: '1.5',
        padding: '6px 8px',
        display: 'block',
      },
      // Tooltip shell — matches .select-dropdown
      '.cm-tooltip': {
        background: 'var(--color-bg-overlay)',
        border: '1px solid var(--color-border-strong)',
        borderRadius: 'var(--radius-md)',
        boxShadow: 'var(--shadow-overlay)',
      },
      '.cm-tooltip.cm-tooltip-autocomplete': {
        padding: '3px',
        backdropFilter: 'blur(20px) saturate(160%)',
      },
      // List element — reset browser defaults
      '.cm-tooltip-autocomplete ul': {
        fontFamily: 'var(--font-family-ui)',
        fontSize: 'var(--font-size-sm)',
        padding: '0',
        margin: '0',
        listStyle: 'none',
        maxHeight: '260px',
        overflowY: 'auto',
      },
      // Each completion row — matches .option
      '.cm-tooltip-autocomplete ul li': {
        display: 'flex',
        alignItems: 'center',
        padding: '0 8px',
        minHeight: '26px',
        borderRadius: 'var(--radius-sm)',
        color: 'var(--color-text-primary)',
        cursor: 'pointer',
        whiteSpace: 'nowrap',
        transition: 'background var(--transition-fast)',
        background: 'transparent',
      },
      // Focused/selected row — matches .option--focused
      '.cm-tooltip-autocomplete ul li[aria-selected]': {
        background: 'var(--color-accent-subtle)',
        color: 'var(--color-text-primary)',
      },
      // Type icon — hidden; we rely on label+detail to convey type
      '.cm-completionIcon': {
        display: 'none',
      },
      // Main label — monospace since it's a SQL token
      '.cm-completionLabel': {
        fontFamily: 'var(--font-family-mono)',
        fontSize: 'var(--font-size-xs)',
        color: 'var(--color-text-primary)',
        flex: '1',
        overflow: 'hidden',
        textOverflow: 'ellipsis',
      },
      // Type / database hint — muted, ui font, right-aligned
      '.cm-completionDetail': {
        fontFamily: 'var(--font-family-ui)',
        fontSize: '10px',
        color: 'var(--color-text-muted)',
        fontStyle: 'normal',
        marginLeft: '8px',
        flexShrink: '0',
      },
    });
  }

  // ── Editor navigation ────────────────────────────────────────────────────

  function findStatementStart(sql: string, statements: string[], targetIndex: number): number {
    let searchFrom = 0;
    for (let i = 0; i <= targetIndex; i++) {
      const stmt = statements[i]?.trim();
      if (!stmt) continue;
      const pos = sql.indexOf(stmt, searchFrom);
      if (pos === -1) return 0;
      if (i === targetIndex) return pos;
      searchFrom = pos + stmt.length;
    }
    return 0;
  }

  function scrollEditorToStatement(index: number): void {
    if (!editorView || executedStatements.length === 0) return;
    const pos = findStatementStart(sqlText, executedStatements, index);
    editorView.dispatch({ selection: { anchor: pos } });
    const coords = editorView.coordsAtPos(pos);
    if (coords) {
      const scrollDOM = editorView.scrollDOM;
      const targetTop = scrollDOM.scrollTop + coords.top - scrollDOM.getBoundingClientRect().top;
      scrollDOM.scrollTo({ top: targetTop, behavior: 'smooth' });
    }
  }

  // ── Query execution ───────────────────────────────────────────────────────

  async function runQuery(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;

    isRunning = true;
    executedStatements = splitStatements(query);
    executedSql = query;
    try {
      results = await executeMultiQuery(
        connectionId,
        stripLineComments(query),
        selectedDatabase || null,
        editorId,
      );
      if (connections.isTransactionActive(connectionId))
        connections.addTxQuery(connectionId, query);
      recording.add(query, connectionId, selectedDatabase || null);
      if (
        revertStore.isRevertingConnection(connectionId) &&
        splitStatements(query).some(isMutatingStatement)
      ) {
        revertStore.add({
          id: crypto.randomUUID(),
          source: 'query',
          connectionId,
          database: selectedDatabase || '',
          table: '',
          sql: query,
          revertSql: '',
          rows: [],
          executedAt: new Date(),
          reverted: false,
        });
      }
      onExecute?.(query);
      await fetchVariableValues();
      if (settingsStore.settings.saveOnRun && currentSavedQueryId && savedSql !== sqlText) {
        await saveQuery();
      }
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationUs: 0,
          affectedRows: null,
          error: errorMessage(err),
        },
      ];
    } finally {
      isRunning = false;
    }
  }

  async function runSelection(): Promise<void> {
    if (!editorView || isRunning) return;
    const state = editorView.state;
    const { from, to } = state.selection.main;
    const selected = from === to ? sqlText : state.sliceDoc(from, to);
    const query = selected.trim();
    if (!query) return;

    isRunning = true;
    executedStatements = splitStatements(query);
    executedSql = query;
    try {
      results = await executeMultiQuery(
        connectionId,
        stripLineComments(query),
        selectedDatabase || null,
        editorId,
      );
      if (connections.isTransactionActive(connectionId))
        connections.addTxQuery(connectionId, query);
      recording.add(query, connectionId, selectedDatabase || null);
      if (
        revertStore.isRevertingConnection(connectionId) &&
        splitStatements(query).some(isMutatingStatement)
      ) {
        revertStore.add({
          id: crypto.randomUUID(),
          source: 'query',
          connectionId,
          database: selectedDatabase || '',
          table: '',
          sql: query,
          revertSql: '',
          rows: [],
          executedAt: new Date(),
          reverted: false,
        });
      }
      onExecute?.(query);
      await fetchVariableValues();
      if (settingsStore.settings.saveOnRun && currentSavedQueryId && savedSql !== sqlText) {
        await saveQuery();
      }
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationUs: 0,
          affectedRows: null,
          error: errorMessage(err),
        },
      ];
    } finally {
      isRunning = false;
    }
  }

  async function runUnderCursor(): Promise<void> {
    if (!editorView || isRunning) return;
    const pos = editorView.state.selection.main.head;
    const stmt = statementAtCursor(sqlText, pos);
    if (!stmt.trim()) return;

    isRunning = true;
    executedStatements = [stmt];
    executedSql = stmt;
    try {
      results = await executeMultiQuery(
        connectionId,
        stripLineComments(stmt),
        selectedDatabase || null,
        editorId,
      );
      if (connections.isTransactionActive(connectionId)) connections.addTxQuery(connectionId, stmt);
      recording.add(stmt, connectionId, selectedDatabase || null);
      if (revertStore.isRevertingConnection(connectionId) && isMutatingStatement(stmt)) {
        revertStore.add({
          id: crypto.randomUUID(),
          source: 'query',
          connectionId,
          database: selectedDatabase || '',
          table: '',
          sql: stmt,
          revertSql: '',
          rows: [],
          executedAt: new Date(),
          reverted: false,
        });
      }
      onExecute?.(stmt);
      await fetchVariableValues();
      if (settingsStore.settings.saveOnRun && currentSavedQueryId && savedSql !== sqlText) {
        await saveQuery();
      }
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationUs: 0,
          affectedRows: null,
          error: errorMessage(err),
        },
      ];
    } finally {
      isRunning = false;
    }
  }

  // Returns true if the line contains a -- comment outside string literals.
  // Used to avoid joining subsequent SQL onto the same line as a -- comment.
  function lineHasLineComment(line: string): boolean {
    const stripped = line
      .replace(/'(?:[^'\\]|\\.)*'/g, '')
      .replace(/"(?:[^"\\]|\\.)*"/g, '')
      .replace(/`[^`]*/g, '');
    return stripped.includes('--');
  }

  function formatQuery(): void {
    if (!editorView) return;
    const dialect = sqlDialect();
    const s = settingsStore.settings;
    const style = s.formatStyle ?? (s.formatCompact ? 'compact' : 'expanded');
    const options: Parameters<typeof sqlFormat>[1] = {
      language: dialect as NonNullable<Parameters<typeof sqlFormat>[1]>['language'],
      keywordCase: s.formatKeywordCase,
      indentStyle: style === 'comfortable' ? 'standard' : s.formatIndentStyle,
      linesBetweenQueries: s.formatLinesBetweenQueries,
    };
    try {
      let formatted = sqlFormat(sqlText, options);
      if (style === 'compact') {
        const separator = ';\n' + '\n'.repeat(s.formatLinesBetweenQueries);
        // Process line by line so -- comment lines are never joined with the
        // following SQL (which would hide the SQL inside the comment).
        const resultParts: string[] = [];
        let sqlBuffer = '';
        const flushBuffer = () => {
          const trimmed = sqlBuffer.trim();
          if (!trimmed) return;
          resultParts.push(trimmed.replace(/\s*;\s*/g, separator).trimEnd());
          sqlBuffer = '';
        };
        for (const rawLine of [...formatted.split(/\n/), null]) {
          const line = rawLine !== null ? rawLine.trim() : null;
          if (line === null) {
            flushBuffer();
            break;
          }
          if (!line) continue;
          if (lineHasLineComment(line)) {
            // Append comment line to buffer, then flush — nothing can follow on same line
            sqlBuffer += (sqlBuffer ? ' ' : '') + line;
            flushBuffer();
          } else {
            sqlBuffer += (sqlBuffer ? ' ' : '') + line;
          }
        }
        formatted = resultParts.join('\n').trim();
      } else if (style === 'comfortable') {
        const THRESHOLD = 80;
        const blankSep = '\n'.repeat(s.formatLinesBetweenQueries + 1);
        formatted = formatted
          .split(/\n{2,}/)
          .map((stmt) => {
            const trimmed = stmt.trim();
            const origLines = trimmed.split('\n');
            const flatLines = origLines.map((l) => l.trim()).filter(Boolean);
            if (!flatLines.some(lineHasLineComment)) {
              const oneLiner = flatLines.join(' ');
              return oneLiner.length <= THRESHOLD ? oneLiner : trimmed;
            }
            // Block contains -- comments: process each SQL run between comments
            // with the same threshold logic as the comment-free path.
            const resultParts: string[] = [];
            let sqlFlat: string[] = [];
            let sqlOrig: string[] = [];
            for (const origLine of [...origLines, null]) {
              const flat = origLine !== null ? origLine.trim() : null;
              if (flat === null || (flat && lineHasLineComment(flat))) {
                if (sqlFlat.length > 0) {
                  const oneLiner = sqlFlat.join(' ');
                  resultParts.push(
                    oneLiner.length <= THRESHOLD ? oneLiner : sqlOrig.join('\n').trim(),
                  );
                  sqlFlat = [];
                  sqlOrig = [];
                }
                if (flat) resultParts.push(flat);
              } else if (flat) {
                sqlFlat.push(flat);
                sqlOrig.push(origLine as string);
              }
            }
            return resultParts.join('\n');
          })
          .join(blankSep);
      }
      editorView.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
      });
    } catch {
      // sql-formatter can fail on certain comment styles. Strip block/line
      // comments and retry — this preserves structure even if comments are lost.
      try {
        const stripped = sqlText.replace(/\/\*[\s\S]*?\*\//g, '').replace(/--[^\n]*/g, '');
        const formatted = sqlFormat(stripped, options);
        editorView.dispatch({
          changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
        });
      } catch {
        // If even the stripped version fails, leave content unchanged.
      }
    }
  }

  async function runExplain(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;
    isRunning = true;
    try {
      const explainResult = await explainQuery(connectionId, query, selectedDatabase || null);
      panelStore.openInFocused({
        kind: 'explain',
        connectionId,
        sql: explainResult.rawJson,
        dialect: explainResult.dialect,
      });
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationUs: 0,
          affectedRows: null,
          error: errorMessage(err),
        },
      ];
    } finally {
      isRunning = false;
    }
  }

  // ── Shortcut action listener ──────────────────────────────────────────────

  function handleShortcutAction(e: Event) {
    const { action } = (e as CustomEvent<{ action: string }>).detail;
    switch (action) {
      case 'QUERY_RUN_ALL':
        runQuery();
        break;
      case 'QUERY_RUN_SELECTION':
        runSelection();
        break;
      case 'QUERY_RUN_UNDER_CURSOR':
        runUnderCursor();
        break;
      case 'QUERY_FORMAT':
        formatQuery();
        break;
      case 'QUERY_EXPLAIN':
        runExplain();
        break;
      case 'FOCUS_EDITOR':
        if (isFocusedPanel) editorView?.focus();
        break;
      case 'FOCUS_RESULTS':
        if (isFocusedPanel) resultsWrapperEl?.focus();
        break;
    }
  }

  // Cmd+Enter runs the query from within the editor.
  function runQueryCommand(): boolean {
    runQuery();
    return true;
  }

  let unlistenShortcut: (() => void) | null = null;
  let unlistenQueryCount: (() => void) | null = null;

  onMount(async () => {
    unlistenQueryCount = await listen<{ queryId: string; totalRows: number }>(
      'query-count-updated',
      (event) => {
        const { queryId, totalRows } = event.payload;
        const idx = results.findIndex((r) => r.queryId === queryId);
        if (idx !== -1) {
          results = results.map((r, i) => (i === idx ? { ...r, totalRows } : r));
        }
      },
    );
  });

  onMount(() => {
    if (!editorContainer) return;

    // Set up note callbacks before creating the editor (widgets reference these via closure)
    noteCallbacksRef.onUpdate = (id: string, text: string) => {
      inlineNotes = inlineNotes.map((n) => (n.id === id ? { ...n, text } : n));
    };
    noteCallbacksRef.onRemove = (id: string) => {
      inlineNotes = inlineNotes.filter((n) => n.id !== id);
      notesStructureVersion++;
    };
    gutterClickRef.onClick = (lineNumber: number, x: number, y: number) => {
      noteMenu = { lineNumber, x, y };
    };

    document.addEventListener('shortcut-action', handleShortcutAction);
    unlistenShortcut = () => document.removeEventListener('shortcut-action', handleShortcutAction);

    const runKeymap = keymap.of([
      { key: 'Mod-Enter', run: runQueryCommand },
      {
        key: 'Mod-Shift-Enter',
        run: () => {
          runSelection();
          return true;
        },
      },
      {
        key: 'Mod-Shift-r',
        run: () => {
          runUnderCursor();
          return true;
        },
      },
      {
        key: 'Mod-Shift-f',
        run: () => {
          formatQuery();
          return true;
        },
      },
      {
        key: 'Alt-e',
        run: () => {
          runExplain();
          return true;
        },
      },
    ]);

    const state = EditorState.create({
      doc: sqlText,
      extensions: [
        buildTheme(),
        lineNumbers(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        foldGutter(),
        drawSelection(),
        dropCursor(),
        rectangularSelection(),
        crosshairCursor(),
        highlightSelectionMatches(),
        syntaxHighlighting(buildHighlightStyle()),
        bracketMatching(),
        closeBrackets(),
        autocompletion({ override: [makeSchemaCompletionSource()] }),
        sqlLang(),
        runKeymap,
        history(),
        keymap.of([
          ...historyKeymap,
          ...defaultKeymap,
          ...completionKeymap,
          ...closeBracketsKeymap,
          indentWithTab,
        ]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            sqlText = update.state.doc.toString();
          }
        }),
        EditorView.domEventHandlers({
          mousedown(event, view) {
            if (!event.metaKey && !event.ctrlKey) return false;

            const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
            if (pos === null) return false;

            const unquoteIdent = (s: string) => s.replace(/^[`"'[]|[`"'\]]$/g, '');
            let node = syntaxTree(view.state).resolveInner(pos, -1);

            // Promote a child of CompositeIdentifier up to the CompositeIdentifier itself
            if (node.parent?.name === 'CompositeIdentifier') node = node.parent;

            let tableName: string | null = null;
            if (node.name === 'CompositeIdentifier') {
              const parts = view.state.sliceDoc(node.from, node.to).split('.');
              tableName = unquoteIdent(parts[parts.length - 1]);
            } else if (node.name === 'Identifier' || node.name === 'QuotedIdentifier') {
              tableName = unquoteIdent(view.state.sliceDoc(node.from, node.to));
            }

            if (!tableName || !isTableNamePosition(view, node)) return false;

            const matched = schemaRef.tables.find(
              (t) => t.name.toLowerCase() === tableName!.toLowerCase(),
            );
            if (!matched) return false;

            event.preventDefault();
            panelStore.openInFocused({
              kind: 'table_browser',
              connectionId,
              database: matched.database,
              table: matched.name,
            });
            return true;
          },
          contextmenu(event, view) {
            event.preventDefault();
            const state = view.state;
            const { from, to } = state.selection.main;
            const hasSelection = from !== to;
            const selectionSql = hasSelection ? state.sliceDoc(from, to).trim() : '';
            const pos =
              view.posAtCoords({ x: event.clientX, y: event.clientY }) ?? state.selection.main.head;
            const cursorSql = statementAtCursor(sqlText, pos);
            editorContextMenu = {
              x: event.clientX,
              y: event.clientY,
              hasSelection,
              selectionSql,
              cursorSql,
            };
            return true;
          },
        }),
        ...makeTableHighlightExtensions(),
        lintGutter(),
        cmTooltips({ tooltipSpace: (view) => view.dom.getBoundingClientRect() }),
        notesField,
        notesDecoField,
        makeNotesGutter(),
      ],
    });

    editorView = new EditorView({ state, parent: editorContainer });
    editorView.focus();

    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });

  onDestroy(() => {
    unlistenShortcut?.();
    unlistenQueryCount?.();
    if (editorId) sessionRelease(editorId).catch(() => {});
  });

  // ── Error highlighting ────────────────────────────────────────────────────

  $effect(() => {
    // Clear diagnostics whenever the SQL text changes (user is editing)
    sqlText;
    const view = editorView;
    if (!view) return;
    view.dispatch(setDiagnostics(view.state, []));
  });

  $effect(() => {
    const view = editorView;
    if (!view || results.length === 0) return;

    // If the SQL has changed since the last run (e.g. formatting, editing),
    // positions would be wrong — clear and don't re-highlight.
    if (sqlText !== executedSql) {
      view.dispatch(setDiagnostics(view.state, []));
      return;
    }

    const errorInputs = results
      .map((result, i) => {
        if (!result.error) return null;
        const stmt = executedStatements[i] ?? executedStatements[0] ?? sqlText;
        const offset = executedStatements[i]
          ? findStatementStart(sqlText, executedStatements, i)
          : 0;
        return { error: result.error, statement: stmt, statementOffset: offset };
      })
      .filter((x): x is NonNullable<typeof x> => x !== null);

    if (errorInputs.length === 0) {
      view.dispatch(setDiagnostics(view.state, []));
      return;
    }

    const docLen = view.state.doc.length;
    const diagnostics = buildDiagnosticsFromErrors(errorInputs).map((d) => ({
      ...d,
      from: Math.max(0, Math.min(d.from, docLen)),
      to: Math.max(0, Math.min(d.to, docLen)),
    }));

    view.dispatch(setDiagnostics(view.state, diagnostics));
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="query-editor-panel"
  bind:this={editorPanelEl}
  onpointermove={onResizerPointerMove}
  onpointerup={onResizerPointerUp}
  onpointercancel={onResizerPointerUp}
>
  <div class="toolbar" bind:this={toolbarEl}>
    <button
      class="run-button"
      onclick={runQuery}
      disabled={isRunning}
      title="Run query (Cmd+Enter)"
      aria-label="Run query"
    >
      ▶
    </button>

    <span class="connection-badge" title={connectionName}>
      {connectionName}
    </span>

    {#if databases.length > 0}
      <Select
        bind:value={selectedDatabase}
        options={databases.map((db) => ({ value: db, label: db }))}
        size="xs"
        aria-label="Select database"
        mono
        searchable
      />
    {:else if selectedDatabase}
      <span class="connection-badge" title="Database">{selectedDatabase}</span>
    {/if}

    <div class="toolbar-spacer"></div>

    {#if currentSavedQueryId && !compact}
      <button
        class="toolbar-btn"
        onclick={saveQueryAs}
        disabled={isSaving}
        title="Save a copy with a new name"
      >
        Save As…
      </button>
    {/if}

    <button
      bind:this={saveDialogTriggerEl}
      class="toolbar-btn toolbar-btn--save"
      onclick={saveQuery}
      disabled={isSaving}
      title={currentSavedQueryId ? 'Save query' : 'Save query as…'}
      aria-label="Save query"
    >
      <SaveIcon width={13} height={13} />
    </button>

    <button
      class="toolbar-btn toolbar-btn--icon toolbar-btn--description"
      class:toolbar-btn--description-active={descriptionOpen}
      onclick={() => {
        descriptionOpen = !descriptionOpen;
      }}
      title={descriptionOpen ? 'Hide description' : 'Show description'}
      aria-label="Toggle description"
      aria-pressed={descriptionOpen}
    >
      <FileDocIcon size={13} />
    </button>

    {#if saveDialogOpen}
      <div
        bind:this={saveDialogEl}
        class="save-dialog"
        role="dialog"
        aria-label="Save query"
        style="top:{saveDialogTop}px;left:{saveDialogLeft}px"
        use:portal
      >
        <form
          onsubmit={(e) => {
            e.preventDefault();
            confirmSave();
          }}
        >
          <input
            bind:this={saveNameInputEl}
            bind:value={saveNameInput}
            class="save-name-input"
            type="text"
            placeholder="Query name"
            maxlength="120"
            autocomplete="off"
            spellcheck={false}
            onkeydown={(e) => {
              if (e.key === 'Escape') saveDialogOpen = false;
            }}
          />
          <button
            type="submit"
            class="save-confirm-btn"
            disabled={!saveNameInput.trim() || isSaving}
          >
            Save
          </button>
        </form>
      </div>
    {/if}

    {#if compact}
      <button
        bind:this={actionsMenuTriggerEl}
        class="toolbar-btn toolbar-btn--icon"
        onclick={() => {
          actionsMenuOpen = !actionsMenuOpen;
        }}
        title="More actions"
        aria-label="More actions"
        aria-expanded={actionsMenuOpen}
        aria-haspopup="menu"
      >
        <DotsIcon width={14} height={14} />
      </button>

      {#if actionsMenuOpen}
        <div
          bind:this={actionsMenuEl}
          class="actions-menu"
          role="menu"
          style="top:{actionsMenuTop}px;left:{actionsMenuLeft}px"
          use:portal
        >
          <button
            class="actions-menu-item"
            role="menuitem"
            onclick={() => {
              runSelection();
              actionsMenuOpen = false;
            }}
            disabled={isRunning}
          >
            <span>Run Selection</span>
            <kbd>⇧↵</kbd>
          </button>
          <button
            class="actions-menu-item"
            role="menuitem"
            onclick={() => {
              runUnderCursor();
              actionsMenuOpen = false;
            }}
            disabled={isRunning}
          >
            <span>Run Cursor</span>
            <kbd>⇧R</kbd>
          </button>
          <div class="actions-menu-sep" role="separator"></div>
          <button
            class="actions-menu-item"
            role="menuitem"
            onclick={() => {
              formatQuery();
              actionsMenuOpen = false;
            }}
          >
            <span>Format SQL</span>
            <kbd>⇧F</kbd>
          </button>
          <button
            class="actions-menu-item"
            role="menuitem"
            onclick={() => {
              runExplain();
              actionsMenuOpen = false;
            }}
            disabled={isRunning}
          >
            <span>Explain</span>
          </button>
          {#if currentSavedQueryId}
            <div class="actions-menu-sep" role="separator"></div>
            <button
              class="actions-menu-item"
              role="menuitem"
              onclick={() => {
                saveQueryAs();
                actionsMenuOpen = false;
              }}
              disabled={isSaving}
            >
              <span>Save As…</span>
            </button>
          {/if}
        </div>
      {/if}
    {:else}
      <button
        class="toolbar-btn"
        onclick={runSelection}
        disabled={isRunning}
        title="Run selection (Cmd+Shift+Enter)"
      >
        Run Selection
      </button>

      <button
        class="toolbar-btn"
        onclick={runUnderCursor}
        disabled={isRunning}
        title="Run statement under cursor (Cmd+Shift+R)"
      >
        Run Cursor
      </button>

      <button class="toolbar-btn" onclick={formatQuery} title="Format SQL (Cmd+Shift+F)">
        Format
      </button>

      <button class="toolbar-btn" onclick={runExplain} disabled={isRunning} title="Explain query">
        Explain
      </button>
      <button
        class="toolbar-btn"
        onclick={openChartForCurrentResult}
        disabled={!results[activeResultTab]?.rows?.length}
        title="Chart results"
      >
        Chart
      </button>
    {/if}
  </div>

  {#if txDatabaseMismatch}
    <div class="tx-db-warning" role="alert">
      <WarningIcon width={13} height={13} />
      Transaction started on <code>{txDatabaseMismatch}</code> — query will run on
      <code>{txDatabaseMismatch}</code>, not <code>{selectedDatabase}</code>
    </div>
  {/if}

  {#if descriptionOpen}
    <div class="description-panel">
      <textarea
        class="description-textarea"
        bind:value={descriptionText}
        placeholder="Add a description for this query…"
        spellcheck={false}
        aria-label="Query description"
      ></textarea>
    </div>
  {/if}

  <div class="editor-area" style="flex-basis: {editorHeightPct}%">
    <div class="editor-wrapper">
      <div class="editor-container" bind:this={editorContainer}></div>
    </div>

    {#if sqlVariableNames.length > 0}
      <div
        class="vars-panel"
        class:vars-panel--open={varsOpen}
        role="region"
        aria-label="SQL variables"
      >
        <button
          class="vars-panel-tab"
          onclick={() => (varsOpen = !varsOpen)}
          title={varsOpen ? 'Collapse variables panel' : 'Expand variables panel'}
          aria-expanded={varsOpen}
        >
          <span class="vars-panel-tab-label">Vars</span>
          <ChevronIcon
            class="vars-panel-chevron{varsOpen ? ' vars-panel-chevron--open' : ''}"
            direction="left"
            width={10}
            height={10}
            strokeWidth={1.8}
          />
        </button>

        {#if varsOpen}
          <div class="vars-panel-body">
            {#each sqlVariableNames as varName}
              <div class="var-row">
                <code
                  class="var-name"
                  title="Session variables are connection-scoped. Values shown are from the last execution on the same connection."
                  >{varName}</code
                >
                <span class="var-eq">=</span>
                <code
                  class="var-value"
                  class:var-value--null={variableValues[varName] === null}
                  class:var-value--unset={!(varName in variableValues)}
                >
                  {#if !(varName in variableValues)}
                    —
                  {:else if variableValues[varName] === null}
                    NULL
                  {:else}
                    {variableValues[varName]}
                  {/if}
                </code>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div
    class="editor-resizer"
    class:editor-resizer--active={isResizingEditor}
    role="slider"
    aria-label="Editor height"
    aria-orientation="vertical"
    aria-valuenow={Math.round(editorHeightPct)}
    aria-valuemin={15}
    aria-valuemax={80}
    tabindex="0"
    onpointerdown={onResizerPointerDown}
    onkeydown={(e) => {
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        editorHeightPct = Math.max(15, editorHeightPct - 2);
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        editorHeightPct = Math.min(80, editorHeightPct + 2);
      }
    }}
  ></div>

  <div class="results-wrapper" bind:this={resultsWrapperEl} tabindex="-1">
    <ResultsPanel
      {results}
      statements={executedStatements}
      {connectionId}
      database={selectedDatabase || undefined}
      {isRunning}
      {variableValues}
      initialActiveTab={activeResultTab}
      onActiveTabChange={(tab) => {
        activeResultTab = tab;
        scrollEditorToStatement(tab);
      }}
    />
  </div>
</div>

{#if fkSearchState}
  <FkSearchPopup
    connectionId={fkSearchState.referencedConnectionId}
    database={fkSearchState.referencedDatabase}
    referencedTable={fkSearchState.referencedTable}
    referencedColumn={fkSearchState.referencedColumn}
    anchorX={fkSearchState.anchorX}
    anchorY={fkSearchState.anchorY}
    onselect={applyFkSelection}
    onclose={closeFkSearch}
  />
{/if}

{#if noteMenu !== null}
  <div
    class="note-menu-backdrop"
    role="presentation"
    onmousedown={() => {
      noteMenu = null;
    }}
    use:portal
  ></div>
  <div class="note-menu" style="top:{noteMenu.y}px;left:{noteMenu.x}px" use:portal>
    {#if settingsStore.settings.aiProvider !== 'none'}
      <button
        class="note-menu-item note-menu-item--ai"
        onmousedown={() => {
          aiModal = { mode: 'generate', insertLine: noteMenu!.lineNumber };
          noteMenu = null;
        }}
      >
        Generate with AI…
      </button>
      <div class="note-menu-separator"></div>
    {/if}
    <button
      class="note-menu-item note-menu-item--builder"
      onmousedown={() => openQueryBuilder(noteMenu!.lineNumber)}
    >
      Build query…
    </button>
    <div class="note-menu-separator"></div>
    <button class="note-menu-item" onmousedown={() => addNote(noteMenu!.lineNumber, 'above')}>
      Add note above line
    </button>
    <button class="note-menu-item" onmousedown={() => addNote(noteMenu!.lineNumber, 'below')}>
      Add note below line
    </button>
  </div>
{/if}

{#if queryBuilderLine !== null}
  {@const qbLine = queryBuilderLine}
  <QueryBuilderModal
    tables={schemaRef.tables}
    loadColumns={async (db, table) => {
      const key = `${db}.${table}`;
      const cached = schemaRef.columns.get(key);
      if (cached) return cached;
      const colInfos = await schemaApi.listColumns(schemaRef.connectionId, db, table);
      const cols = colInfos.map((c) => ({ name: c.name, dataType: c.dataType }));
      schemaRef.columns.set(key, cols);
      return cols;
    }}
    defaultDatabase={selectedDatabase}
    oninsert={(sql) => insertSqlAtLine(qbLine, sql)}
    onclose={() => {
      queryBuilderLine = null;
    }}
  />
{/if}

{#if editorContextMenu !== null}
  {@const ctx = editorContextMenu}
  <div
    class="note-menu-backdrop"
    role="presentation"
    onmousedown={() => {
      editorContextMenu = null;
    }}
    use:portal
  ></div>
  <div class="note-menu" style="top:{ctx.y}px;left:{ctx.x}px" use:portal role="menu">
    {#if ctx.hasSelection}
      <button
        class="note-menu-item"
        role="menuitem"
        disabled={isRunning}
        onmousedown={() => {
          editorContextMenu = null;
          runSelection();
        }}>Run selection</button
      >
      {#if settingsStore.settings.aiProvider !== 'none'}
        <button
          class="note-menu-item note-menu-item--ai"
          role="menuitem"
          onmousedown={() => {
            const sql = ctx.selectionSql;
            editorContextMenu = null;
            aiModal = { mode: 'explain', sql };
          }}>Explain selection</button
        >
      {/if}
    {:else}
      <button
        class="note-menu-item"
        role="menuitem"
        disabled={isRunning}
        onmousedown={() => {
          editorContextMenu = null;
          runUnderCursor();
        }}>Run query under cursor</button
      >
      {#if settingsStore.settings.aiProvider !== 'none'}
        <button
          class="note-menu-item note-menu-item--ai"
          role="menuitem"
          onmousedown={() => {
            const sql = ctx.cursorSql;
            editorContextMenu = null;
            aiModal = { mode: 'explain', sql };
          }}>Explain query under cursor</button
        >
      {/if}
    {/if}
    <div class="note-menu-separator" role="separator"></div>
    <button
      class="note-menu-item"
      role="menuitem"
      disabled={isRunning}
      onmousedown={() => {
        editorContextMenu = null;
        runQuery();
      }}>Run all</button
    >
    {#if settingsStore.settings.aiProvider !== 'none'}
      <button
        class="note-menu-item note-menu-item--ai"
        role="menuitem"
        onmousedown={() => {
          const sql = sqlText;
          editorContextMenu = null;
          aiModal = { mode: 'explain', sql };
        }}>Explain all</button
      >
    {/if}
  </div>
{/if}

{#if aiModal !== null}
  {#if aiModal.mode === 'generate'}
    {@const insertLine = aiModal.insertLine}
    <AiModal
      mode="generate"
      {connectionId}
      database={selectedDatabase}
      oninsert={(sql) => {
        insertSqlAtLine(insertLine, sql);
        aiModal = null;
      }}
      onclose={() => {
        aiModal = null;
      }}
    />
  {:else}
    {@const sql = aiModal.sql}
    <AiModal
      mode="explain"
      {sql}
      {connectionId}
      database={selectedDatabase}
      onclose={() => {
        aiModal = null;
      }}
    />
  {/if}
{/if}

<style>
  .query-editor-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-editor-bg);
  }

  .query-editor-panel:has(.editor-resizer--active) {
    cursor: row-resize;
    user-select: none;
  }

  .toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
  }

  .run-button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--spacing-6);
    height: var(--spacing-6);
    padding: 0;
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    cursor: pointer;
    transition: background var(--transition-fast);
    flex-shrink: 0;
  }

  .run-button:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  .run-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .connection-badge {
    display: inline-flex;
    align-items: center;
    padding: 0 var(--spacing-2);
    height: calc(var(--toolbar-height) - var(--spacing-3));
    color: var(--color-text-secondary);
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: default;
  }

  .toolbar-spacer {
    flex: 1;
  }

  .toolbar-btn {
    padding: 0 var(--spacing-2);
    height: calc(var(--toolbar-height) - var(--spacing-3));
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .toolbar-btn:hover:not(:disabled) {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .toolbar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .toolbar-btn--icon {
    width: calc(var(--toolbar-height) - var(--spacing-3));
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .toolbar-btn--save {
    color: var(--color-text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: calc(var(--toolbar-height) - var(--spacing-3));
    padding: 0;
  }

  /* Save name dialog */
  .save-dialog {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    padding: var(--spacing-2);
    animation: dropdown-in var(--transition-md) both;
    transform-origin: top right;
  }

  .save-dialog form {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .save-name-input {
    width: 200px;
    height: 26px;
    padding: 0 var(--spacing-2);
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
    outline: none;
  }

  .save-name-input:focus {
    border-color: var(--color-accent);
  }

  .save-confirm-btn {
    padding: 0 var(--spacing-3);
    height: 26px;
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: none;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    cursor: pointer;
    white-space: nowrap;
    transition: background var(--transition-fast);
  }

  .save-confirm-btn:hover:not(:disabled) {
    background: var(--color-accent-hover);
  }

  .save-confirm-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Actions overflow menu */
  .actions-menu {
    position: fixed;
    z-index: 9999;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    padding: 3px;
    min-width: 160px;
    animation: dropdown-in var(--transition-md) both;
    transform-origin: top right;
  }

  @keyframes dropdown-in {
    from {
      opacity: 0;
      transform: scaleY(0.92) translateY(-4px);
    }
    to {
      opacity: 1;
      transform: scaleY(1) translateY(0);
    }
  }

  .actions-menu-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-4);
    width: 100%;
    padding: 0 var(--spacing-2);
    min-height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-xs);
    text-align: left;
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .actions-menu-item:hover:not(:disabled) {
    background: var(--color-accent-subtle);
  }

  .actions-menu-item:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .actions-menu-item kbd {
    font-size: 10px;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    font-family: var(--font-family-ui);
  }

  .actions-menu-sep {
    height: 1px;
    background: var(--color-border);
    margin: 3px 0;
  }

  .tx-db-warning {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    height: 28px;
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 10%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 30%, transparent);
    color: var(--color-warning, #f59e0b);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-ui);
  }

  .tx-db-warning code {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
  }

  .toolbar-btn--description-active {
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .description-panel {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    display: flex;
    flex-direction: column;
    max-height: 120px;
  }

  .description-textarea {
    flex: 1;
    resize: none;
    border: none;
    outline: none;
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-sm);
    line-height: 1.5;
    padding: var(--spacing-2) var(--spacing-3);
    min-height: 60px;
    scrollbar-width: thin;
  }

  .description-textarea::placeholder {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .editor-area {
    flex: 0 0 40%;
    min-height: 60px;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .editor-wrapper {
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  /* ── Variables panel ─────────────────────────────────────────────────────── */

  .vars-panel {
    flex-shrink: 0;
    display: flex;
    flex-direction: row;
    border-left: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    overflow: hidden;
    width: 28px;
  }

  .vars-panel--open {
    width: 200px;
  }

  .vars-panel-tab {
    flex-shrink: 0;
    width: 28px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-start;
    padding-top: var(--spacing-3);
    gap: var(--spacing-2);
    border: none;
    border-right: 1px solid var(--color-border);
    background: transparent;
    cursor: pointer;
    color: var(--color-text-muted);
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
  }

  .vars-panel-tab:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .vars-panel-tab-label {
    font-size: 10px;
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    writing-mode: vertical-rl;
    text-orientation: mixed;
    transform: rotate(180deg);
  }

  :global(.vars-panel-chevron) {
    transform: rotate(0deg);
    transition: transform var(--transition-fast);
  }

  :global(.vars-panel-chevron--open) {
    transform: rotate(180deg);
  }

  .vars-panel-body {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: var(--spacing-2);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
    scrollbar-width: thin;
  }

  .var-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .var-name {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-editor-keyword, var(--color-accent));
  }

  .var-eq {
    display: none;
  }

  .var-value {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-editor-string, var(--color-text-primary));
    word-break: break-all;
    padding-left: var(--spacing-1);
    border-left: 2px solid var(--color-border);
  }

  .var-value--null {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .var-value--unset {
    color: var(--color-text-muted);
  }

  .editor-resizer {
    flex-shrink: 0;
    height: 1px;
    background: var(--color-border);
    cursor: row-resize;
    position: relative;
    transition: background var(--transition-fast);
    z-index: 1;
  }

  .editor-resizer:hover,
  .editor-resizer--active {
    background: var(--color-accent);
  }

  .editor-resizer::after {
    content: '';
    position: absolute;
    top: -4px;
    bottom: -4px;
    left: 0;
    right: 0;
  }

  .editor-container {
    height: 100%;
    overflow: hidden;
  }

  .editor-container :global(.cm-editor) {
    height: 100%;
  }

  .editor-container :global(.cm-scroller) {
    overflow: auto;
  }

  .editor-container :global(.cm-table-link) {
    cursor: pointer;
    text-decoration: underline;
    text-decoration-style: dotted;
  }

  .results-wrapper {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  /* ── Notes gutter ─────────────────────────────────────────────────────────── */

  .editor-container :global(.cm-notes-gutter) {
    width: 18px;
  }

  .editor-container :global(.cm-notes-gutter .cm-gutterElement) {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .editor-container :global(.cm-note-plus-btn) {
    opacity: 0;
    background: none;
    border: none;
    color: var(--color-accent);
    font-size: 15px;
    font-weight: 400;
    line-height: 1;
    padding: 0;
    cursor: pointer;
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity var(--transition-fast);
  }

  .editor-container :global(.cm-notes-gutter .cm-gutterElement:hover .cm-note-plus-btn) {
    opacity: 1;
  }

  /* ── Inline note widgets ──────────────────────────────────────────────────── */

  .editor-container :global(.cm-inline-note) {
    position: sticky;
    /* left and width are set dynamically in toDOM to match gutter width */
    display: flex;
    flex-direction: column;
    margin-top: 4px;
    margin-bottom: 4px;
    box-sizing: border-box;
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-left: 3px solid var(--color-accent);
    border-radius: var(--radius-md);
    overflow: visible;
    user-select: text;
  }

  .editor-container :global(.cm-inline-note-delete) {
    position: absolute;
    top: 4px;
    right: 4px;
    background: none;
    border: none;
    color: var(--color-text-muted);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      color var(--transition-fast);
  }

  .editor-container :global(.cm-inline-note:hover .cm-inline-note-delete) {
    opacity: 0.5;
  }

  .editor-container :global(.cm-inline-note-delete:hover) {
    opacity: 1 !important;
    color: var(--color-danger);
  }

  .editor-container :global(.cm-inline-note-textarea) {
    width: 100%;
    min-height: 0;
    padding: 6px 28px 6px 8px;
    background: transparent;
    border: none;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-sm);
    resize: vertical;
    outline: none;
    line-height: var(--line-height-normal);
    box-sizing: border-box;
  }

  .editor-container :global(.cm-inline-note-textarea:focus) {
    background: var(--color-bg-input, var(--color-bg-primary));
  }

  /* ── Note menu popup ──────────────────────────────────────────────────────── */

  .note-menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 499;
  }

  .note-menu {
    position: fixed;
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    z-index: 500;
    padding: 4px;
    display: flex;
    flex-direction: column;
    min-width: 170px;
    backdrop-filter: blur(20px) saturate(160%);
  }

  .note-menu-item {
    padding: 6px 10px;
    text-align: left;
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    border-radius: var(--radius-sm);
    cursor: pointer;
    background: none;
    border: none;
    font-family: var(--font-family-ui);
    transition: background var(--transition-fast);
  }

  .note-menu-item:hover {
    background: var(--color-bg-hover);
  }

  .note-menu-item:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .note-menu-item--builder {
    color: var(--color-accent);
    font-weight: var(--font-weight-medium);
  }

  .note-menu-item--builder:hover {
    background: var(--color-accent-subtle);
  }

  .note-menu-item--ai {
    color: var(--color-accent);
  }

  .note-menu-item--ai:hover {
    background: var(--color-accent-subtle);
  }

  .note-menu-separator {
    height: 1px;
    background: var(--color-border);
    margin: 3px 4px;
  }
</style>
