use crate::cloud::{CloudClient, TokenVerifyResult};
use anyhow::{Context, Result};
use dirs;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Service name used for keyring storage
const KEYRING_SERVICE: &str = "cartographer-agent";
/// Username used for keyring entry (we store a single credential set)
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

/// Get the legacy plaintext credentials file path (for migration only)
fn get_legacy_credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("Failed to find config directory")?;

    let app_dir = config_dir.join("cartographer");
    Ok(app_dir.join("credentials.json"))
}

/// Get the fallback credentials file path for Windows where keyring can be unreliable
#[cfg(target_os = "windows")]
fn get_fallback_credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("Failed to find config directory")?;

    let app_dir = config_dir.join("cartographer");
    // Create directory if it doesn't exist
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir).context("Failed to create config directory")?;
    }
    Ok(app_dir.join(".credentials"))
}

/// Save credentials to fallback file storage (Windows only)
#[cfg(target_os = "windows")]
fn save_credentials_to_file(creds: &Credentials) -> Result<()> {
    let path = get_fallback_credentials_path()?;
    let json = serde_json::to_string(creds).context("Failed to serialize credentials")?;
    fs::write(&path, &json).context("Failed to write credentials file")?;
    tracing::debug!("Credentials saved to fallback file: {:?}", path);
    Ok(())
}

/// Load credentials from fallback file storage (Windows only)
#[cfg(target_os = "windows")]
fn load_credentials_from_file() -> Result<Option<Credentials>> {
    let path = get_fallback_credentials_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).context("Failed to read credentials file")?;
    let creds: Credentials = serde_json::from_str(&content).context("Failed to parse credentials file")?;
    tracing::debug!("Credentials loaded from fallback file");
    Ok(Some(creds))
}

/// Delete credentials from fallback file storage (Windows only)
#[cfg(target_os = "windows")]
fn delete_credentials_from_file() {
    if let Ok(path) = get_fallback_credentials_path() {
        if path.exists() {
            if let Err(e) = fs::remove_file(&path) {
                tracing::warn!("Failed to delete fallback credentials file: {}", e);
            }
        }
    }
}

/// Get keyring entry for credential storage
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

/// Migrate credentials from legacy plaintext file to secure keyring storage
async fn migrate_legacy_credentials() -> Result<bool> {
    // Check if legacy credentials exist
    let legacy_creds = match load_legacy_credentials() {
        Ok(Some(creds)) => creds,
        Ok(None) => return Ok(false),
        Err(e) => {
            tracing::warn!("Failed to load legacy credentials for migration: {}", e);
            return Ok(false);
        }
    };

    tracing::info!(
        "Migrating credentials from plaintext file to secure keyring storage for user: {}",
        legacy_creds.user_email
    );

    // Save to keyring
    if let Err(e) = save_credentials_to_keyring(&legacy_creds) {
        tracing::error!("Failed to migrate credentials to keyring: {}", e);
        // Don't delete legacy file if migration failed
        return Err(e);
    }

    // Delete legacy file after successful migration
    delete_legacy_credentials();

    tracing::info!("Successfully migrated credentials to secure keyring storage");
    Ok(true)
}

/// Save credentials to the platform keyring
fn save_credentials_to_keyring(creds: &Credentials) -> Result<()> {
    let entry = match get_keyring_entry() {
        Ok(e) => e,
        Err(e) => {
            tracing::error!("Failed to create keyring entry for saving: {}", e);
            // On Windows, fall back to file storage
            #[cfg(target_os = "windows")]
            {
                tracing::info!("Falling back to file-based credential storage");
                return save_credentials_to_file(creds);
            }
            #[cfg(not(target_os = "windows"))]
            return Err(e);
        }
    };
    let json = serde_json::to_string(creds).context("Failed to serialize credentials")?;

    if let Err(e) = entry.set_password(&json) {
        tracing::error!("Failed to save credentials to keyring: {}", e);
        // On Windows, fall back to file storage
        #[cfg(target_os = "windows")]
        {
            tracing::info!("Keyring save failed, falling back to file-based storage");
            return save_credentials_to_file(creds);
        }
        #[cfg(not(target_os = "windows"))]
        return Err(anyhow::anyhow!("Failed to save credentials to keyring: {}", e));
    }

    // Verify the save worked by reading it back immediately
    // This catches issues where the keyring reports success but doesn't actually persist
    let verify_entry = get_keyring_entry()?;
    match verify_entry.get_password() {
        Ok(stored_json) => {
            if stored_json == json {
                tracing::debug!("Credentials verified in keyring after save");
                // On Windows, also save to file as backup
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = save_credentials_to_file(creds) {
                        tracing::debug!("Failed to save backup credentials to file: {}", e);
                    }
                }
            } else {
                tracing::warn!("Credentials mismatch after save - stored content differs");
                // On Windows, use file storage as primary
                #[cfg(target_os = "windows")]
                {
                    tracing::info!("Keyring mismatch, using file-based storage as primary");
                    return save_credentials_to_file(creds);
                }
            }
        }
        Err(keyring::Error::NoEntry) => {
            tracing::error!("Credentials verification failed - not found immediately after save");
            // On Windows, fall back to file storage
            #[cfg(target_os = "windows")]
            {
                tracing::info!("Keyring verification failed, falling back to file-based storage");
                return save_credentials_to_file(creds);
            }
            #[cfg(not(target_os = "windows"))]
            return Err(anyhow::anyhow!("Keyring save did not persist - credential not found after save"));
        }
        Err(e) => {
            tracing::warn!("Could not verify credentials after save: {}", e);
            // On Windows, also save to file as backup in case of verification issues
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = save_credentials_to_file(creds) {
                    tracing::warn!("Failed to save backup credentials to file: {}", e);
                }
            }
        }
    }

    Ok(())
}

