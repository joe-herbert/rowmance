import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  getCapabilities,
  listProcesses,
  killSession,
  cancelSession,
  getStatus,
  listVariables,
  setVariable,
  listLocks,
  listScheduledJobs,
  getInnodbStatus,
  getVacuumStatus,
} from './server_admin';
import type {
  ServerAdminCapabilityFlags,
  ProcessInfo,
  ServerStatus,
  ServerVariable,
  LockInfo,
  ScheduledJob,
  VacuumInfo,
} from '$lib/types';

const mockInvoke = vi.mocked(invoke);

const stubCapabilities: ServerAdminCapabilityFlags = {
  processList: { status: 'supported' },
  killSession: { status: 'supported' },
  cancelSession: { status: 'notSupported' },
  serverStatus: { status: 'supported' },
  variables: { status: 'supported' },
  setVariable: { status: 'supported' },
  scheduledJobs: { status: 'insufficientPrivileges' },
  locks: { status: 'supported' },
  innodbStatus: { status: 'notSupported' },
  vacuumStatus: { status: 'notSupported' },
};

const stubProcess: ProcessInfo = {
  id: '42',
  user: 'root',
  host: 'localhost',
  database: 'mydb',
  command: 'Query',
  timeSeconds: 5,
  state: 'executing',
  info: 'SELECT 1',
  canKill: true,
  canCancel: false,
};

const stubStatus: ServerStatus = {
  version: '8.0.33',
  uptimeSeconds: 86400,
  connectionsCurrent: 10,
  connectionsMax: 151,
  queriesPerSecond: 42.5,
  cacheHitRatio: 0.98,
  extra: {},
};

const stubVariable: ServerVariable = {
  name: 'max_connections',
  value: '151',
  scope: 'global',
  isDynamic: true,
  restartRequired: false,
  description: null,
  dataType: null,
};

const stubLock: LockInfo = {
  lockId: 'lock-1',
  blockerSessionId: '10',
  waitingSessionId: '20',
  lockType: 'RECORD',
  lockMode: 'X',
  objectName: 'users',
  durationMs: 1500,
};

const stubJob: ScheduledJob = {
  id: 'job-1',
  name: 'nightly_backup',
  schedule: '0 2 * * *',
  enabled: true,
  lastRun: '2026-07-16T02:00:00',
  nextRun: '2026-07-17T02:00:00',
  body: null,
};

const stubVacuum: VacuumInfo = {
  table: 'users',
  lastVacuum: '2026-07-16T00:00:00',
  lastAutoVacuum: '2026-07-16T12:00:00',
  deadTuples: 500,
  liveTuples: 100000,
  bloatEstimateBytes: null,
};

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('getCapabilities', () => {
  it('invokes server_admin_get_capabilities with connectionId', async () => {
    mockInvoke.mockResolvedValue(stubCapabilities);
    await getCapabilities('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_get_capabilities', {
      connectionId: 'conn-1',
    });
  });

  it('returns the capability flags from invoke', async () => {
    mockInvoke.mockResolvedValue(stubCapabilities);
    const result = await getCapabilities('conn-1');
    expect(result).toBe(stubCapabilities);
  });
});

describe('listProcesses', () => {
  it('invokes server_admin_list_processes with connectionId', async () => {
    mockInvoke.mockResolvedValue([stubProcess]);
    await listProcesses('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_list_processes', {
      connectionId: 'conn-1',
    });
  });

  it('returns the process list from invoke', async () => {
    mockInvoke.mockResolvedValue([stubProcess]);
    const result = await listProcesses('conn-1');
    expect(result).toHaveLength(1);
    expect(result[0].id).toBe('42');
  });
});

describe('killSession', () => {
  it('invokes server_admin_kill_session with connectionId and sessionId', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await killSession('conn-1', '42');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_kill_session', {
      connectionId: 'conn-1',
      sessionId: '42',
    });
  });
});

