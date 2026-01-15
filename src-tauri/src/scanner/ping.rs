//! Ping sweep using system ping command
//! No special drivers or libraries required

use crate::scanner::{hidden_command, Device};
use anyhow::{Context, Result};
use ipnetwork::IpNetwork;
use std::time::Instant;

/// Perform a ping sweep of the subnet using the system ping command
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

    // High parallelism for fast scanning - ping commands are mostly I/O wait
    let batch_size = 50;
    let mut completed = 0;

    for (batch_idx, batch) in ips.chunks(batch_size).enumerate() {
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

/// Ping a single host using the system ping command
async fn ping_host(ip: &str) -> Result<Option<Device>> {
    let start = Instant::now();
    
    #[cfg(target_os = "windows")]
    let output = hidden_command("ping")
        .args(["-n", "1", "-w", "1000", ip])
        .output();
    
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let output = hidden_command("ping")
        .args(["-c", "1", "-W", "1", ip])
        .output();
    
    match output {
        Ok(output) if output.status.success() => {
            let elapsed = start.elapsed();
            let response_time_ms = elapsed.as_secs_f64() * 1000.0;
            
            // Parse the actual ping time from output if possible
            let output_str = String::from_utf8_lossy(&output.stdout);
            let ping_time = parse_ping_time(&output_str).unwrap_or(response_time_ms);
            
            Ok(Some(Device {
                ip: ip.to_string(),
                mac: None,
                response_time_ms: Some(ping_time),
                hostname: None,
            }))
        }
        Ok(_) => {
            // Ping failed (host unreachable)
            Ok(None)
        }
        Err(e) => {
            tracing::debug!("Failed to ping {}: {}", ip, e);
            Ok(None)
        }
    }
}

/// Parse ping response time from command output
fn parse_ping_time(output: &str) -> Option<f64> {
    // Windows format: "Reply from X.X.X.X: bytes=32 time=1ms TTL=64"
    // Linux/macOS format: "64 bytes from X.X.X.X: icmp_seq=1 ttl=64 time=1.23 ms"
    
    // Look for "time=" or "time<" pattern
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
