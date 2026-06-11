/** Tests for keychain.ts typed wrappers. */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { keychainStore, keychainRetrieve, keychainDelete } from './keychain';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
});

describe('keychainStore', () => {
  it('calls keychain_store with correct args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await keychainStore('conn-1', 'db_password', 'secret');
    expect(mockInvoke).toHaveBeenCalledWith('keychain_store', {
      connectionId: 'conn-1',
      secretType: 'db_password',
      value: 'secret',
    });
  });
});

describe('keychainRetrieve', () => {
  it('returns the password string from the backend', async () => {
    mockInvoke.mockResolvedValue('my-secret');
    const result = await keychainRetrieve('conn-1', 'db_password');
    expect(result).toBe('my-secret');
  });

  it('returns null when no entry exists', async () => {
    mockInvoke.mockResolvedValue(null);
    const result = await keychainRetrieve('conn-1', 'db_password');
    expect(result).toBeNull();
  });
});

describe('keychainDelete', () => {
  it('calls keychain_delete with correct args', async () => {
    mockInvoke.mockResolvedValue(undefined);
    await keychainDelete('conn-1', 'ssh_password');
    expect(mockInvoke).toHaveBeenCalledWith('keychain_delete', {
      connectionId: 'conn-1',
      secretType: 'ssh_password',
    });
  });
});
