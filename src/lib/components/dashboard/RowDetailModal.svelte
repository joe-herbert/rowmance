<!--
  RowDetailModal — shows a dashboard table-widget row's values, with an
  optional button to open the underlying table filtered to that row.
-->
<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';
  import ExternalLinkIcon from '$lib/components/icons/ExternalLinkIcon.svelte';
  import CopyIcon from '$lib/components/icons/CopyIcon.svelte';
  import SmallChevronIcon from '$lib/components/icons/SmallChevronIcon.svelte';
  import { portal } from '$lib/actions/portal';

  type CellValue = string | number | boolean | null;

  interface Props {
    title: string;
    columns: string[];
    row: CellValue[];
    onOpenInTable?: () => void;
    onClose: () => void;
  }

  const { title, columns, row, onOpenInTable, onClose }: Props = $props();

  function cellText(value: CellValue): string {
    return value === null ? 'NULL' : String(value);
  }

  function csvEscape(value: string): string {
    return /[",\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
  }

  type RowFormat = 'tsv' | 'csv' | 'json' | 'markdown';

  const FORMATS: { id: RowFormat; label: string }[] = [
    { id: 'tsv', label: 'Tab-separated' },
    { id: 'csv', label: 'CSV' },
    { id: 'json', label: 'JSON' },
    { id: 'markdown', label: 'Markdown table' },
  ];

  function formatRow(format: RowFormat): string {
    if (format === 'csv') {
      return columns.map((col, i) => `${csvEscape(col)},${csvEscape(cellText(row[i]))}`).join('\n');
    }
    if (format === 'json') {
      const obj: Record<string, CellValue> = {};
      columns.forEach((col, i) => (obj[col] = row[i] ?? null));
      return JSON.stringify(obj, null, 2);
    }
    if (format === 'markdown') {
      const lines = ['| Column | Value |', '| --- | --- |'];
      columns.forEach((col, i) => lines.push(`| ${col} | ${cellText(row[i])} |`));
      return lines.join('\n');
    }
    return columns.map((col, i) => `${col}\t${cellText(row[i])}`).join('\n');
  }

  let copiedField = $state<string | null>(null);
  let copiedRow = $state(false);
  let rowMenuOpen = $state(false);
  let rowMenuTriggerEl = $state<HTMLButtonElement | undefined>(undefined);
  let rowMenuEl = $state<HTMLDivElement | undefined>(undefined);
  let rowMenuTop = $state(0);
  let rowMenuLeft = $state(0);

  async function copyValue(col: string, value: CellValue): Promise<void> {
    try {
      await navigator.clipboard.writeText(cellText(value));
      copiedField = col;
      setTimeout(() => {
        if (copiedField === col) copiedField = null;
      }, 1500);
    } catch {
      // clipboard unavailable
    }
  }

  async function copyRow(format: RowFormat): Promise<void> {
    rowMenuOpen = false;
    try {
      await navigator.clipboard.writeText(formatRow(format));
      copiedRow = true;
      setTimeout(() => {
        copiedRow = false;
      }, 1500);
    } catch {
      // clipboard unavailable
    }
  }

  $effect(() => {
    if (!rowMenuOpen) return;

    function position() {
      if (!rowMenuTriggerEl || !rowMenuEl) return;
      const rect = rowMenuTriggerEl.getBoundingClientRect();
      rowMenuTop = rect.top - rowMenuEl.offsetHeight - 4;
      rowMenuLeft = rect.right - rowMenuEl.offsetWidth;
    }

    requestAnimationFrame(position);

    function onMousedown(e: MouseEvent) {
      const t = e.target as Node;
      if (!rowMenuTriggerEl?.contains(t) && !rowMenuEl?.contains(t)) {
        rowMenuOpen = false;
      }
    }

    document.addEventListener('mousedown', onMousedown, true);
    return () => document.removeEventListener('mousedown', onMousedown, true);
  });

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      if (rowMenuOpen) {
        rowMenuOpen = false;
      } else {
        onClose();
      }
    }
  }
</script>

