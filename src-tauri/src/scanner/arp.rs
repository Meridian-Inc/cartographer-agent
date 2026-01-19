//! ARP table scanning using system commands
//! No special drivers or libraries required

use crate::scanner::{hidden_command, Device};
use anyhow::Result;

/// Get devices from the system ARP table
/// This returns devices that have already been seen on the network
pub async fn get_arp_table() -> Result<Vec<Device>> {
    #[cfg(target_os = "windows")]
    {
        get_arp_table_windows()
    }
    
    #[cfg(target_os = "linux")]
    {
        get_arp_table_linux()
    }
    
    #[cfg(target_os = "macos")]
    {
        get_arp_table_macos()
    }
    
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "windows")]
fn get_arp_table_windows() -> Result<Vec<Device>> {
    use std::collections::HashMap;

    let output = hidden_command("arp")
        .args(["-a"])
        .output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    // Use HashMap to deduplicate by IP (arp -a can show same IP under multiple interfaces)
    let mut devices_by_ip: HashMap<String, Device> = HashMap::new();

    for line in output_str.lines() {
        let line = line.trim();

        // Skip empty lines and headers
        if line.is_empty() || line.starts_with("Interface") || line.contains("Internet Address") {
            continue;
        }

        // Parse lines like: "192.168.1.1          00-11-22-33-44-55     dynamic"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let ip = parts[0];
            let mac = parts[1];

            // Validate IP format
            if ip.parse::<std::net::IpAddr>().is_ok() {
                // Skip multicast and broadcast addresses
                if ip.starts_with("224.") || ip.starts_with("239.") || ip.ends_with(".255") {
                    continue;
                }

                // Validate MAC format (Windows uses dashes)
                if mac.contains('-') && mac.len() == 17 {
                    // Only insert if not already present (first occurrence wins)
                    devices_by_ip.entry(ip.to_string()).or_insert_with(|| Device {
                        ip: ip.to_string(),
                        mac: Some(mac.replace('-', ":")),
                        response_time_ms: None,
                        hostname: None,
                        vendor: None,
                        device_type: None,
                    });
                }
            }
        }
    }

    Ok(devices_by_ip.into_values().collect())
}

#[cfg(target_os = "linux")]
fn get_arp_table_linux() -> Result<Vec<Device>> {
    let output = hidden_command("arp")
        .args(["-n"])
        .output()?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    
    for line in output_str.lines().skip(1) { // Skip header
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Parse lines like: "192.168.1.1    ether   00:11:22:33:44:55   C   eth0"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let ip = parts[0];
            let mac = parts[2];
            
            // Validate IP and MAC
            if ip.parse::<std::net::IpAddr>().is_ok() && mac.contains(':') && mac.len() == 17 {
                // Skip incomplete entries
                if mac == "(incomplete)" || mac == "00:00:00:00:00:00" {
                    continue;
                }
                
                devices.push(Device {
                    ip: ip.to_string(),
                    mac: Some(mac.to_string()),
                    response_time_ms: None,
                    hostname: None,
                    vendor: None,
                    device_type: None,
                });
            }
        }
    }
    
    Ok(devices)
}

#[cfg(target_os = "macos")]
fn get_arp_table_macos() -> Result<Vec<Device>> {
    let output = hidden_command("arp")
        .args(["-a", "-n"])
        .output()?;
    
    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    
    for line in output_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        
        // Parse lines like: "? (192.168.1.1) at 00:11:22:33:44:55 on en0 ifscope [ethernet]"
        if let Some(ip_start) = line.find('(') {
            if let Some(ip_end) = line.find(')') {
                let ip = &line[ip_start + 1..ip_end];
                
                if ip.parse::<std::net::IpAddr>().is_ok() {
                    // Find MAC address after "at "
                    if let Some(at_pos) = line.find(" at ") {
                        let after_at = &line[at_pos + 4..];
                        let mac = after_at.split_whitespace().next().unwrap_or("");
                        
                        // Validate MAC format
                        if mac.contains(':') && (mac.len() == 17 || mac.len() == 14) {
                            // Skip incomplete entries
                            if mac == "(incomplete)" {
                                continue;
                            }
                            
                            devices.push(Device {
                                ip: ip.to_string(),
                                mac: Some(mac.to_string()),
                                response_time_ms: None,
                                hostname: None,
                                vendor: None,
                                device_type: None,
                            });
                        }
                    }
                }
            }
        }
    }
    
    Ok(devices)
}
