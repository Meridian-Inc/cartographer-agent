//! Ping sweep using system ping command

use super::{hidden_command_sync, is_scan_cancelled, Device};
use anyhow::{Context, Result};
use ipnetwork::IpNetwork;
use std::time::Instant;

/// Perform a ping sweep of the subnet using the system ping command.
/// Supports cancellation via `request_scan_cancel()`.
pub async fn ping_sweep(subnet: &str) -> Result<Vec<Device>> {
    let ip_net: IpNetwork = subnet.parse().context("Failed to parse subnet")?;

    let mut devices = Vec::new();

    // Generate IP list (skip network and broadcast addresses)
    let ips: Vec<std::net::IpAddr> = ip_net
        .iter()
        .skip(1) // Skip network address
        .take(253) // Limit to prevent scanning too many hosts
        .collect();

    let total_hosts = ips.len();
    tracing::info!("Pinging {} hosts in subnet {}", total_hosts, subnet);

    // High parallelism for fast scanning
    let batch_size = 50;
    let mut completed = 0;

    for (batch_idx, batch) in ips.chunks(batch_size).enumerate() {
        // Check for cancellation before starting each batch
        if is_scan_cancelled() {
            tracing::info!("Ping sweep cancelled after {} hosts", completed);
            return Err(anyhow::anyhow!("Scan cancelled by user"));
        }

        let mut batch_handles = Vec::new();

        for ip in batch {
            let ip_str = ip.to_string();
            let handle = tokio::spawn(async move { ping_host(&ip_str).await });
            batch_handles.push(handle);
        }

        // Wait for this batch to complete
        let mut batch_found = 0;
        for handle in batch_handles {
            if let Ok(Ok(Some(device))) = handle.await {
                devices.push(device);
                batch_found += 1;
            }
        }

        completed += batch.len();
        if batch_found > 0 || (batch_idx + 1) % 3 == 0 {
            tracing::debug!(
                "Ping progress: {}/{} hosts checked, {} responding",
                completed,
                total_hosts,
                devices.len()
            );
        }
    }

    Ok(devices)
}

/// Ping a single host using the system ping command.
async fn ping_host(ip: &str) -> Result<Option<Device>> {
    let ip_owned = ip.to_string();

    let result = tokio::task::spawn_blocking(move || {
        let start = Instant::now();

        #[cfg(target_os = "windows")]
        let output = hidden_command_sync("ping")
            .args(["-n", "1", "-w", "1000", &ip_owned])
            .output();

        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let output = hidden_command_sync("ping")
            .args(["-c", "1", "-W", "1", &ip_owned])
            .output();

        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        let output: std::io::Result<std::process::Output> = Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Unsupported platform",
        ));

        match output {
            Ok(output) => {
                let elapsed = start.elapsed();
                let response_time_ms = elapsed.as_secs_f64() * 1000.0;
                let output_str = String::from_utf8_lossy(&output.stdout);

                #[cfg(target_os = "windows")]
                {
                    let output_lower = output_str.to_lowercase();

                    if output_lower.contains("request timed out")
                        || output_lower.contains("destination host unreachable")
                        || output_lower.contains("transmit failed")
                        || output_lower.contains("general failure")
                    {
                        return Ok(None);
                    }

                    if !output_lower.contains("reply from") {
                        return Ok(None);
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    if !output.status.success() {
                        return Ok(None);
                    }
                }

                let ping_time = parse_ping_time(&output_str).unwrap_or(response_time_ms);

                Ok(Some(Device {
                    ip: ip_owned,
                    mac: None,
                    response_time_ms: Some(ping_time),
                    hostname: None,
                    vendor: None,
                    device_type: None,
                }))
            }
            Err(_) => Ok(None),
        }
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Ok(None),
    }
}

/// Parse ping response time from command output
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
