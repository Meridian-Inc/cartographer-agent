//! Persistence module for saving and loading agent state.
//!
//! Stores scan data, device lists, and settings to survive app restarts.

use crate::scanner::Device;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const STATE_FILE: &str = "agent_state.json";

/// Persisted agent state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentState {
    /// Last scan timestamp (Unix seconds)
    pub last_scan_time: u64,
    /// Known devices from last scan
    pub devices: Vec<Device>,
    /// Scan interval in minutes
    pub scan_interval_minutes: u64,
    /// Health check interval in seconds
    pub health_check_interval_seconds: u64,
}

/// Get the path to the state file
fn get_state_path() -> Result<PathBuf> {
    let data_dir = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .context("Could not find data directory")?;

    let app_dir = data_dir.join("cartographer-agent");
    std::fs::create_dir_all(&app_dir).context("Failed to create app data directory")?;

    Ok(app_dir.join(STATE_FILE))
}

/// Load persisted state from disk
pub fn load_state() -> Result<AgentState> {
    let path = get_state_path()?;

    if !path.exists() {
        tracing::debug!("No state file found, using defaults");
        return Ok(AgentState::default());
    }

    let content = std::fs::read_to_string(&path).context("Failed to read state file")?;

    let state: AgentState =
        serde_json::from_str(&content).context("Failed to parse state file")?;

    tracing::info!(
        "Loaded state: {} devices, last scan: {}",
        state.devices.len(),
        if state.last_scan_time > 0 {
            chrono::DateTime::from_timestamp(state.last_scan_time as i64, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            "never".to_string()
        }
    );

    Ok(state)
}

/// Save state to disk
pub fn save_state(state: &AgentState) -> Result<()> {
    let path = get_state_path()?;

    let content = serde_json::to_string_pretty(state).context("Failed to serialize state")?;

    std::fs::write(&path, content).context("Failed to write state file")?;

    tracing::debug!("Saved state: {} devices", state.devices.len());

    Ok(())
}

/// Update just the devices and scan time
pub fn save_scan_results(devices: &[Device], scan_time: u64) -> Result<()> {
    let mut state = load_state().unwrap_or_default();
    state.devices = devices.to_vec();
    state.last_scan_time = scan_time;
    save_state(&state)
}

/// Get the stored last scan time
pub fn get_stored_last_scan_time() -> u64 {
    load_state().map(|s| s.last_scan_time).unwrap_or(0)
}

/// Get the stored devices
pub fn get_stored_devices() -> Vec<Device> {
    load_state().map(|s| s.devices).unwrap_or_default()
}

/// Update device health data (response times) after a health check
/// Returns the updated devices
pub fn update_device_health(health_results: &[(String, Option<f64>)]) -> Result<Vec<Device>> {
    let mut state = load_state().unwrap_or_default();

    for (ip, response_time) in health_results {
        if let Some(device) = state.devices.iter_mut().find(|d| &d.ip == ip) {
            device.response_time_ms = *response_time;
        }
    }

    save_state(&state)?;
    Ok(state.devices)
}

/// Clear all persisted state (devices, scan times, etc.)
/// Called during logout to remove all local data
pub fn clear_state() -> Result<()> {
    let path = get_state_path()?;
    
    if path.exists() {
        std::fs::remove_file(&path).context("Failed to delete state file")?;
        tracing::info!("Cleared persisted state file");
    }
    
    Ok(())
}
