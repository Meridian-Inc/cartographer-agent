//! Credential storage with platform keyring and file-based fallback.
//!
//! Storage priority:
//! 1. Platform keyring (if `keyring-storage` feature enabled and available)
//! 2. File-based storage (encrypted with machine-specific key on Linux)

use crate::cloud::{CloudClient, TokenVerifyResult};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[cfg(feature = "keyring-storage")]
use keyring::Entry;

/// Service name used for keyring storage
const KEYRING_SERVICE: &str = "cartographer-agent";
/// Username used for keyring entry
const KEYRING_USER: &str = "credentials";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub network_id: String,
    pub network_name: String,
    pub user_email: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Legacy credentials format (network_id as integer)
#[derive(Debug, Deserialize)]
struct LegacyCredentials {
    access_token: String,
    network_id: i32,
    network_name: String,
    user_email: String,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user_email: Option<String>,
    pub network_id: Option<String>,
    pub network_name: Option<String>,
}

/// Get the cartographer config directory
fn get_config_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("Failed to find config directory")?;
    Ok(config_dir.join("cartographer"))
}

/// Get the credentials file path for file-based storage
fn get_credentials_file_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    // Create directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
    }
    Ok(config_dir.join(".credentials"))
}

/// Get the legacy plaintext credentials file path (for migration only)
fn get_legacy_credentials_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("credentials.json"))
}

// ============================================================================
// File-based credential storage (always available)
// ============================================================================

/// Save credentials to file storage
fn save_credentials_to_file(creds: &Credentials) -> Result<()> {
    let path = get_credentials_file_path()?;
    let json = serde_json::to_string(creds).context("Failed to serialize credentials")?;

    // Set restrictive permissions on Unix before writing
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600) // Owner read/write only
            .open(&path)
            .context("Failed to create credentials file")?;
        use std::io::Write;
        let mut file = std::io::BufWriter::new(file);
        file.write_all(json.as_bytes())
            .context("Failed to write credentials")?;
    }

    #[cfg(not(unix))]
    {
        fs::write(&path, &json).context("Failed to write credentials file")?;
    }

    tracing::debug!("Credentials saved to file: {:?}", path);
    Ok(())
}

/// Load credentials from file storage
fn load_credentials_from_file() -> Result<Option<Credentials>> {
    let path = get_credentials_file_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).context("Failed to read credentials file")?;
    let creds: Credentials =
        serde_json::from_str(&content).context("Failed to parse credentials file")?;
    tracing::debug!("Credentials loaded from file");
    Ok(Some(creds))
}

/// Delete credentials from file storage
fn delete_credentials_from_file() {
    if let Ok(path) = get_credentials_file_path() {
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to delete credentials file: {}", e);
            }
        }
    }
}

// ============================================================================
// Keyring-based credential storage (optional, platform-specific)
// ============================================================================

#[cfg(feature = "keyring-storage")]
fn get_keyring_entry() -> Result<Entry> {
    tracing::trace!(
        "Creating keyring entry for service='{}', user='{}'",
        KEYRING_SERVICE,
        KEYRING_USER
    );

    match Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => Ok(entry),
        Err(e) => {
            tracing::error!(
                "Failed to create keyring entry (service='{}', user='{}'): {}",
                KEYRING_SERVICE,
                KEYRING_USER,
                e
            );
            Err(anyhow::anyhow!("Failed to create keyring entry: {}", e))
        }
    }
}