<Modal zindex={500} label="Row details" onbackdropclick={onClose}>
  <div class="modal-dialog" role="presentation" onkeydown={handleKeydown}>
    <header class="modal-header">
      <span class="modal-title">{title}</span>
    </header>

    <div class="modal-body">
      <dl class="field-list">
        {#each columns as col, i}
          <dt>{col}</dt>
          <dd class:is-null={row[i] === null}>
            <span class="field-value">{cellText(row[i])}</span>
            <button
              class="copy-field-btn"
              onclick={() => copyValue(col, row[i])}
              title="Copy value"
              type="button"
            >
              {#if copiedField === col}
                Copied!
              {:else}
                <CopyIcon width={11} height={11} />
              {/if}
            </button>
          </dd>
        {/each}
      </dl>
    </div>

    <footer class="modal-footer">
      <span class="modal-hint">Escape to close</span>
      <div class="modal-actions">
        <div class="copy-row-split">
          <button class="modal-btn btn-copy-row" onclick={() => copyRow('tsv')}>
            <CopyIcon width={11} height={11} />
            {copiedRow ? 'Copied!' : 'Copy row'}
          </button>
          <button
            bind:this={rowMenuTriggerEl}
            class="modal-btn btn-copy-row-caret"
            onclick={() => (rowMenuOpen = !rowMenuOpen)}
            title="Copy row as…"
            aria-label="Copy row as…"
            aria-haspopup="menu"
            aria-expanded={rowMenuOpen}
            type="button"
          >
            <SmallChevronIcon direction={rowMenuOpen ? 'up' : 'down'} />
          </button>
        </div>

        {#if rowMenuOpen}
          <div
            bind:this={rowMenuEl}
            class="row-format-menu"
            role="menu"
            style="top:{rowMenuTop}px;left:{rowMenuLeft}px"
            use:portal
          >
            {#each FORMATS as fmt}
              <button
                class="row-format-item"
                role="menuitem"
                onclick={() => copyRow(fmt.id)}
                type="button"
              >
                {fmt.label}
              </button>
            {/each}
          </div>
        {/if}

        {#if onOpenInTable}
          <button class="modal-btn btn-open-table" onclick={onOpenInTable}>
            <ExternalLinkIcon width={12} height={12} strokeWidth={2} />
            Open in table view
          </button>
        {/if}
        <button class="modal-btn btn-close" onclick={onClose}>Close</button>
      </div>
    </footer>
  </div>
</Modal>

<style>
  .modal-dialog {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: min(560px, calc(100vw - 48px));
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .modal-header {
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .modal-body {
    padding: var(--spacing-3) var(--spacing-4);
    overflow-y: auto;
    min-height: 0;
  }

  .field-list {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
    gap: var(--spacing-2) var(--spacing-3);
    margin: 0;
  }

  .field-list dt {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-secondary);
    padding-top: 2px;
    word-break: break-word;
  }

  .field-list dd {
    margin: 0;
    display: flex;
    align-items: flex-start;
    gap: var(--spacing-2);
  }

  .field-value {
    flex: 1;
    min-width: 0;
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: break-word;
    -webkit-user-select: text;
    user-select: text;
  }

  .field-list dd.is-null .field-value {
    color: var(--color-null);
    font-style: italic;
  }

  .copy-field-btn {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 5px;
    border: none;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    cursor: pointer;
    opacity: 0;
    transition:
      opacity var(--transition-fast),
      background var(--transition-fast),
      color var(--transition-fast);
  }

  .field-list dd:hover .copy-field-btn,
  .copy-field-btn:focus-visible {
    opacity: 1;
  }

  .copy-field-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--spacing-3);
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-hint {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .modal-actions {
    display: flex;
    gap: var(--spacing-2);
  }

  .modal-btn {
    display: flex;
    align-items: center;
    gap: var(--spacing-1);
    padding: var(--spacing-1) var(--spacing-3);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-family: var(--font-family-ui);
    cursor: pointer;
    border: 1px solid var(--color-border);
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
    background: transparent;
    color: var(--color-text-primary);
  }

  .modal-btn:hover {
    background: var(--color-bg-hover);
  }

  .copy-row-split {
    display: flex;
  }

  .btn-copy-row {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    border-radius: var(--radius-sm) 0 0 var(--radius-sm);
    border-right: none;
  }

  .btn-copy-row-caret {
    padding: var(--spacing-1) var(--spacing-2);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    color: var(--color-text-muted);
  }

  .row-format-menu {
    position: fixed;
    z-index: 600;
    background: var(--color-bg-overlay);
    -webkit-backdrop-filter: blur(20px) saturate(160%);
    backdrop-filter: blur(20px) saturate(160%);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-overlay);
    padding: 3px;
    min-width: 160px;
  }

  .row-format-item {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 0 var(--spacing-2);
    min-height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-sm);
    cursor: pointer;
    text-align: left;
  }

  .row-format-item:hover {
    background: var(--color-accent-subtle);
  }

  .btn-open-table {
    color: var(--color-accent);
  }

  .btn-open-table:hover {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent);
  }

  .btn-close:hover {
    background: var(--color-bg-hover);
  }
</style>
