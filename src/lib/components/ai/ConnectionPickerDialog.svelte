<!--
  ConnectionPickerDialog — asks the user to pick a connection (and optionally a
  database) when an action needs one but none could be inferred from context.
  Used by AiChatThread's "open in query editor" action for general chat
  conversations that were never scoped to a connection.
-->
<script lang="ts">
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as schemaApi from '$lib/tauri/schema';

  interface Props {
    title?: string;
    message?: string;
    /** Shown as a checkbox, defaulting to checked; omit to hide the option entirely. */
    rememberLabel?: string;
    onconfirm: (connectionId: string, database: string | undefined, remember: boolean) => void;
    oncancel: () => void;
  }

  const {
    title = 'Choose a connection',
    message = 'Select which connection to open this query in.',
    rememberLabel,
    onconfirm,
    oncancel,
  }: Props = $props();

  const connections = useConnections();

  let connectionId = $state('');
  let database = $state('');
  let databases = $state<string[]>([]);
  let dbLoading = $state(false);
  let remember = $state(true);

  const connectionOptions = $derived([
    { value: '', label: 'Select connection…' },
    ...connections.profiles
      .filter((p) => connections.isActive(p.id))
      .map((p) => ({ value: p.id, label: p.name })),
  ]);

  const databaseOptions = $derived([
    { value: '', label: dbLoading ? 'Loading…' : 'Select database… (optional)' },
    ...databases.map((db) => ({ value: db, label: db })),
  ]);

  async function onConnectionChange(id: string) {
    connectionId = id;
    database = '';
    databases = [];
    if (!id) return;
    dbLoading = true;
    try {
      databases = await schemaApi.listDatabases(id);
      if (databases.length === 1) database = databases[0];
    } catch {
      databases = [];
    } finally {
      dbLoading = false;
    }
  }

  function confirm() {
    if (!connectionId) return;
    onconfirm(connectionId, database || undefined, remember);
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<Modal label={title} onbackdropclick={oncancel}>
  <div class="modal-card">
    <div class="modal-title">{title}</div>
    <div class="modal-body">
      <p class="modal-message">{message}</p>
      <div class="picker-row">
        <Select
          aria-label="Connection"
          bind:value={connectionId}
          options={connectionOptions}
          onchange={onConnectionChange}
          searchable
        />
        <Select
          aria-label="Database"
          bind:value={database}
          options={databaseOptions}
          disabled={!connectionId || dbLoading}
          searchable
        />
      </div>
      {#if rememberLabel}
        <div class="remember-checkbox">
          <Checkbox id="remember-connection" size="sm" bind:checked={remember} />
          <label for="remember-connection" class="remember-checkbox-label">{rememberLabel}</label>
        </div>
      {/if}
    </div>
    <div class="modal-footer">
      <button class="btn" onclick={oncancel}>Cancel</button>
      <button class="btn btn--primary" onclick={confirm} disabled={!connectionId}>Open</button>
    </div>
  </div>
</Modal>

<style>
  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 380px;
    max-width: 92vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-title {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-body {
    padding: var(--spacing-4);
  }

  .modal-message {
    margin: 0 0 var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: var(--line-height-normal);
  }

  .picker-row {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .remember-checkbox {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    margin-top: var(--spacing-3);
  }

  .remember-checkbox-label {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    cursor: pointer;
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

  .btn {
    height: 28px;
    padding: 0 14px;
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

  .btn:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }
</style>
