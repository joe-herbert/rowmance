/**
 * Connections store.
 * Manages the list of saved connection profiles and tracks which connections
 * are currently active (i.e. have an open pool in the Rust backend).
 */
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
      [profiles, groups] = await Promise.all([api.listConnections(), api.listConnectionGroups()]);
      const ids = await api.listActiveConnections();
      activeIds = new Set(ids);
      const now = new Date();
      connectedAt = new Map(ids.map((id) => [id, now]));
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

    /** Toggle read-only mode for a connection. Reconnects the pool if active. */
    async toggleReadOnly(id: string): Promise<void> {
      const profile = profiles.find((p) => p.id === id);
      if (!profile) return;
      const wasActive = activeIds.has(id);
      const updated = await api.updateConnection(id, {
        name: profile.name,
        dbType: profile.dbType,
        host: profile.host,
        port: profile.port,
        database: profile.database,
        username: profile.username,
        color: profile.color,
        readOnly: !profile.readOnly,
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
      });
      profiles = profiles.map((p) => (p.id === id ? updated : p));
      if (wasActive) {
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

    /** Close the connection pool for a profile. */
    async disconnect(id: string): Promise<void> {
      await api.disconnectFromDatabase(id);
      activeIds = new Set([...activeIds].filter((i) => i !== id));
      connectedAt = new Map([...connectedAt].filter(([k]) => k !== id));
      transactionIds = new Set([...transactionIds].filter((i) => i !== id));
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
    setTransactionActive(id: string, active: boolean): void {
      if (active) {
        transactionIds = new Set([...transactionIds, id]);
      } else {
        transactionIds = new Set([...transactionIds].filter((i) => i !== id));
      }
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
