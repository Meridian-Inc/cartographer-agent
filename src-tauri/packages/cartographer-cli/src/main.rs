//! Cartographer CLI - Lightweight network monitoring agent for Linux servers
//!
//! This binary provides a minimal footprint agent that can:
//! - Authenticate using OAuth device flow
//! - Scan the local network for devices
//! - Sync scan results to Cartographer Cloud
//! - Run as a background daemon (for systemd integration)

mod daemon;

use anyhow::Result;
use cartographer_core::{auth, cloud, scanner};
use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "cartographer")]
#[command(author = "Cartographer Team")]
#[command(version)]
#[command(about = "Lightweight network monitoring agent for Linux servers")]
#[command(long_about = "
Cartographer CLI is a lightweight agent for monitoring network devices
and syncing them to Cartographer Cloud. It's designed for headless
Linux servers where the full desktop agent isn't needed.

Quick start:
  1. Connect to cloud:  cartographer connect
  2. Run a scan:        cartographer scan --upload
  3. Start daemon:      cartographer daemon

For systemd integration, see: cartographer daemon --help
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Output format
    #[arg(short, long, global = true, default_value = "text")]
    pub format: OutputFormat,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output for scripting
    Json,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Authenticate with Cartographer Cloud using device flow
    #[command(alias = "login")]
    Connect,

    /// Run a network scan
    Scan {
        /// Upload scan results to cloud after scanning
        #[arg(short, long)]
        upload: bool,
    },

    /// Show connection status
    Status,

    /// Disconnect from Cartographer Cloud
    #[command(alias = "logout")]
    Disconnect,

    /// Run as a background scanning daemon
    Daemon {
        /// Scan interval in minutes
        #[arg(short, long, default_value = "5")]
        interval: u64,

        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },

    /// Show configuration paths and settings
    Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("cartographer={},cartographer_core={}", log_level, log_level).into()),
        )
        .with_target(false)
        .init();

    match cli.command {
        Commands::Connect => cmd_connect(&cli).await,
        Commands::Scan { upload } => cmd_scan(&cli, upload).await,
        Commands::Status => cmd_status(&cli).await,
        Commands::Disconnect => cmd_disconnect(&cli).await,
        Commands::Daemon { interval, foreground } => {
            daemon::run_daemon(interval, foreground).await
        }
        Commands::Config => cmd_config(&cli).await,
    }
}

async fn cmd_connect(cli: &Cli) -> Result<()> {
    // Check if already connected
    if let Ok(status) = auth::check_auth().await {
        if status.authenticated {
            match cli.format {
                OutputFormat::Text => {
                    println!("Already connected to '{}'", status.network_name.unwrap_or_default());
                    println!("Use 'cartographer disconnect' to sign out first.");
                }
                OutputFormat::Json => {
                    println!("{}", serde_json::json!({
                        "status": "already_connected",
                        "network_name": status.network_name,
                        "user_email": status.user_email,
                    }));
                }
            }
            return Ok(());
        }
    }

    match cli.format {
        OutputFormat::Text => println!("Starting authentication..."),
        OutputFormat::Json => {}
    }

    // Request device code
    let login_info = auth::request_login_url().await?;

    match cli.format {
        OutputFormat::Text => {
            println!();
            println!("Please visit the following URL to authorize:\n\n{}\n\n", login_info.verification_url);
        }
        OutputFormat::Json => {
            println!("{}", serde_json::json!({
                "status": "awaiting_authorization",
                "verification_url": login_info.verification_url,
                "user_code": login_info.user_code,
                "expires_in": login_info.expires_in,
            }));
        }
    }

    // Poll for completion
    let status = auth::poll_for_login(
        &login_info.device_code,
        login_info.expires_in,
        login_info.poll_interval,
    )
    .await?;

    match cli.format {
        OutputFormat::Text => {
            println!();
            println!("Connected to '{}' as {}",
                status.network_name.unwrap_or_default(),
                status.user_email.unwrap_or_default()
            );
            println!();
            println!("You can now run manual scans with: cartographer scan --upload");
        }
        OutputFormat::Json => {
            println!("{}", serde_json::json!({
                "status": "connected",
                "network_name": status.network_name,
                "network_id": status.network_id,
                "user_email": status.user_email,
            }));
        }
    }

    Ok(())
}

