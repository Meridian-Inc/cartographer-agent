//! OAuth 2.0 Device Authorization Grant flow implementation.
//!
//! This module implements the device flow for authenticating CLI/headless agents
//! without requiring a browser on the same machine.

use crate::auth::credentials::{save_credentials, Credentials};
use crate::cloud::CloudClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

/// Event payload for login URL notification
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginUrlEvent {
    pub verification_url: String,
    pub user_code: String,
}

/// Response from starting the login flow
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginFlowStarted {
    pub verification_url: String,
    pub user_code: String,
    pub device_code: String,
    pub expires_in: u64,
    pub poll_interval: u64,
}

/// Request a device code and return the verification URL.
///
/// This starts the OAuth device flow by requesting a device code from the cloud.
/// The user must visit the verification URL and enter the user code to authorize.
///
/// If the `browser` feature is enabled, this will automatically attempt to open
/// the verification URL in the default browser.
pub async fn request_login_url() -> Result<LoginFlowStarted> {
    let client = CloudClient::new();

    // Request device code from cloud
    let device_code_resp = client
        .request_device_code()
        .await
        .context("Failed to request device code")?;

    // Build the verification URL with the user code
    let url = format!(
        "{}?code={}",
        device_code_resp.verification_uri, device_code_resp.user_code
    );

    tracing::info!("Login URL generated: {}", url);

    // Try to open browser automatically (if browser feature enabled)
    #[cfg(feature = "browser")]
    {
        if let Err(e) = webbrowser::open(&url) {
            tracing::warn!(
                "Failed to open browser automatically: {}. User can use the manual link.",
                e
            );
        }
    }

    Ok(LoginFlowStarted {
        verification_url: url,
        user_code: device_code_resp.user_code,
        device_code: device_code_resp.device_code,
        expires_in: device_code_resp.expires_in,
        poll_interval: device_code_resp.interval.unwrap_or(5),
    })
}

/// Poll for token completion after starting the login flow.
///
/// This should be called after `request_login_url()`. It will poll the cloud
/// server until the user completes authorization or the device code expires.
///
/// Returns the auth status when the user completes authorization.
pub async fn poll_for_login(
    device_code: &str,
    expires_in: u64,
    poll_interval: u64,
) -> Result<crate::auth::credentials::AuthStatus> {
    let client = CloudClient::new();

    let poll_interval_duration = Duration::from_secs(poll_interval);
    let expires_at = std::time::Instant::now() + Duration::from_secs(expires_in);

    loop {
        if std::time::Instant::now() > expires_at {
            return Err(anyhow::anyhow!("Device code expired. Please try again."));
        }

        match client.poll_for_token(device_code).await {
            Ok(Some(token_resp)) => {
                // Success! Save credentials
                let expires_at = token_resp
                    .expires_in
                    .map(|secs| chrono::Utc::now() + chrono::Duration::seconds(secs as i64));

                let network_id = token_resp.network_id;
                let network_name = token_resp.network_name;
                let user_email = token_resp.user_email;

                let creds = Credentials {
                    access_token: token_resp.access_token,
                    network_id: network_id.clone(),
                    network_name: network_name.clone(),
                    user_email: user_email.clone(),
                    expires_at,
                };

                save_credentials(&creds).await?;

                tracing::info!(
                    "Successfully connected to network '{}' (id: {})",
                    network_name,
                    network_id
                );

                return Ok(crate::auth::credentials::AuthStatus {
                    authenticated: true,
                    user_email: Some(user_email),
                    network_id: Some(network_id),
                    network_name: Some(network_name),
                });
            }
            Ok(None) => {
                // Still waiting for user authorization
                sleep(poll_interval_duration).await;
            }
            Err(e) => {
                return Err(e.context("Failed to poll for token"));
            }
        }
    }
}

/// Start login with an optional event callback (for GUI integration).
///
/// This combines `request_login_url()` and `poll_for_login()` into a single call,
/// emitting the verification URL via a callback if provided.
pub async fn start_login<F>(
    emit_url: Option<F>,
) -> Result<crate::auth::credentials::AuthStatus>
where
    F: Fn(LoginUrlEvent) + Send + Sync,
{
    // Get the login URL first
    let login_info = request_login_url().await?;

    // Emit the URL to callback if provided
    if let Some(emit) = &emit_url {
        emit(LoginUrlEvent {
            verification_url: login_info.verification_url.clone(),
            user_code: login_info.user_code.clone(),
        });
    }

    // Poll for completion
    poll_for_login(
        &login_info.device_code,
        login_info.expires_in,
        login_info.poll_interval,
    )
    .await
}
