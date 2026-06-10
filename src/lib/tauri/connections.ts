/** Typed wrappers around the Tauri `connections::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type {
  ConnectionProfile,
  ConnectionProfileInput,
  ConnectionGroup,
  ConnectionTestResult,
} from '$lib/types';

export async function listConnections(): Promise<ConnectionProfile[]> {
  return invoke<ConnectionProfile[]>('connections_list');
}

export async function createConnection(input: ConnectionProfileInput): Promise<ConnectionProfile> {
  return invoke<ConnectionProfile>('connections_create', { input });
}

export async function updateConnection(
  id: string,
  input: ConnectionProfileInput,
): Promise<ConnectionProfile> {
  return invoke<ConnectionProfile>('connections_update', { id, input });
}

export async function deleteConnection(id: string): Promise<void> {
  return invoke<void>('connections_delete', { id });
}

export async function testConnection(id: string): Promise<ConnectionTestResult> {
  return invoke<ConnectionTestResult>('connections_test', { id });
}

export async function connectToDatabase(id: string): Promise<void> {
  return invoke<void>('connections_connect', { id });
}

export async function disconnectFromDatabase(id: string): Promise<void> {
  return invoke<void>('connections_disconnect', { id });
}

export async function listActiveConnections(): Promise<string[]> {
  return invoke<string[]>('connections_list_active');
}

export async function listConnectionGroups(): Promise<ConnectionGroup[]> {
  return invoke<ConnectionGroup[]>('connection_groups_list');
}

export async function createConnectionGroup(
  name: string,
  parentId?: string,
): Promise<ConnectionGroup> {
  return invoke<ConnectionGroup>('connection_groups_create', { input: { name, parentId } });
}

export async function deleteConnectionGroup(id: string): Promise<void> {
  return invoke<void>('connection_groups_delete', { id });
}
