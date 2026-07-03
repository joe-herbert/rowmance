<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import {
    EditorView,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    drawSelection,
    dropCursor,
    rectangularSelection,
    crosshairCursor,
  } from '@codemirror/view';
  import { EditorState, type Extension } from '@codemirror/state';
  import { highlightSelectionMatches } from '@codemirror/search';
  import { defaultKeymap, indentWithTab } from '@codemirror/commands';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import {
    HighlightStyle,
    syntaxHighlighting,
    bracketMatching,
    foldGutter,
  } from '@codemirror/language';
  import { tags } from '@lezer/highlight';
  import {
    closeBrackets,
    closeBracketsKeymap,
    autocompletion,
    completionKeymap,
    type CompletionContext,
    type CompletionResult,
    type CompletionSource,
  } from '@codemirror/autocomplete';
  import { format as sqlFormat } from 'sql-formatter';
  import type { QueryResult } from '$lib/types';
  import { executeMultiQuery, explainQuery } from '$lib/tauri/query';
  import { listen } from '@tauri-apps/api/event';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { useSettings } from '$lib/stores/settings.svelte';
  import ResultsPanel from '$lib/components/editor/ResultsPanel.svelte';
  import * as schemaApi from '$lib/tauri/schema';
  import { splitStatements, statementAtCursor } from '$lib/utils/sql';
  import { errorMessage } from '$lib/utils/errors';
  import Select from '$lib/components/ui/Select.svelte';
  import { portal } from '$lib/actions/portal';
  import { queryEditorCache } from '$lib/stores/queryEditorState';
  import * as savedQueriesApi from '$lib/tauri/saved_queries';
  import { savedQueriesInvalidator } from '$lib/stores/savedQueriesInvalidator.svelte';
  import type { FileQuery } from '$lib/types';

  interface Props {
    connectionId: string;
    database?: string;
    initialSql?: string;
    editorId?: string;
    savedQueryId?: string;
    savedQueryName?: string;
    onExecute?: (_sql: string) => void;
  }

  let {
    connectionId,
    database: initialDatabase,
    initialSql = '',
    editorId,
    savedQueryId: initialSavedQueryId,
    savedQueryName: initialSavedQueryName,
    onExecute,
  }: Props = $props();

  const connections = useConnections();
  const settingsStore = useSettings();
  const panelStore = usePanels();

  const cached = untrack(() => (editorId ? queryEditorCache.get(editorId) : undefined));

  const isFocusedPanel = $derived(
    panelStore.focusedPanel?.content?.kind === 'query_editor' &&
      panelStore.focusedPanel?.content?.editorId === editorId,
  );

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let resultsWrapperEl = $state<HTMLDivElement | undefined>(undefined);
  let sqlText = $state(untrack(() => cached?.sql ?? initialSql));
  let results = $state<QueryResult[]>(untrack(() => cached?.results ?? []));
  let executedStatements = $state<string[]>(untrack(() => cached?.executedStatements ?? []));
  let isRunning = $state(false);
  let transactionActive = $derived(connections.isTransactionActive(connectionId));

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

  $effect(() => {
    if (!editorId) return;
    queryEditorCache.save(editorId, {
      sql: sqlText,
      results,
      executedStatements,
      selectedDatabase,
    });
  });

  $effect(() => {
    if (!editorId) return;
    const dirtyKey = `query:${editorId}`;
    if (savedSql !== null) {
      panelStore.setItemDirty(dirtyKey, sqlText !== savedSql);
    } else {
      panelStore.setItemDirty(dirtyKey, sqlText.trim() !== '');
    }
  });

  let currentSavedQueryId = $state<string | undefined>(untrack(() => initialSavedQueryId));
  let currentSavedQueryName = $state<string | undefined>(untrack(() => initialSavedQueryName));
  let savedSql = $state<string | null>(
    untrack(() => (initialSavedQueryId ? (cached?.sql ?? initialSql) : null)),
  );

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
          connectionId,
          database: selectedDatabase || null,
        });
        // ID may change if the file was renamed/moved.
        if (updated.id !== currentSavedQueryId) {
          currentSavedQueryId = updated.id;
          panelStore.updateQueryEditorMeta(editorId, { savedQueryId: updated.id });
        }
        savedSql = sqlText;
        savedQueriesInvalidator.invalidate();
      } finally {
        isSaving = false;
      }
      return;
    }
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
        connectionId,
        database: selectedDatabase || null,
      });
      currentSavedQueryId = saved.id;
      currentSavedQueryName = saved.name;
      savedSql = sqlText;
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
  const DB_TYPE_DIALECT: Record<string, string> = {
    mysql: 'mysql',
    mariadb: 'mysql',
    postgres: 'postgresql',
  };

  let sqlDialect = $derived(() => {
    const profile = connections.getById(connectionId);
    return profile ? (DB_TYPE_DIALECT[profile.dbType] ?? 'sql') : 'sql';
  });

  // ── Schema-aware autocomplete ─────────────────────────────────────────────

  interface SchemaTable {
    database: string;
    name: string;
  }

  const schemaRef: {
    connectionId: string;
    tables: SchemaTable[];
    columns: Map<string, string[]>;
  } = { connectionId: '', tables: [], columns: new Map() };

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

  function makeSchemaCompletionSource(): CompletionSource {
    return async (context: CompletionContext): Promise<CompletionResult | null> => {
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
              cols = colInfos.map((c) => c.name);
              schemaRef.columns.set(cacheKey, cols);
            } catch {
              cols = [];
            }
          }
          return {
            from: dotMatch.from + rawTable.length + 1,
            options: cols.map((col) => ({ label: col, type: 'property' })),
            validFor: /^[\w"`]*$/,
          };
        }
      }

      const word = context.matchBefore(/\w*/);
      if (!word || (word.from === word.to && !context.explicit)) return null;
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

  // ── Theme ─────────────────────────────────────────────────────────────────

  function resolveCSSVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
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
      '.cm-tooltip': {
        backgroundColor: 'var(--color-bg-overlay)',
        border: '1px solid var(--color-border)',
        boxShadow: 'var(--shadow-md)',
        borderRadius: 'var(--radius-sm)',
      },
      '.cm-tooltip-autocomplete ul li[aria-selected]': {
        backgroundColor: 'var(--color-bg-active)',
        color: 'var(--color-text-primary)',
      },
    });
  }

  // ── Query execution ───────────────────────────────────────────────────────

  async function runQuery(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;

    isRunning = true;
    executedStatements = splitStatements(query);
    try {
      results = await executeMultiQuery(connectionId, query, selectedDatabase || null);
      onExecute?.(query);
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationMs: 0,
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
    try {
      results = await executeMultiQuery(connectionId, query, selectedDatabase || null);
      onExecute?.(query);
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationMs: 0,
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
    try {
      results = await executeMultiQuery(connectionId, stmt, selectedDatabase || null);
      onExecute?.(stmt);
    } catch (err) {
      results = [
        {
          queryId: '',
          columns: [],
          rows: [],
          totalRows: null,
          durationMs: 0,
          affectedRows: null,
          error: errorMessage(err),
        },
      ];
    } finally {
      isRunning = false;
    }
  }

  function formatQuery(): void {
    if (!editorView) return;
    const dialect = sqlDialect();
    const s = settingsStore.settings;
    try {
      let formatted = sqlFormat(sqlText, {
        language: dialect as NonNullable<Parameters<typeof sqlFormat>[1]>['language'],
        keywordCase: s.formatKeywordCase,
        indentStyle: s.formatIndentStyle,
        linesBetweenQueries: s.formatLinesBetweenQueries,
      });
      if (s.formatCompact) {
        // Collapse each statement to a single line, then re-insert the
        // configured number of blank lines between statements.
        const separator = ';\n' + '\n'.repeat(s.formatLinesBetweenQueries);
        formatted = formatted
          .split(/\n/)
          .map((line) => line.trim())
          .filter(Boolean)
          .join(' ')
          .replace(/\s*;\s*/g, separator)
          .trim();
      }
      editorView.dispatch({
        changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
      });
    } catch {
      // Formatting failed silently — leave content unchanged.
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
          durationMs: 0,
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
        keymap.of([...defaultKeymap, ...completionKeymap, ...closeBracketsKeymap, indentWithTab]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            sqlText = update.state.doc.toString();
          }
        }),
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
  });
