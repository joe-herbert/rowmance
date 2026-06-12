import { describe, it, expect, beforeEach } from 'vitest';

// Re-import the module fresh for each test so state doesn't bleed between tests.
// We do this by using vi.resetModules() in beforeEach.
let usePanels: typeof import('./panels.svelte').usePanels;

describe('usePanels', () => {
  beforeEach(async () => {
    vi.resetModules();
    ({ usePanels } = await import('./panels.svelte'));
  });

  it('starts with one empty panel and mode=none', () => {
    const store = usePanels();
    expect(store.panels).toHaveLength(1);
    expect(store.panels[0].content.kind).toBe('empty');
    expect(store.splitMode).toBe('none');
    expect(store.focusedIndex).toBe(0);
  });

  it('splits right into horizontal mode', () => {
    const store = usePanels();
    store.split('right');
    expect(store.panels).toHaveLength(2);
    expect(store.splitMode).toBe('horizontal');
    expect(store.focusedIndex).toBe(1);
  });

  it('splits down into vertical mode', () => {
    const store = usePanels();
    store.split('down');
    expect(store.panels).toHaveLength(2);
    expect(store.splitMode).toBe('vertical');
  });

  it('splits horizontal then down into quad mode', () => {
    const store = usePanels();
    store.split('right');
    store.split('down');
    expect(store.splitMode).toBe('quad');
    expect(store.panels).toHaveLength(3);
  });

  it('does not split beyond four panels', () => {
    const store = usePanels();
    store.split('right');
    store.split('down');
    store.split('right');
    store.split('down'); // 5th attempt — should be ignored
    expect(store.panels).toHaveLength(4);
  });

  it('closing the only panel resets to empty/none', () => {
    const store = usePanels();
    store.closePanel(0);
    expect(store.panels).toHaveLength(1);
    expect(store.panels[0].content.kind).toBe('empty');
    expect(store.splitMode).toBe('none');
  });

  it('closing a panel in horizontal mode returns to none', () => {
    const store = usePanels();
    store.split('right');
    store.closePanel(1);
    expect(store.panels).toHaveLength(1);
    expect(store.splitMode).toBe('none');
  });

  it('focusNext wraps around', () => {
    const store = usePanels();
    store.split('right');
    store.focus(1);
    store.focusNext();
    expect(store.focusedIndex).toBe(0);
  });

  it('focusPrev wraps around', () => {
    const store = usePanels();
    store.split('right');
    store.focus(0);
    store.focusPrev();
    expect(store.focusedIndex).toBe(1);
  });

  it('openInFocused updates the focused panel content', () => {
    const store = usePanels();
    store.openInFocused({ kind: 'query_editor', connectionId: 'conn-1' });
    expect(store.panels[0].content).toEqual({ kind: 'query_editor', connectionId: 'conn-1' });
  });

  it('openInFocused accepts an erd panel kind', () => {
    const store = usePanels();
    store.openInFocused({ kind: 'erd', connectionId: 'conn-1', database: 'mydb' });
    expect(store.panels[0].content).toEqual({ kind: 'erd', connectionId: 'conn-1', database: 'mydb' });
  });

  it('openInFocused accepts an explain panel kind', () => {
    const store = usePanels();
    store.openInFocused({ kind: 'explain', connectionId: 'conn-1', sql: 'SELECT 1', dialect: 'postgres' });
    const content = store.panels[0].content;
    expect(content.kind).toBe('explain');
    if (content.kind === 'explain') {
      expect(content.sql).toBe('SELECT 1');
      expect(content.dialect).toBe('postgres');
    }
  });

  it('erd panel opened in second panel after split', () => {
    const store = usePanels();
    store.split('right');
    store.focus(1);
    store.openInFocused({ kind: 'erd', connectionId: 'conn-2', database: 'analytics' });
    expect(store.panels[1]?.content).toEqual({ kind: 'erd', connectionId: 'conn-2', database: 'analytics' });
    expect(store.panels[0].content.kind).toBe('empty');
  });
});
