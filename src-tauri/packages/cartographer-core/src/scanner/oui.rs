//! MAC OUI (Organizationally Unique Identifier) vendor lookup
//!
//! Uses the IEEE OUI database to identify device manufacturers from MAC addresses.

/// Lookup the vendor/manufacturer name for a MAC address.
///
/// # Arguments
/// * `mac` - MAC address in any common format (e.g., "00:1A:2B:3C:4D:5E", "00-1A-2B-3C-4D-5E")
///
/// # Returns
/// * `Some(vendor_name)` if found in the OUI database
/// * `None` if the MAC address is invalid or not found
pub fn lookup_vendor(mac: &str) -> Option<String> {
    let normalized = normalize_mac(mac)?;

    match oui_data::lookup(&normalized) {
        Some(record) => {
            let vendor_name = record.organization().to_string();
            tracing::debug!(
                "OUI lookup for {}: found {} (registry: {:?})",
                mac,
                vendor_name,
                record.registry()
            );
            Some(vendor_name)
        }
        None => {
            tracing::debug!("OUI lookup for {}: not found in database", mac);
            None
        }
    }
}

/// Normalize a MAC address to the format XX:XX:XX:XX:XX:XX
fn normalize_mac(mac: &str) -> Option<String> {
    let cleaned: String = mac.replace([':', '-', '.'], "").to_uppercase();

    if cleaned.len() < 6 {
        return None;
    }

    if !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let padded = if cleaned.len() < 12 {
        format!("{:0<12}", cleaned)
    } else {
        cleaned[..12].to_string()
    };

    Some(format!(
        "{}:{}:{}:{}:{}:{}",
        &padded[0..2],
        &padded[2..4],
        &padded[4..6],
        &padded[6..8],
        &padded[8..10],
        &padded[10..12]
    ))
}

/// Infer device type from vendor name.
///
/// Categories:
/// - firewall: Security appliances
/// - network_device: Network equipment (routers, switches, APs)
/// - service: Virtualization and container hosts
/// - server: Server hardware
/// - nas: Network attached storage
/// - apple: Apple devices
/// - iot: Smart home and IoT devices
/// - printer: Printers and scanners
/// - gaming: Gaming consoles
/// - mobile: Mobile phones and tablets
/// - computer: Desktop/laptop computers
pub fn infer_device_type(vendor: &str) -> Option<&'static str> {
    let vendor_lower = vendor.to_lowercase();

    // Firewall / Security appliances
    if vendor_lower.contains("firewalla")
        || vendor_lower.contains("pfsense")
        || vendor_lower.contains("opnsense")
        || vendor_lower.contains("sophos")
        || vendor_lower.contains("watchguard")
        || vendor_lower.contains("sonicwall")
        || vendor_lower.contains("barracuda")
        || vendor_lower.contains("checkpoint")
        || vendor_lower.contains("forcepoint")
        || vendor_lower.contains("untangle")
    {
        return Some("firewall");
    }

    // Virtualization / Container hosts
    if vendor_lower.contains("proxmox")
        || vendor_lower.contains("vmware")
        || vendor_lower.contains("xensource")
        || vendor_lower.contains("parallels")
        || vendor_lower.contains("virtualbox")
        || vendor_lower.contains("qemu")
        || vendor_lower.contains("docker")
        || vendor_lower.contains("kubernetes")
    {
        return Some("service");
    }

    // Network equipment
    if vendor_lower.contains("cisco")
        || vendor_lower.contains("juniper")
        || vendor_lower.contains("arista")
        || vendor_lower.contains("ubiquiti")
        || vendor_lower.contains("netgear")
        || vendor_lower.contains("tp-link")
        || vendor_lower.contains("linksys")
        || vendor_lower.contains("d-link")
        || vendor_lower.contains("mikrotik")
        || vendor_lower.contains("aruba")
        || vendor_lower.contains("ruckus")
        || vendor_lower.contains("fortinet")
        || vendor_lower.contains("palo alto")
        || vendor_lower.contains("zyxel")
        || vendor_lower.contains("draytek")
        || vendor_lower.contains("meraki")
        || vendor_lower.contains("cambium")
        || vendor_lower.contains("routerboard")
    {
        return Some("network_device");
    }

    // Server hardware
    if vendor_lower.contains("supermicro")
        || vendor_lower.contains("dell emc")
        || vendor_lower.contains("hpe")
        || vendor_lower.contains("hewlett packard enterprise")
        || vendor_lower.contains("ibm")
        || vendor_lower.contains("oracle")
        || vendor_lower.contains("fujitsu")
        || vendor_lower.contains("inspur")
    {
        return Some("server");
    }

    // Apple devices
    if vendor_lower.contains("apple") {
        return Some("apple");
    }

    // NAS/Storage
    if vendor_lower.contains("synology")
        || vendor_lower.contains("qnap")
        || vendor_lower.contains("western digital")
        || vendor_lower.contains("buffalo")
        || vendor_lower.contains("drobo")
        || vendor_lower.contains("netgear readynas")
        || vendor_lower.contains("ugreen")
        || vendor_lower.contains("asustor")
        || vendor_lower.contains("terramaster")
    {
        return Some("nas");
    }

    // Smart home / IoT
    if vendor_lower.contains("sonos")
        || vendor_lower.contains("philips")
        || vendor_lower.contains("signify")
        || vendor_lower.contains("ring")
        || vendor_lower.contains("nest")
        || vendor_lower.contains("ecobee")
        || vendor_lower.contains("wyze")
        || vendor_lower.contains("tuya")
        || vendor_lower.contains("shelly")
        || vendor_lower.contains("espressif")
        || vendor_lower.contains("amazon")
        || vendor_lower.contains("google")
        || vendor_lower.contains("roku")
        || vendor_lower.contains("wemo")
        || vendor_lower.contains("lifx")
        || vendor_lower.contains("nanoleaf")
    {
        return Some("iot");
    }

    // Printers
    if vendor_lower.contains("hewlett packard")
        || vendor_lower.contains("hp inc")
        || vendor_lower.contains("canon")
        || vendor_lower.contains("epson")
        || vendor_lower.contains("brother")
        || vendor_lower.contains("xerox")
        || vendor_lower.contains("lexmark")
        || vendor_lower.contains("ricoh")
        || vendor_lower.contains("konica")
        || vendor_lower.contains("kyocera")
    {
        return Some("printer");
    }

    // Gaming consoles
    if vendor_lower.contains("sony")
        || vendor_lower.contains("nintendo")
        || vendor_lower.contains("microsoft")
        || vendor_lower.contains("valve")
    {
        return Some("gaming");
    }

    // Mobile devices
    if vendor_lower.contains("samsung")
        || vendor_lower.contains("huawei")
        || vendor_lower.contains("xiaomi")
        || vendor_lower.contains("oneplus")
        || vendor_lower.contains("oppo")
        || vendor_lower.contains("vivo")
        || vendor_lower.contains("motorola")
        || vendor_lower.contains("lg electronics")
        || vendor_lower.contains("realme")
        || vendor_lower.contains("honor")
    {
        return Some("mobile");
    }

    // PC manufacturers
    if vendor_lower.contains("dell")
        || vendor_lower.contains("lenovo")
        || vendor_lower.contains("acer")
        || vendor_lower.contains("asus")
        || vendor_lower.contains("asustek")
        || vendor_lower.contains("intel")
        || vendor_lower.contains("realtek")
        || vendor_lower.contains("gigabyte")
        || vendor_lower.contains("msi")
        || vendor_lower.contains("hp ")
        || vendor_lower.contains("toshiba")
    {
        return Some("computer");
    }

    None
}

