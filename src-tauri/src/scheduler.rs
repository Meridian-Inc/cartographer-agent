use crate::auth::check_auth;
use crate::cloud::CloudClient;
use crate::commands::SCAN_PROGRESS_EVENT;
use crate::persistence;
use crate::scanner::{check_device_reachable, get_arp_table_ips, scan_network_with_progress, Device, ScanProgress};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Event name for health check progress updates
pub const HEALTH_CHECK_PROGRESS_EVENT: &str = "health-check-progress";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthCheckProgress {
    pub stage: HealthCheckStage,
    pub message: String,
    pub total_devices: usize,
    pub checked_devices: usize,
    pub healthy_devices: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthCheckStage {
    Starting,
    CheckingDevices,
    Uploading,
    Complete,
}

static SCAN_INTERVAL: AtomicU64 = AtomicU64::new(300); // Default 5 minutes
static HEALTH_CHECK_INTERVAL: AtomicU64 = AtomicU64::new(60); // Default 1 minute

// Track if background tasks are already running
static BACKGROUND_RUNNING: AtomicBool = AtomicBool::new(false);

// Track if a network scan is currently in progress
static SCANNING_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

// Flag to request scan cancellation
static SCAN_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

// Cached list of known devices for health checks
static KNOWN_DEVICES: Mutex<Vec<Device>> = Mutex::const_new(Vec::new());

// Last scan timestamp (Unix timestamp in seconds)
static LAST_SCAN_TIME: AtomicU64 = AtomicU64::new(0);

// Global AppHandle for starting background tasks from anywhere
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub fn init(app: AppHandle) {
    APP_HANDLE.set(app).ok();

    // Load persisted state
    if let Ok(state) = persistence::load_state() {
        LAST_SCAN_TIME.store(state.last_scan_time, Ordering::Relaxed);
        if state.scan_interval_minutes > 0 {
            SCAN_INTERVAL.store(state.scan_interval_minutes * 60, Ordering::Relaxed);
        }
        if state.health_check_interval_seconds > 0 {
            HEALTH_CHECK_INTERVAL.store(state.health_check_interval_seconds, Ordering::Relaxed);
        }
        // Load devices into memory (spawn async task)
        if !state.devices.is_empty() {
            let devices = state.devices;
            tauri::async_runtime::spawn(async move {
                let mut known = KNOWN_DEVICES.lock().await;
                *known = devices;
            });
        }
    }

    tracing::info!("Scheduler initialized");
}

/// Get the stored AppHandle
pub fn get_app_handle() -> Option<AppHandle> {
    APP_HANDLE.get().cloned()
}

/// Start background scanning if not already running (can be called from anywhere)
pub async fn ensure_background_scanning() {
    if let Some(app) = get_app_handle() {
        start_background_scanning(app).await;
    }
}

/// Trigger an immediate full scan and health check in the background.
/// This returns immediately and does not block the caller.
/// Use this when reconnecting to cloud after logout.
pub fn trigger_immediate_scan() {
    if is_scanning() {
        tracing::debug!("Scan already in progress, skipping immediate scan");
        return;
    }

    if let Some(app) = get_app_handle() {
        tracing::info!("Triggering immediate scan (reconnect to cloud)");
        // Spawn in background so we don't block the login flow
        tokio::spawn(async move {
            run_initial_scan_sequence(&app).await;
        });
    }
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

/// Merge new devices with existing ones, preserving health data from previous health checks.
/// For devices that exist in both lists:
/// - If the new device has no response_time_ms, preserve the old one
/// - If the new device has response_time_ms, use the new value
/// New devices are added, old devices not in the new list are removed.
pub async fn merge_devices_preserving_health(new_devices: Vec<Device>) {
    let mut known = KNOWN_DEVICES.lock().await;
    
    // Create a map of IP -> old device for quick lookup
    let old_device_map: std::collections::HashMap<String, &Device> = 
        known.iter().map(|d| (d.ip.clone(), d)).collect();
    
    // Merge: for each new device, preserve old health data if new doesn't have it
    let merged: Vec<Device> = new_devices
        .into_iter()
        .map(|mut new_device| {
            if let Some(old_device) = old_device_map.get(&new_device.ip) {
                // If new device has no response time data (or is 0 from ARP), 
                // preserve old health data
                if new_device.response_time_ms.is_none() || new_device.response_time_ms == Some(0.0) {
                    if old_device.response_time_ms.is_some() {
                        new_device.response_time_ms = old_device.response_time_ms;
                    }
                }
                // Also preserve hostname if new doesn't have one
                if new_device.hostname.is_none() && old_device.hostname.is_some() {
                    new_device.hostname = old_device.hostname.clone();
                }
            }
            new_device
        })
        .collect();
    
    *known = merged;
}

/// Get current known devices
pub async fn get_known_devices() -> Vec<Device> {
    KNOWN_DEVICES.lock().await.clone()
}

/// Record that a scan just completed and persist to disk
pub fn record_scan_time() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    LAST_SCAN_TIME.store(now, Ordering::Relaxed);
}

/// Persist current state to disk (call after scans)
pub async fn persist_state() {
    let devices = get_known_devices().await;
    let scan_time = LAST_SCAN_TIME.load(Ordering::Relaxed);

    if let Err(e) = persistence::save_scan_results(&devices, scan_time) {
        tracing::warn!("Failed to persist state: {}", e);
    }
}

/// Get the last scan time as ISO string, or None if never scanned
pub fn get_last_scan_time() -> Option<String> {
    let timestamp = LAST_SCAN_TIME.load(Ordering::Relaxed);
    if timestamp == 0 {
        return None;
    }
    let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)?;
    Some(datetime.to_rfc3339())
}

