//! Network scanning module.
//!
//! Provides cross-platform network discovery using:
//! - ARP table reading
//! - ICMP ping sweep
//! - DNS/mDNS hostname resolution
//! - MAC OUI vendor lookup

mod arp;
mod ping;
pub mod oui;
pub mod privileges;

// Re-export privilege types at module level for cleaner public API
pub use privileges::ScanCapabilities;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Global flag to request scan cancellation
static SCAN_CANCEL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request cancellation of the current scan
pub fn request_scan_cancel() {
    SCAN_CANCEL_REQUESTED.store(true, Ordering::SeqCst);
    tracing::info!("Scan cancellation requested");
}

/// Check if scan cancellation has been requested
pub fn is_scan_cancelled() -> bool {
    SCAN_CANCEL_REQUESTED.load(Ordering::Relaxed)
}

/// Clear the cancellation flag (call when starting a new scan)
pub fn clear_scan_cancel() {
    SCAN_CANCEL_REQUESTED.store(false, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Create a Command that hides the console window on Windows.
pub fn hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Create a hidden command from within a non-async context.
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
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub ip: String,
    pub mac: Option<String>,
    pub response_time_ms: Option<f64>,
    pub hostname: Option<String>,
    /// Device vendor/manufacturer from MAC OUI lookup
    pub vendor: Option<String>,
    /// Inferred device type based on vendor
    pub device_type: Option<String>,
}

/// Network information including interface, subnet, and gateway
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub interface: String,
    pub subnet: String,
    pub gateway_ip: Option<String>,
    pub local_ip: Option<String>,
}

/// Scan result containing devices and network information
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub devices: Vec<Device>,
    pub network_info: NetworkInfo,
    pub capabilities: privileges::ScanCapabilities,
}

/// Progress updates during network scanning
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub stage: ScanStage,
    pub message: String,
    pub percent: Option<u8>,
    pub devices_found: Option<usize>,
    pub elapsed_secs: f64,
}

/// Stages of the network scan process
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStage {
    Starting,
    DetectingNetwork,
    ReadingArp,
    PingSweep,
    ResolvingHostnames,
    Complete,
    Failed,
}

/// Callback type for scan progress updates
pub type ProgressCallback = Box<dyn Fn(ScanProgress) + Send + Sync>;

