use crate::scanner::Device;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const CLOUD_BASE_URL: &str = "https://cloud.cartographer.example/api/v1";

#[derive(Debug, Clone)]
pub struct CloudClient {
    base_url: String,
}

impl CloudClient {
    pub fn new() -> Self {
        Self {
            base_url: CLOUD_BASE_URL.to_string(),
        }
    }

    pub async fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        let url = format!("{}/auth/device", self.base_url);
        
        let client = reqwest::Client::new();
        let resp = client
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
        let url = format!("{}/auth/token", self.base_url);
        
        let client = reqwest::Client::new();
        let resp = client
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
                // Still waiting (authorization_pending)
                Ok(None)
            }
            _ => {
                Err(anyhow::anyhow!("Server returned error: {}", resp.status()))
            }
        }
    }

    pub async fn verify_token(&self, token: &str) -> Result<bool> {
        let url = format!("{}/auth/verify", self.base_url);
        
        let client = reqwest::Client::new();
        let resp = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to verify token")?;
        
        Ok(resp.status().is_success())
    }

    pub async fn upload_scan(&self, devices: &[Device]) -> Result<()> {
        // Get credentials
        let creds = crate::auth::load_credentials().await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
        
        let url = format!("{}/agents/{}/scans", self.base_url, creds.agent_id);
        
        let payload = ScanUpload {
            timestamp: chrono::Utc::now().to_rfc3339(),
            devices: devices.iter().map(|d| ScanDevice {
                ip: d.ip.clone(),
                mac: d.mac.clone(),
                response_time_ms: d.response_time_ms,
                hostname: d.hostname.clone(),
            }).collect(),
        };
        
        let client = reqwest::Client::new();
        let resp = client
            .post(&url)
            .bearer_auth(&creds.access_token)
            .json(&payload)
            .send()
            .await
            .context("Failed to upload scan")?;
        
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Server returned error: {}", resp.status()));
        }
        
        Ok(())
    }

    pub async fn open_dashboard(&self) -> Result<()> {
        let creds = crate::auth::load_credentials().await
            .context("Failed to load credentials")?
            .ok_or_else(|| anyhow::anyhow!("Not authenticated"))?;
        
        let url = format!("https://cloud.cartographer.example/dashboard?agent={}", creds.agent_id);
        webbrowser::open(&url)
            .context("Failed to open dashboard in browser")
    }
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
pub struct TokenResponse {
    pub access_token: String,
    pub agent_id: String,
    pub user_email: String,
    pub expires_in: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ScanUpload {
    timestamp: String,
    devices: Vec<ScanDevice>,
}

#[derive(Debug, Serialize)]
struct ScanDevice {
    ip: String,
    mac: Option<String>,
    response_time_ms: Option<f64>,
    hostname: Option<String>,
}

