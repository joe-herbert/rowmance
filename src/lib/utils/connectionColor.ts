import type { ConnectionProfile } from '$lib/types';

export function connectionColor(profile: ConnectionProfile | null | undefined): string {
  return profile?.color ?? 'var(--color-accent)';
}
