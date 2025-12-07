use crate::auth::credentials::{save_credentials, Credentials};
use crate::cloud::{CloudClient, DeviceCodeResponse, TokenResponse};
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;

pub async fn start_login() -> Result<crate::auth::credentials::AuthStatus> {
    let client = CloudClient::new();
    
    // Step 1: Request device code
    let device_code_resp = client.request_device_code().await
        .context("Failed to request device code")?;
    
    // Step 2: Open browser or show URL
    let url = format!("{}?code={}", device_code_resp.verification_uri, device_code_resp.user_code);
    
    tracing::info!("Opening browser for authentication: {}", url);
    
    // Open browser
    if let Err(e) = webbrowser::open(&url) {
        tracing::warn!("Failed to open browser: {}. Please visit: {}", e, url);
    }
    
    // Step 3: Poll for token
    let poll_interval = Duration::from_secs(device_code_resp.interval.unwrap_or(5));
    let expires_at = std::time::Instant::now() + Duration::from_secs(device_code_resp.expires_in);
    
    loop {
        if std::time::Instant::now() > expires_at {
            return Err(anyhow::anyhow!("Device code expired"));
        }
        
        match client.poll_for_token(&device_code_resp.device_code).await {
            Ok(Some(token_resp)) => {
                // Success! Save credentials
                let expires_at = token_resp.expires_in
                    .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));
                
                let creds = Credentials {
                    access_token: token_resp.access_token,
                    agent_id: token_resp.agent_id,
                    user_email: token_resp.user_email,
                    expires_at,
                };
                
                save_credentials(&creds).await?;
                
                return Ok(crate::auth::credentials::AuthStatus {
                    authenticated: true,
                    user_email: Some(creds.user_email),
                    agent_id: Some(creds.agent_id),
                });
            }
            Ok(None) => {
                // Still waiting for user authorization
                sleep(poll_interval).await;
            }
            Err(e) => {
                return Err(e.context("Failed to poll for token"));
            }
        }
    }
}

