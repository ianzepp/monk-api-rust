pub mod commands;
pub mod config;
pub mod utils;

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "monk")]
#[command(about = "Monk CLI - Command-line interface for PaaS Backend API")]
#[command(version)]
pub struct Cli {
    #[arg(long, global = true, help = "Output in human-readable text format")]
    pub text: bool,

    #[arg(long, global = true, help = "Output in JSON format")]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Initialize configuration directory with required files")]
    Init {
        #[command(subcommand)]
        cmd: commands::init::InitCommands,
    },
    
    #[command(about = "Remote server management")]
    Server {
        #[command(subcommand)]
        cmd: commands::server::ServerCommands,
    },
    
    
    #[command(about = "Authentication and token management")]
    Auth {
        #[command(subcommand)]
        cmd: commands::auth::AuthCommands,
    },
    
    #[command(about = "Data operations on dynamic schemas")]
    Data {
        #[command(subcommand)]
        cmd: commands::data::DataCommands,
    },
    
    #[command(about = "Schema and metadata management")]
    Describe {
        #[command(subcommand)]
        cmd: commands::describe::DescribeCommands,
    },
    
    #[command(about = "Fixture data generation, building, and deployment")]
    Fixture {
        #[command(subcommand)]
        cmd: commands::fixture::FixtureCommands,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    pub fn from_cli(cli: &Cli) -> Self {
        if cli.json {
            OutputFormat::Json
        } else {
            OutputFormat::Text
        }
    }
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let output_format = OutputFormat::from_cli(&cli);
    
    match cli.command {
        Commands::Init { cmd } => commands::init::handle(cmd, output_format).await,
        Commands::Server { cmd } => commands::server::handle(cmd, output_format).await,
        Commands::Auth { cmd } => commands::auth::handle(cmd, output_format).await,
        Commands::Data { cmd } => commands::data::handle(cmd, output_format).await,
        Commands::Describe { cmd } => commands::describe::handle(cmd, output_format).await,
        Commands::Fixture { cmd } => commands::fixture::handle(cmd, output_format).await,
    }
}