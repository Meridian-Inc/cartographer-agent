mod arp;
mod ping;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use std::time::Instant;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Windows flags to hide console window when spawning processes
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Create a Command that hides the console window on Windows.
/// On other platforms, this just creates a normal Command.
///
/// IMPORTANT: Always use this function instead of Command::new() directly
/// to prevent console windows from flashing on Windows.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Create a hidden command from within a non-async context (like spawn_blocking).
/// This is needed because hidden_command imports aren't always available in closures.
/// 
/// pub(crate) so it can be used in submodules like ping.rs
#[cfg(target_os = "windows")]
pub(crate) fn hidden_command_sync(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn hidden_command_sync(program: &str) -> Command {
    Command::new(program)
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
    /// The local IP address of this machine on the network
    pub local_ip: Option<String>,
}

/// Scan result containing devices and network information
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub devices: Vec<Device>,
    pub network_info: NetworkInfo,
}

/// Get the local machine's hostname
fn get_local_hostname() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        // Use COMPUTERNAME environment variable on Windows
        std::env::var("COMPUTERNAME").ok()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // Use hostname command on Unix-like systems
        hidden_command("hostname")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Deduplicate devices by IP address, keeping the most complete record.
/// When duplicates are found, prefers the record with more data (MAC, hostname, response time).
fn deduplicate_devices_by_ip(devices: Vec<Device>) -> Vec<Device> {
    use std::collections::HashMap;

    let mut by_ip: HashMap<String, Device> = HashMap::new();

    for device in devices {
        if let Some(existing) = by_ip.get_mut(&device.ip) {
            // Merge data: keep the most complete record
            if existing.mac.is_none() && device.mac.is_some() {
                existing.mac = device.mac;
            }
            if existing.hostname.is_none() && device.hostname.is_some() {
                existing.hostname = device.hostname;
            }
            // Prefer non-zero response times
            if existing.response_time_ms.is_none()
                || (device.response_time_ms.is_some()
                    && device.response_time_ms.unwrap_or(0.0) > 0.0
                    && existing.response_time_ms.unwrap_or(0.0) == 0.0)
            {
                existing.response_time_ms = device.response_time_ms;
            }
        } else {
            by_ip.insert(device.ip.clone(), device);
        }
    }

    by_ip.into_values().collect()
}

/// Progress updates during network scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// Current stage of the scan
    pub stage: ScanStage,
    /// Human-readable message describing current activity
    pub message: String,
    /// Progress percentage (0-100), if known
    pub percent: Option<u8>,
    /// Number of devices found so far
    pub devices_found: Option<usize>,
    /// Elapsed time in seconds
    pub elapsed_secs: f64,
}

/// Stages of the network scan process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStage {
    /// Scan is starting (initial stage)
    Starting,
    /// Detecting network configuration
    DetectingNetwork,
    /// Reading ARP table for known devices
    ReadingArp,
    /// Performing ping sweep to discover devices
    PingSweep,
    /// Resolving hostnames for discovered devices
    ResolvingHostnames,
    /// Scan complete
    Complete,
    /// Scan failed
    Failed,
}

/// Callback type for scan progress updates
pub type ProgressCallback = Box<dyn Fn(ScanProgress) + Send + Sync>;

/// Scan the local network and return devices with network information.
/// This includes gateway detection and fast hostname resolution.
pub async fn scan_network() -> Result<ScanResult> {
    scan_network_with_progress(None).await
}

