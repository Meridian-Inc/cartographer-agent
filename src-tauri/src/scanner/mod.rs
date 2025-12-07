mod arp;
mod ping;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub ip: String,
    pub mac: Option<String>,
    pub response_time_ms: Option<f64>,
    pub hostname: Option<String>,
}

pub async fn scan_network() -> Result<Vec<Device>> {
    // Get network interface and subnet
    let (interface, subnet) = get_network_info_internal().await?;
    
    tracing::info!("Scanning network: {} on interface {}", subnet, interface);
    
    // Try ARP scan first (faster, more accurate)
    let devices = match arp::scan_subnet(&interface, &subnet).await {
        Ok(devices) => {
            tracing::info!("ARP scan found {} devices", devices.len());
            devices
        }
        Err(e) => {
            tracing::warn!("ARP scan failed: {}, falling back to ping sweep", e);
            // Fall back to ping sweep
            ping::ping_sweep(&subnet).await
                .context("Ping sweep failed")?
        }
    };
    
    // Resolve hostnames for discovered devices
    let mut devices_with_hostnames = Vec::new();
    for device in devices {
        let hostname = resolve_hostname(&device.ip).await;
        devices_with_hostnames.push(Device {
            ip: device.ip,
            mac: device.mac,
            response_time_ms: device.response_time_ms,
            hostname,
        });
    }
    
    Ok(devices_with_hostnames)
}

pub async fn get_network_info() -> Result<String> {
    let (interface, subnet) = get_network_info_internal().await?;
    Ok(format!("{} ({})", subnet, interface))
}

async fn get_network_info_internal() -> Result<(String, String)> {
    // Detect primary network interface and subnet
    // This is a simplified version - in production, use pnet to detect properly
    
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        let output = Command::new("ip")
            .args(&["route", "show", "default"])
            .output()
            .context("Failed to run ip command")?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        // Parse interface from output
        // This is simplified - real implementation would parse properly
        let interface = "eth0".to_string(); // Placeholder
        let subnet = "192.168.1.0/24".to_string(); // Placeholder
        
        Ok((interface, subnet))
    }
    
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("route")
            .args(&["-n", "get", "default"])
            .output()
            .context("Failed to run route command")?;
        
        // Parse output to get interface and subnet
        let interface = "en0".to_string(); // Placeholder
        let subnet = "192.168.1.0/24".to_string(); // Placeholder
        
        Ok((interface, subnet))
    }
    
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let output = Command::new("ipconfig")
            .output()
            .context("Failed to run ipconfig command")?;
        
        // Parse output to get interface and subnet
        let interface = "Ethernet".to_string(); // Placeholder
        let subnet = "192.168.1.0/24".to_string(); // Placeholder
        
        Ok((interface, subnet))
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("Unsupported platform"))
    }
}

async fn resolve_hostname(ip: &str) -> Option<String> {
    use std::net::ToSocketAddrs;
    
    // Try reverse DNS lookup
    if let Ok(addr) = ip.parse::<IpAddr>() {
        if let Ok(mut iter) = (addr, 0).to_socket_addrs() {
            if let Some(addr) = iter.next() {
                if let Ok(hostname) = addr.ip().to_string().parse::<std::net::IpAddr>() {
                    // This is a placeholder - real implementation would do proper reverse DNS
                    return None;
                }
            }
        }
    }
    
    None
}

