<script lang="ts">
  import { onMount } from 'svelte';
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
    defaultHighlightStyle,
    syntaxHighlighting,
    bracketMatching,
    foldGutter,
  } from '@codemirror/language';
  import {
    closeBrackets,
    closeBracketsKeymap,
    autocompletion,
    completionKeymap,
    type CompletionContext,
    type CompletionResult,
    type CompletionSource,
  } from '@codemirror/autocomplete';
  import type { QueryResult } from '$lib/types';
  import { executeQuery } from '$lib/tauri/query';
  import { useConnections } from '$lib/stores/connections.svelte';
  import ResultsPanel from '$lib/components/editor/ResultsPanel.svelte';
  import * as schemaApi from '$lib/tauri/schema';

  interface Props {
    connectionId: string;
    onExecute?: (_sql: string) => void;
  }

  let { connectionId, onExecute }: Props = $props();

  const connections = useConnections();

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView = $state<EditorView | undefined>(undefined);
  let sqlText = $state('');
  let result = $state<QueryResult | null>(null);
  let isRunning = $state(false);

  let connectionName = $derived(connections.getById(connectionId)?.name ?? connectionId);

  // ── Schema-aware autocomplete ─────────────────────────────────────────────

  interface SchemaTable {
    database: string;
    name: string;
  }

  // Plain mutable object — the completion source reads from it without needing
  // Svelte reactivity. Updated whenever the schema loads.
  const schemaRef: {
    connectionId: string;
    tables: SchemaTable[];
    columns: Map<string, string[]>; // key: "database.tableName"
  } = { connectionId: '', tables: [], columns: new Map() };

  async function loadSchemaForCompletion(connId: string): Promise<void> {
    schemaRef.connectionId = connId;
    schemaRef.tables = [];
    schemaRef.columns = new Map();
    try {
      const databases = await schemaApi.listDatabases(connId);
      const tables: SchemaTable[] = [];
      for (const db of databases) {
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

  // Reload schema whenever the connection becomes active.
  $effect(() => {
    if (connections.isActive(connectionId)) {
      loadSchemaForCompletion(connectionId);
    }
  });

  function makeSchemaCompletionSource(): CompletionSource {
    return async (context: CompletionContext): Promise<CompletionResult | null> => {
      // "table.col" context: offer column completions for that table.
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

      // Default: offer table names from the loaded schema.
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

  // CSS variables cannot be read at theme-build time because CodeMirror's theme
  // system needs literal strings. We resolve them at mount time from the computed
  // style of the document root so the theme still reacts to the active CSS theme.
  function resolveCSSVar(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
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
      '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection': {
        backgroundColor: resolveCSSVar('--color-editor-selection'),
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

  async function runQuery(): Promise<void> {
    const query = sqlText.trim();
    if (!query || isRunning) return;

    isRunning = true;
    try {
      result = await executeQuery(connectionId, query, 1, 50);
      onExecute?.(query);
    } catch (err) {
      result = {
        queryId: '',
        columns: [],
        rows: [],
        totalRows: null,
        durationMs: 0,
        affectedRows: null,
        error: err instanceof Error ? err.message : String(err),
      };
    } finally {
      isRunning = false;
    }
  }

  // Ctrl/Cmd+Enter runs the query from within the editor without bubbling to the toolbar.
  function runQueryCommand(): boolean {
    runQuery();
    return true;
  }

  onMount(() => {
    if (!editorContainer) return;

    const runKeymap = keymap.of([{ key: 'Mod-Enter', run: runQueryCommand }]);

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
        syntaxHighlighting(defaultHighlightStyle),
        bracketMatching(),
        closeBrackets(),
        autocompletion({ override: [makeSchemaCompletionSource()] }),
        sqlLang(),
        keymap.of([...defaultKeymap, ...completionKeymap, ...closeBracketsKeymap, indentWithTab]),
        runKeymap,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            sqlText = update.state.doc.toString();
          }
        }),
      ],
    });

    editorView = new EditorView({ state, parent: editorContainer });

    return () => {
      editorView?.destroy();
      editorView = undefined;
    };
  });
</script>

<div class="query-editor-panel">
  <div class="toolbar">
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

    <div class="toolbar-spacer"></div>

    <button
      class="format-button"
      onclick={() => {
        // Format is a no-op until a SQL formatter library is wired up.
        // The button is present for layout completeness and future use.
      }}
      title="Format SQL"
    >
      Format
    </button>
  </div>

  <div class="editor-wrapper">
    <div class="editor-container" bind:this={editorContainer}></div>
  </div>

  <div class="results-wrapper">
    <ResultsPanel {result} />
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
    flex: 0 0 var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    background: var(--color-bg-secondary);
    border-bottom: 1px solid var(--color-border);
    min-height: var(--toolbar-height);
    max-height: var(--toolbar-height);
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
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .toolbar-spacer {
    flex: 1;
  }

  .format-button {
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
  }

  .format-button:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .editor-wrapper {
    flex: 0 0 40%;
    overflow: hidden;
    border-bottom: 1px solid var(--color-border);
  }

  .editor-container {
    height: 100%;
    overflow: hidden;
  }

  /* Pass height down into CodeMirror's own scroll container. */
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