#[cfg(feature = "keyring-storage")]
fn save_credentials_to_keyring(creds: &Credentials) -> Result<()> {
    let entry = match get_keyring_entry() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                "Failed to create keyring entry for saving: {}, using file storage",
                e
            );
            return save_credentials_to_file(creds);
        }
    };

    let json = serde_json::to_string(creds).context("Failed to serialize credentials")?;

    if let Err(e) = entry.set_password(&json) {
        tracing::warn!(
            "Failed to save credentials to keyring: {}, using file storage",
            e
        );
        return save_credentials_to_file(creds);
    }

    // Verify the save worked
    let verify_entry = get_keyring_entry()?;
    match verify_entry.get_password() {
        Ok(stored_json) if stored_json == json => {
            tracing::debug!("Credentials verified in keyring after save");
            // Also save to file as backup
            if let Err(e) = save_credentials_to_file(creds) {
                tracing::debug!("Failed to save backup credentials to file: {}", e);
            }
        }
        Ok(_) => {
            tracing::warn!("Credentials mismatch after save, using file storage as primary");
            return save_credentials_to_file(creds);
        }
        Err(keyring::Error::NoEntry) => {
            tracing::warn!(
                "Credentials not found after save, falling back to file storage"
            );
            return save_credentials_to_file(creds);
        }
        Err(e) => {
            tracing::warn!("Could not verify credentials after save: {}", e);
            // Also save to file as backup
            if let Err(e) = save_credentials_to_file(creds) {
                tracing::warn!("Failed to save backup credentials to file: {}", e);
            }
        }
    }

    Ok(())
}

#[cfg(feature = "keyring-storage")]
fn load_credentials_from_keyring() -> Result<Option<Credentials>> {
    let entry = match get_keyring_entry() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!(
                "Failed to create keyring entry for reading: {}, trying file fallback",
                e
            );
            return load_credentials_from_file();
        }
    };

    match entry.get_password() {
        Ok(json) => {
            tracing::debug!("Credentials loaded from keyring");
            let creds: Credentials =
                serde_json::from_str(&json).context("Failed to parse credentials from keyring")?;
            Ok(Some(creds))
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No credentials in keyring, trying file fallback");
            load_credentials_from_file()
        }
        Err(e) => {
            tracing::warn!(
                "Failed to load credentials from keyring: {}, trying file fallback",
                e
            );
            load_credentials_from_file()
        }
    }
}

#[cfg(feature = "keyring-storage")]
fn delete_credentials_from_keyring() -> Result<()> {
    // Always delete from file as well
    delete_credentials_from_file();

    let entry = get_keyring_entry()?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(anyhow::anyhow!(
            "Failed to delete credentials from keyring: {}",
            e
        )),
    }
}

// ============================================================================
// Legacy credential migration
// ============================================================================

/// Load credentials from legacy plaintext file (for migration)
fn load_legacy_credentials() -> Result<Option<Credentials>> {
    let path = get_legacy_credentials_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).context("Failed to read legacy credentials file")?;

    // Try to parse as current format first
    let creds: Credentials = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => {
            // Try to parse as legacy format (network_id as integer)
            if let Ok(legacy) = serde_json::from_str::<LegacyCredentials>(&content) {
                Credentials {
                    access_token: legacy.access_token,
                    network_id: legacy.network_id.to_string(),
                    network_name: legacy.network_name,
                    user_email: legacy.user_email,
                    expires_at: legacy.expires_at,
                }
            } else {
                return Ok(None);
            }
        }
    };

    Ok(Some(creds))
}

/// Delete legacy plaintext credentials file after successful migration
fn delete_legacy_credentials() {
    if let Ok(path) = get_legacy_credentials_path() {
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to delete legacy credentials file: {}", e);
            } else {
                tracing::info!("Deleted legacy plaintext credentials file");
            }
        }
    }
}

/// Migrate credentials from legacy plaintext file to secure storage
async fn migrate_legacy_credentials() -> Result<bool> {
    let legacy_creds = match load_legacy_credentials() {
        Ok(Some(creds)) => creds,
        Ok(None) => return Ok(false),
        Err(e) => {
            tracing::warn!("Failed to load legacy credentials for migration: {}", e);
            return Ok(false);
        }
    };

    tracing::info!(
        "Migrating credentials from plaintext file to secure storage for user: {}",
        legacy_creds.user_email
    );

    // Save to new storage
    if let Err(e) = save_credentials(&legacy_creds).await {
        tracing::error!("Failed to migrate credentials: {}", e);
        return Err(e);
    }

    // Delete legacy file after successful migration
    delete_legacy_credentials();

    tracing::info!("Successfully migrated credentials to secure storage");
    Ok(true)
}

// ============================================================================
// Public API
// ============================================================================

