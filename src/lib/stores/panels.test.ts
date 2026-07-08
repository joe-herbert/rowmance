import { describe, it, expect, beforeEach } from 'vitest';

// Re-import the module fresh for each test so state doesn't bleed between tests.
let usePanels: typeof import('./panels.svelte').usePanels;

describe('usePanels', () => {
  beforeEach(async () => {
    vi.resetModules();
    ({ usePanels } = await import('./panels.svelte'));
  });

  it('starts with one empty panel', () => {
    const store = usePanels();
    expect(store.panels).toHaveLength(1);
    expect(store.panels[0].content.kind).toBe('empty');
    expect(store.splitCount).toBe(1);
    expect(store.focusedIndex).toBe(0);
  });

  it('splitFocused right creates a second split', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    expect(store.splitCount).toBe(2);
    expect(store.focusedIndex).toBe(1);
  });

  it('splitFocused down creates a second split', () => {
    const store = usePanels();
    store.splitFocused('down', 4, 4);
    expect(store.splitCount).toBe(2);
  });

  it('multiple splitFocused calls stack correctly', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    store.splitFocused('down', 4, 4);
    expect(store.splitCount).toBe(3);
  });

  it('splitFocused respects maxH × maxV limit', () => {
    const store = usePanels();
    store.splitFocused('right', 2, 2); // 2 splits
    store.splitFocused('down', 2, 2); // 3 splits
    store.splitFocused('right', 2, 2); // 4 splits (2×2 = max)
    store.splitFocused('down', 2, 2); // 5th — should be blocked
    expect(store.splitCount).toBe(4);
  });

  it('closeSplit on the last split resets to empty', () => {
    const store = usePanels();
    const splitId = store.focusedSplitId;
    store.closeSplit(splitId);
    expect(store.splitCount).toBe(1);
    expect(store.panels[0].content.kind).toBe('empty');
  });

  it('closeSplit with two splits reduces to one', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    const splitId = store.focusedSplitId;
    store.closeSplit(splitId);
    expect(store.splitCount).toBe(1);
  });

  it('focusNext wraps around', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    store.focus(1);
    store.focusNext();
    expect(store.focusedIndex).toBe(0);
  });

  it('focusPrev wraps around', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    store.focus(0);
    store.focusPrev();
    expect(store.focusedIndex).toBe(1);
  });

  it('openInFocused updates the focused panel content', () => {
    const store = usePanels();
    store.openInFocused({ kind: 'query_editor', connectionId: 'conn-1' });
    const content = store.panels[0].content;
    expect(content.kind).toBe('query_editor');
    if (content.kind === 'query_editor') {
      expect(content.connectionId).toBe('conn-1');
      expect(content.editorId).toBeTruthy();
    }
  });

  it('openInFocused accepts an erd panel kind', () => {
    const store = usePanels();
    store.openInFocused({ kind: 'erd', connectionId: 'conn-1', database: 'mydb' });
    expect(store.panels[0].content).toEqual({
      kind: 'erd',
      connectionId: 'conn-1',
      database: 'mydb',
    });
  });

  it('openInFocused accepts an explain panel kind', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'explain',
      connectionId: 'conn-1',
      sql: 'SELECT 1',
      dialect: 'postgres',
    });
    const content = store.panels[0].content;
    expect(content.kind).toBe('explain');
    if (content.kind === 'explain') {
      expect(content.sql).toBe('SELECT 1');
      expect(content.dialect).toBe('postgres');
    }
  });

  it('erd panel opened in second split after splitFocused', () => {
    const store = usePanels();
    store.splitFocused('right', 4, 4);
    store.focus(1);
    store.openInFocused({ kind: 'erd', connectionId: 'conn-2', database: 'analytics' });
    expect(store.panels[1]?.content).toEqual({
      kind: 'erd',
      connectionId: 'conn-2',
      database: 'analytics',
    });
    expect(store.panels[0].content.kind).toBe('empty');
  });

  it('openInFocused adds to openItems', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    expect(store.openItems).toHaveLength(1);
    expect(store.openItems[0].content).toMatchObject({ kind: 'table_browser', table: 'users' });
  });

  it('openInFocused deduplicates table_browser items in same split', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    expect(store.openItems).toHaveLength(1);
  });

  it('opening a second table keeps first in openItems', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'orders',
    });
    expect(store.openItems).toHaveLength(2);
    expect(store.panels[0].content).toMatchObject({ table: 'orders' });
  });

  it('closeOpenItem removes item and resets displaying panel', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    const itemId = store.openItems[0].id;
    store.closeOpenItem(itemId);
    expect(store.openItems).toHaveLength(0);
    expect(store.panels[0].content.kind).toBe('empty');
  });

  it('openInFocused focuses existing split when content is already shown', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    store.splitFocused('right', 4, 4);
    store.focus(1);
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    expect(store.focusedIndex).toBe(0);
  });

  it('moveItemToSplit moves an item between splits', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    const itemId = store.openItems[0].id;
    const sourceSplitId = store.focusedSplitId;
    store.splitFocused('right', 4, 4);
    const targetSplitId = store.focusedSplitId;

    store.moveItemToSplit(itemId, targetSplitId);

    expect(store.getSplitItems(sourceSplitId)).toHaveLength(0);
    expect(store.getSplitItems(targetSplitId)).toHaveLength(1);
  });

  it('moveItemToSplit auto-closes empty source split', () => {
    const store = usePanels();
    store.openInFocused({
      kind: 'table_browser',
      connectionId: 'c',
      database: 'db',
      table: 'users',
    });
    const itemId = store.openItems[0].id;
    store.splitFocused('right', 4, 4);
    const targetSplitId = store.focusedSplitId;

    store.moveItemToSplit(itemId, targetSplitId);
    // Source split was empty and not the last — it should have been auto-closed
    expect(store.splitCount).toBe(1);
  });
});
