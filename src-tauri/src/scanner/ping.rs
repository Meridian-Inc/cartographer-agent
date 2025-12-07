use crate::scanner::Device;
use anyhow::{Context, Result};
use ipnetwork::IpNetwork;
use std::net::IpAddr;
use surge_ping::{Client, Config, IcmpPacket, PingIdentifier, PingSequence};
use tokio::time::{timeout, Duration};

pub async fn ping_sweep(subnet: &str) -> Result<Vec<Device>> {
    // Parse subnet
    let ip_net: IpNetwork = subnet.parse()
        .context("Failed to parse subnet")?;
    
    // Create ping client
    let client = Client::new(&Config::default())
        .context("Failed to create ping client")?;
    
    let mut devices = Vec::new();
    let mut handles = Vec::new();
    
    // Ping each IP in the subnet (limit to reasonable number)
    let mut count = 0;
    for ip in ip_net.iter().take(254) {
        if count >= 50 {
            // Limit concurrent pings
            break;
        }
        
        let ip_str = ip.to_string();
        let client_clone = client.clone();
        
        let handle = tokio::spawn(async move {
            ping_host(&client_clone, &ip_str).await
        });
        
        handles.push(handle);
        count += 1;
    }
    
    // Collect results
    for handle in handles {
        if let Ok(Ok(Some(device))) = handle.await {
            devices.push(device);
        }
    }
    
    Ok(devices)
}

async fn ping_host(client: &Client, ip: &str) -> Result<Option<Device>> {
    let addr: IpAddr = ip.parse()
        .context("Invalid IP address")?;
    
    let identifier = PingIdentifier(rand::random());
    let mut pinger = client.pinger(addr, identifier).await;
    pinger.timeout(Duration::from_secs(1));
    
    let start = std::time::Instant::now();
    
    match timeout(Duration::from_secs(1), pinger.ping(PingSequence(1), &[0; 32])).await {
        Ok(Ok((IcmpPacket::V4(_), _))) | Ok(Ok((IcmpPacket::V6(_), _))) => {
            let elapsed = start.elapsed();
            let response_time_ms = elapsed.as_secs_f64() * 1000.0;
            
            Ok(Some(Device {
                ip: ip.to_string(),
                mac: None,
                response_time_ms: Some(response_time_ms),
                hostname: None,
            }))
        }
        Ok(Err(_)) | Err(_) => {
            // Timeout or error - host not responding
            Ok(None)
        }
    }
}

