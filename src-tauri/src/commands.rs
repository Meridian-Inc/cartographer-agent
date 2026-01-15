use crate::auth::{check_auth, logout as auth_logout, start_login};
use crate::cloud::CloudClient;
use crate::scanner::{check_device_reachable, get_arp_table_ips, ping_device, scan_network as scanner_scan_network, Device, ScanResult};
use crate::scheduler::{
    ensure_background_scanning, get_known_devices, get_last_scan_time, get_scan_interval,
    persist_state, record_scan_time, set_scan_interval as scheduler_set_scan_interval,
    update_known_devices, DeviceHealthResult,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let devices = get_known_devices().await;
    match check_auth().await {
        Ok(status) => Ok(AgentStatus {
            authenticated: status.authenticated,
            user_email: status.user_email,
            network_id: status.network_id,
            network_name: status.network_name,
            last_scan: get_last_scan_time(),
            next_scan: None,
            device_count: Some(devices.len()),
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn start_login_flow() -> Result<AgentStatus, String> {
    match start_login().await {
        Ok(status) => {
            // Start background scanning if authenticated
            if status.authenticated {
                tracing::info!("Login successful, starting background scanning");
                ensure_background_scanning().await;
            }
            Ok(AgentStatus {
                authenticated: status.authenticated,
                user_email: status.user_email,
                network_id: status.network_id,
                network_name: status.network_name,
                last_scan: get_last_scan_time(),
                next_scan: None,
                device_count: None,
            })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    auth_logout().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_network() -> Result<Vec<Device>, String> {
    let scan_result = scanner_scan_network().await.map_err(|e| format!("{}", e))?;

    tracing::info!(
        "Scan complete, found {} devices (gateway: {:?})",
        scan_result.devices.len(),
        scan_result.network_info.gateway_ip
    );

    // Record scan time
    record_scan_time();

    // Update known devices for health checks
    update_known_devices(scan_result.devices.clone()).await;

    // Persist to disk
    persist_state().await;

    // Upload to cloud if authenticated
    match check_auth().await {
        Ok(status) if status.authenticated => {
            tracing::info!(
                "Authenticated as {}, uploading to network '{}'",
                status.user_email.as_deref().unwrap_or("Unknown"),
                status.network_name.as_deref().unwrap_or("Unknown")
            );
            let client = get_cloud_client().await;
            if let Err(e) = client.upload_scan_result(&scan_result).await {
                tracing::warn!("Failed to upload scan to cloud: {}", e);
            }
        }
        Ok(_) => {
            tracing::info!("Not authenticated, skipping cloud upload");
        }
        Err(e) => {
            tracing::warn!("Failed to check auth status: {}", e);
        }
    }

    Ok(scan_result.devices)
}

#[tauri::command]
pub async fn get_agent_status() -> Result<AgentStatus, String> {
    let status = check_auth().await.map_err(|e| e.to_string())?;
    let devices = get_known_devices().await;

    Ok(AgentStatus {
        authenticated: status.authenticated,
        user_email: status.user_email,
        network_id: status.network_id,
        network_name: status.network_name,
        last_scan: get_last_scan_time(),
        next_scan: None,
        device_count: Some(devices.len()),
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckStatus {
    pub total_devices: usize,
    pub healthy_devices: usize,
    pub unreachable_devices: usize,
    pub synced_to_cloud: bool,
}

#[tauri::command]
pub async fn run_health_check() -> Result<HealthCheckStatus, String> {
    let devices = get_known_devices().await;

    if devices.is_empty() {
        return Err("No devices to check. Run a scan first.".to_string());
    }

    tracing::info!("Running manual health check on {} devices", devices.len());

    // Get current ARP table for fallback checking
    // This helps detect devices that block ICMP but are still on the network
    let arp_ips = get_arp_table_ips().await;
    tracing::debug!("ARP table has {} entries for fallback", arp_ips.len());

    // Check all known devices using ping with ARP fallback
    let mut health_results = Vec::new();
    for device in &devices {
        let result = check_device_reachable(&device.ip, &arp_ips).await;
        health_results.push(DeviceHealthResult {
            ip: device.ip.clone(),
            reachable: result.is_ok(),
            response_time_ms: result.ok(),
        });
    }

    let healthy_count = health_results.iter().filter(|r| r.reachable).count();
    let unreachable_count = health_results.len() - healthy_count;

    tracing::info!(
        "Health check complete: {} healthy, {} unreachable",
        healthy_count,
        unreachable_count
    );

    // Upload to cloud if authenticated
    let mut synced = false;
    match check_auth().await {
        Ok(status) if status.authenticated => {
            let client = get_cloud_client().await;
            match client.upload_health_check(&health_results).await {
                Ok(_) => {
                    tracing::info!("Health check results synced to cloud");
                    synced = true;
                }
                Err(e) => {
                    tracing::warn!("Failed to upload health check to cloud: {}", e);
                }
            }
        }
        Ok(_) => {
            tracing::info!("Not authenticated, skipping cloud upload");
        }
        Err(e) => {
            tracing::warn!("Failed to check auth status: {}", e);
        }
    }

    Ok(HealthCheckStatus {
        total_devices: health_results.len(),
        healthy_devices: healthy_count,
        unreachable_devices: unreachable_count,
        synced_to_cloud: synced,
    })
}

