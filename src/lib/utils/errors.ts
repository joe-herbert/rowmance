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
