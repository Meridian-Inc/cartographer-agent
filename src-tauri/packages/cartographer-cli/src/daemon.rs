//! Background daemon mode for continuous network scanning
//!
//! This module implements a background service that:
//! - Periodically scans the network
//! - Uploads results to Cartographer Cloud
//! - Handles graceful shutdown via SIGTERM/SIGINT

use anyhow::Result;
use cartographer_core::{auth, cloud, scanner};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Global flag for shutdown coordination
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Run the background scanning daemon
pub async fn run_daemon(interval_minutes: u64, foreground: bool) -> Result<()> {
    // Check authentication first
    let auth_status = auth::check_auth().await?;
    if !auth_status.authenticated {
        eprintln!("Error: Not connected to cloud.");
        eprintln!("Run 'cartographer connect' first to authenticate.");
        std::process::exit(1);
    }

    if !foreground {
        // For now, we always run in foreground
        // Proper daemonization would require fork() which complicates things
        // The recommended approach is to use systemd to manage the process
        tracing::info!(
            "Running in foreground mode. Use systemd to run as a background service."
        );
    }

    tracing::info!(
        "Starting daemon: scanning every {} minutes, connected to '{}'",
        interval_minutes,
        auth_status.network_name.unwrap_or_else(|| "cloud".to_string())
    );

    // Set up signal handlers
    let shutdown = Arc::new(AtomicBool::new(false));
    setup_signal_handlers(shutdown.clone());

    // Create cloud client (reuse for all requests)
    let client = cloud::CloudClient::new();

    // Run initial scan immediately
    tracing::info!("Running initial scan...");
    if let Err(e) = run_scan_and_upload(&client).await {
        tracing::error!("Initial scan failed: {}", e);
    }

    // Set up interval timer
    let mut scan_interval = interval(Duration::from_secs(interval_minutes * 60));
    // Skip the first tick since we just ran
    scan_interval.tick().await;

    // Main daemon loop
    loop {
        tokio::select! {
            _ = scan_interval.tick() => {
                // Check if shutdown was requested
                if shutdown.load(Ordering::Relaxed) {
                    tracing::info!("Shutdown requested, stopping daemon");
                    break;
                }

                // Check if still authenticated
                match auth::check_auth().await {
                    Ok(status) if status.authenticated => {
                        if let Err(e) = run_scan_and_upload(&client).await {
                            tracing::error!("Scan failed: {}", e);
                        }
                    }
                    Ok(_) => {
                        tracing::warn!("No longer authenticated, stopping daemon");
                        eprintln!("Authentication expired. Run 'cartographer connect' to reconnect.");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Auth check failed: {}", e);
                        // Continue running, might be temporary network issue
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received Ctrl+C, shutting down");
                break;
            }
        }
    }

    tracing::info!("Daemon stopped");
    Ok(())
}

/// Set up SIGTERM and SIGINT handlers for graceful shutdown
fn setup_signal_handlers(shutdown: Arc<AtomicBool>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        let shutdown_term = shutdown.clone();
        tokio::spawn(async move {
            let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
            sigterm.recv().await;
            tracing::info!("Received SIGTERM");
            shutdown_term.store(true, Ordering::SeqCst);
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        });

        let shutdown_int = shutdown.clone();
        tokio::spawn(async move {
            let mut sigint = signal(SignalKind::interrupt()).expect("Failed to register SIGINT handler");
            sigint.recv().await;
            tracing::info!("Received SIGINT");
            shutdown_int.store(true, Ordering::SeqCst);
            SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
        });
    }

    #[cfg(not(unix))]
    {
        // On non-Unix platforms, rely on tokio::signal::ctrl_c() in the main loop
        let _ = shutdown;
    }
}

/// Run a network scan and upload results to cloud
async fn run_scan_and_upload(client: &cloud::CloudClient) -> Result<()> {
    let start = std::time::Instant::now();

    tracing::info!("Starting network scan...");

    // Run scan without progress callback (daemon mode)
    let scan_result = scanner::scan_network().await?;

    let scan_duration = start.elapsed();
    tracing::info!(
        "Scan complete: {} devices found in {:.1}s",
        scan_result.devices.len(),
        scan_duration.as_secs_f64()
    );

    // Upload to cloud
    tracing::debug!("Uploading results to cloud...");
    client.upload_scan_result(&scan_result).await?;
    tracing::info!("Results synced to cloud");

    Ok(())
}

/// Check if shutdown has been requested (for use by other modules)
#[allow(dead_code)]
pub fn is_shutdown_requested() -> bool {
    SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}
