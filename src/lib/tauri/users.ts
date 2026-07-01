import { invoke } from '@tauri-apps/api/core';
import type { DbUser } from '$lib/types';

export async function listUsers(connectionId: string): Promise<DbUser[]> {
  return invoke<DbUser[]>('users_list', { connectionId });
}

export async function getGrants(
  connectionId: string,
  username: string,
  host: string | null,
): Promise<string[]> {
  return invoke<string[]>('users_get_grants', { connectionId, username, host });
}

export async function createUser(
  connectionId: string,
  username: string,
  host: string | null,
  password: string,
  isSuperuser: boolean,
  canCreateDb: boolean,
  canCreateRole: boolean,
): Promise<void> {
  return invoke<void>('users_create', {
    connectionId,
    username,
    host,
    password,
    isSuperuser,
    canCreateDb,
    canCreateRole,
  });
}

export async function dropUser(
  connectionId: string,
  username: string,
  host: string | null,
): Promise<void> {
  return invoke<void>('users_drop', { connectionId, username, host });
}

export async function renameUser(
  connectionId: string,
  username: string,
  host: string | null,
  newUsername: string,
  newHost: string | null,
): Promise<void> {
  return invoke<void>('users_rename', { connectionId, username, host, newUsername, newHost });
}

export async function setPassword(
  connectionId: string,
  username: string,
  host: string | null,
  password: string,
): Promise<void> {
  return invoke<void>('users_set_password', { connectionId, username, host, password });
}

export async function executeGrant(connectionId: string, sql: string): Promise<void> {
  return invoke<void>('users_execute_grant', { connectionId, sql });
}
