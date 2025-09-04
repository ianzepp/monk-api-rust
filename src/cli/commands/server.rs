use clap::Subcommand;
use serde_json::json;
use url::Url;
use crate::cli::config::*;
use crate::cli::utils::*;
use crate::cli::OutputFormat;

#[derive(Subcommand)]
pub enum ServerCommands {
    #[command(about = "Register remote server")]
    Add {
        #[arg(help = "Server URL")]
        url: String,
        #[arg(help = "Server name")]
        name: Option<String>,
    },
    
    #[command(about = "List all servers with health status")]
    List,
    
    #[command(about = "Show currently selected server")]
    Current,
    
    #[command(about = "Switch to server (persistent selection) or show current server")]
    Use {
        #[arg(help = "Server name to switch to")]
        name: Option<String>,
    },
    
    #[command(about = "Remove server from registry")]
    Delete {
        #[arg(help = "Server name to delete")]
        name: String,
    },
    
    #[command(about = "Health check specific server (defaults to current server)")]
    Ping {
        #[arg(help = "Server name to ping")]
        name: Option<String>,
    },
    
    #[command(about = "Health check all registered servers")]
    PingAll,
    
    #[command(about = "Show server information from API root endpoint")]
    Info {
        #[arg(help = "Server name")]
        name: Option<String>,
    },
    
    #[command(about = "Check server health status from API /health endpoint")]
    Health {
        #[arg(help = "Server name")]
        name: Option<String>,
    },
}

