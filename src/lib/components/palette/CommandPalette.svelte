<!--
  CommandPalette — opens on Cmd+K / Ctrl+K.
  Fuzzy-searches over actions, connections, tables, and saved queries.
  Keyboard-navigable: Arrow Up/Down to move, Enter to execute, Escape to close.
-->
<script lang="ts">
  import Fuse from 'fuse.js';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { SHORTCUT_DEFINITIONS, type ShortcutAction } from '$lib/stores/shortcuts.svelte';
  import { focusTrap } from '$lib/utils/focus-trap';
  import type { SavedQuery } from '$lib/types';

  interface Props {
    savedQueries?: SavedQuery[];
    onclose: () => void;
  }

  const { savedQueries = [], onclose }: Props = $props();

  const connections = useConnections();
  const panels = usePanels();

  let query = $state('');
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);

  // ── Result items ──────────────────────────────────────────────────────────────

  type ResultItem =
    | { kind: 'action'; action: ShortcutAction; label: string; shortcut: string }
    | { kind: 'connection'; id: string; name: string; dbType: string }
    | { kind: 'saved_query'; id: string; name: string; sql: string; connectionId: string | null }
    | { kind: 'new_editor'; connectionId: string; connectionName: string };

  const allItems = $derived<ResultItem[]>(() => {
    const items: ResultItem[] = [];

    // Actions
    for (const def of SHORTCUT_DEFINITIONS) {
      items.push({ kind: 'action', action: def.action, label: def.label, shortcut: def.vscode });
    }

    // Connections (open new editor for each)
    for (const profile of connections.profiles) {
      items.push({
        kind: 'new_editor',
        connectionId: profile.id,
        connectionName: profile.name,
      });
    }

    // Saved queries
    for (const sq of savedQueries) {
      items.push({
        kind: 'saved_query',
        id: sq.id,
        name: sq.name,
        sql: sq.sql,
        connectionId: sq.connectionId,
      });
    }

    return items;
  });

  // ── Fuse.js instances ─────────────────────────────────────────────────────

  type ActionItem = Extract<ResultItem, { kind: 'action' }>;
  type ConnectionItem = Extract<ResultItem, { kind: 'new_editor' }>;
  type SavedQueryItem = Extract<ResultItem, { kind: 'saved_query' }>;

  const fuseActions = $derived(() => {
    const items = allItems().filter((i): i is ActionItem => i.kind === 'action');
    return new Fuse(items, {
      keys: ['label', 'shortcut'],
      threshold: 0.4,
      includeScore: true,
    });
  });

  const fuseConnections = $derived(() => {
    const items = allItems().filter((i): i is ConnectionItem => i.kind === 'new_editor');
    return new Fuse(items, {
      keys: ['connectionName'],
      threshold: 0.4,
      includeScore: true,
    });
  });

  const fuseSavedQueries = $derived(() => {
    const items = allItems().filter((i): i is SavedQueryItem => i.kind === 'saved_query');
    // Truncate sql to 100 chars for indexing (add a derived field)
    const withSnippet = items.map((sq) => ({ ...sq, sqlSnippet: sq.sql.slice(0, 100) }));
    const fuse = new Fuse(withSnippet, {
      keys: ['name', 'sqlSnippet'],
      threshold: 0.4,
      includeScore: true,
    });
    return { fuse, items };
  });

  const filtered = $derived<ResultItem[]>(() => {
    const q = query.trim();
    if (!q) return allItems().slice(0, 20);

    const results: ResultItem[] = [];

    // Actions
    for (const r of fuseActions().search(q)) {
      results.push(r.item);
    }

    // Connections
    for (const r of fuseConnections().search(q)) {
      results.push(r.item);
    }

    // Saved queries — results include the enriched item with sqlSnippet;
    // reconstruct as SavedQueryItem by omitting the extra field.
    const { fuse: sqFuse } = fuseSavedQueries();
    for (const r of sqFuse.search(q)) {
      const { sqlSnippet: _unused, ...original } = r.item;
      results.push(original as SavedQueryItem);
    }

    return results.slice(0, 20);
  });

  function itemLabel(item: ResultItem): string {
    switch (item.kind) {
      case 'action':
        return item.label;
      case 'connection':
        return item.name;
      case 'saved_query':
        return item.name;
      case 'new_editor':
        return `Open editor — ${item.connectionName}`;
    }
  }

  function itemSubtitle(item: ResultItem): string {
    switch (item.kind) {
      case 'action':
        return item.shortcut;
      case 'connection':
        return item.dbType;
      case 'saved_query':
        return item.sql.slice(0, 60);
      case 'new_editor':
        return 'Query editor';
    }
  }

  function itemCategory(item: ResultItem): string {
    switch (item.kind) {
      case 'action':
        return 'Actions';
      case 'connection':
        return 'Connections';
      case 'saved_query':
        return 'Saved Queries';
      case 'new_editor':
        return 'Connections';
    }
  }

  function executeItem(item: ResultItem) {
    switch (item.kind) {
      case 'action':
        document.dispatchEvent(
          new CustomEvent('shortcut-action', {
            detail: { action: item.action },
            bubbles: true,
          }),
        );
        break;
      case 'new_editor':
        panels.openInFocused({ kind: 'query_editor', connectionId: item.connectionId });
        break;
      case 'saved_query': {
        const connId =
          item.connectionId ?? connections.profiles.find((p) => connections.isActive(p.id))?.id ?? '';
        panels.openInFocused({ kind: 'query_editor', connectionId: connId, initialSql: item.sql });
        break;
      }
    }
    onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    const items = filtered();
    if (e.key === 'Escape') {
      onclose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % Math.max(items.length, 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + Math.max(items.length, 1)) % Math.max(items.length, 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const item = items[selectedIndex];
      if (item) executeItem(item);
    }
  }

  // Reset selection when query changes.
  $effect(() => {
    query; // track
    selectedIndex = 0;
  });

  // Focus input on mount.
  $effect(() => {
    inputEl?.focus();
  });

  // Group consecutive items by category.
  interface GroupedItem {
    item: ResultItem;
    showCategory: boolean;
  }

  const groupedItems = $derived<GroupedItem[]>(() => {
    let lastCat = '';
    return filtered().map((item) => {
      const cat = itemCategory(item);
      const showCategory = cat !== lastCat;
      lastCat = cat;
      return { item, showCategory };
    });
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" role="dialog" aria-modal="true" aria-label="Command Palette" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="palette" use:focusTrap>
    <div class="search-row">
      <span class="search-icon" aria-hidden="true">⌘</span>
      <input
        class="search-input"
        type="text"
        placeholder="Search actions, connections, queries…"
        bind:value={query}
        bind:this={inputEl}
        autocomplete="off"
        spellcheck="false"
        aria-label="Command search"
        aria-autocomplete="list"
        aria-controls="palette-list"
      />
    </div>

    <ul id="palette-list" class="results-list" role="listbox">
      {#each groupedItems() as { item, showCategory }, i (i)}
        {#if showCategory}
          <li class="category-label" role="presentation">
            {itemCategory(item)}
          </li>
        {/if}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <li
          class="result-item"
          class:selected={i === selectedIndex}
          role="option"
          aria-selected={i === selectedIndex}
          onclick={() => executeItem(item)}
          onmouseenter={() => (selectedIndex = i)}
        >
          <span class="item-label">{itemLabel(item)}</span>
          {#if itemSubtitle(item)}
            <span class="item-subtitle">{itemSubtitle(item)}</span>
          {/if}
        </li>
      {:else}
        <li class="empty-item" role="presentation">No results</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 15vh;
    z-index: 500;
  }

  .palette {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 560px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: 480px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .search-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .search-icon {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    flex-shrink: 0;
  }

  .search-input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: var(--font-size-md);
    color: var(--color-text-primary);
    outline: none;
    font-family: var(--font-family-ui);
  }

  .search-input::placeholder {
    color: var(--color-text-muted);
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-1) 0;
  }

  .category-label {
    padding: var(--spacing-1) var(--spacing-4);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .result-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-2);
    padding: var(--spacing-2) var(--spacing-4);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .result-item.selected,
  .result-item:hover {
    background: var(--color-bg-hover);
  }

  .item-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
  }

  .item-subtitle {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  .empty-item {
    padding: var(--spacing-4);
    text-align: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
  }
</style>
