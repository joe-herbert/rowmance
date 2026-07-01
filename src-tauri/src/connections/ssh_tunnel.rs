/// SSH tunnel manager using the system `ssh` binary.
///
/// Each tunnel spawns `ssh -L local_port:remote_host:remote_port -N` and waits
/// until the local port accepts connections before returning. Tunnels are stored
/// in a DashMap keyed by connection profile ID so that connections_connect can
/// look up the local port to substitute into the database URL.
use dashmap::DashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

use crate::error::RowmanceError;

pub struct TunnelHandle {
    pub local_port: u16,
    process: tokio::process::Child,
}

impl std::fmt::Debug for TunnelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TunnelHandle")
            .field("local_port", &self.local_port)
            .finish()
    }
}

#[derive(Debug, Default)]
pub struct SshTunnelManager {
    tunnels: DashMap<String, TunnelHandle>,
}

impl SshTunnelManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            tunnels: DashMap::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_tunnel(
        &self,
        connection_id: &str,
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        ssh_password: Option<String>,
        ssh_key_path: Option<String>,
        _ssh_key_passphrase: Option<String>,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<u16, RowmanceError> {
        if let Some(port) = self.local_port(connection_id) {
            return Ok(port);
        }

        // Reserve a free port then release it for ssh to bind.
        let local_port = {
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(RowmanceError::Io)?;
            let port = listener.local_addr().map_err(RowmanceError::Io)?.port();
            drop(listener);
            port
        };

        let local_fwd = format!("{local_port}:{remote_host}:{remote_port}");

        let mut cmd = if let Some(ref password) = ssh_password {
            let mut c = tokio::process::Command::new("sshpass");
            c.arg("-p").arg(password).arg("ssh");
            c
        } else {
            tokio::process::Command::new("ssh")
        };

        cmd.arg("-L")
            .arg(&local_fwd)
            .arg("-N")
            .arg("-o")
            .arg("StrictHostKeyChecking=accept-new")
            .arg("-o")
            .arg("ExitOnForwardFailure=yes")
            .arg("-o")
            .arg("ServerAliveInterval=30");

        if ssh_password.is_none() {
            cmd.arg("-o").arg("BatchMode=yes");
        }

        if let Some(ref key_path) = ssh_key_path {
            cmd.arg("-i").arg(key_path);
        }

        cmd.arg("-p")
            .arg(ssh_port.to_string())
            .arg(format!("{ssh_user}@{ssh_host}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().map_err(RowmanceError::Io)?;

        // Poll until ssh binds the local port (up to 15 seconds).
        let ready = tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                if tokio::net::TcpStream::connect(format!("127.0.0.1:{local_port}"))
                    .await
                    .is_ok()
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;

        if ready.is_err() {
            return Err(RowmanceError::Ssh(format!(
                "SSH tunnel to {ssh_host} did not become ready within 15 seconds"
            )));
        }

        self.tunnels.insert(
            connection_id.to_owned(),
            TunnelHandle {
                local_port,
                process: child,
            },
        );
        Ok(local_port)
    }

    pub fn destroy_tunnel(&self, connection_id: &str) {
        if let Some((_, mut handle)) = self.tunnels.remove(connection_id) {
            let _ = handle.process.start_kill();
        }
    }

    pub fn local_port(&self, connection_id: &str) -> Option<u16> {
        self.tunnels.get(connection_id).map(|h| h.local_port)
    }

    pub fn is_active(&self, connection_id: &str) -> bool {
        self.tunnels.contains_key(connection_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_has_no_tunnels() {
        let manager = SshTunnelManager::new();
        assert!(!manager.is_active("conn-1"));
        assert!(manager.local_port("conn-1").is_none());
    }

    #[test]
    fn destroy_nonexistent_tunnel_is_noop() {
        let manager = SshTunnelManager::new();
        manager.destroy_tunnel("ghost");
        assert!(!manager.is_active("ghost"));
    }
}
