use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

/// Default cloud API URL
const DEFAULT_CLOUD_URL: &str = "https://cartographer.network/api";

/// Default dashboard URL (for opening in browser)
const DEFAULT_DASHBOARD_URL: &str = "https://cartographer.network";

/// Environment variable name for cloud URL override
const ENV_CLOUD_URL: &str = "CARTOGRAPHER_CLOUD_URL";

/// Configuration file structure
#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    cloud: Option<CloudConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct CloudConfig {
    /// API endpoint URL (e.g., "https://your-instance.example.com/api")
    api_url: Option<String>,
    /// Dashboard URL for browser links (e.g., "https://your-instance.example.com")
    dashboard_url: Option<String>,
}

/// Runtime cloud configuration
#[derive(Debug, Clone)]
pub struct CloudEndpointConfig {
    /// Base URL for API calls (e.g., "https://cartographer.network/api")
    pub api_url: String,
    /// Base URL for dashboard links (e.g., "https://cartographer.network")
    pub dashboard_url: String,
    /// Source of the configuration (for logging)
    pub source: ConfigSource,
}

/// Where the configuration came from
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSource {
    /// Using default hardcoded values
    Default,
    /// Loaded from environment variable
    Environment,
    /// Loaded from config file
    ConfigFile,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::Environment => write!(f, "environment variable"),
            ConfigSource::ConfigFile => write!(f, "config file"),
        }
    }
}

/// Get the path to the configuration file
fn get_config_file_path() -> Option<PathBuf> {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map(|p| p.join("cartographer").join("config.toml"))
}

/// Load configuration from the config file
fn load_config_file() -> Option<ConfigFile> {
    let path = get_config_file_path()?;

    if !path.exists() {
        return None;
    }

    match fs::read_to_string(&path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(config) => {
                tracing::debug!("Loaded config from {:?}", path);
                Some(config)
            }
            Err(e) => {
                tracing::warn!("Failed to parse config file {:?}: {}", path, e);
                None
            }
        },
        Err(e) => {
            tracing::warn!("Failed to read config file {:?}: {}", path, e);
            None
        }
    }
}

/// Load cloud endpoint configuration with priority:
/// 1. Environment variable (CARTOGRAPHER_CLOUD_URL)
/// 2. Config file (~/.config/cartographer/config.toml)
/// 3. Default values
pub fn load_cloud_config() -> CloudEndpointConfig {
    // Priority 1: Environment variable
    if let Ok(url) = std::env::var(ENV_CLOUD_URL) {
        let url = url.trim().trim_end_matches('/');
        if !url.is_empty() {
            tracing::info!(
                "Using cloud API URL from environment variable: {}",
                url
            );

            // Derive dashboard URL from API URL (strip /api suffix if present)
            let dashboard_url = url
                .strip_suffix("/api")
                .map(|s| s.to_string())
                .unwrap_or_else(|| url.to_string());

            return CloudEndpointConfig {
                api_url: url.to_string(),
                dashboard_url,
                source: ConfigSource::Environment,
            };
        }
    }

    // Priority 2: Config file
    if let Some(config) = load_config_file() {
        if let Some(cloud_config) = config.cloud {
            let api_url = cloud_config
                .api_url
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty());

            let dashboard_url = cloud_config
                .dashboard_url
                .map(|u| u.trim().trim_end_matches('/').to_string())
                .filter(|u| !u.is_empty());

            if let Some(api) = api_url {
                tracing::info!("Using cloud API URL from config file: {}", api);

                // Use dashboard URL from config or derive from API URL
                let dash = dashboard_url.unwrap_or_else(|| {
                    api.strip_suffix("/api")
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| api.clone())
                });

                return CloudEndpointConfig {
                    api_url: api,
                    dashboard_url: dash,
                    source: ConfigSource::ConfigFile,
                };
            }
        }
    }

    // Priority 3: Default values
    tracing::debug!("Using default cloud API URL: {}", DEFAULT_CLOUD_URL);
    CloudEndpointConfig {
        api_url: DEFAULT_CLOUD_URL.to_string(),
        dashboard_url: DEFAULT_DASHBOARD_URL.to_string(),
        source: ConfigSource::Default,
    }
}

/// Get the path to the config file for documentation purposes
pub fn get_config_file_path_string() -> String {
    get_config_file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "~/.config/cartographer/config.toml".to_string())
}

/// Generate example config file content
pub fn generate_example_config() -> String {
    r#"# Cartographer Agent Configuration
# Place this file at: ~/.config/cartographer/config.toml

[cloud]
# API endpoint URL for self-hosted cloud instances
# Default: https://cartographer.network/api
# api_url = "https://your-instance.example.com/api"

# Dashboard URL for browser links (optional, derived from api_url if not set)
# dashboard_url = "https://your-instance.example.com"
"#
    .to_string()
}
