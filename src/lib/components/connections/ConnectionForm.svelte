<!--
  ConnectionForm — create or edit a connection profile.
  Pass `profile` for edit mode; omit for create mode.
  Calls `onclose` when the dialog should be dismissed.
  Passwords are stored in the OS keychain, not SQLite.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
  import { useConnections } from '$lib/stores/connections.svelte';
  import * as connectionsApi from '$lib/tauri/connections';
  import * as keychainApi from '$lib/tauri/keychain';
  import { getDialect, getAllDialects, urlSchemeToDbType } from '$lib/stores/dialects.svelte';
  import type { ConnectionProfile, DbType } from '$lib/types';
  import { errorMessage } from '$lib/utils/errors';
  import Modal from '$lib/components/Modal.svelte';
  import ErrorMessage from '$lib/components/ErrorMessage.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import LinkIcon from '$lib/components/icons/LinkIcon.svelte';
  import CloseIcon from '$lib/components/icons/CloseIcon.svelte';
  import EyeIcon from '$lib/components/icons/EyeIcon.svelte';
  import EyeOffIcon from '$lib/components/icons/EyeOffIcon.svelte';
  import CheckIcon from '$lib/components/icons/CheckIcon.svelte';

  interface Props {
    profile?: ConnectionProfile;
    groupId?: string | null;
    onclose: () => void;
    ondelete?: () => void;
  }

  const { profile, groupId, onclose, ondelete }: Props = $props();

  const connectionStore = useConnections();

  type Tab = 'basic' | 'ssh' | 'ssl' | 'advanced';
  let activeTab = $state<Tab>('basic');

  // ── Basic fields ──────────────────────────────────────────────────────────────

  let name = $state(untrack(() => profile?.name ?? ''));
  let dbType = $state<DbType>(untrack(() => profile?.dbType ?? 'postgres'));
  let host = $state(untrack(() => profile?.host ?? 'localhost'));
  // Default port when a fresh form opens — 5432 for postgres (the initial default dbType).
  let port = $state(untrack(() => profile?.port ?? 5432));
  let database = $state(untrack(() => profile?.database ?? ''));
  let username = $state(untrack(() => profile?.username ?? ''));
  let password = $state('');
  let passwordDirty = $state(false);
  let showPassword = $state(false);
  // File path for file-based engines (SQLite) — stored in `host` on the profile.
  let filePath = $state(
    untrack(() => (profile?.dialectInfo?.isFileBased ? (profile?.host ?? '') : '')),
  );
  let color = $state(untrack(() => profile?.color ?? ''));
  let readOnly = $state(untrack(() => profile?.readOnly ?? false));

  // ── SSH fields ────────────────────────────────────────────────────────────────

  let sshEnabled = $state(untrack(() => profile?.sshEnabled ?? false));
  let sshHost = $state(untrack(() => profile?.sshHost ?? ''));
  let sshPort = $state(untrack(() => profile?.sshPort ?? 22));
  let sshUser = $state(untrack(() => profile?.sshUser ?? ''));
  let sshAuthType = $state<'password' | 'key'>(untrack(() => profile?.sshAuthType ?? 'password'));
  let sshKeyPath = $state(untrack(() => profile?.sshKeyPath ?? ''));
  let sshPassword = $state('');
  let showSshPassword = $state(false);

  // ── SSL fields ────────────────────────────────────────────────────────────────

  let sslEnabled = $state(untrack(() => profile?.sslEnabled ?? false));
  let sslCaPath = $state(untrack(() => profile?.sslCaPath ?? ''));
  let sslCertPath = $state(untrack(() => profile?.sslCertPath ?? ''));
  let sslKeyPath = $state(untrack(() => profile?.sslKeyPath ?? ''));

  // ── Advanced fields ───────────────────────────────────────────────────────────

  let poolMax = $state(untrack(() => profile?.poolMax ?? 5));
  let pingInterval = $state(untrack(() => profile?.pingInterval ?? null));

  // ── Dialect-driven helpers ────────────────────────────────────────────────────

  const currentDialect = $derived(getDialect(dbType));
  const isFileBased = $derived(currentDialect?.isFileBased ?? false);

  // ── URL import ────────────────────────────────────────────────────────────────

  let showUrlInput = $state(false);
  let connectionUrl = $state('');
  let urlError = $state('');

  function applyConnectionUrl() {
    urlError = '';
    const url = connectionUrl.trim();
    if (!url) return;

    try {
      const parsed = new URL(url);
      const scheme = parsed.protocol.replace(':', '').toLowerCase();
      const matchedDbType = urlSchemeToDbType(scheme);
      if (!matchedDbType) {
        const knownSchemes = getAllDialects()
          .flatMap(({ dialect }) => dialect.urlSchemes)
          .join(', ');
        urlError = `Unsupported scheme "${scheme}". Supported schemes: ${knownSchemes}.`;
        return;
      }

      dbType = matchedDbType;
      const matchedDialect = getDialect(matchedDbType);

      if (matchedDialect?.isFileBased) {
        // File-based engines: extract the path from the URL (everything after the scheme://)
        filePath = (parsed.hostname + parsed.pathname).replace(/^\/+/, '') || parsed.pathname;
        showUrlInput = false;
        connectionUrl = '';
        return;
      }

      host = parsed.hostname || 'localhost';
      port = parsed.port ? parseInt(parsed.port, 10) : (matchedDialect?.defaultPort ?? 0);
      database = parsed.pathname.replace(/^\//, '');
      username = parsed.username ? decodeURIComponent(parsed.username) : '';
      if (parsed.password) {
        password = decodeURIComponent(parsed.password);
        passwordDirty = true;
      }

      showUrlInput = false;
      connectionUrl = '';
    } catch {
      urlError = 'Invalid URL — expected format: postgres://user:pass@host:5432/dbname';
    }
  }

  // ── Status ────────────────────────────────────────────────────────────────────

  let saving = $state(false);
  let testing = $state(false);
  let testResult = $state<{ success: boolean; message: string; latencyMs: number | null } | null>(
    null,
  );
  let saveError = $state<string | null>(null);

  const isEditing = $derived(profile !== undefined);
  const title = $derived(isEditing ? 'Edit Connection' : 'New Connection');

  $effect(() => {
    if (!profile) return;
    keychainApi.keychainRetrieve(profile.id, 'db_password').then((v) => {
      if (v) password = v;
    });
    if (profile.sshEnabled) {
      keychainApi.keychainRetrieve(profile.id, 'ssh_password').then((v) => {
        if (v) sshPassword = v;
      });
    }
  });

  // ── Helpers ───────────────────────────────────────────────────────────────────

  function buildInput() {
    if (isFileBased) {
      return {
        name: name.trim(),
        dbType,
        host: filePath.trim(),
        port: 0,
        database: '',
        username: '',
        color: color || null,
        readOnly,
        groupId: profile?.groupId ?? groupId ?? null,
        sshEnabled: false,
        sshHost: null,
        sshPort: null,
        sshUser: null,
        sshAuthType: null,
        sshKeyPath: null,
        sslEnabled: false,
        sslCaPath: null,
        sslCertPath: null,
        sslKeyPath: null,
        poolMax,
        pingInterval: pingInterval || null,
      };
    }
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
      poolMax,
      pingInterval: pingInterval || null,
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
        const updated = await connectionStore.update(profile!.id, input);
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
        testResult = await connectionsApi.testConnection(profile!.id, undefined);
      } else {
        testResult = await connectionsApi.testConnectionUnsaved(
          buildInput(),
          password || undefined,
          sshPassword || undefined,
        );
      }
    } catch (err) {
      saveError = errorMessage(err);
    } finally {
      testing = false;
    }
  }

  function handleDbTypeChange() {
    const prevDialect = getDialect(profile?.dbType ?? dbType);
    const prevDefault = prevDialect?.defaultPort ?? 0;
    if (port === prevDefault) {
      port = currentDialect?.defaultPort ?? 0;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose();
  }

  const isValid = $derived(
    name.trim() !== '' &&
      (isFileBased
        ? filePath.trim() !== ''
        : host.trim() !== '' && database.trim() !== '' && username.trim() !== ''),
  );

  const allTabs: { id: Tab; label: string }[] = [
    { id: 'basic', label: 'Basic' },
    { id: 'ssh', label: 'SSH' },
    { id: 'ssl', label: 'SSL' },
    { id: 'advanced', label: 'Advanced' },
  ];

  const tabs = $derived(
    isFileBased ? allTabs.filter((t) => t.id !== 'ssh' && t.id !== 'ssl') : allTabs,
  );

  $effect(() => {
    if (isFileBased && (activeTab === 'ssh' || activeTab === 'ssl')) {
      activeTab = 'basic';
    }
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<Modal zindex={200} label={title} onbackdropclick={onclose}>
  <div class="dialog">
    <header class="dialog-header">
      <h2 class="dialog-title">{title}</h2>
      {#if !isEditing}
        <button
          type="button"
          class="url-toggle-btn"
          onclick={() => {
            showUrlInput = !showUrlInput;
            urlError = '';
            if (!showUrlInput) connectionUrl = '';
          }}
        >
          <LinkIcon width={12} height={12} />
          From URL
        </button>
      {/if}
      <button class="close-btn" aria-label="Close" onclick={onclose}
        ><CloseIcon width={12} height={12} strokeWidth={2.5} /></button
      >
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

    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        handleSave();
      }}
    >
      <!-- Basic tab -->
      {#if activeTab === 'basic'}
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
            autocorrect="off"
            autocapitalize="off"
            spellcheck="false"
          />
        </div>

        {#if showUrlInput}
          <div class="url-import-section">
            <div class="field">
              <div class="url-import-header">
                <label for="conn-url" class="label">Connection URL</label>
              </div>
              <div class="url-row">
                <input
                  id="conn-url"
                  class="input"
                  type="text"
                  bind:value={connectionUrl}
                  placeholder="postgres://user:pass@localhost:5432/mydb"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                  onkeydown={(e) => {
                    if (e.key === 'Enter') {
                      e.preventDefault();
                      applyConnectionUrl();
                    }
                  }}
                />
                <button type="button" class="btn btn--ghost btn--sm" onclick={applyConnectionUrl}
                  >Apply</button
                >
              </div>
              {#if urlError}<ErrorMessage message={urlError} />{/if}
            </div>
            <div class="url-import-divider"><span>or fill in manually</span></div>
          </div>
        {/if}

        <div class="field-row">
          <div class="field field--grow">
            <label for="conn-type" class="label">Type</label>
            <Select
              id="conn-type"
              bind:value={dbType}
              options={getAllDialects().map(({ dbType, dialect }) => ({
                value: dbType,
                label: dialect.displayName,
              }))}
              size="md"
              onchange={handleDbTypeChange}
            />
          </div>
          <div class="field field--color">
            <label for="conn-color" class="label">Colour</label>
            <div class="color-row">
              <input id="conn-color" class="color-input" type="color" bind:value={color} />
              {#if color}
                <button
                  type="button"
                  class="color-clear"
                  onclick={() => (color = '')}
                  aria-label="Clear colour">Clear</button
                >
              {/if}
            </div>
          </div>
        </div>

        {#if isFileBased}
          <div class="field">
            <label for="conn-file" class="label">File Path</label>
            <div class="file-row">
              <input
                id="conn-file"
                class="input"
                type="text"
                bind:value={filePath}
                placeholder="/path/to/database.db"
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
              <button
                type="button"
                class="btn btn--ghost btn--sm"
                onclick={() => {
                  openFileDialog({ multiple: false }).then((p) => {
                    if (typeof p === 'string') filePath = p;
                  });
                }}>Browse</button
              >
            </div>
          </div>
        {:else}
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
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
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
                autocomplete="off"
              />
            </div>
          </div>

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
              autocorrect="off"
              autocapitalize="off"
              spellcheck="false"
            />
          </div>

          <div class="field-row">
            <div class="field field--grow">
              <label for="conn-username" class="label">Username</label>
              <input
                id="conn-username"
                class="input"
                type="text"
                bind:value={username}
                placeholder="root"
                required
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
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
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                />
                <button
                  type="button"
                  class="btn btn--ghost btn--sm btn--icon"
                  aria-label={showPassword ? 'Hide password' : 'Show password'}
                  onclick={() => (showPassword = !showPassword)}
                  >{#if showPassword}<EyeOffIcon width={14} height={14} />{:else}<EyeIcon
                      width={14}
                      height={14}
                    />{/if}</button
                >
              </div>
            </div>
          </div>
        {/if}

        <div class="field field--inline">
          <label for="conn-readonly" class="label">Read-only</label>
          <Checkbox id="conn-readonly" bind:checked={readOnly} />
        </div>

        <!-- SSH tab -->
      {:else if activeTab === 'ssh'}
        {#if isFileBased}
          <p class="tab-hint">SSH tunnelling is not available for file-based connections.</p>
        {:else}
          <div class="field field--inline">
            <label for="ssh-enabled" class="label">Enable SSH Tunnel</label>
            <Checkbox id="ssh-enabled" bind:checked={sshEnabled} />
          </div>

          {#if sshEnabled}
            <div class="field-row">
              <div class="field field--grow">
                <label for="ssh-host" class="label">SSH Host</label>
                <input
                  id="ssh-host"
                  class="input"
                  type="text"
                  bind:value={sshHost}
                  placeholder="ssh.example.com"
                  autocomplete="off"
                  autocorrect="off"
                  autocapitalize="off"
                  spellcheck="false"
                />
              </div>
              <div class="field field--port">
                <label for="ssh-port" class="label">Port</label>
                <input
                  id="ssh-port"
                  class="input"
                  type="number"
                  bind:value={sshPort}
                  min="1"
                  max="65535"
                  autocomplete="off"
                />
              </div>
            </div>

            <div class="field">
              <label for="ssh-user" class="label">SSH User</label>
              <input
                id="ssh-user"
                class="input"
                type="text"
                bind:value={sshUser}
                placeholder="ubuntu"
                autocomplete="off"
                autocorrect="off"
                autocapitalize="off"
                spellcheck="false"
              />
            </div>

            <div class="field">
              <label for="ssh-auth" class="label">Authentication</label>
              <Select
                id="ssh-auth"
                bind:value={sshAuthType}
                options={[
                  { value: 'password', label: 'Password' },
                  { value: 'key', label: 'Private Key' },
                ]}
                size="md"
              />
            </div>

            {#if sshAuthType === 'password'}
              <div class="field">
                <label for="ssh-password" class="label">SSH Password</label>
                <div class="password-row">
                  <input
                    id="ssh-password"
                    class="input"
                    type={showSshPassword ? 'text' : 'password'}
                    bind:value={sshPassword}
                    placeholder={isEditing ? '••••••••' : ''}
                    autocomplete="off"
                    autocorrect="off"
                    autocapitalize="off"
                    spellcheck="false"
                  />
                  <button
                    type="button"
                    class="btn btn--ghost btn--sm btn--icon"
                    aria-label={showSshPassword ? 'Hide password' : 'Show password'}
                    onclick={() => (showSshPassword = !showSshPassword)}
                    >{#if showSshPassword}<EyeOffIcon width={14} height={14} />{:else}<EyeIcon
                        width={14}
                        height={14}
                      />{/if}</button
                  >
                </div>
              </div>
            {:else}
              <div class="field">
                <label for="ssh-key" class="label">Private Key File</label>
                <div class="file-row">
                  <input
                    id="ssh-key"
                    class="input"
                    type="text"
                    bind:value={sshKeyPath}
                    placeholder="/Users/you/.ssh/id_rsa"
                    autocomplete="off"
                    spellcheck="false"
                  />
                  <button
                    type="button"
                    class="btn btn--ghost btn--sm"
                    onclick={() => {
                      openFileDialog({ multiple: false }).then((p) => {
                        if (typeof p === 'string') sshKeyPath = p;
                      });
                    }}>Browse</button
                  >
                </div>
              </div>
            {/if}
          {:else}
            <p class="tab-hint">Enable SSH tunnelling to connect through a bastion host.</p>
          {/if}
        {/if}

        <!-- SSL tab -->
      {:else if activeTab === 'ssl'}
        {#if isFileBased}
          <p class="tab-hint">SSL/TLS is not available for file-based connections.</p>
        {:else}
          <div class="field field--inline">
            <label for="ssl-enabled" class="label">Enable SSL/TLS</label>
            <Checkbox id="ssl-enabled" bind:checked={sslEnabled} />
          </div>

          {#if sslEnabled}
            <div class="field">
              <label for="ssl-ca" class="label">CA Certificate</label>
              <div class="file-row">
                <input
                  id="ssl-ca"
                  class="input"
                  type="text"
                  bind:value={sslCaPath}
                  placeholder="/path/to/ca.pem"
                  readonly
                  autocomplete="off"
                  spellcheck="false"
                />
                <button
                  type="button"
                  class="btn btn--ghost btn--sm"
                  onclick={() => {
                    openFileDialog({ multiple: false }).then((p) => {
                      if (typeof p === 'string') sslCaPath = p;
                    });
                  }}>Browse</button
                >
              </div>
            </div>

            <div class="field">
              <label for="ssl-cert" class="label">Client Certificate</label>
              <div class="file-row">
                <input
                  id="ssl-cert"
                  class="input"
                  type="text"
                  bind:value={sslCertPath}
                  placeholder="/path/to/client-cert.pem"
                  readonly
                  autocomplete="off"
                  spellcheck="false"
                />
                <button
                  type="button"
                  class="btn btn--ghost btn--sm"
                  onclick={() => {
                    openFileDialog({ multiple: false }).then((p) => {
                      if (typeof p === 'string') sslCertPath = p;
                    });
                  }}>Browse</button
                >
              </div>
            </div>

            <div class="field">
              <label for="ssl-key" class="label">Client Key</label>
              <div class="file-row">
                <input
                  id="ssl-key"
                  class="input"
                  type="text"
                  bind:value={sslKeyPath}
                  placeholder="/path/to/client-key.pem"
                  readonly
                  autocomplete="off"
                  spellcheck="false"
                />
                <button
                  type="button"
                  class="btn btn--ghost btn--sm"
                  onclick={() => {
                    openFileDialog({ multiple: false }).then((p) => {
                      if (typeof p === 'string') sslKeyPath = p;
                    });
                  }}>Browse</button
                >
              </div>
            </div>
          {:else}
            <p class="tab-hint">Enable SSL/TLS to encrypt the connection to the database server.</p>
          {/if}
        {/if}

        <!-- Advanced tab -->
      {:else if activeTab === 'advanced'}
        <div class="field">
          <label for="pool-max" class="label">Maximum Pool Connections</label>
          <input
            id="pool-max"
            class="input"
            type="number"
            bind:value={poolMax}
            min="1"
            max="100"
            autocomplete="off"
          />
        </div>

        <div class="field">
          <label for="ping-interval" class="label">Ping Interval (seconds)</label>
          <input
            id="ping-interval"
            class="input"
            type="number"
            bind:value={pingInterval}
            min="5"
            max="3600"
            placeholder="Disabled"
            autocomplete="off"
          />
          <p class="tab-hint">
            Periodically checks the connection is still alive. Leave empty to disable.
          </p>
        </div>

        <div class="field field--inline">
          <label for="adv-readonly" class="label">Read-only Mode</label>
          <Checkbox id="adv-readonly" bind:checked={readOnly} />
        </div>
      {/if}

      <!-- Test result -->
      {#if testResult}
        <div
          class="test-result"
          class:test-result--success={testResult.success}
          class:test-result--fail={!testResult.success}
        >
          {#if testResult.success}<CheckIcon
              width={13}
              height={13}
              strokeWidth={2.5}
            />{:else}<CloseIcon width={13} height={13} strokeWidth={2.5} />{/if}
          {testResult.message}
          {#if testResult.latencyMs !== null}
            <span class="latency">{testResult.latencyMs}ms</span>
          {/if}
        </div>
      {/if}

      {#if saveError}
        <ErrorMessage message={saveError} />
      {/if}

      <div class="actions">
        <div class="actions-left">
          {#if isEditing && ondelete}
            <button
              type="button"
              class="btn btn--danger"
              onclick={ondelete}
              disabled={saving || testing}
            >
              Delete
            </button>
          {/if}
          <button
            type="button"
            class="btn btn--ghost"
            onclick={handleTest}
            disabled={testing || saving || !isValid}
          >
            {testing ? 'Testing…' : 'Test Connection'}
          </button>
        </div>
        <div class="actions-right">
          <button type="button" class="btn btn--ghost" onclick={onclose}>Cancel</button>
          <button type="submit" class="btn btn--primary" disabled={saving || testing || !isValid}>
            {saving ? 'Saving…' : isEditing ? 'Save Changes' : 'Add Connection'}
          </button>
        </div>
      </div>
    </form>
  </div>
</Modal>

<style>
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
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
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
    transition:
      color var(--transition-fast),
      border-color var(--transition-fast);
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

  .input {
    height: 32px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-primary);
    color: var(--color-text-primary);
    font-size: var(--font-size-md);
    font-family: var(--font-family-ui);
    transition:
      border-color var(--transition-fast),
      box-shadow var(--transition-fast);
    outline: none;
    width: 100%;
    box-sizing: border-box;
  }

  .input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px rgba(79, 70, 229, 0.15);
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

  .color-clear:hover {
    color: var(--color-text-secondary);
  }

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


  .actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: var(--spacing-2);
    border-top: 1px solid var(--color-border);
    margin-top: var(--spacing-1);
  }

  .actions-left {
    display: flex;
    gap: var(--spacing-2);
  }

  .actions-right {
    display: flex;
    gap: var(--spacing-2);
  }

  .btn--danger {
    background: transparent;
    color: var(--color-danger, #e53e3e);
    border: 1px solid var(--color-danger, #e53e3e);
  }

  .btn--danger:not(:disabled):hover {
    background: var(--color-danger, #e53e3e);
    color: #fff;
  }

  .btn {
    height: 32px;
    padding: 0 var(--spacing-3);
    border-radius: var(--radius-md);
    font-size: var(--font-size-sm);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-ui);
    cursor: pointer;
    transition:
      background var(--transition-fast),
      color var(--transition-fast),
      border-color var(--transition-fast);
    white-space: nowrap;
  }

  .btn--sm {
    height: 28px;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-xs);
    flex-shrink: 0;
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

  .url-toggle-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--spacing-1);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-muted);
    background: none;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    cursor: pointer;
    padding: 0 var(--spacing-2);
    height: 24px;
    font-family: var(--font-family-ui);
    transition:
      color var(--transition-fast),
      background var(--transition-fast);
    margin-right: var(--spacing-2);
    flex-shrink: 0;
  }

  .url-toggle-btn:hover {
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .url-import-section {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .url-import-header {
    display: flex;
    align-items: center;
  }

  .url-row {
    display: flex;
    gap: var(--spacing-2);
    align-items: center;
  }

  .url-row .input {
    flex: 1;
  }


  .url-import-divider {
    display: flex;
    align-items: center;
    gap: var(--spacing-3);
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }

  .url-import-divider::before,
  .url-import-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--color-border);
  }
</style>