async fn cmd_scan(cli: &Cli, upload: bool) -> Result<()> {
    match cli.format {
        OutputFormat::Text => println!("Scanning network..."),
        OutputFormat::Json => {}
    }

    // Create progress callback for text mode
    let progress_callback: Option<scanner::ProgressCallback> = match cli.format {
        OutputFormat::Text => Some(Box::new(|progress: scanner::ScanProgress| {
            if let Some(pct) = progress.percent {
                println!("  [{:>3}%] {}", pct, progress.message);
            } else {
                println!("  {}", progress.message);
            }
        })),
        OutputFormat::Json => None,
    };

    let scan_result = scanner::scan_network_with_progress(progress_callback).await?;

    match cli.format {
        OutputFormat::Text => {
            println!();
            println!("Found {} devices:", scan_result.devices.len());
            println!();
            for device in &scan_result.devices {
                let hostname = device.hostname.as_deref().unwrap_or("-");
                let vendor = device.vendor.as_deref().unwrap_or("");
                let time_str = device
                    .response_time_ms
                    .map(|t| format!("{:.1}ms", t))
                    .unwrap_or_else(|| "-".to_string());

                if vendor.is_empty() {
                    println!("  {:15} {:>8}  {}", device.ip, time_str, hostname);
                } else {
                    println!("  {:15} {:>8}  {} ({})", device.ip, time_str, hostname, vendor);
                }
            }
        }
        OutputFormat::Json => {
            if !upload {
                println!("{}", serde_json::json!({
                    "devices": scan_result.devices,
                    "network_info": {
                        "interface": scan_result.network_info.interface,
                        "subnet": scan_result.network_info.subnet,
                        "gateway_ip": scan_result.network_info.gateway_ip,
                        "local_ip": scan_result.network_info.local_ip,
                    },
                    "uploaded": false,
                }));
            }
        }
    }

    // Upload to cloud if requested
    if upload {
        match auth::check_auth().await {
            Ok(status) if status.authenticated => {
                match cli.format {
                    OutputFormat::Text => println!("\nUploading to cloud..."),
                    OutputFormat::Json => {}
                }

                let client = cloud::CloudClient::new();
                match client.upload_scan_result(&scan_result).await {
                    Ok(_) => {
                        match cli.format {
                            OutputFormat::Text => {
                                println!("Synced to network '{}'",
                                    status.network_name.unwrap_or_default()
                                );
                            }
                            OutputFormat::Json => {
                                println!("{}", serde_json::json!({
                                    "devices": scan_result.devices,
                                    "network_info": {
                                        "interface": scan_result.network_info.interface,
                                        "subnet": scan_result.network_info.subnet,
                                        "gateway_ip": scan_result.network_info.gateway_ip,
                                        "local_ip": scan_result.network_info.local_ip,
                                    },
                                    "uploaded": true,
                                    "network_name": status.network_name,
                                }));
                            }
                        }
                    }
                    Err(e) => {
                        match cli.format {
                            OutputFormat::Text => eprintln!("Upload failed: {}", e),
                            OutputFormat::Json => {
                                println!("{}", serde_json::json!({
                                    "devices": scan_result.devices,
                                    "uploaded": false,
                                    "upload_error": e.to_string(),
                                }));
                            }
                        }
                    }
                }
            }
            _ => {
                match cli.format {
                    OutputFormat::Text => {
                        println!();
                        println!("Not connected to cloud. Run 'cartographer connect' first.");
                    }
                    OutputFormat::Json => {
                        println!("{}", serde_json::json!({
                            "devices": scan_result.devices,
                            "uploaded": false,
                            "upload_error": "Not authenticated",
                        }));
                    }
                }
            }
        }
    }

    Ok(())
}

async fn cmd_status(cli: &Cli) -> Result<()> {
    let auth_status = auth::check_auth().await?;

    match cli.format {
        OutputFormat::Text => {
            if auth_status.authenticated {
                println!("Status: Connected");
                println!("Email:  {}", auth_status.user_email.unwrap_or_else(|| "-".to_string()));
                println!("Network: {} ({})",
                    auth_status.network_name.unwrap_or_else(|| "-".to_string()),
                    auth_status.network_id.unwrap_or_else(|| "-".to_string())
                );
                println!();
                println!("Storage: {}", auth::get_credential_storage_info());
            } else {
                println!("Status: Not connected");
                println!();
                println!("Run 'cartographer connect' to authenticate.");
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::json!({
                "authenticated": auth_status.authenticated,
                "user_email": auth_status.user_email,
                "network_id": auth_status.network_id,
                "network_name": auth_status.network_name,
                "storage_info": auth::get_credential_storage_info(),
            }));
        }
    }

    Ok(())
}

async fn cmd_disconnect(cli: &Cli) -> Result<()> {
    // Check if connected
    let auth_status = auth::check_auth().await?;

    if !auth_status.authenticated {
        match cli.format {
            OutputFormat::Text => println!("Not connected."),
            OutputFormat::Json => {
                println!("{}", serde_json::json!({
                    "status": "not_connected",
                }));
            }
        }
        return Ok(());
    }

    auth::delete_credentials().await?;

    match cli.format {
        OutputFormat::Text => {
            println!("Disconnected from '{}'",
                auth_status.network_name.unwrap_or_else(|| "cloud".to_string())
            );
        }
        OutputFormat::Json => {
            println!("{}", serde_json::json!({
                "status": "disconnected",
                "network_name": auth_status.network_name,
            }));
        }
    }

    Ok(())
}

async fn cmd_config(cli: &Cli) -> Result<()> {
    let cloud_config = cloud::load_cloud_config();
    let config_path = cloud::config::get_config_file_path_string();

    match cli.format {
        OutputFormat::Text => {
            println!("Configuration");
            println!("=============");
            println!();
            println!("Config file:      {}", config_path);
            println!("API endpoint:     {} (from {})", cloud_config.api_url, cloud_config.source);
            println!("Dashboard URL:    {}", cloud_config.dashboard_url);
            println!("Credential store: {}", auth::get_credential_storage_info());
            println!();
            println!("Environment variables:");
            println!("  CARTOGRAPHER_CLOUD_URL - Override API endpoint");
            println!();
            println!("Example config.toml:");
            println!();
            println!("{}", cloud::config::generate_example_config());
        }
        OutputFormat::Json => {
            println!("{}", serde_json::json!({
                "config_file": config_path,
                "api_url": cloud_config.api_url,
                "api_source": format!("{}", cloud_config.source),
                "dashboard_url": cloud_config.dashboard_url,
                "credential_storage": auth::get_credential_storage_info(),
            }));
        }
    }

    Ok(())
}