describe('cancelSession', () => {
  it('invokes server_admin_cancel_session with connectionId and pid', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await cancelSession('conn-1', '1234');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_cancel_session', {
      connectionId: 'conn-1',
      pid: '1234',
    });
  });
});

describe('getStatus', () => {
  it('invokes server_admin_get_status with connectionId', async () => {
    mockInvoke.mockResolvedValue(stubStatus);
    await getStatus('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_get_status', { connectionId: 'conn-1' });
  });

  it('returns the server status from invoke', async () => {
    mockInvoke.mockResolvedValue(stubStatus);
    const result = await getStatus('conn-1');
    expect(result.version).toBe('8.0.33');
    expect(result.uptimeSeconds).toBe(86400);
  });
});

describe('listVariables', () => {
  it('invokes server_admin_list_variables with connectionId', async () => {
    mockInvoke.mockResolvedValue([stubVariable]);
    await listVariables('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_list_variables', {
      connectionId: 'conn-1',
    });
  });

  it('returns the variable list from invoke', async () => {
    mockInvoke.mockResolvedValue([stubVariable]);
    const result = await listVariables('conn-1');
    expect(result[0].name).toBe('max_connections');
  });
});

describe('setVariable', () => {
  it('invokes server_admin_set_variable with all four parameters', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setVariable('conn-1', 'max_connections', '200', 'global');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_set_variable', {
      connectionId: 'conn-1',
      name: 'max_connections',
      value: '200',
      scope: 'global',
    });
  });

  it('passes session scope correctly', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await setVariable('conn-2', 'sort_buffer_size', '1048576', 'session');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_set_variable', {
      connectionId: 'conn-2',
      name: 'sort_buffer_size',
      value: '1048576',
      scope: 'session',
    });
  });
});

describe('listLocks', () => {
  it('invokes server_admin_list_locks with connectionId', async () => {
    mockInvoke.mockResolvedValue([stubLock]);
    await listLocks('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_list_locks', { connectionId: 'conn-1' });
  });

  it('returns the lock list from invoke', async () => {
    mockInvoke.mockResolvedValue([stubLock]);
    const result = await listLocks('conn-1');
    expect(result[0].lockId).toBe('lock-1');
    expect(result[0].blockerSessionId).toBe('10');
  });
});

describe('listScheduledJobs', () => {
  it('invokes server_admin_list_scheduled_jobs with connectionId', async () => {
    mockInvoke.mockResolvedValue([stubJob]);
    await listScheduledJobs('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_list_scheduled_jobs', {
      connectionId: 'conn-1',
    });
  });

  it('returns the job list from invoke', async () => {
    mockInvoke.mockResolvedValue([stubJob]);
    const result = await listScheduledJobs('conn-1');
    expect(result[0].name).toBe('nightly_backup');
    expect(result[0].enabled).toBe(true);
  });
});

describe('getInnodbStatus', () => {
  it('invokes server_admin_get_innodb_status with connectionId', async () => {
    mockInvoke.mockResolvedValue('================\nINNODB STATUS\n================');
    await getInnodbStatus('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_get_innodb_status', {
      connectionId: 'conn-1',
    });
  });

  it('returns the raw InnoDB status string from invoke', async () => {
    const raw = 'BUFFER POOL AND MEMORY\nTotal large memory allocated 2198863872';
    mockInvoke.mockResolvedValue(raw);
    const result = await getInnodbStatus('conn-1');
    expect(result).toBe(raw);
  });
});

describe('getVacuumStatus', () => {
  it('invokes server_admin_get_vacuum_status with connectionId', async () => {
    mockInvoke.mockResolvedValue([stubVacuum]);
    await getVacuumStatus('conn-1');
    expect(mockInvoke).toHaveBeenCalledWith('server_admin_get_vacuum_status', {
      connectionId: 'conn-1',
    });
  });

  it('returns the vacuum info list from invoke', async () => {
    mockInvoke.mockResolvedValue([stubVacuum]);
    const result = await getVacuumStatus('conn-1');
    expect(result[0].table).toBe('users');
    expect(result[0].deadTuples).toBe(500);
  });
});
