<!--
  SqlEditor — lightweight CodeMirror-based SQL input with syntax highlighting
  and a format action, for use outside the main query editor (e.g. widget SQL).
-->
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { EditorView, keymap, drawSelection } from '@codemirror/view';
  import { EditorState, type Extension } from '@codemirror/state';
  import { defaultKeymap, indentWithTab, history, historyKeymap } from '@codemirror/commands';
  import { sql as sqlLang } from '@codemirror/lang-sql';
  import { HighlightStyle, syntaxHighlighting, bracketMatching } from '@codemirror/language';
  import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
  import { tags } from '@lezer/highlight';
  import { useSettings } from '$lib/stores/settings.svelte';
  import { formatSqlText } from '$lib/utils/sqlFormat';

  interface Props {
    value?: string;
    dialect?: string;
    placeholder?: string;
    minHeight?: string;
    id?: string;
  }

  let {
    value = $bindable(''),
    dialect = 'sql',
    placeholder = '',
    minHeight = '110px',
    id,
  }: Props = $props();

  const settingsStore = useSettings();

  let editorContainer = $state<HTMLDivElement | undefined>(undefined);
  let editorView: EditorView | undefined;

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
        backgroundColor: 'transparent',
        color: resolveCSSVar('--color-editor-text'),
        fontFamily: 'var(--font-family-mono)',
        fontSize: '12.5px',
      },
      '&.cm-focused': { outline: 'none' },
      '.cm-content': {
        caretColor: resolveCSSVar('--color-editor-cursor'),
        padding: 'var(--spacing-2)',
        minHeight,
      },
      '.cm-cursor, .cm-dropCursor': {
        borderLeftColor: resolveCSSVar('--color-editor-cursor'),
      },
      '.cm-selectionBackground, &.cm-focused .cm-selectionBackground': {
        backgroundColor: 'var(--color-editor-selection)',
      },
      '.cm-matchingBracket': {
        backgroundColor: resolveCSSVar('--color-editor-bracket-match'),
        outline: 'none',
      },
      '.cm-scroller': {
        overflow: 'auto',
        fontFamily: 'var(--font-family-mono)',
      },
      '.cm-placeholder': {
        color: resolveCSSVar('--color-text-muted'),
      },
    });
  }

  /** Reformats the SQL using the same format settings and shortcut as the query editor. */
  export function format(): void {
    if (!editorView) return;
    const current = editorView.state.doc.toString();
    const formatted = formatSqlText(current, dialect, settingsStore.settings);
    editorView.dispatch({
      changes: { from: 0, to: editorView.state.doc.length, insert: formatted },
    });
  }

  function handleShortcutAction(e: Event) {
    const { action } = (e as CustomEvent<{ action: string }>).detail;
    if (action === 'QUERY_FORMAT' && editorView?.hasFocus) {
      format();
    }
  }

  /** Inserts text at the current caret position (or replaces the selection), keeping focus. */
  export function insertAtCursor(text: string): void {
    if (!editorView) {
      value += text;
      return;
    }
    const { from, to } = editorView.state.selection.main;
    editorView.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + text.length },
    });
    editorView.focus();
  }

  export function focus(): void {
    editorView?.focus();
  }

  onMount(() => {
    document.addEventListener('shortcut-action', handleShortcutAction);

    if (!editorContainer) return;

    const state = EditorState.create({
      doc: value,
      extensions: [
        buildTheme(),
        syntaxHighlighting(buildHighlightStyle()),
        history(),
        drawSelection(),
        bracketMatching(),
        closeBrackets(),
        sqlLang(),
        EditorView.lineWrapping,
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            value = update.state.doc.toString();
          }
        }),
      ],
    });

    editorView = new EditorView({ state, parent: editorContainer });

    return () => {
      document.removeEventListener('shortcut-action', handleShortcutAction);
      editorView?.destroy();
      editorView = undefined;
    };
  });

  onDestroy(() => {
    editorView?.destroy();
    editorView = undefined;
  });

  // Keep the editor in sync when `value` is changed from outside (e.g. Query Builder insert).
  $effect(() => {
    if (!editorView) return;
    const current = editorView.state.doc.toString();
    if (current !== value) {
      editorView.dispatch({
        changes: { from: 0, to: current.length, insert: value },
      });
    }
  });
</script>

<div
  class="sql-editor"
  {id}
  class:empty={!value}
  data-placeholder={placeholder}
  style="height: {minHeight};"
>
  <div class="editor-container" bind:this={editorContainer}></div>
</div>

<style>
  .sql-editor {
    position: relative;
    background: var(--color-bg-input, var(--color-bg-secondary));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    resize: vertical;
    overflow: auto;
    min-height: 90px;
  }

  .sql-editor:focus-within {
    border-color: var(--color-accent);
  }

  .sql-editor.empty::before {
    content: attr(data-placeholder);
    position: absolute;
    top: var(--spacing-2);
    left: var(--spacing-2);
    font-size: 12.5px;
    font-family: var(--font-family-mono);
    color: var(--color-text-muted);
    pointer-events: none;
  }

  .editor-container {
    height: 100%;
  }

  .editor-container :global(.cm-editor) {
    height: 100%;
  }
</style>