/// Load credentials from the platform keyring (with file fallback on Windows)
fn load_credentials_from_keyring() -> Result<Option<Credentials>> {
    let entry = match get_keyring_entry() {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to create keyring entry for reading: {}", e);
            // On Windows, try file fallback
            #[cfg(target_os = "windows")]
            {
                tracing::debug!("Trying file-based fallback due to keyring entry error");
                return load_credentials_from_file();
            }
            #[cfg(not(target_os = "windows"))]
            return Err(e);
        }
    };

    match entry.get_password() {
        Ok(json) => {
            tracing::debug!("Credentials loaded from keyring successfully");
            let creds: Credentials =
                serde_json::from_str(&json).context("Failed to parse credentials from keyring")?;
            Ok(Some(creds))
        }
        Err(keyring::Error::NoEntry) => {
            tracing::debug!("No credentials found in keyring (NoEntry)");
            // On Windows, try file fallback
            #[cfg(target_os = "windows")]
            {
                tracing::debug!("Trying file-based fallback");
                match load_credentials_from_file() {
                    Ok(Some(creds)) => {
                        tracing::info!("Credentials loaded from file fallback");
                        return Ok(Some(creds));
                    }
                    Ok(None) => {
                        tracing::debug!("No credentials in file fallback either");
                    }
                    Err(e) => {
                        tracing::debug!("Failed to load from file fallback: {}", e);
                    }
                }
            }
            Ok(None)
        }
        Err(e) => {
            tracing::warn!("Failed to load credentials from keyring: {}", e);
            // On Windows, try file fallback
            #[cfg(target_os = "windows")]
            {
                tracing::debug!("Trying file-based fallback due to keyring error");
                return load_credentials_from_file();
            }
            #[cfg(not(target_os = "windows"))]
            Err(anyhow::anyhow!("Failed to access keyring: {}", e))
        }
    }
}

/// Delete credentials from the platform keyring
fn delete_credentials_from_keyring() -> Result<()> {
    // On Windows, also delete from file fallback
    #[cfg(target_os = "windows")]
    delete_credentials_from_file();

    let entry = get_keyring_entry()?;

    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()), // Already deleted
        Err(e) => Err(anyhow::anyhow!("Failed to delete credentials from keyring: {}", e)),
    }
}

pub async fn load_credentials() -> Result<Option<Credentials>> {
    // First, try to migrate any legacy plaintext credentials
    if let Ok(true) = migrate_legacy_credentials().await {
        tracing::info!("Credentials migrated from plaintext to secure storage");
    }

    // Load from keyring (includes file fallback on Windows)
    let creds = match load_credentials_from_keyring() {
        Ok(Some(c)) => c,
        Ok(None) => {
            tracing::debug!("No credentials found in any storage");
            return Ok(None);
        }
        Err(e) => {
            tracing::warn!("Failed to load credentials: {}", e);
            return Ok(None);
        }
    };

    // Check if expired
    if let Some(expires_at) = creds.expires_at {
        if chrono::Utc::now() > expires_at {
            tracing::info!("Credentials expired, deleting");
            let _ = delete_credentials_from_keyring();
            return Ok(None);
        }
    }

    Ok(Some(creds))
}

pub async fn save_credentials(creds: &Credentials) -> Result<()> {
    save_credentials_to_keyring(creds)?;
    tracing::info!(
        "Credentials saved securely for user: {}",
        creds.user_email
    );
    Ok(())
}

pub async fn delete_credentials() -> Result<()> {
    delete_credentials_from_keyring()?;
    // Also clean up any legacy file that might exist
    delete_legacy_credentials();
    Ok(())
}

pub async fn check_auth() -> Result<AuthStatus> {
    if let Some(creds) = load_credentials().await? {
        // Verify token is still valid by making a test API call
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
                // Token was explicitly rejected by server (401/403)
                // This means the token is definitely invalid, so delete credentials
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
                // Could not reach server - this is NOT an auth failure
                // Keep credentials and assume still authenticated
                // The next upload will fail but that's handled separately
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
                // Error building the request - treat as network error
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

pub async fn logout() -> Result<()> {
    delete_credentials().await?;
    Ok(())
}

/// Get information about credential storage location (for documentation/debugging)
pub fn get_credential_storage_info() -> String {
    #[cfg(target_os = "windows")]
    {
        "Windows Credential Manager (Control Panel > User Accounts > Credential Manager)".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "macOS Keychain (Keychain Access app, search for 'cartographer-agent')".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "Linux Secret Service (GNOME Keyring or KWallet, via libsecret)".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Platform keyring (platform-specific secure storage)".to_string()
    }
}
