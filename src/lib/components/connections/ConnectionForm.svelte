<!--
  ConnectionForm — create or edit a connection profile.
  Pass `profile` for edit mode; omit for create mode.
  Calls `onclose` when the dialog should be dismissed.
-->
<script lang="ts">
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import type { ConnectionProfile, DbType } from '$lib/types';

  interface Props {
    profile?: ConnectionProfile;
    onclose: () => void;
  }

  const { profile, onclose }: Props = $props();

  const connectionStore = useConnections();

  const DEFAULT_PORTS: Record<DbType, number> = {
    mysql: 3306,
    mariadb: 3306,
    postgres: 5432,
  };

  // ── Form state ───────────────────────────────────────────────────────────────

  let name = $state(profile?.name ?? '');
  let dbType = $state<DbType>(profile?.dbType ?? 'postgres');
  let host = $state(profile?.host ?? 'localhost');
  let port = $state(profile?.port ?? DEFAULT_PORTS['postgres']);
  let database = $state(profile?.database ?? '');
  let username = $state(profile?.username ?? '');
  let password = $state('');
  let color = $state(profile?.color ?? '');

  let saving = $state(false);
  let testing = $state(false);
  let testResult = $state<{ success: boolean; message: string; latencyMs: number | null } | null>(
    null,
  );
  let saveError = $state<string | null>(null);

  const isEditing = profile !== undefined;
  const title = isEditing ? 'Edit Connection' : 'New Connection';

  // ── Handlers ─────────────────────────────────────────────────────────────────

  function handleDbTypeChange() {
    // Only reset port if it still matches the previous default, to avoid
    // overwriting a user-entered custom port.
    const prevDefault = DEFAULT_PORTS[profile?.dbType ?? dbType];
    if (port === prevDefault) {
      port = DEFAULT_PORTS[dbType];
    }
  }

  async function handleSave() {
    saveError = null;
    saving = true;
    try {
      const input = {
        name: name.trim(),
        dbType,
        host: host.trim(),
        port,
        database: database.trim(),
        username: username.trim(),
        ...(password ? { password } : {}),
        color: color || null,
      };

      if (isEditing) {
        await connectionStore.update(profile.id, input);
      } else {
        await connectionStore.create(input);
      }
      onclose();
    } catch (err) {
      saveError = err instanceof Error ? err.message : String(err);
    } finally {
      saving = false;
    }
  }

  async function handleTest() {
    testResult = null;
    saveError = null;
    testing = true;

    try {
      // Save first so the backend has the profile, then test against the saved ID.
      const input = {
        name: name.trim(),
        dbType,
        host: host.trim(),
        port,
        database: database.trim(),
        username: username.trim(),
        ...(password ? { password } : {}),
        color: color || null,
      };

      let savedId: string;
      if (isEditing) {
        const updated = await connectionStore.update(profile.id, input);
        savedId = updated.id;
      } else {
        const created = await connectionStore.create(input);
        savedId = created.id;
      }

      testResult = await connectionsApi.testConnection(savedId);
    } catch (err) {
      saveError = err instanceof Error ? err.message : String(err);
    } finally {
      testing = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="backdrop"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  onclick={handleBackdropClick}
>
  <div class="dialog">
    <header class="dialog-header">
      <h2 class="dialog-title">{title}</h2>
      <button class="close-btn" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    <form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
      <!-- Name -->
      <div class="field">
        <label for="conn-name" class="label">Name</label>
        <input
          id="conn-name"
          class="input"
          type="text"
          bind:value={name}
          placeholder="My Database"
          required
          autocomplete="off"
        />
      </div>

      <!-- Database type -->
      <div class="field">
        <label for="conn-type" class="label">Type</label>
        <select
          id="conn-type"
          class="input select"
          bind:value={dbType}
          onchange={handleDbTypeChange}
        >
          <option value="postgres">PostgreSQL</option>
          <option value="mysql">MySQL</option>
          <option value="mariadb">MariaDB</option>
        </select>
      </div>

      <!-- Host + Port -->
      <div class="field-row">
        <div class="field field--grow">
          <label for="conn-host" class="label">Host</label>
          <input
            id="conn-host"
            class="input"
            type="text"
            bind:value={host}
            placeholder="localhost"
            required
            autocomplete="off"
          />
        </div>
        <div class="field field--port">
          <label for="conn-port" class="label">Port</label>
          <input
            id="conn-port"
            class="input"
            type="number"
            bind:value={port}
            min="1"
            max="65535"
            required
          />
        </div>
      </div>

      <!-- Database -->
      <div class="field">
        <label for="conn-database" class="label">Database</label>
        <input
          id="conn-database"
          class="input"
          type="text"
          bind:value={database}
          placeholder="my_database"
          required
          autocomplete="off"
        />
      </div>

      <!-- Username -->
      <div class="field">
        <label for="conn-username" class="label">Username</label>
        <input
          id="conn-username"
          class="input"
          type="text"
          bind:value={username}
          placeholder="root"
          required
          autocomplete="username"
        />
      </div>

      <!-- Password -->
      <div class="field">
        <label for="conn-password" class="label">
          Password
          {#if isEditing}<span class="label-hint">(leave blank to keep existing)</span>{/if}
        </label>
        <input
          id="conn-password"
          class="input"
          type="password"
          bind:value={password}
          placeholder={isEditing ? '••••••••' : ''}
          autocomplete="current-password"
        />
      </div>

      <!-- Color -->
      <div class="field field--color">
        <label for="conn-color" class="label">Colour</label>
        <div class="color-row">
          <input
            id="conn-color"
            class="color-input"
            type="color"
            bind:value={color}
          />
          {#if color}
            <button
              type="button"
              class="color-clear"
              onclick={() => (color = '')}
              aria-label="Clear colour"
            >
              Clear
            </button>
          {/if}
        </div>
      </div>

      <!-- Test result -->
      {#if testResult}
        <div class="test-result" class:test-result--success={testResult.success} class:test-result--fail={!testResult.success}>
          {testResult.success ? '✓' : '✗'}
          {testResult.message}
          {#if testResult.latencyMs !== null}
            <span class="latency">{testResult.latencyMs}ms</span>
          {/if}
        </div>
      {/if}

      <!-- Save error -->
      {#if saveError}
        <div class="save-error">{saveError}</div>
      {/if}

      <!-- Actions -->
      <div class="actions">
        <button
          type="button"
          class="btn btn--ghost"
          onclick={handleTest}
          disabled={testing || saving || !name.trim() || !host.trim() || !database.trim() || !username.trim()}
        >
          {testing ? 'Testing…' : 'Test Connection'}
        </button>
        <div class="actions-right">
          <button type="button" class="btn btn--ghost" onclick={onclose}>Cancel</button>
          <button
            type="submit"
            class="btn btn--primary"
            disabled={saving || testing || !name.trim() || !host.trim() || !database.trim() || !username.trim()}
          >
            {saving ? 'Saving…' : isEditing ? 'Save Changes' : 'Add Connection'}
          </button>
        </div>
      </div>
    </form>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .dialog {
    background: var(--color-bg-overlay);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 420px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - var(--spacing-8));
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    padding: var(--spacing-4) var(--spacing-5);
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .dialog-title {
    flex: 1;
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .close-btn {
    color: var(--color-text-muted);
    font-size: var(--font-size-md);
    padding: var(--spacing-1);
    border-radius: var(--radius-sm);
    line-height: 1;
    transition: color var(--transition-fast), background var(--transition-fast);
  }

  .close-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .form {
    padding: var(--spacing-5);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-4);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-1);
  }

  .field--grow {
    flex: 1;
  }

  .field--port {
    width: 90px;
    flex-shrink: 0;
  }

  .field-row {
    display: flex;
    gap: var(--spacing-3);
  }

  .label {
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .label-hint {
    font-weight: var(--font-weight-normal);
    color: var(--color-text-muted);
    margin-left: var(--spacing-1);
  }

  .input {
    height: 32px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-md);
    font-family: var(--font-family-ui);
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px rgba(79, 70, 229, 0.15);
  }

  .select {
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8'%3E%3Cpath d='M1 1l5 5 5-5' stroke='%23999' stroke-width='1.5' fill='none' stroke-linecap='round'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right var(--spacing-2) center;
    padding-right: var(--spacing-6);
    cursor: pointer;
  }

  .color-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .color-input {
    width: 40px;
    height: 32px;
    padding: 2px;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-primary);
    cursor: pointer;
  }

  .color-clear {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    text-decoration: underline;
  }

  .color-clear:hover {
    color: var(--color-text-secondary);
  }

  .test-result {
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .test-result--success {
    background: var(--color-success-subtle);
    color: var(--color-success);
  }

  .test-result--fail {
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .latency {
    margin-left: auto;
    font-size: var(--font-size-xs);
    opacity: 0.7;
  }

  .save-error {
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    background: var(--color-danger-subtle);
    color: var(--color-danger);
  }

  .actions {
    display: flex;
    align-items: center;
    padding-top: var(--spacing-2);
    border-top: 1px solid var(--color-border);
    margin-top: var(--spacing-1);
  }

  .actions-right {
    display: flex;
    gap: var(--spacing-2);
    margin-left: auto;
  }

  .btn {
    height: 32px;
    padding: 0 var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition: background var(--transition-fast), color var(--transition-fast),
      border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover {
    background: var(--color-accent-hover);
  }

  .btn--ghost {
    background: transparent;
    color: var(--color-text-secondary);
    border: 1px solid var(--color-border);
  }

  .btn--ghost:not(:disabled):hover {
    background: var(--color-bg-hover);
    color: var(--color-text-primary);
  }
</style>
