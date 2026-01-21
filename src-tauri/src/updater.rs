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
/// Checks immediately on startup, then every hour thereafter
pub fn start_update_checker(app_handle: AppHandle) {
    let handle = app_handle.clone();
    // Use tauri's async runtime - tokio::spawn panics if called during setup
    // because Tauri's runtime isn't fully initialized yet
    tauri::async_runtime::spawn(async move {
        // Brief delay to let the app fully initialize (window, tray, etc.)
        tokio::time::sleep(Duration::from_secs(10)).await;
        
        // Check immediately on startup
        info!("Running startup update check");
        if let Err(e) = check_for_updates(handle.clone()).await {
            warn!("Startup update check failed: {}", e);
        }
        
        // Then check every hour
        let mut interval = interval(Duration::from_secs(3600));
        
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
            
            // Determine update behavior based on version change type:
            // - Patch updates (x.y.Z): silent background update
            // - Minor/Major updates (x.Y.z or X.y.z): prompt user
            if !is_significant {
                // Silent update for patch versions
                info!("Patch update detected ({}), performing silent background update", new_version);

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

                // Check if the window is currently hidden (background update should stay hidden)
                let window_hidden = app_handle
                    .get_webview_window("main")
                    .map(|w| !w.is_visible().unwrap_or(true))
                    .unwrap_or(false);
                
                if let Err(e) = persistence::set_restart_hidden(window_hidden) {
                    warn!("Failed to save restart hidden flag: {}", e);
                }
                info!("Window hidden before restart: {}", window_hidden);

                // Install the update
                info!("Installing update...");
                update.install(bytes)?;
                
                // Explicitly restart the app to apply the update
                info!("Restarting app to apply update...");
                app_handle.restart();
            } else {
                // Minor/Major update - always prompt user regardless of window state
                info!("Significant update detected ({}), prompting user", new_version);
                
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
/// Returns true for minor/major bumps, false for patch-only updates
fn is_significant_update(current: &str, new: &str) -> Result<bool, semver::Error> {
    let current_ver = Version::parse(current)?;
    let new_ver = Version::parse(new)?;
    
    // Significant if major or minor version changed
    Ok(current_ver.major != new_ver.major || current_ver.minor != new_ver.minor)
}

/// Check if a silent update just completed and emit an event to notify the frontend.
/// This should be called on app startup after the frontend is ready.
/// If the window is hidden, this will defer the notification until the window becomes visible.
pub fn check_and_emit_silent_update(app_handle: &AppHandle) {
    // Check if there's a pending silent update notification
    let version = match persistence::get_silent_update_version() {
        Some(v) => v,
        None => return, // No pending notification
    };

    // Check if the window is currently visible
    let window_visible = app_handle
        .get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);

    if window_visible {
        // Window is visible, emit the notification now and clear the flag
        emit_silent_update_notification(app_handle, &version);
        let _ = persistence::take_silent_update_version(); // Clear the flag
    } else {
        // Window is hidden, set up a listener to emit when it becomes visible
        info!("Window is hidden, deferring silent update notification for version {}", version);
        setup_deferred_update_notification(app_handle.clone(), version);
    }
}

/// Emit the silent update completed notification to the frontend
fn emit_silent_update_notification(app_handle: &AppHandle, version: &str) {
    info!("Silent update to version {} completed, notifying frontend", version);

    let event = SilentUpdateCompletedEvent {
        version: version.to_string(),
    };

    if let Err(e) = app_handle.emit(SILENT_UPDATE_COMPLETED_EVENT, &event) {
        error!("Failed to emit silent-update-completed event: {}", e);
    }
}

/// Set up a listener to emit the silent update notification when the window becomes visible.
/// Times out after 5 minutes to prevent infinite polling if user never opens the window.
fn setup_deferred_update_notification(app_handle: AppHandle, version: String) {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Instant;

    let notified = Arc::new(AtomicBool::new(false));
    let notified_clone = notified.clone();

    // Maximum time to wait for window to become visible (5 minutes)
    const MAX_WAIT_DURATION: Duration = Duration::from_secs(5 * 60);

    // Poll for window visibility since we can't easily hook into window events from here
    tauri::async_runtime::spawn(async move {
        let start_time = Instant::now();

        loop {
            // Check every 500ms if the window is visible
            tokio::time::sleep(Duration::from_millis(500)).await;

            // Check if we've already notified
            if notified_clone.load(Ordering::SeqCst) {
                break;
            }

            // Check if we've exceeded the maximum wait time
            if start_time.elapsed() > MAX_WAIT_DURATION {
                info!("Deferred update notification timed out after 5 minutes, clearing flag");
                let _ = persistence::take_silent_update_version();
                break;
            }

            // Check if window is now visible
            let window_visible = app_handle
                .get_webview_window("main")
                .map(|w| w.is_visible().unwrap_or(false))
                .unwrap_or(false);

            if window_visible {
                // Clear the persisted flag first
                let _ = persistence::take_silent_update_version();

                // Emit the notification
                emit_silent_update_notification(&app_handle, &version);
                notified_clone.store(true, Ordering::SeqCst);
                break;
            }
        }
    });
}
