use crate::persistence;
use semver::Version;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

/// Event name for update available notification
pub const UPDATE_AVAILABLE_EVENT: &str = "update-available";

/// Event name for silent update completed notification
pub const SILENT_UPDATE_COMPLETED_EVENT: &str = "silent-update-completed";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailableEvent {
    pub version: String,
    pub body: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SilentUpdateCompletedEvent {
    pub version: String,
}

/// Start the background update checker
/// Checks for updates every hour
pub fn start_update_checker(app_handle: AppHandle) {
    let handle = app_handle.clone();
    tokio::spawn(async move {
        // Wait 5 minutes after startup before first check
        tokio::time::sleep(Duration::from_secs(300)).await;
        
        // Then check every hour
        let mut interval = interval(Duration::from_secs(3600));
        interval.tick().await; // Skip first tick since we already waited
        
        loop {
            interval.tick().await;
            if let Err(e) = check_for_updates(handle.clone()).await {
                warn!("Update check failed: {}", e);
            }
        }
    });
}

/// Check for available updates
async fn check_for_updates(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get the updater from the app handle using the UpdaterExt trait
    // This may fail if the updater is not properly configured (e.g., invalid pubkey)
    let updater = match app_handle.updater() {
        Ok(u) => u,
        Err(e) => {
            warn!("Updater not available (likely not configured): {}", e);
            return Ok(());
        }
    };
    
    // Check for updates
    match updater.check().await {
        Ok(Some(update)) => {
            let current_version = env!("CARGO_PKG_VERSION");
            let new_version = &update.version;
            
            info!("Update available: {} -> {}", current_version, new_version);
            
            // Check if this is a significant update (minor or major)
            let is_significant = match is_significant_update(current_version, new_version) {
                Ok(sig) => sig,
                Err(e) => {
                    warn!("Failed to parse version for significance check: {}", e);
                    true // Default to significant if parsing fails
                }
            };
            
            // Check if main window is visible
            let window_visible = is_main_window_visible(&app_handle);
            
            // Determine update behavior
            if !window_visible || !is_significant {
                // Silent update: window is hidden OR it's just a patch version
                info!("Performing silent update (window_visible={}, significant={})", window_visible, is_significant);

                // Download the update - returns bytes
                let mut downloaded = 0;
                let bytes = update.download(
                    |chunk_length, content_length| {
                        downloaded += chunk_length;
                        info!("Downloading update: {}/{:?} bytes", downloaded, content_length);
                    },
                    || {
                        info!("Update downloaded");
                    }
                ).await?;

                // Save the silent update flag before restarting so we can notify the user
                if let Err(e) = persistence::set_silent_update_version(new_version) {
                    warn!("Failed to save silent update flag: {}", e);
                }

                // Install with the downloaded bytes and restart
                info!("Installing update and restarting...");
                update.install(bytes)?;
            } else {
                // Significant update with window visible - prompt user
                info!("Significant update available, prompting user");
                
                let event = UpdateAvailableEvent {
                    version: new_version.to_string(),
                    body: update.body.clone(),
                    date: update.date.map(|d| d.to_string()),
                };
                
                if let Err(e) = app_handle.emit(UPDATE_AVAILABLE_EVENT, &event) {
                    error!("Failed to emit update-available event: {}", e);
                }
            }
        }
        Ok(None) => {
            // No update available
            info!("No updates available");
        }
        Err(e) => {
            warn!("Error checking for updates: {}", e);
        }
    }
    
    Ok(())
}

/// Check if the update is significant (minor or major version change)
fn is_significant_update(current: &str, new: &str) -> Result<bool, semver::Error> {
    let current_ver = Version::parse(current)?;
    let new_ver = Version::parse(new)?;
    
    // Significant if major or minor version changed
    Ok(current_ver.major != new_ver.major || current_ver.minor != new_ver.minor)
}

/// Check if the main window is currently visible
fn is_main_window_visible(app_handle: &AppHandle) -> bool {
    if let Some(window) = app_handle.get_webview_window("main") {
        window.is_visible().unwrap_or(false)
    } else {
        false
    }
}

/// Check if a silent update just completed and emit an event to notify the frontend.
/// This should be called on app startup after the frontend is ready.
pub fn check_and_emit_silent_update(app_handle: &AppHandle) {
    if let Some(version) = persistence::take_silent_update_version() {
        info!("Silent update to version {} completed, notifying frontend", version);

        let event = SilentUpdateCompletedEvent { version };

        if let Err(e) = app_handle.emit(SILENT_UPDATE_COMPLETED_EVENT, &event) {
            error!("Failed to emit silent-update-completed event: {}", e);
        }
    }
}
