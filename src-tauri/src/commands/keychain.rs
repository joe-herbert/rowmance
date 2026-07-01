/// Tauri commands for storing and retrieving secrets via the OS keychain.
/// Entry names follow the pattern "{connection_id}:{secret_type}" where
/// secret_type is one of: db_password, ssh_password, ssh_key_passphrase.
///
/// On macOS all new items are written to the Data Protection Keychain
/// (kSecUseDataProtectionKeychain = true).  Items there never show ACL
/// confirmation dialogs — access is granted automatically while the device
/// is unlocked.  Items that were previously stored in the legacy keychain
/// by the old `keyring` crate are migrated on first read (delete old item,
/// insert into DPK) so that future reads are silent too.
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
    use security_framework_sys::base::errSecDuplicateItem;
    use security_framework_sys::item::{
        kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecReturnData,
        kSecUseDataProtectionKeychain, kSecValueData,
    };
    use security_framework_sys::keychain_item::{
        SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate,
    };

    const ERR_SEC_NOT_FOUND: i32 = -25300; // errSecItemNotFound

    /// Build the base query attributes for a generic password in the Data
    /// Protection Keychain.  The DPK flag causes macOS to bypass ACL-based
    /// confirmation dialogs entirely.
    fn dpk_query(service: &str, account: &str) -> Vec<(CFString, CFType)> {
        unsafe {
            vec![
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
            ]
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
    pub fn read_dpk(service: &str, account: &str) -> Option<Vec<u8>> {
        let mut query = dpk_query(service, account);
        query.push((
            unsafe { CFString::wrap_under_get_rule(kSecReturnData) },
            CFBoolean::from(true).into_CFType(),
        ));
        let params = CFDictionary::from_CFType_pairs(&query);
        let mut ret: CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(params.as_concrete_TypeRef(), &mut ret) };
        if status != 0 {
            return None;
        }
        extract_bytes(ret)
    }

    /// Write to the Data Protection Keychain — never prompts.
    /// Returns the OSStatus so callers can detect failure and fall back.
    pub fn write_dpk(service: &str, account: &str, value: &[u8]) -> i32 {
        let mut query = dpk_query(service, account);
        let find_len = query.len();
        query.push((
            unsafe { CFString::wrap_under_get_rule(kSecValueData) },
            CFData::from_buffer(value).into_CFType(),
        ));

        let params = CFDictionary::from_CFType_pairs(&query);
        let mut ret = std::ptr::null();
        let status = unsafe { SecItemAdd(params.as_concrete_TypeRef(), &mut ret) };
        if status == errSecDuplicateItem {
            let find = CFDictionary::from_CFType_pairs(&query[..find_len]);
            let attrs = CFDictionary::from_CFType_pairs(&query[find_len..]);
            return unsafe {
                SecItemUpdate(find.as_concrete_TypeRef(), attrs.as_concrete_TypeRef())
            };
        }
        status
    }

    /// Delete from the Data Protection Keychain.
    pub fn remove_dpk(service: &str, account: &str) {
        let query = dpk_query(service, account);
        let params = CFDictionary::from_CFType_pairs(&query);
        unsafe { SecItemDelete(params.as_concrete_TypeRef()) };
    }

    /// Read from the legacy keychain (may show ACL prompt for old items).
    /// Used only as a one-time fallback during migration.
    pub fn read_legacy(service: &str, account: &str) -> Option<Vec<u8>> {
        security_framework::passwords::get_generic_password(service, account).ok()
    }

    /// Delete from the legacy keychain — typically silent for the owning app.
    pub fn remove_legacy(service: &str, account: &str) {
        let _ = security_framework::passwords::delete_generic_password(service, account);
    }
}

// ── Public helpers ────────────────────────────────────────────────────────────

/// Read a secret from the keychain.
///
/// On macOS: checks the Data Protection Keychain first (no prompt).  If not
/// found there, falls back to the legacy keychain (may prompt once for old
/// items created by a previous version) and immediately migrates the item into
/// the DPK so all future reads are silent.
#[cfg(target_os = "macos")]
pub fn read_keychain_secret(service: &str, account: &str) -> Option<String> {
    // Fast path: item already in the Data Protection Keychain — no prompt.
    if let Some(bytes) = macos::read_dpk(service, account) {
        return String::from_utf8(bytes).ok();
    }

    // Slow path: item is in the legacy keychain (stored by old keyring crate).
    // This may show an ACL prompt, but only this one time.
    let bytes = macos::read_legacy(service, account)?;
    let value = String::from_utf8(bytes.clone()).ok()?;

    // Attempt to migrate into the DPK so future reads are silent.
    // Succeeds in production (signed) builds; silently skips in unsigned dev builds.
    if macos::write_dpk(service, account, &bytes) == 0 {
        macos::remove_legacy(service, account);
    }

    Some(value)
}

#[cfg(not(target_os = "macos"))]
pub fn read_keychain_secret(service: &str, account: &str) -> Option<String> {
    Entry::new(service, account)
        .ok()
        .and_then(|e| e.get_password().ok())
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
    Ok(read_keychain_secret(
        "rowmance",
        &format!("{connection_id}:{secret_type}"),
    ))
}

/// Delete a secret from the OS keychain. No-ops if not present.
#[tauri::command]
pub async fn keychain_delete(connection_id: String, secret_type: String) -> Result<(), AppError> {
    let account = format!("{connection_id}:{secret_type}");
    keychain_remove("rowmance", &account)
}

#[cfg(target_os = "macos")]
fn keychain_write(service: &str, account: &str, value: &[u8]) -> Result<(), AppError> {
    let dpk_status = macos::write_dpk(service, account, value);
    if dpk_status == 0 {
        // Successfully written to the Data Protection Keychain.
        // Remove any legacy item so it can't generate a second ACL prompt.
        macos::remove_legacy(service, account);
        return Ok(());
    }

    // DPK write failed (errSecMissingEntitlement=-34018 in unsigned dev builds).
    // Fall back to the legacy keychain via security_framework.
    // To avoid the double-prompt caused by items originally created with the old
    // SecKeychain API, delete any existing item first so set_generic_password
    // always calls SecItemAdd (clean new-API item) rather than SecItemUpdate.
    eprintln!("[keychain] DPK write failed (status {dpk_status}), falling back to legacy keychain");
    security_framework::passwords::delete_generic_password(service, account).ok();
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
                // Only migrate if not already in the keychain.
                if read_keychain_secret("rowmance", &account).is_none() {
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
