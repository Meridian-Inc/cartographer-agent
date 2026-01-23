//! ARP table scanning using system commands

use super::{hidden_command, Device};
use anyhow::Result;

/// Get devices from the system ARP table
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

    let output = hidden_command("arp").args(["-a"]).output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices_by_ip: HashMap<String, Device> = HashMap::new();

    for line in output_str.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with("Interface") || line.contains("Internet Address") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let ip = parts[0];
            let mac = parts[1];

            if ip.parse::<std::net::IpAddr>().is_ok() {
                if ip.starts_with("224.") || ip.starts_with("239.") || ip.ends_with(".255") {
                    continue;
                }

                if mac.contains('-') && mac.len() == 17 {
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
    let output = hidden_command("arp").args(["-n"]).output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in output_str.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let ip = parts[0];
            let mac = parts[2];

            if ip.parse::<std::net::IpAddr>().is_ok() && mac.contains(':') && mac.len() == 17 {
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
    let output = hidden_command("arp").args(["-a", "-n"]).output()?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();

    for line in output_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(ip_start) = line.find('(') {
            if let Some(ip_end) = line.find(')') {
                let ip = &line[ip_start + 1..ip_end];

                if ip.parse::<std::net::IpAddr>().is_ok() {
                    if let Some(at_pos) = line.find(" at ") {
                        let after_at = &line[at_pos + 4..];
                        let mac = after_at.split_whitespace().next().unwrap_or("");

                        if mac.contains(':') && (mac.len() == 17 || mac.len() == 14) {
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
