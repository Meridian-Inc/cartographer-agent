use anyhow::{Context, Result};
use std::path::PathBuf;
use winreg::enums::*;
use winreg::RegKey;

pub fn set_start_at_login(enabled: bool) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let key = hkcu.open_subkey_with_flags(path, KEY_WRITE)
        .context("Failed to open registry key")?;
    
    let app_name = "CartographerAgent";
    let exe_path = std::env::current_exe()
        .context("Failed to get current executable path")?;
    
    if enabled {
        key.set_value(app_name, &exe_path.to_string_lossy().to_string())
            .context("Failed to set registry value")?;
    } else {
        let _ = key.delete_value(app_name);
    }
    
    Ok(())
}

pub fn get_start_at_login() -> Result<bool> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    
    match hkcu.open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => {
            let app_name = "CartographerAgent";
            Ok(key.get_value::<String, _>(app_name).is_ok())
        }
        Err(_) => Ok(false),
    }
}