/// Scan the local network with progress callbacks.
/// This includes gateway detection and fast hostname resolution.
pub async fn scan_network_with_progress(
    on_progress: Option<ProgressCallback>,
) -> Result<ScanResult> {
    let scan_start = Instant::now();

    // Helper to emit progress
    let emit_progress = |stage: ScanStage, message: &str, percent: Option<u8>, devices: Option<usize>| {
        let progress = ScanProgress {
            stage,
            message: message.to_string(),
            percent,
            devices_found: devices,
            elapsed_secs: scan_start.elapsed().as_secs_f64(),
        };
        tracing::info!("[Scan] {}", message);
        if let Some(ref callback) = on_progress {
            callback(progress);
        }
    };

    // Stage 1: Detect network configuration
    emit_progress(
        ScanStage::DetectingNetwork,
        "Detecting network configuration...",
        Some(5),
        None,
    );
    let network_info = get_full_network_info().await?;

    tracing::info!(
        "Network: {} on {} (gateway: {:?})",
        network_info.subnet,
        network_info.interface,
        network_info.gateway_ip
    );

    // Stage 2: Read ARP table
    emit_progress(
        ScanStage::ReadingArp,
        "Reading known devices from ARP table...",
        Some(10),
        None,
    );
    let mut devices = arp::get_arp_table().await.unwrap_or_default();
    let arp_count = devices.len();

    emit_progress(
        ScanStage::ReadingArp,
        &format!("Found {} devices in ARP cache", arp_count),
        Some(15),
        Some(arp_count),
    );

    // Stage 3: Ping sweep
    emit_progress(
        ScanStage::PingSweep,
        "Discovering devices on network (ping sweep)...",
        Some(20),
        Some(devices.len()),
    );

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

            emit_progress(
                ScanStage::PingSweep,
                &format!("Discovered {} total devices", devices.len()),
                Some(50),
                Some(devices.len()),
            );
        }
        Err(e) => {
            tracing::warn!("Ping sweep failed: {}", e);
            emit_progress(
                ScanStage::PingSweep,
                &format!("Ping sweep had issues: {}", e),
                Some(50),
                Some(devices.len()),
            );
        }
    }

    // Ensure the local machine is included in the device list
    if let Some(ref local_ip) = network_info.local_ip {
        if !devices.iter().any(|d| &d.ip == local_ip) {
            tracing::info!("Adding local machine {} to device list", local_ip);
            devices.push(Device {
                ip: local_ip.clone(),
                mac: None,
                response_time_ms: Some(0.0), // Local machine has instant response
                hostname: get_local_hostname(),
            });
        }
    }

    // Stage 4: Hostname resolution
    if !devices.is_empty() {
        emit_progress(
            ScanStage::ResolvingHostnames,
            &format!("Resolving hostnames for {} devices (may take a moment)...", devices.len()),
            Some(55),
            Some(devices.len()),
        );

        let dns_start = Instant::now();
        resolve_hostnames_fast(&mut devices).await;
        let resolved_count = devices.iter().filter(|d| d.hostname.is_some()).count();

        emit_progress(
            ScanStage::ResolvingHostnames,
            &format!(
                "Resolved {}/{} hostnames in {:.1}s",
                resolved_count,
                devices.len(),
                dns_start.elapsed().as_secs_f64()
            ),
            Some(95),
            Some(devices.len()),
        );
    }

    // Deduplicate devices by IP address (in case of duplicates from ARP or ping)
    let devices = deduplicate_devices_by_ip(devices);

    // Stage 5: Complete
    let total_duration = scan_start.elapsed();
    emit_progress(
        ScanStage::Complete,
        &format!(
            "Scan complete: {} devices found in {:.1}s",
            devices.len(),
            total_duration.as_secs_f64()
        ),
        Some(100),
        Some(devices.len()),
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

    // Timeout varies by platform:
    // - Windows uses nbtstat as fallback which is slower (needs ~2s for NetBIOS timeout)
    // - Linux/macOS DNS is typically faster
    #[cfg(target_os = "windows")]
    const TIMEOUT_MS: u64 = 3000;
    #[cfg(not(target_os = "windows"))]
    const TIMEOUT_MS: u64 = 1500;

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
                    // Try multiple methods for hostname resolution on Linux/macOS:
                    // 1. getent - uses system resolver (includes /etc/hosts, DNS, mDNS if configured)
                    // 2. host - reverse DNS lookup
                    // 3. avahi-resolve (Linux) / dns-sd (macOS) - mDNS/Bonjour

                    // Method 1: getent hosts (most comprehensive, uses NSS)
                    if let Ok(output) = hidden_command_sync("getent")
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

                    // Method 2: host command for reverse DNS
                    if let Ok(output) = hidden_command_sync("host")
                        .arg(&ip_owned)
                        .output()
                    {
                        if output.status.success() {
                            let out = String::from_utf8_lossy(&output.stdout);
                            // Parse "X.X.X.X.in-addr.arpa domain name pointer hostname."
                            if let Some(hostname) = out.split("pointer").nth(1) {
                                let hostname = hostname.trim().trim_end_matches('.');
                                if !hostname.is_empty() {
                                    return Some(hostname.to_string());
                                }
                            }
                        }
                    }

                    // Method 3: Try avahi-resolve on Linux for mDNS (.local domains)
                    #[cfg(target_os = "linux")]
                    if let Ok(output) = hidden_command_sync("avahi-resolve")
                        .args(["-a", &ip_owned])
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
                    // Try multiple methods for hostname resolution on Windows:
                    // 1. nslookup - for devices with proper DNS PTR records
                    // 2. nbtstat - for Windows devices via NetBIOS (most home networks)

                    // Method 1: nslookup for reverse DNS
                    if let Ok(output) = hidden_command_sync("nslookup")
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

                    // Method 2: nbtstat for NetBIOS names (works for most Windows devices)
                    // This is slower but works on typical home networks without DNS
                    if let Ok(output) = hidden_command_sync("nbtstat")
                        .args(["-A", &ip_owned])
                        .output()
                    {
                        let out = String::from_utf8_lossy(&output.stdout);
                        // Look for lines with "<00>" and "UNIQUE" which indicate the computer name
                        // Format: "COMPUTER-NAME   <00>  UNIQUE      Registered"
                        for line in out.lines() {
                            let trimmed = line.trim();
                            if trimmed.contains("<00>") && trimmed.contains("UNIQUE") {
                                if let Some(name) = trimmed.split_whitespace().next() {
                                    if !name.is_empty() {
                                        return Some(name.to_string());
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
    // Try PowerShell first (more reliable for getting complete info)
    if let Ok(info) = get_windows_network_info_powershell().await {
        // Validate the result - check for 0.0.0.0 which indicates failure
        if !info.subnet.starts_with("0.0.0.0") && !info.subnet.is_empty() {
            return Ok(info);
        }
        tracing::warn!("PowerShell returned invalid subnet: {}", info.subnet);
    }

    // Fallback to ipconfig parsing
    tracing::info!("Falling back to ipconfig for network detection");
    get_windows_network_info_ipconfig().await
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info_powershell() -> Result<NetworkInfo> {
    // Get default gateway and interface using PowerShell
    // Filter out virtual adapters (WSL, Hyper-V, VirtualBox, VMware, Docker, Tailscale)
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", r#"
            # Get all adapters with a gateway, excluding virtual/VPN adapters
            $virtualPatterns = @('vEthernet', 'WSL', 'Hyper-V', 'VirtualBox', 'VMware', 'Docker', 'Loopback', 'Tailscale')
            $adapters = Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -ne $null }
            
            # Filter out virtual adapters
            $physicalAdapter = $null
            foreach ($adapter in $adapters) {
                $isVirtual = $false
                foreach ($pattern in $virtualPatterns) {
                    if ($adapter.InterfaceAlias -like "*$pattern*") {
                        $isVirtual = $true
                        break
                    }
                }
                if (-not $isVirtual) {
                    $physicalAdapter = $adapter
                    break
                }
            }
            
            # If no physical adapter found, try any adapter as fallback
            if ($physicalAdapter -eq $null -and $adapters.Count -gt 0) {
                $physicalAdapter = $adapters | Select-Object -First 1
            }
            
            if ($physicalAdapter) {
                # Handle IPv4Address being an array - get first element
                $ipv4Addr = $physicalAdapter.IPv4Address
                if ($ipv4Addr -is [array]) {
                    $ipv4Addr = $ipv4Addr[0]
                }
                $ip = $ipv4Addr.IPAddress
                $prefix = $ipv4Addr.PrefixLength
                $iface = $physicalAdapter.InterfaceAlias
                
                # Handle gateway being an array too
                $gw = $physicalAdapter.IPv4DefaultGateway
                if ($gw -is [array]) {
                    $gw = $gw[0]
                }
                $gateway = $gw.NextHop
                
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
                Write-Output "$iface|$network/$prefix|$gateway|$ip"
            }
        "#])
        .output()
        .context("Failed to run PowerShell command")?;

    let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !stderr_str.is_empty() {
        tracing::warn!("PowerShell stderr: {}", stderr_str);
    }

    if output_str.contains('|') {
        let parts: Vec<&str> = output_str.split('|').collect();
        if parts.len() >= 2 {
            let interface = parts[0].to_string();
            let subnet = parts[1].to_string();
            let gateway_ip = parts.get(2).map(|s| s.to_string()).filter(|s| !s.is_empty());
            let local_ip = parts.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty());

            return Ok(NetworkInfo {
                interface,
                subnet,
                gateway_ip,
                local_ip,
            });
        }
    }

    Err(anyhow::anyhow!("PowerShell command returned no valid data"))
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info_ipconfig() -> Result<NetworkInfo> {
    // Parse ipconfig output as fallback
    let output = hidden_command("ipconfig")
        .output()
        .context("Failed to run ipconfig")?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    
    let mut interface = "Ethernet".to_string();
    let mut local_ip: Option<String> = None;
    let mut subnet_mask: Option<String> = None;
    let mut gateway_ip: Option<String> = None;
    let mut current_adapter = String::new();
    let mut found_active_adapter = false;

    // Virtual adapter patterns to skip
    let virtual_patterns = ["vEthernet", "WSL", "Hyper-V", "VirtualBox", "VMware", "Docker", "Loopback", "Tailscale"];
    let mut is_virtual_adapter = false;
    
    for line in output_str.lines() {
        let trimmed = line.trim();
        
        // Detect adapter name (lines that end with ":")
        if line.starts_with("Ethernet adapter") || line.starts_with("Wireless LAN adapter") {
            current_adapter = line.trim_end_matches(':').to_string();
            // Check if this is a virtual adapter
            is_virtual_adapter = virtual_patterns.iter().any(|p| current_adapter.contains(p));
        }
        
        // Look for IPv4 Address (skip virtual adapters)
        if !is_virtual_adapter && (trimmed.starts_with("IPv4 Address") || trimmed.starts_with("IP Address")) {
            if let Some(ip) = trimmed.split(':').nth(1) {
                let ip = ip.trim().trim_start_matches(". ");
                // Skip loopback and APIPA addresses
                if !ip.starts_with("127.") && !ip.starts_with("169.254.") {
                    local_ip = Some(ip.to_string());
                    interface = current_adapter.clone();
                    found_active_adapter = true;
                }
            }
        }
        
        // Look for Subnet Mask
        if found_active_adapter && trimmed.starts_with("Subnet Mask") {
            if let Some(mask) = trimmed.split(':').nth(1) {
                subnet_mask = Some(mask.trim().trim_start_matches(". ").to_string());
            }
        }
        
        // Look for Default Gateway
        if found_active_adapter && trimmed.starts_with("Default Gateway") {
            if let Some(gw) = trimmed.split(':').nth(1) {
                let gw = gw.trim().trim_start_matches(". ");
                if !gw.is_empty() {
                    gateway_ip = Some(gw.to_string());
                }
            }
        }
    }

    // Calculate subnet from IP and mask
    if let (Some(ip_str), Some(mask_str)) = (&local_ip, &subnet_mask) {
        if let (Ok(ip), Ok(mask)) = (ip_str.parse::<std::net::Ipv4Addr>(), mask_str.parse::<std::net::Ipv4Addr>()) {
            let ip_u32 = u32::from(ip);
            let mask_u32 = u32::from(mask);
            let network_u32 = ip_u32 & mask_u32;
            let network = std::net::Ipv4Addr::from(network_u32);
            let prefix = mask_u32.count_ones();
            
            return Ok(NetworkInfo {
                interface,
                subnet: format!("{}/{}", network, prefix),
                gateway_ip,
                local_ip,
            });
        }
    }

    // Last resort fallback
    tracing::warn!("Could not detect network info, using defaults");
    Ok(NetworkInfo {
        interface: "Ethernet".to_string(),
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip: Some("192.168.1.1".to_string()),
        local_ip: None,
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
                    let local_ip = network.ip().to_string();
                    let subnet = format!("{}/{}", network.network(), network.prefix());
                    return Ok(NetworkInfo {
                        interface,
                        subnet,
                        gateway_ip,
                        local_ip: Some(local_ip),
                    });
                }
            }
        }
    }

    Ok(NetworkInfo {
        interface,
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip,
        local_ip: None,
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
                        local_ip: Some(ip.to_string()),
                    });
                }
            }
        }
    }

    Ok(NetworkInfo {
        interface,
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip,
        local_ip: None,
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
///
/// Uses spawn_blocking to avoid blocking the tokio runtime with the
/// synchronous ping command execution.
pub async fn ping_device(ip: &str) -> Result<f64> {
    let ip_owned = ip.to_string();

    // Run the blocking ping command on a separate thread pool
    let result = tokio::task::spawn_blocking(move || {
        let start = Instant::now();

        // Use hidden_command_sync inside spawn_blocking to ensure
        // CREATE_NO_WINDOW flag is applied (prevents console window flash on Windows)
        #[cfg(target_os = "windows")]
        let output = hidden_command_sync("ping")
            .args(["-n", "1", "-w", "2000", &ip_owned])
            .output();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let output = hidden_command_sync("ping")
            .args(["-c", "1", "-W", "2", &ip_owned])
            .output();

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        let output: std::io::Result<std::process::Output> = {
            return Err(anyhow::anyhow!("Unsupported platform"));
        };

        match output {
            Ok(output) => {
                let elapsed = start.elapsed();
                let default_time = elapsed.as_secs_f64() * 1000.0;
                let output_str = String::from_utf8_lossy(&output.stdout);

                // On Windows, check both exit code AND output content
                // Some Windows configurations return exit code 0 even for timeouts
                #[cfg(target_os = "windows")]
                {
                    // Check for failure indicators in output
                    let output_lower = output_str.to_lowercase();
                    if output_lower.contains("request timed out")
                        || output_lower.contains("destination host unreachable")
                        || output_lower.contains("transmit failed")
                        || output_lower.contains("general failure")
                    {
                        return Err(anyhow::anyhow!("Host unreachable"));
                    }

                    // Also check exit code
                    if !output.status.success() {
                        return Err(anyhow::anyhow!("Host unreachable (exit code)"));
                    }

                    // Must have "Reply from" to be considered successful
                    if !output_lower.contains("reply from") {
                        return Err(anyhow::anyhow!("No reply received"));
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    if !output.status.success() {
                        return Err(anyhow::anyhow!("Host unreachable"));
                    }
                }

                // Parse actual ping time from output
                let ping_time = parse_ping_time(&output_str).unwrap_or(default_time);
                Ok(ping_time)
            }
            Err(e) => Err(anyhow::anyhow!("Failed to execute ping: {}", e)),
        }
    })
    .await
    .context("Ping task panicked")?;

    result
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

/// Get a set of all IP addresses currently in the ARP table.
/// This is useful for checking if devices that don't respond to ping
/// are still on the network.
pub async fn get_arp_table_ips() -> HashSet<String> {
    match arp::get_arp_table().await {
        Ok(devices) => devices.into_iter().map(|d| d.ip).collect(),
        Err(_) => HashSet::new(),
    }
}

/// Check if a device is reachable, with ARP table fallback.
///
/// First tries ICMP ping. If that fails, checks if the device is in the
/// ARP table (indicating recent network activity). This handles devices
/// that have firewalls blocking ICMP.
///
/// Returns Ok(response_time_ms) if ping succeeded, or Ok(0.0) if found in ARP,
/// or Err if device is not reachable by either method.
pub async fn check_device_reachable(ip: &str, arp_ips: &HashSet<String>) -> Result<f64> {
    // First try ICMP ping
    match ping_device(ip).await {
        Ok(time) => Ok(time),
        Err(_) => {
            // Ping failed, check ARP table as fallback
            if arp_ips.contains(ip) {
                tracing::debug!(
                    "Device {} doesn't respond to ICMP but is in ARP table",
                    ip
                );
                // Return 0.0 to indicate "reachable but no ping time"
                Ok(0.0)
            } else {
                Err(anyhow::anyhow!("Device not reachable"))
            }
        }
    }
}
