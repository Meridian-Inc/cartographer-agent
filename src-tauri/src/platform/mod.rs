#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "windows")]
pub use windows::{set_start_at_login, get_start_at_login};

#[cfg(target_os = "macos")]
pub use macos::{set_start_at_login, get_start_at_login};

#[cfg(target_os = "linux")]
pub use linux::{set_start_at_login, get_start_at_login};

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn set_start_at_login(_enabled: bool) -> Result<(), String> {
    Err("Unsupported platform".to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
pub fn get_start_at_login() -> Result<bool, String> {
    Err("Unsupported platform".to_string())
}

