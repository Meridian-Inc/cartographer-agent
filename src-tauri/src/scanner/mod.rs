mod arp;
mod ping;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Windows flag to hide console window when spawning processes
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Create a Command that hides the console window on Windows.
/// On other platforms, this just creates a normal Command.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub ip: String,
    pub mac: Option<String>,
    pub response_time_ms: Option<f64>,
    pub hostname: Option<String>,
}

pub async fn scan_network() -> Result<Vec<Device>> {
    // Get network interface and subnet
    let (_interface, subnet) = get_network_info_internal().await?;
    
    tracing::info!("Scanning network: {}", subnet);
    
    // First, get devices from the ARP table (already known devices)
    let mut devices = arp::get_arp_table().await.unwrap_or_default();
    tracing::info!("ARP table has {} entries", devices.len());
    
    // Then do a ping sweep to discover new devices
    match ping::ping_sweep(&subnet).await {
        Ok(pinged_devices) => {
            tracing::info!("Ping sweep found {} responding hosts", pinged_devices.len());
            
            // Merge ping results with ARP data
            for pinged in pinged_devices {
                // Check if we already have this IP from ARP
                if let Some(existing) = devices.iter_mut().find(|d| d.ip == pinged.ip) {
                    // Update with ping response time
                    existing.response_time_ms = pinged.response_time_ms;
                } else {
                    // Add new device from ping
                    devices.push(pinged);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Ping sweep failed: {}", e);
        }
    }
    
    // Note: Hostname resolution is skipped during scan to avoid spawning
    // many external processes (one per device). Hostnames can be resolved
    // lazily in the UI or via a background task if needed.
    
    Ok(devices)
}

pub async fn get_network_info() -> Result<String> {
    let (interface, subnet) = get_network_info_internal().await?;
    Ok(format!("{} ({})", subnet, interface))
}

async fn get_network_info_internal() -> Result<(String, String)> {
    #[cfg(target_os = "windows")]
    {
        get_windows_network_info().await
    }
    
    #[cfg(target_os = "linux")]
    {
        get_linux_network_info().await
    }
    
    #[cfg(target_os = "macos")]
    {
        get_macos_network_info().await
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("Unsupported platform"))
    }
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info() -> Result<(String, String)> {
    // Get default gateway and interface using route print
    let output = hidden_command("powershell")
        .args(["-Command", r#"
            $adapter = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -ne $null } | Select-Object -First 1
            if ($adapter) {
                $ip = $adapter.IPv4Address.IPAddress
                $prefix = $adapter.IPv4Address.PrefixLength
                $iface = $adapter.InterfaceAlias
                # Calculate network address
                $ipBytes = [System.Net.IPAddress]::Parse($ip).GetAddressBytes()
                $maskInt = [uint32](0xFFFFFFFF -shl (32 - $prefix))
                $maskBytes = [BitConverter]::GetBytes($maskInt)
                [Array]::Reverse($maskBytes)
                $networkBytes = @()
                for ($i = 0; $i -lt 4; $i++) {
                    $networkBytes += $ipBytes[$i] -band $maskBytes[$i]
                }
                $network = [System.Net.IPAddress]::new($networkBytes)
                Write-Output "$iface|$network/$prefix"
            }
        "#])
        .output()
        .context("Failed to run PowerShell command")?;
    
    let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    
    if output_str.contains('|') {
        let parts: Vec<&str> = output_str.split('|').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    
    // Fallback to common defaults
    Ok(("Ethernet".to_string(), "192.168.1.0/24".to_string()))
}

#[cfg(target_os = "linux")]
async fn get_linux_network_info() -> Result<(String, String)> {
    // Get default interface
    let route_output = hidden_command("ip")
        .args(["route", "show", "default"])
        .output()
        .context("Failed to run ip route command")?;
    
    let route_str = String::from_utf8_lossy(&route_output.stdout);
    let interface = route_str
        .split_whitespace()
        .skip_while(|&s| s != "dev")
        .nth(1)
        .unwrap_or("eth0")
        .to_string();
    
    // Get IP and subnet for the interface
    let addr_output = hidden_command("ip")
        .args(["addr", "show", &interface])
        .output()
        .context("Failed to run ip addr command")?;
    
    let addr_str = String::from_utf8_lossy(&addr_output.stdout);
    
    // Parse inet line to get IP/prefix
    for line in addr_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("inet ") && !trimmed.contains("127.0.0.1") {
            if let Some(cidr) = trimmed.split_whitespace().nth(1) {
                // Convert IP/prefix to network/prefix
                if let Ok(network) = cidr.parse::<ipnetwork::IpNetwork>() {
                    let subnet = format!("{}/{}", network.network(), network.prefix());
                    return Ok((interface, subnet));
                }
            }
        }
    }
    
    Ok((interface, "192.168.1.0/24".to_string()))
}

#[cfg(target_os = "macos")]
async fn get_macos_network_info() -> Result<(String, String)> {
    // Get default interface
    let route_output = hidden_command("route")
        .args(["-n", "get", "default"])
        .output()
        .context("Failed to run route command")?;
    
    let route_str = String::from_utf8_lossy(&route_output.stdout);
    let interface = route_str
        .lines()
        .find(|line| line.contains("interface:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "en0".to_string());
    
    // Get IP and subnet for the interface
    let ifconfig_output = hidden_command("ifconfig")
        .arg(&interface)
        .output()
        .context("Failed to run ifconfig command")?;
    
    let ifconfig_str = String::from_utf8_lossy(&ifconfig_output.stdout);
    
    // Parse inet line
    for line in ifconfig_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("inet ") && !trimmed.contains("127.0.0.1") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if let (Some(ip), Some(mask)) = (parts.get(1), parts.get(3)) {
                // Convert hex mask to prefix length
                if let Ok(mask_int) = u32::from_str_radix(mask.trim_start_matches("0x"), 16) {
                    let prefix = mask_int.count_ones();
                    let ip_addr: std::net::Ipv4Addr = ip.parse().unwrap_or([192,168,1,1].into());
                    let mask_addr = std::net::Ipv4Addr::from(mask_int);
                    let network_int = u32::from(ip_addr) & u32::from(mask_addr);
                    let network = std::net::Ipv4Addr::from(network_int);
                    return Ok((interface, format!("{}/{}", network, prefix)));
                }
            }
        }
    }
    
    Ok((interface, "192.168.1.0/24".to_string()))
}

/// Resolve hostname for an IP address using system commands.
/// Currently unused during scans to avoid spawning many processes,
/// but kept for potential future lazy resolution.
#[allow(dead_code)]
async fn resolve_hostname(ip: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // Use nbtstat for NetBIOS name resolution
        let output = hidden_command("nbtstat")
            .args(["-A", ip])
            .output()
            .ok()?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        for line in output_str.lines() {
            let trimmed = line.trim();
            if trimmed.contains("<00>") && trimmed.contains("UNIQUE") {
                return trimmed.split_whitespace().next().map(|s| s.to_string());
            }
        }
    }
    
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Try reverse DNS lookup
        let output = hidden_command("host")
            .arg(ip)
            .output()
            .ok()?;
        
        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Parse "X.X.X.X.in-addr.arpa domain name pointer hostname."
            if let Some(hostname) = output_str.split("pointer").nth(1) {
                let hostname = hostname.trim().trim_end_matches('.');
                if !hostname.is_empty() {
                    return Some(hostname.to_string());
                }
            }
        }
    }
    
    None
}
