mod arp;
mod ping;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Instant;

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

/// Network information including interface, subnet, and gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub interface: String,
    pub subnet: String,
    pub gateway_ip: Option<String>,
}

/// Scan result containing devices and network information
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub devices: Vec<Device>,
    pub network_info: NetworkInfo,
}

/// Scan the local network and return devices with network information.
/// This includes gateway detection and fast hostname resolution.
pub async fn scan_network() -> Result<ScanResult> {
    use std::time::Instant;
    let scan_start = Instant::now();

    // Get network interface, subnet, and gateway
    tracing::info!("Detecting network configuration...");
    let network_info = get_full_network_info().await?;

    tracing::info!(
        "Network: {} on {} (gateway: {:?})",
        network_info.subnet,
        network_info.interface,
        network_info.gateway_ip
    );

    // First, get devices from the ARP table (already known devices)
    tracing::info!("Reading ARP table...");
    let mut devices = arp::get_arp_table().await.unwrap_or_default();
    tracing::info!("ARP table: {} known devices", devices.len());

    // Then do a ping sweep to discover new devices
    tracing::info!("Starting ping sweep (this may take 10-20 seconds)...");
    let ping_start = Instant::now();
    match ping::ping_sweep(&network_info.subnet).await {
        Ok(pinged_devices) => {
            let ping_duration = ping_start.elapsed();
            tracing::info!(
                "Ping sweep complete: {} responding hosts in {:.1}s",
                pinged_devices.len(),
                ping_duration.as_secs_f64()
            );

            // Merge ping results with ARP data
            for pinged in pinged_devices {
                if let Some(existing) = devices.iter_mut().find(|d| d.ip == pinged.ip) {
                    existing.response_time_ms = pinged.response_time_ms;
                } else {
                    devices.push(pinged);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Ping sweep failed: {}", e);
        }
    }

    // Fast hostname resolution using DNS (skip slow NetBIOS)
    if !devices.is_empty() {
        tracing::info!("Resolving hostnames for {} devices...", devices.len());
        let dns_start = Instant::now();
        resolve_hostnames_fast(&mut devices).await;
        let resolved_count = devices.iter().filter(|d| d.hostname.is_some()).count();
        tracing::info!(
            "Hostname resolution: {}/{} resolved in {:.1}s",
            resolved_count,
            devices.len(),
            dns_start.elapsed().as_secs_f64()
        );
    }

    let total_duration = scan_start.elapsed();
    tracing::info!(
        "Scan complete: {} devices found in {:.1}s",
        devices.len(),
        total_duration.as_secs_f64()
    );

    Ok(ScanResult {
        devices,
        network_info,
    })
}

/// Legacy function for backward compatibility - returns just devices
pub async fn scan_network_devices_only() -> Result<Vec<Device>> {
    let result = scan_network().await?;
    Ok(result.devices)
}

pub async fn get_network_info() -> Result<String> {
    let info = get_full_network_info().await?;
    Ok(format!("{} ({})", info.subnet, info.interface))
}

/// Get full network information including gateway IP
pub async fn get_full_network_info() -> Result<NetworkInfo> {
    #[cfg(target_os = "windows")]
    {
        get_windows_network_info_full().await
    }

    #[cfg(target_os = "linux")]
    {
        get_linux_network_info_full().await
    }

    #[cfg(target_os = "macos")]
    {
        get_macos_network_info_full().await
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Err(anyhow::anyhow!("Unsupported platform"))
    }
}

/// Fast hostname resolution using DNS with high parallelism and short timeouts.
/// Falls back gracefully if DNS is slow or unavailable.
async fn resolve_hostnames_fast(devices: &mut [Device]) {
    use tokio::time::{timeout, Duration};

    // High parallelism since DNS lookups are mostly I/O bound
    const BATCH_SIZE: usize = 32;
    // Short timeout - if DNS doesn't respond quickly, skip it
    const TIMEOUT_MS: u64 = 500;

    for chunk in devices.chunks_mut(BATCH_SIZE) {
        let futures: Vec<_> = chunk
            .iter()
            .map(|d| {
                let ip = d.ip.clone();
                async move {
                    match timeout(
                        Duration::from_millis(TIMEOUT_MS),
                        resolve_hostname_fast(&ip),
                    )
                    .await
                    {
                        Ok(hostname) => (ip, hostname),
                        Err(_) => (ip, None), // Timeout
                    }
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;

        for (ip, hostname) in results {
            if let Some(device) = chunk.iter_mut().find(|d| d.ip == ip) {
                if hostname.is_some() {
                    device.hostname = hostname;
                }
            }
        }
    }
}

/// Fast hostname resolution using system DNS resolver.
/// Much faster than NetBIOS (nbtstat) on Windows.
async fn resolve_hostname_fast(ip: &str) -> Option<String> {
    use std::net::ToSocketAddrs;

    // Use Rust's built-in DNS resolver which is much faster than spawning processes
    let socket_addr = format!("{}:0", ip);

    // Spawn blocking DNS lookup
    let ip_owned = ip.to_string();
    tokio::task::spawn_blocking(move || {
        // Try to get hostname via reverse DNS
        if let Ok(mut addrs) = socket_addr.to_socket_addrs() {
            if let Some(_addr) = addrs.next() {
                // The to_socket_addrs doesn't give us the hostname directly,
                // so we need to use getnameinfo or similar
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                {
                    // Use getent or host command for reverse lookup
                    if let Ok(output) = std::process::Command::new("getent")
                        .args(["hosts", &ip_owned])
                        .output()
                    {
                        if output.status.success() {
                            let out = String::from_utf8_lossy(&output.stdout);
                            // Format: "192.168.1.1    hostname.local"
                            if let Some(hostname) = out.split_whitespace().nth(1) {
                                if !hostname.is_empty() {
                                    return Some(hostname.to_string());
                                }
                            }
                        }
                    }
                }

                #[cfg(target_os = "windows")]
                {
                    // Use nslookup for reverse DNS (much faster than nbtstat)
                    if let Ok(output) = std::process::Command::new("nslookup")
                        .arg(&ip_owned)
                        .output()
                    {
                        if output.status.success() {
                            let out = String::from_utf8_lossy(&output.stdout);
                            // Look for "Name:    hostname"
                            for line in out.lines() {
                                let trimmed = line.trim();
                                if trimmed.starts_with("Name:") {
                                    if let Some(name) = trimmed.strip_prefix("Name:") {
                                        let hostname = name.trim();
                                        if !hostname.is_empty() && !hostname.contains(&ip_owned) {
                                            return Some(hostname.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info_full() -> Result<NetworkInfo> {
    // Get default gateway and interface using PowerShell
    let output = hidden_command("powershell")
        .args(["-Command", r#"
            $adapter = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -ne $null } | Select-Object -First 1
            if ($adapter) {
                $ip = $adapter.IPv4Address.IPAddress
                $prefix = $adapter.IPv4Address.PrefixLength
                $iface = $adapter.InterfaceAlias
                $gateway = $adapter.IPv4DefaultGateway.NextHop
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
                Write-Output "$iface|$network/$prefix|$gateway"
            }
        "#])
        .output()
        .context("Failed to run PowerShell command")?;

    let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if output_str.contains('|') {
        let parts: Vec<&str> = output_str.split('|').collect();
        if parts.len() >= 2 {
            let interface = parts[0].to_string();
            let subnet = parts[1].to_string();
            let gateway_ip = parts.get(2).map(|s| s.to_string()).filter(|s| !s.is_empty());

            return Ok(NetworkInfo {
                interface,
                subnet,
                gateway_ip,
            });
        }
    }

    // Fallback to common defaults
    Ok(NetworkInfo {
        interface: "Ethernet".to_string(),
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip: Some("192.168.1.1".to_string()),
    })
}

#[cfg(target_os = "linux")]
async fn get_linux_network_info_full() -> Result<NetworkInfo> {
    // Get default route info (includes gateway)
    // Format: "default via 192.168.1.1 dev eth0 proto dhcp metric 100"
    let route_output = hidden_command("ip")
        .args(["route", "show", "default"])
        .output()
        .context("Failed to run ip route command")?;

    let route_str = String::from_utf8_lossy(&route_output.stdout);

    // Parse gateway IP (after "via")
    let gateway_ip = route_str
        .split_whitespace()
        .skip_while(|&s| s != "via")
        .nth(1)
        .map(|s| s.to_string());

    // Parse interface (after "dev")
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
                    return Ok(NetworkInfo {
                        interface,
                        subnet,
                        gateway_ip,
                    });
                }
            }
        }
    }

    Ok(NetworkInfo {
        interface,
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip,
    })
}

#[cfg(target_os = "macos")]
async fn get_macos_network_info_full() -> Result<NetworkInfo> {
    // Get default route info (includes gateway)
    let route_output = hidden_command("route")
        .args(["-n", "get", "default"])
        .output()
        .context("Failed to run route command")?;

    let route_str = String::from_utf8_lossy(&route_output.stdout);

    // Parse gateway IP
    let gateway_ip = route_str
        .lines()
        .find(|line| line.contains("gateway:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());

    // Parse interface
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
                    let ip_addr: std::net::Ipv4Addr = ip.parse().unwrap_or([192, 168, 1, 1].into());
                    let mask_addr = std::net::Ipv4Addr::from(mask_int);
                    let network_int = u32::from(ip_addr) & u32::from(mask_addr);
                    let network = std::net::Ipv4Addr::from(network_int);
                    return Ok(NetworkInfo {
                        interface,
                        subnet: format!("{}/{}", network, prefix),
                        gateway_ip,
                    });
                }
            }
        }
    }

    Ok(NetworkInfo {
        interface,
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip,
    })
}

/// Resolve hostname for an IP address using system commands.
/// Uses reverse DNS on Linux/macOS and NetBIOS on Windows.
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

/// Ping a single device and return response time in ms if successful.
/// Used for health checks on known devices.
pub async fn ping_device(ip: &str) -> Result<f64> {
    let start = Instant::now();
    
    #[cfg(target_os = "windows")]
    let output = hidden_command("ping")
        .args(["-n", "1", "-w", "2000", ip])
        .output()
        .context("Failed to execute ping command")?;
    
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let output = hidden_command("ping")
        .args(["-c", "1", "-W", "2", ip])
        .output()
        .context("Failed to execute ping command")?;
    
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let output = {
        return Err(anyhow::anyhow!("Unsupported platform"));
    };
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("Host unreachable"));
    }
    
    let elapsed = start.elapsed();
    let default_time = elapsed.as_secs_f64() * 1000.0;
    
    // Try to parse actual ping time from output
    let output_str = String::from_utf8_lossy(&output.stdout);
    let ping_time = parse_ping_time(&output_str).unwrap_or(default_time);
    
    Ok(ping_time)
}

/// Parse ping response time from command output
fn parse_ping_time(output: &str) -> Option<f64> {
    // Windows format: "Reply from X.X.X.X: bytes=32 time=1ms TTL=64"
    // Linux/macOS format: "64 bytes from X.X.X.X: icmp_seq=1 ttl=64 time=1.23 ms"
    
    for word in output.split_whitespace() {
        if word.starts_with("time=") || word.starts_with("time<") {
            let time_str = word
                .trim_start_matches("time=")
                .trim_start_matches("time<")
                .trim_end_matches("ms");
            
            if let Ok(time) = time_str.parse::<f64>() {
                return Some(time);
            }
        }
    }
    
    None
}
