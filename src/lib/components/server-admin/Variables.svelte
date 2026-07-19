<script lang="ts">
  import { onMount } from 'svelte';
  import type { ServerVariable, VarScope } from '$lib/types';
  import * as api from '$lib/tauri/server_admin';
  import { errorMessage } from '$lib/utils/errors';
  import { useToast } from '$lib/stores/toast.svelte';
  import Spinner from '$lib/components/ui/Spinner.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';
  import Modal from '$lib/components/Modal.svelte';

  interface Props {
    connectionId: string;
    canSet: boolean;
  }
  const { connectionId, canSet }: Props = $props();

  const toast = useToast();

  let variables = $state<ServerVariable[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let searchQuery = $state('');
  let scopeFilter = $state<'all' | 'session' | 'global'>('all');

  // Edit state
  let editVar = $state<ServerVariable | null>(null);
  let editValue = $state('');
  let editScope = $state<VarScope>('global');
  let editLoading = $state(false);
  let editError = $state('');

  async function load() {
    loading = true;
    error = null;
    try {
      variables = await api.listVariables(connectionId);
    } catch (e) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  const filtered = $derived(
    variables.filter((v) => {
      const matchesSearch =
        !searchQuery ||
        v.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        v.value.toLowerCase().includes(searchQuery.toLowerCase());
      const matchesScope = scopeFilter === 'all' || v.scope === scopeFilter || v.scope === 'both';
      return matchesSearch && matchesScope;
    }),
  );

  function openEdit(v: ServerVariable) {
    editVar = v;
    editValue = v.value;
    editScope = v.scope === 'session' ? 'session' : 'global';
    editError = '';
  }

  async function submitEdit() {
    if (!editVar) return;
    editLoading = true;
    editError = '';
    try {
      await api.setVariable(connectionId, editVar.name, editValue, editScope);
      toast.addToast(`Variable "${editVar.name}" updated`, 'success', 3000);
      editVar = null;
      await load();
    } catch (e) {
      editError = errorMessage(e);
    } finally {
      editLoading = false;
    }
  }
</script>

<div class="variables">
  <div class="toolbar">
    <input
      class="search-input"
      type="search"
      placeholder="Search variables…"
      bind:value={searchQuery}
    />
    <div class="scope-filter">
      {#each ['all', 'session', 'global'] as const as s}
        <button
          class="scope-btn"
          class:active={scopeFilter === s}
          onclick={() => (scopeFilter = s)}
        >
          {s.charAt(0).toUpperCase() + s.slice(1)}
        </button>
      {/each}
    </div>
    <span class="spacer"></span>
    <button class="icon-btn" onclick={load} title="Refresh" disabled={loading}>
      <RefreshIcon />
    </button>
    {#if loading}<Spinner size={12} />{/if}
  </div>

  {#if error}
    <div class="error-msg">{error}</div>
  {:else if !variables.length && loading}
    <div class="loading-msg"><Spinner size={20} /></div>
  {:else}
    <div class="table-wrap">
      <table class="data-table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Value</th>
            <th>Scope</th>
            <th>Flags</th>
            {#if canSet}<th class="edit-col"></th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each filtered as v (v.name)}
            <tr>
              <td class="mono name-cell">{v.name}</td>
              <td class="value-cell" title={v.value}>{v.value}</td>
              <td><span class="badge scope-{v.scope}">{v.scope}</span></td>
              <td class="flags-cell">
                {#if v.restartRequired}
                  <span class="badge restart">restart</span>
                {/if}
                {#if !v.isDynamic}
                  <span class="badge static">static</span>
                {/if}
              </td>
              {#if canSet}
                <td>
                  {#if v.isDynamic}
                    <button class="edit-btn" onclick={() => openEdit(v)}>Edit</button>
                  {/if}
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

{#if editVar}
  <Modal label="Edit Variable: {editVar.name}" onbackdropclick={() => (editVar = null)}>
    <div class="modal-card">
      <div class="modal-header">Edit Variable: {editVar.name}</div>
      <div class="edit-modal">
        {#if editVar.description}
          <p class="var-desc">{editVar.description}</p>
        {/if}
        {#if editVar.restartRequired}
          <div class="warning-notice">This change requires a server restart to take effect.</div>
        {/if}
        <label class="field-label">
          New Value
          <input
            class="field-input"
            type="text"
            bind:value={editValue}
            autocomplete="off"
            spellcheck={false}
          />
        </label>
        {#if editVar.scope === 'both'}
          <label class="field-label">
            Scope
            <select class="field-select" bind:value={editScope}>
              <option value="session">Session</option>
              <option value="global">Global</option>
            </select>
          </label>
        {/if}
        {#if editError}
          <div class="edit-error">{editError}</div>
        {/if}
        <div class="modal-actions">
          <button class="btn-secondary" onclick={() => (editVar = null)}>Cancel</button>
          <button class="btn-primary" onclick={submitEdit} disabled={editLoading}>
            {editLoading ? 'Applying…' : 'Apply'}
          </button>
        </div>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .variables {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  .search-input {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    min-width: 200px;
    outline: none;
  }
  .search-input:focus {
    border-color: var(--color-accent);
  }

  .scope-filter {
    display: flex;
    gap: 2px;
  }
  .scope-btn {
    padding: 3px 10px;
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    cursor: pointer;
    background: transparent;
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    transition: all var(--transition-fast);
  }
  .scope-btn.active {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: var(--color-accent);
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast);
  }
  .icon-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
  .icon-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .error-msg,
  .loading-msg {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
  }
  .error-msg {
    color: var(--color-danger, #e53e3e);
  }

  .table-wrap {
    flex: 1;
    overflow: auto;
  }
  .data-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--font-size-xs);
  }
  .data-table th,
  .data-table td {
    padding: 5px 10px;
    text-align: left;
    border-bottom: 1px solid var(--color-border);
  }
  .data-table th {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--color-text-muted);
    background: var(--color-bg-secondary);
    position: sticky;
    top: 0;
    z-index: 1;
  }
  .data-table tbody tr:hover {
    background: var(--color-bg-hover);
  }
  .mono {
    font-family: var(--font-family-mono, monospace);
  }
  .name-cell {
    white-space: nowrap;
    color: var(--color-text-primary);
  }
  .value-cell {
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-family-mono, monospace);
    color: var(--color-text-secondary);
  }
  .flags-cell {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .edit-col {
    width: 60px;
  }

  .badge {
    display: inline-block;
    padding: 1px 6px;
    border-radius: 99px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }
  .badge.scope-session {
    background: color-mix(in srgb, var(--color-accent) 12%, transparent);
    color: var(--color-accent);
  }
  .badge.scope-global {
    background: color-mix(in srgb, var(--color-success, #22c55e) 12%, transparent);
    color: var(--color-success, #22c55e);
  }
  .badge.scope-both {
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
  }
  .badge.restart {
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 15%, transparent);
    color: var(--color-warning, #f59e0b);
  }
  .badge.static {
    background: var(--color-bg-secondary);
    color: var(--color-text-disabled);
  }

  .edit-btn {
    padding: 2px 8px;
    border-radius: var(--radius-sm);
    font-size: 11px;
    cursor: pointer;
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
    transition: background var(--transition-fast);
  }
  .edit-btn:hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }

  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 420px;
    max-width: 92vw;
    overflow: hidden;
  }

  .modal-header {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .edit-modal {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: var(--spacing-4);
  }
  .var-desc {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
  }
  .warning-notice {
    padding: 8px 12px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    background: color-mix(in srgb, var(--color-warning, #f59e0b) 10%, transparent);
    color: var(--color-text-primary);
    border: 1px solid color-mix(in srgb, var(--color-warning, #f59e0b) 30%, transparent);
  }
  .field-label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }
  .field-input,
  .field-select {
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-sm);
    outline: none;
    font-family: var(--font-family-mono, monospace);
  }
  .field-input:focus,
  .field-select:focus {
    border-color: var(--color-accent);
  }
  .edit-error {
    font-size: var(--font-size-sm);
    color: var(--color-danger, #e53e3e);
  }
  .modal-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .btn-primary {
    padding: 6px 16px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    background: var(--color-accent);
    color: white;
    cursor: pointer;
    transition: opacity var(--transition-fast);
    border: none;
  }
  .btn-primary:hover {
    opacity: 0.88;
  }
  .btn-primary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn-secondary {
    padding: 6px 16px;
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
    cursor: pointer;
  }
  .btn-secondary:hover {
    background: var(--color-bg-hover);
  }
</style>
