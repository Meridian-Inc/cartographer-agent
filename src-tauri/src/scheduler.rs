use crate::scanner::scan_network;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::time::{interval, Duration};

static SCAN_INTERVAL: AtomicU64 = AtomicU64::new(300); // Default 5 minutes

pub fn init(_app: AppHandle) {
    // Initialize scheduler
    tracing::info!("Scheduler initialized");
}

pub fn set_scan_interval(minutes: u64) {
    SCAN_INTERVAL.store(minutes * 60, Ordering::Relaxed);
    tracing::info!("Scan interval set to {} minutes", minutes);
}

pub fn get_scan_interval() -> u64 {
    SCAN_INTERVAL.load(Ordering::Relaxed) / 60
}

pub async fn start_background_scanning(app: AppHandle) {
    let mut scan_timer = interval(Duration::from_secs(SCAN_INTERVAL.load(Ordering::Relaxed)));
    
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
                    tracing::info!("Scheduled scan found {} devices", devices.len());
                    
                    // Update tray icon if needed
                    if let Err(e) = app.emit("scan-complete", devices.len()) {
                        tracing::warn!("Failed to emit scan-complete event: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Scheduled scan failed: {}", e);
                }
            }
        }
    });
}

