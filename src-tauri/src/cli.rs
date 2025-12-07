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
            match crate::commands::start_login_flow().await {
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
            match crate::scanner::scan_network().await {
                Ok(devices) => {
                    println!("✓ Found {} devices", devices.len());
                    for device in devices {
                        println!("  - {} ({:.1}ms)", device.ip, 
                            device.response_time_ms.unwrap_or(0.0));
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
                        println!("Agent ID: {}", status.agent_id.unwrap_or_else(|| "Unknown".to_string()));
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

