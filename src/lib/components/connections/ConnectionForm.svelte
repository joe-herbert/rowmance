<!--
  ConnectionForm — create or edit a connection profile.
  Pass `profile` for edit mode; omit for create mode.
  Calls `onclose` when the dialog should be dismissed.
  Passwords are stored in the OS keychain, not SQLite.
-->
<script lang="ts">
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as keychainApi from '$lib/tauri/keychain';
  import type { ConnectionProfile, DbType } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import { portal } from '$lib/utils/portal';

  interface Props {
    profile?: ConnectionProfile;
    groupId?: string | null;
    onclose: () => void;
  }

  const { profile, groupId, onclose }: Props = $props();

  const connectionStore = useConnections();

  const DEFAULT_PORTS: Record<DbType, number> = {
    mysql: 3306,
    mariadb: 3306,
    postgres: 5432,
  };

  type Tab = 'basic' | 'ssh' | 'ssl' | 'advanced';
  let activeTab = $state<Tab>('basic');

  // ── Basic fields ──────────────────────────────────────────────────────────────

  let name = $state(profile?.name ?? '');
  let dbType = $state<DbType>(profile?.dbType ?? 'postgres');
  let host = $state(profile?.host ?? 'localhost');
  let port = $state(profile?.port ?? DEFAULT_PORTS['postgres']);
  let database = $state(profile?.database ?? '');
  let username = $state(profile?.username ?? '');
  let password = $state('');
  let passwordDirty = $state(false);
  let showPassword = $state(false);
  let color = $state(profile?.color ?? '');
  let readOnly = $state(profile?.readOnly ?? false);

  // ── SSH fields ────────────────────────────────────────────────────────────────

  let sshEnabled = $state(profile?.sshEnabled ?? false);
  let sshHost = $state(profile?.sshHost ?? '');
  let sshPort = $state(profile?.sshPort ?? 22);
  let sshUser = $state(profile?.sshUser ?? '');
  let sshAuthType = $state<'password' | 'key'>(profile?.sshAuthType ?? 'password');
  let sshKeyPath = $state(profile?.sshKeyPath ?? '');
  let sshPassword = $state('');
  let showSshPassword = $state(false);

  // ── SSL fields ────────────────────────────────────────────────────────────────

  let sslEnabled = $state(profile?.sslEnabled ?? false);
  let sslCaPath = $state(profile?.sslCaPath ?? '');
  let sslCertPath = $state(profile?.sslCertPath ?? '');
  let sslKeyPath = $state(profile?.sslKeyPath ?? '');

  // ── Advanced fields ───────────────────────────────────────────────────────────

  let poolMin = $state(profile?.poolMin ?? 1);
  let poolMax = $state(profile?.poolMax ?? 5);

  // ── Status ────────────────────────────────────────────────────────────────────

  let saving = $state(false);
  let testing = $state(false);
  let testResult = $state<{ success: boolean; message: string; latencyMs: number | null } | null>(
    null,
  );
  let saveError = $state<string | null>(null);

  const isEditing = profile !== undefined;
  const title = isEditing ? 'Edit Connection' : 'New Connection';

  $effect(() => {
    if (!isEditing) return;
    keychainApi.keychainRetrieve(profile.id, 'db_password').then((v) => {
      if (v) password = v;
    });
    keychainApi.keychainRetrieve(profile.id, 'ssh_password').then((v) => {
      if (v) sshPassword = v;
    });
  });

  // ── Helpers ───────────────────────────────────────────────────────────────────

  function buildInput() {
    return {
      name: name.trim(),
      dbType,
      host: host.trim(),
      port,
      database: database.trim(),
      username: username.trim(),
      color: color || null,
      readOnly,
      groupId: profile?.groupId ?? groupId ?? null,
      sshEnabled,
      sshHost: sshEnabled ? sshHost.trim() || null : null,
      sshPort: sshEnabled ? sshPort : null,
      sshUser: sshEnabled ? sshUser.trim() || null : null,
      sshAuthType: sshEnabled ? sshAuthType : null,
      sshKeyPath: sshEnabled && sshAuthType === 'key' ? sshKeyPath.trim() || null : null,
      sslEnabled,
      sslCaPath: sslEnabled ? sslCaPath.trim() || null : null,
      sslCertPath: sslEnabled ? sslCertPath.trim() || null : null,
      sslKeyPath: sslEnabled ? sslKeyPath.trim() || null : null,
      poolMin,
      poolMax,
    };
  }

  async function saveSecrets(id: string) {
    if (password && (!isEditing || passwordDirty)) {
      await keychainApi.keychainStore(id, 'db_password', password);
    } else if (passwordDirty && !password) {
      // User explicitly cleared the field — remove any stored password.
      await keychainApi.keychainDelete(id, 'db_password');
    }
    if (sshEnabled && sshPassword) {
      await keychainApi.keychainStore(id, 'ssh_password', sshPassword);
    }
  }

  async function handleSave() {
    saveError = null;
    saving = true;
    try {
      const input = buildInput();
      let savedId: string;

      if (isEditing) {
        const updated = await connectionStore.update(profile.id, input);
        savedId = updated.id;
      } else {
        const created = await connectionStore.create(input);
        savedId = created.id;
      }

      await saveSecrets(savedId);
      onclose();
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      saving = false;
    }
  }

  async function handleTest() {
    testResult = null;
    saveError = null;
    testing = true;

    try {
      if (isEditing && !password) {
        // No new password entered — test with the stored keychain credentials so the
        // result accurately reflects what connecting from the sidebar will do.
        testResult = await connectionsApi.testConnection(profile.id, undefined);
      } else {
        testResult = await connectionsApi.testConnectionUnsaved(buildInput(), password || undefined);
      }
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      testing = false;
    }
  }

  function handleDbTypeChange() {
    const prevDefault = DEFAULT_PORTS[profile?.dbType ?? dbType];
    if (port === prevDefault) {
      port = DEFAULT_PORTS[dbType];
    }
  }

  async function pickFile(stateRef: { value: string }) {
    const selected = await openFileDialog({ multiple: false });
    if (typeof selected === 'string') {
      stateRef.value = selected;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }

  const isValid = $derived(
    name.trim() !== '' && host.trim() !== '' && database.trim() !== '' && username.trim() !== '',
  );

  const tabs: { id: Tab; label: string }[] = [
    { id: 'basic', label: 'Basic' },
    { id: 'ssh', label: 'SSH' },
    { id: 'ssl', label: 'SSL' },
    { id: 'advanced', label: 'Advanced' },
  ];

  // Reactive path holders for file pickers
  let sshKeyPathRef = $derived({ value: sshKeyPath });
  let sslCaPathRef = $derived({ value: sslCaPath });
  let sslCertPathRef = $derived({ value: sslCertPath });
  let sslKeyPathRef = $derived({ value: sslKeyPath });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="backdrop" use:portal role="dialog" aria-modal="true" aria-label={title} onclick={handleBackdropClick}>
  <div class="dialog">
    <header class="dialog-header">
      <h2 class="dialog-title">{title}</h2>
      <button class="close-btn" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    <!-- Tab strip -->
    <div class="tab-strip" role="tablist">
      {#each tabs as tab}
        <button
          class="tab-btn"
          class:active={activeTab === tab.id}
          role="tab"
          aria-selected={activeTab === tab.id}
          onclick={() => (activeTab = tab.id)}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    <form class="form" onsubmit={(e) => { e.preventDefault(); handleSave(); }}>
      <!-- Basic tab -->
      {#if activeTab === 'basic'}
        <div class="field">
          <label for="conn-name" class="label">Name</label>
          <input id="conn-name" class="input" type="text" bind:value={name} placeholder="My Database" required autocomplete="off" />
        </div>

        <div class="field-row">
          <div class="field field--grow">
            <label for="conn-type" class="label">Type</label>
            <select id="conn-type" class="input select" bind:value={dbType} onchange={handleDbTypeChange}>
              <option value="postgres">PostgreSQL</option>
              <option value="mysql">MySQL</option>
              <option value="mariadb">MariaDB</option>
            </select>
          </div>
          <div class="field field--color">
            <label for="conn-color" class="label">Colour</label>
            <div class="color-row">
              <input id="conn-color" class="color-input" type="color" bind:value={color} />
              {#if color}
                <button type="button" class="color-clear" onclick={() => (color = '')} aria-label="Clear colour">Clear</button>
              {/if}
            </div>
          </div>
        </div>

        <div class="field-row">
          <div class="field field--grow">
            <label for="conn-host" class="label">Host</label>
            <input id="conn-host" class="input" type="text" bind:value={host} placeholder="localhost" required autocomplete="off" />
          </div>
          <div class="field field--port">
            <label for="conn-port" class="label">Port</label>
            <input id="conn-port" class="input" type="number" bind:value={port} min="1" max="65535" required />
          </div>
        </div>

        <div class="field">
          <label for="conn-database" class="label">Database</label>
          <input id="conn-database" class="input" type="text" bind:value={database} placeholder="my_database" required autocomplete="off" />
        </div>

        <div class="field-row">
          <div class="field field--grow">
            <label for="conn-username" class="label">Username</label>
            <input id="conn-username" class="input" type="text" bind:value={username} placeholder="root" required autocomplete="username" />
          </div>
          <div class="field field--grow">
            <label for="conn-password" class="label">Password</label>
            <div class="password-row">
              <input
                id="conn-password"
                class="input"
                type={showPassword ? 'text' : 'password'}
                bind:value={password}
                oninput={() => (passwordDirty = true)}
                placeholder={isEditing ? '••••••••' : ''}
                autocomplete="current-password"
              />
              <button
                type="button"
                class="btn btn--ghost btn--sm btn--icon"
                aria-label={showPassword ? 'Hide password' : 'Show password'}
                onclick={() => (showPassword = !showPassword)}
              >{showPassword ? '🙈' : '👁'}</button>
            </div>
          </div>
        </div>

        <div class="field field--inline">
          <label for="conn-readonly" class="label">Read-only</label>
          <input id="conn-readonly" type="checkbox" bind:checked={readOnly} />
        </div>

      <!-- SSH tab -->
      {:else if activeTab === 'ssh'}
        <div class="field field--inline">
          <label for="ssh-enabled" class="label">Enable SSH Tunnel</label>
          <input id="ssh-enabled" type="checkbox" bind:checked={sshEnabled} />
        </div>

        {#if sshEnabled}
          <div class="field-row">
            <div class="field field--grow">
              <label for="ssh-host" class="label">SSH Host</label>
              <input id="ssh-host" class="input" type="text" bind:value={sshHost} placeholder="ssh.example.com" autocomplete="off" />
            </div>
            <div class="field field--port">
              <label for="ssh-port" class="label">Port</label>
              <input id="ssh-port" class="input" type="number" bind:value={sshPort} min="1" max="65535" />
            </div>
          </div>

          <div class="field">
            <label for="ssh-user" class="label">SSH User</label>
            <input id="ssh-user" class="input" type="text" bind:value={sshUser} placeholder="ubuntu" autocomplete="off" />
          </div>

          <div class="field">
            <label for="ssh-auth" class="label">Authentication</label>
            <select id="ssh-auth" class="input select" bind:value={sshAuthType}>
              <option value="password">Password</option>
              <option value="key">Private Key</option>
            </select>
          </div>

          {#if sshAuthType === 'password'}
            <div class="field">
              <label for="ssh-password" class="label">SSH Password</label>
              <div class="password-row">
                <input id="ssh-password" class="input" type={showSshPassword ? 'text' : 'password'} bind:value={sshPassword} placeholder={isEditing ? '••••••••' : ''} />
                <button
                  type="button"
                  class="btn btn--ghost btn--sm btn--icon"
                  aria-label={showSshPassword ? 'Hide password' : 'Show password'}
                  onclick={() => (showSshPassword = !showSshPassword)}
                >{showSshPassword ? '🙈' : '👁'}</button>
              </div>
            </div>
          {:else}
            <div class="field">
              <label for="ssh-key" class="label">Private Key File</label>
              <div class="file-row">
                <input id="ssh-key" class="input" type="text" bind:value={sshKeyPath} placeholder="/Users/you/.ssh/id_rsa" readonly />
                <button type="button" class="btn btn--ghost btn--sm" onclick={() => { openFileDialog({ multiple: false }).then(p => { if (typeof p === 'string') sshKeyPath = p; }); }}>Browse</button>
              </div>
            </div>
          {/if}
        {:else}
          <p class="tab-hint">Enable SSH tunnelling to connect through a bastion host.</p>
        {/if}

      <!-- SSL tab -->
      {:else if activeTab === 'ssl'}
        <div class="field field--inline">
          <label for="ssl-enabled" class="label">Enable SSL/TLS</label>
          <input id="ssl-enabled" type="checkbox" bind:checked={sslEnabled} />
        </div>

        {#if sslEnabled}
          <div class="field">
            <label for="ssl-ca" class="label">CA Certificate</label>
            <div class="file-row">
              <input id="ssl-ca" class="input" type="text" bind:value={sslCaPath} placeholder="/path/to/ca.pem" readonly />
              <button type="button" class="btn btn--ghost btn--sm" onclick={() => { openFileDialog({ multiple: false }).then(p => { if (typeof p === 'string') sslCaPath = p; }); }}>Browse</button>
            </div>
          </div>

          <div class="field">
            <label for="ssl-cert" class="label">Client Certificate</label>
            <div class="file-row">
              <input id="ssl-cert" class="input" type="text" bind:value={sslCertPath} placeholder="/path/to/client-cert.pem" readonly />
              <button type="button" class="btn btn--ghost btn--sm" onclick={() => { openFileDialog({ multiple: false }).then(p => { if (typeof p === 'string') sslCertPath = p; }); }}>Browse</button>
            </div>
          </div>

          <div class="field">
            <label for="ssl-key" class="label">Client Key</label>
            <div class="file-row">
              <input id="ssl-key" class="input" type="text" bind:value={sslKeyPath} placeholder="/path/to/client-key.pem" readonly />
              <button type="button" class="btn btn--ghost btn--sm" onclick={() => { openFileDialog({ multiple: false }).then(p => { if (typeof p === 'string') sslKeyPath = p; }); }}>Browse</button>
            </div>
          </div>
        {:else}
          <p class="tab-hint">Enable SSL/TLS to encrypt the connection to the database server.</p>
        {/if}

      <!-- Advanced tab -->
      {:else if activeTab === 'advanced'}
        <div class="field">
          <label for="pool-min" class="label">Minimum Pool Connections</label>
          <input id="pool-min" class="input" type="number" bind:value={poolMin} min="1" max="50" />
        </div>

        <div class="field">
          <label for="pool-max" class="label">Maximum Pool Connections</label>
          <input id="pool-max" class="input" type="number" bind:value={poolMax} min="1" max="100" />
        </div>

        <div class="field field--inline">
          <label for="adv-readonly" class="label">Read-only Mode</label>
          <input id="adv-readonly" type="checkbox" bind:checked={readOnly} />
        </div>
      {/if}

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

      {#if saveError}
        <div class="save-error">{saveError}</div>
      {/if}

      <div class="actions">
        <button
          type="button"
          class="btn btn--ghost"
          onclick={handleTest}
          disabled={testing || saving || !isValid}
        >
          {testing ? 'Testing…' : 'Test Connection'}
        </button>
        <div class="actions-right">
          <button type="button" class="btn btn--ghost" onclick={onclose}>Cancel</button>
          <button
            type="submit"
            class="btn btn--primary"
            disabled={saving || testing || !isValid}
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
    width: 600px;
    max-width: calc(100vw - var(--spacing-8));
    max-height: calc(100vh - 80px);
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

  .tab-strip {
    display: flex;
    border-bottom: 1px solid var(--color-border);
    padding: 0 var(--spacing-4);
    background: var(--color-bg-secondary);
    flex-shrink: 0;
  }

  .tab-btn {
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    cursor: pointer;
    font-family: var(--font-family-ui);
    transition: color var(--transition-fast), border-color var(--transition-fast);
  }

  .tab-btn:hover {
    color: var(--color-text-primary);
  }

  .tab-btn.active {
    color: var(--color-accent);
    border-bottom-color: var(--color-accent);
    font-weight: var(--font-weight-medium);
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

  .field--inline {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
  }

  .field--grow { flex: 1; }

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

  .file-row {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .file-row .input {
    flex: 1;
  }

  .password-row {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .password-row .input {
    flex: 1;
  }

  .btn--icon {
    flex-shrink: 0;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-md);
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

  .color-clear:hover { color: var(--color-text-secondary); }

  .tab-hint {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-style: italic;
    margin: 0;
    padding: var(--spacing-2) 0;
  }

  .test-result {
    padding: var(--spacing-2) var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .test-result--success { background: var(--color-success-subtle); color: var(--color-success); }
  .test-result--fail { background: var(--color-danger-subtle); color: var(--color-danger); }

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

  .btn--sm {
    height: 28px;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-xs);
    flex-shrink: 0;
  }

  .btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn--primary {
    background: var(--color-accent);
    color: var(--color-text-on-accent);
    border: 1px solid transparent;
  }

  .btn--primary:not(:disabled):hover { background: var(--color-accent-hover); }

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
