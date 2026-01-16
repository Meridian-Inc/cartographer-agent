use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cartographer-agent")]
#[command(about = "Cartographer Agent - Network monitoring desktop application")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    
    /// Run in headless CLI mode (no GUI)
    #[arg(long)]
    pub headless: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Sign in and link to your cloud account
    Login,
    
    /// Run a network scan and upload results
    Scan,
    
    /// Show current connection status
    Status,
    
    /// Sign out and unlink agent
    Logout,
}

pub async fn run_cli() {
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Login) => {
            println!("Starting login flow...");
            // Use auth module directly since CLI doesn't have AppHandle for events
            // Print URL to stdout instead
            let emit_url = |event: crate::auth::LoginUrlEvent| {
                println!("Please visit: {}", event.verification_url);
                println!("Code: {}", event.user_code);
            };
            match crate::auth::start_login(Some(emit_url)).await {
                Ok(status) => {
                    if status.authenticated {
                        println!("✓ Successfully signed in as: {}",
                            status.user_email.unwrap_or_else(|| "Unknown".to_string()));
                    } else {
                        println!("✗ Login failed");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Scan) => {
            println!("Scanning network...");
            // Use scanner directly since CLI doesn't have AppHandle for events
            // Print progress to stdout instead
            let progress_callback: Box<dyn Fn(crate::scanner::ScanProgress) + Send + Sync> =
                Box::new(|progress| {
                    // Print progress to console
                    if let Some(pct) = progress.percent {
                        println!("  [{:>3}%] {}", pct, progress.message);
                    } else {
                        println!("  {}", progress.message);
                    }
                });

            match crate::scanner::scan_network_with_progress(Some(progress_callback)).await {
                Ok(scan_result) => {
                    println!("✓ Found {} devices", scan_result.devices.len());
                    for device in &scan_result.devices {
                        let hostname = device.hostname.as_deref().unwrap_or("-");
                        println!(
                            "  - {} ({:.1}ms) {}",
                            device.ip,
                            device.response_time_ms.unwrap_or(0.0),
                            hostname
                        );
                    }
                    // Upload to cloud if authenticated
                    if let Ok(status) = crate::auth::check_auth().await {
                        if status.authenticated {
                            let client = crate::cloud::CloudClient::new();
                            if let Err(e) = client.upload_scan_result(&scan_result).await {
                                eprintln!("⚠ Failed to upload: {}", e);
                            } else {
                                println!(
                                    "✓ Scan synced to cloud network '{}'",
                                    status.network_name.unwrap_or_else(|| "Unknown".to_string())
                                );
                            }
                        } else {
                            println!("⚠ Not signed in - scan not uploaded to cloud");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Status) => {
            match crate::commands::get_agent_status().await {
                Ok(status) => {
                    if status.authenticated {
                        println!("Status: Connected");
                        println!("Email: {}", status.user_email.unwrap_or_else(|| "Unknown".to_string()));
                        println!("Network: {}", status.network_name.unwrap_or_else(|| "Unknown".to_string()));
                    } else {
                        println!("Status: Not signed in");
                        println!("Run 'cartographer-agent login' to sign in");
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Logout) => {
            match crate::commands::logout().await {
                Ok(_) => {
                    println!("✓ Signed out successfully");
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        None => {
            // No command specified - check if headless mode
            if cli.headless {
                println!("Running in headless mode...");
                // Start background daemon
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                    if let Err(e) = crate::scanner::scan_network().await {
                        eprintln!("Scan error: {}", e);
                    }
                }
            } else {
                // Default: show help
                println!("Cartographer Agent");
                println!("Use --help for available commands");
            }
        }
    }
}

