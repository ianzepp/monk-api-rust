use clap::Subcommand;
use serde_json::json;
use crate::cli::config::*;
use crate::cli::utils::*;
use crate::cli::OutputFormat;

#[derive(Subcommand)]
pub enum TenantCommands {
    #[command(about = "List all tenants")]
    List,
    
    #[command(about = "Show current tenant")]
    Current,
    
    #[command(about = "Switch to tenant")]
    Use {
        #[arg(help = "Tenant ID or name")]
        tenant: String,
    },
    
    #[command(about = "Create new tenant")]
    Create {
        #[arg(help = "Tenant name")]
        name: String,
    },
    
    #[command(about = "Delete tenant")]
    Delete {
        #[arg(help = "Tenant ID or name")]
        tenant: String,
    },
    
    #[command(about = "Show tenant information")]
    Info {
        #[arg(help = "Tenant ID or name")]
        tenant: Option<String>,
    },
}

pub async fn handle(cmd: TenantCommands, output_format: OutputFormat) -> anyhow::Result<()> {
    match cmd {
        TenantCommands::List => {
            let config = load_tenant_config()?;
            let env_config = load_environment_config()?;
            
            if config.tenants.is_empty() {
                return output_empty_collection(&output_format, "tenants", "No tenants configured");
            }
            
            match output_format {
                OutputFormat::Json => {
                    let tenants: Vec<_> = config.tenants.iter().map(|(name, info)| {
                        json!({
                            "name": name,
                            "display_name": info.display_name,
                            "description": info.description,
                            "server": info.server,
                            "added_at": info.added_at,
                            "current": env_config.current_tenant.as_ref() == Some(name)
                        })
                    }).collect();
                    println!("{}", serde_json::to_string_pretty(&json!({"tenants": tenants}))?);
                }
                OutputFormat::Text => {
                    println!("{:<15} {:<25} {:<15} {:<20} {}", "NAME", "DISPLAY NAME", "SERVER", "ADDED", "DESCRIPTION");
                    println!("{}", "-".repeat(90));
                    
                    for (name, info) in &config.tenants {
                        let current_marker = if env_config.current_tenant.as_ref() == Some(name) { "*" } else { " " };
                        let added_date = info.added_at.format("%Y-%m-%d %H:%M").to_string();
                        
                        println!("{}{:<14} {:<25} {:<15} {:<20} {}", 
                            current_marker, name, info.display_name, info.server, added_date, info.description);
                    }
                }
            }
            
            Ok(())
        }
        TenantCommands::Current => {
            let env_config = load_environment_config()?;
            
            match env_config.current_tenant {
                Some(tenant_name) => {
                    let config = load_tenant_config()?;
                    if let Some(tenant_info) = config.tenants.get(&tenant_name) {
                        let details = json!({
                            "name": tenant_name,
                            "display_name": tenant_info.display_name,
                            "server": tenant_info.server,
                            "description": tenant_info.description
                        });
                        output_current_item(&output_format, "tenant", &tenant_name, details)?;
                    } else {
                        return Err(anyhow::anyhow!("Current tenant '{}' not found in configuration", tenant_name));
                    }
                }
                None => {
                    output_no_current_item(&output_format, "tenant")?;
                }
            }
            
            Ok(())
        }
        TenantCommands::Use { tenant } => {
            switch_current_item(
                &tenant,
                "tenant",
                |name| Ok(load_tenant_config()?.tenants.contains_key(name)),
                |name| {
                    let mut env_config = load_environment_config()?;
                    env_config.current_tenant = Some(name.to_string());
                    save_environment_config(&env_config)
                },
                &output_format,
            )?;
            
            Ok(())
        }
        TenantCommands::Create { name } => {
            let mut config = load_tenant_config()?;
            let env_config = load_environment_config()?;
            
            if config.tenants.contains_key(&name) {
                return Err(anyhow::anyhow!("Tenant '{}' already exists", name));
            }
            
            // Get current server for new tenant
            let current_server = match env_config.current_server {
                Some(server) => server,
                None => return Err(anyhow::anyhow!("No current server set. Use 'monk server use <server>' first")),
            };
            
            let tenant_info = TenantInfo::new(
                name.clone(),
                format!("Created via CLI"),
                current_server,
            );
            
            config.tenants.insert(name.clone(), tenant_info);
            save_tenant_config(&config)?;
            
            output_success(
                &output_format,
                &format!("Tenant '{}' created successfully", name),
                Some(json!({ "tenant": name })),
            )?;
            
            Ok(())
        }
        TenantCommands::Delete { tenant } => {
            delete_item_with_current_check(
                &tenant,
                "tenant",
                |name| Ok(load_tenant_config()?.tenants.contains_key(name)),
                |name| {
                    let mut config = load_tenant_config()?;
                    config.tenants.remove(name);
                    save_tenant_config(&config)
                },
                |name| {
                    let mut env_config = load_environment_config()?;
                    if env_config.current_tenant.as_deref() == Some(name) {
                        env_config.current_tenant = None;
                        save_environment_config(&env_config)?;
                    }
                    Ok(())
                },
                &output_format,
            )?;
            
            Ok(())
        }
        TenantCommands::Info { tenant } => {
            let config = load_tenant_config()?;
            let env_config = load_environment_config()?;
            
            let target_tenant = match tenant {
                Some(tenant_name) => {
                    if !config.tenants.contains_key(&tenant_name) {
                        return Err(anyhow::anyhow!("Tenant '{}' not found", tenant_name));
                    }
                    tenant_name
                }
                None => {
                    match env_config.current_tenant {
                        Some(current) => current,
                        None => return Err(anyhow::anyhow!("No current tenant set")),
                    }
                }
            };
            
            let tenant_info = config.tenants.get(&target_tenant).unwrap();
            
            // Try to get server info too
            let server_config = load_server_config()?;
            let server_info = server_config.servers.get(&tenant_info.server);
            
            match output_format {
                OutputFormat::Json => {
                    let mut tenant_json = json!({
                        "name": target_tenant,
                        "display_name": tenant_info.display_name,
                        "description": tenant_info.description,
                        "server": tenant_info.server,
                        "added_at": tenant_info.added_at
                    });
                    
                    if let Some(server) = server_info {
                        tenant_json["server_info"] = json!({
                            "url": server.url(),
                            "status": server.status
                        });
                    }
                    
                    println!("{}", serde_json::to_string_pretty(&tenant_json)?);
                }
                OutputFormat::Text => {
                    println!("Tenant: {}", target_tenant);
                    println!("Display Name: {}", tenant_info.display_name);
                    if !tenant_info.description.is_empty() {
                        println!("Description: {}", tenant_info.description);
                    }
                    println!("Server: {}", tenant_info.server);
                    if let Some(server) = server_info {
                        println!("Server URL: {}", server.url());
                        let status_text = match server.status {
                            ServerStatus::Up => "🟢 UP",
                            ServerStatus::Down => "🔴 DOWN",
                            ServerStatus::Unknown => "⚪ UNKNOWN",
                        };
                        println!("Server Status: {}", status_text);
                    }
                    println!("Added: {}", tenant_info.added_at.format("%Y-%m-%d %H:%M:%S UTC"));
                }
            }
            
            Ok(())
        }
    }
}