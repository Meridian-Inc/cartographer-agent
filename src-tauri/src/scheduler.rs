use crate::auth::check_auth;
use crate::cloud::CloudClient;
use crate::persistence;
use crate::scanner::{ping_device, scan_network, Device};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

static SCAN_INTERVAL: AtomicU64 = AtomicU64::new(300); // Default 5 minutes
static HEALTH_CHECK_INTERVAL: AtomicU64 = AtomicU64::new(60); // Default 1 minute

// Track if background tasks are already running
static BACKGROUND_RUNNING: AtomicBool = AtomicBool::new(false);

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

/// Helper to run a single scan and upload
async fn run_scan_and_upload(app: &AppHandle) {
    tracing::info!("Running network scan");

    match scan_network().await {
        Ok(devices) => {
            let device_count = devices.len();
            tracing::info!("Scan found {} devices", device_count);

            // Record scan time
            record_scan_time();

            // Update known devices for health checks
            update_known_devices(devices.clone()).await;

            // Persist to disk
            persist_state().await;

            // Upload to cloud if authenticated
            if let Ok(status) = check_auth().await {
                if status.authenticated {
                    let client = CloudClient::new();
                    if let Err(e) = client.upload_scan(&devices).await {
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
}

/// Helper to run health checks and upload
async fn run_health_checks_and_upload(app: &AppHandle) {
    let devices = get_known_devices().await;
    if devices.is_empty() {
        return;
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
    if let Err(e) = app.emit("health-check-complete", (healthy_count, health_results.len())) {
        tracing::debug!("Failed to emit health-check-complete event: {}", e);
    }
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
        // Run initial scan immediately
        run_scan_and_upload(&app_scan).await;

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

            run_health_checks_and_upload(&app_health).await;
        }
    });
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceHealthResult {
    pub ip: String,
    pub reachable: bool,
    pub response_time_ms: Option<f64>,
}

