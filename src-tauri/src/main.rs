// Prevents additional console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Cartographer Desktop Agent
//!
//! This is the GUI desktop agent built with Tauri. It uses local implementations
//! of auth, cloud, and scanner modules that are integrated with Tauri's event system.
//!
//! For the lightweight CLI-only agent (used on headless Linux servers), see
//! `packages/cartographer-cli` which uses `packages/cartographer-core` for
//! shared functionality without any GUI dependencies.
//!
//! The core library (`cartographer-core`) can be used here for shared types
//! when needed, but the main functionality uses local Tauri-integrated modules.

mod auth;
mod cli;
mod cloud;
mod commands;
mod persistence;
mod platform;
mod scanner;
mod scheduler;
mod tray;
mod updater;

// Re-export core types for compatibility with CLI
// These can be used when sharing data structures between desktop and CLI agents
#[allow(unused_imports)]
use cartographer_core as core;

use tauri::Manager;
use tracing::info;

fn main() {
    // Check for CLI mode
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1
        && (args[1] == "--headless"
            || args[1] == "login"
            || args[1] == "scan"
            || args[1] == "status"
            || args[1] == "logout")
    {
        // Run in CLI mode
        tauri::async_runtime::block_on(cli::run_cli());
        return;
    }
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cartographer_agent=info".into()),
        )
        .init();

    info!("Starting Cartographer Agent");

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init());

    // Enable updater in release builds
    #[cfg(not(debug_assertions))]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
        info!("Updater plugin enabled");
    }

    builder.setup(|app| {
            // Initialize scheduler (loads persisted state)
            scheduler::init(app.handle().clone());

            // Create system tray
            if let Err(e) = tray::create_tray(app.handle()) {
                tracing::error!("Failed to create system tray: {}", e);
            }

            // Check if we should start hidden (e.g., after a background update)
            if persistence::take_restart_hidden() {
                info!("Starting hidden after background update");
                tray::hide_main_window(app.handle());
            }

            // Set up close handler to minimize to tray instead of quitting
            if let Some(main_window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // Prevent the window from closing
                        api.prevent_close();
                        // Hide to tray (also hides from Dock on macOS)
                        tray::hide_main_window(&app_handle);
                        tracing::info!("Window hidden to tray");
                    }
                });
            } else {
                tracing::warn!("Main window not found during setup");
            }

            // Check if user is authenticated on startup
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(status) = commands::check_auth_status().await {
                    if status.authenticated {
                        info!("User is authenticated, starting background scanning");
                        scheduler::start_background_scanning(handle.clone()).await;
                    }
                }
            });

            // Check for silent update completion and notify frontend
            // Delay slightly to ensure frontend event listeners are ready
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                updater::check_and_emit_silent_update(&update_handle);
            });

            // Start background update checker in release builds
            #[cfg(not(debug_assertions))]
            updater::start_update_checker(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_auth_status,
            commands::start_login_flow,
            commands::request_login,
            commands::complete_login,
            commands::logout,
            commands::scan_network,
            commands::cancel_scan,
            commands::get_agent_status,
            commands::get_devices,
            commands::set_scan_interval,
            commands::get_network_info,
            commands::get_app_version,
            commands::open_cloud_dashboard,
            commands::set_start_at_login,
            commands::get_start_at_login,
            commands::set_notifications_enabled,
            commands::get_notifications_enabled,
            commands::run_health_check,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

