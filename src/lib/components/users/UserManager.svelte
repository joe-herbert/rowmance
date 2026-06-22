<!--
  UserManager — panel for viewing and managing database users and their grants.
-->
<script lang="ts">
  import * as usersApi from '$lib/tauri/users';
  import * as schemaApi from '$lib/tauri/schema';
  import type { DbUser } from '$lib/types';
  import { useConnections } from '$lib/stores/connections.svelte';
  import { errorMessage } from '$lib/utils/errors';
  import Modal from '$lib/components/Modal.svelte';
  import Select from '$lib/components/ui/Select.svelte';
  import ConfirmDialog from '$lib/components/ui/ConfirmDialog.svelte';
  import Checkbox from '$lib/components/ui/Checkbox.svelte';
  import RefreshIcon from '$lib/components/icons/RefreshIcon.svelte';
  import SegmentedControl from '$lib/components/ui/SegmentedControl.svelte';

  interface Props {
    connectionId: string;
  }

  const { connectionId }: Props = $props();

  const connections = useConnections();
  const profile = $derived(connections.getById(connectionId));
  const dbType = $derived(profile?.dbType ?? 'mysql');
  const isReadOnly = $derived(profile?.readOnly ?? false);
  const isMysql = $derived(dbType === 'mysql' || dbType === 'mariadb');
  const isPostgres = $derived(dbType === 'postgres');
  const isSqlite = $derived(dbType === 'sqlite');

  // ── Data state ──────────────────────────────────────────────────────────────

  let isLoading = $state(true);
  let loadError = $state<string | null>(null);
  let users = $state<DbUser[]>([]);
  let selectedUser = $state<DbUser | null>(null);

  let grantsLoading = $state(false);
  let grantsError = $state<string | null>(null);
  let grants = $state<string[]>([]);

  function loadUsers() {
    isLoading = true;
    loadError = null;
    usersApi.listUsers(connectionId).then((result) => {
      users = result;
      isLoading = false;
    }).catch((err) => {
      loadError = errorMessage(err);
      isLoading = false;
    });
  }

  function loadGrants(user: DbUser) {
    grantsLoading = true;
    grantsError = null;
    grants = [];
    usersApi.getGrants(connectionId, user.username, user.host).then((result) => {
      grants = result;
      grantsLoading = false;
    }).catch((err) => {
      grantsError = errorMessage(err);
      grantsLoading = false;
    });
  }

  $effect(() => {
    // Re-run when connectionId changes
    void connectionId;
    loadUsers();
  });

  function selectUser(user: DbUser) {
    selectedUser = user;
    loadGrants(user);
  }

  // ── Add User modal ──────────────────────────────────────────────────────────

  interface AddUserForm {
    username: string;
    host: string;
    password: string;
    isSuperuser: boolean;
    canCreateDb: boolean;
    canCreateRole: boolean;
  }

  let addUserForm = $state<AddUserForm | null>(null);
  let addUserError = $state<string | null>(null);
  let addUserSaving = $state(false);

  function openAddUser() {
    addUserForm = {
      username: '',
      host: '%',
      password: '',
      isSuperuser: false,
      canCreateDb: false,
      canCreateRole: false,
    };
    addUserError = null;
  }

  async function submitAddUser() {
    if (!addUserForm) return;
    if (!addUserForm.username.trim()) { addUserError = 'Username is required'; return; }
    addUserSaving = true;
    addUserError = null;
    const newUsername = addUserForm.username.trim();
    const newHost = isMysql ? addUserForm.host : null;
    try {
      await usersApi.createUser(
        connectionId,
        newUsername,
        newHost,
        addUserForm.password,
        addUserForm.isSuperuser,
        addUserForm.canCreateDb,
        addUserForm.canCreateRole,
      );
      addUserForm = null;
      isLoading = true;
      loadError = null;
      const result = await usersApi.listUsers(connectionId);
      users = result;
      isLoading = false;
      const created = result.find(u => u.username === newUsername && u.host === newHost);
      if (created) selectUser(created);
    } catch (err) {
      addUserError = errorMessage(err);
    } finally {
      addUserSaving = false;
    }
  }

  // ── Set Password modal ──────────────────────────────────────────────────────

  let setPasswordForm = $state<{ password: string } | null>(null);
  let setPasswordError = $state<string | null>(null);
  let setPasswordSaving = $state(false);

  function _openSetPassword() {
    setPasswordForm = { password: '' };
    setPasswordError = null;
  }

  async function submitSetPassword() {
    if (!setPasswordForm || !selectedUser) return;
    setPasswordSaving = true;
    setPasswordError = null;
    try {
      await usersApi.setPassword(
        connectionId,
        selectedUser.username,
        selectedUser.host,
        setPasswordForm.password,
      );
      setPasswordForm = null;
    } catch (err) {
      setPasswordError = errorMessage(err);
    } finally {
      setPasswordSaving = false;
    }
  }

  // ── Edit User modal ─────────────────────────────────────────────────────────

  interface EditUserForm {
    username: string;
    host: string;
    password: string;
  }

  let editUserForm = $state<EditUserForm | null>(null);
  let editUserError = $state<string | null>(null);
  let editUserSaving = $state(false);

  function openEditUser() {
    if (!selectedUser) return;
    editUserForm = {
      username: selectedUser.username,
      host: selectedUser.host ?? '%',
      password: '',
    };
    editUserError = null;
  }

  async function submitEditUser() {
    if (!editUserForm || !selectedUser) return;
    if (!editUserForm.username.trim()) { editUserError = 'Username is required'; return; }
    editUserSaving = true;
    editUserError = null;
    const orig = selectedUser;
    try {
      const usernameChanged = editUserForm.username.trim() !== orig.username;
      const hostChanged = isMysql && editUserForm.host !== (orig.host ?? '%');
      if (usernameChanged || hostChanged) {
        await usersApi.renameUser(
          connectionId,
          orig.username,
          orig.host,
          editUserForm.username.trim(),
          isMysql ? editUserForm.host : null,
        );
      }
      if (editUserForm.password) {
        await usersApi.setPassword(
          connectionId,
          editUserForm.username.trim(),
          isMysql ? editUserForm.host : null,
          editUserForm.password,
        );
      }
      editUserForm = null;
      selectedUser = null;
      loadUsers();
    } catch (err) {
      editUserError = errorMessage(err);
    } finally {
      editUserSaving = false;
    }
  }

  // ── Drop User ───────────────────────────────────────────────────────────────

  let dropError = $state<string | null>(null);
  let dropConfirmUser = $state<typeof selectedUser>(null);

  function handleDropUser() {
    if (!selectedUser) return;
    dropConfirmUser = selectedUser;
  }

  async function confirmDropUser() {
    const user = dropConfirmUser;
    dropConfirmUser = null;
    if (!user) return;
    dropError = null;
    try {
      await usersApi.dropUser(connectionId, user.username, user.host);
      selectedUser = null;
      grants = [];
      loadUsers();
    } catch (err) {
      dropError = errorMessage(err);
    }
  }

  // ── Execute GRANT / REVOKE ──────────────────────────────────────────────────

  let grantSql = $state('');
  let grantError = $state<string | null>(null);
  let grantRunning = $state(false);

  async function runGrant() {
    if (!grantSql.trim()) return;
    grantRunning = true;
    grantError = null;
    try {
      await usersApi.executeGrant(connectionId, grantSql.trim());
      grantSql = '';
      if (selectedUser) loadGrants(selectedUser);
    } catch (err) {
      grantError = errorMessage(err);
    } finally {
      grantRunning = false;
    }
  }

  // ── Grant builder state ─────────────────────────────────────────────────────

  let grantMode = $state<'builder' | 'sql'>('builder');
  let grantAction = $state<'grant' | 'revoke'>('grant');
  let grantScope = $state<string>('global');
  let grantDb = $state('');
  let grantTable = $state('');
  let grantPrivs = $state(new Set<string>());
  let grantWithGrantOption = $state(false);

  let builderDbList = $state<string[]>([]);
  let builderTableList = $state<string[]>([]);
  let builderDbLoading = $state(false);
  let builderTableLoading = $state(false);

  const MYSQL_GLOBAL_PRIVS = ['ALL PRIVILEGES', 'SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'DROP', 'ALTER', 'INDEX', 'REFERENCES', 'CREATE VIEW', 'SHOW VIEW', 'CREATE TEMPORARY TABLES', 'CREATE ROUTINE', 'ALTER ROUTINE', 'EXECUTE', 'TRIGGER', 'LOCK TABLES', 'EVENT', 'SUPER', 'PROCESS', 'RELOAD', 'SHOW DATABASES', 'REPLICATION CLIENT', 'REPLICATION SLAVE'];
  const MYSQL_DB_PRIVS = ['ALL PRIVILEGES', 'SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'DROP', 'ALTER', 'INDEX', 'REFERENCES', 'CREATE VIEW', 'SHOW VIEW', 'CREATE TEMPORARY TABLES', 'CREATE ROUTINE', 'ALTER ROUTINE', 'EXECUTE', 'TRIGGER', 'LOCK TABLES', 'EVENT'];
  const MYSQL_TABLE_PRIVS = ['ALL PRIVILEGES', 'SELECT', 'INSERT', 'UPDATE', 'DELETE', 'CREATE', 'DROP', 'ALTER', 'INDEX', 'REFERENCES', 'TRIGGER'];
  const PG_SCHEMA_PRIVS = ['ALL', 'USAGE', 'CREATE'];
  const PG_TABLE_PRIVS = ['ALL', 'SELECT', 'INSERT', 'UPDATE', 'DELETE', 'TRUNCATE', 'REFERENCES', 'TRIGGER'];

  const scopeOptions = $derived.by(() => {
    if (isMysql) return [
      { value: 'global', label: 'Global (*.*)' },
      { value: 'database', label: 'Database (db.*)' },
      { value: 'table', label: 'Table (db.table)' },
    ];
    return [
      { value: 'schema', label: 'Schema' },
      { value: 'table', label: 'Table' },
    ];
  });

  const availablePrivs = $derived.by(() => {
    if (isMysql) {
      if (grantScope === 'global') return MYSQL_GLOBAL_PRIVS;
      if (grantScope === 'database') return MYSQL_DB_PRIVS;
      return MYSQL_TABLE_PRIVS;
    }
    if (grantScope === 'schema') return PG_SCHEMA_PRIVS;
    return PG_TABLE_PRIVS;
  });

  const builderPreviewSql = $derived.by(() => {
    if (!selectedUser || grantPrivs.size === 0) return '';
    const privList = [...grantPrivs].join(', ');

    let onClause = '';
    if (isMysql) {
      if (grantScope === 'global') onClause = '*.*';
      else if (grantScope === 'database' && grantDb) onClause = `\`${grantDb}\`.*`;
      else if (grantScope === 'table' && grantDb && grantTable) onClause = `\`${grantDb}\`.\`${grantTable}\``;
      else return '';
    } else {
      if (grantScope === 'schema' && grantDb) onClause = `SCHEMA "${grantDb}"`;
      else if (grantScope === 'table' && grantTable) onClause = `TABLE "${grantTable}"`;
      else return '';
    }

    const target = isMysql
      ? `'${selectedUser.username}'@'${selectedUser.host ?? '%'}'`
      : `"${selectedUser.username}"`;
    const verb = grantAction.toUpperCase();
    const prep = grantAction === 'grant' ? 'TO' : 'FROM';
    let sql = `${verb} ${privList} ON ${onClause} ${prep} ${target}`;
    if (grantAction === 'grant' && grantWithGrantOption) sql += ' WITH GRANT OPTION';
    return sql + ';';
  });

  // Load DB list when scope requires it
  $effect(() => {
    const needsDb = isMysql ? (grantScope === 'database' || grantScope === 'table') : (grantScope === 'schema' || grantScope === 'table');
    if (needsDb && builderDbList.length === 0 && !builderDbLoading) {
      builderDbLoading = true;
      schemaApi.listDatabases(connectionId).then(dbs => {
        builderDbList = dbs;
      }).catch(() => {}).finally(() => { builderDbLoading = false; });
    }
  });

  // Load table list when scope=table and db is selected
  $effect(() => {
    if (grantScope === 'table' && grantDb) {
      builderTableLoading = true;
      builderTableList = [];
      schemaApi.listTables(connectionId, grantDb).then(tables => {
        builderTableList = tables.map(t => t.name);
      }).catch(() => {}).finally(() => { builderTableLoading = false; });
    }
  });

  // Reset builder state when selected user changes
  $effect(() => {
    void selectedUser;
    grantScope = isMysql ? 'global' : 'schema';
    grantDb = '';
    grantTable = '';
    grantPrivs = new Set();
    grantWithGrantOption = false;
    builderDbList = [];
    builderTableList = [];
  });

  function togglePriv(priv: string) {
    const next = new Set(grantPrivs);
    if (priv === 'ALL PRIVILEGES' || priv === 'ALL') {
      if (next.has(priv)) next.clear();
      else { next.clear(); next.add(priv); }
    } else {
      next.delete('ALL PRIVILEGES');
      next.delete('ALL');
      if (next.has(priv)) next.delete(priv);
      else next.add(priv);
    }
    grantPrivs = next;
  }

  function unquoteIdent(s: string): string {
    if ((s.startsWith('`') && s.endsWith('`')) || (s.startsWith('"') && s.endsWith('"'))) {
      return s.slice(1, -1);
    }
    return s;
  }

  function parseGrantSqlToBuilder(sql: string): void {
    const trimmed = sql.trim().replace(/;$/, '').trim();
    const upper = trimmed.toUpperCase();

    let action: 'grant' | 'revoke';
    let rest: string;
    if (upper.startsWith('GRANT ')) { action = 'grant'; rest = trimmed.slice(6); }
    else if (upper.startsWith('REVOKE ')) { action = 'revoke'; rest = trimmed.slice(7); }
    else return;

    const onMatch = rest.match(/^(.+?)\s+ON\s+(.+?)\s+(TO|FROM)\s+.+$/i);
    if (!onMatch) return;

    const privsStr = onMatch[1].trim();
    const onClause = onMatch[2].trim();
    const privs = privsStr.split(',').map(p => p.trim().toUpperCase()).filter(Boolean);

    let newScope = grantScope;
    let newDb = '';
    let newTable = '';

    if (isMysql) {
      if (onClause === '*.*') {
        newScope = 'global';
      } else {
        const dotIdx = onClause.indexOf('.');
        if (dotIdx !== -1) {
          const left = unquoteIdent(onClause.slice(0, dotIdx));
          const right = unquoteIdent(onClause.slice(dotIdx + 1));
          if (right === '*') {
            newScope = 'database';
            newDb = left;
          } else {
            newScope = 'table';
            newDb = left;
            newTable = right;
          }
        }
      }
    } else {
      const clauseUpper = onClause.toUpperCase();
      if (clauseUpper.startsWith('SCHEMA ')) {
        newScope = 'schema';
        newDb = unquoteIdent(onClause.slice(7).trim());
      } else if (clauseUpper.startsWith('TABLE ')) {
        const tableStr = unquoteIdent(onClause.slice(6).trim());
        const dotIdx = tableStr.indexOf('.');
        newScope = 'table';
        newTable = dotIdx !== -1 ? unquoteIdent(tableStr.slice(dotIdx + 1)) : tableStr;
      }
    }

    grantAction = action;
    grantScope = newScope;
    grantDb = newDb;
    grantTable = newTable;
    grantPrivs = new Set(privs);

    if (newDb && builderDbList.length === 0) {
      builderDbLoading = true;
      schemaApi.listDatabases(connectionId).then(dbs => {
        builderDbList = dbs;
      }).catch(() => {}).finally(() => { builderDbLoading = false; });
    }
    if (newTable && newDb && builderTableList.length === 0) {
      builderTableLoading = true;
      schemaApi.listTables(connectionId, newDb).then(tables => {
        builderTableList = tables.map(t => t.name);
      }).catch(() => {}).finally(() => { builderTableLoading = false; });
    }
  }

  function switchGrantMode(newMode: 'builder' | 'sql') {
    if (newMode === grantMode) return;
    if (newMode === 'sql') {
      if (builderPreviewSql) grantSql = builderPreviewSql;
    } else {
      if (grantSql.trim()) parseGrantSqlToBuilder(grantSql);
    }
    grantMode = newMode;
  }

  async function runBuilderGrant() {
    const sql = builderPreviewSql;
    if (!sql) return;
    grantRunning = true;
    grantError = null;
    try {
      await usersApi.executeGrant(connectionId, sql);
      if (selectedUser) loadGrants(selectedUser);
    } catch (err) {
      grantError = errorMessage(err);
    } finally {
      grantRunning = false;
    }
  }
</script>

<div class="user-manager">
  {#if isSqlite}
    <div class="unsupported-notice">
      <div class="unsupported-icon" aria-hidden="true">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>
      </div>
      <p class="unsupported-title">Not supported for SQLite</p>
      <p class="unsupported-desc">User management is not supported for SQLite connections.</p>
    </div>
  {:else}
    <!-- ── Toolbar ──────────────────────────────────────────────────────────── -->
    <div class="toolbar">
      <span class="toolbar-title">
        <span class="toolbar-kind">users</span>
        <span class="toolbar-sep">/</span>
        <span class="toolbar-conn">{profile?.name ?? connectionId}</span>
      </span>
      {#if isReadOnly}
        <span class="readonly-badge">Read Only</span>
      {:else}
        <button
          class="add-user-btn"
          disabled={isLoading}
          onclick={openAddUser}
        >+ Add User</button>
      {/if}
    </div>

    {#if isReadOnly}
      <div class="readonly-banner">
        Connection is in read-only mode — create, drop, and grant actions are disabled.
      </div>
    {/if}

    <div class="body">
      <!-- ── Left: user list ─────────────────────────────────────────────── -->
      <div class="user-list-col">
        {#if isLoading}
          <div class="state-overlay">
            <span class="loading-text">Loading…</span>
          </div>
        {:else if loadError}
          <div class="state-overlay state-overlay--error">{loadError}</div>
        {:else if users.length === 0}
          <div class="state-overlay">No users found.</div>
        {:else}
          <div class="user-list">
            {#each users as user (user.username + (user.host ?? ''))}
              <button
                class="user-row"
                class:user-row--active={selectedUser?.username === user.username && selectedUser?.host === user.host}
                onclick={() => selectUser(user)}
              >
                <div class="user-row-main">
                  <span class="user-name">{user.username}</span>
                  {#if isMysql && user.host != null}
                    <span class="user-host">@{user.host}</span>
                  {/if}
                  {#if user.isSuperuser}
                    <span class="badge badge--super">SUPER</span>
                  {/if}
                  {#if user.isLocked}
                    <span class="lock-icon" title="Account locked" aria-label="Locked">
                      <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <rect x="3" y="8" width="10" height="7" rx="1"/>
                        <path d="M5 8V5a3 3 0 0 1 6 0v3"/>
                      </svg>
                    </span>
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        {/if}

      </div>

      <!-- ── Right: detail ──────────────────────────────────────────────── -->
      <div class="detail-col">
        {#if !selectedUser}
          <div class="empty-detail">
            <p>Select a user to view grants</p>
          </div>
        {:else}
          <div class="detail-header">
            <div class="detail-title">
              <span class="detail-username">{selectedUser.username}</span>
              {#if isMysql && selectedUser.host != null}
                <span class="detail-host">@{selectedUser.host}</span>
              {/if}
            </div>
            <div class="detail-actions">
              <button
                class="action-btn"
                disabled={isReadOnly}
                onclick={openEditUser}
              >
                Edit
              </button>
              <button
                class="action-btn action-btn--danger"
                disabled={isReadOnly}
                onclick={handleDropUser}
              >
                Drop User
              </button>
            </div>
          </div>

          {#if dropError}
            <div class="inline-error">{dropError}</div>
          {/if}

          <!-- Grants section -->
          <div class="grants-section">
            <div class="section-header">
              <span>Grants</span>
              <button class="refresh-btn" onclick={() => selectedUser && loadGrants(selectedUser)} title="Refresh grants">
                <RefreshIcon width={12} height={12} />
              </button>
            </div>

            {#if grantsLoading}
              <div class="grants-loading">Loading grants…</div>
            {:else if grantsError}
              <div class="grants-error">{grantsError}</div>
            {:else if grants.length === 0}
              <div class="grants-empty">No grants found.</div>
            {:else}
              <div class="grants-list">
                {#each grants as grant (grant)}
                  <div class="grant-item mono">{grant}</div>
                {/each}
              </div>
            {/if}
          </div>

          <div class="section-sep"></div>

          <!-- Execute GRANT / REVOKE -->
          <div class="grant-exec-section">
            <div class="section-header">
              <span>Grant / Revoke</span>
              <SegmentedControl
                options={[{ value: 'builder', label: 'Builder' }, { value: 'sql', label: 'SQL' }]}
                value={grantMode}
                onchange={(v) => switchGrantMode(v as 'builder' | 'sql')}
              />
            </div>

            {#if grantMode === 'builder'}
              <div class="builder-body">
                <!-- Row 1: action + scope + db + table -->
                <div class="builder-controls">
                  <!-- GRANT / REVOKE toggle -->
                  <div class="action-toggle">
                    <button class="action-toggle-btn" class:action-toggle-btn--active={grantAction === 'grant'} onclick={() => grantAction = 'grant'} disabled={isReadOnly}>GRANT</button>
                    <button class="action-toggle-btn" class:action-toggle-btn--active={grantAction === 'revoke'} onclick={() => grantAction = 'revoke'} disabled={isReadOnly}>REVOKE</button>
                  </div>

                  <!-- Scope -->
                  <Select
                    options={scopeOptions}
                    value={grantScope}
                    size="sm"
                    disabled={isReadOnly}
                    onchange={(v) => { grantScope = v; grantDb = ''; grantTable = ''; grantPrivs = new Set(); }}
                  />

                  <!-- Database / schema selector -->
                  {#if (isMysql && (grantScope === 'database' || grantScope === 'table')) || (!isMysql && (grantScope === 'schema' || grantScope === 'table'))}
                    <Select
                      options={builderDbLoading ? [{ value: '', label: 'Loading…' }] : [{ value: '', label: isMysql ? 'Database…' : 'Schema…' }, ...builderDbList.map(d => ({ value: d, label: d }))]}
                      value={grantDb}
                      size="sm"
                      mono={true}
                      disabled={isReadOnly || builderDbLoading}
                      onchange={(v) => { grantDb = v; grantTable = ''; builderTableList = []; }}
                    />
                  {/if}

                  <!-- Table selector -->
                  {#if grantScope === 'table' && grantDb}
                    <Select
                      options={builderTableLoading ? [{ value: '', label: 'Loading…' }] : [{ value: '', label: 'Table…' }, ...builderTableList.map(t => ({ value: t, label: t }))]}
                      value={grantTable}
                      size="sm"
                      mono={true}
                      disabled={isReadOnly || builderTableLoading}
                      onchange={(v) => { grantTable = v; }}
                    />
                  {/if}
                </div>

                <!-- Privilege chips -->
                <div class="priv-chips">
                  {#each availablePrivs as priv (priv)}
                    {@const isAll = priv === 'ALL PRIVILEGES' || priv === 'ALL'}
                    {@const isSelected = grantPrivs.has(priv)}
                    {@const allSelected = grantPrivs.has('ALL PRIVILEGES') || grantPrivs.has('ALL')}
                    <button
                      class="priv-chip"
                      class:priv-chip--selected={isSelected}
                      class:priv-chip--all={isAll}
                      class:priv-chip--dimmed={!isSelected && !isAll && allSelected}
                      disabled={isReadOnly}
                      onclick={() => togglePriv(priv)}
                    >{priv}</button>
                  {/each}
                </div>

                <!-- WITH GRANT OPTION (only for GRANT action) -->
                {#if grantAction === 'grant'}
                  <label class="with-grant-option">
                    <Checkbox size="sm" checked={grantWithGrantOption} disabled={isReadOnly} onchange={(c) => { grantWithGrantOption = c; }} />
                    <span>WITH GRANT OPTION</span>
                  </label>
                {/if}

                <!-- SQL preview -->
                {#if builderPreviewSql}
                  <div class="sql-preview mono">{builderPreviewSql}</div>
                {/if}

                {#if grantError}
                  <div class="inline-error">{grantError}</div>
                {/if}
                <div class="grant-exec-footer">
                  <button
                    class="action-btn action-btn--primary"
                    disabled={isReadOnly || grantRunning || !builderPreviewSql}
                    onclick={runBuilderGrant}
                  >{grantRunning ? 'Running…' : (grantAction === 'grant' ? 'Grant' : 'Revoke')}</button>
                </div>
              </div>

            {:else}
              <!-- SQL mode -->
              <div class="grant-exec-body">
                <textarea
                  class="grant-textarea mono"
                  rows={4}
                  placeholder="GRANT SELECT ON *.* TO '...'@'...';"
                  value={grantSql}
                  disabled={isReadOnly || grantRunning}
                  autocomplete="off"
                  autocapitalize="off"
                  spellcheck={false}
                  oninput={(e) => { grantSql = (e.target as HTMLTextAreaElement).value; }}
                ></textarea>
                {#if grantError}
                  <div class="inline-error">{grantError}</div>
                {/if}
                <div class="grant-exec-footer">
                  <button
                    class="action-btn action-btn--primary"
                    disabled={isReadOnly || grantRunning || !grantSql.trim()}
                    onclick={runGrant}
                  >{grantRunning ? 'Running…' : 'Run'}</button>
                </div>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<!-- ── Add User Modal ──────────────────────────────────────────────────────── -->
{#if addUserForm}
  {@const form = addUserForm}
  <Modal label="Add User" onbackdropclick={() => !addUserSaving && (addUserForm = null)}>
    <div class="modal-card">
      <div class="modal-title">Add User</div>
      <div class="modal-body">
        <div class="form-row">
          <label class="form-label" for="add-username">Username</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="add-username"
            class="form-input"
            value={form.username}
            autofocus
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck={false}
            oninput={(e) => { addUserForm!.username = (e.target as HTMLInputElement).value; }}
            placeholder="username"
          />
        </div>
        {#if isMysql}
          <div class="form-row">
            <label class="form-label" for="add-host">Host</label>
            <input
              id="add-host"
              class="form-input"
              value={form.host}
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck={false}
              oninput={(e) => { addUserForm!.host = (e.target as HTMLInputElement).value; }}
              placeholder="%"
            />
          </div>
        {/if}
        <div class="form-row">
          <label class="form-label" for="add-password">Password</label>
          <input
            id="add-password"
            type="password"
            class="form-input"
            value={form.password}
            autocomplete="new-password"
            oninput={(e) => { addUserForm!.password = (e.target as HTMLInputElement).value; }}
            placeholder="password"
          />
        </div>
        {#if isPostgres}
          <div class="form-check-row">
            <input
              type="checkbox"
              id="add-superuser"
              checked={form.isSuperuser}
              onchange={(e) => { addUserForm!.isSuperuser = (e.target as HTMLInputElement).checked; }}
            />
            <label for="add-superuser" class="form-check-label">Superuser</label>
          </div>
          <div class="form-check-row">
            <input
              type="checkbox"
              id="add-createdb"
              checked={form.canCreateDb}
              onchange={(e) => { addUserForm!.canCreateDb = (e.target as HTMLInputElement).checked; }}
            />
            <label for="add-createdb" class="form-check-label">Can Create DB</label>
          </div>
          <div class="form-check-row">
            <input
              type="checkbox"
              id="add-createrole"
              checked={form.canCreateRole}
              onchange={(e) => { addUserForm!.canCreateRole = (e.target as HTMLInputElement).checked; }}
            />
            <label for="add-createrole" class="form-check-label">Can Create Role</label>
          </div>
        {/if}
        {#if addUserError}
          <div class="modal-error">{addUserError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (addUserForm = null)} disabled={addUserSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitAddUser} disabled={addUserSaving}>
          {addUserSaving ? 'Creating…' : 'Create User'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Set Password Modal ──────────────────────────────────────────────────── -->
{#if setPasswordForm && selectedUser}
  {@const user = selectedUser}
  <Modal label="Set Password" onbackdropclick={() => !setPasswordSaving && (setPasswordForm = null)}>
    <div class="modal-card">
      <div class="modal-title">Set Password</div>
      <div class="modal-body">
        <p class="modal-subtitle">
          {isMysql ? `'${user.username}'@'${user.host ?? '%'}'` : user.username}
        </p>
        <div class="form-row">
          <label class="form-label" for="set-password">New Password</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="set-password"
            type="password"
            class="form-input"
            value={setPasswordForm.password}
            autofocus
            autocomplete="new-password"
            oninput={(e) => { setPasswordForm!.password = (e.target as HTMLInputElement).value; }}
            placeholder="new password"
          />
        </div>
        {#if setPasswordError}
          <div class="modal-error">{setPasswordError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (setPasswordForm = null)} disabled={setPasswordSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitSetPassword} disabled={setPasswordSaving}>
          {setPasswordSaving ? 'Saving…' : 'Set Password'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Edit User Modal ────────────────────────────────────────────────────── -->
{#if editUserForm && selectedUser}
  {@const form = editUserForm}
  <Modal label="Edit User" onbackdropclick={() => !editUserSaving && (editUserForm = null)}>
    <div class="modal-card">
      <div class="modal-title">Edit User</div>
      <div class="modal-body">
        <div class="form-row">
          <label class="form-label" for="edit-username">Username</label>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="edit-username"
            class="form-input"
            value={form.username}
            autofocus
            autocomplete="off"
            autocorrect="off"
            autocapitalize="off"
            spellcheck={false}
            oninput={(e) => { editUserForm!.username = (e.target as HTMLInputElement).value; }}
          />
        </div>
        {#if isMysql}
          <div class="form-row">
            <label class="form-label" for="edit-host">Host</label>
            <input
              id="edit-host"
              class="form-input"
              value={form.host}
              autocomplete="off"
              autocorrect="off"
              autocapitalize="off"
              spellcheck={false}
              oninput={(e) => { editUserForm!.host = (e.target as HTMLInputElement).value; }}
            />
          </div>
        {/if}
        <div class="form-row">
          <label class="form-label" for="edit-password">New Password <span class="form-label-hint">(leave blank to keep current)</span></label>
          <input
            id="edit-password"
            type="password"
            class="form-input"
            value={form.password}
            autocomplete="new-password"
            placeholder="unchanged"
            oninput={(e) => { editUserForm!.password = (e.target as HTMLInputElement).value; }}
          />
        </div>
        {#if editUserError}
          <div class="modal-error">{editUserError}</div>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn" onclick={() => (editUserForm = null)} disabled={editUserSaving}>Cancel</button>
        <button class="btn btn--primary" onclick={submitEditUser} disabled={editUserSaving}>
          {editUserSaving ? 'Saving…' : 'Save'}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ── Drop User Confirm ──────────────────────────────────────────────────── -->
{#if dropConfirmUser}
  {@const label = isMysql
    ? `'${dropConfirmUser.username}'@'${dropConfirmUser.host ?? '%'}'`
    : dropConfirmUser.username}
  <ConfirmDialog
    title="Drop User"
    message="Drop user {label}? This cannot be undone."
    confirmText="Drop User"
    danger={true}
    onconfirm={confirmDropUser}
    oncancel={() => (dropConfirmUser = null)}
  />
{/if}

<style>
  .user-manager {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  /* ── Unsupported ────────────────────────────────────────────────────────── */

  .unsupported-notice {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: var(--spacing-2);
    color: var(--color-text-muted);
    padding: var(--spacing-4);
    text-align: center;
  }

  .unsupported-icon {
    font-size: 36px;
    opacity: 0.5;
    margin-bottom: var(--spacing-2);
  }

  .unsupported-title {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
    margin: 0;
  }

  .unsupported-desc {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    margin: 0;
  }

  /* ── Toolbar ────────────────────────────────────────────────────────────── */

  .toolbar {
    flex-shrink: 0;
    height: var(--toolbar-height);
    min-height: var(--toolbar-height);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
    border-bottom: 1px solid var(--color-border);
  }

  .toolbar-title {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    min-width: 0;
  }

  .toolbar-kind {
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-disabled);
    text-transform: uppercase;
    letter-spacing: 0.07em;
    flex-shrink: 0;
  }

  .toolbar-sep {
    color: var(--color-border-strong);
    flex-shrink: 0;
    font-size: var(--font-size-xs);
  }

  .toolbar-conn {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .readonly-badge {
    margin-left: auto;
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-warning, #f59e0b);
    background: var(--color-warning-subtle, rgba(245, 158, 11, 0.1));
    border: 1px solid rgba(245, 158, 11, 0.3);
    padding: 2px 7px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    flex-shrink: 0;
  }

  /* ── Read-only banner ───────────────────────────────────────────────────── */

  .readonly-banner {
    flex-shrink: 0;
    padding: 6px var(--spacing-3);
    background: var(--color-warning-subtle, rgba(245, 158, 11, 0.08));
    border-bottom: 1px solid rgba(245, 158, 11, 0.25);
    font-size: var(--font-size-xs);
    color: var(--color-warning, #f59e0b);
  }

  /* ── Body layout ────────────────────────────────────────────────────────── */

  .body {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── User list column ───────────────────────────────────────────────────── */

  .user-list-col {
    width: 220px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--color-border);
    overflow: hidden;
  }

  .user-list {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-2) 0;
    min-height: 0;
  }

  .user-row {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 6px var(--spacing-3);
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    transition: background var(--transition-fast);
  }

  .user-row:hover {
    background: var(--color-bg-hover);
  }

  .user-row--active {
    background: var(--color-accent-subtle);
  }

  .user-row--active:hover {
    background: var(--color-accent-subtle);
  }

  .user-row-main {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    min-width: 0;
    width: 100%;
  }

  .user-name {
    font-size: var(--font-size-sm);
    color: var(--color-text-primary);
    font-weight: var(--font-weight-medium);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .user-row--active .user-name {
    color: var(--color-accent);
  }

  .user-host {
    font-size: 11px;
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
  }

  .lock-icon {
    flex-shrink: 0;
    color: var(--color-text-muted);
    display: flex;
    align-items: center;
    margin-left: auto;
  }

  .add-user-btn {
    margin-left: auto;
    height: 24px;
    padding: 0 var(--spacing-2);
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: all var(--transition-fast);
    font-family: var(--font-family-ui);
    flex-shrink: 0;
  }

  .add-user-btn:hover:not(:disabled) {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .add-user-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* ── Detail column ──────────────────────────────────────────────────────── */

  .detail-col {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .empty-detail {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }

  .detail-header {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: var(--spacing-3) var(--spacing-4);
    border-bottom: 1px solid var(--color-border);
    gap: var(--spacing-3);
    flex-wrap: wrap;
  }

  .detail-title {
    display: flex;
    align-items: baseline;
    gap: 4px;
    overflow: hidden;
    min-width: 0;
    flex: 1;
  }

  .detail-username {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-family-mono);
  }

  .detail-host {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    flex-shrink: 0;
  }

  .detail-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex-shrink: 0;
  }

  .action-btn {
    height: 26px;
    padding: 0 12px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all var(--transition-fast);
    white-space: nowrap;
    font-family: var(--font-family-ui);
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn--danger {
    color: var(--color-danger, #ef4444);
    border-color: rgba(239, 68, 68, 0.3);
  }

  .action-btn--danger:hover:not(:disabled) {
    background: var(--color-danger-subtle, rgba(239, 68, 68, 0.08));
    border-color: rgba(239, 68, 68, 0.5);
    color: var(--color-danger, #ef4444);
  }

  .action-btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .action-btn--primary:hover:not(:disabled) {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }

  .inline-error {
    margin: var(--spacing-2) var(--spacing-4);
    padding: 7px 10px;
    background: var(--color-danger-subtle, rgba(239, 68, 68, 0.08));
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: var(--radius-md);
    color: var(--color-danger, #ef4444);
    font-size: var(--font-size-xs);
  }

  /* ── Grants section ─────────────────────────────────────────────────────── */

  .grants-section {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--color-border);
    max-height: 260px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 5px var(--spacing-4);
    font-size: 10px;
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    border-bottom: 1px solid var(--color-border);
    flex-shrink: 0;
  }

  .refresh-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    cursor: pointer;
    padding: 0;
    transition: all var(--transition-fast);
  }

  .refresh-btn:hover {
    background: var(--color-bg-hover);
    border-color: var(--color-border);
    color: var(--color-text-primary);
  }

  .grants-loading,
  .grants-empty,
  .grants-error {
    padding: var(--spacing-3) var(--spacing-4);
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    font-style: italic;
  }

  .grants-error {
    color: var(--color-danger, #ef4444);
    font-style: normal;
  }

  .grants-list {
    overflow-y: auto;
    padding: var(--spacing-2) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .grant-item {
    padding: 6px 10px;
    background: var(--color-bg-elevated, var(--color-bg-hover));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    word-break: break-all;
    line-height: 1.5;
  }

  /* ── Section separator ──────────────────────────────────────────────────── */

  .section-sep {
    flex-shrink: 0;
    height: 1px;
    background: var(--color-border);
  }

  /* ── Grant exec section ─────────────────────────────────────────────────── */

  .grant-exec-section {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .grant-exec-body {
    padding: var(--spacing-3) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .grant-textarea {
    resize: vertical;
    min-height: 72px;
    padding: var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-size: var(--font-size-xs);
    font-family: var(--font-family-mono);
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
    line-height: 1.5;
  }

  .grant-textarea:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .grant-textarea:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .grant-exec-footer {
    display: flex;
    justify-content: flex-end;
  }

  /* ── State overlays ─────────────────────────────────────────────────────── */

  .state-overlay {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-4);
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    font-style: italic;
  }

  .state-overlay--error {
    color: var(--color-danger, #ef4444);
    font-style: normal;
  }

  .loading-text {
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  /* ── Badges ─────────────────────────────────────────────────────────────── */

  .badge {
    font-size: 9px;
    font-weight: var(--font-weight-semibold);
    padding: 1px 5px;
    border-radius: var(--radius-sm);
    letter-spacing: 0.04em;
    border: 1px solid transparent;
    flex-shrink: 0;
  }

  .badge--super {
    background: var(--color-accent-subtle);
    color: var(--color-accent);
    border-color: rgba(124, 92, 255, 0.22);
  }

  /* ── Modal card ─────────────────────────────────────────────────────────── */

  .modal-card {
    background: var(--color-bg-overlay);
    border: 1px solid var(--color-border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-overlay);
    width: 400px;
    max-width: 92vw;
    overflow: hidden;
    animation: modal-in 140ms ease both;
  }

  @keyframes modal-in {
    from { opacity: 0; transform: scale(0.96) translateY(-6px); }
    to   { opacity: 1; transform: scale(1)    translateY(0); }
  }

  .modal-title {
    padding: var(--spacing-4) var(--spacing-4) var(--spacing-3);
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
    border-bottom: 1px solid var(--color-border);
  }

  .modal-subtitle {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
    font-family: var(--font-family-mono);
    margin: 0;
  }

  .modal-body {
    padding: var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
    max-height: 60vh;
    overflow-y: auto;
  }

  .modal-footer {
    padding: var(--spacing-3) var(--spacing-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    justify-content: flex-end;
  }

  /* ── Form elements ──────────────────────────────────────────────────────── */

  .form-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .form-label {
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    color: var(--color-text-secondary);
  }

  .form-label-hint {
    font-weight: var(--font-weight-normal, 400);
    color: var(--color-text-muted);
  }

  .form-input {
    height: 30px;
    padding: 0 var(--spacing-2);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    background: var(--color-bg-secondary);
    color: var(--color-text-primary);
    font-family: var(--font-family-mono);
    font-size: var(--font-size-sm);
    outline: none;
    width: 100%;
    box-sizing: border-box;
    transition: border-color var(--transition-fast), box-shadow var(--transition-fast);
  }

  .form-input:focus {
    border-color: var(--color-accent);
    box-shadow: 0 0 0 2px var(--color-accent-subtle);
  }

  .form-check-row {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
  }

  .form-check-row input[type="checkbox"] {
    width: 14px;
    height: 14px;
    accent-color: var(--color-accent);
    cursor: pointer;
    flex-shrink: 0;
  }

  .form-check-label {
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
  }

  /* ── Modal buttons ──────────────────────────────────────────────────────── */

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

  .btn:hover:not(:disabled) {
    border-color: var(--color-border-strong);
    color: var(--color-text-primary);
    background: var(--color-bg-hover);
  }

  .btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .btn--primary {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .btn--primary:hover:not(:disabled) {
    opacity: 0.88;
    border-color: var(--color-accent);
    background: var(--color-accent);
    color: white;
  }

  .modal-error {
    padding: 8px 10px;
    background: var(--color-danger-subtle, rgba(239, 68, 68, 0.08));
    border: 1px solid rgba(239, 68, 68, 0.35);
    border-radius: var(--radius-md);
    color: var(--color-danger, #ef4444);
    font-size: var(--font-size-xs);
    line-height: var(--line-height-normal);
  }

  .mono {
    font-family: var(--font-family-mono);
  }

  /* ── Builder mode ─────────────────────────────────────────────────────────── */

  .builder-body {
    padding: var(--spacing-3) var(--spacing-4);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-3);
  }

  .builder-controls {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    flex-wrap: wrap;
  }

  .action-toggle {
    display: flex;
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    overflow: hidden;
    flex-shrink: 0;
  }

  .action-toggle-btn {
    height: 28px;
    padding: 0 10px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-semibold);
    border: none;
    background: var(--color-bg-secondary);
    color: var(--color-text-muted);
    cursor: pointer;
    font-family: var(--font-family-ui);
    transition: all var(--transition-fast);
    letter-spacing: 0.04em;
  }

  .action-toggle-btn + .action-toggle-btn {
    border-left: 1px solid var(--color-border);
  }

  .action-toggle-btn--active {
    background: var(--color-accent);
    color: white;
  }

  .action-toggle-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* ── Privilege chips ──────────────────────────────────────────────────────── */

  .priv-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .priv-chip {
    height: 22px;
    padding: 0 8px;
    font-size: var(--font-size-xs);
    font-weight: var(--font-weight-medium);
    font-family: var(--font-family-mono);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text-secondary);
    cursor: pointer;
    transition: all var(--transition-fast);
    white-space: nowrap;
  }

  .priv-chip:hover:not(:disabled) {
    border-color: var(--color-accent);
    color: var(--color-accent);
    background: var(--color-accent-subtle);
  }

  .priv-chip--selected {
    background: var(--color-accent);
    border-color: var(--color-accent);
    color: white;
  }

  .priv-chip--selected:hover:not(:disabled) {
    opacity: 0.85;
    background: var(--color-accent);
    color: white;
  }

  .priv-chip--all {
    font-weight: var(--font-weight-semibold);
  }

  .priv-chip--dimmed {
    opacity: 0.3;
  }

  .priv-chip:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* ── WITH GRANT OPTION ────────────────────────────────────────────────────── */

  .with-grant-option {
    display: flex;
    align-items: center;
    gap: var(--spacing-2);
    font-size: var(--font-size-xs);
    color: var(--color-text-secondary);
    cursor: pointer;
  }

  /* ── SQL preview ──────────────────────────────────────────────────────────── */

  .sql-preview {
    padding: 8px 10px;
    background: var(--color-bg-elevated, var(--color-bg-hover));
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-size: var(--font-size-xs);
    color: var(--color-accent);
    word-break: break-all;
    line-height: 1.5;
  }
</style>
