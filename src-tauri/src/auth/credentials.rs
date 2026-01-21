use crate::cloud::{CloudClient, TokenVerifyResult};
use anyhow::{Context, Result};
use dirs;
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Keyring service name for credential storage
const KEYRING_SERVICE: &str = "cartographer-agent";

/// Keyring usernames for different credential fields
const KEY_ACCESS_TOKEN: &str = "access_token";
const KEY_METADATA: &str = "metadata";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub network_id: String,
    pub network_name: String,
    pub user_email: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Metadata stored in keyring (non-sensitive parts)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CredentialMetadata {
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

/// Legacy file-based credentials (for migration)
#[derive(Debug, Deserialize)]
struct FileCredentials {
    access_token: String,
    network_id: String,
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

fn get_legacy_credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("Failed to find config directory")?;
    
    let app_dir = config_dir.join("cartographer");
    fs::create_dir_all(&app_dir)?;
    
    Ok(app_dir.join("credentials.json"))
}

/// Get keyring entry for a specific key
fn get_keyring_entry(key: &str) -> Result<Entry> {
    Entry::new(KEYRING_SERVICE, key)
        .map_err(|e| anyhow::anyhow!("Failed to create keyring entry: {}", e))
}

/// Store access token in keyring
fn store_token_in_keyring(token: &str) -> Result<()> {
    let entry = get_keyring_entry(KEY_ACCESS_TOKEN)?;
    entry.set_password(token)
        .map_err(|e| anyhow::anyhow!("Failed to store token in keyring: {}", e))
}

/// Retrieve access token from keyring
fn get_token_from_keyring() -> Result<Option<String>> {
    let entry = get_keyring_entry(KEY_ACCESS_TOKEN)?;
    match entry.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to get token from keyring: {}", e)),
    }
}

/// Store metadata in keyring (as JSON)
fn store_metadata_in_keyring(metadata: &CredentialMetadata) -> Result<()> {
    let entry = get_keyring_entry(KEY_METADATA)?;
    let json = serde_json::to_string(metadata)
        .context("Failed to serialize metadata")?;
    entry.set_password(&json)
        .map_err(|e| anyhow::anyhow!("Failed to store metadata in keyring: {}", e))
}

/// Retrieve metadata from keyring
fn get_metadata_from_keyring() -> Result<Option<CredentialMetadata>> {
    let entry = get_keyring_entry(KEY_METADATA)?;
    match entry.get_password() {
        Ok(json) => {
            let metadata: CredentialMetadata = serde_json::from_str(&json)
                .context("Failed to parse metadata from keyring")?;
            Ok(Some(metadata))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Failed to get metadata from keyring: {}", e)),
    }
}

/// Delete credentials from keyring
fn delete_from_keyring() -> Result<()> {
    // Delete access token
    if let Ok(entry) = get_keyring_entry(KEY_ACCESS_TOKEN) {
        let _ = entry.delete_credential(); // Ignore errors if not found
    }
    // Delete metadata
    if let Ok(entry) = get_keyring_entry(KEY_METADATA) {
        let _ = entry.delete_credential(); // Ignore errors if not found
    }
    Ok(())
}

/// Migrate credentials from legacy JSON file to keyring
async fn migrate_from_file() -> Result<Option<Credentials>> {
    let path = get_legacy_credentials_path()?;
    
    if !path.exists() {
        return Ok(None);
    }
    
    tracing::info!("Found legacy credentials file, migrating to secure storage...");
    
    let content = fs::read_to_string(&path)
        .context("Failed to read legacy credentials file")?;
    
    // Try to parse as current format first
    let creds: Credentials = match serde_json::from_str::<FileCredentials>(&content) {
        Ok(c) => Credentials {
            access_token: c.access_token,
            network_id: c.network_id,
            network_name: c.network_name,
            user_email: c.user_email,
            expires_at: c.expires_at,
        },
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
                // Credentials file is corrupted, delete it
                tracing::warn!("Legacy credentials file corrupted, deleting");
                let _ = fs::remove_file(&path);
                return Ok(None);
            }
        }
    };
    
    // Store in keyring
    if let Err(e) = save_credentials(&creds).await {
        tracing::warn!("Failed to migrate credentials to keyring: {}", e);
        // Fall back to returning file credentials
        return Ok(Some(creds));
    }
    
    // Delete the old file after successful migration
    if let Err(e) = fs::remove_file(&path) {
        tracing::warn!("Failed to delete legacy credentials file: {}", e);
    } else {
        tracing::info!("Successfully migrated credentials to secure storage");
    }
    
    Ok(Some(creds))
}

pub async fn load_credentials() -> Result<Option<Credentials>> {
    // First, try to load from keyring
    let token = match get_token_from_keyring() {
        Ok(Some(t)) => t,
        Ok(None) => {
            // No token in keyring, check for legacy file
            return migrate_from_file().await;
        }
        Err(e) => {
            tracing::warn!("Failed to access keyring: {}, checking legacy file", e);
            return migrate_from_file().await;
        }
    };
    
    // Get metadata
    let metadata = match get_metadata_from_keyring() {
        Ok(Some(m)) => m,
        Ok(None) => {
            tracing::warn!("Token found but no metadata in keyring");
            return Ok(None);
        }
        Err(e) => {
            tracing::warn!("Failed to get metadata from keyring: {}", e);
            return Ok(None);
        }
    };
    
    let creds = Credentials {
        access_token: token,
        network_id: metadata.network_id,
        network_name: metadata.network_name,
        user_email: metadata.user_email,
        expires_at: metadata.expires_at,
    };
    
    // Check if expired
    if let Some(expires_at) = creds.expires_at {
        if chrono::Utc::now() > expires_at {
            // Credentials expired, delete them
            let _ = delete_credentials().await;
            return Ok(None);
        }
    }
    
    Ok(Some(creds))
}

pub async fn save_credentials(creds: &Credentials) -> Result<()> {
    // Store token in keyring
    store_token_in_keyring(&creds.access_token)?;
    
    // Store metadata in keyring
    let metadata = CredentialMetadata {
        network_id: creds.network_id.clone(),
        network_name: creds.network_name.clone(),
        user_email: creds.user_email.clone(),
        expires_at: creds.expires_at,
    };
    store_metadata_in_keyring(&metadata)?;
    
    Ok(())
}

pub async fn delete_credentials() -> Result<()> {
    // Delete from keyring
    delete_from_keyring()?;
    
    // Also delete legacy file if it exists
    if let Ok(path) = get_legacy_credentials_path() {
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }
    
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

