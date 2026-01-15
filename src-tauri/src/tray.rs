//! System tray functionality for Cartographer Agent.
//!
//! Handles minimize to tray on close and tray menu actions.

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

/// Create the system tray icon and menu
pub fn create_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create menu items
    let show = MenuItem::with_id(app, "show", "Show Cartographer", true, None::<&str>)?;
    let scan = MenuItem::with_id(app, "scan", "Scan Now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    // Create menu
    let menu = Menu::with_items(app, &[&show, &scan, &quit])?;

    // Create tray icon
    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .menu_on_left_click(false)
        .tooltip("Cartographer Agent")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                show_main_window(app);
            }
            "scan" => {
                // Trigger scan via command
                let app_clone = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::commands::scan_network().await {
                        tracing::error!("Tray scan failed: {}", e);
                    }
                });
            }
            "quit" => {
                tracing::info!("Quit requested from tray");
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Show window on left click
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    tracing::info!("System tray created");
    Ok(())
}

/// Show and focus the main window
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Hide the main window (minimize to tray)
pub fn hide_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
