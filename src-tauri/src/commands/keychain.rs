/// Tauri commands for storing and retrieving secrets via the OS keychain.
/// Entry names follow the pattern "{connection_id}:{secret_type}" where
/// secret_type is one of: db_password, ssh_password, ssh_key_passphrase.
///
/// On macOS, secrets are written to the legacy (non-DPK) generic-password
/// keychain. The Data Protection Keychain (`kSecUseDataProtectionKeychain`)
/// was tried first, since it's documented to skip ACL confirmation dialogs
/// entirely, but proved unreliable in testing on this app's Team ID/entitlement
/// combination: writes could report success and read back successfully
/// immediately after, yet the item would be gone shortly after — even within
/// the same process. It's still checked on read as a harmless courtesy in case
/// an item ever does land there durably, but nothing is ever written to it.
///
/// With the app's Developer ID signature stable across builds, the legacy
/// keychain's own default ACL (which trusts the app that created an item,
/// identified by code signature) means reads from this same app are silent —
/// no custom access-control-list code needed.
#[cfg(not(target_os = "macos"))]
use keyring::Entry;
use sqlx::SqlitePool;

use crate::error::AppError;

// ── macOS implementation ──────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod macos {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::boolean::CFBoolean;
    use core_foundation::data::CFData;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef};
    use core_foundation_sys::data::CFDataRef;
    use security_framework_sys::base::errSecItemNotFound;
    use security_framework_sys::item::{
        kSecAttrAccessGroup, kSecAttrAccount, kSecAttrService, kSecClass,
        kSecClassGenericPassword, kSecReturnData, kSecUseDataProtectionKeychain,
    };
    use security_framework_sys::keychain_item::{SecItemCopyMatching, SecItemDelete};

    /// The app's `keychain-access-groups` entitlement value, baked in at build time
    /// from the `KEYCHAIN_ACCESS_GROUP` env var (see Makefile / release.yml). `None`
    /// in unsigned dev builds, where there's no custom access group to pin to.
    ///
    /// Without pinning this explicitly, `SecItemAdd` and `SecItemCopyMatching` can
    /// each fall back to a different implicit default access group when the app has
    /// exactly one custom (non-default) group entitled, causing a write to report
    /// success while an immediately-following read of the same item misses.
    const ACCESS_GROUP: Option<&str> = option_env!("KEYCHAIN_ACCESS_GROUP");

    /// Build the base query attributes for a generic password in the Data
    /// Protection Keychain.  The DPK flag causes macOS to bypass ACL-based
    /// confirmation dialogs entirely.
    fn dpk_query(service: &str, account: &str) -> Vec<(CFString, CFType)> {
        unsafe {
            let mut query = vec![
                (
                    CFString::wrap_under_get_rule(kSecClass),
                    CFString::wrap_under_get_rule(kSecClassGenericPassword).into_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrService),
                    CFString::from(service).into_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrAccount),
                    CFString::from(account).into_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecUseDataProtectionKeychain),
                    CFBoolean::from(true).into_CFType(),
                ),
            ];
            if let Some(group) = ACCESS_GROUP {
                query.push((
                    CFString::wrap_under_get_rule(kSecAttrAccessGroup),
                    CFString::from(group).into_CFType(),
                ));
            }
            query
        }
    }

    fn extract_bytes(ret: CFTypeRef) -> Option<Vec<u8>> {
        if ret.is_null() {
            return None;
        }
        let type_id = unsafe { CFGetTypeID(ret) };
        if type_id == CFData::type_id() {
            let data = unsafe { CFData::wrap_under_create_rule(ret as CFDataRef) };
            return Some(data.bytes().to_vec());
        }
        unsafe { CFRelease(ret) };
        None
    }

    /// Read from the Data Protection Keychain — never prompts.
    ///
    /// Returns `Ok(None)` when the item genuinely doesn't exist, and `Err(status)`
    /// for any other failure (e.g. `errSecMissingEntitlement` when the app's
    /// `keychain-access-groups` entitlement isn't valid) so callers can tell
    /// "no secret was ever stored" apart from "the keychain refused access".
    pub fn read_dpk(service: &str, account: &str) -> Result<Option<Vec<u8>>, i32> {
        let mut query = dpk_query(service, account);
        query.push((
            unsafe { CFString::wrap_under_get_rule(kSecReturnData) },
            CFBoolean::from(true).into_CFType(),
        ));
        let params = CFDictionary::from_CFType_pairs(&query);
        let mut ret: CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(params.as_concrete_TypeRef(), &mut ret) };
        if status == errSecItemNotFound {
            return Ok(None);
        }
        if status != 0 {
            return Err(status);
        }
        Ok(extract_bytes(ret))
    }

    /// Delete from the Data Protection Keychain.
    pub fn remove_dpk(service: &str, account: &str) {
        let query = dpk_query(service, account);
        let params = CFDictionary::from_CFType_pairs(&query);
        unsafe { SecItemDelete(params.as_concrete_TypeRef()) };
    }

    /// Read from the legacy keychain (may show ACL prompt for old items).
    /// Used only as a one-time fallback during migration.
    ///
    /// Same `Ok(None)` vs `Err(status)` distinction as [`read_dpk`].
    pub fn read_legacy(service: &str, account: &str) -> Result<Option<Vec<u8>>, i32> {
        match security_framework::passwords::get_generic_password(service, account) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.code() == errSecItemNotFound => Ok(None),
            Err(e) => Err(e.code()),
        }
    }

    /// Delete from the legacy keychain — typically silent for the owning app.
    pub fn remove_legacy(service: &str, account: &str) {
        let _ = security_framework::passwords::delete_generic_password(service, account);
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Reads a secret from the keychain.
///
/// The Data Protection Keychain (`kSecUseDataProtectionKeychain`) has proven
/// unreliable on this app's Team ID/entitlement combination in testing: a
/// write can report success and be immediately read back successfully, yet
/// the item is gone moments later — even within the same process, with no
/// error at any step. It is checked here only as a courtesy in case an item
/// really did land there durably, but it is never written to (see
/// `keychain_write`), and a hit here is not treated as authoritative over a
/// miss — everything durable lives in the legacy keychain.
///
/// `Ok(None)` means no secret was ever stored under this account. `Err(_)`
/// means the keychain itself refused access outright.
#[cfg(target_os = "macos")]
pub fn read_keychain_secret(service: &str, account: &str) -> Result<Option<String>, String> {
    if let Ok(Some(bytes)) = macos::read_dpk(service, account) {
        return Ok(String::from_utf8(bytes).ok());
    }

    match macos::read_legacy(service, account) {
        Ok(Some(bytes)) => Ok(String::from_utf8(bytes).ok()),
        Ok(None) => Ok(None),
        Err(status) => Err(format!(
            "keychain access denied (status {status})"
        )),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn read_keychain_secret(service: &str, account: &str) -> Result<Option<String>, String> {
    let entry = Entry::new(service, account).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Store a secret in the OS keychain.
#[tauri::command]
pub async fn keychain_store(
    connection_id: String,
    secret_type: String,
    value: String,
) -> Result<(), AppError> {
    let account = format!("{connection_id}:{secret_type}");
    keychain_write("rowmance", &account, value.as_bytes())
}

/// Retrieve a secret from the OS keychain; returns None if not present.
#[tauri::command]
pub async fn keychain_retrieve(
    connection_id: String,
    secret_type: String,
) -> Result<Option<String>, AppError> {
    read_keychain_secret("rowmance", &format!("{connection_id}:{secret_type}"))
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e))
}

/// Delete a secret from the OS keychain. No-ops if not present.
#[tauri::command]
pub async fn keychain_delete(connection_id: String, secret_type: String) -> Result<(), AppError> {
    let account = format!("{connection_id}:{secret_type}");
    keychain_remove("rowmance", &account)
}

/// Write a secret by service and account — used by other command modules.
pub fn keychain_write_secret(service: &str, account: &str, value: &str) -> Result<(), AppError> {
    keychain_write(service, account, value.as_bytes())
}

/// Writes a secret to the legacy (non-DPK) generic-password keychain — the
/// only storage observed to be reliable in testing (see `read_keychain_secret`).
/// Delete-then-add ensures a clean `SecItemAdd`-created item rather than an
/// update on top of a pre-existing item from the old `keyring` crate, which
/// avoids a second/duplicate ACL prompt.
#[cfg(target_os = "macos")]
fn keychain_write(service: &str, account: &str, value: &[u8]) -> Result<(), AppError> {
    macos::remove_legacy(service, account);
    security_framework::passwords::set_generic_password(service, account, value)
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))
}

