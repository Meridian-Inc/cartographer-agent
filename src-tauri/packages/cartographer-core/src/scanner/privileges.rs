//! Privilege detection and escalation handling for network scanning
//!
//! Different platforms have different requirements for network scanning:
//! - Windows: ICMP ping works without admin, but some ARP operations may need elevation
//! - Linux: Raw socket access may require root/CAP_NET_RAW, but system ping usually has setuid
//! - macOS: ICMP ping works without root for most operations

use serde::{Deserialize, Serialize};

/// Scan mode indicating the level of access available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    /// Full access - all scan features available
    Full,
    /// Limited access - some features may be restricted
    Limited,
}

impl std::fmt::Display for ScanMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanMode::Full => write!(f, "full"),
            ScanMode::Limited => write!(f, "limited"),
        }
    }
}

/// Information about scan capabilities based on current privileges
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanCapabilities {
    pub mode: ScanMode,
    pub can_ping: bool,
    pub can_read_arp: bool,
    pub can_resolve_hostnames: bool,
    pub is_elevated: bool,
    pub warning: Option<String>,
    pub elevation_instructions: Option<String>,
}

impl Default for ScanCapabilities {
    fn default() -> Self {
        Self {
            mode: ScanMode::Full,
            can_ping: true,
            can_read_arp: true,
            can_resolve_hostnames: true,
            is_elevated: false,
            warning: None,
            elevation_instructions: None,
        }
    }
}

/// Check if the current process is running with elevated privileges
pub fn is_elevated() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_elevated_windows()
    }

    #[cfg(target_os = "linux")]
    {
        is_elevated_linux()
    }

    #[cfg(target_os = "macos")]
    {
        is_elevated_macos()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

#[cfg(target_os = "windows")]
fn is_elevated_windows() -> bool {
    use std::process::Command;

    match Command::new("whoami").args(["/groups"]).output() {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            output_str.contains("S-1-16-12288") || output_str.contains("High Mandatory Level")
        }
        Err(_) => false,
    }
}

#[cfg(target_os = "linux")]
fn is_elevated_linux() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(target_os = "macos")]
fn is_elevated_macos() -> bool {
    unsafe { libc::geteuid() == 0 }
}

/// Test if ping functionality works
pub async fn test_ping_capability() -> bool {
    use super::hidden_command_sync;
    use tokio::task::spawn_blocking;

    let result = spawn_blocking(|| {
        #[cfg(target_os = "windows")]
        let output = hidden_command_sync("ping")
            .args(["-n", "1", "-w", "500", "127.0.0.1"])
            .output();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let output = hidden_command_sync("ping")
            .args(["-c", "1", "-W", "1", "127.0.0.1"])
            .output();

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        let output: std::io::Result<std::process::Output> =
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Unsupported"));

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    })
    .await;

    result.unwrap_or(false)
}

/// Detect scan capabilities based on current privileges
pub async fn detect_capabilities() -> ScanCapabilities {
    let elevated = is_elevated();
    let can_ping = test_ping_capability().await;

    let can_read_arp = true;
    let can_resolve_hostnames = true;

    let mode = if can_ping {
        ScanMode::Full
    } else {
        ScanMode::Limited
    };

    let (warning, elevation_instructions) = if mode == ScanMode::Limited {
        let warning = Some(
            "Running with limited scan capabilities. Some devices may not be discovered."
                .to_string(),
        );
        let instructions = get_elevation_instructions();
        (warning, Some(instructions))
    } else {
        (None, None)
    };

    ScanCapabilities {
        mode,
        can_ping,
        can_read_arp,
        can_resolve_hostnames,
        is_elevated: elevated,
        warning,
        elevation_instructions,
    }
}

/// Get platform-specific instructions for running with elevated privileges
pub fn get_elevation_instructions() -> String {
    #[cfg(target_os = "windows")]
    {
        "To run with full scan capabilities on Windows:\n\
         1. Right-click on Cartographer Agent\n\
         2. Select 'Run as administrator'\n\
         \n\
         Note: Most scan features work without admin rights on Windows."
            .to_string()
    }

    #[cfg(target_os = "linux")]
    {
        "To run with full scan capabilities on Linux:\n\
         \n\
         Option 1 - Run as root (not recommended for regular use):\n\
         $ sudo cartographer\n\
         \n\
         Option 2 - Grant CAP_NET_RAW capability:\n\
         $ sudo setcap cap_net_raw+ep /usr/local/bin/cartographer\n\
         \n\
         Option 3 - Ensure the system ping has setuid (usually default):\n\
         $ ls -la /bin/ping  # Should show '-rwsr-xr-x'\n\
         \n\
         The agent uses the system ping command, which typically works\n\
         without elevation on most Linux distributions."
            .to_string()
    }

    #[cfg(target_os = "macos")]
    {
        "To run with full scan capabilities on macOS:\n\
         \n\
         Option 1 - Run as root (not recommended for regular use):\n\
         $ sudo cartographer\n\
         \n\
         Note: Most scan features work without root on macOS.\n\
         If you're experiencing issues, check System Preferences > Security & Privacy."
            .to_string()
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        "Elevated privileges may be required for full scan capabilities.\n\
         Please consult your operating system documentation."
            .to_string()
    }
}

/// Format a user-friendly message about current scan capabilities
pub fn format_capabilities_message(caps: &ScanCapabilities) -> String {
    if caps.mode == ScanMode::Full {
        "Scanning with full capabilities".to_string()
    } else {
        let mut msg = String::from("Scanning with limited capabilities:\n");

        if !caps.can_ping {
            msg.push_str("  - Ping sweep unavailable (will use ARP table only)\n");
        }
        if !caps.can_read_arp {
            msg.push_str("  - ARP table reading unavailable\n");
        }
        if !caps.can_resolve_hostnames {
            msg.push_str("  - Hostname resolution unavailable\n");
        }

        if let Some(ref instructions) = caps.elevation_instructions {
            msg.push_str("\nTo enable full scanning:\n");
            msg.push_str(instructions);
        }

        msg
    }
}