pub async fn handle(cmd: ServerCommands, output_format: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        ServerCommands::Add { url, name } => {
            let parsed_url = Url::parse(&url)?;
            let hostname = parsed_url.host_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid hostname in URL"))?
                .to_string();
            let port = parsed_url.port()
                .unwrap_or_else(|| if parsed_url.scheme() == "https" { 443 } else { 80 });
            let protocol = parsed_url.scheme().to_string();
            
            let server_name = name.unwrap_or_else(|| hostname.clone());
            
            let mut config = load_server_config()?;
            
            if config.servers.contains_key(&server_name) {
                return Err(anyhow::anyhow!("Server '{}' already exists", server_name));
            }
            
            let server_info = ServerInfo::new(
                hostname,
                port,
                protocol,
                format!("Added via CLI"),
            );
            
            config.servers.insert(server_name.clone(), server_info);
            save_server_config(&config)?;
            
            output_success(
                &output_format,
                &format!("Server '{}' added successfully", server_name),
                Some(json!({ "server": server_name })),
            )?;
            
            Ok(())
        }
        ServerCommands::List => {
            let config = load_server_config()?;
            let env_config = load_environment_config()?;
            
            if config.servers.is_empty() {
                return output_empty_collection(&output_format, "servers", "No servers configured");
            }
            
            match output_format {
                OutputFormat::Json => {
                    let servers: Vec<_> = config.servers.iter().map(|(name, info)| {
                        json!({
                            "name": name,
                            "url": info.url(),
                            "status": info.status,
                            "description": info.description,
                            "last_ping": info.last_ping,
                            "current": env_config.current_server.as_ref() == Some(name)
                        })
                    }).collect();
                    println!("{}", serde_json::to_string_pretty(&json!({"servers": servers}))?);
                }
                OutputFormat::Text => {
                    println!("{:<12} {:<25} {:<8} {:<20} {}", "NAME", "URL", "STATUS", "LAST PING", "DESCRIPTION");
                    println!("{}", "-".repeat(80));
                    
                    for (name, info) in &config.servers {
                        let current_marker = if env_config.current_server.as_ref() == Some(name) { "*" } else { " " };
                        let status = match info.status {
                            ServerStatus::Up => "🟢 up",
                            ServerStatus::Down => "🔴 down",
                            ServerStatus::Unknown => "⚪ unknown",
                        };
                        let last_ping = info.last_ping
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "never".to_string());
                        
                        println!("{}{:<11} {:<25} {:<8} {:<20} {}", 
                            current_marker, name, info.url(), status, last_ping, info.description);
                    }
                }
            }
            
            Ok(())
        }
        ServerCommands::Current => {
            let env_config = load_environment_config()?;
            
            match env_config.current_server {
                Some(server_name) => {
                    let config = load_server_config()?;
                    if let Some(server_info) = config.servers.get(&server_name) {
                        let details = json!({
                            "name": server_name,
                            "url": server_info.url(),
                            "status": server_info.status,
                            "description": server_info.description
                        });
                        output_current_item(&output_format, "server", &server_name, details)?;
                    } else {
                        return Err(anyhow::anyhow!("Current server '{}' not found in configuration", server_name));
                    }
                }
                None => {
                    output_no_current_item(&output_format, "server")?;
                }
            }
            
            Ok(())
        }
        ServerCommands::Use { name } => {
            match name {
                Some(server_name) => {
                    switch_current_item(
                        &server_name,
                        "server",
                        |name| Ok(load_server_config()?.servers.contains_key(name)),
                        |name| {
                            let mut env_config = load_environment_config()?;
                            env_config.current_server = Some(name.to_string());
                            save_environment_config(&env_config)
                        },
                        &output_format,
                    )?;
                }
                None => {
                    // Show current server (same as ServerCommands::Current)
                    let env_config = load_environment_config()?;
                    
                    match env_config.current_server {
                        Some(server_name) => {
                            let config = load_server_config()?;
                            if let Some(server_info) = config.servers.get(&server_name) {
                                match output_format {
                                    OutputFormat::Json => {
                                        println!("{}", serde_json::to_string_pretty(&json!({
                                            "current_server": {
                                                "name": server_name,
                                                "url": server_info.url(),
                                                "status": server_info.status,
                                                "description": server_info.description
                                            }
                                        }))?);
                                    }
                                    OutputFormat::Text => {
                                        println!("Current server: {} ({})", server_name, server_info.url());
                                    }
                                }
                            }
                        }
                        None => {
                            match output_format {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string_pretty(&json!({"current_server": null}))?);
                                }
                                OutputFormat::Text => {
                                    println!("No current server set");
                                }
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        ServerCommands::Delete { name } => {
            delete_item_with_current_check(
                &name,
                "server",
                |name| Ok(load_server_config()?.servers.contains_key(name)),
                |name| {
                    let mut config = load_server_config()?;
                    config.servers.remove(name);
                    save_server_config(&config)
                },
                |name| {
                    let mut env_config = load_environment_config()?;
                    if env_config.current_server.as_deref() == Some(name) {
                        env_config.current_server = None;
                        save_environment_config(&env_config)?;
                    }
                    Ok(())
                },
                &output_format,
            )?;
            
            Ok(())
        }
        ServerCommands::Ping { name } => {
            let config = load_server_config()?;
            let env_config = load_environment_config()?;
            
            let target_server = match name {
                Some(server_name) => {
                    if !config.servers.contains_key(&server_name) {
                        return Err(anyhow::anyhow!("Server '{}' not found", server_name));
                    }
                    server_name
                }
                None => {
                    match env_config.current_server {
                        Some(current) => current,
                        None => return Err(anyhow::anyhow!("No current server set")),
                    }
                }
            };
            
            let server_info = config.servers.get(&target_server).unwrap().clone();
            let status = ping_server(&server_info).await;
            
            // Update the server status in config
            let mut updated_config = config;
            if let Some(server) = updated_config.servers.get_mut(&target_server) {
                server.update_ping(status.clone());
            }
            save_server_config(&updated_config)?;
            
            match output_format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "server": target_server,
                        "url": server_info.url(),
                        "status": status,
                        "timestamp": chrono::Utc::now()
                    }))?);
                }
                OutputFormat::Text => {
                    let status_text = match status {
                        ServerStatus::Up => "🟢 UP",
                        ServerStatus::Down => "🔴 DOWN",
                        ServerStatus::Unknown => "⚪ UNKNOWN",
                    };
                    println!("{} {} ({})", status_text, target_server, server_info.url());
                }
            }
            
            Ok(())
        }
        ServerCommands::PingAll => {
            let config = load_server_config()?;
            
            if config.servers.is_empty() {
                match output_format {
                    OutputFormat::Json => {
                        println!("{}", serde_json::to_string_pretty(&json!({"servers": []}))?);
                    }
                    OutputFormat::Text => {
                        println!("No servers configured");
                    }
                }
                return Ok(());
            }
            
            let mut results = Vec::new();
            
            // Collect server info first to avoid borrowing issues
            let server_list: Vec<_> = config.servers.iter()
                .map(|(name, info)| (name.clone(), info.clone()))
                .collect();
            
            for (name, server_info) in server_list {
                let status = ping_server(&server_info).await;
                results.push((name.clone(), server_info.url(), status.clone()));
            }
            
            // Update all statuses after collecting results
            let mut updated_config = config;
            for (name, _, status) in &results {
                if let Some(server) = updated_config.servers.get_mut(name) {
                    server.update_ping(status.clone());
                }
            }
            
            save_server_config(&updated_config)?;
            
            match output_format {
                OutputFormat::Json => {
                    let json_results: Vec<_> = results.iter().map(|(name, url, status)| {
                        json!({
                            "server": name,
                            "url": url,
                            "status": status
                        })
                    }).collect();
                    println!("{}", serde_json::to_string_pretty(&json!({"results": json_results}))?);
                }
                OutputFormat::Text => {
                    println!("Ping results:");
                    for (name, url, status) in results {
                        let status_text = match status {
                            ServerStatus::Up => "🟢 UP",
                            ServerStatus::Down => "🔴 DOWN",
                            ServerStatus::Unknown => "⚪ UNKNOWN",
                        };
                        println!("{} {} ({})", status_text, name, url);
                    }
                }
            }
            
            Ok(())
        }
        ServerCommands::Info { name } => {
            let config = load_server_config()?;
            let env_config = load_environment_config()?;
            
            let target_server = match name {
                Some(server_name) => {
                    if !config.servers.contains_key(&server_name) {
                        return Err(anyhow::anyhow!("Server '{}' not found", server_name));
                    }
                    server_name
                }
                None => {
                    match env_config.current_server {
                        Some(current) => current,
                        None => return Err(anyhow::anyhow!("No current server set")),
                    }
                }
            };
            
            let server_info = config.servers.get(&target_server).unwrap();
            let client = reqwest::Client::new();
            let url = server_info.url();
            
            match client.get(&url).timeout(std::time::Duration::from_secs(10)).send().await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<serde_json::Value>().await {
                        Ok(info) => {
                            match output_format {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string_pretty(&json!({
                                        "server": target_server,
                                        "url": url,
                                        "info": info
                                    }))?);
                                }
                                OutputFormat::Text => {
                                    println!("Server: {} ({})", target_server, url);
                                    println!("Info: {}", serde_json::to_string_pretty(&info)?);
                                }
                            }
                        }
                        Err(_) => {
                            return Err(anyhow::anyhow!("Server responded but returned invalid JSON"));
                        }
                    }
                }
                Ok(response) => {
                    return Err(anyhow::anyhow!("Server responded with status: {}", response.status()));
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Failed to connect to server: {}", e));
                }
            }
            
            Ok(())
        }
        ServerCommands::Health { name } => {
            let config = load_server_config()?;
            let env_config = load_environment_config()?;
            
            let target_server = match name {
                Some(server_name) => {
                    if !config.servers.contains_key(&server_name) {
                        return Err(anyhow::anyhow!("Server '{}' not found", server_name));
                    }
                    server_name
                }
                None => {
                    match env_config.current_server {
                        Some(current) => current,
                        None => return Err(anyhow::anyhow!("No current server set")),
                    }
                }
            };
            
            let server_info = config.servers.get(&target_server).unwrap();
            let client = reqwest::Client::new();
            let health_url = format!("{}/health", server_info.url());
            
            match client.get(&health_url).timeout(std::time::Duration::from_secs(10)).send().await {
                Ok(response) if response.status().is_success() => {
                    match response.json::<serde_json::Value>().await {
                        Ok(health) => {
                            match output_format {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string_pretty(&json!({
                                        "server": target_server,
                                        "url": server_info.url(),
                                        "health_endpoint": health_url,
                                        "status": "healthy",
                                        "details": health
                                    }))?);
                                }
                                OutputFormat::Text => {
                                    println!("🟢 {} is healthy", target_server);
                                    println!("Health details: {}", serde_json::to_string_pretty(&health)?);
                                }
                            }
                        }
                        Err(_) => {
                            match output_format {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string_pretty(&json!({
                                        "server": target_server,
                                        "url": server_info.url(),
                                        "status": "unhealthy",
                                        "error": "Invalid health response"
                                    }))?);
                                }
                                OutputFormat::Text => {
                                    println!("🔴 {} is unhealthy (invalid response)", target_server);
                                }
                            }
                        }
                    }
                }
                Ok(response) => {
                    match output_format {
                        OutputFormat::Json => {
                            println!("{}", serde_json::to_string_pretty(&json!({
                                "server": target_server,
                                "url": server_info.url(),
                                "status": "unhealthy",
                                "http_status": response.status().as_u16()
                            }))?);
                        }
                        OutputFormat::Text => {
                            println!("🔴 {} is unhealthy (HTTP {})", target_server, response.status());
                        }
                    }
                }
                Err(e) => {
                    match output_format {
                        OutputFormat::Json => {
                            println!("{}", serde_json::to_string_pretty(&json!({
                                "server": target_server,
                                "url": server_info.url(),
                                "status": "unreachable",
                                "error": e.to_string()
                            }))?);
                        }
                        OutputFormat::Text => {
                            println!("🔴 {} is unreachable: {}", target_server, e);
                        }
                    }
                }
            }
            
            Ok(())
        }
    }
}