/// Infer device type from MAC address OUI prefix.
/// Some MAC prefixes indicate VMs or containers.
pub fn infer_device_type_from_mac(mac: &str) -> Option<&'static str> {
    let mac_normalized = mac.replace([':', '-', '.'], "").to_lowercase();

    if mac_normalized.len() < 6 {
        return None;
    }

    let prefix = &mac_normalized[..6];

    match prefix {
        // Docker containers
        "0242ac" => Some("service"),
        // VMware
        "005056" | "000c29" | "000569" => Some("service"),
        // Xen
        "00163e" => Some("service"),
        // Microsoft Hyper-V
        "00155d" => Some("service"),
        // Parallels
        "001c42" => Some("service"),
        // QEMU/KVM
        "525400" => Some("service"),
        // VirtualBox
        "080027" => Some("service"),
        // Proxmox VE
        "bc2411" => Some("service"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_device_type() {
        assert_eq!(
            infer_device_type("Cisco Systems, Inc."),
            Some("network_device")
        );
        assert_eq!(infer_device_type("Apple, Inc."), Some("apple"));
        assert_eq!(infer_device_type("Synology Incorporated"), Some("nas"));
        assert_eq!(infer_device_type("VMware, Inc."), Some("service"));
        assert_eq!(infer_device_type("Firewalla Inc."), Some("firewall"));
        assert_eq!(infer_device_type("Unknown Vendor"), None);
    }

    #[test]
    fn test_infer_device_type_from_mac() {
        assert_eq!(
            infer_device_type_from_mac("00:50:56:12:34:56"),
            Some("service")
        );
        assert_eq!(
            infer_device_type_from_mac("02:42:ac:12:34:56"),
            Some("service")
        );
        assert_eq!(infer_device_type_from_mac("00:17:F2:12:34:56"), None);
    }
}
