use crate::cloud::{CloudClient, TokenVerifyResult};
use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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

fn get_credentials_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .context("Failed to find config directory")?;
    
    let app_dir = config_dir.join("cartographer");
    fs::create_dir_all(&app_dir)?;
    
    Ok(app_dir.join("credentials.json"))
}

pub async fn load_credentials() -> Result<Option<Credentials>> {
    let path = get_credentials_path()?;
    
    if !path.exists() {
        return Ok(None);
    }
    
    let content = fs::read_to_string(&path)
        .context("Failed to read credentials file")?;
    
    // Try to parse as current format first
    let creds: Credentials = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(_) => {
            // Try to parse as legacy format (network_id as integer)
            if let Ok(legacy) = serde_json::from_str::<LegacyCredentials>(&content) {
                tracing::info!("Migrating credentials from legacy format");
                let migrated = Credentials {
                    access_token: legacy.access_token,
                    network_id: legacy.network_id.to_string(),
                    network_name: legacy.network_name,
                    user_email: legacy.user_email,
                    expires_at: legacy.expires_at,
                };
                // Save migrated credentials
                if let Err(e) = save_credentials_sync(&migrated, &path) {
                    tracing::warn!("Failed to save migrated credentials: {}", e);
                }
                migrated
            } else {
                // Credentials file is corrupted, delete it
                tracing::warn!("Credentials file corrupted, deleting");
                let _ = fs::remove_file(&path);
                return Ok(None);
            }
        }
    };
    
    // Check if expired
    if let Some(expires_at) = creds.expires_at {
        if chrono::Utc::now() > expires_at {
            // Credentials expired, delete them
            let _ = fs::remove_file(&path);
            return Ok(None);
        }
    }
    
    Ok(Some(creds))
}

fn save_credentials_sync(creds: &Credentials, path: &PathBuf) -> Result<()> {
    let content = serde_json::to_string_pretty(creds)
        .context("Failed to serialize credentials")?;
    fs::write(path, content)
        .context("Failed to write credentials file")?;
    Ok(())
}

pub async fn save_credentials(creds: &Credentials) -> Result<()> {
    let path = get_credentials_path()?;
    let content = serde_json::to_string_pretty(creds)
        .context("Failed to serialize credentials")?;
    
    fs::write(&path, content)
        .context("Failed to write credentials file")?;
    
    Ok(())
}

pub async fn delete_credentials() -> Result<()> {
    let path = get_credentials_path()?;
    if path.exists() {
        fs::remove_file(&path)?;
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

