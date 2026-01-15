use crate::auth::check_auth;
use crate::cloud::CloudClient;
use crate::scanner::{scan_network, ping_device, Device};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

static SCAN_INTERVAL: AtomicU64 = AtomicU64::new(300); // Default 5 minutes
static HEALTH_CHECK_INTERVAL: AtomicU64 = AtomicU64::new(60); // Default 1 minute

// Cached list of known devices for health checks
static KNOWN_DEVICES: Mutex<Vec<Device>> = Mutex::const_new(Vec::new());

pub fn init(_app: AppHandle) {
    tracing::info!("Scheduler initialized");
}

pub fn set_scan_interval(minutes: u64) {
    SCAN_INTERVAL.store(minutes * 60, Ordering::Relaxed);
    tracing::info!("Scan interval set to {} minutes", minutes);
}

pub fn get_scan_interval() -> u64 {
    SCAN_INTERVAL.load(Ordering::Relaxed) / 60
}

pub fn set_health_check_interval(seconds: u64) {
    HEALTH_CHECK_INTERVAL.store(seconds, Ordering::Relaxed);
    tracing::info!("Health check interval set to {} seconds", seconds);
}

pub fn get_health_check_interval() -> u64 {
    HEALTH_CHECK_INTERVAL.load(Ordering::Relaxed)
}

/// Update the list of known devices (called after successful scans)
pub async fn update_known_devices(devices: Vec<Device>) {
    let mut known = KNOWN_DEVICES.lock().await;
    *known = devices;
}

/// Get current known devices
pub async fn get_known_devices() -> Vec<Device> {
    KNOWN_DEVICES.lock().await.clone()
}

pub async fn start_background_scanning(app: AppHandle) {
    let scan_interval_secs = SCAN_INTERVAL.load(Ordering::Relaxed);
    let mut scan_timer = interval(Duration::from_secs(scan_interval_secs));
    
    let app_clone = app.clone();
    
    // Spawn background scan task
    tokio::spawn(async move {
        loop {
            scan_timer.tick().await;
            
            // Update interval if it changed
            let interval_secs = SCAN_INTERVAL.load(Ordering::Relaxed);
            scan_timer = interval(Duration::from_secs(interval_secs));
            
            tracing::info!("Running scheduled network scan");
            
            // Perform scan
            match scan_network().await {
                Ok(devices) => {
                    let device_count = devices.len();
                    tracing::info!("Scheduled scan found {} devices", device_count);
                    
                    // Update known devices for health checks
                    update_known_devices(devices.clone()).await;
                    
                    // Upload to cloud if authenticated
                    if let Ok(status) = check_auth().await {
                        if status.authenticated {
                            let client = CloudClient::new();
                            if let Err(e) = client.upload_scan(&devices).await {
                                tracing::warn!("Failed to upload scheduled scan to cloud: {}", e);
                            } else {
                                tracing::info!("Scheduled scan synced to cloud");
                            }
                        }
                    }
                    
                    // Emit event for UI update
                    if let Err(e) = app_clone.emit("scan-complete", device_count) {
                        tracing::warn!("Failed to emit scan-complete event: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Scheduled scan failed: {}", e);
                }
            }
        }
    });
    
    // Spawn background health check task
    let app_health = app.clone();
    tokio::spawn(async move {
        // Wait a bit before starting health checks
        tokio::time::sleep(Duration::from_secs(30)).await;
        
        let mut health_timer = interval(Duration::from_secs(HEALTH_CHECK_INTERVAL.load(Ordering::Relaxed)));
        
        loop {
            health_timer.tick().await;
            
            // Update interval if it changed
            let interval_secs = HEALTH_CHECK_INTERVAL.load(Ordering::Relaxed);
            health_timer = interval(Duration::from_secs(interval_secs));
            
            // Get known devices
            let devices = get_known_devices().await;
            if devices.is_empty() {
                continue;
            }
            
            tracing::debug!("Running health checks on {} devices", devices.len());
            
            // Ping all known devices
            let mut health_results = Vec::new();
            for device in &devices {
                let result = ping_device(&device.ip).await;
                health_results.push(DeviceHealthResult {
                    ip: device.ip.clone(),
                    reachable: result.is_ok(),
                    response_time_ms: result.ok(),
                });
            }
            
            // Upload health results to cloud if authenticated
            if let Ok(status) = check_auth().await {
                if status.authenticated {
                    let client = CloudClient::new();
                    if let Err(e) = client.upload_health_check(&health_results).await {
                        tracing::debug!("Failed to upload health check: {}", e);
                    }
                }
            }
            
            // Emit event for UI update
            let healthy_count = health_results.iter().filter(|r| r.reachable).count();
            if let Err(e) = app_health.emit("health-check-complete", (healthy_count, health_results.len())) {
                tracing::debug!("Failed to emit health-check-complete event: {}", e);
            }
        }
    });
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceHealthResult {
    pub ip: String,
    pub reachable: bool,
    pub response_time_ms: Option<f64>,
}

