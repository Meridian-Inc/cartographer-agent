use anyhow::{Context, Result};
use std::process::Command;

pub fn set_start_at_login(enabled: bool) -> Result<()> {
    let app_path = std::env::current_exe()
        .context("Failed to get current executable path")?;
    
    // Use launchctl to manage login items
    // This is a simplified version - in production, use LaunchAgent plist files
    if enabled {
        // Create LaunchAgent plist
        let plist_path = get_launch_agent_path()?;
        std::fs::create_dir_all(plist_path.parent().unwrap())?;
        
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.cartographer.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#,
            app_path.to_string_lossy()
        );
        
        std::fs::write(&plist_path, plist_content)
            .context("Failed to write LaunchAgent plist")?;
        
        // Load the launch agent
        Command::new("launchctl")
            .args(&["load", &plist_path.to_string_lossy().to_string()])
            .output()
            .context("Failed to load LaunchAgent")?;
    } else {
        // Unload the launch agent
        let plist_path = get_launch_agent_path()?;
        if plist_path.exists() {
            let _ = Command::new("launchctl")
                .args(&["unload", &plist_path.to_string_lossy().to_string()])
                .output();
            let _ = std::fs::remove_file(&plist_path);
        }
    }
    
    Ok(())
}

pub fn get_start_at_login() -> Result<bool> {
    let plist_path = get_launch_agent_path()?;
    Ok(plist_path.exists())
}

fn get_launch_agent_path() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .context("Failed to get HOME directory")?;
    Ok(std::path::PathBuf::from(home)
        .join("Library/LaunchAgents/dev.cartographer.agent.plist"))
}

