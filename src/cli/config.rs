use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub hostname: String,
    pub port: u16,
    pub protocol: String,
    pub description: String,
    pub added_at: DateTime<Utc>,
    pub last_ping: Option<DateTime<Utc>>,
    pub status: ServerStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Up,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub servers: HashMap<String, ServerInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantInfo {
    pub display_name: String,
    pub description: String,
    pub server: String,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantConfig {
    pub tenants: HashMap<String, TenantInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub current_server: Option<String>,
    pub current_tenant: Option<String>,
    pub current_user: Option<String>,
    pub recents: Vec<String>,
}

impl ServerInfo {
    pub fn new(hostname: String, port: u16, protocol: String, description: String) -> Self {
        Self {
            hostname,
            port,
            protocol,
            description,
            added_at: Utc::now(),
            last_ping: None,
            status: ServerStatus::Unknown,
        }
    }

    pub fn url(&self) -> String {
        format!("{}://{}:{}", self.protocol, self.hostname, self.port)
    }

    pub fn update_ping(&mut self, status: ServerStatus) {
        self.last_ping = Some(Utc::now());
        self.status = status;
    }
}

impl TenantInfo {
    pub fn new(display_name: String, description: String, server: String) -> Self {
        Self {
            display_name,
            description,
            server,
            added_at: Utc::now(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }
}

impl Default for TenantConfig {
    fn default() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }
}

impl Default for EnvironmentConfig {
    fn default() -> Self {
        Self {
            current_server: None,
            current_tenant: None,
            current_user: None,
            recents: Vec::new(),
        }
    }
}

pub fn get_config_dir() -> anyhow::Result<PathBuf> {
    let config_dir = if let Ok(custom_dir) = std::env::var("MONK_CLI_CONFIG_DIR") {
        PathBuf::from(custom_dir)
    } else {
        let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME environment variable not set"))?;
        PathBuf::from(home).join(".config").join("monk").join("cli")
    };

    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }

    Ok(config_dir)
}

pub fn load_server_config() -> anyhow::Result<ServerConfig> {
    let config_dir = get_config_dir()?;
    let server_file = config_dir.join("server.json");

    if !server_file.exists() {
        return Ok(ServerConfig::default());
    }

    let content = fs::read_to_string(server_file)?;
    let config: ServerConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_server_config(config: &ServerConfig) -> anyhow::Result<()> {
    let config_dir = get_config_dir()?;
    let server_file = config_dir.join("server.json");

    let content = serde_json::to_string_pretty(config)?;
    fs::write(server_file, content)?;
    Ok(())
}

pub fn load_tenant_config() -> anyhow::Result<TenantConfig> {
    let config_dir = get_config_dir()?;
    let tenant_file = config_dir.join("tenant.json");

    if !tenant_file.exists() {
        return Ok(TenantConfig::default());
    }

    let content = fs::read_to_string(tenant_file)?;
    let config: TenantConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_tenant_config(config: &TenantConfig) -> anyhow::Result<()> {
    let config_dir = get_config_dir()?;
    let tenant_file = config_dir.join("tenant.json");

    let content = serde_json::to_string_pretty(config)?;
    fs::write(tenant_file, content)?;
    Ok(())
}

pub fn load_environment_config() -> anyhow::Result<EnvironmentConfig> {
    let config_dir = get_config_dir()?;
    let env_file = config_dir.join("env.json");

    if !env_file.exists() {
        return Ok(EnvironmentConfig::default());
    }

    let content = fs::read_to_string(env_file)?;
    let config: EnvironmentConfig = serde_json::from_str(&content)?;
    Ok(config)
}

pub fn save_environment_config(config: &EnvironmentConfig) -> anyhow::Result<()> {
    let config_dir = get_config_dir()?;
    let env_file = config_dir.join("env.json");

    let content = serde_json::to_string_pretty(config)?;
    fs::write(env_file, content)?;
    Ok(())
}

pub async fn ping_server(server_info: &ServerInfo) -> ServerStatus {
    let client = reqwest::Client::new();
    let url = format!("{}/health", server_info.url());
    
    match client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(response) if response.status().is_success() => ServerStatus::Up,
        _ => ServerStatus::Down,
    }
}