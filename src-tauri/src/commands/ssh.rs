/// Tauri commands for managing SSH tunnels.
use serde::Serialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tauri::State;

use crate::connections::ssh_tunnel::SshTunnelManager;
use crate::db::models::ConnectionProfileRow;
use crate::error::AppError;

#[derive(Debug, Serialize)]
pub struct SshTunnelStatus {
    pub active: bool,
    #[serde(rename = "localPort")]
    pub local_port: Option<u16>,
}

fn retrieve_keychain_secret(connection_id: &str, secret_type: &str) -> Option<String> {
    let account = format!("{connection_id}:{secret_type}");
    crate::commands::keychain::read_keychain_secret("rowmance", &account)
}

/// Create an SSH tunnel for a connection profile.
/// Returns the local port that the database pool should connect to.
#[tauri::command]
pub async fn ssh_create_tunnel(
    sqlite: State<'_, SqlitePool>,
    tunnels: State<'_, Arc<SshTunnelManager>>,
    connection_id: String,
) -> Result<u16, AppError> {
    let row =
        sqlx::query_as::<_, ConnectionProfileRow>("SELECT * FROM connection_profiles WHERE id = ?")
            .bind(&connection_id)
            .fetch_optional(sqlite.inner())
            .await
            .map_err(|e| AppError::new("DB_ERROR", e.to_string()))?
            .ok_or_else(|| {
                AppError::new(
                    "CONNECTION_NOT_FOUND",
                    format!("No connection with id {connection_id}"),
                )
            })?;

    if !row.ssh_enabled {
        return Err(AppError::new(
            "SSH_NOT_ENABLED",
            "SSH is not enabled for this connection",
        ));
    }

    let ssh_host = row
        .ssh_host
        .ok_or_else(|| AppError::new("SSH_ERROR", "SSH host not set"))?;
    let ssh_port = row.ssh_port.unwrap_or(22) as u16;
    let ssh_user = row
        .ssh_user
        .ok_or_else(|| AppError::new("SSH_ERROR", "SSH user not set"))?;

    let auth_type = row.ssh_auth_type.as_deref().unwrap_or("password");
    let ssh_password = (auth_type == "password")
        .then(|| retrieve_keychain_secret(&connection_id, "ssh_password"))
        .flatten();
    let ssh_key_passphrase = (auth_type == "key")
        .then(|| retrieve_keychain_secret(&connection_id, "ssh_key_passphrase"))
        .flatten();

    let local_port = tunnels
        .create_tunnel(
            &connection_id,
            &ssh_host,
            ssh_port,
            &ssh_user,
            ssh_password,
            row.ssh_key_path,
            ssh_key_passphrase,
            &row.host,
            row.port as u16,
        )
        .await
        .map_err(AppError::from)?;

    Ok(local_port)
}

/// Destroy the SSH tunnel for a connection profile.
#[tauri::command]
pub async fn ssh_destroy_tunnel(
    tunnels: State<'_, Arc<SshTunnelManager>>,
    connection_id: String,
) -> Result<(), AppError> {
    tunnels.destroy_tunnel(&connection_id);
    Ok(())
}

/// Return the status of an SSH tunnel.
#[tauri::command]
pub async fn ssh_tunnel_status(
    tunnels: State<'_, Arc<SshTunnelManager>>,
    connection_id: String,
) -> Result<SshTunnelStatus, AppError> {
    let active = tunnels.is_active(&connection_id);
    let local_port = tunnels.local_port(&connection_id);
    Ok(SshTunnelStatus { active, local_port })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connections::ssh_tunnel::SshTunnelManager;

    #[test]
    fn ssh_tunnel_status_serializes() {
        let status = SshTunnelStatus {
            active: true,
            local_port: Some(12345),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"active\":true"));
        assert!(json.contains("\"localPort\":12345"));
    }

    #[test]
    fn ssh_tunnel_status_no_port() {
        let status = SshTunnelStatus {
            active: false,
            local_port: None,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"active\":false"));
        assert!(json.contains("\"localPort\":null"));
    }

    #[test]
    fn tunnel_status_returns_inactive_when_no_tunnel_exists() {
        let manager = SshTunnelManager::new();
        let active = manager.is_active("nonexistent-id");
        let local_port = manager.local_port("nonexistent-id");
        assert!(!active, "expected inactive for unknown tunnel id");
        assert!(
            local_port.is_none(),
            "expected no port for unknown tunnel id"
        );
    }

    #[test]
    fn destroy_tunnel_is_noop_when_tunnel_not_found() {
        // Should not panic when destroying a non-existent tunnel.
        let manager = SshTunnelManager::new();
        manager.destroy_tunnel("ghost-id"); // must not panic
        assert!(!manager.is_active("ghost-id"));
    }

    #[test]
    fn tunnel_status_serializes_correctly() {
        // Verify the full expected JSON shape of SshTunnelStatus.
        let active_status = SshTunnelStatus {
            active: true,
            local_port: Some(54321),
        };
        let json = serde_json::to_string(&active_status).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["active"], serde_json::json!(true));
        assert_eq!(v["localPort"], serde_json::json!(54321));

        let inactive_status = SshTunnelStatus {
            active: false,
            local_port: None,
        };
        let json2 = serde_json::to_string(&inactive_status).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
        assert_eq!(v2["active"], serde_json::json!(false));
        assert_eq!(v2["localPort"], serde_json::json!(null));
    }
}
