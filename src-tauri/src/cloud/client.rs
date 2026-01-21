use crate::scanner::{Device, ScanResult};
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Shared HTTP client for all API calls.
/// Reusing a single client is more efficient as it maintains connection pools,
/// HTTP/2 connections, and DNS caches across requests.
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

/// Default cloud API URL
const DEFAULT_CLOUD_BASE_URL: &str = "https://cartographer.network/api";

/// Environment variable for custom cloud endpoint
const ENV_CLOUD_URL: &str = "CARTOGRAPHER_CLOUD_URL";

/// Config file structure for agent settings
#[derive(Debug, Default, Deserialize)]
struct AgentConfig {
    #[serde(default)]
    cloud_url: Option<String>,
}

fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|dir| dir.join("cartographer").join("config.toml"))
}

fn load_config() -> AgentConfig {
    if let Some(path) = get_config_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
    }
    AgentConfig::default()
}

/// Get the cloud base URL from (in priority order):
/// 1. Environment variable CARTOGRAPHER_CLOUD_URL
/// 2. Config file ~/.config/cartographer/config.toml
/// 3. Default: https://cartographer.network/api
fn get_cloud_base_url() -> String {
    // 1. Check environment variable
    if let Ok(url) = env::var(ENV_CLOUD_URL) {
        if !url.is_empty() {
            tracing::debug!("Using cloud URL from environment: {}", url);
            return url;
        }
    }

    // 2. Check config file
    let config = load_config();
    if let Some(url) = config.cloud_url {
        if !url.is_empty() {
            tracing::debug!("Using cloud URL from config file: {}", url);
            return url;
        }
    }

    // 3. Use default
    tracing::debug!("Using default cloud URL: {}", DEFAULT_CLOUD_BASE_URL);
    DEFAULT_CLOUD_BASE_URL.to_string()
}

#[derive(Debug, Clone)]
pub struct CloudClient {
    base_url: String,
}

impl CloudClient {
    pub fn new() -> Self {
        Self {
            base_url: get_cloud_base_url(),
        }
    }

    /// Create a client with a custom base URL (for testing or self-hosted)
    pub fn with_base_url(base_url: String) -> Self {
        Self { base_url }
    }

    /// Get the current base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        let url = format!("{}/agent/device-code", self.base_url);

