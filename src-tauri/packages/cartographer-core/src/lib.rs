//! Cartographer Core Library
//!
//! This crate provides the core functionality for Cartographer agents:
//! - Network scanning (ARP, ping sweep, hostname resolution)
//! - Cloud synchronization (device code auth, scan upload)
//! - Credential management (keyring with file fallback)
//!
//! # Features
//!
//! - `keyring-storage` (default): Use platform keyring for credential storage
//! - `file-storage`: Use file-based credential storage (for headless Linux)
//! - `browser`: Automatically open browser during OAuth device flow
//!
//! # Example
//!
//! ```no_run
//! use cartographer_core::{auth, cloud, scanner};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // Authenticate using device flow
//!     let login_info = auth::request_login_url().await?;
//!     println!("Visit: {}", login_info.verification_url);
//!
//!     // Poll for completion
//!     let status = auth::poll_for_login(
//!         &login_info.device_code,
//!         login_info.expires_in,
//!         login_info.poll_interval
//!     ).await?;
//!
//!     // Scan network
//!     let result = scanner::scan_network().await?;
//!     println!("Found {} devices", result.devices.len());
//!
//!     // Upload to cloud
//!     let client = cloud::CloudClient::new();
//!     client.upload_scan_result(&result).await?;
//!
//!     Ok(())
//! }
//! ```

pub mod auth;
pub mod cloud;
pub mod scanner;

// Re-export commonly used types
pub use auth::{AuthStatus, Credentials, LoginFlowStarted, LoginUrlEvent};
pub use cloud::{CloudClient, CloudEndpointConfig, ConfigSource, TokenVerifyResult};
pub use scanner::{Device, NetworkInfo, ScanCapabilities, ScanProgress, ScanResult, ScanStage};
