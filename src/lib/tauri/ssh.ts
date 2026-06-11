/** Typed wrappers around the Tauri `ssh::*` commands. */
import { invoke } from '@tauri-apps/api/core';

export interface SshTunnelStatus {
  active: boolean;
  localPort: number | null;
}

export async function sshCreateTunnel(connectionId: string): Promise<number> {
  return invoke<number>('ssh_create_tunnel', { connectionId });
}

export async function sshDestroyTunnel(connectionId: string): Promise<void> {
  return invoke<void>('ssh_destroy_tunnel', { connectionId });
}

export async function sshTunnelStatus(connectionId: string): Promise<SshTunnelStatus> {
  return invoke<SshTunnelStatus>('ssh_tunnel_status', { connectionId });
}