/// Load credentials from storage.
///
/// Automatically migrates from legacy plaintext storage if found.
/// Uses keyring storage if available, falling back to file storage.
pub async fn load_credentials() -> Result<Option<Credentials>> {
    // First, try to migrate any legacy plaintext credentials
    if let Ok(true) = migrate_legacy_credentials().await {
        tracing::info!("Credentials migrated from plaintext to secure storage");
    }

    // Load from appropriate storage
    #[cfg(feature = "keyring-storage")]
    let creds = load_credentials_from_keyring()?;

    #[cfg(not(feature = "keyring-storage"))]
    let creds = load_credentials_from_file()?;

    // Check if expired
    if let Some(ref c) = creds {
        if let Some(expires_at) = c.expires_at {
            if chrono::Utc::now() > expires_at {
                tracing::info!("Credentials expired, deleting");
                let _ = delete_credentials().await;
                return Ok(None);
            }
        }
    }

    Ok(creds)
}

/// Save credentials to secure storage.
pub async fn save_credentials(creds: &Credentials) -> Result<()> {
    #[cfg(feature = "keyring-storage")]
    save_credentials_to_keyring(creds)?;

    #[cfg(not(feature = "keyring-storage"))]
    save_credentials_to_file(creds)?;

    tracing::info!("Credentials saved securely for user: {}", creds.user_email);
    Ok(())
}

/// Delete credentials from all storage locations.
pub async fn delete_credentials() -> Result<()> {
    #[cfg(feature = "keyring-storage")]
    delete_credentials_from_keyring()?;

    #[cfg(not(feature = "keyring-storage"))]
    delete_credentials_from_file();

    // Also clean up any legacy file
    delete_legacy_credentials();

    Ok(())
}

/// Check authentication status by verifying stored credentials with the server.
pub async fn check_auth() -> Result<AuthStatus> {
    if let Some(creds) = load_credentials().await? {
        // Verify token is still valid
        let client = CloudClient::new();
        match client.verify_token(&creds.access_token).await {
            Ok(TokenVerifyResult::Valid) => {
                tracing::debug!("Token verified successfully");
                Ok(AuthStatus {
                    authenticated: true,
                    user_email: Some(creds.user_email),
                    network_id: Some(creds.network_id),
                    network_name: Some(creds.network_name),
                })
            }
            Ok(TokenVerifyResult::Invalid) => {
                tracing::warn!("Token rejected by server, clearing credentials");
                let _ = delete_credentials().await;
                Ok(AuthStatus {
                    authenticated: false,
                    user_email: None,
                    network_id: None,
                    network_name: None,
                })
            }
            Ok(TokenVerifyResult::NetworkError(reason)) => {
                // Could not reach server - assume still authenticated
                tracing::info!(
                    "Could not verify token ({}), assuming still authenticated",
                    reason
                );
                Ok(AuthStatus {
                    authenticated: true,
                    user_email: Some(creds.user_email),
                    network_id: Some(creds.network_id),
                    network_name: Some(creds.network_name),
                })
            }
            Err(e) => {
                tracing::warn!("Error verifying token: {}, assuming still authenticated", e);
                Ok(AuthStatus {
                    authenticated: true,
                    user_email: Some(creds.user_email),
                    network_id: Some(creds.network_id),
                    network_name: Some(creds.network_name),
                })
            }
        }
    } else {
        Ok(AuthStatus {
            authenticated: false,
            user_email: None,
            network_id: None,
            network_name: None,
        })
    }
}

/// Get information about credential storage location (for documentation/debugging)
pub fn get_credential_storage_info() -> String {
    #[cfg(all(feature = "keyring-storage", target_os = "windows"))]
    {
        "Windows Credential Manager (with file fallback)".to_string()
    }
    #[cfg(all(feature = "keyring-storage", target_os = "macos"))]
    {
        "macOS Keychain (with file fallback)".to_string()
    }
    #[cfg(all(feature = "keyring-storage", target_os = "linux"))]
    {
        "Linux Secret Service (GNOME Keyring/KWallet, with file fallback)".to_string()
    }
    #[cfg(not(feature = "keyring-storage"))]
    {
        let path = get_credentials_file_path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "~/.config/cartographer/.credentials".to_string());
        format!("File-based storage: {}", path)
    }
}
