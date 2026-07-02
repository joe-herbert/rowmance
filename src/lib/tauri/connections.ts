/** Typed wrappers around the Tauri `connections::*` commands. */
import { invoke } from '@tauri-apps/api/core';
import type {
  ConnectionProfile,
  ConnectionProfileInput,
  ConnectionGroup,
  ConnectionGroupInput,
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

export async function testConnection(id: string, password?: string): Promise<ConnectionTestResult> {
  return invoke<ConnectionTestResult>('connections_test', { id, password });
}

export async function testConnectionUnsaved(
  input: ConnectionProfileInput,
  password?: string,
  sshPassword?: string,
): Promise<ConnectionTestResult> {
  return invoke<ConnectionTestResult>('connections_test_unsaved', { input, password, sshPassword });
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

export async function updateConnectionGroup(
  id: string,
  input: ConnectionGroupInput,
): Promise<ConnectionGroup> {
  return invoke<ConnectionGroup>('connection_groups_update', { id, input });
}

export async function deleteConnectionGroup(id: string): Promise<void> {
  return invoke<void>('connection_groups_delete', { id });
}

export interface ConnectionImportResult {
  imported: number;
}

export async function exportConnections(
  ids: string[],
  filePath: string,
  includeSensitive: boolean,
): Promise<void> {
  return invoke<void>('connections_export', { ids, filePath, includeSensitive });
}

export async function importConnections(filePath: string): Promise<ConnectionImportResult> {
  return invoke<ConnectionImportResult>('connections_import', { filePath });
}

export async function getConnectionDbUrl(id: string): Promise<string> {
  return invoke<string>('connections_get_db_url', { id });
}
