import type { AppError } from '$lib/types';

/** Extract a human-readable message from an unknown thrown value.
 *  Tauri invoke throws serialized AppError objects, not JS Error instances. */
export function errorMessage(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'object' && err !== null && 'message' in err) {
    return String((err as AppError).message);
  }
  return String(err);
}

/** True when an error looks like the backend's "Connection X is not connected" error. */
export function isNotConnectedError(err: unknown): boolean {
  return /is not connected/i.test(errorMessage(err));
}

/** True when a message indicates the Oracle Instant Client isn't installed locally
 *  (DPI-1047, raised by ODPI-C when it can't dlopen libclntsh). */
export function isOracleClientMissingError(message: string): boolean {
  return /DPI-1047/.test(message);
}
