use crate::auth::credentials::{save_credentials, Credentials};
use crate::cloud::CloudClient;
use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;

pub async fn start_login() -> Result<crate::auth::credentials::AuthStatus> {
    let client = CloudClient::new();
    
    // Step 1: Request device code
    let device_code_resp = client.request_device_code().await
        .context("Failed to request device code")?;
    
    // Step 2: Open browser - use the verification_uri from the response
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
            return Err(anyhow::anyhow!("Device code expired. Please try again."));
        }
        
        match client.poll_for_token(&device_code_resp.device_code).await {
            Ok(Some(token_resp)) => {
                // Success! Save credentials with network info
                let expires_at = token_resp.expires_in
                    .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));
                
                let creds = Credentials {
                    access_token: token_resp.access_token,
                    network_id: token_resp.network_id,
                    network_name: token_resp.network_name.clone(),
                    user_email: token_resp.user_email.clone(),
                    expires_at,
                };
                
                save_credentials(&creds).await?;
                
                tracing::info!(
                    "Successfully connected to network '{}' (id: {})",
                    token_resp.network_name,
                    token_resp.network_id
                );
                
                return Ok(crate::auth::credentials::AuthStatus {
                    authenticated: true,
                    user_email: Some(token_resp.user_email),
                    network_id: Some(token_resp.network_id),
                    network_name: Some(token_resp.network_name),
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

