/**
 * Connections store.
 * Manages the list of saved connection profiles and tracks which connections
 * are currently active (i.e. have an open pool in the Rust backend).
 */
import { untrack } from 'svelte';
import type { ConnectionProfile, ConnectionGroup, ConnectionProfileInput } from '$lib/types';
import * as api from '$lib/tauri/connections';
import { errorMessage } from '$lib/utils/errors';

// ── State ─────────────────────────────────────────────────────────────────────

let profiles = $state<ConnectionProfile[]>([]);
let groups = $state<ConnectionGroup[]>([]);
let activeIds = $state<Set<string>>(new Set());
let connectingIds = $state<Set<string>>(new Set());
let errorIds = $state<Map<string, string>>(new Map());
let connectedAt = $state<Map<string, Date>>(new Map());
let transactionIds = $state<Set<string>>(new Set());
let txQueries = $state<Map<string, string[]>>(new Map());
let txDatabases = $state<Map<string, string | undefined>>(new Map());

// ── Public interface ──────────────────────────────────────────────────────────

export function useConnections() {
  return {
    get profiles() {
      return profiles;
    },
    get groups() {
      return groups;
    },
    get activeIds() {
      return activeIds;
    },
    get connectingIds() {
      return connectingIds;
    },
    get errorIds() {
      return errorIds;
    },

    /** Load all profiles and groups from the backend. Call once at startup. */
    async load() {
      // Read via untrack — this runs synchronously (before the first await) inside
      // whatever effect called load(), and a tracked read here would make that
      // effect depend on `profiles`. Since this function also writes `profiles`,
      // that would re-trigger the calling effect every time load() runs, which
      // calls load() again — an infinite loop.
      const previousProfiles = untrack(() => profiles);
      let loaded: ConnectionProfile[];
      [loaded, groups] = await Promise.all([api.listConnections(), api.listConnectionGroups()]);
      const ids = await api.listActiveConnections();
      activeIds = new Set(ids);
      const now = new Date();
      connectedAt = new Map(ids.map((id) => [id, now]));
      // Unsaved connections live only in the backend's in-memory pool registry,
      // never in SQLite, so `listConnections()` won't return them — preserve any
      // still active, and drop any that have since been disconnected.
      const unsavedProfiles = previousProfiles.filter((p) => p.unsaved && activeIds.has(p.id));
      profiles = [...loaded, ...unsavedProfiles];
    },

    /** Create a new connection profile. */
    async create(input: ConnectionProfileInput): Promise<ConnectionProfile> {
      const created = await api.createConnection(input);
      profiles = [...profiles, created];
      return created;
    },

    /** Update an existing connection profile. */
    async update(id: string, input: ConnectionProfileInput): Promise<ConnectionProfile> {
      const updated = await api.updateConnection(id, input);
      profiles = profiles.map((p) => (p.id === id ? updated : p));
      return updated;
    },

    /**
     * Toggle read-only mode for a connection. Reconnects the pool if active.
     * Read-only and Safe Mode are mutually exclusive — enabling one disables the other.
     */
    async toggleReadOnly(id: string): Promise<void> {
      const profile = profiles.find((p) => p.id === id);
      if (!profile) return;
      const wasActive = activeIds.has(id);
      const newReadOnly = !profile.readOnly;
      const updated = await api.updateConnection(id, {
        name: profile.name,
        dbType: profile.dbType,
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        color: profile.color,
        readOnly: newReadOnly,
        groupId: profile.groupId,
        sshEnabled: profile.sshEnabled,
        sshHost: profile.sshHost,
        sshPort: profile.sshPort,
        sshUser: profile.sshUser,
        sshAuthType: profile.sshAuthType,
        sshKeyPath: profile.sshKeyPath,
        sslEnabled: profile.sslEnabled,
        sslCaPath: profile.sslCaPath,
        sslCertPath: profile.sslCertPath,
        sslKeyPath: profile.sslKeyPath,
        poolMax: profile.poolMax,
        safeMode: newReadOnly ? false : profile.safeMode,
      });
      profiles = profiles.map((p) => (p.id === id ? updated : p));
      if (wasActive) {
        await this.disconnect(id);
        await this.connect(id);
      }
    },

    /**
     * Toggle Safe Mode for a connection (warn before running mutating SQL).
     * Read-only and Safe Mode are mutually exclusive — enabling one disables the other.
     * Reconnects the pool if active and read-only actually changed as a result.
     */
    async toggleSafeMode(id: string): Promise<void> {
      const profile = profiles.find((p) => p.id === id);
      if (!profile) return;
      const wasActive = activeIds.has(id);
      const newSafeMode = !profile.safeMode;
      const newReadOnly = newSafeMode ? false : profile.readOnly;
      const updated = await api.updateConnection(id, {
        name: profile.name,
        dbType: profile.dbType,
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        color: profile.color,
        readOnly: newReadOnly,
        groupId: profile.groupId,
        sshEnabled: profile.sshEnabled,
        sshHost: profile.sshHost,
        sshPort: profile.sshPort,
        sshUser: profile.sshUser,
        sshAuthType: profile.sshAuthType,
        sshKeyPath: profile.sshKeyPath,
        sslEnabled: profile.sslEnabled,
        sslCaPath: profile.sslCaPath,
        sslCertPath: profile.sslCertPath,
        sslKeyPath: profile.sslKeyPath,
        poolMax: profile.poolMax,
        safeMode: newSafeMode,
      });
      profiles = profiles.map((p) => (p.id === id ? updated : p));
      if (wasActive && newReadOnly !== profile.readOnly) {
        await this.disconnect(id);
        await this.connect(id);
      }
    },

    /** Delete a connection profile and disconnect if active. */
    async delete(id: string): Promise<void> {
      if (activeIds.has(id)) {
        await this.disconnect(id);
      }
      await api.deleteConnection(id);
      profiles = profiles.filter((p) => p.id !== id);
    },

    /** Open a connection pool for a profile. */
    async connect(id: string): Promise<void> {
      connectingIds = new Set([...connectingIds, id]);
      errorIds = new Map([...errorIds].filter(([k]) => k !== id));

      try {
        await api.connectToDatabase(id);
        activeIds = new Set([...activeIds, id]);
        connectedAt = new Map([...connectedAt, [id, new Date()]]);
      } catch (err) {
        const message = errorMessage(err);
        errorIds = new Map([...errorIds, [id, message]]);
        throw err;
      } finally {
        connectingIds = new Set([...connectingIds].filter((i) => i !== id));
      }
    },

    /**
     * Connect using raw input without persisting a profile to SQLite.
     * The returned profile is added to the local list only, tagged `unsaved`,
     * and is removed again as soon as it disconnects.
     */
    async connectUnsaved(
      input: ConnectionProfileInput,
      password?: string,
      sshPassword?: string,
    ): Promise<ConnectionProfile> {
      const created = await api.connectUnsaved(input, password, sshPassword);
      const profile: ConnectionProfile = { ...created, unsaved: true };
      profiles = [...profiles, profile];
      activeIds = new Set([...activeIds, profile.id]);
      connectedAt = new Map([...connectedAt, [profile.id, new Date()]]);
      return profile;
    },

    /** Close the connection pool for a profile. */
    async disconnect(id: string): Promise<void> {
      await api.disconnectFromDatabase(id);
      activeIds = new Set([...activeIds].filter((i) => i !== id));
      connectedAt = new Map([...connectedAt].filter(([k]) => k !== id));
      transactionIds = new Set([...transactionIds].filter((i) => i !== id));
      if (profiles.find((p) => p.id === id)?.unsaved) {
        profiles = profiles.filter((p) => p.id !== id);
      }
    },

    /** Update local state when the backend has already disconnected (e.g. SSH dropped). */
    markDisconnected(id: string): void {
      activeIds = new Set([...activeIds].filter((i) => i !== id));
      connectedAt = new Map([...connectedAt].filter(([k]) => k !== id));
      transactionIds = new Set([...transactionIds].filter((i) => i !== id));
      if (profiles.find((p) => p.id === id)?.unsaved) {
        profiles = profiles.filter((p) => p.id !== id);
      }
    },

    /** Check whether a given profile ID is currently connected. */
    isActive(id: string): boolean {
      return activeIds.has(id);
    },

    /** Check whether a transaction is active for the given connection. */
    isTransactionActive(id: string): boolean {
      return transactionIds.has(id);
    },

    /** Mark a transaction as started or ended for the given connection. */
    setTransactionActive(id: string, active: boolean, database?: string): void {
      if (active) {
        transactionIds = new Set([...transactionIds, id]);
        txQueries = new Map([...txQueries, [id, []]]);
        txDatabases = new Map([...txDatabases, [id, database]]);
      } else {
        transactionIds = new Set([...transactionIds].filter((i) => i !== id));
        txQueries = new Map([...txQueries].filter(([k]) => k !== id));
        txDatabases = new Map([...txDatabases].filter(([k]) => k !== id));
      }
    },

    /** Return the database the active transaction was started on, or undefined. */
    getTxDatabase(id: string): string | undefined {
      return txDatabases.get(id);
    },

    /** Append a SQL string to the transaction query log for the given connection. */
    addTxQuery(id: string, sql: string): void {
      const existing = txQueries.get(id) ?? [];
      txQueries = new Map([...txQueries, [id, [...existing, sql]]]);
    },

    /** Return the list of queries run during the active transaction, or an empty array. */
    getTxQueries(id: string): string[] {
      return txQueries.get(id) ?? [];
    },

    /** Return the profile with the given ID, or undefined. */
    getById(id: string): ConnectionProfile | undefined {
      return profiles.find((p) => p.id === id);
    },

    /** Return the timestamp when this connection was established, or undefined. */
    getConnectedAt(id: string): Date | undefined {
      return connectedAt.get(id);
    },
  };
}
