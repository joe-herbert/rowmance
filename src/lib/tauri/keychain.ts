/** Typed wrappers around the Tauri `keychain::*` commands. */
import { invoke } from '@tauri-apps/api/core';

export type KeychainSecretType = 'db_password' | 'ssh_password' | 'ssh_key_passphrase';

export async function keychainStore(
  connectionId: string,
  secretType: KeychainSecretType,
  value: string,
): Promise<void> {
  return invoke<void>('keychain_store', { connectionId, secretType, value });
}

export async function keychainRetrieve(
  connectionId: string,
  secretType: KeychainSecretType,
): Promise<string | null> {
  return invoke<string | null>('keychain_retrieve', { connectionId, secretType });
}

export async function keychainDelete(
  connectionId: string,
  secretType: KeychainSecretType,
): Promise<void> {
  return invoke<void>('keychain_delete', { connectionId, secretType });
}
