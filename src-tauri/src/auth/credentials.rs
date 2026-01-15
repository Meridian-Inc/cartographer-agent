use crate::cloud::CloudClient;
use anyhow::{Context, Result};
use dirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub network_id: i32,
    pub network_name: String,
    pub user_email: String,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user_email: Option<String>,
    pub network_id: Option<i32>,
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
    
    let creds: Credentials = serde_json::from_str(&content)
        .context("Failed to parse credentials")?;
    
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
            Ok(true) => Ok(AuthStatus {
                authenticated: true,
                user_email: Some(creds.user_email),
                network_id: Some(creds.network_id),
                network_name: Some(creds.network_name),
            }),
            Ok(false) | Err(_) => {
                // Token invalid, delete credentials
                let _ = delete_credentials().await;
                Ok(AuthStatus {
                    authenticated: false,
                    user_email: None,
                    network_id: None,
                    network_name: None,
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