</script>

<div class="query-editor-panel">
  <div class="toolbar" bind:this={toolbarEl}>
    <button
      class="run-button"
      onclick={runQuery}
      disabled={isRunning}
      title="Run query (Cmd+Enter)"
      aria-label="Run query"
    >
      {isRunning ? '⏳' : '▶'}
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
      />
    {:else if selectedDatabase}
      <span class="connection-badge" title="Database">{selectedDatabase}</span>
    {/if}

    <div class="toolbar-spacer"></div>

    <button
      bind:this={saveDialogTriggerEl}
      class="toolbar-btn toolbar-btn--save"
      onclick={saveQuery}
      disabled={isSaving}
      title={currentSavedQueryId ? 'Save query' : 'Save query as…'}
      aria-label="Save query"
    >
      <svg
        width="13"
        height="13"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="1.8"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"></path>
        <polyline points="17 21 17 13 7 13 7 21"></polyline>
        <polyline points="7 3 7 8 15 8"></polyline>
      </svg>
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
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
          <circle cx="2" cy="7" r="1.25" fill="currentColor" />
          <circle cx="7" cy="7" r="1.25" fill="currentColor" />
          <circle cx="12" cy="7" r="1.25" fill="currentColor" />
        </svg>
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
    {/if}

  </div>

  <div class="editor-wrapper">
    <div class="editor-container" bind:this={editorContainer}></div>
  </div>

  <div class="results-wrapper" bind:this={resultsWrapperEl} tabindex="-1">
    <ResultsPanel
      {results}
      statements={executedStatements}
      {connectionId}
      database={selectedDatabase || undefined}
    />
  </div>
</div>

<style>
  .query-editor-panel {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--color-editor-bg);
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

  /* Transaction toolbar */
  .editor-wrapper {
    flex: 0 0 40%;
    overflow: hidden;
    border-bottom: 1px solid var(--color-border);
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

  .results-wrapper {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
