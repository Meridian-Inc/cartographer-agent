use crate::auth::{check_auth, start_login, logout as auth_logout};
use crate::cloud::CloudClient;
use crate::scanner::{scan_network as scanner_scan_network, Device};
use crate::scheduler::{set_scan_interval as scheduler_set_scan_interval, get_scan_interval};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentStatus {
    pub authenticated: bool,
    pub user_email: Option<String>,
    pub network_id: Option<String>,
    pub network_name: Option<String>,
    pub last_scan: Option<String>,
    pub next_scan: Option<String>,
    pub device_count: Option<usize>,
}

static CLOUD_CLIENT: Mutex<Option<Arc<CloudClient>>> = Mutex::const_new(None);

async fn get_cloud_client() -> Arc<CloudClient> {
    let mut client = CLOUD_CLIENT.lock().await;
    if client.is_none() {
        *client = Some(Arc::new(CloudClient::new()));
    }
    client.as_ref().unwrap().clone()
}

#[tauri::command]
pub async fn check_auth_status() -> Result<AgentStatus, String> {
    match check_auth().await {
        Ok(status) => Ok(AgentStatus {
            authenticated: status.authenticated,
            user_email: status.user_email,
            network_id: status.network_id,
            network_name: status.network_name,
            last_scan: None,
            next_scan: None,
            device_count: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn start_login_flow() -> Result<AgentStatus, String> {
    match start_login().await {
        Ok(status) => Ok(AgentStatus {
            authenticated: status.authenticated,
            user_email: status.user_email,
            network_id: status.network_id,
            network_name: status.network_name,
            last_scan: None,
            next_scan: None,
            device_count: None,
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    auth_logout().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_network() -> Result<Vec<Device>, String> {
    let devices = scanner_scan_network().await.map_err(|e| format!("{}", e))?;
    
    // Upload to cloud if authenticated
    if let Ok(status) = check_auth().await {
        if status.authenticated {
            let client = get_cloud_client().await;
            if let Err(e) = client.upload_scan(&devices).await {
                tracing::warn!("Failed to upload scan to cloud: {}", e);
            }
        }
    }
    
    Ok(devices)
}

#[tauri::command]
pub async fn get_agent_status() -> Result<AgentStatus, String> {
    let status = check_auth().await.map_err(|e| e.to_string())?;
    let _interval = get_scan_interval();
    
    Ok(AgentStatus {
        authenticated: status.authenticated,
        user_email: status.user_email,
        network_id: status.network_id,
        network_name: status.network_name,
        last_scan: None,
        next_scan: None,
        device_count: None,
    })
}

#[tauri::command]
pub async fn set_scan_interval(minutes: u64) -> Result<(), String> {
    scheduler_set_scan_interval(minutes);
    Ok(())
}

#[tauri::command]
pub async fn get_network_info() -> Result<String, String> {
    crate::scanner::get_network_info().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_cloud_dashboard() -> Result<(), String> {
    let client = get_cloud_client().await;
    client.open_dashboard().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_start_at_login(enabled: bool) -> Result<(), String> {
    crate::platform::set_start_at_login(enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_start_at_login() -> Result<bool, String> {
    crate::platform::get_start_at_login().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_notifications_enabled(_enabled: bool) -> Result<(), String> {
    // Store in config file
    Ok(())
}

#[tauri::command]
pub async fn get_notifications_enabled() -> Result<bool, String> {
    // Read from config file
    Ok(true)
}