/// Get the local machine's hostname
fn get_local_hostname() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMPUTERNAME").ok()
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
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
fn deduplicate_devices_by_ip(devices: Vec<Device>) -> Vec<Device> {
    use std::collections::HashMap;

    let mut by_ip: HashMap<String, Device> = HashMap::new();

    for device in devices {
        if let Some(existing) = by_ip.get_mut(&device.ip) {
            if existing.mac.is_none() && device.mac.is_some() {
                existing.mac = device.mac;
            }
            if existing.hostname.is_none() && device.hostname.is_some() {
                existing.hostname = device.hostname;
            }
            if existing.vendor.is_none() && device.vendor.is_some() {
                existing.vendor = device.vendor;
            }
            if existing.device_type.is_none() && device.device_type.is_some() {
                existing.device_type = device.device_type;
            }
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

/// Enrich devices with vendor information from MAC OUI lookup.
fn enrich_devices_with_vendor(devices: &mut [Device]) {
    let total_devices = devices.len();
    let devices_with_mac = devices.iter().filter(|d| d.mac.is_some()).count();

    tracing::info!(
        "OUI enrichment starting: {} devices total, {} have MAC addresses",
        total_devices,
        devices_with_mac
    );

    let mut lookup_count = 0;
    let mut found_count = 0;

    for device in devices.iter_mut() {
        if device.vendor.is_some() {
            continue;
        }
        if device.mac.is_none() {
            continue;
        }

        if let Some(ref mac) = device.mac {
            lookup_count += 1;

            if let Some(vendor) = oui::lookup_vendor(mac) {
                found_count += 1;

                let mut device_type = oui::infer_device_type(&vendor).map(String::from);

                if device_type.is_none() {
                    device_type = oui::infer_device_type_from_mac(mac).map(String::from);
                }

                tracing::info!(
                    "OUI: {} ({}) -> {} (type: {:?})",
                    device.ip,
                    mac,
                    vendor,
                    device_type
                );
                device.vendor = Some(vendor);
                device.device_type = device_type;
            } else {
                if let Some(device_type) = oui::infer_device_type_from_mac(mac) {
                    found_count += 1;
                    tracing::info!(
                        "OUI: {} ({}) -> VM/Container (type: {})",
                        device.ip,
                        mac,
                        device_type
                    );
                    device.vendor = Some("Virtual Machine".to_string());
                    device.device_type = Some(device_type.to_string());
                } else {
                    tracing::warn!("OUI: {} ({}) -> NOT FOUND", device.ip, mac);
                }
            }
        }
    }

    tracing::info!(
        "OUI enrichment complete: looked up {} MACs, found {} vendors ({:.0}%)",
        lookup_count,
        found_count,
        if lookup_count > 0 {
            (found_count as f64 / lookup_count as f64) * 100.0
        } else {
            0.0
        }
    );
}

/// Scan the local network and return devices with network information.
pub async fn scan_network() -> Result<ScanResult> {
    scan_network_with_progress(None).await
}

/// Scan the local network with progress callbacks.
pub async fn scan_network_with_progress(
    on_progress: Option<ProgressCallback>,
) -> Result<ScanResult> {
    // Clear cancellation flag at start of new scan
    clear_scan_cancel();

    let scan_start = Instant::now();

    let emit_progress =
        |stage: ScanStage, message: &str, percent: Option<u8>, devices: Option<usize>| {
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

    // Stage 0: Detect scan capabilities
    emit_progress(
        ScanStage::Starting,
        "Checking scan capabilities...",
        Some(2),
        None,
    );
    let capabilities = privileges::detect_capabilities().await;

    if capabilities.mode == privileges::ScanMode::Limited {
        tracing::warn!(
            "Running with limited scan capabilities: {:?}",
            capabilities.warning
        );
        emit_progress(
            ScanStage::Starting,
            &format!(
                "Running with limited capabilities: {}",
                capabilities
                    .warning
                    .as_deref()
                    .unwrap_or("some features unavailable")
            ),
            Some(3),
            None,
        );
    } else {
        tracing::info!("Running with full scan capabilities");
    }

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
    if capabilities.can_ping {
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
    } else {
        tracing::warn!("Ping sweep skipped: insufficient privileges");
        emit_progress(
            ScanStage::PingSweep,
            "Ping sweep skipped (requires elevated privileges). Using ARP table only.",
            Some(50),
            Some(devices.len()),
        );
    }

    // Ensure local machine is included
    if let Some(ref local_ip) = network_info.local_ip {
        let local_hostname = get_local_hostname();
        if let Some(existing) = devices.iter_mut().find(|d| &d.ip == local_ip) {
            if existing.hostname.is_none() {
                existing.hostname = local_hostname;
            }
            if existing.response_time_ms.is_none() {
                existing.response_time_ms = Some(0.0);
            }
        } else {
            devices.push(Device {
                ip: local_ip.clone(),
                mac: None,
                response_time_ms: Some(0.0),
                hostname: local_hostname,
                vendor: None,
                device_type: None,
            });
        }
    }

    // Stage 4: Hostname resolution
    if !devices.is_empty() {
        emit_progress(
            ScanStage::ResolvingHostnames,
            &format!(
                "Resolving hostnames for {} devices (may take a moment)...",
                devices.len()
            ),
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

    // Deduplicate and enrich
    let mut devices = deduplicate_devices_by_ip(devices);
    enrich_devices_with_vendor(&mut devices);

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
        capabilities,
    })
}

/// Legacy function for backward compatibility
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

/// Fast hostname resolution using DNS with high parallelism.
async fn resolve_hostnames_fast(devices: &mut [Device]) {
    use tokio::time::{timeout, Duration};

    const BATCH_SIZE: usize = 32;

    #[cfg(target_os = "windows")]
    const TIMEOUT_MS: u64 = 5000;
    #[cfg(not(target_os = "windows"))]
    const TIMEOUT_MS: u64 = 2000;

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
                        Err(_) => (ip, None),
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
async fn resolve_hostname_fast(ip: &str) -> Option<String> {
    let ip_owned = ip.to_string();
    tokio::task::spawn_blocking(move || {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            // Method 1: getent hosts
            if let Ok(output) = hidden_command_sync("getent")
                .args(["hosts", &ip_owned])
                .output()
            {
                if output.status.success() {
                    let out = String::from_utf8_lossy(&output.stdout);
                    if let Some(hostname) = out.split_whitespace().nth(1) {
                        if !hostname.is_empty() {
                            return Some(hostname.to_string());
                        }
                    }
                }
            }

            // Method 2: host command
            if let Ok(output) = hidden_command_sync("host").arg(&ip_owned).output() {
                if output.status.success() {
                    let out = String::from_utf8_lossy(&output.stdout);
                    if let Some(hostname) = out.split("pointer").nth(1) {
                        let hostname = hostname.trim().trim_end_matches('.');
                        if !hostname.is_empty() {
                            return Some(hostname.to_string());
                        }
                    }
                }
            }

            // Method 3: avahi-resolve on Linux
            #[cfg(target_os = "linux")]
            if let Ok(output) = hidden_command_sync("avahi-resolve")
                .args(["-a", &ip_owned])
                .output()
            {
                if output.status.success() {
                    let out = String::from_utf8_lossy(&output.stdout);
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
            // Method 1: PowerShell Resolve-DnsName
            if let Ok(output) = hidden_command_sync("powershell")
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &format!(
                        "try {{ (Resolve-DnsName -Name '{}' -Type PTR -ErrorAction Stop).NameHost }} catch {{ }}",
                        ip_owned
                    ),
                ])
                .output()
            {
                if output.status.success() {
                    let out = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !out.is_empty() && !out.contains("error") && !out.contains(&ip_owned) {
                        return Some(out);
                    }
                }
            }

            // Method 2: nbtstat for NetBIOS names
            if let Ok(output) = hidden_command_sync("nbtstat")
                .args(["-A", &ip_owned])
                .output()
            {
                let out = String::from_utf8_lossy(&output.stdout);
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

        None
    })
    .await
    .ok()
    .flatten()
}

// Platform-specific network info implementations
#[cfg(target_os = "windows")]
async fn get_windows_network_info_full() -> Result<NetworkInfo> {
    if let Ok(info) = get_windows_network_info_ipconfig().await {
        if !info.subnet.starts_with("0.0.0.0") && !info.subnet.is_empty() {
            return Ok(info);
        }
    }
    get_windows_network_info_powershell().await
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info_powershell() -> Result<NetworkInfo> {
    let output = hidden_command("powershell")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", r#"
            $virtualPatterns = @('vEthernet', 'WSL', 'Hyper-V', 'VirtualBox', 'VMware', 'Docker', 'Loopback', 'Tailscale')

            $defaultRoute = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
                Where-Object { $_.NextHop -ne '0.0.0.0' } |
                Select-Object -First 1

            if ($defaultRoute) {
                $ifIndex = $defaultRoute.InterfaceIndex
                $gateway = $defaultRoute.NextHop

                $adapter = Get-NetAdapter -InterfaceIndex $ifIndex -ErrorAction SilentlyContinue
                $iface = $adapter.InterfaceAlias

                $isVirtual = $false
                foreach ($pattern in $virtualPatterns) {
                    if ($iface -like "*$pattern*") {
                        $isVirtual = $true
                        break
                    }
                }

                if ($isVirtual) {
                    $physicalRoute = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue |
                        Where-Object { $_.NextHop -ne '0.0.0.0' } |
                        ForEach-Object {
                            $a = Get-NetAdapter -InterfaceIndex $_.InterfaceIndex -ErrorAction SilentlyContinue
                            $skip = $false
                            foreach ($p in $virtualPatterns) {
                                if ($a.InterfaceAlias -like "*$p*") { $skip = $true; break }
                            }
                            if (-not $skip) { $_ }
                        } |
                        Select-Object -First 1

                    if ($physicalRoute) {
                        $ifIndex = $physicalRoute.InterfaceIndex
                        $gateway = $physicalRoute.NextHop
                        $adapter = Get-NetAdapter -InterfaceIndex $ifIndex -ErrorAction SilentlyContinue
                        $iface = $adapter.InterfaceAlias
                    }
                }

                $ipInfo = Get-NetIPAddress -InterfaceIndex $ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue |
                    Where-Object { $_.PrefixOrigin -ne 'WellKnown' -and $_.IPAddress -notlike '169.254.*' } |
                    Select-Object -First 1

                if ($ipInfo) {
                    $ip = $ipInfo.IPAddress
                    $prefix = $ipInfo.PrefixLength

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
            }
        "#])
        .output()
        .context("Failed to run PowerShell command")?;

    let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if output_str.contains('|') {
        let parts: Vec<&str> = output_str.split('|').collect();
        if parts.len() >= 2 {
            return Ok(NetworkInfo {
                interface: parts[0].to_string(),
                subnet: parts[1].to_string(),
                gateway_ip: parts.get(2).map(|s| s.to_string()).filter(|s| !s.is_empty()),
                local_ip: parts.get(3).map(|s| s.to_string()).filter(|s| !s.is_empty()),
            });
        }
    }

    Err(anyhow::anyhow!("PowerShell command returned no valid data"))
}

#[cfg(target_os = "windows")]
async fn get_windows_network_info_ipconfig() -> Result<NetworkInfo> {
    let output = hidden_command("ipconfig")
        .output()
        .context("Failed to run ipconfig")?;

    let output_str = String::from_utf8_lossy(&output.stdout);
    let virtual_patterns = [
        "vEthernet",
        "WSL",
        "Hyper-V",
        "VirtualBox",
        "VMware",
        "Docker",
        "Loopback",
        "Tailscale",
    ];

    struct AdapterInfo {
        name: String,
        ip: Option<String>,
        mask: Option<String>,
        gateway: Option<String>,
        is_virtual: bool,
    }

    let mut adapters: Vec<AdapterInfo> = Vec::new();
    let mut current = AdapterInfo {
        name: String::new(),
        ip: None,
        mask: None,
        gateway: None,
        is_virtual: false,
    };

    for line in output_str.lines() {
        let trimmed = line.trim();

        if line.starts_with("Ethernet adapter") || line.starts_with("Wireless LAN adapter") {
            if current.ip.is_some() {
                adapters.push(current);
            }
            let name = line.trim_end_matches(':').to_string();
            let is_virtual = virtual_patterns.iter().any(|p| name.contains(p));
            current = AdapterInfo {
                name,
                ip: None,
                mask: None,
                gateway: None,
                is_virtual,
            };
        }

        if trimmed.starts_with("IPv4 Address") || trimmed.starts_with("IP Address") {
            if let Some(ip) = trimmed.split(':').nth(1) {
                let ip = ip.trim().trim_start_matches(". ");
                if !ip.starts_with("127.") && !ip.starts_with("169.254.") {
                    current.ip = Some(ip.to_string());
                }
            }
        }

        if trimmed.starts_with("Subnet Mask") {
            if let Some(mask) = trimmed.split(':').nth(1) {
                current.mask = Some(mask.trim().trim_start_matches(". ").to_string());
            }
        }

        if trimmed.starts_with("Default Gateway") {
            if let Some(gw) = trimmed.split(':').nth(1) {
                let gw = gw.trim().trim_start_matches(". ");
                if !gw.is_empty() {
                    current.gateway = Some(gw.to_string());
                }
            }
        }
    }

    if current.ip.is_some() {
        adapters.push(current);
    }

    let best_adapter = adapters
        .iter()
        .find(|a| !a.is_virtual && a.ip.is_some() && a.gateway.is_some())
        .or_else(|| adapters.iter().find(|a| !a.is_virtual && a.ip.is_some()));

    if let Some(adapter) = best_adapter {
        if let (Some(ip_str), Some(mask_str)) = (&adapter.ip, &adapter.mask) {
            if let (Ok(ip), Ok(mask)) = (
                ip_str.parse::<std::net::Ipv4Addr>(),
                mask_str.parse::<std::net::Ipv4Addr>(),
            ) {
                let ip_u32 = u32::from(ip);
                let mask_u32 = u32::from(mask);
                let network_u32 = ip_u32 & mask_u32;
                let network = std::net::Ipv4Addr::from(network_u32);
                let prefix = mask_u32.count_ones();

                return Ok(NetworkInfo {
                    interface: adapter.name.clone(),
                    subnet: format!("{}/{}", network, prefix),
                    gateway_ip: adapter.gateway.clone(),
                    local_ip: adapter.ip.clone(),
                });
            }
        }
    }

    Ok(NetworkInfo {
        interface: "Ethernet".to_string(),
        subnet: "192.168.1.0/24".to_string(),
        gateway_ip: Some("192.168.1.1".to_string()),
        local_ip: None,
    })
}

#[cfg(target_os = "linux")]
async fn get_linux_network_info_full() -> Result<NetworkInfo> {
    let route_output = hidden_command("ip")
        .args(["route", "show", "default"])
        .output()
        .context("Failed to run ip route command")?;

    let route_str = String::from_utf8_lossy(&route_output.stdout);

    let gateway_ip = route_str
        .split_whitespace()
        .skip_while(|&s| s != "via")
        .nth(1)
        .map(|s| s.to_string());

    let interface = route_str
        .split_whitespace()
        .skip_while(|&s| s != "dev")
        .nth(1)
        .unwrap_or("eth0")
        .to_string();

    let addr_output = hidden_command("ip")
        .args(["addr", "show", &interface])
        .output()
        .context("Failed to run ip addr command")?;

    let addr_str = String::from_utf8_lossy(&addr_output.stdout);

    for line in addr_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("inet ") && !trimmed.contains("127.0.0.1") {
            if let Some(cidr) = trimmed.split_whitespace().nth(1) {
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
    let route_output = hidden_command("route")
        .args(["-n", "get", "default"])
        .output()
        .context("Failed to run route command")?;

    let route_str = String::from_utf8_lossy(&route_output.stdout);

    let gateway_ip = route_str
        .lines()
        .find(|line| line.contains("gateway:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());

    let interface = route_str
        .lines()
        .find(|line| line.contains("interface:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "en0".to_string());

    let ifconfig_output = hidden_command("ifconfig")
        .arg(&interface)
        .output()
        .context("Failed to run ifconfig command")?;

    let ifconfig_str = String::from_utf8_lossy(&ifconfig_output.stdout);

    for line in ifconfig_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("inet ") && !trimmed.contains("127.0.0.1") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if let (Some(ip), Some(mask)) = (parts.get(1), parts.get(3)) {
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

/// Ping a single device and return response time in ms if successful.
pub async fn ping_device(ip: &str) -> Result<f64> {
    let ip_owned = ip.to_string();

    let result = tokio::task::spawn_blocking(move || {
        let start = Instant::now();

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

                #[cfg(target_os = "windows")]
                {
                    let output_lower = output_str.to_lowercase();
                    if output_lower.contains("request timed out")
                        || output_lower.contains("destination host unreachable")
                        || output_lower.contains("transmit failed")
                        || output_lower.contains("general failure")
                    {
                        return Err(anyhow::anyhow!("Host unreachable"));
                    }

                    if !output.status.success() {
                        return Err(anyhow::anyhow!("Host unreachable (exit code)"));
                    }

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

fn parse_ping_time(output: &str) -> Option<f64> {
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
pub async fn get_arp_table_ips() -> HashSet<String> {
    match arp::get_arp_table().await {
        Ok(devices) => devices.into_iter().map(|d| d.ip).collect(),
        Err(_) => HashSet::new(),
    }
}

/// Check if a device is reachable, with ARP table fallback.
pub async fn check_device_reachable(ip: &str, arp_ips: &HashSet<String>) -> Result<f64> {
    match ping_device(ip).await {
        Ok(time) => Ok(time),
        Err(_) => {
            if arp_ips.contains(ip) {
                tracing::debug!(
                    "Device {} doesn't respond to ICMP but is in ARP table",
                    ip
                );
                Ok(0.0)
            } else {
                Err(anyhow::anyhow!("Device not reachable"))
            }
        }
    }
}
