import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { updaterCheck, updaterInstall } from './updater';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('updaterCheck', () => {
  it('invokes updater_check with no arguments', async () => {
    mockInvoke.mockResolvedValue({ available: false, version: null, notes: null });
    await updaterCheck();
    expect(mockInvoke).toHaveBeenCalledWith('updater_check');
  });

  it('returns UpdateCheckResult when no update is available', async () => {
    const stub = { available: false, version: null, notes: null };
    mockInvoke.mockResolvedValue(stub);
    const result = await updaterCheck();
    expect(result).toEqual(stub);
  });

  it('returns UpdateCheckResult with version when update is available', async () => {
    const stub = { available: true, version: '1.2.0', notes: 'Bug fixes' };
    mockInvoke.mockResolvedValue(stub);
    const result = await updaterCheck();
    expect(result.available).toBe(true);
    expect(result.version).toBe('1.2.0');
    expect(result.notes).toBe('Bug fixes');
  });

  it('propagates invoke errors', async () => {
    mockInvoke.mockRejectedValue({ code: 'UPDATER_ERROR', message: 'No endpoint', detail: null });
    await expect(updaterCheck()).rejects.toMatchObject({ code: 'UPDATER_ERROR' });
  });
});

describe('updaterInstall', () => {
  it('invokes updater_install with no arguments', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await updaterInstall();
    expect(mockInvoke).toHaveBeenCalledWith('updater_install');
  });

  it('propagates invoke errors', async () => {
    mockInvoke.mockRejectedValue({
      code: 'UPDATER_ERROR',
      message: 'Download failed',
      detail: null,
    });
    await expect(updaterInstall()).rejects.toMatchObject({ code: 'UPDATER_ERROR' });
  });
});
