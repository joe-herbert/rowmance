/**
 * Detects whether the cursor in a CodeMirror editor is positioned in a value
 * slot that corresponds to a named column in an INSERT or UPDATE statement.
 * Uses the @lezer/sql AST (via @codemirror/lang-sql) for structural detection.
 */
import { syntaxTree } from '@codemirror/language';
import type { EditorState } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';

export interface FkValueContext {
  /** Unquoted table name being written to */
  tableName: string;
  /** Unquoted column name at the cursor's value position */
  columnName: string;
}

function unquote(text: string): string {
  return text.replace(/^[`"'[]|[`"'\]]$/g, '');
}

function childList(node: SyntaxNode): SyntaxNode[] {
  const out: SyntaxNode[] = [];
  let c = node.firstChild;
  while (c) {
    out.push(c);
    c = c.nextSibling;
  }
  return out;
}

/**
 * Count which comma-separated slot (0-based) `pos` falls into within a Parens
 * node. Counts only top-level commas (depth-aware via character scan).
 */
function slotAt(state: EditorState, parens: SyntaxNode, pos: number): number {
  let slot = 0;
  let depth = 0;
  const text = state.sliceDoc(parens.from + 1, Math.min(pos, parens.to));
  for (const ch of text) {
    if ('([{'.includes(ch)) depth++;
    else if (')]}' .includes(ch)) depth--;
    else if (ch === ',' && depth === 0) slot++;
  }
  return slot;
}

/** Walk up the syntax tree from `pos` to find the enclosing Statement node. */
function enclosingStatement(state: EditorState, pos: number): SyntaxNode | null {
  let node: SyntaxNode | null = syntaxTree(state).resolveInner(pos, -1);
  while (node) {
    if (node.name === 'Statement') return node;
    node = node.parent;
  }
  return null;
}

export function getFkValueContext(state: EditorState, pos: number): FkValueContext | null {
  const stmt = enclosingStatement(state, pos);
  if (!stmt) return null;

  const children = childList(stmt);
  const kws = children
    .filter(n => n.name === 'Keyword')
    .map(n => ({ n, upper: state.sliceDoc(n.from, n.to).toUpperCase() }));

  const first = kws[0]?.upper;
  if (first === 'INSERT') return detectInsert(state, children, kws, pos);
  if (first === 'UPDATE') return detectUpdate(state, children, kws, pos);
  return null;
}

function detectInsert(
  state: EditorState,
  children: SyntaxNode[],
  kws: { n: SyntaxNode; upper: string }[],
  pos: number,
): FkValueContext | null {
  const valKw = kws.find(k => k.upper === 'VALUES');
  const intoKw = kws.find(k => k.upper === 'INTO');
  if (!valKw || !intoKw) return null;

  // Table name: first Identifier/QuotedIdentifier after INTO
  const tableNode = children.find(
    n => n.from >= intoKw.n.to && (n.name === 'Identifier' || n.name === 'QuotedIdentifier' || n.name === 'CompositeIdentifier'),
  );
  if (!tableNode) return null;

  // For CompositeIdentifier (schema.table), use the last part
  let tableName: string;
  if (tableNode.name === 'CompositeIdentifier') {
    const parts = state.sliceDoc(tableNode.from, tableNode.to).split('.');
    tableName = unquote(parts[parts.length - 1]);
  } else {
    tableName = unquote(state.sliceDoc(tableNode.from, tableNode.to));
  }

  // Column list: Parens that appears before VALUES keyword
  const colListParens = children.find(
    n => n.name === 'Parens' && n.to <= valKw.n.from,
  );
  if (!colListParens) return null;

  // Values Parens: Parens after VALUES keyword that contains the cursor
  const valueParens = children.find(
    n => n.name === 'Parens' && n.from >= valKw.n.to && pos >= n.from && pos <= n.to,
  );
  if (!valueParens) return null;

  const idx = slotAt(state, valueParens, pos);

  // Collect column names from column list
  const colNames: string[] = [];
  let child = colListParens.firstChild;
  while (child) {
    if (child.name === 'Identifier' || child.name === 'QuotedIdentifier') {
      colNames.push(unquote(state.sliceDoc(child.from, child.to)));
    }
    child = child.nextSibling;
  }

  if (idx >= colNames.length) return null;
  return { tableName, columnName: colNames[idx] };
}

function detectUpdate(
  state: EditorState,
  children: SyntaxNode[],
  kws: { n: SyntaxNode; upper: string }[],
  pos: number,
): FkValueContext | null {
  const updateKw = kws[0]; // UPDATE
  const setKw = kws.find(k => k.upper === 'SET');
  if (!setKw) return null;

  // Table name: first Identifier after UPDATE
  const tableNode = children.find(
    n => n.from >= updateKw.n.to && (n.name === 'Identifier' || n.name === 'QuotedIdentifier' || n.name === 'CompositeIdentifier'),
  );
  if (!tableNode) return null;

  let tableName: string;
  if (tableNode.name === 'CompositeIdentifier') {
    const parts = state.sliceDoc(tableNode.from, tableNode.to).split('.');
    tableName = unquote(parts[parts.length - 1]);
  } else {
    tableName = unquote(state.sliceDoc(tableNode.from, tableNode.to));
  }

  // WHERE clause start (or end of statement)
  const whereKw = kws.find(k => k.upper === 'WHERE');
  const setEnd = whereKw ? whereKw.n.from : children[children.length - 1]?.to ?? pos;

  if (pos < setKw.n.to || pos > setEnd) return null;

  // Guard: cursor must not be inside a subquery/nested Parens within the SET clause
  const enclosing = syntaxTree(state).resolveInner(pos, -1);
  if (enclosing.name === 'Parens' || enclosing.name === 'Braces' || enclosing.name === 'Brackets') {
    // Could be a subquery or array value — skip
    return null;
  }

  // Collect SET-clause children and scan backwards from cursor to find `col =` pattern
  const setCh = children.filter(n => n.from >= setKw.n.to && n.to <= setEnd);

  let colNode: SyntaxNode | null = null;
  let foundEq = false;

  for (let i = setCh.length - 1; i >= 0; i--) {
    const n = setCh[i];
    if (n.to > pos) continue;

    const text = state.sliceDoc(n.from, n.to);

    if (!foundEq) {
      if (n.name === 'Operator' && text === '=') {
        foundEq = true;
      } else if (text === ',') {
        // Comma before finding = — no assignment found
        break;
      }
    } else {
      if (n.name === 'Identifier' || n.name === 'QuotedIdentifier') {
        colNode = n;
        break;
      }
      // Anything unexpected between = and identifier — bail
      if (n.name !== 'Punctuation') break;
    }
  }

  if (!colNode) return null;

  const columnName = unquote(state.sliceDoc(colNode.from, colNode.to));
  return { tableName, columnName };
}