/// Check if background tasks are running
pub fn is_background_running() -> bool {
    BACKGROUND_RUNNING.load(Ordering::Relaxed)
}

/// Check if a network scan is currently in progress
pub fn is_scanning() -> bool {
    SCANNING_IN_PROGRESS.load(Ordering::Relaxed)
}

/// Request cancellation of the current scan
pub fn request_scan_cancel() {
    SCAN_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    tracing::info!("Scan cancellation requested");
}

/// Check if scan cancellation has been requested
pub fn is_scan_cancelled() -> bool {
    SCAN_CANCEL_REQUESTED.load(Ordering::Relaxed)
}

/// Clear the cancellation flag (call when starting a new scan)
pub fn clear_scan_cancel() {
    SCAN_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

/// Helper to run a single scan and upload
async fn run_scan_and_upload(app: &AppHandle) {
    tracing::info!("Running network scan");

    // Mark scan as in progress
    SCANNING_IN_PROGRESS.store(true, Ordering::SeqCst);

    // Create progress callback that emits Tauri events
    let app_clone = app.clone();
    let progress_callback: Box<dyn Fn(ScanProgress) + Send + Sync> = Box::new(move |progress| {
        if let Err(e) = app_clone.emit(SCAN_PROGRESS_EVENT, &progress) {
            tracing::warn!("Failed to emit scan progress event: {}", e);
        }
    });

    match scan_network_with_progress(Some(progress_callback)).await {
        Ok(scan_result) => {
            let device_count = scan_result.devices.len();
            tracing::info!(
                "Scan found {} devices (gateway: {:?})",
                device_count,
                scan_result.network_info.gateway_ip
            );

            // Record scan time
            record_scan_time();

            // Merge new devices with existing ones, preserving health data
            merge_devices_preserving_health(scan_result.devices.clone()).await;

            // Persist to disk
            persist_state().await;

            // Upload to cloud if authenticated
            if let Ok(status) = check_auth().await {
                if status.authenticated {
                    let client = CloudClient::new();
                    if let Err(e) = client.upload_scan_result(&scan_result).await {
                        tracing::warn!("Failed to upload scan to cloud: {}", e);
                    } else {
                        tracing::info!("Scan synced to cloud");
                    }
                }
            }

            // Emit event for UI update
            if let Err(e) = app.emit("scan-complete", device_count) {
                tracing::warn!("Failed to emit scan-complete event: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("Scan failed: {}", e);
        }
    }

    // Mark scan as complete
    SCANNING_IN_PROGRESS.store(false, Ordering::SeqCst);
}

/// Helper to run health checks and upload
async fn run_health_checks_and_upload(app: &AppHandle) {
    let mut devices = get_known_devices().await;
    if devices.is_empty() {
        return;
    }

    tracing::debug!("Running health checks on {} devices", devices.len());

    // Get current ARP table for fallback checking
    // This helps detect devices that block ICMP but are still on the network
    let arp_ips = get_arp_table_ips().await;
    tracing::debug!("ARP table has {} entries for fallback", arp_ips.len());

    // Check all known devices using ping with ARP fallback
    // Update devices in-place to ensure response_time_ms is updated
    let mut health_results = Vec::new();
    for device in &mut devices {
        let result = check_device_reachable(&device.ip, &arp_ips).await;
        let reachable = result.is_ok();
        let response_time = if reachable { result.ok() } else { None };
        
        // Update the device's response time
        device.response_time_ms = response_time;
        
        health_results.push(DeviceHealthResult {
            ip: device.ip.clone(),
            reachable,
            response_time_ms: response_time,
        });
    }

    // Update in-memory devices with updated health data
    update_known_devices(devices).await;

    // Persist to disk
    persist_state().await;

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
    if let Err(e) = app.emit("health-check-complete", (healthy_count, health_results.len())) {
        tracing::debug!("Failed to emit health-check-complete event: {}", e);
    }
}

/// Helper to run health checks with progress events
async fn run_health_checks_with_progress(app: &AppHandle) {
    let mut devices = get_known_devices().await;
    let total = devices.len();

    if total == 0 {
        // Emit complete event even with no devices
        let _ = app.emit(
            HEALTH_CHECK_PROGRESS_EVENT,
            HealthCheckProgress {
                stage: HealthCheckStage::Complete,
                message: "No devices to check".to_string(),
                total_devices: 0,
                checked_devices: 0,
                healthy_devices: 0,
            },
        );
        return;
    }

    tracing::info!("Running health checks with progress on {} devices", total);

    // Emit starting event
    let _ = app.emit(
        HEALTH_CHECK_PROGRESS_EVENT,
        HealthCheckProgress {
            stage: HealthCheckStage::Starting,
            message: format!("Checking {} devices...", total),
            total_devices: total,
            checked_devices: 0,
            healthy_devices: 0,
        },
    );

    // Get current ARP table for fallback checking
    let arp_ips = get_arp_table_ips().await;

    // Check all known devices using ping with ARP fallback
    // Update devices in-place to ensure response_time_ms is updated
    let mut health_results = Vec::new();
    let mut healthy_count = 0;

    for (i, device) in devices.iter_mut().enumerate() {
        let result = check_device_reachable(&device.ip, &arp_ips).await;
        let reachable = result.is_ok();
        let response_time = if reachable { result.ok() } else { None };
        
        if reachable {
            healthy_count += 1;
        }
        
        // Update the device's response time
        device.response_time_ms = response_time;
        
        health_results.push(DeviceHealthResult {
            ip: device.ip.clone(),
            reachable,
            response_time_ms: response_time,
        });

        // Emit progress every device
        let _ = app.emit(
            HEALTH_CHECK_PROGRESS_EVENT,
            HealthCheckProgress {
                stage: HealthCheckStage::CheckingDevices,
                message: format!("Checking {}...", device.ip),
                total_devices: total,
                checked_devices: i + 1,
                healthy_devices: healthy_count,
            },
        );
    }

    // Update in-memory devices with updated health data
    update_known_devices(devices).await;

    // Persist to disk
    persist_state().await;

    // Emit uploading event
    let _ = app.emit(
        HEALTH_CHECK_PROGRESS_EVENT,
        HealthCheckProgress {
            stage: HealthCheckStage::Uploading,
            message: "Syncing results to cloud...".to_string(),
            total_devices: total,
            checked_devices: total,
            healthy_devices: healthy_count,
        },
    );

    // Upload health results to cloud if authenticated
    if let Ok(status) = check_auth().await {
        if status.authenticated {
            let client = CloudClient::new();
            if let Err(e) = client.upload_health_check(&health_results).await {
                tracing::debug!("Failed to upload health check: {}", e);
            }
        }
    }

    // Emit complete event
    let _ = app.emit(
        HEALTH_CHECK_PROGRESS_EVENT,
        HealthCheckProgress {
            stage: HealthCheckStage::Complete,
            message: format!(
                "Health check complete: {} healthy, {} unreachable",
                healthy_count,
                total - healthy_count
            ),
            total_devices: total,
            checked_devices: total,
            healthy_devices: healthy_count,
        },
    );

    // Also emit the legacy event for compatibility
    if let Err(e) = app.emit("health-check-complete", (healthy_count, total)) {
        tracing::debug!("Failed to emit health-check-complete event: {}", e);
    }
}

/// Run initial scan sequence: full scan followed by immediate health check
async fn run_initial_scan_sequence(app: &AppHandle) {
    tracing::info!("Running initial connection scan sequence");

    // Run full network scan (emits scan-progress events)
    run_scan_and_upload(app).await;

    // Immediately run health check after scan completes
    tracing::info!("Full scan complete, starting health check");
    run_health_checks_with_progress(app).await;
}

pub async fn start_background_scanning(app: AppHandle) {
    // Prevent starting multiple times
    if BACKGROUND_RUNNING.swap(true, Ordering::SeqCst) {
        tracing::debug!("Background scanning already running");
        return;
    }

    tracing::info!("Starting background scanning");

    let app_scan = app.clone();

    // Spawn background scan task
    tokio::spawn(async move {
        // Run initial scan sequence (full scan + health check)
        run_initial_scan_sequence(&app_scan).await;

        // Then run on interval
        let mut last_interval = SCAN_INTERVAL.load(Ordering::Relaxed);
        let mut scan_timer = interval(Duration::from_secs(last_interval));
        // Skip the first immediate tick since we just ran
        scan_timer.tick().await;

        loop {
            scan_timer.tick().await;

            // Check if interval changed
            let current_interval = SCAN_INTERVAL.load(Ordering::Relaxed);
            if current_interval != last_interval {
                last_interval = current_interval;
                scan_timer = interval(Duration::from_secs(current_interval));
                scan_timer.tick().await; // Skip immediate tick
                continue;
            }

            run_scan_and_upload(&app_scan).await;
        }
    });

    // Spawn background health check task
    let app_health = app.clone();
    tokio::spawn(async move {
        // Wait for initial scan to complete before starting health checks
        tokio::time::sleep(Duration::from_secs(10)).await;

        let mut last_interval = HEALTH_CHECK_INTERVAL.load(Ordering::Relaxed);
        let mut health_timer = interval(Duration::from_secs(last_interval));
        // Skip the first immediate tick
        health_timer.tick().await;

        loop {
            health_timer.tick().await;

            // Check if interval changed
            let current_interval = HEALTH_CHECK_INTERVAL.load(Ordering::Relaxed);
            if current_interval != last_interval {
                last_interval = current_interval;
                health_timer = interval(Duration::from_secs(current_interval));
                health_timer.tick().await; // Skip immediate tick
                continue;
            }

            run_health_checks_with_progress(&app_health).await;
        }
    });
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceHealthResult {
    pub ip: String,
    pub reachable: bool,
    pub response_time_ms: Option<f64>,
}