        let resp = HTTP_CLIENT
            .post(&url)
            .send()
            .await
            .context("Failed to request device code")?;
        
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Server returned error: {}", resp.status()));
        }
        
        resp.json::<DeviceCodeResponse>()
            .await
            .context("Failed to parse device code response")
    }

    pub async fn poll_for_token(&self, device_code: &str) -> Result<Option<TokenResponse>> {
        let url = format!("{}/agent/token", self.base_url);

        let resp = HTTP_CLIENT
            .post(&url)
            .json(&TokenRequest {
                device_code: device_code.to_string(),
                grant_type: "device_code".to_string(),
            })
            .send()
            .await
            .context("Failed to poll for token")?;
        
        match resp.status().as_u16() {
            200 => {
                let token_resp = resp.json::<TokenResponse>()
                    .await
                    .context("Failed to parse token response")?;
                Ok(Some(token_resp))
            }
            400 => {
                // Still waiting (authorization_pending) or other error
                // FastAPI wraps HTTPException in a "detail" field
                let body = resp.text().await.unwrap_or_default();
                
                // Try FastAPI format first: {"detail": {"error": "...", "error_description": "..."}}
                if let Ok(fastapi_err) = serde_json::from_str::<FastApiErrorResponse>(&body) {
                    if fastapi_err.detail.error == "authorization_pending" {
                        return Ok(None);
                    }
                    return Err(anyhow::anyhow!("{}: {}", 
                        fastapi_err.detail.error, 
                        fastapi_err.detail.error_description.unwrap_or_default()));
                }
                
                // Fall back to direct format: {"error": "...", "error_description": "..."}
                if let Ok(err) = serde_json::from_str::<TokenErrorResponse>(&body) {
                    if err.error == "authorization_pending" {
                        return Ok(None);
                    }
                    return Err(anyhow::anyhow!("{}: {}", err.error, err.error_description.unwrap_or_default()));
                }
                
                // Unknown 400 error - continue polling
                tracing::warn!("Unknown 400 response during token poll: {}", body);
                Ok(None)
            }
            _ => {
                Err(anyhow::anyhow!("Server returned error: {}", resp.status()))
            }
        }
    }

    /// Result of token verification
    /// - Ok(TokenVerifyResult::Valid) - token is valid
    /// - Ok(TokenVerifyResult::Invalid) - token was rejected by server (401/403)
    /// - Ok(TokenVerifyResult::NetworkError) - couldn't reach server
    pub async fn verify_token(&self, token: &str) -> Result<TokenVerifyResult> {
        let url = format!("{}/agent/verify", self.base_url);

        let resp = match HTTP_CLIENT
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                // Network error - could not reach server
                tracing::debug!("Token verification network error: {}", e);
                return Ok(TokenVerifyResult::NetworkError(e.to_string()));
            }
        };

        match resp.status().as_u16() {
            200 => Ok(TokenVerifyResult::Valid),
            401 | 403 => Ok(TokenVerifyResult::Invalid),
            status => {
                // Treat other errors (500, etc.) as transient network issues
                tracing::debug!("Token verification returned status {}", status);
                Ok(TokenVerifyResult::NetworkError(format!("Server returned {}", status)))
            }
        }
    }

    /// Upload scan results to the cloud, including gateway detection and network info.
    pub async fn upload_scan_result(&self, scan_result: &ScanResult) -> Result<()> {
        // Get credentials
        let creds = crate::auth::load_credentials()
            .await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let url = format!("{}/agent/sync", self.base_url);

        let gateway_ip = scan_result.network_info.gateway_ip.as_deref();

        tracing::info!(
            "Uploading {} devices to cloud (network: {}, gateway: {:?})",
            scan_result.devices.len(),
            creds.network_name,
            gateway_ip
        );

        let payload = SyncRequest {
            timestamp: chrono::Utc::now().to_rfc3339(),
            scan_duration_ms: None,
            devices: scan_result
                .devices
                .iter()
                .map(|d| ScanDevice {
                    ip: d.ip.clone(),
                    mac: d.mac.clone(),
                    response_time_ms: d.response_time_ms,
                    hostname: d.hostname.clone(),
                    // Mark device as gateway if its IP matches the detected gateway
                    is_gateway: gateway_ip.map_or(false, |gw| gw == d.ip),
                    vendor: d.vendor.clone(),
                    device_type: d.device_type.clone(),
                })
                .collect(),
            network_info: Some(NetworkInfo {
                subnet: Some(scan_result.network_info.subnet.clone()),
                interface: Some(scan_result.network_info.interface.clone()),
            }),
        };

        let resp = HTTP_CLIENT
            .post(&url)
            .bearer_auth(&creds.access_token)
            .json(&payload)
            .send()
            .await
            .context("Failed to upload scan")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Sync failed: {} - {}", status, body);
            return Err(anyhow::anyhow!("Server returned error: {} - {}", status, body));
        }

        tracing::info!("Scan uploaded successfully");
        Ok(())
    }

    /// Legacy function - upload devices without network info (for backward compatibility)
    pub async fn upload_scan(&self, devices: &[Device]) -> Result<()> {
        // Get credentials
        let creds = crate::auth::load_credentials()
            .await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let url = format!("{}/agent/sync", self.base_url);

        tracing::info!(
            "Uploading {} devices to cloud (network: {})",
            devices.len(),
            creds.network_name
        );

        let payload = SyncRequest {
            timestamp: chrono::Utc::now().to_rfc3339(),
            scan_duration_ms: None,
            devices: devices
                .iter()
                .map(|d| ScanDevice {
                    ip: d.ip.clone(),
                    mac: d.mac.clone(),
                    response_time_ms: d.response_time_ms,
                    hostname: d.hostname.clone(),
                    is_gateway: false,
                    vendor: d.vendor.clone(),
                    device_type: d.device_type.clone(),
                })
                .collect(),
            network_info: None,
        };

        let resp = HTTP_CLIENT
            .post(&url)
            .bearer_auth(&creds.access_token)
            .json(&payload)
            .send()
            .await
            .context("Failed to upload scan")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            tracing::error!("Sync failed: {} - {}", status, body);
            return Err(anyhow::anyhow!("Server returned error: {} - {}", status, body));
        }

        tracing::info!("Scan uploaded successfully");
        Ok(())
    }

    pub async fn get_network_info(&self) -> Result<NetworkInfoResponse> {
        let creds = crate::auth::load_credentials()
            .await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let url = format!("{}/agent/network", self.base_url);

        let resp = HTTP_CLIENT
            .get(&url)
            .bearer_auth(&creds.access_token)
            .send()
            .await
            .context("Failed to get network info")?;
        
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Server returned error: {}", resp.status()));
        }
        
        resp.json::<NetworkInfoResponse>()
            .await
            .context("Failed to parse network info response")
    }

    pub async fn upload_health_check(
        &self,
        results: &[crate::scheduler::DeviceHealthResult],
    ) -> Result<()> {
        let creds = crate::auth::load_credentials()
            .await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;

        let url = format!("{}/agent/health", self.base_url);

        let payload = HealthCheckRequest {
            timestamp: chrono::Utc::now().to_rfc3339(),
            results: results
                .iter()
                .map(|r| HealthCheckResult {
                    ip: r.ip.clone(),
                    reachable: r.reachable,
                    response_time_ms: r.response_time_ms,
                })
                .collect(),
        };

        let resp = HTTP_CLIENT
            .post(&url)
            .bearer_auth(&creds.access_token)
            .json(&payload)
            .send()
            .await
            .context("Failed to upload health check")?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Server returned error: {} - {}", status, body));
        }
        
        Ok(())
    }

    pub async fn open_dashboard(&self) -> Result<()> {
        let creds = crate::auth::load_credentials().await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
        
        // Navigate to the core app with network context
        // Extract base domain from API URL (remove /api suffix)
        let base_domain = self.base_url
            .trim_end_matches("/api")
            .trim_end_matches("/");
        let url = format!("{}/app/network/{}", base_domain, creds.network_id);
        webbrowser::open(&url)
            .context("Failed to open dashboard in browser")
    }
}

