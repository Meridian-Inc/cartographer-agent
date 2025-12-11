// Prevents additional console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod auth;
mod cli;
mod cloud;
mod commands;
mod platform;
mod scanner;
mod scheduler;
// mod tray; // TODO: Re-enable when system tray plugin is available

use tauri::Manager;
use tracing::info;

fn main() {
    // Check for CLI mode
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && (args[1] == "--headless" || args[1] == "login" || 
        args[1] == "scan" || args[1] == "status" || args[1] == "logout") {
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

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // .plugin(tauri_plugin_notification::init()) // TODO: Re-enable when notifications are implemented
        .setup(|app| {
            // Initialize scheduler
            scheduler::init(app.handle().clone());
            
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

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::check_auth_status,
            commands::start_login_flow,
            commands::logout,
            commands::scan_network,
            commands::get_agent_status,
            commands::set_scan_interval,
            commands::get_network_info,
            commands::open_cloud_dashboard,
            commands::set_start_at_login,
            commands::get_start_at_login,
            commands::set_notifications_enabled,
            commands::get_notifications_enabled,
            commands::check_npcap_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