#[cfg(not(target_os = "macos"))]
fn keychain_write(service: &str, account: &str, value: &[u8]) -> Result<(), AppError> {
    let s = String::from_utf8(value.to_vec())
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))?;
    Entry::new(service, account)
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))?
        .set_password(&s)
        .map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))
}

#[cfg(target_os = "macos")]
fn keychain_remove(service: &str, account: &str) -> Result<(), AppError> {
    // Remove from both locations — we don't know which one the item is in.
    macos::remove_dpk(service, account);
    macos::remove_legacy(service, account);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn keychain_remove(service: &str, account: &str) -> Result<(), AppError> {
    let e =
        Entry::new(service, account).map_err(|e| AppError::new("KEYCHAIN_ERROR", e.to_string()))?;
    match e.delete_credential() {
        Ok(_) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(AppError::new("KEYCHAIN_ERROR", err.to_string())),
    }
}

/// Migrate any existing plaintext passwords from SQLite to the keychain.
/// Called once on startup; safe to call multiple times.
pub async fn migrate_passwords_to_keychain(sqlite: &SqlitePool) {
    #[derive(sqlx::FromRow)]
    struct PasswordRow {
        id: String,
        password: Option<String>,
    }

    let rows: Result<Vec<PasswordRow>, _> = sqlx::query_as::<_, PasswordRow>(
        "SELECT id, password FROM connection_profiles WHERE password IS NOT NULL AND password != ''",
    )
    .fetch_all(sqlite)
    .await;

    if let Ok(rows) = rows {
        for row in rows {
            if let Some(pw) = row.password {
                if pw.is_empty() {
                    continue;
                }
                let account = format!("{}:db_password", row.id);
                // Only migrate if not already in the keychain. Treat a read
                // error the same as "not present" here — this is a one-time,
                // best-effort migration, not the security-sensitive read path.
                if !matches!(
                    read_keychain_secret("rowmance", &account),
                    Ok(Some(_))
                ) {
                    let _ = keychain_write("rowmance", &account, pw.as_bytes());
                }
                // Null out the stored password.
                let _ = sqlx::query("UPDATE connection_profiles SET password = NULL WHERE id = ?")
                    .bind(&row.id)
                    .execute(sqlite)
                    .await;
            }
        }
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn entry_name_roundtrip() {
        // On non-macOS we still use keyring; just verify Entry::new doesn't panic.
        let _ = Entry::new("rowmance", "conn-123:db_password");
    }
}
