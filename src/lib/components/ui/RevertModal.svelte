<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';
  import { useRevert, type RevertEntry, type RevertRowChange } from '$lib/stores/revert.svelte';
  import { usePanels } from '$lib/stores/panels.svelte';
  import { executeMultiQuery } from '$lib/tauri/query';
  import { errorMessage } from '$lib/utils/errors';

  const revert = useRevert();
  const panels = usePanels();

  let expandedIds = $state<Set<string>>(new Set());
  let revertingId = $state<string | null>(null);
  let revertErrors = $state<Record<string, string>>({});

  function toggleExpand(id: string) {
    const next = new Set(expandedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedIds = next;
  }

  function close() {
    revert.closeModal();
  }

  function openSqlInEditor(entry: RevertEntry) {
    panels.openInFocused({ kind: 'query_editor', connectionId: entry.connectionId, initialSql: entry.sql });
    close();
  }

  function openRevertSqlInEditor(entry: RevertEntry) {
    if (!entry.revertSql) return;
    panels.openInFocused({ kind: 'query_editor', connectionId: entry.connectionId, initialSql: entry.revertSql });
    close();
  }

  async function revertEntry(entry: RevertEntry) {
    if (!entry.revertSql || entry.reverted || revertingId) return;
    revertingId = entry.id;
    const errs = { ...revertErrors };
    delete errs[entry.id];
    revertErrors = errs;
    try {
      await executeMultiQuery(entry.connectionId, entry.revertSql, entry.database || null);
      revert.markReverted(entry.id);
    } catch (err) {
      revertErrors = { ...revertErrors, [entry.id]: errorMessage(err) };
    } finally {
      revertingId = null;
    }
  }

  function formatValue(val: unknown, isRemoved = false, isAdded = false): string {
    if (isRemoved) return '(removed)';
    if (isAdded) return '(new)';
    if (val === null || val === undefined) return 'NULL';
    if (typeof val === 'boolean') return val ? 'true' : 'false';
    const s = String(val);
    return s.length > 80 ? s.slice(0, 80) + '…' : s;
  }

  function formatPk(pkValues: Record<string, unknown>): string {
    return Object.entries(pkValues)
      .map(([k, v]) => `${k}=${v === null ? 'NULL' : String(v)}`)
      .join(', ');
  }

  function operationLabel(op: RevertRowChange['operation']): string {
    if (op === 'update') return 'UPDATE';
    if (op === 'insert') return 'INSERT';
    return 'DELETE';
  }

  function entrySummary(entry: RevertEntry): string {
    if (entry.source === 'query') return 'Query';
    const counts = { update: 0, insert: 0, delete: 0 };
    for (const r of entry.rows) counts[r.operation]++;
    const parts: string[] = [];
    if (counts.update) parts.push(`${counts.update} update${counts.update > 1 ? 's' : ''}`);
    if (counts.insert) parts.push(`${counts.insert} insert${counts.insert > 1 ? 's' : ''}`);
    if (counts.delete) parts.push(`${counts.delete} delete${counts.delete > 1 ? 's' : ''}`);
    return parts.join(', ') || 'no changes';
  }

  function formatTime(d: Date): string {
    return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  }
</script>

<Modal label="Revert Mode" onbackdropclick={close}>
  <div class="modal-card">
    <div class="modal-header">
      <span class="modal-title">Revert Mode</span>
      <span class="entry-count">{revert.entries.length} {revert.entries.length === 1 ? 'change' : 'changes'}</span>
      <div class="header-actions">
        {#if revert.entries.length > 0}
          <button class="btn" onclick={() => revert.clear()}>Clear All</button>
        {/if}
      </div>
    </div>

    <div class="entries-list">
      {#if revert.entries.length === 0}
        <div class="empty-state">No changes tracked yet.</div>
      {:else}
        {#each revert.entries as entry (entry.id)}
          {@const isExpanded = expandedIds.has(entry.id)}
          <div class="entry" class:entry--reverted={entry.reverted}>
            <button class="entry-header" onclick={() => toggleExpand(entry.id)}>
              <svg
                class="chevron"
                class:chevron--open={isExpanded}
                width="10"
                height="10"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2.5"
                stroke-linecap="round"
                aria-hidden="true"
              >
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
              <span class="entry-table">
                {#if entry.source === 'query'}
                  <span class="source-badge source-badge--query">Query</span>
                {:else}
                  <span class="source-badge source-badge--table">Table</span>
                {/if}
                {#if entry.table}
                  <span class="entry-table-name">{entry.database ? entry.database + '.' : ''}{entry.table}</span>
                {/if}
              </span>
              <span class="entry-summary">{entrySummary(entry)}</span>
              <span class="entry-time">{formatTime(entry.executedAt)}</span>
              {#if entry.reverted}
                <span class="reverted-badge">Reverted</span>
              {/if}
            </button>

            {#if isExpanded}
              <div class="entry-body">
                {#if entry.rows.length > 0}
                  <div class="section-label">Changes</div>
                  {#each entry.rows as row, ri (ri)}
                    <div class="row-change">
                      <div class="row-change-header">
                        <span class="op-badge op-badge--{row.operation}">{operationLabel(row.operation)}</span>
                        {#if Object.keys(row.pkValues).length > 0}
                          <span class="pk-values">{formatPk(row.pkValues)}</span>
                        {:else if row.operation === 'insert'}
                          <span class="pk-values pk-values--unknown">PK not known</span>
                        {/if}
                      </div>
                      <table class="diff-table">
                        <thead>
                          <tr>
                            <th>Column</th>
                            <th>Previous</th>
                            <th>New</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each row.columnChanges as change (change.column)}
                            <tr>
                              <td class="col-name">{change.column}</td>
                              <td class="val-prev" class:val-null={change.previousValue === null || change.previousValue === undefined}>
                                {#if row.operation === 'insert'}
                                  <span class="val-badge val-badge--none">(new row)</span>
                                {:else}
                                  {formatValue(change.previousValue)}
                                {/if}
                              </td>
                              <td class="val-new" class:val-null={change.newValue === null || change.newValue === undefined}>
                                {#if row.operation === 'delete'}
                                  <span class="val-badge val-badge--removed">(deleted)</span>
                                {:else}
                                  {formatValue(change.newValue)}
                                {/if}
                              </td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>
                  {/each}
                {:else if entry.source === 'query'}
                  <div class="no-details">Row-level details not available for query editor statements.</div>
                {/if}

                <div class="section-label">Executed SQL</div>
                <pre class="sql-block">{entry.sql.trim()}</pre>

                {#if entry.revertSql}
                  <div class="section-label">Revert SQL</div>
                  <pre class="sql-block sql-block--revert">{entry.revertSql.trim()}</pre>
                {/if}

                {#if revertErrors[entry.id]}
                  <div class="revert-error">{revertErrors[entry.id]}</div>
                {/if}

                <div class="entry-actions">
                  <button class="btn" onclick={() => openSqlInEditor(entry)}>Open SQL</button>
                  {#if entry.revertSql}
                    <button class="btn" onclick={() => openRevertSqlInEditor(entry)}>Open Revert SQL</button>
                    {#if !entry.reverted}
                      <button
                        class="btn btn--danger"
                        onclick={() => revertEntry(entry)}
                        disabled={!!revertingId}
                      >
                        {revertingId === entry.id ? 'Reverting…' : 'Revert'}
                      </button>
                    {/if}
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn" onclick={close}>Close</button>
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 720px;
    max-width: 94vw;
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from { opacity: 0; transform: scale(0.96) translateY(-6px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .entry-count {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .header-actions {
    margin-left: auto;
    display: flex;
    gap: var(--spacing-2);
  }

  .entries-list {
    flex: 1;
    overflow-y: auto;
  }

  .empty-state {
    padding: var(--spacing-8) var(--spacing-4);
    text-align: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }

  .entry {
    border-bottom: 1px solid var(--color-border);
  }

  .entry:last-child {
    border-bottom: none;
  }

  .entry--reverted {
    opacity: 0.6;
  }

  .entry-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    width: 100%;
    padding: var(--spacing-2) var(--spacing-4);
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    color: var(--color-text-primary);
    font-family: var(--font-family-ui);
    font-size: var(--font-size-xs);
    transition: background var(--transition-fast);
    min-height: 36px;
  }

  .entry-header:hover {
    background: var(--color-bg-hover);
  }

  .chevron {
    flex-shrink: 0;
    color: var(--color-text-muted);
    transition: transform 150ms;
  }

  .chevron--open {
    transform: rotate(180deg);
  }

  .entry-table {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .source-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }

  .source-badge--table {
    background: color-mix(in srgb, var(--color-accent) 15%, transparent);
    color: var(--color-accent);
  }

  .source-badge--query {
    background: color-mix(in srgb, #f59e0b 15%, transparent);
    color: #f59e0b;
  }

  .entry-table-name {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
  }

  .entry-summary {
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
  }

  .entry-time {
    margin-left: auto;
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
  }

  .reverted-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, #22c55e 15%, transparent);
    color: #22c55e;
    flex-shrink: 0;
  }

  .entry-body {
    padding: var(--spacing-1) var(--spacing-4) var(--spacing-3);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .section-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin-top: var(--spacing-1);
  }

  .row-change {
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .row-change-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: var(--spacing-1) var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-tertiary, var(--color-bg-secondary));
  }

  .op-badge {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    text-transform: uppercase;
    letter-spacing: 0.02em;
    flex-shrink: 0;
  }

  .op-badge--update {
    background: color-mix(in srgb, #3b82f6 15%, transparent);
    color: #3b82f6;
  }

  .op-badge--insert {
    background: color-mix(in srgb, #22c55e 15%, transparent);
    color: #22c55e;
  }

  .op-badge--delete {
    background: color-mix(in srgb, #ef4444 15%, transparent);
    color: #ef4444;
  }

  .pk-values {
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
  }

  .pk-values--unknown {
    font-style: italic;
    color: var(--color-text-muted);
  }

  .diff-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }

  .diff-table th {
    padding: var(--spacing-1) var(--spacing-3);
    text-align: left;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
    border-bottom: 1px solid var(--color-border);
  }

  .diff-table td {
    padding: var(--spacing-1) var(--spacing-3);
    vertical-align: middle;
    border-bottom: 1px solid var(--color-border);
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .diff-table tr:last-child td {
    border-bottom: none;
  }

  .col-name {
    font-family: var(--font-family-mono);
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    width: 30%;
  }

  .val-prev {
    color: var(--color-text-secondary);
    background: color-mix(in srgb, #ef4444 5%, transparent);
    width: 35%;
  }

  .val-new {
    color: var(--color-text-secondary);
    background: color-mix(in srgb, #22c55e 5%, transparent);
    width: 35%;
  }

  .val-null {
    color: var(--color-text-muted);
    font-style: italic;
  }

  .val-badge {
    font-size: 10px;
    font-style: italic;
  }

  .val-badge--none {
    color: var(--color-text-muted);
  }

  .val-badge--removed {
    color: #ef4444;
  }

  .no-details {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
    padding: var(--spacing-1) 0;
  }

  .sql-block {
    margin: 0;
    padding: var(--spacing-2) var(--spacing-3);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text-primary);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 120px;
    overflow-y: auto;
  }

  .sql-block--revert {
    border-color: color-mix(in srgb, #f59e0b 40%, var(--color-border));
    background: color-mix(in srgb, #f59e0b 4%, var(--color-bg-secondary));
  }

  .revert-error {
    font-size: var(--font-size-xs);
    color: #ef4444;
    padding: var(--spacing-1) 0;
  }

  .entry-actions {
    display: flex;
    gap: var(--spacing-2);
    flex-wrap: wrap;
    margin-top: var(--spacing-1);
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-shrink: 0;
  }

  .btn {
    height: 28px;
    padding: 0 12px;
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
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--danger {
    background: color-mix(in srgb, #ef4444 12%, var(--color-bg-secondary));
    border-color: color-mix(in srgb, #ef4444 40%, var(--color-border));
    color: #ef4444;
  }

  .btn--danger:hover:not(:disabled) {
    background: color-mix(in srgb, #ef4444 20%, var(--color-bg-secondary));
    border-color: #ef4444;
    color: #ef4444;
  }
</style>