/// Result of token verification attempt
#[derive(Debug, Clone)]
pub enum TokenVerifyResult {
    /// Token is valid
    Valid,
    /// Token was explicitly rejected by the server (401/403)
    Invalid,
    /// Could not reach the server (network error, timeout, server error)
    NetworkError(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenRequest {
    device_code: String,
    grant_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// FastAPI wraps HTTPException responses in a "detail" field
#[derive(Debug, Serialize, Deserialize)]
struct FastApiErrorResponse {
    detail: TokenErrorResponse,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: Option<u64>,
    pub network_id: String,
    pub network_name: String,
    pub user_email: String,
}

#[derive(Debug, Serialize)]
struct SyncRequest {
    timestamp: String,
    scan_duration_ms: Option<u64>,
    devices: Vec<ScanDevice>,
    network_info: Option<NetworkInfo>,
}

#[derive(Debug, Serialize)]
struct NetworkInfo {
    subnet: Option<String>,
    interface: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanDevice {
    ip: String,
    mac: Option<String>,
    response_time_ms: Option<f64>,
    hostname: Option<String>,
    is_gateway: bool,
    /// Device vendor/manufacturer from MAC OUI lookup
    vendor: Option<String>,
    /// Inferred device type based on vendor
    device_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkInfoResponse {
    pub network_id: String,
    pub network_name: String,
    pub last_sync_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthCheckRequest {
    timestamp: String,
    results: Vec<HealthCheckResult>,
}

#[derive(Debug, Serialize)]
struct HealthCheckResult {
    ip: String,
    reachable: bool,
    response_time_ms: Option<f64>,
}

