import { invoke } from '@tauri-apps/api/core';
import type {
  ServerAdminCapabilityFlags,
  ProcessInfo,
  ServerStatus,
  ServerVariable,
  VarScope,
  LockInfo,
  ScheduledJob,
  VacuumInfo,
} from '$lib/types';

export function getCapabilities(connectionId: string): Promise<ServerAdminCapabilityFlags> {
  return invoke('server_admin_get_capabilities', { connectionId });
}

export function listProcesses(connectionId: string): Promise<ProcessInfo[]> {
  return invoke('server_admin_list_processes', { connectionId });
}

export function killSession(connectionId: string, sessionId: string): Promise<void> {
  return invoke('server_admin_kill_session', { connectionId, sessionId });
}

export function cancelSession(connectionId: string, pid: string): Promise<void> {
  return invoke('server_admin_cancel_session', { connectionId, pid });
}

export function getStatus(connectionId: string): Promise<ServerStatus> {
  return invoke('server_admin_get_status', { connectionId });
}

export function listVariables(connectionId: string): Promise<ServerVariable[]> {
  return invoke('server_admin_list_variables', { connectionId });
}

export function setVariable(
  connectionId: string,
  name: string,
  value: string,
  scope: VarScope,
): Promise<void> {
  return invoke('server_admin_set_variable', { connectionId, name, value, scope });
}

export function listLocks(connectionId: string): Promise<LockInfo[]> {
  return invoke('server_admin_list_locks', { connectionId });
}

export function listScheduledJobs(connectionId: string): Promise<ScheduledJob[]> {
  return invoke('server_admin_list_scheduled_jobs', { connectionId });
}

export function getInnodbStatus(connectionId: string): Promise<string> {
  return invoke('server_admin_get_innodb_status', { connectionId });
}

export function getVacuumStatus(connectionId: string): Promise<VacuumInfo[]> {
  return invoke('server_admin_get_vacuum_status', { connectionId });
}
