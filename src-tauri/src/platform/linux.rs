use anyhow::{Context, Result};
use std::path::PathBuf;

pub fn set_start_at_login(enabled: bool) -> Result<()> {
    let autostart_dir = get_autostart_dir()?;
    std::fs::create_dir_all(&autostart_dir)
        .context("Failed to create autostart directory")?;
    
    let desktop_file = autostart_dir.join("cartographer-agent.desktop");
    
    if enabled {
        let exe_path = std::env::current_exe()
            .context("Failed to get current executable path")?;
        
        let desktop_content = format!(
            r#"[Desktop Entry]
Type=Application
Name=Cartographer Agent
Exec={}
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
"#,
            exe_path.to_string_lossy()
        );
        
        std::fs::write(&desktop_file, desktop_content)
            .context("Failed to write desktop file")?;
    } else {
        if desktop_file.exists() {
            std::fs::remove_file(&desktop_file)
                .context("Failed to remove desktop file")?;
        }
    }
    
    Ok(())
}

pub fn get_start_at_login() -> Result<bool> {
    let autostart_dir = get_autostart_dir()?;
    let desktop_file = autostart_dir.join("cartographer-agent.desktop");
    Ok(desktop_file.exists())
}

fn get_autostart_dir() -> Result<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .context("Failed to get HOME directory")
        })?;
    
    Ok(config_home.join("autostart"))
}

