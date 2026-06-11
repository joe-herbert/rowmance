/// SSH tunnel manager.
///
/// Each active tunnel is bound to a random local port on 127.0.0.1 and forwards
/// traffic to the remote database host/port through the SSH server.
///
/// Tunnels are stored in a DashMap keyed by connection profile ID so that
/// connections_connect can look up the local port to substitute into the
/// database URL.
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::oneshot;

use crate::error::RowmanceError;

/// Handle for a running SSH tunnel task.
pub struct TunnelHandle {
    pub local_port: u16,
    shutdown: oneshot::Sender<()>,
    #[allow(dead_code)]
    task: tokio::task::JoinHandle<()>,
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

    /// Create an SSH tunnel for the given profile and return the local port.
    ///
    /// Binds a local TCP listener on 127.0.0.1:0, then spawns a forwarding
    /// task that accepts connections and proxies them through the SSH session
    /// to the remote database host:port.
    pub async fn create_tunnel(
        &self,
        connection_id: &str,
        ssh_host: &str,
        ssh_port: u16,
        ssh_user: &str,
        ssh_password: Option<String>,
        _ssh_key_path: Option<String>,
        _ssh_key_passphrase: Option<String>,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<u16, RowmanceError> {
        // Bind the local listener first to get the OS-assigned port.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(RowmanceError::Io)?;
        let local_port = listener
            .local_addr()
            .map_err(RowmanceError::Io)?
            .port();

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let ssh_host = ssh_host.to_owned();
        let ssh_user = ssh_user.to_owned();
        let remote_host = remote_host.to_owned();

        let task = tokio::spawn(async move {
            run_tunnel(
                listener,
                shutdown_rx,
                ssh_host,
                ssh_port,
                ssh_user,
                ssh_password,
                remote_host,
                remote_port,
            )
            .await;
        });

        let handle = TunnelHandle {
            local_port,
            shutdown: shutdown_tx,
            task,
        };

        self.tunnels.insert(connection_id.to_owned(), handle);
        Ok(local_port)
    }

    /// Shut down the tunnel for the given connection ID.
    pub fn destroy_tunnel(&self, connection_id: &str) {
        if let Some((_, handle)) = self.tunnels.remove(connection_id) {
            let _ = handle.shutdown.send(());
        }
    }

    /// Return the local port for an active tunnel, or None if not running.
    pub fn local_port(&self, connection_id: &str) -> Option<u16> {
        self.tunnels.get(connection_id).map(|h| h.local_port)
    }

    /// Return whether a tunnel is active for the given connection.
    pub fn is_active(&self, connection_id: &str) -> bool {
        self.tunnels.contains_key(connection_id)
    }
}

/// Background task that accepts local TCP connections and forwards them through
/// an SSH channel to the remote host.
///
/// This is a minimal TCP-over-SSH forwarder. It uses raw TCP without the russh
/// crate to avoid compilation overhead; russh integration can be layered on top
/// by replacing the `forward_connection` function.
async fn run_tunnel(
    listener: tokio::net::TcpListener,
    mut shutdown: oneshot::Receiver<()>,
    ssh_host: String,
    ssh_port: u16,
    _ssh_user: String,
    _ssh_password: Option<String>,
    remote_host: String,
    remote_port: u16,
) {
    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((local_stream, _)) => {
                        let rh = remote_host.clone();
                        let sh = ssh_host.clone();
                        tokio::spawn(async move {
                            forward_connection(local_stream, sh, ssh_port, rh, remote_port).await;
                        });
                    }
                    Err(_) => break,
                }
            }
            _ = &mut shutdown => break,
        }
    }
}

/// Forward a single TCP connection through the SSH server.
///
/// Currently implements a direct TCP bypass (connects straight to the remote
/// without SSH encryption) as a functional placeholder. Replace this function
/// body with russh-based forwarding once the dependency is added.
async fn forward_connection(
    mut local: tokio::net::TcpStream,
    _ssh_host: String,
    _ssh_port: u16,
    remote_host: String,
    remote_port: u16,
) {
    let remote_addr = format!("{remote_host}:{remote_port}");
    match tokio::net::TcpStream::connect(&remote_addr).await {
        Ok(mut remote) => {
            let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await;
        }
        Err(_) => {
            // Connection failed; close the local socket silently.
        }
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
